// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-10 — CNode/MDB test-first: cây dẫn xuất cap tối giản (quan hệ cha–con, slot, quyền, revoke).
//!
//! KT-10 **không** làm MDB đầy đủ của production (danh sách liên kết `mdb_prev/mdb_next` trong
//! `Slot` 48 B của thiết kế §4.3) và **không** làm `vifast-model` (KT-11). Mô hình tối giản ở đây
//! là cây dẫn xuất kiểu seL4 đủ để kiểm: mỗi cap sống có cha đúng (hoặc là gốc), con của cap bị
//! xoá thuộc về ông, revoke chỉ xoá con/cháu của cap đích, cap chết không còn trong slot nào,
//! và không cap nào trỏ tới object đã chết.
//!
//! Quy ước **sim** (không phải ABI): `CNode.Copy`/`CNode.Mint` trả `CapId` cho harness;
//! `rights` là bitmask READ|WRITE|GRANT|GRANT_REPLY; `badge` chỉ đặt cho Endpoint/Notification.

use vifast_sim::kobj::{
    CapId, KError, Machine, ObjId, ObjType, SlotRef, RIGHTS_ALL, RIGHT_GRANT_REPLY, RIGHT_READ,
    RIGHT_WRITE,
};

fn slot(cnode: ObjId, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

/// Bộ khung: CNode gốc `r`, `b`/`c` là CNode con; `ep`/`ep2` là object lõi.
struct Rig {
    m: Machine,
    r: ObjId,
    b: ObjId,
    c: ObjId,
    ep: ObjId,
    ep2: ObjId,
}

fn rig() -> Rig {
    let mut m = Machine::new();
    let r = m.add_cnode(8);
    let b = m.add_cnode(4);
    let c = m.add_cnode(4);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep2 = m.add_kernel_object(ObjType::Endpoint);
    Rig {
        m,
        r,
        b,
        c,
        ep,
        ep2,
    }
}

#[test]
fn copy_giu_quan_he_cha_con_va_slot() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc vào R0");
    let info = t.m.cap_info(root).expect("cap gốc");
    assert_eq!(info.parent, None);
    assert_eq!(info.slot, Some(slot(t.r, 0)));
    assert_eq!(info.rights, RIGHTS_ALL, "cap gốc của harness giữ đủ quyền");
    assert_eq!(info.badge, None);
    assert_eq!(info.obj, t.ep);

    let child = t.m.copy(slot(t.r, 0), slot(t.b, 0)).expect("copy R0 → B0");
    let child_info = t.m.cap_info(child).expect("cap con");
    assert_eq!(child_info.parent, Some(root), "cha phải là cap nguồn");
    assert_eq!(child_info.slot, Some(slot(t.b, 0)));
    assert_eq!(child_info.obj, t.ep, "copy giữ nguyên object");
    assert_eq!(child_info.rights, RIGHTS_ALL, "copy giữ nguyên quyền");
    assert_eq!(t.m.cap_children(root).expect("con của gốc"), vec![child]);
    assert_eq!(
        t.m.cap_children(child).expect("con của cap con"),
        Vec::<CapId>::new()
    );
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn mint_rights_hep_va_badge_endpoint() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc");
    let rights = RIGHT_READ | RIGHT_GRANT_REPLY;
    let minted =
        t.m.mint(slot(t.r, 0), slot(t.b, 0), rights, Some(0xBEEF))
            .expect("Mint Endpoint có badge");
    let info = t.m.cap_info(minted).expect("cap mint");
    assert_eq!(info.parent, Some(root));
    assert_eq!(info.rights, rights, "Mint phải thu hẹp đúng quyền yêu cầu");
    assert_eq!(info.badge, Some(0xBEEF), "Mint đặt badge cho Endpoint");
    assert_eq!(info.obj, t.ep);

    // Copy của cap đã mint giữ quyền hẹp + badge (không nới ngược).
    let copy = t.m.copy(slot(t.b, 0), slot(t.b, 1)).expect("copy cap mint");
    let copy_info = t.m.cap_info(copy).expect("cap copy");
    assert_eq!(copy_info.rights, rights);
    assert_eq!(copy_info.badge, Some(0xBEEF));
    assert_eq!(copy_info.parent, Some(minted));
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn mint_badge_chi_cho_endpoint_notification() {
    let mut t = rig();
    let tcb = t.m.add_kernel_object(ObjType::Tcb);
    let tcb_cap = t.m.root_cap(tcb);
    t.m.put_cap(t.r, 0, tcb_cap).expect("cap Tcb");
    assert_eq!(
        t.m.mint(slot(t.r, 0), slot(t.b, 0), RIGHTS_ALL, Some(7)),
        Err(KError::InvalidArgument),
        "badge khác 0 chỉ hợp lệ cho Endpoint/Notification"
    );
    // Some(0) = không đặt badge ⇒ hợp lệ với mọi loại object.
    let cap =
        t.m.mint(slot(t.r, 0), slot(t.b, 0), RIGHTS_ALL, Some(0))
            .expect("mint không badge");
    assert_eq!(t.m.cap_info(cap).expect("cap").badge, None);
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn mint_vuot_quyen_tra_insufficient_rights_va_khong_ro_cap() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc");
    let hep =
        t.m.mint(slot(t.r, 0), slot(t.b, 0), RIGHT_READ, None)
            .expect("mint quyền READ");
    let before = t.m.cap_table().len();
    let live_before = t.m.live_cap_count();
    // WRITE không nằm trong quyền của cap nguồn (chỉ READ).
    assert_eq!(
        t.m.mint(slot(t.b, 0), slot(t.b, 1), RIGHT_READ | RIGHT_WRITE, None),
        Err(KError::InsufficientRights)
    );
    // Bit lạ ngoài 4 bit quyền cũng là vượt quyền.
    assert_eq!(
        t.m.mint(slot(t.b, 0), slot(t.b, 1), 0x80, None),
        Err(KError::InsufficientRights)
    );
    assert_eq!(t.m.cap_table().len(), before, "Mint lỗi không được tạo cap");
    assert_eq!(t.m.live_cap_count(), live_before);
    assert_eq!(t.m.cap_children(hep).expect("con"), Vec::<CapId>::new());
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn mint_va_copy_ma_loi_slot() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc");
    t.m.mint(slot(t.r, 0), slot(t.b, 0), RIGHTS_ALL, None)
        .expect("mint B0");
    // Đích đã có cap.
    assert_eq!(
        t.m.mint(slot(t.r, 0), slot(t.b, 0), RIGHTS_ALL, None),
        Err(KError::SlotOccupied)
    );
    assert_eq!(
        t.m.copy(slot(t.r, 0), slot(t.b, 0)),
        Err(KError::SlotOccupied)
    );
    // Nguồn rỗng / ngoài dải.
    assert_eq!(
        t.m.mint(slot(t.r, 5), slot(t.b, 1), RIGHTS_ALL, None),
        Err(KError::SlotEmpty)
    );
    assert_eq!(
        t.m.mint(slot(t.r, 99), slot(t.b, 1), RIGHTS_ALL, None),
        Err(KError::RangeError)
    );
    // Đích ngoài dải / không phải CNode.
    assert_eq!(
        t.m.mint(slot(t.r, 0), slot(t.b, 99), RIGHTS_ALL, None),
        Err(KError::RangeError)
    );
    assert_eq!(
        t.m.mint(slot(t.r, 0), slot(t.ep2, 0), RIGHTS_ALL, None),
        Err(KError::WrongObjectType)
    );
    // Đích đã có cap thì lỗi, nhưng cap nguồn không bị đụng.
    assert!(t.m.cap_info(root).expect("gốc").alive);
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn delete_cha_reparent_con_cho_ong() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc R0");
    let a1 = t.m.copy(slot(t.r, 0), slot(t.r, 1)).expect("R0 → R1");
    let b0 = t.m.copy(slot(t.r, 1), slot(t.b, 0)).expect("R1 → B0");
    let c0 = t.m.copy(slot(t.b, 0), slot(t.b, 1)).expect("B0 → B1");
    t.m.check_caps().expect("cây trước khi xoá");

    // Xoá cap giữa A1: con B0 phải thuộc về ông (root), không mồ côi.
    t.m.delete(slot(t.r, 1)).expect("xoá R1");
    assert!(!t.m.cap_info(a1).expect("a1").alive);
    assert_eq!(
        t.m.cap_info(b0).expect("b0").parent,
        Some(root),
        "con của cap bị xoá phải được reparent cho ông (MDB seL4)"
    );
    assert_eq!(t.m.cap_children(root).expect("con gốc"), vec![b0]);
    assert_eq!(t.m.cap_children(b0).expect("con b0"), vec![c0]);
    t.m.check_caps().expect("bất biến cây cap sau delete");

    // Revoke tổ tiên vẫn xoá được cháu đã reparent — cây dẫn xuất không bị đứt.
    assert_eq!(
        t.m.revoke(t.r, Some(0)).expect("revoke cap gốc"),
        2,
        "phải xoá B0 và C0 dù A1 đã bị xoá"
    );
    assert!(t.m.cap_info(root).expect("gốc").alive);
    assert_eq!(t.m.cap_at(t.b, 0), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_at(t.b, 1), Err(KError::SlotEmpty));
    t.m.check_caps().expect("bất biến cây cap sau revoke");
}

#[test]
fn delete_cap_goc_thi_con_thanh_goc() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc R0");
    let b0 = t.m.copy(slot(t.r, 0), slot(t.b, 0)).expect("R0 → B0");
    t.m.delete(slot(t.r, 0)).expect("xoá cap gốc");
    let info = t.m.cap_info(b0).expect("b0");
    assert!(info.alive, "con vẫn sống sau khi xoá cha");
    assert_eq!(info.parent, None, "không còn ông ⇒ thành gốc");
    assert_eq!(info.slot, Some(slot(t.b, 0)), "con vẫn nằm trong slot");
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn revoke_dung_pham_vi_khong_dung_nhanh_khac() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc R0");
    let a1 = t.m.copy(slot(t.r, 0), slot(t.r, 1)).expect("R0 → R1");
    let b0 = t.m.copy(slot(t.r, 1), slot(t.b, 0)).expect("R1 → B0");
    let c0 = t.m.copy(slot(t.b, 0), slot(t.b, 1)).expect("B0 → B1");
    let d0 = t.m.copy(slot(t.r, 0), slot(t.c, 0)).expect("R0 → C0");
    let d1 = t.m.copy(slot(t.c, 0), slot(t.c, 1)).expect("C0 → C1");
    let other = t.m.root_cap(t.ep2);
    t.m.put_cap(t.c, 2, other).expect("cap nhánh độc lập C2");
    t.m.check_caps().expect("cây trước revoke");

    let before: Vec<CapId> =
        t.m.cap_table()
            .into_iter()
            .filter(|c| c.alive)
            .map(|c| c.cap)
            .collect();
    assert_eq!(
        t.m.revoke(t.r, Some(1)).expect("revoke A1"),
        2,
        "chỉ B0 và C0 (con/cháu của A1)"
    );
    for cap in [b0, c0] {
        let info = t.m.cap_info(cap).expect("cap chết");
        assert!(!info.alive, "cap trong cây bị revoke phải chết");
        assert_eq!(info.slot, None, "cap chết không còn giữ slot");
        assert_eq!(t.m.cap_obj(cap), Err(KError::InvalidCapability));
    }
    // Mọi cap ngoài cây A1 còn nguyên: cap đích, nhánh anh em, nhánh độc lập.
    for cap in [root, a1, d0, d1, other] {
        assert!(
            t.m.cap_info(cap).expect("cap sống").alive,
            "cap {cap} bị đụng"
        );
    }
    let after: Vec<CapId> =
        t.m.cap_table()
            .into_iter()
            .filter(|c| c.alive)
            .map(|c| c.cap)
            .collect();
    assert_eq!(after.len(), before.len() - 2);
    assert_eq!(t.m.live_cap_count(), after.len());
    t.m.check_caps().expect("bất biến cây cap sau revoke");
}

