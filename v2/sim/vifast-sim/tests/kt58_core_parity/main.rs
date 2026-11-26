// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-58 — parity core `no_std` (`vifast-core`, K3b tầng 1) với **sim** (oracle).
//!
//! Cùng một `Op` chạy trên `vifast_sim::kobj::Machine` và `vifast_core::Machine` qua hai
//! trait [`View`]/[`Mut`]; sau **mỗi** bước so:
//! - kết quả (`Ok`/`Err` + **tên + số** mã lỗi — cùng bộ 19 mã sinh từ spec),
//! - digest toàn trạng thái (object: kiểu/alive/bytes; cap: obj/kiểu/cha/slot/quyền/badge/
//!   alive; từng slot CNode; thống kê byte Untyped),
//! - hai bất biến `check_caps` + `check_all_bytes` của **cả hai** phải `Ok`.
//!
//! Kịch bản (`scripts`): port trực tiếp các ca KT-09 (byte/watermark/Revoke lồng nhau),
//! KT-10 (cây CNode/MDB, reparent, finalise CNode lồng) và KT-11 (quyền/badge/kế thừa);
//! thêm differential 32 seed × 48 op ngẫu nhiên tất định (LCG) để phủ tổ hợp.
//!
//! Đây là test **host** (std) — chạy trong `gate --fast` (`cargo test --workspace`). Core
//! không đổi ngữ nghĩa sim; sim vẫn là oracle.

use vifast_core as kc;
use vifast_sim::kobj as ks;

// **KT-407** — tách theo miền (tệp gốc 2688 dòng).
mod scripts;
use scripts::*;

// ---------------------------------------------------------------------------------------------
// Kiểu chung
// ---------------------------------------------------------------------------------------------

type Res = Result<u64, (&'static str, u16)>;

/// Loại object trong kịch bản (ánh xạ sang enum của sim/core trong impl trait).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Untyped,
    CNode,
    Tcb,
    SchedContext,
    Endpoint,
    Notification,
    Reply,
    SchedControl,
    VSpace,
    IrqControl,
}

impl Ty {
    fn sim(self) -> ks::ObjType {
        match self {
            Ty::Untyped => ks::ObjType::Untyped,
            Ty::CNode => ks::ObjType::CNode,
            Ty::Tcb => ks::ObjType::Tcb,
            Ty::SchedContext => ks::ObjType::SchedContext,
            Ty::Endpoint => ks::ObjType::Endpoint,
            Ty::Notification => ks::ObjType::Notification,
            Ty::Reply => ks::ObjType::Reply,
            Ty::SchedControl => ks::ObjType::SchedControl,
            Ty::VSpace => ks::ObjType::VSpace,
            Ty::IrqControl => ks::ObjType::IrqControl,
        }
    }

    fn core(self) -> kc::ObjType {
        match self {
            Ty::Untyped => kc::ObjType::Untyped,
            Ty::CNode => kc::ObjType::CNode,
            Ty::Tcb => kc::ObjType::Tcb,
            Ty::SchedContext => kc::ObjType::SchedContext,
            Ty::Endpoint => kc::ObjType::Endpoint,
            Ty::Notification => kc::ObjType::Notification,
            Ty::Reply => kc::ObjType::Reply,
            Ty::SchedControl => kc::ObjType::SchedControl,
            Ty::VSpace => kc::ObjType::VSpace,
            Ty::IrqControl => kc::ObjType::IrqControl,
        }
    }
}

/// Một thao tác của kịch bản — dữ liệu thuần, chạy được trên cả hai máy.
#[derive(Debug, Clone, Copy)]
enum Op {
    AddUntyped {
        base: u64,
        size: usize,
    },
    AddCnode {
        slots: usize,
    },
    AddKernel {
        ty: Ty,
    },
    RootCap {
        obj: u32,
    },
    PutCap {
        cnode: u32,
        slot: usize,
        cap: u32,
    },
    Retype {
        cnode: u32,
        slot: usize,
        ty: Ty,
        bits: u32,
        flags: u32,
    },
    Copy {
        sc: u32,
        ss: usize,
        dc: u32,
        ds: usize,
    },
    CopyRights {
        sc: u32,
        ss: usize,
        dc: u32,
        ds: usize,
        rights: Option<u8>,
    },
    Mint {
        sc: u32,
        ss: usize,
        dc: u32,
        ds: usize,
        rights: u8,
        badge: Option<u64>,
    },
    Delete {
        cnode: u32,
        slot: usize,
    },
    Revoke {
        cnode: u32,
        slot: Option<usize>,
    },
    CheckUntyped {
        u: u32,
        used: usize,
        free: usize,
        allocated: usize,
        gap: usize,
    },
    CheckObject {
        obj: u32,
        bytes: Option<(usize, usize)>,
    },
    CheckObjectAlive {
        obj: u32,
        alive: bool,
    },
    CheckCap {
        cap: u32,
        alive: bool,
        rights: u8,
        badge: Option<u64>,
        parent: Option<u32>,
        slot: Option<(u32, usize)>,
    },
    CheckCapAt {
        cnode: u32,
        slot: usize,
        err: Option<&'static str>,
    },
    CheckChildren {
        cap: u32,
        children: &'static [u32],
    },
}

