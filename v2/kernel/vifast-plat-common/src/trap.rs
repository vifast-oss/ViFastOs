// SPDX-License-Identifier: MIT OR Apache-2.0
//! Khung trap dùng chung (KT-45): trapframe, decode `scause`, `Event::Exception`, thống kê trap.
//!
//! Tệp này **không có `unsafe`** (`vifast-plat-common` là `#![forbid(unsafe_code)]`): asm lưu/khôi
//! phục thanh ghi nằm ở `kernel/vifast-plat-riscv64` (D2-17), còn *ngữ nghĩa* trap — bố cục
//! trapframe, mã `scause`, chính sách resume/halt — nằm ở đây để **test được trên host**
//! (D2-72 Q12). Trapframe `#[repr(C)]` với offset hằng số được asm dùng làm `const` operand, và
//! các `const` assert bên dưới chặn lệch offset giữa Rust và asm — đây là rủi ro chính của KT-45
//! (`review/agent0/K3-PLAN.md` §4.1).
//!
//! Phạm vi (D2-73): decode **mọi** `scause` 0/2/3/4/5/6/7/8/9/12/13/15/khác. TAP phần cứng chỉ
//! `breakpoint` (scause 3) và `ecall-S` (do M-mode/OpenSBI xử lý, chỉ chứng minh resume đúng
//! `sepc`); `illegal` (scause 2) kiểm bằng host test + trap **tiêm** vì OpenSBI v1.8.1
//! `delegate_traps()` đặt `MEDELEG = 0xf4b509` (bit 2 = 0, bit 9 = 0). Page fault thật (13/15)
//! thuộc KT-46. Không có U-mode/timer/PLIC ở KT-45.
//!
//! ## Bố cục trapframe (288 B, 8-byte aligned)
//!
//! ```text
//!   0..256  x0..x31   (x0 luôn 0; x2 = sp bị ngắt)
//!   256     sepc
//!   264     sstatus
//!   272     scause
//!   280     stval
//! ```
//!
//! Asm của `vifast-plat-riscv64` **không hardcode** offset: nó nhận các hằng `TF_*_OFFSET` ở đây
//! làm `const` operand, và `const` assert cuối tệp đối chiếu với `offset_of!`.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::{Platform, PlatformWriter};

/// Bit 63 của `scause`: 1 = interrupt, 0 = exception (RISC-V privileged spec).
pub const SCAUSE_INTERRUPT: u64 = 1 << 63;

/// `sstatus.SIE` — supervisor interrupt enable.
pub const SSTATUS_SIE: u64 = 1 << 1;
/// `sstatus.SPIE` — giá trị SIE được khôi phục bởi `sret`.
pub const SSTATUS_SPIE: u64 = 1 << 5;
/// `sstatus.SPP` — privilege trước trap: 1 = S-mode, 0 = U-mode.
pub const SSTATUS_SPP: u64 = 1 << 8;

/// Exception code 0 — instruction address misaligned.
pub const EXC_INSTRUCTION_ADDRESS_MISALIGNED: u64 = 0;
/// Exception code 1 — instruction access fault.
pub const EXC_INSTRUCTION_ACCESS_FAULT: u64 = 1;
/// Exception code 2 — illegal instruction (`stval` = chính instruction lỗi).
pub const EXC_ILLEGAL_INSTRUCTION: u64 = 2;
/// Exception code 3 — breakpoint (`ebreak`).
pub const EXC_BREAKPOINT: u64 = 3;
/// Exception code 4 — load address misaligned.
pub const EXC_LOAD_ADDRESS_MISALIGNED: u64 = 4;
/// Exception code 5 — load access fault.
pub const EXC_LOAD_ACCESS_FAULT: u64 = 5;
/// Exception code 6 — store/AMO address misaligned.
pub const EXC_STORE_ADDRESS_MISALIGNED: u64 = 6;
/// Exception code 7 — store/AMO access fault.
pub const EXC_STORE_ACCESS_FAULT: u64 = 7;
/// Exception code 8 — environment call từ U-mode.
pub const EXC_ENVIRONMENT_CALL_FROM_U: u64 = 8;
/// Exception code 9 — environment call từ S-mode (SBI; OpenSBI **không** delegate — D2-73).
pub const EXC_ENVIRONMENT_CALL_FROM_S: u64 = 9;
/// Exception code 12 — instruction page fault.
pub const EXC_INSTRUCTION_PAGE_FAULT: u64 = 12;
/// Exception code 13 — load page fault.
pub const EXC_LOAD_PAGE_FAULT: u64 = 13;
/// Exception code 15 — store/AMO page fault.
pub const EXC_STORE_PAGE_FAULT: u64 = 15;

