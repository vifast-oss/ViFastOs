// SPDX-License-Identifier: MIT OR Apache-2.0
//! `FaultKind` thật (KT-56, **D2-56a**) — phân loại trap phần cứng thành fault ABI.
//!
//! # Nguồn
//!
//! - `spec/abi.toml` `[method_signature.fault_kind]` (mở **một lần** ở KT-56 — D2-72 Q1):
//!   `Exit{code}`, `Vm{vspace, va, access, reason}`, `UnknownSyscall{nr}`,
//!   `UserException{ip, kind}`.
//! - Mô hình K2 `v2/sim/vifast-sim/src/tcb.rs` (`FaultKind`, `FaultAccess`, `VmFaultReason`):
//!   cùng **tên/trường/ngữ nghĩa**; khác duy nhất `vspace` là nhãn `u64` thay vì `ObjId`/`CapId`
//!   nội bộ backend.
//! - `v2/docs/k3-riscv64.md` §19 (bảng `scause` → `FaultKind`, bằng chứng trap thật).
//!
//! # Bất biến
//!
//! 1. **Trap là nguồn**: `classify_trap` chỉ nhận `TrapFacts` (scause/stval/sepc/from_user) do
//!    trapframe phần cứng sinh ra. `reason` cần PTE (`PteFacts`) vì `scause` một mình không nói
//!    vì sao (chưa map / thiếu quyền / W^X); PTE được đọc từ bảng **thật** của VSpace bị chạm.
//! 2. **W^X ở thời điểm dùng** (khớp `Machine::vm_touch_reason` của K2): ghi trang `X` hoặc thực
//!    thi trang `W` ⇒ `Wx` (kể cả khi cũng thiếu bit tương ứng); **quyền theo privilege** (U-mode
//!    chạm leaf `U=0`, hoặc S-mode chạm leaf `U=1` không `SUM`) thắng W^X ⇒ `Permission`.
//! 3. Không panic với mọi `scause`/PTE/giá trị lạ; `Unknown`-theo-K3-PLAN rơi vào
//!    `UserException{kind: scause}` — **không** thêm biến thể thứ năm ngoài mô hình K2.
//!
//! Tệp này **không `unsafe`** (`vifast-plat-common` là `#![forbid(unsafe_code)]`) và chạy được
//! trên host (D2-72 Q12) — chỉ đọc atomics.

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

// **KT-409** — bit PTE Sv39 nay ở `vifast-pte` (định dạng), không còn ở `crate::mmu` (đã dời
// sang `vifast-plat-riscv64`). `fault` ở lại `plat-common` vì cả 3 arch dùng.
use vifast_pte::{PTE_EXEC, PTE_USER, PTE_WRITE};

/// Hằng `scause` page fault (đã trừ bit interrupt): instruction/load/store.
pub const SCAUSE_INSTRUCTION_PAGE_FAULT: u64 = 12;
/// Load page fault.
pub const SCAUSE_LOAD_PAGE_FAULT: u64 = 13;
/// Store/AMO page fault.
pub const SCAUSE_STORE_PAGE_FAULT: u64 = 15;

/// Quyền truy cập bị fault — cùng tên/ngữ nghĩa `FaultAccess` của K2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultAccess {
    /// Đọc (`scause` 13).
    Read,
    /// Ghi (`scause` 15).
    Write,
    /// Thực thi (`scause` 12).
    Execute,
}

impl FaultAccess {
    /// Tên ổn định cho log/TAP (trùng K2).
    pub const fn name(self) -> &'static str {
        match self {
            FaultAccess::Read => "Read",
            FaultAccess::Write => "Write",
            FaultAccess::Execute => "Execute",
        }
    }

    /// Nghịch đảo [`Self::name`] — chuỗi lạ ⇒ `Read` (không panic).
    pub fn from_name(name: &str) -> Self {
        match name {
            "Write" => FaultAccess::Write,
            "Execute" => FaultAccess::Execute,
            _ => FaultAccess::Read,
        }
    }

    /// Mã ổn định (`Read`=0, `Write`=1, `Execute`=2).
    pub const fn code(self) -> u64 {
        match self {
            FaultAccess::Read => 0,
            FaultAccess::Write => 1,
            FaultAccess::Execute => 2,
        }
    }

    /// Nghịch đảo [`Self::code`] (mã lạ ⇒ `Read`).
    pub const fn from_code(code: u64) -> Self {
        match code {
            1 => FaultAccess::Write,
            2 => FaultAccess::Execute,
            _ => FaultAccess::Read,
        }
    }
}

