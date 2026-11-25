// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-gpu` 2D trên riscv64 — KT-337 (trước: driver đầy đủ KT-263).
//!
//! Toàn bộ quyết định giao thức (status/feature/queue, descriptor, notify, poll, sáu lệnh 2D,
//! offset `TRANSFER_TO_HOST_2D`) nằm ở [`vifast_virtio::gpudev`] — crate **thuần**, test host đầy
//! đủ. Tệp này chỉ cài ba thứ thuộc về phần cứng:
//!
//! 1. [`Mmio`] — đọc/ghi BAR virtio bằng con trỏ thô + `fence rw, rw` (D2-17: `unsafe` ở
//!    `kernel/vifast-plat-*`, không ở crate logic);
//! 2. [`Dma`] — một trang 4 KiB trong `.bss` (identity map ⇒ `VA == PA`), không cần cache hook;
//! 3. [`Tick`] — trần poll theo timebase của `Riscv64Virt`.
//!
//! Framebuffer vẫn là **300 frame** liền nhau truy cập qua **physmap** (xem [`framebuffer`]): đó
//! là hình học của ảnh riscv64, không phải quyết định giao thức.

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};

use vifast_plat_common::Platform;
use vifast_plat_riscv64_logic::timer::TIMEBASE_HZ;
use vifast_virtio::blkdev::Clock;
use vifast_virtio::gpu::Rect;
use vifast_virtio::gpudev::{self, DmaBuf, DmaPool, Gpu, MmioBus};

use crate::mmu::{RAM_BYTES, RAM_PHYS_BASE};
use crate::pci::PciReport;
use crate::Riscv64Virt;

pub use vifast_virtio::gpudev::{
    GpuReport, Stats, ATTACH_CHUNK_BYTES, DMA_PAGE_BYTES, FB_BYTES, FB_HEIGHT, FB_PAGES, FB_STRIDE,
    FB_WIDTH, MAX_ATTACH_ENTRIES, RESOURCE_ID, SCANOUT_ID,
};

/// Thời gian chờ **tối đa** cho một lệnh (tick timebase) — 2 s ảo.
///
/// Lệnh 2D của QEMU xong trong micro-giây; 2 s là dư rất nhiều. Hết hạn ⇒ `Err("timeout")` +
/// TAP đỏ (fail-closed, **không** treo).
pub const GPU_WAIT_TICKS: u64 = 2 * TIMEBASE_HZ;
/// Base **vật lý** của framebuffer: 2 MiB đầu **sau** vùng probe của ảnh `ktest`
/// (`mmu::PROBE2_PA_D + 2 MiB`), trong RAM `-m 128M`, căn 2 MiB.
pub const FB_PHYS_BASE: u64 = 0x8100_0000;

const _: () = assert!(FB_PHYS_BASE % 4096 == 0);
const _: () = assert!(FB_PHYS_BASE >= 0x8100_0000); // sau PROBE2_PA_D + 2 MiB
const _: () = assert!(FB_PHYS_BASE + FB_BYTES as u64 <= RAM_PHYS_BASE + RAM_BYTES);

