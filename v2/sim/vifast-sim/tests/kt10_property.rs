// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-10 — property test cây CNode/MDB (mô hình tối giản **trong test**).
//!
//! Generator tự viết (không crate ngoài): LCG tất định + oracle cây dẫn xuất độc lập với
//! `Machine`. Mỗi seed sinh một chuỗi thao tác `Copy/Mint/Delete/Revoke` (+ thao tác lỗi), sau
//! **mỗi** thao tác kiểm các bất biến của thiết kế §4.3/§9.6:
//! - không cap mồ côi: cap sống có cha thì cha phải sống; con của cap bị xoá thuộc về ông;
//! - không cap nào trỏ object chết; cap sống nằm trong slot thì CNode chứa nó phải sống và
//!   slot phải trỏ ngược lại đúng cap (tính đối xứng của MDB);
//! - mọi con có cha đúng; không chu trình; `cap_children` khớp cây;
//! - Revoke chỉ xoá con/cháu của cap đích (hoặc của từng cap trong CNode khi `slot = None`),
//!   không đụng nhánh khác;
//! - tổng cap sống khớp oracle tại mọi bước.

use std::collections::{BTreeMap, BTreeSet};

use vifast_sim::kobj::{
    CapId, KError, Machine, ObjId, ObjType, SlotRef, RIGHT_GRANT, RIGHT_GRANT_REPLY, RIGHT_READ,
    RIGHT_WRITE,
};
use vifast_sim::Lcg;

type CapFp = (
    CapId,
    ObjId,
    ObjType,
    Option<CapId>,
    Option<(ObjId, usize)>,
    u8,
    Option<u64>,
    bool,
);

fn fingerprint(m: &Machine) -> Vec<CapFp> {
    m.cap_table()
        .into_iter()
        .map(|c| {
            (
                c.cap,
                c.obj,
                c.ty,
                c.parent,
                c.slot.map(|s| (s.cnode, s.slot)),
                c.rights,
                c.badge,
                c.alive,
            )
        })
        .collect()
}

fn plus(set: &BTreeSet<CapId>, cap: CapId) -> BTreeSet<CapId> {
    let mut out = set.clone();
    out.insert(cap);
    out
}

fn pick<T: Copy>(rng: &mut Lcg, xs: &[T]) -> Option<T> {
    if xs.is_empty() {
        None
    } else {
        Some(xs[rng.below(xs.len() as u64) as usize])
    }
}

/// Oracle cây cap: chỉ theo dõi quan hệ dẫn xuất (cha/con/slot/object/alive), không dùng
/// trạng thái nội bộ của `Machine` để quyết định kết quả.
#[derive(Debug)]
struct Oracle {
    next: CapId,
    parent: BTreeMap<CapId, Option<CapId>>,
    obj: BTreeMap<CapId, ObjId>,
    slot: BTreeMap<CapId, SlotRef>,
    alive: BTreeSet<CapId>,
}

impl Oracle {
    fn new() -> Self {
        Self {
            next: 1,
            parent: BTreeMap::new(),
            obj: BTreeMap::new(),
            slot: BTreeMap::new(),
            alive: BTreeSet::new(),
        }
    }

    fn record_root(&mut self, cap: CapId, obj: ObjId) {
        assert_eq!(cap, self.next, "CapId phải tuần tự, không tái dùng");
        self.next += 1;
        self.parent.insert(cap, None);
        self.obj.insert(cap, obj);
        self.alive.insert(cap);
    }

    fn record_child(&mut self, cap: CapId, parent: CapId, obj: ObjId) {
        assert_eq!(cap, self.next, "CapId phải tuần tự, không tái dùng");
        assert!(self.alive.contains(&parent), "cha phải còn sống");
        self.next += 1;
        self.parent.insert(cap, Some(parent));
        self.obj.insert(cap, obj);
        self.alive.insert(cap);
    }

    fn place(&mut self, cap: CapId, s: SlotRef) {
        assert!(self.alive.contains(&cap));
        self.slot.insert(cap, s);
    }

    fn children(&self, cap: CapId) -> Vec<CapId> {
        self.alive
            .iter()
            .copied()
            .filter(|c| self.parent.get(c) == Some(&Some(cap)))
            .collect()
    }

