// SPDX-License-Identifier: MIT OR Apache-2.0
//! Khoá COM1 — **một dòng log không bị hart khác xen ký tự** (KT-316); từ **KT-336a** dùng
//! `vifast_sync::McsLock` **bọc lưu/tắt ngắt** ([`crate::irq_lock`]) thay spinlock CAS tự chế.
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
//! # Vì sao `cli` **trước** khi lấy khoá (D2-238)
//!
//! Có **ba** đường in trong ngắt: `# exc:` (handler exception), `# smp: ipi recv=` (handler IPI),
//! `# sched: hart=<h> picked=` (handler timer của hart phụ). Nếu không tắt ngắt: hart đang giữ
//! khoá bị ngắt **trên chính nó**, handler in ⇒ handler quay lại [`ComLock::lock`] trên khoá
//! **của chính hart ấy** ⇒ **tự deadlock**. Vì vậy mọi lần lấy khoá đi qua
//! [`crate::irq_lock::IrqMcsLock::lock_irqsave`]: lưu `RFLAGS.IF`, `cli`, rồi mới chờ MCS; `Drop`
//! nhả khoá rồi khôi phục `IF` **theo giá trị đã lưu** — không `sti` mù (gọi từ handler đã có
//! `IF = 0` thì nhả khoá **không** được bật ngắt giữa handler).
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
//! - `no-com-cli` (feature của **crate này**): giữ khoá MCS nhưng **không** `cli`/`sti` ⇒ mô
//!   phỏng "khoá mà không tắt ngắt"; hazard tự deadlock như trên.
//! - **KT-336a** — `mut-no-irq` / `mut-no-lock` của [`crate::irq_lock`] cũng đi qua khoá này;
//!   TAP 34 đo trực tiếp phần ngắt + loại trừ hai ngữ cảnh.

use crate::irq_lock::{IrqMcsGuard, IrqMcsLock};

/// Khoá quay vòng MCS (FIFO) **bọc lưu/tắt ngắt** + bộ đếm bằng chứng.
///
/// Từ KT-336a, phần loại trừ do `vifast_sync::McsLock` đảm nhiệm (hàng đợi FIFO, chờ trên cache
/// line của node mình); phần `cli`/`sti` do [`IrqMcsLock`]. Bộ đếm `acquired`/`contended`/
/// `in_cs`/`overlap` **giữ nguyên** nghĩa của KT-316 nên TAP 28 và các ca âm cũ không đổi.
pub struct ComLock {
    inner: IrqMcsLock<()>,
}

impl ComLock {
    /// Khoá mới, chưa ai giữ.
    pub const fn new() -> Self {
        Self {
            inner: IrqMcsLock::new(()),
        }
    }

    /// Lấy khoá: `lưu IF → cli → MCS lock` (FIFO); `Drop` của guard nhả MCS rồi khôi phục `IF`.
    pub fn lock(&self) -> ComGuard<'_> {
        ComGuard {
            inner: self.inner.lock_irqsave(),
        }
    }

    /// Số lần lấy khoá thành công.
    pub fn acquired(&self) -> u32 {
        self.inner.acquired()
    }

    /// Số lần phải chờ khoá.
    pub fn contended(&self) -> u32 {
        self.inner.contended()
    }

    /// Số lần phát hiện chồng lấn (phải luôn `0`).
    pub fn overlap(&self) -> u32 {
        self.inner.overlap()
    }

    /// `true` ⇔ khoá **đã được dùng** ít nhất một lần **và** chưa từng chồng lấn.
    ///
    /// Điều kiện "đã dùng" là **cố ý**: một khoá không bao giờ được gọi (đột biến `no-com-lock`)
    /// không được tự khen `ok`.
    pub fn ok(&self) -> bool {
        self.inner.ok()
    }
}

impl Default for ComLock {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard của [`ComLock`] — nhả khoá + khôi phục `IF` khi `drop` (do [`IrqMcsGuard`]).
pub struct ComGuard<'a> {
    /// Guard MCS + trạng thái `IF`; `Drop` tự nhả khoá rồi khôi phục ngắt.
    #[allow(dead_code)]
    inner: IrqMcsGuard<'a, ()>,
}

/// Khoá **duy nhất** của COM1 trong ảnh — mọi đường in dùng chung một khoá.
pub static COM1: ComLock = ComLock::new();
