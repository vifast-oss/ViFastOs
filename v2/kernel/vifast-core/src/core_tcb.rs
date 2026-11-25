// SPDX-License-Identifier: MIT OR Apache-2.0
//! Phần `impl Machine` của **KT-59**: TCB + transition + scheduler + ngân sách MCS + hộp thư.
//!
//! Tách khỏi `machine.rs` để giữ luật cấu trúc **R1** (mỗi tệp ≤ 1.500 dòng); toàn bộ trạng
//! thái vẫn nằm trong [`Machine`] (`machine.rs`), hai tệp chỉ là hai khối `impl Machine` của
//! **cùng một kiểu** — không có kiểu/trạng thái thứ hai. Port nguyên ngữ nghĩa sim (xem
//! `docs/k3b-core.md` §7); biên giới KT-59: chưa có IPC/Reply/Notification (KT-60), donation
//! KT-21 chỉ giữ trường + công thức.

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::ipc::Message;
use crate::machine::{
    FinaliseEntry, Machine, BODY_SC, BODY_TCB, MAX_FINALISE, MAX_SCHED_CONTEXTS, MAX_TCBS,
};
use crate::mcs::{donated_priority, ScInfo};
use crate::tcb::{
    FaultKind, SleepReason, SuspendFlag, SuspendMode, TcbInfo, TcbSnapshot, TcbState,
    TCB_STATE_COUNT,
};
use crate::tcbdata::{ScData, TcbData};
use crate::NonReadyTarget;

/// **KT-311** — căn lề của `Tcb.Configure` tham số `entry?` (byte). Biên **lệnh** RISC-V: **2 B**
/// khi có extension `C` (target `riscv64gc` của dòng OS — `e_entry` thật của ảnh test là
/// `0x4000_0112`, **không** căn 4 B), 4 B khi không có `C`. Chọn 2 B = biên **phần cứng** thật
/// của target, không phải con số tuỳ ý. Vi phạm ⇒ `Alignment` (6).
pub(crate) const TCB_ENTRY_ALIGN: u64 = 2;

/// **KT-320** — tham số `vspace?` của `Tcb.Configure`; **KT-325** thêm đường **xoá**.
///
/// Kênh invocation (`invoke::configure`) đọc **CPTR** ở word 1 của vùng 608..640:
/// [`VspaceUpdate::Set`] ⇔ CPTR ≠ 0 **và** là cap `VSpace` sống; [`VspaceUpdate::Clear`] ⇔ CPTR
/// **rỗng** (`0`) — "dùng VSpace của ảnh đã nạp" (`TcbData.vspace = 0`); [`VspaceUpdate::Keep`] ⇔
/// mask **không** bật bit 1 (giữ nguyên giá trị hiện có).
///
/// Vì sao enum chứ **không** phải `Option<SlotRef>`: `None` đã có nghĩa "giữ nguyên"; "xoá" là
/// trạng thái **thứ ba** — biểu diễn bằng hai trường (`Option` + `bool`) sẽ cho phép trạng thái
/// vô nghĩa (`Some` **và** xoá cùng lúc). Enum làm trạng thái đó **không biểu diễn được**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VspaceUpdate {
    /// Mask không bật `vspace?` ⇒ **giữ nguyên** `TcbData.vspace`.
    Keep,
    /// CPTR ≠ 0: cap `VSpace` trong CSpace **caller** (kiểm loại khi áp).
    Set(SlotRef),
    /// CPTR **rỗng** (`0`): xoá về "dùng VSpace của ảnh đã nạp".
    Clear,
}

impl Default for VspaceUpdate {
    fn default() -> Self {
        Self::Keep
    }
}

/// **KT-311** — tham số `Tcb.Configure` **đã kiểm** ở tầng invocation, chuyển cho
/// [`Machine::tcb_configure_invocation`] (một nguồn ngữ nghĩa cho invocation + rig).
///
/// `Some` = tham số **có mặt** (mask của kênh invocation bật bit tương ứng); `None` = **giữ
/// nguyên** giá trị hiện có của TCB. Khác `Machine::tcb_configure_with_ipc_buffer` (rig API):
/// ở đó `cspace = None` nghĩa là **xoá** CSpace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TcbConfigure {
    /// `cspace?` — cap `CNode` trong CSpace **caller**.
    pub cspace: Option<SlotRef>,
    /// `entry?` — VA điểm vào, phải căn [`TCB_ENTRY_ALIGN`].
    pub entry: Option<u64>,
    /// **KT-320** — `vspace?` — cap `VSpace` trong CSpace **caller** (D2-83: core chỉ giữ
    /// **danh tính** object; `satp`/PTE thuộc arch). **KT-325**: thêm `Clear` (CPTR rỗng).
    pub vspace: VspaceUpdate,
    /// `ipc_buffer?` — cap `Frame` 4 KiB + VA (căn `IPC_BUFFER_BYTES`).
    pub ipc_buffer: Option<(SlotRef, u64)>,
    /// `priority?` — phải ≤ `max_controlled_priority` (luật MCS).
    pub priority: Option<u8>,
}

