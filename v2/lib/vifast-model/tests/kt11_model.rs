// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-11 — mô hình Untyped/CNode tái sử dụng: F3 (quyền `Copy`/`Retype`/`Delete`/`Revoke`),
//! badge `Mint(Some(0))`, watermark và cây dẫn xuất cap.
//!
//! Đây là test của **crate sản phẩm** `vifast-model`, không phải test-only model: sim dùng
//! `vifast-model` làm mô hình tham chiếu (so vi sai `kt11_diff_sim`), kernel K1a dùng lại được.

use vifast_model::{
    KError, Machine, ObjId, ObjType, SlotRef, RIGHTS_ALL, RIGHT_GRANT, RIGHT_GRANT_REPLY,
    RIGHT_READ, RIGHT_WRITE,
};

fn slot(cnode: ObjId, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

struct Rig {
    m: Machine,
    r: ObjId,
    b: ObjId,
    u: ObjId,
    ep: ObjId,
}

/// Bộ khung: CNode gốc `r` 16 slot, CNode đích `b` 8 slot, Untyped 16 KiB, một Endpoint.
fn rig() -> Rig {
    let mut m = Machine::new();
    let r = m.add_cnode(16);
    let b = m.add_cnode(8);
    let u = m.add_untyped(0x1000, 16 * 1024);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    Rig { m, r, b, u, ep }
}

#[test]
fn untyped_watermark_va_bat_bien_byte() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let tcb_cap =
        t.m.retype(slot(t.r, 0), ObjType::Tcb, 0, 0)
            .expect("Tcb 512 B");
    t.m.put_cap(t.b, 0, tcb_cap).expect("slot Tcb");
    let ep_cap =
        t.m.retype(slot(t.r, 0), ObjType::Endpoint, 0, 0)
            .expect("Endpoint 64 B");
    t.m.put_cap(t.b, 1, ep_cap).expect("slot Endpoint");
    assert_eq!(t.m.untyped_size(t.u), 16 * 1024);
    assert_eq!(t.m.untyped_allocated(t.u), 512 + 64);
    assert_eq!(t.m.untyped_used(t.u), 512 + 64, "Endpoint căn 16 sau 512");
    assert_eq!(t.m.untyped_free(t.u), 16 * 1024 - 576);
    assert_eq!(
        t.m.untyped_used(t.u) - t.m.untyped_allocated(t.u),
        t.m.untyped_gap(t.u)
    );
    assert_eq!(t.m.total_used(), 576);
    t.m.check_bytes(t.u).expect("bất biến byte");
    t.m.check_all_bytes().expect("mọi Untyped");
    t.m.check_caps().expect("cây cap");
    // Cùng `Retype` cùng cỡ phải cùng offset (tất định).
    let tcb2_cap =
        t.m.retype(slot(t.r, 0), ObjType::Tcb, 0, 0)
            .expect("Tcb thứ hai");
    let tcb = t.m.cap_obj(tcb_cap).expect("object Tcb");
    let tcb2 = t.m.cap_obj(tcb2_cap).expect("object Tcb 2");
    assert_eq!(t.m.object_bytes(tcb).expect("byte").0, 0);
    assert_eq!(t.m.object_bytes(tcb2).expect("byte").0, 576);
}

/// F3 — `Retype` phải kiểm quyền: cap Untyped thiếu WRITE không được cấp object.
#[test]
fn retype_tu_cap_thieu_write_tra_insufficient_rights() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let ro =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), RIGHT_READ, None)
            .expect("mint READ");
    assert_eq!(
        t.m.retype(slot(t.r, 1), ObjType::Tcb, 0, 0),
        Err(KError::InsufficientRights),
        "Retype qua cap thiếu WRITE phải InsufficientRights"
    );
    assert_eq!(t.m.untyped_used(t.u), 0, "Retype lỗi không được cấp byte");
    assert_eq!(t.m.object_count(), 4, "không sinh object mới");
    assert!(t.m.cap_info(ro).expect("cap READ").alive);
    t.m.check_caps().expect("cây cap");
}

