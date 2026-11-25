// SPDX-License-Identifier: MIT OR Apache-2.0
//! U-mode + syscall (KT-53 → **KT-62**) — phần **ngữ nghĩa** dùng chung, `no_std`, **không
//! `unsafe`**, chạy được trên host.
//!
//! KT-62 (K3.5 tầng 1): hàm [`dispatch`] **không** còn bảng 3 syscall của KT-53 — nó gọi
//! [`vifast_core::syscall::dispatch`] (16 syscall `0..15` → `Machine` thật), chỉ giữ phần ghi sổ
//! `UserTask`/ánh xạ kết quả cho trap handler.
//!
//! # Phạm vi
//!
//! KT-53 chạy **một** tác vụ user (`init`) trong ảnh test: nạp ELF, `sret` xuống U-mode, bắt
//! `ecall` rồi dispatch 3 syscall, `Exit` đưa tác vụ sang trạng thái `Exited` (không quay lại
//! U-mode), fault U-mode ⇒ log + halt. **Không** CNode/TCB/IPC/scheduler (K3b KT-58..61), **không**
//! procsrv/shell (K3.5), **không** `.scn` (KT-55), **không** `FaultKind` đầy đủ (KT-56) — trạng
//! thái tác vụ ở đây chỉ là "TCB-like state" tối thiểu, không phải TCB.
//!
//! # ABI (chỉ **đọc** `spec/abi.toml`, không sửa)
//!
//! | Thứ | Nguồn |
//! |---|---|
//! | `a7` = số hiệu; `a0`/`a1` = tham số; trả `a0` = error (0 = OK), `a1` = value | `[calling.riscv64]` |
//! | `DebugPutStr` = 12 (`user ptr, len` → `error, value`) | `[syscall]` |
//! | `DebugHalt` = 13 (`code`, không trả) | `[syscall]` (chỉ test) |
//! | `Exit` = 14 (`code`, không trả) | `[syscall]` + **QD §1 Q-A** |
//! | 19 mã lỗi 1..19 | `[error]` |
//!
//! Mọi số hiệu `0..=15` đi vào core thật; **ngoài dải** (kể cả EID SBI) ⇒ `RangeError` (19,
//! **D2-79** — số syscall là chỉ số vào bảng dispatch) — **không panic**. U-mode **không thể**
//! gọi SBI trực tiếp: `ecall` từ U-mode luôn vào `scause=8` rồi tới dispatcher này, và EID SBI
//! nằm ngoài dải syscall nên chỉ nhận lại mã lỗi ABI.
//!
//! # Vì sao tách khỏi arch
//!
//! D2-72 Q12: Miri/`cargo test` không áp kernel `no_std`/asm/MMIO, nên bảng số hiệu, mã lỗi, kiểm
//! dải user và state machine phải nằm ở crate **an toàn** (`#![forbid(unsafe_code)]`) để test
//! host; arch chỉ còn trapframe/CSR/asm/đọc bộ nhớ user (`vifast-plat-riscv64::user`).

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

// KT-62: số hiệu + trần `DebugPutStr` **một nguồn** ở `vifast-core::syscall` (sinh từ spec);
// façade tái xuất để caller cũ (`user::SYS_EXIT`, …) không phải đổi.
pub use vifast_core::syscall::{
    MAX_DEBUG_PUT, SYSCALL_MAX, SYS_DEBUG_GET_INFO, SYS_DEBUG_HALT, SYS_DEBUG_PUT_STR, SYS_EXIT,
};

/// `InvalidArgument` — mã lỗi ABI 5 (`[error]`).
pub const ERR_INVALID_ARGUMENT: u64 = 5;
/// `NotMapped` — mã lỗi ABI 11 (dùng khi con trỏ user chưa map).
pub const ERR_NOT_MAPPED: u64 = 11;
/// `NotSupported` — mã lỗi ABI 18 (syscall trong dải nhưng chưa cài).
pub const ERR_NOT_SUPPORTED: u64 = 18;
/// `RangeError` — mã lỗi ABI 19 (số hiệu syscall **ngoài dải** `0..=15`, D2-79).
pub const ERR_RANGE_ERROR: u64 = 19;

