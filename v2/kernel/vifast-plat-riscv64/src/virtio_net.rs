// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-net** (virtio 1.0, virtio-pci modern) trên QEMU `virt` — KT-245 (D1).
//!
//! # Đường đi
//!
//! `pci::init_for(VirtioKind::Net)` (ECAM của KT-72, **không** nhân đôi code PCI) ⇒ BAR modern +
//! bốn capability ⇒ module này lập trình common config, dựng **hai** split virtqueue
//! (`rx` = queue 0, `tx` = queue 1 — §5.1.2), đọc **MAC thật** từ `virtio_net_config`, rồi
//! `send_frame`/`recv_frame` và **poll `used`** (KT-245 chưa dùng ngắt).
//!
//! # Bất biến
//!
//! - Feature đàm phán: `VIRTIO_F_VERSION_1` (bắt buộc) + `VIRTIO_NET_F_MAC` **nếu** thiết bị có.
//!   **Không** `MRG_RXBUF`, **không** `STATUS` — quyết định nằm ở `vifast_virtio::net`
//!   (`negotiated_features`, test host), không viết tay hai chỗ (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **fence** → ghi `avail->idx` →
//!   **fence** → notify (MMIO) → poll `used->idx` → **fence** → đọc `used->elem`.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - Một frame một lúc mỗi hướng (queue depth 1 thực tế); đủ cho DHCP/ARP/ICMP và dễ chứng minh.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); frame sai độ dài ⇒
//!   `BadFrame` **trước** khi chạm ring; `used->len` vô lý ⇒ `Io` + đếm `errors`.
//! - `rx` arm **trước** (một descriptor device-writable [`RX_BUF_BYTES`]) để frame host gửi tới
//!   không bị rơi; `recv_frame` xong arm lại ngay.
//!
//! `unsafe` (MMIO + `UnsafeCell` vùng DMA) nằm trong `kernel/vifast-plat-*` (D2-17); không lệnh
//! đặc quyền nào ở U-mode.

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::timer::TIMEBASE_HZ;
use vifast_plat_common::Platform;
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::net::{
    decode_config, frame_len, frame_ok, hdr_bytes, mac_negotiated, negotiated_features, NetConfig,
    NetError, NetHdr, HDR_MODERN_BYTES, MAX_FRAME_BYTES, MIN_QUEUE_SIZE, NET_CONFIG_BYTES,
    QUEUES_REQUIRED, QUEUE_SIZE, Q_RX, Q_TX, RX_BUF_BYTES, TX_BUF_BYTES,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::pci::PciReport;
use crate::Riscv64Virt;

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
/// Timeout poll `used` (tick timebase) — 2 s ảo (cùng quy ước KT-72/73).
const POLL_TIMEOUT_TICKS: u64 = 2 * TIMEBASE_HZ;

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
static TX_FRAMES: AtomicU64 = AtomicU64::new(0);
static TX_BYTES: AtomicU64 = AtomicU64::new(0);
static RX_FRAMES: AtomicU64 = AtomicU64::new(0);
static RX_BYTES: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);

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
    /// Số lỗi driver (frame sai, `used->len` vô lý, thiết bị trả lỗi).
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

/// Trạng thái driver — gói trong `UnsafeCell` + `Sync` thủ công: ảnh test chạy **một hart**,
/// driver tự tuần tự hoá (một frame một lúc mỗi hướng), không có truy cập song song.
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
// SAFETY: một hart, truy cập tuần tự trong module này (không chia sẻ giữa các hart).
unsafe impl<T> Sync for Cell<T> {}

