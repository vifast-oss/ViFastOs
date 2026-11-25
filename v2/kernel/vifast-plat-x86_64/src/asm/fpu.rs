// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `asm/fpu.rs` — **ngữ cảnh FPU/SIMD**: truy cập `CR0` (bit `MP`/`EM`/`TS` — bật/tắt lazy
//! switch) và `CR4` (`OSFXSR`/`OSXMMEXCPT` — bật `fxsave`/`xmm`). Vùng lưu **theo TCB**
//! (`fxsave`/`xsave`, KT-418) dựng trên các hàm này; tách cơ học từ `asm.rs`.
//! Stub host (`target_os != "none"`) panic to — không giả lập asm đặc quyền.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use core::arch::asm;

// `super::*` chỉ cần cho **stub host** (`host_only`); trên target x86_64 mọi stub bị
// `cfg`-gate ra nên glob import là thừa (clippy `unused_imports`).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use super::*;

/// Đọc `CR0`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr0() -> u64 {
    let value: u64;
    // SAFETY: đọc thanh ghi điều khiển ở CPL0; không đụng bộ nhớ.
    unsafe {
        asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Ghi `CR0` — **KT-340a**: bật `CR0.WP` (bit 16) để ring 0 **không** ghi được trang chỉ-đọc.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_cr0(value: u64) {
    // SAFETY: `mov cr0, r64` ở CPL0; giữ nguyên các bit khác do caller quyết định.
    unsafe {
        asm!("mov cr0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

/// Đọc `CR4`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr4() -> u64 {
    let value: u64;
    // SAFETY: như `read_cr0`.
    unsafe {
        asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Stub host của [`read_cr0`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr0() -> u64 {
    host_only("read_cr0")
}

/// Stub host của [`read_cr4`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr4() -> u64 {
    host_only("read_cr4")
}

/// Stub host của [`write_cr0`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_cr0(_value: u64) {
    host_only("write_cr0")
}

// =============================================================================================
// **KT-418** — ngữ cảnh FPU/SIMD **eager theo TCB**: `fxsave64`/`fxrstor64` + vùng lưu 528 B
// **ngoài** `TcbCore`.
//
// # Vì sao `fxsave64` (không `xsave`)
//
// `xsave` cần `XCR0` + `CPUID` và ghi vùng **biến cỡ** (đến 2.6 KiB với AVX-512); `fxsave64` ghi
// **đúng 512 B** cố định (FCW/FSW/FTW/FOP/FIP/FDP + MXCSR + `st0..st7` + `xmm0..xmm15`) — đủ cho
// "FPU/SIMD" của KT-418 và vừa **một** cỡ vùng lưu chung 3 arch (528 B). `xsave` để dành cho
// task mở AVX (ngoài phạm vi, ghi nợ).
//
// # Theo dõi bẩn
//
// x86_64 **không** có bit "bẩn" phần cứng tương đương `sstatus.FS` của riscv64 (chỉ có `CR0.TS`
// cho lazy switch). Vì vậy đường này **eager thuần**: lưu mỗi lần tác vụ dừng, khôi phục mỗi lần
// vào ring 3 — đúng chữ "eager" của KT-418, không có đường "quên lưu".
//
// # Bố cục
//
// `fxsave64` ghi 512 B đầu của `FpuArea` (528 B, căn 16 — `fxsave64` đòi 16 B); 16 B cuối để
// trống cho khớp cỡ chung với riscv64/aarch64.

use core::sync::atomic::{AtomicU32, Ordering};

use vifast_plat_common::fpu::{FpuArea, FpuTracker, FPU_SLOTS};

/// `CR0.MP` (bit 1) — giám sát lệnh FP chờ.
pub const CR0_MP: u64 = 1 << 1;
/// `CR0.EM` (bit 2) — **giả lập** FP: phải 0 thì `fxsave64` mới chạy thật.
pub const CR0_EM: u64 = 1 << 2;
/// `CR0.TS` (bit 3) — task switched (lazy switch; KT-418 **không** dùng, eager).
pub const CR0_TS: u64 = 1 << 3;
/// `CR4.OSFXSR` (bit 9) — bật `fxsave`/`fxrstor` + `xmm`.
pub const CR4_OSFXSR: u64 = 1 << 9;
/// `CR4.OSXMMEXCPT` (bit 10) — `#XF` cho ngoại lệ SIMD thay vì `#UD`.
pub const CR4_OSXMMEXCPT: u64 = 1 << 10;

/// Bảng vùng lưu theo khe — `UnsafeCell` vì asm ghi trực tiếp vào bộ nhớ.
struct Areas(core::cell::UnsafeCell<[FpuArea; FPU_SLOTS]>);

// SAFETY: một TCB chạy trên **một** hart tại một thời điểm ⇒ hai hart không chạm cùng một khe.
unsafe impl Sync for Areas {}

/// Vùng lưu FPU/SIMD của mọi TCB — **ngoài** `TcbCore` (528 B × 65 = 34.320 B `.bss`).
static AREAS: Areas = Areas(core::cell::UnsafeCell::new([FpuArea::ZERO; FPU_SLOTS]));

/// Sổ khe theo TCB (bookkeeping thuần, test host ở `vifast-plat-common/tests/fpu.rs`).
static TRACKER: FpuTracker<FPU_SLOTS> = FpuTracker::new();

/// TCB đang chạy — `u32::MAX` = chưa gắn (đường kernel thuần).
static CURRENT_TCB: AtomicU32 = AtomicU32::new(u32::MAX);

/// Gắn TCB đang chạy (gọi trước `enter_user`).
pub fn bind_current(tcb: u32) {
    CURRENT_TCB.store(tcb, Ordering::Release);
}

/// TCB đang chạy (`None` = chưa gắn).
pub fn current_tcb() -> Option<u32> {
    let value = CURRENT_TCB.load(Ordering::Acquire);
    if value == u32::MAX {
        None
    } else {
        Some(value)
    }
}

/// Con trỏ tới vùng lưu của `slot`.
fn area_ptr(slot: usize) -> *mut FpuArea {
    AREAS.0.get().cast::<FpuArea>().wrapping_add(slot)
}

/// Bật FP/SIMD: `CR0.EM = 0`, `CR0.MP = 1`, `CR4.OSFXSR = 1`, `CR4.OSXMMEXCPT = 1`.
///
/// Idempotent — gọi lại không đổi gì. Trả `(cr0, cr4)` **sau** khi bật (bằng chứng TAP).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn enable_fpu() -> (u64, u64) {
    let cr0 = (read_cr0() | CR0_MP) & !(CR0_EM | CR0_TS);
    write_cr0(cr0);
    let cr4 = read_cr4() | CR4_OSFXSR | CR4_OSXMMEXCPT;
    // SAFETY: `mov cr4, r64` ở CPL0; chỉ bật hai bit OSFXSR/OSXMMEXCPT.
    unsafe {
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    (cr0, cr4)
}

/// Lưu 512 B ảnh `fxsave64` vào `area`.
///
/// # Safety
///
/// `area` phải trỏ tới vùng ≥ 512 B **căn 16** và ghi được.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub unsafe fn save_raw(area: *mut FpuArea) {
    // SAFETY: caller bảo đảm `area` hợp lệ + căn 16 (`FpuArea` có `align(16)`).
    unsafe {
        asm!("fxsave64 [{}]", in(reg) area, options(nostack));
    }
}

/// Khôi phục 512 B ảnh `fxsave64` từ `area`.
///
/// # Safety
///
/// `area` phải trỏ tới vùng ≥ 512 B **căn 16**, đọc được, đã ghi bởi [`save_raw`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub unsafe fn restore_raw(area: *const FpuArea) {
    // SAFETY: caller bảo đảm `area` hợp lệ + căn 16.
    unsafe {
        asm!("fxrstor64 [{}]", in(reg) area, options(nostack));
    }
}

/// **Vào ring 3**: khôi phục ngữ cảnh FPU/SIMD của TCB đang chạy (hoặc trạng thái **mới**).
pub fn restore_current() {
    let Some(tcb) = current_tcb() else {
        return;
    };
    if TRACKER.is_valid(tcb) {
        if let Some(slot) = TRACKER.slot_of(tcb) {
            // SAFETY: `slot` do tracker cấp cho đúng `tcb`; vùng đã được `save_raw` ghi.
            unsafe { restore_raw(area_ptr(slot)) };
            TRACKER.record_restore();
            return;
        }
    }
    TRACKER.record_fresh();
}

/// **Ra khỏi ring 3**: lưu ngữ cảnh FPU/SIMD của TCB đang chạy (eager — x86_64 không có bit bẩn).
pub fn save_current() {
    let Some(tcb) = current_tcb() else {
        TRACKER.record_skip();
        return;
    };
    let Some(slot) = TRACKER.slot_for(tcb) else {
        // Tràn khe ⇒ fail-closed: không ghi đè vùng của TCB khác.
        return;
    };
    // SAFETY: `slot` vừa được cấp cho `tcb`; không hart nào khác chạy cùng TCB này.
    unsafe { save_raw(area_ptr(slot)) };
    TRACKER.mark_valid(tcb);
    TRACKER.record_save();
}

/// Ảnh chụp bộ đếm FPU — **bằng chứng** cho TAP/journal.
pub fn stats() -> vifast_plat_common::fpu::FpuStats {
    TRACKER.stats()
}

/// Khe của `tcb` (bằng chứng: hai TCB phải hai khe khác nhau).
pub fn slot_of(tcb: u32) -> Option<usize> {
    TRACKER.slot_of(tcb)
}

/// `true` ⇔ `tcb` đã có vùng lưu hợp lệ.
pub fn is_valid(tcb: u32) -> bool {
    TRACKER.is_valid(tcb)
}

// ---------------------------------------------------------------- stub host (không asm đặc quyền)

/// Stub host của [`enable_fpu`] — **panic**: trên host không có `CR0`/`CR4` của khách.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn enable_fpu() -> (u64, u64) {
    host_only("enable_fpu")
}

/// Stub host của [`save_raw`].
///
/// # Safety
///
/// Cùng hợp đồng với [`save_raw`] (con trỏ tới vùng `FpuArea` hợp lệ); stub host **panic** nên
/// không dereference gì.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub unsafe fn save_raw(_area: *mut FpuArea) {
    host_only("save_raw")
}

