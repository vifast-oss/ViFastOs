// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`KernelStore`** — trait **đóng** mô tả **100 % bảng** của [`crate::Machine`] (KT-408, D2-259).
//!
//! # Vì sao có tệp này
//!
//! Trước KT-408 `Machine` khai báo **trực tiếp** 23 bảng tĩnh (`[Meta; 256]`, `[Cap; 512]`, …) nên
//! chỉ có **một** mô hình kernel: `ViFastOs-Mission` với bảng `.bss` cố định. Kiến trúc đích
//! (`D2-259` mục 5, `B2`) cần **Lõi Kép**:
//!
//! | Mô hình | `S` | Bảng | Dùng |
//! |---|---|---|---|
//! | `ViFastOs-Mission` | [`StaticStore`] | mảng tĩnh `.bss`, biên `MAX_*` | ảnh sản phẩm hiện tại |
//! | `ViFastOs-General` | `UntypedStore` (`KT-410`) | đặt trong `Untyped` + `BootInfo` Arena | mở rộng động |
//!
//! KT-408 chỉ cài **nửa Mission**: trait + `StaticStore` + tham số hóa `Machine<S, O>`. `KT-410`
//! thêm `UntypedStore` **mà không sửa** `Machine`/`vifast-core` lần nữa.
//!
//! # Thiết kế: `S` là **nhà cung cấp bảng ở mức kiểu**, không phải giá trị
//!
//! `Machine<S, O>` **không** giữ một trường `store: S` — nó giữ **đúng** các trường bảng cũ, chỉ
//! đổi **kiểu** sang kiểu liên kết của `S`:
//!
//! ```text
//! pub struct Machine<S: KernelStore = StaticStore, O: CoreObserver = TestObserver> {
//!     pub(crate) objects: S::Objects,   // = Table<Meta, MAX_OBJECTS> với StaticStore
//!     …
//! }
//! ```
//!
//! Nhờ vậy:
//!
//! - **0 sửa** ~250 điểm truy cập `self.objects[i]`/`self.caps.get_mut(..)`/`self.harts.iter_mut()`
//!   — [`Table`] `Deref` về `[T]` nên **mọi** thao tác slice cũ chạy nguyên;
//! - `size_of::<Machine>()` **không đổi** (pin harness `184712`, `D2-84`/`D2-252`) vì
//!   `Table<T, N>` là `#[repr(transparent)]` trên `[T; N]`;
//! - `Machine` vẫn `Copy` (mọi kiểu liên kết `Copy`);
//! - `StaticStore` là **ZST** ⇒ 0 B, không có trạng thái thứ hai để lệch.
//!
//! # Hợp đồng
//!
//! - **Đóng** (`sealed`): chỉ `StaticStore` (KT-408) và `UntypedStore` (`KT-410`) cài được — không
//!   ai ngoài crate thêm mô hình thứ ba làm lệch ngữ nghĩa.
//! - **An toàn**: `#![forbid(unsafe_code)]` của crate áp nguyên; **0** phụ thuộc ngoài.
//! - Mỗi bảng có **đúng** biên `MAX_*` của [`crate::machine`] (`check_kernel.py` đọc các hằng đó
//!   theo **tên**, nên chúng **không** được dời khỏi `machine.rs`).
//! - `const *_EMPTY` là đường dựng **`const fn`** cho `.bss` của kernel
//!   ([`crate::Machine::with_observer`]) — `Default::default()` không dùng được trong `const fn`.

use core::fmt::Debug;
use core::ops::{Deref, DerefMut};

use crate::arena::Region;
use crate::machine::{
    Cap, Meta, MAX_CAPS, MAX_ENDPOINTS, MAX_HARTS, MAX_NOTIFICATIONS, MAX_OBJECTS,
    MAX_QUEUED_MESSAGES, MAX_REPLIES, MAX_SCHED_CONTEXTS, MAX_TCBS, MAX_UNTYPEDS,
};
use crate::objstate::{
    AddrWord, EndpointData, NotificationData, QueuedMsg, ReplyData, WaitNode, MAX_ADDR_WORDS,
    MAX_WAIT_NODES, NODE_NONE, WAITADDR_BUCKETS,
};
use crate::perhart::PerHartState;
use crate::slotmem::{SlotMem, HARNESS_SLOT_WORDS};
use crate::spec::SLOT_BYTES;
use crate::tcbdata::{ScData, TcbData};