    /// Mọi con/cháu của `root` (không gồm chính `root`).
    fn descendants(&self, root: CapId) -> BTreeSet<CapId> {
        let mut out = BTreeSet::new();
        for &cap in &self.alive {
            let mut cur = *self.parent.get(&cap).expect("cap phải có cha");
            let mut guard = 0usize;
            while let Some(p) = cur {
                if p == root {
                    out.insert(cap);
                    break;
                }
                cur = *self.parent.get(&p).expect("cha phải có trong oracle");
                guard += 1;
                assert!(guard <= self.next as usize, "chu trình cha trên cap {cap}");
            }
        }
        out
    }

    /// Delete: con của cap bị xoá thuộc về ông (MDB seL4 `cteDelete`), cap bị xoá chết.
    fn delete(&mut self, cap: CapId) -> Option<CapId> {
        let parent = *self.parent.get(&cap).expect("cha của cap bị xoá");
        for child in self.children(cap) {
            self.parent.insert(child, parent);
        }
        self.alive.remove(&cap);
        self.slot.remove(&cap);
        parent
    }

    fn remove(&mut self, doomed: &BTreeSet<CapId>) {
        for cap in doomed {
            assert!(
                self.alive.remove(cap),
                "cap {cap} phải còn sống trước revoke"
            );
            self.slot.remove(cap);
        }
    }

    fn caps_in(&self, cnode: ObjId) -> Vec<CapId> {
        self.slot
            .iter()
            .filter(|(_, s)| s.cnode == cnode)
            .map(|(c, _)| *c)
            .collect()
    }

