// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-64 — TAP **56..59** của ảnh `ktest`: slot CNode từ arena, `SchedContext.Bind`, resume sau
//! fault, và `TlbFlush` thật (flush cục bộ `sfence.vma` + chờ đủ ack trước khi TCB chạy lại).
//!
//! Tách khỏi `ktest_core.rs` để giữ luật R1 (≤ 1.500 dòng/tệp); dùng chung `Rig` của pha core.
//! `TlbFlush` chạy **ngoài** khoá `core_bridge` (lock không re-entrant) — nó tự `with_machine`
//! cho từng `FlushRequest` của object layer.

use core::fmt::Write;

use vifast_core::vm::{FlushRequest, MapFlags, ObjKind};
use vifast_core::{
    FaultKind, KError, Machine, Message, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState, TlbEnv,
    TlbScope,
};
use vifast_plat_common::{Platform, PlatformWriter};

use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::probe::{self, ProbeKind};
use vifast_plat_riscv64::Riscv64Virt;

use crate::ktest_core::{err_name, err_name_opt, state_name, Rig};

static PLATFORM: Riscv64Virt = Riscv64Virt;

// =============================================================================================
// KT-64 — TAP 56..59: slot CNode từ arena, `SchedContext.Bind`, resume sau fault, `TlbFlush` thật
// =============================================================================================

/// VA của ca `TlbFlush` (ngoài mọi mapping kernel; cùng vùng root index với TAP 35).
const TLB64_VA: u64 = 0x4000_8000;

/// Env TLB của **kernel**: core quyết định tập hart/thứ tự/chờ ack, arch làm `sfence.vma` thật.
///
/// `harts` = hart đang chạy **model core** (D2-88(a): core chỉ chạy trên hart gọi — `core_bridge`
/// giữ một `Machine` sau spin lock, chưa có context switch U-mode). Vì vậy `active_on` giao với
/// mask này ⇒ **targets = 0** trong nhân 1 hart: flush **cục bộ** là bắt buộc và đủ; đường
/// multi-hart (post + chờ ack) được pin bằng test host + sẽ nối vào IPI ở K3.5 sau (KT-65).
struct KernelTlb {
    harts: u64,
}

impl TlbEnv for KernelTlb {
    fn active_on(&self) -> u64 {
        vifast_plat_riscv64::tlb::active_on() & self.harts
    }

    fn generation(&self) -> u64 {
        vifast_plat_riscv64::tlb::generation()
    }

    fn flush_local(&mut self, hart: u16, scope: TlbScope, asid: u16, va: u64) {
        // Trong nhân, hart nhận flush luôn là hart đang chạy (một model core).
        debug_assert_eq!(hart as usize, Riscv64Virt.hart_id());
        match scope {
            TlbScope::Page { va: page } => vifast_plat_riscv64::tlb::sfence_page(asid, page),
            TlbScope::Asid => vifast_plat_riscv64::tlb::sfence_asid(asid),
            TlbScope::All => vifast_plat_riscv64::tlb::sfence_all(),
        }
        let _ = va;
    }
}

type ObjMac = vifast_core::vm::Machine<aobj::PhysmapMem>;

/// Xả **mọi** `FlushRequest` object layer đang chờ **qua core** (`Machine::tlb_flush`): core
/// quyết định `active_on`/targets/chờ ack, arch thực thi `sfence.vma`. Trả số yêu cầu đã xử lý;
/// `seq` = seq cuối.
fn core_flush_requests(om: &mut ObjMac, seq: &mut u64) -> usize {
    let mut n = 0usize;
    om.drain_flushes(|req| {
        let (scope, asid, va) = match req {
            FlushRequest::Page { asid, va } => (TlbScope::Page { va }, asid, va),
            FlushRequest::Asid { asid } => (TlbScope::Asid, asid, 0),
            FlushRequest::All => (TlbScope::All, 0, 0),
        };
        let hart = Riscv64Virt.hart_id();
        let mut env = KernelTlb {
            harts: 1u64 << hart,
        };
        if let Ok(r) = core_bridge::with_machine(|m| m.tlb_flush(&mut env, scope, asid, va)) {
            *seq = r.seq;
            n += 1;
        }
    });
    n
}

