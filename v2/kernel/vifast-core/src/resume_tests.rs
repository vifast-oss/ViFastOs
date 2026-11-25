// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-314** — test host cho `Tcb.Resume` (method **12** của `[object].Tcb` trong
//! `spec/abi.toml`) qua **invocation** (syscall 0 trên cap TCB).
//!
//! # Vì sao test host
//!
//! Ngữ nghĩa **core** ở đây là: kiểm trạng thái đích (`Ready`/`Inactive`), kiểm `TcbData.entry`
//! (fail-closed khi `0`), chuyển `Running` trên hart của **caller**, xoá `exit_code` (D2-25), và
//! **giao `entry` cho arch** qua hook [`SyscallEnv::tcb_resume`] (D2-83: core **không** ghi
//! `sepc` — đó là CSR của arch). Phần **đặt `sepc`** được đo bằng QEMU thật ở
//! `kernel/vifast-kbin-riscv64/src/ktest_ipc.rs` (TAP `ipc: tcb resume`): tác vụ đích **chạy
//! thật** ở `TcbData.entry` với `tf.sepc = 0` — nếu hook không nạp thì nó fault ở `0`.
//!
//! Ranh giới đã kiểm: (a) số method **lấy từ spec** (`vifast_abi::TCB_RESUME`, không chép tay);
//! (b) arity **chính xác 0** (D2-104) — `Tcb.Resume` không có MR vào; (c) cap TCB phải có
//! `RIGHT_WRITE` ⇒ thiếu là `InsufficientRights` 3; (d) `Inactive` **chưa từng chạy** ⇒
//! `Running` + trả `a1 = entry`; (e) `Ready` ⇒ `Running`; (f) `entry == 0` ⇒ `InvalidArgument`
//! 5, **không** đổi trạng thái (fail-closed); (g) `Running` ⇒ `IllegalOperation` 7 — và **kiểm
//! trạng thái đứng trước** kiểm `entry` (TCB `Running` + `entry = 0` vẫn trả **7**, không phải 5);
//! (h) `Inactive` **đã chạy** (`exit_code = Some`) ⇒ 7 (không tự chạy lại tác vụ đã kết thúc);
//! (i) `exit_code` bị xoá khi resume thành công (D2-25); (j) cap không phải TCB ⇒ `NotSupported`
//! 18 (D2-53 Q11, không trả `Ok` giả).
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke;
use crate::machine::Machine;
use crate::spec::RIGHT_READ;
use crate::state_write::NonReadyTarget;
use crate::syscall::SyscallEnv;
use crate::tcb::TcbState;

/// Hook arch tối thiểu — ghi lại **đúng** `(tcb, entry)` mà core giao cho arch (bằng chứng
/// "core không tự đặt `sepc`": nó **giao** giá trị, arch mới là bên ghi CSR).
#[derive(Default)]
struct ResumeEnv {
    calls: std::vec::Vec<(ObjId, u64)>,
}

impl SyscallEnv for ResumeEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(11)
    }
    fn now_ns(&mut self) -> u64 {
        0
    }
    fn features(&mut self) -> u64 {
        0
    }
    fn num_harts(&mut self) -> u64 {
        1
    }
    fn tcb_resume(&mut self, tcb: ObjId, entry: u64) -> bool {
        self.calls.push((tcb, entry));
        true
    }
}

/// Khoá **nối tiếp** cho các ca đọc **bộ đếm toàn cục** `invoke::stats()` (cùng bài học D2-108B
/// với `tls_tests`/`configure_tests`): `resume` là bộ đếm dùng chung giữa các test song song.
use crate::test_lock::STATS_LOCK as RESUME_COUNTER_LOCK;

/// `MessageInfo`: `length` (bit 28..) = số MR dùng, `label` (bit 0..) = method, `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

fn invoke_on(
    m: &mut Machine,
    tcb: ObjId,
    call_slot: SlotRef,
    len: u8,
    mrs: [u64; 4],
    env: &mut ResumeEnv,
) -> Result<(u64, u64), u64> {
    invoke::dispatch(m, tcb, call_slot, info(invoke::TCB_RESUME, len), &mrs, env)
}

