// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-plat-x86_64` — platform x86_64 phần **host-testable** (KT-186 / D2-163, K5' bước M2).
//!
//! K5' là milestone lớn; D2-163 duyệt **M1** (phạm vi "như K3", boot tạm QEMU `-kernel`,
//! `spec/abi.toml` `[calling.x86_64]` **chưa mở**) và cho làm **M2** — bước **không cần QEMU**,
//! đo bằng `cargo test` trên host, đúng khuôn K4' bước một (D2-147).
//!
//! # Nội dung
//!
//! - [`format`]: định dạng **x86_64 4-level paging, 4 KiB, 39-bit VA canonical, 48-bit PA** —
//!   descriptor `P/RW/US/PWT/PCD/A/D/PS/PAT/G/NX`; `impl PageTableFormat for X86_64` (trait
//!   **dùng chung** ở `vifast-pte` từ KT-164);
//! - [`mmu`]: `Mmu` **thuần dữ liệu** — re-export walker dùng chung `PageTable<F, M>` +
//!   [`TableMemory`](mmu::TableMemory) từ `vifast-pte` (KT-186), type alias [`X86Mmu`](mmu::X86Mmu);
//! - [`control`]: hằng bit `CR0`/`CR4`/`IA32_EFER` + địa chỉ MSR (`IA32_EFER`, `STAR`, `LSTAR`,
//!   `FMASK`, `PAT`, …) — bước M4/M7 sẽ nạp;
//! - [`calling`]: bảng thanh ghi **System V AMD64 ABI** + **đề xuất** ánh xạ syscall ViFastOs
//!   (`syscall` vs `int 0x80`) — bản nháp cho M7, **chưa** phải ABI.
//! - [`serial`] (KT-257): dữ liệu COM1 16550 tại `0x3F8` — offset thanh ghi, bit `LSR.THRE/TEMT`,
//!   `divisor()` cho 115200 8N1, **thứ tự** chuỗi khởi tạo;
//! - [`boot`] (KT-257): dữ liệu đường boot — entry bảng trang 2 MiB, thứ tự bật long mode, GDT
//!   tối thiểu, header multiboot2 và note PVH.
//! - [`apic`] (KT-266): dữ liệu **APIC + timer** — địa chỉ MMIO LAPIC/IOAPIC, offset + bit thanh
//!   ghi, bảng chia LAPIC timer, redirection entry IOAPIC, và số học PIT để **đo** `hz`.
//! - [`umode`] (KT-270): dữ liệu **U-mode ring 3** — VA user, selector user, `RFLAGS`, bố cục
//!   frame `int 0x80`, `SyscallRet`; asm `iretq`/stub `int 0x80` nằm ở [`asm`].
//! - [`smp`] (KT-276 + **KT-278**): dữ liệu **SMP 4 hart** — arena stack per-hart (`guard 4 KiB |
//!   stack 64 KiB`), `ICR` (`INIT`/`SIPI`/**fixed-delivery IPI**), `SIPI` vector, topology
//!   `APIC ID`, bố cục trang trampoline `0x8000` (GDT tạm + boot block) và hằng thời gian của
//!   SDM; trampoline 16-bit nằm ở asm của ảnh (`vifast-kbin-x86_64::smp`).
//! - [`tss`]/[`gdt`] (**KT-278**): **per-hart** — mỗi hart một TSS 104 byte, một bảng GDT 7 entry
//!   và một `GDTR`; `check_distinct` kiểm **địa chỉ** bốn vùng TSS rời nhau.
//! - [`com`] (**KT-316**): khoá COM1 — CAS quay vòng **quanh một dòng** (`write_fmt`), `cli` trước
//!   khi lấy khoá và khôi phục `IF` sau khi nhả (đường in trong ngắt không tự deadlock); bộ đếm
//!   `acquired`/`contended`/`overlap` làm bằng chứng `# com: lock` + TAP 28.
//!
//! # Crate này **không** làm gì
//!
//! Không boot, không QEMU, không linker script, không GDT/IDT/TSS, không `CR3`/`INVLPG`, không
//! `syscall`/`sysret` thật, không tuyên bố K5' đạt — D2-163. Bước M3 mới là entry + GDT/IDT/TSS;
//! M4 mới bật paging thật; M7 mới mở `spec/` `[calling.x86_64]` theo khuôn KT-05. **KT-270** đã
//! mở `[calling.x86_64]` (D2-207) và thêm đường **`int 0x80`** + `iretq` xuống ring 3 — vẫn
//! **không** `syscall`/`sysret` (xem `spec/abi.toml` `[calling.x86_64].trap`).
//!
//! KT-257 thêm **dữ liệu** cho bước boot đầu tiên ([`serial`], [`boot`]) và **asm đặc quyền**
//! ([`asm`]); phần `_start` 32-bit + bật long mode vẫn nằm ở crate ảnh `vifast-kbin-x86_64`
//! (`global_asm!` không cần `unsafe`).
//!
//! # Vì sao `deny(unsafe_code)` (không còn `forbid`)
//!
//! Bước M2 là **hợp đồng dữ liệu** và **không** cần `unsafe`. KT-257 thêm [`asm`] — mô-đun duy
//! nhất có asm đặc quyền (`in`/`out`/`mov crN`/`rdmsr`/`hlt`), đúng danh sách đóng D2-17 cho
//! `kernel/vifast-plat-*`. Vì `forbid` **không thể** bị `allow` ở module con, gốc crate đổi sang
//! `deny` và **chỉ** [`asm`] `#![allow(unsafe_code)]` — mọi module khác vẫn bị `deny` cưỡng chế
//! (cùng khuôn `vifast-plat-aarch64`, KT-167).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64
//! cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --target x86_64-unknown-none
//! ```

