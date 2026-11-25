// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-319** — test host cho `Tcb.Suspend` (method **3** của `[object].Tcb` trong
//! `spec/abi.toml`) qua **invocation** (syscall 0 trên cap TCB).
//!
//! # Vì sao test host
//!
//! Ngữ nghĩa **core** ở đây là: kiểm trạng thái đích (`Running`/`Ready`), chuyển `Inactive`
//! qua **một** cửa `transition_tcb` (bảng 11×11 D2-49(d) vẫn là nguồn sự thật), **giữ**
//! `TcbData.entry` (để `Tcb.Resume` sau đó chạy lại được) và **giữ** `exit_code` (D2-25).
//! Phần **chạy lại thật** ở U-mode được đo bằng QEMU ở
//! `kernel/vifast-kbin-riscv64/src/ktest_ipc.rs` (TAP `ipc: tcb suspend`).
//!
//! Ranh giới đã kiểm: (a) số method **lấy từ spec** (`vifast_abi::TCB_SUSPEND`, không chép tay);
//! (b) arity **chính xác 1** (D2-104) — `mode` = MR0; (c) cap TCB phải có `RIGHT_WRITE` ⇒ thiếu
//! là `InsufficientRights` 3; (d) `Running` ⇒ `Inactive`; (e) `Ready` ⇒ `Inactive`;
//! (f) `BlockedOnSend` ⇒ `IllegalOperation` 17 (đường invocation **không** nhận `BlockedOn*` —
//! khác model API KT-65); (g) `Inactive`/`Dead` ⇒ 17; (h) **KT-329** `mode = Defer` ⇒ `(0, 0)`,
//! trạng thái **không** đổi (dừng ở điểm an toàn kế tiếp); (i) mode lạ ⇒ 5; (j) `entry` **giữ
//! nguyên** sau `Suspend` và
//! `Resume` sau đó chạy lại được (cặp `Suspend`/`Resume`); (k) cap không phải TCB ⇒
//! `NotSupported` 18 (D2-53 Q11, không trả `Ok` giả).
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke;
use crate::machine::Machine;
use crate::spec::RIGHT_READ;
use crate::state_write::NonReadyTarget;
use crate::syscall::SyscallEnv;
use crate::tcb::{SuspendFlag, TcbState};

/// Hook arch tối thiểu — `Suspend` **không** cần hook nào (không đụng CSR/bộ nhớ user, D2-83);
/// `tcb_resume` chỉ dùng cho ca "Suspend rồi Resume chạy lại".
#[derive(Default)]
struct SuspendEnv {
    resumed: std::vec::Vec<(ObjId, u64)>,
}

impl SyscallEnv for SuspendEnv {
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
        self.resumed.push((tcb, entry));
        true
    }
}

/// Khoá **nối tiếp** cho các ca đọc **bộ đếm toàn cục** `invoke::stats()` (cùng bài học D2-108B
/// với `tls_tests`/`configure_tests`/`resume_tests`): `suspend` là bộ đếm dùng chung giữa các
/// test song song.
use crate::test_lock::STATS_LOCK as SUSPEND_COUNTER_LOCK;

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
    env: &mut SuspendEnv,
) -> Result<(u64, u64), u64> {
    invoke::dispatch(m, tcb, call_slot, info(invoke::TCB_SUSPEND, len), &mrs, env)
}

