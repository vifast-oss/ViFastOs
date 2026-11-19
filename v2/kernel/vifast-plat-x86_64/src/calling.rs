// SPDX-License-Identifier: MIT OR Apache-2.0
//! Quy ước gọi hàm + syscall của x86_64 — KT-186 / D2-163 (K5' bước M2).
//!
//! Tệp này **thuần dữ liệu**: bảng thanh ghi của **System V AMD64 ABI** (gọi hàm C/user) và
//! **đề xuất** ánh xạ syscall của ViFastOs. **Không** asm, **không** `unsafe`; bước M7 mới viết
//! stub `syscall`/`sysret` + `swapgs` (crate arch, D2-17).
//!
//! # Nguồn
//!
//! - **System V Application Binary Interface — AMD64 Architecture Processor Supplement**
//!   (x86-64 psABI), §3.2 "Function Calling Sequence": §3.2.1 "Registers" (thanh ghi
//!   callee-saved: `RBX`, `RBP`, `R12`–`R15`; `DF` phải bằng 0 khi vào/ra hàm; `MXCSR` control
//!   bit callee-saved), §3.2.2 "The Stack Frame" (căn 16 byte trước `call`; `(RSP + 8)` là bội
//!   của 16 khi vào hàm; red zone 128 byte), §3.2.3 "Parameter Passing" (tham số nguyên
//!   `RDI, RSI, RDX, RCX, R8, R9`; trả về `RAX, RDX`; vector `XMM0`–`XMM7` vào, `XMM0`/`XMM1`
//!   ra; `AL` = số vector register cho hàm variadic).
//! - **Intel SDM Vol 2B**, mục "SYSCALL—Fast System Call" / "SYSRET—Return From Fast System
//!   Call": `syscall` nạp `RIP` từ `IA32_LSTAR`, lưu `RIP` kế tiếp vào `RCX` và `RFLAGS` vào
//!   `R11`; `sysret` lấy `RIP` từ `RCX`, `RFLAGS` từ `R11`. Vì vậy `RCX`/`R11` **không dùng**
//!   làm thanh ghi vào của syscall.
//! - **Intel SDM Vol 2B**, mục "INT n/INTO/INT3/INT1—Call to Interrupt Procedure": `int 0x80`
//!   không clobber `RCX`/`R11`, nhưng đi qua IDT và chậm hơn `syscall`.
//! - **Intel SDM Vol 3A**, mục "Extended Feature Enable Register (IA32_EFER)" (`SCE` bit 0) và
//!   **Vol 4** (`IA32_STAR`/`IA32_LSTAR`/`IA32_FMASK`).
//!
//! # Trạng thái: **đề xuất**, chưa phải ABI
//!
//! `spec/abi.toml` `[calling.x86_64]` vẫn `reserved = true` (D2-163). Bảng syscall dưới đây là
//! **đề xuất** cho bước M7, chưa được Agent 0 duyệt; mở `spec/` **một lần** theo khuôn KT-05 tại
//! M7. Bản nháp đầy đủ + đối chiếu `[calling.riscv64]`:
//! `v2/agent/reports/K5-X86-CALLING-DRAFT.md`.
//!
//! # Vì sao MR3 là `R12` (callee-saved)
//!
//! `syscall` clobber `RCX`/`R11`, nên sau khi dùng `RAX` (số hiệu), `RDI`/`RSI` (hai tham số),
//! `RDX` (MessageInfo) chỉ còn **ba** thanh ghi caller-saved (`R8`, `R9`, `R10`) cho **bốn** MR.
//! Đề xuất dùng `R12` làm MR3: `R12` là callee-saved theo SysV, nên stub syscall (một hàm C/asm
//! bình thường) vẫn giữ được nó; quy ước **syscall** là hợp đồng riêng, không phải quy ước gọi
//! hàm. Phương án thay thế `int 0x80` (không clobber `RCX`/`R11`) dùng `RCX, R8, R9, R10` — xem
//! [`INT80_MR_REGS`]; đánh đổi ở bản nháp §4.

/// Một thanh ghi đa dụng 64-bit, giá trị `repr(u8)` = **mã hoá phần cứng** (0..15).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    /// `RAX` — mã 0; số hiệu syscall / giá trị trả về (lỗi).
    Rax = 0,
    /// `RCX` — mã 1; `syscall` ghi RIP kế tiếp vào đây.
    Rcx = 1,
    /// `RDX` — mã 2; tham số thứ ba SysV / MessageInfo / giá trị trả về.
    Rdx = 2,
    /// `RBX` — mã 3; callee-saved.
    Rbx = 3,
    /// `RSP` — mã 4; con trỏ ngăn xếp.
    Rsp = 4,
    /// `RBP` — mã 5; callee-saved / frame pointer.
    Rbp = 5,
    /// `RSI` — mã 6; tham số thứ hai SysV.
    Rsi = 6,
    /// `RDI` — mã 7; tham số thứ nhất SysV.
    Rdi = 7,
    /// `R8` — mã 8; tham số thứ năm SysV.
    R8 = 8,
    /// `R9` — mã 9; tham số thứ sáu SysV.
    R9 = 9,
    /// `R10` — mã 10; caller-saved.
    R10 = 10,
    /// `R11` — mã 11; `syscall` ghi RFLAGS vào đây.
    R11 = 11,
    /// `R12` — mã 12; callee-saved.
    R12 = 12,
    /// `R13` — mã 13; callee-saved.
    R13 = 13,
    /// `R14` — mã 14; callee-saved.
    R14 = 14,
    /// `R15` — mã 15; callee-saved.
    R15 = 15,
}