/// TAP **59** — `TlbFlush` thật: map → đọc qua VA (phần cứng) → `protect`/`unmap`/**đổi root**
/// ⇒ mỗi thay đổi đi qua `Machine::tlb_flush` (flush cục bộ `sfence.vma` + chờ đủ ack); sau
/// `unmap` một **load thật** phải fault `scause=13` tại đúng VA ⇒ **không còn trang cũ đọc được**.
pub(crate) fn tlb_tap(fails: &mut usize) {
    let obj = aobj::init();
    let om = aobj::machine();
    let (vs, fr) = match (
        om.retype(obj.untyped0, ObjKind::VSpace, 0),
        om.retype(obj.untyped0, ObjKind::Frame, 12),
    ) {
        (Ok(vs), Ok(fr)) => (vs, fr),
        (a, b) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok - core tlb flush: retype vs={} frame={}",
                a.is_ok() as u8,
                b.is_ok() as u8
            );
            *fails += 1;
            return;
        }
    };
    let root = om.vspace_root_pa(vs).unwrap_or(0);
    let asid = om.vspace_asid(vs).unwrap_or(0);
    let prepared = aobj::copy_kernel_root(root);
    let switched = prepared && aobj::switch_root(root, asid);
    let marker = 0x4b54_3634_544c_4201u64;
    let mut requests = 0usize;
    let mut seq = 0u64;
    let mut map_ok = false;
    let mut read_ok = false;
    let mut protect_ok = false;
    let mut read_after_protect = false;
    let mut unmap_ok = false;
    let mut stale_load = 0u64;
    let mut stval_ok = false;
    if switched {
        map_ok = om.map(vs, TLB64_VA, fr, MapFlags::RW_NX_USER).is_ok();
        requests += core_flush_requests(om, &mut seq);
        let w = aobj::va_write_u64_sum(TLB64_VA, marker);
        read_ok = w && aobj::va_read_u64_sum(TLB64_VA) == Some(marker) && aobj::sum_off();
        let ro = MapFlags {
            read: true,
            write: false,
            exec: false,
            user: true,
            device: false,
        };
        protect_ok = om.protect(vs, TLB64_VA, ro).is_ok();
        requests += core_flush_requests(om, &mut seq);
        read_after_protect = protect_ok && aobj::va_read_u64_sum(TLB64_VA) == Some(marker);
        unmap_ok = om.unmap(vs, TLB64_VA).is_ok();
        requests += core_flush_requests(om, &mut seq);
        if let Some(t) = probe::probe_fault(ProbeKind::Load, TLB64_VA) {
            stale_load = t.scause;
            stval_ok = t.stval == TLB64_VA;
        }
    }
    let restored = aobj::restore_kernel_root() && aobj::kernel_root_active();
    let root_fault = if restored {
        probe::probe_fault(ProbeKind::Load, TLB64_VA)
            .map(|t| t.scause)
            .unwrap_or(0)
    } else {
        0
    };
    let _ = om.revoke(vs);
    let _ = om.revoke(fr);
    let clean = om.check_objmem().is_ok();
    let (pending, local, seq_now, mail_ok, disp_ok) = core_bridge::with_machine(|m| {
        (
            m.tlb_pending(0).ok().flatten(),
            m.tlb_local_flushes(),
            m.tlb_seq(),
            m.check_mailboxes().is_ok(),
            m.dispatch(0).is_ok(),
        )
    });
    let ok = switched
        && restored
        && map_ok
        && read_ok
        && protect_ok
        && read_after_protect
        && unmap_ok
        && stale_load == 13
        && stval_ok
        && root_fault == 13
        && requests >= 3
        && pending.is_none()
        && local >= 3
        && seq_now == seq
        && mail_ok
        && disp_ok
        && clean;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tlb: core asid={} requests={} seq={} local={} targets=0 acked=0 pending={} stale_load={} stval_ok={} protect_read={} restore={} root_fault={} harts=1 check={}",
            asid,
            requests,
            seq,
            local,
            pending.unwrap_or(0),
            stale_load,
            stval_ok as u8,
            read_after_protect as u8,
            restored as u8,
            root_fault,
            (clean && mail_ok && disp_ok) as u8
        );
    }
    if ok {
        PLATFORM.put_str("ok 59 - core tlb flush\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok - core tlb flush: switched={} map={} read={} protect={} unmap={} stale={} stval={} restore={} req={} pending={:?} disp={} clean={}",
            switched as u8,
            map_ok as u8,
            read_ok as u8,
            protect_ok as u8,
            unmap_ok as u8,
            stale_load,
            stval_ok as u8,
            restored as u8,
            requests,
            pending,
            disp_ok as u8,
            clean as u8
        );
        *fails += 1;
    }
}