/// Cảnh một tiến trình: CNode 16 slot.
///
/// | ô | nội dung |
/// |---|---|
/// | 0 | cap `CNode` tự (CSpace) |
/// | 1 | TCB **caller** — `Running{hart:0}` (đang gọi invocation) |
/// | 2 | TCB đích **`Inactive`** chưa chạy, `entry = 0x4000_0100` |
/// | 3 | TCB đích **`Inactive`** chưa chạy, `entry = 0` (ca âm fail-closed) |
/// | 4 | TCB đích **`Inactive`** **đã chạy** (`exit_code = Some(0)`) |
/// | 5 | TCB đích **`Ready`** |
/// | 6 | TCB **chỉ đọc** (`RIGHT_READ`) — ca quyền âm |
/// | 7 | cap `Endpoint` — **không phải** TCB (ca âm danh sách trắng) |
/// | 8 | `SchedControl` (cấu hình SC) |
/// | 9 | `SchedContext` cho các TCB sẽ `Running` (bất biến KT-20: `Running ⇒ còn budget`) |
/// | 10 | cap `Untyped` (nguồn retype) |
struct Rig {
    m: Machine,
    caller: ObjId,
    t_inactive: ObjId,
    t_noentry: ObjId,
    t_exited: ObjId,
    t_ready: ObjId,
    caller_slot: SlotRef,
    inactive_slot: SlotRef,
    noentry_slot: SlotRef,
    exited_slot: SlotRef,
    ready_slot: SlotRef,
    ro_slot: SlotRef,
    ep_slot: SlotRef,
}

/// `entry` hợp lệ của TCB đích — VA lệnh RISC-V (căn 2 B), khác `0`.
const ENTRY: u64 = 0x4000_0100;

fn rig() -> Rig {
    let mut m = Machine::<crate::store::StaticStore, crate::observer::TestObserver>::new();
    let u = m.add_untyped(0x8200_0000, 1 << 16);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 10, u_cap).expect("cap untyped");
    // Năm TCB: caller + bốn đích (mỗi đích một trạng thái khác nhau).
    let mut tcb = [0u32; 5];
    for (i, slot) in [1usize, 2, 3, 4, 5].iter().enumerate() {
        let cap = m
            .retype(SlotRef::new(c, 10), ObjType::Tcb, 0, 0)
            .expect("retype tcb");
        m.put_cap(c, *slot, cap).expect("cap tcb");
        tcb[i] = m.cap_obj(cap).expect("obj tcb");
    }
    for slot in [1usize, 2, 3, 4, 5] {
        m.tcb_configure(SlotRef::new(c, slot), Some(SlotRef::new(c, 0)))
            .expect("cspace = c");
    }
    // **KT-20** — `Running ⇒ còn budget`: cấp `SchedContext` (budget 500/period 5.000 như rig
    // `ktest-ipc`) cho **mọi** TCB có thể lên `Running` (caller + hai đích `Inactive`/`Ready`).
    let ctl_cap = m
        .retype(SlotRef::new(c, 10), ObjType::SchedControl, 0, 0)
        .expect("retype sched control");
    m.put_cap(c, 8, ctl_cap).expect("cap sched control");
    let sc_cap = m
        .retype(SlotRef::new(c, 10), ObjType::SchedContext, 0, 0)
        .expect("retype sched context");
    m.put_cap(c, 9, sc_cap).expect("cap sched context");
    m.sched_control_configure(SlotRef::new(c, 8), SlotRef::new(c, 9), 500, 5_000)
        .expect("SC budget");
    for slot in [1usize, 2, 3, 4, 5] {
        m.set_sched_params(SlotRef::new(c, slot), Some(SlotRef::new(c, 9)))
            .expect("sched params");
    }
    // `entry` của đích (2) — đặt **sau** `Configure` (Configure không xoá `entry`, nhưng đặt sau
    // cho rõ ràng: đây là giá trị `Tcb.Configure` tham số `entry?` đã ghi ở KT-311).
    m.tcb_data_mut(tcb[1]).expect("tcb").entry = ENTRY;
    // Đích (4) — **đã chạy rồi**: `Inactive` + `exit_code = Some(0)` (đúng dấu vết `Exit`).
    m.tcb_data_mut(tcb[3]).expect("tcb").exit_code = Some(0);
    // Đích (5) — `Ready` (đã `start_tcb`).
    m.start_tcb(tcb[4]).expect("start ready");
    // Caller — `Running{hart:0}` (đúng trạng thái một tác vụ đang gọi syscall).
    m.start_tcb(tcb[0]).expect("start caller");
    m.transition_tcb(tcb[0], NonReadyTarget::Running { hart: 0 })
        .expect("caller running");
    // Cap TCB **chỉ đọc** (ca quyền âm) + cap **không phải TCB** (Endpoint).
    m.mint(SlotRef::new(c, 2), SlotRef::new(c, 6), RIGHT_READ, None)
        .expect("mint tcb ro");
    let ep_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint");
    m.put_cap(c, 7, ep_cap).expect("cap endpoint");
    Rig {
        m,
        caller: tcb[0],
        t_inactive: tcb[1],
        t_noentry: tcb[2],
        t_exited: tcb[3],
        t_ready: tcb[4],
        caller_slot: SlotRef::new(c, 1),
        inactive_slot: SlotRef::new(c, 2),
        noentry_slot: SlotRef::new(c, 3),
        exited_slot: SlotRef::new(c, 4),
        ready_slot: SlotRef::new(c, 5),
        ro_slot: SlotRef::new(c, 6),
        ep_slot: SlotRef::new(c, 7),
    }
}

