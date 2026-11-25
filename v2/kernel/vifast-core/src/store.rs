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

// =============================================================================================
// **KT-410** — `ViFastOs-General`: `UntypedStore` (`D2-259` mục 5, `B2`)
// =============================================================================================

/// Bộ biên bảng của mô hình **General** (`ViFastOs-General`) — `D2-259` mục 5.
///
/// # Vì sao là **hằng biên** chứ không phải bảng cấp phát động
///
/// `Machine<S, O>` **không** giữ trường `store: S` (xem đầu tệp): `S` chỉ chọn **kiểu** bảng, và
/// mọi bảng là trường của `Machine`. Vì vậy bảng của `S` phải dựng được bằng **`const`**
/// (`S::*_EMPTY`) — tức là `Table<T, N>` với `N` biết lúc biên dịch. `UntypedStore` vì thế dùng
/// **bộ biên General** rộng hơn `MAX_*` của Mission; `Machine<UntypedStore>` **lớn hơn** nhiều
/// (≈ 0,6 MiB) và được đặt **trong RAM Untyped** qua `physmap` bởi
/// `vifast-plat-common::untyped_store` + `vifast-objmem` (không nằm trong `.bss` của ảnh).
///
/// # Hệ quả đã biết (ghi rõ, không giấu)
///
/// `HartSched::next` (`sched.rs`) vẫn là `[u16; MAX_OBJECTS + 1]` **nằm trong `Machine`** ⇒
/// không nới được nếu không phá pin `size_of::<Machine<StaticStore>>() ≤ 184.832 B`. Vì vậy mô
/// hình General **chỉ lập lịch được** TCB có `ObjId ≤ MAX_OBJECTS = 256`; `HartSched::contains`
/// trả `false` (fail-closed) và `enqueue` `debug_assert` chặn. Tạo/`Revoke` TCB **không** đi qua
/// `HartSched` nên tiêu chí `≥ 512 TCB` vẫn đo được. Việc tổng quát hoá `PerHartState` theo
/// `S` thuộc `KT-412`/`KT-413` (tách `PerHartState` khỏi Global Store).
pub const GENERAL_MAX_OBJECTS: usize = 1024;
/// Số cap tối đa của mô hình General (Mission: `MAX_CAPS = 512`).
pub const GENERAL_MAX_CAPS: usize = 1024;
/// Số Untyped tối đa của mô hình General (giữ `16` như Mission: `REGION_SLOT_WORDS` chia theo
/// `MAX_UNTYPEDS`).
pub const GENERAL_MAX_UNTYPEDS: usize = MAX_UNTYPEDS;
/// Số body TCB tối đa của mô hình General (Mission: `MAX_TCBS = 64`) — tiêu chí `KT-410` đòi
/// **≥ 512**.
pub const GENERAL_MAX_TCBS: usize = 512;
/// Số body SchedContext tối đa của mô hình General (Mission: `MAX_SCHED_CONTEXTS = 32`).
pub const GENERAL_MAX_SCHED_CONTEXTS: usize = 64;
/// Số body Endpoint tối đa của mô hình General (Mission: `MAX_ENDPOINTS = 16`).
pub const GENERAL_MAX_ENDPOINTS: usize = 16;
/// Số body Reply tối đa của mô hình General (Mission: `MAX_REPLIES = 32`).
pub const GENERAL_MAX_REPLIES: usize = 32;
/// Số body Notification tối đa của mô hình General (Mission: `MAX_NOTIFICATIONS = 16`).
pub const GENERAL_MAX_NOTIFICATIONS: usize = 16;
/// Pool message đang chờ của mô hình General (Mission: `MAX_QUEUED_MESSAGES = 24`).
pub const GENERAL_MAX_QUEUED_MESSAGES: usize = 24;
/// Bảng `(frame, offset) -> u64` của WaitOnAddress, mô hình General (Mission: `64`).
pub const GENERAL_MAX_ADDR_WORDS: usize = 64;
/// Số nút waiter WaitOnAddress của mô hình General — **bằng `GENERAL_MAX_TCBS`** (mỗi TCB tối đa
/// một nút, cùng quy tắc `MAX_WAIT_NODES = MAX_TCBS` của Mission).
pub const GENERAL_MAX_WAIT_NODES: usize = GENERAL_MAX_TCBS;
/// Số hart tối đa của mô hình General (giữ `8` như Mission — `MAX_HARTS` là biên mô hình).
pub const GENERAL_MAX_HARTS: usize = MAX_HARTS;

/// **`ViFastOs-General`** — nhà cung cấp bảng **rộng** cho chế độ `General` (`D2-259` mục 5).
///
/// ZST như [`StaticStore`]: **0 B** trạng thái, không `unsafe`, không cấp phát. Khác biệt **duy
/// nhất** so với Mission là **cỡ bảng** (`GENERAL_*` thay `MAX_*`), nên **cùng một mã lõi**
/// (`Machine<S, O>`) chạy cả hai mô hình — đó là toàn bộ mục đích của `B2`.
///
/// # Vì sao cài **trong** crate này
///
/// [`KernelStore`] là trait **đóng** (`mod sealed`): mô hình thứ ba ngoài crate sẽ làm lệch ngữ
/// nghĩa `D2-259` mục 5. `KT-408` đã ghi sẵn *"`KT-410` cài `UntypedStore` **trong** crate này"*.
/// Phần **chạm bộ nhớ vật lý** (đặt `Machine<UntypedStore>` vào RAM Untyped qua `physmap`, cắt
/// arena từ `BootInfo`) nằm ở `vifast-plat-common::untyped_store` + `vifast-objmem` — hai nơi
/// **được phép** `unsafe` (D2-17) và `vifast-core` giữ nguyên `#![forbid(unsafe_code)]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UntypedStore;