static STATE: Cell<State> = Cell(UnsafeCell::new(INIT_STATE));

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: như `STATE` — vùng DMA chỉ driver này chạm; thiết bị chạm qua PA (DMA) theo hợp đồng.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA` (TAP kiểm bằng `walk_sw`).
pub fn dma_page_addr() -> u64 {
    DMA.0.get() as *const _ as u64
}

fn state() -> &'static mut State {
    // SAFETY: một hart + tuần tự (xem `Cell`).
    unsafe { &mut *STATE.0.get() }
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một hart + tuần tự; vùng này không ai khác (trừ DMA của thiết bị) chạm.
    unsafe { &mut (*DMA.0.get()).0 }
}

// ---------------------------------------------------------------- MMIO
#[inline]
fn read8(addr: u64) -> u8 {
    // SAFETY: `addr` nằm trong BAR đã gán/map Device RW (kiểm bằng walk trong TAP).
    unsafe { read_volatile(addr as *const u8) }
}
#[inline]
fn read16(addr: u64) -> u16 {
    // SAFETY: như `read8`; bán kính 16 bit căn 2.
    unsafe { read_volatile(addr as *const u16) }
}
#[inline]
fn read32(addr: u64) -> u32 {
    // SAFETY: như `read8`; căn 4.
    unsafe { read_volatile(addr as *const u32) }
}
#[inline]
fn write8(addr: u64, value: u8) {
    // SAFETY: như `read8`; ghi thanh ghi thiết bị đã map.
    unsafe { write_volatile(addr as *mut u8, value) };
}
#[inline]
fn write16(addr: u64, value: u16) {
    // SAFETY: như `write8`.
    unsafe { write_volatile(addr as *mut u16, value) };
}
#[inline]
fn write32(addr: u64, value: u32) {
    // SAFETY: như `write8`.
    unsafe { write_volatile(addr as *mut u32, value) };
}
#[inline]
fn write64(addr: u64, value: u64) {
    // SAFETY: như `write8`.
    unsafe { write_volatile(addr as *mut u64, value) };
}

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13: driver phải
/// có barrier trước notify; thiếu ⇒ thiết bị có thể đọc `avail` cũ).
#[inline]
fn fence() {
    // SAFETY: `fence` chỉ là hàng rào thứ tự, không truy cập địa chỉ.
    unsafe { core::arch::asm!("fence rw, rw", options(nostack, preserves_flags)) };
}

// ---------------------------------------------------------------- báo cáo khởi tạo
/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct NetReport {
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
    /// Địa chỉ (PA) bảng descriptor `rx`.
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
    let bar = pci.bar_assigned;
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
        fence();
        write16(common + C_QUEUE_ENABLE, 1);
        if read16(common + C_QUEUE_ENABLE) != 1 {
            return Err("queue_enable");
        }
        queue_max[slot] = max;
        queue_size[slot] = size;
        notify_off[slot] = read16(common + C_QUEUE_NOTIFY_OFF);
    }
    let notify_addr_rx = notify + notify_off[0] as u64 * notify_mul as u64;
    let notify_addr_tx = notify + notify_off[1] as u64 * notify_mul as u64;
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
    fence();
    write16(notify + notify_off as u64 * notify_mul as u64, Q_RX);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Gửi **một** frame Ethernet (không kể header virtio). `Err` ⇒ không có gì được gửi hoặc thiết
/// bị không hoàn tất trong [`POLL_TIMEOUT_TICKS`].
pub fn send_frame(frame: &[u8]) -> Result<(), NetError> {
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
        // Header rỗng: không checksum offload, không GSO (driver KT-245 không đàm phán feature
        // nào của hai đường đó).
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
    fence();
    write16(notify + notify_off as u64 * notify_mul as u64, Q_TX);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);

    // ---- poll used của `tx` (một chiều)
    let used_idx_addr = dma
        + TX_RING_OFF as u64
        + VqLayout::new(size).ok_or(NetError::NotReady)?.used_idx_off as u64;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(POLL_TIMEOUT_TICKS);
    loop {
        // SAFETY: `used_idx_addr` nằm trong trang DMA đã bàn giao; đọc volatile để compiler
        // **không** hoist khỏi vòng lặp (bẫy kinh điển của poll DMA).
        let idx = unsafe { read_volatile(used_idx_addr as *const u16) };
        let last = state().tx_last_used;
        if idx != last {
            fence();
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
        if Riscv64Virt.now_ticks() > deadline {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            ERRORS.fetch_add(1, Ordering::SeqCst);
            return Err(NetError::Timeout);
        }
        core::hint::spin_loop();
    }
}

/// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có frame.
///
/// `used->len` vô lý (nhỏ hơn header, lớn hơn đệm) ⇒ đếm `errors`, **không** trả dữ liệu rác;
/// descriptor được arm lại để lần sau vẫn nhận được.
pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    recv_frame_wait(out, 0)
}

/// Nhận **một** frame, chờ tối đa `timeout_ticks` tick timebase. `None` = hết hạn / chưa có.
pub fn recv_frame_wait(out: &mut [u8], timeout_ticks: u64) -> Option<usize> {
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
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    loop {
        // SAFETY: như `send_frame` — đọc volatile `used->idx` trong trang DMA.
        let idx = unsafe { read_volatile(used_idx_addr as *const u16) };
        let last = state().rx_last_used;
        if idx != last {
            fence();
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
        if Riscv64Virt.now_ticks() > deadline {
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

/// Địa chỉ ISR (đọc 1 byte để lấy lý do ngắt — KT-245 chưa dùng ngắt, chỉ in chẩn đoán).
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

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    let common = state().common;
    if common == 0 {
        return 0;
    }
    read8(common + C_CONFIG_GENERATION)
}

// ================================================================================
// KT-246 — DHCP client + ARP + ICMP echo (D2-196 mục (1) bước 3)
//
// **NỢ KT-246 (ghi rõ, không giấu)**: codec Ethernet/IPv4/ICMP/UDP/DHCP dưới đây nằm **tạm** trong
// tệp driver vì crate `vifast-net` (KT-244, sub-agent `netlib`, worktree `/tmp/kt243-s2`) **chưa
// có** trong worktree này lúc làm KT-246 (đã kiểm `ls /tmp/kt243-s2/v2/lib/vifast-net` ⇒ không
// tồn tại). Khi crate ấy land: **xoá cả khối này**, thay bằng `vifast_net::{eth, arp, ipv4, icmp,
// udp, dhcp}` và **chuyển** các ca kiểm tương ứng sang host test của crate đó. Mọi hàm ở đây là
// **thuần** (không `unsafe`, không cấp phát, không đọc trạng thái driver) nên việc chuyển đi là
// máy móc; phần **không** chuyển được là `dhcp_acquire`/`arp_resolve`/`ping` (chúng gọi
// `send_frame`/`recv_frame_wait`).
//
// Bất biến: mọi hàm `*_parse` **từ chối** (trả `None`) thay vì đọc tràn/kẹp im lặng; mọi hàm
// `*_build` trả `None` khi đệm không đủ. Không có đường "đoán" nào.
// ================================================================================

/// Cỡ header Ethernet (6 đích + 6 nguồn + 2 ethertype).
pub const ETH_HDR_BYTES: usize = 14;
/// Ethertype IPv4.
pub const ETHERTYPE_IPV4: u16 = 0x0800;
/// Ethertype ARP.
pub const ETHERTYPE_ARP: u16 = 0x0806;
/// Cỡ header IPv4 tối thiểu (không option).
pub const IPV4_HDR_BYTES: usize = 20;
/// Số hiệu giao thức ICMP trong IPv4.
pub const IP_PROTO_ICMP: u8 = 1;
/// Số hiệu giao thức UDP trong IPv4.
pub const IP_PROTO_UDP: u8 = 17;
/// Cỡ header UDP.
pub const UDP_HDR_BYTES: usize = 8;
/// Cổng DHCP client.
pub const DHCP_CLIENT_PORT: u16 = 68;
/// Cổng DHCP server.
pub const DHCP_SERVER_PORT: u16 = 67;
/// Cỡ phần **cố định** của BOOTP trước magic cookie (op..file).
pub const DHCP_FIXED_BYTES: usize = 236;
/// Magic cookie của DHCP (`99.130.83.99`).
pub const DHCP_MAGIC: [u8; 4] = [99, 130, 83, 99];
/// Option `pad`.
pub const DHCP_OPT_PAD: u8 = 0;
/// Option `end`.
pub const DHCP_OPT_END: u8 = 255;
/// Option `message type`.
pub const DHCP_OPT_MSG_TYPE: u8 = 53;
/// Option `subnet mask`.
pub const DHCP_OPT_SUBNET_MASK: u8 = 1;
/// Option `router`.
pub const DHCP_OPT_ROUTER: u8 = 3;
/// Option `requested ip`.
pub const DHCP_OPT_REQUESTED_IP: u8 = 50;
/// Option `server identifier`.
pub const DHCP_OPT_SERVER_ID: u8 = 54;
/// Option `parameter request list`.
pub const DHCP_OPT_PARAM_LIST: u8 = 55;
/// `DHCPDISCOVER`.
pub const DHCP_DISCOVER: u8 = 1;
/// `DHCPOFFER`.
pub const DHCP_OFFER: u8 = 2;
/// `DHCPREQUEST`.
pub const DHCP_REQUEST: u8 = 3;
/// `DHCPACK`.
pub const DHCP_ACK: u8 = 5;
/// MAC broadcast.
pub const BROADCAST_MAC: [u8; 6] = [0xff; 6];
/// IPv4 broadcast.
pub const BROADCAST_IP: [u8; 4] = [255, 255, 255, 255];
/// IPv4 "chưa có địa chỉ".
pub const ZERO_IP: [u8; 4] = [0, 0, 0, 0];
/// Cỡ payload ICMP echo (byte đệm; đủ để RTT có nghĩa, không cần lớn).
pub const ICMP_PAYLOAD_BYTES: usize = 32;
/// Timeout mặc định cho **mỗi** bước mạng (DHCP/ARP/ping) — 3 s ảo (D2-196 mục (1)).
pub const NET_STEP_TIMEOUT_TICKS: u64 = 3 * TIMEBASE_HZ;

/// Tổng one's-complement 16 bit của `data` (byte lẻ được đệm 0 ở cuối).
pub fn ones_complement_sum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut index = 0;
    while index + 1 < data.len() {
        sum += u16::from_be_bytes([data[index], data[index + 1]]) as u32;
        index += 2;
    }
    if index < data.len() {
        sum += (data[index] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    sum as u16
}

/// Checksum Internet (bù một của [`ones_complement_sum`]).
pub fn checksum(data: &[u8]) -> u16 {
    !ones_complement_sum(data)
}

/// Một frame Ethernet đã giải mã (payload là **lát** của frame gốc, không copy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EthFrame<'a> {
    /// MAC đích.
    pub dst: [u8; 6],
    /// MAC nguồn.
    pub src: [u8; 6],
    /// Ethertype.
    pub ethertype: u16,
    /// Phần sau header.
    pub payload: &'a [u8],
}

/// Dựng frame Ethernet vào `out`; trả tổng số byte. `None` nếu `out` không đủ.
pub fn eth_build(
    dst: [u8; 6],
    src: [u8; 6],
    ethertype: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    let total = ETH_HDR_BYTES + payload.len();
    if out.len() < total {
        return None;
    }
    out[0..6].copy_from_slice(&dst);
    out[6..12].copy_from_slice(&src);
    out[12..14].copy_from_slice(&ethertype.to_be_bytes());
    out[ETH_HDR_BYTES..total].copy_from_slice(payload);
    Some(total)
}

/// Giải mã frame Ethernet. `None` nếu ngắn hơn [`ETH_HDR_BYTES`].
pub fn eth_parse(frame: &[u8]) -> Option<EthFrame<'_>> {
    if frame.len() < ETH_HDR_BYTES {
        return None;
    }
    let mut dst = [0u8; 6];
    let mut src = [0u8; 6];
    dst.copy_from_slice(&frame[0..6]);
    src.copy_from_slice(&frame[6..12]);
    Some(EthFrame {
        dst,
        src,
        ethertype: u16::from_be_bytes([frame[12], frame[13]]),
        payload: &frame[ETH_HDR_BYTES..],
    })
}

