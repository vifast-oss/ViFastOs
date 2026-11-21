// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **x86_64** của ViFastOs — KT-257 (K5' bước 1) + KT-261 (K5' bước 2: GDT/TSS/IDT) +
//! KT-266 (K5' bước 3: APIC + timer) + **KT-270** (K5' bước 4: U-mode ring 3 + `int 0x80`) +
//! KT-274 (K5' bước 5: PCI + virtio-net) + KT-276 (K5' bước 6: SMP 2 hart) + **KT-278** (K5' bước
//! 7: SMP **4 hart** + **IPI** + **TSS per-hart**).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-x86_64 -m 128M -nographic -kernel <elf> -no-reboot -smp 2
//!        │
//!        ├─ QEMU đọc **PVH note** (`.note.Xen`, type 18) ⇒ entry 32-bit = `_start`
//!        ├─ _start (32-bit protected mode): cli/cld → esp = __stack_top → zero .bss
//!        │    → dựng PML4/PDPT/PD (identity 1 GiB + **MMIO map 3..4 GiB**, 2 MiB/page)
//!        │    → CR4.PAE → CR3 → EFER.LME → CR0.PG|PE → lgdt (GDT tạm 3 entry)
//!        │    → ljmp 0x08:long_mode_entry
//!        ├─ long_mode_entry (64-bit): DS/ES/SS = 0x10 → rsp = __stack_top
//!        │    → kmain(eax, ebx, __stack_top, __ist1_top)
//!        ├─ kmain: COM1 115200 8N1 → TAP version/plan → in bằng chứng long mode
//!        │    → TSS (RSP0 + IST1) → GDT thật 7 entry + `ltr` → IDT 256 ô + `lidt`
//!        │    → TAP 1..7 (gdt/idt loaded + int3/ud2/#PF/#GP/unknown)
//!        │    → LAPIC (SVR/TPR) → IOAPIC (mask hết) → timer (LVT + initial count)
//!        │    → `sti` → TAP 8..11 (lapic id, ioapic, timer ticks, timer eoi) → `cli`
//!        │    → TSS.RSP0 = stack syscall riêng → `iretq` xuống ring 3 (CS=0x1B, SS=0x23, IF=1)
//!        │    ↓
//!        │  ring 3: ghi stack user → `int 0x80` DebugPutStr → handler in chuỗi user
//!        │    → TAP 12/13 + `# umode: ring3 ran nr=12 ret=14` → `iretq` về ring 3
//!        │    → `int 0x80` Exit → TAP 14 → PCI/net (TAP 15..17) → **SMP** (TAP 18..21)
//!        │    → **preemption** (TAP 22/23) → marker → halt
//!        └─ asm::halt: chờ LSR.TEMT (có trần) → outw(0x604, 0x2000) ⇒ QEMU thoát mã 0
//!
//! # KT-276 + KT-278 — SMP 4 hart, IPI, TSS per-hart (K5' bước 6 → bước 7)
//!
//! Sau TAP 17, [`smp::run`] copy trang trampoline (link 64 KiB-aligned trong `.text`) xuống
//! `0x8000`, rồi với **từng** hart 1..3 **tuần tự**: ghi boot block, `INIT` + `SIPI` + `SIPI` qua
//! `ICR` (`0x300`/`0x310`) tới `APIC ID = hart`, chờ hart ấy online. Hart phụ chạy trampoline
//! **real mode → long mode** (không qua 32-bit protected mode), nạp **TSS riêng + GDT riêng +
//! `ltr`** (KT-278), bật LAPIC **của nó** (`SVR`/`TPR`/`LINT`), in `# smp: hart=N online`, rồi
//! vòng `hlt` được đánh thức bằng LAPIC timer **của hart ấy** (vector 32 dùng chung stub; handler
//! phân biệt tick BSP/AP bằng `LAPIC_ID` đọc qua MMIO). BSP gửi **IPI** fixed-delivery (vector
//! `0x30`) tới hart 1 — hart 1 đếm trong handler và in `# smp: ipi recv=1 intid=0x30`. Cuối cùng
//! cả bốn hart in `# smp: hart=N alive`; TAP 18..21 chốt. **Không** scheduler/preemption,
//! **không** `-cpu max`, **không** `syscall`/`sysret`.
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
//! # Phạm vi (D2-203, KT-261; D2-204, KT-266; D2-207, KT-270)
//!
//! **Đã có**: GDT/TSS/IDT + handler exception (KT-261); LAPIC (ID/version/SVR/TPR), IOAPIC
//! (ID/version + mask toàn bộ redirection), LAPIC timer periodic + `EOI` (KT-266); **U-mode ring
//! 3** — trang mã/stack user `US = 1`, `iretq` xuống CPL3, `int 0x80` (gate `DPL = 3`) → handler
//! → `iretq` về ring 3, `Exit` kết thúc (KT-270); **preemption** — hai tác vụ kernel, timer
//! vector 32 chuyển ngữ cảnh bằng cách đổi `rsp` sang frame tác vụ kế (KT-286).
//!
//! **Chưa**: IPC từ ring 3, nhiều process, U-mode task, nhiều hart schedule, priority,
//! sleep/wake, `-cpu max`, `syscall`/`sysret`, soak 600 boot. **Không** tuyên bố K5' đạt: tiêu
//! chí K5' là "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot, D2-149) — đây là **bước 8**
//! (preemption hai tác vụ kernel).

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::apic;
use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{
    CR0_PE, CR0_PG, CR4_PAE, EFER_LME, EFER_NXE, MSR_IA32_APIC_BASE,
};
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::gdt;
use vifast_plat_x86_64::idt;
use vifast_plat_x86_64::serial;
use vifast_plat_x86_64::tss;

/// **KT-270** (K5' bước 4, D2-207): U-mode ring 3 — stub `.user`, handler `int 0x80`, `iretq`
/// xuống CPL3 và đường `Exit` kết thúc. Tách tệp riêng (luật R1: tệp trong `kernel/` ≤ 1.500
/// dòng).
mod umode;

/// **KT-274** (K5' bước 5, D2-208): PCI (ECAM/legacy) + virtio-net + ARP — TAP 15..17. Tách tệp
/// riêng (luật R1) và chạy **sau** `Exit` của ring 3 (chưa có procsrv nên handler là nơi duy nhất
/// còn chạy được tiếp mà không cần scheduler).
mod net;

/// **KT-276 + KT-278** (K5' bước 6 → 7): SMP **4 hart** — trampoline real mode → long mode,
/// `INIT`+`SIPI` tuần tự qua ICR, LAPIC riêng cho từng hart phụ, **IPI** fixed-delivery vector
/// `0x30`, **TSS/GDT/IST1 per-hart**, TAP 18..21. Tách tệp riêng (luật R1) và chạy **sau** TAP 17.
mod smp;

/// **KT-286** (K5' bước 8, D2-212): **preemption** — hai tác vụ kernel, timer vector 32 chuyển
/// ngữ cảnh bằng cách đổi `rsp` sang frame của tác vụ kế, TAP 22/23. Tách tệp riêng (luật R1) và
/// chạy **sau** TAP 21 (SMP), **trước** marker.
mod sched;

