// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::virtio_blk` — KT-273 (K4' bước 3, virtio-blk).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test virtio_blk
//! ```
//!
//! Phần **thuần** của driver được chốt trước khi chạm phần cứng (D2-05):
//!
//! 1. **Bố cục trang DMA**: ring của `VqLayout` phải nằm **trọn** trước vùng request, request
//!    trước data, data trước status, status trong trang — cùng bất biến `const _: () = assert!`
//!    của driver (test này là bản chạy được, in ra số đo khi lệch).
//! 2. **Request dựng bằng `vifast_virtio::blk::build_request`** — cùng mã mà driver gửi: `IN` 1
//!    sector ⇒ `data_len = 512`, `status_index = 2`; `FLUSH` ⇒ `data_len = 0`, `status_index = 1`.
//! 3. **Bằng chứng sector 0**: hex 16 byte, ASCII chỉ khi in được, magic chỉ khi 8 byte đầu khớp
//!    — ba trường này là thứ runner đối chiếu với đĩa tạm, nên phải là **hàm thuần** test được.
//! 4. **Đổi VA nửa cao → PA**: driver DMA bằng PA, còn con trỏ trong ảnh là alias nửa cao; công
//!    thức `text_start_pa + (va - KERNEL_HIGH_BASE)` phải round-trip với `high_alias_va`.
//! 5. **Hằng số thiết bị**: sector 512 B, queue 8 descriptor (đủ chain 3), timeout > 0.

// Test này **ghim hằng số** của driver (địa chỉ ECAM, cửa sổ BAR, bố cục trang DMA) —
// khẳng định trên hằng là mục đích, không phải sơ suất (cùng khuôn `vifast-plat-x86_64/tests`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_aarch64::split_mmu::{high_alias_va, KERNEL_HIGH_BASE};
use vifast_plat_aarch64::virtio_blk::{
    ascii16, has_magic, hex16, high_to_pa, sector0_evidence, DATA_OFF, DMA_PAGE_BYTES, QUEUE_SIZE,
    REQ_OFF, SECTOR0_MAGIC, SECTOR_BYTES, STATUS_OFF,
};
use vifast_virtio::blk::{build_request, T_FLUSH, T_IN, T_OUT};
use vifast_virtio::queue::VqLayout;

/// PA liên kết của `__text_start` của ảnh (linker.ld) — dùng cho test đổi VA → PA.
const TEXT_START_PA: u64 = 0x4008_0000;

// ---------------------------------------------------------------- 1. bố cục trang DMA

#[test]
fn dma_layout_fits_one_page() {
    let layout = VqLayout::new(QUEUE_SIZE).expect("queue size hợp lệ");
    // Ring (desc + avail + used) phải nằm trọn **trước** vùng request.
    assert!(
        layout.end <= REQ_OFF,
        "ring kết thúc ở {} > REQ_OFF {REQ_OFF}",
        layout.end
    );
    // Header 16 B, data 512 B, status 1 B — không chồng nhau và nằm trong trang.
    assert!(REQ_OFF + 16 <= DATA_OFF);
    assert!(DATA_OFF + SECTOR_BYTES <= STATUS_OFF);
    assert!(STATUS_OFF < DMA_PAGE_BYTES);
    // Queue phải chứa được chain 3 descriptor (header → data → status).
    assert!(QUEUE_SIZE >= 3);
    assert!(QUEUE_SIZE.is_power_of_two());
    // Sector của thiết bị = sector của `vifast_fs::BLOCK_SIZE` (512).
    assert_eq!(SECTOR_BYTES, 512);
    assert_eq!(DMA_PAGE_BYTES, 4096);
}

// ---------------------------------------------------------------- 2. request