/// Bước = thao tác + kỳ vọng (id trả về / số cap bị revoke / mã lỗi).
#[derive(Debug, Clone, Copy)]
struct Step {
    op: Op,
    expect: Option<Res>,
}

fn s(op: Op) -> Step {
    Step { op, expect: None }
}

fn ok(op: Op, v: u64) -> Step {
    Step {
        op,
        expect: Some(Ok(v)),
    }
}

fn err(op: Op, e: kc::KError) -> Step {
    Step {
        op,
        expect: Some(Err((e.name(), e.id()))),
    }
}

// ---------------------------------------------------------------------------------------------
// Trait: cùng logic test chạy trên hai bản
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CapFp {
    obj: u32,
    ty: &'static str,
    parent: Option<u32>,
    slot: Option<(u32, usize)>,
    rights: u8,
    badge: Option<u64>,
    alive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UntFp {
    size: usize,
    used: usize,
    free: usize,
    allocated: usize,
    gap: usize,
}

trait View {
    fn otype(&self, id: u32) -> Option<&'static str>;
    fn oalive(&self, id: u32) -> bool;
    fn obytes(&self, id: u32) -> Option<(usize, usize)>;
    fn cap_fp(&self, id: u32) -> Option<CapFp>;
    fn cnode_slots(&self, id: u32) -> Option<usize>;
    fn cap_at(&self, cnode: u32, slot: usize) -> Result<u32, &'static str>;
    fn children(&self, cap: u32) -> Vec<u32>;
    fn ustat(&self, id: u32) -> Option<UntFp>;
    fn check_caps(&self) -> Result<(), String>;
    fn check_all_bytes(&self) -> Result<(), String>;
}

trait Mut: View {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32;
    fn add_cnode(&mut self, slots: usize) -> u32;
    fn add_kernel(&mut self, ty: Ty) -> u32;
    fn root_cap(&mut self, obj: u32) -> u32;
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res;
    fn retype(&mut self, cnode: u32, slot: usize, ty: Ty, bits: u32, flags: u32) -> Res;
    fn copy(&mut self, sc: u32, ss: usize, dc: u32, ds: usize) -> Res;
    fn copy_rights(&mut self, sc: u32, ss: usize, dc: u32, ds: usize, rights: Option<u8>) -> Res;
    fn mint(
        &mut self,
        sc: u32,
        ss: usize,
        dc: u32,
        ds: usize,
        rights: u8,
        badge: Option<u64>,
    ) -> Res;
    fn delete(&mut self, cnode: u32, slot: usize) -> Res;
    fn revoke(&mut self, cnode: u32, slot: Option<usize>) -> Res;
}

fn se(e: ks::KError) -> (&'static str, u16) {
    (e.name(), e.id())
}

fn ce(e: kc::KError) -> (&'static str, u16) {
    (e.name(), e.id())
}

impl View for ks::Machine {
    fn otype(&self, id: u32) -> Option<&'static str> {
        self.object_type(id).map(|t| t.name())
    }
    fn oalive(&self, id: u32) -> bool {
        self.object_alive(id)
    }
    fn obytes(&self, id: u32) -> Option<(usize, usize)> {
        self.object_bytes(id)
    }
    fn cap_fp(&self, id: u32) -> Option<CapFp> {
        self.cap_info(id).ok().map(|c| CapFp {
            obj: c.obj,
            ty: c.ty.name(),
            parent: c.parent,
            slot: c.slot.map(|s| (s.cnode, s.slot)),
            rights: c.rights,
            badge: c.badge,
            alive: c.alive,
        })
    }
    fn cnode_slots(&self, id: u32) -> Option<usize> {
        self.cnode_slot_count(id).ok()
    }
    fn cap_at(&self, cnode: u32, slot: usize) -> Result<u32, &'static str> {
        ks::Machine::cap_at(self, cnode, slot).map_err(|e| e.name())
    }
    fn children(&self, cap: u32) -> Vec<u32> {
        self.cap_children(cap).unwrap_or_default()
    }
    fn ustat(&self, id: u32) -> Option<UntFp> {
        match self.object_type(id) {
            Some(ks::ObjType::Untyped) => Some(UntFp {
                size: self.untyped_size(id),
                used: self.untyped_used(id),
                free: self.untyped_free(id),
                allocated: self.untyped_allocated(id),
                gap: self.untyped_gap(id),
            }),
            _ => None,
        }
    }
    fn check_caps(&self) -> Result<(), String> {
        ks::Machine::check_caps(self)
    }
    fn check_all_bytes(&self) -> Result<(), String> {
        ks::Machine::check_all_bytes(self)
    }
}

