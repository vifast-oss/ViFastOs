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
//! 4. **adapter** chính sách client của ảnh (DHCP/ARP/ICMP/UDP/DNS) — **KT-335b** dời chính sách
//!    về `vifast_net::{dhcp,arp,icmp,udp,dns}_client` (host test được); tệp này chỉ còn
//!    `Micro`/`Wire` + hàm mỏng giữ **nguyên** chữ ký cũ cho ảnh.
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
use vifast_net::arp_client;
use vifast_net::client::{Clock as PolicyClock, Link, LinkError, NetPeer, RetryBudget};
use vifast_net::dhcp_client;
use vifast_net::dns::{DnsRetry, DNS_MAX_ATTEMPTS};
use vifast_net::dns_client;
use vifast_net::icmp_client;
use vifast_net::udp_client;

use crate::asm;
use crate::pci::PciReport;

/// Trần poll `used` — 2 s (µs), như bản cũ.
pub const POLL_TIMEOUT_US: u64 = 2_000_000;
/// Timeout mặc định cho **một bước** mạng (DHCP/ARP/ping) — 3 s (D2-196 mục (1)).
pub const NET_STEP_TIMEOUT_US: u64 = 3_000_000;

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
// ================================================================================
// KT-335b — adapter chính sách DHCP + ARP + ICMP + UDP + DNS
//
// Trước KT-335b, khối này là **bản sao thứ hai** trong ba bản (`vifast-plat-*`): cùng codec, cùng
// điều kiện nhận, chỉ khác đồng hồ (`CNTPCT_EL0` ở đây). Sau KT-335b, chính sách nằm **một** chỗ
// trong `vifast-net` (host test được — D2-05); tệp này chỉ còn:
//
// 1. [`Micro`] — đồng hồ `CNTPCT_EL0 / CNTFRQ_EL0` (µs) đã có từ trước;
// 2. [`Wire`] — đường frame trên `send_frame`/`recv_frame_wait` của driver;
// 3. hàm mỏng giữ **nguyên** chữ ký cũ cho ảnh `vifast-kbin-aarch64` (TAP không đổi).
//
// # Ba trần giữ nguyên cho aarch64 (không nới)
//
// Bản cũ: **một** ngân sách 3 s cho mỗi pha DHCP / mỗi echo; DNS thử lại tối đa
// `DNS_MAX_ATTEMPTS` (2) lần, mỗi lần 3 s (KT-294). Adapter truyền `RetryBudget` đúng như vậy
// (`max_attempts = 1` cho DHCP/ICMP), nên **hành vi timeout của aarch64 không đổi**.
// ================================================================================

impl PolicyClock for Micro {
    fn now(&self) -> u64 {
        now_us()
    }
}

/// Đường frame của driver: gửi/nhận **một** frame Ethernet trên virtio-net.
struct Wire;

impl Link for Wire {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        send_frame(frame, POLL_TIMEOUT_US).map_err(to_link_error)
    }

    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize> {
        recv_frame_wait(out, timeout)
    }
}

/// `NetError` của driver → `LinkError` của crate chính sách (ba tên trùng nhau).
///
/// `NotReady`/`NoDevice`/`QueueFull` không có nghĩa tương đương trong crate logic; ánh xạ `Io`
/// là fail-closed (đường thành công không đi qua đây: `init` đã kiểm `ready`).
fn to_link_error(error: NetError) -> LinkError {
    match error {
        NetError::BadFrame => LinkError::BadFrame,
        NetError::Timeout => LinkError::Timeout,
        _ => LinkError::Io,
    }
}

/// `LinkError` của crate chính sách → `NetError` của driver — **giữ nguyên ba mã lỗi** TAP in.
fn to_net_error(error: LinkError) -> NetError {
    match error {
        LinkError::BadFrame => NetError::BadFrame,
        LinkError::Timeout => NetError::Timeout,
        LinkError::Io => NetError::Io,
    }
}

/// Cấu hình IPv4 lấy được qua DHCP (giữ nguyên kiểu cũ để ảnh không phải đổi).
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

impl From<DhcpLease> for NetConfig4 {
    fn from(lease: DhcpLease) -> Self {
        Self {
            ip: lease.ip,
            gw: lease.gw,
            mask: lease.mask,
            server: lease.server,
        }
    }
}

/// `xid` cố định của DHCP client aarch64 (chưa có RNG) — giá trị cũ, giữ nguyên.
const DHCP_XID: u32 = 0x4b54_3235;

/// **DHCP client** đầy đủ: DISCOVER → OFFER → REQUEST → ACK, mỗi pha có ngân sách `timeout_us`.
///
/// Adapter giữ **hành vi cũ**: một lần gửi mỗi pha, không thử lại.
pub fn dhcp_acquire(timeout_us: u64) -> Result<NetConfig4, NetError> {
    let clock = Micro;
    let mut wire = Wire;
    let budget = RetryBudget::new(timeout_us.saturating_mul(2), timeout_us, 1);
    let lease =
        dhcp_client::dhcp_acquire_with(&clock, &mut wire, MacAddr::new(mac()), DHCP_XID, budget)
            .map_err(to_net_error)?;
    Ok(NetConfig4::from(lease))
}

