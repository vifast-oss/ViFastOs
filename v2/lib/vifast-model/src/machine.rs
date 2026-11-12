// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mô hình kernel tái sử dụng: Untyped (watermark) + CNode (Copy/Mint/Move/Delete/Revoke +
//! cây dẫn xuất kiểu MDB) + object lõi để chứa cap. Không Frame/VSpace/IPC — phạm vi K1a.
//!
//! Nguồn ngữ nghĩa: `spec/abi.toml` (`[object]`, `[object_size]`, `[error]`,
//! `[method_signature]`), thiết kế §4.2/§4.3, D2-05/D2-14/D2-35. Các quy ước **mô hình**
//! (không phải ABI) được ghi rõ tại từng method:
//! - `Untyped.Retype` trả `CapId` cho harness (ABI `returns = []`, D2-35).
//! - `size_bits = 0` với object cỡ cố định = dùng cỡ tự nhiên; nhỏ hơn ⇒ `Alignment`;
//!   lớn hơn ⇒ `InvalidArgument`. `Untyped` con: `size_bits` = log2 cỡ, tối thiểu 4 KiB.
//! - **F3 (review KT-10)**: `Retype` đòi cap nguồn có `WRITE` và cap con **kế thừa quyền**
//!   cap nguồn; `Copy` có `rights?` của spec và không được nới quyền; `Mint(Some(0))` **xoá**
//!   badge nguồn (`None` mới là giữ nguyên); lớp `invoke_*` đòi cap CNode triệu hồi có
//!   `WRITE` cho `Copy`/`Mint`/`Move`/`Delete`/`Revoke`.
//! - `CNode.Move` (chưa có trong sim KT-10): đích đã có cap thì cap đích bị xoá trước rồi
//!   chuyển — `01-cnode-copy.scn:13` yêu cầu `ok` (D2-44); đích rỗng là đường thường.
//!
//! Bất biến (thiết kế §9.6): `byte đã cấp + byte đệm + byte trống == cỡ Untyped`;
//! không object chồng lấn; mỗi cap sống có cha sống (hoặc là gốc); con của cap bị xoá thuộc
//! về ông; cap sống nằm trong slot thì slot trỏ ngược lại; không cap nào trỏ object chết.

use crate::cap::{CapId, CapInfo, ObjId, SlotRef, RIGHTS_ALL, RIGHT_WRITE};
use crate::error::KError;
use crate::object::{align_up, ObjType, CNODE_BASE_BYTES, DEFAULT_ALIGN, FLAG_GUEST, SLOT_BYTES};

#[derive(Debug)]
enum Kind {
    Untyped { size: usize, watermark: usize },
    CNode { slots: Vec<Option<CapId>> },
    Plain,
}

#[derive(Debug)]
struct Object {
    ty: ObjType,
    alive: bool,
    untyped: Option<usize>,
    offset: usize,
    len: usize,
    kind: Kind,
}

#[derive(Debug)]
struct Cap {
    obj: ObjId,
    ty: ObjType,
    parent: Option<CapId>,
    slot: Option<SlotRef>,
    rights: u8,
    badge: Option<u64>,
    alive: bool,
}

