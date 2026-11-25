// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-58 — kịch bản port từ KT-09/10/11 (tách khỏi `main.rs`, KT-407).
use super::*;

// ---------------------------------------------------------------------------------------------
// Kịch bản port từ KT-09/10/11
// ---------------------------------------------------------------------------------------------

pub(crate) const ALL: u8 = kc::RIGHTS_ALL;
pub(crate) const READ: u8 = kc::RIGHT_READ;
pub(crate) const WRITE: u8 = kc::RIGHT_WRITE;

/// KT-09 `tong_byte_bat_bien_sau_moi_retype`: bốn loại object lõi xếp liền, không gap.
pub(crate) fn script_bytes() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x1000,
                size: 8192,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            2,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 512,
            free: 7680,
            allocated: 512,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Endpoint,
                bits: 0,
                flags: 0,
            },
            3,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 576,
            free: 7616,
            allocated: 576,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Reply,
                bits: 0,
                flags: 0,
            },
            4,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 640,
            free: 7552,
            allocated: 640,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::SchedContext,
                bits: 0,
                flags: 0,
            },
            5,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 896,
            free: 7296,
            allocated: 896,
            gap: 0,
        }),
        s(Op::CheckObject {
            obj: 3,
            bytes: Some((0, 512)),
        }),
        s(Op::CheckObject {
            obj: 4,
            bytes: Some((512, 64)),
        }),
        s(Op::CheckObject {
            obj: 6,
            bytes: Some((640, 256)),
        }),
    ]
}

/// KT-09 `watermark_chi_tien_va_het_cho_khong_ro_byte`: vừa khít 1024 B rồi hết chỗ.
pub(crate) fn script_exhaust() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x2000,
                size: 1024,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            2,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 512,
            free: 512,
            allocated: 512,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Endpoint,
                bits: 0,
                flags: 0,
            },
            3,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 576,
            free: 448,
            allocated: 576,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Reply,
                bits: 0,
                flags: 0,
            },
            4,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 640,
            free: 384,
            allocated: 640,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::SchedContext,
                bits: 0,
                flags: 0,
            },
            5,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 896,
            free: 128,
            allocated: 896,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Endpoint,
                bits: 0,
                flags: 0,
            },
            6,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 960,
            free: 64,
            allocated: 960,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Reply,
                bits: 0,
                flags: 0,
            },
            7,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 1024,
            free: 0,
            allocated: 1024,
            gap: 0,
        }),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Endpoint,
                bits: 0,
                flags: 0,
            },
            kc::KError::NotEnoughMemory,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 1024,
            free: 0,
            allocated: 1024,
            gap: 0,
        }),
    ]
}

/// KT-09 `retype_loi_tra_dung_ma`: mọi lỗi không đốt byte.
pub(crate) fn script_retype_errors() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x3000,
                size: 4096,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 1,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            kc::KError::SlotEmpty,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 9,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            kc::KError::RangeError,
        ),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 3),
        ok(Op::RootCap { obj: 3 }, 2),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 2,
                cap: 2,
            },
            0,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 2,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            kc::KError::WrongObjectType,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 3,
                flags: 0,
            },
            kc::KError::Alignment,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 12,
                flags: 0,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 31,
                flags: 0,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::VSpace,
                bits: 0,
                flags: 0,
            },
            kc::KError::NotSupported,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::VSpace,
                bits: 0,
                flags: 1,
            },
            kc::KError::NotSupported,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 0,
                flags: 1,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Untyped,
                bits: 11,
                flags: 0,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Untyped,
                bits: 64,
                flags: 0,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Untyped,
                bits: 13,
                flags: 0,
            },
            kc::KError::NotEnoughMemory,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::IrqControl,
                bits: 0,
                flags: 0,
            },
            kc::KError::NotSupported,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 0,
            free: 4096,
            allocated: 0,
            gap: 0,
        }),
    ]
}

