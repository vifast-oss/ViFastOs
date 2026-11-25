// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho U-mode (EL0) AArch64 — KT-269 (K4' bước 3, B3.7; D2-206).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng là **giá trị tuyệt đối viết tay** từ Arm Architecture Reference Manual (DDI 0487,
//! `SPSR_EL1.M[3:0]`: `0b0000` = EL0t, `0b0101` = EL1h; `ESR_EL1.ISS[15:0]` = imm16 của `svc`),
//! **không** lấy hằng của crate để so — nếu lấy chính hằng cần kiểm thì test không có răng.
//!
//! Đây là ca kiểm **đỏ trước** của KT-269: viết trước khi `umode` tồn tại. **Đột biến bắt buộc**
//! (i) — `SPSR_EL1` để EL1 (`M = 0b0101`) thay vì EL0 — làm [`is_el0`] sai ngay ở tầng dữ liệu
//! (`spsr_el0_is_el0t` ĐỎ); log QEMU của đột biến ở `v2/docs/k4-aarch64-boot.md` §9.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối viết tay**
// từ Arm ARM (SPSR/ESR/VA) — một hằng đổi giá trị thì test phải đỏ. Clippy coi
// `assert!(HẰNG …)` là "constant value"; đó chính là mục đích của tệp này (cùng khuôn
// `vifast-plat-x86_64/tests/boot.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_aarch64::split_mmu::{half_of, Half};
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass, SPSR_M_MASK};
use vifast_plat_aarch64::umode::{
    is_el0, is_el1h, mode_name, spsr_el0, spsr_el1h, svc_imm, EL0_CODE_VA, EL0_STACK_BYTES,
    EL0_STACK_TOP, EL0_STACK_VA, FRAME_SLOTS, FRAME_X0, FRAME_X1, FRAME_X8, SVC_IMM_MASK,
};

/// `SPSR_EL1` cho EL0: `M[3:0] = 0b0000` và `DAIF` mask đủ (`D|A|I|F` = bit 9..6 = `0x3C0`).
#[test]
fn spsr_el0_is_el0t() {
    let spsr = spsr_el0();
    assert_eq!(spsr & SPSR_M_MASK, 0b0000, "M[3:0] phải là EL0t (0b0000)");
    assert_eq!(spsr & 0x3C0, 0x3C0, "DAIF phải mask đủ (bit 9..6)");
    assert!(is_el0(spsr), "spsr_el0 phải được nhận là EL0");
    assert!(!is_el1h(spsr), "spsr_el0 KHÔNG được nhận là EL1h");
    assert_eq!(mode_name(spsr), "EL0t");
}

/// `SPSR_EL1` quay về EL1h: `M[3:0] = 0b0101` — đường `Exit` của EL0 trả quyền về kernel.
#[test]
fn spsr_el1h_is_el1h() {
    let spsr = spsr_el1h();
    assert_eq!(spsr & SPSR_M_MASK, 0b0101, "M[3:0] phải là EL1h (0b0101)");
    assert!(is_el1h(spsr), "spsr_el1h phải được nhận là EL1h");
    assert!(!is_el0(spsr), "spsr_el1h KHÔNG được nhận là EL0");
    assert_eq!(mode_name(spsr), "EL1h");
}

/// **Đột biến (i)** ở tầng dữ liệu: `SPSR_EL1` để EL1 (`M = 0b0101`) **không** được nhận là EL0.
///
/// Đây là điều kiện để handler từ chối `eret` sai chế độ thay vì chạy mã EL0 ở EL1.
#[test]
fn el1h_spsr_is_not_el0() {
    let wrong = spsr_el1h();
    assert!(!is_el0(wrong), "SPSR M=0b0101 KHÔNG được coi là EL0");
    assert_ne!(spsr_el0() & SPSR_M_MASK, wrong & SPSR_M_MASK);
    // Bit 4 (`nRW`) = 0 ⇒ AArch64; cả hai chế độ đều phải giữ điều này.
    assert_eq!(spsr_el0() & (1 << 4), 0);
    assert_eq!(spsr_el1h() & (1 << 4), 0);
}