#[test]
fn request_matches_driver_contract() {
    let read = build_request(T_IN, 0, 1);
    assert_eq!(read.data_len, SECTOR_BYTES as u32);
    assert_eq!(read.status_index, 2);
    assert_eq!(read.sector, 0);
    // Header 16 byte: `type` little-endian ở 0..4, `sector` ở 8..16.
    assert_eq!(&read.header[0..4], &T_IN.to_le_bytes());
    assert_eq!(&read.header[8..16], &0u64.to_le_bytes());

    let write = build_request(T_OUT, 7, 1);
    assert_eq!(write.data_len, SECTOR_BYTES as u32);
    assert_eq!(&write.header[8..16], &7u64.to_le_bytes());

    // FLUSH không có đệm dữ liệu ⇒ status là descriptor **thứ hai** (index 1).
    let flush = build_request(T_FLUSH, 0, 0);
    assert_eq!(flush.data_len, 0);
    assert_eq!(flush.status_index, 1);
}

// ---------------------------------------------------------------- 3. bằng chứng sector 0

#[test]
fn sector0_hex_is_lowercase_and_exact() {
    let mut bytes = [0u8; 16];
    bytes[0] = 0xde;
    bytes[1] = 0xad;
    bytes[15] = 0x01;
    let hex = hex16(&bytes);
    assert_eq!(&hex[0..4], b"dead");
    assert_eq!(&hex[30..32], b"01");
    // Toàn bộ 32 ký tự đều là hex thường (runner grep `[0-9a-f]{32}`).
    assert!(hex
        .iter()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)));
}

#[test]
fn sector0_ascii_only_when_printable() {
    let mut text = [0u8; 16];
    text.copy_from_slice(b"VIFASTOS-KT273-S");
    assert_eq!(ascii16(&text), Some(text));

    let mut binary = text;
    binary[3] = 0x00; // NUL không in được
    assert_eq!(ascii16(&binary), None);
    binary[3] = 0x7f; // DEL cũng không
    assert_eq!(ascii16(&binary), None);
    binary[3] = 0x20; // space là in được
    assert_eq!(ascii16(&binary), Some(binary));
}

#[test]
fn sector0_magic_requires_exact_prefix() {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&SECTOR0_MAGIC);
    assert!(has_magic(&bytes));
    // Lệch một byte ⇒ không phải chữ ký (không "gần đúng").
    bytes[7] ^= 0x01;
    assert!(!has_magic(&bytes));
    // Ngắn hơn chữ ký ⇒ không đọc ngoài lát.
    assert!(!has_magic(&SECTOR0_MAGIC[..4]));
}

#[test]
fn sector0_evidence_bundles_three_fields() {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(b"VIFASTOS-KT273-S");
    let evidence = sector0_evidence(&bytes);
    assert!(evidence.magic);
    assert_eq!(evidence.ascii, Some(*b"VIFASTOS-KT273-S"));
    assert_eq!(&evidence.hex[0..16], b"5649464153544f53"); // "VIFASTOS"
                                                           // Đĩa không có chữ ký: vẫn in hex, magic = 0, ascii = None nếu có byte nhị phân.
    let zeros = [0u8; 16];
    let empty = sector0_evidence(&zeros);
    assert!(!empty.magic);
    assert_eq!(empty.ascii, None);
    assert_eq!(&empty.hex[..], b"00000000000000000000000000000000");
}

// ---------------------------------------------------------------- 4. VA nửa cao → PA

#[test]
fn high_va_converts_back_to_pa() {
    // Alias nửa cao của PA `TEXT_START_PA + 0x3000` là `KERNEL_HIGH_BASE + 0x3000`.
    let pa = TEXT_START_PA + 0x3000;
    let va = high_alias_va(pa, TEXT_START_PA, KERNEL_HIGH_BASE);
    assert_eq!(high_to_pa(va, TEXT_START_PA), pa);
    // Đầu ảnh: VA nửa cao ↔ PA liên kết.
    assert_eq!(high_to_pa(KERNEL_HIGH_BASE, TEXT_START_PA), TEXT_START_PA);
    // Trang DMA nằm trong `.bss` của ảnh ⇒ cùng công thức, không có nhánh riêng.
    let dma_pa = TEXT_START_PA + 0x2_0000;
    let dma_va = high_alias_va(dma_pa, TEXT_START_PA, KERNEL_HIGH_BASE);
    assert_eq!(high_to_pa(dma_va, TEXT_START_PA), dma_pa);
    // PA phải căn 4 KiB để descriptor DMA hợp lệ.
    assert_eq!(dma_pa & 0xfff, 0);
}
