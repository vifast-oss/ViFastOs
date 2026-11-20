// SPDX-License-Identifier: MIT OR Apache-2.0
//! Asm nguyên thủy AArch64 — **mô-đun duy nhất** của crate có `unsafe` (KT-167 / K4' bước 2).
//!
//! # Vì sao tách riêng
//!
//! Crate giữ `#![deny(unsafe_code)]` ở gốc; **chỉ** tệp này `#![allow(unsafe_code)]` (D2-17 cho
//! phép `unsafe` trong `kernel/vifast-plat-*`). Mọi phần dữ liệu ([`crate::sysreg`],
//! [`crate::tlb`], [`crate::barrier`], [`crate::format`], [`crate::mmu`]) vẫn **không** `unsafe`.
//!
//! # Host stub
//!
//! Mỗi hàm có hai bản: bản **bare-metal aarch64** (`target_arch = "aarch64"` **và**
//! `target_os = "none"`) là asm thật; bản host **panic** (không phải no-op) để `cargo test` trên
//! host vẫn biên dịch được mà không ai lỡ gọi asm đặc quyền trên máy thật. Test
//! `host_stub_is_fail_loud` chứng minh stub **không im lặng**.
//!
//! **Vì sao có thêm `target_os = "none"`**: máy phát triển của repo là **Apple Silicon**
//! (`aarch64-apple-darwin`) — chỉ `cfg(target_arch = "aarch64")` sẽ biên dịch asm đặc quyền
//! (`TLBI`, `msr`) vào **test host** và chạy nó ở EL0 ⇒ fault. Điều kiện đúng là "aarch64
//! bare-metal", không phải "aarch64 bất kỳ"; `cargo build --target aarch64-unknown-none` thoả
//! `target_os = "none"` nên vẫn dùng asm thật.
//!
//! # Encoding lấy từ đâu
//!
//! `TLBI`/barrier dùng `.inst <hằng>` với hằng ở [`crate::tlb`]/[`crate::barrier`] — nghĩa là
//! **đúng giá trị đã được test host so tuyệt đối** là giá trị phát ra instruction, không có
//! đường thứ hai để lệch. Các hằng này đã đối chiếu bằng hợp dịch thật (`clang` +
//! `llvm-objdump`), xem tài liệu hai tệp đó.
//!
//! # Sysreg
//!
//! `write_*`/`read_*` chỉ là `msr`/`mrs`; **không** tự chèn barrier. Caller phải gọi
//! [`isb`] sau khi ghi thanh ghi điều khiển MMU (`TCR_EL1`, `SCTLR_EL1`, `TTBR*`, `VBAR_EL1`) —
//! đúng yêu cầu context synchronization của đặc tả.

#![allow(unsafe_code)] // D2-17: crate `kernel/vifast-plat-*`; chỉ asm mới cần.

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
use core::arch::global_asm;

// Probe `brk #0` (KT-248) — **một** instruction, địa chỉ lấy được từ Rust để so `ELR_EL1`.
//
// Đặt ở tệp này (không phải ảnh) vì gọi một `extern "C"` fn cần `unsafe`, mà `unsafe` chỉ được
// nằm trong mô-đun này (D2-17 / luật R5). Ảnh gọi `brk_probe` — hàm an toàn.
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
global_asm!(
    ".section .text, \"ax\"",
    ".globl __vifast_brk_probe",
    "__vifast_brk_probe:",
    "brk #0",
    "ret",
);

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
extern "C" {
    /// Nhãn asm của probe — chỉ dùng để lấy địa chỉ và gọi.
    fn __vifast_brk_probe();
}

/// `PSCI SYSTEM_OFF` (function id 64-bit) — tắt máy qua hypervisor/QEMU.
///
/// Giá trị theo PSCI spec (ARM DEN 0022): `0x8400_0008` = `SYSTEM_OFF` trong dải function id
/// 64-bit (`0x8400_0000` + offset 8).
pub const PSCI_SYSTEM_OFF: u64 = 0x8400_0008;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
mod imp {
    use core::arch::asm;

    // ------------------------------------------------------------------ TLBI

