// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Dữ liệu exception x86_64** — KT-261 (K5' bước 2): vector, mã lỗi, slot stub, bố cục frame.
//!
//! Tệp này **thuần dữ liệu** (không asm, không `unsafe`): nó chốt
//!
//! - **vector** của các exception bước này xử lý (`#DE` 0, `#DB` 1, `#BP` 3, `#UD` 6, `#DF` 8,
//!   `#GP` 13, `#PF` 14) và **tập vector có mã lỗi** (SDM Vol 3A Bảng 6-1);
//! - **slot stub** — ánh xạ vector → stub asm, để [`crate::idt::install`] đọc địa chỉ stub từ
//!   [`crate::asm::stub_addr`] mà không cần bảng 256 phần tử;
//! - **bố cục frame** trên stack sau khi stub đẩy `vector` + `mã lỗi` — asm của ảnh dùng chính các
//!   hằng ở đây làm `const` operand, nên asm và Rust **không thể lệch nhau**.
//!
//! # Vì sao "vector có mã lỗi" là **dữ liệu**
//!
//! CPU đẩy mã lỗi cho **đúng** 10 vector (8, 10, 11, 12, 13, 14, 17, 21, 29, 30). Nếu stub của
//! một vector có mã lỗi lại đẩy thêm một `0` "giả", frame lệch 8 byte và handler đọc `rip` từ chỗ
//! của `cs` — **không** lỗi biên dịch, chỉ sai khi đã có fault. Vì vậy tập này là dữ liệu có test
//! host, và stub asm chia **hai** họ: `isr_err_*` (không đẩy gì thêm) và `isr_noerr_*` (đẩy `0`).
//!
//! # Bố cục frame (sau `push` của stub, trước khi `push` 15 GPR)
//!
//! ```text
//!   rsp + 0    vector        ← stub đẩy (sau cùng)
//!   rsp + 8    mã lỗi        ← CPU đẩy, hoặc stub đẩy 0
//!   rsp + 16   rip           ← CPU đẩy
//!   rsp + 24   cs            ← CPU đẩy
//!   rsp + 32   rflags        ← CPU đẩy
//!   rsp + 40   rsp cũ        ← chỉ khi đổi CPL hoặc dùng IST
//!   rsp + 48   ss cũ         ← chỉ khi đổi CPL hoặc dùng IST
//! ```
//!
//! Stub chung đẩy **15 GPR** (120 byte) trước khi gọi handler, nên handler đọc frame qua các
//! offset [`FRAME_VECTOR_OFFSET`]…[`FRAME_RFLAGS_OFFSET`] (đã cộng 120).

/// `#DE` — divide error (không mã lỗi).
pub const VEC_DE: u8 = 0;
/// `#DB` — debug (không mã lỗi).
pub const VEC_DB: u8 = 1;
/// `#BP` — breakpoint (`int3`, không mã lỗi).
pub const VEC_BP: u8 = 3;
/// `#UD` — invalid opcode (`ud2`, không mã lỗi).
pub const VEC_UD: u8 = 6;
/// `#DF` — double fault (**có** mã lỗi, luôn 0) — vector duy nhất dùng `IST1`.
pub const VEC_DF: u8 = 8;
/// `#TS` — invalid TSS (có mã lỗi).
pub const VEC_TS: u8 = 10;
/// `#NP` — segment not present (có mã lỗi).
pub const VEC_NP: u8 = 11;
/// `#SS` — stack-segment fault (có mã lỗi).
pub const VEC_SS: u8 = 12;
/// `#GP` — general protection (có mã lỗi).
pub const VEC_GP: u8 = 13;
/// `#PF` — page fault (có mã lỗi; `CR2` = địa chỉ gây lỗi).
pub const VEC_PF: u8 = 14;
/// `#AC` — alignment check (có mã lỗi).
pub const VEC_AC: u8 = 17;
/// `#CP` — control protection (có mã lỗi).
pub const VEC_CP: u8 = 21;
/// `#VC` — VMM communication (có mã lỗi).
pub const VEC_VC: u8 = 29;
/// `#SX` — security exception (có mã lỗi).
pub const VEC_SX: u8 = 30;

/// Vector **giả** mà stub `isr_unknown` đẩy: stub dùng chung không biết vector thật, nên nó đẩy
/// một sentinel và handler in `vec=255`. Chọn `0xFF` vì nó **không** trùng vector thật nào
/// (0..31) và không trùng `0` (giá trị "chưa có" của biến đếm).
pub const VEC_UNKNOWN: u64 = 0xFF;

/// Tập vector **có mã lỗi** (SDM Vol 3A Bảng 6-1) — đúng 10 vector.
pub const ERROR_CODE_VECTORS: [u8; 10] = [
    VEC_DF, VEC_TS, VEC_NP, VEC_SS, VEC_GP, VEC_PF, VEC_AC, VEC_CP, VEC_VC, VEC_SX,
];