/// Chỉ số thanh ghi trong [`TrapFrame::x`] (theo ABI RISC-V).
pub const REG_ZERO: usize = 0;
/// `ra` / `x1`.
pub const REG_RA: usize = 1;
/// `sp` / `x2`.
pub const REG_SP: usize = 2;
/// `gp` / `x3`.
pub const REG_GP: usize = 3;
/// `tp` / `x4`.
pub const REG_TP: usize = 4;
/// `t0` / `x5`.
pub const REG_T0: usize = 5;
/// `a0` / `x10` — tham số 0 và mã trả về SBI.
pub const REG_A0: usize = 10;
/// `a7` / `x17` — số hiệu syscall/SBI.
pub const REG_A7: usize = 17;

/// Số thanh ghi được lưu trong trapframe (x0..x31).
pub const TF_REG_COUNT: usize = 32;
/// Số byte một thanh ghi trong trapframe.
pub const TF_REG_BYTES: usize = 8;
/// Offset của `sepc` trong [`TrapFrame`].
pub const TF_SEPC_OFFSET: usize = TF_REG_COUNT * TF_REG_BYTES;
/// Offset của `sstatus` trong [`TrapFrame`].
pub const TF_SSTATUS_OFFSET: usize = TF_SEPC_OFFSET + 8;
/// Offset của `scause` trong [`TrapFrame`].
pub const TF_SCAUSE_OFFSET: usize = TF_SSTATUS_OFFSET + 8;
/// Offset của `stval` trong [`TrapFrame`].
pub const TF_STVAL_OFFSET: usize = TF_SCAUSE_OFFSET + 8;
/// Tổng cỡ trapframe (32 thanh ghi + 4 CSR).
pub const TF_SIZE: usize = TF_STVAL_OFFSET + 8;
/// Offset của một thanh ghi tổng quát trong [`TrapFrame`].
///
/// `const fn` để asm dùng được `TF_REG_OFFSET + 8 * n` làm `const` operand.
pub const fn tf_reg_offset(index: usize) -> usize {
    index * TF_REG_BYTES
}

/// Trapframe đúng thứ tự asm lưu/khôi phục (`#[repr(C)]`, 8-byte aligned).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapFrame {
    /// x0..x31; `x[0]` luôn 0, `x[2]` là `sp` **bị ngắt** (asm lưu vào đây sau khi đổi stack).
    pub x: [u64; TF_REG_COUNT],
    /// `sepc` tại thời điểm trap.
    pub sepc: u64,
    /// `sstatus` tại thời điểm trap (asm khôi phục nguyên giá trị trước `sret`).
    pub sstatus: u64,
    /// `scause` — nguyên giá trị (bit 63 phân biệt interrupt/exception).
    pub scause: u64,
    /// `stval` — thông tin phụ (địa chỉ lỗi, instruction lỗi…).
    pub stval: u64,
}

impl TrapFrame {
    /// Trapframe rỗng — dùng cho test host và trap **tiêm**.
    pub const fn zeroed() -> Self {
        Self {
            x: [0; TF_REG_COUNT],
            sepc: 0,
            sstatus: 0,
            scause: 0,
            stval: 0,
        }
    }

    /// Đọc thanh ghi thứ `index` (0..32).
    pub fn reg(&self, index: usize) -> u64 {
        self.x[index]
    }

    /// Ghi thanh ghi thứ `index` (0..32).
    pub fn set_reg(&mut self, index: usize, value: u64) {
        self.x[index] = value;
    }

    /// Đọc tham số `a0..a7` (`index` 0..8).
    pub fn arg(&self, index: usize) -> u64 {
        self.x[REG_A0 + index]
    }

