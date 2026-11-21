// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-276** (K5' bước 6) — SMP 2 hart trên x86_64: trampoline real mode → long mode, `INIT` +
//! `SIPI` qua ICR, LAPIC riêng cho hart phụ, TAP 18/19.
//!
//! Tách khỏi `main.rs` vì luật **R1** (`v2/bin/v2 rules`): tệp trong `kernel/` ≤ **1.500 dòng**.
//! Tên mô-đun là `smp` (không đụng `vifast_plat_x86_64::smp` — crate arch giữ phần **dữ liệu**:
//! arena, ICR, bố cục trampoline; tệp này giữ phần **điều phối** + asm trampoline).
//!
//! # Đường đánh thức hart phụ
//!
//! ```text
//!   BSP (kmain → net::run → smp::run)
//!     │ 1. copy trang trampoline (link 64 KiB-aligned trong `.text`) xuống 0x8000
//!     │ 2. ghi boot block vào 0x8900 (PML4, stack, hart, entry, IDTR, CR0/CR4/EFER, LAPIC)
//!     │ 3. ICR: INIT (level assert) → chờ 10 ms → SIPI vector 0x08 → chờ 200 µs → SIPI
//!     │ 4. chờ `SMP_ONLINE_MASK` bit 1 (trần 2 s, đo bằng PIT kênh 2) ⇒ TAP 18
//!     │ 5. đặt `SMP_CMD[1] = CMD_REPORT`, chờ `SMP_ALIVE_MASK` bit 1 ⇒ in `# smp: hart=0 alive`
//!     │ 6. TAP 19 (cả hai hart báo alive) → trả về cho `net::run` in marker + halt
//!     ↓
//!   AP (SIPI ⇒ 0x8000, real mode)
//!     ├─ 16-bit: cli/cld, DS=ES=SS=CS, lgdt GDT tạm, CR4.PAE, CR3, EFER.LME|NXE, CR0.PG|PE
//!     ├─ ljmp 0x08:0x8046 ⇒ 64-bit: DS/ES/SS=0x10, rsp = boot.stack_top, lidt boot.idt
//!     ├─ call boot.entry = `kmain_secondary(hart)`
//!     └─ kmain_secondary: kiểm slot stack + LAPIC_ID, bật LAPIC (SVR/TPR/LINT), in
//!        `# smp: hart=1 online`, bật bit online, cấu hình LAPIC timer (vector 32) + `sti`,
//!        rồi vòng `hlt`: thấy `CMD_REPORT` ⇒ in `# smp: hart=1 alive` + bật bit alive.
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
//! # Vì sao không có đoạn 32-bit protected mode
//!
//! Từ real mode có thể đặt thẳng `CR4.PAE`/`CR3`/`EFER.LME` (đều là lệnh CPL0 hợp lệ trong real
//! mode), rồi `CR0.PG|PE` và `ljmp` vào đoạn code 64-bit — CPU vào thẳng IA-32e mode. Nhờ vậy GDT
//! tạm chỉ cần **3 entry** (null + code 64-bit + data), không cần descriptor code 32-bit, và
//! selector `0x08`/`0x10` **trùng** GDT thật của BSP nên bảng IDT dùng chung (`selector 0x08`)
//! vẫn hợp lệ trên hart phụ.
//!
//! # Phạm vi (KT-276)
//!
//! Chỉ **2 hart**. **Không** 4 hart, **không** IPI, **không** scheduler/preemption, **không**
//! TSS per-hart (hart phụ chỉ chạy ring 0 với `IF` bật cho timer đánh thức; `#DF` trên AP dùng
//! chung `IST1` của BSP — nợ đã ghi). **Không** tuyên bố K5' đạt.

use core::arch::global_asm;
use core::fmt::Write;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_x86_64::apic;
use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{EFER_LME, EFER_NXE, MSR_IA32_APIC_BASE};
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::idt;
use vifast_plat_x86_64::smp::{self, HartStacks};
use vifast_plat_x86_64::timebase::PitClock;

use crate::{check, pit2_latch_read, Console};

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

