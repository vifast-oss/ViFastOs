// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-net** (virtio 1.0, virtio-pci modern) trên x86_64 — KT-274 (K5' bước 5).
//!
//! # Đường đi
//!
//! [`crate::pci::init_for`]`(VirtioKind::Net)` (ECAM/legacy của KT-274, **không** nhân đôi code
//! PCI) ⇒ BAR modern + bốn capability ⇒ tệp này lập trình common config, dựng **hai** split
//! virtqueue (`rx` = queue 0, `tx` = queue 1 — Virtio 1.2 §5.1.2), đọc **MAC thật** từ
//! `virtio_net_config`, rồi `send_frame`/`recv_frame_wait` và **poll `used`** (chưa dùng ngắt).
//!
//! # Bất biến (giữ nguyên từ khuôn riscv64 KT-245)
//!
//! - Feature đàm phán: `VIRTIO_F_VERSION_1` (bắt buộc) + `VIRTIO_NET_F_MAC` **nếu** thiết bị có.
//!   **Không** `MRG_RXBUF`, **không** `STATUS` — quyết định nằm ở `vifast_virtio::net`
//!   (`negotiated_features`, test host), không viết tay hai chỗ (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **fence** → ghi `avail->idx` →
//!   **fence** → notify (MMIO) → poll `used->idx` → **fence** → đọc `used->elem`.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - Một frame một lúc mỗi hướng (queue depth 1 thực tế); đủ cho ARP và dễ chứng minh.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); frame sai độ dài ⇒
//!   `BadFrame` **trước** khi chạm ring; `used->len` vô lý ⇒ `Io` + đếm `errors`.
//! - `rx` arm **trước** (một descriptor device-writable [`RX_BUF_BYTES`]) để frame host gửi tới
//!   không bị rơi; `recv_frame_wait` xong arm lại ngay.
//!
//! # Đồng hồ: **tham số**, không phải singleton
//!
//! Khuôn riscv64 gọi `Riscv64Virt.now_ticks()`. Trên x86_64 bước này chạy sau `cli` nên **không**
//! dùng được nguồn tick PIT→IOAPIC→LAPIC; đồng hồ là PIT kênh 2 ([`crate::timebase::PitClock`])
//! do ảnh sở hữu. Vì vậy mọi hàm chờ nhận `now: &mut dyn FnMut() -> u64` (µs) — driver **không**
//! giữ trạng thái thời gian, và test host không cần đồng hồ thật.
//!
//! # Vì sao tệp này có `unsafe`
//!
//! MMIO của BAR virtio + vùng DMA (`UnsafeCell` + `Sync` thủ công) là truy cập con trỏ thô, đúng
//! danh sách đóng D2-17 (`kernel/vifast-plat-*`) và **y hệt** khuôn
//! `kernel/vifast-plat-riscv64/src/virtio_net.rs`. Crate vẫn `#![deny(unsafe_code)]`; chỉ tệp này
//! (và [`crate::asm`]) `#![allow(unsafe_code)]`.

#![allow(unsafe_code)] // D2-17: `kernel/vifast-plat-*`; MMIO + vùng DMA.

