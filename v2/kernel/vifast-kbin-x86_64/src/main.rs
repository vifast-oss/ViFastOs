// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **x86_64** của ViFastOs — KT-257 (K5' bước 1) + KT-261 (K5' bước 2: GDT/TSS/IDT).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-x86_64 -m 128M -nographic -kernel <elf> -no-reboot
//!        │
//!        ├─ QEMU đọc **PVH note** (`.note.Xen`, type 18) ⇒ entry 32-bit = `_start`
//!        ├─ _start (32-bit protected mode): cli/cld → esp = __stack_top → zero .bss
//!        │    → dựng PML4/PDPT/PD (identity 1 GiB, 2 MiB/page) → CR4.PAE → CR3
//!        │    → EFER.LME → CR0.PG|PE → lgdt (GDT tạm 3 entry) → ljmp 0x08:long_mode_entry
//!        ├─ long_mode_entry (64-bit): DS/ES/SS = 0x10 → rsp = __stack_top
//!        │    → kmain(eax, ebx, __stack_top, __ist1_top)
//!        ├─ kmain: COM1 115200 8N1 → TAP version/plan → in bằng chứng long mode
//!        │    → TSS (RSP0 + IST1) → GDT thật 7 entry + `ltr` → IDT 256 ô + `lidt`
//!        │    → TAP 1..7 (gdt/idt loaded + int3/ud2/#PF/#GP/unknown) → marker → halt
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
//! # KT-261 — GDT/TSS/IDT + handler exception
//!
//! - **GDT thật** 7 entry (null, kernel code/data, user code/data, TSS 16 byte) nằm ở
//!   [`vifast_plat_x86_64::gdt`]; `_start` vẫn dùng **GDT tạm** 3 entry để vào long mode (cùng
//!   selector `0x08`/`0x10`, cùng giá trị descriptor) rồi `kmain` nạp bảng thật.
//! - **TSS** ([`vifast_plat_x86_64::tss`]): `RSP0` = đỉnh stack kernel, `IST1` = stack riêng cho
//!   `#DF` (vector 8), `I/O map base = 104` (không bitmap).
//! - **IDT** ([`vifast_plat_x86_64::idt`]): **256** ô × 16 byte, mọi ô `P=1`; 14 vector có stub
//!   riêng, còn lại trỏ `isr_unknown` (đẩy sentinel [`exc::VEC_UNKNOWN`]).
//! - **Handler** [`exc_dispatch`]: in `# exc: vec=… err=0x… rip=0x… cs=0x… rflags=0x…` (thêm
//!   `# exc: cr2=0x…` cho `#PF`), ghi lại bằng chứng, rồi **resume** (`iretq`) nếu exception là ca
//!   kiểm đã arm, hoặc **halt sạch** nếu không (không bao giờ quay lại chỗ lỗi).
//!
//! # Vì sao ảnh **không** có `unsafe`
//!
//! Luật R5 (`v2/bin/v2 rules`) chỉ cho `unsafe` trong `kernel/vifast-plat-*`/`boot/`. Ảnh này chỉ
//! dùng `global_asm!` (macro **không** cần `unsafe`) cho `_start` 32-bit + `long_mode_entry`; mọi
//! lệnh đặc quyền (`in`/`out`/`mov crN`/`rdmsr`/`lgdt`/`lidt`/`ltr`/`hlt`) và **stub exception**
//! nằm ở [`vifast_plat_x86_64::asm`] — crate `kernel/vifast-plat-*` mà D2-17 cho phép `unsafe`.
//! Nhờ vậy ảnh giữ đúng luật R5 mà vẫn có đủ đường boot + xử lý exception.
//!
//! # Phạm vi (D2-203, KT-261)
//!
//! **Không** APIC, **không** timer, **không** U-mode, **không** PCI, **không** SMP, **không**
//! `-cpu max`. **Không** tuyên bố K5' đạt: tiêu chí K5' là "như K3" (TAP đầy đủ smp 1|2|4 + soak
//! 600 boot, D2-149) — đây là **bước 2**.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{CR0_PE, CR0_PG, CR4_PAE};
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::gdt;
use vifast_plat_x86_64::idt;
use vifast_plat_x86_64::serial;
use vifast_plat_x86_64::tss;

/// Dòng marker **duy nhất** của ảnh — runner (`v2/test/boot/run-x86_64.sh`) so **nguyên văn** dòng
/// này và đòi nó xuất hiện **đúng một lần**.
#[cfg(not(feature = "wrong-marker"))]
pub const BOOT_MARKER: &str = "# viFastOs x86_64: boot ok\n";
/// Ca âm của runner: **cùng** ảnh, chỉ đổi chuỗi marker (feature `wrong-marker`).
#[cfg(feature = "wrong-marker")]
pub const BOOT_MARKER: &str = "# viFastOs x86_64: boot BAD\n";

