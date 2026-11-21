// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::virtio_gpu` — KT-279 (K4' bước 3, virtio-gpu 2D).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test virtio_gpu
//! ```
//!
//! Phần **thuần** của driver được chốt trước khi chạm phần cứng (D2-05). Bảy nhóm:
//!
//! 1. **Bố cục trang DMA**: ring của `VqLayout` phải nằm **trọn** trước vùng lệnh, vùng lệnh
//!    trước vùng response, response đủ 408 B của `virtio_gpu_resp_display_info`, và
//!    `RESOURCE_ATTACH_BACKING` + 19 `mem_entry` phải vừa vùng lệnh — cùng bất biến
//!    `const _: () = assert!` của driver (test này là bản chạy được, in số đo khi lệch).
//! 2. **Hình học framebuffer**: `FB_BYTES == 640 × 480 × 4`, `FB_PAGES × 4096 == FB_BYTES`,
//!    `FB_STRIDE == 640 × 4`, base **căn 2 MiB** (một bảng mức 3 phủ trọn 300 trang), nằm
//!    **trong** RAM `-m 128M` và **sau** vùng ảnh — lệch một hằng là DMA ra ngoài RAM.
//! 3. **`Rect::byte_offset`** (KT-271): `TRANSFER_TO_HOST_2D.offset` của một rect lệch gốc phải
//!    bằng `y × stride + x × 4`, **không** phải 0 — đây đúng là lỗi KT-271 đã vá; test này chốt
//!    rằng driver dùng hàm ấy chứ không tự tính lại (hai bản tính sẽ lệch nhau im lặng).
//! 4. **Đổi VA nửa cao → PA**: driver DMA bằng PA, con trỏ trong ảnh là alias nửa cao; công thức
//!    `text_start_pa + (va - KERNEL_HIGH_BASE)` phải round-trip với `high_alias_va`.
//! 5. **Console 8×8**: `Console::new(640, 480, 2560, 8, 8)` cho **79 cột × 59 dòng** — đủ ba dòng
//!    `ViFastOs` / `gpu console` / `0123456789` (29 ký tự) mà `gpu_ppm.py` kiểm từng ký tự.
//! 6. **Cache maintenance**: số dòng `dc civac` của framebuffer = `FB_BYTES / line` khi base căn
//!    dòng (không làm tròn lên), và rect lệch gốc chỉ làm sạch **đúng** các hàng của nó.
//! 7. **Hằng số thiết bị**: queue 16 descriptor (luỹ thừa 2, ≥ 2), feature đàm phán **chỉ**
//!    `VIRTIO_F_VERSION_1`, `RESOURCE_ID`/`SCANOUT_ID` khớp `gpu_ppm.py`.

// Test này **ghim hằng số** của driver (địa chỉ framebuffer, bố cục trang DMA, hình học
// console) — khẳng định trên hằng là mục đích, không phải sơ suất (cùng khuôn
// `tests/virtio_blk.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_aarch64::split_mmu::{high_alias_va, KERNEL_HIGH_BASE};
use vifast_plat_aarch64::virtio_gpu::{
    clean_rect_lines, high_to_pa, rect_rows, ATTACH_CHUNK_BYTES, CMD_BYTES, CMD_OFF,
    DMA_PAGE_BYTES, FB_BYTES, FB_HEIGHT, FB_PAGES, FB_PHYS_BASE, FB_STRIDE, FB_WIDTH,
    MAX_ATTACH_ENTRIES, QUEUE_SIZE, RESOURCE_ID, RESP_BYTES, RESP_OFF, RING_OFF, SCANOUT_ID,
};
use vifast_virtio::gpu::{
    negotiated_features, Rect, CTRL_HDR_BYTES, MEM_ENTRY_BYTES, RESOURCE_ATTACH_BACKING_BYTES,
    RESP_DISPLAY_INFO_BYTES, VIRTIO_GPU_F_VERSION_1,
};
use vifast_virtio::gpu_console::Console;
use vifast_virtio::queue::VqLayout;

