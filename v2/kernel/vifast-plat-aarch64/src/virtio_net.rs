// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter** `virtio-net` của aarch64 — KT-335 (trước: driver đầy đủ 1.395 dòng, KT-275).
//!
//! Toàn bộ quyết định giao thức virtio (status/feature/hai queue, descriptor, notify, poll
//! `used`, `virtio_net_config`, `virtio_net_hdr`) nằm ở [`vifast_virtio::netdev`] — crate
//! **thuần**, test host được (D2-05). Tệp này chỉ còn phần thuộc về nền tảng:
//!
//! 1. [`Mmio`] — đọc/ghi thanh ghi qua [`crate::asm`] (`asm::mmio_*`, `dsb sy`) — `unsafe` nằm ở
//!    `asm.rs` của crate này, không ở crate logic (D2-17);
//! 2. [`Dma`] — trang DMA **do `asm` giữ** (`.bss` dưới `KERNEL_HIGH_BASE`), nên PA **truyền
//!    vào** [`init`] chứ không suy từ VA (KT-273/KT-338a);
//! 3. [`Micro`] — đồng hồ `CNTPCT_EL0 / CNTFRQ_EL0` (µs), như bản cũ;
//! 4. **chính sách** client của ảnh (DHCP/ARP/ICMP/UDP/DNS) — cần `send_frame` nên thuộc nền tảng.
//!
//! Hành vi/TAP giữ **nguyên**: cùng chữ ký hàm, cùng mã lỗi, cùng bố cục trang DMA.

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_virtio::blkdev::Transport;
use vifast_virtio::dma::{DmaBuf, DmaPool};
use vifast_virtio::mmio::MmioBus;
use vifast_virtio::net::{hdr_bytes, QUEUE_SIZE, RX_BUF_BYTES, TX_BUF_BYTES};
use vifast_virtio::netdev::{Clock, Net};
pub use vifast_virtio::netdev::{NetReport, Stats};
use vifast_virtio::queue::VqLayout;

// Mã lỗi và giới hạn frame: **một** nguồn ở crate thuần (D2-110B).
pub use vifast_virtio::net::{NetError, MAX_FRAME_BYTES};
// Bố cục trang DMA + `past_deadline`: dùng chung 3 arch, tên giữ nguyên như trước KT-335.
pub use vifast_virtio::netdev::{
    past_deadline, DMA_PAGE_BYTES, RX_BUF_OFF, RX_RING_OFF, TX_BUF_OFF, TX_RING_OFF,
};

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::arp::{ArpPacket, ARP_LEN};
use vifast_net::dhcp::{DhcpMessage, DHCPACK, DHCPOFFER};
use vifast_net::dns::{ARecord, DnsMessage, DnsQuery, DnsQuestion, DnsRetry, TYPE_A};
use vifast_net::eth::{EthHeader, ETHERTYPE_ARP, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp::{IcmpEcho, IcmpMessage};
use vifast_net::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_ICMP, PROTO_UDP};
use vifast_net::udp::{UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, UDP_HEADER_LEN};

use crate::asm;
use crate::pci::PciReport;

/// Trần poll `used` — 2 s (µs), như bản cũ.
pub const POLL_TIMEOUT_US: u64 = 2_000_000;
/// Timeout mặc định cho **một bước** mạng (DHCP/ARP/ping) — 3 s (D2-196 mục (1)).
pub const NET_STEP_TIMEOUT_US: u64 = 3_000_000;
/// Cỡ payload ICMP echo (byte đệm; đủ để RTT có nghĩa, không cần lớn).
pub const ICMP_PAYLOAD_BYTES: usize = 32;

// ---------------------------------------------------------------- adapter MMIO

struct Mmio;

impl MmioBus for Mmio {
    #[inline]
    fn read8(&self, addr: u64) -> u8 {
        asm::mmio_read_u8(addr)
    }
    #[inline]
    fn read16(&self, addr: u64) -> u16 {
        asm::mmio_read_u16(addr)
    }
    #[inline]
    fn read32(&self, addr: u64) -> u32 {
        asm::mmio_read_u32(addr)
    }
    #[inline]
    fn write8(&mut self, addr: u64, value: u8) {
        asm::mmio_write_u8(addr, value);
    }
    #[inline]
    fn write16(&mut self, addr: u64, value: u16) {
        asm::mmio_write_u16(addr, value);
    }
    #[inline]
    fn write32(&mut self, addr: u64, value: u32) {
        asm::mmio_write_u32(addr, value);
    }
    #[inline]
    fn write64(&mut self, addr: u64, value: u64) {
        asm::mmio_write_u64(addr, value);
    }
    #[inline]
    fn fence(&self) {
        // `dsb sy` (không phải `dmb`): DMA của thiết bị nằm **ngoài** domain shareability của CPU
        // nên cần barrier **đầy đủ** — cùng lý do `fence rw, rw` bên riscv64.
        asm::dsb_sy();
    }
}

