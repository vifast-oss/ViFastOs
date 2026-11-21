// SPDX-License-Identifier: MIT OR Apache-2.0
//! **virtio-blk của ảnh x86_64** — KT-288 (K5' bước 9; D2-213).
//!
//! # Vì sao chạy **sau** preemption (TAP 22/23)
//!
//! TAP phải **tuần tự**: KT-274 đã chiếm 15..17 (PCI/net), KT-276/278 chiếm 18..21 (SMP),
//! KT-286 chiếm 22/23 (preemption). Đĩa là bước **cuối** trước marker, nên TAP 24/25 và chạy
//! sau `sched::run` — chỗ cuối cùng kernel còn chạy được (chưa có procsrv).
//!
//! # Hai phép đo (mỗi phép một dòng `#` + một TAP)
//!
//! 1. `# blk: device …` — `pci::init_for(VirtioKind::Blk)` tìm thấy thiết bị, BAR firmware đã
//!    gán **rời** BAR của virtio-net ([`virtio_blk::bar_disjoint`] — "leaf riêng" trên x86_64),
//!    driver init xong với `DRIVER_OK` đọc lại + `capacity > 0` + queue ≥ 3 descriptor.
//! 2. `# blk: sector0 hex=<32 hex> ascii="…" magic=…` — đọc **sector 0** của đĩa QEMU thật, in
//!    16 byte đầu dạng hex + ASCII (nếu in được) + `magic=` (khớp chữ ký runner ghi vào đĩa).
//!    Runner băm lại **cùng** 16 byte từ tệp bằng `od` ⇒ "đọc được sector 0" không thể là đọc ra
//!    số 0 rồi tự khen.
//!
//! Không có đĩa ⇒ `# blk: no device` + TAP 24 ĐỎ (ca âm của runner) — driver **fail-closed**,
//! không đọc ra số 0 rồi tự khen.
//!
//! # Đột biến (chỉ dùng để chứng minh bộ kiểm có răng, **không** phải feature sản phẩm)
//!
//! - `--features bad-blk-magic`: chữ ký đổi một byte ⇒ `magic=0` ⇒ TAP 25 ĐỎ.
//! - `--features bad-blk-bar`: BAR lấy **sai leaf** (địa chỉ chưa map) ⇒ `#PF` khi đọc common
//!   config ⇒ ảnh halt, không marker (ĐỎ).

use core::fmt::Write;

use vifast_plat_x86_64::pci::{self, PciError};
use vifast_plat_x86_64::timebase::PitClock;
use vifast_plat_x86_64::virtio_blk::{self, bar_disjoint, sector0_evidence, SECTOR_BYTES};
use vifast_virtio::features::DRIVER_OK;
use vifast_virtio::pci::VirtioKind;

use crate::{check, pit2_latch_read, Console};