/// TAP **56** — byte slot CNode **từ arena**: `Retype` CNode từ Untyped ⇒ `cnode_slot_span` nằm
/// **trong** vùng arena tại `offset + cnode_base`, `untyped_used` tăng đúng `cnode_base+slots*48`,
/// `Revoke` trả byte (span biến mất). Một nguồn sự thật cho byte arena.
pub(crate) fn slot_arena_tap(m: &mut Machine, u: ObjId, rig: &Rig, fails: &mut usize) {
    let used0 = m.untyped_used(u);
    let cap = match m.retype(rig.us, ObjType::CNode, 4, 0) {
        Ok(cap) => cap,
        Err(e) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "not ok - core slot arena: retype {}", e.name());
            *fails += 1;
            return;
        }
    };
    let cnode = m.cap_obj(cap).unwrap_or(0);
    let used1 = m.untyped_used(u);
    let span = m.cnode_slot_span(cnode).ok().flatten();
    let arena = m.untyped_arena(u).unwrap_or((0, 0));
    let bytes = 16usize * 48;
    let within = span
        .map(|(pa, len)| {
            pa >= arena.0
                && pa
                    .checked_add(len as u64)
                    .map(|end| end <= arena.0 + arena.1 as u64)
                    .unwrap_or(false)
                && len == bytes
        })
        .unwrap_or(false);
    let grown = used1 == used0 + 64 + bytes;
    let child = m.root_cap(cnode);
    let put_ok = m.put_cap(cnode, 0, child).is_ok();
    let back_ok = m.cap_at(cnode, 0) == Ok(child);
    let check_ok = m.check_bytes(u).is_ok() && m.check_all_bytes().is_ok();
    let rev = m.revoke(rig.us.cnode, Some(rig.us.slot)).is_ok();
    let used2 = m.untyped_used(u);
    let span2 = m.cnode_slot_span(cnode).ok().flatten();
    // `Revoke` Untyped trả **toàn bộ** arena của cây (mọi object TAP 53..55 còn sống) ⇒ `used2 = 0`.
    let revoke_back = used2 == 0 && span2.is_none();
    let ok = within && grown && put_ok && back_ok && check_ok && rev && revoke_back;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# slot: arena=0x{:x}+{} span=0x{:x} bytes={} within={} used0={} used1={} used2={} grown={} slot_ok={} revoke_back={} check={}",
            arena.0,
            arena.1,
            span.map(|s| s.0).unwrap_or(0),
            span.map(|s| s.1).unwrap_or(0),
            within as u8,
            used0,
            used1,
            used2,
            grown as u8,
            back_ok as u8,
            revoke_back as u8,
            check_ok as u8
        );
    }
    if ok {
        PLATFORM.put_str("ok 56 - core slot arena\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok - core slot arena: within={} grown={} put={} back={} check={} rev={} used0={} used1={} used2={} span2={:?}",
            within as u8,
            grown as u8,
            put_ok as u8,
            back_ok as u8,
            check_ok as u8,
            rev as u8,
            used0,
            used1,
            used2,
            span2
        );
        *fails += 1;
    }
}

