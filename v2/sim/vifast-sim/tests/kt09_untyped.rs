// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-09 — Untyped trên sim: watermark, invariant tổng byte, mã lỗi Retype.
//!
//! Viết **trước** khi có `vifast_sim::kobj` (D2-05): chạy `cargo test --test kt09_untyped`
//! phải đỏ vì thiếu mô hình, rồi mới hiện thực.
//!
//! Bất biến trung tâm (thiết kế §9.6 mục 9): `byte đã cấp + byte còn trống == cỡ Untyped ban đầu`,
//! không byte nào bị tính hai lần, watermark chỉ tiến trong một chuỗi Retype.

use vifast_sim::kobj::{KError, Machine, ObjType, SlotRef, FLAG_GUEST};

const BASE: u64 = 0x1000;

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

/// `(machine, cnode gốc, untyped)` — slot 0 của cnode giữ cap Untyped.
fn setup(size: usize) -> (Machine, u32, u32) {
    let mut m = Machine::new();
    let c0 = m.add_cnode(8);
    let u0 = m.add_untyped(BASE, size);
    let cap = m.root_cap(u0);
    m.put_cap(c0, 0, cap).expect("đặt cap Untyped vào slot 0");
    (m, c0, u0)
}

#[test]
fn tong_byte_bat_bien_sau_moi_retype() {
    let (mut m, c0, u0) = setup(8192);
    let src = slot(c0, 0);
    let plan = [
        (ObjType::Tcb, 512usize),
        (ObjType::Endpoint, 64),
        (ObjType::Reply, 64),
        (ObjType::SchedContext, 256),
    ];
    let mut want_used = 0usize;
    for (ty, bytes) in plan {
        let cap = m
            .retype(src, ty, 0, 0)
            .unwrap_or_else(|e| panic!("retype {ty:?} phải ok, gặp {e:?}"));
        want_used += bytes;
        let obj = m.cap_obj(cap).expect("cap mới phải sống");
        let (offset, len) = m.object_bytes(obj).expect("object phải có vùng nhớ");
        assert_eq!(len, bytes, "cỡ object {ty:?}");
        assert_eq!(
            offset + len,
            want_used,
            "object phải xếp liền sau watermark"
        );
        assert_eq!(m.untyped_used(u0), want_used);
        assert_eq!(m.untyped_free(u0), 8192 - want_used);
        m.check_bytes(u0)
            .unwrap_or_else(|e| panic!("bất biến byte vỡ sau {ty:?}: {e}"));
    }
    assert_eq!(m.untyped_used(u0), 896);
    assert_eq!(m.untyped_free(u0), 8192 - 896);
}

#[test]
fn watermark_chi_tien_va_het_cho_khong_ro_byte() {
    let (mut m, c0, u0) = setup(1024);
    let src = slot(c0, 0);
    let mut last = 0usize;
    for ty in [
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::SchedContext,
        ObjType::Endpoint,
        ObjType::Reply,
    ] {
        m.retype(src, ty, 0, 0).expect("còn chỗ");
        let used = m.untyped_used(u0);
        assert!(used >= last, "watermark phải tiến: {last} -> {used}");
        last = used;
        m.check_bytes(u0).expect("bất biến byte");
    }
    assert_eq!(last, 1024, "vừa khít 1024 byte");

    // Hết chỗ: NotEnoughMemory, watermark **không đổi**, không rò byte.
    assert_eq!(
        m.retype(src, ObjType::Endpoint, 0, 0),
        Err(KError::NotEnoughMemory)
    );
    assert_eq!(m.untyped_used(u0), 1024);
    assert_eq!(m.untyped_free(u0), 0);
    m.check_bytes(u0).expect("bất biến byte sau khi hết chỗ");
}

#[test]
fn retype_loi_tra_dung_ma() {
    let (mut m, c0, _u0) = setup(4096);
    let src = slot(c0, 0);

    // Slot rỗng / ngoài dải.
    assert_eq!(
        m.retype(slot(c0, 1), ObjType::Tcb, 0, 0),
        Err(KError::SlotEmpty)
    );
    assert_eq!(
        m.retype(slot(c0, 9), ObjType::Tcb, 0, 0),
        Err(KError::RangeError)
    );

    // Cap không phải Untyped.
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep_cap = m.root_cap(ep);
    m.put_cap(c0, 2, ep_cap).expect("đặt cap Endpoint");
    assert_eq!(
        m.retype(slot(c0, 2), ObjType::Tcb, 0, 0),
        Err(KError::WrongObjectType)
    );

    // size_bits lạ cho object cố định: nhỏ hơn cỡ ⇒ Alignment, lớn hơn ⇒ InvalidArgument.
    assert_eq!(
        m.retype(src, ObjType::Tcb, 3, 0),
        Err(KError::Alignment),
        "size_bits=3 (8 B) < 512 B ⇒ Alignment"
    );
    assert_eq!(
        m.retype(src, ObjType::Tcb, 12, 0),
        Err(KError::InvalidArgument),
        "size_bits=12 (4096 B) > 512 B ⇒ InvalidArgument"
    );

    // Object reserved chưa cài ⇒ NotSupported; GUEST chỉ hợp lệ với VSpace (D2-32).
    assert_eq!(
        m.retype(src, ObjType::VSpace, 0, 0),
        Err(KError::NotSupported)
    );
    assert_eq!(
        m.retype(src, ObjType::VSpace, 0, FLAG_GUEST),
        Err(KError::NotSupported),
        "VSpace + GUEST vẫn NotSupported tới H1 (D2-32)"
    );
    assert_eq!(
        m.retype(src, ObjType::Tcb, 0, FLAG_GUEST),
        Err(KError::InvalidArgument),
        "GUEST ngoài VSpace là tham số sai"
    );

    // Untyped con: tối thiểu 4096 B; quá lớn ⇒ NotEnoughMemory; size_bits vô lý ⇒ InvalidArgument.
    assert_eq!(
        m.retype(src, ObjType::Untyped, 11, 0),
        Err(KError::InvalidArgument)
    );
    assert_eq!(
        m.retype(src, ObjType::Untyped, 64, 0),
        Err(KError::InvalidArgument),
        "size_bits=64 không được panic tràn shift"
    );
    assert_eq!(
        m.retype(src, ObjType::Untyped, 13, 0),
        Err(KError::NotEnoughMemory)
    );
    m.check_bytes(_u0).expect("lỗi không được đốt byte");
    assert_eq!(m.untyped_used(_u0), 0, "mọi lần lỗi phải không cấp byte");
}