impl Mut for ks::Machine {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32 {
        ks::Machine::add_untyped(self, base, size)
    }
    fn add_cnode(&mut self, slots: usize) -> u32 {
        ks::Machine::add_cnode(self, slots)
    }
    fn add_kernel(&mut self, ty: Ty) -> u32 {
        ks::Machine::add_kernel_object(self, ty.sim())
    }
    fn root_cap(&mut self, obj: u32) -> u32 {
        ks::Machine::root_cap(self, obj)
    }
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res {
        ks::Machine::put_cap(self, cnode, slot, cap)
            .map(|_| 0)
            .map_err(se)
    }
    fn retype(&mut self, cnode: u32, slot: usize, ty: Ty, bits: u32, flags: u32) -> Res {
        ks::Machine::retype(self, ks::SlotRef::new(cnode, slot), ty.sim(), bits, flags)
            .map(u64::from)
            .map_err(se)
    }
    fn copy(&mut self, sc: u32, ss: usize, dc: u32, ds: usize) -> Res {
        ks::Machine::copy(self, ks::SlotRef::new(sc, ss), ks::SlotRef::new(dc, ds))
            .map(u64::from)
            .map_err(se)
    }
    fn copy_rights(&mut self, sc: u32, ss: usize, dc: u32, ds: usize, rights: Option<u8>) -> Res {
        ks::Machine::copy_rights(
            self,
            ks::SlotRef::new(sc, ss),
            ks::SlotRef::new(dc, ds),
            rights,
        )
        .map(u64::from)
        .map_err(se)
    }
    fn mint(
        &mut self,
        sc: u32,
        ss: usize,
        dc: u32,
        ds: usize,
        rights: u8,
        badge: Option<u64>,
    ) -> Res {
        ks::Machine::mint(
            self,
            ks::SlotRef::new(sc, ss),
            ks::SlotRef::new(dc, ds),
            rights,
            badge,
        )
        .map(u64::from)
        .map_err(se)
    }
    fn delete(&mut self, cnode: u32, slot: usize) -> Res {
        ks::Machine::delete(self, ks::SlotRef::new(cnode, slot))
            .map(|_| 0)
            .map_err(se)
    }
    fn revoke(&mut self, cnode: u32, slot: Option<usize>) -> Res {
        ks::Machine::revoke(self, cnode, slot)
            .map(|n| n as u64)
            .map_err(se)
    }
}

