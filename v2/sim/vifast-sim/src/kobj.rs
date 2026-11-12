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
//!   `untyped_min_bytes` = 4096, căn lề = cỡ.
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

use std::collections::VecDeque;

use vifast_objmem::{Dead, KObj, ObjRef, ObjStore, OwnedMem};

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

/// Nguyên nhân fault mang theo trạng thái chặn (D2-25: Exit mang mã thoát).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    Exit { code: u64 },
}

/// Lý do `Sleeping` — variants của `spec/[method_signature.state.Sleeping]` (D2-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepReason {
    Timeout,
    Budget,
}

impl SleepReason {
    pub fn name(self) -> &'static str {
        match self {
            SleepReason::Timeout => "Timeout",
            SleepReason::Budget => "Budget",
        }
    }
}

/// Trạng thái TCB — **đủ 11 tên** của `spec/[method_signature.state]` (D2-33);
/// unit test `tcb_state_du_11_ten_theo_spec` so trực tiếp với `vifast_abi::TCB_STATES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcbState {
    Inactive,
    Ready,
    Running {
        hart: u16,
    },
    BlockedOnSend {
        ep: ObjId,
        call: bool,
        fault: Option<FaultKind>,
    },
    BlockedOnRecv {
        ep: ObjId,
    },
    BlockedOnReply {
        reply: ObjId,
    },
    BlockedOnNotification {
        ntfn: ObjId,
    },
    BlockedOnAddress {
        frame: ObjId,
        offset: u64,
    },
    Sleeping {
        reason: SleepReason,
    },
    Stopping,
    Dead,
}

impl TcbState {
    /// Tên trạng thái (không kèm trường) — khớp `vifast_abi::TCB_STATES`.
    pub fn name(&self) -> &'static str {
        match self {
            TcbState::Inactive => "Inactive",
            TcbState::Ready => "Ready",
            TcbState::Running { .. } => "Running",
            TcbState::BlockedOnSend { .. } => "BlockedOnSend",
            TcbState::BlockedOnRecv { .. } => "BlockedOnRecv",
            TcbState::BlockedOnReply { .. } => "BlockedOnReply",
            TcbState::BlockedOnNotification { .. } => "BlockedOnNotification",
            TcbState::BlockedOnAddress { .. } => "BlockedOnAddress",
            TcbState::Sleeping { .. } => "Sleeping",
            TcbState::Stopping => "Stopping",
            TcbState::Dead => "Dead",
        }
    }

    /// Chỉ `Inactive`/`Ready`/`Running` là chạy được; còn lại là chặn/ngủ/dừng/chết.
    pub fn is_runnable(&self) -> bool {
        matches!(
            self,
            TcbState::Inactive | TcbState::Ready | TcbState::Running { .. }
        )
    }

    pub fn render(&self) -> String {
        match self {
            TcbState::Inactive => "Inactive".to_string(),
            TcbState::Ready => "Ready".to_string(),
            TcbState::Running { hart } => format!("Running{{hart={hart}}}"),
            TcbState::BlockedOnSend { ep, call, fault } => {
                let fault = match fault {
                    None => "none".to_string(),
                    Some(FaultKind::Exit { code }) => format!("Exit{{{code}}}"),
                };
                format!("BlockedOnSend{{ep={ep}, call={call}, fault={fault}}}")
            }
            TcbState::BlockedOnRecv { ep } => format!("BlockedOnRecv{{ep={ep}}}"),
            TcbState::BlockedOnReply { reply } => format!("BlockedOnReply{{reply={reply}}}"),
            TcbState::BlockedOnNotification { ntfn } => {
                format!("BlockedOnNotification{{ntfn={ntfn}}}")
            }
            TcbState::BlockedOnAddress { frame, offset } => {
                format!("BlockedOnAddress{{frame={frame}, offset={offset}}}")
            }
            TcbState::Sleeping { reason } => format!("Sleeping{{reason={}}}", reason.name()),
            TcbState::Stopping => "Stopping".to_string(),
            TcbState::Dead => "Dead".to_string(),
        }
    }
}