/// **Số method lấy từ spec, không chép tay** — `[object].Tcb` khai `Resume = 12` và hằng core
/// bằng đúng số đó; `is_whitelisted` phải nhận cặp `(Tcb, Resume)`.
#[test]
fn so_method_resume_sinh_tu_spec() {
    assert_eq!(invoke::TCB_RESUME, vifast_abi::TCB_RESUME);
    assert_eq!(invoke::TCB_RESUME, 12);
    assert!(vifast_abi::TCB_METHODS
        .iter()
        .any(|(name, id)| *name == "Resume" && u32::from(*id) == invoke::TCB_RESUME));
    assert!(invoke::is_whitelisted(ObjType::Tcb, invoke::TCB_RESUME));
    // `Suspend` (3) **đã** cài ở KT-319 ⇒ trong danh sách trắng; `SetPriority` (4) **chưa** cài
    // ⇒ vẫn ngoài danh sách (không mở rộng ngoài phạm vi).
    assert!(invoke::is_whitelisted(ObjType::Tcb, 3));
    assert!(!invoke::is_whitelisted(ObjType::Tcb, 4));
}

/// `Resume` trên TCB `Inactive` **chưa chạy**: chuyển `Running{hart:0}`, trả `a1 = entry`, và
/// **giao đúng `(tcb, entry)`** cho hook arch (arch mới ghi `sepc` — D2-83).
#[test]
fn resume_inactive_chuyen_running_va_giao_entry() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = ResumeEnv::default();
    let base = invoke::stats().resume;
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.inactive_slot, 0, [0; 4], &mut env),
        Ok((0, ENTRY))
    );
    assert_eq!(
        r.m.tcb_state(r.t_inactive).expect("state"),
        TcbState::Running { hart: 0 }
    );
    // Hook arch nhận **đúng** giá trị `TcbData.entry` — không phải hằng nào khác.
    assert_eq!(env.calls, std::vec![(r.t_inactive, ENTRY)]);
    assert_eq!(invoke::stats().resume, base + 1);
}

/// `Resume` trên TCB `Ready`: chuyển `Running` (không cần `start_tcb`), trả `entry`.
#[test]
fn resume_ready_chuyen_running() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    r.m.tcb_data_mut(r.t_ready).expect("tcb").entry = ENTRY;
    let mut env = ResumeEnv::default();
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.ready_slot, 0, [0; 4], &mut env),
        Ok((0, ENTRY))
    );
    assert_eq!(
        r.m.tcb_state(r.t_ready).expect("state"),
        TcbState::Running { hart: 0 }
    );
}