impl UntypedStore {
    /// Giá trị duy nhất của nhà cung cấp bảng General (ZST).
    pub const EMPTY: Self = Self;
}

impl sealed::Sealed for UntypedStore {}

impl KernelStore for UntypedStore {
    type Regions = Table<Region, GENERAL_MAX_UNTYPEDS>;
    type Objects = Table<Meta, GENERAL_MAX_OBJECTS>;
    type Caps = Table<Cap, GENERAL_MAX_CAPS>;
    type Slots = SlotMem;
    type Harness = Region;
    type Tcbs = Table<TcbData, GENERAL_MAX_TCBS>;
    type TcbFree = Table<u16, GENERAL_MAX_TCBS>;
    type Scs = Table<ScData, GENERAL_MAX_SCHED_CONTEXTS>;
    type ScFree = Table<u16, GENERAL_MAX_SCHED_CONTEXTS>;
    type Endpoints = Table<EndpointData, GENERAL_MAX_ENDPOINTS>;
    type EndpointFree = Table<u16, GENERAL_MAX_ENDPOINTS>;
    type Replies = Table<ReplyData, GENERAL_MAX_REPLIES>;
    type ReplyFree = Table<u16, GENERAL_MAX_REPLIES>;
    type Notifications = Table<NotificationData, GENERAL_MAX_NOTIFICATIONS>;
    type NotificationFree = Table<u16, GENERAL_MAX_NOTIFICATIONS>;
    type IpcMsgs = Table<QueuedMsg, GENERAL_MAX_QUEUED_MESSAGES>;
    type IpcMsgFree = Table<u16, GENERAL_MAX_QUEUED_MESSAGES>;
    type AddrWords = Table<AddrWord, GENERAL_MAX_ADDR_WORDS>;
    type WaitNodes = Table<WaitNode, GENERAL_MAX_WAIT_NODES>;
    type WaitFree = Table<u16, GENERAL_MAX_WAIT_NODES>;
    type WaitBuckets = Table<u16, WAITADDR_BUCKETS>;
    type WaitBucketsTail = Table<u16, WAITADDR_BUCKETS>;
    type Harts = Table<PerHartState, GENERAL_MAX_HARTS>;

    const REGIONS_EMPTY: Self::Regions = Table::new([Region::EMPTY; GENERAL_MAX_UNTYPEDS]);
    const OBJECTS_EMPTY: Self::Objects = Table::new([Meta::EMPTY; GENERAL_MAX_OBJECTS]);
    const CAPS_EMPTY: Self::Caps = Table::new([Cap::EMPTY; GENERAL_MAX_CAPS]);
    const SLOTS_EMPTY: Self::Slots = SlotMem::EMPTY;
    const HARNESS_EMPTY: Self::Harness = Region::new(0, HARNESS_SLOT_WORDS * SLOT_BYTES);
    const TCBS_EMPTY: Self::Tcbs = Table::new([TcbData::EMPTY; GENERAL_MAX_TCBS]);
    const TCB_FREE_EMPTY: Self::TcbFree = Table::new([0; GENERAL_MAX_TCBS]);
    const SCS_EMPTY: Self::Scs = Table::new([ScData::EMPTY; GENERAL_MAX_SCHED_CONTEXTS]);
    const SC_FREE_EMPTY: Self::ScFree = Table::new([0; GENERAL_MAX_SCHED_CONTEXTS]);
    const ENDPOINTS_EMPTY: Self::Endpoints =
        Table::new([EndpointData::EMPTY; GENERAL_MAX_ENDPOINTS]);
    const ENDPOINT_FREE_EMPTY: Self::EndpointFree = Table::new([0; GENERAL_MAX_ENDPOINTS]);
    const REPLIES_EMPTY: Self::Replies = Table::new([ReplyData::EMPTY; GENERAL_MAX_REPLIES]);
    const REPLY_FREE_EMPTY: Self::ReplyFree = Table::new([0; GENERAL_MAX_REPLIES]);
    const NOTIFICATIONS_EMPTY: Self::Notifications = Table::new(
        [NotificationData {
            word: 0,
            badge: 0,
            waiter: None,
            bound_sc: None,
        }; GENERAL_MAX_NOTIFICATIONS],
    );
    const NOTIFICATION_FREE_EMPTY: Self::NotificationFree =
        Table::new([0; GENERAL_MAX_NOTIFICATIONS]);
    const IPC_MSGS_EMPTY: Self::IpcMsgs =
        Table::new([QueuedMsg::EMPTY; GENERAL_MAX_QUEUED_MESSAGES]);
    const IPC_MSG_FREE_EMPTY: Self::IpcMsgFree = Table::new([0; GENERAL_MAX_QUEUED_MESSAGES]);
    const ADDR_WORDS_EMPTY: Self::AddrWords = Table::new([AddrWord::EMPTY; GENERAL_MAX_ADDR_WORDS]);
    const WAIT_NODES_EMPTY: Self::WaitNodes = Table::new([WaitNode::EMPTY; GENERAL_MAX_WAIT_NODES]);
    const WAIT_FREE_EMPTY: Self::WaitFree = Table::new([0; GENERAL_MAX_WAIT_NODES]);
    const WAIT_BUCKETS_EMPTY: Self::WaitBuckets = Table::new([NODE_NONE; WAITADDR_BUCKETS]);
    const WAIT_BUCKETS_TAIL_EMPTY: Self::WaitBucketsTail =
        Table::new([NODE_NONE; WAITADDR_BUCKETS]);
    const HARTS_EMPTY: Self::Harts = Table::new([PerHartState::new(); GENERAL_MAX_HARTS]);
}

