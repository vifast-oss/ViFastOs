// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-65 — **hợp đồng procsrv** trên sim (oracle của core): `Exit{code}` ghi vào `last_fault`
//! của người nhận đang `Recv` trên `fault_ep`, TCB chết `Inactive`, `take_last_fault` tiêu thụ
//! **một lần**; `kill` = `Tcb.Suspend(Abort)` đặt cờ `suspended`. Đây là các ngữ nghĩa mà
//! procsrv/shell U-mode dựa vào (không thêm syscall); core có test song song cùng tên hành vi.

use vifast_sim::kobj::{
    FaultKind, Machine, NonReadyTarget, ObjType, SlotRef, SuspendMode, TcbState,
};

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

fn dat_cap(m: &mut Machine, c0: u32, slot_idx: usize, cap: u32) {
    m.put_cap(c0, slot_idx, cap).expect("đặt cap");
}

/// Dựng procsrv + con: cspace dùng chung, `fault_ep` trỏ Endpoint slot 1.
fn rig() -> (Machine, u32, u32, u32) {
    let mut m = Machine::new();
    let c0 = m.add_cnode(32);
    let u = m.add_untyped(0x1000, 1 << 16);
    let u_cap = m.root_cap(u);
    dat_cap(&mut m, c0, 0, u_cap);
    // Cap CNode gốc để `Tcb.Configure cspace=` hợp lệ.
    let c0_cap = m.root_cap(c0);
    dat_cap(&mut m, c0, 4, c0_cap);
    for (s, ty) in [(1, ObjType::Endpoint), (2, ObjType::Tcb), (3, ObjType::Tcb)] {
        let cap = m.retype(slot(c0, 0), ty, 0, 0).expect("retype object");
        dat_cap(&mut m, c0, s, cap);
    }
    let ps = m.cap_obj_at(c0, 2).expect("obj procsrv");
    let child = m.cap_obj_at(c0, 3).expect("obj con");
    m.tcb_configure(slot(c0, 2), Some(slot(c0, 4)))
        .expect("cspace procsrv");
    m.tcb_configure(slot(c0, 3), Some(slot(c0, 4)))
        .expect("cspace con");
    m.tcb_set_fault_ep(slot(c0, 3), slot(c0, 1))
        .expect("fault_ep con");
    (m, c0, ps, child)
}

#[test]
fn kt65_exit_ghi_last_fault_cho_procsrv_roi_inactive() {
    let (mut m, c0, ps, child) = rig();
    m.start_tcb(ps).expect("procsrv Ready");
    // procsrv `Recv` chặn trên EP_FAULT — lượt nhận fault của QD §1 Q-A.
    m.recv(ps, slot(c0, 1), None).expect("procsrv Recv");
    m.exit(child, 42).expect("con Exit");
    assert_eq!(m.tcb_state(child), Ok(TcbState::Inactive));
    assert_eq!(
        m.take_last_fault(ps),
        Ok(Some((child, FaultKind::Exit { code: 42 })))
    );
    assert_eq!(m.take_last_fault(ps), Ok(None), "take chỉ một lần");
}

#[test]
fn kt65_kill_bang_suspend_abort_dat_co_treo() {
    let (mut m, c0, _ps, victim) = rig();
    m.start_tcb(victim).expect("Ready");
    m.transition_tcb(victim, NonReadyTarget::Running { hart: 0 })
        .expect("Running");
    m.tcb_suspend(slot(c0, 3), SuspendMode::Abort)
        .expect("Suspend Abort");
    assert_eq!(m.tcb_state(victim), Ok(TcbState::Ready));
    let snap = m.tcb_read_state(slot(c0, 3)).expect("đọc state");
    assert!(snap.suspended, "Abort phải đặt suspended");
    m.tcb_resume(slot(c0, 3)).expect("Resume");
    let snap = m.tcb_read_state(slot(c0, 3)).expect("đọc state");
    assert!(!snap.suspended);
}