/// Một gói IPv4 đã giải mã (payload là lát của gói gốc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4Packet<'a> {
    /// Địa chỉ nguồn.
    pub src: [u8; 4],
    /// Địa chỉ đích.
    pub dst: [u8; 4],
    /// Số hiệu giao thức.
    pub proto: u8,
    /// Phần sau header (đã cắt theo `total_length`).
    pub payload: &'a [u8],
}

/// Dựng gói IPv4 (không option) vào `out`; trả tổng số byte.
///
/// `None` nếu `out` không đủ hoặc tổng vượt MTU 1500 (driver không phân mảnh).
pub fn ipv4_build(
    src: [u8; 4],
    dst: [u8; 4],
    proto: u8,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    let total = IPV4_HDR_BYTES + payload.len();
    if out.len() < total || total > 1500 {
        return None;
    }
    for byte in out[..IPV4_HDR_BYTES].iter_mut() {
        *byte = 0;
    }
    out[0] = 0x45; // version 4, IHL 5 (20 byte)
    out[2..4].copy_from_slice(&(total as u16).to_be_bytes());
    out[6..8].copy_from_slice(&0x4000u16.to_be_bytes()); // DF: không phân mảnh
    out[8] = 64; // TTL
    out[9] = proto;
    out[12..16].copy_from_slice(&src);
    out[16..20].copy_from_slice(&dst);
    let sum = checksum(&out[..IPV4_HDR_BYTES]);
    out[10..12].copy_from_slice(&sum.to_be_bytes());
    out[IPV4_HDR_BYTES..total].copy_from_slice(payload);
    Some(total)
}