/// Cảnh một tiến trình: CNode 16 slot.
///
/// | ô | nội dung |
/// |---|---|
/// | 0 | cap `CNode` tự (CSpace) |
/// | 1 | TCB **caller** — `Running{hart:0}` (đang gọi invocation) |
/// | 2 | TCB đích **`Running{hart:0}`** (đường xanh; đẩy caller về `Ready`) |
/// | 3 | TCB đích **`Ready`** |
/// | 4 | TCB đích **`BlockedOnSend`** (ca âm: đường invocation **không** nhận `BlockedOn*`) |
/// | 5 | TCB đích **`Inactive`** (ca âm) |
/// | 6 | TCB đích **`Dead`** (ca âm) |
/// | 7 | TCB **chỉ đọc** (`RIGHT_READ`) — ca quyền âm |
/// | 8 | cap `Endpoint` — **không phải** TCB (ca âm danh sách trắng) |
/// | 9 | `SchedControl` (cấu hình SC) |
/// | 10 | `SchedContext` cho các TCB sẽ `Running` (bất biến KT-20: `Running ⇒ còn budget`) |
/// | 11 | cap `Untyped` (nguồn retype) |
struct Rig {
    m: Machine,
    caller: ObjId,
    t_running: ObjId,
    t_ready: ObjId,
    t_blocked: ObjId,
    t_inactive: ObjId,
    t_dead: ObjId,
    running_slot: SlotRef,
    ready_slot: SlotRef,
    blocked_slot: SlotRef,
    inactive_slot: SlotRef,
    dead_slot: SlotRef,
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
    m.put_cap(c, 11, u_cap).expect("cap untyped");
    // Sáu TCB: caller + năm đích (mỗi đích một trạng thái khác nhau).
    let mut tcb = [0u32; 6];
    for (i, slot) in [1usize, 2, 3, 4, 5, 6].iter().enumerate() {
        let cap = m
            .retype(SlotRef::new(c, 11), ObjType::Tcb, 0, 0)
            .expect("retype tcb");
        m.put_cap(c, *slot, cap).expect("cap tcb");
        tcb[i] = m.cap_obj(cap).expect("obj tcb");
    }
    for slot in [1usize, 2, 3, 4, 5, 6] {
        m.tcb_configure(SlotRef::new(c, slot), Some(SlotRef::new(c, 0)))
            .expect("cspace = c");
    }
    // **KT-20** — `Running ⇒ còn budget`: cấp `SchedContext` (budget 500/period 5.000 như rig
    // `ktest-ipc`) cho **mọi** TCB có thể lên `Running` (caller + hai đích `Running`/`Ready`).
    let ctl_cap = m
        .retype(SlotRef::new(c, 11), ObjType::SchedControl, 0, 0)
        .expect("retype sched control");
    m.put_cap(c, 9, ctl_cap).expect("cap sched control");
    let sc_cap = m
        .retype(SlotRef::new(c, 11), ObjType::SchedContext, 0, 0)
        .expect("retype sched context");
    m.put_cap(c, 10, sc_cap).expect("cap sched context");
    m.sched_control_configure(SlotRef::new(c, 9), SlotRef::new(c, 10), 500, 5_000)
        .expect("SC budget");
    for slot in [1usize, 2, 3, 4, 5, 6] {
        m.set_sched_params(SlotRef::new(c, slot), Some(SlotRef::new(c, 10)))
            .expect("sched params");
    }
    // `entry` của đích (2) — giá trị `Tcb.Configure` tham số `entry?` đã ghi ở KT-311.
    m.tcb_data_mut(tcb[1]).expect("tcb").entry = ENTRY;
    // Đích (4) — `BlockedOnSend` trên Endpoint (ca âm: đường invocation **không** nhận).
    let ep_cap = m
        .retype(SlotRef::new(c, 11), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint");
    m.put_cap(c, 8, ep_cap).expect("cap endpoint");
    let ep = m.cap_obj(ep_cap).expect("obj endpoint");
    m.transition_tcb(
        tcb[3],
        NonReadyTarget::BlockedOnSend {
            ep,
            call: false,
            fault: None,
        },
    )
    .expect("blocked on send");
    // Đích (6) — `Dead` (trạng thái cuối).
    m.transition_tcb(tcb[5], NonReadyTarget::Dead)
        .expect("dead");
    // Đích (3) — `Ready` (đã `start_tcb`).
    m.start_tcb(tcb[2]).expect("start ready");
    // Caller — `Running{hart:0}` (đúng trạng thái một tác vụ đang gọi syscall).
    m.start_tcb(tcb[0]).expect("start caller");
    m.transition_tcb(tcb[0], NonReadyTarget::Running { hart: 0 })
        .expect("caller running");
    // Đích (2) — `Running{hart:0}`: đẩy caller về `Ready` (bất biến ≤1 `Running`/hart).
    m.start_tcb(tcb[1]).expect("start running");
    m.transition_tcb(tcb[1], NonReadyTarget::Running { hart: 0 })
        .expect("target running");
    // Cap TCB **chỉ đọc** (ca quyền âm).
    m.mint(SlotRef::new(c, 3), SlotRef::new(c, 7), RIGHT_READ, None)
        .expect("mint tcb ro");
    Rig {
        m,
        caller: tcb[0],
        t_running: tcb[1],
        t_ready: tcb[2],
        t_blocked: tcb[3],
        t_inactive: tcb[4],
        t_dead: tcb[5],
        running_slot: SlotRef::new(c, 2),
        ready_slot: SlotRef::new(c, 3),
        blocked_slot: SlotRef::new(c, 4),
        inactive_slot: SlotRef::new(c, 5),
        dead_slot: SlotRef::new(c, 6),
        ro_slot: SlotRef::new(c, 7),
        ep_slot: SlotRef::new(c, 8),
    }
}

/// **Số method lấy từ spec, không chép tay** — `[object].Tcb` khai `Suspend = 3` và hằng core
/// bằng đúng số đó; `is_whitelisted` phải nhận cặp `(Tcb, Suspend)`.
#[test]
fn so_method_suspend_sinh_tu_spec() {
    assert_eq!(invoke::TCB_SUSPEND, vifast_abi::TCB_SUSPEND);
    assert_eq!(invoke::TCB_SUSPEND, 3);
    assert!(vifast_abi::TCB_METHODS
        .iter()
        .any(|(name, id)| *name == "Suspend" && u32::from(*id) == invoke::TCB_SUSPEND));
    assert!(invoke::is_whitelisted(ObjType::Tcb, invoke::TCB_SUSPEND));
    // `SetPriority` (4) **chưa** cài ⇒ vẫn ngoài danh sách trắng (không mở rộng ngoài phạm vi).
    assert!(!invoke::is_whitelisted(ObjType::Tcb, 4));
}

/// `Suspend(Abort)` trên TCB `Running` ⇒ `Inactive`, trả `(0, 0)`, **giữ** `entry` + `exit_code`
/// (để `Resume` chạy lại được — mốc thật của KT-319).
#[test]
fn suspend_running_chuyen_inactive_giu_entry() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    let base = invoke::stats().suspend;
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.running_slot,
            1,
            [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
            &mut env
        ),
        Ok((0, 0))
    );
    assert_eq!(
        r.m.tcb_state(r.t_running).expect("state"),
        TcbState::Inactive
    );
    assert_eq!(r.m.tcb_entry(r.t_running).expect("entry"), ENTRY);
    assert_eq!(r.m.tcb_data(r.t_running).expect("tcb").exit_code, None);
    assert_eq!(invoke::stats().suspend, base + 1);
}

