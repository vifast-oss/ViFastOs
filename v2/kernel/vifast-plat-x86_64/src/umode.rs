// SPDX-License-Identifier: MIT OR Apache-2.0
//! **U-mode (ring 3) trên x86_64** — dữ liệu + helper thuần cho K5' bước 4 (KT-270, D2-207).
//!
//! # Vì sao tệp này **không** có `unsafe` và **không** asm
//!
//! Crate giữ `#![deny(unsafe_code)]`; `unsafe` chỉ ở [`crate::asm`] (D2-17). Tệp này là **dữ
//! liệu** mà cả asm lẫn handler dùng:
//!
//! - **VA** của trang mã ring 3 và trang stack user ([`USER_CODE_VA`], [`USER_STACK_VA`]) — hai
//!   VA **canonical nửa thấp** nằm ngoài identity map 1 GiB và ngoài MMIO map 3..4 GiB của ảnh;
//! - **selector** user code/data ([`USER_CS`], [`USER_SS`]) — lấy từ [`crate::gdt`] (KT-261), không
//!   chép tay `0x1B`/`0x23`;
//! - **`RFLAGS`** cho `iretq` xuống ring 3 ([`USER_RFLAGS`]) — `IF = 1` (bit 9) + bit 1 luôn 1;
//! - **bố cục frame** của stub `int 0x80` ([`SYSCALL_FRAME_*`]) — khớp **tuyệt đối** với thứ tự
//!   `push` của stub trong [`crate::asm`] (test host chốt);
//! - **`SyscallRet`** — kiểu trả của handler `syscall_dispatch` (hai `u64` theo SysV AMD64:
//!   `rax` = error, `rdx` = value).
//!
//! Nhờ vậy toàn bộ hình học U-mode chạy được trên host (`tests/umode.rs`), kể cả **đột biến
//! `cs` để ring 0** (đột biến (i) của KT-270): nó làm [`is_user_cs`] sai ngay ở tầng dữ liệu.
//!
//! # Hợp đồng với `spec/abi.toml`
//!
//! Quy ước gọi (`rax` = số hiệu + error, `rdx` = MessageInfo + value, `rdi`/`rsi` = tham số,
//! `r10`/`r8`/`r9`/`r12` = 4 MR, chuyển bằng `int 0x80`) là **nguồn sự thật** ở `spec/abi.toml`
//! `[calling.x86_64]` (D2-207) và được `vifast-gen` sinh thành hằng trong
//! `vifast_abi::generated` (`X86_64_*`). Ảnh dùng hằng **sinh** cho **số hiệu syscall**
//! (`SYS_DEBUG_PUT_STR`, `SYS_EXIT`); tên thanh ghi trong asm của ảnh là chỗ duy nhất còn viết
//! tay — hành vi được đo bằng TAP 12/13/14 trên QEMU (sai thanh ghi ⇒ số hiệu đọc ra 0 ⇒ TAP
//! `not ok`), không thể chứng minh bằng test host.

use crate::exc;
use crate::gdt;

/// VA **canonical nửa thấp** của trang mã ring 3 (map `US = 1`, chỉ đọc + exec).
///
/// Chọn `0x0000_0040_0000_0000` (256 GiB): ngoài identity map 1 GiB của ảnh, ngoài MMIO map
/// `0xC000_0000..0x1_0000_0000`, và **canonical nửa thấp** (bit 47 = 0, bit 63..48 = 0) nên đi
/// qua `PML4[0]` — cùng gốc với identity map nhưng `PDPT[256]` là một cây riêng (PML4[0] đã
/// `US = 1`; identity map vẫn supervisor vì `PDPT[0]` giữ `US = 0`).
pub const USER_CODE_VA: u64 = 0x0000_0040_0000_0000;

