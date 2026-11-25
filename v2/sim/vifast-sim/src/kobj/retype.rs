// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `retype`: Retype: Untyped → object, Frame/PageTable/VSpace, cấp ASID.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ---------------------------------------------------------------- Retype

    /// Cấp object từ Untyped theo `type/size_bits/flags` (spec `Untyped.Retype`).
    /// Trả `CapId` cho harness (ABI `returns = []`, D2-35); cap là con của cap nguồn.
    ///
    /// **KT-16**: vùng cấp phát là arena objmem của Untyped; `offset` trả về là watermark
    /// cũ, `untyped_used` = `Arena::used()`, nên không còn phép `align_up` viết tay.
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
            let region = m.region.ok_or(KError::WrongObjectType)?;
            (idx, region, self.regions[region].capacity(), c.rights)
        };
        // F3 (review KT-10): Retype là thao tác cấp phát trên Untyped ⇒ cap nguồn phải có
        // WRITE; cap con kế thừa quyền nguồn (không nới ngược).
        if src_rights & RIGHT_WRITE == 0 {
            return Err(KError::InsufficientRights);
        }

        // GUEST: chỉ VSpace nhận; tới H1 VSpace+GUEST vẫn NotSupported (D2-32).
        if flags & FLAG_GUEST != 0 {
            return Err(if ty == ObjType::VSpace {
                KError::NotSupported
            } else {
                KError::InvalidArgument
            });
        }
        // **KT-36** — Frame có đường riêng: metadata thuần, cấp từ **cửa sổ PA** của Untyped
        // (`FrameArea`), **không** tiêu byte arena (K2-PLAN §1.1, D2-65 Q2/Q3/Q7). Đi trước
        // cổng `is_core` (Frame vẫn là object `reserved` của `spec/` tới K3).
        if ty == ObjType::Frame {
            return self.retype_frame(src_cap, u_idx, size_bits, src_rights);
        }
        // **KT-37** — PageTable cũng là object `reserved` (install_at K2 nhưng `is_core()` false):
        // đường riêng, cấp 4 KiB arena + dữ liệu mô hình ở `Machine::page_tables`.
        if ty == ObjType::PageTable {
            return self.retype_page_table(src_cap, u_idx, size_bits, src_rights);
        }
        // **KT-38** — VSpace **không** đi qua ABI `Retype`: `spec/abi.toml` giữ `VSpace`
        // `reserved`/`unsupported` nên `Retype(VSpace)` **vẫn** `NotSupported` (conformance
        // reserved, và test cũ `kt09_untyped` giữ nguyên). VSpace được tạo bằng **model API**
        // [`Machine::vspace_create`] (K2-PLAN §3 KT-38 "tạo VSpace root (model API)"): cùng
        // arena/revoke với Untyped nguồn, nhưng không mở chữ ký ABI.
        if ty == ObjType::VSpace {
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
                // KT-17 (follow-up KT-16): arena của mô hình chỉ bảo đảm căn `ARENA_ALIGN`
                // (4096 B). Untyped con có `len` = căn của block; `size_bits ≥ 13` (8192 B)
                // không thể cấp `pa` đúng căn tuyệt đối ⇒ **từ chối rõ** bằng `Alignment`
                // (trước KT-17: trả `Ok` với `pa` có thể sai căn, vi phạm hợp đồng `Layout`).
                // Đây là giới hạn của arena sim, không phải thay đổi ABI; `vifast-model` (đã gỡ
                // ở KT-410/`B2`)
                // (oracle) vẫn nhận 12..=30 và parity 10.000 seed chỉ sinh `size_bits = 12`.
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

        // Cấp **ngay trong arena của Untyped nguồn** (objmem): block đúng `len`/`align`
        // của spec ⇒ watermark mô hình = `Arena::used()`.
        let kind = Self::fresh_kind(ty, child_slots);
        let (body, offset) = self.regions[u_region]
            .alloc_place_span(kind, len, align)
            .ok_or(KError::NotEnoughMemory)?;
        // Untyped con có vùng nhớ riêng bằng cỡ vùng vừa cắt (tạo **sau** khi cấp body để
        // lỗi hết chỗ không đốt byte).
        let region = if ty == ObjType::Untyped {
            Some(self.push_region(len))
        } else {
            None
        };
        // **KT-36** — Untyped con có cửa sổ PA riêng cho Frame, đáy = `base_cha + offset_con`
        // (đơn vị offset arena/PA của mô hình là byte, cùng một hệ). Chồng `pa` thật giữa cha
        // và con vẫn bị `AlreadyMapped` chặn: xem [`Machine::retype_frame`].
        let frame_area = if ty == ObjType::Untyped {
            let base_cha = self.objects[u_idx].frame_area.map(|a| a.base).unwrap_or(0);
            Some(FrameArea::new(base_cha.saturating_add(offset as u64)))
        } else {
            None
        };
        let obj = self.push_meta(
            ty,
            Loc::Untyped(u_region as u32),
            body,
            ChoDat {
                untyped: Some(u_idx),
                offset,
                len,
                region,
                slots: if ty == ObjType::CNode {
                    Some(child_slots)
                } else {
                    None
                },
                frame_area,
                ..ChoDat::default()
            },
        );
        if ty == ObjType::Untyped {
            self.frame_windows.push(obj as usize - 1);
        }
        // KT-19 DoD 3: Retê có thể tạo TCB `Inactive` (không vào hàng) — kiểm lại bất biến
        // sau mỗi public op như các op TCB khác.
        self.sched_after_op(false);
        Ok(self.push_cap(obj, ty, Some(src_cap), src_rights, None))
    }

    /// **KT-36** — `Untyped.Retype(Untyped, Frame, size_bits)`: cấp một Frame **metadata
    /// thuần** từ cửa sổ PA của Untyped `u_idx` (xem [`crate::frame`], `k2-vm.md` §10).
    ///
    /// Thứ tự kiểm — **mọi** lỗi đều **không** đổi trạng thái (atomic):
    /// 1. `size_bits ∉ {12, 21, 30}` ⇒ `InvalidArgument` (cỡ đọc từ `vifast_pte::FrameSize`,
    ///    không chép tay giá trị);
    /// 2. `pa = base + watermark` **chưa** căn đúng cỡ ⇒ `Alignment` — mô hình **không** đệm
    ///    căn lề ngầm (DoD KT-36 "pa lệch 1 ⇒ Alignment"); muốn trộn cỡ thì cấp **giảm dần**;
    /// 3. `pa + cỡ` ra ngoài cửa sổ [`UNTYPED_PA_WINDOW`] ⇒ `NotEnoughMemory`;
    /// 4. `[pa, pa + cỡ)` chồng Frame **sống** của Untyped khác ⇒ `AlreadyMapped`.
    ///
    /// Frame **không** tiêu byte arena: `untyped_used`/`check_bytes` không đổi (K2-PLAN §1.1,
    /// "không cấp byte thật"). Body trong arena kernel chỉ là mốc [`Kind::Frame`] vài chục byte
    /// metadata, không phải vùng nhớ của Frame.
    pub(super) fn retype_frame(
        &mut self,
        src_cap: CapId,
        u_idx: usize,
        size_bits: u32,
        src_rights: u8,
    ) -> Result<CapId, KError> {
        let size = u8::try_from(size_bits)
            .map_err(|_| KError::InvalidArgument)
            .and_then(|bits| {
                FrameSize::from_size_bits(bits).map_err(|_| KError::InvalidArgument)
            })?;
        // `area` là **bản sao**: watermark chỉ ghi lại khi mọi bước sau thành công (atomic).
        let mut area = self.objects[u_idx]
            .frame_area
            .ok_or(KError::WrongObjectType)?;
        let pa = area.carve(size)?;
        if self.frame_pa_overlap(u_idx, pa, size.size_bytes()) {
            return Err(KError::AlreadyMapped);
        }
        let body = self
            .kernel
            .alloc_place(Kind::Frame)
            .ok_or(KError::NotEnoughMemory)?;
        let obj = self.push_meta(
            ObjType::Frame,
            Loc::Kernel,
            body,
            ChoDat {
                untyped: Some(u_idx),
                offset: (pa - area.base) as usize,
                len: size.size_bytes() as usize,
                frame: Some(FrameMeta::new(pa, size)),
                ..ChoDat::default()
            },
        );
        self.objects[u_idx].frame_area = Some(area);
        self.sched_after_op(false);
        Ok(self.push_cap(obj, ObjType::Frame, Some(src_cap), src_rights, None))
    }

    /// **KT-37** — `Untyped.Retype(Untyped, PageTable, size_bits)`: cấp một bảng trang **4 KiB**
    /// từ arena của Untyped nguồn (PageTable **có** tiêu byte arena, khác Frame — một bảng thật
    /// là 4096 B PTE) và tạo dữ liệu mô hình rỗng ở `Machine::page_tables`.
    ///
    /// `size_bits` nhận `0` (mặc định = 4096 B) hoặc `12` (= 4096 B); cỡ khác ⇒
    /// [`KError::InvalidArgument`], **atomic** (không đốt byte, không tạo object). Bảng mới có
    /// `level = 2` (root), chưa attach, rỗng; muốn map Frame 4 KiB/2 MiB phải `page_table_attach`
    /// các bảng cấp dưới trước (thiếu ⇒ `MissingPageTable` — KT-37 DoD).
    pub(super) fn retype_page_table(
        &mut self,
        src_cap: CapId,
        u_idx: usize,
        size_bits: u32,
        src_rights: u8,
    ) -> Result<CapId, KError> {
        // PageTable của mô hình đúng một trang 4096 B; `spec/abi.toml` `page_bytes = 4096`.
        if size_bits != 0 && size_bits != 12 {
            return Err(KError::InvalidArgument);
        }
        let region = self.objects[u_idx].region.ok_or(KError::WrongObjectType)?;
        let len = PT_BYTES;
        let align = PT_BYTES;
        let kind = Kind::PageTable;
        let (body, offset) = self.regions[region]
            .alloc_place_span(kind, len, align)
            .ok_or(KError::NotEnoughMemory)?;
        // Dữ liệu trang: cấp **sau** khi chắc chắn arena còn chỗ (lỗi hết chỗ không để lại
        // tombstone rỗng trong `page_tables`).
        let pt_idx = self.page_tables.len();
        self.page_tables.push(Some(PageTableData::new()));
        let obj = self.push_meta(
            ObjType::PageTable,
            Loc::Untyped(region as u32),
            body,
            ChoDat {
                untyped: Some(u_idx),
                offset,
                len,
                pt: Some(pt_idx),
                ..ChoDat::default()
            },
        );
        self.sched_after_op(false);
        Ok(self.push_cap(obj, ObjType::PageTable, Some(src_cap), src_rights, None))
    }

    /// **KT-38** — **model API** tạo một `VSpace`: `VSpace` = **metadata** (asid + thế hệ +
    /// `active_on`, nằm ở `Machine::vspaces`, body chỉ là mốc [`Kind::VSpace`] trong arena kernel)
    /// **+ một root PageTable thật** 4 KiB trong arena của Untyped `src`.
    ///
    /// **Vì sao không qua `Retype`:** `spec/abi.toml` giữ `VSpace` `reserved`/`unsupported`
    /// (D2-65 Q1/Q9; K2-PLAN §1.2) ⇒ `Untyped.Retype(VSpace)` **vẫn** `NotSupported` (conformance
    /// reserved + test cũ `kt09_untyped` giữ nguyên). Đây **không** phải ABI mới: model API của
    /// sim, cùng khuôn [`Machine::retype`] (cap con của cap Untyped nguồn ⇒ `Revoke` Untyped
    /// finalise VSpace + root).
    ///
    /// Root **không mang cap** (object nội bộ của VSpace): người dùng đi qua
    /// [`Machine::vspace_root`]/[`Machine::vspace_walk`] (oracle = [`Machine::page_table_walk`]),
    /// không qua cap. Root bị `page_table_attach` từ chối làm **bảng con** (giữ bất biến
    /// "root = bảng gốc").
    ///
    /// ASID lấy từ free-list của thế hệ hiện tại; cạn ⇒ `generation += 1` + **một** lệnh
    /// [`FlushScope::Generation`] cho mọi hart online ([`Machine::asid_alloc`]). Lỗi arena xảy
    /// ra **trước** khi cấp ASID nên một lần tạo hỏng không làm tăng thế hệ (atomic).
    pub fn vspace_create(&mut self, src: SlotRef) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (u_idx, src_rights) = {
            let c = self.cap(src_cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let idx = self.object_index(c.obj)?;
            if self.objects[idx].ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            (idx, c.rights)
        };
        // Như `Retype`: cấp phát trên Untyped cần quyền WRITE; cap con kế thừa quyền nguồn.
        if src_rights & RIGHT_WRITE == 0 {
            return Err(KError::InsufficientRights);
        }
        self.create_vspace(src_cap, u_idx, src_rights)
    }

    /// Thân của [`Machine::vspace_create`] (một cửa cấp phát duy nhất, như
    /// `retype_frame`/`retype_page_table`) — mọi lỗi **atomic**.
    pub(super) fn create_vspace(
        &mut self,
        src_cap: CapId,
        u_idx: usize,
        src_rights: u8,
    ) -> Result<CapId, KError> {
        let region = self.objects[u_idx].region.ok_or(KError::WrongObjectType)?;
        // 1. Root PageTable: **một trang** arena của Untyped (nó *là* bảng gốc thật của mô hình).
        let (root_body, root_offset) = self.regions[region]
            .alloc_place_span(Kind::PageTable, PT_BYTES, PT_BYTES)
            .ok_or(KError::NotEnoughMemory)?;
        // 2. Body VSpace (mốc metadata) trong arena kernel — VSpace **không** tiêu byte arena.
        //    Hết chỗ ⇒ trả byte arena vừa cấp (không để lại lỗ hổng) rồi báo lỗi.
        let Some(vs_body) = self.kernel.alloc_place(Kind::VSpace) else {
            self.regions[region].free(root_body);
            return Err(KError::NotEnoughMemory);
        };
        // 3. ASID — cửa duy nhất đổi free-list/thế hệ; cạn ⇒ `gen += 1` + một lệnh flush `gen`.
        //    Không bao giờ lỗi (thế hệ mới luôn đủ 256 ASID).
        let (asid, generation) = self.asid_alloc();
        // 4. Ghi dữ liệu mô hình + object. Root **không** có cap (object nội bộ).
        let pt_idx = self.page_tables.len();
        self.page_tables.push(Some(PageTableData::new()));
        let root = self.push_meta(
            ObjType::PageTable,
            Loc::Untyped(region as u32),
            root_body,
            ChoDat {
                untyped: Some(u_idx),
                offset: root_offset,
                len: PT_BYTES,
                pt: Some(pt_idx),
                ..ChoDat::default()
            },
        );
        let space_idx = self.vspaces.len();
        self.vspaces.push(Some(VSpaceData {
            root,
            asid,
            generation,
            active_on: HartMask::EMPTY,
        }));
        let obj = self.push_meta(
            ObjType::VSpace,
            Loc::Kernel,
            vs_body,
            ChoDat {
                untyped: Some(u_idx),
                // VSpace không chiếm byte arena (offset/len = 0); root ở trên mới chiếm 4 KiB.
                space: Some(space_idx),
                ..ChoDat::default()
            },
        );
        self.sched_after_op(false);
        Ok(self.push_cap(obj, ObjType::VSpace, Some(src_cap), src_rights, None))
    }

    /// **KT-38** — cấp một ASID của thế hệ hiện tại; **cạn** ⇒ `generation += 1`, nạp lại
    /// free-list đủ [`ASID_COUNT`] và phát **đúng một** lệnh [`FlushScope::Generation`] cho
    /// **mọi hart online** (Q4: "flush toàn bộ một lần theo `gen`", không flush từng ASID).
    ///
    /// Trả `(asid, generation)`. ASID **không** quay lại free-list khi teardown (chỉ thế hệ mới
    /// tái lập free-list) — xem `k2-vm.md` §12.
    pub(super) fn asid_alloc(&mut self) -> (u16, u64) {
        if self.asid_free.is_empty() {
            self.asid_generation += 1;
            self.asid_exhaustions += 1;
            self.asid_issued = 0;
            self.asid_free = fresh_asid_free();
            let generation = self.asid_generation;
            let harts = self.hart_online;
            self.push_flush(FlushScope::Generation, generation, None, harts);
            // **KT-40** — cạn ASID ⇒ **flush theo `gen`** một lần cho mọi hart: mọi entry của
            // thế hệ cũ mất hiệu lực (Q4; không flush từng ASID).
            for hart in 0..self.tlbs.len() {
                self.tlb_flush_hart_only(hart as u16, generation, 0, TlbFlushScope::Generation);
            }
        }
        let asid = self
            .asid_free
            .pop()
            .expect("free-list vừa được nạp lại ASID_COUNT phần tử");
        self.asid_issued += 1;
        (asid, self.asid_generation)
    }

    /// **KT-38** — ghi một lệnh flush TLB vào [`Machine::flush_log`] (số thứ tự 1-based).
    pub(super) fn push_flush(
        &mut self,
        scope: FlushScope,
        generation: u64,
        asid: Option<u16>,
        harts: HartMask,
    ) {
        let seq = self.flush_log.len() as u64 + 1;
        self.flush_log.push(FlushCmd {
            seq,
            scope,
            generation,
            asid,
            harts,
        });
    }

    /// Có Frame **sống** nào thuộc Untyped khác chồng `[pa, pa + len)` không.
    ///
    /// Chỉ quét Untyped có cửa sổ PA **giao** với khoảng cần cấp — số Untyped nhỏ; 10.000 Frame
    /// trong **một** Untyped thì watermark bảo đảm không chồng theo cấu trúc nên không quét.
    /// `check_frames` kiểm lại bất biến này **toàn cục** (O(n log n), sau mỗi op ở test).
    pub(super) fn frame_pa_overlap(&self, u_idx: usize, pa: u64, len: u64) -> bool {
        let end = pa.saturating_add(len);
        for &i in &self.frame_windows {
            if i == u_idx {
                continue;
            }
            let m = &self.objects[i];
            if !m.alive || m.ty != ObjType::Untyped {
                continue;
            }
            let Some(area) = m.frame_area else { continue };
            let Some((win_base, win_end)) = area.window() else {
                continue;
            };
            if win_end <= pa || end <= win_base {
                continue;
            }
            for o in self.objects.iter() {
                if !o.alive || o.ty != ObjType::Frame || o.untyped != Some(i) {
                    continue;
                }
                let Some(f) = o.frame else { continue };
                let fe = f.pa.saturating_add(o.len as u64);
                if f.pa < end && pa < fe {
                    return true;
                }
            }
        }
        false
    }
}