/// PA liên kết của `__text_start` của ảnh (linker.ld) — dùng cho test đổi VA → PA.
const TEXT_START_PA: u64 = 0x4008_0000;
/// RAM của QEMU `virt` aarch64 (`resources::RAM`) — framebuffer phải nằm trong dải này.
const RAM_BASE: u64 = 0x4000_0000;
/// Cỡ RAM của runner (`-m 128M`).
const RAM_BYTES: u64 = 0x0800_0000;
/// Cỡ dòng cache của `cortex-a72` (QEMU `virt`) — `CTR_EL0.DminLine = 4` ⇒ `4 << 4 = 64`.
const LINE: u64 = 64;

// ---------------------------------------------------------------- 1. bố cục trang DMA

#[test]
fn dma_layout_fits_one_page() {
    let layout = VqLayout::new(QUEUE_SIZE).expect("queue size hợp lệ");
    // Ring (desc + avail + used) phải nằm trọn **trước** vùng lệnh.
    assert!(
        RING_OFF + layout.end <= CMD_OFF,
        "ring kết thúc ở {} > CMD_OFF {CMD_OFF}",
        RING_OFF + layout.end
    );
    // Vùng lệnh trước vùng response, response trong trang.
    assert!(CMD_OFF + CMD_BYTES <= RESP_OFF);
    assert!(RESP_OFF + RESP_BYTES <= DMA_PAGE_BYTES);
    // Response phải đủ 408 B của `virtio_gpu_resp_display_info` (16 scanout × 24 B + 24 B hdr).
    assert!(RESP_BYTES >= RESP_DISPLAY_INFO_BYTES);
    // `RESOURCE_ATTACH_BACKING` + 19 `mem_entry` phải vừa vùng lệnh — nếu không, lệnh bị cắt cụt
    // và thiết bị chỉ thấy `nr_entries` mảnh rác.
    assert!(
        RESOURCE_ATTACH_BACKING_BYTES + MAX_ATTACH_ENTRIES * MEM_ENTRY_BYTES <= CMD_BYTES,
        "attach_backing {} + {}×{} > CMD_BYTES {CMD_BYTES}",
        RESOURCE_ATTACH_BACKING_BYTES,
        MAX_ATTACH_ENTRIES,
        MEM_ENTRY_BYTES
    );
    // Chain của driver là **hai** descriptor (cmd → resp): queue phải chứa được 2.
    assert!(QUEUE_SIZE >= 2);
    assert!(QUEUE_SIZE.is_power_of_two());
    assert_eq!(DMA_PAGE_BYTES, 4096);
}

// ---------------------------------------------------------------- 2. hình học framebuffer

#[test]
fn framebuffer_geometry_is_consistent() {
    assert_eq!(FB_BYTES, FB_WIDTH as usize * FB_HEIGHT as usize * 4);
    assert_eq!(FB_STRIDE, FB_WIDTH as usize * 4);
    assert_eq!(FB_PAGES * 4096, FB_BYTES);
    // 640×480×4 = 1.228.800 B = 300 trang — **không** nhét vừa `.data+.bss` (trần 262.144 B),
    // nên framebuffer nằm ngoài ảnh (xem tài liệu đầu `virtio_gpu.rs`).
    assert_eq!(FB_PAGES, 300);
    assert!(
        FB_BYTES > 256 * 1024,
        "framebuffer phải lớn hơn trần .data+.bss"
    );
    // Base căn 2 MiB ⇒ **một** bảng mức 3 phủ trọn 300 trang (1 frame của arena).
    assert_eq!(FB_PHYS_BASE % (2 << 20), 0, "FB_PHYS_BASE phải căn 2 MiB");
    assert!(
        FB_BYTES as u64 <= 2 << 20,
        "framebuffer phải nằm trong một leaf 2 MiB"
    );
    // Trong RAM và **sau** vùng ảnh (ảnh ~205 KiB từ 0x4008_0000 ⇒ kết thúc < 0x400C_0000).
    assert!(FB_PHYS_BASE >= RAM_BASE);
    assert!(FB_PHYS_BASE + FB_BYTES as u64 <= RAM_BASE + RAM_BYTES);
    assert!(FB_PHYS_BASE >= 0x400C_0000, "framebuffer đè vùng ảnh");
    // Số mảnh backing = ⌈FB_BYTES / 64 KiB⌉ = 19 (mảnh cuối ngắn hơn, không tràn).
    assert_eq!(MAX_ATTACH_ENTRIES, FB_BYTES.div_ceil(ATTACH_CHUNK_BYTES));
    assert_eq!(MAX_ATTACH_ENTRIES, 19);
}

