// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `machine/init.rs` — **khởi tạo/cấp phát/truy vấn** của [`Machine`] (KT-58/59):
//! `add_untyped`/`add_cnode`/`add_kernel_object`, cap gốc & truy vấn cap/object/Untyped; port
//! nguyên ngữ nghĩa từ `vifast-sim` (oracle).

use super::*;

impl Machine {
    // ------------------------------------------------------------- cấp phát

    /// Thêm Untyped **do harness cấp** (đáy `base`, `size` byte arena).
    ///
    /// Sim `add_untyped` (KT-16): arena do objmem sở hữu; ở đây arena là `[base, base+size)`
    /// **logic** — `untyped_arena()` trả đúng `(base, size)`, watermark bắt đầu 0.
    ///
    /// # Panics
    /// `size == 0`, hoặc hết bảng `MAX_UNTYPEDS`/`MAX_OBJECTS` (lỗi init của harness; sim
    /// cũng panic khi arena kernel/Untyped hết chỗ).
    pub fn add_untyped(&mut self, base: u64, size: usize) -> ObjId {
        assert!(size > 0, "Untyped phải có cỡ dương");
        assert!(
            self.n_regions < MAX_UNTYPEDS,
            "vifast-core: hết MAX_UNTYPEDS ({MAX_UNTYPEDS})"
        );
        let region = self.n_regions;
        self.regions[region] = Region::new(base, size);
        self.n_regions += 1;
        self.push_meta(
            ObjType::Untyped,
            None,
            0,
            0,
            Some(region),
            None,
            0,
            0,
            BodyRef::NONE,
        )
    }

    /// Thêm CNode **do harness cấp** (init giữ CNode gốc) — như sim `add_cnode`.
    ///
    /// # Panics
    /// `slots == 0` hoặc pool slot/bảng đầy (lỗi init).
    pub fn add_cnode(&mut self, slots: usize) -> ObjId {
        assert!(slots > 0, "CNode phải có ít nhất một slot");
        let offset = self
            .harness
            .probe(ObjType::cnode_bytes(slots), CNODE_BASE_BYTES)
            .expect("vifast-core: arena harness hết chỗ (HARNESS_SLOT_WORDS)");
        let base = harness_slot_base(offset, slots)
            .expect("vifast-core: vùng B slot hết chỗ (HARNESS_SLOT_WORDS)");
        let _ = self
            .harness
            .alloc(ObjType::cnode_bytes(slots), CNODE_BASE_BYTES)
            .expect("arena harness đã probe");
        self.slots.fill_zero(base, slots);
        self.push_meta(
            ObjType::CNode,
            None,
            0,
            0,
            None,
            Some(slots),
            base,
            slots,
            BodyRef::NONE,
        )
    }

    /// Thêm object lõi **do harness cấp** (SchedControl, TCB khởi tạo, ...).
    ///
    /// # Panics
    /// `ty` không phải object lõi, hoặc bảng đầy.
    pub fn add_kernel_object(&mut self, ty: ObjType) -> ObjId {
        assert!(ty.is_core(), "chỉ object lõi mới dựng trực tiếp");
        // Sim: `fresh_kind(CNode, 0)` cho body 1 slot dù metadata `slots = None`; giữ đúng
        // hành vi đó để `cnode_slot_count` của CNode harness-cấp là 1 (không dùng trong test).
        let (slot_base, slot_len, slots_meta) = if ty == ObjType::CNode {
            let offset = self
                .harness
                .probe(ObjType::cnode_bytes(1), CNODE_BASE_BYTES)
                .expect("vifast-core: arena harness hết chỗ (CNode harness)");
            let base = harness_slot_base(offset, 1)
                .expect("vifast-core: vùng B slot hết chỗ (CNode harness)");
            let _ = self
                .harness
                .alloc(ObjType::cnode_bytes(1), CNODE_BASE_BYTES)
                .expect("arena harness đã probe");
            self.slots.fill_zero(base, 1);
            (base, 1, None)
        } else {
            (0, 0, None)
        };
        let body = match ty {
            ObjType::Tcb => self
                .alloc_tcb_body()
                .expect("vifast-core: hết MAX_TCBS (harness)"),
            ObjType::SchedContext => self
                .alloc_sc_body()
                .expect("vifast-core: hết MAX_SCHED_CONTEXTS (harness)"),
            ObjType::Endpoint => self
                .alloc_endpoint_body()
                .expect("vifast-core: hết MAX_ENDPOINTS (harness)"),
            ObjType::Reply => self
                .alloc_reply_body()
                .expect("vifast-core: hết MAX_REPLIES (harness)"),
            ObjType::Notification => self
                .alloc_notification_body()
                .expect("vifast-core: hết MAX_NOTIFICATIONS (harness)"),
            _ => BodyRef::NONE,
        };
        self.push_meta(ty, None, 0, 0, None, slots_meta, slot_base, slot_len, body)
    }

    /// Cap gốc cho một object (không cha) — harness đặt vào CNode bằng [`Machine::put_cap`].
    ///
    /// # Panics
    /// Object không tồn tại hoặc bảng cap đầy.
    pub fn root_cap(&mut self, obj: ObjId) -> CapId {
        let ty = self
            .object_type(obj)
            .expect("root_cap: object phải tồn tại");
        self.push_cap(obj, ty, None, RIGHTS_ALL, None)
    }

