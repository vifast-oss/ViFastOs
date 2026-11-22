// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-net** dùng chung 3 kiến trúc — trên [`MmioBus`] + [`DmaPool`] + [`Clock`]
//! (KT-335, D2-231/D2-239).
//!
//! # Vì sao tệp này tồn tại
//!
//! Ba bản `vifast-plat-{riscv64,aarch64,x86_64}/src/virtio_net.rs` trước KT-335 là **ba bản sao**
//! của cùng một chuỗi khởi tạo/IO virtio-net: `reset → ACK → DRIVER → đọc/đàm phán feature →
//! FEATURES_OK → cấu hình hai queue → DRIVER_OK → đọc `virtio_net_config` → dựng descriptor/
//! avail/used → notify → poll used`. Đo được (`wc -l`): 1.338 + 1.395 + 1.271 = **4.004 dòng**,
//! khác nhau **chỉ** ở:
//!
//! 1. **cửa sổ MMIO** — con trỏ thô `read_volatile` (riscv/x86) vs `asm::mmio_*` + `dsb sy`
//!    (aarch64) vs `fence(SeqCst)` (x86);
//! 2. **cách lấy PA của trang DMA** — identity map (riscv/x86, `VA == PA`) vs PA truyền vào
//!    (aarch64, trang DMA trong `.bss` dưới `KERNEL_HIGH_BASE` — KT-273/KT-338a);
//! 3. **nguồn thời gian** — timebase SBI (riscv, tick) vs `CNTPCT_EL0` (aarch64, µs) vs PIT
//!    (x86, µs);
//! 4. **bộ trường `NetReport`** — mỗi bản một tập con, không bản nào là superset.
//!
//! KT-335 gộp phần **quyết định** vào đây (D2-05: test host được, `#![forbid(unsafe_code)]`) và
//! để mỗi nền tảng giữ đúng phần **chạm phần cứng** (D2-17): ba impl `MmioBus`/`DmaPool`/`Clock`
//! ~120 dòng mỗi bản (xem `vifast-plat-*/src/virtio_net.rs`).
//!
//! # Bất biến (giữ **nguyên** từ KT-245/KT-274/KT-275)
//!
//! - Feature đàm phán: `VIRTIO_F_VERSION_1` (bắt buộc) + `VIRTIO_NET_F_MAC` **nếu** thiết bị có.
//!   **Không** `MRG_RXBUF`, **không** `STATUS` — quyết định ở [`crate::net::negotiated_features`]
//!   (một chỗ, test host; D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **fence** → ghi `avail->idx` →
//!   **fence** → notify (MMIO) → poll `used->idx` → **fence** → đọc `used->elem`.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - Một frame một lúc mỗi hướng (queue depth 1 thực tế); đủ cho DHCP/ARP/ICMP/DNS/TCP.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); frame sai độ dài ⇒
//!   [`NetError::BadFrame`] **trước** khi chạm ring; `used->len` vô lý ⇒ [`NetError::Io`] + đếm.
//! - `rx` arm **trước** (một descriptor device-writable [`crate::net::RX_BUF_BYTES`]) để frame
//!   host gửi tới không bị rơi; `recv_frame_wait` xong arm lại ngay.
//!
//! # Trạng thái bằng `Atomic*`, không `UnsafeCell`
//!
//! Như [`crate::blkdev::Blk`]: mọi trường đổi được là `Atomic*` để nền tảng khai
//! `static NET: Net = Net::new()` mà **không** cần `UnsafeCell`/`unsafe` (D2-17). Ảnh boot chạy
//! **một** hart trên đường driver và driver tự tuần tự hoá (một frame một lúc mỗi hướng) ⇒
//! `load`/`store(Relaxed)` là đủ — đúng khuôn `virtio_blk`/`virtio_gpu` của KT-334/KT-337.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::blkdev::Transport;
use crate::dma::{DmaBuf, DmaPool};
use crate::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use crate::mmio::MmioBus;
use crate::net::{
    decode_config, frame_len, frame_ok, hdr_bytes, mac_negotiated, negotiated_features, NetConfig,
    NetError, NetHdr, MIN_QUEUE_SIZE, NET_CONFIG_BYTES, QUEUES_REQUIRED, QUEUE_SIZE, Q_RX, Q_TX,
    RX_BUF_BYTES, TX_BUF_BYTES,
};
use crate::queue::{Desc, DescFlags, SplitQueue, VqLayout};

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `rx` trong trang DMA.
pub const RX_RING_OFF: usize = 0x000;
/// Offset ring `tx` trong trang DMA.
pub const TX_RING_OFF: usize = 0x200;
/// Offset đệm `rx` (device-writable: header + frame).
pub const RX_BUF_OFF: usize = 0x400;
/// Offset đệm `tx` (driver-writable: header + frame).
pub const TX_BUF_OFF: usize = 0xA00;

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