// **KT-410** — bằng chứng **0 B** của nhà cung cấp bảng General (const assert: chạy cả khi build
// bare-metal, không chỉ `cargo test`).
const _: () = assert!(
    core::mem::size_of::<UntypedStore>() == 0,
    "UntypedStore phải 0 B (ZST) — mô hình General không trả byte cho nhà cung cấp bảng"
);
// **KT-410** — bộ biên General phải **thật sự rộng hơn** Mission ở đúng hai bảng mà tiêu chí
// nghiệm thu đo (`≥ 512 TCB`, `≥ 1.024 Cap`); nếu ai hạ chúng xuống, cổng đỏ ngay tại biên dịch.
const _: () = assert!(
    GENERAL_MAX_TCBS >= 512,
    "KT-410: GENERAL_MAX_TCBS phải >= 512 (tiêu chí nghiệm thu)"
);
const _: () = assert!(
    GENERAL_MAX_CAPS >= 1024,
    "KT-410: GENERAL_MAX_CAPS phải >= 1024 (tiêu chí nghiệm thu)"
);
const _: () = assert!(
    GENERAL_MAX_OBJECTS > MAX_OBJECTS,
    "KT-410: bảng object General phải rộng hơn Mission"
);

// =============================================================================================
// **KT-410** — test host cho chế độ `ViFastOs-General` (`UntypedStore`) + bằng chứng hai mô
// hình của `B2` (`D2-259` mục 5). Khai **trong** `store.rs` (không phải `lib.rs`) để
// `check-api-boundary.py` (KT-241) vẫn chạy được trên worktree bẩn — nó chỉ chèn probe vào
// `lib.rs` và fail-closed nếu tệp đó đã bẩn.
// =============================================================================================
#[cfg(test)]
mod kt410_tests {
    extern crate std;

    use core::mem::size_of;

    use crate::error::{KError, ObjType, SlotRef};
    use crate::machine::{Machine, MAX_TCBS};
    use crate::observer::{NoopObserver, TestObserver};
    use crate::store::{StaticStore, UntypedStore};

    /// Mô hình **sản phẩm** (`Mission`): bảng tĩnh `.bss`, biên `MAX_*`.
    type Mission = Machine<StaticStore, TestObserver>;
    /// Mô hình **General**: bảng rộng hơn (`GENERAL_*`), cùng mã lõi.
    type General = Machine<UntypedStore, NoopObserver>;

    /// Dựng cảnh tối thiểu cho một mô hình: 1 Untyped 16 MiB + 1 CNode **1.022** slot (harness —
    /// `HARNESS_SLOT_WORDS = 1024` word, `cnode_bytes(1022) = 64 + 1022×48 = 49.120 ≤ 49.152`), cap
    /// Untyped đặt ở slot 0. Trả `(machine, cnode, untyped)`.
    ///
    /// 16 MiB đủ cho **512** TCB (512 × `size_of::<TcbData>()` = 512 × 824 = 421.888 B) + căn lề.
    fn rig<S: crate::store::KernelStore, O: crate::observer::CoreObserver>(
    ) -> (Machine<S, O>, u32, u32) {
        let mut m = Machine::<S, O>::with_observer(1);
        let u = m.add_untyped(0x8000_0000, 16 << 20);
        let c = m.add_cnode(1022);
        let u_cap = m.root_cap(u);
        m.put_cap(c, 0, u_cap).expect("cap Untyped vào slot 0");
        (m, c, u)
    }

    /// Đếm **object TCB còn sống** bằng API công khai (không thêm accessor vào sản phẩm).
    fn dem_tcb<S: crate::store::KernelStore, O: crate::observer::CoreObserver>(
        m: &Machine<S, O>,
    ) -> usize {
        (1..=m.object_count() as u32)
            .filter(|&id| m.object_alive(id) && m.object_type(id) == Some(ObjType::Tcb))
            .count()
    }

    /// **Ca 1** — `UntypedStore` là ZST + cài `KernelStore`; `Machine<UntypedStore, _>` dựng được
    /// bằng **đúng** đường `with_observer` mà `StaticStore` dùng (không constructor mới).
    #[test]
    fn kt410_untyped_store_la_mot_kernel_store() {
        assert_eq!(
        size_of::<UntypedStore>(),
        0,
        "UntypedStore phải là ZST như StaticStore (S là bộ chọn bảng ở mức kiểu, Machine không giữ S)"
    );
        let m = General::with_observer(1);
        assert_eq!(m.n_harts(), 1, "dựng được 1 hart");
        assert_eq!(m.total_used(), 0, "mô hình rỗng: watermark 0");
        // Cùng mã lõi: `Machine<StaticStore>` vẫn dựng được y nguyên.
        let s = Mission::with_observer(1);
        assert_eq!(s.n_harts(), 1);
        assert_eq!(s.total_used(), 0);
    }

