// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho phần dữ liệu boot x86_64 — KT-257 (K5' bước 1).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test boot
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-257: hai mô-đun [`serial`] và [`boot`] chưa tồn tại
//! khi tệp này được viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do). Sau khi
//! hiện thực, các khẳng định dưới đây chốt **giá trị tuyệt đối** mà asm của ảnh
//! `vifast-kbin-x86_64` dùng làm `const` operand — nếu ai đổi một hằng, test đỏ **trước** khi có
//! ảnh boot, đúng bài học "xanh giả" của audit K5' §5 R6.
//!
//! Nguồn giá trị:
//!
//! - **16550 UART**: PC16550D datasheet — thanh ghi `RBR/THR/DLL` (0), `IER/DLM` (1), `IIR/FCR`
//!   (2), `LCR` (3), `MCR` (4), `LSR` (5), `MSR` (6), `SCR` (7); `LSR.THRE` bit 5, `LSR.TEMT`
//!   bit 6; `LCR.DLAB` bit 7; `LCR` 8N1 = `0b0000_0011`; `FCR` bật+xoá FIFO ngưỡng 14 = `0xC7`;
//!   `MCR` DTR|RTS|OUT2 = `0x0B`. QEMU `-nographic` nối COM1 ra stdio.
//! - **Intel SDM Vol 3A** ch. 4 (paging) + §9.8.5 (Initializing IA-32e Mode): thứ tự bật long
//!   mode `CR4.PAE` → `CR3` → `IA32_EFER.LME` → `CR0.PG|PE` → `ljmp` vào đoạn code 64-bit.
//! - **Multiboot2 specification** §3.1: magic `0xE85250D6`, architecture `0` (i386 protected
//!   mode), header dài 24 byte, checksum sao cho tổng 32-bit bằng 0, end tag `{0, 0, 8}`.
//! - **Xen PVH boot protocol** (`docs/misc/pvh.pandoc`): note tên `"Xen"`, type
//!   `XEN_ELFNOTE_PHYS32_ENTRY = 18`, desc 4 byte = địa chỉ entry 32-bit; `hvm_start_info.magic`
//!   = `0x336ec578`.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà asm của
// ảnh boot nạp (địa chỉ, bit, checksum). Clippy coi `assert!(HẰNG < HẰNG)` là "constant value";
// đó chính là mục đích của tệp này — một hằng đổi giá trị thì test phải đỏ.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::boot::{
    gdt_kernel_code, gdt_kernel_data, mmio_pd_entry, pd_entry, table_entry, LongModeStep,
    BOOT_LOAD_ADDR, GDT_KERNEL_CODE, GDT_KERNEL_DATA, GDT_NULL, IDENTITY_MAP_BYTES,
    LARGE_PAGE_BYTES, LONG_MODE_SEQUENCE, MMIO_MAP_BASE, MMIO_MAP_BYTES, MMIO_PDPT_INDEX,
    MULTIBOOT2_ARCH_I386, MULTIBOOT2_CHECKSUM, MULTIBOOT2_END_TAG_SIZE, MULTIBOOT2_HEADER_LEN,
    MULTIBOOT2_MAGIC, PD_ENTRIES, PD_MMIO_FLAGS, PVH_NOTE_NAME, PVH_NOTE_TYPE, PVH_START_MAGIC,
};
use vifast_plat_x86_64::serial::{
    divisor, init_sequence, COM1_BASE, FCR_ENABLE_CLEAR_14, INIT_STEPS, LCR_8N1, LCR_DLAB, LSR_DR,
    LSR_TEMT, LSR_THRE, MCR_DTR_RTS_OUT2, REG_DATA, REG_FCR, REG_IER, REG_LCR, REG_LSR, REG_MCR,
    REG_MSR, REG_SCR, TX_WAIT_LIMIT, UART_CLOCK_HZ,
};

// ---------------------------------------------------------------------------------------------
// COM1 — 16550 tại 0x3F8
// ---------------------------------------------------------------------------------------------