/// Giải mã gói IPv4. `None` nếu: ngắn hơn header, version ≠ 4, `IHL` < 5, `total_length` vô lý,
/// hoặc **checksum header sai** (không tin gói hỏng).
pub fn ipv4_parse(packet: &[u8]) -> Option<Ipv4Packet<'_>> {
    if packet.len() < IPV4_HDR_BYTES {
        return None;
    }
    if packet[0] >> 4 != 4 {
        return None;
    }
    let ihl = ((packet[0] & 0x0f) as usize) * 4;
    if ihl < IPV4_HDR_BYTES || packet.len() < ihl {
        return None;
    }
    let total = u16::from_be_bytes([packet[2], packet[3]]) as usize;
    if total < ihl || total > packet.len() {
        return None;
    }
    // Checksum hợp lệ ⇔ bù một của tổng (kể cả trường checksum) bằng 0.
    if checksum(&packet[..ihl]) != 0 {
        return None;
    }
    let mut src = [0u8; 4];
    let mut dst = [0u8; 4];
    src.copy_from_slice(&packet[12..16]);
    dst.copy_from_slice(&packet[16..20]);
    Some(Ipv4Packet {
        src,
        dst,
        proto: packet[9],
        payload: &packet[ihl..total],
    })
}

/// Dựng ICMP echo request (type 8, code 0) vào `out`; trả tổng số byte.
pub fn icmp_build_echo(id: u16, seq: u16, payload: &[u8], out: &mut [u8]) -> Option<usize> {
    let total = 8 + payload.len();
    if out.len() < total {
        return None;
    }
    out[0] = 8; // echo request
    out[1] = 0; // code
    out[4..6].copy_from_slice(&id.to_be_bytes());
    out[6..8].copy_from_slice(&seq.to_be_bytes());
    out[8..total].copy_from_slice(payload);
    let sum = checksum(&out[..total]);
    out[2..4].copy_from_slice(&sum.to_be_bytes());
    Some(total)
}

/// Kiểm một ICMP echo **reply** (type 0, code 0) khớp `id`/`seq`; trả số byte payload.
///
/// `None` nếu: ngắn hơn 8, không phải reply, checksum sai, hoặc `id`/`seq` lệch (reply của lần
/// ping khác — **không** nhận bừa).
pub fn icmp_parse_echo_reply(packet: &[u8], id: u16, seq: u16) -> Option<usize> {
    if packet.len() < 8 {
        return None;
    }
    if packet[0] != 0 || packet[1] != 0 {
        return None;
    }
    if checksum(packet) != 0 {
        return None;
    }
    if u16::from_be_bytes([packet[4], packet[5]]) != id {
        return None;
    }
    if u16::from_be_bytes([packet[6], packet[7]]) != seq {
        return None;
    }
    Some(packet.len() - 8)
}

