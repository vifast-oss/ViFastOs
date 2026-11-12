// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-09 — Revoke qua CNode trên sim: đúng phạm vi con/cháu, không đụng cap khác.
//!
//! Cây cap dựng bằng `CNode.Copy`: `A1 (Endpoint) → B0 → C0` (con, cháu) và
//! `A0 (Untyped) → B1` (nhánh độc lập). `CNode.Revoke` xoá **con/cháu** của cap
//! trong slot, giữ cap gốc; slot bị xoá trả `SlotEmpty`, cap cũ (đã chết) trả
//! `InvalidCapability` — đúng bảng mã lỗi `spec/abi.toml`.

use vifast_sim::kobj::{KError, Machine, ObjType, SlotRef};

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

struct Tree {
    m: Machine,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    u0: u32,
    ep: u32,
    other: u32,
    b0: u32,
    c0: u32,
    b1: u32,
}

fn tree() -> Tree {
    let mut m = Machine::new();
    let a = m.add_cnode(8);
    let b = m.add_cnode(4);
    let c = m.add_cnode(4);
    let d = m.add_cnode(4);
    let u0 = m.add_untyped(0x1000, 8192);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let other = m.add_kernel_object(ObjType::Endpoint);

    let u_cap = m.root_cap(u0);
    m.put_cap(a, 0, u_cap).expect("cap Untyped ở A0");
    let ep_cap = m.root_cap(ep);
    m.put_cap(a, 1, ep_cap).expect("cap Endpoint ở A1");
    let other_cap = m.root_cap(other);
    m.put_cap(d, 0, other_cap).expect("cap Endpoint khác ở D0");

    let b0 = m.copy(slot(a, 1), slot(b, 0)).expect("A1 → B0");
    let c0 = m.copy(slot(b, 0), slot(c, 0)).expect("B0 → C0");
    let b1 = m.copy(slot(a, 0), slot(b, 1)).expect("A0 → B1");
    Tree {
        m,
        a,
        b,
        c,
        d,
        u0,
        ep,
        other,
        b0,
        c0,
        b1,
    }
}

#[test]
fn revoke_chi_xoa_con_chau_cua_cap() {
    let mut t = tree();
    let deleted = t.m.revoke(t.a, Some(1)).expect("revoke cap Endpoint ở A1");
    assert_eq!(deleted, 2, "phải xoá đúng B0 (con) và C0 (cháu)");

    // Con/cháu bị xoá: slot rỗng, cap cũ chết.
    assert_eq!(t.m.cap_at(t.b, 0), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_at(t.c, 0), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_obj(t.b0), Err(KError::InvalidCapability));
    assert_eq!(t.m.cap_obj(t.c0), Err(KError::InvalidCapability));

    // Cap gốc và nhánh khác không bị đụng.
    assert!(t.m.cap_at(t.a, 1).is_ok(), "cap gốc A1 phải còn");
    assert!(t.m.cap_at(t.b, 1).is_ok(), "nhánh A0 → B1 phải còn");
    assert!(t.m.object_alive(t.ep), "object Endpoint không bị finalise");
    assert_eq!(t.m.untyped_used(t.u0), 0);
    t.m.check_bytes(t.u0).expect("bất biến byte");

    // Object của cap gốc vẫn dùng được.
    assert_eq!(t.m.cap_obj(t.b1), Ok(t.u0));
}

#[test]
fn revoke_khong_slot_xoa_moi_con_trong_cnode() {
    let mut t = tree();
    let deleted = t.m.revoke(t.a, None).expect("revoke toàn bộ cap trong A");
    assert_eq!(deleted, 3, "B0, C0 (dưới A1) + B1 (dưới A0)");
    assert_eq!(t.m.cap_at(t.b, 0), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_at(t.b, 1), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_at(t.c, 0), Err(KError::SlotEmpty));
    assert!(t.m.cap_at(t.a, 0).is_ok(), "cap Untyped gốc phải còn");
    assert!(t.m.cap_at(t.a, 1).is_ok(), "cap Endpoint gốc phải còn");
    t.m.check_bytes(t.u0).expect("bất biến byte");
}

