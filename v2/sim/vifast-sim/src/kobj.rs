// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-09/KT-10 — mô hình object tối giản trên sim: Untyped (watermark), CNode (Copy/Mint/
//! Delete/Revoke + cây dẫn xuất kiểu MDB), TCB/Endpoint/Reply/SchedContext đủ cho
//! `Retype → Configure → Call/Reply`.
//!
//! Phạm vi có chủ ý: **không** phải MDB đầy đủ của production (không `mdb_prev/mdb_next` trong
//! slot 48 B — việc của K1a/`vifast-model`, KT-11), không có Frame/VSpace. Cây dẫn xuất ở đây
//! giữ đúng bất biến mà MDB phải có: mỗi cap sống có cha sống (hoặc là gốc), con của cap bị xoá
//! thuộc về ông, không cap nào trỏ object chết, slot ↔ cap đối xứng (`Machine::check_caps`).
//! Object nằm trong Untyped (D2-14); cỡ object lấy từ `spec/abi.toml` mục
//! `[object_size].riscv64` (code sinh không chép tay sang đây).
//!
//! Quy ước **sim** (không phải ABI — D2-35 để K1a chốt):
//! - `Untyped.Retype` của spec có `returns = []`; mô hình trả `CapId` cho harness đặt
//!   vào slot. ABI không đổi.
//! - `size_bits` với object cố định: `0` = dùng cỡ tự nhiên; nhỏ hơn cỡ ⇒ `Alignment`;
//!   lớn hơn ⇒ `InvalidArgument`. Với `Untyped` con: `size_bits` = log2 cỡ, tối thiểu
//!   `untyped_min_bytes` = 4096, căn lề = cỡ; cỡ > `ARENA_ALIGN` (4096) — tức
//!   `size_bits ≥ 13` — bị từ chối rõ bằng `Alignment` vì arena của mô hình chỉ bảo đảm
//!   căn 4096 (KT-17; giới hạn triển khai, không phải ABI).
//! - Revoke cap **Untyped**: finalise mọi object trong Untyped, xoá cap con/cháu, đặt
//!   watermark về 0 (thiết kế §4.2). Revoke cap khác: chỉ xoá cap con/cháu, object sống.
//!
//! Bất biến (thiết kế §9.6 mục 9): `byte đã cấp + byte còn trống == cỡ Untyped ban đầu`;
//! không object nào chồng lấn; watermark chỉ tiến trong một chuỗi Retype (revoke Untyped
//! là ngoại lệ có chủ ý: đặt về 0 để làm tiếp được).
//!
//! **KT-16 — tích hợp `vifast-objmem`** (D2-14, D2-47d): body của object (`Kind`) nằm
//! trong `ObjMem`; mỗi Untyped có một arena vùng nhớ riêng do objmem cấp, nên watermark
//! của mô hình **chính là** `Arena::used()` (không còn `Vec<Object>`/bộ đếm viết tay).
//! Truy cập body chỉ qua closure (`with`/`with_mut`) — `v2/sim/**` không có `unsafe`
//! (R5); `ObjRef` không mang identity store nên `Machine` cấp `id` duy nhất theo vùng và
//! phân phối truy cập theo `Meta::loc`. Bảng `Cap` vẫn là `Vec<Cap>` (D2-47d).
//! Object đã Revoke bị finalise trong objmem ⇒ truy cập trả `Dead`; arena của cây Untyped
//! được `reset` sau khi mọi body trong vùng đã finalise.
//!
//! Untyped con: vùng nhớ của nó là arena **riêng** cấp đúng cỡ vùng cắt (`len`) — quan hệ
//! cha/con trong mô hình là quan hệ **logic** (offset/cỡ trong `object_bytes`), không phải
//! containment vật lý; chi tiết của harness sim, không phải ABI.
//!
//! **KT-25 — IPC core 2** (D2-53 Q6/Q8/Q15; `v2/docs/k1c-ipc-semantics.md` §10-§12):
//! - **receive window** 4 ô CSpace/TCB (`tcb_set_recv_window`; model API `[TK]`, không thêm chữ
//!   ký ABI) + chuyển **≤ 4 cap** (`ipc::plan_cap_transfer`): cap nhận **mất `RIGHT_GRANT`**,
//!   giữ badge; thiếu GRANT/ô đích có cap ⇒ **thất bại mềm** (`cap_transfer_failed`), message
//!   vẫn giao; cap trỏ chính Endpoint đang gọi + badge ⇒ **unwrap** vào `Message.badge`
//!   (`extra_caps_unwrapped`).
//! - **Vòng đời Reply** (Q8): cap Reply bị `Delete`/`Revoke` (hoặc Reply nằm trong Untyped bị
//!   revoke) khi caller `BlockedOnReply` ⇒ caller `Cancelled` + `Ready` (một cửa
//!   `transition_tcb` ⇒ donation KT-21 trả về đúng chủ, không rò SC).
//! - **`ReplyRecv`/`NBSendRecv`/`NBSendWait`** (Q15a): reply trước rồi `Recv`; NBSend không giao
//!   được ⇒ **vẫn** `Recv`/`Wait` (message chưa vào IPC nên không bị coi là mất). `NBRecv` là
//!   **poll** ⇒ TCB không chạy được trả `Busy` (không đổi trạng thái).

use crate::ipc::{self, IpcArea, Message, MessageInfo};
use crate::mailbox::{HartMailbox, Mailbox, MailboxMsg, MAILBOX_CAP};
use crate::sched::HartSched;
use vifast_objmem::{Dead, KObj, ObjRef, ObjStore, OwnedMem, ARENA_ALIGN};

/// `flags` bit GUEST của `Untyped.Retype` (D2-32: chỉ VSpace nhận, tới H1 trả NotSupported).
pub const FLAG_GUEST: u32 = 1;

/// `spec/abi.toml` `[object_size].slot.bytes` (QD §3.9).
pub const SLOT_BYTES: usize = 48;
/// `spec/abi.toml` `[object_size].riscv64.cnode_base`.
pub const CNODE_BASE_BYTES: usize = 64;
/// `spec/abi.toml` `[object_size].untyped_min_bytes`.
pub const UNTYPED_MIN_BYTES: usize = 4096;
/// `spec/abi.toml` `[object_size].default_align_bytes`.
pub const DEFAULT_ALIGN: usize = 16;

/// Quyền của cap — thiết kế §4.3 (không có bit EXEC: EXEC là thuộc tính lúc map, QD §3.12).
pub const RIGHT_READ: u8 = 1 << 0;
pub const RIGHT_WRITE: u8 = 1 << 1;
pub const RIGHT_GRANT: u8 = 1 << 2;
pub const RIGHT_GRANT_REPLY: u8 = 1 << 3;
/// Cap gốc do kernel/harness cấp giữ đủ 4 quyền.
pub const RIGHTS_ALL: u8 = RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT | RIGHT_GRANT_REPLY;

pub type ObjId = u32;
pub type CapId = u32;

/// Số hart của mô hình K1b: **1** (D2-49(a); push migration hoãn sang K1c).
///
/// Dùng để kiểm `affinity`/`home` của model API `Tcb.SetAffinity/SetHome` `[TK]`; KT-19 thay
/// bằng cấu hình hart của `Machine`.
pub const HART_COUNT: u16 = 1;
/// `HartMask` mặc định: hart 0 (K1b 1 hart).
pub const AFFINITY_HOME: u64 = 1;

/// Loại object — 8 lõi + 8 reserved (spec `[object]`). Reserved ⇒ `NotSupported` khi Retype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjType {
    Untyped,
    CNode,
    Tcb,
    SchedContext,
    Endpoint,
    Notification,
    Reply,
    SchedControl,
    VSpace,
    PageTable,
    Frame,
    IrqControl,
    IrqHandler,
    IOSpace,
    VCpu,
    PowerControl,
}

impl ObjType {
    pub fn name(self) -> &'static str {
        match self {
            ObjType::Untyped => "Untyped",
            ObjType::CNode => "CNode",
            ObjType::Tcb => "Tcb",
            ObjType::SchedContext => "SchedContext",
            ObjType::Endpoint => "Endpoint",
            ObjType::Notification => "Notification",
            ObjType::Reply => "Reply",
            ObjType::SchedControl => "SchedControl",
            ObjType::VSpace => "VSpace",
            ObjType::PageTable => "PageTable",
            ObjType::Frame => "Frame",
            ObjType::IrqControl => "IrqControl",
            ObjType::IrqHandler => "IrqHandler",
            ObjType::IOSpace => "IOSpace",
            ObjType::VCpu => "VCpu",
            ObjType::PowerControl => "PowerControl",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        const ALL: [ObjType; 16] = [
            ObjType::Untyped,
            ObjType::CNode,
            ObjType::Tcb,
            ObjType::SchedContext,
            ObjType::Endpoint,
            ObjType::Notification,
            ObjType::Reply,
            ObjType::SchedControl,
            ObjType::VSpace,
            ObjType::PageTable,
            ObjType::Frame,
            ObjType::IrqControl,
            ObjType::IrqHandler,
            ObjType::IOSpace,
            ObjType::VCpu,
            ObjType::PowerControl,
        ];
        ALL.iter().copied().find(|t| t.name() == name)
    }

    /// Lõi 1..8 của spec (`[object]`); reserved 9..16 chưa cài (spec `unsupported = true`).
    pub fn is_core(self) -> bool {
        !matches!(
            self,
            ObjType::VSpace
                | ObjType::PageTable
                | ObjType::Frame
                | ObjType::IrqControl
                | ObjType::IrqHandler
                | ObjType::IOSpace
                | ObjType::VCpu
                | ObjType::PowerControl
        )
    }

    /// Cỡ cố định theo `spec/abi.toml` `[object_size].riscv64`; `None` nếu cỡ biến thiên.
    pub fn fixed_size(self) -> Option<usize> {
        match self {
            ObjType::Tcb => Some(512),
            ObjType::SchedContext => Some(256),
            ObjType::Endpoint | ObjType::Notification | ObjType::Reply | ObjType::SchedControl => {
                Some(64)
            }
            ObjType::Untyped | ObjType::CNode => None,
            _ => None,
        }
    }
}

/// 19 mã lỗi của `spec/abi.toml` — tên/số lấy từ `vifast-abi` (sinh từ spec, D2-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KError {
    InvalidCapability,
    WrongObjectType,
    InsufficientRights,
    WxViolation,
    InvalidArgument,
    Alignment,
    SlotOccupied,
    SlotEmpty,
    NotEnoughMemory,
    AlreadyMapped,
    NotMapped,
    MissingPageTable,
    HasChildren,
    Busy,
    Cancelled,
    Timeout,
    IllegalOperation,
    NotSupported,
    RangeError,
}

impl KError {
    /// Chỉ số trong `vifast_abi::ERRORS` (0-based, đúng thứ tự spec).
    fn spec_index(self) -> usize {
        match self {
            KError::InvalidCapability => 0,
            KError::WrongObjectType => 1,
            KError::InsufficientRights => 2,
            KError::WxViolation => 3,
            KError::InvalidArgument => 4,
            KError::Alignment => 5,
            KError::SlotOccupied => 6,
            KError::SlotEmpty => 7,
            KError::NotEnoughMemory => 8,
            KError::AlreadyMapped => 9,
            KError::NotMapped => 10,
            KError::MissingPageTable => 11,
            KError::HasChildren => 12,
            KError::Busy => 13,
            KError::Cancelled => 14,
            KError::Timeout => 15,
            KError::IllegalOperation => 16,
            KError::NotSupported => 17,
            KError::RangeError => 18,
        }
    }

    pub fn id(self) -> u16 {
        vifast_abi::ERRORS[self.spec_index()].1
    }

    pub fn name(self) -> &'static str {
        vifast_abi::ERRORS[self.spec_index()].0
    }
}

/// Vị trí slot cap: `(cnode, slot)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotRef {
    pub cnode: ObjId,
    pub slot: usize,
}

impl SlotRef {
    pub fn new(cnode: ObjId, slot: usize) -> Self {
        Self { cnode, slot }
    }
}

/// Ảnh chụp một cap cho harness/test — đọc-only, không lộ cấu trúc nội bộ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapInfo {
    pub cap: CapId,
    pub obj: ObjId,
    pub ty: ObjType,
    pub parent: Option<CapId>,
    pub slot: Option<SlotRef>,
    pub rights: u8,
    pub badge: Option<u64>,
    pub alive: bool,
}

/// KT-22 — ảnh chụp **hộp thư của Endpoint** (model-only `[TK]`, không phải ABI).
///
/// `pushed == popped + removed + len` là bất biến "không mất/thêm message"; `capacity` là sức
/// chứa **cố định** [`MAILBOX_CAP`] (K1B-PLAN §5 KT-22 DoD 1). `removed` đếm message bị huỷ khi
/// TCB chết/treo (D2-25) — đường duy nhất làm giảm hàng mà **không** qua `recv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointInfo {
    pub capacity: usize,
    pub len: usize,
    pub pushed: u64,
    pub popped: u64,
    pub removed: u64,
    /// TCB đang chặn `Recv` chờ message (tối đa một; bất biến: chỉ khi hàng rỗng).
    pub recv_waiter: Option<ObjId>,
}

// KT-18: `TcbState` + bảng `ALLOWED` 11×11 nằm ở `crate::tcb` (D2-49(d)) để mọi đường đổi
// trạng thái đi qua **một** cửa `Machine::transition_tcb`. Re-export để đường dẫn cũ
// (`vifast_sim::kobj::TcbState`, `::FaultKind`, `::SleepReason`) không đổi.
pub use crate::tcb::{
    BlockedOnKind, FaultAccess, FaultKind, SleepReason, SuspendMode, TcbSnapshot, TcbState,
    ALLOWED, TCB_STATE_COUNT,
};
// KT-20: ngân sách MCS (`SchedContext`) — model-only `[TK]`, không phải ABI (D2-49(b,e)).
pub use crate::mcs::{ScBudget, ScInfo};

/// Một message đang chờ trong hàng gửi của Endpoint — **KT-24** (trước KT-24 hàng chỉ giữ
/// `(sender, fault)`; message thật nằm trong `TcbData`/`last_mr` **một** word).
///
/// - `msg: Some` = message **người dùng** (Send/NBSend/Call) đã được kernel cấp `msg_id`;
///   `msg: None` = **fault** `Exit` (mô hình K1b, D2-25) — không phải message IPC nên
///   **không** tính vào bộ đếm `ipc_sent/delivered/pending` (KT-24 giữ nguyên đường fault
///   của KT-18/23; fault khác Exit thuộc KT-26).
/// - `call` = message từ `Call` đang chờ `Reply` (người gửi `BlockedOnSend{call:true}`);
///   `false` = `Send`/`NBSend`/`Call` có `no_reply` (D2-53 Q7).
/// - `blocking` = người gửi đang `BlockedOnSend` chờ giao (`send`/`call` chặn); `false` =
///   gửi **không chặn** (`nb_send`, `call no_reply=1`) nên người gửi vẫn `Ready`/`Running`.
#[derive(Debug)]
struct QueuedMessage {
    msg: Option<Box<Message>>,
    sender: ObjId,
    call: bool,
    fault: Option<FaultKind>,
    blocking: bool,
    /// **KT-25** — ảnh chụp cap nguồn của message, chụp lúc message vào đường IPC (bên gửi
    /// có thể xoá cap trước khi message được giao); rỗng với message fault/lấy từ hàng của
    /// KT-24. Cài vào receive window của bên nhận khi message được giao.
    snaps: Vec<ipc::CapSnapshot>,
}

#[derive(Debug, Default)]
struct EndpointState {
    recv: Option<(ObjId, Option<CapId>)>,
    /// KT-22: hàng gửi **FIFO cố định** ([`MAILBOX_CAP`] message); đầy ⇒ `call`/`send`/`exit`
    /// trả `Busy` (backpressure) — không còn `VecDeque` không giới hạn.
    send: Mailbox<QueuedMessage>,
    last_mr: Option<u64>,
}

/// Payload ("body") của object — **phần được đặt trong `vifast-objmem`** (KT-16, D2-14).
///
/// Một kiểu Rust duy nhất cho mọi loại object; bảng `kind → ctor` của objmem đăng ký
/// `ctor::<Kind>` theo `kind()` (D2-48). Các variant lớn (`TcbData`, `SchedData`,
/// `EndpointState`, `ReplyData`) nằm sau `Box`/`Vec` để `size_of::<Kind>()` vừa 48 B —
/// cỡ nhỏ nhất của body trong object 64 B (`Endpoint`/`Notification`/`Reply`/`SchedControl`)
/// sau khi trừ header 16 B; assert bên dưới cưỡng chế **lúc biên dịch**.
#[derive(Debug)]
enum Kind {
    Untyped,
    CNode { slots: Vec<Option<CapId>> },
    Tcb(Box<TcbData>),
    SchedContext(Box<SchedData>),
    Endpoint(Box<EndpointState>),
    Reply(Box<ReplyData>),
    Notification(Box<NotificationState>),
    Plain,
}

/// Ruột TCB — **đủ trường DoD KT-18** (thiết kế §4.4 `:285`, D2-33).
///
/// `suspended` và `exit_code` là cờ **trực giao** với `state` (D2-33): `Ready + suspended`
/// không nằm trong ready queue, `Sleeping + suspended` vẫn `Sleeping`.
#[derive(Debug)]
struct TcbData {
    state: TcbState,
    suspended: bool,
    exit_code: Option<u64>,
    fault_ep: Option<CapId>,
    cspace: Option<ObjId>,
    priority: u8,
    max_controlled_priority: u8,
    /// `HartMask` ≤ 64 bit (D2-49(a): K1b 1 hart ⇒ bit 0).
    affinity: u64,
    home: u16,
    /// Model API `set_sched_params(tcb, sc)` — D2-49(b); ABI `Tcb.SetSchedParams` để K1c/K3.
    sched_context: Option<ObjId>,
    /// **KT-21** — `SchedContext` được **donate** cho TCB này (server passive chạy bằng ngân
    /// sách của client); model-only `[TK]`, D2-49(c). `None` ⇔ TCB tự chủ theo `sched_context`.
    /// Bất biến: `donated_sc.is_some()` ⇒ `sched_context.is_none()` (một SC một TCB một lúc).
    donated_sc: Option<ObjId>,
    bound_notification: Option<ObjId>,
    /// **KT-24** — vùng nhận IPC của TCB (4 MR "thanh ghi" + 60 MR "IPC buffer model",
    /// hai field riêng trong [`IpcArea`], không gộp). Ghi **chỉ** qua `crate::ipc::transfer`.
    ipc: IpcArea,
    /// **KT-25** — **receive window** (CPTR) của TCB: 4 ô CSpace nhận cap của message vào.
    /// `None` = **chưa bind** (message có cap ⇒ `InvalidArgument`); `Some([s0..s3])` = đã bind
    /// (ô nào đang có cap thì cap tương ứng **thất bại mềm**, D2-53 Q6). Model-only `[TK]`.
    recv_window: Option<[SlotRef; ipc::MAX_CAPS]>,
    /// **KT-24** — lỗi IPC mà kernel gán cho TCB này ở lần giao message gần nhất (model-only
    /// `[TK]`): `Cancelled` khi `Call` gặp bên nhận `Recv` **không** kèm Reply (D2-53 Q7 nhánh
    /// (b)). Cần trường này vì mô hình gọi syscall **đồng bộ**: nhánh caller đã xếp hàng nhận
    /// kết quả lúc message được giao, không phải lúc `call` trả về.
    last_ipc_error: Option<KError>,
    /// **KT-26** — `(sender, FaultKind)` của **fault** vừa được giao cho TCB này làm bên nhận
    /// (procsrv). Model-only `[TK]`: message fault không mang nội dung MR (`msg: None`) nên
    /// procsrv cần biết sender + loại fault để `recv_exit`/`wait`; `take_last_fault` tiêu thụ.
    /// Ghi **một chỗ** trong [`Machine::deliver_queued`] (và đường nhanh của `exit`/`inject_fault`).
    last_fault: Option<(ObjId, FaultKind)>,
}

impl TcbData {
    /// Ảnh chụp toàn bộ trường TCB (model API `[TK]`; `Tcb.ReadState` ABI chỉ trả 4 giá trị).
    fn snapshot(&self) -> TcbInfo {
        TcbInfo {
            state: self.state,
            suspended: self.suspended,
            exit_code: self.exit_code,
            fault_ep: self.fault_ep,
            cspace: self.cspace,
            priority: self.priority,
            max_controlled_priority: self.max_controlled_priority,
            affinity: self.affinity,
            home: self.home,
            sched_context: self.sched_context,
            donated_sc: self.donated_sc,
            bound_notification: self.bound_notification,
        }
    }
}

/// Notification tối giản của K1b: **một** waiter (thiết kế §4.7: `Signal` đánh thức một
/// người chờ). Word/badge/signal thuộc K1c (D2-49(a)) — KT-18 chỉ cần chỗ ghi waiter để
/// `Suspend(Abort)`/finalise chứng minh "gỡ khỏi hàng chờ notification".
#[derive(Debug, Default)]
struct NotificationState {
    waiter: Option<ObjId>,
}

#[derive(Debug)]
struct SchedData {
    bound: Option<CapId>,
    /// KT-20: ngân sách MCS — `budget`/`period`/`consumed`/`refill` (model-only `[TK]`).
    budget: ScBudget,
    /// **KT-21** — TCB **đang nhận** donation từ SC này (server passive); `None` khi SC đang ở
    /// chủ sở hữu hợp lệ. Model-only `[TK]`, D2-49(c).
    donated_to: Option<ObjId>,
    /// **KT-21** — TCB **cho mượn** SC (client đang `BlockedOnReply`); cặp hai chiều với
    /// [`SchedData::donated_to`] — cả hai cùng `Some` hoặc cùng `None`.
    donor: Option<ObjId>,
}

#[derive(Debug)]
struct ReplyData {
    caller: Option<ObjId>,
}

/// Ảnh chụp **toàn bộ** trường TCB của mô hình — đọc/ghi qua model API `[TK]` (D2-49(b,c)).
///
/// `Tcb.ReadState` (ABI) chỉ trả 4 giá trị ([`TcbSnapshot`]); các trường còn lại không có
/// chữ ký method trong draft D2-33 nên K1b **không** tuyên bố ABI cho chúng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcbInfo {
    pub state: TcbState,
    pub suspended: bool,
    pub exit_code: Option<u64>,
    pub fault_ep: Option<CapId>,
    pub cspace: Option<ObjId>,
    pub priority: u8,
    pub max_controlled_priority: u8,
    pub affinity: u64,
    pub home: u16,
    pub sched_context: Option<ObjId>,
    /// **KT-21** — SC được **donate** cho TCB (server passive): model-only `[TK]`, D2-49(c).
    pub donated_sc: Option<ObjId>,
    pub bound_notification: Option<ObjId>,
}

/// Một dòng log finalise TCB (KT-18 DoD 9): ghi **trước** khi body bị destroy.
///
/// `released_sched_context` là kết quả của hook trả `SchedContext` (thiết kế §4.3 bảng
/// Finalise). K1b **chưa có** scheduler/refill queue (KT-19/KT-20) nên hook chỉ trả ObjId;
/// KT-19 gắn thêm bước gỡ ready queue, KT-20 gỡ refill queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinaliseEntry {
    pub tcb: ObjId,
    pub state: TcbState,
    pub released_sched_context: Option<ObjId>,
}

/// Body phải vừa object **nhỏ nhất** của spec (64 B) sau header 16 B: `Kind ≤ 48 B`.
const _: () = assert!(
    core::mem::size_of::<Kind>() + vifast_objmem::HEADER_SIZE <= 64,
    "Kind phải vừa body của object 64 B (Reply/Endpoint/Notification/SchedControl)"
);

impl KObj for Kind {
    fn kind(&self) -> u8 {
        // Một kiểu Rust ⇒ một `ctor`; các mã dưới đây chỉ để header/`kind` dễ đọc.
        match self {
            Kind::Untyped => 1,
            Kind::CNode { .. } => 2,
            Kind::Tcb(_) => 3,
            Kind::SchedContext(_) => 4,
            Kind::Endpoint(_) => 5,
            Kind::Plain => 6,
            Kind::Reply(_) => 7,
            Kind::Notification(_) => 8,
        }
    }
}

/// Nơi body của object nằm (KT-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Loc {
    /// Arena kernel của `Machine`: object do harness cấp + body của Untyped gốc.
    Kernel,
    /// Arena của Untyped thứ `i` trong `Machine::regions`.
    Untyped(u32),
}

/// Sổ cái của một object — **metadata mô hình**, không phải object body.
///
/// Body (`Kind`) nằm trong `vifast-objmem`; metadata ở đây giữ phần mô hình cần kể cả khi
/// object đã chết (`object_type`/`object_bytes` của object đã Revoke vẫn phải trả lời —
/// `kt11_diff_sim` băm cả object chết), nên không thể đọc ngược từ body đã finalise.
#[derive(Debug)]
struct Meta {
    ty: ObjType,
    alive: bool,
    /// Chỉ số (0-based) của Untyped chứa object; `None` với object không cấp từ Untyped.
    untyped: Option<usize>,
    /// Offset logic trong Untyped; với Untyped gốc là 0 (không cấp từ Untyped).
    offset: usize,
    /// Cỡ logic của spec (512 TCB, 64 Reply, ...); với Untyped gốc là 0.
    len: usize,
    loc: Loc,
    body: ObjRef<Kind>,
    /// Với object Untyped: chỉ số arena vùng nhớ của chính nó (watermark = `used()`).
    region: Option<usize>,
    /// Với CNode: số slot (giữ để `cap_at`/`cnode_slot_count` của CNode **đã finalise**
    /// vẫn trả `SlotEmpty` như API cũ — body đã bị `drop_in_place` nên không đọc ngược được).
    slots: Option<usize>,
}

#[derive(Debug)]
struct Cap {
    obj: ObjId,
    ty: ObjType,
    parent: Option<CapId>,
    slot: Option<SlotRef>,
    rights: u8,
    badge: Option<u64>,
    alive: bool,
}

/// Cỡ arena kernel mặc định của `Machine` [ƯL] (K1A-PLAN §4/KT-16).
pub const DEFAULT_ARENA_BYTES: usize = 1 << 20;

/// Mô hình kernel tối giản: object + cap + IPC tối thiểu cho KT-09.
///
/// **KT-16**: object body nằm trong `vifast-objmem` (`OwnedMem`): mỗi Untyped một arena
/// vùng nhớ riêng (`Meta::region`), object Retype nằm **ngay trong** vùng của Untyped và
/// watermark của mô hình chính là `Arena::used()` — không còn `Vec<Object>` hay bộ đếm
/// watermark viết tay. Object không cấp từ Untyped (harness cấp + body Untyped gốc) nằm
/// trong arena kernel. Bảng `Cap` vẫn là `Vec<Cap>` (D2-47d).
#[derive(Debug)]
pub struct Machine {
    /// Arena kernel: `add_untyped`/`add_cnode`/`add_kernel_object` và body Untyped gốc.
    kernel: OwnedMem,
    /// Mỗi Untyped một vùng nhớ: object cấp từ Untyped nằm ở đây (kể cả Untyped con).
    regions: Vec<OwnedMem>,
    objects: Vec<Meta>,
    caps: Vec<Cap>,
    /// Bộ đếm cặp chuyển trạng thái TCB 11×11 (`hàng * 11 + cột`) — KT-18 DoD 4.
    pair_hits: [u32; TCB_STATE_COUNT * TCB_STATE_COUNT],
    /// **KT-23**: bộ đếm cặp **cấm** bị **thử** (`hàng * 11 + cột`) — mỗi lần
    /// [`Machine::transition_tcb`] từ chối một cặp `.` của bảng `ALLOWED`. Là nguồn của
    /// `forbidden_attempts`/`forbidden_pairs_attempted` trong [`K1bReport`] (DoD 3).
    forbidden_hits: [u32; TCB_STATE_COUNT * TCB_STATE_COUNT],
    /// **KT-23**: số lần `Running → Sleeping{Budget}` (hết budget, D2-49(e)) — đếm ở **một cửa**
    /// `transition_tcb`; `k1b_report().budget.budget_sleeps` (DoD 2/8).
    budget_sleeps: u64,
    /// **KT-23**: số lần một `SchedContext` được nạp lại (`consumed = 0` ở biên `period`) —
    /// `k1b_report().budget.refills`.
    budget_refills: u64,
    /// **KT-24** — số message **người dùng** đã được nhận vào đường IPC (`send`/`nb_send`/
    /// `call`, kể cả `call no_reply`); message fault `Exit` **không** tính (không phải IPC
    /// message). `msg_id` cấp từ bộ đếm này nên **không bao giờ tái dùng**.
    ipc_sent: u64,
    /// **KT-24** — số message đã **giao** vào vùng nhận của TCB (mỗi lần giao đúng **một**
    /// lần qua `crate::ipc::transfer`). Bất biến `transfer_calls == ipc_delivered`.
    ipc_delivered: u64,
    /// **KT-24** — số message bị **huỷ** khi TCB chết/treo hoặc Endpoint bị finalise
    /// (D2-25: hàng chờ cũ bị huỷ). Bất biến bảo toàn message:
    /// `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`.
    ipc_dropped: u64,
    /// **KT-24** — **test-only** đếm số lần gọi `crate::ipc::transfer`; bằng chứng "chỉ có
    /// **một** đường copy MR" (K1C-PLAN §5 KT-24 DoD 3). Luôn bằng `ipc_delivered`.
    transfer_calls: u64,
    /// **KT-25** — số cap **cài được** vào receive window qua đường IPC (`derive`, bỏ
    /// `RIGHT_GRANT`). Bộ đếm **mềm** (không phải bit MessageInfo — D2-53 Q6).
    cap_transfer_installed: u64,
    /// **KT-25** — số cap **thất bại mềm** khi chuyển (thiếu `RIGHT_GRANT`, ô đích đã có cap,
    /// hoặc cap nguồn chết giữa lúc xếp hàng và lúc giao). Message **vẫn** giao.
    cap_transfer_failed: u64,
    /// **KT-25** — số cap **unwrap** thành badge (cap trỏ chính Endpoint đang gọi + có badge;
    /// không chiếm receive window).
    cap_unwrapped: u64,
    /// Log finalise TCB (KT-18 DoD 9), ghi trước khi body bị destroy trong objmem.
    finalise_log: Vec<FinaliseEntry>,
    /// Hàng chờ WaitOnAddress mô hình hoá (thiết kế §4.7: bucket theo `(frame, offset)`).
    /// K1b chưa có object Frame (K2) nên khoá dùng `ObjId` thô của frame.
    address_waiters: Vec<(ObjId, u32, ObjId)>,
    /// Scheduler theo hart (KT-19): KT-19 dựng **1** hart (D2-49(a)); mỗi hart một
    /// [`HartSched`] (256 mức + bitmap) để K1c thêm hart/push migration mà không đổi API.
    sched: Vec<HartSched>,
    /// KT-22: hộp thư **theo hart** (thiết kế §4.17) — `HART_COUNT` = 1 ở K1b (D2-49(a)),
    /// mô hình tất định: `post_mail`/`process_mail`/`send_mail` (không atomic/thread/loom).
    mail: Vec<HartMailbox>,
    /// Preempt **ép tại mọi public op** (thiết kế §4.13) — mặc định **tắt** để không đổi
    /// ngữ nghĩa KT-18; test KT-19 bật bằng [`Machine::set_preempt_every_step`].
    preempt_every_step: bool,
    /// KT-20: thời gian ảo (tick), **không** đọc `Instant`/wall clock (D2-49(h)).
    now: u64,
}