/// **ARP resolve**: hỏi MAC của `target_ip` trong `timeout_us` (fail-closed).
pub fn arp_resolve(
    our_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    timeout_us: u64,
) -> Result<MacAddr, NetError> {
    let clock = Micro;
    let mut wire = Wire;
    arp_client::arp_resolve(
        &clock,
        &mut wire,
        our_ip,
        MacAddr::new(mac()),
        target_ip,
        timeout_us,
    )
    .map_err(to_net_error)
}

/// **ICMP echo** `count` lần tới `target_ip` qua `target_mac` (đã resolve bằng ARP).
///
/// `Err(NetError::Timeout)` nếu **không** nhận được reply nào; `Err(NetError::Io)` nếu nhận được
/// ít hơn `count` (mất gói — không giấu). Mỗi echo có ngân sách `timeout_us` như bản cũ.
pub fn ping(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    target_mac: MacAddr,
    count: u32,
    timeout_us: u64,
) -> Result<PingStats, NetError> {
    let clock = Micro;
    let mut wire = Wire;
    let peer = NetPeer::new(our_ip, our_mac, target_ip, target_mac);
    let budget = RetryBudget::new(timeout_us, timeout_us, 1);
    icmp_client::ping_with(&clock, &mut wire, peer, count, budget).map_err(to_net_error)
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
    let mut wire = Wire;
    udp_client::send(
        &mut wire, our_ip, our_mac, dst_ip, dst_mac, src_port, dst_port, payload,
    )
    .map_err(to_net_error)
}

/// Chờ **một** datagram UDP khớp `src_ip:src_port` → `our_ip:dst_port`, tối đa `timeout_us`.
pub fn udp_recv(
    our_ip: Ipv4Addr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    out: &mut [u8],
    timeout_us: u64,
) -> Result<usize, NetError> {
    let clock = Micro;
    let mut wire = Wire;
    udp_client::recv(
        &clock, &mut wire, our_ip, src_ip, src_port, dst_port, out, timeout_us,
    )
    .map_err(to_net_error)
}

/// Gửi `payload` tới `dst_ip:dst_port` và chờ **đúng** payload đó quay lại (echo).
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
    let clock = Micro;
    let mut wire = Wire;
    udp_client::echo(
        &clock, &mut wire, our_ip, our_mac, dst_ip, dst_mac, src_port, dst_port, payload,
        timeout_us,
    )
    .map_err(to_net_error)
}

/// Gửi truy vấn DNS `A` tới `dns_ip:53` và parse reply.
///
/// Tối đa [`vifast_net::dns::DNS_MAX_ATTEMPTS`] lần, mỗi lần `timeout_us`; **D2-246**: reply hỏng
/// được **đếm rồi thử lại** trong cùng ngân sách thay vì bỏ cả truy vấn (hành vi mới so với bản
/// aarch64 cũ — xem journal KT-335b §hợp đồng). Chế độ `expect_a = None` giữ đúng hành vi cũ:
/// reply NXDOMAIN/NODATA vẫn đọc được (chỉ đòi `id`/`QR`/câu hỏi đúng).
///
/// `retry` là bộ đếm của **lần gọi này**: sau khi trả về, `retry.attempts()` là số lần đã gửi —
/// caller in nó vào evidence, **kể cả** khi lỗi.
#[allow(clippy::too_many_arguments)] // hai đầu cuối + id + tên + timeout + bộ đếm (chữ ký cũ)
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
    let clock = Micro;
    let mut wire = Wire;
    let peer = NetPeer::new(our_ip, our_mac, dns_ip, dns_mac);
    let budget = RetryBudget::new(
        timeout_us.saturating_mul(u64::from(DNS_MAX_ATTEMPTS)),
        timeout_us,
        u32::from(DNS_MAX_ATTEMPTS),
    );
    let mut reply = [0u8; DNS_MAX_LEN];
    dns_client::dns_query_with(
        &clock, &mut wire, peer, id, name, None, budget, &mut reply, retry,
    )
    .map_err(to_net_error)
}

// API khung/đọc reply: **re-export** từ crate chung để host test và ảnh dùng cùng một đường.
pub use vifast_net::arp_client::{arp_build_request, arp_parse_reply};
pub use vifast_net::dhcp_client::{dhcp_frame, dhcp_frame_parse, DhcpLease};
pub use vifast_net::dns_client::{
    dns_filter_selfcheck, dns_reply_accept, DnsAnswer, DnsReply, DNS_CLIENT_PORT, DNS_FRAME_BYTES,
    DNS_MAX_LEN, DNS_PORT,
};
pub use vifast_net::icmp_client::{
    icmp_echo_frame, icmp_filter_selfcheck, icmp_reply_accept, icmp_reply_frame, PingStats,
    ICMP_ECHO_ID, ICMP_ECHO_SEQ, ICMP_FRAME_BYTES, ICMP_PAYLOAD_BYTES,
};
pub use vifast_net::udp_client::{
    UdpEcho, UDP_CLIENT_PORT, UDP_ECHO_PAYLOAD, UDP_ECHO_PORT, UDP_MAX_PAYLOAD,
};

/// Địa chỉ DNS server của slirp (`-netdev user`): `10.0.2.3` (dữ kiện nền tảng QEMU).
pub const DNS_SERVER_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 3);

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