#[test]
fn revoke_none_khong_loi_khi_cap_long_trong_cung_cnode() {
    let mut m = Machine::new();
    let a = m.add_cnode(4);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let cap = m.root_cap(ep);
    m.put_cap(a, 0, cap).expect("cap gốc");
    m.copy(SlotRef::new(a, 0), SlotRef::new(a, 1))
        .expect("con trong cùng CNode");
    // A1 là con của A0: revoke(None) phải xoá A1 một lần, không lỗi vì cap đã chết.
    assert_eq!(m.revoke(a, None).expect("revoke không slot"), 1);
    assert_eq!(m.cap_at(a, 1), Err(KError::SlotEmpty));
    assert!(m.cap_at(a, 0).is_ok(), "cap gốc phải còn");
}

#[test]
fn revoke_slot_rong_hoac_ngoai_dai() {
    let mut t = tree();
    assert_eq!(t.m.revoke(t.a, Some(5)), Err(KError::SlotEmpty));
    assert_eq!(t.m.revoke(t.a, Some(99)), Err(KError::RangeError));
    assert_eq!(t.m.revoke(t.a, Some(0)).expect("chỉ có B1"), 1);
    assert_eq!(t.m.cap_at(t.b, 1), Err(KError::SlotEmpty));
}

#[test]
fn revoke_khong_dung_cay_cap_khac() {
    let mut t = tree();
    t.m.revoke(t.a, Some(1)).expect("revoke cây A1");
    assert!(t.m.cap_at(t.d, 0).is_ok(), "cây D0 phải nguyên");
    assert!(t.m.object_alive(t.other));
    // Cây D vẫn copy/revoke được bình thường.
    let child = t.m.copy(slot(t.d, 0), slot(t.d, 1)).expect("copy trong D");
    assert!(t.m.cap_obj(child).is_ok());
    t.m.revoke(t.d, Some(0)).expect("revoke D0");
    assert_eq!(t.m.cap_at(t.d, 1), Err(KError::SlotEmpty));
}

#[test]
fn copy_tra_dung_ma_loi() {
    let mut t = tree();
    // Đích đã có cap.
    assert_eq!(
        t.m.copy(slot(t.a, 1), slot(t.b, 0)),
        Err(KError::SlotOccupied)
    );
    // Nguồn rỗng / ngoài dải.
    assert_eq!(t.m.copy(slot(t.a, 3), slot(t.b, 2)), Err(KError::SlotEmpty));
    assert_eq!(
        t.m.copy(slot(t.a, 99), slot(t.b, 2)),
        Err(KError::RangeError)
    );
    // Nguồn là cap đã chết.
    t.m.revoke(t.a, Some(1)).expect("revoke A1");
    assert_eq!(t.m.copy(slot(t.b, 0), slot(t.b, 2)), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_obj(t.b0), Err(KError::InvalidCapability));
}

#[test]
fn delete_roi_dung_lai_cap_cu() {
    let mut t = tree();
    t.m.delete(slot(t.a, 1)).expect("xoá A1");
    assert_eq!(t.m.cap_at(t.a, 1), Err(KError::SlotEmpty));
    assert_eq!(t.m.cap_obj(t.c0), Ok(t.ep), "con của cap bị xoá vẫn sống");
    assert_eq!(t.m.delete(slot(t.a, 1)), Err(KError::SlotEmpty));
    // Con của cap đã xoá vẫn revoke được qua chính nó.
    assert_eq!(t.m.revoke(t.b, Some(0)).expect("revoke B0"), 1);
    assert_eq!(t.m.cap_at(t.c, 0), Err(KError::SlotEmpty));
}