/// Mặt nạ hart đã online (bit `i` = hart `i` đã in `# smp: hart=i online` + LAPIC sẵn sàng).
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
/// Số vòng BSP đã chờ hart phụ online (µs, đo bằng PIT) — bằng chứng "chờ bao lâu".
static SMP_WAIT_ONLINE_US: AtomicU64 = AtomicU64::new(u64::MAX);
/// Số vòng BSP đã chờ hart phụ báo alive (µs).
static SMP_WAIT_ALIVE_US: AtomicU64 = AtomicU64::new(u64::MAX);
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

/// Ghi boot block vào trang trampoline (địa chỉ tuyệt đối `0x8900..`).
///
/// Mọi giá trị lấy từ **thanh ghi của BSP** hoặc hằng của crate arch — không có bản thứ hai của
/// con số nào trong tệp này.
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
// INIT + SIPI
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

/// Chờ bit `hart` trong `mask` bật; trả số µs đã chờ, `None` nếu quá [`smp::ONLINE_TIMEOUT_US`].
fn wait_mask(mask: &AtomicU32, hart: usize, clock: &mut PitClock) -> Option<u64> {
    let start = pit_now(clock);
    loop {
        if mask.load(Ordering::Acquire) & (1u32 << hart) != 0 {
            return Some(pit_now(clock).saturating_sub(start));
        }
        let elapsed = pit_now(clock).saturating_sub(start);
        if elapsed > smp::ONLINE_TIMEOUT_US {
            return None;
        }
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------------------------
// BSP — smp::run
// ---------------------------------------------------------------------------------------------

/// **KT-276** — đánh thức hart 1, in bằng chứng, phát TAP 18/19.
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

    // BSP **luôn** online (nó đang chạy đây): bật bit 0 để `SMP_ONLINE_MASK` là mặt nạ **cả hai**
    // hart, không phải "một hart phụ" — TAP 19 so `alive == online` trên cùng mặt nạ.
    SMP_ONLINE_MASK.fetch_or(1, Ordering::Release);

    // ---- 2. Copy trampoline xuống 0x8000 + ghi boot block.
    let (src, bytes, copy_ok) = copy_trampoline();
    let hart = 1usize;
    // `stack_top` (tham số `kmain`) là đỉnh slot **hart 0**; đỉnh slot hart phụ là biến khác —
    // đặt cùng tên sẽ che mất giá trị của BSP và `bsp_slot` thành `Some(1)` (đo được lần chạy đầu).
    let ap_stack_top = match stacks.stack_top(hart) {
        Some(top) => top,
        None => {
            let _ = writeln!(console, "PANIC: smp: hart {hart} ngoài arena");
            asm::halt(4)
        }
    };
    let lapic_base = crate::lapic_base();
    let bsp_lapic_id = crate::lapic_id();
    let dest = match smp::apic_id_for_hart(hart) {
        Some(id) => id,
        None => {
            let _ = writeln!(console, "PANIC: smp: không có APIC ID cho hart {hart}");
            asm::halt(4)
        }
    };
    publish_boot_block(
        hart,
        ap_stack_top,
        kmain_secondary as *const () as u64,
        lapic_base,
        dest,
    );
    let magic = trampoline_magic();
    let tramp_ok = copy_ok && magic == smp::TRAMPOLINE_MAGIC;
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
        "# smp: boot block pml4=0x{:x} stack=0x{ap_stack_top:x} hart={hart} entry=0x{:x} idt=0x{:x}/0x{:x} cr0=0x{:x} cr4=0x{:x} efer=0x{:x} lapic=0x{lapic_base:x} bsp_id={bsp_lapic_id} dest={dest}",
        asm::read_cr3() & !0xFFF,
        kmain_secondary as *const () as u64,
        idt::read_idtr().0,
        idt::read_idtr().1,
        asm::read_cr0(),
        asm::read_cr4(),
        (EFER_LME | EFER_NXE) as u64,
    );

    // ---- 3. INIT + SIPI + SIPI.
    let vector = match smp::sipi_vector(smp::TRAMPOLINE_BASE) {
        Some(vector) => vector,
        None => {
            let _ = writeln!(console, "PANIC: smp: trampoline không có SIPI vector");
            asm::halt(4)
        }
    };
    // **Đột biến (ii) của KT-276** (`--features bad-trampoline`): SIPI tới **trang bên cạnh**
    // (0x9000) — vùng không có code trampoline. Đo được: hart phụ chạy vào vùng 0 rồi kẹt, **không**
    // online; BSP hết trần 2 s ⇒ TAP 18 ĐỎ, không marker; QEMU **không** tự thoát (treo **có kiểm
    // soát** — runner timeout 20 s giết). Không có `# smp: hart=1 online`.
    let vector = if cfg!(feature = "bad-trampoline") {
        vector.wrapping_add(1)
    } else {
        vector
    };
    // In **trước** khi gửi: hart phụ in `# smp: hart=1 online` ngay khi lên (đo được ~171 µs sau
    // SIPI), nên in sau `send_init_sipi` có thể xen ký tự với dòng của AP trên COM1. Dòng này chỉ
    // chứa giá trị **sắp ghi** (hằng của crate arch); kết quả từng bước in ở dòng `cpu_on`.
    let _ = writeln!(
        console,
        "# smp: icr init dest={dest} low=0x{:04x} high=0x{:08x} sipi vector=0x{vector:02x} low=0x{:04x}",
        smp::icr_init_low(),
        smp::icr_high(dest),
        smp::icr_sipi_low(vector),
    );
    let (init_ok, sipi1_ok, sipi2_ok) = send_init_sipi(dest, vector, &mut clock);

    // ---- 4. Chờ hart phụ online (trần 2 s, đo bằng PIT).
    let waited = wait_mask(&SMP_ONLINE_MASK, hart, &mut clock);
    SMP_WAIT_ONLINE_US.store(waited.unwrap_or(u64::MAX), Ordering::Release);
    let ap_lapic_id = SMP_HART_LAPIC_ID[hart].load(Ordering::Acquire);
    let ap_sp = SMP_HART_SP[hart].load(Ordering::Acquire);
    let ap_slot = stacks.slot_of(ap_sp);
    let bsp_slot = stacks.slot_of_top(bsp_stack_top());
    let online = SMP_ONLINE_MASK.load(Ordering::Acquire);
    let _ = writeln!(
        console,
        "# smp: cpu_on hart={hart} online={} waited_us={} lapic_id={ap_lapic_id} sp=0x{ap_sp:x} slot={ap_slot:?} bsp_slot={bsp_slot:?} init_ok={} sipi_ok={}/{}",
        waited.is_some() as u8,
        waited.unwrap_or(u64::MAX),
        init_ok as u8,
        sipi1_ok as u8,
        sipi2_ok as u8,
    );
    // TAP 18: hart phụ **online** + LAPIC ID đúng + stack hai hart **khác slot** (đột biến iii).
    let ap_online_ok = init_ok
        && sipi1_ok
        && sipi2_ok
        && waited.is_some()
        && online & (1u32 << hart) != 0
        && ap_lapic_id == dest as u64
        && ap_slot == Some(hart)
        && bsp_slot == Some(0)
        && ap_slot != bsp_slot;
    check(console, tap_first, "smp: ap online", ap_online_ok);

    // ---- 5. Lệnh `CMD_REPORT` cho hart phụ; chờ nó **in xong** rồi mới in dòng của BSP.
    //
    // Quy tắc "in trước, đếm sau" (bài học KT-259): hart phụ in `# smp: hart=1 alive` rồi mới bật
    // bit; BSP chờ bit ấy rồi mới in `# smp: hart=0 alive` ⇒ hai dòng không xen ký tự trên COM1.
    SMP_CMD[hart].store(CMD_REPORT, Ordering::Release);
    let waited_alive = wait_mask(&SMP_ALIVE_MASK, hart, &mut clock);
    SMP_WAIT_ALIVE_US.store(waited_alive.unwrap_or(u64::MAX), Ordering::Release);
    let ap_alive = SMP_ALIVE_MASK.load(Ordering::Acquire) & (1u32 << hart) != 0;
    if ap_alive {
        let _ = writeln!(console, "# smp: hart=0 alive");
        SMP_ALIVE_MASK.fetch_or(1, Ordering::Release);
    }
    let alive = SMP_ALIVE_MASK.load(Ordering::Acquire);
    let _ = writeln!(
        console,
        "# smp: online mask=0x{online:x} alive=0x{alive:x} expect=0x{:x} harts={} ap_ticks={} waited_online_us={} waited_alive_us={}",
        smp::SMP_EXPECT_MASK,
        online.count_ones(),
        ap_ticks(),
        wait_online_us(),
        wait_alive_us(),
    );
    // TAP 19: **cả hai** hart đã báo alive (bit 0 do BSP, bit 1 do hart phụ).
    check(
        console,
        tap_first + 1,
        "smp: both harts report",
        ap_alive && alive == smp::SMP_EXPECT_MASK,
    );
}

