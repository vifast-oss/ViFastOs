// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho SMP aarch64 — KT-259 (K4' bước 3, B3.9: hai hart).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng **viết tay từ đặc tả**, không lấy hằng của crate để so:
//!
//! - **PSCI** (ARM DEN 0022): `CPU_ON` = `0xC400_0003` (function id 64-bit), trả `0` = thành công,
//!   `-2` = `INVALID_PARAMETERS`, `-4` = `ALREADY_ON`;
//! - **`MPIDR_EL1`** (DDI 0487): `Aff0[7:0]`, `Aff1[15:8]`, `Aff2[23:16]`, `Aff3[39:32]`; QEMU
//!   `virt` đặt chỉ số CPU vào `Aff0` (DTB `cpu@0`/`cpu@1` có `reg = <0>`/`<1>`), nên hart id
//!   tuyến tính = `Aff0` khi ba trường còn lại bằng 0;
//! - **arena stack per-hart**: mỗi hart một record `guard 4 KiB | stack 64 KiB` = stride
//!   `0x1_1000`; `MAX_HARTS = 8` (D2-195 Q5) ⇒ vùng `0x8_8000` (544 KiB) NOLOAD.
//!
//! Đây là ca kiểm **đỏ trước** của KT-259 (log: `v2/agent/JOURNAL/kt259-red.log`).
//! **Đột biến bắt buộc (iii)** — "stack per-hart chồng nhau" — nằm ở
//! [`stack_overlap_without_guard_is_rejected`]: bỏ guard khỏi stride thì record của hai hart
//! **đè nhau** và `check_layout` phải trả `Overlap` (log: `kt259-mutation.log`).

use vifast_plat_aarch64::smp::{
    check_layout, cpu_on_ok, hart_index, mpidr_affinity, mpidr_for_hart, stack_bottom_offset,
    stack_top_offset, valid_hart, HartStacks, MmuBootRegs, StackError, HART_AREA_BYTES,
    HART_GUARD_BYTES, HART_STACK_BYTES, HART_STRIDE, MAX_HARTS, PSCI_ALREADY_ON, PSCI_CPU_ON,
    PSCI_INVALID_PARAMS, PSCI_SUCCESS,
};

/// PA gốc của arena trong test — căn 4 KiB (đúng yêu cầu `TTBR`/linker).
const ARENA_BASE: u64 = 0x4000_0000;

/// `CPU_ON` là function id **64-bit** `0xC400_0003` (PSCI DEN 0022 §5.7) — không phải `0x8400_0003`
/// (dải 32-bit chỉ dành cho `CPU_SUSPEND`/`SYSTEM_OFF`/`SYSTEM_RESET`).
#[test]
fn psci_cpu_on_function_id() {
    assert_eq!(PSCI_CPU_ON, 0xC400_0003);
    assert_ne!(
        PSCI_CPU_ON, 0x8400_0003,
        "CPU_ON phải ở dải function id 64-bit"
    );
}

/// Mã trả của PSCI: chỉ `0` là thành công; `-2`/`-4` là lỗi **phải** làm boot đỏ, không im lặng.
#[test]
fn psci_return_codes() {
    assert_eq!(PSCI_SUCCESS, 0);
    assert_eq!(PSCI_INVALID_PARAMS, -2);
    assert_eq!(PSCI_ALREADY_ON, -4);
    assert!(cpu_on_ok(PSCI_SUCCESS));
    assert!(!cpu_on_ok(PSCI_INVALID_PARAMS));
    assert!(!cpu_on_ok(PSCI_ALREADY_ON));
    assert!(!cpu_on_ok(-1), "mã lạ cũng phải là đỏ (fail-closed)");
}

/// `MPIDR_EL1` của QEMU `virt`: `Aff0` = chỉ số CPU, `Aff1..3` = 0, bit 31 (`MT`) = 1.
#[test]
fn mpidr_decode() {
    assert_eq!(mpidr_affinity(0x8000_0000), (0, 0, 0, 0));
    assert_eq!(mpidr_affinity(0x8000_0001), (1, 0, 0, 0));
    assert_eq!(mpidr_affinity(0x8000_0102), (2, 1, 0, 0));
    assert_eq!(mpidr_affinity(0x8000_0000_0003_0405), (5, 4, 3, 0));
    assert_eq!(mpidr_affinity(0x0000_0000_0000_0007), (7, 0, 0, 0));
}

