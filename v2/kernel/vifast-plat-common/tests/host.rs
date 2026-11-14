// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-common` (KT-44).
//!
//! D2-72 Q12: Miri **không** áp cho kernel `no_std`/asm/MMIO, nhưng phần **logic** của
//! `vifast-plat-common` đã tách khỏi asm nên chạy được trên host — tệp này là bằng chứng:
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```

use std::cell::RefCell;
use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};

use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};

/// Nền tảng giả ghi lại mọi lần in/dừng máy.
#[derive(Default)]
struct Recorder {
    out: RefCell<String>,
    halt_code: RefCell<Option<usize>>,
}

impl Platform for Recorder {
    fn put_str(&self, s: &str) {
        self.out.borrow_mut().push_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        *self.halt_code.borrow_mut() = Some(code);
        // Cùng hợp đồng với OpenSBI SRST: không quay lại. Ở host, "không quay lại" = panic.
        panic!("halt({code})");
    }

    fn now(&self) -> u64 {
        42
    }

    fn tick_hz(&self) -> u64 {
        10_000_000
    }

    fn hart_id(&self) -> usize {
        3
    }
}

#[test]
fn platform_writer_in_dung_thu_tu() {
    let r = Recorder::default();
    let mut w = PlatformWriter(&r);
    write!(w, "hart={} now={}", r.hart_id(), r.now()).unwrap();
    assert_eq!(r.out.borrow().as_str(), "hart=3 now=42");
}

#[test]
fn panic_handler_in_vi_tri_va_halt_1() {
    let r = Recorder::default();
    let handler = PanicHandler::new(&r);
    let caught = catch_unwind(AssertUnwindSafe(|| {
        handler.handle_with(Some(("kt44.rs", 7, 3)), format_args!("hỏng {}", 9))
    }));
    assert!(caught.is_err(), "halt phải panic ở host");
    let out = r.out.borrow();
    assert!(out.starts_with("PANIC at kt44.rs:7:3\n"), "out = {out:?}");
    assert!(out.contains("PANIC: hỏng 9\n"), "out = {out:?}");
    assert_eq!(*r.halt_code.borrow(), Some(1));
}

#[test]
fn panic_handler_khong_vi_tri_van_halt_1() {
    let r = Recorder::default();
    let handler = PanicHandler::new(&r);
    let caught = catch_unwind(AssertUnwindSafe(|| {
        handler.handle_with(None, format_args!("không vị trí"))
    }));
    assert!(caught.is_err());
    assert_eq!(r.out.borrow().as_str(), "PANIC\nPANIC: không vị trí\n");
    assert_eq!(*r.halt_code.borrow(), Some(1));
}
