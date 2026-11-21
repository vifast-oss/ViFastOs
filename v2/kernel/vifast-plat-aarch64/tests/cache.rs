// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho cache maintenance trước/sau DMA — KT-277 (K4' bước 3).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng là **giá trị tuyệt đối viết tay**, đối chiếu bằng hợp dịch thật
//! (`clang --target=aarch64-none-elf` + `llvm-objdump`) — xem bảng trong `src/cache.rs`.
//!
//! # Vì sao test này là **bằng chứng** cho đột biến "bỏ `dc civac`"
//!
//! QEMU TCG **không** mô hình hoá cache, nên bỏ lệnh `dc` không làm ảnh hưởng kết quả DMA trên
//! QEMU. Bằng chứng ở tầng host vì thế là **số lệnh + địa chỉ từng dòng** mà vòng lặp phát ra:
//! [`for_each_line`] là vòng lặp **duy nhất** mà driver dùng, và test dưới đây khẳng định chính
//! xác dãy địa chỉ nó gọi. Đột biến "bỏ bước làm sạch" khỏi [`HANDOFF_SEQUENCE`] làm
//! `handoff_sequence_cleans_before_notify` **ĐỎ**; đột biến "bỏ lời gọi `dc`" làm bộ đếm
//! `lines=0` ở ảnh ⇒ runner **ĐỎ** (tầng quan sát được trên QEMU).

use vifast_plat_aarch64::cache::{
    cache_line_bytes, dc_encoding, for_each_line, line_count, CompleteStep, HandoffStep,
    COMPLETE_SEQUENCE, CTR_DMINLINE_MASK, CTR_DMINLINE_SHIFT, DC_CIVAC, DC_CVAC, DC_CVAU, DC_IVAC,
    DC_SYS_BASE_OP1_0, DC_SYS_BASE_OP1_3, DEFAULT_CACHE_LINE, HANDOFF_SEQUENCE, IC_IVAU,
};

// ---------------------------------------------------------------- encoding

/// Mã `DC`/`IC` — giá trị tuyệt đối của `llvm-objdump`:
/// `dc civac d50b7e20`, `dc cvac d50b7a20`, `dc cvau d50b7b20`, `dc ivac d5087620`,
/// `ic ivau d50b7520`.
///
/// **`dc ivac` là `op1 = 0`**, không phải `op1 = 3` như `dc civac` — đây là chỗ dễ sai nhất của
/// cả họ lệnh, và là lý do test so **từng mã tuyệt đối** thay vì chỉ so "khác nhau".
#[test]
fn dc_encodings() {
    assert_eq!(DC_SYS_BASE_OP1_0, 0xD508_7000);
    assert_eq!(DC_SYS_BASE_OP1_3, 0xD50B_7000);
    assert_eq!(DC_CIVAC, 0xD50B_7E20);
    assert_eq!(DC_CVAC, 0xD50B_7A20);
    assert_eq!(DC_CVAU, 0xD50B_7B20);
    assert_eq!(DC_IVAC, 0xD508_7620);
    assert_eq!(IC_IVAU, 0xD50B_7520);
    // Công thức ghép: base | CRm<<8 | op2<<5 | Rt.
    assert_eq!(dc_encoding(3, 14, 1, 0), 0xD50B_7E20);
    assert_eq!(dc_encoding(0, 6, 1, 0), 0xD508_7620);
    assert_eq!(dc_encoding(3, 10, 1, 0), 0xD50B_7A20);
    assert_eq!(dc_encoding(3, 11, 1, 0), 0xD50B_7B20);
    assert_eq!(dc_encoding(3, 5, 1, 0), 0xD50B_7520);
    // Trường bị cắt đúng số bit: CRm 4 bit, op2 3 bit, Rt 5 bit.
    assert_eq!(dc_encoding(3, 0x1F, 0x9, 0x3F), 0xD50B_7F3F);
    // `dc civac` và `dc ivac` **khác nhau ở op1** — không thể suy ra lẫn nhau.
    assert_eq!(DC_CIVAC & 0x7_0000, 0x3_0000);
    assert_eq!(DC_IVAC & 0x7_0000, 0);
    // Cả hai dùng `Rt = 0` (asm đặt tham số vào `x0`).
    assert_eq!(DC_CIVAC & 0x1F, 0);
    assert_eq!(DC_IVAC & 0x1F, 0);
}

// ---------------------------------------------------------------- cỡ dòng

/// `CTR_EL0.DminLine` → cỡ dòng: `4 << DminLine`, có sàn/trần chống giá trị vô lý.
#[test]
fn cache_line_bytes_from_ctr() {
    // `cortex-a72` (QEMU virt): DminLine = 4 ⇒ 64 B.
    assert_eq!(cache_line_bytes(0x8444_C004), 64);
    // DminLine = 5 ⇒ 128 B (Apple M-series).
    assert_eq!(cache_line_bytes(5 << CTR_DMINLINE_SHIFT), 128);
    // DminLine = 0 ⇒ 4 B < sàn 16 ⇒ dự phòng.
    assert_eq!(cache_line_bytes(0), DEFAULT_CACHE_LINE);
    // DminLine = 15 ⇒ 128 KiB > trần 4 KiB ⇒ dự phòng.
    assert_eq!(
        cache_line_bytes(0xF << CTR_DMINLINE_SHIFT),
        DEFAULT_CACHE_LINE
    );
    // Chỉ đọc đúng bốn bit của trường.
    assert_eq!(CTR_DMINLINE_MASK, 0xF);
    assert_eq!(cache_line_bytes(0xFFFF_FFFF_FFFF_FFFF), DEFAULT_CACHE_LINE);
}

