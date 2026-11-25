// SPDX-License-Identifier: MIT OR Apache-2.0
//! Truy cập **slot CNode** của [`Machine`] — KT-64: slot đọc/ghi trực tiếp trong **cửa sổ byte
//! slot của arena** ([`crate::slotmem`]), không còn pool riêng. Tách khỏi `machine.rs` để giữ
//! luật R1 (≤ 1.500 dòng/tệp); cùng `impl Machine` nên API không đổi.

use crate::error::{CapId, KError, ObjId, ObjType, SlotRef};
use crate::machine::{Cap, Machine, Meta};
use crate::observer::CoreObserver;
use crate::spec::{CNODE_BASE_BYTES, SLOT_BYTES};
use crate::store::{KernelStore, SlotWindow};

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    /// Số slot của CNode (đọc được cả sau khi CNode chết — như `Meta::slots` của sim).
    pub fn cnode_slot_count(&self, cnode: ObjId) -> Result<usize, KError> {
        let m = self.meta(cnode)?;
        if !m.alive {
            return m.slots_meta.map(usize::from).ok_or(KError::WrongObjectType);
        }
        if m.ty != ObjType::CNode {
            return Err(KError::WrongObjectType);
        }
        Ok(m.slot_len)
    }

    /// **KT-64** — cửa sổ **byte slot trong arena** của CNode: `(PA probe, số byte slot)`.
    ///
    /// - CNode **Retype từ Untyped** ⇒ `Some((region.base + offset + cnode_base, slots*48))`:
    ///   đây là **đúng** byte slot nằm trong vùng Untyped (D2-83: `base` là **nhãn PA**; core
    ///   không dereference — TAP chỉ so nhãn/khoảng).
    /// - CNode **harness** (`add_cnode`/`add_kernel_object`) ⇒ `None`: như `add_cnode` của sim,
    ///   CNode do kernel cấp **không** tiêu byte arena (byte slot của nó ở vùng B của
    ///   [`SlotMem`], không thuộc Untyped nào).
    /// - CNode đã chết / sai kiểu ⇒ `Ok(None)`/lỗi như [`Machine::cnode_slot_count`].
    pub fn cnode_slot_span(&self, cnode: ObjId) -> Result<Option<(u64, usize)>, KError> {
        let m = self.meta(cnode)?;
        if m.ty != ObjType::CNode {
            return Err(KError::WrongObjectType);
        }
        if !m.alive {
            return Ok(None);
        }
        let n = m
            .slots_meta
            .map(usize::from)
            .ok_or(KError::WrongObjectType)?;
        let Some(u_idx) = m.untyped.map(usize::from) else {
            // CNode harness: không tiêu byte arena (như `add_cnode` của sim).
            return Ok(None);
        };
        let region = self
            .objects
            .get(u_idx)
            .and_then(|u| u.region.map(usize::from))
            .ok_or(KError::WrongObjectType)?;
        let pa = self.regions[region]
            .base
            .saturating_add(m.offset as u64)
            .saturating_add(CNODE_BASE_BYTES as u64);
        Ok(Some((pa, n.saturating_mul(SLOT_BYTES))))
    }

    pub(crate) fn meta(&self, obj: ObjId) -> Result<&Meta, KError> {
        Ok(&self.objects[self.object_index(obj)?])
    }

    pub(crate) fn cap(&self, cap: CapId) -> Result<&Cap, KError> {
        if cap == 0 || cap as usize > self.n_caps {
            return Err(KError::InvalidCapability);
        }
        Ok(&self.caps[cap as usize - 1])
    }

    pub(crate) fn cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<Option<CapId>, KError> {
        let m = self.meta(cnode)?;
        if !m.alive {
            let n = m
                .slots_meta
                .map(usize::from)
                .ok_or(KError::WrongObjectType)?;
            return if slot < n {
                Ok(None)
            } else {
                Err(KError::RangeError)
            };
        }
        if m.ty != ObjType::CNode {
            return Err(KError::WrongObjectType);
        }
        if slot >= m.slot_len {
            return Err(KError::RangeError);
        }
        let word = self.slots.get(m.slot_base, slot);
        Ok(if word == 0 { None } else { Some(word) })
    }

    pub(crate) fn cnode_set_slot(
        &mut self,
        cnode: ObjId,
        slot: usize,
        cap: Option<CapId>,
    ) -> Result<(), KError> {
        let (base, len) = {
            let m = self.meta(cnode)?;
            // Như sim `cnode_set_slot` → `body_mut`: object đã finalise ⇒ `Dead` ⇒
            // `InvalidCapability`; object sống sai kiểu ⇒ `WrongObjectType`.
            if !m.alive {
                return Err(KError::InvalidCapability);
            }
            if m.ty != ObjType::CNode {
                return Err(KError::WrongObjectType);
            }
            (m.slot_base, m.slot_len)
        };
        if slot >= len {
            return Err(KError::RangeError);
        }
        self.slots.set(base, slot, cap.unwrap_or(0));
        Ok(())
    }

    pub(crate) fn check_cnode_slot(&self, cnode: ObjId, slot: usize) -> Result<(), KError> {
        let n = self.cnode_slot_count(cnode)?;
        if slot >= n {
            return Err(KError::RangeError);
        }
        Ok(())
    }

    pub(crate) fn clear_slot_of(&mut self, cap: CapId) {
        let slot = self
            .caps
            .get_mut(cap as usize - 1)
            .and_then(|c| c.slot.take());
        if let Some(SlotRef { cnode, slot: s }) = slot {
            if let Ok(Some(cur)) = self.cnode_slot(cnode, s) {
                if cur == cap {
                    let _ = self.cnode_set_slot(cnode, s, None);
                }
            }
        }
    }
}
