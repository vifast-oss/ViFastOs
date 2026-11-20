// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **x86_64** của ViFastOs — KT-257 (K5' bước 1).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-x86_64 -m 128M -nographic -kernel <elf> -no-reboot
//!        │
//!        ├─ QEMU đọc **PVH note** (`.note.Xen`, type 18) ⇒ entry 32-bit = `_start`
//!        ├─ _start (32-bit protected mode): cli/cld → esp = __stack_top → zero .bss
//!        │    → dựng PML4/PDPT/PD (identity 1 GiB, 2 MiB/page) → CR4.PAE → CR3
//!        │    → EFER.LME → CR0.PG|PE → lgdt → ljmp 0x08:long_mode_entry
//!        ├─ long_mode_entry (64-bit): DS/ES/SS = 0x10 → rsp = __stack_top → kmain(eax, ebx)
//!        ├─ kmain: COM1 115200 8N1 → in bằng chứng (CR0/CR3/CR4/EFER.LMA, PVH magic)
//!        │    → marker **một** dòng → dòng halt → ACPI shutdown
//!        └─ asm::halt: chờ LSR.TEMT (có trần) → outw(0x604, 0x2000) ⇒ QEMU thoát mã 0
//! ```
//!
//! # Vì sao có **hai** header
//!
//! QEMU 11.1.3 `-kernel` cho x86 **không** dùng multiboot2: `hw/i386/multiboot.c` chỉ tìm magic
//! multiboot **1** (`0x1BADB002`) và từ chối ELF64 (`"Cannot load x86-64 image, give a 32bit
//! one."`). Đường QEMU thật sự nhận ELF64 là **PVH** (`hw/i386/x86-common.c:load_elfboot`): nó
//! đọc note `"Xen"` type 18 trong `PT_NOTE` để lấy entry 32-bit. Vì vậy ảnh mang:
//!
//! - **multiboot2 header** (đúng đặc tả, 24 byte, đầu `.text`) — để GRUB/multiboot2 cũng nạp được;
//! - **PVH note** — đường `qemu-system-x86_64 -kernel <ELF>` thực tế dùng.
//!
//! `_start` **không** phụ thuộc giao thức: nó chỉ ghi lại `eax`/`ebx` để `kmain` in bằng chứng.
//!
//! # Phạm vi (D2-200, KT-257)
//!
//! **Không** IDT/GDT đầy đủ (chỉ GDT tối thiểu để vào long mode), **không** APIC, **không** timer,
//! **không** U-mode, **không** PCI, **không** SMP, **không** `-cpu max`. **Không** tuyên bố K5'
//! đạt: tiêu chí K5' là "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot, D2-149) — đây là bước 1.
//!
//! # Vì sao ảnh **không** có `unsafe`
//!
//! Luật R5 (`v2/bin/v2 rules`) chỉ cho `unsafe` trong `kernel/vifast-plat-*`/`boot/`. Ảnh này chỉ
//! dùng `global_asm!` (macro **không** cần `unsafe`) cho `_start` 32-bit + `long_mode_entry`; mọi
//! lệnh đặc quyền gọi từ Rust (`in`/`out`/`mov crN`/`rdmsr`/`hlt`) nằm ở
//! [`vifast_plat_x86_64::asm`] — crate `kernel/vifast-plat-*` mà D2-17 cho phép `unsafe`. Nhờ vậy
//! ảnh giữ đúng luật R5 mà vẫn có đủ đường boot.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::{self, Write};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{CR0_PE, CR0_PG, CR4_PAE};
use vifast_plat_x86_64::serial;

