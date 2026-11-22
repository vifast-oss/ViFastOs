// SPDX-License-Identifier: MIT OR Apache-2.0
//! **PCI ECAM + virtio-blk của ảnh aarch64** — KT-273 (K4' bước 3).
//!
//! # Vì sao chạy ở `el0_done` (sau U-mode) chứ không phải trong `high_entry`
//!
//! TAP phải **tuần tự**: KT-269 đã chiếm TAP 16/17/18 cho U-mode, nên PCI là 19/20/21 và phải
//! in **sau** `ok 18`. `el0_done` chạy ở EL1h với **cùng** `VSpace` đang hoạt động (U-mode chỉ
//! `eret` xuống EL0 rồi quay lại), nên MMIO tới ECAM/BAR vẫn đi qua bảng trang đã map trong
//! `high_entry` — không cần `&mut VSpace` ở đây.
//!
//! # Ba phép đo (mỗi phép một dòng `#` + một TAP)
//!
//! 1. `# pci: ecam …` — quét **toàn dải** bus/device/function, đọc host bridge (class phải
//!    `0x060000`) và một slot trống (phải `0xffff`). Hai điều kiện ấy **cùng đúng** chỉ khi ECAM
//!    trỏ đúng chỗ và giải mã thật.
//! 2. `# pci: virtio-blk found …` — tìm theo `(vendor, device)`, đo cỡ BAR bằng mask, **gán BAR**
//!    trong cửa sổ MMIO, đọc lại, bật `MEMORY|BUS_MASTER`, đọc capability list.
//! 3. `# blk: sector0 …` — khởi tạo driver, đọc **sector 0** của đĩa QEMU, in 16 byte đầu dạng
//!    hex + ASCII (nếu in được) + `magic=` (khớp chữ ký runner ghi vào đĩa).
//!
//! Không có đĩa ⇒ `# blk: no device` + TAP 20/21 ĐỎ (ca âm của runner) — driver **fail-closed**,
//! không đọc ra số 0 rồi tự khen.

use core::fmt::Write;

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::pci::{self, bar_kind_name, PciError, VirtioKind};
use vifast_plat_aarch64::virtio_blk::{self, sector0_evidence, SECTOR_BYTES};

use crate::{tap, Console};

/// Chạy ba phép đo PCI/virtio-blk, in TAP 19/20/21. Trả `true` ⇔ **cả ba** xanh.
///
/// In **đủ** ba dòng TAP kể cả khi phép trước đỏ: log phải cho biết **phép nào** hỏng, không
/// dừng ở phép đầu (một ảnh không có đĩa vẫn phải chứng minh ECAM chạy).
pub fn run(console: &mut Console) -> bool {
    let mut all_ok = true;

    // ---------------------------------------------------------------- TAP 19: ECAM
    let probe = pci::probe_ecam();
    let _ = writeln!(
        console,
        "# pci: ecam pa=0x{:x} va=0x{:x} host={:04x}:{:04x} class=0x{:06x} rev=0x{:02x} empty=0x{:04x} functions={} buses={}",
        pci::ECAM_BASE,
        pci::ECAM_VA_BASE,
        probe.host_vendor,
        probe.host_device,
        probe.host_class,
        probe.host_revision,
        probe.empty_vendor,
        probe.functions_seen,
        probe.buses_scanned,
    );
    let ecam_ok = probe.ok();
    if !tap(console, 19, "pci: ecam", ecam_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 20: tìm thiết bị
    let found = pci::init_for(VirtioKind::Blk);
    let mut found_ok = false;
    match &found {
        Ok(report) => {
            let _ = writeln!(
                console,
                "# pci: virtio-blk found bus={} dev={} func={} vendor=0x{:04x} device=0x{:04x} class=0x{:06x} rev=0x{:02x} bar={} kind={} size=0x{:x} assigned=0x{:x} readback=0x{:x} command=0x{:04x} caps=1 other_bars={} reserved={}",
                report.bus,
                report.dev,
                report.func,
                report.vendor,
                report.device,
                report.class,
                report.revision,
                report.bar_index,
                bar_kind_name(report.bar_kind),
                report.bar_size,
                report.bar_assigned,
                report.bar_readback,
                report.command,
                report.other_bars,
                report.reserved_ranges,
            );
            found_ok = true;
        }
        Err(fail) => {
            let _ = writeln!(
                console,
                "# pci: no virtio-blk stage={} error={}",
                fail.stage,
                fail.error.name()
            );
        }
    }
    if !tap(console, 20, "pci: virtio-blk found", found_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 21: sector 0
    let mut sector_ok = false;
    match &found {
        Ok(report) => {
            // **KT-338a**: trang DMA cấp động từ arena ngoài ảnh, map **identity** (`VA == PA`)
            // nên `dma_page_addr()` đã là PA — không còn đổi alias nửa cao như thời `.bss`.
            let dma_pa = asm::dma_page_addr();
            match virtio_blk::init(report, dma_pa) {
                Ok(blk) => {
                    let mut sector = [0u8; SECTOR_BYTES];
                    match virtio_blk::read_sector(0, &mut sector) {
                        Ok(()) => {
                            let mut first = [0u8; 16];
                            first.copy_from_slice(&sector[..16]);
                            let evidence = sector0_evidence(&first);
                            let hex = core::str::from_utf8(&evidence.hex[..]).unwrap_or("?");
                            // Mượn **`&evidence.ascii`** (không move ra biến tạm): `evidence`
                            // sống tới hết khối, còn `bytes` move ra khỏi `Option` sẽ chết ngay
                            // trong nhánh `match` (E0597 đo được khi dựng KT-273).
                            let ascii = match &evidence.ascii {
                                Some(bytes) => core::str::from_utf8(&bytes[..]).unwrap_or("?"),
                                None => "-",
                            };
                            let _ = writeln!(
                                console,
                                "# blk: sector0 hex={hex} ascii=\"{ascii}\" magic={} capacity={} status=0x{:02x} dma=0x{:x} desc=0x{:x} avail=0x{:x} used=0x{:x}",
                                evidence.magic as u8,
                                blk.capacity,
                                virtio_blk::device_status(),
                                blk.dma_pa,
                                blk.desc,
                                blk.avail,
                                blk.used,
                            );
                            sector_ok = evidence.magic;
                        }
                        Err(error) => {
                            let _ = writeln!(
                                console,
                                "# blk: sector0 read failed error={}",
                                error.name()
                            );
                        }
                    }
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
    if !tap(console, 21, "blk: sector0", sector_ok) {
        all_ok = false;
    }

    all_ok
}
