// SPDX-License-Identifier: MIT OR Apache-2.0
//! Body **tĩnh** của Endpoint/Reply/Notification + hàng chờ IPC + bảng WaitOnAddress —
//! KT-60 port từ `EndpointState`/`ReplyData`/`NotificationState`/`QueuedMessage` của
//! `v2/sim/vifast-sim/src/kobj.rs` (oracle), thay `Vec`/`Box` bằng mảng có biên.
//!
//! Model-only `[TK]`, không phải ABI. Mọi bảng nằm trong `.bss` của [`crate::Machine`];
//! đầy bảng ⇒ lỗi `NotEnoughMemory` **nguyên tử** ở tầng thao tác (biên triển khai, ghi trong
//! `docs/k3b-core.md` §8), không panic, không cắt im lặng.
//!
//! # Vì sao message nằm trong **pool chung**
//!
//! Sim bọc mỗi `QueuedMessage` trong `Box` (heap); core không có `alloc` nên message đang chờ
//! nằm trong pool [`crate::machine::MAX_QUEUED_MESSAGES`], hàng của Endpoint giữ **chỉ số**
//! (`u16`). Sức chứa **mỗi** Endpoint vẫn là [`ENDPOINT_QUEUE_CAP`] = 8 như
//! `MAILBOX_CAP` của sim; pool chung nhỏ hơn tổng 8 × `MAX_ENDPOINTS` là biên tĩnh của mô
//! hình (một `Machine` biểu diễn tối đa `MAX_QUEUED_MESSAGES` message đang chờ, giống
//! `MAX_TCBS`).

use crate::error::{CapId, ObjId};
use crate::ipc::Message;
use crate::tcb::FaultKind;

/// Sức chứa hàng gửi của **một** Endpoint — bằng `MAILBOX_CAP` của sim (KT-22: "cố định 8").
pub const ENDPOINT_QUEUE_CAP: usize = 8;
/// Số bucket bảng băm WaitOnAddress (`[TK]`, D2-53 Q3 — như sim `WAITADDR_BUCKETS`).
pub const WAITADDR_BUCKETS: usize = 64;
/// Số ô giá trị "từ" tối đa của mô hình WaitOnAddress (biên tĩnh `[TK]`).
pub const MAX_ADDR_WORDS: usize = 64;
/// Số nút waiter WaitOnAddress tối đa (mỗi TCB tối đa một nút ⇒ bằng `MAX_TCBS`).
pub const MAX_WAIT_NODES: usize = 64;

/// Hàm băm bucket WaitOnAddress — **nguyên văn** sim (`(frame * 0x9E37_79B1) ^ offset) % 64`).
pub const fn waitaddr_bucket(frame: ObjId, offset: u32) -> usize {
    ((frame.wrapping_mul(0x9E37_79B1) ^ offset) as usize) % WAITADDR_BUCKETS
}

/// Message đang chờ trong hàng gửi của Endpoint (port `QueuedMessage`).
///
/// - `msg: Some` = message người dùng (Send/NBSend/Call) đã được cấp `msg_id`;
///   `msg: None` = **fault** (`Exit` không có message — D2-25).
/// - `call` = message từ `Call` đang chờ `Reply`; `false` = `Send`/`NBSend`/`Call no_reply`.
/// - `blocking` = người gửi đang `BlockedOnSend` chờ giao; `false` = gửi không chặn.
#[derive(Debug, Clone, Copy)]
pub struct QueuedMsg {
    pub(crate) msg: Option<Message>,
    pub(crate) sender: ObjId,
    pub(crate) call: bool,
    pub(crate) fault: Option<FaultKind>,
    pub(crate) blocking: bool,
    pub(crate) alive: bool,
    /// **KT-405** — mắt xích **xâm nhập** của hàng gửi Endpoint (`QUEUE_NONE` = cuối hàng).
    /// Trước KT-405 hàng giữ `slots: [u16; 8]` (16 B) trong [`IpcQueue`]; chuyển mắt xích vào
    /// chính message ⇒ `IpcQueue` còn **32 B** và `EndpointCore` vừa **48 B** của
    /// `spec/abi.toml:159` (`endpoint = 64`, chừa 16 B header).
    pub(crate) next: u16,
}

impl QueuedMsg {
    pub(crate) const EMPTY: Self = Self {
        msg: None,
        sender: 0,
        call: false,
        fault: None,
        blocking: false,
        alive: false,
        next: QUEUE_NONE,
    };
}

/// Chỉ số "không có mắt xích" của hàng gửi xâm nhập.
pub(crate) const QUEUE_NONE: u16 = u16::MAX;

