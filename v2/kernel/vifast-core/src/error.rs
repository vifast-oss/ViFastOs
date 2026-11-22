// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kiểu dùng chung của core: `ObjType`, `KError` (19 mã sinh từ spec), `SlotRef`, `CapInfo`,
//! và `CheckFailure` cho các hàm bất biến (`no_std`, không cấp phát).

use core::fmt;

use crate::spec::{
    CNODE_BASE_BYTES, OBJ_ENDPOINT_BYTES, OBJ_NOTIFICATION_BYTES, OBJ_REPLY_BYTES,
    OBJ_SCHED_CONTEXT_BYTES, OBJ_SCHED_CONTROL_BYTES, OBJ_TCB_BYTES, SLOT_BYTES,
};

/// Danh tính object — chỉ số 1-based, **không tái dùng** (giống `Vec<Meta>` của sim).
pub type ObjId = u32;
/// Danh tính cap — chỉ số 1-based, **không tái dùng** (giống `Vec<Cap>` của sim).
pub type CapId = u32;

/// Vị trí slot cap: `(cnode, slot)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotRef {
    /// Object CNode chứa slot.
    pub cnode: ObjId,
    /// Chỉ số slot trong CNode.
    pub slot: usize,
}

impl SlotRef {
    /// Tạo tham chiếu slot.
    pub const fn new(cnode: ObjId, slot: usize) -> Self {
        Self { cnode, slot }
    }
}

/// 16 loại object của `spec/abi.toml` `[object]` — 8 lõi + 8 reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjType {
    /// Vùng nhớ nguồn cấp object (object gốc).
    Untyped,
    /// Bảng slot cap (CSpace một cấp).
    CNode,
    /// Thread control block (KT-59 mới có trạng thái).
    Tcb,
    /// Ngân sách MCS (KT-59).
    SchedContext,
    /// Endpoint IPC (KT-60).
    Endpoint,
    /// Notification (KT-60).
    Notification,
    /// Reply (KT-60).
    Reply,
    /// SchedControl (KT-59).
    SchedControl,
    /// Không gian địa chỉ (reserved tới K3b).
    VSpace,
    /// Bảng trang (reserved).
    PageTable,
    /// Khung vật lý (reserved).
    Frame,
    /// Điều khiển IRQ (reserved).
    IrqControl,
    /// Handler IRQ (reserved).
    IrqHandler,
    /// IOSpace (reserved).
    IOSpace,
    /// VCpu (reserved).
    VCpu,
    /// PowerControl (reserved).
    PowerControl,
}

impl ObjType {
    /// Tên ổn định (gắn log/TAP, không phải ABI thứ hai).
    pub const fn name(self) -> &'static str {
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

    /// Tra theo tên (harness/scenario; `no_std` nên so chuỗi trực tiếp).
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

    /// Lõi `1..11` của spec: 8 object gốc (`1..8`) + **3 object VM** (`VSpace` 9/`PageTable` 10/
    /// `Frame` 11 — D2-242 nhóm 9 nâng `stability` lên `core`/`frozen`). Reserved còn
    /// `12..16` (`IrqControl`..`PowerControl`) — chưa cài (`NotSupported`).
    pub const fn is_core(self) -> bool {
        !matches!(
            self,
            ObjType::IrqControl
                | ObjType::IrqHandler
                | ObjType::IOSpace
                | ObjType::VCpu
                | ObjType::PowerControl
        )
    }

    /// Cỡ cố định theo `[object_size].riscv64`; `None` nếu cỡ biến thiên (Untyped/CNode).
    pub const fn fixed_size(self) -> Option<usize> {
        match self {
            ObjType::Tcb => Some(OBJ_TCB_BYTES),
            ObjType::SchedContext => Some(OBJ_SCHED_CONTEXT_BYTES),
            ObjType::Endpoint => Some(OBJ_ENDPOINT_BYTES),
            ObjType::Notification => Some(OBJ_NOTIFICATION_BYTES),
            ObjType::Reply => Some(OBJ_REPLY_BYTES),
            ObjType::SchedControl => Some(OBJ_SCHED_CONTROL_BYTES),
            _ => None,
        }
    }