/// Dòng marker **duy nhất** của ảnh — runner (`v2/test/boot/run-x86_64.sh`) so **nguyên văn** dòng
/// này và đòi nó xuất hiện **đúng một lần**.
#[cfg(not(feature = "wrong-marker"))]
pub const BOOT_MARKER: &str = "# viFastOs x86_64: boot ok\n";
/// Ca âm của runner: **cùng** ảnh, chỉ đổi chuỗi marker (feature `wrong-marker`).
#[cfg(feature = "wrong-marker")]
pub const BOOT_MARKER: &str = "# viFastOs x86_64: boot BAD\n";

/// Dòng in **trước** ACPI shutdown — chứng minh thoát sạch, không triple-fault.
pub const HALT_LINE: &str = "# viFastOs x86_64: halt";

// ---------------------------------------------------------------------------------------------
// Header boot + entry 32-bit + long mode
// ---------------------------------------------------------------------------------------------
//
// Toàn bộ nằm trong **một** `global_asm!` để thứ tự section trong tệp là tường minh: `.multiboot`
// (header multiboot2) → `.note.Xen` (PVH) → `.text.entry` (`_start` 32-bit) → `.code64`
// (`long_mode_entry`) → `.bss` (bảng trang) → `.data` (GDT).
global_asm!(
    // ---- multiboot2 header: magic, arch=0 (i386 protected mode), len=24, checksum, end tag.
    ".section .multiboot, \"a\"",
    ".align 8",
    ".long {mb_magic}",
    ".long {mb_arch}",
    ".long {mb_len}",
    ".long {mb_checksum}",
    ".short 0", // end tag: type
    ".short 0", // end tag: flags
    ".long {mb_end_size}",
    // ---- PVH note: namesz=4, descsz=4, type=18, "Xen\0", desc = địa chỉ entry 32-bit.
    ".section .note.Xen, \"a\", @note",
    ".align 4",
    ".long 4",
    ".long {pvh_desc_bytes}",
    ".long {pvh_type}",
    ".asciz \"Xen\"",
    ".align 4",
    ".long _start",
    ".align 4",
    // ---- entry 32-bit (QEMU nhảy vào đây ở protected mode, paging TẮT).
    ".section .text.entry, \"ax\"",
    ".code32",
    ".globl _start",
    "_start:",
    "cli",
    "cld",
    // 1. stack: trước mọi lệnh dùng stack.
    "mov esp, offset __stack_top",
    // 2. giữ eax/ebx của giao thức boot qua bước zero .bss (esi/ebp không bị `rep stosb` đụng).
    "mov esi, eax",
    "mov ebp, ebx",
    // 3. zero .bss: bảng trang + biến boot nằm ở đó, không dựa vào "QEMU tình cờ trả 0".
    "mov edi, offset __bss_start",
    "mov ecx, offset __bss_end",
    "sub ecx, edi",
    "xor eax, eax",
    "rep stosb",
    "mov dword ptr [__boot_eax], esi",
    "mov dword ptr [__boot_ebx], ebp",
    // 4. PML4[0] = __pdpt | P|RW.
    "mov edi, offset __pml4",
    "mov eax, offset __pdpt",
    "or eax, 3",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    // 5. PDPT[0] = __pd | P|RW.
    "mov edi, offset __pdpt",
    "mov eax, offset __pd",
    "or eax, 3",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    // 6. PD[i] = i*2MiB | P|RW|PS — identity map 1 GiB bằng 512 lá 2 MiB.
    "mov edi, offset __pd",
    "mov eax, 0x83",
    "mov ecx, 512",
    "1:",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    "add eax, 0x200000",
    "add edi, 8",
    "dec ecx",
    "jnz 1b",
    // 7. CR4.PAE = 1 (bắt buộc trước CR3 cho long mode).
    "mov eax, cr4",
    "or eax, 0x20",
    "mov cr4, eax",
    // 8. CR3 = PML4 (địa chỉ vật lý).
    "mov eax, offset __pml4",
    "mov cr3, eax",
    // 9. IA32_EFER.LME = 1.
    "mov ecx, 0xC0000080",
    "rdmsr",
    "or eax, 0x100",
    "wrmsr",
    // 10. CR0.PG | CR0.PE — từ lệnh này paging bật, mọi địa chỉ đi qua bảng trang identity.
    "mov eax, cr0",
    "or eax, 0x80000001",
    "mov cr0, eax",
    // 11. GDT tối thiểu (null + code 64-bit + data) rồi `ljmp` vào đoạn code 64-bit.
    "lgdt [__gdt_ptr]",
    ".byte 0xEA", // ljmp ptr16:32 — mã hoá tường minh, không phụ thuộc cú pháp assembler
    ".long long_mode_entry",
    ".word {selector_code}",
    // ---- long mode (64-bit).
    ".code64",
    "long_mode_entry:",
    "mov ax, {selector_data}",
    "mov ds, ax",
    "mov es, ax",
    "mov ss, ax",
    "mov fs, ax",
    "mov gs, ax",
    "lea rsp, [rip + __stack_top]",
    "mov edi, dword ptr [rip + __boot_eax]",
    "mov esi, dword ptr [rip + __boot_ebx]",
    "call kmain",
    "2:",
    "cli",
    "hlt",
    "jmp 2b",
    // ---- .bss: biến boot + ba bảng trang 4 KiB (căn 4 KiB).
    ".section .bss",
    ".align 8",
    "__boot_eax:",
    ".skip 4",
    "__boot_ebx:",
    ".skip 4",
    ".align 4096",
    "__pml4:",
    ".skip 4096",
    ".align 4096",
    "__pdpt:",
    ".skip 4096",
    ".align 4096",
    "__pd:",
    ".skip 4096",
    // ---- .data: GDT + con trỏ `lgdt` (6 byte: limit 16-bit + base 32-bit).
    ".section .data",
    ".align 8",
    "__gdt:",
    ".quad {gdt_null}",
    ".quad {gdt_code}",
    ".quad {gdt_data}",
    "__gdt_end:",
    "__gdt_ptr:",
    ".word __gdt_end - __gdt - 1",
    ".long __gdt",
    mb_magic = const boot::MULTIBOOT2_MAGIC,
    mb_arch = const boot::MULTIBOOT2_ARCH_I386,
    mb_len = const boot::MULTIBOOT2_HEADER_LEN,
    mb_checksum = const boot::MULTIBOOT2_CHECKSUM,
    mb_end_size = const boot::MULTIBOOT2_END_TAG_SIZE,
    pvh_type = const boot::PVH_NOTE_TYPE,
    pvh_desc_bytes = const boot::PVH_NOTE_DESC_BYTES,
    selector_code = const boot::SELECTOR_CODE,
    selector_data = const boot::SELECTOR_DATA,
    gdt_null = const boot::GDT_NULL,
    gdt_code = const boot::GDT_KERNEL_CODE,
    gdt_data = const boot::GDT_KERNEL_DATA,
);

