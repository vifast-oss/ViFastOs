// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hộp thư **FIFO cố định** + backpressure — KT-59 port từ `v2/sim/vifast-sim/src/mailbox.rs`
//! (oracle), không `alloc`.
//!
//! Model-only `[TK]`, không phải spec/ABI (D2-49(f)): mô hình **một luồng tất định**, không
//! atomic/thread/loom. Ngữ nghĩa giữ nguyên sim:
//!
//! - MPSC **kích thước cố định** [`MAILBOX_CAP`] = 8 message; thứ tự **FIFO**;
//! - đầy ⇒ [`Mailbox::try_push`] **trả lại** message (không ghi đè, không panic, không mất);
//!   tầng `Machine` quy về `KError::Busy` (backpressure);
//! - bất biến `pushed == popped + removed + len`, `len ≤ cap` (không mất/thêm message);
//! - `HartMailbox` giữ thêm **nhật ký message đã xử lý** theo đúng thứ tự drain + cờ `stopped`.
//!
//! # Biên tĩnh (khác sim)
//!
//! Sim dùng `Vec` cho nhật ký nên không biên; core có **biên tĩnh** [`MAX_MAIL_LOG`] message
//! mỗi hart. Khi nhật ký đầy, message vẫn được xử lý đúng một lần và **vẫn đếm** trong
//! `log_overflow` — chỉ phần *nhật ký* bị cắt (hàng đợi/bất biến không đổi). Đây là biên triển
//! khai đã ghi trong `docs/k3b-core.md` §7, không phải ABI; parity chỉ so nhật ký trong biên.

use crate::error::{CheckFailure, ObjId};

/// Sức chứa hộp thư `[TK]` (K1B-PLAN §5 KT-22 DoD 1: "cố định **8 message**").
pub const MAILBOX_CAP: usize = 8;

/// Số message **đã xử lý** tối đa được ghi nhật ký mỗi hart (biên `.bss`, `[TK]`).
pub const MAX_MAIL_LOG: usize = 32;

/// Một lệnh shootdown TLB trong message `TlbFlush` — kiểu dữ liệu thuần (không TLB).
///
/// Core KT-59 chưa sở hữu TLB (object layer K2 / KT-61); biến thể này giữ **hình dạng**
/// message của sim để KT-61 nối handler thật mà không đổi enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlushMsg {
    /// Số hiệu shootdown toàn cục.
    pub seq: u64,
    /// Thế hệ ack của hart đích.
    pub ack: u64,
    /// Thế hệ ASID.
    pub generation: u64,
    /// ASID.
    pub asid: u16,
    /// VA gốc.
    pub va: u64,
}

impl FlushMsg {
    /// Giá trị rỗng (mốc const-init).
    pub const EMPTY: Self = Self {
        seq: 0,
        ack: 0,
        generation: 0,
        asid: 0,
        va: 0,
    };
}

/// Loại message của hộp thư **theo hart** (thiết kế §4.17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxMsg {
    /// Đánh thức TCB đang chặn/ngủ.
    Wake(ObjId),
    /// Ép `dispatch` trên hart này (preempt/reschedule).
    Reschedule,
    /// Di trú TCB sang hart khác — 1 hart từ chối (`NotSupported`).
    Migrate {
        /// TCB cần di trú.
        tcb: ObjId,
        /// Hart đích.
        to_hart: u16,
    },
    /// Dừng hart (đặt cờ `stopped`, không đổi hàng chờ).
    Stop,
    /// Shootdown TLB (KT-40; core KT-59 chưa có TLB — giữ chỗ, handler no-op).
    TlbFlush(FlushMsg),
}

impl MailboxMsg {
    /// Mốc rỗng cho khởi tạo mảng tĩnh.
    pub const EMPTY: Self = Self::Wake(0);

    /// Mã biến thể ổn định (digest/parity).
    pub fn tag(self) -> u8 {
        match self {
            MailboxMsg::Wake(_) => 1,
            MailboxMsg::Reschedule => 2,
            MailboxMsg::Migrate { .. } => 3,
            MailboxMsg::Stop => 4,
            MailboxMsg::TlbFlush(_) => 5,
        }
    }

    /// Tên biến thể ổn định.
    pub fn name(self) -> &'static str {
        match self {
            MailboxMsg::Wake(_) => "Wake",
            MailboxMsg::Reschedule => "Reschedule",
            MailboxMsg::Migrate { .. } => "Migrate",
            MailboxMsg::Stop => "Stop",
            MailboxMsg::TlbFlush(_) => "TlbFlush",
        }
    }
}

/// Hàng FIFO **cố định** `CAP` phần tử.
///
/// Bất biến (kiểm bởi [`Mailbox::invariant`]): `len ≤ CAP` và
/// `pushed == popped + removed + len` — mọi message được đẩy vào đều được pop, bị `retain` gỡ,
/// hoặc còn trong hàng; **không** có đường nào ghi đè/làm mất message.
#[derive(Debug, Clone, Copy)]
pub struct Mailbox<T: Copy, const CAP: usize> {
    buf: [Option<T>; CAP],
    pushed: u64,
    popped: u64,
    removed: u64,
}

