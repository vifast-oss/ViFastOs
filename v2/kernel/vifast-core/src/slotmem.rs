// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Byte slot CNode nằm trong arena** (`no_std`, không `alloc`) — KT-64 thay `SlotPool` của
//! KT-58..63.
//!
//! # Vì sao (nợ §6.1/§9.6/§12.8)
//!
//! KT-58..63 giữ `CapId` của mỗi slot trong một **pool `.bss` riêng** (`SlotPool`, bump +
//! free-list). Byte slot vẫn được Untyped kế toán (`cnode_base + slots*48`) nhưng **không** có
//! byte thật nào ở đó ⇒ **hai nguồn** cho cùng một cap: pool (nơi giữ `CapId`) và arena (nơi
//! kế toán byte). KT-64 bỏ pool: `CapId` của slot được lưu **trong byte slot của arena**, tại
//! đúng `offset + cnode_base` mà `Retype` đã cấp.
//!
//! # Một nguồn sự thật = `Region`
//!
//! Không còn allocator thứ hai: [`SlotMem`] chỉ là **cửa sổ backing** của byte arena. Chỉ số
//! word của một slot là **hàm của `(Region, offset)`** — **không** chỉ `offset`:
//!
//! ```text
//! word(region, offset, slot) = region * REGION_SLOT_WORDS
//!                            + (offset + CNODE_BASE_BYTES) / SLOT_BYTES
//!                            + slot
//! ```
//!
//! `REGION_SLOT_WORDS = ARENA_SLOT_WORDS / MAX_UNTYPEDS` — mỗi Untyped có **dải word riêng, chia
//! đều và cố định**. Đây là điểm sửa **F1 của review KT-64 (D2-92a)**: nếu chỉ dùng `offset` thì
//! hai Untyped khác nhau (đều bắt đầu `cursor = 0`) sẽ có **cùng** chỉ số word ⇒ cap của CNode
//! này ghi đè lên CNode kia, **im lặng** (đọc ra cap của CSpace khác = sai quyền). Dải-theo-
//! Region làm hai miền word **rời nhau** nên không bao giờ alias.
//!
//! Trong **cùng** một Region, hai CNode liên tiếp **không chồng** cửa sổ: block `i+1` bắt đầu
//! sau `64 + 48*s` byte (và căn 64) nên chỉ số đầu của nó `>= index_i + s + 1` (chứng minh:
//! `(o+64+48s+64)/48 = (o+128)/48 + s >= (o+64)/48 + 1 + s` với `o` bội 64). Test
//! `kt64_slot_hai_cnode_khong_chong` + `kt64_slot_hai_untyped_cung_offset_khong_alias` pin.
//!
//! Cửa sổ word chia **hai vùng rời**:
//!
//! - **Arena** (Untyped): [`ARENA_SLOT_WORDS`] word đầu, chia **đều** cho [`crate::machine::MAX_UNTYPEDS`]
//!   Region (`REGION_SLOT_WORDS` mỗi Region). CNode vượt dải của Region mình ⇒ `Retype` trả
//!   `NotEnoughMemory` **trước** khi cấp (nguyên tử), không đọc/ghi ngoài mảng.
//! - **Harness** (CNode do kernel cấp như `add_cnode` của sim — không có byte arena): cùng công
//!   thức nhưng ánh xạ **lùi từ cuối** mảng (`MAX_SLOT_WORDS - 1 - index - slots`), nên hai vùng
//!   không bao giờ gặp nhau. CNode harness **không** tiêu byte arena (đúng như sim: `add_cnode`
//!   không trừ `untyped_used`).
//!
//! Không free-list: byte slot trả lại **cùng** byte arena khi `Revoke` Untyped (`Region::reset`);
//! CNode mới tại cùng offset dùng lại đúng cửa sổ word và cửa sổ được **xoá** lúc tạo.

use crate::spec::{CNODE_BASE_BYTES, SLOT_BYTES};