    /// Ghi tham số `a0..a7` (`index` 0..8).
    pub fn set_arg(&mut self, index: usize, value: u64) {
        self.x[REG_A0 + index] = value;
    }

    /// Trap có đến từ S-mode không (`sstatus.SPP = 1`). Ở KT-45 luôn đúng; U-mode là KT-53.
    pub fn from_supervisor(&self) -> bool {
        self.sstatus & SSTATUS_SPP != 0
    }

    /// Cộng `bytes` vào `sepc` (bỏ qua instruction vừa trap) — chính sách resume của KT-45.
    pub fn advance_pc(&mut self, bytes: usize) {
        self.sepc = self.sepc.wrapping_add(bytes as u64);
    }
}

/// Loại exception theo mã `scause` (0/1/2/3/4/5/6/7/8/9/12/13/15; còn lại `Unknown`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExceptionKind {
    /// 0 — instruction address misaligned.
    InstructionAddressMisaligned,
    /// 1 — instruction access fault.
    InstructionAccessFault,
    /// 2 — illegal instruction (OpenSBI **không** delegate — D2-73).
    IllegalInstruction,
    /// 3 — breakpoint (`ebreak`; OpenSBI delegate).
    Breakpoint,
    /// 4 — load address misaligned.
    LoadAddressMisaligned,
    /// 5 — load access fault.
    LoadAccessFault,
    /// 6 — store/AMO address misaligned.
    StoreAddressMisaligned,
    /// 7 — store/AMO access fault.
    StoreAccessFault,
    /// 8 — environment call từ U-mode (OpenSBI delegate; syscall là KT-53).
    EnvironmentCallFromU,
    /// 9 — environment call từ S-mode (SBI; OpenSBI **không** delegate — D2-73).
    EnvironmentCallFromS,
    /// 12 — instruction page fault.
    InstructionPageFault,
    /// 13 — load page fault (guard page thật là KT-46).
    LoadPageFault,
    /// 15 — store/AMO page fault (guard page thật là KT-46).
    StorePageFault,
    /// Mã khác — không thuộc bảng KT-45.
    Unknown(u64),
}

impl ExceptionKind {
    /// Ánh xạ mã `scause` (đã bỏ bit interrupt) sang loại exception.
    pub const fn from_code(code: u64) -> Self {
        match code {
            EXC_INSTRUCTION_ADDRESS_MISALIGNED => Self::InstructionAddressMisaligned,
            EXC_INSTRUCTION_ACCESS_FAULT => Self::InstructionAccessFault,
            EXC_ILLEGAL_INSTRUCTION => Self::IllegalInstruction,
            EXC_BREAKPOINT => Self::Breakpoint,
            EXC_LOAD_ADDRESS_MISALIGNED => Self::LoadAddressMisaligned,
            EXC_LOAD_ACCESS_FAULT => Self::LoadAccessFault,
            EXC_STORE_ADDRESS_MISALIGNED => Self::StoreAddressMisaligned,
            EXC_STORE_ACCESS_FAULT => Self::StoreAccessFault,
            EXC_ENVIRONMENT_CALL_FROM_U => Self::EnvironmentCallFromU,
            EXC_ENVIRONMENT_CALL_FROM_S => Self::EnvironmentCallFromS,
            EXC_INSTRUCTION_PAGE_FAULT => Self::InstructionPageFault,
            EXC_LOAD_PAGE_FAULT => Self::LoadPageFault,
            EXC_STORE_PAGE_FAULT => Self::StorePageFault,
            other => Self::Unknown(other),
        }
    }