    /// **Ca 2** — `StaticStore` **chặn** TCB thứ **65**: `MAX_TCBS = 64`, TCB 65 trả
    /// `NotEnoughMemory` và **không** đốt byte/cap/object (nguyên tử).
    #[test]
    fn kt410_static_store_chan_tcb_thu_65() {
        let (mut m, c, u) = rig::<StaticStore, TestObserver>();
        for i in 0..MAX_TCBS {
            m.retype(SlotRef::new(c, 0), ObjType::Tcb, 0, 0)
                .unwrap_or_else(|e| panic!("TCB thứ {i} phải cấp được: {e:?}"));
        }
        assert_eq!(dem_tcb(&m), MAX_TCBS, "đã cấp đúng MAX_TCBS = {MAX_TCBS}");
        // Mốc so sánh **ngay trước** lần Retype thứ 65 (sau 64 TCB đã tiêu 64 × 512 = 32.768 B).
        let before_used = m.total_used();
        let before_caps = m.cap_count();
        let before_objects = m.object_count();
        let err = m
            .retype(SlotRef::new(c, 0), ObjType::Tcb, 0, 0)
            .expect_err("TCB thứ 65 phải bị CHẶN");
        assert_eq!(
            err,
            KError::NotEnoughMemory,
            "mã lỗi phải là NotEnoughMemory (hết bảng TCB tĩnh)"
        );
        assert_eq!(dem_tcb(&m), MAX_TCBS, "không tăng bảng TCB sau lỗi");
        assert_eq!(m.total_used(), before_used, "lỗi nguyên tử: không đốt byte");
        assert_eq!(m.cap_count(), before_caps, "lỗi nguyên tử: không đốt cap");
        assert_eq!(
            m.object_count(),
            before_objects,
            "lỗi nguyên tử: không đốt object"
        );
        m.check_bytes(u).expect("bất biến byte của Untyped");
    }

    /// **Ca 3** — **cùng mã lõi**, `UntypedStore` tạo **512 TCB** + **1.024 Cap** rồi `Revoke`
    /// Untyped ⇒ finalise sạch, watermark về **0**.
    ///
    /// Bố cục cap (1.024 cap sống):
    /// - 1 cap gốc CNode + 1 cap gốc Untyped = 2;
    /// - 512 cap TCB do `retype` sinh (đặt vào slot 1..=512);
    /// - 510 cap `copy` từ 510 cap TCB đầu (slot 513..=1.022).
    ///
    /// Chạy trong luồng **stack 32 MiB** vì `Machine<UntypedStore>` ≈ 0,6 MiB (bảng General rộng).
    #[test]
    fn kt410_untyped_store_tao_512_tcb_1024_cap_va_revoke_sach() {
        std::thread::Builder::new()
            .stack_size(32 << 20)
            .spawn(|| {
                const N_TCB: usize = 512;
                // 1.022 slot harness: slot 0 = cap Untyped, slot 1..=512 = 512 cap TCB,
                // slot 513..=1.021 = 509 cap `copy` ⇒ 1 + 512 + 509 = **1.022** cap; thêm **2** cap
                // gốc từ 2 Notification harness ⇒ **1.024** cap sống (đúng tiêu chí).
                const N_COPY: usize = 509;
                let (mut m, c, u) = rig::<UntypedStore, NoopObserver>();
                // 512 TCB từ cap Untyped ở slot 0 — cap mới **chưa** vào slot.
                for i in 0..N_TCB {
                    let cap = m
                        .retype(SlotRef::new(c, 0), ObjType::Tcb, 0, 0)
                        .unwrap_or_else(|e| panic!("TCB thứ {i} phải cấp được: {e:?}"));
                    m.put_cap(c, 1 + i, cap)
                        .unwrap_or_else(|e| panic!("đặt cap TCB {i} vào slot: {e:?}"));
                }
                assert!(
                    dem_tcb(&m) >= 512,
                    "UntypedStore phải tạo ≥ 512 TCB, đo được {}",
                    dem_tcb(&m)
                );
                // 510 cap `copy` ⇒ tổng cap sống = 2 + 512 + 510 = 1.024.
                for i in 0..N_COPY {
                    m.copy(SlotRef::new(c, 1 + i), SlotRef::new(c, 513 + i))
                        .unwrap_or_else(|e| panic!("copy cap {i}: {e:?}"));
                }
                // 2 cap gốc còn lại (harness Notification) ⇒ tổng **1.024** cap sống.
                for _ in 0..2 {
                    let n = m.add_kernel_object(ObjType::Notification);
                    let _ = m.root_cap(n);
                }
                assert!(
                    m.live_cap_count() >= 1024,
                    "UntypedStore phải giữ ≥ 1.024 Cap, đo được {}",
                    m.live_cap_count()
                );
                assert!(m.total_used() > 0, "đã cấp byte từ Untyped");
                m.check_bytes(u).expect("bất biến byte trước Revoke");
                m.check_caps().expect("bất biến cap trước Revoke");
                // Revoke cap Untyped ở slot 0 ⇒ finalise toàn bộ cây con (512 TCB + 510 cap copy).
                let n = m.revoke(c, Some(0)).expect("Revoke Untyped");
                assert!(n > 0, "Revoke phải gỡ cap con, đo được {n}");
                assert_eq!(dem_tcb(&m), 0, "mọi body TCB phải được finalise");
                assert_eq!(
                    m.total_used(),
                    0,
                    "watermark Untyped phải về 0 sau Revoke (không rò rỉ byte)"
                );
                m.check_bytes(u).expect("bất biến byte sau Revoke");
            })
            .expect("spawn luồng stack 32 MiB")
            .join()
            .expect("ca 3 không được panic");
    }

