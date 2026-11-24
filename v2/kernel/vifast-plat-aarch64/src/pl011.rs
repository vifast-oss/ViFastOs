// SPDX-License-Identifier: MIT OR Apache-2.0
//! **PL011 UART** của QEMU `virt` aarch64 — KT-247 (K4' bước 3, B3.1).
//!
//! # Vì sao tệp này **không** có `unsafe`
//!
//! Crate giữ `#![deny(unsafe_code)]`; `unsafe` chỉ nằm ở [`crate::asm`] (D2-17). Tệp này là
//! **dữ liệu + logic**: bảng offset thanh ghi, bit cờ, công thức chia tần, thứ tự khởi tạo — tất
//! cả host-testable (`tests/pl011.rs`). Truy cập MMIO thật đi qua [`crate::asm::mmio_read_u32`]/
//! [`crate::asm::mmio_write_u32`], hai hàm có **stub host panic** như mọi hàm asm khác.
//!
//! # Vì sao cần driver này
//!
//! `vifast-plat-riscv64` in bằng 16550A (`write_volatile` vào `THR`, QEMU không cần chờ `LSR`).
//! PL011 **khác**: có FIFO TX 16 byte và bit `FR.TXFF`; ghi khi FIFO đầy là **mất byte** (QEMU
//! bỏ qua, không báo lỗi) ⇒ boot in thiếu dòng mà không ai biết. Vì vậy `write_byte` **chờ**
//! `FR.TXFF` clear trước khi ghi `DR`, có **trần** vòng chờ để UART hỏng không treo boot.
//!
//! # Khởi tạo
//!
//! QEMU `virt` đã cấu hình PL011 đủ để in (đo được: probe của lead in được `A` mà không cần
//! khởi tạo), nhưng bước 3 vẫn nạp **đủ** chuỗi [`INIT_SEQUENCE`] để ảnh không phụ thuộc vào
//! trạng thái reset của firmware/QEMU: tắt UART → `IBRD`/`FBRD` → `LCR_H` → xoá `ICR` → bật
//! `UARTEN|TXE|RXE`. Thứ tự là **dữ liệu** ([`INIT_SEQUENCE`]) nên test host khẳng định được
//! từng bước và đúng thứ tự — không phải chuỗi lệnh asm không đọc được.

use crate::asm;
use crate::resources::UART0;

// ---------------------------------------------------------------- offset thanh ghi (DDI 0183 §3.2)

/// `UARTDR` — data register (ghi ⇒ TX, đọc ⇒ RX).
pub const DR: u32 = 0x000;
/// `UARTFR` — flag register.
pub const FR: u32 = 0x018;
/// `UARTIBRD` — integer baud rate divisor.
pub const IBRD: u32 = 0x024;
/// `UARTFBRD` — fractional baud rate divisor.
pub const FBRD: u32 = 0x028;
/// `UARTLCR_H` — line control register.
pub const LCR_H: u32 = 0x02C;
/// `UARTCR` — control register.
pub const CR: u32 = 0x030;
/// `UARTIMSC` — interrupt mask set/clear.
pub const IMSC: u32 = 0x038;
/// `UARTICR` — interrupt clear register.
pub const ICR: u32 = 0x044;
/// `UARTMIS` — **masked** interrupt status (DDI 0183 §3.2): bit nào bật trong `IMSC` **và** đang
/// có sự kiện thì bit ấy `1`. **KT-406** dùng để kiểm lại rằng `RXIM` thật sự bật.
pub const MIS: u32 = 0x040;

// ---------------------------------------------------------------- bit (DDI 0183 §3.2)

/// `FR.TXFF` (bit 5) — TX FIFO **đầy**; ghi `DR` lúc này là mất byte.
pub const FR_TXFF: u32 = 1 << 5;
/// `FR.RXFE` (bit 4) — RX FIFO rỗng.
pub const FR_RXFE: u32 = 1 << 4;
/// `FR.BUSY` (bit 3) — UART đang truyền.
pub const FR_BUSY: u32 = 1 << 3;

/// `CR.UARTEN` (bit 0) — bật UART.
pub const CR_UARTEN: u32 = 1 << 0;
/// `CR.TXE` (bit 8) — bật đường phát.
pub const CR_TXE: u32 = 1 << 8;
/// `CR.RXE` (bit 9) — bật đường thu.
pub const CR_RXE: u32 = 1 << 9;

/// `LCR_H.FEN` (bit 4) — bật FIFO.
pub const LCR_H_FEN: u32 = 1 << 4;
/// `LCR_H.WLEN` (bit 6:5) = `0b11` — 8 bit dữ liệu.
pub const LCR_H_WLEN_8: u32 = 0b11 << 5;