/// **KT-288** (K5' bước 9, D2-213): **virtio-blk** — PCI tìm thiết bị, BAR leaf riêng, virtqueue,
/// đọc **sector 0** của đĩa QEMU thật, TAP 24/25. Tách tệp riêng (luật R1) và chạy **sau** TAP 23
/// (preemption), **trước** marker.
mod blk;

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
///
/// KT-266 thêm **bốn** ca: `apic: lapic id`, `apic: ioapic`, `timer: ticks`, `timer: eoi`.
/// **KT-270** thêm **ba** ca U-mode: `umode: ring3 entered`, `umode: syscall handled`,
/// `umode: returned to ring3`. **KT-274** thêm **ba** ca PCI/net: `pci: ecam`, `net: mac`,
/// `net: arp` (TAP 15..17, chạy sau `Exit` của ring 3 — xem [`net`]).
/// **KT-276** thêm **hai** ca SMP: `smp: ap online`, `smp: both harts report` (TAP 18/19, chạy
/// sau TAP 17 — xem [`smp`]).
/// **KT-278** thay hai ca ấy bằng **bốn** ca SMP: `smp: 4 harts online` (18), `smp: tss per-hart`
/// (19), `smp: ipi delivered` (20), `smp: all harts report` (21).
/// **KT-286** thêm **hai** ca preemption: `sched: two tasks ran` (22), `sched: preempted` (23).
/// **KT-288** thêm **hai** ca virtio-blk: `blk: device` (24), `blk: sector0` (25) — tổng **25**.
pub const TAP_TESTS: usize = 25;

/// `IA32_EFER` mà `_start` bật: `LME` (bit 8) | `NXE` (bit 11).
///
/// `NXE` cần cho **NX** của trang stack ring 3 (KT-270): không bật thì bit 63 của entry bảng
/// trang là reserved và mọi truy cập sinh `#PF` với `RSVD = 1`.
pub(crate) const EFER_LME_NXE: u32 = (EFER_LME | EFER_NXE) as u32;

