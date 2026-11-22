// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter** `virtio-net` của x86_64 — KT-335 (trước: driver đầy đủ 1.271 dòng, KT-274).
//!
//! Toàn bộ quyết định giao thức virtio (status/feature/hai queue, descriptor, notify, poll
//! `used`, `virtio_net_config`, `virtio_net_hdr`) nằm ở [`vifast_virtio::netdev`] — crate
//! **thuần**, test host được (D2-05). Tệp này chỉ còn phần thuộc về nền tảng:
//!
//! 1. [`Mmio`] — đọc/ghi BAR bằng con trỏ thô + `fence(SeqCst)` (D2-17);
//! 2. [`Dma`] — một trang 4 KiB trong `.bss` (identity map ⇒ `VA == PA`), hai hook cache no-op
//!    **có chủ đích** (x86 không có cache không nhất quán với DMA);
//! 3. [`Pit`] — đồng hồ do **người gọi** truyền vào (PIT kênh 2 thuộc ảnh), giữ nguyên chữ ký
//!    `send_frame(frame, timeout_us, now)` / `recv_frame_wait(out, timeout_us, now)`;
//! 4. **chính sách** client của ảnh (ARP/DHCP/ICMP/UDP/DNS) + ba trait test được của KT-326/KT-328.
//!
//! Hành vi/TAP giữ **nguyên**: cùng chữ ký hàm, cùng mã lỗi, cùng bố cục trang DMA.

#![allow(unsafe_code)] // D2-17: `kernel/vifast-plat-*`; MMIO + vùng DMA (như bản trước KT-335).

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering};

use vifast_virtio::blkdev::Transport;
use vifast_virtio::dma::{DmaBuf, DmaPool};
use vifast_virtio::mmio::MmioBus;
use vifast_virtio::net::{QUEUE_SIZE, RX_BUF_BYTES, TX_BUF_BYTES};
use vifast_virtio::netdev::{Clock, Net};
pub use vifast_virtio::netdev::{NetReport, Stats};
use vifast_virtio::queue::VqLayout;

// Mã lỗi và giới hạn frame: **một** nguồn ở crate thuần (D2-110B).
pub use vifast_virtio::net::{NetError, MAX_FRAME_BYTES};
// Bố cục trang DMA + `past_deadline`: dùng chung 3 arch, tên giữ nguyên như trước KT-335.
pub use vifast_virtio::netdev::{
    past_deadline, DMA_PAGE_BYTES, RX_BUF_OFF, RX_RING_OFF, TX_BUF_OFF, TX_RING_OFF,
};

use vifast_net::addr::{same_subnet, Ipv4Addr, MacAddr, SubnetMask};
use vifast_net::arp::{ArpPacket, ARP_LEN};
use vifast_net::dhcp::{DhcpMessage, DHCPACK, DHCPOFFER, DHCP_MAX_LEN};
use vifast_net::eth::{EthHeader, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use vifast_net::frame::EthernetFrame;
use vifast_net::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_UDP};
use vifast_net::udp::{UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, UDP_HEADER_LEN};

use crate::pci::PciReport;

/// Trần poll `used` — 2 s (µs), như bản cũ.
pub const POLL_TIMEOUT_US: u64 = 2_000_000;
/// Timeout mặc định cho **một bước** mạng — 3 s.
pub const NET_STEP_TIMEOUT_US: u64 = 3_000_000;

