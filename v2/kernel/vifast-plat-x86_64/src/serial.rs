// SPDX-License-Identifier: MIT OR Apache-2.0
//! COM1 (16550 UART) tại `0x3F8` — **dữ liệu thuần** cho ảnh boot x86_64 (KT-257, K5' bước 1).
//!
//! Tệp này **không** asm, **không** `unsafe`: nó chỉ chốt **địa chỉ thanh ghi**, **bit trạng
//! thái**, **giá trị khung truyền** và **thứ tự chuỗi khởi tạo**. Ảnh `vifast-kbin-x86_64` dùng
//! đúng các hằng này (qua `const` operand của `global_asm!` và qua [`init_sequence`]) nên test host
//! dưới đây là bằng chứng cho **giá trị asm thật sự nạp** — không có bản thứ hai của con số.
//!
//! # Nguồn
//!
//! - **PC16550D datasheet** (National Semiconductor): bảng thanh ghi `RBR/THR/DLL` (offset 0),
//!   `IER/DLM` (1), `IIR/FCR` (2), `LCR` (3), `MCR` (4), `LSR` (5), `MSR` (6), `SCR` (7);
//!   `LSR.THRE` bit 5, `LSR.TEMT` bit 6; `LCR.DLAB` bit 7; `LCR` 8N1 = `0b0000_0011`;
//!   `FCR` bật+xoá FIFO ngưỡng 14 = `0xC7`; `MCR` DTR|RTS|OUT2 = `0x0B`.
//! - **QEMU** `-nographic`: COM1 (`0x3F8`, IRQ 4) được nối ra stdio; QEMU **không** giới hạn tốc
//!   độ, nhưng divisor vẫn phải đúng ngữ nghĩa 115200 8N1 để ảnh chạy được trên máy thật.
//!
//! # Vì sao `divisor` là `const fn`
//!
//! Chuỗi khởi tạo phải **tất định** và **kiểm được trên host**: `divisor(115_200)` trả `(1, 0)`
//! (1.8432 MHz / (16 × 115200) = 1), và [`init_sequence`] nhét đúng hai byte ấy vào `DLL`/`DLM`
//! **sau** khi bật `DLAB` — thứ tự sai là lỗi im lặng (đặt divisor vào `THR`).

/// Địa chỉ gốc COM1 theo PC standard.
pub const COM1_BASE: u16 = 0x3F8;

/// `RBR` (đọc) / `THR` (ghi) khi `DLAB = 0`; `DLL` khi `DLAB = 1`.
pub const REG_DATA: u16 = 0;
/// `IER` khi `DLAB = 0`; `DLM` khi `DLAB = 1`.
pub const REG_IER: u16 = 1;
/// `IIR` (đọc) / `FCR` (ghi).
pub const REG_FCR: u16 = 2;
/// `LCR` — khung truyền + `DLAB`.
pub const REG_LCR: u16 = 3;
/// `MCR` — điều khiển modem (DTR/RTS/OUT2).
pub const REG_MCR: u16 = 4;
/// `LSR` — trạng thái đường truyền (chỉ đọc).
pub const REG_LSR: u16 = 5;
/// `MSR` — trạng thái modem (không dùng ở bước 1).
pub const REG_MSR: u16 = 6;
/// `SCR` — scratch (không dùng ở bước 1).
pub const REG_SCR: u16 = 7;

/// `LSR.DR` bit 0 — có byte để đọc.
pub const LSR_DR: u8 = 1 << 0;
/// `LSR.THRE` bit 5 — `THR` rỗng, ghi được byte kế. **Đây là bit `write_byte` phải chờ.**
pub const LSR_THRE: u8 = 1 << 5;
/// `LSR.TEMT` bit 6 — bộ truyền hoàn toàn rỗng (dùng trước khi tắt máy để không mất marker).
pub const LSR_TEMT: u8 = 1 << 6;

/// `LCR.DLAB` bit 7 — đổi `offset 0/1` thành `DLL`/`DLM`.
pub const LCR_DLAB: u8 = 0x80;
/// `LCR` cho 8 data bit, không parity, 1 stop bit (`0b0000_0011`).
pub const LCR_8N1: u8 = 0x03;