    /// `tlbi vmalle1` — xoá toàn bộ TLB EL1&0 của PE hiện tại.
    pub fn tlbi_vmalle1() {
        // SAFETY: system instruction hợp lệ ở EL1; không đọc/ghi bộ nhớ, không đổi cờ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VMALLE1,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi vmalle1is` — xoá toàn bộ TLB EL1&0, broadcast inner-shareable.
    pub fn tlbi_vmalle1is() {
        // SAFETY: như `tlbi_vmalle1`; broadcast chỉ ảnh hưởng TLB của các PE cùng domain.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VMALLE1IS,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi vae1, x0` — `arg` = `VA[55:12]`.
    pub fn tlbi_vae1(arg: u64) {
        // SAFETY: `arg` được đặt vào x0 đúng như encoding `Rt = 0`; system instruction hợp lệ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VAE1,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi vae1is, x0` — `arg` = `VA[55:12]`, broadcast inner-shareable.
    pub fn tlbi_vae1is(arg: u64) {
        // SAFETY: như `tlbi_vae1`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VAE1IS,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi aside1, x0` — `arg` = ASID (16 bit thấp).
    pub fn tlbi_aside1(arg: u64) {
        // SAFETY: `arg` được đặt vào x0 đúng như encoding `Rt = 0`; system instruction hợp lệ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_ASIDE1,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi aside1is, x0` — `arg` = ASID, broadcast inner-shareable.
    pub fn tlbi_aside1is(arg: u64) {
        // SAFETY: như `tlbi_aside1`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_ASIDE1IS,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    // ------------------------------------------------------------------ barrier

    /// `dsb sy`.
    pub fn dsb_sy() {
        // SAFETY: barrier hợp lệ ở mọi EL; không đọc/ghi thanh ghi, không đổi cờ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DSB_SY,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dsb ish`.
    pub fn dsb_ish() {
        // SAFETY: như `dsb_sy`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DSB_ISH,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dmb sy`.
    pub fn dmb_sy() {
        // SAFETY: barrier hợp lệ ở mọi EL; không đọc/ghi thanh ghi, không đổi cờ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DMB_SY,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dmb ish`.
    pub fn dmb_ish() {
        // SAFETY: như `dmb_sy`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DMB_ISH,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `isb`.
    pub fn isb() {
        // SAFETY: context synchronization event hợp lệ ở mọi EL; không đọc/ghi thanh ghi.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::ISB,
                options(nostack, preserves_flags),
            )
        };
    }

    // ------------------------------------------------------------------ MMIO

    /// Đọc 32 bit từ MMIO (thanh ghi thiết bị) — `ldr w` với địa chỉ tuyệt đối.
    ///
    /// **Vì sao ở đây**: `unsafe` chỉ được nằm trong mô-đun này (D2-17); driver PL011
    /// ([`crate::pl011`]) vì vậy **không** có `unsafe` mà vẫn truy cập được thanh ghi.
    pub fn mmio_read_u32(addr: u64) -> u32 {
        // SAFETY: `addr` do caller cấp và phải là địa chỉ MMIO của thiết bị đã map (PL011 ở
        // `0x0900_0000` trên QEMU virt). Đọc 4 byte căn lề tự nhiên; không ghi.
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }

    /// Ghi 32 bit ra MMIO (thanh ghi thiết bị).
    pub fn mmio_write_u32(addr: u64, value: u32) {
        // SAFETY: như `mmio_read_u32`; ghi 4 byte vào thanh ghi thiết bị đã map.
        unsafe { core::ptr::write_volatile(addr as *mut u32, value) };
    }

    // ------------------------------------------------------------------ bộ nhớ thô (KT-249)

    /// Đọc 64 bit tại **địa chỉ tuyệt đối** `addr` (VA sau khi MMU bật, PA trước đó).
    ///
    /// Dùng cho ca kiểm "MMU thật sự bật": đọc cùng một trang qua **VA không identity** — nếu
    /// MMU chưa bật thì địa chỉ ấy là PA và trả giá trị khác. `unsafe` nằm ở đây (D2-17).
    pub fn read_u64(addr: u64) -> u64 {
        // SAFETY: `addr` do caller cấp và phải là địa chỉ đọc được trong bản đồ hiện hành
        // (identity map của kernel hoặc VA đã map tường minh). Đọc 8 byte căn lề tự nhiên.
        unsafe { core::ptr::read_volatile(addr as *const u64) }
    }

    /// Ghi 64 bit tại **địa chỉ tuyệt đối** `addr` (xem [`read_u64`]).
    pub fn write_u64(addr: u64, value: u64) {
        // SAFETY: như `read_u64`; ghi 8 byte vào vùng nhớ đã map và ghi được.
        unsafe { core::ptr::write_volatile(addr as *mut u64, value) };
    }

    // ------------------------------------------------------------------ probe BRK (KT-248)

    /// `brk #0` — trap **có chủ đích** vào ô `SyncCurrentElSpX` (`0x200`) của bảng vector.
    ///
    /// `ELR_EL1` của `BRK` là **địa chỉ instruction `BRK`** (không phải `+4`), nên handler phải
    /// tự cộng 4 khi resume — xem `vec_handler` của ảnh. Hàm này **trả về** sau khi handler
    /// resume; nếu chưa cài bảng vector thì `brk` đi vào `VBAR_EL1 = 0` và treo (đúng: probe chỉ
    /// dùng sau khi đã cài).
    pub fn brk_probe() {
        // SAFETY: `brk #0` là instruction hợp lệ ở EL1; nó **không** đọc/ghi bộ nhớ, chỉ sinh
        // exception đồng bộ. Handler của ảnh resume bằng `ELR += 4`.
        unsafe { super::__vifast_brk_probe() };
    }

    /// Địa chỉ instruction `brk #0` của [`brk_probe`] — để handler so với `ELR_EL1`.
    pub fn brk_probe_addr() -> u64 {
        super::__vifast_brk_probe as *const () as u64
    }

    // ------------------------------------------------------------------ PSCI / dừng máy

    /// `PSCI SYSTEM_OFF` qua `hvc #0` với `x0 = 0x8400_0008` (PSCI v1.x, 64-bit calling
    /// convention, function id `0x8400_0008` = `SYSTEM_OFF`).
    ///
    /// QEMU `virt` đặt PSCI conduit = **HVC** khi khách khởi động ở EL1 (đúng đường `-kernel`
    /// của bước 3); `SYSTEM_OFF` làm QEMU thoát với mã 0. Hàm **trả về** nếu PSCI không có
    /// (hypervisor khác/không hỗ trợ) — caller phải có đường dự phòng, không giả định đã tắt.
    pub fn psci_system_off() {
        // SAFETY: `hvc #0` là lệnh gọi hypervisor hợp lệ ở EL1; `x0` mang function id PSCI.
        // Không đọc/ghi bộ nhớ của kernel.
        unsafe {
            asm!(
                "hvc #0",
                in("x0") super::PSCI_SYSTEM_OFF,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `wfi` trong vòng lặp vô hạn — đường dừng **dự phòng** khi PSCI không tắt được máy.
    ///
    /// Không phải "treo im lặng": caller đã in marker/`PANIC` trước đó, và runner phân biệt
    /// đường sạch với đường treo bằng chính các dòng ấy (xem `v2/test/boot/run-aarch64.sh`).
    pub fn wfi_forever() -> ! {
        loop {
            // SAFETY: `wfi` chỉ chờ ngắt; không truy cập bộ nhớ, không đổi thanh ghi.
            unsafe { asm!("wfi", options(nostack, preserves_flags, nomem)) };
        }
    }

    // ------------------------------------------------------------------ sysreg: ghi

    /// Ghi `TCR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_tcr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Tcr`.
        unsafe { asm!("msr tcr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `MAIR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_mair_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Mair`.
        unsafe { asm!("msr mair_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `SCTLR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_sctlr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Sctlr`.
        unsafe { asm!("msr sctlr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `TTBR0_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_ttbr0_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Ttbr`.
        unsafe { asm!("msr ttbr0_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `TTBR1_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_ttbr1_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Ttbr`.
        unsafe { asm!("msr ttbr1_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `VBAR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_vbar_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Vbar`.
        unsafe { asm!("msr vbar_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `ELR_EL1`.
    pub fn write_elr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::ElrEl1`.
        unsafe { asm!("msr elr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `SPSR_EL1`.
    pub fn write_spsr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Spsr`.
        unsafe { asm!("msr spsr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `DAIF` (mask/unmask ngắt).
    pub fn write_daif(value: u64) {
        // SAFETY: `msr daif` chỉ đổi bốn bit mask ngắt của PSTATE; giá trị từ `sysreg::Daif`.
        unsafe { asm!("msr daif, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    // ------------------------------------------------------------------ sysreg: đọc

    /// Đọc `TCR_EL1`.
    pub fn read_tcr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc, không tác dụng phụ.
        unsafe { asm!("mrs {}, tcr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `MAIR_EL1`.
    pub fn read_mair_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, mair_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `SCTLR_EL1`.
    pub fn read_sctlr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, sctlr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `TTBR0_EL1`.
    pub fn read_ttbr0_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `TTBR1_EL1`.
    pub fn read_ttbr1_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, ttbr1_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `VBAR_EL1`.
    pub fn read_vbar_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, vbar_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `ESR_EL1` (nguyên nhân trap).
    pub fn read_esr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, esr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `FAR_EL1` (địa chỉ gây fault).
    pub fn read_far_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, far_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `ELR_EL1`.
    pub fn read_elr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, elr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `SPSR_EL1`.
    pub fn read_spsr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, spsr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `DAIF`.
    pub fn read_daif() -> u64 {
        let value: u64;
        // SAFETY: `mrs daif` chỉ đọc bốn bit mask ngắt của PSTATE.
        unsafe { asm!("mrs {}, daif", out(reg) value, options(nostack, preserves_flags)) };
        value
    }
}

#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
mod imp {
    /// Stub host: **panic** thay vì no-op — gọi asm đặc quyền trên host là lỗi lập trình.
    fn host_stub(name: &str) -> ! {
        panic!("vifast-plat-aarch64: {name} chỉ chạy trên aarch64 (host stub, KT-167)")
    }

    /// Stub host của `tlbi vmalle1`.
    pub fn tlbi_vmalle1() {
        host_stub("tlbi vmalle1")
    }
    /// Stub host của `tlbi vmalle1is`.
    pub fn tlbi_vmalle1is() {
        host_stub("tlbi vmalle1is")
    }
    /// Stub host của `tlbi vae1`.
    pub fn tlbi_vae1(_arg: u64) {
        host_stub("tlbi vae1")
    }
    /// Stub host của `tlbi vae1is`.
    pub fn tlbi_vae1is(_arg: u64) {
        host_stub("tlbi vae1is")
    }
    /// Stub host của `tlbi aside1`.
    pub fn tlbi_aside1(_arg: u64) {
        host_stub("tlbi aside1")
    }
    /// Stub host của `tlbi aside1is`.
    pub fn tlbi_aside1is(_arg: u64) {
        host_stub("tlbi aside1is")
    }
    /// Stub host của `dsb sy`.
    pub fn dsb_sy() {
        host_stub("dsb sy")
    }
    /// Stub host của `dsb ish`.
    pub fn dsb_ish() {
        host_stub("dsb ish")
    }
    /// Stub host của `dmb sy`.
    pub fn dmb_sy() {
        host_stub("dmb sy")
    }
    /// Stub host của `dmb ish`.
    pub fn dmb_ish() {
        host_stub("dmb ish")
    }
    /// Stub host của `isb`.
    pub fn isb() {
        host_stub("isb")
    }
    /// Stub host của MMIO đọc — **panic**: trên host không có thiết bị PL011 đã map.
    pub fn mmio_read_u32(_addr: u64) -> u32 {
        host_stub("mmio read u32")
    }
    /// Stub host của MMIO ghi — **panic**: trên host không có thiết bị PL011 đã map.
    pub fn mmio_write_u32(_addr: u64, _value: u32) {
        host_stub("mmio write u32")
    }
    /// Stub host của đọc bộ nhớ thô — **panic**: trên host không có bản đồ của kernel.
    pub fn read_u64(_addr: u64) -> u64 {
        host_stub("read u64")
    }
    /// Stub host của ghi bộ nhớ thô — **panic**: trên host không có bản đồ của kernel.
    pub fn write_u64(_addr: u64, _value: u64) {
        host_stub("write u64")
    }
    /// Stub host của `brk #0` — **panic**: trên host `brk` sẽ giết tiến trình test.
    pub fn brk_probe() {
        host_stub("brk #0")
    }
    /// Stub host của địa chỉ probe — **panic**: không có asm trên host.
    pub fn brk_probe_addr() -> u64 {
        host_stub("brk #0 addr")
    }
    /// Stub host của `PSCI SYSTEM_OFF` — **panic**: trên host không có PSCI.
    pub fn psci_system_off() {
        host_stub("psci system_off")
    }
    /// Stub host của `wfi` vô hạn — **panic**: trên host không được treo tiến trình test.
    pub fn wfi_forever() -> ! {
        host_stub("wfi forever")
    }
    /// Stub host của `msr tcr_el1`.
    pub fn write_tcr_el1(_value: u64) {
        host_stub("msr tcr_el1")
    }
    /// Stub host của `msr mair_el1`.
    pub fn write_mair_el1(_value: u64) {
        host_stub("msr mair_el1")
    }
    /// Stub host của `msr sctlr_el1`.
    pub fn write_sctlr_el1(_value: u64) {
        host_stub("msr sctlr_el1")
    }
    /// Stub host của `msr ttbr0_el1`.
    pub fn write_ttbr0_el1(_value: u64) {
        host_stub("msr ttbr0_el1")
    }
    /// Stub host của `msr ttbr1_el1`.
    pub fn write_ttbr1_el1(_value: u64) {
        host_stub("msr ttbr1_el1")
    }
    /// Stub host của `msr vbar_el1`.
    pub fn write_vbar_el1(_value: u64) {
        host_stub("msr vbar_el1")
    }
    /// Stub host của `msr elr_el1`.
    pub fn write_elr_el1(_value: u64) {
        host_stub("msr elr_el1")
    }
    /// Stub host của `msr spsr_el1`.
    pub fn write_spsr_el1(_value: u64) {
        host_stub("msr spsr_el1")
    }
    /// Stub host của `msr daif`.
    pub fn write_daif(_value: u64) {
        host_stub("msr daif")
    }
    /// Stub host của `mrs tcr_el1`.
    pub fn read_tcr_el1() -> u64 {
        host_stub("mrs tcr_el1")
    }
    /// Stub host của `mrs mair_el1`.
    pub fn read_mair_el1() -> u64 {
        host_stub("mrs mair_el1")
    }
    /// Stub host của `mrs sctlr_el1`.
    pub fn read_sctlr_el1() -> u64 {
        host_stub("mrs sctlr_el1")
    }
    /// Stub host của `mrs ttbr0_el1`.
    pub fn read_ttbr0_el1() -> u64 {
        host_stub("mrs ttbr0_el1")
    }
    /// Stub host của `mrs ttbr1_el1`.
    pub fn read_ttbr1_el1() -> u64 {
        host_stub("mrs ttbr1_el1")
    }
    /// Stub host của `mrs vbar_el1`.
    pub fn read_vbar_el1() -> u64 {
        host_stub("mrs vbar_el1")
    }
    /// Stub host của `mrs esr_el1`.
    pub fn read_esr_el1() -> u64 {
        host_stub("mrs esr_el1")
    }
    /// Stub host của `mrs far_el1`.
    pub fn read_far_el1() -> u64 {
        host_stub("mrs far_el1")
    }
    /// Stub host của `mrs elr_el1`.
    pub fn read_elr_el1() -> u64 {
        host_stub("mrs elr_el1")
    }
    /// Stub host của `mrs spsr_el1`.
    pub fn read_spsr_el1() -> u64 {
        host_stub("mrs spsr_el1")
    }
    /// Stub host của `mrs daif`.
    pub fn read_daif() -> u64 {
        host_stub("mrs daif")
    }
}

pub use imp::*;
