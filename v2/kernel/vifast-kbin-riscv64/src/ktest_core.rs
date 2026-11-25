// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **core kernel thật** của ảnh test `vifast-kbin-ktest` (KT-61) — TAP **43..52**; **KT-63**
//! thêm TAP **53..55** (chuyển cap qua IPC + finalise IPC khi Revoke); trap lồng dời sang **56**.
//!
//! Mọi thao tác đi qua [`vifast_plat_riscv64::core_bridge::with_machine`] — tức `Machine` **static
//! trong `.bss` của ảnh kernel** (không phải model host, không phải sim). Bằng chứng `# core:`
//! in số object/cap/TCB **thật** của lần boot này + trạng thái các bất biến
//! (`check_caps`/`check_bytes`/`check_sched`/`check_ipc`/…).
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 43 - core link` | `Machine` là static trong **vùng ảnh** `[0x8020_0000, guard)`; `size_of::<Machine>()` ≤ pin D2-84 **196.608 B**; bảng phân bố 27 dòng khớp `sum + scalars + padding == total`, padding ≤ 8 (D2-85c) |
//! | `ok 44 - core untyped boot` | Untyped boot dựng từ **vùng nhãn PA** ngay sau arena object layer (D2-83: không dereference); byte `used=0 free=size`, `check_bytes`/`check_all_bytes` xanh |
//! | `ok 45 - core tcb retype` | `Retype` **từ Untyped**: 1 CNode gốc (32 slot) + 3 TCB + Endpoint + Reply + Notification + SchedControl + SchedContext; cap vào slot, TCB `Inactive`, `check_caps`/`check_bytes` xanh |
//! | `ok 46 - core configure resume suspend` | `Tcb.Configure` (cspace) + `SchedControl.Configure(4,8)` + `SetSchedParams` + `SetPriority(3,10)` + `Resume`/`Suspend(Abort)`; `suspended` đúng, priority/cspace/SC đọc lại khớp, `check_sched`/`check_sc_refs`/`check_budget` xanh |
//! | `ok 47 - core sched 2 tcb` | 2 TCB Ready khác priority: `sched_pick` chọn đúng; `dispatch` chạy TCB 1; `Suspend` TCB 1 ⇒ **không** còn trong hàng đợi ⇒ `dispatch` chuyển sang TCB 2; `Resume` đưa lại |
//! | `ok 48 - core ipc round` | `Call → Recv → Reply` **một vòng** trên Endpoint/Reply thật; MR tới đích, `BlockedOnSend{call}`→`BlockedOnReply`→`Ready`, `Reply.caller` sạch, `check_ipc`/`check_reply_links` xanh |
//! | `ok 49 - core notification signal` | bind + `Wait` (chặn) + `Signal(0x42)` (đánh thức, giao word/badge) + `Signal` không waiter rồi `Poll`; `check_notifications` xanh |
//! | `ok 50 - core waitaddr` | `WaitAddr(nhãn Frame, +64, 0)` chặn, `SetAddr`/`WakeAddr(1)` đánh thức; nhãn **không có byte** (D2-83), bucket rỗng, `check_waitaddr` xanh |
//! | `ok 51 - core d2-83 boundary` | `Retype` `Frame`/`PageTable`/`VSpace` trong core ⇒ **`NotSupported` 3/3**, `untyped_used` không đổi (atomic); **cùng lúc** object layer tạo/revoke VSpace+Frame thật ⇒ biên giới một chiều giữ nguyên |
//! | `ok 52 - core invariants` | Revoke cây Untyped ⇒ byte về 0 (`used=0 free=size gap=0`), cap còn đúng 1; **12/12** bất biến xanh, 0 rò |
//! | `ok 53 - core cap transfer` | 4 cap Endpoint→Recv qua 2 TCB: cap nhận **bỏ `RIGHT_GRANT`** (giữ badge), thiếu GRANT/ô bận ⇒ thất bại **mềm**, unwrap badge thành `Message.badge`; `check_ipc`/`check_caps` xanh |
//! | `ok 54 - core cap message` | message mang cap qua **Call/Reply**: caller `BlockedOnReply` nhận 2 cap vào receive window, caller `Ready`, `Reply.caller` sạch; `check_ipc`/`check_reply_links` xanh |
//! | `ok 55 - core revoke finalise` | `Revoke` cây Untyped khi có caller/Recv waiter/Notification waiter/hàng ⇒ caller `Ready + Cancelled`, cả hai waiter thức (`Ready + Cancelled`), hàng rỗng, `ipc_dropped=1`, phủ QEMU cho đường finalise IPC |
//!
//! Số đo do chính boot in (`# core:`); `ktest.sh` grep đúng các dòng đó — tài liệu không chép số
//! tay (QD §9 Q12). Xem `v2/docs/k3b-core.md` §9.

