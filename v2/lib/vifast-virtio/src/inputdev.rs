// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Driver `virtio-input` (bàn phím)** — phần **thuần** dùng chung 3 arch (KT-337,
//! D2-231/D2-239).
//!
//! # Vì sao tệp này tồn tại
//!
//! Trước KT-337, `kernel/vifast-plat-{riscv64,aarch64}/src/virtio_input.rs` là **hai bản sao** của
//! cùng một driver: status → feature → queue `event` → arm đệm → notify → poll `used` → đọc
//! `virtio_input_config` (tên/devids/EV_BITS). Chúng chỉ khác **cách chạm phần cứng** và **topology**
//! (aarch64 bật thêm queue `status` cho LED, riscv64 thì không). Tệp này giữ toàn bộ quyết định;
//! platform chỉ còn adapter (D2-17).
//!
//! # Bất biến giữ nguyên từ KT-255/KT-279/KT-283
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1` — quyết định ở
//!   [`crate::input::negotiated_features`] (test host).
//! - Arm **nhiều** đệm ngay lúc init ([`EVENT_BUFS`] = 8): QEMU huỷ **cả lô** nếu thiếu đệm.
//! - Thứ tự spec §2.7: ghi descriptor → `avail->ring`/`idx` → **bàn giao DMA** (aarch64 `dc
//!   civac` + `dsb`) → **fence** → notify → poll `used->idx` → đọc `used->elem` → **arm lại**.
//! - `used->id` ngoài queue hoặc `used->len ≠ 8` ⇒ đếm `errors`/`id_mismatch`, **không** trả dữ
//!   liệu rác.
//! - Fail-closed: thiếu VERSION_1 / queue / tên thiết bị ⇒ `Err(stage)`; không PANIC, không treo.
//!
//! `unsafe` **không** có ở đây (`#![forbid(unsafe_code)]` của crate); mọi truy cập thanh ghi/DMA
//! đi qua trait.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub use crate::dma::{DmaBuf, DmaPool};
use crate::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use crate::input::{
    decode_devids, decode_name, event_len, is_shift, key_bit, negotiated_features,
    InputConfigHeader, InputDevIds, InputEvent, CFG_EV_BITS, CFG_HEADER_BYTES, CFG_ID_DEVIDS,
    CFG_ID_NAME, CFG_SELECT_OFF, CFG_SUBSEL_OFF, CFG_U_BYTES, CFG_U_OFF, EVENT_BUFS, EVENT_BYTES,
    EV_KEY, MIN_QUEUE_SIZE, QUEUES_REQUIRED, QUEUE_SIZE, Q_EVENT, Q_STATUS,
};
pub use crate::mmio::MmioBus;
use crate::pcibus::PciReport;
use crate::queue::{Desc, DescFlags, SplitQueue, VqLayout};

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `event` trong trang DMA.
pub const RING_OFF: usize = 0x000;
/// Offset ring `status` (aarch64 bật queue LED) trong trang DMA.
pub const STATUS_RING_OFF: usize = 0x200;
/// Offset vùng đệm sự kiện trong trang DMA.
pub const BUF_OFF: usize = 0x400;
/// Cỡ tối đa của tên thiết bị đọc từ `virtio_input_config`.
pub const NAME_MAX_BYTES: usize = CFG_U_BYTES;
/// Cỡ bitmap `EV_BITS`.
pub const EV_BITS_BYTES: usize = CFG_U_BYTES;

// Bất biến biên dịch: hai ring + vùng đệm phải nằm gọn trong **một** trang DMA 4 KiB.
const _: () = {
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2 trong [2, 256]"),
    };
    assert!(RING_OFF + layout.end <= STATUS_RING_OFF);
    assert!(STATUS_RING_OFF + layout.end <= BUF_OFF);
    assert!(BUF_OFF + EVENT_BUFS * EVENT_BYTES <= DMA_PAGE_BYTES);
};

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

