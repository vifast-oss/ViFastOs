// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-22 — **hộp thư FIFO cố định** (mailbox) cho mô hình endpoint/TCB (thiết kế §4.16/§4.17
//! `docs/v2/ViFastOs-v2-thiet-ke.md:403-428`; K1B-PLAN §5 KT-22; **D2-49(a)** 1 hart —
//! push migration hoãn K1c; **D2-49(f)** chỉ hộp thư **mô hình tất định**, không MCS lock/
//! lock-free/atomic/thread/loom — `vifast-sync`/stress thuộc K1c/SMP).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Ngữ nghĩa lấy đúng phần K1b:
//!
//! - MPSC **kích thước cố định** `[MAILBOX_CAP]` = 8 message `[TK]`; thứ tự **FIFO**.
//! - Đầy ⇒ **backpressure**: [`Mailbox::try_push`] **trả lại** message (không ghi đè, không
//!   panic, không mất); tầng `Machine` quy về `KError::Busy` để người gửi "xử lý hộp thư của
//!   mình rồi thử lại" (§4.17). Vì mô hình **1 luồng**, "thử lại" là quyết định của người gọi
//!   (test), **không** có vòng chờ/atomic nào trong kiểu này.
//! - Không có ack thế hệ trong K1b: [`Mailbox`] chỉ giữ `pushed/popped/removed` để kiểm bất
//!   biến `pushed == popped + removed + len` (không mất/thêm message). Ack thế hệ + `loom`/
//!   stress để K1c.
//!
//! Kiểu này **không** dùng `std::sync`/atomic: `VecDeque` thường, biên dịch được cả dưới Miri.

use std::collections::VecDeque;

use crate::kobj::ObjId;

/// Sức chứa hộp thư `[TK]` (K1B-PLAN §5 KT-22 DoD 1: "cố định **8 message**").
///
/// Dùng chung cho **hàng gửi của Endpoint** và **hộp thư theo hart**; "số nhỏ, có thể tăng nếu
/// Agent 0 chốt" — đổi hằng này là đổi ngưỡng mô hình, phải qua Agent 0.
pub const MAILBOX_CAP: usize = 8;

/// Loại message của hộp thư **theo hart** (thiết kế §4.17: `Wake`, `TlbFlush`, `Reschedule`,
/// `Migrate`, `Stop`, `VmKick`, `PowerReq`). K1b có phần không phụ thuộc TLB/VCpu/Power:
/// `Wake`, `Reschedule`, `Migrate`, `Stop`; **KT-40** thêm **đúng một** kind `TlbFlush` (nới
/// D2-59 theo D2-65 Q5) — `VmKick`/`PowerReq` vẫn để K3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxMsg {
    /// Đánh thức TCB đang chặn/ngủ (`Machine::wake`, idempotent).
    Wake(ObjId),
    /// Ép `dispatch` trên hart này (preempt/reschedule).
    Reschedule,
    /// Di trú TCB sang hart khác — **K1b 1 hart từ chối** (`NotSupported`); cài thật ở K1c.
    Migrate { tcb: ObjId, to_hart: u16 },
    /// Dừng hart (mô hình: đặt cờ `stopped`, không đổi hàng chờ).
    Stop,
    /// **KT-40** — shootdown TLB: hart đích xả entry `(generation, asid)` phủ `va` rồi **ack**
    /// thế hệ `flush.ack` của hộp thư mình (mô hình của "IPI + `sfence.vma`"; D2-65 Q5).
    TlbFlush(crate::tlb::TlbFlush),
}

/// Hàng FIFO **cố định** `cap` phần tử.
///
/// Bất biến (kiểm bởi [`Mailbox::invariant`]): `len ≤ cap` và
/// `pushed == popped + removed + len` — mọi message được đẩy vào đều được pop, bị `retain` gỡ,
/// hoặc còn trong hàng; **không** có đường nào ghi đè/làm mất message.
#[derive(Debug)]
pub struct Mailbox<T> {
    cap: usize,
    buf: VecDeque<T>,
    pushed: u64,
    popped: u64,
    removed: u64,
}

impl<T> Mailbox<T> {
    /// Hộp thư sức chứa [`MAILBOX_CAP`] (mặc định của mô hình K1b).
    pub fn new() -> Self {
        Self::with_capacity(MAILBOX_CAP)
    }

