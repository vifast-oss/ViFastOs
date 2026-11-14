// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-plat-common` — phần platform **an toàn**, dùng chung cho mọi arch (K3, KT-44).
//!
//! Crate này cố ý **không có `unsafe`** ([`#![forbid(unsafe_code)]`]): mọi thứ chạm asm/MMIO
//! nằm ở crate arch (`kernel/vifast-plat-riscv64`, thuộc danh sách đóng `kernel/vifast-plat-*`
//! của D2-17). Nhờ vậy phần logic ở đây chạy được trên host (unit test / Miri, D2-72 Q12) tách
//! khỏi phần không chạy được trên host.
//!
//! Phạm vi K3 (KT-44) đúng bằng cái vòng trap cần:
//! - [`Platform`]: `put_str`/`halt`/`now_ticks`/`now_ns`/`timebase_hz`/`hart_id` — chữ ký ổn
//!   định, arch tự cài;
//! - [`PanicHandler`]: handler panic **dùng chung** cho ảnh sản phẩm và ảnh ktest (D2-08:
//!   hai ảnh khác nhau nhưng cùng một handler, không chép đôi);
//! - [`PlatformWriter`]: `core::fmt::Write` trên [`Platform::put_str`], để in số không cần `alloc`.
//!
//! KT-45 mở rộng cùng crate: module [`trap`] (trapframe 32 thanh ghi + `sepc/sstatus/scause/stval`,
//! decode **mọi** `scause` 0/2/3/4/5/6/7/8/9/12/13/15/khác ⇒ [`trap::Event::Exception`], chính
//! sách resume/fatal, bộ đếm trap) và ba method mới của [`Platform`]: `set_trap_vector` +
//! `enable_irq`/`disable_irq` (stub ở KT-45; **PLIC thật từ KT-48**).
//!
//! KT-46 mở module [`mmu`]: page table **Sv39 thật** dựng từ arena RAM (không `static`), map
//! 4 KiB / 2 MiB / 1 GiB, walk phần mềm đối chiếu phần cứng, cờ PTE `V/R/W/X/A/D` và `satp` Sv39.
//! Phần asm/CSR/MMIO/`satp` nằm ở arch (`kernel/vifast-plat-riscv64`) theo D2-17.
//!
//! KT-47 mở module [`timer`] (SBI TIME, tick 100 Hz, timebase 10 MHz — D2-72 Q6): hằng số `[TK]`,
//! phân loại tick/deadline **tuyệt đối**, phát hiện mất tick ("trễ > 1 tick"), bộ đếm
//! `ticks/late/max_delta` + bằng chứng `sstatus.SIE`; [`Platform`] thêm `now_ticks`/`now_ns`,
//! `set_timer_deadline`, `enable_timer`/`disable_timer`.
//!
//! KT-48 mở module [`plic`] (PLIC cổ điển — D2-72 Q7/Q8): hình học đăng ký priority/pending/
//! enable/context `0x20_0000 + 0x1000*ctx`, context S-mode `2*hart+1`, phân loại claim
//! (UART / spurious / line lạ), bộ đếm claim/complete + ngắt UART cho TAP "0 IRQ giả"; `trap` thêm
//! mã ngắt external **9** ⇒ `Event::Interrupt { code: 9 }` do `trap_dispatch` đưa vào
//! claim/complete. **Không** thuộc KT-48: IPI (KT-50), SMP (KT-49), ASID/shootdown (KT-51),
//! U-mode (KT-53), MSI/AIA — xem `v2/docs/k3-riscv64.md` §11.
#![no_std]
#![forbid(unsafe_code)]

pub mod mmu;
pub mod plic;
pub mod timer;
pub mod trap;

use core::fmt::{self, Write};
use core::panic::PanicInfo;

use timer::ticks_to_ns;