// ---------------------------------------------------------------- trang DMA (identity map)

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: ảnh chạy một hart trên đường driver và driver tự tuần tự hoá (một frame một lúc mỗi
// hướng); vùng này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA`.
pub fn dma_page_addr() -> u64 {
    DMA.0.get() as *const _ as u64
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một hart + tuần tự (xem `DmaCell`).
    unsafe { &mut (*DMA.0.get()).0 }
}

// ---------------------------------------------------------------- adapter MMIO

struct Mmio;

impl MmioBus for Mmio {
    #[inline]
    fn read8(&self, addr: u64) -> u8 {
        // SAFETY: `addr` nằm trong BAR đã gán/map Device RW.
        unsafe { read_volatile(addr as *const u8) }
    }
    #[inline]
    fn read16(&self, addr: u64) -> u16 {
        // SAFETY: như `read8`; căn 2.
        unsafe { read_volatile(addr as *const u16) }
    }
    #[inline]
    fn read32(&self, addr: u64) -> u32 {
        // SAFETY: như `read8`; căn 4.
        unsafe { read_volatile(addr as *const u32) }
    }
    #[inline]
    fn write8(&mut self, addr: u64, value: u8) {
        // SAFETY: như `read8`; ghi thanh ghi thiết bị đã map.
        unsafe { write_volatile(addr as *mut u8, value) };
    }
    #[inline]
    fn write16(&mut self, addr: u64, value: u16) {
        // SAFETY: như `write8`; căn 2.
        unsafe { write_volatile(addr as *mut u16, value) };
    }
    #[inline]
    fn write32(&mut self, addr: u64, value: u32) {
        // SAFETY: như `write8`; căn 4.
        unsafe { write_volatile(addr as *mut u32, value) };
    }
    #[inline]
    fn write64(&mut self, addr: u64, value: u64) {
        // SAFETY: như `write8`; căn 8.
        unsafe { write_volatile(addr as *mut u64, value) };
    }
    #[inline]
    fn fence(&self) {
        // `mfence` qua `fence(SeqCst)`: x86 không có cờ `fence` của RISC-V nhưng cùng ngữ nghĩa
        // "mọi ghi trước đã thấy trước cửa thông báo" (spec §2.7.13).
        fence(Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- adapter DMA

/// Pool DMA một vùng: đúng trang `.bss` ở trên (không cấp phát động).
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
        if align > DMA_PAGE_BYTES || DMA_PAGE_BYTES % align != 0 {
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
        // SAFETY: `off` nằm trong trang DMA; đọc volatile để compiler không hoist khỏi vòng poll.
        unsafe { read_volatile((dma_page_addr() + off as u64) as *const u16) }
    }
    fn read_u8(&self, off: usize) -> u8 {
        // SAFETY: như `read_u16`.
        unsafe { read_volatile((dma_page_addr() + off as u64) as *const u8) }
    }
    fn sync_for_device(&mut self, _off: usize, _len: usize) {
        // x86: identity map, không có cache không nhất quán với DMA ⇒ no-op **có chủ đích**.
    }
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {
        // Như `sync_for_device`.
    }
}

// ---------------------------------------------------------------- adapter đồng hồ (PIT)

/// Đồng hồ do **người gọi** cấp (PIT kênh 2 thuộc ảnh, không thuộc crate platform) — giữ nguyên
/// chữ ký `now: &mut dyn FnMut() -> u64` mà `ktest_net`/`net_client` đã dùng.
struct Pit<'a>(&'a mut dyn FnMut() -> u64);

impl Clock for Pit<'_> {
    fn now(&mut self) -> u64 {
        (self.0)()
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static NET: Net = Net::new();

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** để TAP in ra lý do đo được.
pub fn init(pci: &PciReport) -> Result<NetReport, &'static str> {
    let transport = Transport::from_report(pci);
    NET.init(&mut Mmio, &transport, &mut Dma)
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    NET.stats()
}

/// Gửi **một** frame Ethernet, chờ tối đa `timeout_us`; `now` là đồng hồ PIT của ảnh.
pub fn send_frame(
    frame: &[u8],
    timeout_us: u64,
    now: &mut dyn FnMut() -> u64,
) -> Result<(), NetError> {
    NET.send_frame(&mut Mmio, &mut Dma, &mut Pit(now), frame, timeout_us)
}

/// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ).
pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    recv_frame_wait(out, 0, &mut || 0)
}

/// Nhận **một** frame, chờ tối đa `timeout_us`; `now` là đồng hồ PIT của ảnh.
pub fn recv_frame_wait(
    out: &mut [u8],
    timeout_us: u64,
    now: &mut dyn FnMut() -> u64,
) -> Option<usize> {
    NET.recv_frame_wait(&mut Mmio, &mut Dma, &mut Pit(now), out, timeout_us)
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

// ================================================================================
// DHCP client — KT-326 (K5': DHCP trên x86_64)
// ================================================================================
//
// # Vì sao client DHCP nằm ở đây (và **không** viết lại codec)
//
// Codec BOOTP/DHCP (DISCOVER/REQUEST + đọc OFFER/ACK, magic cookie, option TLV) đã có trong
// `vifast_net::dhcp` — dùng lại nguyên vẹn (D2-110A/B). Tệp này chỉ ghép khung Ethernet + IPv4 +
// UDP (checksum thật), chạy máy trạng thái DISCOVER → OFFER → REQUEST → ACK, và **chặn** frame
// không phải của mình.
//
// # Trần thời gian — khuôn KT-301, không phải "timeout rồi thôi"
//
// Một client DHCP chạy trong ảnh **không** được phép chờ vô hạn: nếu slirp/server không trả lời,
// boot sẽ treo tới khi runner giết QEMU, và khi đó log **không** nói được phép đo nào hỏng. Ba
// trần độc lập:
//
// 1. **toàn bước** [`DHCP_TOTAL_TIMEOUT_US`] (6 s) — deadline tuyệt đối, mọi vòng lặp so với nó;
// 2. **mỗi lần thử** [`DHCP_ATTEMPT_TIMEOUT_US`] (1 s) — DISCOVER/REQUEST được gửi lại trong
//    ngân sách tổng;
// 3. **số lần thử** [`DHCP_MAX_ATTEMPTS`] (3) cho **mỗi** pha — không retry vô hạn.
//
// `timeout = 0` **không** được hiểu là "chờ mãi": [`dhcp_acquire_with`] trả `Timeout` ngay khi
// ngân sách/lần thử/số lần thử bằng 0. Đây là hợp đồng mà đột biến (iii) của KT-326 kiểm.

/// Trần **toàn bộ** bước DHCP (µs): 6 s — dưới ngân sách boot 20 s của runner.
pub const DHCP_TOTAL_TIMEOUT_US: u64 = 6_000_000;
/// Trần **mỗi lần gửi/chờ** (µs): 1 s.
pub const DHCP_ATTEMPT_TIMEOUT_US: u64 = 1_000_000;
/// Số lần thử **mỗi pha** (DISCOVER, REQUEST) — trần, không retry vô hạn.
pub const DHCP_MAX_ATTEMPTS: u32 = 3;
/// `xid` cố định của client (chưa có RNG); reply còn phải khớp `chaddr` = MAC của ta.
pub const DHCP_XID: u32 = 0x4b54_3332;

/// Lease DHCP đã kiểm — đủ trường để in bằng chứng và để TAP 31 quyết định.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DhcpLease {
    /// Địa chỉ server cấp (`yiaddr`).
    pub ip: Ipv4Addr,
    /// Mặt nạ mạng (option 1).
    pub mask: Ipv4Addr,
    /// Gateway/router đầu tiên (option 3).
    pub gw: Ipv4Addr,
    /// DNS đầu tiên (option 6).
    pub dns: Ipv4Addr,
    /// Server identifier (option 54).
    pub server: Ipv4Addr,
    /// Thời hạn lease (option 51), giây.
    pub lease_secs: u32,
    /// Tổng số frame đã **gửi** (DISCOVER + REQUEST), kể cả lần gửi lại.
    pub attempts: u32,
    /// `true` ⇔ bộ lọc đã từ chối một ACK bị hỏng **magic cookie** (đột biến (i)).
    pub cookie_rejected: bool,
    /// `true` ⇔ bộ lọc đã từ chối một ACK bị đổi **xid** (đột biến (ii)).
    pub xid_rejected: bool,
}

impl DhcpLease {
    /// `true` ⇔ lease **nhất quán**: địa chỉ/mặt nạ/gateway/DNS/server hợp lệ, cùng mạng, lease
    /// dương. Đây là điều kiện **của TAP 31**, không phải "đọc được một dòng".
    pub fn valid(&self) -> bool {
        if self.ip.is_unspecified()
            || self.ip.is_broadcast()
            || self.gw.is_unspecified()
            || self.gw.is_broadcast()
            || self.dns.is_unspecified()
            || self.dns.is_broadcast()
            || self.server.is_unspecified()
            || self.server.is_broadcast()
            || self.lease_secs == 0
            || self.ip == self.gw
            || self.ip == self.dns
        {
            return false;
        }
        let Some(mask) = SubnetMask::from_addr(self.mask) else {
            return false;
        };
        // Mặt nạ /0 hoặc /32 không phải cấu hình mà một DHCP server lành gửi cho client.
        if mask.prefix() == 0 || mask.prefix() == 32 {
            return false;
        }
        same_subnet(self.ip, self.gw, mask) && same_subnet(self.ip, self.dns, mask)
    }

    /// `true` ⇔ **cả hai** phép tự kiểm của bộ lọc đều bắt được gói độc (xem
    /// [`dhcp_filter_selfcheck`]).
    pub const fn filter_ok(&self) -> bool {
        self.cookie_rejected && self.xid_rejected
    }
}

/// Ghép **một** frame Ethernet + IPv4 + UDP (checksum đầy đủ) vào `out`; trả tổng số byte.
///
/// Dùng cho cả chiều client → server (DHCP DISCOVER/REQUEST) **và** frame "độc" của phép tự kiểm
/// bộ lọc — nhờ vậy frame độc có checksum **đúng**, và lý do duy nhất bị từ chối là cookie/xid.
#[allow(clippy::too_many_arguments)] // một frame = các trường của nó; gói lại chỉ thêm lớp.
pub fn udp_frame(
    src_mac: MacAddr,
    dst_mac: MacAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    let mut udp = [0u8; DHCP_MAX_LEN + UDP_HEADER_LEN];
    let udp_len = UdpHeader::encode(src_ip, dst_ip, src_port, dst_port, payload, &mut udp).ok()?;
    let mut ip = [0u8; DHCP_MAX_LEN + UDP_HEADER_LEN + IPV4_MIN_HEADER_LEN];
    let header_len = Ipv4Header::new(PROTO_UDP, src_ip, dst_ip, udp_len)
        .encode(&mut ip)
        .ok()?;
    ip[header_len..header_len + udp_len].copy_from_slice(&udp[..udp_len]);
    let ip_len = header_len + udp_len;
    EthernetFrame {
        header: EthHeader::new(dst_mac, src_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(out)
    .ok()
}

/// Dựng frame DHCP **client → server** (broadcast, cổng 68 → 67) vào `out`; trả tổng số byte.
pub fn dhcp_frame(message: &DhcpMessage, mac: MacAddr, out: &mut [u8]) -> Option<usize> {
    udp_frame(
        mac,
        MacAddr::BROADCAST,
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        message.as_bytes(),
        out,
    )
}

/// Đọc frame DHCP **server → client** và chỉ nhận khi khớp **cả ba**: `xid`, `chaddr`,
/// `msg_type`. Trả `None` cho frame khác (không PANIC, không đoán).
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

/// Đọc lease từ ACK; **từ chối** ACK thiếu mặt nạ/router/DNS/lease/server hoặc lease không nhất
/// quán (không đoán giá trị thiếu).
pub fn dhcp_lease_from_ack(ack: &DhcpMessage, attempts: u32) -> Result<DhcpLease, NetError> {
    let lease = DhcpLease {
        ip: ack.yiaddr(),
        mask: ack.subnet_mask().ok_or(NetError::Io)?,
        gw: ack.router().ok_or(NetError::Io)?,
        dns: ack.dns().ok_or(NetError::Io)?,
        server: ack.server_id().ok_or(NetError::Io)?,
        lease_secs: ack.lease_secs().ok_or(NetError::Io)?,
        attempts,
        cookie_rejected: false,
        xid_rejected: false,
    };
    if !lease.valid() {
        return Err(NetError::Io);
    }
    Ok(lease)
}

/// Bộ lọc tự kiểm **chạy trong ảnh** (TAP 31): bơm lại chính payload ACK với
/// (1) magic cookie sai, (2) `xid` lạ; trả `(cookie_bị_từ_chối, xid_bị_từ_chối)`.
///
/// Frame độc được dựng qua [`udp_frame`] nên **checksum đúng** — nếu bộ lọc nhận nó, nguyên nhân
/// **chỉ** có thể là thiếu phép kiểm cookie/xid, không phải lỗi checksum.
pub fn dhcp_filter_selfcheck(mac: MacAddr, xid: u32, ack_payload: &[u8]) -> (bool, bool) {
    let Ok(ack) = DhcpMessage::parse(ack_payload) else {
        return (false, false);
    };
    let src_ip = ack.server_id().unwrap_or(Ipv4Addr::UNSPECIFIED);
    let dst_ip = ack.yiaddr();
    let mut payload = [0u8; DHCP_MAX_LEN];
    let len = ack_payload.len().min(DHCP_MAX_LEN);
    payload[..len].copy_from_slice(&ack_payload[..len]);

    // (1) magic cookie sai — mọi byte còn lại giữ nguyên.
    payload[236] ^= 0xff;
    let cookie_rejected = dhcp_poison_rejected(&payload[..len], mac, xid, src_ip, dst_ip);
    payload[236] ^= 0xff;

    // (2) `xid` lạ — cookie và checksum vẫn đúng.
    payload[4] ^= 0xff;
    let xid_rejected = dhcp_poison_rejected(&payload[..len], mac, xid, src_ip, dst_ip);
    (cookie_rejected, xid_rejected)
}

/// `true` ⇔ frame chứa `payload` bị [`dhcp_frame_parse`] **từ chối**.
fn dhcp_poison_rejected(
    payload: &[u8],
    mac: MacAddr,
    xid: u32,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
) -> bool {
    let mut frame = [0u8; 720];
    let Some(len) = udp_frame(
        mac,
        mac,
        src_ip,
        dst_ip,
        DHCP_SERVER_PORT,
        DHCP_CLIENT_PORT,
        payload,
        &mut frame,
    ) else {
        return false;
    };
    dhcp_frame_parse(&frame[..len], xid, mac, DHCPACK).is_none()
}

/// Nguồn khung/đồng hồ của **một bước mạng** (DHCP client, ICMP echo, DNS query) — tách khỏi
/// driver để **host test** bơm frame giả và chứng minh trần thời gian mà không cần QEMU.
///
/// **KT-328**: ba bước mạng dùng **cùng** một trait (không viết bản thứ hai cho ICMP/DNS); tên cũ
/// [`DhcpIo`] giữ làm bí danh để call-site/host test của KT-326 không phải đổi.
pub trait NetIo {
    /// Đồng hồ đơn điệu (µs).
    fn now_us(&mut self) -> u64;
    /// Gửi một frame; `timeout_us` là trần poll-used.
    fn send(&mut self, frame: &[u8], timeout_us: u64) -> Result<(), NetError>;
    /// Nhận một frame, chờ tối đa `timeout_us`; `None` = hết hạn/chưa có.
    fn recv(&mut self, out: &mut [u8], timeout_us: u64) -> Option<usize>;
}

/// Bí danh của [`NetIo`] cho tên đã dùng từ KT-326 (một trait, **không** hai bản).
pub use NetIo as DhcpIo;

/// `NetIo` trên driver thật + đồng hồ PIT kênh 2 do ảnh sở hữu.
///
/// `pub(crate)` vì [`crate::net_client`] (ICMP/DNS) dùng **cùng** một nguồn khung/đồng hồ — không
/// có bản thứ hai.
pub(crate) struct VirtioNetIo<'a> {
    now: &'a mut dyn FnMut() -> u64,
}

impl<'a> VirtioNetIo<'a> {
    /// Bọc đồng hồ của ảnh thành nguồn khung/đồng hồ của một bước mạng.
    pub(crate) fn new(now: &'a mut dyn FnMut() -> u64) -> Self {
        Self { now }
    }
}

impl NetIo for VirtioNetIo<'_> {
    fn now_us(&mut self) -> u64 {
        (self.now)()
    }

    fn send(&mut self, frame: &[u8], timeout_us: u64) -> Result<(), NetError> {
        send_frame(frame, timeout_us, &mut *self.now)
    }

    fn recv(&mut self, out: &mut [u8], timeout_us: u64) -> Option<usize> {
        recv_frame_wait(out, timeout_us, &mut *self.now)
    }
}

/// Ngân sách còn lại cho **một lần thử**: `min(attempt_timeout, còn lại tới deadline tổng)`.
///
/// `pub(crate)`: DHCP (tệp này) và ICMP/DNS ([`crate::net_client`]) dùng **cùng** một phép tính —
/// ba trần phải được áp giống nhau ở mọi bước mạng.
pub(crate) fn attempt_budget(now: u64, total_deadline: u64, attempt_timeout_us: u64) -> u64 {
    let remaining = total_deadline.saturating_sub(now);
    if remaining < attempt_timeout_us {
        remaining
    } else {
        attempt_timeout_us
    }
}

/// Chờ một frame DHCP khớp `xid`/`mac`/`want_type` tới **deadline tuyệt đối** `deadline_us`.
fn dhcp_wait(
    io: &mut dyn NetIo,
    xid: u32,
    mac: MacAddr,
    want_type: u8,
    deadline_us: u64,
) -> Result<DhcpMessage, NetError> {
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = io.now_us();
        if now >= deadline_us {
            return Err(NetError::Timeout);
        }
        let remaining = deadline_us.saturating_sub(now);
        // `remaining > 0` (đã kiểm ở trên) — **không bao giờ** truyền 0 xuống như "chờ mãi".
        if let Some(len) = io.recv(&mut frame, remaining) {
            if let Some(message) = dhcp_frame_parse(&frame[..len], xid, mac, want_type) {
                return Ok(message);
            }
        }
    }
}

