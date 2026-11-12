// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-11 — F3 (review KT-10) trên mô hình của sim: `Mint(Some(0))` phải xoá badge nguồn;
//! `Retype` phải kiểm WRITE và cap con kế thừa quyền cap nguồn.
//!
//! Viết **trước** khi sửa `kobj.rs` (D2-05): cả 3 test đỏ trên code KT-10.

use vifast_sim::kobj::{KError, Machine, ObjType, SlotRef, RIGHTS_ALL, RIGHT_READ, RIGHT_WRITE};

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

struct Rig {
    m: Machine,
    r: u32,
    b: u32,
    u: u32,
    ep: u32,
}

fn rig() -> Rig {
    let mut m = Machine::new();
    let r = m.add_cnode(8);
    let b = m.add_cnode(8);
    let u = m.add_untyped(0x1000, 16 * 1024);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    Rig { m, r, b, u, ep }
}

#[test]
fn mint_badge_zero_phai_xoa_badge_nguon() {
    let mut t = rig();
    let epcap = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, epcap).expect("cap Endpoint");
    let badged =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), RIGHTS_ALL, Some(7))
            .expect("mint badge 7");
    assert_eq!(t.m.cap_info(badged).expect("cap").badge, Some(7));
    let cleared =
        t.m.mint(slot(t.r, 1), slot(t.b, 0), RIGHTS_ALL, Some(0))
            .expect("mint Some(0)");
    assert_eq!(
        t.m.cap_info(cleared).expect("cap").badge,
        None,
        "F3: Some(0) phải xoá badge nguồn, không giữ Some(7)"
    );
    let inherited =
        t.m.mint(slot(t.r, 1), slot(t.b, 1), RIGHTS_ALL, None)
            .expect("mint None");
    assert_eq!(
        t.m.cap_info(inherited).expect("cap").badge,
        Some(7),
        "None mới là giữ badge nguồn"
    );
}

#[test]
fn retype_cap_untyped_thieu_write_phai_insufficient_rights() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let ro =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), RIGHT_READ, None)
            .expect("mint READ");
    assert!(t.m.cap_info(ro).expect("cap").alive);
    assert_eq!(
        t.m.retype(slot(t.r, 1), ObjType::Tcb, 0, 0),
        Err(KError::InsufficientRights),
        "F3: Retype qua cap Untyped thiếu WRITE phải InsufficientRights"
    );
    assert_eq!(t.m.untyped_used(t.u), 0, "Retype lỗi không được cấp byte");
}

#[test]
fn retype_cap_con_phai_ke_thua_quyen() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let narrowed = RIGHT_READ | RIGHT_WRITE;
    let u_ro =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), narrowed, None)
            .expect("mint Untyped hẹp");
    let tcb =
        t.m.retype(slot(t.r, 1), ObjType::Tcb, 0, 0)
            .expect("retype qua cap có WRITE");
    let info = t.m.cap_info(tcb).expect("cap con");
    assert_eq!(
        info.rights, narrowed,
        "F3: cap con của Retype phải kế thừa quyền cap nguồn"
    );
    assert_eq!(info.parent, Some(u_ro));
    let nested =
        t.m.retype(slot(t.r, 1), ObjType::Untyped, 12, 0)
            .expect("Untyped con");
    assert_eq!(t.m.cap_info(nested).expect("cap").rights, narrowed);
}