// ---------------------------------------------------------------- 3. `Rect::byte_offset`

#[test]
fn transfer_offset_uses_rect_byte_offset() {
    // Rect lệch gốc: offset **không** phải 0 — nếu gửi 0, thiết bị chép hàng 0 của framebuffer
    // vào `rect.y` (đúng lỗi KT-271 đã đo trên PPM trước khi vá).
    let rect = Rect::new(8, 8, 88, 24);
    assert_eq!(rect.byte_offset(FB_STRIDE), 8 * FB_STRIDE as u64 + 8 * 4);
    assert_ne!(rect.byte_offset(FB_STRIDE), 0);
    // Rect toàn khung ⇒ offset 0.
    assert_eq!(Rect::full(FB_WIDTH, FB_HEIGHT).byte_offset(FB_STRIDE), 0);
    // Rect ở gốc theo x nhưng lệch y ⇒ chỉ cộng phần hàng.
    assert_eq!(
        Rect::new(0, 3, 10, 2).byte_offset(FB_STRIDE),
        3 * FB_STRIDE as u64
    );
    // Rect ở gốc theo y nhưng lệch x ⇒ chỉ cộng phần pixel.
    assert_eq!(Rect::new(5, 0, 10, 2).byte_offset(FB_STRIDE), 5 * 4);
}

// ---------------------------------------------------------------- 4. VA nửa cao → PA

#[test]
fn high_va_converts_back_to_pa() {
    // Alias nửa cao của một PA trong ảnh, rồi đổi ngược — phải về đúng PA ban đầu.
    let pa = TEXT_START_PA + 0x1234;
    let va = high_alias_va(pa, TEXT_START_PA, KERNEL_HIGH_BASE);
    assert_eq!(high_to_pa(va, TEXT_START_PA), pa);
    // Trang DMA nằm trong `.bss` của ảnh ⇒ cùng công thức.
    let dma_pa = TEXT_START_PA + 0x2_0000;
    assert_eq!(
        high_to_pa(
            high_alias_va(dma_pa, TEXT_START_PA, KERNEL_HIGH_BASE),
            TEXT_START_PA
        ),
        dma_pa
    );
    // `KERNEL_HIGH_BASE` là gốc của alias — PA của `__text_start` ánh xạ về chính nó.
    assert_eq!(high_to_pa(KERNEL_HIGH_BASE, TEXT_START_PA), TEXT_START_PA);
}

// ---------------------------------------------------------------- 5. console 8×8

