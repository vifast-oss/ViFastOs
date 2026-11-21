// SPDX-License-Identifier: MIT OR Apache-2.0
//! **PCI + virtio-net + ARP** của ảnh x86_64 — KT-274 (K5' bước 5; D2-208).
//!
//! # Vì sao chạy **sau** `Exit` của ring 3
//!
//! Ảnh chưa có procsrv/scheduler (D2-207 §10.6): `Exit` là chỗ duy nhất mà kernel còn chạy tiếp
//! được sau khi ring 3 kết thúc, và nó đang giữ `Console`. Vì vậy handler `Exit` gọi
//! [`run`] — TAP 15..17 — rồi **KT-276** gọi `smp::run` (TAP 18/19) và cuối cùng in marker + tắt
//! máy. Thứ tự TAP **giữ nguyên** 1..14 của các bước trước (KT-257/261/266/270), các ca mới nối
//! tiếp ở cuối.
//!
//! # Đường đi
//!
//! ```text
//!   pci::scan()            → ECAM (MCFG) hoặc legacy 0xCF8/0xCFC; in từng function có mặt
//!   TAP 15 `pci: ecam`     → ECAM **thật sự** dùng được (không phải legacy)
//!   pci::init_for(Net)     → BAR firmware đã gán + 4 capability + command MEMORY|BUS_MASTER
//!   virtio_net::init()     → reset/feature/2 queue/DRIVER_OK + MAC từ `virtio_net_config`
//!   TAP 16 `net: mac`      → MAC hợp lệ (không toàn 0/broadcast) + `DRIVER_OK` đọc lại
//!   virtio_net::arp_resolve(10.0.2.15 → 10.0.2.2)  → ARP request → **reply thật** của slirp
//!   TAP 17 `net: arp`      → MAC gateway không rỗng + đã nhận ≥ 1 frame + 0 lỗi
//!   smp::run()             → **KT-276**: INIT+SIPI hart 1, TAP 18/19 (xem `crate::smp`)
//!   marker + halt
//! ```
//!
//! # Không có thiết bị ⇒ **ĐỎ**, không im lặng
//!
//! `pci::init_for` trả `Err` ⇒ in `# net: no device` (dòng **nguyên văn** mà runner dùng cho ca âm
//! "không có mạng") rồi `check(16, …)` đỏ ⇒ ảnh dừng **trước** marker. Nhờ vậy một lần boot thiếu
//! `-device virtio-net-pci` **không** thể trông giống boot sạch.

use core::fmt::Write;

use vifast_net::addr::Ipv4Addr;
use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::pci::{self, AccessKind};
use vifast_plat_x86_64::timebase::PitClock;
use vifast_plat_x86_64::virtio_net;
use vifast_virtio::features::DRIVER_OK;
use vifast_virtio::pci::VirtioKind;

use crate::{check, pit2_latch_read, Console, BOOT_MARKER, HALT_LINE};

/// IP của ta trong mạng slirp (`-netdev user`): `10.0.2.15` — dữ kiện nền tảng QEMU (slirp cấp
/// dải `10.0.2.15..10.0.2.30`; bước này **không** DHCP nên dùng địa chỉ chuẩn).
pub const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// Gateway của slirp: `10.0.2.2` — nơi ARP request nhắm tới.
pub const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
/// Ngân sách ARP: 3 s (cùng ngân sách bước mạng của khuôn riscv64 KT-246).
pub const ARP_TIMEOUT_US: u64 = 3_000_000;

/// In MAC dạng `aa:bb:cc:dd:ee:ff`.
fn mac_str(mac: [u8; 6]) -> MacText {
    MacText(mac)
}

/// Định dạng MAC không cần `alloc`.
struct MacText([u8; 6]);

impl core::fmt::Display for MacText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

impl core::fmt::Display for IpText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let o = self.0.octets();
        write!(f, "{}.{}.{}.{}", o[0], o[1], o[2], o[3])
    }
}