/// `FCR`: bật FIFO + xoá cả hai FIFO + ngưỡng ngắt 14 byte (`0b1100_0111`).
pub const FCR_ENABLE_CLEAR_14: u8 = 0xC7;
/// `MCR`: DTR (bit 0) + RTS (bit 1) + OUT2 (bit 3) — OUT2 phải bật để IRQ đi qua (dù bước 1
/// không dùng ngắt, giữ đúng khởi tạo chuẩn).
pub const MCR_DTR_RTS_OUT2: u8 = 0x0B;

/// Xung nhịp chuẩn của 16550 trên PC: 1.8432 MHz.
pub const UART_CLOCK_HZ: u32 = 1_843_200;
/// Tốc độ của ảnh boot (không phải lựa chọn của kernel — chỉ là giá trị khởi tạo).
pub const BAUD_115200: u32 = 115_200;
/// Divisor dự phòng nếu [`divisor`] từ chối tốc độ (không xảy ra với [`BAUD_115200`]).
pub const DIVISOR_FALLBACK: (u8, u8) = (1, 0);

/// Trần số vòng chờ `LSR.THRE` trước khi ghi **bất chấp** — UART hỏng không được treo máy.
///
/// 100.000 vòng là rất lớn so với thời gian một byte ở 115200 baud (~87 µs) nhưng vẫn hữu hạn;
/// đây là **trần**, không phải ngưỡng thời gian, nên không phụ thuộc tốc độ CPU.
pub const TX_WAIT_LIMIT: u32 = 100_000;

/// Số bước của chuỗi khởi tạo (khớp độ dài [`init_sequence`]).
pub const INIT_STEPS: usize = 6;

/// Một bước khởi tạo: ghi `value` vào thanh ghi `COM1_BASE + offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitStep {
    /// Offset thanh ghi (một trong `REG_*`).
    pub offset: u16,
    /// Giá trị ghi.
    pub value: u8,
}

/// `divisor = UART_CLOCK_HZ / (16 × baud)` — trả `(DLL, DLM)`; `None` nếu không biểu diễn được.
///
/// Từ chối **hai** phía: divisor 0 (baud quá cao) và divisor > `0xFFFF` (baud quá thấp). Không
/// bao giờ chia cho 0 vì `baud == 0` được chặn trước.
pub const fn divisor(baud: u32) -> Option<(u8, u8)> {
    if baud == 0 {
        return None;
    }
    let denom = 16u64 * baud as u64;
    let d = UART_CLOCK_HZ as u64 / denom;
    if d == 0 || d > 0xFFFF {
        return None;
    }
    Some(((d & 0xFF) as u8, ((d >> 8) & 0xFF) as u8))
}

/// Chuỗi khởi tạo **đúng thứ tự** của 16550: `DLAB` → `DLL` → `DLM` → `LCR` 8N1 → `FCR` → `MCR`.
///
/// Vì sao thứ tự này là hợp đồng: `DLL`/`DLM` chỉ tồn tại khi `DLAB = 1`; nếu đặt `LCR` 8N1 trước
/// thì hai byte divisor rơi vào `THR`/`IER` (mất một ký tự rác + bật ngắt sai). `FCR` trước `MCR`
/// để FIFO sạch trước khi bật modem control.
pub const fn init_sequence(divisor: (u8, u8)) -> [InitStep; INIT_STEPS] {
    [
        InitStep {
            offset: REG_LCR,
            value: LCR_DLAB,
        },
        InitStep {
            offset: REG_DATA,
            value: divisor.0,
        },
        InitStep {
            offset: REG_IER,
            value: divisor.1,
        },
        InitStep {
            offset: REG_LCR,
            value: LCR_8N1,
        },
        InitStep {
            offset: REG_FCR,
            value: FCR_ENABLE_CLEAR_14,
        },
        InitStep {
            offset: REG_MCR,
            value: MCR_DTR_RTS_OUT2,
        },
    ]
}