/// Cỡ trang mã ring 3 — **một** trang 4 KiB (linker `ASSERT` section `.user` vừa một trang).
///
/// **KT-304** dùng hằng này để [`crate::sched::switch_frame_ok`] kiểm `rip` của frame ring 3 nằm
/// trong trang mã user: chạy mã kernel ở CPL3 là fault ngay lệnh đầu.
pub const USER_CODE_BYTES: u64 = 0x1000;

/// VA **canonical nửa thấp** của trang stack ring 3 (map `US = 1`, đọc + ghi, `NX = 1`).
pub const USER_STACK_VA: u64 = 0x0000_0040_0000_1000;

/// Cỡ trang stack user — **một** trang 4 KiB (đủ cho stub ring 3 của bước này, không phải stack
/// đầy đủ).
pub const USER_STACK_BYTES: u64 = 0x1000;

/// Đỉnh stack user = `RSP` lúc `iretq` (stack mọc xuống).
pub const USER_STACK_TOP: u64 = USER_STACK_VA + USER_STACK_BYTES;

// ---------------------------------------------------------------------------------------------
// **KT-340b** — cửa sổ VA của tác vụ ring 3 chạy trong `VSpace` **động**
// ---------------------------------------------------------------------------------------------
//
// `USER_CODE_VA`/`USER_STACK_VA` ở trên là cây user **tĩnh** của ảnh (`PDPT[256]`, VA 256 GiB)
// — đường của KT-270 và scheduler KT-304, **không đổi**. `VSpace` động dựng cây riêng bằng
// `vifast_pte::PageTable`, và walker áp **chính sách VA canonical 39-bit** của format x86_64
// (`X86_64_VA_BITS = 39` — D2-147): VA 256 GiB có bit 38 = 1 nhưng bit 39..63 = 0 nên **không**
// canonical theo chính sách đó (phần cứng 48-bit vẫn nhận, nhưng `PageTable::map` từ chối —
// đúng, vì kernel chọn cửa sổ 39-bit). Vì vậy tác vụ của `VSpace` dùng **base 64 GiB**
// (`0x0000_0010_0000_0000`, bit 38 = 0, `PDPT[64]`) — trong cửa sổ 39-bit, ngoài identity map
// `0..1 GiB` và ngoài MMIO `2..4 GiB`, nên không đè mapping nào của ảnh.
//
// Bố cục **cố định** của một tác vụ (mỗi trang 4 KiB):
//
// ```text
//   base + 0x0000   mã        USER_RX  (P|US, không W, không NX)
//   base + 0x1000   stack     USER_RW  (P|RW|US|NX)   — RSP bắt đầu ở base + 0x2000
//   base + 0x2000   guard     KHÔNG map (đỉnh stack chạm là fault, không tràn sang trang khác)
//   base + 0x3000   peer      của **tác vụ khác**: VSpace này KHÔNG map ⇒ ghi vào là #PF
//   base + 0x4000   probe     trang đo `INVLPG` (map → đọc → unmap → map → đọc)
// ```

/// Base VA của tác vụ ring 3 trong `VSpace` **động** — 64 GiB, canonical 39-bit (xem khối trên).
pub const VSPACE_BASE: u64 = 0x0000_0010_0000_0000;
/// VA trang **mã** (RX) của tác vụ trong `VSpace` động.
pub const VSPACE_CODE_VA: u64 = VSPACE_BASE;
/// VA trang **stack** (RW, NX) của tác vụ trong `VSpace` động.
pub const VSPACE_STACK_VA: u64 = VSPACE_BASE + 0x1000;
/// Đỉnh stack user (`RSP` lúc `iretq`) — trang `base+0x2000` là **guard** (không map).
pub const VSPACE_STACK_TOP: u64 = VSPACE_BASE + 0x2000;
/// VA "vùng của **tác vụ khác**": chỉ `VSpace` của tác vụ kia map trang này. Tác vụ đang chạy
/// ghi vào đây ⇒ `#PF` (`P = 0`, `W = 1`, `U = 1` ⇒ `err = 0x6`) — bằng chứng cô lập bộ nhớ.
pub const VSPACE_PEER_VA: u64 = VSPACE_BASE + 0x3000;
/// VA trang đo `INVLPG`: map frame A → đọc; unmap; map frame B → đọc. Thiếu `INVLPG` ⇒ đọc ra
/// dữ liệu **cũ** của frame A (đột biến `no-invlpg`).
pub const VSPACE_PROBE_VA: u64 = VSPACE_BASE + 0x4000;