/// Vị trí/cỡ của object trong mô hình — gom tham số `push_meta`/`push_kernel`.
#[derive(Debug, Default, Clone, Copy)]
struct ChoDat {
    untyped: Option<usize>,
    offset: usize,
    len: usize,
    region: Option<usize>,
    slots: Option<usize>,
}

/// Ảnh chụp **bộ đếm chuyển trạng thái** (KT-18) + **cặp cấm bị thử** (KT-23).
///
/// `transition_hit/allowed` là **46 cặp suy ra từ thiết kế** (bảng 48 trừ 2 cặp
/// `Stopping` reserved cho K1c — [`crate::tcb::coverage_design`]); `transition_hit_table`
/// /`transition_table_allowed` là con số **toàn bảng 48** (gồm 2 cặp reserved, chỉ đi được
/// qua cổng `transition_tcb` trực tiếp). `forbidden_*` đếm **mọi** lần thử cặp `.` của bảng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct K1bTransitionReport {
    pub hit: usize,
    pub allowed: usize,
    pub hit_table: usize,
    pub table_allowed: usize,
    pub reserved: usize,
    pub forbidden_attempts: u64,
    pub forbidden_pairs_attempted: usize,
    pub forbidden_pairs_total: usize,
}

/// Ảnh chụp **ready queue/trạng thái TCB** của mô hình 1 hart (KT-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct K1bQueueReport {
    pub live_tcbs: usize,
    pub ready: usize,
    pub running: usize,
    pub blocked: usize,
    pub sleeping: usize,
    pub inactive: usize,
    pub dead: usize,
    /// Số TCB **thực sự** nằm trong ready queue (không tính `current`).
    pub queued: usize,
    pub current: Option<ObjId>,
    pub idle: bool,
}

/// Ảnh chụp **ngân sách MCS** (KT-20/21): tick ảo đã chạy, số lần
/// `Running → Sleeping{Budget}`, số lần refill, và tổng `charged` của các SC sống.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct K1bBudgetReport {
    pub ticks: u64,
    pub budget_sleeps: u64,
    pub refills: u64,
    pub live_sched_contexts: usize,
    pub configured: usize,
    pub charged: u64,
    pub consumed_now: u64,
}

/// **KT-23** — tổng kết K1b trên **một** `Machine` (test/harness). Gồm 4 miền:
/// transition (KT-18/23), queue (KT-19), budget (KT-20/21), và **số miền bất biến đang
/// vi phạm tại thời điểm dựng report** (`*_violations = 1` nếu `check_*` tương ứng lỗi —
/// 0 khi sạch). Test KT-23 còn cộng dồn vi phạm sau **mỗi** bước; report là ảnh chụp cuối.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct K1bReport {
    pub transition: K1bTransitionReport,
    pub queue: K1bQueueReport,
    pub budget: K1bBudgetReport,
    pub sched_violations: u64,
    pub sc_ref_violations: u64,
    pub budget_violations: u64,
    pub mailbox_violations: u64,
    pub byte_violations: u64,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        // Finalise body còn sống để `Box`/`Vec` bên trong `Kind` được drop trước khi arena
        // giải phóng vùng nhớ (Miri: không rò; object đã chết đã được finalise ở Revoke).
        let Machine {
            kernel,
            regions,
            objects,
            ..
        } = self;
        for m in objects.iter() {
            if !m.alive {
                continue;
            }
            match m.loc {
                Loc::Kernel => kernel.free(m.body),
                Loc::Untyped(i) => regions[i as usize].free(m.body),
            }
        }
    }
}

impl Machine {
    pub fn new() -> Self {
        Self::with_arena_bytes(DEFAULT_ARENA_BYTES)
    }

    /// `Machine` với arena kernel `bytes` byte (mặc định [`DEFAULT_ARENA_BYTES`] = 1 MiB).
    ///
    /// Arena kernel chứa body của object **không** cấp từ Untyped; object Retype nằm trong
    /// arena riêng của Untyped chứa nó.
    pub fn with_arena_bytes(bytes: usize) -> Self {
        assert!(bytes > 0, "arena kernel phải có cỡ dương");
        Self {
            kernel: OwnedMem::new(bytes).expect("cấp arena kernel của Machine"),
            regions: Vec::new(),
            objects: Vec::new(),
            caps: Vec::new(),
            pair_hits: [0; TCB_STATE_COUNT * TCB_STATE_COUNT],
            forbidden_hits: [0; TCB_STATE_COUNT * TCB_STATE_COUNT],
            budget_sleeps: 0,
            budget_refills: 0,
            ipc_sent: 0,
            ipc_delivered: 0,
            ipc_dropped: 0,
            transfer_calls: 0,
            cap_transfer_installed: 0,
            cap_transfer_failed: 0,
            cap_unwrapped: 0,
            finalise_log: Vec::new(),
            address_waiters: Vec::new(),
            sched: (0..HART_COUNT).map(|_| HartSched::new()).collect(),
            mail: (0..HART_COUNT).map(|_| HartMailbox::new()).collect(),
            preempt_every_step: false,
            now: 0,
        }
    }

    /// Dung lượng arena kernel (byte).
    pub fn arena_capacity(&self) -> usize {
        self.kernel.capacity()
    }

    /// Số byte arena kernel đã cấp.
    pub fn arena_used(&self) -> usize {
        self.kernel.used()
    }

    // ---------------------------------------------------------------- dựng cảnh

    /// Thêm một Untyped gốc. `base` chỉ để kịch bản ghi nhớ; mô hình tính offset từ 0.
    ///
    /// Vùng nhớ thật của Untyped (arena của nó) do objmem cấp với cỡ `size`.
    pub fn add_untyped(&mut self, base: u64, size: usize) -> ObjId {
        let _ = base;
        assert!(size > 0, "Untyped phải có cỡ dương");
        let region = self.push_region(size);
        self.push_kernel(
            ObjType::Untyped,
            Kind::Untyped,
            ChoDat {
                region: Some(region),
                ..ChoDat::default()
            },
        )
    }

    /// Thêm CNode do harness cấp (init giữ CNode gốc — không cần Retype CNode ở KT-09).
    pub fn add_cnode(&mut self, slots: usize) -> ObjId {
        assert!(slots > 0, "CNode phải có ít nhất một slot");
        self.push_kernel(
            ObjType::CNode,
            Self::fresh_kind(ObjType::CNode, slots),
            ChoDat {
                slots: Some(slots),
                ..ChoDat::default()
            },
        )
    }

    /// Thêm object lõi do harness cấp (SchedControl, TCB khởi tạo, ...) — không tính byte Untyped.
    pub fn add_kernel_object(&mut self, ty: ObjType) -> ObjId {
        assert!(ty.is_core(), "chỉ object lõi mới dựng trực tiếp");
        self.push_kernel(ty, Self::fresh_kind(ty, 0), ChoDat::default())
    }

    /// Cap gốc cho một object (không có cha) — harness đặt vào CNode bằng `put_cap`.
    pub fn root_cap(&mut self, obj: ObjId) -> CapId {
        let ty = self
            .objects
            .get(obj as usize - 1)
            .map(|m| m.ty)
            .expect("root_cap: object phải tồn tại");
        self.push_cap(obj, ty, None, RIGHTS_ALL, None)
    }

