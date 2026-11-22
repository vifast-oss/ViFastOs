// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `pagetable`: Frame + PageTable: map/walk/unmap/protect, W^X, kiểm bảng trang.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------------------- Frame (KT-36)

    /// **KT-36** — `Frame.GetAddress` qua **cap** (method 2 của `spec/abi.toml`, vẫn `reserved`
    /// nên đây là semantic **sau model API**, không đổi spec).
    ///
    /// Cap phải **sống** và trỏ `Frame` (`WrongObjectType`); Frame đã chết ⇒ `InvalidCapability`.
    /// Trả `pa` của mô hình (địa chỉ vật lý **ảo** — Frame không có vùng nhớ thật ở K2).
    pub fn frame_get_address(&self, cap: SlotRef) -> Result<u64, KError> {
        let obj = self.require_type(cap, ObjType::Frame)?;
        self.frame_address(obj)
    }

    /// **KT-36** — `pa` của Frame theo `ObjId` (model API cho KT-37/KT-39; `GetAddress` qua cap
    /// là [`Machine::frame_get_address`]).
    pub fn frame_address(&self, frame: ObjId) -> Result<u64, KError> {
        let m = self.meta(frame)?;
        if m.ty != ObjType::Frame {
            return Err(KError::WrongObjectType);
        }
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        m.frame.map(|f| f.pa).ok_or(KError::WrongObjectType)
    }

    /// **KT-36** — ảnh chụp [`FrameMeta`] của Frame, **kể cả** Frame đã chết (`live = false` +
    /// bộ đếm 0 — DoD "frame chết sạch" phải đọc lại được). Object không phải Frame ⇒
    /// `WrongObjectType`.
    pub fn frame_meta(&self, frame: ObjId) -> Result<FrameMeta, KError> {
        let m = self.meta(frame)?;
        if m.ty != ObjType::Frame {
            return Err(KError::WrongObjectType);
        }
        m.frame.ok_or(KError::WrongObjectType)
    }

    /// **KT-36** — số Frame **còn sống** (không đếm Frame đã chết trong `objects`).
    pub fn frame_count(&self) -> usize {
        self.objects
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::Frame)
            .count()
    }

    /// **KT-36** — Frame **còn sống** thuộc Untyped `untyped`, theo thứ tự cấp phát. Không phải
    /// Untyped ⇒ `vec![]`.
    pub fn untyped_frames(&self, untyped: ObjId) -> Vec<ObjId> {
        let Ok(idx) = self.object_index(untyped) else {
            return Vec::new();
        };
        if self.objects[idx].ty != ObjType::Untyped {
            return Vec::new();
        }
        self.objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::Frame && m.untyped == Some(idx))
            .map(|(i, _)| (i as u32) + 1)
            .collect()
    }

    /// **KT-36** — `(đáy, đỉnh)` cửa sổ PA mà Untyped cấp cho Frame; `None` nếu không phải
    /// Untyped (hoặc Untyped đã chết).
    pub fn untyped_pa_window(&self, untyped: ObjId) -> Option<(u64, u64)> {
        let idx = self.object_index(untyped).ok()?;
        let m = &self.objects[idx];
        if !m.alive || m.ty != ObjType::Untyped {
            return None;
        }
        m.frame_area.and_then(|a| a.window())
    }

    /// **KT-36/KT-39** — ghi nhận **một mapping sống** mới của Frame (`write` = có quyền WRITE,
    /// `exec` = có thuộc tính EXEC). Đây là cửa **duy nhất** của đường **map**; `protect` không
    /// thêm mapping nên đi cửa **delta** riêng trong `frame.rs` — hai cửa, cùng bất biến W^X.
    ///
    /// **Luật W^X toàn cục (KT-39, D2-65 Q3 + D2-67)** — kiểm **trước** mọi thay đổi (atomic):
    ///
    /// - mapping mới là W mà Frame đang có mapping X (`mapped_x > 0`) ⇒ [`KError::WxViolation`];
    /// - mapping mới là X mà Frame đang có mapping W (`mapped_w > 0`) ⇒ [`KError::WxViolation`];
    /// - mapping mới **vừa W vừa X** (một lời gọi) ⇒ [`KError::WxViolation`] — tự nó tạo
    ///   `mapped_w > 0 ∧ mapped_x > 0`.
    ///
    /// **Không** tự unmap/hạ quyền mapping cũ: JIT phải `unmap` W rồi `map` X. Bộ đếm là **toàn
    /// cục theo Frame vật lý**, nên luật đúng cả khi Frame nằm trong nhiều VSpace/PageTable.
    ///
    /// Frame chết ⇒ `InvalidCapability`; không phải Frame ⇒ `WrongObjectType`; tràn `u32` ⇒
    /// `InvalidArgument`.
    pub fn frame_note_map(&mut self, frame: ObjId, write: bool, exec: bool) -> Result<(), KError> {
        let f = self.frame_meta_live_mut(frame)?;
        // **KT-39** — W^X trước mọi thay đổi. Thứ tự đọc bộ đếm rồi mới quyết định, không ghi.
        if write && (exec || f.mapped_x > 0) {
            return Err(KError::WxViolation);
        }
        if exec && f.mapped_w > 0 {
            return Err(KError::WxViolation);
        }
        let maps = f.maps.checked_add(1).ok_or(KError::InvalidArgument)?;
        let mapped_w = f
            .mapped_w
            .checked_add(u32::from(write))
            .ok_or(KError::InvalidArgument)?;
        let mapped_x = f
            .mapped_x
            .checked_add(u32::from(exec))
            .ok_or(KError::InvalidArgument)?;
        f.maps = maps;
        f.mapped_w = mapped_w;
        f.mapped_x = mapped_x;
        Ok(())
    }

    /// **KT-36** — gỡ **một mapping sống** của Frame (đối ứng [`Machine::frame_note_map`]).
    ///
    /// Không có mapping / `mapped_w == 0` khi `write` / `mapped_x == 0` khi `exec` ⇒
    /// `IllegalOperation` (không trừ âm, không im lặng); atomic.
    pub fn frame_note_unmap(
        &mut self,
        frame: ObjId,
        write: bool,
        exec: bool,
    ) -> Result<(), KError> {
        let f = self.frame_meta_live_mut(frame)?;
        if f.maps == 0 || (write && f.mapped_w == 0) || (exec && f.mapped_x == 0) {
            return Err(KError::IllegalOperation);
        }
        f.maps -= 1;
        if write {
            f.mapped_w -= 1;
        }
        if exec {
            f.mapped_x -= 1;
        }
        Ok(())
    }

    /// Frame **sống** + cho ghi metadata (nội bộ; giữ một cửa để hai hàm đếm không lệch lỗi).
    pub(super) fn frame_meta_live_mut(&mut self, frame: ObjId) -> Result<&mut FrameMeta, KError> {
        let idx = self.object_index(frame)?;
        let m = &mut self.objects[idx];
        if m.ty != ObjType::Frame {
            return Err(KError::WrongObjectType);
        }
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        m.frame.as_mut().ok_or(KError::WrongObjectType)
    }

    // ------------------------------------------------------- PageTable (KT-37)

    /// Chỉ số `Machine::page_tables` của object PageTable **sống**; object lạ/chết/không phải
    /// PageTable ⇒ lỗi tương ứng. Đây là **một cửa** để mọi hàm PageTable không lệch cách lấy.
    pub(super) fn page_table_slot_of(&self, pt: ObjId) -> Result<usize, KError> {
        let idx = self.object_index(pt)?;
        let m = &self.objects[idx];
        if m.ty != ObjType::PageTable {
            return Err(KError::WrongObjectType);
        }
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        m.pt.ok_or(KError::WrongObjectType)
    }

    pub(super) fn page_table_data(&self, pt: ObjId) -> Result<&PageTableData, KError> {
        let slot = self.page_table_slot_of(pt)?;
        self.page_tables
            .get(slot)
            .and_then(|d| d.as_ref())
            .ok_or(KError::IllegalOperation)
    }

    pub(super) fn page_table_data_mut(&mut self, pt: ObjId) -> Result<&mut PageTableData, KError> {
        let slot = self.page_table_slot_of(pt)?;
        self.page_tables
            .get_mut(slot)
            .and_then(|d| d.as_mut())
            .ok_or(KError::IllegalOperation)
    }

    /// Bảng **gốc** của cây: chưa attach và ở cấp 2. Bảng đã attach (cấp 1/0) hoặc không phải
    /// root ⇒ [`KError::MissingPageTable`] — `map`/`unmap`/`walk` **chỉ** nhận bảng gốc (KT-37
    /// DoD: "phải có PageTable cấp trên đã map, thiếu ⇒ `MissingPageTable`").
    pub(super) fn page_table_root(&self, pt: ObjId) -> Result<&PageTableData, KError> {
        let d = self.page_table_data(pt)?;
        if d.attached.is_some() || d.level != PT_ROOT_LEVEL {
            return Err(KError::MissingPageTable);
        }
        Ok(d)
    }

    /// **KT-37** — số object `PageTable` **còn sống**.
    pub fn page_table_count(&self) -> usize {
        self.objects
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::PageTable)
            .count()
    }

    /// **KT-37** — ảnh chụp một PageTable (cấp, bảng cha, số entry, số lá trong cây con).
    pub fn page_table_info(&self, pt: ObjId) -> Result<PageTableInfo, KError> {
        let d = self.page_table_data(pt)?;
        Ok(PageTableInfo {
            level: d.level,
            attached: d.attached,
            entries: d.entry_count(),
            mapped: d.mapped,
        })
    }

    /// **KT-37** — PTE thô tại `index` (test layout PTE; `index >= 512` ⇒ `RangeError`).
    pub fn page_table_pte_raw(&self, pt: ObjId, index: usize) -> Result<u64, KError> {
        if index >= PT_ENTRY_COUNT {
            return Err(KError::RangeError);
        }
        let d = self.page_table_data(pt)?;
        d.entries
            .get(index)
            .map(|p| p.raw())
            .ok_or(KError::RangeError)
    }

    /// Dựng [`Mapping`] từ một PTE lá ở `level`; kiểm Frame còn sống + cỡ khớp cấp lá + `pa`
    /// căn đúng cỡ (bất biến; lệch ⇒ `IllegalOperation`, `check_pages` bắt sớm hơn).
    pub(super) fn mapping_for_leaf(
        &self,
        va: u64,
        level: u8,
        frame: ObjId,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        let size_bits = size_bits_for_level(level).ok_or(KError::IllegalOperation)?;
        let meta = self
            .frame_meta(frame)
            .map_err(|_| KError::IllegalOperation)?;
        if !meta.live || meta.size_bits != size_bits {
            return Err(KError::IllegalOperation);
        }
        let size = 1u64 << size_bits;
        if !meta.pa.is_multiple_of(size) {
            return Err(KError::IllegalOperation);
        }
        Ok(Mapping {
            va: va & !(size - 1),
            pa: meta.pa,
            size_bits,
            frame,
            rights,
            attrs,
        })
    }

    /// **KT-37** — walk ngược VA→PA trong cây gốc `root` (model API; cũng là oracle cho KT-42).
    ///
    /// Lá ở **bất kỳ** cấp nào phủ `va` đều trả về; `va` lệch căn trong trang lớn vẫn trả mapping
    /// của trang lớn (đúng ngữ nghĩa MMU). Chưa map ⇒ [`KError::NotMapped`]; `va` ngoài VA 39 bit
    /// ⇒ [`KError::RangeError`]; `root` không phải bảng gốc ⇒ [`KError::MissingPageTable`].
    pub fn page_table_walk(&self, root: ObjId, va: u64) -> Result<Mapping, KError> {
        self.page_table_root(root)?;
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        let mut cur = root;
        let mut level = PT_ROOT_LEVEL;
        loop {
            let idx = index_in_level(va, level).ok_or(KError::IllegalOperation)?;
            let d = self.page_table_data(cur)?;
            let pte = d.entries.get(idx).ok_or(KError::IllegalOperation)?;
            match decode_pte(pte) {
                DecodedPte::Empty => return Err(KError::NotMapped),
                DecodedPte::Table(child) => {
                    if level == 0 {
                        return Err(KError::IllegalOperation);
                    }
                    let cd = self.page_table_data(child)?;
                    if cd.attached != Some((cur, idx as u16)) || cd.level + 1 != level {
                        return Err(KError::IllegalOperation);
                    }
                    cur = child;
                    level -= 1;
                }
                DecodedPte::Leaf {
                    frame,
                    rights,
                    attrs,
                } => return self.mapping_for_leaf(va, level, frame, rights, attrs),
            }
        }
    }

    /// **KT-37** — `PageTable.Map` qua **cap Frame** (method 0 của `PageTable` trong spec, vẫn
    /// `reserved` nên đây là semantic model API, không đổi `spec/`). Quyền/attrs lấy từ **tham
    /// số**, không suy từ quyền cap (DoD KT-37).
    pub fn page_table_map(
        &mut self,
        root: ObjId,
        va: u64,
        frame_cap: SlotRef,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        let frame = self.require_type(frame_cap, ObjType::Frame)?;
        self.page_table_map_obj(root, va, frame, rights, attrs)
    }

    /// **KT-37** — `map` theo `ObjId` Frame (model API cho property/oracle; cap chỉ là đường ABI).
    ///
    /// Thứ tự kiểm (mọi lỗi **atomic** — không đổi cây/PTE/bộ đếm):
    /// 1. `root` phải là PageTable **gốc** (chưa attach, cấp 2), nếu không ⇒ `MissingPageTable`;
    /// 2. Frame phải sống + cỡ hợp lệ; `size_bits` lạ ⇒ `InvalidArgument`;
    /// 3. `va`/`va + cỡ` ngoài VA 39 bit ⇒ `RangeError`;
    /// 4. `va` hoặc `pa` lệch căn cỡ Frame ⇒ `Alignment` (2M/1G phủ **đúng một** entry lá);
    /// 5. thiếu bảng cấp trên (mid/leaf) ⇒ `MissingPageTable`;
    /// 6. entry lá đã có (kể cả lá lớn phủ VA) ⇒ `AlreadyMapped`;
    /// 7. vượt `MAX_FRAMES_PER_VSPACE` ⇒ `NotEnoughMemory`;
    /// 8. **KT-39** — `attrs` không hợp lệ ⇒ `InvalidArgument`/`WxViolation` (luật ở
    ///    `pagetable::validate_attrs`);
    /// 9. **KT-39** — W^X toàn cục theo Frame (`mapped_x/mapped_w`) ⇒ `WxViolation` **trước khi**
    ///    cộng bộ đếm/ghi PTE;
    /// 10. cộng `maps/mapped_w/mapped_x` của Frame rồi mới ghi PTE (rollback nếu lỗi).
    pub fn page_table_map_obj(
        &mut self,
        root: ObjId,
        va: u64,
        frame: ObjId,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        self.page_table_root(root)?;
        let fmeta = self.frame_meta(frame)?;
        if !fmeta.live {
            return Err(KError::InvalidCapability);
        }
        let size_bits = fmeta.size_bits;
        let leaf_level = level_for_size_bits(size_bits).ok_or(KError::InvalidArgument)?;
        let size = 1u64 << size_bits;
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        let end = va.checked_add(size).ok_or(KError::RangeError)?;
        if end > VA_LIMIT {
            return Err(KError::RangeError);
        }
        if !va.is_multiple_of(size) || !fmeta.pa.is_multiple_of(size) {
            return Err(KError::Alignment);
        }
        // Đi từ root xuống **bảng giữ lá**, chỉ đọc; thiếu bảng con ⇒ MissingPageTable.
        let mut path: Vec<ObjId> = Vec::with_capacity(3);
        let mut cur = root;
        let mut level = PT_ROOT_LEVEL;
        path.push(root);
        while level > leaf_level {
            let idx = index_in_level(va, level).ok_or(KError::IllegalOperation)?;
            let pte = self
                .page_table_data(cur)?
                .entries
                .get(idx)
                .ok_or(KError::IllegalOperation)?;
            match decode_pte(pte) {
                DecodedPte::Empty => return Err(KError::MissingPageTable),
                DecodedPte::Leaf { .. } => return Err(KError::AlreadyMapped),
                DecodedPte::Table(child) => {
                    let cd = self.page_table_data(child)?;
                    if cd.level + 1 != level {
                        return Err(KError::IllegalOperation);
                    }
                    cur = child;
                    level -= 1;
                    path.push(cur);
                }
            }
        }
        let idx = index_in_level(va, level).ok_or(KError::IllegalOperation)?;
        if !self
            .page_table_data(cur)?
            .entries
            .get(idx)
            .ok_or(KError::IllegalOperation)?
            .is_empty()
        {
            return Err(KError::AlreadyMapped);
        }
        // Biên số lá của **cả cây** (đúng thứ tự trong `k2-vm.md` §11.4: sau
        // `AlreadyMapped`, trước khi đổi bộ đếm).
        if self.page_table_data(root)?.mapped as usize >= vifast_pte::MAX_FRAMES_PER_VSPACE {
            return Err(KError::NotEnoughMemory);
        }

        // **KT-39** — luật `attrs` (một kiểu bộ nhớ, NX⊥EXEC, Device⊥EXEC) **trước** mọi thay
        // đổi cây/bộ đếm; `Device+EXEC` trả `WxViolation` (D2-67).
        validate_attrs(attrs)?;
        let wants_w = rights.contains(MapRights::WRITE);
        let wants_x = attrs.contains(MapAttrs::EXEC);
        // Cộng bộ đếm Frame **trước** khi ghi PTE; `frame_note_map` áp luật W^X toàn cục
        // (map W khi `mapped_x > 0`, map X khi `mapped_w > 0`, W+X cùng lời gọi) — lỗi ⇒ chưa
        // có gì đổi.
        self.frame_note_map(frame, wants_w, wants_x)?;
        let pte = leaf_pte(frame, rights, attrs);
        self.page_table_data_mut(cur)?
            .entries
            .set(idx, pte)
            .ok_or(KError::IllegalOperation)?;
        for t in path {
            let d = self.page_table_data_mut(t)?;
            d.mapped += 1;
        }
        let mapping = Mapping {
            va,
            pa: fmeta.pa,
            size_bits,
            frame,
            rights,
            attrs,
        };
        self.sched_after_op(false);
        Ok(mapping)
    }

    /// **KT-37** — `PageTable.Unmap`: gỡ lá phủ `va` (method 1 của `PageTable`).
    ///
    /// Chỉ nhận `va` **đúng gốc** của mapping (lệch ⇒ [`KError::Alignment`] — không xoá ngầm
    /// trang lớn khi người gọi chỉ định một địa chỉ bên trong). Chưa map ⇒
    /// [`KError::NotMapped`]; trừ bộ đếm Frame + `mapped` của mọi bảng trên đường đi.
    pub fn page_table_unmap(&mut self, root: ObjId, va: u64) -> Result<Mapping, KError> {
        self.page_table_root(root)?;
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        let mut path: Vec<ObjId> = Vec::with_capacity(3);
        let mut cur = root;
        let mut level = PT_ROOT_LEVEL;
        path.push(root);
        let (leaf_idx, mapping) = loop {
            let idx = index_in_level(va, level).ok_or(KError::IllegalOperation)?;
            let pte = self
                .page_table_data(cur)?
                .entries
                .get(idx)
                .ok_or(KError::IllegalOperation)?;
            match decode_pte(pte) {
                DecodedPte::Empty => return Err(KError::NotMapped),
                DecodedPte::Table(child) => {
                    if level == 0 {
                        return Err(KError::IllegalOperation);
                    }
                    let cd = self.page_table_data(child)?;
                    if cd.attached != Some((cur, idx as u16)) || cd.level + 1 != level {
                        return Err(KError::IllegalOperation);
                    }
                    cur = child;
                    level -= 1;
                    path.push(cur);
                }
                DecodedPte::Leaf {
                    frame,
                    rights,
                    attrs,
                } => {
                    let m = self.mapping_for_leaf(va, level, frame, rights, attrs)?;
                    break (idx, m);
                }
            }
        };
        if mapping.va != va {
            return Err(KError::Alignment);
        }
        // Bộ đếm `mapped` của mọi bảng trên đường đi phải dương — kiểm **trước** khi đổi cây
        // để lỗi bất biến không để lại trạng thái nửa vời (thay cho `saturating_sub` cũ).
        for t in &path {
            if self.page_table_data(*t)?.mapped == 0 {
                return Err(KError::IllegalOperation);
            }
        }
        let wants_w = mapping.rights.contains(MapRights::WRITE);
        let wants_x = mapping.attrs.contains(MapAttrs::EXEC);
        // Trừ bộ đếm Frame **trước** khi xoá PTE; lỗi ⇒ PTE/cây còn nguyên.
        self.frame_note_unmap(mapping.frame, wants_w, wants_x)?;
        self.page_table_data_mut(cur)?
            .entries
            .set(leaf_idx, Pte::EMPTY)
            .ok_or(KError::IllegalOperation)?;
        for t in path {
            let d = self.page_table_data_mut(t)?;
            debug_assert!(d.mapped > 0);
            d.mapped -= 1;
        }
        self.sched_after_op(false);
        Ok(mapping)
    }

    /// **KT-39** — `PageTable.Protect` (model API; `Frame.Protect`/hạ quyền của design §4.10):
    /// đổi `rights`/`attrs` của **một** lá đang sống, **tính lại** `mapped_w`/`mapped_x` của Frame
    /// theo delta và áp luật W^X **trước** khi ghi.
    ///
    /// Ngữ nghĩa:
    /// - `va` phải trỏ đúng **gốc** mapping (lệch trong trang lớn ⇒ [`KError::Alignment`]; chưa
    ///   map ⇒ [`KError::NotMapped`]) — không sửa ngầm cả trang lớn;
    /// - `attrs` theo luật [`validate_attrs`] (một kiểu bộ nhớ, NX⊥EXEC, Device⊥EXEC);
    /// - nếu bộ đếm **kết quả** có `w > 0 && x > 0` ⇒ [`KError::WxViolation`] **atomic** (không
    ///   đổi PTE/bộ đếm). Chuyển W→X hoặc X→W là **yêu cầu tường minh** của người gọi nên được
    ///   phép khi kết quả thoả bất biến (khác `map`, không bao giờ tự hạ quyền mapping cũ);
    /// - thành công: `maps` giữ nguyên, `mapped_w/mapped_x` đổi đúng hiệu, PTE đổi rights/attrs,
    ///   trả [`Mapping`] mới.
    pub fn page_table_protect(
        &mut self,
        root: ObjId,
        va: u64,
        rights: MapRights,
        attrs: MapAttrs,
    ) -> Result<Mapping, KError> {
        self.page_table_root(root)?;
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        // Walk tới **lá phủ `va`** (chỉ đọc) — cùng đường với `unmap`, không tự xoá mapping.
        let mut cur = root;
        let mut level = PT_ROOT_LEVEL;
        let (leaf_idx, mapping) = loop {
            let idx = index_in_level(va, level).ok_or(KError::IllegalOperation)?;
            let pte = self
                .page_table_data(cur)?
                .entries
                .get(idx)
                .ok_or(KError::IllegalOperation)?;
            match decode_pte(pte) {
                DecodedPte::Empty => return Err(KError::NotMapped),
                DecodedPte::Table(child) => {
                    if level == 0 {
                        return Err(KError::IllegalOperation);
                    }
                    let cd = self.page_table_data(child)?;
                    if cd.attached != Some((cur, idx as u16)) || cd.level + 1 != level {
                        return Err(KError::IllegalOperation);
                    }
                    cur = child;
                    level -= 1;
                }
                DecodedPte::Leaf {
                    frame,
                    rights,
                    attrs,
                } => {
                    let m = self.mapping_for_leaf(va, level, frame, rights, attrs)?;
                    break (idx, m);
                }
            }
        };
        if mapping.va != va {
            return Err(KError::Alignment);
        }
        validate_attrs(attrs)?;
        let frame = mapping.frame;
        let meta = self.frame_meta(frame)?;
        if !meta.live {
            return Err(KError::InvalidCapability);
        }
        let old_w = u32::from(mapping.rights.contains(MapRights::WRITE));
        let old_x = u32::from(mapping.attrs.contains(MapAttrs::EXEC));
        let new_w = u32::from(rights.contains(MapRights::WRITE));
        let new_x = u32::from(attrs.contains(MapAttrs::EXEC));
        if meta.mapped_w < old_w || meta.mapped_x < old_x {
            return Err(KError::IllegalOperation);
        }
        // **W^X trước mọi thay đổi**: kết quả `w>0 ∧ x>0` ⇒ từ chối, PTE/bộ đếm còn nguyên.
        if meta.mapped_w - old_w + new_w > 0 && meta.mapped_x - old_x + new_x > 0 {
            return Err(KError::WxViolation);
        }
        // Áp **một cửa** duy nhất vào bộ đếm Frame (kiểm lại để không trừ âm), rồi ghi PTE.
        {
            let f = self.frame_meta_live_mut(frame)?;
            if f.mapped_w < old_w || f.mapped_x < old_x {
                return Err(KError::IllegalOperation);
            }
            f.mapped_w = f.mapped_w - old_w + new_w;
            f.mapped_x = f.mapped_x - old_x + new_x;
        }
        self.page_table_data_mut(cur)?
            .entries
            .set(leaf_idx, leaf_pte(frame, rights, attrs))
            .ok_or(KError::IllegalOperation)?;
        self.sched_after_op(false);
        Ok(Mapping {
            va: mapping.va,
            pa: mapping.pa,
            size_bits: mapping.size_bits,
            frame,
            rights,
            attrs,
        })
    }

    /// **KT-37** — gắn một PageTable con (`child`, chưa attach, rỗng) vào `parent` tại `va`.
    ///
    /// Cấp của con do vị trí quyết định: cha cấp 2 ⇒ con cấp 1 (phủ 1 GiB), cha cấp 1 ⇒ con cấp 0
    /// (phủ 2 MiB). `va` phải căn đúng **vùng con phủ** (`Alignment`), trong VA 39 bit
    /// (`RangeError`), ô đích phải rỗng (`AlreadyMapped`), cha cấp 0 không có cấp dưới
    /// (`InvalidArgument`), con đã attach/không rỗng (`AlreadyMapped`/`InvalidArgument`).
    pub fn page_table_attach(
        &mut self,
        parent: ObjId,
        va: u64,
        child: ObjId,
    ) -> Result<(), KError> {
        let p_slot = self.page_table_slot_of(parent)?;
        let c_slot = self.page_table_slot_of(child)?;
        if parent == child {
            return Err(KError::InvalidArgument);
        }
        // **KT-38** — root PageTable của một VSpace là object **nội bộ** (không cap) và phải luôn
        // là **bảng gốc**; gắn nó làm bảng con sẽ phá bất biến `check_vspace` ⇒ từ chối rõ.
        if self.vspaces.iter().flatten().any(|d| d.root == child) {
            return Err(KError::IllegalOperation);
        }
        let plevel = self.page_tables[p_slot]
            .as_ref()
            .ok_or(KError::IllegalOperation)?
            .level;
        if plevel == 0 {
            return Err(KError::InvalidArgument);
        }
        let clevel = plevel - 1;
        let span = table_span(clevel).ok_or(KError::InvalidArgument)?;
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        let end = va.checked_add(span).ok_or(KError::RangeError)?;
        if end > VA_LIMIT {
            return Err(KError::RangeError);
        }
        if !va.is_multiple_of(span) {
            return Err(KError::Alignment);
        }
        {
            let cd = self.page_tables[c_slot]
                .as_ref()
                .ok_or(KError::IllegalOperation)?;
            if cd.attached.is_some() {
                return Err(KError::AlreadyMapped);
            }
            if cd.mapped != 0 || cd.entries.iter().any(|p| !p.is_empty()) {
                return Err(KError::InvalidArgument);
            }
        }
        let idx = index_in_level(va, plevel).ok_or(KError::InvalidArgument)?;
        {
            let pd = self.page_tables[p_slot]
                .as_ref()
                .ok_or(KError::IllegalOperation)?;
            if !pd
                .entries
                .get(idx)
                .ok_or(KError::IllegalOperation)?
                .is_empty()
            {
                return Err(KError::AlreadyMapped);
            }
        }
        {
            let cd = self.page_tables[c_slot]
                .as_mut()
                .ok_or(KError::IllegalOperation)?;
            cd.level = clevel;
            cd.attached = Some((parent, idx as u16));
        }
        let pd = self.page_tables[p_slot]
            .as_mut()
            .ok_or(KError::IllegalOperation)?;
        pd.entries
            .set(idx, table_pte(child))
            .ok_or(KError::IllegalOperation)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-37** — gỡ PageTable con khỏi `parent` tại `va` (đối ứng [`Machine::page_table_attach`]).
    ///
    /// Con còn **bất kỳ** entry/lá nào ⇒ [`KError::HasChildren`] và **không đổi gì** (không im
    /// lặng bỏ cây/mapping — KT-37 DoD). Con rỗng ⇒ gỡ, trả về làm bảng gốc cấp 2 (dùng lại được).
    /// Ô không phải bảng (rỗng/lá) ⇒ `NotMapped`.
    pub fn page_table_detach(&mut self, parent: ObjId, va: u64) -> Result<ObjId, KError> {
        let p_slot = self.page_table_slot_of(parent)?;
        let plevel = self.page_tables[p_slot]
            .as_ref()
            .ok_or(KError::IllegalOperation)?
            .level;
        if plevel == 0 {
            return Err(KError::NotMapped);
        }
        // Cùng hợp đồng căn lề với `attach`: `va` phải đúng **gốc vùng** của bảng con
        // (span của cấp `plevel - 1`), nếu không ⇒ `Alignment` (review N1).
        let clevel = plevel - 1;
        let span = table_span(clevel).ok_or(KError::InvalidArgument)?;
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        let end = va.checked_add(span).ok_or(KError::RangeError)?;
        if end > VA_LIMIT {
            return Err(KError::RangeError);
        }
        if !va.is_multiple_of(span) {
            return Err(KError::Alignment);
        }
        let idx = index_in_level(va, plevel).ok_or(KError::IllegalOperation)?;
        let child = match decode_pte(
            self.page_tables[p_slot]
                .as_ref()
                .ok_or(KError::IllegalOperation)?
                .entries
                .get(idx)
                .ok_or(KError::IllegalOperation)?,
        ) {
            DecodedPte::Table(child) => child,
            _ => return Err(KError::NotMapped),
        };
        let c_slot = self.page_table_slot_of(child)?;
        {
            let cd = self.page_tables[c_slot]
                .as_ref()
                .ok_or(KError::IllegalOperation)?;
            if cd.mapped != 0 || cd.entries.iter().any(|p| !p.is_empty()) {
                return Err(KError::HasChildren);
            }
        }
        self.page_tables[p_slot]
            .as_mut()
            .ok_or(KError::IllegalOperation)?
            .entries
            .set(idx, Pte::EMPTY)
            .ok_or(KError::IllegalOperation)?;
        let cd = self.page_tables[c_slot]
            .as_mut()
            .ok_or(KError::IllegalOperation)?;
        cd.level = PT_ROOT_LEVEL;
        cd.attached = None;
        cd.mapped = 0;
        self.sched_after_op(false);
        Ok(child)
    }

    /// **KT-37** — bất biến **cây PageTable + bộ đếm toàn cục** ("check_pages sớm"; KT-42 sẽ gộp
    /// vào `check_vm`). Chạy sau **mỗi** public op qua [`Machine::sched_after_op`], với cửa sổ
    /// nhanh `page_table_objects == 0` như `check_frames` của KT-36.
    ///
    /// Kiểm:
    /// 1. **kiểu/dữ liệu**: `ty == PageTable` ⇔ có `Meta.pt` + dữ liệu; object chết ⇒ dữ liệu
    ///    `None`; object sống ⇒ dữ liệu `Some`, `len == 4096`;
    /// 2. **cấp/attach**: root ⇒ chưa attach + cấp 2; con ⇒ cấp = cha − 1, PTE của cha trỏ
    ///    **đúng** con tại **đúng** chỉ số;
    /// 3. **cây**: mọi bảng con được trỏ **đúng một lần**, không bảng mồ côi; mỗi lá cấp đúng cỡ
    ///    Frame, Frame sống, `pa` căn cỡ;
    /// 4. **bộ đếm**: `PageTableData.mapped` = số lá của cây con (đệ quy); với **mọi** Frame:
    ///    `maps/mapped_w/mapped_x` = số lá trỏ tới Frame + quyền/EXEC tương ứng (bộ đếm toàn cục,
    ///    mọi root);
    /// 5. **biên**: `mapped ≤ MAX_FRAMES_PER_VSPACE` mỗi cây.
    pub fn check_pages(&self) -> Result<(), String> {
        if self.page_table_objects == 0 {
            return Ok(());
        }
        // (1) kiểu + dữ liệu + cấp/attach.
        for (i, m) in self.objects.iter().enumerate() {
            let obj = (i as u32) + 1;
            match (m.ty == ObjType::PageTable, m.pt) {
                (true, None) => {
                    return Err(format!("object {obj} là PageTable nhưng thiếu dữ liệu"))
                }
                (false, Some(_)) => {
                    return Err(format!(
                        "object {obj} ({}) không phải PageTable nhưng có dữ liệu PageTable",
                        m.ty.name()
                    ))
                }
                _ => {}
            }
            let Some(pt_idx) = m.pt else { continue };
            let data = self.page_tables.get(pt_idx).and_then(|d| d.as_ref());
            if !m.alive {
                if data.is_some() {
                    return Err(format!("PageTable {obj} đã chết nhưng còn dữ liệu"));
                }
                continue;
            }
            let Some(d) = data else {
                return Err(format!("PageTable {obj} sống nhưng thiếu dữ liệu"));
            };
            if m.len != PT_BYTES {
                return Err(format!("PageTable {obj}: len {} ≠ {PT_BYTES}", m.len));
            }
            if d.level > PT_ROOT_LEVEL {
                return Err(format!("PageTable {obj}: cấp {} lạ", d.level));
            }
            match d.attached {
                Some((parent, idx)) => {
                    if d.level == PT_ROOT_LEVEL {
                        return Err(format!("PageTable {obj}: root nhưng lại attach"));
                    }
                    let Some(pm) = self.objects.get(parent as usize - 1) else {
                        return Err(format!("PageTable {obj}: cha {parent} không tồn tại"));
                    };
                    if !pm.alive || pm.ty != ObjType::PageTable {
                        return Err(format!(
                            "PageTable {obj}: cha {parent} không phải PageTable sống"
                        ));
                    }
                    let Some(pd) = pm
                        .pt
                        .and_then(|p| self.page_tables.get(p))
                        .and_then(|d| d.as_ref())
                    else {
                        return Err(format!("PageTable {obj}: cha {parent} thiếu dữ liệu"));
                    };
                    if pd.level != d.level + 1 {
                        return Err(format!(
                            "PageTable {obj}: cấp {} nhưng cha {parent} cấp {}",
                            d.level, pd.level
                        ));
                    }
                    let pte = pd
                        .entries
                        .get(idx as usize)
                        .ok_or_else(|| format!("PageTable {obj}: chỉ số cha {idx} ngoài biên"))?;
                    if decode_pte(pte) != DecodedPte::Table(obj) {
                        return Err(format!(
                            "PageTable {obj}: PTE cha {parent}[{idx}] không trỏ đúng con"
                        ));
                    }
                }
                None => {
                    if d.level != PT_ROOT_LEVEL {
                        return Err(format!(
                            "PageTable {obj}: cấp {} chưa attach (không phải root cấp 2)",
                            d.level
                        ));
                    }
                }
            }
        }
        // (2) DFS từ mọi root: bảng con trỏ 2 lần / lá sai cấp / Frame chết; đếm lá theo Frame.
        let mut reached = vec![false; self.objects.len()];
        let mut counts = vec![(0u32, 0u32, 0u32); self.objects.len()];
        for (i, m) in self.objects.iter().enumerate() {
            if !m.alive || m.ty != ObjType::PageTable {
                continue;
            }
            let Some(pt_idx) = m.pt else { continue };
            let Some(d) = self.page_tables.get(pt_idx).and_then(|d| d.as_ref()) else {
                continue;
            };
            if d.attached.is_some() {
                continue;
            }
            let obj = (i as u32) + 1;
            let total = self.check_page_subtree(obj, &mut reached, &mut counts)?;
            if total != d.mapped {
                return Err(format!(
                    "PageTable {obj}: mapped {} ≠ đếm cây {total}",
                    d.mapped
                ));
            }
        }
        for (i, m) in self.objects.iter().enumerate() {
            if m.alive && m.ty == ObjType::PageTable && !reached[i] {
                let obj = (i as u32) + 1;
                return Err(format!("PageTable {obj}: mồ côi (không root nào trỏ tới)"));
            }
        }
        // (3) bộ đếm Frame toàn cục khớp số lá thật.
        for (i, m) in self.objects.iter().enumerate() {
            if !m.alive || m.ty != ObjType::Frame {
                continue;
            }
            let obj = (i as u32) + 1;
            let f = m
                .frame
                .ok_or_else(|| format!("Frame {obj}: thiếu metadata"))?;
            let (maps, w, x) = counts[i];
            if f.maps != maps || f.mapped_w != w || f.mapped_x != x {
                return Err(format!(
                    "Frame {obj}: bộ đếm ({}, {}, {}) ≠ số lá ({maps}, {w}, {x})",
                    f.maps, f.mapped_w, f.mapped_x
                ));
            }
        }
        Ok(())
    }

    /// **KT-39** — bất biến **W^X toàn cục** trên **mọi** Frame có metadata (kể cả Frame chỉ
    /// được map bởi PageTable thuần, không thuộc VSpace nào — D2-67 "không có primitive miễn
    /// luật"). Chạy sau **mỗi** public op qua [`Machine::sched_after_op`]; cửa sổ nhanh
    /// `frame_objects == 0` giữ chi phí per-op của mọi test K1a..K1c như trước.
    ///
    /// Kiểm (rẻ, không đếm lại cây — việc đếm lại là [`Machine::wx_recount`] ở cổng):
    /// 1. **không** Frame nào vừa `mapped_w > 0` vừa `mapped_x > 0` (bất biến `!(w>0 && x>0)`);
    /// 2. `mapped_w + mapped_x ≤ maps` (suy ra `w ≤ maps`, `x ≤ maps`; mọi bộ đếm là `u32` nên
    ///    không âm).
    ///
    /// Frame chết + bộ đếm khác 0 do [`Machine::check_frames`] bắt (**chạy ngay trước** mỗi op);
    /// `check_wx` cố ý **không** lặp kiểm đó (Frame chết `maps=1,w=1,x=0` vẫn thoả hai bất biến ở
    /// đây) — khi gọi **một mình** phải coi `check_frames` là lưới đi kèm.
    pub fn check_wx(&self) -> Result<(), String> {
        if self.frame_objects == 0 {
            return Ok(());
        }
        for (i, m) in self.objects.iter().enumerate() {
            if m.ty != ObjType::Frame {
                continue;
            }
            let Some(f) = m.frame else {
                // `check_frames` báo "thiếu metadata"; ở đây bỏ qua để hai checker không trùng
                // thông điệp nhưng vẫn không panic.
                continue;
            };
            let obj = (i as u32) + 1;
            if f.mapped_w > 0 && f.mapped_x > 0 {
                return Err(format!(
                    "Frame {obj}: vừa WRITE ({}) vừa EXEC ({}) — vi phạm W^X",
                    f.mapped_w, f.mapped_x
                ));
            }
            match f.mapped_w.checked_add(f.mapped_x) {
                Some(tong) if tong <= f.maps => {}
                _ => {
                    return Err(format!(
                        "Frame {obj}: mapped_w {} + mapped_x {} > maps {}",
                        f.mapped_w, f.mapped_x, f.maps
                    ))
                }
            }
        }
        Ok(())
    }

    /// **KT-39** — **đếm lại từ cây PageTable** (mọi root, kể cả PageTable thuần) và so với bộ
    /// đếm trong [`FrameMeta`]; trả tổng [`WxRecount`] cho test cổng. Đây là bản đếm **độc lập**
    /// với `check_pages` (cùng bất biến nhưng đường đi riêng) — "recount từ cây ở cổng" của
    /// K2-PLAN §3 KT-39.
    ///
    /// Lỗi (không im lặng): bảng bị trỏ nhiều lần/mồ côi; lá trỏ Frame chết; bộ đếm Frame khác
    /// số lá; Frame vừa W vừa X (lưới thứ hai sau [`Machine::check_wx`]).
    pub fn wx_recount(&self) -> Result<WxRecount, String> {
        let n = self.objects.len();
        let mut reached = vec![false; n];
        let mut counts = vec![(0u32, 0u32, 0u32); n];
        for (i, m) in self.objects.iter().enumerate() {
            if !m.alive || m.ty != ObjType::PageTable {
                continue;
            }
            let Some(pit) = m.pt else {
                return Err(format!("PageTable {}: thiếu dữ liệu", i + 1));
            };
            let Some(d) = self.page_tables.get(pit).and_then(|d| d.as_ref()) else {
                return Err(format!("PageTable {}: thiếu dữ liệu", i + 1));
            };
            if d.attached.is_some() {
                continue;
            }
            self.recount_wx_subtree((i as u32) + 1, &mut reached, &mut counts)?;
        }
        for (i, m) in self.objects.iter().enumerate() {
            if m.alive && m.ty == ObjType::PageTable && !reached[i] {
                return Err(format!(
                    "PageTable {}: mồ côi (không root nào trỏ tới)",
                    i + 1
                ));
            }
        }
        let mut out = WxRecount::default();
        for (i, m) in self.objects.iter().enumerate() {
            if m.ty != ObjType::Frame {
                continue;
            }
            let Some(f) = m.frame else { continue };
            let obj = (i as u32) + 1;
            let (maps, w, x) = counts[i];
            if f.maps != maps || f.mapped_w != w || f.mapped_x != x {
                return Err(format!(
                    "Frame {obj}: bộ đếm ({}, {}, {}) ≠ đếm lại từ cây ({maps}, {w}, {x})",
                    f.maps, f.mapped_w, f.mapped_x
                ));
            }
            if f.mapped_w > 0 && f.mapped_x > 0 {
                return Err(format!(
                    "Frame {obj}: vừa WRITE ({}) vừa EXEC ({}) — vi phạm W^X",
                    f.mapped_w, f.mapped_x
                ));
            }
            if maps > 0 {
                out.frames += 1;
                out.maps += u64::from(maps);
                out.mapped_w += u64::from(w);
                out.mapped_x += u64::from(x);
            }
        }
        Ok(out)
    }

    /// Đệ quy **một** cây PageTable cho [`Machine::wx_recount`]: cộng `(maps, w, x)` theo Frame
    /// và đánh dấu bảng đã đi (bảng bị trỏ hai lần ⇒ lỗi, không đếm đôi).
    pub(super) fn recount_wx_subtree(
        &self,
        table: ObjId,
        reached: &mut [bool],
        counts: &mut [(u32, u32, u32)],
    ) -> Result<(), String> {
        let ti = table as usize - 1;
        if reached[ti] {
            return Err(format!("PageTable {table}: bị trỏ nhiều lần"));
        }
        reached[ti] = true;
        let d = self
            .page_table_data(table)
            .map_err(|e| format!("PageTable {table}: {e:?}"))?;
        for (i, pte) in d.entries.iter().enumerate() {
            match decode_pte(*pte) {
                DecodedPte::Empty => {}
                DecodedPte::Table(child) => {
                    if d.level == 0 {
                        return Err(format!("PageTable {table}: cấp 0 có bảng con ở [{i}]"));
                    }
                    let cd = self
                        .page_table_data(child)
                        .map_err(|e| format!("PageTable {table}[{i}]: {e:?}"))?;
                    if cd.attached != Some((table, i as u16)) {
                        return Err(format!(
                            "PageTable {table}[{i}]: con {child} không attach đúng cha/chỉ số"
                        ));
                    }
                    self.recount_wx_subtree(child, reached, counts)?;
                }
                DecodedPte::Leaf {
                    frame,
                    rights,
                    attrs,
                } => {
                    let meta = self
                        .frame_meta(frame)
                        .map_err(|e| format!("PageTable {table}[{i}]: {e:?}"))?;
                    if !meta.live {
                        return Err(format!("PageTable {table}[{i}]: Frame {frame} đã chết"));
                    }
                    let c = &mut counts[frame as usize - 1];
                    c.0 =
                        c.0.checked_add(1)
                            .ok_or_else(|| "tràn bộ đếm maps".to_string())?;
                    c.1 =
                        c.1.checked_add(u32::from(rights.contains(MapRights::WRITE)))
                            .ok_or_else(|| "tràn bộ đếm mapped_w".to_string())?;
                    c.2 =
                        c.2.checked_add(u32::from(attrs.contains(MapAttrs::EXEC)))
                            .ok_or_else(|| "tràn bộ đếm mapped_x".to_string())?;
                }
            }
        }
        Ok(())
    }

    /// Đệ quy một cây PageTable cho [`Machine::check_pages`]: trả số lá trong cây con, đánh dấu
    /// bảng đã đi, cộng bộ đếm theo Frame. Không cấp phát ngoài hai `Vec` của caller.
    pub(super) fn check_page_subtree(
        &self,
        table: ObjId,
        reached: &mut [bool],
        counts: &mut [(u32, u32, u32)],
    ) -> Result<u32, String> {
        let ti = table as usize - 1;
        if reached[ti] {
            return Err(format!(
                "PageTable {table}: bị trỏ nhiều lần (cây không phải cây)"
            ));
        }
        reached[ti] = true;
        let d = self
            .page_table_data(table)
            .map_err(|e| format!("PageTable {table}: {e:?}"))?;
        let mut total = 0u32;
        for (i, pte) in d.entries.iter().enumerate() {
            match decode_pte(*pte) {
                DecodedPte::Empty => {}
                DecodedPte::Table(child) => {
                    if d.level == 0 {
                        return Err(format!("PageTable {table}: cấp 0 có bảng con ở [{i}]"));
                    }
                    let cd = self
                        .page_table_data(child)
                        .map_err(|e| format!("PageTable {table}[{i}]: {e:?}"))?;
                    if cd.level + 1 != d.level {
                        return Err(format!(
                            "PageTable {table}[{i}]: con {child} cấp {} ≠ {}",
                            cd.level, d.level
                        ));
                    }
                    if cd.attached != Some((table, i as u16)) {
                        return Err(format!(
                            "PageTable {table}[{i}]: con {child} không attach đúng cha/chỉ số"
                        ));
                    }
                    total = total
                        .checked_add(self.check_page_subtree(child, reached, counts)?)
                        .ok_or_else(|| format!("PageTable {table}: tràn bộ đếm cây"))?;
                }
                DecodedPte::Leaf {
                    frame,
                    rights,
                    attrs,
                } => {
                    let meta = self
                        .frame_meta(frame)
                        .map_err(|e| format!("PageTable {table}[{i}]: {e:?}"))?;
                    let want = size_bits_for_level(d.level).ok_or_else(|| {
                        format!("PageTable {table}: cấp {} không có cỡ lá", d.level)
                    })?;
                    if !meta.live {
                        return Err(format!("PageTable {table}[{i}]: Frame {frame} đã chết"));
                    }
                    if meta.size_bits != want {
                        return Err(format!(
                            "PageTable {table}[{i}]: lá cấp {} phải là Frame {want} bit, gặp {}",
                            d.level, meta.size_bits
                        ));
                    }
                    if !meta.pa.is_multiple_of(1u64 << want) {
                        return Err(format!(
                            "PageTable {table}[{i}]: pa {:#x} lệch căn {want} bit",
                            meta.pa
                        ));
                    }
                    let c = &mut counts[frame as usize - 1];
                    c.0 =
                        c.0.checked_add(1)
                            .ok_or_else(|| "tràn bộ đếm maps".to_string())?;
                    c.1 =
                        c.1.checked_add(u32::from(rights.contains(MapRights::WRITE)))
                            .ok_or_else(|| "tràn bộ đếm mapped_w".to_string())?;
                    c.2 =
                        c.2.checked_add(u32::from(attrs.contains(MapAttrs::EXEC)))
                            .ok_or_else(|| "tràn bộ đếm mapped_x".to_string())?;
                    total = total
                        .checked_add(1)
                        .ok_or_else(|| format!("PageTable {table}: tràn bộ đếm lá"))?;
                }
            }
        }
        if d.mapped != total {
            return Err(format!(
                "PageTable {table}: mapped {} ≠ đếm cây {total}",
                d.mapped
            ));
        }
        if total as usize > vifast_pte::MAX_FRAMES_PER_VSPACE {
            return Err(format!(
                "PageTable {table}: {total} lá > MAX_FRAMES_PER_VSPACE"
            ));
        }
        Ok(total)
    }
}