use core::fmt::Write;

use vifast_core::{
    CapId, KError, Machine, Message, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL, RIGHT_GRANT,
};
use vifast_plat_common::{Platform, PlatformWriter};

use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// **KT-411** — bộ định danh + tên trạng thái + tên mã lỗi nay ở **một** bản dùng chung
/// (`vifast_ktest::core_suite`), không còn ba bản sao trong ba arch. Re-export để các pha riêng
/// của ảnh riscv64 (`ktest_core64`) giữ nguyên import.
pub(crate) use vifast_ktest::core_suite::{err_name, err_name_opt, state_name, Rig};

/// Đưa TCB về `Ready` qua **cửa** khớp trạng thái nguồn (test-only).
///
/// **D2-187/D2-194** — không còn cửa "đặt `Ready` vô điều kiện": `Inactive` ⇒ `start_tcb`,
/// `Running` ⇒ `tcb_yield`, `BlockedOn*`/`Sleeping` ⇒ `wake` (API **test-only**, D2-144).
///
/// Vì sao hàm này ở **đây** mà không ở `vifast-ktest`: `wake()` là API CHỈ-DÙNG-CHO-TEST và luật
/// `E_STRICT_TEST_ONLY_WAKE` chỉ miễn tệp test-only **theo đồ thị module** (tệp chỉ tới được từ
/// bin có `required-features` chứa `ktest`). Tệp này là `src/ktest_core.rs` của bin
/// `vifast-kbin-ktest` ⇒ được miễn; crate thư viện dùng chung thì không. Bộ ca chung nhận hàm
/// này qua `vifast_ktest::core_suite::ReadyFn`.
fn task_ready(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    match m.tcb_state(tcb)? {
        TcbState::Inactive => m.start_tcb(tcb),
        TcbState::Ready => Ok(()),
        TcbState::Running { .. } => m.tcb_yield(tcb),
        _ => m.wake(tcb),
    }
}

/// Viết message mang `caps` (CapId) — dùng cho TAP 53..55.
fn msg_caps(label: u32, words: &[u64], caps: &[CapId]) -> Message {
    Message::with_cap_ids(label, words, caps).expect("kt63: message cap hợp lệ")
}

/// `true` ⇔ `n` ô window `base..base+n` đều có cap quyền **đã bỏ `RIGHT_GRANT`**.
fn window_rights_ok_n(m: &Machine, root: ObjId, base: usize, n: usize) -> bool {
    (0..n).all(|k| {
        m.cap_at(root, base + k)
            .ok()
            .and_then(|c| m.cap_info(c).ok())
            .map(|i| i.rights == (RIGHTS_ALL & !RIGHT_GRANT))
            .unwrap_or(false)
    })
}

