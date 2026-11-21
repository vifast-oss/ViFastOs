// SPDX-License-Identifier: MIT OR Apache-2.0
//! Console COM1 của ảnh x86_64 + khoá **một dòng** (KT-316).
//!
//! Tách khỏi `main.rs` (luật R1: tệp trong `kernel/` ≤ 1.500 dòng) nhưng vẫn là **một** đường
//! ghi: `Console` (có khoá) → [`RawConsole`] (không khoá) → `crate::write_byte` (một byte).
//!
//! # Vì sao khoá ở `write_fmt`, không phải `write_str`
//!
//! `writeln!` mở ra `write_fmt`, và impl mặc định của `core::fmt::Write` gọi `write_str` cho
//! **từng mảnh literal + từng tham số** (đo được: `writeln!("# sched: ticks={ticks} tasks=3")` =
//! **3** lời gọi `write_str`). Khoá từng `write_str` vẫn để hart khác xen ký tự ở **ranh giới
//! mảnh** — đúng lớp lỗi soak vòng 21 (`# exc:`/`PANIC` bị cắt ghép). Vì vậy [`Console`] override
//! `write_fmt` để giữ khoá **suốt một dòng**, còn [`Console::with_lock`] dành cho dòng ghép từ
//! nhiều `write!` (ví dụ `write_hex_list` của `smp.rs`).
//!
//! # Vì sao `cli` nằm trong khoá
//!
//! Có **ba** đường in trong ngắt (`# exc:`, `# smp: ipi recv=`, `# sched: hart=<h> picked=`) và
//! một đường in từng byte trong handler `int 0x80` (`print_user`). Không `cli` thì hart đang giữ
//! khoá bị ngắt **trên chính nó**, handler in ⇒ tự deadlock. `vifast_plat_x86_64::com` làm việc
//! ấy và khôi phục `IF` theo giá trị đã lưu (không `sti` mù trong handler).

use core::fmt::{self, Write};

use vifast_plat_x86_64::com;

use crate::{check, write_byte};

/// `core::fmt::Write` trên COM1 — để `panic!`/`write!` in được số mà không cần `alloc`.
///
/// **KT-316** — mọi lời gọi `write_fmt` (một `writeln!`/`write!` = **một dòng**) giữ khoá COM1
/// **suốt** lời gọi, nên hart khác không thể xen ký tự vào giữa dòng.
pub(crate) struct Console;

/// Đường ghi **không khoá** — chỉ dùng khi caller **đã** giữ [`com::COM1`] (qua
/// [`Console::with_lock`] hoặc [`com_guard`]).
pub(crate) struct RawConsole;

impl Write for RawConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            write_byte(byte);
        }
        Ok(())
    }
}

impl Console {
    /// Giữ khoá COM1 cho **cả một dòng** rồi ghi qua [`RawConsole`].
    ///
    /// Dùng cho dòng ghép từ **nhiều** `write!` (ví dụ `write_hex_list` của `smp.rs`): nếu mỗi
    /// `write!` tự lấy khoá thì hart khác chen được vào giữa các mảnh.
    pub(crate) fn with_lock<R>(&mut self, f: impl FnOnce(&mut RawConsole) -> R) -> R {
        let _guard = com_guard();
        f(&mut RawConsole)
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.with_lock(|raw| raw.write_str(s))
    }

    /// **KT-316** — khoá **cả** lời gọi định dạng: đây là chỗ duy nhất biến "một dòng" thành
    /// nguyên tử, vì `writeln!` gọi `write_str` nhiều lần.
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.with_lock(|raw| fmt::write(raw, args))
    }
}

/// Lấy khoá COM1 — `None` khi đột biến `no-com-lock` (bỏ khoá để chứng minh TAP 28 có răng).
///
/// Trả `Option<ComGuard>` thay vì luôn `ComGuard` để đột biến **không** phải sửa từng chỗ gọi.
pub(crate) fn com_guard() -> Option<com::ComGuard<'static>> {
    if cfg!(feature = "no-com-lock") {
        None
    } else {
        Some(com::COM1.lock())
    }
}

/// **KT-316** — in bằng chứng khoá COM1 **một lần** ở cuối boot + TAP 28.
///
/// `ok` đòi khoá **đã được dùng** (`acquired > 0`) và **chưa từng** có hai người trong vùng găng
/// (`overlap == 0`). `contended` là số lần phải chờ — **số đo**, không phải điều kiện (boot yên
/// tĩnh có thể `0`).
pub(crate) fn com_report(console: &mut Console, tap: usize) {
    let ok = com::COM1.ok();
    let contended = com::COM1.contended();
    let _ = writeln!(console, "# com: lock ok={} contended={contended}", ok as u8);
    check(console, tap, "com: lock", ok);
}
