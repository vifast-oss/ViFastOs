// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-420** (D2-262) — **nguồn cấp trang** (`PageSource`) + arena trang cho allocator của
//! `vifast-rt`.
//!
//! # Vì sao tách khỏi `bump.rs`
//!
//! `bump.rs` chỉ là **số học offset trên một vùng đệm đã có**; nó không nói vùng đệm **đến từ
//! đâu**. Hôm nay nó là một `static [u8; 8192]` trong `.bss` của chương trình; `KT-420b` sẽ thay
//! bằng trang thật cấp từ cap `Frame`/`Untyped` (U-mode map rồi cấp cho allocator). Tách **nguồn**
//! (trait [`PageSource`]) khỏi **cách chia** (arena) là điều kiện để đổi nguồn mà **không** đụng
//! `GlobalAlloc` (phần `unsafe` duy nhất, ở `v2/user/runtime/lib.rs`).
//!
//! | Thành phần | Vai trò | `unsafe` |
//! |---|---|---|
//! | [`PageSource`] | cấp **một trang** (trả VA) — `StaticPages` hôm nay, `Frame`/`Untyped` ở `KT-420b` | không |
//! | [`StaticPages`] | nguồn trên **arena tĩnh** của chương trình (giữ nguyên `.bss`, 0 đổi byte ảnh) | không |
//! | [`PageArena`] | chia trang thành khối căn lề, **không** cấp chồng lấn, hết chỗ ⇒ `None` (fail-closed) | không |
//!
//! # Vì sao trả **địa chỉ tuyệt đối** (VA) chứ không phải `&mut [u8]`
//!
//! Cấp phát cần con trỏ để `GlobalAlloc` trả cho `alloc::alloc`; chỉ phần `unsafe` đó được phép
//! nằm trong danh sách đóng D2-17 (`v2/user/runtime/lib.rs`, xem `D2-253`). Ở đây mọi thứ là **số
//! học trên `usize`** — kiểm được trên host, không `unsafe`, không con trỏ.
//!
//! # Hợp đồng của [`PageSource`]
//!
//! * `alloc_page()` trả VA của **một** trang **chưa từng cấp** (không aliasing), hoặc `None` khi
//!   cạn — **không** panic, không trả trang đã cấp.
//! * VA `0` là **giá trị dành riêng** nghĩa "chưa có trang" (kernel không map VA 0) ⇒ nguồn
//!   **không** được trả `0`.
//! * `pages_total()` là **trần** số trang nguồn có thể cấp (dùng cho `capacity`, không cần chính
//!   xác tuyệt đối với nguồn động — `KT-420b` sẽ trả số trang đã map).
//!
//! # Giới hạn có chủ ý (ghi thẳng, không tô hồng)
//!
//! Một lần cấp **không vượt một trang** (`PAGE_SIZE`); cấp lớn hơn ⇒ `None` và caller fail-closed
//! (`# u1a: alloc fail …`). `KT-420b` mở rộng khi có nguồn cấp nhiều trang liền (Frame lớn/
//! `Untyped`): thêm `alloc_pages(n)` vào trait, không đổi `PageArena` cho trường hợp một trang.

/// Cỡ một trang — 4 KiB, đúng cỡ trang của cả 3 arch (Sv39/4-level 4 KiB).
pub const PAGE_SIZE: usize = 4096;

/// Nguồn cấp **trang** cho allocator. Xem hợp đồng ở đầu tệp.
pub trait PageSource {
    /// Cấp một trang: VA tuyệt đối của trang **chưa cấp**, hoặc `None` khi cạn.
    ///
    /// **Không** được trả `0` (giá trị dành riêng "chưa có trang").
    fn alloc_page(&mut self) -> Option<usize>;

    /// Trần số trang nguồn có thể cấp (cho `capacity`/`heap_stats`).
    fn pages_total(&self) -> usize;
}

/// Nguồn trang trên **arena tĩnh** của chương trình — giữ nguyên 8 KiB `.bss` của PoC U1a.
///
/// `base` là địa chỉ đầu arena (lấy **an toàn** bằng `UnsafeCell::get` ở `lib.rs`, không `unsafe`
/// ở đây); các trang là `base + i * page_size`. `issued` là số trang đã cấp — nằm trong nguồn chứ
/// không trong arena, nhờ vậy arena **không** phải giữ trạng thái toàn cục.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticPages {
    base: usize,
    page_size: usize,
    max_pages: usize,
    issued: usize,
}

