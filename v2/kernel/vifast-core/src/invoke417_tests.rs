// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-417** — test host cho bốn invocation mở ở cửa sổ `spec/` `D2-261`:
//! `Tcb.{Bind,Unbind}Notification` (13/14) và `CNode.{Mutate,Rotate}` (3/6), cộng hợp đồng
//! `[boot_caps]` sinh từ `spec/abi.toml`.
//!
//! Vì sao test host: đây là ngữ nghĩa **core** (cap/quyền/arity/bảng slot/MDB) và **không**
//! phụ thuộc hook arch (không VM) ⇒ kiểm được không cần QEMU. Bằng chứng boot (`# inv:`) nằm ở
//! TAP của ktest; tệp này kiểm **hợp đồng** trước.
//!
//! Ranh giới đã kiểm: (a) số method **lấy từ spec** (`vifast_abi::*`, không chép tay);
//! (b) arity **chính xác** (D2-104) — thiếu/thừa đều `InvalidArgument` 5; (c) cap được gọi phải
//! có `RIGHT_WRITE` ⇒ thiếu là `InsufficientRights` 3; (d) `Mutate` chỉ thu hẹp quyền trong
//! `RIGHTS_ALL`, badge theo quy ước `Mint`; (e) `Rotate` hoán vị **hai** slot, giữ bất biến MDB
//! (`check_caps`); (f) `Bind` 1:1 (`Busy`/`IllegalOperation`), `Unbind` idempotent;
//! (g) `Tcb.Exec` (16) **chưa cài** ⇒ `NotSupported` 18; (h) `[boot_caps]` sinh đủ 2 profile.
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke;
use crate::machine::Machine;
use crate::spec::{RIGHTS_ALL, RIGHT_GRANT, RIGHT_READ, RIGHT_WRITE};
use crate::syscall::SyscallEnv;

/// Hook arch tối thiểu — bốn invocation này không dùng hook nào (D2-83 chỉ VM mới cần).
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

/// `MessageInfo`: `length` (bit 28..) = số MR dùng, `label` (bit 0..) = method, `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

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

/// Cảnh một tiến trình: CNode 16 slot.
///
/// | Slot | Cap |
/// |---|---|
/// | 0 | CNode tự (`RIGHTS_ALL`) — cap **được gọi** cho `CNode.*` |
/// | 1 | TCB (gọi được `Tcb.*`) |
/// | 2, 3 | hai `Notification` (bind/rotate) |
/// | 10 | `Untyped` (nguồn `Retype`) |
/// | 11 | bản sao TCB **chỉ đọc** (ca quyền) |
/// | 12 | bản sao CNode **chỉ đọc** (ca quyền) |
struct Rig {
    m: Machine,
    tcb: ObjId,
    c: ObjId,
    tcb_slot: SlotRef,
    ro_tcb_slot: SlotRef,
    ro_cnode_slot: SlotRef,
    n1: ObjId,
    n2: ObjId,
    n1_slot: SlotRef,
    n2_slot: SlotRef,
}

fn rig() -> Rig {
    let mut m = Machine::<crate::store::StaticStore, crate::observer::TestObserver>::new();
    let u = m.add_untyped(0x8300_0000, 1 << 16);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 10, u_cap).expect("cap untyped");
    let tcb_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Tcb, 0, 0)
        .expect("retype tcb");
    m.put_cap(c, 1, tcb_cap).expect("cap tcb");
    m.tcb_configure(SlotRef::new(c, 1), Some(SlotRef::new(c, 0)))
        .expect("cspace = c");
    let n1_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Notification, 0, 0)
        .expect("retype ntfn 1");
    m.put_cap(c, 2, n1_cap).expect("cap ntfn 1");
    let n2_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Notification, 0, 0)
        .expect("retype ntfn 2");
    m.put_cap(c, 3, n2_cap).expect("cap ntfn 2");
    m.mint(SlotRef::new(c, 1), SlotRef::new(c, 11), RIGHT_READ, None)
        .expect("mint tcb ro");
    m.mint(SlotRef::new(c, 0), SlotRef::new(c, 12), RIGHT_READ, None)
        .expect("mint cnode ro");
    let tcb = m.cap_obj(tcb_cap).expect("obj tcb");
    Rig {
        m,
        tcb,
        c,
        tcb_slot: SlotRef::new(c, 1),
        ro_tcb_slot: SlotRef::new(c, 11),
        ro_cnode_slot: SlotRef::new(c, 12),
        n1: m.cap_obj(n1_cap).expect("obj ntfn 1"),
        n2: m.cap_obj(n2_cap).expect("obj ntfn 2"),
        n1_slot: SlotRef::new(c, 2),
        n2_slot: SlotRef::new(c, 3),
    }
}