/// Địa chỉ **canonical** nhưng **chưa map** dùng cho ca `#PF`: `0x0000_7000_0000_0000`.
///
/// Canonical vì bit 63..48 = 0 trùng bit 47 = 0 (4-level paging, 48-bit VA); chưa map vì
/// `PML4[224]` = 0 (identity map của ảnh chỉ phủ `0..1 GiB`, MMIO map phủ `3..4 GiB`). Đọc nó
/// ⇒ `#PF` với `P = 0`.
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
    // 4. PML4[0] = __pdpt | P|RW|US. `US = 1` ở **gốc** là điều kiện cần để cây con user
    //    (PDPT[256]) truy cập được từ CPL3; identity map vẫn **supervisor** vì `PDPT[0]` giữ
    //    `US = 0` (bit US được AND qua các cấp).
    "mov edi, offset __pml4",
    "mov eax, offset __pdpt",
    "or eax, 7",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    // 5. PDPT[0] = __pd | P|RW.
    "mov edi, offset __pdpt",
    "mov eax, offset __pd",
    "or eax, 3",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    // 5b. PDPT[3] = __pd_mmio | P|RW — **MMIO map 3..4 GiB** (KT-266): LAPIC `0xFEE0_0000` và
    //     IOAPIC `0xFEC0_0000` nằm trên 3 GiB, ngoài identity map 1 GiB của KT-257. PDPT entry 3
    //     phủ `0xC000_0000..0x1_0000_0000`; mỗi entry PDPT là 8 byte nên offset = 3 × 8 = 24.
    "mov edi, offset __pdpt",
    "mov eax, offset __pd_mmio",
    "or eax, 3",
    "mov dword ptr [edi + 24], eax",
    "mov dword ptr [edi + 28], 0",
    // 5c. **KT-274** — PDPT[2] = __pd_mmio_low | P|RW: **MMIO map 2..3 GiB**. ECAM của QEMU
    //     `-M q35` ở `0xB000_0000` (đo được: `info mtree` ⇒ `pcie-mmcfg-mmio @ 0xb0000000`,
    //     256 MiB) — **dưới** 3 GiB nên ngoài cả identity map 1 GiB lẫn vùng 3..4 GiB của
    //     KT-266. Offset của PDPT entry 2 = 2 × 8 = 16.
    "mov edi, offset __pdpt",
    "mov eax, offset __pd_mmio_low",
    "or eax, 3",
    "mov dword ptr [edi + 16], eax",
    "mov dword ptr [edi + 20], 0",
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
    // 6b. __pd_mmio[i] = 0xC000_0000 + i*2MiB | P|RW|PS|PCD — 512 lá 2 MiB, **uncached** (PCD
    //     bit 4 = 0x10) vì vùng 3..4 GiB của PC là MMIO (PCI hole, IOAPIC, LAPIC, BIOS).
    "mov edi, offset __pd_mmio",
    "mov eax, 0xC0000093",
    "mov ecx, 512",
    "2:",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    "add eax, 0x200000",
    "add edi, 8",
    "dec ecx",
    "jnz 2b",
    // 6b2. **KT-274** — __pd_mmio_low[i] = 0x8000_0000 + i*2MiB | P|RW|PS|PCD: 512 lá 2 MiB
    //      phủ 2..3 GiB (PCI hole 32-bit + ECAM). Cùng cờ `0x93` = P|RW|PS|PCD như vùng 3..4 GiB.
    "mov edi, offset __pd_mmio_low",
    "mov eax, 0x80000093",
    "mov ecx, 512",
    "3:",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    "add eax, 0x200000",
    "add edi, 8",
    "dec ecx",
    "jnz 3b",
    // 6c. **KT-270** — cây bảng trang **user** cho ring 3: `PDPT[256]` (VA 0x0000_0040_0000_0000
    //     = 256 GiB, `PML4[0]`) → `__pd_user` → `__pt_user` → hai lá 4 KiB. `PML4[0]` đã
    //     `US = 1` ở bước 4; các entry dưới đây cũng `US = 1`, còn identity map/MMIO giữ `US = 0`.
    "mov edi, offset __pdpt",
    "mov eax, offset __pd_user",
    "or eax, 7",
    "mov dword ptr [edi + 2048], eax", // 256 × 8 = 2048
    "mov dword ptr [edi + 2052], 0",
    // 6d. __pd_user[0] = __pt_user | P|RW|US.
    "mov edi, offset __pd_user",
    "mov eax, offset __pt_user",
    "or eax, 7",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    // 6e. __pt_user[0] = __user_start | P|US — trang mã ring 3: **chỉ đọc + exec** (W^X).
    "mov edi, offset __pt_user",
    "mov eax, offset __user_start",
    "or eax, 5",
    "mov dword ptr [edi], eax",
    "mov dword ptr [edi + 4], 0",
    // 6f. __pt_user[1] = __user_stack | P|RW|US|NX — trang stack ring 3: đọc + ghi, **không**
    //     exec (NX = bit 63 ⇒ dword cao = 0x8000_0000; EFER.NXE bật ở bước 9).
    "mov eax, offset __user_stack",
    "or eax, 7",
    "mov dword ptr [edi + 8], eax",
    "mov dword ptr [edi + 12], 0x80000000",
    // 7. CR4.PAE = 1 (bắt buộc trước CR3 cho long mode).
    "mov eax, cr4",
    "or eax, 0x20",
    "mov cr4, eax",
    // 8. CR3 = PML4 (địa chỉ vật lý).
    "mov eax, offset __pml4",
    "mov cr3, eax",
    // 9. IA32_EFER.LME = 1 **và** NXE = 1 (KT-270: NX cho trang stack ring 3).
    "mov ecx, 0xC0000080",
    "rdmsr",
    "or eax, {efer_lme_nxe}",
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
    // KT-266: bảng PD **thứ hai** cho MMIO map 3..4 GiB (LAPIC/IOAPIC) — 512 lá 2 MiB, PCD.
    ".align 4096",
    "__pd_mmio:",
    ".skip 4096",
    // **KT-274**: bảng PD thứ ba cho MMIO map 2..3 GiB (ECAM + PCI hole 32-bit).
    ".align 4096",
    "__pd_mmio_low:",
    ".skip 4096",
    // Stack IST1 cho `#DF` (KT-261): 4 KiB riêng, `__ist1_top` = đỉnh (stack mọc xuống).
    //
    // **KT-278**: **mỗi hart một stack IST1** — 4 × 4 KiB. `__ist1_top` vẫn là đỉnh của hart 0
    // (nên `long_mode_entry` không đổi); đỉnh của hart `h` là `__ist1_top + h * 0x1000`, và
    // `__ist1_stacks_end` là đỉnh của hart 3. Hai hart cùng `#DF` không còn ghi vào một stack.
    ".align 4096",
    "__ist1_stacks:",
    ".skip 4096",
    "__ist1_top:",
    ".skip 4096",
    ".skip 4096",
    ".skip 4096",
    "__ist1_stacks_end:",
    // **KT-270** — bảng trang user (PD + PT) cho ring 3, và hai stack:
    //   - `__user_stack`: stack **user** (map ở USER_STACK_VA, `US = 1`, RW, NX);
    //   - `__syscall_stack`: stack **kernel** riêng cho đường vào từ ring 3 (`TSS.RSP0`), để
    //     `int 0x80`/ngắt không đè lên stack boot của `kmain` (kmain không quay lại sau `iretq`).
    ".align 4096",
    "__pd_user:",
    ".skip 4096",
    ".align 4096",
    "__pt_user:",
    ".skip 4096",
    ".align 4096",
    "__user_stack:",
    ".skip 4096",
    "__user_stack_top:",
    ".align 4096",
    "__syscall_stack:",
    // **KT-274**: 32 KiB (trước 16 KiB) — sau `Exit`, handler chạy tiếp phần PCI/net (TAP 15..17)
    // trên stack này; `find_mcfg` dùng hai bộ đệm ACPI 4 KiB lồng nhau.
    ".skip 32768",
    "__syscall_stack_top:",
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
    // ---- **KT-270**: trang mã **ring 3** (section `.user`, linker đặt một trang 4 KiB).
    //
    // Chạy ở CPL3 với `RSP = USER_STACK_TOP`; mọi địa chỉ đều **PC-relative** (`lea`/`rip`) nên
    // đúng ở VA user dù ảnh liên kết ở PA. Số hiệu syscall lấy từ **hằng sinh**
    // (`vifast_abi::SYS_DEBUG_PUT_STR`/`SYS_EXIT`) — không có số 12/14 viết tay (D2-11).
    ".section .user, \"ax\"",
    ".p2align 12",
    ".globl __user_entry",
    "__user_entry:",
    // Chạm stack user (chứng minh trang stack map `US = 1`): ghi rồi đọc lại.
    "mov qword ptr [rsp - 8], 0x1234",
    "mov rax, qword ptr [rsp - 8]",
    // DebugPutStr(ptr, len): rdi = chuỗi (cùng trang), rsi = độ dài, rax = 12.
    "lea rdi, [rip + __user_msg]",
    "mov rsi, {user_msg_len}",
    "mov rax, {sys_debug_put_str}",
    "int 0x80",
    // Trả về theo `[calling.x86_64]`: rax = error, rdx = value. Giữ lại rồi gọi Exit.
    "mov r12, rax",
    "mov r13, rdx",
    // Exit(0): rdi = 0, rax = 14. Không trả về ring 3.
    "xor rdi, rdi",
    "mov rax, {sys_exit}",
    "int 0x80",
    // `Exit` không trả về; nếu có thì dừng an toàn thay vì chạy vào rác.
    "1:",
    "jmp 1b",
    "__user_msg:",
    ".ascii \"ring3 says hi\\n\"",
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
    // KT-270: EFER.LME|NXE + số hiệu syscall sinh từ spec + độ dài chuỗi ring 3.
    efer_lme_nxe = const EFER_LME_NXE,
    sys_debug_put_str = const vifast_abi::SYS_DEBUG_PUT_STR,
    sys_exit = const vifast_abi::SYS_EXIT,
    user_msg_len = const umode::USER_MSG_LEN,
);

// ---------------------------------------------------------------------------------------------
// Console tối giản trên COM1
// ---------------------------------------------------------------------------------------------

/// `core::fmt::Write` trên COM1 — để `panic!`/`write!` in được số mà không cần `alloc`.
pub(crate) struct Console;

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
pub(crate) fn check(console: &mut Console, n: usize, name: &str, ok: bool) {
    if !tap(console, n, name, ok) {
        let _ = writeln!(console, "# TAP {n} FAIL — dừng (không in marker)");
        asm::halt(3)
    }
}

// ---------------------------------------------------------------------------------------------
// IST1 per-hart — KT-278
// ---------------------------------------------------------------------------------------------

// Bốn stack IST1 (mỗi hart một stack 4 KiB) do `global_asm!` ở trên khai trong `.bss`.
extern "C" {
    static __ist1_top: u8;
    static __ist1_stacks_end: u8;
}

