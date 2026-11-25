// SPDX-License-Identifier: MIT OR Apache-2.0
//! Scheduler **theo hart** — KT-59 port từ `v2/sim/vifast-sim/src/sched.rs` (oracle), nhưng
//! ready queue là **bảng tĩnh** thay `VecDeque` (không `alloc`).
//!
//! Model-only `[TK]`, không phải spec/ABI. Mỗi hart một [`HartSched`]: **256 mức ưu tiên**
//! (mức `0` thấp nhất, `255` cao nhất), bitmap 4×`u64`, hàng trong cùng mức là **FIFO**
//! (pick đầu hàng, enqueue cuối hàng ⇒ round-robin khi preempt/yield).
//!
//! `pick` vẫn **O(1)** khi phần tử đầu hàng hợp lệ: tìm word bitmap cao nhất khác 0 rồi
//! `leading_zeros` trên 64 bit (tối đa 4 bước), không quét 256 hàng. Danh sách liên kết đơn
//! cố định `next: [u16; MAX_OBJECTS + 1]` thay `VecDeque` — mỗi TCB nằm tối đa một hàng.
//!
//! Bất biến (kiểm bởi [`HartSched::check_local`] và `Machine::check_sched` sau **mỗi** op):
//! `bitmap` ⇔ hàng khác rỗng; `len` khớp tổng; `idle ⇔ current == None`; `cur_prio` khớp TCB
//! đang chạy.

use crate::error::{CheckFailure, ObjId};
use crate::machine::MAX_OBJECTS;

/// Số mức ưu tiên của một hart (thiết kế §4.8: "256 mức `ilist` + bitmap").
pub const MAX_PRIORITY: usize = 256;
/// Số word `u64` của bitmap 256 bit.
pub const BITMAP_WORDS: usize = MAX_PRIORITY / 64;

/// **KT-29** — trạng thái một hart **công bố** cho các hart khác đọc (thiết kế §4.8/§4.16 luật
/// 3; D2-53 Q1/Q2). Model-only `[TK]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Published {
    /// Hart không có TCB đang chạy (`current == None`).
    pub idle: bool,
    /// Mức ưu tiên của TCB đang chạy; `0` khi rảnh.
    pub cur_prio: u8,
}

/// Ready queue + `current` của **một hart** — model-only `[TK]`.
///
/// Hàng ưu tiên `p` là danh sách liên kết đơn: `head[p]` là TCB đầu (FIFO), `tail[p]` là TCB
/// cuối; `next[tcb]` là TCB kế tiếp (0 = hết hàng). `len_at[p]` đếm phần tử mức `p`.
#[derive(Debug, Clone, Copy)]
pub struct HartSched {
    /// TCB đầu hàng của mức `p` (0 = rỗng).
    head: [u16; MAX_PRIORITY],
    /// TCB cuối hàng của mức `p` (0 = rỗng).
    tail: [u16; MAX_PRIORITY],
    /// Số TCB ở mức `p`.
    len_at: [u16; MAX_PRIORITY],
    /// Con trỏ kế tiếp trong hàng, chỉ số theo `ObjId` (0 = hết).
    next: [u16; MAX_OBJECTS + 1],
    /// Bit thứ `p` ⇔ hàng mức `p` khác rỗng.
    bitmap: [u64; BITMAP_WORDS],
    /// Tổng số TCB trong 256 hàng (không tính `current`).
    ///
    /// **KT-405** — `u16` thay `usize`: miền giá trị **chứng minh được** là `≤ MAX_OBJECTS =
    /// 256` (mỗi TCB là một object, mỗi TCB nằm tối đa một hàng) nên `u16` thừa sức. Kiểu hẹp
    /// giữ `HartSched` **2.096 B** thay 2.104 B ⇒ `PerHartState` không đệm thêm 6 B/hart và
    /// `size_of::<Machine>()` **không** vượt trần KT-312 `184.832 B` (D2-84: tối ưu, không nới
    /// pin). Ngữ nghĩa `len()`/`is_empty()`/`check_local` **không đổi**.
    len: u16,
    /// TCB đang chạy của hart (`Running{hart}`) — tối đa một.
    current: Option<ObjId>,
    /// Hart không có TCB chạy.
    idle: bool,
    /// Mức ưu tiên của `current`; `0` khi rảnh.
    cur_prio: u8,
}