// ---------------------------------------------------------------- adapter DMA

/// PA trang DMA của virtio-net — **truyền vào** [`init`] (trang nằm trong `.bss` dưới
/// `KERNEL_HIGH_BASE`, nên `addr_of!` cho alias nửa cao, **không** phải PA).
static DMA_PA: AtomicU64 = AtomicU64::new(0);

/// Pool DMA một vùng: đúng trang do `asm` giữ (không cấp phát động).
struct Dma;

impl DmaPool for Dma {
    fn base_pa(&self) -> u64 {
        DMA_PA.load(Ordering::Relaxed)
    }
    fn capacity(&self) -> usize {
        DMA_PAGE_BYTES
    }
    fn bytes(&mut self) -> &mut [u8] {
        asm::dma_page_net()
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        let pa = DMA_PA.load(Ordering::Relaxed);
        // `pa == 0` ⇒ caller chưa truyền PA ⇒ **fail-closed** (không DMA vào địa chỉ 0).
        if pa == 0 || len == 0 || len > DMA_PAGE_BYTES || align == 0 || !align.is_power_of_two() {
            return None;
        }
        if align > DMA_PAGE_BYTES || DMA_PAGE_BYTES % align != 0 {
            return None;
        }
        Some(DmaBuf { off: 0, len, pa })
    }
    fn free(&mut self, _buf: DmaBuf) -> bool {
        true
    }
    fn read_u16(&self, off: usize) -> u16 {
        asm::mmio_read_u16(DMA_PA.load(Ordering::Relaxed) + off as u64)
    }
    fn read_u8(&self, off: usize) -> u8 {
        asm::mmio_read_u8(DMA_PA.load(Ordering::Relaxed) + off as u64)
    }
    fn sync_for_device(&mut self, _off: usize, _len: usize) {
        // KT-275 **không** có bảo trì cache cho virtio-net (chỉ virtio-blk cần, KT-277) và
        // KT-335 giữ **nguyên** hành vi ấy: thêm `dc civac` ở đây là đổi hành vi DMA chưa đo lại
        // (rủi ro hồi quy trên TAP mạng aarch64). Hook vẫn tồn tại theo hợp đồng `DmaPool`
        // (D2-239(1)) để lần mở sau chỉ phải sửa **một** chỗ.
    }
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {
        // Như `sync_for_device`.
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

struct Micro;

impl Clock for Micro {
    fn now(&mut self) -> u64 {
        now_us()
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static NET: Net = Net::new();

/// Bộ đếm vật lý hiện tại (`CNTPCT_EL0`).
pub fn now_ticks() -> u64 {
    asm::read_cntpct_el0()
}

/// Đồng hồ **µs** của driver — `CNTPCT_EL0 / CNTFRQ_EL0`.
///
/// `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ trả **tick** thay vì µs: ngân sách 3 s khi ấy là 3 M tick,
/// tức **ngắn hơn** 3 s thật ⇒ fail-closed (timeout), không bao giờ dài hơn ngân sách.
pub fn now_us() -> u64 {
    let freq = asm::read_cntfrq_el0();
    let ticks = now_ticks();
    match ticks.saturating_mul(1_000_000).checked_div(freq) {
        Some(us) => us,
        None => ticks,
    }
}

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết (cùng công thức `virtio_blk` KT-273).
pub const fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - crate::split_mmu::KERNEL_HIGH_BASE)
}

/// Khởi tạo driver từ kết quả PCI + **PA** của trang DMA. Lỗi trả **tên chặng** để TAP in ra.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<NetReport, &'static str> {
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    let transport = Transport::from_report(pci);
    NET.init(&mut Mmio, &transport, &mut Dma)
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    NET.stats()
}

/// Gửi **một** frame Ethernet (không kể header virtio), chờ tối đa `timeout_us`.
pub fn send_frame(frame: &[u8], timeout_us: u64) -> Result<(), NetError> {
    NET.send_frame(&mut Mmio, &mut Dma, &mut Micro, frame, timeout_us)
}

/// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có frame.
pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    NET.recv_frame(&mut Mmio, &mut Dma, &mut Micro, out)
}