/// Dòng in **trước** ACPI shutdown — chứng minh thoát sạch, không triple-fault.
pub const HALT_LINE: &str = "# viFastOs x86_64: halt";

/// Số ca TAP mà ảnh phát ra (khớp `1..N` và `EXPECT_TESTS` của runner).
pub const TAP_TESTS: usize = 7;

/// Địa chỉ **canonical** nhưng **chưa map** dùng cho ca `#PF`: `0x0000_7000_0000_0000`.
///
/// Canonical vì bit 63..48 = 0 trùng bit 47 = 0 (4-level paging, 48-bit VA); chưa map vì
/// `PML4[224]` = 0 (identity map của ảnh chỉ phủ `0..1 GiB`). Đọc nó ⇒ `#PF` với `P = 0`.
pub const PF_UNMAPPED_ADDR: u64 = 0x0000_7000_0000_0000;

/// Selector **ngoài** GDT (bảng có 7 entry ⇒ limit 55; selector `0x38` = index 7 = byte 56).
///
/// Nạp nó vào `DS` ⇒ `#GP(0x38)` theo SDM Vol 3A §5.4.1 ("segment selector index outside the
/// descriptor table limits") — cách sinh `#GP` **chắc chắn** và **không đổi trạng thái** (DS giữ
/// nguyên khi fault), khác với `mov cr0` (có thể tắt paging nếu QEMU không fault).
pub const GP_BAD_SELECTOR: u16 = 0x38;

/// Sentinel "không chờ exception nào" — khác mọi vector thật (0..31) và [`exc::VEC_UNKNOWN`].
const NO_EXPECT: u64 = u64::MAX;

// ---------------------------------------------------------------------------------------------
// Header boot + entry 32-bit + long mode
// ---------------------------------------------------------------------------------------------
//
// Toàn bộ nằm trong **một** `global_asm!` để thứ tự section trong tệp là tường minh: `.multiboot`
// (header multiboot2) → `.note.Xen` (PVH) → `.text.entry` (`_start` 32-bit) → `.code64`
// (`long_mode_entry`) → `.bss` (bảng trang + stack IST) → `.data` (GDT tạm).
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
    // 11. GDT **tạm** (null + code 64-bit + data) rồi `ljmp` vào đoạn code 64-bit. GDT **thật**
    //     7 entry (thêm user + TSS) do `kmain` nạp sau, khi đã có Rust để dựng descriptor TSS.
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
    // kmain(boot_eax, boot_ebx, stack_top, ist1_top) — SysV: rdi, rsi, rdx, rcx.
    "mov edi, dword ptr [rip + __boot_eax]",
    "mov esi, dword ptr [rip + __boot_ebx]",
    "lea rdx, [rip + __stack_top]",
    "lea rcx, [rip + __ist1_top]",
    "call kmain",
    "2:",
    "cli",
    "hlt",
    "jmp 2b",
    // ---- .bss: biến boot + ba bảng trang 4 KiB + stack IST 4 KiB (căn 4 KiB).
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
    // Stack IST1 cho `#DF` (KT-261): 4 KiB riêng, `__ist1_top` = đỉnh (stack mọc xuống).
    ".align 4096",
    "__ist1_stack:",
    ".skip 4096",
    "__ist1_top:",
    // ---- .data: GDT **tạm** + con trỏ `lgdt` (6 byte: limit 16-bit + base 32-bit).
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
// TAP
// ---------------------------------------------------------------------------------------------

/// In một dòng TAP `ok N - <tên>` / `not ok N - <tên>`; trả `ok`.
///
/// TAP 13 (Test Anything Protocol): dòng `ok`/`not ok` phải bắt đầu dòng, số hiệu liên tục, và
/// runner (`run-x86_64.sh`) đòi **đủ** `1..N` dòng `ok`, **0** dòng `not ok`, và coi mọi
/// `not ok`/`PANIC` là đỏ.
fn tap(console: &mut Console, n: usize, name: &str, ok: bool) -> bool {
    let _ = writeln!(console, "{} {n} - {name}", if ok { "ok" } else { "not ok" });
    ok
}

/// Phát TAP `n`; **halt ngay** nếu đỏ (fail-fast: một ca sai thì các ca sau không còn nghĩa).
fn check(console: &mut Console, n: usize, name: &str, ok: bool) {
    if !tap(console, n, name, ok) {
        let _ = writeln!(console, "# TAP {n} FAIL — dừng (không in marker)");
        asm::halt(3)
    }
}

