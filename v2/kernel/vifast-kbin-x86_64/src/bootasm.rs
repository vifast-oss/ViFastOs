// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`_start` 32-bit + long mode của ảnh x86_64** — tách khỏi `main.rs` ở **KT-340b** (luật R1).
//!
//! Tệp này là **toàn bộ asm boot** của ảnh (không có một dòng Rust chạy nào ngoài `global_asm!`):
//!
//! ```text
//!   multiboot2 header + PVH note (`_start` 32-bit) → bảng trang identity/MMIO/user →
//!   CR4.PAE → CR3 → EFER.LME|NXE → CR0.PG|PE → GDT tạm → ljmp long_mode_entry →
//!   kmain(boot_eax, boot_ebx, stack_top, ist1_top)
//! ```
//!
//! # Vì sao tách (không phải cắt cơ học)
//!
//! `KT-340b` phải đưa `main.rs` từ **1.500** (trần R1) về **< 1.100** dòng. Miền "boot/MMU"
//! (header giao thức, `_start` 32-bit, bảng trang, trampoline, stub ring 3) là một **miền riêng**
//! — không phụ thuộc `Console`/TAP/khoá COM1 — nên tách nguyên khối, **không** đổi một lệnh asm
//! nào (khuôn `KT-338c`/`bootram.rs` của aarch64). Bảng trang `__pml4`…`__pt_user` và arena bảng
//! trang động do [`crate::bootpt`] định nghĩa (Rust static, `link_section = ".bootpt"`) — asm chỉ
//! tham chiếu **symbol**, địa chỉ do linker đặt trong section NOLOAD `.bootpt`.
//!
//! # Bất biến giữ nguyên
//!
//! - `_start` chạy ở **protected mode 32-bit**, địa chỉ tuyệt đối; `.multiboot` phải ở 32 KiB
//!   đầu tệp; `.note.Xen` là `SHT_NOTE` riêng; `__trampoline_start` 64 KiB-aligned **trong**
//!   `.text` (KT-276) — ba `ASSERT` của `linker.ld` vẫn chặn nếu lệch;
//! - `_start` zero `.bss` **và** `.bootpt` (KT-340b: bảng trang ra NOLOAD nên `.bss` không phủ);
//! - stub ring 3 `__user_entry` (KT-270 + KT-340b) nằm trong section `.user`, chạy được ở **mọi**
//!   VA mà ánh xạ tới cùng PA (mọi địa chỉ PC-relative) — nhờ vậy cùng một trang mã chạy được ở
//!   cây tĩnh (`USER_CODE_VA`, scheduler KT-304) và cây `VSpace` động (`VSPACE_CODE_VA`).

use core::arch::global_asm;

use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{EFER_LME, EFER_NXE};

use crate::product;

