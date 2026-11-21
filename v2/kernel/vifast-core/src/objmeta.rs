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
pub(crate) struct Meta {
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

/// Một cap: object + quan hệ cha (MDB) + slot + quyền + badge.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cap {
    pub(crate) obj: ObjId,
    pub(crate) ty: ObjType,
    pub(crate) parent: Option<CapId>,
    pub(crate) slot: Option<SlotRef>,
    pub(crate) rights: u8,
    pub(crate) badge: Option<u64>,
    pub(crate) alive: bool,
}

impl Cap {
    pub(crate) const EMPTY: Self = Self {
        obj: 0,
        ty: ObjType::Untyped,
        parent: None,
        slot: None,
        rights: 0,
        badge: None,
        alive: false,
    };
}