/// Tên 19 mã lỗi + 0 = OK, đúng `spec/abi.toml` `[error]` (không sinh tên thứ hai).
pub const fn error_name(code: u64) -> &'static str {
    match code {
        0 => "OK",
        1 => "InvalidCapability",
        2 => "WrongObjectType",
        3 => "InsufficientRights",
        4 => "WxViolation",
        5 => "InvalidArgument",
        6 => "Alignment",
        7 => "SlotOccupied",
        8 => "SlotEmpty",
        9 => "NotEnoughMemory",
        10 => "AlreadyMapped",
        11 => "NotMapped",
        12 => "MissingPageTable",
        13 => "HasChildren",
        14 => "Busy",
        15 => "Cancelled",
        16 => "Timeout",
        17 => "IllegalOperation",
        18 => "NotSupported",
        19 => "RangeError",
        _ => "Unknown",
    }
}

/// Trạng thái "TCB-like" tối thiểu của tác vụ user (không phải TCB của K3b).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskState {
    /// Chưa vào U-mode.
    #[default]
    Created,
    /// Đang chạy/hạ `sret` xuống U-mode.
    Running,
    /// `Exit(code)` — không bao giờ quay lại U-mode.
    Exited,
    /// Fault U-mode — chính sách là log + halt.
    Faulted,
}

impl TaskState {
    /// Mã ổn định trong `UserSnapshot` (không đổi giữa các boot).
    pub const fn code(self) -> u8 {
        match self {
            Self::Created => 0,
            Self::Running => 1,
            Self::Exited => 2,
            Self::Faulted => 3,
        }
    }

    /// Nghịch đảo [`Self::code`] — mã lạ ⇒ `Created` (không panic).
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Running,
            2 => Self::Exited,
            3 => Self::Faulted,
            _ => Self::Created,
        }
    }

    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Running => "Running",
            Self::Exited => "Exited",
            Self::Faulted => "Faulted",
        }
    }
}

/// Ảnh chụp trạng thái tác vụ — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserSnapshot {
    /// Trạng thái hiện tại.
    pub state: TaskState,
    /// Số lần hạ `sret` xuống U-mode.
    pub entries: u64,
    /// `sstatus.SPP` ghi vào trapframe lúc vào U-mode (phải 0 = U-mode).
    pub enter_spp: u64,
    /// Số `ecall` từ U-mode đã vào dispatcher.
    pub ecalls: u64,
    /// `sstatus.SPP` của trap `ecall` gần nhất (phải 0 = từ U-mode).
    pub ecall_spp: u64,
    /// Tổng số syscall đã dispatch.
    pub syscalls: u64,
    /// Số hiệu syscall gần nhất.
    pub last_nr: u64,
    /// `a0` (error) trả cho lần gần nhất.
    pub last_error: u64,
    /// `a1` (value) trả cho lần gần nhất.
    pub last_value: u64,
    /// Số syscall **ngoài** dải `0..=15` (kể cả EID SBI).
    pub bad_calls: u64,
    /// Số hiệu syscall lạ gần nhất.
    pub last_bad_nr: u64,
    /// Mã lỗi ABI trả cho syscall lạ gần nhất.
    pub last_bad_error: u64,
    /// Số lần `DebugPutStr` in thành công.
    pub put_calls: u64,
    /// Tổng số byte user đã in qua `DebugPutStr`.
    pub put_bytes: u64,
    /// Số lần `Exit`.
    pub exits: u64,
    /// Mã thoát (`Exit(code)`).
    pub exit_code: u64,
    /// Số fault U-mode.
    pub faults: u64,
    /// `scause` của fault gần nhất.
    pub fault_scause: u64,
    /// `stval` của fault gần nhất (địa chỉ U-mode chạm).
    pub fault_stval: u64,
    /// `sepc` của fault gần nhất.
    pub fault_sepc: u64,
    /// `sstatus` **ghi vào CSR lúc hạ `sret`** xuống U-mode (bằng chứng SPP/SIE/SPIE/SUM = 0).
    pub enter_sstatus: u64,
    /// `sstatus` phần cứng để lại khi trap `ecall` đầu tiên từ U-mode (SPP=0, SIE=0, SUM=0).
    pub trap_sstatus: u64,
}

