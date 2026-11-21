// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-68** — test host cho `CNode.Copy/Mint/Move/Delete/Revoke` qua **invocation** (syscall 0)
//! và cho thuật toán **capalloc** (cấp từ slot rảnh → thu hồi → cấp lại **cùng slot**).
//!
//! Vì sao test host: đây là ngữ nghĩa **core** (MDB/cap/quyền). Bằng chứng QEMU nằm ở
//! `kernel/vifast-kbin-riscv64/src/ktest_capa.rs` (TAP 20..24, `# capa:`); test này chạy trên
//! mọi `cargo test -p vifast-core` — lớp chặn hồi quy nhanh trước khi boot.
//!
//! Ranh giới (model `vifast-model::invoke_*`, F3 — oracle):
//! - **nguồn** (`src`) là CPTR trong **CSpace của caller**;
//! - **đích** (`dst`) là slot trong **CNode của cap được gọi**;
//! - cap CNode được gọi phải có `RIGHT_WRITE`; `dst` phải nằm trong CNode đó.
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke;
use crate::machine::Machine;
use crate::spec::{RIGHTS_ALL, RIGHT_READ, RIGHT_WRITE};
use crate::syscall::SyscallEnv;

/// Hook arch tối thiểu — CNode invocation không dùng hook này (D2-83 chỉ VM mới cần).
struct NoEnv;

impl SyscallEnv for NoEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(11)
    }
    fn now_ns(&mut self) -> u64 {
        0
    }
    fn features(&mut self) -> u64 {
        0
    }
    fn num_harts(&mut self) -> u64 {
        1
    }
}

/// `MessageInfo` 35 bit: `length` = số MR dùng, `label` = method. `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

/// Gọi invocation trên cap ở `call_slot`, MR0..MR3 = `mrs`.
fn invoke_on(
    m: &mut Machine,
    tcb: ObjId,
    call_slot: SlotRef,
    label: u32,
    len: u8,
    mrs: [u64; 4],
) -> Result<(u64, u64), u64> {
    invoke::dispatch(m, tcb, call_slot, info(label, len), &mrs, &mut NoEnv)
}

fn cap_info_at(m: &Machine, cnode: ObjId, slot: usize) -> crate::error::CapInfo {
    let cap = m.cap_at(cnode, slot).expect("cap phai song");
    m.cap_info(cap).expect("cap info")
}

/// Cảnh **một tiến trình**: caller CSpace = `c` (CNode 16 slot), nên `src` và `dst` cùng nằm
/// trong `c` — dựng được cây MDB 2 tầng và `Move/Delete/Revoke` đầy đủ.
///
/// Slot: `0` self cap CNode (RIGHTS_ALL), `1` Endpoint, `2` TCB (object **không** nhận badge),
/// `10` Untyped. TCB có cspace = `c`.
struct SelfRig {
    m: Machine,
    tcb: ObjId,
    c: ObjId,
    self_cap: SlotRef,
    ep: u32,
}

fn rig_self() -> SelfRig {
    let mut m = Machine::new();
    let u = m.add_untyped(0x8200_0000, 1 << 16);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 10, u_cap).expect("cap untyped");
    let ep_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint");
    m.put_cap(c, 1, ep_cap).expect("cap endpoint");
    let tcb_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Tcb, 0, 0)
        .expect("retype tcb");
    m.put_cap(c, 2, tcb_cap).expect("cap tcb");
    m.tcb_configure(SlotRef::new(c, 2), Some(SlotRef::new(c, 0)))
        .expect("cspace = c");
    // Bỏ cap Untyped khỏi CNode sau khi retype: nếu để lại, `Revoke` không slot sẽ finalise
    // cả cây Untyped (E/TCB) — đúng ngữ nghĩa nhưng không phải thứ các ca dưới muốn đo.
    m.delete(SlotRef::new(c, 10)).expect("bo cap untyped");
    let tcb = m.cap_obj(tcb_cap).expect("obj tcb");
    let ep = m.cap_at(c, 1).expect("cap ep");
    SelfRig {
        m,
        tcb,
        c,
        self_cap: SlotRef::new(c, 0),
        ep,
    }
}

