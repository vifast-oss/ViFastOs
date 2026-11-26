// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sổ cái **metadata** của [`crate::Machine`] — `BodyRef`/`Meta`/`Cap`/`FinaliseEntry` + mã loại
//! body. Tách khỏi `machine.rs` (KT-312) để giữ luật **R1** (≤ 1.500 dòng/tệp) sau khi `Meta`
//! được thu hẹp kiểu; `machine.rs` **tái xuất đúng tên cũ** nên mọi `use crate::machine::{…}`
//! không phải sửa.
//!
//! **KT-312** — `Meta` **48 B** (trước **88 B**): ba trường chỉ số dùng kiểu hẹp có **chứng minh**
//! miền giá trị — `untyped: Option<u8>` (chỉ số object `< MAX_OBJECTS = 256`), `region:
//! Option<u8>` (chỉ số arena `< MAX_UNTYPEDS = 16`), `slots_meta: Option<u16>` (số slot CNode
//! `≤ MAX_SLOT_WORDS = 4096`). Ngữ nghĩa **không đổi** (`Some`/`None`, cùng giá trị);
//! `objects[256]` giảm **22.528 → 12.288 B** ⇒ `size_of::<Machine>()` **194.520 → 184.280 B**
//! (pin D2-84 `196.608` **không** nới).

use crate::error::{CapId, ObjId, ObjType, SlotRef};
use crate::tcb::TcbState;

// ---------------------------------------------------------------------------------------------
// Body của object (phần trạng thái KT-59 thêm vào; `.bss`, không cấp phát)
// ---------------------------------------------------------------------------------------------

/// Loại body đã cấp cho một object — `NONE` với các loại chưa port body (Endpoint/...).
pub(crate) const BODY_NONE: u8 = 0;
/// Body TCB (`Machine::tcbs`).
pub(crate) const BODY_TCB: u8 = 1;
/// Body SchedContext (`Machine::scs`).
pub(crate) const BODY_SC: u8 = 2;
/// Body Endpoint (`Machine::endpoints`, KT-60).
pub(crate) const BODY_ENDPOINT: u8 = 3;
/// Body Reply (`Machine::replies`, KT-60).
pub(crate) const BODY_REPLY: u8 = 4;
/// Body Notification (`Machine::notifications`, KT-60).
pub(crate) const BODY_NOTIFICATION: u8 = 5;

/// Tham chiếu body của object — chỉ số + cờ sống.
///
/// sim `destroy` body khi object chết; core đánh dấu `alive = false` và trả chỉ số vào
/// free-list để tái dùng. Truy cập body đã chết trả `InvalidCapability` như sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BodyRef {
    pub(crate) kind: u8,
    pub(crate) idx: u16,
    pub(crate) alive: bool,
}

impl BodyRef {
    pub(crate) const NONE: Self = Self {
        kind: BODY_NONE,
        idx: 0,
        alive: false,
    };
}

/// Một dòng log finalise TCB (KT-18 DoD 9) — port `FinaliseEntry` của sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinaliseEntry {
    /// TCB.
    pub tcb: ObjId,
    /// Trạng thái ghi log (luôn `Dead` ở sim).
    pub state: TcbState,
    /// SC được trả lại (hook finalise).
    pub released_sched_context: Option<ObjId>,
}
/// Sổ cái một object (metadata; body của object lõi **chưa** port ở tầng này).
#[derive(Debug, Clone, Copy)]
pub struct Meta {
    pub(crate) ty: ObjType,
    pub(crate) alive: bool,
    /// Chỉ số **object** của Untyped chứa object (None với object harness cấp / Untyped gốc).
    ///
    /// **KT-312** — kiểu hẹp `u8`: miền giá trị **chứng minh được** là `< MAX_OBJECTS = 256`
    /// (chỉ số object `0..=255`, `push_meta` nhận `object_index()`). `Option<usize>` chiếm 16 B
    /// (không niche) ⇒ `Meta` phình; `Option<u8>` = 2 B, **cùng** ngữ nghĩa `Some`/`None`.
    pub(crate) untyped: Option<u8>,
    /// Offset logic trong Untyped.
    pub(crate) offset: usize,
    /// Cỡ logic của spec.
    pub(crate) len: usize,
    /// Untyped: chỉ số arena trong `Machine::regions`.
    ///
    /// **KT-312** — kiểu hẹp `u8`, cùng lý do `untyped` (`< MAX_UNTYPEDS = 16`).
    pub(crate) region: Option<u8>,
    /// CNode: số slot trong **metadata** (đọc được cả sau khi chết; `None` với CNode
    /// `add_kernel_object` như sim `ChoDat::default`).
    ///
    /// **KT-312** — kiểu hẹp `u16`: số slot CNode `≤ MAX_SLOT_WORDS = 4096` (harness
    /// `≤ HARNESS_SLOT_WORDS = 1024`, arena `≤ REGION_SLOT_WORDS = 192`).
    pub(crate) slots_meta: Option<u16>,
    /// CNode: số slot **của body** (bảng từ).
    pub(crate) slot_len: usize,
    /// CNode: offset trong pool word.
    pub(crate) slot_base: usize,
    /// Body TCB/SchedContext (KT-59); `NONE` với loại chưa port body.
    pub(crate) body: BodyRef,
}

impl Meta {
    pub(crate) const EMPTY: Self = Self {
        ty: ObjType::Untyped,
        alive: false,
        untyped: None,
        offset: 0,
        len: 0,
        region: None,
        slots_meta: None,
        slot_len: 0,
        slot_base: 0,
        body: BodyRef::NONE,
    };
}

