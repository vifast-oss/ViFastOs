// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test virtio-gpu 2D** riscv64 (KT-263, D1 phần 4) — bin `vifast-kbin-ktest-gpu`, feature
//! `ktest,gpu`, nạp qua OpenSBI **kèm** `-device virtio-gpu-pci` + monitor để `screendump`.
//!
//! # Vì sao ảnh riêng
//!
//! (a) Ảnh `ktest` đã ở **92,6 %** trần `.data+.bss` (D2-109) — driver + trang DMA 4 KiB đẩy nó
//! sát trần; (b) ảnh này cần **thiết bị thật** (`-device virtio-gpu-pci`) **và** điều khiển QEMU
//! từ ngoài (monitor `screendump`) trong lúc khách đang chạy — tách ra thì 21 boot matrix cũ
//! **không đổi**; (c) boot kết thúc bằng **một** dòng `PANIC:` chủ đích (cùng quy ước ảnh
//! guard/user/proc/vblk/con/net/input).
//!
//! # TAP (5 test; tên khớp `v2/test/boot/gpu.sh`)
//!
//! 1. `gpu device` — quét ECAM tìm **vendor/device đọc từ thiết bị** (`0x1af4:0x1050`), đo cỡ BAR
//!    bằng mask, **gán** BAR (firmware không enumerate), đọc lại BAR + `command`, đọc bốn
//!    capability; ECAM/BAR được map identity (walk phần mềm); queue `controlq` bật; đọc
//!    `virtio_gpu_config` (`num_scanouts` **từ thiết bị**).
//! 2. `gpu display info` — `GET_DISPLAY_INFO`: in `w×h` **thiết bị báo** + số scanout đang bật.
//!    Trước `SET_SCANOUT` đầu tiên QEMU trả `0×0 enabled=0` — đó là số đo thật, không phải lỗi.
//! 3. `gpu resource` — `RESOURCE_CREATE_2D` (640×480, `B8G8R8A8_UNORM`) +
//!    `RESOURCE_ATTACH_BACKING` (19 mảnh 64 KiB của dải PA 300 frame).
//! 4. `gpu scanout` — `SET_SCANOUT` rồi `GET_DISPLAY_INFO` **lại**: thiết bị phải báo đúng
//!    `640×480 enabled=1` ⇒ chứng minh scanout đã ăn (không chỉ "lệnh gửi xong").
//! 5. `gpu draw` — tô nền đen, hình chữ nhật **đỏ** `(40,40,200,120)`, chữ **"ViFast" trắng** tại
//!    `(40,200)` bằng font 8×8, rồi `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH`. In `# gpu: drawn` để
//!    runner biết lúc `screendump` — bằng chứng **thật** là điểm ảnh trong PPM, không phải TAP này.
//!
//! Mọi dòng `# gpu:` in từ **biến đo** (D2-102); giá trị suy ra in kèm công thức/đầu vào
//! (D2-106). Không `unsafe` trong tệp này (MMIO nằm ở `kernel/vifast-plat-riscv64`).

#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::timer::TIMEBASE_HZ;
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::virtio_gpu;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::gpu::{
    draw_text, fill_rect, Rect, COLOR_BLACK, COLOR_RED, COLOR_WHITE, Q_CONTROL,
};
use vifast_virtio::pci::{COMMAND_BUS_MASTER, COMMAND_MEMORY};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Hình chữ nhật **đỏ** vẽ lên framebuffer — runner kiểm đúng bốn điểm ảnh quanh biên của nó.
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
/// Thời gian **giữ khung hình** sau khi in `# gpu: drawn` (tick timebase) — 5 s ảo.
///
/// QEMU tắt máy ngay khi khách `halt`, nên nếu ảnh thoát ngay sau marker thì runner **không bao
/// giờ** kịp `screendump` (đo được: monitor socket biến mất trước khi runner nối). Giữ **có hạn**
/// để runner chụp xong; hết hạn thì ảnh tự `halt` — không cần host kill, không treo.
pub const HOLD_TICKS: u64 = 5 * TIMEBASE_HZ;

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

