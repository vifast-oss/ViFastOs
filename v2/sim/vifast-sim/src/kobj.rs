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

use std::collections::VecDeque;

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

#[derive(Debug)]
enum Kind {
    Untyped {
        size: usize,
        watermark: usize,
    },
    CNode {
        slots: Vec<Option<CapId>>,
    },
    Tcb {
        state: TcbState,
        fault_ep: Option<CapId>,
        cspace: Option<ObjId>,
        exit_code: Option<u64>,
    },
    SchedContext {
        bound: Option<CapId>,
        budget: u64,
        period: u64,
    },
    Endpoint(EndpointState),
    Reply {
        caller: Option<ObjId>,
    },
    Plain,
}

#[derive(Debug)]
struct Object {
    ty: ObjType,
    alive: bool,
    untyped: Option<usize>,
    offset: usize,
    len: usize,
    kind: Kind,
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

/// Mô hình kernel tối giản: object + cap + IPC tối thiểu cho KT-09.
#[derive(Debug)]
pub struct Machine {
    objects: Vec<Object>,
    caps: Vec<Cap>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

impl Machine {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            caps: Vec::new(),
        }
    }

    // ---------------------------------------------------------------- dựng cảnh

    /// Thêm một Untyped gốc. `base` chỉ để kịch bản ghi nhớ; mô hình tính offset từ 0.
    pub fn add_untyped(&mut self, base: u64, size: usize) -> ObjId {
        let _ = base;
        assert!(size > 0, "Untyped phải có cỡ dương");
        self.push_object(
            ObjType::Untyped,
            Kind::Untyped { size, watermark: 0 },
            None,
            0,
            0,
        )
    }

    /// Thêm CNode do harness cấp (init giữ CNode gốc — không cần Retype CNode ở KT-09).
    pub fn add_cnode(&mut self, slots: usize) -> ObjId {
        assert!(slots > 0, "CNode phải có ít nhất một slot");
        self.push_object(
            ObjType::CNode,
            Kind::CNode {
                slots: vec![None; slots],
            },
            None,
            0,
            0,
        )
    }

    /// Thêm object lõi do harness cấp (SchedControl, TCB khởi tạo, ...) — không tính byte Untyped.
    pub fn add_kernel_object(&mut self, ty: ObjType) -> ObjId {
        assert!(ty.is_core(), "chỉ object lõi mới dựng trực tiếp");
        self.push_object(ty, Self::fresh_kind(ty, 0), None, 0, 0)
    }

    /// Cap gốc cho một object (không có cha) — harness đặt vào CNode bằng `put_cap`.
    pub fn root_cap(&mut self, obj: ObjId) -> CapId {
        let ty = self
            .objects
            .get(obj as usize - 1)
            .map(|o| o.ty)
            .expect("root_cap: object phải tồn tại");
        self.push_cap(obj, ty, None, RIGHTS_ALL, None)
    }

