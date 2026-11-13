// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Tuỳ chọn hạ** — tách khỏi `lower.rs` cho luật R1 (≤ 1.500 dòng/tệp) ở **ST-57**.

use crate::ty::Type;

use super::TraitSig;

/// Chính sách báo `VS0301` cho **tên giá trị** chưa khai.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NamePolicy {
    /// **Mặc định ST-11 (VD-45)**: tên tự do được phép (giữ `sym = None`). Vẫn báo `VS0301` khi
    /// **gán** cho tên chưa khai — chỗ chắc chắn sai. `Strict` là mặc định từ ST-13/14.
    #[default]
    Permissive,
    /// Báo `VS0301` cho **mọi** tên giá trị chưa phân giải (mặc định từ ST-13/14 khi có prelude).
    Strict,
}

/// Tuỳ chọn hạ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerOptions {
    pub names: NamePolicy,
    /// Module của tệp đang hạ (ST-25); `0` = biên dịch một tệp.
    pub mod_id: u16,
    /// Module mà tệp này `use` (ST-25) — được phép thấy item `pub` của chúng.
    pub imports: Vec<u16>,
    /// Kiểu **công khai** nhập từ module khác (ST-25): tên → kiểu đã hạ.
    pub imported_types: Vec<(String, Type)>,
    /// **ST-57 (VD-98 (1))**: chữ ký `trait` `pub` nhập từ module khác — để `impl`/siêu trait kiểm được.
    pub trait_ngoai: Vec<TraitSig>,
    /// Module `std.*` đã `use` (ST-26) — bật dạng gọi `str.split(…)` trong tệp này.
    pub std_mods: Vec<String>,
    /// Cho phép **chương trình ngoài** (ST-27). `false` (vd `vis check --no-extern`) ⇒ lệnh lạ báo
    /// `VS0601` như trước.
    pub cho_ngoai: bool,
}

impl Default for LowerOptions {
    fn default() -> Self {
        Self {
            names: NamePolicy::Permissive,
            mod_id: 0,
            imports: Vec::new(),
            imported_types: Vec::new(),
            trait_ngoai: Vec::new(),
            std_mods: Vec::new(),
            cho_ngoai: true,
        }
    }
}

impl LowerOptions {
    /// Chế độ **chặt**: mọi tên giá trị chưa khai đều `VS0301`.
    pub fn strict() -> Self {
        Self {
            names: NamePolicy::Strict,
            ..Self::default()
        }
    }
}
