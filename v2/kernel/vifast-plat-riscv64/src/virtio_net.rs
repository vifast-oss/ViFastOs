// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter** `virtio-net` của riscv64 — KT-335 (trước: driver đầy đủ 1.338 dòng, KT-245).
//!
//! Toàn bộ quyết định giao thức virtio (status/feature/hai queue, descriptor, notify, poll
//! `used`, `virtio_net_config`, `virtio_net_hdr`, `used->len`) nằm ở
//! [`vifast_virtio::netdev`] — crate **thuần**, test host được (D2-05). Tệp này chỉ còn:
//!
//! 1. [`Mmio`] — đọc/ghi BAR virtio bằng con trỏ thô + `fence rw, rw` (D2-17: `unsafe` ở
//!    `kernel/vifast-plat-*`, không ở crate logic);
//! 2. [`Dma`] — một trang 4 KiB trong `.bss` (identity map ⇒ `VA == PA`), hai hook cache là no-op
//!    **có chủ đích** (riscv64 không có cache không nhất quán với DMA);
//! 3. [`Tick`] — đồng hồ timebase của `Riscv64Virt`;
//! 4. **adapter** chính sách client của ảnh (DHCP/ARP/ICMP/UDP/DNS) — **KT-335b** dời chính
//!    sách về `vifast_net::{dhcp,arp,icmp,udp,dns}_client` (host test được); tệp này chỉ còn
//!    `Tick`/`Wire` + hàm mỏng giữ **nguyên** chữ ký cũ cho ảnh.
//!
//! Hành vi/TAP giữ **nguyên**: cùng chữ ký hàm, cùng mã lỗi, cùng bố cục trang DMA — ba thứ ấy
//! do `vifast_virtio::net`/`netdev` quyết định và test host chốt.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::read_volatile;

use vifast_plat_common::timer::TIMEBASE_HZ;
use vifast_plat_common::Platform;
use vifast_virtio::blkdev::Transport;
use vifast_virtio::dma::{DmaBuf, DmaPool};
use vifast_virtio::mmio::MmioBus;
use vifast_virtio::netdev::{Clock, Net, NetReport, Stats};

// Mã lỗi và giới hạn frame: **một** nguồn ở crate thuần (D2-110B).
pub use vifast_virtio::net::{
    hdr_bytes, NetError, HDR_MODERN_BYTES, MAX_FRAME_BYTES, QUEUE_SIZE, RX_BUF_BYTES, TX_BUF_BYTES,
};
use vifast_virtio::queue::VqLayout;
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

use crate::pci::PciReport;
use crate::Riscv64Virt;

/// Timeout poll `used` (tick timebase) — 2 s ảo (cùng quy ước KT-72/73).
pub const POLL_TIMEOUT_TICKS: u64 = 2 * TIMEBASE_HZ;