    /// Cỡ byte của CNode có `slots` slot: `cnode_base + slots * slot_bytes` (spec).
    pub const fn cnode_bytes(slots: usize) -> usize {
        CNODE_BASE_BYTES + slots * SLOT_BYTES
    }
}

/// 19 mã lỗi `spec/abi.toml` `[error]`; tên/số đọc từ `vifast_abi::ERRORS` (D2-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KError {
    /// Cap không tồn tại/đã chết.
    InvalidCapability,
    /// Cap trỏ sai loại object.
    WrongObjectType,
    /// Cap thiếu quyền cho thao tác.
    InsufficientRights,
    /// W^X (K3b: `Frame`, chưa port).
    WxViolation,
    /// Tham số không hợp lệ.
    InvalidArgument,
    /// Căn lề không thoả.
    Alignment,
    /// Slot đích đã có cap.
    SlotOccupied,
    /// Slot nguồn rỗng.
    SlotEmpty,
    /// Hết bộ nhớ/không đủ byte trong Untyped.
    NotEnoughMemory,
    /// Địa chỉ đã map (K3b).
    AlreadyMapped,
    /// Địa chỉ chưa map (K3b).
    NotMapped,
    /// Thiếu bảng trang (K3b).
    MissingPageTable,
    /// Còn con (K3b `Frame`/`PageTable`).
    HasChildren,
    /// Đối tượng bận (K3b IPC).
    Busy,
    /// Bị huỷ (K3b Reply).
    Cancelled,
    /// Hết hạn (K3b).
    Timeout,
    /// Thao tác không hợp lệ ở trạng thái này.
    IllegalOperation,
    /// Chưa hỗ trợ.
    NotSupported,
    /// Ngoài dải.
    RangeError,
}

impl KError {
    const fn spec_index(self) -> usize {
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

    /// Số mã lỗi trong ABI (1..19) — sinh từ spec.
    pub const fn id(self) -> u16 {
        crate::spec::ERRORS[self.spec_index()].1
    }

    /// Tên mã lỗi — sinh từ spec.
    pub const fn name(self) -> &'static str {
        crate::spec::ERRORS[self.spec_index()].0
    }
}

/// Ảnh chụp một cap (đọc-only; không lộ cấu trúc nội bộ). Cùng trường với sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapInfo {
    /// Danh tính cap.
    pub cap: CapId,
    /// Object cap trỏ tới.
    pub obj: ObjId,
    /// Loại object.
    pub ty: ObjType,
    /// Cap cha trong MDB (None = gốc).
    pub parent: Option<CapId>,
    /// Slot đang giữ cap (None = cap không nằm trong CNode).
    pub slot: Option<SlotRef>,
    /// Bốn bit quyền.
    pub rights: u8,
    /// Badge (chỉ Endpoint/Notification).
    pub badge: Option<u64>,
    /// Còn sống?
    pub alive: bool,
}

/// Lỗi của hàm bất biến (`check_caps`/`check_bytes`) — `no_std`, không cấp phát.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckFailure {
    /// Mô tả ngắn (ASCII/không dấu để log ổn định).
    pub what: &'static str,
    /// Đối tượng chính (cap/object/Untyped).
    pub a: u32,
    /// Đối tượng phụ (0 nếu không có).
    pub b: u32,
}

impl CheckFailure {
    /// Lỗi một đối tượng.
    pub const fn one(what: &'static str, a: u32) -> Self {
        Self { what, a, b: 0 }
    }

    /// Lỗi hai đối tượng.
    pub const fn two(what: &'static str, a: u32, b: u32) -> Self {
        Self { what, a, b }
    }
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.b == 0 {
            write!(f, "{} {}", self.what, self.a)
        } else {
            write!(f, "{} {} (khac {})", self.what, self.a, self.b)
        }
    }
}