/// Một datagram UDP đã giải mã.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpDatagram<'a> {
    /// Cổng nguồn.
    pub src_port: u16,
    /// Cổng đích.
    pub dst_port: u16,
    /// Payload (đã cắt theo `length` của UDP).
    pub payload: &'a [u8],
}

/// Dựng datagram UDP vào `out`; trả tổng số byte. Checksum = 0 (IPv4 cho phép — RFC 768).
pub fn udp_build(src_port: u16, dst_port: u16, payload: &[u8], out: &mut [u8]) -> Option<usize> {
    let total = UDP_HDR_BYTES + payload.len();
    if out.len() < total || total > u16::MAX as usize {
        return None;
    }
    out[0..2].copy_from_slice(&src_port.to_be_bytes());
    out[2..4].copy_from_slice(&dst_port.to_be_bytes());
    out[4..6].copy_from_slice(&(total as u16).to_be_bytes());
    out[6..8].copy_from_slice(&0u16.to_be_bytes());
    out[UDP_HDR_BYTES..total].copy_from_slice(payload);
    Some(total)
}

/// Giải mã datagram UDP. `None` nếu ngắn hơn header hoặc `length` vô lý.
pub fn udp_parse(packet: &[u8]) -> Option<UdpDatagram<'_>> {
    if packet.len() < UDP_HDR_BYTES {
        return None;
    }
    let len = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    if len < UDP_HDR_BYTES || len > packet.len() {
        return None;
    }
    Some(UdpDatagram {
        src_port: u16::from_be_bytes([packet[0], packet[1]]),
        dst_port: u16::from_be_bytes([packet[2], packet[3]]),
        payload: &packet[UDP_HDR_BYTES..len],
    })
}

/// Một thông điệp DHCP đã giải mã (chỉ các trường driver dùng).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DhcpMessage {
    /// Loại thông điệp (option 53).
    pub msg_type: u8,
    /// Transaction id.
    pub xid: u32,
    /// `yiaddr` — địa chỉ server cấp cho ta.
    pub yiaddr: [u8; 4],
    /// `siaddr` — địa chỉ next server.
    pub siaddr: [u8; 4],
    /// `chaddr[0..6]` — MAC mà thông điệp nhắm tới.
    pub chaddr: [u8; 6],
    /// Option 1 — mặt nạ mạng.
    pub subnet_mask: Option<[u8; 4]>,
    /// Option 3 — router đầu tiên.
    pub router: Option<[u8; 4]>,
    /// Option 54 — server identifier.
    pub server_id: Option<[u8; 4]>,
}

/// Dựng thông điệp DHCP (BOOTREQUEST) vào `out`; trả tổng số byte.
///
/// `None` nếu `out` không đủ. `requested_ip`/`server_id` chỉ có ở `DHCPREQUEST`.
pub fn dhcp_build(
    msg_type: u8,
    xid: u32,
    mac: [u8; 6],
    requested_ip: Option<[u8; 4]>,
    server_id: Option<[u8; 4]>,
    out: &mut [u8],
) -> Option<usize> {
    // 236 cố định + 4 magic + (3 + 6 + 6 + 6) option + 1 end = 262 tối đa.
    let mut len = DHCP_FIXED_BYTES + 4;
    if out.len() < len + 22 {
        return None;
    }
    for byte in out[..len].iter_mut() {
        *byte = 0;
    }
    out[0] = 1; // BOOTREQUEST
    out[1] = 1; // htype = Ethernet
    out[2] = 6; // hlen
    out[3] = 0; // hops
    out[4..8].copy_from_slice(&xid.to_be_bytes());
    // flags = 0x8000: yêu cầu server **broadcast** reply (ta chưa có IP nên không nhận unicast).
    out[10..12].copy_from_slice(&0x8000u16.to_be_bytes());
    out[28..34].copy_from_slice(&mac); // chaddr
    out[DHCP_FIXED_BYTES..DHCP_FIXED_BYTES + 4].copy_from_slice(&DHCP_MAGIC);
    out[len] = DHCP_OPT_MSG_TYPE;
    out[len + 1] = 1;
    out[len + 2] = msg_type;
    len += 3;
    if let Some(ip) = requested_ip {
        out[len] = DHCP_OPT_REQUESTED_IP;
        out[len + 1] = 4;
        out[len + 2..len + 6].copy_from_slice(&ip);
        len += 6;
    }
    if let Some(id) = server_id {
        out[len] = DHCP_OPT_SERVER_ID;
        out[len + 1] = 4;
        out[len + 2..len + 6].copy_from_slice(&id);
        len += 6;
    }
    // Danh sách tham số muốn server gửi: mặt nạ (1), router (3), DNS (6), domain (15).
    out[len] = DHCP_OPT_PARAM_LIST;
    out[len + 1] = 4;
    out[len + 2] = DHCP_OPT_SUBNET_MASK;
    out[len + 3] = DHCP_OPT_ROUTER;
    out[len + 4] = 6;
    out[len + 5] = 15;
    len += 6;
    out[len] = DHCP_OPT_END;
    len += 1;
    Some(len)
}

