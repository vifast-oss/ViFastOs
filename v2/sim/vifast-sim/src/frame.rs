// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Frame` của K2 — **metadata thuần, không cấp byte thật** (KT-36; D2-65 Q2/Q3/Q7;
//! `v2/docs/k2-vm.md` §10; thiết kế §4.10 `:353-356`).
//!
//! **K2 không phải kernel thật**: `Frame` ở đây là một bản ghi mô hình (pa/cỡ/bộ đếm), không có
//! PTE, không có vùng nhớ 1 GiB nào được cấp, không MMU/TLB. KT-37 mới map Frame vào PageTable,
//! KT-39 mới áp luật W^X lên bộ đếm.
//!
//! # Cửa sổ PA của Untyped `[TK]`
//!
//! Mỗi Untyped sở hữu **một cửa sổ PA ảo** [`UNTYPED_PA_WINDOW`] = 1 GiB cho Frame:
//!
//! - đáy cửa sổ = tham số `base` của `Machine::add_untyped(base, size)` (đã có từ KT-09 nhưng
//!   trước KT-36 **bị bỏ qua**; nay là đáy cửa sổ PA của Frame);
//! - [`FrameArea::watermark`] tiến theo từng Frame đã cấp, **không** đệm căn lề ngầm:
//!   `pa = base + watermark`; nếu `pa` chưa căn cỡ Frame ⇒ [`KError::Alignment`] (đúng DoD
//!   KT-36 "pa lệch 1 ⇒ Alignment"). Muốn trộn cỡ thì cấp theo thứ tự **giảm dần**;
//! - vượt cửa sổ ⇒ [`KError::NotEnoughMemory`]; Frame khác **chồng** pa ⇒
//!   [`KError::AlreadyMapped`].
//!
//! Vì sao cửa sổ **ảo** mà không dùng thẳng `size` của Untyped: Frame 1 GiB phải cấp được mà
//! **không** ăn RSS (KT-36 rủi ro (a): cấm mọi test dựa trên `Vec` 1 GiB). Arena host của
//! Untyped vẫn chỉ là `size` byte và **không** bị Frame đụng tới (`untyped_used` không đổi);
//! `pa` chỉ là nhãn vật lý của mô hình, đủ để KT-37 dựng PTE và KT-39 đếm W^X.

use crate::kobj::KError;
use vifast_pte::FrameSize;

/// Cửa sổ PA ảo mà **mỗi** Untyped cấp cho Frame: **1 GiB** — `[TK]` (KT-36, `k2-vm.md` §10).
///
/// Đúng bằng cỡ Frame lớn nhất (`FrameSize::G1`) nên: một Frame 1 GiB chiếm trọn cửa sổ, còn
/// 10.000 Frame 4 KiB chỉ dùng 40 MiB. Không phải ngưỡng cổng — đổi giá trị phải có D2 mới.
pub const UNTYPED_PA_WINDOW: u64 = 1 << 30;

/// Metadata của một Frame — **đúng 6 trường DoD KT-36** (`pa`, `size_bits`, `live`,
/// `mapped_w`, `mapped_x`, `maps`), model-only `[TK]`, không phải bảng ABI.
///
/// `mapped_w`/`mapped_x` là **số mapping sống** của chính Frame này có quyền `WRITE` / thuộc
/// tính `EXEC` (thiết kế §4.10: "bộ đếm mapped_w/mapped_x, giữ từ v1"); `maps` là tổng số
/// mapping sống.
///
/// Bất biến của **KT-36**: `mapped_w ≤ maps`, `mapped_x ≤ maps`; frame chết ⇒ cả ba bằng 0.
/// Bất biến chặt hơn `mapped_w + mapped_x ≤ maps` (và `!(w > 0 && x > 0)`) chỉ đúng **sau khi**
/// KT-39 áp luật W^X — KT-36 cố ý **chưa** áp luật đó (K2-PLAN Q3: "KT-39 toàn bộ luật").
///
/// KT-36 **chưa** map Frame vào VSpace/PageTable nên ba bộ đếm chỉ đổi qua
/// [`crate::kobj::Machine::frame_note_map`]/[`crate::kobj::Machine::frame_note_unmap`]; KT-37/
/// KT-39 sẽ gọi đúng hai hàm đó từ `map`/`unmap` thật.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameMeta {
    /// Địa chỉ vật lý của mô hình = `base` của Untyped + watermark cửa sổ (căn đúng cỡ Frame).
    pub pa: u64,
    /// `12` (4 KiB) / `21` (2 MiB) / `30` (1 GiB) — cùng nguồn với `vifast-pte`.
    pub size_bits: u8,
    /// Frame còn sống (`false` sau khi Untyped sở hữu bị `Revoke`).
    pub live: bool,
    /// Tổng số mapping sống của Frame này (mọi VSpace, sau KT-37).
    pub maps: u32,
    /// Số mapping sống có quyền `WRITE`.
    pub mapped_w: u32,
    /// Số mapping sống có thuộc tính `EXEC`.
    pub mapped_x: u32,
}