/// Selector **user code** dùng thật (`0x1B` = `0x18 | RPL 3`) — lấy từ GDT của KT-261.
pub const USER_CS: u16 = gdt::SELECTOR_USER_CODE_RPL3;

/// Selector **user data** dùng thật (`0x23` = `0x20 | RPL 3`) — lấy từ GDT của KT-261.
pub const USER_SS: u16 = gdt::SELECTOR_USER_DATA_RPL3;

/// Selector **kernel code** (`0x08`) — `CS` của kernel, dùng để nhận ra "không hạ đặc quyền".
pub const KERNEL_CS: u16 = gdt::SELECTOR_KERNEL_CODE;

/// Selector **kernel data** (`0x10`) — `SS` của kernel.
pub const KERNEL_DS: u16 = gdt::SELECTOR_KERNEL_DATA;

/// `RFLAGS` lúc `iretq` xuống ring 3: `IF = 1` (bit 9) + bit 1 (luôn 1) = `0x202`.
///
/// `IOPL = 0`, `NT = 0`, `TF = 0` — ring 3 **không** được `in`/`out` (TSS `I/O map base = 104`
/// cũng chặn), không single-step, không task switch.
pub const USER_RFLAGS: u64 = 0x202;

/// Bit `IF` của `RFLAGS` (ngắt che được).
pub const RFLAGS_IF: u64 = 1 << 9;

/// `true` ⇔ `cs` là selector **user code** (`0x1B`) — điều kiện "đang thật sự ở ring 3".
pub const fn is_user_cs(cs: u64) -> bool {
    cs == USER_CS as u64
}

/// `true` ⇔ `cs` là selector **kernel code** (`0x08`).
pub const fn is_kernel_cs(cs: u64) -> bool {
    cs == KERNEL_CS as u64
}

/// `true` ⇔ `rflags` có `IF = 1`.
pub const fn rflags_if(rflags: u64) -> bool {
    rflags & RFLAGS_IF != 0
}

// ---------------------------------------------------------------------------------------------
// Bố cục frame của stub `int 0x80` — asm của crate dùng làm `const` operand
// ---------------------------------------------------------------------------------------------
//
// `int 0x80` từ CPL3 đẩy `SS`, `RSP`, `RFLAGS`, `CS`, `RIP` (5 qword) rồi stub đẩy `err = 0`,
// `vector = 0x80`, và **15 GPR** — cùng bố cục với frame exception chung (`crate::exc::FRAME_*`),
// nên handler đọc `nr`/tham số theo các offset dưới đây.
//
// **Thứ tự `push` quyết định offset**: stub đẩy `rax rcx rdx rbx rbp rsi rdi r8 r9 r10 r11 r12
// r13 r14 r15` — `r15` đẩy **cuối** nên nằm ở offset **0**, `rax` đẩy **đầu** nên nằm ở offset
// **112**. Đây là bẫy đã đo được ở lần chạy đầu KT-270: đọc `nr` ở offset 0 ra `r15` (địa chỉ
// entry) thay vì `rax` (số hiệu 12) — TAP 12 vẫn xanh nhưng `nr` sai; test host chốt từng offset.