    /// Đặt cap (chưa nằm trong slot nào) vào slot của CNode.
    pub fn put_cap(&mut self, cnode: ObjId, slot: usize, cap: CapId) -> Result<(), KError> {
        self.check_cnode_slot(cnode, slot)?;
        if self.cap(cap)?.slot.is_some() {
            return Err(KError::IllegalOperation);
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

    // ------------------------------------------------------------- truy vấn

    /// Cap trong slot; slot rỗng/ngoài dải/cap chết trả đúng mã của sim.
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

    /// Object của cap (cap phải sống).
    pub fn cap_obj(&self, cap: CapId) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if c.alive {
            Ok(c.obj)
        } else {
            Err(KError::InvalidCapability)
        }
    }

    /// Object của cap trong slot.
    pub fn cap_obj_at(&self, cnode: ObjId, slot: usize) -> Result<ObjId, KError> {
        self.cap_obj(self.cap_at(cnode, slot)?)
    }

    /// Ảnh chụp cap; cap **đã chết** vẫn `Ok` với `alive = false` (như sim).
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

    /// Duyệt toàn bộ bảng cap theo thứ tự cấp phát (gồm cap đã chết).
    pub fn cap_table(&self) -> CapTable<'_> {
        CapTable {
            caps: &self.caps[..self.n_caps],
            idx: 0,
        }
    }

    /// Số cap còn sống.
    pub fn live_cap_count(&self) -> usize {
        self.caps[..self.n_caps].iter().filter(|c| c.alive).count()
    }

    /// Con trực tiếp còn sống của `cap`, theo thứ tự cấp phát (thứ tự MDB của mô hình).
    pub fn cap_children(&self, cap: CapId) -> Result<CapChildren<'_>, KError> {
        self.cap(cap)?;
        Ok(CapChildren {
            caps: &self.caps[..self.n_caps],
            parent: cap,
            idx: 0,
        })
    }

    /// Object còn sống?
    pub fn object_alive(&self, obj: ObjId) -> bool {
        if obj == 0 || obj as usize > self.n_objects {
            return false;
        }
        self.objects[obj as usize - 1].alive
    }

    /// Loại object (kể cả object đã chết — như sim).
    pub fn object_type(&self, obj: ObjId) -> Option<ObjType> {
        if obj == 0 || obj as usize > self.n_objects {
            return None;
        }
        Some(self.objects[obj as usize - 1].ty)
    }

    /// `(offset, len)` trong Untyped; `None` với object harness cấp (như sim `object_bytes`).
    pub fn object_bytes(&self, obj: ObjId) -> Option<(usize, usize)> {
        if obj == 0 || obj as usize > self.n_objects {
            return None;
        }
        let m = &self.objects[obj as usize - 1];
        m.untyped.map(|_| (m.offset, m.len))
    }

    /// Số object đã từng cấp (kể cả đã chết; id = 1..=số này).
    pub fn object_count(&self) -> usize {
        self.n_objects
    }

    /// Số cap đã từng cấp (kể cả đã chết; id = 1..=số này).
    pub fn cap_count(&self) -> usize {
        self.n_caps
    }

    /// Untyped: cỡ byte arena (0 nếu không phải Untyped).
    pub fn untyped_size(&self, obj: ObjId) -> usize {
        if obj == 0 || obj as usize > self.n_objects {
            return 0;
        }
        let m = &self.objects[obj as usize - 1];
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region
            .map(|r| self.regions[r as usize].capacity)
            .unwrap_or(0)
    }

    /// Untyped: watermark byte đã cấp (0 nếu không phải Untyped).
    pub fn untyped_used(&self, obj: ObjId) -> usize {
        if obj == 0 || obj as usize > self.n_objects {
            return 0;
        }
        let m = &self.objects[obj as usize - 1];
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region
            .map(|r| self.regions[r as usize].cursor)
            .unwrap_or(0)
    }

    /// Untyped: byte còn tự do.
    pub fn untyped_free(&self, obj: ObjId) -> usize {
        self.untyped_size(obj)
            .saturating_sub(self.untyped_used(obj))
    }

    /// Tổng `len` của object **sống** cấp từ Untyped (không gồm byte đệm căn lề).
    pub fn untyped_allocated(&self, obj: ObjId) -> usize {
        if obj == 0 || obj as usize > self.n_objects {
            return 0;
        }
        let idx = obj as usize - 1;
        self.objects[..self.n_objects]
            .iter()
            .filter(|m| m.alive && m.untyped.map(usize::from) == Some(idx))
            .map(|m| m.len)
            .sum()
    }

    /// Byte đệm căn lề nằm giữa watermark và tổng `len` object sống.
    pub fn untyped_gap(&self, obj: ObjId) -> usize {
        self.untyped_used(obj)
            .saturating_sub(self.untyped_allocated(obj))
    }

    /// Tổng watermark của mọi Untyped còn sống.
    pub fn total_used(&self) -> usize {
        self.objects[..self.n_objects]
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::Untyped)
            .map(|m| {
                m.region
                    .map(|r| self.regions[r as usize].cursor)
                    .unwrap_or(0)
            })
            .sum()
    }

    /// `(base, capacity)` arena của Untyped (kể cả đã chết — như sim).
    pub fn untyped_arena(&self, obj: ObjId) -> Option<(u64, usize)> {
        if obj == 0 || obj as usize > self.n_objects {
            return None;
        }
        let m = &self.objects[obj as usize - 1];
        if m.ty != ObjType::Untyped {
            return None;
        }
        let r = m.region? as usize;
        Some((self.regions[r].base, self.regions[r].capacity))
    }
}