    /// Mã `scause` tương ứng (ổn định cho log/trace).
    pub const fn code(&self) -> u64 {
        match self {
            Self::InstructionAddressMisaligned => EXC_INSTRUCTION_ADDRESS_MISALIGNED,
            Self::InstructionAccessFault => EXC_INSTRUCTION_ACCESS_FAULT,
            Self::IllegalInstruction => EXC_ILLEGAL_INSTRUCTION,
            Self::Breakpoint => EXC_BREAKPOINT,
            Self::LoadAddressMisaligned => EXC_LOAD_ADDRESS_MISALIGNED,
            Self::LoadAccessFault => EXC_LOAD_ACCESS_FAULT,
            Self::StoreAddressMisaligned => EXC_STORE_ADDRESS_MISALIGNED,
            Self::StoreAccessFault => EXC_STORE_ACCESS_FAULT,
            Self::EnvironmentCallFromU => EXC_ENVIRONMENT_CALL_FROM_U,
            Self::EnvironmentCallFromS => EXC_ENVIRONMENT_CALL_FROM_S,
            Self::InstructionPageFault => EXC_INSTRUCTION_PAGE_FAULT,
            Self::LoadPageFault => EXC_LOAD_PAGE_FAULT,
            Self::StorePageFault => EXC_STORE_PAGE_FAULT,
            Self::Unknown(code) => *code,
        }
    }

    /// Tên ổn định cho log/TAP (`Breakpoint`, `IllegalInstruction`, …).
    pub const fn name(&self) -> &'static str {
        match self {
            Self::InstructionAddressMisaligned => "InstructionAddressMisaligned",
            Self::InstructionAccessFault => "InstructionAccessFault",
            Self::IllegalInstruction => "IllegalInstruction",
            Self::Breakpoint => "Breakpoint",
            Self::LoadAddressMisaligned => "LoadAddressMisaligned",
            Self::LoadAccessFault => "LoadAccessFault",
            Self::StoreAddressMisaligned => "StoreAddressMisaligned",
            Self::StoreAccessFault => "StoreAccessFault",
            Self::EnvironmentCallFromU => "EnvironmentCallFromU",
            Self::EnvironmentCallFromS => "EnvironmentCallFromS",
            Self::InstructionPageFault => "InstructionPageFault",
            Self::LoadPageFault => "LoadPageFault",
            Self::StorePageFault => "StorePageFault",
            Self::Unknown(_) => "Unknown",
        }
    }

    /// KT-45 coi một số exception là **resume được** (bỏ qua instruction rồi chạy tiếp).
    ///
    /// - `Breakpoint`, `IllegalInstruction`: instruction đã fetch được, chỉ cần `sepc += len`.
    /// - `EnvironmentCallFromU`: KT-53 sẽ biến thành syscall; KT-45 chỉ resume để không treo.
    /// - `EnvironmentCallFromS`: OpenSBI **không delegate** (D2-73) nên thực tế không vào `stvec`;
    ///   vẫn xử lý phòng thủ (resume) để một firmware delegate khác không làm treo.
    ///
    /// Page fault/access fault/misaligned là **fatal** ở KT-45; guard page thật là KT-46.
    pub const fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Breakpoint
                | Self::IllegalInstruction
                | Self::EnvironmentCallFromU
                | Self::EnvironmentCallFromS
        )
    }
}

/// Một exception đã decode, kèm ngữ cảnh trapframe (đủ để dump).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exception {
    /// `scause` nguyên giá trị (bit 63 = 0).
    pub scause: u64,
    /// Loại exception.
    pub kind: ExceptionKind,
    /// `sepc` tại thời điểm trap.
    pub sepc: u64,
    /// `stval` tại thời điểm trap.
    pub stval: u64,
    /// Trap đến từ S-mode (`sstatus.SPP`).
    pub from_supervisor: bool,
}

impl Exception {
    /// Tên loại exception — tiện cho `write!`.
    pub const fn name(&self) -> &'static str {
        self.kind.name()
    }

    /// Mã exception — tiện so sánh/log.
    pub const fn code(&self) -> u64 {
        self.kind.code()
    }
}

/// Sự kiện trap đã decode — yêu cầu KT-45: `scause` exception ⇒ [`Event::Exception`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Trap đồng bộ (exception) — xem [`Exception`].
    Exception(Exception),
    /// Trap ngắt (bit 63 = 1). Timer/PLIC là KT-47/48; KT-45 chỉ log rồi halt.
    Interrupt {
        /// Mã interrupt (đã bỏ bit 63).
        code: u64,
    },
}