/// Đồng hồ đơn điệu **một hàm** — đơn vị do nền tảng quy ước (tick timebase, µs `CNTPCT`, µs PIT)
/// và **phải** cùng đơn vị với mọi `timeout` truyền vào [`Net::send_frame`]/[`Net::recv_frame_wait`].
///
/// Vì sao không dùng chung `blkdev::Clock`: đồng hồ của blk mang thêm `deadline()`/`spin_limit()`
/// cho mô hình trần **tuyệt đối** của nó, còn virtio-net nhận **timeout tương đối** ở từng lời
/// gọi (`recv_frame_wait(out, timeout)` là API đã có của cả ba nền tảng) — hai hợp đồng khác nhau
/// thì để hai trait khác nhau, không gộp cho đẹp.
pub trait Clock {
    /// Thời điểm hiện tại.
    fn now(&mut self) -> u64;
}

/// `true` ⇔ `now` **đã** qua `deadline`; an toàn với wrap của bộ đếm (`rdtime`/`CNTPCT`).
pub const fn past_deadline(now: u64, deadline: u64) -> bool {
    now.wrapping_sub(deadline) < u64::MAX / 2
}

/// Ảnh chụp bộ đếm để in evidence (đọc một lần, không tách rời từng số).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Số frame đã gửi thành công.
    pub tx_frames: u64,
    /// Số byte frame đã gửi (không kể header virtio).
    pub tx_bytes: u64,
    /// Số frame đã nhận.
    pub rx_frames: u64,
    /// Số byte frame đã nhận (không kể header virtio).
    pub rx_bytes: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lỗi driver (frame sai, `used->len` vô lý, thiết bị trả lỗi).
    pub errors: u64,
    /// Số lần poll quá hạn (phải 0 ở đường thành công).
    pub timeouts: u64,
    /// Số lần `used->id` lệch head (phải 0).
    pub id_mismatch: u64,
}

/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
///
/// Đây là **hợp** của ba bộ trường cũ (riscv64 1.338 dòng, aarch64, x86_64) — không bản nào là
/// superset, nên gộp phải giữ **tất cả** trường mà ảnh đang in (xem
/// `vifast-kbin-*/src/{net,ktest_net}.rs`).
#[derive(Clone, Copy, Debug)]
pub struct NetReport {
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
    /// BAR driver dùng.
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
    /// `queue_notify_off` của `rx`.
    pub notify_off_rx: u16,
    /// `queue_notify_off` của `tx`.
    pub notify_off_tx: u16,
    /// Địa chỉ cửa thông báo `rx` = notify + off * multiplier.
    pub notify_addr_rx: u64,
    /// Địa chỉ cửa thông báo `tx`.
    pub notify_addr_tx: u64,
    /// `num_queues` thiết bị báo.
    pub num_queues: u16,
    /// Cỡ queue tối đa thiết bị cho phép (`rx`).
    pub queue_max_rx: u16,
    /// Cỡ queue tối đa thiết bị cho phép (`tx`).
    pub queue_max_tx: u16,
    /// Cỡ queue driver đặt (`rx`).
    pub queue_size_rx: u16,
    /// Cỡ queue driver đặt (`tx`).
    pub queue_size_tx: u16,
    /// Feature thiết bị (64 bit, đọc hai trang).
    pub device_features: u64,
    /// Feature driver đã ghi (đọc lại).
    pub driver_features: u64,
    /// `true` ⇔ đã đàm phán `VIRTIO_NET_F_MAC` (được tin `mac`).
    pub mac_ok: bool,
    /// `device_status` sau DRIVER_OK.
    pub status: u8,
    /// `config_generation` đọc **sau** khi đọc config (spec: đọc lại để phát hiện đổi).
    pub config_generation: u8,
    /// `virtio_net_config` đã giải mã.
    pub config: NetConfig,
    /// PA trang DMA driver dùng.
    pub dma_pa: u64,
    /// PA bảng descriptor `rx`.
    pub desc_rx: u64,
    /// PA khối `avail` `rx`.
    pub avail_rx: u64,
    /// PA khối `used` `rx`.
    pub used_rx: u64,
    /// PA bảng descriptor `tx`.
    pub desc_tx: u64,
    /// PA khối `avail` `tx`.
    pub avail_tx: u64,
    /// PA khối `used` `tx`.
    pub used_tx: u64,
    /// PA đệm `rx` (header + frame).
    pub rx_buf_pa: u64,
    /// PA đệm `tx` (header + frame).
    pub tx_buf_pa: u64,
}