/// **Bảng tĩnh** `[T; N]` có ngữ nghĩa **slice** — kiểu bảng của [`StaticStore`].
///
/// `#[repr(transparent)]` trên `[T; N]` ⇒ `size_of`/`align` **y hệt** mảng trần (pin
/// `size_of::<Machine>()` không đổi). `Deref`/`DerefMut` về `[T]` cho **mọi** thao tác cũ
/// (`len`, `get`, `get_mut`, `iter`, `iter_mut`, chỉ số `[]`, ép `&[T]`) chạy **nguyên văn** —
/// đó là lý do KT-408 không phải sửa ~250 điểm truy cập.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Table<T, const N: usize>([T; N]);

impl<T, const N: usize> Table<T, N> {
    /// Bọc một mảng tĩnh (dùng trong `const` của [`StaticStore`]).
    pub const fn new(inner: [T; N]) -> Self {
        Self(inner)
    }
}

impl<T, const N: usize> Deref for Table<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T, const N: usize> DerefMut for Table<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.0
    }
}

impl<T, const N: usize> Default for Table<T, N>
where
    T: Copy + Default,
{
    fn default() -> Self {
        Self([T::default(); N])
    }
}

/// **Cửa sổ byte slot** — 3 thao tác `Machine` dùng trên bảng slot (KT-408).
///
/// Tách thành trait để [`KernelStore::Slots`] **không** bị đóng cứng vào [`SlotMem`]: `KT-410`
/// (`UntypedStore`) có thể cấp cửa sổ slot từ `BootInfo` Arena mà `Machine` không đổi một dòng.
pub trait SlotWindow: Copy + Debug {
    /// Đọc word thứ `i` của cửa sổ bắt đầu `base` (ngoài dải ⇒ 0, fail-closed).
    fn get(&self, base: usize, i: usize) -> u32;
    /// Ghi word thứ `i` của cửa sổ bắt đầu `base` (ngoài dải ⇒ bỏ, fail-closed).
    fn set(&mut self, base: usize, i: usize, value: u32);
    /// Zero `n` word từ `base` (finalise CNode trả slot về 0).
    fn fill_zero(&mut self, base: usize, n: usize);
}

impl SlotWindow for SlotMem {
    fn get(&self, base: usize, i: usize) -> u32 {
        SlotMem::get(self, base, i)
    }

    fn set(&mut self, base: usize, i: usize, value: u32) {
        SlotMem::set(self, base, i, value);
    }

    fn fill_zero(&mut self, base: usize, n: usize) {
        SlotMem::fill_zero(self, base, n);
    }
}

/// **Arena harness** — 2 thao tác `Machine` dùng trên arena CNode kernel cấp (KT-408).
pub trait HarnessArena: Copy + Debug {
    /// Hỏi offset cấp được cho `len` byte căn `align` (không cấp).
    fn probe(&self, len: usize, align: usize) -> Option<usize>;
    /// Cấp `len` byte căn `align`.
    fn alloc(&mut self, len: usize, align: usize) -> Option<usize>;
}

impl HarnessArena for Region {
    fn probe(&self, len: usize, align: usize) -> Option<usize> {
        Region::probe(self, len, align)
    }

    fn alloc(&mut self, len: usize, align: usize) -> Option<usize> {
        Region::alloc(self, len, align)
    }
}