impl StaticPages {
    /// Nguồn mới trên `max_pages` trang bắt đầu từ `base`.
    pub const fn new(base: usize, page_size: usize, max_pages: usize) -> Self {
        Self {
            base,
            page_size,
            max_pages,
            issued: 0,
        }
    }

    /// Dựng lại từ `issued` (trạng thái nằm trong `AtomicUsize` của `GlobalAlloc`).
    ///
    /// `None` ⇔ `page_size = 0` hoặc `issued > max_pages` (trạng thái hỏng ⇒ caller fail-closed).
    pub const fn restore(
        base: usize,
        page_size: usize,
        max_pages: usize,
        issued: usize,
    ) -> Option<Self> {
        if page_size == 0 || issued > max_pages {
            None
        } else {
            Some(Self {
                base,
                page_size,
                max_pages,
                issued,
            })
        }
    }

    /// Số trang đã cấp.
    pub const fn issued(&self) -> usize {
        self.issued
    }

    /// Cỡ trang của nguồn.
    pub const fn page_size(&self) -> usize {
        self.page_size
    }
}

impl PageSource for StaticPages {
    fn alloc_page(&mut self) -> Option<usize> {
        if self.issued >= self.max_pages {
            return None;
        }
        let offset = self.issued.checked_mul(self.page_size)?;
        let va = self.base.checked_add(offset)?;
        if va == 0 {
            return None;
        }
        self.issued += 1;
        Some(va)
    }

    fn pages_total(&self) -> usize {
        self.max_pages
    }
}

/// Trạng thái **tuần tự hoá được** của [`PageArena`] — `GlobalAlloc` giữ nó trong `AtomicUsize`
/// (không `static mut`, không `unsafe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaState {
    /// VA trang hiện tại; `0` = chưa cấp trang nào.
    pub page: usize,
    /// Byte đã dùng trong trang hiện tại.
    pub cursor: usize,
    /// Số trang đã lấy từ nguồn.
    pub pages: usize,
    /// Tổng byte **không còn dùng được** (đã cấp + phần đuôi bỏ lại khi sang trang).
    pub used: usize,
}

impl ArenaState {
    /// Trạng thái rỗng (chưa cấp gì).
    pub const EMPTY: Self = Self {
        page: 0,
        cursor: 0,
        pages: 0,
        used: 0,
    };
}

/// Arena trang: chia mỗi trang thành các khối **căn `align` tuyệt đối**, không cấp chồng lấn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageArena {
    page_size: usize,
    state: ArenaState,
}

impl PageArena {
    /// Arena rỗng trên trang cỡ `page_size`.
    pub const fn new(page_size: usize) -> Self {
        Self {
            page_size,
            state: ArenaState::EMPTY,
        }
    }

    /// Dựng lại từ trạng thái đã lưu; `None` ⇔ trạng thái hỏng (fail-closed, không cắt bớt).
    ///
    /// Hỏng = `page_size = 0`, `cursor > page_size`, `page = 0` mà `pages > 0` (hoặc ngược lại),
    /// hoặc `used > pages * page_size` (tràn số cũng là hỏng).
    pub const fn restore(page_size: usize, state: ArenaState) -> Option<Self> {
        if page_size == 0 {
            return None;
        }
        if state.cursor > page_size {
            return None;
        }
        if (state.page == 0) != (state.pages == 0) {
            return None;
        }
        let capacity = match state.pages.checked_mul(page_size) {
            Some(capacity) => capacity,
            None => return None,
        };
        if state.used > capacity {
            return None;
        }
        Some(Self { page_size, state })
    }

    /// Trạng thái hiện tại (để `GlobalAlloc` ghi lại vào atomic).
    pub const fn state(&self) -> ArenaState {
        self.state
    }

    /// Cỡ trang.
    pub const fn page_size(&self) -> usize {
        self.page_size
    }

