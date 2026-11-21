// SPDX-License-Identifier: MIT OR Apache-2.0
//! Khoá COM1 — **một dòng log không bị hart khác xen ký tự** (KT-316).
//!
//! # Vì sao khoá **một dòng**, không phải một byte
//!
//! Đường ghi COM1 của ảnh x86_64 là **từng byte** (`Console::write_str` lặp `write_byte`), và
//! **không** có khái niệm "dòng" ở tầng nào. Một `writeln!` mở ra `write_fmt`, mà impl mặc định
//! của `core::fmt::Write` gọi `write_str` cho **từng mảnh literal và từng tham số** (đo được:
//! `writeln!("# sched: ticks={ticks} tasks=3")` = **3** lời gọi `write_str`). Vì vậy:
//!
//! - khoá **mỗi byte** ⇒ hai hart vẫn xen ký tự ở giữa hai byte;
//! - khoá **mỗi `write_str`** ⇒ hai hart vẫn xen ký tự ở **ranh giới mảnh** (hart A in
//!   `# sched: ticks=`, hart B in `# smp: hart=1 online`, rồi A in `10 tasks=3`);
//! - khoá **cả `write_fmt`** (một `writeln!`/`write!` = một dòng) ⇒ dòng ra **nguyên vẹn**.
//!
//! Ảnh dùng [`ComLock`] ở tầng `write_fmt` (xem `vifast-kbin-x86_64::Console`), cộng một lần
//! khoá cho cả dòng ghép từ nhiều `write!` (`write_hex_list`) và cho đường in từng byte của
//! syscall `DebugPutStr`.
//!
//! # Vì sao `cli` **trước** khi lấy khoá
//!
//! Có **ba** đường in trong ngắt: `# exc:` (handler exception), `# smp: ipi recv=` (handler IPI),
//! `# sched: hart=<h> picked=` (handler timer của hart phụ). Nếu chỉ CAS mà không `cli`: hart
//! đang giữ khoá bị ngắt **trên chính nó**, handler in ⇒ handler quay lại [`ComLock::lock`] trên
//! khoá **của chính hart ấy** ⇒ **tự deadlock**. Vì vậy [`ComLock::lock`] `cli` trước, và
//! [`ComGuard`] khôi phục `RFLAGS.IF` **theo giá trị đã lưu** sau khi nhả — không `sti` mù (gọi
//! từ handler đã có `IF = 0` thì nhả khoá **không** được bật ngắt giữa handler).
//!
//! # Bất biến đo được
//!
//! [`ComLock::acquired`] đếm số lần lấy khoá, [`ComLock::contended`] đếm số lần **phải chờ**,
//! [`ComLock::overlap`] đếm số lần phát hiện **hai** người trong vùng găng (phải luôn `0`).
//! [`ComLock::ok`] = "đã dùng khoá **và** chưa từng chồng" — ảnh in `# com: lock ok=…` + TAP 28.
//!
//! # Đột biến (chỉ để chứng minh bộ kiểm có răng)
//!
//! - `no-com-lock` (feature của **ảnh**): bỏ hẳn khoá ⇒ `acquired = 0` ⇒ TAP 28 ĐỎ.
//! - `no-com-cli` (feature của **crate này**): giữ CAS nhưng **không** `cli`/`sti` ⇒ mô phỏng
//!   "khoá mà không tắt ngắt"; hazard tự deadlock như trên.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::asm;

/// Vào vùng găng: trả `RFLAGS.IF` **trước** khi tắt ngắt.
///
/// Trên host (test `cargo test`) là no-op: `cli`/`pushfq` không tồn tại, và phần **loại trừ**
/// (CAS) vẫn được đo thật bằng nhiều luồng.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
fn enter_critical() -> bool {
    if cfg!(feature = "no-com-cli") {
        return false;
    }
    let was_enabled = asm::interrupts_enabled();
    asm::cli();
    was_enabled
}

/// Ra khỏi vùng găng: bật lại ngắt **chỉ khi** trước đó đang bật.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
fn leave_critical(was_enabled: bool) {
    if cfg!(feature = "no-com-cli") {
        return;
    }
    if was_enabled {
        asm::sti();
    }
}