// ---------------------------------------------------------------- số dòng + địa chỉ

/// Trang DMA 4 KiB căn lề (đúng `virtio_blk::DMA_PAGE_BYTES`) ⇒ **64** dòng 64 B.
#[test]
fn line_count_page() {
    let base = 0x400C_C000u64;
    assert_eq!(line_count(base, 4096, 64), 64);
    assert_eq!(line_count(base, 4096, 128), 32);
    assert_eq!(line_count(base, 0, 64), 0);
    assert_eq!(line_count(base, 4096, 0), 0);
    // Đầu **không** căn dòng ⇒ làm tròn ra ngoài: 1 + 64 = 65 dòng.
    assert_eq!(line_count(base + 0x34, 4096, 64), 65);
    // Một byte nằm gọn trong một dòng.
    assert_eq!(line_count(base + 0x40, 1, 64), 1);
}

/// Vòng lặp phát `dc` phải gọi **đúng** dãy địa chỉ dòng — đây là "đếm số lệnh/địa chỉ" mà
/// KT-277 yêu cầu để bù cho việc QEMU TCG không mô hình hoá cache.
#[test]
fn for_each_line_addresses() {
    let base = 0x400C_C000u64;
    let mut seen: Vec<u64> = Vec::new();
    let n = for_each_line(base, 4096, 64, |at| seen.push(at));
    assert_eq!(n, 64);
    assert_eq!(seen.len(), 64);
    assert_eq!(seen[0], base);
    assert_eq!(seen[1], base + 64);
    assert_eq!(seen[63], base + 63 * 64);
    // Mọi địa chỉ đều căn dòng và tăng đều 64 B.
    for (index, at) in seen.iter().enumerate() {
        assert_eq!(*at, base + index as u64 * 64);
        assert_eq!(at & 63, 0);
    }
    // Đầu lệch dòng: dòng đầu bị **làm tròn xuống**, không bỏ sót byte nào.
    let mut seen2: Vec<u64> = Vec::new();
    let n2 = for_each_line(base + 0x34, 4096, 64, |at| seen2.push(at));
    assert_eq!(n2, 65);
    assert_eq!(seen2[0], base);
    assert_eq!(seen2[64], base + 64 * 64);
    // Khoảng rỗng ⇒ không lệnh nào.
    let mut calls = 0;
    assert_eq!(for_each_line(base, 0, 64, |_| calls += 1), 0);
    assert_eq!(calls, 0);
}

// ---------------------------------------------------------------- thứ tự bắt buộc

/// **Trước** khi bàn giao: `dc civac` phải đứng **trước** mọi barrier và trước `notify`.
///
/// Đột biến "bỏ bước làm sạch" (xoá `CleanInvalidate` khỏi mảng) làm test này **ĐỎ** — đó là
/// tầng host quan sát được của đột biến ấy.
#[test]
fn handoff_sequence_cleans_before_notify() {
    assert_eq!(HANDOFF_SEQUENCE.len(), 4);
    assert_eq!(HANDOFF_SEQUENCE[0], HandoffStep::CleanInvalidate);
    assert_eq!(HANDOFF_SEQUENCE[1], HandoffStep::DsbIsh);
    assert_eq!(HANDOFF_SEQUENCE[2], HandoffStep::DsbSy);
    assert_eq!(HANDOFF_SEQUENCE[3], HandoffStep::Notify);
    // Đúng **một** bước làm sạch, và nó đứng trước `Notify`.
    let cleans = HANDOFF_SEQUENCE
        .iter()
        .filter(|step| **step == HandoffStep::CleanInvalidate)
        .count();
    assert_eq!(cleans, 1);
    let clean_at = HANDOFF_SEQUENCE
        .iter()
        .position(|step| *step == HandoffStep::CleanInvalidate)
        .expect("phải có bước làm sạch");
    let notify_at = HANDOFF_SEQUENCE
        .iter()
        .position(|step| *step == HandoffStep::Notify)
        .expect("phải có bước notify");
    assert!(clean_at < notify_at, "làm sạch phải đứng trước notify");
    // Tên in log là dữ liệu, không phải chuỗi viết tay ở chỗ khác.
    assert_eq!(HandoffStep::CleanInvalidate.name(), "clean");
    assert_eq!(HandoffStep::Notify.name(), "notify");
}

/// **Sau** khi thiết bị ghi xong: `dc ivac` phải đứng **trước** barrier (và trước mọi lần đọc).
#[test]
fn complete_sequence_invalidates_first() {
    assert_eq!(COMPLETE_SEQUENCE.len(), 2);
    assert_eq!(COMPLETE_SEQUENCE[0], CompleteStep::Invalidate);
    assert_eq!(COMPLETE_SEQUENCE[1], CompleteStep::DsbIsh);
    assert_eq!(CompleteStep::Invalidate.name(), "invalidate");
}

// ---------------------------------------------------------------- stub host

/// Stub host phải **panic**, không được là no-op im lặng: một no-op sẽ khiến test "xanh" trong
/// khi chẳng có lệnh `dc` nào chạy.
#[test]
fn host_stub_is_fail_loud() {
    let civac = std::panic::catch_unwind(|| vifast_plat_aarch64::asm::dc_civac(0x4000_0000));
    assert!(civac.is_err(), "stub host của `dc civac` phải panic");
    let ivac = std::panic::catch_unwind(|| vifast_plat_aarch64::asm::dc_ivac(0x4000_0000));
    assert!(ivac.is_err(), "stub host của `dc ivac` phải panic");
    let ctr = std::panic::catch_unwind(vifast_plat_aarch64::asm::read_ctr_el0);
    assert!(ctr.is_err(), "stub host của `mrs ctr_el0` phải panic");
}