    /// Tổng byte **còn dùng được** nếu nguồn còn `pages_left` trang nữa.
    ///
    /// = byte còn trong trang hiện tại + `pages_left * page_size`; `saturating` để không tràn.
    pub fn capacity(&self, pages_left: usize) -> usize {
        let current = if self.state.page == 0 {
            0
        } else {
            self.page_size - self.state.cursor
        };
        current.saturating_add(pages_left.saturating_mul(self.page_size))
    }

    /// Cấp `size` byte căn `align` (luỹ thừa 2), lấy trang mới từ `source` khi cần.
    ///
    /// Trả **VA tuyệt đối** của khối, hoặc `None` khi:
    /// * `align` không phải luỹ thừa 2 / bằng 0;
    /// * `size` lớn hơn **một trang** (giới hạn có chủ ý, xem đầu tệp);
    /// * nguồn cạn trang.
    ///
    /// Khi `None`, trạng thái **không đổi** (không tiêu trang, không tiêu byte) — caller fail-closed.
    pub fn alloc<S: PageSource>(
        &mut self,
        size: usize,
        align: usize,
        source: &mut S,
    ) -> Option<usize> {
        if align == 0 || !align.is_power_of_two() || size > self.page_size {
            return None;
        }
        if self.state.page != 0 {
            if let Some(va) = self.fit(size, align) {
                return Some(va);
            }
        }
        // Trang mới: phần đuôi trang cũ **không** dùng lại được (bump) ⇒ tính vào `used` cho
        // trung thực (`used + còn-trống = pages * page_size`).
        let next = source.alloc_page()?;
        if self.state.page != 0 {
            self.state.used = self
                .state
                .used
                .saturating_add(self.page_size - self.state.cursor);
        }
        self.state.page = next;
        self.state.cursor = 0;
        self.state.pages += 1;
        self.fit(size, align)
    }

    /// Thử cấp trong **trang hiện tại** (không lấy trang mới). `None` ⇒ không vừa.
    fn fit(&mut self, size: usize, align: usize) -> Option<usize> {
        let base = self.state.page.checked_add(self.state.cursor)?;
        // `align` là luỹ thừa 2 ⇒ `base % align` < `align`; pad để `base + pad` chia hết `align`.
        let pad = match align.checked_sub(base % align) {
            Some(pad) if pad != align => pad,
            _ => 0,
        };
        let end = pad.checked_add(size)?;
        let left = self.page_size - self.state.cursor;
        if end > left {
            return None;
        }
        self.state.cursor += end;
        self.state.used += end;
        base.checked_add(pad)
    }
}

#[cfg(test)]
mod tests {
    use super::{ArenaState, PageArena, PageSource, StaticPages, PAGE_SIZE};

    /// Nguồn thử tuỳ ý: trang tại `base + i * PAGE_SIZE`, tối đa `max`.
    fn source(base: usize, max: usize) -> StaticPages {
        StaticPages::new(base, PAGE_SIZE, max)
    }

