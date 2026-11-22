// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Driver virtio-blk (virtio 1.0, virtio-pci modern)** — phần **thuần** dùng chung 3 arch
//! (KT-334, D2-231/D2-239).
//!
//! # Vì sao tệp này tồn tại
//!
//! Ba `kernel/vifast-plat-*/src/virtio_blk.rs` trước KT-334 là ba bản sao của **cùng** một driver:
//! lập trình status/feature/queue của common config, dựng split virtqueue, gửi `IN`/`OUT`/`FLUSH`,
//! poll `used`, đọc `virtio_blk_config`. Chúng chỉ khác nhau ở **cách chạm phần cứng** (MMIO, PA,
//! cache) và **đồng hồ**. Tệp này giữ toàn bộ **quyết định**; platform chỉ còn adapter:
//!
//! ```text
//! vifast_virtio::blkdev::Blk (tệp này)
//!         │  MmioBus (đọc/ghi thanh ghi), DmaPool (vùng DMA + sync_for_device/cpu), Clock
//!         ▼
//! kernel/vifast-plat-*  — adapter `unsafe` tối giản (D2-17)
//! ```
//!
//! # Bất biến giữ nguyên từ KT-72/KT-273/KT-288
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1` (bit 32) + `VIRTIO_BLK_F_FLUSH` **nếu**
//!   thiết bị có — quyết định ở `crate::blk::negotiated_features` (test host).
//! - Thứ tự spec §2.7: ghi descriptor → `avail->ring` → **barrier** → `avail->idx` → **barrier**
//!   → notify → poll `used->idx` → **barrier** → đọc `used->elem`/status.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi.
//! - LBA ngoài dung lượng bị **kiểm trước** ⇒ `OutOfRange`, không phát request.
//! - **DMA cache**: gọi `DmaPool::sync_for_device` **trước** notify và `sync_for_cpu` **sau** khi
//!   thiết bị báo xong (aarch64 `dc civac`/`dc ivac`, KT-277). Test host đếm lời gọi nên bỏ một
//!   trong hai là đỏ.
//! - Fail-closed: thiếu thiết bị/feature/queue ⇒ `Err(stage)`; không PANIC, không treo vô hạn.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::blk::{
    build_request, decode_config, flush_negotiated, negotiated_features, status_to_result,
    BlkError, BLK_CONFIG_BYTES, T_FLUSH, T_IN, T_OUT,
};
pub use crate::dma::{DmaBuf, DmaPool};
use crate::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
pub use crate::mmio::MmioBus;
use crate::pcibus::PciReport;
use crate::queue::{Desc, DescFlags, SplitQueue, VqLayout};

/// Số descriptor của queue (luỹ thừa 2). 8 đủ cho một request 3 descriptor và rẻ về `.bss`.
pub const QUEUE_SIZE: u16 = 8;
/// Cỡ mỗi sector của `virtio_blk` và của `vifast_fs::BLOCK_SIZE`.
pub const SECTOR_BYTES: usize = 512;
/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset vùng request header trong trang DMA.
pub const REQ_OFF: usize = 0x200;
/// Offset vùng đệm dữ liệu 512 B.
pub const DATA_OFF: usize = 0x220;
/// Offset byte status (thiết bị ghi đè).
pub const STATUS_OFF: usize = 0x420;

/// Chữ ký mà runner ghi vào **8 byte đầu** của sector 0 của đĩa tạm (`VIFASTOS-…`).
///
/// Đây là **hợp đồng với runner**: ảnh in `magic=1` chỉ khi 8 byte đầu khớp đúng chữ ký này, nên
/// "đọc được sector 0" không thể là đọc ra số 0 rồi tự khen.
pub const SECTOR0_MAGIC: [u8; 8] = *b"VIFASTOS";

/// Trần số vòng dự phòng khi adapter không có trần thời gian (`Clock::deadline` trả `None`).
pub const DEFAULT_SPIN_LIMIT: u64 = 200_000_000;

// ---------------------------------------------------------------- thanh ghi common config
const C_DEVICE_FEATURE_SELECT: u64 = 0x00;
const C_DEVICE_FEATURE: u64 = 0x04;
const C_DRIVER_FEATURE_SELECT: u64 = 0x08;
const C_DRIVER_FEATURE: u64 = 0x0c;
const C_NUM_QUEUES: u64 = 0x12;
const C_DEVICE_STATUS: u64 = 0x14;
const C_CONFIG_GENERATION: u64 = 0x15;
const C_QUEUE_SELECT: u64 = 0x16;
const C_QUEUE_SIZE: u64 = 0x18;
const C_QUEUE_ENABLE: u64 = 0x1c;
const C_QUEUE_NOTIFY_OFF: u64 = 0x1e;
const C_QUEUE_DESC: u64 = 0x20;
const C_QUEUE_DRIVER: u64 = 0x28;
const C_QUEUE_DEVICE: u64 = 0x30;

/// Địa chỉ tuyệt đối của bốn vùng capability + siêu dữ liệu PCI cần cho báo cáo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    /// Vendor id (từ PCI config).
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// Bus.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Chỉ số BAR driver dùng.
    pub bar_index: u8,
    /// Địa chỉ BAR.
    pub bar_base: u64,
    /// Cỡ BAR.
    pub bar_size: u64,
    /// Địa chỉ common config (tuyệt đối).
    pub common: u64,
    /// Địa chỉ notify (tuyệt đối).
    pub notify: u64,
    /// Địa chỉ device config (tuyệt đối).
    pub device_cfg: u64,
    /// Địa chỉ ISR (tuyệt đối).
    pub isr: u64,
    /// `notify_off_multiplier`.
    pub notify_mul: u32,
}

impl Transport {
    /// Dựng từ [`PciReport`] — **một** chỗ quy ước "BAR base" (`bar_base` luôn đã được đặt đúng
    /// cho cả hai chế độ gán ở [`crate::pcibus`]).
    pub fn from_report(pci: &PciReport) -> Self {
        let bar = pci.bar_base;
        let caps = pci.caps;
        Self {
            vendor: pci.vendor,
            device: pci.device,
            bus: pci.bus,
            dev: pci.dev,
            func: pci.func,
            bar_index: pci.bar_index,
            bar_base: bar,
            bar_size: pci.bar_size,
            common: bar + caps.common.offset as u64,
            notify: bar + caps.notify.offset as u64,
            device_cfg: bar + caps.device.offset as u64,
            isr: bar + caps.isr.offset as u64,
            notify_mul: caps.notify_off_multiplier,
        }
    }
}

/// Bộ đếm của driver — **ảnh chụp** đọc một lần (D2-102: in từ dữ liệu, không chép số).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Tổng request đã phát (kể cả lỗi).
    pub requests: u64,
    /// Request đọc.
    pub reads: u64,
    /// Request ghi.
    pub writes: u64,
    /// Request flush.
    pub flushes: u64,
    /// Request hoàn tất với status OK.
    pub completions: u64,
    /// Request lỗi (status ≠ 0 hoặc lỗi driver sau khi phát).
    pub errors: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lần `used->id` lệch head (phải 0).
    pub id_mismatch: u64,
    /// Số lần poll quá hạn (phải 0).
    pub timeouts: u64,
}

/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlkReport {
    /// Vendor id.
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// Bus.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Chỉ số BAR driver dùng.
    pub bar_index: u8,
    /// Địa chỉ BAR.
    pub bar_base: u64,
    /// Cỡ BAR.
    pub bar_size: u64,
    /// Địa chỉ common config.
    pub common: u64,
    /// Địa chỉ notify.
    pub notify: u64,
    /// Địa chỉ device config.
    pub device_cfg: u64,
    /// Địa chỉ ISR.
    pub isr: u64,
    /// `notify_off_multiplier`.
    pub notify_mul: u32,
    /// `queue_notify_off` của queue 0.
    pub notify_off: u16,
    /// Địa chỉ cửa thông báo = notify + notify_off * multiplier.
    pub notify_addr: u64,
    /// `num_queues` thiết bị báo.
    pub num_queues: u16,
    /// Cỡ queue thiết bị cho phép tối đa.
    pub queue_max: u16,
    /// Cỡ queue driver đặt.
    pub queue_size: u16,
    /// Feature thiết bị (64 bit, đọc hai trang).
    pub device_features: u64,
    /// Feature driver đã ghi (đọc lại).
    pub driver_features: u64,
    /// `true` ⇔ đàm phán được `VIRTIO_BLK_F_FLUSH` (bit 9).
    pub flush_ok: bool,
    /// `device_status` sau DRIVER_OK.
    pub status: u8,
    /// `config_generation` đọc **sau** khi đọc config.
    pub config_generation: u8,
    /// Dung lượng theo sector 512 B.
    pub capacity: u64,
    /// `size_max` thiết bị.
    pub size_max: u32,
    /// `seg_max` thiết bị.
    pub seg_max: u32,
    /// `blk_size` thiết bị.
    pub blk_size: u32,
    /// PA trang DMA.
    pub dma_pa: u64,
    /// PA bảng descriptor.
    pub desc: u64,
    /// PA khối `avail`.
    pub avail: u64,
    /// PA khối `used`.
    pub used: u64,
    /// PA vùng request header.
    pub req_pa: u64,
    /// PA vùng dữ liệu 512 B.
    pub data_pa: u64,
    /// PA byte status.
    pub status_pa: u64,
}

/// Đồng hồ của adapter — tách khỏi driver để test host điều khiển được thời gian.
pub trait Clock {
    /// Tick hiện tại (đơn vị do adapter quy ước; phải cùng đơn vị với [`Clock::deadline`]).
    fn now(&mut self) -> u64;

    /// Hạn chót **tuyệt đối** cho một lần chờ; `None` ⇒ dùng [`Clock::spin_limit`].
    fn deadline(&mut self) -> Option<u64>;

    /// Số vòng tối đa khi không có trần thời gian (0 ⇒ dùng [`DEFAULT_SPIN_LIMIT`]).
    fn spin_limit(&self) -> u64;
}

/// Trạng thái driver — mọi trường đổi được đều là `Atomic*` để ảnh boot (một hart trên đường
/// driver) khai `static BLK: Blk` mà **không** cần `UnsafeCell`/`unsafe` ở platform.
pub struct Blk {
    common: AtomicU64,
    notify: AtomicU64,
    device_cfg: AtomicU64,
    isr: AtomicU64,
    notify_mul: AtomicU64,
    notify_off: AtomicU64,
    queue_size: AtomicU64,
    dma_off: AtomicU64,
    dma_pa: AtomicU64,
    req_pa: AtomicU64,
    data_pa: AtomicU64,
    status_pa: AtomicU64,
    last_used: AtomicU64,
    capacity: AtomicU64,
    ready: AtomicBool,
    flush_ok: AtomicBool,
    requests: AtomicU64,
    reads: AtomicU64,
    writes: AtomicU64,
    flushes: AtomicU64,
    completions: AtomicU64,
    errors: AtomicU64,
    notifies: AtomicU64,
    id_mismatch: AtomicU64,
    timeouts: AtomicU64,
}

impl Default for Blk {
    fn default() -> Self {
        Self::new()
    }
}

impl Blk {
    /// Trạng thái rỗng (chưa init).
    pub const fn new() -> Self {
        Self {
            common: AtomicU64::new(0),
            notify: AtomicU64::new(0),
            device_cfg: AtomicU64::new(0),
            isr: AtomicU64::new(0),
            notify_mul: AtomicU64::new(0),
            notify_off: AtomicU64::new(0),
            queue_size: AtomicU64::new(0),
            dma_off: AtomicU64::new(0),
            dma_pa: AtomicU64::new(0),
            req_pa: AtomicU64::new(0),
            data_pa: AtomicU64::new(0),
            status_pa: AtomicU64::new(0),
            last_used: AtomicU64::new(0),
            capacity: AtomicU64::new(0),
            ready: AtomicBool::new(false),
            flush_ok: AtomicBool::new(false),
            requests: AtomicU64::new(0),
            reads: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            flushes: AtomicU64::new(0),
            completions: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            notifies: AtomicU64::new(0),
            id_mismatch: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
        }
    }

    /// Ảnh chụp bộ đếm.
    pub fn stats(&self) -> Stats {
        Stats {
            requests: self.requests.load(Ordering::Relaxed),
            reads: self.reads.load(Ordering::Relaxed),
            writes: self.writes.load(Ordering::Relaxed),
            flushes: self.flushes.load(Ordering::Relaxed),
            completions: self.completions.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            notifies: self.notifies.load(Ordering::Relaxed),
            id_mismatch: self.id_mismatch.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
        }
    }

    /// Dung lượng thiết bị theo sector 512 B (0 nếu chưa init).
    pub fn capacity_sectors(&self) -> u64 {
        self.capacity.load(Ordering::Relaxed)
    }

    /// `true` ⇔ driver đã init thành công.
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// `true` ⇔ đã đàm phán `VIRTIO_BLK_F_FLUSH`.
    pub fn flush_ok(&self) -> bool {
        self.flush_ok.load(Ordering::Relaxed)
    }

    /// Địa chỉ ISR (chẩn đoán; K6 chưa dùng ngắt).
    pub fn isr_addr(&self) -> u64 {
        self.isr.load(Ordering::Relaxed)
    }

    /// Địa chỉ `device_cfg` (chẩn đoán).
    pub fn device_cfg_addr(&self) -> u64 {
        self.device_cfg.load(Ordering::Relaxed)
    }

    /// PA trang DMA đã cấp.
    pub fn dma_pa(&self) -> u64 {
        self.dma_pa.load(Ordering::Relaxed)
    }

    /// Khởi tạo driver trên transport + vùng DMA đã cấp. Lỗi trả **tên chặng** (`&'static str`)
    /// để TAP in ra lý do đo được — không PANIC im lặng.
    pub fn init<B: MmioBus, D: DmaPool>(
        &self,
        bus: &mut B,
        cfg: &Transport,
        dma: &mut D,
    ) -> Result<BlkReport, &'static str> {
        let layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
        if layout.total_bytes() > DMA_PAGE_BYTES {
            return Err("layout");
        }
        let page: DmaBuf = dma
            .alloc(DMA_PAGE_BYTES, DMA_PAGE_BYTES)
            .ok_or("dma_align")?;
        if page.len < DMA_PAGE_BYTES || page.pa & 0xfff != 0 {
            return Err("dma_align");
        }
        // Vùng DMA sạch trước khi bàn giao (kể cả khi init gọi lại).
        dma.zero(page);
        let common = cfg.common;
        let notify = cfg.notify;
        let device_cfg = cfg.device_cfg;
        let isr = cfg.isr;
        let notify_mul = cfg.notify_mul;

        // ---- 1) reset (§2.4.2 bước 1)
        bus.write8(common + C_DEVICE_STATUS, 0);
        if bus.read8(common + C_DEVICE_STATUS) != 0 {
            return Err("reset");
        }
        // ---- 2) ACK + DRIVER
        bus.write8(common + C_DEVICE_STATUS, ACK);
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER);
        // ---- 3) feature thiết bị (hai trang 32 bit)
        bus.write32(common + C_DEVICE_FEATURE_SELECT, 0);
        let low = bus.read32(common + C_DEVICE_FEATURE) as u64;
        bus.write32(common + C_DEVICE_FEATURE_SELECT, 1);
        let high = bus.read32(common + C_DEVICE_FEATURE) as u64;
        let device_features = (high << 32) | low;
        // VIRTIO_F_VERSION_1 bắt buộc: thiếu ⇒ từ chối, **không** chạy với thiết bị legacy.
        if device_features & (1u64 << F_VERSION_1) == 0 {
            return Err("version1");
        }
        // ---- 4) đàm phán: VERSION_1 bắt buộc + `VIRTIO_BLK_F_FLUSH` **nếu** thiết bị có (CAO-1:
        // gửi `T_FLUSH` mà không đàm phán là vi phạm spec §5.2.3). Quyết định nằm ở hàm thuần.
        let wanted = negotiated_features(device_features);
        if wanted & (1u64 << F_VERSION_1) == 0 {
            return Err("version1");
        }
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 0);
        bus.write32(common + C_DRIVER_FEATURE, (wanted & 0xffff_ffff) as u32);
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 1);
        bus.write32(common + C_DRIVER_FEATURE, (wanted >> 32) as u32);
        // đọc lại cả hai trang
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 0);
        let drv_low = bus.read32(common + C_DRIVER_FEATURE) as u64;
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 1);
        let drv_high = bus.read32(common + C_DRIVER_FEATURE) as u64;
        let driver_features = (drv_high << 32) | drv_low;
        if driver_features != wanted {
            return Err("features");
        }
        let flush_ok = flush_negotiated(driver_features);
        // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !features_ok(status) {
            return Err("features_ok");
        }
        // ---- 6) queue 0
        let num_queues = bus.read16(common + C_NUM_QUEUES);
        if num_queues == 0 {
            return Err("queues");
        }
        bus.write16(common + C_QUEUE_SELECT, 0);
        let queue_max = bus.read16(common + C_QUEUE_SIZE);
        // Chain của driver dùng 3 descriptor (header + data + status; FLUSH dùng 2) ⇒ queue **phải**
        // chứa được 3; `queue_size = 2` từng làm `set_desc(2)` bị bỏ im lặng (D2-110 medium).
        if queue_max < 3 || !queue_max.is_power_of_two() {
            return Err("queue_size");
        }
        let queue_size = QUEUE_SIZE.min(queue_max);
        bus.write16(common + C_QUEUE_SIZE, queue_size);
        if bus.read16(common + C_QUEUE_SIZE) != queue_size {
            return Err("queue_size");
        }
        let dma_off = page.off as u64;
        let desc_pa = page.pa + layout.desc_off as u64;
        let avail_pa = page.pa + layout.avail_off as u64;
        let used_pa = page.pa + layout.used_off as u64;
        let req_pa = page.pa + REQ_OFF as u64;
        let data_pa = page.pa + DATA_OFF as u64;
        let status_pa = page.pa + STATUS_OFF as u64;
        bus.write64(common + C_QUEUE_DESC, desc_pa);
        bus.write64(common + C_QUEUE_DRIVER, avail_pa);
        bus.write64(common + C_QUEUE_DEVICE, used_pa);
        bus.fence();
        bus.write16(common + C_QUEUE_ENABLE, 1);
        if bus.read16(common + C_QUEUE_ENABLE) != 1 {
            return Err("queue_enable");
        }
        let notify_off = bus.read16(common + C_QUEUE_NOTIFY_OFF);
        let notify_addr = notify + notify_off as u64 * notify_mul as u64;
        // ---- 7) DRIVER_OK
        bus.write8(
            common + C_DEVICE_STATUS,
            ACK | DRIVER | FEATURES_OK | DRIVER_OK,
        );
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !status_sequence_ok(status) {
            return Err("driver_ok");
        }
        // ---- 8) device config: capacity/size_max/seg_max/blk_size đọc từ thiết bị
        let generation_before = bus.read8(common + C_CONFIG_GENERATION);
        let mut raw = [0u8; BLK_CONFIG_BYTES];
        bus.read_bytes(device_cfg, &mut raw);
        let config_generation = bus.read8(common + C_CONFIG_GENERATION);
        // Spec §2.4.4: nếu `config_generation` đổi trong lúc đọc config thì dữ liệu có thể **xé**
        // ⇒ fail-closed (không dùng capacity đo được nửa vời).
        if config_generation != generation_before {
            return Err("config_generation");
        }
        let config = decode_config(&raw).ok_or("config")?;
        if config.capacity_sectors == 0 {
            return Err("capacity");
        }
        let capacity = config.capacity_sectors;
        if capacity > u32::MAX as u64 {
            return Err("capacity");
        }
        // ---- 9) lưu trạng thái
        self.common.store(common, Ordering::Relaxed);
        self.notify.store(notify, Ordering::Relaxed);
        self.device_cfg.store(device_cfg, Ordering::Relaxed);
        self.isr.store(isr, Ordering::Relaxed);
        self.notify_mul.store(notify_mul as u64, Ordering::Relaxed);
        self.notify_off.store(notify_off as u64, Ordering::Relaxed);
        self.queue_size.store(queue_size as u64, Ordering::Relaxed);
        self.dma_off.store(dma_off, Ordering::Relaxed);
        self.dma_pa.store(page.pa, Ordering::Relaxed);
        self.req_pa.store(req_pa, Ordering::Relaxed);
        self.data_pa.store(data_pa, Ordering::Relaxed);
        self.status_pa.store(status_pa, Ordering::Relaxed);
        self.last_used.store(0, Ordering::Relaxed);
        self.capacity.store(capacity, Ordering::Relaxed);
        self.flush_ok.store(flush_ok, Ordering::Relaxed);
        self.ready.store(true, Ordering::Relaxed);

        Ok(BlkReport {
            vendor: cfg.vendor,
            device: cfg.device,
            bus: cfg.bus,
            dev: cfg.dev,
            func: cfg.func,
            bar_index: cfg.bar_index,
            bar_base: cfg.bar_base,
            bar_size: cfg.bar_size,
            common,
            notify,
            device_cfg,
            isr,
            notify_mul,
            notify_off,
            notify_addr,
            num_queues,
            queue_max,
            queue_size,
            device_features,
            driver_features,
            flush_ok,
            status,
            config_generation,
            capacity,
            size_max: config.size_max,
            seg_max: config.seg_max,
            blk_size: config.blk_size,
            dma_pa: page.pa,
            desc: desc_pa,
            avail: avail_pa,
            used: used_pa,
            req_pa,
            data_pa,
            status_pa,
        })
    }

    /// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều request).
    pub fn device_status<B: MmioBus>(&self, bus: &mut B) -> u8 {
        let common = self.common.load(Ordering::Relaxed);
        if common == 0 {
            return 0;
        }
        bus.read8(common + C_DEVICE_STATUS)
    }

    /// Đọc `config_generation` hiện tại.
    pub fn config_generation<B: MmioBus>(&self, bus: &mut B) -> u8 {
        let common = self.common.load(Ordering::Relaxed);
        if common == 0 {
            return 0;
        }
        bus.read8(common + C_CONFIG_GENERATION)
    }

    /// Đọc một sector, **kiểm dải trước** (không phát request ngoài dung lượng).
    pub fn read_sector<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        lba: u32,
        out: &mut [u8; SECTOR_BYTES],
    ) -> Result<(), BlkError> {
        if lba as u64 >= self.capacity_sectors() {
            return Err(BlkError::OutOfRange);
        }
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.submit(bus, dma, clock, T_IN, lba as u64, None, Some(out))
    }

    /// Ghi một sector, kiểm dải trước.
    pub fn write_sector<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        lba: u32,
        data: &[u8; SECTOR_BYTES],
    ) -> Result<(), BlkError> {
        if lba as u64 >= self.capacity_sectors() {
            return Err(BlkError::OutOfRange);
        }
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.submit(bus, dma, clock, T_OUT, lba as u64, Some(data), None)
    }

    /// Đẩy cache ghi của thiết bị xuống phương tiện (`VIRTIO_BLK_T_FLUSH`) — **thật**, không no-op.
    pub fn flush<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
    ) -> Result<(), BlkError> {
        if !self.ready() {
            return Err(BlkError::NotReady);
        }
        // **Không** gửi lệnh của feature chưa đàm phán (spec §5.2.3).
        if !self.flush_ok() {
            return Err(BlkError::Unsupported);
        }
        self.flushes.fetch_add(1, Ordering::Relaxed);
        self.submit(bus, dma, clock, T_FLUSH, 0, None, None)
    }

    /// **Chỉ cho ca âm của TAP**: phát request đọc **vượt** kiểm dải để chứng minh thiết bị trả
    /// `IOERR` thật (và không treo). Không dùng trong đường FS.
    pub fn read_sector_force<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        lba: u32,
        out: &mut [u8; SECTOR_BYTES],
    ) -> Result<(), BlkError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.submit(bus, dma, clock, T_IN, lba as u64, None, Some(out))
    }

    /// Gửi một request và chờ `used` — điểm **duy nhất** phát request (mọi lối vào đi qua đây để
    /// bộ đếm và thứ tự barrier/cache không bị bỏ sót).
    // `submit` gom đủ thứ một request cần (bus/dma/clock/loại/sector/hai đệm); tách nhỏ chỉ
    // làm đường phát request khó đọc — đây là **một** chỗ duy nhất, không phải API công khai.
    #[allow(clippy::too_many_arguments)]
    fn submit<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        kind: u32,
        sector: u64,
        data_in: Option<&[u8; SECTOR_BYTES]>,
        data_out: Option<&mut [u8; SECTOR_BYTES]>,
    ) -> Result<(), BlkError> {
        if !self.ready() {
            return Err(BlkError::NotReady);
        }
        let queue_size = self.queue_size.load(Ordering::Relaxed) as u16;
        let layout = VqLayout::new(queue_size).ok_or(BlkError::NotReady)?;
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let req_pa = self.req_pa.load(Ordering::Relaxed);
        let data_pa = self.data_pa.load(Ordering::Relaxed);
        let status_pa = self.status_pa.load(Ordering::Relaxed);
        // Dữ liệu vào (OUT) chép **vào** vùng DMA; FLUSH không có dữ liệu.
        if let Some(bytes) = data_in {
            let at = dma_off + DATA_OFF;
            dma.bytes()[at..at + SECTOR_BYTES].copy_from_slice(bytes);
        }
        // Header + status khởi tạo (status = 0xff để phát hiện thiết bị không ghi).
        let req = build_request(kind, sector, 1);
        {
            let bytes = dma.bytes();
            bytes[dma_off + REQ_OFF..dma_off + REQ_OFF + 16].copy_from_slice(&req.header);
            bytes[dma_off + STATUS_OFF] = 0xff;
        }

        self.requests.fetch_add(1, Ordering::Relaxed);
        let status_index = req.status_index;
        {
            let mem = &mut dma.bytes()[dma_off..dma_off + DMA_PAGE_BYTES];
            let mut q = SplitQueue::new(mem, queue_size).ok_or(BlkError::NotReady)?;
            if !q.set_desc(
                0,
                Desc {
                    addr: req_pa,
                    len: 16,
                    flags: DescFlags::NEXT,
                    next: 1,
                },
            ) {
                return Err(BlkError::NotReady);
            }
            if kind != T_FLUSH {
                let flags = if kind == T_IN {
                    // device-writable
                    DescFlags(DescFlags::NEXT.bits() | DescFlags::WRITE.bits())
                } else {
                    DescFlags::NEXT
                };
                if !q.set_desc(
                    1,
                    Desc {
                        addr: data_pa,
                        // Độ dài descriptor lấy từ **cùng** giá trị đã dựng request — không viết
                        // `SECTOR_BYTES` lần thứ hai (đó đúng là chỗ lỗi v1 #2 sinh ra).
                        len: req.data_len,
                        flags,
                        next: 2,
                    },
                ) {
                    return Err(BlkError::NotReady);
                }
            }
            if !q.set_desc(
                status_index,
                Desc {
                    addr: status_pa,
                    len: 1,
                    flags: DescFlags::WRITE,
                    next: 0,
                },
            ) {
                return Err(BlkError::NotReady);
            }
            // avail -> fence -> idx: `push_avail` ghi ring rồi idx; fence **sau** để thiết bị chỉ
            // thấy idx khi ring đã xong. (Spec §2.7.13.)
            q.push_avail(0);
        }
        // **Cache**: bàn giao **toàn** vùng DMA cho thiết bị trước khi báo (aarch64: `dc civac`
        // + `dsb`). Thiếu lời gọi này ⇒ test host đếm lời gọi ĐỎ, và trên aarch64 thiết bị có
        // thể đọc bản cũ trong RAM (KT-277).
        dma.sync_for_device(dma_off, DMA_PAGE_BYTES);
        bus.fence();
        // Ghi **index queue** vào cửa thông báo (driver → thiết bị). `notify_off` đã đọc lúc init
        // và **cache** trong trạng thái (không ghi `queue_select` giữa lúc request đang bay).
        let notify = self.notify.load(Ordering::Relaxed);
        let notify_off = self.notify_off.load(Ordering::Relaxed);
        let notify_mul = self.notify_mul.load(Ordering::Relaxed);
        bus.notify(notify + notify_off * notify_mul, 0);
        self.notifies.fetch_add(1, Ordering::Relaxed);

        // ---- poll used (một chiều) — offset lấy từ **queue_size thật** của queue.
        let deadline = clock.deadline();
        let spin_limit = match clock.spin_limit() {
            0 => DEFAULT_SPIN_LIMIT,
            limit => limit,
        };
        let mut spins = 0u64;
        loop {
            // **Cache**: bỏ dòng chứa `used->idx` trước mỗi lần đọc (aarch64 `dc ivac`). Thiết bị
            // ghi qua DMA (không đi qua cache CPU) — đọc không bỏ cache có thể thấy mãi giá trị cũ.
            dma.sync_for_cpu(dma_off + layout.used_idx_off, 2);
            // `used->idx` nằm trong **RAM** (DMA), không phải MMIO ⇒ đọc qua pool (volatile).
            let idx = dma.read_u16(dma_off + layout.used_idx_off);
            let last = self.last_used.load(Ordering::Relaxed) as u16;
            if idx != last {
                // Thiết bị đã ghi xong — bỏ cache **trước** khi đọc `used->elem`, byte status và
                // đệm dữ liệu (aarch64 `dc ivac` toàn trang).
                dma.sync_for_cpu(dma_off, DMA_PAGE_BYTES);
                bus.fence();
                let element = {
                    let mem = &mut dma.bytes()[dma_off..dma_off + DMA_PAGE_BYTES];
                    let q = SplitQueue::new(mem, queue_size).ok_or(BlkError::NotReady)?;
                    q.used_elem(last & (queue_size - 1))
                };
                self.last_used
                    .store(last.wrapping_add(1) as u64, Ordering::Relaxed);
                if element.0 != 0 {
                    self.id_mismatch.fetch_add(1, Ordering::Relaxed);
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    return Err(BlkError::Io);
                }
                let status = dma.read_u8(dma_off + STATUS_OFF);
                match status_to_result(status) {
                    Ok(()) => {
                        self.completions.fetch_add(1, Ordering::Relaxed);
                        if let Some(out) = data_out {
                            let at = dma_off + DATA_OFF;
                            out.copy_from_slice(&dma.bytes()[at..at + SECTOR_BYTES]);
                        }
                        return Ok(());
                    }
                    Err(error) => {
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        return Err(error);
                    }
                }
            }
            match deadline {
                Some(limit) => {
                    if clock.now() > limit {
                        self.timeouts.fetch_add(1, Ordering::Relaxed);
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        return Err(BlkError::Timeout);
                    }
                }
                None => {
                    spins += 1;
                    if spins > spin_limit {
                        self.timeouts.fetch_add(1, Ordering::Relaxed);
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        return Err(BlkError::Timeout);
                    }
                }
            }
            core::hint::spin_loop();
        }
    }
}

