// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test virtio-input** (bàn phím) riscv64 (KT-255, D1 phần 3) — bin
//! `vifast-kbin-ktest-input`, feature `ktest,input`, nạp qua OpenSBI **kèm**
//! `-device virtio-keyboard-pci` + monitor để bơm `sendkey`.
//!
//! # Vì sao ảnh riêng
//!
//! (a) Ảnh `ktest` đã ở **92,6 %** trần `.data+.bss` (D2-109) — driver + trang DMA 4 KiB đẩy nó
//! sát trần; (b) ảnh này cần **thiết bị thật** (`-device virtio-keyboard-pci`) **và** điều khiển
//! QEMU từ ngoài (monitor `sendkey`) trong lúc khách đang chạy — tách ra thì 21 boot matrix cũ
//! **không đổi**; (c) boot kết thúc bằng **một** dòng `PANIC:` chủ đích (cùng quy ước ảnh
//! guard/user/proc/vblk/con/net).
//!
//! # TAP (5 test; tên khớp `v2/test/boot/input.sh`)
//!
//! 1. `input device` — quét ECAM tìm **vendor/device đọc từ thiết bị** (`0x1af4:0x1012`), đo cỡ
//!    BAR bằng mask, **gán** BAR (firmware không enumerate), đọc lại BAR + `command`, đọc bốn
//!    capability; ECAM/BAR được map identity (walk phần mềm); queue `event` bật + arm 8 đệm.
//! 2. `input name` — đọc `virtio_input_config` với `select = ID_NAME`: tên **đọc từ thiết bị**
//!    (QEMU: `QEMU Virtio Keyboard`), kèm `ID_DEVIDS` + bitmap `EV_BITS`/`EV_KEY`.
//! 3. `input key a` — chờ sự kiện `EV_KEY KEY_A 1` do **QEMU `sendkey a`** bơm vào, chuyển mã ra
//!    `'a'` (0x61).
//! 4. `input shift` — chờ lô `sendkey shift-a`: `KEY_LEFTSHIFT 1` rồi `KEY_A 1` ⇒ `'A'` (0x41).
//! 5. `input unknown key` — chờ `sendkey f1` (`KEY_F1` = 59): mã **không** có trong bảng chuyển mã
//!    ⇒ `key_to_ascii` trả `None` (ca âm của bảng, chạy trên thiết bị thật).
//!
//! Mọi dòng `# input:` in từ **biến đo** (D2-102); giá trị suy ra in kèm công thức/đầu vào
//! (D2-106). Không `unsafe` trong tệp này (MMIO nằm ở `kernel/vifast-plat-riscv64`).

use core::fmt::Write;

use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::timer::TIMEBASE_HZ;
use vifast_plat_riscv64::virtio_input;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::input::{
    is_shift, key_to_ascii, KeyState, EVENT_BUFS, EV_KEY, KEY_A, KEY_F1, Q_EVENT,
};
use vifast_virtio::pci::{COMMAND_BUS_MASTER, COMMAND_MEMORY};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Thời gian chờ **tối đa** cho ba lô phím (tick timebase) — 10 s ảo.
///
/// Runner bơm phím **sau** khi thấy `# input: ready` (không dùng `sleep` mù), nên 10 s là dư rất
/// nhiều; hết hạn ⇒ TAP 3..5 đỏ (fail-closed, **không** treo). Runner chạy QEMU **không** `-icount`
/// nên 1 tick = 1/10.000.000 s thật (xem `input.sh`).
const INPUT_WAIT_TICKS: u64 = 10 * TIMEBASE_HZ;

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

