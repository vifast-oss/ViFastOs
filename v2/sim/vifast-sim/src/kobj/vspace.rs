// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `vspace`: VSpace/ASID: activate, flush, view, check_vm.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------------------- VSpace (KT-38)
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-65 Q4/Q9; `v2/docs/k2-vm.md` §12). VSpace
    // là **object `reserved` id 9**: metadata (asid/thế hệ/`active_on`) + **một** root PageTable
    // thật của mô hình (object nội bộ, không cap). Không có TLB/shootdown thật — KT-38 chỉ ghi
    // **lệnh flush** cho KT-40; `page_table_walk(root)` là **oracle** cho `vspace_walk`.

    /// Chỉ số `Machine::vspaces` của object VSpace **sống**; object lạ/chết/không phải VSpace ⇒
    /// lỗi tương ứng (một cửa để mọi hàm VSpace không lệch cách lấy).
    pub(super) fn vspace_slot_of(&self, vs: ObjId) -> Result<usize, KError> {
        let idx = self.object_index(vs)?;
        let m = &self.objects[idx];
        if m.ty != ObjType::VSpace {
            return Err(KError::WrongObjectType);
        }
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        m.space.ok_or(KError::WrongObjectType)
    }

    pub(super) fn vspace_data(&self, vs: ObjId) -> Result<&VSpaceData, KError> {
        let slot = self.vspace_slot_of(vs)?;
        self.vspaces
            .get(slot)
            .and_then(|d| d.as_ref())
            .ok_or(KError::IllegalOperation)
    }

    pub(super) fn vspace_data_mut(&mut self, vs: ObjId) -> Result<&mut VSpaceData, KError> {
        let slot = self.vspace_slot_of(vs)?;
        self.vspaces
            .get_mut(slot)
            .and_then(|d| d.as_mut())
            .ok_or(KError::IllegalOperation)
    }

    /// **KT-38** — số object `VSpace` **còn sống**.
    pub fn vspace_count(&self) -> usize {
        self.objects
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::VSpace)
            .count()
    }

    /// **KT-38** — ảnh chụp một VSpace: root PageTable, asid, thế hệ lúc cấp, `active_on`.
    pub fn vspace_info(&self, vs: ObjId) -> Result<VSpaceInfo, KError> {
        let d = self.vspace_data(vs)?;
        Ok(VSpaceInfo {
            root: d.root,
            asid: d.asid,
            generation: d.generation,
            active_on: d.active_on,
        })
    }

    /// **KT-38** — root PageTable của VSpace (dùng được ngay với
    /// [`Machine::page_table_walk`] — oracle của KT-38).
    pub fn vspace_root(&self, vs: ObjId) -> Result<ObjId, KError> {
        Ok(self.vspace_data(vs)?.root)
    }

    /// **KT-38** — `VSpace.GetAsid` (method 0 của spec, **debug**; `unsupported` tới K3 tức
    /// đây là model API): ASID đã cấp cho VSpace.
    pub fn vspace_get_asid(&self, vs: ObjId) -> Result<u16, KError> {
        Ok(self.vspace_data(vs)?.asid)
    }

    /// **KT-38** — tập hart đang nạp ASID của VSpace (`active_on ⊆ hart online`).
    pub fn vspace_active_on(&self, vs: ObjId) -> Result<HartMask, KError> {
        Ok(self.vspace_data(vs)?.active_on)
    }

    /// **KT-38** — mask hart **online** của mô hình (khởi tạo = `for_harts(hart_count)`).
    pub fn hart_online(&self) -> HartMask {
        self.hart_online
    }

    /// **KT-38** — bật/tắt một hart **online**. Hart offline bị bỏ khỏi **mọi** `active_on`
    /// (không còn chạy VSpace nào) nên **không** nhận shootdown sau đó; bất biến
    /// `active_on ⊆ hart_online` được giữ ở đây, không để caller tự sửa mask.
    ///
    /// `hart >= hart_count()` ⇒ [`KError::InvalidArgument`] (không panic). Bật lại hart **không**
    /// tự gắn nó vào VSpace nào (phải `vspace_activate`).
    pub fn set_hart_online(&mut self, hart: u16, online: bool) -> Result<(), KError> {
        if hart as usize >= self.hart_vspace.len() {
            return Err(KError::InvalidArgument);
        }
        if online {
            self.hart_online = self.hart_online.insert(hart);
        } else {
            self.hart_online = self.hart_online.remove(hart);
            self.hart_vspace[hart as usize] = None;
            for d in self.vspaces.iter_mut().flatten() {
                d.active_on = d.active_on.remove(hart);
            }
            // **KT-40** — hart offline không giữ TLB (không thể dịch khi đã tắt); xoá sạch để
            // oracle không thấy entry mồ côi và hart đã tắt không nhận shootdown.
            self.tlbs[hart as usize].clear();
            self.tlb_pending[hart as usize] = None;
        }
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-38** — hart `hart` **chạy** trên VSpace `vs`: bật bit trong `active_on`.
    ///
    /// Một hart chỉ nạp **một** ASID hiện hành ⇒ nếu hart đang chạy VSpace khác thì bit bị bỏ ở
    /// VSpace cũ trước. **KT-40**: hart **rời** VSpace cũ nên TLB của chính hart này **flush cục
    /// bộ** mọi entry của ASID cũ (thanh ghi ASID đổi) — không phát shootdown vì ASID của VSpace
    /// cũ **không** được phát lại trong cùng thế hệ, nên entry TLB cũ không thể khớp nhầm (xem
    /// `k2-vm.md` §12/§14). Gọi lại trên đúng VSpace đang chạy ⇒ **no-op** (idempotent). Hart
    /// offline/không có thật ⇒ [`KError::InvalidArgument`].
    pub fn vspace_activate(&mut self, vs: ObjId, hart: u16) -> Result<(), KError> {
        self.vspace_slot_of(vs)?;
        if hart as usize >= self.hart_vspace.len() || !self.hart_online.contains(hart) {
            return Err(KError::InvalidArgument);
        }
        if self.hart_vspace[hart as usize] == Some(vs) {
            return Ok(());
        }
        if let Some(old) = self.hart_vspace[hart as usize] {
            // **KT-40** — hart **rời** VSpace cũ (chuyển ASID): TLB của chính hart này xoá mọi
            // entry của ASID cũ (một hart chỉ nạp một ASID hiện hành, như thanh ghi ASID).
            let (old_gen, old_asid) = {
                let d = self.vspace_data(old)?;
                (d.generation, d.asid)
            };
            self.tlb_flush_hart_only(hart, old_gen, old_asid, TlbFlushScope::Asid);
            let old_data = self.vspace_data_mut(old)?;
            old_data.active_on = old_data.active_on.remove(hart);
        }
        {
            let d = self.vspace_data_mut(vs)?;
            d.active_on = d.active_on.insert(hart);
        }
        self.hart_vspace[hart as usize] = Some(vs);
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-38** — hart `hart` **nghỉ** (rời) VSpace `vs`: tắt bit `active_on`.
    ///
    /// Hart không chạy đúng VSpace này ⇒ [`KError::IllegalOperation`] (không im lặng bỏ qua —
    /// bắt lỗi gọi sai thứ tự). **KT-40**: hart rời VSpace nên TLB của chính nó **flush cục bộ**
    /// mọi entry của ASID (flush theo `asid`, không phát shootdown — ASID chưa bị phát lại);
    /// oracle `check_tlb` **bắt buộc** bước này (entry trên hart ngoài `active_on` là vi phạm).
    pub fn vspace_deactivate(&mut self, vs: ObjId, hart: u16) -> Result<(), KError> {
        self.vspace_slot_of(vs)?;
        if hart as usize >= self.hart_vspace.len() {
            return Err(KError::InvalidArgument);
        }
        if self.hart_vspace[hart as usize] != Some(vs) {
            return Err(KError::IllegalOperation);
        }
        {
            // **KT-40** — hart rời VSpace ⇒ TLB của chính nó xoá entry ASID đó (flush theo
            // `asid` cục bộ); hart khác không bị đụng.
            let (generation, asid) = {
                let d = self.vspace_data(vs)?;
                (d.generation, d.asid)
            };
            self.tlb_flush_hart_only(hart, generation, asid, TlbFlushScope::Asid);
        }
        {
            let d = self.vspace_data_mut(vs)?;
            d.active_on = d.active_on.remove(hart);
        }
        self.hart_vspace[hart as usize] = None;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-38** — trạng thái ASID toàn cục (thế hệ, free-list còn lại, đã cấp, số lần cạn).
    pub fn asid_state(&self) -> AsidState {
        AsidState {
            generation: self.asid_generation,
            free: self.asid_free.len(),
            issued: self.asid_issued,
            exhaustions: self.asid_exhaustions,
        }
    }

    /// **KT-38** — log **lệnh flush** (mô hình; KT-40 tiêu thụ). KT-38 chỉ phát
    /// [`FlushScope::Asid`] (teardown) và [`FlushScope::Generation`] (cạn ASID).
    pub fn flush_log(&self) -> &[FlushCmd] {
        &self.flush_log
    }

    /// **KT-38** — số lệnh flush đã phát (= `flush_log().len()`).
    pub fn flush_count(&self) -> u64 {
        self.flush_log.len() as u64
    }

    /// **KT-38** — lệnh flush gần nhất (test "không gửi shootdown cho hart offline").
    pub fn last_flush(&self) -> Option<FlushCmd> {
        self.flush_log.last().copied()
    }

    /// **KT-38** — walk VA→PA **qua VSpace** = [`Machine::page_table_walk`] trên root của nó
    /// (**oracle**; không có bảng riêng để hai đường lệch nhau).
    pub fn vspace_walk(&self, vs: ObjId, va: u64) -> Result<Mapping, KError> {
        let root = self.vspace_root(vs)?;
        self.page_table_walk(root, va)
    }

    /// **KT-38** — `Frame.Map(vspace, va, rights, attrs)` của thiết kế §4.10, model API: map qua
    /// **cap Frame** vào root của VSpace (uỷ quyền nguyên [`Machine::page_table_map`], nên **áp**
    /// luật W^X/kiểu bộ nhớ của KT-39 — không có đường vòng).
    pub fn vspace_map(
        &mut self,
        vs: ObjId,
        va: u64,
        frame_cap: SlotRef,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        let root = self.vspace_root(vs)?;
        self.page_table_map(root, va, frame_cap, rights, attrs)
    }

    /// **KT-38** — như [`Machine::vspace_map`] nhưng nhận `ObjId` Frame (không qua cap), dùng cho
    /// unit test/đường nội bộ — cùng cửa [`Machine::page_table_map_obj`].
    pub fn vspace_map_obj(
        &mut self,
        vs: ObjId,
        va: u64,
        frame: ObjId,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        let root = self.vspace_root(vs)?;
        self.page_table_map_obj(root, va, frame, rights, attrs)
    }

    /// **KT-38/KT-40** — `Frame.Unmap` của thiết kế §4.10, model API: sửa PTE qua root của
    /// VSpace rồi **shootdown** (flush cục bộ + `TlbFlush` tới `active_on` + chờ **đủ** ack).
    ///
    /// Bất biến (Q5): `unmap` trả `Ok` ⇒ mọi hart trong `active_on` đã xử lý `TlbFlush`
    /// (`acked_mask == active_on`) ⇒ không hart nào còn entry cũ cho `va`. Lỗi của
    /// [`Machine::page_table_unmap`] (chưa map/lệch căn/…) trả **nguyên**, không gửi message.
    pub fn vspace_unmap(&mut self, vs: ObjId, va: u64) -> Result<Mapping, KError> {
        let info = self.vspace_info(vs)?;
        if info.active_on.is_empty() {
            // Không hart nào chạy VSpace ⇒ không entry nào tồn tại (deactivate/offline đã xoá);
            // vẫn flush cục bộ cho chắc rồi sửa PTE (DoD: "active_on rỗng ⇒ không gửi gì").
            let _ = self.tlb_flush_hart_only(
                self.current_hart,
                info.generation,
                info.asid,
                TlbFlushScope::Page { va },
            );
            return self.page_table_unmap(info.root, va);
        }
        let hart = self.current_hart;
        self.tlb_shootdown_prepare(hart, info, va)?;
        let mapping = match self.page_table_unmap(info.root, va) {
            Ok(m) => m,
            Err(e) => {
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
        };
        self.tlb_shootdown_post(hart)?;
        self.tlb_shootdown_wait(hart)?;
        Ok(mapping)
    }

    /// **KT-39/KT-40** — `Frame.Protect`/hạ quyền của design §4.10, model API qua VSpace: đổi
    /// quyền/attrs của một lá (uỷ quyền nguyên [`Machine::page_table_protect`] — cùng luật
    /// `validate_attrs` + W^X theo bộ đếm Frame toàn cục) rồi **shootdown** như `unmap`.
    ///
    /// Entry TLB mang quyền/attrs **cũ** phải bị vô hiệu trước khi `protect` trả `Ok`; nếu
    /// không, hart khác có thể dùng translation cũ (mạnh hơn?) — vi phạm TLB-safety.
    pub fn vspace_protect(
        &mut self,
        vs: ObjId,
        va: u64,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        let info = self.vspace_info(vs)?;
        if info.active_on.is_empty() {
            let _ = self.tlb_flush_hart_only(
                self.current_hart,
                info.generation,
                info.asid,
                TlbFlushScope::Page { va },
            );
            return self.page_table_protect(info.root, va, rights, attrs);
        }
        let hart = self.current_hart;
        self.tlb_shootdown_prepare(hart, info, va)?;
        let mapping = match self.page_table_protect(info.root, va, rights, attrs) {
            Ok(m) => m,
            Err(e) => {
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
        };
        self.tlb_shootdown_post(hart)?;
        self.tlb_shootdown_wait(hart)?;
        Ok(mapping)
    }

    // ------------------------------------------- VM fault (KT-41) `[TK]`
    //
    // **K2 không phải kernel thật, không có pager/memory server** (D2-65 Q6; COW/pager = K3):
    // `vm_touch` là op **model** mô phỏng một truy cập bộ nhớ của TCB đang chạy. Nó **không**
    // cấp trang, **không** sửa PTE — chỉ `vspace_walk` rồi, nếu miss/thiếu quyền, giao fault
    // qua **đúng đường fault K1c** (`inject_fault`): TCB `BlockedOnSend{call:true,
    // fault:Some(Vm{..})}` tới `fault_ep`. procsrv model chỉ nhận + ghi log; muốn chạy tiếp thì
    // `Reply` (fault-resume) — trang **vẫn** chưa được cấp (test chạm lại vẫn fault).

    /// **KT-41** — `Touch(vspace, va, access)` của mô hình VM (K2-PLAN §3 KT-41; D2-65 Q6):
    /// TCB **đang chạy** trên hart hiện tại chạm `va` trong `vs`.
    ///
    /// - `vspace_walk` miss ⇒ `FaultKind::Vm { reason: NotMapped }`.
    /// - Có lá nhưng thiếu quyền ⇒ `Permission` / `Wx` (xem [`Machine::vm_touch_reason`]).
    /// - Fault giao qua [`Machine::inject_fault`] tới `fault_ep` của TCB: **không** có `fault_ep`
    ///   ⇒ `InvalidArgument` (giữ nguyên K1c — D2-56(c)); TCB không `Ready`/`Running` ⇒
    ///   `IllegalOperation`; lỗi walk khác `NotMapped` (VA ngoài dải, VSpace chết) trả nguyên.
    /// - Không fault ⇒ `Ok(())`, trạng thái TCB **không đổi**.
    ///
    /// Model-only `[TK]`, **không** phải ABI: không sửa `spec/`; không cấp trang (không pager ở
    /// K2). `vs` là tham số tường minh vì TCB của K2 **chưa có** trường `vspace` (K3 mới có).
    pub fn vm_touch(&mut self, vs: ObjId, va: u64, access: FaultAccess) -> Result<(), KError> {
        let tcb = self
            .current(self.current_hart)
            .ok_or(KError::IllegalOperation)?;
        let reason = match self.vspace_walk(vs, va) {
            Ok(m) => Self::vm_touch_reason(&m, access),
            Err(KError::NotMapped) => Some(VmFaultReason::NotMapped),
            Err(e) => return Err(e),
        };
        if let Some(reason) = reason {
            self.inject_fault(
                tcb,
                FaultKind::Vm {
                    vspace: vs,
                    va,
                    access,
                    reason,
                },
            )?;
        }
        Ok(())
    }

    /// **KT-41** — quyết định `reason` của một truy cập trên mapping **đã có** (hàm thuần, tách
    /// để property/`vm_touch` dùng **cùng một** luật; `None` = truy cập hợp lệ).
    ///
    /// Thứ tự luật (ghi ở `k2-vm.md` §15):
    /// 1. `Read` cần `READ`; `Write` cần `WRITE`; `Execute` cần `EXEC && !NX`.
    /// 2. **W^X ở thời điểm dùng** (KT-39/D2-67 đã cấm *map* W+X): exec trang `WRITE`, hoặc ghi
    ///    trang `EXEC` ⇒ `Wx` — kể cả khi trang cũng thiếu bit tương ứng (W^X là lý do mạnh
    ///    hơn `Permission`, và là lý do **duy nhất** khiến `Wx` quan sát được sau khi map).
    /// 3. Còn lại thiếu quyền ⇒ `Permission`.
    pub fn vm_touch_reason(
        m: &crate::pagetable::Mapping,
        access: FaultAccess,
    ) -> Option<VmFaultReason> {
        let x = m.attrs.contains(MapAttrs::EXEC) && !m.attrs.contains(MapAttrs::NX);
        match access {
            FaultAccess::Read => {
                (!m.rights.contains(MapRights::READ)).then_some(VmFaultReason::Permission)
            }
            FaultAccess::Write => {
                if m.rights.contains(MapRights::WRITE) {
                    None
                } else if x {
                    Some(VmFaultReason::Wx)
                } else {
                    Some(VmFaultReason::Permission)
                }
            }
            FaultAccess::Execute => {
                if x {
                    None
                } else if m.rights.contains(MapRights::WRITE) {
                    Some(VmFaultReason::Wx)
                } else {
                    Some(VmFaultReason::Permission)
                }
            }
        }
    }

    /// **KT-38** — bất biến **VSpace + `active_on`** ("check_vspace sớm"; KT-42 gộp vào
    /// `check_vm`). Chạy sau **mỗi** public op qua [`Machine::sched_after_op`], cửa sổ nhanh
    /// `vspace_objects == 0 && hart_vspace rỗng`.
    ///
    /// Kiểm:
    /// 1. **kiểu/dữ liệu**: `ty == VSpace` ⇔ có `Meta.space` + dữ liệu; object chết ⇒ dữ liệu
    ///    `None`; object sống ⇒ dữ liệu `Some`; số dữ liệu `Some` = số VSpace sống;
    /// 2. **root**: là PageTable **sống**, **bảng gốc** (cấp 2, chưa attach), không VSpace nào
    ///    dùng chung root;
    /// 3. **ASID**: `asid < ASID_COUNT`, `generation <= asid_generation` (thế hệ của VSpace không
    ///    đi trước thế hệ toàn cục);
    /// 4. **`active_on`**: `active_on ⊆ hart_online` và `hart_vspace[h] == Some(vs)` ⇔ bit `h`
    ///    bật — mỗi hart tối đa **một** VSpace (một ASID hiện hành);
    /// 5. **không tham chiếu chết**: `hart_vspace[h]` chỉ trỏ VSpace **sống**.
    pub fn check_vspace(&self) -> Result<(), String> {
        if self.vspace_objects == 0 && self.hart_vspace.iter().all(|v| v.is_none()) {
            return Ok(());
        }
        // Chỉ cần bảng "root đã dùng" khi có **≥ 2** VSpace sống; đường nóng (1 VSpace: vòng
        // tạo/huỷ 10.000) không cấp phát gì.
        let data_live = self.vspaces.iter().filter(|d| d.is_some()).count();
        let mut root_taken = if data_live > 1 {
            vec![false; self.objects.len()]
        } else {
            Vec::new()
        };
        let mut live = 0usize;
        for (i, m) in self.objects.iter().enumerate() {
            let obj = (i as u32) + 1;
            match (m.ty == ObjType::VSpace, m.space) {
                (true, None) => return Err(format!("object {obj} là VSpace nhưng thiếu dữ liệu")),
                (false, Some(_)) => {
                    return Err(format!(
                        "object {obj} ({}) không phải VSpace nhưng có dữ liệu VSpace",
                        m.ty.name()
                    ))
                }
                _ => {}
            }
            let Some(sidx) = m.space else { continue };
            let data = self.vspaces.get(sidx).and_then(|d| d.as_ref());
            if !m.alive {
                if data.is_some() {
                    return Err(format!("VSpace {obj} đã chết nhưng còn dữ liệu"));
                }
                continue;
            }
            live += 1;
            let Some(d) = data else {
                return Err(format!("VSpace {obj} sống nhưng thiếu dữ liệu"));
            };
            // (2) root: PageTable sống + bảng gốc + không dùng chung.
            let ridx = self
                .object_index(d.root)
                .map_err(|e| format!("VSpace {obj}: root {} lỗi {e:?}", d.root))?;
            let rm = &self.objects[ridx];
            if !rm.alive || rm.ty != ObjType::PageTable {
                return Err(format!(
                    "VSpace {obj}: root {} không phải PageTable sống",
                    d.root
                ));
            }
            let Some(rd) = rm
                .pt
                .and_then(|p| self.page_tables.get(p))
                .and_then(|d| d.as_ref())
            else {
                return Err(format!("VSpace {obj}: root {} thiếu dữ liệu", d.root));
            };
            if rd.attached.is_some() || rd.level != PT_ROOT_LEVEL {
                return Err(format!(
                    "VSpace {obj}: root {} không phải bảng gốc (cấp {}, attach {:?})",
                    d.root, rd.level, rd.attached
                ));
            }
            if data_live > 1 {
                if root_taken[ridx] {
                    return Err(format!(
                        "VSpace {obj}: root {} dùng chung với VSpace khác",
                        d.root
                    ));
                }
                root_taken[ridx] = true;
            }
            // (3) ASID/thế hệ.
            if (d.asid as usize) >= ASID_COUNT {
                return Err(format!("VSpace {obj}: asid {} ngoài dải", d.asid));
            }
            if d.generation > self.asid_generation {
                return Err(format!(
                    "VSpace {obj}: thế hệ {} > thế hệ toàn cục {}",
                    d.generation, self.asid_generation
                ));
            }
            // (4) active_on ⊆ hart online + đối xứng với hart_vspace.
            if !d.active_on.is_subset_of(self.hart_online) {
                return Err(format!(
                    "VSpace {obj}: active_on {:?} ⊄ hart online {:?}",
                    d.active_on, self.hart_online
                ));
            }
            for (hart, slot) in self.hart_vspace.iter().enumerate() {
                let bit = d.active_on.contains(hart as u16);
                let cur = *slot == Some(obj);
                if bit != cur {
                    return Err(format!(
                        "VSpace {obj}: active_on và hart_vspace lệch ở hart {hart}"
                    ));
                }
            }
        }
        let data_some = self.vspaces.iter().filter(|d| d.is_some()).count();
        if live != data_some {
            return Err(format!(
                "số VSpace sống {live} ≠ số dữ liệu VSpace {data_some} (rò/tombstone sai)"
            ));
        }
        // (5) hart_vspace chỉ trỏ VSpace sống.
        for (hart, cur) in self.hart_vspace.iter().enumerate() {
            let Some(vs) = cur else { continue };
            let idx = self
                .object_index(*vs)
                .map_err(|e| format!("hart {hart}: hart_vspace lỗi {e:?}"))?;
            let m = &self.objects[idx];
            if !m.alive || m.ty != ObjType::VSpace {
                return Err(format!(
                    "hart {hart}: hart_vspace trỏ object {} không phải VSpace sống",
                    vs
                ));
            }
        }
        Ok(())
    }

    /// **KT-38** — bất biến **ASID/free-list/thế hệ** ("check_asid sớm"; KT-42 gộp vào
    /// `check_vm`). Chạy sau **mỗi** public op qua [`Machine::sched_after_op`], cửa sổ nhanh:
    /// chưa từng có VSpace và trạng thái ASID còn nguyên thủy.
    ///
    /// Kiểm:
    /// 1. **free-list**: `len + issued == ASID_COUNT`, mọi ASID `< ASID_COUNT`, **giảm dần
    ///    nghiêm ngặt** (không phần tử trùng — bắt "trả ASID về free-list" sai);
    /// 2. **không trùng trong một thế hệ**: không hai VSpace sống cùng `(generation, asid)`
    ///    (ASID chỉ tái dùng ở **thế hệ khác** — DoD KT-38);
    /// 3. **nhất quán với free-list**: VSpace **thuộc thế hệ hiện tại** giữ ASID **không** nằm
    ///    trong free-list, và số VSpace sống của thế hệ hiện tại `≤ issued`.
    pub fn check_asid(&self) -> Result<(), String> {
        if self.vspace_objects == 0
            && self.asid_generation == 0
            && self.asid_exhaustions == 0
            && self.asid_issued == 0
            && self.asid_free.len() == ASID_COUNT
        {
            return Ok(());
        }
        if self.asid_free.len() + self.asid_issued as usize != ASID_COUNT {
            return Err(format!(
                "free-list {} + issued {} ≠ {ASID_COUNT}",
                self.asid_free.len(),
                self.asid_issued
            ));
        }
        let mut prev: Option<u16> = None;
        for &a in &self.asid_free {
            if (a as usize) >= ASID_COUNT {
                return Err(format!("free-list có ASID ngoài dải: {a}"));
            }
            if let Some(p) = prev {
                if a >= p {
                    return Err(format!(
                        "free-list không giảm dần nghiêm ngặt ({p} rồi {a})"
                    ));
                }
            }
            prev = Some(a);
        }
        // Mask 256 bit của free-list để kiểm "ASID còn trong free-list" O(1)/VSpace (check chạy
        // sau **mỗi** op; 257 VSpace là ca thật của KT-38 — `Vec::contains` sẽ là O(n·256)).
        let mut free_mask = [0u64; ASID_COUNT / 64];
        for &a in &self.asid_free {
            free_mask[(a as usize) / 64] |= 1u64 << (a % 64);
        }
        let in_free = |a: u16| free_mask[(a as usize) / 64] & (1u64 << (a % 64)) != 0;
        // Thu thập `(generation, asid, object)` của VSpace sống rồi **sort** để bắt trùng trong
        // một thế hệ bằng O(n log n), không phải `contains` O(n²).
        let mut seen: Vec<(u64, u16, ObjId)> = Vec::new();
        let mut cur_live: u32 = 0;
        for (i, m) in self.objects.iter().enumerate() {
            if !m.alive || m.ty != ObjType::VSpace {
                continue;
            }
            let Some(sidx) = m.space else { continue };
            let Some(d) = self.vspaces.get(sidx).and_then(|d| d.as_ref()) else {
                continue;
            };
            seen.push((d.generation, d.asid, (i as u32) + 1));
            if d.generation == self.asid_generation {
                cur_live += 1;
                if in_free(d.asid) {
                    return Err(format!(
                        "VSpace {} giữ ASID {} nhưng ASID còn trong free-list thế hệ {}",
                        i + 1,
                        d.asid,
                        self.asid_generation
                    ));
                }
            }
        }
        seen.sort_unstable();
        for w in seen.windows(2) {
            if w[0].0 == w[1].0 && w[0].1 == w[1].1 {
                return Err(format!(
                    "ASID {} trùng trong thế hệ {} (VSpace {} và {})",
                    w[1].1, w[1].0, w[0].2, w[1].2
                ));
            }
        }
        if cur_live > self.asid_issued {
            return Err(format!(
                "VSpace sống của thế hệ hiện tại {cur_live} > issued {}",
                self.asid_issued
            ));
        }
        Ok(())
    }

    /// **KT-36** — bất biến của **toàn bộ Frame** + cửa sổ PA của Untyped ("check_frames sớm";
    /// KT-42 sẽ gộp vào `check_vm` cùng VSpace/PageTable/TLB).
    ///
    /// **Bảy nhóm** bất biến (khớp D2-66):
    /// 1. **kiểu**: `ty == Frame` ⇔ có [`FrameMeta`] (và ngược lại);
    /// 2. **trạng thái**: `FrameMeta.live == Meta.alive`;
    /// 3. **cỡ**: `size_bits ∈ {12, 21, 30}`, `len == 1 << size_bits`;
    /// 4. **cửa sổ PA + chủ sở hữu**: `pa == base + offset`, `pa` căn đúng cỡ, `[pa, pa + cỡ)`
    ///    nằm trong cửa sổ, chủ sở hữu là Untyped và **sống** nếu Frame sống (không mồ côi);
    /// 5. **bộ đếm cấu trúc**: `mapped_w ≤ maps`, `mapped_x ≤ maps` (luật W^X chặt hơn là KT-39);
    /// 6. **frame chết sạch**: Frame đã chết ⇒ cả ba bộ đếm bằng **0** ("frame chết ⇒ không còn
    ///    mapping" — DoD KT-36);
    /// 7. **toàn cục**: không hai Frame **sống** chồng `pa` (mọi Untyped; O(n log n) bằng sort)
    ///    và watermark cửa sổ của mỗi Untyped sống = `max(offset + len)` của Frame sống của nó
    ///    (bắt rò watermark/không reset sau `Revoke`), ≤ [`UNTYPED_PA_WINDOW`].
    pub fn check_frames(&self) -> Result<(), String> {
        // Cửa sổ nhanh: mô hình **chưa từng** có Frame (mọi test K1a..K1c, L1 1.000 seed) thì
        // bất biến Frame là rỗng — không quét/không cấp phát gì trên đường nóng của mỗi op.
        if self.frame_objects == 0 {
            return Ok(());
        }
        let mut ranges: Vec<(u64, u64, ObjId)> = Vec::new();
        // `max_end[i]` = `max(offset + len)` của Frame **sống** thuộc Untyped chỉ số `i` — gom
        // trong cùng vòng quét để bước 6 không phải quét chéo O(số Untyped × số object).
        let mut max_end: Vec<u64> = vec![0; self.objects.len()];
        for (i, m) in self.objects.iter().enumerate() {
            let obj = (i as u32) + 1;
            match (m.ty == ObjType::Frame, m.frame) {
                (true, None) => {
                    return Err(format!("object {obj} là Frame nhưng thiếu metadata Frame"))
                }
                (false, Some(_)) => {
                    return Err(format!(
                        "object {obj} ({}) không phải Frame nhưng có metadata Frame",
                        m.ty.name()
                    ))
                }
                _ => {}
            }
            let Some(f) = m.frame else { continue };
            if f.live != m.alive {
                return Err(format!(
                    "Frame {obj}: live {} ≠ alive {} của sổ cái",
                    f.live, m.alive
                ));
            }
            let size = FrameSize::from_size_bits(f.size_bits)
                .map_err(|_| format!("Frame {obj}: size_bits {} lạ", f.size_bits))?;
            let need = size.size_bytes();
            if m.len as u64 != need {
                return Err(format!("Frame {obj}: len {} ≠ cỡ {need}", m.len));
            }
            if f.pa % need != 0 {
                return Err(format!("Frame {obj}: pa {:#x} lệch căn {need}", f.pa));
            }
            let Some(u_idx) = m.untyped else {
                return Err(format!("Frame {obj}: mồ côi (không thuộc Untyped nào)"));
            };
            let Some(owner) = self.objects.get(u_idx) else {
                return Err(format!("Frame {obj}: Untyped #{} không tồn tại", u_idx + 1));
            };
            if owner.ty != ObjType::Untyped {
                return Err(format!(
                    "Frame {obj}: chủ sở hữu #{} không phải Untyped",
                    u_idx + 1
                ));
            }
            if m.alive && !owner.alive {
                return Err(format!(
                    "Frame {obj}: mồ côi — Untyped #{} đã chết",
                    u_idx + 1
                ));
            }
            let Some(area) = owner.frame_area else {
                return Err(format!(
                    "Untyped #{}: Frame {obj} nhưng thiếu cửa sổ PA",
                    u_idx + 1
                ));
            };
            let Some((base, win_end)) = area.window() else {
                return Err(format!("Untyped #{}: cửa sổ PA tràn u64", u_idx + 1));
            };
            if f.pa != base.saturating_add(m.offset as u64) {
                return Err(format!(
                    "Frame {obj}: pa {:#x} ≠ base {base:#x} + offset {}",
                    f.pa, m.offset
                ));
            }
            let end =
                f.pa.checked_add(need)
                    .ok_or_else(|| format!("Frame {obj}: pa + cỡ tràn u64"))?;
            if end > win_end {
                return Err(format!(
                    "Frame {obj}: [{:#x}, {end:#x}) ra ngoài cửa sổ [{base:#x}, {win_end:#x})",
                    f.pa
                ));
            }
            // KT-36 chỉ giữ bộ đếm **đúng cấu trúc**: mỗi bộ đếm ≤ tổng số mapping. Bất biến
            // chặt hơn `mapped_w + mapped_x ≤ maps` chỉ đúng **sau** khi KT-39 áp W^X (một
            // mapping không thể vừa W vừa X); KT-36 cố ý chưa áp luật đó.
            if f.mapped_w > f.maps || f.mapped_x > f.maps {
                return Err(format!(
                    "Frame {obj}: mapped_w {} / mapped_x {} > maps {}",
                    f.mapped_w, f.mapped_x, f.maps
                ));
            }
            if !m.alive && !f.counters_zero() {
                return Err(format!(
                    "Frame {obj}: đã chết nhưng còn mapping (maps {}, w {}, x {})",
                    f.maps, f.mapped_w, f.mapped_x
                ));
            }
            if m.alive {
                ranges.push((f.pa, end, obj));
                let e = m.offset as u64 + m.len as u64;
                if e > max_end[u_idx] {
                    max_end[u_idx] = e;
                }
            }
        }
        // 5) Không hai Frame sống chồng pa (mọi Untyped).
        ranges.sort_unstable();
        for pair in ranges.windows(2) {
            if pair[0].1 > pair[1].0 {
                return Err(format!(
                    "Frame {} [{:#x}, {:#x}) chồng Frame {} [{:#x}, {:#x})",
                    pair[0].2, pair[0].0, pair[0].1, pair[1].2, pair[1].0, pair[1].1
                ));
            }
        }
        // 6) watermark cửa sổ = max(offset + len) của Frame sống của Untyped đó.
        for (i, m) in self.objects.iter().enumerate() {
            if !m.alive || m.ty != ObjType::Untyped {
                continue;
            }
            let obj = (i as u32) + 1;
            let Some(area) = m.frame_area else {
                return Err(format!("Untyped {obj}: thiếu cửa sổ PA"));
            };
            if area.watermark > UNTYPED_PA_WINDOW {
                return Err(format!(
                    "Untyped {obj}: watermark {} > cửa sổ {UNTYPED_PA_WINDOW}",
                    area.watermark
                ));
            }
            if area.watermark != max_end[i] {
                return Err(format!(
                    "Untyped {obj}: watermark {} ≠ max(offset+len) {} của Frame sống",
                    area.watermark, max_end[i]
                ));
            }
        }
        Ok(())
    }

    // ------------------------------------------- KT-42: một cửa kiểm VM ở cổng

    /// **KT-42** — **một cửa kiểm VM** cho cổng (L0): gộp toàn bộ bất biến của K2 VM —
    /// [`Machine::check_frames`] + [`Machine::check_pages`] + [`Machine::check_wx`] +
    /// [`Machine::wx_recount`] (đếm lại **độc lập** từ cây) + [`Machine::check_vspace`] +
    /// [`Machine::check_asid`] + [`Machine::check_tlb`] + [`Machine::check_fault`].
    ///
    /// **Không thay hook per-op** (`sched_after_op` vẫn gọi từng checker như trước): đây là
    /// lưới **đầy đủ ở cổng**, nơi đắt hơn nhưng chạy một lần; L0 chạy nó sau mỗi kịch bản
    /// `.scn` và trong `tests/kt42_vmcheck.rs`.
    ///
    /// **Chi phí** (đo trên mô hình, không phải ABI): `O(F log F)` sắp xếp cửa sổ PA của
    /// `check_frames` + **hai** lần đi toàn bộ cây PageTable (`check_pages` và `wx_recount`) +
    /// `O(F + V + H)` cho W^X/VSpace/ASID + `O(TLB entries × walk)` cho `check_tlb` +
    /// `O(endpoint × hàng)` cho `check_fault`; mỗi checker tự có cửa sổ nhanh (`*_objects == 0`
    /// / `tlb_fill_total == 0` / `fault_events == 0`) nên trạng thái **không có VM** gần như
    /// miễn phí. Không cấp phát thêm ngoài các `Vec` mà từng checker đã dùng.
    ///
    /// Trả lỗi **có tiền tố** tên checker (`check_pages: …`) để log cổng chỉ đúng lưới vỡ;
    /// dừng ở lỗi đầu tiên (không có nhiều lỗi đồng thời trong mô hình đúng).
    pub fn check_vm(&self) -> Result<(), String> {
        self.check_frames()
            .map_err(|e| format!("check_frames: {e}"))?;
        self.check_pages()
            .map_err(|e| format!("check_pages: {e}"))?;
        self.check_wx().map_err(|e| format!("check_wx: {e}"))?;
        self.wx_recount().map_err(|e| format!("wx_recount: {e}"))?;
        self.check_vspace()
            .map_err(|e| format!("check_vspace: {e}"))?;
        self.check_asid().map_err(|e| format!("check_asid: {e}"))?;
        self.check_tlb().map_err(|e| format!("check_tlb: {e}"))?;
        self.check_fault()
            .map_err(|e| format!("check_fault: {e}"))?;
        Ok(())
    }
}
