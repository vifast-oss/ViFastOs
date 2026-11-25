// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-rng** dùng chung 3 kiến trúc — trên [`MmioBus`] + [`DmaPool`] + [`Clock`]
//! (KT-418b, cùng khuôn `netdev`/`blkdev` của KT-334/335).
//!
//! # Vì sao tệp này tồn tại
//!
//! `virtio-rng` là thiết bị **đơn giản nhất** của virtio: một hàng đợi, một descriptor
//! device-writable, không `device config`. Nhưng nó là **nguồn entropy duy nhất** lấy từ phần cứng
//! ⇒ ba bất biến phải đo được, không được "chạy được là xong":
//!
//! 1. **thứ tự spec §2.7**: ghi descriptor → ghi `avail->ring` → **fence** → ghi `avail->idx` →
//!    **fence** → notify → poll `used->idx` → **fence** → đọc `used->elem`;
//! 2. **`used->id` phải khớp head descriptor (0)** và `used->len` phải trong `[MIN, out.len()]` —
//!    lệch ⇒ [`RngError::Io`] + đếm `errors`, **không** im lặng;
//! 3. **cổng chất lượng** [`crate::rng::entropy_ok`] chạy **trước khi** byte được tính là entropy:
//!    thiết bị trả toàn 0 ⇒ [`RngError::LowEntropy`] + đếm `errors`, `bytes` **không** tăng.
//!
//! # Fail-closed
//!
//! Thiếu thiết bị ⇒ [`RngError::NoDevice`] (không PANIC, không treo); thiết bị không hoàn tất ⇒
//! [`RngError::Timeout`] + đếm `timeouts`; gọi `fill` trước `init` ⇒ [`RngError::NotReady`]; độ
//! dài yêu cầu sai ⇒ [`RngError::BadLen`] **trước** khi chạm ring (không đẩy descriptor rác).
//!
//! # Trạng thái bằng `Atomic*`, không `UnsafeCell`
//!
//! Như [`crate::netdev::Net`]/[`crate::blkdev::Blk`]: mọi trường đổi được là `Atomic*` để nền tảng
//! khai `static RNG: Rng = Rng::new()` mà **không** cần `UnsafeCell`/`unsafe` (D2-17). Ảnh chạy
//! **một** hart trên đường driver và driver tự tuần tự hoá (một request một lúc) ⇒
//! `load`/`store(Relaxed)` là đủ.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::blkdev::Transport;
use crate::dma::{DmaBuf, DmaPool};
use crate::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use crate::mmio::MmioBus;
use crate::queue::{Desc, DescFlags, SplitQueue, VqLayout};
use crate::rng::{
    distinct_bytes, entropy_ok, longest_run, RngError, RngReport, MAX_REQUEST_BYTES,
    MIN_QUEUE_SIZE, MIN_REQUEST_BYTES, QUEUES_REQUIRED, QUEUE_SIZE, Q_REQUEST,
};

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `requestq` trong trang DMA.
pub const RNG_RING_OFF: usize = 0x000;
/// Offset đệm entropy (device-writable) trong trang DMA.
pub const RNG_BUF_OFF: usize = 0x400;
/// Cỡ đệm entropy = trần một request.
pub const RNG_BUF_BYTES: usize = MAX_REQUEST_BYTES;

// ---------------------------------------------------------------- thanh ghi common config
const C_DEVICE_FEATURE_SELECT: u64 = 0x00;
const C_DEVICE_FEATURE: u64 = 0x04;
const C_DRIVER_FEATURE_SELECT: u64 = 0x08;
const C_DRIVER_FEATURE: u64 = 0x0c;
const C_NUM_QUEUES: u64 = 0x12;
const C_DEVICE_STATUS: u64 = 0x14;
const C_QUEUE_SELECT: u64 = 0x16;
const C_QUEUE_SIZE: u64 = 0x18;
const C_QUEUE_ENABLE: u64 = 0x1c;
const C_QUEUE_NOTIFY_OFF: u64 = 0x1e;
const C_QUEUE_DESC: u64 = 0x20;
const C_QUEUE_DRIVER: u64 = 0x28;
const C_QUEUE_DEVICE: u64 = 0x30;

