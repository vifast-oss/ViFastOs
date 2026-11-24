// SPDX-License-Identifier: MIT OR Apache-2.0
//! Asm nguyên thủy x86_64 — **mô-đun duy nhất** của crate có `unsafe` (KT-257, K5' bước 1).
//!
//! # Vì sao tách riêng
//!
//! Crate giữ `#![deny(unsafe_code)]` ở gốc; **chỉ** tệp này `#![allow(unsafe_code)]` (D2-17 cho
//! phép `unsafe` trong `kernel/vifast-plat-*`). Mọi phần dữ liệu ([`crate::serial`],
//! [`crate::boot`], [`crate::format`], [`crate::mmu`], [`crate::control`], [`crate::calling`])
//! vẫn **không** `unsafe`. Ảnh boot `vifast-kbin-x86_64` gọi các hàm **an toàn** ở đây nên bản
//! thân ảnh cũng không có `unsafe` (luật R5).
//!
//! # Host stub
//!
//! Mỗi hàm có hai bản: bản **bare-metal x86_64** (`target_arch = "x86_64"` **và**
//! `target_os = "none"`) là asm thật; bản host **panic** (không phải no-op) để `cargo test` trên
//! host vẫn biên dịch được mà không ai lỡ gọi cổng I/O đặc quyền trên máy thật.
//!
//! **Vì sao có thêm `target_os = "none"`**: máy phát triển của repo là **Apple Silicon**
//! (`aarch64-apple-darwin`), nhưng nếu ai chạy test trên macOS x86_64 thì `target_arch = "x86_64"`
//! vẫn khớp và `in`/`out` sẽ fault ở user mode. Điều kiện đúng là "x86_64 bare-metal", không phải
//! "x86_64 bất kỳ"; `cargo build --target x86_64-unknown-none` thoả `target_os = "none"`.
//!
//! # Vì sao `halt` phân nhánh theo `code`
//!
//! QEMU ACPI shutdown (`outw(0x604, 0x2000)`) **luôn** làm QEMU thoát mã 0 — không có cách truyền
//! mã lỗi qua đường đó. Vì vậy: `code == 0` ⇒ shutdown sạch (runner thấy `qg_boot` rc=0);
//! `code != 0` ⇒ **không** shutdown, chỉ `cli; hlt` ⇒ QEMU treo tới hết ngân sách ⇒ runner ĐỎ.
//! Nhờ vậy một `PANIC` không bao giờ trông giống "boot thành công".

#![allow(unsafe_code)] // D2-17: crate `kernel/vifast-plat-*`; chỉ asm mới cần.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::boot::{ACPI_PM1A_CNT, ACPI_SHUTDOWN_VALUE};
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::serial::{self, COM1_BASE, REG_LSR, TX_WAIT_LIMIT};

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use core::arch::asm;

// **KT-401** — tách cơ học tệp nóng về `< 950` dòng: `trap`/`syscall`/`fpu`.
mod fpu;
mod syscall;
mod trap;

pub use fpu::{read_cr0, read_cr4, write_cr0};
pub use trap::*;

// ---------------------------------------------------------------------------------------------
// Preemption — KT-286 (K5' bước 8), bàn giao `rsp` sửa ở KT-298
// ---------------------------------------------------------------------------------------------

// **KT-298** — *không còn* ô `static` bàn giao `rsp` ở đây.
//
// KT-286 đặt `rsp` của tác vụ kế vào `static VIFAST_SCHED_NEW_RSP` để stub chung đọc. Ô ấy
// **dùng chung mọi hart**: hart phụ nhận tick PIT trong khoảng giữa lúc handler ghi ô và lúc
// stub đọc ô sẽ **lấy trộm** giá trị của BSP ⇒ `mov rsp, <frame của BSP>` ⇒ hai hart chạy
// **cùng** một ngữ cảnh trên **cùng** một stack ⇒ `#GP` storm (boot 410/600 của KT-293).
//
// Bàn giao nay đi qua **thanh ghi** `rdx` của giá trị trả `exc_dispatch`
// ([`crate::sched::FrameReturn`]) — per-hart theo định nghĩa, không có cửa sổ, không cần khoá.

