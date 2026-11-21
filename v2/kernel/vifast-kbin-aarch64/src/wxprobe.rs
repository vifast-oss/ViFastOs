// SPDX-License-Identifier: MIT OR Apache-2.0
//! **W^X toàn ảnh + cache maintenance trước DMA** của ảnh aarch64 — KT-277 (K4' bước 3).
//!
//! # Hai phép đo, hai TAP (22/23)
//!
//! 1. **`# wx:`** — vùng ảnh đã tách `.text`/`.rodata` **RX** và `.data`/`.bss`/stack **RW**,
//!    trang guard của stack per-hart **không map**. Bằng chứng **không thể giả**:
//!    - quyền của lá đọc lại **từ bảng trang** (`VSpace::walk`) — không phải từ hằng số;
//!    - **ghi** vào `.text` từ EL1 ⇒ **data abort quyền** (`EC=0x25`, `WnR=1`, `FAR` đúng địa
//!      chỉ đã ghi) — thử ở **cả** alias nửa cao (nơi kernel đang chạy) **và** identity nửa thấp
//!      (bảng `TTBR0` của `VSpace`);
//!    - 8 byte của `.text` **không đổi** sau lần ghi bị từ chối;
//!    - ghi vào `.data` thì **thành công** và đọc lại đúng ⇒ RW thật, không phải "RX cả ảnh".
//! 2. **`# dma:`** — bộ đếm của **chính vòng lặp phát `dc`** trong `virtio_blk`: số dòng đã
//!    `dc civac` trước khi bàn giao ring và số dòng đã `dc ivac` sau khi thiết bị ghi.
//!
//! # Vì sao phép đo W^X phải chạy **sau** `activate`
//!
//! Trước `activate`, kernel chạy trên bảng **boot** (cũng đã W^X từ KT-277), nhưng phép đo chỉ có
//! nghĩa khi bảng **đang hoạt động** là bảng của `VSpace` — đó mới là bảng mà chương trình thật
//! dùng. Vì vậy `run` được gọi từ `el0_done` (sau U-mode + PCI), nơi `VSpace` đã `activate`.
//!
//! # Ghi trung thực về giới hạn của QEMU TCG
//!
//! QEMU TCG **không** mô hình hoá cache, nên **hiệu ứng** của `dc civac`/`dc ivac` không quan sát
//! được trên QEMU (xem `v2/docs/k4-aarch64-boot.md` §11). Tầng quan sát được là **bộ đếm**:
//! bỏ lời gọi `dc` ⇒ `lines=0` ⇒ runner ĐỎ. Tầng host là `tests/cache.rs` (đếm số lệnh + địa chỉ
//! từng dòng). **Không** tuyên bố "đã kiểm trên phần cứng".

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::bootmem::SharedBootMemory;
use vifast_plat_aarch64::cache;
use vifast_plat_aarch64::fault::{classify_esr, Access, EC_DATA_ABORT_SAME_EL};
use vifast_plat_aarch64::format::Perms;
use vifast_plat_aarch64::pl011;
use vifast_plat_aarch64::split_mmu::{high_alias_va, KERNEL_HIGH_BASE};
use vifast_plat_aarch64::sysreg::Esr;
use vifast_plat_aarch64::virtio_blk;
use vifast_plat_aarch64::vspace::VSpace;
use vifast_plat_aarch64::wx::{self, ImageLayout};

use crate::{faults, tap, Console, TEXT_START_PA};