/// Trạng thái driver — mọi trường đổi được đều là `Atomic*` (xem đầu tệp).
pub struct Net {
    common: AtomicU64,
    notify: AtomicU64,
    device_cfg: AtomicU64,
    isr: AtomicU64,
    notify_mul: AtomicU64,
    notify_off_rx: AtomicU64,
    notify_off_tx: AtomicU64,
    queue_size_rx: AtomicU64,
    queue_size_tx: AtomicU64,
    dma_off: AtomicU64,
    dma_pa: AtomicU64,
    mac: AtomicU64,
    rx_last_used: AtomicU64,
    tx_last_used: AtomicU64,
    ready: AtomicBool,
    tx_frames: AtomicU64,
    tx_bytes: AtomicU64,
    rx_frames: AtomicU64,
    rx_bytes: AtomicU64,
    notifies: AtomicU64,
    errors: AtomicU64,
    timeouts: AtomicU64,
    id_mismatch: AtomicU64,
}

impl Default for Net {
    fn default() -> Self {
        Self::new()
    }
}

impl Net {
    /// Trạng thái rỗng (chưa init).
    pub const fn new() -> Self {
        Self {
            common: AtomicU64::new(0),
            notify: AtomicU64::new(0),
            device_cfg: AtomicU64::new(0),
            isr: AtomicU64::new(0),
            notify_mul: AtomicU64::new(0),
            notify_off_rx: AtomicU64::new(0),
            notify_off_tx: AtomicU64::new(0),
            queue_size_rx: AtomicU64::new(0),
            queue_size_tx: AtomicU64::new(0),
            dma_off: AtomicU64::new(0),
            dma_pa: AtomicU64::new(0),
            mac: AtomicU64::new(0),
            rx_last_used: AtomicU64::new(0),
            tx_last_used: AtomicU64::new(0),
            ready: AtomicBool::new(false),
            tx_frames: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            rx_frames: AtomicU64::new(0),
            rx_bytes: AtomicU64::new(0),
            notifies: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            id_mismatch: AtomicU64::new(0),
        }
    }

    /// Ảnh chụp bộ đếm.
    pub fn stats(&self) -> Stats {
        Stats {
            tx_frames: self.tx_frames.load(Ordering::Relaxed),
            tx_bytes: self.tx_bytes.load(Ordering::Relaxed),
            rx_frames: self.rx_frames.load(Ordering::Relaxed),
            rx_bytes: self.rx_bytes.load(Ordering::Relaxed),
            notifies: self.notifies.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            id_mismatch: self.id_mismatch.load(Ordering::Relaxed),
        }
    }

    /// MAC đọc từ thiết bị (toàn 0 nếu chưa init).
    pub fn mac(&self) -> [u8; 6] {
        unpack_mac(self.mac.load(Ordering::Relaxed))
    }

    /// `true` ⇔ driver đã init thành công.
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Địa chỉ ISR (chẩn đoán; K6 chưa dùng ngắt).
    pub fn isr_addr(&self) -> u64 {
        self.isr.load(Ordering::Relaxed)
    }

    /// PA trang DMA driver dùng (0 nếu chưa init).
    pub fn dma_pa(&self) -> u64 {
        self.dma_pa.load(Ordering::Relaxed)
    }

    /// Đọc thanh ghi `device_status` hiện tại.
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