impl HartSched {
    /// Scheduler rỗng của một hart.
    pub const fn new() -> Self {
        Self {
            head: [0; MAX_PRIORITY],
            tail: [0; MAX_PRIORITY],
            len_at: [0; MAX_PRIORITY],
            next: [0; MAX_OBJECTS + 1],
            bitmap: [0; BITMAP_WORDS],
            len: 0,
            current: None,
            idle: true,
            cur_prio: 0,
        }
    }

    /// Khởi tạo **tại chỗ** hàng đợi rỗng (KT-61: static `.bss` đã zero ⇒ đặt `idle = true`,
    /// phần còn lại đều là 0 như [`HartSched::new`]).
    pub(crate) fn reset(&mut self) {
        self.head = [0; MAX_PRIORITY];
        self.tail = [0; MAX_PRIORITY];
        self.len_at = [0; MAX_PRIORITY];
        self.next = [0; MAX_OBJECTS + 1];
        self.bitmap = [0; BITMAP_WORDS];
        self.len = 0;
        self.current = None;
        self.idle = true;
        self.cur_prio = 0;
    }

    /// Số TCB trong ready queue (không tính `current`).
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Hàng rỗng?
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// TCB đang chạy của hart.
    pub fn current(&self) -> Option<ObjId> {
        self.current
    }

    /// Hart rảnh (`current == None`).
    pub fn idle(&self) -> bool {
        self.idle
    }

    /// Mức ưu tiên của TCB đang chạy; `0` khi rảnh.
    pub fn cur_prio(&self) -> u8 {
        self.cur_prio
    }

    /// Số TCB ở một mức.
    pub fn len_at(&self, prio: u8) -> usize {
        self.len_at[prio as usize] as usize
    }

    /// TCB **cuối hàng** mức `prio` (push migration lấy cuối hàng — chốt Q2).
    pub fn last_at(&self, prio: u8) -> Option<ObjId> {
        match self.tail[prio as usize] {
            0 => None,
            t => Some(t as ObjId),
        }
    }

    /// Mức cao nhất đang có TCB — `None` nếu hàng rỗng. O(1) theo bitmap 4 word.
    pub fn highest_priority(&self) -> Option<u8> {
        for w in (0..BITMAP_WORDS).rev() {
            let bits = self.bitmap[w];
            if bits != 0 {
                let bit = 63 - bits.leading_zeros() as usize;
                return Some((w * 64 + bit) as u8);
            }
        }
        None
    }

    /// `pick()` O(1): TCB mức cao nhất, cùng mức FIFO (đầu hàng). **Không** gỡ khỏi hàng.
    pub fn pick(&self) -> Option<ObjId> {
        let prio = self.highest_priority()?;
        self.front_at(prio)
    }

    /// `pick` có **điều kiện** (KT-20): mức cao nhất trước, cùng mức FIFO, chỉ nhận TCB thoả
    /// `ok`. **Không** gỡ khỏi hàng.
    pub fn pick_where(&self, mut ok: impl FnMut(ObjId) -> bool) -> Option<ObjId> {
        for w in (0..BITMAP_WORDS).rev() {
            let mut bits = self.bitmap[w];
            while bits != 0 {
                let bit = 63 - bits.leading_zeros() as usize;
                bits &= !(1u64 << bit);
                let prio = w * 64 + bit;
                let mut cur = self.head[prio];
                while cur != 0 {
                    if ok(cur as ObjId) {
                        return Some(cur as ObjId);
                    }
                    cur = self.next[cur as usize];
                }
            }
        }
        None
    }

    /// TCB có nằm trong ready queue của hart này không.
    pub fn contains(&self, tcb: ObjId) -> bool {
        if tcb == 0 || tcb as usize > MAX_OBJECTS {
            return false;
        }
        for p in 0..MAX_PRIORITY {
            let mut cur = self.head[p];
            while cur != 0 {
                if cur as ObjId == tcb {
                    return true;
                }
                cur = self.next[cur as usize];
            }
        }
        false
    }

