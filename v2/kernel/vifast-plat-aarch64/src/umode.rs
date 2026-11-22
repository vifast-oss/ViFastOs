// SPDX-License-Identifier: MIT OR Apache-2.0
//! **U-mode (EL0) trên aarch64** — dữ liệu + helper thuần cho B3.7 (KT-269, D2-206).
//!
//! # Vì sao tệp này **không** có `unsafe` và **không** asm
//!
//! Crate giữ `#![deny(unsafe_code)]`; `unsafe` chỉ ở [`crate::asm`] (D2-17). Tệp này là **dữ
//! liệu** mà cả asm lẫn handler dùng:
//!
//! - **VA** của trang mã EL0 và trang stack user ([`EL0_CODE_VA`], [`EL0_STACK_VA`]) — hai VA
//!   **nửa thấp** (đi qua `TTBR0`, xem [`crate::split_mmu::half_of`]) nằm ngoài mọi VA mà các
//!   phép đo trước đã dùng (`0x5000_0000` probe MMU, `0x6000_0000` probe VSpace);
//! - **`SPSR_EL1`** cho `eret` xuống EL0 ([`spsr_el0`]) và quay về EL1h ([`spsr_el1h`]) — dựng
//!   bằng builder const [`crate::sysreg::Spsr`], không viết tay bit;
//! - **`ESR_EL1.ISS[15:0]`** = imm16 của `svc` ([`svc_imm`]) — đúng trường mà handler đọc;
//! - **chỉ số ô** của khung trap ([`FRAME_X0`], [`FRAME_X1`], [`FRAME_X8`]) — khớp **tuyệt đối**
//!   với thứ tự `stp` của stub vector trong ảnh (test host chốt).
//!
//! Nhờ vậy toàn bộ hình học U-mode chạy được trên host (`tests/umode.rs`), kể cả **đột biến
//! `SPSR_EL1` để EL1** (đột biến (i) của KT-269): nó làm [`is_el0`] sai ngay ở tầng dữ liệu.
//!
//! # Hợp đồng với `spec/abi.toml`
//!
//! Quy ước gọi (`x8` = số hiệu, `x0`/`x1` = tham số + `(error, value)`, `x2` = MessageInfo,
//! `x3..x6` = 4 MR, chuyển bằng `svc #0`) là **nguồn sự thật** ở `spec/abi.toml`
//! `[calling.aarch64]` (D2-206) và được `vifast-gen` sinh thành hằng trong
//! `vifast_abi::generated` (`AARCH64_*`). Ảnh dùng hằng **sinh** cho **số hiệu syscall**
//! (`SYS_DEBUG_PUT_STR`, `SYS_EXIT`); tên thanh ghi trong asm của ảnh là chỗ duy nhất còn viết
//! tay — hành vi được đo bằng TAP 16/17/18 trên QEMU (sai thanh ghi ⇒ số hiệu đọc ra 0 ⇒ TAP
//! `not ok`), không thể chứng minh bằng test host.

use crate::sysreg::{Daif, Spsr, SpsrMode, SPSR_M_MASK};

/// VA **nửa thấp** của trang mã EL0 (map `Perms::USER_RX` từ section `.el0` của ảnh).
///
/// Chọn `0x7000_0000`: ngoài RAM 128 MiB của QEMU `virt` (kết thúc `0x4800_0000`), khác
/// `0x5000_0000` (probe MMU của KT-249) và `0x6000_0000` (probe `VSpace` của KT-254) — ba phép
/// đo không thể vô tình dùng chung một trang.
pub const EL0_CODE_VA: u64 = 0x7000_0000;

/// VA **nửa thấp** của trang stack user (map `Perms::USER_RW`).
pub const EL0_STACK_VA: u64 = 0x7000_1000;

/// Cỡ trang stack user — **một** trang 4 KiB (đủ cho stub EL0 của B3.7, không phải stack đầy đủ).
pub const EL0_STACK_BYTES: u64 = 0x1000;

