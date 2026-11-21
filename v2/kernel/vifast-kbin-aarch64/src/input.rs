// SPDX-License-Identifier: MIT OR Apache-2.0
//! **virtio-input (bàn phím) của ảnh aarch64** — KT-283 (K4' bước 3, D1 phần 3 trên aarch64).
//!
//! # Vì sao chạy ở `el0_done` (sau U-mode, sau virtio-gpu)
//!
//! TAP phải **tuần tự**: KT-279 đã chiếm TAP 28/29/30 cho virtio-gpu, nên bàn phím là 31/32 và
//! phải in **sau** `ok 30`. `el0_done` chạy ở EL1h với **cùng** `VSpace` đang hoạt động, nên MMIO
//! tới ECAM/BAR đi qua bảng trang đã map trong `high_entry` — không cần `&mut VSpace` ở đây.
//!
//! # Hai phép đo (mỗi phép một dòng `#` + một TAP)
//!
//! 1. `# input pci:` + `# input cap:` + `# input name:` + `# input dev:` + `# input dma:` — tìm
//!    `virtio-input` trên bus PCI (ECAM của KT-273), gán BAR trong leaf **riêng** của loại Input
//!    (`0x1060_0000`, không đụng leaf blk/net/gpu), khởi tạo driver, đọc **tên thật** từ
//!    `virtio_input_config` (`select = ID_NAME`), danh tính (`ID_DEVIDS`), bitmap `EV_BITS` với
//!    `subsel = EV_KEY`, và bật **hai** queue `event`/`status`.
//! 2. `# input: key a code=30 ascii=0x61 shift=0` — chờ sự kiện `EV_KEY KEY_A 1` do **QEMU
//!    `sendkey a`** bơm vào (runner gửi qua HMP monitor **sau** dòng `# input: ready`), chuyển mã
//!    ra `'a'` (0x61) bằng `vifast_virtio::input::KeyState` — **không** có bảng thứ hai.
//!
//! # Không có thiết bị ⇒ **ĐỎ**, không im lặng
//!
//! `pci::init_for` trả `Err(NoDevice)` ⇒ in `# input: no device` (dòng **nguyên văn** mà runner
//! dùng cho ca âm "không có bàn phím") rồi **cả hai** TAP đỏ ⇒ `el0_done` dừng **trước** marker.
//! Nhờ vậy một lần boot thiếu `-device virtio-keyboard-pci` **không** thể trông giống boot sạch.
//!
//! # Bảng chuyển mã: **không** viết lại
//!
//! Mã phím → ASCII là của `vifast_virtio::input` (16 test host bên riscv64, KT-255). Tệp này chỉ
//! gọi driver `vifast_plat_aarch64::virtio_input` và in bằng chứng.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::mmu::{MapError, TableMemory};
use vifast_plat_aarch64::pci::{self, bar_kind_name, PciError, VirtioKind};
use vifast_plat_aarch64::virtio_input::{self, InputReport};
use vifast_plat_aarch64::vspace::VSpace;
use vifast_virtio::input::{key_to_ascii, KeyState, EVENT_BUFS, KEY_A};
use vifast_virtio::pci::{COMMAND_BUS_MASTER, COMMAND_MEMORY};

use crate::mapwin::map_window_into;
use crate::{tap, Console, TEXT_START_PA};

/// Số giây chờ **tối đa** cho lô `sendkey a` (bộ đếm vật lý `CNTPCT_EL0`).
///
/// Runner bơm phím **sau** khi thấy `# input: ready` (không `sleep` mù), nên 5 s là dư rất nhiều;
/// hết hạn ⇒ TAP 32 đỏ (fail-closed, **không** treo). Ngắn hơn 10 s của riscv64 (`input.sh`) vì
/// soak aarch64 chạy 1.000 boot — một boot mất phím chỉ chịu 5 s, không 10 s.
pub const INPUT_WAIT_SECS: u64 = 5;

/// PA của trang DMA **đọc lại từ bảng trang** (0 ⇔ chưa đo) — `main.rs` ghi sau khi map ảnh.
static DMA_WALK_PA: AtomicU64 = AtomicU64::new(0);

/// Ghi PA của trang DMA đọc lại từ bảng trang (gọi từ `main.rs` trong `high_entry`).
pub fn publish_dma_walk(pa: u64) {
    DMA_WALK_PA.store(pa, Ordering::Relaxed);
}