/// Đỉnh stack `IST1` của `hart` — `__ist1_top + hart * 0x1000`.
///
/// `__ist1_top` là đỉnh stack của **hart 0** (giá trị `long_mode_entry` truyền vào `kmain`), nên
/// hart 0 giữ nguyên đường boot; hart `h` dùng stack kế tiếp. `hart >= 4` trả `0` để caller
/// fail-closed thay vì trỏ vào rác.
pub(crate) fn ist1_top(hart: usize) -> u64 {
    let base = core::ptr::addr_of!(__ist1_top) as u64;
    let end = core::ptr::addr_of!(__ist1_stacks_end) as u64;
    let top = base + (hart as u64) * 0x1000;
    if top > end {
        return 0;
    }
    top
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

/// Handler **chung** của mọi exception **và** ngắt — stub asm (`vifast_isr_common`) gọi hàm này.
///
/// KT-266 thêm hai nhánh **ngắt** (vector 32 timer, vector 255 spurious) xử lý **trước** mọi
/// thứ khác và **không** in dòng `# exc:`; phần còn lại vẫn là đường exception của KT-261.
///
/// Trả **`rip` để resume** (⇒ stub `iretq` về đó) hoặc **`0`** (⇒ stub halt sạch). Hàm **không**
/// bao giờ trả về nếu exception không phải ca kiểm đã arm: nó in nguyên nhân rồi `halt(2)` — che
/// một fault thật bằng cách resume là điều tệ nhất có thể làm.
///
/// # Vì sao nhận **giá trị** chứ không nhận con trỏ frame
///
/// Ảnh này **không** được có `unsafe` (R5), nên không thể deref con trỏ frame. Stub asm đọc 5
/// trường của frame và truyền thành 5 tham số SysV; **KT-286** thêm tham số thứ 6 `frame_rsp`
/// (địa chỉ frame, **giá trị** `u64`) để scheduler lưu `rsp` của tác vụ đang chạy — cùng thông
/// tin, không cần `unsafe`.
#[no_mangle]
pub extern "C" fn exc_dispatch(
    vec: u64,
    err: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    frame_rsp: u64,
) -> u64 {
    // ---- KT-266: **ngắt** (không phải exception) — xử lý trước, **không** in dòng `# exc:`.
    //
    // Timer: tăng tick, ghi `EOI`, tăng bộ đếm EOI, rồi `iretq` về đúng chỗ bị ngắt. Không in
    // gì: một dòng log mỗi tick sẽ làm nghẽn COM1 và biến phép đo thành đo tốc độ UART.
    if vec == exc::VEC_TIMER as u64 {
        // **KT-276**: vector 32 dùng chung cho tick PIT của BSP **và** timer đánh thức của hart
        // phụ. `LAPIC_ID` đọc qua MMIO là **của hart đang chạy**, nên đây là chỗ duy nhất phân
        // biệt được ai vừa ngắt — không có nó, tick của AP làm bẩn `TICK_COUNT`/`EOI_COUNT` của
        // BSP (và `timer: eoi` sẽ đỏ oan).
        let id = lapic_id() as u64;
        if id == smp::bsp_lapic_id() {
            TICK_COUNT.fetch_add(1, Ordering::Relaxed);
            // **Đột biến (iii)** (`--features no-eoi`): không `EOI` ⇒ LAPIC giữ ISR bit của
            // vector 32 ⇒ timer **tắt sau tick đầu** (ĐỎ ở TAP 10 `timer: ticks`).
            if !cfg!(feature = "no-eoi") {
                lapic_eoi();
            }
            EOI_COUNT.fetch_add(1, Ordering::Relaxed);
            // **KT-286**: preemption — handler lưu `rsp` của tác vụ đang chạy, chọn tác vụ kế,
            // và (nếu chuyển) đặt `rsp` frame đích vào ô của stub. Stub chung đọc ô ấy rồi
            // `pop`/`iretq` trên frame mới — đó là toàn bộ context switch.
            if let Some((new_rsp, new_rip)) = sched::on_tick(frame_rsp) {
                vifast_plat_x86_64::sched::set_new_rsp(new_rsp);
                return new_rip;
            }
        } else {
            smp::note_ap_tick();
            lapic_eoi();
        }
        return rip;
    }
    // Spurious: SDM Vol 3A §10.9 — **không** `EOI`, chỉ đếm rồi quay lại.
    if vec == exc::VEC_SPURIOUS as u64 {
        SPURIOUS_COUNT.fetch_add(1, Ordering::Relaxed);
        return rip;
    }
    // **KT-278**: IPI (vector `0x30`) — hart đích đếm + in `# smp: ipi recv=<n> intid=0x30` rồi
    // `EOI`. In **trước**, tăng biến đếm **sau** (quy tắc "in trước, đếm sau" của KT-259): BSP
    // chờ đúng biến đếm nên dòng log chắc chắn đã ra trước khi BSP in dòng của nó.
    if vec == exc::VEC_IPI as u64 {
        smp::note_ipi();
        lapic_eoi();
        return rip;
    }

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
// APIC + timer — KT-266 (K5' bước 3)
// ---------------------------------------------------------------------------------------------

/// Địa chỉ MMIO của LAPIC, đọc từ `IA32_APIC_BASE` lúc khởi tạo — handler `EOI` dùng lại.
static LAPIC_BASE: AtomicU64 = AtomicU64::new(0);
/// Số tick timer đã nhận (handler tăng) — nguồn tick là PIT kênh 0 qua IOAPIC → LAPIC.
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
/// Số lần ghi `EOI` (handler tăng **sau** khi ghi EOI thành công).
static EOI_COUNT: AtomicU64 = AtomicU64::new(0);
/// Số ngắt **spurious** đã nhận — không `EOI` (SDM Vol 3A §10.9).
static SPURIOUS_COUNT: AtomicU64 = AtomicU64::new(0);

/// Số chia LAPIC timer: **1** (mã `0b1011`).
const TIMER_DIVIDE: u32 = 1;
/// Giá trị đếm đầu tiên của LAPIC timer — 1 ms danh định (mô hình QEMU: 1 count/ns).
///
/// **Không** dùng làm nguồn tick: QEMU TCG giao ngắt LAPIC timer không đều (đo được chu kỳ
/// 2,8–38,5 ms cho chu kỳ danh định 100 µs). Ảnh cấu hình đủ ba thanh ghi + **kiểm đếm ngược**
/// (bằng chứng timer chạy) rồi mask `LVT`; nguồn tick là **PIT** (xem [`timer_init`]).
const TIMER_INITIAL_COUNT: u32 = 1_000_000;
/// Số chia PIT cho **100 Hz**: 1_193_182 / 100 = 11_931,82 ⇒ 11_932 (sai số 0,0015 %).
const PIT_DIVISOR_100HZ: u16 = 11_932;
/// Tần số tick danh định của PIT (Hz) — **tính** từ hằng số PIT, làm tròn, không hardcode.
const PIT_TICK_HZ: u32 =
    (apic::PIT_FREQUENCY_HZ + PIT_DIVISOR_100HZ as u32 / 2) / PIT_DIVISOR_100HZ as u32;
/// Chân IOAPIC của IRQ0 (PIT) trên PC: **pin 2** (IRQ0 → pin 2, IRQ1 → pin 1, IRQ2 → pin 0).
const IOAPIC_PIT_PIN: u32 = 2;
/// Trần vòng chờ **một** mốc tick — mốc đầu busy-wait (mỗi vòng 3 cổng I/O PIT) nên timer hỏng
/// thì trần này cắt (ĐỎ) thay vì treo; hai mốc sau dùng `hlt` (timer đã chứng minh chạy).
///
/// Đo được: mốc đầu tới sau ~150 ms ≈ vài chục nghìn vòng ⇒ 500_000 là dư ~10×.
const TICK_SPIN_BOUND: u32 = 500_000;
/// Chênh lệch tick tối đa giữa hai mốc liên tiếp — vượt là timer chạy sai nhịp (không hardcode
/// số tick chính xác: QEMU TCG không đều).
const MAX_TICK_DELTA: u64 = 1_000;

/// Đọc thanh ghi LAPIC (MMIO 32-bit) theo offset.
pub(crate) fn lapic_read(offset: u32) -> u32 {
    asm::read_u32(LAPIC_BASE.load(Ordering::Relaxed) + offset as u64)
}

/// Ghi thanh ghi LAPIC (MMIO 32-bit) theo offset.
pub(crate) fn lapic_write(offset: u32, value: u32) {
    asm::write_u32(LAPIC_BASE.load(Ordering::Relaxed) + offset as u64, value);
}

/// Địa chỉ MMIO LAPIC mà BSP đã dùng — **KT-276**: hart phụ đọc lại từ `IA32_APIC_BASE` của nó
/// (cùng giá trị) và boot block công bố giá trị này.
pub(crate) fn lapic_base() -> u64 {
    LAPIC_BASE.load(Ordering::Relaxed)
}

/// `LAPIC_ID` của hart **đang chạy** — đọc MMIO nên mỗi hart thấy ID của chính nó.
pub(crate) fn lapic_id() -> u8 {
    apic::lapic_id(lapic_read(apic::LAPIC_ID))
}

/// Ghi `EOI` — báo LAPIC đã xử lý xong ngắt hiện tại (SDM §10.6.1).
fn lapic_eoi() {
    lapic_write(apic::LAPIC_EOI, 0);
}

/// Đọc thanh ghi IOAPIC qua cặp `IOREGSEL`/`IOWIN`.
fn ioapic_read(reg: u32) -> u32 {
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_REGSEL as u64, reg);
    asm::read_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_WIN as u64)
}