#[test]
fn untyped_con_duoc_can_le_va_tinh_du_byte() {
    let (mut m, c0, u0) = setup(8192);
    let src = slot(c0, 0);
    m.retype(src, ObjType::Tcb, 0, 0).expect("TCB 512 B");
    let child = m
        .retype(src, ObjType::Untyped, 12, 0)
        .expect("Untyped con 4096 B");
    let obj = m.cap_obj(child).expect("cap con sống");
    let (offset, len) = m.object_bytes(obj).expect("vùng nhớ con");
    assert_eq!((offset, len), (4096, 4096), "con 4096 B phải căn 4096");
    assert_eq!(offset % 4096, 0);
    assert_eq!(
        m.untyped_used(u0),
        8192,
        "khoảng đệm căn lề vẫn tính là đã cấp"
    );
    assert_eq!(m.untyped_free(u0), 0);
    m.check_bytes(u0)
        .expect("bất biến byte có khoảng đệm căn lề");
    assert_eq!(
        m.retype(src, ObjType::Untyped, 12, 0),
        Err(KError::NotEnoughMemory)
    );
}

#[test]
fn revoke_untyped_tra_byte_ve_0_va_lam_tiep_duoc() {
    let (mut m, c0, u0) = setup(8192);
    let src = slot(c0, 0);
    let tcb = m.retype(src, ObjType::Tcb, 0, 0).expect("TCB");
    let ep = m.retype(src, ObjType::Endpoint, 0, 0).expect("Endpoint");
    m.put_cap(c0, 3, tcb).expect("cap TCB");
    m.put_cap(c0, 4, ep).expect("cap Endpoint");
    assert_eq!(m.untyped_used(u0), 576);

    let deleted = m.revoke(c0, Some(0)).expect("revoke cap Untyped");
    assert_eq!(deleted, 2, "phải xoá đúng 2 cap con (TCB, Endpoint)");
    assert_eq!(m.untyped_used(u0), 0, "revoke Untyped đặt watermark về 0");
    assert_eq!(m.untyped_free(u0), 8192);
    m.check_bytes(u0).expect("bất biến byte sau revoke");

    // Cap cũ đã chết: dùng lại trả InvalidCapability, slot thì SlotEmpty.
    assert_eq!(m.cap_obj(tcb), Err(KError::InvalidCapability));
    assert_eq!(m.cap_at(c0, 3), Err(KError::SlotEmpty));

    // Làm tiếp được: cấp lại từ đầu, byte không bị nhân đôi.
    let tcb2 = m
        .retype(src, ObjType::Tcb, 0, 0)
        .expect("retype lại sau revoke");
    let obj2 = m.cap_obj(tcb2).expect("cap mới sống");
    assert_eq!(m.object_bytes(obj2), Some((0, 512)));
    assert_eq!(m.untyped_used(u0), 512);
    m.check_bytes(u0).expect("bất biến byte sau khi cấp lại");
}

#[test]
fn double_free_bi_bat() {
    let (mut m, c0, u0) = setup(4096);
    let src = slot(c0, 0);
    let ep = m.retype(src, ObjType::Endpoint, 0, 0).expect("Endpoint");
    let ep_obj = m.cap_obj(ep).expect("object Endpoint");
    m.revoke(c0, Some(0)).expect("revoke lần 1");
    assert_eq!(m.untyped_used(u0), 0);

    // Xoá cap Untyped rồi revoke lại: phải là SlotEmpty, không free lần hai.
    m.delete(src).expect("xoá cap Untyped");
    assert_eq!(m.revoke(c0, Some(0)), Err(KError::SlotEmpty));
    assert_eq!(m.untyped_used(u0), 0, "không được free lần hai");
    m.check_bytes(u0).expect("bất biến byte sau double-free");

    // Cap của object đã bị finalise không dùng lại được.
    assert_eq!(m.cap_obj(ep), Err(KError::InvalidCapability));
    assert!(!m.object_alive(ep_obj));
}

#[test]
fn lap_lai_retype_va_revoke_khong_ro_byte() {
    let (mut m, c0, u0) = setup(4096);
    let src = slot(c0, 0);
    for vòng in 0..10 {
        m.retype(src, ObjType::Endpoint, 0, 0)
            .unwrap_or_else(|e| panic!("vòng {vòng}: Endpoint {e:?}"));
        m.retype(src, ObjType::Reply, 0, 0)
            .unwrap_or_else(|e| panic!("vòng {vòng}: Reply {e:?}"));
        assert_eq!(m.untyped_used(u0), 128);
        m.check_bytes(u0).expect("bất biến byte trong vòng");
        m.revoke(c0, Some(0))
            .unwrap_or_else(|e| panic!("vòng {vòng}: revoke {e:?}"));
        assert_eq!(m.untyped_used(u0), 0, "vòng {vòng}: phải trả hết byte");
        m.check_bytes(u0).expect("bất biến byte sau revoke");
    }
}
