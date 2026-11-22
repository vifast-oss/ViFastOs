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
//!
//! **KT-36 — Frame 3 cỡ, ownership + `GetAddress` + bộ đếm W^X** (K2-PLAN §3, D2-65 Q2/Q3/Q7;
//! `v2/docs/k2-vm.md` §10; thiết kế §4.10 `:353-356`):
//! - `Retype(Untyped, Frame, size_bits)` chỉ nhận `size_bits ∈ {12, 21, 30}` (cỡ lấy từ
//!   `vifast_pte::FrameSize`, không chép tay); cỡ khác ⇒ `InvalidArgument` (không còn
//!   `NotSupported` như thời reserved).
//! - Frame **không tiêu byte arena** ("không cấp byte thật", rủi ro (a) của KT-36): nó cấp từ
//!   **cửa sổ PA ảo** [`UNTYPED_PA_WINDOW`] của Untyped sở hữu, `pa = base + watermark`; lệch
//!   căn ⇒ `Alignment` (không đệm ngầm), đầy cửa sổ ⇒ `NotEnoughMemory`, chồng `pa` với Frame
//!   sống khác ⇒ `AlreadyMapped`. Body của Frame chỉ là mốc [`Kind::Frame`] trong arena kernel
//!   (vài chục byte metadata), **không** phải vùng nhớ của Frame.
//! - `Frame.GetAddress` (method 2) = [`Machine::frame_get_address`] (qua cap) /
//!   [`Machine::frame_address`] (theo `ObjId`).
//! - Bộ đếm `maps/mapped_w/mapped_x` nằm trong [`FrameMeta`]; KT-36 chỉ mở hai cửa
//!   [`Machine::frame_note_map`]/[`Machine::frame_note_unmap`] (map thật là KT-37/KT-39).
//! - Frame chết ⇒ **không còn mapping**: `Revoke` Untyped còn Frame đang map bị từ chối
//!   `HasChildren` (atomic, "lỗi bất biến, không im lặng"); [`Machine::check_frames`] kiểm lại
//!   sau mỗi op (KT-42 mở rộng thành `check_vm`).

use crate::clock::MonotonicClock;
use crate::frame::FrameArea;
/// **KT-36** — metadata Frame + cửa sổ PA của Untyped (định nghĩa ở [`crate::frame`];
/// re-export để test/harness dùng `vifast_sim::kobj::{FrameMeta, UNTYPED_PA_WINDOW}`).
pub use crate::frame::{FrameMeta, WxRecount, UNTYPED_PA_WINDOW};
use crate::ipc::{self, IpcArea, Message, MessageInfo};
use crate::mailbox::{HartMailbox, Mailbox, MailboxMsg, MAILBOX_CAP};
use crate::pagetable::{
    decode_pte, index_in_level, leaf_pte, level_for_size_bits, size_bits_for_level, table_pte,
    table_span, validate_attrs, DecodedPte, PageTableData, PT_BYTES, PT_ENTRY_COUNT, PT_ROOT_LEVEL,
    VA_LIMIT,
};
/// **KT-37** — kiểu quyền/attrs/mapping/ảnh chụp PageTable (định nghĩa ở [`crate::pagetable`];
/// re-export để test dùng `vifast_sim::kobj::{MapRights, MapAttrs, Mapping, PageTableInfo}`).
pub use crate::pagetable::{MapAttrs, MapRights, Mapping, PageTableInfo};
use crate::sched::HartSched;
/// **KT-29** — trường công bố `idle`/`cur_prio` của mỗi hart (định nghĩa ở [`crate::sched`];
/// re-export để test dùng `vifast_sim::kobj::Published`).
pub use crate::sched::Published;
use crate::tlb::{HartTlb, TlbEntry};
/// **KT-40** — TLB + shootdown + ack (định nghĩa ở [`crate::tlb`]; re-export để test dùng
/// `vifast_sim::kobj::{TlbFlush, TlbFlushScope, TlbHartInfo, TlbShootdownInfo, TlbTranslation,
/// TLB_ENTRIES_PER_HART}`). Message `TlbFlush` nằm trong [`MailboxMsg::TlbFlush`] (D2-65 Q5).
pub use crate::tlb::{
    TlbEntry as TlbEntryInfo, TlbFlush, TlbFlushScope, TlbHartInfo, TlbKey, TlbShootdownInfo,
    TlbTranslation, TLB_ENTRIES_PER_HART,
};
use crate::vspace::{fresh_asid_free, VSpaceData};
/// **KT-38** — `VSpace` + ASID + thế hệ + `active_on: HartMask` (định nghĩa ở [`crate::vspace`];
/// re-export để test dùng `vifast_sim::kobj::{HartMask, AsidState, FlushCmd, FlushScope,
/// VSpaceInfo, ASID_COUNT}`). `ASID_COUNT` = `1 << vifast_pte::ASID_BITS` — đọc từ crate hình
/// PTE, không chép tay `256`.
pub use crate::vspace::{AsidState, FlushCmd, FlushScope, HartMask, VSpaceInfo, ASID_COUNT};
use vifast_objmem::{Dead, KObj, ObjRef, ObjStore, OwnedMem, ARENA_ALIGN};
use vifast_pte::{FrameSize, Pte};