impl<T: Copy, const CAP: usize> Mailbox<T, CAP> {
    /// Hộp thư rỗng sức chứa `CAP`.
    pub const fn new() -> Self {
        Self {
            buf: [None; CAP],
            pushed: 0,
            popped: 0,
            removed: 0,
        }
    }

    /// Sức chứa.
    pub fn capacity(&self) -> usize {
        CAP
    }

    /// Số message đang nằm trong hàng.
    pub fn len(&self) -> usize {
        self.buf.iter().filter(|x| x.is_some()).count()
    }

    /// Hàng rỗng?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Hàng đầy?
    pub fn is_full(&self) -> bool {
        self.len() >= CAP
    }

    /// Đẩy `item` vào **cuối** hàng; đầy ⇒ `Err(item)` — trả lại nguyên message (backpressure),
    /// không ghi đè phần tử cũ.
    pub fn try_push(&mut self, item: T) -> Result<(), T> {
        for i in 0..CAP {
            if self.buf[i].is_none() {
                self.buf[i] = Some(item);
                self.pushed += 1;
                return Ok(());
            }
        }
        Err(item)
    }

    /// Lấy phần tử **đầu** hàng (FIFO). Không có ⇒ `None`.
    pub fn pop_front(&mut self) -> Option<T> {
        for i in 0..CAP {
            if let Some(item) = self.buf[i].take() {
                // Dồn phần còn lại về trước để đầu hàng luôn ở ô 0 (mảng nhỏ, 8 ô).
                for j in i + 1..CAP {
                    self.buf[j - 1] = self.buf[j];
                }
                self.buf[CAP - 1] = None;
                self.popped += 1;
                return Some(item);
            }
        }
        None
    }

    /// Phần tử đầu hàng (không pop).
    pub fn front(&self) -> Option<&T> {
        self.buf.iter().find_map(|x| x.as_ref())
    }

    /// Duyệt message đang trong hàng theo FIFO.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter().filter_map(|x| x.as_ref())
    }

    /// Gỡ mọi phần tử `f` trả `false` (giữ thứ tự FIFO); cộng `removed`.
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) -> usize {
        let mut removed = 0usize;
        let mut w = 0usize;
        for r in 0..CAP {
            if let Some(item) = self.buf[r] {
                if f(&item) {
                    self.buf[w] = Some(item);
                    w += 1;
                } else {
                    removed += 1;
                }
            }
        }
        for slot in self.buf.iter_mut().take(CAP).skip(w) {
            *slot = None;
        }
        self.removed += removed as u64;
        removed
    }

    /// Số message đã được **nhận** vào hàng (push thành công).
    pub fn pushed(&self) -> u64 {
        self.pushed
    }

    /// Số message đã **pop** ra khỏi hàng.
    pub fn popped(&self) -> u64 {
        self.popped
    }

    /// Số message bị **gỡ** bởi [`Mailbox::retain`].
    pub fn removed(&self) -> u64 {
        self.removed
    }

    /// `pushed == popped + removed + len` và `len ≤ CAP`.
    pub fn invariant(&self) -> bool {
        let len = self.len();
        len <= CAP && self.pushed == self.popped + self.removed + len as u64
    }

    /// Bất biến + chẩn đoán (`Machine::check_mailboxes`).
    pub fn check(&self, hart: u16) -> Result<(), CheckFailure> {
        let len = self.len();
        if len > CAP {
            return Err(CheckFailure::two(
                "hop thu qua suc chua",
                hart as u32,
                len as u32,
            ));
        }
        if self.pushed != self.popped + self.removed + len as u64 {
            return Err(CheckFailure::two(
                "hop thu mat/them message",
                hart as u32,
                len as u32,
            ));
        }
        Ok(())
    }
}

impl<T: Copy, const CAP: usize> Default for Mailbox<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

/// Hộp thư của **một hart**: hàng FIFO cố định + nhật ký message đã **xử lý** (theo đúng thứ
/// tự drain) + cờ `stopped`.
#[derive(Debug, Clone, Copy)]
pub struct HartMailbox {
    mb: Mailbox<MailboxMsg, MAILBOX_CAP>,
    log: [MailboxMsg; MAX_MAIL_LOG],
    n_log: usize,
    log_overflow: u64,
    stopped: bool,
}

impl HartMailbox {
    /// Hộp thư rỗng.
    pub const fn new() -> Self {
        Self {
            mb: Mailbox::new(),
            log: [MailboxMsg::EMPTY; MAX_MAIL_LOG],
            n_log: 0,
            log_overflow: 0,
            stopped: false,
        }
    }

    /// Khởi tạo **tại chỗ** hộp thư rỗng (KT-61: static `.bss` đã zero). `Option<MailboxMsg>`
    /// **không** có `None == 0` (niche của enum) nên để zero thì cả 8 ô đọc thành
    /// `Some(Wake(0))` và `check_mailboxes` báo "hop thu mat/them message".
    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    /// Số message đang nằm trong hàng.
    pub fn len(&self) -> usize {
        self.mb.len()
    }