    /// **Ca 4** — pin hai mô hình: `Meta == 48 B`, `Machine<StaticStore>` ≤ 184.832 B (**không**
    /// nới), `Machine<UntypedStore>` **lớn hơn** (bảng General rộng hơn) ⇒ hai mô hình khác nhau
    /// thật.
    #[test]
    fn kt410_pin_hai_mo_hinh() {
        assert_eq!(size_of::<crate::machine::Meta>(), 48, "Meta phải 48 B");
        let mission = size_of::<Machine<StaticStore, NoopObserver>>();
        let general = size_of::<Machine<UntypedStore, NoopObserver>>();
        assert!(
            mission <= 184_832,
            "pin D2-84/KT-312: Machine<StaticStore> = {mission} > 184832"
        );
        assert!(
            general > mission,
            "bảng General phải rộng hơn Mission: general = {general}, mission = {mission}"
        );
        std::println!(
            "# kt410 pin: Meta=48 MachineMission={mission} MachineGeneral={general} \
         tcbs={} caps={} objects={}",
            crate::store::GENERAL_MAX_TCBS,
            crate::store::GENERAL_MAX_CAPS,
            crate::store::GENERAL_MAX_OBJECTS,
        );
    }
}

// =============================================================================================
// **KT-410** — `SimHeapStore`: bảng **cấp phát động** cho mô hình sim (`B2`)
// =============================================================================================

/// **Nhà cung cấp bảng heap** — nửa **kiểu** của [`SimHeapStore`] (`KT-410`).
///
/// # Vì sao tách khỏi [`KernelStore`]
///
/// [`KernelStore`] là trait **đóng** và `vifast-core` giữ `#![forbid(unsafe_code)]`: bảng heap
/// phải là con trỏ thô (`Copy` + `Deref<Target = [T]>` trên vùng `alloc`), tức **`unsafe`** ⇒
/// không viết được trong crate này. Vì vậy:
///
/// - `vifast-core` khai **hợp đồng** [`HeapBacking`] (23 kiểu bảng + 23 hằng rỗng) và cài
///   `KernelStore` cho [`SimHeapStore<B>`] — **giữ seal**;
/// - crate **được phép `unsafe`** (`vifast-objmem`, D2-17) cài [`HeapBacking`] bằng bảng thật
///   trên `alloc` ([`vifast_objmem::HeapTable`]);
/// - `vifast-sim` (dev-dependency `vifast-core`) dùng `Machine<SimHeapStore<…>, TestObserver>`
///   để chạy **vi sai** trên **cùng mã lõi** với `Machine<StaticStore>`.
///
/// Nhờ vậy `B2` còn **đúng 2** mô hình: `vifast-sim` (oracle) + `vifast-core` (sản phẩm) — mô
/// hình thứ ba `vifast-model` đã gỡ, còn `SimHeapStore` chỉ là **một cách cấp bảng khác** của
/// cùng `vifast-core`.
pub trait HeapBacking: Copy + Debug + Sized + 'static {
    /// `regions` — arena Untyped.
    type Regions: Copy + Debug + Deref<Target = [Region]> + DerefMut;
    /// `objects` — `Meta` (48 B).
    type Objects: Copy + Debug + Deref<Target = [Meta]> + DerefMut;
    /// `caps`.
    type Caps: Copy + Debug + Deref<Target = [Cap]> + DerefMut;
    /// Cửa sổ byte slot CNode.
    type Slots: SlotWindow;
    /// Arena logic của CNode harness.
    type Harness: HarnessArena;
    /// `tcbs`.
    type Tcbs: Copy + Debug + Deref<Target = [TcbData]> + DerefMut;
    /// `tcb_free`.
    type TcbFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `scs`.
    type Scs: Copy + Debug + Deref<Target = [ScData]> + DerefMut;
    /// `sc_free`.
    type ScFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `endpoints`.
    type Endpoints: Copy + Debug + Deref<Target = [EndpointData]> + DerefMut;
    /// `endpoint_free`.
    type EndpointFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `replies`.
    type Replies: Copy + Debug + Deref<Target = [ReplyData]> + DerefMut;
    /// `reply_free`.
    type ReplyFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `notifications`.
    type Notifications: Copy + Debug + Deref<Target = [NotificationData]> + DerefMut;
    /// `notification_free`.
    type NotificationFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `ipc_msgs`.
    type IpcMsgs: Copy + Debug + Deref<Target = [QueuedMsg]> + DerefMut;
    /// `ipc_msg_free`.
    type IpcMsgFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `addr_words`.
    type AddrWords: Copy + Debug + Deref<Target = [AddrWord]> + DerefMut;
    /// `wait_nodes`.
    type WaitNodes: Copy + Debug + Deref<Target = [WaitNode]> + DerefMut;
    /// `wait_free`.
    type WaitFree: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `wait_buckets`.
    type WaitBuckets: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `wait_buckets_tail`.
    type WaitBucketsTail: Copy + Debug + Deref<Target = [u16]> + DerefMut;
    /// `harts`.
    type Harts: Copy + Debug + Deref<Target = [PerHartState]> + DerefMut;

    /// Giá trị rỗng **const** của từng bảng (đường dựng `Machine::with_observer`).
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

/// **`SimHeapStore`** — nhà cung cấp bảng **cấp phát động** cho mô hình sim (`KT-410`, `B2`).
///
/// ZST: `B` chỉ là **bộ chọn kiểu bảng** (cùng khuôn [`StaticStore`]/[`UntypedStore`]) — `Machine`
/// không giữ `S`. Bảng thật do `B` cài (xem [`HeapBacking`]); `vifast-objmem::HeapTable` là bản
/// cài tham chiếu (con trỏ thô trên `alloc`, `Copy`, `Deref` về `[T]`).
///
/// # Vì sao cần
///
/// `vifast-sim` là **oracle độc lập** nhưng phải chạy **cùng mã lõi** với sản phẩm để so vi sai
/// (`B2`). Bảng tĩnh `.bss` của [`StaticStore`] khoá cứng biên `MAX_*`; bảng heap cho phép sim
/// dựng **cùng** `Machine` với cỡ bảng tuỳ ý mà **không** thêm mô hình kernel thứ ba.
#[derive(Debug, Clone, Copy)]
pub struct SimHeapStore<B: HeapBacking>(core::marker::PhantomData<B>);