// ---------------------------------------------------------------- trang DMA (identity map)

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: ảnh chạy một hart trên đường driver và driver tự tuần tự hoá (một lệnh một lúc); vùng
// này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
unsafe impl Sync for DmaCell {}
static DMA_PAGE: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA` (TAP kiểm bằng `walk_sw`).
pub fn dma_page_addr() -> u64 {
    DMA_PAGE.0.get() as *const _ as u64
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một hart + tuần tự (xem `DmaCell`).
    unsafe { &mut (*DMA_PAGE.0.get()).0 }
}

// ---------------------------------------------------------------- adapter MMIO

struct Mmio;

impl MmioBus for Mmio {
    #[inline]
    fn read8(&self, addr: u64) -> u8 {
        // SAFETY: `addr` nằm trong BAR đã gán/map Device RW (kiểm bằng walk trong TAP).
        unsafe { read_volatile(addr as *const u8) }
    }
    #[inline]
    fn read16(&self, addr: u64) -> u16 {
        // SAFETY: như `read8`; căn 2.
        unsafe { read_volatile(addr as *const u16) }
    }
    #[inline]
    fn read32(&self, addr: u64) -> u32 {
        // SAFETY: như `read8`; căn 4.
        unsafe { read_volatile(addr as *const u32) }
    }
    #[inline]
    fn write8(&mut self, addr: u64, value: u8) {
        // SAFETY: như `read8`; ghi thanh ghi thiết bị đã map.
        unsafe { write_volatile(addr as *mut u8, value) };
    }
    #[inline]
    fn write16(&mut self, addr: u64, value: u16) {
        // SAFETY: như `write8`; căn 2.
        unsafe { write_volatile(addr as *mut u16, value) };
    }
    #[inline]
    fn write32(&mut self, addr: u64, value: u32) {
        // SAFETY: như `write8`; căn 4.
        unsafe { write_volatile(addr as *mut u32, value) };
    }
    #[inline]
    fn write64(&mut self, addr: u64, value: u64) {
        // SAFETY: như `write8`; căn 8.
        unsafe { write_volatile(addr as *mut u64, value) };
    }
    #[inline]
    fn fence(&self) {
        // SAFETY: `fence` chỉ là hàng rào thứ tự, không truy cập địa chỉ.
        unsafe { core::arch::asm!("fence rw, rw", options(nostack, preserves_flags)) };
    }
}

// ---------------------------------------------------------------- adapter DMA

/// Pool DMA một vùng: đúng trang `.bss` ở trên (không cấp phát động).
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
        // SAFETY: `off` nằm trong trang DMA; đọc volatile để compiler không hoist khỏi vòng poll.
        unsafe { read_volatile((dma_page_addr() + off as u64) as *const u16) }
    }
    fn read_u8(&self, off: usize) -> u8 {
        // SAFETY: như `read_u16`.
        unsafe { read_volatile((dma_page_addr() + off as u64) as *const u8) }
    }
    fn sync_for_device(&mut self, _off: usize, _len: usize) {
        // riscv64: không có cache không nhất quán với DMA (identity map, không maintenance).
    }
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {
        // Như `sync_for_device`: no-op có chủ đích, không phải thiếu sót.
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

struct Tick;

impl Clock for Tick {
    fn now(&mut self) -> u64 {
        Riscv64Virt.now_ticks()
    }
    fn deadline(&mut self) -> Option<u64> {
        Some(self.now().wrapping_add(GPU_WAIT_TICKS))
    }
    fn spin_limit(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static GPU: Gpu = Gpu::new();

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** để TAP in ra lý do đo được.
pub fn init(pci: &PciReport) -> Result<GpuReport, &'static str> {
    GPU.init(&mut Mmio, &mut Dma, &mut Tick, pci, FB_PHYS_BASE)
}

/// Ảnh chụp bộ đếm (superset dùng chung; riscv64 không có bộ đếm cache framebuffer).
pub fn stats() -> Stats {
    GPU.stats()
}

/// `GET_DISPLAY_INFO` — mode **thiết bị báo**.
pub fn get_display_info() -> Result<vifast_virtio::gpu::RespDisplayInfo, &'static str> {
    GPU.get_display_info(&mut Mmio, &mut Dma, &mut Tick)
}

/// `RESOURCE_CREATE_2D` — tạo resource 2D.
pub fn create_resource(width: u32, height: u32) -> Result<(), &'static str> {
    GPU.create_resource(&mut Mmio, &mut Dma, &mut Tick, width, height)
}

/// `RESOURCE_ATTACH_BACKING` — gắn 19 mảnh của dải framebuffer.
pub fn attach_backing() -> Result<u32, &'static str> {
    GPU.attach_backing(&mut Mmio, &mut Dma, &mut Tick)
}

/// `SET_SCANOUT` — gắn resource vào scanout.
pub fn set_scanout(rect: Rect) -> Result<(), &'static str> {
    GPU.set_scanout(&mut Mmio, &mut Dma, &mut Tick, rect)
}

/// Ca âm đối chứng: `SET_SCANOUT` với resource không tồn tại.
pub fn probe_bogus_scanout(bogus_id: u32, rect: Rect) -> Result<&'static str, &'static str> {
    GPU.probe_bogus_scanout(&mut Mmio, &mut Dma, &mut Tick, bogus_id, rect)
}

/// `TRANSFER_TO_HOST_2D` — offset = `Rect::byte_offset` (một chỗ duy nhất ở crate thuần).
pub fn transfer_to_host_2d(rect: Rect) -> Result<(), &'static str> {
    GPU.transfer_to_host_2d(&mut Mmio, &mut Dma, &mut Tick, rect)
}

/// `RESOURCE_FLUSH` — đẩy vùng ra màn hình.
pub fn resource_flush(rect: Rect) -> Result<(), &'static str> {
    GPU.resource_flush(&mut Mmio, &mut Dma, &mut Tick, rect)
}

/// Flush một vùng đã đổi: `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH` (đúng hai lệnh).
pub fn flush_rect(rect: Rect) -> Result<(), &'static str> {
    GPU.flush_rect(&mut Mmio, &mut Dma, &mut Tick, rect)
}

/// Framebuffer driver vẽ — **alias physmap** của dải PA [`FB_PHYS_BASE`] (leaf 2 MiB RW NX đã map
/// sẵn cho toàn bộ RAM), nên `VA = PHYSMAP_BASE + PA` và thiết bị DMA đọc đúng dải PA đó.
pub fn framebuffer() -> &'static mut [u8] {
    let va = crate::mmu::PHYSMAP_BASE + FB_PHYS_BASE;
    // SAFETY: `va` là alias physmap của RAM `[FB_PHYS_BASE, FB_PHYS_BASE + FB_BYTES)` — đã map RW
    // NX bởi `mmu::init` (leaf 2 MiB), nằm ngoài image/arena/vùng probe; một hart, tuần tự.
    unsafe { core::slice::from_raw_parts_mut(va as *mut u8, FB_BYTES) }
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    GPU.ready()
}

/// Địa chỉ ISR (chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    GPU.isr_addr()
}

/// Đọc thanh ghi `device_status` hiện tại.
pub fn device_status() -> u8 {
    GPU.device_status(&mut Mmio)
}

/// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng.
pub fn published() -> u16 {
    GPU.published()
}

// Bất biến biên dịch: ring + vùng lệnh + vùng response nằm gọn trong **một** trang DMA — kiểm
// bằng chính layout của crate thuần để hai bên không thể lệch nhau.
const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(gpudev::QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2"),
    };
    assert!(gpudev::RING_OFF + layout.end <= gpudev::CMD_OFF);
    assert!(gpudev::CMD_OFF + gpudev::CMD_BYTES <= gpudev::RESP_OFF);
    assert!(gpudev::RESP_OFF + gpudev::RESP_BYTES <= DMA_PAGE_BYTES);
};