// ---------------------------------------------------------------- bằng chứng sector 0 (thuần)

/// 16 byte đầu → **32 ký tự hex thường** (không dấu cách) — đúng thứ runner grep.
pub fn hex16(bytes: &[u8; 16]) -> [u8; 32] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 16 {
        out[i * 2] = DIGITS[(bytes[i] >> 4) as usize];
        out[i * 2 + 1] = DIGITS[(bytes[i] & 0xf) as usize];
        i += 1;
    }
    out
}

/// 16 byte → ASCII **chỉ khi mọi byte in được** (`0x20..=0x7e`); ngược lại `None`.
///
/// Vì sao không thay byte không in được bằng `'.'`: log là **bằng chứng**, không phải bản xem
/// trước — một chuỗi "trông như chữ" từ đĩa nhị phân sẽ đánh lừa người đọc.
pub fn ascii16(bytes: &[u8; 16]) -> Option<[u8; 16]> {
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        let byte = bytes[i];
        if !(0x20..=0x7e).contains(&byte) {
            return None;
        }
        out[i] = byte;
        i += 1;
    }
    Some(out)
}

/// `true` ⇔ `magic` là **tiền tố đúng byte** của `bytes` (ngắn hơn chữ ký ⇒ `false`).
pub fn has_magic(bytes: &[u8], magic: &[u8; 8]) -> bool {
    if bytes.len() < magic.len() {
        return false;
    }
    let mut i = 0;
    while i < magic.len() {
        if bytes[i] != magic[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Ba trường bằng chứng của 16 byte đầu sector 0 — **một** giá trị để log và test không lệch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sector0 {
    /// Hex 32 ký tự.
    pub hex: [u8; 32],
    /// ASCII nếu mọi byte in được.
    pub ascii: Option<[u8; 16]>,
    /// `true` ⇔ 8 byte đầu khớp chữ ký.
    pub magic: bool,
}

/// Dựng bằng chứng sector 0 từ 16 byte đầu.
pub fn sector0_evidence(bytes: &[u8; 16], magic: &[u8; 8]) -> Sector0 {
    Sector0 {
        hex: hex16(bytes),
        ascii: ascii16(bytes),
        magic: has_magic(bytes, magic),
    }
}

const _: () = {
    // `LAYOUT` của queue phải nằm gọn **trước** vùng request trong cùng trang DMA.
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(l) => l,
        None => panic!("queue size khong hop le"),
    };
    assert!(layout.end <= REQ_OFF);
    assert!(REQ_OFF + 16 <= DATA_OFF);
    assert!(DATA_OFF + SECTOR_BYTES <= STATUS_OFF);
    assert!(STATUS_OFF < DMA_PAGE_BYTES);
    // Chữ ký phải là tiền tố của chuỗi 17 byte mà runner ghi vào đĩa tạm.
    assert!(SECTOR0_MAGIC[0] == b'V' && SECTOR0_MAGIC[7] == b'S');
};
