// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-gpu` 2D trên x86_64 — KT-337 (mở 2D lần đầu trên x86_64).
//!
//! Quyết định giao thức (status/feature/queue, descriptor, notify, poll, sáu lệnh 2D, offset
//! `TRANSFER_TO_HOST_2D`) nằm ở [`vifast_virtio::gpudev`] — crate **thuần**, test host đầy đủ.
//! Tệp này giữ đúng phần x86_64:
//!
//! 1. **DMA bằng VA = PA**: ảnh chạy identity map 1 GiB nên địa chỉ trang `.bss` **là** PA;
//! 2. **MMIO qua [`crate::asm`]** (mô-đun duy nhất có `unsafe`); tệp này chỉ thêm `unsafe` cho
//!    **trang DMA** (`UnsafeCell` + `Sync` thủ công) và **framebuffer** — cùng khuôn
//!    `virtio_blk.rs` (D2-17);
//! 3. **Đồng hồ là tham số**: pha này chạy sau `cli` (không dùng tick PIT→LAPIC), nên mọi hàm
//!    nhận `now: &mut dyn FnMut() -> u64` (µs, PIT kênh 2) — đúng khuôn `virtio_blk::read_sector`.
//!
//! # BAR do **firmware** gán
//!
//! x86_64 (`-M q35`) để SeaBIOS enumerate: driver **không** ghi đè BAR (khác riscv64/aarch64), chỉ
//! kiểm BAR nằm trong cửa sổ MMIO đã map (`boot::mmio_mapped`) rồi dùng. `qemu_guard.sh` của
//! runner cấp `-device virtio-gpu-pci`; firmware đặt BAR 16 KiB trong dải `0xFE……`.

#![allow(unsafe_code)] // D2-17: `kernel/vifast-plat-*`; chỉ trang DMA + framebuffer cần `unsafe`.

use core::cell::UnsafeCell;
use core::sync::atomic::{fence, Ordering};

use vifast_virtio::blkdev::Clock;
use vifast_virtio::gpu::Rect;
use vifast_virtio::gpudev::{DmaBuf, DmaPool, Gpu, MmioBus};

use crate::asm;
use crate::pci::PciReport;

pub use vifast_virtio::gpudev::{
    GpuReport, Stats, ATTACH_CHUNK_BYTES, CMD_BYTES, CMD_OFF, DMA_PAGE_BYTES, FB_BYTES, FB_HEIGHT,
    FB_PAGES, FB_STRIDE, FB_WIDTH, MAX_ATTACH_ENTRIES, QUEUE_SIZE, RESOURCE_ID, RESP_BYTES,
    RESP_OFF, RING_OFF, SCANOUT_ID,
};

/// Trần poll **một** lệnh 2D (µs) — 2 s, cùng ngưỡng `virtio_blk::POLL_TIMEOUT_US` (không nới).
pub const POLL_TIMEOUT_US: u64 = 2_000_000;

/// Base **vật lý** của framebuffer: 32 MiB — trong identity map 1 GiB, **trên** ảnh + arena
/// stack + bảng trang (ảnh kết thúc dưới 8 MiB), **dưới** trần RAM `-m 128M`.
///
/// 640×480×4 = 1.228.800 B = 300 trang; 32 MiB căn 2 MiB nên một leaf 2 MiB phủ **một phần**
/// dải (dải trải 1.228.800 B < 2 MiB nên vẫn gọn trong một leaf).
pub const FB_PHYS_BASE: u64 = 0x0200_0000;

const _: () = assert!(FB_PHYS_BASE % 4096 == 0);
const _: () = assert!(FB_PHYS_BASE >= 0x0100_0000); // trên ảnh + SMP stack arena
const _: () = assert!(FB_PHYS_BASE + FB_BYTES as u64 <= 0x0800_0000); // RAM `-m 128M`
const _: () = assert!(FB_PHYS_BASE % (2 << 20) == 0);
const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2"),
    };
    assert!(RING_OFF + layout.end <= CMD_OFF);
    assert!(CMD_OFF + CMD_BYTES <= RESP_OFF);
    assert!(RESP_OFF + RESP_BYTES <= DMA_PAGE_BYTES);
};

