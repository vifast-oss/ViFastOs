// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-net** (virtio 1.0, virtio-pci modern) trên QEMU `virt` aarch64 — KT-275
//! (K4' bước 3, D1 trên aarch64).
//!
//! # Đường đi
//!
//! [`crate::pci::init_for`]`(VirtioKind::Net)` (ECAM của KT-273, **không** nhân đôi code PCI) ⇒
//! BAR modern + bốn capability ⇒ tệp này lập trình common config, dựng **hai** split virtqueue
//! (`rx` = queue 0, `tx` = queue 1 — Virtio 1.2 §5.1.2), đọc **MAC thật** từ
//! `virtio_net_config`, rồi `send_frame`/`recv_frame_wait` và **poll `used`** (chưa dùng ngắt).
//!
//! # Khác khuôn riscv64/x86_64 ở đúng ba điểm (đều do kiến trúc, không do phong cách)
//!
//! 1. **Địa chỉ DMA là PA, không phải VA**: sau `activate`, hart 0 chạy ở **alias nửa cao**
//!    (`KERNEL_HIGH_BASE + …`), nên `addr_of!(trang DMA)` **không** phải PA. Thiết bị DMA bằng
//!    PA ⇒ caller đổi bằng [`high_to_pa`] rồi truyền vào [`init`] (cùng khuôn `virtio_blk` KT-273).
//! 2. **Trạng thái driver bằng `Atomic*`, không `UnsafeCell`**: crate này `#![deny(unsafe_code)]`
//!    ngoài [`crate::asm`] (D2-17), nên không có `unsafe impl Sync`/`&mut *cell`. Ảnh boot chạy
//!    **một** hart trên đường driver và driver tự tuần tự hoá (một frame một lúc mỗi hướng) ⇒
//!    `load`/`store(Relaxed)` là đủ, đúng khuôn `virtio_blk.rs` (KT-273).
//! 3. **Trần thời gian bằng `CNTPCT_EL0`** (bộ đếm vật lý, luôn có) thay vì timebase của
//!    `Platform`: bước 3 **chưa** có timer per-hart (nợ §7.1 của `k4-aarch64-boot.md`), nhưng bộ
//!    đếm đọc được không cần cấu hình. `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ [`now_us`] trả **tick**
//!    thay vì µs, ghi rõ: ngân sách 3 s khi ấy là 3 M tick (ngắn hơn thật) ⇒ **fail-closed**
//!    (timeout, không treo), không bao giờ dài hơn ngân sách.
//!
//! # Bất biến (giữ nguyên từ khuôn riscv64 KT-245 / x86_64 KT-274)
//!
//! - Feature đàm phán: `VIRTIO_F_VERSION_1` (bắt buộc) + `VIRTIO_NET_F_MAC` **nếu** thiết bị có.
//!   **Không** `MRG_RXBUF`, **không** `STATUS` — quyết định nằm ở `vifast_virtio::net`
//!   (`negotiated_features`, test host), không viết tay hai chỗ (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **`dsb sy`** → ghi `avail->idx` →
//!   **`dsb sy`** → notify (MMIO) → poll `used->idx` → **`dsb sy`** → đọc `used->elem`.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - Một frame một lúc mỗi hướng (queue depth 1 thực tế); đủ cho DHCP/ARP/ICMP và dễ chứng minh.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); frame sai độ dài ⇒
//!   `BadFrame` **trước** khi chạm ring; `used->len` vô lý ⇒ `Io` + đếm `errors`.
//! - `rx` arm **trước** (một descriptor device-writable [`RX_BUF_BYTES`]) để frame host gửi tới
//!   không bị rơi; `recv_frame_wait` xong arm lại ngay.
//!
//! # Codec: **không** viết lại
//!
//! Toàn bộ encode/decode Ethernet/ARP/IPv4/ICMP/UDP/DHCP nằm ở crate thuần `vifast-net`
//! (KT-244, host test). Tệp này chỉ còn **chính sách** client (gửi gì, chờ gì, timeout bao lâu)
//! và phần chạm thanh ghi — cùng cách khuôn riscv64 KT-246b đã de-dup.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::arp::{ArpPacket, ARP_LEN};
use vifast_net::dhcp::{DhcpMessage, DHCPACK, DHCPOFFER};
use vifast_net::dns::{ARecord, DnsMessage, DnsQuery, DnsQuestion, TYPE_A};
use vifast_net::eth::{EthHeader, ETHERTYPE_ARP, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp::{IcmpEcho, IcmpMessage};
use vifast_net::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_ICMP, PROTO_UDP};
use vifast_net::udp::{UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, UDP_HEADER_LEN};
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
pub const RX_RING_OFF: usize = 0x000;
/// Offset ring `tx` trong trang DMA.
pub const TX_RING_OFF: usize = 0x200;
/// Offset đệm `rx` (device-writable: header + frame).
pub const RX_BUF_OFF: usize = 0x400;
/// Offset đệm `tx` (driver-writable: header + frame).
pub const TX_BUF_OFF: usize = 0xA00;
/// Timeout poll `used` mặc định — 2 s (cùng ngân sách khuôn riscv64 KT-72/73/245).
pub const POLL_TIMEOUT_US: u64 = 2_000_000;
/// Timeout mặc định cho **một bước** mạng (DHCP/ARP/ping) — 3 s (D2-196 mục (1)).
pub const NET_STEP_TIMEOUT_US: u64 = 3_000_000;
/// Cỡ payload ICMP echo (byte đệm; đủ để RTT có nghĩa, không cần lớn).
pub const ICMP_PAYLOAD_BYTES: usize = 32;

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

