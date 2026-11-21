// SPDX-License-Identifier: MIT OR Apache-2.0
//! **virtio-net + DHCP + ARP + ICMP + TCP + DNS của ảnh aarch64** — KT-275 (K4' bước 3, D1 trên
//! aarch64) + KT-289 (TCP client + DNS).
//!
//! # Vì sao chạy ở `el0_done` (sau U-mode, sau virtio-blk)
//!
//! TAP phải **tuần tự**: KT-273 đã chiếm TAP 19/20/21 cho PCI/virtio-blk, nên mạng là 22/23/24/25
//! và phải in **sau** `ok 21`. `el0_done` chạy ở EL1h với **cùng** `VSpace` đang hoạt động, nên
//! MMIO tới ECAM/BAR đi qua bảng trang đã map trong `high_entry` — không cần `&mut VSpace` ở đây.
//!
//! # Bảy phép đo (mỗi phép một dòng `#` + một TAP)
//!
//! 1. `# net: device …` — tìm `virtio-net` trên bus PCI (ECAM của KT-273), gán BAR trong leaf
//!    **riêng** của loại Net (không đụng leaf blk), khởi tạo driver, đọc **MAC thật** từ
//!    `virtio_net_config`.
//! 2. `# net: dhcp ip=… gw=… mask=… server=…` — DHCP **thật** của slirp (DISCOVER/OFFER/
//!    REQUEST/ACK), cùng khuôn `net.sh` riscv64 (KT-246).
//! 3. `# net: arp gw=<mac>` — ARP reply **thật** của slirp cho gateway.
//! 4. `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — ICMP echo **thật** qua gateway.
//! 5. `# net: tcp connect 10.0.2.2:45678 ok rtt=<ms> state=established` — **KT-289**: bắt tay ba
//!    bước **thật** với server TCP trên host (slirp ánh xạ `10.0.2.2` → loopback host).
//! 6. `# net: tcp recv=13 first=vifast-tcp-ok` + `# net tcp data: … send=ok close=ok` — **KT-289**:
//!    nhận **đúng** chuỗi server gửi, gửi lại payload, đóng bằng FIN.
//! 7. `# net: dns example.com a=<ip> ttl=<n>` — **KT-289**: truy vấn DNS `A` **thật** tới
//!    `10.0.2.3:53` của slirp (codec `vifast_net::dns`, không viết lại).
//!
//! # Không có thiết bị ⇒ **ĐỎ**, không im lặng
//!
//! `pci::init_for` trả `Err(NoDevice)` ⇒ in `# net: no device` (dòng **nguyên văn** mà runner
//! dùng cho ca âm "không có mạng") rồi cả bảy TAP đỏ ⇒ `el0_done` dừng **trước** marker. Nhờ vậy
//! một lần boot thiếu `-device virtio-net-pci` **không** thể trông giống boot sạch.
//!
//! # Codec: **không** viết lại
//!
//! Ethernet/ARP/IPv4/ICMP/UDP/DHCP/TCP/DNS là của `vifast-net` (KT-244/KT-260/KT-268, host test);
//! tệp này chỉ gọi driver `vifast_plat_aarch64::virtio_net`/`tcp` và in bằng chứng.

use core::fmt::{self, Write};
use core::sync::atomic::Ordering;

/// **KT-303** — TAP 35 (DNS) đỏ ⇒ ảnh **không** in marker nhưng **vẫn chạy** các bước sau.
pub static DNS_RED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Đã có TAP đỏ vì DNS chưa (dùng để chặn marker).
pub fn dns_red() -> bool {
    DNS_RED.load(core::sync::atomic::Ordering::Relaxed)
}

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::dns::DnsRetry;
use vifast_net::tcp::State;
use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::pci::{self, bar_kind_name, PciError, VirtioKind};
use vifast_plat_aarch64::tcp;
use vifast_plat_aarch64::virtio_net::{self, NetConfig4, NetReport};
use vifast_virtio::features::DRIVER_OK;

use crate::{tap, Console, TEXT_START_PA};

/// Số lần echo request của phép đo ping (khớp `n=3` mà runner đòi).
pub const PING_COUNT: u32 = 3;

