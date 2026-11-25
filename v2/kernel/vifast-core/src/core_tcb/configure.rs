// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `core_tcb/configure.rs` — **cấu hình TCB** (KT-59): `Configure`/`ConfigureIpcBuffer`,
//! cSpace/entry/VSpace, fault EP, TLS, sched params, `Suspend`/`Resume`/`ReadState`, finalise TCB;
//! port nguyên ngữ nghĩa từ `vifast-sim` (oracle).

use super::*;
use crate::store::KernelStore;

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
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
        // **KT-329** — `Resume` là **đường khôi phục duy nhất** của `Suspend(Defer)`: xoá cờ
        // (nếu còn) trước khi chạy. Chỉ xoá `Defer`; cờ `Abort` của đường model KT-65 giữ
        // nguyên ngữ nghĩa KT-314 (invocation `Resume` **không** đụng cờ đó).
        if self.tcb_data(obj)?.suspend == SuspendFlag::Defer {
            self.set_tcb_suspend_flag(obj, SuspendFlag::None)?;
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

    /// **D2-242 nhóm 3 (3.1A)** — khôi phục `TcbData.vspace` về giá trị **trước** `Configure` khi
    /// hook arch trả [`crate::syscall::VspaceOutcome::Failed`] (vd bảng khe `VSpace` của arch đầy
    /// ⇒ `Busy` 14). Chỉ ghi **một** trường của `TcbData` — **không** thêm trường ⇒
    /// `size_of::<Machine>()` giữ nguyên; các tham số khác của `Configure` là `[TK]`/không có
    /// rollback riêng (giữ đúng hợp đồng cũ).
    pub fn tcb_restore_vspace(&mut self, tcb: ObjId, vspace: ObjId) -> Result<(), KError> {
        self.tcb_data_mut(tcb)?.vspace = vspace;
        Ok(())
    }

    /// **KT-319** — `Tcb.Suspend` qua **invocation** (method 3 của `[object].Tcb`): treo tác vụ
    /// đích bằng cách chuyển `Inactive`.
    ///
    /// # Ngữ nghĩa (thứ tự **bắt buộc** — lỗi không đổi gì)
    ///
    /// 1. Cap phải là `Tcb` sống ⇒ không: `WrongObjectType` 2 / `InvalidCapability` 1.
    /// 2. `mode = Abort` ⇒ chuyển `Inactive` **ngay** (KT-319, giữ nguyên từng byte ngữ nghĩa).
    ///    `mode = Defer` (**KT-329**) ⇒ **không** đổi trạng thái, chỉ đặt [`SuspendFlag::Defer`];
    ///    TCB dừng ở **điểm an toàn** kế tiếp ([`Machine::tcb_defer_safe_point`], gọi ở cuối
    ///    [`crate::syscall::dispatch`]) và khôi phục bằng `Tcb.Resume` (method 12 — spec **không**
    ///    có method `Restart`, xem `v2/docs/abi-v1-v2-mapping.md:73`).
    /// 3. **Trạng thái trước**: chỉ `Running`/`Ready` ⇒ đi; còn lại — `Inactive`, `BlockedOn*`,
    ///    `Sleeping`, `Stopping`, `Dead` — ⇒ `IllegalOperation` 17. Đường **invocation** hẹp hơn
    ///    model API KT-65 (model nhận cả `BlockedOn*`): treo một tác vụ đang chờ IPC là việc của
    ///    `Reply`/`Notification`/`Exit`, không phải `Suspend` (`[TK]` KT-319).
    /// 4. `Abort`: chuyển `Inactive` qua **một** cửa [`Machine::transition_tcb`] (bảng 11×11
    ///    D2-49(d) vẫn là nguồn sự thật: `Running → Inactive` và `Ready → Inactive` đều ALLOWED).
    /// 5. **Giữ** `TcbData.entry` (để [`Machine::tcb_resume_invocation`] sau đó chạy lại được —
    ///    mốc thật của KT-319) và **giữ** `exit_code` (D2-25: chỉ `Configure`/`Resume` xoá).
    pub fn tcb_suspend_invocation(
        &mut self,
        tcb: SlotRef,
        mode: SuspendMode,
    ) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        let state = self.tcb_state(obj)?;
        if !matches!(state, TcbState::Running { .. } | TcbState::Ready) {
            return Err(KError::IllegalOperation);
        }
        match mode {
            // KT-319 — **giữ nguyên** hành vi `Abort`: chuyển `Inactive` ngay, giữ `entry` +
            // `exit_code`. Cờ `Defer` còn treo (nếu có) bị **huỷ** vì TCB đã dừng thật.
            SuspendMode::Abort => {
                self.transition_tcb(obj, NonReadyTarget::Inactive)?;
                if self.tcb_data(obj)?.suspend == SuspendFlag::Defer {
                    self.set_tcb_suspend_flag(obj, SuspendFlag::None)?;
                }
                self.sched_after_op(true);
                Ok(())
            }
            // KT-329 — `Defer`: **không** đổi trạng thái; chỉ đặt cờ. TCB **vẫn** chạy được
            // syscall kế tiếp rồi mới dừng ở điểm an toàn.
            SuspendMode::Defer => {
                if self.tcb_data(obj)?.suspend == SuspendFlag::Abort {
                    // Đang bị đường model KT-65 treo ⇒ không thể "hoãn" thêm.
                    return Err(KError::IllegalOperation);
                }
                self.set_tcb_suspend_flag(obj, SuspendFlag::Defer)?;
                self.sched_after_op(true);
                Ok(())
            }
        }
    }

    /// **KT-329** — **điểm an toàn** của `Suspend(Defer)`: gọi ở **cuối**
    /// [`crate::syscall::dispatch`] khi syscall đã hoàn tất và TCB sắp trả về U-mode.
    ///
    /// Nếu TCB mang cờ [`SuspendFlag::Defer`] ⇒ chuyển `Inactive` **như `Abort`** (**giữ**
    /// `entry`, **giữ** `exit_code` — D2-25) rồi **xoá cờ**. Trả `true` ⇔ vừa treo.
    ///
    /// Chỉ nhận `Running`/`Ready`; trạng thái khác ⇒ **xoá cờ** (fail-closed, không để cờ mồ
    /// côi) và trả `false`. Đột biến "bỏ xoá cờ" ⇒ cờ còn trên TCB `Inactive`, lần chạy sau
    /// `Resume` lại bị treo tiếp — TAP 11 đỏ **có trần**, không treo máy.
    pub(crate) fn tcb_defer_safe_point(&mut self, tcb: ObjId) -> bool {
        if self.tcb_data(tcb).map(|d| d.suspend) != Ok(SuspendFlag::Defer) {
            return false;
        }
        if !matches!(
            self.tcb_state(tcb),
            Ok(TcbState::Running { .. }) | Ok(TcbState::Ready)
        ) {
            let _ = self.set_tcb_suspend_flag(tcb, SuspendFlag::None);
            return false;
        }
        if self.transition_tcb(tcb, NonReadyTarget::Inactive).is_err() {
            return false;
        }
        let _ = self.set_tcb_suspend_flag(tcb, SuspendFlag::None);
        self.sched_after_op(true);
        true
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
            suspended: t.suspend.is_suspended(),
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
        Ok(self.tcb_data(tcb)?.model.ipc.buffer)
    }

    /// 4 MR "thanh ghi" của message **vừa nhận** ở TCB (KT-60, `IpcArea`).
    pub fn tcb_regs(&self, tcb: ObjId) -> Result<[u64; 4], KError> {
        Ok(self.tcb_data(tcb)?.model.ipc.regs)
    }

    /// `MessageInfo` của message **vừa nhận** (`None` = chưa nhận).
    pub fn tcb_ipc_info(&self, tcb: ObjId) -> Result<Option<crate::ipc::MessageInfo>, KError> {
        Ok(self.tcb_data(tcb)?.model.ipc.info)
    }

    /// `msg_id` của message **vừa nhận** (`None` = chưa nhận).
    pub fn tcb_ipc_msg_id(&self, tcb: ObjId) -> Result<Option<u64>, KError> {
        Ok(self.tcb_data(tcb)?.model.ipc.msg_id)
    }

    /// Badge của message **vừa nhận**.
    pub fn tcb_ipc_badge(&self, tcb: ObjId) -> Result<u64, KError> {
        Ok(self.tcb_data(tcb)?.model.ipc.badge)
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
        Ok(self.tcb_data(tcb)?.model.ipc)
    }

    /// Số lần cặp `(from, to)` đã đi qua cổng.
    pub fn pair_hit_count(&self, from: usize, to: usize) -> u32 {
        self.obs.pair_hit_count(from, to)
    }

    /// `(số cặp ALLOWED đã có hit, 48)`.
    pub fn transition_coverage(&self) -> (usize, usize) {
        self.obs.coverage()
    }

    /// Số lần cặp **cấm** `(from, to)` bị thử.
    pub fn forbidden_attempt_count(&self, from: usize, to: usize) -> u32 {
        self.obs.forbidden_attempt_count(from, to)
    }

    /// `(số cặp thiết kế đã có hit, 46)` — bỏ 2 cặp `Stopping` reserved.
    pub fn design_coverage(&self) -> (usize, usize) {
        self.obs.design_coverage()
    }

    /// Log finalise TCB (theo thứ tự finalise).
    pub fn finalise_log(&self) -> &[FinaliseEntry] {
        self.obs.finalise_log()
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
        self.set_tcb_suspend_flag(obj, SuspendFlag::Abort)?;
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
        let dang_treo = self.tcb_data(obj)?.suspend.is_suspended();
        {
            let t = self.tcb_data_mut(obj)?;
            t.suspend = SuspendFlag::None;
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
            self.set_tcb_suspend_flag(obj, SuspendFlag::None)?;
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
        if self.obs.finalise_is_full() {
            self.finalise_overflow = self.finalise_overflow.saturating_add(1);
            return Err(KError::NotEnoughMemory);
        }
        // KT-21 DoD 5: huỷ donation trước khi finalise — không để SC trỏ tới TCB chết.
        self.release_donation_of(tcb)?;
        // **KT-342b/KT-401** — TCB **chết**: message `NBSend` đã trả `Ok` **ở lại** hàng (thuộc
        // về bên nhận); chỉ message **chặn** bị gỡ + đếm `ipc_dropped` (cùng luật với mọi đường
        // `unlink_tcb_queues`).
        self.unlink_tcb_queues(tcb);
        let released = {
            let t = self.tcb_data_mut(tcb)?;
            let released = t.sched_context.take();
            t.bound_notification = None;
            released
        };
        self.transition_tcb(tcb, NonReadyTarget::Dead)?;
        self.obs.finalise_push(FinaliseEntry {
            tcb,
            state: TcbState::Dead,
            released_sched_context: released,
        });
        self.sched_after_op(false);
        Ok(())
    }
}