// ---------------------------------------------------------------- trang DMA (identity map)

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: ảnh chạy một hart trên đường driver và driver tự tuần tự hoá (một frame một lúc mỗi
// hướng); vùng này không ai khác (trừ DMA của thiết bị) giữ tham chiếu.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA` (TAP kiểm bằng `walk_sw`).
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
        // SAFETY: `addr` nằm trong BAR đã gán/map Device RW (kiểm bằng walk trong TAP).
        unsafe { read_volatile(addr as *const u8) }
    }
    #[inline]
    fn read16(&self, addr: u64) -> u16 {
        // SAFETY: như `read8`; bán kính 16 bit căn 2.
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
        unsafe { core::ptr::write_volatile(addr as *mut u8, value) };
    }
    #[inline]
    fn write16(&mut self, addr: u64, value: u16) {
        // SAFETY: như `write8`; căn 2.
        unsafe { core::ptr::write_volatile(addr as *mut u16, value) };
    }
    #[inline]
    fn write32(&mut self, addr: u64, value: u32) {
        // SAFETY: như `write8`; căn 4.
        unsafe { core::ptr::write_volatile(addr as *mut u32, value) };
    }
    #[inline]
    fn write64(&mut self, addr: u64, value: u64) {
        // SAFETY: như `write8`; căn 8.
        unsafe { core::ptr::write_volatile(addr as *mut u64, value) };
    }
    #[inline]
    fn fence(&self) {
        // SAFETY: `fence` chỉ là hàng rào thứ tự, không truy cập địa chỉ.
        unsafe { asm!("fence rw, rw", options(nostack, preserves_flags)) };
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
        // riscv64: identity map, không có cache không nhất quán với DMA ⇒ no-op **có chủ đích**.
    }
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {
        // Như `sync_for_device`.
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

struct Tick;

impl Clock for Tick {
    fn now(&mut self) -> u64 {
        Riscv64Virt.now_ticks()
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

/// Gửi **một** frame Ethernet (không kể header virtio).
pub fn send_frame(frame: &[u8]) -> Result<(), NetError> {
    NET.send_frame(&mut Mmio, &mut Dma, &mut Tick, frame, POLL_TIMEOUT_TICKS)
}

/// Nhận **một** frame nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có frame.
pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    NET.recv_frame(&mut Mmio, &mut Dma, &mut Tick, out)
}

/// Nhận **một** frame, chờ tối đa `timeout_ticks` tick timebase. `None` = hết hạn / chưa có.
pub fn recv_frame_wait(out: &mut [u8], timeout_ticks: u64) -> Option<usize> {
    NET.recv_frame_wait(&mut Mmio, &mut Dma, &mut Tick, out, timeout_ticks)
}

/// MAC đọc từ thiết bị (toàn 0 nếu chưa init).
pub fn mac() -> [u8; 6] {
    NET.mac()
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    NET.ready()
}

/// Địa chỉ ISR (đọc 1 byte để lấy lý do ngắt — chưa dùng ngắt, chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    NET.isr_addr()
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều frame).
pub fn device_status() -> u8 {
    NET.device_status(&mut Mmio)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    NET.config_generation(&mut Mmio)
}

// ================================================================================
// KT-335b — chính sách DHCP + ARP + ICMP + UDP + DNS dùng chung (`vifast_net::*_client`)
//
// Trước KT-335b, khối này là **bản sao thứ nhất** trong ba bản (`vifast-plat-*`): cùng codec,
// cùng điều kiện nhận, chỉ khác đồng hồ (tick timebase ở đây). Sau KT-335b, chính sách nằm
// **một** chỗ trong `vifast-net` (host test được — D2-05); tệp này chỉ còn:
//
// 1. [`Tick`] — đồng hồ timebase của `Riscv64Virt` (đơn vị **tick**);
// 2. [`Wire`] — đường frame trên `send_frame`/`recv_frame_wait` của driver;
// 3. hàm mỏng giữ **nguyên** chữ ký cũ cho ảnh `ktest`/`ktest-netsrv` (TAP không đổi).
//
// # Ba trần giữ nguyên cho riscv64 (không nới)
//
// Bản cũ: **một** ngân sách 3 s ảo cho mỗi pha DHCP / mỗi echo / mỗi lần DNS, **không** gửi lại
// (DNS: tối đa 2 lần theo KT-294). Adapter truyền `RetryBudget` đúng như vậy
// (`max_attempts = 1` cho DHCP/ICMP), nên **hành vi timeout của riscv64 không đổi**.
// ================================================================================

/// Timeout mặc định cho **mỗi** bước mạng (DHCP/ARP/ping) — 3 s ảo (D2-196 mục (1)).
pub const NET_STEP_TIMEOUT_TICKS: u64 = 3 * TIMEBASE_HZ;

impl PolicyClock for Tick {
    fn now(&self) -> u64 {
        Riscv64Virt.now_ticks()
    }
}

/// Đường frame của driver: gửi/nhận **một** frame Ethernet trên virtio-net.
struct Wire;

impl Link for Wire {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        send_frame(frame).map_err(to_link_error)
    }

    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize> {
        recv_frame_wait(out, timeout)
    }
}

/// `NetError` của driver → `LinkError` của crate chính sách (ba tên trùng nhau).
///
/// `NotReady`/`NoDevice`/`QueueFull` **không** có nghĩa tương đương trong crate logic; ánh xạ
/// `Io` là fail-closed (đường thành công không bao giờ đi qua đây: `init` đã kiểm `ready`, và
/// queue `tx` được thu hồi trước mỗi frame).
fn to_link_error(error: NetError) -> LinkError {
    match error {
        NetError::BadFrame => LinkError::BadFrame,
        NetError::Timeout => LinkError::Timeout,
        _ => LinkError::Io,
    }
}

/// `LinkError` của crate chính sách → `NetError` của driver — **giữ nguyên ba mã lỗi** mà TAP in.
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

/// `xid` cố định của DHCP client riscv64 (chưa có RNG) — giá trị cũ, giữ nguyên.
const DHCP_XID: u32 = 0x4b54_3234;

