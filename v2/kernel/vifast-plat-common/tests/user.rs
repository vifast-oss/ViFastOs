// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho ngữ nghĩa U-mode/syscall (KT-53 → **KT-62**) của
//! `vifast-plat-common::user`.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! D2-72 Q12: Miri **không** áp kernel `no_std`/asm/MMIO; phần *ngữ nghĩa* syscall (façade
//! `abi::dispatch` + bảng dispatch thật của `vifast-core::syscall`) chạy được trên host — tệp
//! này kiểm **cùng** mã sẽ chạy trong trap handler U-mode trên QEMU:
//!
//! - KT-62: **16 syscall `0..15`** đi vào `Machine` thật (không còn nhánh `NotSupported` giả);
//!   host test kiểm nhóm cap/sổ sách của façade;
//! - `DebugPutStr` (12): kiểm độ dài/dải user trước khi chạm bộ nhớ, mã lỗi từ lớp đọc user
//!   được truyền nguyên (không panic);
//! - ngoài dải `0..15` (kể cả EID SBI) ⇒ `RangeError` 19 (D2-79) + `bad_calls` ghi sổ;
//! - `Exit` (14) ⇒ `SyscallAction::Exit` + task `Exited` + `exit_code`;
//! - `DebugHalt` (13) ⇒ `SyscallAction::Halt`; TCB chặn ⇒ `Switched`;
//! - `DebugGetInfo` (15) ⇒ kênh `a1/a3/a4/a5` đúng `spec/abi.toml` `[debug_info]`.

use vifast_core::syscall::{SyscallEnv, SyscallOutcome, REPLY_NONE};
use vifast_core::{Machine, NonReadyTarget, ObjType, SlotRef, TcbState};
use vifast_plat_common::user::{
    self as abi, SyscallAction, TaskState, UserRange, UserTask, ERR_NOT_MAPPED, MAX_DEBUG_PUT,
    SYS_DEBUG_GET_INFO, SYS_DEBUG_HALT, SYS_DEBUG_PUT_STR, SYS_EXIT,
};

const BASE: u64 = 0x1000_0000;

/// Hook arch giả: ghi lại lần `put_str`, trả `len` hoặc mã lỗi cấu hình trước.
struct FakeEnv {
    calls: usize,
    last_ptr: u64,
    last_len: u64,
    fail_with: Option<u64>,
    now: u64,
    harts: u64,
}

impl FakeEnv {
    fn new() -> Self {
        Self {
            calls: 0,
            last_ptr: 0,
            last_len: 0,
            fail_with: None,
            now: 1_000,
            harts: 2,
        }
    }
}

impl SyscallEnv for FakeEnv {
    fn put_str(&mut self, ptr: u64, len: u64) -> Result<u64, u64> {
        self.calls += 1;
        self.last_ptr = ptr;
        self.last_len = len;
        match self.fail_with {
            Some(code) => Err(code),
            None => Ok(len),
        }
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(ERR_NOT_MAPPED)
    }
    fn now_ns(&mut self) -> u64 {
        self.now += 10;
        self.now
    }
    fn features(&mut self) -> u64 {
        0b1111
    }
    fn num_harts(&mut self) -> u64 {
        self.harts
    }
}

struct Rig {
    tcb: u32,
    tcb_slot: SlotRef,
    ep_slot: SlotRef,
}

/// Machine + 1 TCB `Running{0}` + CNode có cap Endpoint (để `Recv` chặn được).
fn rig(task: &UserTask) -> (Machine, Rig) {
    let _ = task;
    let mut m = Machine::with_harts(1);
    m.add_untyped(BASE, 1 << 20);
    let u = m.add_untyped(BASE, 1 << 20); // Untyped thứ 2 (id 2) chứa object
    let root = m.add_cnode(16);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("cap untyped");
    let mut obj = [0u32; 6];
    for (i, ty) in [
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::SchedControl,
        ObjType::SchedContext,
        ObjType::Reply,
        ObjType::Notification,
    ]
    .iter()
    .enumerate()
    {
        let cap = m.retype(SlotRef::new(root, 0), *ty, 0, 0).expect("retype");
        m.put_cap(root, 1 + i, cap).expect("put cap");
        obj[i] = m.cap_obj(cap).expect("obj");
    }
    let cs = m.root_cap(root);
    m.put_cap(root, 9, cs).expect("cap cspace");
    let tcb_slot = SlotRef::new(root, 1);
    m.tcb_configure(tcb_slot, Some(SlotRef::new(root, 9)))
        .expect("cspace");
    m.sched_control_configure(SlotRef::new(root, 3), SlotRef::new(root, 4), 100, 1_000)
        .expect("SC");
    m.set_sched_params(tcb_slot, Some(SlotRef::new(root, 4)))
        .expect("params");
    m.tcb_set_priority(tcb_slot, 3, 10).expect("priority");
    m.start_tcb(obj[0]).expect("Ready");
    m.transition_tcb(obj[0], NonReadyTarget::Running { hart: 0 })
        .expect("Running");
    (
        m,
        Rig {
            tcb: obj[0],
            tcb_slot,
            ep_slot: SlotRef::new(root, 2),
        },
    )
}