    fn check(&self, m: &Machine, ctx: &str) {
        let table = m.cap_table();
        assert_eq!(
            table.len() as CapId + 1,
            self.next,
            "{ctx}: số cap trong Machine lệch oracle"
        );
        assert_eq!(
            m.live_cap_count(),
            self.alive.len(),
            "{ctx}: tổng cap sống lệch oracle"
        );
        let mut seen = BTreeSet::new();
        for info in &table {
            if !self.alive.contains(&info.cap) {
                assert!(!info.alive, "{ctx}: cap {} phải chết", info.cap);
                assert_eq!(info.slot, None, "{ctx}: cap chết {} còn giữ slot", info.cap);
                continue;
            }
            assert!(info.alive, "{ctx}: cap {} phải sống", info.cap);
            assert_eq!(
                self.parent.get(&info.cap),
                Some(&info.parent),
                "{ctx}: cha của cap {} lệch oracle",
                info.cap
            );
            assert_eq!(
                self.slot.get(&info.cap).copied(),
                info.slot,
                "{ctx}: slot của cap {} lệch oracle",
                info.cap
            );
            assert_eq!(
                self.obj.get(&info.cap).copied(),
                Some(info.obj),
                "{ctx}: object của cap {} lệch oracle",
                info.cap
            );
            assert!(m.object_alive(info.obj), "{ctx}: cap trỏ object chết");
            assert_eq!(
                m.object_type(info.obj),
                Some(info.ty),
                "{ctx}: loại cap khác loại object"
            );
            if let Some(p) = info.parent {
                let pi = m
                    .cap_info(p)
                    .unwrap_or_else(|e| panic!("{ctx}: cha {p}: {e:?}"));
                assert!(pi.alive, "{ctx}: cap mồ côi, cha {p} đã chết");
                assert_eq!(pi.obj, info.obj, "{ctx}: cha khác object");
                assert_eq!(
                    info.rights & !pi.rights,
                    0,
                    "{ctx}: quyền cap {} vượt quyền cha",
                    info.cap
                );
            }
            if let Some(s) = info.slot {
                assert!(m.object_alive(s.cnode), "{ctx}: slot nằm trong CNode chết");
                assert_eq!(
                    m.cap_at(s.cnode, s.slot),
                    Ok(info.cap),
                    "{ctx}: slot không trỏ ngược lại cap (MDB đối xứng)"
                );
            } else {
                assert!(
                    info.parent.is_none(),
                    "{ctx}: cap {} là con nhưng không nằm trong slot nào",
                    info.cap
                );
            }
            if !matches!(info.ty, ObjType::Endpoint | ObjType::Notification) {
                assert_eq!(
                    info.badge, None,
                    "{ctx}: badge chỉ đặt cho Endpoint/Notification"
                );
            }
            seen.insert(info.cap);
        }
        assert_eq!(seen, self.alive, "{ctx}: tập cap sống lệch oracle");
        for &cap in &self.alive {
            assert_eq!(
                m.cap_children(cap).expect("cap sống"),
                self.children(cap),
                "{ctx}: danh sách con của cap {cap} lệch cây"
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    ops: usize,
    copies: usize,
    mints: usize,
    deletes: usize,
    revokes: usize,
    revoke_caps: usize,
    invalid: usize,
    adds: usize,
}

struct World {
    m: Machine,
    oracle: Oracle,
    cnodes: Vec<ObjId>,
    objects: Vec<ObjId>,
    stats: Stats,
}

impl World {
    fn new() -> Self {
        let mut m = Machine::new();
        let oracle = Oracle::new();
        let cnodes = vec![m.add_cnode(8), m.add_cnode(8), m.add_cnode(6)];
        let objects = vec![
            m.add_kernel_object(ObjType::Endpoint),
            m.add_kernel_object(ObjType::Endpoint),
            m.add_kernel_object(ObjType::Notification),
            m.add_kernel_object(ObjType::Tcb),
        ];
        let mut w = World {
            m,
            oracle,
            cnodes,
            objects,
            stats: Stats::default(),
        };
        // 4 cap gốc cho object lõi + 1 cap gốc cho chính CNode #1 ⇒ có cap kiểu CNode trong cây.
        let mut roots: Vec<ObjId> = w.objects.clone();
        roots.push(w.cnodes[1]);
        for (i, obj) in roots.into_iter().enumerate() {
            let cap = w.m.root_cap(obj);
            w.oracle.record_root(cap, obj);
            w.m.put_cap(w.cnodes[0], i, cap)
                .expect("setup: đặt cap gốc");
            w.oracle.place(cap, SlotRef::new(w.cnodes[0], i));
        }
        w.check("setup");
        w
    }

    fn check(&self, ctx: &str) {
        self.m
            .check_caps()
            .unwrap_or_else(|e| panic!("{ctx}: bất biến cây cap vỡ: {e}"));
        self.oracle.check(&self.m, ctx);
    }

    fn alive_set(&self) -> BTreeSet<CapId> {
        self.m
            .cap_table()
            .into_iter()
            .filter(|c| c.alive)
            .map(|c| c.cap)
            .collect()
    }

    fn slotted(&self) -> Vec<CapId> {
        self.m
            .cap_table()
            .into_iter()
            .filter(|c| c.alive && c.slot.is_some())
            .map(|c| c.cap)
            .collect()
    }

    fn free_slots(&self) -> Vec<SlotRef> {
        let mut out = Vec::new();
        for &cn in &self.cnodes {
            let n = self.m.cnode_slot_count(cn).expect("CNode sống");
            for slot in 0..n {
                if self.m.cap_at(cn, slot) == Err(KError::SlotEmpty) {
                    out.push(SlotRef::new(cn, slot));
                }
            }
        }
        out
    }

    fn step(&mut self, rng: &mut Lcg) {
        self.stats.ops += 1;
        match rng.below(16) {
            0..=2 => self.op_new_root(rng),
            3..=5 => self.op_copy(rng),
            6..=8 => self.op_mint(rng),
            9..=10 => self.op_delete(rng),
            11..=13 => self.op_revoke(rng),
            _ => self.op_invalid(rng),
        }
    }

    fn op_new_root(&mut self, rng: &mut Lcg) {
        let Some(obj) = pick(rng, &self.objects) else {
            return;
        };
        let Some(dst) = pick(rng, &self.free_slots()) else {
            return;
        };
        let cap = self.m.root_cap(obj);
        self.oracle.record_root(cap, obj);
        self.m
            .put_cap(dst.cnode, dst.slot, cap)
            .expect("slot trống");
        self.oracle.place(cap, dst);
        self.stats.adds += 1;
    }

    fn op_copy(&mut self, rng: &mut Lcg) {
        let Some(src) = pick(rng, &self.slotted()) else {
            return;
        };
        let Some(dst) = pick(rng, &self.free_slots()) else {
            return;
        };
        let info = self.m.cap_info(src).expect("cap nguồn");
        let src_slot = info.slot.expect("cap trong slot");
        let before = self.alive_set();
        let child = self.m.copy(src_slot, dst).expect("copy hợp lệ");
        self.oracle.record_child(child, src, info.obj);
        self.oracle.place(child, dst);
        assert_eq!(self.alive_set(), plus(&before, child));
        self.stats.copies += 1;
    }

    fn op_mint(&mut self, rng: &mut Lcg) {
        let Some(src) = pick(rng, &self.slotted()) else {
            return;
        };
        let Some(dst) = pick(rng, &self.free_slots()) else {
            return;
        };
        let info = self.m.cap_info(src).expect("cap nguồn");
        let src_slot = info.slot.expect("cap trong slot");
        let mut rights = 0u8;
        for mask in [RIGHT_READ, RIGHT_WRITE, RIGHT_GRANT, RIGHT_GRANT_REPLY] {
            if info.rights & mask != 0 && rng.below(2) == 1 {
                rights |= mask;
            }
        }
        let badge =
            if matches!(info.ty, ObjType::Endpoint | ObjType::Notification) && rng.below(2) == 1 {
                Some(1 + rng.below(0xFFFF))
            } else {
                None
            };
        let before = self.alive_set();
        let cap = self
            .m
            .mint(src_slot, dst, rights, badge)
            .expect("mint hợp lệ");
        self.oracle.record_child(cap, src, info.obj);
        self.oracle.place(cap, dst);
        assert_eq!(self.m.cap_info(cap).expect("cap mint").rights, rights);
        assert_eq!(self.alive_set(), plus(&before, cap));
        self.stats.mints += 1;
    }

    fn op_delete(&mut self, rng: &mut Lcg) {
        let Some(cap) = pick(rng, &self.slotted()) else {
            return;
        };
        let slot = self.m.cap_info(cap).expect("cap").slot.expect("có slot");
        let kids = self.oracle.children(cap);
        let before = self.alive_set();
        let parent = self.oracle.delete(cap);
        self.m.delete(slot).expect("delete hợp lệ");
        let after = self.alive_set();
        let mut want = before.clone();
        want.remove(&cap);
        assert_eq!(after, want, "Delete chỉ xoá đúng cap trong slot");
        for kid in kids {
            assert!(after.contains(&kid), "con của cap bị xoá phải còn sống");
            assert_eq!(
                self.m.cap_info(kid).expect("con").parent,
                parent,
                "con của cap bị xoá phải thuộc về ông"
            );
        }
        for alive in &after {
            if let Some(p) = self.m.cap_info(*alive).expect("cap sống").parent {
                assert!(after.contains(&p), "cap mồ côi: cha {p} đã chết");
            }
        }
        self.stats.deletes += 1;
    }

    fn op_revoke(&mut self, rng: &mut Lcg) {
        let before = self.alive_set();
        if rng.below(2) == 0 {
            let Some(target) = pick(rng, &self.slotted()) else {
                return;
            };
            let slot = self.m.cap_info(target).expect("cap").slot.expect("có slot");
            let doomed = self.oracle.descendants(target);
            let deleted = self
                .m
                .revoke(slot.cnode, Some(slot.slot))
                .expect("revoke hợp lệ");
            assert_eq!(deleted, doomed.len(), "revoke slot: số cap bị xoá lệch cây");
            self.oracle.remove(&doomed);
            let want: BTreeSet<CapId> = before.difference(&doomed).copied().collect();
            assert_eq!(
                self.alive_set(),
                want,
                "Revoke không được đụng ngoài cây con của cap đích"
            );
            assert!(
                self.alive_set().contains(&target),
                "Revoke phải giữ cap đích"
            );
            self.stats.revoke_caps += doomed.len();
        } else {
            let Some(cn) = pick(rng, &self.cnodes) else {
                return;
            };
            let targets = self.oracle.caps_in(cn);
            if targets.is_empty() {
                return;
            }
            let mut doomed = BTreeSet::new();
            for t in targets {
                doomed.extend(self.oracle.descendants(t));
            }
            let deleted = self.m.revoke(cn, None).expect("revoke hợp lệ");
            assert_eq!(
                deleted,
                doomed.len(),
                "revoke(None): số cap bị xoá lệch cây"
            );
            self.oracle.remove(&doomed);
            let want: BTreeSet<CapId> = before.difference(&doomed).copied().collect();
            assert_eq!(
                self.alive_set(),
                want,
                "revoke(None) không được đụng ngoài các cây con"
            );
            self.stats.revoke_caps += doomed.len();
        }
        self.stats.revokes += 1;
    }

    fn op_invalid(&mut self, rng: &mut Lcg) {
        let before = fingerprint(&self.m);
        let live_before = self.m.live_cap_count();
        match rng.below(4) {
            0 => {
                let Some(src) = pick(rng, &self.slotted()) else {
                    return;
                };
                let src_slot = self
                    .m
                    .cap_info(src)
                    .expect("cap nguồn")
                    .slot
                    .expect("có slot");
                let Some(dst_cap) = pick(rng, &self.slotted()) else {
                    return;
                };
                let dst = self
                    .m
                    .cap_info(dst_cap)
                    .expect("cap đích")
                    .slot
                    .expect("có slot");
                assert_eq!(
                    self.m.copy(src_slot, dst),
                    Err(KError::SlotOccupied),
                    "copy vào slot đã chiếm phải SlotOccupied"
                );
            }
            1 => {
                let free = self.free_slots();
                if free.len() < 2 {
                    return;
                }
                assert_eq!(
                    self.m.copy(free[0], free[1]),
                    Err(KError::SlotEmpty),
                    "copy từ slot trống phải SlotEmpty"
                );
            }
            2 => {
                let Some(src) = pick(rng, &self.slotted()) else {
                    return;
                };
                let src_slot = self
                    .m
                    .cap_info(src)
                    .expect("cap nguồn")
                    .slot
                    .expect("có slot");
                let Some(dst) = pick(rng, &self.free_slots()) else {
                    return;
                };
                assert_eq!(
                    self.m.mint(src_slot, dst, 0x80, None),
                    Err(KError::InsufficientRights),
                    "mint vượt quyền phải InsufficientRights"
                );
            }
            _ => {
                let Some(cn) = pick(rng, &self.cnodes) else {
                    return;
                };
                assert_eq!(
                    self.m.revoke(cn, Some(999)),
                    Err(KError::RangeError),
                    "revoke ngoài dải phải RangeError"
                );
            }
        }
        assert_eq!(
            fingerprint(&self.m),
            before,
            "thao tác lỗi không được đổi cây cap"
        );
        assert_eq!(self.m.live_cap_count(), live_before);
        self.stats.invalid += 1;
    }

    fn run(&mut self, seed: u64, steps: usize) -> Stats {
        let mut rng = Lcg::new(seed);
        for step in 0..steps {
            self.step(&mut rng);
            self.check(&format!("seed {seed} bước {step}"));
        }
        self.stats
    }
}

const SEEDS: u64 = 128;
const STEPS: usize = 48;

#[test]
fn property_cay_cap_tat_dinh_giu_bat_bien() {
    let mut total = Stats::default();
    for seed in 0..SEEDS {
        let mut w = World::new();
        let stats = w.run(seed, STEPS);
        total.ops += stats.ops;
        total.copies += stats.copies;
        total.mints += stats.mints;
        total.deletes += stats.deletes;
        total.revokes += stats.revokes;
        total.revoke_caps += stats.revoke_caps;
        total.invalid += stats.invalid;
        total.adds += stats.adds;
    }
    assert_eq!(
        total.ops,
        SEEDS as usize * STEPS,
        "phải chạy đủ số thao tác"
    );
    assert!(total.copies > 100, "generator phải sinh đủ Copy: {total:?}");
    assert!(total.mints > 100, "generator phải sinh đủ Mint: {total:?}");
    assert!(
        total.deletes > 50,
        "generator phải sinh đủ Delete: {total:?}"
    );
    assert!(
        total.revokes > 100,
        "generator phải sinh đủ Revoke: {total:?}"
    );
    assert!(total.revoke_caps > 100, "revoke phải thực sự xoá cap");
    assert!(total.invalid > 30, "generator phải sinh thao tác lỗi");
    println!(
        "KT-10 property: {SEEDS} seed × {STEPS} bước = {} thao tác \
         (copy {}, mint {}, delete {}, revoke {} xoá {} cap, lỗi {}, thêm gốc {})",
        total.ops,
        total.copies,
        total.mints,
        total.deletes,
        total.revokes,
        total.revoke_caps,
        total.invalid,
        total.adds
    );
}

#[test]
fn cung_seed_cung_cay_tat_dinh() {
    let mut a = World::new();
    let mut b = World::new();
    a.run(42, 40);
    b.run(42, 40);
    assert_eq!(
        fingerprint(&a.m),
        fingerprint(&b.m),
        "cùng seed ⇒ cùng cây cap"
    );
    assert_eq!(a.m.live_cap_count(), b.m.live_cap_count());
}

#[test]
fn chuoi_sau_bi_revoke_toan_bo_nhanh_khac_con_nguyen() {
    let mut m = Machine::new();
    let r = m.add_cnode(256);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let root = m.root_cap(ep);
    m.put_cap(r, 0, root).expect("gốc chuỗi");
    let other_root = m.root_cap(ep);
    m.put_cap(r, 200, other_root).expect("gốc nhánh khác");
    for i in 1..128 {
        m.copy(SlotRef::new(r, i - 1), SlotRef::new(r, i))
            .expect("copy chuỗi");
    }
    let other = m
        .copy(SlotRef::new(r, 200), SlotRef::new(r, 201))
        .expect("nhánh khác");
    m.check_caps().expect("cây trước revoke");
    assert_eq!(
        m.revoke(r, Some(0)).expect("revoke gốc chuỗi"),
        127,
        "phải xoá đúng 127 con/cháu"
    );
    assert!(m.cap_info(root).expect("gốc").alive);
    assert!(m.cap_info(other_root).expect("nhánh khác").alive);
    assert!(m.cap_info(other).expect("con nhánh khác").alive);
    assert_eq!(m.live_cap_count(), 3, "root + other_root + other");
    m.check_caps().expect("bất biến cây cap");
}

#[test]
fn revoke_giua_chuoi_chi_xoa_duoi() {
    let mut m = Machine::new();
    let r = m.add_cnode(32);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let root = m.root_cap(ep);
    m.put_cap(r, 0, root).expect("gốc chuỗi");
    for i in 1..10 {
        m.copy(SlotRef::new(r, i - 1), SlotRef::new(r, i))
            .expect("copy chuỗi");
    }
    m.check_caps().expect("cây trước revoke");
    assert_eq!(m.revoke(r, Some(5)).expect("revoke giữa chuỗi"), 4);
    for i in 0..=5 {
        assert!(m.cap_at(r, i).is_ok(), "slot {i} phải còn");
    }
    for i in 6..10 {
        assert_eq!(m.cap_at(r, i), Err(KError::SlotEmpty), "slot {i} phải chết");
    }
    m.check_caps().expect("bất biến cây cap");
}

#[test]
fn property_untyped_revoke_khong_de_cap_tro_object_chet() {
    for seed in 0..64u64 {
        let mut m = Machine::new();
        let r = m.add_cnode(16);
        let u = m.add_untyped(0, 65_536);
        let u_cap = m.root_cap(u);
        m.put_cap(r, 0, u_cap).expect("cap Untyped R0");
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(r, 1, ep_cap)
            .expect("cap Endpoint ngoài Untyped R1");

        let n = 2 + (seed % 5) as usize;
        for i in 0..n {
            let ty = match i % 3 {
                0 => ObjType::CNode,
                1 => ObjType::Tcb,
                _ => ObjType::Endpoint,
            };
            let bits = if ty == ObjType::CNode { 4 } else { 0 };
            let cap = m
                .retype(SlotRef::new(r, 0), ty, bits, 0)
                .expect("retype từ Untyped");
            let obj = m.cap_obj(cap).expect("object retype");
            m.put_cap(r, 2 + i, cap).expect("cap vào CNode gốc");
            if ty == ObjType::CNode {
                // Cap của object **ngoài** Untyped nằm trong CNode sắp bị finalise.
                let side = m.root_cap(ep);
                m.put_cap(obj, 0, side).expect("cap trong CNode con");
            }
        }
        m.check_caps().expect("cây trước revoke");
        // n cap retype (con của u_cap) + mỗi CNode trong số đó chứa 1 cap "side".
        let num_cnodes = n.div_ceil(3);
        assert_eq!(
            m.revoke(r, Some(0)).expect("revoke Untyped"),
            n + num_cnodes,
            "seed {seed}: số cap bị xoá lệch"
        );
        m.check_caps().expect("bất biến sau finalise");
        // Chỉ còn cap Untyped + cap Endpoint ngoài Untyped.
        assert_eq!(m.live_cap_count(), 2, "seed {seed}: cap sống lệch");
        let live: BTreeSet<CapId> = m
            .cap_table()
            .into_iter()
            .filter(|c| c.alive)
            .map(|c| c.cap)
            .collect();
        assert_eq!(live, BTreeSet::from([u_cap, ep_cap]));
        for info in m.cap_table().into_iter().filter(|c| c.alive) {
            assert!(m.object_alive(info.obj), "cap trỏ object chết");
            if let Some(s) = info.slot {
                assert!(m.object_alive(s.cnode), "slot nằm trong CNode chết");
                assert_eq!(m.cap_at(s.cnode, s.slot), Ok(info.cap));
            }
        }
        assert_eq!(m.untyped_used(u), 0);
        m.check_bytes(u).expect("bất biến byte");
    }
}
