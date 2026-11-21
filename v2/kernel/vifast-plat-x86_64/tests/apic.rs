// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho APIC + timer x86_64 — KT-266 (K5' bước 3).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test apic
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-266: mô-đun [`apic`] chưa tồn tại khi tệp này được
//! viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do). Sau khi hiện thực, các
//! khẳng định dưới đây chốt **giá trị tuyệt đối** mà asm/ảnh `vifast-kbin-x86_64` dùng — nếu ai
//! đổi một bit, test đỏ **trước** khi có ảnh boot.
//!
//! Nguồn giá trị:
//!
//! - **Intel SDM Vol 3A** §10.4.1 (LAPIC MMIO register map), §10.4.6 (SVR), §10.5.4 (LVT timer),
//!   §10.5.4 "Divide Configuration Register" (bảng chia), §10.6.1 (EOI), §10.12.1 (`IA32_APIC_BASE`).
//! - **Intel SDM Vol 3A** §11.5.1 (IOAPIC MMIO: `IOREGSEL`/`IOWIN`), §11.5.2 (`IOAPICID`),
//!   §11.5.3 (`IOAPICVER`), §11.5.4 (redirection table entry, bit 16 = mask).
//! - **Intel 8254** datasheet: tần số 1,193182 MHz; mode 0 (interrupt on terminal count);
//!   lệnh latch đọc counter.
//!
//! # Vì sao PIT nằm trong test này
//!
//! `hz` của LAPIC timer **không** được hardcode: ảnh đo nó bằng **PIT kênh 0** (nguồn thời gian
//! chuẩn 1,193182 MHz của PC). Các hằng số PIT ở đây là **hợp đồng** giữa ảnh và test host.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà asm của
// ảnh boot nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::apic::{
    hz_from_pit, hz_ok, ioapic_id, ioapic_redir_is_masked, ioapic_redir_low_masked,
    ioapic_redirection_count, ioapic_redtbl_reg, ioapic_version, lapic_id, lapic_timer_runs,
    lapic_version, lvt_is_masked, lvt_timer_value, lvt_vector, pit_divisor_ok, pit_elapsed_ticks,
    svr_value, timer_divide_code, timer_divide_value, APIC_BASE_ADDR_MASK, APIC_BASE_BSP,
    APIC_BASE_ENABLE, HZ_MAX, HZ_MIN, IOAPIC_DEFAULT_BASE, IOAPIC_ID_REG, IOAPIC_MAX_REDIRECTIONS,
    IOAPIC_REDIR_MASKED, IOAPIC_REDTBL_BASE, IOAPIC_REDTBL_STRIDE, IOAPIC_REGSEL, IOAPIC_VER_REG,
    IOAPIC_WIN, LAPIC_DEFAULT_BASE, LAPIC_EOI, LAPIC_ID, LAPIC_ID_SHIFT, LAPIC_LVT_LINT0,
    LAPIC_LVT_LINT1, LAPIC_LVT_TIMER, LAPIC_SVR, LAPIC_TIMER_CURRENT_COUNT, LAPIC_TIMER_DIVIDE,
    LAPIC_TIMER_INITIAL_COUNT, LAPIC_TPR, LAPIC_VERSION, LVT_MASKED, LVT_TIMER_PERIODIC,
    LVT_VECTOR_MASK, PIC1_DATA, PIC2_DATA, PIC_MASK_ALL, PIT_CH0_CMD, PIT_CH0_DATA, PIT_CH2_DATA,
    PIT_CMD_CH0_MODE0_LOHI, PIT_CMD_CH0_MODE2_LOHI, PIT_CMD_CH2_MODE2_LOHI, PIT_CMD_LATCH_CH0,
    PIT_CMD_LATCH_CH2, PIT_FREQUENCY_HZ, PIT_MAX_DIVISOR, PIT_SPEAKER_DATA, PIT_SPEAKER_GATE2,
    PIT_SPEAKER_PORT, SVR_APIC_ENABLE, SVR_VECTOR_MASK, TIMER_DIVIDE_BY_1, TIMER_DIVIDE_BY_128,
    TIMER_DIVIDE_BY_16, TIMER_DIVIDE_BY_2, TIMER_DIVIDE_BY_32, TIMER_DIVIDE_BY_4,
    TIMER_DIVIDE_BY_64, TIMER_DIVIDE_BY_8,
};
use vifast_plat_x86_64::boot::{MMIO_MAP_BASE, MMIO_MAP_BYTES};
use vifast_plat_x86_64::exc::{
    name as exc_name, stub_slot, STUB_COUNT, STUB_IPI, STUB_SPURIOUS, STUB_TIMER, STUB_UNKNOWN,
    VEC_IPI, VEC_SPURIOUS, VEC_TIMER, VEC_UNKNOWN, VEC_UNKNOWN_PROBE,
};

