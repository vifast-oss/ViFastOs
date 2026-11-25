// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho SMP x86_64 — KT-276 (K5' bước 6: 2 hart).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test smp
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-276: mô-đun [`smp`] chưa tồn tại khi tệp này được
//! viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do). Sau khi hiện thực, các
//! khẳng định dưới đây chốt **giá trị tuyệt đối** mà asm/ảnh `vifast-kbin-x86_64` dùng — nếu ai
//! đổi một bit của ICR, một offset của trampoline hay một hằng bố cục arena, test đỏ **trước**
//! khi có ảnh boot.
//!
//! Nguồn giá trị:
//!
//! - **Intel SDM Vol 3A** §10.6.1 "Interrupt Command Register (ICR)" — Hình 10-27: vector bit
//!   0..7, delivery mode bit 8..10, destination mode bit 11, delivery status bit 12, level bit 14,
//!   trigger mode bit 15, destination shorthand bit 18..19, destination field bit 24..31 của
//!   thanh ghi **cao** (`ICR[63:56]`).
//! - **Intel SDM Vol 3A** Bảng 10-1 "Delivery Mode": `101` = INIT, `110` = Start-Up (SIPI).
//! - **Intel SDM Vol 3A** §10.12.1 "APIC Base Address Register" — `IA32_APIC_BASE` mặc định
//!   `0xFEE0_0000`.
//! - **Intel SDM Vol 3A** §9.1.4 "First Instruction Executed" / §10.12.10 "SIPI": CPU phụ bắt đầu
//!   ở **real mode** với `CS:IP = (vector << 8):0`, tức địa chỉ **vật lý** `vector << 12`; vector
//!   là **byte** trong `ICR[7:0]` ⇒ địa chỉ phải `< 1 MiB` và căn 4 KiB.
//! - **Intel SDM Vol 3A** §10.12.9 "INIT": INIT đặt CPU đích vào trạng thái chờ SIPI; phải chờ
//!   **10 ms** sau INIT và **200 µs** giữa hai SIPI (thời gian do đặc tả, không phải số đo).
//! - **Intel SDM Vol 3A** §7.2.1 / Hình 7-11: TSS 64-bit 104 byte (dùng lại hằng của `tss`).
//!
//! # Vì sao bố cục arena là **hợp đồng**
//!
//! `linker.ld` cấp vùng NOLOAD `__smp_stacks_start..__smp_stacks_end` với **đúng** bố cục
//! `guard 4 KiB | stack 64 KiB` × 8 hart. Ảnh kiểm `__smp_stacks_end - __smp_stacks_start ==
//! HART_AREA_BYTES` lúc chạy; test này chốt các con số tuyệt đối để một thay đổi hằng số ở crate
//! không thể lặng lẽ lệch linker.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà asm của
// ảnh boot nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::exc::{
    name as exc_name, stub_slot, STUB_COUNT, STUB_IPI, STUB_UNKNOWN, VEC_IPI, VEC_SPURIOUS,
    VEC_SYSCALL, VEC_TIMER, VEC_UNKNOWN, VEC_UNKNOWN_PROBE,
};
use vifast_plat_x86_64::gdt::{base as gdt_base, tss_base_of, tss_descriptor, GDT_BYTES};
use vifast_plat_x86_64::smp::{
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
use vifast_plat_x86_64::tss::{
    base as tss_base, check_distinct as tss_check_distinct, TssError, TSS_LIMIT,
};

// ---------------------------------------------------------------------------------------------
// Bố cục arena stack per-hart
// ---------------------------------------------------------------------------------------------

#[test]
fn arena_layout_is_absolute() {
    // 4 KiB guard + 64 KiB stack = 68 KiB = 0x1_1000 mỗi hart; 8 hart = 544 KiB = 0x8_8000.
    assert_eq!(HART_GUARD_BYTES, 0x1000);
    assert_eq!(HART_STACK_BYTES, 0x1_0000);
    assert_eq!(HART_STRIDE, 0x1_1000);
    assert_eq!(MAX_HARTS, 8);
    assert_eq!(HART_AREA_BYTES, 0x8_8000);
    // Mọi record bắt đầu ở biên trang ⇒ guard cưỡng chế được ở granularity 4 KiB.
    assert_eq!(HART_STRIDE % 0x1000, 0);
    assert_eq!(HART_AREA_BYTES % 0x1000, 0);
}

#[test]
fn hart_offsets_are_absolute() {
    assert!(valid_hart(0));
    assert!(valid_hart(MAX_HARTS - 1));
    assert!(!valid_hart(MAX_HARTS));
    // Hart 0: guard [0, 0x1000), stack [0x1000, 0x11000), đỉnh 0x11000.
    assert_eq!(guard_offset(0), Some(0));
    assert_eq!(stack_bottom_offset(0), Some(0x1000));
    assert_eq!(stack_top_offset(0), Some(0x1_1000));
    // Hart 1: record thứ hai, guard [0x11000, 0x12000), stack [0x12000, 0x22000).
    assert_eq!(guard_offset(1), Some(0x1_1000));
    assert_eq!(stack_bottom_offset(1), Some(0x1_2000));
    assert_eq!(stack_top_offset(1), Some(0x2_2000));
    // Hart cuối: đỉnh stack == cuối arena.
    assert_eq!(stack_top_offset(MAX_HARTS - 1), Some(HART_AREA_BYTES));
    // Hart ngoài dải: `None`, không đoán.
    assert_eq!(guard_offset(MAX_HARTS), None);
    assert_eq!(stack_bottom_offset(MAX_HARTS), None);
    assert_eq!(stack_top_offset(MAX_HARTS), None);
}

#[test]
fn layout_accepts_the_real_arena() {
    let base = 0x20_0000;
    assert_eq!(
        check_layout(
            base,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            MAX_HARTS
        ),
        Ok(())
    );
    // 2 hart cũng hợp lệ (ảnh KT-276 chỉ đánh thức 2).
    assert_eq!(
        check_layout(
            base,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            SMP_HARTS
        ),
        Ok(())
    );
}

#[test]
fn layout_rejects_overlap_without_guard() {
    // Đột biến (iii) của KT-276: bỏ guard khỏi stride ⇒ record hart 1 bắt đầu **trước** khi
    // record hart 0 kết thúc ⇒ `Overlap { a: 0, b: 1 }`, không phải "vừa khít".
    assert_eq!(
        check_layout(
            0x20_0000,
            HART_STACK_BYTES,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            2
        ),
        Err(StackError::Overlap { a: 0, b: 1 })
    );
    // Stride nhỏ hơn record nhưng chưa tới mức đè nhau ở hart 0/1 (count = 1) ⇒ StrideTooSmall.
    assert_eq!(
        check_layout(
            0x20_0000,
            HART_STACK_BYTES,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            1
        ),
        Err(StackError::StrideTooSmall)
    );
}

#[test]
fn layout_rejects_bad_parameters() {
    let base = 0x20_0000;
    assert_eq!(
        check_layout(base, HART_STRIDE, HART_GUARD_BYTES, HART_STACK_BYTES, 0),
        Err(StackError::Empty)
    );
    assert_eq!(
        check_layout(
            base,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            MAX_HARTS + 1
        ),
        Err(StackError::OutOfRange)
    );
    // Base lệch 4 KiB ⇒ guard không cưỡng chế được ở granularity trang.
    assert_eq!(
        check_layout(base + 8, HART_STRIDE, HART_GUARD_BYTES, HART_STACK_BYTES, 2),
        Err(StackError::Unaligned)
    );
    // Guard hoặc stack bằng 0 ⇒ record rỗng, không phải "không cần guard".
    assert_eq!(
        check_layout(base, HART_STRIDE, 0, HART_STACK_BYTES, 2),
        Err(StackError::StrideTooSmall)
    );
    assert_eq!(
        check_layout(base, HART_STRIDE, HART_GUARD_BYTES, 0, 2),
        Err(StackError::StrideTooSmall)
    );
}

#[test]
fn hart_stacks_map_addresses_to_owners() {
    let base = 0x20_0000;
    let stacks = HartStacks::new(base).expect("base căn 4 KiB");
    assert_eq!(stacks.base(), base);
    assert_eq!(stacks.guard(0), Some((base, base + 0x1000)));
    assert_eq!(stacks.stack(0), Some((base + 0x1000, base + 0x1_1000)));
    assert_eq!(stacks.stack_top(0), Some(base + 0x1_1000));
    assert_eq!(stacks.stack(1), Some((base + 0x1_2000, base + 0x2_2000)));
    assert_eq!(stacks.stack_top(1), Some(base + 0x2_2000));
    assert_eq!(stacks.stack_top(MAX_HARTS), None);
    // `slot_of`: địa chỉ trong stack của hart nào thuộc hart ấy; guard/ngoài arena ⇒ `None`.
    assert_eq!(stacks.slot_of(base + 0x1000), Some(0));
    assert_eq!(stacks.slot_of(base + 0x1_0FFF), Some(0));
    assert_eq!(stacks.slot_of(base + 0x1_1000), None); // guard của hart 1
    assert_eq!(stacks.slot_of(base + 0x1_2000), Some(1));
    assert_eq!(stacks.slot_of(base + 0x2_1FFF), Some(1));
    assert_eq!(stacks.slot_of(base + 0x2_2000), None); // ngoài arena
    assert_eq!(stacks.slot_of(base - 1), None);
    // `slot_of_top`: đỉnh stack là giá trị `sp` ban đầu — phải quy về **đúng** hart.
    assert_eq!(stacks.slot_of_top(base + 0x1_1000), Some(0));
    assert_eq!(stacks.slot_of_top(base + 0x2_2000), Some(1));
    assert_eq!(stacks.slot_of_top(base + 0x1_2000), None); // đáy stack, không phải đỉnh
    assert_eq!(stacks.check(), Ok(()));
    // Base lệch trang ⇒ không dựng được arena (không có trạng thái nửa hợp lệ).
    assert!(HartStacks::new(base + 4).is_none());
}

// ---------------------------------------------------------------------------------------------
// ICR — INIT / SIPI
// ---------------------------------------------------------------------------------------------

#[test]
fn icr_register_offsets_are_absolute() {
    // SDM Hình 10-27: ICR nằm ở 0x300 (thấp) và 0x310 (cao) trong trang MMIO LAPIC.
    assert_eq!(LAPIC_ICR_LOW, 0x300);
    assert_eq!(LAPIC_ICR_HIGH, 0x310);
    assert_eq!(ICR_VECTOR_MASK, 0xFF);
    assert_eq!(ICR_DELIVERY_MODE_SHIFT, 8);
    assert_eq!(ICR_DELIVERY_MODE_MASK, 0x700);
    assert_eq!(ICR_DELIVERY_PENDING, 0x1000);
    assert_eq!(ICR_LEVEL_ASSERT, 0x4000);
    assert_eq!(ICR_LEVEL_TRIGGER, 0x8000);
    assert_eq!(ICR_DEST_SHIFT, 24);
    // Bảng 10-1: INIT = 101, Start-Up = 110.
    assert_eq!(ICR_DM_INIT, 0b101);
    assert_eq!(ICR_DM_STARTUP, 0b110);
}

#[test]
fn icr_init_is_level_asserted() {
    // INIT: delivery mode 101 << 8 = 0x500, level assert 0x4000, level trigger 0x8000 ⇒ 0xC500.
    // Vector **không** có nghĩa với INIT (đặc tả bỏ qua) ⇒ 0.
    assert_eq!(icr_init_low(), 0xC500);
    assert_eq!(icr_low(ICR_DM_INIT, 0, true, true), 0xC500);
    // Thiếu level assert/trigger là **sai** — test chốt cả hai bit.
    assert_ne!(icr_low(ICR_DM_INIT, 0, false, true), icr_init_low());
    assert_ne!(icr_low(ICR_DM_INIT, 0, true, false), icr_init_low());
}

#[test]
fn icr_sipi_carries_the_vector() {
    // SIPI: delivery mode 110 << 8 = 0x600 | vector; **không** assert, **không** level trigger.
    assert_eq!(icr_sipi_low(0x08), 0x608);
    assert_eq!(icr_sipi_low(0x00), 0x600);
    assert_eq!(icr_sipi_low(0xFF), 0x6FF);
    assert_eq!(icr_low(ICR_DM_STARTUP, 0x08, false, false), 0x608);
    // Vector là `u8` ⇒ không thể tràn sang delivery mode; bit cao của vector vẫn nằm trong 8 bit.
    assert_eq!(icr_sipi_low(0x80), 0x680);
}

#[test]
fn icr_high_carries_the_destination() {
    // Destination field = ICR[63:56] ⇒ thanh ghi cao bit 24..31.
    assert_eq!(icr_high(0), 0);
    assert_eq!(icr_high(1), 0x0100_0000);
    assert_eq!(icr_high(0xFF), 0xFF00_0000);
    // Delivery status là bit 12 của thanh ghi **thấp**.
    assert!(icr_delivery_pending(0x1000));
    assert!(!icr_delivery_pending(0x0FFF));
    assert!(!icr_delivery_pending(icr_sipi_low(0x08)));
}

// ---------------------------------------------------------------------------------------------
// SIPI vector + topology APIC ID
// ---------------------------------------------------------------------------------------------

#[test]
fn sipi_vector_is_address_shifted_by_12() {
    // 0x8000 >> 12 = 0x08 — địa chỉ trampoline của KT-276.
    assert_eq!(sipi_vector(0x8000), Some(0x08));
    assert_eq!(sipi_vector(0x9000), Some(0x09));
    assert_eq!(sipi_vector(0x0), Some(0x00));
    // Địa chỉ cuối cùng còn dùng được: 0xF_F000 ⇒ vector 0xFF.
    assert_eq!(sipi_vector(0xF_F000), Some(0xFF));
    // ≥ 1 MiB: không biểu diễn được bằng vector 8 bit ⇒ `None` (fail-closed).
    assert_eq!(sipi_vector(SIPI_MAX_ADDR), None);
    assert_eq!(sipi_vector(0x10_0000), None);
    // Không căn 4 KiB: vector sẽ trỏ vào **giữa** trang ⇒ `None`.
    assert_eq!(sipi_vector(0x8010), None);
    assert_eq!(sipi_vector(0x8FFF), None);
}

#[test]
fn apic_id_is_the_hart_index_on_qemu_flat_topology() {
    // QEMU `-smp 2` đặt APIC ID = chỉ số CPU (đo được: BSP id=0, AP id=1). Ảnh **kiểm lại**
    // trên AP (`LAPIC_ID` đọc từ MMIO) và PANIC nếu lệch — không tin con số này mù.
    assert_eq!(apic_id_for_hart(0), Some(0));
    assert_eq!(apic_id_for_hart(1), Some(1));
    assert_eq!(apic_id_for_hart(MAX_HARTS - 1), Some((MAX_HARTS - 1) as u8));
    assert_eq!(apic_id_for_hart(MAX_HARTS), None);
}

// ---------------------------------------------------------------------------------------------
// Trampoline — bố cục trang 0x8000
// ---------------------------------------------------------------------------------------------

#[test]
fn trampoline_layout_is_absolute() {
    // Trang trampoline nằm ở 0x8000 (dưới 1 MiB, căn 4 KiB) — SIPI vector 0x08.
    assert_eq!(TRAMPOLINE_BASE, 0x8000);
    assert_eq!(TRAMPOLINE_BYTES, 0x1000);
    assert_eq!(sipi_vector(TRAMPOLINE_BASE), Some(0x08));
    // GDT tạm 3 entry ở offset 0x800, pseudo-descriptor 6 byte ở 0x880, boot block ở 0x900.
    assert_eq!(TRAMPOLINE_GDT_OFFSET, 0x800);
    assert_eq!(TRAMPOLINE_GDTR_OFFSET, 0x880);
    assert_eq!(TRAMPOLINE_BOOT_OFFSET, 0x900);
    // GDT 3 entry = 24 byte ⇒ limit 23; GDTR nằm **sau** GDT, không đè.
    assert!(TRAMPOLINE_GDTR_OFFSET >= TRAMPOLINE_GDT_OFFSET + 24);
    // Boot block nằm sau GDTR (6 byte) và trước cuối trang.
    assert!(TRAMPOLINE_BOOT_OFFSET >= TRAMPOLINE_GDTR_OFFSET + 6);
    assert!(TRAMPOLINE_BOOT_OFFSET < TRAMPOLINE_BYTES);
    // Địa chỉ tuyệt đối = base + offset.
    assert_eq!(trampoline_addr(TRAMPOLINE_GDT_OFFSET), 0x8800);
    assert_eq!(trampoline_addr(TRAMPOLINE_GDTR_OFFSET), 0x8880);
    assert_eq!(trampoline_addr(TRAMPOLINE_BOOT_OFFSET), 0x8900);
}

#[test]
fn boot_block_fields_are_contiguous_and_aligned() {
    // Thứ tự trường là **hợp đồng** với asm của trampoline: mỗi trường 8 byte, trừ cặp
    // `IDT limit (u16) | IDT base (u64)` phải **liền nhau** để `lidt [mem]` đọc đúng 10 byte.
    assert_eq!(BOOT_PML4, 0x900);
    assert_eq!(BOOT_STACK_TOP, 0x908);
    assert_eq!(BOOT_HART, 0x910);
    assert_eq!(BOOT_ENTRY, 0x918);
    assert_eq!(BOOT_IDT_LIMIT, 0x920);
    assert_eq!(BOOT_IDT_BASE, 0x922); // ngay sau limit 2 byte ⇒ `lidt` đọc 10 byte liền
    assert_eq!(BOOT_CR0, 0x930);
    assert_eq!(BOOT_CR4, 0x938);
    assert_eq!(BOOT_EFER, 0x940);
    assert_eq!(BOOT_LAPIC_BASE, 0x948);
    assert_eq!(BOOT_LAPIC_ID, 0x950);
    assert_eq!(BOOT_MAGIC, 0x958);
    // Mọi trường 8 byte căn 8 (trừ cặp IDT cố ý lệch 2).
    for off in [
        BOOT_PML4,
        BOOT_STACK_TOP,
        BOOT_HART,
        BOOT_ENTRY,
        BOOT_CR0,
        BOOT_CR4,
        BOOT_EFER,
        BOOT_LAPIC_BASE,
        BOOT_LAPIC_ID,
        BOOT_MAGIC,
    ] {
        assert_eq!(off % 8, 0, "offset 0x{off:x} phải căn 8");
    }
    // Boot block nằm trọn trong trang trampoline.
    assert!(BOOT_MAGIC + 8 <= TRAMPOLINE_BYTES);
    // Magic khác 0 và khác mọi giá trị "trông như dữ liệu rác".
    assert_eq!(TRAMPOLINE_MAGIC, 0x534D_5032);
    assert_ne!(TRAMPOLINE_MAGIC, 0);
}

// ---------------------------------------------------------------------------------------------
// Thời gian + timer của AP
// ---------------------------------------------------------------------------------------------

#[test]
fn smp_timing_constants_are_spec_values() {
    // SDM §10.12.9/§10.12.10: 10 ms sau INIT, 200 µs giữa hai SIPI. Trần chờ online là **ngân
    // sách của ảnh** (2 s), không phải hằng đặc tả — đo bằng PIT kênh 2, không phải số vòng.
    assert_eq!(INIT_WAIT_US, 10_000);
    assert_eq!(SIPI_WAIT_US, 200);
    assert_eq!(ONLINE_TIMEOUT_US, 2_000_000);
    assert!(ONLINE_TIMEOUT_US > INIT_WAIT_US + 2 * SIPI_WAIT_US);
    // Timer của AP: chia 1, count 100_000 (100 µs danh định theo mô hình QEMU 1 count/ns).
    assert_eq!(AP_TIMER_DIVIDE, 1);
    assert_eq!(AP_TIMER_INITIAL_COUNT, 100_000);
    assert!(AP_TIMER_INITIAL_COUNT > 0);
}

#[test]
fn smp_hart_count_matches_the_runner() {
    // **KT-278**: runner `run-x86_64.sh` chạy `-smp 4`; ảnh đánh thức **ba** hart phụ (1, 2, 3).
    assert_eq!(SMP_HARTS, 4);
    assert_eq!(SMP_EXPECT_MASK, 0xF);
    assert!(SMP_HARTS <= MAX_HARTS);
    // Mặt nạ kỳ vọng phủ **đúng** `SMP_HARTS` bit thấp — không thừa, không thiếu.
    assert_eq!(SMP_EXPECT_MASK.count_ones() as usize, SMP_HARTS);
}

// ---------------------------------------------------------------------------------------------
// KT-278 — IPI: vector riêng + ICR fixed-delivery
// ---------------------------------------------------------------------------------------------

#[test]
fn ipi_vector_is_private_and_has_its_own_stub() {
    // Vector IPI là **0x30** — nằm ngoài dải exception 0..31 và khác timer (32), syscall (0x80),
    // spurious (255), sentinel `isr_unknown` (254).
    assert_eq!(VEC_IPI, 0x30);
    assert!(VEC_IPI > 31);
    assert_ne!(VEC_IPI, VEC_TIMER);
    assert_ne!(VEC_IPI, VEC_SYSCALL);
    assert_ne!(VEC_IPI, VEC_SPURIOUS);
    assert_ne!(VEC_IPI as u64, VEC_UNKNOWN);
    // Có stub **riêng** (không rơi về `isr_unknown`), và slot nằm trong `STUB_COUNT`.
    assert_eq!(stub_slot(VEC_IPI), STUB_IPI);
    assert!(STUB_IPI < STUB_COUNT);
    // **KT-406** — tổng slot tăng 19 → 20 (thêm `STUB_SERIAL` cho COM1 RX, vector `0x24`).
    // **KT-413** (nợ `N-3`) — 20 → **21** (thêm `STUB_WATCHDOG` cho vector `0x32`).
    assert_eq!(STUB_COUNT, 21);
    // Tên trong log.
    assert_eq!(exc_name(VEC_IPI as u64), "ipi");
    // Vector của probe `int` (ca `isr_unknown`) **phải khác** vector IPI — nếu trùng, TAP 7 sẽ
    // chạy vào handler IPI thay vì `isr_unknown` và mất bằng chứng "vector lạ vẫn thấy được".
    assert_ne!(VEC_UNKNOWN_PROBE, VEC_IPI);
    assert_eq!(stub_slot(VEC_UNKNOWN_PROBE), STUB_UNKNOWN);
    assert_eq!(exc_name(VEC_UNKNOWN_PROBE as u64), "other");
}

#[test]
fn icr_fixed_delivery_encoding_is_sdm_literal() {
    // SDM Vol 3A Hình 10-27: vector bit 0..7, delivery mode bit 8..10, level bit 14, trigger bit 15.
    // Fixed = `000`, level = **assert** (1), trigger = **edge** (0) ⇒ `0x4000 | vector`.
    assert_eq!(icr_fixed_low(VEC_IPI), 0x4030);
    assert_eq!(icr_fixed_low(0x30) & ICR_VECTOR_MASK, 0x30);
    assert_eq!(
        (icr_fixed_low(0x30) & ICR_DELIVERY_MODE_MASK) >> ICR_DELIVERY_MODE_SHIFT,
        ICR_DM_FIXED
    );
    assert_eq!(ICR_DM_FIXED, 0);
    assert_ne!(icr_fixed_low(0x30) & ICR_LEVEL_ASSERT, 0);
    assert_eq!(icr_fixed_low(0x30) & ICR_LEVEL_TRIGGER, 0);
    // Không phải INIT/SIPI: hai lệnh kia có delivery mode khác.
    assert_ne!(icr_fixed_low(0x30), icr_init_low());
    assert_ne!(icr_fixed_low(0x30), icr_sipi_low(0x08));
    // Đích nằm ở thanh ghi **cao** (bit 24..31) — gửi tới hart 1 ⇒ `0x0100_0000`.
    assert_eq!(icr_high(1), 0x0100_0000);
    assert_eq!(icr_high(3), 0x0300_0000);
    // Lệnh fixed-delivery **không** đặt bit delivery-pending khi mới dựng.
    assert!(!icr_delivery_pending(icr_fixed_low(VEC_IPI)));
}

#[test]
fn ipi_timeout_is_bounded() {
    // Trần chờ IPI là **ngân sách của ảnh** (100 ms), đo bằng PIT — không phải số vòng, và phải
    // **hữu hạn** để đột biến (ii) "IPI tới hart không tồn tại" **không treo**.
    assert_eq!(IPI_TIMEOUT_US, 100_000);
    assert!(IPI_TIMEOUT_US > 0);
    assert!(IPI_TIMEOUT_US < ONLINE_TIMEOUT_US);
}

// ---------------------------------------------------------------------------------------------
// KT-278 — TSS per-hart: địa chỉ rời nhau
// ---------------------------------------------------------------------------------------------

#[test]
fn tss_per_hart_addresses_are_distinct() {
    // Mỗi hart một vùng TSS 104 byte; 4 hart của ảnh ⇒ bốn địa chỉ **rời nhau**.
    let bases = [tss_base(0), tss_base(1), tss_base(2), tss_base(3)];
    assert_eq!(tss_check_distinct(&bases), Ok(()));
    // Đột biến (iii): trùng địa chỉ ⇒ `Duplicate`; chồng lấn ⇒ `Overlap`.
    assert_eq!(
        tss_check_distinct(&[bases[0], bases[0]]),
        Err(TssError::Duplicate { a: 0, b: 1 })
    );
    assert_eq!(
        tss_check_distinct(&[bases[0], bases[0] + 4]),
        Err(TssError::Overlap { a: 0, b: 1 })
    );
    // Một hart: luôn hợp lệ (không có cặp nào để so).
    assert_eq!(tss_check_distinct(&bases[..1]), Ok(()));
    // Rỗng: hợp lệ (không có gì để kiểm) — caller phải tự đòi `SMP_HARTS` phần tử.
    assert_eq!(tss_check_distinct(&[]), Ok(()));
}

#[test]
fn gdt_per_hart_is_a_separate_table() {
    // Mỗi hart một bảng GDT riêng (7 entry) để descriptor TSS trỏ **TSS của chính nó**.
    assert_ne!(gdt_base(0), gdt_base(1));
    assert_ne!(gdt_base(1), gdt_base(2));
    assert_eq!(gdt_base(1) - gdt_base(0), GDT_BYTES as u64);
    // Giải mã base từ descriptor TSS 16 byte — khứ hồi với `tss_descriptor`.
    let want = 0x0000_0000_1234_5000u64;
    let [low, high] = tss_descriptor(want, TSS_LIMIT as u32);
    assert_eq!(tss_base_of(low, high), want);
    // Base > 4 GiB vẫn giải mã đúng (trường `base[63:32]` ở `u64` thứ hai).
    let high_base = 0x0000_0001_2345_6000u64;
    let [low2, high2] = tss_descriptor(high_base, TSS_LIMIT as u32);
    assert_eq!(tss_base_of(low2, high2), high_base);
    // Limit vẫn là byte cuối (103) — không bị base làm nhiễu.
    assert_eq!(TSS_LIMIT, 103);
}