/// Hart id tuyến tính = `Aff0` **chỉ khi** `Aff1..3` bằng 0 và `Aff0 < MAX_HARTS`; ngoài ra `None`
/// (không đoán — hart lạ phải bị từ chối, không lấy `Aff0` của nó làm chỉ số arena).
#[test]
fn hart_index_from_mpidr() {
    assert_eq!(hart_index(0x8000_0000), Some(0));
    assert_eq!(hart_index(0x8000_0001), Some(1));
    assert_eq!(hart_index(0x8000_0007), Some(7));
    assert_eq!(hart_index(0x8000_0008), None, "Aff0 = 8 vượt MAX_HARTS");
    assert_eq!(
        hart_index(0x8000_0100),
        None,
        "Aff1 khác 0 — không phải topology phẳng"
    );
    assert_eq!(hart_index(0x8000_0000_0001_0000), None, "Aff2 khác 0");
}

/// `mpidr_for_hart` là **nghịch đảo** của `hart_index` trên toàn dải hợp lệ.
#[test]
fn mpidr_for_hart_roundtrip() {
    for hart in 0..MAX_HARTS {
        let mpidr = mpidr_for_hart(hart).expect("hart hợp lệ phải có MPIDR");
        assert_eq!(hart_index(mpidr), Some(hart), "hart {hart}");
    }
    assert_eq!(mpidr_for_hart(MAX_HARTS), None);
}

/// Hình học arena: `guard 4 KiB | stack 64 KiB`, stride `0x1_1000`, cả vùng `0x8_8000`.
#[test]
fn stack_geometry() {
    assert_eq!(MAX_HARTS, 8, "D2-195 Q5: MAX_HARTS giữ 8");
    assert_eq!(HART_GUARD_BYTES, 0x1000);
    assert_eq!(HART_STACK_BYTES, 0x1_0000);
    assert_eq!(HART_STRIDE, 0x1_1000);
    assert_eq!(HART_AREA_BYTES, 0x8_8000);
    assert_eq!(HART_AREA_BYTES, HART_STRIDE * MAX_HARTS as u64);
}

/// Offset từng phần của record: guard ở **đầu** record, stack **ngay trên** guard của chính hart
/// đó (tràn stack đi xuống chạm guard của mình, không phải guard của hart khác).
#[test]
fn stack_offsets_per_hart() {
    for hart in 0..MAX_HARTS {
        let base = hart as u64 * HART_STRIDE;
        assert_eq!(
            stack_bottom_offset(hart),
            Some(base + HART_GUARD_BYTES),
            "hart {hart}"
        );
        assert_eq!(
            stack_top_offset(hart),
            Some(base + HART_STRIDE),
            "hart {hart}"
        );
        assert_eq!(
            stack_top_offset(hart).unwrap() - stack_bottom_offset(hart).unwrap(),
            HART_STACK_BYTES
        );
    }
    assert_eq!(stack_bottom_offset(MAX_HARTS), None);
    assert_eq!(stack_top_offset(MAX_HARTS), None);
    assert!(!valid_hart(MAX_HARTS));
}

/// Arena hợp lệ: 8 record **rời nhau**, mỗi stack nằm trọn trong record của nó.
#[test]
fn stacks_are_disjoint() {
    let stacks = HartStacks::new(ARENA_BASE).expect("arena căn 4 KiB");
    assert_eq!(stacks.check(), Ok(()));
    for a in 0..MAX_HARTS {
        let (a_bottom, a_top) = stacks.stack(a).expect("hart hợp lệ");
        assert_eq!(a_top - a_bottom, HART_STACK_BYTES);
        for b in (a + 1)..MAX_HARTS {
            let (b_bottom, b_top) = stacks.stack(b).expect("hart hợp lệ");
            assert!(
                a_top <= b_bottom || b_top <= a_bottom,
                "stack hart {a} và {b} chồng nhau"
            );
        }
    }
}