/// Mô hình Untyped/CNode: object + cap + cây dẫn xuất + quyền.
#[derive(Debug)]
pub struct Machine {
    objects: Vec<Object>,
    caps: Vec<Cap>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            caps: Vec::new(),
        }
    }

    // ---------------------------------------------------------------- dựng cảnh

    /// Thêm một Untyped gốc. `base` chỉ để cảnh ghi nhớ; mô hình tính offset từ 0.
    pub fn add_untyped(&mut self, base: u64, size: usize) -> ObjId {
        let _ = base;
        assert!(size > 0, "Untyped phải có cỡ dương");
        self.push_object(
            ObjType::Untyped,
            Kind::Untyped { size, watermark: 0 },
            None,
            0,
            0,
        )
    }

    /// Thêm CNode do harness cấp (init giữ CNode gốc).
    pub fn add_cnode(&mut self, slots: usize) -> ObjId {
        assert!(slots > 0, "CNode phải có ít nhất một slot");
        self.push_object(
            ObjType::CNode,
            Kind::CNode {
                slots: vec![None; slots],
            },
            None,
            0,
            0,
        )
    }

    /// Thêm object lõi do harness cấp — không tính byte Untyped.
    pub fn add_kernel_object(&mut self, ty: ObjType) -> ObjId {
        assert!(ty.is_core(), "chỉ object lõi mới dựng trực tiếp");
        self.push_object(ty, Self::fresh_kind(ty, 0), None, 0, 0)
    }

    /// Cap gốc cho một object (không có cha) — harness đặt vào CNode bằng `put_cap`.
    pub fn root_cap(&mut self, obj: ObjId) -> CapId {
        let ty = self
            .objects
            .get(obj as usize - 1)
            .map(|o| o.ty)
            .expect("root_cap: object phải tồn tại");
        self.push_cap(obj, ty, None, RIGHTS_ALL, None)
    }

    pub fn put_cap(&mut self, cnode: ObjId, slot: usize, cap: CapId) -> Result<(), KError> {
        self.check_cnode_slot(cnode, slot)?;
        let cap_ref = self.cap(cap)?;
        if cap_ref.slot.is_some() {
            return Err(KError::IllegalOperation);
        }
        let slots = self.cnode_slots_mut(cnode)?;
        if slots[slot].is_some() {
            return Err(KError::SlotOccupied);
        }
        slots[slot] = Some(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(SlotRef::new(cnode, slot));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- truy vấn

    pub fn cap_at(&self, cnode: ObjId, slot: usize) -> Result<CapId, KError> {
        let slots = self.cnode_slots(cnode)?;
        if slot >= slots.len() {
            return Err(KError::RangeError);
        }
        match slots[slot] {
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

    /// Ảnh chụp cap; `Err(InvalidCapability)` khi id ngoài dải. Cap **đã chết** vẫn trả
    /// `Ok` với `alive = false` để test kiểm dấu vết.
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

    /// Toàn bộ bảng cap theo thứ tự cấp phát (gồm cap đã chết).
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
        Ok(self.cnode_slots(cnode)?.len())
    }

    pub fn object_alive(&self, obj: ObjId) -> bool {
        self.objects
            .get(obj as usize - 1)
            .map(|o| o.alive)
            .unwrap_or(false)
    }

    pub fn object_type(&self, obj: ObjId) -> Option<ObjType> {
        self.objects.get(obj as usize - 1).map(|o| o.ty)
    }

    /// Số object đã cấp (kể cả đã chết) — id hợp lệ là `1..=object_count()`.
    pub fn object_count(&self) -> u32 {
        self.objects.len() as u32
    }

    /// `(offset, len)` trong Untyped; `None` với object không cấp từ Untyped.
    pub fn object_bytes(&self, obj: ObjId) -> Option<(usize, usize)> {
        let o = self.objects.get(obj as usize - 1)?;
        o.untyped.map(|_| (o.offset, o.len))
    }

    pub fn untyped_size(&self, obj: ObjId) -> usize {
        match self.objects.get(obj as usize - 1).map(|o| &o.kind) {
            Some(Kind::Untyped { size, .. }) => *size,
            _ => 0,
        }
    }

    pub fn untyped_used(&self, obj: ObjId) -> usize {
        match self.objects.get(obj as usize - 1).map(|o| &o.kind) {
            Some(Kind::Untyped { watermark, .. }) => *watermark,
            _ => 0,
        }
    }

    pub fn untyped_free(&self, obj: ObjId) -> usize {
        self.untyped_size(obj)
            .saturating_sub(self.untyped_used(obj))
    }

    /// Tổng `len` của object **sống** cấp từ Untyped (không gồm byte đệm căn lề).
    pub fn untyped_allocated(&self, obj: ObjId) -> usize {
        let Some(idx) = self.objects.get(obj as usize - 1).map(|_| obj as usize - 1) else {
            return 0;
        };
        self.objects
            .iter()
            .filter(|o| o.alive && o.untyped == Some(idx))
            .map(|o| o.len)
            .sum()
    }

    /// Byte đệm căn lề nằm giữa watermark và tổng `len` của object sống.
    pub fn untyped_gap(&self, obj: ObjId) -> usize {
        self.untyped_used(obj)
            .saturating_sub(self.untyped_allocated(obj))
    }

    /// Tổng watermark của mọi Untyped còn sống.
    pub fn total_used(&self) -> usize {
        self.objects
            .iter()
            .filter(|o| o.alive)
            .map(|o| match &o.kind {
                Kind::Untyped { watermark, .. } => *watermark,
                _ => 0,
            })
            .sum()
    }

    // ---------------------------------------------------------------- bất biến

    /// Không chồng lấn, không vượt biên/watermark, `allocated + gap + free == size`.
    pub fn check_bytes(&self, untyped: ObjId) -> Result<(), String> {
        let idx = self
            .objects
            .get(untyped as usize - 1)
            .map(|_| untyped as usize - 1)
            .ok_or_else(|| format!("Untyped {untyped} không tồn tại"))?;
        let (size, watermark) = match &self.objects[idx].kind {
            Kind::Untyped { size, watermark } => (*size, *watermark),
            _ => return Err(format!("object {untyped} không phải Untyped")),
        };
        if watermark > size {
            return Err(format!("watermark {watermark} > cỡ {size}"));
        }
        let mut ranges: Vec<(usize, usize, ObjId)> = Vec::new();
        let mut allocated = 0usize;
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.untyped != Some(idx) {
                continue;
            }
            let end = o.offset + o.len;
            if end > size {
                return Err(format!(
                    "object {} vượt biên Untyped ({end} > {size})",
                    i + 1
                ));
            }
            if end > watermark {
                return Err(format!(
                    "object {} kết thúc ở {end} > watermark {watermark}",
                    i + 1
                ));
            }
            allocated += o.len;
            ranges.push((o.offset, end, (i as u32) + 1));
        }
        ranges.sort_unstable();
        for pair in ranges.windows(2) {
            if pair[0].1 > pair[1].0 {
                return Err(format!(
                    "object {} và {} chồng lấn ({:?})",
                    pair[0].2, pair[1].2, pair
                ));
            }
        }
        if allocated > watermark {
            return Err(format!("tổng len {allocated} > watermark {watermark}"));
        }
        let gap = watermark - allocated;
        let free = size - watermark;
        if allocated + gap + free != size {
            return Err(format!(
                "tổng byte sai: allocated {allocated} + gap {gap} + free {free} != {size}"
            ));
        }
        Ok(())
    }

    pub fn check_all_bytes(&self) -> Result<(), String> {
        for (i, o) in self.objects.iter().enumerate() {
            if o.alive && o.ty == ObjType::Untyped {
                self.check_bytes((i as u32) + 1)?;
            }
        }
        Ok(())
    }

    /// Bất biến cây dẫn xuất cap (MDB tối giản) — thiết kế §4.3/§9.6:
    /// cap sống trỏ object sống cùng kiểu; cap sống có cha thì cha sống, không chu trình;
    /// cap sống nằm trong slot thì slot trỏ ngược lại và mọi slot của CNode sống chỉ chứa
    /// cap sống; cap sống là con thì phải nằm trong một slot (không mồ côi).
    pub fn check_caps(&self) -> Result<(), String> {
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
            if !self.objects[c.obj as usize - 1].alive {
                return Err(format!("cap {cap} trỏ object {} đã chết", c.obj));
            }
            if self.objects[c.obj as usize - 1].ty != c.ty {
                return Err(format!(
                    "cap {cap} kiểu {:?} nhưng object {} kiểu {:?}",
                    c.ty,
                    c.obj,
                    self.objects[c.obj as usize - 1].ty
                ));
            }
        }
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
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.ty != ObjType::CNode {
                continue;
            }
            let cnode = (i as u32) + 1;
            let Kind::CNode { slots } = &o.kind else {
                return Err(format!("object {cnode} kiểu CNode nhưng kind lạ"));
            };
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
                    let slots = self
                        .cnode_slots(cnode)
                        .map_err(|e| format!("cap {cap}: CNode {cnode}: {e:?}"))?;
                    if slot >= slots.len() {
                        return Err(format!(
                            "cap {cap} ở slot {slot} ngoài dải của CNode {cnode}"
                        ));
                    }
                    if slots[slot] != Some(cap) {
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

    // ---------------------------------------------------------------- Retype

    /// `Untyped.Retype` — F3: cap nguồn phải có `WRITE`; cap con kế thừa quyền cap nguồn.
    /// Trả `CapId` cho harness (ABI `returns = []`, D2-35).
    pub fn retype(
        &mut self,
        src: SlotRef,
        ty: ObjType,
        size_bits: u32,
        flags: u32,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (u_idx, size, watermark, src_rights) = {
            let c = self.cap(src_cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let idx = self.object_index(c.obj)?;
            match &self.objects[idx].kind {
                Kind::Untyped { size, watermark } => (idx, *size, *watermark, c.rights),
                _ => return Err(KError::WrongObjectType),
            }
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
        if !ty.is_core() {
            return Err(KError::NotSupported);
        }

        let (len, align, child_slots) = match ty {
            ObjType::Untyped => {
                if !(12..=30).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let len = 1usize << size_bits;
                if len > size {
                    return Err(KError::NotEnoughMemory);
                }
                (len, len, 0)
            }
            ObjType::CNode => {
                if !(4..=20).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let slots = 1usize << size_bits;
                (
                    CNODE_BASE_BYTES + slots * SLOT_BYTES,
                    CNODE_BASE_BYTES,
                    slots,
                )
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
                (fixed, DEFAULT_ALIGN, 0)
            }
        };

        let start = align_up(watermark, align);
        let end = start.checked_add(len).ok_or(KError::InvalidArgument)?;
        if end > size {
            return Err(KError::NotEnoughMemory);
        }

        let obj = self.push_object(
            ty,
            Self::fresh_kind(ty, child_slots),
            Some(u_idx),
            start,
            len,
        );
        if ty == ObjType::Untyped {
            if let Kind::Untyped { size: s, .. } = &mut self.objects[obj as usize - 1].kind {
                *s = len;
            }
        }
        if let Kind::Untyped { watermark: w, .. } = &mut self.objects[u_idx].kind {
            *w = end;
        }
        Ok(self.push_cap(obj, ty, Some(src_cap), src_rights, None))
    }

    // ---------------------------------------------------------------- CSpace

    /// `CNode.Copy` — giữ nguyên quyền + badge của nguồn (không nới quyền được).
    pub fn copy(&mut self, src: SlotRef, dst: SlotRef) -> Result<CapId, KError> {
        self.copy_rights(src, dst, None)
    }

    /// `CNode.Copy` với `rights?` của spec: `None` = giữ quyền nguồn; `Some(r)` phải là
    /// tập con của quyền nguồn, nếu không `InsufficientRights` (F3).
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

    /// `CNode.Mint` — quyền ⊆ nguồn; badge: `None` giữ badge nguồn, `Some(0)` **xoá**
    /// badge (F3), `Some(b)` chỉ cho Endpoint/Notification.
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

    /// Tạo cap con của `src_cap` rồi gắn vào `dst` — đã kiểm nguồn/quyền trước khi cấp
    /// ⇒ thao tác lỗi không để lại cap mồ côi.
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
        if self.cnode_slots(dst.cnode)?[dst.slot].is_some() {
            return Err(KError::SlotOccupied);
        }
        let new_cap = self.push_cap(obj, ty, Some(src_cap), rights, badge);
        self.cnode_slots_mut(dst.cnode)?[dst.slot] = Some(new_cap);
        self.caps[new_cap as usize - 1].slot = Some(dst);
        Ok(new_cap)
    }

    /// `CNode.Move` — chuyển cap sang slot khác; đích đã có cap thì cap đích bị xoá trước
    /// (quy ước mô hình, xem đầu tệp). Nguồn rỗng ⇒ `SlotEmpty`; cùng slot ⇒ `IllegalOperation`.
    pub fn move_cap(&mut self, src: SlotRef, dst: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(src.cnode, src.slot)?;
        self.check_cnode_slot(dst.cnode, dst.slot)?;
        if src == dst {
            return Err(KError::IllegalOperation);
        }
        if self.cnode_slots(dst.cnode)?[dst.slot].is_some() {
            self.delete(dst)?;
        }
        self.clear_slot_of(cap);
        self.cnode_slots_mut(dst.cnode)?[dst.slot] = Some(cap);
        self.caps[cap as usize - 1].slot = Some(dst);
        Ok(())
    }

    /// `CNode.Delete` — xoá cap trong slot (object không bị finalise; chỉ `Revoke` mới
    /// finalise). Con của cap bị xoá thuộc về **ông** (MDB kiểu seL4).
    pub fn delete(&mut self, slot: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(slot.cnode, slot.slot)?;
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
        self.clear_slot_of(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.alive = false;
            c.parent = None;
        }
        Ok(())
    }

    /// `CNode.Revoke` — xoá cap con/cháu của cap trong `slot` (hoặc mọi cap của CNode khi
    /// `slot = None`). Revoke cap Untyped finalise mọi object trong Untyped và đặt watermark
    /// về 0 (thiết kế §4.2). Trả số cap bị xoá.
    pub fn revoke(&mut self, cnode: ObjId, slot: Option<usize>) -> Result<usize, KError> {
        let targets: Vec<CapId> = {
            let slots = self.cnode_slots(cnode)?;
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
            if !self.cap(target)?.alive {
                continue;
            }
            deleted += self.revoke_children(target)?;
        }
        Ok(deleted)
    }

    // ---------------------------------------------------------------- lớp triệu hồi (F3)

    /// Cap CNode dùng để sửa CSpace phải sống, đúng kiểu, và có `WRITE` (F3).
    fn require_cnode_write(&self, cap_slot: SlotRef) -> Result<ObjId, KError> {
        let cap = self.cap_at(cap_slot.cnode, cap_slot.slot)?;
        let c = self.cap(cap)?;
        if c.ty != ObjType::CNode {
            return Err(KError::WrongObjectType);
        }
        if c.rights & RIGHT_WRITE == 0 {
            return Err(KError::InsufficientRights);
        }
        Ok(c.obj)
    }

    fn check_dst_in_cnode(dst: SlotRef, cnode: ObjId) -> Result<(), KError> {
        if dst.cnode != cnode {
            return Err(KError::InvalidArgument);
        }
        Ok(())
    }

    /// `CNode.Copy` qua cap CNode (F3): cap triệu hồi cần `WRITE`, đích thuộc CNode đó.
    pub fn invoke_copy(
        &mut self,
        cnode_cap: SlotRef,
        src: SlotRef,
        dst: SlotRef,
        rights: Option<u8>,
    ) -> Result<CapId, KError> {
        let cnode = self.require_cnode_write(cnode_cap)?;
        Self::check_dst_in_cnode(dst, cnode)?;
        self.copy_rights(src, dst, rights)
    }

    /// `CNode.Mint` qua cap CNode (F3).
    pub fn invoke_mint(
        &mut self,
        cnode_cap: SlotRef,
        src: SlotRef,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        let cnode = self.require_cnode_write(cnode_cap)?;
        Self::check_dst_in_cnode(dst, cnode)?;
        self.mint(src, dst, rights, badge)
    }

    /// `CNode.Move` qua cap CNode (F3).
    pub fn invoke_move(
        &mut self,
        cnode_cap: SlotRef,
        src: SlotRef,
        dst: SlotRef,
    ) -> Result<(), KError> {
        let cnode = self.require_cnode_write(cnode_cap)?;
        Self::check_dst_in_cnode(dst, cnode)?;
        self.move_cap(src, dst)
    }

    /// `CNode.Delete` qua cap CNode (F3): sửa slot của CNode cần `WRITE`.
    pub fn invoke_delete(&mut self, cnode_cap: SlotRef, dst: SlotRef) -> Result<(), KError> {
        let cnode = self.require_cnode_write(cnode_cap)?;
        Self::check_dst_in_cnode(dst, cnode)?;
        self.delete(dst)
    }

    /// `CNode.Revoke` qua cap CNode (F3): cap triệu hồi cần `WRITE`.
    pub fn invoke_revoke(
        &mut self,
        cnode_cap: SlotRef,
        slot: Option<usize>,
    ) -> Result<usize, KError> {
        let cnode = self.require_cnode_write(cnode_cap)?;
        self.revoke(cnode, slot)
    }

    // ---------------------------------------------------------------- nội bộ

    fn push_object(
        &mut self,
        ty: ObjType,
        kind: Kind,
        untyped: Option<usize>,
        offset: usize,
        len: usize,
    ) -> ObjId {
        self.objects.push(Object {
            ty,
            alive: true,
            untyped,
            offset,
            len,
            kind,
        });
        self.objects.len() as ObjId
    }

    fn push_cap(
        &mut self,
        obj: ObjId,
        ty: ObjType,
        parent: Option<CapId>,
        rights: u8,
        badge: Option<u64>,
    ) -> CapId {
        self.caps.push(Cap {
            obj,
            ty,
            parent,
            slot: None,
            rights,
            badge,
            alive: true,
        });
        self.caps.len() as CapId
    }

    fn fresh_kind(ty: ObjType, child_slots: usize) -> Kind {
        match ty {
            ObjType::Untyped => Kind::Untyped {
                size: 0,
                watermark: 0,
            },
            ObjType::CNode => Kind::CNode {
                slots: vec![None; child_slots.max(1)],
            },
            _ => Kind::Plain,
        }
    }

    fn object_index(&self, obj: ObjId) -> Result<usize, KError> {
        if obj == 0 || obj as usize > self.objects.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(obj as usize - 1)
    }

    fn object(&self, obj: ObjId) -> Result<&Object, KError> {
        Ok(&self.objects[self.object_index(obj)?])
    }

    fn cap(&self, cap: CapId) -> Result<&Cap, KError> {
        if cap == 0 || cap as usize > self.caps.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(&self.caps[cap as usize - 1])
    }

    fn cnode_slots(&self, cnode: ObjId) -> Result<&[Option<CapId>], KError> {
        match &self.object(cnode)?.kind {
            Kind::CNode { slots } => Ok(slots),
            _ => Err(KError::WrongObjectType),
        }
    }

    fn cnode_slots_mut(&mut self, cnode: ObjId) -> Result<&mut [Option<CapId>], KError> {
        let idx = self.object_index(cnode)?;
        match &mut self.objects[idx].kind {
            Kind::CNode { slots } => Ok(slots),
            _ => Err(KError::WrongObjectType),
        }
    }

    fn check_cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<(), KError> {
        let slots = self.cnode_slots(cnode)?;
        if slot >= slots.len() {
            return Err(KError::RangeError);
        }
        Ok(())
    }

    fn clear_slot_of(&mut self, cap: CapId) {
        let slot = self
            .caps
            .get_mut(cap as usize - 1)
            .and_then(|c| c.slot.take());
        if let Some(SlotRef { cnode, slot: s }) = slot {
            if let Ok(slots) = self.cnode_slots_mut(cnode) {
                if slots.get(s).copied().flatten() == Some(cap) {
                    slots[s] = None;
                }
            }
        }
    }

    /// Xoá cap con/cháu của `root`; nếu `root` là cap Untyped thì finalise object + reset watermark.
    fn revoke_children(&mut self, root: CapId) -> Result<usize, KError> {
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
            self.collect_untyped_tree(u_idx, &mut dead_objs);
            if let Kind::Untyped { watermark, .. } = &mut self.objects[u_idx].kind {
                *watermark = 0;
            }
            for id in &dead_objs {
                let idx = *id as usize - 1;
                if let Kind::Untyped { watermark, .. } = &mut self.objects[idx].kind {
                    *watermark = 0;
                }
                self.objects[idx].alive = false;
            }
            for i in 1..=self.caps.len() as CapId {
                if self.caps[i as usize - 1].alive
                    && dead_objs.contains(&self.caps[i as usize - 1].obj)
                {
                    doomed.push(i);
                }
            }
            let mut cnode_caps: Vec<CapId> = Vec::new();
            for id in &dead_objs {
                let idx = *id as usize - 1;
                if self.objects[idx].ty != ObjType::CNode {
                    continue;
                }
                if let Kind::CNode { slots } = &self.objects[idx].kind {
                    cnode_caps.extend(slots.iter().flatten().copied());
                }
            }
            for cap in cnode_caps {
                if self.cap(cap)?.alive {
                    self.collect_subtree(cap, &mut doomed);
                }
            }
            for id in &dead_objs {
                let idx = *id as usize - 1;
                if let Kind::CNode { slots } = &mut self.objects[idx].kind {
                    slots.iter_mut().for_each(|s| *s = None);
                }
            }
        }

        doomed.sort_unstable();
        doomed.dedup();
        let mut deleted = 0;
        for cap in doomed {
            if self.caps[cap as usize - 1].alive {
                self.clear_slot_of(cap);
                self.caps[cap as usize - 1].alive = false;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Gom `root` và mọi con/cháu còn sống của nó (theo cạnh cha) vào `out`.
    fn collect_subtree(&self, root: CapId, out: &mut Vec<CapId>) {
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
    fn collect_untyped_tree(&self, u_idx: usize, out: &mut Vec<ObjId>) {
        for (i, o) in self.objects.iter().enumerate() {
            if o.alive && o.untyped == Some(u_idx) {
                out.push((i as u32) + 1);
                if o.ty == ObjType::Untyped {
                    self.collect_untyped_tree(i, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `check_caps` không được là checker rỗng: gieo cap mồ côi thì phải bắt.
    #[test]
    fn check_caps_bat_cap_mo_coi() {
        let mut m = Machine::new();
        let a = m.add_cnode(4);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let root = m.root_cap(ep);
        m.put_cap(a, 0, root).expect("cap gốc");
        let child = m
            .copy(SlotRef::new(a, 0), SlotRef::new(a, 1))
            .expect("cap con");
        m.check_caps().expect("cây hợp lệ");
        m.caps[root as usize - 1].alive = false;
        m.caps[root as usize - 1].slot = None;
        m.cnode_slots_mut(a).expect("CNode")[0] = None;
        let err = m.check_caps().expect_err("phải bắt cap mồ côi");
        assert!(err.contains("mồ côi"), "{err}");
        assert!(m.cap_info(child).expect("con").alive);
    }

    /// Slot trỏ cap nhưng cap không trỏ ngược (MDB hỏng) phải bị bắt.
    #[test]
    fn check_caps_bat_slot_khong_doi_xung() {
        let mut m = Machine::new();
        let a = m.add_cnode(4);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let cap = m.root_cap(ep);
        m.put_cap(a, 0, cap).expect("cap gốc");
        m.caps[cap as usize - 1].slot = None;
        let err = m.check_caps().expect_err("phải bắt slot không đối xứng");
        assert!(
            err.contains("không trỏ ngược") || err.contains("không trỏ lại"),
            "{err}"
        );
        let mut m2 = Machine::new();
        let b = m2.add_cnode(4);
        let ep2 = m2.add_kernel_object(ObjType::Endpoint);
        let cap2 = m2.root_cap(ep2);
        m2.put_cap(b, 0, cap2).expect("cap gốc");
        m2.caps[cap2 as usize - 1].alive = false;
        let err = m2.check_caps().expect_err("phải bắt cap chết còn giữ slot");
        assert!(err.contains("cap chết"), "{err}");
    }

    /// `check_bytes` phải bắt watermark vượt biên và object chồng lấn (không checker rỗng).
    #[test]
    fn check_bytes_bat_watermark_va_chong_lan() {
        let mut m = Machine::new();
        let u = m.add_untyped(0, 4096);
        if let Kind::Untyped { watermark, .. } = &mut m.objects[u as usize - 1].kind {
            *watermark = 4097;
        }
        let err = m.check_bytes(u).expect_err("phải bắt watermark > cỡ");
        assert!(err.contains("watermark"), "{err}");

        let mut m2 = Machine::new();
        let u2 = m2.add_untyped(0, 4096);
        let f1 = m2.push_object(ObjType::Tcb, Kind::Plain, Some(u2 as usize - 1), 0, 512);
        let _f2 = m2.push_object(ObjType::Tcb, Kind::Plain, Some(u2 as usize - 1), 256, 512);
        if let Kind::Untyped { watermark, .. } = &mut m2.objects[u2 as usize - 1].kind {
            *watermark = 1024;
        }
        let err = m2.check_bytes(u2).expect_err("phải bắt chồng lấn");
        assert!(err.contains("chồng lấn"), "{err}");
        let _ = f1;
    }
}