/// Stub host của [`restore_raw`].
///
/// # Safety
///
/// Cùng hợp đồng với [`restore_raw`] (con trỏ tới vùng `FpuArea` hợp lệ); stub host **panic** nên
/// không dereference gì.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub unsafe fn restore_raw(_area: *const FpuArea) {
    host_only("restore_raw")
}

// =============================================================================================
// **KT-418** — cứng hóa **SMEP**/**SMAP** (ưu tiên 2 của task): bật `CR4.SMEP`/`CR4.SMAP` **chỉ
// khi** `CPUID.07H:EBX` báo CPU có, rồi đọc lại `CR4` làm bằng chứng.
//
// # Vì sao phải kiểm `CPUID` trước (fail-safe, không đoán)
//
// Ghi `CR4` với bit CPU **không** hỗ trợ ⇒ `#GP` ngay lệnh `mov cr4` ⇒ boot chết. Model mặc định
// của runner là `qemu64` (KT-323 giữ nguyên để số đo cũ tái lập được), nên không được giả định
// SMEP/SMAP có mặt. Hàm trả **cả** "CPU có" và "đã bật" để TAP phân biệt "không hỗ trợ" với
// "hỗ trợ nhưng bật hỏng".
// =============================================================================================

/// `CR4.SMEP` (bit 20) — ring 0 **không** thực thi được trang `US=1`.
pub const CR4_SMEP: u64 = 1 << 20;
/// `CR4.SMAP` (bit 21) — ring 0 **không** đọc/ghi được trang `US=1` khi `RFLAGS.AC = 0`.
pub const CR4_SMAP: u64 = 1 << 21;
/// `CPUID.07H:EBX[7]` — CPU có SMEP.
pub const CPUID_7_EBX_SMEP: u32 = 1 << 7;
/// `CPUID.07H:EBX[20]` — CPU có SMAP.
pub const CPUID_7_EBX_SMAP: u32 = 1 << 20;