/// **KT-241** — cửa ghi trạng thái TCB (D2-187/D2-194): nguyên thuỷ thô **private** +
/// năm cửa `Ready`. Là **module con** của `kobj` nên thấy được item private của `kobj`
/// (không phải nới `pub(crate)` cho field/method nào của [`Machine`]).
mod state_write;
/// **KT-241** — đích chuyển trạng thái **loại trừ `Ready`** (10 biến thể còn lại của
/// [`TcbState`]); re-export để `vifast_sim::kobj::NonReadyTarget` hoạt động, đối xứng
/// `vifast_core::NonReadyTarget`.
pub use state_write::NonReadyTarget;

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

/// Số hart **mặc định** của mô hình: **1** (`Machine::new()` — D2-49(a); KT-29 mở rộng bằng
/// [`Machine::with_harts`]).
///
/// Không còn là trần: `Machine::new()` giữ 1 hart để **không đổi ngữ nghĩa** 427 test cũ; SMP
/// 1/2/4/8 phải opt-in bằng `Machine::with_harts(n)` (K1C-PLAN §5 KT-29 rủi ro (d)).
pub const HART_COUNT: u16 = 1;

/// **KT-29** — trần số hart của mô hình `[TK]` (**D2-53 Q1/Q3**; thiết kế §4.16
/// `docs/v2/ViFastOs-v2-thiet-ke.md:425-428`: `MAX_HARTS` mặc định 8, trần cấu hình 64 là hằng
/// model để K3). K1c chỉ cần 1/2/4/8; `with_harts(n)` assert `1 ≤ n ≤ MAX_HARTS`.
pub const MAX_HARTS: u16 = 8;

/// `HartMask` mặc định: hart 0 (K1b 1 hart; TCB mới tạo home 0/affinity 0).
pub const AFFINITY_HOME: u64 = 1;

/// Số bucket của bảng băm WaitOnAddress — hằng số **model** `[TK]` (D2-53 Q3; K1C-PLAN §5
/// KT-28 DoD 1). `spec/kernel.toml` **chưa tồn tại** (K3), nên ngưỡng bucket ghi ở
/// `v2/docs/k1c-waitaddr.md` §1 chứ **không** phải `spec/`.
pub const WAITADDR_BUCKETS: usize = 64;

/// Bucket của khoá `(frame, offset)` trong bảng WaitOnAddress — hàm băm **tất định** `[TK]`:
/// `(frame.wrapping_mul(0x9E37_79B1) ^ offset) % WAITADDR_BUCKETS`.
///
/// `WAITADDR_BUCKETS` = 2^6 nên `% 64` chỉ lấy 6 bit thấp của phép XOR ⇒ hai offset lệch bội
/// 64 **va chạm có chủ ý** (test KT-28 dùng cặp `(f,0)`/`(f,64)`); khoá được so **đầy đủ**
/// `(frame, offset)` trong bucket nên va chạm không ảnh hưởng tìm/đánh thức.
pub fn waitaddr_bucket(frame: ObjId, offset: u32) -> usize {
    ((frame.wrapping_mul(0x9E37_79B1) ^ offset) as usize) % WAITADDR_BUCKETS
}

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
            ObjType::IrqControl
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