    pub fn put_cap(&mut self, cnode: ObjId, slot: usize, cap: CapId) -> Result<(), KError> {
        self.check_cnode_slot(cnode, slot)?;
        let cap_ref = self.cap(cap)?;
        if cap_ref.slot.is_some() {
            return Err(KError::IllegalOperation);
        }
        let slots = self.cnode_slots_mut(cnode)?;
        if slots[slot].is_some() {
            return Err(KError::SlotOccupied);
        }
        slots[slot] = Some(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(SlotRef::new(cnode, slot));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- truy vấn

    pub fn cap_at(&self, cnode: ObjId, slot: usize) -> Result<CapId, KError> {
        let slots = self.cnode_slots(cnode)?;
        if slot >= slots.len() {
            return Err(KError::RangeError);
        }
        match slots[slot] {
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
        Ok(self.cnode_slots(cnode)?.len())
    }

    pub fn object_alive(&self, obj: ObjId) -> bool {
        self.objects
            .get(obj as usize - 1)
            .map(|o| o.alive)
            .unwrap_or(false)
    }

    pub fn object_type(&self, obj: ObjId) -> Option<ObjType> {
        self.objects.get(obj as usize - 1).map(|o| o.ty)
    }

    /// `(offset, len)` trong Untyped; `None` với object không cấp từ Untyped.
    pub fn object_bytes(&self, obj: ObjId) -> Option<(usize, usize)> {
        let o = self.objects.get(obj as usize - 1)?;
        o.untyped.map(|_| (o.offset, o.len))
    }

    pub fn untyped_size(&self, obj: ObjId) -> usize {
        match self.objects.get(obj as usize - 1).map(|o| &o.kind) {
            Some(Kind::Untyped { size, .. }) => *size,
            _ => 0,
        }
    }

    pub fn untyped_used(&self, obj: ObjId) -> usize {
        match self.objects.get(obj as usize - 1).map(|o| &o.kind) {
            Some(Kind::Untyped { watermark, .. }) => *watermark,
            _ => 0,
        }
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
            .filter(|o| o.alive && o.untyped == Some(idx))
            .map(|o| o.len)
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
            .filter(|o| o.alive)
            .map(|o| match &o.kind {
                Kind::Untyped { watermark, .. } => *watermark,
                _ => 0,
            })
            .sum()
    }

    // ---------------------------------------------------------------- bất biến

    /// Bất biến tổng byte của một Untyped: không chồng lấn, không vượt biên, không đếm hai lần.
    pub fn check_bytes(&self, untyped: ObjId) -> Result<(), String> {
        let idx = self
            .objects
            .get(untyped as usize - 1)
            .map(|_| untyped as usize - 1)
            .ok_or_else(|| format!("Untyped {untyped} không tồn tại"))?;
        let (size, watermark) = match &self.objects[idx].kind {
            Kind::Untyped { size, watermark } => (*size, *watermark),
            _ => return Err(format!("object {untyped} không phải Untyped")),
        };
        if watermark > size {
            return Err(format!("watermark {watermark} > cỡ {size}"));
        }
        let mut ranges: Vec<(usize, usize, ObjId)> = Vec::new();
        let mut allocated = 0usize;
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.untyped != Some(idx) {
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
        for (i, o) in self.objects.iter().enumerate() {
            if o.alive && o.ty == ObjType::Untyped {
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
            if !self.objects[c.obj as usize - 1].alive {
                return Err(format!("cap {cap} trỏ object {} đã chết", c.obj));
            }
            if self.objects[c.obj as usize - 1].ty != c.ty {
                return Err(format!(
                    "cap {cap} kiểu {:?} nhưng object {} kiểu {:?}",
                    c.ty,
                    c.obj,
                    self.objects[c.obj as usize - 1].ty
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
            let Kind::CNode { slots } = &o.kind else {
                return Err(format!("object {cnode} kiểu CNode nhưng kind lạ"));
            };
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
                    let slots = self
                        .cnode_slots(cnode)
                        .map_err(|e| format!("cap {cap}: CNode {cnode}: {e:?}"))?;
                    if slot >= slots.len() {
                        return Err(format!(
                            "cap {cap} ở slot {slot} ngoài dải của CNode {cnode}"
                        ));
                    }
                    if slots[slot] != Some(cap) {
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
    pub fn retype(
        &mut self,
        src: SlotRef,
        ty: ObjType,
        size_bits: u32,
        flags: u32,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (u_idx, size, watermark, src_rights) = {
            let c = self.cap(src_cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let idx = self.object_index(c.obj)?;
            match &self.objects[idx].kind {
                Kind::Untyped { size, watermark } => (idx, *size, *watermark, c.rights),
                _ => return Err(KError::WrongObjectType),
            }
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
                if len > size {
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

        let start = align_up(watermark, align);
        let end = start.checked_add(len).ok_or(KError::InvalidArgument)?;
        if end > size {
            return Err(KError::NotEnoughMemory);
        }

        let obj = self.push_object(
            ty,
            Self::fresh_kind(ty, child_slots),
            Some(u_idx),
            start,
            len,
        );
        if ty == ObjType::Untyped {
            // Untyped con có cỡ riêng = vùng vừa cắt (fresh_kind dùng 0 cho object chưa cắt).
            if let Kind::Untyped { size: s, .. } = &mut self.objects[obj as usize - 1].kind {
                *s = len;
            }
        }
        if let Kind::Untyped { watermark, .. } = &mut self.objects[u_idx].kind {
            *watermark = end;
        }
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
        if self.cnode_slots(dst.cnode)?[dst.slot].is_some() {
            return Err(KError::SlotOccupied);
        }
        let new_cap = self.push_cap(obj, ty, Some(src_cap), rights, badge);
        self.cnode_slots_mut(dst.cnode)?[dst.slot] = Some(new_cap);
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
            let slots = self.cnode_slots(cnode)?;
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
        match &mut self.object_mut(tcb_obj)?.kind {
            Kind::Tcb { cspace: c, .. } => {
                *c = cspace_obj;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    pub fn tcb_set_fault_ep(&mut self, tcb: SlotRef, ep: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ep_cap = self.cap_at(ep.cnode, ep.slot)?;
        if self.cap(ep_cap)?.ty != ObjType::Endpoint {
            return Err(KError::WrongObjectType);
        }
        match &mut self.object_mut(tcb_obj)?.kind {
            Kind::Tcb { fault_ep, .. } => {
                *fault_ep = Some(ep_cap);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        }
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
        match &mut self.object_mut(sched_obj)?.kind {
            Kind::SchedContext {
                budget: b,
                period: p,
                ..
            } => {
                *b = budget;
                *p = period;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    pub fn sched_context_bind(&mut self, sched: SlotRef, target: SlotRef) -> Result<(), KError> {
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        let target_cap = self.cap_at(target.cnode, target.slot)?;
        let target_ty = self.cap(target_cap)?.ty;
        if !matches!(target_ty, ObjType::Reply | ObjType::Notification) {
            return Err(KError::WrongObjectType);
        }
        match &mut self.object_mut(sched_obj)?.kind {
            Kind::SchedContext { bound, .. } => {
                *bound = Some(target_cap);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    // ---------------------------------------------------------------- IPC

    pub fn tcb_state(&self, tcb: ObjId) -> Result<TcbState, KError> {
        match &self.object(tcb)?.kind {
            Kind::Tcb { state, .. } => Ok(*state),
            _ => Err(KError::WrongObjectType),
        }
    }

    pub fn tcb_read_state(&self, tcb: SlotRef) -> Result<TcbState, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        self.tcb_state(obj)
    }

    /// `Call` trên Endpoint: chặn chờ reply; nếu có server đang `Recv` thì giao ngay.
    pub fn call(&mut self, tcb: ObjId, ep: SlotRef, mr: u64) -> Result<(), KError> {
        let _ = self.tcb_state(tcb)?;
        let ep_obj = self.require_type(ep, ObjType::Endpoint)?;
        let waiter = match &mut self.object_mut(ep_obj)?.kind {
            Kind::Endpoint(e) => {
                e.last_mr = Some(mr);
                e.recv.take()
            }
            _ => return Err(KError::WrongObjectType),
        };
        if let Some((server, reply)) = waiter {
            self.set_tcb_state(server, TcbState::Ready)?;
            let reply_cap = reply.ok_or(KError::IllegalOperation)?;
            let reply_obj = self.require_cap_type(reply_cap, ObjType::Reply)?;
            self.reply_set_caller(reply_obj, Some(tcb))?;
            self.set_tcb_state(tcb, TcbState::BlockedOnReply { reply: reply_obj })?;
        } else {
            match &mut self.object_mut(ep_obj)?.kind {
                Kind::Endpoint(e) => e.send.push_back((tcb, None)),
                _ => return Err(KError::WrongObjectType),
            }
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
        let sender = match &mut self.object_mut(ep_obj)?.kind {
            Kind::Endpoint(e) => e.send.pop_front(),
            _ => return Err(KError::WrongObjectType),
        };
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
                match &mut self.object_mut(ep_obj)?.kind {
                    Kind::Endpoint(e) => e.recv = Some((tcb, reply_cap)),
                    _ => return Err(KError::WrongObjectType),
                }
                self.set_tcb_state(tcb, TcbState::BlockedOnRecv { ep: ep_obj })?;
            }
        }
        Ok(())
    }

    /// `Send` trên Reply: đánh thức người gọi đang chờ.
    pub fn reply(&mut self, tcb: ObjId, reply: SlotRef) -> Result<(), KError> {
        let _ = self.tcb_state(tcb)?;
        let reply_obj = self.require_type(reply, ObjType::Reply)?;
        let caller = match &mut self.object_mut(reply_obj)?.kind {
            Kind::Reply { caller } => caller.take(),
            _ => return Err(KError::WrongObjectType),
        };
        let caller = caller.ok_or(KError::IllegalOperation)?;
        self.set_tcb_state(caller, TcbState::Ready)?;
        self.set_tcb_state(tcb, TcbState::Ready)?;
        Ok(())
    }

    /// `Exit`: có `fault_ep` ⇒ `BlockedOnSend{call=false, fault=Exit{code}}`; không ⇒ `Inactive`.
    pub fn exit(&mut self, tcb: ObjId, code: u64) -> Result<(), KError> {
        let fault_ep = match &mut self.object_mut(tcb)?.kind {
            Kind::Tcb {
                fault_ep,
                exit_code,
                ..
            } => {
                *exit_code = Some(code);
                *fault_ep
            }
            _ => return Err(KError::WrongObjectType),
        };
        match fault_ep {
            Some(cap) => {
                let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
                match &mut self.object_mut(ep_obj)?.kind {
                    Kind::Endpoint(e) => {
                        e.send.push_back((tcb, Some(FaultKind::Exit { code })));
                    }
                    _ => return Err(KError::WrongObjectType),
                }
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

    fn push_object(
        &mut self,
        ty: ObjType,
        kind: Kind,
        untyped: Option<usize>,
        offset: usize,
        len: usize,
    ) -> ObjId {
        self.objects.push(Object {
            ty,
            alive: true,
            untyped,
            offset,
            len,
            kind,
        });
        self.objects.len() as ObjId
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
            ObjType::Untyped => Kind::Untyped {
                size: 0,
                watermark: 0,
            },
            ObjType::CNode => Kind::CNode {
                slots: vec![None; child_slots.max(1)],
            },
            ObjType::Tcb => Kind::Tcb {
                state: TcbState::Inactive,
                fault_ep: None,
                cspace: None,
                exit_code: None,
            },
            ObjType::SchedContext => Kind::SchedContext {
                bound: None,
                budget: 0,
                period: 0,
            },
            ObjType::Endpoint => Kind::Endpoint(EndpointState::default()),
            ObjType::Reply => Kind::Reply { caller: None },
            _ => Kind::Plain,
        }
    }

    fn object_index(&self, obj: ObjId) -> Result<usize, KError> {
        if obj == 0 || obj as usize > self.objects.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(obj as usize - 1)
    }

    fn object(&self, obj: ObjId) -> Result<&Object, KError> {
        Ok(&self.objects[self.object_index(obj)?])
    }

    fn object_mut(&mut self, obj: ObjId) -> Result<&mut Object, KError> {
        let idx = self.object_index(obj)?;
        Ok(&mut self.objects[idx])
    }

    fn cap(&self, cap: CapId) -> Result<&Cap, KError> {
        if cap == 0 || cap as usize > self.caps.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(&self.caps[cap as usize - 1])
    }

    fn cnode_slots(&self, cnode: ObjId) -> Result<&[Option<CapId>], KError> {
        match &self.object(cnode)?.kind {
            Kind::CNode { slots } => Ok(slots),
            _ => Err(KError::WrongObjectType),
        }
    }

    fn cnode_slots_mut(&mut self, cnode: ObjId) -> Result<&mut [Option<CapId>], KError> {
        match &mut self.object_mut(cnode)?.kind {
            Kind::CNode { slots } => Ok(slots),
            _ => Err(KError::WrongObjectType),
        }
    }

    fn check_cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<(), KError> {
        let slots = self.cnode_slots(cnode)?;
        if slot >= slots.len() {
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
        match &mut self.object_mut(tcb)?.kind {
            Kind::Tcb { state: s, .. } => {
                *s = state;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    fn reply_set_caller(&mut self, reply: ObjId, caller: Option<ObjId>) -> Result<(), KError> {
        match &mut self.object_mut(reply)?.kind {
            Kind::Reply { caller: c } => {
                *c = caller;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    fn clear_slot_of(&mut self, cap: CapId) {
        let slot = self
            .caps
            .get_mut(cap as usize - 1)
            .and_then(|c| c.slot.take());
        if let Some(SlotRef { cnode, slot: s }) = slot {
            if let Ok(slots) = self.cnode_slots_mut(cnode) {
                if slots.get(s).copied().flatten() == Some(cap) {
                    slots[s] = None;
                }
            }
        }
    }

    /// Xoá cap con/cháu của `root`; nếu `root` là cap Untyped thì finalise object + reset watermark.
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
            // Watermark của chính Untyped gốc về 0 (thiết kế §4.2) — con/cháu về 0 ở dưới.
            if let Kind::Untyped { watermark, .. } = &mut self.objects[u_idx].kind {
                *watermark = 0;
            }
            for id in &dead_objs {
                let idx = *id as usize - 1;
                if let Kind::Untyped { watermark, .. } = &mut self.objects[idx].kind {
                    *watermark = 0;
                }
                self.objects[idx].alive = false;
            }
            for i in 1..=self.caps.len() as CapId {
                if self.caps[i as usize - 1].alive
                    && dead_objs.contains(&self.caps[i as usize - 1].obj)
                {
                    doomed.push(i);
                }
            }
            // Finalise CNode (seL4 `finaliseCNode`): cap nằm trong slot của CNode chết phải chết
            // theo **cả cây con của nó**, kể cả khi object đích còn sống (object ngoài Untyped).
            let mut cnode_caps: Vec<CapId> = Vec::new();
            for id in &dead_objs {
                let idx = *id as usize - 1;
                if self.objects[idx].ty != ObjType::CNode {
                    continue;
                }
                if let Kind::CNode { slots } = &self.objects[idx].kind {
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
                let idx = *id as usize - 1;
                if let Kind::CNode { slots } = &mut self.objects[idx].kind {
                    slots.iter_mut().for_each(|s| *s = None);
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
        for (i, o) in self.objects.iter().enumerate() {
            if o.alive && o.untyped == Some(u_idx) {
                out.push((i as u32) + 1);
                if o.ty == ObjType::Untyped {
                    self.collect_untyped_tree(i, out);
                }
            }
        }
    }

    /// Bỏ tham chiếu tới object chết (thiết kế §4.2: không trường nào trỏ tới object chết).
    fn purge_dead_refs(&mut self) {
        let alive: Vec<bool> = self.objects.iter().map(|o| o.alive).collect();
        let is_alive = |id: ObjId| -> bool {
            id > 0 && (id as usize) <= alive.len() && alive[id as usize - 1]
        };
        for o in &mut self.objects {
            if !o.alive {
                continue;
            }
            match &mut o.kind {
                Kind::Endpoint(e) => {
                    if let Some((tcb, _)) = e.recv {
                        if !is_alive(tcb) {
                            e.recv = None;
                        }
                    }
                    e.send.retain(|(tcb, _)| is_alive(*tcb));
                }
                Kind::Reply { caller } => {
                    if let Some(c) = caller {
                        if !is_alive(*c) {
                            *caller = None;
                        }
                    }
                }
                Kind::Tcb {
                    fault_ep, cspace, ..
                } => {
                    if let Some(cap) = fault_ep {
                        if !self
                            .caps
                            .get(*cap as usize - 1)
                            .map(|c| c.alive)
                            .unwrap_or(false)
                        {
                            *fault_ep = None;
                        }
                    }
                    if let Some(c) = cspace {
                        if !is_alive(*c) {
                            *cspace = None;
                        }
                    }
                }
                _ => {}
            }
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
        m.cnode_slots_mut(a).expect("CNode")[0] = None;
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