    pub fn put_cap(&mut self, cnode: ObjId, slot: usize, cap: CapId) -> Result<(), KError> {
        self.check_cnode_slot(cnode, slot)?;
        {
            let cap_ref = self.cap(cap)?;
            if cap_ref.slot.is_some() {
                return Err(KError::IllegalOperation);
            }
        }
        if self.cnode_slot(cnode, slot)?.is_some() {
            return Err(KError::SlotOccupied);
        }
        self.cnode_set_slot(cnode, slot, Some(cap))?;
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(SlotRef::new(cnode, slot));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- truy vấn

    pub fn cap_at(&self, cnode: ObjId, slot: usize) -> Result<CapId, KError> {
        let n = self.cnode_slot_count(cnode)?;
        if slot >= n {
            return Err(KError::RangeError);
        }
        match self.cnode_slot(cnode, slot)? {
            None => Err(KError::SlotEmpty),
            Some(cap) => {
                if self.cap(cap)?.alive {
                    Ok(cap)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
        }
    }

    pub fn cap_obj(&self, cap: CapId) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if c.alive {
            Ok(c.obj)
        } else {
            Err(KError::InvalidCapability)
        }
    }

    pub fn cap_obj_at(&self, cnode: ObjId, slot: usize) -> Result<ObjId, KError> {
        self.cap_obj(self.cap_at(cnode, slot)?)
    }

    /// Ảnh chụp cap; `Err(InvalidCapability)` khi id không hợp lệ (0 hoặc quá dải).
    /// Cap **đã chết** vẫn trả `Ok` với `alive = false` để test kiểm dấu vết.
    pub fn cap_info(&self, cap: CapId) -> Result<CapInfo, KError> {
        let c = self.cap(cap)?;
        Ok(CapInfo {
            cap,
            obj: c.obj,
            ty: c.ty,
            parent: c.parent,
            slot: c.slot,
            rights: c.rights,
            badge: c.badge,
            alive: c.alive,
        })
    }

    /// Toàn bộ bảng cap theo thứ tự cấp phát (gồm cap đã chết) — test/property dùng.
    pub fn cap_table(&self) -> Vec<CapInfo> {
        (1..=self.caps.len() as CapId)
            .map(|cap| {
                self.cap_info(cap)
                    .expect("cap_table: id trong dải phải hợp lệ")
            })
            .collect()
    }

    pub fn live_cap_count(&self) -> usize {
        self.caps.iter().filter(|c| c.alive).count()
    }

    /// Con trực tiếp còn sống của `cap`, theo thứ tự cấp phát (thứ tự MDB của mô hình).
    pub fn cap_children(&self, cap: CapId) -> Result<Vec<CapId>, KError> {
        self.cap(cap)?;
        Ok((1..=self.caps.len() as CapId)
            .filter(|i| {
                let c = &self.caps[*i as usize - 1];
                c.alive && c.parent == Some(cap)
            })
            .collect())
    }

    pub fn cnode_slot_count(&self, cnode: ObjId) -> Result<usize, KError> {
        let (alive, slots) = {
            let m = self.meta(cnode)?;
            (m.alive, m.slots)
        };
        // CNode đã finalise: slot đã xoá trắng trước khi body bị destroy; giữ nguyên
        // kết quả API cũ (`SlotEmpty`) bằng số slot ghi trong metadata.
        if !alive {
            return slots.ok_or(KError::WrongObjectType);
        }
        self.body(cnode, |k| match k {
            Kind::CNode { slots } => Ok(slots.len()),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn object_alive(&self, obj: ObjId) -> bool {
        self.objects
            .get(obj as usize - 1)
            .map(|m| m.alive)
            .unwrap_or(false)
    }

    pub fn object_type(&self, obj: ObjId) -> Option<ObjType> {
        self.objects.get(obj as usize - 1).map(|m| m.ty)
    }

    /// `(offset, len)` trong Untyped; `None` với object không cấp từ Untyped.
    pub fn object_bytes(&self, obj: ObjId) -> Option<(usize, usize)> {
        let m = self.objects.get(obj as usize - 1)?;
        m.untyped.map(|_| (m.offset, m.len))
    }

    /// Địa chỉ **thật** của body trong arena objmem (KT-16).
    pub fn object_addr(&self, obj: ObjId) -> Option<u64> {
        self.objects.get(obj as usize - 1).map(|m| m.body.pa())
    }

    /// `(base, capacity)` của vùng nhớ thật của một Untyped (KT-16); `None` nếu không phải
    /// Untyped. Object Retype từ Untyped đó nằm trong `[base, base + capacity)`.
    pub fn untyped_arena(&self, untyped: ObjId) -> Option<(u64, usize)> {
        let m = self.objects.get(untyped as usize - 1)?;
        if m.ty != ObjType::Untyped {
            return None;
        }
        let r = m.region?;
        Some((self.regions[r].base(), self.regions[r].capacity()))
    }

    /// Truy cập body qua `vifast-objmem` (KT-16): object đã finalise ⇒ `Dead`.
    ///
    /// Đây là đường duy nhất lộ body ra ngoài crate mô hình (dạng `&dyn KObj`), dùng cho
    /// test tích hợp; `ObjRef` không mang identity store nên không lộ ref thô.
    pub fn with_object<R>(&self, obj: ObjId, f: impl FnOnce(&dyn KObj) -> R) -> Result<R, Dead> {
        let Some(m) = self.objects.get(obj as usize - 1) else {
            return Err(Dead);
        };
        self.store(m.loc).with(m.body, |k| f(k))
    }

    pub fn untyped_size(&self, obj: ObjId) -> usize {
        let Some(m) = self.objects.get(obj as usize - 1) else {
            return 0;
        };
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region.map(|r| self.regions[r].capacity()).unwrap_or(0)
    }

    pub fn untyped_used(&self, obj: ObjId) -> usize {
        let Some(m) = self.objects.get(obj as usize - 1) else {
            return 0;
        };
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region.map(|r| self.regions[r].used()).unwrap_or(0)
    }

    pub fn untyped_free(&self, obj: ObjId) -> usize {
        let size = self.untyped_size(obj);
        size.saturating_sub(self.untyped_used(obj))
    }

    /// Tổng `len` của object **sống** cấp từ Untyped (không gồm byte đệm căn lề).
    pub fn untyped_allocated(&self, obj: ObjId) -> usize {
        let Some(idx) = self.objects.get(obj as usize - 1).map(|_| obj as usize - 1) else {
            return 0;
        };
        self.objects
            .iter()
            .filter(|m| m.alive && m.untyped == Some(idx))
            .map(|m| m.len)
            .sum()
    }

    /// Byte đệm căn lề nằm giữa watermark và tổng `len` của object sống.
    pub fn untyped_gap(&self, obj: ObjId) -> usize {
        self.untyped_used(obj)
            .saturating_sub(self.untyped_allocated(obj))
    }

    /// Tổng watermark của mọi Untyped còn sống — dùng cho trace/tất định.
    pub fn total_used(&self) -> usize {
        self.objects
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::Untyped)
            .map(|m| m.region.map(|r| self.regions[r].used()).unwrap_or(0))
            .sum()
    }

    // ---------------------------------------------------------------- bất biến

    /// Bất biến tổng byte của một Untyped: không chồng lấn, không vượt biên, không đếm hai lần.
    pub fn check_bytes(&self, untyped: ObjId) -> Result<(), String> {
        let Some(m) = self.objects.get(untyped as usize - 1) else {
            return Err(format!("Untyped {untyped} không tồn tại"));
        };
        if m.ty != ObjType::Untyped {
            return Err(format!("object {untyped} không phải Untyped"));
        }
        let Some(region) = m.region else {
            return Err(format!("object {untyped} không có vùng nhớ"));
        };
        let size = self.regions[region].capacity();
        // Watermark của mô hình = `Arena::used()` của vùng (KT-16).
        let watermark = self.regions[region].used();
        if watermark > size {
            return Err(format!("watermark {watermark} > cỡ {size}"));
        }
        let u_idx = untyped as usize - 1;
        let mut ranges: Vec<(usize, usize, ObjId)> = Vec::new();
        let mut allocated = 0usize;
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.untyped != Some(u_idx) {
                continue;
            }
            let end = o.offset + o.len;
            if end > size {
                return Err(format!(
                    "object {} vượt biên Untyped ({end} > {size})",
                    i + 1
                ));
            }
            // Từng range phải nằm trong vùng đã cấp (0..=watermark), không chỉ trong `size`.
            if end > watermark {
                return Err(format!(
                    "object {} kết thúc ở {end} > watermark {watermark}",
                    i + 1
                ));
            }
            allocated += o.len;
            ranges.push((o.offset, end, (i as u32) + 1));
        }
        ranges.sort_unstable();
        for pair in ranges.windows(2) {
            if pair[0].1 > pair[1].0 {
                return Err(format!(
                    "object {} và {} chồng lấn ({:?})",
                    pair[0].2, pair[1].2, pair
                ));
            }
        }
        // Không object nào chồng lấn ⇒ tổng `len` không thể vượt watermark; byte đệm căn lề
        // (gap) là phần `watermark - allocated` và **không** thuộc object nào.
        if allocated > watermark {
            return Err(format!("tổng len {allocated} > watermark {watermark}"));
        }
        let gap = watermark - allocated;
        let free = size - watermark;
        // "đã cấp + còn trống == cỡ ban đầu", tách rõ object thật vs byte đệm căn lề.
        if allocated + gap + free != size {
            return Err(format!(
                "tổng byte sai: allocated {allocated} + gap {gap} + free {free} != {size}"
            ));
        }
        Ok(())
    }

    pub fn check_all_bytes(&self) -> Result<(), String> {
        for (i, m) in self.objects.iter().enumerate() {
            if m.alive && m.ty == ObjType::Untyped {
                self.check_bytes((i as u32) + 1)?;
            }
        }
        Ok(())
    }

    /// Bất biến cây dẫn xuất cap (MDB tối giản) — thiết kế §4.3/§9.6:
    /// - cap sống trỏ object **sống** cùng loại với cap; không cap nào trỏ object chết;
    /// - cap sống có cha ⇒ cha sống và không có chu trình (cha của cap do `Retype` sinh trỏ
    ///   object khác là bình thường — quan hệ dẫn xuất, không phải quan hệ cùng object);
    /// - cap sống nằm trong slot ⇒ CNode sống và slot trỏ ngược lại đúng cap; mọi slot của
    ///   CNode sống chỉ chứa cap sống đúng chiều (không cap chết nằm lại trong slot);
    /// - cap sống là con thì **phải** nằm trong một slot (không mồ côi); cap gốc được phép
    ///   không slot (harness giữ trước khi đặt).
    ///
    /// Bất biến riêng của `Copy`/`Mint` — con trỏ **cùng object** với cha và quyền con
    /// ⊆ quyền cha — được kiểm trong property test CSpace (`tests/kt10_property.rs`), nơi
    /// toàn bộ cây chỉ dựng bằng Copy/Mint.
    pub fn check_caps(&self) -> Result<(), String> {
        // 1) cap ↔ object.
        for (i, c) in self.caps.iter().enumerate() {
            let cap = (i as u32) + 1;
            if !c.alive {
                if c.slot.is_some() {
                    return Err(format!("cap chết {cap} còn giữ slot"));
                }
                continue;
            }
            if c.obj == 0 || c.obj as usize > self.objects.len() {
                return Err(format!("cap {cap} trỏ object ngoài dải ({})", c.obj));
            }
            let m = &self.objects[c.obj as usize - 1];
            if !m.alive {
                return Err(format!("cap {cap} trỏ object {} đã chết", c.obj));
            }
            if m.ty != c.ty {
                return Err(format!(
                    "cap {cap} kiểu {:?} nhưng object {} kiểu {:?}",
                    c.ty, c.obj, m.ty
                ));
            }
        }
        // 2) quan hệ cha–con + không chu trình.
        for (i, c) in self.caps.iter().enumerate() {
            let cap = (i as u32) + 1;
            if !c.alive {
                continue;
            }
            let Some(p) = c.parent else { continue };
            if p == 0 || p as usize > self.caps.len() {
                return Err(format!("cap {cap} có cha ngoài dải ({p})"));
            }
            let parent = &self.caps[p as usize - 1];
            if !parent.alive {
                return Err(format!("cap {cap} mồ côi: cha {p} đã chết"));
            }
            let mut cur = Some(p);
            let mut steps = 0usize;
            while let Some(x) = cur {
                if x == 0 || x as usize > self.caps.len() {
                    return Err(format!("cap {cap}: chuỗi cha có id ngoài dải ({x})"));
                }
                if x == cap {
                    return Err(format!("cap {cap}: chu trình cha"));
                }
                cur = self.caps[x as usize - 1].parent;
                steps += 1;
                if steps > self.caps.len() {
                    return Err(format!("cap {cap}: chuỗi cha dài bất thường"));
                }
            }
        }
        // 3) slot ↔ cap (đối xứng MDB) và slot của CNode sống.
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.ty != ObjType::CNode {
                continue;
            }
            let cnode = (i as u32) + 1;
            let slots = self
                .cnode_slots_of(cnode)
                .map_err(|e| format!("CNode {cnode}: kind lạ ({e:?})"))?;
            for (slot, entry) in slots.iter().enumerate() {
                let Some(cap) = entry else { continue };
                if *cap == 0 || *cap as usize > self.caps.len() {
                    return Err(format!("slot {slot} của CNode {cnode} có cap ngoài dải"));
                }
                let c = &self.caps[*cap as usize - 1];
                if !c.alive {
                    return Err(format!("slot {slot} của CNode {cnode} chứa cap chết {cap}"));
                }
                if c.slot != Some(SlotRef::new(cnode, slot)) {
                    return Err(format!(
                        "slot {slot} của CNode {cnode} trỏ cap {cap} nhưng cap không trỏ ngược"
                    ));
                }
            }
        }
        for (i, c) in self.caps.iter().enumerate() {
            let cap = (i as u32) + 1;
            if !c.alive {
                continue;
            }
            match c.slot {
                Some(SlotRef { cnode, slot }) => {
                    if !self.object_alive(cnode) {
                        return Err(format!("cap {cap} nằm trong CNode {cnode} đã chết"));
                    }
                    if self.object_type(cnode) != Some(ObjType::CNode) {
                        return Err(format!(
                            "cap {cap} nằm trong object {cnode} không phải CNode"
                        ));
                    }
                    let n = self
                        .cnode_slot_count(cnode)
                        .map_err(|e| format!("cap {cap}: CNode {cnode}: {e:?}"))?;
                    if slot >= n {
                        return Err(format!(
                            "cap {cap} ở slot {slot} ngoài dải của CNode {cnode}"
                        ));
                    }
                    if self
                        .cnode_slot(cnode, slot)
                        .map_err(|e| format!("cap {cap}: CNode {cnode}: {e:?}"))?
                        != Some(cap)
                    {
                        return Err(format!(
                            "cap {cap} nói nằm ở ({cnode}, {slot}) nhưng slot không trỏ lại"
                        ));
                    }
                }
                None => {
                    if c.parent.is_some() {
                        return Err(format!("cap {cap} là con nhưng không nằm trong slot nào"));
                    }
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- Retype

    /// Cấp object từ Untyped theo `type/size_bits/flags` (spec `Untyped.Retype`).
    /// Trả `CapId` cho harness (ABI `returns = []`, D2-35); cap là con của cap nguồn.
    ///
    /// **KT-16**: vùng cấp phát là arena objmem của Untyped; `offset` trả về là watermark
    /// cũ, `untyped_used` = `Arena::used()`, nên không còn phép `align_up` viết tay.
    pub fn retype(
        &mut self,
        src: SlotRef,
        ty: ObjType,
        size_bits: u32,
        flags: u32,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (u_idx, u_region, u_size, src_rights) = {
            let c = self.cap(src_cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let idx = self.object_index(c.obj)?;
            let m = &self.objects[idx];
            if m.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let region = m.region.ok_or(KError::WrongObjectType)?;
            (idx, region, self.regions[region].capacity(), c.rights)
        };
        // F3 (review KT-10): Retype là thao tác cấp phát trên Untyped ⇒ cap nguồn phải có
        // WRITE; cap con kế thừa quyền nguồn (không nới ngược).
        if src_rights & RIGHT_WRITE == 0 {
            return Err(KError::InsufficientRights);
        }

        // GUEST: chỉ VSpace nhận; tới H1 VSpace+GUEST vẫn NotSupported (D2-32).
        if flags & FLAG_GUEST != 0 {
            return Err(if ty == ObjType::VSpace {
                KError::NotSupported
            } else {
                KError::InvalidArgument
            });
        }
        if !ty.is_core() {
            return Err(KError::NotSupported);
        }

        let (len, align, child_slots) = match ty {
            ObjType::Untyped => {
                if !(12..=30).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let len = 1usize << size_bits;
                if len > u_size {
                    return Err(KError::NotEnoughMemory);
                }
                // KT-17 (follow-up KT-16): arena của mô hình chỉ bảo đảm căn `ARENA_ALIGN`
                // (4096 B). Untyped con có `len` = căn của block; `size_bits ≥ 13` (8192 B)
                // không thể cấp `pa` đúng căn tuyệt đối ⇒ **từ chối rõ** bằng `Alignment`
                // (trước KT-17: trả `Ok` với `pa` có thể sai căn, vi phạm hợp đồng `Layout`).
                // Đây là giới hạn của arena sim, không phải thay đổi ABI; `vifast-model`
                // (oracle) vẫn nhận 12..=30 và parity 10.000 seed chỉ sinh `size_bits = 12`.
                if len > ARENA_ALIGN {
                    return Err(KError::Alignment);
                }
                (len, len, 0)
            }
            ObjType::CNode => {
                if !(4..=20).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let slots = 1usize << size_bits;
                (
                    CNODE_BASE_BYTES + slots * SLOT_BYTES,
                    CNODE_BASE_BYTES,
                    slots,
                )
            }
            _ => {
                let fixed = ty.fixed_size().ok_or(KError::InvalidArgument)?;
                if size_bits != 0 {
                    if size_bits > 30 {
                        return Err(KError::InvalidArgument);
                    }
                    let requested = 1usize << size_bits;
                    if requested < fixed {
                        return Err(KError::Alignment);
                    }
                    if requested > fixed {
                        return Err(KError::InvalidArgument);
                    }
                }
                (fixed, DEFAULT_ALIGN, 0)
            }
        };

        // Cấp **ngay trong arena của Untyped nguồn** (objmem): block đúng `len`/`align`
        // của spec ⇒ watermark mô hình = `Arena::used()`.
        let kind = Self::fresh_kind(ty, child_slots);
        let (body, offset) = self.regions[u_region]
            .alloc_place_span(kind, len, align)
            .ok_or(KError::NotEnoughMemory)?;
        // Untyped con có vùng nhớ riêng bằng cỡ vùng vừa cắt (tạo **sau** khi cấp body để
        // lỗi hết chỗ không đốt byte).
        let region = if ty == ObjType::Untyped {
            Some(self.push_region(len))
        } else {
            None
        };
        let obj = self.push_meta(
            ty,
            Loc::Untyped(u_region as u32),
            body,
            ChoDat {
                untyped: Some(u_idx),
                offset,
                len,
                region,
                slots: if ty == ObjType::CNode {
                    Some(child_slots)
                } else {
                    None
                },
            },
        );
        // KT-19 DoD 3: Retê có thể tạo TCB `Inactive` (không vào hàng) — kiểm lại bất biến
        // sau mỗi public op như các op TCB khác.
        self.sched_after_op(false);
        Ok(self.push_cap(obj, ty, Some(src_cap), src_rights, None))
    }

    // ---------------------------------------------------------------- CSpace

    /// `CNode.Copy`: tạo cap con ở đích; đích phải rỗng. Giữ nguyên quyền + badge của nguồn.
    pub fn copy(&mut self, src: SlotRef, dst: SlotRef) -> Result<CapId, KError> {
        self.copy_rights(src, dst, None)
    }

    /// `CNode.Copy` với `rights?` của spec (`[method_signature."CNode.Copy"]`): `None` =
    /// giữ quyền nguồn; `Some(r)` phải là tập con, nếu không `InsufficientRights` (F3).
    pub fn copy_rights(
        &mut self,
        src: SlotRef,
        dst: SlotRef,
        rights: Option<u8>,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (obj, ty, src_rights, badge) = {
            let c = self.cap(src_cap)?;
            (c.obj, c.ty, c.rights, c.badge)
        };
        let rights = match rights {
            Some(r) => {
                if r & !src_rights != 0 {
                    return Err(KError::InsufficientRights);
                }
                r
            }
            None => src_rights,
        };
        self.derive(src_cap, obj, ty, dst, rights, badge)
    }

    /// `CNode.Mint`: như Copy nhưng đặt lại quyền (phải ⊆ quyền nguồn, nếu không
    /// `InsufficientRights`) và badge (chỉ Endpoint/Notification; `None`/`Some(0)` = giữ
    /// badge nguồn). Badge cho loại khác ⇒ `InvalidArgument`.
    pub fn mint(
        &mut self,
        src: SlotRef,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (obj, ty, src_rights, src_badge) = {
            let c = self.cap(src_cap)?;
            (c.obj, c.ty, c.rights, c.badge)
        };
        if rights & !src_rights != 0 {
            return Err(KError::InsufficientRights);
        }
        let badge = match badge {
            // F3: `None` = giữ badge nguồn; `Some(0)` = badge 0 (không badge) ⇒ **xoá**.
            None => src_badge,
            Some(0) => None,
            Some(b) => {
                if !matches!(ty, ObjType::Endpoint | ObjType::Notification) {
                    return Err(KError::InvalidArgument);
                }
                Some(b)
            }
        };
        self.derive(src_cap, obj, ty, dst, rights, badge)
    }

    /// Tạo cap con của `src_cap` rồi gắn vào `dst` — đã kiểm nguồn/quyền trước khi cấp
    /// ⇒ thao tác lỗi không để lại cap mồ côi.
    fn derive(
        &mut self,
        src_cap: CapId,
        obj: ObjId,
        ty: ObjType,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        self.check_cnode_slot(dst.cnode, dst.slot)?;
        if self.cnode_slot(dst.cnode, dst.slot)?.is_some() {
            return Err(KError::SlotOccupied);
        }
        let new_cap = self.push_cap(obj, ty, Some(src_cap), rights, badge);
        self.cnode_set_slot(dst.cnode, dst.slot, Some(new_cap))?;
        self.caps[new_cap as usize - 1].slot = Some(dst);
        Ok(new_cap)
    }

    /// `CNode.Delete`: xoá cap trong slot (object không bị finalise — chỉ Revoke mới finalise).
    /// Con của cap bị xoá thuộc về **ông** để cây dẫn xuất không mồ côi (MDB kiểu seL4).
    pub fn delete(&mut self, slot: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(slot.cnode, slot.slot)?;
        // MDB seL4 `cteDelete`: con của cap bị xoá thuộc về ông (tổ tiên sống gần nhất),
        // nếu không cây dẫn xuất sẽ mồ côi và Revoke của ông không còn thấy cháu.
        let mut new_parent = self.cap(cap)?.parent;
        while let Some(p) = new_parent {
            if p != 0 && (p as usize) <= self.caps.len() && self.caps[p as usize - 1].alive {
                break;
            }
            new_parent = if p != 0 && (p as usize) <= self.caps.len() {
                self.caps[p as usize - 1].parent
            } else {
                None
            };
        }
        for c in &mut self.caps {
            if c.alive && c.parent == Some(cap) {
                c.parent = new_parent;
            }
        }
        let (obj, ty) = {
            let c = self.cap(cap)?;
            (c.obj, c.ty)
        };
        self.clear_slot_of(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.alive = false;
            c.parent = None;
        }
        // **KT-25** — cap Reply bị xoá ⇒ caller đang `BlockedOnReply` được đánh thức
        // `Cancelled` (D2-53 Q8, `.scn` 07).
        if ty == ObjType::Reply {
            self.cancel_reply_of_cap(obj)?;
        }
        // **KT-25** — `Delete` cũng làm cap chết: Reply cap chết còn nằm trong Recv waiter của
        // Endpoint phải được coi như "không cấp Reply" (Q7(b)), nếu không `Call` sau đó gặp cap
        // chết và trả **cứng** `InvalidCapability` (message không giao) — `.scn` 07 tạo đúng
        // trạng thái này. **Không** quét toàn bộ ref khác ở đây: `fault_ep`/`cspace`... có ngữ
        // nghĩa riêng (vd KT-18: `Exit` với `fault_ep` đã chết ⇒ `InvalidCapability`).
        if ty == ObjType::Reply {
            self.clear_dead_reply_in_waiters();
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `CNode.Revoke`: xoá cap con/cháu của cap trong `slot` (hoặc mọi cap của CNode khi
    /// `slot = None`). Revoke cap Untyped finalise mọi object trong Untyped và đặt
    /// watermark về 0 (thiết kế §4.2). Trả số cap bị xoá.
    pub fn revoke(&mut self, cnode: ObjId, slot: Option<usize>) -> Result<usize, KError> {
        let targets: Vec<CapId> = {
            let slots = self.cnode_slots_of(cnode)?;
            match slot {
                Some(s) => {
                    if s >= slots.len() {
                        return Err(KError::RangeError);
                    }
                    match slots[s] {
                        None => return Err(KError::SlotEmpty),
                        Some(cap) => vec![cap],
                    }
                }
                None => slots.iter().flatten().copied().collect(),
            }
        };
        let mut deleted = 0;
        for target in targets {
            // Cap đích có thể đã bị xoá bởi lần revoke trước (cap lồng trong cùng CNode).
            if !self.cap(target)?.alive {
                continue;
            }
            deleted += self.revoke_children(target)?;
        }
        self.sched_after_op(true);
        Ok(deleted)
    }

    // ---------------------------------------------------------------- cấu hình

    /// `Tcb.Configure`: gắn CSpace; **xoá `exit_code`** (D2-25: `Configure`/`Resume` xoá).
    pub fn tcb_configure(&mut self, tcb: SlotRef, cspace: Option<SlotRef>) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let cspace_obj = match cspace {
            Some(s) => Some(self.require_type(s, ObjType::CNode)?),
            None => None,
        };
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.cspace = cspace_obj;
                t.exit_code = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    pub fn tcb_set_fault_ep(&mut self, tcb: SlotRef, ep: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ep_cap = self.cap_at(ep.cnode, ep.slot)?;
        if self.cap(ep_cap)?.ty != ObjType::Endpoint {
            return Err(KError::WrongObjectType);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.fault_ep = Some(ep_cap);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    pub fn sched_control_configure(
        &mut self,
        sched_control: SlotRef,
        sched: SlotRef,
        budget: u64,
        period: u64,
    ) -> Result<(), KError> {
        self.require_type(sched_control, ObjType::SchedControl)?;
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        if budget == 0 || period == 0 || budget > period {
            return Err(KError::InvalidArgument);
        }
        let now = self.now;
        self.body_mut(sched_obj, |k| match k {
            Kind::SchedContext(s) => {
                s.budget.configure(now, budget, period);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-20: `Configure` hợp lệ luôn đặt `consumed = 0` (cấu hình lại giữa chu kỳ = bắt
        // đầu chu kỳ mới theo `period` mới — quy ước model-only `[TK]`, xem `mcs`) nên TCB
        // đang gắn (kể cả `Running`) chắc chắn còn budget; chỉ cần kiểm bất biến sau op.
        self.sched_after_op(false);
        Ok(())
    }

    /// `SchedContext.Bind(sc, reply|notification)` — chỉ **gắn/kiểm** quan hệ cấu trúc
    /// (chữ ký đã duyệt, D2-33). **KT-21 DoD 3**: `Bind` **không** tạo/xoá donation (donation tự
    /// động qua `Call`/`Reply`, D2-49(c)); `Bind` Notification chỉ kiểm cấu trúc (thức dậy thật ở
    /// K1c). `Bind` không đổi ngân sách.
    pub fn sched_context_bind(&mut self, sched: SlotRef, target: SlotRef) -> Result<(), KError> {
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        let target_cap = self.cap_at(target.cnode, target.slot)?;
        let target_ty = self.cap(target_cap)?.ty;
        if !matches!(target_ty, ObjType::Reply | ObjType::Notification) {
            return Err(KError::WrongObjectType);
        }
        self.body_mut(sched_obj, |k| match k {
            Kind::SchedContext(s) => {
                s.bound = Some(target_cap);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    // ------------------------------------------- trường TCB qua model API `[TK]` (KT-18 DoD 1)
    //
    // Các method dưới đây **không** có chữ ký trong draft D2-33 (trừ `Tcb.ReadState`), nên
    // K1b chỉ cài **model API**, không tuyên bố ABI (D2-49(b,c)). KT-21 sẽ dùng chúng cho
    // donation/MCP.

    /// `Tcb.SetPriority`/`Tcb.SetMcp` `[TK]`: `max_controlled_priority ≥ priority`
    /// (thiết kế §4.5 `:311`: MCP ngăn tự nâng ưu tiên).
    pub fn tcb_set_priority(
        &mut self,
        tcb: SlotRef,
        priority: u8,
        max_controlled_priority: u8,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        if max_controlled_priority < priority {
            return Err(KError::InvalidArgument);
        }
        // KT-21: priority của **donor** đang được donate không đổi được (sẽ đổi priority hiệu
        // lực của donee giữa chừng) — phải `Reply`/huỷ donation trước. Donee thì đổi được
        // `priority` gốc (không ảnh hưởng khi đang donate) và **MCP** (đổi priority hiệu lực).
        let info = self.tcb_info_obj(tcb_obj)?;
        if self.is_donation_donor(tcb_obj) {
            return Err(KError::IllegalOperation);
        }
        // KT-19: priority đổi ⇒ gỡ khỏi bucket cũ **trước** khi sửa (bucket == priority hiệu
        // lực), rồi `sched_sync` đưa vào bucket mới nếu đang `Ready`. Donee phải gỡ cả khi
        // không `Ready` (MCP đổi ⇒ priority hiệu lực đổi).
        if info.state == TcbState::Ready || info.donated_sc.is_some() {
            self.sched_remove_all(tcb_obj);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.priority = priority;
                t.max_controlled_priority = max_controlled_priority;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.SetAffinity` `[TK]`: `HartMask` ≤ 64 bit, khác 0, chỉ bit của hart có thật.
    pub fn tcb_set_affinity(&mut self, tcb: SlotRef, affinity: u64) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let hart_mask = (1u64 << HART_COUNT) - 1;
        if affinity == 0 || affinity & !hart_mask != 0 {
            return Err(KError::InvalidArgument);
        }
        if self.tcb_state(tcb_obj)? == TcbState::Ready {
            self.sched_remove_all(tcb_obj);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.affinity = affinity;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.SetHome` `[TK]`: home phải là hart có thật (K1b: hart 0).
    pub fn tcb_set_home(&mut self, tcb: SlotRef, home: u16) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        if home >= HART_COUNT {
            return Err(KError::InvalidArgument);
        }
        if self.tcb_state(tcb_obj)? == TcbState::Ready {
            self.sched_remove_all(tcb_obj);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.home = home;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// Model API `set_sched_params(tcb, sc)` — **D2-49(b)**: gắn/gỡ `SchedContext` của TCB,
    /// không sửa `spec/`/ABI ở K1b (`Tcb.SetSchedParams` để K1c/K3).
    pub fn set_sched_params(&mut self, tcb: SlotRef, sched: Option<SlotRef>) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let sched_obj = match sched {
            Some(s) => Some(self.require_type(s, ObjType::SchedContext)?),
            None => None,
        };
        // KT-21: không đổi ngân sách của TCB đang nhận donation / đang cho mượn SC (phải
        // `Reply` hoặc huỷ donation trước); không lấy SC đang bị donate cho TCB khác.
        let info = self.tcb_info_obj(tcb_obj)?;
        if info.donated_sc.is_some() || self.is_donation_donor(tcb_obj) {
            return Err(KError::IllegalOperation);
        }
        if let Some(sc) = sched_obj {
            if self.donation_of(sc)?.0.is_some() {
                return Err(KError::Busy);
            }
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.sched_context = sched_obj;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-20: gắn SC **đã cạn** cho TCB đang `Running` ⇒ ngủ ngay (`Running → Sleeping`
        // là cặp ALLOWED; không để `Running` mà hết budget). TCB `Ready` giữ nguyên trong
        // hàng — `pick` bỏ qua theo điều kiện budget, tới biên refill lại chọn được.
        self.enforce_budget(tcb_obj)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// Bind notification vào TCB (thiết kế §4.7): model API `[TK]`, chưa có chữ ký ABI.
    pub fn tcb_bind_notification(&mut self, tcb: SlotRef, ntfn: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.bound_notification = Some(ntfn_obj);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    /// Ảnh chụp toàn bộ trường TCB của mô hình (đọc/ghi qua model API `[TK]`).
    pub fn tcb_info(&self, tcb: SlotRef) -> Result<TcbInfo, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        self.tcb_info_obj(obj)
    }

    /// **KT-25** — bind **receive window** (CPTR 4 ô CSpace) cho TCB — model API `[TK]`,
    /// **không** thêm chữ ký ABI (`Tcb.Configure` của `spec/abi.toml` chỉ có CSpace; D2-33).
    ///
    /// Mỗi ô phải là slot **có thật** của một CNode còn sống; các ô phải **khác nhau** (hai cap
    /// cài vào cùng một slot sẽ đè nhau ⇒ `InvalidArgument`). Bind `[s0..s3]` với các ô **rỗng**
    /// là hợp lệ — khác "chưa bind" (`None`: message có cap ⇒ `InvalidArgument` vì không có chỗ
    /// cài, KT-25 rủi ro (b) của K1C-PLAN).
    pub fn tcb_set_recv_window(
        &mut self,
        tcb: SlotRef,
        window: [SlotRef; ipc::MAX_CAPS],
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        for (i, w) in window.iter().enumerate() {
            self.check_cnode_slot(w.cnode, w.slot)?;
            if window[..i].contains(w) {
                return Err(KError::InvalidArgument);
            }
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.recv_window = Some(window);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-25** — receive window hiện tại của TCB (`None` = chưa bind).
    pub fn tcb_recv_window(&self, tcb: ObjId) -> Result<Option<[SlotRef; ipc::MAX_CAPS]>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.recv_window),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Như [`Machine::tcb_info`] nhưng nhận thẳng `ObjId` — cho scheduler (KT-19) đọc
    /// `state`/`suspended`/`priority`/`affinity`/`home` không cần cap.
    fn tcb_info_obj(&self, tcb: ObjId) -> Result<TcbInfo, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.snapshot()),
            _ => Err(KError::WrongObjectType),
        })
    }

    // ------------------------------------------- một cửa chuyển trạng thái (KT-18 DoD 3+4)

    /// `Tcb.ReadState` — trả **4 giá trị** `{state, suspended, blocked_on_kind, exit_code}`
    /// (thiết kế §4.4 `:282`, D2-33). Trước KT-18 hàm này trả `TcbState` (drift đã sửa).
    pub fn tcb_read_state(&self, tcb: SlotRef) -> Result<TcbSnapshot, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        self.body(obj, |k| match k {
            Kind::Tcb(t) => Ok(TcbSnapshot {
                state: t.state,
                suspended: t.suspended,
                blocked_on_kind: t.state.blocked_on_kind(),
                exit_code: t.exit_code,
            }),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **Một cửa** đổi trạng thái TCB (D2-49(d)): tra bảng [`ALLOWED`]; cặp cấm trả
    /// `IllegalOperation` và **không** đổi trạng thái; mỗi lần chuyển hợp lệ cộng
    /// `pair_hits` để `transition_coverage()` báo phủ cặp.
    ///
    /// Mọi đường `call`/`recv`/`reply`/`exit`/`tcb_suspend`/`finalise_tcb` dùng hàm này.
    /// Thao tác **không** đổi trạng thái (ví dụ `Resume`, `Suspend` trên `Ready`/`Sleeping`)
    /// **không** gọi cổng — bảng không có cặp danh tính ngoài `BlockedOnSend → BlockedOnSend`.
    pub fn transition_tcb(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        let from = self.tcb_state(tcb)?;
        if !crate::tcb::allowed(from, to) {
            // KT-23: đếm **cặp cấm bị thử** (DoD 3) — không đổi trạng thái, không đếm hit.
            self.forbidden_hits[from.index() * TCB_STATE_COUNT + to.index()] += 1;
            return Err(KError::IllegalOperation);
        }
        // KT-21: rời `BlockedOnReply` bằng đường **khác** `Reply` (wake/suspend/exit/finalise
        // hoặc `transition_tcb` trực tiếp) = bỏ cuộc gọi ⇒ SC donation phải trả về client ngay,
        // không để SC treo trên TCB không còn chờ reply (bất biến "donor đang BlockedOnReply").
        if matches!(from, TcbState::BlockedOnReply { .. })
            && !matches!(to, TcbState::BlockedOnReply { .. })
            && self.is_donation_donor(tcb)
        {
            self.release_donation_of(tcb)?;
        }
        // Bất biến ≤1 `Running`/hart (KT-19 DoD 2): TCB vào `Running{hart}` khi hart đang có
        // TCB chạy ⇒ đưa TCB cũ về `Ready` **trước**, qua đúng cửa này (không đường tắt).
        if let TcbState::Running { hart } = to {
            if hart as usize >= self.sched.len() {
                return Err(KError::InvalidArgument);
            }
            if let Some(cur) = self.sched[hart as usize].current() {
                if cur != tcb {
                    self.transition_tcb(cur, TcbState::Ready)?;
                }
            }
        }
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.state = to;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.pair_hits[from.index() * TCB_STATE_COUNT + to.index()] += 1;
        // KT-23: đếm hoạt động ngân sách ở **một cửa** (DoD 2/8): `Running` hết budget ⇒
        // `Sleeping{Budget}` (bảng §4.5 `:302-311`; refill đếm ở `process_refills`).
        if matches!(from, TcbState::Running { .. })
            && matches!(
                to,
                TcbState::Sleeping {
                    reason: SleepReason::Budget
                }
            )
        {
            self.budget_sleeps += 1;
        }
        // KT-19: state đổi ⇒ ready queue/`current` đổi theo (một cửa duy nhất).
        self.sched_sync(tcb)?;
        Ok(())
    }

    /// Số lần cặp `(from, to)` (chỉ số `TcbState::index`) đã đi qua cổng.
    pub fn pair_hit_count(&self, from: usize, to: usize) -> u32 {
        self.pair_hits[from * TCB_STATE_COUNT + to]
    }

    /// `(số cặp ALLOWED đã có ít nhất một hit, tổng số cặp ALLOWED)` — KT-18 DoD 4.
    pub fn transition_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage(&self.pair_hits)
    }

    /// **KT-23** — số lần cặp **cấm** `(from, to)` bị thử (mọi lần trả `IllegalOperation`, state
    /// không đổi). Dùng cho DoD 3: **mọi** cặp cấm phải được thử ít nhất một lần.
    pub fn forbidden_attempt_count(&self, from: usize, to: usize) -> u32 {
        self.forbidden_hits[from * TCB_STATE_COUNT + to]
    }

    /// **KT-23** — `(số cặp thiết kế đã có hit, 46)`; bỏ 2 cặp `Stopping` reserved (K1c).
    pub fn design_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage_design(&self.pair_hits)
    }

    /// Log finalise TCB (theo thứ tự finalise) — KT-18 DoD 9.
    pub fn finalise_log(&self) -> &[FinaliseEntry] {
        &self.finalise_log
    }

    pub fn tcb_state(&self, tcb: ObjId) -> Result<TcbState, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.state),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `Tcb.Suspend(mode)` (thiết kế §4.4 `:278-283`).
    ///
    /// - `Abort`: gỡ TCB khỏi **mọi** hàng chờ **trước** khi bật cờ `suspended`; `BlockedOn*`
    ///   và `Running` → `Ready`; `Ready` giữ `Ready`; `Sleeping` **giữ** `Sleeping` (bảng §4.5
    ///   `:302-311`). `exit_code` **giữ nguyên** (D2-25, DoD 8).
    /// - `Defer`: K1b chưa có thao tác dài có `Restart` (§4.4 `:282`) ⇒ **luôn**
    ///   `InvalidArgument`, trạng thái không đổi (DoD 6).
    /// - `Inactive`/`Stopping`/`Dead`: không có gì để dừng ⇒ `IllegalOperation`.
    pub fn tcb_suspend(&mut self, tcb: SlotRef, mode: SuspendMode) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if mode == SuspendMode::Defer {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        if !state.is_live() || state == TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        // KT-21 DoD 5: huỷ donation **trước** khi gỡ hàng/đổi trạng thái — TCB là donor thì thu
        // hồi SC (donee đang `Running` mất ngân sách ⇒ `Sleeping{Budget}` ngay, D2-50); TCB là
        // donee thì trả SC về client và đánh thức client (reply sẽ không tới).
        self.release_donation_of(obj)?;
        // Invariant §4.4: gỡ khỏi hàng **trước** khi bật cờ để bất biến kiểm được sau event.
        self.unlink_tcb_queues(obj);
        if state.is_blocked() || matches!(state, TcbState::Running { .. }) {
            self.transition_tcb(obj, TcbState::Ready)?;
        }
        self.set_tcb_suspended(obj, true)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.Resume`: chỉ xoá cờ `suspended` + `exit_code` (D2-25); **không bao giờ** phát lại
    /// Call/Send/Recv/ReplyRecv (thiết kế §4.4 `:283`, DoD 7). Idempotent khi TCB không treo.
    pub fn tcb_resume(&mut self, tcb: SlotRef) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if self.tcb_state(obj)? == TcbState::Dead {
            return Err(KError::IllegalOperation);
        }
        let dang_treo = self.tcb_info_obj(obj)?.suspended;
        self.body_mut(obj, |k| match k {
            Kind::Tcb(t) => {
                t.suspended = false;
                t.exit_code = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-19 DoD 5: chỉ khi thực sự gỡ cờ `suspended` mới enqueue lại **đúng priority**;
        // `Resume` trên TCB đang chạy không được làm nó nhảy về đuôi hàng FIFO.
        if dang_treo {
            self.sched_sync(obj)?;
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-26** — model API `[TK]` `tcb_resume_with_regs(tcb, regs)`: cho TCB **chạy tiếp** sau
    /// fault với **thanh ghi/MR mới**, đúng đường thiết kế §4.4 `:279` — *"message ghi thẳng vào
    /// context + IPC buffer rồi mới chuyển `Ready`. Đường mất message kiểu r319 không biểu diễn
    /// được"*. Đây là **một cửa** cho fault-resume:
    ///
    /// - Ghi `regs` qua **cùng** [`crate::ipc::transfer`] (một message thanh ghi được `admit` +
    ///   giao như message thường: `ipc_sent`/`ipc_delivered` cùng tăng, `check_ipc` giữ nguyên)
    ///   **trước** khi đổi trạng thái ⇒ không có cửa sổ "Ready nhưng message chưa tới" (r319).
    /// - Nhận các trạng thái: `BlockedOnReply{reply}` (fault đã giao, procsrv đang giữ Reply),
    ///   `BlockedOnSend{call:true, fault:Some(k≠Exit)}` (fault còn trong hàng — huỷ message đó),
    ///   `Ready` (bỏ cờ `suspended` — `Tcb.Resume` + thanh ghi) và `Sleeping` (giữ **nguyên**
    ///   `Sleeping{reason}`, bảng §4.5 `:309-310`). Trạng thái khác ⇒ `IllegalOperation`.
    /// - **Không phát lại** Call/Send/Recv/Reply cũ: trả TCB về `Ready` (hoặc giữ `Sleeping`)
    ///   **đúng priority** của nó; không đụng `exit_code` (khác `Tcb.Resume` của KT-18 — D2-25
    ///   chỉ buộc `Tcb.Resume`/`Configure` xoá `exit_code`; đường fault-resume phải **giữ** mã
    ///   để procsrv còn đọc được).
    /// - Message **không** mang cap (model API cho thanh ghi; chuyển cap thuộc Call/Reply).
    pub fn tcb_resume_with_regs(&mut self, tcb: SlotRef, msg: Message) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if !msg.caps.is_empty() || msg.caps.len() != msg.info.caps() as usize {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        let suspended = self.tcb_info_obj(obj)?.suspended;
        // Trạng thái đích — kiểm **trước** khi gỡ hàng/giao thanh ghi (lỗi không đổi gì).
        match state {
            TcbState::BlockedOnReply { reply } => {
                if self.reply_caller(reply)? != Some(obj) {
                    return Err(KError::IllegalOperation);
                }
            }
            TcbState::BlockedOnSend {
                call: true,
                fault: Some(kind),
                ..
            } if !kind.is_exit() => {}
            TcbState::Ready => {}
            TcbState::Sleeping { .. } => {}
            _ => return Err(KError::IllegalOperation),
        }
        // Gỡ khỏi **mọi** hàng chờ/Reply.caller **trước** khi giao (bất biến §4.4); với
        // `BlockedOnSend` đây là chỗ huỷ message fault đang chờ (fault message không có `msg_id`
        // nên không tăng `ipc_dropped`).
        self.unlink_tcb_queues(obj);
        // Ghi thanh ghi **trước** khi `Ready` — qua đúng một hàm `ipc::transfer`.
        let snaps = self.resolve_caps(&msg)?;
        let mut m = self.admit_message(msg, snaps.clone())?;
        self.deliver_message(obj, &mut m, &snaps, None)?;
        // Bỏ cờ `suspended` (nếu có) rồi về trạng thái đích qua **một** cửa `transition_tcb`.
        if suspended {
            self.set_tcb_suspended(obj, false)?;
        }
        match state {
            // `Sleeping` giữ nguyên (bảng §4.5 `:309-310`; không có cặp danh tính trong bảng).
            TcbState::Sleeping { .. } => {}
            // Đã `Ready`: nếu vừa bỏ `suspended`, `set_tcb_suspended(false)` đã sync đúng bucket;
            // nếu không, TCB vẫn ở đúng chỗ — **không** gọi `transition_tcb` (cặp danh tính cấm).
            TcbState::Ready => {}
            _ => self.transition_tcb(obj, TcbState::Ready)?,
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// Finalise TCB (thiết kế §4.3 bảng Finalise; KT-18 DoD 9): gỡ khỏi mọi hàng chờ, chuyển
    /// `Dead` qua một cửa, trả `sched_context` bằng hook rỗng, ghi log.
    ///
    /// `revoke_children` gọi hàm này **trước** `destroy` trong objmem. Gỡ ready queue/refill
    /// queue là hook của KT-19/KT-20 (K1b **chưa có** scheduler).
    pub fn finalise_tcb(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Dead {
            return Ok(());
        }
        // KT-21 DoD 5: huỷ donation trước khi finalise — không để SC trỏ tới TCB chết.
        self.release_donation_of(tcb)?;
        self.unlink_tcb_queues(tcb);
        let released = self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                let released = t.sched_context.take();
                t.bound_notification = None;
                Ok(released)
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.transition_tcb(tcb, TcbState::Dead)?;
        self.finalise_log.push(FinaliseEntry {
            tcb,
            state: TcbState::Dead,
            released_sched_context: released,
        });
        // KT-19 DoD 6: `transition_tcb(Dead)` đã gỡ khỏi ready queue; kiểm lại bất biến.
        self.sched_after_op(false);
        Ok(())
    }

    // ------------------------------------------- scheduler theo hart (KT-19) `[TK]`
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-49(a)): KT-19 dựng **1 hart** nhưng mọi
    // thao tác viết theo hart để K1c thêm hart/push migration. `HartSched` nằm ở
    // [`crate::sched`]; phần dưới là **một cửa** nối TCB ↔ ready queue.
    //
    // Bất biến (DoD 2/3, `check_sched` kiểm sau **mỗi** public op):
    // - `Ready` (không treo) ⇔ có trong ready queue; cùng mức FIFO;
    // - `Running{hart}` ⇔ `current(hart)`, **không** nằm trong hàng; ≤1 `Running`/hart;
    // - bucket == `priority`; `len`/bitmap/`idle`/`cur_prio` khớp.

    /// Số hart của mô hình (KT-19: 1 — D2-49(a); K1c mở rộng).
    pub fn hart_count(&self) -> u16 {
        self.sched.len() as u16
    }

    fn hart_index(&self, hart: u16) -> usize {
        assert!(
            (hart as usize) < self.sched.len(),
            "hart {hart} ngoài số hart {} của mô hình",
            self.sched.len()
        );
        hart as usize
    }

    /// Số TCB trong ready queue của `hart` (không tính `current`).
    pub fn ready_len(&self, hart: u16) -> usize {
        self.sched[self.hart_index(hart)].len()
    }

    /// Số TCB trong bucket mức `prio` của `hart` — kiểm "bucket khớp priority".
    pub fn ready_len_at(&self, hart: u16, prio: u8) -> usize {
        self.sched[self.hart_index(hart)].len_at(prio)
    }

    /// TCB có nằm trong ready queue của `hart` không.
    pub fn ready_contains(&self, hart: u16, tcb: ObjId) -> bool {
        self.sched[self.hart_index(hart)].contains(tcb)
    }

    /// TCB đang chạy của `hart` (`None` ⇔ hart rảnh).
    pub fn current(&self, hart: u16) -> Option<ObjId> {
        self.sched[self.hart_index(hart)].current()
    }

    /// Hart có rảnh không (`current == None`).
    pub fn sched_idle(&self, hart: u16) -> bool {
        self.sched[self.hart_index(hart)].idle()
    }

    /// Mức ưu tiên của TCB đang chạy; `0` khi hart rảnh.
    pub fn sched_cur_prio(&self, hart: u16) -> u8 {
        self.sched[self.hart_index(hart)].cur_prio()
    }

    /// `pick()` O(1) của hart 0 — tiện cho mô hình **1 hart**; nhiều hart dùng
    /// [`Machine::sched_pick_on`].
    pub fn sched_pick(&self) -> Option<ObjId> {
        self.sched_pick_on(0)
    }

    /// `pick()` O(1) của `hart`: mức cao nhất, cùng mức FIFO; **không** gỡ khỏi hàng.
    ///
    /// **KT-20** (follow-up KT-19): chỉ trả TCB **có budget** — TCB không SC **hoặc** SC chưa
    /// `SchedControl.Configure` **không có budget** nên bị bỏ qua, TCB có SC đã cấu hình mà
    /// `consumed == budget` cũng bị bỏ qua (§4.5, K1B-PLAN §5 KT-20 DoD 4). `pick` vẫn O(1)
    /// khi phần tử đầu hàng hợp lệ (KT-19), chỉ quét tiếp khi gặp TCB bị loại.
    /// **KT-21**: server passive (`donated_sc`) được `pick` nhờ ngân sách SC của client; server
    /// passive **không** được donate vẫn bị bỏ qua (D2-50).
    pub fn sched_pick_on(&self, hart: u16) -> Option<ObjId> {
        self.sched[self.hart_index(hart)].pick_where(|tcb| self.tcb_has_budget(tcb))
    }

    /// Bật/tắt preempt **ép sau mỗi** public op (thiết kế §4.13). Mặc định tắt để giữ
    /// nguyên ngữ nghĩa KT-18; test KT-19 bật.
    pub fn set_preempt_every_step(&mut self, on: bool) {
        self.preempt_every_step = on;
    }

    pub fn preempt_every_step(&self) -> bool {
        self.preempt_every_step
    }

    /// `dispatch(hart)`: đưa TCB đang chạy (`Running`) về `Ready` (enqueue lại cuối hàng
    /// cùng mức) rồi chọn TCB mức cao nhất (`Ready → Running`). Trả `None` khi hàng rỗng
    /// (hart rảnh). Mọi chuyển trạng thái đi qua [`Machine::transition_tcb`].
    pub fn dispatch(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        let got = self.dispatch_step(hart)?;
        self.sched_after_op(false);
        Ok(got)
    }

    /// Thân của [`Machine::dispatch`] **không** tự kiểm bất biến — `sched_after_op` gọi
    /// thẳng để một op không phải kiểm hai lần (hiệu năng Miri), còn `dispatch` công khai
    /// kiểm sau khi xong.
    fn dispatch_step(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        let h = hart as usize;
        if let Some(cur) = self.sched[h].current() {
            if !matches!(self.tcb_state(cur)?, TcbState::Running { .. }) {
                return Err(KError::IllegalOperation);
            }
            self.transition_tcb(cur, TcbState::Ready)?;
        }
        let next = match self.sched_pick_on(hart) {
            Some(tcb) => tcb,
            None => {
                self.sched[h].clear_current();
                return Ok(None);
            }
        };
        // Hàng chỉ chứa TCB `Ready` + `!suspended`; kiểm lại để lỗi rõ nếu bất biến vỡ.
        let info = self.tcb_info_obj(next)?;
        if info.state != TcbState::Ready || info.suspended {
            return Err(KError::IllegalOperation);
        }
        self.transition_tcb(next, TcbState::Running { hart })?;
        Ok(Some(next))
    }

    /// `block_current(hart, to)` — model API `[TK]` (DoD 5): TCB đang chạy của hart chuyển
    /// sang trạng thái chặn/ngủ `to` qua [`Machine::transition_tcb`] và rời `current`.
    /// Chỉ nhận trạng thái `BlockedOn*`/`Sleeping` (không dùng để đổi sang `Ready`).
    ///
    /// Đây là **primitive scheduler** cho test, không công bố wait trên object nào: các
    /// đường IPC thật (`call`/`recv`/`tcb_wait_*`) mới ghi hàng chờ của Endpoint/Reply/
    /// Notification; [`Machine::wake`] gỡ mọi tham chiếu còn lại trước khi `→ Ready`.
    pub fn block_current(&mut self, hart: u16, to: TcbState) -> Result<ObjId, KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        if !(to.is_blocked() || matches!(to, TcbState::Sleeping { .. })) {
            return Err(KError::InvalidArgument);
        }
        let cur = self.sched[hart as usize]
            .current()
            .ok_or(KError::IllegalOperation)?;
        self.unlink_tcb_queues(cur);
        self.transition_tcb(cur, to)?;
        self.sched_after_op(false);
        Ok(cur)
    }

    /// `wake(tcb)` — model API `[TK]` (DoD 5): đánh thức TCB đang chặn/ngủ bằng cách gỡ
    /// khỏi hàng chờ của object (endpoint/Reply/notification/address) rồi `→ Ready`.
    /// **Không** phát lại Call/Send/Recv (như `Resume`). TCB `suspended` ⇒ vẫn `Ready` nhưng
    /// **không** vào ready queue (D2-33: `Ready + suspended`). TCB đã `Ready` ⇒ no-op.
    pub fn wake(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.wake_step(tcb)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// Thân của [`Machine::wake`] **không** tự kiểm bất biến — dùng cho đường xử lý hộp thư
    /// (KT-22, [`Machine::process_mail`]) để **một** bước chỉ kiểm bất biến một lần; `wake`
    /// công khai kiểm sau khi xong. Idempotent: TCB đã `Ready` ⇒ no-op.
    fn wake_step(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Ready {
            return Ok(());
        }
        if !(state.is_blocked() || matches!(state, TcbState::Sleeping { .. })) {
            return Err(KError::IllegalOperation);
        }
        self.unlink_tcb_queues(tcb);
        self.transition_tcb(tcb, TcbState::Ready)
    }

    /// `yield_current(hart)` — model API `[TK]` (DoD 5): TCB đang chạy nhường CPU
    /// (`Running → Ready`, vào **cuối** hàng cùng mức) rồi `dispatch` chọn TCB kế tiếp.
    /// Trả `current` mới (có thể chính TCB vừa nhường nếu nó vẫn cao nhất và hàng rỗng).
    pub fn yield_current(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        let cur = self.sched[hart as usize]
            .current()
            .ok_or(KError::IllegalOperation)?;
        self.transition_tcb(cur, TcbState::Ready)?;
        let next = self.dispatch(hart)?;
        self.sched_after_op(false);
        Ok(next)
    }

    // ------------------------------------------- hộp thư theo hart (KT-22) `[TK]`
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-49(f)): mô hình **1 luồng tất định**, không
    // atomic/thread/loom. `post_mail` là **cạnh gửi** (kiểm sức chứa ⇒ `Busy`/`NotSupported`);
    // `process_mail` là **cuối bước** (drain FIFO, mỗi message áp dụng đúng một lần);
    // `send_mail` = gửi + xử lý trong một bước (DoD 6: không có "đã pop chưa áp dụng").

    /// Gửi một message vào hộp thư của hart (không chờ).
    ///
    /// - hart không tồn tại ⇒ `InvalidArgument`; `HART_COUNT == 1` (K1b, D2-49(a)) và message
    ///   `Migrate` ⇒ `NotSupported` — **không** im lặng bỏ (push migration thuộc K1c);
    /// - hộp thư đầy ⇒ `Busy` (backpressure §4.17) — message **không** bị ghi đè/mất, người gửi
    ///   xử lý hộp thư (hoặc gọi [`Machine::process_mail`]) rồi thử lại.
    pub fn post_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        if matches!(msg, MailboxMsg::Migrate { .. }) && self.mail.len() == 1 {
            return Err(KError::NotSupported);
        }
        let posted = match self.mail[hart as usize].try_push(msg) {
            Ok(()) => Ok(()),
            Err(_) => Err(KError::Busy),
        };
        self.sched_after_op(false);
        posted
    }

    /// Xử lý (drain) hộp thư của hart theo **FIFO**; mỗi message áp dụng **đúng một lần**:
    ///
    /// - `Wake(tcb)` — đánh thức TCB đang chặn/ngủ (idempotent: TCB khác `Ready` ⇒ no-op);
    /// - `Reschedule` — `dispatch` hart một lượt;
    /// - `Stop` — bật cờ `stopped` của hart (mô hình; không đổi hàng chờ);
    /// - `Migrate` — không thể có trong hàng ở K1b (đã bị `post_mail` từ chối); di trú thật K1c.
    ///
    /// Trả số message đã xử lý. Hộp thư rỗng sau khi xong (drain tới cạn) ⇒ không có trạng thái
    /// trung gian "message đã pop nhưng chưa áp dụng".
    pub fn process_mail(&mut self, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        let mut n = 0usize;
        while let Some(msg) = self.mail[hart as usize].pop() {
            self.apply_mail(hart, msg);
            self.mail[hart as usize].record(msg);
            n += 1;
        }
        self.sched_after_op(false);
        Ok(n)
    }

    /// `send_mail` = [`Machine::post_mail`] + [`Machine::process_mail`] trong **một** bước:
    /// qua được backpressure thì message được xử lý ngay, không quan sát được hàng đợi giữa
    /// chừng (DoD 6). Lỗi `Busy`/`NotSupported` của `post_mail` giữ nguyên, **không** drain.
    pub fn send_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        self.post_mail(hart, msg)?;
        self.process_mail(hart)?;
        Ok(())
    }

    /// Số message đang nằm trong hộp thư của hart (bất biến: `≤ MAILBOX_CAP`).
    pub fn mail_len(&self, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].len())
    }

    /// Nhật ký message **đã xử lý** của hộp thư hart, đúng thứ tự drain (nguồn kiểm FIFO).
    pub fn mail_log(&self, hart: u16) -> Result<&[MailboxMsg], KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].log())
    }

    /// Hart đã nhận message `Stop` chưa (mô hình `[TK]`; không tắt scheduler — K1b chưa có
    /// quy trình dừng hart).
    pub fn hart_stopped(&self, hart: u16) -> Result<bool, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].stopped())
    }

    /// Áp dụng **một** message lên mô hình — infallible (message không hợp lệ là **no-op** hoặc
    /// `debug_assert`), để drain không để lại trạng thái nửa vời.
    fn apply_mail(&mut self, hart: u16, msg: MailboxMsg) {
        match msg {
            MailboxMsg::Wake(tcb) => {
                let state = match self.tcb_state(tcb) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                if state.is_blocked() || matches!(state, TcbState::Sleeping { .. }) {
                    let _ = self.wake_step(tcb);
                }
            }
            MailboxMsg::Reschedule => {
                if self.dispatch_step(hart).is_err() {
                    debug_assert!(false, "Reschedule: dispatch lỗi (bất biến scheduler vỡ)");
                }
            }
            MailboxMsg::Stop => self.mail[hart as usize].set_stopped(),
            MailboxMsg::Migrate { .. } => {
                // K1b: `post_mail` đã từ chối `Migrate` trên 1 hart ⇒ không thể ở đây.
                debug_assert!(false, "Migrate không được nhận trên 1 hart (K1c)");
            }
        }
    }

    /// Đồng bộ **một** TCB với ready queue sau khi `state`/`suspended`/`priority`/`home`/
    /// `affinity` đổi. Idempotent: gỡ bản cũ rồi đặt chỗ mới theo state hiện tại.
    fn sched_sync(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        // KT-21: bucket dùng **priority hiệu lực** (`donated_priority` khi nhận donation, ngược
        // lại priority gốc) — giống hệt `check_sched`.
        let prio = self.effective_priority(tcb)?;
        // 1) gỡ khỏi bucket priority hiện tại (bất biến: bucket == priority hiệu lực).
        for h in 0..self.sched.len() {
            self.sched[h].remove_at(tcb, prio);
        }
        // 2) nhả `current` nếu TCB không còn `Running` đúng hart đó.
        for h in 0..self.sched.len() {
            let giu = matches!(info.state, TcbState::Running { hart } if hart as usize == h);
            if !giu && self.sched[h].current() == Some(tcb) {
                self.sched[h].clear_current();
            }
        }
        // 3) đặt chỗ mới: Ready (không treo, home trong affinity) hoặc Running.
        match info.state {
            TcbState::Ready if !info.suspended && info.affinity & (1u64 << info.home) != 0 => {
                let h = info.home as usize;
                if h >= self.sched.len() {
                    return Err(KError::InvalidArgument);
                }
                self.sched[h].enqueue(tcb, prio);
            }
            TcbState::Running { hart } => {
                let h = hart as usize;
                if h >= self.sched.len() {
                    return Err(KError::InvalidArgument);
                }
                if let Some(cur) = self.sched[h].current() {
                    if cur != tcb {
                        return Err(KError::IllegalOperation);
                    }
                }
                self.sched[h].set_current(tcb, prio);
            }
            _ => {}
        }
        Ok(())
    }

    /// Gỡ `tcb` khỏi **mọi** bucket của mọi hart — gọi **trước** khi sửa
    /// `priority`/`home`/`affinity` để bucket luôn khớp priority.
    fn sched_remove_all(&mut self, tcb: ObjId) {
        for h in 0..self.sched.len() {
            self.sched[h].remove_all(tcb);
        }
    }

    /// KT-19 DoD 3+4: sau **mỗi** public op — nếu bật `preempt_every_step` thì ép preempt
    /// (`dispatch` đưa `Running` về `Ready` rồi chọn lại), rồi tự kiểm bất biến
    /// [`Machine::check_sched`]. Vi phạm là lỗi mô hình: panic ở build có debug assertion.
    fn sched_after_op(&mut self, preempt: bool) {
        if preempt && self.preempt_every_step {
            for h in 0..self.sched.len() {
                if self.dispatch_step(h as u16).is_err() {
                    debug_assert!(false, "preempt_every_step: dispatch lỗi");
                    return;
                }
            }
        }
        if let Err(e) = self.check_sched() {
            debug_assert!(false, "bất biến scheduler vỡ sau public op: {e}");
        }
        if let Err(e) = self.check_mailboxes() {
            debug_assert!(false, "bất biến mailbox vỡ sau public op: {e}");
        }
        if let Err(e) = self.check_ipc() {
            debug_assert!(
                false,
                "bất biến IPC (bảo toàn message) vỡ sau public op: {e}"
            );
        }
    }

    /// KT-22 — kiểm **toàn bộ** bất biến hộp thư (chạy sau **mỗi** public op qua
    /// [`Machine::sched_after_op`], cùng chỗ với `check_sched`):
    ///
    /// 1. mỗi hộp thư hart: `pushed == popped + removed + len`, `len ≤ cap`;
    /// 2. mỗi Endpoint: sức chứa **cố định** [`MAILBOX_CAP`], cùng bất biến trên, và "có message
    ///    ⇒ không có Recv waiter";
    /// 3. mọi message trong hàng gửi có sender **còn sống**; message **chặn** (`Send`/`Call`
    ///    phải xếp hàng) có sender đúng `BlockedOnSend{ep, call, fault}`; message **không chặn**
    ///    (`NBSend`/`Call no_reply=1` — KT-24) chỉ cần sender còn sống vì người gửi vẫn
    ///    `Ready`/`Running`; fault `Exit` không có `msg_id`, không bao giờ `call = true`.
    ///
    /// **KT-24** đã nới bất biến (3) của KT-22 (trước đây mã hoá `call == fault.is_none()` vì
    /// K1b chỉ có `Call` + fault `Exit`): nay `call` là trường của **message** (`Call` chờ Reply)
    /// và `blocking` là trường của **đường gửi** (chặn/không chặn), không suy từ `fault`.
    ///
    /// Không dùng `HashMap`/`format!` trên đường nóng trừ khi **có** vi phạm (thông điệp lỗi).
    pub fn check_mailboxes(&self) -> Result<(), String> {
        for (h, hb) in self.mail.iter().enumerate() {
            if !hb.invariant() {
                return Err(format!(
                    "hộp thư hart {h}: bất biến pushed/popped/removed vỡ"
                ));
            }
        }
        // Bảng sống theo `ObjId` để tra sender O(1) thay vì quét object (giống `check_sched`).
        let mut song: Vec<bool> = vec![false; self.objects.len() + 1];
        for (i, m) in self.objects.iter().enumerate() {
            song[i + 1] = m.alive;
        }
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            let (cap, len, pushed, popped, removed, items, waiter) = self
                .body(ep, |k| match k {
                    Kind::Endpoint(e) => Ok((
                        e.send.capacity(),
                        e.send.len(),
                        e.send.pushed(),
                        e.send.popped(),
                        e.send.removed(),
                        e.send
                            .iter()
                            .map(|q| {
                                (
                                    q.sender,
                                    q.call,
                                    q.fault,
                                    q.blocking,
                                    q.msg.as_ref().map(|m| (m.msg_id, m.info.length())),
                                )
                            })
                            .collect::<Vec<_>>(),
                        e.recv.map(|(t, _)| t),
                    )),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("Endpoint {ep}: không đọc được body: {e:?}"))?;
            if cap != MAILBOX_CAP {
                return Err(format!("Endpoint {ep}: sức chứa {cap} != {MAILBOX_CAP}"));
            }
            if len > cap {
                return Err(format!("Endpoint {ep}: len {len} > cap {cap}"));
            }
            if pushed != popped + removed + len as u64 {
                return Err(format!(
                    "Endpoint {ep}: message mất/thêm (pushed={pushed} popped={popped} removed={removed} len={len})"
                ));
            }
            if len > 0 && waiter.is_some() {
                return Err(format!(
                    "Endpoint {ep}: có {len} message mà vẫn có Recv waiter"
                ));
            }
            for (sender, call, fault, blocking, msg) in items {
                if !song.get(sender as usize).copied().unwrap_or(false) {
                    return Err(format!(
                        "Endpoint {ep}: sender {sender} đã chết nhưng còn trong hàng"
                    ));
                }
                match msg {
                    Some((msg_id, _len)) => {
                        if msg_id == 0 {
                            return Err(format!(
                                "Endpoint {ep}: message người dùng chưa được cấp msg_id"
                            ));
                        }
                    }
                    None => {
                        if fault.is_none() {
                            return Err(format!(
                                "Endpoint {ep}: message không có nội dung và không phải fault"
                            ));
                        }
                    }
                }
                // **KT-26**: fault `Exit` **không bao giờ** là `Call` (D2-25: đi qua hàng gửi,
                // không Reply); fault ≠ Exit **phải** là `Call` (thiết kế §4.6 `:321`: procsrv
                // nhận qua Call rồi Reply để chạy tiếp). Hai chiều đều cấm nửa vời.
                match fault {
                    None => {}
                    Some(kind) if kind.is_exit() && call => {
                        return Err(format!("Endpoint {ep}: fault Exit nhưng cờ call=true"));
                    }
                    Some(kind) if !kind.is_exit() && !call => {
                        return Err(format!(
                            "Endpoint {ep}: fault {} phải là Call (có Reply)",
                            kind.name()
                        ));
                    }
                    Some(_) => {}
                }
                // Người gửi **chặn** ⇒ phải đang `BlockedOnSend` khớp đúng hàng/cờ/fault.
                // Người gửi **không chặn** (`NBSend`/`Call no_reply=1`) ⇒ chỉ cần còn sống.
                if blocking {
                    match self.tcb_state(sender) {
                        Ok(TcbState::BlockedOnSend {
                            ep: hang_ep,
                            call: hang_call,
                            fault: hang_fault,
                        }) => {
                            if hang_ep != ep || hang_call != call || hang_fault != fault {
                                return Err(format!(
                                    "Endpoint {ep}: sender {sender} ở trạng thái chặn không khớp message"
                                ));
                            }
                        }
                        other => {
                            return Err(format!(
                                "Endpoint {ep}: sender {sender} ở {other:?} nhưng còn trong hàng"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// **KT-24** — kiểm bất biến **bảo toàn message** (K1C-PLAN §1.1 C1) sau mỗi public op:
    ///
    /// 1. mỗi message **người dùng** còn trong mô hình có `msg_id` khác 0, `<= ipc_sent`, và
    ///    **không trùng** (một `msg_id` xuất hiện đúng một nơi: hàng gửi **hoặc** vùng nhận TCB);
    /// 2. message fault `Exit` không có `msg_id` (không phải message IPC) và không có message nào
    ///    vừa không có nội dung vừa không phải fault;
    /// 3. `transfer_calls == ipc_delivered` — **mọi** lần giao đi qua `crate::ipc::transfer`
    ///    (một đường copy MR duy nhất, DoD 3);
    /// 4. `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`; `ipc_pending` đếm lại từ hàng
    ///    thật (không tin bộ đếm), `ipc_dropped` chỉ tăng khi huỷ message (D2-25).
    pub fn check_ipc(&self) -> Result<(), String> {
        // Đường nhanh cho mô hình **chưa từng** dùng IPC (L1 của KT-08..KT-23, hầu hết unit
        // test object): không có message ⇒ mọi bất biến đều đúng khi 3 bộ đếm còn 0. Giữ
        // `check_ipc` sau **mỗi** public op mà không làm chậm L1 1.000 seed (QD §5.2).
        if self.ipc_sent == 0 {
            if self.ipc_delivered == 0 && self.ipc_dropped == 0 && self.transfer_calls == 0 {
                return Ok(());
            }
            return Err("ipc_sent = 0 nhưng bộ đếm giao/huỷ/transfer khác 0".to_string());
        }
        let mut ids: Vec<u64> = Vec::new();
        let mut pending = 0u64;
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            let items = self
                .body(ep, |k| match k {
                    Kind::Endpoint(e) => Ok(e
                        .send
                        .iter()
                        .map(|q| (q.msg.as_ref().map(|m| m.msg_id), q.fault.is_some()))
                        .collect::<Vec<_>>()),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("Endpoint {ep}: không đọc được hàng gửi: {e:?}"))?;
            for (id, is_fault) in items {
                match id {
                    Some(0) => {
                        return Err(format!("Endpoint {ep}: message chưa được cấp msg_id"));
                    }
                    Some(id) => {
                        if id > self.ipc_sent {
                            return Err(format!(
                                "Endpoint {ep}: msg_id {id} > ipc_sent {}",
                                self.ipc_sent
                            ));
                        }
                        if ids.contains(&id) {
                            return Err(format!("msg_id {id} xuất hiện nhiều hơn một nơi"));
                        }
                        ids.push(id);
                        pending += 1;
                    }
                    None => {
                        if !is_fault {
                            return Err(format!(
                                "Endpoint {ep}: message không có nội dung và không phải fault"
                            ));
                        }
                    }
                }
            }
        }
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i + 1) as ObjId;
            let id = self
                .body(tcb, |k| match k {
                    Kind::Tcb(t) => Ok(t.ipc.msg_id),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("TCB {tcb}: không đọc được vùng IPC: {e:?}"))?;
            let Some(id) = id else { continue };
            if id == 0 || id > self.ipc_sent {
                return Err(format!(
                    "TCB {tcb}: msg_id {id} không nằm trong dải 1..=ipc_sent"
                ));
            }
            if ids.contains(&id) {
                return Err(format!(
                    "TCB {tcb}: msg_id {id} vừa ở vùng nhận vừa còn trong hàng gửi"
                ));
            }
        }
        if self.transfer_calls != self.ipc_delivered {
            return Err(format!(
                "transfer_calls {} != ipc_delivered {}",
                self.transfer_calls, self.ipc_delivered
            ));
        }
        if self.ipc_delivered > self.ipc_sent {
            return Err(format!(
                "ipc_delivered {} > ipc_sent {}",
                self.ipc_delivered, self.ipc_sent
            ));
        }
        if self.ipc_sent != self.ipc_delivered + pending + self.ipc_dropped {
            return Err(format!(
                "message mất/thêm: ipc_sent={} != delivered={} + pending={pan} + dropped={}",
                self.ipc_sent,
                self.ipc_delivered,
                self.ipc_dropped,
                pan = pending
            ));
        }
        Ok(())
    }

    // ------------------------------------------- KT-24: bộ đếm/đọc vùng nhận IPC

    /// Số message **người dùng** đã nhận vào đường IPC (message fault `Exit` không tính).
    pub fn ipc_sent(&self) -> u64 {
        self.ipc_sent
    }

    /// Số message đã giao vào vùng nhận của TCB (mỗi lần giao đúng một `ipc::transfer`).
    pub fn ipc_delivered(&self) -> u64 {
        self.ipc_delivered
    }

    /// Số message bị huỷ khi TCB/Endpoint chết hoặc bị `Suspend(Abort)` (D2-25).
    pub fn ipc_dropped(&self) -> u64 {
        self.ipc_dropped
    }

    /// Số lần gọi `crate::ipc::transfer` — **test-only**, luôn bằng [`Machine::ipc_delivered`].
    pub fn transfer_calls(&self) -> u64 {
        self.transfer_calls
    }

    // ------------------------------------------- KT-25: bộ đếm chuyển cap (mềm, D2-53 Q6)

    /// Số cap **cài được** vào receive window qua đường IPC (đã bỏ `RIGHT_GRANT`).
    pub fn cap_transfer_installed(&self) -> u64 {
        self.cap_transfer_installed
    }

    /// Số cap **thất bại mềm** khi chuyển (thiếu `RIGHT_GRANT`, ô đích đã có cap, cap nguồn
    /// chết giữa chừng) — **không** phải bit MessageInfo (D2-53 Q6).
    pub fn cap_transfer_failed(&self) -> u64 {
        self.cap_transfer_failed
    }

    /// Số cap **unwrap** thành badge (cap tới chính Endpoint đang gọi + có badge).
    pub fn cap_unwrapped(&self) -> u64 {
        self.cap_unwrapped
    }

    /// `(cài được, unwrap, thất bại mềm)` — ảnh chụp bộ đếm chuyển cap.
    pub fn cap_transfer_stats(&self) -> (u64, u64, u64) {
        (
            self.cap_transfer_installed,
            self.cap_unwrapped,
            self.cap_transfer_failed,
        )
    }

    /// Số message **người dùng** đang nằm trong các hàng gửi Endpoint (đếm lại từ hàng thật).
    pub fn ipc_pending(&self) -> u64 {
        let mut n = 0u64;
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            if let Ok(v) = self.body(ep, |k| match k {
                Kind::Endpoint(e) => Ok(e.send.iter().filter(|q| q.msg.is_some()).count() as u64),
                _ => Err(KError::WrongObjectType),
            }) {
                n += v;
            }
        }
        n
    }

    /// `MessageInfo` của message **vừa nhận** ở vùng nhận của TCB (`None` = chưa nhận).
    pub fn tcb_ipc_info(&self, tcb: ObjId) -> Result<Option<MessageInfo>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.info),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// 4 MR "thanh ghi" (MR0..MR3) của message vừa nhận ở TCB.
    pub fn tcb_ipc_regs(&self, tcb: ObjId) -> Result<[u64; ipc::WORDS_IN_REGISTERS], KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.regs),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-26** — alias model API `[TK]` của [`Machine::tcb_ipc_regs`] cho đường **fault-resume**:
    /// "thanh ghi" của TCB sau khi được `Reply`/`tcb_resume_with_regs` ghi **trước** `Ready`
    /// (thiết kế §4.4 `:279`, chống đường mất message r319).
    pub fn tcb_regs(&self, tcb: ObjId) -> Result<[u64; ipc::WORDS_IN_REGISTERS], KError> {
        self.tcb_ipc_regs(tcb)
    }

    /// **KT-26** — `(sender, FaultKind)` của **fault vừa giao** cho TCB làm bên nhận (procsrv);
    /// `None` nếu lần nhận gần nhất không phải fault. Model-only `[TK]`: message fault không có
    /// nội dung MR nên procsrv đọc sender + loại fault qua đây.
    pub fn tcb_last_fault(&self, tcb: ObjId) -> Result<Option<(ObjId, FaultKind)>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.last_fault),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-26** — như [`Machine::tcb_last_fault`] nhưng **tiêu thụ** bản ghi (đặt `None`).
    /// `ProcsrvModel::recv_exit` dùng để không xử lý hai lần cùng một fault.
    pub fn take_last_fault(&mut self, tcb: ObjId) -> Result<Option<(ObjId, FaultKind)>, KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.last_fault.take()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// 60 MR "IPC buffer model" (MR4..MR63) của message vừa nhận ở TCB — **vùng riêng**, không
    /// gộp với thanh ghi (KT-24 DoD 2).
    pub fn tcb_ipc_buffer(&self, tcb: ObjId) -> Result<[u64; ipc::WORDS_IN_IPC_BUFFER], KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.buffer),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `msg_id` của message vừa nhận ở TCB (kernel cấp; `None` = chưa nhận).
    pub fn tcb_ipc_msg_id(&self, tcb: ObjId) -> Result<Option<u64>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.msg_id),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Badge của message vừa nhận ở TCB (KT-25 sẽ chuyển badge thật; KT-24 giữ 0).
    pub fn tcb_ipc_badge(&self, tcb: ObjId) -> Result<u64, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.badge),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Lỗi IPC kernel gán cho TCB ở lần giao gần nhất (`Cancelled` cho nhánh `no_reply` (b) —
    /// D2-53 Q7); `None` nếu không có.
    pub fn tcb_last_ipc_error(&self, tcb: ObjId) -> Result<Option<KError>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.last_ipc_error),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Kiểm **toàn bộ** bất biến scheduler (DoD 3):
    ///
    /// 1. không TCB nào trùng trong hàng; bucket khớp `priority`;
    /// 2. mọi TCB trong hàng tồn tại, `Ready`, **không** `suspended`, home trong affinity;
    /// 3. `Ready` (không treo) ⇔ trong hàng; `Running{hart}` ⇔ `current`; trạng thái khác
    ///    **không** nằm trong hàng; ≤ 1 `Running`/hart;
    /// 4. `len`/bitmap khớp; `idle ⇔ current == None`; `cur_prio` khớp priority của `current`.
    ///
    /// Dùng **bảng đánh dấu theo `ObjId`** (`ObjId` 1-based, tối đa `objects.len()`) nên mỗi
    /// TCB tra O(1) thay vì quét 256 hàng — kiểm được sau **mỗi** op mà vẫn rẻ.
    pub fn check_sched(&self) -> Result<(), String> {
        if self.sched.is_empty() {
            return Err("mô hình không có hart nào".to_string());
        }
        // `None` = không nằm trong hàng; `Some(h)` = đang ở ready queue của hart `h`.
        let mut hang_cua: Vec<Option<usize>> = vec![None; self.objects.len() + 1];
        for (h, hs) in self.sched.iter().enumerate() {
            hs.check_local()?;
            hs.for_each_queued(|prio, tcb| {
                let o = hang_cua
                    .get_mut(tcb as usize)
                    .ok_or_else(|| format!("TCB {tcb} trong hàng nhưng ngoài bảng object"))?;
                if o.is_some() {
                    return Err(format!("TCB {tcb} trùng trong ready queue"));
                }
                *o = Some(h);
                let info = self
                    .tcb_info_obj(tcb)
                    .map_err(|e| format!("TCB {tcb} trong hàng nhưng không đọc được: {e:?}"))?;
                if info.state != TcbState::Ready {
                    return Err(format!(
                        "TCB {tcb} trong hàng nhưng state {}",
                        info.state.name()
                    ));
                }
                if info.suspended {
                    return Err(format!("TCB {tcb} suspended nhưng vẫn nằm trong hàng"));
                }
                // KT-21: bucket phải khớp **priority hiệu lực** (priority gốc khi không nhận
                // donation, `donated_priority` khi đang nhận).
                let eff = self
                    .effective_priority(tcb)
                    .map_err(|e| format!("TCB {tcb}: priority hiệu lực lỗi {e:?}"))?;
                if eff != prio {
                    return Err(format!(
                        "TCB {tcb} ở bucket {prio} nhưng priority hiệu lực {eff}"
                    ));
                }
                if info.affinity & (1u64 << info.home) == 0 {
                    return Err(format!(
                        "TCB {tcb} home {} không nằm trong affinity",
                        info.home
                    ));
                }
                Ok(())
            })?;
            match hs.current() {
                Some(cur) => {
                    let info = self
                        .tcb_info_obj(cur)
                        .map_err(|e| format!("current {cur} của hart {h} không đọc được: {e:?}"))?;
                    match info.state {
                        TcbState::Running { hart } if hart as usize == h => {}
                        other => {
                            return Err(format!(
                                "current của hart {h} là TCB {cur} nhưng state {}",
                                other.name()
                            ));
                        }
                    }
                    if hang_cua.get(cur as usize).copied().flatten().is_some() {
                        return Err(format!("TCB {cur} Running nhưng vẫn nằm trong hàng"));
                    }
                    let eff = self
                        .effective_priority(cur)
                        .map_err(|e| format!("current {cur}: priority hiệu lực lỗi {e:?}"))?;
                    if eff != hs.cur_prio() {
                        return Err(format!(
                            "cur_prio {} nhưng priority hiệu lực của TCB {cur} là {eff}",
                            hs.cur_prio()
                        ));
                    }
                    if hs.idle() {
                        return Err(format!("hart {h} có current nhưng idle=true"));
                    }
                }
                None => {
                    if !hs.idle() {
                        return Err(format!("hart {h} không có current nhưng idle=false"));
                    }
                }
            }
        }
        // Mọi TCB sống: Ready ⇔ trong hàng (đúng hart home); Running ⇔ current; còn lại
        // **không** được nằm trong hàng.
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB sống {obj} không đọc được: {e:?}"))?;
            let hang = hang_cua.get(obj as usize).copied().flatten();
            // KT-23: phần "Running còn budget" **tách sang** `check_budget` (DoD 2 — kiểm riêng
            // từng miền bất biến); `check_sched` vẫn gọi `check_budget` ở cuối nên sau mỗi op
            // bất biến vẫn được kiểm đủ.
            match info.state {
                TcbState::Ready => {
                    if info.suspended {
                        if hang.is_some() {
                            return Err(format!("TCB {obj} Ready+suspended nhưng nằm trong hàng"));
                        }
                    } else if hang != Some(info.home as usize) {
                        return Err(format!(
                            "TCB {obj} Ready nhưng không nằm trong ready queue (hart home {})",
                            info.home
                        ));
                    }
                }
                TcbState::Running { hart } => {
                    if self.sched.get(hart as usize).and_then(|x| x.current()) != Some(obj) {
                        return Err(format!(
                            "TCB {obj} Running{{hart={hart}}} nhưng không phải current"
                        ));
                    }
                    if hang.is_some() {
                        return Err(format!("TCB {obj} Running nhưng vẫn nằm trong hàng"));
                    }
                }
                _ => {
                    if hang.is_some() {
                        return Err(format!(
                            "TCB {obj} state {} nhưng vẫn nằm trong hàng",
                            info.state.name()
                        ));
                    }
                }
            }
        }
        // KT-23: hai miền bất biến còn lại tách thành hàm riêng (`check_sc_refs`: tham chiếu
        // `SchedContext`/donation; `check_budget`: ngân sách) để DoD 2 kiểm **từng miền**;
        // `check_sched` vẫn gọi cả hai nên sau **mỗi** public op bất biến vẫn được kiểm đủ
        // (không nới lỏng so với KT-20/KT-21).
        self.check_sc_refs()?;
        self.check_budget()?;
        Ok(())
    }

    /// **KT-23** — miền bất biến **tham chiếu `SchedContext` + donation** (tách nguyên văn từ
    /// `check_sched` của KT-21, không đổi ngữ nghĩa):
    ///
    /// 1. donation **hai chiều** khớp (`SchedContext.donated_to/donor` ↔ `Tcb.donated_sc`), mỗi
    ///    SC ↔ đúng một donee, donee **không** có SC riêng và **không** phải donor (không chuỗi
    ///    donation nhiều tầng), donor đang `BlockedOnReply`, donor ≠ donee, donee còn sống,
    ///    donor/donee **không** cùng `Running`, priority hiệu lực không vượt MCP của một bên;
    /// 2. mọi `Tcb.sched_context`/`Tcb.donated_sc` trỏ tới **object còn sống đúng kiểu**
    ///    `SchedContext` (bắt SC đã bị Revoke/finalise mà TCB còn giữ tham chiếu);
    /// 3. `SchedContext.bound` (nếu có) trỏ tới **cap còn sống** thuộc Reply/Notification.
    pub fn check_sc_refs(&self) -> Result<(), String> {
        let scs = self.live_sched_contexts();
        let mut donee_cua: Vec<Option<ObjId>> = vec![None; self.objects.len() + 1];
        for sc in &scs {
            let (donated_to, donor) = self
                .donation_of(*sc)
                .map_err(|e| format!("SC {sc}: donation không đọc được: {e:?}"))?;
            match (donated_to, donor) {
                (None, None) => {}
                (Some(donee), Some(donor)) => {
                    if donee == donor {
                        return Err(format!("SC {sc}: donor trùng donee"));
                    }
                    let di = self
                        .tcb_info_obj(donee)
                        .map_err(|e| format!("SC {sc}: donee {donee} không đọc được: {e:?}"))?;
                    let oi = self
                        .tcb_info_obj(donor)
                        .map_err(|e| format!("SC {sc}: donor {donor} không đọc được: {e:?}"))?;
                    if di.donated_sc != Some(*sc) {
                        return Err(format!(
                            "SC {sc} donate cho {donee} nhưng donee không trỏ lại (donated_sc={:?})",
                            di.donated_sc
                        ));
                    }
                    if di.sched_context.is_some() {
                        return Err(format!(
                            "donee {donee} vừa có SC riêng vừa nhận donation (D2-49c)"
                        ));
                    }
                    if oi.sched_context != Some(*sc) {
                        return Err(format!("donor {donor} không còn gắn SC {sc}"));
                    }
                    if oi.donated_sc.is_some() {
                        return Err(format!("TCB {donor} vừa donor vừa donee (chuỗi >1)"));
                    }
                    if !matches!(oi.state, TcbState::BlockedOnReply { .. }) {
                        return Err(format!(
                            "donor {donor} state {} nhưng phải BlockedOnReply",
                            oi.state.name()
                        ));
                    }
                    if matches!(di.state, TcbState::Inactive | TcbState::Dead) {
                        return Err(format!("donee {donee} state {}", di.state.name()));
                    }
                    if matches!(di.state, TcbState::Running { .. })
                        && matches!(oi.state, TcbState::Running { .. })
                    {
                        return Err(format!("donor {donor} và donee {donee} cùng Running"));
                    }
                    let eff = crate::mcs::donated_priority(
                        oi.priority,
                        oi.max_controlled_priority,
                        di.max_controlled_priority,
                    );
                    if eff > oi.max_controlled_priority || eff > di.max_controlled_priority {
                        return Err(format!(
                            "SC {sc}: priority donation {eff} vượt MCP (donor {} / donee {})",
                            oi.max_controlled_priority, di.max_controlled_priority
                        ));
                    }
                    if let Some(slot) = donee_cua.get_mut(donee as usize) {
                        if slot.replace(*sc).is_some() {
                            return Err(format!("donee {donee} nhận donation từ >1 SC"));
                        }
                    }
                }
                other => return Err(format!("SC {sc}: donation nửa vời {other:?}")),
            }
        }
        // Chiều ngược lại: `Tcb.donated_sc` phải khớp `SchedContext.donated_to`, và donee
        // **không** được có SC riêng (một SC một TCB tại một thời điểm).
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let Some(sc) = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?
                .donated_sc
            else {
                continue;
            };
            let (donated_to, _) = self
                .donation_of(sc)
                .map_err(|e| format!("TCB {obj}: donated_sc {sc} không đọc được: {e:?}"))?;
            if donated_to != Some(obj) {
                return Err(format!(
                    "TCB {obj} giữ donated_sc {sc} nhưng SC không trỏ lại (donated_to={donated_to:?})"
                ));
            }
            if self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?
                .sched_context
                .is_some()
            {
                return Err(format!("donee {obj} vừa có SC riêng vừa nhận donation"));
            }
        }
        // KT-20: bất biến ngân sách của **mọi** `SchedContext` sống — `consumed ≤ budget`,
        // `budget`/`period` dương khi đã cấu hình, `refill` ở tương lai; `Sleeping{Budget}`
        // không nằm ready queue đã được nhánh "trạng thái khác" ở trên kiểm.
        for sc in scs {
            let budget = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.budget),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SchedContext {sc} không đọc được: {e:?}"))?;
            budget
                .check(self.now)
                .map_err(|e| format!("SchedContext {sc}: {e}"))?;
        }

        // KT-23 mục 2: tham chiếu SC của TCB phải trỏ tới object **sống** đúng kiểu
        // `SchedContext` (không chỉ kiểm donation — bắt cả SC bị Revoke mà TCB còn giữ).
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?;
            if let Some(sc) = info.sched_context {
                self.check_sc_alive(obj, sc, "sched_context")?;
            }
            if let Some(sc) = info.donated_sc {
                self.check_sc_alive(obj, sc, "donated_sc")?;
            }
        }
        // KT-23 mục 3: `SchedContext.bound` (Reply/Notification) còn sống, đúng kiểu.
        for sc in self.live_sched_contexts() {
            let bound = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.bound),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SchedContext {sc} không đọc được: {e:?}"))?;
            let Some(cap) = bound else { continue };
            let c = self
                .cap(cap)
                .map_err(|e| format!("SchedContext {sc}: bound cap {cap} không đọc được: {e:?}"))?;
            if !c.alive {
                return Err(format!("SchedContext {sc}: bound cap {cap} đã chết"));
            }
            if !matches!(c.ty, ObjType::Reply | ObjType::Notification) {
                return Err(format!(
                    "SchedContext {sc}: bound cap {cap} kiểu {:?}, phải Reply/Notification",
                    c.ty
                ));
            }
        }
        Ok(())
    }

    /// Kiểm một tham chiếu `SchedContext` của TCB trỏ tới object sống đúng kiểu.
    fn check_sc_alive(&self, tcb: ObjId, sc: ObjId, truong: &str) -> Result<(), String> {
        let meta = self
            .objects
            .get(sc as usize - 1)
            .ok_or_else(|| format!("TCB {tcb}: {truong} trỏ tới {sc} ngoài bảng object"))?;
        if !meta.alive {
            return Err(format!("TCB {tcb}: {truong} trỏ tới {sc} đã chết"));
        }
        if meta.ty != ObjType::SchedContext {
            return Err(format!(
                "TCB {tcb}: {truong} trỏ tới {sc} kiểu {:?}, phải SchedContext",
                meta.ty
            ));
        }
        Ok(())
    }

    /// **KT-23** — miền bất biến **ngân sách** (KT-20, giữ nguyên ngữ nghĩa):
    ///
    /// 1. `Running` ⇒ SC cấp ngân sách **đã `Configure` và còn budget** (bất biến "TCB chỉ chạy
    ///    khi SC còn budget" — mọi đường rời `Running` hết budget đi qua `Sleeping{Budget}`);
    /// 2. mọi `SchedContext` sống: `consumed ≤ budget`, `budget`/`period` dương khi đã cấu hình,
    ///    `refill` ở tương lai ([`ScBudget::check`]).
    pub fn check_budget(&self) -> Result<(), String> {
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?;
            if matches!(info.state, TcbState::Running { .. }) && !self.tcb_has_budget(obj) {
                return Err(format!(
                    "TCB {obj} Running nhưng SchedContext đã cạn budget (consumed == budget)"
                ));
            }
        }
        // KT-20: bất biến ngân sách của **mọi** `SchedContext` sống — `consumed ≤ budget`,
        // `budget`/`period` dương khi đã cấu hình, `refill` ở tương lai.
        for sc in self.live_sched_contexts() {
            let budget = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.budget),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SchedContext {sc} không đọc được: {e:?}"))?;
            budget
                .check(self.now)
                .map_err(|e| format!("SchedContext {sc}: {e}"))?;
        }
        Ok(())
    }

    // ------------------------------------------- tổng kết K1b cho test/harness (KT-23) `[TK]`
    //
    // **Model-only, không phải ABI**: `k1b_report` là ảnh chụp bộ đếm của mô hình để KT-23
    // in `transitions hit/allowed`, `forbidden_attempts`, `budget_violations` (K1B-PLAN §5
    // KT-23 DoD 2/3) — **không** thêm method ABI nào, không đổi ngữ nghĩa.

    /// Bất biến K1b tại **một** thời điểm, gộp cả 5 miền — trả lỗi **đầu tiên** kèm tên miền
    /// (test KT-23 gọi sau **mỗi** bước; `sched_after_op` vẫn kiểm sau mỗi public op).
    pub fn k1b_audit(&self) -> Result<(), String> {
        if let Err(e) = self.check_sched() {
            return Err(format!("sched: {e}"));
        }
        if let Err(e) = self.check_sc_refs() {
            return Err(format!("sc_refs: {e}"));
        }
        if let Err(e) = self.check_budget() {
            return Err(format!("budget: {e}"));
        }
        if let Err(e) = self.check_mailboxes() {
            return Err(format!("mailbox: {e}"));
        }
        self.check_all_bytes().map_err(|e| format!("bytes: {e}"))?;
        Ok(())
    }

    /// Dựng [`K1bReport`] từ bộ đếm của mô hình (KT-23 DoD 2/3). Chạy các `check_*` để điền
    /// `*_violations` (0 = sạch tại thời điểm gọi).
    pub fn k1b_report(&self) -> K1bReport {
        let (hit, allowed) = crate::tcb::coverage_design(&self.pair_hits);
        let (hit_table, table_allowed) = crate::tcb::coverage(&self.pair_hits);
        let mut forbidden_attempts = 0u64;
        let mut forbidden_pairs_attempted = 0usize;
        for from in 0..TCB_STATE_COUNT {
            for to in 0..TCB_STATE_COUNT {
                if crate::tcb::ALLOWED[from][to] {
                    continue;
                }
                let n = self.forbidden_hits[from * TCB_STATE_COUNT + to] as u64;
                if n > 0 {
                    forbidden_attempts += n;
                    forbidden_pairs_attempted += 1;
                }
            }
        }
        let mut q = K1bQueueReport {
            live_tcbs: 0,
            ready: 0,
            running: 0,
            blocked: 0,
            sleeping: 0,
            inactive: 0,
            dead: 0,
            queued: 0,
            current: self.sched[0].current(),
            idle: self.sched[0].idle(),
        };
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let Ok(info) = self.tcb_info_obj(obj) else {
                continue;
            };
            q.live_tcbs += 1;
            match info.state {
                TcbState::Ready => q.ready += 1,
                TcbState::Running { .. } => q.running += 1,
                TcbState::Sleeping { .. } => q.sleeping += 1,
                TcbState::Inactive => q.inactive += 1,
                TcbState::Dead => q.dead += 1,
                other => {
                    if other.is_blocked() {
                        q.blocked += 1;
                    }
                }
            }
        }
        q.queued = self.sched[0].len();
        let mut b = K1bBudgetReport {
            ticks: self.now,
            budget_sleeps: self.budget_sleeps,
            refills: self.budget_refills,
            live_sched_contexts: 0,
            configured: 0,
            charged: 0,
            consumed_now: 0,
        };
        for sc in self.live_sched_contexts() {
            let Ok(budget) = self.body(sc, |k| match k {
                Kind::SchedContext(s) => Ok(s.budget),
                _ => Err(KError::WrongObjectType),
            }) else {
                continue;
            };
            b.live_sched_contexts += 1;
            if budget.is_configured() {
                b.configured += 1;
            }
            b.charged = b.charged.saturating_add(budget.charged());
            b.consumed_now = b.consumed_now.saturating_add(budget.consumed());
        }
        K1bReport {
            transition: K1bTransitionReport {
                hit,
                allowed,
                hit_table,
                table_allowed,
                reserved: crate::tcb::reserved_count(),
                forbidden_attempts,
                forbidden_pairs_attempted,
                forbidden_pairs_total: crate::tcb::forbidden_count(),
            },
            queue: q,
            budget: b,
            sched_violations: u64::from(self.check_sched().is_err()),
            sc_ref_violations: u64::from(self.check_sc_refs().is_err()),
            budget_violations: u64::from(self.check_budget().is_err()),
            mailbox_violations: u64::from(self.check_mailboxes().is_err()),
            byte_violations: u64::from(self.check_all_bytes().is_err()),
        }
    }

    // ------------------------------------------- ngân sách MCS (KT-20) `[TK]`
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-49(b,e,h)): `SchedContext` có
    // `budget`/`period`/`consumed`/`refill`; thời gian là đồng hồ ảo `tick(n)`; TCB chỉ chạy
    // khi SC còn budget, hết budget ⇒ `Sleeping{reason = Budget}` qua `transition_tcb`, biên
    // refill ⇒ `Ready` nếu `!suspended`. **Không** làm refill sporadic/timeout fault (D2-16).
    // **Donation** (`Call`/`Reply`, passive server, MCP) là phần KT-21 ở mục riêng ngay dưới:
    // charge/refill vẫn tính trên cùng một `SchedContext`, nên ngân sách của client bị trừ đúng
    // bằng thời gian server phục vụ (D2-49(c)).

    /// Thời gian ảo hiện tại (đơn vị tick) — **không** đọc `Instant`/wall clock (D2-49(h)).
    pub fn virtual_now(&self) -> u64 {
        self.now
    }

    /// `tick(n)` — chạy đồng hồ ảo `n` đơn vị; tất định, cùng kịch bản ⇒ cùng mốc refill.
    ///
    /// Mỗi đơn vị: charge TCB đang `Running` cho `[now, now+1)`, `now += 1`, rồi xử lý biên
    /// refill tại `now` mới (charge trước — refill sau, xem `mcs`). **Không** tự `dispatch`:
    /// chọn TCB vẫn là thao tác tường minh như KT-19.
    pub fn tick(&mut self, n: u64) -> Result<(), KError> {
        for _ in 0..n {
            self.tick_one()?;
        }
        self.sched_after_op(false);
        Ok(())
    }

    fn tick_one(&mut self) -> Result<(), KError> {
        // 1) Charge TCB đang chạy của từng hart (K1b: 1 hart — D2-49(a)); hết budget ⇒
        //    `Running → Sleeping{Budget}` ngay, gỡ `current` qua `sched_sync`.
        for h in 0..self.sched.len() {
            if let Some(cur) = self.sched[h].current() {
                self.charge_tick(cur)?;
            }
        }
        // 2) Thời gian ảo tiến 1 (bão hoà ở trần `u64`, không wrap).
        self.now = self.now.saturating_add(1);
        // 3) Biên refill tại `now` mới: `Sleeping{Budget} + !suspended → Ready`.
        self.process_refills()
    }

    /// Charge **một** tick cho TCB đang chạy; hết budget ⇒ `Sleeping{Budget}` (một cửa
    /// [`Machine::transition_tcb`]). TCB không SC cấp ngân sách/SC chưa cấu hình không bị charge.
    /// **KT-21**: SC cấp ngân sách có thể là SC **được donate** — charge vào SC của client.
    fn charge_tick(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        if !matches!(info.state, TcbState::Running { .. }) {
            return Ok(());
        }
        // KT-21: SC cấp ngân sách = SC **riêng** (không bị donate) hoặc SC **được donate** cho
        // TCB (server passive). Charge vào chính SC đó ⇒ thời gian server chạy trừ ngân sách của
        // client (D2-49(c)); SC của client có thể đã cạn ⇒ server `Sleeping{Budget}`.
        let Some(sc) = self.funding_sc(tcb) else {
            // Không SC = không có budget ⇒ không chạy (chỉ tới `Running` bằng transition trực
            // tiếp; `pick` đã chặn từ đầu, D2-50).
            self.transition_tcb(
                tcb,
                TcbState::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        };
        // SC chưa `SchedControl.Configure` = **không có budget** ⇒ không charge; nếu TCB đang
        // `Running` (chỉ tới được bằng `transition_tcb` trực tiếp, không qua `pick`), đưa về
        // `Sleeping{Budget}` để giữ "không có budget ⇒ không chạy".
        let configured = self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.is_configured()),
            _ => Err(KError::WrongObjectType),
        })?;
        if !configured {
            self.transition_tcb(
                tcb,
                TcbState::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        }
        let charged = self.body_mut(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.charge()),
            _ => Err(KError::WrongObjectType),
        })?;
        let can = self.tcb_has_budget(tcb);
        if !charged || !can {
            self.transition_tcb(
                tcb,
                TcbState::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }

    /// Xử lý biên nạp lại tại `now` cho mọi `SchedContext` sống: `consumed = 0`, rồi đánh
    /// thức TCB `Sleeping{Budget}` **không** `suspended` đang gắn SC đó.
    fn process_refills(&mut self) -> Result<(), KError> {
        let now = self.now;
        let scs: Vec<ObjId> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::SchedContext)
            .map(|(i, _)| (i as u32) + 1)
            .collect();
        let mut refilled: Vec<ObjId> = Vec::new();
        for sc in scs {
            let did = self.body_mut(sc, |k| match k {
                Kind::SchedContext(s) => Ok(s.budget.refill_due(now)),
                _ => Err(KError::WrongObjectType),
            })?;
            if did {
                // KT-23: đếm lần nạp lại (một SC một lần/chu kỳ) cho `k1b_report`.
                self.budget_refills += 1;
                refilled.push(sc);
            }
        }
        if refilled.is_empty() {
            return Ok(());
        }
        let tcbs: Vec<ObjId> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::Tcb)
            .map(|(i, _)| (i as u32) + 1)
            .collect();
        let mut wake: Vec<ObjId> = Vec::new();
        for tcb in tcbs {
            let info = self.tcb_info_obj(tcb)?;
            if matches!(
                info.state,
                TcbState::Sleeping {
                    reason: SleepReason::Budget
                }
            ) && !info.suspended
            {
                // KT-21: SC cấp ngân sách có thể là SC riêng **hoặc** SC được donate — cả hai
                // đều phải được đánh thức ở biên refill (DoD 4: server ngủ, refill, Reply).
                if let Some(sc) = self.funding_sc(tcb) {
                    if refilled.contains(&sc) {
                        wake.push(tcb);
                    }
                }
            }
        }
        for tcb in wake {
            self.transition_tcb(tcb, TcbState::Ready)?;
        }
        Ok(())
    }

    /// Gắn/gỡ SC khiến TCB đang `Running` **không còn budget** (SC đã cạn, SC chưa cấu hình,
    /// hoặc gỡ SC) ⇒ ngủ ngay `Sleeping{Budget}` (giữ bất biến "Running ⇒ còn budget", và
    /// "không SC/SC chưa cấu hình ⇒ không chạy" — reviewer-kt20 mục 1). TCB `Ready` không đổi
    /// trạng thái (bảng ALLOWED cấm `Ready → Sleeping`) — `pick` bỏ qua nó cho tới khi có SC
    /// đã cấu hình còn budget.
    fn enforce_budget(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        if matches!(info.state, TcbState::Running { .. }) && !self.tcb_has_budget(tcb) {
            self.transition_tcb(
                tcb,
                TcbState::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }

    /// Ảnh chụp ngân sách của `SchedContext` — model API `[TK]` (D2-49(b)).
    pub fn sched_info(&self, sched: SlotRef) -> Result<ScInfo, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        self.body(obj, |k| match k {
            Kind::SchedContext(s) => {
                let mut info = s.budget.info();
                info.donated_to = s.donated_to;
                info.donor = s.donor;
                Ok(info)
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `SchedContext.Consumed` `[TK]`: số tick đã charge trong chu kỳ hiện tại.
    pub fn sched_consumed(&self, sched: SlotRef) -> Result<u64, KError> {
        Ok(self.sched_info(sched)?.consumed)
    }

    /// Budget còn lại của chu kỳ hiện tại; `0` khi SC chưa cấu hình (không có budget).
    pub fn sched_remaining(&self, sched: SlotRef) -> Result<u64, KError> {
        let info = self.sched_info(sched)?;
        Ok(if info.configured {
            info.budget - info.consumed
        } else {
            0
        })
    }

    /// TCB có được phép chạy theo ngân sách không: **phải có** SC cấp ngân sách **đã**
    /// `SchedControl.Configure` và `consumed < budget`. SC cấp ngân sách là SC **riêng**
    /// (không bị donate) hoặc SC **được donate** cho TCB (KT-21, D2-49(c)); không SC / SC chưa
    /// cấu hình ⇒ `false` — TCB không được `pick` (ngữ nghĩa §4.5, K1B-PLAN §5 KT-20 DoD 4,
    /// D2-50: không SC **không phải** "không giới hạn"). Dùng bởi `pick`/`dispatch`,
    /// `enforce_budget`, `charge_tick` và test bất biến.
    pub fn tcb_has_budget(&self, tcb: ObjId) -> bool {
        let Some(sc) = self.funding_sc(tcb) else {
            return false;
        };
        self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.has_budget()),
            _ => Err(KError::WrongObjectType),
        })
        .unwrap_or(false)
    }

    // ------------------------------------------- MCS donation (KT-21) `[TK]`
    //
    // Nguồn: **D2-49(c)** ("donation **tự động** qua Call/Reply; server có SC riêng giữ SC
    // riêng; các method chưa có chữ ký chỉ cài model API"), **D2-50** ("TCB không SC **không
    // chạy**, trừ khi được donation"), thiết kế §4.5 (`:288-311`), K1B-PLAN §5 KT-21.
    // Model-only `[TK]` — **không** mở chữ ký ABI nào.
    //
    // Liên kết **hai chiều** để tra O(1) và kiểm bất biến:
    //   `SchedData { donated_to: Option<donee>, donor: Option<donor> }`
    //   `TcbData  { donated_sc: Option<sc> }`
    // Bất biến (kiểm trong [`Machine::check_sched`] sau **mỗi** public op): hai chiều khớp;
    // donee **không** có SC riêng; donor đang `BlockedOnReply`; donor ≠ donee; mỗi SC một donee
    // và mỗi donee một SC; donor/donee **không** cùng `Running`; priority hiệu lực không vượt
    // MCP của một trong hai bên.
    //
    // **Không chuỗi >1** (K1B-PLAN §5 KT-21 rủi ro (a)): chỉ TCB có `sched_context` **và**
    // không phải donee mới donate được. K1b cũng **không** mô hình hoá một server phục vụ nhiều
    // client cùng lúc (server chỉ nhận donation khi đang không có donation khác).

    /// `(donated_to, donor)` của một `SchedContext` — `(None, None)` khi SC không bị donate.
    fn donation_of(&self, sc: ObjId) -> Result<(Option<ObjId>, Option<ObjId>), KError> {
        self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok((s.donated_to, s.donor)),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Danh sách `SchedContext` còn sống (mô hình nhỏ; dùng cho donation/bất biến).
    fn live_sched_contexts(&self) -> Vec<ObjId> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::SchedContext)
            .map(|(i, _)| (i as u32) + 1)
            .collect()
    }

    /// SC đang **cấp ngân sách** cho `tcb`: SC riêng (khi **không** bị donate) hoặc SC được
    /// donate cho TCB. `None` ⇔ không có ngân sách ⇒ không được `pick` (D2-50).
    fn funding_sc(&self, tcb: ObjId) -> Option<ObjId> {
        let info = self.tcb_info_obj(tcb).ok()?;
        if let Some(sc) = info.donated_sc {
            return Some(sc);
        }
        let own = info.sched_context?;
        match self.donation_of(own) {
            Ok((None, _)) => Some(own),
            // SC riêng đang cho người khác mượn (hoặc SC chết) ⇒ chủ sở hữu **không** có budget.
            _ => None,
        }
    }

    /// `tcb` có phải **donor** của một donation đang hiệu lực không.
    fn is_donation_donor(&self, tcb: ObjId) -> bool {
        self.live_sched_contexts()
            .into_iter()
            .any(|sc| matches!(self.donation_of(sc), Ok((_, Some(o))) if o == tcb))
    }

    /// Model API `[TK]` (K1B-PLAN §5 KT-21 #1): SC đang cấp ngân sách cho `tcb` — SC được
    /// donate nếu `tcb` là donee, SC riêng nếu không bị donate, `None` nếu không có.
    pub fn sc_of(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        let info = self.tcb_info_obj(tcb)?;
        if let Some(sc) = info.donated_sc {
            return Ok(Some(sc));
        }
        let Some(own) = info.sched_context else {
            return Ok(None);
        };
        Ok(match self.donation_of(own)? {
            (None, _) => Some(own),
            _ => None,
        })
    }

    /// Model API `[TK]`: SC **được donate cho** `tcb` (`None` nếu không phải donee).
    pub fn donated_sc(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.tcb_info_obj(tcb)?.donated_sc)
    }

    /// `SchedContext.DonatedTo` `[TK]`: TCB đang nhận donation của `sched` (`None` nếu không).
    pub fn sched_donated_to(&self, sched: SlotRef) -> Result<Option<ObjId>, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        Ok(self.donation_of(obj)?.0)
    }

    /// `SchedContext.Donor` `[TK]`: TCB đang cho mượn `sched` (`None` nếu không).
    pub fn sched_donor_of(&self, sched: SlotRef) -> Result<Option<ObjId>, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        Ok(self.donation_of(obj)?.1)
    }

    /// Priority **hiệu lực** của `tcb` (KT-21 DoD 6): khi nhận donation =
    /// [`crate::mcs::donated_priority`] (kẹp theo MCP của donor và donee); ngược lại priority
    /// gốc. Priority gốc **không** bị đổi — chỉ bucket ready queue/`cur_prio` dùng giá trị này.
    pub fn effective_priority(&self, tcb: ObjId) -> Result<u8, KError> {
        let info = self.tcb_info_obj(tcb)?;
        let Some(sc) = info.donated_sc else {
            return Ok(info.priority);
        };
        let (_, donor) = self.donation_of(sc)?;
        let Some(donor) = donor else {
            return Ok(info.priority);
        };
        let oi = self.tcb_info_obj(donor)?;
        Ok(crate::mcs::donated_priority(
            oi.priority,
            oi.max_controlled_priority,
            info.max_controlled_priority,
        ))
    }

    /// Đặt donation tự động `donor → donee` (KT-21 DoD 1). Trả `false` (**không đổi gì**) khi
    /// không đủ điều kiện — quan trọng: `Call` vẫn thành công, chỉ là server không có ngân sách
    /// nên không chạy (D2-50); **không** từ chối IPC vì thiếu SC (giữ KT-20 DoD 4).
    ///
    /// Điều kiện: donor có SC riêng **đã** `SchedControl.Configure`, donor **không** đang là
    /// donee (không chuỗi >1), donee **không** có SC riêng và **không** đang nhận donation khác
    /// (D2-49(c): "server có SC riêng giữ SC riêng"), SC chưa bị donate.
    fn donate_sc(&mut self, donor: ObjId, donee: ObjId) -> Result<bool, KError> {
        if donor == donee {
            return Ok(false);
        }
        let dinfo = self.tcb_info_obj(donor)?;
        if dinfo.donated_sc.is_some() {
            return Ok(false);
        }
        let Some(sc) = dinfo.sched_context else {
            return Ok(false);
        };
        let configured = self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.is_configured()),
            _ => Err(KError::WrongObjectType),
        })?;
        if !configured {
            return Ok(false);
        }
        // KT-21 **siết** "một SC một TCB tại một thời điểm" cho donation: mô hình KT-20 chưa
        // cấm hai TCB chia một SC; nếu SC đang gắn cho TCB khác thì **không donate** (nếu vẫn
        // donate, TCB kia đang `Running` sẽ mất ngân sách giữa chừng — vỡ bất biến "Running ⇒
        // còn budget"). SC dùng chung vẫn chạy bình thường theo KT-20, chỉ không donate.
        let mut chu_so_huu = 0usize;
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            if let Ok(info) = self.tcb_info_obj((idx as u32) + 1) {
                if info.sched_context == Some(sc) {
                    chu_so_huu += 1;
                }
            }
        }
        if chu_so_huu != 1 {
            return Ok(false);
        }
        let donee_info = self.tcb_info_obj(donee)?;
        if donee_info.sched_context.is_some() || donee_info.donated_sc.is_some() {
            return Ok(false);
        }
        if self.donation_of(sc)?.0.is_some() {
            return Ok(false);
        }
        // Liên kết hai chiều **trước** khi sync bucket (priority hiệu lực đọc từ đây).
        self.body_mut(sc, |k| match k {
            Kind::SchedContext(s) => {
                s.donated_to = Some(donee);
                s.donor = Some(donor);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.body_mut(donee, |k| match k {
            Kind::Tcb(t) => {
                t.donated_sc = Some(sc);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // Donee đang `Ready` ⇒ chuyển sang bucket priority hiệu lực (một SC một TCB một lúc).
        self.sched_remove_all(donee);
        self.sched_sync(donee)?;
        Ok(true)
    }

    /// Trả SC donation của `donee` về donor (KT-21 DoD 2). Gỡ bucket **cũ** (priority donate)
    /// trước khi đổi priority hiệu lực, rồi sync lại theo priority gốc. Trả SC đã trả (nếu có).
    fn return_donation(&mut self, donee: ObjId) -> Result<Option<ObjId>, KError> {
        let Some(sc) = self.tcb_info_obj(donee)?.donated_sc else {
            return Ok(None);
        };
        self.sched_remove_all(donee);
        self.body_mut(donee, |k| match k {
            Kind::Tcb(t) => {
                t.donated_sc = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.body_mut(sc, |k| match k {
            Kind::SchedContext(s) => {
                s.donated_to = None;
                s.donor = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(donee)?;
        Ok(Some(sc))
    }

    /// Donor của SC đang donate cho `donee` (`None` nếu `donee` không nhận donation).
    fn donation_donor_of(&self, donee: ObjId) -> Option<ObjId> {
        let sc = self.tcb_info_obj(donee).ok()?.donated_sc?;
        self.donation_of(sc).ok()?.1
    }

    /// Huỷ reply đang chờ của `donor`: gỡ `Reply.caller` rồi đánh thức về `Ready`.
    ///
    /// Model-only `[TK]` (KT-21 DoD 5): server chết/treo khi đang được donate thì reply **không
    /// bao giờ** tới, nên client không được treo vĩnh viễn. Mã kết quả `Cancelled` đầy đủ (mang
    /// giá trị trả về của `Call`) thuộc K1c — K1b chỉ mô hình hoá việc đánh thức.
    fn cancel_reply_of(&mut self, donor: ObjId) -> Result<(), KError> {
        if let TcbState::BlockedOnReply { reply } = self.tcb_state(donor)? {
            self.reply_set_caller(reply, None)?;
            self.transition_tcb(donor, TcbState::Ready)?;
        }
        Ok(())
    }

    /// **KT-25** — `Reply` **bị xoá/revoke** khi caller đang `BlockedOnReply` (D2-53 **Q8**):
    /// lấy `Reply.caller`, đặt lỗi IPC `Cancelled` cho caller và đánh thức caller `Ready`.
    /// Không deadlock, `Reply.caller == None` sau khi huỷ. Đi qua **một** cửa
    /// [`Machine::transition_tcb`] ⇒ hook KT-21 tự trả `SchedContext` donation về donor (không
    /// rò SC). Trả `Some(caller)` nếu đã đánh thức ai.
    fn cancel_reply(&mut self, reply_obj: ObjId) -> Result<Option<ObjId>, KError> {
        let caller = self.body_mut(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller.take()),
            _ => Err(KError::WrongObjectType),
        })?;
        if let Some(c) = caller {
            self.set_last_ipc_error(c, KError::Cancelled)?;
            if matches!(
                self.tcb_state(c)?,
                TcbState::BlockedOnReply { reply } if reply == reply_obj
            ) {
                self.transition_tcb(c, TcbState::Ready)?;
            }
        }
        Ok(caller)
    }

    /// **KT-25** — một **cap Reply** vừa bị `Delete`/`Revoke` ⇒ coi như đường reply bị xoá:
    /// caller đang chờ được đánh thức `Cancelled` (D2-53 Q8, `.scn` 07/08).
    ///
    /// Model-only `[TK]`: seL4 chỉ mất object khi Untyped bị revoke, nhưng hai kịch bản `.scn`
    /// của K1c xoá **cap** Reply (`CNode.Delete`/`CNode.Revoke`) rồi kỳ vọng caller không treo;
    /// Q8 chốt caller `Cancelled`. Object **không** bị finalise (chỉ huỷ **đường reply**), nên
    /// gọi nhiều lần là idempotent (`caller = None` sau lần đầu).
    fn cancel_reply_of_cap(&mut self, obj: ObjId) -> Result<(), KError> {
        if self.object_alive(obj) && self.object_type(obj) == Some(ObjType::Reply) {
            self.cancel_reply(obj)?;
        }
        Ok(())
    }

    /// **KT-25** — gỡ **Reply cap đã chết** khỏi Recv waiter của mọi Endpoint: waiter vẫn chờ
    /// nhưng coi như **không cấp Reply** ⇒ `Call` tới đi nhánh Q7(b) (message vẫn giao + caller
    /// `Cancelled`) thay vì lỗi cứng `InvalidCapability` (`.scn` 07: `Delete(rep0)` sau `Recv`).
    fn clear_dead_reply_in_waiters(&mut self) {
        let alive: Vec<bool> = self.caps.iter().map(|c| c.alive).collect();
        let cap_alive = |cap: CapId| -> bool {
            cap > 0 && (cap as usize) <= alive.len() && alive[cap as usize - 1]
        };
        let eps: Vec<ObjId> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::Endpoint)
            .map(|(i, _)| (i as u32) + 1)
            .collect();
        for ep in eps {
            let _ = self.body_mut(ep, |k| {
                if let Kind::Endpoint(e) = k {
                    if let Some((tcb, Some(cap))) = e.recv {
                        if !cap_alive(cap) {
                            e.recv = Some((tcb, None));
                        }
                    }
                }
                Ok::<(), KError>(())
            });
        }
    }

    /// Huỷ donation khi `tcb` là **donee** hoặc **donor** (suspend/exit/finalise — DoD 5).
    ///
    /// - `tcb` là donee ⇒ trả SC về donor + đánh thức donor (reply sẽ không tới).
    /// - `tcb` là donor ⇒ thu hồi SC khỏi donee; donee đang `Running` mất ngân sách ⇒
    ///   `Sleeping{Budget}` ngay (D2-50), `Ready` thì ở lại hàng và bị `pick` bỏ qua.
    ///
    /// Không rò SC: sau hàm này không còn `donated_to`/`donor`/`donated_sc` nào trỏ tới `tcb`.
    fn release_donation_of(&mut self, tcb: ObjId) -> Result<(), KError> {
        // (1) `tcb` là donee.
        if let Some(sc) = self.tcb_info_obj(tcb)?.donated_sc {
            let (_, donor) = self.donation_of(sc)?;
            self.return_donation(tcb)?;
            if let Some(donor) = donor {
                self.cancel_reply_of(donor)?;
            }
        }
        // (2) `tcb` là donor (SC của nó đang cho mượn).
        for sc in self.live_sched_contexts() {
            let (donated_to, donor) = self.donation_of(sc)?;
            if donor != Some(tcb) {
                continue;
            }
            let Some(donee) = donated_to else {
                continue;
            };
            self.sched_remove_all(donee);
            self.body_mut(donee, |k| match k {
                Kind::Tcb(t) => {
                    t.donated_sc = None;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            self.body_mut(sc, |k| match k {
                Kind::SchedContext(s) => {
                    s.donated_to = None;
                    s.donor = None;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            if matches!(self.tcb_state(donee)?, TcbState::Running { .. }) {
                self.transition_tcb(
                    donee,
                    TcbState::Sleeping {
                        reason: SleepReason::Budget,
                    },
                )?;
            } else {
                self.sched_sync(donee)?;
            }
        }
        Ok(())
    }

    // ------------------------------------------- hàng chờ phụ của mô hình K1b `[TK]`

    /// `Wait` trên Notification — **mô hình K1b** `[TK]` (D2-49(a): tín hiệu/word là K1c):
    /// đặt TCB vào `BlockedOnNotification` và ghi **một** waiter trên object Notification.
    /// Notification đã có waiter ⇒ `Busy` (thiết kế §4.7: `Signal` đánh thức một người chờ).
    pub fn tcb_wait_notification(&mut self, tcb: SlotRef, ntfn: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        self.wait_notification_obj(tcb_obj, ntfn_obj)
    }

    /// Thân của `Wait` trên Notification theo `ObjId` — dùng chung cho `tcb_wait_notification`
    /// (model API K1b) và **`NBSendWait`** của KT-25.
    fn wait_notification_obj(&mut self, tcb_obj: ObjId, ntfn_obj: ObjId) -> Result<(), KError> {
        let state = self.ensure_started(tcb_obj)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let busy = self.body(ntfn_obj, |k| match k {
            Kind::Notification(n) => Ok(n.waiter.is_some()),
            _ => Err(KError::WrongObjectType),
        })?;
        if busy {
            return Err(KError::Busy);
        }
        self.body_mut(ntfn_obj, |k| match k {
            Kind::Notification(n) => {
                n.waiter = Some(tcb_obj);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.transition_tcb(tcb_obj, TcbState::BlockedOnNotification { ntfn: ntfn_obj })?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `WaitAddr` — **mô hình K1b** `[TK]` (thiết kế §4.7; object Frame thuộc K2): xếp TCB vào
    /// bucket `(frame, offset)` rồi `BlockedOnAddress`. Khoá theo địa chỉ vật lý là K2/K1c.
    pub fn tcb_wait_address(
        &mut self,
        tcb: SlotRef,
        frame: ObjId,
        offset: u32,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let state = self.ensure_started(tcb_obj)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.address_waiters.push((frame, offset, tcb_obj));
        self.transition_tcb(tcb_obj, TcbState::BlockedOnAddress { frame, offset })?;
        self.sched_after_op(true);
        Ok(())
    }

    // ------------------------------------------- đọc hàng chờ (test/đối chứng)

    pub fn endpoint_send_len(&self, ep: ObjId) -> Result<usize, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.len()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// KT-22 — ảnh chụp hộp thư (mailbox) của Endpoint: sức chứa, số đang chờ, bộ đếm
    /// `pushed/popped/removed` (bất biến `pushed == popped + removed + len`) + Recv waiter.
    pub fn endpoint_info(&self, ep: ObjId) -> Result<EndpointInfo, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(EndpointInfo {
                capacity: e.send.capacity(),
                len: e.send.len(),
                pushed: e.send.pushed(),
                popped: e.send.popped(),
                removed: e.send.removed(),
                recv_waiter: e.recv.map(|(tcb, _)| tcb),
            }),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Hàng gửi của Endpoint theo đúng **thứ tự FIFO** — API K1b (KT-22) giữ nguyên: trả
    /// `(sender, fault)` để `kt22_mailbox`/`kt23_k1b` không phải đổi. Nội dung message đọc qua
    /// [`Machine::endpoint_msg_ids`] (KT-24).
    pub fn endpoint_send_items(
        &self,
        ep: ObjId,
    ) -> Result<Vec<(ObjId, Option<FaultKind>)>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.iter().map(|q| (q.sender, q.fault)).collect()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-24** — `msg_id` của các message **người dùng** trong hàng gửi, đúng thứ tự FIFO
    /// (message fault `Exit` không có `msg_id` nên **không** xuất hiện).
    pub fn endpoint_msg_ids(&self, ep: ObjId) -> Result<Vec<u64>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e
                .send
                .iter()
                .filter_map(|q| q.msg.as_ref().map(|m| m.msg_id))
                .collect()),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn endpoint_recv_waiter(&self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv.map(|(tcb, _)| tcb)),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn endpoint_last_mr(&self, ep: ObjId) -> Result<Option<u64>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.last_mr),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn reply_caller(&self, reply: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(reply, |k| match k {
            Kind::Reply(r) => Ok(r.caller),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn notification_waiter(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok(n.waiter),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn address_waiter(&self, frame: ObjId, offset: u32) -> Option<ObjId> {
        self.address_waiters
            .iter()
            .find(|(f, o, _)| *f == frame && *o == offset)
            .map(|(_, _, tcb)| *tcb)
    }

    pub fn address_waiters_len(&self) -> usize {
        self.address_waiters.len()
    }

    // ---------------------------------------------------------------- IPC (KT-24)

    /// Resolve cap cho entry IPC: `SlotEmpty` quy về `InvalidCapability` đúng nghĩa ABI
    /// ("cap không hợp lệ" cho tham số syscall — `SlotEmpty` là mã của thao tác CNode, không
    /// phải của `Call`/`Send`; `.scn` 06 `call cap=c1 => InvalidCapability`).
    fn ipc_slot_cap(&self, slot: SlotRef) -> Result<CapId, KError> {
        self.cap_at(slot.cnode, slot.slot).map_err(|e| match e {
            KError::SlotEmpty => KError::InvalidCapability,
            other => other,
        })
    }

    /// Endpoint đích của `Call`. Object **không** phải Endpoint = invocation chưa cài ⇒
    /// `NotSupported`, **không** chặn, **không** Reply, `TcbState` không đổi (D2-53 Q11).
    fn ipc_call_endpoint(&self, slot: SlotRef) -> Result<ObjId, KError> {
        let cap = self.ipc_slot_cap(slot)?;
        let c = self.cap(cap)?;
        if c.ty != ObjType::Endpoint {
            return Err(KError::NotSupported);
        }
        Ok(c.obj)
    }

    /// Loại object của cap cho `Send` (dispatch `Endpoint`/`Reply`/`Notification`).
    fn ipc_cap_type(&self, slot: SlotRef) -> Result<ObjType, KError> {
        let cap = self.ipc_slot_cap(slot)?;
        Ok(self.cap(cap)?.ty)
    }

    /// Peek waiter `Recv` của Endpoint: `(có waiter, Reply cap người nhận đã cấp)`.
    fn endpoint_recv_slot(&self, ep: ObjId) -> Result<(bool, Option<CapId>), KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok((e.recv.is_some(), e.recv.as_ref().and_then(|(_, cap)| *cap))),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// TCB đang chờ `Recv` trên Endpoint (chỉ **đọc**, không gỡ hàng) — KT-25 dùng để kiểm
    /// receive window của bên nhận **trước** khi message vào đường IPC.
    fn endpoint_recv_waiter_obj(&self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv.map(|(t, _)| t)),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Lấy (và gỡ) TCB đang chờ `Recv` trên Endpoint — **sau khi** đã peek/kiểm Reply cap.
    fn endpoint_take_waiter(&mut self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv.take().map(|(t, _)| t)),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Hàng gửi đầy? (chỉ kiểm khi **không** có waiter — có waiter thì giao thẳng, không dùng hàng.)
    fn endpoint_full(&self, ep: ObjId) -> Result<bool, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.is_full()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-25** — ảnh chụp cap nguồn của message **đầu** hàng gửi (`None` = hàng rỗng) —
    /// để kiểm receive window trước khi pop (không giao nửa chừng).
    fn endpoint_front_snaps(&self, ep: ObjId) -> Result<Option<Vec<ipc::CapSnapshot>>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.front().map(|q| q.snaps.clone())),
            _ => Err(KError::WrongObjectType),
        })
    }

    fn endpoint_pop(&mut self, ep: ObjId) -> Result<Option<QueuedMessage>, KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.pop_front()),
            _ => Err(KError::WrongObjectType),
        })
    }

    fn endpoint_enqueue(&mut self, ep: ObjId, q: QueuedMessage) -> Result<(), KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => e.send.try_push(q).map_err(|_| KError::Busy),
            _ => Err(KError::WrongObjectType),
        })
    }

    fn set_endpoint_last_mr(&mut self, ep: ObjId, mr: u64) -> Result<(), KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => {
                e.last_mr = Some(mr);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Nhận message vào đường IPC: kiểm biên cap (`≤ 4`, khớp `MessageInfo.caps`) và danh sách
    /// ảnh chụp cap đã resolve (KT-25) rồi **cấp `msg_id`** từ bộ đếm `ipc_sent` (không bao giờ
    /// tái dùng). Cap **không** bị bỏ im lặng: cap chết ⇒ `InvalidCapability` ở `resolve_caps`;
    /// thiếu `RIGHT_GRANT`/ô đích bận ⇒ **thất bại mềm** khi giao (`deliver_caps`, D2-53 Q6).
    fn admit_message(
        &mut self,
        mut msg: Message,
        snaps: Vec<ipc::CapSnapshot>,
    ) -> Result<Message, KError> {
        // **KT-25**: biên ABI `max_caps = 4` (không nới) + `info.caps` phải khớp danh sách.
        if msg.caps.len() > ipc::MAX_CAPS
            || msg.caps.len() != msg.info.caps() as usize
            || snaps.len() != msg.caps.len()
        {
            return Err(KError::InvalidArgument);
        }
        self.ipc_sent += 1;
        msg.msg_id = self.ipc_sent;
        Ok(msg)
    }

    /// **KT-25** — chụp ảnh cap nguồn của message: **lỗi cứng** `InvalidCapability` khi cap
    /// chết/không tồn tại (`cap = 0`/ngoài bảng), **trước** khi cấp `msg_id` (message không bị
    /// giao nửa chừng — KT-25 DoD 4).
    fn resolve_caps(&self, msg: &Message) -> Result<Vec<ipc::CapSnapshot>, KError> {
        let mut snaps = Vec::with_capacity(msg.caps.len());
        for cap in &msg.caps {
            let c = self.cap(*cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            snaps.push(ipc::CapSnapshot {
                cap: *cap,
                obj: c.obj,
                ty: c.ty,
                rights: c.rights,
                badge: c.badge,
            });
        }
        Ok(snaps)
    }

    /// **KT-25** — lập kế hoạch chuyển cap vào receive window của `receiver`.
    ///
    /// - message **không** cap ⇒ `None` (không cần window);
    /// - window **chưa bind** mà message có cap ⇒ `InvalidArgument` (lỗi cấu hình, **trước** khi
    ///   message được giao — khác "bind 4 ô rỗng" hợp lệ);
    /// - ngược lại ⇒ [`ipc::plan_cap_transfer`] với ô nào đang có cap (thất bại **mềm**, Q6).
    fn prepare_caps(
        &self,
        receiver: ObjId,
        snaps: &[ipc::CapSnapshot],
        endpoint: Option<ObjId>,
    ) -> Result<Option<ipc::CapPlan>, KError> {
        if snaps.is_empty() {
            return Ok(None);
        }
        let window = self.tcb_recv_window(receiver)?;
        let mut occupied = [false; ipc::MAX_CAPS];
        if let Some(window) = window.as_ref() {
            for (i, w) in window.iter().enumerate() {
                occupied[i] = self.cnode_slot(w.cnode, w.slot)?.is_some();
            }
        }
        Ok(Some(ipc::plan_cap_transfer(
            snaps,
            window.as_ref(),
            &occupied,
            endpoint,
        )?))
    }

    /// **KT-25** — cài cap theo kế hoạch vào receive window và cập nhật `MessageInfo` của
    /// message **nhận**: `caps` = số cap **cài được**, `extra_caps_unwrapped` = số cap unwrap,
    /// `badge` = OR các badge unwrap. Cap nguồn chết giữa lúc xếp hàng và lúc giao ⇒ thất bại
    /// **mềm** (không tạo cap mồ côi, không mất message — D2-53 Q6).
    fn deliver_caps(
        &mut self,
        msg: &mut Message,
        snaps: &[ipc::CapSnapshot],
        plan: Option<&ipc::CapPlan>,
    ) -> Result<(), KError> {
        if snaps.is_empty() {
            return Ok(());
        }
        let Some(plan) = plan else {
            return Err(KError::InvalidArgument);
        };
        let mut installed = 0u8;
        for (idx, dst) in &plan.install {
            let snap = &snaps[*idx];
            let alive = snap.cap > 0
                && self
                    .caps
                    .get(snap.cap as usize - 1)
                    .map(|c| c.alive)
                    .unwrap_or(false);
            if !alive {
                self.cap_transfer_failed += 1;
                continue;
            }
            // Cap nhận **mất** `RIGHT_GRANT` (thiết kế `:319`), giữ badge nguồn.
            let rights = snap.rights & !RIGHT_GRANT;
            let cap = self.push_cap(snap.obj, snap.ty, Some(snap.cap), rights, snap.badge);
            self.cnode_set_slot(dst.cnode, dst.slot, Some(cap))?;
            self.caps[cap as usize - 1].slot = Some(*dst);
            installed += 1;
        }
        self.cap_transfer_failed += plan.skipped as u64;
        self.cap_transfer_installed += installed as u64;
        self.cap_unwrapped += plan.unwrapped() as u64;
        msg.info = msg
            .info
            .with_caps(installed)?
            .with_extra_caps_unwrapped(plan.unwrapped() as u8)?;
        msg.badge |= plan.badge();
        // Cap đã cài vào window của bên nhận; message nhận **không** còn `CapId` nguồn.
        msg.caps.clear();
        Ok(())
    }

    /// Giao message vào vùng nhận của TCB qua **`crate::ipc::transfer`** (đường copy **duy nhất**);
    /// cộng `ipc_delivered` + `transfer_calls` (hai bộ đếm phải luôn bằng nhau).
    fn deliver_to(&mut self, receiver: ObjId, msg: &Message) -> Result<(), KError> {
        self.body_mut(receiver, |k| match k {
            Kind::Tcb(t) => {
                ipc::transfer(msg, &mut t.ipc);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.ipc_delivered += 1;
        self.transfer_calls += 1;
        Ok(())
    }

    /// **KT-25** — giao message: chuyển cap theo kế hoạch rồi copy message vào vùng nhận.
    fn deliver_message(
        &mut self,
        receiver: ObjId,
        msg: &mut Message,
        snaps: &[ipc::CapSnapshot],
        plan: Option<&ipc::CapPlan>,
    ) -> Result<(), KError> {
        self.deliver_caps(msg, snaps, plan)?;
        self.deliver_to(receiver, msg)
    }

    fn reset_ipc_error(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.last_ipc_error = None;
                // **KT-26**: fault vừa giao thuộc về lần nhận **trước**; một op IPC mới của
                // bên nhận bắt đầu ⇒ bản ghi "fault vừa nhận" hết hiệu lực (tránh procsrv
                // tiêu thụ nhầm fault cũ).
                t.last_fault = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Ghi `(sender, fault)` vừa giao cho **bên nhận** (procsrv) — model-only `[TK]`, KT-26.
    fn record_last_fault(
        &mut self,
        receiver: ObjId,
        sender: ObjId,
        kind: FaultKind,
    ) -> Result<(), KError> {
        self.body_mut(receiver, |k| match k {
            Kind::Tcb(t) => {
                t.last_fault = Some((sender, kind));
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    fn set_last_ipc_error(&mut self, tcb: ObjId, e: KError) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.last_ipc_error = Some(e);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Giao một message **đã xếp hàng** cho TCB nhận (`recv`/`nb_recv`): chép qua `transfer`,
    /// xử lý người gửi (`Send` ⇒ `Ready`; `Call` + Reply ⇒ `BlockedOnReply` + donation;
    /// fault `Exit` ⇒ `Inactive`), và **nhánh (b)** của `no_reply`: thiếu Reply ⇒ kernel bật cờ
    /// trên message + người gọi `Cancelled` (D2-53 Q7). Trả message đã giao (nếu là message
    /// người dùng) để `nb_recv` trả lại cho test.
    fn deliver_queued(
        &mut self,
        receiver: ObjId,
        q: QueuedMessage,
        reply_obj: Option<ObjId>,
        plan: Option<ipc::CapPlan>,
    ) -> Result<Option<Message>, KError> {
        let queued_snaps = q.snaps;
        let mut out = q.msg.map(|m| *m);
        let mut kernel_no_reply = false;
        if let Some(m) = out.as_mut() {
            if q.call && q.blocking && reply_obj.is_none() && !m.info.no_reply() {
                // Kernel thiếu Reply: message **vẫn** giao, cờ bật phía nhận (thiết kế `:322`).
                m.info = m.info.with_no_reply(true);
                kernel_no_reply = true;
            }
            self.deliver_message(receiver, m, &queued_snaps, plan.as_ref())?;
        }
        match q.fault {
            // **KT-26** — fault được ghi nhận cho bên nhận (procsrv) **trước** mọi chuyển
            // trạng thái; message fault không mang MR nên đây là đường duy nhất procsrv biết
            // `(sender, kind)`.
            Some(kind) => {
                self.record_last_fault(receiver, q.sender, kind)?;
                if kind.is_exit() {
                    // Fault Exit: procsrv Recv xong mới Inactive (QD §1 Q-A, D2-25).
                    self.transition_tcb(q.sender, TcbState::Inactive)?;
                } else {
                    // Fault ≠ Exit đi **Call + Reply** tới `fault_ep` (thiết kế §4.6 `:321`):
                    // caller chờ `BlockedOnReply` để procsrv trả lời kèm thanh ghi mới.
                    // Bên nhận **không** cấp Reply ⇒ không có đường resume: caller
                    // `Cancelled` (cùng khuôn Q7(b) cho `Call`), không treo vĩnh viễn.
                    match (q.call, reply_obj) {
                        (true, Some(rep)) if q.blocking => {
                            self.reply_set_caller(rep, Some(q.sender))?;
                            self.transition_tcb(q.sender, TcbState::BlockedOnReply { reply: rep })?;
                            self.donate_sc(q.sender, receiver)?;
                        }
                        _ => {
                            if q.blocking {
                                self.set_last_ipc_error(q.sender, KError::Cancelled)?;
                                self.transition_tcb(q.sender, TcbState::Ready)?;
                            }
                        }
                    }
                }
            }
            None => match (q.call, reply_obj) {
                (true, Some(rep)) if q.blocking => {
                    self.reply_set_caller(rep, Some(q.sender))?;
                    self.transition_tcb(q.sender, TcbState::BlockedOnReply { reply: rep })?;
                    // KT-21: `Call` xếp hàng nay mới giao được ⇒ SC của client đi theo.
                    self.donate_sc(q.sender, receiver)?;
                }
                _ => {
                    if q.blocking {
                        self.transition_tcb(q.sender, TcbState::Ready)?;
                    }
                }
            },
        }
        if kernel_no_reply {
            self.set_last_ipc_error(q.sender, KError::Cancelled)?;
        }
        Ok(out)
    }

    /// `Call` trên Endpoint — bản **message đầy đủ** (KT-24). Ngữ nghĩa:
    ///
    /// - Đích **không** phải Endpoint ⇒ `NotSupported`, không chặn/không Reply (D2-53 Q11).
    /// - Có waiter + Reply cap ⇒ giao thẳng, caller `BlockedOnReply` (như K1b).
    /// - Có waiter nhưng **không** Reply cap ⇒ giao thẳng, kernel bật `no_reply`, caller
    ///   `Cancelled` (D2-53 Q7 nhánh (b)).
    /// - Caller đặt `no_reply=1` ⇒ **Call = Send**, `Ok` ngay, **không** sinh Reply (Q7 nhánh (a)).
    /// - Không có waiter ⇒ xếp hàng + `BlockedOnSend{call:true}` (Call thường) hoặc xếp hàng
    ///   không chặn (no_reply).
    pub fn call_msg(&mut self, tcb: ObjId, ep: SlotRef, msg: Message) -> Result<(), KError> {
        let ep_obj = self.ipc_call_endpoint(ep)?;
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        let (has_waiter, reply_cap) = self.endpoint_recv_slot(ep_obj)?;
        // Reply cap của người nhận phải hợp lệ **trước** khi nhận message (không mất message).
        let reply_obj = match reply_cap {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            None => None,
        };
        // Hàng gửi cố định: đầy ⇒ `Busy` **trước** mọi thay đổi (không cấp `msg_id`, không
        // `last_mr`, không đổi trạng thái) — người gọi xử lý hộp thư rồi thử lại (KT-22 §4.17).
        if !has_waiter && self.endpoint_full(ep_obj)? {
            return Err(KError::Busy);
        }
        // **KT-25**: chụp ảnh cap nguồn + kiểm receive window bên nhận **trước** khi cấp
        // `msg_id` (lỗi cứng ⇒ message không bị giao nửa chừng, không mất).
        let snaps = self.resolve_caps(&msg)?;
        let plan = match self.endpoint_recv_waiter_obj(ep_obj)? {
            Some(w) => self.prepare_caps(w, &snaps, Some(ep_obj))?,
            None => None,
        };
        let mut msg = self.admit_message(msg, snaps.clone())?;
        self.set_endpoint_last_mr(ep_obj, msg.mr[0])?;
        let user_no_reply = msg.info.no_reply();
        if has_waiter {
            let waiter = self
                .endpoint_take_waiter(ep_obj)?
                .expect("has_waiter vừa kiểm");
            let kernel_no_reply = !user_no_reply && reply_obj.is_none();
            if kernel_no_reply {
                msg.info = msg.info.with_no_reply(true);
            }
            self.deliver_message(waiter, &mut msg, &snaps, plan.as_ref())?;
            self.transition_tcb(waiter, TcbState::Ready)?;
            if user_no_reply {
                // (a) Caller đặt cờ: Call = Send, **không** sinh Reply.
                self.sched_after_op(true);
                return Ok(());
            }
            match reply_obj {
                Some(rep) => {
                    self.reply_set_caller(rep, Some(tcb))?;
                    self.transition_tcb(tcb, TcbState::BlockedOnReply { reply: rep })?;
                    // KT-21 (D2-49(c)): SC của client đi theo `Call` sang server passive.
                    self.donate_sc(tcb, waiter)?;
                }
                None => {
                    // (b) Kernel thiếu Reply: message vẫn giao, caller `Cancelled`.
                    self.set_last_ipc_error(tcb, KError::Cancelled)?;
                    self.sched_after_op(true);
                    return Err(KError::Cancelled);
                }
            }
        } else {
            let blocking = !user_no_reply;
            self.endpoint_enqueue(
                ep_obj,
                QueuedMessage {
                    msg: Some(Box::new(msg)),
                    sender: tcb,
                    call: blocking,
                    fault: None,
                    blocking,
                    snaps,
                },
            )?;
            if blocking {
                self.transition_tcb(
                    tcb,
                    TcbState::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: None,
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `Call` bản **một MR** của K1b (giữ API cũ cho `kt09_e2e`/`kt18_tcb`/`kt21_donation`) —
    /// wrapper mỏng của [`Machine::call_msg`] với `Message` 1 word, `label = 0`.
    pub fn call(&mut self, tcb: ObjId, ep: SlotRef, mr: u64) -> Result<(), KError> {
        let info = MessageInfo::new(0, 0, false, 0, 1)?;
        let msg = Message::new(info, &[mr])?;
        self.call_msg(tcb, ep, msg)
    }

    /// `Send` trên Endpoint/Reply: dispatch theo loại cap đúng ABI (`spec/abi.toml:107`):
    /// `Endpoint` ⇒ gửi message (chặn: `BlockedOnSend` khi phải xếp hàng), `Reply` ⇒ trả lời,
    /// `Notification` ⇒ KT-27 (`NotSupported`), loại khác ⇒ `WrongObjectType`.
    pub fn send(&mut self, tcb: ObjId, cap: SlotRef, msg: Message) -> Result<(), KError> {
        match self.ipc_cap_type(cap)? {
            ObjType::Endpoint => self.endpoint_send(tcb, cap, msg, true),
            ObjType::Reply => self.reply_msg(tcb, cap, msg),
            ObjType::Notification => Err(KError::NotSupported),
            _ => Err(KError::WrongObjectType),
        }
    }

    /// `NBSend` — **không chặn**: bên nhận đang `Recv` ⇒ giao ngay; không ⇒ vào hàng gửi của
    /// Endpoint và người gửi **vẫn** `Ready`/`Running` (`spec/abi.toml:108` "không chờ").
    pub fn nb_send(&mut self, tcb: ObjId, ep: SlotRef, msg: Message) -> Result<(), KError> {
        match self.ipc_cap_type(ep)? {
            ObjType::Endpoint => self.endpoint_send(tcb, ep, msg, false),
            // `NBSend` lên Notification/Reply không có nghĩa trong 16 syscall K1c hiện tại.
            ObjType::Notification => Err(KError::NotSupported),
            _ => Err(KError::WrongObjectType),
        }
    }

    /// Đường gửi chung của `Send` (blocking) / `NBSend` (non-blocking) trên Endpoint.
    fn endpoint_send(
        &mut self,
        tcb: ObjId,
        ep: SlotRef,
        msg: Message,
        blocking: bool,
    ) -> Result<(), KError> {
        let cap = self.ipc_slot_cap(ep)?;
        let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        let (has_waiter, _) = self.endpoint_recv_slot(ep_obj)?;
        if !has_waiter && self.endpoint_full(ep_obj)? {
            return Err(KError::Busy);
        }
        // **KT-25**: cap nguồn + receive window của bên nhận kiểm **trước** khi cấp `msg_id`.
        let snaps = self.resolve_caps(&msg)?;
        let plan = match self.endpoint_recv_waiter_obj(ep_obj)? {
            Some(w) => self.prepare_caps(w, &snaps, Some(ep_obj))?,
            None => None,
        };
        let mut msg = self.admit_message(msg, snaps.clone())?;
        self.set_endpoint_last_mr(ep_obj, msg.mr[0])?;
        if has_waiter {
            let waiter = self
                .endpoint_take_waiter(ep_obj)?
                .expect("has_waiter vừa kiểm");
            self.deliver_message(waiter, &mut msg, &snaps, plan.as_ref())?;
            self.transition_tcb(waiter, TcbState::Ready)?;
        } else {
            self.endpoint_enqueue(
                ep_obj,
                QueuedMessage {
                    msg: Some(Box::new(msg)),
                    sender: tcb,
                    call: false,
                    fault: None,
                    blocking,
                    snaps,
                },
            )?;
            if blocking {
                self.transition_tcb(
                    tcb,
                    TcbState::BlockedOnSend {
                        ep: ep_obj,
                        call: false,
                        fault: None,
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `Recv` trên Endpoint: lấy message đang chờ (chép qua `ipc::transfer`), hoặc chặn
    /// `BlockedOnRecv{ep, reply}`. `reply` là Reply cap của **người nhận** (nếu cấp); thiếu nó
    /// khi nhận `Call` ⇒ kernel bật `no_reply` + caller `Cancelled` (D2-53 Q7).
    pub fn recv(&mut self, tcb: ObjId, ep: SlotRef, reply: Option<SlotRef>) -> Result<(), KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let cap = self.ipc_slot_cap(ep)?;
        let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
        self.reset_ipc_error(tcb)?;
        let reply_cap = match reply {
            Some(r) => Some(self.cap_at(r.cnode, r.slot)?),
            None => None,
        };
        let reply_obj = match reply_cap {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            None => None,
        };
        // **KT-25**: kiểm receive window **trước** khi pop (message có cap mà window chưa bind
        // ⇒ `InvalidArgument`, message **ở lại** hàng — không mất, không giao nửa chừng).
        let plan = match self.endpoint_front_snaps(ep_obj)? {
            Some(snaps) => self.prepare_caps(tcb, &snaps, Some(ep_obj))?,
            None => None,
        };
        match self.endpoint_pop(ep_obj)? {
            Some(q) => {
                let _ = self.deliver_queued(tcb, q, reply_obj, plan)?;
            }
            None => {
                self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => {
                        e.recv = Some((tcb, reply_cap));
                        Ok(())
                    }
                    _ => Err(KError::WrongObjectType),
                })?;
                self.transition_tcb(
                    tcb,
                    TcbState::BlockedOnRecv {
                        ep: ep_obj,
                        reply: reply_obj,
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `NBRecv` — **Poll**: `Busy` khi hàng rỗng (`spec/abi.toml:110`), không chặn. Trả message
    /// vừa nhận (đã chép vào `TcbData.ipc`) để test kiểm 64 MR/label/len.
    pub fn nb_recv(&mut self, tcb: ObjId, ep: SlotRef) -> Result<Option<Message>, KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            // **KT-25** — `NBRecv` là **poll** (`spec/abi.toml:110` "không chặn"): TCB không
            // chạy được (đang `BlockedOnReply`/`BlockedOnSend`/`BlockedOnRecv`…) thì "không có
            // message ngay" ⇒ `Busy`, **không** đổi trạng thái. Poll không chặn nên không thể
            // tự nó tạo chuyển trạng thái trái `ALLOWED`; `.scn` 05 dựa vào đúng ngữ nghĩa này
            // (client poll Endpoint sau khi `ReplyRecv` của server đã lấy message của nó).
            return Err(KError::Busy);
        }
        let cap = self.ipc_slot_cap(ep)?;
        let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
        self.reset_ipc_error(tcb)?;
        // **KT-25**: như `recv` — kiểm receive window trước khi pop.
        let plan = match self.endpoint_front_snaps(ep_obj)? {
            Some(snaps) => self.prepare_caps(tcb, &snaps, Some(ep_obj))?,
            None => None,
        };
        match self.endpoint_pop(ep_obj)? {
            Some(q) => {
                let got = self.deliver_queued(tcb, q, None, plan)?;
                self.sched_after_op(true);
                Ok(got)
            }
            None => Err(KError::Busy),
        }
    }

    /// `Send` trên Reply: đánh thức người gọi `BlockedOnReply → Ready` (qua một cửa).
    ///
    /// Nếu `Reply.caller` đã bị `Suspend(Abort)` tách ra: thiết kế §4.4 `:281` — Send **thành
    /// công nhưng reply bị bỏ** (không lỗi, không đánh thức ai).
    pub fn reply(&mut self, tcb: ObjId, reply: SlotRef) -> Result<(), KError> {
        // API K1b không có tham số message ⇒ reply mang message **rỗng** (0 MR) — tương đương
        // `Send(reply)` với `length = 0` của ABI (`spec/abi.toml:107`).
        self.reply_with(tcb, reply, None)
    }

    /// `Send` trên **Reply** với message đầy đủ (ABI `Send` trên Reply = trả lời) — message đi
    /// qua `ipc::transfer` tới **caller** trước khi caller `Ready` (thiết kế §4.6 `:324`).
    pub fn reply_msg(&mut self, tcb: ObjId, reply: SlotRef, msg: Message) -> Result<(), KError> {
        self.reply_with(tcb, reply, Some(msg))
    }

    /// Thân chung của `reply`: trả donation (KT-21), giao message (nếu có) cho caller, rồi đánh
    /// thức caller `BlockedOnReply → Ready` qua **một** cửa `transition_tcb`.
    fn reply_with(
        &mut self,
        tcb: ObjId,
        reply: SlotRef,
        msg: Option<Message>,
    ) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let reply_obj = self.require_type(reply, ObjType::Reply)?;
        // **KT-25**: kiểm cap của message reply + receive window của caller **trước** khi lấy
        // `caller` — lỗi cứng (cap chết/window chưa bind) **không** được để `Reply.caller = None`
        // trong khi caller vẫn `BlockedOnReply` (mất binding = treo).
        let caller_peek = self.body(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller),
            _ => Err(KError::WrongObjectType),
        })?;
        let (snaps, plan) = match msg.as_ref() {
            Some(m) => {
                if m.caps.len() > ipc::MAX_CAPS || m.caps.len() != m.info.caps() as usize {
                    return Err(KError::InvalidArgument);
                }
                let snaps = self.resolve_caps(m)?;
                let plan = match caller_peek {
                    Some(c) => self.prepare_caps(c, &snaps, None)?,
                    None => None,
                };
                (snaps, plan)
            }
            None => (Vec::new(), None),
        };
        let caller = self.body_mut(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller.take()),
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-21 (D2-49(c)): **chỉ** trả SC donation khi Reply đúng Reply của **donor** (người
        // cho mượn SC). Reply nhầm Reply object (`caller = None` hoặc người khác) **không** trả
        // SC và không đánh thức nhầm ai — xem `JOURNAL/kt21-reply-owner.md` (kiểm chủ sở hữu
        // cap/`ReplyRecv`/4 cap đầy đủ thuộc K1c). Khi trả: bucket server về priority gốc,
        // client giữ nguyên `consumed` của chu kỳ (DoD 2).
        if let Some(donor) = self.donation_donor_of(tcb) {
            if caller == Some(donor) {
                self.return_donation(tcb)?;
            }
        }
        if let Some(m) = msg {
            // Message của Reply cũng đi qua **cùng** `admit_message` + `ipc::transfer`; không có
            // caller (reply đã bị bỏ — thiết kế §4.4 `:281`) ⇒ đếm `ipc_dropped` để bất biến
            // bảo toàn message vẫn đúng, không im lặng "mất" message. **KT-25**: cap của Reply
            // cũng đi theo message (đã chụp ảnh + kiểm window **trước** khi lấy `caller`).
            let mut m = self.admit_message(m, snaps.clone())?;
            match caller {
                Some(c) => self.deliver_message(c, &mut m, &snaps, plan.as_ref())?,
                None => self.ipc_dropped += 1,
            }
        }
        if let Some(caller) = caller {
            self.transition_tcb(caller, TcbState::Ready)?;
        }
        if matches!(state, TcbState::Running { .. }) {
            self.transition_tcb(tcb, TcbState::Ready)?;
        }
        // **KT-26**: server vừa trả lời (Reply/ReplyRecv) ⇒ bản ghi "fault vừa nhận" đã xử lý
        // xong; xoá để `recv_exit` kế tiếp không tiêu thụ nhầm fault cũ.
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.last_fault = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-25** — `ReplyRecv` (đường nóng server; thiết kế §4.6 `:320`): giao **reply trước**
    /// (`Send` trên Reply của server), rồi `Recv` trên Endpoint với Reply **mới**.
    ///
    /// Thứ tự bắt buộc: reply được giao **xong** trước khi `Recv` có thể chặn — nếu làm ngược,
    /// `Recv` chặn sẽ bỏ mất reply (K1C-PLAN §5 KT-25 rủi ro (a)). Không có message ⇒
    /// `BlockedOnRecv{ep, reply: Some(reply mới)}` (mang Reply **mới**, không mang Reply vừa
    /// dùng).
    ///
    /// Mọi tham số được kiểm **trước** khi giao reply (cap Reply/Endpoint + Reply mới) ⇒ lỗi
    /// cấu hình không để lại reply đã gửi nửa chừng. `recv_reply = None` ⇒ `Recv` không cấp
    /// Reply cho Call kế (Q7 nhánh (b)).
    pub fn reply_recv(
        &mut self,
        tcb: ObjId,
        reply: SlotRef,
        ep: SlotRef,
        msg: Message,
        recv_reply: Option<SlotRef>,
    ) -> Result<(), KError> {
        let _ = self.require_type(reply, ObjType::Reply)?;
        let _ = self.require_type(ep, ObjType::Endpoint)?;
        if let Some(r) = recv_reply {
            let _ = self.require_type(r, ObjType::Reply)?;
        }
        self.reply_with(tcb, reply, Some(msg))?;
        self.recv(tcb, ep, recv_reply)
    }

    /// **KT-25** — `NBSendRecv` (D2-53 **Q15a**): `NBSend` **không chặn** rồi `Recv` **chặn**.
    ///
    /// Nếu `NBSend` **không giao được** (hàng đầy ⇒ `Busy`, cap sai ⇒ lỗi…) thì **vẫn** `Recv`;
    /// message **không** bị kernel coi là mất (nó chưa hề vào đường IPC — `ipc_sent` không đổi,
    /// người gọi vẫn giữ message để thử lại). Lỗi của `NBSend` được **trả về** sau khi `Recv`
    /// đã chạy: `Ok(Some(err))`; `Ok(None)` = NBSend giao được. `Err` = lỗi cứng của `Recv`
    /// (chưa/không chặn được — vd cap nhận sai) sau khi NBSend đã chạy.
    pub fn nb_send_recv(
        &mut self,
        tcb: ObjId,
        send_ep: SlotRef,
        msg: Message,
        recv_ep: SlotRef,
        recv_reply: Option<SlotRef>,
    ) -> Result<Option<KError>, KError> {
        // Kiểm phía nhận **trước** để lỗi cấu hình không làm message gửi đi rồi mới báo lỗi.
        let _ = self.require_type(recv_ep, ObjType::Endpoint)?;
        if let Some(r) = recv_reply {
            let _ = self.require_type(r, ObjType::Reply)?;
        }
        let send_err = self.nb_send(tcb, send_ep, msg).err();
        match self.recv(tcb, recv_ep, recv_reply) {
            Ok(()) => Ok(send_err),
            Err(e) => Err(e),
        }
    }

    /// **KT-25** — `NBSendWait` (D2-53 **Q15a**): `NBSend` **không chặn** rồi `Wait` trên
    /// Notification. `NBSend` không giao được ⇒ **vẫn** `Wait`; lỗi send trả về dạng
    /// `Ok(Some(err))` như [`Machine::nb_send_recv`]. Ngữ nghĩa `Wait` (word/badge/`Signal`) đầy
    /// đủ thuộc KT-27 — KT-25 dùng `BlockedOnNotification` + một waiter của mô hình K1b.
    pub fn nb_send_wait(
        &mut self,
        tcb: ObjId,
        send_ep: SlotRef,
        msg: Message,
        ntfn: SlotRef,
    ) -> Result<Option<KError>, KError> {
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        let send_err = self.nb_send(tcb, send_ep, msg).err();
        match self.wait_notification_obj(tcb, ntfn_obj) {
            Ok(()) => Ok(send_err),
            Err(e) => Err(e),
        }
    }

    /// `Exit(code)`: có `fault_ep` ⇒ `BlockedOnSend{call=false, fault=Exit{code}}`; không ⇒
    /// `Inactive` ngay; TCB đang `Inactive` + không `fault_ep` thì **không** chuyển trạng thái
    /// (đặt `exit_code` thôi). Message/hàng chờ cũ bị huỷ **trước** khi giao fault (D2-25).
    pub fn exit(&mut self, tcb: ObjId, code: u64) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if !state.is_live() {
            return Err(KError::IllegalOperation);
        }
        // Reviewer-kt18 mục 1: **kiểm `fault_ep` trước** khi ghi `exit_code`/gỡ hàng chờ.
        // Cap `fault_ep` có thể đã bị Delete/Revoke ⇒ `Exit` lỗi mà TCB **không** bị đổi
        // trạng thái/hàng chờ (thiết kế §4.4 invariant; trước sửa: state rác).
        let fault_ep = self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.fault_ep),
            _ => Err(KError::WrongObjectType),
        })?;
        let fault_ep_obj = match fault_ep {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Endpoint)?),
            None => None,
        };
        // KT-23 (bug fuzz tìm ra, xem `kt23_k1b` + JOURNAL): nếu `fault_ep` đang có **người nhận
        // chờ** (`Recv` waiter) thì giao fault **trực tiếp** như đường nhanh của `call` — message
        // **không** vào hàng đợi, nên bất biến "có message ⇒ không `Recv` waiter" (KT-22) giữ
        // nguyên. Procsrv đã chờ sẵn ⇒ coi như `Recv` xong tại thời điểm giao: TCB thoát đi
        // `Inactive` ngay (D2-25 nói `Inactive` sau khi procsrv `Recv`) và **không** bị
        // `BlockedOnSend` trên một message không còn trong hàng.
        // Không có waiter ⇒ giữ nguyên D2-25: hàng gửi cố định, đầy ⇒ `Busy` **trước** mọi thay
        // đổi (không ghi `exit_code`, không đổi trạng thái, không trả donation) — message fault
        // không được "mất nửa chừng"; người gọi thử lại sau khi procsrv `Recv` (§4.17).
        let nguoi_nhan = match fault_ep_obj {
            Some(ep_obj) => self.body(ep_obj, |k| match k {
                Kind::Endpoint(e) => Ok(e.recv.is_some()),
                _ => Err(KError::WrongObjectType),
            })?,
            None => false,
        };
        if let Some(ep_obj) = fault_ep_obj {
            if !nguoi_nhan {
                let con_cho = self.body(ep_obj, |k| match k {
                    Kind::Endpoint(e) => Ok(!e.send.is_full()),
                    _ => Err(KError::WrongObjectType),
                })?;
                if !con_cho {
                    return Err(KError::Busy);
                }
            }
        }
        // KT-21 DoD 5: `Exit` khi đang donate ⇒ trả SC (server chết) / thu hồi SC (client chết)
        // trước khi ghi `exit_code` + đổi trạng thái; không rò SC/không bỏ treo client.
        self.release_donation_of(tcb)?;
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.exit_code = Some(code);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        match fault_ep_obj {
            // Có `Recv` waiter: giao **thẳng** (không vào hàng) — xem chú thích phía trên.
            Some(ep_obj) if nguoi_nhan => {
                self.unlink_tcb_queues(tcb);
                let waiter = self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => Ok(e.recv.take()),
                    _ => Err(KError::WrongObjectType),
                })?;
                if let Some((w, _)) = waiter {
                    // Waiter khác TCB đang thoát (procsrv) ⇒ đánh thức; nếu chính nó là waiter
                    // (procsrv tự Exit khi đang chờ `Recv` cùng endpoint) thì đi thẳng `Inactive`.
                    if w != tcb {
                        // **KT-26**: procsrv phải biết `(sender, Exit{code})` vừa nhận.
                        self.record_last_fault(w, tcb, FaultKind::Exit { code })?;
                        self.transition_tcb(w, TcbState::Ready)?;
                    }
                }
                // KT-23: `Exit` từ `Inactive` giữ `Inactive` (cặp danh tính `(Inactive, Inactive)`
                // bị cấm — cùng quy ước với nhánh không `fault_ep`, D2-25/KT-18).
                if state != TcbState::Inactive {
                    self.transition_tcb(tcb, TcbState::Inactive)?;
                }
            }
            Some(ep_obj) => {
                self.unlink_tcb_queues(tcb);
                self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => {
                        // Đã kiểm chỗ phía trên ⇒ chỉ có thể thành công; map về `Busy` để
                        // không `panic` nếu bất biến đổi (không dùng `expect` trong mô hình).
                        // KT-24: fault `Exit` là **message fault** (không phải IPC message) nên
                        // không có `msg_id`, không tính vào `ipc_sent/delivered/pending`.
                        e.send
                            .try_push(QueuedMessage {
                                msg: None,
                                sender: tcb,
                                call: false,
                                fault: Some(FaultKind::Exit { code }),
                                blocking: true,
                                snaps: Vec::new(),
                            })
                            .map_err(|_| KError::Busy)
                    }
                    _ => Err(KError::WrongObjectType),
                })?;
                self.transition_tcb(
                    tcb,
                    TcbState::BlockedOnSend {
                        ep: ep_obj,
                        call: false,
                        fault: Some(FaultKind::Exit { code }),
                    },
                )?;
            }
            None => {
                if state == TcbState::Inactive {
                    self.sched_after_op(true);
                    return Ok(());
                }
                self.unlink_tcb_queues(tcb);
                self.transition_tcb(tcb, TcbState::Inactive)?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-26** — `inject_fault(tcb, kind)`: mô phỏng một **trap/fault** xảy ra khi `tcb` đang
    /// chạy (`Ready`/`Running`), theo thiết kế §4.6 `:321`: fault ≠ Exit đi **Call tới
    /// `fault_ep`** (caller chờ `BlockedOnReply`, procsrv trả lời để chạy tiếp); `Exit` **không**
    /// đi đường này — dùng [`Machine::exit`] (D2-25, Send không Reply).
    ///
    /// **Model-only `[TK]`, không phải ABI** (D2-53 Q5): `tcb.fault_ep` **phải** có (KT-26 chỉ mô
    /// hình đường có fault endpoint; fault không handler để K3 chốt — trả `InvalidArgument` và
    /// **không** đổi trạng thái). Cap `fault_ep` chết ⇒ `InvalidCapability` (như `Exit`), hàng
    /// gửi của `fault_ep` đầy và không có `Recv` waiter ⇒ `Busy` — **trước** mọi thay đổi, không
    /// giao nửa chừng.
    ///
    /// Có `Recv` waiter (procsrv đang chờ) ⇒ giao **thẳng** như đường nhanh của `call`/`exit`
    /// (message không vào hàng, bất biến "có message ⇒ không waiter" giữ nguyên); waiter **không**
    /// cấp Reply cap ⇒ caller `Cancelled` (không treo vĩnh viễn — cùng khuôn D2-53 Q7(b)).
    pub fn inject_fault(&mut self, tcb: ObjId, kind: FaultKind) -> Result<(), KError> {
        if kind.is_exit() {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(tcb)?;
        if !matches!(state, TcbState::Ready | TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let fault_ep = self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.fault_ep),
            _ => Err(KError::WrongObjectType),
        })?;
        let fault_ep_cap = fault_ep.ok_or(KError::InvalidArgument)?;
        let ep_obj = self.require_cap_type(fault_ep_cap, ObjType::Endpoint)?;
        // Đọc waiter + Reply cap của nó **trước** khi đổi bất cứ thứ gì (lỗi cap cứng không để
        // fault mất nửa chừng).
        let waiter = self.body(ep_obj, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv),
            _ => Err(KError::WrongObjectType),
        })?;
        let reply_obj = match waiter {
            Some((_, Some(cap))) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            _ => None,
        };
        if waiter.is_none() && self.endpoint_full(ep_obj)? {
            return Err(KError::Busy);
        }
        match waiter {
            Some((w, _)) => {
                // Giao thẳng: đưa TCB vào trạng thái chặn **trước** (mọi chuyển trạng thái bên
                // trong `deliver_queued` khi đó nằm trong bảng `ALLOWED`: `3→5` Reply, `3→1`
                // Cancelled), rồi lấy waiter khỏi Endpoint và dùng **cùng** đường
                // `deliver_queued` (một chỗ xử lý fault: `last_fault` + Reply/Cancelled).
                self.transition_tcb(
                    tcb,
                    TcbState::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: Some(kind),
                    },
                )?;
                let taken = self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => Ok(e.recv.take()),
                    _ => Err(KError::WrongObjectType),
                })?;
                debug_assert!(matches!(taken, Some((w2, _)) if w2 == w));
                let q = QueuedMessage {
                    msg: None,
                    sender: tcb,
                    call: true,
                    fault: Some(kind),
                    blocking: true,
                    snaps: Vec::new(),
                };
                let _ = self.deliver_queued(w, q, reply_obj, None)?;
                if w != tcb {
                    self.transition_tcb(w, TcbState::Ready)?;
                }
            }
            None => {
                self.endpoint_enqueue(
                    ep_obj,
                    QueuedMessage {
                        msg: None,
                        sender: tcb,
                        call: true,
                        fault: Some(kind),
                        blocking: true,
                        snaps: Vec::new(),
                    },
                )?;
                self.transition_tcb(
                    tcb,
                    TcbState::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: Some(kind),
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    // ---------------------------------------------------------------- nội bộ

    /// Cấp vùng nhớ (arena) cho một Untyped; trả chỉ số trong `regions`.
    ///
    /// `id_base` khác nhau cho từng vùng để `ObjRef` của vùng này không trùng `id` vùng
    /// khác (ref không mang identity store — KT-15/KT-16); tối đa `2^40` object/vùng.
    fn push_region(&mut self, size: usize) -> usize {
        let id_base = ((self.regions.len() as u64) + 1) << 40;
        let mem = OwnedMem::with_id_base(size, id_base).expect("cấp vùng nhớ cho Untyped");
        self.regions.push(mem);
        self.regions.len() - 1
    }

    fn push_meta(&mut self, ty: ObjType, loc: Loc, body: ObjRef<Kind>, cho: ChoDat) -> ObjId {
        self.objects.push(Meta {
            ty,
            alive: true,
            untyped: cho.untyped,
            offset: cho.offset,
            len: cho.len,
            loc,
            body,
            region: cho.region,
            slots: cho.slots,
        });
        self.objects.len() as ObjId
    }

    /// Đặt body của object do harness cấp vào arena kernel.
    fn push_kernel(&mut self, ty: ObjType, kind: Kind, cho: ChoDat) -> ObjId {
        let body = self
            .kernel
            .alloc_place(kind)
            .expect("arena kernel còn chỗ (tăng bằng Machine::with_arena_bytes)");
        self.push_meta(ty, Loc::Kernel, body, cho)
    }

    fn push_cap(
        &mut self,
        obj: ObjId,
        ty: ObjType,
        parent: Option<CapId>,
        rights: u8,
        badge: Option<u64>,
    ) -> CapId {
        self.caps.push(Cap {
            obj,
            ty,
            parent,
            slot: None,
            rights,
            badge,
            alive: true,
        });
        self.caps.len() as CapId
    }

    fn fresh_kind(ty: ObjType, child_slots: usize) -> Kind {
        match ty {
            ObjType::Untyped => Kind::Untyped,
            ObjType::CNode => Kind::CNode {
                slots: vec![None; child_slots.max(1)],
            },
            ObjType::Tcb => Kind::Tcb(Box::new(TcbData {
                state: TcbState::Inactive,
                suspended: false,
                exit_code: None,
                fault_ep: None,
                cspace: None,
                priority: 0,
                max_controlled_priority: 0,
                affinity: AFFINITY_HOME,
                home: 0,
                sched_context: None,
                donated_sc: None,
                bound_notification: None,
                ipc: IpcArea::new(),
                recv_window: None,
                last_ipc_error: None,
                last_fault: None,
            })),
            ObjType::SchedContext => Kind::SchedContext(Box::new(SchedData {
                bound: None,
                budget: ScBudget::new(),
                donated_to: None,
                donor: None,
            })),
            ObjType::Endpoint => Kind::Endpoint(Box::default()),
            ObjType::Reply => Kind::Reply(Box::new(ReplyData { caller: None })),
            ObjType::Notification => Kind::Notification(Box::default()),
            _ => Kind::Plain,
        }
    }

    fn object_index(&self, obj: ObjId) -> Result<usize, KError> {
        if obj == 0 || obj as usize > self.objects.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(obj as usize - 1)
    }

    fn meta(&self, obj: ObjId) -> Result<&Meta, KError> {
        Ok(&self.objects[self.object_index(obj)?])
    }

    fn store(&self, loc: Loc) -> &OwnedMem {
        match loc {
            Loc::Kernel => &self.kernel,
            Loc::Untyped(i) => &self.regions[i as usize],
        }
    }

    fn store_mut(&mut self, loc: Loc) -> &mut OwnedMem {
        match loc {
            Loc::Kernel => &mut self.kernel,
            Loc::Untyped(i) => &mut self.regions[i as usize],
        }
    }

    /// Truy cập body qua store chứa nó; lỗi objmem (`Dead`) quy về `InvalidCapability`
    /// như `object()` cũ (mọi thao tác công khai đều kiểm `cap`/`require_*` trước).
    fn body<R>(&self, obj: ObjId, f: impl FnOnce(&Kind) -> Result<R, KError>) -> Result<R, KError> {
        let (loc, body) = {
            let m = self.meta(obj)?;
            (m.loc, m.body)
        };
        self.store(loc)
            .with(body, f)
            .map_err(|_| KError::InvalidCapability)?
    }

    fn body_mut<R>(
        &mut self,
        obj: ObjId,
        f: impl FnOnce(&mut Kind) -> Result<R, KError>,
    ) -> Result<R, KError> {
        let (loc, body) = {
            let m = self.meta(obj)?;
            (m.loc, m.body)
        };
        self.store_mut(loc)
            .with_mut(body, f)
            .map_err(|_| KError::InvalidCapability)?
    }

    fn cap(&self, cap: CapId) -> Result<&Cap, KError> {
        if cap == 0 || cap as usize > self.caps.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(&self.caps[cap as usize - 1])
    }

    /// Ảnh chụp slot của CNode (clone — body nằm trong arena, closure không trả tham chiếu).
    fn cnode_slots_of(&self, cnode: ObjId) -> Result<Vec<Option<CapId>>, KError> {
        self.body(cnode, |k| match k {
            Kind::CNode { slots } => Ok(slots.clone()),
            _ => Err(KError::WrongObjectType),
        })
    }

    fn cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<Option<CapId>, KError> {
        let (alive, slots) = {
            let m = self.meta(cnode)?;
            (m.alive, m.slots)
        };
        if !alive {
            let n = slots.ok_or(KError::WrongObjectType)?;
            return if slot < n {
                Ok(None)
            } else {
                Err(KError::RangeError)
            };
        }
        self.body(cnode, |k| match k {
            Kind::CNode { slots } => slots.get(slot).copied().ok_or(KError::RangeError),
            _ => Err(KError::WrongObjectType),
        })
    }

    fn cnode_set_slot(
        &mut self,
        cnode: ObjId,
        slot: usize,
        cap: Option<CapId>,
    ) -> Result<(), KError> {
        self.body_mut(cnode, |k| match k {
            Kind::CNode { slots } => match slots.get_mut(slot) {
                Some(s) => {
                    *s = cap;
                    Ok(())
                }
                None => Err(KError::RangeError),
            },
            _ => Err(KError::WrongObjectType),
        })
    }

    fn check_cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<(), KError> {
        let n = self.cnode_slot_count(cnode)?;
        if slot >= n {
            return Err(KError::RangeError);
        }
        Ok(())
    }

    fn require_cap_type(&self, cap: CapId, ty: ObjType) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if !c.alive {
            return Err(KError::InvalidCapability);
        }
        if c.ty != ty {
            return Err(KError::WrongObjectType);
        }
        Ok(c.obj)
    }

    fn require_type(&self, slot: SlotRef, ty: ObjType) -> Result<ObjId, KError> {
        self.require_cap_type(self.cap_at(slot.cnode, slot.slot)?, ty)
    }

    /// Kích hoạt ngầm của **mô hình K1b**: mô hình chưa có method `Activate`, nên TCB
    /// `Inactive` tự chuyển `Inactive → Ready` khi nó phát syscall IPC đầu tiên — cặp
    /// `(Inactive, Ready)` của bảng ALLOWED. KHÔNG phải ABI; KT-19 thêm `pick`/`Ready`.
    fn ensure_started(&mut self, tcb: ObjId) -> Result<TcbState, KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Inactive {
            self.transition_tcb(tcb, TcbState::Ready)?;
            return Ok(TcbState::Ready);
        }
        Ok(state)
    }

    fn set_tcb_suspended(&mut self, tcb: ObjId, suspended: bool) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.suspended = suspended;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // `suspended` trực giao với state: bật ⇒ gỡ khỏi ready queue; tắt ⇒ sched_sync
        // của caller đưa lại đúng mức (KT-19 DoD 5).
        self.sched_sync(tcb)
    }

    /// Gỡ `tcb` khỏi **mọi** hàng chờ nó đang nằm trong: hàng gửi/nhận của Endpoint,
    /// `Reply.caller`, waiter của Notification, bucket WaitOnAddress. Trả **số hàng** đã gỡ.
    ///
    /// Gọi **trước** khi bật `suspended`/chuyển trạng thái (thiết kế §4.4 invariant: TCB nằm
    /// trong hàng của X **khi và chỉ khi** trạng thái mang X **và** `!suspended`).
    fn unlink_tcb_queues(&mut self, tcb: ObjId) -> usize {
        let live: Vec<(ObjId, ObjType)> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive)
            .map(|(i, m)| ((i as u32) + 1, m.ty))
            .collect();
        let mut removed = 0usize;
        // KT-24: message **người dùng** bị huỷ theo TCB (D2-25) phải đếm vào `ipc_dropped`
        // để bất biến `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped` giữ nguyên.
        let mut msg_dropped = 0usize;
        for (obj, ty) in live {
            match ty {
                ObjType::Endpoint => {
                    let _ = self.body_mut(obj, |k| match k {
                        Kind::Endpoint(e) => {
                            if let Some((waiter, _)) = e.recv {
                                if waiter == tcb {
                                    e.recv = None;
                                    removed += 1;
                                }
                            }
                            // KT-22: gỡ message của TCB khỏi **mailbox cố định**; `retain`
                            // đếm `removed` nên bất biến `pushed == popped + removed + len`
                            // vẫn đúng sau khi huỷ message (D2-25).
                            removed += e.send.retain(|q| {
                                if q.sender != tcb {
                                    return true;
                                }
                                if q.msg.is_some() {
                                    msg_dropped += 1;
                                }
                                false
                            });
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    });
                }
                ObjType::Reply => {
                    let _ = self.body_mut(obj, |k| match k {
                        Kind::Reply(r) => {
                            if r.caller == Some(tcb) {
                                r.caller = None;
                                removed += 1;
                            }
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    });
                }
                ObjType::Notification => {
                    let _ = self.body_mut(obj, |k| match k {
                        Kind::Notification(n) => {
                            if n.waiter == Some(tcb) {
                                n.waiter = None;
                                removed += 1;
                            }
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    });
                }
                _ => {}
            }
        }
        let before = self.address_waiters.len();
        self.address_waiters.retain(|(_, _, t)| *t != tcb);
        removed += before - self.address_waiters.len();
        self.ipc_dropped += msg_dropped as u64;
        removed
    }

    fn reply_set_caller(&mut self, reply: ObjId, caller: Option<ObjId>) -> Result<(), KError> {
        self.body_mut(reply, |k| match k {
            Kind::Reply(r) => {
                r.caller = caller;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    fn clear_slot_of(&mut self, cap: CapId) {
        let slot = self
            .caps
            .get_mut(cap as usize - 1)
            .and_then(|c| c.slot.take());
        if let Some(SlotRef { cnode, slot: s }) = slot {
            // Object có thể đã bị đánh dấu chết nhưng body còn sống trong lúc revoke;
            // lỗi `Dead` (nếu có) được bỏ qua — slot của CNode chết đã được xoá trắng trước.
            let _ = self.body_mut(cnode, |k| {
                if let Kind::CNode { slots } = k {
                    if slots.get(s).copied().flatten() == Some(cap) {
                        slots[s] = None;
                    }
                }
                Ok(())
            });
        }
    }

    /// Xoá cap con/cháu của `root`; nếu `root` là cap Untyped thì finalise object + reset
    /// arena của cả cây Untyped (watermark về 0, thiết kế §4.2 — KT-16: arena `reset`).
    fn revoke_children(&mut self, root: CapId) -> Result<usize, KError> {
        if !self.cap(root)?.alive {
            return Err(KError::InvalidCapability);
        }
        let mut doomed: Vec<CapId> = Vec::new();
        for i in 1..=self.caps.len() as CapId {
            if !self.caps[i as usize - 1].alive || i == root {
                continue;
            }
            let mut parent = self.caps[i as usize - 1].parent;
            while let Some(p) = parent {
                if p == root {
                    doomed.push(i);
                    break;
                }
                parent = self.caps[p as usize - 1].parent;
            }
        }

        let root_ty = self.cap(root)?.ty;
        let root_obj = self.cap(root)?.obj;
        if root_ty == ObjType::Untyped {
            let u_idx = self.object_index(root_obj)?;
            let mut dead_objs: Vec<ObjId> = Vec::new();
            // Untyped lồng nhau: finalise **cả cây con** (object của Untyped con cũng chết,
            // không để cap trỏ tới object mồ côi — review Agent 0 mục 3).
            self.collect_untyped_tree(u_idx, &mut dead_objs);
            // Finalise CNode (seL4 `finaliseCNode`): cap nằm trong slot của CNode chết phải chết
            // theo **cả cây con của nó**, kể cả khi object đích còn sống (object ngoài Untyped).
            let mut cnode_caps: Vec<CapId> = Vec::new();
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty != ObjType::CNode {
                    continue;
                }
                if let Ok(slots) = self.cnode_slots_of(*id) {
                    cnode_caps.extend(slots.iter().flatten().copied());
                }
            }
            for cap in cnode_caps {
                if self.cap(cap)?.alive {
                    self.collect_subtree(cap, &mut doomed);
                }
            }
            // Slot của CNode chết không được giữ cap nào (kể cả cap đã chết từ trước).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty != ObjType::CNode {
                    continue;
                }
                let _ = self.body_mut(*id, |k| {
                    if let Kind::CNode { slots } = k {
                        slots.fill(None);
                    }
                    Ok(())
                });
            }
            // KT-18 (DoD 9): finalise TCB **trước** khi object chết — gỡ hàng chờ, `Dead`,
            // trả `sched_context` qua hook, ghi log (thiết kế §4.3 bảng Finalise).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::Tcb {
                    let _ = self.finalise_tcb(*id);
                }
            }
            // KT-24: message **người dùng** còn trong Endpoint sắp bị finalise phải đếm
            // `ipc_dropped` — nếu không, bất biến bảo toàn message thấy "mất" message khi
            // Revoke Untyped chứa Endpoint (D2-25: message bị huỷ là hợp lệ).
            let mut dropped_msgs = 0u64;
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty != ObjType::Endpoint {
                    continue;
                }
                if let Ok(n) = self.body(*id, |k| match k {
                    Kind::Endpoint(e) => {
                        Ok(e.send.iter().filter(|q| q.msg.is_some()).count() as u64)
                    }
                    _ => Err(KError::WrongObjectType),
                }) {
                    dropped_msgs += n;
                }
            }
            self.ipc_dropped += dropped_msgs;
            // **KT-25** — Reply nằm trong Untyped bị revoke cũng "bị xoá": caller đang chờ được
            // đánh thức `Cancelled` (Q8). Phải làm **trước** khi body bị free (không đọc được sau).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::Reply {
                    self.cancel_reply(*id)?;
                }
            }
            // Đánh dấu chết + **finalise body trong objmem** (KT-16; `free` guard `pa`).
            for id in &dead_objs {
                let (loc, body) = {
                    let m = &mut self.objects[*id as usize - 1];
                    m.alive = false;
                    (m.loc, m.body)
                };
                self.store_mut(loc).free(body);
            }
            // Reset arena của **mọi** Untyped trong cây: watermark của mô hình = `used()`;
            // chỉ reset sau khi mọi body trong vùng đã finalise.
            let mut regions: Vec<usize> = Vec::new();
            for id in &dead_objs {
                if let Some(r) = self.objects[*id as usize - 1].region {
                    regions.push(r);
                }
            }
            if let Some(r) = self.objects[u_idx].region {
                regions.push(r);
            }
            for r in regions {
                self.regions[r].reset();
            }
            for i in 1..=self.caps.len() as CapId {
                if self.caps[i as usize - 1].alive
                    && dead_objs.contains(&self.caps[i as usize - 1].obj)
                {
                    doomed.push(i);
                }
            }
        }

        doomed.sort_unstable();
        doomed.dedup();
        // **KT-25** — Reply mất cap cuối qua `Revoke` cũng phải huỷ đường reply (Q8): ghi lại
        // object Reply của các cap sắp chết để kiểm sau khi xoá xong.
        let mut reply_objs: Vec<ObjId> = Vec::new();
        for cap in &doomed {
            let c = &self.caps[*cap as usize - 1];
            if c.alive && c.ty == ObjType::Reply && !reply_objs.contains(&c.obj) {
                reply_objs.push(c.obj);
            }
        }
        let mut deleted = 0;
        for cap in doomed {
            if self.caps[cap as usize - 1].alive {
                self.clear_slot_of(cap);
                self.caps[cap as usize - 1].alive = false;
                deleted += 1;
            }
        }
        for obj in reply_objs {
            self.cancel_reply_of_cap(obj)?;
        }
        self.purge_dead_refs();
        Ok(deleted)
    }

    /// Gom `root` và mọi con/cháu còn sống của nó (theo cạnh cha) vào `out`.
    fn collect_subtree(&self, root: CapId, out: &mut Vec<CapId>) {
        for i in 1..=self.caps.len() as CapId {
            if !self.caps[i as usize - 1].alive {
                continue;
            }
            let mut cur = Some(i);
            while let Some(p) = cur {
                if p == root {
                    out.push(i);
                    break;
                }
                if p == 0 || p as usize > self.caps.len() {
                    break;
                }
                cur = self.caps[p as usize - 1].parent;
            }
        }
    }

    /// Gom mọi object sống nằm trong `u_idx`, đệ quy qua Untyped con.
    fn collect_untyped_tree(&self, u_idx: usize, out: &mut Vec<ObjId>) {
        for (i, m) in self.objects.iter().enumerate() {
            if m.alive && m.untyped == Some(u_idx) {
                out.push((i as u32) + 1);
                if m.ty == ObjType::Untyped {
                    self.collect_untyped_tree(i, out);
                }
            }
        }
    }

    /// Bỏ tham chiếu tới object/cap chết (thiết kế §4.2: không trường nào trỏ tới object chết).
    fn purge_dead_refs(&mut self) {
        let alive_objs: Vec<bool> = self.objects.iter().map(|m| m.alive).collect();
        let alive_caps: Vec<bool> = self.caps.iter().map(|c| c.alive).collect();
        let n = alive_objs.len();
        let n_caps = alive_caps.len();
        let is_alive =
            |id: ObjId| -> bool { id > 0 && (id as usize) <= n && alive_objs[id as usize - 1] };
        let cap_alive = |cap: CapId| -> bool {
            cap > 0 && (cap as usize) <= n_caps && alive_caps[cap as usize - 1]
        };
        let mut tong_dropped = 0u64;
        for (i, alive) in alive_objs.iter().enumerate() {
            if !*alive {
                continue;
            }
            let obj = (i + 1) as ObjId;
            // KT-24: message người dùng bị gỡ ở đây (sender đã chết) cũng phải đếm `ipc_dropped`.
            let mut dropped = 0u64;
            let _ = self.body_mut(obj, |k| {
                match k {
                    Kind::Endpoint(e) => {
                        if let Some((tcb, reply_cap)) = e.recv {
                            if !is_alive(tcb) {
                                e.recv = None;
                            } else if let Some(cap) = reply_cap {
                                // **KT-25**: Reply cap của waiter đã bị xoá ⇒ coi như `Recv`
                                // không cấp Reply (Q7 nhánh (b) khi Call tới) — không giữ cap chết.
                                if !cap_alive(cap) {
                                    e.recv = Some((tcb, None));
                                }
                            }
                        }
                        e.send.retain(|q| {
                            if is_alive(q.sender) {
                                return true;
                            }
                            if q.msg.is_some() {
                                dropped += 1;
                            }
                            false
                        });
                    }
                    Kind::Reply(r) => {
                        if let Some(c) = r.caller {
                            if !is_alive(c) {
                                r.caller = None;
                            }
                        }
                    }
                    Kind::Notification(n) => {
                        if let Some(w) = n.waiter {
                            if !is_alive(w) {
                                n.waiter = None;
                            }
                        }
                    }
                    Kind::Tcb(t) => {
                        if let Some(cap) = t.fault_ep {
                            if !cap_alive(cap) {
                                t.fault_ep = None;
                            }
                        }
                        if let Some(c) = t.cspace {
                            if !is_alive(c) {
                                t.cspace = None;
                            }
                        }
                        if let Some(c) = t.sched_context {
                            if !is_alive(c) {
                                t.sched_context = None;
                            }
                        }
                        if let Some(c) = t.bound_notification {
                            if !is_alive(c) {
                                t.bound_notification = None;
                            }
                        }
                        if let Some(c) = t.donated_sc {
                            if !is_alive(c) {
                                t.donated_sc = None;
                            }
                        }
                    }
                    Kind::SchedContext(sd) => {
                        // KT-21: donation chỉ còn hiệu lực khi **cả hai** TCB sống.
                        let con = match (sd.donated_to, sd.donor) {
                            (Some(d), Some(o)) => is_alive(d) && is_alive(o),
                            (None, None) => true,
                            _ => false,
                        };
                        if !con {
                            sd.donated_to = None;
                            sd.donor = None;
                        }
                    }
                    _ => {}
                }
                Ok(())
            });
            tong_dropped += dropped;
        }
        self.ipc_dropped += tong_dropped;
        // Bucket WaitOnAddress: bỏ waiter/nút có frame hoặc TCB đã chết (§4.7).
        self.address_waiters
            .retain(|(frame, _, tcb)| is_alive(*frame) && is_alive(*tcb));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_loi_lay_dung_tu_spec() {
        assert_eq!(KError::InvalidCapability.name(), "InvalidCapability");
        assert_eq!(KError::InvalidCapability.id(), 1);
        assert_eq!(KError::Alignment.id(), 6);
        assert_eq!(KError::NotEnoughMemory.id(), 9);
        assert_eq!(KError::NotSupported.id(), 18);
        assert_eq!(KError::RangeError.id(), 19);
        for (i, (name, id)) in vifast_abi::ERRORS.iter().enumerate() {
            let k = [
                KError::InvalidCapability,
                KError::WrongObjectType,
                KError::InsufficientRights,
                KError::WxViolation,
                KError::InvalidArgument,
                KError::Alignment,
                KError::SlotOccupied,
                KError::SlotEmpty,
                KError::NotEnoughMemory,
                KError::AlreadyMapped,
                KError::NotMapped,
                KError::MissingPageTable,
                KError::HasChildren,
                KError::Busy,
                KError::Cancelled,
                KError::Timeout,
                KError::IllegalOperation,
                KError::NotSupported,
                KError::RangeError,
            ][i];
            assert_eq!(k.name(), *name, "thứ tự mã lỗi lệch spec ở {i}");
            assert_eq!(k.id(), *id);
        }
    }

    #[test]
    fn ten_object_khop_spec() {
        for (name, _id) in vifast_abi::CORE_OBJECTS {
            assert!(ObjType::from_name(name).is_some(), "thiếu {name}");
        }
        for (name, _id) in vifast_abi::RESERVED_OBJECTS {
            let t = ObjType::from_name(name).expect("reserved phải có tên");
            assert!(!t.is_core(), "{name} phải là reserved");
        }
        assert_eq!(ObjType::from_name("KhongCo"), None);
    }

    #[test]
    fn tcb_state_du_11_ten_theo_spec() {
        use std::collections::BTreeSet;
        let all = [
            TcbState::Inactive,
            TcbState::Ready,
            TcbState::Running { hart: 0 },
            TcbState::BlockedOnSend {
                ep: 1,
                call: true,
                fault: None,
            },
            TcbState::BlockedOnRecv { ep: 1, reply: None },
            TcbState::BlockedOnReply { reply: 1 },
            TcbState::BlockedOnNotification { ntfn: 1 },
            TcbState::BlockedOnAddress {
                frame: 1,
                offset: 16,
            },
            TcbState::Sleeping {
                reason: SleepReason::Timeout,
            },
            TcbState::Stopping,
            TcbState::Dead,
        ];
        let names: BTreeSet<&str> = all.iter().map(TcbState::name).collect();
        let want: BTreeSet<&str> = vifast_abi::TCB_STATES.iter().copied().collect();
        assert_eq!(names, want, "TcbState phải đủ 11 tên của spec D2-33");
        assert_eq!(all.len(), 11);
        assert_eq!(TcbState::Running { hart: 2 }.render(), "Running{hart=2}");
        assert_eq!(
            TcbState::BlockedOnNotification { ntfn: 5 }.render(),
            "BlockedOnNotification{ntfn=5}"
        );
        assert_eq!(
            TcbState::BlockedOnAddress {
                frame: 3,
                offset: 16
            }
            .render(),
            "BlockedOnAddress{frame=3, offset=16}"
        );
        assert_eq!(
            TcbState::Sleeping {
                reason: SleepReason::Budget
            }
            .render(),
            "Sleeping{reason=Budget}"
        );
        assert_eq!(TcbState::Stopping.render(), "Stopping");
        assert!(!TcbState::Stopping.is_runnable());
        assert!(TcbState::Running { hart: 0 }.is_runnable());
    }

    /// Công thức căn lề cũ — giữ làm **mốc so** cho phần đã chuyển vào arena objmem
    /// (KT-16: `Arena::alloc` phải cho cùng kết quả alignment này).
    fn align_up(value: usize, align: usize) -> usize {
        (value + align - 1) & !(align - 1)
    }

    #[test]
    fn can_le_va_can_duoi() {
        assert_eq!(align_up(0, 16), 0);
        assert_eq!(align_up(1, 16), 16);
        assert_eq!(align_up(512, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
    }

    /// `check_caps` không được là checker rỗng: gieo hỏng cây thì phải bắt.
    #[test]
    fn check_caps_bat_cap_mo_coi() {
        let mut m = Machine::new();
        let a = m.add_cnode(4);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let root = m.root_cap(ep);
        m.put_cap(a, 0, root).expect("cap gốc");
        let child = m
            .copy(SlotRef::new(a, 0), SlotRef::new(a, 1))
            .expect("cap con");
        m.check_caps().expect("cây hợp lệ");
        // Cha bị xoá (mất slot, chết) nhưng con **không được reparent** — đúng lỗi cũ
        // của `delete`; checker phải bắt cap mồ côi.
        m.caps[root as usize - 1].alive = false;
        m.caps[root as usize - 1].slot = None;
        m.cnode_set_slot(a, 0, None).expect("CNode");
        let err = m.check_caps().expect_err("phải bắt cap mồ côi");
        assert!(err.contains("mồ côi"), "{err}");
        assert!(m.cap_info(child).expect("con").alive);
    }

    /// Slot trỏ cap nhưng cap không trỏ ngược (MDB hỏng) phải bị bắt.
    #[test]
    fn check_caps_bat_slot_khong_doi_xung() {
        let mut m = Machine::new();
        let a = m.add_cnode(4);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let cap = m.root_cap(ep);
        m.put_cap(a, 0, cap).expect("cap gốc");
        m.caps[cap as usize - 1].slot = None;
        let err = m.check_caps().expect_err("phải bắt slot không đối xứng");
        assert!(
            err.contains("không trỏ ngược") || err.contains("không trỏ lại"),
            "{err}"
        );
        // Cap chết còn giữ slot cũng là hỏng.
        let mut m2 = Machine::new();
        let b = m2.add_cnode(4);
        let ep2 = m2.add_kernel_object(ObjType::Endpoint);
        let cap2 = m2.root_cap(ep2);
        m2.put_cap(b, 0, cap2).expect("cap gốc");
        m2.caps[cap2 as usize - 1].alive = false;
        let err = m2.check_caps().expect_err("phải bắt cap chết còn giữ slot");
        assert!(err.contains("cap chết"), "{err}");
    }
}
