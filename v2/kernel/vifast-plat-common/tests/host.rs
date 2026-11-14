// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-common` (KT-44, mở rộng KT-45).
//!
//! D2-72 Q12: Miri **không** áp cho kernel `no_std`/asm/MMIO, nhưng phần **logic** của
//! `vifast-plat-common` đã tách khỏi asm nên chạy được trên host — tệp này là bằng chứng:
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! KT-45: decode **mọi** `scause` (0/2/3/4/5/6/7/8/9/12/13/15/khác), bố cục trapframe khớp asm,
//! chính sách resume/fatal, dump `PANIC` + halt(1), stub IRQ của [`Platform`]. Đây là chỗ kiểm
//! `illegal` (scause 2) **không cần phần cứng** — OpenSBI v1.8.1 không delegate bit 2 (D2-73).

use std::cell::RefCell;
use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};

use vifast_plat_common::trap::{
    decode_scause, decode_trap, handle_trap, is_nested_trap_sp, report_fatal, report_interrupt,
    Event, Exception, ExceptionKind, TrapAction, TrapFrame, TrapStats, REG_A0, REG_A7, REG_RA,
    REG_SP, SCAUSE_INTERRUPT, SSTATUS_SPP, TF_REG_COUNT, TF_SCAUSE_OFFSET, TF_SEPC_OFFSET, TF_SIZE,
    TF_SSTATUS_OFFSET, TF_STVAL_OFFSET,
};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};

/// Nền tảng giả ghi lại mọi lần in/dừng máy và mọi lệnh platform KT-45.
#[derive(Default)]
struct Recorder {
    out: RefCell<String>,
    halt_code: RefCell<Option<usize>>,
    trap_vector: RefCell<Option<usize>>,
    irq_log: RefCell<Vec<(u32, bool)>>,
    timer_deadline: RefCell<Option<u64>>,
    timer_enabled: std::cell::Cell<bool>,
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

    fn now_ticks(&self) -> u64 {
        42
    }

    fn timebase_hz(&self) -> u64 {
        10_000_000
    }

    fn hart_id(&self) -> usize {
        3
    }

    fn set_trap_vector(&self, vector: usize) {
        *self.trap_vector.borrow_mut() = Some(vector);
    }

    fn enable_irq(&self, irq: u32) -> bool {
        self.irq_log.borrow_mut().push((irq, true));
        false // stub KT-45: chưa PLIC (KT-48)
    }

    fn disable_irq(&self, irq: u32) -> bool {
        self.irq_log.borrow_mut().push((irq, false));
        false
    }

    fn set_timer_deadline(&self, deadline: u64) {
        *self.timer_deadline.borrow_mut() = Some(deadline);
    }

    fn enable_timer(&self) -> bool {
        self.timer_enabled.set(true);
        true
    }

    fn disable_timer(&self) {
        self.timer_enabled.set(false);
    }
}

#[test]
fn platform_writer_in_dung_thu_tu() {
    let r = Recorder::default();
    let mut w = PlatformWriter(&r);
    write!(w, "hart={} now={}", r.hart_id(), r.now_ticks()).unwrap();
    assert_eq!(r.out.borrow().as_str(), "hart=3 now=42");
}