/// (a) Số method + `[boot_caps]` đều **sinh từ spec**, không chép tay; `Tcb.Exec` (16) chưa cài.
#[test]
fn so_method_va_boot_caps_sinh_tu_spec() {
    assert_eq!(invoke::CNODE_MUTATE, 3);
    assert_eq!(invoke::CNODE_ROTATE, 6);
    assert_eq!(invoke::TCB_BIND_NOTIFICATION, 13);
    assert_eq!(invoke::TCB_UNBIND_NOTIFICATION, 14);
    assert_eq!(vifast_abi::TCB_EXEC, 16);
    assert!(vifast_abi::TCB_METHODS
        .iter()
        .any(|(name, id)| *name == "Exec" && u32::from(*id) == invoke::TCB_EXEC));
    // `[boot_caps]`: 2 profile khác nhau ở **cùng** ô 2 (D2-261 nhóm 8).
    assert_eq!(vifast_abi::BOOT_ABI, 1);
    assert_eq!(vifast_abi::BOOT_CAPS_TEST.child, 2);
    assert_eq!(
        vifast_abi::BOOT_CAPS_TEST.netsrv,
        vifast_abi::BOOT_CAPS_NONE_PROC
    );
    assert_eq!(vifast_abi::BOOT_CAPS_PRODUCT.netsrv, 2);
    assert_eq!(
        vifast_abi::BOOT_CAPS_PRODUCT.child,
        vifast_abi::BOOT_CAPS_NONE_PROC
    );
    assert_eq!(vifast_abi::BOOT_CAPS_TEST.tcb, [10, 11, 12, 13, 14]);
    assert_eq!(vifast_abi::BOOT_CAPS_TEST.sc, [30, 31, 32, 33, 34]);
    assert_eq!(vifast_abi::BOOT_CAPS_TEST.fault, 1);
    assert_eq!(vifast_abi::BOOT_CAPS_TEST.self_cspace, 20);
    assert_eq!(vifast_abi::BOOT_CAPS_NONE_CSPACE, 0xFFFF);
}

/// (d) `Mutate`: đổi **tại chỗ** quyền + badge; `None` giữ nguyên; badge `0` xoá; MDB không đổi.
#[test]
fn mutate_doi_quyen_va_badge() {
    let mut r = rig();
    let call = SlotRef::new(r.c, 0);
    let cap = r.m.cap_at(r.c, 2).expect("cap ntfn");
    assert_eq!(r.m.cap_info(cap).expect("info").rights, RIGHTS_ALL);
    let base = invoke::stats().mutated;
    // Thu hẹp quyền + đặt badge.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            3,
            [2, u64::from(RIGHT_READ | RIGHT_GRANT), 0xBEEF, 0]
        ),
        Ok((0, 0))
    );
    let info = r.m.cap_info(cap).expect("info");
    assert_eq!(info.rights, RIGHT_READ | RIGHT_GRANT);
    assert_eq!(info.badge, Some(0xBEEF));
    assert_eq!(
        info.slot,
        Some(SlotRef::new(r.c, 2)),
        "Mutate không đổi slot"
    );
    // `rights?` vắng (arity 1) ⇒ không đổi quyền/badge (đường `None`).
    let before = r.m.cap_info(cap).expect("info");
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, call, invoke::CNODE_MUTATE, 1, [2, 0, 0, 0]),
        Ok((0, 0))
    );
    let after = r.m.cap_info(cap).expect("info");
    assert_eq!(after.rights, before.rights);
    assert_eq!(after.badge, before.badge);
    // Có `rights?` trùng quyền hiện tại ⇒ chỉ đổi badge.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            3,
            [2, u64::from(RIGHT_READ | RIGHT_GRANT), 0x1234, 0]
        ),
        Ok((0, 0))
    );
    let info = r.m.cap_info(cap).expect("info");
    assert_eq!(info.rights, RIGHT_READ | RIGHT_GRANT);
    assert_eq!(info.badge, Some(0x1234));
    // `badge = 0` ⇒ xoá badge (quy ước `Mint`).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            3,
            [2, u64::from(RIGHT_READ), 0, 0]
        ),
        Ok((0, 0))
    );
    let info = r.m.cap_info(cap).expect("info");
    assert_eq!(info.rights, RIGHT_READ);
    assert_eq!(info.badge, None);
    assert!(r.m.check_caps().is_ok(), "Mutate giữ bất biến MDB");
    assert!(
        invoke::stats().mutated >= base + 3,
        "delta mutated: {} -> {}",
        base,
        invoke::stats().mutated
    );
}