    /// Hàng rỗng?
    pub fn is_empty(&self) -> bool {
        self.mb.is_empty()
    }

    /// Hàng đầy?
    pub fn is_full(&self) -> bool {
        self.mb.is_full()
    }

    /// Đẩy message vào hàng; đầy ⇒ `Err(msg)` (người gọi quy về `Busy`).
    pub fn try_push(&mut self, msg: MailboxMsg) -> Result<(), MailboxMsg> {
        self.mb.try_push(msg)
    }

    /// Lấy message đầu hàng (FIFO) — **chưa** đánh dấu đã xử lý.
    pub fn pop(&mut self) -> Option<MailboxMsg> {
        self.mb.pop_front()
    }

    /// Duyệt message **đang nằm trong hàng** theo FIFO.
    pub fn iter(&self) -> impl Iterator<Item = &MailboxMsg> {
        self.mb.iter()
    }

    /// Ghi message **vừa xử lý xong** vào nhật ký (theo thứ tự drain).
    ///
    /// Trả `true` nếu ghi được; `false` nếu nhật ký đã đầy (message **vẫn** đã xử lý, chỉ
    /// không lưu được — xem [`HartMailbox::log_overflow`]).
    pub fn record(&mut self, msg: MailboxMsg) -> bool {
        if self.n_log < MAX_MAIL_LOG {
            self.log[self.n_log] = msg;
            self.n_log += 1;
            true
        } else {
            self.log_overflow += 1;
            false
        }
    }

    /// Nhật ký message đã xử lý (theo thứ tự drain) — nguồn kiểm FIFO/tất định.
    pub fn log(&self) -> &[MailboxMsg] {
        &self.log[..self.n_log]
    }

    /// Số message đã xử lý nhưng không còn chỗ trong nhật ký (biên tĩnh).
    pub fn log_overflow(&self) -> u64 {
        self.log_overflow
    }

    /// Hart đã nhận `Stop` chưa.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Bật cờ `stopped` (không đổi hàng chờ).
    pub fn set_stopped(&mut self) {
        self.stopped = true;
    }

    /// Bất biến hàng đợi.
    pub fn invariant(&self) -> bool {
        self.mb.invariant()
    }

    /// Bất biến + chẩn đoán.
    pub fn check(&self, hart: u16) -> Result<(), CheckFailure> {
        self.mb.check(hart)
    }
}

impl Default for HartMailbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_va_backpressure_tra_lai_message() {
        let mut mb: Mailbox<u32, MAILBOX_CAP> = Mailbox::new();
        assert_eq!(mb.capacity(), MAILBOX_CAP);
        for i in 0..MAILBOX_CAP as u32 {
            assert_eq!(mb.try_push(i), Ok(()));
        }
        assert!(mb.is_full());
        assert_eq!(mb.try_push(7), Err(7), "day ⇒ tra lai nguyen message");
        assert_eq!(mb.len(), MAILBOX_CAP, "khong ghi de");
        for i in 0..MAILBOX_CAP as u32 {
            assert_eq!(mb.pop_front(), Some(i), "FIFO");
        }
        assert_eq!(mb.pop_front(), None);
        assert!(mb.invariant());
        assert_eq!(mb.pushed(), MAILBOX_CAP as u64);
        assert_eq!(mb.popped(), MAILBOX_CAP as u64);
    }

    #[test]
    fn retain_giu_fifo_va_dem_removed() {
        let mut mb: Mailbox<u32, MAILBOX_CAP> = Mailbox::new();
        for i in 0..MAILBOX_CAP as u32 {
            mb.try_push(i).unwrap();
        }
        assert_eq!(mb.retain(|v| v % 2 == 0), MAILBOX_CAP / 2);
        let con: [u32; 4] = [0, 2, 4, 6];
        for (i, v) in mb.iter().enumerate() {
            assert_eq!(*v, con[i], "FIFO phan con lai");
        }
        assert_eq!(mb.removed(), (MAILBOX_CAP / 2) as u64);
        assert!(mb.invariant());
    }

    #[test]
    fn hart_mailbox_log_dung_thu_tu_va_co_stopped() {
        let mut h = HartMailbox::new();
        let msg = [
            MailboxMsg::Wake(3),
            MailboxMsg::Reschedule,
            MailboxMsg::Stop,
        ];
        for m in msg {
            h.try_push(m).expect("con cho");
        }
        assert_eq!(h.len(), 3);
        for m in msg {
            let got = h.pop().expect("FIFO");
            assert_eq!(got, m);
            assert!(h.record(got));
        }
        assert_eq!(h.log(), &msg[..]);
        assert!(!h.stopped());
        h.set_stopped();
        assert!(h.stopped());
        assert!(h.invariant());
    }

    #[test]
    fn nhat_ky_co_bien_va_dem_overflow() {
        let mut h = HartMailbox::new();
        for i in 0..(MAX_MAIL_LOG + 3) {
            assert_eq!(h.record(MailboxMsg::Wake(i as u32 + 1)), i < MAX_MAIL_LOG);
        }
        assert_eq!(h.log().len(), MAX_MAIL_LOG);
        assert_eq!(h.log_overflow(), 3);
    }
}