// ---------------------------------------------------------------------------------------------
// Cổng I/O
// ---------------------------------------------------------------------------------------------

/// Ghi một byte ra cổng I/O 8-bit (`out dx, al`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn outb(port: u16, value: u8) {
    // SAFETY: `out` chỉ ghi cổng I/O; `nomem`/`nostack` đúng vì không đụng bộ nhớ/stack.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Ghi một word ra cổng I/O 16-bit (`out dx, ax`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn outw(port: u16, value: u16) {
    // SAFETY: như `outb`.
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Đọc một byte từ cổng I/O 8-bit (`in al, dx`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: `in` chỉ đọc cổng I/O; không đụng bộ nhớ.
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Ghi một dword ra cổng I/O 32-bit (`out dx, eax`) — **KT-274**: `CONFIG_ADDRESS` `0xCF8`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn outl(port: u16, value: u32) {
    // SAFETY: `out` chỉ ghi cổng I/O; `nomem`/`nostack` đúng vì không đụng bộ nhớ/stack.
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Đọc một dword từ cổng I/O 32-bit (`in eax, dx`) — **KT-274**: `CONFIG_DATA` `0xCFC`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: `in` chỉ đọc cổng I/O; không đụng bộ nhớ.
    unsafe {
        asm!(
            "in eax, dx",
            in("dx") port,
            out("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

// ---------------------------------------------------------------------------------------------
// Thanh ghi điều khiển + MSR
// ---------------------------------------------------------------------------------------------

/// Ghi `CR3` — **KT-340a**: dùng `mov cr3, cr3` để **xả TLB** sau khi đổi bảng trang.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_cr3(value: u64) {
    // SAFETY: `mov cr3, r64` ở CPL0; caller truyền địa chỉ PML4 vật lý hợp lệ.
    unsafe {
        asm!("mov cr3, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

/// `invlpg` cho **một** trang — **KT-340a** (sau khi đổi lá bảng trang).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn invlpg(va: u64) {
    // SAFETY: `invlpg` ở CPL0; `va` là địa chỉ canonical bất kỳ.
    unsafe {
        asm!("invlpg [{}]", in(reg) va, options(nostack, preserves_flags));
    }
}

/// Đọc `CR3`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: như `read_cr0`.
    unsafe {
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// **KT-340d** — `CPUID.01H` → `(eax, ecx, edx)`; đo `PCID` (`ECX[17]`) và `PGE` (`EDX[13]`).
///
/// Vì sao cần đo thay vì giả định: đóng lỗi v1 **N5** phải **chứng minh** lựa chọn "tắt `PCID`
/// có kiểm soát" là đúng trên CPU đang chạy — `-cpu qemu64` **không** có `PCID`, `-cpu max` **có**.
/// `CPUID` không truy cập bộ nhớ, không tác dụng phụ; `EBX` để LLVM tự cấp thanh ghi.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn cpuid_leaf1() -> (u32, u32, u32) {
    let eax: u32;
    let ecx: u32;
    let edx: u32;
    // `rbx` do LLVM giữ riêng (không được là toán hạng `asm!`) nên `cpuid` phải **tự** cứu/khôi
    // phục: ghi `rbx` ra một thanh ghi tạm trước, `cpuid`, rồi trả lại. Ta **không** cần `ebx`
    // (`PCID` ở `ECX[17]`, `PGE` ở `EDX[13]`).
    // SAFETY: `cpuid` chỉ đọc thanh ghi; `eax = 1` là leaf hợp lệ trên mọi CPU ≥ 486.
    unsafe {
        asm!(
            "mov {save:r}, rbx",
            "cpuid",
            "mov rbx, {save:r}",
            save = out(reg) _,
            inout("eax") 1u32 => eax,
            out("ecx") ecx,
            out("edx") edx,
            options(nomem, nostack, preserves_flags)
        );
    }
    (eax, ecx, edx)
}

/// Đọc MSR 64-bit (`rdmsr`): `EDX:EAX` → `u64`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: `rdmsr` ở CPL0 với MSR hợp lệ; không đụng bộ nhớ.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

/// Ghi MSR 64-bit (`wrmsr`): `u64` → `EDX:EAX`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    // SAFETY: `wrmsr` ở CPL0 với MSR hợp lệ; không đụng bộ nhớ.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Đọc 32-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng để đọc `hvm_start_info.magic` mà `%ebx` trỏ tới — bằng chứng giao thức PVH. Địa chỉ nằm
/// trong identity map 1 GiB nên không fault kể cả khi `%ebx` rác.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u32(addr: u64) -> u32 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map 1 GiB của ảnh boot).
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// Ghi 32-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng cho **MMIO** của LAPIC/IOAPIC (KT-266): địa chỉ nằm trong MMIO map 3..4 GiB mà ảnh dựng
/// ở `_start`; `volatile` để compiler không gộp/bỏ lần ghi.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u32(addr: u64, value: u32) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (LAPIC/IOAPIC).
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}

/// Đọc 64-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng cho bảng stub/`GDTR`/`IDTR` — mọi địa chỉ ấy nằm trong identity map 1 GiB của ảnh.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u64(addr: u64) -> u64 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map 1 GiB của ảnh boot).
    unsafe { core::ptr::read_volatile(addr as *const u64) }
}

/// Đọc 16-bit tại địa chỉ **vật lý** — **KT-274**: thanh ghi cấu hình PCI (ECAM) và `used->idx`
/// của virtqueue (đọc `volatile` để compiler **không** hoist khỏi vòng poll DMA).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u16(addr: u64) -> u16 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map/MMIO map của ảnh).
    unsafe { core::ptr::read_volatile(addr as *const u16) }
}

/// Ghi 8-bit tại địa chỉ **vật lý** — **KT-274**: thanh ghi `device_status`/`queue_enable` của
/// virtio-pci (MMIO).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u8(addr: u64, value: u8) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (BAR virtio).
    unsafe { core::ptr::write_volatile(addr as *mut u8, value) }
}

/// Ghi 16-bit tại địa chỉ **vật lý** — **KT-274**: `queue_select`/`queue_size`/cửa thông báo
/// (notify) của virtio-pci.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u16(addr: u64, value: u16) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (BAR virtio).
    unsafe { core::ptr::write_volatile(addr as *mut u16, value) }
}

/// Ghi 64-bit tại địa chỉ **vật lý** — **KT-274**: `queue_desc`/`queue_driver`/`queue_device`
/// của virtio-pci (địa chỉ 64 bit).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u64(addr: u64, value: u64) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (BAR virtio).
    unsafe { core::ptr::write_volatile(addr as *mut u64, value) }
}

