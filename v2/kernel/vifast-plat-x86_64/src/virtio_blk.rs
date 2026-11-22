// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-blk` trên x86_64 — KT-334 (trước: driver đầy đủ KT-288).
//!
//! Quyết định giao thức (status/feature/queue, descriptor, notify, poll, config) nằm ở
//! [`vifast_virtio::blkdev`] — crate **thuần**, test host đầy đủ. Tệp này giữ đúng phần x86_64:
//!
//! 1. **DMA bằng VA = PA**: ảnh chạy identity map 1 GiB nên địa chỉ trang `.bss` **là** PA;
//! 2. **MMIO qua [`crate::asm`]** (mô-đun duy nhất có `unsafe`); tệp này chỉ thêm `unsafe` cho
//!    **trang DMA** (`UnsafeCell` + `Sync` thủ công) — cùng khuôn bản trước KT-334 (D2-17);
//! 3. **Đồng hồ là tham số**: bước này chạy sau `cli` nên không dùng tick PIT→LAPIC; mọi hàm chờ
//!    nhận `now: &mut dyn FnMut() -> u64` (µs) và adapter bọc nó thành [`Clock`].
//!
//! Giữ nguyên API/TAP: `init(&PciReport)`, `read_sector(..., now)`, `bar_disjoint`, các hằng
//! `POLL_TIMEOUT_US`/`SECTOR0_MAGIC`, và ba hàm bằng chứng sector 0.

#![allow(unsafe_code)] // D2-17: `kernel/vifast-plat-*`; chỉ trang DMA cần `unsafe`.

use core::cell::UnsafeCell;
use core::sync::atomic::{fence, Ordering};

use vifast_virtio::blk::BlkError;
use vifast_virtio::blkdev::{self, Clock, DmaBuf, DmaPool, MmioBus};

use crate::asm;
use crate::pci::PciReport;

pub use vifast_virtio::blkdev::{
    ascii16, hex16, BlkReport, Sector0, Stats, DATA_OFF, DMA_PAGE_BYTES, QUEUE_SIZE, REQ_OFF,
    SECTOR_BYTES, STATUS_OFF,
};

/// Chữ ký runner ghi vào 8 byte đầu sector 0 của đĩa tạm (`run-x86_64.sh`: `VIFASTOS-KT288-S0`).
pub const SECTOR0_MAGIC: [u8; 8] = *b"VIFASTOS";

/// Trần poll (µs) của PIT kênh 2 — 2 s, cùng ngưỡng bản KT-288 (không nới).
pub const POLL_TIMEOUT_US: u64 = 2_000_000;

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
// SAFETY: ảnh chạy một CPU trên đường driver và driver tự tuần tự hoá (một request một lúc);
// vùng này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
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
        if align > DMA_PAGE_BYTES || !DMA_PAGE_BYTES.is_multiple_of(align) {
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
        // x86_64: DMA nhất quán với cache (không cần `clflush`) — no-op có chủ đích.
    }
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {
        // Như `sync_for_device`.
    }
}

// ---------------------------------------------------------------- adapter đồng hồ (PIT µs)

/// Bọc `now: &mut dyn FnMut() -> u64` của ảnh thành [`Clock`] — driver **không** giữ trạng thái
/// thời gian (cùng khuôn `virtio_net` x86_64).
struct PitClock<'a>(&'a mut dyn FnMut() -> u64);

impl Clock for PitClock<'_> {
    fn now(&mut self) -> u64 {
        (self.0)()
    }
    fn deadline(&mut self) -> Option<u64> {
        Some(self.now().saturating_add(POLL_TIMEOUT_US))
    }
    fn spin_limit(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------- BAR rời nhau (leaf riêng)

/// `true` ⇔ hai vùng BAR **rời nhau** (không chồng byte nào) và cả hai đều hợp lệ.
///
/// Trên x86_64 firmware gán BAR, nên "leaf riêng" không phải một hằng địa chỉ như aarch64 mà là
/// **bất biến kiểm được**: BAR của blk không được trùng/đè BAR của virtio-net. Hàm này là chỗ
/// duy nhất quyết định điều đó (ảnh in `bar_distinct=1`, test host chốt cả ca âm).
///
/// Fail-closed: BAR chưa gán (`0`), cỡ `0`, hoặc phép cộng tràn 64 bit ⇒ `false`.
pub const fn bar_disjoint(a_base: u64, a_size: u64, b_base: u64, b_size: u64) -> bool {
    if a_base == 0 || b_base == 0 || a_size == 0 || b_size == 0 {
        return false;
    }
    let Some(a_end) = a_base.checked_add(a_size) else {
        return false;
    };
    let Some(b_end) = b_base.checked_add(b_size) else {
        return false;
    };
    a_end <= b_base || b_end <= a_base
}

// ---------------------------------------------------------------- trạng thái + API cũ

static BLK: vifast_virtio::blkdev::Blk = vifast_virtio::blkdev::Blk::new();

/// Khởi tạo driver từ kết quả PCI (BAR đã do firmware gán).
pub fn init(pci: &PciReport) -> Result<BlkReport, &'static str> {
    let transport = blkdev::Transport::from_report(pci);
    BLK.init(&mut Mmio, &transport, &mut Dma)
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    BLK.stats()
}

/// Dung lượng thiết bị theo sector 512 B (0 nếu chưa init).
pub fn capacity_sectors() -> u64 {
    BLK.capacity_sectors()
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    BLK.ready()
}

/// Gửi một request đọc, kiểm dải trước.
pub fn read_sector(
    lba: u32,
    out: &mut [u8; SECTOR_BYTES],
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    let mut clock = PitClock(now);
    BLK.read_sector(&mut Mmio, &mut Dma, &mut clock, lba, out)
}

/// Gửi một request ghi, kiểm dải trước.
pub fn write_sector(
    lba: u32,
    data: &[u8; SECTOR_BYTES],
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    let mut clock = PitClock(now);
    BLK.write_sector(&mut Mmio, &mut Dma, &mut clock, lba, data)
}

/// `VIRTIO_BLK_T_FLUSH` thật (chỉ khi đã đàm phán feature).
pub fn flush(now: &mut dyn FnMut() -> u64) -> Result<(), BlkError> {
    let mut clock = PitClock(now);
    BLK.flush(&mut Mmio, &mut Dma, &mut clock)
}

/// **Chỉ cho ca âm của TAP**: đọc vượt kiểm dải để chứng minh thiết bị trả `IOERR` thật.
pub fn read_sector_force(
    lba: u32,
    out: &mut [u8; SECTOR_BYTES],
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    let mut clock = PitClock(now);
    BLK.read_sector_force(&mut Mmio, &mut Dma, &mut clock, lba, out)
}

/// Địa chỉ ISR (chẩn đoán; bước này chưa dùng ngắt).
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

/// Địa chỉ `device_cfg` (chẩn đoán).
pub fn device_cfg_addr() -> u64 {
    BLK.device_cfg_addr()
}

/// `true` ⇔ 8 byte đầu khớp chữ ký của runner (hàm thuần, dùng chung 3 arch).
pub fn has_magic(bytes: &[u8]) -> bool {
    blkdev::has_magic(bytes, &SECTOR0_MAGIC)
}

/// Bằng chứng sector 0 (hex/ascii/magic) — cùng chữ ký cũ, chữ ký lấy từ [`SECTOR0_MAGIC`].
pub fn sector0_evidence(bytes: &[u8; 16]) -> Sector0 {
    blkdev::sector0_evidence(bytes, &SECTOR0_MAGIC)
}
