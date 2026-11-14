// SPDX-License-Identifier: MIT OR Apache-2.0
//! `VSpace` + ASID + `generation` + `active_on: HartMask` của K2 — model-only `[TK]` (KT-38;
//! D2-65 Q4/Q9; `v2/docs/k2-vm.md` §12; thiết kế §4.10 `:353-366`).
//!
//! **K2 không phải kernel thật**: đây là **mô hình** trong sim — không `sfence.vma`, không
//! IPI/TLB phần cứng, không MMU. `VSpace` của mô hình là **metadata** (asid/thế hệ/`active_on`)
//! cộng một **root PageTable** thật của mô hình (KT-37); root đó tiêu **một trang 4 KiB** arena
//! của Untyped, còn bản thân `VSpace` **không** tiêu byte arena nào (`len = 0`).
//!
//! # ASID: 8 bit, 256/thế hệ, free-list, **không tái dùng trong cùng thế hệ**
//!
//! - [`ASID_COUNT`] = `1 << vifast_pte::ASID_BITS` = **256** — **đọc từ crate hình PTE**, không
//!   chép tay `256`.
//! - **Cấp**: lấy phần tử thấp nhất còn lại của free-list ([`fresh_asid_free`]); free-list của
//!   một thế hệ được nạp lại **đủ 256** khi thế hệ tăng.
//! - **Không tái dùng trong cùng thế hệ**: ASID đã cấp thì **về hưu** cho tới hết thế hệ —
//!   teardown VSpace **không** trả ASID về free-list hiện tại. Lý do (K2-PLAN §3 KT-38 "rủi ro"):
//!   hart đã rời VSpace vẫn có thể còn entry TLB mang `(gen, asid)` cũ; nếu ASID được phát lại
//!   trong cùng thế hệ thì entry đó sẽ khớp nhầm một VSpace mới ⇒ lỗi "stale TLB" giả ở K3.
//!   Chỉ khi `generation += 1` thì mọi entry cũ mất hiệu lực (flush theo `gen`).
//! - **Cạn** (free-list rỗng) ⇒ `generation += 1`, free-list nạp lại 256, phát **đúng một** lệnh
//!   [`FlushScope::Generation`] cho **mọi hart online** — không flush từng ASID (Q4).
//!
//! # `active_on: HartMask`
//!
//! `active_on` = tập hart đang nạp ASID của VSpace (hart **chạy** trên nó); tối đa **một**
//! VSpace mỗi hart ([`crate::kobj::Machine::vspace_activate`] tự bỏ hart khỏi VSpace cũ). Bất
//! biến: `active_on ⊆ hart online`; hart offline bị bỏ khỏi **mọi** `active_on` nên không nhận
//! shootdown. Teardown phát [`FlushScope::Asid`] tới `active_on` (đã lọc online).
//!
//! **Chưa có TLB/shootdown thật** (KT-40): KT-38 chỉ ghi **lệnh flush** vào
//! [`crate::kobj::Machine::flush_log`] (`(asid, gen, harts)`); KT-40 dựng TLB model và tiêu thụ
//! log này, thêm kind `(asid, va)` cho unmap/protect theo Q4.

use crate::kobj::ObjId;
use vifast_pte::ASID_BITS;

/// Số ASID mỗi thế hệ: `1 << ASID_BITS` = **256** — `[TK]`, Q4 (`k2-vm.md` §2/§12).
pub const ASID_COUNT: usize = 1 << ASID_BITS;

/// `HartMask` của mô hình: **≤ 64 bit**, bit `h` = hart `h` (K2-PLAN KT-38 "(D2-59 tinh thần
/// `HartMask`)").
///
/// Kiểu riêng (không phải `u64` trần) để `active_on`/`hart_online` không lẫn với các mask khác
/// và để mọi phép kiểm "chỉ hart có thật" đi qua **một** cửa. `MAX_HARTS` = 8 nên 64 bit là dư;
/// bit `≥ 64` bị [`HartMask::from_bits`] giữ nguyên nhưng mọi phép `insert`/`contains` bỏ qua
/// (không panic, không UB — chỉ là ngoài dải hart của mô hình).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HartMask(u64);

impl HartMask {
    /// Không hart nào.
    pub const EMPTY: Self = Self(0);

