// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Cầu nối `vifast-core` vào ảnh x86_64** (KT-340c, D2-233/D2-244/D2-247) — `Machine` là
//! `static` **trong `.bss`** của ảnh, bọc [`IrqMcsLock`] (D2-238: khoá lá phải tắt `RFLAGS.IF`).
//!
//! # Khuôn: `vifast-plat-aarch64/src/core_bridge.rs` (KT-338b) + riscv64 (KT-61/KT-336c)
//!
//! Cùng **một** mô hình duy nhất cho ảnh; không có `&mut Machine` thứ hai, không `static mut`
//! công khai, không bản sao. Khác biệt **chỉ** ở nền tảng: khoá dùng `RFLAGS.IF` (x86_64) thay
//! `PSTATE.DAIF`/`sstatus.SIE`, và **không** có phép đổi VA nửa cao → PA: ảnh x86_64 chạy
//! `VA == PA` (identity map của `_start`) nên mọi địa chỉ symbol **đã là** PA.
//!
//! # Vì sao `MaybeUninit` + `init_zeroed` (không `MaybeUninit::write`)
//!
//! `Machine` có trường khởi tạo **khác 0** (`NODE_NONE` cho bảng WaitAddr, `n_harts`, …) nên
//! không thể là zeroed `.bss` thuần, và `MACHINE.write(Machine::init_zeroed(..))` dựng **bản tạm
//! 184.792 B trên stack** (bài học KT-61/KT-338b: tràn stack 64 KiB). Đường đúng: `.bss`
//! (`MaybeUninit`) → `write_bytes(0)` → [`Machine::init_zeroed`] **tại chỗ** (không bản tạm,
//! không copy).
//!
//! # `.data+.bss` — có đo, không hứa
//!
//! Ảnh **ktest** cộng đúng `size_of::<Machine>()` vào `.data+.bss` (đo bằng
//! `budget-x86_64.sh`, trần 262.144 B). Ảnh **sản phẩm** **không** mang model: `core_bridge` nằm
//! sau feature `core` của crate arch, chỉ ảnh kiểm bật (cùng quyết định KT-338b của aarch64) —
//! trần `.data+.bss` sản phẩm của x86_64 là **65.536 B** (D2-243/D2-244), nhỏ hơn
//! `size_of::<Machine>()`. Bật model cho ảnh sản phẩm là quyết định **ngân sách** của `KT-341`.
//!
//! `unsafe` ở đây thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17) và **chỉ** gồm một `static`
//! + con trỏ tới chính nó (không asm/MMIO mới — phần `cli`/`sti` nằm ở [`crate::irq_lock`]).

#![allow(unsafe_code)]

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_core::{Machine, ObjId};

use crate::irq_lock::{IrqMcsGuard, IrqMcsLock};

/// Pin **D2-84**: `size_of::<Machine>()` ≤ **196.608 B** (192 KiB). Vượt ⇒ tối ưu cấu trúc,
/// **không** nâng pin.
pub const CORE_PIN_BYTES: usize = 196_608;

/// Trần **ảnh link** (QD §3.5): `.data+.sdata+.bss+.sbss` ≤ **262.144 B** — ngưỡng **khác** pin
/// trên, `budget-x86_64.sh` đo trên ELF thật.
pub const IMAGE_CAP_BYTES: usize = 262_144;

/// `size_of::<Machine>()` — in ra trong TAP/báo cáo cùng số đo `budget-x86_64.sh`.
pub const CORE_STATIC_BYTES: usize = core::mem::size_of::<Machine>();

/// Model core duy nhất của ảnh — `MaybeUninit`, đặt trong section **NOLOAD `.corespace`**
/// (KT-341) để `size_of::<Machine>()` **không** rơi vào `.data+.bss` của ảnh sản phẩm
/// (trần 65.536 B). NOLOAD vẫn nằm trong `[__text_start, __kernel_end)` ⇒ `in_image_region`
/// vẫn đúng và `init` zero tường minh toàn bộ (không dựa vào zero của `.bss`).
/// Đột biến (i) `mut-machine-bss` bỏ `link_section` ⇒ `Machine` rơi lại `.bss` ⇒
/// `budget-x86_64.sh` **ĐỎ** (43.072 + 184.792 = 227.864 ≥ 65.536).
#[cfg_attr(not(feature = "mut-machine-bss"), link_section = ".corespace")]
static mut MACHINE: MaybeUninit<Machine> = MaybeUninit::uninit();

/// Khoá một-người-ghi cho [`MACHINE`] — MCS **bọc lưu/tắt `RFLAGS.IF`** ([`IrqMcsLock`]).
///
/// Vì sao **không** `McsLock` trần: handler ngắt (`trap.rs`) có thể chạm `Machine` trong khi hart
/// đang giữ khoá ⇒ quay lại cùng khoá của chính hart ⇒ tự deadlock (D2-238/D2-245). Guard tắt
/// `IF` **trước** khi chờ và khôi phục **đúng** giá trị đã lưu sau khi nhả.
static LOCKED: IrqMcsLock<()> = IrqMcsLock::new(());

