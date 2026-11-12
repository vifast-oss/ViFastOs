// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-11 — so vi sai `vifast-model` với `vifast-sim` trên **10.000 seed**.
//!
//! Cùng seed ⇒ cùng kịch bản (bộ sinh LCG nội bộ); hai bên nhận **cùng chuỗi thao tác**,
//! so **cùng kết quả** (Ok + số cap / tên mã lỗi) và **cùng trạng thái chuẩn hoá** sau mỗi
//! bước (bảng cap + bảng object + `check_caps`/`check_all_bytes`), băm thành một chuỗi trace.
//! Đây là **parity test**: `vifast-model` không re-export `kobj` của sim (hai cài đặt riêng,
//! cùng đọc `spec/`), nên phép so có nghĩa; không tuyên bố "độc lập" tuyệt đối.
//! `check_caps` + `check_all_bytes` được assert sau **mỗi** bước (không chỉ cuối seed).

use std::collections::BTreeSet;

use vifast_model::{
    KError as ModelError, Machine as ModelMachine, ObjId as ModelObjId, ObjType as ModelObjType,
    SlotRef as ModelSlot,
};
use vifast_sim::kobj::{
    KError as SimError, Machine as SimMachine, ObjId as SimObjId, ObjType as SimObjType,
    SlotRef as SimSlot,
};

const SEEDS: u64 = 10_000;
const OPS_PER_SEED: usize = 24;

/// Bộ sinh tất định — cùng seed ⇒ cùng dãy; không dùng thời gian.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(
            seed.wrapping_mul(6364136223846793005)
                .wrapping_add(0x9e3779b97f4a7c15),
        )
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 17
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

fn fold(h: u64, v: u64) -> u64 {
    h.rotate_left(7)
        ^ v.wrapping_add(0x9e3779b97f4a7c15)
            .wrapping_mul(0x100000001b3)
}

fn ty_id_model(ty: ModelObjType) -> u8 {
    match ty {
        ModelObjType::Untyped => 1,
        ModelObjType::CNode => 2,
        ModelObjType::Tcb => 3,
        ModelObjType::SchedContext => 4,
        ModelObjType::Endpoint => 5,
        ModelObjType::Notification => 6,
        ModelObjType::Reply => 7,
        ModelObjType::SchedControl => 8,
        ModelObjType::VSpace => 9,
        ModelObjType::PageTable => 10,
        ModelObjType::Frame => 11,
        ModelObjType::IrqControl => 12,
        ModelObjType::IrqHandler => 13,
        ModelObjType::IOSpace => 14,
        ModelObjType::VCpu => 15,
        ModelObjType::PowerControl => 16,
    }
}

fn ty_id_sim(ty: SimObjType) -> u8 {
    match ty {
        SimObjType::Untyped => 1,
        SimObjType::CNode => 2,
        SimObjType::Tcb => 3,
        SimObjType::SchedContext => 4,
        SimObjType::Endpoint => 5,
        SimObjType::Notification => 6,
        SimObjType::Reply => 7,
        SimObjType::SchedControl => 8,
        SimObjType::VSpace => 9,
        SimObjType::PageTable => 10,
        SimObjType::Frame => 11,
        SimObjType::IrqControl => 12,
        SimObjType::IrqHandler => 13,
        SimObjType::IOSpace => 14,
        SimObjType::VCpu => 15,
        SimObjType::PowerControl => 16,
    }
}