/// Đọc `CR2` — địa chỉ gây `#PF` (SDM Vol 3A §6.15).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr2() -> u64 {
    let value: u64;
    // SAFETY: đọc thanh ghi điều khiển ở CPL0; không đụng bộ nhớ.
    unsafe {
        asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

// ---------------------------------------------------------------------------------------------
// COM1 một byte + PIT kênh 2 — **KT-340c** (dùng cho `SyscallEnv::put_str`/`now_ns`)
// ---------------------------------------------------------------------------------------------

/// Ghi **một byte** ra COM1: chờ `LSR.THRE` (**có trần**) rồi ghi `THR`.
///
/// Trần là **số vòng**, không phải thời gian (UART hỏng không được treo máy). Hàm này **không**
/// giữ khoá COM1 — caller giữ (`com::COM1`) để cả một chuỗi ra nguyên vẹn (KT-316). Trước
/// KT-340c đường này nằm trong ảnh (`vifast-kbin-x86_64::write_byte`); đưa xuống crate arch để
/// `syscall_env::X86_64Env::put_str` (cũng ở crate arch) dùng **cùng** một đường byte, không
/// sinh đường in thứ hai (D2-83: core không đọc/in bộ nhớ user).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn com1_write_byte(byte: u8) {
    use crate::serial;
    let mut spins = 0u32;
    while spins < serial::TX_WAIT_LIMIT {
        if crate::asm::inb(serial::COM1_BASE + serial::REG_LSR) & serial::LSR_THRE != 0 {
            break;
        }
        spins += 1;
    }
    crate::asm::outb(serial::COM1_BASE + serial::REG_DATA, byte);
}