    #[test]
    fn trang_dau_can_le_tuyet_doi() {
        // base lệch 8 ⇒ cấp 16 B căn 16 phải nhảy lên mốc chia hết 16, không phải offset 0.
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x1008, 2);
        assert_eq!(arena.alloc(16, 16, &mut src), Some(0x1010));
        let state = arena.state();
        assert_eq!(state.pages, 1);
        assert_eq!(state.cursor, 16 + 8);
        assert_eq!(state.used, 16 + 8);
        // Lần hai nối tiếp, vẫn căn 16.
        // 0x1020 đã chia hết 16 ⇒ không thêm pad; chỉ tiêu đúng 8 byte.
        assert_eq!(arena.alloc(8, 16, &mut src), Some(0x1020));
        assert_eq!(arena.state().cursor, 16 + 8 + 8);
    }

    #[test]
    fn cap_trong_cung_trang_khong_chong_lan() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x4000, 1);
        let mut last_end = 0;
        for _ in 0..8 {
            let va = arena.alloc(24, 8, &mut src).expect("còn chỗ");
            assert!(va >= last_end, "VA phải tăng, không chồng lấn");
            assert_eq!(va % 8, 0, "phải căn 8 tuyệt đối");
            last_end = va + 24;
        }
        assert_eq!(arena.state().pages, 1, "8×24 B vẫn trong một trang");
        assert!(arena.state().used <= PAGE_SIZE);
    }

    #[test]
    fn vuot_trang_thi_lay_trang_moi_va_dem_duoi_bo_lai() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x8000, 2);
        // Lấp gần đầy trang: 4000 B.
        assert_eq!(arena.alloc(4000, 1, &mut src), Some(0x8000));
        // 200 B không vừa 96 B còn lại ⇒ sang trang 2 (0x9000), tính 96 B đuôi vào `used`.
        assert_eq!(arena.alloc(200, 1, &mut src), Some(0x9000));
        let state = arena.state();
        assert_eq!(state.pages, 2);
        assert_eq!(state.cursor, 200);
        assert_eq!(state.used, 4000 + 96 + 200);
        assert_eq!(state.used + arena.capacity(0), 2 * PAGE_SIZE);
    }

    #[test]
    fn het_trang_tra_none_va_khong_doi_trang_thai() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x1000, 1);
        assert_eq!(arena.alloc(PAGE_SIZE, 1, &mut src), Some(0x1000));
        assert_eq!(src.issued(), 1);
        let before = arena.state();
        // Trang đã đầy, nguồn cạn ⇒ None, trạng thái không đổi.
        assert_eq!(arena.alloc(1, 1, &mut src), None);
        assert_eq!(arena.state(), before);
        assert_eq!(src.issued(), 1);
    }

    #[test]
    fn cap_lon_hon_trang_va_align_sai_bi_tu_choi() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x1000, 2);
        assert_eq!(arena.alloc(PAGE_SIZE + 1, 1, &mut src), None);
        assert_eq!(arena.alloc(16, 0, &mut src), None);
        assert_eq!(arena.alloc(16, 3, &mut src), None);
        assert_eq!(src.issued(), 0, "không được tiêu trang cho lời gọi sai");
        assert_eq!(arena.state(), ArenaState::EMPTY);
    }

    #[test]
    fn align_lon_co_the_can_sang_trang_moi() {
        let mut arena = PageArena::new(PAGE_SIZE);
        // Trang nguồn chia hết trang (0x2000) ⇒ khối căn 4096 vừa đúng mốc đầu trang.
        let mut src = source(0x2000, 2);
        assert_eq!(arena.alloc(4000, 1, &mut src), Some(0x2000));
        // 16 B căn 4096: pad tới 0x3000 (96 B) + 16 > 96 B còn lại ⇒ **phải** sang trang 2.
        assert_eq!(arena.alloc(16, PAGE_SIZE, &mut src), Some(0x3000));
        assert_eq!(arena.state().pages, 2);
        assert_eq!(arena.state().cursor, 16);
        assert_eq!(arena.state().used, 4000 + 96 + 16);
    }

    #[test]
    fn size_0_van_can_le_va_khong_tieu_byte_du_lieu() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x1001, 1);
        assert_eq!(arena.alloc(0, 8, &mut src), Some(0x1008));
        assert_eq!(arena.state().cursor, 7, "chỉ tiêu phần căn lề");
        assert_eq!(arena.state().used, 7);
        assert_eq!(arena.alloc(1, 1, &mut src), Some(0x1008));
    }

    #[test]
    fn restore_tu_state_va_tiep_tuc_khong_chong_lan() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x2000, 2);
        let first = arena.alloc(64, 64, &mut src).expect("cấp được");
        let snapshot = arena.state();
        // Dựng lại đúng như `GlobalAlloc` làm giữa hai lời gọi (state trong atomic).
        let mut again = PageArena::restore(PAGE_SIZE, snapshot).expect("state hợp lệ");
        let second = again.alloc(64, 64, &mut src).expect("còn chỗ");
        assert!(second >= first + 64, "khối sau không chồng khối trước");
        assert_eq!(again.state().pages, 1);
        // Nguồn cũng dựng lại được từ `issued` (trạng thái atomic thứ hai).
        let mut src2 = StaticPages::restore(0x2000, PAGE_SIZE, 2, 1).expect("issued hợp lệ");
        assert_eq!(src2.issued(), 1);
        assert_eq!(src2.alloc_page(), Some(0x3000));
    }

    #[test]
    fn restore_tu_choi_state_hong() {
        assert_eq!(PageArena::restore(0, ArenaState::EMPTY), None);
        assert_eq!(
            PageArena::restore(
                PAGE_SIZE,
                ArenaState {
                    page: 0x1000,
                    cursor: PAGE_SIZE + 1,
                    pages: 1,
                    used: 0
                }
            ),
            None
        );
        // `page = 0` mà `pages = 1` ⇒ hỏng.
        assert_eq!(
            PageArena::restore(
                PAGE_SIZE,
                ArenaState {
                    page: 0,
                    cursor: 0,
                    pages: 1,
                    used: 0
                }
            ),
            None
        );
        // `used` vượt sức chứa.
        assert_eq!(
            PageArena::restore(
                PAGE_SIZE,
                ArenaState {
                    page: 0x1000,
                    cursor: 0,
                    pages: 1,
                    used: PAGE_SIZE + 1
                }
            ),
            None
        );
        // `issued` vượt trần nguồn.
        assert_eq!(StaticPages::restore(0x1000, PAGE_SIZE, 2, 3), None);
        assert_eq!(StaticPages::restore(0x1000, 0, 2, 0), None);
    }

    #[test]
    fn bat_bien_used_khong_vuot_suc_chua() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x5000, 4);
        for index in 0..200usize {
            let size = 1 + (index % 97);
            let align = 1 << (index % 5);
            match arena.alloc(size, align, &mut src) {
                Some(va) => {
                    assert_eq!(va % align, 0, "VA phải căn đúng align yêu cầu");
                    let state = arena.state();
                    assert!(
                        state.used <= state.pages * PAGE_SIZE,
                        "used={} vượt {} trang",
                        state.used,
                        state.pages
                    );
                }
                None => break, // nguồn cạn là hợp lệ (fail-closed), không phải lỗi bất biến
            }
        }
        assert!(arena.state().pages > 0, "phải cấp được ít nhất một trang");
    }

    /// Trình tự cấp phát của PoC `u1a.elf` (`Vec<u64>` lớn dần tới 16 phần tử + `String`) phải
    /// **vừa** arena 2 trang và `used` vẫn > 0, < `2 * PAGE_SIZE` — bảo toàn dòng evidence
    /// `# u1a: heap used=…/8192` của ảnh sản phẩm (KT-347) sau khi đổi sang arena trang (KT-420).
    #[test]
    fn trinh_tu_cap_phat_cua_u1a_van_vua_arena_2_trang() {
        let mut arena = PageArena::new(PAGE_SIZE);
        let mut src = source(0x8000_0000, 2);
        // `Vec<u64>`: sức chứa 4 → 8 → 16 ⇒ 3 lần cấp (32/64/128 B).
        for elements in [4usize, 8, 16] {
            assert!(arena.alloc(elements * 8, 8, &mut src).is_some());
        }
        // `String` của `format!("alloc-string:376")` (24 B) + đệm.
        assert!(arena.alloc(24, 1, &mut src).is_some());
        let state = arena.state();
        assert_eq!(state.pages, 1, "PoC chỉ cần một trang 4 KiB");
        assert!(state.used > 0, "phải cấp thật (evidence heap used>0)");
        assert!(state.used < 2 * PAGE_SIZE, "phải dưới trần 8192");
        assert!(state.used <= PAGE_SIZE);
    }

    #[test]
    fn nguon_trang_co_tran_va_bao_dung_pages_total() {
        let mut src = source(0x1000, 3);
        assert_eq!(src.pages_total(), 3);
        assert_eq!(src.alloc_page(), Some(0x1000));
        assert_eq!(src.alloc_page(), Some(0x2000));
        assert_eq!(src.alloc_page(), Some(0x3000));
        assert_eq!(src.alloc_page(), None);
        assert_eq!(src.issued(), 3);
        // Tràn số ⇒ None (không trả VA rác).
        let mut wide = StaticPages::new(usize::MAX - 8, PAGE_SIZE, 2);
        assert_eq!(wide.alloc_page(), Some(usize::MAX - 8));
        assert_eq!(wide.alloc_page(), None, "base + 4096 tràn usize ⇒ từ chối");
    }
}
