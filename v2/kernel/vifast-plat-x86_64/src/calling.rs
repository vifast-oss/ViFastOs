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
//! # Trạng thái: **đã mở** ở `spec/abi.toml` (D2-207 / KT-270)
//!
//! `spec/abi.toml` `[calling.x86_64]` đã mở ở **KT-270** (D2-207): `rax` = số hiệu + error,
//! `rdx` = MessageInfo + value, `rdi`/`rsi` = tham số, `r10`/`r8`/`r9`/`r12` = 4 MR, chuyển bằng
//! **`int 0x80`** (IDT vector `0x80`, gate `DPL = 3`) — xem `spec/abi.toml` và
//! `v2/docs/k5-x86_64-boot.md` §10. Bảng dưới đây là **cùng** quy ước, giữ làm dữ liệu host-test;
//! nguồn sự thật vẫn là `spec/` (D2-11). `SYSCALL_MSRS`/`SYSCALL_CLOBBERED` mô tả đường
//! `syscall`/`sysret` **chưa dùng** (bước sau); đường đang chạy là `int 0x80` với
//! [`INT80_CLOBBERED`].
//!
//! # Một hợp đồng thanh ghi cho **cả** `int 0x80` và `syscall`/`sysret` (D2-242 nhóm 7)
//!
//! `syscall` clobber `RCX`/`R11`, nên sau khi dùng `RAX` (số hiệu), `RDI`/`RSI` (hai tham số),
//! `RDX` (MessageInfo) chỉ còn **ba** thanh ghi caller-saved (`R8`, `R9`, `R10`) cho **bốn** MR.
//! Dùng `R12` làm MR3: `R12` là callee-saved theo SysV, nên stub syscall (một hàm C/asm bình
//! thường) vẫn giữ được nó; quy ước **syscall** là hợp đồng riêng, không phải quy ước gọi hàm.
//!
//! **D2-242 nhóm 7 = `7C-r13`** chốt **một** hợp đồng cho cả hai đường vào ring 0: MR vẫn là
//! `R10/R8/R9/R12`, `return_message_info = R13` (không phải `R11` — CPU ghi `RFLAGS` vào `R11`
//! khi bật `syscall`/`sysret` ở Chu kỳ 3), `RCX`/`R11` **hoàn toàn trống cho CPU**. Vì vậy
//! [`INT80_MR_REGS`] **bằng** [`SYSCALL_MR_REGS`] — không còn "phương án thay thế" dùng `RCX`.

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
/// Bốn MR trên đường `int 0x80`: **bằng** [`SYSCALL_MR_REGS`] — `R10`, `R8`, `R9`, `R12`
/// (D2-242 nhóm 7: một hợp đồng thanh ghi duy nhất cho `int 0x80` và `syscall`/`sysret`).
pub const INT80_MR_REGS: [Reg; 4] = [Reg::R10, Reg::R8, Reg::R9, Reg::R12];
/// Kênh trả `MessageInfo` trên đường `int 0x80`: **`R13`** — **D2-242 nhóm 7 = `7C-r13`**.
///
/// Khớp [`SYSCALL_RETURN_MSGINFO_REG`] (cùng `R13`) ⇒ **một** hợp đồng thanh ghi cho cả
/// `int 0x80` lẫn `syscall`/`sysret`; `RCX`/`R11` để trống cho CPU (`syscall` ghi RIP vào `RCX`,
/// RFLAGS vào `R11`). `vifast_isr_syscall` đã lưu/khôi phục đủ 15 GPR nên `R13` an toàn.
pub const INT80_RETURN_MSGINFO_REG: Reg = Reg::R13;
/// Thanh ghi **syscall được phép đổi** trên đường `int 0x80` — khớp `spec/abi.toml`
/// `[calling.x86_64].clobbered` (**D2-242 nhóm 7**): `rax` = error, `rdx` = value,
/// `r13` = MessageInfo trả về, `r10/r8/r9/r12` = MR0..MR3.
///
/// Khác [`SYSCALL_CLOBBERED`] (`RCX`/`R11` bị **lệnh** `syscall` clobber): `int 0x80` không clobber
/// `RCX`/`R11`, và stub của ảnh khôi phục **mọi** GPR khác từ frame trước `iretq`.
pub const INT80_CLOBBERED: [Reg; 7] = [
    Reg::Rax,
    Reg::Rdx,
    Reg::R13,
    Reg::R10,
    Reg::R8,
    Reg::R9,
    Reg::R12,
];

/// MSR phải nạp trước khi dùng `syscall`/`sysret` (bước M7): `STAR`, `LSTAR`, `FMASK`.
pub const SYSCALL_MSRS: [u32; 3] = [
    crate::control::MSR_IA32_STAR,
    crate::control::MSR_IA32_LSTAR,
    crate::control::MSR_IA32_FMASK,
];