/// Chốt + đọc 16-bit counter **PIT kênh 2** (mode 2 đã lập trình lúc đo timer).
///
/// Cùng chuỗi lệnh với `vifast-kbin-x86_64::pit2_latch_read` (KT-274), đưa xuống crate arch để
/// `syscall_env::X86_64Env::now_ns` có đồng hồ đơn điệu mà **không** phụ thuộc ảnh.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_pit2_latch() -> u16 {
    use crate::apic;
    crate::asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_LATCH_CH2);
    let lo = crate::asm::inb(apic::PIT_CH2_DATA) as u16;
    let hi = crate::asm::inb(apic::PIT_CH2_DATA) as u16;
    (hi << 8) | lo
}

// ---------------------------------------------------------------------------------------------
// Ngắt — KT-266 (K5' bước 3)
// ---------------------------------------------------------------------------------------------

/// `sti` — bật ngắt che được (`RFLAGS.IF = 1`).
///
/// Sau lệnh này ngắt (PIT qua IOAPIC → LAPIC → vector 32) bắt đầu được CPU nhận. Ảnh gọi **một
/// lần** ngay trước khi đo tick; mọi phần còn lại của boot chạy với ngắt tắt (trạng thái từ
/// `_start`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn sti() {
    // SAFETY: `sti` ở CPL0; không đụng bộ nhớ/stack.
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

/// `hlt` — dừng CPU tới **ngắt kế tiếp** (không đụng `RFLAGS.IF`).
///
/// Dùng trong vòng chờ khi ngắt đã bật: nhường BQL để QEMU xử lý timer, thay vì busy-wait giữ
/// BQL và làm ngắt bị trễ hàng trăm ms (đo được ở KT-266).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn hlt() {
    // SAFETY: `hlt` ở CPL0; không đụng bộ nhớ/stack.
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

/// `cli` — tắt ngắt che được (`RFLAGS.IF = 0`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn cli() {
    // SAFETY: `cli` ở CPL0; không đụng bộ nhớ/stack.
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

/// Đọc `RFLAGS.IF` — `true` ⇔ ngắt che được đang **bật**.
///
/// **KT-316** — khoá COM1 cần biết trạng thái ngắt **trước** khi `cli` để khôi phục đúng: gọi từ
/// handler ngắt (`IF = 0`) thì nhả khoá **không** được `sti` (sẽ bật ngắt giữa handler); gọi từ
/// luồng thường (`IF = 1`) thì phải `sti` lại. `pushfq`/`pop` là cặp **cân bằng stack**, không
/// đụng bộ nhớ khác và không đổi cờ.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    // SAFETY: `pushfq`/`pop` ở CPL0; cặp lệnh cân bằng stack, chỉ đọc `RFLAGS`.
    unsafe {
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
    }
    flags & (1 << 9) != 0
}

/// Đọc một byte tại địa chỉ **user** (identity map ⇒ VA == PA) — dùng cho `DebugPutStr` từ ring 3.
///
/// Kernel đọc được trang user vì `CR4.SMAP = 0` (chưa bật SMAP); nếu bật SMAP, lần đọc đầu fault
/// và handler in `# exc: vec=14` thay vì im lặng.
///
/// **KT-293** — cũng dùng cho [`crate::image::image_digest`]: băm `.text` **đã nạp** từng byte
/// một (D2-80). Đọc `u64` ở đuôi section sẽ vượt `__text_end` và băm cả byte của section kế tiếp
/// ⇒ hash khác bản băm từ ELF; đọc byte là cách duy nhất khớp tuyệt đối.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u8(addr: u64) -> u8 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (trang user của ring 3 / `.text` của ảnh).
    unsafe { core::ptr::read_volatile(addr as *const u8) }
}

// ---------------------------------------------------------------------------------------------
// Dừng máy
// ---------------------------------------------------------------------------------------------

