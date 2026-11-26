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

/// **Timer** — vector 32 (KT-266). Nằm ngoài dải exception 0..31: đây là **ngắt** phần cứng đầu
/// tiên của ảnh. Đây là vector của `LVT_TIMER`; nguồn tick thực tế là **PIT** (kênh 0) đi qua
/// IOAPIC → LAPIC (xem `v2/docs/k5-x86_64-boot.md` §5.4).
pub const VEC_TIMER: u8 = 32;

/// **Spurious interrupt** của LAPIC — vector 255 (KT-266). SDM Vol 3A §10.9: ngắt spurious
/// **không** cần `EOI`; handler chỉ đếm rồi `iretq`.
pub const VEC_SPURIOUS: u8 = 255;

/// **Syscall** — vector `0x80` của `int 0x80` (KT-270, D2-207).
///
/// Đây là **software interrupt** (SDM Vol 2B "INT n/INTO/INT3/INT1"): CPU lưu RIP của lệnh **kế
/// tiếp** (khác fault), nên handler **không** cộng RIP. Gate IDT của vector này có `DPL = 3`
/// ([`crate::idt::GATE_INTERRUPT_USER`]) để ring 3 gọi được; mọi vector khác giữ `DPL = 0`.
pub const VEC_SYSCALL: u8 = 0x80;

/// **COM1 RX** — vector `0x24` (36) của **IRQ 4** (ISA) qua IOAPIC → LAPIC, **KT-406**.
///
/// Vector nằm ngoài dải exception `0..31`, khác timer `32`, IPI `0x30`, syscall `0x80`,
/// spurious `255`, sentinel `254`. QEMU `q35` route IRQ 4 của `isa-serial` qua IOAPIC pin 4
/// (`-nographic` ⇒ `isa-serial` ở `0x3F8` = COM1), nên đây là **ngắt phần cứng thật** của UART,
/// không phải `int` phần mềm.
pub const VEC_SERIAL: u8 = 0x24;

/// **IPI** — vector `0x30` của ngắt liên-hart (KT-278, K5' bước 7).
///
/// BSP gửi bằng `ICR` với delivery mode **fixed** ([`crate::smp::icr_fixed_low`]); hart đích nhận
/// qua LAPIC **của nó** và vào stub riêng [`STUB_IPI`]. Vector nằm **ngoài** dải exception
/// `0..31` và khác mọi vector đã dùng: timer `32`, syscall `0x80`, spurious `255`, sentinel
/// [`VEC_UNKNOWN`] `254`.
///
/// **Vì sao không dùng `0x30` cho probe `isr_unknown` nữa**: KT-261 dùng `int 0x30` để chứng minh
/// "vector lạ vẫn thấy được"; từ KT-278 `0x30` là vector IPI **có stub riêng**, nên probe dời
/// sang [`VEC_UNKNOWN_PROBE`] (`0x31`) — nếu không, TAP 7 sẽ chạy vào handler IPI.
pub const VEC_IPI: u8 = 0x30;

/// Vector dùng cho probe `int` của ca `isr_unknown` (TAP 7) — **không** có stub riêng nên rơi về
/// [`STUB_UNKNOWN`] và handler in `vec=254`.
///
/// `0x31` là vector **kế tiếp** vector IPI: vẫn ngoài dải exception, vẫn không có stub riêng, và
/// cách `VEC_IPI` đúng một số để một thay đổi `stub_slot` không thể vô tình gộp hai ca làm một.
pub const VEC_UNKNOWN_PROBE: u8 = 0x31;

/// **Watchdog LAPIC** — vector `0x32` (50) của **nguồn đánh thức dự phòng** (KT-413, đóng nợ
/// `N-3` của `KT-348a`).
///
/// # Vì sao cần một vector **riêng**
///
/// `measure_timer` chờ tick PIT bằng `sti; hlt`. `hlt` chỉ thức khi **có** ngắt; nếu mất IRQ0
/// (IOAPIC mask sai, PIT hỏng, đường redirection đứt) thì **không** ngắt nào tới ⇒ `hlt` ngủ
/// **vĩnh viễn** ⇒ boot treo im lặng. `KT-348a` đo được đúng lớp ấy và để lại nợ `N-3`.
///
/// Dùng lại vector **timer 32** thì không được: handler timer tăng `TICK_COUNT`, nên một ngắt
/// watchdog sẽ **giả** một tick và phép đo `hz`/`ticks_ok` mất nghĩa. Vector `0x32` có stub +
/// handler **riêng**, chỉ tăng `WATCHDOG_WAKES` rồi `EOI` — nó **đánh thức** `hlt` mà **không**
/// làm bẩn bộ đếm tick.
///
/// # Vì sao `0x32` (không phải `0x31`)
///
/// `0x31` là vector của probe `isr_unknown` (TAP 7) — dùng nó sẽ làm probe ấy chạy vào handler
/// watchdog. `0x32` là số **kế tiếp** tự do: khác timer `32`, serial `0x24`, IPI `0x30`, probe
/// `0x31`, syscall `0x80`, sentinel `254`, spurious `255`.
pub const VEC_WATCHDOG: u8 = 0x32;