// ---------------------------------------------------------------------------------------------
// Trạng thái exception — handler ghi, ca kiểm đọc
// ---------------------------------------------------------------------------------------------

/// Số exception đã vào handler (bằng chứng "handler chạy thật", không chỉ in).
static EXC_COUNT: AtomicU64 = AtomicU64::new(0);
/// Vector của exception **gần nhất**.
static EXC_VEC: AtomicU64 = AtomicU64::new(NO_EXPECT);
/// Mã lỗi của exception gần nhất.
static EXC_ERR: AtomicU64 = AtomicU64::new(NO_EXPECT);
/// `rip` của exception gần nhất (CPU đẩy — **địa chỉ lệnh gây lỗi**).
static EXC_RIP: AtomicU64 = AtomicU64::new(0);
/// `cs` của exception gần nhất.
static EXC_CS: AtomicU64 = AtomicU64::new(0);
/// `rflags` của exception gần nhất.
static EXC_RFLAGS: AtomicU64 = AtomicU64::new(0);
/// `cr2` tại thời điểm exception gần nhất (chỉ có nghĩa với `#PF`).
static EXC_CR2: AtomicU64 = AtomicU64::new(0);
/// Vector mà ca kiểm **đang chờ** (`NO_EXPECT` = không chờ) — quyết định resume hay halt.
static EXPECT_VEC: AtomicU64 = AtomicU64::new(NO_EXPECT);
/// `rip` sẽ resume cho `#PF`/`#GP` (nhãn ngay sau lệnh gây lỗi).
static RESUME_RIP: AtomicU64 = AtomicU64::new(0);

/// Vector `#BP` dưới dạng `u64` — hằng để dùng làm pattern.
const VEC_BP: u64 = exc::VEC_BP as u64;
/// Vector `#UD`.
const VEC_UD: u64 = exc::VEC_UD as u64;
/// Vector `#GP`.
const VEC_GP: u64 = exc::VEC_GP as u64;
/// Vector `#PF`.
const VEC_PF: u64 = exc::VEC_PF as u64;

/// Xoá bằng chứng cũ trước một ca kiểm — nếu handler **không** chạy, `EXC_VEC` vẫn là
/// [`NO_EXPECT`] và ca kiểm đỏ (không thể "xanh nhờ dữ liệu cũ").
fn exc_reset() {
    EXC_VEC.store(NO_EXPECT, Ordering::Relaxed);
    EXC_ERR.store(NO_EXPECT, Ordering::Relaxed);
    EXC_RIP.store(0, Ordering::Relaxed);
    EXC_CS.store(0, Ordering::Relaxed);
    EXC_RFLAGS.store(0, Ordering::Relaxed);
    EXC_CR2.store(0, Ordering::Relaxed);
}

/// Arm ca kiểm: chờ `vec`, và (với `#PF`/`#GP`) resume ở `resume`.
fn exc_arm(vec: u64, resume: u64) {
    RESUME_RIP.store(resume, Ordering::Relaxed);
    EXPECT_VEC.store(vec, Ordering::Relaxed);
}

/// `true` ⇔ exception gần nhất đúng `(vec, err, rip)` và đến từ kernel (`cs = 0x08`).
fn exc_caught(vec: u64, err: u64, rip: u64) -> bool {
    EXC_VEC.load(Ordering::Relaxed) == vec
        && EXC_ERR.load(Ordering::Relaxed) == err
        && EXC_RIP.load(Ordering::Relaxed) == rip
        && EXC_CS.load(Ordering::Relaxed) == exc::KERNEL_CS
}

