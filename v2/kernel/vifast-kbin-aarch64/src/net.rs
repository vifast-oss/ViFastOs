// SPDX-License-Identifier: MIT OR Apache-2.0
//! **virtio-net + DHCP + ARP + ICMP của ảnh aarch64** — KT-275 (K4' bước 3, D1 trên aarch64).
//!
//! # Vì sao chạy ở `el0_done` (sau U-mode, sau virtio-blk)
//!
//! TAP phải **tuần tự**: KT-273 đã chiếm TAP 19/20/21 cho PCI/virtio-blk, nên mạng là 22/23/24/25
//! và phải in **sau** `ok 21`. `el0_done` chạy ở EL1h với **cùng** `VSpace` đang hoạt động, nên
//! MMIO tới ECAM/BAR đi qua bảng trang đã map trong `high_entry` — không cần `&mut VSpace` ở đây.
//!
//! # Bốn phép đo (mỗi phép một dòng `#` + một TAP)
//!
//! 1. `# net: device …` — tìm `virtio-net` trên bus PCI (ECAM của KT-273), gán BAR trong leaf
//!    **riêng** của loại Net (không đụng leaf blk), khởi tạo driver, đọc **MAC thật** từ
//!    `virtio_net_config`.
//! 2. `# net: dhcp ip=… gw=… mask=… server=…` — DHCP **thật** của slirp (DISCOVER/OFFER/
//!    REQUEST/ACK), cùng khuôn `net.sh` riscv64 (KT-246).
//! 3. `# net: arp gw=<mac>` — ARP reply **thật** của slirp cho gateway.
//! 4. `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — ICMP echo **thật** qua gateway.
//!
//! # Không có thiết bị ⇒ **ĐỎ**, không im lặng
//!
//! `pci::init_for` trả `Err(NoDevice)` ⇒ in `# net: no device` (dòng **nguyên văn** mà runner
//! dùng cho ca âm "không có mạng") rồi cả bốn TAP đỏ ⇒ `el0_done` dừng **trước** marker. Nhờ vậy
//! một lần boot thiếu `-device virtio-net-pci` **không** thể trông giống boot sạch.
//!
//! # Codec: **không** viết lại
//!
//! Ethernet/ARP/IPv4/ICMP/UDP/DHCP là của `vifast-net` (KT-244, host test); tệp này chỉ gọi
//! driver `vifast_plat_aarch64::virtio_net` và in bằng chứng.

use core::fmt::{self, Write};
use core::sync::atomic::Ordering;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::pci::{self, bar_kind_name, PciError, VirtioKind};
use vifast_plat_aarch64::virtio_net::{self, NetConfig4, NetReport};
use vifast_virtio::features::DRIVER_OK;

use crate::{tap, Console, TEXT_START_PA};

/// Số lần echo request của phép đo ping (khớp `n=3` mà runner đòi).
pub const PING_COUNT: u32 = 3;

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