impl Reg {
    /// Mã hoá phần cứng (0..15) — dùng cho encoder/decoder asm ở bước M7.
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// Tên AT&T/Intel của thanh ghi.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rax => "RAX",
            Self::Rcx => "RCX",
            Self::Rdx => "RDX",
            Self::Rbx => "RBX",
            Self::Rsp => "RSP",
            Self::Rbp => "RBP",
            Self::Rsi => "RSI",
            Self::Rdi => "RDI",
            Self::R8 => "R8",
            Self::R9 => "R9",
            Self::R10 => "R10",
            Self::R11 => "R11",
            Self::R12 => "R12",
            Self::R13 => "R13",
            Self::R14 => "R14",
            Self::R15 => "R15",
        }
    }
}

/// Tham số nguyên của SysV AMD64 ABI, đúng thứ tự (psABI §3.2.3).
pub const SYSV_ARG_REGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9];
/// Thanh ghi trả về nguyên của SysV AMD64 ABI (psABI §3.2.3 "Returning of Values").
pub const SYSV_RETURN_REGS: [Reg; 2] = [Reg::Rax, Reg::Rdx];
/// Thanh ghi **callee-saved** của SysV AMD64 ABI (psABI §3.2.1) — `RSP` là ngăn xếp, không tính.
pub const SYSV_CALLEE_SAVED: [Reg; 6] =
    [Reg::Rbx, Reg::Rbp, Reg::R12, Reg::R13, Reg::R14, Reg::R15];
/// Thanh ghi **caller-saved** (scratch) của SysV AMD64 ABI.
pub const SYSV_CALLER_SAVED: [Reg; 9] = [
    Reg::Rax,
    Reg::Rcx,
    Reg::Rdx,
    Reg::Rsi,
    Reg::Rdi,
    Reg::R8,
    Reg::R9,
    Reg::R10,
    Reg::R11,
];
/// Căn ngăn xếp SysV: `(RSP + 8)` là bội của 16 khi vào hàm (psABI §3.2.2).
pub const SYSV_STACK_ALIGN_BYTES: u64 = 16;
/// Red zone SysV: 128 byte dưới `RSP` (psABI §3.2.2) — **kernel không được dùng**.
pub const SYSV_RED_ZONE_BYTES: u64 = 128;
/// Số thanh ghi vector (`XMM0`–`XMM7`) nhận tham số SSE (psABI §3.2.3).
pub const SYSV_VECTOR_ARG_REGS: u8 = 8;
/// Số thanh ghi vector trả về (`XMM0`, `XMM1`).
pub const SYSV_VECTOR_RETURN_REGS: u8 = 2;
/// Hàm variadic: `AL` = số thanh ghi vector đã dùng (psABI §3.2.4).
pub const SYSV_VARIADIC_VECTOR_COUNT_REG: Reg = Reg::Rax;
/// `DF` phải bằng 0 khi vào và khi ra hàm (psABI §3.2.1).
pub const SYSV_DIRECTION_FLAG_CLEAR: bool = true;
/// Con trỏ ngăn xếp.
pub const SYSV_STACK_POINTER: Reg = Reg::Rsp;
/// Frame pointer.
pub const SYSV_FRAME_POINTER: Reg = Reg::Rbp;

/// **Đề xuất** số hiệu syscall của ViFastOs trên x86_64: `RAX`.
pub const SYSCALL_NUMBER_REG: Reg = Reg::Rax;
/// **Đề xuất** hai tham số syscall (`cap`, `reply_cptr`/`arg1`): `RDI`, `RSI`.
pub const SYSCALL_ARG_REGS: [Reg; 2] = [Reg::Rdi, Reg::Rsi];
/// **Đề xuất** `MessageInfo`: `RDX`.
pub const SYSCALL_MSGINFO_REG: Reg = Reg::Rdx;
/// **Đề xuất** bốn MR trong thanh ghi: `R10`, `R8`, `R9`, `R12` (xem đầu tệp).
pub const SYSCALL_MR_REGS: [Reg; 4] = [Reg::R10, Reg::R8, Reg::R9, Reg::R12];
/// **Đề xuất** kênh trả lỗi: `RAX` (0 = thành công).
pub const SYSCALL_RETURN_ERROR_REG: Reg = Reg::Rax;
/// **Đề xuất** kênh trả giá trị: `RDX`.
pub const SYSCALL_RETURN_VALUE_REG: Reg = Reg::Rdx;
/// **Đề xuất** kênh trả `MessageInfo` cho `Recv`/`ReplyRecv`: `R13`.
pub const SYSCALL_RETURN_MSGINFO_REG: Reg = Reg::R13;
/// Thanh ghi `syscall` **tự động clobber**: `RCX` (RIP kế tiếp), `R11` (RFLAGS).
pub const SYSCALL_CLOBBERED: [Reg; 2] = [Reg::Rcx, Reg::R11];
/// Vector IDT của `int 0x80` (đường syscall thay thế).
pub const INT80_VECTOR: u8 = 0x80;
/// **Đề xuất** bốn MR nếu dùng `int 0x80` (không clobber `RCX`/`R11`): `RCX`, `R8`, `R9`, `R10`.
pub const INT80_MR_REGS: [Reg; 4] = [Reg::Rcx, Reg::R8, Reg::R9, Reg::R10];
/// **Đề xuất** kênh trả `MessageInfo` nếu dùng `int 0x80`: `R11`.
pub const INT80_RETURN_MSGINFO_REG: Reg = Reg::R11;

/// MSR phải nạp trước khi dùng `syscall`/`sysret` (bước M7): `STAR`, `LSTAR`, `FMASK`.
pub const SYSCALL_MSRS: [u32; 3] = [
    crate::control::MSR_IA32_STAR,
    crate::control::MSR_IA32_LSTAR,
    crate::control::MSR_IA32_FMASK,
];