fn hash_model(m: &ModelMachine) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for c in m.cap_table() {
        h = fold(h, c.alive as u64);
        h = fold(h, c.obj as u64);
        h = fold(h, ty_id_model(c.ty) as u64);
        h = fold(h, c.parent.map(u64::from).unwrap_or(0));
        h = fold(
            h,
            c.slot
                .map(|s| ((s.cnode as u64) << 32) | (s.slot as u64 + 1))
                .unwrap_or(0),
        );
        h = fold(h, c.rights as u64);
        h = fold(h, c.badge.map(|b| b + 1).unwrap_or(0));
    }
    let mut obj = 1u32;
    while let Some(ty) = m.object_type(obj) {
        h = fold(h, obj as u64);
        h = fold(h, m.object_alive(obj) as u64);
        h = fold(h, ty_id_model(ty) as u64);
        h = fold(
            h,
            m.object_bytes(obj)
                .map(|(o, l)| ((o as u64) << 32) | l as u64)
                .unwrap_or(0),
        );
        if ty == ModelObjType::Untyped {
            h = fold(h, m.untyped_size(obj) as u64);
            h = fold(h, m.untyped_used(obj) as u64);
            h = fold(h, m.untyped_allocated(obj) as u64);
            h = fold(h, m.untyped_gap(obj) as u64);
        }
        obj += 1;
    }
    h = fold(h, m.check_caps().is_ok() as u64);
    h = fold(h, m.check_all_bytes().is_ok() as u64);
    h
}

fn hash_sim(m: &SimMachine) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for c in m.cap_table() {
        h = fold(h, c.alive as u64);
        h = fold(h, c.obj as u64);
        h = fold(h, ty_id_sim(c.ty) as u64);
        h = fold(h, c.parent.map(u64::from).unwrap_or(0));
        h = fold(
            h,
            c.slot
                .map(|s| ((s.cnode as u64) << 32) | (s.slot as u64 + 1))
                .unwrap_or(0),
        );
        h = fold(h, c.rights as u64);
        h = fold(h, c.badge.map(|b| b + 1).unwrap_or(0));
    }
    let mut obj = 1u32;
    while let Some(ty) = m.object_type(obj) {
        h = fold(h, obj as u64);
        h = fold(h, m.object_alive(obj) as u64);
        h = fold(h, ty_id_sim(ty) as u64);
        h = fold(
            h,
            m.object_bytes(obj)
                .map(|(o, l)| ((o as u64) << 32) | l as u64)
                .unwrap_or(0),
        );
        if ty == SimObjType::Untyped {
            h = fold(h, m.untyped_size(obj) as u64);
            h = fold(h, m.untyped_used(obj) as u64);
            h = fold(h, m.untyped_allocated(obj) as u64);
            h = fold(h, m.untyped_gap(obj) as u64);
        }
        obj += 1;
    }
    h = fold(h, m.check_caps().is_ok() as u64);
    h = fold(h, m.check_all_bytes().is_ok() as u64);
    h
}

fn render_model(m: &ModelMachine) -> String {
    let mut out = String::new();
    for c in m.cap_table() {
        out.push_str(&format!(
            "cap {}: alive={} obj={} ty={} parent={:?} slot={:?} rights={:#04x} badge={:?}\n",
            c.cap,
            c.alive,
            c.obj,
            ty_id_model(c.ty),
            c.parent,
            c.slot,
            c.rights,
            c.badge
        ));
    }
    let mut obj = 1u32;
    while let Some(ty) = m.object_type(obj) {
        out.push_str(&format!(
            "obj {}: alive={} ty={} bytes={:?} untyped={:?}/{:?}/{:?}\n",
            obj,
            m.object_alive(obj),
            ty_id_model(ty),
            m.object_bytes(obj),
            m.untyped_size(obj),
            m.untyped_used(obj),
            m.untyped_allocated(obj)
        ));
        obj += 1;
    }
    out
}

fn render_sim(m: &SimMachine) -> String {
    let mut out = String::new();
    for c in m.cap_table() {
        out.push_str(&format!(
            "cap {}: alive={} obj={} ty={} parent={:?} slot={:?} rights={:#04x} badge={:?}\n",
            c.cap,
            c.alive,
            c.obj,
            ty_id_sim(c.ty),
            c.parent,
            c.slot,
            c.rights,
            c.badge
        ));
    }
    let mut obj = 1u32;
    while let Some(ty) = m.object_type(obj) {
        out.push_str(&format!(
            "obj {}: alive={} ty={} bytes={:?} untyped={:?}/{:?}/{:?}\n",
            obj,
            m.object_alive(obj),
            ty_id_sim(ty),
            m.object_bytes(obj),
            m.untyped_size(obj),
            m.untyped_used(obj),
            m.untyped_allocated(obj)
        ));
        obj += 1;
    }
    out
}

