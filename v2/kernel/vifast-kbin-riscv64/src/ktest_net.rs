// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test virtio-net** riscv64 (KT-245 + KT-246, D1) — bin `vifast-kbin-ktest-net`, feature
//! `ktest,net`, nạp qua OpenSBI **kèm** `-netdev user,id=n0 -device virtio-net-pci,netdev=n0`.
//!
//! # Vì sao ảnh riêng
//!
//! (a) Ảnh `ktest` đã ở **92,6 %** trần `.data+.bss` (D2-109) — driver + trang DMA 4 KiB đẩy nó
//! sát trần; (b) ảnh này cần **thiết bị thật** (`-netdev`/`-device`) trong dòng QEMU, tách ra thì
//! 21 boot matrix cũ **không đổi**; (c) boot kết thúc bằng **một** dòng `PANIC:` chủ đích (cùng
//! quy ước ảnh guard/user/proc/vblk/con/exec).
//!
//! # TAP (11 test; tên khớp `v2/test/boot/net.sh`)
//!
//! 1. `net device` — quét ECAM tìm **vendor/device đọc từ thiết bị** (`0x1af4:0x1000`), đo cỡ BAR
//!    bằng mask, **gán** BAR (firmware không enumerate), đọc lại BAR + `command`, đọc bốn
//!    capability; ECAM/BAR được map identity (walk phần mềm).
//! 2. `net mac` — đàm phán `VIRTIO_F_VERSION_1` + `VIRTIO_NET_F_MAC` (**không** `MRG_RXBUF`,
//!    **không** `STATUS`), hai queue `rx`/`tx` bật, **MAC đọc từ `virtio_net_config`** (tiền tố
//!    QEMU `52:54:00:`), địa chỉ DMA identity (`VA == PA`).
//! 3. `net dhcp` — **DHCP client thật**: DISCOVER (broadcast) → OFFER → REQUEST → ACK; in
//!    `net: dhcp ip=… gw=… mask=… server=…`. Không có server trong 3 s ⇒ `net: no link` + TAP đỏ
//!    (fail-closed, **không** treo).
//! 4. `net arp` — hỏi MAC của gateway bằng ARP request, nhận reply ⇒ `net: arp gw=<mac>`.
//! 5. `net ping` — **3** ICMP echo request tới gateway, nhận đủ 3 reply ⇒
//!    `net: ping <ip> ok n=3 rtt=<ms>`.
//! 6. `net tx` — gửi **một** frame ARP request thô (42 byte, broadcast) qua queue `tx`; thiết bị
//!    hoàn tất `used` (`used->id` khớp head, `used->len` không vượt số byte đã đưa).
//! 7. `net rx` — nhận **frame trả lời** của slirp trên queue `rx`: ARP reply từ gateway
//!    `10.0.2.2`, đúng `oper=2`, `sender_mac` khác 0, `target_mac` = MAC của ta.
//! 8. `net udp echo` — **KT-260**: gửi UDP payload `vifast-udp-echo` và nhận lại **đúng** payload
//!    đó. libslirp **không** có echo service UDP ở `10.0.2.2:7` (xem chú thích trong
//!    `virtio_net.rs`), nên theo fallback của KT-260, "gương" là **reply DNS**: QNAME =
//!    `vifast-udp-echo`, server chép nguyên question section (RFC 1035 §4.1.2) ⇒
//!    `net: udp echo ok len=15 rtt=<ms>`.
//! 9. `net dns a` — **KT-260**: truy vấn `example.com` type `A` tới `10.0.2.3:53`, parse bản ghi
//!    `A` đầu tiên ⇒ `net: dns example.com a=<ip> ttl=<n>`.
//! 10. `net tcp connect` — **KT-268**: mở TCP thật tới **server trên host** (runner dựng
//!     `python3` lắng nghe `127.0.0.1:<port>`; khách nối `10.0.2.2:<port>` — slirp ánh xạ
//!     `10.0.2.2` → loopback của host): SYN → SYN+ACK → ACK ⇒
//!     `net: tcp connect 10.0.2.2:<port> ok rtt=<ms>`.
//! 11. `net tcp data` — **KT-268**: nhận **đúng** chuỗi server gửi (`net: tcp recv=<n> first=<…>`),
//!     gửi lại một payload (server ghi `recv=` trong log của nó), rồi đóng bằng FIN.
//!
//! Mọi dòng `# net:` in từ **biến đo** (D2-102); giá trị suy ra in kèm công thức/đầu vào
//! (D2-106). Không `unsafe` trong tệp này (MMIO nằm ở `kernel/vifast-plat-riscv64`).

