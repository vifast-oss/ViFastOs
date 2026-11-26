// SPDX-License-Identifier: MIT OR Apache-2.0
//! VSpace: map/unmap/protect/walk trên PTE Sv39 + bảng trung gian cấp từ Untyped (KT-52).
//!
//! Mọi thay đổi PTE đều kiểm **trước** (atomic) rồi đẩy yêu cầu `sfence.vma` cho arch
//! (`vifast_plat_riscv64::obj::run_flush`) — không tự gọi asm ở crate platform-common.

// **KT-409** — `is_canonical` + bit PTE Sv39 nay ở `vifast-pte` (định dạng).
use vifast_pte::{
    is_canonical, walk_shape, PAGE_SIZE_BITS, PTE_EXEC, PTE_LEVELS, PTE_PPN_SHIFT, PTE_READ,
    PTE_VALID, PTE_WRITE,
};

use super::*;

/// **KT-417d** — kết quả cài bảng mức lá của `PageTable.Map`, để rollback **đúng phần vừa đổi**.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LeafInstall {
    /// Bảng đã đứng sẵn ở mức lá của `va` từ **trước** lời gọi (không có gì để rollback).
    Already,
    /// Vừa nối entry của **bảng mức 1 có sẵn** trỏ tới bảng lá.
    Linked,
    /// Vừa cấp **bảng mức 1** mới (object + block) làm cha của bảng lá.
    Created {
        /// Slot object meta của bảng mức 1 vừa cấp.
        obj_slot: u16,
        /// Offset block trong Untyped của VSpace.
        off: u32,
        /// Untyped đã cấp block (Untyped của VSpace).
        untyped: u8,
    },
}

impl<M: PhysMem> Machine<M> {
    // ------------------------------------------------------------------------------------ VSpace