/// **DHCP client** đầy đủ: DISCOVER → OFFER → REQUEST → ACK, với **ba trần** ở đầu tệp.
///
/// Trả `Err(NetError::Timeout)` khi hết ngân sách/tổng số lần thử, `Err(NetError::Io)` khi server
/// trả ACK thiếu trường hoặc OFFER/ACK lệch nhau (fail-closed, không đoán).
pub fn dhcp_acquire(
    total_timeout_us: u64,
    now: &mut dyn FnMut() -> u64,
) -> Result<DhcpLease, NetError> {
    let mut io = VirtioNetIo::new(now);
    dhcp_acquire_with(
        &mut io,
        MacAddr::new(mac()),
        DHCP_XID,
        total_timeout_us,
        DHCP_ATTEMPT_TIMEOUT_US,
        DHCP_MAX_ATTEMPTS,
    )
}

/// Máy trạng thái DHCP trên một [`DhcpIo`] bất kỳ (driver thật hoặc fake trong test host).
///
/// `total_timeout_us == 0` (hoặc `attempt_timeout_us == 0`, `max_attempts == 0`) ⇒ `Timeout`
/// **ngay**, không gửi/không chờ: 0 **không** phải "vô hạn".
pub fn dhcp_acquire_with(
    io: &mut dyn NetIo,
    mac: MacAddr,
    xid: u32,
    total_timeout_us: u64,
    attempt_timeout_us: u64,
    max_attempts: u32,
) -> Result<DhcpLease, NetError> {
    if total_timeout_us == 0 || attempt_timeout_us == 0 || max_attempts == 0 {
        return Err(NetError::Timeout);
    }
    let total_deadline = io.now_us().saturating_add(total_timeout_us);
    let mut frame = [0u8; 720];
    let mut attempts = 0u32;

    // ---------------------------------------------------------------- DISCOVER → OFFER
    let discover = DhcpMessage::discover(xid, mac);
    let discover_len = dhcp_frame(&discover, mac, &mut frame).ok_or(NetError::BadFrame)?;
    let mut offer: Option<DhcpMessage> = None;
    let mut tried = 0u32;
    while tried < max_attempts {
        tried += 1;
        attempts = attempts.saturating_add(1);
        let now = io.now_us();
        if now >= total_deadline {
            return Err(NetError::Timeout);
        }
        let send_budget = attempt_budget(now, total_deadline, attempt_timeout_us);
        io.send(&frame[..discover_len], send_budget)?;
        // Đo lại sau khi gửi: thời gian gửi cũng tiêu ngân sách (một đồng hồ, một trần).
        let now = io.now_us();
        if now >= total_deadline {
            return Err(NetError::Timeout);
        }
        let attempt_deadline =
            now.saturating_add(attempt_budget(now, total_deadline, attempt_timeout_us));
        match dhcp_wait(
            io,
            xid,
            mac,
            DHCPOFFER,
            attempt_deadline.min(total_deadline),
        ) {
            Ok(message) => {
                offer = Some(message);
                break;
            }
            Err(NetError::Timeout) => continue,
            Err(error) => return Err(error),
        }
    }
    let offer = offer.ok_or(NetError::Timeout)?;
    let yiaddr = offer.yiaddr();
    if yiaddr.is_unspecified() || yiaddr.is_broadcast() {
        return Err(NetError::Io);
    }
    let server = offer.server_id().ok_or(NetError::Io)?;

    // ---------------------------------------------------------------- REQUEST → ACK
    let request = DhcpMessage::request(xid, mac, yiaddr, server);
    let request_len = dhcp_frame(&request, mac, &mut frame).ok_or(NetError::BadFrame)?;
    let mut ack: Option<DhcpMessage> = None;
    let mut tried = 0u32;
    while tried < max_attempts {
        tried += 1;
        attempts = attempts.saturating_add(1);
        let now = io.now_us();
        if now >= total_deadline {
            return Err(NetError::Timeout);
        }
        let send_budget = attempt_budget(now, total_deadline, attempt_timeout_us);
        io.send(&frame[..request_len], send_budget)?;
        let now = io.now_us();
        if now >= total_deadline {
            return Err(NetError::Timeout);
        }
        let attempt_deadline =
            now.saturating_add(attempt_budget(now, total_deadline, attempt_timeout_us));
        match dhcp_wait(io, xid, mac, DHCPACK, attempt_deadline.min(total_deadline)) {
            Ok(message) => {
                ack = Some(message);
                break;
            }
            Err(NetError::Timeout) => continue,
            Err(error) => return Err(error),
        }
    }
    let ack = ack.ok_or(NetError::Timeout)?;
    if ack.yiaddr() != yiaddr {
        return Err(NetError::Io);
    }
    let (cookie_rejected, xid_rejected) = dhcp_filter_selfcheck(mac, xid, ack.as_bytes());
    let mut lease = dhcp_lease_from_ack(&ack, attempts)?;
    lease.cookie_rejected = cookie_rejected;
    lease.xid_rejected = xid_rejected;
    Ok(lease)
}

/// **Re-export** của [`crate::net_client`] (KT-328) để ảnh và host test dùng **một** đường
/// (`vifast_plat_x86_64::virtio_net::{ping, dns_query, …}`) — không có API thứ hai cho ICMP/DNS.
pub use crate::net_client::{
    dns_filter_selfcheck, dns_query, dns_query_with, dns_reply_accept, icmp_echo_frame,
    icmp_filter_selfcheck, icmp_reply_accept, icmp_reply_frame, ping, ping_with, DnsAnswer,
    DnsReply, NetPeer, PingStats, RetryBudget, DNS_ATTEMPT_TIMEOUT_US, DNS_CLIENT_PORT,
    DNS_FRAME_BYTES, DNS_ID, DNS_MAX_ATTEMPTS, DNS_MAX_LEN, DNS_PORT, DNS_TOTAL_TIMEOUT_US,
    ICMP_ATTEMPT_TIMEOUT_US, ICMP_ECHO_ID, ICMP_ECHO_SEQ, ICMP_FRAME_BYTES, ICMP_MAX_ATTEMPTS,
    ICMP_PAYLOAD_BYTES, ICMP_TOTAL_TIMEOUT_US,
};

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
