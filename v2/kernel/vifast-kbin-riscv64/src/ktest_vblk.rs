// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test virtio-blk** riscv64 (KT-72, K6) — bin `vifast-kbin-ktest-vblk`, feature `ktest`,
//! nạp qua OpenSBI **kèm** `-drive file=…,format=raw,if=none,id=hd0 -device virtio-blk-pci`.
//!
//! # Vì sao ảnh riêng
//!
//! (a) Ảnh `ktest` đã ở **92,6 %** trần `.data+.bss` (D2-109) — driver + trang DMA 4 KiB đẩy nó
//! sát trần; (b) ảnh này cần **thiết bị thật** (`-drive`) trong dòng QEMU, tách ra thì 12 boot
//! matrix cũ **không đổi**; (c) boot kết thúc bằng **một** dòng `PANIC:` chủ đích (mỗi boot chỉ
//! kết thúc được một lần — cùng quy ước ảnh guard/user/proc, `ktest.sh` đòi đúng 1 PANIC).
//!
//! # TAP (6 test; tên khớp `expected.txt` + `ktest.sh`)
//!
//! 1. `vblk pci` — quét ECAM tìm **vendor/device đọc từ thiết bị** (`0x1af4:0x1001`), đo cỡ BAR
//!    bằng mask, **gán** BAR (firmware không enumerate), đọc lại BAR + `command`, đọc bốn
//!    capability; ECAM/BAR được map identity (walk phần mềm).
//! 2. `vblk queue` — đàm phán **duy nhất** `VIRTIO_F_VERSION_1`, thiết bị giữ `FEATURES_OK`,
//!    queue 0 bật, **dung lượng đọc từ `virtio_blk_config`**, địa chỉ DMA identity (`VA == PA`).
//! 3. `vblk ghi doc lai` — **16 request liên tiếp** (8 sector × 2 lượt ghi đè) rồi đọc lại **so
//!    từng byte** — vòng request/used chạy nhiều lần, không chỉ ca một request.
//! 4. `vblk flush` — `VIRTIO_BLK_T_FLUSH` **thật** (không no-op) được thiết bị hoàn tất
//!    (`status=OK`); `device_status` vẫn `DRIVER_OK`.
//! 5. `vblk fs tren dia` — `vifast_fs` **format/mount/ghi/fsck** trên **đĩa virtio thật** (impl
//!    mới của `BlockDev`; pager/journal **không sửa**), rồi mount lại từ đĩa (Fs mới, không cache
//!    cũ) đọc lại **so byte**, và đọc **thô** LBA 0/1: LBA 0 zero (thiết kế `fs.rs:638`), LBA 1
//!    = superblock `VF04` (`fs.rs:505-523`).
//! 6. `vblk loi ngoai dai` — LBA ≥ capacity: driver **từ chối trước** (`OutOfRange`, **không**
//!    phát request ⇒ không treo); ca **ép** phát request để thiết bị trả `IOERR` thật; sau lỗi
//!    thiết bị vẫn phục vụ đọc hợp lệ; `timeouts=0 id_mismatch=0`.
//!
//! Mọi dòng `# vblk:` in từ **biến đo** (D2-102); giá trị suy ra in kèm công thức/đầu vào
//! (D2-106). Không `unsafe` trong tệp này (MMIO nằm ở `kernel/vifast-plat-riscv64`).

use core::fmt::Write;

use vifast_fs::{BlockDev, FormatOptions, Fs, FsError, BLOCK_SIZE, INODE_FILE};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::virtio_blk;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::blk::BlkError;
use vifast_virtio::pci::{COMMAND_BUS_MASTER, COMMAND_MEMORY};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Cỡ ảnh đĩa mà runner tạo (`-drive`): 4 MiB = 8192 sector 512 B = đúng
/// `vifast_fs::TOTAL_BLOCKS`. Driver **đọc lại** capacity từ thiết bị; hằng này chỉ để đối chiếu
/// (`const:` trong evidence), không dùng làm số đo.
const DISK_BYTES: u64 = 4 << 20;

/// Handle `BlockDev` trên thiết bị virtio — **impl mới** (không sửa `vifast-fs`/`vifast-vfs`).
struct VirtioDev;