impl FrameMeta {
    /// Frame sống, chưa có mapping nào.
    pub const fn new(pa: u64, size: FrameSize) -> Self {
        Self {
            pa,
            size_bits: size.size_bits(),
            live: true,
            maps: 0,
            mapped_w: 0,
            mapped_x: 0,
        }
    }

    /// Cỡ Frame theo byte; `0` nếu `size_bits` không thuộc `{12, 21, 30}` (trạng thái không
    /// hợp lệ — [`crate::kobj::Machine::check_frames`] bắt).
    pub const fn size_bytes(&self) -> u64 {
        match FrameSize::from_size_bits(self.size_bits) {
            Ok(size) => size.size_bytes(),
            Err(_) => 0,
        }
    }

    /// Không còn mapping nào (bất biến bắt buộc của frame **đã chết**).
    pub const fn counters_zero(&self) -> bool {
        self.maps == 0 && self.mapped_w == 0 && self.mapped_x == 0
    }

    /// Có ít nhất một mapping sống.
    pub const fn has_mapping(&self) -> bool {
        self.maps != 0
    }
}

/// Cửa sổ PA ảo của **một** Untyped: đáy + watermark (byte đã cấp cho Frame).
///
/// Không liên quan tới arena host của Untyped (Frame không tiêu byte thật) — xem doc module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameArea {
    /// Đáy cửa sổ PA (= `base` của `add_untyped`; Untyped con = `base_cha + offset_con`).
    pub base: u64,
    /// Byte cửa sổ đã cấp (tổng cỡ các Frame sống, theo thứ tự cấp).
    pub watermark: u64,
}

impl FrameArea {
    /// Cửa sổ mới ở `base`, chưa cấp Frame nào.
    pub const fn new(base: u64) -> Self {
        Self { base, watermark: 0 }
    }

    /// Cấp một Frame cỡ `size` tại watermark hiện tại.
    ///
    /// **Không** căn lề ngầm: `pa = base + watermark` phải đã căn đúng `size`, nếu không trả
    /// [`KError::Alignment`] và **không** đổi watermark (atomic — DoD KT-36). Cửa sổ đầy ⇒
    /// [`KError::NotEnoughMemory`]. Trả `pa` vừa cấp.
    pub fn carve(&mut self, size: FrameSize) -> Result<u64, KError> {
        let need = size.size_bytes();
        let pa = self
            .base
            .checked_add(self.watermark)
            .ok_or(KError::InvalidArgument)?;
        if pa % need != 0 {
            return Err(KError::Alignment);
        }
        let end = pa.checked_add(need).ok_or(KError::NotEnoughMemory)?;
        let win_end = self
            .base
            .checked_add(UNTYPED_PA_WINDOW)
            .ok_or(KError::InvalidArgument)?;
        if end > win_end {
            return Err(KError::NotEnoughMemory);
        }
        self.watermark += need;
        Ok(pa)
    }

    /// Đáy/đỉnh cửa sổ PA (đỉnh **không** phụ thuộc watermark) — cho `check_frames`/test.
    pub fn window(&self) -> Option<(u64, u64)> {
        self.base
            .checked_add(UNTYPED_PA_WINDOW)
            .map(|end| (self.base, end))
    }
}