// ---------------------------------------------------------------------------------------------
// LAPIC — địa chỉ MMIO + thanh ghi
// ---------------------------------------------------------------------------------------------

/// Địa chỉ mặc định: LAPIC `0xFEE0_0000`, IOAPIC `0xFEC0_0000` (SDM Vol 3A §10.4.1, §11.5.1).
#[test]
fn apic_default_bases_literal() {
    assert_eq!(LAPIC_DEFAULT_BASE, 0xFEE0_0000);
    assert_eq!(IOAPIC_DEFAULT_BASE, 0xFEC0_0000);
    // Cả hai căn 4 KiB và nằm trong **MMIO map** 3..4 GiB của ảnh boot (KT-266 mở rộng bảng
    // trang: identity map 1 GiB của KT-257 **không** phủ 0xFEE0_0000).
    assert_eq!(LAPIC_DEFAULT_BASE & 0xFFF, 0);
    assert_eq!(IOAPIC_DEFAULT_BASE & 0xFFF, 0);
    for base in [LAPIC_DEFAULT_BASE, IOAPIC_DEFAULT_BASE] {
        assert!(
            base >= MMIO_MAP_BASE && base < MMIO_MAP_BASE + MMIO_MAP_BYTES,
            "0x{base:x} phải nằm trong MMIO map 0x{MMIO_MAP_BASE:x}..0x{:x}",
            MMIO_MAP_BASE + MMIO_MAP_BYTES
        );
    }
}

/// `IA32_APIC_BASE` (MSR `0x1B`): bit 8 = BSP, bit 11 = enable, base ở bit 12..51.
#[test]
fn ia32_apic_base_bits_literal() {
    assert_eq!(APIC_BASE_BSP, 1 << 8);
    assert_eq!(APIC_BASE_ENABLE, 1 << 11);
    assert_eq!(APIC_BASE_ADDR_MASK, 0x000F_FFFF_FFFF_F000);
    // Mặt nạ base **không** đụng ba bit điều khiển (8, 10, 11) và căn 4 KiB.
    assert_eq!(APIC_BASE_ADDR_MASK & APIC_BASE_ENABLE, 0);
    assert_eq!(APIC_BASE_ADDR_MASK & APIC_BASE_BSP, 0);
    assert_eq!(APIC_BASE_ADDR_MASK & 0xFFF, 0);
    // Base mặc định đọc qua mặt nạ vẫn là chính nó.
    assert_eq!(LAPIC_DEFAULT_BASE & APIC_BASE_ADDR_MASK, LAPIC_DEFAULT_BASE);
}