/// F3 — cap con của `Retype` kế thừa quyền cap nguồn (không nới ngược).
#[test]
fn retype_cap_con_ke_thua_quyen_nguon() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let narrowed = RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT;
    let u_ro =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), narrowed, None)
            .expect("mint Untyped hẹp");
    let child =
        t.m.retype(slot(t.r, 1), ObjType::Tcb, 0, 0)
            .expect("Retype qua cap có WRITE");
    t.m.put_cap(t.b, 0, child).expect("slot Tcb con");
    let info = t.m.cap_info(child).expect("cap con");
    assert_eq!(info.rights, narrowed, "cap con phải kế thừa quyền nguồn");
    assert_eq!(info.parent, Some(u_ro));
    let u_child =
        t.m.retype(slot(t.r, 1), ObjType::Untyped, 12, 0)
            .expect("Retype Untyped con");
    t.m.put_cap(t.b, 1, u_child).expect("slot Untyped con");
    assert_eq!(
        t.m.cap_info(u_child).expect("cap Untyped con").rights,
        narrowed,
        "Untyped con cũng kế thừa quyền"
    );
    t.m.check_caps().expect("cây cap");
    t.m.check_all_bytes().expect("byte");
}

/// F3 — `Copy` không nới quyền; `copy_rights` chỉ nhận tập con.
#[test]
fn copy_giu_quyen_va_copy_rights_khong_noi_quyen() {
    let mut t = rig();
    let epcap = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, epcap).expect("cap Endpoint");
    let narrow = RIGHT_READ | RIGHT_GRANT;
    let ro =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), narrow, Some(9))
            .expect("mint Endpoint badge 9");
    let copied =
        t.m.copy(slot(t.r, 1), slot(t.b, 0))
            .expect("Copy giữ quyền");
    let ci = t.m.cap_info(copied).expect("cap copy");
    assert_eq!(ci.rights, narrow, "Copy giữ nguyên quyền");
    assert_eq!(ci.badge, Some(9), "Copy giữ nguyên badge");
    assert_eq!(ci.parent, Some(ro));

    let sub =
        t.m.copy_rights(slot(t.r, 1), slot(t.b, 1), Some(RIGHT_READ))
            .expect("Copy thu hẹp còn READ");
    assert_eq!(t.m.cap_info(sub).expect("cap sub").rights, RIGHT_READ);

    let before = t.m.cap_table().len();
    let live_before = t.m.live_cap_count();
    assert_eq!(
        t.m.copy_rights(slot(t.r, 1), slot(t.b, 2), Some(RIGHT_WRITE)),
        Err(KError::InsufficientRights),
        "Copy xin quyền ngoài nguồn phải InsufficientRights"
    );
    assert_eq!(t.m.cap_table().len(), before, "Copy lỗi không sinh cap");
    assert_eq!(t.m.live_cap_count(), live_before);
    let kept =
        t.m.copy_rights(slot(t.r, 1), slot(t.b, 2), None)
            .expect("Copy None giữ quyền");
    assert_eq!(kept, before as u32 + 1, "Copy None giữ nguyên quyền");
    assert_eq!(t.m.cap_info(kept).expect("cap kept").rights, narrow);
    t.m.check_caps().expect("cây cap");
}

/// F3 — `Mint(Some(0))` phải **xoá** badge nguồn, không âm thầm giữ lại.
#[test]
fn mint_badge_zero_xoa_badge_nguon() {
    let mut t = rig();
    let epcap = t.m.root_cap(t.ep);
    t.m.put_cap(t.r, 0, epcap).expect("cap Endpoint");
    let badged =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), RIGHTS_ALL, Some(7))
            .expect("mint badge 7");
    assert_eq!(t.m.cap_info(badged).expect("badged").badge, Some(7));
    let cleared =
        t.m.mint(slot(t.r, 1), slot(t.b, 0), RIGHTS_ALL, Some(0))
            .expect("mint Some(0)");
    assert_eq!(
        t.m.cap_info(cleared).expect("cleared").badge,
        None,
        "Some(0) phải xoá badge (giá trị 0 = không badge)"
    );
    // `None` mới là "giữ nguyên badge nguồn".
    let inherited =
        t.m.mint(slot(t.r, 1), slot(t.b, 1), RIGHTS_ALL, None)
            .expect("mint None");
    assert_eq!(
        t.m.cap_info(inherited).expect("inherited").badge,
        Some(7),
        "None giữ badge nguồn"
    );
    // Badge khác 0 chỉ hợp lệ cho Endpoint/Notification.
    let tcb = t.m.add_kernel_object(ObjType::Tcb);
    let tcb_cap = t.m.root_cap(tcb);
    t.m.put_cap(t.b, 2, tcb_cap).expect("cap Tcb");
    assert_eq!(
        t.m.mint(slot(t.b, 2), slot(t.b, 3), RIGHTS_ALL, Some(7)),
        Err(KError::InvalidArgument)
    );
    t.m.check_caps().expect("cây cap");
}