impl BlockDev for VirtioDev {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        virtio_blk::read_sector(lba, out).map_err(map_fs_error)
    }
    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        virtio_blk::write_sector(lba, data).map_err(map_fs_error)
    }
    fn flush(&mut self) -> Result<(), FsError> {
        virtio_blk::flush().map_err(map_fs_error)
    }
    fn block_count(&self) -> u32 {
        virtio_blk::capacity_sectors()
    }
}

/// Ánh xạ lỗi driver → `FsError` **một chỗ** (tên in ra lấy từ `BlkError::to_fs_name`).
fn map_fs_error(error: BlkError) -> FsError {
    if error == BlkError::OutOfRange {
        FsError::Invalid
    } else {
        FsError::Io
    }
}

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

pub fn run() -> ! {
    PLATFORM.put_str("TAP version 13\n1..6\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# vblk image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} tables={} used={} \
             const:disk_bytes={DISK_BYTES}",
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
    let pci_report = match pci::init() {
        Ok(report) => {
            let ecam_ok = identity(report.ecam_addr);
            let bar_ok = identity(report.bar_assigned);
            let ok = report.vendor == 0x1af4
                && (report.device == 0x1001 || report.device == 0x1042)
                && report.bar_readback == report.bar_assigned
                && report.command & (COMMAND_MEMORY | COMMAND_BUS_MASTER)
                    == COMMAND_MEMORY | COMMAND_BUS_MASTER
                && report.bar_size >= 0x1000
                && ecam_ok
                && bar_ok
                && report.overlap_checked
                && report.cap_ranges_ok
                && report.caps.common.offset as u64 + report.caps.common.length as u64
                    <= report.bar_size
                && report.caps.notify.offset as u64 + report.caps.notify.length as u64
                    <= report.bar_size
                && report.caps.device.offset as u64 + report.caps.device.length as u64
                    <= report.bar_size
                && report.caps.isr.offset as u64 + report.caps.isr.length as u64 <= report.bar_size;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# vblk pci: functions={} bus={} dev={} func={} vendor=0x{:x} device=0x{:x} \
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
                    "# vblk cap: common=0x{:x} notify=0x{:x} device=0x{:x} isr=0x{:x} mul={} \
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
                "vblk pci",
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
                "# vblk pci: err={} stage={}",
                fail.error.name(),
                fail.stage
            );
            put(
                false,
                1,
                "vblk pci",
                &mut fails,
                "không tìm/khởi tạo được PCI",
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };

    // ------------------------------------------------ 2) feature + queue + DMA identity
    let blk = match virtio_blk::init(&pci_report) {
        Ok(report) => {
            let dma_ok = identity(report.desc)
                && identity(report.avail)
                && identity(report.used)
                && identity(report.req_pa)
                && identity(report.data_pa)
                && identity(report.status_pa);
            let status = virtio_blk::device_status();
            let version_ok = report.device_features & (1u64 << 32) != 0
                && report.driver_features & (1u64 << 32) != 0;
            let ok = version_ok
                && report.flush_ok
                && report.driver_features
                    == vifast_virtio::blk::negotiated_features(report.device_features)
                && status & 0x0f == 0x0f
                && report.queue_size >= 2
                && report.queue_size.is_power_of_two()
                && report.capacity > 0
                && report.capacity == DISK_BYTES / 512
                && report.blk_size == 512
                && report.num_queues >= 1
                && dma_ok;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# vblk dev: capacity={} blk_size={} size_max={} seg_max={} num_queues={} \
                     queue_max={} queue_size={} device_features=0x{:x} driver_features=0x{:x} \
                     flush_feature={} status=0x{:x} notify_off={} notify_addr=0x{:x} \
                     config_generation={}",
                    report.capacity,
                    report.blk_size,
                    report.size_max,
                    report.seg_max,
                    report.num_queues,
                    report.queue_max,
                    report.queue_size,
                    report.device_features,
                    report.driver_features,
                    report.flush_ok as u8,
                    report.status,
                    report.notify_off,
                    report.notify_addr,
                    report.config_generation
                );
                let _ = writeln!(
                    w,
                    "# vblk dma: desc=0x{:x} avail=0x{:x} used=0x{:x} req=0x{:x} data=0x{:x} \
                     status_pa=0x{:x} identity={} version1={} status_readback=0x{:x}",
                    report.desc,
                    report.avail,
                    report.used,
                    report.req_pa,
                    report.data_pa,
                    report.status_pa,
                    dma_ok as u8,
                    version_ok as u8,
                    status
                );
            }
            put(
                ok,
                2,
                "vblk queue",
                &mut fails,
                "feature/queue/DMA/capacity không đúng",
            );
            if !ok {
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
            report
        }
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# vblk dev: init_fail={stage}");
            put(false, 2, "vblk queue", &mut fails, "init thiết bị thất bại");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };

    // ------------------------------------------------ 3) nhiều request: ghi → đọc lại so byte
    {
        /// LBA đầu của vùng thử (xa vùng metadata FS sẽ format ở TAP 5 — format ghi đè sau đó).
        const FIRST_LBA: u32 = 6000;
        const COUNT: u32 = 8;
        let mut mismatched = 0u32;
        let mut compared = 0u32;
        let mut first_diff: u32 = 0xffff_ffff;
        let before = virtio_blk::stats();
        for pass in 0..2u32 {
            for index in 0..COUNT {
                let lba = FIRST_LBA + index;
                let mut data = [0u8; BLOCK_SIZE];
                for (i, byte) in data.iter_mut().enumerate() {
                    *byte = (lba as usize * 17 + i * 3 + pass as usize) as u8;
                }
                if virtio_blk::write_sector(lba, &data).is_err() {
                    mismatched += 1;
                    continue;
                }
                let mut back = [0u8; BLOCK_SIZE];
                if virtio_blk::read_sector(lba, &mut back).is_err() {
                    mismatched += 1;
                    continue;
                }
                compared += BLOCK_SIZE as u32;
                for (offset, (want, got)) in data.iter().zip(back.iter()).enumerate() {
                    if want != got {
                        mismatched += 1;
                        if first_diff == 0xffff_ffff {
                            first_diff = offset as u32;
                        }
                        break;
                    }
                }
            }
        }
        let after = virtio_blk::stats();
        let writes = after.writes - before.writes;
        let reads = after.reads - before.reads;
        let completions = after.completions - before.completions;
        let ok = mismatched == 0
            && compared == 2 * COUNT * BLOCK_SIZE as u32
            && writes == 2 * COUNT as u64
            && reads == 2 * COUNT as u64
            && completions == 4 * COUNT as u64;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# vblk io: const:sectors={COUNT} const:passes=2 writes={writes} reads={reads} \
                 compared={compared} (=reads({reads})*512) mismatched={mismatched} \
                 first_diff={first_diff} requests={} completions={} const:first_lba={FIRST_LBA}",
                after.requests, after.completions
            );
        }
        put(
            ok,
            3,
            "vblk ghi doc lai",
            &mut fails,
            "so byte ghi/đọc lại lệch",
        );
    }

    // ------------------------------------------------ 4) flush thật
    {
        let before = virtio_blk::stats();
        let mut ok = true;
        for _ in 0..2 {
            if virtio_blk::flush().is_err() {
                ok = false;
            }
        }
        let after = virtio_blk::stats();
        let status = virtio_blk::device_status();
        let calls = after.flushes - before.flushes;
        let completions = after.completions - before.completions;
        ok = ok
            && calls == 2
            && completions == 2
            && status & 0x04 != 0
            && after.errors == before.errors;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# vblk flush: calls={calls} flushes={} completions={completions} \
                 device_status=0x{:x} driver_ok={} errors={} timeouts={}",
                after.flushes,
                status,
                (status & 0x04 != 0) as u8,
                after.errors,
                after.timeouts
            );
        }
        put(
            ok,
            4,
            "vblk flush",
            &mut fails,
            "FLUSH không được thiết bị hoàn tất",
        );
    }

    // ------------------------------------------------ 5) FS thật trên đĩa virtio
    fs_phase(&mut fails);

    // ------------------------------------------------ 6) ca lỗi: ngoài capacity
    {
        let capacity = virtio_blk::capacity_sectors();
        let before = virtio_blk::stats();
        let mut sector = [0u8; BLOCK_SIZE];
        let refused = virtio_blk::read_sector(capacity, &mut sector);
        let after_refused = virtio_blk::stats();
        let no_request = after_refused.requests == before.requests;
        let refused_name = match refused {
            Err(error) => error.name(),
            Ok(()) => "Ok",
        };
        let refused_ok = matches!(refused, Err(BlkError::OutOfRange)) && no_request;
        // Tên lỗi FS tương ứng in từ **hàm ánh xạ** (D2-102), không phải chuỗi viết tay.
        let refused_fs = BlkError::OutOfRange.to_fs_name();
        // Ca ÉP: phát request thật vượt dung lượng để **thiết bị** trả IOERR (không kiểm trước).
        let forced = virtio_blk::read_sector_force(capacity + 1, &mut sector);
        let after_forced = virtio_blk::stats();
        let forced_name = match forced {
            Err(error) => error.name(),
            Ok(()) => "Ok",
        };
        let forced_requests = after_forced.requests - after_refused.requests;
        let forced_ok = matches!(forced, Err(BlkError::Io))
            && forced_requests == 1
            && after_forced.timeouts == 0;
        // Sau lỗi, thiết bị vẫn phục vụ đọc hợp lệ (queue không kẹt).
        let recovered = virtio_blk::read_sector(0, &mut sector).is_ok();
        let after = virtio_blk::stats();
        // Bộ đếm `errors` phải **tăng đúng 1** cho ca ép (ca từ chối không phát request ⇒ không
        // tăng) — đây là ca dương chứng minh bộ đếm lỗi thật, không phải hằng 0 (D2-108B).
        let errors_delta = after.errors - after_refused.errors;
        // D2-110: `id_mismatch`/`timeouts` **không** nằm trong điều kiện `ok` — chưa có ca âm
        // (D2-108B) nên TAP 6 **không phải** bằng chứng cho chúng; vẫn in ra như **quan sát**.
        let ok = refused_ok && forced_ok && recovered && errors_delta == 1;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# vblk loi: capacity={capacity} refused={refused_name} \
                 refused_no_request={} (=requests_after({})-requests_before({})) \
                 forced={forced_name} forced_requests={forced_requests} \
                 (=requests_after({})-requests_before({})) recovered={} device_status=0x{:x} \
                 id_mismatch={} timeouts={} const:id_timeout_proven=0 errors_before={} \
                 errors_after={} errors_delta={errors_delta} (=after-before) \
                 fs_err_refused={refused_fs}",
                no_request as u8,
                after_refused.requests,
                before.requests,
                after_forced.requests,
                after_refused.requests,
                recovered as u8,
                virtio_blk::device_status(),
                after.id_mismatch,
                after.timeouts,
                after_refused.errors,
                after.errors
            );
        }
        put(
            ok,
            6,
            "vblk loi ngoai dai",
            &mut fails,
            "mã lỗi ngoài dải không đúng/treo",
        );
    }

    // ------------------------------------------------ tổng kết + dấu kết thúc chủ đích
    {
        let stats = virtio_blk::stats();
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# vblk stats: requests={} reads={} writes={} flushes={} completions={} errors={} \
             notifies={} id_mismatch={} timeouts={} capacity={}",
            stats.requests,
            stats.reads,
            stats.writes,
            stats.flushes,
            stats.completions,
            stats.errors,
            stats.notifies,
            stats.id_mismatch,
            stats.timeouts,
            blk.capacity
        );
    }
    // Dấu kết thúc **chủ đích** (không có fault): `ktest.sh` đòi đúng **một** dòng PANIC cho mỗi
    // boot; ảnh này không fault nên tự in dấu kết thúc rồi `halt(0)`. Nội dung lấy từ biến `fails`.
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(w, "PANIC: KT-72 vblk xong (6 TAP, {fails} fail)");
    }
    PLATFORM.halt(0)
}