    /// Duyệt mọi TCB trong hàng theo thứ tự **mức tăng dần** rồi FIFO.
    pub fn for_each_queued(
        &self,
        mut f: impl FnMut(u8, ObjId) -> Result<(), CheckFailure>,
    ) -> Result<(), CheckFailure> {
        for w in 0..BITMAP_WORDS {
            let mut bits = self.bitmap[w];
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let prio = (w * 64 + bit) as u8;
                let mut cur = self.head[prio as usize];
                while cur != 0 {
                    f(prio, cur as ObjId)?;
                    cur = self.next[cur as usize];
                }
            }
        }
        Ok(())
    }

    /// TCB đầu hàng mức `prio`.
    fn front_at(&self, prio: u8) -> Option<ObjId> {
        match self.head[prio as usize] {
            0 => None,
            t => Some(t as ObjId),
        }
    }

    /// Đưa `tcb` vào **cuối** hàng mức `prio` (cùng mức FIFO).
    pub(crate) fn enqueue(&mut self, tcb: ObjId, prio: u8) {
        let p = prio as usize;
        debug_assert!(tcb != 0 && tcb as usize <= MAX_OBJECTS);
        self.next[tcb as usize] = 0;
        match self.tail[p] {
            0 => {
                self.head[p] = tcb as u16;
                self.tail[p] = tcb as u16;
            }
            t => {
                self.next[t as usize] = tcb as u16;
                self.tail[p] = tcb as u16;
            }
        }
        self.len_at[p] += 1;
        self.bitmap[p / 64] |= 1u64 << (p % 64);
        self.len += 1;
    }

    /// Gỡ `tcb` khỏi bucket `prio` (bất biến: bucket == priority hiện tại của TCB).
    pub(crate) fn remove_at(&mut self, tcb: ObjId, prio: u8) -> bool {
        let p = prio as usize;
        let mut prev: u16 = 0;
        let mut cur = self.head[p];
        while cur != 0 {
            if cur as ObjId == tcb {
                let nxt = self.next[cur as usize];
                if prev == 0 {
                    self.head[p] = nxt;
                } else {
                    self.next[prev as usize] = nxt;
                }
                if self.tail[p] == cur {
                    self.tail[p] = prev;
                }
                self.next[cur as usize] = 0;
                self.len_at[p] -= 1;
                self.len -= 1;
                if self.head[p] == 0 {
                    self.bitmap[p / 64] &= !(1u64 << (p % 64));
                }
                return true;
            }
            prev = cur;
            cur = self.next[cur as usize];
        }
        false
    }

    /// Gỡ `tcb` khỏi **mọi** bucket (đổi `priority`/`home` phải gỡ trước khi sửa trường).
    pub(crate) fn remove_all(&mut self, tcb: ObjId) -> usize {
        let mut removed = 0usize;
        for prio in 0..MAX_PRIORITY {
            while self.remove_at(tcb, prio as u8) {
                removed += 1;
            }
        }
        removed
    }

    /// Đặt TCB đang chạy (sau `transition_tcb` sang `Running{hart}`).
    pub(crate) fn set_current(&mut self, tcb: ObjId, prio: u8) {
        self.current = Some(tcb);
        self.cur_prio = prio;
        self.idle = false;
    }

    /// Nhả TCB đang chạy (hart về rảnh). Trả TCB cũ (nếu có).
    pub(crate) fn clear_current(&mut self) -> Option<ObjId> {
        self.cur_prio = 0;
        self.idle = true;
        self.current.take()
    }

    /// Kiểm phần **nội tại** của hart: `bitmap` ⇔ các hàng khác rỗng và `len` khớp tổng.
    pub fn check_local(&self) -> Result<(), CheckFailure> {
        let mut tong = 0usize;
        for w in 0..BITMAP_WORDS {
            let mut bits = self.bitmap[w];
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let prio = w * 64 + bit;
                let n = self.len_at[prio] as usize;
                if n == 0 || self.head[prio] == 0 {
                    return Err(CheckFailure::one("bitmap thua bit o muc", prio as u32));
                }
                // Đếm lại hàng để bắt `len_at` sai (O(số phần tử)).
                let mut dem = 0usize;
                let mut cur = self.head[prio];
                let mut steps = 0usize;
                while cur != 0 {
                    dem += 1;
                    steps += 1;
                    if steps > MAX_OBJECTS {
                        return Err(CheckFailure::one("ready queue co vong", prio as u32));
                    }
                    cur = self.next[cur as usize];
                }
                if dem != n {
                    return Err(CheckFailure::two(
                        "ready queue len_at lech",
                        prio as u32,
                        dem as u32,
                    ));
                }
                tong += n;
            }
        }
        if tong != self.len as usize {
            return Err(CheckFailure::two(
                "ready queue len lech bitmap",
                self.len as u32,
                tong as u32,
            ));
        }
        if self.idle != self.current.is_none() {
            return Err(CheckFailure::one("idle lech current", self.idle as u32));
        }
        if self.current.is_none() && self.cur_prio != 0 {
            return Err(CheckFailure::one(
                "cur_prio khac 0 khi ranh",
                self.cur_prio as u32,
            ));
        }
        Ok(())
    }
}