/// TAP **53..55** (KT-63): chuyển cap Endpoint→Recv qua 2 TCB, message mang cap qua
/// Call/Reply, và `Revoke` finalise IPC (caller/waiter/hàng). Chạy trên `Machine` **thật**
/// của ảnh kernel; mọi số trong `# ipc:` là số đo của lần boot này.
fn cap_ipc_taps(m: &mut Machine, u: ObjId, fails: &mut usize) {
    let root = m.add_cnode(96);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("kt63: cap Untyped");
    let mk = |m: &mut Machine, ty: ObjType, slot: usize| -> ObjId {
        let cap = m.retype(us, ty, 0, 0).expect("kt63: retype");
        let obj = m.cap_obj(cap).expect("kt63: cap_obj");
        m.put_cap(root, slot, cap).expect("kt63: put cap");
        obj
    };
    let t0 = mk(m, ObjType::Tcb, 1);
    let t1 = mk(m, ObjType::Tcb, 2);
    let t2 = mk(m, ObjType::Tcb, 3);
    let t3 = mk(m, ObjType::Tcb, 4);
    let t4 = mk(m, ObjType::Tcb, 5);
    let _ep0 = mk(m, ObjType::Endpoint, 10);
    let _ep1 = mk(m, ObjType::Endpoint, 11);
    let _rep = mk(m, ObjType::Reply, 12);
    for t in [t0, t1, t2, t3, t4] {
        m.start_tcb(t).expect("kt63: Ready");
    }
    let win = |base: usize| {
        [
            SlotRef::new(root, base),
            SlotRef::new(root, base + 1),
            SlotRef::new(root, base + 2),
            SlotRef::new(root, base + 3),
        ]
    };
    let ep0s = SlotRef::new(root, 10);
    let ep1s = SlotRef::new(root, 11);
    let reps = SlotRef::new(root, 12);
    // 4 cap nguồn (Endpoint 2 — khác endpoint nhận ⇒ không unwrap), 1 cap thiếu GRANT,
    // 1 cap badge trên **chính** ep0 (unwrap khi gửi tới ep0).
    let mut src = [0u32; 4];
    for (i, slot) in [40usize, 41, 42, 43].iter().enumerate() {
        src[i] = m
            .copy(ep1s, SlotRef::new(root, *slot))
            .expect("kt63: copy cap nguồn");
    }
    let nogrant = m
        .copy_rights(
            SlotRef::new(root, 11),
            SlotRef::new(root, 44),
            Some(RIGHTS_ALL & !RIGHT_GRANT),
        )
        .expect("kt63: cap thiếu GRANT");
    let badge_cap = m
        .mint(ep0s, SlotRef::new(root, 45), RIGHTS_ALL, Some(0xA5A5))
        .expect("kt63: cap badge");

    // ---------------------------------------------------------------- TAP 53: chuyển 4 cap
    m.tcb_set_recv_window(SlotRef::new(root, 2), win(30))
        .expect("kt63: bind window t1");
    m.recv(t1, ep0s, Some(reps)).expect("kt63: Recv t1");
    let words = [0x11u64, 0x22];
    m.send(t0, ep0s, msg_caps(7, &words, &src))
        .expect("kt63: Send 4 cap");
    let info1 = m.tcb_ipc_info(t1).ok().flatten();
    let caps1 = info1.map(|i| i.caps()).unwrap_or(9);
    let len1 = info1.map(|i| i.length()).unwrap_or(0);
    let badge1 = m.tcb_ipc_badge(t1).unwrap_or(0);
    let rights_ok1 = window_rights_ok_n(m, root, 30, 4);
    // TCB t2 (window sạch): 1 cap thiếu GRANT ⇒ 3 cài, 1 thất bại mềm.
    m.tcb_set_recv_window(SlotRef::new(root, 3), win(34))
        .expect("kt63: bind window t2");
    m.recv(t2, ep0s, None).expect("kt63: Recv t2");
    m.send(
        t0,
        ep0s,
        msg_caps(8, &[0x33], &[src[0], nogrant, src[2], src[3]]),
    )
    .expect("kt63: Send 3+1 cap");
    let caps2 = m
        .tcb_ipc_info(t2)
        .ok()
        .flatten()
        .map(|i| i.caps())
        .unwrap_or(9);
    // TCB t3 (window sạch): unwrap badge ep0 + 1 cap thường ⇒ caps=1, extra=1.
    m.tcb_set_recv_window(SlotRef::new(root, 4), win(46))
        .expect("kt63: bind window t3");
    m.recv(t3, ep0s, None).expect("kt63: Recv t3");
    m.send(t0, ep0s, msg_caps(12, &[0xAB], &[badge_cap, src[1]]))
        .expect("kt63: Send unwrap");
    let info3 = m.tcb_ipc_info(t3).ok().flatten();
    let caps3 = info3.map(|i| i.caps()).unwrap_or(9);
    let extra3 = info3.map(|i| i.extra_caps_unwrapped()).unwrap_or(9);
    let badge3 = m.tcb_ipc_badge(t3).unwrap_or(0);
    let (installed, unwrapped, failed) = m.cap_transfer_stats();
    let rights_ok3 = m.cap_at(root, 46).is_err()
        && m.cap_at(root, 47)
            .ok()
            .and_then(|c| m.cap_info(c).ok())
            .map(|i| i.rights == (RIGHTS_ALL & !RIGHT_GRANT))
            .unwrap_or(false);
    let ok53 = caps1 == 4
        && len1 == 2
        && badge1 == 0
        && rights_ok1
        && caps2 == 3
        && caps3 == 1
        && extra3 == 1
        && badge3 == 0xA5A5
        && rights_ok3
        && installed == 8
        && failed == 1
        && unwrapped == 1
        && m.check_ipc().is_ok()
        && m.check_caps().is_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: cap-transfer installed={} failed={} unwrap={} caps4={} len={} rights=0x{:x} caps3={} caps1={} extra={} badge=0x{:x} check_ipc={} check_caps={}",
            installed,
            failed,
            unwrapped,
            caps1,
            len1,
            if rights_ok1 { RIGHTS_ALL & !RIGHT_GRANT } else { 0 },
            caps2,
            caps3,
            extra3,
            badge3,
            m.check_ipc().is_ok() as u8,
            m.check_caps().is_ok() as u8
        );
    }
    if ok53 {
        PLATFORM.put_str("ok 53 - core cap transfer\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 53 - core cap transfer: caps4={caps1} caps3={caps2} caps1={caps3} extra={extra3} badge=0x{badge3:x} inst={installed} fail={failed} unwrap={unwrapped}"
        );
        *fails += 1;
    }

    // ------------------------------------------------- TAP 54: message mang cap qua Call/Reply
    let t5 = mk(m, ObjType::Tcb, 6);
    m.start_tcb(t5).expect("kt63: Ready t5");
    m.tcb_set_recv_window(SlotRef::new(root, 6), win(50))
        .expect("kt63: bind window t5");
    m.tcb_set_recv_window(SlotRef::new(root, 1), win(54))
        .expect("kt63: bind window t0");
    m.recv(t5, ep1s, Some(reps)).expect("kt63: Recv t5");
    m.call_msg(t0, ep1s, msg_caps(20, &[0x55], &[src[0], src[1]]))
        .expect("kt63: Call 2 cap");
    let caller = m
        .reply_caller(m.cap_obj_at(root, 12).expect("kt63: rep obj"))
        .unwrap_or(None);
    let t5_caps = m
        .tcb_ipc_info(t5)
        .ok()
        .flatten()
        .map(|i| i.caps())
        .unwrap_or(9);
    let blocked = m.tcb_state(t0)
        == Ok(TcbState::BlockedOnReply {
            reply: m.cap_obj_at(root, 12).expect("kt63: rep obj"),
        });
    m.reply_msg(t5, reps, msg_caps(21, &[0x66], &[src[2], src[3]]))
        .expect("kt63: Reply 2 cap");
    let t0_caps = m
        .tcb_ipc_info(t0)
        .ok()
        .flatten()
        .map(|i| i.caps())
        .unwrap_or(9);
    let t0_ready = m.tcb_state(t0) == Ok(TcbState::Ready);
    let t0_rights = window_rights_ok_n(m, root, 54, 2);
    let reply_clear = m
        .reply_caller(m.cap_obj_at(root, 12).expect("kt63: rep obj"))
        .unwrap_or(None)
        .is_none();
    let ok54 = t5_caps == 2
        && t0_caps == 2
        && caller == Some(t0)
        && blocked
        && t0_ready
        && t0_rights
        && reply_clear
        && m.check_ipc().is_ok()
        && m.check_reply_links().is_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: cap-message direction=call-reply caller_caps=2 reply_caps=2 caller_bound={} caller_ready={} rights=0x{:x} reply_caller=0 check_ipc={} check_reply_links={}",
            blocked as u8,
            t0_ready as u8,
            if t0_rights { RIGHTS_ALL & !RIGHT_GRANT } else { 0 },
            m.check_ipc().is_ok() as u8,
            m.check_reply_links().is_ok() as u8
        );
    }
    if ok54 {
        PLATFORM.put_str("ok 54 - core cap message\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 54 - core cap message: recv_caps={t5_caps} caller_caps={t0_caps} bound={} ready={} caller={caller:?}",
            blocked as u8, t0_ready as u8
        );
        *fails += 1;
    }

    // --------------------------------------- TAP 55: Revoke finalise (caller/waiter/hàng)
    // Cây Untyped 2: epA/epB/epC/repA/ntnA — Revoke ⇒ caller Ready, Recv/Notif waiter thức,
    // hàng rỗng + `ipc_dropped` cộng đủ.
    let u2cap = m
        .retype(us, ObjType::Untyped, 12, 0)
        .expect("kt63: retype Untyped 2");
    m.put_cap(root, 20, u2cap).expect("kt63: put cap u2");
    let u2s = SlotRef::new(root, 20);
    let mk2 = |m: &mut Machine, ty: ObjType, slot: usize| -> ObjId {
        let cap = m.retype(u2s, ty, 0, 0).expect("kt63: retype cây 2");
        let obj = m.cap_obj(cap).expect("kt63: cap_obj cây 2");
        m.put_cap(root, slot, cap).expect("kt63: put cây 2");
        obj
    };
    let _ep_a = mk2(m, ObjType::Endpoint, 21);
    let ep_b = mk2(m, ObjType::Endpoint, 22);
    let _ep_c = mk2(m, ObjType::Endpoint, 23);
    let rep_a = mk2(m, ObjType::Reply, 24);
    let _ntn_a = mk2(m, ObjType::Notification, 25);
    let ep_as = SlotRef::new(root, 21);
    let ep_bs = SlotRef::new(root, 22);
    let ep_cs = SlotRef::new(root, 23);
    let rep_as = SlotRef::new(root, 24);
    let ntn_as = SlotRef::new(root, 25);
    let sent_truoc = m.ipc_sent();
    let dropped_truoc = m.ipc_dropped();
    // repA.caller = t0 qua Call epC (t4 là waiter cấp Reply — phải có receive window).
    m.tcb_set_recv_window(SlotRef::new(root, 5), win(58))
        .expect("kt63: bind window t4");
    m.recv(t4, ep_cs, Some(rep_as)).expect("kt63: Recv epC");
    m.call_msg(t0, ep_cs, msg_caps(30, &[0x77], &[src[0]]))
        .expect("kt63: Call epC");
    let caller55 = m.reply_caller(rep_a).unwrap_or(None) == Some(t0);
    // Recv waiter trên epA (t3) + hàng trên epB (t2) + Notif waiter trên ntnA (t4).
    m.recv(t3, ep_as, None).expect("kt63: Recv epA");
    m.send(t2, ep_bs, msg_caps(31, &[0x88], &[src[1]]))
        .expect("kt63: Send epB");
    let queued55 = m.endpoint_send_len(ep_b).unwrap_or(9);
    m.recv(t4, ntn_as, None).expect("kt63: Wait ntnA");
    let deleted55 = m.revoke(root, Some(20)).unwrap_or(0);
    let t0_free = m.tcb_state(t0) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t0) == Ok(Some(KError::Cancelled));
    let t3_free = m.tcb_state(t3) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t3) == Ok(Some(KError::Cancelled));
    let t4_free = m.tcb_state(t4) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t4) == Ok(Some(KError::Cancelled));
    // **KT-64a (D2-90)** — sender `BlockedOnSend` trên Endpoint bị finalise cũng phải
    // `Cancelled + Ready` (nhánh core trước KT-64a để **treo**; nay đóng nợ D2-90).
    let t2_free = m.tcb_state(t2) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t2) == Ok(Some(KError::Cancelled));
    let pending55 = m.ipc_pending();
    let dropped55 = m.ipc_dropped().saturating_sub(dropped_truoc);
    let sent55 = m.ipc_sent().saturating_sub(sent_truoc);
    let ok55 = caller55
        && queued55 == 1
        && deleted55 >= 5
        && t0_free
        && t2_free
        && t3_free
        && t4_free
        && pending55 == 0
        && dropped55 == 1
        && sent55 >= 2
        && m.check_ipc().is_ok()
        && m.check_reply_links().is_ok()
        && m.check_notifications().is_ok()
        && vifast_ktest::invariants::check_all(m, u).all_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: revoke deleted={} caller=Ready caller_err=Cancelled recv_waiter=Ready notif_waiter=Ready sender=Ready sender_err=Cancelled queue={} queued={} pending={} sent={} dropped={} check_ipc={} check_reply_links={} check_notifications={}",
            deleted55,
            pending55,
            queued55,
            pending55,
            sent55,
            dropped55,
            m.check_ipc().is_ok() as u8,
            m.check_reply_links().is_ok() as u8,
            m.check_notifications().is_ok() as u8
        );
    }
    if ok55 {
        PLATFORM.put_str("ok 55 - core revoke finalise\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 55 - core revoke finalise: caller={caller55} t0={t0_free} t2={t2_free} t3={t3_free} t4={t4_free} pending={pending55} dropped={dropped55} deleted={deleted55}"
        );
        *fails += 1;
    }
}