/// Nhận **một** frame, chờ tối đa `timeout_us`. `None` = hết hạn / chưa có.
pub fn recv_frame_wait(out: &mut [u8], timeout_us: u64) -> Option<usize> {
    NET.recv_frame_wait(&mut Mmio, &mut Dma, &mut Micro, out, timeout_us)
}

/// MAC đọc từ thiết bị (toàn 0 nếu chưa init).
pub fn mac() -> [u8; 6] {
    NET.mac()
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    NET.ready()
}

/// Địa chỉ ISR (chẩn đoán; chưa dùng ngắt).
pub fn isr_addr() -> u64 {
    NET.isr_addr()
}

/// Đọc thanh ghi `device_status` hiện tại.
pub fn device_status() -> u8 {
    NET.device_status(&mut Mmio)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    NET.config_generation(&mut Mmio)
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

/// Gửi truy vấn DNS `A` tới `dns_ip:53` qua UDP và parse reply.
///
/// **KT-294** — **thử lại** theo RFC 1035 §4.2.1: lần thử **timeout** thì gửi lại **đúng** message
/// cũ (**cùng `id`** — RFC cho phép `id` mới, nhưng giữ `id` để reply của lần trước không bị nhận
/// nhầm; `parse_response` kiểm `id`), tối đa [`DNS_MAX_ATTEMPTS`] lần, mỗi lần có ngân sách
/// `timeout_us` **riêng** (3 s — **không** đổi). Reply hỏng **không** thử lại: đó là lỗi giao
/// thức, thử lại chỉ che lỗi.
///
/// `retry` là bộ đếm của **lần gọi này** (tạo mới cho mỗi truy vấn): sau khi trả về,
/// `retry.attempts()` là số lần đã gửi — caller in nó vào dòng evidence, **kể cả** khi lỗi.
///
/// `Err(NetError::Timeout)` nếu **cả hai** lần thử không có reply; `Err(NetError::BadFrame)` nếu
/// reply không parse được / `id` lệch / không đúng một câu hỏi.
// 8 tham số: hai đầu cuối (IP + MAC) + `id` + tên + timeout + bộ đếm thử lại — gói thành struct sẽ
// làm call-site khó đọc hơn ở ảnh test; cùng khuôn `#[allow]` đã dùng ở `udp_echo`.
#[allow(clippy::too_many_arguments)]
pub fn dns_query(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dns_ip: Ipv4Addr,
    dns_mac: MacAddr,
    id: u16,
    name: &[u8],
    timeout_us: u64,
    retry: &mut DnsRetry,
) -> Result<DnsAnswer, NetError> {
    let query = DnsQuery::new(id, name, TYPE_A).map_err(|_| NetError::BadFrame)?;
    let mut request = [0u8; UDP_MAX_PAYLOAD];
    let request_len = query.encode(&mut request).map_err(|_| NetError::BadFrame)?;
    let started = now_us();
    loop {
        retry.record_attempt();
        let attempt_started = now_us();
        // Một lần thử = **gửi + chờ**; ngân sách `timeout_us` tính cho cả hai (như trước KT-294),
        // nên `elapsed_ms` của ca timeout là `attempts × timeout_us`, không cộng thời gian gửi.
        let sent = udp_send(
            our_ip,
            our_mac,
            dns_ip,
            dns_mac,
            UDP_CLIENT_PORT,
            DNS_PORT,
            &request[..request_len],
        );
        let mut reply = [0u8; UDP_MAX_PAYLOAD];
        let received = match sent {
            Ok(()) => {
                let spent = now_us().saturating_sub(attempt_started);
                udp_recv(
                    our_ip,
                    dns_ip,
                    DNS_PORT,
                    UDP_CLIENT_PORT,
                    &mut reply,
                    timeout_us.saturating_sub(spent),
                )
            }
            Err(error) => Err(error),
        };
        match received {
            Ok(reply_len) => {
                let message = DnsMessage::parse_response(&reply[..reply_len], id)
                    .map_err(|_| NetError::BadFrame)?;
                let question = message
                    .question()
                    .map_err(|_| NetError::BadFrame)?
                    .ok_or(NetError::BadFrame)?;
                let a = message.first_a().map_err(|_| NetError::BadFrame)?;
                return Ok(DnsAnswer {
                    id,
                    rcode: message.header().rcode(),
                    question,
                    a,
                    rtt_ms: us_to_ms(now_us().saturating_sub(started)),
                    reply_len,
                });
            }
            // Chỉ **timeout** mới thử lại; hết lượt thì trả lỗi (fail-closed, không treo).
            Err(NetError::Timeout) if retry.should_retry(true) => continue,
            Err(error) => return Err(error),
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
    assert!(hdr_bytes(true) <= RX_BUF_BYTES);
};