/// Cảnh **hai CSpace**: caller = `root` (server), target = `cnode` (client) — đúng mô hình
/// capalloc: server giữ cap nguồn, cấp bản sao vào CNode của client, `Revoke` cap nguồn xoá
/// **xuyên CNode**.
struct CrossRig {
    m: Machine,
    tcb: ObjId,
    root: ObjId,
    cnode: ObjId,
    u: ObjId,
    /// Cap CNode client trong CSpace server (root slot 3).
    cnode_cap: SlotRef,
    /// Cap Endpoint nguồn trong CSpace server (root slot 4).
    ep_slot: SlotRef,
    ep: u32,
}

fn rig_cross() -> CrossRig {
    let mut m = Machine::new();
    let root = m.add_cnode(32);
    let u = m.add_untyped(0x8200_0000, 1 << 16);
    let u_cap = m.root_cap(u);
    m.put_cap(root, 0, u_cap).expect("cap untyped");
    let tcb_cap = m
        .retype(SlotRef::new(root, 0), ObjType::Tcb, 0, 0)
        .expect("retype tcb");
    m.put_cap(root, 1, tcb_cap).expect("cap tcb");
    let cnode_self = m.root_cap(root);
    m.put_cap(root, 2, cnode_self).expect("cap cspace");
    m.tcb_configure(SlotRef::new(root, 1), Some(SlotRef::new(root, 2)))
        .expect("configure cspace");
    let tcb = m.cap_obj(tcb_cap).expect("obj tcb");
    let cnode = m.add_cnode(16);
    let cnode_self = m.root_cap(cnode);
    m.put_cap(cnode, 0, cnode_self).expect("cap cnode self");
    let cnode_cap = m.root_cap(cnode);
    m.put_cap(root, 3, cnode_cap).expect("cap cnode target");
    let ep_cap = m
        .retype(SlotRef::new(root, 0), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint");
    m.put_cap(root, 4, ep_cap).expect("cap endpoint");
    let ep = m.cap_at(root, 4).expect("cap ep");
    CrossRig {
        m,
        tcb,
        root,
        cnode,
        u,
        cnode_cap: SlotRef::new(root, 3),
        ep_slot: SlotRef::new(root, 4),
        ep,
    }
}

// ---------------------------------------------------------------------------------------------
// Copy / Mint — quyền + badge + MDB
// ---------------------------------------------------------------------------------------------

/// `CNode.Copy` (id 0): cap con **giữ nguyên** quyền + badge nguồn; cha MDB = cap nguồn.
#[test]
fn cnode_copy_giu_quyen_va_badge() {
    let mut r = rig_self();
    // Tạo nguồn có badge + quyền hẹp: Mint EP (slot 1) → slot 3.
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_MINT,
        4,
        [1, 3, u64::from(RIGHT_READ | RIGHT_WRITE), 0x77],
    )
    .expect("mint nguon");
    let src = cap_info_at(&r.m, r.c, 3);
    assert_eq!(src.badge, Some(0x77));
    assert_eq!(src.rights, RIGHT_READ | RIGHT_WRITE);
    // Copy slot 3 → slot 4: giữ quyền + badge, cha = cap nguồn.
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_COPY,
        2,
        [3, 4, 0, 0],
    )
    .expect("copy");
    let dst = cap_info_at(&r.m, r.c, 4);
    assert_eq!(dst.rights, src.rights, "Copy giữ quyền nguồn");
    assert_eq!(dst.badge, src.badge, "Copy giữ badge nguồn");
    assert_eq!(dst.obj, src.obj);
    assert_eq!(dst.parent, Some(src.cap), "cha MDB = cap nguồn");
    assert_eq!(src.slot, Some(SlotRef::new(r.c, 3)));
    assert!(r.m.check_caps().is_ok());
    assert!(invoke::stats().copy >= 1);
}