/// Ô `r15` trong frame (đẩy **cuối** ⇒ offset 0).
pub const SYSCALL_FRAME_R15: usize = 0;
/// Ô `r14` trong frame.
pub const SYSCALL_FRAME_R14: usize = 8;
/// Ô `r13` trong frame.
pub const SYSCALL_FRAME_R13: usize = 16;
/// Ô `r12` (MR3) trong frame.
pub const SYSCALL_FRAME_R12: usize = 24;
/// Ô `r11` trong frame.
pub const SYSCALL_FRAME_R11: usize = 32;
/// Ô `r10` (MR0) trong frame.
pub const SYSCALL_FRAME_R10: usize = 40;
/// Ô `r9` (MR2) trong frame.
pub const SYSCALL_FRAME_R9: usize = 48;
/// Ô `r8` (MR1) trong frame.
pub const SYSCALL_FRAME_R8: usize = 56;
/// Ô `rdi` (tham số 1) trong frame.
pub const SYSCALL_FRAME_RDI: usize = 64;
/// Ô `rsi` (tham số 2) trong frame.
pub const SYSCALL_FRAME_RSI: usize = 72;
/// Ô `rbp` trong frame.
pub const SYSCALL_FRAME_RBP: usize = 80;
/// Ô `rbx` trong frame.
pub const SYSCALL_FRAME_RBX: usize = 88;
/// Ô `rdx` (MessageInfo vào, value ra) trong frame.
pub const SYSCALL_FRAME_RDX: usize = 96;
/// Ô `rcx` trong frame.
pub const SYSCALL_FRAME_RCX: usize = 104;
/// Ô `rax` (số hiệu syscall vào, error ra) trong frame — đẩy **đầu** ⇒ offset 112.
pub const SYSCALL_FRAME_RAX: usize = 112;
/// Ô `vector` (stub đẩy `0x80`) trong frame.
pub const SYSCALL_FRAME_VECTOR: usize = exc::FRAME_VECTOR_OFFSET;
/// Ô mã lỗi (stub đẩy `0`) trong frame.
pub const SYSCALL_FRAME_ERROR: usize = exc::FRAME_ERROR_OFFSET;
/// Ô `rip` (CPU đẩy — **lệnh kế tiếp** `int 0x80`) trong frame.
pub const SYSCALL_FRAME_RIP: usize = exc::FRAME_RIP_OFFSET;
/// Ô `cs` (CPU đẩy — `0x1B` nếu vào từ ring 3) trong frame.
pub const SYSCALL_FRAME_CS: usize = exc::FRAME_CS_OFFSET;
/// Ô `rflags` (CPU đẩy) trong frame.
pub const SYSCALL_FRAME_RFLAGS: usize = exc::FRAME_RFLAGS_OFFSET;
/// Ô `rsp` user (CPU đẩy khi đổi CPL) trong frame.
pub const SYSCALL_FRAME_RSP: usize = exc::FRAME_RFLAGS_OFFSET + 8;
/// Ô `ss` user (CPU đẩy khi đổi CPL) trong frame.
pub const SYSCALL_FRAME_SS: usize = exc::FRAME_RFLAGS_OFFSET + 16;

/// Kết quả một syscall — **hai** `u64` theo SysV AMD64: `rax` = error, `rdx` = value.
///
/// `#[repr(C)]` + hai trường `u64` ⇒ SysV phân loại hai eightbyte INTEGER và trả ở `rax:rdx`
/// (psABI §3.2.3 "Returning of Values"). Stub asm ghi hai giá trị này vào ô `rax`/`rdx` của
/// frame trước `iretq`, nên ring 3 đọc đúng `[calling.x86_64].return_error`/`return_value`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyscallRet {
    /// `0` = thành công, khác `0` = mã lỗi (1..19).
    pub error: u64,
    /// Giá trị trả (với `DebugPutStr` = số byte đã in).
    pub value: u64,
}

impl SyscallRet {
    /// Kết quả thành công với `value`.
    pub const fn ok(value: u64) -> Self {
        Self { error: 0, value }
    }

    /// Kết quả lỗi với mã `error` (1..19).
    pub const fn err(error: u64) -> Self {
        Self { error, value: 0 }
    }
}