impl<B: HeapBacking> Default for SimHeapStore<B> {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<B: HeapBacking> SimHeapStore<B> {
    /// Giá trị duy nhất của nhà cung cấp bảng heap (ZST).
    pub const EMPTY: Self = Self(core::marker::PhantomData);
}

impl<B: HeapBacking> sealed::Sealed for SimHeapStore<B> {}

impl<B: HeapBacking> KernelStore for SimHeapStore<B> {
    type Regions = B::Regions;
    type Objects = B::Objects;
    type Caps = B::Caps;
    type Slots = B::Slots;
    type Harness = B::Harness;
    type Tcbs = B::Tcbs;
    type TcbFree = B::TcbFree;
    type Scs = B::Scs;
    type ScFree = B::ScFree;
    type Endpoints = B::Endpoints;
    type EndpointFree = B::EndpointFree;
    type Replies = B::Replies;
    type ReplyFree = B::ReplyFree;
    type Notifications = B::Notifications;
    type NotificationFree = B::NotificationFree;
    type IpcMsgs = B::IpcMsgs;
    type IpcMsgFree = B::IpcMsgFree;
    type AddrWords = B::AddrWords;
    type WaitNodes = B::WaitNodes;
    type WaitFree = B::WaitFree;
    type WaitBuckets = B::WaitBuckets;
    type WaitBucketsTail = B::WaitBucketsTail;
    type Harts = B::Harts;

    const REGIONS_EMPTY: Self::Regions = B::REGIONS_EMPTY;
    const OBJECTS_EMPTY: Self::Objects = B::OBJECTS_EMPTY;
    const CAPS_EMPTY: Self::Caps = B::CAPS_EMPTY;
    const SLOTS_EMPTY: Self::Slots = B::SLOTS_EMPTY;
    const HARNESS_EMPTY: Self::Harness = B::HARNESS_EMPTY;
    const TCBS_EMPTY: Self::Tcbs = B::TCBS_EMPTY;
    const TCB_FREE_EMPTY: Self::TcbFree = B::TCB_FREE_EMPTY;
    const SCS_EMPTY: Self::Scs = B::SCS_EMPTY;
    const SC_FREE_EMPTY: Self::ScFree = B::SC_FREE_EMPTY;
    const ENDPOINTS_EMPTY: Self::Endpoints = B::ENDPOINTS_EMPTY;
    const ENDPOINT_FREE_EMPTY: Self::EndpointFree = B::ENDPOINT_FREE_EMPTY;
    const REPLIES_EMPTY: Self::Replies = B::REPLIES_EMPTY;
    const REPLY_FREE_EMPTY: Self::ReplyFree = B::REPLY_FREE_EMPTY;
    const NOTIFICATIONS_EMPTY: Self::Notifications = B::NOTIFICATIONS_EMPTY;
    const NOTIFICATION_FREE_EMPTY: Self::NotificationFree = B::NOTIFICATION_FREE_EMPTY;
    const IPC_MSGS_EMPTY: Self::IpcMsgs = B::IPC_MSGS_EMPTY;
    const IPC_MSG_FREE_EMPTY: Self::IpcMsgFree = B::IPC_MSG_FREE_EMPTY;
    const ADDR_WORDS_EMPTY: Self::AddrWords = B::ADDR_WORDS_EMPTY;
    const WAIT_NODES_EMPTY: Self::WaitNodes = B::WAIT_NODES_EMPTY;
    const WAIT_FREE_EMPTY: Self::WaitFree = B::WAIT_FREE_EMPTY;
    const WAIT_BUCKETS_EMPTY: Self::WaitBuckets = B::WAIT_BUCKETS_EMPTY;
    const WAIT_BUCKETS_TAIL_EMPTY: Self::WaitBucketsTail = B::WAIT_BUCKETS_TAIL_EMPTY;
    const HARTS_EMPTY: Self::Harts = B::HARTS_EMPTY;
}

// =============================================================================================
// **KT-410** — vi sai `SimHeapStore` trên **cùng mã lõi** (`B2`)
// =============================================================================================

/// **KT-410** — backing **test** cho [`SimHeapStore`]: bảng `Table<T, N>` với bộ biên **General**.
///
/// Đây là bản cài **kiểm chứng hợp đồng** [`HeapBacking`]: nó chứng minh `Machine<SimHeapStore<B>>`
/// là một `KernelStore` **thật**, chạy **cùng** `Machine` code với `StaticStore`/`UntypedStore`.
/// Bản cài **heap thật** (`vifast_objmem::HeapTable`, con trỏ thô trên `alloc`) nằm ở
/// `vifast-objmem` — nó cần một đường dựng `Machine` nhận **bảng runtime** (xem ghi chú
/// "còn lại" trong `JOURNAL/kt410-general-store.md`), vì `Machine::with_observer` chỉ dựng được
/// từ `const *_EMPTY`.
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TestHeapBacking;

#[cfg(test)]
impl HeapBacking for TestHeapBacking {
    type Regions = Table<Region, GENERAL_MAX_UNTYPEDS>;
    type Objects = Table<Meta, GENERAL_MAX_OBJECTS>;
    type Caps = Table<Cap, GENERAL_MAX_CAPS>;
    type Slots = SlotMem;
    type Harness = Region;
    type Tcbs = Table<TcbData, GENERAL_MAX_TCBS>;
    type TcbFree = Table<u16, GENERAL_MAX_TCBS>;
    type Scs = Table<ScData, GENERAL_MAX_SCHED_CONTEXTS>;
    type ScFree = Table<u16, GENERAL_MAX_SCHED_CONTEXTS>;
    type Endpoints = Table<EndpointData, GENERAL_MAX_ENDPOINTS>;
    type EndpointFree = Table<u16, GENERAL_MAX_ENDPOINTS>;
    type Replies = Table<ReplyData, GENERAL_MAX_REPLIES>;
    type ReplyFree = Table<u16, GENERAL_MAX_REPLIES>;
    type Notifications = Table<NotificationData, GENERAL_MAX_NOTIFICATIONS>;
    type NotificationFree = Table<u16, GENERAL_MAX_NOTIFICATIONS>;
    type IpcMsgs = Table<QueuedMsg, GENERAL_MAX_QUEUED_MESSAGES>;
    type IpcMsgFree = Table<u16, GENERAL_MAX_QUEUED_MESSAGES>;
    type AddrWords = Table<AddrWord, GENERAL_MAX_ADDR_WORDS>;
    type WaitNodes = Table<WaitNode, GENERAL_MAX_WAIT_NODES>;
    type WaitFree = Table<u16, GENERAL_MAX_WAIT_NODES>;
    type WaitBuckets = Table<u16, WAITADDR_BUCKETS>;
    type WaitBucketsTail = Table<u16, WAITADDR_BUCKETS>;
    type Harts = Table<PerHartState, GENERAL_MAX_HARTS>;