/// KT-17/KT-09 `untyped_con_duoc_can_le_va_tinh_du_byte`: gap căn lề vẫn tính byte.
pub(crate) fn script_nested_align() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x4000,
                size: 8192,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            2,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Untyped,
                bits: 12,
                flags: 0,
            },
            3,
        ),
        s(Op::CheckObject {
            obj: 4,
            bytes: Some((4096, 4096)),
        }),
        s(Op::CheckUntyped {
            u: 2,
            used: 8192,
            free: 0,
            allocated: 4608,
            gap: 3584,
        }),
        err(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Untyped,
                bits: 12,
                flags: 0,
            },
            kc::KError::NotEnoughMemory,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 8192,
            free: 0,
            allocated: 4608,
            gap: 3584,
        }),
    ]
}

/// KT-09 `revoke_untyped_long_nhau_finalise_ca_cay`: finalise cả cây Untyped con.
pub(crate) fn script_revoke_nested() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x5000,
                size: 16384,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Untyped,
                bits: 12,
                flags: 0,
            },
            2,
        ),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 3,
                cap: 2,
            },
            0,
        ),
        s(Op::CheckObject {
            obj: 3,
            bytes: Some((0, 4096)),
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 3,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            3,
        ),
        s(Op::CheckUntyped {
            u: 3,
            used: 512,
            free: 3584,
            allocated: 512,
            gap: 0,
        }),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            2,
        ),
        s(Op::CheckObjectAlive {
            obj: 3,
            alive: false,
        }),
        s(Op::CheckObjectAlive {
            obj: 4,
            alive: false,
        }),
        s(Op::CheckUntyped {
            u: 2,
            used: 0,
            free: 16384,
            allocated: 0,
            gap: 0,
        }),
        s(Op::CheckUntyped {
            u: 3,
            used: 0,
            free: 4096,
            allocated: 0,
            gap: 0,
        }),
        s(Op::CheckCapAt {
            cnode: 1,
            slot: 3,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCapAt {
            cnode: 1,
            slot: 4,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCap {
            cap: 2,
            alive: false,
            rights: ALL,
            badge: None,
            parent: Some(1),
            slot: None,
        }),
        s(Op::CheckCap {
            cap: 3,
            alive: false,
            rights: ALL,
            badge: None,
            parent: Some(2),
            slot: None,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            4,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 512,
            free: 15872,
            allocated: 512,
            gap: 0,
        }),
    ]
}

/// KT-09 `double_free_bi_bat`: revoke lần hai không trả byte lần nữa.
pub(crate) fn script_double_free() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x6000,
                size: 4096,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Endpoint,
                bits: 0,
                flags: 0,
            },
            2,
        ),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            1,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 0,
            free: 4096,
            allocated: 0,
            gap: 0,
        }),
        ok(Op::Delete { cnode: 1, slot: 0 }, 0),
        err(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            kc::KError::SlotEmpty,
        ),
        s(Op::CheckUntyped {
            u: 2,
            used: 0,
            free: 4096,
            allocated: 0,
            gap: 0,
        }),
        s(Op::CheckCap {
            cap: 2,
            alive: false,
            rights: ALL,
            badge: None,
            parent: Some(1),
            slot: None,
        }),
        s(Op::CheckObjectAlive {
            obj: 3,
            alive: false,
        }),
    ]
}