/// `true` ⇔ CPU đẩy mã lỗi cho `vec` (stub **không** được đẩy thêm `0`).
pub const fn has_error_code(vec: u8) -> bool {
    let mut i = 0;
    while i < ERROR_CODE_VECTORS.len() {
        if ERROR_CODE_VECTORS[i] == vec {
            return true;
        }
        i += 1;
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Slot stub — ánh xạ vector → stub asm
// ---------------------------------------------------------------------------------------------

/// Slot stub dùng chung cho mọi vector **không** có stub riêng.
pub const STUB_UNKNOWN: usize = 0;
/// Slot stub `#DE`.
pub const STUB_DE: usize = 1;
/// Slot stub `#DB`.
pub const STUB_DB: usize = 2;
/// Slot stub `#BP`.
pub const STUB_BP: usize = 3;
/// Slot stub `#UD`.
pub const STUB_UD: usize = 4;
/// Slot stub `#DF` (có mã lỗi).
pub const STUB_DF: usize = 5;
/// Slot stub `#GP` (có mã lỗi).
pub const STUB_GP: usize = 6;
/// Slot stub `#PF` (có mã lỗi).
pub const STUB_PF: usize = 7;
/// Slot stub `#TS` (có mã lỗi).
pub const STUB_TS: usize = 8;
/// Slot stub `#NP` (có mã lỗi).
pub const STUB_NP: usize = 9;
/// Slot stub `#SS` (có mã lỗi).
pub const STUB_SS: usize = 10;
/// Slot stub `#AC` (có mã lỗi).
pub const STUB_AC: usize = 11;
/// Slot stub `#CP` (có mã lỗi).
pub const STUB_CP: usize = 12;
/// Slot stub `#VC` (có mã lỗi).
pub const STUB_VC: usize = 13;
/// Slot stub `#SX` (có mã lỗi).
pub const STUB_SX: usize = 14;
/// Số slot stub — **khớp** số nhánh của `asm::stub_addr`.
pub const STUB_COUNT: usize = 15;

/// Vector → slot stub. Vector không có stub riêng rơi về [`STUB_UNKNOWN`].
///
/// **Hợp đồng với asm**: `asm::stub_addr(slot)` phải trả đúng stub của slot này, và stub phải
/// đẩy đúng vector (hoặc [`VEC_UNKNOWN`] với slot 0). Test host chốt ánh xạ; ảnh boot chốt hành
/// vi thật (`int3` ⇒ `vec=3`, …).
pub const fn stub_slot(vec: u8) -> usize {
    match vec {
        VEC_DE => STUB_DE,
        VEC_DB => STUB_DB,
        VEC_BP => STUB_BP,
        VEC_UD => STUB_UD,
        VEC_DF => STUB_DF,
        VEC_GP => STUB_GP,
        VEC_PF => STUB_PF,
        VEC_TS => STUB_TS,
        VEC_NP => STUB_NP,
        VEC_SS => STUB_SS,
        VEC_AC => STUB_AC,
        VEC_CP => STUB_CP,
        VEC_VC => STUB_VC,
        VEC_SX => STUB_SX,
        _ => STUB_UNKNOWN,
    }
}

/// Tên ngắn của vector để in log (D2-102: in từ dữ liệu, không chép chuỗi tại chỗ gọi).
pub const fn name(vec: u64) -> &'static str {
    match vec {
        0 => "de",
        1 => "db",
        3 => "bp",
        6 => "ud",
        8 => "df",
        10 => "ts",
        11 => "np",
        12 => "ss",
        13 => "gp",
        14 => "pf",
        17 => "ac",
        21 => "cp",
        29 => "vc",
        30 => "sx",
        255 => "unknown",
        _ => "other",
    }
}

// ---------------------------------------------------------------------------------------------
// Bố cục frame — asm của ảnh dùng làm `const` operand
// ---------------------------------------------------------------------------------------------

/// Số GPR mà stub chung đẩy trước khi gọi handler (`rax rcx rdx rbx rbp rsi rdi r8..r15`).
pub const FRAME_GPR_COUNT: usize = 15;
/// Offset của `vector` trong frame **sau** khi đẩy 15 GPR (15 × 8 = 120).
pub const FRAME_VECTOR_OFFSET: usize = FRAME_GPR_COUNT * 8;
/// Offset của mã lỗi.
pub const FRAME_ERROR_OFFSET: usize = FRAME_VECTOR_OFFSET + 8;
/// Offset của `rip` (CPU đẩy).
pub const FRAME_RIP_OFFSET: usize = FRAME_ERROR_OFFSET + 8;
/// Offset của `cs` (CPU đẩy).
pub const FRAME_CS_OFFSET: usize = FRAME_RIP_OFFSET + 8;
/// Offset của `rflags` (CPU đẩy).
pub const FRAME_RFLAGS_OFFSET: usize = FRAME_CS_OFFSET + 8;

/// `CS` của kernel ở long mode — selector [`crate::gdt::SELECTOR_KERNEL_CODE`].
pub const KERNEL_CS: u64 = crate::gdt::SELECTOR_KERNEL_CODE as u64;

/// Bit `P` (present) của mã lỗi `#PF` — `0` nghĩa là "trang chưa map / không có quyền".
pub const PF_ERR_PRESENT: u64 = 1 << 0;
/// Bit `W/R` của mã lỗi `#PF` — `1` là ghi, `0` là đọc.
pub const PF_ERR_WRITE: u64 = 1 << 1;
/// Bit `U/S` của mã lỗi `#PF` — `1` là CPL3.
pub const PF_ERR_USER: u64 = 1 << 2;
/// Bit `RSVD` của mã lỗi `#PF` — `1` là vi phạm bit dự phòng trong bảng trang.
pub const PF_ERR_RESERVED: u64 = 1 << 3;
/// Bit `I/D` của mã lỗi `#PF` — `1` là lỗi khi **nạp lệnh**.
pub const PF_ERR_INSTRUCTION: u64 = 1 << 4;