/// TAP **57** — `SchedContext.Bind`: `bound_sc` **sống** (hai chiều), 1:1 đúng mã lỗi, `Signal`
/// chỉ đánh thức TCB **đúng bound**, `Unbind`/`Delete` gỡ binding (không mồ côi).
pub(crate) fn bind_tap(m: &mut Machine, u: ObjId, fails: &mut usize) {
    let root = m.add_cnode(32);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("kt64: cap untyped");
    let us = SlotRef::new(root, 0);
    let mk = |m: &mut Machine, ty: ObjType, slot: usize| -> (ObjId, SlotRef) {
        let cap = m.retype(us, ty, 0, 0).expect("kt64: retype");
        let obj = m.cap_obj(cap).expect("kt64: cap_obj");
        m.put_cap(root, slot, cap).expect("kt64: put cap");
        (obj, SlotRef::new(root, slot))
    };
    let (sc, sc_s) = mk(m, ObjType::SchedContext, 1);
    let (_sc2, sc2_s) = mk(m, ObjType::SchedContext, 2);
    let (n, n_s) = mk(m, ObjType::Notification, 3);
    let (_rep, rep_s) = mk(m, ObjType::Reply, 4);
    let (_ep, ep_s) = mk(m, ObjType::Endpoint, 5);
    let (t0, t0_s) = mk(m, ObjType::Tcb, 10);
    let (t1, _t1_s) = mk(m, ObjType::Tcb, 11);
    m.start_tcb(t0).expect("kt64: Ready");
    m.start_tcb(t1).expect("kt64: Ready");
    let n_cap = m.cap_at(root, 3).unwrap_or(0);
    // Bind hai chiều.
    let bind_ok = m.sched_context_bind(sc_s, n_s).is_ok();
    let two_way = m.notification_bound_sc(n).ok().flatten() == Some(sc)
        && m.sched_context_bound(sc).ok().flatten() == Some(n_cap);
    // Notification đã bind SC khác ⇒ IllegalOperation (không đổi gì).
    let second = m.sched_context_bind(sc2_s, n_s);
    let second_ok = second.as_ref().err() == Some(&KError::IllegalOperation);
    // SC đổi target ⇒ gỡ phía notification cũ; target sai kiểu ⇒ WrongObjectType.
    let rebind_ok = m.sched_context_bind(sc_s, rep_s).is_ok();
    let cleared = m.notification_bound_sc(n).ok().flatten().is_none();
    let wrong = m.sched_context_bind(sc_s, ep_s).err() == Some(KError::WrongObjectType);
    // Signal chỉ đánh thức TCB **đúng bound**: t0 bound n và chờ Recv; t1 chờ Recv (không bound).
    let bound_t0 = m.tcb_bind_notification(t0_s, n_s).is_ok();
    let r0 = m.recv(t0, ep_s, None).is_ok();
    let r1 = m.recv(t1, ep_s, None).is_ok();
    let sig = m.notification_signal(n, 0x42).is_ok();
    let wake_ok = m.tcb_state(t0).ok() == Some(TcbState::Ready);
    let other_blocked = matches!(m.tcb_state(t1).ok(), Some(TcbState::BlockedOnRecv { .. }));
    // Unbind xoá hai chiều (SC đang bind Reply — không có phía notification); rồi bind lại n và
    // Delete cap Notification ⇒ binding SC↔n bị gỡ (không để `bound_sc` mồ côi).
    let unbind1 = m.sched_context_unbind(sc_s).is_ok();
    let unbind_clear = m.sched_context_bound(sc).ok().flatten().is_none();
    let bind_again = m.sched_context_bind(sc_s, n_s).is_ok();
    let del = m.delete(n_s).is_ok();
    let del_clear = m.sched_context_bound(sc).ok().flatten().is_none();
    let undbind_idem = m.sched_context_unbind(sc_s).is_ok();
    let check_ok =
        m.check_notifications().is_ok() && m.check_caps().is_ok() && m.check_ipc().is_ok();
    let ok = bind_ok
        && two_way
        && second_ok
        && rebind_ok
        && cleared
        && wrong
        && bound_t0
        && r0
        && r1
        && sig
        && wake_ok
        && other_blocked
        && unbind1
        && unbind_clear
        && bind_again
        && del
        && del_clear
        && undbind_idem
        && check_ok;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# bind: sc={} ntfn={} two_way={} second={} rebind_clears={} wrong_target={} signal_wake={} other_blocked={} unbind={} delete_clears={} check={}",
            sc,
            n,
            two_way as u8,
            err_name(&second),
            cleared as u8,
            wrong as u8,
            wake_ok as u8,
            other_blocked as u8,
            (unbind1 && unbind_clear && undbind_idem) as u8,
            del_clear as u8,
            check_ok as u8
        );
    }
    if ok {
        PLATFORM.put_str("ok 57 - core sched bind\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok - core sched bind: bind={} two_way={} second={} rebind={} cleared={} wrong={} bound={} recv={}/{} sig={} wake={} other={} unbind={} del={} check={}",
            bind_ok as u8,
            two_way as u8,
            err_name(&second),
            rebind_ok as u8,
            cleared as u8,
            wrong as u8,
            bound_t0 as u8,
            r0 as u8,
            r1 as u8,
            sig as u8,
            wake_ok as u8,
            other_blocked as u8,
            (unbind1 && unbind_clear) as u8,
            del_clear as u8,
            check_ok as u8
        );
        *fails += 1;
    }
}