impl TcbConfigure {
    /// Không tham số nào (tương đương `Tcb.Configure` rỗng — chỉ xoá `exit_code`).
    pub const EMPTY: Self = Self {
        cspace: None,
        entry: None,
        vspace: VspaceUpdate::Keep,
        ipc_buffer: None,
        priority: None,
    };
}

impl Machine {
    // ------------------------------------------------------- body TCB/SchedContext

    /// Body TCB của object — lỗi đúng như sim: object ngoài bảng/body đã chết ⇒
    /// `InvalidCapability`; object sống sai kiểu ⇒ `WrongObjectType`.
    pub(crate) fn tcb_data(&self, obj: ObjId) -> Result<&TcbData, KError> {
        let idx = self.object_index(obj)?;
        let m = &self.objects[idx];
        match m.body.kind {
            BODY_TCB => {
                if m.body.alive {
                    Ok(&self.tcbs[m.body.idx as usize])
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            BODY_SC => {
                if m.body.alive {
                    Err(KError::WrongObjectType)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    pub(crate) fn tcb_data_mut(&mut self, obj: ObjId) -> Result<&mut TcbData, KError> {
        let idx = self.object_index(obj)?;
        let m = &self.objects[idx];
        let body = m.body;
        match body.kind {
            BODY_TCB => {
                if body.alive {
                    Ok(&mut self.tcbs[body.idx as usize])
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            BODY_SC => {
                if body.alive {
                    Err(KError::WrongObjectType)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    /// Body SchedContext của object — cùng quy ước lỗi như [`Machine::tcb_data`].
    pub(crate) fn sc_data(&self, obj: ObjId) -> Result<&ScData, KError> {
        let idx = self.object_index(obj)?;
        let m = &self.objects[idx];
        match m.body.kind {
            BODY_SC => {
                if m.body.alive {
                    Ok(&self.scs[m.body.idx as usize])
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            BODY_TCB => {
                if m.body.alive {
                    Err(KError::WrongObjectType)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    pub(crate) fn sc_data_mut(&mut self, obj: ObjId) -> Result<&mut ScData, KError> {
        let idx = self.object_index(obj)?;
        let body = self.objects[idx].body;
        match body.kind {
            BODY_SC => {
                if body.alive {
                    Ok(&mut self.scs[body.idx as usize])
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            BODY_TCB => {
                if body.alive {
                    Err(KError::WrongObjectType)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
            _ => Err(KError::WrongObjectType),
        }
    }

    /// `require_type` của sim: cap phải sống và đúng **kiểu cap**, trả `ObjId`.
    pub(crate) fn require_type(&self, slot: SlotRef, ty: ObjType) -> Result<ObjId, KError> {
        let cap = self.cap_at(slot.cnode, slot.slot)?;
        let c = self.cap(cap)?;
        if c.ty != ty {
            return Err(KError::WrongObjectType);
        }
        Ok(c.obj)
    }

    // =========================================================================================
    // KT-59 — TCB / transition / scheduler / hộp thư (port từ sim `kobj.rs`)
    // =========================================================================================
}

mod budget;
mod configure;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{KError, ObjType, SlotRef};
    use crate::machine::{Machine, MAX_FINALISE};

    /// **F1**: log finalise có biên `MAX_OBJECTS` (256), đầy ⇒ `NotEnoughMemory` **atomic**
    /// (không đổi trạng thái TCB), không panic; 257 lần gọi liên tiếp đều trả lỗi và đếm
    /// `finalise_overflow`.
    #[test]
    fn finalise_log_day_tra_not_enough_memory_257_lan() {
        let mut m = Machine::new();
        let t = m.add_kernel_object(ObjType::Tcb);
        m.n_finalise = MAX_FINALISE;
        for i in 1..=257 {
            assert_eq!(
                m.finalise_tcb(t),
                Err(KError::NotEnoughMemory),
                "lần {i}: log đầy phải NotEnoughMemory"
            );
            assert_eq!(
                m.tcb_state(t),
                Ok(TcbState::Inactive),
                "lỗi atomic, state không đổi"
            );
        }
        assert_eq!(m.finalise_log().len(), MAX_FINALISE);
        assert_eq!(m.finalise_overflow(), 257);
    }

    /// Reviewer-kt59 tái hiện: **2 vòng × 64 TCB** (body tái dùng) phải ghi đủ 128 dòng
    /// finalise, không panic — chứng minh `MAX_FINALISE = MAX_TCBS` cũ là sai.
    #[test]
    fn finalise_hai_vong_64_tcb_du_128_dong() {
        let mut m = Machine::new();
        let c0 = m.add_cnode(128);
        let u = m.add_untyped(0x1000, 1 << 16);
        let cap = m.root_cap(u);
        m.put_cap(c0, 0, cap).expect("cap Untyped");
        for vong in 0..2 {
            for i in 0..64 {
                let cap = m
                    .retype(SlotRef::new(c0, 0), ObjType::Tcb, 0, 0)
                    .expect("retype TCB");
                m.put_cap(c0, i + 1, cap).expect("dat cap TCB");
            }
            let deleted = m.revoke(c0, Some(0)).expect("revoke 64 TCB");
            assert_eq!(deleted, 64, "vòng {vong}: phải revoke đủ 64 TCB");
        }
        assert_eq!(m.finalise_log().len(), 128);
        assert_eq!(m.finalise_overflow(), 0);
        assert!(m.finalise_log().iter().all(|e| e.state == TcbState::Dead));
    }

    /// **KT-65** — kênh fault mà procsrv dùng: `Exit{code}` được ghi vào `last_fault` của
    /// **người nhận** đang `Recv` trên `fault_ep`, TCB chết chuyển `Inactive`, và
    /// `take_last_fault` **lấy một lần** (lần hai `None`). Đây là hợp đồng core mà bridge
    /// `procsrv` (U-mode) dựa vào — không thêm syscall.
    #[test]
    fn kt65_exit_ghi_last_fault_cho_procsrv_roi_inactive() {
        let mut m = Machine::new();
        let c0 = m.add_cnode(32);
        let u = m.add_untyped(0x10000, 1 << 16);
        let root_u = m.root_cap(u);
        m.put_cap(c0, 0, root_u).expect("cap Untyped");
        for (slot, ty) in [
            (1, ObjType::Endpoint),
            (2, ObjType::Tcb),
            (3, ObjType::Tcb),
            (4, ObjType::CNode),
        ] {
            let base = if slot == 4 {
                m.root_cap(c0)
            } else {
                m.retype(SlotRef::new(c0, 0), ty, 0, 0).expect("retype")
            };
            m.put_cap(c0, slot, base).expect("dat cap");
        }
        let ps_slot = SlotRef::new(c0, 2);
        let child_slot = SlotRef::new(c0, 3);
        let ep_slot = SlotRef::new(c0, 1);
        let cspace = SlotRef::new(c0, 4);
        let ps = m.cap_obj_at(c0, 2).expect("obj procsrv");
        let child = m.cap_obj_at(c0, 3).expect("obj child");
        m.tcb_configure(ps_slot, Some(cspace)).expect("cspace ps");
        m.tcb_configure(child_slot, Some(cspace))
            .expect("cspace child");
        m.tcb_set_fault_ep(child_slot, ep_slot)
            .expect("fault_ep child");
        m.start_tcb(ps).expect("ps Ready");
        // procsrv `Recv` chặn trên EP_FAULT — đúng lượt nhận fault của QD §1 Q-A.
        m.recv(ps, ep_slot, None).expect("ps Recv");
        m.exit(child, 42).expect("child Exit");
        assert_eq!(
            m.tcb_state(child),
            Ok(TcbState::Inactive),
            "Exit ⇒ TCB chết phải Inactive (D2-25)"
        );
        assert_eq!(
            m.take_last_fault(ps),
            Ok(Some((child, FaultKind::Exit { code: 42 }))),
            "fault phải ghi vào người nhận"
        );
        assert_eq!(
            m.take_last_fault(ps),
            Ok(None),
            "take là lấy một lần (procsrv đọc đúng một lượt)"
        );
    }

    /// **KT-65** — `kill(pid)` của procsrv dùng `Tcb.Suspend(Abort)`: TCB đang `Running`
    /// chuyển `Ready` + cờ `suspended`, `Resume` xoá cờ. Không có syscall mới.
    #[test]
    fn kt65_kill_bang_suspend_abort_dat_co_treo() {
        let mut m = Machine::new();
        let c0 = m.add_cnode(16);
        let u = m.add_untyped(0x20000, 1 << 16);
        let root_u = m.root_cap(u);
        m.put_cap(c0, 0, root_u).expect("cap Untyped");
        let cap = m
            .retype(SlotRef::new(c0, 0), ObjType::Tcb, 0, 0)
            .expect("retype TCB");
        m.put_cap(c0, 1, cap).expect("dat cap TCB");
        let slot = SlotRef::new(c0, 1);
        let victim = m.cap_obj_at(c0, 1).expect("obj victim");
        m.start_tcb(victim).expect("Ready");
        m.transition_tcb(victim, NonReadyTarget::Running { hart: 0 })
            .expect("Running");
        m.tcb_suspend(slot, SuspendMode::Abort).expect("Suspend");
        assert_eq!(m.tcb_state(victim), Ok(TcbState::Ready));
        assert_eq!(
            m.tcb_data(victim).expect("tcb data").suspend,
            SuspendFlag::Abort,
            "Abort phải đặt cờ suspended"
        );
        m.tcb_resume(slot).expect("Resume");
        assert_eq!(
            m.tcb_data(victim).expect("tcb data").suspend,
            SuspendFlag::None
        );
    }
}
