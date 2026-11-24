// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho driver PL011 + bảng tài nguyên aarch64 — KT-247 (K4' bước 3, B3.1).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Mọi kỳ vọng dưới đây là **giá trị tuyệt đối viết tay** từ hai nguồn ngoài crate:
//!
//! - **PL011 Technical Reference Manual** (ARM DDI 0183): offset thanh ghi, bit `FR`/`CR`/`LCR_H`,
//!   công thức chia tần `IBRD = UARTCLK/(16*baud)`, `FBRD = round(frac*64)`;
//! - **QEMU `virt` memory map** (`hw/arm/virt.c`, tài liệu `qemu-system-aarch64 -M virt`):
//!   PL011 tại `0x0900_0000`, GIC distributor `0x0800_0000`, GIC CPU interface `0x0801_0000`,
//!   RAM bắt đầu `0x4000_0000`.
//!
//! **Không** lấy hằng của chính crate để so (cùng luật với `tests/sysreg.rs` của KT-167): một
//! offset sai hay một bit sai đều làm test **ĐỎ**. Đây là ca kiểm **đỏ trước** của KT-247 — nó
//! được viết trước khi `pl011`/`resources` tồn tại (log: `v2/agent/JOURNAL/kt247-red.log`).

use vifast_plat_aarch64::pl011::{
    divisors, Register, CR_RXE, CR_TXE, CR_UARTEN, DIVISORS, DR, FBRD, FR, FR_BUSY, FR_RXFE,
    FR_TXFF, IBRD, ICR, ICR_ALL, IMSC, IMSC_RXIM, IMSC_TXIM, INIT_SEQUENCE, LCR_H, LCR_H_FEN,
    LCR_H_WLEN_8, MIS, QEMU_VIRT_UARTCLK_HZ, RX_DRAIN_LIMIT, UART_BAUD,
};
use vifast_plat_aarch64::resources::{first_overlap, GIC, GIC_CPU, RAM, UART0};

// ---------------------------------------------------------------- offset thanh ghi (DDI 0183 §3.2)

/// Offset thanh ghi PL011 — bảng 3-1 của DDI 0183. Sai một offset ⇒ ghi vào thanh ghi khác.
#[test]
fn register_offsets() {
    assert_eq!(DR, 0x000, "UARTDR");
    assert_eq!(FR, 0x018, "UARTFR");
    assert_eq!(IBRD, 0x024, "UARTIBRD");
    assert_eq!(FBRD, 0x028, "UARTFBRD");
    assert_eq!(LCR_H, 0x02C, "UARTLCR_H");
    assert_eq!(IMSC, 0x038, "UARTIMSC");
    assert_eq!(ICR, 0x044, "UARTICR");
}

/// Bit cờ `UARTFR` (DDI 0183 §3.2.4): `TXFF` bit 5, `RXFE` bit 4, `BUSY` bit 3.
#[test]
fn flag_register_bits() {
    assert_eq!(FR_TXFF, 1 << 5, "TX FIFO full");
    assert_eq!(FR_RXFE, 1 << 4, "RX FIFO empty");
    assert_eq!(FR_BUSY, 1 << 3, "UART busy");
}

/// Bit `UARTCR` (DDI 0183 §3.2.8): `UARTEN` bit 0, `TXE` bit 8, `RXE` bit 9.
#[test]
fn control_register_bits() {
    assert_eq!(CR_UARTEN, 1 << 0);
    assert_eq!(CR_TXE, 1 << 8);
    assert_eq!(CR_RXE, 1 << 9);
}

/// Bit `UARTLCR_H` (DDI 0183 §3.2.7): `FEN` bit 4, `WLEN` bit 6:5 = `0b11` cho 8 bit.
#[test]
fn line_control_bits() {
    assert_eq!(LCR_H_FEN, 1 << 4, "FIFO enable");
    assert_eq!(LCR_H_WLEN_8, 0b11 << 5, "word length 8 bit");
}

/// `UARTICR` (DDI 0183 §3.2.12) có 11 bit ngắt ⇒ xoá tất cả = `0x7FF`; `IMSC` bit 4/5 là RX/TX.
#[test]
fn interrupt_bits() {
    assert_eq!(ICR_ALL, 0x7FF);
    assert_eq!(IMSC_RXIM, 1 << 4);
    assert_eq!(IMSC_TXIM, 1 << 5);
}

// ---------------------------------------------------------------- chia tần (DDI 0183 §3.3.6)

/// QEMU `virt` cấp PL011 xung 24 MHz; 115200 baud ⇒ `IBRD = 13`, `FBRD = 1`
/// (`24e6/(16*115200) = 13,0208…`; `0,0208…*64 = 1,33` ⇒ làm tròn 1).
#[test]
fn divisors_qemu_virt_115200() {
    assert_eq!(QEMU_VIRT_UARTCLK_HZ, 24_000_000);
    assert_eq!(UART_BAUD, 115_200);
    assert_eq!(divisors(QEMU_VIRT_UARTCLK_HZ, UART_BAUD), Some((13, 1)));
}

/// Cùng công thức ở hai tần số khác — chứng minh không phải hằng số cắm sẵn cho một cặp:
/// `24e6/(16*9600) = 156,25` ⇒ `(156, 16)`; `48e6/(16*115200) = 26,0416…` ⇒ `(26, 3)`.
#[test]
fn divisors_other_rates() {
    assert_eq!(divisors(24_000_000, 9_600), Some((156, 16)));
    assert_eq!(divisors(48_000_000, 115_200), Some((26, 3)));
}