/// `true` ⇔ alias physmap của `pa` trỏ **đúng** `pa` (leaf 2 MiB RW NX của `mmu::init`).
///
/// Khác [`identity`]: `walk.phys` là **đáy leaf** (không phải PA đã dịch), nên phải so
/// `walk.phys == pa & !(leaf-1)` **và** offset trong leaf của VA bằng offset của PA. So `VA - PA`
/// như đường identity là **sai** ở đây (VA physmap cách PA cả một base 1 GiB).
fn physmap_ok(pa: u64) -> bool {
    let va = mmu::PHYSMAP_BASE + pa;
    let walk = mmu::walk_sw(va);
    if !walk.valid || !walk.leaf {
        return false;
    }
    let leaf_bytes = 4096u64 << (9 * walk.level as u32);
    walk.phys == (pa & !(leaf_bytes - 1)) && (va & (leaf_bytes - 1)) == (pa & (leaf_bytes - 1))
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..5\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# gpu image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} tables={} used={} \
             const:wait_ms={} const:fb={}x{} const:fb_pages={} const:attach_entries={}",
            PLATFORM.hart_id(),
            stvec,
            (stvec & 0b11 == 0) as u8,
            mmu_report.satp,
            mmu_report.satp >> 60,
            mmu_report.tables,
            mmu_report.used,
            virtio_gpu::GPU_WAIT_TICKS * 1000 / TIMEBASE_HZ,
            virtio_gpu::FB_WIDTH,
            virtio_gpu::FB_HEIGHT,
            virtio_gpu::FB_PAGES,
            virtio_gpu::MAX_ATTACH_ENTRIES
        );
    }
    let mut fails = 0usize;

    // ------------------------------------------------ 1) PCI: tìm + gán BAR + capability
    let pci_report = match pci::init_for(vifast_virtio::pci::VirtioKind::Gpu) {
        Ok(report) => {
            let ecam_ok = identity(report.ecam_addr);
            let bar_ok = identity(report.bar_assigned);
            let ok = report.vendor == 0x1af4
                && (report.device == 0x1050 || report.device == 0x1010)
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
                    "# gpu pci: functions={} bus={} dev={} func={} vendor=0x{:x} device=0x{:x} \
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
                    "# gpu cap: common=0x{:x} notify=0x{:x} device=0x{:x} isr=0x{:x} mul={} \
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
            // **Chưa** in TAP 1 ở đây: TAP 1 gộp PCI **và** init (feature/queue/config/DMA) —
            // "gpu device" chỉ xanh khi thiết bị đã ở trạng thái dùng được, không phải chỉ "thấy
            // trên bus". PCI hỏng ⇒ fail-closed ngay (không in `ok`).
            if !ok {
                put(
                    false,
                    1,
                    "gpu device",
                    &mut fails,
                    "vendor/device/BAR/capability không đúng",
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            report
        }
        Err(fail) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# gpu pci: err={} stage={}",
                fail.error.name(),
                fail.stage
            );
            // Dòng **cố định** cho runner + ca âm (D2-203): không có `-device virtio-gpu-pci` thì
            // ảnh phải in đúng dòng này và TAP **đỏ**, không PASS giả.
            let _ = writeln!(w, "# gpu: no device");
            put(
                false,
                1,
                "gpu device",
                &mut fails,
                "không tìm/khởi tạo được PCI",
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };

    // ------------------------------------------------ 1b) feature + queue + config (vẫn TAP 1)
    let gpu = match virtio_gpu::init(&pci_report) {
        Ok(report) => {
            let dma_ok = identity(report.desc)
                && identity(report.avail)
                && identity(report.used)
                && identity(report.cmd_pa)
                && identity(report.resp_pa);
            let fb_ok = physmap_ok(report.fb_pa)
                && report.fb_bytes == virtio_gpu::FB_BYTES
                && report.fb_pages == virtio_gpu::FB_PAGES;
            let version_ok = report.device_features & (1u64 << 32) != 0
                && report.driver_features & (1u64 << 32) != 0;
            let status = virtio_gpu::device_status();
            let ok = version_ok
                && report.driver_features
                    == vifast_virtio::gpu::negotiated_features(report.device_features)
                && status & 0x0f == 0x0f
                && report.queue_size >= 2
                && report.queue_size.is_power_of_two()
                && report.num_queues >= 1
                && report.config.num_scanouts >= 1
                && dma_ok
                && fb_ok;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# gpu dev: num_queues={} queue_max={} queue_size={} device_features=0x{:x} \
                     driver_features=0x{:x} version1={} device_status=0x{:x} num_scanouts={} \
                     num_capsets={}",
                    report.num_queues,
                    report.queue_max,
                    report.queue_size,
                    report.device_features,
                    report.driver_features,
                    version_ok as u8,
                    status,
                    report.config.num_scanouts,
                    report.config.num_capsets
                );
                let _ = writeln!(
                    w,
                    "# gpu dma: desc=0x{:x} avail=0x{:x} used=0x{:x} cmd=0x{:x} resp=0x{:x} \
                     identity={} notify_off={} notify_addr=0x{:x} queue={}",
                    report.desc,
                    report.avail,
                    report.used,
                    report.cmd_pa,
                    report.resp_pa,
                    dma_ok as u8,
                    report.notify_off,
                    report.notify_addr,
                    Q_CONTROL
                );
                let _ = writeln!(
                    w,
                    "# gpu fb: pa=0x{:x} bytes={} pages={} physmap={} stride={}",
                    report.fb_pa,
                    report.fb_bytes,
                    report.fb_pages,
                    fb_ok as u8,
                    virtio_gpu::FB_WIDTH * 4
                );
            }
            // TAP 1 gộp **cả** PCI (đã xanh ở trên) và init (feature/queue/config/DMA).
            put(
                ok,
                1,
                "gpu device",
                &mut fails,
                "feature/queue/config/DMA không đúng",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            report
        }
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# gpu dev: init_fail={stage}");
            put(false, 1, "gpu device", &mut fails, "init thiết bị thất bại");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let _ = gpu;

    // ------------------------------------------------ 2) GET_DISPLAY_INFO (mode ban đầu)
    let initial = match virtio_gpu::get_display_info() {
        Ok(info) => {
            let enabled = info.enabled_count();
            let first = info.first_enabled();
            let (w, h) = match first {
                Some((_, rect)) => (rect.width, rect.height),
                None => (0, 0),
            };
            {
                let mut writer = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    writer,
                    "# gpu display: w={} h={} enabled={} scanouts={}",
                    w, h, enabled, gpu.config.num_scanouts
                );
            }
            // Hợp lệ ⇔ response đúng kiểu `RESP_OK_DISPLAY_INFO` (đã kiểm trong `parse`) và thiết
            // bị báo **có** ít nhất một scanout. `0×0` trước `SET_SCANOUT` là số đo thật của QEMU.
            let ok = gpu.config.num_scanouts >= 1 && enabled <= vifast_virtio::gpu::MAX_SCANOUTS;
            put(
                ok,
                2,
                "gpu display info",
                &mut fails,
                "GET_DISPLAY_INFO không trả bảng mode hợp lệ",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            info
        }
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# gpu display: err={stage}");
            put(
                false,
                2,
                "gpu display info",
                &mut fails,
                "GET_DISPLAY_INFO thất bại",
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let _ = initial;

    // ------------------------------------------------ 3) RESOURCE_CREATE_2D + ATTACH_BACKING
    let entries = match virtio_gpu::create_resource(virtio_gpu::FB_WIDTH, virtio_gpu::FB_HEIGHT) {
        Ok(()) => match virtio_gpu::attach_backing() {
            Ok(entries) => {
                let ok = entries as usize == virtio_gpu::MAX_ATTACH_ENTRIES;
                {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(
                        w,
                        "# gpu resource: id={} format={} w={} h={} entries={} chunk={}",
                        virtio_gpu::RESOURCE_ID,
                        vifast_virtio::gpu::VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
                        virtio_gpu::FB_WIDTH,
                        virtio_gpu::FB_HEIGHT,
                        entries,
                        virtio_gpu::ATTACH_CHUNK_BYTES
                    );
                }
                put(
                    ok,
                    3,
                    "gpu resource",
                    &mut fails,
                    "RESOURCE_CREATE_2D/ATTACH_BACKING không đúng",
                );
                if !ok {
                    PLATFORM.put_str("# ktest halt\n");
                    PLATFORM.halt(1)
                }
                entries
            }
            Err(stage) => {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(w, "# gpu resource: attach_fail={stage}");
                put(
                    false,
                    3,
                    "gpu resource",
                    &mut fails,
                    "ATTACH_BACKING thất bại",
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        },
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# gpu resource: create_fail={stage}");
            put(
                false,
                3,
                "gpu resource",
                &mut fails,
                "RESOURCE_CREATE_2D thất bại",
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let _ = entries;

    // ------------------------------------------------ 4) SET_SCANOUT + ca âm resource rác
    //
    // `GET_DISPLAY_INFO` của QEMU trả mode **của host** (`req_state`), không phải scanout của
    // khách — nên nó **không** đổi khi `SET_SCANOUT` (đo được: 1280×800 trước và sau). Vì vậy
    // phép đo "scanout đã ăn" ở đây là **ca âm đối chứng**: cùng lệnh, cùng rect, `resource_id`
    // **không tồn tại** phải bị thiết bị từ chối `InvalidResourceId`; nếu thiết bị chấp nhận cả id
    // rác thì "SET_SCANOUT thành công" không chứng minh được gì. Bằng chứng **thật** rằng đã vẽ là
    // điểm ảnh trong PPM của runner.
    let full = Rect::full(virtio_gpu::FB_WIDTH, virtio_gpu::FB_HEIGHT);
    match virtio_gpu::set_scanout(full) {
        Ok(()) => {
            let bogus_name = match virtio_gpu::probe_bogus_scanout(999, full) {
                Ok(name) => name,
                Err(stage) => stage,
            };
            // Đọc lại mode host để in bằng chứng (không dùng làm điều kiện xanh).
            let (host_w, host_h, enabled) = match virtio_gpu::get_display_info() {
                Ok(info) => match info.first_enabled() {
                    Some((_, rect)) => (rect.width, rect.height, info.enabled_count()),
                    None => (0, 0, 0),
                },
                Err(_) => (0, 0, 0),
            };
            {
                let mut writer = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    writer,
                    "# gpu scanout: id={} resource={} w={} h={} host_w={} host_h={} enabled={} \
                     bogus={}",
                    virtio_gpu::SCANOUT_ID,
                    virtio_gpu::RESOURCE_ID,
                    virtio_gpu::FB_WIDTH,
                    virtio_gpu::FB_HEIGHT,
                    host_w,
                    host_h,
                    enabled,
                    bogus_name
                );
            }
            let ok = bogus_name == "InvalidResourceId" && enabled >= 1;
            put(
                ok,
                4,
                "gpu scanout",
                &mut fails,
                "SET_SCANOUT không phân biệt được resource thật với resource rác",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        }
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# gpu scanout: err={stage}");
            put(false, 4, "gpu scanout", &mut fails, "SET_SCANOUT thất bại");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    }

    // ------------------------------------------------ 5) vẽ + TRANSFER + FLUSH
    let (fb_w, fb_h) = (
        virtio_gpu::FB_WIDTH as usize,
        virtio_gpu::FB_HEIGHT as usize,
    );
    let stride = fb_w * vifast_virtio::gpu::BYTES_PER_PIXEL;
    let rect = Rect::new(RECT_X, RECT_Y, RECT_W, RECT_H);
    let drawn = {
        let fb = virtio_gpu::framebuffer();
        // Nền đen toàn khung ⇒ mọi pixel ngoài hình chữ nhật/chữ có màu **xác định** để runner
        // kiểm được ca "không vẽ gì" (nếu driver không ghi, PPM vẫn là nền console cũ, không đen).
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
    let flushed = drawn
        && virtio_gpu::transfer_to_host_2d(full).is_ok()
        && virtio_gpu::resource_flush(full).is_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# gpu draw: rect={},{},{},{} color=0xff0000 text={} at={},{} fg=0xffffff \
             bg=0x000000 stride={}",
            RECT_X, RECT_Y, RECT_W, RECT_H, TEXT, TEXT_X, TEXT_Y, stride
        );
    }
    put(
        flushed,
        5,
        "gpu draw",
        &mut fails,
        "vẽ/TRANSFER_TO_HOST_2D/RESOURCE_FLUSH thất bại",
    );
    // Marker cho runner: **sau** khi flush xong, QEMU đã cập nhật console ⇒ `screendump` lúc này
    // đọc đúng nội dung vừa vẽ. Runner **không** dùng `sleep` mù.
    PLATFORM.put_str("# gpu: drawn\n");

    // ------------------------------------------------ kết thúc chủ đích
    let stats = virtio_gpu::stats();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# gpu stats: commands={} notifies={} errors={} timeouts={} id_mismatch={} \
             rejects={} published={} queue_size={} device_status=0x{:x}",
            stats.commands,
            stats.notifies,
            stats.errors,
            stats.timeouts,
            stats.id_mismatch,
            stats.rejects,
            virtio_gpu::published(),
            vifast_virtio::gpu::QUEUE_SIZE,
            virtio_gpu::device_status()
        );
        // Dấu kết thúc **chủ đích** (không có fault): runner đòi đúng **một** dòng PANIC cho mỗi
        // boot; ảnh này không fault nên tự in dấu kết thúc rồi `halt(0)`. Nội dung lấy từ `fails`.
        let _ = writeln!(w, "PANIC: KT-263 gpu xong (5 TAP, {fails} fail)");
    }
    // Giữ khung hình cho runner kịp `screendump` (xem [`HOLD_TICKS`]). Log đã **đủ** trước khi
    // giữ, nên runner đọc được toàn bộ bằng chứng trong lúc chờ.
    let hold_until = PLATFORM.now_ticks().wrapping_add(HOLD_TICKS);
    while PLATFORM.now_ticks() < hold_until {
        core::hint::spin_loop();
    }
    PLATFORM.halt(0)
}

/// Entry hart **thứ cấp** — ảnh gpu chỉ cần symbol để link (không gọi `smp::init`, không hart
/// thứ cấp nào chạy; nếu có thì park, không panic).
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