    /// Từ bit thô (giữ nguyên mọi bit, kể cả bit `≥ 64` không tồn tại).
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Bit thô — dùng cho test/so sánh mask.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Hợp các hart `0..harts` (mask **hart online** của mô hình; `harts ≥ 64` ⇒ đủ 64 bit).
    pub const fn for_harts(harts: u16) -> Self {
        if harts >= 64 {
            Self(u64::MAX)
        } else {
            Self((1u64 << harts) - 1)
        }
    }

    /// Rỗng?
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Số hart trong mask.
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Có hart `hart`? Bit `≥ 64` ⇒ `false` (không panic).
    pub const fn contains(self, hart: u16) -> bool {
        hart < 64 && (self.0 >> hart) & 1 == 1
    }

    /// Bật hart `hart` (bit `≥ 64` ⇒ không đổi).
    pub const fn insert(self, hart: u16) -> Self {
        if hart < 64 {
            Self(self.0 | (1u64 << hart))
        } else {
            self
        }
    }

    /// Tắt hart `hart` (bit `≥ 64` ⇒ không đổi).
    pub const fn remove(self, hart: u16) -> Self {
        if hart < 64 {
            Self(self.0 & !(1u64 << hart))
        } else {
            self
        }
    }

    /// Hợp hai mask.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Giao hai mask.
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// `self ⊆ other` (mọi bit của `self` đều có trong `other`).
    pub const fn is_subset_of(self, other: Self) -> bool {
        self.0 & other.0 == self.0
    }
}

impl core::ops::BitOr for HartMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl core::ops::BitAnd for HartMask {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.intersection(rhs)
    }
}

/// Dữ liệu của một `VSpace` — model-only, nằm trong `Machine::vspaces`, không phải object body
/// (body chỉ là mốc [`crate::kobj::Kind::VSpace`] để objmem cấp metadata).
///
/// `generation` là **thế hệ lúc cấp ASID** (thế hệ toàn cục của mô hình, không phải trường của
/// VSpace): VSpace cũ giữ nguyên `(asid, generation)` cũ kể cả sau khi thế hệ tăng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VSpaceData {
    /// PageTable gốc của cây (object **nội bộ**, không mang cap).
    pub(crate) root: ObjId,
    /// ASID đã cấp (0..[`ASID_COUNT`]).
    pub(crate) asid: u16,
    /// Thế hệ lúc cấp ASID.
    pub(crate) generation: u64,
    /// Tập hart đang nạp ASID của VSpace này (`⊆` hart online).
    pub(crate) active_on: HartMask,
}

/// Ảnh chụp một `VSpace` cho API/debug (`GetAsid` chỉ trả asid; đây trả cả thế hệ/active_on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VSpaceInfo {
    /// PageTable gốc (dùng được ngay với [`crate::kobj::Machine::page_table_walk`] — oracle).
    pub root: ObjId,
    /// ASID đã cấp.
    pub asid: u16,
    /// Thế hệ lúc cấp.
    pub generation: u64,
    /// Tập hart đang chạy VSpace này.
    pub active_on: HartMask,
}

/// Ảnh chụp **trạng thái ASID toàn cục** của mô hình (debug/test).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AsidState {
    /// Thế hệ hiện tại (0 lúc khởi tạo; `+1` mỗi lần cạn ASID).
    pub generation: u64,
    /// Số ASID còn cấp được **trong thế hệ hiện tại** (free-list).
    pub free: usize,
    /// Số ASID **đã cấp** trong thế hệ hiện tại (cộng dồn; teardown không trả lại).
    pub issued: u32,
    /// Số lần cạn ASID (⇒ `generation += 1`) từ lúc mở `Machine`.
    pub exhaustions: u64,
}

/// Loại lệnh flush TLB mà KT-38 phát ra (mô hình; KT-40 tiêu thụ).
///
/// KT-38 **chỉ** phát hai loại này: [`FlushScope::Asid`] khi teardown VSpace và
/// [`FlushScope::Generation`] khi cạn ASID (Q4). Kind `(asid, va)` cho unmap/protect là KT-40.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushScope {
    /// Flush mọi entry của **một** `(asid, generation)` trên các hart trong `harts`
    /// (teardown VSpace).
    Asid,
    /// Flush **toàn bộ** entry của mọi thế hệ cũ trên các hart trong `harts` (cạn ASID) — phát
    /// **một lần** cho mỗi lần tăng thế hệ, không flush từng ASID.
    Generation,
}

impl FlushScope {
    /// Tên để in log/assert.
    pub const fn name(self) -> &'static str {
        match self {
            FlushScope::Asid => "Asid",
            FlushScope::Generation => "Generation",
        }
    }
}