/// `CNode.Mint` (id 1): thu hẹp quyền + đặt badge (chỉ Endpoint/Notification); quyền **vượt**
/// nguồn ⇒ `InsufficientRights` (3); badge trên object khác ⇒ `InvalidArgument` (5); badge = 0
/// xoá badge.
#[test]
fn cnode_mint_badge_va_quyen() {
    let mut r = rig_self();
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_MINT,
        3,
        [1, 3, u64::from(RIGHT_READ), 0],
    )
    .expect("mint READ");
    let cap = cap_info_at(&r.m, r.c, 3);
    assert_eq!((cap.rights, cap.badge), (RIGHT_READ, None));
    // Vượt quyền **nguồn hẹp** (slot 3 chỉ READ) ⇒ 3.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_MINT,
            4,
            [3, 4, u64::from(RIGHT_READ | RIGHT_WRITE), 0xAB],
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    // Badge hợp lệ trên Endpoint.
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_MINT,
        4,
        [1, 4, u64::from(RIGHTS_ALL), 0xAB],
    )
    .expect("mint badge");
    assert_eq!(cap_info_at(&r.m, r.c, 4).badge, Some(0xAB));
    // badge = 0 ⇒ xoá badge.
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_MINT,
        4,
        [1, 5, u64::from(RIGHT_READ), 0],
    )
    .expect("mint badge 0");
    assert_eq!(cap_info_at(&r.m, r.c, 5).badge, None);
    // Badge trên TCB (không phải Endpoint/Notification) ⇒ 5.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_MINT,
            4,
            [2, 6, u64::from(RIGHTS_ALL), 1],
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert!(r.m.check_caps().is_ok());
}

/// `CNode.Move` (id 2): **xoá nguồn**, giữ `CapId`/cha MDB; đích có cap ⇒ cap đích bị xoá
/// trước (D2-44) và con của nó reparent lên ông; `src == dst` ⇒ `IllegalOperation` (17).
#[test]
fn cnode_move_xoa_nguon_va_de_cheo() {
    let mut r = rig_self();
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_COPY,
        2,
        [1, 3, 0, 0],
    )
    .expect("A");
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_COPY,
        2,
        [3, 4, 0, 0],
    )
    .expect("B");
    let a = cap_info_at(&r.m, r.c, 3);
    let b = cap_info_at(&r.m, r.c, 4);
    assert_eq!(b.parent, Some(a.cap));
    // Move B (slot 4) → slot 3 (đang giữ cha A): A bị xoá trước, B reparent lên ông (EP).
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_MOVE,
        2,
        [4, 3, 0, 0],
    )
    .expect("move");
    assert_eq!(
        r.m.cap_at(r.c, 4),
        Err(KError::SlotEmpty),
        "nguồn phải rỗng"
    );
    let moved = cap_info_at(&r.m, r.c, 3);
    assert_eq!(moved.cap, b.cap, "Move giữ nguyên CapId");
    assert_eq!(moved.parent, Some(r.ep), "con bị xoá reparent lên ông");
    assert_eq!(moved.slot, Some(SlotRef::new(r.c, 3)));
    assert!(r.m.check_caps().is_ok());
    // src == dst ⇒ IllegalOperation.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_MOVE,
            2,
            [3, 3, 0, 0]
        ),
        Err(KError::IllegalOperation.id() as u64)
    );
}

/// `CNode.Delete` (id 4): xoá cap, **object không bị finalise**, con reparent lên ông; ô rỗng
/// ⇒ `SlotEmpty` (8).
#[test]
fn cnode_delete_reparent_mdb() {
    let mut r = rig_self();
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_COPY,
        2,
        [1, 3, 0, 0],
    )
    .expect("A");
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_COPY,
        2,
        [3, 4, 0, 0],
    )
    .expect("B");
    let b = cap_info_at(&r.m, r.c, 4);
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_DELETE,
        1,
        [3, 0, 0, 0],
    )
    .expect("delete");
    assert_eq!(r.m.cap_at(r.c, 3), Err(KError::SlotEmpty));
    let after = cap_info_at(&r.m, r.c, 4);
    assert_eq!(after.cap, b.cap, "B không bị xoá theo");
    assert_eq!(after.parent, Some(r.ep), "B reparent lên ông");
    assert!(r.m.object_alive(after.obj), "Delete không finalise object");
    assert!(r.m.check_caps().is_ok());
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_DELETE,
            1,
            [3, 0, 0, 0]
        ),
        Err(KError::SlotEmpty.id() as u64)
    );
}