/// Ghi thanh ghi IOAPIC qua cặp `IOREGSEL`/`IOWIN`.
fn ioapic_write(reg: u32, value: u32) {
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_REGSEL as u64, reg);
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_WIN as u64, value);
}

/// Nạp PIT kênh 0 **mode 2** (rate generator — tự nạp lại) với `divisor`.
fn pit_program_mode2(divisor: u16) {
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_CH0_MODE2_LOHI);
    asm::outb(apic::PIT_CH0_DATA, divisor as u8);
    asm::outb(apic::PIT_CH0_DATA, (divisor >> 8) as u8);
}

/// Đọc counter PIT kênh 0 bằng lệnh **latch** (hai lần đọc LSB/MSB là một ảnh ổn định).
fn pit_latch_read() -> u16 {
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_LATCH_CH0);
    let lo = asm::inb(apic::PIT_CH0_DATA) as u16;
    let hi = asm::inb(apic::PIT_CH0_DATA) as u16;
    (hi << 8) | lo
}

/// Nạp PIT **kênh 2** mode 2 (rate generator) với `divisor` — **đồng hồ đo** độc lập kênh 0.
///
/// Kênh 2 là timer của loa: phải bật **gate** (cổng `0x61` bit 0) thì counter mới chạy, và giữ
/// bit dữ liệu loa = 0 để không phát âm. Dùng divisor `0xFFFF` (chu kỳ 54,9 ms) — lớn hơn nhiều
/// so với khoảng cách giữa hai lần đọc (~10 ms), nên `pit_elapsed_ticks` không bị mất vòng.
fn pit2_program_mode2(divisor: u16) {
    let p61 = asm::inb(apic::PIT_SPEAKER_PORT);
    asm::outb(
        apic::PIT_SPEAKER_PORT,
        (p61 & !apic::PIT_SPEAKER_DATA) | apic::PIT_SPEAKER_GATE2,
    );
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_CH2_MODE2_LOHI);
    asm::outb(apic::PIT_CH2_DATA, divisor as u8);
    asm::outb(apic::PIT_CH2_DATA, (divisor >> 8) as u8);
}

/// Đọc counter PIT **kênh 2** bằng lệnh latch.
///
/// **KT-274**: `pub(crate)` — đồng hồ của bước PCI/net ([`net`]) dùng lại chính counter này
/// (mode 2 divisor `0xFFFF` đã được [`measure_timer`] lập trình, không phát ngắt).
pub(crate) fn pit2_latch_read() -> u16 {
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_LATCH_CH2);
    let lo = asm::inb(apic::PIT_CH2_DATA) as u16;
    let hi = asm::inb(apic::PIT_CH2_DATA) as u16;
    (hi << 8) | lo
}

/// Chờ tới khi `TICK_COUNT >= target`, tối đa `bound` vòng; trả tick hiện tại hoặc `None`.
///
/// Trong lúc chờ, **cộng dồn** số tick PIT đã trôi qua vào `elapsed` (xử lý bọc mode 2 mỗi lần
/// đọc) — nhờ vậy cửa sổ đo `hz` không bị đếm thiếu khi counter PIT nạp lại nhiều lần.
fn wait_ticks_pit(
    target: u64,
    bound: u32,
    prev: &mut u16,
    elapsed: &mut u32,
    allow_hlt: bool,
) -> Option<u64> {
    let mut spins = 0u32;
    loop {
        let now = TICK_COUNT.load(Ordering::Relaxed);
        if now >= target {
            return Some(now);
        }
        if spins >= bound {
            return None;
        }
        let p = pit2_latch_read();
        *elapsed += apic::pit_elapsed_ticks(*prev, p);
        *prev = p;
        spins += 1;
        if allow_hlt {
            // `hlt` (ngắt đã bật) nhường BQL cho QEMU xử lý timer — busy-wait làm ngắt trễ
            // ~100 ms. Chỉ dùng **sau** khi đã thấy tick đầu tiên: nếu timer hỏng, `hlt` sẽ
            // chặn tới khi runner hết ngân sách, còn busy-wait thì trần `bound` cắt được.
            asm::hlt();
        } else {
            core::hint::spin_loop();
        }
    }
}

/// Kết quả khởi tạo LAPIC — bằng chứng in ra log + dùng cho TAP.
struct LapicInfo {
    /// `IA32_APIC_BASE` đọc lại sau khi bật (bit 11 = enable).
    base_msr: u64,
    /// Địa chỉ MMIO đã dùng.
    base: u64,
    /// `LAPIC_ID` thô.
    id_raw: u32,
    /// `LAPIC_VERSION` thô.
    version_raw: u32,
    /// `SVR` đọc lại sau khi ghi.
    svr: u32,
}

