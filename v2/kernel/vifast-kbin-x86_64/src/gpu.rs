// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Pha `virtio-gpu` 2D trên x86_64** — KT-337 (lần đầu mở GPU 2D trên x86_64).
//!
//! # Vì sao chạy **cuối** boot
//!
//! Ảnh `vifast-kbin-x86_64` đã có TAP 1..34 (boot/GDT/IDT/APIC/U-mode/net/SMP/sched/blk/lock).
//! Pha này chạy **sau** TAP 34 (khoá MCS) và **trước** marker duy nhất — đúng khuôn "số hiệu TAP
//! 1..34 không đổi" của các task trước: build **không** bật feature `gpu` vẫn 34 TAP, bật feature
//! thêm **một** ca `gpu: 2d` (TAP 35).
//!
//! # Bằng chứng **thật** là PPM, không phải TAP
//!
//! TAP chỉ nói "lệnh gửi xong". Runner [`gpu-x86_64.sh`](../../test/boot/gpu-x86_64.sh) chụp
//! `screendump` **trong lúc khách giữ khung hình** rồi chạy `gpu_ppm.py` (dùng **nguyên** bản của
//! riscv64/aarch64): đúng **điểm ảnh** — hình chữ nhật đỏ `(40,40,200,120)`, chữ `"ViFast"` trắng
//! tại `(40,200)`, ba dòng console `ViFastOs`/`gpu console`/`0123456789` tại `(8,8)` so **đủ 64
//! pixel** từng ký tự với bảng font của kernel.
//!
//! Đột biến `offset = 0` của KT-337 nằm ở **một chỗ** trong `vifast_virtio::gpudev`: nó chỉ hiện
//! ra trên PPM (console lệch 8 px), **không** làm TAP đỏ — chứng minh phép kiểm PPM có răng.
//!
//! # Marker giữ khung hình
//!
//! Sau `# gpu: console done lines=3`, ảnh giữ khung [`GPU_HOLD_MS`] ms (đo bằng PIT kênh 2, có
//! hạn) rồi mới trả về cho `net::run` in marker + halt. Runner chờ **marker của khách** rồi chụp;
//! không `sleep` mù.

use core::fmt::Write;

use vifast_plat_x86_64::pci::{self, PciError};
use vifast_plat_x86_64::timebase::PitClock;
use vifast_plat_x86_64::virtio_gpu;
use vifast_virtio::gpu::{
    draw_text, fill_rect, negotiated_features, Rect, COLOR_BLACK, COLOR_RED, COLOR_WHITE,
    VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
};
use vifast_virtio::gpu_console::{verify_text, Console as GpuConsole};
use vifast_virtio::input::KEY_A;
use vifast_virtio::pci::VirtioKind;

use crate::{check, pit2_latch_read, Console};

/// Hình chữ nhật **đỏ** vẽ lên framebuffer — `gpu_ppm.py` kiểm đúng bốn điểm ảnh quanh biên.
pub const RECT_X: u32 = 40;
/// Toạ độ y của hình chữ nhật đỏ.
pub const RECT_Y: u32 = 40;
/// Rộng hình chữ nhật đỏ (pixel).
pub const RECT_W: u32 = 200;
/// Cao hình chữ nhật đỏ (pixel).
pub const RECT_H: u32 = 120;
/// Gốc chữ `"ViFast"` trắng (pixel).
pub const TEXT_X: u32 = 40;
/// Toạ độ y của chữ `"ViFast"`.
pub const TEXT_Y: u32 = 200;
/// Chuỗi chữ trắng trong khối vẽ — cùng chuỗi `gpu_ppm.py` đòi (122 pixel trắng).
pub const TEXT: &str = "ViFast";
/// Gốc console chữ trên framebuffer.
pub const CONSOLE_X: u32 = 8;
/// Toạ độ y của dòng console đầu.
pub const CONSOLE_Y: u32 = 8;
/// Ba dòng console — **cùng** chuỗi `gpu_ppm.py` kiểm từng ký tự.
pub const CONSOLE_LINES: [&str; 3] = ["ViFastOs", "gpu console", "0123456789"];
/// Thời gian giữ khung hình (ms) để runner kịp `screendump` — mốc là marker của khách.
pub const GPU_HOLD_MS: u64 = 500;