/// (d) `Mutate`: quyền **ngoài** `RIGHTS_ALL`, mở rộng quyền, sai badge/kiểu, arity, thiếu quyền.
#[test]
fn mutate_kiem_quyen_va_arity() {
    let mut r = rig();
    let call = SlotRef::new(r.c, 0);
    let denied_before = invoke::stats().denied;
    // Bit ngoài RIGHTS_ALL (0x80) ⇒ InvalidArgument 5.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            2,
            [2, 0x80, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    // Quyền không phải tập con của quyền hiện có: thu hẹp trước rồi mở lại ⇒ InsufficientRights 3.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            2,
            [2, u64::from(RIGHT_READ), 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            2,
            [2, u64::from(RIGHT_READ | RIGHT_WRITE), 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    // Badge trên TCB (không phải Endpoint/Notification) ⇒ InvalidArgument 5.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            call,
            invoke::CNODE_MUTATE,
            2,
            [1, 0xAA, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    // Arity chính xác 1..3 (D2-104): thiếu/thừa ⇒ InvalidArgument 5.
    for len in [0u8, 4u8] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.tcb,
                call,
                invoke::CNODE_MUTATE,
                len,
                [2, 0, 0, 0]
            ),
            Err(KError::InvalidArgument.id() as u64),
            "arity len={len} phải bị từ chối"
        );
    }
    // Cap CNode **chỉ đọc** ⇒ InsufficientRights 3 (cùng luật quyền D2-99(e)).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ro_cnode_slot,
            invoke::CNODE_MUTATE,
            2,
            [2, u64::from(RIGHT_READ), 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    assert!(r.m.check_caps().is_ok());
    assert!(
        invoke::stats().denied > denied_before,
        "ca âm của Mutate phải **có đếm** `denied` (D2-53 Q11)"
    );
}

/// (e) `Rotate`: hoán vị hai slot, một rỗng ⇒ chuyển, hai rỗng ⇒ `SlotEmpty`, MDB sạch.
#[test]
fn rotate_hoan_vi_hai_slot() {
    let mut r = rig();
    let call = SlotRef::new(r.c, 0);
    let denied_before = invoke::stats().denied;
    // Swap 2 ↔ 3: hai Notification đổi chỗ.
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, call, invoke::CNODE_ROTATE, 2, [2, 3, 0, 0]),
        Ok((0, 0))
    );
    assert_eq!(r.m.cap_obj_at(r.c, 2), Ok(r.n2));
    assert_eq!(r.m.cap_obj_at(r.c, 3), Ok(r.n1));
    assert!(r.m.check_caps().is_ok());
    // Swap với ô rỗng 4: cap ở 2 chuyển sang 4, ô 2 rỗng.
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, call, invoke::CNODE_ROTATE, 2, [2, 4, 0, 0]),
        Ok((0, 0))
    );
    assert_eq!(r.m.cap_obj_at(r.c, 2), Err(KError::SlotEmpty));
    assert_eq!(r.m.cap_obj_at(r.c, 4), Ok(r.n2));
    // Hai ô rỗng ⇒ SlotEmpty 8.
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, call, invoke::CNODE_ROTATE, 2, [5, 6, 0, 0]),
        Err(KError::SlotEmpty.id() as u64)
    );
    // Cùng một ô ⇒ không đổi gì (Ok), không phải lỗi.
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, call, invoke::CNODE_ROTATE, 2, [4, 4, 0, 0]),
        Ok((0, 0))
    );
    // Arity chính xác 2: thiếu/thừa ⇒ InvalidArgument 5.
    for len in [1u8, 3u8] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.tcb,
                call,
                invoke::CNODE_ROTATE,
                len,
                [2, 3, 0, 0]
            ),
            Err(KError::InvalidArgument.id() as u64)
        );
    }
    // Cap chỉ đọc ⇒ InsufficientRights 3.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ro_cnode_slot,
            invoke::CNODE_ROTATE,
            2,
            [4, 3, 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    assert!(r.m.check_caps().is_ok());
    assert!(
        invoke::stats().denied > denied_before,
        "ca âm của Rotate phải **có đếm** `denied`"
    );
}