    /// Khởi tạo driver từ transport PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
    /// đo được — không PANIC im lặng. Giữ **đúng** mọi tên chặng của ba bản cũ.
    pub fn init<B: MmioBus, D: DmaPool>(
        &self,
        bus: &mut B,
        cfg: &Transport,
        dma: &mut D,
    ) -> Result<NetReport, &'static str> {
        let rx_layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
        let tx_layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
        // Hai ring + hai đệm phải nằm gọn trong **một** trang DMA 4 KiB.
        if RX_RING_OFF + rx_layout.total_bytes() > TX_RING_OFF
            || TX_RING_OFF + tx_layout.total_bytes() > RX_BUF_OFF
            || RX_BUF_OFF + RX_BUF_BYTES > TX_BUF_OFF
            || TX_BUF_OFF + TX_BUF_BYTES > DMA_PAGE_BYTES
        {
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
        let dma_off = page.off;
        let dma_pa = page.pa;
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
        if device_features & (1u64 << F_VERSION_1) == 0 {
            return Err("version1");
        }
        // ---- 4) đàm phán: VERSION_1 bắt buộc + MAC nếu thiết bị có (quyết định ở hàm thuần).
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
        let mac_ok = mac_negotiated(driver_features);
        // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !features_ok(status) {
            return Err("features_ok");
        }
        // ---- 6) hai queue: rx = 0, tx = 1
        let num_queues = bus.read16(common + C_NUM_QUEUES);
        if num_queues < QUEUES_REQUIRED {
            return Err("queues");
        }
        let ring_off = [RX_RING_OFF, TX_RING_OFF];
        let mut queue_max = [0u16; 2];
        let mut queue_size = [0u16; 2];
        let mut notify_off = [0u16; 2];
        for (slot, index) in [Q_RX, Q_TX].iter().enumerate() {
            bus.write16(common + C_QUEUE_SELECT, *index);
            let max = bus.read16(common + C_QUEUE_SIZE);
            // Chain của driver dùng **một** descriptor mỗi frame ⇒ queue chỉ cần ≥ 2 (luỹ thừa 2).
            if max < MIN_QUEUE_SIZE || !max.is_power_of_two() {
                return Err("queue_size");
            }
            let size = QUEUE_SIZE.min(max);
            bus.write16(common + C_QUEUE_SIZE, size);
            if bus.read16(common + C_QUEUE_SIZE) != size {
                return Err("queue_size");
            }
            let layout = VqLayout::new(size).ok_or("layout")?;
            let base = dma_pa + ring_off[slot] as u64;
            bus.write64(common + C_QUEUE_DESC, base + layout.desc_off as u64);
            bus.write64(common + C_QUEUE_DRIVER, base + layout.avail_off as u64);
            bus.write64(common + C_QUEUE_DEVICE, base + layout.used_off as u64);
            bus.fence();
            bus.write16(common + C_QUEUE_ENABLE, 1);
            if bus.read16(common + C_QUEUE_ENABLE) != 1 {
                return Err("queue_enable");
            }
            queue_max[slot] = max;
            queue_size[slot] = size;
            notify_off[slot] = bus.read16(common + C_QUEUE_NOTIFY_OFF);
        }
        let notify_addr_rx = notify + notify_off[0] as u64 * notify_mul as u64;
        let notify_addr_tx = notify + notify_off[1] as u64 * notify_mul as u64;
        // ---- 7) DRIVER_OK
        bus.write8(
            common + C_DEVICE_STATUS,
            ACK | DRIVER | FEATURES_OK | DRIVER_OK,
        );
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !status_sequence_ok(status) {
            return Err("driver_ok");
        }
        // ---- 8) device config: mac/status/max_virtqueue_pairs/mtu đọc từ thiết bị
        let generation_before = bus.read8(common + C_CONFIG_GENERATION);
        let mut raw = [0u8; NET_CONFIG_BYTES];
        for (index, byte) in raw.iter_mut().enumerate() {
            *byte = bus.read8(device_cfg + index as u64);
        }
        let config_generation = bus.read8(common + C_CONFIG_GENERATION);
        // Spec §2.4.4: `config_generation` đổi trong lúc đọc config ⇒ dữ liệu có thể **xé** ⇒
        // fail-closed (không dùng MAC đọc được nửa vời).
        if config_generation != generation_before {
            return Err("config_generation");
        }
        let config = decode_config(&raw).ok_or("config")?;
        // MAC chỉ được tin khi đã đàm phán `VIRTIO_NET_F_MAC`; giá trị phải **hợp lệ** (không toàn
        // 0, không broadcast) — thiết bị thật không phát hai giá trị đó.
        if !mac_ok || !config.mac_hop_le() {
            return Err("mac");
        }
        // ---- 9) lưu trạng thái (khối riêng: không giữ trạng thái qua lời gọi `arm_rx`)
        self.common.store(common, Ordering::Relaxed);
        self.notify.store(notify, Ordering::Relaxed);
        self.device_cfg.store(device_cfg, Ordering::Relaxed);
        self.isr.store(isr, Ordering::Relaxed);
        self.notify_mul.store(notify_mul as u64, Ordering::Relaxed);
        self.notify_off_rx
            .store(notify_off[0] as u64, Ordering::Relaxed);
        self.notify_off_tx
            .store(notify_off[1] as u64, Ordering::Relaxed);
        self.queue_size_rx
            .store(queue_size[0] as u64, Ordering::Relaxed);
        self.queue_size_tx
            .store(queue_size[1] as u64, Ordering::Relaxed);
        self.dma_off.store(dma_off as u64, Ordering::Relaxed);
        self.dma_pa.store(dma_pa, Ordering::Relaxed);
        self.mac.store(pack_mac(config.mac), Ordering::Relaxed);
        self.rx_last_used.store(0, Ordering::Relaxed);
        self.tx_last_used.store(0, Ordering::Relaxed);
        self.ready.store(true, Ordering::Relaxed);
        // ---- 10) arm `rx` ngay: frame host gửi tới phải có đệm trước khi người gọi dùng driver.
        self.arm_rx(bus, dma)?;
        Ok(NetReport {
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
            notify_off_rx: notify_off[0],
            notify_off_tx: notify_off[1],
            notify_addr_rx,
            notify_addr_tx,
            num_queues,
            queue_max_rx: queue_max[0],
            queue_max_tx: queue_max[1],
            queue_size_rx: queue_size[0],
            queue_size_tx: queue_size[1],
            device_features,
            driver_features,
            mac_ok,
            status,
            config_generation,
            config,
            dma_pa,
            desc_rx: dma_pa + RX_RING_OFF as u64 + rx_layout.desc_off as u64,
            avail_rx: dma_pa + RX_RING_OFF as u64 + rx_layout.avail_off as u64,
            used_rx: dma_pa + RX_RING_OFF as u64 + rx_layout.used_off as u64,
            desc_tx: dma_pa + TX_RING_OFF as u64 + tx_layout.desc_off as u64,
            avail_tx: dma_pa + TX_RING_OFF as u64 + tx_layout.avail_off as u64,
            used_tx: dma_pa + TX_RING_OFF as u64 + tx_layout.used_off as u64,
            rx_buf_pa: dma_pa + RX_BUF_OFF as u64,
            tx_buf_pa: dma_pa + TX_BUF_OFF as u64,
        })
    }

    /// Bọc ring của queue `slot` (0 = rx, 1 = tx) thành split queue với cỡ **thật** của nó.
    fn queue_with<'a>(
        &self,
        dma: &'a mut dyn DmaPool,
        slot: usize,
        size: u16,
    ) -> Result<SplitQueue<'a>, NetError> {
        if size < MIN_QUEUE_SIZE {
            return Err(NetError::NotReady);
        }
        let layout = VqLayout::new(size).ok_or(NetError::NotReady)?;
        let off = if slot == 0 { RX_RING_OFF } else { TX_RING_OFF };
        let at = self.dma_off.load(Ordering::Relaxed) as usize + off;
        SplitQueue::new(&mut dma.bytes()[at..at + layout.total_bytes()], size)
            .ok_or(NetError::NotReady)
    }

    /// Đăng một descriptor device-writable cho `rx` và publish — gọi lúc init và sau mỗi frame nhận.
    fn arm_rx<B: MmioBus, D: DmaPool>(&self, bus: &mut B, dma: &mut D) -> Result<(), &'static str> {
        let size = self.queue_size_rx.load(Ordering::Relaxed) as u16;
        if size < MIN_QUEUE_SIZE {
            return Err("arm");
        }
        let dma_pa = self.dma_pa.load(Ordering::Relaxed);
        {
            let mut q = self.queue_with(dma, 0, size).map_err(|_| "arm")?;
            if !q.set_desc(
                0,
                Desc {
                    addr: dma_pa + RX_BUF_OFF as u64,
                    len: RX_BUF_BYTES as u32,
                    flags: DescFlags::WRITE,
                    next: 0,
                },
            ) {
                return Err("arm");
            }
            q.push_avail(0);
        }
        // Bàn giao đệm cho thiết bị **trước** khi notify (aarch64: bảo trì cache ở hook này).
        dma.sync_for_device(
            self.dma_off.load(Ordering::Relaxed) as usize + RX_BUF_OFF,
            RX_BUF_BYTES,
        );
        bus.fence();
        bus.notify(
            self.notify_addr_rx(),
            self.notify_off_rx.load(Ordering::Relaxed) as u16,
        );
        self.notifies.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Địa chỉ cửa thông báo `rx`.
    fn notify_addr_rx(&self) -> u64 {
        self.notify.load(Ordering::Relaxed)
            + self.notify_off_rx.load(Ordering::Relaxed) * self.notify_mul.load(Ordering::Relaxed)
    }

    /// Địa chỉ cửa thông báo `tx`.
    fn notify_addr_tx(&self) -> u64 {
        self.notify.load(Ordering::Relaxed)
            + self.notify_off_tx.load(Ordering::Relaxed) * self.notify_mul.load(Ordering::Relaxed)
    }

    /// Gửi **một** frame Ethernet (không kể header virtio). `Err` ⇒ không có gì được gửi hoặc
    /// thiết bị không hoàn tất trong `timeout` (cùng đơn vị [`Clock`]).
    pub fn send_frame<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        frame: &[u8],
        timeout: u64,
    ) -> Result<(), NetError> {
        if !self.ready() {
            return Err(NetError::NotReady);
        }
        // Kiểm **trước** khi chạm ring: frame sai độ dài không được đưa vào descriptor.
        if !frame_ok(frame) {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(NetError::BadFrame);
        }
        let size = self.queue_size_tx.load(Ordering::Relaxed) as u16;
        if size < MIN_QUEUE_SIZE {
            return Err(NetError::NotReady);
        }
        let hdr_len = hdr_bytes(true);
        let total = hdr_len + frame.len();
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let dma_pa = self.dma_pa.load(Ordering::Relaxed);
        {
            let bytes = dma.bytes();
            // Header rỗng: không checksum offload, không GSO (driver không đàm phán feature nào
            // của hai đường đó).
            bytes[dma_off + TX_BUF_OFF..dma_off + TX_BUF_OFF + hdr_len]
                .copy_from_slice(&NetHdr::zero().encode(true)[..hdr_len]);
            bytes[dma_off + TX_BUF_OFF + hdr_len..dma_off + TX_BUF_OFF + total]
                .copy_from_slice(frame);
        }
        dma.sync_for_device(dma_off + TX_BUF_OFF, total);
        {
            let mut q = self.queue_with(dma, 1, size)?;
            if !q.set_desc(
                0,
                Desc {
                    addr: dma_pa + TX_BUF_OFF as u64,
                    len: total as u32,
                    // Device-readable: **không** cờ WRITE.
                    flags: DescFlags::NONE,
                    next: 0,
                },
            ) {
                return Err(NetError::NotReady);
            }
            q.push_avail(0);
        }
        bus.fence();
        bus.notify(
            self.notify_addr_tx(),
            self.notify_off_tx.load(Ordering::Relaxed) as u16,
        );
        self.notifies.fetch_add(1, Ordering::Relaxed);

        // ---- poll used của `tx` (một chiều)
        let layout = VqLayout::new(size).ok_or(NetError::NotReady)?;
        let used_idx_off = dma_off + TX_RING_OFF + layout.used_idx_off;
        let deadline = clock.now().wrapping_add(timeout);
        loop {
            let idx = dma.read_u16(used_idx_off);
            let last = self.tx_last_used.load(Ordering::Relaxed) as u16;
            if idx != last {
                bus.fence();
                let element = self.queue_with(dma, 1, size)?.used_elem(last & (size - 1));
                self.tx_last_used
                    .store(last.wrapping_add(1) as u64, Ordering::Relaxed);
                if element.0 != 0 {
                    self.id_mismatch.fetch_add(1, Ordering::Relaxed);
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    return Err(NetError::Io);
                }
                // `used->len` của `tx` **không** mang nghĩa "đã nhận bao nhiêu byte" (không có
                // đệm device-writable) — chỉ kiểm nó không vượt số byte đã đưa.
                if element.1 > total as u32 {
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    return Err(NetError::Io);
                }
                self.tx_frames.fetch_add(1, Ordering::Relaxed);
                self.tx_bytes
                    .fetch_add(frame.len() as u64, Ordering::Relaxed);
                return Ok(());
            }
            if past_deadline(clock.now(), deadline) {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                self.errors.fetch_add(1, Ordering::Relaxed);
                return Err(NetError::Timeout);
            }
            core::hint::spin_loop();
        }
    }

    /// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có frame.
    pub fn recv_frame<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        out: &mut [u8],
    ) -> Option<usize> {
        self.recv_frame_wait(bus, dma, clock, out, 0)
    }

    /// Nhận **một** frame, chờ tối đa `timeout` (cùng đơn vị [`Clock`]). `None` = hết hạn / chưa có.
    ///
    /// `used->len` vô lý (nhỏ hơn header, lớn hơn đệm) ⇒ đếm `errors`, **không** trả dữ liệu rác;
    /// descriptor được arm lại để lần sau vẫn nhận được.
    pub fn recv_frame_wait<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        out: &mut [u8],
        timeout: u64,
    ) -> Option<usize> {
        if !self.ready() {
            return None;
        }
        let size = self.queue_size_rx.load(Ordering::Relaxed) as u16;
        if size < MIN_QUEUE_SIZE {
            return None;
        }
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let layout = VqLayout::new(size)?;
        let used_idx_off = dma_off + RX_RING_OFF + layout.used_idx_off;
        let deadline = clock.now().wrapping_add(timeout);
        loop {
            let idx = dma.read_u16(used_idx_off);
            let last = self.rx_last_used.load(Ordering::Relaxed) as u16;
            if idx != last {
                bus.fence();
                // Nhận lại đệm từ thiết bị **trước** khi đọc nội dung (aarch64: hook cache).
                dma.sync_for_cpu(dma_off + RX_BUF_OFF, RX_BUF_BYTES);
                let element = self
                    .queue_with(dma, 0, size)
                    .ok()?
                    .used_elem(last & (size - 1));
                self.rx_last_used
                    .store(last.wrapping_add(1) as u64, Ordering::Relaxed);
                if element.0 != 0 {
                    self.id_mismatch.fetch_add(1, Ordering::Relaxed);
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    let _ = self.arm_rx(bus, dma);
                    return None;
                }
                let frame = match frame_len(element.1, true) {
                    Ok(n) => n,
                    Err(_) => {
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        let _ = self.arm_rx(bus, dma);
                        return None;
                    }
                };
                if frame > out.len() {
                    // Đệm người gọi nhỏ hơn frame: **không** cắt im lặng.
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    let _ = self.arm_rx(bus, dma);
                    return None;
                }
                let hdr_len = hdr_bytes(true);
                out[..frame].copy_from_slice(
                    &dma.bytes()
                        [dma_off + RX_BUF_OFF + hdr_len..dma_off + RX_BUF_OFF + hdr_len + frame],
                );
                self.rx_frames.fetch_add(1, Ordering::Relaxed);
                self.rx_bytes.fetch_add(frame as u64, Ordering::Relaxed);
                // Arm lại **trước** khi trả: cửa sổ không có đệm là nhỏ nhất.
                let _ = self.arm_rx(bus, dma);
                return Some(frame);
            }
            if past_deadline(clock.now(), deadline) {
                return None;
            }
            core::hint::spin_loop();
        }
    }
}

/// Đóng MAC 6 byte vào một `u64` để lưu bằng `AtomicU64` (không cần `UnsafeCell`).
const fn pack_mac(mac: [u8; 6]) -> u64 {
    (mac[0] as u64)
        | ((mac[1] as u64) << 8)
        | ((mac[2] as u64) << 16)
        | ((mac[3] as u64) << 24)
        | ((mac[4] as u64) << 32)
        | ((mac[5] as u64) << 40)
}

/// Mở `u64` thành MAC 6 byte.
const fn unpack_mac(packed: u64) -> [u8; 6] {
    [
        packed as u8,
        (packed >> 8) as u8,
        (packed >> 16) as u8,
        (packed >> 24) as u8,
        (packed >> 32) as u8,
        (packed >> 40) as u8,
    ]
}
