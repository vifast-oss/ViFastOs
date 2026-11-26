// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-417d** — tách khỏi `invoke.rs` để giữ luật R1 (`≤ 1.500` dòng/tệp, KT-331):
//! test host của `invoke` (cờ map, id object sinh từ spec, danh sách trắng, `Retype` VM,
//! `drop_root_cap`) — nội dung **không đổi**, chỉ đổi vị trí tệp.
#![cfg(test)]

use super::*;

/// Cờ map: `R`/`W ⇒ R`/`X`/`U`/`Device` hợp lệ; 0 quyền, `W` không `R`, bit lạ ⇒
/// `InvalidArgument` (5).
#[test]
fn map_flags_kiem_bien() {
    assert_eq!(map_flags(MAP_READ), Ok(MAP_READ));
    assert_eq!(map_flags(MAP_READ | MAP_WRITE), Ok(MAP_READ | MAP_WRITE));
    assert_eq!(map_flags(MAP_READ | MAP_EXEC), Ok(MAP_READ | MAP_EXEC));
    assert_eq!(
        map_flags(MAP_READ | MAP_WRITE | MAP_USER),
        Ok(MAP_READ | MAP_WRITE | MAP_USER)
    );
    assert_eq!(
        map_flags(MAP_READ | MAP_WRITE | MAP_DEVICE),
        Ok(MAP_READ | MAP_WRITE | MAP_DEVICE)
    );
    assert_eq!(map_flags(0), Err(KError::InvalidArgument));
    assert_eq!(map_flags(MAP_WRITE), Err(KError::InvalidArgument));
    assert_eq!(map_flags(MAP_READ | (1 << 5)), Err(KError::InvalidArgument));
}

/// Id object lấy từ `spec/abi.toml` (`vifast_abi`), không chép số: Frame 11, PageTable 10,
/// VSpace 9; loại lõi (Tcb) không có id trong bảng VM.
#[test]
fn type_id_sinh_tu_spec() {
    assert_eq!(type_id(ObjType::Frame), object_id("Frame"));
    assert_eq!(type_id(ObjType::PageTable), object_id("PageTable"));
    assert_eq!(type_id(ObjType::VSpace), object_id("VSpace"));
    assert_eq!(object_id("Frame"), Some(11));
    assert_eq!(object_id("PageTable"), Some(10));
    assert_eq!(object_id("VSpace"), Some(9));
    assert_eq!(type_id(ObjType::Tcb), None);
}

/// Danh sách trắng hẹp: năm cặp VM (Untyped.Retype, Frame.Map/Unmap, PageTable.Map/Unmap) + bảy cặp
/// `CNode` (`Copy/Mint/Move/Mutate/Delete/Revoke/Rotate`) + các cặp `Tcb` đã cài
/// (`Configure/SetTls/Resume/Suspend/BindNotification/UnbindNotification`); method **không**
/// có trong `[object]` (vd `CNode` 7) và mọi cặp khác **không** nằm trong danh sách
/// (⇒ `NotSupported`).
#[test]
fn danh_sach_trang_hep() {
    assert!(is_whitelisted(ObjType::Untyped, METHOD_RETYPE));
    assert!(is_whitelisted(ObjType::Frame, METHOD_MAP));
    assert!(is_whitelisted(ObjType::Frame, METHOD_UNMAP));
    // **KT-417d**: `PageTable.{Map,Unmap}` đã cài — chỉ đúng hai method ấy.
    assert!(is_whitelisted(ObjType::PageTable, METHOD_MAP));
    assert!(is_whitelisted(ObjType::PageTable, METHOD_UNMAP));
    assert!(!is_whitelisted(ObjType::PageTable, 2));
    for method in [
        CNODE_COPY,
        CNODE_MINT,
        CNODE_MOVE,
        CNODE_MUTATE,
        CNODE_DELETE,
        CNODE_REVOKE,
        CNODE_ROTATE,
    ] {
        assert!(is_whitelisted(ObjType::CNode, method));
    }
    // **KT-417**: `Mutate` (3) / `Rotate` (6) **đã cài**; method 7 vẫn ngoài bảng `[object]`.
    assert!(is_whitelisted(ObjType::CNode, 3));
    assert!(is_whitelisted(ObjType::CNode, 6));
    assert!(!is_whitelisted(ObjType::CNode, 7));
    assert!(!is_whitelisted(ObjType::Frame, 2));
    assert!(!is_whitelisted(ObjType::Untyped, 7));
    assert!(!is_whitelisted(ObjType::VSpace, METHOD_MAP));
    assert!(!is_whitelisted(ObjType::Endpoint, METHOD_RETYPE));
    // **KT-290**: `Tcb.Configure` (0) **cài**; **KT-319**: `Tcb.Suspend` (3) **cài**;
    // **KT-417**: `BindNotification` (13) / `UnbindNotification` (14) **cài**; method khác
    // của Tcb **chưa** cài.
    assert!(is_whitelisted(ObjType::Tcb, TCB_CONFIGURE));
    assert!(is_whitelisted(ObjType::Tcb, TCB_SUSPEND));
    assert!(is_whitelisted(ObjType::Tcb, TCB_BIND_NOTIFICATION));
    assert!(is_whitelisted(ObjType::Tcb, TCB_UNBIND_NOTIFICATION));
    assert!(!is_whitelisted(ObjType::Tcb, 1), "Tcb.SetFaultEp chưa cài");
    assert!(!is_whitelisted(ObjType::Tcb, 4), "Tcb.SetPriority chưa cài");
    assert!(!is_whitelisted(ObjType::Tcb, vifast_abi::TCB_EXEC));
}