/// Kịch bản: chỉ các thao tác **có ở cả hai** mô hình (sim chưa có `Move`).
#[derive(Debug, Clone, Copy)]
enum Op {
    Retype {
        src: usize,
        ty: u8,
        size_bits: u32,
    },
    Copy {
        src: usize,
        dst: usize,
    },
    Mint {
        src: usize,
        dst: usize,
        rights: u8,
        badge: u8,
    },
    Delete {
        slot: usize,
    },
    Revoke {
        cnode: usize,
        slot: u8,
    },
}

fn norm_model(r: Result<u64, ModelError>) -> Result<u64, &'static str> {
    r.map_err(ModelError::name)
}

fn norm_sim(r: Result<u64, SimError>) -> Result<u64, &'static str> {
    r.map_err(SimError::name)
}

fn model_obj(ty: u8) -> ModelObjType {
    match ty {
        0 => ModelObjType::Tcb,
        1 => ModelObjType::Endpoint,
        2 => ModelObjType::Notification,
        3 => ModelObjType::Reply,
        4 => ModelObjType::SchedContext,
        _ => ModelObjType::CNode,
    }
}

fn sim_obj(ty: u8) -> SimObjType {
    match ty {
        0 => SimObjType::Tcb,
        1 => SimObjType::Endpoint,
        2 => SimObjType::Notification,
        3 => SimObjType::Reply,
        4 => SimObjType::SchedContext,
        _ => SimObjType::CNode,
    }
}

fn badge_of(code: u8) -> Option<u64> {
    match code {
        0 => None,
        1 => Some(0),
        2 => Some(7),
        _ => Some(1),
    }
}

fn apply_model(
    m: &mut ModelMachine,
    slots: &[ModelSlot],
    cnodes: &[ModelObjId],
    op: Op,
) -> Result<u64, &'static str> {
    match op {
        Op::Retype { src, ty, size_bits } => {
            let result = match ty {
                5 => m.retype(slots[src], ModelObjType::Untyped, size_bits, 0),
                6 => m.retype(slots[src], ModelObjType::CNode, size_bits, 0),
                _ => m.retype(slots[src], model_obj(ty), 0, 0),
            };
            match result {
                Ok(cap) => match slots
                    .iter()
                    .position(|s| m.cap_at(s.cnode, s.slot).is_err())
                {
                    Some(i) => norm_model(
                        m.put_cap(slots[i].cnode, slots[i].slot, cap)
                            .map(|()| u64::from(cap)),
                    ),
                    None => Ok(u64::from(cap)),
                },
                Err(e) => Err(e.name()),
            }
        }
        Op::Copy { src, dst } => norm_model(m.copy(slots[src], slots[dst]).map(u64::from)),
        Op::Mint {
            src,
            dst,
            rights,
            badge,
        } => norm_model(
            m.mint(slots[src], slots[dst], rights, badge_of(badge))
                .map(u64::from),
        ),
        Op::Delete { slot } => norm_model(m.delete(slots[slot]).map(|()| 1)),
        Op::Revoke { cnode, slot } => {
            let slot = if slot == 0xFF {
                None
            } else {
                Some(slot as usize)
            };
            norm_model(m.revoke(cnodes[cnode], slot).map(|n| n as u64))
        }
    }
}

