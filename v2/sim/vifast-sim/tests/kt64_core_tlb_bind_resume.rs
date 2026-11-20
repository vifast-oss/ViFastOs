// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-64 — test **core** cho bốn mảng của K3.5 tầng 3: `TlbFlush` thật (cầu nối hook),
//! `SlotPool` → arena (một nguồn sự thật cho byte slot), `SchedContext.Bind`/`Unbind` (hết
//! “trường chết” `bound_sc`), và `tcb_resume_with_regs` (một cửa resume sau fault).
//!
//! Parity sim↔core của **Bind** + **resume_with_regs** nằm ở `kt64_core_parity.rs`; tệp này pin
//! phần **hành vi core-only** (không mô hình hoá được ở sim):
//!
//! - `TlbFlush`: sim K1–K2 chỉ có **mô hình** TLB (không `sfence.vma`, không IPI thật) và
//!   không có VSpace trong core (D2-83) ⇒ flush thật đi qua hook [`TlbEnv`]; test dùng env giả
//!   để chứng minh: luôn flush **cục bộ** (kể cả 1 hart), chỉ post tới `active_on`, chờ **đủ**
//!   ack, và `dispatch` **từ chối** chạy TCB trên hart còn flush chưa xử lý (chống đọc trang cũ).
//! - `SlotPool` → arena: slot CNode **Retype từ Untyped** nằm trong **byte arena** tại
//!   `offset + cnode_base` (probe PA), `Revoke` trả byte, `check_bytes` không đổi. CNode do
//!   harness cấp (như `add_cnode` của sim) không có byte arena — dùng cửa sổ riêng, ghi rõ.
//! - `Bind`: 1:1 hai chiều SC↔Notification, mã lỗi như sim; `Signal` chỉ đánh thức TCB **đúng**
//!   bound; `Delete`/`Revoke` gỡ binding (không để `bound_sc` mồ côi).
//! - `resume_with_regs`: chỉ nhận `BlockedOnReply` (đúng caller), `BlockedOnSend{call, fault≠Exit}`,
//!   `Ready`, `Sleeping`; trạng thái khác ⇒ `IllegalOperation` (không mở đường user tự resume).

use kc::{
    FaultKind, KError, Machine, Message, MessageInfo, NonReadyTarget, ObjId, ObjType, SleepReason,
    SlotRef, TcbState, TlbEnv, TlbScope,
};
use vifast_core as kc;

fn slot(c: ObjId, s: usize) -> SlotRef {
    SlotRef::new(c, s)
}

fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("info hợp lệ");
    Message::new(info, words).expect("message hợp lệ")
}

/// `ObjId` của object trong slot (cửa công khai của core).
fn obj_at(m: &Machine, c: ObjId, s: usize) -> ObjId {
    m.cap_obj_at(c, s).expect("slot phải có cap sống")
}

// -------------------------------------------------------------------------------------------
// TlbEnv giả: đếm lần flush **theo hart** + mask `active_on` điều khiển được.
// -------------------------------------------------------------------------------------------

#[derive(Default)]
struct TestTlb {
    active: u64,
    generation: u64,
    flushes: std::vec::Vec<(u16, TlbScope, u16, u64)>,
}