/// Số word slot dành cho CNode **harness** ở **cuối** mảng (vùng B).
///
/// `[TK]`: đủ cho CNode gốc init + CNode test (ca kiểm F1 của KT-63 dựng 6 CNode × 96 slot +
/// 1 CNode × 64 slot ⇒ ~650 slot; chừa biên cho TAP). CNode từ Untyped vẫn là đường chính
/// (arena) ⇒ 1024 word là biên triển khai, không phải ABI.
pub const HARNESS_SLOT_WORDS: usize = 1024;

/// Số word slot của vùng **arena** (vùng A) = `MAX_SLOT_WORDS - HARNESS_SLOT_WORDS`.
pub const ARENA_SLOT_WORDS: usize = crate::MAX_SLOT_WORDS - HARNESS_SLOT_WORDS;

/// Số word slot của **một Region** trong vùng A: `ARENA_SLOT_WORDS / MAX_UNTYPEDS` — mỗi Untyped
/// có **dải word riêng**, chia đều và cố định, nên hai Region **không bao giờ** chồng byte slot.
///
/// `[TK]`: 3072 word / 16 = **192 word/Region** (≈ 190 slot, `offset` arena < 9.152 B). Đây là
/// biên triển khai, **không** phải ABI: CNode vượt dải ⇒ `NotEnoughMemory`.
pub const REGION_SLOT_WORDS: usize = ARENA_SLOT_WORDS / crate::machine::MAX_UNTYPEDS;

const _: () = assert!(
    REGION_SLOT_WORDS * crate::machine::MAX_UNTYPEDS == ARENA_SLOT_WORDS,
    "vùng A phải chia hết cho MAX_UNTYPEDS"
);

/// Chỉ số word đầu của cửa sổ slot cho CNode trong **arena** của `region` tại `offset` — `None`
/// nếu vượt dải word của Region đó hoặc tràn `usize`.
///
/// Cửa sổ là **hàm của `(region, offset)`**, không phải chỉ `offset`: hai Untyped khác nhau cùng
/// `offset` (đều bắt đầu ở 0) có dải word khác nhau ⇒ **không** alias byte slot (F1 review /
/// D2-92a).
pub(crate) fn arena_slot_base(region: usize, offset: usize, slots: usize) -> Option<usize> {
    if region >= crate::machine::MAX_UNTYPEDS {
        return None;
    }
    let local = offset.checked_add(CNODE_BASE_BYTES)? / SLOT_BYTES;
    if local == 0 || local.checked_add(slots)? > REGION_SLOT_WORDS {
        return None;
    }
    region.checked_mul(REGION_SLOT_WORDS)?.checked_add(local)
}

/// Chỉ số word đầu của cửa sổ slot cho CNode **harness** tại `offset` (byte, arena harness) —
/// `None` nếu vượt vùng B.
///
/// Cửa sổ là `[base, base + slots)` với `base = MAX_SLOT_WORDS - index - slots`; vì
/// `index + slots ≤ HARNESS_SLOT_WORDS = 1024` nên `base ≥ ARENA_SLOT_WORDS = 3072` — **không**
/// bao giờ trùng word cuối vùng A (Region 15, local 191). Biên đầy **đúng** 1024 word cho
/// `base = 3072` (word đầu vùng B), không trả 3071; vượt ⇒ `None` (fail-closed).
pub(crate) fn harness_slot_base(offset: usize, slots: usize) -> Option<usize> {
    let index = offset.checked_add(CNODE_BASE_BYTES)? / SLOT_BYTES;
    if slots == 0 || index.checked_add(slots)? > HARNESS_SLOT_WORDS {
        return None;
    }
    let base = crate::MAX_SLOT_WORDS
        .checked_sub(index)?
        .checked_sub(slots)?;
    (base >= ARENA_SLOT_WORDS).then_some(base)
}

/// Backing store của byte slot — **chỉ** là cửa sổ của arena, không cấp phát.
#[derive(Debug, Clone, Copy)]
pub struct SlotMem {
    words: [u32; crate::MAX_SLOT_WORDS],
}

impl SlotMem {
    /// Cửa sổ rỗng (không byte slot nào được cấp).
    pub(crate) const EMPTY: Self = Self {
        words: [0u32; crate::MAX_SLOT_WORDS],
    };

    /// Đọc word thứ `i` của cửa sổ bắt đầu `base` (ngoài dải ⇒ 0, fail-closed).
    pub(crate) fn get(&self, base: usize, i: usize) -> u32 {
        self.words.get(base + i).copied().unwrap_or(0)
    }