/// KT-10 `delete_cha_reparent_con_cho_ong` + `revoke_none_xoa_moi_cay_con_trong_cnode`.
pub(crate) fn script_mdb_tree() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(Op::AddCnode { slots: 4 }, 2),
        ok(Op::AddCnode { slots: 4 }, 3),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 4),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 5),
        ok(Op::RootCap { obj: 4 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        s(Op::CheckCap {
            cap: 1,
            alive: true,
            rights: ALL,
            badge: None,
            parent: None,
            slot: Some((1, 0)),
        }),
        ok(
            Op::Copy {
                sc: 1,
                ss: 0,
                dc: 1,
                ds: 1,
            },
            2,
        ),
        ok(
            Op::Copy {
                sc: 1,
                ss: 1,
                dc: 2,
                ds: 0,
            },
            3,
        ),
        ok(
            Op::Copy {
                sc: 2,
                ss: 0,
                dc: 2,
                ds: 1,
            },
            4,
        ),
        s(Op::CheckChildren {
            cap: 1,
            children: &[2],
        }),
        s(Op::CheckChildren {
            cap: 2,
            children: &[3],
        }),
        s(Op::CheckChildren {
            cap: 3,
            children: &[4],
        }),
        ok(Op::Delete { cnode: 1, slot: 1 }, 0),
        s(Op::CheckCap {
            cap: 2,
            alive: false,
            rights: ALL,
            badge: None,
            parent: None,
            slot: None,
        }),
        s(Op::CheckCap {
            cap: 3,
            alive: true,
            rights: ALL,
            badge: None,
            parent: Some(1),
            slot: Some((2, 0)),
        }),
        s(Op::CheckChildren {
            cap: 1,
            children: &[3],
        }),
        s(Op::CheckChildren {
            cap: 3,
            children: &[4],
        }),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            2,
        ),
        s(Op::CheckCapAt {
            cnode: 2,
            slot: 0,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCapAt {
            cnode: 2,
            slot: 1,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCap {
            cap: 1,
            alive: true,
            rights: ALL,
            badge: None,
            parent: None,
            slot: Some((1, 0)),
        }),
        ok(Op::RootCap { obj: 5 }, 5),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 2,
                cap: 5,
            },
            0,
        ),
        ok(
            Op::Copy {
                sc: 1,
                ss: 0,
                dc: 3,
                ds: 0,
            },
            6,
        ),
        ok(
            Op::Copy {
                sc: 3,
                ss: 0,
                dc: 3,
                ds: 1,
            },
            7,
        ),
        ok(
            Op::Copy {
                sc: 1,
                ss: 0,
                dc: 3,
                ds: 2,
            },
            8,
        ),
        s(Op::CheckChildren {
            cap: 1,
            children: &[6, 8],
        }),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: None,
            },
            3,
        ),
        s(Op::CheckCap {
            cap: 1,
            alive: true,
            rights: ALL,
            badge: None,
            parent: None,
            slot: Some((1, 0)),
        }),
        s(Op::CheckCap {
            cap: 5,
            alive: true,
            rights: ALL,
            badge: None,
            parent: None,
            slot: Some((1, 2)),
        }),
        s(Op::CheckCap {
            cap: 6,
            alive: false,
            rights: ALL,
            badge: None,
            parent: Some(1),
            slot: None,
        }),
        s(Op::CheckCapAt {
            cnode: 3,
            slot: 0,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCapAt {
            cnode: 3,
            slot: 1,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCapAt {
            cnode: 3,
            slot: 2,
            err: Some("SlotEmpty"),
        }),
    ]
}

/// KT-10 `revoke_untyped_finalise_cnode_va_xoa_cap_trong_slot`.
pub(crate) fn script_finalise_cnode() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x7000,
                size: 16384,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::CNode,
                bits: 4,
                flags: 0,
            },
            2,
        ),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 1,
                cap: 2,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            3,
        ),
        ok(
            Op::PutCap {
                cnode: 3,
                slot: 1,
                cap: 3,
            },
            0,
        ),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 5),
        ok(Op::RootCap { obj: 5 }, 4),
        ok(
            Op::PutCap {
                cnode: 3,
                slot: 0,
                cap: 4,
            },
            0,
        ),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            3,
        ),
        s(Op::CheckObjectAlive {
            obj: 3,
            alive: false,
        }),
        s(Op::CheckObjectAlive {
            obj: 4,
            alive: false,
        }),
        s(Op::CheckObjectAlive {
            obj: 5,
            alive: true,
        }),
        s(Op::CheckCap {
            cap: 4,
            alive: false,
            rights: ALL,
            badge: None,
            parent: None,
            slot: None,
        }),
        s(Op::CheckCapAt {
            cnode: 3,
            slot: 0,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckCapAt {
            cnode: 3,
            slot: 1,
            err: Some("SlotEmpty"),
        }),
        s(Op::CheckUntyped {
            u: 2,
            used: 0,
            free: 16384,
            allocated: 0,
            gap: 0,
        }),
        s(Op::CheckCapAt {
            cnode: 1,
            slot: 1,
            err: Some("SlotEmpty"),
        }),
    ]
}