impl Default for HartSched {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chèn/xoá/`pick` theo mức + FIFO; bitmap phủ đủ 256 mức.
    #[test]
    fn pick_va_fifo_cung_muc() {
        let mut h = HartSched::new();
        assert_eq!(h.pick(), None);
        h.enqueue(10, 0);
        h.enqueue(11, 255);
        h.enqueue(12, 255);
        h.enqueue(13, 64);
        assert_eq!(h.pick(), Some(11));
        assert_eq!(h.highest_priority(), Some(255));
        assert_eq!(h.len(), 4);
        assert!(h.remove_at(11, 255));
        assert_eq!(h.pick(), Some(12));
        assert_eq!(h.remove_all(13), 1);
        assert!(!h.contains(13));
        h.remove_at(12, 255);
        h.remove_at(10, 0);
        assert_eq!(h.pick(), None);
        assert_eq!(h.len(), 0);
        h.check_local().expect("bat bien noi tai");
    }

    /// `pick_where` bỏ qua phần tử bị loại, giữ thứ tự ưu tiên/FIFO.
    #[test]
    fn pick_where_bo_qua_phan_tu_bi_loai() {
        let mut h = HartSched::new();
        h.enqueue(1, 5);
        h.enqueue(2, 5);
        h.enqueue(3, 9);
        h.enqueue(4, 9);
        assert_eq!(h.pick_where(|_| true), Some(3));
        assert_eq!(h.pick_where(|t| t != 3), Some(4));
        assert_eq!(h.pick_where(|t| t < 3), Some(1));
        assert_eq!(h.pick_where(|t| t == 2), Some(2));
        assert_eq!(h.pick_where(|_| false), None);
        h.check_local().expect("bat bien noi tai");
    }

    /// Đủ 256 mức: mỗi mức một TCB, bitmap/len_at/`last_at` đúng.
    #[test]
    fn bitmap_phu_du_256_muc() {
        let mut h = HartSched::new();
        for p in 0..MAX_PRIORITY {
            h.enqueue((p + 1) as ObjId, p as u8);
            assert_eq!(h.highest_priority(), Some(p as u8));
            assert_eq!(h.len_at(p as u8), 1);
            assert_eq!(h.last_at(p as u8), Some((p + 1) as ObjId));
        }
        assert_eq!(h.len(), MAX_PRIORITY);
        h.check_local().expect("bat bien noi tai");
        for p in 0..MAX_PRIORITY {
            assert_eq!(h.pick(), Some((MAX_PRIORITY - p) as ObjId));
            assert!(h.remove_at((MAX_PRIORITY - p) as ObjId, (MAX_PRIORITY - 1 - p) as u8));
        }
        assert_eq!(h.pick(), None);
        h.check_local().expect("bat bien noi tai sau khi rut het");
    }
}