// ---------------------------------------------------------------- trạng thái (atomic)
static READY: AtomicBool = AtomicBool::new(false);
static DMA_PA: AtomicU64 = AtomicU64::new(0);
static COMMON: AtomicU64 = AtomicU64::new(0);
static NOTIFY: AtomicU64 = AtomicU64::new(0);
static DEVICE_CFG: AtomicU64 = AtomicU64::new(0);
static ISR: AtomicU64 = AtomicU64::new(0);
static NOTIFY_MUL: AtomicU64 = AtomicU64::new(0);
static NOTIFY_OFF_RX: AtomicU64 = AtomicU64::new(0);
static NOTIFY_OFF_TX: AtomicU64 = AtomicU64::new(0);
static QUEUE_SIZE_RX: AtomicU64 = AtomicU64::new(0);
static QUEUE_SIZE_TX: AtomicU64 = AtomicU64::new(0);
/// MAC đọc từ `virtio_net_config`, đóng gói 6 byte vào 48 bit thấp.
static MAC: AtomicU64 = AtomicU64::new(0);
static RX_LAST_USED: AtomicU64 = AtomicU64::new(0);
static TX_LAST_USED: AtomicU64 = AtomicU64::new(0);

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

// ---------------------------------------------------------------- MMIO (qua `asm`)
#[inline]
fn read8(addr: u64) -> u8 {
    asm::mmio_read_u8(addr)
}
#[inline]
fn read16(addr: u64) -> u16 {
    asm::mmio_read_u16(addr)
}
#[inline]
fn read32(addr: u64) -> u32 {
    asm::mmio_read_u32(addr)
}
#[inline]
fn write8(addr: u64, value: u8) {
    asm::mmio_write_u8(addr, value);
}
#[inline]
fn write16(addr: u64, value: u16) {
    asm::mmio_write_u16(addr, value);
}
#[inline]
fn write32(addr: u64, value: u32) {
    asm::mmio_write_u32(addr, value);
}
#[inline]
fn write64(addr: u64, value: u64) {
    asm::mmio_write_u64(addr, value);
}

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13: driver phải
/// có barrier trước notify; thiếu ⇒ thiết bị có thể đọc `avail` cũ).
///
/// `dsb sy` (không phải `dmb`): DMA của thiết bị nằm **ngoài** domain shareability của CPU nên
/// cần barrier **đầy đủ** — cùng lý do `fence rw, rw` bên riscv64.
#[inline]
fn fence() {
    asm::dsb_sy();
}

/// Trang DMA của virtio-net (4 KiB căn lề) — vùng ring + đệm `rx`/`tx`.
fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    asm::dma_page_net()
}

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết (cùng công thức `virtio_blk` KT-273).
///
/// Sau khi nhảy nửa cao, `adrp` cho alias `KERNEL_HIGH_BASE + (pa - text_start)`; PA thật là
/// `text_start_pa + (va - KERNEL_HIGH_BASE)`. Chỉ đúng cho địa chỉ **trong ảnh** — trang DMA
/// nằm trong `.bss` nên thuộc trường hợp này.
pub const fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - crate::split_mmu::KERNEL_HIGH_BASE)
}

/// Bộ đếm vật lý hiện tại (`CNTPCT_EL0`).
pub fn now_ticks() -> u64 {
    asm::read_cntpct_el0()
}

/// Đồng hồ **µs** của driver — `CNTPCT_EL0 / CNTFRQ_EL0`.
///
/// `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ trả **tick** thay vì µs: ngân sách 3 s khi ấy là 3 M tick,
/// tức **ngắn hơn** 3 s thật ⇒ fail-closed (timeout), không bao giờ dài hơn ngân sách. QEMU
/// `virt` đặt `CNTFRQ_EL0 = 62.5 MHz` nên đường đo thật luôn dùng nhánh chia.
pub fn now_us() -> u64 {
    let freq = asm::read_cntfrq_el0();
    let ticks = now_ticks();
    // `checked_div` thay vì `if freq == 0`: cùng ngữ nghĩa, không có phép chia nào bị bỏ kiểm.
    match ticks.saturating_mul(1_000_000).checked_div(freq) {
        Some(us) => us,
        // `freq == 0`: trả **tick** (ngân sách ngắn hơn thật ⇒ fail-closed, không treo).
        None => ticks,
    }
}