/// KT-10 `finalise_cnode_long_nhau_xoa_ca_cap_trong_cnode_con`.
pub(crate) fn script_finalise_cnode_nested() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0x8000,
                size: 16384,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::CNode,
                bits: 4,
                flags: 0,
            },
            2,
        ),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 1,
                cap: 2,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::CNode,
                bits: 4,
                flags: 0,
            },
            3,
        ),
        ok(
            Op::PutCap {
                cnode: 3,
                slot: 0,
                cap: 3,
            },
            0,
        ),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 5),
        ok(Op::RootCap { obj: 5 }, 4),
        ok(
            Op::PutCap {
                cnode: 4,
                slot: 0,
                cap: 4,
            },
            0,
        ),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            3,
        ),
        s(Op::CheckObjectAlive {
            obj: 3,
            alive: false,
        }),
        s(Op::CheckObjectAlive {
            obj: 4,
            alive: false,
        }),
        s(Op::CheckObjectAlive {
            obj: 5,
            alive: true,
        }),
        s(Op::CheckCap {
            cap: 4,
            alive: false,
            rights: ALL,
            badge: None,
            parent: None,
            slot: None,
        }),
    ]
}

/// KT-11: `Mint(Some(0))` xoá badge; Copy/Mint không nới quyền; Retype kiểm WRITE và kế thừa.
pub(crate) fn script_rights() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(Op::AddCnode { slots: 8 }, 2),
        ok(
            Op::AddUntyped {
                base: 0x9000,
                size: 16384,
            },
            3,
        ),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 4),
        ok(Op::RootCap { obj: 4 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Mint {
                sc: 1,
                ss: 0,
                dc: 1,
                ds: 1,
                rights: ALL,
                badge: Some(7),
            },
            2,
        ),
        s(Op::CheckCap {
            cap: 2,
            alive: true,
            rights: ALL,
            badge: Some(7),
            parent: Some(1),
            slot: Some((1, 1)),
        }),
        ok(
            Op::Mint {
                sc: 1,
                ss: 1,
                dc: 2,
                ds: 0,
                rights: ALL,
                badge: Some(0),
            },
            3,
        ),
        s(Op::CheckCap {
            cap: 3,
            alive: true,
            rights: ALL,
            badge: None,
            parent: Some(2),
            slot: Some((2, 0)),
        }),
        ok(
            Op::Mint {
                sc: 1,
                ss: 1,
                dc: 2,
                ds: 1,
                rights: ALL,
                badge: None,
            },
            4,
        ),
        s(Op::CheckCap {
            cap: 4,
            alive: true,
            rights: ALL,
            badge: Some(7),
            parent: Some(2),
            slot: Some((2, 1)),
        }),
        ok(
            Op::CopyRights {
                sc: 2,
                ss: 0,
                dc: 2,
                ds: 2,
                rights: Some(READ),
            },
            5,
        ),
        s(Op::CheckCap {
            cap: 5,
            alive: true,
            rights: READ,
            badge: None,
            parent: Some(3),
            slot: Some((2, 2)),
        }),
        err(
            Op::CopyRights {
                sc: 2,
                ss: 2,
                dc: 2,
                ds: 3,
                rights: Some(WRITE),
            },
            kc::KError::InsufficientRights,
        ),
        ok(
            Op::Copy {
                sc: 2,
                ss: 2,
                dc: 2,
                ds: 3,
            },
            6,
        ),
        s(Op::CheckCap {
            cap: 6,
            alive: true,
            rights: READ,
            badge: None,
            parent: Some(5),
            slot: Some((2, 3)),
        }),
        ok(Op::RootCap { obj: 3 }, 7),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 2,
                cap: 7,
            },
            0,
        ),
        ok(
            Op::Mint {
                sc: 1,
                ss: 2,
                dc: 1,
                ds: 3,
                rights: READ,
                badge: None,
            },
            8,
        ),
        err(
            Op::Retype {
                cnode: 1,
                slot: 3,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            kc::KError::InsufficientRights,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 2,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            9,
        ),
        s(Op::CheckCap {
            cap: 9,
            alive: true,
            rights: ALL,
            badge: None,
            parent: Some(7),
            slot: None,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 2,
                ty: Ty::Untyped,
                bits: 12,
                flags: 0,
            },
            10,
        ),
        s(Op::CheckUntyped {
            u: 3,
            used: 8192,
            free: 8192,
            allocated: 4608,
            gap: 3584,
        }),
        ok(
            Op::Mint {
                sc: 1,
                ss: 2,
                dc: 1,
                ds: 4,
                rights: READ | WRITE,
                badge: None,
            },
            11,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 4,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            12,
        ),
        s(Op::CheckCap {
            cap: 12,
            alive: true,
            rights: READ | WRITE,
            badge: None,
            parent: Some(11),
            slot: None,
        }),
        s(Op::CheckUntyped {
            u: 3,
            used: 8704,
            free: 7680,
            allocated: 5120,
            gap: 3584,
        }),
    ]
}