/// Giải mã thông điệp DHCP. `None` nếu: ngắn hơn phần cố định + magic, `op` ≠ 2 (BOOTREPLY),
/// magic cookie sai, option bị cắt cụt, hoặc **không** có option 53.
pub fn dhcp_parse(packet: &[u8]) -> Option<DhcpMessage> {
    if packet.len() < DHCP_FIXED_BYTES + 4 {
        return None;
    }
    if packet[0] != 2 {
        return None;
    }
    if packet[DHCP_FIXED_BYTES..DHCP_FIXED_BYTES + 4] != DHCP_MAGIC {
        return None;
    }
    let mut message = DhcpMessage {
        msg_type: 0,
        xid: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
        yiaddr: [packet[16], packet[17], packet[18], packet[19]],
        siaddr: [packet[20], packet[21], packet[22], packet[23]],
        chaddr: [
            packet[28], packet[29], packet[30], packet[31], packet[32], packet[33],
        ],
        subnet_mask: None,
        router: None,
        server_id: None,
    };
    let mut index = DHCP_FIXED_BYTES + 4;
    while index < packet.len() {
        let code = packet[index];
        if code == DHCP_OPT_END {
            break;
        }
        if code == DHCP_OPT_PAD {
            index += 1;
            continue;
        }
        if index + 1 >= packet.len() {
            return None;
        }
        let size = packet[index + 1] as usize;
        if index + 2 + size > packet.len() {
            return None;
        }
        let value = &packet[index + 2..index + 2 + size];
        match code {
            DHCP_OPT_MSG_TYPE if size == 1 => message.msg_type = value[0],
            DHCP_OPT_SUBNET_MASK if size == 4 => {
                message.subnet_mask = Some([value[0], value[1], value[2], value[3]]);
            }
            DHCP_OPT_ROUTER if size >= 4 => {
                message.router = Some([value[0], value[1], value[2], value[3]]);
            }
            DHCP_OPT_SERVER_ID if size == 4 => {
                message.server_id = Some([value[0], value[1], value[2], value[3]]);
            }
            _ => {}
        }
        index += 2 + size;
    }
    if message.msg_type == 0 {
        return None;
    }
    Some(message)
}

/// Dựng **ARP request** (Ethernet/IPv4) hỏi MAC của `target_ip`; trả tổng số byte (42).
pub fn arp_build_request(
    mac: [u8; 6],
    source_ip: [u8; 4],
    target_ip: [u8; 4],
    out: &mut [u8],
) -> Option<usize> {
    let mut arp = [0u8; 28];
    arp[0..2].copy_from_slice(&1u16.to_be_bytes()); // htype = Ethernet
    arp[2..4].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes()); // ptype
    arp[4] = 6; // hlen
    arp[5] = 4; // plen
    arp[6..8].copy_from_slice(&1u16.to_be_bytes()); // oper = request
    arp[8..14].copy_from_slice(&mac); // sha
    arp[14..18].copy_from_slice(&source_ip); // spa
    arp[18..24].copy_from_slice(&[0u8; 6]); // tha (chưa biết)
    arp[24..28].copy_from_slice(&target_ip); // tpa
    eth_build(BROADCAST_MAC, mac, ETHERTYPE_ARP, &arp, out)
}

/// Kiểm một **ARP reply** cho `target_ip` gửi tới `our_ip`/`our_mac`; trả MAC của người trả lời.
///
/// # Bẫy đã tự gặp (KT-246)
///
/// Trong **ARP reply**, `ar_tip` là địa chỉ của **người hỏi** (`our_ip`), **không** phải
/// `target_ip` — `ar_spa` mới là `target_ip`. Bản đầu kiểm `ar_tip == target_ip` nên **mọi** reply
/// hợp lệ đều bị từ chối và ARP timeout dù thiết bị đã trả lời (đo được: `# net arp:
/// elapsed_ms=3000 err=Timeout` trong khi TAP `net rx` thô vẫn nhận đúng frame).
///
/// `None` nếu: không phải ARP/IPv4, `oper` ≠ 2, `spa` ≠ `target_ip`, `tpa` ≠ `our_ip`,
/// `tha` ≠ `our_mac`, hoặc MAC nguồn toàn 0 / broadcast (giá trị vô nghĩa).
pub fn arp_parse_reply(
    frame: &[u8],
    our_ip: [u8; 4],
    target_ip: [u8; 4],
    our_mac: [u8; 6],
) -> Option<[u8; 6]> {
    let eth = eth_parse(frame)?;
    if eth.ethertype != ETHERTYPE_ARP || eth.payload.len() < 28 {
        return None;
    }
    let arp = eth.payload;
    if u16::from_be_bytes([arp[0], arp[1]]) != 1
        || u16::from_be_bytes([arp[2], arp[3]]) != ETHERTYPE_IPV4
        || arp[4] != 6
        || arp[5] != 4
        || u16::from_be_bytes([arp[6], arp[7]]) != 2
    {
        return None;
    }
    let mut sender_mac = [0u8; 6];
    sender_mac.copy_from_slice(&arp[8..14]);
    let sender_ip = [arp[14], arp[15], arp[16], arp[17]];
    let target_mac = [arp[18], arp[19], arp[20], arp[21], arp[22], arp[23]];
    let asked_ip = [arp[24], arp[25], arp[26], arp[27]];
    if sender_ip != target_ip || asked_ip != our_ip || target_mac != our_mac {
        return None;
    }
    if sender_mac == [0u8; 6] || sender_mac == BROADCAST_MAC {
        return None;
    }
    Some(sender_mac)
}

