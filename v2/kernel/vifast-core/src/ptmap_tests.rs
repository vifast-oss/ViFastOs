// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-417d** — test host cho `PageTable.{Map,Unmap}` (object 10, method 0/1) ở tầng **core**:
//! phân giải cap (`page_table` được gọi, `vspace` ở `MR0`, `frame` ở `MR2`), quyền, arity/cờ và
//! hợp đồng `InvokeOp` chuyển cho hook arch (D2-83: core không ghi PTE).
//!
//! Object layer (`vifast_core::vm`) là nơi **thi hành** thật; ngữ nghĩa PTE/cài bảng được kiểm ở
//! `v2/kernel/vifast-plat-common/tests/obj.rs` (`page_table_map_unmap_*`). Tệp này kiểm **hợp
//! đồng ABI** trước khi tới arch: sai loại/quyền/cờ phải trả đúng mã (2/3/5), thành công phải
//! chuyển **đủ ba khoá** (bảng, VSpace, Frame) và trả `value` từ hook.
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke::{self, InvokeOp, InvokeOutcome};
use crate::machine::Machine;
use crate::spec::RIGHT_READ;
use crate::syscall::SyscallEnv;

/// Hook arch giả: ghi lại `InvokeOp` cuối + trả `Value(7)` (hoặc lỗi cài sẵn).
#[derive(Default)]
struct RecEnv {
    calls: u32,
    last: Option<InvokeOp>,
    fail: Option<u64>,
    value: u64,
}

impl SyscallEnv for RecEnv {
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
    fn invoke(&mut self, op: InvokeOp) -> Result<InvokeOutcome, u64> {
        self.calls += 1;
        self.last = Some(op);
        match self.fail {
            Some(err) => Err(err),
            None => Ok(InvokeOutcome::Value(self.value)),
        }
    }
}

/// `MessageInfo`: `length` ở bit 28.., `label` ở bit 0..; `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

/// Cảnh: CNode 16 slot. Slot 0 CNode tự, 1 TCB, 10 Untyped, 4 `PageTable`, 5 `VSpace`,
/// 6 `Frame`, 7 bản sao `PageTable` **chỉ đọc**, 8 `Frame` chỉ đọc.
struct Rig {
    m: Machine,
    tcb: ObjId,
    c: ObjId,
    pt: ObjId,
    vs: ObjId,
    fr: ObjId,
    pt_key: u64,
    vs_key: u64,
    fr_key: u64,
}

const PT_KEY: u64 = 0x0001_0000;
const VS_KEY: u64 = 0x0002_0000;
const FR_KEY: u64 = 0x0003_0000;
const VA: u64 = 0x4000_0000;

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
    let tcb = m.cap_obj(tcb_cap).expect("obj tcb");
    // Ba object VM **đại diện** (D2-83) — object layer không có mặt ở test host; khoá giả để
    // kiểm hợp đồng chuyển khoá sang hook arch.
    let pt = m
        .add_foreign_object(ObjType::PageTable, PT_KEY)
        .expect("pt");
    let pt_cap = m.root_cap(pt);
    m.put_cap(c, 4, pt_cap).expect("cap pt");
    let vs = m.add_foreign_object(ObjType::VSpace, VS_KEY).expect("vs");
    let vs_cap = m.root_cap(vs);
    m.put_cap(c, 5, vs_cap).expect("cap vs");
    let fr = m.add_foreign_object(ObjType::Frame, FR_KEY).expect("fr");
    let fr_cap = m.root_cap(fr);
    m.put_cap(c, 6, fr_cap).expect("cap fr");
    // Ca quyền: bảng + Frame bản sao **chỉ đọc**.
    m.mint(SlotRef::new(c, 4), SlotRef::new(c, 7), RIGHT_READ, None)
        .expect("mint pt ro");
    m.mint(SlotRef::new(c, 6), SlotRef::new(c, 8), RIGHT_READ, None)
        .expect("mint fr ro");
    Rig {
        m,
        tcb,
        c,
        pt,
        vs,
        fr,
        pt_key: PT_KEY,
        vs_key: VS_KEY,
        fr_key: FR_KEY,
    }
}

fn call(
    r: &mut Rig,
    env: &mut RecEnv,
    slot: usize,
    label: u32,
    len: u8,
    mrs: [u64; 4],
) -> Result<(u64, u64), u64> {
    invoke::dispatch(
        &mut r.m,
        r.tcb,
        SlotRef::new(r.c, slot),
        info(label, len),
        &mrs,
        env,
    )
}