    /// Hộp thư sức chứa `cap` (dương). Không cấp phát theo `cap` để không phụ thuộc allocator:
    /// `VecDeque` chỉ lớn tới `cap` vì [`Mailbox::try_push`] từ chối khi đầy.
    pub fn with_capacity(cap: usize) -> Self {
        assert!(cap > 0, "sức chứa hộp thư phải dương");
        Self {
            cap,
            buf: VecDeque::new(),
            pushed: 0,
            popped: 0,
            removed: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buf.len() >= self.cap
    }

    /// Đẩy `item` vào **cuối** hàng; đầy ⇒ `Err(item)` — **trả lại nguyên** message để người
    /// gọi thử lại sau (backpressure, design §4.17), không ghi đè phần tử cũ.
    pub fn try_push(&mut self, item: T) -> Result<(), T> {
        if self.is_full() {
            return Err(item);
        }
        self.buf.push_back(item);
        self.pushed += 1;
        Ok(())
    }

    /// Lấy phần tử **đầu** hàng (FIFO). Không có ⇒ `None`.
    pub fn pop_front(&mut self) -> Option<T> {
        let item = self.buf.pop_front();
        if item.is_some() {
            self.popped += 1;
        }
        item
    }

    pub fn front(&self) -> Option<&T> {
        self.buf.front()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    /// Gỡ mọi phần tử `f` trả `false` (giữ thứ tự FIFO của phần còn lại); cộng `removed`.
    ///
    /// Đây là đường gỡ message khi TCB chết/treo (D2-25: hàng chờ cũ bị huỷ) — message bị gỡ
    /// **đếm** vào `removed` để bất biến không mất/thêm vẫn đúng.
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) -> usize {
        let truoc = self.buf.len();
        self.buf.retain(|it| f(it));
        let n = truoc - self.buf.len();
        self.removed += n as u64;
        n
    }

    /// Số message đã được **nhận** vào hàng (push thành công).
    pub fn pushed(&self) -> u64 {
        self.pushed
    }

    /// Số message đã **pop** ra khỏi hàng.
    pub fn popped(&self) -> u64 {
        self.popped
    }

    /// Số message bị **gỡ** bởi [`Mailbox::retain`] (huỷ vì TCB chết/treo).
    pub fn removed(&self) -> u64 {
        self.removed
    }

    /// `pushed == popped + removed + len` và `len ≤ cap` — không message nào bị mất/thêm.
    pub fn invariant(&self) -> bool {
        self.buf.len() <= self.cap
            && self.pushed == self.popped + self.removed + self.buf.len() as u64
    }
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Hộp thư của **một hart**: hàng FIFO cố định + nhật ký message đã **xử lý** (theo đúng thứ
/// tự drain) + cờ `stopped`.
///
/// Nhật ký chỉ ghi message **đã xử lý** (không ghi message bị từ chối) nên test kiểm được
/// "drain đúng thứ tự FIFO, đúng một lần mỗi message" mà không cần ack thế hệ (K1b — D2-49(f)).
#[derive(Debug)]
pub struct HartMailbox {
    mb: Mailbox<MailboxMsg>,
    log: Vec<MailboxMsg>,
    stopped: bool,
}

impl HartMailbox {
    pub fn new() -> Self {
        Self {
            mb: Mailbox::new(),
            log: Vec::new(),
            stopped: false,
        }
    }

    pub fn len(&self) -> usize {
        self.mb.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mb.is_empty()
    }

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

    /// Duyệt message **đang nằm trong hàng** theo FIFO (không pop, không đổi bộ đếm).
    ///
    /// **KT-29** dùng để `check_smp` đếm/kiểm message `Migrate` đang chờ.
    pub fn iter(&self) -> impl Iterator<Item = &MailboxMsg> {
        self.mb.iter()
    }

    /// Ghi message **vừa xử lý xong** vào nhật ký (theo thứ tự drain).
    pub fn record(&mut self, msg: MailboxMsg) {
        self.log.push(msg);
    }

    /// Nhật ký message đã xử lý — nguồn kiểm FIFO/tất định của test.
    pub fn log(&self) -> &[MailboxMsg] {
        &self.log
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn set_stopped(&mut self) {
        self.stopped = true;
    }

    /// Bất biến hàng đợi (`pushed == popped + removed + len`, `len ≤ cap`).
    pub fn invariant(&self) -> bool {
        self.mb.invariant()
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
        let mut mb: Mailbox<u32> = Mailbox::new();
        assert_eq!(mb.capacity(), MAILBOX_CAP);
        for i in 0..MAILBOX_CAP as u32 {
            assert_eq!(mb.try_push(i), Ok(()));
        }
        assert!(mb.is_full());
        assert_eq!(mb.try_push(7), Err(7), "đầy ⇒ trả lại nguyên message");
        assert_eq!(mb.len(), MAILBOX_CAP, "không ghi đè");
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
        let mut mb: Mailbox<u32> = Mailbox::new();
        for i in 0..MAILBOX_CAP as u32 {
            mb.try_push(i).unwrap();
        }
        assert_eq!(mb.retain(|v| v % 2 == 0), MAILBOX_CAP / 2);
        assert_eq!(
            mb.iter().copied().collect::<Vec<_>>(),
            vec![0, 2, 4, 6],
            "FIFO phần còn lại"
        );
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
            h.try_push(m).expect("còn chỗ");
        }
        assert_eq!(h.len(), 3);
        for m in msg {
            let got = h.pop().expect("FIFO");
            assert_eq!(got, m);
            h.record(got);
        }
        assert_eq!(h.log(), &msg[..]);
        assert!(!h.stopped());
        h.set_stopped();
        assert!(h.stopped());
        assert!(h.invariant());
    }
}