/// `entry == 0` ⇒ `InvalidArgument` **5**, **không** đổi trạng thái (fail-closed: không đoán
/// điểm vào), **không** gọi hook arch.
#[test]
fn resume_entry_zero_invalid_argument() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = ResumeEnv::default();
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.noentry_slot, 0, [0; 4], &mut env),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        r.m.tcb_state(r.t_noentry).expect("state"),
        TcbState::Inactive
    );
    assert!(env.calls.is_empty(), "hook arch không được gọi khi lỗi");
}

/// `Resume` trên tác vụ **`Running`** ⇒ `IllegalOperation` **7**; và **kiểm trạng thái đứng
/// trước** kiểm `entry`: caller `Running` + `entry = 0` vẫn trả **7** (không phải 5).
#[test]
fn resume_running_illegal_va_thu_tu_kiem() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = ResumeEnv::default();
    // Caller đang `Running` và `entry` của nó = 0 (chưa `Configure` `entry?`) ⇒ **7**, không 5.
    assert_eq!(r.m.tcb_entry(r.caller).expect("entry"), 0);
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.caller_slot, 0, [0; 4], &mut env),
        Err(KError::IllegalOperation.id() as u64)
    );
    assert_eq!(
        r.m.tcb_state(r.caller).expect("state"),
        TcbState::Running { hart: 0 }
    );
    assert!(env.calls.is_empty());
}

/// `Inactive` **đã chạy** (`exit_code = Some`) ⇒ `IllegalOperation` **7**: không tự chạy lại tác
/// vụ đã kết thúc (muốn chạy lại phải `Configure` trước — D2-25 xoá `exit_code`).
#[test]
fn resume_inactive_da_chay_illegal() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    r.m.tcb_data_mut(r.t_exited).expect("tcb").entry = ENTRY;
    let mut env = ResumeEnv::default();
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.exited_slot, 0, [0; 4], &mut env),
        Err(KError::IllegalOperation.id() as u64)
    );
    assert_eq!(
        r.m.tcb_state(r.t_exited).expect("state"),
        TcbState::Inactive
    );
    assert!(env.calls.is_empty());
}

/// `Resume` thành công **xoá `exit_code`** (D2-25) — `Ready` + `exit_code = Some(9)` ⇒ `None`.
#[test]
fn resume_xoa_exit_code() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    r.m.tcb_data_mut(r.t_ready).expect("tcb").entry = ENTRY;
    r.m.tcb_data_mut(r.t_ready).expect("tcb").exit_code = Some(9);
    let mut env = ResumeEnv::default();
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.ready_slot, 0, [0; 4], &mut env),
        Ok((0, ENTRY))
    );
    assert_eq!(r.m.tcb_data(r.t_ready).expect("tcb").exit_code, None);
}

/// **Arity chính xác 0** (D2-104): `length` khác 0 ⇒ `InvalidArgument` **5**, có đếm `denied`.
#[test]
fn resume_arity_chinh_xac_khong() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = ResumeEnv::default();
    let before = invoke::stats().denied;
    for len in [1u8, 2, 4] {
        assert_eq!(
            invoke_on(&mut r.m, r.caller, r.inactive_slot, len, [0; 4], &mut env),
            Err(KError::InvalidArgument.id() as u64),
            "length = {len} phải bị từ chối"
        );
    }
    assert_eq!(invoke::stats().denied, before + 3);
    assert_eq!(
        r.m.tcb_state(r.t_inactive).expect("state"),
        TcbState::Inactive
    );
}

/// Cap TCB **thiếu `RIGHT_WRITE`** ⇒ `InsufficientRights` **3** (cùng luật quyền như
/// `Tcb.Configure`/`Tcb.SetTls`, D2-99(e)).
#[test]
fn resume_thieu_quyen_ghi() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = ResumeEnv::default();
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.ro_slot, 0, [0; 4], &mut env),
        Err(KError::InsufficientRights.id() as u64)
    );
}

/// Cap **không phải TCB** ⇒ `NotSupported` **18** (D2-53 Q11 — danh sách trắng hẹp, không trả
/// `Ok` giả).
#[test]
fn resume_cap_khong_phai_tcb() {
    let _guard = RESUME_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = ResumeEnv::default();
    assert_eq!(
        invoke_on(&mut r.m, r.caller, r.ep_slot, 0, [0; 4], &mut env),
        Err(KError::NotSupported.id() as u64)
    );
}