/// Đồng hồ đơn điệu **một hàm** — đơn vị do nền tảng quy ước (tick timebase, µs `CNTPCT`, µs PIT)
/// và **phải** cùng đơn vị với mọi `timeout` truyền vào [`Rng::fill`].
pub trait Clock {
    /// Thời điểm hiện tại.
    fn now(&mut self) -> u64;
}

/// `true` ⇔ `now` **đã** qua `deadline`; an toàn với wrap của bộ đếm (`rdtime`/`CNTPCT`).
pub const fn past_deadline(now: u64, deadline: u64) -> bool {
    now.wrapping_sub(deadline) < u64::MAX / 2
}

/// Driver `virtio-rng` — trạng thái `Atomic*`, khai được `static`.
pub struct Rng {
    ready: AtomicBool,
    device_id: AtomicU64,
    version1: AtomicBool,
    queue_size: AtomicU64,
    requests: AtomicU64,
    bytes: AtomicU64,
    distinct: AtomicU64,
    longest_run: AtomicU64,
    entropy_ok: AtomicBool,
    notifies: AtomicU64,
    timeouts: AtomicU64,
    errors: AtomicU64,
    /// Offset đệm entropy trong pool DMA.
    buf_off: AtomicU64,
    /// PA của đệm entropy.
    buf_pa: AtomicU64,
    /// Địa chỉ tuyệt đối của cửa thông báo hàng đợi 0.
    notify_addr: AtomicU64,
    /// Offset ring trong pool DMA.
    ring_off: AtomicU64,
    /// `used->idx` driver đã thấy (một chiều).
    used_seen: AtomicU64,
}

impl Default for Rng {
    fn default() -> Self {
        Self::new()
    }
}

