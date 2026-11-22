// SPDX-License-Identifier: MIT OR Apache-2.0
//! **virtio-gpu 2D + console trên framebuffer của ảnh aarch64** — KT-279 (K4' bước 3, D1 phần 4
//! trên aarch64).
//!
//! # Vì sao chạy ở `el0_done` (sau U-mode, sau virtio-blk, sau virtio-net, sau W^X)
//!
//! TAP phải **tuần tự**: KT-277 đã chiếm TAP 26/27 cho W^X/cache-DMA, nên GPU là 28/29/30 và phải
//! in **sau** `ok 27`. `el0_done` chạy ở EL1h với **cùng** `VSpace` đang hoạt động, nên MMIO tới
//! ECAM/BAR và truy cập framebuffer đi qua bảng trang đã map trong `high_entry` — không cần
//! `&mut VSpace` ở đây.
//!
//! # Ba phép đo (mỗi phép một dòng `#` + một TAP)
//!
//! 1. `# gpu pci:` + `# gpu dev:` + `# gpu dma:` + `# gpu fb:` — tìm `virtio-gpu` trên bus PCI
//!    (ECAM của KT-273), gán BAR trong leaf **riêng** của loại Gpu (leaf 4, không đụng leaf
//!    blk/net), khởi tạo driver (feature/queue/config), và chứng minh **framebuffer nằm ngoài
//!    ảnh** (PA `0x4200_0000`, 300 trang, map identity — `walk` đọc lại từ bảng trang).
//! 2. `# gpu display:` + `# gpu resource:` + `# gpu scanout:` + `# gpu draw:` — `GET_DISPLAY_INFO`,
//!    `RESOURCE_CREATE_2D` (640×480 `B8G8R8A8_UNORM`), `RESOURCE_ATTACH_BACKING` (19 mảnh 64 KiB),
//!    `SET_SCANOUT` + **ca âm đối chứng** `resource_id` rác phải bị từ chối `InvalidResourceId`,
//!    rồi vẽ nền đen + hình chữ nhật đỏ `(40,40,200,120)` + chữ `ViFast` trắng tại `(40,200)` và
//!    `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH`.
//! 3. `# gpu console:` — console chữ 8×8 in **ba** dòng `ViFastOs` / `gpu console` / `0123456789`
//!    tại gốc `(8,8)`, flush **một lô** (đúng **hai** lệnh cho cả khối — đo bằng delta bộ đếm
//!    `commands`), rồi **đọc lại framebuffer** và so **bitmap 8×8** từng ký tự (`matched=29/29`).
//!
//! # Không có thiết bị ⇒ **ĐỎ**, không im lặng
//!
//! `pci::init_for` trả `Err(NoDevice)` ⇒ in `# gpu: no device` (dòng **nguyên văn** mà runner
//! dùng cho ca âm "không có GPU") rồi cả ba TAP đỏ ⇒ `el0_done` dừng **trước** marker. Nhờ vậy
//! một lần boot thiếu `-device virtio-gpu-pci` **không** thể trông giống boot sạch.
//!
//! # Bằng chứng **thật** là điểm ảnh, không phải TAP
//!
//! TAP chỉ nói "lệnh gửi xong". Runner chụp màn hình bằng `screendump` (monitor HMP) và
//! `gpu_ppm.py` đòi **đúng màu tại đúng toạ độ** + **từng ký tự** của ba dòng console so bitmap
//! 8×8 đọc từ `v2/lib/vifast-virtio/src/gpu.rs` — không có bản font thứ hai chép tay.
//!
//! # Giữ khung hình cho runner kịp chụp
//!
//! QEMU tắt máy ngay khi khách `halt`, nên nếu ảnh thoát ngay sau marker thì runner **không bao
//! giờ** kịp `screendump` (đo được bên riscv64: monitor socket biến mất trước khi runner nối).
//! Ảnh giữ khung hình [`GPU_HOLD_MS`] ms bằng `CNTPCT_EL0` — **có hạn**, hết hạn thì tự `halt`,
//! không cần host kill, không treo. Đây là chi phí **thật** của soak (1.000 boot × 0,5 s) và được
//! ghi rõ trong `soak-aarch64.sh`.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::pci::{self, bar_kind_name, PciError, VirtioKind};
use vifast_plat_aarch64::virtio_gpu::{self, GpuReport};
use vifast_virtio::features::DRIVER_OK;
use vifast_virtio::gpu::{
    draw_text, fill_rect, Rect, COLOR_BLACK, COLOR_RED, COLOR_WHITE, GLYPH_H, GLYPH_W, Q_CONTROL,
    VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
};
use vifast_virtio::gpu_console::{verify_text, Console};

