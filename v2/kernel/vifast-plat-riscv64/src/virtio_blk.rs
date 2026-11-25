// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-blk` trên riscv64 — KT-334 (trước: driver đầy đủ KT-72).
//!
//! Toàn bộ quyết định giao thức (status/feature/queue, descriptor, notify, poll, `virtio_blk_config`)
//! nằm ở [`vifast_virtio::blkdev`] — crate **thuần**, test host đầy đủ. Tệp này chỉ cài ba thứ
//! thuộc về phần cứng:
//!
//! 1. [`Mmio`] — đọc/ghi BAR virtio bằng con trỏ thô + `fence rw, rw` (D2-17: `unsafe` ở
//!    `kernel/vifast-plat-*`, không ở crate logic);
//! 2. [`Dma`] — một trang 4 KiB trong `.bss` (identity map ⇒ `VA == PA`), không cần cache hook;
//! 3. [`Tick`] — trần poll theo timebase của `Riscv64Virt`.
//!
//! Hành vi/TAP của `ktest-vblk` giữ **nguyên**: cùng chữ ký hàm, cùng mã lỗi, cùng bố cục trang
//! DMA — ba thứ ấy do `vifast_virtio::blkdev` quyết định và test host chốt.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};

use vifast_plat_common::Platform;
use vifast_plat_riscv64_logic::timer::TIMEBASE_HZ;
use vifast_virtio::blk::BlkError;
use vifast_virtio::blkdev::{self, Clock, DmaBuf, DmaPool, MmioBus};

use crate::pci::PciReport;
use crate::Riscv64Virt;

pub use vifast_virtio::blkdev::{
    ascii16, hex16, BlkReport, Sector0, Stats, DATA_OFF, DMA_PAGE_BYTES, QUEUE_SIZE, REQ_OFF,
    SECTOR0_MAGIC, SECTOR_BYTES, STATUS_OFF,
};

/// Trần chờ `used` — 2 s ảo (dưới `-icount` vẫn tiến theo lệnh đã chạy).
pub const POLL_TIMEOUT_TICKS: u64 = 2 * TIMEBASE_HZ;

// Bất biến biên dịch: một trang DMA phải chứa trọn ring 8 descriptor của virtqueue — đây là lý
// do `QUEUE_SIZE` và bố cục trang trong `vifast_virtio::blkdev` khớp nhau (layout lấy từ crate).
const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2 trong [2, 256]"),
    };
    assert!(DMA_PAGE_BYTES >= layout.total_bytes());
};

// ---------------------------------------------------------------- trang DMA (identity map)

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: ảnh chạy một hart trên đường driver và driver tự tuần tự hoá (một request một lúc);
// vùng này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
unsafe impl Sync for DmaCell {}
static DMA_PAGE: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ trang DMA — identity map nên `VA == PA` (TAP kiểm bằng `walk_sw`).
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
        unsafe { asm!("fence rw, rw", options(nostack, preserves_flags)) };
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
        // Pool **một vùng**: cấp chính trang tĩnh; từ chối nếu yêu cầu không khớp căn lề/cỡ.
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
        // Trang sống cùng ảnh ⇒ không có gì để trả cho allocator; `true` = đã "thu" xong.
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
        Some(self.now().wrapping_add(POLL_TIMEOUT_TICKS))
    }
    fn spin_limit(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static BLK: vifast_virtio::blkdev::Blk = vifast_virtio::blkdev::Blk::new();

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** để TAP in ra lý do đo được.
pub fn init(pci: &PciReport) -> Result<BlkReport, &'static str> {
    let transport = blkdev::Transport::from_report(pci);
    BLK.init(&mut Mmio, &transport, &mut Dma)
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    BLK.stats()
}

/// Dung lượng thiết bị theo sector 512 B (0 nếu chưa init).
pub fn capacity_sectors() -> u32 {
    BLK.capacity_sectors() as u32
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    BLK.ready()
}

/// Gửi một request đọc, kiểm dải trước.
pub fn read_sector(lba: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    BLK.read_sector(&mut Mmio, &mut Dma, &mut Tick, lba, out)
}

/// Gửi một request ghi, kiểm dải trước.
pub fn write_sector(lba: u32, data: &[u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    BLK.write_sector(&mut Mmio, &mut Dma, &mut Tick, lba, data)
}

/// `VIRTIO_BLK_T_FLUSH` thật (chỉ khi đã đàm phán feature).
pub fn flush() -> Result<(), BlkError> {
    BLK.flush(&mut Mmio, &mut Dma, &mut Tick)
}

/// **Chỉ cho ca âm của TAP**: đọc vượt kiểm dải để chứng minh thiết bị trả `IOERR` thật.
pub fn read_sector_force(lba: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    BLK.read_sector_force(&mut Mmio, &mut Dma, &mut Tick, lba, out)
}

/// Địa chỉ ISR (chẩn đoán; K6 chưa dùng ngắt).
pub fn isr_addr() -> u64 {
    BLK.isr_addr()
}

/// Đọc `device_status` hiện tại.
pub fn device_status() -> u8 {
    BLK.device_status(&mut Mmio)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    BLK.config_generation(&mut Mmio)
}

/// `true` ⇔ 8 byte đầu khớp chữ ký của runner (hàm thuần, dùng chung 3 arch).
pub fn has_magic(bytes: &[u8]) -> bool {
    blkdev::has_magic(bytes, &SECTOR0_MAGIC)
}

/// Bằng chứng sector 0 (hex/ascii/magic) — cùng chữ ký cũ, chữ ký lấy từ [`SECTOR0_MAGIC`].
pub fn sector0_evidence(bytes: &[u8; 16]) -> Sector0 {
    blkdev::sector0_evidence(bytes, &SECTOR0_MAGIC)
}