/// Handler **chung** của mọi exception — stub asm (`vifast_isr_common`) gọi hàm này.
///
/// Trả **`rip` để resume** (⇒ stub `iretq` về đó) hoặc **`0`** (⇒ stub halt sạch). Hàm **không**
/// bao giờ trả về nếu exception không phải ca kiểm đã arm: nó in nguyên nhân rồi `halt(2)` — che
/// một fault thật bằng cách resume là điều tệ nhất có thể làm.
///
/// # Vì sao nhận **giá trị** chứ không nhận con trỏ frame
///
/// Ảnh này **không** được có `unsafe` (R5), nên không thể deref con trỏ frame. Stub asm đọc 5
/// trường của frame và truyền thành 5 tham số SysV — cùng thông tin, không cần `unsafe`.
#[no_mangle]
pub extern "C" fn exc_dispatch(vec: u64, err: u64, rip: u64, cs: u64, rflags: u64) -> u64 {
    let cr2 = asm::read_cr2();
    let mut console = Console;
    let _ = writeln!(
        console,
        "# exc: vec={vec} err=0x{err:x} rip=0x{rip:x} cs=0x{cs:x} rflags=0x{rflags:x}"
    );
    if vec == VEC_PF {
        let _ = writeln!(console, "# exc: cr2=0x{cr2:x}");
    }

    EXC_COUNT.fetch_add(1, Ordering::Relaxed);
    EXC_VEC.store(vec, Ordering::Relaxed);
    EXC_ERR.store(err, Ordering::Relaxed);
    EXC_RIP.store(rip, Ordering::Relaxed);
    EXC_CS.store(cs, Ordering::Relaxed);
    EXC_RFLAGS.store(rflags, Ordering::Relaxed);
    EXC_CR2.store(cr2, Ordering::Relaxed);

    if EXPECT_VEC.load(Ordering::Relaxed) == vec {
        EXPECT_VEC.store(NO_EXPECT, Ordering::Relaxed);
        return match vec {
            // `INT3` là **software interrupt**: RIP đã lưu đã trỏ lệnh **kế tiếp** ⇒ resume nguyên.
            VEC_BP => rip,
            // `ud2` = 2 byte (`0F 0B`) ⇒ `rip + 2`.
            VEC_UD => rip + 2,
            // `int imm8` cũng là **software interrupt** ⇒ RIP đã lưu trỏ lệnh kế tiếp.
            exc::VEC_UNKNOWN => rip,
            // `#PF`/`#GP`: độ dài lệnh gây lỗi không biết chắc ⇒ nhảy tới nhãn resume do ca kiểm
            // đặt (ngay sau lệnh, cùng trạng thái stack).
            _ => RESUME_RIP.load(Ordering::Relaxed),
        };
    }

    // Exception **không** được chờ: in xong halt sạch — không quay lại chỗ lỗi.
    asm::halt(2)
}

// ---------------------------------------------------------------------------------------------
// kmain
// ---------------------------------------------------------------------------------------------