use crate::{tap, Console as SerialConsole};

/// Hình chữ nhật **đỏ** vẽ lên framebuffer — runner kiểm đúng bốn điểm ảnh quanh biên của nó
/// (cùng hằng với `gpu_ppm.py`: `RECT_X/Y/W/H`).
pub const RECT_X: u32 = 40;
/// Toạ độ y của hình chữ nhật đỏ.
pub const RECT_Y: u32 = 40;
/// Rộng hình chữ nhật đỏ (pixel).
pub const RECT_W: u32 = 200;
/// Cao hình chữ nhật đỏ (pixel).
pub const RECT_H: u32 = 120;
/// Gốc x của chữ "ViFast" (pixel).
pub const TEXT_X: u32 = 40;
/// Gốc y của chữ "ViFast" (pixel).
pub const TEXT_Y: u32 = 200;
/// Chuỗi vẽ lên framebuffer.
pub const TEXT: &str = "ViFast";
/// Gốc x của console (pixel) — **lệch 8 px** để pixel `(0,0)` vẫn là nền đen (ca PPM "gốc") và
/// khối chữ không chạm hình chữ nhật đỏ `(40,40,200,120)`.
pub const CONSOLE_X: usize = 8;
/// Gốc y của console (pixel) — cùng lý do [`CONSOLE_X`].
pub const CONSOLE_Y: usize = 8;
/// Ba dòng console in lên framebuffer. Runner (`run-aarch64.sh`) và `gpu_ppm.py` đối chiếu **từng
/// ký tự** với bảng font trong `vifast_virtio::gpu::FONT_8X8` — đổi một bên phải đổi cả ba.
pub const CONSOLE_LINES: [&str; 3] = ["ViFastOs", "gpu console", "0123456789"];
/// Số dòng của phép kiểm **cuộn**: console 640×480 gốc `(8,8)` có `rows = 59`, nên 60 dòng ⇒
/// **đúng một** lần cuộn; dòng 0 phải là ký tự của dòng thứ hai (`'1'`), dòng cuối (`'9'`) của
/// dòng 60. Chạy **trước** khung vẽ của TAP 29 (khung tô nền đen xoá sạch vùng này) nên PPM không
/// đổi — đây là phép kiểm **bộ nhớ framebuffer**, không phải phép kiểm thiết bị.
pub const SCROLL_PROBE_LINES: usize = 60;
/// Thời gian **giữ khung hình** sau `# gpu: console done lines=3` (mili-giây) — xem tài liệu đầu
/// tệp. 500 ms là **5×** chu kỳ poll 100 ms của runner.
pub const GPU_HOLD_MS: u64 = 500;

/// PA mà `VSpace` **đọc lại** cho VA [`virtio_gpu::FB_PHYS_BASE`] — do `high_entry` công bố sau
/// khi map cửa sổ framebuffer (trước `activate`). `0` ⇔ chưa map.
static FB_WALK_PA: AtomicU64 = AtomicU64::new(0);

/// Công bố PA mà bảng trang trả cho VA của framebuffer — gọi **một** lần trong `high_entry`.
pub fn publish_fb_walk(pa: u64) {
    FB_WALK_PA.store(pa, Ordering::Relaxed);
}

/// PA đã đọc lại từ bảng trang (0 ⇔ chưa map).
pub fn fb_walk_pa() -> u64 {
    FB_WALK_PA.load(Ordering::Relaxed)
}