// ---------------------------------------------------------------- trang DMA (identity map)

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: ảnh chạy một CPU trên đường driver và driver tự tuần tự hoá (một lệnh một lúc); vùng
// này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
unsafe impl Sync for DmaCell {}
static DMA_PAGE: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ trang DMA — identity map nên `VA == PA`.
pub fn dma_page_addr() -> u64 {
    DMA_PAGE.0.get() as *const _ as u64
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một CPU + tuần tự (xem `DmaCell`).
    unsafe { &mut (*DMA_PAGE.0.get()).0 }
}

// ---------------------------------------------------------------- adapter MMIO

struct Mmio;

impl MmioBus for Mmio {
    fn read8(&self, addr: u64) -> u8 {
        asm::read_u8(addr)
    }
    fn read16(&self, addr: u64) -> u16 {
        asm::read_u16(addr)
    }
    fn read32(&self, addr: u64) -> u32 {
        asm::read_u32(addr)
    }
    fn write8(&mut self, addr: u64, value: u8) {
        asm::write_u8(addr, value);
    }
    fn write16(&mut self, addr: u64, value: u16) {
        asm::write_u16(addr, value);
    }
    fn write32(&mut self, addr: u64, value: u32) {
        asm::write_u32(addr, value);
    }
    fn write64(&mut self, addr: u64, value: u64) {
        asm::write_u64(addr, value);
    }
    fn fence(&self) {
        // `mfence` (qua `core::sync::atomic::fence(SeqCst)`) trước khi bàn giao ring.
        fence(Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- adapter DMA

/// Pool DMA một vùng: trang `.bss` ở trên; x86_64 không có cache maintenance cho DMA.
struct Dma;

impl DmaPool for Dma {
    fn base_pa(&self) -> u64 {
        dma_page_addr()
    }
    fn capacity(&self) -> usize {
        DMA_PAGE_BYTES
    }
    fn bytes(&mut self) -> &mut [u8] {
        page()
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        if len == 0 || len > DMA_PAGE_BYTES || align == 0 || !align.is_power_of_two() {
            return None;
        }
        if align > DMA_PAGE_BYTES || DMA_PAGE_BYTES % align != 0 {
            return None;
        }
        Some(DmaBuf {
            off: 0,
            len,
            pa: dma_page_addr(),
        })
    }
    fn free(&mut self, _buf: DmaBuf) -> bool {
        true
    }
    fn read_u16(&self, off: usize) -> u16 {
        asm::read_u16(dma_page_addr() + off as u64)
    }
    fn read_u8(&self, off: usize) -> u8 {
        asm::read_u8(dma_page_addr() + off as u64)
    }
    fn sync_for_device(&mut self, _off: usize, _len: usize) {
        // x86_64: DMA nhất quán với cache (không có cache maintenance cho MMIO/DMA).
    }
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {
        // Như `sync_for_device`: no-op có chủ đích, không phải thiếu sót.
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

/// Bọc closure `now()` (µs) của ảnh thành [`Clock`] — cùng khuôn `virtio_blk` trên x86_64.
struct ClockFn<'a> {
    now: &'a mut dyn FnMut() -> u64,
}

impl Clock for ClockFn<'_> {
    fn now(&mut self) -> u64 {
        (self.now)()
    }
    fn deadline(&mut self) -> Option<u64> {
        Some((self.now)().wrapping_add(POLL_TIMEOUT_US))
    }
    fn spin_limit(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------- trạng thái + API

static GPU: Gpu = Gpu::new();

/// Khởi tạo driver từ kết quả PCI. `now` là đồng hồ µs (PIT kênh 2).
pub fn init(pci: &PciReport, now: &mut dyn FnMut() -> u64) -> Result<GpuReport, &'static str> {
    let mut clock = ClockFn { now };
    GPU.init(&mut Mmio, &mut Dma, &mut clock, pci, FB_PHYS_BASE)
}

/// Ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    GPU.stats()
}

/// `GET_DISPLAY_INFO` — mode **thiết bị báo**.
pub fn get_display_info(
    now: &mut dyn FnMut() -> u64,
) -> Result<vifast_virtio::gpu::RespDisplayInfo, &'static str> {
    let mut clock = ClockFn { now };
    GPU.get_display_info(&mut Mmio, &mut Dma, &mut clock)
}

/// `RESOURCE_CREATE_2D` — tạo resource 2D.
pub fn create_resource(
    width: u32,
    height: u32,
    now: &mut dyn FnMut() -> u64,
) -> Result<(), &'static str> {
    let mut clock = ClockFn { now };
    GPU.create_resource(&mut Mmio, &mut Dma, &mut clock, width, height)
}

/// `RESOURCE_ATTACH_BACKING` — gắn 19 mảnh của dải framebuffer.
pub fn attach_backing(now: &mut dyn FnMut() -> u64) -> Result<u32, &'static str> {
    let mut clock = ClockFn { now };
    GPU.attach_backing(&mut Mmio, &mut Dma, &mut clock)
}