    /// Ghi word thứ `i` của cửa sổ bắt đầu `base` (ngoài dải ⇒ bỏ, không panic).
    pub(crate) fn set(&mut self, base: usize, i: usize, value: u32) {
        if let Some(w) = self.words.get_mut(base + i) {
            *w = value;
        }
    }

    /// Xoá `n` word của cửa sổ (CNode mới dùng lại cửa sổ / CNode chết).
    pub(crate) fn fill_zero(&mut self, base: usize, n: usize) {
        let end = base.saturating_add(n).min(crate::MAX_SLOT_WORDS);
        let start = base.min(end);
        for w in &mut self.words[start..end] {
            *w = 0;
        }
    }

    /// Số word của cửa sổ (cùng nghĩa `slot_len` của metadata; chẩn đoán/test).
    #[cfg(test)]
    pub(crate) fn window_words(&self) -> usize {
        self.words.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cua_so_arena_va_harness_roi_nhau_va_tang_dan() {
        // Cùng Region: CNode 4 slot tại offset 0 và 256 (không chồng).
        let a = arena_slot_base(0, 0, 4).expect("vùng A");
        let b = arena_slot_base(0, 256, 4).expect("vùng A");
        assert!(a + 4 <= b, "không chồng: {a}+4 vs {b}");
        // **F1 (D2-92a)**: hai Region khác nhau, cùng offset 0 ⇒ word **khác nhau** (không alias).
        let r0 = arena_slot_base(0, 0, 4).expect("region 0");
        let r1 = arena_slot_base(1, 0, 4).expect("region 1");
        assert_eq!(r0, 1);
        assert_eq!(r1, REGION_SLOT_WORDS + 1);
        assert!(r0 + 4 <= r1 || r1 + 4 <= r0, "hai region rời nhau");
        // Dải word của Region có hạn: vượt ⇒ None (không đọc/ghi ngoài mảng).
        assert!(
            arena_slot_base(0, REGION_SLOT_WORDS * SLOT_BYTES, 4).is_none(),
            "vượt dải word Region ⇒ None"
        );
        // Harness: ánh xạ lùi, không chạm vùng A.
        let h0 = harness_slot_base(0, 8).expect("vùng B");
        assert!(h0 >= ARENA_SLOT_WORDS, "harness nằm trên vùng A: {h0}");
        let h1 = harness_slot_base(1024, 8).expect("vùng B");
        assert!(h1 + 8 < h0, "harness lùi dần: {h1}+8 < {h0}");
        assert!(harness_slot_base(HARNESS_SLOT_WORDS * SLOT_BYTES, 4).is_none());
        // **Biên (review KT-64b)**: harness đầy **đúng** 1024 word ⇒ base = 3072 (word đầu vùng B),
        // KHÔNG trả 3071 (word cuối vùng A của Region 15); vượt 1 word ⇒ None (fail-closed).
        let full = 1023 * SLOT_BYTES - CNODE_BASE_BYTES; // index = 1023
        let b = harness_slot_base(full, 1).expect("đầy đúng 1024 word");
        assert_eq!(b, ARENA_SLOT_WORDS, "không trùng word cuối vùng A");
        assert!(b < crate::MAX_SLOT_WORDS, "cửa sổ nằm trong mảng");
        assert!(
            harness_slot_base(full + SLOT_BYTES, 1).is_none(),
            "vượt harness ⇒ None"
        );
        assert!(harness_slot_base(0, 0).is_none(), "slot 0 ⇒ None");
    }

    #[test]
    fn slot_mem_ghi_doc_va_xoa_cua_so() {
        let mut s = SlotMem::EMPTY;
        s.set(10, 0, 7);
        s.set(10, 3, 9);
        assert_eq!(s.get(10, 0), 7);
        assert_eq!(s.get(10, 3), 9);
        assert_eq!(s.get(10, 4), 0);
        s.fill_zero(10, 4);
        assert_eq!(s.get(10, 0), 0);
        assert_eq!(s.window_words(), crate::MAX_SLOT_WORDS);
    }
}
