// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::virtio_input` — KT-283 (K4' bước 3, virtio-input).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test virtio_input
//! ```
//!
//! Phần **thuần** của driver được chốt trước khi chạm phần cứng (D2-05). Sáu nhóm:
//!
//! 1. **Bố cục trang DMA**: ring `event` phải nằm **trọn** trước ring `status`, ring `status`
//!    trước vùng đệm sự kiện, và 8 đệm × 8 byte phải vừa trang 4 KiB — cùng bất biến
//!    `const _: () = assert!` của driver (test này là bản chạy được, in số đo khi lệch).
//! 2. **Leaf BAR riêng**: `pci::bar_assign_base(VirtioKind::Input)` = leaf 3 (`0x1060_0000`),
//!    **khác** leaf blk/net/gpu/console và nằm trong cửa sổ MMIO 32-bit — gán nhầm leaf là
//!    driver thứ hai ghi đè BAR của driver thứ nhất (bài học KT-275).
//! 3. **Device id**: `VirtioKind::Input` nhận `0x1052` (modern-only, QEMU `virtio-keyboard-pci`)
//!    **và** `0x1012` (transitional), **không** nhận id của loại khác.
//! 4. **Đổi VA nửa cao → PA**: driver DMA bằng PA, con trỏ trong ảnh là alias nửa cao; công thức
//!    `text_start_pa + (va - KERNEL_HIGH_BASE)` phải round-trip với `high_alias_va`.
//! 5. **Bảng chuyển mã là của crate thuần**: `KeyState`/`key_to_ascii` của `vifast_virtio::input`
//!    cho `'a'`/`'A'`/`None` — driver **không** có bảng thứ hai (D2-110B). Đây là ca chốt rằng
//!    driver aarch64 dùng **nguyên** codec đã có 16 test host bên riscv64.
//! 6. **Hằng số thiết bị**: 16 descriptor (luỹ thừa 2, ≥ 2), 8 đệm (≥ 5 cho lô `sendkey shift-a`),
//!    hai queue `event`/`status`, `virtio_input_event` 8 byte.

// Test này **ghim hằng số** của driver (bố cục trang DMA, leaf BAR, số queue) — khẳng định trên
// hằng là mục đích, không phải sơ suất (cùng khuôn `tests/virtio_gpu.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_aarch64::pci::{self, VirtioKind};
use vifast_plat_aarch64::split_mmu::{high_alias_va, KERNEL_HIGH_BASE};
use vifast_plat_aarch64::virtio_input::{
    high_to_pa, BUF_OFF, DMA_PAGE_BYTES, EVENT_BUFS, QUEUE_SIZE, RING_OFF, STATUS_RING_OFF,
};
use vifast_virtio::input::{
    key_to_ascii, InputEvent, KeyState, EVENT_BYTES, EV_KEY, EV_SYN, KEY_A, KEY_F1, KEY_LEFTSHIFT,
    QUEUES_REQUIRED, Q_EVENT, Q_STATUS, SYN_REPORT,
};
use vifast_virtio::queue::VqLayout;

/// PA liên kết của `__text_start` của ảnh (linker.ld) — dùng cho test đổi VA → PA.
const TEXT_START_PA: u64 = 0x4008_0000;

// ---------------------------------------------------------------- 1. bố cục trang DMA

#[test]
fn dma_layout_fits_one_page() {
    let layout = VqLayout::new(QUEUE_SIZE).expect("queue size hợp lệ");
    let ring = layout.total_bytes();
    assert_eq!(DMA_PAGE_BYTES, 4096, "trang DMA phải là 4 KiB");
    assert!(
        RING_OFF + ring <= STATUS_RING_OFF,
        "ring event {ring} B tại {RING_OFF:#x} tràn vào ring status {STATUS_RING_OFF:#x}"
    );
    assert!(
        STATUS_RING_OFF + ring <= BUF_OFF,
        "ring status {ring} B tại {STATUS_RING_OFF:#x} tràn vào vùng đệm {BUF_OFF:#x}"
    );
    assert!(
        BUF_OFF + EVENT_BUFS * EVENT_BYTES <= DMA_PAGE_BYTES,
        "8 đệm × 8 B tại {BUF_OFF:#x} tràn trang {DMA_PAGE_BYTES} B"
    );
    // Hai ring phải **rời nhau** — dùng chung một ring là driver thứ hai ghi đè `avail->idx`
    // của driver thứ nhất (hỏng im lặng, chỉ lộ khi thứ tự gọi đổi).
    assert!(RING_OFF + ring <= STATUS_RING_OFF);
    assert!(STATUS_RING_OFF >= RING_OFF + ring);
}

// ---------------------------------------------------------------- 2. leaf BAR riêng

#[test]
fn bar_leaf_is_distinct() {
    let input = pci::bar_assign_base(VirtioKind::Input);
    assert_eq!(input, 0x1060_0000, "leaf 3 của cửa sổ PCIe");
    for other in [
        VirtioKind::Blk,
        VirtioKind::Console,
        VirtioKind::Net,
        VirtioKind::Gpu,
    ] {
        assert_ne!(
            input,
            pci::bar_assign_base(other),
            "leaf của input trùng leaf của {}",
            other.name()
        );
    }
    assert!(input >= pci::PCI_MMIO_WINDOW_BASE);
    assert!(
        input + pci::BAR_ASSIGN_BYTES <= pci::PCI_MMIO_WINDOW_BASE + pci::PCI_MMIO_WINDOW_BYTES
    );
    // Căn 2 MiB ⇒ một bảng mức 3 phủ trọn leaf (cùng khuôn blk/net/gpu).
    assert_eq!(input % pci::BAR_ASSIGN_BYTES, 0);
}

