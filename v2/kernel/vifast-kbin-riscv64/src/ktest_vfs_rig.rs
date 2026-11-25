// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-157** — rig của pha pager: **năm** VSpace (pager + 4 client U-mode A/B/C/D), CNode/TCB/SC,
//! Endpoint, Frame pool/đĩa/stats/kết quả.
//!
//! Tách khỏi `ktest_vfs.rs` để giữ luật cấu trúc R1 (mỗi tệp kernel/lib ≤ 1500 dòng) — cùng lý do
//! `ktest_vfs_once.rs` được tách ở KT-150. Không đổi ngữ nghĩa: mọi hằng CSpace/VA vẫn lấy từ
//! `init/pager_common.rs` (một nguồn), không chép tay.

use core::fmt::Write;

use vifast_core::{ObjId, ObjType, SlotRef};
use vifast_plat_common::obj::{CapId, ObjKind};
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, LoadedInit};
use vifast_plat_riscv64::Riscv64Virt;
use vifast_vfs::CACHE_CAPACITY;

#[path = "../init/pager_common.rs"]
mod common;

use common::{
    C_RESULT, C_VSPACE, DISK_FRAMES, P_DISK0, P_EP_FAULT, P_EP_REPLY, P_POOL0, P_STATS, P_VSPACE,
    P_VS_A, P_VS_B, P_VS_C, P_VS_D,
};

static PLATFORM: Riscv64Virt = Riscv64Virt;

static PAGER_SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_server.elf"));
static PAGER_CLIENT_A_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_client_a.elf"));
static PAGER_CLIENT_B_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_client_b.elf"));
static PAGER_CLIENT_C_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_client_c.elf"));
static PAGER_CLIENT_D_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_client_d.elf"));

/// Ô bảng dải user của **năm** tác vụ (KT-157: pager 1, A 2, B 3, C 4, D 5).
pub(crate) const LOAD_SLOT_PAGER: usize = 1;
pub(crate) const LOAD_SLOT_A: usize = 2;
pub(crate) const LOAD_SLOT_B: usize = 3;
/// KT-157: hai client C/D — ô bảng dải riêng (`MAX_PROCS = 6`).
pub(crate) const LOAD_SLOT_C: usize = 4;
pub(crate) const LOAD_SLOT_D: usize = 5;

/// Slot cspace **gốc** cho TCB/SC/Endpoint của pha pager (13..24 — các pha trước đã dọn).
const R_TCB_PAGER: usize = 13;
const R_TCB_A: usize = 14;
const R_TCB_B: usize = 15;
const R_TCB_KERN: usize = 16;
const R_SC_PAGER: usize = 17;
const R_SC_A: usize = 18;
const R_SC_B: usize = 19;
const R_EP_FAULT: usize = 20;
const R_EP_REPLY: usize = 21;
const R_CSPACE: usize = 22;
/// KT-157: TCB/SC của client C/D nằm trong **CNode của chính client** (slot 6/7) — root CNode
/// 32 slot đã bị các pha trước dùng tới slot 28 (`ktest_ring.rs:56-59`), không đủ 4 slot trống.
const C_TCB: usize = 6;
const C_SC: usize = 7;

/// Slot cspace của client cho cap Endpoint fault.
const C_FAULT_EP: usize = 5;
/// Slot cspace của **kernel** (TCB điều phối): self, ep_fault, ep_reply.
pub(crate) const K_EP_FAULT: usize = 1;
pub(crate) const K_EP_REPLY: usize = 2;

/// `u64::MAX` = chưa có.
pub(crate) const NONE_U64: u64 = u64::MAX;

/// Rig của pha pager.
pub(crate) struct Rig {
    pub(crate) cn_kern: ObjId,
    pub(crate) tcb_pager: ObjId,
    pub(crate) tcb_a: ObjId,
    pub(crate) tcb_b: ObjId,
    pub(crate) tcb_c: ObjId,
    pub(crate) tcb_d: ObjId,
    pub(crate) tcb_kern: ObjId,
    pub(crate) vs_a: CapId,
    pub(crate) vs_b: CapId,
    pub(crate) vs_c: CapId,
    pub(crate) vs_d: CapId,
    pub(crate) core_vs_a: ObjId,
    pub(crate) core_vs_b: ObjId,
    pub(crate) core_vs_c: ObjId,
    pub(crate) core_vs_d: ObjId,
    pub(crate) stats_pa: u64,
    pub(crate) result_a_pa: u64,
    pub(crate) result_b_pa: u64,
    pub(crate) pool_pa: [u64; CACHE_CAPACITY],
}

