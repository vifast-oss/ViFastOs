// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho GICv3 aarch64 — KT-259 (K4' bước 3, B3.9: IPI `SGI 0`).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng **viết tay từ đặc tả**, không lấy hằng của crate để so:
//!
//! - **GICv3 Architecture Specification** (ARM IHI 0069) — bảng offset `GICD_*`/`GICR_*`, vị trí
//!   bit `GICD_CTLR`/`GICR_WAKER`/`ICC_SRE_EL1`, dải INTID đặc biệt `1020..=1023`;
//! - **DDI 0487** — `ICC_SGI1R_EL1`: `TargetList[15:0]`, `Aff1[23:16]`, `INTID[31:24]`,
//!   `Aff2[39:32]`, `IRM[40]`, `Aff3[47:44]`, `RS[55:48]`;
//! - **QEMU `virt` + `gic-version=3`** (đo bằng `-M virt,dumpdtb` + `dtc`, 2026-11-21): distributor
//!   `0x0800_0000` (64 KiB), redistributor `0x080A_0000`, **mỗi hart `0x2_0000`** (RD 64 KiB + SGI
//!   64 KiB) — xem `v2/agent/JOURNAL/kt259-aarch64-smp.md` §"QEMU".
//!
//! Đây là ca kiểm **đỏ trước** của KT-259 (log: `v2/agent/JOURNAL/kt259-red.log`).
//! **Đột biến (i)** — "bỏ clear `GICR_WAKER.ProcessorSleep`" — không kiểm được trên host (nó là
//! hành vi MMIO); nó được kiểm trên QEMU: hart 1 **không** nhận IPI ⇒ TAP 11 đỏ
//! (log: `kt259-mutation.log`). Ở đây chỉ chốt **vị trí bit** mà đột biến đó dựa vào.

use vifast_plat_aarch64::gic::{
    is_spurious, redistributor_base, sgi1r, sgi1r_for_hart, sgi_frame_base, sgi_target_list,
    GICD_CTLR, GICD_CTLR_ARE_NS, GICD_CTLR_ENABLE_GRP1NS, GICD_CTLR_RWP, GICD_IGROUPR,
    GICD_IPRIORITYR, GICD_ISENABLER, GICD_TYPER, GICR_BASE, GICR_CTLR, GICR_IGROUPR0,
    GICR_IPRIORITYR, GICR_ISENABLER0, GICR_SGI_OFFSET, GICR_STRIDE, GICR_TYPER, GICR_WAKER,
    GICR_WAKER_CHILDREN_ASLEEP, GICR_WAKER_PROCESSOR_SLEEP, ICC_IGRPEN1_ENABLE, ICC_PMR_ALL,
    ICC_SGI1R_AFF1_SHIFT, ICC_SGI1R_AFF2_SHIFT, ICC_SGI1R_AFF3_SHIFT, ICC_SGI1R_INTID_SHIFT,
    ICC_SGI1R_IRM, ICC_SGI1R_IRM_SHIFT, ICC_SGI1R_RS_SHIFT, ICC_SGI1R_TARGET_LIST_SHIFT,
    ICC_SPURIOUS_MIN, ICC_SRE_EL1_SRE, SGI_IPI,
};

/// Offset `GICD_*` — bảng register map của GICv3 (IHI 0069 §8.2), viết tay.
#[test]
fn gicd_offsets() {
    assert_eq!(GICD_CTLR, 0x0000);
    assert_eq!(GICD_TYPER, 0x0004);
    assert_eq!(GICD_IGROUPR, 0x0080);
    assert_eq!(GICD_ISENABLER, 0x0100);
    assert_eq!(GICD_IPRIORITYR, 0x0400);
}

/// Offset `GICR_*` — RD_base (IHI 0069 §9.2) và SGI_base = RD_base + `0x1_0000` (§9.3).
#[test]
fn gicr_offsets() {
    assert_eq!(GICR_CTLR, 0x0000);
    assert_eq!(GICR_TYPER, 0x0008);
    assert_eq!(GICR_WAKER, 0x0014);
    assert_eq!(GICR_SGI_OFFSET, 0x1_0000);
    assert_eq!(GICR_IGROUPR0, 0x0080);
    assert_eq!(GICR_ISENABLER0, 0x0100);
    assert_eq!(GICR_IPRIORITYR, 0x0400);
}

/// `GICR_WAKER`: `ProcessorSleep` bit 1, `ChildrenAsleep` bit 2 (IHI 0069 §9.2.7).
///
/// Đột biến (i) của KT-259 là **bỏ** bước clear bit 1 ⇒ redistributor của hart 1 ngủ ⇒ SGI
/// không được chuyển tới CPU ⇒ TAP 11 đỏ. Vị trí bit sai thì đột biến đó vô nghĩa.
#[test]
fn gicr_waker_bits() {
    assert_eq!(GICR_WAKER_PROCESSOR_SLEEP, 1 << 1);
    assert_eq!(GICR_WAKER_CHILDREN_ASLEEP, 1 << 2);
    assert_eq!(GICR_WAKER_PROCESSOR_SLEEP & GICR_WAKER_CHILDREN_ASLEEP, 0);
}