/// `IMSC.RXIM` (bit 4) — ngắt RX. **KT-406** bật bit này để PL011 kéo đường IRQ khi có byte.
pub const IMSC_RXIM: u32 = 1 << 4;
/// `IMSC.TXIM` (bit 5) — ngắt TX.
pub const IMSC_TXIM: u32 = 1 << 5;
/// `ICR` có 11 bit ngắt ⇒ xoá tất cả = `0x7FF`.
pub const ICR_ALL: u32 = 0x7FF;

// ---------------------------------------------------------------- chia tần (DDI 0183 §3.3.6)

/// Xung PL011 của QEMU `virt` aarch64: **24 MHz** (`hw/arm/virt.c`, `VIRT_UART_CLK`).
pub const QEMU_VIRT_UARTCLK_HZ: u32 = 24_000_000;
/// Tốc độ console: **115200 8N1** (cùng tốc độ `run.sh` riscv64 dùng cho QEMU).
pub const UART_BAUD: u32 = 115_200;

/// Cặp chia tần `(IBRD, FBRD)` cho `clock_hz`/`baud`, `None` nếu cấu hình **không biểu diễn được**.
///
/// Công thức DDI 0183 §3.3.6:
///
/// ```text
///   divisor = UARTCLK / (16 * baud)
///   IBRD    = phần nguyên (16 bit)
///   FBRD    = round(phần lẻ * 64)   (6 bit)
/// ```
///
/// Làm tròn **nửa lên** bằng số nguyên (`+ den/2` trước khi chia) — không dùng dấu phẩy động
/// (`no_std`, không `libm`). `baud = 0` hoặc `clock = 0` ⇒ `None` (không chia cho 0);
/// `IBRD > 0xFFFF` ⇒ `None` (không biểu diễn được, **không** cắt cụt im lặng).
pub const fn divisors(clock_hz: u32, baud: u32) -> Option<(u16, u8)> {
    if baud == 0 || clock_hz == 0 {
        return None;
    }
    let den = 16u64 * baud as u64;
    let num = clock_hz as u64;
    let ibrd = num / den;
    if ibrd > 0xFFFF {
        return None;
    }
    let rem = num % den;
    // round(rem * 64 / den) với làm tròn nửa lên.
    let fbrd = (rem * 64 + den / 2) / den;
    // `fbrd` có thể bằng 64 khi phần lẻ rất sát 1 ⇒ cộng vào IBRD (chuẩn hoá).
    if fbrd >= 64 {
        if ibrd + 1 > 0xFFFF {
            return None;
        }
        return Some(((ibrd + 1) as u16, 0));
    }
    Some((ibrd as u16, fbrd as u8))
}

/// Cặp chia tần **đã chọn** cho console của ảnh — tính lúc biên dịch từ hai hằng ở trên.
///
/// `divisors` là `const fn` nên giá trị này là hằng; nếu ai đổi `UART_BAUD` thành cấu hình không
/// biểu diễn được thì `(0, 0)` xuất hiện và test `init_sequence_order` **ĐỎ** (không im lặng).
pub const DIVISORS: (u16, u8) = match divisors(QEMU_VIRT_UARTCLK_HZ, UART_BAUD) {
    Some(d) => d,
    None => (0, 0),
};

// ---------------------------------------------------------------- thứ tự khởi tạo

/// Thanh ghi mà một bước khởi tạo ghi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    /// `UARTCR`.
    Cr,
    /// `UARTIBRD`.
    Ibrd,
    /// `UARTFBRD`.
    Fbrd,
    /// `UARTLCR_H`.
    LcrH,
    /// `UARTICR`.
    Icr,
}

impl Register {
    /// Offset của thanh ghi trong khối PL011.
    pub const fn offset(self) -> u32 {
        match self {
            Self::Cr => CR,
            Self::Ibrd => IBRD,
            Self::Fbrd => FBRD,
            Self::LcrH => LCR_H,
            Self::Icr => ICR,
        }
    }
}

/// Chuỗi khởi tạo **bắt buộc** (DDI 0183 §3.3): `(thanh ghi, giá trị)` theo đúng thứ tự.
///
/// 1. `CR = 0` — tắt UART **trước** khi đổi chia tần/khung (đổi lúc đang chạy ⇒ khung sai);
/// 2. `IBRD`, `FBRD` — chia tần cho 115200 từ xung 24 MHz ⇒ `(13, 1)`;
/// 3. `LCR_H = FEN|WLEN_8 = 0x70` — FIFO bật, 8 bit, không parity, 1 stop;
/// 4. `ICR = 0x7FF` — xoá mọi ngắt treo từ firmware;
/// 5. `CR = UARTEN|TXE|RXE = 0x301` — bật.
pub const INIT_SEQUENCE: [(Register, u32); 6] = [
    (Register::Cr, 0),
    (Register::Ibrd, DIVISORS.0 as u32),
    (Register::Fbrd, DIVISORS.1 as u32),
    (Register::LcrH, LCR_H_FEN | LCR_H_WLEN_8),
    (Register::Icr, ICR_ALL),
    (Register::Cr, CR_UARTEN | CR_TXE | CR_RXE),
];