    const REGIONS_EMPTY: Self::Regions = Table::new([Region::EMPTY; GENERAL_MAX_UNTYPEDS]);
    const OBJECTS_EMPTY: Self::Objects = Table::new([Meta::EMPTY; GENERAL_MAX_OBJECTS]);
    const CAPS_EMPTY: Self::Caps = Table::new([Cap::EMPTY; GENERAL_MAX_CAPS]);
    const SLOTS_EMPTY: Self::Slots = SlotMem::EMPTY;
    const HARNESS_EMPTY: Self::Harness = Region::new(0, HARNESS_SLOT_WORDS * SLOT_BYTES);
    const TCBS_EMPTY: Self::Tcbs = Table::new([TcbData::EMPTY; GENERAL_MAX_TCBS]);
    const TCB_FREE_EMPTY: Self::TcbFree = Table::new([0; GENERAL_MAX_TCBS]);
    const SCS_EMPTY: Self::Scs = Table::new([ScData::EMPTY; GENERAL_MAX_SCHED_CONTEXTS]);
    const SC_FREE_EMPTY: Self::ScFree = Table::new([0; GENERAL_MAX_SCHED_CONTEXTS]);
    const ENDPOINTS_EMPTY: Self::Endpoints =
        Table::new([EndpointData::EMPTY; GENERAL_MAX_ENDPOINTS]);
    const ENDPOINT_FREE_EMPTY: Self::EndpointFree = Table::new([0; GENERAL_MAX_ENDPOINTS]);
    const REPLIES_EMPTY: Self::Replies = Table::new([ReplyData::EMPTY; GENERAL_MAX_REPLIES]);
    const REPLY_FREE_EMPTY: Self::ReplyFree = Table::new([0; GENERAL_MAX_REPLIES]);
    const NOTIFICATIONS_EMPTY: Self::Notifications = Table::new(
        [NotificationData {
            word: 0,
            badge: 0,
            waiter: None,
            bound_sc: None,
        }; GENERAL_MAX_NOTIFICATIONS],
    );
    const NOTIFICATION_FREE_EMPTY: Self::NotificationFree =
        Table::new([0; GENERAL_MAX_NOTIFICATIONS]);
    const IPC_MSGS_EMPTY: Self::IpcMsgs =
        Table::new([QueuedMsg::EMPTY; GENERAL_MAX_QUEUED_MESSAGES]);
    const IPC_MSG_FREE_EMPTY: Self::IpcMsgFree = Table::new([0; GENERAL_MAX_QUEUED_MESSAGES]);
    const ADDR_WORDS_EMPTY: Self::AddrWords = Table::new([AddrWord::EMPTY; GENERAL_MAX_ADDR_WORDS]);
    const WAIT_NODES_EMPTY: Self::WaitNodes = Table::new([WaitNode::EMPTY; GENERAL_MAX_WAIT_NODES]);
    const WAIT_FREE_EMPTY: Self::WaitFree = Table::new([0; GENERAL_MAX_WAIT_NODES]);
    const WAIT_BUCKETS_EMPTY: Self::WaitBuckets = Table::new([NODE_NONE; WAITADDR_BUCKETS]);
    const WAIT_BUCKETS_TAIL_EMPTY: Self::WaitBucketsTail =
        Table::new([NODE_NONE; WAITADDR_BUCKETS]);
    const HARTS_EMPTY: Self::Harts = Table::new([PerHartState::new(); GENERAL_MAX_HARTS]);
}

#[cfg(test)]
mod kt410_heap_tests {
    extern crate std;

    use super::*;
    use crate::error::{KError, ObjType, SlotRef};
    use crate::machine::Machine;
    use crate::observer::TestObserver;

    /// Mô hình **heap** (backing test) — cùng mã lõi, bảng do `TestHeapBacking` cấp.
    type HeapMachine = Machine<SimHeapStore<TestHeapBacking>, TestObserver>;

    /// Kết quả **quan sát được** của một kịch bản — dùng để so vi sai hai mô hình.
    #[derive(Debug, PartialEq, Eq)]
    struct Observed {
        ids: std::vec::Vec<u32>,
        errors: std::vec::Vec<&'static str>,
        total_used: usize,
        objects: usize,
        caps: usize,
        live_caps: usize,
    }