/// Chạy hai phép đo PCI/virtio-blk, in TAP `tap_first`/`tap_first + 1`.
///
/// `scan` là kết quả quét PCI **đã có** của bước net (không quét lại toàn bus — cùng dữ liệu,
/// cùng thời điểm). `net_bar_base` là BAR của virtio-net: blk phải dùng **leaf khác**.
pub fn run(console: &mut Console, tap_first: usize, scan: &pci::ScanReport, net_bar_base: u64) {
    // ---------------------------------------------------------------- TAP 24: thiết bị
    let found = pci::init_for(VirtioKind::Blk, scan);
    let mut device_ok = false;
    let mut blk_report: Option<virtio_blk::BlkReport> = None;
    match &found {
        Ok(report) => {
            let _ = writeln!(
                console,
                "# pci: blk {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x} bar={} kind={} base=0x{:x} size=0x{:x} command=0x{:04x} caps=4",
                report.bus,
                report.dev,
                report.func,
                report.vendor,
                report.device,
                report.bar_index,
                pci::bar_kind_name(report.bar_kind),
                report.bar_base,
                report.bar_size,
                report.command,
            );
            // **Đột biến (iii)**: BAR sai leaf (địa chỉ chưa map) ⇒ lần đọc common config đầu
            // tiên `#PF` ⇒ ảnh halt, không marker. Chỉ dùng cho đột biến.
            let mut report = *report;
            if cfg!(feature = "bad-blk-bar") {
                report.bar_base = crate::PF_UNMAPPED_ADDR;
            }
            match virtio_blk::init(&report) {
                Ok(blk) => {
                    // "Leaf riêng" trên x86_64 = BAR blk **rời** BAR net (firmware gán, driver
                    // không ghi đè). Fail-closed: BAR 0/cỡ 0/chồng ⇒ `false`.
                    let distinct =
                        bar_disjoint(blk.bar_base, blk.bar_size, net_bar_base, report.bar_size);
                    let _ = writeln!(
                        console,
                        "# blk: device vendor=0x{:04x} device=0x{:04x} bar={} base=0x{:x} size=0x{:x} net_base=0x{:x} bar_distinct={} capacity={} blk_size={} size_max={} seg_max={} queue={} max={} status=0x{:02x} features=0x{:016x} driver=0x{:016x} flush={}",
                        blk.vendor,
                        blk.device,
                        blk.bar_index,
                        blk.bar_base,
                        blk.bar_size,
                        net_bar_base,
                        distinct as u8,
                        blk.capacity,
                        blk.blk_size,
                        blk.size_max,
                        blk.seg_max,
                        blk.queue_size,
                        blk.queue_max,
                        blk.status,
                        blk.device_features,
                        blk.driver_features,
                        blk.flush_ok as u8,
                    );
                    // TAP 24: thiết bị **thật** + BAR leaf riêng + driver ở DRIVER_OK + dung
                    // lượng đọc từ thiết bị + queue đủ chain 3 descriptor.
                    device_ok = distinct
                        && blk.status & DRIVER_OK != 0
                        && blk.capacity > 0
                        && blk.queue_size >= 3
                        && blk.blk_size == SECTOR_BYTES as u32;
                    blk_report = Some(blk);
                }
                Err(stage) => {
                    let _ = writeln!(console, "# blk: init failed stage={stage}");
                }
            }
        }
        Err(fail) => {
            // Không có thiết bị **khác** lỗi cấu hình: ca âm "không có đĩa" của runner grep
            // đúng dòng `# blk: no device`, nên chỉ in nó khi nguyên nhân **là** `NoDevice`.
            if fail.error == PciError::NoDevice {
                let _ = writeln!(console, "# blk: no device");
            } else {
                let _ = writeln!(
                    console,
                    "# blk: pci failed stage={} error={}",
                    fail.stage,
                    fail.error.name()
                );
            }
        }
    }
    check(console, tap_first, "blk: device", device_ok);

    // ---------------------------------------------------------------- TAP 25: sector 0
    //
    // Đồng hồ là PIT kênh 2 (mode 2 divisor `0xFFFF`, không ngắt) — bước này chạy sau `cli` nên
    // `TICK_COUNT` không tăng. `PitClock` cộng dồn count đếm-lùi và đổi sang µs (cùng khuôn net).
    let mut sector_ok = false;
    match blk_report {
        Some(blk) => {
            let mut clock = PitClock::new();
            clock.reset(pit2_latch_read());
            let mut now = || clock.now_us(pit2_latch_read());
            let mut sector = [0u8; SECTOR_BYTES];
            match virtio_blk::read_sector(0, &mut sector, &mut now) {
                Ok(()) => {
                    let mut first = [0u8; 16];
                    first.copy_from_slice(&sector[..16]);
                    let evidence = sector0_evidence(&first);
                    // **Đột biến (ii)**: chữ ký sai một byte ⇒ `magic=0` ⇒ TAP 25 ĐỎ (hex vẫn
                    // đúng — đây đúng là lớp lỗi "đọc được nhưng không nhận ra chữ ký").
                    let magic = if cfg!(feature = "bad-blk-magic") {
                        false
                    } else {
                        evidence.magic
                    };
                    let hex = core::str::from_utf8(&evidence.hex[..]).unwrap_or("?");
                    // Mượn **`&evidence.ascii`** (không move ra biến tạm): `evidence` sống tới
                    // hết khối, còn `bytes` move ra khỏi `Option` sẽ chết ngay trong nhánh
                    // `match` (E0597 đo được khi dựng KT-273).
                    let ascii = match &evidence.ascii {
                        Some(bytes) => core::str::from_utf8(&bytes[..]).unwrap_or("?"),
                        None => "-",
                    };
                    let stats = virtio_blk::stats();
                    let _ = writeln!(
                        console,
                        "# blk: sector0 hex={hex} ascii=\"{ascii}\" magic={} capacity={} status=0x{:02x} dma=0x{:x} desc=0x{:x} avail=0x{:x} used=0x{:x} requests={} completions={} errors={} timeouts={}",
                        magic as u8,
                        blk.capacity,
                        virtio_blk::device_status(),
                        blk.dma_pa,
                        blk.desc,
                        blk.avail,
                        blk.used,
                        stats.requests,
                        stats.completions,
                        stats.errors,
                        stats.timeouts,
                    );
                    // TAP 25: chữ ký khớp **và** request hoàn tất thật (≥ 1 completion, 0 lỗi,
                    // 0 timeout) — một ảnh đọc ra 512 byte số 0 vẫn có `magic=0` nên không thể
                    // xanh.
                    sector_ok = magic
                        && stats.completions >= 1
                        && stats.errors == 0
                        && stats.timeouts == 0
                        && stats.id_mismatch == 0;
                }
                Err(error) => {
                    let _ = writeln!(console, "# blk: sector0 read failed error={}", error.name());
                }
            }
        }
        None => {
            let _ = writeln!(console, "# blk: sector0 skipped (no device)");
        }
    }
    check(console, tap_first + 1, "blk: sector0", sector_ok);
}
