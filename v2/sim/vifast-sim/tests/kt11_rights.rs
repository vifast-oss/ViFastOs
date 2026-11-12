// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-11 — F3 (review KT-10) trên mô hình của sim: `Copy` có tham số `rights?` của spec
//! (`[method_signature."CNode.Copy"] params = ["cnode","src","dst","rights?"]`) và **không
//! được nới quyền**. Viết **trước** khi thêm `copy_rights` (D2-05) ⇒ đỏ vì chưa có API.

use vifast_sim::kobj::{
    KError, Machine, ObjType, SlotRef, RIGHTS_ALL, RIGHT_GRANT, RIGHT_READ, RIGHT_WRITE,
};

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

#[test]
fn copy_rights_giu_hoac_thu_hep_quyen() {
    let mut m = Machine::new();
    let r = m.add_cnode(8);
    let b = m.add_cnode(8);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let root = m.root_cap(ep);
    m.put_cap(r, 0, root).expect("cap gốc");
    let narrow = RIGHT_READ | RIGHT_GRANT;
    let ro = m
        .mint(slot(r, 0), slot(r, 1), narrow, Some(3))
        .expect("mint hẹp");
    // `None` = giữ nguyên quyền + badge nguồn.
    let kept = m
        .copy_rights(slot(r, 1), slot(b, 0), None)
        .expect("copy_rights None");
    let info = m.cap_info(kept).expect("cap");
    assert_eq!(info.rights, narrow);
    assert_eq!(info.badge, Some(3));
    // Tập con: chỉ READ.
    let sub = m
        .copy_rights(slot(r, 1), slot(b, 1), Some(RIGHT_READ))
        .expect("copy_rights READ");
    assert_eq!(m.cap_info(sub).expect("cap").rights, RIGHT_READ);
    // Nới quyền: InsufficientRights, không sinh cap.
    let before = m.cap_table().len();
    assert_eq!(
        m.copy_rights(slot(r, 1), slot(b, 2), Some(RIGHT_WRITE)),
        Err(KError::InsufficientRights)
    );
    assert_eq!(m.cap_table().len(), before);
    assert_eq!(m.cap_info(ro).expect("nguồn").rights, narrow);
    m.check_caps().expect("cây cap");
    // `copy` cũ = `copy_rights(None)`.
    let old = m.copy(slot(r, 1), slot(b, 2)).expect("copy cũ");
    assert_eq!(m.cap_info(old).expect("cap").rights, narrow);
    assert_eq!(m.cap_info(old).expect("cap").badge, Some(3));
    let _ = RIGHTS_ALL;
}
