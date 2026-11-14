// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-plat-common` — phần platform **an toàn**, dùng chung cho mọi arch (K3, KT-44).
//!
//! Crate này cố ý **không có `unsafe`** ([`#![forbid(unsafe_code)]`]): mọi thứ chạm asm/MMIO
//! nằm ở crate arch (`kernel/vifast-plat-riscv64`, thuộc danh sách đóng `kernel/vifast-plat-*`
//! của D2-17). Nhờ vậy phần logic ở đây chạy được trên host (unit test / Miri, D2-72 Q12) tách
//! khỏi phần không chạy được trên host.
//!
//! Phạm vi K3 (KT-44) đúng bằng cái vòng trap cần:
//! - [`Platform`]: `put_str`/`halt`/`now`/`tick_hz`/`hart_id` — chữ ký ổn định, arch tự cài;
//! - [`PanicHandler`]: handler panic **dùng chung** cho ảnh sản phẩm và ảnh ktest (D2-08:
//!   hai ảnh khác nhau nhưng cùng một handler, không chép đôi);
//! - [`PlatformWriter`]: `core::fmt::Write` trên [`Platform::put_str`], để in số không cần `alloc`.
//!
//! KT-45 mở rộng cùng crate: module [`trap`] (trapframe 32 thanh ghi + `sepc/sstatus/scause/stval`,
//! decode **mọi** `scause` 0/2/3/4/5/6/7/8/9/12/13/15/khác ⇒ [`trap::Event::Exception`], chính
//! sách resume/fatal, bộ đếm trap) và ba method mới của [`Platform`]: `set_trap_vector` +
//! `enable_irq`/`disable_irq` (stub, PLIC là KT-48). **Không** thuộc KT-45: MMU/satp (KT-46), timer
//! (KT-47), PLIC (KT-48), SMP (KT-49), U-mode/syscall (KT-53) — xem `v2/docs/k3-riscv64.md`.
#![no_std]
#![forbid(unsafe_code)]

pub mod trap;

use core::fmt::{self, Write};
use core::panic::PanicInfo;

/// Nền tảng tối thiểu mà kernel cần từ arch (K3).
///
/// `halt` trả `!` vì OpenSBI SRST không quay lại. `now`/`tick_hz` hiện là **stub đồng hồ
/// monotonic** (đọc `time` CSR); timer thật (`set_timer`, tick 100 Hz, deadline mỗi hart) là
/// KT-47 theo D2-72 Q6.
pub trait Platform {
    /// In một chuỗi ra console debug (`DebugPutStr` dùng lại ở K3).
    fn put_str(&self, s: &str);

    /// Dừng máy. `code` chỉ để phân loại trong log: SBI SRST không mang mã thoát; runner phân
    /// biệt đường sạch/đường panic bằng marker TAP và dòng `PANIC`.
    fn halt(&self, code: usize) -> !;

    /// Đọc đồng hồ monotonic thô, đơn vị là tick của [`Self::tick_hz`].
    fn now(&self) -> u64;

    /// Tần số timebase (tick/giây). QEMU `virt`: 10 MHz (D2-72 Q6).
    fn tick_hz(&self) -> u64;

    /// Hart đang chạy (Boot HART id OpenSBI truyền ở `a0`).
    fn hart_id(&self) -> usize;

    /// Cài vector trap **direct mode** (`stvec.MODE = 0`), địa chỉ phải 4-byte aligned.
    ///
    /// KT-45 dùng **một** vector duy nhất: asm lưu trapframe rồi gọi handler chung; không vectored,
    /// không bảng vector theo mã `scause` (K3-PLAN §4.1 KT-45).
    fn set_trap_vector(&self, vector: usize);

    /// Bật một dòng IRQ — **stub** ở KT-45, chưa PLIC (KT-48).
    ///
    /// Trả `true` nếu dòng đã được bật thật; K3 KT-45 luôn trả `false` (chưa có PLIC). Giữ chữ ký
    /// ổn định để KT-48 chỉ thay phần thân, không đổi người gọi.
    fn enable_irq(&self, irq: u32) -> bool;

    /// Tắt một dòng IRQ — **stub** ở KT-45, chưa PLIC (KT-48); cùng hợp đồng với [`Self::enable_irq`].
    fn disable_irq(&self, irq: u32) -> bool;
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

    fn now(&self) -> u64 {
        (**self).now()
    }

    fn tick_hz(&self) -> u64 {
        (**self).tick_hz()
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