/// Hàng gửi FIFO **xâm nhập** của một Endpoint — mắt xích nằm trong [`QueuedMsg::next`]
/// (pool message của `Machine`), hàng chỉ giữ `head`/`tail`/`len`.
///
/// **KT-405** — thay `slots: [u16; 8]` (16 B) bằng `head`/`tail` (`u16`) ⇒ `IpcQueue` **32 B**,
/// `EndpointCore` **48 B** (spec `endpoint = 64`, chừa 16 B header). Ngữ nghĩa FIFO, sức chứa
/// [`ENDPOINT_QUEUE_CAP`], bất biến `pushed == popped + removed + len` **không đổi**.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IpcQueue {
    /// Chỉ số message đầu hàng (`QUEUE_NONE` = rỗng).
    head: u16,
    /// Chỉ số message cuối hàng (`QUEUE_NONE` = rỗng).
    tail: u16,
    len: u8,
    pub(crate) pushed: u64,
    pub(crate) popped: u64,
    pub(crate) removed: u64,
}

impl IpcQueue {
    pub(crate) const EMPTY: Self = Self {
        head: QUEUE_NONE,
        tail: QUEUE_NONE,
        len: 0,
        pushed: 0,
        popped: 0,
        removed: 0,
    };

    /// Sức chứa (luôn [`ENDPOINT_QUEUE_CAP`]).
    pub(crate) fn capacity(&self) -> usize {
        ENDPOINT_QUEUE_CAP
    }

    /// Số message đang chờ.
    pub(crate) fn len(&self) -> usize {
        self.len as usize
    }

    /// Hàng rỗng?
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Hàng đầy?
    pub(crate) fn is_full(&self) -> bool {
        self.len as usize >= ENDPOINT_QUEUE_CAP
    }

    /// Đẩy chỉ số message vào cuối hàng — trả lại chỉ số nếu đầy (không ghi đè).
    pub(crate) fn try_push(&mut self, idx: u16, pool: &mut [QueuedMsg]) -> Result<(), u16> {
        if self.is_full() {
            return Err(idx);
        }
        pool[idx as usize].next = QUEUE_NONE;
        if self.tail == QUEUE_NONE {
            self.head = idx;
        } else {
            pool[self.tail as usize].next = idx;
        }
        self.tail = idx;
        self.len += 1;
        self.pushed += 1;
        Ok(())
    }

    /// Lấy chỉ số message đầu hàng.
    pub(crate) fn pop_front(&mut self, pool: &[QueuedMsg]) -> Option<u16> {
        if self.len == 0 {
            return None;
        }
        let idx = self.head;
        self.head = pool[idx as usize].next;
        if self.head == QUEUE_NONE {
            self.tail = QUEUE_NONE;
        }
        self.len -= 1;
        self.popped += 1;
        Some(idx)
    }

    /// Duyệt chỉ số theo FIFO (đi theo mắt xích trong `pool`).
    pub(crate) fn iter<'a>(&'a self, pool: &'a [QueuedMsg]) -> impl Iterator<Item = u16> + 'a {
        let len = self.len as usize;
        let mut cur = self.head;
        let mut n = 0usize;
        core::iter::from_fn(move || {
            if n >= len || cur == QUEUE_NONE {
                return None;
            }
            let idx = cur;
            cur = pool[idx as usize].next;
            n += 1;
            Some(idx)
        })
    }

    /// Thay nội dung hàng bằng `kept` (giữ thứ tự), cộng `removed` phần bị bỏ.
    pub(crate) fn kept(&mut self, kept: &[u16], pool: &mut [QueuedMsg]) {
        let old = self.len as usize;
        self.head = QUEUE_NONE;
        self.tail = QUEUE_NONE;
        self.len = 0;
        for &mi in kept.iter().take(ENDPOINT_QUEUE_CAP) {
            pool[mi as usize].next = QUEUE_NONE;
            if self.tail == QUEUE_NONE {
                self.head = mi;
            } else {
                pool[self.tail as usize].next = mi;
            }
            self.tail = mi;
            self.len += 1;
        }
        self.removed += (old - self.len as usize) as u64;
    }

    /// Bất biến cục bộ: `pushed == popped + removed + len`, `len ≤ cap`, chuỗi mắt xích không
    /// đứt/không vòng và đếm đúng `len`.
    pub(crate) fn invariant(&self, pool: &[QueuedMsg]) -> bool {
        if self.len as usize > ENDPOINT_QUEUE_CAP {
            return false;
        }
        if self.pushed != self.popped + self.removed + self.len as u64 {
            return false;
        }
        if (self.len == 0) != (self.head == QUEUE_NONE) {
            return false;
        }
        let mut cur = self.head;
        let mut n = 0usize;
        while cur != QUEUE_NONE {
            if cur as usize >= pool.len() || n > ENDPOINT_QUEUE_CAP {
                return false;
            }
            n += 1;
            cur = pool[cur as usize].next;
        }
        n == self.len as usize
    }
}