/// PA đã đọc lại từ bảng trang (0 ⇔ chưa map).
pub fn dma_walk_pa() -> u64 {
    DMA_WALK_PA.load(Ordering::Relaxed)
}

/// `true` ⇔ trang DMA map **identity** (`VA == PA`) — điều kiện để thiết bị DMA ghi đúng dải PA
/// mà driver đọc bằng con trỏ trong ảnh.
fn dma_identity() -> bool {
    let walk = dma_walk_pa();
    walk != 0 && walk == virtio_input::dma_pa()
}

/// Map **cửa sổ BAR** của virtio-input (leaf 3, `0x1060_0000`) vào `VSpace` và đo **PA** của
/// trang DMA từ bảng trang — gọi từ `high_entry` **trước** `activate`, cùng chỗ với leaf
/// blk/net/gpu.
///
/// Trả `(bar_base, pages, dma_va, dma_walk_pa)`. Vì sao ở đây chứ không ở `main.rs`: `main.rs` đã
/// sát trần R1 (1.500 dòng) — luật cấu trúc, không phải phong cách.
///
/// Hai phép **đo** (không phải lời khai):
/// - cửa sổ BAR phải **chưa** có mapping trước khi gán (đè im lặng = CAO-2/D2-110C);
/// - `walk(dma_va)` phải trả **đúng** PA mà driver ghi vào descriptor — `run` in `identity=` từ
///   số này, nên "trang DMA map identity" là kết quả của bảng trang, không phải hằng.
pub fn map_window<M: TableMemory>(
    vs: &mut VSpace<M>,
) -> Result<(u64, usize, u64, u64), &'static str> {
    let bar_base = pci::bar_assign_base(VirtioKind::Input);
    if vs.walk(bar_base) != Err(MapError::NotMapped) {
        return Err("bar_mapped");
    }
    let pages = map_window_into(
        vs,
        bar_base,
        bar_base,
        pci::BAR_ASSIGN_BYTES,
        Perms::KERNEL_RW,
        MemAttr::Device,
    )
    .map_err(|_| "bar_map")?;
    let dma_va = asm::dma_page_input_addr();
    let dma_walk_pa = vs.walk(dma_va).map_err(|_| "dma_walk")?.pa;
    publish_dma_walk(dma_walk_pa);
    Ok((bar_base, pages, dma_va, dma_walk_pa))
}

/// `true` ⇔ `haystack` chứa `needle` (không dùng `str::contains` để tránh kéo thêm code vào ảnh).
fn contains(haystack: &str, needle: &str) -> bool {
    let hay = haystack.as_bytes();
    let pin = needle.as_bytes();
    if pin.is_empty() || hay.len() < pin.len() {
        return false;
    }
    let mut start = 0;
    while start + pin.len() <= hay.len() {
        if &hay[start..start + pin.len()] == pin {
            return true;
        }
        start += 1;
    }
    false
}

