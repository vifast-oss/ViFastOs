// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_x86_64::virtio_blk` — KT-288 (K5' bước 9, virtio-blk).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test virtio_blk
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
//! 4. **Chữ ký với runner**: `SECTOR0_MAGIC` phải là **tiền tố** của chuỗi 17 byte mà
//!    `run-x86_64.sh` ghi vào đĩa tạm (`VIFASTOS-KT288-S0`) — lệch một byte là `magic=0` và
//!    runner ĐỎ, nên hợp đồng này phải có test chứ không chỉ nằm trong chú thích.
//! 5. **BAR rời nhau**: driver blk phải dùng **leaf riêng**, không đè BAR của virtio-net — hàm
//!    thuần `bar_disjoint` là chỗ duy nhất quyết định điều đó.
//! 6. **Hằng số thiết bị**: sector 512 B, queue 8 descriptor (đủ chain 3), timeout > 0, và
//!    `VirtioKind::Blk` khớp đúng hai device id 0x1001/0x1042 (không khớp net/gpu).

// Test này **ghim hằng số** của driver (bố cục trang DMA, chữ ký sector 0, cỡ queue) — khẳng
// định trên hằng là mục đích, không phải sơ suất (cùng khuôn `tests/virtio_net.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::virtio_blk::{
    ascii16, bar_disjoint, has_magic, hex16, sector0_evidence, DATA_OFF, DMA_PAGE_BYTES,
    POLL_TIMEOUT_US, QUEUE_SIZE, REQ_OFF, SECTOR0_MAGIC, SECTOR_BYTES, STATUS_OFF,
};
use vifast_virtio::blk::{build_request, T_FLUSH, T_IN, T_OUT};
use vifast_virtio::pci::VirtioKind;
use vifast_virtio::queue::VqLayout;

/// Chuỗi 17 byte mà `run-x86_64.sh` ghi vào đầu đĩa tạm (hợp đồng runner ↔ ảnh).
const RUNNER_SIGNATURE: &[u8; 17] = b"VIFASTOS-KT288-S0";

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
    // Trần poll phải có thật (spec §2.7.13: không ngắt thì driver poll, phải có trần).
    assert!(POLL_TIMEOUT_US > 0);
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
    text.copy_from_slice(b"VIFASTOS-KT288-S");
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
    bytes.copy_from_slice(b"VIFASTOS-KT288-S");
    let evidence = sector0_evidence(&bytes);
    assert!(evidence.magic);
    assert_eq!(evidence.ascii, Some(*b"VIFASTOS-KT288-S"));
    assert_eq!(&evidence.hex[0..16], b"5649464153544f53"); // "VIFASTOS"
                                                           // Đĩa không có chữ ký: vẫn in hex, magic = 0, ascii = None nếu có byte nhị phân.
    let zeros = [0u8; 16];
    let empty = sector0_evidence(&zeros);
    assert!(!empty.magic);
    assert_eq!(empty.ascii, None);
    assert_eq!(&empty.hex[..], b"00000000000000000000000000000000");
}

// ---------------------------------------------------------------- 4. hợp đồng với runner

#[test]
fn magic_is_prefix_of_runner_signature() {
    // Runner ghi 17 byte `VIFASTOS-KT288-S0` vào sector 0; ảnh chỉ in `magic=1` khi 8 byte đầu
    // khớp `SECTOR0_MAGIC`. Hai bên phải nói **cùng** một chữ ký.
    assert_eq!(&SECTOR0_MAGIC[..], b"VIFASTOS");
    assert_eq!(&RUNNER_SIGNATURE[..SECTOR0_MAGIC.len()], &SECTOR0_MAGIC[..]);
    // 16 byte đầu của đĩa tạm (thứ runner băm lại bằng `od`) — hex phải đúng chuỗi này.
    let mut first = [0u8; 16];
    first.copy_from_slice(&RUNNER_SIGNATURE[..16]);
    assert_eq!(&hex16(&first)[..], b"5649464153544f532d4b543238382d53");
    assert!(has_magic(&first));
}

// ---------------------------------------------------------------- 5. BAR rời nhau

#[test]
fn bar_disjoint_rejects_overlap() {
    // Hai leaf 4 KiB liền nhau: rời nhau.
    assert!(bar_disjoint(0xfe00_0000, 0x1000, 0xfe00_1000, 0x1000));
    // Cùng một leaf ⇒ **không** rời (đây là lỗi "blk dùng nhầm BAR của net").
    assert!(!bar_disjoint(0xfe00_0000, 0x1000, 0xfe00_0000, 0x1000));
    // Chồng một phần ⇒ không rời.
    assert!(!bar_disjoint(0xfe00_0000, 0x2000, 0xfe00_1000, 0x1000));
    // BAR cỡ 0 hoặc chưa gán (0) ⇒ không bao giờ "rời nhau" (fail-closed).
    assert!(!bar_disjoint(0xfe00_0000, 0, 0xfe00_1000, 0x1000));
    assert!(!bar_disjoint(0, 0x1000, 0xfe00_1000, 0x1000));
    // Tràn 64 bit không được làm phép kiểm thành "rời".
    assert!(!bar_disjoint(u64::MAX - 0x10, 0x1000, 0x1000, 0x1000));
}

// ---------------------------------------------------------------- 6. hằng số thiết bị

#[test]
fn device_ids_are_virtio_blk() {
    // Hai device id mà driver nhận (transitional + modern-only), vendor Red Hat/QEMU.
    assert!(VirtioKind::Blk.matches(0x1af4, 0x1001));
    assert!(VirtioKind::Blk.matches(0x1af4, 0x1042));
    // Không nhận nhầm thiết bị khác (net 0x1000/0x1041, gpu 0x1050/0x1010).
    assert!(!VirtioKind::Blk.matches(0x1af4, 0x1000));
    assert!(!VirtioKind::Blk.matches(0x1af4, 0x1041));
    assert!(!VirtioKind::Blk.matches(0x1af4, 0x1050));
    // Vendor khác ⇒ không khớp, dù device id trùng.
    assert!(!VirtioKind::Blk.matches(0x8086, 0x1001));
}