#[test]
fn revoke_none_xoa_moi_cay_con_trong_cnode() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc R0");
    let a1 = t.m.copy(slot(t.r, 0), slot(t.r, 1)).expect("R0 → R1");
    t.m.copy(slot(t.r, 1), slot(t.b, 0)).expect("R1 → B0");
    t.m.copy(slot(t.b, 0), slot(t.b, 1)).expect("B0 → B1");
    t.m.copy(slot(t.r, 0), slot(t.c, 0)).expect("R0 → C0");
    t.m.check_caps().expect("cây trước revoke");
    // `revoke(cnode, None)` revoke **từng cap trong slot**: xoá con/cháu của chúng,
    // không xoá chính cap trong slot. A1 là con của root ⇒ A1 cũng bị xoá.
    assert_eq!(
        t.m.revoke(t.r, None).expect("revoke cả CNode"),
        4,
        "A1, B0, B1 (dưới root qua A1) + C0 (dưới root)"
    );
    assert!(
        t.m.cap_info(root).expect("gốc").alive,
        "cap trong slot không bị xoá"
    );
    assert!(
        !t.m.cap_info(a1).expect("a1").alive,
        "A1 là con của root nên chết"
    );
    assert_eq!(t.m.cap_at(t.b, 0), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_at(t.c, 0), Err(KError::SlotEmpty));
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn revoke_va_delete_tra_dung_ma_loi() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc");
    assert_eq!(t.m.revoke(t.r, Some(3)), Err(KError::SlotEmpty));
    assert_eq!(t.m.revoke(t.r, Some(99)), Err(KError::RangeError));
    assert_eq!(t.m.revoke(t.ep, Some(0)), Err(KError::WrongObjectType));
    assert_eq!(t.m.delete(slot(t.r, 3)), Err(KError::SlotEmpty));
    assert_eq!(t.m.delete(slot(t.r, 99)), Err(KError::RangeError));
    assert_eq!(
        t.m.delete(slot(t.r, 0)),
        Ok(()),
        "xoá cap gốc hợp lệ (object không bị finalise)"
    );
    assert_eq!(
        t.m.delete(slot(t.r, 0)),
        Err(KError::SlotEmpty),
        "xoá lần hai là SlotEmpty"
    );
    assert!(t.m.object_alive(t.ep), "Delete không finalise object");
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn cap_chet_tra_invalid_capability() {
    let mut t = rig();
    let root = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, root).expect("cap gốc");
    let child = t.m.copy(slot(t.r, 0), slot(t.b, 0)).expect("copy");
    t.m.revoke(t.r, Some(0)).expect("revoke cây gốc");
    assert_eq!(t.m.cap_obj(child), Err(KError::InvalidCapability));
    assert!(!t.m.cap_info(child).expect("cap chết").alive);
    assert_eq!(t.m.cap_info(0), Err(KError::InvalidCapability));
    assert_eq!(t.m.cap_info(999), Err(KError::InvalidCapability));
    assert_eq!(t.m.cap_at(t.b, 0), Err(KError::SlotEmpty));
    t.m.check_caps().expect("bất biến cây cap");
}