impl Rng {
    /// Driver **chưa** init — mọi trường 0, `ready = false`.
    pub const fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            device_id: AtomicU64::new(0),
            version1: AtomicBool::new(false),
            queue_size: AtomicU64::new(0),
            requests: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            distinct: AtomicU64::new(0),
            longest_run: AtomicU64::new(0),
            entropy_ok: AtomicBool::new(false),
            notifies: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            buf_off: AtomicU64::new(0),
            buf_pa: AtomicU64::new(0),
            notify_addr: AtomicU64::new(0),
            ring_off: AtomicU64::new(0),
            used_seen: AtomicU64::new(0),
        }
    }

    /// Khởi tạo: `reset → ACK → DRIVER → feature → FEATURES_OK → queue 0 → DRIVER_OK`.
    ///
    /// Trả [`RngReport`] **đọc từ thiết bị** (device id, `VERSION_1`, cỡ hàng đợi) — không hằng.
    pub fn init<B: MmioBus, D: DmaPool>(
        &self,
        bus: &mut B,
        cfg: &Transport,
        dma: &mut D,
    ) -> Result<RngReport, RngError> {
        self.ready.store(false, Ordering::Relaxed);
        let layout = VqLayout::new(QUEUE_SIZE).ok_or(RngError::QueueSize)?;
        // Ring + đệm phải nằm gọn trong **một** trang DMA 4 KiB.
        if RNG_RING_OFF + layout.total_bytes() > RNG_BUF_OFF
            || RNG_BUF_OFF + RNG_BUF_BYTES > DMA_PAGE_BYTES
        {
            return Err(RngError::QueueSize);
        }
        let page: DmaBuf = dma
            .alloc(DMA_PAGE_BYTES, DMA_PAGE_BYTES)
            .ok_or(RngError::NoDevice)?;
        if page.len < DMA_PAGE_BYTES || page.pa & 0xfff != 0 {
            return Err(RngError::NoDevice);
        }
        // Vùng DMA sạch trước khi bàn giao (kể cả khi init gọi lại).
        dma.zero(page);
        let dma_off = page.off;
        let dma_pa = page.pa;
        let common = cfg.common;
        let notify = cfg.notify;
        let notify_mul = cfg.notify_mul;

        // ---- 1) reset (§2.4.2 bước 1)
        bus.write8(common + C_DEVICE_STATUS, 0);
        if bus.read8(common + C_DEVICE_STATUS) != 0 {
            return Err(RngError::NoDevice);
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
        if device_features & (1u64 << F_VERSION_1) == 0 {
            return Err(RngError::Version1);
        }
        // ---- 4) đàm phán: **chỉ** VERSION_1 (rng không có feature riêng cần đàm phán)
        let wanted = 1u64 << F_VERSION_1;
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 0);
        bus.write32(common + C_DRIVER_FEATURE, (wanted & 0xffff_ffff) as u32);
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 1);
        bus.write32(common + C_DRIVER_FEATURE, (wanted >> 32) as u32);
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 0);
        let drv_low = bus.read32(common + C_DRIVER_FEATURE) as u64;
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 1);
        let drv_high = bus.read32(common + C_DRIVER_FEATURE) as u64;
        if ((drv_high << 32) | drv_low) != wanted {
            return Err(RngError::Features);
        }
        // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
        if !features_ok(bus.read8(common + C_DEVICE_STATUS)) {
            return Err(RngError::FeaturesOk);
        }
        // ---- 6) hàng đợi 0 (`requestq`)
        if bus.read16(common + C_NUM_QUEUES) < QUEUES_REQUIRED {
            return Err(RngError::Queues);
        }
        bus.write16(common + C_QUEUE_SELECT, Q_REQUEST);
        let max = bus.read16(common + C_QUEUE_SIZE);
        if max < MIN_QUEUE_SIZE || !max.is_power_of_two() {
            return Err(RngError::QueueSize);
        }
        let size = QUEUE_SIZE.min(max);
        bus.write16(common + C_QUEUE_SIZE, size);
        if bus.read16(common + C_QUEUE_SIZE) != size {
            return Err(RngError::QueueSize);
        }
        let layout = VqLayout::new(size).ok_or(RngError::QueueSize)?;
        let base = dma_pa + RNG_RING_OFF as u64;
        bus.write64(common + C_QUEUE_DESC, base + layout.desc_off as u64);
        bus.write64(common + C_QUEUE_DRIVER, base + layout.avail_off as u64);
        bus.write64(common + C_QUEUE_DEVICE, base + layout.used_off as u64);
        bus.fence();
        bus.write16(common + C_QUEUE_ENABLE, 1);
        if bus.read16(common + C_QUEUE_ENABLE) != 1 {
            return Err(RngError::QueueEnable);
        }
        let notify_off = bus.read16(common + C_QUEUE_NOTIFY_OFF);
        // ---- 7) DRIVER_OK
        bus.write8(
            common + C_DEVICE_STATUS,
            ACK | DRIVER | FEATURES_OK | DRIVER_OK,
        );
        if !status_sequence_ok(bus.read8(common + C_DEVICE_STATUS)) {
            return Err(RngError::DriverOk);
        }

        self.device_id
            .store(u64::from(cfg.device), Ordering::Relaxed);
        self.version1.store(true, Ordering::Relaxed);
        self.queue_size.store(u64::from(size), Ordering::Relaxed);
        self.buf_off
            .store((dma_off + RNG_BUF_OFF) as u64, Ordering::Relaxed);
        self.buf_pa
            .store(dma_pa + RNG_BUF_OFF as u64, Ordering::Relaxed);
        self.ring_off.store(dma_off as u64, Ordering::Relaxed);
        self.notify_addr.store(
            notify + u64::from(notify_off) * u64::from(notify_mul),
            Ordering::Relaxed,
        );
        self.used_seen.store(0, Ordering::Relaxed);
        self.ready.store(true, Ordering::Relaxed);
        Ok(self.report())
    }

    /// Lấy `out.len()` byte entropy **đã qua cổng chất lượng** từ thiết bị.
    ///
    /// Trả số byte **thực nhận** (bằng `out.len()` ở đường thành công). Fail-closed: mọi bất
    /// thường ⇒ `Err` + bộ đếm tương ứng, `out` **không** bị coi là entropy hợp lệ.
    pub fn fill<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        out: &mut [u8],
        timeout: u64,
        clock: &mut C,
    ) -> Result<usize, RngError> {
        if !self.ready.load(Ordering::Relaxed) {
            return Err(RngError::NotReady);
        }
        if out.len() < MIN_REQUEST_BYTES || out.len() > MAX_REQUEST_BYTES {
            return Err(RngError::BadLen);
        }
        let ring_off = self.ring_off.load(Ordering::Relaxed) as usize;
        let buf_off = self.buf_off.load(Ordering::Relaxed) as usize;
        let buf_pa = self.buf_pa.load(Ordering::Relaxed);
        let queue_size = self.queue_size.load(Ordering::Relaxed) as u16;
        let layout = VqLayout::new(queue_size).ok_or(RngError::QueueSize)?;
        let used_base = self.used_seen.load(Ordering::Relaxed) as u16;

        // ---- 1) descriptor 0: đệm entropy device-writable, **một** descriptor mỗi request
        {
            let page = &mut dma.bytes()[ring_off..ring_off + DMA_PAGE_BYTES];
            let mut queue = SplitQueue::new(page, queue_size).ok_or(RngError::QueueSize)?;
            let desc = Desc {
                addr: buf_pa,
                len: out.len() as u32,
                flags: DescFlags::WRITE,
                next: 0,
            };
            if !queue.set_desc(0, desc) {
                return Err(RngError::QueueSize);
            }
            // ---- 2) publish head rồi fence **trước** khi báo thiết bị (§2.7.13)
            queue.push_avail(0);
        }
        bus.fence();
        dma.sync_for_device(buf_off, out.len());
        // ---- 3) doorbell
        let notify_addr = self.notify_addr.load(Ordering::Relaxed);
        bus.notify(notify_addr, Q_REQUEST);
        self.notifies.fetch_add(1, Ordering::Relaxed);

        // ---- 4) poll `used->idx` (một chiều) tới hạn
        let deadline = clock.now().wrapping_add(timeout);
        let used_off = ring_off + layout.used_idx_off;
        let used = loop {
            let current = dma.read_u16(used_off);
            if current != used_base {
                break current;
            }
            if past_deadline(clock.now(), deadline) {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                self.errors.fetch_add(1, Ordering::Relaxed);
                return Err(RngError::Timeout);
            }
        };
        bus.fence();
        dma.sync_for_cpu(buf_off, out.len());

        // ---- 5) `used->elem`: id phải là head 0, len phải trong [MIN, out.len()]
        let slot = used_base & (queue_size - 1);
        let (id, len) = {
            // Đọc thẳng theo `VqLayout` (cùng nguồn với `SplitQueue`) — không chép số offset.
            let at = ring_off + layout.used_ring_off + slot as usize * 8;
            let bytes: &[u8] = &dma.bytes()[at..at + 8];
            (
                u32::from_le_bytes(bytes[0..4].try_into().unwrap_or([0; 4])),
                u32::from_le_bytes(bytes[4..8].try_into().unwrap_or([0; 4])),
            )
        };
        if id != 0 || len as usize > out.len() || (len as usize) < MIN_REQUEST_BYTES {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(RngError::Io);
        }
        let len = len as usize;
        // ---- 6) cổng chất lượng **trước khi** byte được tính là entropy
        let sample = &dma.bytes()[buf_off..buf_off + len];
        let distinct = distinct_bytes(sample);
        let run = longest_run(sample);
        let ok = entropy_ok(sample);
        self.distinct.store(u64::from(distinct), Ordering::Relaxed);
        self.longest_run.store(run as u64, Ordering::Relaxed);
        self.entropy_ok.store(ok, Ordering::Relaxed);
        if !ok {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(RngError::LowEntropy);
        }
        out[..len].copy_from_slice(sample);
        self.used_seen.store(u64::from(used), Ordering::Relaxed);
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(len as u64, Ordering::Relaxed);
        Ok(len)
    }

    /// Ảnh chụp bộ đếm (đọc một lần, in từ dữ liệu — D2-102).
    pub fn report(&self) -> RngReport {
        RngReport {
            device_id: self.device_id.load(Ordering::Relaxed) as u16,
            version1: self.version1.load(Ordering::Relaxed),
            queue_size: self.queue_size.load(Ordering::Relaxed) as u16,
            requests: self.requests.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            distinct: self.distinct.load(Ordering::Relaxed) as u16,
            longest_run: self.longest_run.load(Ordering::Relaxed) as usize,
            entropy_ok: self.entropy_ok.load(Ordering::Relaxed),
            notifies: self.notifies.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}