/// Trạng thái tác vụ user — atomics để trap handler (arch) ghi và TAP đọc không cần `static mut`.
#[derive(Debug)]
pub struct UserTask {
    state: AtomicU8,
    entries: AtomicU64,
    enter_spp: AtomicU64,
    ecalls: AtomicU64,
    ecall_spp: AtomicU64,
    syscalls: AtomicU64,
    last_nr: AtomicU64,
    last_error: AtomicU64,
    last_value: AtomicU64,
    bad_calls: AtomicU64,
    last_bad_nr: AtomicU64,
    last_bad_error: AtomicU64,
    put_calls: AtomicU64,
    put_bytes: AtomicU64,
    exits: AtomicU64,
    exit_code: AtomicU64,
    faults: AtomicU64,
    fault_scause: AtomicU64,
    fault_stval: AtomicU64,
    fault_sepc: AtomicU64,
    enter_sstatus: AtomicU64,
    trap_sstatus: AtomicU64,
}

impl UserTask {
    /// Trạng thái rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(TaskState::Created.code()),
            entries: AtomicU64::new(0),
            enter_spp: AtomicU64::new(0),
            ecalls: AtomicU64::new(0),
            ecall_spp: AtomicU64::new(0),
            syscalls: AtomicU64::new(0),
            last_nr: AtomicU64::new(0),
            last_error: AtomicU64::new(0),
            last_value: AtomicU64::new(0),
            bad_calls: AtomicU64::new(0),
            last_bad_nr: AtomicU64::new(0),
            last_bad_error: AtomicU64::new(0),
            put_calls: AtomicU64::new(0),
            put_bytes: AtomicU64::new(0),
            exits: AtomicU64::new(0),
            exit_code: AtomicU64::new(0),
            faults: AtomicU64::new(0),
            fault_scause: AtomicU64::new(0),
            fault_stval: AtomicU64::new(0),
            fault_sepc: AtomicU64::new(0),
            enter_sstatus: AtomicU64::new(0),
            trap_sstatus: AtomicU64::new(0),
        }
    }

    /// Ghi nhận hạ `sret` xuống U-mode; `spp` phải `false` (U-mode). Trạng thái ⇒ `Running`.
    pub fn begin_run(&self, spp: bool) {
        self.state
            .store(TaskState::Running.code(), Ordering::Relaxed);
        self.entries.fetch_add(1, Ordering::Relaxed);
        self.enter_spp.store(spp as u64, Ordering::Relaxed);
    }

    /// Ghi nhận một trap `ecall` từ U-mode (kèm `sstatus.SPP` — phải 0).
    pub fn record_ecall(&self, spp: bool) {
        self.ecalls.fetch_add(1, Ordering::Relaxed);
        self.ecall_spp.store(spp as u64, Ordering::Relaxed);
    }

    /// Ghi nhận syscall đã vào dispatcher.
    pub fn record_syscall(&self, nr: u64) {
        self.syscalls.fetch_add(1, Ordering::Relaxed);
        self.last_nr.store(nr, Ordering::Relaxed);
    }

    /// Ghi nhận `(a0, a1)` trả về cho user.
    pub fn record_return(&self, error: u64, value: u64) {
        self.last_error.store(error, Ordering::Relaxed);
        self.last_value.store(value, Ordering::Relaxed);
    }

    /// Ghi nhận một syscall **ngoài dải** đã bị từ chối bằng mã lỗi ABI.
    pub fn record_bad(&self, nr: u64, error: u64) {
        self.bad_calls.fetch_add(1, Ordering::Relaxed);
        self.last_bad_nr.store(nr, Ordering::Relaxed);
        self.last_bad_error.store(error, Ordering::Relaxed);
    }

    /// Ghi nhận `DebugPutStr` in thành công `bytes` byte user.
    pub fn record_put(&self, bytes: u64) {
        self.put_calls.fetch_add(1, Ordering::Relaxed);
        self.put_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// `Exit(code)` — trạng thái ⇒ `Exited`; **không** quay lại U-mode.
    pub fn record_exit(&self, code: u64) {
        self.state
            .store(TaskState::Exited.code(), Ordering::Relaxed);
        self.exits.fetch_add(1, Ordering::Relaxed);
        self.exit_code.store(code, Ordering::Relaxed);
    }

    /// Ghi `sstatus` **đã đặt vào CSR** ngay trước `sret` xuống U-mode (bằng chứng SPP/SIE/SUM).
    pub fn record_enter_sstatus(&self, bits: u64) {
        self.enter_sstatus.store(bits, Ordering::Relaxed);
    }

    /// Ghi `sstatus` phần cứng tại trap `ecall` từ U-mode (SPP=0, SIE=0, SUM=0 phải giữ).
    pub fn record_trap_sstatus(&self, bits: u64) {
        self.trap_sstatus.store(bits, Ordering::Relaxed);
    }

    /// Fault U-mode — trạng thái ⇒ `Faulted` (chính sách arch: log + halt).
    pub fn record_fault(&self, scause: u64, stval: u64, sepc: u64) {
        self.state
            .store(TaskState::Faulted.code(), Ordering::Relaxed);
        self.faults.fetch_add(1, Ordering::Relaxed);
        self.fault_scause.store(scause, Ordering::Relaxed);
        self.fault_stval.store(stval, Ordering::Relaxed);
        self.fault_sepc.store(sepc, Ordering::Relaxed);
    }

    /// Đọc nhất quán từng trường (một hart ở KT-53; K3b mới cần đồng bộ chặt hơn).
    pub fn snapshot(&self) -> UserSnapshot {
        UserSnapshot {
            state: TaskState::from_code(self.state.load(Ordering::Relaxed)),
            entries: self.entries.load(Ordering::Relaxed),
            enter_spp: self.enter_spp.load(Ordering::Relaxed),
            ecalls: self.ecalls.load(Ordering::Relaxed),
            ecall_spp: self.ecall_spp.load(Ordering::Relaxed),
            syscalls: self.syscalls.load(Ordering::Relaxed),
            last_nr: self.last_nr.load(Ordering::Relaxed),
            last_error: self.last_error.load(Ordering::Relaxed),
            last_value: self.last_value.load(Ordering::Relaxed),
            bad_calls: self.bad_calls.load(Ordering::Relaxed),
            last_bad_nr: self.last_bad_nr.load(Ordering::Relaxed),
            last_bad_error: self.last_bad_error.load(Ordering::Relaxed),
            put_calls: self.put_calls.load(Ordering::Relaxed),
            put_bytes: self.put_bytes.load(Ordering::Relaxed),
            exits: self.exits.load(Ordering::Relaxed),
            exit_code: self.exit_code.load(Ordering::Relaxed),
            faults: self.faults.load(Ordering::Relaxed),
            fault_scause: self.fault_scause.load(Ordering::Relaxed),
            fault_stval: self.fault_stval.load(Ordering::Relaxed),
            fault_sepc: self.fault_sepc.load(Ordering::Relaxed),
            enter_sstatus: self.enter_sstatus.load(Ordering::Relaxed),
            trap_sstatus: self.trap_sstatus.load(Ordering::Relaxed),
        }
    }
}

