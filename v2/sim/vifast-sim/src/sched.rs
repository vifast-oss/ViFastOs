// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-19 — **scheduler theo hart** cho mô hình TCB (thiết kế §4.8
//! `docs/v2/ViFastOs-v2-thiet-ke.md:336-344`, §4.13 `:382-387`).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** K1b dựng **1 hart** (D2-49(a), push
//! migration hoãn sang K1c) nhưng cấu trúc viết theo hart: mỗi hart một [`HartSched`]
//! gồm 256 mức ưu tiên + bitmap 4×u64, `current`, `idle`, `cur_prio`.
//!
//! Bất biến (DoD KT-19, kiểm bởi `Machine::check_sched` sau **mỗi** public op):
//! - `Ready` (không `suspended`) ⇔ có trong ready queue; cùng mức ⇒ FIFO theo thứ tự vào hàng;
//! - `Running{hart}` ⇔ là `current` của hart đó, **không** nằm trong hàng; ≤ 1 `Running`/hart;
//! - mọi TCB trong hàng tồn tại, `Ready`, `!suspended`, và **bucket == priority**;
//! - `len`/`bitmap` khớp số phần tử; `idle` ⇔ `current == None`.
//!
//! `pick` là **O(1)**: tìm word bitmap cao nhất khác 0 rồi `leading_zeros` trên 64 bit —
//! tối đa 4 bước, **không** quét 256 hàng. Cùng mức lấy phần tử đầu hàng (`VecDeque::front`)
//! nên đúng FIFO; không dùng `HashMap`/thời gian thực nên quỹ đạo tất định theo seed.

use std::collections::VecDeque;

use crate::kobj::ObjId;

/// Số mức ưu tiên của một hart (thiết kế §4.8: "256 mức `ilist` + bitmap").
pub const MAX_PRIORITY: usize = 256;
/// Số word `u64` của bitmap 256 bit (`256 / 64`).
pub const BITMAP_WORDS: usize = MAX_PRIORITY / 64;

/// Trạng thái scheduler của **một hart** — model-only `[TK]`.
///
/// Mức `0` thấp nhất, `255` cao nhất (cùng quy ước seL4/thiết kế §4.8: "mức cao nhất được
/// chọn"). Bit `p` của `bitmap[p / 64]` bật ⇔ hàng mức `p` khác rỗng.
#[derive(Debug)]
pub struct HartSched {
    /// 256 ready queue, mỗi mức một `VecDeque` FIFO.
    queues: [VecDeque<ObjId>; MAX_PRIORITY],
    /// Bit thứ `p` ⇔ `queues[p]` khác rỗng (bitmap 4 word).
    bitmap: [u64; BITMAP_WORDS],
    /// Tổng số TCB trong 256 hàng (không tính `current`).
    len: usize,
    /// TCB đang chạy của hart (`Running{hart}`) — tối đa một.
    current: Option<ObjId>,
    /// Hart không có TCB chạy (`current == None`).
    idle: bool,
    /// Mức ưu tiên của `current` (đọc bởi push migration ở K1c — D2-49(a)); `0` khi rảnh.
    cur_prio: u8,
}

impl HartSched {
    pub fn new() -> Self {
        Self {
            queues: std::array::from_fn(|_| VecDeque::new()),
            bitmap: [0; BITMAP_WORDS],
            len: 0,
            current: None,
            idle: true,
            cur_prio: 0,
        }
    }

    /// Số TCB trong ready queue (không tính `current`).
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// TCB đang chạy của hart (bất biến: state của nó là `Running{hart}`).
    pub fn current(&self) -> Option<ObjId> {
        self.current
    }

    /// Hart rảnh (`current == None`).
    pub fn idle(&self) -> bool {
        self.idle
    }

    /// Mức ưu tiên của TCB đang chạy; `0` khi hart rảnh.
    pub fn cur_prio(&self) -> u8 {
        self.cur_prio
    }

    /// Số TCB ở một mức (dùng cho test/bất biến "bucket khớp priority").
    pub fn len_at(&self, prio: u8) -> usize {
        self.queues[prio as usize].len()
    }