/// `GICD_CTLR` khi `ARE_NS = 1`: `EnableGrp1NS` bit 1, `ARE_NS` bit 4, `RWP` bit 31.
#[test]
fn gicd_ctlr_bits() {
    assert_eq!(GICD_CTLR_ENABLE_GRP1NS, 1 << 1);
    assert_eq!(GICD_CTLR_ARE_NS, 1 << 4);
    assert_eq!(GICD_CTLR_RWP, 1 << 31);
    // Giá trị ảnh ghi: bật affinity routing **và** nhóm 1 non-secure trong **một** lần ghi.
    assert_eq!(GICD_CTLR_ARE_NS | GICD_CTLR_ENABLE_GRP1NS, 0x12);
}

/// `ICC_SRE_EL1.SRE` bit 0 — phải bật **trước** khi dùng `ICC_PMR_EL1`/`ICC_IGRPEN1_EL1`.
#[test]
fn icc_sre_and_group_bits() {
    assert_eq!(ICC_SRE_EL1_SRE, 1);
    assert_eq!(
        ICC_PMR_ALL, 0xF0,
        "PMR 0xF0: cho phép mọi ưu tiên 0x00..0xEF"
    );
    assert_eq!(ICC_IGRPEN1_ENABLE, 1);
}

/// Redistributor của hart `n` = `GICR_BASE + n * 0x2_0000`; SGI frame = RD + `0x1_0000`.
#[test]
fn redistributor_bases() {
    assert_eq!(GICR_BASE, 0x080A_0000);
    assert_eq!(GICR_STRIDE, 0x2_0000);
    assert_eq!(redistributor_base(0), Some(0x080A_0000));
    assert_eq!(redistributor_base(1), Some(0x080C_0000));
    assert_eq!(redistributor_base(7), Some(0x0818_0000));
    assert_eq!(
        redistributor_base(8),
        None,
        "hart ngoài MAX_HARTS phải bị từ chối"
    );
    assert_eq!(sgi_frame_base(1), Some(0x080D_0000));
    assert_eq!(sgi_frame_base(8), None);
}

/// `ICC_SGI1R_EL1` — vị trí trường theo DDI 0487 (đối chiếu `arch/arm64/include/asm/gic-v3.h`).
#[test]
fn sgi1r_field_positions() {
    assert_eq!(ICC_SGI1R_TARGET_LIST_SHIFT, 0);
    assert_eq!(ICC_SGI1R_AFF1_SHIFT, 16);
    assert_eq!(ICC_SGI1R_INTID_SHIFT, 24);
    assert_eq!(ICC_SGI1R_AFF2_SHIFT, 32);
    assert_eq!(ICC_SGI1R_IRM_SHIFT, 40);
    assert_eq!(ICC_SGI1R_IRM, 1 << 40);
    assert_eq!(ICC_SGI1R_AFF3_SHIFT, 44);
    assert_eq!(ICC_SGI1R_RS_SHIFT, 48);
}

/// Giá trị `ICC_SGI1R_EL1` **tính tay** cho vài tổ hợp (không lấy hàm của crate để so).
#[test]
fn sgi1r_encoding() {
    // SGI 0 tới Aff0 = 1, mọi affinity khác 0, IRM = 0, RS = 0.
    assert_eq!(sgi1r(0, 0b10, 0, 0, 0, false), 0x2);
    // SGI 3 tới Aff0 = 0, Aff1 = 2, Aff2 = 3, Aff3 = 4.
    assert_eq!(
        sgi1r(3, 0b1, 2, 3, 4, false),
        (3 << 24) | 1 | (2 << 16) | (3 << 32) | (4 << 44)
    );
    assert_eq!(sgi1r(3, 0b1, 2, 3, 4, false), 0x4003_0302_0001);
    // IRM = 1 (gửi tới mọi hart trừ chính nó) — bit 40, không đụng Aff3.
    assert_eq!(sgi1r(0, 0, 0, 0, 0, true), 1 << 40);
    assert_eq!(sgi1r(0, 0, 0, 0, 0, true) & (0xF << 44), 0);
}

/// `TargetList` là **bitmask Aff0**: hart 1 ⇒ `0b10`; `Aff0 >= 8` không biểu diễn được ⇒ 0.
#[test]
fn sgi_target_list_is_aff0_bitmask() {
    assert_eq!(sgi_target_list(0), 0b1);
    assert_eq!(sgi_target_list(1), 0b10);
    assert_eq!(sgi_target_list(7), 0x80);
    assert_eq!(
        sgi_target_list(8),
        0,
        "Aff0 ngoài 0..7 phải là 0 (caller từ chối)"
    );
}

/// `sgi1r_for_hart`: hart 1 nhận SGI 0 ⇒ `0x2`; hart ngoài dải ⇒ `None`.
#[test]
fn sgi1r_for_hart_targets_aff0() {
    assert_eq!(sgi1r_for_hart(SGI_IPI, 1), Some(0x2));
    assert_eq!(sgi1r_for_hart(SGI_IPI, 0), Some(0x1));
    assert_eq!(sgi1r_for_hart(SGI_IPI, 8), None);
    assert_eq!(SGI_IPI, 0, "SGI 0 là IPI của KT-259");
}

/// INTID đặc biệt `1020..=1023` **không** phải ngắt thật — handler không được đếm chúng.
#[test]
fn spurious_intid_range() {
    assert_eq!(ICC_SPURIOUS_MIN, 1020);
    assert!(!is_spurious(0));
    assert!(!is_spurious(15));
    assert!(!is_spurious(1019));
    assert!(is_spurious(1020));
    assert!(is_spurious(1023));
}