/// PA của `__data_start` — đọc lúc PC còn ở nửa thấp (sau khi nhảy, `adrp` cho alias nửa cao).
static DATA_START_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__smp_stacks_start`.
static SMP_STACKS_START_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__smp_stacks_end`.
static SMP_STACKS_END_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__kernel_end`.
static KERNEL_END_PA: AtomicU64 = AtomicU64::new(0);

/// Quyền lá tại VA của `.text`, đọc lại **từ bảng trang** (`u64::MAX` = chưa đo).
static TEXT_PERMS: AtomicU64 = AtomicU64::new(u64::MAX);
/// Quyền lá tại VA của `.data`, đọc lại **từ bảng trang**.
static DATA_PERMS: AtomicU64 = AtomicU64::new(u64::MAX);

/// Probe `.data`: khởi tạo **khác 0** ⇒ linker đặt trong `.data` (không phải `.bss`).
///
/// Đây là chỗ duy nhất trong ảnh chứng minh `.data` **ghi được** — `.bss` thì cả ảnh đã ghi suốt
/// từ `_start`, nên nó không nói gì về `.data`.
static DATA_PROBE: AtomicU64 = AtomicU64::new(0x5758_4441_5441_0001);

/// Giá trị ghi thử vào `.text` (không bao giờ được ghi ra — phải fault).
const TEXT_WRITE_MAGIC: u64 = 0x5758_5F54_4558_5401;
/// Giá trị ghi thử vào `.data` (phải ghi được).
const DATA_WRITE_MAGIC: u64 = 0x5758_5F44_4154_4102;

/// VA (nửa cao) của hàm **đã chạy xong** dùng làm đích ghi thử W^X.
///
/// `pl011::init` chạy ở TAP 1 và **không bao giờ** chạy lại. Nhờ vậy nếu đột biến "map `.text`
/// RW" làm lần ghi **thành công**, 8 byte bị đổi không ảnh hưởng phần còn lại của lần boot —
/// đột biến vẫn đỏ vì `ok=0`, **không** đỏ vì crash ngẫu nhiên ở một hàm khác.
fn text_probe_va() -> u64 {
    pl011::init as *const () as u64
}

/// Số trang đã map vào `VSpace` cho ba đường của vùng ảnh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VSpaceImagePages {
    /// Alias nửa cao (`TTBR1`).
    pub high: usize,
    /// Physmap (`TTBR1`).
    pub phys: usize,
    /// Identity nửa thấp (`TTBR0`).
    pub identity: usize,
}

/// Ghi PA của năm symbol linker vào static — gọi **một lần** trong `mmu_bringup`, lúc PC còn ở
/// nửa thấp (sau khi nhảy, `adrp` trả alias nửa cao, không dùng làm PA được).
pub fn publish_layout(
    text_start: u64,
    data_start: u64,
    kernel_end: u64,
    stacks_start: u64,
    stacks_end: u64,
) {
    TEXT_START_PA.store(text_start, Ordering::Relaxed);
    DATA_START_PA.store(data_start, Ordering::Relaxed);
    KERNEL_END_PA.store(kernel_end, Ordering::Relaxed);
    SMP_STACKS_START_PA.store(stacks_start, Ordering::Relaxed);
    SMP_STACKS_END_PA.store(stacks_end, Ordering::Relaxed);
}

/// Bố cục ảnh từ các PA đã công bố.
pub fn layout() -> ImageLayout {
    ImageLayout::new(
        TEXT_START_PA.load(Ordering::Relaxed),
        DATA_START_PA.load(Ordering::Relaxed),
        KERNEL_END_PA.load(Ordering::Relaxed),
        SMP_STACKS_START_PA.load(Ordering::Relaxed),
        SMP_STACKS_END_PA.load(Ordering::Relaxed),
    )
}

/// Mã hoá quyền của một lá để in (`u64::MAX` = không đọc được bảng trang).
const fn perms_code(perms: Option<Perms>) -> u64 {
    match perms {
        None => u64::MAX,
        Some(p) => {
            (p.read as u64)
                | ((p.write as u64) << 1)
                | ((p.exec as u64) << 2)
                | ((p.user as u64) << 3)
        }
    }
}

/// Tên quyền in ra log — **suy từ bit**, không phải chuỗi viết tay ở chỗ khác.
const fn perms_name(code: u64) -> &'static str {
    match code {
        0b0101 => "RX",
        0b0011 => "RW",
        0b0001 => "R",
        0b0111 => "RWX",
        _ => "?",
    }
}

/// Map vùng ảnh vào `VSpace` theo W^X (alias nửa cao + physmap + identity nửa thấp) rồi **đọc
/// lại quyền từ bảng trang** để dòng `# wx:` là bằng chứng, không phải lời khai.
///
/// Trả `None` nếu một đường map lỗi (caller PANIC — fail-loud, không bỏ trang im lặng).
pub fn map_vspace_image(
    vs: &mut VSpace<SharedBootMemory>,
    console: &mut Console,
) -> Option<VSpaceImagePages> {
    let layout = layout();
    let high = match wx::map_vspace_high(vs, &layout) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: wx: map kernel high lỗi {error:?}");
            return None;
        }
    };
    let phys = match wx::map_vspace_phys(vs, &layout) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: wx: map physmap lỗi {error:?}");
            return None;
        }
    };
    let identity = match wx::map_vspace_low(vs, &layout) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: wx: map image identity lỗi {error:?}");
            return None;
        }
    };
    // Quyền **đọc lại từ bảng trang** — nguồn sự thật của phép đo, không phải hằng `Perms`.
    let text_va = text_probe_va();
    let data_va = high_alias_va(layout.data_start, layout.text_start, KERNEL_HIGH_BASE);
    TEXT_PERMS.store(
        perms_code(vs.walk(text_va).ok().map(|t| t.perms)),
        Ordering::Relaxed,
    );
    DATA_PERMS.store(
        perms_code(vs.walk(data_va).ok().map(|t| t.perms)),
        Ordering::Relaxed,
    );
    Some(VSpaceImagePages {
        high,
        phys,
        identity,
    })
}