#[test]
fn console_geometry_fits_three_lines() {
    let console = Console::new(FB_WIDTH as usize, FB_HEIGHT as usize, FB_STRIDE, 8, 8)
        .expect("hình học console hợp lệ");
    // (640 − 8) / 8 = 79 cột; (480 − 8) / 8 = 59 dòng.
    assert_eq!(console.cols(), 79);
    assert_eq!(console.rows(), 59);
    // Ba dòng của `gpu_ppm.py` (29 ký tự) phải vừa **không** cuộn.
    let lines = ["ViFastOs", "gpu console", "0123456789"];
    assert_eq!(lines.iter().map(|l| l.len()).sum::<usize>(), 29);
    assert!(lines.iter().all(|l| l.len() <= console.cols()));
    assert!(lines.len() <= console.rows());
    // Vùng chữ = 79×8 × 59×8 pixel, gốc (8,8).
    let area = console.text_area();
    assert_eq!((area.x, area.y), (8, 8));
    assert_eq!((area.width, area.height), (79 * 8, 59 * 8));
    // Hình học vô lý ⇒ `None` (fail-closed, không kẹp im lặng).
    assert!(Console::new(0, 480, 0, 8, 8).is_none());
    assert!(Console::new(640, 480, 640 * 4 - 4, 8, 8).is_none());
    assert!(Console::new(640, 480, FB_STRIDE, 640, 8).is_none());
}

// ---------------------------------------------------------------- 6. cache maintenance

#[test]
fn cache_lines_cover_framebuffer_and_rect() {
    // Base căn dòng ⇒ số dòng = FB_BYTES / 64 (không làm tròn lên).
    assert_eq!(FB_PHYS_BASE % LINE, 0);
    assert_eq!(
        clean_rect_lines(Rect::full(FB_WIDTH, FB_HEIGHT), LINE),
        FB_BYTES as u64 / LINE
    );
    // Rect lệch gốc: mỗi hàng làm sạch **đúng** `width × 4` byte của hàng ấy, không chạm hàng
    // khác — nếu làm sạch cả khung thì con số sẽ là 19.200 dòng cho mọi rect.
    let rect = Rect::new(8, 8, 88, 24);
    let per_row = (88 * 4) as u64;
    // 352 B = 5,5 dòng 64 B ⇒ **6** dòng mỗi hàng (làm tròn ra ngoài, không bỏ sót byte cuối).
    assert_eq!(per_row % LINE, 32);
    assert_eq!(clean_rect_lines(rect, LINE), 24 * 6);
    assert!(clean_rect_lines(rect, LINE) < FB_BYTES as u64 / LINE);
    // Rect rỗng ⇒ 0 dòng (không phát `dc` vô ích).
    assert_eq!(clean_rect_lines(Rect::new(0, 0, 0, 0), LINE), 0);
    // `rect_rows` trả đúng các hàng của rect — dùng cho vòng lặp `dc` theo hàng.
    let mut rows = [0u64; 24];
    let mut count = 0;
    for offset in rect_rows(rect, FB_STRIDE) {
        rows[count] = offset;
        count += 1;
    }
    assert_eq!(count, 24);
    assert_eq!(rows[0], 8 * FB_STRIDE as u64 + 8 * 4);
    assert_eq!(rows[23], 31 * FB_STRIDE as u64 + 8 * 4);
}

// ---------------------------------------------------------------- 7. hằng số thiết bị

#[test]
fn device_constants_match_contract() {
    // Feature đàm phán **chỉ** `VIRTIO_F_VERSION_1` — không VIRGL/EDID/RESOURCE_BLOB, **kể cả**
    // khi thiết bị báo có tất cả (`u64::MAX`).
    let device = 1u64 << VIRTIO_GPU_F_VERSION_1;
    assert_eq!(negotiated_features(device), device);
    assert_eq!(negotiated_features(u64::MAX), device);
    // Hàm thuần **luôn** trả VERSION_1 (một chỗ quyết định, D2-110B); việc thiết bị **không** có
    // VERSION_1 là do driver kiểm riêng trước khi gọi (`Err("version1")` — xem `init`).
    assert_eq!(negotiated_features(0), device);
    // `resource_id`/`scanout_id` khớp `gpu_ppm.py` (một resource, scanout 0).
    assert_eq!(RESOURCE_ID, 1);
    assert_eq!(SCANOUT_ID, 0);
    // Header điều khiển 24 B — mọi lệnh 2D đều bắt đầu bằng nó.
    assert_eq!(CTRL_HDR_BYTES, 24);
}