/// `Suspend(Abort)` trên TCB `Ready` ⇒ `Inactive` (bảng 11×11: `Ready → Inactive` ALLOWED).
#[test]
fn suspend_ready_chuyen_inactive() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.ready_slot,
            1,
            [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
            &mut env
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_state(r.t_ready).expect("state"), TcbState::Inactive);
}

/// `Suspend` trên tác vụ **`BlockedOnSend`** ⇒ `IllegalOperation` **17**, trạng thái **không**
/// đổi (đường invocation **không** nhận `BlockedOn*` — khác model API KT-65).
#[test]
fn suspend_blocked_illegal() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.blocked_slot,
            1,
            [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
            &mut env
        ),
        Err(KError::IllegalOperation.id() as u64)
    );
    assert!(matches!(
        r.m.tcb_state(r.t_blocked).expect("state"),
        TcbState::BlockedOnSend { .. }
    ));
}

/// `Suspend` trên `Inactive` và `Dead` ⇒ `IllegalOperation` **17** (không đổi trạng thái).
#[test]
fn suspend_inactive_va_dead_illegal() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    for slot in [r.inactive_slot, r.dead_slot] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.caller,
                slot,
                1,
                [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
                &mut env
            ),
            Err(KError::IllegalOperation.id() as u64)
        );
    }
    assert_eq!(
        r.m.tcb_state(r.t_inactive).expect("state"),
        TcbState::Inactive
    );
    assert_eq!(r.m.tcb_state(r.t_dead).expect("state"), TcbState::Dead);
}