/// `CNode.Revoke` (id 5): xoá **đúng cây con** của slot (con + cháu), `a1` = số cap bị xoá,
/// **không** lan sang nhánh khác; `slot?` bỏ trống ⇒ mọi slot của CNode; cap trong slot được
/// giữ. Slot ngoài bảng ⇒ `RangeError` (19), ô rỗng ⇒ `SlotEmpty` (8).
#[test]
fn cnode_revoke_cay_hai_tang() {
    let mut r = rig_self();
    // Cây: 3 = A (con EP); 4 = B (con 3); 5 = C (con 4); 6 = D (con EP, nhánh khác).
    for (src, dst) in [(1u64, 3u64), (3, 4), (4, 5), (1, 6)] {
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            2,
            [src, dst, 0, 0],
        )
        .expect("dung cay");
    }
    let before = r.m.live_cap_count();
    let (err, deleted) = invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_REVOKE,
        1,
        [3, 0, 0, 0],
    )
    .expect("revoke slot 3");
    assert_eq!((err, deleted), (0, 2), "chỉ B và C bị xoá");
    assert_eq!(r.m.cap_at(r.c, 4), Err(KError::SlotEmpty));
    assert_eq!(r.m.cap_at(r.c, 5), Err(KError::SlotEmpty));
    assert!(r.m.cap_at(r.c, 3).is_ok(), "cap trong slot được giữ");
    assert!(r.m.cap_at(r.c, 6).is_ok(), "nhánh khác không bị lan");
    assert_eq!(r.m.live_cap_count(), before - 2);
    assert!(r.m.check_caps().is_ok());
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_REVOKE,
            1,
            [4, 0, 0, 0]
        ),
        Err(KError::SlotEmpty.id() as u64)
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_REVOKE,
            1,
            [99, 0, 0, 0]
        ),
        Err(KError::RangeError.id() as u64)
    );
    // `slot?` bỏ trống ⇒ revoke con/cháu của **mọi** slot (cảnh mới, cây đơn giản):
    // 3, 4 = hai con của EP (slot 1); revoke(None) xoá cả hai, giữ EP.
    let mut r2 = rig_self();
    for dst in [3u64, 4] {
        invoke_on(
            &mut r2.m,
            r2.tcb,
            r2.self_cap,
            invoke::CNODE_COPY,
            2,
            [1, dst, 0, 0],
        )
        .expect("con EP");
    }
    let (_, deleted) = invoke_on(
        &mut r2.m,
        r2.tcb,
        r2.self_cap,
        invoke::CNODE_REVOKE,
        0,
        [0; 4],
    )
    .expect("revoke all");
    assert_eq!(deleted, 2, "hai con của EP");
    assert_eq!(r2.m.cap_at(r2.c, 3), Err(KError::SlotEmpty));
    assert_eq!(r2.m.cap_at(r2.c, 4), Err(KError::SlotEmpty));
    assert!(r2.m.cap_at(r2.c, 1).is_ok(), "cap trong slot được giữ");
    assert!(r2.m.check_caps().is_ok());
}

// ---------------------------------------------------------------------------------------------
// Xuyên CNode (mô hình capalloc server → client) + cap chết
// ---------------------------------------------------------------------------------------------