impl TlbEnv for TestTlb {
    fn active_on(&self) -> u64 {
        self.active
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn flush_local(&mut self, hart: u16, scope: TlbScope, asid: u16, va: u64) {
        self.flushes.push((hart, scope, asid, va));
    }
}

// -------------------------------------------------------------------------------------------
// Mảng 1 — TlbFlush thật (hook TlbEnv; sim chỉ mô hình hoá)
// -------------------------------------------------------------------------------------------

/// 1 hart: **luôn** flush cục bộ, không post, không chờ; trang cũ không còn trong TLB.
#[test]
fn kt64_tlb_mot_hart_flush_cuc_bo_va_khong_post() {
    let mut m = Machine::with_harts(1);
    let mut env = TestTlb {
        active: 1,
        generation: 3,
        flushes: std::vec::Vec::new(),
    };
    let r = m
        .tlb_flush(&mut env, TlbScope::Page { va: 0x4000 }, 7, 0x4000)
        .expect("tlb_flush 1 hart");
    assert_eq!(env.flushes.len(), 1, "một hart vẫn phải flush cục bộ");
    assert_eq!(env.flushes[0].0, 0, "flush trên hart gọi");
    assert_eq!(env.flushes[0].1, TlbScope::Page { va: 0x4000 });
    assert_eq!(env.flushes[0].2, 7, "asid");
    assert_eq!(r.targets, 0, "không có hart đích");
    assert_eq!(r.acked, r.targets, "ack đủ ngay");
    assert_eq!(r.seq, 1, "seq 1-based");
    assert_eq!(
        m.mail_len(0).unwrap(),
        0,
        "không post TlbFlush cho chính mình"
    );
    assert!(m.tlb_pending(0).unwrap().is_none(), "không còn chờ");
    assert!(m.check_mailboxes().is_ok());
}

/// 4 hart, `active_on = 0b1111`: post cho 3 hart khác, `dispatch` **từ chối** chạy TCB trên
/// hart còn flush chưa xử lý, `tlb_service` xử lý rồi `dispatch` chạy được (chống trang cũ).
#[test]
fn kt64_tlb_nhieu_hart_chan_dispatch_va_cho_du_ack() {
    let mut m = Machine::with_harts(4);
    let mut env = TestTlb {
        active: 0b1111,
        generation: 5,
        flushes: std::vec::Vec::new(),
    };
    m.set_current_hart(0).unwrap();
    // Một TCB Ready trên hart 1 **có budget** — `dispatch(1)` phải bị chặn khi còn flush chưa
    // xử lý (và chạy được ngay sau khi flush xong: chứng minh deny là do TLB, không do hàng rỗng).
    let u = m.add_untyped(0x1000, 1 << 14);
    let c0 = m.add_cnode(16);
    let ucap = m.root_cap(u);
    m.put_cap(c0, 0, ucap).expect("cap untyped");
    let ctl_cap = m
        .retype(slot(c0, 0), ObjType::SchedControl, 0, 0)
        .expect("retype SchedControl");
    m.put_cap(c0, 1, ctl_cap).expect("cap ctl");
    let sc_cap = m
        .retype(slot(c0, 0), ObjType::SchedContext, 0, 0)
        .expect("retype SC");
    m.put_cap(c0, 2, sc_cap).expect("cap sc");
    m.sched_control_configure(slot(c0, 1), slot(c0, 2), 4, 8)
        .expect("configure budget");
    let t = m.add_kernel_object(ObjType::Tcb);
    let tc = m.root_cap(t);
    m.put_cap(c0, 3, tc).expect("cap tcb");
    let ts = slot(c0, 3);
    m.set_sched_params(ts, Some(slot(c0, 2))).expect("gắn SC");
    m.tcb_set_affinity(ts, 0b11).expect("affinity 0|1");
    m.tcb_set_home(ts, 1).expect("home hart 1");
    m.start_tcb(t).expect("Ready");
    let seq = m
        .tlb_shootdown_begin(&mut env, TlbScope::Asid, 9, 0)
        .expect("begin");
    assert_eq!(seq, 1);
    assert_eq!(env.flushes.len(), 1, "flush cục bộ **trước** khi post");
    assert_eq!(env.flushes[0].0, 0);
    for h in 1..4u16 {
        assert_eq!(m.mail_len(h).unwrap(), 1, "post TlbFlush cho hart {h}");
        assert_eq!(m.tlb_pending(h).unwrap(), Some(seq), "hart {h} chờ ack");
    }
    // `dispatch` tôn trọng nhà làm sạch: hart 1 chưa xử lý ⇒ Busy (không chạy TCB trên ASID cũ).
    assert_eq!(m.dispatch(1).err(), Some(KError::Busy));
    // Hart 1 tự phục vụ: flush thật trên hart 1 rồi mới cho chạy.
    assert_eq!(m.tlb_service(&mut env, 1).unwrap(), 1);
    assert_eq!(env.flushes.len(), 2);
    assert_eq!(env.flushes[1].0, 1, "flush trên hart 1");
    assert!(m.tlb_pending(1).unwrap().is_none());
    assert_eq!(m.dispatch(1).unwrap(), Some(t), "dispatch chạy sau flush");
    // D2-92b — hart gọi **không** ack hộ: hart 2, 3 phải tự chạy `tlb_service` rồi `wait` mới xong.
    assert_eq!(m.tlb_service(&mut env, 2).unwrap(), 1, "hart 2 flush thật");
    assert_eq!(m.tlb_service(&mut env, 3).unwrap(), 1, "hart 3 flush thật");
    m.tlb_shootdown_wait(&mut env, 0).expect("wait đủ ack");
    let mut harts: std::vec::Vec<u16> = env.flushes.iter().map(|f| f.0).collect();
    harts.sort_unstable();
    assert_eq!(
        harts,
        std::vec![0, 1, 2, 3],
        "mỗi hart trong active_on flush đúng một lần"
    );
    assert!(m.tlb_pending(0).unwrap().is_none(), "xong thì hết chờ");
    for h in 0..4u16 {
        assert_eq!(m.mail_len(h).unwrap(), 0, "hộp thư hart {h} rỗng");
        assert!(m.check_mailboxes().is_ok());
    }
}

/// Hart **ngoài** `active_on` không bị chờ/post; hart gọi ngoài `active_on` ⇒ lỗi.
#[test]
fn kt64_tlb_ngoai_active_on_khong_bi_cho() {
    let mut m = Machine::with_harts(4);
    let mut env = TestTlb {
        active: 0b0111, // hart 3 online nhưng **không** chạy VSpace này
        generation: 1,
        flushes: std::vec::Vec::new(),
    };
    m.set_current_hart(0).unwrap();
    let seq = m
        .tlb_shootdown_begin(&mut env, TlbScope::All, 5, 0)
        .expect("begin");
    assert_eq!(
        m.mail_len(3).unwrap(),
        0,
        "hart ngoài active_on không bị IPI"
    );
    assert!(m.tlb_pending(3).unwrap().is_none(), "không chờ hart ngoài");
    assert_eq!(m.tlb_pending(1).unwrap(), Some(seq));
    // Đang có shootdown chờ ⇒ begin thứ hai ⇒ Busy (không đè lệnh chưa ack).
    assert_eq!(
        m.tlb_shootdown_begin(&mut env, TlbScope::All, 5, 0).err(),
        Some(KError::Busy)
    );
    // D2-92b — chỉ hart đích tự phục vụ mới có ack; hart gọi không flush hộ.
    assert_eq!(m.tlb_service(&mut env, 1).unwrap(), 1);
    assert_eq!(m.tlb_service(&mut env, 2).unwrap(), 1);
    m.tlb_shootdown_wait(&mut env, 0).expect("wait");
    // Hart gọi **không** thuộc active_on ⇒ IllegalOperation (không flush “hộ” VSpace khác).
    m.set_current_hart(3).unwrap();
    assert_eq!(
        m.tlb_flush(&mut env, TlbScope::All, 5, 0).err(),
        Some(KError::IllegalOperation)
    );
}

/// **F4/D2-92b**: `wait` **không** ack hộ ⇒ ack không tới là `Busy` (reachable, không treo), và
/// `tlb_flush` một cửa cũng trả `Busy` khi targets chưa được phục vụ; phục vụ xong thì `Ok`.
#[test]
fn kt64_tlb_ack_khong_toi_tra_busy_khong_treo() {
    let mut m = Machine::with_harts(4);
    let mut env = TestTlb {
        active: 0b1111,
        generation: 2,
        flushes: std::vec::Vec::new(),
    };
    m.set_current_hart(0).unwrap();
    let seq = m
        .tlb_shootdown_begin(&mut env, TlbScope::All, 1, 0)
        .expect("begin");
    // Không hart đích nào chạy ⇒ vòng chờ quá hạn ⇒ `Busy` (không treo, không Ok giả).
    assert_eq!(
        m.tlb_shootdown_wait(&mut env, 0).err(),
        Some(KError::Busy),
        "ack không tới ⇒ Busy"
    );
    assert_eq!(
        m.tlb_pending(0).unwrap(),
        Some(seq),
        "Busy giữ nguyên pending (fail-closed)"
    );
    assert_eq!(m.dispatch(1).err(), Some(KError::Busy), "vẫn chặn dispatch");
    // `tlb_flush` một cửa trên đường nhiều hart cũng không tự ack hộ.
    assert_eq!(
        m.tlb_flush(&mut env, TlbScope::All, 1, 0).err(),
        Some(KError::Busy)
    );
    // Hart đích tự phục vụ ⇒ wait xong, dispatch chạy được.
    for h in 1..4u16 {
        assert_eq!(m.tlb_service(&mut env, h).unwrap(), 1, "hart {h} flush");
    }
    m.tlb_shootdown_wait(&mut env, 0).expect("wait đủ ack");
    assert!(m.tlb_pending(0).unwrap().is_none());
}

// -------------------------------------------------------------------------------------------
// Mảng 2 — SlotPool → arena: một nguồn sự thật cho byte slot
// -------------------------------------------------------------------------------------------

/// CNode **Retype từ Untyped**: slot nằm trong byte arena tại `offset + cnode_base`; `Revoke`
/// trả byte; `check_bytes` không đổi; `cap_at` đọc đúng.
#[test]
fn kt64_slot_cnode_tu_arena_va_revoke_tra_byte() {
    let mut m = Machine::new();
    let base = 0x1000_0000u64;
    let u = m.add_untyped(base, 1 << 16);
    let c0 = m.add_cnode(8);
    let ucap = m.root_cap(u);
    m.put_cap(c0, 0, ucap).expect("cap untyped");
    let before = m.untyped_used(u);
    let cap = m
        .retype(slot(c0, 0), ObjType::CNode, 4, 0)
        .expect("retype CNode");
    let cnode = m.cap_obj(cap).expect("cap_obj");
    let used = m.untyped_used(u);
    let expect_bytes = 64 + 16 * 48; // cnode_base + slots * slot_bytes
    assert_eq!(used, before + expect_bytes, "byte arena đúng spec");
    let span = m
        .cnode_slot_span(cnode)
        .expect("span")
        .expect("CNode từ arena có span");
    assert_eq!(span.1, 16 * 48, "byte slot trong arena");
    assert_eq!(span.0, base + 64, "probe PA = base arena + cnode_base");
    assert!(
        span.0 + span.1 as u64 <= base + (1 << 16),
        "slot trong vùng Untyped"
    );
    // Slot thật: đặt cap vào rồi đọc lại qua CSpace.
    let child = m.root_cap(cnode);
    m.put_cap(cnode, 0, child).expect("cap con vào slot thật");
    assert_eq!(m.cap_at(cnode, 0).unwrap(), child, "slot 0 của CNode mới");
    assert!(m.check_bytes(u).is_ok());
    assert!(m.check_all_bytes().is_ok());
    // Revoke cây Untyped: byte về 0, span biến mất (byte slot trả lại arena).
    m.revoke(c0, Some(0)).expect("revoke untyped");
    let after = m.untyped_used(u);
    assert_eq!(after, before, "Revoke trả byte slot");
    assert_eq!(
        m.cnode_slot_span(cnode).unwrap(),
        None,
        "CNode chết không còn span"
    );
    assert!(m.check_bytes(u).is_ok());
}

/// Hai CNode Retype liên tiếp: cửa sổ slot **không chồng** (ánh xạ offset→word chứng minh
/// không có allocator thứ hai); CNode harness không có byte arena nhưng slot vẫn chạy.
#[test]
fn kt64_slot_hai_cnode_khong_chong_va_harness_khong_byte_arena() {
    let mut m = Machine::new();
    let u = m.add_untyped(0x2000_0000, 1 << 16);
    let c0 = m.add_cnode(8);
    let ucap = m.root_cap(u);
    m.put_cap(c0, 0, ucap).expect("cap untyped");
    let a_cap = m.retype(slot(c0, 0), ObjType::CNode, 4, 0).unwrap();
    let a = m.cap_obj(a_cap).unwrap();
    let b_cap = m.retype(slot(c0, 0), ObjType::CNode, 4, 0).unwrap();
    let b = m.cap_obj(b_cap).unwrap();
    let sa = m.cnode_slot_span(a).unwrap().expect("span a");
    let sb = m.cnode_slot_span(b).unwrap().expect("span b");
    assert!(
        sa.0 + sa.1 as u64 <= sb.0 || sb.0 + sb.1 as u64 <= sa.0,
        "hai cửa sổ slot không chồng: {sa:?} vs {sb:?}"
    );
    // Ghi cap khác nhau vào cùng chỉ số slot của hai CNode: đọc lại đúng từng cái.
    let ac = m.root_cap(a);
    let bc = m.root_cap(b);
    m.put_cap(a, 0, ac).expect("cap a0");
    m.put_cap(b, 0, bc).expect("cap b0");
    assert_eq!(m.cap_at(a, 0).unwrap(), ac);
    assert_eq!(m.cap_at(b, 0).unwrap(), bc);
    // Harness CNode: không có byte arena (như `add_cnode` của sim — CNode do kernel cấp).
    assert_eq!(
        m.cnode_slot_span(c0).unwrap(),
        None,
        "harness CNode không tiêu byte arena"
    );
    assert!(m.check_bytes(u).is_ok());
    assert!(m.check_all_bytes().is_ok());
}

/// **F1 (D2-92a)** — hồi quy: hai CNode Retype từ **hai Untyped khác nhau**, **cùng offset 0**,
/// phải có cửa sổ word **rời nhau**; ghi cap khác nhau vào slot 0 rồi đọc lại **đúng** từng cái
/// (nếu chỉ dùng offset, hai CSpace sẽ đè cap lên nhau và đọc ra cap của CNode kia).
#[test]
fn kt64_slot_hai_untyped_cung_offset_khong_alias() {
    let mut m = Machine::new();
    let ua = m.add_untyped(0x3000_0000, 1 << 14);
    let ub = m.add_untyped(0x4000_0000, 1 << 14);
    let c0 = m.add_cnode(8);
    let ca_u = m.root_cap(ua);
    let cb_u = m.root_cap(ub);
    m.put_cap(c0, 0, ca_u).expect("cap untyped A");
    m.put_cap(c0, 1, cb_u).expect("cap untyped B");
    let ca = m
        .retype(slot(c0, 0), ObjType::CNode, 4, 0)
        .expect("CNode A");
    let a = m.cap_obj(ca).expect("obj A");
    let cb = m
        .retype(slot(c0, 1), ObjType::CNode, 4, 0)
        .expect("CNode B");
    let b = m.cap_obj(cb).expect("obj B");
    // Cùng offset (cả hai bắt đầu ở 0 của Untyped mình) ⇒ span PA **khác** base, và word khác.
    let sa = m.cnode_slot_span(a).unwrap().expect("span a");
    let sb = m.cnode_slot_span(b).unwrap().expect("span b");
    assert_eq!(sa.0, 0x3000_0000 + 64, "A: base arena A + cnode_base");
    assert_eq!(sb.0, 0x4000_0000 + 64, "B: base arena B + cnode_base");
    assert_ne!(sa.0, sb.0, "hai Untyped khác base ⇒ span khác");
    // Cap riêng: ghi hai cap khác nhau vào slot 0 của từng CNode rồi đọc lại.
    let ac = m.root_cap(a);
    let bc = m.root_cap(b);
    m.put_cap(a, 0, ac).expect("cap a0");
    m.put_cap(b, 0, bc).expect("cap b0");
    assert_eq!(m.cap_at(a, 0).unwrap(), ac, "slot 0 của A phải là cap A");
    assert_eq!(m.cap_at(b, 0).unwrap(), bc, "slot 0 của B phải là cap B");
    assert_ne!(m.cap_at(a, 0).unwrap(), m.cap_at(b, 0).unwrap());
    assert!(m.check_bytes(ua).is_ok() && m.check_bytes(ub).is_ok());
    assert!(m.check_all_bytes().is_ok());
}

// -------------------------------------------------------------------------------------------
// Mảng 3 — SchedContext.Bind / Unbind (bound_sc sống)
// -------------------------------------------------------------------------------------------

struct BindRig {
    m: Machine,
    c0: ObjId,
    sc: [SlotRef; 2],
    ntfn: [SlotRef; 2],
    ntfn_obj: [ObjId; 2],
    rep: SlotRef,
    ep: SlotRef,
}

fn bind_rig() -> BindRig {
    let mut m = Machine::new();
    let u = m.add_untyped(0x8000_0000, 1 << 16);
    let c0 = m.add_cnode(64);
    let ucap = m.root_cap(u);
    m.put_cap(c0, 0, ucap).expect("cap untyped");
    let mut sc = [slot(c0, 1); 2];
    let mut ntfn = [slot(c0, 8); 2];
    let mut ntfn_obj = [0u32; 2];
    for i in 0..2 {
        let cap = m
            .retype(slot(c0, 0), ObjType::SchedContext, 0, 0)
            .expect("retype SC");
        m.put_cap(c0, 1 + i, cap).expect("cap SC");
        sc[i] = slot(c0, 1 + i);
        let ncap = m
            .retype(slot(c0, 0), ObjType::Notification, 0, 0)
            .expect("retype Notification");
        let n = m.cap_obj(ncap).expect("obj ntfn");
        m.put_cap(c0, 8 + i, ncap).expect("cap Notification");
        ntfn[i] = slot(c0, 8 + i);
        ntfn_obj[i] = n;
    }
    let rcap = m
        .retype(slot(c0, 0), ObjType::Reply, 0, 0)
        .expect("retype Reply");
    m.put_cap(c0, 6, rcap).expect("cap Reply");
    let ecap = m
        .retype(slot(c0, 0), ObjType::Endpoint, 0, 0)
        .expect("retype Endpoint");
    m.put_cap(c0, 7, ecap).expect("cap Endpoint");
    BindRig {
        m,
        c0,
        sc,
        ntfn,
        ntfn_obj,
        rep: slot(c0, 6),
        ep: slot(c0, 7),
    }
}

/// **F5 (D2-92d)** — `Revoke` Untyped chứa Notification ⇒ finalise Notification ⇒ hook gỡ
/// `SC.bound` (không mồ côi); bất biến 1:1 hai chiều **xanh** trước và sau `Revoke`.
#[test]
fn kt64_bind_revoke_go_binding_mo_coi() {
    let mut m = Machine::new();
    let ua = m.add_untyped(0x9000_0000, 1 << 14);
    let ub = m.add_untyped(0xA000_0000, 1 << 14);
    let c0 = m.add_cnode(32);
    let ca = m.root_cap(ua);
    let cb = m.root_cap(ub);
    m.put_cap(c0, 0, ca).expect("cap untyped A");
    m.put_cap(c0, 1, cb).expect("cap untyped B");
    let sc_cap = m
        .retype(slot(c0, 0), ObjType::SchedContext, 0, 0)
        .expect("retype SC");
    m.put_cap(c0, 2, sc_cap).expect("cap SC");
    let n_cap = m
        .retype(slot(c0, 1), ObjType::Notification, 0, 0)
        .expect("retype Notification");
    let n_obj = m.cap_obj(n_cap).expect("obj ntfn");
    m.put_cap(c0, 3, n_cap).expect("cap Notification");
    let sc_obj = m.cap_obj(sc_cap).expect("obj SC");
    m.sched_context_bind(slot(c0, 2), slot(c0, 3))
        .expect("bind SC↔Notification");
    assert_eq!(m.notification_bound_sc(n_obj).unwrap(), Some(sc_obj));
    assert!(m.check_notifications().is_ok(), "1:1 hai chiều xanh");
    // `Revoke` Untyped B (chứa Notification) ⇒ finalise Notification ⇒ gỡ binding phía SC.
    m.revoke(c0, Some(1)).expect("revoke untyped B");
    assert!(!m.object_alive(n_obj), "Notification đã finalise");
    assert_eq!(
        m.sched_context_bound(sc_obj).unwrap(),
        None,
        "Revoke gỡ SC.bound — không mồ côi"
    );
    assert!(
        m.check_notifications().is_ok(),
        "bất biến 1:1 vẫn xanh sau Revoke"
    );
    assert!(m.check_caps().is_ok());
}

/// Bind ghi **hai chiều** 1:1; đổi target gỡ binding cũ; `Unbind` xoá cả hai; mã lỗi như sim.
#[test]
fn kt64_bind_hai_chieu_ma_loi_va_unbind() {
    let mut r = bind_rig();
    r.m.sched_context_bind(r.sc[0], r.ntfn[0])
        .expect("bind SC↔Notification");
    let sc_obj = obj_at(&r.m, r.c0, 1);
    assert_eq!(
        r.m.notification_bound_sc(r.ntfn_obj[0]).unwrap(),
        Some(sc_obj)
    );
    assert!(
        r.m.sched_context_bound(sc_obj).unwrap().is_some(),
        "phía SC có bound"
    );
    // Notification đã bind SC khác ⇒ IllegalOperation, **không** đổi gì.
    assert_eq!(
        r.m.sched_context_bind(r.sc[1], r.ntfn[0]).err(),
        Some(KError::IllegalOperation)
    );
    assert_eq!(
        r.m.notification_bound_sc(r.ntfn_obj[0]).unwrap(),
        Some(sc_obj)
    );
    // SC đổi sang Notification khác ⇒ gỡ phía notification cũ.
    r.m.sched_context_bind(r.sc[0], r.ntfn[1]).expect("rebind");
    assert_eq!(
        r.m.notification_bound_sc(r.ntfn_obj[0]).unwrap(),
        None,
        "gỡ cũ"
    );
    assert_eq!(
        r.m.notification_bound_sc(r.ntfn_obj[1]).unwrap(),
        Some(sc_obj)
    );
    // Target sai kiểu ⇒ WrongObjectType.
    assert_eq!(
        r.m.sched_context_bind(r.sc[0], r.ep).err(),
        Some(KError::WrongObjectType)
    );
    // Cap đã xoá ⇒ slot rỗng (`SlotEmpty`, đúng như `cap_at` của sim — kiểm trước `require_type`).
    let c0 = r.c0;
    let ntfn1 = r.ntfn[1];
    r.m.delete(ntfn1).expect("xoá cap Notification");
    assert_eq!(
        r.m.sched_context_bind(r.sc[0], ntfn1).err(),
        Some(KError::SlotEmpty)
    );
    // Unbind: xoá hai chiều, idempotent.
    r.m.sched_context_unbind(r.sc[0]).expect("unbind");
    assert_eq!(r.m.sched_context_bound(sc_obj).unwrap(), None);
    assert_eq!(
        r.m.notification_bound_sc(r.ntfn_obj[1]).unwrap(),
        None,
        "gỡ phía notification"
    );
    r.m.sched_context_unbind(r.sc[0])
        .expect("unbind idempotent");
    let _ = c0;
}

/// `Signal` chỉ đánh thức TCB **đúng** bound: hai TCB bound hai Notification đang chờ Recv,
/// signal ntfn[0] ⇒ chỉ TCB0 thức.
#[test]
fn kt64_bind_signal_chi_danh_thuc_dung_bound() {
    let mut r = bind_rig();
    let c0 = r.c0;
    // Endpoint nhận Recv của hai TCB.
    let ecap =
        r.m.retype(slot(c0, 0), ObjType::Endpoint, 0, 0)
            .expect("retype endpoint");
    r.m.put_cap(c0, 20, ecap).expect("cap endpoint");
    let ep = slot(c0, 20);
    let mut ts = [slot(c0, 30); 2];
    let mut tobj = [0u32; 2];
    for i in 0..2 {
        let t = r.m.add_kernel_object(ObjType::Tcb);
        let tc = r.m.root_cap(t);
        r.m.put_cap(c0, 30 + i, tc).expect("cap tcb");
        ts[i] = slot(c0, 30 + i);
        tobj[i] = t;
        r.m.start_tcb(t).expect("Ready");
        r.m.tcb_bind_notification(ts[i], r.ntfn[i])
            .expect("bind ntfn");
        r.m.recv(t, ep, None).expect("recv chặn");
    }
    r.m.notification_signal(r.ntfn_obj[0], 0x42)
        .expect("signal 1");
    assert_eq!(
        r.m.tcb_state(tobj[0]).unwrap(),
        TcbState::Ready,
        "đúng bound thức"
    );
    assert!(
        matches!(
            r.m.tcb_state(tobj[1]).unwrap(),
            TcbState::BlockedOnRecv { .. }
        ),
        "TCB bound notification khác **không** bị đánh thức"
    );
    assert!(r.m.check_notifications().is_ok());
    assert!(r.m.check_ipc().is_ok());
    let _ = r.rep;
}

// -------------------------------------------------------------------------------------------
// Mảng 4 — tcb_resume_with_regs
// -------------------------------------------------------------------------------------------

struct ResumeRig {
    m: Machine,
    c0: ObjId,
    ep: SlotRef,
    rep: SlotRef,
    ts: SlotRef,
    t: ObjId,
}

fn resume_rig() -> ResumeRig {
    let mut m = Machine::new();
    let u = m.add_untyped(0x8000_0000, 1 << 16);
    let c0 = m.add_cnode(64);
    let ucap = m.root_cap(u);
    m.put_cap(c0, 0, ucap).expect("cap untyped");
    let ecap = m
        .retype(slot(c0, 0), ObjType::Endpoint, 0, 0)
        .expect("endpoint");
    m.put_cap(c0, 1, ecap).expect("cap ep");
    let rcap = m.retype(slot(c0, 0), ObjType::Reply, 0, 0).expect("reply");
    m.put_cap(c0, 2, rcap).expect("cap reply");
    let t = m.add_kernel_object(ObjType::Tcb);
    let tc = m.root_cap(t);
    m.put_cap(c0, 10, tc).expect("cap tcb");
    m.start_tcb(t).expect("Ready");
    ResumeRig {
        m,
        c0,
        ep: slot(c0, 1),
        rep: slot(c0, 2),
        ts: slot(c0, 10),
        t,
    }
}

/// Fault đã giao (BlockedOnSend{call, fault}) ⇒ resume ghi regs **trước** khi Ready, không phát
/// lại IPC; `exit_code` giữ nguyên `None`.
#[test]
fn kt64_resume_sau_fault_ghi_regs_truoc_ready() {
    let mut r = resume_rig();
    r.m.tcb_set_fault_ep(r.ts, r.ep).expect("fault_ep");
    r.m.inject_fault(r.t, FaultKind::UnknownSyscall { nr: 99 })
        .expect("fault");
    assert!(matches!(
        r.m.tcb_state(r.t).unwrap(),
        TcbState::BlockedOnSend {
            call: true,
            fault: Some(_),
            ..
        }
    ));
    let sent = r.m.ipc_sent();
    r.m.tcb_resume_with_regs(r.ts, msg(1, &[0xAABB, 0xCCDD, 0x1122, 0x3344]))
        .expect("resume sau fault");
    assert_eq!(
        r.m.tcb_state(r.t).unwrap(),
        TcbState::Ready,
        "quay lại Ready"
    );
    assert_eq!(r.m.tcb_regs(r.t).unwrap(), [0xAABB, 0xCCDD, 0x1122, 0x3344]);
    assert_eq!(
        r.m.tcb_info_obj(r.t).unwrap().exit_code,
        None,
        "giữ exit_code"
    );
    assert!(r.m.ipc_sent() > sent, "ghi thanh ghi qua đúng cửa transfer");
    assert!(r.m.ipc_delivered() > 0);
    assert!(r.m.check_ipc().is_ok());
    assert!(r.m.check_sched().is_ok());
    let _ = r.c0;
}

/// Trạng thái **không** được phép resume ⇒ IllegalOperation và không đổi gì (không mở đường
/// user tự resume tuỳ ý: Running/Inactive/Dead đều bị từ chối).
#[test]
fn kt64_resume_trang_thai_cam_bi_tu_choi() {
    let mut r = resume_rig();
    // Ready: được phép (bỏ suspended/ghi regs).
    r.m.tcb_resume_with_regs(r.ts, msg(2, &[7]))
        .expect("Ready hợp lệ");
    assert_eq!(r.m.tcb_regs(r.t).unwrap()[0], 7);
    // Running: cấm (đang chạy trên hart).
    r.m.set_current_hart(0).unwrap();
    r.m.transition_tcb(r.t, NonReadyTarget::Running { hart: 0 })
        .expect("Running");
    assert_eq!(
        r.m.tcb_resume_with_regs(r.ts, msg(3, &[9])).err(),
        Some(KError::IllegalOperation)
    );
    assert_eq!(r.m.tcb_regs(r.t).unwrap()[0], 7, "lỗi không đổi thanh ghi");
    r.m.tcb_yield(r.t).expect("Ready lại");
    // Inactive: cấm (không có context để resume).
    let t2 = r.m.add_kernel_object(ObjType::Tcb);
    let t2c = r.m.root_cap(t2);
    r.m.put_cap(r.c0, 11, t2c).expect("cap t2");
    assert_eq!(
        r.m.tcb_resume_with_regs(slot(r.c0, 11), msg(4, &[1])).err(),
        Some(KError::IllegalOperation)
    );
    // Dead: cấm.
    r.m.finalise_tcb(t2).expect("finalise t2");
    assert_eq!(
        r.m.tcb_resume_with_regs(slot(r.c0, 11), msg(5, &[1])).err(),
        Some(KError::IllegalOperation)
    );
}

/// `BlockedOnReply` (đúng caller) được resume; message mang cap ⇒ InvalidArgument.
#[test]
fn kt64_resume_blocked_on_reply_va_message_cap() {
    let mut r = resume_rig();
    // t2 Recv(ep, reply) rồi t Call(ep) ⇒ t BlockedOnReply.
    let t2 = r.m.add_kernel_object(ObjType::Tcb);
    let t2c = r.m.root_cap(t2);
    r.m.put_cap(r.c0, 11, t2c).expect("cap t2");
    let ts2 = slot(r.c0, 11);
    r.m.start_tcb(t2).expect("t2 Ready");
    r.m.recv(t2, r.ep, Some(r.rep))
        .expect("t2 recv + reply cap");
    r.m.call_msg(r.t, r.ep, msg(5, &[0x55])).expect("call");
    assert!(
        matches!(r.m.tcb_state(r.t).unwrap(), TcbState::BlockedOnReply { .. }),
        "caller chờ reply"
    );
    r.m.tcb_resume_with_regs(r.ts, msg(6, &[0x66]))
        .expect("resume caller");
    assert_eq!(r.m.tcb_state(r.t).unwrap(), TcbState::Ready);
    let rep_obj = r.m.cap_obj_at(r.c0, 2).unwrap();
    assert_eq!(
        r.m.reply_caller(rep_obj).unwrap(),
        None,
        "Reply.caller đã gỡ"
    );
    // Message mang cap ⇒ InvalidArgument, trạng thái không đổi.
    let bad = Message::with_cap_ids(7, &[1], &[r.m.root_cap(r.t)]).expect("message cap");
    assert_eq!(
        r.m.tcb_resume_with_regs(r.ts, bad).err(),
        Some(KError::InvalidArgument)
    );
    assert_eq!(r.m.tcb_state(r.t).unwrap(), TcbState::Ready);
    let _ = ts2;
}

/// `Sleeping` giữ nguyên reason (bảng §4.5) — không về `Ready`.
#[test]
fn kt64_resume_sleeping_giu_nguyen() {
    let mut r = resume_rig();
    r.m.set_current_hart(0).unwrap();
    r.m.transition_tcb(r.t, NonReadyTarget::Running { hart: 0 })
        .expect("Running");
    r.m.transition_tcb(
        r.t,
        NonReadyTarget::Sleeping {
            reason: SleepReason::Budget,
        },
    )
    .expect("Running→Sleeping{Budget}");
    r.m.tcb_resume_with_regs(r.ts, msg(8, &[0x99]))
        .expect("resume Sleeping");
    assert_eq!(
        r.m.tcb_state(r.t).unwrap(),
        TcbState::Sleeping {
            reason: SleepReason::Budget
        },
        "giữ Sleeping"
    );
    assert_eq!(
        r.m.tcb_regs(r.t).unwrap()[0],
        0x99,
        "thanh ghi vẫn được ghi"
    );
}

// -------------------------------------------------------------------------------------------
// Pin phụ: `size_of::<Machine>()` không vượt pin D2-84 sau bốn mảng
// -------------------------------------------------------------------------------------------

#[test]
fn kt64_pin_d2_84_van_giu() {
    let f = Machine::footprint();
    assert!(f.total <= 196_608, "D2-84: {} > 196608", f.total);
    assert!(f.padding <= 8, "bảng phân bố phải đủ (D2-85c)");
}