impl View for kc::Machine {
    fn otype(&self, id: u32) -> Option<&'static str> {
        self.object_type(id).map(|t| t.name())
    }
    fn oalive(&self, id: u32) -> bool {
        self.object_alive(id)
    }
    fn obytes(&self, id: u32) -> Option<(usize, usize)> {
        self.object_bytes(id)
    }
    fn cap_fp(&self, id: u32) -> Option<CapFp> {
        self.cap_info(id).ok().map(|c| CapFp {
            obj: c.obj,
            ty: c.ty.name(),
            parent: c.parent,
            slot: c.slot.map(|s| (s.cnode, s.slot)),
            rights: c.rights,
            badge: c.badge,
            alive: c.alive,
        })
    }
    fn cnode_slots(&self, id: u32) -> Option<usize> {
        self.cnode_slot_count(id).ok()
    }
    fn cap_at(&self, cnode: u32, slot: usize) -> Result<u32, &'static str> {
        kc::Machine::cap_at(self, cnode, slot).map_err(|e| e.name())
    }
    fn children(&self, cap: u32) -> Vec<u32> {
        self.cap_children(cap)
            .map(|it| it.collect())
            .unwrap_or_default()
    }
    fn ustat(&self, id: u32) -> Option<UntFp> {
        match self.object_type(id) {
            Some(kc::ObjType::Untyped) => Some(UntFp {
                size: self.untyped_size(id),
                used: self.untyped_used(id),
                free: self.untyped_free(id),
                allocated: self.untyped_allocated(id),
                gap: self.untyped_gap(id),
            }),
            _ => None,
        }
    }
    fn check_caps(&self) -> Result<(), String> {
        kc::Machine::check_caps(self).map_err(|e| format!("{e}"))
    }
    fn check_all_bytes(&self) -> Result<(), String> {
        kc::Machine::check_all_bytes(self).map_err(|e| format!("{e}"))
    }
}

impl Mut for kc::Machine {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32 {
        kc::Machine::add_untyped(self, base, size)
    }
    fn add_cnode(&mut self, slots: usize) -> u32 {
        kc::Machine::add_cnode(self, slots)
    }
    fn add_kernel(&mut self, ty: Ty) -> u32 {
        kc::Machine::add_kernel_object(self, ty.core())
    }
    fn root_cap(&mut self, obj: u32) -> u32 {
        kc::Machine::root_cap(self, obj)
    }
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res {
        kc::Machine::put_cap(self, cnode, slot, cap)
            .map(|_| 0)
            .map_err(ce)
    }
    fn retype(&mut self, cnode: u32, slot: usize, ty: Ty, bits: u32, flags: u32) -> Res {
        kc::Machine::retype(self, kc::SlotRef::new(cnode, slot), ty.core(), bits, flags)
            .map(u64::from)
            .map_err(ce)
    }
    fn copy(&mut self, sc: u32, ss: usize, dc: u32, ds: usize) -> Res {
        kc::Machine::copy(self, kc::SlotRef::new(sc, ss), kc::SlotRef::new(dc, ds))
            .map(u64::from)
            .map_err(ce)
    }
    fn copy_rights(&mut self, sc: u32, ss: usize, dc: u32, ds: usize, rights: Option<u8>) -> Res {
        kc::Machine::copy_rights(
            self,
            kc::SlotRef::new(sc, ss),
            kc::SlotRef::new(dc, ds),
            rights,
        )
        .map(u64::from)
        .map_err(ce)
    }
    fn mint(
        &mut self,
        sc: u32,
        ss: usize,
        dc: u32,
        ds: usize,
        rights: u8,
        badge: Option<u64>,
    ) -> Res {
        kc::Machine::mint(
            self,
            kc::SlotRef::new(sc, ss),
            kc::SlotRef::new(dc, ds),
            rights,
            badge,
        )
        .map(u64::from)
        .map_err(ce)
    }
    fn delete(&mut self, cnode: u32, slot: usize) -> Res {
        kc::Machine::delete(self, kc::SlotRef::new(cnode, slot))
            .map(|_| 0)
            .map_err(ce)
    }
    fn revoke(&mut self, cnode: u32, slot: Option<usize>) -> Res {
        kc::Machine::revoke(self, cnode, slot)
            .map(|n| n as u64)
            .map_err(ce)
    }
}

// ---------------------------------------------------------------------------------------------
// Digest + chạy kịch bản
// ---------------------------------------------------------------------------------------------