fn args(values: &[u64]) -> [u64; 6] {
    let mut out = [0u64; 6];
    out[..values.len().min(6)].copy_from_slice(&values[..values.len().min(6)]);
    out
}

#[test]
fn debug_put_str_ghi_so_va_tra_kênh_abi() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    let action = abi::dispatch(
        &mut m,
        r.tcb,
        0,
        SYS_DEBUG_PUT_STR,
        args(&[0x4000_0000, 26]),
        &mut env,
        &task,
    );
    assert_eq!(
        action,
        SyscallAction::Return {
            error: 0,
            value: 26,
            info: 0,
            mr: [0; 4]
        }
    );
    assert_eq!(
        (env.calls, env.last_ptr, env.last_len),
        (1, 0x4000_0000, 26)
    );
    let snap = task.snapshot();
    assert_eq!(snap.syscalls, 1);
    assert_eq!(snap.last_nr, SYS_DEBUG_PUT_STR);
    assert_eq!((snap.last_error, snap.last_value), (0, 26));
    assert_eq!((snap.put_calls, snap.put_bytes), (1, 26));
}

#[test]
fn debug_put_str_qua_tran_va_loi_lop_doc_user() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    // Quá trần ⇒ 5, **không** chạm lớp đọc user.
    let action = abi::dispatch(
        &mut m,
        r.tcb,
        0,
        SYS_DEBUG_PUT_STR,
        args(&[0, MAX_DEBUG_PUT + 1]),
        &mut env,
        &task,
    );
    assert_eq!(
        action,
        SyscallAction::Return {
            error: 5,
            value: 0,
            info: 0,
            mr: [0; 4]
        }
    );
    assert_eq!(env.calls, 0);
    // Lỗi lớp đọc user được truyền nguyên mã (11 NotMapped), không panic.
    env.fail_with = Some(ERR_NOT_MAPPED);
    let action = abi::dispatch(
        &mut m,
        r.tcb,
        0,
        SYS_DEBUG_PUT_STR,
        args(&[0xdead_0000, 4]),
        &mut env,
        &task,
    );
    assert_eq!(
        action,
        SyscallAction::Return {
            error: ERR_NOT_MAPPED,
            value: 0,
            info: 0,
            mr: [0; 4]
        }
    );
    assert_eq!(task.snapshot().put_calls, 0, "chỉ đếm lần in thành công");
}

#[test]
fn exit_va_halt_ghi_so_dung() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    let action = abi::dispatch(&mut m, r.tcb, 0, SYS_EXIT, args(&[42]), &mut env, &task);
    assert_eq!(action, SyscallAction::Exit { code: 42 });
    let snap = task.snapshot();
    assert_eq!(snap.state, TaskState::Exited);
    assert_eq!((snap.exits, snap.exit_code), (1, 42));
    assert_eq!(
        m.tcb_state(r.tcb),
        Ok(TcbState::Inactive),
        "Exit không fault_ep ⇒ Inactive"
    );

    // DebugHalt: chỉ trả outcome, không dừng host.
    let task2 = UserTask::new();
    let (mut m2, r2) = rig(&task2);
    let action = abi::dispatch(
        &mut m2,
        r2.tcb,
        0,
        SYS_DEBUG_HALT,
        args(&[7]),
        &mut env,
        &task2,
    );
    assert_eq!(action, SyscallAction::Halt { code: 7 });
}