/// **DHCP client** đầy đủ: DISCOVER → OFFER → REQUEST → ACK, mỗi pha có ngân sách `timeout_ticks`.
///
/// Adapter giữ **hành vi cũ**: một lần gửi mỗi pha, không thử lại. Ba trần của hợp đồng chung
/// (`RetryBudget`) được truyền `{ total = 2 × timeout, attempt = timeout, max_attempts = 1 }`.
pub fn dhcp_acquire(timeout_ticks: u64) -> Result<NetConfig4, NetError> {
    let clock = Tick;
    let mut wire = Wire;
    let budget = RetryBudget::new(timeout_ticks.saturating_mul(2), timeout_ticks, 1);
    let lease =
        dhcp_client::dhcp_acquire_with(&clock, &mut wire, MacAddr::new(mac()), DHCP_XID, budget)
            .map_err(to_net_error)?;
    Ok(NetConfig4::from(lease))
}

/// **ARP resolve**: hỏi MAC của `target_ip` trong `timeout_ticks` (fail-closed).
pub fn arp_resolve(
    our_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    timeout_ticks: u64,
) -> Result<MacAddr, NetError> {
    let clock = Tick;
    let mut wire = Wire;
    arp_client::arp_resolve(
        &clock,
        &mut wire,
        our_ip,
        MacAddr::new(mac()),
        target_ip,
        timeout_ticks,
    )
    .map_err(to_net_error)
}

/// **ICMP echo** `count` lần tới `target_ip` qua `target_mac` (đã resolve bằng ARP).
///
/// `Err(NetError::Timeout)` nếu **không** nhận được reply nào; `Err(NetError::Io)` nếu nhận được
/// ít hơn `count` (mất gói — không giấu). Mỗi echo có ngân sách `timeout_ticks` như bản cũ.
pub fn ping(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    target_mac: MacAddr,
    count: u32,
    timeout_ticks: u64,
) -> Result<PingStats, NetError> {
    let clock = Tick;
    let mut wire = Wire;
    let peer = NetPeer::new(our_ip, our_mac, target_ip, target_mac);
    let budget = RetryBudget::new(timeout_ticks, timeout_ticks, 1);
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

/// Chờ **một** datagram UDP khớp `src_ip:src_port` → `our_ip:dst_port`, tối đa `timeout_ticks`.
pub fn udp_recv(
    our_ip: Ipv4Addr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    out: &mut [u8],
    timeout_ticks: u64,
) -> Result<usize, NetError> {
    let clock = Tick;
    let mut wire = Wire;
    udp_client::recv(
        &clock,
        &mut wire,
        our_ip,
        src_ip,
        src_port,
        dst_port,
        out,
        timeout_ticks,
    )
    .map_err(to_net_error)
}

/// Gửi `payload` tới `dst_ip:dst_port` và chờ **đúng** payload đó quay lại (echo).
#[allow(clippy::too_many_arguments)] // hai đầu cuối + 2 cổng + payload + timeout (chữ ký cũ, KT-328)
pub fn udp_echo(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    timeout_ticks: u64,
) -> Result<UdpEcho, NetError> {
    let clock = Tick;
    let mut wire = Wire;
    udp_client::echo(
        &clock,
        &mut wire,
        our_ip,
        our_mac,
        dst_ip,
        dst_mac,
        src_port,
        dst_port,
        payload,
        timeout_ticks,
    )
    .map_err(to_net_error)
}

/// Gửi truy vấn DNS `A` tới `dns_ip:53` và parse reply.
///
/// Tối đa [`vifast_net::dns::DNS_MAX_ATTEMPTS`] lần, mỗi lần `timeout_ticks`; **D2-246**: reply
/// hỏng được **đếm rồi thử lại** trong cùng ngân sách thay vì bỏ cả truy vấn (hành vi mới so với
/// bản riscv64 cũ — xem journal KT-335b §hợp đồng).
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
    timeout_ticks: u64,
    retry: &mut DnsRetry,
) -> Result<DnsAnswer, NetError> {
    let clock = Tick;
    let mut wire = Wire;
    let peer = NetPeer::new(our_ip, our_mac, dns_ip, dns_mac);
    let budget = RetryBudget::new(
        timeout_ticks.saturating_mul(u64::from(DNS_MAX_ATTEMPTS)),
        timeout_ticks,
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
pub use vifast_net::udp_client::UdpEcho;
pub use vifast_net::udp_client::{
    UDP_CLIENT_PORT, UDP_ECHO_PAYLOAD, UDP_ECHO_PORT, UDP_MAX_PAYLOAD,
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
    assert!(HDR_MODERN_BYTES <= RX_BUF_BYTES);
};