/// Số vòng chờ `FR.TXFF` clear tối đa trước khi bỏ (UART hỏng **không** được treo boot).
///
/// QEMU `virt` truyền 1 byte tức thời nên vòng chờ thực tế ~0; trần này chỉ để fail-loud thay vì
/// treo. 1.000.000 vòng ở ~1 GHz là dưới 10 ms.
pub const TX_WAIT_LIMIT: u32 = 1_000_000;

// ---------------------------------------------------------------- truy cập thanh ghi

/// Địa chỉ MMIO của thanh ghi `offset` trong khối PL011.
pub const fn reg_addr(offset: u32) -> u64 {
    UART0.base + offset as u64
}

/// Đọc thanh ghi PL011.
pub fn read_reg(offset: u32) -> u32 {
    asm::mmio_read_u32(reg_addr(offset))
}

/// Ghi thanh ghi PL011.
pub fn write_reg(offset: u32, value: u32) {
    asm::mmio_write_u32(reg_addr(offset), value);
}

/// Nạp [`INIT_SEQUENCE`] theo đúng thứ tự; trả số bước đã ghi (để ảnh in bằng chứng).
pub fn init() -> usize {
    for (reg, value) in INIT_SEQUENCE {
        write_reg(reg.offset(), value);
    }
    INIT_SEQUENCE.len()
}

/// Ghi **một** byte ra `DR`, chờ `FR.TXFF` clear trước (có trần [`TX_WAIT_LIMIT`]).
///
/// Trả `true` ⇔ byte đã được ghi (FIFO không đầy trong trần chờ). `false` = UART kẹt: caller
/// đếm được, **không** treo và **không** ghi đè byte khác.
pub fn write_byte(byte: u8) -> bool {
    let mut spins = 0;
    while read_reg(FR) & FR_TXFF != 0 {
        if spins >= TX_WAIT_LIMIT {
            return false;
        }
        spins += 1;
        core::hint::spin_loop();
    }
    write_reg(DR, byte as u32);
    true
}

/// Ghi một chuỗi; trả số byte **không** ghi được (0 = in đủ).
pub fn write_str(s: &str) -> usize {
    let mut dropped = 0;
    for byte in s.bytes() {
        if !write_byte(byte) {
            dropped += 1;
        }
    }
    dropped
}

/// `true` ⇔ UART đang bận truyền (`FR.BUSY`).
pub fn is_busy() -> bool {
    read_reg(FR) & FR_BUSY != 0
}

// ---------------------------------------------------------------- **KT-406** — đường RX

/// Trần byte **một** lần vét FIFO RX — handler ngắt không bao giờ quay vô hạn.
pub const RX_DRAIN_LIMIT: usize = 64;

/// Bật `IMSC.RXIM`; trả `true` ⇔ đọc lại `IMSC` thấy bit đã bật.
///
/// Dùng `|=` (không ghi đè) để không tắt `TXIM` nếu bước khác đã bật; sau khi ghi có
/// `dsb` (hàng rào thiết bị) trước lần đọc lại — không tin MMIO đã nhận lệnh ghi.
pub fn rx_enable() -> bool {
    write_reg(IMSC, read_reg(IMSC) | IMSC_RXIM);
    asm::dsb_sy();
    read_reg(IMSC) & IMSC_RXIM != 0
}

/// Tắt `IMSC.RXIM`; trả `true` ⇔ đọc lại thấy bit đã 0.
pub fn rx_disable() -> bool {
    write_reg(IMSC, read_reg(IMSC) & !IMSC_RXIM);
    asm::dsb_sy();
    read_reg(IMSC) & IMSC_RXIM == 0
}

/// `UARTMIS` đọc lại — bằng chứng `irq=… mis=0x…` và ca âm "ngắt treo".
pub fn masked_status() -> u32 {
    read_reg(MIS)
}

/// `true` ⇔ còn byte trong FIFO RX (`FR.RXFE = 0`).
pub fn rx_pending() -> bool {
    read_reg(FR) & FR_RXFE == 0
}

/// Vét FIFO RX vào `out` tới khi `FR.RXFE = 1` hoặc đầy `out`; trả số byte đã đọc.
///
/// Đọc `DR` **xoá** nguồn ngắt RX tương ứng (DDI 0183 §3.2) ⇒ vòng lặp này là toàn bộ phần
/// "ack" của ngắt RX phía thiết bị; GIC vẫn phải `EOI` riêng.
pub fn rx_drain(out: &mut [u8]) -> usize {
    let limit = out.len().min(RX_DRAIN_LIMIT);
    let mut count = 0usize;
    while count < limit && rx_pending() {
        // 12 bit thấp của `DR` là dữ liệu; bit 8..11 là lỗi khung/parity/break — bỏ qua, byte
        // vẫn được đẩy vào ring để người dùng thấy dữ liệu thật (không nuốt im lặng).
        out[count] = (read_reg(DR) & 0xFF) as u8;
        count += 1;
    }
    count
}