#[test]
fn ngoai_dai_tra_range_error_19() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    for nr in [16u64, 17, 31, 99, 0x5352_5354, u64::MAX] {
        let action = abi::dispatch(&mut m, r.tcb, 0, nr, args(&[]), &mut env, &task);
        match action {
            SyscallAction::Return { error, .. } => assert_eq!(error, 19, "nr={nr}"),
            other => panic!("nr={nr}: cần Return, thấy {other:?}"),
        }
    }
    let snap = task.snapshot();
    assert_eq!(snap.bad_calls, 6);
    assert_eq!(snap.last_bad_nr, u64::MAX);
    assert_eq!(snap.last_bad_error, 19);
    // TCB không đổi trạng thái.
    assert_eq!(m.tcb_state(r.tcb), Ok(TcbState::Running { hart: 0 }));
}

#[test]
fn tbloc_thanh_switched_khi_tcb_khong_con_running() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    // Recv trên Endpoint rỗng ⇒ TCB `BlockedOnRecv`; façade trả `Switched` (arch phải lập lịch).
    let action = abi::dispatch(
        &mut m,
        r.tcb,
        0,
        3,
        args(&[r.ep_slot.slot as u64, REPLY_NONE]),
        &mut env,
        &task,
    );
    match action {
        SyscallAction::Switched { state } => {
            assert!(matches!(state, TcbState::BlockedOnRecv { .. }));
        }
        other => panic!("cần Switched, thấy {other:?}"),
    }
    assert_eq!(task.snapshot().syscalls, 1);
}

#[test]
fn debug_get_info_tra_a1_a3_a4_a5() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    match abi::dispatch(
        &mut m,
        r.tcb,
        0,
        SYS_DEBUG_GET_INFO,
        args(&[]),
        &mut env,
        &task,
    ) {
        SyscallAction::Return {
            error, value, mr, ..
        } => {
            assert_eq!(error, 0);
            assert_eq!(value, vifast_core::syscall::ABI_VERSION);
            assert_eq!(mr[0], vifast_core::syscall::MIN_COMPAT);
            assert_eq!(mr[1], 0b1111);
            assert_eq!(mr[2], env.harts, "num_harts = số hart nền tảng");
            assert_eq!(mr[3], 0);
        }
        other => panic!("cần Return, thấy {other:?}"),
    }
}

#[test]
fn cap_sai_tra_dung_ma_thay_vi_notsupported_gia() {
    let task = UserTask::new();
    let (mut m, r) = rig(&task);
    let mut env = FakeEnv::new();
    // Slot rỗng ⇒ 1.
    match abi::dispatch(&mut m, r.tcb, 0, 1, args(&[15, 0]), &mut env, &task) {
        SyscallAction::Return { error, .. } => assert_eq!(error, 1),
        other => panic!("cần Return, thấy {other:?}"),
    }
    // Send trên cap TCB ⇒ 2 (không phải "NotSupported giả").
    match abi::dispatch(
        &mut m,
        r.tcb,
        0,
        1,
        args(&[r.tcb_slot.slot as u64, 0]),
        &mut env,
        &task,
    ) {
        SyscallAction::Return { error, .. } => assert_eq!(error, 2),
        other => panic!("cần Return, thấy {other:?}"),
    }
}

#[test]
fn user_range_va_ten_loi() {
    let range = UserRange::new(0x4000_0000, 0x8000_0000);
    assert!(range.contains(0x4000_0000, 1));
    assert!(range.contains(0x7fff_ffff, 1));
    assert!(!range.contains(0x8000_0000, 1));
    assert!(!range.contains(0x7fff_ffff, 2));
    assert!(range.contains(0x4000_0000, 0));
    assert!(!range.contains(u64::MAX, 1));
    assert_eq!(abi::error_name(0), "OK");
    assert_eq!(abi::error_name(19), "RangeError");
    assert_eq!(abi::error_name(1), "InvalidCapability");
}

#[test]
fn facade_khong_re_tao_bang_so_hieu_rieng() {
    // KT-62: hằng số ABI **một nguồn** — façade tái xuất đúng của core (`spec` sinh).
    assert_eq!(SYS_DEBUG_PUT_STR, vifast_core::syscall::SYS_DEBUG_PUT_STR);
    assert_eq!(SYS_DEBUG_HALT, vifast_core::syscall::SYS_DEBUG_HALT);
    assert_eq!(SYS_EXIT, vifast_core::syscall::SYS_EXIT);
    assert_eq!(SYS_DEBUG_GET_INFO, vifast_core::syscall::SYS_DEBUG_GET_INFO);
    assert_eq!(MAX_DEBUG_PUT, vifast_core::syscall::MAX_DEBUG_PUT);
    // `SyscallOutcome` được tái xuất cho arch/tests.
    let _ = core::mem::size_of::<SyscallOutcome>();
}