/// Decode `scause` **không** kèm ngữ cảnh (sepc/stval = 0) — dùng cho unit test và bảng mã.
pub fn decode_scause(scause: u64) -> Event {
    if scause & SCAUSE_INTERRUPT != 0 {
        Event::Interrupt {
            code: scause & !SCAUSE_INTERRUPT,
        }
    } else {
        Event::Exception(Exception {
            scause,
            kind: ExceptionKind::from_code(scause),
            sepc: 0,
            stval: 0,
            from_supervisor: false,
        })
    }
}

/// Decode `scause` **kèm** ngữ cảnh trapframe — đường đi thật của trap handler.
pub fn decode_trap(tf: &TrapFrame) -> Event {
    match decode_scause(tf.scause) {
        Event::Exception(mut exc) => {
            exc.sepc = tf.sepc;
            exc.stval = tf.stval;
            exc.from_supervisor = tf.from_supervisor();
            Event::Exception(exc)
        }
        Event::Interrupt { code } => Event::Interrupt { code },
    }
}

/// Predicate phát hiện trap **lồng**: `sp` bị ngắt nằm trong trap stack.
///
/// Asm của `vifast-plat-riscv64` cài đúng predicate này bằng `bgeu`/`bltu` (biên dưới **không**
/// tính, biên trên **không** tính); hàm ở đây để host test kiểm cùng ngữ nghĩa (P2b, KT-45).
pub const fn is_nested_trap_sp(sp: u64, stack_bottom: u64, stack_top: u64) -> bool {
    sp > stack_bottom && sp < stack_top
}

/// Việc trap handler phải làm sau khi decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapAction {
    /// Đã cộng `sepc` qua instruction lỗi; asm khôi phục thanh ghi rồi `sret`.
    Resume,
    /// Exception không cứu được ở KT-45 — dump rồi `DebugHalt(1)`.
    Fatal(Exception),
    /// Ngắt bất ngờ (chưa bật timer/PLIC) — dump rồi `DebugHalt(1)`.
    Interrupt {
        /// Mã interrupt (đã bỏ bit 63).
        code: u64,
    },
}

/// Chính sách KT-45: exception resume được ⇒ cộng `sepc` đúng cỡ instruction; còn lại fatal.
///
/// `insn_len` do arch đo (2 hay 4 byte). Với `ecall` luôn 4; với `illegal`/`breakpoint` arch đọc
/// halfword tại `sepc` (instruction đã fetch được nên đọc an toàn).
pub fn handle_trap(tf: &mut TrapFrame, insn_len: usize) -> TrapAction {
    match decode_trap(tf) {
        Event::Exception(exc) => {
            if exc.kind.is_recoverable() {
                tf.advance_pc(insn_len);
                TrapAction::Resume
            } else {
                TrapAction::Fatal(exc)
            }
        }
        Event::Interrupt { code } => TrapAction::Interrupt { code },
    }
}

/// Ảnh chụp bộ đếm trap — `Copy` để test đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrapSnapshot {
    /// Tổng số trap đã vào `stvec`.
    pub traps: u64,
    /// Số trap là exception.
    pub exceptions: u64,
    /// Số trap là interrupt.
    pub interrupts: u64,
    /// Số exception được resume (đã cộng `sepc`).
    pub recovered: u64,
    /// Số exception fatal (đã dump + halt).
    pub fatal: u64,
    /// `scause` của trap gần nhất.
    pub last_scause: u64,
    /// `sepc` **tại thời điểm trap** của trap gần nhất (không phải sepc sau resume).
    pub last_sepc: u64,
    /// `stval` của trap gần nhất.
    pub last_stval: u64,
    /// `sstatus` của trap gần nhất.
    pub last_sstatus: u64,
    /// Mã exception/interrupt của trap gần nhất.
    pub last_code: u64,
}

/// Bộ đếm trap toàn cục — ktest đọc để xác nhận `scause`/`sepc` (không cần `static mut`).
#[derive(Debug)]
pub struct TrapStats {
    traps: AtomicU64,
    exceptions: AtomicU64,
    interrupts: AtomicU64,
    recovered: AtomicU64,
    fatal: AtomicU64,
    last_scause: AtomicU64,
    last_sepc: AtomicU64,
    last_stval: AtomicU64,
    last_sstatus: AtomicU64,
    last_code: AtomicU64,
}