/// Đỉnh stack user = `SP_EL0` lúc `eret` (stack mọc xuống).
pub const EL0_STACK_TOP: u64 = EL0_STACK_VA + EL0_STACK_BYTES;

/// Mặt nạ `ESR_EL1.ISS[15:0]` — imm16 của `svc` (DDI 0487 §D17.2.37).
pub const SVC_IMM_MASK: u32 = 0xFFFF;

/// Chỉ số ô `x0` trong khung trap của stub vector (ô 0).
pub const FRAME_X0: usize = 0;
/// Chỉ số ô `x1` trong khung trap (ô 1).
pub const FRAME_X1: usize = 1;
/// Chỉ số ô `x2` trong khung trap (ô 2) — `MessageInfo` **trả về** + tham số thứ ba của syscall.
pub const FRAME_X2: usize = 2;
/// Chỉ số ô `x3` trong khung trap (ô 3) — `MR0` **trả về** (và tham số thứ tư của syscall).
pub const FRAME_X3: usize = 3;
/// Chỉ số ô `x4` trong khung trap (ô 4) — `MR1` trả về.
pub const FRAME_X4: usize = 4;
/// Chỉ số ô `x5` trong khung trap (ô 5) — `MR2` trả về.
pub const FRAME_X5: usize = 5;
/// Chỉ số ô `x6` trong khung trap (ô 6) — `MR3` trả về (`[calling.aarch64].mr_registers`).
pub const FRAME_X6: usize = 6;
/// Chỉ số ô `x8` trong khung trap (ô 8) — **số hiệu syscall** theo `[calling.aarch64]`.
pub const FRAME_X8: usize = 8;

/// Số ô của khung trap: `x0..x18` (19) + `x29` + `x30` = **21** ô × 8 B = 168 B trong khung 176 B.
pub const FRAME_SLOTS: usize = 21;

/// `ESR_EL1.ISS[15:0]` — imm16 của instruction `svc` (giá trị `svc #0` ⇒ `0`).
pub const fn svc_imm(iss: u32) -> u16 {
    (iss & SVC_IMM_MASK) as u16
}

/// `SPSR_EL1` để `eret` xuống **EL0t** với `DAIF` mask đủ.
///
/// `M[3:0] = 0b0000` (EL0t) là điều kiện **bắt buộc** để `eret` hạ đặc quyền; `DAIF` mask đủ
/// để EL0 không nhận ngắt khi chưa có đường xử lý ngắt từ EL0 (B3.7 **không** làm preemption).
pub const fn spsr_el0() -> u64 {
    Spsr::new(SpsrMode::El0t).with_daif(Daif::all()).bits()
}

/// `SPSR_EL1` để `eret` về **EL1h** (đường `Exit` của EL0 quay lại kernel), `DAIF` mask đủ.
pub const fn spsr_el1h() -> u64 {
    Spsr::new(SpsrMode::El1h).with_daif(Daif::all()).bits()
}

/// `true` ⇔ `spsr` có `M[3:0] = 0b0000` (EL0t) — điều kiện "đang thật sự ở EL0".
pub const fn is_el0(spsr: u64) -> bool {
    spsr & SPSR_M_MASK == SpsrMode::El0t.bits()
}

/// `true` ⇔ `spsr` có `M[3:0] = 0b0101` (EL1h).
pub const fn is_el1h(spsr: u64) -> bool {
    spsr & SPSR_M_MASK == SpsrMode::El1h.bits()
}

/// Tên chế độ của `SPSR_EL1` để in log (D2-102: in từ dữ liệu, không chép chuỗi tại chỗ gọi).
pub const fn mode_name(spsr: u64) -> &'static str {
    match SpsrMode::from_bits(spsr) {
        Some(SpsrMode::El0t) => "EL0t",
        Some(SpsrMode::El1t) => "EL1t",
        Some(SpsrMode::El1h) => "EL1h",
        Some(SpsrMode::El2t) => "EL2t",
        Some(SpsrMode::El2h) => "EL2h",
        Some(SpsrMode::El3t) => "EL3t",
        Some(SpsrMode::El3h) => "EL3h",
        None => "?",
    }
}