/// Cấu hình IPv4 lấy được qua DHCP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetConfig4 {
    /// Địa chỉ của ta.
    pub ip: [u8; 4],
    /// Gateway (router đầu tiên).
    pub gw: [u8; 4],
    /// Mặt nạ mạng.
    pub mask: [u8; 4],
    /// Server identifier.
    pub server: [u8; 4],
}

/// Dựng frame DHCP hoàn chỉnh (Ethernet + IPv4 + UDP + BOOTP) vào `out`; trả tổng số byte.
fn dhcp_frame(
    msg_type: u8,
    xid: u32,
    mac: [u8; 6],
    requested_ip: Option<[u8; 4]>,
    server_id: Option<[u8; 4]>,
    out: &mut [u8],
) -> Option<usize> {
    let mut bootp = [0u8; 320];
    let bootp_len = dhcp_build(msg_type, xid, mac, requested_ip, server_id, &mut bootp)?;
    let mut udp = [0u8; 340];
    let udp_len = udp_build(
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        &bootp[..bootp_len],
        &mut udp,
    )?;
    let mut ip = [0u8; 360];
    let ip_len = ipv4_build(
        ZERO_IP,
        BROADCAST_IP,
        IP_PROTO_UDP,
        &udp[..udp_len],
        &mut ip,
    )?;
    eth_build(BROADCAST_MAC, mac, ETHERTYPE_IPV4, &ip[..ip_len], out)
}

/// Giải một frame DHCP **khớp** `xid`/`mac`/`want_type`; `None` nếu không khớp (bỏ qua frame khác).
fn dhcp_frame_parse(frame: &[u8], xid: u32, mac: [u8; 6], want_type: u8) -> Option<DhcpMessage> {
    let eth = eth_parse(frame)?;
    if eth.ethertype != ETHERTYPE_IPV4 {
        return None;
    }
    let ip = ipv4_parse(eth.payload)?;
    if ip.proto != IP_PROTO_UDP {
        return None;
    }
    let udp = udp_parse(ip.payload)?;
    if udp.src_port != DHCP_SERVER_PORT || udp.dst_port != DHCP_CLIENT_PORT {
        return None;
    }
    let message = dhcp_parse(udp.payload)?;
    if message.xid != xid || message.chaddr != mac || message.msg_type != want_type {
        return None;
    }
    Some(message)
}

/// Chờ một frame DHCP loại `want_type` khớp `xid`/`mac`, tối đa `timeout_ticks`.
fn dhcp_wait(
    xid: u32,
    mac: [u8; 6],
    want_type: u8,
    timeout_ticks: u64,
) -> Result<DhcpMessage, NetError> {
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = Riscv64Virt.now_ticks();
        if now.wrapping_sub(deadline) < u64::MAX / 2 {
            return Err(NetError::Timeout);
        }
        let remaining = deadline.wrapping_sub(now);
        if let Some(len) = recv_frame_wait(&mut frame, remaining) {
            if let Some(message) = dhcp_frame_parse(&frame[..len], xid, mac, want_type) {
                return Ok(message);
            }
        }
    }
}

/// **DHCP client** đầy đủ: DISCOVER → OFFER → REQUEST → ACK.
///
/// `Err(NetError::Timeout)` nếu không có server trả lời trong `timeout_ticks` **mỗi bước** — đây là
/// đường "không có mạng" (fail-closed, không treo). `Err(NetError::Io)` nếu server trả ACK thiếu
/// mặt nạ/router (không đoán giá trị thiếu).
pub fn dhcp_acquire(timeout_ticks: u64) -> Result<NetConfig4, NetError> {
    let mac = mac();
    // xid **tất định**: K6 chưa có RNG; hằng số vẫn đủ phân biệt reply của ta với frame khác vì
    // còn kiểm `chaddr` = MAC của ta.
    let xid = 0x4b54_3234u32;
    let mut frame = [0u8; 400];
    // ---- DISCOVER
    let len =
        dhcp_frame(DHCP_DISCOVER, xid, mac, None, None, &mut frame).ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len])?;
    let offer = dhcp_wait(xid, mac, DHCP_OFFER, timeout_ticks)?;
    if offer.yiaddr == ZERO_IP {
        return Err(NetError::Io);
    }
    let server = offer.server_id.ok_or(NetError::Io)?;
    // ---- REQUEST
    let len = dhcp_frame(
        DHCP_REQUEST,
        xid,
        mac,
        Some(offer.yiaddr),
        Some(server),
        &mut frame,
    )
    .ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len])?;
    let ack = dhcp_wait(xid, mac, DHCP_ACK, timeout_ticks)?;
    if ack.yiaddr != offer.yiaddr {
        return Err(NetError::Io);
    }
    Ok(NetConfig4 {
        ip: ack.yiaddr,
        gw: ack.router.ok_or(NetError::Io)?,
        mask: ack.subnet_mask.ok_or(NetError::Io)?,
        server: ack.server_id.unwrap_or(server),
    })
}