impl TrapStats {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            traps: AtomicU64::new(0),
            exceptions: AtomicU64::new(0),
            interrupts: AtomicU64::new(0),
            recovered: AtomicU64::new(0),
            fatal: AtomicU64::new(0),
            last_scause: AtomicU64::new(0),
            last_sepc: AtomicU64::new(0),
            last_stval: AtomicU64::new(0),
            last_sstatus: AtomicU64::new(0),
            last_code: AtomicU64::new(0),
        }
    }

    /// Ghi một trap exception; `tf` phải là bản **trước** khi cộng `sepc`.
    pub fn record_exception(&self, tf: &TrapFrame, kind: ExceptionKind, recovered: bool) {
        self.traps.fetch_add(1, Ordering::Relaxed);
        self.exceptions.fetch_add(1, Ordering::Relaxed);
        if recovered {
            self.recovered.fetch_add(1, Ordering::Relaxed);
        } else {
            self.fatal.fetch_add(1, Ordering::Relaxed);
        }
        self.last_scause.store(tf.scause, Ordering::Relaxed);
        self.last_sepc.store(tf.sepc, Ordering::Relaxed);
        self.last_stval.store(tf.stval, Ordering::Relaxed);
        self.last_sstatus.store(tf.sstatus, Ordering::Relaxed);
        self.last_code.store(kind.code(), Ordering::Relaxed);
    }

    /// Ghi một trap interrupt (timer/PLIC là KT-47/48; KT-45 chỉ ghi nhận).
    pub fn record_interrupt(&self, tf: &TrapFrame, code: u64) {
        self.traps.fetch_add(1, Ordering::Relaxed);
        self.interrupts.fetch_add(1, Ordering::Relaxed);
        self.last_scause
            .store(SCAUSE_INTERRUPT | code, Ordering::Relaxed);
        self.last_sepc.store(tf.sepc, Ordering::Relaxed);
        self.last_stval.store(tf.stval, Ordering::Relaxed);
        self.last_sstatus.store(tf.sstatus, Ordering::Relaxed);
        self.last_code.store(code, Ordering::Relaxed);
    }

    /// Đọc nhất quán (từng trường `Relaxed`; đủ cho test một hart, KT-45).
    pub fn snapshot(&self) -> TrapSnapshot {
        TrapSnapshot {
            traps: self.traps.load(Ordering::Relaxed),
            exceptions: self.exceptions.load(Ordering::Relaxed),
            interrupts: self.interrupts.load(Ordering::Relaxed),
            recovered: self.recovered.load(Ordering::Relaxed),
            fatal: self.fatal.load(Ordering::Relaxed),
            last_scause: self.last_scause.load(Ordering::Relaxed),
            last_sepc: self.last_sepc.load(Ordering::Relaxed),
            last_stval: self.last_stval.load(Ordering::Relaxed),
            last_sstatus: self.last_sstatus.load(Ordering::Relaxed),
            last_code: self.last_code.load(Ordering::Relaxed),
        }
    }

    /// Xoá bộ đếm — chỉ dùng trong test host (ảnh thật không reset).
    pub fn reset(&self) {
        for field in [
            &self.traps,
            &self.exceptions,
            &self.interrupts,
            &self.recovered,
            &self.fatal,
            &self.last_scause,
            &self.last_sepc,
            &self.last_stval,
            &self.last_sstatus,
            &self.last_code,
        ] {
            field.store(0, Ordering::Relaxed);
        }
    }
}

impl Default for TrapStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm trap toàn cục của ảnh kernel (một hart ở KT-45; PerHart là KT-49).
pub static TRAP_STATS: TrapStats = TrapStats::new();