/// Kết quả bật SMEP/SMAP — **bằng chứng** cho TAP/journal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmepSmapReport {
    /// `CPUID.07H:EBX[7]` — CPU **có** SMEP.
    pub cpuid_smep: bool,
    /// `CPUID.07H:EBX[20]` — CPU **có** SMAP.
    pub cpuid_smap: bool,
    /// `CR4.SMEP` **sau** khi bật (đọc lại từ thanh ghi).
    pub smep: bool,
    /// `CR4.SMAP` **sau** khi bật (đọc lại từ thanh ghi).
    pub smap: bool,
    /// `CR4` đọc lại sau khi bật.
    pub cr4: u64,
}

impl SmepSmapReport {
    /// `true` ⇔ **SMEP** (bit CPU có) đã bật **thật** (đọc lại từ `CR4`).
    ///
    /// **SMAP chưa nằm trong điều kiện này** — xem [`Self::smap_deferred`].
    pub const fn hardened(&self) -> bool {
        !self.cpuid_smep || self.smep
    }

    /// `true` ⇔ CPU **có** SMAP nhưng kernel **chưa** bật (nợ có tên, xem [`enable_smep_smap`]).
    pub const fn smap_deferred(&self) -> bool {
        self.cpuid_smap && !self.smap
    }
}