/// Offset thanh ghi LAPIC (SDM Vol 3A Bảng 10-1) — **giá trị tuyệt đối**.
#[test]
fn lapic_register_offsets_literal() {
    assert_eq!(LAPIC_ID, 0x020);
    assert_eq!(LAPIC_VERSION, 0x030);
    assert_eq!(LAPIC_TPR, 0x080);
    assert_eq!(LAPIC_EOI, 0x0B0);
    assert_eq!(LAPIC_SVR, 0x0F0);
    assert_eq!(LAPIC_LVT_TIMER, 0x320);
    assert_eq!(LAPIC_TIMER_INITIAL_COUNT, 0x380);
    assert_eq!(LAPIC_TIMER_CURRENT_COUNT, 0x390);
    assert_eq!(LAPIC_TIMER_DIVIDE, 0x3E0);
    // Mọi thanh ghi căn 16 byte (SDM: "registers are 32 bits wide and 16-byte aligned").
    for off in [
        LAPIC_ID,
        LAPIC_VERSION,
        LAPIC_TPR,
        LAPIC_EOI,
        LAPIC_SVR,
        LAPIC_LVT_TIMER,
        LAPIC_TIMER_INITIAL_COUNT,
        LAPIC_TIMER_CURRENT_COUNT,
        LAPIC_TIMER_DIVIDE,
    ] {
        assert_eq!(off & 0xF, 0, "offset 0x{off:x} phải căn 16 byte");
    }
}

/// 8259 PIC phải bị **che** khi dùng APIC: BIOS đặt vector IRQ0 = `0x08` (trùng `#DF`), và PIC
/// nối vào `LAPIC.LINT0` (virtual wire) ⇒ không che thì ngắt PIT giết ảnh ngay sau `sti`.
#[test]
fn pic_mask_literal() {
    assert_eq!(PIC1_DATA, 0x21);
    assert_eq!(PIC2_DATA, 0xA1);
    assert_eq!(PIC_MASK_ALL, 0xFF);
    // LINT0/LINT1 (SDM Bảng 10-1) — cũng bị mask để PIC không lọt qua đường virtual wire.
    assert_eq!(LAPIC_LVT_LINT0, 0x350);
    assert_eq!(LAPIC_LVT_LINT1, 0x360);
    assert_eq!(LAPIC_LVT_LINT0 & 0xF, 0);
    assert_eq!(LAPIC_LVT_LINT1 & 0xF, 0);
}

/// `LAPIC_ID`: ID nằm ở bit 24..31 (SDM Hình 10-6).#[test]
fn lapic_id_decode_literal() {
    assert_eq!(LAPIC_ID_SHIFT, 24);
    assert_eq!(lapic_id(0x0000_0000), 0);
    assert_eq!(lapic_id(0x0100_0000), 1);
    assert_eq!(lapic_id(0x0A00_0000), 10);
    assert_eq!(lapic_id(0xFF00_0000), 0xFF);
    // Bit thấp (reserved) không được lọt vào ID.
    assert_eq!(lapic_id(0x0000_00FF), 0);
}

/// `LAPIC_VERSION`: byte thấp là version (SDM Hình 10-7).
#[test]
fn lapic_version_decode_literal() {
    assert_eq!(lapic_version(0x0005_0014), 0x14);
    assert_eq!(lapic_version(0x0005_000F), 0x0F);
    assert_eq!(lapic_version(0x0005_0000), 0);
}

/// `SVR`: vector spurious ở bit 0..7, **bit 8 = APIC enable** (SDM §10.4.6).
#[test]
fn svr_encoding_literal() {
    assert_eq!(SVR_APIC_ENABLE, 1 << 8);
    assert_eq!(SVR_VECTOR_MASK, 0xFF);
    // Vector 255 + enable = 0x1FF — giá trị ảnh ghi vào SVR.
    assert_eq!(svr_value(255), 0x1FF);
    assert_eq!(svr_value(0xFF), 0x1FF);
    assert_eq!(svr_value(0x20), 0x120);
    // Vector 0 vẫn bật APIC (bit 8), không phải 0.
    assert_eq!(svr_value(0), 0x100);
    assert_eq!(svr_value(255) & SVR_APIC_ENABLE, SVR_APIC_ENABLE);
    assert_eq!(svr_value(255) & SVR_VECTOR_MASK, 255);
}