/// Một lệnh flush TLB của mô hình, ghi vào [`crate::kobj::Machine::flush_log`].
///
/// `seq` bắt đầu từ 1 và tăng đơn điệu (bằng `flush_log.len()`); `harts` là HartMask mục tiêu
/// (**đã lọc hart online** — hart offline không bao giờ nhận lệnh).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlushCmd {
    /// Số thứ tự lệnh (1-based, đơn điệu).
    pub seq: u64,
    /// Loại flush.
    pub scope: FlushScope,
    /// Thế hệ mà lệnh áp dụng (thế hệ của VSpace bị teardown, hoặc thế hệ **mới** khi cạn ASID).
    pub generation: u64,
    /// ASID bị flush — `Some` với [`FlushScope::Asid`], `None` với `Generation`.
    pub asid: Option<u16>,
    /// Tập hart phải flush (HartMask, `⊆` hart online tại thời điểm phát).
    pub harts: HartMask,
}

/// Free-list **đầy đủ** của một thế hệ mới: `255, 254, …, 0` — [`pop`](Vec::pop) trả ASID **thấp
/// nhất** trước, nên chuỗi cấp trong một thế hệ là `0, 1, 2, …` (tất định, dễ đọc trong log).
pub fn fresh_asid_free() -> Vec<u16> {
    (0..ASID_COUNT as u16).rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hart_mask_bit_contains_subset() {
        let m = HartMask::EMPTY;
        assert!(m.is_empty());
        assert_eq!(m.bits(), 0);
        assert!(!m.contains(0));
        let m = m.insert(0).insert(2);
        assert_eq!(m.bits(), 0b101);
        assert!(m.contains(0) && m.contains(2) && !m.contains(1));
        assert_eq!(m.count(), 2);
        let m = m.remove(0);
        assert_eq!(m.bits(), 0b100);
        assert_eq!(HartMask::for_harts(4).bits(), 0b1111);
        assert_eq!(HartMask::for_harts(64).bits(), u64::MAX);
        assert_eq!(HartMask::for_harts(0).bits(), 0);
        assert!(HartMask::from_bits(0b011).is_subset_of(HartMask::from_bits(0b111)));
        assert!(!HartMask::from_bits(0b1000).is_subset_of(HartMask::from_bits(0b111)));
        assert_eq!(
            HartMask::from_bits(0b1010) | HartMask::from_bits(0b0101),
            HartMask::from_bits(0b1111)
        );
        assert_eq!(
            HartMask::from_bits(0b1010) & HartMask::from_bits(0b0110),
            HartMask::from_bits(0b0010)
        );
        // Bit ngoài 64 không panic, chỉ là không có hart.
        assert!(!HartMask::from_bits(1).contains(64));
        assert_eq!(HartMask::EMPTY.insert(64).bits(), 0);
    }

    #[test]
    fn asid_count_bang_hai_mu_asid_bits() {
        assert_eq!(ASID_COUNT, 1 << ASID_BITS);
        assert_eq!(ASID_COUNT, 256);
        let free = fresh_asid_free();
        assert_eq!(free.len(), ASID_COUNT);
        assert_eq!(free.first(), Some(&255), "phần tử cuối (pop) là 0");
        assert_eq!(free.last(), Some(&0));
        let mut f = fresh_asid_free();
        assert_eq!(f.pop(), Some(0));
        assert_eq!(f.pop(), Some(1));
        assert_eq!(f.len(), ASID_COUNT - 2);
    }

    #[test]
    fn vspace_info_va_flush_cmd_de_so_sanh() {
        let info = VSpaceInfo {
            root: 7,
            asid: 3,
            generation: 2,
            active_on: HartMask::from_bits(0b01),
        };
        assert_eq!(info, info);
        let cmd = FlushCmd {
            seq: 1,
            scope: FlushScope::Generation,
            generation: 3,
            asid: None,
            harts: HartMask::from_bits(0b11),
        };
        assert_eq!(cmd.scope.name(), "Generation");
        assert_eq!(cmd.asid, None);
        let asid_cmd = FlushCmd {
            asid: Some(5),
            scope: FlushScope::Asid,
            ..cmd
        };
        assert_eq!(asid_cmd.scope.name(), "Asid");
        assert_eq!(asid_cmd.asid, Some(5));
        assert_eq!(
            AsidState::default(),
            AsidState {
                generation: 0,
                free: 0,
                issued: 0,
                exhaustions: 0
            }
        );
    }
}