/// `ESR_EL1.ISS[15:0]` = imm16 của `svc`; bit cao của ISS **không** được lẫn vào.
#[test]
fn svc_imm_takes_low_16_bits() {
    assert_eq!(SVC_IMM_MASK, 0xFFFF);
    assert_eq!(svc_imm(0), 0, "`svc #0` ⇒ imm16 = 0");
    assert_eq!(svc_imm(0x1234_5678), 0x5678);
    assert_eq!(svc_imm(0xFFFF_FFFF), 0xFFFF);
    // `Esr::new(Svc64, il=false, iss)` — đường handler thật dùng.
    let esr = Esr::new(ExceptionClass::Svc64, false, 0x0000_0007);
    assert_eq!(esr.ec(), Some(ExceptionClass::Svc64));
    assert_eq!(esr.ec_raw(), 0x15, "EC của SVC64 phải là 0x15");
    assert_eq!(svc_imm(esr.iss()), 7);
}

/// Hai VA của U-mode: căn 4 KiB, không đè nhau, stack mọc xuống từ `EL0_STACK_TOP`.
#[test]
fn el0_vas_are_page_aligned_and_disjoint() {
    assert_eq!(EL0_CODE_VA & 0xFFF, 0, "VA mã EL0 phải căn 4 KiB");
    assert_eq!(EL0_STACK_VA & 0xFFF, 0, "VA stack EL0 phải căn 4 KiB");
    assert_eq!(EL0_STACK_BYTES, 0x1000, "stack user là một trang 4 KiB");
    assert_eq!(EL0_STACK_TOP, EL0_STACK_VA + EL0_STACK_BYTES);
    assert_ne!(EL0_CODE_VA, EL0_STACK_VA, "mã và stack phải khác trang");
    assert!(
        EL0_CODE_VA + 0x1000 <= EL0_STACK_VA,
        "trang mã phải nằm dưới trang stack, không chồng lấn"
    );
}

/// VA của U-mode phải thuộc **nửa thấp** ⇒ đi qua `TTBR0` (bảng user), không phải `TTBR1`.
///
/// Nếu rơi vào nửa cao, `VSpace::map_obj` sẽ map vào bảng kernel và EL0 không bao giờ chạm tới.
#[test]
fn el0_vas_route_to_ttbr0() {
    assert_eq!(half_of(EL0_CODE_VA), Some(Half::Low));
    assert_eq!(half_of(EL0_STACK_VA), Some(Half::Low));
    assert_eq!(half_of(EL0_STACK_TOP - 8), Some(Half::Low));
}

/// VA của U-mode **không** được trùng hai VA probe của các phép đo trước (KT-249/KT-254).
///
/// Trùng thì phép đo "MMU thật sự bật" hoặc "VSpace activate" sẽ đọc/ghi nhầm trang của EL0 và
/// cả hai phép đo mất nghĩa — đây là bất biến bố cục, không phải sở thích.
#[test]
fn el0_vas_do_not_collide_with_earlier_probes() {
    const PROBE_VA_MMU: u64 = 0x5000_0000;
    const PROBE_VA_VSPACE: u64 = 0x6000_0000;
    for va in [EL0_CODE_VA, EL0_STACK_VA] {
        assert_ne!(va, PROBE_VA_MMU);
        assert_ne!(va, PROBE_VA_VSPACE);
    }
    // Và nằm ngoài RAM 128 MiB của QEMU `virt` (kết thúc `0x4800_0000`) ⇒ không phải identity.
    assert!(EL0_CODE_VA >= 0x4800_0000);
    assert!(
        EL0_STACK_TOP <= 0x0000_0040_0000_0000,
        "phải còn trong nửa thấp 39-bit"
    );
}

/// Chỉ số ô khung trap khớp **tuyệt đối** thứ tự `stp` của stub vector (`x0..x18, x29, x30`).
///
/// Stub lưu `x0` ở ô 0, `x1` ở ô 1, …, `x8` ở ô 8. Handler đọc số hiệu syscall ở `FRAME_X8`;
/// lệch một ô là đọc nhầm thanh ghi — không lỗi biên dịch, chỉ sai âm thầm.
#[test]
fn frame_slot_indices_match_vector_stub() {
    assert_eq!(FRAME_X0, 0);
    assert_eq!(FRAME_X1, 1);
    assert_eq!(FRAME_X8, 8);
    assert_eq!(FRAME_SLOTS, 21, "x0..x18 (19) + x29 + x30 = 21 ô");
    // 21 ô × 8 B = 168 B ≤ khung 176 B của stub (16 B dư cho `x9` gốc + căn 16).
    assert!(FRAME_SLOTS * 8 <= 176);
    assert!(FRAME_X8 < FRAME_SLOTS);
}