/// Nền tảng tối thiểu mà kernel cần từ arch (K3).
///
/// `halt` trả `!` vì OpenSBI SRST không quay lại. KT-47: `now_ticks` là `rdtime` (timebase
/// [`Self::timebase_hz`]); `set_timer_deadline`/`enable_timer`/`disable_timer` là SBI TIME +
/// `sie.STIE`/`sstatus.SIE` (D2-72 Q6). Ngữ nghĩa tick/deadline ở module [`timer`] — phần arch chỉ
/// là asm/CSR/SBI.
pub trait Platform {
    /// In một chuỗi ra console debug (`DebugPutStr` dùng lại ở K3).
    fn put_str(&self, s: &str);

    /// Dừng máy. `code` chỉ để phân loại trong log: SBI SRST không mang mã thoát; runner phân
    /// biệt đường sạch/đường panic bằng marker TAP và dòng `PANIC`.
    fn halt(&self, code: usize) -> !;

    /// Đọc đồng hồ monotonic thô (`rdtime`), đơn vị là tick timebase của [`Self::timebase_hz`].
    fn now_ticks(&self) -> u64;

    /// Đọc đồng hồ monotonic theo **ns** — quy đổi từ [`Self::now_ticks`] bằng
    /// [`timer::ticks_to_ns`] (10 MHz ⇒ chính xác tuyệt đối).
    fn now_ns(&self) -> u64 {
        ticks_to_ns(self.now_ticks())
    }

    /// Tần số **timebase** (tick/giây): QEMU `virt` = 10 MHz (D2-72 Q6). Tick kernel là
    /// [`timer::TICK_HZ`] = 100 Hz, **khác** tần số này.
    fn timebase_hz(&self) -> u64;

    /// Hart đang chạy (Boot HART id OpenSBI truyền ở `a0`).
    fn hart_id(&self) -> usize;

    /// Cài vector trap **direct mode** (`stvec.MODE = 0`), địa chỉ phải 4-byte aligned.
    ///
    /// KT-45 dùng **một** vector duy nhất: asm lưu trapframe rồi gọi handler chung; không vectored,
    /// không bảng vector theo mã `scause` (K3-PLAN §4.1 KT-45).
    fn set_trap_vector(&self, vector: usize);

    /// Bật một dòng IRQ trên **PLIC** (KT-48): set bit enable của context S-mode `2*hart+1` cho
    /// `irq`, đọc lại bit để trả kết quả **thật**.
    ///
    /// Đây chỉ là cổng **nguồn** của PLIC; cổng CPU (`sie.SEIE` + `sstatus.SIE`) do arch bật/tắt
    /// riêng (`vifast-plat-riscv64::plic::enable_external`, đối xứng `timer::enable_timer`).
    /// Priority/threshold do `plic::init` đặt trước (PLIC chỉ forward nguồn priority > threshold).
    fn enable_irq(&self, irq: u32) -> bool;

    /// Tắt một dòng IRQ trên **PLIC** (KT-48): clear bit enable + đọc lại; `true` ⇔ bit đã về 0.
    fn disable_irq(&self, irq: u32) -> bool;

    /// Đặt **deadline tuyệt đối** cho ngắt timer kế (đơn vị timebase) — SBI TIME EID
    /// `0x54494D45`, FID 0 (`sbi_set_timer`), D2-72 Q6.
    ///
    /// Deadline ở **tương lai** đồng thời xoá ngắt timer đang chờ; `u64::MAX` = tắt hữu hiệu
    /// (dùng trước khi `disable_timer` hoặc khi kết thúc test).
    fn set_timer_deadline(&self, deadline: u64);

    /// Bật ngắt timer: `sie.STIE = 1` **rồi** `sstatus.SIE = 1` (nguồn trước, cổng sau).
    ///
    /// Trả `true` ⇔ đọc lại thấy **cả hai** bit đã bật. KT-47 (P2a): đây là chỗ **duy nhất** bật
    /// `sstatus.SIE`; `trap_entry`/handler **không** ghi `sstatus` — vào trap hardware xoá SIE,
    /// `sret` khôi phục SIE từ SPIE (không có trap lồng trong handler).
    fn enable_timer(&self) -> bool;

    /// Tắt ngắt timer: `sstatus.SIE = 0` **rồi** `sie.STIE = 0` (đảo thứ tự của [`Self::enable_timer`]).
    fn disable_timer(&self);
}