fn apply_sim(
    m: &mut SimMachine,
    slots: &[SimSlot],
    cnodes: &[SimObjId],
    op: Op,
) -> Result<u64, &'static str> {
    match op {
        Op::Retype { src, ty, size_bits } => {
            let result = match ty {
                5 => m.retype(slots[src], SimObjType::Untyped, size_bits, 0),
                6 => m.retype(slots[src], SimObjType::CNode, size_bits, 0),
                _ => m.retype(slots[src], sim_obj(ty), 0, 0),
            };
            match result {
                Ok(cap) => match slots
                    .iter()
                    .position(|s| m.cap_at(s.cnode, s.slot).is_err())
                {
                    Some(i) => norm_sim(
                        m.put_cap(slots[i].cnode, slots[i].slot, cap)
                            .map(|()| u64::from(cap)),
                    ),
                    None => Ok(u64::from(cap)),
                },
                Err(e) => Err(e.name()),
            }
        }
        Op::Copy { src, dst } => norm_sim(m.copy(slots[src], slots[dst]).map(u64::from)),
        Op::Mint {
            src,
            dst,
            rights,
            badge,
        } => norm_sim(
            m.mint(slots[src], slots[dst], rights, badge_of(badge))
                .map(u64::from),
        ),
        Op::Delete { slot } => norm_sim(m.delete(slots[slot]).map(|()| 1)),
        Op::Revoke { cnode, slot } => {
            let slot = if slot == 0xFF {
                None
            } else {
                Some(slot as usize)
            };
            norm_sim(m.revoke(cnodes[cnode], slot).map(|n| n as u64))
        }
    }
}

fn build_model() -> (ModelMachine, Vec<ModelSlot>, Vec<ModelObjId>) {
    let mut m = ModelMachine::new();
    let u0 = m.add_untyped(0x1000, 8192);
    let u1 = m.add_untyped(0x4000, 4096);
    let c0 = m.add_cnode(8);
    let c1 = m.add_cnode(8);
    let c2 = m.add_cnode(8);
    let ep = m.add_kernel_object(ModelObjType::Endpoint);
    let ntf = m.add_kernel_object(ModelObjType::Notification);
    let tcb = m.add_kernel_object(ModelObjType::Tcb);
    for (slot, obj) in [(0usize, u0), (1, u1), (2, ep), (3, ntf), (4, tcb), (5, c0)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, slot, cap).expect("cap gốc");
    }
    let mut slots = Vec::new();
    for c in [c0, c1, c2] {
        for s in 0..8 {
            slots.push(ModelSlot::new(c, s));
        }
    }
    (m, slots, vec![c0, c1, c2])
}

fn build_sim() -> (SimMachine, Vec<SimSlot>, Vec<SimObjId>) {
    let mut m = SimMachine::new();
    let u0 = m.add_untyped(0x1000, 8192);
    let u1 = m.add_untyped(0x4000, 4096);
    let c0 = m.add_cnode(8);
    let c1 = m.add_cnode(8);
    let c2 = m.add_cnode(8);
    let ep = m.add_kernel_object(SimObjType::Endpoint);
    let ntf = m.add_kernel_object(SimObjType::Notification);
    let tcb = m.add_kernel_object(SimObjType::Tcb);
    for (slot, obj) in [(0usize, u0), (1, u1), (2, ep), (3, ntf), (4, tcb), (5, c0)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, slot, cap).expect("cap gốc");
    }
    let mut slots = Vec::new();
    for c in [c0, c1, c2] {
        for s in 0..8 {
            slots.push(SimSlot::new(c, s));
        }
    }
    (m, slots, vec![c0, c1, c2])
}