/// **Trait đóng** — nhà cung cấp **toàn bộ** bảng của [`crate::Machine`] (KT-408).
///
/// 23 bảng, đúng bảng `Machine` khai báo trước KT-408:
///
/// | Nhóm | Kiểu liên kết |
/// |---|---|
/// | Untyped / object / cap | [`Self::Regions`], [`Self::Objects`], [`Self::Caps`] |
/// | Cửa sổ byte slot | [`Self::Slots`] |
/// | Arena harness | [`Self::Harness`] |
/// | 5 thân ABI | [`Self::Tcbs`], [`Self::Scs`], [`Self::Endpoints`], [`Self::Replies`], [`Self::Notifications`] |
/// | Free-list chỉ số | [`Self::TcbFree`], [`Self::ScFree`], [`Self::EndpointFree`], [`Self::ReplyFree`], [`Self::NotificationFree`] |
/// | Hàng gửi IPC | [`Self::IpcMsgs`], [`Self::IpcMsgFree`] |
/// | WaitOnAddress | [`Self::AddrWords`], [`Self::WaitNodes`], [`Self::WaitFree`], [`Self::WaitBuckets`], [`Self::WaitBucketsTail`] |
/// | Theo hart | [`Self::Harts`] |
///
/// Mỗi kiểu liên kết phải `Deref<Target = [T]> + DerefMut` để `Machine` giữ **nguyên văn** mã
/// truy cập cũ; `const *_EMPTY` là đường dựng `const fn` cho `.bss`.
pub trait KernelStore: sealed::Sealed + Copy + Debug + Sized + 'static {
    /// `regions[16]` — arena Untyped.
    type Regions: Copy + Debug + Deref<Target = [Region]> + DerefMut;
    /// `objects[256]` — `Meta` (48 B) của mọi object.
    type Objects: Copy + Debug + Deref<Target = [Meta]> + DerefMut;
    /// `caps[512]` — bảng capability.
    type Caps: Copy + Debug + Deref<Target = [Cap]> + DerefMut;
    /// Cửa sổ byte slot CNode (`SlotMem`, 16.384 B) — xem [`SlotWindow`].
    type Slots: SlotWindow;
    /// Arena logic của CNode **harness** (kernel cấp, không có byte Untyped) — xem
    /// [`HarnessArena`].
    type Harness: HarnessArena;
    /// `tcbs[64]` — thân TCB.
    type Tcbs: Copy + Debug + Deref<Target = [TcbData]> + DerefMut;
    /// `tcb_free[64]` — free-list chỉ số TCB.
    type TcbFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `scs[32]` — thân SchedContext.
    type Scs: Copy + Debug + Deref<Target = [ScData]> + DerefMut;
    /// `sc_free[32]`.
    type ScFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `endpoints[16]` — thân Endpoint.
    type Endpoints: Copy + Debug + Deref<Target = [EndpointData]> + DerefMut;
    /// `endpoint_free[16]`.
    type EndpointFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `replies[32]` — thân Reply.
    type Replies: Copy + Debug + Deref<Target = [ReplyData]> + DerefMut;
    /// `reply_free[32]`.
    type ReplyFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `notifications[16]` — thân Notification.
    type Notifications: Copy + Debug + Deref<Target = [NotificationData]> + DerefMut;
    /// `notification_free[16]`.
    type NotificationFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `ipc_msgs[24]` — pool message đang chờ trong hàng gửi Endpoint.
    type IpcMsgs: Copy + Debug + Deref<Target = [QueuedMsg]> + DerefMut;
    /// `ipc_msg_free[24]`.
    type IpcMsgFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `addr_words[64]` — bảng `(frame, offset) -> u64` của WaitOnAddress.
    type AddrWords: Copy + Debug + Deref<Target = [AddrWord]> + DerefMut;
    /// `wait_nodes[64]` — waiter của WaitOnAddress.
    type WaitNodes: Copy + Debug + Deref<Target = [WaitNode]> + DerefMut;
    /// `wait_free[64]`.
    type WaitFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `wait_buckets[64]` — đầu bucket băm.
    type WaitBuckets: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `wait_buckets_tail[64]` — đuôi bucket băm.
    type WaitBucketsTail: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `harts[8]` — [`PerHartState`] (sched/published/mail/tlb_pending/trace).
    type Harts: Copy + Debug + Deref<Target = [PerHartState]> + DerefMut;

    /// Giá trị rỗng **const** của từng bảng (đường dựng `.bss` của kernel).
    const REGIONS_EMPTY: Self::Regions;
    /// Xem [`Self::REGIONS_EMPTY`].
    const OBJECTS_EMPTY: Self::Objects;
    /// Xem [`Self::REGIONS_EMPTY`].
    const CAPS_EMPTY: Self::Caps;
    /// Xem [`Self::REGIONS_EMPTY`].
    const SLOTS_EMPTY: Self::Slots;
    /// Xem [`Self::REGIONS_EMPTY`].
    const HARNESS_EMPTY: Self::Harness;
    /// Xem [`Self::REGIONS_EMPTY`].
    const TCBS_EMPTY: Self::Tcbs;
    /// Xem [`Self::REGIONS_EMPTY`].
    const TCB_FREE_EMPTY: Self::TcbFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const SCS_EMPTY: Self::Scs;
    /// Xem [`Self::REGIONS_EMPTY`].
    const SC_FREE_EMPTY: Self::ScFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const ENDPOINTS_EMPTY: Self::Endpoints;
    /// Xem [`Self::REGIONS_EMPTY`].
    const ENDPOINT_FREE_EMPTY: Self::EndpointFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const REPLIES_EMPTY: Self::Replies;
    /// Xem [`Self::REGIONS_EMPTY`].
    const REPLY_FREE_EMPTY: Self::ReplyFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const NOTIFICATIONS_EMPTY: Self::Notifications;
    /// Xem [`Self::REGIONS_EMPTY`].
    const NOTIFICATION_FREE_EMPTY: Self::NotificationFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const IPC_MSGS_EMPTY: Self::IpcMsgs;
    /// Xem [`Self::REGIONS_EMPTY`].
    const IPC_MSG_FREE_EMPTY: Self::IpcMsgFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const ADDR_WORDS_EMPTY: Self::AddrWords;
    /// Xem [`Self::REGIONS_EMPTY`].
    const WAIT_NODES_EMPTY: Self::WaitNodes;
    /// Xem [`Self::REGIONS_EMPTY`].
    const WAIT_FREE_EMPTY: Self::WaitFree;
    /// Xem [`Self::REGIONS_EMPTY`].
    const WAIT_BUCKETS_EMPTY: Self::WaitBuckets;
    /// Xem [`Self::REGIONS_EMPTY`].
    const WAIT_BUCKETS_TAIL_EMPTY: Self::WaitBucketsTail;
    /// Xem [`Self::REGIONS_EMPTY`].
    const HARTS_EMPTY: Self::Harts;
}

