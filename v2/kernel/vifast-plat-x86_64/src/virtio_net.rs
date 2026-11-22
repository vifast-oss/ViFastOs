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
//! 4. **adapter** chính sách client của ảnh (ARP/DHCP + ICMP/DNS ở [`crate::net_client`]) —
//!    **KT-335b** dời chính sách về `vifast_net` (host test được); tệp này chỉ còn `NetIo` +
//!    cầu `RefCell` sang `Clock`/`Link` + hàm mỏng giữ nguyên chữ ký cũ.
//!
//! Hành vi/TAP giữ **nguyên**: cùng chữ ký hàm, cùng mã lỗi, cùng bố cục trang DMA.

#![allow(unsafe_code)] // D2-17: `kernel/vifast-plat-*`; MMIO + vùng DMA (như bản trước KT-335).

use core::cell::{RefCell, UnsafeCell};
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

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::arp_client;
use vifast_net::client::{Clock as PolicyClock, Link, LinkError};
use vifast_net::dhcp_client;

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
// KT-335b — adapter chính sách ARP/DHCP (+ ICMP/DNS ở `net_client`) trên `vifast-net`
//
// Trước KT-335b, tệp này + `net_client.rs` giữ **bản sao thứ ba** của chính sách DHCP/ARP/ICMP/
// DNS (KT-326/KT-328). Sau KT-335b, chính sách nằm **một** chỗ trong `vifast-net` (host test
// được — D2-05); tệp này chỉ còn:
//
// 1. [`NetIo`] — nguồn frame/đồng hồ của x86_64 (driver thật hoặc fake trong host test);
// 2. [`Bridge`] — cầu `NetIo` → hai trait chính sách `Clock`/`Link` (dùng `RefCell` vì
//    `Clock::now(&self)` còn `Link::send_frame(&mut self)`);
// 3. hàm mỏng giữ **nguyên** chữ ký cũ cho ảnh + host test.
//
// # Ba trần giữ nguyên (không nới)
//
// x86_64 là bản **đã có** hợp đồng ba trần từ KT-326/KT-328; KT-335b lấy nó làm hợp đồng chung
// (`RetryBudget`) và **giữ nguyên số**: DHCP 6 s / 1 s / 3 lần mỗi pha, ICMP 6 s / 1 s / 3 lần,
// DNS 6 s / 3 s / 2 lần. TAP 31/32/33 vì vậy vẫn in `attempts`/`elapsed` trong cùng khoảng cũ.
// ================================================================================

/// Nguồn khung/đồng hồ của **một bước mạng** (DHCP client, ICMP echo, DNS query) — tách khỏi
/// driver để **host test** bơm frame giả và chứng minh trần thời gian mà không cần QEMU.
///
/// **KT-328**: ba bước mạng dùng **cùng** một trait; tên cũ [`DhcpIo`] giữ làm bí danh để call-site
/// của KT-326 không phải đổi. **KT-335b**: trait này là **adapter** của x86_64; chính sách dùng
/// chung nằm ở `vifast_net` trên `Clock` + `Link`.
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
/// `pub(crate)` vì [`crate::net_client`] (ICMP/DNS) dùng **cùng** một nguồn frame/đồng hồ — không
/// có bản thứ hai.
pub(crate) struct VirtioNetIo<'a> {
    now: &'a mut dyn FnMut() -> u64,
}

impl<'a> VirtioNetIo<'a> {
    /// Bọc đồng hồ của ảnh thành nguồn frame/đồng hồ của một bước mạng.
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

/// Cầu `NetIo` → `Clock` + `Link` của crate chính sách.
///
/// `RefCell` là cần thiết vì `Clock::now(&self)` (đồng hồ đơn điệu) còn `Link::send_frame(&mut
/// self)` (đường dây) — hai trait tách nhau từ KT-335, trong khi x86_64 gộp cả hai vào **một**
/// `NetIo`. Mượn `RefCell` chỉ tồn tại trong từng lời gọi (không lồng) nên không thể panic
/// `BorrowMutError`; chính sách chỉ gọi `clock.now()` **rồi** `link.recv_frame(...)` tuần tự.
pub(crate) struct Bridge<'a> {
    io: RefCell<&'a mut dyn NetIo>,
}

impl<'a> Bridge<'a> {
    pub(crate) fn new(io: &'a mut dyn NetIo) -> Self {
        Self {
            io: RefCell::new(io),
        }
    }

    pub(crate) fn clock(&self) -> BridgeClock<'_, 'a> {
        BridgeClock(self)
    }

    pub(crate) fn link(&self) -> BridgeLink<'_, 'a> {
        BridgeLink(self)
    }
}