#[derive(Debug, Default)]
struct EndpointState {
    recv: Option<(ObjId, Option<CapId>)>,
    send: VecDeque<(ObjId, Option<FaultKind>)>,
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
    Plain,
}

#[derive(Debug)]
struct TcbData {
    state: TcbState,
    fault_ep: Option<CapId>,
    cspace: Option<ObjId>,
    exit_code: Option<u64>,
}

#[derive(Debug)]
struct SchedData {
    bound: Option<CapId>,
    budget: u64,
    period: u64,
}

#[derive(Debug)]
struct ReplyData {
    caller: Option<ObjId>,
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
        self.clear_slot_of(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.alive = false;
            c.parent = None;
        }
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
        Ok(deleted)
    }

    // ---------------------------------------------------------------- cấu hình

    pub fn tcb_configure(&mut self, tcb: SlotRef, cspace: Option<SlotRef>) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let cspace_obj = match cspace {
            Some(s) => Some(self.require_type(s, ObjType::CNode)?),
            None => None,
        };
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.cspace = cspace_obj;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
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
        })
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
        self.body_mut(sched_obj, |k| match k {
            Kind::SchedContext(s) => {
                s.budget = budget;
                s.period = period;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

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
        })
    }

    // ---------------------------------------------------------------- IPC

    pub fn tcb_state(&self, tcb: ObjId) -> Result<TcbState, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.state),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn tcb_read_state(&self, tcb: SlotRef) -> Result<TcbState, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        self.tcb_state(obj)
    }

    /// `Call` trên Endpoint: chặn chờ reply; nếu có server đang `Recv` thì giao ngay.
    pub fn call(&mut self, tcb: ObjId, ep: SlotRef, mr: u64) -> Result<(), KError> {
        let _ = self.tcb_state(tcb)?;
        let ep_obj = self.require_type(ep, ObjType::Endpoint)?;
        let waiter = self.body_mut(ep_obj, |k| match k {
            Kind::Endpoint(e) => {
                e.last_mr = Some(mr);
                Ok(e.recv.take())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        if let Some((server, reply)) = waiter {
            self.set_tcb_state(server, TcbState::Ready)?;
            let reply_cap = reply.ok_or(KError::IllegalOperation)?;
            let reply_obj = self.require_cap_type(reply_cap, ObjType::Reply)?;
            self.reply_set_caller(reply_obj, Some(tcb))?;
            self.set_tcb_state(tcb, TcbState::BlockedOnReply { reply: reply_obj })?;
        } else {
            self.body_mut(ep_obj, |k| match k {
                Kind::Endpoint(e) => {
                    e.send.push_back((tcb, None));
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            self.set_tcb_state(
                tcb,
                TcbState::BlockedOnSend {
                    ep: ep_obj,
                    call: true,
                    fault: None,
                },
            )?;
        }
        Ok(())
    }

    /// `Recv` trên Endpoint: lấy message đang chờ, hoặc chặn `BlockedOnRecv`.
    pub fn recv(&mut self, tcb: ObjId, ep: SlotRef, reply: Option<SlotRef>) -> Result<(), KError> {
        let _ = self.tcb_state(tcb)?;
        let ep_obj = self.require_type(ep, ObjType::Endpoint)?;
        let reply_cap = match reply {
            Some(r) => Some(self.cap_at(r.cnode, r.slot)?),
            None => None,
        };
        let sender = self.body_mut(ep_obj, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.pop_front()),
            _ => Err(KError::WrongObjectType),
        })?;
        match sender {
            Some((sender, fault)) => {
                match fault {
                    // Fault Exit: procsrv Recv xong mới Inactive (QD §1 Q-A).
                    Some(FaultKind::Exit { .. }) => {
                        self.set_tcb_state(sender, TcbState::Inactive)?;
                    }
                    None => match reply_cap {
                        Some(rc) => {
                            let reply_obj = self.require_cap_type(rc, ObjType::Reply)?;
                            self.reply_set_caller(reply_obj, Some(sender))?;
                            self.set_tcb_state(
                                sender,
                                TcbState::BlockedOnReply { reply: reply_obj },
                            )?;
                        }
                        None => self.set_tcb_state(sender, TcbState::Ready)?,
                    },
                }
                self.set_tcb_state(tcb, TcbState::Ready)?;
            }
            None => {
                self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => {
                        e.recv = Some((tcb, reply_cap));
                        Ok(())
                    }
                    _ => Err(KError::WrongObjectType),
                })?;
                self.set_tcb_state(tcb, TcbState::BlockedOnRecv { ep: ep_obj })?;
            }
        }
        Ok(())
    }

    /// `Send` trên Reply: đánh thức người gọi đang chờ.
    pub fn reply(&mut self, tcb: ObjId, reply: SlotRef) -> Result<(), KError> {
        let _ = self.tcb_state(tcb)?;
        let reply_obj = self.require_type(reply, ObjType::Reply)?;
        let caller = self.body_mut(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller.take()),
            _ => Err(KError::WrongObjectType),
        })?;
        let caller = caller.ok_or(KError::IllegalOperation)?;
        self.set_tcb_state(caller, TcbState::Ready)?;
        self.set_tcb_state(tcb, TcbState::Ready)?;
        Ok(())
    }

    /// `Exit`: có `fault_ep` ⇒ `BlockedOnSend{call=false, fault=Exit{code}}`; không ⇒ `Inactive`.
    pub fn exit(&mut self, tcb: ObjId, code: u64) -> Result<(), KError> {
        let fault_ep = self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.exit_code = Some(code);
                Ok(t.fault_ep)
            }
            _ => Err(KError::WrongObjectType),
        })?;
        match fault_ep {
            Some(cap) => {
                let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
                self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => {
                        e.send.push_back((tcb, Some(FaultKind::Exit { code })));
                        Ok(())
                    }
                    _ => Err(KError::WrongObjectType),
                })?;
                self.set_tcb_state(
                    tcb,
                    TcbState::BlockedOnSend {
                        ep: ep_obj,
                        call: false,
                        fault: Some(FaultKind::Exit { code }),
                    },
                )?;
            }
            None => self.set_tcb_state(tcb, TcbState::Inactive)?,
        }
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
                fault_ep: None,
                cspace: None,
                exit_code: None,
            })),
            ObjType::SchedContext => Kind::SchedContext(Box::new(SchedData {
                bound: None,
                budget: 0,
                period: 0,
            })),
            ObjType::Endpoint => Kind::Endpoint(Box::default()),
            ObjType::Reply => Kind::Reply(Box::new(ReplyData { caller: None })),
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

    fn set_tcb_state(&mut self, tcb: ObjId, state: TcbState) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.state = state;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
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
        let mut deleted = 0;
        for cap in doomed {
            if self.caps[cap as usize - 1].alive {
                self.clear_slot_of(cap);
                self.caps[cap as usize - 1].alive = false;
                deleted += 1;
            }
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
        for (i, alive) in alive_objs.iter().enumerate() {
            if !*alive {
                continue;
            }
            let obj = (i + 1) as ObjId;
            let _ = self.body_mut(obj, |k| {
                match k {
                    Kind::Endpoint(e) => {
                        if let Some((tcb, _)) = e.recv {
                            if !is_alive(tcb) {
                                e.recv = None;
                            }
                        }
                        e.send.retain(|(tcb, _)| is_alive(*tcb));
                    }
                    Kind::Reply(r) => {
                        if let Some(c) = r.caller {
                            if !is_alive(c) {
                                r.caller = None;
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
                    }
                    _ => {}
                }
                Ok(())
            });
        }
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
            TcbState::BlockedOnRecv { ep: 1 },
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