    /// Kịch bản **giống hệt** chạy trên cả hai mô hình: Untyped + CNode + 8 TCB + 8 SC +
    /// `SchedControl.Configure` + `Tcb.Configure` + `Start` + `Delete` + `Revoke`.
    fn scenario<S: KernelStore>(m: &mut Machine<S, TestObserver>) -> Observed {
        let mut ids = std::vec::Vec::new();
        let mut errors = std::vec::Vec::new();
        let u = m.add_untyped(0x8200_0000, 64 * 1024);
        ids.push(u);
        let c = m.add_cnode(32);
        ids.push(c);
        let self_cap = m.root_cap(c);
        m.put_cap(c, 0, self_cap).expect("self cap");
        let u_cap = m.root_cap(u);
        m.put_cap(c, 1, u_cap).expect("cap untyped");
        let ctl = m
            .retype(SlotRef::new(c, 1), ObjType::SchedControl, 0, 0)
            .expect("SchedControl");
        m.put_cap(c, 7, ctl).expect("put ctl");
        for i in 0..8u32 {
            let tcb_cap = m
                .retype(SlotRef::new(c, 1), ObjType::Tcb, 0, 0)
                .expect("retype Tcb");
            ids.push(tcb_cap);
            let sc_cap = m
                .retype(SlotRef::new(c, 1), ObjType::SchedContext, 0, 0)
                .expect("retype SC");
            ids.push(sc_cap);
            let slot_tcb = 8 + (i as usize) * 2;
            m.put_cap(c, slot_tcb, tcb_cap).expect("put Tcb");
            m.put_cap(c, slot_tcb + 1, sc_cap).expect("put SC");
            let tcb = SlotRef::new(c, slot_tcb);
            let sc = SlotRef::new(c, slot_tcb + 1);
            m.sched_control_configure(SlotRef::new(c, 7), sc, 1_000, 10_000)
                .expect("SC budget");
            m.set_sched_params(tcb, Some(sc)).expect("sched params");
            m.tcb_configure(tcb, Some(SlotRef::new(c, 0)))
                .expect("cspace");
            let tcb_obj = m.cap_obj(tcb_cap).expect("tcb obj");
            m.start_tcb(tcb_obj).expect("Ready");
            // Lỗi **có kiểm soát**: cấp TCB từ cap TCB (sai loại) ⇒ cùng mã lỗi ở hai mô hình.
            match m.retype(tcb, ObjType::Tcb, 0, 0) {
                Ok(_) => errors.push("ok"),
                Err(KError::WrongObjectType) => errors.push("WrongObjectType"),
                Err(_) => errors.push("khac"),
            }
        }
        // Destroy: xoá cap TCB/SC rồi Revoke Untyped ⇒ watermark 0.
        for i in 0..8usize {
            let slot_tcb = 8 + i * 2;
            m.delete(SlotRef::new(c, slot_tcb)).expect("delete Tcb");
            m.delete(SlotRef::new(c, slot_tcb + 1)).expect("delete SC");
        }
        m.revoke(c, Some(1)).expect("revoke untyped");
        Observed {
            ids,
            errors,
            total_used: m.total_used(),
            objects: m.object_count(),
            caps: m.cap_count(),
            live_caps: m.live_cap_count(),
        }
    }

    /// **Ca chính** — `Machine<SimHeapStore<TestHeapBacking>>` và `Machine<StaticStore>` cho
    /// **cùng** kết quả trên **cùng** kịch bản: cùng id, cùng mã lỗi, cùng watermark, cùng số
    /// object/cap. Đây là bằng chứng `SimHeapStore` là mô hình **thay thế được** của cùng mã lõi
    /// (`B2`: chỉ còn `vifast-core` + `vifast-sim`, không có mô hình kernel thứ ba).
    #[test]
    fn kt410_sim_heap_store_vi_sai_cung_ma_loi() {
        let mut mission = Machine::<StaticStore, TestObserver>::new();
        let mut heap = HeapMachine::with_observer(1);
        let a = scenario(&mut mission);
        let b = scenario(&mut heap);
        assert_eq!(a, b, "hai mô hình phải cho cùng kết quả trên cùng kịch bản");
        assert_eq!(a.total_used, 0, "Revoke phải trả watermark về 0");
        assert_eq!(a.live_caps, 2, "chỉ còn cap CNode + cap Untyped gốc");
        assert!(
            a.errors.iter().all(|e| *e == "WrongObjectType"),
            "mọi lần retype sai loại phải cùng mã lỗi: {:?}",
            a.errors
        );
        std::println!(
            "# kt410 heap vi sai: ids={} errors={} total_used={} objects={} caps={} live={}",
            a.ids.len(),
            a.errors.len(),
            a.total_used,
            a.objects,
            a.caps,
            a.live_caps,
        );
    }

    /// **Ca phụ** — `SimHeapStore` là ZST và **khác kiểu** với `StaticStore`/`UntypedStore`
    /// (ba nhà cung cấp bảng, **một** mã lõi).
    #[test]
    fn kt410_sim_heap_store_la_zst_va_khac_kieu() {
        assert_eq!(core::mem::size_of::<SimHeapStore<TestHeapBacking>>(), 0);
        assert_eq!(core::mem::size_of::<StaticStore>(), 0);
        assert_eq!(core::mem::size_of::<UntypedStore>(), 0);
        let m = HeapMachine::with_observer(1);
        assert_eq!(m.n_harts(), 1);
        assert_eq!(m.total_used(), 0);
    }
}