/// Khởi tạo LAPIC: che 8259 PIC, đọc `IA32_APIC_BASE`, bật APIC toàn cục nếu cần, đọc
/// ID/version, bật `SVR`, đặt `TPR = 0` (nhận mọi ưu tiên), mask `LINT0`/`LINT1`.
fn lapic_init() -> LapicInfo {
    // 0. Che **8259 PIC** trước mọi thứ: trên PC, PIC nối vào `LAPIC.LINT0` (virtual wire) và
    //    BIOS đặt vector IRQ0 = `0x08` — **trùng `#DF`**. Không che thì ngắt PIT (IRQ0) được giao
    //    thẳng cho CPU với vector 8 và ảnh chết trong `#DF` ngay sau `sti` (đo được ở lần chạy
    //    đầu KT-266: `# exc: vec=8 err=0x100479 rip=0x8 cs=0x203`).
    asm::outb(apic::PIC1_DATA, apic::PIC_MASK_ALL);
    asm::outb(apic::PIC2_DATA, apic::PIC_MASK_ALL);

    // 1. `IA32_APIC_BASE`: base ở bit 12..51, bit 11 = enable toàn cục.
    let mut msr = asm::read_msr(MSR_IA32_APIC_BASE);
    if msr & apic::APIC_BASE_ENABLE == 0 {
        msr |= apic::APIC_BASE_ENABLE;
        asm::write_msr(MSR_IA32_APIC_BASE, msr);
    }
    let base = msr & apic::APIC_BASE_ADDR_MASK;
    LAPIC_BASE.store(base, Ordering::Relaxed);

    // 2. Đọc ID/version **trước** khi bật SVR — nếu MMIO không map/không có APIC, đây là chỗ
    //    fault hoặc đọc ra 0/0xFFFFFFFF và TAP sẽ đỏ.
    let id_raw = lapic_read(apic::LAPIC_ID);
    let version_raw = lapic_read(apic::LAPIC_VERSION);
    // **KT-276**: chốt `LAPIC_ID` của BSP **trước** khi bật ngắt — handler timer dùng nó để tách
    // tick của BSP khỏi tick đánh thức của hart phụ.
    smp::set_bsp_lapic_id(apic::lapic_id(id_raw));

    // 3. Bật APIC bằng phần mềm: `SVR = vector spurious | APIC_ENABLE`.
    lapic_write(apic::LAPIC_SVR, apic::svr_value(exc::VEC_SPURIOUS));
    // 4. `TPR = 0` — không che ưu tiên nào (mọi ngắt đều được nhận).
    lapic_write(apic::LAPIC_TPR, 0);
    // 4b. Mask `LINT0`/`LINT1` — chặn nốt đường virtual wire của PIC và NMI ngoài; nguồn ngắt
    //     duy nhất còn lại là IOAPIC (đã mask toàn bộ, sau đó mở đúng IRQ0 cho PIT).
    lapic_write(apic::LAPIC_LVT_LINT0, apic::LVT_MASKED);
    lapic_write(apic::LAPIC_LVT_LINT1, apic::LVT_MASKED);
    // 5. Đọc lại SVR — bằng chứng ghi **thật**, không chỉ "đã gọi".
    let svr = lapic_read(apic::LAPIC_SVR);

    LapicInfo {
        base_msr: asm::read_msr(MSR_IA32_APIC_BASE),
        base,
        id_raw,
        version_raw,
        svr,
    }
}

/// Kết quả khởi tạo IOAPIC.
struct IoapicInfo {
    /// `IOAPICID` thô.
    id_raw: u32,
    /// `IOAPICVER` thô.
    version_raw: u32,
    /// Số redirection entry (`IOAPICVER` + 1).
    entries: u32,
    /// `true` ⇔ **mọi** entry đọc lại đều có bit mask.
    all_masked: bool,
}

/// Khởi tạo IOAPIC: đọc `IOAPICID`/`IOAPICVER`, rồi **mask toàn bộ** redirection entry.
///
/// Bước này **chưa** cần ngoại vi: mục tiêu là chứng minh có IOAPIC thật (đọc được ID/version)
/// và che hết nguồn ngắt ngoài. [`timer_init`] sau đó mở **đúng một** entry (IRQ0/PIT) làm nguồn
/// tick — đó là lựa chọn dự phòng của KT-266, ghi rõ trong tài liệu.
fn ioapic_init() -> IoapicInfo {
    let id_raw = ioapic_read(apic::IOAPIC_ID_REG);
    let version_raw = ioapic_read(apic::IOAPIC_VER_REG);
    let entries = apic::ioapic_redirection_count(version_raw);
    let mut all_masked = true;
    let mut pin = 0u32;
    while pin < entries {
        if let Some(reg) = apic::ioapic_redtbl_reg(pin) {
            let low = ioapic_read(reg);
            ioapic_write(reg, apic::ioapic_redir_low_masked(low));
            // Đọc lại: mask phải **thật sự** nằm trong thanh ghi.
            if !apic::ioapic_redir_is_masked(ioapic_read(reg)) {
                all_masked = false;
            }
        }
        pin += 1;
    }
    IoapicInfo {
        id_raw,
        version_raw,
        entries,
        all_masked,
    }
}

/// Kết quả khởi tạo timer.
struct TimerInit {
    /// `LAPIC_TIMER_CURRENT_COUNT` lần đọc thứ nhất.
    lapic_count0: u32,
    /// `LAPIC_TIMER_CURRENT_COUNT` lần đọc thứ hai (sau vài vòng).
    lapic_count1: u32,
    /// `true` ⇔ current count **giảm** ⇒ LAPIC timer chạy thật.
    lapic_timer_runs: bool,
    /// `true` ⇔ redirection IRQ0 của IOAPIC trỏ vector 32, **không** mask, dest 0.
    ioapic_route_ok: bool,
    /// Counter PIT đọc ngay sau khi nạp mode 2 (phải ≈ divisor).
    pit_counter0: u16,
    /// Counter PIT sau vài vòng (phải **giảm**).
    pit_counter1: u16,
}