/// **ARP resolve**: hỏi MAC của `target_ip` (dùng `our_ip`/MAC của ta làm nguồn).
pub fn arp_resolve(
    our_ip: [u8; 4],
    target_ip: [u8; 4],
    timeout_ticks: u64,
) -> Result<[u8; 6], NetError> {
    let our_mac = mac();
    let mut frame = [0u8; 64];
    let len =
        arp_build_request(our_mac, our_ip, target_ip, &mut frame).ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len])?;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let mut reply = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = Riscv64Virt.now_ticks();
        if now.wrapping_sub(deadline) < u64::MAX / 2 {
            return Err(NetError::Timeout);
        }
        let remaining = deadline.wrapping_sub(now);
        if let Some(got) = recv_frame_wait(&mut reply, remaining) {
            if let Some(found) = arp_parse_reply(&reply[..got], our_ip, target_ip, our_mac) {
                return Ok(found);
            }
        }
    }
}

/// Kết quả một loạt ping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PingStats {
    /// Số echo request đã gửi.
    pub sent: u32,
    /// Số echo reply nhận được.
    pub received: u32,
    /// RTT nhỏ nhất (ms).
    pub rtt_min_ms: u64,
    /// RTT lớn nhất (ms).
    pub rtt_max_ms: u64,
}

/// Đổi tick timebase sang ms (làm tròn xuống).
fn ticks_to_ms(ticks: u64) -> u64 {
    ticks.saturating_mul(1000) / TIMEBASE_HZ
}

/// **ICMP echo** `count` lần tới `target_ip` qua `target_mac` (đã resolve bằng ARP).
///
/// `Err(NetError::Timeout)` nếu **không** nhận được reply nào; `Err(NetError::Io)` nếu nhận được
/// ít hơn `count` (mất gói — không giấu).
pub fn ping(
    our_ip: [u8; 4],
    our_mac: [u8; 6],
    target_ip: [u8; 4],
    target_mac: [u8; 6],
    count: u32,
    timeout_ticks: u64,
) -> Result<PingStats, NetError> {
    let mut stats = PingStats::default();
    let mut payload = [0u8; ICMP_PAYLOAD_BYTES];
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let id = 0x4b54u16;
    let mut frame = [0u8; 128];
    let mut reply = [0u8; MAX_FRAME_BYTES];
    let mut seq = 1u16;
    while stats.sent < count {
        let mut icmp = [0u8; 8 + ICMP_PAYLOAD_BYTES];
        let icmp_len = icmp_build_echo(id, seq, &payload, &mut icmp).ok_or(NetError::BadFrame)?;
        let mut ip = [0u8; 64];
        let ip_len = ipv4_build(our_ip, target_ip, IP_PROTO_ICMP, &icmp[..icmp_len], &mut ip)
            .ok_or(NetError::BadFrame)?;
        let len = eth_build(
            target_mac,
            our_mac,
            ETHERTYPE_IPV4,
            &ip[..ip_len],
            &mut frame,
        )
        .ok_or(NetError::BadFrame)?;
        let started = Riscv64Virt.now_ticks();
        send_frame(&frame[..len])?;
        stats.sent += 1;
        let deadline = started.wrapping_add(timeout_ticks);
        let mut answered = false;
        loop {
            let now = Riscv64Virt.now_ticks();
            if now.wrapping_sub(deadline) < u64::MAX / 2 {
                break;
            }
            let remaining = deadline.wrapping_sub(now);
            let Some(got) = recv_frame_wait(&mut reply, remaining) else {
                continue;
            };
            let Some(eth) = eth_parse(&reply[..got]) else {
                continue;
            };
            if eth.ethertype != ETHERTYPE_IPV4 {
                continue;
            }
            let Some(ip) = ipv4_parse(eth.payload) else {
                continue;
            };
            if ip.proto != IP_PROTO_ICMP || ip.src != target_ip {
                continue;
            }
            if icmp_parse_echo_reply(ip.payload, id, seq).is_none() {
                continue;
            }
            let rtt = ticks_to_ms(Riscv64Virt.now_ticks().wrapping_sub(started));
            stats.received += 1;
            if stats.received == 1 || rtt < stats.rtt_min_ms {
                stats.rtt_min_ms = rtt;
            }
            if rtt > stats.rtt_max_ms {
                stats.rtt_max_ms = rtt;
            }
            answered = true;
            break;
        }
        if !answered && stats.received == 0 {
            // Chưa từng có reply nào ⇒ coi như không có mạng (fail-closed, không chờ 3 lần).
            return Err(NetError::Timeout);
        }
        seq = seq.wrapping_add(1);
    }
    if stats.received != count {
        return Err(NetError::Io);
    }
    Ok(stats)
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
    assert!(HDR_MODERN_BYTES <= RX_BUF_BYTES);
};