use core::cell::UnsafeCell;
use core::sync::atomic::{fence, Ordering};

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::arp::{ArpPacket, ARP_LEN};
use vifast_net::eth::{EthHeader, ETHERTYPE_ARP};
use vifast_net::frame::EthernetFrame;
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::net::{
    decode_config, frame_len, frame_ok, hdr_bytes, mac_negotiated, negotiated_features, NetConfig,
    NetError, NetHdr, MAX_FRAME_BYTES, MIN_QUEUE_SIZE, NET_CONFIG_BYTES, QUEUES_REQUIRED,
    QUEUE_SIZE, Q_RX, Q_TX, RX_BUF_BYTES, TX_BUF_BYTES,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::asm;
use crate::pci::PciReport;

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `rx` trong trang DMA.
const RX_RING_OFF: usize = 0x000;
/// Offset ring `tx` trong trang DMA.
const TX_RING_OFF: usize = 0x200;
/// Offset đệm `rx` (device-writable: header + frame).
const RX_BUF_OFF: usize = 0x400;
/// Offset đệm `tx` (driver-writable: header + frame).
const TX_BUF_OFF: usize = 0xA00;
/// Timeout poll `used` mặc định — 2 s (cùng ngân sách khuôn riscv64 KT-72/73/245).
pub const POLL_TIMEOUT_US: u64 = 2_000_000;
/// Timeout mặc định cho **một bước** mạng (ARP) — 3 s.
pub const NET_STEP_TIMEOUT_US: u64 = 3_000_000;

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

// ---------------------------------------------------------------- bộ đếm (an toàn, atomic)
static TX_FRAMES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static TX_BYTES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static RX_FRAMES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static RX_BYTES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static NOTIFIES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static ERRORS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static TIMEOUTS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static ID_MISMATCH: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Ảnh chụp bộ đếm để in evidence (đọc một lần, không tách rời từng số).
#[derive(Clone, Copy, Debug, Default)]
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
    /// Số lỗi driver.
    pub errors: u64,
    /// Số lần poll quá hạn (phải 0 ở đường thành công).
    pub timeouts: u64,
    /// Số lần `used->id` lệch head (phải 0).
    pub id_mismatch: u64,
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    Stats {
        tx_frames: TX_FRAMES.load(Ordering::SeqCst),
        tx_bytes: TX_BYTES.load(Ordering::SeqCst),
        rx_frames: RX_FRAMES.load(Ordering::SeqCst),
        rx_bytes: RX_BYTES.load(Ordering::SeqCst),
        notifies: NOTIFIES.load(Ordering::SeqCst),
        errors: ERRORS.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
        id_mismatch: ID_MISMATCH.load(Ordering::SeqCst),
    }
}

/// Trạng thái driver — gói trong `UnsafeCell` + `Sync` thủ công: ảnh chạy **một** CPU, driver tự
/// tuần tự hoá (một frame một lúc mỗi hướng), không có truy cập song song.
struct State {
    ready: bool,
    common: u64,
    notify: u64,
    isr: u64,
    notify_mul: u32,
    /// `queue_notify_off` của từng queue (chỉ số 0 = rx, 1 = tx).
    notify_off: [u16; 2],
    /// Cỡ queue thật của từng queue (đọc lại từ thiết bị).
    queue_size: [u16; 2],
    /// MAC đọc từ `virtio_net_config`.
    mac: [u8; 6],
    /// `true` ⇔ header là bản modern 12 byte (luôn đúng khi đã đàm phán `VERSION_1`).
    modern: bool,
    /// `used->idx` đã xử lý của `rx`.
    rx_last_used: u16,
    /// `used->idx` đã xử lý của `tx`.
    tx_last_used: u16,
}

const INIT_STATE: State = State {
    ready: false,
    common: 0,
    notify: 0,
    isr: 0,
    notify_mul: 0,
    notify_off: [0; 2],
    queue_size: [0; 2],
    mac: [0; 6],
    modern: true,
    rx_last_used: 0,
    tx_last_used: 0,
};

struct Cell<T>(UnsafeCell<T>);
// SAFETY: một CPU, truy cập tuần tự trong module này (không chia sẻ giữa các CPU).
unsafe impl<T> Sync for Cell<T> {}