/// Nửa `Clock` của [`Bridge`].
pub(crate) struct BridgeClock<'a, 'b>(&'b Bridge<'a>);

impl PolicyClock for BridgeClock<'_, '_> {
    fn now(&self) -> u64 {
        self.0.io.borrow_mut().now_us()
    }
}

/// Nửa `Link` của [`Bridge`].
pub(crate) struct BridgeLink<'a, 'b>(&'b Bridge<'a>);

impl Link for BridgeLink<'_, '_> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        // Trần poll `used` của driver là 2 s (POLL_TIMEOUT_US) — không phải ngân sách chính sách;
        // chính sách đo ngân sách **sau** khi gửi (một đồng hồ, một trần).
        self.0
            .io
            .borrow_mut()
            .send(frame, POLL_TIMEOUT_US)
            .map_err(to_link_error)
    }

    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize> {
        self.0.io.borrow_mut().recv(out, timeout)
    }
}

/// `NetError` của driver → `LinkError` của crate chính sách (ba tên trùng nhau).
///
/// `NotReady`/`NoDevice`/`QueueFull` không có nghĩa tương đương trong crate logic; ánh xạ `Io`
/// là fail-closed (đường thành công không đi qua đây: `init` đã kiểm `ready`).
pub(crate) fn to_link_error(error: NetError) -> LinkError {
    match error {
        NetError::BadFrame => LinkError::BadFrame,
        NetError::Timeout => LinkError::Timeout,
        _ => LinkError::Io,
    }
}

/// `LinkError` của crate chính sách → `NetError` của driver — **giữ nguyên ba mã lỗi** TAP in.
pub(crate) fn to_net_error(error: LinkError) -> NetError {
    match error {
        LinkError::BadFrame => NetError::BadFrame,
        LinkError::Timeout => NetError::Timeout,
        LinkError::Io => NetError::Io,
    }
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
    let mut io = VirtioNetIo::new(now);
    let bridge = Bridge::new(&mut io);
    let clock = bridge.clock();
    let mut link = bridge.link();
    arp_client::arp_resolve(
        &clock,
        &mut link,
        our_ip,
        MacAddr::new(mac()),
        target_ip,
        timeout_us,
    )
    .map_err(to_net_error)
}

/// **DHCP client** đầy đủ: DISCOVER → OFFER → REQUEST → ACK, với **ba trần** ở đầu tệp.
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

/// Máy trạng thái DHCP trên một [`NetIo`] bất kỳ (driver thật hoặc fake trong test host).
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
    let bridge = Bridge::new(io);
    let clock = bridge.clock();
    let mut link = bridge.link();
    dhcp_client::dhcp_acquire_with(
        &clock,
        &mut link,
        mac,
        xid,
        RetryBudget::new(total_timeout_us, attempt_timeout_us, max_attempts),
    )
    .map_err(to_net_error)
}

/// Trần **toàn bộ** bước DHCP (µs): 6 s — dưới ngân sách boot 20 s của runner.
pub const DHCP_TOTAL_TIMEOUT_US: u64 = 6_000_000;
/// Trần **mỗi lần gửi/chờ** (µs): 1 s.
pub const DHCP_ATTEMPT_TIMEOUT_US: u64 = 1_000_000;
/// Số lần thử **mỗi pha** (DISCOVER, REQUEST) — trần, không retry vô hạn.
pub const DHCP_MAX_ATTEMPTS: u32 = 3;
/// `xid` cố định của client (chưa có RNG); reply còn phải khớp `chaddr` = MAC của ta.
pub const DHCP_XID: u32 = 0x4b54_3332;

// Khung/đọc reply DHCP: **re-export** từ crate chung (một codec, một bộ lọc).
pub use vifast_net::arp_client::{arp_build_request, arp_parse_reply};
pub use vifast_net::client::udp_frame;
pub use vifast_net::dhcp_client::{dhcp_filter_selfcheck, dhcp_frame, dhcp_frame_parse, DhcpLease};

/// Đọc lease từ ACK trên **API cũ** của x86_64: lỗi trả `NetError` (ba tên trùng crate chính
/// sách) để ảnh/host test không phải đổi kiểu lỗi.
pub fn dhcp_lease_from_ack(
    ack: &vifast_net::dhcp::DhcpMessage,
    attempts: u32,
) -> Result<DhcpLease, NetError> {
    vifast_net::dhcp_client::dhcp_lease_from_ack(ack, attempts).map_err(to_net_error)
}

/// **Re-export** của [`crate::net_client`] (KT-328 → KT-335b) để ảnh và host test dùng **một**
/// đường (`vifast_plat_x86_64::virtio_net::{ping, dns_query, …}`) — không có API thứ hai cho
/// ICMP/DNS.
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