/// Ảnh chụp bộ đếm **guard page fault** (KT-46) — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuardSnapshot {
    /// Tổng số page fault trúng trang bảo vệ.
    pub faults: u64,
    /// Số fault là load page fault (`scause=13`).
    pub load_faults: u64,
    /// Số fault là store page fault (`scause=15`).
    pub store_faults: u64,
    /// Số fault được resume (chỉ probe test của KT-46).
    pub recovered: u64,
    /// Số fault fatal (dump + halt) — chính sách mặc định.
    pub fatal: u64,
    /// `scause` gần nhất.
    pub last_scause: u64,
    /// `stval` gần nhất (địa chỉ trong guard page).
    pub last_stval: u64,
    /// `sepc` gần nhất (instruction chạm guard).
    pub last_sepc: u64,
}

/// Bộ đếm guard page fault toàn cục — TAP KT-46 đọc để chứng minh fault **thật**.
#[derive(Debug)]
pub struct GuardStats {
    faults: AtomicU64,
    load_faults: AtomicU64,
    store_faults: AtomicU64,
    recovered: AtomicU64,
    fatal: AtomicU64,
    last_scause: AtomicU64,
    last_stval: AtomicU64,
    last_sepc: AtomicU64,
}

impl GuardStats {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            faults: AtomicU64::new(0),
            load_faults: AtomicU64::new(0),
            store_faults: AtomicU64::new(0),
            recovered: AtomicU64::new(0),
            fatal: AtomicU64::new(0),
            last_scause: AtomicU64::new(0),
            last_stval: AtomicU64::new(0),
            last_sepc: AtomicU64::new(0),
        }
    }

    /// Ghi một guard fault; `tf` là trapframe **tại thời điểm trap** (trước khi resume).
    pub fn record(&self, tf: &TrapFrame, recovered: bool) {
        self.faults.fetch_add(1, Ordering::Relaxed);
        match tf.scause {
            EXC_LOAD_PAGE_FAULT => {
                self.load_faults.fetch_add(1, Ordering::Relaxed);
            }
            EXC_STORE_PAGE_FAULT => {
                self.store_faults.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        if recovered {
            self.recovered.fetch_add(1, Ordering::Relaxed);
        } else {
            self.fatal.fetch_add(1, Ordering::Relaxed);
        }
        self.last_scause.store(tf.scause, Ordering::Relaxed);
        self.last_stval.store(tf.stval, Ordering::Relaxed);
        self.last_sepc.store(tf.sepc, Ordering::Relaxed);
    }

    /// Đọc nhất quán (một hart, KT-46).
    pub fn snapshot(&self) -> GuardSnapshot {
        GuardSnapshot {
            faults: self.faults.load(Ordering::Relaxed),
            load_faults: self.load_faults.load(Ordering::Relaxed),
            store_faults: self.store_faults.load(Ordering::Relaxed),
            recovered: self.recovered.load(Ordering::Relaxed),
            fatal: self.fatal.load(Ordering::Relaxed),
            last_scause: self.last_scause.load(Ordering::Relaxed),
            last_stval: self.last_stval.load(Ordering::Relaxed),
            last_sepc: self.last_sepc.load(Ordering::Relaxed),
        }
    }

    /// Xoá bộ đếm — chỉ dùng trong test host.
    pub fn reset(&self) {
        for field in [
            &self.faults,
            &self.load_faults,
            &self.store_faults,
            &self.recovered,
            &self.fatal,
            &self.last_scause,
            &self.last_stval,
            &self.last_sepc,
        ] {
            field.store(0, Ordering::Relaxed);
        }
    }
}

impl Default for GuardStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm guard page fault của ảnh kernel (một hart ở KT-46).
pub static GUARD_STATS: GuardStats = GuardStats::new();

/// In các thanh ghi chính của trapframe (dùng chung cho dump fatal/guard/interrupt).
fn dump_regs<P: Platform + ?Sized>(w: &mut PlatformWriter<'_, P>, tf: &TrapFrame) {
    let _ = writeln!(
        w,
        "# regs ra=0x{:x} sp=0x{:x} gp=0x{:x} tp=0x{:x}",
        tf.reg(REG_RA),
        tf.reg(REG_SP),
        tf.reg(REG_GP),
        tf.reg(REG_TP)
    );
    let _ = writeln!(
        w,
        "# regs a0=0x{:x} a1=0x{:x} a2=0x{:x} a3=0x{:x} a4=0x{:x} a5=0x{:x} a6=0x{:x} a7=0x{:x}",
        tf.arg(0),
        tf.arg(1),
        tf.arg(2),
        tf.arg(3),
        tf.arg(4),
        tf.arg(5),
        tf.arg(6),
        tf.arg(7)
    );
}

/// In dump fatal (bắt đầu bằng `PANIC`) rồi `DebugHalt(1)`.
///
/// Runner coi dòng bắt đầu `PANIC` là đỏ (D2-08). Dump gồm `scause/sepc/stval/sstatus` và các
/// thanh ghi chính: `ra/sp/gp/tp` + `a0..a7` — đủ để lần lại `hl`/`hq` mà không cần thêm stack.
pub fn report_fatal<P: Platform + ?Sized>(platform: &P, exc: &Exception, tf: &TrapFrame) -> ! {
    platform.put_str("PANIC: ");
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "trap {} scause=0x{:x} sepc=0x{:x} stval=0x{:x} sstatus=0x{:x}",
            exc.name(),
            exc.scause,
            exc.sepc,
            exc.stval,
            tf.sstatus
        );
        dump_regs(&mut w, tf);
    }
    platform.halt(1)
}