/// Khởi tạo timer: cấu hình + kiểm LAPIC timer, rồi dùng **PIT** làm nguồn tick.
///
/// # Vì sao nguồn tick là PIT (D2-204, KT-266)
///
/// Đo trên QEMU 11.1.3 TCG: LAPIC timer đặt `initial_count = 100_000`, chia 1 (chu kỳ danh định
/// 100 µs theo mô hình QEMU `(count+1) ns`) nhưng **khoảng cách giữa hai ngắt đo được** là
/// 2,8–38,5 ms và đổi giữa các lần boot — QEMU chỉ xử lý timer ở ranh giới vòng lặp chính, không
/// theo từng chu kỳ. Vì vậy ảnh dùng **PIT kênh 0 mode 2** (1,193182 MHz, chia 11_932 ⇒ 100 Hz)
/// làm nguồn tick, còn LAPIC **nhận** ngắt qua IOAPIC (redirection IRQ0 → vector 32) và ghi
/// `EOI`. LAPIC timer vẫn được cấu hình đủ ba thanh ghi và **kiểm đếm ngược** — bằng chứng nó
/// chạy — rồi mask để không tranh nguồn tick.
fn timer_init() -> TimerInit {
    // ---- 1. LAPIC timer: đủ ba thanh ghi (divide, LVT, initial count) + kiểm đếm ngược.
    let divide_code = apic::timer_divide_code(TIMER_DIVIDE).unwrap_or(apic::TIMER_DIVIDE_BY_1);
    lapic_write(apic::LAPIC_TIMER_DIVIDE, divide_code);
    lapic_write(
        apic::LAPIC_LVT_TIMER,
        apic::lvt_timer_value(exc::VEC_TIMER, true),
    );
    lapic_write(apic::LAPIC_TIMER_INITIAL_COUNT, TIMER_INITIAL_COUNT);
    let lapic_count0 = lapic_read(apic::LAPIC_TIMER_CURRENT_COUNT);
    let mut spins = 0u32;
    while spins < 1_000 {
        core::hint::spin_loop();
        spins += 1;
    }
    let lapic_count1 = lapic_read(apic::LAPIC_TIMER_CURRENT_COUNT);
    let lapic_timer_runs = lapic_count1 < lapic_count0;
    // Mask `LVT`: nguồn tick là PIT, LAPIC timer **không** được phát ngắt.
    lapic_write(
        apic::LAPIC_LVT_TIMER,
        apic::lvt_timer_value(exc::VEC_TIMER, true) | apic::LVT_MASKED,
    );

    // ---- 2. PIT kênh 0 mode 2 — 100 Hz.
    pit_program_mode2(PIT_DIVISOR_100HZ);
    let pit_counter0 = pit_latch_read();
    let mut pit_spins = 0u32;
    while pit_spins < 2000 {
        core::hint::spin_loop();
        pit_spins += 1;
    }
    let pit_counter1 = pit_latch_read();

    // ---- 3. IOAPIC: mở **đúng một** redirection (IRQ0/PIT) → vector 32, dest = LAPIC 0.
    let mut ioapic_route_ok = false;
    if let Some(reg) = apic::ioapic_redtbl_reg(IOAPIC_PIT_PIN) {
        ioapic_write(reg, exc::VEC_TIMER as u32);
        ioapic_write(reg + 1, 0);
        let low = ioapic_read(reg);
        let high = ioapic_read(reg + 1);
        ioapic_route_ok = (low & 0xFF) == exc::VEC_TIMER as u32
            && !apic::ioapic_redir_is_masked(low)
            && high == 0;
    }

    TimerInit {
        lapic_count0,
        lapic_count1,
        lapic_timer_runs,
        ioapic_route_ok,
        pit_counter0,
        pit_counter1,
    }
}

/// Kết quả đo timer.
struct TimerInfo {
    /// Tần số tick **đo được** (Hz) trên cửa sổ ba mốc — không phải tần số danh định.
    hz: u32,
    /// Tick tại mốc thứ ba.
    ticks: u64,
    /// `true` ⇔ ba mốc **tăng nghiêm ngặt** và chênh lệch trong khoảng hợp lý.
    ticks_ok: bool,
    /// `true` ⇔ số `EOI` == số tick.
    eoi_ok: bool,
    /// Cửa sổ đo **đo được** (tick PIT) từ mốc đầu tới mốc cuối — bằng chứng nhịp.
    period_pit: u32,
}

