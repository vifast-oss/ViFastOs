// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bất biến của core tầng 1 — port `Machine::check_caps`/`check_bytes` của sim, **không cấp
//! phát** (`Vec` + `sort` của sim thành mảng tĩnh/pass O(n²)).
//!
//! Bốn nhóm của `check_caps` (sim §MDB): cap ↔ object, cha–con không chu trình, slot ↔ cap
//! đối xứng, cap nói nằm ở slot thì slot phải trỏ lại. `check_bytes` giữ bất biến
//! `đã cấp + byte đệm + còn trống == cỡ Untyped`, không chồng lấn, không vượt watermark.

use crate::error::{CheckFailure, ObjId, ObjType, SlotRef};
use crate::machine::Machine;
use crate::observer::CoreObserver;

impl<O: CoreObserver> Machine<O> {
    /// Bất biến toàn cục của cap + cây MDB + slot CNode.
    pub fn check_caps(&self) -> Result<(), CheckFailure> {
        let n_caps = self.n_caps;
        // 1) cap ↔ object.
        for i in 0..n_caps {
            let cap = (i as u32) + 1;
            let c = &self.caps[i];
            if !c.alive {
                if c.slot.is_some() {
                    return Err(CheckFailure::one("cap chet con giu slot", cap));
                }
                continue;
            }
            if c.obj == 0 || c.obj as usize > self.n_objects {
                return Err(CheckFailure::one("cap tro object ngoai dai", cap));
            }
            let m = &self.objects[c.obj as usize - 1];
            if !m.alive {
                return Err(CheckFailure::two("cap tro object da chet", cap, c.obj));
            }
            if m.ty != c.ty {
                return Err(CheckFailure::two("cap kieu lech object", cap, c.obj));
            }
        }
        // 2) quan hệ cha–con + không chu trình.
        for i in 0..n_caps {
            let cap = (i as u32) + 1;
            let c = &self.caps[i];
            if !c.alive {
                continue;
            }
            let Some(p) = c.parent else { continue };
            if p == 0 || p as usize > n_caps {
                return Err(CheckFailure::one("cap co cha ngoai dai", cap));
            }
            if !self.caps[p as usize - 1].alive {
                return Err(CheckFailure::two("cap mo coi: cha da chet", cap, p));
            }
            let mut cur = Some(p);
            let mut steps = 0usize;
            while let Some(x) = cur {
                if x == 0 || x as usize > n_caps {
                    return Err(CheckFailure::two("chuoi cha co id ngoai dai", cap, x));
                }
                if x == cap {
                    return Err(CheckFailure::one("cap co chu trinh cha", cap));
                }
                cur = self.caps[x as usize - 1].parent;
                steps += 1;
                if steps > n_caps {
                    return Err(CheckFailure::one("chuoi cha dai bat thuong", cap));
                }
            }
        }
        // 3) slot ↔ cap (đối xứng MDB) của mọi CNode sống.
        for i in 0..self.n_objects {
            let (alive, ty, slot_len) = {
                let o = &self.objects[i];
                (o.alive, o.ty, o.slot_len)
            };
            if !alive || ty != ObjType::CNode {
                continue;
            }
            let cnode = (i as u32) + 1;
            for slot in 0..slot_len {
                let cap = match self.cnode_slot(cnode, slot) {
                    Ok(Some(cap)) => cap,
                    Ok(None) => continue,
                    Err(_) => return Err(CheckFailure::one("CNode doc slot loi", cnode)),
                };
                if cap == 0 || cap as usize > n_caps {
                    return Err(CheckFailure::two("slot co cap ngoai dai", cnode, cap));
                }
                let c = &self.caps[cap as usize - 1];
                if !c.alive {
                    return Err(CheckFailure::two("slot chua cap chet", cnode, cap));
                }
                if c.slot != Some(SlotRef::new(cnode, slot)) {
                    return Err(CheckFailure::two("slot tro cap khong doi xung", cnode, cap));
                }
            }
        }
        // 4) cap ↔ slot + vị trí cap.
        for i in 0..n_caps {
            let cap = (i as u32) + 1;
            let c = &self.caps[i];
            if !c.alive {
                continue;
            }
            match c.slot {
                Some(SlotRef { cnode, slot }) => {
                    if !self.object_alive(cnode) {
                        return Err(CheckFailure::two("cap nam trong CNode da chet", cap, cnode));
                    }
                    if self.object_type(cnode) != Some(ObjType::CNode) {
                        return Err(CheckFailure::two(
                            "cap nam trong object khong phai CNode",
                            cap,
                            cnode,
                        ));
                    }
                    let n = match self.cnode_slot_count(cnode) {
                        Ok(n) => n,
                        Err(_) => return Err(CheckFailure::two("cap thuoc CNode loi", cap, cnode)),
                    };
                    if slot >= n {
                        return Err(CheckFailure::two("cap o slot ngoai dai", cap, cnode));
                    }
                    match self.cnode_slot(cnode, slot) {
                        Ok(Some(x)) if x == cap => {}
                        _ => {
                            return Err(CheckFailure::two(
                                "cap noi nam o slot nhung slot khong tro lai",
                                cap,
                                cnode,
                            ))
                        }
                    }
                }
                None => {
                    if c.parent.is_some() {
                        return Err(CheckFailure::one(
                            "cap la con nhung khong nam trong slot nao",
                            cap,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Bất biến byte của **một** Untyped: không chồng lấn, không vượt biên/watermark,
    /// `allocated + gap + free == capacity`.
    pub fn check_bytes(&self, untyped: ObjId) -> Result<(), CheckFailure> {
        if untyped == 0 || untyped as usize > self.n_objects {
            return Err(CheckFailure::one("Untyped khong ton tai", untyped));
        }
        let m = &self.objects[untyped as usize - 1];
        if m.ty != ObjType::Untyped {
            return Err(CheckFailure::one("object khong phai Untyped", untyped));
        }
        let region = m
            .region
            .map(usize::from)
            .ok_or(CheckFailure::one("Untyped khong co vung nho", untyped))?;
        let size = self.regions[region].capacity;
        let watermark = self.regions[region].cursor;
        if watermark > size {
            return Err(CheckFailure::one("watermark > co Untyped", untyped));
        }
        let u_idx = untyped as usize - 1;
        let mut allocated = 0usize;
        for (i, o) in self.objects[..self.n_objects].iter().enumerate() {
            if !o.alive || o.untyped.map(usize::from) != Some(u_idx) {
                continue;
            }
            let Some(end) = o.offset.checked_add(o.len) else {
                return Err(CheckFailure::one("object tran offset", (i as u32) + 1));
            };
            if end > size {
                return Err(CheckFailure::one(
                    "object vuot bien Untyped",
                    (i as u32) + 1,
                ));
            }
            if end > watermark {
                return Err(CheckFailure::one(
                    "object ket thuc sau watermark",
                    (i as u32) + 1,
                ));
            }
            allocated += o.len;
        }
        for (i, a) in self.objects[..self.n_objects].iter().enumerate() {
            if !a.alive || a.untyped.map(usize::from) != Some(u_idx) {
                continue;
            }
            let (a0, a1) = (a.offset, a.offset + a.len);
            for (j, b) in self.objects[..self.n_objects]
                .iter()
                .enumerate()
                .skip(i + 1)
            {
                if !b.alive || b.untyped.map(usize::from) != Some(u_idx) {
                    continue;
                }
                let (b0, b1) = (b.offset, b.offset + b.len);
                if a0 < b1 && b0 < a1 {
                    return Err(CheckFailure::two(
                        "object chong lan",
                        (i as u32) + 1,
                        (j as u32) + 1,
                    ));
                }
            }
        }
        if allocated > watermark {
            return Err(CheckFailure::one("tong len > watermark", untyped));
        }
        let gap = watermark - allocated;
        let free = size - watermark;
        if allocated + gap + free != size {
            return Err(CheckFailure::one(
                "tong byte sai (allocated+gap+free)",
                untyped,
            ));
        }
        Ok(())
    }

    /// `check_bytes` cho mọi Untyped còn sống.
    pub fn check_all_bytes(&self) -> Result<(), CheckFailure> {
        for i in 0..self.n_objects {
            let (alive, ty) = {
                let m = &self.objects[i];
                (m.alive, m.ty)
            };
            if alive && ty == ObjType::Untyped {
                self.check_bytes((i as u32) + 1)?;
            }
        }
        Ok(())
    }
}