/// F3 — `Delete`/`Revoke` kiểm quyền của cap CNode dùng để triệu hồi.
#[test]
fn delete_revoke_kiem_quyen_cnode_cap() {
    let mut t = rig();
    let c0 = t.m.add_cnode(8);
    let c0_cap = t.m.root_cap(c0);
    t.m.put_cap(t.r, 0, c0_cap).expect("cap CNode c0");
    let epcap = t.m.root_cap(t.ep);
    t.m.put_cap(c0, 0, epcap).expect("cap Endpoint trong c0");
    let ro =
        t.m.mint(slot(t.r, 0), slot(t.r, 1), RIGHT_READ, None)
            .expect("cap CNode READ");
    assert_eq!(
        t.m.invoke_delete(slot(t.r, 1), slot(c0, 0)),
        Err(KError::InsufficientRights),
        "Delete qua cap CNode thiếu WRITE"
    );
    assert_eq!(
        t.m.invoke_revoke(slot(t.r, 1), None),
        Err(KError::InsufficientRights),
        "Revoke qua cap CNode thiếu WRITE"
    );
    // Đích không thuộc CNode được triệu hồi.
    assert_eq!(
        t.m.invoke_delete(slot(t.r, 0), slot(t.b, 0)),
        Err(KError::InvalidArgument)
    );
    // Cap CNode đủ quyền thì được.
    t.m.invoke_revoke(slot(t.r, 0), None).expect("revoke rỗng");
    t.m.invoke_delete(slot(t.r, 0), slot(c0, 0))
        .expect("delete qua cap WRITE");
    assert_eq!(t.m.cap_at(c0, 0), Err(KError::SlotEmpty));
    assert!(
        t.m.cap_info(ro).expect("cap READ").alive,
        "cap khác không bị đụng"
    );
    t.m.check_caps().expect("cây cap");
}

#[test]
fn move_di_chuyen_cap_va_dich_da_co_cap() {
    let mut t = rig();
    let epcap = t.m.root_cap(t.ep);
    t.m.put_cap(t.b, 0, epcap).expect("cap Endpoint B0");
    t.m.move_cap(slot(t.b, 0), slot(t.b, 1))
        .expect("move B0 → B1");
    assert_eq!(t.m.cap_at(t.b, 0), Err(KError::SlotEmpty), "nguồn rỗng");
    assert_eq!(t.m.cap_obj_at(t.b, 1).expect("đích"), t.ep);
    let moved = t.m.cap_at(t.b, 1).expect("cap di chuyển");
    assert_eq!(
        t.m.cap_info(moved).expect("cap").slot,
        Some(slot(t.b, 1)),
        "cap phải trỏ slot mới"
    );
    // Đích đã có cap: chuyển vào được (thay cap đích) — 01-cnode-copy.scn:13 yêu cầu `ok` (D2-44).
    let ep2 = t.m.add_kernel_object(ObjType::Endpoint);
    let ep2cap = t.m.root_cap(ep2);
    t.m.put_cap(t.b, 2, ep2cap).expect("cap Endpoint B2");
    let old_dst = t.m.copy(slot(t.b, 1), slot(t.b, 3)).expect("cap đích cũ");
    let grand =
        t.m.copy(slot(t.b, 3), slot(t.b, 4))
            .expect("cháu của cap đích");
    let moved_cap = t.m.cap_at(t.b, 1).expect("cap b1");
    t.m.move_cap(slot(t.b, 2), slot(t.b, 3))
        .expect("move vào slot có cap");
    assert!(
        !t.m.cap_info(old_dst).expect("cap cũ").alive,
        "cap đích cũ chết"
    );
    assert_eq!(t.m.cap_obj_at(t.b, 3).expect("đích"), ep2);
    assert_eq!(t.m.cap_at(t.b, 2), Err(KError::SlotEmpty));
    // Con của cap đích bị xoá phải reparent cho ông (không mồ côi).
    let gi = t.m.cap_info(grand).expect("cháu");
    assert!(gi.alive, "cháu phải sống");
    assert_eq!(gi.parent, Some(moved_cap), "cháu reparent cho ông");
    t.m.check_caps().expect("không mồ côi sau Move overwrite");
    assert_eq!(
        t.m.move_cap(slot(t.b, 2), slot(t.b, 0)),
        Err(KError::SlotEmpty),
        "nguồn rỗng phải SlotEmpty"
    );
    t.m.check_caps().expect("cây cap");
}