/// Lý do fault VM — cùng tên/ngữ nghĩa `VmFaultReason` của K2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmFaultReason {
    /// Không có leaf phủ `va` (chưa map / bảng invalid).
    NotMapped,
    /// Có leaf nhưng thiếu quyền, hoặc quyền theo privilege (U/S) không cho.
    Permission,
    /// Truy cập tạo thế W+X ở thời điểm dùng (ghi trang X / thực thi trang W).
    Wx,
}

impl VmFaultReason {
    /// Tên ổn định cho log/TAP (trùng K2).
    pub const fn name(self) -> &'static str {
        match self {
            VmFaultReason::NotMapped => "NotMapped",
            VmFaultReason::Permission => "Permission",
            VmFaultReason::Wx => "Wx",
        }
    }

    /// Mã ổn định (`NotMapped`=0, `Permission`=1, `Wx`=2).
    pub const fn code(self) -> u64 {
        match self {
            VmFaultReason::NotMapped => 0,
            VmFaultReason::Permission => 1,
            VmFaultReason::Wx => 2,
        }
    }

    /// Nghịch đảo [`Self::code`] (mã lạ ⇒ `NotMapped`).
    pub const fn from_code(code: u64) -> Self {
        match code {
            1 => VmFaultReason::Permission,
            2 => VmFaultReason::Wx,
            _ => VmFaultReason::NotMapped,
        }
    }
}

/// `FaultKind` ABI — **đúng** `spec/abi.toml` `[method_signature.fault_kind]` (D2-56a).
///
/// `Copy` để log/TAP đọc nhất quán; `vspace` là nhãn u64 (ASID/ObjId) — không phải cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// `Exit(14)` — đi hàng gửi **không Reply** (D2-25).
    Exit {
        /// Mã thoát (`a0` của `Exit`).
        code: u64,
    },
    /// Page fault **thật** (scause 12/13/15) — K2/D2-65 Q6.
    Vm {
        /// Nhãn VSpace bị chạm (ASID u64; 0 = root kernel/không xác định).
        vspace: u64,
        /// `stval` — địa chỉ **yêu cầu** (không làm tròn về gốc mapping).
        va: u64,
        /// Quyền truy cập theo scause.
        access: FaultAccess,
        /// Lý do từ PTE (xem [`vm_reason`]).
        reason: VmFaultReason,
    },
    /// Syscall không nhận diện được — biến thể ABI cho procsrv K3.5 (K3 không tạo từ phần cứng).
    UnknownSyscall {
        /// Số hiệu syscall.
        nr: u64,
    },
    /// Exception phần cứng khác (illegal/breakpoint/misaligned/access fault/…).
    UserException {
        /// `sepc` — địa chỉ instruction gây exception.
        ip: u64,
        /// `scause` (không có bit interrupt).
        kind: u64,
    },
}

impl FaultKind {
    /// Tên biến thể (khớp `names` của spec và K2).
    pub const fn name(&self) -> &'static str {
        match self {
            FaultKind::Exit { .. } => "Exit",
            FaultKind::Vm { .. } => "Vm",
            FaultKind::UnknownSyscall { .. } => "UnknownSyscall",
            FaultKind::UserException { .. } => "UserException",
        }
    }

    /// Chỉ `Exit` đi hàng gửi không Reply (D2-25).
    pub const fn is_exit(self) -> bool {
        matches!(self, FaultKind::Exit { .. })
    }

    /// Mã thoát nếu là `Exit`.
    pub const fn exit_code(self) -> Option<u64> {
        match self {
            FaultKind::Exit { code } => Some(code),
            _ => None,
        }
    }

    /// `(vspace, va, access, reason)` nếu là `Vm`.
    pub const fn vm(self) -> Option<(u64, u64, FaultAccess, VmFaultReason)> {
        match self {
            FaultKind::Vm {
                vspace,
                va,
                access,
                reason,
            } => Some((vspace, va, access, reason)),
            _ => None,
        }
    }
}

/// `Display` ổn định (không dùng `Debug`) — **một** định dạng cho log/journal/TAP:
/// `Exit{42}`, `Vm{vspace=1, va=0x200040, access=Write, reason=NotMapped}`,
/// `UnknownSyscall{nr=99}`, `UserException{ip=0x40000000, kind=2}`.
impl fmt::Display for FaultKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            FaultKind::Exit { code } => write!(f, "Exit{{{code}}}"),
            FaultKind::Vm {
                vspace,
                va,
                access,
                reason,
            } => write!(
                f,
                "Vm{{vspace={vspace}, va={va:#x}, access={}, reason={}}}",
                access.name(),
                reason.name()
            ),
            FaultKind::UnknownSyscall { nr } => write!(f, "UnknownSyscall{{nr={nr}}}"),
            FaultKind::UserException { ip, kind } => {
                write!(f, "UserException{{ip={ip:#x}, kind={kind}}}")
            }
        }
    }
}