/// **KT-329** — `mode = Defer` **không** dừng ngay: trả `(0, 0)`, trạng thái **giữ**
/// `Running{hart:0}`, `entry` giữ nguyên; mode lạ (2, `u64::MAX`) **vẫn** `InvalidArgument` 5.
///
/// Đây là mốc của `Suspend(Defer)` (KT-329): việc dừng xảy ra ở **điểm an toàn** kế tiếp
/// (cuối `syscall::dispatch`), không phải trong chính lệnh `Suspend`.
#[test]
fn suspend_defer_khong_doi_trang_thai_ngay() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.running_slot,
            1,
            [invoke::SUSPEND_MODE_DEFER, 0, 0, 0],
            &mut env
        ),
        Ok((0, 0))
    );
    assert_eq!(
        r.m.tcb_state(r.t_running).expect("state"),
        TcbState::Running { hart: 0 },
        "Defer KHÔNG được đổi trạng thái ngay"
    );
    assert_eq!(r.m.tcb_entry(r.t_running).expect("entry"), ENTRY);
    // Mode lạ vẫn bị từ chối — chỉ `Abort` (0) và `Defer` (1) hợp lệ.
    for mode in [2, u64::MAX] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.caller,
                r.running_slot,
                1,
                [mode, 0, 0, 0],
                &mut env
            ),
            Err(KError::InvalidArgument.id() as u64),
            "mode = {mode} phải bị từ chối"
        );
    }
}

/// **KT-329** — `Suspend(Defer)` trên `Ready` **không** đổi trạng thái; **điểm an toàn** kế
/// tiếp (cuối `syscall::dispatch`) mới chuyển `Inactive`, **giữ** `entry`, **xoá** cờ; gọi lại
/// điểm an toàn là **no-op** (cờ đã tiêu thụ).
#[test]
fn suspend_defer_dung_o_diem_an_toan() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    r.m.tcb_data_mut(r.t_ready).expect("tcb").entry = ENTRY;
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.ready_slot,
            1,
            [invoke::SUSPEND_MODE_DEFER, 0, 0, 0],
            &mut env
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_state(r.t_ready).expect("state"), TcbState::Ready);
    assert_eq!(
        r.m.tcb_data(r.t_ready).expect("tcb").suspend,
        SuspendFlag::Defer
    );
    // Điểm an toàn — đúng một lần.
    assert!(r.m.tcb_defer_safe_point(r.t_ready));
    assert_eq!(r.m.tcb_state(r.t_ready).expect("state"), TcbState::Inactive);
    assert_eq!(r.m.tcb_entry(r.t_ready).expect("entry"), ENTRY);
    assert_eq!(
        r.m.tcb_data(r.t_ready).expect("tcb").suspend,
        SuspendFlag::None
    );
    assert!(!r.m.tcb_defer_safe_point(r.t_ready), "cờ đã tiêu thụ");
}

/// **KT-329** — `Tcb.Resume` (method 12) là **đường khôi phục duy nhất**: xoá cờ `Defer` còn
/// treo trước khi đưa TCB lên `Running` và trả `entry`.
#[test]
fn resume_xoa_co_defer() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    r.m.tcb_data_mut(r.t_ready).expect("tcb").entry = ENTRY;
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.ready_slot,
            1,
            [invoke::SUSPEND_MODE_DEFER, 0, 0, 0],
            &mut env
        ),
        Ok((0, 0))
    );
    let resumed = invoke::dispatch(
        &mut r.m,
        r.caller,
        r.ready_slot,
        info(invoke::TCB_RESUME, 0),
        &[0; 4],
        &mut env,
    );
    assert_eq!(resumed, Ok((0, ENTRY)));
    assert_eq!(
        r.m.tcb_state(r.t_ready).expect("state"),
        TcbState::Running { hart: 0 }
    );
    assert_eq!(
        r.m.tcb_data(r.t_ready).expect("tcb").suspend,
        SuspendFlag::None,
        "Resume phải xoá cờ Defer"
    );
}