/// Một tác vụ U-mode chạy tiếp được.
pub(crate) struct Task<'a> {
    pub(crate) loaded: &'a LoadedInit,
    pub(crate) tf: TrapFrame,
    pub(crate) tcb: ObjId,
    pub(crate) slot: usize,
    pub(crate) exited: bool,
    pub(crate) code: u64,
}

impl<'a> Task<'a> {
    pub(crate) fn new(loaded: &'a LoadedInit, tcb: ObjId, slot: usize) -> Self {
        let mut tf = TrapFrame::zeroed();
        tf.x[REG_SP] = user::USER_STACK_TOP;
        tf.sepc = loaded.entry;
        Self {
            loaded,
            tf,
            tcb,
            slot,
            exited: false,
            code: NONE_U64,
        }
    }
}

/// Tạo một Frame 4 KiB từ Untyped + trả `(key bắc cầu, PA)` — qua **object layer**.
fn new_frame(load_untyped: CapId, size_bits: u8) -> Option<(u64, u64)> {
    let m = aobj::machine();
    let cap = m.retype(load_untyped, ObjKind::Frame, size_bits).ok()?;
    let id = m.cap_key(cap).ok()?;
    let pa = m.frame_pa(cap).ok()?;
    Some((mem_bridge::make_key(cap, id), pa))
}