/// Điểm vào Rust, được `long_mode_entry` gọi **sau** khi long mode đã bật.
///
/// `boot_eax`/`boot_ebx` là hai thanh ghi giao thức boot lúc `_start` (PVH: `ebx` =
/// `hvm_start_info`; multiboot2: `eax = 0x36D76289`, `ebx` = MBI) — chỉ dùng để **in bằng chứng**.
/// `stack_top` = đỉnh stack kernel (`RSP0` của TSS), `ist1_top` = đỉnh stack IST1 cho `#DF`.
#[no_mangle]
pub extern "C" fn kmain(boot_eax: u32, boot_ebx: u32, stack_top: u64, ist1_top: u64) -> ! {
    let divisor = serial_init();
    let mut console = Console;

    // TAP 13: dòng phiên bản phải là dòng **đầu tiên**; plan `1..N` ngay sau.
    //
    // Xuống dòng **trước** là bắt buộc trên x86: SeaBIOS in `Booting from ROM..` **không** kèm
    // newline, nên nếu in thẳng thì dòng đầu log là `Booting from ROM..TAP version 13` và runner
    // (`grep -qx 'TAP version 13'`) đỏ oan. Một newline đặt TAP về đúng dòng đầu của **phần ảnh**.
    let _ = writeln!(console);
    let _ = writeln!(console, "TAP version 13");
    let _ = writeln!(console, "1..{TAP_TESTS}");

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

    // ---- KT-261: TSS → GDT thật (7 entry) + `ltr` → IDT 256 ô + `lidt`.
    //
    // Thứ tự là hợp đồng: TSS phải hợp lệ **trước** khi descriptor trỏ vào nó, và GDT phải nạp
    // **trước** `ltr` (nếu không `ltr` đọc bảng tạm 3 entry ⇒ #GP).
    tss::init(stack_top, ist1_top);
    gdt::install(tss::base());
    idt::install();

    // ---- TAP 1: GDT đã nạp **thật** — `sgdt`/`str` đọc lại khớp bảng + descriptor TSS.
    let (gdt_base, gdt_limit) = gdt::read_gdtr();
    let tr = asm::str_reg();
    // Sau `ltr`, CPU đánh dấu descriptor TSS là **busy** (`0x89 → 0x8B`) — kiểm dạng busy
    // là bằng chứng `ltr` đã nạp thật, không chỉ "đã gọi".
    let want_tss =
        gdt::tss_descriptor_type(tss::base(), tss::TSS_LIMIT as u32, gdt::TSS_TYPE_BUSY_64);
    let got_tss = [gdt::entry(gdt::INDEX_TSS), gdt::entry(gdt::INDEX_TSS + 1)];
    let gdt_ok = gdt_base == gdt::base()
        && gdt_limit == gdt::GDT_LIMIT
        && tr == gdt::SELECTOR_TSS
        && got_tss == want_tss
        && tss::rsp0() == stack_top
        && tss::ist(1) == ist1_top
        && tss::io_map_base() == tss::TSS_IO_MAP_BASE;
    let _ = writeln!(
        console,
        "# viFastOs x86_64: gdt base=0x{gdt_base:x} limit={gdt_limit} tr=0x{tr:x} tss=0x{:x} ist1=0x{ist1_top:x}",
        tss::base(),
    );
    check(&mut console, 1, "exc: gdt loaded", gdt_ok);

    // ---- TAP 2: IDT đã nạp **thật** — `sidt` khớp bảng, ô `#BP` trỏ stub, `#DF` có IST1.
    let (idt_base, idt_limit) = idt::read_idtr();
    let bp = exc::VEC_BP as usize;
    let df = exc::VEC_DF as usize;
    let idt_ok = idt_base == idt::base()
        && idt_limit == idt::IDT_LIMIT
        && idt::offset_of(bp) == asm::stub_addr(exc::STUB_BP)
        && idt::selector_of(bp) == gdt::SELECTOR_KERNEL_CODE
        && idt::type_attr_of(bp) == idt::GATE_INTERRUPT
        && idt::ist_of(bp) == 0
        && idt::ist_of(df) == idt::IST_DOUBLE_FAULT
        && idt::offset_of(df) == asm::stub_addr(exc::STUB_DF);
    let _ = writeln!(
        console,
        "# viFastOs x86_64: idt base=0x{idt_base:x} limit={idt_limit} entries={} df_ist={}",
        idt::IDT_ENTRIES,
        idt::ist_of(df),
    );
    check(&mut console, 2, "exc: idt loaded", idt_ok);

    // ---- TAP 3: `int3` ⇒ `#BP` (vector 3, err 0), resume ở lệnh kế tiếp.
    exc_reset();
    exc_arm(VEC_BP, 0);
    asm::probe_int3();
    check(
        &mut console,
        3,
        "exc: int3",
        exc_caught(VEC_BP, 0, asm::probe_int3_next()),
    );

    // ---- TAP 4: `ud2` ⇒ `#UD` (vector 6, err 0).
    exc_reset();
    exc_arm(VEC_UD, 0);
    asm::probe_ud2();
    check(
        &mut console,
        4,
        "exc: ud2",
        exc_caught(VEC_UD, 0, asm::probe_ud2_insn()),
    );

    // ---- TAP 5: đọc địa chỉ **chưa map** ⇒ `#PF` (vector 14, `P = 0`, `cr2` đúng).
    exc_reset();
    exc_arm(VEC_PF, asm::probe_pf_resume());
    asm::probe_pf(PF_UNMAPPED_ADDR);
    let pf_err = EXC_ERR.load(Ordering::Relaxed);
    let pf_ok = exc_caught(VEC_PF, 0, asm::probe_pf_insn())
        && (pf_err & exc::PF_ERR_PRESENT) == 0
        && EXC_CR2.load(Ordering::Relaxed) == PF_UNMAPPED_ADDR;
    check(&mut console, 5, "exc: page fault", pf_ok);

    // ---- TAP 6: nạp `DS` bằng selector **ngoài** GDT ⇒ `#GP` (vector 13, err = selector).
    exc_reset();
    exc_arm(VEC_GP, asm::probe_gp_resume());
    asm::probe_gp();
    check(
        &mut console,
        6,
        "exc: gp",
        exc_caught(VEC_GP, GP_BAD_SELECTOR as u64, asm::probe_gp_insn()),
    );

    // ---- TAP 7: `int 0x30` ⇒ ô `isr_unknown` (sentinel 255) — mọi vector khác vẫn **thấy được**.
    exc_reset();
    exc_arm(exc::VEC_UNKNOWN, asm::probe_unknown_resume());
    asm::probe_unknown();
    check(
        &mut console,
        7,
        "exc: unknown",
        exc_caught(exc::VEC_UNKNOWN, 0, asm::probe_unknown_resume()),
    );

    let _ = writeln!(
        console,
        "# viFastOs x86_64: exc count={} last_vec={}",
        EXC_COUNT.load(Ordering::Relaxed),
        EXC_VEC.load(Ordering::Relaxed),
    );

    let _ = write!(console, "{BOOT_MARKER}");
    let _ = writeln!(console, "{HALT_LINE}");
    asm::halt(0)
}