/// **KT-329** — `Defer` vẫn giữ **kiểm trạng thái** của KT-319: `BlockedOnSend`/`Inactive`/`Dead`
/// ⇒ `IllegalOperation` 17, không đổi trạng thái, không đặt cờ.
#[test]
fn suspend_defer_giu_kiem_trang_thai() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    for slot in [r.blocked_slot, r.inactive_slot, r.dead_slot] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.caller,
                slot,
                1,
                [invoke::SUSPEND_MODE_DEFER, 0, 0, 0],
                &mut env
            ),
            Err(KError::IllegalOperation.id() as u64)
        );
    }
}

/// **Arity chính xác 1** (D2-104): `length` khác 1 — thiếu (0) hoặc thừa (2..4) — đều
/// `InvalidArgument` **5**, có đếm `denied`.
#[test]
fn suspend_arity_chinh_xac_mot() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    let before = invoke::stats().denied;
    for len in [0u8, 2, 4] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.caller,
                r.running_slot,
                len,
                [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
                &mut env
            ),
            Err(KError::InvalidArgument.id() as u64),
            "length = {len} phải bị từ chối"
        );
    }
    assert_eq!(invoke::stats().denied, before + 3);
    assert_eq!(
        r.m.tcb_state(r.t_running).expect("state"),
        TcbState::Running { hart: 0 }
    );
}

/// Cap TCB **thiếu `RIGHT_WRITE`** ⇒ `InsufficientRights` **3** (cùng luật quyền như
/// `Tcb.Configure`/`Tcb.SetTls`/`Tcb.Resume`, D2-99(e)).
#[test]
fn suspend_thieu_quyen_ghi() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.ro_slot,
            1,
            [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
            &mut env
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
}

/// Cap **không phải TCB** ⇒ `NotSupported` **18** (D2-53 Q11 — danh sách trắng hẹp, không trả
/// `Ok` giả).
#[test]
fn suspend_cap_khong_phai_tcb() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.ep_slot,
            1,
            [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
            &mut env
        ),
        Err(KError::NotSupported.id() as u64)
    );
}

/// **Cặp `Suspend`/`Resume`** (mốc thật của KT-319): `Suspend` một tác vụ `Ready` ⇒ `Inactive`,
/// rồi `Resume` ⇒ `Running{hart:0}` với **đúng** `entry` đã giữ — tác vụ chạy lại được.
#[test]
fn suspend_roi_resume_chay_lai_duoc() {
    let _guard = SUSPEND_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mut env = SuspendEnv::default();
    r.m.tcb_data_mut(r.t_ready).expect("tcb").entry = ENTRY;
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.caller,
            r.ready_slot,
            1,
            [invoke::SUSPEND_MODE_ABORT, 0, 0, 0],
            &mut env
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_state(r.t_ready).expect("state"), TcbState::Inactive);
    // `Resume` (method 12, KT-314) trên **cùng** cap: `Inactive` chưa chạy ⇒ `Running` + entry.
    let resumed = invoke::dispatch(
        &mut r.m,
        r.caller,
        r.ready_slot,
        info(invoke::TCB_RESUME, 0),
        &[0; 4],
        &mut env,
    );
    assert_eq!(resumed, Ok((0, ENTRY)));
    assert_eq!(
        r.m.tcb_state(r.t_ready).expect("state"),
        TcbState::Running { hart: 0 }
    );
    assert_eq!(env.resumed, std::vec![(r.t_ready, ENTRY)]);
}