/// **KT-60** — một message đang chờ trong hàng gửi Endpoint (ảnh chụp đọc-only cho parity).
#[derive(Debug, Clone)]
pub struct QueuedView {
    pub sender: ObjId,
    pub msg: Option<Message>,
    pub call: bool,
    pub fault: Option<FaultKind>,
    pub blocking: bool,
}

/// **KT-27** — giá trị `Wait`/`Poll` trên Notification: **word 64 bit** + **badge** (thiết kế
/// §4.7 `:328`). Model-only `[TK]`, **không** phải ABI.
///
/// `Signal` OR badge vào word **và** badge (seL4: word = OR badge của các cap đã signal;
/// `Wait` trả word làm badge), nên hai trường bằng nhau — `check_notifications` kiểm bất biến
/// `word == badge`. `Wait`/`Poll` lấy giá trị xong **xoá** cả hai.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationValue {
    pub word: u64,
    pub badge: u64,
}

/// **KT-27** — `Message` trả cho `NBRecv` trên Notification (model-only `[TK]`): `MR0 = word`,
/// `badge = badge`, `label = 0`, `len = 1` (ABI `NBRecv` trả `value` + `MessageInfo` + `MR*`).
fn notification_message(v: NotificationValue) -> Message {
    let info = MessageInfo::new(0, 0, false, 0, 1).expect("MessageInfo hằng số hợp lệ");
    let mut msg = Message::new(info, &[v.word]).expect("Message 1 MR hợp lệ");
    msg.badge = v.badge;
    msg
}