#[test]
fn revoke_untyped_finalise_ca_cay_con() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let u_child =
        t.m.retype(slot(t.r, 0), ObjType::Untyped, 12, 0)
            .expect("Untyped con 4 KiB");
    t.m.put_cap(t.r, 1, u_child).expect("cap Untyped con");
    let tcb_cap =
        t.m.retype(slot(t.r, 1), ObjType::Tcb, 0, 0)
            .expect("Tcb trong Untyped con");
    t.m.put_cap(t.b, 0, tcb_cap).expect("cap Tcb");
    let cnode_cap =
        t.m.retype(slot(t.r, 1), ObjType::CNode, 4, 0)
            .expect("CNode trong Untyped con");
    t.m.put_cap(t.b, 1, cnode_cap).expect("cap CNode");
    let tcb = t.m.cap_obj(tcb_cap).expect("object Tcb");
    let cnode = t.m.cap_obj(cnode_cap).expect("object CNode");
    let u_child_obj = t.m.cap_obj(u_child).expect("object Untyped con");
    let ep = t.m.add_kernel_object(ObjType::Endpoint);
    let epcap = t.m.root_cap(ep);
    t.m.put_cap(cnode, 0, epcap)
        .expect("cap Endpoint trong CNode");
    assert!(t.m.object_alive(cnode), "CNode sống trước revoke");
    t.m.check_caps().expect("cây trước revoke");
    let removed = t.m.revoke(t.r, Some(0)).expect("Revoke cap Untyped");
    assert!(removed >= 3, "phải xoá cap con/cháu, xoá {removed}");
    assert_eq!(t.m.untyped_used(t.u), 0, "watermark Untyped gốc về 0");
    assert!(!t.m.object_alive(u_child_obj), "Untyped con bị finalise");
    assert!(!t.m.object_alive(tcb), "Tcb bị finalise");
    assert!(!t.m.object_alive(cnode), "CNode bị finalise");
    assert!(
        t.m.object_alive(ep),
        "object ngoài Untyped còn sống (chỉ cap chết)"
    );
    assert!(
        !t.m.cap_info(epcap).expect("cap ep").alive,
        "cap trong CNode chết"
    );
    assert_eq!(t.m.cap_at(cnode, 0), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_at(t.r, 1), Err(KError::SlotEmpty));
    t.m.check_caps().expect("cây sau revoke");
    t.m.check_all_bytes().expect("byte sau revoke");
}

#[test]
fn revoke_chi_dung_pham_vi_cap_dich() {
    let mut t = rig();
    let ucap = t.m.root_cap(t.u);
    t.m.put_cap(t.r, 0, ucap).expect("cap Untyped");
    let a = t.m.retype(slot(t.r, 0), ObjType::Tcb, 0, 0).expect("A");
    t.m.put_cap(t.r, 1, a).expect("A ở R1");
    let b = t.m.copy(slot(t.r, 1), slot(t.b, 0)).expect("B con A");
    let c = t.m.copy(slot(t.r, 0), slot(t.b, 1)).expect("C con gốc");
    t.m.revoke(t.r, Some(1)).expect("revoke con của A");
    assert!(
        t.m.cap_info(a).expect("A").alive,
        "Revoke không xoá chính cap đích"
    );
    assert!(!t.m.cap_info(b).expect("B").alive, "B là con A ⇒ chết");
    assert!(
        t.m.cap_info(c).expect("C").alive,
        "C thuộc nhánh khác ⇒ còn sống"
    );
    t.m.check_caps().expect("cây cap");
}

#[test]
fn loi_va_ten_object_khop_spec() {
    for (i, (name, id)) in vifast_model::ERRORS.iter().enumerate() {
        let k = vifast_model::KError::from_index(i).expect("đủ 19 mã lỗi");
        assert_eq!(k.name(), *name, "tên mã lỗi {i}");
        assert_eq!(k.id(), *id, "số mã lỗi {i}");
    }
    assert_eq!(vifast_model::ERRORS.len(), 19);
    for (name, _id) in vifast_model::CORE_OBJECTS {
        assert!(ObjType::from_name(name).is_some(), "thiếu {name}");
    }
    let _: u8 = RIGHT_GRANT_REPLY | RIGHT_GRANT;
}
