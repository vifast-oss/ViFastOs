// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `caps`: CSpace: cap, cây dẫn xuất MDB, copy/mint/delete/revoke.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    pub fn put_cap(&mut self, cnode: ObjId, slot: usize, cap: CapId) -> Result<(), KError> {
        self.check_cnode_slot(cnode, slot)?;
        {
            let cap_ref = self.cap(cap)?;
            if cap_ref.slot.is_some() {
                return Err(KError::IllegalOperation);
            }
        }
        if self.cnode_slot(cnode, slot)?.is_some() {
            return Err(KError::SlotOccupied);
        }
        self.cnode_set_slot(cnode, slot, Some(cap))?;
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(SlotRef::new(cnode, slot));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- truy vấn

    pub fn cap_at(&self, cnode: ObjId, slot: usize) -> Result<CapId, KError> {
        let n = self.cnode_slot_count(cnode)?;
        if slot >= n {
            return Err(KError::RangeError);
        }
        match self.cnode_slot(cnode, slot)? {
            None => Err(KError::SlotEmpty),
            Some(cap) => {
                if self.cap(cap)?.alive {
                    Ok(cap)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
        }
    }

    pub fn cap_obj(&self, cap: CapId) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if c.alive {
            Ok(c.obj)
        } else {
            Err(KError::InvalidCapability)
        }
    }

    pub fn cap_obj_at(&self, cnode: ObjId, slot: usize) -> Result<ObjId, KError> {
        self.cap_obj(self.cap_at(cnode, slot)?)
    }

    /// Ảnh chụp cap; `Err(InvalidCapability)` khi id không hợp lệ (0 hoặc quá dải).
    /// Cap **đã chết** vẫn trả `Ok` với `alive = false` để test kiểm dấu vết.
    pub fn cap_info(&self, cap: CapId) -> Result<CapInfo, KError> {
        let c = self.cap(cap)?;
        Ok(CapInfo {
            cap,
            obj: c.obj,
            ty: c.ty,
            parent: c.parent,
            slot: c.slot,
            rights: c.rights,
            badge: c.badge,
            alive: c.alive,
        })
    }

    /// Toàn bộ bảng cap theo thứ tự cấp phát (gồm cap đã chết) — test/property dùng.
    pub fn cap_table(&self) -> Vec<CapInfo> {
        (1..=self.caps.len() as CapId)
            .map(|cap| {
                self.cap_info(cap)
                    .expect("cap_table: id trong dải phải hợp lệ")
            })
            .collect()
    }

    pub fn live_cap_count(&self) -> usize {
        self.caps.iter().filter(|c| c.alive).count()
    }

    /// Con trực tiếp còn sống của `cap`, theo thứ tự cấp phát (thứ tự MDB của mô hình).
    pub fn cap_children(&self, cap: CapId) -> Result<Vec<CapId>, KError> {
        self.cap(cap)?;
        Ok((1..=self.caps.len() as CapId)
            .filter(|i| {
                let c = &self.caps[*i as usize - 1];
                c.alive && c.parent == Some(cap)
            })
            .collect())
    }

    pub fn cnode_slot_count(&self, cnode: ObjId) -> Result<usize, KError> {
        let (alive, slots) = {
            let m = self.meta(cnode)?;
            (m.alive, m.slots)
        };
        // CNode đã finalise: slot đã xoá trắng trước khi body bị destroy; giữ nguyên
        // kết quả API cũ (`SlotEmpty`) bằng số slot ghi trong metadata.
        if !alive {
            return slots.ok_or(KError::WrongObjectType);
        }
        self.body(cnode, |k| match k {
            Kind::CNode { slots } => Ok(slots.len()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Bất biến cây dẫn xuất cap (MDB tối giản) — thiết kế §4.3/§9.6:
    /// - cap sống trỏ object **sống** cùng loại với cap; không cap nào trỏ object chết;
    /// - cap sống có cha ⇒ cha sống và không có chu trình (cha của cap do `Retype` sinh trỏ
    ///   object khác là bình thường — quan hệ dẫn xuất, không phải quan hệ cùng object);
    /// - cap sống nằm trong slot ⇒ CNode sống và slot trỏ ngược lại đúng cap; mọi slot của
    ///   CNode sống chỉ chứa cap sống đúng chiều (không cap chết nằm lại trong slot);
    /// - cap sống là con thì **phải** nằm trong một slot (không mồ côi); cap gốc được phép
    ///   không slot (harness giữ trước khi đặt).
    ///
    /// Bất biến riêng của `Copy`/`Mint` — con trỏ **cùng object** với cha và quyền con
    /// ⊆ quyền cha — được kiểm trong property test CSpace (`tests/kt10_property.rs`), nơi
    /// toàn bộ cây chỉ dựng bằng Copy/Mint.
    pub fn check_caps(&self) -> Result<(), String> {
        // 1) cap ↔ object.
        for (i, c) in self.caps.iter().enumerate() {
            let cap = (i as u32) + 1;
            if !c.alive {
                if c.slot.is_some() {
                    return Err(format!("cap chết {cap} còn giữ slot"));
                }
                continue;
            }
            if c.obj == 0 || c.obj as usize > self.objects.len() {
                return Err(format!("cap {cap} trỏ object ngoài dải ({})", c.obj));
            }
            let m = &self.objects[c.obj as usize - 1];
            if !m.alive {
                return Err(format!("cap {cap} trỏ object {} đã chết", c.obj));
            }
            if m.ty != c.ty {
                return Err(format!(
                    "cap {cap} kiểu {:?} nhưng object {} kiểu {:?}",
                    c.ty, c.obj, m.ty
                ));
            }
        }
        // 2) quan hệ cha–con + không chu trình.
        for (i, c) in self.caps.iter().enumerate() {
            let cap = (i as u32) + 1;
            if !c.alive {
                continue;
            }
            let Some(p) = c.parent else { continue };
            if p == 0 || p as usize > self.caps.len() {
                return Err(format!("cap {cap} có cha ngoài dải ({p})"));
            }
            let parent = &self.caps[p as usize - 1];
            if !parent.alive {
                return Err(format!("cap {cap} mồ côi: cha {p} đã chết"));
            }
            let mut cur = Some(p);
            let mut steps = 0usize;
            while let Some(x) = cur {
                if x == 0 || x as usize > self.caps.len() {
                    return Err(format!("cap {cap}: chuỗi cha có id ngoài dải ({x})"));
                }
                if x == cap {
                    return Err(format!("cap {cap}: chu trình cha"));
                }
                cur = self.caps[x as usize - 1].parent;
                steps += 1;
                if steps > self.caps.len() {
                    return Err(format!("cap {cap}: chuỗi cha dài bất thường"));
                }
            }
        }
        // 3) slot ↔ cap (đối xứng MDB) và slot của CNode sống.
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.ty != ObjType::CNode {
                continue;
            }
            let cnode = (i as u32) + 1;
            let slots = self
                .cnode_slots_of(cnode)
                .map_err(|e| format!("CNode {cnode}: kind lạ ({e:?})"))?;
            for (slot, entry) in slots.iter().enumerate() {
                let Some(cap) = entry else { continue };
                if *cap == 0 || *cap as usize > self.caps.len() {
                    return Err(format!("slot {slot} của CNode {cnode} có cap ngoài dải"));
                }
                let c = &self.caps[*cap as usize - 1];
                if !c.alive {
                    return Err(format!("slot {slot} của CNode {cnode} chứa cap chết {cap}"));
                }
                if c.slot != Some(SlotRef::new(cnode, slot)) {
                    return Err(format!(
                        "slot {slot} của CNode {cnode} trỏ cap {cap} nhưng cap không trỏ ngược"
                    ));
                }
            }
        }
        for (i, c) in self.caps.iter().enumerate() {
            let cap = (i as u32) + 1;
            if !c.alive {
                continue;
            }
            match c.slot {
                Some(SlotRef { cnode, slot }) => {
                    if !self.object_alive(cnode) {
                        return Err(format!("cap {cap} nằm trong CNode {cnode} đã chết"));
                    }
                    if self.object_type(cnode) != Some(ObjType::CNode) {
                        return Err(format!(
                            "cap {cap} nằm trong object {cnode} không phải CNode"
                        ));
                    }
                    let n = self
                        .cnode_slot_count(cnode)
                        .map_err(|e| format!("cap {cap}: CNode {cnode}: {e:?}"))?;
                    if slot >= n {
                        return Err(format!(
                            "cap {cap} ở slot {slot} ngoài dải của CNode {cnode}"
                        ));
                    }
                    if self
                        .cnode_slot(cnode, slot)
                        .map_err(|e| format!("cap {cap}: CNode {cnode}: {e:?}"))?
                        != Some(cap)
                    {
                        return Err(format!(
                            "cap {cap} nói nằm ở ({cnode}, {slot}) nhưng slot không trỏ lại"
                        ));
                    }
                }
                None => {
                    if c.parent.is_some() {
                        return Err(format!("cap {cap} là con nhưng không nằm trong slot nào"));
                    }
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- CSpace

    /// `CNode.Copy`: tạo cap con ở đích; đích phải rỗng. Giữ nguyên quyền + badge của nguồn.
    pub fn copy(&mut self, src: SlotRef, dst: SlotRef) -> Result<CapId, KError> {
        self.copy_rights(src, dst, None)
    }

    /// `CNode.Copy` với `rights?` của spec (`[method_signature."CNode.Copy"]`): `None` =
    /// giữ quyền nguồn; `Some(r)` phải là tập con, nếu không `InsufficientRights` (F3).
    pub fn copy_rights(
        &mut self,
        src: SlotRef,
        dst: SlotRef,
        rights: Option<u8>,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (obj, ty, src_rights, badge) = {
            let c = self.cap(src_cap)?;
            (c.obj, c.ty, c.rights, c.badge)
        };
        let rights = match rights {
            Some(r) => {
                if r & !src_rights != 0 {
                    return Err(KError::InsufficientRights);
                }
                r
            }
            None => src_rights,
        };
        self.derive(src_cap, obj, ty, dst, rights, badge)
    }

    /// `CNode.Mint`: như Copy nhưng đặt lại quyền (phải ⊆ quyền nguồn, nếu không
    /// `InsufficientRights`) và badge (chỉ Endpoint/Notification; `None`/`Some(0)` = giữ
    /// badge nguồn). Badge cho loại khác ⇒ `InvalidArgument`.
    pub fn mint(
        &mut self,
        src: SlotRef,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (obj, ty, src_rights, src_badge) = {
            let c = self.cap(src_cap)?;
            (c.obj, c.ty, c.rights, c.badge)
        };
        if rights & !src_rights != 0 {
            return Err(KError::InsufficientRights);
        }
        let badge = match badge {
            // F3: `None` = giữ badge nguồn; `Some(0)` = badge 0 (không badge) ⇒ **xoá**.
            None => src_badge,
            Some(0) => None,
            Some(b) => {
                if !matches!(ty, ObjType::Endpoint | ObjType::Notification) {
                    return Err(KError::InvalidArgument);
                }
                Some(b)
            }
        };
        self.derive(src_cap, obj, ty, dst, rights, badge)
    }

    /// Tạo cap con của `src_cap` rồi gắn vào `dst` — đã kiểm nguồn/quyền trước khi cấp
    /// ⇒ thao tác lỗi không để lại cap mồ côi.
    pub(super) fn derive(
        &mut self,
        src_cap: CapId,
        obj: ObjId,
        ty: ObjType,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        self.check_cnode_slot(dst.cnode, dst.slot)?;
        if self.cnode_slot(dst.cnode, dst.slot)?.is_some() {
            return Err(KError::SlotOccupied);
        }
        let new_cap = self.push_cap(obj, ty, Some(src_cap), rights, badge);
        self.cnode_set_slot(dst.cnode, dst.slot, Some(new_cap))?;
        self.caps[new_cap as usize - 1].slot = Some(dst);
        Ok(new_cap)
    }

    /// `CNode.Delete`: xoá cap trong slot (object không bị finalise — chỉ Revoke mới finalise).
    /// Con của cap bị xoá thuộc về **ông** để cây dẫn xuất không mồ côi (MDB kiểu seL4).
    pub fn delete(&mut self, slot: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(slot.cnode, slot.slot)?;
        // MDB seL4 `cteDelete`: con của cap bị xoá thuộc về ông (tổ tiên sống gần nhất),
        // nếu không cây dẫn xuất sẽ mồ côi và Revoke của ông không còn thấy cháu.
        let mut new_parent = self.cap(cap)?.parent;
        while let Some(p) = new_parent {
            if p != 0 && (p as usize) <= self.caps.len() && self.caps[p as usize - 1].alive {
                break;
            }
            new_parent = if p != 0 && (p as usize) <= self.caps.len() {
                self.caps[p as usize - 1].parent
            } else {
                None
            };
        }
        for c in &mut self.caps {
            if c.alive && c.parent == Some(cap) {
                c.parent = new_parent;
            }
        }
        let (obj, ty) = {
            let c = self.cap(cap)?;
            (c.obj, c.ty)
        };
        self.clear_slot_of(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.alive = false;
            c.parent = None;
        }
        // **KT-25** — cap Reply bị xoá ⇒ caller đang `BlockedOnReply` được đánh thức
        // `Cancelled` (D2-53 Q8, `.scn` 07).
        if ty == ObjType::Reply {
            self.cancel_reply_of_cap(obj)?;
        }
        // **KT-25** — `Delete` cũng làm cap chết: Reply cap chết còn nằm trong Recv waiter của
        // Endpoint phải được coi như "không cấp Reply" (Q7(b)), nếu không `Call` sau đó gặp cap
        // chết và trả **cứng** `InvalidCapability` (message không giao) — `.scn` 07 tạo đúng
        // trạng thái này. **Không** quét toàn bộ ref khác ở đây: `fault_ep`/`cspace`... có ngữ
        // nghĩa riêng (vd KT-18: `Exit` với `fault_ep` đã chết ⇒ `InvalidCapability`).
        if ty == ObjType::Reply {
            self.clear_dead_reply_in_waiters();
        }
        // **KT-27** — cap Notification bị xoá ⇒ gỡ binding SC↔Notification đang trỏ **cap này**
        // (không để `SchedData.bound` trỏ cap chết, `Notification.bound_sc` mồ côi).
        if ty == ObjType::Notification {
            self.drop_sched_binding_to_cap(cap, obj)?;
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-27** — gỡ binding `SchedContext.bound` đang trỏ `cap` (Notification vừa chết):
    /// SC về `bound = None`; `Notification.bound_sc` trỏ SC đó cũng xoá (1:1 hai chiều).
    pub(super) fn drop_sched_binding_to_cap(
        &mut self,
        cap: CapId,
        obj: ObjId,
    ) -> Result<(), KError> {
        for sc in self.live_sched_contexts() {
            let bound = self.body(sc, |k| match k {
                Kind::SchedContext(s) => Ok(s.bound),
                _ => Err(KError::WrongObjectType),
            })?;
            if bound != Some(cap) {
                continue;
            }
            self.body_mut(sc, |k| match k {
                Kind::SchedContext(s) => {
                    s.bound = None;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            self.body_mut(obj, |k| match k {
                Kind::Notification(n) => {
                    if n.bound_sc == Some(sc) {
                        n.bound_sc = None;
                    }
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
        }
        Ok(())
    }

    /// `CNode.Revoke`: xoá cap con/cháu của cap trong `slot` (hoặc mọi cap của CNode khi
    /// `slot = None`). Revoke cap Untyped finalise mọi object trong Untyped và đặt
    /// watermark về 0 (thiết kế §4.2). Trả số cap bị xoá.
    pub fn revoke(&mut self, cnode: ObjId, slot: Option<usize>) -> Result<usize, KError> {
        let targets: Vec<CapId> = {
            let slots = self.cnode_slots_of(cnode)?;
            match slot {
                Some(s) => {
                    if s >= slots.len() {
                        return Err(KError::RangeError);
                    }
                    match slots[s] {
                        None => return Err(KError::SlotEmpty),
                        Some(cap) => vec![cap],
                    }
                }
                None => slots.iter().flatten().copied().collect(),
            }
        };
        let mut deleted = 0;
        for target in targets {
            // Cap đích có thể đã bị xoá bởi lần revoke trước (cap lồng trong cùng CNode).
            if !self.cap(target)?.alive {
                continue;
            }
            deleted += self.revoke_children(target)?;
        }
        self.sched_after_op(true);
        Ok(deleted)
    }

    pub(super) fn cap(&self, cap: CapId) -> Result<&Cap, KError> {
        if cap == 0 || cap as usize > self.caps.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(&self.caps[cap as usize - 1])
    }

    /// Ảnh chụp slot của CNode (clone — body nằm trong arena, closure không trả tham chiếu).
    pub(super) fn cnode_slots_of(&self, cnode: ObjId) -> Result<Vec<Option<CapId>>, KError> {
        self.body(cnode, |k| match k {
            Kind::CNode { slots } => Ok(slots.clone()),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<Option<CapId>, KError> {
        let (alive, slots) = {
            let m = self.meta(cnode)?;
            (m.alive, m.slots)
        };
        if !alive {
            let n = slots.ok_or(KError::WrongObjectType)?;
            return if slot < n {
                Ok(None)
            } else {
                Err(KError::RangeError)
            };
        }
        self.body(cnode, |k| match k {
            Kind::CNode { slots } => slots.get(slot).copied().ok_or(KError::RangeError),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn cnode_set_slot(
        &mut self,
        cnode: ObjId,
        slot: usize,
        cap: Option<CapId>,
    ) -> Result<(), KError> {
        self.body_mut(cnode, |k| match k {
            Kind::CNode { slots } => match slots.get_mut(slot) {
                Some(s) => {
                    *s = cap;
                    Ok(())
                }
                None => Err(KError::RangeError),
            },
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn check_cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<(), KError> {
        let n = self.cnode_slot_count(cnode)?;
        if slot >= n {
            return Err(KError::RangeError);
        }
        Ok(())
    }

    pub(super) fn require_cap_type(&self, cap: CapId, ty: ObjType) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if !c.alive {
            return Err(KError::InvalidCapability);
        }
        if c.ty != ty {
            return Err(KError::WrongObjectType);
        }
        Ok(c.obj)
    }

    pub(super) fn require_type(&self, slot: SlotRef, ty: ObjType) -> Result<ObjId, KError> {
        self.require_cap_type(self.cap_at(slot.cnode, slot.slot)?, ty)
    }

    pub(super) fn clear_slot_of(&mut self, cap: CapId) {
        let slot = self
            .caps
            .get_mut(cap as usize - 1)
            .and_then(|c| c.slot.take());
        if let Some(SlotRef { cnode, slot: s }) = slot {
            // Object có thể đã bị đánh dấu chết nhưng body còn sống trong lúc revoke;
            // lỗi `Dead` (nếu có) được bỏ qua — slot của CNode chết đã được xoá trắng trước.
            let _ = self.body_mut(cnode, |k| {
                if let Kind::CNode { slots } = k {
                    if slots.get(s).copied().flatten() == Some(cap) {
                        slots[s] = None;
                    }
                }
                Ok(())
            });
        }
    }

    /// Xoá cap con/cháu của `root`; nếu `root` là cap Untyped thì finalise object + reset
    /// arena của cả cây Untyped (watermark về 0, thiết kế §4.2 — KT-16: arena `reset`).
    pub(super) fn revoke_children(&mut self, root: CapId) -> Result<usize, KError> {
        if !self.cap(root)?.alive {
            return Err(KError::InvalidCapability);
        }
        let mut doomed: Vec<CapId> = Vec::new();
        for i in 1..=self.caps.len() as CapId {
            if !self.caps[i as usize - 1].alive || i == root {
                continue;
            }
            let mut parent = self.caps[i as usize - 1].parent;
            while let Some(p) = parent {
                if p == root {
                    doomed.push(i);
                    break;
                }
                parent = self.caps[p as usize - 1].parent;
            }
        }

        let root_ty = self.cap(root)?.ty;
        let root_obj = self.cap(root)?.obj;
        if root_ty == ObjType::Untyped {
            let u_idx = self.object_index(root_obj)?;
            let mut dead_objs: Vec<ObjId> = Vec::new();
            // Untyped lồng nhau: finalise **cả cây con** (object của Untyped con cũng chết,
            // không để cap trỏ tới object mồ côi — review Agent 0 mục 3).
            self.collect_untyped_tree(u_idx, &mut dead_objs);
            // **KT-36** — Frame còn mapping sống thì **không** được chết. Kiểm **trước** mọi
            // thay đổi để `Revoke` là atomic: trả `HasChildren` và để nguyên cả cây (không im
            // lặng bỏ mapping — bất biến "frame chết ⇒ `maps = mapped_w = mapped_x = 0`").
            // `check_frames` giữ cùng bất biến này như lưới an toàn thứ hai.
            for id in &dead_objs {
                let m = &self.objects[*id as usize - 1];
                if m.ty == ObjType::Frame && m.frame.is_some_and(|f| !f.counters_zero()) {
                    return Err(KError::HasChildren);
                }
                // **KT-37** — PageTable còn PTE/đang attach cũng không được chết: "không im
                // lặng bỏ cây/mapping" (KT-37 DoD); kiểm **trước** mọi thay đổi để atomic.
                if m.ty == ObjType::PageTable {
                    let Some(pit) = m.pt else {
                        return Err(KError::IllegalOperation);
                    };
                    let Some(d) = self.page_tables.get(pit).and_then(|d| d.as_ref()) else {
                        return Err(KError::IllegalOperation);
                    };
                    if !d.is_pristine() {
                        return Err(KError::HasChildren);
                    }
                }
                // **KT-38** — VSpace còn root chưa tinh khiết cũng không được chết (cùng luật
                // "không im lặng bỏ mapping"); root của nó cũng nằm trong `dead_objs` nên đã bị
                // kiểm ở trên, đây là lưới thứ hai nếu root ở Untyped khác trong tương lai.
                if m.ty == ObjType::VSpace {
                    let Some(sidx) = m.space else {
                        return Err(KError::IllegalOperation);
                    };
                    let Some(d) = self.vspaces.get(sidx).and_then(|d| d.as_ref()) else {
                        return Err(KError::IllegalOperation);
                    };
                    if let Ok(ridx) = self.object_index(d.root) {
                        let rm = &self.objects[ridx];
                        if rm.alive && rm.ty == ObjType::PageTable {
                            if let Some(rd) = rm
                                .pt
                                .and_then(|p| self.page_tables.get(p))
                                .and_then(|x| x.as_ref())
                            {
                                if !rd.is_pristine() {
                                    return Err(KError::HasChildren);
                                }
                            }
                        }
                    }
                }
            }
            // Finalise CNode (seL4 `finaliseCNode`): cap nằm trong slot của CNode chết phải chết
            // theo **cả cây con của nó**, kể cả khi object đích còn sống (object ngoài Untyped).
            let mut cnode_caps: Vec<CapId> = Vec::new();
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty != ObjType::CNode {
                    continue;
                }
                if let Ok(slots) = self.cnode_slots_of(*id) {
                    cnode_caps.extend(slots.iter().flatten().copied());
                }
            }
            for cap in cnode_caps {
                if self.cap(cap)?.alive {
                    self.collect_subtree(cap, &mut doomed);
                }
            }
            // Slot của CNode chết không được giữ cap nào (kể cả cap đã chết từ trước).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty != ObjType::CNode {
                    continue;
                }
                let _ = self.body_mut(*id, |k| {
                    if let Kind::CNode { slots } = k {
                        slots.fill(None);
                    }
                    Ok(())
                });
            }
            // KT-18 (DoD 9): finalise TCB **trước** khi object chết — gỡ hàng chờ, `Dead`,
            // trả `sched_context` qua hook, ghi log (thiết kế §4.3 bảng Finalise).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::Tcb {
                    let _ = self.finalise_tcb(*id);
                }
            }
            // KT-24: message **người dùng** còn trong Endpoint sắp bị finalise phải đếm
            // `ipc_dropped` — nếu không, bất biến bảo toàn message thấy "mất" message khi
            // Revoke Untyped chứa Endpoint (D2-25: message bị huỷ là hợp lệ).
            let mut dropped_msgs = 0u64;
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty != ObjType::Endpoint {
                    continue;
                }
                if let Ok(n) = self.body(*id, |k| match k {
                    Kind::Endpoint(e) => {
                        Ok(e.send.iter().filter(|q| q.msg.is_some()).count() as u64)
                    }
                    _ => Err(KError::WrongObjectType),
                }) {
                    dropped_msgs += n;
                }
            }
            self.ipc_dropped += dropped_msgs;
            // **KT-25** — Reply nằm trong Untyped bị revoke cũng "bị xoá": caller đang chờ được
            // đánh thức `Cancelled` (Q8). Phải làm **trước** khi body bị free (không đọc được sau).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::Reply {
                    self.cancel_reply(*id)?;
                }
            }
            // **KT-64a (D2-89/D2-90)** — anti-hang: waiter `Recv`/`Notification` **và** sender
            // `BlockedOnSend` trên object sắp chết được đánh thức `Cancelled + Ready` (core KT-63
            // đã làm cho waiter; đây là chỗ sim khớp chuẩn và **bỏ loại trừ parity**). Làm **trước**
            // khi body về free-list để còn đọc hàng gửi/waiter; message bị huỷ đã đếm `ipc_dropped`
            // ở vòng `dropped_msgs` phía trên (D2-90: drop **có đếm**).
            for id in &dead_objs {
                self.finalise_ipc_waiters(*id);
            }
            // **KT-36** — finalise Frame trước khi object chết (bộ đếm đã kiểm bằng 0 ở trên;
            // đánh dấu `live = false` để `frame_meta` sau chết đọc được trạng thái đúng).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::Frame {
                    self.finalise_frame(*id)?;
                }
            }
            // **KT-38** — finalise VSpace **trước** PageTable root: phát lệnh flush theo asid,
            // gỡ hart khỏi `hart_vspace`, tombstone dữ liệu VSpace (ASID về hưu cho hết thế hệ).
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::VSpace {
                    self.finalise_vspace(*id)?;
                }
            }
            // **KT-37** — finalise PageTable trước khi object chết (đã kiểm tinh khiết ở trên):
            // dữ liệu `page_tables` chuyển tombstone `None`, không rò 4 KiB PTE.
            for id in &dead_objs {
                if self.objects[*id as usize - 1].ty == ObjType::PageTable {
                    self.finalise_page_table(*id)?;
                }
            }
            // Đánh dấu chết + **finalise body trong objmem** (KT-16; `free` guard `pa`).
            for id in &dead_objs {
                let (loc, body) = {
                    let m = &mut self.objects[*id as usize - 1];
                    m.alive = false;
                    (m.loc, m.body)
                };
                self.store_mut(loc).free(body);
            }
            // Reset arena của **mọi** Untyped trong cây: watermark của mô hình = `used()`;
            // chỉ reset sau khi mọi body trong vùng đã finalise.
            let mut regions: Vec<usize> = Vec::new();
            for id in &dead_objs {
                if let Some(r) = self.objects[*id as usize - 1].region {
                    regions.push(r);
                }
            }
            if let Some(r) = self.objects[u_idx].region {
                regions.push(r);
            }
            for r in regions {
                self.regions[r].reset();
            }
            // **KT-36** — cửa sổ PA của Untyped (và Untyped con) về watermark 0, song song với
            // watermark arena ở trên: Frame trong vùng đã finalise, cấp lại bắt đầu ở `base`.
            for id in &dead_objs {
                if let Some(area) = self.objects[*id as usize - 1].frame_area {
                    self.objects[*id as usize - 1].frame_area = Some(FrameArea::new(area.base));
                }
            }
            if let Some(area) = self.objects[u_idx].frame_area {
                self.objects[u_idx].frame_area = Some(FrameArea::new(area.base));
            }
            for i in 1..=self.caps.len() as CapId {
                if self.caps[i as usize - 1].alive
                    && dead_objs.contains(&self.caps[i as usize - 1].obj)
                {
                    doomed.push(i);
                }
            }
        }

        doomed.sort_unstable();
        doomed.dedup();
        // **KT-25** — Reply mất cap cuối qua `Revoke` cũng phải huỷ đường reply (Q8): ghi lại
        // object Reply của các cap sắp chết để kiểm sau khi xoá xong.
        let mut reply_objs: Vec<ObjId> = Vec::new();
        for cap in &doomed {
            let c = &self.caps[*cap as usize - 1];
            if c.alive && c.ty == ObjType::Reply && !reply_objs.contains(&c.obj) {
                reply_objs.push(c.obj);
            }
        }
        let mut deleted = 0;
        for cap in doomed {
            if self.caps[cap as usize - 1].alive {
                self.clear_slot_of(cap);
                self.caps[cap as usize - 1].alive = false;
                deleted += 1;
            }
        }
        for obj in reply_objs {
            self.cancel_reply_of_cap(obj)?;
        }
        self.purge_dead_refs();
        Ok(deleted)
    }

    /// Gom `root` và mọi con/cháu còn sống của nó (theo cạnh cha) vào `out`.
    pub(super) fn collect_subtree(&self, root: CapId, out: &mut Vec<CapId>) {
        for i in 1..=self.caps.len() as CapId {
            if !self.caps[i as usize - 1].alive {
                continue;
            }
            let mut cur = Some(i);
            while let Some(p) = cur {
                if p == root {
                    out.push(i);
                    break;
                }
                if p == 0 || p as usize > self.caps.len() {
                    break;
                }
                cur = self.caps[p as usize - 1].parent;
            }
        }
    }

    /// Gom mọi object sống nằm trong `u_idx`, đệ quy qua Untyped con.
    pub(super) fn collect_untyped_tree(&self, u_idx: usize, out: &mut Vec<ObjId>) {
        for (i, m) in self.objects.iter().enumerate() {
            if m.alive && m.untyped == Some(u_idx) {
                out.push((i as u32) + 1);
                if m.ty == ObjType::Untyped {
                    self.collect_untyped_tree(i, out);
                }
            }
        }
    }
}