    /// Map `frame` vào `vspace` tại `va`, cấp bảng trung gian còn thiếu từ Untyped của VSpace.
    ///
    /// Kiểm **trước** mọi thay đổi (atomic): cap/loại, `va` canonical + căn cỡ Frame, cờ hợp lệ,
    /// VA chưa map, **W^X toàn cục** (D2-67): `W` khi Frame đã `mapped_x > 0`, `X` khi
    /// `mapped_w > 0`, hoặc `Device + EXEC` ⇒ [`ObjError::WxViolation`].
    pub fn map(
        &mut self,
        vspace: CapId,
        va: u64,
        frame: CapId,
        flags: MapFlags,
    ) -> Result<(), ObjError> {
        let (vs_idx, vs) = self.vspace_cap_full(vspace)?;
        let (fr_idx, fr) = self.frame_cap_full(frame)?;
        let level = frame_level(fr.size_bits)?;
        let size = 1u64 << fr.size_bits;
        if !flags.is_valid() {
            return Err(ObjError::InvalidArgument);
        }
        if !is_canonical(va) || va & (size - 1) != 0 {
            return Err(ObjError::Alignment);
        }
        if flags.device && flags.exec {
            return Err(ObjError::WxViolation);
        }
        // KT-55 (conformance bắt được): **một** mapping `W|X` cùng lúc cũng là W^X — nếu cho qua
        // thì `mapped_w` và `mapped_x` cùng tăng trên một Frame, tức chính object layer tạo ra
        // trạng thái mà `check_objmem` (D2-67) coi là sai. K2 trả `WxViolation`; kernel khớp.
        if flags.write && flags.exec {
            return Err(ObjError::WxViolation);
        }
        // L7 (reviewer): cửa sổ PA của Frame 1 GiB trên `-m 128M` **trùm RAM thật** (nhãn
        // D2-66); chưa cho map Frame 1 GiB tới K4/K5 (K3 chỉ kiểm nhãn ở TAP 32).
        if fr.size_bits == PAGE_SIZE_BITS[2] {
            return Err(ObjError::NotSupported);
        }
        if flags.write && fr.mapped_x > 0 {
            return Err(ObjError::WxViolation);
        }
        if flags.exec && fr.mapped_w > 0 {
            return Err(ObjError::WxViolation);
        }
        // KT-56 (nợ L3 KT-52): **Device toàn cục** — một Frame đang là Device ở mapping khác
        // không được trở thành executable, và ngược lại. Trước KT-56 chỉ kiểm cờ của **một**
        // mapping nên `Device(RW)` ở vs0 + `RX` ở vs1 lọt qua; `check_objmem` không có bộ đếm
        // Device nên cũng không bắt. Đây là bất biến của D2-67 ("toàn cục").
        if flags.exec && fr.mapped_device > 0 {
            return Err(ObjError::WxViolation);
        }
        if flags.device && fr.mapped_x > 0 {
            return Err(ObjError::WxViolation);
        }
        if self.va_conflict(vs_idx, va, size) {
            return Err(ObjError::AlreadyMapped);
        }
        let (root_pa, missing) = self.missing_tables(&vs, va, level)?;
        // Cấp slot trước mọi thay đổi (atomic).
        let mut reserved = [OBJ_NONE; PTE_LEVELS - 1];
        let mut n_res = 0usize;
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live && n_res < missing {
                reserved[n_res] = i as u16;
                n_res += 1;
            }
        }
        if n_res < missing {
            return Err(ObjError::NoSlots);
        }
        let map_slot = self.free_mapping_slot()?;
        self.ensure_flush_slots(1)?;
        // Cấp block cho bảng thiếu (chưa ghi PTE; lỗi ⇒ trả lại block).
        let table_untyped = self.vspace_untyped(&vs)?;
        let mut block_pa = [0u64; PTE_LEVELS - 1];
        let mut block_off = [0u32; PTE_LEVELS - 1];
        let mut n_block = 0usize;
        while n_block < missing {
            let off = match self.untypeds[table_untyped as usize]
                .alloc(PAGE_TABLE_BYTES, PAGE_TABLE_BYTES)
            {
                Some(off) => off,
                None => {
                    self.release_blocks(table_untyped, &block_off, n_block);
                    return Err(ObjError::NotEnoughMemory);
                }
            };
            let pa = self.untypeds[table_untyped as usize].arena_base + off as u64;
            if !zero_block(&mut self.mem, pa, PAGE_TABLE_BYTES as usize) {
                let _ = self.untypeds[table_untyped as usize].free(off, PAGE_TABLE_BYTES);
                self.release_blocks(table_untyped, &block_off, n_block);
                return Err(ObjError::Corrupt);
            }
            block_off[n_block] = off;
            block_pa[n_block] = pa;
            n_block += 1;
        }
        // Ghi đường đi: bảng thiếu được nối dần từ gốc xuống.
        let indices = walk_shape(va).indices();
        let mut table = root_pa;
        let mut used_blocks = 0usize;
        let mut written = [(0u64, 0usize); PTE_LEVELS - 1];
        let mut n_written = 0usize;
        let mut fail = false;
        for lvl in (level as usize + 1..PTE_LEVELS).rev() {
            let index = indices[PTE_LEVELS - 1 - lvl] as usize;
            let raw = match self.pte_load(table, index) {
                Some(raw) => raw,
                None => {
                    fail = true;
                    break;
                }
            };
            if raw & PTE_VALID == 0 {
                if used_blocks >= n_block {
                    fail = true;
                    break;
                }
                let new_pa = block_pa[used_blocks];
                let obj_idx = reserved[used_blocks];
                used_blocks += 1;
                let table_pte = ((new_pa >> 12) << PTE_PPN_SHIFT) | PTE_VALID;
                if !self.pte_store(table, index, table_pte) {
                    fail = true;
                    break;
                }
                written[n_written] = (table, index);
                n_written += 1;
                let off = block_off[used_blocks - 1];
                let id = self.take_id();
                self.objects[obj_idx as usize] = Object {
                    live: true,
                    kind: ObjKind::PageTable,
                    id,
                    untyped: table_untyped,
                    cap: CAP_NONE,
                    off,
                    len: PAGE_TABLE_BYTES,
                    owner: vs_idx,
                    ..Object::EMPTY
                };
                table = new_pa;
            } else if raw & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0 {
                fail = true;
                break;
            } else {
                table = (raw >> PTE_PPN_SHIFT) << 12;
            }
        }
        if !fail {
            let leaf_index = indices[PTE_LEVELS - 1 - level as usize] as usize;
            match self.pte_load(table, leaf_index) {
                Some(raw) if raw & PTE_VALID != 0 => fail = true,
                Some(_) => {
                    let frame_pa =
                        self.untypeds[fr.untyped as usize].pa_base() + u64::from(fr.pa_off);
                    let pte = ((frame_pa >> 12) << PTE_PPN_SHIFT) | flags.bits();
                    if !self.pte_store(table, leaf_index, pte) {
                        fail = true;
                    } else {
                        // Commit: mapping + đếm Frame + flush.
                        self.mappings[map_slot as usize] =
                            Mapping::new(vs_idx, fr_idx, va, level, flags);
                        let mut f = fr;
                        f.maps += 1;
                        if flags.write {
                            f.mapped_w += 1;
                        }
                        if flags.exec {
                            f.mapped_x += 1;
                        }
                        if flags.device {
                            f.mapped_device += 1;
                        }
                        self.objects[fr_idx as usize] = f;
                        if !self.push_flush(Flush {
                            kind: FLUSH_PAGE,
                            asid: vs.asid,
                            va,
                        }) {
                            // Đã `ensure_flush_slots(1)` trước mutation; nhánh này là lỗi nội bộ.
                            return Err(ObjError::NoSlots);
                        }
                    }
                }
                None => fail = true,
            }
        }
        if fail {
            // Rollback: xoá PTE đã nối, trả block chưa dùng, xoá object meta vừa tạo.
            for &(t, i) in written[..n_written].iter().rev() {
                let _ = self.pte_store(t, i, 0);
            }
            for &off in block_off[..n_block].iter() {
                // Block đã dùng (object meta) cũng phải trả: meta vừa tạo khớp `reserved[k]`.
                let _ = self.untypeds[table_untyped as usize].free(off, PAGE_TABLE_BYTES);
            }
            for &idx in reserved[..n_res].iter() {
                self.objects[idx as usize] = Object::EMPTY;
            }
            return Err(ObjError::Corrupt);
        }
        Ok(())
    }

    /// Gỡ mapping tại `va` (phải tồn tại), xoá PTE, trả đếm Frame, sinh `sfence` trang.
    pub fn unmap(&mut self, vspace: CapId, va: u64) -> Result<(), ObjError> {
        let (vs_idx, vs) = self.vspace_cap_full(vspace)?;
        let slot = self.find_mapping(vs_idx, va).ok_or(ObjError::NotMapped)?;
        self.ensure_flush_slots(1)?;
        let map = self.mappings[slot];
        let level = map.level;
        let (root_pa, _) = self.missing_tables(&vs, va, level)?;
        let indices = walk_shape(va).indices();
        let mut table = root_pa;
        for lvl in (level + 1..PTE_LEVELS as u8).rev() {
            let index = indices[PTE_LEVELS - 1 - lvl as usize] as usize;
            let raw = self.pte_load(table, index).ok_or(ObjError::Corrupt)?;
            if raw & PTE_VALID == 0 {
                return Err(ObjError::NotMapped);
            }
            table = (raw >> PTE_PPN_SHIFT) << 12;
        }
        let leaf_index = indices[PTE_LEVELS - 1 - level as usize] as usize;
        let raw = self.pte_load(table, leaf_index).ok_or(ObjError::Corrupt)?;
        if raw & PTE_VALID == 0 {
            return Err(ObjError::NotMapped);
        }
        if !self.pte_store(table, leaf_index, 0) {
            return Err(ObjError::Corrupt);
        }
        if let Ok(frame) = self.object(map.frame) {
            let flags = map.flags();
            let mut f = frame;
            f.maps = f.maps.saturating_sub(1);
            if flags.write {
                f.mapped_w = f.mapped_w.saturating_sub(1);
            }
            if flags.exec {
                f.mapped_x = f.mapped_x.saturating_sub(1);
            }
            if flags.device {
                f.mapped_device = f.mapped_device.saturating_sub(1);
            }
            self.objects[map.frame as usize] = f;
        }
        self.mappings[slot].live = false;
        if !self.push_flush(Flush {
            kind: FLUSH_PAGE,
            asid: vs.asid,
            va,
        }) {
            return Err(ObjError::NoSlots);
        }
        Ok(())
    }

    // ------------------------------------------------------- KT-417d — PageTable.{Map,Unmap}

    /// **KT-417d (D2-262 nhóm 4, ABI `PageTable` id 10 method 0)** — `PageTable.Map`: cài
    /// **bảng trang 4 KiB do cap `pt` sở hữu** làm bảng **mức lá** cho `va` trong `vspace`, rồi
    /// map `frame` **bằng chính bảng đó** với `flags`.
    ///
    /// Khác [`Self::map`] ở **chủ sở hữu bảng**: `Frame.Map` để VSpace tự cấp bảng trung gian từ
    /// Untyped của nó; ở đây bảng mức lá là object **của caller** (retype từ Untyped của caller),
    /// nên mở đường cấp quyền sở hữu bảng cho pager userland ở Chặng 3. Bảng mức 1 (cha) vẫn do
    /// VSpace cấp nếu thiếu, và bị **thu hồi cùng VSpace** (`owner`).
    ///
    /// Kiểm **trước** mọi thay đổi (atomic): loại cap, **cỡ `frame` = đúng 4 KiB**, cờ hợp lệ,
    /// `va` canonical + căn 4 KiB, W^X; sau đó cài bảng rồi mới map — map hỏng ⇒ **tháo bảng vừa
    /// cài** (không để nửa vời).
    ///
    /// **F2 (review W8a)** — `frame` cỡ khác 4 KiB (2 MiB/1 GiB, `Retype` hợp lệ) bị từ chối
    /// **ngay** bằng [`ObjError::NotSupported`] *trước* khi cài bảng lá / cấp bảng mức 1: PTE
    /// siêu trang ghi vào ô `leaf_pte` sẽ gặp `pte_load != 0` ⇒ `Corrupt` (rò mã 5 ở ABI) và
    /// `PageTable.Unmap` cùng bộ cap không đối xứng. Method này chỉ mở đường **lá 4 KiB** do
    /// caller sở hữu; lá lớn vẫn đi `Frame.Map` (VSpace tự cấp bảng).
    pub fn map_page_table(
        &mut self,
        pt: CapId,
        vspace: CapId,
        va: u64,
        frame: CapId,
        flags: MapFlags,
    ) -> Result<(), ObjError> {
        let (vs_idx, vs) = self.vspace_cap_full(vspace)?;
        let pt_rec = self.cap(pt)?;
        if pt_rec.kind != ObjKind::PageTable {
            return Err(ObjError::WrongObjectType);
        }
        let pt_obj = self.object(pt_rec.obj)?;
        if pt_obj.kind != ObjKind::PageTable || pt_obj.len != PAGE_TABLE_BYTES {
            return Err(ObjError::WrongObjectType);
        }
        // Bảng đã thuộc **VSpace khác** ⇒ không dùng chéo (D2-83: một chủ sở hữu).
        if pt_obj.owner != VS_NONE && pt_obj.owner != vs_idx {
            return Err(ObjError::Busy);
        }
        if !flags.is_valid() {
            return Err(ObjError::InvalidArgument);
        }
        if !is_canonical(va) || va & (PAGE_TABLE_BYTES as u64 - 1) != 0 {
            return Err(ObjError::Alignment);
        }
        if flags.device && flags.exec {
            return Err(ObjError::WxViolation);
        }
        if flags.write && flags.exec {
            return Err(ObjError::WxViolation);
        }
        let (fr_idx, fr) = self.frame_cap_full(frame)?;
        if fr.kind != ObjKind::Frame {
            return Err(ObjError::WrongObjectType);
        }
        // **F2 (review W8a)** — tiền điều kiện **cỡ Frame**: chỉ lá 4 KiB. Từ chối **trước mọi
        // thay đổi** (không cài bảng, không cấp bảng mức 1): Frame 2 MiB sẽ được `map` tính
        // `level = 1` rồi ghi PTE siêu trang vào đúng ô vừa đặt `leaf_pte` ⇒ `Corrupt` (và qua
        // cầu invoke rò thành 5), còn `PageTable.Unmap` cùng bộ cap luôn `NotMapped`.
        if fr.size_bits != PAGE_SIZE_BITS[0] {
            return Err(ObjError::NotSupported);
        }
        // Cài bảng mức lá (cấp bảng mức 1 nếu thiếu) rồi map Frame qua đúng bảng ấy.
        let installed = self.install_leaf_table(vs_idx, &vs, va, pt_rec.obj)?;
        match self.map(vspace, va, frame, flags) {
            Ok(()) => Ok(()),
            Err(e) => {
                // Rollback: tháo bảng vừa cài để không để lại bảng mồ côi trong VSpace.
                let _ = self.rollback_leaf_table(vs_idx, &vs, va, pt_rec.obj, installed);
                let _ = fr_idx;
                Err(e)
            }
        }
    }

    /// **KT-417d (ABI `PageTable` id 10 method 1)** — `PageTable.Unmap`: gỡ lá 4 KiB tại `va`
    /// **qua bảng `pt`**. Yêu cầu bảng đã cài đúng ở mức lá của `va` trong `vspace` — sai bảng ⇒
    /// [`ObjError::MissingPageTable`] (không im lặng), chưa cài ⇒ [`ObjError::InvalidCapability`].
    pub fn unmap_page_table(&mut self, pt: CapId, vspace: CapId, va: u64) -> Result<(), ObjError> {
        let (vs_idx, vs) = self.vspace_cap_full(vspace)?;
        let pt_rec = self.cap(pt)?;
        if pt_rec.kind != ObjKind::PageTable {
            return Err(ObjError::WrongObjectType);
        }
        let pt_obj = self.object(pt_rec.obj)?;
        if pt_obj.owner != vs_idx {
            return Err(ObjError::InvalidCapability);
        }
        if !is_canonical(va) || va & (PAGE_TABLE_BYTES as u64 - 1) != 0 {
            return Err(ObjError::Alignment);
        }
        // Bảng phải đang đứng ở mức lá của `va` trong VSpace này.
        let (root_pa, missing) = match self.missing_tables(&vs, va, 0) {
            Ok(v) => v,
            Err(ObjError::AlreadyMapped) => return Err(ObjError::NotMapped),
            Err(e) => return Err(e),
        };
        if missing != 0 {
            return Err(ObjError::MissingPageTable);
        }
        if self.leaf_table_pa(root_pa, va) != self.obj_pa_of(&pt_obj) {
            return Err(ObjError::MissingPageTable);
        }
        let slot = self.find_mapping(vs_idx, va).ok_or(ObjError::NotMapped)?;
        if self.mappings[slot].level != 0 {
            return Err(ObjError::NotMapped);
        }
        self.unmap(vspace, va)
    }

    /// PA của bảng trang mức lá (level 0) đi từ `root_pa` tại `va` — chuỗi phải đủ.
    fn leaf_table_pa(&self, root_pa: u64, va: u64) -> u64 {
        let indices = walk_shape(va).indices();
        let mut table = root_pa;
        for lvl in (1..PTE_LEVELS).rev() {
            let index = indices[PTE_LEVELS - 1 - lvl] as usize;
            let Some(raw) = self.pte_load(table, index) else {
                return 0;
            };
            table = (raw >> PTE_PPN_SHIFT) << 12;
        }
        table
    }

    /// Cài bảng `pt_idx` làm bảng **mức lá** cho `va` (cấp bảng mức 1 nếu thiếu).
    ///
    /// Trả [`LeafInstall`] để caller **rollback đúng phần vừa đổi**: bảng đã cài sẵn từ trước thì
    /// **không** được tháo khi lệnh map sau đó hỏng.
    fn install_leaf_table(
        &mut self,
        vs_idx: u8,
        vs: &Object,
        va: u64,
        pt_idx: u16,
    ) -> Result<LeafInstall, ObjError> {
        let pt_obj = self.object(pt_idx)?;
        let pt_pa = self.obj_pa_of(&pt_obj);
        let (root_pa, missing) = self.missing_tables(vs, va, 0)?;
        let indices = walk_shape(va).indices();
        let i_root = indices[0] as usize;
        let i_l1 = indices[1] as usize;
        let leaf_pte = ((pt_pa >> 12) << PTE_PPN_SHIFT) | PTE_VALID;
        if missing == 0 {
            // Chuỗi đủ: bảng mức lá **phải chính là** `pt`, ngược lại không được ghi đè.
            if self.leaf_table_pa(root_pa, va) != pt_pa {
                return Err(ObjError::AlreadyMapped);
            }
            return Ok(LeafInstall::Already);
        }
        // missing ∈ {1,2}: cần bảng mức 1 làm cha của bảng lá.
        let raw_root = self.pte_load(root_pa, i_root).ok_or(ObjError::Corrupt)?;
        if raw_root & PTE_VALID != 0 {
            // Bảng mức 1 đã có ⇒ chỉ nối entry của nó tới bảng lá.
            let l1 = (raw_root >> PTE_PPN_SHIFT) << 12;
            let raw = self.pte_load(l1, i_l1).ok_or(ObjError::Corrupt)?;
            if raw & PTE_VALID != 0 {
                return Err(ObjError::AlreadyMapped);
            }
            if !self.pte_store(l1, i_l1, leaf_pte) {
                return Err(ObjError::Corrupt);
            }
            self.objects[pt_idx as usize].owner = vs_idx;
            return Ok(LeafInstall::Linked);
        }
        // Cấp bảng mức 1 từ Untyped của VSpace (khuôn `map`: reserve slot trước, atomic).
        let u = self.vspace_untyped(vs)?;
        let obj_slot = self.free_object_slot()?;
        let Some(off) = self.untypeds[u as usize].alloc(PAGE_TABLE_BYTES, PAGE_TABLE_BYTES) else {
            return Err(ObjError::NotEnoughMemory);
        };
        let pa = self.untypeds[u as usize].arena_base + u64::from(off);
        if !zero_block(&mut self.mem, pa, PAGE_TABLE_BYTES as usize) {
            let _ = self.untypeds[u as usize].free(off, PAGE_TABLE_BYTES);
            return Err(ObjError::Corrupt);
        }
        let id = self.take_id();
        self.objects[obj_slot as usize] = Object {
            live: true,
            kind: ObjKind::PageTable,
            id,
            untyped: u,
            cap: CAP_NONE,
            off,
            len: PAGE_TABLE_BYTES,
            owner: vs_idx,
            ..Object::EMPTY
        };
        let l1_pte = ((pa >> 12) << PTE_PPN_SHIFT) | PTE_VALID;
        if !self.pte_store(root_pa, i_root, l1_pte) || !self.pte_store(pa, i_l1, leaf_pte) {
            let _ = self.pte_store(root_pa, i_root, 0);
            let _ = self.pte_store(pa, i_l1, 0);
            self.objects[obj_slot as usize] = Object::EMPTY;
            let _ = self.untypeds[u as usize].free(off, PAGE_TABLE_BYTES);
            return Err(ObjError::Corrupt);
        }
        self.objects[pt_idx as usize].owner = vs_idx;
        Ok(LeafInstall::Created {
            obj_slot,
            off,
            untyped: u,
        })
    }

    /// Tháo **đúng phần vừa đổi** ở [`Self::install_leaf_table`] khi `map` hỏng (atomic theo
    /// nghĩa quan sát được: byte arena, PTE và `owner` trở về trạng thái trước lời gọi).
    fn rollback_leaf_table(
        &mut self,
        vs_idx: u8,
        vs: &Object,
        va: u64,
        pt_idx: u16,
        installed: LeafInstall,
    ) -> Result<(), ObjError> {
        if installed == LeafInstall::Already {
            // Bảng đã cài từ trước lời gọi này ⇒ không phải thay đổi của ta, không tháo.
            return Ok(());
        }
        let (root_pa, _) = self.missing_tables(vs, va, 0)?;
        let indices = walk_shape(va).indices();
        let i_root = indices[0] as usize;
        let i_l1 = indices[1] as usize;
        let root_raw = self.pte_load(root_pa, i_root).ok_or(ObjError::Corrupt)?;
        let l1 = (root_raw >> PTE_PPN_SHIFT) << 12;
        match installed {
            LeafInstall::Linked => {
                // Ta vừa ghi entry ở bảng mức 1 có sẵn ⇒ gỡ nó.
                if root_raw & PTE_VALID != 0 {
                    let _ = self.pte_store(l1, i_l1, 0);
                }
            }
            LeafInstall::Created {
                obj_slot,
                off,
                untyped,
            } => {
                // Ta vừa cấp bảng mức 1 ⇒ gỡ entry gốc, trả block + object meta.
                let _ = self.pte_store(root_pa, i_root, 0);
                self.objects[obj_slot as usize] = Object::EMPTY;
                let _ = self.untypeds[untyped as usize].free(off, PAGE_TABLE_BYTES);
            }
            LeafInstall::Already => return Ok(()),
        }
        // Bảng lá trở lại "bảng rời" (chưa thuộc VSpace).
        if let Ok(mut pt) = self.object(pt_idx) {
            if pt.owner == vs_idx {
                pt.owner = VS_NONE;
                self.objects[pt_idx as usize] = pt;
            }
        }
        Ok(())
    }

    /// Đổi cờ một mapping đã có, kiểm W^X **trên phần còn lại** của Frame (D2-67).
    pub fn protect(&mut self, vspace: CapId, va: u64, flags: MapFlags) -> Result<(), ObjError> {
        let (vs_idx, vs) = self.vspace_cap_full(vspace)?;
        if !flags.is_valid() {
            return Err(ObjError::InvalidArgument);
        }
        if flags.device && flags.exec {
            return Err(ObjError::WxViolation);
        }
        // KT-55: cùng lý do với `map` — `protect` sang `W|X` trên **một** mapping cũng phá W^X.
        if flags.write && flags.exec {
            return Err(ObjError::WxViolation);
        }
        let slot = self.find_mapping(vs_idx, va).ok_or(ObjError::NotMapped)?;
        self.ensure_flush_slots(1)?;
        let map = self.mappings[slot];
        let fr = self.object(map.frame)?;
        let old = map.flags();
        let rest_w = fr.mapped_w.saturating_sub(u16::from(old.write));
        let rest_x = fr.mapped_x.saturating_sub(u16::from(old.exec));
        if flags.write && rest_x > 0 {
            return Err(ObjError::WxViolation);
        }
        if flags.exec && rest_w > 0 {
            return Err(ObjError::WxViolation);
        }
        // KT-56: cùng bất biến Device toàn cục khi `protect` (Frame đã Device ở mapping khác).
        let rest_dev = fr.mapped_device.saturating_sub(u16::from(old.device));
        if flags.exec && rest_dev > 0 {
            return Err(ObjError::WxViolation);
        }
        if flags.device && rest_x > 0 {
            return Err(ObjError::WxViolation);
        }
        let level = map.level;
        let (root_pa, _) = self.missing_tables(&vs, va, level)?;
        let indices = walk_shape(va).indices();
        let mut table = root_pa;
        for lvl in (level + 1..PTE_LEVELS as u8).rev() {
            let index = indices[PTE_LEVELS - 1 - lvl as usize] as usize;
            let raw = self.pte_load(table, index).ok_or(ObjError::Corrupt)?;
            if raw & PTE_VALID == 0 {
                return Err(ObjError::NotMapped);
            }
            table = (raw >> PTE_PPN_SHIFT) << 12;
        }
        let leaf_index = indices[PTE_LEVELS - 1 - level as usize] as usize;
        let raw = self.pte_load(table, leaf_index).ok_or(ObjError::Corrupt)?;
        if raw & PTE_VALID == 0 {
            return Err(ObjError::NotMapped);
        }
        let frame_pa = (raw >> PTE_PPN_SHIFT) << 12;
        let pte = ((frame_pa >> 12) << PTE_PPN_SHIFT) | flags.bits();
        if !self.pte_store(table, leaf_index, pte) {
            return Err(ObjError::Corrupt);
        }
        let mut f = fr;
        f.mapped_w = f.mapped_w - u16::from(old.write) + u16::from(flags.write);
        f.mapped_x = f.mapped_x - u16::from(old.exec) + u16::from(flags.exec);
        f.mapped_device = f.mapped_device - u16::from(old.device) + u16::from(flags.device);
        self.objects[map.frame as usize] = f;
        self.mappings[slot] = Mapping::new(vs_idx, map.frame, va, level, flags);
        if !self.push_flush(Flush {
            kind: FLUSH_PAGE,
            asid: vs.asid,
            va,
        }) {
            return Err(ObjError::NoSlots);
        }
        Ok(())
    }

    // ------------------------------------------------------------------------------------- Walk

    /// Walk PTE phần mềm trên root của `vspace` (đọc qua [`PhysMem`]).
    pub fn walk(&self, vspace: CapId, va: u64) -> Result<WalkInfo, ObjError> {
        let (_, vs) = self.vspace_cap_full(vspace)?;
        self.walk_obj(&vs, va)
    }

    // ------------------------------------------------------------------------- nội bộ (helper)

    fn va_conflict(&self, vs_idx: u8, va: u64, size: u64) -> bool {
        for map in self.mappings.iter() {
            if !map.live || map.vspace != vs_idx {
                continue;
            }
            let other = 1u64 << self.objects[map.frame as usize].size_bits;
            if va < map.va + other && map.va < va + size {
                return true;
            }
        }
        false
    }

    /// Đi tới chỗ thiếu bảng: trả `(root_pa, số bảng phải cấp)` — chỉ đọc.
    fn missing_tables(&self, vs: &Object, va: u64, level: u8) -> Result<(u64, usize), ObjError> {
        let root = self.object(vs.root)?;
        let root_pa = self.untypeds[root.untyped as usize].arena_base + root.off as u64;
        let indices = walk_shape(va).indices();
        let mut table = root_pa;
        for lvl in (level as usize + 1..PTE_LEVELS).rev() {
            let index = indices[PTE_LEVELS - 1 - lvl] as usize;
            let raw = self.pte_load(table, index).ok_or(ObjError::Corrupt)?;
            if raw & PTE_VALID == 0 {
                return Ok((root_pa, lvl - level as usize));
            }
            if raw & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0 {
                return Err(ObjError::AlreadyMapped);
            }
            table = (raw >> PTE_PPN_SHIFT) << 12;
        }
        Ok((root_pa, 0))
    }

    /// PA của một object arena (bảng trang) — `arena_base + off`.
    fn obj_pa_of(&self, obj: &Object) -> u64 {
        self.untypeds[obj.untyped as usize].arena_base + u64::from(obj.off)
    }

    fn vspace_untyped(&self, vs: &Object) -> Result<u8, ObjError> {
        let root = self.object(vs.root)?;
        Ok(root.untyped)
    }

    fn release_blocks(&mut self, u_idx: u8, offs: &[u32; PTE_LEVELS - 1], n: usize) {
        for k in (0..n).rev() {
            let _ = self.untypeds[u_idx as usize].free(offs[k], PAGE_TABLE_BYTES);
        }
    }

    fn find_mapping(&self, vs_idx: u8, va: u64) -> Option<usize> {
        for (i, map) in self.mappings.iter().enumerate() {
            if map.live && map.vspace == vs_idx && map.va == va {
                return Some(i);
            }
        }
        None
    }
    pub(super) fn walk_obj(&self, vs: &Object, va: u64) -> Result<WalkInfo, ObjError> {
        let root = self.object(vs.root)?;
        let root_pa = self.untypeds[root.untyped as usize].arena_base + root.off as u64;
        self.walk_root(root_pa, va)
    }

    pub(super) fn walk_root(&self, root_pa: u64, va: u64) -> Result<WalkInfo, ObjError> {
        let indices = walk_shape(va).indices();
        let mut table = root_pa;
        for level in (0..PTE_LEVELS).rev() {
            let index = indices[PTE_LEVELS - 1 - level] as usize;
            let raw = self.pte_load(table, index).ok_or(ObjError::Corrupt)?;
            if raw & PTE_VALID == 0 {
                return Ok(WalkInfo {
                    valid: false,
                    leaf: false,
                    level: level as u8,
                    phys: 0,
                    raw,
                });
            }
            if raw & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0 {
                return Ok(WalkInfo {
                    valid: true,
                    leaf: true,
                    level: level as u8,
                    phys: (raw >> PTE_PPN_SHIFT) << 12,
                    raw,
                });
            }
            if level == 0 {
                return Ok(WalkInfo {
                    valid: true,
                    leaf: false,
                    level: 0,
                    phys: 0,
                    raw,
                });
            }
            table = (raw >> PTE_PPN_SHIFT) << 12;
        }
        Err(ObjError::Corrupt)
    }
}
