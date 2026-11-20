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
//! # TAP (7 test; tên khớp `v2/test/boot/net.sh`)
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
//!
//! Mọi dòng `# net:` in từ **biến đo** (D2-102); giá trị suy ra in kèm công thức/đầu vào
//! (D2-106). Không `unsafe` trong tệp này (MMIO nằm ở `kernel/vifast-plat-riscv64`).

#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::virtio_net;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::net::{MAX_FRAME_BYTES, Q_RX, Q_TX};
use vifast_virtio::pci::{COMMAND_BUS_MASTER, COMMAND_MEMORY};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// IP gateway của QEMU slirp (`-netdev user`): `10.0.2.2` (dữ kiện nền tảng QEMU, không phải số đo).
const GATEWAY_IP: [u8; 4] = [10, 0, 2, 2];
/// Ethertype ARP.
const ETHERTYPE_ARP: u16 = 0x0806;
/// Cỡ một ARP request Ethernet/IPv4: 14 (Ethernet) + 28 (ARP) = 42.
const ARP_FRAME_BYTES: usize = 42;
/// Số echo request của TAP 5 (D2-196 mục (1): "gửi 3 echo request").
const PING_COUNT: u32 = 3;

fn put(ok: bool, number: usize, name: &str, fails: &mut usize, why: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    if ok {
        let _ = writeln!(w, "ok {number} - {name}");
    } else {
        let _ = writeln!(w, "not ok {number} - {name}: {why}");
        *fails += 1;
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

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..7\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# net image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} tables={} used={} \
             const:gateway=10.0.2.2 const:ping_count={PING_COUNT}",
            PLATFORM.hart_id(),
            stvec,
            (stvec & 0b11 == 0) as u8,
            mmu_report.satp,
            mmu_report.satp >> 60,
            mmu_report.tables,
            mmu_report.used
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
            / vifast_plat_common::timer::TIMEBASE_HZ;
        match result {
            Ok(config) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net: dhcp ip={}.{}.{}.{} gw={}.{}.{}.{} mask={}.{}.{}.{} \
                         server={}.{}.{}.{}",
                        config.ip[0],
                        config.ip[1],
                        config.ip[2],
                        config.ip[3],
                        config.gw[0],
                        config.gw[1],
                        config.gw[2],
                        config.gw[3],
                        config.mask[0],
                        config.mask[1],
                        config.mask[2],
                        config.mask[3],
                        config.server[0],
                        config.server[1],
                        config.server[2],
                        config.server[3]
                    );
                    let _ = writeln!(
                        w,
                        "# net dhcp: elapsed_ms={elapsed_ms} xid=0x{:x} \
                         const:timeout_ms={}",
                        0x4b54_3234u32,
                        virtio_net::NET_STEP_TIMEOUT_TICKS * 1000
                            / vifast_plat_common::timer::TIMEBASE_HZ
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
            / vifast_plat_common::timer::TIMEBASE_HZ;
        match result {
            Ok(found) => {
                let mut text = [0u8; 17];
                mac_str(found, &mut text);
                let name = core::str::from_utf8(&text).unwrap_or("?");
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# net: arp gw={name}");
                    let _ = writeln!(
                        w,
                        "# net arp: elapsed_ms={elapsed_ms} gw={}.{}.{}.{} mac_khong_broadcast={}",
                        config.gw[0],
                        config.gw[1],
                        config.gw[2],
                        config.gw[3],
                        (found != [0u8; 6] && found != [0xffu8; 6]) as u8
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
            mac,
            config.gw,
            gateway_mac,
            PING_COUNT,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
        );
        let elapsed_ms = (PLATFORM.now_ticks().wrapping_sub(started)) * 1000
            / vifast_plat_common::timer::TIMEBASE_HZ;
        match result {
            Ok(stats) => {
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# net: ping {}.{}.{}.{} ok n={} rtt={}",
                        config.gw[0],
                        config.gw[1],
                        config.gw[2],
                        config.gw[3],
                        stats.received,
                        stats.rtt_max_ms
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
            virtio_net::arp_build_request(mac, config.ip, config.gw, &mut request).unwrap_or(0);
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
            && sender_ip == GATEWAY_IP
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
        let _ = writeln!(w, "PANIC: KT-246 net xong (7 TAP, {fails} fail)");
    }
    PLATFORM.halt(0)
}

/// Entry hart **thứ cấp** — ảnh net chỉ cần symbol để link (không gọi `smp::init`, không hart thứ
/// cấp nào chạy; nếu có thì park, không panic).
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