/// Bộ đếm của driver — **ảnh chụp** đọc một lần (D2-102: in từ dữ liệu, không chép số).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Số sự kiện đã nhận và giải mã.
    pub events: u64,
    /// Số sự kiện `EV_SYN`.
    pub syncs: u64,
    /// Số lần phím xuống (`EV_KEY value != 0`).
    pub key_downs: u64,
    /// Số lần phím nhả (`EV_KEY value == 0`).
    pub key_ups: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lỗi driver (`used->id`/`used->len` vô lý, event không giải mã được).
    pub errors: u64,
    /// Số lần poll quá hạn (chỉ tăng ở `poll_event_wait`).
    pub timeouts: u64,
    /// Số lần `used->id` ngoài queue (phải 0).
    pub id_mismatch: u64,
}

/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct InputReport {
    /// Vendor/device (từ PCI config).
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// BAR driver dùng + địa chỉ đã gán + cỡ đo được.
    pub bar_index: u8,
    /// Địa chỉ BAR đã gán.
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
    /// `notify_off_multiplier` đọc từ capability.
    pub notify_mul: u32,
    /// `queue_notify_off` của queue `event`.
    pub notify_off: u16,
    /// Địa chỉ cửa thông báo `event` = notify + off * multiplier.
    pub notify_addr: u64,
    /// `num_queues` thiết bị báo (QEMU: 2 — `event` + `status`).
    pub num_queues: u16,
    /// Cỡ queue tối đa thiết bị cho phép.
    pub queue_max: u16,
    /// Cỡ queue driver đặt.
    pub queue_size: u16,
    /// Cỡ queue `status` driver đặt (0 nếu không bật).
    pub status_queue_size: u16,
    /// `queue_notify_off` của queue `status` (0 nếu không bật).
    pub status_notify_off: u16,
    /// Feature thiết bị (64 bit, đọc hai trang).
    pub device_features: u64,
    /// Feature driver đã ghi (đọc lại).
    pub driver_features: u64,
    /// `device_status` sau DRIVER_OK.
    pub status: u8,
    /// `config_generation` đọc **sau** khi đọc config (chỉ để làm bằng chứng — với input, mỗi lần
    /// ghi `select`/`subsel` **đổi** generation nên **không** dùng để kiểm tính nguyên vẹn).
    pub config_generation: u8,
    /// Tên thiết bị đọc từ `select = ID_NAME` (đã bỏ NUL cuối).
    pub name: [u8; NAME_MAX_BYTES],
    /// Số byte tên có nghĩa.
    pub name_len: u8,
    /// Danh tính thiết bị (`select = ID_DEVIDS`).
    pub devids: InputDevIds,
    /// Bitmap `EV_BITS` với `subsel = EV_KEY`.
    pub ev_bits: [u8; EV_BITS_BYTES],
    /// Số byte bitmap có nghĩa.
    pub ev_bits_len: u8,
    /// `true` ⇔ bitmap `EV_BITS` bật bit `KEY_A`.
    pub key_a_supported: bool,
    /// PA trang DMA.
    pub dma_pa: u64,
    /// Địa chỉ (PA) bảng descriptor.
    pub desc: u64,
    /// PA khối `avail`.
    pub avail: u64,
    /// PA khối `used`.
    pub used: u64,
    /// PA vùng đệm sự kiện.
    pub buf_pa: u64,
    /// Số đệm đã arm lúc init.
    pub armed: u16,
}

impl InputReport {
    /// Tên thiết bị dạng `&str` (rỗng nếu thiết bị không có tên — nhưng `init` đã fail-closed).
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len as usize]).unwrap_or("?")
    }
}