impl Default for UserTask {
    fn default() -> Self {
        Self::new()
    }
}

/// Dải địa chỉ user hợp lệ `[base, top)` — arch cung cấp hằng số, logic kiểm ở đây để host-test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserRange {
    base: u64,
    top: u64,
}

impl UserRange {
    /// Dải `[base, top)`; `base >= top` ⇒ dải rỗng (mọi `contains` false).
    pub const fn new(base: u64, top: u64) -> Self {
        Self { base, top }
    }

    /// Địa chỉ nằm trong dải.
    pub const fn contains_addr(&self, addr: u64) -> bool {
        addr >= self.base && addr < self.top
    }

    /// `[ptr, ptr+len)` nằm **trọn** trong dải; tràn số ⇒ false (không panic trong debug).
    pub fn contains(&self, ptr: u64, len: u64) -> bool {
        if self.base >= self.top {
            return false;
        }
        if !self.contains_addr(ptr) {
            return false;
        }
        if len == 0 {
            return true;
        }
        match ptr.checked_add(len - 1) {
            Some(last) => self.contains_addr(last),
            None => false,
        }
    }
}

/// Lớp đọc bộ nhớ user (arch cài bằng `sstatus.SUM` + kiểm PTE; host test dùng bản giả).
///
/// KT-62: trait **giữ nguyên** cho arch; `put_str` là một phần của
/// [`vifast_core::syscall::SyscallEnv`] — đường `DebugPutStr` (12) đi qua đây.
pub trait UserMem {
    /// In `len` byte tại con trỏ user; trả số byte đã in hoặc mã lỗi ABI (1..19).
    fn put_str(&mut self, ptr: u64, len: u64) -> Result<u64, u64>;
}