/// Dữ kiện trap phần cứng — **chỉ** các trường trapframe, không suy diễn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapFacts {
    /// `scause` nguyên giá trị (bit 63 = 0; caller chỉ gọi cho exception).
    pub scause: u64,
    /// `stval`.
    pub stval: u64,
    /// `sepc`.
    pub sepc: u64,
    /// `sstatus.SPP == 0` (trap từ U-mode).
    pub from_user: bool,
}

impl TrapFacts {
    /// Mã exception (bỏ bit interrupt nếu có — phòng thủ).
    pub const fn code(&self) -> u64 {
        self.scause & !(1u64 << 63)
    }
}

/// Dữ kiện PTE tại `stval` — đọc từ bảng **thật** của VSpace bị chạm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PteFacts {
    /// Entry có bit `V`.
    pub valid: bool,
    /// Entry là leaf (`R|W|X`).
    pub leaf: bool,
    /// Bit thô của entry.
    pub raw: u64,
}

impl PteFacts {
    /// Từ entry thô: `valid = V`, `leaf = V && (R|W|X)`.
    pub const fn from_raw(raw: u64) -> Self {
        // `PTE_VALID` = 1; R/W/X = bit 1/2/3.
        let valid = raw & 1 != 0;
        let leaf = valid && raw & 0b1110 != 0;
        Self { valid, leaf, raw }
    }

    /// Từ kết quả walk của object layer.
    pub const fn new(valid: bool, leaf: bool, raw: u64) -> Self {
        Self { valid, leaf, raw }
    }

    /// Leaf có bit `U`.
    pub const fn user(&self) -> bool {
        self.raw & PTE_USER != 0
    }

    /// Leaf có bit `W`.
    pub const fn writable(&self) -> bool {
        self.raw & PTE_WRITE != 0
    }

    /// Leaf có bit `X`.
    pub const fn executable(&self) -> bool {
        self.raw & PTE_EXEC != 0
    }
}

/// Lý do fault VM từ **quyền truy cập + PTE thật** (hàm thuần; khớp luật K2
/// `Machine::vm_touch_reason`, cộng kiểm privilege U/S của K3).
///
/// Thứ tự luật:
/// 1. Không có leaf hợp lệ ⇒ `NotMapped`.
/// 2. U-mode chạm leaf `U=0`, hoặc S-mode chạm leaf `U=1` (không `SUM`) ⇒ `Permission`
///    — quyền privilege thắng W^X (kernel text `RX` không biến thành `Wx` khi user ghi).
/// 3. `Write` trên leaf không `W` mà có `X` ⇒ `Wx`; `Execute` trên leaf không `X` mà có `W`
///    ⇒ `Wx`; còn lại thiếu quyền ⇒ `Permission`.
pub fn vm_reason(access: FaultAccess, from_user: bool, pte: Option<PteFacts>) -> VmFaultReason {
    let Some(pte) = pte else {
        return VmFaultReason::NotMapped;
    };
    if !pte.valid || !pte.leaf {
        return VmFaultReason::NotMapped;
    }
    if from_user != pte.user() {
        // U-mode → leaf U=0; S-mode → leaf U=1 mà SUM tắt (fault path luôn SUM=0).
        return VmFaultReason::Permission;
    }
    let w = pte.writable();
    let x = pte.executable();
    match access {
        // Fault đọc trên leaf hợp lệ: thiếu `R` (Sv39 `W=1 ⇒ R=1`, nên chỉ còn X-only).
        FaultAccess::Read => VmFaultReason::Permission,
        FaultAccess::Write => {
            if x {
                VmFaultReason::Wx
            } else {
                VmFaultReason::Permission
            }
        }
        FaultAccess::Execute => {
            if w {
                VmFaultReason::Wx
            } else {
                VmFaultReason::Permission
            }
        }
    }
}