use core::fmt::Write;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::dns::{DnsRetry, TYPE_A};
use vifast_net::tcp::State;
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::tcp;
use vifast_plat_riscv64::virtio_net;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::net::{MAX_FRAME_BYTES, Q_RX, Q_TX};
use vifast_virtio::pci::{COMMAND_BUS_MASTER, COMMAND_MEMORY};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// IP gateway của QEMU slirp (`-netdev user`): `10.0.2.2` (dữ kiện nền tảng QEMU, không phải số đo).
const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
/// Ethertype ARP.
const ETHERTYPE_ARP: u16 = 0x0806;
/// Cỡ một ARP request Ethernet/IPv4: 14 (Ethernet) + 28 (ARP) = 42.
const ARP_FRAME_BYTES: usize = 42;
/// Số echo request của TAP 5 (D2-196 mục (1): "gửi 3 echo request").
const PING_COUNT: u32 = 3;
/// `id` của truy vấn DNS "gương" UDP echo (TAP 8) — hằng số vì chưa có RNG.
const DNS_ID_ECHO: u16 = 0x4b54;
/// `id` của truy vấn DNS `example.com` A (TAP 9).
const DNS_ID_A: u16 = 0x4b55;
/// Tên hỏi cho TAP 9.
const DNS_NAME_A: &[u8] = b"example.com";
/// Payload khách gửi lại cho server ở TAP 11 (server ghi `recv=` trong log của nó).
const TCP_SEND_PAYLOAD: &[u8] = b"vifast-tcp-send";
/// Cỡ tối đa của `first=` in ra ở TAP 11 (đủ thấy chuỗi server gửi, không tràn dòng log).
const TCP_FIRST_BYTES: usize = 32;

fn put(ok: bool, number: usize, name: &str, fails: &mut usize, why: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    if ok {
        let _ = writeln!(w, "ok {number} - {name}");
    } else {
        let _ = writeln!(w, "not ok {number} - {name}: {why}");
        *fails += 1;
    }
}

/// Tên trạng thái TCP để in evidence (D2-102: tên ổn định, không in số trần).
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

/// `true` ⇔ VA `addr` được map **identity**: PA = VA (cùng offset trong leaf). Tính cả leaf 4 KiB
/// (level 0) lẫn leaf lớn 2 MiB (level 1) — ECAM/BAR dùng leaf 2 MiB, vùng `.bss` dùng 4 KiB.
fn identity(addr: u64) -> bool {
    let walk = mmu::walk_sw(addr);
    if !walk.valid || !walk.leaf || addr < walk.phys {
        return false;
    }
    let leaf_bytes = 4096u64 << (9 * walk.level as u32);
    addr - walk.phys < leaf_bytes
}

/// In MAC dạng `52:54:00:12:34:56` vào `out` (17 byte, không NUL).
fn mac_str(mac: [u8; 6], out: &mut [u8; 17]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < 6 {
        out[index * 3] = HEX[(mac[index] >> 4) as usize];
        out[index * 3 + 1] = HEX[(mac[index] & 0xf) as usize];
        if index < 5 {
            out[index * 3 + 2] = b':';
        }
        index += 1;
    }
}