/// **KT-405** — **thân ABI tại chỗ** của Endpoint: `recv` + hàng gửi xâm nhập. `const` assert
/// `size_of::<EndpointCore>() <= 48` ở cuối tệp (spec/abi.toml:159 `endpoint = 64`, chừa 16 B
/// header cho `UntypedStore` ở KT-410).
///
/// Body Endpoint — port `EndpointState` của sim.
#[derive(Debug, Clone, Copy)]
pub struct EndpointCore {
    /// `(TCB đang chờ Recv, Reply cap người nhận đã cấp)` — tối đa **một** waiter.
    pub(crate) recv: Option<(ObjId, Option<CapId>)>,
    /// Hàng gửi FIFO xâm nhập.
    pub(crate) send: IpcQueue,
}

impl EndpointCore {
    pub(crate) const EMPTY: Self = Self {
        recv: None,
        send: IpcQueue::EMPTY,
    };
}

/// Phần **mô phỏng** của body Endpoint (không phải ABI) — hiện chỉ `last_mr` (model API).
#[derive(Debug, Clone, Copy)]
pub(crate) struct EndpointModel {
    /// MR0 của message gần nhất (model API).
    pub(crate) last_mr: Option<u64>,
}

impl EndpointModel {
    pub(crate) const EMPTY: Self = Self { last_mr: None };
}

/// Body Endpoint = **thân ABI** ([`EndpointCore`]) + **phần mô phỏng** ([`EndpointModel`]).
///
/// `Deref`/`DerefMut` sang [`EndpointCore`] để mọi truy cập `ep.recv`/`ep.send` giữ nguyên;
/// `last_mr` đọc qua `ep.model.last_mr`.
#[derive(Debug, Clone, Copy)]
pub struct EndpointData {
    pub(crate) core: EndpointCore,
    pub(crate) model: EndpointModel,
}

impl core::ops::Deref for EndpointData {
    type Target = EndpointCore;

    fn deref(&self) -> &EndpointCore {
        &self.core
    }
}

impl core::ops::DerefMut for EndpointData {
    fn deref_mut(&mut self) -> &mut EndpointCore {
        &mut self.core
    }
}

impl EndpointData {
    pub(crate) const EMPTY: Self = Self {
        core: EndpointCore::EMPTY,
        model: EndpointModel::EMPTY,
    };
}

// **KT-405** — `const` assert khớp `spec/abi.toml:159` (`endpoint = 64`): thân ABI ≤ 48 B
// (64 − 16 B header/đệm của `UntypedStore` ở KT-410).
const _: () = assert!(
    core::mem::size_of::<EndpointCore>() <= 48,
    "EndpointCore phải ≤ 48 B (spec/abi.toml:159 endpoint = 64, chừa 16 B header)"
);

/// Body Reply — port `ReplyData` của sim.
#[derive(Debug, Clone, Copy)]
pub struct ReplyData {
    /// TCB đang `BlockedOnReply` trên Reply này (bất biến C2: `caller ⇔ BlockedOnReply`).
    pub(crate) caller: Option<ObjId>,
}

impl ReplyData {
    pub(crate) const EMPTY: Self = Self { caller: None };
}

/// Body Notification — port `NotificationState` của sim (KT-27).
///
/// `word` OR badge của mọi `Signal` chưa được `Wait`/`Poll` lấy; `badge` OR song song (bất
/// biến `badge == word`); **một** waiter (TCB chờ thứ hai ⇒ `Busy`); `bound_sc` là quan hệ
/// 1:1 `SchedContext.Bind(sc, notification)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationData {
    pub(crate) word: u64,
    pub(crate) badge: u64,
    pub(crate) waiter: Option<ObjId>,
    pub(crate) bound_sc: Option<ObjId>,
}

/// Một nút waiter WaitOnAddress — nằm trong danh sách liên kết của bucket.
#[derive(Debug, Clone, Copy)]
pub struct WaitNode {
    pub(crate) frame: ObjId,
    pub(crate) offset: u32,
    pub(crate) tcb: ObjId,
    /// Chỉ số nút kế tiếp (`NODE_NONE` = hết).
    pub(crate) next: u16,
    pub(crate) alive: bool,
}

/// Chỉ số "không có nút".
pub(crate) const NODE_NONE: u16 = u16::MAX;

impl WaitNode {
    pub(crate) const EMPTY: Self = Self {
        frame: 0,
        offset: 0,
        tcb: 0,
        next: NODE_NONE,
        alive: false,
    };
}

/// Một ô giá trị "từ" của WaitOnAddress (`(frame, offset) -> u64`).
#[derive(Debug, Clone, Copy)]
pub struct AddrWord {
    pub(crate) frame: ObjId,
    pub(crate) offset: u32,
    pub(crate) value: u64,
    pub(crate) alive: bool,
}

impl AddrWord {
    pub(crate) const EMPTY: Self = Self {
        frame: 0,
        offset: 0,
        value: 0,
        alive: false,
    };
}