/// Phân loại **một** trap exception phần cứng thành `FaultKind` (D2-56a).
///
/// `vspace` = nhãn VSpace bị chạm; `pte` = entry tại `stval` (nếu có). `None` cho `ecall`
/// (scause 8 — syscall, không phải fault) để caller đi đường dispatch; mọi mã khác đều trả
/// một biến thể (không panic).
pub fn classify_trap(trap: &TrapFacts, vspace: u64, pte: Option<PteFacts>) -> Option<FaultKind> {
    let code = trap.code();
    match code {
        SCAUSE_INSTRUCTION_PAGE_FAULT => Some(FaultKind::Vm {
            vspace,
            va: trap.stval,
            access: FaultAccess::Execute,
            reason: vm_reason(FaultAccess::Execute, trap.from_user, pte),
        }),
        SCAUSE_LOAD_PAGE_FAULT => Some(FaultKind::Vm {
            vspace,
            va: trap.stval,
            access: FaultAccess::Read,
            reason: vm_reason(FaultAccess::Read, trap.from_user, pte),
        }),
        SCAUSE_STORE_PAGE_FAULT => Some(FaultKind::Vm {
            vspace,
            va: trap.stval,
            access: FaultAccess::Write,
            reason: vm_reason(FaultAccess::Write, trap.from_user, pte),
        }),
        // syscall từ U-mode: không phải fault, đi dispatcher ABI.
        8 => None,
        // illegal (2), breakpoint (3), misaligned/access fault (0/1/4/5/6/7), ecall-S (9),
        // và mọi mã lạ: `UserException` mang `kind` = scause (không thêm biến thể thứ năm).
        other => Some(FaultKind::UserException {
            ip: trap.sepc,
            kind: other,
        }),
    }
}

/// Ghi nhận `FaultKind` gần nhất + đếm theo biến thể — atomics, đọc từ TAP không cần `static mut`.
#[derive(Debug)]
pub struct FaultStats {
    tag: AtomicU64,
    a: AtomicU64,
    b: AtomicU64,
    c: AtomicU64,
    d: AtomicU64,
    count: AtomicU64,
}

/// Mã tag nội bộ của [`FaultStats`].
const TAG_NONE: u64 = 0;
/// `Exit`.
pub const TAG_EXIT: u64 = 1;
/// `Vm`.
pub const TAG_VM: u64 = 2;
/// `UnknownSyscall`.
pub const TAG_UNKNOWN_SYSCALL: u64 = 3;
/// `UserException`.
pub const TAG_USER_EXCEPTION: u64 = 4;

impl FaultStats {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            tag: AtomicU64::new(TAG_NONE),
            a: AtomicU64::new(0),
            b: AtomicU64::new(0),
            c: AtomicU64::new(0),
            d: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Ghi `FaultKind` vừa phân loại (một hart ở K3; K3b mới cần đồng bộ chặt hơn).
    pub fn record(&self, kind: FaultKind) {
        let (tag, a, b, c, d) = match kind {
            FaultKind::Exit { code } => (TAG_EXIT, code, 0, 0, 0),
            FaultKind::Vm {
                vspace,
                va,
                access,
                reason,
            } => (TAG_VM, vspace, va, access.code(), reason.code()),
            FaultKind::UnknownSyscall { nr } => (TAG_UNKNOWN_SYSCALL, nr, 0, 0, 0),
            FaultKind::UserException { ip, kind } => (TAG_USER_EXCEPTION, ip, kind, 0, 0),
        };
        self.a.store(a, Ordering::Relaxed);
        self.b.store(b, Ordering::Relaxed);
        self.c.store(c, Ordering::Relaxed);
        self.d.store(d, Ordering::Relaxed);
        self.tag.store(tag, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// `FaultKind` gần nhất (nếu có).
    pub fn last(&self) -> Option<FaultKind> {
        let a = self.a.load(Ordering::Relaxed);
        match self.tag.load(Ordering::Relaxed) {
            TAG_EXIT => Some(FaultKind::Exit { code: a }),
            TAG_VM => Some(FaultKind::Vm {
                vspace: a,
                va: self.b.load(Ordering::Relaxed),
                access: FaultAccess::from_code(self.c.load(Ordering::Relaxed)),
                reason: VmFaultReason::from_code(self.d.load(Ordering::Relaxed)),
            }),
            TAG_UNKNOWN_SYSCALL => Some(FaultKind::UnknownSyscall { nr: a }),
            TAG_USER_EXCEPTION => Some(FaultKind::UserException {
                ip: a,
                kind: self.b.load(Ordering::Relaxed),
            }),
            _ => None,
        }
    }

    /// Tổng số fault đã ghi.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

impl Default for FaultStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm toàn cục của ảnh kernel — trap handler ghi, TAP đọc.
pub static FAULT_STATS: FaultStats = FaultStats::new();