/// Host: không có `cli`/`sti`; khoá vẫn loại trừ bằng CAS.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
fn enter_critical() -> bool {
    false
}

/// Host: không có `sti`.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
fn leave_critical(_was_enabled: bool) {}

/// Khoá quay vòng bằng **một** `AtomicBool` (CAS) + bộ đếm bằng chứng.
///
/// Không phải mutex công bằng: thứ tự giành khoá không bảo đảm — chỉ bảo đảm **loại trừ**.
/// Vùng găng là **một dòng log** (vài chục byte tới vài trăm byte), nên quay vòng là đủ và
/// không cần hàng đợi.
pub struct ComLock {
    /// `true` ⇔ có hart đang trong vùng găng.
    locked: AtomicBool,
    /// Số lần lấy khoá **thành công** (bằng chứng khoá được dùng, không phải trang trí).
    acquired: AtomicU32,
    /// Số lần phải **chờ** (CAS đầu tiên thất bại) — số liệu `contended=` của `# com: lock`.
    contended: AtomicU32,
    /// Số lần phát hiện **hai** người trong vùng găng — phải luôn `0`.
    overlap: AtomicU32,
    /// Số người đang trong vùng găng (chỉ để phát hiện chồng lấn).
    in_cs: AtomicU32,
}

impl ComLock {
    /// Khoá mới, chưa ai giữ.
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            acquired: AtomicU32::new(0),
            contended: AtomicU32::new(0),
            overlap: AtomicU32::new(0),
            in_cs: AtomicU32::new(0),
        }
    }

    /// Lấy khoá: `cli` → CAS (quay vòng tới khi được) → đếm. Trả guard; nhả khi guard `drop`.
    ///
    /// `Ordering::Acquire`/`Release` là đủ: vùng găng chỉ bảo vệ **thứ tự byte ra UART**, không
    /// bảo vệ dữ liệu chia sẻ nào khác.
    pub fn lock(&self) -> ComGuard<'_> {
        let was_enabled = enter_critical();
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.contended.fetch_add(1, Ordering::Relaxed);
            while self
                .locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                core::hint::spin_loop();
            }
        }
        if self.in_cs.fetch_add(1, Ordering::AcqRel) != 0 {
            self.overlap.fetch_add(1, Ordering::Relaxed);
        }
        self.acquired.fetch_add(1, Ordering::Relaxed);
        ComGuard {
            lock: self,
            was_enabled,
        }
    }

    /// Số lần lấy khoá thành công.
    pub fn acquired(&self) -> u32 {
        self.acquired.load(Ordering::Relaxed)
    }

    /// Số lần phải chờ khoá.
    pub fn contended(&self) -> u32 {
        self.contended.load(Ordering::Relaxed)
    }

    /// Số lần phát hiện chồng lấn (phải luôn `0`).
    pub fn overlap(&self) -> u32 {
        self.overlap.load(Ordering::Relaxed)
    }

    /// `true` ⇔ khoá **đã được dùng** ít nhất một lần **và** chưa từng chồng lấn.
    ///
    /// Điều kiện "đã dùng" là **cố ý**: một khoá không bao giờ được gọi (đột biến `no-com-lock`)
    /// không được tự khen `ok`.
    pub fn ok(&self) -> bool {
        self.acquired() > 0 && self.overlap() == 0
    }
}

impl Default for ComLock {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard của [`ComLock`] — nhả khoá + khôi phục `IF` khi `drop`.
pub struct ComGuard<'a> {
    lock: &'a ComLock,
    was_enabled: bool,
}

impl Drop for ComGuard<'_> {
    fn drop(&mut self) {
        self.lock.in_cs.fetch_sub(1, Ordering::AcqRel);
        self.lock.locked.store(false, Ordering::Release);
        leave_critical(self.was_enabled);
    }
}

/// Khoá **duy nhất** của COM1 trong ảnh — mọi đường in dùng chung một khoá.
pub static COM1: ComLock = ComLock::new();