/// Bật **`CR4.SMEP`** khi `CPUID` báo có; đọc lại `CR4` và trả bằng chứng.
///
/// # Vì sao **chưa** bật `CR4.SMAP` (nợ có tên, đo được)
///
/// Kernel hiện **đọc/ghi trang `US=1` từ CPL0** ở nhiều đường hợp lệ: `umode::run` đọc
/// `VSPACE_PROBE_VA` cho phép đo `INVLPG`, loader ELF ghi trang user, IPC buffer đọc/ghi vùng
/// user. Bật `SMAP` mà không bọc `stac`/`clac` quanh **mọi** đường đó thì boot `-cpu max` **chết**
/// ngay phép đo `INVLPG` — đo được: `# exc: vec=14 err=0x1 rip=0x110a14` (đọc
/// `0x1000004000`) rồi halt. Vì vậy KT-418 bật **SMEP** (kernel **không** bao giờ thực thi trang
/// user ⇒ an toàn tuyệt đối) và **giữ SMAP tắt**, ghi nợ: bọc `stac`/`clac` cho các đường chạm
/// trang user rồi mới bật `CR4.SMAP`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn enable_smep_smap() -> SmepSmapReport {
    let leaf7 = super::cpuid_leaf7_ebx();
    let cpuid_smep = leaf7
        .map(|ebx| ebx & CPUID_7_EBX_SMEP != 0)
        .unwrap_or(false);
    let cpuid_smap = leaf7
        .map(|ebx| ebx & CPUID_7_EBX_SMAP != 0)
        .unwrap_or(false);
    if cpuid_smep {
        let cr4 = read_cr4() | CR4_SMEP;
        // SAFETY: `mov cr4, r64` ở CPL0; chỉ bật bit mà `CPUID` vừa xác nhận là có.
        unsafe {
            asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
        }
    }
    let cr4 = read_cr4();
    SmepSmapReport {
        cpuid_smep,
        cpuid_smap,
        smep: cr4 & CR4_SMEP != 0,
        smap: cr4 & CR4_SMAP != 0,
        cr4,
    }
}

/// Stub host của [`enable_smep_smap`] — **panic**: trên host không có `CR4` của khách.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn enable_smep_smap() -> SmepSmapReport {
    host_only("enable_smep_smap")
}