type ObjFp = (u32, &'static str, bool, Option<(usize, usize)>);
type CapRow = (u32, CapFp);
type CnRow = (u32, usize, Vec<Option<u32>>);
type UnRow = (u32, UntFp);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dig {
    objs: Vec<ObjFp>,
    caps: Vec<CapRow>,
    cn: Vec<CnRow>,
    un: Vec<UnRow>,
}

fn digest(m: &impl View) -> Dig {
    let mut objs = Vec::new();
    let mut id = 1u32;
    while let Some(ty) = m.otype(id) {
        objs.push((id, ty, m.oalive(id), m.obytes(id)));
        id += 1;
    }
    let mut caps = Vec::new();
    let mut cid = 1u32;
    while let Some(fp) = m.cap_fp(cid) {
        caps.push((cid, fp));
        cid += 1;
    }
    let mut cn = Vec::new();
    for &(oid, ty, alive, _) in &objs {
        if ty == "CNode" && alive {
            let n = m.cnode_slots(oid).expect("CNode sống phải có số slot");
            let mut slots = Vec::new();
            for slot in 0..n {
                slots.push(m.cap_at(oid, slot).ok());
            }
            cn.push((oid, n, slots));
        }
    }
    let mut un = Vec::new();
    for &(oid, ty, alive, _) in &objs {
        if ty == "Untyped" && alive {
            if let Some(st) = m.ustat(oid) {
                un.push((oid, st));
            }
        }
    }
    Dig { objs, caps, cn, un }
}

/// Bất biến byte luôn phải giữ; `check_caps` chỉ có nghĩa khi **mọi cap con đã nằm trong
/// slot** — sim cho phép trạng thái trung gian "cap Retype chưa đặt vào CNode" (parent có,
/// slot chưa có), và test KT-09/10 cũng không gọi `check_caps` trong khoảng đó.
fn invariant<S: View>(name: &str, m: &S, caps_closed: bool) -> Result<(), String> {
    m.check_all_bytes()
        .map_err(|e| format!("{name}: check_all_bytes: {e}"))?;
    if caps_closed {
        m.check_caps()
            .map_err(|e| format!("{name}: check_caps: {e}"))?;
    }
    Ok(())
}

fn apply<M: Mut>(m: &mut M, op: Op) -> Res {
    match op {
        Op::AddUntyped { base, size } => Ok(u64::from(m.add_untyped(base, size))),
        Op::AddCnode { slots } => Ok(u64::from(m.add_cnode(slots))),
        Op::AddKernel { ty } => Ok(u64::from(m.add_kernel(ty))),
        Op::RootCap { obj } => Ok(u64::from(m.root_cap(obj))),
        Op::PutCap { cnode, slot, cap } => m.put_cap(cnode, slot, cap),
        Op::Retype {
            cnode,
            slot,
            ty,
            bits,
            flags,
        } => m.retype(cnode, slot, ty, bits, flags),
        Op::Copy { sc, ss, dc, ds } => m.copy(sc, ss, dc, ds),
        Op::CopyRights {
            sc,
            ss,
            dc,
            ds,
            rights,
        } => m.copy_rights(sc, ss, dc, ds, rights),
        Op::Mint {
            sc,
            ss,
            dc,
            ds,
            rights,
            badge,
        } => m.mint(sc, ss, dc, ds, rights, badge),
        Op::Delete { cnode, slot } => m.delete(cnode, slot),
        Op::Revoke { cnode, slot } => m.revoke(cnode, slot),
        Op::CheckUntyped {
            u,
            used,
            free,
            allocated,
            gap,
        } => {
            let st = m.ustat(u).expect("Untyped phải tồn tại");
            assert_eq!(st.used, used, "Untyped {u}: used");
            assert_eq!(st.free, free, "Untyped {u}: free");
            assert_eq!(st.allocated, allocated, "Untyped {u}: allocated");
            assert_eq!(st.gap, gap, "Untyped {u}: gap");
            Ok(0)
        }
        Op::CheckObject { obj, bytes } => {
            assert_eq!(m.obytes(obj), bytes, "object {obj}: bytes");
            Ok(0)
        }
        Op::CheckObjectAlive { obj, alive } => {
            assert_eq!(m.oalive(obj), alive, "object {obj}: alive");
            Ok(0)
        }
        Op::CheckCap {
            cap,
            alive,
            rights,
            badge,
            parent,
            slot,
        } => {
            let c = m.cap_fp(cap).expect("cap phải tồn tại");
            assert_eq!(c.alive, alive, "cap {cap}: alive");
            assert_eq!(c.rights, rights, "cap {cap}: rights");
            assert_eq!(c.badge, badge, "cap {cap}: badge");
            assert_eq!(c.parent, parent, "cap {cap}: parent");
            assert_eq!(c.slot, slot, "cap {cap}: slot");
            Ok(0)
        }
        Op::CheckCapAt { cnode, slot, err } => match (err, m.cap_at(cnode, slot)) {
            (None, Ok(id)) => {
                assert!(id > 0, "cap_at ({cnode},{slot}) phải trả cap id");
                Ok(0)
            }
            (Some(want), Err(got)) => {
                assert_eq!(got, want, "cap_at ({cnode},{slot}): mã lỗi");
                Ok(0)
            }
            (None, Err(got)) => panic!("cap_at ({cnode},{slot}): muốn Ok, gặp {got}"),
            (Some(want), Ok(id)) => panic!("cap_at ({cnode},{slot}): muốn {want}, gặp Ok({id})"),
        },
        Op::CheckChildren { cap, children } => {
            assert_eq!(m.children(cap), children.to_vec(), "cap {cap}: children");
            Ok(0)
        }
    }
}

/// Đếm phủ của kịch bản (bằng chứng "không phải toàn lỗi").
#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    steps: usize,
    ok: usize,
    err: usize,
    retype_ok: usize,
    copy_ok: usize,
    mint_ok: usize,
    delete_ok: usize,
    revoke_ok: usize,
    revoke_deleted: u64,
    caps_closed: usize,
}