// KT-18: `TcbState` + bảng `ALLOWED` 11×11 nằm ở `crate::tcb` (D2-49(d)) để mọi đường đổi
// trạng thái đi qua **một** cửa `Machine::transition_tcb`. Re-export để đường dẫn cũ
// (`vifast_sim::kobj::TcbState`, `::FaultKind`, `::SleepReason`) không đổi.
pub use crate::tcb::{
    BlockedOnKind, FaultAccess, FaultKind, SleepReason, SuspendMode, TcbSnapshot, TcbState,
    VmFaultReason, ALLOWED, TCB_STATE_COUNT,
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
    CNode {
        slots: Vec<Option<CapId>>,
    },
    Tcb(Box<TcbData>),
    SchedContext(Box<SchedData>),
    Endpoint(Box<EndpointState>),
    Reply(Box<ReplyData>),
    Notification(Box<NotificationState>),
    /// **KT-36** — mốc body của `Frame`: chỉ để `Meta.body` có chỗ (objmem cấp metadata vài
    /// chục byte), **không** mang vùng nhớ của Frame. Dữ liệu thật nằm ở [`Meta::frame`]
    /// (`FrameMeta`) vì nó phải đọc được **sau khi** object chết (DoD "frame chết sạch").
    Frame,
    /// **KT-37** — mốc body của `PageTable`: giống Frame, body chỉ là mốc vài chục byte; dữ
    /// liệu thật (512 PTE + cấp + `attached` + `mapped`) nằm ở `Machine::page_tables`, để đọc
    /// được sau khi object chết và không làm `size_of::<Kind>()` phình lên 4 KiB.
    PageTable,
    /// **KT-38** — mốc body của `VSpace`: metadata (asid/thế hệ/`active_on`) nằm ở
    /// `Machine::vspaces`; body chỉ là mốc vài chục byte trong arena kernel (VSpace **không**
    /// tiêu byte arena; root PageTable của nó mới tiêu một trang 4 KiB).
    VSpace,
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
    /// **KT-27** — `(word, badge)` của lần `Wait`/`Poll` Notification gần nhất giao cho TCB này.
    /// Model-only `[TK]`: ABI `Recv` trả `value` (word) + badge qua thanh ghi, nhưng mô hình
    /// gọi syscall **đồng bộ** nên nhánh "signal đánh thức waiter" cần chỗ ghi để test/procsrv
    /// đọc (`tcb_notification`/`take_tcb_notification`). `None` = chưa nhận lần nào.
    notif: Option<(u64, u64)>,
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

/// **KT-27** — Notification đúng ngữ nghĩa thiết kế §4.7 `:328` (QD §3.11): **word 64 bit**,
/// `Signal` **OR** badge + đánh thức **một** waiter + giao word rồi **xoá word**, `Wait`/`Poll`,
/// bound notification với TCB **và** `SchedContext`. Model-only `[TK]`, **không** phải ABI.
///
/// - `word`: OR của mọi badge đã `Signal` mà **chưa** được `Wait`/`Poll` lấy. Chỉ bị xoá khi
///   giao cho người nhận (bất biến `check_notifications`).
/// - `badge`: OR badge song song với `word` — mô hình giữ **cả hai** vì ABI `Recv` trả
///   `value` (word) **và** badge; theo seL4 hai giá trị này bằng nhau (`Signal` OR badge của
///   cap vào word, `Wait` trả word làm badge) nên bất biến `badge == word` được kiểm.
/// - `waiter`: **một** waiter (khác v1 "đánh thức tất cả" — QD §3.11); TCB chờ thứ hai ⇒ `Busy`.
/// - `bound_sc`: `SchedContext.Bind(sc, notification)` ghi quan hệ **1:1** (D2-53 Q6: model
///   API, **không** thêm chữ ký TCB vào ABI).
#[derive(Debug, Default)]
struct NotificationState {
    word: u64,
    badge: u64,
    waiter: Option<ObjId>,
    bound_sc: Option<ObjId>,
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
            Kind::Frame => 9,
            Kind::PageTable => 10,
            Kind::VSpace => 11,
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
    /// **KT-36** — metadata Frame (sống **sau** khi object chết: DoD "frame chết sạch" đọc lại
    /// bộ đếm). `Some` **đúng khi** `ty == ObjType::Frame` (bất biến `check_frames`).
    ///
    /// Với Frame, `offset`/`len` ở trên là **offset trong cửa sổ PA** và **cỡ Frame theo
    /// byte** (không phải byte arena — Frame không tiêu byte arena).
    frame: Option<FrameMeta>,
    /// **KT-36** — với object Untyped: cửa sổ PA ảo cấp cho Frame (`base` + watermark).
    /// Untyped con thừa hưởng `base_cha + offset_con`; `Revoke` Untyped đưa watermark về 0.
    frame_area: Option<FrameArea>,
    /// **KT-37** — chỉ số vào `Machine::page_tables` cho object `PageTable`; `Some` **đúng khi**
    /// `ty == ObjType::PageTable` (bất biến `check_pages`). Giống `Meta::frame`, dữ liệu PageTable
    /// phải đọc được sau khi object chết (bất biến "bảng chết không còn PTE").
    pt: Option<usize>,
    /// **KT-38** — chỉ số vào `Machine::vspaces` cho object `VSpace`; `Some` **đúng khi**
    /// `ty == ObjType::VSpace` (bất biến `check_vspace`). Giống `pt`, dữ liệu VSpace phải đọc
    /// được sau khi object chết (bất biến "VSpace chết không còn asid/active_on").
    space: Option<usize>,
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
    /// **KT-41** — số **sự kiện fault** đã đi vào đường fault (fault message tạo ra ở
    /// `inject_fault`/`exit`), dùng làm cửa sổ nhanh cho [`Machine::check_fault`]: mô hình chưa
    /// từng có fault ⇒ `Ok` ngay, giữ chi phí per-op của L1/test cũ. Không phải bộ đếm ABI.
    fault_events: u64,
    /// Log finalise TCB (KT-18 DoD 9), ghi trước khi body bị destroy trong objmem.
    finalise_log: Vec<FinaliseEntry>,
    /// **KT-28** — bảng băm waiter WaitOnAddress (thiết kế §4.7 `:330-334`, QD §3.8):
    /// `[Vec<(frame, offset, tcb)>; WAITADDR_BUCKETS]`, bucket =
    /// [`waitaddr_bucket`]. K1b chưa có object Frame (K2) nên khoá dùng `ObjId` thô của frame.
    address_waiters: [Vec<(ObjId, u32, ObjId)>; WAITADDR_BUCKETS],
    /// **KT-28** — giá trị "từ" tại `(frame, offset)` (mô hình thay vùng nhớ Frame của K2):
    /// [`Machine::set_addr_word`] ghi, [`Machine::wait_addr`] đọc **trong cùng một hàm** với
    /// lúc xếp hàng ("giữ khoá" của mô hình một luồng). Tra tuyến tính theo khoá đầy đủ; chưa
    /// ghi ⇒ 0 (như bộ nhớ mới).
    addr_words: Vec<(ObjId, u32, u64)>,
    /// Scheduler theo hart (KT-19): KT-19 dựng **1** hart (D2-49(a)); mỗi hart một
    /// [`HartSched`] (256 mức + bitmap) để K1c thêm hart/push migration mà không đổi API.
    sched: Vec<HartSched>,
    /// KT-22: hộp thư **theo hart** (thiết kế §4.17) — `HART_COUNT` = 1 ở K1b (D2-49(a)),
    /// mô hình tất định: `post_mail`/`process_mail`/`send_mail` (không atomic/thread/loom).
    /// **KT-29**: mỗi hart một hộp thư; `Migrate` chỉ hợp lệ khi có ≥ 2 hart.
    mail: Vec<HartMailbox>,
    /// **KT-29** — trường **công bố** `idle`/`cur_prio` của từng hart (thiết kế §4.8 `:338-342`,
    /// §4.16 luật 3): hart khác **đọc** được, chỉ hart đang chạy **ghi** được qua
    /// [`Machine::publish`]. Model một luồng: làm tươi từ `HartSched` sau mỗi op, nên
    /// `published.idle ⇔ current == None` là bất biến [`Machine::check_smp`].
    published: Vec<Published>,
    /// **KT-29** — hart "đang chạy" của mô hình tại thời điểm gọi (D2-53 Q1: mô hình tất định
    /// một luồng mô phỏng N hart). [`Machine::publish`] chỉ ghi được hart này (luật 3).
    current_hart: u16,
    /// **KT-29** — số TCB đã **di trú** thành công qua `MailboxMsg::Migrate` (nguồn test/đo).
    migrate_count: u64,
    /// Preempt **ép tại mọi public op** (thiết kế §4.13) — mặc định **tắt** để không đổi
    /// ngữ nghĩa KT-18; test KT-19 bật bằng [`Machine::set_preempt_every_step`].
    preempt_every_step: bool,
    /// KT-20: thời gian ảo (tick), **không** đọc `Instant`/wall clock (D2-49(h)).
    ///
    /// Đây là đồng hồ **ngân sách MCS** (charge `budget`/`period`), **không** phải đồng hồ wall
    /// mà `GetTime` đọc — đồng hồ wall là [`Machine::clock`] (KT-152/N1), có **nguồn theo hart**
    /// và max-update như v1. Hai đồng hồ tách nhau vì ngữ nghĩa khác nhau: `now` bị `tick` đẩy
    /// theo TCB đang chạy, còn đồng hồ wall phải đơn điệu qua migrate **và** qua `reset()` của
    /// một hart muộn (v1 T33a).
    now: u64,
    /// **KT-152 (N1)** — đồng hồ **wall monotonic** (một nguồn hệ thống + nguồn theo hart), mô
    /// hình cơ chế v1 T33a. Xem [`crate::clock`].
    clock: MonotonicClock,
    /// **KT-36** — chỉ số (0-based) của các Untyped có cửa sổ PA, để `retype_frame` kiểm chồng
    /// `pa` mà **không** quét toàn bộ `objects` (10.000 Frame trong một Untyped ⇒ O(1)/lần).
    /// Phần tử có thể trỏ Untyped đã chết — nơi dùng phải kiểm `alive`/`ty`.
    frame_windows: Vec<usize>,
    /// **KT-36** — số object `Frame` **đã từng** tạo. Dùng làm cửa sổ nhanh cho
    /// [`Machine::check_frames`] (chạy sau **mỗi** public op): mô hình chưa từng có Frame ⇒ trả
    /// `Ok` ngay, giữ chi phí per-op của L1/property K1a..K1c như trước KT-36 (đo được: L1 1.000
    /// seed 54,5 s → về mức baseline sau khi thêm cửa sổ này).
    frame_objects: usize,
    /// **KT-37** — dữ liệu PageTable theo chỉ số trong [`Meta::pt`] (tombstone `None` khi object
    /// chết); object PageTable **không** mang 4 KiB trong `Kind`.
    page_tables: Vec<Option<PageTableData>>,
    /// **KT-37** — số object `PageTable` **đã từng** tạo (cửa sổ nhanh cho `check_pages`, cùng
    /// lý do `frame_objects` của KT-36).
    page_table_objects: usize,
    /// **KT-38** — dữ liệu VSpace theo chỉ số trong [`Meta::space`] (tombstone `None` khi object
    /// chết): `(root PageTable, asid, generation, active_on)`.
    vspaces: Vec<Option<VSpaceData>>,
    /// **KT-38** — số object `VSpace` **đã từng** tạo (cửa sổ nhanh cho `check_vspace`).
    vspace_objects: usize,
    /// **KT-38** — thế hệ ASID **toàn cục** của mô hình (Q4): `+1` mỗi lần cạn ASID; mọi ASID cũ
    /// của thế hệ trước thành vô hiệu sau lệnh flush theo `gen` (KT-40 tiêu thụ).
    asid_generation: u64,
    /// **KT-38** — free-list của **thế hệ hiện tại**: các ASID **chưa từng** được cấp trong thế
    /// hệ này, giảm dần (`pop` ⇒ ASID thấp nhất). ASID đã cấp **không** quay lại free-list cho
    /// tới khi `generation += 1` ("không tái dùng trong cùng thế hệ" — `k2-vm.md` §12).
    asid_free: Vec<u16>,
    /// **KT-38** — số ASID **đã cấp** trong thế hệ hiện tại (cộng dồn; teardown **không** trả
    /// lại). Bất biến: `asid_free.len() + asid_issued == ASID_COUNT`.
    asid_issued: u32,
    /// **KT-38** — số lần cạn ASID (⇒ `generation += 1`) từ lúc mở `Machine`.
    asid_exhaustions: u64,
    /// **KT-38** — tập hart **online** (`⊆` `HartMask::for_harts(hart_count)`); `active_on` của
    /// mọi VSpace phải là tập con của mask này. Hart offline bị bỏ khỏi mọi `active_on`.
    hart_online: HartMask,
    /// **KT-38** — VSpace mỗi hart đang chạy (`None` = hart không nạp ASID nào). Bất biến:
    /// `hart_vspace[h] == Some(vs)` ⇔ bit `h` bật trong `active_on` của `vs`; mỗi hart tối đa
    /// một VSpace (một ASID hiện hành — như thanh ghi ASID của MMU).
    hart_vspace: Vec<Option<ObjId>>,
    /// **KT-38** — **lệnh flush**: teardown VSpace ⇒ một lệnh [`FlushScope::Asid`]; cạn ASID ⇒
    /// một lệnh [`FlushScope::Generation`] cho mọi hart online. **KT-40** tiêu thụ log này:
    /// mỗi lệnh cũng là một lần xoá entry TLB thật trên các hart liên quan.
    flush_log: Vec<FlushCmd>,
    /// **KT-40** — TLB của từng hart (16 entry, LRU tất định) — `k2-vm.md` §14.
    tlbs: Vec<HartTlb>,
    /// **KT-40** — shootdown **đang chờ ack** của mỗi hart (tối đa một; `tlb_shootdown_begin`).
    tlb_pending: Vec<Option<PendingShootdown>>,
    /// **KT-40** — shootdown gần nhất đã kết thúc (thành công hoặc quá hạn) — nguồn của bất biến
    /// "`unmap`/`protect` trả `Ok` ⇒ `acked == targets`".
    last_tlb_shootdown: Option<TlbShootdownInfo>,
    /// **KT-40** — số hiệu shootdown toàn cục (1-based, tăng đơn điệu).
    tlb_shootdown_seq: u64,
    /// **KT-40** — số lệnh shootdown đã phát (kể cả `active_on` rỗng: lệnh vẫn tồn tại).
    tlb_shootdown_count: u64,
    /// **KT-40** — tổng số `TlbFlush` đã giao thành công vào các hộp thư.
    tlb_msg_total: u64,
    /// **KT-40** — tổng số ack **mới** ghi nhận (`fetch_max` delta).
    tlb_ack_total: u64,
    /// **KT-40** — tổng số entry đã nạp từ walk (cửa sổ nhanh cho [`Machine::check_tlb`]).
    tlb_fill_total: u64,
    /// **KT-40** — số lần vòng chờ ack hết hạn (chẩn đoán; >0 nghĩa là ack không tới).
    tlb_wait_timeouts: u64,
}

/// **KT-40** — một shootdown **đang chờ ack** của một hart (model-only `[TK]`).
///
/// `acks` giữ `(hart, ack_seq)` của từng message đã giao: `ack_seq` = `delivered` của hart
/// đích **sau** khi giao (Q5: "`seq` = số send Ok"). Ack đủ ⇔ mọi hart đích có `acked >=
/// ack_seq`; khi đó `acked_mask == targets` (bất biến của `unmap`/`protect`).
#[derive(Debug, Clone)]
struct PendingShootdown {
    /// Số hiệu toàn cục (gán lúc post).
    seq: u64,
    /// Thế hệ ASID của VSpace lúc phát.
    generation: u64,
    /// ASID của VSpace lúc phát.
    asid: u16,
    /// VA gốc bị đổi PTE.
    va: u64,
    /// Mask hart **phải** ack (`= active_on` lúc phát, đã `⊆` hart online).
    targets: HartMask,
    /// Mask hart **ack ngầm** vì không cần message: hart **gọi** (nếu nó thuộc `active_on`) —
    /// nó flush cục bộ đồng bộ nên không IPI cho chính mình (D2-68; xem `k2-vm.md` §14.3).
    implicit: HartMask,
    /// `(hart đích, thế hệ ack đã giao)` — thứ tự tăng dần theo hart.
    acks: Vec<(u16, u64)>,
    /// Số `TlbFlush` đã giao.
    messages: u32,
    /// Số vòng chờ đã chạy (chẩn đoán deadlock).
    rounds: u64,
    /// Đã post message chưa (`false` trong cửa sổ "pending trước khi đổi PTE").
    posted: bool,
}

impl PendingShootdown {
    /// Ảnh chụp cho [`TlbShootdownInfo`] — nguồn của bất biến `Ok ⇒ acked == targets`.
    fn info(&self, acked: HartMask, completed: bool) -> TlbShootdownInfo {
        TlbShootdownInfo {
            seq: self.seq,
            generation: self.generation,
            asid: self.asid,
            va: self.va,
            targets: self.targets,
            acked,
            messages: self.messages,
            rounds: self.rounds,
            completed,
        }
    }
}

/// Vị trí/cỡ của object trong mô hình — gom tham số `push_meta`/`push_kernel`.
#[derive(Debug, Default, Clone, Copy)]
struct ChoDat {
    untyped: Option<usize>,
    offset: usize,
    len: usize,
    region: Option<usize>,
    slots: Option<usize>,
    /// **KT-36** — metadata Frame (chỉ Frame mới có).
    frame: Option<FrameMeta>,
    /// **KT-36** — cửa sổ PA cấp Frame (chỉ Untyped mới có).
    frame_area: Option<FrameArea>,
    /// **KT-37** — chỉ số `Machine::page_tables` (chỉ PageTable mới có).
    pt: Option<usize>,
    /// **KT-38** — chỉ số `Machine::vspaces` (chỉ VSpace mới có).
    space: Option<usize>,
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

mod budget;
mod caps;
mod clock;
mod fault;
mod ipc_ops;
mod notification;
mod object;
mod pagetable;
mod retype;
mod sched;
mod tcb;
mod tlb;
mod vspace;

#[cfg(test)]
mod tests;