/// `true` ⇔ `now` **đã** qua `deadline` (so sánh an toàn với wrap của bộ đếm).
///
/// `pub` từ KT-289: `tcp.rs` dùng **cùng** phép so sánh — không có bản thứ hai lệch ngữ nghĩa.
pub fn past_deadline(now: u64, deadline: u64) -> bool {
    now.wrapping_sub(deadline) < u64::MAX / 2
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
    /// Địa chỉ BAR driver **gán** (PA = VA, identity).
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
    /// PA trang DMA (thiết bị DMA vào đây).
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

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
///
/// `dma_pa` là **PA** của trang DMA (caller đổi alias nửa cao → PA bằng [`high_to_pa`]); phải
/// căn 4 KiB vì descriptor/`avail`/`used` đều suy từ nó bằng phép cộng offset.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<NetReport, &'static str> {
    if dma_pa & 0xfff != 0 {
        return Err("dma_align");
    }
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
        let base = dma_pa + ring_off[slot] as u64;
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
    // ---- 9) lưu trạng thái (khối riêng: không giữ `&mut` qua lời gọi `arm_rx`)
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    COMMON.store(common, Ordering::Relaxed);
    NOTIFY.store(notify, Ordering::Relaxed);
    DEVICE_CFG.store(device_cfg, Ordering::Relaxed);
    ISR.store(isr, Ordering::Relaxed);
    NOTIFY_MUL.store(notify_mul as u64, Ordering::Relaxed);
    NOTIFY_OFF_RX.store(notify_off[0] as u64, Ordering::Relaxed);
    NOTIFY_OFF_TX.store(notify_off[1] as u64, Ordering::Relaxed);
    QUEUE_SIZE_RX.store(queue_size[0] as u64, Ordering::Relaxed);
    QUEUE_SIZE_TX.store(queue_size[1] as u64, Ordering::Relaxed);
    MAC.store(pack_mac(config.mac), Ordering::Relaxed);
    RX_LAST_USED.store(0, Ordering::Relaxed);
    TX_LAST_USED.store(0, Ordering::Relaxed);
    READY.store(true, Ordering::Relaxed);
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

/// Đóng gói 6 byte MAC vào 48 bit thấp của `u64` (trạng thái driver là `AtomicU64`).
const fn pack_mac(mac: [u8; 6]) -> u64 {
    (mac[0] as u64)
        | ((mac[1] as u64) << 8)
        | ((mac[2] as u64) << 16)
        | ((mac[3] as u64) << 24)
        | ((mac[4] as u64) << 32)
        | ((mac[5] as u64) << 40)
}

/// Giải 48 bit thấp của `u64` về 6 byte MAC.
const fn unpack_mac(packed: u64) -> [u8; 6] {
    [
        (packed & 0xff) as u8,
        ((packed >> 8) & 0xff) as u8,
        ((packed >> 16) & 0xff) as u8,
        ((packed >> 24) & 0xff) as u8,
        ((packed >> 32) & 0xff) as u8,
        ((packed >> 40) & 0xff) as u8,
    ]
}

/// Bọc ring của queue `slot` (0 = rx, 1 = tx) thành split queue với cỡ **thật** của nó.
///
/// Nhận `size` từ người gọi (không tự đọc trạng thái) để không có hai `&mut` sống cùng lúc.
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
    let size = QUEUE_SIZE_RX.load(Ordering::Relaxed) as u16;
    let notify = NOTIFY.load(Ordering::Relaxed);
    let notify_off = NOTIFY_OFF_RX.load(Ordering::Relaxed) as u16;
    let notify_mul = NOTIFY_MUL.load(Ordering::Relaxed) as u32;
    let dma = DMA_PA.load(Ordering::Relaxed);
    if size < MIN_QUEUE_SIZE || dma == 0 {
        return Err("arm");
    }
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
/// bị không hoàn tất trong `timeout_us`.
pub fn send_frame(frame: &[u8], timeout_us: u64) -> Result<(), NetError> {
    if !READY.load(Ordering::Relaxed) {
        return Err(NetError::NotReady);
    }
    // Kiểm **trước** khi chạm ring: frame sai độ dài không được đưa vào descriptor.
    if !frame_ok(frame) {
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err(NetError::BadFrame);
    }
    // Driver chỉ chấp nhận **modern** (`VIRTIO_F_VERSION_1` bắt buộc ở `init`) ⇒ header 12 byte.
    let hdr_len = hdr_bytes(true);
    let total = hdr_len + frame.len();
    let size = QUEUE_SIZE_TX.load(Ordering::Relaxed) as u16;
    let notify = NOTIFY.load(Ordering::Relaxed);
    let notify_off = NOTIFY_OFF_TX.load(Ordering::Relaxed) as u16;
    let notify_mul = NOTIFY_MUL.load(Ordering::Relaxed) as u32;
    let dma = DMA_PA.load(Ordering::Relaxed);
    if size < MIN_QUEUE_SIZE || dma == 0 {
        return Err(NetError::NotReady);
    }
    let tx_pa = dma + TX_BUF_OFF as u64;
    {
        let buf = &mut page()[TX_BUF_OFF..TX_BUF_OFF + total];
        // Header rỗng: không checksum offload, không GSO (driver không đàm phán feature nào của
        // hai đường đó).
        buf[..hdr_len].copy_from_slice(&NetHdr::zero().encode(true)[..hdr_len]);
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
    let layout = VqLayout::new(size).ok_or(NetError::NotReady)?;
    let used_idx_addr = dma + TX_RING_OFF as u64 + layout.used_idx_off as u64;
    let deadline = now_us().saturating_add(timeout_us);
    loop {
        // `asm::mmio_read_u16` là đọc `volatile` — compiler **không** hoist khỏi vòng lặp (bẫy
        // kinh điển của poll DMA).
        let idx = read16(used_idx_addr);
        let last = TX_LAST_USED.load(Ordering::Relaxed) as u16;
        if idx != last {
            fence();
            let element = queue_with(1, size)?.used_elem(last & (size - 1));
            TX_LAST_USED.store(last.wrapping_add(1) as u64, Ordering::Relaxed);
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
        if past_deadline(now_us(), deadline) {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            ERRORS.fetch_add(1, Ordering::SeqCst);
            return Err(NetError::Timeout);
        }
        core::hint::spin_loop();
    }
}

/// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ).
pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    recv_frame_wait(out, 0)
}

/// Nhận **một** frame, chờ tối đa `timeout_us`. `None` = hết hạn / chưa có.
///
/// `used->len` vô lý (nhỏ hơn header, lớn hơn đệm) ⇒ đếm `errors`, **không** trả dữ liệu rác;
/// descriptor được arm lại để lần sau vẫn nhận được.
pub fn recv_frame_wait(out: &mut [u8], timeout_us: u64) -> Option<usize> {
    if !READY.load(Ordering::Relaxed) {
        return None;
    }
    let size = QUEUE_SIZE_RX.load(Ordering::Relaxed) as u16;
    let dma = DMA_PA.load(Ordering::Relaxed);
    if size < MIN_QUEUE_SIZE || dma == 0 {
        return None;
    }
    let layout = VqLayout::new(size)?;
    let used_idx_addr = dma + RX_RING_OFF as u64 + layout.used_idx_off as u64;
    let deadline = now_us().saturating_add(timeout_us);
    loop {
        let idx = read16(used_idx_addr);
        let last = RX_LAST_USED.load(Ordering::Relaxed) as u16;
        if idx != last {
            fence();
            let element = queue_with(0, size).ok()?.used_elem(last & (size - 1));
            RX_LAST_USED.store(last.wrapping_add(1) as u64, Ordering::Relaxed);
            if element.0 != 0 {
                ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
                ERRORS.fetch_add(1, Ordering::SeqCst);
                let _ = arm_rx();
                return None;
            }
            let frame = match frame_len(element.1, true) {
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
            let hdr_len = hdr_bytes(true);
            let src = &page()[RX_BUF_OFF + hdr_len..RX_BUF_OFF + hdr_len + frame];
            out[..frame].copy_from_slice(src);
            RX_FRAMES.fetch_add(1, Ordering::SeqCst);
            RX_BYTES.fetch_add(frame as u64, Ordering::SeqCst);
            // Arm lại **trước** khi trả: cửa sổ không có đệm là nhỏ nhất.
            let _ = arm_rx();
            return Some(frame);
        }
        if past_deadline(now_us(), deadline) {
            return None;
        }
        core::hint::spin_loop();
    }
}

/// MAC đọc từ thiết bị (toàn 0 nếu chưa init).
pub fn mac() -> [u8; 6] {
    unpack_mac(MAC.load(Ordering::Relaxed))
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

/// Địa chỉ ISR (đọc 1 byte để lấy lý do ngắt — bước này chưa dùng ngắt, chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    ISR.load(Ordering::Relaxed)
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều frame).
pub fn device_status() -> u8 {
    let common = COMMON.load(Ordering::Relaxed);
    if common == 0 {
        return 0;
    }
    read8(common + C_DEVICE_STATUS)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    let common = COMMON.load(Ordering::Relaxed);
    if common == 0 {
        return 0;
    }
    read8(common + C_CONFIG_GENERATION)
}

/// Địa chỉ `device_cfg` (chẩn đoán).
pub fn device_cfg_addr() -> u64 {
    DEVICE_CFG.load(Ordering::Relaxed)
}

// ================================================================================
// KT-275 — DHCP client + ARP + ICMP echo (D1 trên aarch64)
//
// Codec **không** nằm ở đây: Ethernet/ARP/IPv4/ICMP/UDP/DHCP là của `vifast-net` (KT-244, host
// test). Tệp này chỉ còn **chính sách** client (gửi gì, chờ gì, timeout bao lâu) — chính sách
// cần `send_frame`/`recv_frame_wait` nên không chuyển sang crate thuần được.
//
// Bất biến: mỗi bước (DHCP/ARP/ping) có timeout riêng [`NET_STEP_TIMEOUT_US`] = 3 s; thiếu mạng
// ⇒ `Err(NetError::Timeout)` (fail-closed, không treo); server trả ACK thiếu mặt nạ/router ⇒
// `Err(NetError::Io)` (không đoán giá trị thiếu).
// ================================================================================

/// Cấu hình IPv4 lấy được qua DHCP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetConfig4 {
    /// Địa chỉ của ta.
    pub ip: Ipv4Addr,
    /// Gateway (router đầu tiên).
    pub gw: Ipv4Addr,
    /// Mặt nạ mạng.
    pub mask: Ipv4Addr,
    /// Server identifier.
    pub server: Ipv4Addr,
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

/// Đổi µs sang ms (làm tròn xuống).
fn us_to_ms(us: u64) -> u64 {
    us / 1000
}

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
/// `target_ip` — `ar_spa` mới là `target_ip`. Bản đầu kiểm `ar_tip == target_ip` nên **mọi**
/// reply hợp lệ đều bị từ chối và ARP timeout dù thiết bị đã trả lời.
pub fn arp_parse_reply(
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

/// Dựng frame DHCP hoàn chỉnh (Ethernet + IPv4 + UDP + BOOTP) vào `out`; trả tổng số byte.
pub fn dhcp_frame(message: &DhcpMessage, mac: MacAddr, out: &mut [u8]) -> Option<usize> {
    let mut udp = [0u8; 640];
    let udp_len = UdpHeader::encode(
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        message.as_bytes(),
        &mut udp,
    )
    .ok()?;
    let mut ip = [0u8; 680];
    let header_len = Ipv4Header::new(
        PROTO_UDP,
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        udp_len,
    )
    .encode(&mut ip)
    .ok()?;
    ip[header_len..header_len + udp_len].copy_from_slice(&udp[..udp_len]);
    let ip_len = header_len + udp_len;
    EthernetFrame {
        header: EthHeader::new(MacAddr::BROADCAST, mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(out)
    .ok()
}

/// Giải một frame DHCP **khớp** `xid`/`mac`/`want_type`; `None` nếu không khớp (bỏ qua frame khác).
pub fn dhcp_frame_parse(
    frame: &[u8],
    xid: u32,
    mac: MacAddr,
    want_type: u8,
) -> Option<DhcpMessage> {
    let eth = EthernetFrame::parse(frame).ok()?;
    if eth.header.ethertype != ETHERTYPE_IPV4 {
        return None;
    }
    let ip = Ipv4Header::parse(eth.payload, true).ok()?;
    if ip.header.protocol != PROTO_UDP {
        return None;
    }
    let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true).ok()?;
    if udp.header.src_port != DHCP_SERVER_PORT || udp.header.dst_port != DHCP_CLIENT_PORT {
        return None;
    }
    let message = DhcpMessage::parse(udp.payload()).ok()?;
    if message.xid() != xid || message.chaddr() != mac || message.msg_type() != Some(want_type) {
        return None;
    }
    Some(message)
}

/// Chờ một frame DHCP loại `want_type` khớp `xid`/`mac`, tối đa `timeout_us`.
fn dhcp_wait(
    xid: u32,
    mac: MacAddr,
    want_type: u8,
    timeout_us: u64,
) -> Result<DhcpMessage, NetError> {
    let deadline = now_us().saturating_add(timeout_us);
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = now_us();
        if past_deadline(now, deadline) {
            return Err(NetError::Timeout);
        }
        let remaining = deadline.saturating_sub(now);
        if let Some(len) = recv_frame_wait(&mut frame, remaining) {
            if let Some(message) = dhcp_frame_parse(&frame[..len], xid, mac, want_type) {
                return Ok(message);
            }
        }
    }
}

/// **DHCP client** đầy đủ: DISCOVER → OFFER → REQUEST → ACK.
///
/// `Err(NetError::Timeout)` nếu không có server trả lời trong `timeout_us` **mỗi bước** — đây là
/// đường "không có mạng" (fail-closed, không treo). `Err(NetError::Io)` nếu server trả ACK thiếu
/// mặt nạ/router (không đoán giá trị thiếu).
pub fn dhcp_acquire(timeout_us: u64) -> Result<NetConfig4, NetError> {
    let mac = MacAddr::new(mac());
    // xid **tất định**: bước này chưa có RNG; hằng số vẫn đủ phân biệt reply của ta với frame
    // khác vì còn kiểm `chaddr` = MAC của ta.
    let xid = 0x4b54_3235u32;
    let mut frame = [0u8; 720];
    // ---- DISCOVER
    let discover = DhcpMessage::discover(xid, mac);
    let len = dhcp_frame(&discover, mac, &mut frame).ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len], POLL_TIMEOUT_US)?;
    let offer = dhcp_wait(xid, mac, DHCPOFFER, timeout_us)?;
    if offer.yiaddr().is_unspecified() {
        return Err(NetError::Io);
    }
    let server = offer.server_id().ok_or(NetError::Io)?;
    // ---- REQUEST
    let request = DhcpMessage::request(xid, mac, offer.yiaddr(), server);
    let len = dhcp_frame(&request, mac, &mut frame).ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len], POLL_TIMEOUT_US)?;
    let ack = dhcp_wait(xid, mac, DHCPACK, timeout_us)?;
    if ack.yiaddr() != offer.yiaddr() {
        return Err(NetError::Io);
    }
    Ok(NetConfig4 {
        ip: ack.yiaddr(),
        gw: ack.router().ok_or(NetError::Io)?,
        mask: ack.subnet_mask().ok_or(NetError::Io)?,
        server: ack.server_id().unwrap_or(server),
    })
}