fn run_script(name: &str, steps: &[Step]) -> Stats {
    let mut sm = ks::Machine::new();
    let mut cm = kc::Machine::new();
    let mut stats = Stats::default();
    for (i, st) in steps.iter().enumerate() {
        let sr = apply(&mut sm, st.op);
        let cr = apply(&mut cm, st.op);
        assert_eq!(
            sr, cr,
            "[{name}#{i}] {:?}: sim {sr:?} != core {cr:?}",
            st.op
        );
        if let Some(want) = st.expect {
            assert_eq!(sr, want, "[{name}#{i}] {:?}: kỳ vọng", st.op);
        }
        let sd = digest(&sm);
        let cd = digest(&cm);
        assert_eq!(sd, cd, "[{name}#{i}] {:?}: digest sim != core", st.op);
        let caps_closed = sd
            .caps
            .iter()
            .all(|(_, c)| c.parent.is_none() || c.slot.is_some());
        invariant("sim", &sm, caps_closed).unwrap();
        invariant("core", &cm, caps_closed).unwrap();
        stats.steps += 1;
        match sr {
            Ok(v) => {
                stats.ok += 1;
                match st.op {
                    Op::Retype { .. } => stats.retype_ok += 1,
                    Op::Copy { .. } | Op::CopyRights { .. } => stats.copy_ok += 1,
                    Op::Mint { .. } => stats.mint_ok += 1,
                    Op::Delete { .. } => stats.delete_ok += 1,
                    Op::Revoke { .. } => {
                        stats.revoke_ok += 1;
                        stats.revoke_deleted += v;
                    }
                    _ => {}
                }
            }
            Err(_) => stats.err += 1,
        }
        if caps_closed {
            stats.caps_closed += 1;
        }
    }
    stats
}

#[test]
fn parity_kich_ban_kt09_kt10_kt11() {
    for (name, steps) in [
        ("bytes", script_bytes()),
        ("exhaust", script_exhaust()),
        ("retype_errors", script_retype_errors()),
        ("nested_align", script_nested_align()),
        ("revoke_nested", script_revoke_nested()),
        ("double_free", script_double_free()),
        ("mdb_tree", script_mdb_tree()),
        ("finalise_cnode", script_finalise_cnode()),
        ("finalise_cnode_nested", script_finalise_cnode_nested()),
        ("rights", script_rights()),
        ("retype_revoke_loop", script_retype_revoke_loop()),
        ("dead_cnode_errors", script_dead_cnode_errors()),
        ("delete_root_cap", script_delete_root_cap()),
        ("badge_and_align", script_badge_and_align()),
    ] {
        run_script(name, &steps);
    }
}

// ---------------------------------------------------------------------------------------------
// Differential ngẫu nhiên tất định
// ---------------------------------------------------------------------------------------------

fn pick_slot(rng: &mut vifast_sim::Lcg) -> (u32, usize) {
    // CNode 1 có 8 slot, CNode 2/3 có 4 slot — cố ý chọn slot ngoài dải để phủ RangeError.
    let cnode = [1u32, 2, 3][rng.below(3) as usize];
    (cnode, rng.below(8) as usize)
}

/// Slot nguồn: ưu tiên vùng 0..4 (nơi rig đã đặt cap) để bài học Copy/Retype không rỗng.
fn pick_src(rng: &mut vifast_sim::Lcg) -> (u32, usize) {
    let cnode = [1u32, 2, 3][rng.below(3) as usize];
    (cnode, rng.below(4) as usize)
}

