// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Hộp thư theo hart** — MPSC kích thước cố định của thiết kế §4.17 (`:423-428`).
//!
//! - **Nhiều người gửi, một người nhận** (MPSC): nhiều hart gửi message vào hộp thư của một
//!   hart đích; chỉ hart đích gọi [`Mailbox::try_recv`]/[`Mailbox::drain`].
//! - **Cố định**: `N` slot cấp sẵn trong chính hộp thư, **không cấp phát**.
//! - **Lock-free** ở cả hai phía: hàng đợi vòng có bộ đếm thế hệ theo slot (thuật toán
//!   Vyukov, biên giới hạn `N`); người gửi không bao giờ ghi đè message chưa được rút.
//! - **Đầy ⇒ backpressure**: [`Mailbox::send`] trả [`Busy`], message cũ còn nguyên; sau
//!   `drain`/`try_recv` thì gửi lại được.
//! - **Ack theo bộ đếm thế hệ**: `generation` đếm số message đã nhận, `ack(gen)` đánh dấu đã
//!   xử lý tới thế hệ đó, `pending(gen)` trả số message chưa ack tới `gen` (`[TK]`).
//!
//! ## Vì sao `N ≥ 2`
//!
//! Bộ đếm `seq` của slot mã hoá vị trí đã đặt chỗ (`pos + 1`) và vị trí đã rút (`pos + N`);
//! với `N = 1` hai giá trị này trùng nhau nên không phân biệt được "đầy" và "trống" — thuật
//! toán giả định `N ≥ 2`. [`Mailbox::new`] `assert!` điều kiện này.
//!
//! ## Trạng thái chuyển tiếp
//!
//! `send` đặt chỗ slot (`head`), tăng `generation`, ghi message, rồi **công bố** bằng `seq`.
//! Giữa các bước đó người nhận có thể thấy `generation` đã đếm nhưng message chưa rút được;
//! đây là trạng thái chuyển tiếp [TK] có chủ ý (đếm "đã nhận" sớm hơn "đã đọc") — bất biến
//! không mất message không phụ thuộc nó.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release, SeqCst};
use core::sync::atomic::{AtomicU32, AtomicUsize};

use crate::Busy;

/// Bảy loại message của hộp thư theo hart (thiết kế §4.17, `[TK]` cho payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Msg {
    /// Đánh thức TCB `tcb` trên hart `hart`.
    Wake { hart: u16, tcb: u32 },
    /// Xả TLB cho không gian địa chỉ của `tcb` trên hart `hart`.
    TlbFlush { hart: u16, tcb: u32 },
    /// Yêu cầu hart `hart` lập lịch lại (có thể kèm TCB `tcb` mới đến).
    Reschedule { hart: u16, tcb: u32 },
    /// Đẩy TCB `tcb` sang hart `hart` (push migration, §4.8).
    Migrate { hart: u16, tcb: u32 },
    /// Dừng hart `hart` (shutdown/khởi động lại).
    Stop { hart: u16 },
    /// Đá hart `hart` ra khỏi chế độ guest (VM exit, H1).
    VmKick { hart: u16 },
    /// Yêu cầu trạng thái nguồn của hart `hart` (PSCI/tương đương).
    PowerReq { hart: u16, state: PowerState },
}

/// Trạng thái nguồn cho [`Msg::PowerReq`] (`[TK]` — mô hình, chưa phải ABI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerState {
    /// Bật/giữ hart chạy.
    On,
    /// Tắt hart.
    Off,
    /// Ngủ tới khi có đánh thức.
    Suspend,
}

/// Loại (không kèm payload) của [`Msg`] — dùng để kiểm "đủ 7 loại".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MsgKind {
    Wake,
    TlbFlush,
    Reschedule,
    Migrate,
    Stop,
    VmKick,
    PowerReq,
}

impl MsgKind {
    /// Cả bảy loại, theo đúng thứ tự thiết kế §4.17.
    pub const ALL: [MsgKind; 7] = [
        MsgKind::Wake,
        MsgKind::TlbFlush,
        MsgKind::Reschedule,
        MsgKind::Migrate,
        MsgKind::Stop,
        MsgKind::VmKick,
        MsgKind::PowerReq,
    ];
}