#[test]
fn revoke_untyped_finalise_cnode_va_xoa_cap_trong_slot() {
    let mut m = Machine::new();
    let r = m.add_cnode(8);
    let u = m.add_untyped(0, 16_384);
    let u_cap = m.root_cap(u);
    m.put_cap(r, 0, u_cap).expect("cap Untyped R0");

    // CNode + TCB cấp từ Untyped.
    let cn_cap = m
        .retype(slot(r, 0), ObjType::CNode, 4, 0)
        .expect("retype CNode");
    let cn = m.cap_obj(cn_cap).expect("object CNode");
    m.put_cap(r, 1, cn_cap).expect("cap CNode R1");
    let tcb_cap = m
        .retype(slot(r, 0), ObjType::Tcb, 0, 0)
        .expect("retype Tcb");
    let tcb = m.cap_obj(tcb_cap).expect("object Tcb");
    m.put_cap(cn, 1, tcb_cap).expect("cap Tcb vào CNode");

    // Object lõi do harness cấp, sống độc lập: cap nằm trong slot của CNode sắp chết.
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep_cap = m.root_cap(ep);
    m.put_cap(cn, 0, ep_cap).expect("cap Endpoint trong CNode");
    m.check_caps().expect("cây trước revoke");

    // Revoke Untyped ⇒ finalise CNode (xoá cap trong slot) + TCB, không để cap trỏ object chết.
    assert_eq!(
        m.revoke(r, Some(0)).expect("revoke Untyped"),
        3,
        "cn_cap, tcb_cap (con của u_cap) + ep_cap (nằm trong CNode chết)"
    );
    assert!(!m.object_alive(cn), "CNode phải bị finalise");
    assert!(!m.object_alive(tcb), "TCB trong Untyped bị finalise");
    assert!(m.object_alive(ep), "object ngoài Untyped không bị finalise");
    assert_eq!(m.cap_obj(ep_cap), Err(KError::InvalidCapability));
    assert_eq!(
        m.cap_at(cn, 0),
        Err(KError::SlotEmpty),
        "slot CNode chết đã xoá"
    );
    assert_eq!(m.cap_at(cn, 1), Err(KError::SlotEmpty));
    assert_eq!(m.cap_at(r, 1), Err(KError::SlotEmpty));
    assert_eq!(m.untyped_used(u), 0, "revoke Untyped đặt watermark về 0");
    m.check_bytes(u).expect("bất biến byte");
    m.check_caps().expect("bất biến cây cap sau finalise");
}