/// Server cấp bản sao cap vào CNode **của client**; `Revoke` trên cap nguồn **của server** xoá
/// bản sao **xuyên CNode** (đúng cây MDB), không đụng nhánh khác.
#[test]
fn revoke_xuyen_cnode_theo_cay_mdb() {
    let mut r = rig_cross();
    // Cấp 2 bản sao vào client cnode (slot 1, 2) + 1 nhánh khác (Endpoint thứ hai? dùng EP).
    for (src, dst) in [(r.ep_slot.slot as u64, 1u64), (r.ep_slot.slot as u64, 2)] {
        invoke_on(
            &mut r.m,
            r.tcb,
            r.cnode_cap,
            invoke::CNODE_COPY,
            2,
            [src, dst, 0, 0],
        )
        .expect("cap vao client");
    }
    assert_eq!(cap_info_at(&r.m, r.cnode, 1).parent, Some(r.ep));
    assert_eq!(cap_info_at(&r.m, r.cnode, 2).parent, Some(r.ep));
    // Revoke trên **cap nguồn** của server (CNode của server = root, slot 4).
    let (_, deleted) = invoke_on(
        &mut r.m,
        r.tcb,
        SlotRef::new(r.root, 2),
        invoke::CNODE_REVOKE,
        1,
        [4, 0, 0, 0],
    )
    .expect("revoke nguon");
    assert_eq!(deleted, 2, "hai bản sao xuyên CNode");
    assert_eq!(r.m.cap_at(r.cnode, 1), Err(KError::SlotEmpty));
    assert_eq!(r.m.cap_at(r.cnode, 2), Err(KError::SlotEmpty));
    assert!(r.m.cap_at(r.root, 4).is_ok(), "cap nguồn được giữ");
    assert!(r.m.check_caps().is_ok());
}

/// **Cap chết** (không thể sinh qua ABI): cap đã chết đặt vào slot ⇒ `cap_at` trả
/// `InvalidCapability` (1) **và** `check_caps` báo "slot chua cap chet" (bất biến 3) — chứng
/// minh nhánh phòng thủ của invocation không bị bỏ quên, và trạng thái đó **không** sống qua
/// bất biến. Vì vậy ca âm trên QEMU dùng CPTR cũ sau Revoke/Delete ⇒ `SlotEmpty` (8).
#[test]
fn cap_chet_tra_invalid_capability_va_bat_bien_do() {
    let mut r = rig_self();
    let ep_obj = r.m.cap_obj(r.ep).expect("obj ep");
    let dead = r.m.root_cap(ep_obj);
    r.m.drop_root_cap(dead).expect("giet cap chua vao slot");
    assert!(!r.m.cap_info(dead).expect("info").alive);
    r.m.put_cap(r.c, 9, dead).expect("dat cap chet (co tinh)");
    assert_eq!(r.m.cap_at(r.c, 9), Err(KError::InvalidCapability));
    assert!(r.m.check_caps().is_err(), "bat bien phai bat slot cap chet");
    // Invocation đọc cap chết ⇒ 1 (không panic).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            2,
            [9, 11, 0, 0]
        ),
        Err(KError::InvalidCapability.id() as u64)
    );
}

// ---------------------------------------------------------------------------------------------
// Ca âm
// ---------------------------------------------------------------------------------------------