pub fn run() -> ! {
    PLATFORM.put_str("TAP version 13\n1..5\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# input image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} tables={} used={} \
             const:wait_ms={} const:event_bufs={}",
            PLATFORM.hart_id(),
            stvec,
            (stvec & 0b11 == 0) as u8,
            mmu_report.satp,
            mmu_report.satp >> 60,
            mmu_report.tables,
            mmu_report.used,
            INPUT_WAIT_TICKS * 1000 / TIMEBASE_HZ,
            EVENT_BUFS
        );
    }
    let mut fails = 0usize;

    // ------------------------------------------------ 1) PCI: tìm + gán BAR + capability
    let pci_report = match pci::init_for(vifast_virtio::pci::VirtioKind::Input) {
        Ok(report) => {
            let ecam_ok = identity(report.ecam_addr);
            let bar_ok = identity(report.bar_assigned);
            let ok = report.vendor == 0x1af4
                && (report.device == 0x1012 || report.device == 0x1052)
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
                    "# input pci: functions={} bus={} dev={} func={} vendor=0x{:x} device=0x{:x} \
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
                    "# input cap: common=0x{:x} notify=0x{:x} device=0x{:x} isr=0x{:x} mul={} \
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
                "input device",
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
                "# input pci: err={} stage={}",
                fail.error.name(),
                fail.stage
            );
            // Dòng **cố định** cho runner + ca âm (D2-200): không có `-device virtio-keyboard-pci`
            // thì ảnh phải in đúng dòng này và TAP **đỏ**, không PASS giả.
            let _ = writeln!(w, "# input: no device");
            put(
                false,
                1,
                "input device",
                &mut fails,
                "không tìm/khởi tạo được PCI",
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };

    // ------------------------------------------------ 2) feature + queue + tên thật
    let input = match virtio_input::init(&pci_report) {
        Ok(report) => {
            let dma_ok = identity(report.desc)
                && identity(report.avail)
                && identity(report.used)
                && identity(report.buf_pa);
            let version_ok = report.device_features & (1u64 << 32) != 0
                && report.driver_features & (1u64 << 32) != 0;
            let name = report.name();
            // Tên QEMU đặt cho `virtio-keyboard` là `QEMU Virtio Keyboard`
            // (`VIRTIO_ID_NAME_KEYBOARD`, hw/input/virtio-input-hid.c) — hằng này chỉ để đối chiếu
            // **nguồn gốc**, tên vẫn là giá trị **đọc từ thiết bị** (không hardcode trong ảnh).
            let name_ok = report.name_len as usize >= 8 && contains(name, "Keyboard");
            let status = virtio_input::device_status();
            let ok = version_ok
                && report.driver_features
                    == vifast_virtio::input::negotiated_features(report.device_features)
                && status & 0x0f == 0x0f
                && report.queue_size >= 2
                && report.queue_size.is_power_of_two()
                && report.num_queues >= 1
                && report.armed == EVENT_BUFS as u16
                && report.key_a_supported
                && dma_ok
                && name_ok;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(w, "# input name: {name}");
                let _ = writeln!(
                    w,
                    "# input dev: name_len={} devids_bustype={} devids_vendor=0x{:x} \
                     devids_product=0x{:x} devids_version=0x{:x} num_queues={} queue_max={} \
                     queue_size={} device_features=0x{:x} driver_features=0x{:x} version1={} \
                     rep_feature={} device_status=0x{:x} config_generation={} ev_bits_len={} \
                     key_a_bit={}",
                    report.name_len,
                    report.devids.bustype,
                    report.devids.vendor,
                    report.devids.product,
                    report.devids.version,
                    report.num_queues,
                    report.queue_max,
                    report.queue_size,
                    report.device_features,
                    report.driver_features,
                    version_ok as u8,
                    (report.driver_features & 1) as u8,
                    status,
                    report.config_generation,
                    report.ev_bits_len,
                    report.key_a_supported as u8
                );
                let _ = writeln!(
                    w,
                    "# input dma: desc=0x{:x} avail=0x{:x} used=0x{:x} buf=0x{:x} identity={} \
                     armed={} notify_off={} notify_addr=0x{:x} queue={}",
                    report.desc,
                    report.avail,
                    report.used,
                    report.buf_pa,
                    dma_ok as u8,
                    report.armed,
                    report.notify_off,
                    report.notify_addr,
                    Q_EVENT
                );
            }
            put(
                ok,
                2,
                "input name",
                &mut fails,
                "feature/queue/tên/DMA không đúng",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            report
        }
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# input dev: init_fail={stage}");
            put(false, 2, "input name", &mut fails, "init thiết bị thất bại");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let _ = input;

    // ------------------------------------------------ 3..5) chờ ba lô phím của QEMU
    // Runner bơm: `sendkey a`, `sendkey shift-a`, `sendkey f1` **sau** dòng `# input: ready`.
    PLATFORM.put_str("# input: ready\n");
    let mut keys = KeyState::new();
    let mut saw_a = false;
    let mut saw_shift = false;
    let mut saw_unknown = false;
    let mut a_code = 0u16;
    let mut shift_code = 0u16;
    let mut unknown_code = 0u16;
    let started = PLATFORM.now_ticks();
    let deadline = started.wrapping_add(INPUT_WAIT_TICKS);
    while !(saw_a && saw_shift && saw_unknown) {
        let now = PLATFORM.now_ticks();
        if now > deadline {
            break;
        }
        let remaining = deadline.wrapping_sub(now);
        let Some(event) = virtio_input::poll_event_wait(remaining) else {
            break;
        };
        if event.event_type != EV_KEY {
            // `EV_SYN`/`SYN_REPORT` kết thúc lô — không sinh ký tự, không đổi Shift.
            keys.feed(event);
            continue;
        }
        let shift_before = keys.shift();
        let ascii = keys.feed(event);
        if !event.is_key_down() {
            continue;
        }
        match ascii {
            Some(b'a') => {
                saw_a = true;
                a_code = event.code;
            }
            Some(b'A') => {
                saw_shift = true;
                shift_code = event.code;
            }
            _ => {
                // Phím **không** sinh ASCII và **không** phải Shift ⇒ ca "mã lạ" của bảng.
                if !is_shift(event.code) && key_to_ascii(event.code, shift_before).is_none() {
                    saw_unknown = true;
                    unknown_code = event.code;
                }
            }
        }
    }
    let elapsed_ms = PLATFORM.now_ticks().wrapping_sub(started) * 1000 / TIMEBASE_HZ;

    // ------------------------------------------------ 3) `input key a`
    {
        let ok = saw_a && a_code == KEY_A;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# input: key a code={} ascii=0x{:02x} shift=0 elapsed_ms={elapsed_ms}",
                a_code,
                key_to_ascii(a_code, false).unwrap_or(0)
            );
        }
        put(
            ok,
            3,
            "input key a",
            &mut fails,
            "không nhận được EV_KEY KEY_A (sendkey a)",
        );
    }

    // ------------------------------------------------ 4) `input shift`
    {
        let ok = saw_shift && shift_code == KEY_A;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# input: shift code={} ascii=0x{:02x} shift=1 elapsed_ms={elapsed_ms}",
                shift_code,
                key_to_ascii(shift_code, true).unwrap_or(0)
            );
        }
        put(
            ok,
            4,
            "input shift",
            &mut fails,
            "không nhận được lô sendkey shift-a",
        );
    }

    // ------------------------------------------------ 5) `input unknown key`
    {
        let ok = saw_unknown && key_to_ascii(unknown_code, false).is_none();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# input: unknown code={} ascii=none const:f1={} elapsed_ms={elapsed_ms}",
                unknown_code, KEY_F1
            );
        }
        put(
            ok,
            5,
            "input unknown key",
            &mut fails,
            "không nhận được phím không có ASCII (sendkey f1)",
        );
    }

    // ------------------------------------------------ kết thúc chủ đích
    let stats = virtio_input::stats();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# input stats: events={} syncs={} key_downs={} key_ups={} notifies={} errors={} \
             timeouts={} id_mismatch={} published={} queue_size={} device_status=0x{:x}",
            stats.events,
            stats.syncs,
            stats.key_downs,
            stats.key_ups,
            stats.notifies,
            stats.errors,
            stats.timeouts,
            stats.id_mismatch,
            virtio_input::published(),
            input.queue_size,
            virtio_input::device_status()
        );
        // Dấu kết thúc **chủ đích** (không có fault): runner đòi đúng **một** dòng PANIC cho mỗi
        // boot; ảnh này không fault nên tự in dấu kết thúc rồi `halt(0)`. Nội dung lấy từ `fails`.
        let _ = writeln!(w, "PANIC: KT-255 input xong (5 TAP, {fails} fail)");
    }
    PLATFORM.halt(0)
}