/// Việc trap handler phải làm sau khi dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallAction {
    /// Trả `a0 = error`, `a1 = value`, `a2 = info`, `a3..a6 = mr` rồi `sepc += 4`.
    Return {
        /// `a0` — 0 = OK, 1..19 = mã lỗi ABI.
        error: u64,
        /// `a1`.
        value: u64,
        /// `a2` — `MessageInfo` đóng gói (syscall nhận) hoặc 0.
        info: u64,
        /// `a3..a6` — MR0..MR3 (hoặc `min_compat`/`features`/`num_harts` của `DebugGetInfo`).
        mr: [u64; 4],
    },
    /// `Exit(code)` — TCB đã vào `BlockedOnSend{fault_ep}` hoặc `Inactive`; **không** quay lại
    /// U-mode.
    Exit {
        /// Mã thoát.
        code: u64,
    },
    /// `DebugHalt(code)` — arch log rồi halt (chỉ ảnh test).
    Halt {
        /// Mã halt.
        code: u64,
    },
    /// TCB không còn `Running` (chặn/ngủ/sẵn sàng) — chưa có scheduler U-mode ở KT-62.
    Switched {
        /// Trạng thái TCB sau syscall.
        state: vifast_core::TcbState,
    },
}

/// Dispatch một syscall theo ABI — **façade** ghi state `UserTask` rồi chuyển sang bảng
/// dispatch thật của core ([`vifast_core::syscall::dispatch`]).
///
/// KT-62: không còn nhánh giả cho 0..11 — mọi số hiệu `0..15` đi vào [`vifast_core::Machine`];
/// ngoài dải ⇒ `RangeError` 19 (D2-79) từ **cùng** một chỗ trong core. Hàm này chỉ:
///
/// 1. ghi `syscalls`/`last_nr`;
/// 2. ghi `put_calls`/`put_bytes` (12), `exits`/`exit_code` (14), `bad_calls` (nr > 15);
/// 3. ghi `(a0, a1)` trả về;
/// 4. đổi [`SyscallOutcome`] thành [`SyscallAction`].
pub fn dispatch(
    m: &mut vifast_core::Machine,
    tcb: vifast_core::ObjId,
    hart: u16,
    nr: u64,
    args: [u64; 6],
    env: &mut impl vifast_core::syscall::SyscallEnv,
    task: &UserTask,
) -> SyscallAction {
    task.record_syscall(nr);
    match vifast_core::syscall::dispatch(m, tcb, hart, nr, args, env) {
        vifast_core::syscall::SyscallOutcome::Return(r) => {
            // Ngoài dải 0..15 (kể cả EID SBI) — core đã trả 19; façade chỉ ghi sổ `bad_calls`.
            if nr > SYSCALL_MAX {
                task.record_bad(nr, r.error);
            }
            if nr == SYS_DEBUG_PUT_STR && r.error == 0 {
                task.record_put(r.value);
            }
            task.record_return(r.error, r.value);
            SyscallAction::Return {
                error: r.error,
                value: r.value,
                info: r.info,
                mr: r.mr,
            }
        }
        vifast_core::syscall::SyscallOutcome::Exit { code } => {
            task.record_exit(code);
            SyscallAction::Exit { code }
        }
        vifast_core::syscall::SyscallOutcome::Halt { code } => SyscallAction::Halt { code },
        vifast_core::syscall::SyscallOutcome::Switched { state } => {
            SyscallAction::Switched { state }
        }
    }
}