/// `IA32_EFER` mà `_start` bật: `LME` (bit 8) | `NXE` (bit 11).
///
/// `NXE` cần cho **NX** của trang stack ring 3 (KT-270): không bật thì bit 63 của entry bảng
/// trang là reserved và mọi truy cập sinh `#PF` với `RSVD = 1`.
const EFER_LME_NXE: u32 = (EFER_LME | EFER_NXE) as u32;

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
    // 3b. **KT-340b** — zero `.bootpt` (NOLOAD: `.bss` không phủ tới). Vùng này chứa tám bảng
    //     trang boot + arena bảng trang động (32 frame) — entry không dùng **phải** bằng 0, không
    //     dựa vào "QEMU tình cờ trả 0" (bài học bước 3). `.bootstacks` không cần zero (stack).
    "mov edi, offset __bootpt_start",
    "mov ecx, offset __bootpt_end",
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
    // ---- `.bss`: **chỉ** biến boot 4+4 B và `__syscall_stack` 32 KiB (KT-340b).
    //
    // **KT-340b** — tám bảng trang boot (32 KiB) đã ra `.bootpt`, bốn stack IST1 + stack user
    // (20 KiB) đã ra `.bootstacks`; cả hai là section **NOLOAD** trong `linker.ld` (khuôn
    // `.smp_stacks` của KT-276, D2-243/D2-244). `.data+.bss` ảnh sản phẩm: 96.320 → 43.072 B.
    // `__syscall_stack` **ở lại** `.bss`: nó là stack kernel của đường vào từ ring 3, 32 KiB, và
    // D2-244 chỉ đòi `.data+.bss < 64 KiB` (43.072 B ⇒ còn 22.464 B dư). Giữ nó trong `.bss`
    // cũng làm đột biến (i) có nghĩa: đưa 8 bảng trang về `.bss` ⇒ 43.072 + 32.768 = 75.840 B
    // > 65.536 B ⇒ `budget-x86_64.sh` **ĐỎ**.
    ".section .bss",
    ".align 8",
    "__boot_eax:",
    ".skip 4",
    "__boot_ebx:",
    ".skip 4",
    ".align 4096",
    "__syscall_stack:",
    // KT-274: 32 KiB (trước 16 KiB) — sau `Exit`, handler chạy tiếp phần PCI/net (TAP 15..17)
    // trên stack này; `find_mcfg` dùng hai bộ đệm ACPI 4 KiB lồng nhau.
    ".skip 32768",
    "__syscall_stack_top:",
    // ---- **KT-340b** — section `.bootstacks` (NOLOAD): bốn stack IST1 của KT-278 (mỗi hart một
    // stack cho `#DF`) + trang stack **user** của KT-270 (map ở `USER_STACK_VA`, `US = 1`, RW,
    // NX). NOLOAD ⇒ không chiếm `filesz`, không cộng vào `.data+.bss` — đúng loại section
    // `.smp_stacks` mà D2-243 cho phép ("section stack riêng kiểu `.smp_stacks`").
    //
    // `__ist1_top` vẫn là đỉnh stack của hart 0 (giá trị `long_mode_entry` truyền vào `kmain`);
    // đỉnh của hart `h` là `__ist1_top + h * 0x1000`, `__ist1_stacks_end` là đỉnh của hart 3.
    ".section .bootstacks, \"aw\", @nobits",
    ".align 4096",
    "__ist1_stacks:",
    ".skip 4096",
    "__ist1_top:",
    ".skip 4096",
    ".skip 4096",
    ".skip 4096",
    "__ist1_stacks_end:",
    ".align 4096",
    "__user_stack:",
    ".skip 4096",
    "__user_stack_top:",
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
    // ---- **KT-270**: trang mã **ring 3** (section `.user`, linker đặt một trang 4 KiB). Mọi địa
    // chỉ **PC-relative** (`lea`/`rip`) nên đúng ở VA user dù ảnh liên kết ở PA; số hiệu syscall
    // lấy từ hằng sinh (D2-11). Không `.p2align 12` ở đây (KT-304): stub ring 3 của scheduler ở
    // `umode.rs` có thể được phát **trước** khối này, căn 4 KiB giữa section làm ASSERT linker đỏ.
    ".section .user, \"ax\"",
    ".globl __user_entry",
    "__user_entry:",
    // Chạm stack user (chứng minh trang stack map `US = 1`): ghi rồi đọc lại.
    "mov qword ptr [rsp - 8], 0x1234",
    "mov rax, qword ptr [rsp - 8]",
    // **KT-340b** — ghi vào **vùng của tác vụ khác**: `VSPACE_PEER_VA` **không** map trong
    // `VSpace` của tác vụ này (chỉ `VSpace` của tác vụ kia map nó) ⇒ `#PF` (`P=0, W=1, U=1`
    // ⇒ `err=0x6` — lỗi "trang không có mặt khi ghi từ CPL3").
    // Handler (`exc_dispatch`) đã arm đúng `CR2` và resume ở nhãn `__user_after_peer`; nếu
    // **không** fault thì chạy tiếp qua nhãn — ca kiểm đỏ vì thiếu bằng chứng fault.
    "mov rax, {vspace_peer_va}",
    "mov qword ptr [rax], 0x0bad",
    ".globl __user_after_peer",
    "__user_after_peer:",
    // **KT-418c** — phủ **preempt ring-3** (nợ `KT-413c` §6.1). Quay ở CPL3 **có trần** với
    // `RFLAGS.IF = 1` (`USER_RFLAGS = 0x202`): tick PIT cắt ngang CPL3, handler x86_64 ghi bằng
    // chứng rồi **đổi RIP** về nhãn `__user_cpl3_resumed` — tất định, không dựa vào ngưỡng thời
    // gian. Trần để PIT chết thì chạy tiếp và kernel in `preempted=0` (**ĐỎ có tên**), không treo.
    ".globl __user_cpl3_spin",
    "__user_cpl3_spin:",
    "mov rcx, {cpl3_spin}",
    "2:",
    "dec rcx",
    "jnz 2b",
    ".globl __user_cpl3_resumed",
    "__user_cpl3_resumed:",
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
    vspace_peer_va = const vifast_plat_x86_64::umode::VSPACE_PEER_VA,
    selector_code = const boot::SELECTOR_CODE,
    selector_data = const boot::SELECTOR_DATA,
    gdt_null = const boot::GDT_NULL,
    gdt_code = const boot::GDT_KERNEL_CODE,
    gdt_data = const boot::GDT_KERNEL_DATA,
    // KT-270: EFER.LME|NXE + số hiệu syscall sinh từ spec + độ dài chuỗi ring 3.
    efer_lme_nxe = const EFER_LME_NXE,
    sys_debug_put_str = const vifast_abi::SYS_DEBUG_PUT_STR,
    sys_exit = const vifast_abi::SYS_EXIT,
    user_msg_len = const product::USER_MSG_LEN,
    // KT-418c: trần vòng quay CPL3 (~0,4 s ở TCG) — đủ rộng cho ≥1 tick PIT (20 ms), đủ ngắn để
    // boot không bị đốt nếu PIT chết.
    cpl3_spin = const product::CPL3_SPIN,
);