/// Ca âm: thiếu `RIGHT_WRITE` trên cap CNode ⇒ 3; đích đã có cap ⇒ `SlotOccupied` (7); quyền
/// vượt nguồn ⇒ 3; method chưa cài (`Mutate` 3 / `Rotate` 6) và gọi trên object khác ⇒
/// `NotSupported` (18, **có đếm**); `length` thiếu/thừa ⇒ `InvalidArgument` (5); slot ngoài
/// bảng ⇒ `RangeError` (19).
#[test]
fn cnode_ca_am_ma_loi() {
    let mut r = rig_self();
    // Cap CNode **chỉ READ** (slot 9) — gọi Copy phải trả 3 (model F3).
    r.m.copy_rights(r.self_cap, SlotRef::new(r.c, 9), Some(RIGHT_READ))
        .expect("cnode RO");
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            SlotRef::new(r.c, 9),
            invoke::CNODE_COPY,
            2,
            [1, 3, 0, 0],
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    // Đích đã có cap ⇒ 7.
    invoke_on(
        &mut r.m,
        r.tcb,
        r.self_cap,
        invoke::CNODE_COPY,
        2,
        [1, 3, 0, 0],
    )
    .expect("copy");
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            2,
            [1, 3, 0, 0]
        ),
        Err(KError::SlotOccupied.id() as u64)
    );
    // Quyền vượt nguồn ⇒ 3.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            3,
            [1, 4, 0xFF, 0],
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    // Method có số trong spec nhưng chưa cài ⇒ 18, **có đếm** `denied`.
    let denied0 = invoke::stats().denied;
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, r.self_cap, 3, 2, [1, 4, 0, 0]),
        Err(KError::NotSupported.id() as u64)
    );
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, r.self_cap, 6, 2, [1, 4, 0, 0]),
        Err(KError::NotSupported.id() as u64)
    );
    assert_eq!(invoke::stats().denied, denied0 + 2);
    // Gọi `CNode.Copy` trên cap **không phải CNode** (Endpoint, slot 1) ⇒ 18.
    // **KT-290**: trước đây ca này dùng cap TCB (slot 2) + method 0; từ KT-290 `(Tcb, 0)` là
    // `Tcb.Configure` **đã cài** nên không còn là "method ngoài danh sách" — đổi sang Endpoint
    // (vẫn giữ nguyên điều cần đo: cap sai loại ⇒ 18, không trả `Ok` giả).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            SlotRef::new(r.c, 1),
            invoke::CNODE_COPY,
            2,
            [1, 4, 0, 0],
        ),
        Err(KError::NotSupported.id() as u64)
    );
    // Thiếu MR (`length = 1`) ⇒ 5; `length = 5` (> 4 MR) ⇒ 5.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            1,
            [1, 4, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            5,
            [1, 4, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    // D2-104 — arity **chính xác**: thừa MR cho **từng** method cũng ⇒ 5 (không bỏ qua).
    // (Copy 2|3, Mint 2|3|4, Move 2, Delete 1, Revoke 0|1.)
    for (method, len) in [
        (invoke::CNODE_COPY, 4),
        (invoke::CNODE_MINT, 5),
        (invoke::CNODE_MOVE, 3),
        (invoke::CNODE_MOVE, 4),
        (invoke::CNODE_DELETE, 2),
        (invoke::CNODE_DELETE, 3),
        (invoke::CNODE_DELETE, 4),
        (invoke::CNODE_REVOKE, 2),
        (invoke::CNODE_REVOKE, 3),
        (invoke::CNODE_REVOKE, 4),
    ] {
        assert_eq!(
            invoke_on(&mut r.m, r.tcb, r.self_cap, method, len, [1, 4, 0, 0]),
            Err(KError::InvalidArgument.id() as u64),
            "method {method} len {len}"
        );
    }
    // Các ca thừa **không** được chạm máy trạng thái: EP slot 1 sống, đích 4 vẫn rỗng.
    assert!(r.m.cap_at(r.c, 1).is_ok());
    assert_eq!(r.m.cap_at(r.c, 4), Err(KError::SlotEmpty));
    // Slot đích ngoài bảng CNode (16 slot) ⇒ 19.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.self_cap,
            invoke::CNODE_COPY,
            2,
            [1, 40, 0, 0]
        ),
        Err(KError::RangeError.id() as u64)
    );
    assert!(r.m.check_caps().is_ok());
}

// ---------------------------------------------------------------------------------------------
// capalloc
// ---------------------------------------------------------------------------------------------