/// Pha FS **trên đĩa virtio thật** (TAP 5).
///
/// (A) format đĩa rồi mount → tạo/ghi `/kt72.txt` → `fsck` + `fsck_repair` (phải 0 hỏng).
/// (B) **drop** `Fs` rồi mount lại từ thiết bị (Fs mới ⇒ mọi block đọc lại qua virtio), đọc file
///     **so byte** + `fsck` sạch — chứng minh dữ liệu nằm trên đĩa.
/// (C) đọc **thô** LBA 0/1 qua thiết bị, kiểm superblock `VF04` (LBA 1, `fs.rs:639`).
///
/// Một object `Fs` (13 KiB) tại một thời điểm, hai khối tuần tự ⇒ stack phẳng (luật KT-71).
#[inline(never)]
fn fs_phase(fails: &mut usize) {
    const PATH: &[u8] = b"/kt72.txt";
    const PAYLOAD: &[u8] = b"ViFastOs KT-72: du lieu nay nam tren dia virtio that, khong phai RAM.";
    let mut ok = true;

    // ---- A: format + ghi + fsck
    let mut formatted_ok = false;
    let mut wrote = 0usize;
    let mut fsck_clean = false;
    let mut found = 0u32;
    let mut fixed = 0u32;
    let mut unfixed = 0u32;
    {
        let mut fs = Fs::empty(VirtioDev);
        let mut dev = VirtioDev;
        if vifast_fs::format_with_options(&mut dev, FormatOptions::default()).is_ok()
            && fs.initialize().is_ok()
        {
            formatted_ok = true;
            match fs.create(PATH, 0, INODE_FILE) {
                Ok(ino) => match fs.write(ino, 0, PAYLOAD, 0) {
                    Ok(count) => wrote = count,
                    Err(_) => ok = false,
                },
                Err(_) => ok = false,
            }
            fsck_clean = fs.fsck().is_ok();
            match fs.fsck_repair() {
                Ok(report) => {
                    found = report.found;
                    fixed = report.fixed;
                    unfixed = report.unfixed;
                }
                Err(_) => ok = false,
            }
        } else {
            ok = false;
        }
    }
    if !formatted_ok || !fsck_clean || wrote != PAYLOAD.len() || found != 0 || unfixed != 0 {
        ok = false;
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# vblk fs: phase=write formatted={} wrote={} fsck_clean={} found={found} fixed={fixed} \
             unfixed={unfixed} const:payload_len={}",
            formatted_ok as u8,
            wrote,
            fsck_clean as u8,
            PAYLOAD.len()
        );
    }

    // ---- B: mount lại từ đĩa + đọc lại so byte
    let mut read_len = 0usize;
    let mut matches = false;
    let mut remount_fsck = false;
    {
        let mut fs = Fs::empty(VirtioDev);
        if fs.initialize().is_ok() {
            if let Ok(ino) = fs.lookup(PATH, 0) {
                let mut back = [0u8; 128];
                if let Ok(count) = fs.read(ino, 0, &mut back[..PAYLOAD.len()], 0) {
                    read_len = count;
                    matches = count == PAYLOAD.len() && &back[..count] == PAYLOAD;
                }
            }
            remount_fsck = fs.fsck().is_ok();
        } else {
            ok = false;
        }
    }
    if !matches || !remount_fsck || read_len != PAYLOAD.len() {
        ok = false;
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# vblk fs: phase=remount read_len={read_len} content_match={} fsck_clean={} \
             const:payload_len={}",
            matches as u8,
            remount_fsck as u8,
            PAYLOAD.len()
        );
    }

    // ---- C: đọc THÔ LBA 0/1 từ thiết bị (ngoài mọi cache của FS)
    {
        let mut lba0 = [0u8; BLOCK_SIZE];
        let mut lba1 = [0u8; BLOCK_SIZE];
        let read0 = virtio_blk::read_sector(0, &mut lba0).is_ok();
        let read1 = virtio_blk::read_sector(1, &mut lba1).is_ok();
        let lba0_zero = lba0.iter().all(|byte| *byte == 0);
        let magic = &lba1[..4];
        let version = u32::from_le_bytes([lba1[4], lba1[5], lba1[6], lba1[7]]);
        let total_blocks = u32::from_le_bytes([lba1[12], lba1[13], lba1[14], lba1[15]]);
        let magic_ok = magic == b"VF04" && version == 4;
        if !(read0 && read1 && lba0_zero && magic_ok) {
            ok = false;
        }
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# vblk magic: lba0_read={} lba0_zero={} lba1_read={} lba1_magic={} \
                 lba1_version={version} lba1_total_blocks={total_blocks}",
                read0 as u8,
                lba0_zero as u8,
                read1 as u8,
                core::str::from_utf8(magic).unwrap_or("?")
            );
        }
    }

    put(
        ok,
        5,
        "vblk fs tren dia",
        fails,
        "format/mount/ghi/đọc lại/fsck trên đĩa virtio không sạch",
    );
    let stats = virtio_blk::stats();
    let mut w = PlatformWriter(&PLATFORM);
    let _ = writeln!(
        w,
        "# vblk fs: requests={} flushes={} completions={} errors={} capacity={}",
        stats.requests,
        stats.flushes,
        stats.completions,
        stats.errors,
        virtio_blk::capacity_sectors()
    );
}
