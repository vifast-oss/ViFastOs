// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-276 + KT-278** (K5' bước 6 → bước 7) — SMP trên x86_64: trampoline real mode → long mode,
//! `INIT`+`SIPI` qua ICR, LAPIC per-hart, **4 hart**, **IPI** fixed-delivery, **TSS per-hart**.
//!
//! Tách khỏi `main.rs` vì luật **R1** (`v2/bin/v2 rules`): tệp trong `kernel/` ≤ **1.500 dòng**.
//! Tên mô-đun là `smp` (không đụng `vifast_plat_x86_64::smp` — crate arch giữ phần **dữ liệu**:
//! arena, ICR, bố cục trampoline, hằng thời gian; tệp này giữ phần **điều phối** + asm trampoline).
//!
//! # Đường đánh thức hart phụ (KT-278: **ba** hart phụ, tuần tự)
//!
//! ```text
//!   BSP (kmain → umode::run → net::run → smp::run)
//!     │ 1. copy trang trampoline (link 64 KiB-aligned trong `.text`) xuống 0x8000
//!     │ 2. bật bit online của hart 0 (BSP luôn online — nó đang chạy đây)
//!     │ 3. với hart = 1, 2, 3 **tuần tự**:
//!     │      a. ghi boot block (PML4, stack của hart ấy, hart id, entry, IDTR, CR0/CR4/EFER,
//!     │         LAPIC base, LAPIC ID kỳ vọng, magic)
//!     │      b. ICR: INIT (level assert) → 10 ms → SIPI vector 0x08 → 200 µs → SIPI
//!     │      c. chờ `SMP_ONLINE_MASK` bit hart (trần 2 s, đo bằng PIT kênh 2)
//!     │ 4. TAP 18: **bốn** hart online (mask 0xf, LAPIC ID đúng, stack **khác slot**)
//!     │ 5. TAP 19: **TSS per-hart** — bốn địa chỉ TSS rời nhau, `tr=0x28`, descriptor busy
//!     │ 6. IPI: ICR fixed-delivery vector 0x30 → hart 1; chờ `SMP_IPI_COUNT == 1` (trần 100 ms)
//!     │    ⇒ TAP 20
//!     │ 7. `SMP_CMD[h] = CMD_REPORT` **tuần tự** từng hart phụ, chờ `SMP_ALIVE_MASK` bit h
//!     │ 8. TAP 21 (cả bốn hart báo alive) → trả về cho `net::run` in marker + halt
//!     ↓
//!   AP (SIPI ⇒ 0x8000, real mode)
//!     ├─ 16-bit: cli/cld, DS=ES=SS=CS, lgdt GDT tạm, CR4.PAE, CR3, EFER.LME|NXE, CR0.PG|PE
//!     ├─ ljmp 0x08:0x8046 ⇒ 64-bit: DS/ES/SS=0x10, rsp = boot.stack_top, lidt boot.idt
//!     ├─ call boot.entry = `kmain_secondary(hart)`
//!     └─ kmain_secondary: kiểm slot stack + LAPIC_ID → **TSS riêng + GDT riêng + `ltr`** →
//!        bật LAPIC (SVR/TPR/LINT) → in `# smp: hart=N online` → bật bit online → LAPIC timer
//!        (vector 32) + `sti` → vòng `hlt`: thấy `CMD_REPORT` ⇒ in `# smp: hart=N alive` + bit
//!        alive; thấy IPI (vector 0x30) ⇒ handler in `# smp: ipi recv=<n> intid=0x30` + `EOI`.
//! ```
//!
//! # Vì sao trampoline **copy xuống** thay vì để linker đặt ở 0x8000
//!
//! Đo được: QEMU `-kernel` (PVH) **có** nạp segment ở 0x8000, nhưng **SeaBIOS chạy sau đó** và
//! vùng thấp bị ghi đè — đọc lại `u32 @ 0x8000` trong `kmain` ra `0`. Vì vậy trampoline được
//! **link ở địa chỉ 64 KiB-aligned** (`. = ALIGN(64K)` **trong** `.text` của `linker.ld`) rồi BSP
//! **copy** xuống `0x8000` lúc chạy. Link ở địa chỉ 64 KiB-aligned là điều kiện để `offset label`
//! **16-bit** trong đoạn real mode bằng đúng offset trong trang (địa chỉ link `0x110000 + off`
//! cắt còn 16 bit = `off`); mọi địa chỉ tuyệt đối trong đoạn 64-bit dùng hằng `0x8000 + off` nên
//! **đúng sau khi copy**.
//!
//! # Vì sao boot block **dùng lại một chỗ** cho ba hart phụ
//!
//! Trang trampoline chỉ có **một** boot block (`0x8900`). Ba hart phụ được đánh thức **tuần tự**:
//! BSP ghi boot block cho hart `h`, gửi `INIT`+`SIPI`, rồi **chờ hart ấy online** trước khi ghi
//! boot block cho hart `kế`. Hart phụ đọc **mọi** trường của boot block (kể cả `BOOT_LAPIC_ID`)
//! **trước** khi bật bit online, nên sau khi thấy bit ấy BSP ghi đè là an toàn — không cần ba bản
//! boot block, và không có hai hart phụ nào chạy trampoline cùng lúc (stack 16-bit tạm `0xFC00`
//! cũng chỉ một người dùng).
//!
//! # Vì sao không có đoạn 32-bit protected mode
//!
//! Từ real mode có thể đặt thẳng `CR4.PAE`/`CR3`/`EFER.LME` (đều là lệnh CPL0 hợp lệ trong real
//! mode), rồi `CR0.PG|PE` và `ljmp` vào đoạn code 64-bit — CPU vào thẳng IA-32e mode. Nhờ vậy GDT
//! tạm chỉ cần **3 entry** (null + code 64-bit + data), không cần descriptor code 32-bit, và
//! selector `0x08`/`0x10` **trùng** GDT thật của BSP nên bảng IDT dùng chung (`selector 0x08`)
//! vẫn hợp lệ trên hart phụ.
//!
//! # KT-278 — TSS per-hart (bỏ nợ của KT-276)
//!
//! KT-276 để hart phụ dùng **chung** GDT tạm 3 entry + IDT của BSP, **không** `ltr`; `#DF` trên AP
//! vì thế dùng chung `IST1` của BSP. Với **bốn** hart nợ đó thành nguy hiểm thật (hai hart cùng
//! `#DF` ghi vào một stack). KT-278: mỗi hart gọi [`tss::init`] cho **slot của mình**, rồi
//! [`gdt::install`] nạp **bảng GDT riêng** với descriptor TSS trỏ **TSS của chính nó**, và `ltr`.
//! Hart phụ **đọc lại** base từ descriptor nó vừa nạp ([`gdt::tss_base_of`]) rồi công bố — BSP
//! kiểm **địa chỉ** ([`tss::check_distinct`]) nên đột biến "TSS trùng địa chỉ" **đỏ ở tầng kiểm**,
//! không phải "hy vọng nó không xảy ra".
//!
//! # Phạm vi (KT-278)
//!
//! **Bốn** hart + IPI + TSS per-hart. **Không** scheduler/preemption, **không** `-cpu max`,
//! **không** `syscall`/`sysret`, **không** tuyên bố K5' đạt.

use core::arch::global_asm;
use core::fmt::Write;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_x86_64::apic;
use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{EFER_LME, EFER_NXE, MSR_IA32_APIC_BASE};
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::gdt;
use vifast_plat_x86_64::idt;
use vifast_plat_x86_64::smp::{self, HartStacks};
use vifast_plat_x86_64::timebase::PitClock;
use vifast_plat_x86_64::tss;

use crate::{check, ist1_top, pit2_latch_read, Console};

// Arena stack per-hart do `linker.ld` cấp (`__smp_stacks_start`/`__smp_stacks_end`).
extern "C" {
    static __smp_stacks_start: u8;
    static __smp_stacks_end: u8;
    static __trampoline_start: u8;
    static __trampoline_end: u8;
}

/// Lệnh cho hart phụ (giống khuôn `smpboot.rs` aarch64): `0` = không có gì.
const CMD_NONE: u64 = 0;
/// In `# smp: hart=<id> alive` rồi bật bit trong [`SMP_ALIVE_MASK`].
const CMD_REPORT: u64 = 1;

