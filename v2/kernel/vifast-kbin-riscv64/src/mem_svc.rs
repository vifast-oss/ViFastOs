// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-345b** — `MemSvc`: dịch vụ bộ nhớ của **kernel** cho lệnh `free` trên ảnh sản phẩm
//! (`spec/protocols/meminfo.vidl`, D2-242 nhóm 4C).
//!
//! # Vì sao không thêm syscall
//!
//! `spec/abi.toml` đóng băng 16 syscall `0..15` và vòng này chỉ được sửa `spec/v1-parity.toml`
//! (D2-252), nên `free` đi **đúng đường** mà `net_svc`/`FsSvc` đã dùng: shell gửi `NBSend` trên
//! một `Endpoint`, kernel `nb_recv` request, đo rồi `nb_send` reply về `EP_REPLY` của shell. Đây
//! là dịch vụ **của kernel** (`MemSvc`), không phải chương trình U-mode thứ sáu.
//!
//! # Số liệu là số **thật** của object layer
//!
//! - `total` = tổng `Untyped::capacity` của **mọi Untyped còn sống** trong object layer của core
//!   (`.corespace`: nơi cấp TCB/SchedContext/Endpoint) — đúng lượng bộ nhớ kernel quản lý, và
//!   **cùng đại lượng** mà `loop` đo `before`/`after` (nên `free` trước/sau `loop` không đổi).
//! - `free` = `total − total_used()` với `total_used()` = tổng watermark của các Untyped (cùng
//!   đại lượng `untyped_used()` của sim, KT-16).
//!
//! Không `unsafe`, không cấp phát; mọi request đều có reply (không nhánh nào nuốt message).

use vifast_core::{Machine, Message, MessageInfo, ObjId, ObjType, SlotRef};

use crate::proc_abi::{EP_MEM, EP_REPLY, MEM_FREE, MEM_REPLY, MEM_REQ, MEM_TOTAL};

/// Dịch vụ bộ nhớ của kernel: endpoint request + endpoint reply của shell.
pub struct MemSvc {
    ep: SlotRef,
    reply: SlotRef,
    tcb_k: ObjId,
}

/// Kết quả dựng: `proc_rt` giữ dịch vụ trong `Rig`.
pub struct MemRig {
    pub svc: MemSvc,
}

fn msg(label: u64, words: &[u64]) -> Message {
    let info = MessageInfo::new(label as u32, 0, false, 0, words.len() as u8)
        .expect("KT-345b: MessageInfo mem");
    Message::new(info, words).expect("KT-345b: Message mem")
}

/// Dựng `Endpoint EP_MEM` (ô 21) và dịch vụ. `tcb_k` = TCB kernel nhận request / gửi reply,
/// `root` = CNode gốc của ảnh sản phẩm (kernel và shell dùng chung).
pub fn build(m: &mut Machine, root: ObjId, us: SlotRef, tcb_k: ObjId) -> MemRig {
    let cap = m
        .retype(us, ObjType::Endpoint, 0, 0)
        .expect("KT-345b: retype EP_MEM");
    m.put_cap(root, EP_MEM, cap).expect("KT-345b: put EP_MEM");
    MemRig {
        svc: MemSvc {
            ep: SlotRef::new(root, EP_MEM),
            reply: SlotRef::new(root, EP_REPLY),
            tcb_k,
        },
    }
}

impl MemSvc {
    /// `total` = tổng cỡ mọi Untyped còn sống của core; `free` = phần còn cấp được.
    /// `MEM_TOTAL` trả **cả hai** (`[total, free]`) ⇒ `free` chỉ cần **một** round trip.
    fn measure(m: &Machine, op: u64) -> [u64; 2] {
        let total: usize = (1..=m.object_count())
            .map(|id| m.untyped_size(id as ObjId))
            .sum();
        let free = total.saturating_sub(m.total_used());
        match op {
            MEM_FREE => [free as u64, 0],
            _ => [total as u64, free as u64],
        }
    }

    /// Phục vụ **mọi** request đang chờ (không chặn). Trả `true` ⇔ có ít nhất một request.
    pub fn service(&mut self, m: &mut Machine) -> bool {
        let mut did = false;
        while let Ok(Some(request)) = m.nb_recv(self.tcb_k, self.ep) {
            if u64::from(request.info.label()) != MEM_REQ {
                continue;
            }
            let op = request.words().first().copied().unwrap_or(MEM_TOTAL);
            let value = Self::measure(m, op);
            let _ = m.nb_send(
                self.tcb_k,
                self.reply,
                msg(MEM_REPLY, &[value[0], value[1], 0, 0]),
            );
            did = true;
        }
        did
    }
}