fn pick_ty(rng: &mut vifast_sim::Lcg) -> Ty {
    [
        Ty::Tcb,
        Ty::Endpoint,
        Ty::Notification,
        Ty::Reply,
        Ty::SchedContext,
        Ty::SchedControl,
        Ty::CNode,
    ][rng.below(7) as usize]
}

fn random_op(rng: &mut vifast_sim::Lcg) -> Op {
    match rng.below(10) {
        0..=2 => {
            let (cnode, slot) = pick_src(rng);
            let ty = pick_ty(rng);
            let bits = if ty == Ty::CNode {
                4 + rng.below(2) as u32
            } else if rng.below(8) == 0 {
                [3u32, 12][rng.below(2) as usize]
            } else {
                0
            };
            let flags = if rng.below(16) == 0 { 1 } else { 0 };
            Op::Retype {
                cnode,
                slot,
                ty,
                bits,
                flags,
            }
        }
        3 | 4 => {
            let (sc, ss) = pick_src(rng);
            let (dc, ds) = pick_slot(rng);
            Op::Copy { sc, ss, dc, ds }
        }
        5 => {
            let (sc, ss) = pick_src(rng);
            let (dc, ds) = pick_slot(rng);
            let rights = [ALL, READ, WRITE, READ | WRITE][rng.below(4) as usize];
            let badge = [None, Some(0), Some(9), Some(0xdead)][rng.below(4) as usize];
            Op::Mint {
                sc,
                ss,
                dc,
                ds,
                rights,
                badge,
            }
        }
        6 => {
            let (sc, ss) = pick_src(rng);
            let (dc, ds) = pick_slot(rng);
            let rights = [None, Some(READ), Some(WRITE), Some(ALL)][rng.below(4) as usize];
            Op::CopyRights {
                sc,
                ss,
                dc,
                ds,
                rights,
            }
        }
        7 => {
            let (cnode, slot) = pick_src(rng);
            Op::Delete { cnode, slot }
        }
        _ => {
            let cnode = [1u32, 2, 3][rng.below(3) as usize];
            let slot = if rng.below(2) == 0 {
                None
            } else {
                Some(rng.below(4) as usize)
            };
            Op::Revoke { cnode, slot }
        }
    }
}

fn differential_rig() -> Vec<Step> {
    vec![
        ok(Op::AddCnode { slots: 8 }, 1),
        ok(Op::AddCnode { slots: 4 }, 2),
        ok(Op::AddCnode { slots: 4 }, 3),
        ok(Op::AddKernel { ty: Ty::Endpoint }, 4),
        ok(
            Op::AddKernel {
                ty: Ty::Notification,
            },
            5,
        ),
        ok(
            Op::AddUntyped {
                base: 0x1000,
                size: 32 * 1024,
            },
            6,
        ),
        ok(
            Op::AddUntyped {
                base: 0x9000,
                size: 16 * 1024,
            },
            7,
        ),
        ok(Op::RootCap { obj: 4 }, 1),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 0,
                cap: 1,
            },
            0,
        ),
        ok(Op::RootCap { obj: 5 }, 2),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 1,
                cap: 2,
            },
            0,
        ),
        ok(Op::RootCap { obj: 6 }, 3),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 2,
                cap: 3,
            },
            0,
        ),
        ok(Op::RootCap { obj: 7 }, 4),
        ok(
            Op::PutCap {
                cnode: 1,
                slot: 3,
                cap: 4,
            },
            0,
        ),
    ]
}

fn differential_seed(seed: u64) -> Stats {
    let mut steps = differential_rig();
    let mut rng = vifast_sim::Lcg::new(seed);
    for _ in 0..48 {
        steps.push(s(random_op(&mut rng)));
    }
    run_script(&format!("diff-{seed}"), &steps)
}