/// **KT-301** — độ trễ **150 ms** của kiểm đỏ-trước `--features slow-ipi` (đo bằng PIT kênh 2).
///
/// 150 ms **lớn hơn** trần thời gian cũ 100 ms (`smp::IPI_TIMEOUT_US`) và **nhỏ hơn** trần số vòng
/// mới ⇒ cùng một ảnh cho **hai** kết quả: trước KT-301 ĐỎ, sau KT-301 XANH. Đây là **kiểm**, không
/// phải ngưỡng của sản phẩm.
const SLOW_IPI_DELAY_US: u64 = 150_000;

/// Mặt nạ hart đã online (bit `i` = hart `i` đã in `# smp: hart=i online` + LAPIC + TSS sẵn sàng).
static SMP_ONLINE_MASK: AtomicU32 = AtomicU32::new(0);
/// Mặt nạ hart đã in `# smp: hart=i alive` (bit `i` = hart `i`).
static SMP_ALIVE_MASK: AtomicU32 = AtomicU32::new(0);
/// Lệnh đang chờ **từng** hart phụ thực thi ([`CMD_NONE`] = không có).
static SMP_CMD: [AtomicU64; smp::MAX_HARTS] = [const { AtomicU64::new(CMD_NONE) }; smp::MAX_HARTS];
/// `LAPIC_ID` **từng hart tự đọc** — bằng chứng hart id lấy từ phần cứng, không từ chỉ số vòng lặp.
static SMP_HART_LAPIC_ID: [AtomicU64; smp::MAX_HARTS] =
    [const { AtomicU64::new(u64::MAX) }; smp::MAX_HARTS];
/// `sp` **từng hart tự đọc** sau khi vào long mode (PA, identity map).
static SMP_HART_SP: [AtomicU64; smp::MAX_HARTS] = [const { AtomicU64::new(0) }; smp::MAX_HARTS];
/// **KT-278** — địa chỉ TSS **hart phụ đọc lại từ descriptor GDT của chính nó** (không phải giá
/// trị nó vừa ghi): bằng chứng `ltr` nạp đúng TSS, và là dữ liệu của phép kiểm địa chỉ.
static SMP_HART_TSS_BASE: [AtomicU64; smp::MAX_HARTS] =
    [const { AtomicU64::new(0) }; smp::MAX_HARTS];
/// `TR` (`str`) **từng hart tự đọc** — phải bằng [`gdt::SELECTOR_TSS`].
static SMP_HART_TR: [AtomicU64; smp::MAX_HARTS] = [const { AtomicU64::new(0) }; smp::MAX_HARTS];
/// Byte `type_attr` của descriptor TSS **sau** `ltr` — CPU đổi `0x89` → `0x8B` (busy).
static SMP_HART_TSS_TYPE: [AtomicU64; smp::MAX_HARTS] =
    [const { AtomicU64::new(0) }; smp::MAX_HARTS];
/// `IST1` **từng hart** đã nạp vào TSS của chính nó — bốn giá trị phải **rời nhau**.
static SMP_HART_IST1: [AtomicU64; smp::MAX_HARTS] = [const { AtomicU64::new(0) }; smp::MAX_HARTS];
/// Số µs BSP đã chờ từng hart phụ online (đo bằng PIT) — bằng chứng "chờ bao lâu".
static SMP_WAIT_ONLINE_US: [AtomicU64; smp::MAX_HARTS] =
    [const { AtomicU64::new(u64::MAX) }; smp::MAX_HARTS];
/// Số µs BSP đã chờ hart phụ báo alive (µs).
static SMP_WAIT_ALIVE_US: AtomicU64 = AtomicU64::new(u64::MAX);
/// **KT-278** — số IPI (vector `0x30`) đã nhận, tăng trong **handler ngắt** của hart đích.
static SMP_IPI_COUNT: AtomicU64 = AtomicU64::new(0);
/// Số µs BSP đã chờ IPI (`u64::MAX` = chưa chờ).
static SMP_WAIT_IPI_US: AtomicU64 = AtomicU64::new(u64::MAX);
/// Số tick timer hart phụ đã nhận (handler vector 32 tăng khi `LAPIC_ID != BSP`).
static AP_TICK_COUNT: AtomicU64 = AtomicU64::new(0);
/// `LAPIC_ID` của BSP — handler timer dùng để **phân biệt** tick của BSP và của AP.
static BSP_LAPIC_ID: AtomicU64 = AtomicU64::new(u64::MAX);
/// `sp` ban đầu của BSP (`__stack_top` linker truyền vào `kmain`) — `run` so với slot 0 của arena.
static BSP_STACK_TOP: AtomicU64 = AtomicU64::new(0);

/// Trần vòng poll `ICR.Delivery Status` — 100 ms (đo bằng PIT), không phải số vòng.
const ICR_IDLE_TIMEOUT_US: u64 = 100_000;

// ---------------------------------------------------------------------------------------------
// Trampoline — 16-bit real mode → 64-bit long mode
// ---------------------------------------------------------------------------------------------
//
// Toàn bộ nằm trong **một** `global_asm!` để thứ tự byte trong section là tường minh:
// code 16-bit → code 64-bit → GDT tạm (0x800) → GDTR 6 byte (0x880) → boot block (0x900) → đệm
// tới đúng 0x1000. Section `.trampoline` do `linker.ld` đặt ở địa chỉ **64 KiB-aligned** bằng
// `. = ALIGN(64K)` **trong** `.text`; BSP copy nguyên trang xuống `0x8000` (xem
// [`copy_trampoline`]).
//
// **Mọi tham chiếu dữ liệu 16-bit** dùng `DS = CS = 0x0800` nên `[0x900]` = `0x8900`; dạng
// `mov bx, 0x900; mov eax, [bx]` là **tường minh** (xem chú thích trong asm). Đoạn 64-bit dùng
// địa chỉ tuyệt đối `[0x8908]` (ModRM `mod=00, rm=100, SIB base=101` — **không** RIP-relative),
// nên cũng đúng sau khi copy.
global_asm!(
    ".section .trampoline, \"ax\"",
    ".p2align 4",
    ".globl __trampoline_start",
    "__trampoline_start:",
    // ---- 16-bit real mode: SIPI đặt CS:IP = 0x0800:0x0000 ⇒ đây là 0x8000.
    ".code16",
    "cli",
    "cld",
    // DS/ES/SS = CS ⇒ mọi offset 16-bit là offset trong trang trampoline.
    "mov ax, cs",
    "mov ds, ax",
    "mov es, ax",
    "mov ss, ax",
    // Stack tạm dưới trang trampoline (0x0800:0x7C00 = 0xFC00) — không đè code/guard.
    "mov sp, 0x7C00",
    // GDT tạm: 3 entry ở 0x8800, pseudo-descriptor 6 byte ở 0x8880.
    "lgdt [0x880]",
    // CR4.PAE = 1 (bắt buộc trước CR3 cho long mode).
    "mov eax, cr4",
    "or eax, 0x20",
    "mov cr4, eax",
    // CR3 = PML4 của BSP (boot block).
    //
    // **Địa chỉ hoá 16-bit**: `mov eax, [0x900]` trong `.code16` biên dịch thành `A1` + **moffs16**
    // (địa chỉ 16-bit) — không phải moffs32 — nên 4 byte sau bị CPU hiểu là lệnh khác. Dùng
    // `[bx]` (ModRM 16-bit, DS:bx) là dạng **tường minh** và đúng ở mọi assembler.
    "mov bx, 0x900",
    "mov eax, [bx]",
    "mov cr3, eax",
    // IA32_EFER.LME|NXE = 1 (boot block).
    "mov ecx, 0xC0000080",
    "rdmsr",
    "mov bx, 0x940",
    "or eax, [bx]",
    "wrmsr",
    // CR0 = PG|PE|ET (giá trị BSP đã dùng — boot block).
    "mov bx, 0x930",
    "mov eax, [bx]",
    "mov cr0, eax",
    // ljmp 0x08:<0x8000 + lm_off> — `0x66` cho offset 32-bit (16:32 far jump).
    ".byte 0x66, 0xEA",
    ".long 0x8000 + (__trampoline_lm - __trampoline_start)",
    ".word 0x08",
    // ---- 64-bit long mode.
    ".code64",
    "__trampoline_lm:",
    "mov ax, 0x10",
    "mov ds, ax",
    "mov es, ax",
    "mov ss, ax",
    "mov fs, ax",
    "mov gs, ax",
    // Stack per-hart (boot block) — đỉnh slot của hart này trong arena.
    //
    // **Địa chỉ tuyệt đối**: ở 64-bit mode `DS`/`SS` có base 0, nên `[0x908]` là **linear 0x908**
    // — phải cộng `0x8000` của trang trampoline. Đo được lần chạy đầu: quên cộng ⇒ `rsp = 0`,
    // `call` đẩy return address xuống `0xFFFF_FFFF_FFFF_FFF8` ⇒ `#PF` rồi `#DF` (AP chết).
    "mov rsp, qword ptr [{boot_stack_top_abs}]",
    // IDT **dùng chung** của BSP (boot block: limit u16 @ 0x920, base u64 @ 0x922).
    "lidt [{boot_idt_limit_abs}]",
    // kmain_secondary(hart) — `call` **tuyệt đối** qua thanh ghi: `call rel32` sẽ sai sau khi
    // copy vì đích không dịch cùng đoạn.
    "mov rdi, qword ptr [{boot_hart_abs}]",
    "mov rax, qword ptr [{boot_entry_abs}]",
    "call rax",
    // `kmain_secondary` không trả về; nếu có thì dừng an toàn thay vì chạy vào rác.
    "1:",
    "cli",
    "hlt",
    "jmp 1b",
    // ---- GDT tạm (3 entry × 8 byte) ở offset 0x800 ⇒ linear 0x8800.
    ".org 0x800",
    "__trampoline_gdt:",
    ".quad 0",
    ".quad {gdt_code}",
    ".quad {gdt_data}",
    // ---- GDTR 6 byte (real mode: limit 16-bit + base 32-bit) ở offset 0x880 ⇒ linear 0x8880.
    ".org 0x880",
    "__trampoline_gdtr:",
    ".word 23",
    ".long 0x8800",
    // ---- Boot block ở offset 0x900 ⇒ linear 0x8900 (BSP ghi lúc chạy; đệm 0 ở đây).
    ".org 0x900",
    "__trampoline_boot:",
    // ---- Đệm tới đúng một trang 4 KiB.
    ".org 0xFFF",
    ".byte 0",
    ".globl __trampoline_end",
    "__trampoline_end:",
    gdt_code = const boot::GDT_KERNEL_CODE,
    gdt_data = const boot::GDT_KERNEL_DATA,
    // Địa chỉ **tuyệt đối** của các trường boot block (trang trampoline + offset).
    boot_stack_top_abs = const smp::trampoline_addr(smp::BOOT_STACK_TOP),
    boot_idt_limit_abs = const smp::trampoline_addr(smp::BOOT_IDT_LIMIT),
    boot_hart_abs = const smp::trampoline_addr(smp::BOOT_HART),
    boot_entry_abs = const smp::trampoline_addr(smp::BOOT_ENTRY),
);

