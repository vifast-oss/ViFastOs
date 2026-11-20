// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test virtio-net** riscv64 (KT-245, D1) — bin `vifast-kbin-ktest-net`, feature `ktest,net`,
//! nạp qua OpenSBI **kèm** `-netdev user,id=n0 -device virtio-net-pci,netdev=n0`.
//!
//! # Vì sao ảnh riêng
//!
//! (a) Ảnh `ktest` đã ở **92,6 %** trần `.data+.bss` (D2-109) — driver + trang DMA 4 KiB đẩy nó
//! sát trần; (b) ảnh này cần **thiết bị thật** (`-netdev`/`-device`) trong dòng QEMU, tách ra thì
//! 21 boot matrix cũ **không đổi**; (c) boot kết thúc bằng **một** dòng `PANIC:` chủ đích (cùng
//! quy ước ảnh guard/user/proc/vblk/con/exec).
//!
//! # TAP (4 test; tên khớp `v2/test/boot/net.sh`)
//!
//! 1. `net device` — quét ECAM tìm **vendor/device đọc từ thiết bị** (`0x1af4:0x1000`), đo cỡ BAR
//!    bằng mask, **gán** BAR (firmware không enumerate), đọc lại BAR + `command`, đọc bốn
//!    capability; ECAM/BAR được map identity (walk phần mềm).
//! 2. `net mac` — đàm phán `VIRTIO_F_VERSION_1` + `VIRTIO_NET_F_MAC` (**không** `MRG_RXBUF`,
//!    **không** `STATUS`), hai queue `rx`/`tx` bật, **MAC đọc từ `virtio_net_config`** (tiền tố
//!    QEMU `52:54:00:`), địa chỉ DMA identity (`VA == PA`).
//! 3. `net tx` — gửi **một** frame ARP request thật (42 byte, broadcast) qua queue `tx`; thiết bị
//!    hoàn tất `used` (`used->id` khớp head, `used->len` không vượt số byte đã đưa).
//! 4. `net rx` — nhận **frame trả lời** của slirp trên queue `rx`: ARP reply từ gateway
//!    `10.0.2.2`, đúng `oper=2`, `sender_mac` khác 0, `target_mac` = MAC của ta. Đây là **một**
//!    vòng tx→rx thật qua thiết bị, không phải loopback phần mềm.
//!
//! Mọi dòng `# net:` in từ **biến đo** (D2-102); giá trị suy ra in kèm công thức/đầu vào
//! (D2-106). Không `unsafe` trong tệp này (MMIO nằm ở `kernel/vifast-plat-riscv64`).

#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::timer::TIMEBASE_HZ;
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
/// IP mặc định của guest trong slirp: `10.0.2.15` — dùng làm `spa` của ARP request.
const GUEST_IP: [u8; 4] = [10, 0, 2, 15];
/// Ethertype ARP.
const ETHERTYPE_ARP: u16 = 0x0806;
/// Cỡ một ARP request Ethernet/IPv4: 14 (Ethernet) + 28 (ARP) = 42.
const ARP_FRAME_BYTES: usize = 42;
/// Timeout chờ frame trả lời (tick timebase) — 1 s ảo.
const RX_TIMEOUT_TICKS: u64 = TIMEBASE_HZ;

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

/// In MAC dạng `52:54:00:12:34:56` (không dùng `{:02x}` lặp trong `writeln!` để giữ `no_std` gọn).
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

/// Dựng **ARP request** Ethernet/IPv4 hỏi MAC của `target_ip` (42 byte, broadcast).
fn build_arp_request(mac: [u8; 6], target_ip: [u8; 4], out: &mut [u8; ARP_FRAME_BYTES]) {
    out[0..6].copy_from_slice(&[0xff; 6]); // đích: broadcast
    out[6..12].copy_from_slice(&mac); // nguồn: MAC của ta
    out[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());
    out[14..16].copy_from_slice(&1u16.to_be_bytes()); // htype = Ethernet
    out[16..18].copy_from_slice(&0x0800u16.to_be_bytes()); // ptype = IPv4
    out[18] = 6; // hlen
    out[19] = 4; // plen
    out[20..22].copy_from_slice(&1u16.to_be_bytes()); // oper = request
    out[22..28].copy_from_slice(&mac); // sha
    out[28..32].copy_from_slice(&GUEST_IP); // spa
    out[32..38].copy_from_slice(&[0u8; 6]); // tha (chưa biết)
    out[38..42].copy_from_slice(&target_ip); // tpa
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..4\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# net image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} tables={} used={} \
             const:gateway=10.0.2.2 const:guest=10.0.2.15",
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

    // ------------------------------------------------ 3) TX: gửi ARP request thật
    let mac = net.config.mac;
    let mut request = [0u8; ARP_FRAME_BYTES];
    build_arp_request(mac, GATEWAY_IP, &mut request);
    {
        let before = virtio_net::stats();
        let result = virtio_net::send_frame(&request);
        let after = virtio_net::stats();
        let ok = result.is_ok()
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
            3,
            "net tx",
            &mut fails,
            "gửi frame ARP request thất bại",
        );
    }

    // ------------------------------------------------ 4) RX: nhận ARP reply của slirp
    {
        let mut frame = [0u8; MAX_FRAME_BYTES];
        let before = virtio_net::stats();
        let got = virtio_net::recv_frame_wait(&mut frame, RX_TIMEOUT_TICKS);
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
            4,
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
        let _ = writeln!(w, "PANIC: KT-245 net xong (4 TAP, {fails} fail)");
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