/// Địa chỉ gốc COM1 theo PC standard (và theo QEMU `-nographic`).
#[test]
fn com1_base_and_register_offsets() {
    assert_eq!(COM1_BASE, 0x3F8);
    // Thứ tự thanh ghi 16550 là **bất biến phần cứng**, không phải lựa chọn thiết kế.
    assert_eq!(REG_DATA, 0);
    assert_eq!(REG_IER, 1);
    assert_eq!(REG_FCR, 2);
    assert_eq!(REG_LCR, 3);
    assert_eq!(REG_MCR, 4);
    assert_eq!(REG_LSR, 5);
    assert_eq!(REG_MSR, 6);
    assert_eq!(REG_SCR, 7);
    // Không hai thanh ghi nào trùng địa chỉ.
    let mut seen = [false; 8];
    for offset in [
        REG_DATA, REG_IER, REG_FCR, REG_LCR, REG_MCR, REG_LSR, REG_MSR, REG_SCR,
    ] {
        assert!(!seen[offset as usize], "offset {offset} trùng");
        seen[offset as usize] = true;
    }
}

/// Bit trạng thái đường truyền — `THRE` là bit **5**, đúng thứ `write_byte` phải chờ.
#[test]
fn lsr_bits_literal_16550() {
    assert_eq!(LSR_DR, 0x01);
    assert_eq!(LSR_THRE, 0x20);
    assert_eq!(LSR_TEMT, 0x40);
    // `THRE` và `TEMT` là hai bit khác nhau: chờ nhầm bit là lỗi im lặng.
    assert_ne!(LSR_THRE, LSR_TEMT);
}

/// Khung truyền 8N1 + `DLAB` + FIFO + modem control — giá trị tuyệt đối.
#[test]
fn line_control_and_fifo_values() {
    assert_eq!(LCR_DLAB, 0x80);
    assert_eq!(LCR_8N1, 0x03);
    assert_eq!(FCR_ENABLE_CLEAR_14, 0xC7);
    assert_eq!(MCR_DTR_RTS_OUT2, 0x0B);
    // 8N1: word length 8 (`0b11`) + 1 stop bit (bit 2 = 0) + no parity (bit 3 = 0).
    assert_eq!(LCR_8N1 & 0x03, 0x03);
    assert_eq!(LCR_8N1 & 0x04, 0x00);
    assert_eq!(LCR_8N1 & 0x08, 0x00);
}

/// `divisor = clock / (16 × baud)` — tính tay, không so với công thức trong mã.
#[test]
fn divisor_literal_values() {
    assert_eq!(UART_CLOCK_HZ, 1_843_200);
    assert_eq!(divisor(115_200), Some((1, 0)));
    assert_eq!(divisor(57_600), Some((2, 0)));
    assert_eq!(divisor(9_600), Some((12, 0)));
    assert_eq!(divisor(300), Some((0x80, 0x01))); // 384 = 0x0180
    assert_eq!(divisor(50), Some((0x00, 0x09))); // 2304 = 0x0900
}

/// Ca âm: baud không biểu diễn được ⇒ `None` (không được chia cho 0 hay trả rác).
#[test]
fn divisor_rejects_impossible_baud() {
    assert_eq!(divisor(0), None);
    // 1.8432 MHz / (16 × 2 MHz) = 0 ⇒ không có divisor hợp lệ.
    assert_eq!(divisor(2_000_000), None);
    // 1.8432 MHz / (16 × 1) = 115200 > 65535 ⇒ không vừa 16 bit.
    assert_eq!(divisor(1), None);
}

/// Chuỗi khởi tạo: **thứ tự** là một phần của hợp đồng (`DLAB` trước divisor, tắt `DLAB` trước
/// khi đặt khung, FIFO trước modem control).
#[test]
fn init_sequence_order_and_values() {
    let steps = init_sequence((1, 0));
    assert_eq!(INIT_STEPS, 6);
    assert_eq!(steps.len(), INIT_STEPS);

    // 1. LCR.DLAB = 1 để hai thanh ghi 0/1 thành DLL/DLM.
    assert_eq!(steps[0].offset, REG_LCR);
    assert_eq!(steps[0].value, LCR_DLAB);
    // 2. DLL = divisor thấp; 3. DLM = divisor cao.
    assert_eq!(steps[1].offset, REG_DATA);
    assert_eq!(steps[1].value, 1);
    assert_eq!(steps[2].offset, REG_IER);
    assert_eq!(steps[2].value, 0);
    // 4. LCR = 8N1 (đồng thời tắt DLAB).
    assert_eq!(steps[3].offset, REG_LCR);
    assert_eq!(steps[3].value, LCR_8N1);
    assert_eq!(steps[3].value & LCR_DLAB, 0);
    // 5. FCR: bật + xoá FIFO.
    assert_eq!(steps[4].offset, REG_FCR);
    assert_eq!(steps[4].value, FCR_ENABLE_CLEAR_14);
    // 6. MCR: DTR + RTS + OUT2.
    assert_eq!(steps[5].offset, REG_MCR);
    assert_eq!(steps[5].value, MCR_DTR_RTS_OUT2);

    // Không bước nào ghi vào LSR (chỉ đọc) hay SCR (không dùng).
    for step in steps {
        assert_ne!(step.offset, REG_LSR);
        assert_ne!(step.offset, REG_SCR);
    }
}

