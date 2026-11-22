// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-blk` trên aarch64 — KT-334 (trước: driver đầy đủ KT-273/KT-277).
//!
//! Quyết định giao thức (status/feature/queue, descriptor, notify, poll, config) nằm ở
//! [`vifast_virtio::blkdev`] — crate **thuần**, test host đầy đủ. Tệp này giữ đúng những gì thuộc
//! nền tảng aarch64:
//!
//! 1. **PA của DMA**: trang DMA là **frame động** cấp từ arena ngoài ảnh (KT-338a), map
//!    **identity** (`VA == PA`) ⇒ caller truyền thẳng **PA** vào [`init`]; các lần sau dùng PA đã
//!    lưu. [`high_to_pa`] chỉ còn dùng cho địa chỉ **trong ảnh** (U-mode/host test).
//! 2. **Cache maintenance** (KT-277): [`Dma`] cài `sync_for_device` = `dc civac` từng dòng
//!    (`cache::HANDOFF_SEQUENCE`, bỏ bước `Notify` vì [`MmioBus::notify`] của transport đã làm)
//!    và `sync_for_cpu` = `dc ivac` — toàn trang sau hoàn tất, một dòng trong vòng poll.
//! 3. **Đồng hồ**: `CNTPCT_EL0`; `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ lùi về trần **số vòng**
//!    [`POLL_SPIN_LIMIT`], có ghi lý do như bản cũ.
//!
//! Bộ đếm cache (cho dòng `# dma:` trong TAP) đếm **chính các lệnh `dc` đã chạy** trong adapter —
//! bỏ lời gọi `dc` ⇒ `lines=0` ⇒ runner ĐỎ (tầng quan sát được trên QEMU TCG; hiệu ứng cache thì
//! TCG không mô hình hoá — ghi trung thực ở `v2/docs/k4-aarch64-boot.md` §11).

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_virtio::blk::BlkError;
use vifast_virtio::blkdev::{self, Clock, DmaBuf, DmaPool, MmioBus};

use crate::asm;
use crate::cache;
use crate::pci::PciReport;

pub use vifast_virtio::blkdev::{
    ascii16, hex16, BlkReport, Sector0, Stats, DATA_OFF, DMA_PAGE_BYTES, QUEUE_SIZE, REQ_OFF,
    SECTOR0_MAGIC, SECTOR_BYTES, STATUS_OFF,
};

/// Số giây trần khi poll `used` (spec §2.7.13: không ngắt thì driver poll, phải có trần).
pub const POLL_TIMEOUT_SECS: u64 = 2;
/// Trần **số vòng** dự phòng khi `CNTFRQ_EL0 = 0` (không có trần thời gian).
pub const POLL_SPIN_LIMIT: u64 = 200_000_000;

// Bất biến biên dịch: một trang DMA phải chứa trọn ring 8 descriptor của virtqueue — đây là lý
// do `QUEUE_SIZE` và bố cục trang trong `vifast_virtio::blkdev` khớp nhau (layout lấy từ crate).
const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2 trong [2, 256]"),
    };
    assert!(DMA_PAGE_BYTES >= layout.total_bytes());
};

// ---------------------------------------------------------------- cache maintenance (KT-277)
//
// Bộ đếm **của chính vòng lặp phát `dc`** — nhờ vậy `# dma: … lines=` là số lệnh `dc` **đã
// chạy**, không phải một hằng viết tay.
static CLEAN_LINES: AtomicU64 = AtomicU64::new(0);
static CLEAN_BYTES: AtomicU64 = AtomicU64::new(0);
static CLEAN_OPS: AtomicU64 = AtomicU64::new(0);
static INVAL_LINES: AtomicU64 = AtomicU64::new(0);
static INVAL_BYTES: AtomicU64 = AtomicU64::new(0);
static INVAL_OPS: AtomicU64 = AtomicU64::new(0);
static POLL_INVAL: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm cache maintenance — in vào dòng `# dma:` của ảnh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Số dòng đã `dc civac` **trước** khi bàn giao ring (tổng qua mọi request).
    pub clean_lines: u64,
    /// Số byte vùng DMA đã làm sạch (tổng, mỗi request một lần toàn trang).
    pub clean_bytes: u64,
    /// Số lần chạy chuỗi bàn giao (mỗi request một lần).
    pub clean_ops: u64,
    /// Số dòng đã `dc ivac` **sau** khi thiết bị báo hoàn tất.
    pub inval_lines: u64,
    /// Số byte vùng DMA đã bỏ cache sau khi thiết bị ghi.
    pub inval_bytes: u64,
    /// Số lần chạy chuỗi hoàn tất.
    pub inval_ops: u64,
    /// Số lần `dc ivac` một dòng trong vòng poll `used->idx`.
    pub poll_inval: u64,
}