/// **`ViFastOs-Mission`** — bảng tĩnh `.bss` với biên `MAX_*` của [`crate::machine`] (KT-408).
///
/// ZST: **0 B**, không trạng thái, không `unsafe`. Đây là mô hình của **ảnh sản phẩm** hiện tại
/// (`D2-259` mục 5) — `KT-410` thêm `UntypedStore` cho `ViFastOs-General` **song song**, không
/// thay thế.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StaticStore;

impl StaticStore {
    /// Giá trị duy nhất của nhà cung cấp bảng Mission (ZST).
    pub const EMPTY: Self = Self;
}

impl sealed::Sealed for StaticStore {}

impl KernelStore for StaticStore {
    type Regions = Table<Region, MAX_UNTYPEDS>;
    type Objects = Table<Meta, MAX_OBJECTS>;
    type Caps = Table<Cap, MAX_CAPS>;
    type Slots = SlotMem;
    type Harness = Region;
    type Tcbs = Table<TcbData, MAX_TCBS>;
    type TcbFree = Table<u16, MAX_TCBS>;
    type Scs = Table<ScData, MAX_SCHED_CONTEXTS>;
    type ScFree = Table<u16, MAX_SCHED_CONTEXTS>;
    type Endpoints = Table<EndpointData, MAX_ENDPOINTS>;
    type EndpointFree = Table<u16, MAX_ENDPOINTS>;
    type Replies = Table<ReplyData, MAX_REPLIES>;
    type ReplyFree = Table<u16, MAX_REPLIES>;
    type Notifications = Table<NotificationData, MAX_NOTIFICATIONS>;
    type NotificationFree = Table<u16, MAX_NOTIFICATIONS>;
    type IpcMsgs = Table<QueuedMsg, MAX_QUEUED_MESSAGES>;
    type IpcMsgFree = Table<u16, MAX_QUEUED_MESSAGES>;
    type AddrWords = Table<AddrWord, MAX_ADDR_WORDS>;
    type WaitNodes = Table<WaitNode, MAX_WAIT_NODES>;
    type WaitFree = Table<u16, MAX_WAIT_NODES>;
    type WaitBuckets = Table<u16, WAITADDR_BUCKETS>;
    type WaitBucketsTail = Table<u16, WAITADDR_BUCKETS>;
    type Harts = Table<PerHartState, MAX_HARTS>;