impl Msg {
    /// Loại của message.
    pub fn kind(&self) -> MsgKind {
        match self {
            Msg::Wake { .. } => MsgKind::Wake,
            Msg::TlbFlush { .. } => MsgKind::TlbFlush,
            Msg::Reschedule { .. } => MsgKind::Reschedule,
            Msg::Migrate { .. } => MsgKind::Migrate,
            Msg::Stop { .. } => MsgKind::Stop,
            Msg::VmKick { .. } => MsgKind::VmKick,
            Msg::PowerReq { .. } => MsgKind::PowerReq,
        }
    }

    /// Hart đích của message.
    pub fn hart(&self) -> u16 {
        match self {
            Msg::Wake { hart, .. }
            | Msg::TlbFlush { hart, .. }
            | Msg::Reschedule { hart, .. }
            | Msg::Migrate { hart, .. }
            | Msg::Stop { hart }
            | Msg::VmKick { hart }
            | Msg::PowerReq { hart, .. } => *hart,
        }
    }
}

/// Một slot: `seq` là bộ đếm thế hệ của slot, `value` là chỗ chứa message.
struct Slot {
    seq: AtomicUsize,
    value: UnsafeCell<MaybeUninit<Msg>>,
}

impl Slot {
    const fn new() -> Self {
        Slot {
            seq: AtomicUsize::new(0),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

/// Hộp thư MPSC cố định `N` message (MPSC: nhiều người gửi, **một** người nhận).
///
/// ```
/// let mb: vifast_sync::Mailbox<4> = vifast_sync::Mailbox::new();
/// assert_eq!(mb.send(vifast_sync::Msg::Stop { hart: 1 }), Ok(()));
/// let mut got = 0;
/// assert_eq!(mb.drain(|_m| got += 1), 1);
/// assert_eq!(got, 1);
/// ```
pub struct Mailbox<const N: usize> {
    slots: [Slot; N],
    /// Vị trí đặt chỗ kế tiếp (đơn điệu, wrap `usize`).
    head: AtomicUsize,
    /// Vị trí rút kế tiếp (đơn điệu, wrap `usize`).
    tail: AtomicUsize,
    /// Số message đã nhận (`send` Ok) — bộ đếm thế hệ.
    generation: AtomicU32,
    /// Số message đã được ack (đơn điệu không giảm).
    acked: AtomicU32,
}

// SAFETY: message là `Msg` (Copy, không tham chiếu); mọi trạng thái dùng chung là atomic, chỗ
// chứa message chỉ được ghi bởi người đã đặt chỗ slot và chỉ được đọc sau khi `seq` công bố
// (release/acquire) bởi đúng một người nhận ⇒ chia sẻ giữa các luồng là an toàn.
unsafe impl<const N: usize> Send for Mailbox<N> {}
// SAFETY: như trên — `&Mailbox` cho phép `send`/`drain` từ nhiều luồng theo đúng giao thức.
unsafe impl<const N: usize> Sync for Mailbox<N> {}

impl<const N: usize> Mailbox<N> {
    /// Hộp thư rỗng, cấp sẵn `N` slot; dùng được trong `static`/`const`.
    ///
    /// # Panics
    ///
    /// `N` phải `≥ 2` (xem tài liệu module).
    pub const fn new() -> Self {
        assert!(N >= 2, "Mailbox: N phải >= 2");
        let mut slots = [const { Slot::new() }; N];
        let mut i = 0;
        while i < N {
            slots[i].seq = AtomicUsize::new(i);
            i += 1;
        }
        Mailbox {
            slots,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            generation: AtomicU32::new(0),
            acked: AtomicU32::new(0),
        }
    }

    /// Số message tối đa (`N`).
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Gửi một message. `Err(Busy)` khi hộp thư đầy: **không** ghi đè, **không** mất message cũ.
    pub fn send(&self, msg: Msg) -> Result<(), Busy> {
        let mut pos = self.head.load(Relaxed);
        loop {
            let slot = &self.slots[pos % N];
            let seq = slot.seq.load(Acquire);
            let dif = seq.wrapping_sub(pos) as isize;
            if dif == 0 {
                // Slot này đã sẵn sàng cho vị trí `pos`: đặt chỗ rồi mới ghi.
                match self
                    .head
                    .compare_exchange_weak(pos, pos.wrapping_add(1), Relaxed, Relaxed)
                {
                    Ok(_) => {
                        // Thế hệ đếm "đã nhận" trước khi công bố message (xem tài liệu module).
                        self.generation.fetch_add(1, SeqCst);
                        // SAFETY: slot `pos % N` đã được đặt chỗ độc quyền cho lần ghi này
                        // (`head` đã tăng), và chưa được công bố (`seq` còn là `pos`) nên không
                        // người nhận nào đọc; `MaybeUninit::write` không cần giá trị cũ.
                        unsafe {
                            (*slot.value.get()).write(msg);
                        }
                        slot.seq.store(pos.wrapping_add(1), Release);
                        return Ok(());
                    }
                    Err(actual) => pos = actual,
                }
            } else if dif < 0 {
                // Slot còn thuộc vòng trước ⇒ hộp thư đầy.
                return Err(Busy);
            } else {
                // Slot đã vượt `pos` (người khác vừa đặt chỗ): đọc lại vị trí đầu.
                pos = self.head.load(Relaxed);
            }
        }
    }

    /// Rút một message theo FIFO; `None` khi rỗng.
    pub fn try_recv(&self) -> Option<Msg> {
        let mut pos = self.tail.load(Relaxed);
        loop {
            let slot = &self.slots[pos % N];
            let seq = slot.seq.load(Acquire);
            let dif = seq.wrapping_sub(pos.wrapping_add(1)) as isize;
            if dif == 0 {
                match self
                    .tail
                    .compare_exchange_weak(pos, pos.wrapping_add(1), Relaxed, Relaxed)
                {
                    Ok(_) => {
                        // SAFETY: `seq == pos + 1` (acquire) nghĩa là người gửi đã ghi xong
                        // message và công bố; đặt chỗ `tail` cho lần rút này là duy nhất nên
                        // không ai đọc/ghi trùng.
                        let msg = unsafe { (*slot.value.get()).assume_init_read() };
                        slot.seq.store(pos.wrapping_add(N), Release);
                        return Some(msg);
                    }
                    Err(actual) => pos = actual,
                }
            } else if dif < 0 {
                return None;
            } else {
                pos = self.tail.load(Relaxed);
            }
        }
    }

    /// Rút **toàn bộ** message theo FIFO, gọi `f` cho từng message; trả số message đã rút.
    pub fn drain<F: FnMut(Msg)>(&self, mut f: F) -> usize {
        let mut n = 0;
        while let Some(msg) = self.try_recv() {
            f(msg);
            n += 1;
        }
        n
    }

    /// Số message đang chiếm slot: `head - tail` (gồm cả slot vừa đặt chỗ chưa công bố — trạng
    /// thái chuyển tiếp; dùng cho chẩn đoán/test).
    pub fn len(&self) -> usize {
        self.head
            .load(Acquire)
            .wrapping_sub(self.tail.load(Acquire))
    }

    /// Hộp thư rỗng?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Số message đã nhận (`send` trả `Ok`) — bộ đếm thế hệ.
    pub fn generation(&self) -> u32 {
        self.generation.load(SeqCst)
    }

    /// Số message đã được ack (`0..=generation`).
    pub fn acked(&self) -> u32 {
        self.acked.load(SeqCst)
    }

    /// Ack tới thế hệ `gen` (kẹp về `generation`, không bao giờ lùi); trả số message **mới**
    /// được ack.
    pub fn ack(&self, gen: u32) -> u32 {
        let sent = self.generation.load(SeqCst);
        let target = gen.min(sent);
        let prev = self.acked.fetch_max(target, SeqCst);
        target.saturating_sub(prev)
    }

    /// Số message **chưa ack** tính tới thế hệ `gen` (kẹp về `generation`).
    pub fn pending(&self, gen: u32) -> u32 {
        let sent = self.generation.load(SeqCst);
        let target = gen.min(sent);
        target.saturating_sub(self.acked.load(SeqCst))
    }
}

impl<const N: usize> Default for Mailbox<N> {
    fn default() -> Self {
        Self::new()
    }
}