// ---------------------------------------------------------------------------------------------
// Console tối giản trên COM1
// ---------------------------------------------------------------------------------------------

/// `core::fmt::Write` trên COM1 — để `panic!`/`write!` in được số mà không cần `alloc`.
struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            write_byte(byte);
        }
        Ok(())
    }
}

/// Khởi tạo COM1 115200 8N1 theo [`serial::init_sequence`]; trả divisor đã dùng để in bằng chứng.
fn serial_init() -> (u8, u8) {
    let divisor = serial::divisor(serial::BAUD_115200).unwrap_or(serial::DIVISOR_FALLBACK);
    for step in serial::init_sequence(divisor) {
        asm::outb(serial::COM1_BASE + step.offset, step.value);
    }
    divisor
}

/// Ghi một byte: chờ `LSR.THRE` (**có trần** [`serial::TX_WAIT_LIMIT`]) rồi ghi `THR`.
///
/// Trần là **số vòng**, không phải thời gian: UART hỏng không được treo máy, và một lần ghi bình
/// thường không bao giờ chạm trần.
fn write_byte(byte: u8) {
    let mut spins = 0u32;
    while spins < serial::TX_WAIT_LIMIT {
        if asm::inb(serial::COM1_BASE + serial::REG_LSR) & serial::LSR_THRE != 0 {
            break;
        }
        spins += 1;
    }
    asm::outb(serial::COM1_BASE + serial::REG_DATA, byte);
}