    const REGIONS_EMPTY: Self::Regions = Table::new([Region::EMPTY; MAX_UNTYPEDS]);
    const OBJECTS_EMPTY: Self::Objects = Table::new([Meta::EMPTY; MAX_OBJECTS]);
    const CAPS_EMPTY: Self::Caps = Table::new([Cap::EMPTY; MAX_CAPS]);
    const SLOTS_EMPTY: Self::Slots = SlotMem::EMPTY;
    const HARNESS_EMPTY: Self::Harness = Region::new(0, HARNESS_SLOT_WORDS * SLOT_BYTES);
    const TCBS_EMPTY: Self::Tcbs = Table::new([TcbData::EMPTY; MAX_TCBS]);
    const TCB_FREE_EMPTY: Self::TcbFree = Table::new([0; MAX_TCBS]);
    const SCS_EMPTY: Self::Scs = Table::new([ScData::EMPTY; MAX_SCHED_CONTEXTS]);
    const SC_FREE_EMPTY: Self::ScFree = Table::new([0; MAX_SCHED_CONTEXTS]);
    const ENDPOINTS_EMPTY: Self::Endpoints = Table::new([EndpointData::EMPTY; MAX_ENDPOINTS]);
    const ENDPOINT_FREE_EMPTY: Self::EndpointFree = Table::new([0; MAX_ENDPOINTS]);
    const REPLIES_EMPTY: Self::Replies = Table::new([ReplyData::EMPTY; MAX_REPLIES]);
    const REPLY_FREE_EMPTY: Self::ReplyFree = Table::new([0; MAX_REPLIES]);
    const NOTIFICATIONS_EMPTY: Self::Notifications = Table::new(
        [NotificationData {
            word: 0,
            badge: 0,
            waiter: None,
            bound_sc: None,
        }; MAX_NOTIFICATIONS],
    );
    const NOTIFICATION_FREE_EMPTY: Self::NotificationFree = Table::new([0; MAX_NOTIFICATIONS]);
    const IPC_MSGS_EMPTY: Self::IpcMsgs = Table::new([QueuedMsg::EMPTY; MAX_QUEUED_MESSAGES]);
    const IPC_MSG_FREE_EMPTY: Self::IpcMsgFree = Table::new([0; MAX_QUEUED_MESSAGES]);
    const ADDR_WORDS_EMPTY: Self::AddrWords = Table::new([AddrWord::EMPTY; MAX_ADDR_WORDS]);
    const WAIT_NODES_EMPTY: Self::WaitNodes = Table::new([WaitNode::EMPTY; MAX_WAIT_NODES]);
    const WAIT_FREE_EMPTY: Self::WaitFree = Table::new([0; MAX_WAIT_NODES]);
    const WAIT_BUCKETS_EMPTY: Self::WaitBuckets = Table::new([NODE_NONE; WAITADDR_BUCKETS]);
    const WAIT_BUCKETS_TAIL_EMPTY: Self::WaitBucketsTail =
        Table::new([NODE_NONE; WAITADDR_BUCKETS]);
    const HARTS_EMPTY: Self::Harts = Table::new([PerHartState::new(); MAX_HARTS]);
}

/// **Trait đóng** — không ai ngoài `vifast-core` cài [`KernelStore`] (mô hình thứ ba sẽ làm lệch
/// ngữ nghĩa `D2-259` mục 5). `KT-410` cài `UntypedStore` **trong** crate này.
mod sealed {
    /// Chỉ `vifast-core` cài được (xem [`super::KernelStore`]).
    pub trait Sealed {}
}

// **KT-408** — bằng chứng **0 B** của nhà cung cấp bảng Mission (const assert: chạy cả khi build
// bare-metal, không chỉ `cargo test`).
const _: () = assert!(
    core::mem::size_of::<StaticStore>() == 0,
    "StaticStore phải 0 B (ZST) — mô hình Mission không trả byte cho nhà cung cấp bảng"
);
// **KT-408** — `Table` phải **trong suốt**: cỡ/align y hệt mảng trần, nếu không pin
// `size_of::<Machine>()` (184712) sẽ lệch.
const _: () = assert!(
    core::mem::size_of::<Table<Meta, MAX_OBJECTS>>() == core::mem::size_of::<[Meta; MAX_OBJECTS]>(),
    "Table<T, N> phải cùng cỡ [T; N] (repr(transparent))"
);