/// KT-09 `lap_lai_retype_va_revoke_khong_ro_byte`: vòng cấp/revoke 20 lần trả đủ byte.
pub(crate) fn script_retype_revoke_loop() -> Vec<Step> {
    let mut steps = vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0xa000,
                size: 4096,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
    ];
    for _ in 0..20 {
        steps.push(s(Op::Retype {
            cnode: 1,
            slot: 0,
            ty: Ty::Endpoint,
            bits: 0,
            flags: 0,
        }));
        steps.push(s(Op::Retype {
            cnode: 1,
            slot: 0,
            ty: Ty::Reply,
            bits: 0,
            flags: 0,
        }));
        steps.push(s(Op::CheckUntyped {
            u: 2,
            used: 128,
            free: 3968,
            allocated: 128,
            gap: 0,
        }));
        steps.push(s(Op::Revoke {
            cnode: 1,
            slot: Some(0),
        }));
        steps.push(s(Op::CheckUntyped {
            u: 2,
            used: 0,
            free: 4096,
            allocated: 0,
            gap: 0,
        }));
    }
    steps
}

/// D2-83: `Revoke`/`put_cap` trên CNode **đã finalise** phải trả `InvalidCapability` như sim
/// (sim đọc body → `Dead`), không đọc metadata trả `Ok(0)`/`SlotEmpty`/`WrongObjectType`.
pub(crate) fn script_dead_cnode_errors() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(
            Op::AddUntyped {
                base: 0xb000,
                size: 16384,
            },
            2,
        ),
        ok(Op::RootCap { obj: 2 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 0,
                ty: Ty::CNode,
                bits: 4,
                flags: 0,
            },
            2,
        ),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 1,
                cap: 2,
            },
            0,
        ),
        ok(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            1,
        ),
        err(
            Op::Revoke {
                cnode: 3,
                slot: Some(0),
            },
            kc::KError::InvalidCapability,
        ),
        err(
            Op::Revoke {
                cnode: 3,
                slot: Some(99),
            },
            kc::KError::InvalidCapability,
        ),
        err(
            Op::Revoke {
                cnode: 3,
                slot: None,
            },
            kc::KError::InvalidCapability,
        ),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 4),
        ok(Op::RootCap { obj: 4 }, 3),
        err(
            Op::PutCap {
                cnode: 3,
                slot: 0,
                cap: 3,
            },
            kc::KError::InvalidCapability,
        ),
        s(Op::CheckCap {
            cap: 3,
            alive: true,
            rights: ALL,
            badge: None,
            parent: None,
            slot: None,
        }),
        s(Op::CheckCapAt {
            cnode: 3,
            slot: 0,
            err: Some("SlotEmpty"),
        }),
    ]
}