    /// Mức cao nhất đang có TCB — `None` nếu hàng rỗng.
    ///
    /// O(1): duyệt tối đa [`BITMAP_WORDS`] word từ cao xuống, tìm bit cao nhất mỗi word
    /// bằng `leading_zeros`; không quét 256 hàng.
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
        self.queues[prio as usize].front().copied()
    }

    /// `pick` có **điều kiện** (KT-20): mức cao nhất trước, cùng mức FIFO, nhưng chỉ nhận TCB
    /// thoả `ok`. Duyệt từng bit đang bật của bitmap (tối đa 4 word, mỗi word lấy bit cao nhất
    /// trước) nên **vẫn O(1) khi phần tử đầu hàng thoả**; chỉ khi TCB đầu hàng bị loại (ví dụ
    /// `Ready` nhưng `SchedContext` đã cạn budget) mới quét tiếp trong hàng/các mức thấp hơn.
    /// **Không** gỡ khỏi hàng.
    pub fn pick_where(&self, mut ok: impl FnMut(ObjId) -> bool) -> Option<ObjId> {
        for w in (0..BITMAP_WORDS).rev() {
            let mut bits = self.bitmap[w];
            while bits != 0 {
                let bit = 63 - bits.leading_zeros() as usize;
                bits &= !(1u64 << bit);
                let prio = w * 64 + bit;
                for &t in self.queues[prio].iter() {
                    if ok(t) {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    /// TCB có nằm trong ready queue của hart này không (dùng cho `check_sched`/test).
    pub fn contains(&self, tcb: ObjId) -> bool {
        self.queues.iter().any(|q| q.iter().any(|t| *t == tcb))
    }

    /// Duyệt mọi TCB trong hàng theo thứ tự **mức tăng dần** rồi FIFO — nguồn cho
    /// `check_sched`. Chỉ đi qua các bit **đang bật** của bitmap (4 word) nên chi phí
    /// O(số hàng khác rỗng + số TCB), không quét 256 `VecDeque` mỗi lần kiểm.
    pub fn for_each_queued(
        &self,
        mut f: impl FnMut(u8, ObjId) -> Result<(), String>,
    ) -> Result<(), String> {
        for w in 0..BITMAP_WORDS {
            let mut bits = self.bitmap[w];
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let prio = (w * 64 + bit) as u8;
                for &t in self.queues[prio as usize].iter() {
                    f(prio, t)?;
                }
            }
        }
        Ok(())
    }

    /// Đưa `tcb` vào **cuối** hàng mức `prio` (cùng mức FIFO). TCB phải chưa có trong hàng.
    pub(crate) fn enqueue(&mut self, tcb: ObjId, prio: u8) {
        // Không kiểm trùng ở đây: `sched_sync` gỡ bản cũ trước khi gọi, còn
        // `Machine::check_sched` (sau **mỗi** public op) kiểm trùng bằng bảng O(1) — tránh
        // quét 256 hàng mỗi lần enqueue (đo được ~vài µs/hart/op trong debug).
        self.queues[prio as usize].push_back(tcb);
        self.bitmap[prio as usize / 64] |= 1u64 << (prio % 64);
        self.len += 1;
    }

    /// Gỡ `tcb` khỏi bucket `prio` (bất biến: bucket == `priority` hiện tại của TCB).
    /// Trả `true` nếu có gỡ.
    pub(crate) fn remove_at(&mut self, tcb: ObjId, prio: u8) -> bool {
        let q = &mut self.queues[prio as usize];
        let Some(pos) = q.iter().position(|t| *t == tcb) else {
            return false;
        };
        q.remove(pos);
        self.len -= 1;
        if q.is_empty() {
            self.bitmap[prio as usize / 64] &= !(1u64 << (prio % 64));
        }
        true
    }

    /// Gỡ `tcb` khỏi **mọi** bucket — chỉ dùng khi đổi `priority`/`home` (bucket phải khớp
    /// priority nên phải gỡ **trước** khi sửa trường). Trả số bản đã gỡ.
    pub(crate) fn remove_all(&mut self, tcb: ObjId) -> usize {
        let mut removed = 0;
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
    ///
    /// Chỉ đi qua các bit **đang bật** (O(số hàng khác rỗng)): bit bật mà hàng rỗng ⇒ thừa;
    /// tổng phần tử của các hàng có bit < `len` ⇒ thiếu bit. Phần liên kết TCB ↔ state và
    /// chống trùng do `Machine::check_sched` kiểm bằng bảng đánh dấu O(1).
    pub fn check_local(&self) -> Result<(), String> {
        let mut tong = 0usize;
        for w in 0..BITMAP_WORDS {
            let mut bits = self.bitmap[w];
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let prio = w * 64 + bit;
                let n = self.queues[prio].len();
                if n == 0 {
                    return Err(format!("bitmap thừa bit ở mức {prio}"));
                }
                tong += n;
            }
        }
        if tong != self.len {
            return Err(format!(
                "len={} nhưng bitmap chỉ phủ {tong} TCB (thiếu bit?)",
                self.len
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

    #[test]
    fn pick_where_bo_qua_phan_tu_bi_loai_giu_thu_tu_uu_tien_fifo() {
        let mut h = HartSched::new();
        h.enqueue(1, 5);
        h.enqueue(2, 5);
        h.enqueue(3, 9);
        h.enqueue(4, 9);
        // Tất cả thoả ⇒ y như `pick` (mức cao nhất, FIFO).
        assert_eq!(h.pick_where(|_| true), Some(3));
        // Loại phần tử đầu mức cao nhất ⇒ phần tử kế tiếp cùng mức.
        assert_eq!(h.pick_where(|t| t != 3), Some(4));
        // Loại cả mức 9 ⇒ xuống mức 5, FIFO.
        assert_eq!(h.pick_where(|t| t < 3), Some(1));
        assert_eq!(h.pick_where(|t| t == 2), Some(2));
        // Không ai thoả ⇒ None, hàng không đổi.
        assert_eq!(h.pick_where(|_| false), None);
        assert_eq!(h.len(), 4);
        h.check_local().expect("bất biến nội tại");
    }

    #[test]
    fn pick_lay_muc_cao_nhat_va_fifo_cung_muc() {
        let mut h = HartSched::new();
        assert_eq!(h.pick(), None);
        assert!(h.idle() && h.is_empty());
        h.enqueue(10, 0);
        h.enqueue(11, 255);
        h.enqueue(12, 255);
        h.enqueue(13, 64);
        assert_eq!(h.pick(), Some(11), "mức 255 FIFO: 11 vào trước 12");
        assert_eq!(h.highest_priority(), Some(255));
        assert_eq!(h.len(), 4);
        h.remove_at(11, 255);
        assert_eq!(h.pick(), Some(12));
        assert_eq!(h.remove_all(13), 1);
        assert!(!h.contains(13));
        h.remove_at(12, 255);
        h.remove_at(10, 0);
        assert_eq!(h.pick(), None);
        assert_eq!(h.len(), 0);
        h.check_local().expect("bất biến nội tại");
    }

    #[test]
    fn bitmap_phu_du_256_muc() {
        let mut h = HartSched::new();
        for p in 0..MAX_PRIORITY {
            h.enqueue(p as ObjId + 1, p as u8);
            assert_eq!(h.highest_priority(), Some(p as u8));
            assert_eq!(h.len_at(p as u8), 1);
        }
        assert_eq!(h.len(), MAX_PRIORITY);
        h.check_local().expect("bất biến nội tại");
        for p in 0..MAX_PRIORITY {
            assert_eq!(h.pick(), Some((MAX_PRIORITY - p) as ObjId));
            h.remove_at((MAX_PRIORITY - p) as ObjId, (MAX_PRIORITY - 1 - p) as u8);
        }
        assert_eq!(h.pick(), None);
        h.check_local().expect("bất biến nội tại sau khi rút hết");
    }
}