/// Pha core: TAP **43..52**; trả số TAP đỏ.
pub fn core_phase() -> usize {
    let mut fails = 0usize;
    // D2-83: object layer **vẫn** là chủ sở hữu Frame/PageTable/VSpace; `init` cho report +
    // cap Untyped để TAP 51 chứng minh VM sống ở đó, không nhân đôi chủ sở hữu.
    let obj = aobj::init();
    let report = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    // **KT-411** (`B5`) — TAP **43..52** nay là **một** bộ ca dùng chung 3 arch
    // (`vifast_ktest::core_suite`); bản sao cục bộ của riscv64 đã xoá. `record` ghi bốn số chỉ
    // `init` biết (base/arena/untyped/harts) để `CoreBridge::link` dựng lại báo cáo mà không cần
    // `static mut` mới.
    vifast_plat_riscv64::ktest_bridge::record(&report, &obj);
    let link = vifast_plat_riscv64::ktest_bridge::link_report();
    if !vifast_ktest::core_suite::link_tap(&PLATFORM, &link) {
        fails += 1;
    }
    // TAP 53..60 của riêng riscv64 chạy **trong cùng** lượt giữ khoá với TAP 44..52 (khoá không
    // re-entrant) và dùng lại `Rig` do bộ chung dựng — đúng thứ tự cũ.
    core_bridge::with_machine(|m| {
        let (f, _taps, rig) =
            vifast_ktest::core_suite::run_locked(&PLATFORM, &PLATFORM, m, &link, task_ready);
        fails += f;
        // ------------------------------------------- TAP 53..55: chuyển cap + Revoke (KT-63)
        cap_ipc_taps(m, report.untyped, &mut fails);
        // ------------------------------------------- TAP 56..58, 60: KT-64 (trong khoá core)
        crate::ktest_core64::slot_arena_tap(m, report.untyped, &rig, &mut fails);
        crate::ktest_core64::bind_tap(m, report.untyped, &mut fails);
        crate::ktest_core64::resume_tap(m, report.untyped, &mut fails);
    });
    // TAP 59 chạy **ngoài** khoá core (tự `with_machine` per `FlushRequest`), trước TAP 58..60
    // về số hiệu nhưng sau phần khoá — thứ tự `ok` là ...55, 56, 57, 58, 59, 60.
    crate::ktest_core64::tlb_tap(&mut fails);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# core: phase fails={} static={} pin={} image_cap={}",
            fails,
            report.static_bytes,
            report.pin,
            core_bridge::IMAGE_CAP_BYTES
        );
    }
    fails
}

/// **KT-412** — pha **SMP dùng chung** (`vifast_ktest::smp_suite`, TAP **71..73**).
///
/// Vì sao ở đây mà không ở `ktest_smp.rs`: `ktest_smp.rs` kiểm **bring-up phần cứng** (SBI HSM,
/// stack/guard, PLIC, barrier) và là bản **riêng của riscv64**; pha này kiểm **ngữ nghĩa
/// scheduler đa hart của core** và là **cùng một hàm** mà ảnh `aarch64` gọi (chỉ khác số TAP
/// đầu) — đúng yêu cầu "chạy giống hệt nhau trên 2 arch" của KT-412.
///
/// Chạy **sau** pha FS/khoá và **trước** `trap nested panic` (TAP cuối, boot kết thúc bằng
/// `PANIC` có chủ đích) nên `Machine` vẫn sống và Untyped boot còn chỗ cho 8 TCB + 8
/// `SchedContext` của bộ ca.
pub fn smp_phase() -> usize {
    let outcome = vifast_ktest::smp_suite::run(&PLATFORM, &PLATFORM, 71);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# smp core: taps={} fails={}",
            outcome.taps, outcome.fails
        );
    }
    outcome.fails
}