// ---------------------------------------------------------------------------------------------
// AP — kmain_secondary
// ---------------------------------------------------------------------------------------------

/// **KT-276** — điểm vào Rust của **hart phụ** (trampoline gọi sau khi đã vào long mode).
///
/// `hart` = hart id BSP ghi vào boot block (trampoline đọc rồi truyền qua `rdi`).
///
/// Thứ tự **bắt buộc**:
///
/// 1. kiểm `sp` nằm trong **slot của chính mình** — đột biến (iii) "stack chồng nhau" lộ ra ở đây;
/// 2. bật LAPIC **của hart này** (`IA32_APIC_BASE`, `SVR`, `TPR`, mask `LINT0/1`) và **so**
///    `LAPIC_ID` với giá trị BSP đã ghi — lệch ⇒ PANIC (không tin chỉ số vòng lặp);
/// 3. in `# smp: hart=<id> online` rồi bật bit online;
/// 4. cấu hình LAPIC timer (vector 32, periodic) + `sti` — nguồn **đánh thức** để vòng `hlt`
///    kiểm lệnh; **không** phải nguồn tick của hệ (PIT của BSP vẫn là nguồn tick);
/// 5. vòng `hlt`: thấy `CMD_REPORT` ⇒ in `# smp: hart=<id> alive` + bật bit alive.
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
    // Bật APIC bằng phần mềm + TPR = 0 + mask LINT0/LINT1 (nguồn ngắt duy nhất là timer của AP).
    asm::write_u32(
        base + apic::LAPIC_SVR as u64,
        apic::svr_value(exc::VEC_SPURIOUS),
    );
    asm::write_u32(base + apic::LAPIC_TPR as u64, 0);
    asm::write_u32(base + apic::LAPIC_LVT_LINT0 as u64, apic::LVT_MASKED);
    asm::write_u32(base + apic::LAPIC_LVT_LINT1 as u64, apic::LVT_MASKED);
    let svr = asm::read_u32(base + apic::LAPIC_SVR as u64);
    let tpr = asm::read_u32(base + apic::LAPIC_TPR as u64);

    // ---- 3. Báo online **sau** khi LAPIC sẵn sàng (BSP gửi lệnh ngay khi thấy cờ này).
    let _ = writeln!(console, "# smp: hart={hart} online");
    let _ = writeln!(
        console,
        "# smp: hart={hart} lapic id={id} base=0x{base:x} svr=0x{svr:03x} tpr={tpr} sp=0x{sp:x} slot={hart} stack=0x{:x}-0x{:x}",
        stacks.stack(hart).map(|(b, _)| b).unwrap_or(0),
        stacks.stack(hart).map(|(_, t)| t).unwrap_or(0),
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

    // ---- 5. Vòng lệnh: `hlt` (timer đánh thức) → kiểm `SMP_CMD` → in + bật bit.
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
// Truy vấn trạng thái cho `main.rs` (handler timer + TAP)
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

/// Tăng bộ đếm tick của hart phụ — handler vector 32 gọi khi `LAPIC_ID != BSP`.
pub(crate) fn note_ap_tick() {
    AP_TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Số tick timer hart phụ đã nhận (bằng chứng LAPIC per-hart chạy thật).
pub(crate) fn ap_ticks() -> u64 {
    AP_TICK_COUNT.load(Ordering::Relaxed)
}

/// Số µs BSP đã chờ hart phụ online (`u64::MAX` = chưa chờ).
pub(crate) fn wait_online_us() -> u64 {
    SMP_WAIT_ONLINE_US.load(Ordering::Acquire)
}

/// Số µs BSP đã chờ hart phụ báo alive (`u64::MAX` = chưa chờ).
pub(crate) fn wait_alive_us() -> u64 {
    SMP_WAIT_ALIVE_US.load(Ordering::Acquire)
}