#[test]
fn platform_kt47_doi_dong_ho_tick_sang_ns_va_timer_api() {
    // KT-47: `now_ns` là method mặc định của trait — quy đổi từ `now_ticks` bằng `ticks_to_ns`
    // (timebase 10 MHz ⇒ 42 tick = 4.200 ns); `timebase_hz` = 10 MHz; ba method timer uỷ quyền.
    let r = Recorder::default();
    assert_eq!(r.now_ticks(), 42);
    assert_eq!(r.now_ns(), 4_200);
    assert_eq!(r.timebase_hz(), 10_000_000);
    assert!(!r.timer_enabled.get());
    r.set_timer_deadline(123_456);
    assert_eq!(*r.timer_deadline.borrow(), Some(123_456));
    assert!(r.enable_timer());
    assert!(r.timer_enabled.get());
    r.disable_timer();
    assert!(!r.timer_enabled.get());
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

#[test]
fn trapframe_bo_cuc_khop_hang_so_asm() {
    // Asm dùng các hằng này làm `const` operand; sai lệch = lỗi nặng nhất của KT-45.
    assert_eq!(std::mem::offset_of!(TrapFrame, sepc), TF_SEPC_OFFSET);
    assert_eq!(std::mem::offset_of!(TrapFrame, sstatus), TF_SSTATUS_OFFSET);
    assert_eq!(std::mem::offset_of!(TrapFrame, scause), TF_SCAUSE_OFFSET);
    assert_eq!(std::mem::offset_of!(TrapFrame, stval), TF_STVAL_OFFSET);
    assert_eq!(std::mem::size_of::<TrapFrame>(), TF_SIZE);
    assert_eq!(std::mem::size_of::<TrapFrame>(), 288);
    assert_eq!(TF_REG_COUNT, 32);
    // Truy cập qua API theo chỉ số ABI: a0 = x10, a7 = x17, sp = x2.
    let mut tf = TrapFrame::zeroed();
    tf.set_arg(0, 0x11);
    tf.set_arg(7, 0x77);
    tf.set_reg(REG_SP, 0x8021_3000);
    tf.set_reg(REG_RA, 0x8020_0100);
    assert_eq!(tf.reg(REG_A0), 0x11);
    assert_eq!(tf.reg(REG_A7), 0x77);
    assert_eq!(tf.arg(0), 0x11);
    assert_eq!(tf.arg(7), 0x77);
    assert_eq!(tf.reg(2), 0x8021_3000);
    assert_eq!(tf.reg(1), 0x8020_0100);
}

#[test]
fn decode_scause_du_15_ma_va_interrupt() {
    // Bảng KT-45: 0/2/3/4/5/6/7/8/9/12/13/15 + Unknown; bit 63 = interrupt.
    for (code, kind) in [
        (0, ExceptionKind::InstructionAddressMisaligned),
        (1, ExceptionKind::InstructionAccessFault),
        (2, ExceptionKind::IllegalInstruction),
        (3, ExceptionKind::Breakpoint),
        (4, ExceptionKind::LoadAddressMisaligned),
        (5, ExceptionKind::LoadAccessFault),
        (6, ExceptionKind::StoreAddressMisaligned),
        (7, ExceptionKind::StoreAccessFault),
        (8, ExceptionKind::EnvironmentCallFromU),
        (9, ExceptionKind::EnvironmentCallFromS),
        (12, ExceptionKind::InstructionPageFault),
        (13, ExceptionKind::LoadPageFault),
        (15, ExceptionKind::StorePageFault),
    ] {
        match decode_scause(code) {
            Event::Exception(exc) => {
                assert_eq!(exc.kind, kind, "scause {code}");
                assert_eq!(exc.kind.code(), code, "mã ngược {code}");
            }
            other => panic!("scause {code} phải là Exception, được {other:?}"),
        }
    }
    for code in [10u64, 11, 14, 16, 18, 19, 63] {
        match decode_scause(code) {
            Event::Exception(exc) => assert_eq!(exc.kind, ExceptionKind::Unknown(code)),
            other => panic!("scause {code} phải là Exception, được {other:?}"),
        }
    }
    match decode_scause(SCAUSE_INTERRUPT | 1) {
        Event::Interrupt { code } => assert_eq!(code, 1),
        other => panic!("scause interrupt phải là Interrupt, được {other:?}"),
    }
    // KT-47: ngắt S-mode timer (mã 5) decode thành Event::Timer — KHÔNG phải Exception/Interrupt.
    assert_eq!(decode_scause(SCAUSE_INTERRUPT | 5), Event::Timer);
    assert_eq!(decode_scause(SCAUSE_INTERRUPT | 9), {
        Event::Interrupt { code: 9 }
    });
}

#[test]
fn decode_trap_lay_du_ngu_canh_trapframe() {
    let mut tf = TrapFrame::zeroed();
    tf.scause = 3;
    tf.sepc = 0x8020_0123;
    tf.stval = 0x8020_0123;
    tf.sstatus = SSTATUS_SPP; // trap từ S-mode
    match decode_trap(&tf) {
        Event::Exception(exc) => {
            assert_eq!(exc.kind, ExceptionKind::Breakpoint);
            assert_eq!(exc.scause, 3);
            assert_eq!(exc.sepc, 0x8020_0123);
            assert_eq!(exc.stval, 0x8020_0123);
            assert!(exc.from_supervisor);
        }
        other => panic!("phải là Exception, được {other:?}"),
    }
    // Cùng scause nhưng SPP=0 ⇒ trap từ U-mode (KT-53), không phải S-mode.
    tf.sstatus = 0;
    match decode_trap(&tf) {
        Event::Exception(exc) => assert!(!exc.from_supervisor),
        other => panic!("phải là Exception, được {other:?}"),
    }
}

#[test]
fn handle_trap_breakpoint_va_illegal_resume_dung_sepc() {
    // ebreak/illegal 4 byte: sepc += 4, action Resume.
    for (scause, sepc) in [(3u64, 0x8020_0200u64), (2, 0x8020_0300)] {
        let mut tf = TrapFrame::zeroed();
        tf.scause = scause;
        tf.sepc = sepc;
        tf.stval = 0xffff_ffff;
        assert_eq!(
            handle_trap(&mut tf, 4),
            TrapAction::Resume,
            "scause {scause}"
        );
        assert_eq!(
            tf.sepc,
            sepc + 4,
            "scause {scause} phải resume sau instruction"
        );
        assert_eq!(tf.stval, 0xffff_ffff, "handler không được bịa stval");
    }
    // C extension: instruction 2 byte ⇒ sepc += 2 (đo bằng halfword thật, không hardcode 4).
    let mut tf = TrapFrame::zeroed();
    tf.scause = 3;
    tf.sepc = 0x8020_0400;
    assert_eq!(handle_trap(&mut tf, 2), TrapAction::Resume);
    assert_eq!(tf.sepc, 0x8020_0402);
}

#[test]
fn handle_trap_page_fault_va_fatal_giu_nguyen_sepc() {
    // KT-45: page fault/access fault/misaligned là fatal; guard page thật là KT-46.
    for (scause, kind) in [
        (13u64, ExceptionKind::LoadPageFault),
        (15, ExceptionKind::StorePageFault),
        (12, ExceptionKind::InstructionPageFault),
        (5, ExceptionKind::LoadAccessFault),
        (7, ExceptionKind::StoreAccessFault),
        (4, ExceptionKind::LoadAddressMisaligned),
        (6, ExceptionKind::StoreAddressMisaligned),
        (0, ExceptionKind::InstructionAddressMisaligned),
        (1, ExceptionKind::InstructionAccessFault),
        (18, ExceptionKind::Unknown(18)),
    ] {
        let mut tf = TrapFrame::zeroed();
        tf.scause = scause;
        tf.sepc = 0x8020_0500;
        tf.stval = 0x0010_0000;
        match handle_trap(&mut tf, 4) {
            TrapAction::Fatal(exc) => {
                assert_eq!(exc.kind, kind, "scause {scause}");
                assert_eq!(exc.sepc, 0x8020_0500, "fatal không được cộng sepc");
                assert_eq!(exc.scause, scause);
            }
            other => panic!("scause {scause} phải Fatal, được {other:?}"),
        }
        assert_eq!(tf.sepc, 0x8020_0500, "fatal giữ nguyên sepc");
    }
}

#[test]
fn report_fatal_in_panic_kem_scause_va_thanh_ghi() {
    let r = Recorder::default();
    let mut tf = TrapFrame::zeroed();
    tf.scause = 13;
    tf.sepc = 0x8020_0600;
    tf.stval = 0x0010_0000;
    tf.set_reg(REG_RA, 0x8020_0004);
    tf.set_reg(REG_SP, 0x8021_3000);
    tf.set_arg(0, 0x1111);
    tf.set_arg(7, 0x7777);
    let exc = Exception {
        scause: 13,
        kind: ExceptionKind::LoadPageFault,
        sepc: 0x8020_0600,
        stval: 0x0010_0000,
        from_supervisor: true,
    };
    let caught = catch_unwind(AssertUnwindSafe(|| report_fatal(&r, &exc, &tf)));
    assert!(caught.is_err(), "report_fatal phải halt (panic ở host)");
    let out = r.out.borrow();
    assert!(out.starts_with("PANIC: "), "out = {out:?}");
    assert!(out.contains("trap LoadPageFault"), "out = {out:?}");
    assert!(out.contains("scause=0xd"), "out = {out:?}");
    assert!(out.contains("sepc=0x80200600"), "out = {out:?}");
    assert!(out.contains("stval=0x100000"), "out = {out:?}");
    assert!(out.contains("ra=0x80200004"), "out = {out:?}");
    assert!(out.contains("a0=0x1111"), "out = {out:?}");
    assert!(out.contains("a7=0x7777"), "out = {out:?}");
    assert_eq!(*r.halt_code.borrow(), Some(1));
}

#[test]
fn report_interrupt_in_panic_va_halt_1() {
    let r = Recorder::default();
    let mut tf = TrapFrame::zeroed();
    tf.scause = SCAUSE_INTERRUPT | 5;
    tf.sepc = 0x8020_0700;
    let caught = catch_unwind(AssertUnwindSafe(|| report_interrupt(&r, 5, &tf)));
    assert!(caught.is_err());
    let out = r.out.borrow();
    assert!(
        out.starts_with("PANIC: unexpected interrupt code=5"),
        "out = {out:?}"
    );
    assert!(out.contains("scause=0x8000000000000005"), "out = {out:?}");
    assert_eq!(*r.halt_code.borrow(), Some(1));
}

#[test]
fn trap_stats_dem_va_luu_trap_gan_nhat() {
    let stats = TrapStats::new();
    let mut tf = TrapFrame::zeroed();
    tf.scause = 3;
    tf.sepc = 0x8020_0800;
    tf.stval = 0x8020_0800;
    tf.sstatus = SSTATUS_SPP;
    stats.record_exception(&tf, ExceptionKind::Breakpoint, true);
    let snap = stats.snapshot();
    assert_eq!(snap.traps, 1);
    assert_eq!(snap.exceptions, 1);
    assert_eq!(snap.recovered, 1);
    assert_eq!(snap.fatal, 0);
    assert_eq!(snap.last_scause, 3);
    assert_eq!(snap.last_sepc, 0x8020_0800);
    assert_eq!(snap.last_code, 3);

    let mut tf2 = TrapFrame::zeroed();
    tf2.scause = 13;
    tf2.sepc = 0x8020_0900;
    stats.record_exception(&tf2, ExceptionKind::LoadPageFault, false);
    let snap = stats.snapshot();
    assert_eq!(snap.traps, 2);
    assert_eq!(snap.fatal, 1);
    assert_eq!(snap.last_scause, 13);
    assert_eq!(snap.last_code, 13);

    stats.record_interrupt(&tf, 5);
    let snap = stats.snapshot();
    assert_eq!(snap.traps, 3);
    assert_eq!(snap.interrupts, 1);
    assert_eq!(snap.last_scause, SCAUSE_INTERRUPT | 5);
    assert_eq!(snap.last_code, 5);

    stats.reset();
    assert_eq!(
        stats.snapshot(),
        vifast_plat_common::trap::TrapSnapshot::default()
    );
}

#[test]
fn platform_set_trap_vector_va_stub_irq() {
    let r = Recorder::default();
    r.set_trap_vector(0x8020_1000);
    assert_eq!(*r.trap_vector.borrow(), Some(0x8020_1000));
    // KT-45: stub luôn false + ghi nhận lời gọi; PLIC thật là KT-48.
    assert!(!r.enable_irq(10));
    assert!(!r.disable_irq(10));
    assert_eq!(*r.irq_log.borrow(), vec![(10, true), (10, false)]);
}

#[test]
fn nested_trap_sp_nhan_dien_dung_bien() {
    // Trap stack [0x8021_5000, 0x8021_9000); boot stack ở dưới biên dưới.
    let bottom = 0x8021_5000u64;
    let top = 0x8021_9000u64;
    // Trap từ boot stack (sp <= bottom) KHÔNG phải lồng — kể cả đúng biên dưới.
    assert!(!is_nested_trap_sp(bottom, bottom, top));
    assert!(!is_nested_trap_sp(bottom - 0x1000, bottom, top));
    assert!(!is_nested_trap_sp(0x8021_3000, bottom, top));
    // Trap khi đang chạy trên trap stack (bottom < sp < top) LÀ lồng.
    assert!(is_nested_trap_sp(bottom + 8, bottom, top));
    assert!(is_nested_trap_sp(top - 0x120 - 8, bottom, top));
    assert!(is_nested_trap_sp(top - 0x100, bottom, top));
    // Biên trên không tính (không có sp hợp lệ nào bằng top khi đang chạy handler).
    assert!(!is_nested_trap_sp(top, bottom, top));
}