/// Số method `CNode` **khớp `spec/abi.toml`** `[object].CNode` — đọc từ spec sinh
/// (`vifast-abi`), không chép tay: `Copy=0, Mint=1, Move=2, Mutate=3, Delete=4, Revoke=5,
/// Rotate=6`. Đây là chốt chống "tự đặt số" (yêu cầu KT-68).
#[test]
fn so_method_cnode_khop_spec() {
    assert_eq!(
        vifast_abi::CNODE_METHODS,
        [
            ("Copy", 0),
            ("Mint", 1),
            ("Move", 2),
            ("Mutate", 3),
            ("Delete", 4),
            ("Revoke", 5),
            ("Rotate", 6),
        ]
    );
    assert_eq!(CNODE_COPY, 0);
    assert_eq!(CNODE_MINT, 1);
    assert_eq!(CNODE_MOVE, 2);
    assert_eq!(CNODE_DELETE, 4);
    assert_eq!(CNODE_REVOKE, 5);
}

/// `Retype` chỉ nhận ba loại VM của object layer (D2-83); loại lõi do core tự cấp — không
/// đi qua invocation.
#[test]
fn retype_chi_ba_loai_vm() {
    assert!(retype_supported(ObjType::Frame));
    assert!(retype_supported(ObjType::PageTable));
    assert!(retype_supported(ObjType::VSpace));
    assert!(!retype_supported(ObjType::Untyped));
    assert!(!retype_supported(ObjType::Tcb));
    assert!(!retype_supported(ObjType::Endpoint));
}

/// **L2**: `drop_root_cap` chỉ bỏ cap **chưa vào slot**; cap đang ở slot (hoặc đã chết) ⇒
/// `IllegalOperation`. Sau khi bỏ, `check_caps` vẫn sạch (không cap mồ côi).
#[test]
fn drop_root_cap_chi_cap_chua_vao_slot() {
    let mut m = Machine::<crate::store::StaticStore, crate::observer::TestObserver>::new();
    let root = m.add_cnode(8);
    let u = m.add_untyped(0x1000, 1 << 16);
    let ucap = m.root_cap(u);
    let src = SlotRef::new(root, 0);
    m.put_cap(root, 0, ucap).expect("cap untyped");
    let cap = m.retype(src, ObjType::Tcb, 0, 0).expect("retype tcb");
    assert!(m.drop_root_cap(cap).is_ok());
    assert!(m.check_caps().is_ok());
    assert_eq!(
        m.drop_root_cap(cap),
        Err(KError::IllegalOperation),
        "cap đã chết"
    );
    let cap2 = m.retype(src, ObjType::Tcb, 0, 0).expect("retype tcb 2");
    m.put_cap(root, 1, cap2).expect("put cap");
    assert_eq!(
        m.drop_root_cap(cap2),
        Err(KError::IllegalOperation),
        "cap đang ở slot phải dùng delete"
    );
    assert!(m.check_caps().is_ok());
}

/// Số đếm `# inv:` chỉ tăng (TAP đọc **delta**, không phụ thuộc mốc cũ).
#[test]
fn so_dem_chi_tang() {
    let before = stats();
    bump(&DENIED);
    let after = stats();
    assert!(after.denied > before.denied);
    assert_eq!(after.allowed, before.allowed);
}