/// Chạy hai phép đo virtio-input, in TAP 31/32. Trả `true` ⇔ **cả hai** xanh.
///
/// In **đủ** hai dòng TAP kể cả khi phép đầu đỏ: log phải cho biết **phép nào** hỏng, và một ảnh
/// không có bàn phím vẫn phải in `not ok 31` **và** `not ok 32` (plan `1..32` đủ dòng).
pub fn run(console: &mut Console) -> bool {
    let mut all_ok = true;

    // ---------------------------------------------------------------- TAP 31: thiết bị
    let found = pci::init_for(VirtioKind::Input);
    let mut device_ok = false;
    let mut report: Option<InputReport> = None;
    match &found {
        Ok(pci_report) => {
            let _ = writeln!(
                console,
                "# input pci: bus={} dev={} func={} vendor=0x{:04x} device=0x{:04x} class=0x{:06x} rev=0x{:02x} bar={} kind={} size=0x{:x} assigned=0x{:x} readback=0x{:x} command=0x{:04x} caps=1 other_bars={} reserved={}",
                pci_report.bus,
                pci_report.dev,
                pci_report.func,
                pci_report.vendor,
                pci_report.device,
                pci_report.class,
                pci_report.revision,
                pci_report.bar_index,
                bar_kind_name(pci_report.bar_kind),
                pci_report.bar_size,
                pci_report.bar_assigned,
                pci_report.bar_readback,
                pci_report.command,
                pci_report.other_bars,
                pci_report.reserved_ranges,
            );
            let _ = writeln!(
                console,
                "# input cap: common=0x{:x} notify=0x{:x} device=0x{:x} isr=0x{:x} mul={} cap_bar={} ecam=0x{:x} cap_ranges_ok={} overlap_checked={}",
                pci_report.caps.common.offset,
                pci_report.caps.notify.offset,
                pci_report.caps.device.offset,
                pci_report.caps.isr.offset,
                pci_report.caps.notify_off_multiplier,
                pci_report.caps.bar,
                pci_report.ecam_va,
                pci_report.cap_ranges_ok as u8,
                pci_report.overlap_checked as u8,
            );
            // Trang DMA nằm trong `.bss` của ảnh: sau `activate` con trỏ là **alias nửa cao**,
            // còn thiết bị DMA bằng **PA** ⇒ đổi bằng `high_to_pa` (cùng công thức blk/gpu).
            let text_start = TEXT_START_PA.load(Ordering::Relaxed);
            let dma_pa = virtio_input::high_to_pa(asm::dma_page_input_addr(), text_start);
            match virtio_input::init(pci_report, dma_pa) {
                Ok(input) => {
                    let name = input.name();
                    let version_ok = input.device_features & (1u64 << 32) != 0
                        && input.driver_features & (1u64 << 32) != 0;
                    let status = virtio_input::device_status();
                    let dma_ok = dma_identity();
                    // Tên QEMU đặt cho `virtio-keyboard` là `QEMU Virtio Keyboard`
                    // (`VIRTIO_ID_NAME_KEYBOARD`, hw/input/virtio-input-hid.c) — hằng này chỉ để
                    // đối chiếu **nguồn gốc**; tên vẫn là giá trị **đọc từ thiết bị**.
                    let name_ok = input.name_len as usize >= 8 && contains(name, "Keyboard");
                    let _ = writeln!(console, "# input name: {name}");
                    let _ = writeln!(
                        console,
                        "# input dev: name_len={} devids_bustype={} devids_vendor=0x{:x} devids_product=0x{:x} devids_version=0x{:x} num_queues={} queue_max={} queue_size={} status_queue_size={} device_features=0x{:x} driver_features=0x{:x} version1={} rep_feature={} device_status=0x{:02x} config_generation={} ev_bits_len={} key_a_bit={}",
                        input.name_len,
                        input.devids.bustype,
                        input.devids.vendor,
                        input.devids.product,
                        input.devids.version,
                        input.num_queues,
                        input.queue_max,
                        input.queue_size,
                        input.status_queue_size,
                        input.device_features,
                        input.driver_features,
                        version_ok as u8,
                        (input.driver_features & 1) as u8,
                        status,
                        input.config_generation,
                        input.ev_bits_len,
                        input.key_a_supported as u8,
                    );
                    let _ = writeln!(
                        console,
                        "# input dma: desc=0x{:x} avail=0x{:x} used=0x{:x} buf=0x{:x} dma=0x{:x} walk=0x{:x} identity={} armed={} notify_off={} notify_addr=0x{:x} queue=0 status_notify_off={} status_queue_idle={}",
                        input.desc,
                        input.avail,
                        input.used,
                        input.buf_pa,
                        input.dma_pa,
                        dma_walk_pa(),
                        dma_ok as u8,
                        input.armed,
                        input.notify_off,
                        input.notify_addr,
                        virtio_input::status_notify_off(),
                        virtio_input::status_queue_idle() as u8,
                    );
                    device_ok = pci_report.vendor == 0x1af4
                        && (pci_report.device == 0x1052 || pci_report.device == 0x1012)
                        && pci_report.bar_readback == pci_report.bar_assigned
                        && pci_report.command & (COMMAND_MEMORY | COMMAND_BUS_MASTER)
                            == COMMAND_MEMORY | COMMAND_BUS_MASTER
                        && pci_report.bar_size >= 0x1000
                        && pci_report.overlap_checked
                        && pci_report.cap_ranges_ok
                        && version_ok
                        && input.driver_features
                            == vifast_virtio::input::negotiated_features(input.device_features)
                        && status & 0x0f == 0x0f
                        && input.queue_size >= 2
                        && input.queue_size.is_power_of_two()
                        && input.num_queues >= 2
                        && input.status_queue_size >= 2
                        && input.armed == EVENT_BUFS as u16
                        && input.key_a_supported
                        && dma_ok
                        && name_ok;
                    report = Some(input);
                }
                Err(stage) => {
                    let _ = writeln!(console, "# input dev: init_fail={stage}");
                }
            }
        }
        Err(fail) => {
            // Không có thiết bị **khác** lỗi cấu hình: ca âm "không có bàn phím" của runner grep
            // đúng dòng `# input: no device`, nên chỉ in nó khi nguyên nhân **là** `NoDevice`.
            if fail.error == PciError::NoDevice {
                let _ = writeln!(console, "# input: no device");
            } else {
                let _ = writeln!(
                    console,
                    "# input pci: err={} stage={}",
                    fail.error.name(),
                    fail.stage
                );
            }
        }
    }
    if !tap(console, 31, "input: device", device_ok) {
        all_ok = false;
    }
    if !device_ok {
        // Plan `1..32` phải đủ dòng: ca "không có bàn phím" in **cả hai** TAP đỏ.
        tap(console, 32, "input: key", false);
        return false;
    }
    let _ = report;

    // ---------------------------------------------------------------- TAP 32: một phím thật
    // Runner bơm `sendkey a` **sau** dòng này (không `sleep` mù): ảnh đã arm 8 đệm ở `init`.
    let _ = writeln!(console, "# input: ready");
    let mut keys = KeyState::new();
    let mut saw_a = false;
    let mut a_code = 0u16;
    let mut a_shift = false;
    let started = virtio_input::now_ticks();
    let deadline = virtio_input::ticks_for_secs(INPUT_WAIT_SECS).map(|t| started.wrapping_add(t));
    loop {
        let remaining = match deadline {
            Some(limit) => {
                let now = virtio_input::now_ticks();
                if now > limit {
                    break;
                }
                limit.wrapping_sub(now)
            }
            // `CNTFRQ_EL0 = 0` ⇒ `poll_event_wait(0)` lùi về trần **số vòng** (ghi rõ ở driver).
            None => 0,
        };
        let Some(event) = virtio_input::poll_event_wait(remaining) else {
            break;
        };
        let shift_before = keys.shift();
        let ascii = keys.feed(event);
        if event.is_key_down() && ascii == Some(b'a') {
            saw_a = true;
            a_code = event.code;
            a_shift = shift_before;
            break;
        }
    }
    let elapsed_ms = match virtio_input::ticks_for_secs(1) {
        Some(freq) => virtio_input::now_ticks().wrapping_sub(started) * 1000 / freq,
        None => 0,
    };
    let key_ok = saw_a && a_code == KEY_A && !a_shift;
    {
        let _ = writeln!(
            console,
            "# input: key a code={} ascii=0x{:02x} shift={} elapsed_ms={elapsed_ms}",
            a_code,
            key_to_ascii(a_code, a_shift).unwrap_or(0),
            a_shift as u8,
        );
    }
    if !tap(console, 32, "input: key", key_ok) {
        all_ok = false;
    }

    // Bộ đếm driver: `errors=0 timeouts=0 id_mismatch=0` là bằng chứng đường đi sạch; ba bộ đếm
    // `clean_*`/`inval_*` là bằng chứng cache maintenance **đã chạy** (KT-277 — bỏ `dc` ⇒ 0 ⇒ ĐỎ).
    let stats = virtio_input::stats();
    let _ = writeln!(
        console,
        "# input stats: events={} syncs={} key_downs={} key_ups={} notifies={} errors={} timeouts={} id_mismatch={} clean_lines={} clean_bytes={} clean_ops={} inval_lines={} inval_ops={} published={} queue_size={} device_status=0x{:02x}",
        stats.events,
        stats.syncs,
        stats.key_downs,
        stats.key_ups,
        stats.notifies,
        stats.errors,
        stats.timeouts,
        stats.id_mismatch,
        stats.clean_lines,
        stats.clean_bytes,
        stats.clean_ops,
        stats.inval_lines,
        stats.inval_ops,
        virtio_input::published(),
        virtio_input::queue_size(),
        virtio_input::device_status(),
    );

    all_ok
}
