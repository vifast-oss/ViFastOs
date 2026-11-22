// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-input` trên riscv64 — KT-337 (trước: driver đầy đủ KT-255).
//!
//! Quyết định giao thức (status/feature/queue, descriptor, arm đệm, notify, poll `used`, đọc
//! `virtio_input_config`) nằm ở [`vifast_virtio::inputdev`] — crate **thuần**, test host đầy đủ.
//! Tệp này chỉ cài ba thứ thuộc về phần cứng:
//!
//! 1. [`Mmio`] — đọc/ghi BAR virtio bằng con trỏ thô + `fence rw, rw` (D2-17);
//! 2. [`Dma`] — một trang 4 KiB trong `.bss` (identity map ⇒ `VA == PA`), không cache hook;
//! 3. [`Tick`] — nguồn tick của `Riscv64Virt` cho `poll_event_wait`.
//!
//! riscv64 giữ **một** queue (`event`) như KT-255 — `with_status_queue = false` — để topology
//! `num_queues`/`queue_size` của TAP không đổi.

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};

use vifast_plat_common::Platform;
use vifast_virtio::blkdev::Clock;
use vifast_virtio::dma::{DmaBuf, DmaPool};
use vifast_virtio::input::{InputEvent, CFG_U_BYTES};
use vifast_virtio::inputdev::{Input, InputReport, Stats};
use vifast_virtio::mmio::MmioBus;

use crate::pci::PciReport;
use crate::Riscv64Virt;

pub use vifast_virtio::inputdev::{DMA_PAGE_BYTES, EV_BITS_BYTES, NAME_MAX_BYTES};

/// Trần chờ mặc định khi không truyền `timeout_ticks` (tick timebase) — 2 s ảo.
pub const WAIT_TICKS: u64 = 2 * vifast_plat_common::timer::TIMEBASE_HZ;

// ---------------------------------------------------------------- trang DMA (identity map)

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: ảnh chạy một hart trên đường driver và driver tự tuần tự hoá (một sự kiện một lúc);
// vùng này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
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
        Some(self.now().wrapping_add(WAIT_TICKS))
    }
    fn spin_limit(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static INPUT: Input = Input::new();

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** để TAP in ra lý do đo được.
pub fn init(pci: &PciReport) -> Result<InputReport, &'static str> {
    INPUT.init(&mut Mmio, &mut Dma, pci, false)
}

/// Ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    INPUT.stats()
}

/// Nhận **một** sự kiện nếu thiết bị đã hoàn tất (không chờ).
pub fn poll_event() -> Option<InputEvent> {
    INPUT.poll_event(&mut Mmio, &mut Dma, &mut Tick)
}

/// Nhận **một** sự kiện, chờ tối đa `timeout_ticks` tick timebase.
pub fn poll_event_wait(timeout_ticks: u64) -> Option<InputEvent> {
    INPUT.poll_event_wait(&mut Mmio, &mut Dma, &mut Tick, timeout_ticks)
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    INPUT.ready()
}

/// Địa chỉ ISR (KT-255 chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    INPUT.isr_addr()
}

/// Đọc thanh ghi `device_status` hiện tại.
pub fn device_status() -> u8 {
    INPUT.device_status(&mut Mmio)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    INPUT.config_generation(&mut Mmio)
}

/// Số descriptor đã publish (một chiều).
pub fn published() -> u16 {
    INPUT.published()
}

// Bất biến: hai vùng ring + 8 đệm phải nằm trong một trang DMA — dùng chính hằng của crate thuần.
const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(vifast_virtio::input::QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2"),
    };
    assert!(vifast_virtio::inputdev::RING_OFF + layout.end <= DMA_PAGE_BYTES);
    assert!(
        vifast_virtio::inputdev::BUF_OFF
            + vifast_virtio::input::EVENT_BUFS * vifast_virtio::input::EVENT_BYTES
            <= DMA_PAGE_BYTES
    );
};

const _: () = assert!(CFG_U_BYTES == NAME_MAX_BYTES);