/// `#[panic_handler]` của ảnh: in `PANIC` + thông điệp rồi tắt máy (QEMU treo ⇒ runner đỏ).
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let mut console = Console;
    let _ = writeln!(console, "PANIC: {info}");
    asm::halt(1)
}

// ---------------------------------------------------------------------------------------------
// kmain
// ---------------------------------------------------------------------------------------------

/// Điểm vào Rust, được `long_mode_entry` gọi **sau** khi long mode đã bật.
///
/// `boot_eax`/`boot_ebx` là hai thanh ghi giao thức boot lúc `_start` (PVH: `ebx` =
/// `hvm_start_info`; multiboot2: `eax = 0x36D76289`, `ebx` = MBI). Chúng chỉ dùng để **in bằng
/// chứng**, không điều khiển đường boot.
#[no_mangle]
pub extern "C" fn kmain(boot_eax: u32, boot_ebx: u32) -> ! {
    let divisor = serial_init();
    let mut console = Console;

    let cr0 = asm::read_cr0();
    let cr3 = asm::read_cr3();
    let cr4 = asm::read_cr4();
    let efer = asm::read_msr(boot::LONG_MODE_EFER_MSR);
    let lma = (efer & boot::LONG_MODE_EFER_CHECK) != 0;
    let pg = (cr0 & CR0_PG) != 0;
    let pe = (cr0 & CR0_PE) != 0;
    let pae = (cr4 & CR4_PAE) != 0;
    let pvh_magic = asm::read_u32(boot_ebx as u64);
    let pvh_ok = pvh_magic == boot::PVH_START_MAGIC;

    // Bằng chứng long mode: đọc lại **thanh ghi** sau khi bật, không chỉ tin asm.
    let _ = writeln!(
        console,
        "# viFastOs x86_64: long mode cr0=0x{cr0:016x} cr4=0x{cr4:016x} cr3=0x{cr3:016x} efer=0x{efer:016x} lma={}",
        lma as u8,
    );
    let _ = writeln!(
        console,
        "# viFastOs x86_64: boot eax=0x{boot_eax:08x} ebx=0x{boot_ebx:08x} pvh_magic=0x{pvh_magic:08x} pvh_ok={}",
        pvh_ok as u8,
    );
    let _ = writeln!(
        console,
        "# viFastOs x86_64: serial com1=0x{:03x} divisor={} lcr=0x{:02x}",
        serial::COM1_BASE,
        (divisor.0 as u16) | ((divisor.1 as u16) << 8),
        serial::LCR_8N1,
    );
    let _ = writeln!(
        console,
        "# viFastOs x86_64: identity map 0x0..0x{:x} pages={} pml4=0x{:x}",
        boot::IDENTITY_MAP_BYTES,
        boot::PD_ENTRIES,
        cr3 & !0xFFF,
    );

    // Marker **chỉ** in khi long mode thật sự bật (LMA + PG + PE + PAE). Nếu in sớm, một ảnh hỏng
    // vẫn có marker và runner dễ nhận nhầm là boot sạch.
    if !(lma && pg && pe && pae) {
        let _ = writeln!(
            console,
            "PANIC: long mode chưa bật (lma={} pg={} pe={} pae={})",
            lma as u8, pg as u8, pe as u8, pae as u8,
        );
        asm::halt(1)
    }

    let _ = write!(console, "{BOOT_MARKER}");
    let _ = writeln!(console, "{HALT_LINE}");
    asm::halt(0)
}