// ---------------------------------------------------------------- 3. device id

#[test]
fn device_id_matches_modern_only() {
    assert!(
        VirtioKind::Input.matches(0x1af4, 0x1052),
        "modern-only 0x1052"
    );
    assert!(
        VirtioKind::Input.matches(0x1af4, 0x1012),
        "transitional 0x1012"
    );
    assert!(
        !VirtioKind::Input.matches(0x1af4, 0x1041),
        "0x1041 là virtio-net"
    );
    assert!(
        !VirtioKind::Input.matches(0x1af4, 0x1050),
        "0x1050 là virtio-gpu"
    );
    assert!(
        !VirtioKind::Input.matches(0x8086, 0x1052),
        "vendor khác virtio"
    );
}

// ---------------------------------------------------------------- 4. VA nửa cao → PA

#[test]
fn high_va_converts_back_to_pa() {
    // Một địa chỉ trong ảnh: PA liên kết + offset bất kỳ.
    let pa = TEXT_START_PA + 0x1234;
    let va = high_alias_va(pa, TEXT_START_PA, KERNEL_HIGH_BASE);
    assert!(va >= KERNEL_HIGH_BASE, "alias nửa cao phải ở nửa cao");
    assert_eq!(high_to_pa(va, TEXT_START_PA), pa, "round-trip VA → PA");
    // Trang DMA nằm trong `.bss` của ảnh ⇒ cùng công thức; lệch một hằng là DMA ra ngoài ảnh.
    let dma_pa = TEXT_START_PA + 0x2_0000;
    assert_eq!(
        high_to_pa(
            high_alias_va(dma_pa, TEXT_START_PA, KERNEL_HIGH_BASE),
            TEXT_START_PA
        ),
        dma_pa
    );
}

// ---------------------------------------------------------------- 5. bảng chuyển mã dùng chung

#[test]
fn keymap_comes_from_shared_codec() {
    // `sendkey a`: KEY_A xuống ⇒ 'a'; nhả ⇒ không sinh ký tự.
    let mut keys = KeyState::new();
    assert_eq!(
        keys.feed(InputEvent::new(EV_KEY, KEY_A, 1)),
        Some(b'a'),
        "KEY_A xuống phải ra 'a' (0x61)"
    );
    assert_eq!(keys.feed(InputEvent::new(EV_KEY, KEY_A, 0)), None);
    // `sendkey shift-a`: Shift xuống (không sinh ký tự) rồi KEY_A xuống ⇒ 'A'.
    let mut keys = KeyState::new();
    assert_eq!(keys.feed(InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 1)), None);
    assert!(keys.shift());
    assert_eq!(keys.feed(InputEvent::new(EV_KEY, KEY_A, 1)), Some(b'A'));
    // `EV_SYN` không sinh ký tự, không đổi Shift.
    assert_eq!(keys.feed(InputEvent::new(EV_SYN, SYN_REPORT, 0)), None);
    assert!(keys.shift());
    // `sendkey f1`: phím **không** có ASCII — ca âm của bảng.
    assert_eq!(key_to_ascii(KEY_F1, false), None);
    // Mã hoá/giải mã 8 byte: driver đọc `used->len` = 8 rồi `InputEvent::parse`.
    let raw = InputEvent::new(EV_KEY, KEY_A, 1).encode();
    assert_eq!(raw.len(), EVENT_BYTES);
    assert_eq!(
        InputEvent::parse(&raw),
        Some(InputEvent::new(EV_KEY, KEY_A, 1))
    );
    assert_eq!(
        InputEvent::parse(&raw[..EVENT_BYTES - 1]),
        None,
        "thiếu byte ⇒ None"
    );
}

// ---------------------------------------------------------------- 6. hằng số thiết bị

#[test]
fn device_constants_match_contract() {
    assert_eq!(QUEUE_SIZE, 16, "16 descriptor (luỹ thừa 2, ≥ 2)");
    assert!(QUEUE_SIZE.is_power_of_two());
    assert!(QUEUE_SIZE >= 2);
    assert_eq!(EVENT_BUFS, 8, "8 đệm arm sẵn");
    // Một lô `sendkey shift-a` là 5 sự kiện (Shift↓ A↓ A↑ Shift↑ SYN) — arm < 5 là mất cả lô.
    assert!(EVENT_BUFS >= 5);
    assert!(EVENT_BUFS as u16 <= QUEUE_SIZE);
    assert_eq!(
        EVENT_BYTES, 8,
        "virtio_input_event = u16 type + u16 code + u32 value"
    );
    assert_eq!(Q_EVENT, 0, "queue event = 0");
    assert_eq!(Q_STATUS, 1, "queue status = 1");
    assert_eq!(
        QUEUES_REQUIRED, 1,
        "codec chỉ **đòi** một queue; driver đặt cả hai"
    );
    // Hai queue phải nằm trong cùng trang DMA mà vẫn rời nhau (test 1 đã kiểm khoảng).
    assert!(STATUS_RING_OFF > RING_OFF);
}