/// Cổng server TCP trên host — khớp `tcp::TCP_SERVER_PORT` của driver và `TCP_PORT` của runner.
pub const TCP_SERVER_PORT: u16 = tcp::TCP_SERVER_PORT;
/// Payload khách gửi lại server (15 byte — runner đối chiếu `recv=15` trong log server).
pub const TCP_SEND_PAYLOAD: &[u8] = b"vifast-tcp-send";
/// Số byte đầu của chuỗi nhận được in vào evidence (đủ để đối chiếu `vifast-tcp-ok`).
pub const TCP_FIRST_BYTES: usize = 32;
/// `id` cố định của truy vấn DNS (chưa có RNG; còn kiểm `id` trong reply).
pub const DNS_ID_A: u16 = 0x4b55;
/// Tên truy vấn DNS `A`.
pub const DNS_NAME_A: &[u8] = b"example.com";

/// Tên trạng thái TCP để in evidence (khớp `vifast_net::tcp::State`).
fn state_name(state: State) -> &'static str {
    match state {
        State::Closed => "closed",
        State::SynSent => "syn-sent",
        State::Established => "established",
        State::FinWait1 => "fin-wait-1",
        State::FinWait2 => "fin-wait-2",
        State::Closing => "closing",
        State::TimeWait => "time-wait",
        State::LastAck => "last-ack",
    }
}

/// In MAC dạng `aa:bb:cc:dd:ee:ff` (không cần `alloc`).
struct MacText([u8; 6]);

impl fmt::Display for MacText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            m[0], m[1], m[2], m[3], m[4], m[5]
        )
    }
}

/// In IPv4 dạng `a.b.c.d`.
struct IpText(Ipv4Addr);

impl fmt::Display for IpText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let o = self.0.octets();
        write!(f, "{}.{}.{}.{}", o[0], o[1], o[2], o[3])
    }
}