/// (f) `BindNotification` 1:1 + `UnbindNotification` idempotent; `Busy` phía Notification.
#[test]
fn bind_unbind_notification() {
    let mut r = rig();
    let base_bound = invoke::stats().bound;
    let base_unbound = invoke::stats().unbound;
    // Bind lần đầu.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [2, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_bound_notification(r.tcb), Ok(Some(r.n1)));
    // Bind **cùng** Notification ⇒ idempotent (Ok).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [2, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    // TCB đã gắn Notification **khác** ⇒ IllegalOperation 17.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [3, 0, 0, 0]
        ),
        Err(KError::IllegalOperation.id() as u64)
    );
    // MR0 trỏ cap TCB (sai loại) ⇒ WrongObjectType 2.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [1, 0, 0, 0]
        ),
        Err(KError::WrongObjectType.id() as u64)
    );
    // Arity chính xác 1 cho Bind: 0 / 2 ⇒ InvalidArgument 5.
    for len in [0u8, 2u8] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.tcb,
                r.tcb_slot,
                invoke::TCB_BIND_NOTIFICATION,
                len,
                [2, 0, 0, 0]
            ),
            Err(KError::InvalidArgument.id() as u64)
        );
    }
    // Unbind ⇒ gỡ; gọi lại ⇒ idempotent.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_UNBIND_NOTIFICATION,
            0,
            [0, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_bound_notification(r.tcb), Ok(None));
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_UNBIND_NOTIFICATION,
            0,
            [0, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    // Sau Unbind gắn được Notification khác.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [3, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_bound_notification(r.tcb), Ok(Some(r.n2)));
    // Unbind arity 0: thừa MR ⇒ InvalidArgument 5.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_UNBIND_NOTIFICATION,
            1,
            [0, 0, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    // Cap TCB chỉ đọc ⇒ InsufficientRights 3 (cả Bind lẫn Unbind).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ro_tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [2, 0, 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ro_tcb_slot,
            invoke::TCB_UNBIND_NOTIFICATION,
            0,
            [0, 0, 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    assert!(
        invoke::stats().bound >= base_bound + 3,
        "delta bound: {} -> {}",
        base_bound,
        invoke::stats().bound
    );
    assert!(
        invoke::stats().unbound >= base_unbound + 2,
        "delta unbound: {} -> {}",
        base_unbound,
        invoke::stats().unbound
    );
}

/// (f) `Busy` 14: Notification đã gắn TCB khác thì TCB thứ hai không gắn được.
#[test]
fn bind_busy_khi_notification_da_gan_tcb_khac() {
    let mut r = rig();
    let tcb2_cap =
        r.m.retype(SlotRef::new(r.c, 10), ObjType::Tcb, 0, 0)
            .expect("retype tcb 2");
    r.m.put_cap(r.c, 4, tcb2_cap).expect("cap tcb 2");
    r.m.tcb_configure(SlotRef::new(r.c, 4), Some(SlotRef::new(r.c, 0)))
        .expect("cspace tcb2");
    let tcb2 = r.m.cap_obj(tcb2_cap).expect("obj tcb 2");
    // TCB 2 gắn Notification 1.
    assert_eq!(
        invoke_on(
            &mut r.m,
            tcb2,
            SlotRef::new(r.c, 4),
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [2, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    // TCB 1 gắn **cùng** Notification 1 ⇒ Busy 14 (1:1 phía Notification).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [2, 0, 0, 0]
        ),
        Err(KError::Busy.id() as u64)
    );
    assert_eq!(r.m.tcb_bound_notification(r.tcb), Ok(None));
    assert_eq!(r.m.tcb_bound_notification(tcb2), Ok(Some(r.n1)));
}

/// (g) `Tcb.Exec` (16) có số trong spec nhưng **chưa cài** ⇒ `NotSupported` 18 (D2-53 Q11).
#[test]
fn exec_chua_cai_tra_not_supported() {
    let mut r = rig();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            vifast_abi::TCB_EXEC,
            3,
            [2, 0, 0, 0]
        ),
        Err(KError::NotSupported.id() as u64)
    );
    // Method ngoài bảng `[object].Tcb` (17) cũng 18 — không trả Ok giả.
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, r.tcb_slot, 17, 1, [0, 0, 0, 0]),
        Err(KError::NotSupported.id() as u64)
    );
    let _ = (r.n1_slot, r.n2_slot);
}

/// (i) **F1 — reviewer KT-417**: Notification chết thì mọi TCB đang gắn nó phải được **gỡ liên
/// kết**. Không gỡ ⇒ `TcbData.bound_notification` trỏ `ObjId` đã chết: `check_notifications`
/// đỏ, và nếu `ObjId` được cấp lại thì TCB bị gắn **nhầm** Notification khác / `Busy 14` oan.
#[test]
fn notification_chet_thi_go_lien_ket_tcb() {
    let mut r = rig();
    // Bind n1 qua đúng ABI (không cài đặt tắt).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_BIND_NOTIFICATION,
            1,
            [2, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_bound_notification(r.tcb), Ok(Some(r.n1)));
    assert!(r.m.check_notifications().is_ok());
    // Đường chết thật: `Revoke` Untyped chủ → `revoke_children` → `finalise_notification_body`
    // (gọi thẳng hook finalise ở đây; `revoke` cần cap Untyped đã bị `retype` tiêu).
    r.m.finalise_notification_body(r.n1);
    assert_eq!(
        r.m.tcb_bound_notification(r.tcb),
        Ok(None),
        "F1: TCB còn trỏ Notification đã chết ⇒ bind nhầm sau khi ObjId tái dùng"
    );
    assert!(
        r.m.check_notifications().is_ok(),
        "F1: bất biến check_notifications phải xanh sau khi Notification chết"
    );
}