/// `SET_SCANOUT` — gắn resource vào scanout.
pub fn set_scanout(rect: Rect, now: &mut dyn FnMut() -> u64) -> Result<(), &'static str> {
    let mut clock = ClockFn { now };
    GPU.set_scanout(&mut Mmio, &mut Dma, &mut clock, rect)
}

/// Ca âm đối chứng: `SET_SCANOUT` với resource không tồn tại.
pub fn probe_bogus_scanout(
    bogus_id: u32,
    rect: Rect,
    now: &mut dyn FnMut() -> u64,
) -> Result<&'static str, &'static str> {
    let mut clock = ClockFn { now };
    GPU.probe_bogus_scanout(&mut Mmio, &mut Dma, &mut clock, bogus_id, rect)
}

/// `TRANSFER_TO_HOST_2D` — offset do crate thuần tính bằng `Rect::byte_offset`.
pub fn transfer_to_host_2d(rect: Rect, now: &mut dyn FnMut() -> u64) -> Result<(), &'static str> {
    let mut clock = ClockFn { now };
    GPU.transfer_to_host_2d(&mut Mmio, &mut Dma, &mut clock, rect)
}

/// `RESOURCE_FLUSH` — đẩy vùng ra màn hình.
pub fn resource_flush(rect: Rect, now: &mut dyn FnMut() -> u64) -> Result<(), &'static str> {
    let mut clock = ClockFn { now };
    GPU.resource_flush(&mut Mmio, &mut Dma, &mut clock, rect)
}

/// Flush một vùng đã đổi: `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH` (đúng hai lệnh).
pub fn flush_rect(rect: Rect, now: &mut dyn FnMut() -> u64) -> Result<(), &'static str> {
    transfer_to_host_2d(rect, now)?;
    resource_flush(rect, now)
}

/// Framebuffer driver vẽ — dải PA [`FB_PHYS_BASE`] map **identity** trong 1 GiB đầu, nên
/// `VA == PA` và thiết bị DMA đọc đúng dải PA đó.
pub fn framebuffer() -> &'static mut [u8] {
    // SAFETY: `FB_PHYS_BASE` nằm trong identity map RW của ảnh (leaf 2 MiB `P|RW|PS`), nằm ngoài
    // ảnh/arena/bảng trang, trong RAM `-m 128M`; một CPU, tuần tự.
    unsafe { core::slice::from_raw_parts_mut(FB_PHYS_BASE as *mut u8, FB_BYTES) }
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    GPU.ready()
}

/// Địa chỉ ISR (chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    GPU.isr_addr()
}

/// PA trang DMA đã cấp (identity map ⇒ `VA == PA`).
pub fn dma_pa() -> u64 {
    GPU.dma_pa()
}

/// Đọc thanh ghi `device_status` hiện tại.
pub fn device_status() -> u8 {
    GPU.device_status(&mut Mmio)
}

/// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng.
pub fn published() -> u16 {
    GPU.published()
}