#[test]
fn parity_differential_32_seed_x48_op() {
    let mut total = Stats::default();
    for seed in 0..32u64 {
        let st = differential_seed(seed);
        total.steps += st.steps;
        total.ok += st.ok;
        total.err += st.err;
        total.retype_ok += st.retype_ok;
        total.copy_ok += st.copy_ok;
        total.mint_ok += st.mint_ok;
        total.delete_ok += st.delete_ok;
        total.revoke_ok += st.revoke_ok;
        total.revoke_deleted += st.revoke_deleted;
        total.caps_closed += st.caps_closed;
    }
    eprintln!("# parity differential: {total:?}");
    // Ngưỡng phủ tối thiểu (không nới để "cho xanh": chỉ chứng minh kịch bản không rỗng).
    assert!(total.ok >= 300, "quá ít thao tác thành công: {total:?}");
    assert!(total.err >= 300, "quá ít nhánh lỗi: {total:?}");
    assert!(total.retype_ok >= 40, "quá ít Retype thành công: {total:?}");
    assert!(
        total.copy_ok >= 40,
        "quá ít Copy/Mint thành công: {total:?}"
    );
    assert!(
        total.revoke_deleted >= 20,
        "quá ít cap bị Revoke: {total:?}"
    );
    assert!(
        total.caps_closed >= 100,
        "quá ít trạng thái check_caps: {total:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Hằng số ABI + dấu chân tĩnh
// ---------------------------------------------------------------------------------------------

/// D2-83 — biên giới core/VM: core **không** Retype Frame/PageTable/VSpace (object layer
/// `vifast_core::vm` sở hữu); pin để không ai "vá" thành port VM vào core.
#[test]
fn core_d2_83_vm_reserved_not_supported() {
    use kc::{KError, Machine, ObjType, SlotRef};
    let mut m = Machine::new();
    let c = m.add_cnode(8);
    let u = m.add_untyped(0x1000, 8192);
    let cap = m.root_cap(u);
    m.put_cap(c, 0, cap).expect("cap Untyped");
    for ty in [ObjType::Frame, ObjType::PageTable, ObjType::VSpace] {
        assert_eq!(
            m.retype(SlotRef::new(c, 0), ty, 0, 0),
            Err(KError::NotSupported),
            "D2-83: {ty:?} phải do obj layer sở hữu, core trả NotSupported"
        );
    }
    assert_eq!(
        m.retype(SlotRef::new(c, 0), ObjType::IrqControl, 0, 0),
        Err(KError::NotSupported)
    );
    assert_eq!(m.untyped_used(u), 0, "Retype reserved không đốt byte");
    assert_eq!(m.cap_count(), 1, "Retype reserved không tạo cap");
}

#[test]
fn core_hang_so_spec_va_dau_chan_tinh() {
    // D2-11: hằng đọc từ crate sinh (`vifast-abi`), không hardcode.
    assert_eq!(kc::spec::SYSCALL_COUNT, 16);
    assert_eq!(kc::spec::ERROR_COUNT, 19);
    assert_eq!(kc::SLOT_BYTES, 48);
    assert_eq!(kc::CNODE_BASE_BYTES, 64);
    assert_eq!(kc::PAGE_BYTES, 4096);
    assert_eq!(kc::UNTYPED_MIN_BYTES, 4096);
    assert_eq!(kc::RIGHTS_ALL.count_ones(), 4, "đúng 4 quyền");
    assert_eq!(kc::KError::SlotOccupied.id(), 7);
    assert_eq!(kc::KError::RangeError.id(), 19);
    assert_eq!(kc::ObjType::from_name("CNode"), Some(kc::ObjType::CNode));
    assert_eq!(kc::ObjType::cnode_bytes(16), 64 + 16 * 48);
    let size = core::mem::size_of::<kc::Machine>();
    eprintln!(
        "# core: size_of::<Machine>() = {size} B, MAX_OBJECTS={}, MAX_CAPS={}, MAX_SLOT_WORDS={}",
        kc::MAX_OBJECTS,
        kc::MAX_CAPS,
        kc::MAX_SLOT_WORDS
    );
    // **D2-84**: pin core = **≤ 196.608 B (192 KiB)** = trần ảnh 262.144 B trừ 64 KiB
    // headroom cho static kernel + cấu trúc tương lai. KT-58 pin `≤ 96 KiB` là dấu chân
    // **tầng 1**; KT-59 thêm 64 body TCB (512 B IPC buffer) + 32 SchedContext + 8 hart + 8 hộp
    // thư nên dấu chân tăng đúng phạm vi. Vượt 192 KiB ⇒ **tối ưu** (giảm `MAX_MAIL_LOG`,
    // chuyển body TCB sang arena), **không** nâng pin; KT-61 đo lại ảnh link thật.
    assert!(
        size <= 196_608,
        "core tĩnh phải ≤ 192 KiB theo D2-84 (đo {size} B)"
    );
}