/// Triển khai [`Platform`] cho tham chiếu — để `PanicHandler::new(&plat)` dùng được mà không
/// phải `Copy` nền tảng; không thêm `unsafe`, không thêm trạng thái.
impl<P: Platform + ?Sized> Platform for &P {
    fn put_str(&self, s: &str) {
        (**self).put_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        (**self).halt(code)
    }

    fn now_ticks(&self) -> u64 {
        (**self).now_ticks()
    }

    fn now_ns(&self) -> u64 {
        (**self).now_ns()
    }

    fn timebase_hz(&self) -> u64 {
        (**self).timebase_hz()
    }

    fn hart_id(&self) -> usize {
        (**self).hart_id()
    }

    fn set_trap_vector(&self, vector: usize) {
        (**self).set_trap_vector(vector);
    }

    fn enable_irq(&self, irq: u32) -> bool {
        (**self).enable_irq(irq)
    }

    fn disable_irq(&self, irq: u32) -> bool {
        (**self).disable_irq(irq)
    }

    fn set_timer_deadline(&self, deadline: u64) {
        (**self).set_timer_deadline(deadline);
    }

    fn enable_timer(&self) -> bool {
        (**self).enable_timer()
    }

    fn disable_timer(&self) {
        (**self).disable_timer();
    }
}

/// `core::fmt::Write` ghi thẳng qua [`Platform::put_str`] — không cấp phát, không `unsafe`.
///
/// Dùng để in số (hart id, tick, mã lỗi) mà không cần `alloc`:
/// `let mut w = PlatformWriter(&plat); let _ = write!(w, "hart={}", plat.hart_id());`
pub struct PlatformWriter<'a, P: Platform + ?Sized>(pub &'a P);

impl<P: Platform + ?Sized> fmt::Write for PlatformWriter<'_, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.put_str(s);
        Ok(())
    }
}

/// Vị trí panic đã tách khỏi [`PanicInfo`] — cho phép test host gọi đúng một đường in.
pub type PanicLocation = (&'static str, u32, u32);

/// Handler panic **dùng chung** cho mọi ảnh (sản phẩm + ktest), xem D2-08.
///
/// Mỗi ảnh giữ **một** `static` của kiểu này rồi cài `#[panic_handler]` một dòng:
///
/// ```ignore
/// static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);
///
/// #[panic_handler]
/// fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
///     PANIC.handle(info)
/// }
/// ```
///
/// Không có `static mut`/`unsafe`: nền tảng được truyền vào lúc khởi tạo trong `const`, không
/// cần trạng thái toàn cục ghi được.
pub struct PanicHandler<P: Platform> {
    platform: P,
}

impl<P: Platform> PanicHandler<P> {
    /// Tạo handler từ một nền tảng đã biết; `const fn` nên đặt được trong `static`.
    pub const fn new(platform: P) -> Self {
        Self { platform }
    }

    /// In `PANIC` + vị trí + thông điệp rồi [`Platform::halt`]`(1)`.
    ///
    /// Dòng `PANIC` không bao giờ là `ok` trong TAP, nên runner coi lần boot đó là đỏ.
    pub fn handle(&self, info: &PanicInfo<'_>) -> ! {
        let location = info.location().map(|l| (l.file(), l.line(), l.column()));
        let message = info.message();
        self.handle_with(location, format_args!("{message}"))
    }

    /// Nhân của [`Self::handle`] không cần [`PanicInfo`] — host test không dựng được `PanicInfo`
    /// trên stable, nên đây là đường test được (cùng một chuỗi in, cùng `halt(1)`).
    pub fn handle_with(&self, location: Option<PanicLocation>, message: fmt::Arguments<'_>) -> ! {
        self.platform.put_str("PANIC");
        if let Some((file, line, column)) = location {
            let mut w = PlatformWriter(&self.platform);
            let _ = write!(w, " at {file}:{line}:{column}");
        }
        let mut w = PlatformWriter(&self.platform);
        let _ = write!(w, "\nPANIC: {message}\n");
        self.platform.halt(1)
    }
}