/// Chạy phần PCI + virtio-net + ARP (TAP `tap_first`..`tap_first + 2`) rồi in marker + tắt máy.
///
/// **Không trả về**: `check` đỏ ⇒ `halt(3)` (không marker); xong hết ⇒ marker + `halt(0)`.
pub fn run(console: &mut Console, tap_first: usize) -> ! {
    // ---- 1. Quét PCI: in đường truy cập + từng function có mặt (bằng chứng enumerate thật).
    let scan = pci::scan();
    let _ = writeln!(
        console,
        "# pci: {} base=0x{:x} buses={}..{} functions={}",
        scan.access.name(),
        scan.ecam_base,
        scan.ecam_buses.0,
        scan.ecam_buses.1,
        scan.functions_seen,
    );
    for device in scan.devices() {
        let _ = writeln!(
            console,
            "# pci: dev {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x} class=0x{:02x}{:02x} {} hdr=0x{:02x}",
            device.bus,
            device.dev,
            device.func,
            device.vendor,
            device.device,
            device.class,
            device.subclass,
            pci::class_name(device.class),
            device.header_type,
        );
    }
    // TAP 15: ECAM **thật sự** là đường đã dùng (không rơi xuống legacy) và đọc được thiết bị.
    // `-M pc` (không có MCFG) ⇒ ĐỎ — đó là hợp đồng của runner (`-M q35`); đường legacy vẫn được
    // hiện thực và in `# pci: legacy …` trước khi TAP này đỏ.
    check(
        console,
        tap_first,
        "pci: ecam",
        scan.access == AccessKind::Ecam && scan.functions_seen > 0,
    );

    // ---- 2. Tìm virtio-net + BAR + capability.
    let pci = match pci::init_for(VirtioKind::Net, &scan) {
        Ok(report) => report,
        Err(fail) => {
            // Dòng **nguyên văn** cho ca âm "không có mạng" của runner.
            let _ = writeln!(console, "# net: no device");
            let _ = writeln!(
                console,
                "# net: pci stage={} error={}",
                fail.stage,
                fail.error.name(),
            );
            check(console, tap_first + 1, "net: mac", false);
            unreachable!()
        }
    };
    let _ = writeln!(
        console,
        "# pci: net {:02x}:{:02x}.{} bar={} kind={} base=0x{:x} size=0x{:x} command=0x{:04x} caps=4",
        pci.bus,
        pci.dev,
        pci.func,
        pci.bar_index,
        pci::bar_kind_name(pci.bar_kind),
        pci.bar_base,
        pci.bar_size,
        pci.command,
    );

    // ---- 3. Khởi tạo driver: reset → feature → 2 queue → DRIVER_OK → MAC.
    let report = match virtio_net::init(&pci) {
        Ok(report) => report,
        Err(stage) => {
            let _ = writeln!(console, "# net: init failed stage={stage}");
            check(console, tap_first + 1, "net: mac", false);
            unreachable!()
        }
    };
    let _ = writeln!(
        console,
        "# net: mac={} mtu={} status=0x{:04x} queues={} rx={} tx={} features=0x{:016x} driver=0x{:016x}",
        mac_str(report.config.mac),
        report.config.effective_mtu(),
        report.config.status,
        report.num_queues,
        report.queue_size_rx,
        report.queue_size_tx,
        report.device_features,
        report.driver_features,
    );
    // TAP 16: MAC **đọc từ thiết bị** và hợp lệ (đã đàm phán `VIRTIO_NET_F_MAC`, không toàn
    // 0/broadcast) + `device_status` đọc lại còn `DRIVER_OK`.
    check(
        console,
        tap_first + 1,
        "net: mac",
        report.mac_ok
            && report.config.mac_hop_le()
            && report.status & DRIVER_OK != 0
            && report.queue_size_rx >= 2
            && report.queue_size_tx >= 2,
    );

    // ---- 4. ARP: hỏi MAC của gateway, chờ reply **thật** của slirp.
    //
    // Đồng hồ là PIT kênh 2 (mode 2 divisor `0xFFFF`, không ngắt) — bước này chạy sau `cli` nên
    // `TICK_COUNT` không tăng. `PitClock` cộng dồn count đếm-lùi và đổi sang µs.
    let mut clock = PitClock::new();
    clock.reset(pit2_latch_read());
    let mut now = || clock.now_us(pit2_latch_read());
    let started = now();
    match virtio_net::arp_resolve(OUR_IP, GATEWAY_IP, ARP_TIMEOUT_US, &mut now) {
        Ok(found) => {
            let elapsed = now().saturating_sub(started);
            let stats = virtio_net::stats();
            let _ = writeln!(
                console,
                "# net: arp gw={} mac={} sent={} recv={} elapsed_us={} errors={} timeouts={}",
                IpText(GATEWAY_IP),
                mac_str(found.octets()),
                stats.tx_frames,
                stats.rx_frames,
                elapsed,
                stats.errors,
                stats.timeouts,
            );
            // TAP 17: reply **thật** — MAC gateway không rỗng/broadcast, đã nhận ≥ 1 frame, và
            // không có lỗi/timeout nào (một timeout rồi thử lại là dấu hiệu đường hỏng).
            check(
                console,
                tap_first + 2,
                "net: arp",
                !found.is_zero()
                    && !found.is_broadcast()
                    && stats.rx_frames >= 1
                    && stats.errors == 0
                    && stats.timeouts == 0,
            );
        }
        Err(error) => {
            let _ = writeln!(
                console,
                "# net: arp gw={} error={}",
                IpText(GATEWAY_IP),
                error.name(),
            );
            check(console, tap_first + 2, "net: arp", false);
        }
    }

    // ---- 5. **KT-276** (K5' bước 6): SMP 2 hart — TAP 18/19. Chạy **sau** TAP 17 và **trước**
    // marker: đây là chỗ cuối cùng kernel còn chạy được sau `Exit` của ring 3 (chưa có procsrv).
    crate::smp::run(console, tap_first + 3);

    // ---- 5b. **KT-286** (K5' bước 8): preemption — hai tác vụ kernel + timer chuyển ngữ cảnh,
    // TAP 22/23. Chạy **sau** TAP 21 (SMP) và **trước** marker; thứ tự TAP 1..21 **không đổi**.
    crate::sched::run(console, tap_first + 7);

    // ---- 5c. **KT-288** (K5' bước 9): virtio-blk — TAP 24/25. Chạy **sau** TAP 23 và **trước**
    // marker. `scan` là kết quả quét PCI **đã có** (không quét lại toàn bus) và `pci.bar_base`
    // là BAR của virtio-net — blk phải chứng minh **leaf riêng** (không đè BAR net).
    crate::blk::run(console, tap_first + 9, &scan, pci.bar_base);

    // ---- 6. Xong: marker **duy nhất** + halt sạch (đường cũ của handler `Exit`).
    let _ = write!(console, "{BOOT_MARKER}");
    let _ = writeln!(console, "{HALT_LINE}");
    asm::halt(0)
}