// ---------------------------------------------------------------------------------------------
// Đồng hồ PIT kênh 2 — trần chờ đo bằng thời gian, không phải số vòng
// ---------------------------------------------------------------------------------------------

/// Đọc PIT kênh 2 và trả µs đã trôi qua kể từ `clock.reset`.
fn pit_now(clock: &mut PitClock) -> u64 {
    clock.now_us(pit2_latch_read())
}

/// Chờ `us` µs (đo bằng PIT kênh 2); trả `false` nếu vượt trần `bound_us` (PIT hỏng).
fn wait_us(clock: &mut PitClock, us: u64) -> bool {
    let start = pit_now(clock);
    // Trần = `us` + 1 s: đủ rộng cho mọi trễ TCG, nhưng PIT hỏng thì cắt được.
    let bound = us.saturating_add(1_000_000);
    loop {
        let now = pit_now(clock);
        if now.saturating_sub(start) >= us {
            return true;
        }
        if now.saturating_sub(start) > bound {
            return false;
        }
        core::hint::spin_loop();
    }
}

/// Chờ `ICR.Delivery Status` về 0 (lệnh trước đã gửi xong); `false` nếu quá trần.
fn wait_icr_idle(clock: &mut PitClock) -> bool {
    let start = pit_now(clock);
    loop {
        let low = crate::lapic_read(smp::LAPIC_ICR_LOW);
        if !smp::icr_delivery_pending(low) {
            return true;
        }
        if pit_now(clock).saturating_sub(start) > ICR_IDLE_TIMEOUT_US {
            return false;
        }
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------------------------
// Copy trampoline + boot block
// ---------------------------------------------------------------------------------------------

/// Copy trang trampoline (link ở `.trampoline`, 64 KiB-aligned) xuống `0x8000`.
///
/// Trả `(src, bytes, ok)` — `ok` ⇔ section đúng **một** trang 4 KiB và magic đọc lại khớp sau khi
/// BSP ghi boot block (xem [`publish_boot_block`]).
fn copy_trampoline() -> (u64, u64, bool) {
    let src = core::ptr::addr_of!(__trampoline_start) as u64;
    let end = core::ptr::addr_of!(__trampoline_end) as u64;
    let bytes = end.wrapping_sub(src);
    if bytes != smp::TRAMPOLINE_BYTES {
        return (src, bytes, false);
    }
    // Copy 8 byte một lần: cả hai vùng đều identity-map, đọc/ghi an toàn qua `asm` (không `unsafe`).
    let mut offset = 0u64;
    while offset < bytes {
        let value = asm::read_u64(src + offset);
        asm::write_u64(smp::TRAMPOLINE_BASE + offset, value);
        offset += 8;
    }
    (src, bytes, true)
}

/// Ghi boot block vào trang trampoline (địa chỉ tuyệt đối `0x8900..`) cho hart `hart`.
///
/// Mọi giá trị lấy từ **thanh ghi của BSP** hoặc hằng của crate arch — không có bản thứ hai của
/// con số nào trong tệp này. Gọi **tuần tự** cho từng hart phụ: boot block chỉ có một chỗ, và
/// hart phụ đọc xong **mọi** trường trước khi bật bit online (xem đầu tệp).
fn publish_boot_block(hart: usize, stack_top: u64, entry: u64, lapic_base: u64, lapic_id: u8) {
    let (idt_base, idt_limit) = idt::read_idtr();
    asm::write_u64(
        smp::trampoline_addr(smp::BOOT_PML4),
        asm::read_cr3() & !0xFFF,
    );
    asm::write_u64(smp::trampoline_addr(smp::BOOT_STACK_TOP), stack_top);
    asm::write_u64(smp::trampoline_addr(smp::BOOT_HART), hart as u64);
    asm::write_u64(smp::trampoline_addr(smp::BOOT_ENTRY), entry);
    asm::write_u16(smp::trampoline_addr(smp::BOOT_IDT_LIMIT), idt_limit);
    asm::write_u64(smp::trampoline_addr(smp::BOOT_IDT_BASE), idt_base);
    asm::write_u64(smp::trampoline_addr(smp::BOOT_CR0), asm::read_cr0());
    asm::write_u64(smp::trampoline_addr(smp::BOOT_CR4), asm::read_cr4());
    asm::write_u64(
        smp::trampoline_addr(smp::BOOT_EFER),
        (EFER_LME | EFER_NXE) as u64,
    );
    asm::write_u64(smp::trampoline_addr(smp::BOOT_LAPIC_BASE), lapic_base);
    asm::write_u64(smp::trampoline_addr(smp::BOOT_LAPIC_ID), lapic_id as u64);
    asm::write_u64(smp::trampoline_addr(smp::BOOT_MAGIC), smp::TRAMPOLINE_MAGIC);
}

/// Đọc lại magic trong boot block — bằng chứng **copy + ghi** đã chạy, không chỉ "đã gọi".
fn trampoline_magic() -> u64 {
    asm::read_u64(smp::trampoline_addr(smp::BOOT_MAGIC))
}

// ---------------------------------------------------------------------------------------------
// INIT + SIPI + IPI
// ---------------------------------------------------------------------------------------------

/// Gửi `INIT` rồi **hai** `SIPI` tới `dest` (APIC ID), đúng thứ tự + thời gian của SDM.
///
/// Trả `(init_ok, sipi1_ok, sipi2_ok)` — mỗi bước có bằng chứng riêng để log nói **bước nào**
/// hỏng, không chỉ "đã gửi".
fn send_init_sipi(dest: u8, vector: u8, clock: &mut PitClock) -> (bool, bool, bool) {
    let mut ok = [false; 3];
    // ---- INIT: level assert + level trigger, chờ gửi xong rồi chờ 10 ms.
    if wait_icr_idle(clock) {
        crate::lapic_write(smp::LAPIC_ICR_HIGH, smp::icr_high(dest));
        crate::lapic_write(smp::LAPIC_ICR_LOW, smp::icr_init_low());
        ok[0] = wait_icr_idle(clock) && wait_us(clock, smp::INIT_WAIT_US);
    }
    // **Đột biến (i) của KT-276** (`--features no-sipi`): bỏ **cả hai** SIPI. Hart phụ ở lại
    // trạng thái chờ SIPI ⇒ không online ⇒ TAP 18 ĐỎ. Đây là đường chứng minh TAP 18 **đo** hart
    // phụ thật, không phải "in ok vô điều kiện".
    if cfg!(feature = "no-sipi") {
        return (ok[0], false, false);
    }
    // ---- SIPI #1: edge, vector = addr >> 12; chờ 200 µs trước SIPI #2.
    if ok[0] {
        crate::lapic_write(smp::LAPIC_ICR_HIGH, smp::icr_high(dest));
        crate::lapic_write(smp::LAPIC_ICR_LOW, smp::icr_sipi_low(vector));
        ok[1] = wait_icr_idle(clock) && wait_us(clock, smp::SIPI_WAIT_US);
    }
    // ---- SIPI #2: cùng vector (SIPI có thể bị mất nếu AP chưa sẵn sàng).
    if ok[1] {
        crate::lapic_write(smp::LAPIC_ICR_HIGH, smp::icr_high(dest));
        crate::lapic_write(smp::LAPIC_ICR_LOW, smp::icr_sipi_low(vector));
        ok[2] = wait_icr_idle(clock);
    }
    (ok[0], ok[1], ok[2])
}

/// **KT-278** — gửi **một** IPI fixed-delivery (vector [`exc::VEC_IPI`]) tới `dest` (APIC ID).
///
/// Trả `true` ⇔ lệnh đã được gửi xong (`ICR.Delivery Status` về 0). Việc hart đích **có nhận**
/// hay không là chuyện khác — nó được đo bằng [`SMP_IPI_COUNT`], nên "gửi xong" không bao giờ bị
/// nhầm với "đã nhận" (đó là toàn bộ ý nghĩa của đột biến (ii)).
fn send_ipi(dest: u8, clock: &mut PitClock) -> bool {
    if !wait_icr_idle(clock) {
        return false;
    }
    crate::lapic_write(smp::LAPIC_ICR_HIGH, smp::icr_high(dest));
    crate::lapic_write(smp::LAPIC_ICR_LOW, smp::icr_fixed_low(exc::VEC_IPI));
    wait_icr_idle(clock)
}

/// Số vòng tối đa của **chốt an toàn thời gian** — chỉ dùng khi trần vòng cạn **trước** khi
/// `latch_us` trôi qua (vòng chạy nhanh bất thường); PIT hỏng (elapsed luôn 0) cũng dừng sau
/// chừng ấy vòng ⇒ **không treo**.
const SMP_SPIN_ROUNDS: u32 = 4;

/// Chờ `ready()` đúng — **điều kiện dừng chính là biến đếm**, không phải thời gian (KT-301).
///
/// Trả số µs đã chờ (đo bằng PIT kênh 2, chỉ để **báo**), `None` nếu hết trần **số vòng**.
///
/// # Vì sao đổi cách chờ (KT-301)
///
/// KT-298 soak 600 bắt boot 422: `# smp: ipi sent=1 count=0 waited_us=18446744073709551615 ok=0`
/// **trước** khi `# smp: ipi recv=1` in ra, dưới `qemu_foreign=3 loadavg_1m=6.95`. Trần **thời
/// gian tuyệt đối** đo bằng PIT là **thời gian tường**: host deschedule vCPU hart phụ quá 100 ms
/// thì IPI **không thể** tới trong hạn — phép chờ sai **đại lượng**, không phải ngưỡng quá chặt.
/// Vì vậy điều kiện dừng là **biến đếm** (hart đích tăng trong handler), với trần **số vòng** rất
/// lớn ([`smp::SMP_SPIN_LIMIT`]); thời gian chỉ còn là **chốt an toàn** hỏi **sau** khi trần vòng
/// cạn. Đây là **sửa cách chờ**, không phải nới ngưỡng: `IPI_TIMEOUT_US` (100 ms) và
/// `ONLINE_TIMEOUT_US` (2 s) **giữ nguyên**.
fn wait_counter(
    clock: &mut PitClock,
    latch_us: u64,
    mut ready: impl FnMut() -> bool,
) -> Option<u64> {
    let start = pit_now(clock);
    let mut spins = 0u64;
    let mut rounds = 0u32;
    loop {
        if ready() {
            return Some(pit_now(clock).saturating_sub(start));
        }
        if spins >= smp::SMP_SPIN_LIMIT {
            // Trần **số vòng** đã cạn — điều kiện dừng **chính**. Chốt an toàn **thời gian**:
            // chỉ khi PIT đã trôi quá `latch_us` mới kết luận hết trần; nếu chưa (vòng chạy
            // nhanh bất thường) thì chờ thêm một vòng, tối đa [`SMP_SPIN_ROUNDS`] vòng.
            if rounds + 1 >= SMP_SPIN_ROUNDS || pit_now(clock).saturating_sub(start) > latch_us {
                return None;
            }
            rounds += 1;
            spins = 0;
            continue;
        }
        spins += 1;
        // `pause` mỗi 64 vòng: nhường host scheduler cho vCPU hart phụ. Đo được `pause` ~0,5 µs
        // dưới TCG nên pause **mỗi** vòng làm vòng chậm ~40× (10 triệu vòng = 5,3 s thay vì
        // 0,13 s) — không dùng mỗi vòng.
        if spins & 0x3F == 0 {
            core::hint::spin_loop();
        }
        // Nạp ảnh PIT mỗi 65.536 vòng (~1,7 ms): `PitClock` chỉ xử lý **một** lần bọc (54,9 ms)
        // giữa hai lần đọc nên không được để thưa hơn.
        if spins & 0xFFFF == 0 {
            let _ = pit_now(clock);
        }
    }
}

/// Chờ bit `hart` trong `mask` bật; trả số µs đã chờ, `None` nếu hết trần **số vòng**.
fn wait_mask(mask: &AtomicU32, hart: usize, clock: &mut PitClock) -> Option<u64> {
    wait_counter(clock, smp::ONLINE_TIMEOUT_US, || {
        mask.load(Ordering::Acquire) & (1u32 << hart) != 0
    })
}

/// In một danh sách `u64` dạng `[0x…, 0x…]` **một dòng** — `{:#x?}` của `core::fmt` bật
/// **pretty-print nhiều dòng** (cờ `#` của `Debug`), làm hỏng hợp đồng "mỗi dòng `# smp:` là một
/// dòng log" của runner. Hàm này in tường minh nên không phụ thuộc `SMP_HARTS`.
///
/// **KT-316** — cả danh sách nằm trong **một** lần giữ khoá COM1: dòng này ghép từ nhiều `write!`
/// (mỗi `write!` là một `write_fmt` riêng), nên khoá từng `write!` sẽ để hart khác chen vào giữa
/// `[` và `]`.
fn write_hex_list(console: &mut Console, values: &[u64]) {
    console.with_lock(|raw| {
        let _ = write!(raw, "[");
        let mut i = 0;
        while i < values.len() {
            if i > 0 {
                let _ = write!(raw, ", ");
            }
            let _ = write!(raw, "0x{:x}", values[i]);
            i += 1;
        }
        let _ = write!(raw, "]");
    });
}

/// Chờ `SMP_IPI_COUNT >= 1`; trả số µs đã chờ, `None` nếu hết trần **số vòng**.
///
/// Trần **hữu hạn** là điều kiện của đột biến (ii): IPI tới một `APIC ID` không tồn tại (hoặc
/// hart đích nhận mà **không** tăng bộ đếm — `--features no-ipi-count`) thì BSP phải **đi tiếp**
/// (TAP 20 ĐỎ) — **không treo**. Trần là **số vòng** ([`smp::SMP_SPIN_LIMIT`]), không phải thời
/// gian: xem [`wait_counter`].
fn wait_ipi(clock: &mut PitClock) -> Option<u64> {
    wait_counter(clock, smp::IPI_TIMEOUT_US, || {
        SMP_IPI_COUNT.load(Ordering::Acquire) >= 1
    })
}

// ---------------------------------------------------------------------------------------------
// BSP — smp::run
// ---------------------------------------------------------------------------------------------

/// **KT-278** — đánh thức hart 1..3, kiểm TSS per-hart, gửi IPI, in bằng chứng, phát TAP 18..21.
///
/// Gọi từ `net::run` **sau** TAP 17 và **trước** marker: chưa có procsrv/scheduler nên đây là
/// chỗ duy nhất kernel còn chạy được sau `Exit` của ring 3. Thứ tự TAP 1..17 **không đổi**.
pub fn run(console: &mut Console, tap_first: usize) {
    let mut clock = PitClock::new();
    clock.reset(pit2_latch_read());

    // ---- 1. Arena stack per-hart: `linker.ld` và crate arch phải khớp **tuyệt đối**.
    let arena = core::ptr::addr_of!(__smp_stacks_start) as u64;
    let arena_end = core::ptr::addr_of!(__smp_stacks_end) as u64;
    let arena_bytes = arena_end.wrapping_sub(arena);
    let stacks = match HartStacks::new(arena) {
        Some(stacks) => stacks,
        None => {
            let _ = writeln!(console, "PANIC: smp: arena 0x{arena:x} không căn 4 KiB");
            asm::halt(4)
        }
    };
    let layout = stacks.check();
    let layout_ok = layout.is_ok() && arena_bytes == smp::HART_AREA_BYTES;
    let _ = writeln!(
        console,
        "# smp: arena pa=0x{arena:x} bytes={arena_bytes} expect={} layout={layout:?} ok={}",
        smp::HART_AREA_BYTES,
        layout_ok as u8,
    );
    if !layout_ok {
        let _ = writeln!(console, "PANIC: smp: bố cục arena stack không khớp linker");
        asm::halt(4)
    }

    // BSP **luôn** online (nó đang chạy đây): bật bit 0 để `SMP_ONLINE_MASK` là mặt nạ **cả bốn**
    // hart, không phải "ba hart phụ" — TAP 18/21 so `alive == online` trên cùng mặt nạ.
    SMP_ONLINE_MASK.fetch_or(1, Ordering::Release);

    // ---- 2. Copy trampoline xuống 0x8000 (một lần cho cả ba hart phụ).
    let (src, bytes, copy_ok) = copy_trampoline();
    let lapic_base = crate::lapic_base();
    let bsp_lapic_id = crate::lapic_id();
    let entry = kmain_secondary as *const () as u64;
    let vector = match smp::sipi_vector(smp::TRAMPOLINE_BASE) {
        Some(vector) => vector,
        None => {
            let _ = writeln!(console, "PANIC: smp: trampoline không có SIPI vector");
            asm::halt(4)
        }
    };
    // **Đột biến (ii) của KT-276** (`--features bad-trampoline`): SIPI tới **trang bên cạnh**
    // (0x9000) — vùng không có code trampoline. Hart phụ chạy vào vùng 0 rồi kẹt, **không**
    // online; BSP hết trần 2 s ⇒ TAP 18 ĐỎ, không marker.
    let vector = if cfg!(feature = "bad-trampoline") {
        vector.wrapping_add(1)
    } else {
        vector
    };

    // ---- 3. Từng hart phụ, **tuần tự**: ghi boot block → INIT+SIPI+SIPI → chờ online.
    let mut init_ok = [false; smp::MAX_HARTS];
    let mut sipi_ok = [false; smp::MAX_HARTS];
    let mut waited = [None; smp::MAX_HARTS];
    let mut booted = 0usize;
    for hart in 1..smp::SMP_HARTS {
        let ap_stack_top = match stacks.stack_top(hart) {
            Some(top) => top,
            None => {
                let _ = writeln!(console, "PANIC: smp: hart {hart} ngoài arena");
                asm::halt(4)
            }
        };
        let dest = match smp::apic_id_for_hart(hart) {
            Some(id) => id,
            None => {
                let _ = writeln!(console, "PANIC: smp: không có APIC ID cho hart {hart}");
                asm::halt(4)
            }
        };
        publish_boot_block(hart, ap_stack_top, entry, lapic_base, dest);
        let magic = trampoline_magic();
        let tramp_ok = copy_ok && magic == smp::TRAMPOLINE_MAGIC;
        if hart == 1 {
            let _ = writeln!(
                console,
                "# smp: trampoline base=0x{:x} bytes={bytes} src=0x{src:x} magic=0x{magic:016x} ok={}",
                smp::TRAMPOLINE_BASE,
                tramp_ok as u8,
            );
            if !tramp_ok {
                let _ = writeln!(console, "PANIC: smp: copy trampoline thất bại");
                asm::halt(4)
            }
            let _ = writeln!(
                console,
                "# smp: boot block pml4=0x{:x} stack=0x{ap_stack_top:x} hart={hart} entry=0x{entry:x} idt=0x{:x}/0x{:x} cr0=0x{:x} cr4=0x{:x} efer=0x{:x} lapic=0x{lapic_base:x} bsp_id={bsp_lapic_id} dest={dest}",
                asm::read_cr3() & !0xFFF,
                idt::read_idtr().0,
                idt::read_idtr().1,
                asm::read_cr0(),
                asm::read_cr4(),
                (EFER_LME | EFER_NXE) as u64,
            );
        }
        // In **trước** khi gửi: hart phụ in `# smp: hart=N online` ngay khi lên (đo được ~171 µs
        // sau SIPI), nên in sau `send_init_sipi` có thể xen ký tự với dòng của AP trên COM1.
        let _ = writeln!(
            console,
            "# smp: icr init dest={dest} low=0x{:04x} high=0x{:08x} sipi vector=0x{vector:02x} low=0x{:04x}",
            smp::icr_init_low(),
            smp::icr_high(dest),
            smp::icr_sipi_low(vector),
        );
        let (i_ok, s1_ok, s2_ok) = send_init_sipi(dest, vector, &mut clock);
        init_ok[hart] = i_ok;
        sipi_ok[hart] = s1_ok && s2_ok;
        waited[hart] = wait_mask(&SMP_ONLINE_MASK, hart, &mut clock);
        SMP_WAIT_ONLINE_US[hart].store(waited[hart].unwrap_or(u64::MAX), Ordering::Release);
        let ap_lapic_id = SMP_HART_LAPIC_ID[hart].load(Ordering::Acquire);
        let ap_sp = SMP_HART_SP[hart].load(Ordering::Acquire);
        let ap_slot = stacks.slot_of(ap_sp);
        let _ = writeln!(
            console,
            "# smp: cpu_on hart={hart} online={} waited_us={} lapic_id={ap_lapic_id} sp=0x{ap_sp:x} slot={ap_slot:?} init_ok={} sipi_ok={}/{}",
            waited[hart].is_some() as u8,
            waited[hart].unwrap_or(u64::MAX),
            i_ok as u8,
            s1_ok as u8,
            s2_ok as u8,
        );
        if waited[hart].is_none() {
            // Fail-fast: hart này không lên thì ba hart sau cũng không có nghĩa, và chờ tiếp chỉ
            // đốt ngân sách của runner.
            break;
        }
        booted += 1;
    }

    // ---- 4. TAP 18: **bốn** hart online — mask đủ, LAPIC ID đúng, stack **khác slot**.
    let online = SMP_ONLINE_MASK.load(Ordering::Acquire);
    let bsp_slot = stacks.slot_of_top(bsp_stack_top());
    let mut online_ok = online == smp::SMP_EXPECT_MASK && bsp_slot == Some(0);
    let mut slots = [None; smp::MAX_HARTS];
    slots[0] = bsp_slot;
    for hart in 1..smp::SMP_HARTS {
        let dest = smp::apic_id_for_hart(hart).unwrap_or(u8::MAX);
        let ap_lapic_id = SMP_HART_LAPIC_ID[hart].load(Ordering::Acquire);
        let ap_sp = SMP_HART_SP[hart].load(Ordering::Acquire);
        slots[hart] = stacks.slot_of(ap_sp);
        online_ok &= init_ok[hart]
            && sipi_ok[hart]
            && waited[hart].is_some()
            && online & (1u32 << hart) != 0
            && ap_lapic_id == dest as u64
            && slots[hart] == Some(hart);
    }
    // Bốn slot phải **đôi một khác nhau** — đột biến (iii) "stack chồng nhau" của KT-276, mở rộng
    // cho bốn hart.
    for a in 0..smp::SMP_HARTS {
        for b in (a + 1)..smp::SMP_HARTS {
            online_ok &= slots[a].is_some() && slots[a] != slots[b];
        }
    }
    let _ = writeln!(
        console,
        "# smp: online mask=0x{online:x} expect=0x{:x} harts={} booted={booted} slots={:?}",
        smp::SMP_EXPECT_MASK,
        online.count_ones(),
        &slots[..smp::SMP_HARTS],
    );
    check(console, tap_first, "smp: 4 harts online", online_ok);

    // ---- 5. TAP 19: **TSS per-hart** — bốn địa chỉ rời nhau, `tr=0x28`, descriptor busy.
    //
    // BSP đọc base từ **descriptor của chính nó** (không tin `tss::base(0)`), hart phụ đã công bố
    // base đọc từ descriptor của nó ⇒ phép kiểm là "kiểm **địa chỉ**", đúng yêu cầu của KT-278.
    let bsp_tss = gdt::tss_base_of(
        gdt::entry(0, gdt::INDEX_TSS),
        gdt::entry(0, gdt::INDEX_TSS + 1),
    );
    let mut tss_bases = [0u64; smp::MAX_HARTS];
    let mut ist1_tops = [0u64; smp::MAX_HARTS];
    tss_bases[0] = bsp_tss;
    ist1_tops[0] = tss::ist(0, 1);
    let mut tss_ok = bsp_tss == tss::base(0)
        && asm::str_reg() == gdt::SELECTOR_TSS
        && gdt::tss_type_of(gdt::entry(0, gdt::INDEX_TSS)) == gdt::TSS_TYPE_BUSY_64
        && ist1_tops[0] == ist1_top(0);
    for hart in 1..smp::SMP_HARTS {
        tss_bases[hart] = SMP_HART_TSS_BASE[hart].load(Ordering::Acquire);
        ist1_tops[hart] = SMP_HART_IST1[hart].load(Ordering::Acquire);
        tss_ok &= tss_bases[hart] == tss::base(hart)
            && SMP_HART_TR[hart].load(Ordering::Acquire) == gdt::SELECTOR_TSS as u64
            && SMP_HART_TSS_TYPE[hart].load(Ordering::Acquire) == gdt::TSS_TYPE_BUSY_64 as u64
            && ist1_tops[hart] == ist1_top(hart);
    }
    // **Kiểm địa chỉ**: bốn vùng TSS rời nhau, bốn stack IST1 rời nhau.
    let distinct = tss::check_distinct(&tss_bases[..smp::SMP_HARTS]);
    let ist1_distinct = tss::check_distinct(&ist1_tops[..smp::SMP_HARTS]);
    tss_ok &= distinct.is_ok() && ist1_distinct.is_ok();
    let _ = write!(console, "# smp: tss bases=");
    write_hex_list(console, &tss_bases[..smp::SMP_HARTS]);
    let _ = write!(console, " ist1=");
    write_hex_list(console, &ist1_tops[..smp::SMP_HARTS]);
    let _ = writeln!(
        console,
        " distinct={distinct:?} ist1_distinct={ist1_distinct:?} tr=0x{:x} type=0x{:02x} ok={}",
        asm::str_reg(),
        gdt::tss_type_of(gdt::entry(0, gdt::INDEX_TSS)),
        tss_ok as u8,
    );
    check(console, tap_first + 1, "smp: tss per-hart", tss_ok);

    // ---- 6. TAP 20: **IPI** fixed-delivery hart 0 → hart 1 (hart 1 đếm trong **handler ngắt**).
    //
    // **Đột biến (i)** (`--features no-ipi`): **không** gửi ⇒ `recv=0` ⇒ TAP 20 ĐỎ.
    // **Đột biến (ii)** (`--features bad-ipi-dest`): gửi tới `APIC ID = 7` — hart **không tồn tại**
    // với `-smp 4` ⇒ không ai nhận, BSP hết trần 100 ms rồi **đi tiếp** (không treo) ⇒ TAP 20 ĐỎ.
    let ipi_dest = if cfg!(feature = "bad-ipi-dest") {
        7u8
    } else {
        1u8
    };
    // In **trước** khi gửi (cùng lý do với dòng `icr init`): hart đích in
    // `# smp: ipi recv=1 intid=0x30` ngay khi nhận, nên in sau `send_ipi` sẽ xen ký tự trên COM1
    // (đo được ở lần chạy đầu: `# smp: ipi send … low# smp: ipi r=0x4030 …`).
    let _ = writeln!(
        console,
        "# smp: ipi send dest={ipi_dest} vector=0x{:02x} low=0x{:04x} high=0x{:08x}",
        exc::VEC_IPI,
        smp::icr_fixed_low(exc::VEC_IPI),
        smp::icr_high(ipi_dest),
    );
    let ipi_sent = if cfg!(feature = "no-ipi") {
        false
    } else {
        send_ipi(ipi_dest, &mut clock)
    };
    let ipi_waited = wait_ipi(&mut clock);
    SMP_WAIT_IPI_US.store(ipi_waited.unwrap_or(u64::MAX), Ordering::Release);
    let ipi_count = SMP_IPI_COUNT.load(Ordering::Acquire);
    let ipi_ok = ipi_sent && ipi_waited.is_some() && ipi_count == 1;
    let _ = writeln!(
        console,
        "# smp: ipi sent={} count={ipi_count} waited_us={} ok={}",
        ipi_sent as u8,
        ipi_waited.unwrap_or(u64::MAX),
        ipi_ok as u8,
    );
    check(console, tap_first + 2, "smp: ipi delivered", ipi_ok);

    // ---- 7. Lệnh `CMD_REPORT` cho **từng** hart phụ, tuần tự; chờ nó **in xong** rồi mới sang
    // hart kế (quy tắc "in trước, đếm sau" — bài học KT-259: hai hart in xen ký tự trên COM1).
    let mut report_ok = true;
    let mut alive_wait_total = 0u64;
    for hart in 1..smp::SMP_HARTS {
        SMP_CMD[hart].store(CMD_REPORT, Ordering::Release);
        let waited_alive = wait_mask(&SMP_ALIVE_MASK, hart, &mut clock);
        alive_wait_total = alive_wait_total.saturating_add(waited_alive.unwrap_or(u64::MAX));
        let alive = SMP_ALIVE_MASK.load(Ordering::Acquire) & (1u32 << hart) != 0;
        let _ = writeln!(
            console,
            "# smp: report hart={hart} done={} waited_us={} alive={}",
            waited_alive.is_some() as u8,
            waited_alive.unwrap_or(u64::MAX),
            alive as u8,
        );
        report_ok &= waited_alive.is_some() && alive;
    }
    SMP_WAIT_ALIVE_US.store(alive_wait_total, Ordering::Release);
    if report_ok {
        let _ = writeln!(console, "# smp: hart=0 alive");
        SMP_ALIVE_MASK.fetch_or(1, Ordering::Release);
    }
    let alive = SMP_ALIVE_MASK.load(Ordering::Acquire);
    let online = SMP_ONLINE_MASK.load(Ordering::Acquire);
    let _ = writeln!(
        console,
        "# smp: online mask=0x{online:x} alive=0x{alive:x} expect=0x{:x} harts={} ap_ticks={} waited_online_us={} waited_alive_us={}",
        smp::SMP_EXPECT_MASK,
        online.count_ones(),
        ap_ticks(),
        wait_online_us(),
        wait_alive_us(),
    );
    // TAP 21: **cả bốn** hart đã báo alive (bit 0 do BSP, bit 1..3 do hart phụ).
    check(
        console,
        tap_first + 3,
        "smp: all harts report",
        report_ok && alive == smp::SMP_EXPECT_MASK,
    );
}

// ---------------------------------------------------------------------------------------------
// AP — kmain_secondary
// ---------------------------------------------------------------------------------------------

/// **KT-276 + KT-278** — điểm vào Rust của **hart phụ** (trampoline gọi sau khi đã vào long mode).
///
/// `hart` = hart id BSP ghi vào boot block (trampoline đọc rồi truyền qua `rdi`).
///
/// Thứ tự **bắt buộc**:
///
/// 1. kiểm `sp` nằm trong **slot của chính mình** — đột biến (iii) "stack chồng nhau" lộ ra ở đây;
/// 2. **TSS riêng + GDT riêng + `ltr`** (KT-278): `tss::init(hart, …)`, `gdt::install(hart, …)`,
///    rồi **đọc lại** base từ descriptor + `str` + type busy — bằng chứng `ltr` nạp **đúng** TSS
///    của hart này, không phải TSS của hart khác;
/// 3. bật LAPIC **của hart này** (`IA32_APIC_BASE`, `SVR`, `TPR`, mask `LINT0/1`) và **so**
///    `LAPIC_ID` với giá trị BSP đã ghi — lệch ⇒ PANIC (không tin chỉ số vòng lặp);
/// 4. in `# smp: hart=<id> online` rồi bật bit online;
/// 5. cấu hình LAPIC timer (vector 32, periodic) + `sti` — nguồn **đánh thức** để vòng `hlt`
///    kiểm lệnh; **không** phải nguồn tick của hệ (PIT của BSP vẫn là nguồn tick);
/// 6. vòng `hlt`: thấy `CMD_REPORT` ⇒ in `# smp: hart=<id> alive` + bật bit alive. IPI (vector
///    `0x30`) do handler chung xử lý ([`note_ipi`]).
#[no_mangle]
pub extern "C" fn kmain_secondary(hart: u64) -> ! {
    let mut console = Console;
    let hart = hart as usize;
    if !smp::valid_hart(hart) {
        let _ = writeln!(console, "PANIC: smp: hart={hart} ngoài MAX_HARTS");
        asm::halt(4)
    }

    // ---- 1. Stack per-hart: `sp` phải nằm trong slot của **chính** hart này.
    let arena = core::ptr::addr_of!(__smp_stacks_start) as u64;
    let stacks = match HartStacks::new(arena) {
        Some(stacks) => stacks,
        None => {
            let _ = writeln!(console, "PANIC: smp: arena 0x{arena:x} không căn 4 KiB");
            asm::halt(4)
        }
    };
    let sp = asm::read_rsp();
    let slot = stacks.slot_of(sp);
    if slot != Some(hart) {
        let _ = writeln!(
            console,
            "PANIC: smp: sp=0x{sp:x} không thuộc slot {hart} (slot={slot:?}) — stack per-hart sai"
        );
        asm::halt(4)
    }
    let stack_top = stacks.stack_top(hart).unwrap_or(0);

    // ---- 1b. IDT **dùng chung**: trampoline đã `lidt` từ boot block; nạp lại bằng API của crate
    // (idempotent) rồi đọc lại bằng `sidt` — bằng chứng `IDTR` của **hart này** trỏ đúng bảng,
    // không phải "tin trampoline đã làm đúng".
    idt::reload();
    let (idt_base, idt_limit) = idt::read_idtr();
    if idt_base != idt::base() || idt_limit != idt::IDT_LIMIT {
        let _ = writeln!(
            console,
            "PANIC: smp: hart {hart} IDTR=0x{idt_base:x}/0x{idt_limit:x} lệch bảng 0x{:x}/0x{:x}",
            idt::base(),
            idt::IDT_LIMIT,
        );
        asm::halt(4)
    }

    // ---- 1c. **KT-278** — TSS **riêng** + GDT **riêng** + `ltr`.
    //
    // `IST1` của hart này là stack IST1 **của nó** (`__ist1_top + hart * 0x1000`): hai hart cùng
    // `#DF` không còn ghi vào một stack. `RSP0` = đỉnh slot stack của hart (AP không chạy ring 3,
    // nhưng TSS phải hợp lệ — và giá trị **khác nhau** giữa các hart là bằng chứng per-hart).
    let ist1 = ist1_top(hart);
    if ist1 == 0 {
        let _ = writeln!(console, "PANIC: smp: hart {hart} không có stack IST1");
        asm::halt(4)
    }
    tss::init(hart, stack_top, ist1);
    // **Đột biến (iii) của KT-278** (`--features same-tss`): descriptor TSS của hart phụ trỏ
    // **TSS của hart 0** — đúng lỗi "TSS per-hart trùng địa chỉ". Không fault (TSS 0 hợp lệ),
    // nhưng phép **kiểm địa chỉ** của BSP thấy trùng ⇒ TAP 19 ĐỎ.
    let tss_base = if cfg!(feature = "same-tss") {
        tss::base(0)
    } else {
        tss::base(hart)
    };
    gdt::install(hart, tss_base);
    // Đọc lại **từ descriptor** (không tin giá trị vừa ghi) + `str` + type busy.
    let tss_low = gdt::entry(hart, gdt::INDEX_TSS);
    let tss_high = gdt::entry(hart, gdt::INDEX_TSS + 1);
    let loaded_tss = gdt::tss_base_of(tss_low, tss_high);
    let tr = asm::str_reg();
    let tss_type = gdt::tss_type_of(tss_low);
    let (gdt_base, gdt_limit) = gdt::read_gdtr(hart);
    if tr != gdt::SELECTOR_TSS
        || tss_type != gdt::TSS_TYPE_BUSY_64
        || gdt_base != gdt::base(hart)
        || gdt_limit != gdt::GDT_LIMIT
    {
        let _ = writeln!(
            console,
            "PANIC: smp: hart {hart} GDT/TSS sai tr=0x{tr:x} type=0x{tss_type:02x} gdt=0x{gdt_base:x}/0x{gdt_limit:x} cần 0x{:x}/0x{:x}",
            gdt::base(hart),
            gdt::GDT_LIMIT,
        );
        asm::halt(4)
    }
    SMP_HART_TSS_BASE[hart].store(loaded_tss, Ordering::Release);
    SMP_HART_TR[hart].store(tr as u64, Ordering::Release);
    SMP_HART_TSS_TYPE[hart].store(tss_type as u64, Ordering::Release);
    SMP_HART_IST1[hart].store(tss::ist(hart, 1), Ordering::Release);

    // ---- 2. LAPIC của hart này.
    let mut msr = asm::read_msr(MSR_IA32_APIC_BASE);
    if msr & apic::APIC_BASE_ENABLE == 0 {
        msr |= apic::APIC_BASE_ENABLE;
        asm::write_msr(MSR_IA32_APIC_BASE, msr);
    }
    let base = msr & apic::APIC_BASE_ADDR_MASK;
    let expected_id = asm::read_u64(smp::trampoline_addr(smp::BOOT_LAPIC_ID)) as u8;
    let id_raw = asm::read_u32(base + apic::LAPIC_ID as u64);
    let id = apic::lapic_id(id_raw);
    if id != expected_id {
        let _ = writeln!(
            console,
            "PANIC: smp: hart {hart} LAPIC_ID={id} nhưng BSP ghi {expected_id} — topology lệch"
        );
        asm::halt(4)
    }
    // Bật APIC bằng phần mềm + TPR = 0 + mask LINT0/LINT1 (nguồn ngắt duy nhất là timer của AP
    // và IPI từ BSP).
    asm::write_u32(
        base + apic::LAPIC_SVR as u64,
        apic::svr_value(exc::VEC_SPURIOUS),
    );
    asm::write_u32(base + apic::LAPIC_TPR as u64, 0);
    asm::write_u32(base + apic::LAPIC_LVT_LINT0 as u64, apic::LVT_MASKED);
    asm::write_u32(base + apic::LAPIC_LVT_LINT1 as u64, apic::LVT_MASKED);
    let svr = asm::read_u32(base + apic::LAPIC_SVR as u64);
    let tpr = asm::read_u32(base + apic::LAPIC_TPR as u64);

    // ---- 3. Báo online **sau** khi LAPIC + TSS sẵn sàng (BSP gửi lệnh ngay khi thấy cờ này).
    let _ = writeln!(console, "# smp: hart={hart} online");
    let _ = writeln!(
        console,
        "# smp: hart={hart} lapic id={id} base=0x{base:x} svr=0x{svr:03x} tpr={tpr} sp=0x{sp:x} slot={hart} stack=0x{:x}-0x{:x}",
        stacks.stack(hart).map(|(b, _)| b).unwrap_or(0),
        stacks.stack(hart).map(|(_, t)| t).unwrap_or(0),
    );
    let _ = writeln!(
        console,
        "# smp: hart={hart} tss base=0x{loaded_tss:x} tr=0x{tr:x} type=0x{tss_type:02x} ist1=0x{ist1:x} rsp0=0x{stack_top:x}",
    );
    SMP_HART_LAPIC_ID[hart].store(id as u64, Ordering::Release);
    SMP_HART_SP[hart].store(sp, Ordering::Release);
    SMP_ONLINE_MASK.fetch_or(1u32 << hart, Ordering::Release);

    // ---- 4. LAPIC timer **của hart này** — nhịp đánh thức cho vòng `hlt`.
    //
    // Vector 32 dùng **chung** stub/handler với BSP; handler phân biệt tick của ai bằng `LAPIC_ID`
    // (MMIO đọc trên chính hart đang chạy) nên `TICK_COUNT` của BSP **không** bị hart phụ làm bẩn.
    let divide = apic::timer_divide_code(smp::AP_TIMER_DIVIDE).unwrap_or(apic::TIMER_DIVIDE_BY_1);
    asm::write_u32(base + apic::LAPIC_TIMER_DIVIDE as u64, divide);
    asm::write_u32(
        base + apic::LAPIC_LVT_TIMER as u64,
        apic::lvt_timer_value(exc::VEC_TIMER, true),
    );
    asm::write_u32(
        base + apic::LAPIC_TIMER_INITIAL_COUNT as u64,
        smp::AP_TIMER_INITIAL_COUNT,
    );
    asm::sti();

    // ---- 5. Vòng lệnh: `hlt` (timer/IPI đánh thức) → kiểm `SMP_CMD` → in + bật bit.
    loop {
        let cmd = SMP_CMD[hart].load(Ordering::Acquire);
        if cmd == CMD_REPORT {
            // **In trước, đếm sau** — BSP chờ đúng bit này rồi mới in dòng của nó.
            let _ = writeln!(console, "# smp: hart={hart} alive");
            SMP_ALIVE_MASK.fetch_or(1u32 << hart, Ordering::Release);
            SMP_CMD[hart].store(CMD_NONE, Ordering::Release);
        }
        asm::hlt();
    }
}

// ---------------------------------------------------------------------------------------------
// Truy vấn trạng thái cho `main.rs` (handler timer/IPI + TAP)
// ---------------------------------------------------------------------------------------------

/// Ghi `sp` ban đầu của BSP — gọi **một lần** trong `kmain` (giá trị `__stack_top` của linker).
pub(crate) fn set_bsp_stack_top(top: u64) {
    BSP_STACK_TOP.store(top, Ordering::Release);
}

/// `sp` ban đầu của BSP (`0` = chưa đặt).
pub(crate) fn bsp_stack_top() -> u64 {
    BSP_STACK_TOP.load(Ordering::Acquire)
}

/// Ghi `LAPIC_ID` của BSP — gọi **một lần** trong `lapic_init` trước khi bật ngắt.
pub(crate) fn set_bsp_lapic_id(id: u8) {
    BSP_LAPIC_ID.store(id as u64, Ordering::Release);
}

/// `LAPIC_ID` của BSP (`u64::MAX` = chưa đặt).
pub(crate) fn bsp_lapic_id() -> u64 {
    BSP_LAPIC_ID.load(Ordering::Acquire)
}

/// **KT-310** — chỉ số hart sở hữu `LAPIC_ID` này (`None` = chưa hart nào công bố id ấy).
///
/// Handler timer đọc `LAPIC_ID` qua MMIO (**của hart đang chạy**) nên đây là đường **duy nhất**
/// biết "tick này của hart nào" mà không tin một biến toàn cục. Bảng [`SMP_HART_LAPIC_ID`] do
/// **chính hart phụ** ghi trước khi bật bit online, nên tra bảng là tra dữ liệu hart ấy công bố.
pub(crate) fn hart_of_lapic(id: u8) -> Option<usize> {
    let mut hart = 0;
    while hart < smp::SMP_HARTS {
        if SMP_HART_LAPIC_ID[hart].load(Ordering::Acquire) == id as u64 {
            return Some(hart);
        }
        hart += 1;
    }
    None
}

/// Tăng bộ đếm tick của hart phụ — handler vector 32 gọi khi `LAPIC_ID != BSP`.
pub(crate) fn note_ap_tick() {
    AP_TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Số tick timer hart phụ đã nhận (bằng chứng LAPIC per-hart chạy thật).
pub(crate) fn ap_ticks() -> u64 {
    AP_TICK_COUNT.load(Ordering::Relaxed)
}

/// **KT-278** — ghi nhận **một** IPI (vector `0x30`) vừa tới; gọi từ handler ngắt của hart đích.
///
/// **In trước, đếm sau**: BSP chờ `SMP_IPI_COUNT` nên khi nó thấy `1` thì dòng
/// `# smp: ipi recv=1 intid=0x30` **chắc chắn** đã ra COM1. **KT-316** — "in trước" chỉ bảo đảm
/// **thứ tự**; dòng ra **nguyên vẹn** là nhờ khoá COM1 (trước KT-316 nó có thể bị hart khác xen
/// ký tự).
pub(crate) fn note_ipi() {
    // **KT-301** — kiểm **đỏ-trước** (`--features slow-ipi`): hart đích **nhận** IPI nhưng trì
    // hoãn việc tăng bộ đếm **150 ms** (đo bằng PIT kênh 2). Đây là mô phỏng **tất định** của độ
    // trễ lập lịch host đã làm boot 422/600 của KT-298 đỏ: `wait_ipi` cũ hết trần **thời gian**
    // 100 ms **trước** khi `# smp: ipi recv=1` in ra. Chỉ dùng cho kiểm, không phải feature sản phẩm.
    if cfg!(feature = "slow-ipi") {
        let mut clock = PitClock::new();
        clock.reset(pit2_latch_read());
        while clock.now_us(pit2_latch_read()) < SLOW_IPI_DELAY_US {
            core::hint::spin_loop();
        }
    }
    let n = SMP_IPI_COUNT.load(Ordering::Acquire) + 1;
    let mut console = Console;
    let _ = writeln!(console, "# smp: ipi recv={n} intid=0x{:02x}", exc::VEC_IPI,);
    // **KT-301** — đột biến (ii) (`--features no-ipi-count`): hart đích **nhận** IPI và in dòng
    // `# smp: ipi recv=1` nhưng **không** tăng bộ đếm ⇒ BSP chờ hết **trần số vòng** rồi đi tiếp
    // (TAP 20 ĐỎ, **không treo**). Chứng minh điều kiện dừng là **biến đếm**, không phải thời gian.
    if !cfg!(feature = "no-ipi-count") {
        SMP_IPI_COUNT.store(n, Ordering::Release);
    }
}

/// Số µs BSP đã chờ hart phụ online (`u64::MAX` = chưa chờ) — tổng trên mọi hart phụ.
pub(crate) fn wait_online_us() -> u64 {
    let mut total = 0u64;
    let mut hart = 1;
    while hart < smp::SMP_HARTS {
        let value = SMP_WAIT_ONLINE_US[hart].load(Ordering::Acquire);
        if value == u64::MAX {
            return u64::MAX;
        }
        total = total.saturating_add(value);
        hart += 1;
    }
    total
}

/// Số µs BSP đã chờ hart phụ báo alive (`u64::MAX` = chưa chờ).
pub(crate) fn wait_alive_us() -> u64 {
    SMP_WAIT_ALIVE_US.load(Ordering::Acquire)
}