/// Bit `flags` của [`Cap`]: cap còn sống (thay trường `alive: bool`).
pub(crate) const CAP_ALIVE: u8 = 1 << 0;
/// Bit `flags` của [`Cap`]: `badge_val` có nghĩa (thay `Option<u64>`).
pub(crate) const CAP_HAS_BADGE: u8 = 1 << 1;

/// Một cap: object + quan hệ cha (MDB) + slot + quyền + badge.
///
/// **KT-417b (`D2-262` `Q2(c)`)** — **32 B** (trước **56 B**). Ba trường `Option` không có
/// niche chiếm 8 + **24** + **16** B; mọi trường đều `pub(crate)` nên đóng gói là thay đổi
/// **nội bộ**, **0** đổi ngữ nghĩa, **100 %** `Copy`, **0** `unsafe`, **0** global `static`:
///
/// - `parent: Option<CapId>` → [`Cap::parent_raw`] `u32` (`0 == None`; `CapId` 1-based),
/// - `slot: Option<SlotRef>` → [`Cap::slot_cnode`] `u32` + [`Cap::slot_idx`] `u16`
///   (`slot_cnode == 0 == None`, `slot_idx ≤ MAX_SLOT_WORDS = 4096`),
/// - `badge: Option<u64>` → [`Cap::badge_val`] `u64` + bit [`CAP_HAS_BADGE`] trong `flags`,
/// - `alive: bool` → bit [`CAP_ALIVE`] trong `flags`.
///
/// Đọc/ghi vẫn qua **cùng** API ngữ nghĩa ([`Cap::parent`]/[`Cap::set_parent`],
/// [`Cap::slot`]/[`Cap::set_slot`], [`Cap::badge`]/[`Cap::set_badge`], [`Cap::alive`]/
/// [`Cap::set_alive`]) ⇒ **một** nguồn sự thật cho ý nghĩa `Some`/`None`.
#[derive(Debug, Clone, Copy)]
pub struct Cap {
    pub(crate) obj: ObjId,
    /// MDB: cap cha; `0` = gốc (`CapId` là chỉ số 1-based nên `0` an toàn làm sentinel).
    pub(crate) parent_raw: u32,
    /// Slot đang giữ cap: object `CNode`; `0` = cap không nằm trong CNode.
    pub(crate) slot_cnode: u32,
    /// Slot đang giữ cap: chỉ số slot trong CNode (`≤ MAX_SLOT_WORDS = 4096`).
    pub(crate) slot_idx: u16,
    /// Badge — chỉ có nghĩa khi `flags & CAP_HAS_BADGE != 0`.
    pub(crate) badge_val: u64,
    pub(crate) ty: ObjType,
    pub(crate) rights: u8,
    /// Bit cờ: [`CAP_ALIVE`] / [`CAP_HAS_BADGE`].
    pub(crate) flags: u8,
}

/// `slot_idx: u16` chỉ đúng khi mọi chỉ số slot vừa 16 bit — `MAX_SLOT_WORDS` là chặn trên
/// **duy nhất** của chỉ số slot trong `Machine` (`retype`/`put_cap` kiểm trước khi ghi).
const _: () = assert!(crate::machine::MAX_SLOT_WORDS <= u16::MAX as usize);

impl Cap {
    pub(crate) const EMPTY: Self = Self {
        obj: 0,
        parent_raw: 0,
        slot_cnode: 0,
        slot_idx: 0,
        badge_val: 0,
        ty: ObjType::Untyped,
        rights: 0,
        flags: 0,
    };

    /// Còn sống? (bit [`CAP_ALIVE`])
    pub(crate) const fn alive(&self) -> bool {
        self.flags & CAP_ALIVE != 0
    }

    /// Ghi cờ sống.
    pub(crate) const fn set_alive(&mut self, alive: bool) {
        if alive {
            self.flags |= CAP_ALIVE;
        } else {
            self.flags &= !CAP_ALIVE;
        }
    }

    /// Cap cha trong MDB (`None` = gốc) — `parent_raw == 0` ⇔ `None`.
    pub(crate) const fn parent(&self) -> Option<CapId> {
        if self.parent_raw == 0 {
            None
        } else {
            Some(self.parent_raw)
        }
    }

    /// Ghi cap cha (`0`/`None` ⇒ xoá quan hệ).
    pub(crate) const fn set_parent(&mut self, parent: Option<CapId>) {
        self.parent_raw = match parent {
            Some(p) => p,
            None => 0,
        };
    }

    /// Slot đang giữ cap (`None` = cap không nằm trong CNode).
    pub(crate) const fn slot(&self) -> Option<SlotRef> {
        if self.slot_cnode == 0 {
            None
        } else {
            Some(SlotRef {
                cnode: self.slot_cnode,
                slot: self.slot_idx as usize,
            })
        }
    }

    /// Ghi slot đang giữ cap. `slot` phải `< MAX_SLOT_WORDS` (bất biến của `Machine`).
    pub(crate) const fn set_slot(&mut self, slot: Option<SlotRef>) {
        match slot {
            Some(s) => {
                self.slot_cnode = s.cnode;
                self.slot_idx = s.slot as u16;
            }
            None => {
                self.slot_cnode = 0;
                self.slot_idx = 0;
            }
        }
    }

    /// Badge (`None` = không có badge; `Some(0)` **khác** `None`).
    pub(crate) const fn badge(&self) -> Option<u64> {
        if self.flags & CAP_HAS_BADGE != 0 {
            Some(self.badge_val)
        } else {
            None
        }
    }

    /// Ghi badge + bit [`CAP_HAS_BADGE`].
    pub(crate) const fn set_badge(&mut self, badge: Option<u64>) {
        match badge {
            Some(b) => {
                self.badge_val = b;
                self.flags |= CAP_HAS_BADGE;
            }
            None => {
                self.badge_val = 0;
                self.flags &= !CAP_HAS_BADGE;
            }
        }
    }
}