pub fn run() -> ! {
    PLATFORM.put_str("TAP version 13\n1..11\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# net image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} tables={} used={} \
             const:gateway=10.0.2.2 const:ping_count={PING_COUNT} const:dns=10.0.2.3:53 \
             const:tcp=10.0.2.2:{}",
            PLATFORM.hart_id(),
            stvec,
            (stvec & 0b11 == 0) as u8,
            mmu_report.satp,
            mmu_report.satp >> 60,
            mmu_report.tables,
            mmu_report.used,
            tcp::TCP_SERVER_PORT
        );
    }
    let mut fails = 0usize;

    // ------------------------------------------------ 1) PCI: tìm + gán BAR + capability
    let pci_report = match pci::init_for(vifast_virtio::pci::VirtioKind::Net) {
        Ok(report) => {
            let ecam_ok = identity(report.ecam_addr);
            let bar_ok = identity(report.bar_assigned);
            let ok = report.vendor == 0x1af4
                && (report.device == 0x1000 || report.device == 0x1041)
                && report.bar_readback == report.bar_assigned
                && report.command & (COMMAND_MEMORY | COMMAND_BUS_MASTER)
                    == COMMAND_MEMORY | COMMAND_BUS_MASTER
                && report.bar_size >= 0x1000
                && ecam_ok
                && bar_ok
                && report.overlap_checked
                && report.cap_ranges_ok;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# net pci: functions={} bus={} dev={} func={} vendor=0x{:x} device=0x{:x} \
                     bar={} kind={} size={} assigned=0x{:x} readback=0x{:x} command=0x{:x}",
                    report.functions_seen,
                    report.bus,
                    report.dev,
                    report.func,
                    report.vendor,
                    report.device,
                    report.bar_index,
                    pci::bar_kind_name(report.bar_kind),
                    report.bar_size,
                    report.bar_assigned,
                    report.bar_readback,
                    report.command
                );
                let _ = writeln!(
                    w,
                    "# net cap: common=0x{:x} notify=0x{:x} device=0x{:x} isr=0x{:x} mul={} \
                     cap_bar={} ecam=0x{:x} ecam_identity={} bar_identity={} cap_ranges_ok={} \
                     overlap_checked={} other_bars={} reserved_ranges={}",
                    report.caps.common.offset,
                    report.caps.notify.offset,
                    report.caps.device.offset,
                    report.caps.isr.offset,
                    report.caps.notify_off_multiplier,
                    report.caps.bar,
                    report.ecam_addr,
                    ecam_ok as u8,
                    bar_ok as u8,
                    report.cap_ranges_ok as u8,
                    report.overlap_checked as u8,
                    report.other_bars,
                    report.reserved_ranges
                );
            }
            put(
                ok,
                1,
                "net device",
                &mut fails,
                "vendor/device/BAR/capability không đúng",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            report
        }
        Err(fail) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# net pci: err={} stage={}",
                fail.error.name(),
                fail.stage
            );
            // Dòng **cố định** cho runner + ca âm (D2-196 mục (3)): không có `-netdev`/`-device`
            // thì ảnh phải in đúng dòng này và TAP **đỏ**, không PASS giả.
            let _ = writeln!(w, "# net: no device");
            put(
                false,
                1,
                "net device",
                &mut fails,
                "không tìm/khởi tạo được PCI",
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };

    // ------------------------------------------------ 2) feature + hai queue + MAC thật
    let net = match virtio_net::init(&pci_report) {
        Ok(report) => {
            let dma_ok = identity(report.desc_rx)
                && identity(report.avail_rx)
                && identity(report.used_rx)
                && identity(report.desc_tx)
                && identity(report.avail_tx)
                && identity(report.used_tx)
                && identity(report.rx_buf_pa)
                && identity(report.tx_buf_pa);
            let version_ok = report.device_features & (1u64 << 32) != 0
                && report.driver_features & (1u64 << 32) != 0;
            let mac = report.config.mac;
            // Tiền tố `52:54:00` là OUI mà QEMU gán cho card slirp — MAC **đọc từ thiết bị**, hằng
            // này chỉ để đối chiếu nguồn gốc (không dùng làm số đo).
            let qemu_oui = mac[0] == 0x52 && mac[1] == 0x54 && mac[2] == 0x00;
            let status = virtio_net::device_status();
            let ok = version_ok
                && report.mac_ok
                && report.driver_features
                    == vifast_virtio::net::negotiated_features(report.device_features)
                && status & 0x0f == 0x0f
                && report.queue_size_rx >= 2
                && report.queue_size_rx.is_power_of_two()
                && report.queue_size_tx >= 2
                && report.queue_size_tx.is_power_of_two()
                && report.num_queues >= 2
                && report.config.mac_hop_le()
                && qemu_oui
                && dma_ok;
            let mut text = [0u8; 17];
            mac_str(mac, &mut text);
            let name = core::str::from_utf8(&text).unwrap_or("?");
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# net dev: mac={name} status=0x{:x} mtu_raw={} max_virtqueue_pairs={} \
                     num_queues={} queue_max_rx={} queue_max_tx={} queue_size_rx={} \
                     queue_size_tx={} device_features=0x{:x} driver_features=0x{:x} \
                     mac_feature={} status_feature={} mrg_rxbuf_feature={} device_status=0x{:x} \
                     config_generation={}",
                    report.config.status,
                    report.config.mtu,
                    report.config.max_virtqueue_pairs,
                    report.num_queues,
                    report.queue_max_rx,
                    report.queue_max_tx,
                    report.queue_size_rx,
                    report.queue_size_tx,
                    report.device_features,
                    report.driver_features,
                    report.mac_ok as u8,
                    vifast_virtio::net::status_negotiated(report.driver_features) as u8,
                    vifast_virtio::net::mrg_rxbuf_negotiated(report.driver_features) as u8,
                    status,
                    report.config_generation
                );
                let _ = writeln!(
                    w,
                    "# net dma: desc_rx=0x{:x} avail_rx=0x{:x} used_rx=0x{:x} desc_tx=0x{:x} \
                     avail_tx=0x{:x} used_tx=0x{:x} rx_buf=0x{:x} tx_buf=0x{:x} identity={} \
                     version1={} notify_off_rx={} notify_off_tx={} notify_addr_rx=0x{:x} \
                     notify_addr_tx=0x{:x}",
                    report.desc_rx,
                    report.avail_rx,
                    report.used_rx,
                    report.desc_tx,
                    report.avail_tx,
                    report.used_tx,
                    report.rx_buf_pa,
                    report.tx_buf_pa,
                    dma_ok as u8,
                    version_ok as u8,
                    report.notify_off_rx,
                    report.notify_off_tx,
                    report.notify_addr_rx,
                    report.notify_addr_tx
                );
            }
            put(
                ok,
                2,
                "net mac",
                &mut fails,
                "feature/queue/MAC/DMA không đúng",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            report
        }
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# net dev: init_fail={stage}");
            put(false, 2, "net mac", &mut fails, "init thiết bị thất bại");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let mac = net.config.mac;

    // ------------------------------------------------ 3) DHCP: DISCOVER → OFFER → REQUEST → ACK
    let config = {
        let started = PLATFORM.now_ticks();
        let result = virtio_net::dhcp_acquire(virtio_net::NET_STEP_TIMEOUT_TICKS);
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(config) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net: dhcp ip={} gw={} mask={} server={}",
                        config.ip, config.gw, config.mask, config.server
                    );
                    let _ = writeln!(
                        w,
                        "# net dhcp: elapsed_ms={elapsed_ms} xid=0x{:x} \
                         const:timeout_ms={}",
                        0x4b54_3234u32,
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ
                    );
                }
                put(true, 3, "net dhcp", &mut fails, "");
                config
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net dhcp: elapsed_ms={elapsed_ms} err={}",
                        error.name()
                    );
                    // Dòng **cố định** cho runner + ca "không mạng" (D2-196 mục (3)).
                    let _ = writeln!(w, "# net: no link");
                }
                put(
                    false,
                    3,
                    "net dhcp",
                    &mut fails,
                    "không nhận được DHCP OFFER/ACK trong 3 s",
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        }
    };

    // ------------------------------------------------ 4) ARP: MAC của gateway
    let gateway_mac = {
        let started = PLATFORM.now_ticks();
        let result =
            virtio_net::arp_resolve(config.ip, config.gw, virtio_net::NET_STEP_TIMEOUT_TICKS);
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(found) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# net: arp gw={found}");
                    let _ = writeln!(
                        w,
                        "# net arp: elapsed_ms={elapsed_ms} gw={} mac_khong_broadcast={}",
                        config.gw,
                        (!found.is_zero() && !found.is_broadcast()) as u8
                    );
                }
                put(true, 4, "net arp", &mut fails, "");
                found
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# net arp: elapsed_ms={elapsed_ms} err={}", error.name());
                    let _ = writeln!(w, "# net: no link");
                }
                put(
                    false,
                    4,
                    "net arp",
                    &mut fails,
                    "không nhận được ARP reply của gateway",
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        }
    };

    // ------------------------------------------------ 5) ICMP echo ×3 tới gateway
    {
        let started = PLATFORM.now_ticks();
        let result = virtio_net::ping(
            config.ip,
            MacAddr::new(mac),
            config.gw,
            gateway_mac,
            PING_COUNT,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(stats) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net: ping {} ok n={} rtt={}",
                        config.gw, stats.received, stats.rtt_max_ms
                    );
                    let _ = writeln!(
                        w,
                        "# net ping: sent={} received={} rtt_min_ms={} rtt_max_ms={} \
                         elapsed_ms={elapsed_ms} const:count={PING_COUNT}",
                        stats.sent, stats.received, stats.rtt_min_ms, stats.rtt_max_ms
                    );
                }
                put(true, 5, "net ping", &mut fails, "");
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net ping: elapsed_ms={elapsed_ms} err={}",
                        error.name()
                    );
                    let _ = writeln!(w, "# net: no link");
                }
                put(
                    false,
                    5,
                    "net ping",
                    &mut fails,
                    "không nhận đủ 3 ICMP echo reply",
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        }
    }

    // ------------------------------------------------ 6) TX thô: gửi ARP request thật
    let mut request = [0u8; ARP_FRAME_BYTES];
    {
        let len =
            virtio_net::arp_build_request(MacAddr::new(mac), config.ip, config.gw, &mut request)
                .unwrap_or(0);
        let before = virtio_net::stats();
        let result = virtio_net::send_frame(&request[..len]);
        let after = virtio_net::stats();
        let ok = len == ARP_FRAME_BYTES
            && result.is_ok()
            && after.tx_frames == before.tx_frames + 1
            && after.tx_bytes == before.tx_bytes + ARP_FRAME_BYTES as u64
            && after.errors == before.errors
            && after.timeouts == before.timeouts
            && after.id_mismatch == 0;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# net tx: frame={} (=14 ethernet + 28 arp) result={} tx_frames={} tx_bytes={} \
                 notifies={} errors={} timeouts={} id_mismatch={} queue={}",
                ARP_FRAME_BYTES,
                match result {
                    Ok(()) => "ok",
                    Err(error) => error.name(),
                },
                after.tx_frames,
                after.tx_bytes,
                after.notifies,
                after.errors,
                after.timeouts,
                after.id_mismatch,
                Q_TX
            );
        }
        put(
            ok,
            6,
            "net tx",
            &mut fails,
            "gửi frame ARP request thất bại",
        );
    }

    // ------------------------------------------------ 7) RX thô: nhận ARP reply của slirp
    {
        let mut frame = [0u8; MAX_FRAME_BYTES];
        let before = virtio_net::stats();
        let got = virtio_net::recv_frame_wait(&mut frame, virtio_net::NET_STEP_TIMEOUT_TICKS);
        let after = virtio_net::stats();
        let mut sender_mac = [0u8; 6];
        let mut sender_ip = [0u8; 4];
        let mut target_mac = [0u8; 6];
        let mut oper = 0u16;
        let mut ethertype = 0u16;
        if let Some(len) = got {
            if len >= ARP_FRAME_BYTES {
                ethertype = u16::from_be_bytes([frame[12], frame[13]]);
                oper = u16::from_be_bytes([frame[20], frame[21]]);
                sender_mac.copy_from_slice(&frame[22..28]);
                sender_ip.copy_from_slice(&frame[28..32]);
                target_mac.copy_from_slice(&frame[32..38]);
            }
        }
        let ok = got.is_some()
            && got.unwrap_or(0) >= ARP_FRAME_BYTES
            && ethertype == ETHERTYPE_ARP
            && oper == 2
            && sender_ip == GATEWAY_IP.octets()
            && sender_mac != [0u8; 6]
            && target_mac == mac
            && after.rx_frames == before.rx_frames + 1
            && after.errors == before.errors
            && after.id_mismatch == 0;
        let mut text = [0u8; 17];
        mac_str(sender_mac, &mut text);
        let sender = core::str::from_utf8(&text).unwrap_or("?");
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# net rx: frame={} oper={} ethertype=0x{:x} sender_ip={}.{}.{}.{} \
                 sender_mac={sender} target_mac_khop={} rx_frames={} rx_bytes={} errors={} \
                 timeouts={} id_mismatch={} queue={}",
                got.unwrap_or(0),
                oper,
                ethertype,
                sender_ip[0],
                sender_ip[1],
                sender_ip[2],
                sender_ip[3],
                (target_mac == mac) as u8,
                after.rx_frames,
                after.rx_bytes,
                after.errors,
                after.timeouts,
                after.id_mismatch,
                Q_RX
            );
        }
        put(
            ok,
            7,
            "net rx",
            &mut fails,
            "không nhận được ARP reply của slirp",
        );
    }

    // ------------------------------------------------ 8) UDP echo (KT-260)
    // libslirp **không** có echo service UDP ở `10.0.2.2:7` (chỉ BOOTP/TFTP rồi chuyển cho socket
    // host; `10.0.2.2` ánh xạ về loopback host). Theo fallback của KT-260, "gương" UDP là **reply
    // DNS**: QNAME = payload, server chép nguyên question section (RFC 1035 §4.1.2) ⇒ nhận lại
    // đúng chuỗi đã gửi là bằng chứng UDP hai chiều.
    //
    // Chẩn đoán (không phải TAP): gọi `udp_echo` tới `10.0.2.2:7` để đo **fail-closed 3 s**; nếu
    // môi trường có echo thật, dòng này đổi `result=ok` và runner sẽ báo (không im lặng đổi đường).
    {
        let started = PLATFORM.now_ticks();
        let result = virtio_net::udp_echo(
            config.ip,
            MacAddr::new(mac),
            GATEWAY_IP,
            gateway_mac,
            virtio_net::UDP_CLIENT_PORT,
            virtio_net::UDP_ECHO_PORT,
            virtio_net::UDP_ECHO_PAYLOAD,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        let mut w = PlatformWriter(&PLATFORM);
        match result {
            Ok(echo) => {
                let _ = writeln!(
                    w,
                    "# net udp echo: target={}:{} result=ok len={} rtt_ms={} elapsed_ms={elapsed_ms}",
                    GATEWAY_IP,
                    virtio_net::UDP_ECHO_PORT,
                    echo.len,
                    echo.rtt_ms
                );
            }
            Err(error) => {
                let _ = writeln!(
                    w,
                    "# net udp echo: target={}:{} result={} elapsed_ms={elapsed_ms} \
                     const:timeout_ms={}",
                    GATEWAY_IP,
                    virtio_net::UDP_ECHO_PORT,
                    error.name(),
                    virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                        / vifast_plat_riscv64::timer::TIMEBASE_HZ
                );
            }
        }
    }

    let dns_mac = {
        let started = PLATFORM.now_ticks();
        let result = virtio_net::arp_resolve(
            config.ip,
            virtio_net::DNS_SERVER_IP,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(found) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net: arp dns={found} ip={} elapsed_ms={elapsed_ms}",
                        virtio_net::DNS_SERVER_IP
                    );
                }
                Some(found)
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net arp dns: ip={} elapsed_ms={elapsed_ms} err={}",
                        virtio_net::DNS_SERVER_IP,
                        error.name()
                    );
                    let _ = writeln!(w, "# net: no dns");
                }
                None
            }
        }
    };
    if let Some(dns_mac) = dns_mac {
        // ---- TAP 8: UDP echo qua "gương" DNS (payload = QNAME)
        let started = PLATFORM.now_ticks();
        // **KT-294**: bộ đếm thử lại của **lần gọi này** (tạo mới mỗi truy vấn).
        let mut retry = DnsRetry::new();
        let result = virtio_net::dns_query(
            config.ip,
            MacAddr::new(mac),
            virtio_net::DNS_SERVER_IP,
            dns_mac,
            DNS_ID_ECHO,
            virtio_net::UDP_ECHO_PAYLOAD,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
            &mut retry,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(answer) => {
                let echoed = answer
                    .question
                    .name
                    .eq_ignore_ascii_case(virtio_net::UDP_ECHO_PAYLOAD)
                    && answer.question.qtype == TYPE_A;
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net udp echo: target={}:{} via=dns-question-echo payload_len={} \
                         reply_len={} rcode={} echoed={} rtt_ms={} elapsed_ms={elapsed_ms} \
                         const:timeout_ms={}",
                        virtio_net::DNS_SERVER_IP,
                        virtio_net::DNS_PORT,
                        virtio_net::UDP_ECHO_PAYLOAD.len(),
                        answer.reply_len,
                        answer.rcode,
                        echoed as u8,
                        answer.rtt_ms,
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ
                    );
                    if echoed {
                        let _ = writeln!(
                            w,
                            "# net: udp echo ok len={} rtt={}",
                            virtio_net::UDP_ECHO_PAYLOAD.len(),
                            answer.rtt_ms
                        );
                    } else {
                        let _ = writeln!(w, "# net: no udp");
                    }
                }
                put(
                    echoed,
                    8,
                    "net udp echo",
                    &mut fails,
                    "reply DNS không chép lại đúng payload",
                );
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net udp echo: elapsed_ms={elapsed_ms} err={}",
                        error.name()
                    );
                    let _ = writeln!(w, "# net: no udp");
                }
                put(
                    false,
                    8,
                    "net udp echo",
                    &mut fails,
                    "không nhận reply UDP trong 3 s",
                );
            }
        }

        // ---- TAP 9: DNS query example.com A
        let started = PLATFORM.now_ticks();
        // **KT-294**: bộ đếm thử lại — in vào dòng evidence ở cả hai nhánh.
        let mut retry = DnsRetry::new();
        let result = virtio_net::dns_query(
            config.ip,
            MacAddr::new(mac),
            virtio_net::DNS_SERVER_IP,
            dns_mac,
            DNS_ID_A,
            DNS_NAME_A,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
            &mut retry,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(answer) => {
                let record = answer.a;
                let ok = record.is_some() && answer.rcode == 0;
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net dns: name=example.com id=0x{:x} rcode={} an={} reply_len={} \
                         rtt_ms={} elapsed_ms={elapsed_ms} attempts={} const:timeout_ms={}",
                        answer.id,
                        answer.rcode,
                        record.is_some() as u8,
                        answer.reply_len,
                        answer.rtt_ms,
                        retry.attempts(),
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ
                    );
                    match record {
                        Some(record) => {
                            let _ = writeln!(
                                w,
                                "# net: dns example.com a={} ttl={}",
                                record.addr, record.ttl
                            );
                        }
                        None => {
                            let _ = writeln!(w, "# net: no dns");
                        }
                    }
                }
                put(
                    ok,
                    9,
                    "net dns a",
                    &mut fails,
                    "reply không có bản ghi A hoặc rcode khác 0",
                );
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net dns: elapsed_ms={elapsed_ms} attempts={} err={}",
                        retry.attempts(),
                        error.name()
                    );
                    let _ = writeln!(w, "# net: no dns");
                }
                put(
                    false,
                    9,
                    "net dns a",
                    &mut fails,
                    "không nhận reply DNS trong 2 lần thử × 3 s",
                );
            }
        }
    } else {
        put(
            false,
            8,
            "net udp echo",
            &mut fails,
            "không resolve được MAC của DNS server",
        );
        put(
            false,
            9,
            "net dns a",
            &mut fails,
            "không resolve được MAC của DNS server",
        );
    }

    // ---- Chẩn đoán (không phải TAP): DNS timeout trên địa chỉ **không có** server.
    // `192.0.2.1` là TEST-NET-1 (RFC 5737) — không có dịch vụ DNS; frame đi qua MAC gateway vì
    // slirp định tuyến theo IP đích, không theo MAC đích. Không reply ⇒ **KT-294**: hai lần thử,
    // mỗi lần 3 s ⇒ `Timeout` sau đúng 6 s với `attempts=2` (bằng chứng retransmit **thật**).
    {
        let started = PLATFORM.now_ticks();
        // **KT-294**: đích **không có** server ⇒ **cả hai** lần thử timeout; `attempts=2` +
        // `elapsed_ms=6000` là bằng chứng retransmit **thật** (không phải suy luận từ mã).
        let mut retry = DnsRetry::new();
        let result = virtio_net::dns_query(
            config.ip,
            MacAddr::new(mac),
            Ipv4Addr::new(192, 0, 2, 1),
            gateway_mac,
            DNS_ID_A,
            DNS_NAME_A,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
            &mut retry,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        let mut w = PlatformWriter(&PLATFORM);
        match result {
            Ok(answer) => {
                let _ = writeln!(
                    w,
                    "# net dns: target=192.0.2.1:53 result=ok rcode={} reply_len={} \
                     elapsed_ms={elapsed_ms} attempts={}",
                    answer.rcode,
                    answer.reply_len,
                    retry.attempts()
                );
            }
            Err(error) => {
                let _ = writeln!(
                    w,
                    "# net dns: target=192.0.2.1:53 result={} elapsed_ms={elapsed_ms} \
                     attempts={} const:timeout_ms={}",
                    error.name(),
                    retry.attempts(),
                    virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                        / vifast_plat_riscv64::timer::TIMEBASE_HZ
                );
            }
        }
    }

    // ------------------------------------------------ 10) TCP connect tới server trên host
    // **Server thật**: runner dựng `python3` lắng nghe `127.0.0.1:<TCP_SERVER_PORT>`; khách nối
    // `10.0.2.2:<port>` — slirp ánh xạ `10.0.2.2` → loopback của host (`sotranslate_out4`).
    // Không có server ⇒ `tcp_connect` timeout 3 s (fail-closed, không treo).
    let mut tcp = {
        let started = PLATFORM.now_ticks();
        let result = tcp::tcp_connect(
            config.ip,
            MacAddr::new(mac),
            GATEWAY_IP,
            gateway_mac,
            tcp::TCP_SERVER_PORT,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
            Ok(conn) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net: tcp connect {}:{} ok rtt={} state={}",
                        GATEWAY_IP,
                        tcp::TCP_SERVER_PORT,
                        elapsed_ms,
                        state_name(conn.state())
                    );
                    let _ = writeln!(
                        w,
                        "# net tcp connect: iss=0x{:x} snd_una=0x{:x} rcv_nxt=0x{:x} \
                         retransmits={} elapsed_ms={elapsed_ms} const:timeout_ms={} \
                         const:port={}",
                        conn.iss(),
                        conn.snd_una(),
                        conn.rcv_nxt(),
                        conn.retries(),
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ,
                        tcp::TCP_SERVER_PORT
                    );
                }
                put(true, 10, "net tcp connect", &mut fails, "");
                conn
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net tcp connect: target={}:{} err={} elapsed_ms={elapsed_ms} \
                         const:timeout_ms={}",
                        GATEWAY_IP,
                        tcp::TCP_SERVER_PORT,
                        error.name(),
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ
                    );
                    let _ = writeln!(w, "# net: no tcp");
                }
                put(
                    false,
                    10,
                    "net tcp connect",
                    &mut fails,
                    "không mở được TCP trong 3 s",
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        }
    };

    // ------------------------------------------------ 11) TCP data: nhận chuỗi server + gửi lại + đóng
    {
        let mut buf = [0u8; 64];
        let started = PLATFORM.now_ticks();
        let result = tcp::tcp_recv(&mut tcp, &mut buf, virtio_net::NET_STEP_TIMEOUT_TICKS);
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_riscv64::timer::TIMEBASE_HZ;
        match result {
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
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# net: tcp recv={n} first={first}");
                }
                // Chiều khách → host: server ghi `recv=` trong log của nó (bằng chứng hai chiều).
                let sent = tcp::tcp_send(
                    &mut tcp,
                    TCP_SEND_PAYLOAD,
                    virtio_net::NET_STEP_TIMEOUT_TICKS,
                );
                let closed = tcp::tcp_close(&mut tcp, virtio_net::NET_STEP_TIMEOUT_TICKS);
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net tcp data: recv={n} sent={} send={} close={} state={} \
                         retransmits={} elapsed_ms={elapsed_ms} const:timeout_ms={}",
                        TCP_SEND_PAYLOAD.len(),
                        match sent {
                            Ok(_) => "ok",
                            Err(error) => error.name(),
                        },
                        match closed {
                            Ok(()) => "ok",
                            Err(error) => error.name(),
                        },
                        state_name(tcp.state()),
                        tcp.retries(),
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ
                    );
                }
                let ok = sent.is_ok() && closed.is_ok();
                put(ok, 11, "net tcp data", &mut fails, "gửi/đóng TCP thất bại");
            }
            Ok(_) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net tcp data: recv=0 elapsed_ms={elapsed_ms} state={}",
                        state_name(tcp.state())
                    );
                    let _ = writeln!(w, "# net: no tcp");
                }
                put(
                    false,
                    11,
                    "net tcp data",
                    &mut fails,
                    "peer đóng trước khi gửi dữ liệu",
                );
            }
            Err(error) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net tcp data: recv_err={} elapsed_ms={elapsed_ms} state={} \
                         const:timeout_ms={}",
                        error.name(),
                        state_name(tcp.state()),
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_riscv64::timer::TIMEBASE_HZ
                    );
                    let _ = writeln!(w, "# net: no tcp");
                }
                put(
                    false,
                    11,
                    "net tcp data",
                    &mut fails,
                    "không nhận được dữ liệu TCP trong 3 s",
                );
            }
        }
    }

    // ------------------------------------------------ kết thúc chủ đích
    let stats = virtio_net::stats();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# net stats: tx_frames={} tx_bytes={} rx_frames={} rx_bytes={} notifies={} \
             errors={} timeouts={} id_mismatch={} device_status=0x{:x}",
            stats.tx_frames,
            stats.tx_bytes,
            stats.rx_frames,
            stats.rx_bytes,
            stats.notifies,
            stats.errors,
            stats.timeouts,
            stats.id_mismatch,
            virtio_net::device_status()
        );
        // Dấu kết thúc **chủ đích** (không có fault): runner đòi đúng **một** dòng PANIC cho mỗi
        // boot; ảnh này không fault nên tự in dấu kết thúc rồi `halt(0)`. Nội dung lấy từ `fails`.
        let _ = writeln!(w, "PANIC: KT-268 net xong (11 TAP, {fails} fail)");
    }
    PLATFORM.halt(0)
}
