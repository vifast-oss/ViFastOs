// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `machine/lifecycle.rs` — **vòng đời object/cap** của [`Machine`] (KT-58): `retype`,
//! CSpace (`copy`/`mint`/`move_cap`/`delete`/`revoke`/`revoke_children`), `push_meta`/`push_cap`
//! và các bộ cấp/phát body TCB/SchedContext/Endpoint/Reply/Notification; port từ oracle.

use super::*;
use crate::objstate::{EndpointData, NotificationData, ReplyData};
use crate::store::{KernelStore, SlotWindow};

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    // ------------------------------------------------------------- Retype

    /// Cấp object từ Untyped theo `type/size_bits/flags` (spec `Untyped.Retype`).
    ///
    /// Trả `CapId` cho harness (ABI `returns = []`); cap là con của cap nguồn, **kế thừa**
    /// quyền cap nguồn. Mọi lỗi **nguyên tử**: không đốt byte/cap/object.
    pub fn retype(
        &mut self,
        src: SlotRef,
        ty: ObjType,
        size_bits: u32,
        flags: u32,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (u_idx, u_region, u_size, src_rights) = {
            let c = self.cap(src_cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let idx = self.object_index(c.obj)?;
            let m = &self.objects[idx];
            if m.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let region = m.region.map(usize::from).ok_or(KError::WrongObjectType)?;
            (idx, region, self.regions[region].capacity, c.rights)
        };
        if src_rights & RIGHT_WRITE == 0 {
            return Err(KError::InsufficientRights);
        }
        if flags & FLAG_GUEST != 0 {
            return Err(if ty == ObjType::VSpace {
                KError::NotSupported
            } else {
                KError::InvalidArgument
            });
        }
        // **D2-242 nhóm 9 (9B)** — ba object VM đã `stability = core`/`frozen` (nhãn ABI) nhưng
        // đường `Retype` của **mô hình** chưa cài (chỉ object layer của sim/plat cài Frame/
        // PageTable); giữ đúng mã ABI cũ `NotSupported` cho tới khi mở chữ ký mới.
        if matches!(ty, ObjType::VSpace | ObjType::PageTable | ObjType::Frame) {
            return Err(KError::NotSupported);
        }
        if !ty.is_core() {
            return Err(KError::NotSupported);
        }

        let (len, align, child_slots) = match ty {
            ObjType::Untyped => {
                if !(12..=30).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let len = 1usize << size_bits;
                if len > u_size {
                    return Err(KError::NotEnoughMemory);
                }
                // KT-17: arena của sim chỉ bảo đảm căn `ARENA_ALIGN` (4096 B).
                if len > ARENA_ALIGN {
                    return Err(KError::Alignment);
                }
                (len, len, 0)
            }
            ObjType::CNode => {
                if !(4..=20).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let slots = 1usize << size_bits;
                (ObjType::cnode_bytes(slots), CNODE_BASE_BYTES, slots)
            }
            _ => {
                let fixed = ty.fixed_size().ok_or(KError::InvalidArgument)?;
                if size_bits != 0 {
                    if size_bits > 30 {
                        return Err(KError::InvalidArgument);
                    }
                    let requested = 1usize << size_bits;
                    if requested < fixed {
                        return Err(KError::Alignment);
                    }
                    if requested > fixed {
                        return Err(KError::InvalidArgument);
                    }
                }
                (fixed, DEFAULT_ALIGN_BYTES, 0)
            }
        };

        // Hạn mức bảng tĩnh (khác sim chỉ có hạn arena): kiểm **trước** mọi thay đổi để
        // `NotEnoughMemory` nguyên tử.
        let need_region = ty == ObjType::Untyped;
        if self.n_objects >= MAX_OBJECTS
            || self.n_caps >= MAX_CAPS
            || (need_region && self.n_regions >= MAX_UNTYPEDS)
            || (ty == ObjType::Tcb && !self.tcb_body_available())
            || (ty == ObjType::SchedContext && !self.sc_body_available())
            || (ty == ObjType::Endpoint && !self.endpoint_body_available())
            || (ty == ObjType::Reply && !self.reply_body_available())
            || (ty == ObjType::Notification && !self.notification_body_available())
        {
            return Err(KError::NotEnoughMemory);
        }
        // **KT-64** — cửa sổ byte slot phải nằm trong arena (vùng A) **trước** khi cấp: kiểm
        // bằng `Region::probe` (không đổi watermark) ⇒ lỗi nguyên tử, không đọc ngoài mảng.
        let slot_win = if ty == ObjType::CNode {
            let probe = self.regions[u_region]
                .probe(len, align)
                .ok_or(KError::NotEnoughMemory)?;
            Some(arena_slot_base(u_region, probe, child_slots).ok_or(KError::NotEnoughMemory)?)
        } else {
            None
        };
        let offset = self.regions[u_region]
            .alloc(len, align)
            .ok_or(KError::NotEnoughMemory)?;
        let region = if ty == ObjType::Untyped {
            let base = self.regions[u_region]
                .base
                .checked_add(offset as u64)
                .ok_or(KError::InvalidArgument)?;
            let idx = self.n_regions;
            self.regions[idx] = Region::new(base, len);
            self.n_regions += 1;
            Some(idx)
        } else {
            None
        };
        let (slot_base, slot_len) = match slot_win {
            Some(base) => {
                // Cửa sổ mới (hoặc tái dùng sau `Revoke`) phải **trắng** — như CNode mới của sim.
                self.slots.fill_zero(base, child_slots);
                (base, child_slots)
            }
            None => (0, 0),
        };
        // Đã kiểm sức chứa body ở trên ⇒ chỉ có thể thành công; lỗi nội bộ là bất biến vỡ.
        let body = match ty {
            ObjType::Tcb => self.alloc_tcb_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::SchedContext => self.alloc_sc_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::Endpoint => self.alloc_endpoint_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::Reply => self.alloc_reply_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::Notification => self
                .alloc_notification_body()
                .ok_or(KError::NotEnoughMemory)?,
            _ => BodyRef::NONE,
        };
        let obj = self.push_meta(
            ty,
            Some(u_idx),
            offset,
            len,
            region,
            if ty == ObjType::CNode {
                Some(child_slots)
            } else {
                None
            },
            slot_base,
            slot_len,
            body,
        );
        Ok(self.push_cap(obj, ty, Some(src_cap), src_rights, None))
    }

    // ------------------------------------------------------------- CSpace

    /// `CNode.Copy`: tạo cap con ở đích; đích phải rỗng. Giữ quyền + badge nguồn.
    pub fn copy(&mut self, src: SlotRef, dst: SlotRef) -> Result<CapId, KError> {
        self.copy_rights(src, dst, None)
    }

    /// `CNode.Copy` với `rights?`: `None` = giữ quyền nguồn; `Some(r)` phải là tập con.
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

    /// `CNode.Mint`: như Copy nhưng thu hẹp quyền và đặt badge (chỉ Endpoint/Notification).
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

    /// Tạo cap con của `src_cap` rồi gắn vào `dst` — đã kiểm nguồn/quyền trước khi cấp.
    fn derive(
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

    /// `CNode.Move` (KT-68): chuyển cap nguồn sang `dst`, **xoá nguồn**.
    ///
    /// Ngữ nghĩa **D2-44**: `dst` đã có cap thì cap đích bị `delete` trước (con của cap đích
    /// được reparent lên ông — MDB kiểu seL4), rồi cap nguồn rời slot cũ sang `dst`; **không**
    /// tạo cap mới (cùng `CapId`, giữ nguyên cha MDB, quyền, badge). `src == dst` ⇒
    /// `IllegalOperation` (không có việc gì để làm, không được tự xoá).
    pub fn move_cap(&mut self, src: SlotRef, dst: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(src.cnode, src.slot)?;
        self.check_cnode_slot(dst.cnode, dst.slot)?;
        if src == dst {
            return Err(KError::IllegalOperation);
        }
        if self.cnode_slot(dst.cnode, dst.slot)?.is_some() {
            self.delete(dst)?;
        }
        self.clear_slot_of(cap);
        self.cnode_set_slot(dst.cnode, dst.slot, Some(cap))?;
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(dst);
        }
        Ok(())
    }

    /// `CNode.Delete`: xoá cap trong slot (object không bị finalise — chỉ Revoke mới).
    ///
    /// Con của cap bị xoá thuộc về **ông** (tổ tiên sống gần nhất) — MDB kiểu seL4.
    pub fn delete(&mut self, slot: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(slot.cnode, slot.slot)?;
        let (cap_ty, cap_obj) = {
            let c = self.cap(cap)?;
            (c.ty, c.obj)
        };
        let mut new_parent = self.cap(cap)?.parent;
        while let Some(p) = new_parent {
            if p != 0 && (p as usize) <= self.n_caps && self.caps[p as usize - 1].alive {
                break;
            }
            new_parent = if p != 0 && (p as usize) <= self.n_caps {
                self.caps[p as usize - 1].parent
            } else {
                None
            };
        }
        for c in &mut self.caps[..self.n_caps] {
            if c.alive && c.parent == Some(cap) {
                c.parent = new_parent;
            }
        }
        self.clear_slot_of(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.alive = false;
            c.parent = None;
        }
        // **KT-63** — cap Reply bị xoá ⇒ caller đang `BlockedOnReply` được đánh thức
        // `Cancelled` + `Ready` (D2-53 Q8, như `Machine::delete` của sim); waiter Recv còn giữ
        // cap Reply **đã chết** được coi như "không cấp Reply" (Q7b) thay vì lỗi cứng.
        if cap_ty == ObjType::Reply && self.object_alive(cap_obj) {
            self.cancel_reply_object(cap_obj);
        }
        if cap_ty == ObjType::Reply {
            self.clear_dead_reply_in_waiters();
        }
        // **KT-64** — cap Notification bị xoá ⇒ gỡ binding `SchedContext.bound` đang trỏ **cap
        // này** (không để `bound` trỏ cap chết, `bound_sc` mồ côi) — như sim `delete`.
        if cap_ty == ObjType::Notification && self.object_alive(cap_obj) {
            self.drop_sched_binding_to_cap(cap, cap_obj);
        }
        Ok(())
    }

    /// **KT-67a (reviewer L2)** — bỏ một cap **chưa nằm trong slot** (rollback nội bộ khi cài
    /// cap đại diện thất bại): đánh dấu cap chết + reparent con về ông gần nhất, **không** đụng
    /// CNode. Từ chối cap đang ở slot (dùng `delete`) hoặc cap đã chết.
    pub fn drop_root_cap(&mut self, cap: CapId) -> Result<(), KError> {
        let c = self.cap(cap)?;
        if !c.alive || c.slot.is_some() {
            return Err(KError::IllegalOperation);
        }
        let mut new_parent = c.parent;
        while let Some(p) = new_parent {
            if p != 0 && (p as usize) <= self.n_caps && self.caps[p as usize - 1].alive {
                break;
            }
            new_parent = if p != 0 && (p as usize) <= self.n_caps {
                self.caps[p as usize - 1].parent
            } else {
                None
            };
        }
        for other in &mut self.caps[..self.n_caps] {
            if other.alive && other.parent == Some(cap) {
                other.parent = new_parent;
            }
        }
        if let Some(entry) = self.caps.get_mut(cap as usize - 1) {
            entry.alive = false;
            entry.parent = None;
        }
        Ok(())
    }

    /// `CNode.Revoke`: xoá con/cháu của cap trong `slot` (hoặc mọi cap của CNode khi
    /// `slot = None`). Revoke cap Untyped finalise mọi object trong cây Untyped và đặt
    /// watermark về 0. Trả số cap bị xoá.
    pub fn revoke(&mut self, cnode: ObjId, slot: Option<usize>) -> Result<usize, KError> {
        // D2-83/KT-58: sim đọc **body** CNode trước (`cnode_slots_of`), nên CNode đã finalise
        // trả `InvalidCapability` (body `Dead`), không đọc metadata. `cnode_slot_count` thì
        // vẫn trả số slot của metadata (API truy vấn) — `Revoke` phải đi cửa body như sim.
        let n = {
            let m = self.meta(cnode)?;
            if !m.alive {
                return Err(KError::InvalidCapability);
            }
            if m.ty != ObjType::CNode {
                return Err(KError::WrongObjectType);
            }
            m.slot_len
        };
        let mut deleted = 0;
        match slot {
            Some(s) => {
                if s >= n {
                    return Err(KError::RangeError);
                }
                match self.cnode_slot(cnode, s)? {
                    None => Err(KError::SlotEmpty),
                    Some(cap) => {
                        if self.cap(cap)?.alive {
                            deleted += self.revoke_children(cap)?;
                        }
                        Ok(deleted)
                    }
                }
            }
            None => {
                for s in 0..n {
                    if let Some(cap) = self.cnode_slot(cnode, s)? {
                        if self.cap(cap)?.alive {
                            deleted += self.revoke_children(cap)?;
                        }
                    }
                }
                Ok(deleted)
            }
        }
    }

    /// Xoá cap con/cháu của `root`; nếu `root` là cap Untyped thì finalise object + reset
    /// arena của cả cây Untyped (watermark về 0, thiết kế §4.2).
    fn revoke_children(&mut self, root: CapId) -> Result<usize, KError> {
        if !self.cap(root)?.alive {
            return Err(KError::InvalidCapability);
        }
        let mut doomed = [0u32; MAX_CAPS];
        let mut nd = 0usize;
        for i in 1..=self.n_caps as CapId {
            if !self.caps[i as usize - 1].alive || i == root {
                continue;
            }
            let mut parent = self.caps[i as usize - 1].parent;
            let mut steps = 0usize;
            while let Some(p) = parent {
                if p == root {
                    push_id(&mut doomed, &mut nd, i);
                    break;
                }
                if p == 0 || p as usize > self.n_caps {
                    break;
                }
                parent = self.caps[p as usize - 1].parent;
                steps += 1;
                if steps > self.n_caps {
                    break;
                }
            }
        }

        let root_ty = self.cap(root)?.ty;
        let root_obj = self.cap(root)?.obj;
        if root_ty == ObjType::Untyped {
            let u_idx = self.object_index(root_obj)?;
            let mut dead = [0u32; MAX_OBJECTS];
            let mut ndead = 0usize;
            self.collect_untyped_tree(u_idx, &mut dead, &mut ndead);
            // Finalise CNode (seL4 `finaliseCNode`): cap nằm trong slot của CNode chết phải
            // chết theo **cả cây con của nó**, kể cả khi object đích còn sống.
            for &id in &dead[..ndead] {
                if self.objects[id as usize - 1].ty != ObjType::CNode {
                    continue;
                }
                let len = self.objects[id as usize - 1].slot_len;
                for s in 0..len {
                    if let Some(cap) = self.cnode_slot(id, s)? {
                        if self.cap(cap)?.alive {
                            self.collect_subtree(cap, &mut doomed, &mut nd);
                        }
                    }
                }
            }
            // Slot của CNode chết không giữ cap nào.
            for &id in &dead[..ndead] {
                if self.objects[id as usize - 1].ty == ObjType::CNode {
                    let (base, len) = {
                        let m = &self.objects[id as usize - 1];
                        (m.slot_base, m.slot_len)
                    };
                    self.slots.fill_zero(base, len);
                }
            }
            // KT-18/KT-59: finalise TCB **trước** khi object chết (gỡ ready queue, `Dead`,
            // trả `sched_context` qua hook, ghi log) — như sim `revoke_children`.
            for &id in &dead[..ndead] {
                if self.objects[id as usize - 1].ty == ObjType::Tcb {
                    let _ = self.finalise_tcb(id);
                }
            }
            // **KT-63** — finalise IPC của object chết **trước** khi body về free-list:
            // Reply huỷ caller (`Cancelled + Ready`), Endpoint đánh thức Recv waiter + đếm
            // message bị huỷ (`ipc_dropped`), Notification đánh thức waiter (chống treo).
            for &id in &dead[..ndead] {
                match self.objects[id as usize - 1].ty {
                    ObjType::Reply => {
                        self.cancel_reply_object(id);
                    }
                    ObjType::Endpoint => self.finalise_endpoint_body(id),
                    ObjType::Notification => {
                        // **KT-64** — gỡ `SchedContext.bound` trỏ notification sắp chết.
                        self.drop_sched_binding_to_obj(id);
                        self.finalise_notification_body(id);
                    }
                    _ => {}
                }
            }
            // Đánh dấu chết + trả word slot pool + trả body TCB/SchedContext về free-list.
            for &id in &dead[..ndead] {
                let idx = id as usize - 1;
                let body = self.objects[idx].body;
                {
                    let m = &mut self.objects[idx];
                    m.alive = false;
                    m.body.alive = false;
                }
                match body.kind {
                    BODY_TCB => self.free_tcb_body(body.idx),
                    BODY_SC => self.free_sc_body(body.idx),
                    BODY_ENDPOINT => self.free_endpoint_body(body.idx),
                    BODY_REPLY => self.free_reply_body(body.idx),
                    BODY_NOTIFICATION => self.free_notification_body(body.idx),
                    _ => {}
                }
                let (base, len) = (self.objects[idx].slot_base, self.objects[idx].slot_len);
                if len > 0 {
                    // Không còn free-list: byte slot trả lại **cùng** byte arena (`Region::reset`
                    // ngay dưới); chỉ xoá cửa sổ để không ai đọc rác.
                    self.slots.fill_zero(base, len);
                }
            }
            // Reset arena của **mọi** Untyped trong cây + Untyped gốc.
            for &id in &dead[..ndead] {
                if let Some(r) = self.objects[id as usize - 1].region {
                    self.regions[r as usize].reset();
                }
            }
            if let Some(r) = self.objects[u_idx].region {
                self.regions[r as usize].reset();
            }
            // Cap còn sống trỏ object vừa chết cũng phải chết (không cap nào trỏ object chết).
            for i in 1..=self.n_caps as CapId {
                if self.caps[i as usize - 1].alive
                    && dead[..ndead].contains(&self.caps[i as usize - 1].obj)
                {
                    push_id(&mut doomed, &mut nd, i);
                }
            }
        }

        sort_dedup(&mut doomed, &mut nd);
        // **KT-63** — Reply mất cap cuối qua `Revoke` cũng phải huỷ đường reply (Q8): ghi lại
        // object Reply của các cap sắp chết để huỷ **sau** khi xoá (như `revoke_children` sim).
        let mut reply_objs = [0u32; MAX_CAPS];
        let mut n_reply = 0usize;
        for &cap in &doomed[..nd] {
            let c = self.caps[cap as usize - 1];
            if c.alive && c.ty == ObjType::Reply && !reply_objs[..n_reply].contains(&c.obj) {
                reply_objs[n_reply] = c.obj;
                n_reply += 1;
            }
        }
        let mut deleted = 0;
        for &cap in &doomed[..nd] {
            if self.caps[cap as usize - 1].alive {
                self.clear_slot_of(cap);
                self.caps[cap as usize - 1].alive = false;
                deleted += 1;
            }
        }
        for &obj in &reply_objs[..n_reply] {
            self.cancel_reply_object(obj);
        }
        self.clear_dead_reply_in_waiters();
        Ok(deleted)
    }

    // ------------------------------------------------------------- nội bộ

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_meta(
        &mut self,
        ty: ObjType,
        untyped: Option<usize>,
        offset: usize,
        len: usize,
        region: Option<usize>,
        slots_meta: Option<usize>,
        slot_base: usize,
        slot_len: usize,
        body: BodyRef,
    ) -> ObjId {
        assert!(
            self.n_objects < MAX_OBJECTS,
            "vifast-core: hết MAX_OBJECTS ({MAX_OBJECTS})"
        );
        // **KT-312** — thu hẹp kiểu **có chứng minh**: ba trường chỉ số chỉ nhận giá trị trong
        // miền của bảng tương ứng (`untyped` = chỉ số **object** < `MAX_OBJECTS`; `region` =
        // chỉ số arena < `MAX_UNTYPEDS`; `slots_meta` = số slot CNode ≤ `MAX_SLOT_WORDS`).
        // `debug_assert` giữ bất biến này khi phát triển; bản release dùng `as` (không thể tràn
        // vì mọi caller đã bị chặn bởi `MAX_OBJECTS`/`MAX_UNTYPEDS`/`HARNESS_SLOT_WORDS`).
        let untyped = untyped.map(|u| {
            debug_assert!(u < MAX_OBJECTS, "untyped {u} >= MAX_OBJECTS");
            u as u8
        });
        let region = region.map(|r| {
            debug_assert!(r < MAX_UNTYPEDS, "region {r} >= MAX_UNTYPEDS");
            r as u8
        });
        let slots_meta = slots_meta.map(|n| {
            debug_assert!(n <= MAX_SLOT_WORDS, "slots_meta {n} > MAX_SLOT_WORDS");
            n as u16
        });
        self.objects[self.n_objects] = Meta {
            ty,
            alive: true,
            untyped,
            offset,
            len,
            region,
            slots_meta,
            slot_len,
            slot_base,
            body,
        };
        self.n_objects += 1;
        self.n_objects as ObjId
    }

    pub(crate) fn push_cap(
        &mut self,
        obj: ObjId,
        ty: ObjType,
        parent: Option<CapId>,
        rights: u8,
        badge: Option<u64>,
    ) -> CapId {
        assert!(
            self.n_caps < MAX_CAPS,
            "vifast-core: hết MAX_CAPS ({MAX_CAPS})"
        );
        self.caps[self.n_caps] = Cap {
            obj,
            ty,
            parent,
            slot: None,
            rights,
            badge,
            alive: true,
        };
        self.n_caps += 1;
        self.n_caps as CapId
    }

    fn tcb_body_available(&self) -> bool {
        self.n_tcb_free > 0 || self.n_tcbs < MAX_TCBS
    }

    fn sc_body_available(&self) -> bool {
        self.n_sc_free > 0 || self.n_scs < MAX_SCHED_CONTEXTS
    }

    pub(super) fn alloc_tcb_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_tcb_free > 0 {
            self.n_tcb_free -= 1;
            self.tcb_free[self.n_tcb_free] as usize
        } else if self.n_tcbs < MAX_TCBS {
            let i = self.n_tcbs;
            self.n_tcbs += 1;
            i
        } else {
            return None;
        };
        self.tcbs[idx] = TcbData::EMPTY;
        Some(BodyRef {
            kind: BODY_TCB,
            idx: idx as u16,
            alive: true,
        })
    }

    pub(super) fn alloc_sc_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_sc_free > 0 {
            self.n_sc_free -= 1;
            self.sc_free[self.n_sc_free] as usize
        } else if self.n_scs < MAX_SCHED_CONTEXTS {
            let i = self.n_scs;
            self.n_scs += 1;
            i
        } else {
            return None;
        };
        self.scs[idx] = ScData::EMPTY;
        Some(BodyRef {
            kind: BODY_SC,
            idx: idx as u16,
            alive: true,
        })
    }

    fn free_tcb_body(&mut self, idx: u16) {
        if self.n_tcb_free < MAX_TCBS {
            self.tcb_free[self.n_tcb_free] = idx;
            self.n_tcb_free += 1;
        }
    }

    fn free_sc_body(&mut self, idx: u16) {
        if self.n_sc_free < MAX_SCHED_CONTEXTS {
            self.sc_free[self.n_sc_free] = idx;
            self.n_sc_free += 1;
        }
    }

    pub(crate) fn endpoint_body_available(&self) -> bool {
        self.n_endpoint_free > 0 || self.n_endpoints < MAX_ENDPOINTS
    }

    pub(crate) fn reply_body_available(&self) -> bool {
        self.n_reply_free > 0 || self.n_replies < MAX_REPLIES
    }

    pub(crate) fn notification_body_available(&self) -> bool {
        self.n_notification_free > 0 || self.n_notifications < MAX_NOTIFICATIONS
    }

    pub(super) fn alloc_endpoint_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_endpoint_free > 0 {
            self.n_endpoint_free -= 1;
            self.endpoint_free[self.n_endpoint_free] as usize
        } else if self.n_endpoints < MAX_ENDPOINTS {
            let i = self.n_endpoints;
            self.n_endpoints += 1;
            i
        } else {
            return None;
        };
        self.endpoints[idx] = EndpointData::EMPTY;
        Some(BodyRef {
            kind: BODY_ENDPOINT,
            idx: idx as u16,
            alive: true,
        })
    }

    pub(super) fn alloc_reply_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_reply_free > 0 {
            self.n_reply_free -= 1;
            self.reply_free[self.n_reply_free] as usize
        } else if self.n_replies < MAX_REPLIES {
            let i = self.n_replies;
            self.n_replies += 1;
            i
        } else {
            return None;
        };
        self.replies[idx] = ReplyData::EMPTY;
        Some(BodyRef {
            kind: BODY_REPLY,
            idx: idx as u16,
            alive: true,
        })
    }

    pub(super) fn alloc_notification_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_notification_free > 0 {
            self.n_notification_free -= 1;
            self.notification_free[self.n_notification_free] as usize
        } else if self.n_notifications < MAX_NOTIFICATIONS {
            let i = self.n_notifications;
            self.n_notifications += 1;
            i
        } else {
            return None;
        };
        self.notifications[idx] = NotificationData::default();
        Some(BodyRef {
            kind: BODY_NOTIFICATION,
            idx: idx as u16,
            alive: true,
        })
    }

    fn free_endpoint_body(&mut self, idx: u16) {
        // Message đang chờ trong hàng của Endpoint bị finalise phải trả về pool (không rò
        // chỉ số pool sau revoke) — tương đương sim huỷ `Box` khi body chết.
        let mut q = self.endpoints[idx as usize].send;
        while let Some(m) = q.pop_front(&self.ipc_msgs) {
            self.free_ipc_msg(m);
        }
        self.endpoints[idx as usize] = EndpointData::EMPTY;
        if self.n_endpoint_free < MAX_ENDPOINTS {
            self.endpoint_free[self.n_endpoint_free] = idx;
            self.n_endpoint_free += 1;
        }
    }

    fn free_reply_body(&mut self, idx: u16) {
        self.replies[idx as usize] = ReplyData::EMPTY;
        if self.n_reply_free < MAX_REPLIES {
            self.reply_free[self.n_reply_free] = idx;
            self.n_reply_free += 1;
        }
    }

    fn free_notification_body(&mut self, idx: u16) {
        self.notifications[idx as usize] = NotificationData::default();
        if self.n_notification_free < MAX_NOTIFICATIONS {
            self.notification_free[self.n_notification_free] = idx;
            self.n_notification_free += 1;
        }
    }

    pub(crate) fn object_index(&self, obj: ObjId) -> Result<usize, KError> {
        if obj == 0 || obj as usize > self.n_objects {
            return Err(KError::InvalidCapability);
        }
        Ok(obj as usize - 1)
    }
}
