// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `asm/smap.rs` — **KT-418b**: `stac`/`clac` + **guard** cho những đường kernel **cố ý** đọc/ghi
//! trang `US=1` từ CPL0.
//!
//! # Vì sao cần
//!
//! `CR4.SMAP` (Supervisor Mode Access Prevention) làm **mọi** truy cập dữ liệu của CPL0 vào trang
//! có `PTE.U/S = 1` thành `#PF` — trừ khi `RFLAGS.AC = 1`. Đó chính là thứ ta muốn: một con trỏ
//! user bị lẫn vào đường kernel (hoặc một `memcpy` độ dài sai) **không** thể đọc/ghi im lặng bộ
//! nhớ của tác vụ. Nhưng kernel có **ba** đường **hợp lệ** phải chạm trang user:
//!
//! 1. phép đo `INVLPG` của `VSpace` (`umode.rs` đọc `VSPACE_PROBE_VA` để chứng minh TLB đã nạp
//!    lại) — trang đó là `US=1` trong bảng trang đang hoạt động;
//! 2. đọc message/`mr` từ vùng user trong syscall (`syscall_env::read_user_words`);
//! 3. (tương lai) ghi kết quả trả về vào vùng user — hiện chưa có đường nào ghi qua VA user.
//!
//! Mỗi đường bọc bằng [`UserAccess::begin`] — guard **tự** `clac` khi ra khỏi scope, kể cả khi
//! đường đó `return` sớm. Không có `clac` thủ công rải rác: một chỗ quên là `AC=1` rò sang đường
//! khác và SMAP thành vô nghĩa.
//!
//! # Vì sao **không** `preserves_flags`
//!
//! `stac`/`clac` **đổi** `RFLAGS.AC` — khai `preserves_flags` là nói dối compiler (nó có thể dùng
//! lại cờ đã tính trước lời gọi). Vì vậy hai hàm này chỉ `nomem, nostack`.
//!
//! # Stub host
//!
//! Trên host (`target_os != "none"`) hai hàm **panic** như mọi hàm asm đặc quyền khác của crate
//! (`asm.rs`): không ai lỡ "thử SMAP" trên máy thật.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

/// `true` ⇔ `CR4.SMAP` **đã** bật (do [`super::fpu::enable_smep_smap`] ghi).
///
/// # Vì sao phải có cờ này (đo được, không phải phòng xa)
///
/// `stac`/`clac` là lệnh của **SMAP**: trên CPU **không** có SMAP (model `qemu64` của cổng) hai
/// lệnh đó `#UD`. Bản đầu của KT-418b gọi `stac` vô điều kiện ⇒ boot `qemu64` chết ngay phép đo
/// `INVLPG`: `# exc: vec=6 err=0x0 rip=0x110a10 cs=0x8` (`#UD`). Vì vậy guard chỉ `stac`/`clac`
/// khi cờ này bật — trên CPU không có SMAP, guard là **no-op** (không có gì để mở).
static SMAP_ON: AtomicBool = AtomicBool::new(false);

/// Ghi cờ "SMAP đã bật" — gọi **một** chỗ: [`super::fpu::enable_smep_smap`] sau khi ghi `CR4`.
pub fn set_smap_enabled(on: bool) {
    SMAP_ON.store(on, Ordering::Relaxed);
}

/// `true` ⇔ `CR4.SMAP` đã bật ⇒ `stac`/`clac` dùng được.
pub fn smap_enabled() -> bool {
    SMAP_ON.load(Ordering::Relaxed)
}

/// Bật `RFLAGS.AC` — cho phép CPL0 chạm trang `US=1` cho tới `clac` kế tiếp.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn stac() {
    // SAFETY: `stac` ở CPL0; chỉ đổi `RFLAGS.AC`, không đụng bộ nhớ.
    unsafe {
        asm!("stac", options(nomem, nostack));
    }
}

/// Tắt `RFLAGS.AC` — đóng cửa sổ truy cập trang user.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn clac() {
    // SAFETY: như `stac`.
    unsafe {
        asm!("clac", options(nomem, nostack));
    }
}

/// Stub host của [`stac`] — **panic** (không có `RFLAGS` của khách trên host).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn stac() {
    super::host_only("stac")
}

/// Stub host của [`clac`] — **panic**.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn clac() {
    super::host_only("clac")
}

/// Guard mở cửa sổ truy cập trang `US=1`; `Drop` **luôn** `clac`.
///
/// Dùng:
///
/// ```ignore
/// let _user = asm::smap::UserAccess::begin();
/// let word = asm::read_u64(user_va);
/// // ra khỏi scope ⇒ clac
/// ```
///
/// `#[must_use]`: bỏ guard vào `_` là mất `clac` ⇒ `AC=1` rò sang đường sau. Đây là lý do kiểu
/// này tồn tại thay vì hai lời gọi `stac()`/`clac()` rời.
#[must_use = "guard phải được giữ trong scope — bỏ ngay là mất clac"]
pub struct UserAccess {
    /// `true` ⇔ guard **đã** `stac` (chỉ khi SMAP bật) ⇒ `Drop` mới `clac`.
    active: bool,
}

impl UserAccess {
    /// `stac` (nếu SMAP bật) rồi trả guard; `clac` xảy ra khi guard bị drop.
    ///
    /// CPU **không** có SMAP ⇒ no-op: `stac`/`clac` sẽ `#UD`, và cũng không có gì để mở.
    pub fn begin() -> Self {
        let active = smap_enabled();
        if active {
            stac();
        }
        Self { active }
    }
}

impl Drop for UserAccess {
    fn drop(&mut self) {
        if self.active {
            clac();
        }
    }
}