/// `init` đã ghi model chưa (debug assert của [`with_machine`]).
static MACHINE_INIT: AtomicBool = AtomicBool::new(false);

/// Số hart **nền tảng** đã truyền vào `Machine::init_zeroed` (D2-88(a)) — đọc bởi
/// `SyscallEnv::num_harts`, để `DebugGetInfo` trả số hart thật của lần boot này.
static HART_COUNT: AtomicU64 = AtomicU64::new(1);

/// Số hart nền tảng của lần boot này (`init` đã chạy).
pub fn harts() -> usize {
    HART_COUNT.load(Ordering::Acquire) as usize
}

/// Biên vùng ảnh kernel `[start, end)` — do ảnh (linker) công bố bằng [`set_image_bounds`]
/// **trước** [`init`]; dùng để chứng minh `static MACHINE` nằm **trong ảnh** (không phải
/// arena/RAM ngoài).
static IMAGE_START: AtomicU64 = AtomicU64::new(0);
static IMAGE_END: AtomicU64 = AtomicU64::new(0);

/// `true` ⇔ `[addr, addr+bytes)` nằm trong vùng ảnh do ảnh công bố.
///
/// Khác aarch64: x86_64 **không** có nửa cao — ảnh liên kết ở `0x0010_0000` và `_start` dựng
/// identity map `0..1 GiB`, nên `VA == PA` và **không** có phép chuẩn hoá nào. Thêm một phép
/// chuẩn hoá ở đây sẽ là bản sao công thức của nền tảng khác (đúng thứ D2-83 cấm).
pub fn in_image_region(addr: u64, bytes: usize) -> bool {
    let start = IMAGE_START.load(Ordering::Acquire);
    let end = IMAGE_END.load(Ordering::Acquire);
    let stop = addr.saturating_add(bytes as u64);
    start != 0 && end > start && addr >= start && stop <= end && stop > addr
}

/// Công bố biên vùng ảnh (gọi **một** lần trước [`init`]).
pub fn set_image_bounds(start: u64, end: u64) {
    IMAGE_START.store(start, Ordering::Release);
    IMAGE_END.store(end, Ordering::Release);
}

/// Địa chỉ `static MACHINE` của ảnh — bằng chứng "model nằm trong `.bss` ảnh".
pub fn machine_addr() -> u64 {
    core::ptr::addr_of!(MACHINE) as u64
}

/// `size_of::<Machine>()`.
pub fn machine_bytes() -> usize {
    CORE_STATIC_BYTES
}

/// Báo cáo [`init`] — số **thật** của lần boot này (in vào `# core:` của TAP).
#[derive(Clone, Copy, Debug)]
pub struct CoreReport {
    /// Địa chỉ `static MACHINE` trong ảnh.
    pub addr: u64,
    /// `size_of::<Machine>()`.
    pub static_bytes: usize,
    /// Pin D2-84.
    pub pin: usize,
    /// Đáy Untyped boot (nhãn PA, D2-83).
    pub base: u64,
    /// Cỡ Untyped boot.
    pub arena_bytes: usize,
    /// Object Untyped của core.
    pub untyped: ObjId,
    /// Số hart **nền tảng** đã truyền vào `Machine::init_zeroed` (D2-88(a)).
    pub harts: usize,
    /// Số object trong bảng core.
    pub objects: usize,
    /// Số cap sống.
    pub caps: usize,
    /// `check_caps` ĐẠT.
    pub check_caps: bool,
    /// `check_bytes(untyped)` ĐẠT.
    pub check_bytes: bool,
    /// `check_all_bytes` ĐẠT.
    pub check_all_bytes: bool,
    /// `static MACHINE` nằm trong `[image_start, image_end)`.
    pub in_image: bool,
}

impl CoreReport {
    /// `true` ⇔ mọi bất biến tầng 1 xanh + pin D2-84 + static trong ảnh.
    pub fn healthy(&self) -> bool {
        self.static_bytes <= self.pin
            && self.in_image
            && self.check_caps
            && self.check_bytes
            && self.check_all_bytes
    }
}

/// Chạy `f` với **`&mut Machine` duy nhất** của ảnh (khoá; **không** re-entrant).
///
/// # Panic
/// [`init`] chưa chạy ⇒ `MACHINE` chưa bit-valid. Mọi caller trong repo gọi [`init`] trước
/// (ảnh ktest `init` ngay sau khi có arena). Debug build có assert.
pub fn with_machine<R>(f: impl FnOnce(&mut Machine) -> R) -> R {
    // Guard giữ tới hết closure: `cli` **trước** khi chờ khoá, nhả khoá **trước** khi khôi phục
    // `IF` (thứ tự bắt buộc của `irq_lock`). `_guard` rơi ở cuối hàm ⇒ cả đường panic.
    let _guard: IrqMcsGuard<'_, ()> = LOCKED.lock_irqsave();
    debug_assert!(
        MACHINE_INIT.load(Ordering::Acquire),
        "core_bridge: with_machine trước init"
    );
    // SAFETY: `MACHINE` là static duy nhất; `init` đã zero + `init_zeroed` (bit-valid `Machine`)
    // trước mọi lần đọc, và khoá bảo đảm chỉ một `&mut` tại một thời điểm. Không đường nào khác
    // chạm static này.
    let m = unsafe { &mut *core::ptr::addr_of_mut!(MACHINE).cast::<Machine>() };
    f(m)
}