/// Trạng thái driver — mọi trường đổi được là `Atomic*` để ảnh boot khai `static INPUT: Input`
/// mà **không** cần `UnsafeCell`/`unsafe` ở platform.
pub struct Input {
    common: AtomicU64,
    notify: AtomicU64,
    device_cfg: AtomicU64,
    isr: AtomicU64,
    notify_mul: AtomicU64,
    notify_off: AtomicU64,
    queue_size: AtomicU64,
    status_notify_off: AtomicU64,
    status_queue_size: AtomicU64,
    dma_off: AtomicU64,
    dma_pa: AtomicU64,
    last_used: AtomicU64,
    published: AtomicU64,
    ready: AtomicBool,
    events: AtomicU64,
    syncs: AtomicU64,
    key_downs: AtomicU64,
    key_ups: AtomicU64,
    notifies: AtomicU64,
    errors: AtomicU64,
    timeouts: AtomicU64,
    id_mismatch: AtomicU64,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
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
            status_notify_off: AtomicU64::new(0),
            status_queue_size: AtomicU64::new(0),
            dma_off: AtomicU64::new(0),
            dma_pa: AtomicU64::new(0),
            last_used: AtomicU64::new(0),
            published: AtomicU64::new(0),
            ready: AtomicBool::new(false),
            events: AtomicU64::new(0),
            syncs: AtomicU64::new(0),
            key_downs: AtomicU64::new(0),
            key_ups: AtomicU64::new(0),
            notifies: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            id_mismatch: AtomicU64::new(0),
        }
    }

    /// Ảnh chụp bộ đếm.
    pub fn stats(&self) -> Stats {
        Stats {
            events: self.events.load(Ordering::Relaxed),
            syncs: self.syncs.load(Ordering::Relaxed),
            key_downs: self.key_downs.load(Ordering::Relaxed),
            key_ups: self.key_ups.load(Ordering::Relaxed),
            notifies: self.notifies.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            id_mismatch: self.id_mismatch.load(Ordering::Relaxed),
        }
    }

    /// `true` ⇔ driver đã init thành công.
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Địa chỉ ISR (chẩn đoán; chưa dùng ngắt).
    pub fn isr_addr(&self) -> u64 {
        self.isr.load(Ordering::Relaxed)
    }

    /// PA trang DMA đã cấp.
    pub fn dma_pa(&self) -> u64 {
        self.dma_pa.load(Ordering::Relaxed)
    }

    /// Số descriptor đã publish (một chiều) — bằng chứng ring quay vòng.
    pub fn published(&self) -> u16 {
        self.published.load(Ordering::Relaxed) as u16
    }

    /// Cỡ queue `event` driver đã đặt (0 nếu chưa init).
    pub fn queue_size(&self) -> u16 {
        self.queue_size.load(Ordering::Relaxed) as u16
    }

    /// Cỡ queue `status` driver đã đặt (0 nếu không bật).
    pub fn status_queue_size(&self) -> u16 {
        self.status_queue_size.load(Ordering::Relaxed) as u16
    }

    /// `queue_notify_off` của queue `status` (bằng chứng driver đọc **cả hai** queue).
    pub fn status_notify_off(&self) -> u16 {
        self.status_notify_off.load(Ordering::Relaxed) as u16
    }

    /// Đọc `device_status` hiện tại.
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

    /// `true` ⇔ ring `status` đã bật và **không** có descriptor nào được publish (không arm đệm).
    ///
    /// Đây là bất biến của thiết kế: driver chỉ **đọc** phím, nên queue `status` (LED) được bật
    /// cho đúng topology nhưng không bao giờ có đệm — thiết bị không thể ghi vào ring ấy.
    pub fn status_queue_idle<D: DmaPool>(&self, dma: &mut D) -> bool {
        let size = self.status_queue_size();
        if size < MIN_QUEUE_SIZE {
            return false;
        }
        let Some(layout) = VqLayout::new(size) else {
            return false;
        };
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let at = dma_off + STATUS_RING_OFF;
        let mem = &mut dma.bytes()[at..at + layout.total_bytes()];
        match SplitQueue::new(mem, size) {
            Some(queue) => queue.avail_idx() == 0,
            None => false,
        }
    }

    /// Ghi `select`/`subsel` vào device config rồi đọc lại 8 byte đầu.
    ///
    /// Thiết bị **phải** trả đúng cặp `(select, subsel)` đã ghi — nếu không, ta đang đọc một view
    /// khác (QEMU trả toàn 0 khi không có config khớp) ⇒ fail-closed thay vì dùng dữ liệu rác.
    fn select_config<B: MmioBus>(
        bus: &mut B,
        device_cfg: u64,
        select: u8,
        subsel: u8,
    ) -> Result<InputConfigHeader, &'static str> {
        bus.write8(device_cfg + CFG_SELECT_OFF as u64, select);
        bus.write8(device_cfg + CFG_SUBSEL_OFF as u64, subsel);
        let mut raw = [0u8; CFG_HEADER_BYTES];
        bus.read_bytes(device_cfg, &mut raw);
        let header = InputConfigHeader::parse(&raw).ok_or("config")?;
        if header.select != select || header.subsel != subsel {
            return Err("config_select");
        }
        Ok(header)
    }

    /// Đọc `n` byte union (offset 8) của view đang chọn vào `out`; trả số byte đã đọc.
    fn read_config_payload<B: MmioBus>(
        bus: &mut B,
        device_cfg: u64,
        header: &InputConfigHeader,
        out: &mut [u8],
    ) -> usize {
        let count = header.payload_bytes().min(out.len());
        for (index, byte) in out.iter_mut().take(count).enumerate() {
            *byte = bus.read8(device_cfg + CFG_U_OFF as u64 + index as u64);
        }
        count
    }

    /// Khởi tạo driver từ kết quả PCI + vùng DMA đã cấp.
    ///
    /// `with_status_queue` = `true` ⇒ bật thêm queue `status` (1) cho đúng topology của
    /// aarch64/KT-283 (không arm đệm). riscv64 giữ **một** queue như KT-255 để TAP không đổi.
    pub fn init<B: MmioBus, D: DmaPool>(
        &self,
        bus: &mut B,
        dma: &mut D,
        pci: &PciReport,
        with_status_queue: bool,
    ) -> Result<InputReport, &'static str> {
        let layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
        if RING_OFF + layout.total_bytes() > STATUS_RING_OFF
            || STATUS_RING_OFF + layout.total_bytes() > BUF_OFF
            || BUF_OFF + EVENT_BUFS * EVENT_BYTES > DMA_PAGE_BYTES
        {
            return Err("layout");
        }
        let page: DmaBuf = dma
            .alloc(DMA_PAGE_BYTES, DMA_PAGE_BYTES)
            .ok_or("dma_align")?;
        if page.len < DMA_PAGE_BYTES || page.pa & 0xfff != 0 || page.off & 0xfff != 0 {
            return Err("dma_align");
        }
        // Vùng DMA sạch trước khi bàn giao (kể cả khi init gọi lại).
        dma.zero(page);
        let bar = pci.bar_base;
        let caps = pci.caps;
        let common = bar + caps.common.offset as u64;
        let notify = bar + caps.notify.offset as u64;
        let device_cfg = bar + caps.device.offset as u64;
        let isr = bar + caps.isr.offset as u64;
        let notify_mul = caps.notify_off_multiplier;

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
        if device_features & (1u64 << F_VERSION_1) == 0 {
            return Err("version1");
        }
        // ---- 4) đàm phán: **chỉ** VERSION_1 (quyết định ở hàm thuần `negotiated_features`).
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
        // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !features_ok(status) {
            return Err("features_ok");
        }
        // ---- 6) queue `event` = 0 (+ `status` = 1 nếu platform cần topology hai queue)
        let num_queues = bus.read16(common + C_NUM_QUEUES);
        if num_queues < QUEUES_REQUIRED {
            return Err("queues");
        }
        if with_status_queue && num_queues < 2 {
            return Err("queues2");
        }
        bus.write16(common + C_QUEUE_SELECT, Q_EVENT);
        let queue_max = bus.read16(common + C_QUEUE_SIZE);
        if queue_max < MIN_QUEUE_SIZE || !queue_max.is_power_of_two() {
            return Err("queue_size");
        }
        let queue_size = QUEUE_SIZE.min(queue_max);
        bus.write16(common + C_QUEUE_SIZE, queue_size);
        if bus.read16(common + C_QUEUE_SIZE) != queue_size {
            return Err("queue_size");
        }
        let layout = VqLayout::new(queue_size).ok_or("layout")?;
        let base = page.pa + RING_OFF as u64;
        let desc_pa = base + layout.desc_off as u64;
        let avail_pa = base + layout.avail_off as u64;
        let used_pa = base + layout.used_off as u64;
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
        // ---- queue `status` (1): cùng cỡ, ring riêng trong **cùng** trang DMA.
        let mut status_queue_size = 0u16;
        let mut status_notify_off = 0u16;
        if with_status_queue {
            bus.write16(common + C_QUEUE_SELECT, Q_STATUS);
            let status_queue_max = bus.read16(common + C_QUEUE_SIZE);
            if status_queue_max < MIN_QUEUE_SIZE || !status_queue_max.is_power_of_two() {
                return Err("status_queue_size");
            }
            status_queue_size = QUEUE_SIZE.min(status_queue_max);
            bus.write16(common + C_QUEUE_SIZE, status_queue_size);
            if bus.read16(common + C_QUEUE_SIZE) != status_queue_size {
                return Err("status_queue_size");
            }
            let status_layout = VqLayout::new(status_queue_size).ok_or("layout")?;
            let status_base = page.pa + STATUS_RING_OFF as u64;
            bus.write64(
                common + C_QUEUE_DESC,
                status_base + status_layout.desc_off as u64,
            );
            bus.write64(
                common + C_QUEUE_DRIVER,
                status_base + status_layout.avail_off as u64,
            );
            bus.write64(
                common + C_QUEUE_DEVICE,
                status_base + status_layout.used_off as u64,
            );
            bus.fence();
            bus.write16(common + C_QUEUE_ENABLE, 1);
            if bus.read16(common + C_QUEUE_ENABLE) != 1 {
                return Err("status_queue_enable");
            }
            status_notify_off = bus.read16(common + C_QUEUE_NOTIFY_OFF);
        }
        // ---- 7) DRIVER_OK
        bus.write8(
            common + C_DEVICE_STATUS,
            ACK | DRIVER | FEATURES_OK | DRIVER_OK,
        );
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !status_sequence_ok(status) {
            return Err("driver_ok");
        }
        // ---- 8) device config: tên + danh tính + bitmap EV_BITS (đọc **từ thiết bị**)
        let name_header = Self::select_config(bus, device_cfg, CFG_ID_NAME, 0)?;
        let mut name = [0u8; NAME_MAX_BYTES];
        let payload_len = Self::read_config_payload(bus, device_cfg, &name_header, &mut name);
        // `size` của QEMU **gồm** NUL cuối (`sizeof("QEMU Virtio Keyboard")` = 21) ⇒ `name_len`
        // phải là số byte **có nghĩa** sau khi bỏ NUL, không phải `size`.
        let name_len = decode_name(&name[..payload_len]).ok_or("name")?.len();
        let ids_header = Self::select_config(bus, device_cfg, CFG_ID_DEVIDS, 0)?;
        let mut ids_raw = [0u8; 8];
        let ids_len = Self::read_config_payload(bus, device_cfg, &ids_header, &mut ids_raw);
        let devids = decode_devids(&ids_raw[..ids_len]).ok_or("devids")?;
        let ev_header = Self::select_config(bus, device_cfg, CFG_EV_BITS, EV_KEY as u8)?;
        let mut ev_bits = [0u8; EV_BITS_BYTES];
        let ev_bits_len = Self::read_config_payload(bus, device_cfg, &ev_header, &mut ev_bits);
        let key_a_supported = key_bit(&ev_bits[..ev_bits_len], crate::input::KEY_A);
        let config_generation = bus.read8(common + C_CONFIG_GENERATION);
        // ---- 9) lưu trạng thái (khối riêng: không giữ `&mut` qua lời gọi `arm_all`)
        self.common.store(common, Ordering::Relaxed);
        self.notify.store(notify, Ordering::Relaxed);
        self.device_cfg.store(device_cfg, Ordering::Relaxed);
        self.isr.store(isr, Ordering::Relaxed);
        self.notify_mul.store(notify_mul as u64, Ordering::Relaxed);
        self.notify_off.store(notify_off as u64, Ordering::Relaxed);
        self.queue_size.store(queue_size as u64, Ordering::Relaxed);
        self.status_queue_size
            .store(status_queue_size as u64, Ordering::Relaxed);
        self.status_notify_off
            .store(status_notify_off as u64, Ordering::Relaxed);
        self.dma_off.store(page.off as u64, Ordering::Relaxed);
        self.dma_pa.store(page.pa, Ordering::Relaxed);
        self.last_used.store(0, Ordering::Relaxed);
        self.published.store(0, Ordering::Relaxed);
        self.ready.store(true, Ordering::Relaxed);
        // ---- 10) arm **nhiều** đệm ngay: sự kiện host gửi tới phải có đệm trước khi người gọi
        // dùng driver (QEMU huỷ **cả lô** nếu thiếu đệm — xem đầu tệp).
        let armed = self.arm_all(bus, dma)?;
        Ok(InputReport {
            vendor: pci.vendor,
            device: pci.device,
            bar_index: pci.bar_index,
            bar_base: bar,
            bar_size: pci.bar_size,
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
            status_queue_size,
            status_notify_off,
            device_features,
            driver_features,
            status,
            config_generation,
            name,
            name_len: name_len as u8,
            devids,
            ev_bits,
            ev_bits_len: ev_bits_len as u8,
            key_a_supported,
            dma_pa: page.pa,
            desc: desc_pa,
            avail: avail_pa,
            used: used_pa,
            buf_pa: page.pa + BUF_OFF as u64,
            armed,
        })
    }

    /// Bọc ring của queue `event` thành split queue với cỡ **thật** của nó.
    fn queue_with<'a, D: DmaPool>(
        &self,
        dma: &'a mut D,
        size: u16,
    ) -> Result<SplitQueue<'a>, crate::input::InputError> {
        if size < MIN_QUEUE_SIZE {
            return Err(crate::input::InputError::NotReady);
        }
        let layout = VqLayout::new(size).ok_or(crate::input::InputError::NotReady)?;
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let at = dma_off + RING_OFF;
        SplitQueue::new(&mut dma.bytes()[at..at + layout.total_bytes()], size)
            .ok_or(crate::input::InputError::NotReady)
    }

    /// Bàn giao vùng DMA cho thiết bị rồi notify queue `event`.
    ///
    /// Thứ tự là **dữ liệu**: `sync_for_device` (aarch64 `dc civac` + `dsb`) → `fence` → notify.
    /// Bỏ `sync_for_device` ⇒ test host `input_submit_syncs_for_device_before_notify` ĐỎ.
    fn handoff_and_notify<B: MmioBus, D: DmaPool>(&self, bus: &mut B, dma: &mut D) {
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        dma.sync_for_device(dma_off, DMA_PAGE_BYTES);
        bus.fence();
        let notify = self.notify.load(Ordering::Relaxed);
        let notify_off = self.notify_off.load(Ordering::Relaxed);
        let notify_mul = self.notify_mul.load(Ordering::Relaxed);
        bus.notify(notify + notify_off * notify_mul, Q_EVENT);
        self.notifies.fetch_add(1, Ordering::Relaxed);
    }

    /// Đăng descriptor `index` (đệm 8 byte device-writable) và publish — gọi lúc init và sau mỗi
    /// sự kiện nhận.
    fn arm_one<B: MmioBus, D: DmaPool>(&self, bus: &mut B, dma: &mut D, index: u16) -> bool {
        let size = self.queue_size.load(Ordering::Relaxed) as u16;
        if size < MIN_QUEUE_SIZE || index >= size {
            return false;
        }
        let dma_pa = self.dma_pa.load(Ordering::Relaxed);
        {
            let Ok(mut queue) = self.queue_with(dma, size) else {
                return false;
            };
            if !queue.set_desc(
                index,
                Desc {
                    addr: dma_pa + BUF_OFF as u64 + index as u64 * EVENT_BYTES as u64,
                    len: EVENT_BYTES as u32,
                    flags: DescFlags::WRITE,
                    next: 0,
                },
            ) {
                return false;
            }
            queue.push_avail(index);
        }
        self.handoff_and_notify(bus, dma);
        self.published.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Arm [`EVENT_BUFS`] đệm đầu tiên rồi notify **một** lần (init).
    fn arm_all<B: MmioBus, D: DmaPool>(
        &self,
        bus: &mut B,
        dma: &mut D,
    ) -> Result<u16, &'static str> {
        let size = self.queue_size.load(Ordering::Relaxed) as u16;
        if size < EVENT_BUFS as u16 {
            return Err("arm");
        }
        let dma_pa = self.dma_pa.load(Ordering::Relaxed);
        {
            let mut queue = self.queue_with(dma, size).map_err(|_| "arm")?;
            for index in 0..EVENT_BUFS as u16 {
                if !queue.set_desc(
                    index,
                    Desc {
                        addr: dma_pa + BUF_OFF as u64 + index as u64 * EVENT_BYTES as u64,
                        len: EVENT_BYTES as u32,
                        flags: DescFlags::WRITE,
                        next: 0,
                    },
                ) {
                    return Err("arm");
                }
                queue.push_avail(index);
            }
        }
        self.handoff_and_notify(bus, dma);
        self.published.store(EVENT_BUFS as u64, Ordering::Relaxed);
        Ok(EVENT_BUFS as u16)
    }

    /// Nhận **một** sự kiện nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có sự kiện.
    ///
    /// `used->len` khác 8 hoặc `used->id` ngoài queue ⇒ đếm `errors`/`id_mismatch`, **không** trả
    /// dữ liệu rác; descriptor vẫn được arm lại để lần sau nhận được.
    pub fn poll_event<B: MmioBus, D: DmaPool, C: crate::blkdev::Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        _clock: &mut C,
    ) -> Option<InputEvent> {
        if !self.ready.load(Ordering::Relaxed) {
            return None;
        }
        let size = self.queue_size.load(Ordering::Relaxed) as u16;
        if size < MIN_QUEUE_SIZE {
            return None;
        }
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let layout = VqLayout::new(size)?;
        let used_idx_off = dma_off + RING_OFF + layout.used_idx_off;
        // **KT-277**: bỏ dòng chứa `used->idx` trước mỗi lần đọc (aarch64 `dc ivac`) — thiết bị
        // ghi qua DMA không đi qua cache CPU.
        dma.sync_for_cpu(used_idx_off, 2);
        let idx = dma.read_u16(used_idx_off);
        let last = self.last_used.load(Ordering::Relaxed) as u16;
        if idx == last {
            return None;
        }
        // Thiết bị đã ghi xong — bỏ cache **trước** khi đọc `used->elem` và đệm sự kiện.
        dma.sync_for_cpu(dma_off, DMA_PAGE_BYTES);
        bus.fence();
        let (id, len) = {
            let Ok(queue) = self.queue_with(dma, size) else {
                return None;
            };
            queue.used_elem(last & (size - 1))
        };
        self.last_used
            .store(last.wrapping_add(1) as u64, Ordering::Relaxed);
        if id as u16 >= size {
            self.id_mismatch.fetch_add(1, Ordering::Relaxed);
            self.errors.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let index = id as u16;
        let event = match event_len(len) {
            Ok(count) => {
                let start = dma_off + BUF_OFF + index as usize * EVENT_BYTES;
                let mut raw = [0u8; EVENT_BYTES];
                raw.copy_from_slice(&dma.bytes()[start..start + count]);
                InputEvent::parse(&raw)
            }
            Err(_) => None,
        };
        // Arm lại **trước** khi trả: cửa sổ không có đệm là nhỏ nhất (QEMU cần đệm cho **mọi**
        // sự kiện của lô kế tiếp).
        if !self.arm_one(bus, dma, index) {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
        let event = match event {
            Some(event) => event,
            None => {
                self.errors.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        self.events.fetch_add(1, Ordering::Relaxed);
        if event.is_syn() {
            self.syncs.fetch_add(1, Ordering::Relaxed);
        } else if event.is_key_down() {
            self.key_downs.fetch_add(1, Ordering::Relaxed);
        } else if event.is_key_up() {
            self.key_ups.fetch_add(1, Ordering::Relaxed);
        }
        Some(event)
    }

    /// Nhận **một** sự kiện, chờ tối đa `timeout_ticks` tick của `clock`. `None` = hết hạn/chưa có.
    pub fn poll_event_wait<B: MmioBus, D: DmaPool, C: crate::blkdev::Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        timeout_ticks: u64,
    ) -> Option<InputEvent> {
        let deadline = clock.now().wrapping_add(timeout_ticks);
        loop {
            if let Some(event) = self.poll_event(bus, dma, clock) {
                return Some(event);
            }
            if clock.now() > deadline {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            core::hint::spin_loop();
        }
    }
}

/// Tên phím (chẩn đoán) — bằng chứng `# input: key …` dùng chuỗi này thay vì in số trần.
pub fn key_name(code: u16) -> &'static str {
    match code {
        crate::input::KEY_A => "a",
        crate::input::KEY_ENTER => "enter",
        crate::input::KEY_SPACE => "space",
        crate::input::KEY_LEFTSHIFT | crate::input::KEY_RIGHTSHIFT => "shift",
        _ => "?",
    }
}

/// `true` ⇔ `code` là phím Shift.
pub const fn shift_key(code: u16) -> bool {
    is_shift(code)
}