/// Trần chờ `THRE` phải hữu hạn và đủ lớn để một lần ghi bình thường không rơi vào nhánh bỏ.
#[test]
fn tx_wait_limit_is_finite() {
    assert!(TX_WAIT_LIMIT > 0);
    assert!(TX_WAIT_LIMIT >= 1_000);
}

// ---------------------------------------------------------------------------------------------
// Long mode — bảng trang 4 mức, 2 MiB/page, identity map
// ---------------------------------------------------------------------------------------------

/// Entry bảng trang: bit `P|RW|PS` cho lá 2 MiB, `P|RW` cho bảng.
#[test]
fn page_table_entry_encoding_literal() {
    assert_eq!(BOOT_LOAD_ADDR, 0x10_0000);
    assert_eq!(LARGE_PAGE_BYTES, 0x20_0000);
    assert_eq!(PD_ENTRIES, 512);
    // Lá 2 MiB: PA | P | RW | PS = 0 | 0x1 | 0x2 | 0x80.
    assert_eq!(pd_entry(0), 0x83);
    assert_eq!(pd_entry(1), 0x20_0083);
    assert_eq!(pd_entry(2), 0x40_0083);
    assert_eq!(pd_entry(511), 0x3FE0_0083);
    // Bảng: PA | P | RW (không PS).
    assert_eq!(table_entry(0x102000), 0x102003);
    assert_eq!(table_entry(0x103000), 0x103003);
    // Mọi entry đều 4 KiB-aligned ở phần địa chỉ.
    for index in [0usize, 1, 255, 511] {
        assert_eq!(pd_entry(index) & 0xFFF, 0x83);
    }
}

/// Identity map 1 GiB phủ **toàn bộ** ảnh kernel (nạp ở 1 MiB) và vùng MMIO thấp (VGA 0xA0000).
#[test]
fn identity_map_covers_kernel_and_low_mmio() {
    assert_eq!(IDENTITY_MAP_BYTES, 1 << 30);
    assert_eq!(IDENTITY_MAP_BYTES / LARGE_PAGE_BYTES, PD_ENTRIES as u64);
    assert!(BOOT_LOAD_ADDR < IDENTITY_MAP_BYTES);
    // VGA framebuffer text 0xB8000 và vùng ROM 0xC0000 nằm trong map.
    assert!(0xB_8000 < IDENTITY_MAP_BYTES);
    assert!(0xC_0000 < IDENTITY_MAP_BYTES);
    // Entry cuối kết thúc đúng cuối map (không hở, không tràn).
    let last = pd_entry(PD_ENTRIES - 1) & !0xFFF;
    assert_eq!(last + LARGE_PAGE_BYTES, IDENTITY_MAP_BYTES);
}

/// **KT-266**: MMIO map 3..4 GiB — LAPIC/IOAPIC nằm ngoài identity map 1 GiB.
#[test]
fn mmio_map_covers_lapic_and_ioapic() {
    assert_eq!(MMIO_MAP_BASE, 0xC000_0000);
    assert_eq!(MMIO_MAP_BYTES, 1 << 30);
    assert_eq!(MMIO_PDPT_INDEX, 3);
    // `PDPT[3]` phủ đúng `3 GiB..4 GiB` (mỗi PDPT entry 1 GiB).
    assert_eq!(MMIO_PDPT_INDEX as u64 * (1 << 30), MMIO_MAP_BASE);
    assert_eq!(MMIO_MAP_BYTES / LARGE_PAGE_BYTES, PD_ENTRIES as u64);
    // Lá 2 MiB của vùng MMIO: P|RW|PS|PCD = 0x93, địa chỉ bắt đầu từ 3 GiB.
    assert_eq!(PD_MMIO_FLAGS, 0x93);
    assert_eq!(mmio_pd_entry(0), 0xC000_0093);
    assert_eq!(mmio_pd_entry(1), 0xC020_0093);
    assert_eq!(mmio_pd_entry(511), 0xFFE0_0093);
    // LAPIC `0xFEE0_0000` và IOAPIC `0xFEC0_0000` nằm trong map, đúng chỉ số entry.
    for base in [0xFEE0_0000u64, 0xFEC0_0000] {
        assert!(base >= MMIO_MAP_BASE && base < MMIO_MAP_BASE + MMIO_MAP_BYTES);
        let index = ((base - MMIO_MAP_BASE) / LARGE_PAGE_BYTES) as usize;
        assert_eq!(
            mmio_pd_entry(index) & !0xFFF,
            base & !(LARGE_PAGE_BYTES - 1)
        );
    }
    // Entry cuối kết thúc đúng 4 GiB.
    let last = mmio_pd_entry(PD_ENTRIES - 1) & !0xFFF;
    assert_eq!(last + LARGE_PAGE_BYTES, MMIO_MAP_BASE + MMIO_MAP_BYTES);
}