/// `PageTable.Map`: `MR0` = cap VSpace, `MR1` = `va`, `MR2` = cap Frame, `MR3` = cờ; thành công
/// chuyển **đủ ba khoá** cho hook arch và trả `value` của hook ở `a1`.
#[test]
fn pt_map_chuyen_du_ba_khoa_va_tra_value() {
    let mut r = rig();
    let mut env = RecEnv {
        value: 7,
        ..RecEnv::default()
    };
    let flags = invoke::MAP_READ | invoke::MAP_WRITE | invoke::MAP_USER;
    let before = invoke::stats().map;
    assert_eq!(
        call(
            &mut r,
            &mut env,
            4,
            invoke::METHOD_MAP,
            4,
            [5, VA, 6, flags]
        ),
        Ok((0, 7))
    );
    let op = env.last.expect("hook phải được gọi");
    assert_eq!(op.ty, ObjType::PageTable);
    assert_eq!(op.method, invoke::METHOD_MAP);
    assert_eq!(op.obj, r.pt);
    assert_eq!(op.key, r.pt_key);
    assert_eq!(op.aux_obj, r.vs);
    assert_eq!(op.aux_key, r.vs_key);
    assert_eq!(op.aux2_obj, r.fr, "cap Frame ở MR2 phải thành aux2");
    assert_eq!(op.aux2_key, r.fr_key);
    assert_eq!(op.args[1], VA);
    assert_eq!(op.args[3], flags);
    assert_eq!(invoke::stats().map, before + 1, "đếm map VM");
}

/// `PageTable.Unmap`: `MR0` = cap VSpace, `MR1` = `va`; không có cap Frame thứ ba.
#[test]
fn pt_unmap_chuyen_hai_khoa_va_khong_aux2() {
    let mut r = rig();
    let mut env = RecEnv::default();
    let before = invoke::stats().unmap;
    assert_eq!(
        call(&mut r, &mut env, 4, invoke::METHOD_UNMAP, 2, [5, VA, 0, 0]),
        Ok((0, 0))
    );
    let op = env.last.expect("hook phải được gọi");
    assert_eq!(op.ty, ObjType::PageTable);
    assert_eq!(op.method, invoke::METHOD_UNMAP);
    assert_eq!(op.key, r.pt_key);
    assert_eq!(op.aux_key, r.vs_key);
    assert_eq!(op.aux2_obj, 0);
    assert_eq!(op.aux2_key, 0);
    assert_eq!(invoke::stats().unmap, before + 1);
}

/// Ca quyền: cap `PageTable` **chỉ đọc** (slot 7) ⇒ `InsufficientRights` 3, hook **không** gọi;
/// cap `Frame` chỉ đọc (slot 8) ở `MR2` cũng ⇒ 3 (map sửa PTE nên cần `WRITE`).
#[test]
fn pt_map_unmap_thieu_quyen_ghi() {
    let mut r = rig();
    let mut env = RecEnv::default();
    let flags = invoke::MAP_READ;
    assert_eq!(
        call(
            &mut r,
            &mut env,
            7,
            invoke::METHOD_MAP,
            4,
            [5, VA, 6, flags]
        ),
        Err(u64::from(KError::InsufficientRights.id()))
    );
    assert_eq!(
        call(
            &mut r,
            &mut env,
            4,
            invoke::METHOD_MAP,
            4,
            [5, VA, 8, flags]
        ),
        Err(u64::from(KError::InsufficientRights.id()))
    );
    assert_eq!(
        call(&mut r, &mut env, 7, invoke::METHOD_UNMAP, 2, [5, VA, 0, 0]),
        Err(u64::from(KError::InsufficientRights.id()))
    );
    assert_eq!(env.calls, 0, "không được gọi hook khi core đã từ chối");
}

/// Ca loại cap: `MR0` phải là `VSpace`, `MR2` phải là `Frame` ⇒ `WrongObjectType` 2.
#[test]
fn pt_map_sai_loai_cap_phu() {
    let mut r = rig();
    let mut env = RecEnv::default();
    let flags = invoke::MAP_READ;
    // MR0 = Frame thay VSpace.
    assert_eq!(
        call(
            &mut r,
            &mut env,
            4,
            invoke::METHOD_MAP,
            4,
            [6, VA, 6, flags]
        ),
        Err(u64::from(KError::WrongObjectType.id()))
    );
    // MR2 = VSpace thay Frame.
    assert_eq!(
        call(
            &mut r,
            &mut env,
            4,
            invoke::METHOD_MAP,
            4,
            [5, VA, 5, flags]
        ),
        Err(u64::from(KError::WrongObjectType.id()))
    );
    assert_eq!(
        call(&mut r, &mut env, 4, invoke::METHOD_UNMAP, 2, [6, VA, 0, 0]),
        Err(u64::from(KError::WrongObjectType.id()))
    );
    assert_eq!(env.calls, 0);
}