impl CacheStats {
    /// `true` ⇔ **cả hai** chiều đã chạy ít nhất một lần và mỗi lần làm sạch đều có dòng thật.
    pub const fn ok(self) -> bool {
        self.clean_ops > 0
            && self.clean_lines > 0
            && self.inval_ops > 0
            && self.inval_lines > 0
            && self.clean_bytes > 0
            && self.inval_bytes > 0
    }
}

/// Đọc ảnh chụp bộ đếm cache maintenance.
pub fn cache_stats() -> CacheStats {
    CacheStats {
        clean_lines: CLEAN_LINES.load(Ordering::SeqCst),
        clean_bytes: CLEAN_BYTES.load(Ordering::SeqCst),
        clean_ops: CLEAN_OPS.load(Ordering::SeqCst),
        inval_lines: INVAL_LINES.load(Ordering::SeqCst),
        inval_bytes: INVAL_BYTES.load(Ordering::SeqCst),
        inval_ops: INVAL_OPS.load(Ordering::SeqCst),
        poll_inval: POLL_INVAL.load(Ordering::SeqCst),
    }
}

// ---------------------------------------------------------------- PA của trang DMA

/// PA trang DMA — đặt lúc [`init`] (caller truyền thẳng PA; trang là frame động identity-map).
static DMA_PA: AtomicU64 = AtomicU64::new(0);

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết.
///
/// Sau khi nhảy nửa cao, `adrp` cho alias `KERNEL_HIGH_BASE + (pa - text_start)`; PA thật là
/// `text_start_pa + (va - KERNEL_HIGH_BASE)`. Chỉ đúng cho địa chỉ **trong ảnh** — trang DMA
/// từ KT-338a nằm **ngoài** ảnh (arena, identity `VA == PA`) nên **không** dùng hàm này.
pub const fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - crate::split_mmu::KERNEL_HIGH_BASE)
}

/// Bộ đếm vật lý hiện tại (`CNTPCT_EL0`).
pub fn now_ticks() -> u64 {
    asm::read_cntpct_el0()
}

// ---------------------------------------------------------------- adapter MMIO

struct Mmio;

impl MmioBus for Mmio {
    fn read8(&self, addr: u64) -> u8 {
        asm::mmio_read_u8(addr)
    }
    fn read16(&self, addr: u64) -> u16 {
        asm::mmio_read_u16(addr)
    }
    fn read32(&self, addr: u64) -> u32 {
        asm::mmio_read_u32(addr)
    }
    fn write8(&mut self, addr: u64, value: u8) {
        asm::mmio_write_u8(addr, value);
    }
    fn write16(&mut self, addr: u64, value: u16) {
        asm::mmio_write_u16(addr, value);
    }
    fn write32(&mut self, addr: u64, value: u32) {
        asm::mmio_write_u32(addr, value);
    }
    fn write64(&mut self, addr: u64, value: u64) {
        asm::mmio_write_u64(addr, value);
    }
    fn fence(&self) {
        // `dsb sy`: DMA của thiết bị nằm **ngoài** domain shareability của CPU.
        asm::dsb_sy();
    }
}

// ---------------------------------------------------------------- adapter DMA + cache

/// Pool DMA một vùng: frame động identity-map ngoài ảnh (KT-338a), PA do caller truyền thẳng.
struct Dma {
    base: u64,
}