/// Chờ bộ truyền UART rỗng hẳn (`LSR.TEMT`) — **có trần** [`TX_WAIT_LIMIT`] — rồi:
///
/// - `code == 0`: ACPI shutdown `outw(0x604, 0x2000)` ⇒ QEMU thoát mã 0;
/// - `code != 0`: **không** shutdown, `cli; hlt` ⇒ QEMU treo ⇒ runner ĐỎ.
///
/// Không bao giờ trả về.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn halt(code: u32) -> ! {
    let mut spins = 0u32;
    while spins < TX_WAIT_LIMIT {
        if inb(COM1_BASE + REG_LSR) & serial::LSR_TEMT != 0 {
            break;
        }
        spins += 1;
    }
    if code == 0 {
        outw(ACPI_PM1A_CNT, ACPI_SHUTDOWN_VALUE);
    }
    loop {
        // SAFETY: `cli; hlt` ở CPL0; `nomem`/`nostack` đúng.
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Host stub — **panic**, không im lặng (cùng khuôn `vifast-plat-aarch64::asm`)
// ---------------------------------------------------------------------------------------------

/// Stub host: gọi asm đặc quyền trên máy thật là lỗi lập trình, phải **kêu to**.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
fn host_only(name: &str) -> ! {
    panic!("vifast_plat_x86_64::asm::{name}: chỉ chạy trên x86_64 bare-metal (target_os = none)")
}
/// Stub host của [`outb`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn outb(_port: u16, _value: u8) {
    host_only("outb")
}

/// Stub host của [`outw`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn outw(_port: u16, _value: u16) {
    host_only("outw")
}

/// Stub host của [`inb`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn inb(_port: u16) -> u8 {
    host_only("inb")
}

/// Stub host của [`outl`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn outl(_port: u16, _value: u32) {
    host_only("outl")
}

/// Stub host của [`inl`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn inl(_port: u16) -> u32 {
    host_only("inl")
}

/// Stub host của [`read_cr3`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr3() -> u64 {
    host_only("read_cr3")
}

/// Stub host của [`write_cr3`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_cr3(_value: u64) {
    host_only("write_cr3")
}

/// Stub host của [`invlpg`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn invlpg(_va: u64) {
    host_only("invlpg")
}

/// Stub host của [`read_msr`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_msr(_msr: u32) -> u64 {
    host_only("read_msr")
}

/// Stub host của [`cpuid_leaf1`] (**KT-340d**).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn cpuid_leaf1() -> (u32, u32, u32) {
    host_only("cpuid_leaf1")
}

/// Stub host của [`write_msr`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_msr(_msr: u32, _value: u64) {
    host_only("write_msr")
}

/// Stub host của [`write_u32`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u32(_addr: u64, _value: u32) {
    host_only("write_u32")
}

/// Stub host của [`sti`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn sti() {
    host_only("sti")
}

/// Stub host của [`cli`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn cli() {
    host_only("cli")
}

/// Stub host của [`interrupts_enabled`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn interrupts_enabled() -> bool {
    host_only("interrupts_enabled")
}

/// Stub host của [`hlt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn hlt() {
    host_only("hlt")
}

/// Stub host của [`read_u32`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u32(_addr: u64) -> u32 {
    host_only("read_u32")
}

/// Stub host của [`halt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn halt(_code: u32) -> ! {
    host_only("halt")
}

/// Stub host của [`read_u64`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u64(_addr: u64) -> u64 {
    host_only("read_u64")
}

/// Stub host của [`read_u16`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u16(_addr: u64) -> u16 {
    host_only("read_u16")
}

/// Stub host của [`write_u8`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u8(_addr: u64, _value: u8) {
    host_only("write_u8")
}

/// Stub host của [`write_u16`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u16(_addr: u64, _value: u16) {
    host_only("write_u16")
}

/// Stub host của [`write_u64`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u64(_addr: u64, _value: u64) {
    host_only("write_u64")
}

/// Stub host của [`read_cr2`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr2() -> u64 {
    host_only("read_cr2")
}
/// Stub host của [`com1_write_byte`] (**KT-340c**).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn com1_write_byte(_byte: u8) {
    host_only("com1_write_byte")
}

/// Stub host của [`read_pit2_latch`] (**KT-340c**).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_pit2_latch() -> u16 {
    host_only("read_pit2_latch")
}
/// Stub host của [`read_u8`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u8(_addr: u64) -> u8 {
    host_only("read_u8")
}