/// **capalloc** (xuyên CNode): quét slot rảnh bằng `Copy` (lỗi 7 = đã dùng) → cấp →
/// `Revoke`+`Delete` → slot trả lại → cấp lại **cùng slot**; `live_cap_count` và số slot rảnh
/// trở về mốc đầu (không rò cap/slot). Đây đúng thuật toán `capa_server` dùng trên QEMU.
#[test]
fn capalloc_cap_thu_hoi_cap_lai_cung_slot() {
    let mut r = rig_cross();
    let free_slots = |m: &Machine, cnode: ObjId| -> usize {
        (0..16)
            .filter(|s| m.cap_at(cnode, *s) == Err(KError::SlotEmpty))
            .count()
    };
    let base_live = r.m.live_cap_count();
    let base_free = free_slots(&r.m, r.cnode);
    assert_eq!(base_free, 15, "chỉ self cap ở slot 0");
    // **KT-74** — bộ đếm `invoke::stats()` là **toàn cục** và test chạy **song song**, nên khẳng
    // định `>= 4` cố định là **phụ thuộc thứ tự** (đo được: ca này FAIL một mình trên HEAD sạch
    // vì chưa có test nào khác cộng dồn). Đo **delta** của chính ca này: 2 `Copy` (2 lần `alloc`),
    // 2 `Revoke`, 2 `Delete` — bất biến đúng, không phụ thuộc lịch chạy.
    let base_stats = invoke::stats();
    let alloc = |m: &mut Machine, tcb: ObjId, cnode_cap: SlotRef, ep: u64| -> usize {
        for slot in 1..16u64 {
            match invoke_on(m, tcb, cnode_cap, invoke::CNODE_COPY, 2, [ep, slot, 0, 0]) {
                Ok((0, _)) => return slot as usize,
                Err(code) if code == KError::SlotOccupied.id() as u64 => continue,
                other => panic!("capalloc: loi bat ngo {other:?}"),
            }
        }
        panic!("capalloc: het slot");
    };
    let first = alloc(&mut r.m, r.tcb, r.cnode_cap, r.ep_slot.slot as u64);
    assert_eq!(first, 1, "slot rảnh đầu tiên");
    let granted = cap_info_at(&r.m, r.cnode, first);
    assert_eq!(granted.obj, r.m.cap_obj_at(r.root, 4).expect("ep obj"));
    assert_eq!(granted.parent, Some(r.ep));
    assert_eq!(r.m.live_cap_count(), base_live + 1);
    assert_eq!(free_slots(&r.m, r.cnode), base_free - 1);
    // Thu hồi: Revoke (cây con rỗng — 0 cap) rồi Delete (giải phóng slot).
    let (_, deleted) = invoke_on(
        &mut r.m,
        r.tcb,
        r.cnode_cap,
        invoke::CNODE_REVOKE,
        1,
        [first as u64, 0, 0, 0],
    )
    .expect("revoke");
    assert_eq!(deleted, 0);
    invoke_on(
        &mut r.m,
        r.tcb,
        r.cnode_cap,
        invoke::CNODE_DELETE,
        1,
        [first as u64, 0, 0, 0],
    )
    .expect("delete");
    assert_eq!(r.m.live_cap_count(), base_live);
    assert_eq!(free_slots(&r.m, r.cnode), base_free);
    assert_eq!(r.m.cap_at(r.cnode, first), Err(KError::SlotEmpty));
    // Cấp lại ⇒ **cùng slot** (không rò); thu hồi lần hai vẫn sạch.
    let second = alloc(&mut r.m, r.tcb, r.cnode_cap, r.ep_slot.slot as u64);
    assert_eq!(second, first, "cấp lại phải rơi đúng slot vừa trả");
    invoke_on(
        &mut r.m,
        r.tcb,
        r.cnode_cap,
        invoke::CNODE_REVOKE,
        1,
        [second as u64, 0, 0, 0],
    )
    .expect("revoke 2");
    invoke_on(
        &mut r.m,
        r.tcb,
        r.cnode_cap,
        invoke::CNODE_DELETE,
        1,
        [second as u64, 0, 0, 0],
    )
    .expect("delete 2");
    assert_eq!(r.m.live_cap_count(), base_live);
    assert_eq!(free_slots(&r.m, r.cnode), base_free);
    assert!(r.m.check_caps().is_ok());
    assert!(r.m.check_bytes(r.u).is_ok());
    // Bộ đếm là **toàn cục** (mọi test trong cùng tiến trình): khẳng định **delta** của chính ca
    // này (KT-74) — `Copy 2 · Revoke 2 · Delete 2` — nên không phụ thuộc thứ tự/song song.
    let s = invoke::stats();
    assert!(
        s.copy >= base_stats.copy + 2
            && s.revoke >= base_stats.revoke + 2
            && s.delete >= base_stats.delete + 2,
        "delta capalloc: copy {} -> {}, revoke {} -> {}, delete {} -> {}",
        base_stats.copy,
        s.copy,
        base_stats.revoke,
        s.revoke,
        base_stats.delete,
        s.delete
    );
}