/// Ghi model core vào `.bss` và dựng **Untyped boot** từ `[base, base+bytes)`.
///
/// `base` là **nhãn PA** (D2-83): core chỉ kế toán byte; ảnh cấp vùng RAM thật từ arena boot
/// (`bootmem`) và **không** để core đọc/ghi vùng đó. Idempotent (ảnh test gọi một lần).
pub fn init(base: u64, bytes: usize, harts: usize) -> CoreReport {
    let _guard: IrqMcsGuard<'_, ()> = LOCKED.lock_irqsave();
    debug_assert!(bytes > 0, "core_bridge: Untyped boot phải có cỡ dương");
    // SAFETY: static `.bss` (`MaybeUninit`); `write_bytes` ghi 0 toàn bộ **không đọc** giá trị cũ,
    // rồi `init_zeroed` đặt các sentinel khác 0. `Machine` không có `Drop`/con trỏ nên zero là
    // bit-valid. Khoá đang giữ ⇒ không ai đọc chen giữa.
    let raw = core::ptr::addr_of_mut!(MACHINE).cast::<Machine>();
    unsafe { core::ptr::write_bytes(raw, 0, 1) };
    let m = unsafe { &mut *raw };
    // D2-88(a): `Machine` chạy **số hart nền tảng thật** (`-smp` của QEMU do `smp` mang lên),
    // không hardcode 1. Ảnh truyền số **đo được** vào, không đọc hằng trang trí.
    let harts = harts.clamp(1, vifast_core::MAX_HARTS);
    HART_COUNT.store(harts as u64, Ordering::Release);
    m.init_zeroed(u16::try_from(harts).unwrap_or(1));
    MACHINE_INIT.store(true, Ordering::Release);
    let untyped = m.add_untyped(base, bytes);
    let check_caps = m.check_caps().is_ok();
    let check_bytes = m.check_bytes(untyped).is_ok();
    let check_all_bytes = m.check_all_bytes().is_ok();
    CoreReport {
        addr: machine_addr(),
        static_bytes: CORE_STATIC_BYTES,
        pin: CORE_PIN_BYTES,
        base,
        arena_bytes: bytes,
        untyped,
        harts,
        objects: m.object_count(),
        caps: m.live_cap_count(),
        check_caps,
        check_bytes,
        check_all_bytes,
        in_image: in_image_region(machine_addr(), CORE_STATIC_BYTES),
    }
}

/// Số liệu **bằng chứng** của khoá (in vào `# core lock:` của TAP — không phải trang trí).
#[derive(Clone, Copy, Debug)]
pub struct LockReport {
    /// Số lần lấy khoá thành công.
    pub acquired: u32,
    /// Số lần phải chờ.
    pub contended: u32,
    /// Số người **đang** trong vùng găng (phải `0` khi đọc).
    pub in_cs: u32,
    /// Số lần chồng lấn (phải `0`).
    pub overlap: u32,
}

impl LockReport {
    /// `true` ⇔ khoá đã được dùng **và** chưa từng chồng lấn **và** hiện rảnh.
    pub fn ok(&self) -> bool {
        self.acquired > 0 && self.overlap == 0 && self.in_cs == 0
    }
}

/// Đọc số liệu khoá của [`LOCKED`].
pub fn lock_stats() -> LockReport {
    LockReport {
        acquired: LOCKED.acquired(),
        contended: LOCKED.contended(),
        in_cs: LOCKED.in_cs(),
        overlap: LOCKED.overlap(),
    }
}

/// `true` ⇔ khoá đã dùng ít nhất một lần và chưa từng chồng lấn.
pub fn lock_ok() -> bool {
    LOCKED.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin D2-84 và trần ảnh link là **hai** ngưỡng khác nhau — không được đánh tráo.
    #[test]
    fn pin_va_tran_anh_khac_nhau() {
        assert_eq!(CORE_PIN_BYTES, 196_608);
        assert_eq!(IMAGE_CAP_BYTES, 262_144);
        assert!(CORE_PIN_BYTES < IMAGE_CAP_BYTES);
        assert!(CORE_STATIC_BYTES <= CORE_PIN_BYTES);
    }

    /// `in_image_region` fail-closed khi chưa công bố biên — không được "đoán ảnh".
    #[test]
    fn image_region_fail_closed() {
        // Không gọi `set_image_bounds` trong test này (trạng thái mặc định `0`).
        assert!(!in_image_region(0x10_0000, 8));
        assert!(!in_image_region(0, 0));
    }
}