impl DmaPool for Dma {
    fn base_pa(&self) -> u64 {
        self.base
    }
    fn capacity(&self) -> usize {
        DMA_PAGE_BYTES
    }
    fn bytes(&mut self) -> &mut [u8] {
        asm::dma_page()
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        if len == 0 || len > DMA_PAGE_BYTES || align == 0 || !align.is_power_of_two() {
            return None;
        }
        if align > DMA_PAGE_BYTES || !DMA_PAGE_BYTES.is_multiple_of(align) || self.base & 0xfff != 0
        {
            return None;
        }
        Some(DmaBuf {
            off: 0,
            len,
            pa: self.base,
        })
    }
    fn free(&mut self, _buf: DmaBuf) -> bool {
        true
    }
    fn read_u16(&self, off: usize) -> u16 {
        asm::mmio_read_u16(self.base + off as u64)
    }
    fn read_u8(&self, off: usize) -> u8 {
        asm::mmio_read_u8(self.base + off as u64)
    }
    fn sync_for_device(&mut self, off: usize, len: usize) {
        // `dc civac` từng dòng (ghi bản bẩn ra PoC + bỏ bản sao) → `dsb ish` → `dsb sy`.
        // Bước `Notify` của chuỗi do transport làm qua `MmioBus::notify` — adapter **không** ghi
        // cửa thông báo lần thứ hai.
        let addr = self.base + off as u64;
        let mut clean_lines = 0u64;
        for step in cache::HANDOFF_SEQUENCE {
            if matches!(step, cache::HandoffStep::Notify) {
                continue;
            }
            clean_lines += cache::apply_handoff(step, addr, len as u64, 0);
        }
        CLEAN_LINES.fetch_add(clean_lines, Ordering::SeqCst);
        CLEAN_BYTES.fetch_add(len as u64, Ordering::SeqCst);
        CLEAN_OPS.fetch_add(1, Ordering::SeqCst);
    }
    fn sync_for_cpu(&mut self, off: usize, len: usize) {
        let addr = self.base + off as u64;
        if len <= 8 {
            // Một dòng chứa `used->idx` trong vòng poll: mọi dòng đã **sạch** từ lần handoff, nên
            // `dc ivac` chỉ bỏ bản sao, không mất dữ liệu chưa ghi.
            cache::invalidate_line(addr);
            POLL_INVAL.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let mut inval_lines = 0u64;
        for step in cache::COMPLETE_SEQUENCE {
            inval_lines += cache::apply_complete(step, addr, len as u64);
        }
        INVAL_LINES.fetch_add(inval_lines, Ordering::SeqCst);
        INVAL_BYTES.fetch_add(len as u64, Ordering::SeqCst);
        INVAL_OPS.fetch_add(1, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

struct Tick;

impl Clock for Tick {
    fn now(&mut self) -> u64 {
        now_ticks()
    }
    fn deadline(&mut self) -> Option<u64> {
        let freq = asm::read_cntfrq_el0();
        if freq == 0 {
            None
        } else {
            Some(now_ticks().wrapping_add(POLL_TIMEOUT_SECS.wrapping_mul(freq)))
        }
    }
    fn spin_limit(&self) -> u64 {
        POLL_SPIN_LIMIT
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static BLK: vifast_virtio::blkdev::Blk = vifast_virtio::blkdev::Blk::new();

/// Khởi tạo driver từ kết quả PCI. `dma_pa` là **PA** của trang DMA — phải căn 4 KiB.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<BlkReport, &'static str> {
    if dma_pa & 0xfff != 0 {
        return Err("dma_align");
    }
    let transport = blkdev::Transport::from_report(pci);
    let mut dma = Dma { base: dma_pa };
    let report = BLK.init(&mut Mmio, &transport, &mut dma)?;
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    Ok(report)
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
pub fn read_sector(lba: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    let mut dma = dma();
    BLK.read_sector(&mut Mmio, &mut dma, &mut Tick, lba, out)
}

/// Gửi một request ghi, kiểm dải trước.
pub fn write_sector(lba: u32, data: &[u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    let mut dma = dma();
    BLK.write_sector(&mut Mmio, &mut dma, &mut Tick, lba, data)
}

/// `VIRTIO_BLK_T_FLUSH` thật (chỉ khi đã đàm phán feature).
pub fn flush() -> Result<(), BlkError> {
    let mut dma = dma();
    BLK.flush(&mut Mmio, &mut dma, &mut Tick)
}

/// **Chỉ cho ca âm của TAP**: đọc vượt kiểm dải để chứng minh thiết bị trả `IOERR` thật.
pub fn read_sector_force(lba: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    let mut dma = dma();
    BLK.read_sector_force(&mut Mmio, &mut dma, &mut Tick, lba, out)
}

/// Địa chỉ ISR (chẩn đoán; bước 3 chưa dùng ngắt).
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

/// Adapter DMA với PA đã lưu từ [`init`].
fn dma() -> Dma {
    Dma {
        base: DMA_PA.load(Ordering::Relaxed),
    }
}