/// Cờ sai **theo luật của core** (bit lạ / 0 / `W` mà không `R`) ⇒ `InvalidArgument` 5.
///
/// `W|X` cùng lúc **không** phải lỗi của core: D2-83 để luật `MapFlags` (W^X) cho object layer,
/// nên core chuyển **nguyên bit** xuống hook — ca W^X thật được kiểm ở
/// `vifast-plat-common/tests/obj.rs` (`page_table_map_unmap_ca_am`).
#[test]
fn pt_map_co_sai() {
    let mut r = rig();
    let mut env = RecEnv::default();
    for bad in [0u64, invoke::MAP_READ | (1 << 5), invoke::MAP_WRITE] {
        assert_eq!(
            call(&mut r, &mut env, 4, invoke::METHOD_MAP, 4, [5, VA, 6, bad]),
            Err(u64::from(KError::InvalidArgument.id()))
        );
    }
    assert_eq!(env.calls, 0);
    // Cờ hợp lệ **về cú pháp** (kể cả W|X) đi nguyên vẹn xuống object layer.
    let wx = invoke::MAP_READ | invoke::MAP_WRITE | invoke::MAP_EXEC;
    assert_eq!(
        call(&mut r, &mut env, 4, invoke::METHOD_MAP, 4, [5, VA, 6, wx]),
        Ok((0, 0))
    );
    assert_eq!(env.last.expect("hook").args[3], wx);
}

/// Lỗi từ object layer (hook) đi **nguyên mã**: `AlreadyMapped` 10 / `NotMapped` 11 /
/// `MissingPageTable` 12 — không nuốt, không đổi mã.
#[test]
fn pt_map_loi_tu_object_layer_di_nguyen_ma() {
    let mut r = rig();
    for code in [10u64, 11, 12] {
        let mut env = RecEnv {
            fail: Some(code),
            ..RecEnv::default()
        };
        assert_eq!(
            call(
                &mut r,
                &mut env,
                4,
                invoke::METHOD_MAP,
                4,
                [5, VA, 6, invoke::MAP_READ]
            ),
            Err(code)
        );
        assert_eq!(env.calls, 1);
    }
}

/// Method khác của `PageTable` vẫn ngoài danh sách trắng ⇒ `NotSupported` 18, hook không gọi.
#[test]
fn page_table_method_khac_ngoai_danh_sach_trang() {
    let mut r = rig();
    let mut env = RecEnv::default();
    for label in [2u32, 3] {
        assert_eq!(
            call(&mut r, &mut env, 4, label, 2, [5, VA, 0, 0]),
            Err(u64::from(KError::NotSupported.id()))
        );
    }
    assert_eq!(env.calls, 0);
    // Bảng ánh xạ object **đại diện** đúng khoá của object layer.
    assert_eq!(
        r.m.foreign_key(r.pt).expect("khoá pt"),
        PT_KEY,
        "khoá PageTable đại diện"
    );
    assert_eq!(r.m.foreign_key(r.vs).expect("khoá vs"), VS_KEY);
    assert_eq!(r.m.foreign_key(r.fr).expect("khoá fr"), FR_KEY);
}

/// Danh sách trắng: hai cặp mới **đã cài**, nhưng chỉ đúng (ty, method) — không mở `VSpace.Map`.
#[test]
fn danh_sach_trang_co_hai_cap_pagetable() {
    assert!(invoke::is_whitelisted(
        ObjType::PageTable,
        invoke::METHOD_MAP
    ));
    assert!(invoke::is_whitelisted(
        ObjType::PageTable,
        invoke::METHOD_UNMAP
    ));
    assert!(!invoke::is_whitelisted(ObjType::PageTable, 2));
    assert!(!invoke::is_whitelisted(ObjType::VSpace, invoke::METHOD_MAP));
    assert!(!invoke::is_whitelisted(ObjType::VSpace, 1));
    // Số method lấy từ `spec/abi.toml` (không chép tay — D2-11).
    assert_eq!(invoke::METHOD_MAP, 0);
    assert_eq!(invoke::METHOD_UNMAP, 1);
    assert_eq!(invoke::type_id(ObjType::PageTable), Some(10));
}