/// Sinh một thao tác từ seed; `model` chỉ dùng để chọn tập con quyền của cap nguồn.
fn gen_op(rng: &mut Rng, model: &ModelMachine, slots: &[ModelSlot], cnodes: &[ModelObjId]) -> Op {
    match rng.below(100) {
        0..=29 => {
            // Retype trả cap cho harness đặt vào slot; hết slot trống thì đổi sang Delete
            // để không tạo cap con mồ côi (bất biến check_caps kiểm sau mỗi bước).
            if slots.iter().all(|s| model.cap_at(s.cnode, s.slot).is_ok()) {
                return Op::Delete {
                    slot: rng.below(slots.len() as u64) as usize,
                };
            }
            let kind = rng.below(7);
            let (ty, size_bits) = match kind {
                0 => (0, 0),
                1 => (1, 0),
                2 => (2, 0),
                3 => (3, 0),
                4 => (4, 0),
                5 => (6, 2 + rng.below(2) as u32),
                _ => (5, 12),
            };
            Op::Retype {
                src: rng.below(slots.len() as u64) as usize,
                ty,
                size_bits,
            }
        }
        30..=49 => Op::Copy {
            src: rng.below(slots.len() as u64) as usize,
            dst: rng.below(slots.len() as u64) as usize,
        },
        50..=69 => {
            let src = rng.below(slots.len() as u64) as usize;
            let rights = match model.cap_at(slots[src].cnode, slots[src].slot) {
                Ok(cap) => {
                    let r = model.cap_info(cap).expect("cap").rights;
                    r & (rng.next() as u8)
                }
                Err(_) => 0,
            };
            Op::Mint {
                src,
                dst: rng.below(slots.len() as u64) as usize,
                rights,
                badge: rng.below(4) as u8,
            }
        }
        70..=84 => Op::Delete {
            slot: rng.below(slots.len() as u64) as usize,
        },
        _ => Op::Revoke {
            cnode: rng.below(cnodes.len() as u64) as usize,
            slot: rng.below(9) as u8,
        },
    }
}

#[test]
fn kt11_diff_model_va_sim_10000_seed() {
    let start = std::time::Instant::now();
    let mut total_ops = 0u64;
    let mut traces: BTreeSet<u64> = BTreeSet::new();
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed);
        let (mut ma, slots_m, cnodes_m) = build_model();
        let (mut mb, slots_s, cnodes_s) = build_sim();
        assert_eq!(
            hash_model(&ma),
            hash_sim(&mb),
            "seed {seed}: trạng thái đầu khác"
        );
        let mut chain = 0xcbf29ce484222325u64;
        for step in 0..OPS_PER_SEED {
            let op = gen_op(&mut rng, &ma, &slots_m, &cnodes_m);
            let ra = apply_model(&mut ma, &slots_m, &cnodes_m, op);
            let rb = apply_sim(&mut mb, &slots_s, &cnodes_s, op);
            if ra != rb {
                panic!(
                    "seed {seed} bước {step}: kết quả khác\nop={op:?}\nmodel={ra:?}\nsim={rb:?}"
                );
            }
            let ha = hash_model(&ma);
            let hb = hash_sim(&mb);
            if ha != hb {
                panic!(
                    "seed {seed} bước {step}: trạng thái khác\nop={op:?}\n--- model\n{}\n--- sim\n{}",
                    render_model(&ma),
                    render_sim(&mb)
                );
            }
            if let Err(e) = ma.check_caps() {
                panic!("seed {seed} bước {step}: model check_caps hỏng: {e}\nop={op:?}");
            }
            if let Err(e) = ma.check_all_bytes() {
                panic!("seed {seed} bước {step}: model check_all_bytes hỏng: {e}\nop={op:?}");
            }
            if let Err(e) = mb.check_caps() {
                panic!("seed {seed} bước {step}: sim check_caps hỏng: {e}\nop={op:?}");
            }
            if let Err(e) = mb.check_all_bytes() {
                panic!("seed {seed} bước {step}: sim check_all_bytes hỏng: {e}\nop={op:?}");
            }
            chain = fold(chain, ha);
            total_ops += 1;
        }
        ma.check_caps().expect("model: cây cap sau seed");
        ma.check_all_bytes().expect("model: byte sau seed");
        mb.check_caps().expect("sim: cây cap sau seed");
        mb.check_all_bytes().expect("sim: byte sau seed");
        traces.insert(chain);
    }
    let elapsed = start.elapsed().as_millis();
    println!(
        "# KT-11 diff: {SEEDS} seed, {total_ops} thao tác, {} trace khác nhau, 0 lệch ({elapsed} ms)",
        traces.len()
    );
    assert_eq!(total_ops, SEEDS * OPS_PER_SEED as u64);
    assert!(
        traces.len() > 9_000,
        "trace quá trùng lặp ({}), bộ sinh chưa đủ đa dạng",
        traces.len()
    );
}