#![no_std]
#![deny(unsafe_code)]

pub mod apic;
pub mod asm;
pub mod boot;
pub mod calling;
pub mod com;
pub mod control;
pub mod exc;
pub mod format;
pub mod gdt;
pub mod idt;
pub mod image;
pub mod irq_lock;
pub mod mmu;
pub mod net_client;
pub mod pci;
pub mod sched;
pub mod serial;
pub mod smp;
pub mod timebase;
pub mod tss;
pub mod umode;
pub mod virtio_blk;
pub mod virtio_net;

/// `Walk` chẩn đoán phải ghi được mọi mức của format x86_64 — kiểm lúc biên dịch.
const _: () = assert!(mmu::MAX_LEVELS >= format::X86_64_LEVELS);

pub use apic::{
    hz_from_pit, ioapic_id, ioapic_redir_is_masked, ioapic_redir_low_masked,
    ioapic_redirection_count, ioapic_redtbl_reg, ioapic_version, lapic_id, lapic_version,
    lvt_is_masked, lvt_timer_value, lvt_vector, pit_elapsed_ticks, svr_value, timer_divide_code,
    timer_divide_value, APIC_BASE_ADDR_MASK, APIC_BASE_BSP, APIC_BASE_ENABLE, IOAPIC_DEFAULT_BASE,
    IOAPIC_ID_REG, IOAPIC_MAX_REDIRECTIONS, IOAPIC_REDIR_MASKED, IOAPIC_REDTBL_BASE,
    IOAPIC_REDTBL_STRIDE, IOAPIC_REGSEL, IOAPIC_VER_REG, IOAPIC_WIN, LAPIC_DEFAULT_BASE, LAPIC_EOI,
    LAPIC_ID, LAPIC_ID_SHIFT, LAPIC_LVT_LINT0, LAPIC_LVT_LINT1, LAPIC_LVT_TIMER, LAPIC_SVR,
    LAPIC_TIMER_CURRENT_COUNT, LAPIC_TIMER_DIVIDE, LAPIC_TIMER_INITIAL_COUNT, LAPIC_TPR,
    LAPIC_VERSION, LAPIC_VERSION_MASK, LVT_MASKED, LVT_TIMER_PERIODIC, LVT_VECTOR_MASK, PIC1_DATA,
    PIC2_DATA, PIC_MASK_ALL, PIT_CH0_CMD, PIT_CH0_DATA, PIT_CH2_DATA, PIT_CMD_CH0_MODE0_LOHI,
    PIT_CMD_CH0_MODE2_LOHI, PIT_CMD_CH2_MODE2_LOHI, PIT_CMD_LATCH_CH0, PIT_CMD_LATCH_CH2,
    PIT_FREQUENCY_HZ, PIT_MAX_DIVISOR, PIT_SPEAKER_DATA, PIT_SPEAKER_GATE2, PIT_SPEAKER_PORT,
    SVR_APIC_ENABLE, SVR_VECTOR_MASK, TIMER_DIVIDE_BY_1, TIMER_DIVIDE_BY_128, TIMER_DIVIDE_BY_16,
    TIMER_DIVIDE_BY_2, TIMER_DIVIDE_BY_32, TIMER_DIVIDE_BY_4, TIMER_DIVIDE_BY_64,
    TIMER_DIVIDE_BY_8,
};
pub use calling::{
    Reg, INT80_CLOBBERED, INT80_MR_REGS, INT80_RETURN_MSGINFO_REG, INT80_VECTOR, SYSCALL_ARG_REGS,
    SYSCALL_CLOBBERED, SYSCALL_MR_REGS, SYSCALL_MSGINFO_REG, SYSCALL_MSRS, SYSCALL_NUMBER_REG,
    SYSCALL_RETURN_ERROR_REG, SYSCALL_RETURN_MSGINFO_REG, SYSCALL_RETURN_VALUE_REG, SYSV_ARG_REGS,
    SYSV_CALLEE_SAVED, SYSV_CALLER_SAVED, SYSV_DIRECTION_FLAG_CLEAR, SYSV_FRAME_POINTER,
    SYSV_RED_ZONE_BYTES, SYSV_RETURN_REGS, SYSV_STACK_ALIGN_BYTES, SYSV_STACK_POINTER,
    SYSV_VARIADIC_VECTOR_COUNT_REG, SYSV_VECTOR_ARG_REGS, SYSV_VECTOR_RETURN_REGS,
};
pub use control::{
    CR0_AM, CR0_CD, CR0_EM, CR0_ET, CR0_MP, CR0_NE, CR0_NW, CR0_PE, CR0_PG, CR0_TS, CR0_WP,
    CR3_ADDR_MASK, CR3_PCD, CR3_PCID_MASK, CR3_PWT, CR4_CET, CR4_DE, CR4_FSGSBASE, CR4_LA57,
    CR4_MCE, CR4_OSFXSR, CR4_OSXMMEXCPT, CR4_OSXSAVE, CR4_PAE, CR4_PCE, CR4_PCIDE, CR4_PGE,
    CR4_PKE, CR4_PKS, CR4_PSE, CR4_PVI, CR4_SMAP, CR4_SMEP, CR4_SMXE, CR4_TSD, CR4_UMIP, CR4_VME,
    CR4_VMXE, EFER_FFXSR, EFER_LMA, EFER_LME, EFER_LMSLE, EFER_NXE, EFER_SCE, EFER_SVME, EFER_TCE,
    LONG_MODE_CR0, LONG_MODE_CR4, LONG_MODE_EFER, MSR_IA32_APIC_BASE, MSR_IA32_EFER,
    MSR_IA32_FMASK, MSR_IA32_FS_BASE, MSR_IA32_GS_BASE, MSR_IA32_KERNEL_GS_BASE, MSR_IA32_LSTAR,
    MSR_IA32_PAT, MSR_IA32_STAR,
};
pub use exc::{
    has_error_code, name as exc_name, stub_slot, ERROR_CODE_VECTORS, FRAME_CS_OFFSET,
    FRAME_ERROR_OFFSET, FRAME_GPR_COUNT, FRAME_RFLAGS_OFFSET, FRAME_RIP_OFFSET,
    FRAME_VECTOR_OFFSET, KERNEL_CS, PF_ERR_INSTRUCTION, PF_ERR_PRESENT, PF_ERR_RESERVED,
    PF_ERR_USER, PF_ERR_WRITE, STUB_AC, STUB_BP, STUB_COUNT, STUB_CP, STUB_DB, STUB_DE, STUB_DF,
    STUB_GP, STUB_IPI, STUB_NP, STUB_PF, STUB_SPURIOUS, STUB_SS, STUB_SX, STUB_SYSCALL, STUB_TIMER,
    STUB_TS, STUB_UD, STUB_UNKNOWN, STUB_VC, VEC_AC, VEC_BP, VEC_CP, VEC_DB, VEC_DE, VEC_DF,
    VEC_GP, VEC_IPI, VEC_NP, VEC_PF, VEC_SPURIOUS, VEC_SS, VEC_SX, VEC_SYSCALL, VEC_TIMER, VEC_TS,
    VEC_UD, VEC_UNKNOWN, VEC_UNKNOWN_PROBE, VEC_VC,
};
pub use format::{
    is_canonical_bits, Descriptor, MemAttr, PageTableFormat, Perms, Shareability, IA32_PAT_DEFAULT,
    PTE_ACCESSED, PTE_ADDR_MASK, PTE_ADDR_SHIFT, PTE_DIRTY, PTE_GLOBAL, PTE_NX, PTE_PAT,
    PTE_PAT_LARGE, PTE_PCD, PTE_PRESENT, PTE_PS, PTE_PWT, PTE_RESERVED_MASK, PTE_RW, PTE_USER,
    X86_64, X86_64_ENTRIES, X86_64_INDEX_BITS, X86_64_LEVELS, X86_64_MAX_VA_BITS, X86_64_PAGE_BITS,
    X86_64_PAGE_BYTES, X86_64_PA_BITS, X86_64_VA_BITS,
};
pub use gdt::{
    gdt_descriptor, gdt_kernel_code, gdt_kernel_data, gdt_user_code, gdt_user_data, tss_base_of,
    tss_descriptor, tss_descriptor_type, ACCESS_KERNEL_CODE, ACCESS_KERNEL_DATA, ACCESS_USER_CODE,
    ACCESS_USER_DATA, FLAGS_DATA, FLAGS_LONG_CODE, GDT_BYTES, GDT_ENTRIES, GDT_KERNEL_CODE,
    GDT_KERNEL_DATA, GDT_LIMIT, GDT_NULL, GDT_USER_CODE, GDT_USER_DATA, INDEX_KERNEL_CODE,
    INDEX_KERNEL_DATA, INDEX_NULL, INDEX_TSS, INDEX_USER_CODE, INDEX_USER_DATA, RPL_USER,
    SELECTOR_KERNEL_CODE, SELECTOR_KERNEL_DATA, SELECTOR_TSS, SELECTOR_USER_CODE,
    SELECTOR_USER_CODE_RPL3, SELECTOR_USER_DATA, SELECTOR_USER_DATA_RPL3, TSS_TYPE_AVAILABLE_64,
    TSS_TYPE_BUSY_64,
};
pub use idt::{
    entry as idt_entry, entry_ist, entry_offset, entry_selector, entry_type_attr, gate_type_attr,
    GATE_INTERRUPT, GATE_INTERRUPT_USER, GATE_TRAP, IDT_BYTES, IDT_ENTRIES, IDT_LIMIT,
    IST_DOUBLE_FAULT,
};
pub use mmu::{
    MapError, Mmu, PageTable, TableMemory, Translation, Walk, WalkOutcome, WalkStep, X86Mmu,
    MAX_LEVELS,
};
pub use sched::{
    init_frame_words, next_task, switch_frame_ok, FrameReturn, FRAME_BYTES, FRAME_WORDS,
    INITIAL_RSP_SLACK, MAX_SWITCHES, TASK_A, TASK_B, TASK_C, TASK_COUNT, TASK_MAIN, TASK_RFLAGS,
    TASK_SLOTS,
};
pub use smp::{
    apic_id_for_hart, check_layout, guard_offset, icr_delivery_pending, icr_fixed_low, icr_high,
    icr_init_low, icr_low, icr_sipi_low, sipi_vector, stack_bottom_offset, stack_top_offset,
    trampoline_addr, valid_hart, HartStacks, StackError, AP_TIMER_DIVIDE, AP_TIMER_INITIAL_COUNT,
    BOOT_CR0, BOOT_CR4, BOOT_EFER, BOOT_ENTRY, BOOT_HART, BOOT_IDT_BASE, BOOT_IDT_LIMIT,
    BOOT_LAPIC_BASE, BOOT_LAPIC_ID, BOOT_MAGIC, BOOT_PML4, BOOT_STACK_TOP, HART_AREA_BYTES,
    HART_GUARD_BYTES, HART_STACK_BYTES, HART_STRIDE, ICR_DELIVERY_MODE_MASK,
    ICR_DELIVERY_MODE_SHIFT, ICR_DELIVERY_PENDING, ICR_DEST_SHIFT, ICR_DM_FIXED, ICR_DM_INIT,
    ICR_DM_STARTUP, ICR_LEVEL_ASSERT, ICR_LEVEL_TRIGGER, ICR_VECTOR_MASK, INIT_WAIT_US,
    IPI_TIMEOUT_US, LAPIC_ICR_HIGH, LAPIC_ICR_LOW, MAX_HARTS, ONLINE_TIMEOUT_US, SIPI_MAX_ADDR,
    SIPI_WAIT_US, SMP_EXPECT_MASK, SMP_HARTS, TRAMPOLINE_BASE, TRAMPOLINE_BOOT_OFFSET,
    TRAMPOLINE_BYTES, TRAMPOLINE_GDTR_OFFSET, TRAMPOLINE_GDT_OFFSET, TRAMPOLINE_MAGIC,
};
pub use tss::{
    check_distinct as tss_check_distinct, TssError, TSS_BYTES, TSS_IO_MAP_BASE, TSS_IO_MAP_OFFSET,
    TSS_IST1_OFFSET, TSS_IST_COUNT, TSS_IST_STRIDE, TSS_LIMIT, TSS_RSP0_OFFSET, TSS_WORDS,
};
pub use umode::{
    is_kernel_cs, is_user_cs, rflags_if, SyscallRet, KERNEL_DS, RFLAGS_IF, SYSCALL_FRAME_CS,
    SYSCALL_FRAME_ERROR, SYSCALL_FRAME_R10, SYSCALL_FRAME_R11, SYSCALL_FRAME_R12,
    SYSCALL_FRAME_R13, SYSCALL_FRAME_R14, SYSCALL_FRAME_R15, SYSCALL_FRAME_R8, SYSCALL_FRAME_R9,
    SYSCALL_FRAME_RAX, SYSCALL_FRAME_RBP, SYSCALL_FRAME_RBX, SYSCALL_FRAME_RCX, SYSCALL_FRAME_RDI,
    SYSCALL_FRAME_RDX, SYSCALL_FRAME_RFLAGS, SYSCALL_FRAME_RIP, SYSCALL_FRAME_RSI,
    SYSCALL_FRAME_RSP, SYSCALL_FRAME_SS, SYSCALL_FRAME_VECTOR, USER_CODE_VA, USER_CS, USER_RFLAGS,
    USER_SS, USER_STACK_BYTES, USER_STACK_TOP, USER_STACK_VA,
};
