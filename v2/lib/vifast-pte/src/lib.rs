// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
#![forbid(unsafe_code)]
//! `vifast-pte` — hình ba cấp và layout PTE **model-only** của K2 (KT-35).
//!
//! Crate này chỉ cung cấp **hình/layout**: cỡ Frame ↔ `size_bits`, PTE thô 64 bit, mảng PTE cố
//! định, và `walk_shape` tách VA 39 bit thành ba chỉ số 9 bit. **Không** có PageTable walker,
//! map/unmap, quyền/attrs, W^X, ASID, TLB — những phần đó thuộc KT-37..KT-41. Tài liệu mô hình
//! và bảy hằng số `[TK]` ở `v2/docs/k2-vm.md`.
//!
//! **K2 không phải kernel thật**: đây là mô hình tất định trong sim, không PTE phần cứng, không
//! MMU/TLB thật, không QEMU, không `sfence.vma`; K3 mới là kernel/QEMU thật. Xem
//! `review/agent0/K2-PLAN.md` §1 và D2-65.
//!
//! # Ràng buộc (D2-65 Q1/Q2/Q8)
//!
//! - `#![no_std]`; test tích hợp dùng `std` của **crate test** (D2-42), crate không cần feature.
//! - `#![forbid(unsafe_code)]` — không `unsafe`, không con trỏ thô, không `transmute`.
//! - **Không** dependency ngoài (D2-21): không `vifast-abi`, không `alloc`, không crate thứ ba.
//! - **Không** `cfg` theo kiến trúc (luật R2) — hình PTE là của **mô hình**, không của kiến trúc.
//! - **Không** đổi ABI/`spec/`: `VSpace`/`PageTable`/`Frame` vẫn `reserved` + `unsupported`;
//!   `size_bits` lạ trả lỗi model/local [`PteError::InvalidArgument`], không thêm mã ABI.
//!
//! # Bảy hằng số `[TK]` (Q1)
//!
//! Giá trị dưới đây là **thiết kế** (`[TK]`), không phải số đo; nguồn ngữ nghĩa:
//! `review/agent0/K2-PLAN.md` §2 Q1/Q2/Q4 và D2-65. Tài liệu `v2/docs/k2-vm.md` chép lại bảng
//! này; task sau **đọc từ crate**, không chép tay giá trị.

mod error;
mod frame;
mod pte;
mod walk;

pub use error::PteError;
pub use frame::FrameSize;
pub use pte::{Pte, PteArray};
pub use walk::{walk_shape, WalkShape};

/// `size_bits` của ba cỡ Frame: `[12, 21, 30]` (4 KiB / 2 MiB / 1 GiB) — `[TK]`, Q1.
///
/// Thứ tự khớp [`FrameSize::ALL`] (nhỏ → lớn). Phần tử thứ `i` là cấp lá `i` của hình 3 cấp.
pub const PAGE_SIZE_BITS: [u8; 3] = [K4_BITS, M2_BITS, G1_BITS];

/// Số cấp của hình PTE: **3** (gốc → giữa → lá) — `[TK]`, Q2 (kiểu Sv39).
pub const PTE_LEVELS: usize = 3;

/// Số bit không gian địa chỉ ảo: **39** — `[TK]`, Q2.
///
/// VA chia `9 / 9 / 9 / 12`; bit `38..12` vào ba chỉ số, bit `11..0` là offset trong trang.
pub const VA_BITS: u8 = 39;

/// Số bit ASID: **8** ⇒ 256 ASID mỗi thế hệ — `[TK]`, Q4.
pub const ASID_BITS: u8 = 8;

/// Số entry TLB mỗi hart: **16** — `[TK]`, Q4 (TLB model tất định của K2).
pub const TLB_ENTRIES_PER_HART: usize = 16;

/// Biên số Frame sống trong **một** VSpace: **4096** — `[TK]`, Q1.
///
/// Luỹ thừa 2 (4096 = `2^12`) để biên chia hết cho bảng 512 entry và để mô hình không cấp phát
/// động. Vượt biên là lỗi của task cài đặt (KT-36/KT-37 quyết định mã), **không** phải hằng ABI.
pub const MAX_FRAMES_PER_VSPACE: usize = 4096;

/// Độ sâu tối đa khi đi/ dựng cây: **3** — `[TK]`, Q1.
///
/// Bằng [`PTE_LEVELS`] (giữ tên riêng vì Q1 yêu cầu và vì KT-37 dùng làm biên vòng lặp); không
/// phải chỗ để nới hình 3 cấp.
pub const MAX_MAP_DEPTH: usize = PTE_LEVELS;

// `size_bits` từng cỡ, để `FrameSize` và `PAGE_SIZE_BITS` không lệch nhau (const-pattern).
const K4_BITS: u8 = 12;
const M2_BITS: u8 = 21;
const G1_BITS: u8 = 30;