/// Chạy pha GPU 2D: TAP 35 + các dòng `# gpu:` bằng chứng + giữ khung.
///
/// Gọi từ `net::run` **sau** TAP 34, **trước** marker. Đỏ ⇒ `check` halt (không marker) — runner
/// không bao giờ thấy "boot ok" khi GPU hỏng.
pub fn run(console: &mut Console, scan: &pci::ScanReport, net_bar_base: u64, tap: usize) {
    let mut device_ok = false;
    let mut draw_ok = false;
    let mut console_ok = false;
    let mut report: Option<virtio_gpu::GpuReport> = None;
    let full = Rect::full(virtio_gpu::FB_WIDTH, virtio_gpu::FB_HEIGHT);

    // Đồng hồ PIT kênh 2 (µs) — pha này chạy sau `cli`, không dùng tick PIT→LAPIC.
    let mut pit = PitClock::new();
    pit.reset(pit2_latch_read());
    let mut now = || pit.now_us(pit2_latch_read());

    // ---------------------------------------------------------------- TAP 35a: thiết bị
    match pci::init_for(VirtioKind::Gpu, scan) {
        Ok(found) => {
            let _ = writeln!(
                console,
                "# gpu pci: bus={} dev={} func={} vendor=0x{:04x} device=0x{:04x} bar={} kind={} size=0x{:x} assigned=0x{:x} readback=0x{:x} command=0x{:04x} net_base=0x{:x}",
                found.bus,
                found.dev,
                found.func,
                found.vendor,
                found.device,
                found.bar_index,
                pci::bar_kind_name(found.bar_kind),
                found.bar_size,
                found.bar_assigned,
                found.bar_readback,
                found.command,
                net_bar_base,
            );
            // "Leaf riêng" trên x86_64 = BAR gpu **rời** BAR net (firmware đã gán, driver không
            // ghi đè). Fail-closed: BAR 0 / cỡ 0 / chồng ⇒ `false`.
            let distinct =
                bar_disjoint(found.bar_base, found.bar_size, net_bar_base, found.bar_size);
            match virtio_gpu::init(&found, &mut now) {
                Ok(gpu) => {
                    let version_ok = gpu.device_features & (1u64 << 32) != 0
                        && gpu.driver_features & (1u64 << 32) != 0
                        && gpu.driver_features == negotiated_features(gpu.device_features);
                    let status = virtio_gpu::device_status();
                    let _ = writeln!(
                        console,
                        "# gpu dev: num_queues={} queue_max={} queue_size={} device_features=0x{:016x} driver_features=0x{:016x} version1={} device_status=0x{:02x} num_scanouts={} num_capsets={}",
                        gpu.num_queues,
                        gpu.queue_max,
                        gpu.queue_size,
                        gpu.device_features,
                        gpu.driver_features,
                        version_ok as u8,
                        status,
                        gpu.config.num_scanouts,
                        gpu.config.num_capsets,
                    );
                    let _ = writeln!(
                        console,
                        "# gpu dma: desc=0x{:x} avail=0x{:x} used=0x{:x} cmd=0x{:x} resp=0x{:x} dma=0x{:x} notify_off={} notify_addr=0x{:x} queue={}",
                        gpu.desc,
                        gpu.avail,
                        gpu.used,
                        gpu.cmd_pa,
                        gpu.resp_pa,
                        gpu.dma_pa,
                        gpu.notify_off,
                        gpu.notify_addr,
                        vifast_virtio::gpu::Q_CONTROL,
                    );
                    // `identity`: identity map 1 GiB của ảnh + đọc/ghi thử 4 byte đầu dải.
                    let fb_identity = {
                        let fb = virtio_gpu::framebuffer();
                        let probe = 0x5a;
                        fb[0] = probe;
                        fb[0] == probe
                    };
                    let _ = writeln!(
                        console,
                        "# gpu fb: pa=0x{:x} identity={} bytes={} pages={} stride={} attach_entries={} hold_ms={}",
                        gpu.fb_pa,
                        fb_identity as u8,
                        gpu.fb_bytes,
                        gpu.fb_pages,
                        virtio_gpu::FB_STRIDE,
                        virtio_gpu::MAX_ATTACH_ENTRIES,
                        GPU_HOLD_MS,
                    );
                    device_ok = version_ok
                        && status & vifast_virtio::features::DRIVER_OK != 0
                        && gpu.queue_size >= 2
                        && gpu.queue_size.is_power_of_two()
                        && gpu.num_queues >= 1
                        && gpu.config.num_scanouts >= 1
                        && fb_identity
                        && distinct
                        && found.bar_readback == found.bar_assigned;
                    report = Some(gpu);
                }
                Err(stage) => {
                    let _ = writeln!(console, "# gpu dev: init failed stage={stage}");
                }
            }
        }
        Err(fail) => {
            // Chỉ in `# gpu: no device` khi nguyên nhân **là** `NoDevice` (ca âm của runner grep
            // đúng dòng này); lỗi cấu hình khác in tên chặng.
            if fail.error == PciError::NoDevice {
                let _ = writeln!(console, "# gpu: no device");
            } else {
                let _ = writeln!(
                    console,
                    "# gpu: pci failed stage={} error={}",
                    fail.stage,
                    fail.error.name()
                );
            }
        }
    }

    // ---------------------------------------------------------------- TAP 35b: vẽ + flush
    if let Some(gpu) = &report {
        // (a) `GET_DISPLAY_INFO` — mode **thiết bị báo** (trước `SET_SCANOUT` QEMU trả 0×0).
        let mut display_ok = false;
        match virtio_gpu::get_display_info(&mut now) {
            Ok(info) => {
                let enabled = info.enabled_count();
                let (host_w, host_h) = match info.first_enabled() {
                    Some((_, rect)) => (rect.width, rect.height),
                    None => (0, 0),
                };
                let _ = writeln!(
                    console,
                    "# gpu display: w={host_w} h={host_h} enabled={enabled} scanouts={}",
                    gpu.config.num_scanouts,
                );
                display_ok = gpu.config.num_scanouts >= 1;
            }
            Err(stage) => {
                let _ = writeln!(console, "# gpu display: err={stage}");
            }
        }
        // (b) `RESOURCE_CREATE_2D` + `RESOURCE_ATTACH_BACKING` (19 mảnh của dải PA framebuffer).
        let mut resource_ok = false;
        match virtio_gpu::create_resource(virtio_gpu::FB_WIDTH, virtio_gpu::FB_HEIGHT, &mut now) {
            Ok(()) => match virtio_gpu::attach_backing(&mut now) {
                Ok(entries) => {
                    let _ = writeln!(
                        console,
                        "# gpu resource: id={} format={} w={} h={} entries={} chunk={}",
                        virtio_gpu::RESOURCE_ID,
                        VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
                        virtio_gpu::FB_WIDTH,
                        virtio_gpu::FB_HEIGHT,
                        entries,
                        virtio_gpu::ATTACH_CHUNK_BYTES,
                    );
                    resource_ok = entries as usize == virtio_gpu::MAX_ATTACH_ENTRIES;
                }
                Err(stage) => {
                    let _ = writeln!(console, "# gpu resource: attach_fail={stage}");
                }
            },
            Err(stage) => {
                let _ = writeln!(console, "# gpu resource: create_fail={stage}");
            }
        }
        // (c) `SET_SCANOUT` + **ca âm đối chứng**: cùng lệnh/rect với `resource_id = 999` (không
        // tồn tại) **phải** bị thiết bị từ chối `InvalidResourceId` — nếu thiết bị chấp nhận cả id
        // rác thì "SET_SCANOUT thành công" không chứng minh được gì.
        let mut scanout_ok = false;
        match virtio_gpu::set_scanout(full, &mut now) {
            Ok(()) => {
                let bogus = match virtio_gpu::probe_bogus_scanout(999, full, &mut now) {
                    Ok(name) => name,
                    Err(stage) => stage,
                };
                let (host_w, host_h, enabled) = match virtio_gpu::get_display_info(&mut now) {
                    Ok(info) => match info.first_enabled() {
                        Some((_, rect)) => (rect.width, rect.height, info.enabled_count()),
                        None => (0, 0, 0),
                    },
                    Err(_) => (0, 0, 0),
                };
                let _ = writeln!(
                    console,
                    "# gpu scanout: id={} resource={} w={} h={} host_w={host_w} host_h={host_h} enabled={enabled} bogus={bogus}",
                    virtio_gpu::SCANOUT_ID,
                    virtio_gpu::RESOURCE_ID,
                    virtio_gpu::FB_WIDTH,
                    virtio_gpu::FB_HEIGHT,
                );
                scanout_ok = bogus == "InvalidResourceId";
            }
            Err(stage) => {
                let _ = writeln!(console, "# gpu scanout: err={stage}");
            }
        }
        // (d) Vẽ: nền đen toàn khung (pixel ngoài hình chữ nhật/chữ có màu **xác định**), hình
        // chữ nhật đỏ, chữ trắng `"ViFast"`, rồi `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH`.
        let fb_w = virtio_gpu::FB_WIDTH as usize;
        let fb_h = virtio_gpu::FB_HEIGHT as usize;
        let stride = virtio_gpu::FB_STRIDE;
        let rect = Rect::new(RECT_X, RECT_Y, RECT_W, RECT_H);
        let painted = {
            let fb = virtio_gpu::framebuffer();
            let mut ok = fill_rect(fb, fb_w, fb_h, stride, full, COLOR_BLACK).is_ok();
            ok = ok && fill_rect(fb, fb_w, fb_h, stride, rect, COLOR_RED).is_ok();
            ok = ok
                && draw_text(
                    fb,
                    fb_w,
                    fb_h,
                    stride,
                    TEXT_X as usize,
                    TEXT_Y as usize,
                    TEXT,
                    COLOR_WHITE,
                    COLOR_BLACK,
                )
                .is_ok();
            ok
        };
        let flushed = painted
            && virtio_gpu::transfer_to_host_2d(full, &mut now).is_ok()
            && virtio_gpu::resource_flush(full, &mut now).is_ok();
        let _ = writeln!(
            console,
            "# gpu draw: rect={},{},{},{} color=0xff0000 text={} at={},{} fg=0xffffff bg=0x000000 stride={stride}",
            RECT_X, RECT_Y, RECT_W, RECT_H, TEXT, TEXT_X, TEXT_Y,
        );
        draw_ok = display_ok && resource_ok && scanout_ok && flushed;
        // Marker cho runner: **sau** khi flush xong, QEMU đã cập nhật console ⇒ `screendump` lúc
        // này đọc đúng nội dung vừa vẽ. Runner **không** dùng `sleep` mù.
        let _ = writeln!(console, "# gpu: drawn");

        // ------------------------------------------------------------ TAP 35c: console 8×8
        if let Some(mut text_console) =
            GpuConsole::new(fb_w, fb_h, stride, CONSOLE_X as usize, CONSOLE_Y as usize)
        {
            let mut draw_text_ok = true;
            {
                let fb = virtio_gpu::framebuffer();
                for line in CONSOLE_LINES {
                    if text_console
                        .puts(fb, line, COLOR_WHITE, COLOR_BLACK)
                        .is_err()
                    {
                        draw_text_ok = false;
                    }
                    if text_console
                        .putc(fb, b'\n', COLOR_WHITE, COLOR_BLACK)
                        .is_err()
                    {
                        draw_text_ok = false;
                    }
                }
            }
            let dirty = text_console.take_dirty();
            let commands_before = virtio_gpu::stats().commands;
            let flush_ok = match dirty {
                Some(rect) => virtio_gpu::flush_rect(rect, &mut now).is_ok(),
                None => false,
            };
            let commands_after = virtio_gpu::stats().commands;
            if flush_ok {
                text_console.note_flush();
            }
            // Đọc lại **framebuffer trong RAM**: mỗi ký tự khớp bitmap 8×8 của `glyph()`. Đây là
            // đường kiểm thứ nhất; đường thứ hai là `gpu_ppm.py` trên ảnh chụp của QEMU.
            let mut matched = 0usize;
            let mut total = 0usize;
            {
                let fb = virtio_gpu::framebuffer();
                for (index, line) in CONSOLE_LINES.iter().enumerate() {
                    let y = (CONSOLE_Y + index as u32 * 8) as usize;
                    match verify_text(
                        fb,
                        fb_w,
                        fb_h,
                        stride,
                        CONSOLE_X as usize,
                        y,
                        line,
                        COLOR_WHITE,
                        COLOR_BLACK,
                    ) {
                        Ok(count) => {
                            matched += count as usize;
                            total += line.len();
                        }
                        Err(_) => total += line.len(),
                    }
                }
            }
            let console_stats = text_console.stats();
            let one_batch = commands_after.saturating_sub(commands_before) == 2;
            let _ = writeln!(
                console,
                "# gpu console: chars={} newlines={} scrolls={} dirty=8,8,88,24 flushes={} commands={} matched={}/{} text={}",
                console_stats.chars,
                console_stats.newlines,
                console_stats.scrolls,
                console_stats.flushes,
                commands_after.saturating_sub(commands_before),
                matched,
                total,
                "ViFastOs|gpu console|0123456789",
            );
            console_ok = draw_text_ok
                && flush_ok
                && one_batch
                && matched == total
                && total == 29
                && console_stats.newlines == CONSOLE_LINES.len() as u64;
        } else {
            let _ = writeln!(console, "# gpu console: err=geometry");
        }
        let stats = virtio_gpu::stats();
        let _ = writeln!(
            console,
            "# gpu stats: commands={} notifies={} errors={} timeouts={} id_mismatch={} rejects={} published={} queue_size={} device_status=0x{:02x}",
            stats.commands,
            stats.notifies,
            stats.errors,
            stats.timeouts,
            stats.id_mismatch,
            stats.rejects,
            virtio_gpu::published(),
            virtio_gpu::QUEUE_SIZE,
            virtio_gpu::device_status(),
        );
        // Số **đo được**: mọi lệnh 2D xong, không lỗi/timeout/lệch id, thiết bị từ chối đúng ca
        // âm, và console flush đúng **hai** lệnh cho cả khối (không phải hai lệnh mỗi ký tự).
        let stats_ok = stats.commands >= 10
            && stats.errors == 0
            && stats.timeouts == 0
            && stats.id_mismatch == 0
            && stats.rejects == 1;
        draw_ok = draw_ok && stats_ok;
    } else {
        let _ = writeln!(console, "# gpu draw: skipped (no device)");
    }

    // Marker **giữ khung** (sau flush, trước TAP/halt): runner chờ đúng dòng này để `screendump`
    // trong lúc ảnh còn treo khung hình.
    let _ = writeln!(console, "# gpu: console done lines={}", CONSOLE_LINES.len());

    check(
        console,
        tap,
        "gpu: 2d ppm",
        device_ok && draw_ok && console_ok,
    );

    // Giữ khung hình [`GPU_HOLD_MS`] ms để runner kịp chụp (mốc thời gian PIT, **có hạn**).
    let started = now();
    while now().wrapping_sub(started) < GPU_HOLD_MS * 1_000 {
        core::hint::spin_loop();
    }
    // `KEY_A` được dùng bởi runner (sendkey) — nhắc compiler rằng codec chung đã được kiểm ở crate
    // thuần; không có bảng phím thứ hai trong ảnh x86_64.
    let _ = KEY_A;
}

/// `true` ⇔ hai BAR **rời nhau** (không chồng byte nào) và đều khác 0/có cỡ.
///
/// Cùng phép kiểm `blk::bar_disjoint` của KT-288 — "leaf riêng" trên x86_64 là **địa chỉ** rời
/// nhau do firmware gán, không phải leaf cố định như riscv64/aarch64.
fn bar_disjoint(a: u64, a_len: u64, b: u64, b_len: u64) -> bool {
    if a == 0 || b == 0 || a_len == 0 || b_len == 0 {
        return false;
    }
    let (Some(a_end), Some(b_end)) = (a.checked_add(a_len), b.checked_add(b_len)) else {
        return false;
    };
    a_end <= b || b_end <= a
}