/// Ca âm: baud 0 (chia cho 0) và chia tần vượt 16 bit ⇒ `None`, **không** panic, **không** trả
/// giá trị rác. `IBRD` chỉ 16 bit nên `clock/(16*baud) > 0xFFFF` là cấu hình không biểu diễn được.
#[test]
fn divisors_rejects_impossible() {
    assert_eq!(divisors(24_000_000, 0), None, "baud 0");
    assert_eq!(divisors(24_000_000, 1), None, "IBRD = 1.500.000 > 0xFFFF");
    assert_eq!(divisors(0, 115_200), None, "clock 0");
}

// ---------------------------------------------------------------- thứ tự khởi tạo

/// Thứ tự khởi tạo **bắt buộc** (DDI 0183 §3.3): tắt UART trước khi đổi `IBRD`/`FBRD`/`LCR_H`,
/// xoá ngắt treo, rồi mới bật `UARTEN|TXE|RXE`. Đảo thứ tự ⇒ khung dữ liệu sai hoặc mất byte.
#[test]
fn init_sequence_order() {
    assert_eq!(INIT_SEQUENCE.len(), 6);
    assert_eq!(INIT_SEQUENCE[0].0, Register::Cr, "1. tắt UART");
    assert_eq!(INIT_SEQUENCE[1].0, Register::Ibrd, "2. IBRD");
    assert_eq!(INIT_SEQUENCE[2].0, Register::Fbrd, "3. FBRD");
    assert_eq!(INIT_SEQUENCE[3].0, Register::LcrH, "4. khung dữ liệu");
    assert_eq!(INIT_SEQUENCE[4].0, Register::Icr, "5. xoá ngắt");
    assert_eq!(INIT_SEQUENCE[5].0, Register::Cr, "6. bật");
    assert_eq!(DIVISORS, (13, 1), "chia tần đã chọn cho 24 MHz/115200");
}

/// Giá trị **tuyệt đối** mà từng bước ghi: `CR = 0`; `IBRD = 13`; `FBRD = 1`;
/// `LCR_H = FEN|WLEN_8 = 0x70`; `ICR = 0x7FF`; `CR = UARTEN|TXE|RXE = 0x301`.
#[test]
fn init_step_values() {
    assert_eq!(INIT_SEQUENCE[0].1, 0, "CR tắt");
    assert_eq!(INIT_SEQUENCE[1].1, 13, "IBRD");
    assert_eq!(INIT_SEQUENCE[2].1, 1, "FBRD");
    assert_eq!(INIT_SEQUENCE[3].1, 0x70, "LCR_H");
    assert_eq!(INIT_SEQUENCE[4].1, 0x7FF, "ICR");
    assert_eq!(INIT_SEQUENCE[5].1, 0x301, "CR bật");
    assert_eq!(
        INIT_SEQUENCE[5].1,
        CR_UARTEN | CR_TXE | CR_RXE,
        "ba bit bật phải khớp hằng"
    );
    assert_eq!(INIT_SEQUENCE[3].1, LCR_H_FEN | LCR_H_WLEN_8);
}

/// Offset của từng [`Register`] phải khớp bảng thanh ghi — nếu không, chuỗi khởi tạo ghi vào
/// thanh ghi khác dù thứ tự đúng.
#[test]
fn register_offsets_match_table() {
    assert_eq!(Register::Cr.offset(), 0x030);
    assert_eq!(Register::Ibrd.offset(), 0x024);
    assert_eq!(Register::Fbrd.offset(), 0x028);
    assert_eq!(Register::LcrH.offset(), 0x02C);
    assert_eq!(Register::Icr.offset(), 0x044);
}

/// **KT-406** — thanh ghi/bit của đường **RX** viết tay từ DDI 0183 §3.2:
/// `UARTMIS = 0x040`, `FR.RXFE = bit 4`, `IMSC.RXIM = bit 4`, trần drain 64 byte.
#[test]
fn rx_register_bits() {
    assert_eq!(MIS, 0x040, "UARTMIS offset");
    assert_eq!(DR, 0x000);
    assert_eq!(FR, 0x018);
    assert_eq!(IMSC, 0x038);
    assert_eq!(ICR, 0x044);
    assert_eq!(FR_RXFE, 1 << 4, "FR.RXFE");
    assert_eq!(IMSC_RXIM, 1 << 4, "IMSC.RXIM");
    assert_eq!(RX_DRAIN_LIMIT, 64, "trần drain một lần ngắt");
}

// ---------------------------------------------------------------- bảng tài nguyên QEMU virt

/// Địa chỉ QEMU `virt` (aarch64) — viết tay từ memory map, **không** lấy từ crate.
#[test]
fn qemu_virt_addresses() {
    assert_eq!(UART0.base, 0x0900_0000, "PL011 UART0");
    assert_eq!(UART0.size, 0x1000);
    assert_eq!(GIC.base, 0x0800_0000, "GIC distributor");
    assert_eq!(GIC.size, 0x1_0000);
    assert_eq!(GIC_CPU.base, 0x0801_0000, "GIC CPU interface");
    assert_eq!(RAM.base, 0x4000_0000, "RAM bắt đầu");
}

/// Bảng tài nguyên **không** chồng lấn — cùng phép kiểm với `vifast-plat-common::resources`
/// (KT-81): hai vùng đè nhau thì MMU map một đằng, driver đọc một nẻo, không lỗi biên dịch.
#[test]
fn resources_do_not_overlap() {
    assert_eq!(first_overlap(), None);
}