/// In **guard page fault** (bắt đầu bằng `PANIC: guard page fault`) rồi `DebugHalt(1)` — KT-46.
///
/// Chính sách mặc định của guard page là fatal: dump `scause/stval/sepc/sstatus` + thanh ghi rồi
/// halt sạch, không treo/im lặng. TAP của ảnh `vifast-kbin-ktest-guard` đòi **đúng** tiền tố này
/// cho store fault `scause=0xf` (load fault `scause=0xd` được probe có cờ resume để kiểm cả hai
/// loại fault trong một boot — xem `vifast-plat-riscv64::mmu`).
pub fn report_guard_fault<P: Platform + ?Sized>(
    platform: &P,
    exc: &Exception,
    tf: &TrapFrame,
) -> ! {
    platform.put_str("PANIC: ");
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "guard page fault scause=0x{:x} stval=0x{:x} sepc=0x{:x} sstatus=0x{:x}",
            exc.scause, exc.stval, exc.sepc, tf.sstatus
        );
        dump_regs(&mut w, tf);
    }
    platform.halt(1)
}

/// In dump ngắt bất ngờ (bắt đầu bằng `PANIC`) rồi `DebugHalt(1)`.
///
/// KT-45 chưa bật timer (KT-47) hay PLIC (KT-48), nên mọi interrupt vào `stvec` là bất thường.
pub fn report_interrupt<P: Platform + ?Sized>(platform: &P, code: u64, tf: &TrapFrame) -> ! {
    platform.put_str("PANIC: ");
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "unexpected interrupt code={code} scause=0x{:x} sepc=0x{:x} stval=0x{:x} sstatus=0x{:x}",
            tf.scause, tf.sepc, tf.stval, tf.sstatus
        );
    }
    platform.halt(1)
}

// Đối chiếu bố cục trapframe với asm: asm dùng các `TF_*_OFFSET` ở trên làm `const` operand,
// nên nếu `#[repr(C)]` đổi thứ tự/kiểu, build **dừng** ở đây thay vì hỏng lúc chạy (rủi ro
// "sai offset trapframe" của K3-PLAN §4.1).
const _: () = {
    assert!(core::mem::offset_of!(TrapFrame, x) == 0);
    assert!(core::mem::offset_of!(TrapFrame, sepc) == TF_SEPC_OFFSET);
    assert!(core::mem::offset_of!(TrapFrame, sstatus) == TF_SSTATUS_OFFSET);
    assert!(core::mem::offset_of!(TrapFrame, scause) == TF_SCAUSE_OFFSET);
    assert!(core::mem::offset_of!(TrapFrame, stval) == TF_STVAL_OFFSET);
    assert!(core::mem::size_of::<TrapFrame>() == TF_SIZE);
    assert!(core::mem::align_of::<TrapFrame>() == 8);
    assert!(core::mem::offset_of!(TrapFrame, x) + 8 * REG_SP == 16);
    assert!(core::mem::offset_of!(TrapFrame, x) + 8 * REG_A0 == 80);
};