/// Đo timer: ba mốc tick + `hz` **đo** bằng PIT + đối chiếu `EOI` với tick.
///
/// # Vì sao **không** hardcode số tick
///
/// QEMU TCG không bảo đảm nhịp đều: một mốc có thể nhảy 1 hoặc vài tick. Vì vậy ca kiểm chỉ đòi
/// (i) ba mốc **tăng nghiêm ngặt**, (ii) mỗi chênh lệch ≤ [`MAX_TICK_DELTA`], (iii) `hz > 0`.
/// Số tick **tuyệt đối** không phải hợp đồng.
///
/// `hz` là **trung bình đo được** trên cả cửa sổ ba mốc (không phải tần số danh định): PIT đếm
/// bằng counter mode 2, còn tick đếm bằng handler. Dưới TCG, QEMU xử lý timer ở ranh giới vòng
/// lặp chính nên `hz` đo được **thấp hơn** 100 Hz danh định và đổi giữa các lần boot — đó là số
/// thật, không phải lỗi cấu hình.
fn measure_timer() -> TimerInfo {
    // ---- 1. Ba mốc tick: mỗi mốc chờ tick tăng thêm 1 (có trần), **cộng dồn** PIT kênh 2.
    pit2_program_mode2(apic::PIT_MAX_DIVISOR);
    let mut prev = pit2_latch_read();
    let mut elapsed = 0u32;
    let s0 = TICK_COUNT.load(Ordering::Relaxed);
    // Mốc đầu **busy-wait** (trần `bound` cắt được nếu timer hỏng); hai mốc sau `hlt` vì timer
    // đã chứng minh chạy.
    let s1 = wait_ticks_pit(s0 + 1, TICK_SPIN_BOUND, &mut prev, &mut elapsed, false);
    // `hz` đo trên **hai mốc sau** (busy-wait đầu chậm hơn hẳn nên không đại diện nhịp).
    let mut prev2 = pit2_latch_read();
    let mut elapsed2 = 0u32;
    let s2 = match s1 {
        Some(t) => wait_ticks_pit(t + 1, TICK_SPIN_BOUND, &mut prev2, &mut elapsed2, true),
        None => None,
    };
    let s3 = match s2 {
        Some(t) => wait_ticks_pit(t + 1, TICK_SPIN_BOUND, &mut prev2, &mut elapsed2, true),
        None => None,
    };
    let ticks_ok = match (s1, s2, s3) {
        (Some(a), Some(b), Some(c)) => {
            a > s0
                && b > a
                && c > b
                && (a - s0) <= MAX_TICK_DELTA
                && (b - a) <= MAX_TICK_DELTA
                && (c - b) <= MAX_TICK_DELTA
        }
        _ => false,
    };
    let ticks = s3.unwrap_or_else(|| TICK_COUNT.load(Ordering::Relaxed));

    // ---- 2. `hz` trung bình trên cửa sổ hai mốc sau: tick LAPIC / tick PIT × 1,193182 MHz.
    let tick_delta = match (s1, s3) {
        (Some(a), Some(c)) => c.saturating_sub(a).min(u32::MAX as u64) as u32,
        _ => 0,
    };
    let hz = apic::hz_from_pit(tick_delta, elapsed2);

    // ---- 3. `EOI` phải bằng tick: handler ghi EOI **mỗi** tick, không thừa không thiếu.
    let eoi_ok = EOI_COUNT.load(Ordering::Relaxed) == TICK_COUNT.load(Ordering::Relaxed);

    TimerInfo {
        hz,
        ticks,
        ticks_ok,
        eoi_ok,
        period_pit: elapsed2,
    }
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
    // **KT-276**: chốt `sp` ban đầu của BSP (`__stack_top` của linker) để bước SMP so nó với
    // slot 0 của arena — lệch nghĩa là linker và crate arch không còn khớp bố cục.
    smp::set_bsp_stack_top(stack_top);
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
    // KT-266: MMIO map 3..4 GiB — LAPIC/IOAPIC nằm ngoài identity map 1 GiB.
    // KT-274: thêm MMIO map 2..3 GiB — ECAM của q35 (`0xB000_0000`) nằm dưới 3 GiB.
    let _ = writeln!(
        console,
        "# viFastOs x86_64: mmio map 0x{:x}..0x{:x} pages={} pcd={}",
        boot::MMIO_MAP_LOW_BASE,
        boot::MMIO_MAP_BASE + boot::MMIO_MAP_BYTES,
        boot::PD_ENTRIES * 2,
        (boot::PD_MMIO_FLAGS & 0x10) >> 4,
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
    // **KT-278**: TSS/GDT là **per-hart** — hart 0 dùng slot 0, `IST1` là stack IST1 của slot 0.
    tss::init(0, stack_top, ist1_top);
    gdt::install(0, tss::base(0));
    idt::install();

    // ---- TAP 1: GDT đã nạp **thật** — `sgdt`/`str` đọc lại khớp bảng + descriptor TSS.
    let (gdt_base, gdt_limit) = gdt::read_gdtr(0);
    let tr = asm::str_reg();
    // Sau `ltr`, CPU đánh dấu descriptor TSS là **busy** (`0x89 → 0x8B`) — kiểm dạng busy
    // là bằng chứng `ltr` đã nạp thật, không chỉ "đã gọi".
    let want_tss =
        gdt::tss_descriptor_type(tss::base(0), tss::TSS_LIMIT as u32, gdt::TSS_TYPE_BUSY_64);
    let got_tss = [
        gdt::entry(0, gdt::INDEX_TSS),
        gdt::entry(0, gdt::INDEX_TSS + 1),
    ];
    let gdt_ok = gdt_base == gdt::base(0)
        && gdt_limit == gdt::GDT_LIMIT
        && tr == gdt::SELECTOR_TSS
        && got_tss == want_tss
        && tss::rsp0(0) == stack_top
        && tss::ist(0, 1) == ist1_top
        && tss::io_map_base(0) == tss::TSS_IO_MAP_BASE;
    let _ = writeln!(
        console,
        "# viFastOs x86_64: gdt base=0x{gdt_base:x} limit={gdt_limit} tr=0x{tr:x} tss=0x{:x} ist1=0x{ist1_top:x}",
        tss::base(0),
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

    // ---- TAP 7: `int 0x30` ⇒ ô `isr_unknown` (sentinel 254) — mọi vector khác vẫn **thấy được**.
    exc_reset();
    exc_arm(exc::VEC_UNKNOWN, asm::probe_unknown_resume());
    asm::probe_unknown();
    check(
        &mut console,
        7,
        "exc: unknown",
        exc_caught(exc::VEC_UNKNOWN, 0, asm::probe_unknown_resume()),
    );

    // ---- KT-266: APIC + timer.
    //
    // Thứ tự: LAPIC (SVR/TPR) → IOAPIC (mask hết) → timer (LVT + initial count) → `sti` → đo.
    // Ngắt **chỉ** bật sau khi mọi thứ đã cấu hình: một ngắt đến sớm sẽ vào handler khi
    // `LAPIC_BASE`/`TICK_COUNT` chưa sẵn sàng.

    // ---- TAP 8: LAPIC — đọc được ID/version qua MMIO, `SVR` đã bật (bit 8) + vector spurious.
    let lapic = lapic_init();
    let lapic_id = apic::lapic_id(lapic.id_raw);
    let lapic_version = apic::lapic_version(lapic.version_raw);
    let lapic_ok = lapic.base == apic::LAPIC_DEFAULT_BASE
        && lapic.base_msr & apic::APIC_BASE_ENABLE != 0
        && lapic.svr & apic::SVR_APIC_ENABLE != 0
        && (lapic.svr & apic::SVR_VECTOR_MASK) == exc::VEC_SPURIOUS as u32
        && lapic_version != 0
        && lapic_version != 0xFF;
    let _ = writeln!(
        console,
        "# apic: id={lapic_id} version=0x{lapic_version:02x} svr=0x{:03x}",
        lapic.svr,
    );
    check(&mut console, 8, "apic: lapic id", lapic_ok);

    // ---- TAP 9: IOAPIC — đọc được `IOAPICID`/`IOAPICVER`, **mask toàn bộ** redirection entry.
    let ioapic = ioapic_init();
    let ioapic_id = apic::ioapic_id(ioapic.id_raw);
    let ioapic_version = apic::ioapic_version(ioapic.version_raw);
    let ioapic_ok = ioapic.entries >= 1
        && ioapic.entries <= apic::IOAPIC_MAX_REDIRECTIONS
        && ioapic_version != 0
        && ioapic_version != 0xFF
        && ioapic.all_masked;
    let _ = writeln!(
        console,
        "# apic: ioapic id={ioapic_id} ver=0x{ioapic_version:02x} entries={}",
        ioapic.entries,
    );
    check(&mut console, 9, "apic: ioapic", ioapic_ok);

    // ---- TAP 10/11: timer — cấu hình LAPIC timer + PIT, bật ngắt, đo tick + EOI.
    let timer_init = timer_init();
    let _ = writeln!(
        console,
        "# timer: lapic count0={} count1={} runs={} ioapic_route={} nominal_hz={} pit=0x{:04x}->0x{:04x}",
        timer_init.lapic_count0,
        timer_init.lapic_count1,
        timer_init.lapic_timer_runs as u8,
        timer_init.ioapic_route_ok as u8,
        PIT_TICK_HZ,
        timer_init.pit_counter0,
        timer_init.pit_counter1,
    );
    asm::sti();
    let timer = measure_timer();
    asm::cli();
    let _ = writeln!(
        console,
        "# timer: hz={} ticks={} window_pit={}",
        timer.hz, timer.ticks, timer.period_pit,
    );
    check(
        &mut console,
        10,
        "timer: ticks",
        timer.ticks_ok && timer.hz > 0 && timer_init.lapic_timer_runs && timer_init.ioapic_route_ok,
    );
    let _ = writeln!(
        console,
        "# timer: eoi={} ticks={} spurious={}",
        EOI_COUNT.load(Ordering::Relaxed),
        TICK_COUNT.load(Ordering::Relaxed),
        SPURIOUS_COUNT.load(Ordering::Relaxed),
    );
    check(&mut console, 11, "timer: eoi", timer.eoi_ok);

    let _ = writeln!(
        console,
        "# viFastOs x86_64: exc count={} last_vec={}",
        EXC_COUNT.load(Ordering::Relaxed),
        EXC_VEC.load(Ordering::Relaxed),
    );

    // ---- **KT-270** (K5' bước 4, D2-207): U-mode ring 3. Map trang mã/stack user đã dựng ở
    // `_start`, đặt `TSS.RSP0` sang stack syscall riêng rồi `iretq` xuống CPL3; **không trả về**
    // — đường `Exit` của ring 3 in marker **duy nhất** của ảnh rồi tắt máy. Marker chỉ in khi
    // TAP 1..14 đã xanh.
    umode::run(&mut console)
}