/// Chạy bốn phép đo mạng, in TAP 22/23/24/25. Trả `true` ⇔ **cả bốn** xanh.
///
/// In **đủ** bốn dòng TAP kể cả khi phép trước đỏ: log phải cho biết **phép nào** hỏng, không
/// dừng ở phép đầu (một ảnh không có mạng vẫn phải chứng minh PCI/ECAM chạy).
pub fn run(console: &mut Console) -> bool {
    let mut all_ok = true;

    // ---------------------------------------------------------------- TAP 22: thiết bị + MAC
    let mut report: Option<NetReport> = None;
    match pci::init_for(VirtioKind::Net) {
        Ok(found) => {
            // Trang DMA nằm trong `.bss` của ảnh: sau `activate` con trỏ là **alias nửa cao**,
            // còn thiết bị DMA bằng **PA** ⇒ đổi bằng `high_to_pa` (cùng công thức `blk` dùng).
            let text_start = TEXT_START_PA.load(Ordering::Relaxed);
            let dma_pa = virtio_net::high_to_pa(asm::dma_page_net_addr(), text_start);
            match virtio_net::init(&found, dma_pa) {
                Ok(net) => {
                    let _ = writeln!(
                        console,
                        "# net: device bus={} dev={} func={} vendor=0x{:04x} device=0x{:04x} bar={} kind={} size=0x{:x} assigned=0x{:x} readback=0x{:x} mac={} mtu={} status=0x{:04x} queues={} rx={} tx={} features=0x{:016x} driver=0x{:016x} dma=0x{:x}",
                        found.bus,
                        found.dev,
                        found.func,
                        found.vendor,
                        found.device,
                        found.bar_index,
                        bar_kind_name(found.bar_kind),
                        found.bar_size,
                        found.bar_assigned,
                        found.bar_readback,
                        MacText(net.config.mac),
                        net.config.effective_mtu(),
                        net.config.status,
                        net.num_queues,
                        net.queue_size_rx,
                        net.queue_size_tx,
                        net.device_features,
                        net.driver_features,
                        net.dma_pa,
                    );
                    report = Some(net);
                }
                Err(stage) => {
                    let _ = writeln!(console, "# net: init failed stage={stage}");
                }
            }
        }
        Err(fail) => {
            // Không có thiết bị **khác** lỗi cấu hình: ca âm "không có mạng" của runner grep
            // đúng dòng `# net: no device`, nên chỉ in nó khi nguyên nhân **là** `NoDevice`.
            if fail.error == PciError::NoDevice {
                let _ = writeln!(console, "# net: no device");
            } else {
                let _ = writeln!(
                    console,
                    "# net: pci failed stage={} error={}",
                    fail.stage,
                    fail.error.name()
                );
            }
        }
    }
    // MAC chỉ được tin khi đã đàm phán `VIRTIO_NET_F_MAC` và giá trị hợp lệ (không toàn
    // 0/broadcast); `device_status` đọc lại phải còn `DRIVER_OK`; hai queue phải dùng được.
    let device_ok = match &report {
        Some(net) => {
            net.mac_ok
                && net.config.mac_hop_le()
                && net.status & DRIVER_OK != 0
                && net.queue_size_rx >= 2
                && net.queue_size_tx >= 2
        }
        None => false,
    };
    if !tap(console, 22, "net: device", device_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 23: DHCP
    let mut config: Option<NetConfig4> = None;
    if report.is_some() {
        match virtio_net::dhcp_acquire(virtio_net::NET_STEP_TIMEOUT_US) {
            Ok(cfg) => {
                let _ = writeln!(
                    console,
                    "# net: dhcp ip={} gw={} mask={} server={}",
                    IpText(cfg.ip),
                    IpText(cfg.gw),
                    IpText(cfg.mask),
                    IpText(cfg.server),
                );
                config = Some(cfg);
            }
            Err(error) => {
                let _ = writeln!(console, "# net: dhcp error={}", error.name());
            }
        }
    } else {
        let _ = writeln!(console, "# net: dhcp skipped (no device)");
    }
    if !tap(console, 23, "net: dhcp", config.is_some()) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 24: ARP gateway
    let mut gateway_mac: Option<MacAddr> = None;
    if let Some(cfg) = &config {
        match virtio_net::arp_resolve(cfg.ip, cfg.gw, virtio_net::NET_STEP_TIMEOUT_US) {
            Ok(found) => {
                let _ = writeln!(console, "# net: arp gw={}", MacText(found.octets()));
                gateway_mac = Some(found);
            }
            Err(error) => {
                let _ = writeln!(
                    console,
                    "# net: arp gw={} error={}",
                    IpText(cfg.gw),
                    error.name()
                );
            }
        }
    } else {
        let _ = writeln!(console, "# net: arp skipped (no dhcp)");
    }
    // MAC toàn 0 / broadcast là giá trị vô nghĩa cho một host — không nhận là "resolve được".
    let arp_ok = gateway_mac
        .map(|mac| !mac.is_zero() && !mac.is_broadcast())
        .unwrap_or(false);
    if !tap(console, 24, "net: arp", arp_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 25: ICMP echo
    let mut ping_ok = false;
    if let (Some(cfg), Some(target_mac)) = (&config, gateway_mac) {
        let our_mac = MacAddr::new(virtio_net::mac());
        match virtio_net::ping(
            cfg.ip,
            our_mac,
            cfg.gw,
            target_mac,
            PING_COUNT,
            virtio_net::NET_STEP_TIMEOUT_US,
        ) {
            Ok(stats) => {
                let _ = writeln!(
                    console,
                    "# net: ping {} ok n={} rtt={}",
                    IpText(cfg.gw),
                    stats.received,
                    stats.rtt_max_ms,
                );
                ping_ok = stats.received == PING_COUNT;
            }
            Err(error) => {
                let _ = writeln!(
                    console,
                    "# net: ping {} error={}",
                    IpText(cfg.gw),
                    error.name()
                );
            }
        }
    } else {
        let _ = writeln!(console, "# net: ping skipped (no arp)");
    }
    if !tap(console, 25, "net: ping", ping_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 33: TCP connect
    // **Server thật trên host**: runner dựng `python3` lắng nghe `127.0.0.1:45678`; khách nối
    // `10.0.2.2:45678` — slirp ánh xạ `10.0.2.2` → loopback của host (`sotranslate_out4`).
    // `hostfwd` của QEMU là chiều host → guest nên **không** dùng được cho chiều này (KT-268).
    // Không có server ⇒ `tcp_connect` timeout 3 s (fail-closed, không treo).
    let mut tcp_conn: Option<tcp::TcpConn> = None;
    if let (Some(cfg), Some(gw_mac)) = (&config, gateway_mac) {
        let our_mac = MacAddr::new(virtio_net::mac());
        let started = virtio_net::now_us();
        match tcp::tcp_connect(
            cfg.ip,
            our_mac,
            cfg.gw,
            gw_mac,
            TCP_SERVER_PORT,
            virtio_net::NET_STEP_TIMEOUT_US,
        ) {
            Ok(conn) => {
                let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                let _ = writeln!(
                    console,
                    "# net: tcp connect {}:{} ok rtt={} state={}",
                    IpText(cfg.gw),
                    TCP_SERVER_PORT,
                    elapsed_ms,
                    state_name(conn.state())
                );
                let _ = writeln!(
                    console,
                    "# net tcp connect: iss=0x{:x} snd_una=0x{:x} rcv_nxt=0x{:x} retransmits={} \
                     elapsed_ms={elapsed_ms} const:timeout_ms={} const:port={}",
                    conn.iss(),
                    conn.snd_una(),
                    conn.rcv_nxt(),
                    conn.retries(),
                    virtio_net::NET_STEP_TIMEOUT_US / 1000,
                    TCP_SERVER_PORT
                );
                tcp_conn = Some(conn);
            }
            Err(error) => {
                let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                let _ = writeln!(
                    console,
                    "# net tcp connect: target={}:{} err={} elapsed_ms={elapsed_ms} \
                     const:timeout_ms={}",
                    IpText(cfg.gw),
                    TCP_SERVER_PORT,
                    error.name(),
                    virtio_net::NET_STEP_TIMEOUT_US / 1000
                );
                let _ = writeln!(console, "# net: no tcp");
            }
        }
    } else {
        let _ = writeln!(console, "# net: tcp skipped (no arp)");
    }
    if !tap(console, 33, "net: tcp connect", tcp_conn.is_some()) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 34: TCP data
    // Nhận **đúng** chuỗi server gửi, gửi lại payload, đóng bằng FIN. Server ghi `recv=15` trong
    // log của nó — bằng chứng **hai chiều** khách ↔ host (runner đối chiếu).
    let mut tcp_data_ok = false;
    if let Some(conn) = tcp_conn.as_mut() {
        let mut buf = [0u8; 64];
        let started = virtio_net::now_us();
        match tcp::tcp_recv(conn, &mut buf, virtio_net::NET_STEP_TIMEOUT_US) {
            Ok(n) if n > 0 => {
                // `first=` in **đúng byte nhận được** (ký tự không in được thay bằng '.'), tối đa
                // `TCP_FIRST_BYTES` — runner đối chiếu với chuỗi server gửi.
                let mut first = [0u8; TCP_FIRST_BYTES];
                let mut first_len = 0;
                while first_len < n && first_len < TCP_FIRST_BYTES {
                    let byte = buf[first_len];
                    first[first_len] = if (0x20..0x7f).contains(&byte) {
                        byte
                    } else {
                        b'.'
                    };
                    first_len += 1;
                }
                let first = core::str::from_utf8(&first[..first_len]).unwrap_or("?");
                let _ = writeln!(console, "# net: tcp recv={n} first={first}");
                let sent = tcp::tcp_send(conn, TCP_SEND_PAYLOAD, virtio_net::NET_STEP_TIMEOUT_US);
                let closed = tcp::tcp_close(conn, virtio_net::NET_STEP_TIMEOUT_US);
                let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                let _ = writeln!(
                    console,
                    "# net tcp data: recv={n} sent={} send={} close={} state={} retransmits={} \
                     elapsed_ms={elapsed_ms} const:timeout_ms={}",
                    TCP_SEND_PAYLOAD.len(),
                    match sent {
                        Ok(_) => "ok",
                        Err(error) => error.name(),
                    },
                    match closed {
                        Ok(()) => "ok",
                        Err(error) => error.name(),
                    },
                    state_name(conn.state()),
                    conn.retries(),
                    virtio_net::NET_STEP_TIMEOUT_US / 1000
                );
                tcp_data_ok = sent.is_ok() && closed.is_ok();
            }
            Ok(_) => {
                let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                let _ = writeln!(
                    console,
                    "# net tcp data: recv=0 elapsed_ms={elapsed_ms} state={}",
                    state_name(conn.state())
                );
                let _ = writeln!(console, "# net: no tcp");
            }
            Err(error) => {
                let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                let _ = writeln!(
                    console,
                    "# net tcp data: recv_err={} elapsed_ms={elapsed_ms} state={} \
                     const:timeout_ms={}",
                    error.name(),
                    state_name(conn.state()),
                    virtio_net::NET_STEP_TIMEOUT_US / 1000
                );
                let _ = writeln!(console, "# net: no tcp");
            }
        }
    } else {
        let _ = writeln!(console, "# net: tcp data skipped (no connect)");
    }
    if !tap(console, 34, "net: tcp data", tcp_data_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 35: DNS
    // Truy vấn `example.com` type `A` tới `10.0.2.3:53` (DNS server của slirp). MAC của DNS
    // server resolve bằng ARP **thật** (10.0.2.3 cùng dải 10.0.2.0/24 với ta) — không dùng MAC
    // gateway cho một host cùng mạng.
    let mut dns_ok = false;
    if let Some(cfg) = &config {
        let our_mac = MacAddr::new(virtio_net::mac());
        match virtio_net::arp_resolve(
            cfg.ip,
            virtio_net::DNS_SERVER_IP,
            virtio_net::NET_STEP_TIMEOUT_US,
        ) {
            Ok(dns_mac) => {
                let _ = writeln!(
                    console,
                    "# net: arp dns={} ip={}",
                    MacText(dns_mac.octets()),
                    IpText(virtio_net::DNS_SERVER_IP)
                );
                let started = virtio_net::now_us();
                // **KT-294**: bộ đếm thử lại của **lần gọi này** — in vào dòng evidence ở cả hai
                // nhánh (thành công và lỗi), nên `attempts=2` là bằng chứng đã retransmit.
                let mut retry = DnsRetry::new();
                match virtio_net::dns_query(
                    cfg.ip,
                    our_mac,
                    virtio_net::DNS_SERVER_IP,
                    dns_mac,
                    DNS_ID_A,
                    DNS_NAME_A,
                    virtio_net::NET_STEP_TIMEOUT_US,
                    &mut retry,
                ) {
                    Ok(answer) => {
                        let record = answer.a;
                        let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                        let _ = writeln!(
                            console,
                            "# net dns: name=example.com id=0x{:x} rcode={} an={} reply_len={} \
                             rtt_ms={} elapsed_ms={elapsed_ms} attempts={} const:timeout_ms={}",
                            answer.id,
                            answer.rcode,
                            record.is_some() as u8,
                            answer.reply_len,
                            answer.rtt_ms,
                            retry.attempts(),
                            virtio_net::NET_STEP_TIMEOUT_US / 1000
                        );
                        match record {
                            Some(record) => {
                                let _ = writeln!(
                                    console,
                                    "# net: dns example.com a={} ttl={}",
                                    record.addr, record.ttl
                                );
                                dns_ok = answer.rcode == 0;
                            }
                            None => {
                                let _ = writeln!(console, "# net: no dns");
                            }
                        }
                    }
                    Err(error) => {
                        let elapsed_ms = virtio_net::now_us().saturating_sub(started) / 1000;
                        let _ = writeln!(
                            console,
                            "# net dns: elapsed_ms={elapsed_ms} attempts={} err={}",
                            retry.attempts(),
                            error.name()
                        );
                        let _ = writeln!(console, "# net: no dns");
                    }
                }
            }
            Err(error) => {
                let _ = writeln!(
                    console,
                    "# net arp dns: ip={} err={}",
                    IpText(virtio_net::DNS_SERVER_IP),
                    error.name()
                );
                let _ = writeln!(console, "# net: no dns");
            }
        }
    } else {
        let _ = writeln!(console, "# net: dns skipped (no dhcp)");
    }
    // **KT-303** — DNS đi qua **resolver của host** (dịch vụ ngoài) nên có thể timeout; nếu
    // `net::run` trả `false` thì ảnh **dừng** ở đây và **che** kết quả `wx`/`gpu`/`input` (đo được:
    // boot không bàn phím dừng ở `not ok 35` nên **không** in `# input: no device` ⇒ ca âm (q) của
    // runner đỏ **vì lý do sai**). Vì vậy TAP 35 đỏ **không** dừng ảnh: đặt cờ `DNS_RED` để
    // `umode::run` **không in marker** (bất biến "TAP đỏ ⇒ không marker" giữ nguyên), các bước sau
    // vẫn chạy và in TAP của chúng.
    if !tap(console, 35, "net: dns", dns_ok) {
        DNS_RED.store(true, core::sync::atomic::Ordering::Relaxed);
    }

    // Bộ đếm driver: `errors=0 timeouts=0 id_mismatch=0` là bằng chứng đường đi sạch (cùng khuôn
    // `# net stats:` của riscv64 KT-246).
    let stats = virtio_net::stats();
    let _ = writeln!(
        console,
        "# net stats: tx_frames={} tx_bytes={} rx_frames={} rx_bytes={} notifies={} errors={} timeouts={} id_mismatch={}",
        stats.tx_frames,
        stats.tx_bytes,
        stats.rx_frames,
        stats.rx_bytes,
        stats.notifies,
        stats.errors,
        stats.timeouts,
        stats.id_mismatch,
    );

    all_ok
}
