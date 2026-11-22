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
    FaultKind, SleepReason, SuspendMode, TcbInfo, TcbSnapshot, TcbState, TCB_STATE_COUNT,
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

    // ------------------------------------------------------------- cấu hình TCB

    /// `Tcb.Configure`: gắn CSpace; **xoá `exit_code`** (D2-25: `Configure`/`Resume` xoá).
    ///
    /// **KT-282**: đây là dạng **arity 2** (chỉ CSpace) — giữ nguyên cho 20+ điểm gọi đã có;
    /// tham số `ipc_buffer?` của `spec/abi.toml` `[method_signature."Tcb.Configure"]` đi qua
    /// [`Machine::tcb_configure_with_ipc_buffer`] (cùng một `Tcb.Configure`, cùng ngữ nghĩa
    /// "xoá `exit_code`").
    pub fn tcb_configure(&mut self, tcb: SlotRef, cspace: Option<SlotRef>) -> Result<(), KError> {
        self.tcb_configure_with_ipc_buffer(tcb, cspace, None)
    }

    /// **KT-282** — `Tcb.Configure` **đầy đủ**: CSpace + `ipc_buffer?` (frame 4 KiB + VA).
    ///
    /// `ipc_buffer = Some((frame_cap, va))` bind **IPC buffer** của TCB: `frame_cap` phải là cap
    /// `Frame` sống (core kiểm loại — D2-83: core không sở hữu PTE/PA, chỉ giữ danh tính), `va`
    /// phải **căn `IPC_BUFFER_BYTES`** (4 KiB) để frame phủ đúng một trang và `va + 4096` không
    /// tràn `u64`. Vi phạm ⇒ `InvalidArgument` (5), **nguyên tử** (không đổi CSpace/`exit_code`).
    ///
    /// `ipc_buffer = None` ⇒ **bỏ bind** (TCB không có buffer ⇒ message `length > 4` bị
    /// `InvalidArgument` — fail-closed, không đoán địa chỉ).
    pub fn tcb_configure_with_ipc_buffer(
        &mut self,
        tcb: SlotRef,
        cspace: Option<SlotRef>,
        ipc_buffer: Option<(SlotRef, u64)>,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let cspace_obj = match cspace {
            Some(s) => Some(self.require_type(s, ObjType::CNode)?),
            None => None,
        };
        let binding = self.ipc_buffer_binding(ipc_buffer)?;
        {
            let t = self.tcb_data_mut(tcb_obj)?;
            t.cspace = cspace_obj;
            t.ipc_buffer = binding;
            t.exit_code = None;
        }
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-290** — `Tcb.Configure` qua **invocation** từ U-mode: bind **chỉ** `ipc_buffer?`,
    /// **giữ nguyên** CSpace (tham số `cspace?` của `[method_signature."Tcb.Configure"]` **chưa**
    /// cài ở kênh invocation — xem `invoke::configure`).
    ///
    /// Vì sao không dùng [`Machine::tcb_configure_with_ipc_buffer`] với `cspace = None`: API đó
    /// **xoá** CSpace (`t.cspace = None`) — đúng ngữ nghĩa "Configure không có cspace" của rig,
    /// nhưng invocation **không** mang tham số `cspace?` nên phải **giữ** CSpace hiện có, nếu
    /// không TCB mất CSpace và mọi syscall sau đó trả `InvalidCapability`.
    ///
    /// Ngữ nghĩa còn lại giống hệt `Tcb.Configure`: kiểm cap `Frame` sống + `va` căn
    /// `IPC_BUFFER_BYTES` (4 KiB) + không tràn `u64` (vi phạm ⇒ `InvalidArgument` 5, **nguyên
    /// tử**), **xoá `exit_code`** (D2-25), **không** đổi trạng thái TCB/scheduler.
    ///
    /// **KT-311**: đây là dạng `length = 2` của kênh invocation; dạng đầy đủ (`length = 3`,
    /// `cspace?`/`entry?`/`priority?`) đi qua [`Machine::tcb_configure_invocation`].
    pub fn tcb_configure_ipc_buffer(
        &mut self,
        tcb: SlotRef,
        ipc_buffer: Option<(SlotRef, u64)>,
    ) -> Result<(), KError> {
        self.tcb_configure_invocation(
            tcb,
            TcbConfigure {
                ipc_buffer,
                ..TcbConfigure::EMPTY
            },
        )
    }

    /// **KT-311** — `Tcb.Configure` **kênh đầy đủ** qua invocation: `cspace?` + `entry?` +
    /// `ipc_buffer?` + `priority?` (mỗi tham số `Some` = **có mặt**; `None` = **giữ nguyên**).
    ///
    /// # Ngữ nghĩa
    ///
    /// - `cspace`: cap `CNode` sống trong CSpace **caller** ⇒ gán `TcbData.cspace` (khác
    ///   [`Machine::tcb_configure_with_ipc_buffer`] với `cspace = None` — API đó **xoá** CSpace).
    /// - `vspace`: cap `VSpace` sống trong CSpace **caller** ⇒ gán `TcbData.vspace` (danh tính
    ///   object; D2-83: core **không** sở hữu PTE/PA). Sai loại ⇒ `WrongObjectType` (2).
    ///   **KT-325** — `VspaceUpdate::Clear` (CPTR rỗng) ⇒ `TcbData.vspace = 0` = "dùng VSpace
    ///   của ảnh đã nạp"; **không** tra ô 0 của CSpace, **không** đổi tham số nào khác.
    /// - `entry`: VA điểm vào, phải **căn [`TCB_ENTRY_ALIGN`]** (2 B — biên lệnh RISC-V có `C`)
    ///   ⇒ không: `Alignment` (6).
    /// - `ipc_buffer`: như [`Machine::tcb_configure_ipc_buffer`] (cap `Frame` sống + VA căn
    ///   `IPC_BUFFER_BYTES` + không tràn `u64`).
    /// - `priority`: phải ≤ `max_controlled_priority` (luật MCS của
    ///   [`Machine::tcb_set_priority`]) ⇒ không: `InvalidArgument` (5); cập nhật bucket scheduler
    ///   qua `tcb_set_priority` (giữ nguyên `mcp`).
    ///
    /// **Nguyên tử**: mọi tham số được kiểm **trước** khi ghi bất cứ trường nào — cùng luật
    /// `tcb_configure_with_ipc_buffer`. `Configure` **xoá `exit_code`** (D2-25) và **không** đổi
    /// trạng thái TCB.
    pub(crate) fn tcb_configure_invocation(
        &mut self,
        tcb: SlotRef,
        params: TcbConfigure,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        // ---- kiểm TẤT CẢ trước (nguyên tử) ----
        let cspace_obj = match params.cspace {
            Some(slot) => Some(self.require_type(slot, ObjType::CNode)?),
            None => None,
        };
        // **KT-320** — `vspace?` phải là cap `VSpace` **sống** (D2-83: core không sở hữu
        // PTE/PA, chỉ giữ danh tính object đại diện). Cap `Frame`/`PageTable`/loại khác ⇒
        // `WrongObjectType` (2); ô rỗng/chết ⇒ `InvalidCapability` (1) — `require_type` lo.
        //
        // **KT-325** — `VspaceUpdate::Clear` (CPTR rỗng) **không** cần cap nào ⇒ không gọi
        // `require_type`; nó chỉ ghi `0` ở bước áp (bên dưới), cùng một luật **nguyên tử**.
        let vspace_obj = match params.vspace {
            VspaceUpdate::Set(slot) => Some(self.require_type(slot, ObjType::VSpace)?),
            VspaceUpdate::Keep | VspaceUpdate::Clear => None,
        };
        let binding = self.ipc_buffer_binding(params.ipc_buffer)?;
        if let Some(entry) = params.entry {
            if entry % TCB_ENTRY_ALIGN != 0 {
                return Err(KError::Alignment);
            }
        }
        let mcp = self.tcb_data(tcb_obj)?.max_controlled_priority;
        if let Some(priority) = params.priority {
            if priority > mcp {
                return Err(KError::InvalidArgument);
            }
        }
        // ---- áp ----
        {
            let t = self.tcb_data_mut(tcb_obj)?;
            if params.cspace.is_some() {
                t.cspace = cspace_obj;
            }
            if let Some(entry) = params.entry {
                t.entry = entry;
            }
            if let Some(vspace) = vspace_obj {
                t.vspace = vspace;
            } else if params.vspace == VspaceUpdate::Clear {
                // **KT-325** — CPTR rỗng: về "dùng VSpace của ảnh đã nạp".
                t.vspace = 0;
            }
            if params.ipc_buffer.is_some() {
                t.ipc_buffer = binding;
            }
            t.exit_code = None;
        }
        if let Some(priority) = params.priority {
            self.tcb_set_priority(tcb, priority, mcp)?;
        }
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-314** — `Tcb.Resume` qua **invocation** (method 12 của `[object].Tcb`): đưa tác vụ
    /// đích lên `Running` trên `hart` và trả **điểm vào** đã cấu hình để arch nạp `sepc`.
    ///
    /// # Ngữ nghĩa (thứ tự **bắt buộc** — lỗi không đổi gì)
    ///
    /// 1. Cap phải là `Tcb` sống ⇒ không: `WrongObjectType` 2 / `InvalidCapability` 1.
    /// 2. **Trạng thái trước**: `Ready` ⇒ đi; `Inactive` **chưa từng chạy** (`exit_code = None`)
    ///    ⇒ `start_tcb` rồi đi; còn lại — `Running`, `BlockedOn*`, `Sleeping`, `Stopping`,
    ///    `Dead`, và `Inactive` **đã chạy** (`exit_code = Some`) — ⇒ `IllegalOperation` 7.
    ///    `Inactive` đã chạy bị từ chối vì **không tự chạy lại** một tác vụ đã kết thúc; muốn
    ///    chạy lại thì `Tcb.Configure` trước (D2-25: `Configure` xoá `exit_code`).
    /// 3. **`TcbData.entry`** (KT-311): `0` = chưa cấu hình ⇒ `InvalidArgument` 5 — fail-closed,
    ///    **không** đoán điểm vào. Kiểm **sau** trạng thái: TCB `Running` + `entry = 0` trả **7**
    ///    (không phải 5) — trạng thái là điều kiện mạnh hơn.
    /// 4. Chuyển `Running{hart}` qua **một** cửa [`Machine::transition_tcb`] (bảng 11×11 D2-49(d)
    ///    vẫn là nguồn sự thật; `Inactive → Running` không có trong bảng nên phải qua
    ///    `start_tcb`), xoá `exit_code` (D2-25).
    ///
    /// Trả `Ok(entry)` — **cùng** giá trị arch phải nạp vào `sepc` (D2-83: core không ghi CSR).
    pub fn tcb_resume_invocation(&mut self, tcb: SlotRef, hart: u16) -> Result<u64, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        let state = self.tcb_state(obj)?;
        let da_chay = self.tcb_data(obj)?.exit_code.is_some();
        if state == TcbState::Inactive && da_chay {
            return Err(KError::IllegalOperation);
        }
        if !matches!(state, TcbState::Ready | TcbState::Inactive) {
            return Err(KError::IllegalOperation);
        }
        let entry = self.tcb_data(obj)?.entry;
        if entry == 0 {
            return Err(KError::InvalidArgument);
        }
        if state == TcbState::Inactive {
            self.start_tcb(obj)?;
        }
        self.transition_tcb(obj, NonReadyTarget::Running { hart })?;
        self.tcb_data_mut(obj)?.exit_code = None;
        self.sched_after_op(true);
        Ok(entry)
    }

    /// **KT-311** — điểm vào đã cấu hình của TCB (`Tcb.Configure` tham số `entry?`); `0` = chưa
    /// đặt. Model-only `[TK]` — **KT-314** đã có đường tiêu thụ: `Tcb.Resume` đọc giá trị này
    /// ([`Machine::tcb_resume_invocation`]) rồi arch nạp vào `sepc`.
    pub fn tcb_entry(&self, tcb: ObjId) -> Result<u64, KError> {
        Ok(self.tcb_data(tcb)?.entry)
    }

    /// **KT-320** — **VSpace** đã cấu hình của TCB (`Tcb.Configure` tham số `vspace?`); `0` =
    /// **chưa** cấu hình (`ObjId` 1-based ⇒ `0` không phải object hợp lệ, cùng quy ước
    /// [`Machine::tcb_entry`]).
    ///
    /// Đây là **danh tính** object đại diện trong core (D2-83: `VSpace` thuộc object layer);
    /// arch đọc hàm này để **tiêu thụ** khi vào U-mode — hook `SyscallEnv::tcb_vspace` chỉ là
    /// đường **giao** giá trị (core **không** ghi `satp`).
    pub fn tcb_vspace(&self, tcb: ObjId) -> Result<ObjId, KError> {
        Ok(self.tcb_data(tcb)?.vspace)
    }

    /// **KT-319** — `Tcb.Suspend` qua **invocation** (method 3 của `[object].Tcb`): treo tác vụ
    /// đích bằng cách chuyển `Inactive`.
    ///
    /// # Ngữ nghĩa (thứ tự **bắt buộc** — lỗi không đổi gì)
    ///
    /// 1. Cap phải là `Tcb` sống ⇒ không: `WrongObjectType` 2 / `InvalidCapability` 1.
    /// 2. `mode = Defer` ⇒ `InvalidArgument` 5 (K1b chưa có `Restart` — cùng ngữ nghĩa model
    ///    [`Machine::tcb_suspend`], KT-65).
    /// 3. **Trạng thái trước**: chỉ `Running`/`Ready` ⇒ đi; còn lại — `Inactive`, `BlockedOn*`,
    ///    `Sleeping`, `Stopping`, `Dead` — ⇒ `IllegalOperation` 17. Đường **invocation** hẹp hơn
    ///    model API KT-65 (model nhận cả `BlockedOn*`): treo một tác vụ đang chờ IPC là việc của
    ///    `Reply`/`Notification`/`Exit`, không phải `Suspend` (`[TK]` KT-319).
    /// 4. Chuyển `Inactive` qua **một** cửa [`Machine::transition_tcb`] (bảng 11×11 D2-49(d) vẫn
    ///    là nguồn sự thật: `Running → Inactive` và `Ready → Inactive` đều ALLOWED).
    /// 5. **Giữ** `TcbData.entry` (để [`Machine::tcb_resume_invocation`] sau đó chạy lại được —
    ///    mốc thật của KT-319) và **giữ** `exit_code` (D2-25: chỉ `Configure`/`Resume` xoá).
    pub fn tcb_suspend_invocation(
        &mut self,
        tcb: SlotRef,
        mode: SuspendMode,
    ) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if mode == SuspendMode::Defer {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        if !matches!(state, TcbState::Running { .. } | TcbState::Ready) {
            return Err(KError::IllegalOperation);
        }
        self.transition_tcb(obj, NonReadyTarget::Inactive)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// Kiểm + dựng [`crate::ipc::IpcBufferBinding`] từ tham số `ipc_buffer?` — **một** nguồn cho
    /// cả [`Machine::tcb_configure_with_ipc_buffer`] (rig/ảnh test) và
    /// [`Machine::tcb_configure_ipc_buffer`] (invocation KT-290): cap phải là `Frame` sống,
    /// `va` căn 4 KiB và `va + 4096` không tràn `u64`.
    fn ipc_buffer_binding(
        &self,
        ipc_buffer: Option<(SlotRef, u64)>,
    ) -> Result<Option<crate::ipc::IpcBufferBinding>, KError> {
        match ipc_buffer {
            Some((frame, va)) => {
                let frame_obj = self.require_type(frame, ObjType::Frame)?;
                if va % crate::ipc::IPC_BUFFER_BYTES as u64 != 0 {
                    return Err(KError::InvalidArgument);
                }
                if va
                    .checked_add(crate::ipc::IPC_BUFFER_BYTES as u64)
                    .is_none()
                {
                    return Err(KError::InvalidArgument);
                }
                Ok(Some(crate::ipc::IpcBufferBinding {
                    frame: frame_obj,
                    va,
                }))
            }
            None => Ok(None),
        }
    }

    /// **KT-282** — IPC buffer đã bind của TCB (`None` = chưa bind). Arch đọc hàm này để biết
    /// **VA** cần đọc/ghi vùng MR4..MR63 (D2-83: core không chạm bộ nhớ user).
    pub fn tcb_ipc_buffer_binding(
        &self,
        tcb: ObjId,
    ) -> Result<Option<crate::ipc::IpcBufferBinding>, KError> {
        Ok(self.tcb_data(tcb)?.ipc_buffer)
    }

    /// **KT-309** — slot **đích** trong CSpace của TCB khi nó là **bên nhận** extra cap
    /// (`Recv` đọc word `dst` từ IPC buffer của chính nó rồi ghi vào đây). `None` = "slot trống
    /// đầu tiên". Model-only `[TK]`, không phải ABI.
    pub fn tcb_set_extra_caps_dst(&mut self, tcb: ObjId, dst: Option<u16>) -> Result<(), KError> {
        self.tcb_data_mut(tcb)?.extra_caps_dst = dst;
        Ok(())
    }

    /// **KT-309** — slot đích đã khai cho extra cap (`None` = chưa khai ⇒ slot trống đầu tiên).
    pub fn tcb_extra_caps_dst(&self, tcb: ObjId) -> Result<Option<u16>, KError> {
        Ok(self.tcb_data(tcb)?.extra_caps_dst)
    }

    /// `Tcb.SetFaultEp`: cap Endpoint; ghi `fault_ep`.
    pub fn tcb_set_fault_ep(&mut self, tcb: SlotRef, ep: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ep_cap = self.cap_at(ep.cnode, ep.slot)?;
        if self.cap(ep_cap)?.ty != ObjType::Endpoint {
            return Err(KError::WrongObjectType);
        }
        self.tcb_data_mut(tcb_obj)?.fault_ep = Some(ep_cap);
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-74** — `Tcb.SetTls` (method **15** của `[object].Tcb`, hằng
    /// `vifast_abi::TCB_SETTLS`).
    ///
    /// Ghi **con trỏ TLS** của TCB. Kernel **không** đọc/ghi vùng TLS — nó chỉ giữ giá trị và
    /// nạp vào `x4`/`tp` của U-mode ở mỗi lần resume (`vifast-plat-riscv64::user`). Ghi được ở
    /// **mọi** trạng thái sống: giá trị có hiệu lực từ lần vào U-mode **kế tiếp** (giống
    /// `Tcb.Configure` chỉ lưu cấu hình), nên chuỗi `exec → SetTls → resume` không cần TCB
    /// `Inactive`.
    ///
    /// Kênh tham số ở tầng invocation: `MR0 = tls`, `length = 1` (xem `invoke::dispatch`).
    /// `spec/abi.toml` **chưa** có khối `[method_signature."Tcb.SetTls"]` (chỉ `[object].Tcb` có
    /// số 15) ⇒ arity 1 là **đề xuất** của KT-74, đã ghi ở `docs/k6-exec.md` §3 để Agent 0 bổ
    /// sung chữ ký; **không** tự đặt số method (số vẫn lấy từ spec).
    pub fn tcb_set_tls(&mut self, tcb: SlotRef, tls: u64) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        // Chỉ ghi dữ liệu: **không** đổi trạng thái/scheduler ⇒ không gọi `sched_after_op`
        // (hàm đó chỉ có tác dụng khi `preempt = true`).
        self.tcb_data_mut(tcb_obj)?.tls = tls;
        Ok(())
    }

    /// **KT-74** — con trỏ TLS hiện tại của TCB (theo `ObjId`). Arch đọc hàm này khi dựng
    /// trapframe U-mode (`tf.x[4] = tp`) — **một** nguồn cho `SetTls` và resume.
    pub fn tcb_tls(&self, tcb: ObjId) -> Result<u64, KError> {
        Ok(self.tcb_data(tcb)?.tls)
    }

    /// `SchedControl.Configure(sc, budget, period)` — `budget > 0`, `period > 0`,
    /// `budget ≤ period`; cấu hình lại giữa chu kỳ đặt `consumed = 0` (model-only `[TK]`).
    pub fn sched_control_configure(
        &mut self,
        sched_control: SlotRef,
        sched: SlotRef,
        budget: u64,
        period: u64,
    ) -> Result<(), KError> {
        self.require_type(sched_control, ObjType::SchedControl)?;
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        if budget == 0 || period == 0 || budget > period {
            return Err(KError::InvalidArgument);
        }
        let now = self.now;
        self.sc_data_mut(sched_obj)?
            .budget
            .configure(now, budget, period);
        self.sched_after_op(false);
        Ok(())
    }

    /// `Tcb.SetPriority` `[TK]`: `max_controlled_priority ≥ priority` (thiết kế §4.5 `:311`).
    pub fn tcb_set_priority(
        &mut self,
        tcb: SlotRef,
        priority: u8,
        max_controlled_priority: u8,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        if max_controlled_priority < priority {
            return Err(KError::InvalidArgument);
        }
        let info = self.tcb_data(tcb_obj)?.snapshot();
        // KT-21: priority của **donor** đang được donate không đổi được (sẽ đổi priority hiệu
        // lực của donee giữa chừng) — phải `Reply`/huỷ donation trước (như sim).
        if self.is_donation_donor(tcb_obj) {
            return Err(KError::IllegalOperation);
        }
        if info.state == TcbState::Ready || info.donated_sc.is_some() {
            self.sched_remove_all(tcb_obj);
        }
        {
            let t = self.tcb_data_mut(tcb_obj)?;
            t.priority = priority;
            t.max_controlled_priority = max_controlled_priority;
        }
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.SetAffinity` `[TK]`: mask ≤ 64 bit, khác 0, chỉ hart có thật, chứa `home`.
    pub fn tcb_set_affinity(&mut self, tcb: SlotRef, affinity: u64) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let hart_mask = if self.n_harts >= 64 {
            u64::MAX
        } else {
            (1u64 << self.n_harts) - 1
        };
        if affinity == 0 || affinity & !hart_mask != 0 {
            return Err(KError::InvalidArgument);
        }
        let home = self.tcb_data(tcb_obj)?.home;
        if affinity & (1u64 << home) == 0 {
            return Err(KError::InvalidArgument);
        }
        if self.tcb_state(tcb_obj)? == TcbState::Ready {
            self.sched_remove_all(tcb_obj);
        }
        self.tcb_data_mut(tcb_obj)?.affinity = affinity;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.SetHome` `[TK]`: home phải có thật và nằm trong affinity; **KT-29**: không đổi
    /// `home` của TCB đang `Running` khi `n > 1` (`Running{hart} ⇒ home == hart`).
    pub fn tcb_set_home(&mut self, tcb: SlotRef, home: u16) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        if home as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let info = self.tcb_data(tcb_obj)?.snapshot();
        if self.n_harts > 1 && matches!(info.state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        if info.affinity & (1u64 << home) == 0 {
            return Err(KError::InvalidArgument);
        }
        if info.state == TcbState::Ready {
            self.sched_remove_all(tcb_obj);
        }
        self.tcb_data_mut(tcb_obj)?.home = home;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// Model API `set_sched_params(tcb, sc)` — **D2-49(b)**: gắn/gỡ `SchedContext` của TCB.
    ///
    /// KT-21: nhánh "TCB đang nhận/cho donation" của sim chưa kích hoạt ở KT-59 (chưa có
    /// `Call`/`Reply`); giữ đúng mã lỗi `Busy` khi SC đang bị donate cho TCB khác.
    pub fn set_sched_params(&mut self, tcb: SlotRef, sched: Option<SlotRef>) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let sched_obj = match sched {
            Some(s) => Some(self.require_type(s, ObjType::SchedContext)?),
            None => None,
        };
        let info = self.tcb_data(tcb_obj)?.snapshot();
        if info.donated_sc.is_some() || self.is_donation_donor(tcb_obj) {
            return Err(KError::IllegalOperation);
        }
        if let Some(sc) = sched_obj {
            if self.sc_data(sc)?.donated_to.is_some() {
                return Err(KError::Busy);
            }
        }
        self.tcb_data_mut(tcb_obj)?.sched_context = sched_obj;
        self.enforce_budget(tcb_obj)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// Ảnh chụp toàn bộ trường TCB.
    pub fn tcb_info(&self, tcb: SlotRef) -> Result<TcbInfo, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        Ok(self.tcb_data(obj)?.snapshot())
    }

    /// Như [`Machine::tcb_info`] nhưng nhận thẳng `ObjId` (scheduler/nội bộ).
    pub fn tcb_info_obj(&self, tcb: ObjId) -> Result<TcbInfo, KError> {
        Ok(self.tcb_data(tcb)?.snapshot())
    }

    /// `Tcb.ReadState` — trả **4 giá trị** `{state, suspended, blocked_on_kind, exit_code}`.
    pub fn tcb_read_state(&self, tcb: SlotRef) -> Result<TcbSnapshot, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        let t = self.tcb_data(obj)?;
        Ok(TcbSnapshot {
            state: t.state,
            suspended: t.suspended,
            blocked_on_kind: t.state.blocked_on_kind(),
            exit_code: t.exit_code,
        })
    }

    /// Trạng thái TCB theo `ObjId`.
    pub fn tcb_state(&self, tcb: ObjId) -> Result<TcbState, KError> {
        Ok(self.tcb_data(tcb)?.state)
    }

    /// 60 word "IPC buffer model" của message **vừa nhận** ở TCB (KT-60, `IpcArea`).
    pub fn tcb_ipc_buffer(&self, tcb: ObjId) -> Result<[u64; 60], KError> {
        Ok(self.tcb_data(tcb)?.ipc.buffer)
    }

    /// 4 MR "thanh ghi" của message **vừa nhận** ở TCB (KT-60, `IpcArea`).
    pub fn tcb_regs(&self, tcb: ObjId) -> Result<[u64; 4], KError> {
        Ok(self.tcb_data(tcb)?.ipc.regs)
    }

    /// `MessageInfo` của message **vừa nhận** (`None` = chưa nhận).
    pub fn tcb_ipc_info(&self, tcb: ObjId) -> Result<Option<crate::ipc::MessageInfo>, KError> {
        Ok(self.tcb_data(tcb)?.ipc.info)
    }

    /// `msg_id` của message **vừa nhận** (`None` = chưa nhận).
    pub fn tcb_ipc_msg_id(&self, tcb: ObjId) -> Result<Option<u64>, KError> {
        Ok(self.tcb_data(tcb)?.ipc.msg_id)
    }

    /// Badge của message **vừa nhận**.
    pub fn tcb_ipc_badge(&self, tcb: ObjId) -> Result<u64, KError> {
        Ok(self.tcb_data(tcb)?.ipc.badge)
    }

    /// Lỗi IPC kernel gán ở lần giao gần nhất (`Cancelled` — D2-53 Q7(b)).
    pub fn tcb_last_ipc_error(&self, tcb: ObjId) -> Result<Option<KError>, KError> {
        Ok(self.tcb_data(tcb)?.last_ipc_error)
    }

    /// `(sender, FaultKind)` của fault **vừa giao** cho TCB làm bên nhận.
    pub fn tcb_last_fault(&self, tcb: ObjId) -> Result<Option<(ObjId, FaultKind)>, KError> {
        Ok(self.tcb_data(tcb)?.last_fault)
    }

    /// Như [`Machine::tcb_last_fault`] nhưng **tiêu thụ** bản ghi.
    pub fn take_last_fault(&mut self, tcb: ObjId) -> Result<Option<(ObjId, FaultKind)>, KError> {
        Ok(self.tcb_data_mut(tcb)?.last_fault.take())
    }

    /// Ảnh chụp vùng nhận IPC (info/regs/buffer/badge/msg_id) — model API `[TK]`.
    pub fn tcb_ipc_area(&self, tcb: ObjId) -> Result<crate::ipc::IpcArea, KError> {
        Ok(self.tcb_data(tcb)?.ipc)
    }

    /// Số lần cặp `(from, to)` đã đi qua cổng.
    pub fn pair_hit_count(&self, from: usize, to: usize) -> u32 {
        self.pair_hits[from * TCB_STATE_COUNT + to]
    }

    /// `(số cặp ALLOWED đã có hit, 48)`.
    pub fn transition_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage(&self.pair_hits)
    }

    /// Số lần cặp **cấm** `(from, to)` bị thử.
    pub fn forbidden_attempt_count(&self, from: usize, to: usize) -> u32 {
        self.forbidden_hits[from * TCB_STATE_COUNT + to]
    }

    /// `(số cặp thiết kế đã có hit, 46)` — bỏ 2 cặp `Stopping` reserved.
    pub fn design_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage_design(&self.pair_hits)
    }

    /// Log finalise TCB (theo thứ tự finalise).
    pub fn finalise_log(&self) -> &[FinaliseEntry] {
        &self.finalise_log[..self.n_finalise]
    }

    /// Số lần finalise bị từ chối vì log đầy (`MAX_FINALISE`) — chẩn đoán, không phải ABI.
    pub fn finalise_overflow(&self) -> u32 {
        self.finalise_overflow
    }

    /// `Tcb.Suspend(mode)` (thiết kế §4.4 `:278-283`).
    ///
    /// - `Abort`: gỡ khỏi hàng, `BlockedOn*`/`Running` → `Ready`, `Ready` giữ `Ready`,
    ///   `Sleeping` giữ `Sleeping`; `exit_code` giữ nguyên (D2-25).
    /// - `Defer`: K1b chưa có `Restart` ⇒ **luôn** `InvalidArgument`, trạng thái không đổi.
    /// - `Inactive`/`Stopping`/`Dead`: `IllegalOperation`.
    pub fn tcb_suspend(&mut self, tcb: SlotRef, mode: SuspendMode) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if mode == SuspendMode::Defer {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        if !state.is_live() || state == TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        // KT-21 DoD 5 (hook F1 reviewer-kt60): huỷ donation **trước** khi gỡ hàng — donor thì
        // thu SC (donee Running mất ngân sách ⇒ Sleeping{Budget}); donee thì trả SC + đánh
        // thức client (`cancel_reply_of`) như sim, không để T0 treo trên Reply không ai trả.
        self.release_donation_of(obj)?;
        self.unlink_tcb_queues(obj);
        if state.is_blocked() || matches!(state, TcbState::Running { .. }) {
            self.suspend_to_ready(obj)?;
        }
        self.set_tcb_suspended(obj, true)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.Resume`: chỉ xoá cờ `suspended` + `exit_code` (D2-25); **không** phát lại
    /// Call/Send/Recv/ReplyRecv. Idempotent khi TCB không treo.
    pub fn tcb_resume(&mut self, tcb: SlotRef) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if self.tcb_state(obj)? == TcbState::Dead {
            return Err(KError::IllegalOperation);
        }
        let dang_treo = self.tcb_data(obj)?.suspended;
        {
            let t = self.tcb_data_mut(obj)?;
            t.suspended = false;
            t.exit_code = None;
        }
        if dang_treo {
            self.sched_sync(obj)?;
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-64** — `tcb_resume_with_regs(tcb, msg)`: resume một TCB **sau fault** với thanh
    /// ghi/MR mới, đúng một cửa của sim (KT-26, thiết kế §4.4 `:279`).
    ///
    /// Thứ tự **bắt buộc**: kiểm trạng thái đích **trước** → gỡ khỏi mọi hàng chờ/`Reply.caller`
    /// → ghi `regs` qua **cùng** một cửa `ipc::transfer` (`admit` + `deliver`) **trước** khi đổi
    /// trạng thái (không có cửa sổ “Ready nhưng message chưa tới”, r319) → bỏ `suspended` →
    /// về trạng thái đích qua **một** cửa `transition_tcb`.
    ///
    /// Nhận **đúng bốn** trạng thái (sim KT-26):
    /// - `BlockedOnReply { reply }` — chỉ khi `Reply.caller == tcb` (fault đã giao, procsrv giữ
    ///   Reply); Reply của người khác ⇒ `IllegalOperation`;
    /// - `BlockedOnSend { call: true, fault: Some(k) }` với `k` **không** phải `Exit` (huỷ message
    ///   fault đang chờ — message fault không có `msg_id` nên không tăng `ipc_dropped`);
    /// - `Ready` (bỏ `suspended`); `Sleeping { .. }` (**giữ nguyên** reason).
    ///
    /// Trạng thái khác (`Inactive`/`Running`/`Dead`/`BlockedOnRecv`/…) ⇒ `IllegalOperation` và
    /// **không** đổi gì: đây là đường **model API** cho fault-resume, **không** phải syscall —
    /// user **không** có cửa nào để tự resume tuỳ ý (TAP `# resume:` + test pin).
    ///
    /// Không phát lại Call/Send/Recv/Reply cũ; **giữ** `exit_code` (khác `Tcb.Resume`).
    pub fn tcb_resume_with_regs(&mut self, tcb: SlotRef, msg: Message) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if msg.n_caps != 0 || msg.n_snaps != 0 || msg.info.caps() != 0 {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        let suspended = self.tcb_info_obj(obj)?.suspended;
        // Trạng thái đích — kiểm **trước** khi gỡ hàng/giao thanh ghi (lỗi không đổi gì).
        match state {
            TcbState::BlockedOnReply { reply } => {
                if self.reply_caller(reply)? != Some(obj) {
                    return Err(KError::IllegalOperation);
                }
            }
            TcbState::BlockedOnSend {
                call: true,
                fault: Some(kind),
                ..
            } if !kind.is_exit() => {}
            TcbState::Ready => {}
            TcbState::Sleeping { .. } => {}
            _ => return Err(KError::IllegalOperation),
        }
        // Gỡ khỏi **mọi** hàng chờ/`Reply.caller` trước khi giao (bất biến §4.4); với
        // `BlockedOnSend` đây là chỗ huỷ message fault đang chờ.
        self.unlink_tcb_queues(obj);
        // Ghi thanh ghi **trước** khi `Ready` — qua đúng một hàm `ipc::transfer`.
        let mut m = msg;
        self.resolve_caps(&mut m)?;
        let mut m = self.admit_message(m)?;
        self.deliver_message(obj, &mut m, None)?;
        if suspended {
            self.set_tcb_suspended(obj, false)?;
        }
        match state {
            // `Sleeping` giữ nguyên (bảng §4.5); `Ready` đã đúng chỗ (cặp danh tính cấm).
            TcbState::Sleeping { .. } | TcbState::Ready => {}
            _ => self.wake_from(obj)?,
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// Finalise TCB (thiết kế §4.3 bảng Finalise; KT-18 DoD 9): gỡ khỏi hàng, chuyển `Dead`
    /// qua một cửa, trả `sched_context` bằng hook, ghi log (biên [`MAX_FINALISE`]).
    pub fn finalise_tcb(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Dead {
            return Ok(());
        }
        // **F1 (reviewer-kt59)**: log có biên `MAX_OBJECTS`; đầy ⇒ `NotEnoughMemory` **trước**
        // mọi thay đổi (atomic) — không `debug_assert`, không cắt im lặng như bản trước.
        if self.n_finalise >= MAX_FINALISE {
            self.finalise_overflow = self.finalise_overflow.saturating_add(1);
            return Err(KError::NotEnoughMemory);
        }
        // KT-21 DoD 5: huỷ donation trước khi finalise — không để SC trỏ tới TCB chết.
        self.release_donation_of(tcb)?;
        self.unlink_tcb_queues(tcb);
        let released = {
            let t = self.tcb_data_mut(tcb)?;
            let released = t.sched_context.take();
            t.bound_notification = None;
            released
        };
        self.transition_tcb(tcb, NonReadyTarget::Dead)?;
        self.finalise_log[self.n_finalise] = FinaliseEntry {
            tcb,
            state: TcbState::Dead,
            released_sched_context: released,
        };
        self.n_finalise += 1;
        self.sched_after_op(false);
        Ok(())
    }

    // ------------------------------------------------------------- ngân sách MCS (KT-20)

    /// Thời gian ảo hiện tại (tick) — **không** đọc wall clock (D2-49(h)).
    pub fn virtual_now(&self) -> u64 {
        self.now
    }

    /// `tick(n)` — chạy đồng hồ ảo `n` đơn vị; tất định. Mỗi đơn vị: charge TCB đang `Running`
    /// cho `[now, now+1)`, `now += 1`, rồi xử lý biên refill (charge trước — refill sau).
    /// **Không** tự `dispatch`.
    pub fn tick(&mut self, n: u64) -> Result<(), KError> {
        for _ in 0..n {
            self.tick_one()?;
        }
        self.sched_after_op(false);
        Ok(())
    }

    fn tick_one(&mut self) -> Result<(), KError> {
        for h in 0..self.n_harts {
            if let Some(cur) = self.sched[h].current() {
                self.charge_tick(cur)?;
            }
        }
        self.now = self.now.saturating_add(1);
        self.process_refills()
    }

    /// Charge **một** tick cho TCB đang chạy; hết budget ⇒ `Running → Sleeping{Budget}`.
    fn charge_tick(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        if !matches!(info.state, TcbState::Running { .. }) {
            return Ok(());
        }
        let Some(sc) = self.funding_sc(tcb) else {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        };
        if !self.sc_data(sc)?.budget.is_configured() {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        }
        let charged = self.sc_data_mut(sc)?.budget.charge();
        if !charged || !self.tcb_has_budget(tcb) {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }

    /// Xử lý biên nạp lại tại `now` cho mọi `SchedContext` sống; `Sleeping{Budget}` +
    /// `!suspended` được đánh thức `Ready`.
    fn process_refills(&mut self) -> Result<(), KError> {
        let now = self.now;
        let mut refilled = [0u32; MAX_SCHED_CONTEXTS];
        let mut n_refilled = 0usize;
        for i in 0..self.n_objects {
            let obj = (i as u32) + 1;
            if !self.objects[i].alive || self.objects[i].ty != ObjType::SchedContext {
                continue;
            }
            let body = self.objects[i].body;
            if body.kind != BODY_SC || !body.alive {
                continue;
            }
            if self.scs[body.idx as usize].budget.refill_due(now) {
                self.budget_refills += 1;
                if n_refilled < MAX_SCHED_CONTEXTS {
                    refilled[n_refilled] = obj;
                    n_refilled += 1;
                }
            }
        }
        if n_refilled == 0 {
            return Ok(());
        }
        let mut wake = [0u32; MAX_TCBS];
        let mut n_wake = 0usize;
        for i in 0..self.n_objects {
            let obj = (i as u32) + 1;
            if !self.objects[i].alive || self.objects[i].ty != ObjType::Tcb {
                continue;
            }
            let info = self.tcb_data(obj)?.snapshot();
            if matches!(
                info.state,
                TcbState::Sleeping {
                    reason: SleepReason::Budget
                }
            ) && !info.suspended
            {
                if let Some(sc) = self.funding_sc(obj) {
                    if refilled[..n_refilled].contains(&sc) && n_wake < MAX_TCBS {
                        wake[n_wake] = obj;
                        n_wake += 1;
                    }
                }
            }
        }
        for &tcb in &wake[..n_wake] {
            self.wake_from(tcb)?;
        }
        Ok(())
    }

    /// Gắn/gỡ SC khiến TCB đang `Running` **không còn budget** ⇒ ngủ ngay `Sleeping{Budget}`.
    fn enforce_budget(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        if matches!(info.state, TcbState::Running { .. }) && !self.tcb_has_budget(tcb) {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }

    /// Ảnh chụp ngân sách của `SchedContext`.
    pub fn sched_info(&self, sched: SlotRef) -> Result<ScInfo, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        let s = self.sc_data(obj)?;
        let mut info = s.budget.info();
        info.donated_to = s.donated_to;
        info.donor = s.donor;
        Ok(info)
    }

    /// `SchedContext.Consumed` `[TK]`: số tick đã charge trong chu kỳ hiện tại.
    pub fn sched_consumed(&self, sched: SlotRef) -> Result<u64, KError> {
        Ok(self.sched_info(sched)?.consumed)
    }

    /// Budget còn lại của chu kỳ hiện tại; `0` khi SC chưa cấu hình.
    pub fn sched_remaining(&self, sched: SlotRef) -> Result<u64, KError> {
        let info = self.sched_info(sched)?;
        Ok(if info.configured {
            info.budget - info.consumed
        } else {
            0
        })
    }

    /// SC đang **cấp ngân sách** cho `tcb`: SC được donate (KT-21, chưa có ở KT-59) hoặc SC
    /// riêng không bị donate; `None` ⇒ không có budget ⇒ không được `pick` (D2-50).
    fn funding_sc(&self, tcb: ObjId) -> Option<ObjId> {
        let info = self.tcb_data(tcb).ok()?.snapshot();
        if let Some(sc) = info.donated_sc {
            return Some(sc);
        }
        let own = info.sched_context?;
        match self.sc_data(own) {
            Ok(s) if s.donated_to.is_none() => Some(own),
            _ => None,
        }
    }

    /// Model API `[TK]`: SC đang cấp ngân sách cho `tcb`.
    pub fn sc_of(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.funding_sc(tcb))
    }

    /// Ngân sách **còn lại** của SC đang cấp cho `tcb` (0 khi không có SC/chưa cấu hình).
    ///
    /// KT-62: `Yield` (8) trả phần budget còn lại cho user (`spec/abi.toml` `[syscall]`) mà
    /// không cần slot cap của SC; cùng nguồn với [`Machine::sched_remaining`].
    pub fn tcb_remaining(&self, tcb: ObjId) -> u64 {
        self.funding_sc(tcb)
            .and_then(|sc| self.sc_data(sc).ok())
            .map(|s| s.budget.remaining())
            .unwrap_or(0)
    }

    /// TCB có được phép chạy theo ngân sách không (SC đã cấu hình và `consumed < budget`).
    pub fn tcb_has_budget(&self, tcb: ObjId) -> bool {
        let Some(sc) = self.funding_sc(tcb) else {
            return false;
        };
        self.sc_data(sc)
            .map(|s| s.budget.has_budget())
            .unwrap_or(false)
    }

    /// Priority **hiệu lực** của `tcb` (KT-21 DoD 6): khi nhận donation =
    /// [`donated_priority`]; ngược lại priority gốc. KT-59 chưa tạo donation nên luôn là
    /// priority gốc; công thức + trường đã sẵn cho KT-60.
    pub fn effective_priority(&self, tcb: ObjId) -> Result<u8, KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        let Some(sc) = info.donated_sc else {
            return Ok(info.priority);
        };
        let donor = self.sc_data(sc)?.donor;
        let Some(donor) = donor else {
            return Ok(info.priority);
        };
        let oi = self.tcb_data(donor)?.snapshot();
        Ok(donated_priority(
            oi.priority,
            oi.max_controlled_priority,
            info.max_controlled_priority,
        ))
    }
}

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
        assert!(
            m.tcb_data(victim).expect("tcb data").suspended,
            "Abort phải đặt cờ suspended"
        );
        m.tcb_resume(slot).expect("Resume");
        assert!(!m.tcb_data(victim).expect("tcb data").suspended);
    }
}