static STATE: Cell<State> = Cell(UnsafeCell::new(INIT_STATE));

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: như `STATE` — vùng DMA chỉ driver này chạm; thiết bị chạm qua PA (DMA) theo hợp đồng.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA`.
pub fn dma_page_addr() -> u64 {
    DMA.0.get() as *const _ as u64
}

fn state() -> &'static mut State {
    // SAFETY: một CPU + tuần tự (xem `Cell`).
    unsafe { &mut *STATE.0.get() }
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một CPU + tuần tự; vùng này không ai khác (trừ DMA của thiết bị) chạm.
    unsafe { &mut (*DMA.0.get()).0 }
}

// ---------------------------------------------------------------- MMIO
#[inline]
fn read8(addr: u64) -> u8 {
    asm::read_u8(addr)
}
#[inline]
fn read16(addr: u64) -> u16 {
    asm::read_u16(addr)
}
#[inline]
fn read32(addr: u64) -> u32 {
    asm::read_u32(addr)
}
#[inline]
fn write8(addr: u64, value: u8) {
    asm::write_u8(addr, value);
}
#[inline]
fn write16(addr: u64, value: u16) {
    asm::write_u16(addr, value);
}
#[inline]
fn write32(addr: u64, value: u32) {
    asm::write_u32(addr, value);
}
#[inline]
fn write64(addr: u64, value: u64) {
    asm::write_u64(addr, value);
}

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13: driver phải
/// có barrier trước notify; thiếu ⇒ thiết bị có thể đọc `avail` cũ).
#[inline]
fn barrier() {
    fence(Ordering::SeqCst);
}

// ---------------------------------------------------------------- báo cáo khởi tạo
/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct NetReport {
    /// Vendor/device (từ PCI config).
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// Bus/device/function.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Chỉ số BAR driver dùng.
    pub bar_index: u8,
    /// Địa chỉ BAR firmware đã gán.
    pub bar_base: u64,
    /// Cỡ BAR đo được.
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

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
pub fn init(pci: &PciReport) -> Result<NetReport, &'static str> {
    let caps = pci.caps;
    let bar = pci.bar_base;
    let common = bar + caps.common.offset as u64;
    let notify = bar + caps.notify.offset as u64;
    let device_cfg = bar + caps.device.offset as u64;
    let isr = bar + caps.isr.offset as u64;
    let notify_mul = caps.notify_off_multiplier;
    let rx_layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    let tx_layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    if RX_RING_OFF + rx_layout.total_bytes() > TX_RING_OFF
        || TX_RING_OFF + tx_layout.total_bytes() > RX_BUF_OFF
        || RX_BUF_OFF + RX_BUF_BYTES > TX_BUF_OFF
        || TX_BUF_OFF + TX_BUF_BYTES > DMA_PAGE_BYTES
    {
        return Err("layout");
    }
    // Vùng DMA sạch trước khi bàn giao (kể cả khi init gọi lại).
    for byte in page().iter_mut() {
        *byte = 0;
    }

    // ---- 1) reset (§2.4.2 bước 1)
    write8(common + C_DEVICE_STATUS, 0);
    if read8(common + C_DEVICE_STATUS) != 0 {
        return Err("reset");
    }
    // ---- 2) ACK + DRIVER
    write8(common + C_DEVICE_STATUS, ACK);
    write8(common + C_DEVICE_STATUS, ACK | DRIVER);
    // ---- 3) feature thiết bị (hai trang 32 bit)
    write32(common + C_DEVICE_FEATURE_SELECT, 0);
    let low = read32(common + C_DEVICE_FEATURE) as u64;
    write32(common + C_DEVICE_FEATURE_SELECT, 1);
    let high = read32(common + C_DEVICE_FEATURE) as u64;
    let device_features = (high << 32) | low;
    if device_features & (1u64 << F_VERSION_1) == 0 {
        return Err("version1");
    }
    // ---- 4) đàm phán: VERSION_1 bắt buộc + MAC nếu thiết bị có (quyết định ở hàm thuần
    // `negotiated_features`, test host — không viết tay hai chỗ).
    let wanted = negotiated_features(device_features);
    if wanted & (1u64 << F_VERSION_1) == 0 {
        return Err("version1");
    }
    write32(common + C_DRIVER_FEATURE_SELECT, 0);
    write32(common + C_DRIVER_FEATURE, (wanted & 0xffff_ffff) as u32);
    write32(common + C_DRIVER_FEATURE_SELECT, 1);
    write32(common + C_DRIVER_FEATURE, (wanted >> 32) as u32);
    // đọc lại cả hai trang
    write32(common + C_DRIVER_FEATURE_SELECT, 0);
    let drv_low = read32(common + C_DRIVER_FEATURE) as u64;
    write32(common + C_DRIVER_FEATURE_SELECT, 1);
    let drv_high = read32(common + C_DRIVER_FEATURE) as u64;
    let driver_features = (drv_high << 32) | drv_low;
    if driver_features != wanted {
        return Err("features");
    }
    let mac_ok = mac_negotiated(driver_features);
    // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
    write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
    let status = read8(common + C_DEVICE_STATUS);
    if !features_ok(status) {
        return Err("features_ok");
    }
    // ---- 6) hai queue: rx = 0, tx = 1
    let num_queues = read16(common + C_NUM_QUEUES);
    if num_queues < QUEUES_REQUIRED {
        return Err("queues");
    }
    let dma = dma_page_addr();
    let ring_off = [RX_RING_OFF, TX_RING_OFF];
    let mut queue_max = [0u16; 2];
    let mut queue_size = [0u16; 2];
    let mut notify_off = [0u16; 2];
    for (slot, index) in [Q_RX, Q_TX].iter().enumerate() {
        write16(common + C_QUEUE_SELECT, *index);
        let max = read16(common + C_QUEUE_SIZE);
        // Chain của driver dùng **một** descriptor mỗi frame ⇒ queue chỉ cần ≥ 2 (spec: luỹ thừa 2).
        if max < MIN_QUEUE_SIZE || !max.is_power_of_two() {
            return Err("queue_size");
        }
        let size = QUEUE_SIZE.min(max);
        write16(common + C_QUEUE_SIZE, size);
        if read16(common + C_QUEUE_SIZE) != size {
            return Err("queue_size");
        }
        let layout = VqLayout::new(size).ok_or("layout")?;
        let base = dma + ring_off[slot] as u64;
        write64(common + C_QUEUE_DESC, base + layout.desc_off as u64);
        write64(common + C_QUEUE_DRIVER, base + layout.avail_off as u64);
        write64(common + C_QUEUE_DEVICE, base + layout.used_off as u64);
        barrier();
        write16(common + C_QUEUE_ENABLE, 1);
        if read16(common + C_QUEUE_ENABLE) != 1 {
            return Err("queue_enable");
        }
        queue_max[slot] = max;
        queue_size[slot] = size;
        notify_off[slot] = read16(common + C_QUEUE_NOTIFY_OFF);
    }
    // ---- 7) DRIVER_OK
    write8(
        common + C_DEVICE_STATUS,
        ACK | DRIVER | FEATURES_OK | DRIVER_OK,
    );
    let status = read8(common + C_DEVICE_STATUS);
    if !status_sequence_ok(status) {
        return Err("driver_ok");
    }
    // ---- 8) device config: mac/status/max_virtqueue_pairs/mtu đọc từ thiết bị
    let generation_before = read8(common + C_CONFIG_GENERATION);
    let mut raw = [0u8; NET_CONFIG_BYTES];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = read8(device_cfg + index as u64);
    }
    let config_generation = read8(common + C_CONFIG_GENERATION);
    // Spec §2.4.4: nếu `config_generation` đổi trong lúc đọc config thì dữ liệu có thể **xé** ⇒
    // fail-closed (không dùng MAC đọc được nửa vời).
    if config_generation != generation_before {
        return Err("config_generation");
    }
    let config = decode_config(&raw).ok_or("config")?;
    // MAC chỉ được tin khi đã đàm phán `VIRTIO_NET_F_MAC`; và giá trị phải **hợp lệ** (không toàn
    // 0, không broadcast) — thiết bị thật không phát hai giá trị đó.
    if !mac_ok || !config.mac_hop_le() {
        return Err("mac");
    }
    // ---- 9) lưu trạng thái (khối riêng: không giữ `&mut State` qua lời gọi `arm_rx`)
    {
        let s = state();
        s.ready = true;
        s.common = common;
        s.notify = notify;
        s.isr = isr;
        s.notify_mul = notify_mul;
        s.notify_off = notify_off;
        s.queue_size = queue_size;
        s.mac = config.mac;
        s.modern = true;
        s.rx_last_used = 0;
        s.tx_last_used = 0;
    }
    // ---- 10) arm `rx` ngay: frame host gửi tới phải có đệm trước khi người gọi dùng driver.
    arm_rx()?;
    Ok(NetReport {
        vendor: pci.vendor,
        device: pci.device,
        bus: pci.bus,
        dev: pci.dev,
        func: pci.func,
        bar_index: pci.bar_index,
        bar_base: bar,
        bar_size: pci.bar_size,
        common,
        notify,
        device_cfg,
        isr,
        notify_mul,
        notify_off_rx: notify_off[0],
        notify_off_tx: notify_off[1],
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
        desc_rx: dma + RX_RING_OFF as u64 + rx_layout.desc_off as u64,
        avail_rx: dma + RX_RING_OFF as u64 + rx_layout.avail_off as u64,
        used_rx: dma + RX_RING_OFF as u64 + rx_layout.used_off as u64,
        desc_tx: dma + TX_RING_OFF as u64 + tx_layout.desc_off as u64,
        avail_tx: dma + TX_RING_OFF as u64 + tx_layout.avail_off as u64,
        used_tx: dma + TX_RING_OFF as u64 + tx_layout.used_off as u64,
        rx_buf_pa: dma + RX_BUF_OFF as u64,
        tx_buf_pa: dma + TX_BUF_OFF as u64,
    })
}

/// Bọc ring của queue `slot` (0 = rx, 1 = tx) thành split queue với cỡ **thật** của nó.
///
/// Nhận `size` từ người gọi (không tự đọc trạng thái) để không có hai `&mut State` sống cùng lúc.
fn queue_with(slot: usize, size: u16) -> Result<SplitQueue<'static>, NetError> {
    if size < MIN_QUEUE_SIZE {
        return Err(NetError::NotReady);
    }
    let layout = VqLayout::new(size).ok_or(NetError::NotReady)?;
    let off = if slot == 0 { RX_RING_OFF } else { TX_RING_OFF };
    SplitQueue::new(&mut page()[off..off + layout.total_bytes()], size).ok_or(NetError::NotReady)
}

/// Đăng một descriptor device-writable cho `rx` và publish — gọi lúc init và sau mỗi frame nhận.
fn arm_rx() -> Result<(), &'static str> {
    let size = state().queue_size[0];
    let notify = state().notify;
    let notify_off = state().notify_off[0];
    let notify_mul = state().notify_mul;
    if size < MIN_QUEUE_SIZE {
        return Err("arm");
    }
    let dma = dma_page_addr();
    {
        let mut q = queue_with(0, size).map_err(|_| "arm")?;
        if !q.set_desc(
            0,
            Desc {
                addr: dma + RX_BUF_OFF as u64,
                len: RX_BUF_BYTES as u32,
                flags: DescFlags::WRITE,
                next: 0,
            },
        ) {
            return Err("arm");
        }
        q.push_avail(0);
    }
    barrier();
    write16(notify + notify_off as u64 * notify_mul as u64, Q_RX);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Gửi **một** frame Ethernet (không kể header virtio). `Err` ⇒ không có gì được gửi hoặc thiết
/// bị không hoàn tất trong `timeout_us`.
pub fn send_frame(
    frame: &[u8],
    timeout_us: u64,
    now: &mut dyn FnMut() -> u64,
) -> Result<(), NetError> {
    if !state().ready {
        return Err(NetError::NotReady);
    }
    // Kiểm **trước** khi chạm ring: frame sai độ dài không được đưa vào descriptor.
    if !frame_ok(frame) {
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err(NetError::BadFrame);
    }
    let modern = state().modern;
    let size = state().queue_size[1];
    let notify = state().notify;
    let notify_off = state().notify_off[1];
    let notify_mul = state().notify_mul;
    if size < MIN_QUEUE_SIZE {
        return Err(NetError::NotReady);
    }
    let hdr_len = hdr_bytes(modern);
    let total = hdr_len + frame.len();
    let dma = dma_page_addr();
    let tx_pa = dma + TX_BUF_OFF as u64;
    {
        let buf = &mut page()[TX_BUF_OFF..TX_BUF_OFF + total];
        // Header rỗng: không checksum offload, không GSO (driver không đàm phán feature nào của
        // hai đường đó).
        buf[..hdr_len].copy_from_slice(&NetHdr::zero().encode(modern)[..hdr_len]);
        buf[hdr_len..total].copy_from_slice(frame);
    }
    {
        let mut q = queue_with(1, size)?;
        if !q.set_desc(
            0,
            Desc {
                addr: tx_pa,
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
    barrier();
    write16(notify + notify_off as u64 * notify_mul as u64, Q_TX);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);

    // ---- poll used của `tx` (một chiều)
    let used_idx_addr = dma
        + TX_RING_OFF as u64
        + VqLayout::new(size).ok_or(NetError::NotReady)?.used_idx_off as u64;
    let deadline = now().saturating_add(timeout_us);
    loop {
        // `asm::read_u16` là đọc `volatile` — compiler **không** hoist khỏi vòng lặp (bẫy kinh
        // điển của poll DMA).
        let idx = read16(used_idx_addr);
        let last = state().tx_last_used;
        if idx != last {
            barrier();
            let element = queue_with(1, size)?.used_elem(last & (size - 1));
            state().tx_last_used = last.wrapping_add(1);
            if element.0 != 0 {
                ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
                ERRORS.fetch_add(1, Ordering::SeqCst);
                return Err(NetError::Io);
            }
            // `used->len` của `tx` **không** mang nghĩa "đã nhận bao nhiêu byte" (không có đệm
            // device-writable) — chỉ kiểm nó không vượt số byte đã đưa (cùng lý lẽ KT-73).
            if element.1 > total as u32 {
                ERRORS.fetch_add(1, Ordering::SeqCst);
                return Err(NetError::Io);
            }
            TX_FRAMES.fetch_add(1, Ordering::SeqCst);
            TX_BYTES.fetch_add(frame.len() as u64, Ordering::SeqCst);
            return Ok(());
        }
        if now() >= deadline {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            ERRORS.fetch_add(1, Ordering::SeqCst);
            return Err(NetError::Timeout);
        }
        core::hint::spin_loop();
    }
}

/// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ).
pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    recv_frame_wait(out, 0, &mut || 0)
}

/// Nhận **một** frame, chờ tối đa `timeout_us`. `None` = hết hạn / chưa có.
///
/// `used->len` vô lý (nhỏ hơn header, lớn hơn đệm) ⇒ đếm `errors`, **không** trả dữ liệu rác;
/// descriptor được arm lại để lần sau vẫn nhận được.
pub fn recv_frame_wait(
    out: &mut [u8],
    timeout_us: u64,
    now: &mut dyn FnMut() -> u64,
) -> Option<usize> {
    if !state().ready {
        return None;
    }
    let size = state().queue_size[0];
    if size < MIN_QUEUE_SIZE {
        return None;
    }
    let modern = state().modern;
    let dma = dma_page_addr();
    let used_idx_addr = dma + RX_RING_OFF as u64 + VqLayout::new(size)?.used_idx_off as u64;
    let deadline = now().saturating_add(timeout_us);
    loop {
        let idx = read16(used_idx_addr);
        let last = state().rx_last_used;
        if idx != last {
            barrier();
            let element = queue_with(0, size).ok()?.used_elem(last & (size - 1));
            state().rx_last_used = last.wrapping_add(1);
            if element.0 != 0 {
                ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
                ERRORS.fetch_add(1, Ordering::SeqCst);
                let _ = arm_rx();
                return None;
            }
            let frame = match frame_len(element.1, modern) {
                Ok(n) => n,
                Err(_) => {
                    ERRORS.fetch_add(1, Ordering::SeqCst);
                    let _ = arm_rx();
                    return None;
                }
            };
            if frame > out.len() {
                // Đệm người gọi nhỏ hơn frame: **không** cắt im lặng.
                ERRORS.fetch_add(1, Ordering::SeqCst);
                let _ = arm_rx();
                return None;
            }
            let hdr_len = hdr_bytes(modern);
            let src = &page()[RX_BUF_OFF + hdr_len..RX_BUF_OFF + hdr_len + frame];
            out[..frame].copy_from_slice(src);
            RX_FRAMES.fetch_add(1, Ordering::SeqCst);
            RX_BYTES.fetch_add(frame as u64, Ordering::SeqCst);
            // Arm lại **trước** khi trả: cửa sổ không có đệm là nhỏ nhất.
            let _ = arm_rx();
            return Some(frame);
        }
        if now() >= deadline {
            return None;
        }
        core::hint::spin_loop();
    }
}

/// MAC đọc từ thiết bị (toàn 0 nếu chưa init).
pub fn mac() -> [u8; 6] {
    state().mac
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    state().ready
}

/// Địa chỉ ISR (đọc 1 byte để lấy lý do ngắt — bước này chưa dùng ngắt, chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    state().isr
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều frame).
pub fn device_status() -> u8 {
    let common = state().common;
    if common == 0 {
        return 0;
    }
    read8(common + C_DEVICE_STATUS)
}

// ================================================================================
// ARP — KT-274 (D2-208): hỏi MAC của gateway và nhận **reply thật** từ slirp
// ================================================================================

/// Dựng **ARP request** (Ethernet/IPv4) hỏi MAC của `target_ip`; trả tổng số byte (42).
///
/// Chỉ là lớp ghép khung cho driver: codec ARP + Ethernet nằm ở `vifast_net` (không viết lại).
pub fn arp_build_request(
    mac: MacAddr,
    source_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    out: &mut [u8],
) -> Option<usize> {
    let mut arp = [0u8; ARP_LEN];
    let arp_len = ArpPacket::request(mac, source_ip, target_ip)
        .encode(&mut arp)
        .ok()?;
    EthernetFrame {
        header: EthHeader::new(MacAddr::BROADCAST, mac, ETHERTYPE_ARP),
        payload: &arp[..arp_len],
    }
    .encode(out)
    .ok()
}

/// Kiểm một **ARP reply** cho `target_ip` gửi tới `our_ip`/`our_mac`; trả MAC người trả lời.
///
/// # Bẫy đã tự gặp (KT-246, giữ nguyên ở đây)
///
/// Trong **ARP reply**, `ar_tip` là địa chỉ của **người hỏi** (`our_ip`), **không** phải
/// `target_ip` — `ar_spa` mới là `target_ip`. Bản đầu kiểm `ar_tip == target_ip` nên **mọi** reply
/// hợp lệ đều bị từ chối và ARP timeout dù thiết bị đã trả lời.
fn arp_parse_reply(
    frame: &[u8],
    our_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    our_mac: MacAddr,
) -> Option<MacAddr> {
    let eth = EthernetFrame::parse(frame).ok()?;
    if eth.header.ethertype != ETHERTYPE_ARP {
        return None;
    }
    let arp = ArpPacket::parse(eth.payload).ok()?;
    if !arp.is_reply() || arp.spa != target_ip || arp.tpa != our_ip || arp.tha != our_mac {
        return None;
    }
    // MAC toàn 0 / broadcast là giá trị vô nghĩa cho một host — không nhận.
    if arp.sha.is_zero() || arp.sha.is_broadcast() {
        return None;
    }
    Some(arp.sha)
}

/// **ARP resolve**: hỏi MAC của `target_ip` (dùng `our_ip`/MAC của ta làm nguồn).
///
/// `Err(NetError::Timeout)` nếu không có reply trong `timeout_us` — đường "không có mạng"
/// (fail-closed, không treo).
pub fn arp_resolve(
    our_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    timeout_us: u64,
    now: &mut dyn FnMut() -> u64,
) -> Result<MacAddr, NetError> {
    let our_mac = MacAddr::new(mac());
    let mut frame = [0u8; 64];
    let len =
        arp_build_request(our_mac, our_ip, target_ip, &mut frame).ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len], POLL_TIMEOUT_US, now)?;
    let deadline = now().saturating_add(timeout_us);
    let mut reply = [0u8; MAX_FRAME_BYTES];
    loop {
        let remaining = deadline.saturating_sub(now());
        if remaining == 0 {
            return Err(NetError::Timeout);
        }
        if let Some(got) = recv_frame_wait(&mut reply, remaining, now) {
            if let Some(found) = arp_parse_reply(&reply[..got], our_ip, target_ip, our_mac) {
                return Ok(found);
            }
        }
    }
}

const _: () = {
    // Hai ring + hai đệm phải nằm gọn trong **một** trang DMA 4 KiB.
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(l) => l,
        None => panic!("queue size khong hop le"),
    };
    assert!(RX_RING_OFF + layout.end <= TX_RING_OFF);
    assert!(TX_RING_OFF + layout.end <= RX_BUF_OFF);
    assert!(RX_BUF_OFF + RX_BUF_BYTES <= TX_BUF_OFF);
    assert!(TX_BUF_OFF + TX_BUF_BYTES <= DMA_PAGE_BYTES);
    assert!(vifast_virtio::net::HDR_MODERN_BYTES <= RX_BUF_BYTES);
};
