// SPDX-License-Identifier: MIT OR Apache-2.0
//! `check_objmem` — tương đương `check_vm` của K2, chạy `no_std` (KT-52).

use super::*;

impl<M: PhysMem> Machine<M> {
    // ----------------------------------------------------------------------------------- Soát

    /// `check_vm` tương đương **no_std**: bất biến byte + Frame + W^X + walk khớp + cap mồ côi.
    pub fn check_objmem(&self) -> Result<ObjStats, ObjError> {
        // Không cờ tự khai: mọi checker sai đều trả `Err` (TAP in `check=0`), `Ok` mới có stats.
        let mut stats = ObjStats {
            counts: self.counts(),
            ..ObjStats::default()
        };
        let mut frame_maps = [0u16; MAX_OBJECTS];
        let mut frame_w = [0u16; MAX_OBJECTS];
        let mut frame_x = [0u16; MAX_OBJECTS];
        let mut frame_dev = [0u16; MAX_OBJECTS];
        // 1) Bất biến byte + PA của từng Untyped.
        //    Span Frame sống để allocator đối chiếu — liệt kê từ **bảng object** ở dưới.
        let mut spans = [(0u32, 0u32); MAX_OBJECTS];
        let mut n_spans;
        for u_idx in 0..self.n_untypeds as usize {
            n_spans = 0usize;
            let u = &self.untypeds[u_idx];
            if !u.live {
                return Err(ObjError::Corrupt);
            }
            // `Untyped.object` phải trỏ đúng object Untyped của chính nó (không mồ côi).
            let root_obj = self.object(u.object)?;
            if root_obj.kind != ObjKind::Untyped || root_obj.untyped as usize != u_idx {
                return Err(ObjError::Corrupt);
            }
            if u.bump > u.capacity {
                return Err(ObjError::Corrupt);
            }
            // Hole: sắp thứ tự, trong [0, bump), không chồng.
            let mut prev_end = 0u64;
            let mut hole_sum = 0u64;
            for i in 0..u.n_holes as usize {
                let h = u.holes[i];
                if h.len == 0 || h.off as u64 + h.len as u64 > u.bump as u64 {
                    return Err(ObjError::Corrupt);
                }
                if (h.off as u64) < prev_end {
                    return Err(ObjError::Corrupt);
                }
                prev_end = h.off as u64 + h.len as u64;
                hole_sum += h.len as u64;
            }
            let mut live_sum = 0u64;
            for (i, obj) in self.objects.iter().enumerate() {
                if !obj.live || obj.untyped as usize != u_idx || obj.kind == ObjKind::Untyped {
                    continue;
                }
                if obj.kind == ObjKind::Frame {
                    let size = 1u64 << obj.size_bits;
                    let pa = u.pa_base() + u64::from(obj.pa_off);
                    if pa & (size - 1) != 0 || u64::from(obj.pa_off) + size > UNTYPED_PA_WINDOW {
                        return Err(ObjError::Corrupt);
                    }
                    // Không chồng Frame sống khác (toàn cục).
                    for (j, other) in self.objects.iter().enumerate() {
                        if j <= i || !other.live || other.kind != ObjKind::Frame {
                            continue;
                        }
                        let opa = self.untypeds[other.untyped as usize].pa_base()
                            + u64::from(other.pa_off);
                        let osize = 1u64 << other.size_bits;
                        if pa < opa + osize && opa < pa + size {
                            return Err(ObjError::AlreadyMapped);
                        }
                    }
                    if n_spans < spans.len() {
                        spans[n_spans] = (obj.pa_off, size as u32);
                        n_spans += 1;
                    } else {
                        return Err(ObjError::Corrupt);
                    }
                } else if obj.kind == ObjKind::VSpace {
                    // Metadata VSpace nằm trong bảng tĩnh `.bss` (root PT là object riêng đã
                    // được tính ở nhánh dưới) — VSpace **không** tiêu byte arena.
                } else {
                    if obj.len == 0 || obj.off as u64 + obj.len as u64 > u.bump as u64 {
                        return Err(ObjError::Corrupt);
                    }
                    live_sum += obj.len as u64;
                    // Không chồng hole.
                    for &h in u.holes.iter().take(u.n_holes as usize) {
                        if obj.off < h.off + h.len && h.off < obj.off + obj.len {
                            return Err(ObjError::Corrupt);
                        }
                    }
                }
            }
            // Không chồng giữa các block sống.
            for (i, a) in self.objects.iter().enumerate() {
                if !a.live
                    || a.untyped as usize != u_idx
                    || a.kind == ObjKind::Frame
                    || a.kind == ObjKind::VSpace
                {
                    continue;
                }
                for (j, b) in self.objects.iter().enumerate() {
                    if j <= i
                        || !b.live
                        || b.untyped as usize != u_idx
                        || b.kind == ObjKind::Frame
                        || b.kind == ObjKind::VSpace
                    {
                        continue;
                    }
                    if a.off < b.off + b.len && b.off < a.off + a.len {
                        return Err(ObjError::Corrupt);
                    }
                }
            }
            // Phủ **kín** [0, bump): live + hole (byte đệm căn lề là hole, không phải gap tự
            // khai) và hai tập đã kiểm không chồng lẫn nhau ⇒ không một byte nào bị "rò".
            if live_sum + hole_sum != u.bump as u64 {
                return Err(ObjError::Corrupt);
            }
            // PA: allocator Frame (KT-67) kiểm **danh sách Frame sống liệt kê ở trên** +
            // hole + watermark — không tautology, không bản sao danh sách trong allocator.
            if !u.pa_check(&spans[..n_spans]) {
                return Err(ObjError::Corrupt);
            }
            stats.bytes_cap += u.capacity as usize;
            stats.bytes_used += live_sum as usize;
            stats.bytes_free += (u.capacity as u64 - live_sum) as usize;
        }
        // 1b) VSpace root + bảng nội bộ phải trỏ đúng chủ (không bảng mồ côi).
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live {
                continue;
            }
            if obj.kind == ObjKind::VSpace {
                let root = self.object(obj.root)?;
                if root.kind != ObjKind::PageTable || root.owner as usize != i {
                    return Err(ObjError::Corrupt);
                }
            }
            if obj.kind == ObjKind::PageTable && obj.owner != VS_NONE {
                let Some(vs) = self.objects.get(obj.owner as usize) else {
                    return Err(ObjError::Corrupt);
                };
                if !vs.live || vs.kind != ObjKind::VSpace {
                    return Err(ObjError::Corrupt);
                }
            }
        }
        // 2) Mapping: walk khớp + đếm lại Frame.
        for map in self.mappings.iter() {
            if !map.live {
                continue;
            }
            let vs = self.object(map.vspace as u16)?;
            if vs.kind != ObjKind::VSpace {
                return Err(ObjError::Corrupt);
            }
            let fr = self.object(map.frame)?;
            if fr.kind != ObjKind::Frame {
                return Err(ObjError::Corrupt);
            }
            if frame_level(fr.size_bits)? != map.level {
                return Err(ObjError::Corrupt);
            }
            let flags = map.flags();
            if !flags.is_valid() || (flags.device && flags.exec) {
                return Err(ObjError::WxViolation);
            }
            let walk = self.walk_obj(&vs, map.va)?;
            if !walk.valid || !walk.leaf || walk.level != map.level {
                return Err(ObjError::Corrupt);
            }
            let size = 1u64 << fr.size_bits;
            let expected = self.untypeds[fr.untyped as usize].pa_base()
                + u64::from(fr.pa_off)
                + (map.va & (size - 1));
            if walk.phys != expected || !flags.matches_pte(walk.raw) {
                return Err(ObjError::Corrupt);
            }
            stats.walked += 1;
            frame_maps[map.frame as usize] += 1;
            if flags.write {
                frame_w[map.frame as usize] += 1;
            }
            if flags.exec {
                frame_x[map.frame as usize] += 1;
            }
            if flags.device {
                frame_dev[map.frame as usize] += 1;
            }
        }
        // 3) Đếm lại Frame + W^X toàn cục (kể cả **Device** — KT-56, nợ L3 KT-52).
        for (i, fr) in self.objects.iter().enumerate() {
            if !fr.live || fr.kind != ObjKind::Frame {
                continue;
            }
            if fr.maps != frame_maps[i]
                || fr.mapped_w != frame_w[i]
                || fr.mapped_x != frame_x[i]
                || fr.mapped_device != frame_dev[i]
            {
                return Err(ObjError::Corrupt);
            }
            if (fr.mapped_w > 0 && fr.mapped_x > 0) || (fr.mapped_device > 0 && fr.mapped_x > 0) {
                return Err(ObjError::WxViolation);
            }
        }
        // 4) Không frame/object mồ côi + cap hợp lệ.
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live {
                continue;
            }
            if obj.kind == ObjKind::Frame {
                let has_cap = obj.cap != CAP_NONE
                    && self
                        .caps
                        .get(obj.cap as usize)
                        .is_some_and(|c| c.live && c.obj == i as u16);
                if !has_cap {
                    stats.orphans += 1;
                }
            } else if !self.reachable(i as u16) {
                stats.orphans += 1;
            }
        }
        if stats.orphans != 0 {
            return Err(ObjError::InvalidCapability);
        }
        for (i, cap) in self.caps.iter().enumerate() {
            if !cap.live {
                continue;
            }
            let obj = self.object(cap.obj)?;
            if obj.kind != cap.kind || obj.id == 0 || obj.id != cap.id {
                return Err(ObjError::Corrupt);
            }
            if cap.parent != CAP_NONE && !self.caps.get(cap.parent as usize).is_some_and(|p| p.live)
            {
                return Err(ObjError::InvalidCapability);
            }
            let _ = i;
        }
        Ok(stats)
    }

    /// `true` ⇔ object `idx` có cap sống hoặc là bảng nội bộ của VSpace sống.
    fn reachable(&self, idx: u16) -> bool {
        let Some(obj) = self.objects.get(idx as usize).copied() else {
            return false;
        };
        if !obj.live {
            return false;
        }
        if obj.cap != CAP_NONE
            && self
                .caps
                .get(obj.cap as usize)
                .is_some_and(|c| c.live && c.obj == idx)
        {
            return true;
        }
        if obj.kind == ObjKind::PageTable && obj.owner != VS_NONE {
            if let Some(vs) = self.objects.get(obj.owner as usize).copied() {
                return vs.live && self.reachable(obj.owner as u16);
            }
        }
        false
    }

    // ----------------------------------------------------------------------------- truy vấn khác

    /// Thống kê nhanh (không kiểm bất biến).
    pub fn counts(&self) -> ObjCounts {
        let mut out = ObjCounts {
            untypeds: self.n_untypeds as usize,
            ..ObjCounts::default()
        };
        for obj in &self.objects {
            if !obj.live {
                continue;
            }
            out.objects += 1;
            match obj.kind {
                ObjKind::PageTable => out.page_tables += 1,
                ObjKind::Frame => out.frames += 1,
                ObjKind::VSpace => out.vspaces += 1,
                ObjKind::Untyped => {}
            }
        }
        for cap in &self.caps {
            if cap.live {
                out.caps += 1;
            }
        }
        for map in &self.mappings {
            if map.live {
                out.mappings += 1;
            }
        }
        out
    }

    /// Loại object của một cap.
    pub fn cap_kind(&self, cap: CapId) -> Result<ObjKind, ObjError> {
        Ok(self.cap(cap)?.kind)
    }

    /// Key (id duy nhất) của cap — "key tối thiểu" của KT-52.
    pub fn cap_key(&self, cap: CapId) -> Result<u32, ObjError> {
        Ok(self.cap(cap)?.id)
    }

    /// PA nhãn của Frame.
    pub fn frame_pa(&self, cap: CapId) -> Result<u64, ObjError> {
        let (_, fr) = self.frame_cap_full(cap)?;
        Ok(self.untypeds[fr.untyped as usize].pa_base() + u64::from(fr.pa_off))
    }

    /// `size_bits` của Frame (12/21/30).
    pub fn frame_size_bits(&self, cap: CapId) -> Result<u8, ObjError> {
        Ok(self.frame_cap_full(cap)?.1.size_bits)
    }

    /// PA root PageTable của VSpace.
    pub fn vspace_root_pa(&self, cap: CapId) -> Result<u64, ObjError> {
        let (_, vs) = self.vspace_cap_full(cap)?;
        let root = self.object(vs.root)?;
        if root.kind != ObjKind::PageTable || root.len != PAGE_TABLE_BYTES {
            return Err(ObjError::Corrupt);
        }
        Ok(self.untypeds[root.untyped as usize].arena_base + root.off as u64)
    }

    /// ASID nhãn của VSpace (arch dùng khi thật sự ghi `satp`).
    pub fn vspace_asid(&self, cap: CapId) -> Result<u16, ObjError> {
        Ok(self.vspace_cap_full(cap)?.1.asid)
    }

    /// `(maps, mapped_w, mapped_x)` của Frame — TAP đọc để chứng minh W^X atomic.
    pub fn frame_counts(&self, cap: CapId) -> Result<(u16, u16, u16), ObjError> {
        let (_, fr) = self.frame_cap_full(cap)?;
        Ok((fr.maps, fr.mapped_w, fr.mapped_x))
    }

    /// Số mapping **Device** đang sống trên Frame (KT-56 — bất biến toàn cục D2-67).
    pub fn frame_device(&self, cap: CapId) -> Result<u16, ObjError> {
        let (_, fr) = self.frame_cap_full(cap)?;
        Ok(fr.mapped_device)
    }

    /// Đọc 8 B vật lý qua [`PhysMem`] — chẩn đoán/TAP (vd. chứng minh Frame 4 KiB/2 MiB là
    /// RAM thật bằng cách ghi/đọc tại `pa`). Không cấp quyền mới: cùng cửa `PhysMem` của PTE.
    pub fn read_phys_u64(&self, pa: u64) -> Option<u64> {
        self.mem.read_u64(pa)
    }

    /// Ghi 8 B vật lý qua [`PhysMem`] — xem [`Machine::read_phys_u64`].
    pub fn write_phys_u64(&mut self, pa: u64, value: u64) -> bool {
        self.mem.write_u64(pa, value)
    }

    /// Thống kê byte của Untyped sau cap.
    pub fn untyped_stats(&self, cap: CapId) -> Result<UntypedStats, ObjError> {
        let (u_idx, _) = self.untyped_cap(cap)?;
        let u = &self.untypeds[u_idx as usize];
        let mut used = 0usize;
        let mut frames = 0usize;
        let mut objects = 0usize;
        // `pa_used` = **Σ cỡ Frame đang sống** suy từ bảng object — đường **độc lập** với
        // watermark của allocator (nhờ vậy TAP so được hai nguồn, không tautology).
        let mut pa_live = 0u64;
        for obj in &self.objects {
            if !obj.live || obj.untyped != u_idx || obj.kind == ObjKind::Untyped {
                continue;
            }
            if obj.kind == ObjKind::Frame {
                frames += 1;
                pa_live += 1u64 << obj.size_bits;
            } else {
                used += obj.len as usize;
                objects += 1;
            }
        }
        Ok(UntypedStats {
            capacity: u.capacity as usize,
            used,
            free: (u.capacity as usize).saturating_sub(used),
            holes: u.n_holes as usize,
            watermark: u.bump as usize,
            pa_used: pa_live,
            frames,
            objects,
        })
    }

    /// **KT-67** — ảnh chụp cửa sổ PA của Untyped (cầu nối D2-83/memsrv): mọi trường đọc
    /// **thẳng** từ [`vifast_alloc::FrameArena`] mà object layer giữ, không phải allocator thứ
    /// hai — nhờ vậy `check_objmem`, TAP và memsrv không thể lệch số.
    pub fn frame_budget(&self, cap: CapId) -> Result<FrameBudget, ObjError> {
        let (u_idx, _) = self.untyped_cap(cap)?;
        let arena = &self.untypeds[u_idx as usize].pa;
        Ok(FrameBudget {
            base: arena.base(),
            capacity: u64::from(arena.capacity()),
            used: u64::from(arena.used_bytes()),
            hole_bytes: u64::from(arena.hole_bytes()),
            free_bytes: u64::from(arena.free_bytes()),
            holes: arena.hole_count(),
            live_4k: arena.live_frames(12),
            live_2m: arena.live_frames(21),
            live_1g: arena.live_frames(30),
            allocs: arena.alloc_count(),
            frees: arena.free_count(),
        })
    }

    /// Lấy các yêu cầu flush đang chờ (theo thứ tự) và xoá hàng đợi.
    pub fn drain_flushes(&mut self, mut f: impl FnMut(FlushRequest)) -> usize {
        let mut n = 0usize;
        for &fl in self.flushes.iter().take(self.n_flushes as usize) {
            let req = match fl.kind {
                FLUSH_PAGE => FlushRequest::Page {
                    asid: fl.asid,
                    va: fl.va,
                },
                FLUSH_ASID => FlushRequest::Asid { asid: fl.asid },
                _ => FlushRequest::All,
            };
            f(req);
            n += 1;
        }
        self.n_flushes = 0;
        self.flushes = [Flush {
            kind: 0,
            asid: 0,
            va: 0,
        }; MAX_FLUSHES];
        n
    }
}