/// Vector **giả** mà stub `isr_unknown` đẩy: stub dùng chung không biết vector thật, nên nó đẩy
/// một sentinel và handler in `vec=254`.
///
/// **KT-266 dời sentinel từ 255 sang 254**: từ bước 3, vector **255 là vector thật** (spurious
/// của LAPIC) và có stub riêng, nên sentinel phải khác nó — nếu không, handler không phân biệt
/// được "ngắt spurious" với "vector lạ". `254` vẫn không trùng vector thật nào (0..31, 32, 255)
/// và không trùng `0` (giá trị "chưa có" của biến đếm).
pub const VEC_UNKNOWN: u64 = 0xFE;

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
/// Slot stub **LAPIC timer** (vector 32, không mã lỗi) — KT-266.
pub const STUB_TIMER: usize = 15;
/// Slot stub **spurious** (vector 255, không mã lỗi) — KT-266.
pub const STUB_SPURIOUS: usize = 16;
/// Slot stub **syscall** (vector `0x80`, không mã lỗi) — KT-270.
pub const STUB_SYSCALL: usize = 17;
/// Slot stub **IPI** (vector `0x30`, không mã lỗi) — KT-278.
pub const STUB_IPI: usize = 18;
/// Slot stub **COM1 RX** (vector `0x24`, không mã lỗi) — **KT-406**.
pub const STUB_SERIAL: usize = 19;
/// Slot stub **watchdog LAPIC** (vector `0x32`, không mã lỗi) — **KT-413** (nợ `N-3`).
pub const STUB_WATCHDOG: usize = 20;
/// Số slot stub — **khớp** số nhánh của `asm::stub_addr`.
pub const STUB_COUNT: usize = 21;

/// Vector → slot stub. Vector không có stub riêng rơi về [`STUB_UNKNOWN`].
///
/// **Hợp đồng với asm**: `asm::stub_addr(slot)` phải trả đúng stub của slot này, và stub phải
/// đẩy đúng vector (hoặc [`VEC_UNKNOWN`] với slot 0). Test host chốt ánh xạ; ảnh boot chốt hành
/// vi thật (`int3` ⇒ `vec=3`, timer ⇒ `vec=32`, IPI ⇒ `vec=0x30`, …).
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
        VEC_TIMER => STUB_TIMER,
        VEC_SPURIOUS => STUB_SPURIOUS,
        VEC_SYSCALL => STUB_SYSCALL,
        VEC_IPI => STUB_IPI,
        VEC_SERIAL => STUB_SERIAL,
        VEC_WATCHDOG => STUB_WATCHDOG,
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
        32 => "timer",
        36 => "com1",
        48 => "ipi",
        50 => "watchdog",
        128 => "syscall",
        254 => "unknown",
        255 => "spurious",
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

/// Giá trị stub chung (`vifast_isr_common`) nhận lại từ `exc_dispatch` — **hai** qword trong
/// `rax`/`rdx` (SysV AMD64).
///
/// **KT-413c** — chuyển từ `sched.rs` (đã xoá cùng mô hình scheduler tự viết) sang đây: kiểu
/// này là hợp đồng của **mọi** ngắt/exception, không riêng scheduler. Chỉ còn `resume()`
/// (đường exception/ngắt thường); nhánh chuyển ngữ cảnh phần cứng của `sched.rs` cũ đã gỡ cùng
/// `TASK_SEQUENCE`/4 stack 16 KiB.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameReturn {
    /// `rip` để `iretq` về. `0` ⇒ stub **halt sạch**.
    pub rip: u64,
    /// `rsp` của frame kế; `0` ⇒ giữ frame hiện tại.
    pub new_rsp: u64,
}

impl FrameReturn {
    /// Trả về **đúng chỗ bị ngắt** — không đổi `rsp`.
    pub const fn resume(rip: u64) -> Self {
        Self { rip, new_rsp: 0 }
    }

    /// `true` ⇔ stub phải **halt** thay vì `iretq` (handler trả `rip = 0`).
    pub const fn is_halt(&self) -> bool {
        self.rip == 0 && self.new_rsp == 0
    }
}