/// `LVT_TIMER`: vector bit 0..7, mask bit 16, periodic bit 17 (SDM §10.5.4).
#[test]
fn lvt_timer_encoding_literal() {
    assert_eq!(LVT_VECTOR_MASK, 0xFF);
    assert_eq!(LVT_MASKED, 1 << 16);
    assert_eq!(LVT_TIMER_PERIODIC, 1 << 17);
    // Vector 32, periodic, **không** mask = 0x0002_0020.
    assert_eq!(lvt_timer_value(32, true), 0x0002_0020);
    // Vector 32, one-shot = 0x0000_0020.
    assert_eq!(lvt_timer_value(32, false), 0x0000_0020);
    // Vector 255, periodic = 0x0002_00FF.
    assert_eq!(lvt_timer_value(255, true), 0x0002_00FF);
    // Giải mã ngược.
    assert_eq!(lvt_vector(0x0002_0020), 32);
    assert!(!lvt_is_masked(0x0002_0020));
    assert!(lvt_is_masked(0x0001_0020));
    assert_eq!(lvt_vector(0x0001_00FF), 255);
}

/// Bảng chia của `TIMER_DIVIDE` (SDM Bảng 10-2) — **mã 4 bit**, không phải số chia.
#[test]
fn timer_divide_table_literal() {
    assert_eq!(TIMER_DIVIDE_BY_1, 0b1011);
    assert_eq!(TIMER_DIVIDE_BY_2, 0b0000);
    assert_eq!(TIMER_DIVIDE_BY_4, 0b0001);
    assert_eq!(TIMER_DIVIDE_BY_8, 0b0010);
    assert_eq!(TIMER_DIVIDE_BY_16, 0b0011);
    assert_eq!(TIMER_DIVIDE_BY_32, 0b1000);
    assert_eq!(TIMER_DIVIDE_BY_64, 0b1001);
    assert_eq!(TIMER_DIVIDE_BY_128, 0b1010);
    // Khứ hồi mã ↔ giá trị cho **đủ tám** mức.
    for (code, value) in [
        (TIMER_DIVIDE_BY_1, 1u32),
        (TIMER_DIVIDE_BY_2, 2),
        (TIMER_DIVIDE_BY_4, 4),
        (TIMER_DIVIDE_BY_8, 8),
        (TIMER_DIVIDE_BY_16, 16),
        (TIMER_DIVIDE_BY_32, 32),
        (TIMER_DIVIDE_BY_64, 64),
        (TIMER_DIVIDE_BY_128, 128),
    ] {
        assert_eq!(timer_divide_value(code), Some(value));
        assert_eq!(timer_divide_code(value), Some(code));
    }
    // Mã không dùng (0b0100..0b0111, 0b1100..0b1111) ⇒ `None`, không đoán bừa.
    for code in [
        0b0100u32, 0b0101, 0b0110, 0b0111, 0b1100, 0b1101, 0b1110, 0b1111,
    ] {
        assert_eq!(
            timer_divide_value(code),
            None,
            "mã 0x{code:x} phải bị từ chối"
        );
    }
    // Số chia không phải luỹ thừa 2 trong bảng ⇒ `None`.
    for value in [0u32, 3, 5, 7, 9, 100, 256] {
        assert_eq!(
            timer_divide_code(value),
            None,
            "chia {value} phải bị từ chối"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// IOAPIC
// ---------------------------------------------------------------------------------------------

/// IOAPIC MMIO: `IOREGSEL` 0x00, `IOWIN` 0x10 (SDM §11.5.1).
#[test]
fn ioapic_mmio_literal() {
    assert_eq!(IOAPIC_REGSEL, 0x00);
    assert_eq!(IOAPIC_WIN, 0x10);
    assert_eq!(IOAPIC_ID_REG, 0x00);
    assert_eq!(IOAPIC_VER_REG, 0x01);
    assert_eq!(IOAPIC_REDTBL_BASE, 0x10);
    assert_eq!(IOAPIC_REDTBL_STRIDE, 2);
    assert_eq!(IOAPIC_MAX_REDIRECTIONS, 24);
    // 24 entry × 2 thanh ghi 32-bit ⇒ thanh ghi cuối = 0x10 + 47 = 0x3F (SDM: 0x10..0x3F).
    assert_eq!(
        IOAPIC_REDTBL_BASE + IOAPIC_MAX_REDIRECTIONS * IOAPIC_REDTBL_STRIDE - 1,
        0x3F
    );
}

/// `IOAPICID`: ID ở bit 24..27 (SDM Hình 11-6).
#[test]
fn ioapic_id_decode_literal() {
    assert_eq!(ioapic_id(0x0000_0000), 0);
    assert_eq!(ioapic_id(0x0100_0000), 1);
    assert_eq!(ioapic_id(0x0F00_0000), 0xF);
    // Bit ngoài 24..27 không lọt vào ID.
    assert_eq!(ioapic_id(0x00FF_FFFF), 0);
}

/// `IOAPICVER`: version bit 0..7, số entry tối đa bit 16..23 **+ 1** (SDM Hình 11-7).
#[test]
fn ioapic_version_decode_literal() {
    assert_eq!(ioapic_version(0x0017_0011), 0x11);
    // QEMU: VER = 0x0017_0011 ⇒ 0x17 + 1 = 24 redirection entry.
    assert_eq!(ioapic_redirection_count(0x0017_0011), 24);
    assert_eq!(ioapic_redirection_count(0x0000_0011), 1);
    assert_eq!(ioapic_redirection_count(0x00FF_0011), 256);
}

/// Redirection entry: bit 16 = mask (SDM §11.5.4). Mask **toàn bộ** là yêu cầu của KT-266.
#[test]
fn ioapic_redirection_mask_literal() {
    assert_eq!(IOAPIC_REDIR_MASKED, 1 << 16);
    // Entry đã cấu hình (vector 0x21, dest 0) ⇒ mask chỉ **bật** bit 16, không đụng bit khác.
    assert_eq!(ioapic_redir_low_masked(0x0000_0021), 0x0001_0021);
    assert!(ioapic_redir_is_masked(ioapic_redir_low_masked(0x0000_0021)));
    // Entry đã mask sẵn ⇒ giữ nguyên (idempotent).
    assert_eq!(ioapic_redir_low_masked(0x0001_0021), 0x0001_0021);
    // Entry 0 ⇒ 0x0001_0000.
    assert_eq!(ioapic_redir_low_masked(0), 0x0001_0000);
    assert!(!ioapic_redir_is_masked(0));
}

/// Thanh ghi redirection của pin `n`: `0x10 + 2n`; pin ngoài 24 ⇒ `None` (không ghi bừa).
#[test]
fn ioapic_redtbl_register_literal() {
    assert_eq!(ioapic_redtbl_reg(0), Some(0x10));
    assert_eq!(ioapic_redtbl_reg(1), Some(0x12));
    assert_eq!(ioapic_redtbl_reg(23), Some(0x3E));
    assert_eq!(ioapic_redtbl_reg(24), None);
    assert_eq!(ioapic_redtbl_reg(u32::MAX), None);
}

// ---------------------------------------------------------------------------------------------
// PIT — nguồn thời gian tham chiếu để **đo** hz
// ---------------------------------------------------------------------------------------------

/// PIT: kênh 0 ở cổng `0x40`, lệnh ở `0x43`, tần số 1,193182 MHz (Intel 8254).
#[test]
fn pit_constants_literal() {
    assert_eq!(PIT_CH0_DATA, 0x40);
    assert_eq!(PIT_CH0_CMD, 0x43);
    assert_eq!(PIT_FREQUENCY_HZ, 1_193_182);
    assert_eq!(PIT_MAX_DIVISOR, 0xFFFF);
    // Mode 0 (interrupt on terminal count), lobyte/hibyte, kênh 0, nhị phân:
    //   bit 7..6 = 00 (kênh 0), bit 5..4 = 11 (lo/hi), bit 3..1 = 000 (mode 0), bit 0 = 0.
    assert_eq!(PIT_CMD_CH0_MODE0_LOHI, 0x30);
    // Mode 2 (rate generator): bit 7..6 = 00, bit 5..4 = 11, bit 3..1 = 010, bit 0 = 0.
    assert_eq!(PIT_CMD_CH0_MODE2_LOHI, 0x34);
    // Lệnh latch: bit 7..6 = 00 (kênh 0), bit 5..4 = 00 (latch).
    assert_eq!(PIT_CMD_LATCH_CH0, 0x00);
    // Kênh 2 (đồng hồ đo độc lập): data 0x42, mode 2 = 0xB4, latch = 0x80.
    assert_eq!(PIT_CH2_DATA, 0x42);
    assert_eq!(PIT_CMD_CH2_MODE2_LOHI, 0xB4);
    assert_eq!(PIT_CMD_LATCH_CH2, 0x80);
    // Cổng loa: bit 0 = gate kênh 2 (phải bật), bit 1 = dữ liệu loa (phải tắt).
    assert_eq!(PIT_SPEAKER_PORT, 0x61);
    assert_eq!(PIT_SPEAKER_GATE2, 1 << 0);
    assert_eq!(PIT_SPEAKER_DATA, 1 << 1);
    assert_eq!(PIT_SPEAKER_GATE2 & PIT_SPEAKER_DATA, 0);
}

/// Đếm ngược 16-bit: `start - end` theo modulo 65536, **không** tràn thành số âm.
#[test]
fn pit_elapsed_ticks_literal() {
    assert_eq!(pit_elapsed_ticks(0xFFFF, 0xFFFF), 0);
    assert_eq!(pit_elapsed_ticks(0xFFFF, 0xD8F0), 0x270F);
    assert_eq!(pit_elapsed_ticks(0x8000, 0x4000), 0x4000);
    // Trường hợp bọc (counter chạy tiếp sau 0) vẫn cho hiệu đúng theo modulo.
    assert_eq!(pit_elapsed_ticks(0x0010, 0xFFF0), 0x0020);
}

/// `hz = ticks × 1_193_182 / pit_ticks` — số nguyên, không chia 0.
#[test]
fn hz_from_pit_literal() {
    // 10 ms ≈ 11_932 PIT tick; 10 LAPIC tick ⇒ 999 Hz (chia nguyên, làm tròn **xuống**).
    assert_eq!(hz_from_pit(10, 11_932), 999);
    // 11_931 PIT tick (10 ms − 1) ⇒ 1_000 Hz.
    assert_eq!(hz_from_pit(10, 11_931), 1_000);
    // 1 giây (1_193_182 PIT tick) có 1_000 tick ⇒ 1 kHz.
    assert_eq!(hz_from_pit(1_000, PIT_FREQUENCY_HZ), 1_000);
    // 0 tick ⇒ 0 Hz (không panic).
    assert_eq!(hz_from_pit(0, 11_932), 0);
    // Chia 0 ⇒ 0 (fail-closed, không panic trong ảnh).
    assert_eq!(hz_from_pit(10, 0), 0);
    // Làm tròn xuống: 1 tick / 11_932 ⇒ 99 Hz.
    assert_eq!(hz_from_pit(1, 11_932), 99);
}

// ---------------------------------------------------------------------------------------------
// Vector + slot stub — hợp đồng với IDT/asm
// ---------------------------------------------------------------------------------------------

/// Vector 32 = LAPIC timer, 255 = spurious; sentinel `isr_unknown` **dời** sang 254.
#[test]
fn apic_vectors_literal() {
    assert_eq!(VEC_TIMER, 32);
    assert_eq!(VEC_SPURIOUS, 255);
    // 255 giờ là vector **thật** (spurious) ⇒ sentinel của `isr_unknown` phải khác 255.
    assert_eq!(VEC_UNKNOWN, 254);
    assert_ne!(VEC_UNKNOWN, VEC_SPURIOUS as u64);
    assert_ne!(VEC_UNKNOWN, VEC_TIMER as u64);
}

/// Slot stub: timer 15, spurious 16, syscall 17 (KT-270), **IPI 18 (KT-278)**, tổng 19 — khớp
/// `asm::stub_addr`.
#[test]
fn apic_stub_slots_literal() {
    assert_eq!(STUB_TIMER, 15);
    assert_eq!(STUB_SPURIOUS, 16);
    assert_eq!(STUB_COUNT, 19);
    assert_eq!(stub_slot(VEC_TIMER), STUB_TIMER);
    assert_eq!(stub_slot(VEC_SPURIOUS), STUB_SPURIOUS);
    // **KT-278**: `0x30` **không còn** là "vector lạ" — nó là vector IPI có stub riêng. Vector
    // lạ của ca `isr_unknown` dời sang `0x31` (`VEC_UNKNOWN_PROBE`).
    assert_eq!(stub_slot(VEC_IPI), STUB_IPI);
    assert_eq!(stub_slot(VEC_UNKNOWN_PROBE), STUB_UNKNOWN);
    assert_eq!(stub_slot(0xFE), STUB_UNKNOWN);
}

/// Tên vector để in log: timer/spurious có tên riêng, sentinel vẫn là "unknown".
#[test]
fn apic_vector_names() {
    assert_eq!(exc_name(VEC_TIMER as u64), "timer");
    assert_eq!(exc_name(VEC_SPURIOUS as u64), "spurious");
    assert_eq!(exc_name(VEC_UNKNOWN), "unknown");
}

// ---------------------------------------------------------------------------------------------
// KT-297 — phép kiểm timer **periodic** + khoảng `hz` hợp lệ
// ---------------------------------------------------------------------------------------------

/// `lapic_timer_runs` phải chấp nhận **quay vòng** của LAPIC timer periodic.
///
/// KT-293 bắt 4/600 boot `runs=0` **giả** vì phép kiểm cũ đòi `count1 < count0` (giả định
/// one-shot). `LVT_TIMER` của ảnh đặt **periodic** (bit 17) ⇒ counter nạp lại, nên
/// `count1 > count0` là hợp lệ. Bốn cặp số dưới đây là **nguyên văn** log đỏ của KT-293.
#[test]
fn lapic_timer_runs_accepts_periodic_wrap() {
    const INITIAL: u32 = 1_000_000;
    // Nguyên văn bốn boot đỏ KT-293: count1 > count0 nhưng timer **đang chạy**.
    assert!(lapic_timer_runs(317_000, 803_001, INITIAL));
    assert!(lapic_timer_runs(233_000, 718_001, INITIAL));
    assert!(lapic_timer_runs(969_000, 975_002, INITIAL));
    assert!(lapic_timer_runs(18_003, 472_004, INITIAL));
    // Đếm lùi bình thường (không quay vòng) vẫn đúng.
    assert!(lapic_timer_runs(979_000, 283_000, INITIAL));
    // Ảnh chụp hiếm gặp `0` (vừa hết hạn, chưa nạp lại) vẫn là timer chạy nếu lần sau khác 0.
    assert!(lapic_timer_runs(0, 500_000, INITIAL));
}

/// `lapic_timer_runs` phải **fail-closed** khi timer không chạy hoặc đọc ra rác.
#[test]
fn lapic_timer_runs_rejects_stopped_or_garbage() {
    const INITIAL: u32 = 1_000_000;
    // Timer chưa nạp `INITIAL_COUNT` (hoặc đã dừng): counter đứng yên ở 0.
    assert!(!lapic_timer_runs(0, 0, INITIAL));
    // Counter đứng yên ở giá trị bất kỳ (timer bị tắt sau khi nạp).
    assert!(!lapic_timer_runs(500_000, 500_000, INITIAL));
    assert!(!lapic_timer_runs(INITIAL, INITIAL, INITIAL));
    // Không có APIC: MMIO trả `0xFFFF_FFFF` — ngoài dải kiến trúc ⇒ fail-closed.
    assert!(!lapic_timer_runs(0xFFFF_FFFF, 0xFFFF_FFFE, INITIAL));
    assert!(!lapic_timer_runs(0, 0xFFFF_FFFF, INITIAL));
    // Giá trị vượt `INITIAL_COUNT` không thể là counter hợp lệ.
    assert!(!lapic_timer_runs(INITIAL + 1, 500_000, INITIAL));
}

/// `hz_ok` chốt khoảng hợp lệ của nguồn tick 100 Hz **đo được** dưới TCG.
///
/// Đây là **kiểm đúng đắn** (đột biến `wrong-hz` phải ĐỎ), không phải nới ngưỡng: trước KT-297
/// cổng chỉ đòi `hz > 0` nên một timer chạy sai nhịp vẫn xanh.
#[test]
fn hz_ok_literal() {
    // Danh định 100 Hz — đo được trên ảnh thật (KT-293: 36..~150 Hz trên cửa sổ 2 tick).
    assert!(hz_ok(100));
    assert!(hz_ok(HZ_MIN));
    assert!(hz_ok(HZ_MAX));
    // 0 = không đo được (fail-closed).
    assert!(!hz_ok(0));
    // Sai nhịp 10× (đột biến `wrong-hz`): 1.000 Hz và 10 Hz đều ngoài khoảng.
    assert!(!hz_ok(1_000));
    assert!(!hz_ok(10));
    // Trần/dưới biên.
    assert!(!hz_ok(HZ_MIN - 1));
    assert!(!hz_ok(HZ_MAX + 1));
}

/// `pit_divisor_ok` chốt **divisor đã nạp** từ counter đọc ngay sau khi nạp mode 2.
///
/// Dưới TCG, tốc độ tick **giao được** bị chặn trên bởi độ trễ đánh thức `hlt` (~20 ms) nên
/// `hz` đo được **bão hoà** ~35..55 Hz cho cả nguồn 100 Hz lẫn 1.000 Hz. Kiểm divisor trực tiếp
/// là lớp bắt **đột biến `wrong-hz`** (đổi divisor) mà phép đo `hz` không phân biệt được.
#[test]
fn pit_divisor_ok_literal() {
    const DIV: u16 = 11_932;
    // Đọc ngay sau khi nạp: counter còn ≈ divisor (KT-293 đo được 0x2e31 = 11.825).
    assert!(pit_divisor_ok(11_825, DIV));
    assert!(pit_divisor_ok(DIV, DIV));
    // Đọc trễ tới nửa chu kỳ (5 ms ở 100 Hz) vẫn chấp nhận.
    assert!(pit_divisor_ok(DIV / 2, DIV));
    // Đột biến `wrong-hz` nhanh 10× (divisor 1.193): counter ≈ 1.125 ⇒ ngoài dải.
    assert!(!pit_divisor_ok(1_125, DIV));
    assert!(!pit_divisor_ok(1_193, DIV));
    // Đột biến chậm (divisor 65.535): counter > divisor ⇒ ngoài dải.
    assert!(!pit_divisor_ok(65_512, DIV));
    // Counter 0 (PIT không chạy / không nạp) ⇒ fail-closed.
    assert!(!pit_divisor_ok(0, DIV));
}