/// **ARP resolve**: hỏi MAC của `target_ip` (dùng `our_ip`/MAC của ta làm nguồn).
///
/// `Err(NetError::Timeout)` nếu không có reply trong `timeout_us` — đường "không có mạng"
/// (fail-closed, không treo).
pub fn arp_resolve(
    our_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    timeout_us: u64,
) -> Result<MacAddr, NetError> {
    let our_mac = MacAddr::new(mac());
    let mut frame = [0u8; 64];
    let len =
        arp_build_request(our_mac, our_ip, target_ip, &mut frame).ok_or(NetError::BadFrame)?;
    send_frame(&frame[..len], POLL_TIMEOUT_US)?;
    let deadline = now_us().saturating_add(timeout_us);
    let mut reply = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = now_us();
        if past_deadline(now, deadline) {
            return Err(NetError::Timeout);
        }
        let remaining = deadline.saturating_sub(now);
        if let Some(got) = recv_frame_wait(&mut reply, remaining) {
            if let Some(found) = arp_parse_reply(&reply[..got], our_ip, target_ip, our_mac) {
                return Ok(found);
            }
        }
    }
}

/// Dựng **một** frame ICMP echo request (Ethernet + IPv4 + ICMP) vào `out`; trả tổng số byte.
///
/// Tách khỏi [`ping`] để **test host** kiểm được khung thật (checksum, độ dài, ethertype) mà
/// không cần thiết bị — cùng lý do `arp_build_request` là hàm thuần.
#[allow(clippy::too_many_arguments)]
pub fn icmp_echo_frame(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    target_mac: MacAddr,
    id: u16,
    seq: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    let mut icmp = [0u8; 8 + ICMP_PAYLOAD_BYTES];
    let icmp_len = IcmpEcho::request(id, seq, payload).encode(&mut icmp).ok()?;
    let mut ip = [0u8; 64];
    let header_len = Ipv4Header::new(PROTO_ICMP, our_ip, target_ip, icmp_len)
        .encode(&mut ip)
        .ok()?;
    ip[header_len..header_len + icmp_len].copy_from_slice(&icmp[..icmp_len]);
    let ip_len = header_len + icmp_len;
    EthernetFrame {
        header: EthHeader::new(target_mac, our_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(out)
    .ok()
}

/// **ICMP echo** `count` lần tới `target_ip` qua `target_mac` (đã resolve bằng ARP).
///
/// `Err(NetError::Timeout)` nếu **không** nhận được reply nào; `Err(NetError::Io)` nếu nhận được
/// ít hơn `count` (mất gói — không giấu).
pub fn ping(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    target_mac: MacAddr,
    count: u32,
    timeout_us: u64,
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
        let len = icmp_echo_frame(
            our_ip, our_mac, target_ip, target_mac, id, seq, &payload, &mut frame,
        )
        .ok_or(NetError::BadFrame)?;
        let started = now_us();
        send_frame(&frame[..len], POLL_TIMEOUT_US)?;
        stats.sent += 1;
        let deadline = started.saturating_add(timeout_us);
        let mut answered = false;
        loop {
            let now = now_us();
            if past_deadline(now, deadline) {
                break;
            }
            let remaining = deadline.saturating_sub(now);
            let Some(got) = recv_frame_wait(&mut reply, remaining) else {
                continue;
            };
            let Ok(eth) = EthernetFrame::parse(&reply[..got]) else {
                continue;
            };
            if eth.header.ethertype != ETHERTYPE_IPV4 {
                continue;
            }
            let Ok(ip) = Ipv4Header::parse(eth.payload, true) else {
                continue;
            };
            if ip.header.protocol != PROTO_ICMP || ip.header.src != target_ip {
                continue;
            }
            // `IcmpEcho::parse` kiểm checksum trên **toàn** lát byte đưa vào — `ip.payload()` đã
            // được cắt đúng `total length` nên không lẫn padding Ethernet.
            match IcmpEcho::parse(ip.payload()) {
                Ok(IcmpMessage::EchoReply(echo)) if echo.id == id && echo.seq == seq => {}
                _ => continue,
            }
            let rtt = us_to_ms(now_us().saturating_sub(started));
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

// ================================================================================
// KT-289 — UDP + DNS query (N1 phần đầu trên aarch64)
//
// Cùng khuôn `vifast-plat-riscv64/src/virtio_net.rs` (KT-260): codec DNS nằm ở `vifast_net::dns`
// (host test, gồm ca âm), tệp này chỉ còn **chính sách** client (gửi gì, chờ gì, timeout bao
// lâu). Khác biệt **chỉ** ở đồng hồ: aarch64 dùng [`now_us`] (`CNTPCT_EL0 / CNTFRQ_EL0`).
//
// **Vì sao UDP echo dùng DNS làm "gương"**: libslirp **không** có echo service UDP ở
// `10.0.2.2:7` — `udp_input` chỉ xử lý BOOTP (67) và TFTP (69) rồi chuyển mọi datagram khác cho
// socket của **host**; `sotranslate_out4` ánh xạ `10.0.2.2` → loopback host, nên `10.0.2.2:7`
// chỉ tới `127.0.0.1:7` (không có gì lắng nghe). Vì vậy, theo fallback của KT-260, bằng chứng UDP
// hai chiều lấy từ **reply DNS**: RFC 1035 §4.1.2 buộc server **chép nguyên** question section
// vào response, nên gửi QNAME = `vifast-udp-echo` rồi đọc lại đúng chuỗi đó trong reply là một
// vòng UDP **gửi → nhận → đúng payload**. `udp_echo` tới `10.0.2.2:7` vẫn được hiện thực đầy đủ
// (dùng ở ca âm timeout) để hành vi fail-closed 3 s có bằng chứng đo được.
// ================================================================================

/// Cổng UDP của slirp echo service (`10.0.2.2:7`) — dữ kiện nền tảng QEMU.
pub const UDP_ECHO_PORT: u16 = 7;
/// Cổng UDP của DNS (`10.0.2.3:53`).
pub const DNS_PORT: u16 = 53;
/// Địa chỉ DNS server của slirp (`-netdev user`): `10.0.2.3` (dữ kiện nền tảng QEMU).
pub const DNS_SERVER_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 3);
/// Cổng nguồn UDP cố định của client (không có RNG; còn kiểm địa chỉ/cổng đích nên đủ phân biệt).
pub const UDP_CLIENT_PORT: u16 = 0x4b54;
/// Payload UDP echo theo KT-260.
pub const UDP_ECHO_PAYLOAD: &[u8] = b"vifast-udp-echo";
/// Cỡ payload UDP tối đa (DNS không EDNS: 512 byte, RFC 1035 §4.2.1).
pub const UDP_MAX_PAYLOAD: usize = 512;

/// Kết quả một lần UDP echo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UdpEcho {
    /// Số byte payload nhận lại (bằng payload đã gửi).
    pub len: usize,
    /// RTT (ms).
    pub rtt_ms: u64,
}

/// Kết quả một truy vấn DNS `A`.
#[derive(Clone, Copy, Debug)]
pub struct DnsAnswer {
    /// `id` của query.
    pub id: u16,
    /// `rcode` của reply.
    pub rcode: u8,
    /// Câu hỏi trong reply (server **chép nguyên** question section — RFC 1035 §4.1.2).
    pub question: DnsQuestion,
    /// Bản ghi `A` đầu tiên; `None` nếu reply không có `A` (ví dụ NODATA/NXDOMAIN).
    pub a: Option<ARecord>,
    /// RTT (ms).
    pub rtt_ms: u64,
    /// Độ dài message reply (byte).
    pub reply_len: usize,
}

/// Gửi **một** datagram UDP (Ethernet + IPv4 + UDP) tới `dst_ip:dst_port`.
pub fn udp_send(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<(), NetError> {
    if payload.len() > UDP_MAX_PAYLOAD {
        return Err(NetError::BadFrame);
    }
    let mut udp = [0u8; UDP_HEADER_LEN + UDP_MAX_PAYLOAD];
    let udp_len = UdpHeader::encode(our_ip, dst_ip, src_port, dst_port, payload, &mut udp)
        .map_err(|_| NetError::BadFrame)?;
    let mut ip = [0u8; IPV4_MIN_HEADER_LEN + UDP_HEADER_LEN + UDP_MAX_PAYLOAD];
    let header_len = Ipv4Header::new(PROTO_UDP, our_ip, dst_ip, udp_len)
        .encode(&mut ip)
        .map_err(|_| NetError::BadFrame)?;
    ip[header_len..header_len + udp_len].copy_from_slice(&udp[..udp_len]);
    let ip_len = header_len + udp_len;
    let mut frame = [0u8; ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + UDP_HEADER_LEN + UDP_MAX_PAYLOAD];
    let len = EthernetFrame {
        header: EthHeader::new(dst_mac, our_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(&mut frame)
    .map_err(|_| NetError::BadFrame)?;
    send_frame(&frame[..len], POLL_TIMEOUT_US)
}

/// Chờ **một** datagram UDP khớp `src_ip:src_port` → `our_ip:dst_port`, tối đa `timeout_us`.
///
/// Trả độ dài payload và copy payload vào `out`. Frame không khớp (ARP, ICMP, cổng khác) bị bỏ
/// qua — cùng cách `dhcp_wait`/`ping` đang làm. Hết hạn ⇒ [`NetError::Timeout`] (fail-closed).
pub fn udp_recv(
    our_ip: Ipv4Addr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    out: &mut [u8],
    timeout_us: u64,
) -> Result<usize, NetError> {
    let deadline = now_us().saturating_add(timeout_us);
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = now_us();
        if past_deadline(now, deadline) {
            return Err(NetError::Timeout);
        }
        let remaining = deadline.saturating_sub(now);
        let Some(got) = recv_frame_wait(&mut frame, remaining) else {
            continue;
        };
        let Ok(eth) = EthernetFrame::parse(&frame[..got]) else {
            continue;
        };
        if eth.header.ethertype != ETHERTYPE_IPV4 {
            continue;
        }
        let Ok(ip) = Ipv4Header::parse(eth.payload, true) else {
            continue;
        };
        if ip.header.protocol != PROTO_UDP || ip.header.src != src_ip || ip.header.dst != our_ip {
            continue;
        }
        let Ok(udp) = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true) else {
            continue;
        };
        if udp.header.src_port != src_port || udp.header.dst_port != dst_port {
            continue;
        }
        let payload = udp.payload();
        if payload.len() > out.len() {
            return Err(NetError::BadFrame);
        }
        out[..payload.len()].copy_from_slice(payload);
        return Ok(payload.len());
    }
}

/// Gửi `payload` tới `dst_ip:dst_port` và chờ **đúng** payload đó quay lại (echo), tối đa
/// `timeout_us`.
///
/// `Err(NetError::Timeout)` nếu không có reply; `Err(NetError::BadFrame)` nếu reply khác payload
/// (không nhận "gần đúng").
// 8 tham số: hai đầu cuối (IP + MAC) + hai cổng + payload + timeout — gói thành struct sẽ làm
// call-site khó đọc hơn ở ảnh test; cùng khuôn `#[allow]` đã dùng ở riscv64 KT-260.
#[allow(clippy::too_many_arguments)]
pub fn udp_echo(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    timeout_us: u64,
) -> Result<UdpEcho, NetError> {
    let started = now_us();
    udp_send(
        our_ip, our_mac, dst_ip, dst_mac, src_port, dst_port, payload,
    )?;
    // Ngân sách `timeout_us` tính cho **cả bước** (gửi + chờ) — cùng lý do KT-268 bên riscv64:
    // nếu hạn chờ bắt đầu **sau** khi gửi, `elapsed_ms` của ca timeout là 3000 + thời gian gửi.
    let spent = now_us().saturating_sub(started);
    let mut reply = [0u8; UDP_MAX_PAYLOAD];
    let len = udp_recv(
        our_ip,
        dst_ip,
        dst_port,
        src_port,
        &mut reply,
        timeout_us.saturating_sub(spent),
    )?;
    if &reply[..len] != payload {
        return Err(NetError::BadFrame);
    }
    Ok(UdpEcho {
        len,
        rtt_ms: us_to_ms(now_us().saturating_sub(started)),
    })
}

/// Gửi **một** truy vấn DNS `A` tới `dns_ip:53` qua UDP và parse reply.
///
/// `Err(NetError::Timeout)` nếu không có reply trong `timeout_us`; `Err(NetError::BadFrame)`
/// nếu reply không parse được / `id` lệch / không đúng một câu hỏi.
pub fn dns_query(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dns_ip: Ipv4Addr,
    dns_mac: MacAddr,
    id: u16,
    name: &[u8],
    timeout_us: u64,
) -> Result<DnsAnswer, NetError> {
    let query = DnsQuery::new(id, name, TYPE_A).map_err(|_| NetError::BadFrame)?;
    let mut request = [0u8; UDP_MAX_PAYLOAD];
    let request_len = query.encode(&mut request).map_err(|_| NetError::BadFrame)?;
    let started = now_us();
    udp_send(
        our_ip,
        our_mac,
        dns_ip,
        dns_mac,
        UDP_CLIENT_PORT,
        DNS_PORT,
        &request[..request_len],
    )?;
    // Như `udp_echo`: ngân sách tính cho cả bước, nên `elapsed_ms` của ca timeout đúng bằng
    // `timeout_us` (3000 ms), không cộng thời gian gửi.
    let spent = now_us().saturating_sub(started);
    let mut reply = [0u8; UDP_MAX_PAYLOAD];
    let reply_len = udp_recv(
        our_ip,
        dns_ip,
        DNS_PORT,
        UDP_CLIENT_PORT,
        &mut reply,
        timeout_us.saturating_sub(spent),
    )?;
    let message =
        DnsMessage::parse_response(&reply[..reply_len], id).map_err(|_| NetError::BadFrame)?;
    let question = message
        .question()
        .map_err(|_| NetError::BadFrame)?
        .ok_or(NetError::BadFrame)?;
    let a = message.first_a().map_err(|_| NetError::BadFrame)?;
    Ok(DnsAnswer {
        id,
        rcode: message.header().rcode(),
        question,
        a,
        rtt_ms: us_to_ms(now_us().saturating_sub(started)),
        reply_len,
    })
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
    assert!(hdr_bytes(true) <= RX_BUF_BYTES);
};