/// TAP **58** — `tcb_resume_with_regs`: resume sau fault **một cửa** (ghi `sepc`/regs trước khi
/// `Ready`), `BlockedOnReply` đúng caller, `Ready`/`Sleeping`; **cấm** `Running`/`Inactive`/
/// message mang cap; `exit_code` giữ. Đây là model API — **không** syscall nào (0..15) mở đường
/// cho user tự resume (`# resume: user_path=0`).
pub(crate) fn resume_tap(m: &mut Machine, u: ObjId, fails: &mut usize) {
    let root = m.add_cnode(32);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("kt64: cap untyped");
    let us = SlotRef::new(root, 0);
    let mk = |m: &mut Machine, ty: ObjType, slot: usize| -> (ObjId, SlotRef) {
        let cap = m.retype(us, ty, 0, 0).expect("kt64: retype");
        let obj = m.cap_obj(cap).expect("kt64: cap_obj");
        m.put_cap(root, slot, cap).expect("kt64: put cap");
        (obj, SlotRef::new(root, slot))
    };
    let (t, t_s) = mk(m, ObjType::Tcb, 1);
    let (t2, _t2_s) = mk(m, ObjType::Tcb, 2);
    let (_t3, t3_s) = mk(m, ObjType::Tcb, 3);
    let (_ep, ep_s) = mk(m, ObjType::Endpoint, 4);
    let (rep, rep_s) = mk(m, ObjType::Reply, 5);
    m.start_tcb(t).expect("kt64: Ready");
    m.start_tcb(t2).expect("kt64: Ready");
    m.tcb_set_fault_ep(t_s, ep_s).expect("kt64: fault_ep");
    m.inject_fault(t, FaultKind::UnknownSyscall { nr: 7 })
        .expect("kt64: fault");
    let fault_state = matches!(
        m.tcb_state(t).ok(),
        Some(TcbState::BlockedOnSend {
            call: true,
            fault: Some(_),
            ..
        })
    );
    let msg = Message::new(
        vifast_core::MessageInfo::new(0, 0, false, 0, 2).expect("info"),
        &[0xAA, 0xBB],
    )
    .expect("message");
    let resume_ok = m.tcb_resume_with_regs(t_s, msg).is_ok();
    let ready = m.tcb_state(t).ok() == Some(TcbState::Ready);
    let regs_ok = m.tcb_regs(t).ok() == Some([0xAA, 0xBB, 0, 0]);
    let exit_kept = m
        .tcb_info_obj(t)
        .map(|i| i.exit_code.is_none())
        .unwrap_or(false);
    // `Running` ⇒ cấm; thanh ghi không đổi.
    let _ = m.set_current_hart(0);
    let running = m
        .transition_tcb(t, NonReadyTarget::Running { hart: 0 })
        .is_ok();
    let running_err = m
        .tcb_resume_with_regs(
            t_s,
            Message::new(
                vifast_core::MessageInfo::new(1, 0, false, 0, 1).expect("info"),
                &[0x77],
            )
            .expect("message"),
        )
        .err();
    let regs_after = m.tcb_regs(t).ok() == Some([0xAA, 0xBB, 0, 0]);
    let _ = m.tcb_yield(t);
    // `BlockedOnReply` đúng caller ⇒ resume được; `Reply.caller` sạch.
    let r2 = m.recv(t2, ep_s, Some(rep_s)).is_ok();
    let call = m
        .call_msg(
            t,
            ep_s,
            Message::new(
                vifast_core::MessageInfo::new(2, 0, false, 0, 1).expect("info"),
                &[0x55],
            )
            .expect("message"),
        )
        .is_ok();
    let blocked_reply = matches!(m.tcb_state(t).ok(), Some(TcbState::BlockedOnReply { .. }));
    let resume_reply = m
        .tcb_resume_with_regs(
            t_s,
            Message::new(
                vifast_core::MessageInfo::new(3, 0, false, 0, 1).expect("info"),
                &[0x66],
            )
            .expect("message"),
        )
        .is_ok();
    let reply_clear = m.reply_caller(rep).ok().flatten().is_none();
    // Message mang cap ⇒ InvalidArgument; `Inactive` ⇒ IllegalOperation.
    let cap = m.root_cap(t);
    let bad = Message::with_cap_ids(4, &[1], &[cap]).expect("message cap");
    let cap_err = m.tcb_resume_with_regs(t_s, bad).err();
    let inactive_err = m
        .tcb_resume_with_regs(
            t3_s,
            Message::new(
                vifast_core::MessageInfo::new(5, 0, false, 0, 1).expect("info"),
                &[1],
            )
            .expect("message"),
        )
        .err();
    let user_nr_max = vifast_core::spec::SYS_DEBUG_GET_INFO as u32;
    let check_ok = m.check_ipc().is_ok() && m.check_sched().is_ok() && m.check_caps().is_ok();
    let ok = fault_state
        && resume_ok
        && ready
        && regs_ok
        && exit_kept
        && running
        && running_err == Some(KError::IllegalOperation)
        && regs_after
        && r2
        && call
        && blocked_reply
        && resume_reply
        && reply_clear
        && cap_err == Some(KError::InvalidArgument)
        && inactive_err == Some(KError::IllegalOperation)
        && user_nr_max == 15
        && check_ok;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# resume: fault={} state={} regs=0x{:x}/0x{:x} running={} regs_kept={} reply={} reply_caller=0 cap={} inactive={} exit_kept={} user_nr_max={} user_path=0 check={}",
            fault_state as u8,
            state_name(m.tcb_state(t).unwrap_or(TcbState::Dead)),
            0xAA,
            0xBB,
            err_name_opt(running_err),
            regs_after as u8,
            resume_reply as u8,
            err_name_opt(cap_err),
            err_name_opt(inactive_err),
            exit_kept as u8,
            user_nr_max,
            check_ok as u8
        );
    }
    if ok {
        PLATFORM.put_str("ok 58 - core fault resume\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok - core fault resume: fault={} resume={} ready={} regs={} exit={} running={} regs_kept={} reply={} blocked={} resume_reply={} clear={} cap={} inactive={} nr={} check={}",
            fault_state as u8,
            resume_ok as u8,
            ready as u8,
            regs_ok as u8,
            exit_kept as u8,
            running as u8,
            regs_after as u8,
            r2 as u8,
            blocked_reply as u8,
            resume_reply as u8,
            reply_clear as u8,
            err_name_opt(cap_err),
            err_name_opt(inactive_err),
            user_nr_max,
            check_ok as u8
        );
        *fails += 1;
    }
}