/// Thứ tự bật long mode theo Intel SDM §9.8.5 — sai thứ tự là triple fault, không phải "chậm".
#[test]
fn long_mode_sequence_order() {
    assert_eq!(LONG_MODE_SEQUENCE.len(), 5);
    assert_eq!(LONG_MODE_SEQUENCE[0], LongModeStep::Cr4Pae);
    assert_eq!(LONG_MODE_SEQUENCE[1], LongModeStep::Cr3);
    assert_eq!(LONG_MODE_SEQUENCE[2], LongModeStep::EferLme);
    assert_eq!(LONG_MODE_SEQUENCE[3], LongModeStep::Cr0PgPe);
    assert_eq!(LONG_MODE_SEQUENCE[4], LongModeStep::Ljmp64);
    // Tên dùng trong log/asm phải ổn định và không rỗng.
    for step in LONG_MODE_SEQUENCE {
        assert!(!step.name().is_empty());
    }
}

/// GDT tối thiểu: null + code 64-bit (`L=1`, access `0x9A`) + data (`0x92`).
#[test]
fn gdt_descriptors_literal() {
    assert_eq!(GDT_NULL, 0);
    assert_eq!(GDT_KERNEL_CODE, 0x0020_9A00_0000_0000);
    assert_eq!(GDT_KERNEL_DATA, 0x0000_9200_0000_0000);
    // Access byte ở bit 40..47.
    assert_eq!((GDT_KERNEL_CODE >> 40) & 0xFF, 0x9A);
    assert_eq!((GDT_KERNEL_DATA >> 40) & 0xFF, 0x92);
    // `L` (long mode) là bit 53; `D/B` bit 54 phải 0 khi `L=1`.
    assert_eq!((GDT_KERNEL_CODE >> 53) & 1, 1);
    assert_eq!((GDT_KERNEL_CODE >> 54) & 1, 0);
    // Hàm dựng phải trả đúng hằng (asm dùng hằng, không dùng hàm).
    assert_eq!(gdt_kernel_code(), GDT_KERNEL_CODE);
    assert_eq!(gdt_kernel_data(), GDT_KERNEL_DATA);
}

// ---------------------------------------------------------------------------------------------
// Header boot: multiboot2 (dự phòng GRUB) + PVH note (đường QEMU `-kernel` dùng)
// ---------------------------------------------------------------------------------------------

/// Multiboot2 header: magic, architecture 0, dài 24, checksum làm tổng 32-bit bằng 0.
#[test]
fn multiboot2_header_checksum() {
    assert_eq!(MULTIBOOT2_MAGIC, 0xE852_50D6);
    assert_eq!(MULTIBOOT2_ARCH_I386, 0);
    assert_eq!(MULTIBOOT2_HEADER_LEN, 24);
    assert_eq!(MULTIBOOT2_END_TAG_SIZE, 8);
    // checksum = 2^32 − (magic + arch + len) — tính tay.
    assert_eq!(MULTIBOOT2_CHECKSUM, 0x17AD_AF12);
    let sum = MULTIBOOT2_MAGIC
        .wrapping_add(MULTIBOOT2_ARCH_I386)
        .wrapping_add(MULTIBOOT2_HEADER_LEN)
        .wrapping_add(MULTIBOOT2_CHECKSUM);
    assert_eq!(sum, 0);
}

/// PVH note: tên `"Xen"`, type 18, magic `hvm_start_info` — đường boot **thật** của QEMU 11.1.3.
#[test]
fn pvh_note_constants() {
    assert_eq!(PVH_NOTE_TYPE, 18);
    assert_eq!(PVH_NOTE_NAME, b"Xen\0");
    assert_eq!(PVH_NOTE_NAME.len(), 4);
    assert_eq!(PVH_START_MAGIC, 0x336E_C578);
}