/// `true` ⇔ framebuffer map **identity** (`VA == PA`) — điều kiện để thiết bị DMA đọc đúng dải
/// PA mà driver ghi vào `RESOURCE_ATTACH_BACKING`.
fn fb_identity() -> bool {
    fb_walk_pa() == virtio_gpu::FB_PHYS_BASE
}

/// Giữ khung hình [`GPU_HOLD_MS`] ms bằng `CNTPCT_EL0` (có hạn, không treo).
///
/// `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ **không** giữ: thà mất ảnh chụp (runner đỏ, thấy ngay) còn
/// hơn treo máy vì một vòng lặp không có mốc thời gian.
fn hold_frame() {
    let freq = asm::read_cntfrq_el0();
    if freq == 0 {
        return;
    }
    let ticks = freq / 1000 * GPU_HOLD_MS;
    let until = asm::read_cntpct_el0().wrapping_add(ticks);
    while asm::read_cntpct_el0() < until {
        core::hint::spin_loop();
    }
}

/// Chạy ba phép đo GPU, in TAP 28/29/30. Trả `true` ⇔ **cả ba** xanh.
///
/// In **đủ** ba dòng TAP kể cả khi phép trước đỏ: log phải cho biết **phép nào** hỏng, không dừng
/// ở phép đầu (một ảnh không có GPU vẫn phải chứng minh PCI/ECAM chạy).
pub fn run(console: &mut SerialConsole) -> bool {
    let mut all_ok = true;

    // ---------------------------------------------------------------- TAP 28: thiết bị
    let mut report: Option<GpuReport> = None;
    match pci::init_for(VirtioKind::Gpu) {
        Ok(found) => {
            let _ = writeln!(
                console,
                "# gpu pci: bus={} dev={} func={} vendor=0x{:04x} device=0x{:04x} bar={} kind={} size=0x{:x} assigned=0x{:x} readback=0x{:x} command=0x{:04x} caps=1 other_bars={} reserved={}",
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
                found.command,
                found.other_bars,
                found.reserved_ranges,
            );
            // **KT-338a**: trang DMA cấp động từ arena ngoài ảnh, map **identity** (`VA == PA`)
            // nên `dma_page_gpu_addr()` đã là PA.
            let dma_pa = asm::dma_page_gpu_addr();
            match virtio_gpu::init(&found, dma_pa) {
                Ok(gpu) => {
                    let version_ok = gpu.device_features & (1u64 << 32) != 0
                        && gpu.driver_features & (1u64 << 32) != 0;
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
                        Q_CONTROL,
                    );
                    let _ = writeln!(
                        console,
                        "# gpu fb: pa=0x{:x} walk=0x{:x} identity={} bytes={} pages={} stride={} attach_entries={}",
                        gpu.fb_pa,
                        fb_walk_pa(),
                        fb_identity() as u8,
                        gpu.fb_bytes,
                        gpu.fb_pages,
                        virtio_gpu::FB_STRIDE,
                        virtio_gpu::MAX_ATTACH_ENTRIES,
                    );
                    report = Some(gpu);
                }
                Err(stage) => {
                    let _ = writeln!(console, "# gpu dev: init failed stage={stage}");
                }
            }
        }
        Err(fail) => {
            // Không có thiết bị **khác** lỗi cấu hình: ca âm "không có GPU" của runner grep đúng
            // dòng `# gpu: no device`, nên chỉ in nó khi nguyên nhân **là** `NoDevice`.
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
    // Thiết bị chỉ được tin khi: feature VERSION_1 hai chiều, `device_status` còn `DRIVER_OK`,
    // queue dùng được, thiết bị báo **có** scanout, và framebuffer map **identity** (nếu không,
    // thiết bị DMA đọc một dải PA khác với dải driver vẽ ⇒ PPM trống).
    let device_ok = match &report {
        Some(gpu) => {
            gpu.device_features & (1u64 << 32) != 0
                && gpu.driver_features & (1u64 << 32) != 0
                && virtio_gpu::device_status() & DRIVER_OK != 0
                && gpu.queue_size >= 2
                && gpu.queue_size.is_power_of_two()
                && gpu.num_queues >= 1
                && gpu.config.num_scanouts >= 1
                && fb_identity()
        }
        None => false,
    };
    if !tap(console, 28, "gpu: device", device_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 29: vẽ + flush
    let mut draw_ok = false;
    if let Some(gpu) = &report {
        let full = Rect::full(virtio_gpu::FB_WIDTH, virtio_gpu::FB_HEIGHT);
        // (a) `GET_DISPLAY_INFO` — mode **thiết bị báo** (trước `SET_SCANOUT` QEMU trả 0×0).
        let mut display_ok = false;
        match virtio_gpu::get_display_info() {
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
        // (b) `RESOURCE_CREATE_2D` + `RESOURCE_ATTACH_BACKING`.
        let mut resource_ok = false;
        match virtio_gpu::create_resource(virtio_gpu::FB_WIDTH, virtio_gpu::FB_HEIGHT) {
            Ok(()) => match virtio_gpu::attach_backing() {
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
        // (c) `SET_SCANOUT` + **ca âm đối chứng**: cùng lệnh, cùng rect, `resource_id` **không tồn
        // tại** phải bị thiết bị từ chối `InvalidResourceId`; nếu thiết bị chấp nhận cả id rác thì
        // "SET_SCANOUT thành công" không chứng minh được gì.
        let mut scanout_ok = false;
        match virtio_gpu::set_scanout(full) {
            Ok(()) => {
                let bogus = match virtio_gpu::probe_bogus_scanout(999, full) {
                    Ok(name) => name,
                    Err(stage) => stage,
                };
                let (host_w, host_h, enabled) = match virtio_gpu::get_display_info() {
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
        // (d) Vẽ: nền đen toàn khung ⇒ mọi pixel ngoài hình chữ nhật/chữ có màu **xác định** để
        // runner kiểm được ca "không vẽ gì" (nếu driver không ghi, PPM vẫn là nền console cũ).
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
            && virtio_gpu::transfer_to_host_2d(full).is_ok()
            && virtio_gpu::resource_flush(full).is_ok();
        let _ = writeln!(
            console,
            "# gpu draw: rect={},{},{},{} color=0xff0000 text={} at={},{} fg=0xffffff bg=0x000000 stride={stride}",
            RECT_X, RECT_Y, RECT_W, RECT_H, TEXT, TEXT_X, TEXT_Y,
        );
        draw_ok = display_ok && resource_ok && scanout_ok && flushed;
        // Marker cho runner: **sau** khi flush xong, QEMU đã cập nhật console ⇒ `screendump` lúc
        // này đọc đúng nội dung vừa vẽ. Runner **không** dùng `sleep` mù.
        let _ = writeln!(console, "# gpu: drawn");
    } else {
        let _ = writeln!(console, "# gpu draw: skipped (no device)");
    }
    if !tap(console, 29, "gpu: draw", draw_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 30: console
    let mut console_ok = false;
    if report.is_some() {
        let fb_w = virtio_gpu::FB_WIDTH as usize;
        let fb_h = virtio_gpu::FB_HEIGHT as usize;
        let stride = virtio_gpu::FB_STRIDE;
        // **Cuộn** trên framebuffer thật: 60 dòng > `rows = 59` ⇒ đúng một lần cuộn. Chạy **trước**
        // khung vẽ của TAP 29? Không — TAP 29 đã vẽ rồi; khung vẽ tô nền đen **toàn khung** nên
        // phép cuộn phải chạy **sau** nó và **trước** lô console của TAP 30, rồi TAP 30 vẽ lại ba
        // dòng đè lên vùng vừa cuộn. Vùng cuộn chỉ là cột console nên hình chữ nhật đỏ không đổi.
        let mut scroll_ok = false;
        if let Some(mut probe) = Console::new(fb_w, fb_h, stride, CONSOLE_X, CONSOLE_Y) {
            let mut probe_ok = true;
            {
                let fb = virtio_gpu::framebuffer();
                let mut index = 0;
                while index < SCROLL_PROBE_LINES {
                    let ch = b'0' + (index % 10) as u8;
                    if probe.putc(fb, ch, COLOR_WHITE, COLOR_BLACK).is_err() {
                        probe_ok = false;
                    }
                    // **Không** xuống dòng sau dòng **cuối**: nếu có, dòng cuối ở hàng `rows - 1`
                    // sẽ cuộn thêm lần nữa (đo được bên riscv64: `scrolls=2`, dòng 0 thành `'2'`).
                    if index + 1 < SCROLL_PROBE_LINES
                        && probe.putc(fb, b'\n', COLOR_WHITE, COLOR_BLACK).is_err()
                    {
                        probe_ok = false;
                    }
                    index += 1;
                }
            }
            let probe_stats = probe.stats();
            let last_y = CONSOLE_Y + (probe.rows() - 1) * GLYPH_H;
            let (row0, row_last) = {
                let fb = virtio_gpu::framebuffer();
                let row0 = verify_text(
                    fb,
                    fb_w,
                    fb_h,
                    stride,
                    CONSOLE_X,
                    CONSOLE_Y,
                    "1",
                    COLOR_WHITE,
                    COLOR_BLACK,
                )
                .unwrap_or(0);
                let row_last = verify_text(
                    fb,
                    fb_w,
                    fb_h,
                    stride,
                    CONSOLE_X,
                    last_y,
                    "9",
                    COLOR_WHITE,
                    COLOR_BLACK,
                )
                .unwrap_or(0);
                (row0, row_last)
            };
            scroll_ok = probe_ok
                && probe_stats.chars == SCROLL_PROBE_LINES as u64
                && probe_stats.scrolls == 1
                && row0 == 1
                && row_last == 1;
            let _ = writeln!(
                console,
                "# gpu console scroll: lines={} scrolls={} row0={row0} row_last={row_last} ok={}",
                probe_stats.chars, probe_stats.scrolls, scroll_ok as u8,
            );
        } else {
            let _ = writeln!(console, "# gpu console scroll: err=geometry");
        }

        // Ba dòng chữ tại gốc `(8,8)`: console **tích** vùng đã đổi rồi flush **một lô** — đúng
        // hai lệnh cho cả khối, không phải hai lệnh mỗi ký tự. Delta bộ đếm `commands` là số **đo
        // được** (D2-102), không phải lời hứa.
        let mut text_console = match Console::new(fb_w, fb_h, stride, CONSOLE_X, CONSOLE_Y) {
            Some(text_console) => text_console,
            None => {
                let _ = writeln!(console, "# gpu console: err=geometry");
                let _ = tap(console, 30, "gpu: console", false);
                let stats = virtio_gpu::stats();
                let _ = writeln!(
                    console,
                    "# gpu stats: commands={} notifies={} errors={} timeouts={} id_mismatch={} rejects={} fb_clean_lines={} fb_clean_bytes={} fb_clean_ops={} published={} queue_size={} device_status=0x{:02x}",
                    stats.commands,
                    stats.notifies,
                    stats.errors,
                    stats.timeouts,
                    stats.id_mismatch,
                    stats.rejects,
                    stats.fb_clean_lines,
                    stats.fb_clean_bytes,
                    stats.fb_clean_ops,
                    virtio_gpu::published(),
                    virtio_gpu::QUEUE_SIZE,
                    virtio_gpu::device_status(),
                );
                return false;
            }
        };
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
        let flushed = match dirty {
            Some(rect) => virtio_gpu::flush_rect(rect).is_ok(),
            None => false,
        };
        let commands_after = virtio_gpu::stats().commands;
        if flushed {
            text_console.note_flush();
        }
        // Đọc lại **framebuffer trong RAM**: mỗi ký tự phải khớp bitmap 8×8 của `glyph()`. Đây là
        // đường kiểm thứ nhất; đường thứ hai là `gpu_ppm.py` trên ảnh chụp của QEMU.
        let mut matched = 0usize;
        let mut total = 0usize;
        {
            let fb = virtio_gpu::framebuffer();
            for (index, line) in CONSOLE_LINES.iter().enumerate() {
                let y = CONSOLE_Y + index * GLYPH_H;
                match verify_text(
                    fb,
                    fb_w,
                    fb_h,
                    stride,
                    CONSOLE_X,
                    y,
                    line,
                    COLOR_WHITE,
                    COLOR_BLACK,
                ) {
                    Ok(count) => {
                        matched += count;
                        total += line.len();
                    }
                    Err(_) => draw_text_ok = false,
                }
            }
        }
        let console_stats = text_console.stats();
        let cols = text_console.cols();
        let rows = text_console.rows();
        let commands = commands_after.wrapping_sub(commands_before);
        let mut widest = 0usize;
        for line in CONSOLE_LINES {
            if line.len() > widest {
                widest = line.len();
            }
        }
        // Hộp bao **kỳ vọng** suy từ chính ba dòng (không hardcode số): rộng nhất × 8, cao = số
        // dòng × 8.
        let expect_dirty = Rect::new(
            CONSOLE_X as u32,
            CONSOLE_Y as u32,
            (widest * GLYPH_W) as u32,
            (CONSOLE_LINES.len() * GLYPH_H) as u32,
        );
        console_ok = draw_text_ok
            && scroll_ok
            && flushed
            && dirty == Some(expect_dirty)
            && console_stats.chars as usize == total
            && console_stats.newlines as usize == CONSOLE_LINES.len()
            && console_stats.scrolls == 0
            && console_stats.flushes == 1
            && commands == 2
            && matched == total;
        {
            let _ = write!(
                console,
                "# gpu console: origin={},{} cols={} rows={} lines={} chars={} newlines={} scrolls={} dirty={},{},{},{} flushes={} commands={} matched={}/{} text=",
                CONSOLE_X,
                CONSOLE_Y,
                cols,
                rows,
                CONSOLE_LINES.len(),
                console_stats.chars,
                console_stats.newlines,
                console_stats.scrolls,
                expect_dirty.x,
                expect_dirty.y,
                expect_dirty.width,
                expect_dirty.height,
                console_stats.flushes,
                commands,
                matched,
                total,
            );
            for (index, line) in CONSOLE_LINES.iter().enumerate() {
                if index > 0 {
                    let _ = console.write_str("|");
                }
                let _ = console.write_str(line);
            }
            let _ = writeln!(console);
        }
    } else {
        let _ = writeln!(console, "# gpu console: skipped (no device)");
    }
    if !tap(console, 30, "gpu: console", console_ok) {
        all_ok = false;
    }
    // Marker cho runner: **sau** khi flush lô console xong ⇒ `screendump` lúc này có đủ ba dòng.
    // In **kể cả** khi TAP 30 đỏ: ảnh chụp là bằng chứng để chẩn đoán, không phải phần thưởng.
    let _ = writeln!(console, "# gpu: console done lines=3");

    // ---------------------------------------------------------------- bộ đếm + giữ khung
    let stats = virtio_gpu::stats();
    let _ = writeln!(
        console,
        "# gpu stats: commands={} notifies={} errors={} timeouts={} id_mismatch={} rejects={} fb_clean_lines={} fb_clean_bytes={} fb_clean_ops={} published={} queue_size={} device_status=0x{:02x}",
        stats.commands,
        stats.notifies,
        stats.errors,
        stats.timeouts,
        stats.id_mismatch,
        stats.rejects,
        stats.fb_clean_lines,
        stats.fb_clean_bytes,
        stats.fb_clean_ops,
        virtio_gpu::published(),
        virtio_gpu::QUEUE_SIZE,
        virtio_gpu::device_status(),
    );
    // Giữ khung hình cho runner kịp `screendump` — log đã **đủ** trước khi giữ.
    hold_frame();

    all_ok
}