/// KT-10 `delete_cap_goc_thi_con_thanh_goc`: xoá cap gốc ⇒ con thành gốc (không mồ côi).
pub(crate) fn script_delete_root_cap() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(Op::AddCnode { slots: 4 }, 2),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 3),
        ok(Op::RootCap { obj: 3 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(
            Op::Copy {
                sc: 1,
                ss: 0,
                dc: 2,
                ds: 0,
            },
            2,
        ),
        s(Op::CheckChildren {
            cap: 1,
            children: &[2],
        }),
        ok(Op::Delete { cnode: 1, slot: 0 }, 0),
        s(Op::CheckCap {
            cap: 1,
            alive: false,
            rights: ALL,
            badge: None,
            parent: None,
            slot: None,
        }),
        s(Op::CheckCap {
            cap: 2,
            alive: true,
            rights: ALL,
            badge: None,
            parent: None,
            slot: Some((2, 0)),
        }),
        s(Op::CheckChildren {
            cap: 2,
            children: &[],
        }),
        err(
            Op::Revoke {
                cnode: 1,
                slot: Some(0),
            },
            kc::KError::SlotEmpty,
        ),
    ]
}

/// Nhánh còn lại: `put_cap` cap đã có slot ⇒ `IllegalOperation`; badge `Some(7)` trên Tcb ⇒
/// `InvalidArgument` còn `Some(0)` hợp lệ; Retype Untyped `len` 8192 > `ARENA_ALIGN` ⇒
/// `Alignment` (sau khi đã qua cổng `len <= u_size`).
pub(crate) fn script_badge_and_align() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(Op::AddCnode { slots: 8 }, 2),
        ok(
            Op::AddUntyped {
                base: 0xc000,
                size: 8192,
            },
            3,
        ),
        ok(Op::AddKernel { ty: Ty::Tcb }, 4),
        ok(Op::RootCap { obj: 4 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        err(
            Op::PutCap {
                cnode: 2,
                slot: 0,
                cap: 1,
            },
            kc::KError::IllegalOperation,
        ),
        err(
            Op::Mint {
                sc: 1,
                ss: 0,
                dc: 2,
                ds: 0,
                rights: ALL,
                badge: Some(7),
            },
            kc::KError::InvalidArgument,
        ),
        s(Op::CheckCapAt {
            cnode: 2,
            slot: 0,
            err: Some("SlotEmpty"),
        }),
        ok(
            Op::Mint {
                sc: 1,
                ss: 0,
                dc: 2,
                ds: 0,
                rights: ALL,
                badge: Some(0),
            },
            2,
        ),
        s(Op::CheckCap {
            cap: 2,
            alive: true,
            rights: ALL,
            badge: None,
            parent: Some(1),
            slot: Some((2, 0)),
        }),
        ok(Op::RootCap { obj: 3 }, 3),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 1,
                cap: 3,
            },
            0,
        ),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 1,
                ty: Ty::Tcb,
                bits: 0,
                flags: 0,
            },
            4,
        ),
        s(Op::CheckUntyped {
            u: 3,
            used: 512,
            free: 7680,
            allocated: 512,
            gap: 0,
        }),
        ok(
            Op::Retype {
                cnode: 1,
                slot: 1,
                ty: Ty::Untyped,
                bits: 12,
                flags: 0,
            },
            5,
        ),
        s(Op::CheckUntyped {
            u: 3,
            used: 8192,
            free: 0,
            allocated: 4608,
            gap: 3584,
        }),
        err(
            Op::Retype {
                cnode: 1,
                slot: 1,
                ty: Ty::Untyped,
                bits: 13,
                flags: 0,
            },
            kc::KError::Alignment,
        ),
        s(Op::CheckUntyped {
            u: 3,
            used: 8192,
            free: 0,
            allocated: 4608,
            gap: 3584,
        }),
    ]
}
