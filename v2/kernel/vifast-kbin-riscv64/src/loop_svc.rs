// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-345b** — `LoopSvc`: dịch vụ `loop <n>` của **kernel** trên ảnh sản phẩm riscv64.
//!
//! # Hợp đồng (không phải ABI — thoả thuận shell ↔ kernel, như `proc_fs`/`net_svc`)
//!
//! ```text
//! shell U-mode                              kernel (dịch vụ này)
//!   NBSend(EP_LOOP, Loop(n))  ─────────────►  nb_recv(EP_LOOP)
//!                                              lặp n vòng: Retype(Tcb)+Retype(SC) từ arena riêng,
//!                                              Configure/Start, rồi Delete+Revoke ⇒ finalise
//!   NBRecv(EP_REPLY)  ◄───────────────────    nb_send(EP_REPLY, Loop(ok, before, after))
//! ```
//!
//! # Vì sao có Untyped riêng (ô [`LOOP_ARENA`])
//!
//! `Revoke` một Untyped đặt watermark về 0 **và finalise mọi object trong cây** — nếu dùng chung
//! Untyped U2 thì mỗi vòng `loop` sẽ thổi bay endpoint/TCB của cả ảnh. Vì vậy lúc dựng, kernel
//! `Retype(Untyped, size_bits=12)` tách một arena **4 KiB** riêng; mỗi vòng cấp TCB +
//! `SchedContext` từ đó, `Start`, rồi `Delete` hai cap + `Revoke` arena ⇒ vòng đời spawn→destroy
//! **thật** (không phải đếm suông). `before`/`after` là `total_used()` của object layer: hai số
//! phải **bằng nhau** thì vòng đó mới được tính `ok` — đây là phép kiểm rò rỉ ngay trên máy.
//!
//! Không `unsafe`, không cấp phát; `n` bị chặn trên bởi [`LOOP_MAX`] (= 32, cùng ngưỡng v1).

use vifast_core::{Machine, Message, MessageInfo, ObjId, ObjType, SlotRef};

use crate::proc_abi::{
    EP_CTL, EP_LOOP, EP_REPLY, LOOP_ARENA, LOOP_MAX, LOOP_REPLY, LOOP_REQ, LOOP_SC_SLOT,
    LOOP_TCB_SLOT, SLOT_CSPACE,
};

/// Dịch vụ `loop`: endpoint request + reply + arena riêng + cap cần để dựng TCB.
pub struct LoopSvc {
    ep: SlotRef,
    reply: SlotRef,
    tcb_k: ObjId,
    arena: SlotRef,
    ctl: SlotRef,
    cspace: SlotRef,
}

/// Kết quả dựng: `proc_rt` giữ dịch vụ trong `Rig`.
pub struct LoopRig {
    pub svc: LoopSvc,
}

fn msg(label: u64, words: &[u64]) -> Message {
    let info = MessageInfo::new(label as u32, 0, false, 0, words.len() as u8)
        .expect("KT-345b: MessageInfo loop");
    Message::new(info, words).expect("KT-345b: Message loop")
}

/// Dựng `Endpoint EP_LOOP` + Untyped 4 KiB riêng và dịch vụ.
pub fn build(m: &mut Machine, root: ObjId, us: SlotRef, tcb_k: ObjId) -> LoopRig {
    // `size_bits = 12` ⇒ 4096 B: đủ cho TCB + SchedContext của **một** vòng (Revoke sau mỗi vòng
    // nên không tích luỹ), và ≤ `ARENA_ALIGN` mà `Untyped.Retype` chấp nhận.
    let arena_cap = m
        .retype(us, ObjType::Untyped, 12, 0)
        .expect("KT-345b: retype LOOP_ARENA");
    m.put_cap(root, LOOP_ARENA, arena_cap)
        .expect("KT-345b: put LOOP_ARENA");
    let ep_cap = m
        .retype(us, ObjType::Endpoint, 0, 0)
        .expect("KT-345b: retype EP_LOOP");
    m.put_cap(root, EP_LOOP, ep_cap)
        .expect("KT-345b: put EP_LOOP");
    LoopRig {
        svc: LoopSvc {
            ep: SlotRef::new(root, EP_LOOP),
            reply: SlotRef::new(root, EP_REPLY),
            tcb_k,
            arena: SlotRef::new(root, LOOP_ARENA),
            ctl: SlotRef::new(root, EP_CTL),
            cspace: SlotRef::new(root, SLOT_CSPACE),
        },
    }
}

impl LoopSvc {
    /// Một vòng spawn→destroy: cấp TCB + SC từ arena, cấu hình, `Start`, rồi `Delete` + `Revoke`.
    /// Trả `true` ⇔ vòng đó hoàn tất **và** `total_used()` không đổi (không rò rỉ).
    fn round(&mut self, m: &mut Machine, root: ObjId, before: usize) -> bool {
        let tcb_cap = match m.retype(self.arena, ObjType::Tcb, 0, 0) {
            Ok(cap) => cap,
            Err(_) => return false,
        };
        if m.put_cap(root, LOOP_TCB_SLOT, tcb_cap).is_err() {
            let _ = m.drop_root_cap(tcb_cap);
            return false;
        }
        let tcb = SlotRef::new(root, LOOP_TCB_SLOT);
        let sc_cap = match m.retype(self.arena, ObjType::SchedContext, 0, 0) {
            Ok(cap) => cap,
            Err(_) => {
                let _ = m.delete(tcb);
                let _ = m.revoke(root, Some(LOOP_ARENA));
                return false;
            }
        };
        if m.put_cap(root, LOOP_SC_SLOT, sc_cap).is_err() {
            let _ = m.delete(tcb);
            let _ = m.drop_root_cap(sc_cap);
            let _ = m.revoke(root, Some(LOOP_ARENA));
            return false;
        }
        let sc = SlotRef::new(root, LOOP_SC_SLOT);
        let tcb_obj = m.cap_obj(tcb_cap).unwrap_or(0);
        let mut ok = m
            .sched_control_configure(self.ctl, sc, 1_000, 10_000)
            .is_ok()
            && m.set_sched_params(tcb, Some(sc)).is_ok()
            && m.tcb_configure(tcb, Some(self.cspace)).is_ok()
            && m.tcb_set_priority(tcb, 5, 10).is_ok();
        if ok {
            ok = m.start_tcb(tcb_obj).is_ok();
        }
        // Destroy: xoá hai cap rồi `Revoke` arena ⇒ `finalise_tcb` gỡ ready-queue/SC, watermark 0.
        let _ = m.delete(tcb);
        let _ = m.delete(sc);
        let _ = m.revoke(root, Some(LOOP_ARENA));
        ok && m.total_used() == before
    }

    /// Phục vụ request `Loop(n)` đang chờ (không chặn). Trả `true` ⇔ có request.
    pub fn service(&mut self, m: &mut Machine) -> bool {
        let mut did = false;
        while let Ok(Some(request)) = m.nb_recv(self.tcb_k, self.ep) {
            if u64::from(request.info.label()) != LOOP_REQ {
                continue;
            }
            let n = core::cmp::min(request.words().first().copied().unwrap_or(0), LOOP_MAX);
            let root = self.ep.cnode;
            let before = m.total_used();
            let mut ok = 0u64;
            let mut index = 0u64;
            while index < n {
                if !self.round(m, root, before) {
                    break;
                }
                ok += 1;
                index += 1;
            }
            let after = m.total_used();
            let _ = m.nb_send(
                self.tcb_k,
                self.reply,
                msg(LOOP_REPLY, &[ok, before as u64, after as u64, n]),
            );
            did = true;
        }
        did
    }
}