/// Kết quả một lần ghi thử bị từ chối: `(đúng_loại_fault, ec, far, elr)`.
fn check_write_fault(fault: Option<faults::RawFault>, expect_va: u64) -> (bool, u8, u64, u64) {
    let Some(raw) = fault else {
        return (false, 0, 0, 0);
    };
    let esr = Esr::from_bits(raw.esr);
    let ec = esr.ec_raw();
    let abort = classify_esr(raw.esr, raw.far);
    let ok = matches!(
        abort,
        Some(abort) if abort.access == Access::Write
            && abort.va == expect_va
            && abort.ec == EC_DATA_ABORT_SAME_EL
    );
    (ok, ec, raw.far, raw.elr)
}

/// Chạy hai phép đo KT-277, in `# wx:`/`# dma:` + TAP 22/23. Trả `true` ⇔ **cả hai** xanh.
///
/// `blk_ok` = kết quả ba ca PCI/virtio-blk (TAP 19/20/21): dòng `# dma:` chỉ có nghĩa khi driver
/// **thật sự** phát request (không có đĩa ⇒ không có `dc` nào để đếm ⇒ `ok=0`, đúng fail-closed).
pub fn run(console: &mut Console, blk_ok: bool) -> bool {
    let mut all_ok = true;
    let layout = layout();
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    let text_va = text_probe_va();
    // PA của cùng ô: alias nửa cao là `KERNEL_HIGH_BASE + (pa - text_start)`.
    let text_pa = text_start + (text_va - KERNEL_HIGH_BASE);
    let data_va = high_alias_va(layout.data_start, text_start, KERNEL_HIGH_BASE);
    let kernel_end_va = high_alias_va(layout.kernel_end, text_start, KERNEL_HIGH_BASE);

    // ---------------------------------------------------------------- TAP 22: W^X
    // (1) 8 byte gốc của `.text` — phải **không đổi** sau lần ghi bị từ chối.
    let text_before = asm::read_u64(text_va);
    // (2) Ghi vào `.text` qua **alias nửa cao** (nơi kernel đang chạy) ⇒ phải fault.
    faults::reset_raw(0);
    faults::expect_raw(0);
    asm::write_u64(text_va, TEXT_WRITE_MAGIC);
    let (high_fault_ok, high_ec, high_far, high_elr) = check_write_fault(faults::raw(0), text_va);
    // Nếu lần ghi **không** fault (đột biến "map `.text` RW"), cờ "đang chờ" còn set — xoá để
    // một fault **thật** sau đó không bị nuốt. Đường sản phẩm: handler đã tiêu cờ ⇒ no-op.
    let _ = faults::take_expect_raw(0);
    let text_after = asm::read_u64(text_va);
    // (3) Ghi vào `.text` qua **identity nửa thấp** (bảng `TTBR0` của `VSpace`) ⇒ cũng phải fault.
    faults::reset_raw(0);
    faults::expect_raw(0);
    asm::write_u64(text_pa, TEXT_WRITE_MAGIC);
    let (low_fault_ok, low_ec, low_far, _low_elr) = check_write_fault(faults::raw(0), text_pa);
    let _ = faults::take_expect_raw(0);
    let text_after_low = asm::read_u64(text_pa);
    // (4) Ghi vào `.data` ⇒ phải **thành công** và đọc lại đúng (RW thật).
    let data_addr = core::ptr::addr_of!(DATA_PROBE) as u64;
    DATA_PROBE.store(DATA_WRITE_MAGIC, Ordering::Relaxed);
    let data_readback = DATA_PROBE.load(Ordering::Relaxed);
    let data_rw = data_readback == DATA_WRITE_MAGIC;
    let data_in_rw_region = data_addr >= data_va && data_addr < kernel_end_va;

    let text_perms = TEXT_PERMS.load(Ordering::Relaxed);
    let data_perms = DATA_PERMS.load(Ordering::Relaxed);
    let text_unchanged = text_before == text_after && text_before == text_after_low;
    let wx_ok = text_perms == perms_code(Some(Perms::KERNEL_RX))
        && data_perms == perms_code(Some(Perms::KERNEL_RW))
        && high_fault_ok
        && low_fault_ok
        && text_unchanged
        && data_rw
        && data_in_rw_region;
    let _ = writeln!(
        console,
        "# wx: text=RX data=RW ok={} text_bytes={} data_bytes={} guard_pages={} guard_bytes={} text_va=0x{text_va:x} data_va=0x{data_va:x} text_perms={} data_perms={} fault_ec=0x{high_ec:02x} fault_access=Write fault_va=0x{high_far:x} fault_elr=0x{high_elr:x} low_fault_ec=0x{low_ec:02x} low_fault_va=0x{low_far:x} text_unchanged={} data_rw={} data_in_rw={}",
        wx_ok as u8,
        layout.text_bytes(),
        layout.data_bytes(),
        layout.guard_pages(),
        layout.guard_bytes(),
        perms_name(text_perms),
        perms_name(data_perms),
        text_unchanged as u8,
        data_rw as u8,
        data_in_rw_region as u8,
    );
    if !tap(console, 26, "wx: text RX (write faults), data RW", wx_ok) {
        all_ok = false;
    }

    // ---------------------------------------------------------------- TAP 23: cache DMA
    let stats = virtio_blk::cache_stats();
    let ctr = asm::read_ctr_el0();
    let line = cache::cache_line_bytes(ctr);
    let dma_ok = blk_ok && stats.ok();
    let _ = writeln!(
        console,
        "# dma: cache-clean ok={} lines={} bytes={} ops={} inval_lines={} inval_bytes={} inval_ops={} poll_inval={} line={} ctr=0x{ctr:016x}",
        dma_ok as u8,
        stats.clean_lines,
        stats.clean_bytes,
        stats.clean_ops,
        stats.inval_lines,
        stats.inval_bytes,
        stats.inval_ops,
        stats.poll_inval,
        line,
    );
    if !tap(
        console,
        27,
        "dma: cache maintenance before/after DMA",
        dma_ok,
    ) {
        all_ok = false;
    }

    all_ok
}