#[test]
fn finalise_cnode_long_nhau_xoa_ca_cap_trong_cnode_con() {
    let mut m = Machine::new();
    let r = m.add_cnode(8);
    let u = m.add_untyped(0, 16_384);
    let u_cap = m.root_cap(u);
    m.put_cap(r, 0, u_cap).expect("cap Untyped R0");
    let cn1_cap = m
        .retype(slot(r, 0), ObjType::CNode, 4, 0)
        .expect("retype CNode 1");
    let cn1 = m.cap_obj(cn1_cap).expect("object CNode 1");
    m.put_cap(r, 1, cn1_cap).expect("cap CNode 1");
    let cn2_cap = m
        .retype(slot(r, 0), ObjType::CNode, 4, 0)
        .expect("retype CNode 2");
    let cn2 = m.cap_obj(cn2_cap).expect("object CNode 2");
    m.put_cap(cn1, 0, cn2_cap)
        .expect("cap CNode 2 trong CNode 1");
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep_cap = m.root_cap(ep);
    m.put_cap(cn2, 0, ep_cap)
        .expect("cap Endpoint trong CNode 2");
    m.check_caps().expect("cây trước revoke");

    m.revoke(r, Some(0)).expect("revoke Untyped");
    assert!(!m.object_alive(cn1));
    assert!(!m.object_alive(cn2));
    assert_eq!(m.cap_obj(ep_cap), Err(KError::InvalidCapability));
    assert_eq!(m.cap_obj(cn2_cap), Err(KError::InvalidCapability));
    m.check_caps().expect("bất biến cây cap");
}
