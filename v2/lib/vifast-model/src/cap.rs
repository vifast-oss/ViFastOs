// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cap, quyền và vị trí slot — mô hình tối giản của thiết kế §4.3 (chưa phải MDB 48 B).

pub type ObjId = u32;
pub type CapId = u32;

/// Quyền của cap — thiết kế §4.3 (không có bit EXEC: EXEC là thuộc tính lúc map, QD §3.12).
pub const RIGHT_READ: u8 = 1 << 0;
pub const RIGHT_WRITE: u8 = 1 << 1;
pub const RIGHT_GRANT: u8 = 1 << 2;
pub const RIGHT_GRANT_REPLY: u8 = 1 << 3;
/// Cap gốc do kernel/harness cấp giữ đủ 4 quyền.
pub const RIGHTS_ALL: u8 = RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT | RIGHT_GRANT_REPLY;

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
    pub ty: crate::object::ObjType,
    pub parent: Option<CapId>,
    pub slot: Option<SlotRef>,
    pub rights: u8,
    pub badge: Option<u64>,
    pub alive: bool,
}