/// `slot_of` ánh xạ **địa chỉ stack** về đúng hart; địa chỉ trong guard hoặc ngoài arena ⇒ `None`.
#[test]
fn stack_slot_of_address() {
    let stacks = HartStacks::new(ARENA_BASE).expect("arena căn 4 KiB");
    for hart in 0..MAX_HARTS {
        let top = stacks.stack_top(hart).expect("hart hợp lệ");
        assert_eq!(stacks.slot_of(top - 8), Some(hart), "hart {hart}");
        assert_eq!(
            stacks.slot_of(top - HART_STACK_BYTES),
            Some(hart),
            "đáy stack hart {hart}"
        );
        // Guard nằm **dưới** đáy stack của chính hart đó.
        assert_eq!(
            stacks.slot_of(top - HART_STACK_BYTES - 8),
            None,
            "guard hart {hart}"
        );
    }
    assert_eq!(stacks.slot_of(ARENA_BASE + HART_AREA_BYTES), None);
    assert_eq!(stacks.slot_of(ARENA_BASE - 8), None);
}

/// **Đột biến (iii)** — bỏ guard khỏi stride (`stride = stack`) thì record của hart `i` và `i+1`
/// **đè nhau** 4 KiB; `check_layout` phải trả `Overlap`, không được im lặng.
#[test]
fn stack_overlap_without_guard_is_rejected() {
    let bad = check_layout(
        ARENA_BASE,
        HART_STACK_BYTES,
        HART_GUARD_BYTES,
        HART_STACK_BYTES,
        2,
    );
    match bad {
        Err(StackError::Overlap { a, b }) => {
            assert_eq!((a, b), (0, 1), "hai record đè nhau phải là hart 0 và 1");
        }
        other => panic!("stride thiếu guard phải bị bắt là Overlap, nhận {other:?}"),
    }
    // Đối chứng dương: cùng hàm với stride **đúng** thì xanh.
    assert_eq!(
        check_layout(
            ARENA_BASE,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            MAX_HARTS
        ),
        Ok(())
    );
}

/// Arena không căn 4 KiB ⇒ `None` (guard chỉ cưỡng chế được khi mọi record căn trang).
#[test]
fn stack_arena_must_be_page_aligned() {
    assert!(HartStacks::new(ARENA_BASE + 0x800).is_none());
    assert!(HartStacks::new(ARENA_BASE).is_some());
}

/// `check_layout` từ chối stride **nhỏ hơn** `guard + stack` (record không thể vừa) và số hart 0.
#[test]
fn stack_layout_rejects_impossible_geometry() {
    assert_eq!(
        check_layout(
            ARENA_BASE,
            HART_GUARD_BYTES,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            1
        ),
        Err(StackError::StrideTooSmall)
    );
    assert_eq!(
        check_layout(
            ARENA_BASE,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            0
        ),
        Err(StackError::Empty)
    );
    assert_eq!(
        check_layout(
            ARENA_BASE + 1,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            1
        ),
        Err(StackError::Unaligned)
    );
}

/// Bộ thanh ghi MMU công bố cho hart phụ: **đủ** cả năm trường mới được coi là sẵn sàng.
///
/// `ttbr0 = 0` mà vẫn "sẵn sàng" là lỗi chết người: hart phụ nạp `TTBR0 = 0` rồi bật `M` ⇒ walk
/// đọc frame 0 (không phải bảng trang) ⇒ fault ngay, không log.
#[test]
fn mmu_boot_regs_completeness() {
    let full = MmuBootRegs::new(0x1, 0x2, 0x3, 0x4, 0x5);
    assert!(full.is_complete());
    assert!(!MmuBootRegs::default().is_complete());
    for zero in 0..5 {
        let mut regs = full;
        match zero {
            0 => regs.tcr = 0,
            1 => regs.mair = 0,
            2 => regs.ttbr0 = 0,
            3 => regs.ttbr1 = 0,
            _ => regs.sctlr = 0,
        }
        assert!(
            !regs.is_complete(),
            "thiếu trường {zero} phải là chưa sẵn sàng"
        );
    }
}