/// Dựng rig: **năm** CNode/TCB/SC (pager + 4 client + kernel), hai Endpoint, Frame + cap VSpace.
///
/// KT-157: bốn client U-mode **cùng lúc**, mỗi client một VSpace + CSpace riêng; C/D tối giản
/// (không khối kết quả, stack 1 trang) để vừa `MAX_OBJECTS = 96` và arena 96 KiB.
pub(crate) fn build_rig(
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    stage: &mut &'static str,
) -> Option<(
    Rig,
    LoadedInit,
    LoadedInit,
    LoadedInit,
    LoadedInit,
    LoadedInit,
)> {
    *stage = "load pager";
    let pager = match user::load_init_slot(PAGER_SERVER_ELF, load_untyped, LOAD_SLOT_PAGER) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "load client a";
    let client_a = match user::load_init_slot(PAGER_CLIENT_A_ELF, load_untyped, LOAD_SLOT_A) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "load client b";
    let client_b = match user::load_init_slot(PAGER_CLIENT_B_ELF, load_untyped, LOAD_SLOT_B) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    // KT-157: C/D tối giản — stack **1 trang** (không đệ quy) để vừa `MAX_MAPPINGS = 64`.
    *stage = "load client c";
    let client_c =
        match user::load_init_slot_with_stack(PAGER_CLIENT_C_ELF, load_untyped, LOAD_SLOT_C, 1) {
            Ok(loaded) => loaded,
            Err(error) => {
                *stage = error.name();
                return None;
            }
        };
    *stage = "load client d";
    let client_d =
        match user::load_init_slot_with_stack(PAGER_CLIENT_D_ELF, load_untyped, LOAD_SLOT_D, 1) {
            Ok(loaded) => loaded,
            Err(error) => {
                *stage = error.name();
                return None;
            }
        };

    // ---- object layer: key VSpace + Frame (3 pool, 5 đĩa 4 KiB, stats, 2 result)
    *stage = "object keys";
    let vs_pager_key = {
        let m = aobj::machine();
        let id = m.cap_key(pager.vspace).ok()?;
        mem_bridge::make_key(pager.vspace, id)
    };
    let vs_a_key = {
        let m = aobj::machine();
        let id = m.cap_key(client_a.vspace).ok()?;
        mem_bridge::make_key(client_a.vspace, id)
    };
    let vs_b_key = {
        let m = aobj::machine();
        let id = m.cap_key(client_b.vspace).ok()?;
        mem_bridge::make_key(client_b.vspace, id)
    };
    let vs_c_key = {
        let m = aobj::machine();
        let id = m.cap_key(client_c.vspace).ok()?;
        mem_bridge::make_key(client_c.vspace, id)
    };
    let vs_d_key = {
        let m = aobj::machine();
        let id = m.cap_key(client_d.vspace).ok()?;
        mem_bridge::make_key(client_d.vspace, id)
    };
    *stage = "object frames";
    let mut pool_pa = [0u64; CACHE_CAPACITY];
    let mut frame_keys = [0u64; CACHE_CAPACITY];
    let mut index = 0usize;
    while index < CACHE_CAPACITY {
        let (key, pa) = new_frame(load_untyped, 12)?;
        frame_keys[index] = key;
        pool_pa[index] = pa;
        index += 1;
    }
    let mut disk_keys = [0u64; DISK_FRAMES as usize];
    let mut disk_index = 0usize;
    while disk_index < DISK_FRAMES as usize {
        let (key, _pa) = new_frame(load_untyped, 12)?;
        disk_keys[disk_index] = key;
        disk_index += 1;
    }
    let (stats_key, stats_pa) = new_frame(load_untyped, 12)?;
    let (result_a_key, result_a_pa) = new_frame(load_untyped, 12)?;
    let (result_b_key, result_b_pa) = new_frame(load_untyped, 12)?;

    let rig = core_bridge::with_machine(|m| -> Option<Rig> {
        *stage = "cnodes";
        let cn_pager = m.add_cnode(32);
        let cn_a = m.add_cnode(16);
        let cn_b = m.add_cnode(16);
        let cn_c = m.add_cnode(16);
        let cn_d = m.add_cnode(16);
        let cn_kern = m.add_cnode(16);
        // Slot 0 của mỗi CNode = cap **CNode** của chính nó (không phải TCB) — pager dùng
        // `P_SELF` làm aux sai loại cho ca âm TAP 35.
        for cn in [cn_pager, cn_a, cn_b, cn_c, cn_d, cn_kern] {
            let self_cap = m.root_cap(cn);
            m.put_cap(cn, 0, self_cap).ok()?;
        }
        let root_self = m.root_cap(root);
        m.put_cap(root, R_CSPACE, root_self).ok()?;
        *stage = "endpoints";
        let ep_fault_cap = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
        let ep_reply_cap = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
        m.put_cap(root, R_EP_FAULT, ep_fault_cap).ok()?;
        m.put_cap(root, R_EP_REPLY, ep_reply_cap).ok()?;
        m.copy(
            SlotRef::new(root, R_EP_FAULT),
            SlotRef::new(cn_pager, P_EP_FAULT as usize),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_REPLY),
            SlotRef::new(cn_pager, P_EP_REPLY as usize),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_FAULT),
            SlotRef::new(cn_kern, K_EP_FAULT),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_REPLY),
            SlotRef::new(cn_kern, K_EP_REPLY),
        )
        .ok()?;
        for cn in [cn_a, cn_b, cn_c, cn_d] {
            m.copy(SlotRef::new(root, R_EP_FAULT), SlotRef::new(cn, C_FAULT_EP))
                .ok()?;
        }

        *stage = "vspaces";
        let vs_pager_obj = m.add_foreign_object(ObjType::VSpace, vs_pager_key).ok()?;
        let vs_a_obj = m.add_foreign_object(ObjType::VSpace, vs_a_key).ok()?;
        let vs_b_obj = m.add_foreign_object(ObjType::VSpace, vs_b_key).ok()?;
        let vs_c_obj = m.add_foreign_object(ObjType::VSpace, vs_c_key).ok()?;
        let vs_d_obj = m.add_foreign_object(ObjType::VSpace, vs_d_key).ok()?;
        {
            let cap = m.root_cap(vs_pager_obj);
            m.put_cap(cn_pager, P_VSPACE as usize, cap)
        }
        .ok()?;
        for (slot, obj) in [
            (P_VS_A, vs_a_obj),
            (P_VS_B, vs_b_obj),
            (P_VS_C, vs_c_obj),
            (P_VS_D, vs_d_obj),
        ] {
            let cap = m.root_cap(obj);
            m.put_cap(cn_pager, slot as usize, cap).ok()?;
        }
        for (cn, obj) in [
            (cn_a, vs_a_obj),
            (cn_b, vs_b_obj),
            (cn_c, vs_c_obj),
            (cn_d, vs_d_obj),
        ] {
            let cap = m.root_cap(obj);
            m.put_cap(cn, C_VSPACE as usize, cap).ok()?;
        }

        *stage = "frame caps";
        for (slot, frame_key) in frame_keys.iter().enumerate() {
            let obj = m.add_foreign_object(ObjType::Frame, *frame_key).ok()?;
            {
                let cap = m.root_cap(obj);
                m.put_cap(cn_pager, P_POOL0 as usize + slot, cap)
            }
            .ok()?;
        }
        for (slot, disk_key) in disk_keys.iter().enumerate() {
            let obj = m.add_foreign_object(ObjType::Frame, *disk_key).ok()?;
            let cap = m.root_cap(obj);
            m.put_cap(cn_pager, P_DISK0 as usize + slot, cap).ok()?;
        }
        let stats_obj = m.add_foreign_object(ObjType::Frame, stats_key).ok()?;
        {
            let cap = m.root_cap(stats_obj);
            m.put_cap(cn_pager, P_STATS as usize, cap)
        }
        .ok()?;
        let result_a_obj = m.add_foreign_object(ObjType::Frame, result_a_key).ok()?;
        {
            let cap = m.root_cap(result_a_obj);
            m.put_cap(cn_a, C_RESULT as usize, cap)
        }
        .ok()?;
        let result_b_obj = m.add_foreign_object(ObjType::Frame, result_b_key).ok()?;
        {
            let cap = m.root_cap(result_b_obj);
            m.put_cap(cn_b, C_RESULT as usize, cap)
        }
        .ok()?;

        *stage = "tbcs";
        let mut tcb_objs = [0u32; 6];
        // (cnode TCB, slot TCB, cnode SC, slot SC, cspace của tác vụ)
        let plans = [
            (root, R_TCB_PAGER, root, R_SC_PAGER, cn_pager),
            (root, R_TCB_A, root, R_SC_A, cn_a),
            (root, R_TCB_B, root, R_SC_B, cn_b),
            (cn_c, C_TCB, cn_c, C_SC, cn_c),
            (cn_d, C_TCB, cn_d, C_SC, cn_d),
            (root, R_TCB_KERN, root, R_SC_PAGER, cn_kern),
        ];
        for (index, (tcb_cnode, tcb_slot, sc_cnode, sc_slot, cnode)) in
            plans.into_iter().enumerate()
        {
            let tcb_cap = match m.retype(us, ObjType::Tcb, 0, 0) {
                Ok(cap) => cap,
                Err(error) => {
                    *stage = "tcb retype fail";
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# pager: tcb retype err={:?} index={index}", error);
                    return None;
                }
            };
            *stage = "tcb cap_obj";
            let tcb = m.cap_obj(tcb_cap).ok()?;
            *stage = "tcb put_cap";
            m.put_cap(tcb_cnode, tcb_slot, tcb_cap).ok()?;
            if index != 5 {
                *stage = "tcb sc retype";
                let sc_cap = m.retype(us, ObjType::SchedContext, 0, 0).ok()?;
                *stage = "tcb sc put_cap";
                m.put_cap(sc_cnode, sc_slot, sc_cap).ok()?;
            }
            *stage = "tcb configure";
            m.tcb_configure(
                SlotRef::new(tcb_cnode, tcb_slot),
                Some(SlotRef::new(cnode, 0)),
            )
            .ok()?;
            *stage = "tcb sc configure";
            m.sched_control_configure(ctl, SlotRef::new(sc_cnode, sc_slot), 500, 5_000)
                .ok()?;
            *stage = "tcb sched params";
            m.set_sched_params(
                SlotRef::new(tcb_cnode, tcb_slot),
                Some(SlotRef::new(sc_cnode, sc_slot)),
            )
            .ok()?;
            *stage = "tcb priority";
            m.tcb_set_priority(SlotRef::new(tcb_cnode, tcb_slot), 3, 10)
                .ok()?;
            if index == 5 {
                *stage = "tcb ready";
                m.start_tcb(tcb).ok()?;
            }
            tcb_objs[index] = tcb;
        }
        for (tcb_cnode, tcb_slot, cn) in [
            (root, R_TCB_A, cn_a),
            (root, R_TCB_B, cn_b),
            (cn_c, C_TCB, cn_c),
            (cn_d, C_TCB, cn_d),
        ] {
            m.tcb_set_fault_ep(
                SlotRef::new(tcb_cnode, tcb_slot),
                SlotRef::new(cn, C_FAULT_EP),
            )
            .ok()?;
        }
        Some(Rig {
            cn_kern,
            tcb_pager: tcb_objs[0],
            tcb_a: tcb_objs[1],
            tcb_b: tcb_objs[2],
            tcb_c: tcb_objs[3],
            tcb_d: tcb_objs[4],
            tcb_kern: tcb_objs[5],
            vs_a: client_a.vspace,
            vs_b: client_b.vspace,
            vs_c: client_c.vspace,
            vs_d: client_d.vspace,
            core_vs_a: vs_a_obj,
            core_vs_b: vs_b_obj,
            core_vs_c: vs_c_obj,
            core_vs_d: vs_d_obj,
            stats_pa,
            result_a_pa,
            result_b_pa,
            pool_pa,
        })
    });
    rig.map(|rig| (rig, pager, client_a, client_b, client_c, client_d))
}
