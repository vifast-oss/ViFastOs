// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `object`: vòng đời object/arena (`Untyped` watermark, `Kind`, `ObjMem`, finalise).
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    pub fn new() -> Self {
        Self::with_arena_bytes(DEFAULT_ARENA_BYTES)
    }

    /// `Machine` với arena kernel `bytes` byte (mặc định [`DEFAULT_ARENA_BYTES`] = 1 MiB) và
    /// **1 hart** ([`HART_COUNT`]) — đường mặc định của mọi test cũ.
    ///
    /// Arena kernel chứa body của object **không** cấp từ Untyped; object Retype nằm trong
    /// arena riêng của Untyped chứa nó.
    pub fn with_arena_bytes(bytes: usize) -> Self {
        Self::with_harts_and_arena_bytes(HART_COUNT, bytes)
    }

    /// **KT-29** — `Machine` mô hình **N hart tất định** (`1 ≤ n ≤ MAX_HARTS`, D2-53 Q1):
    /// mỗi hart một [`HartSched`] + [`HartMailbox`] + trường công bố [`Published`]; arena kernel
    /// mặc định. `n = 0`/`n > MAX_HARTS` ⇒ `assert!` (cùng kiểu `with_arena_bytes`).
    ///
    /// `Machine::new()` **vẫn 1 hart** — không đổi ngữ nghĩa 427 test cũ; SMP phải opt-in.
    pub fn with_harts(harts: u16) -> Self {
        Self::with_harts_and_arena_bytes(harts, DEFAULT_ARENA_BYTES)
    }

    /// Thân chung của [`Machine::with_arena_bytes`]/[`Machine::with_harts`]: kiểm tham số rồi
    /// dựng mô hình (một đường cấp phát duy nhất để hai constructor không lệch nhau).
    pub(super) fn with_harts_and_arena_bytes(harts: u16, bytes: usize) -> Self {
        assert!(
            harts > 0 && harts <= MAX_HARTS,
            "số hart phải trong 1..={MAX_HARTS} (nhận {harts})"
        );
        assert!(bytes > 0, "arena kernel phải có cỡ dương");
        Self {
            kernel: OwnedMem::new(bytes).expect("cấp arena kernel của Machine"),
            regions: Vec::new(),
            objects: Vec::new(),
            caps: Vec::new(),
            pair_hits: [0; TCB_STATE_COUNT * TCB_STATE_COUNT],
            forbidden_hits: [0; TCB_STATE_COUNT * TCB_STATE_COUNT],
            budget_sleeps: 0,
            budget_refills: 0,
            ipc_sent: 0,
            ipc_delivered: 0,
            ipc_dropped: 0,
            transfer_calls: 0,
            cap_transfer_installed: 0,
            cap_transfer_failed: 0,
            cap_unwrapped: 0,
            fault_events: 0,
            finalise_log: Vec::new(),
            address_waiters: std::array::from_fn(|_| Vec::new()),
            addr_words: Vec::new(),
            sched: (0..harts).map(|_| HartSched::new()).collect(),
            mail: (0..harts).map(|_| HartMailbox::new()).collect(),
            published: (0..harts)
                .map(|_| Published {
                    idle: true,
                    cur_prio: 0,
                })
                .collect(),
            current_hart: 0,
            migrate_count: 0,
            preempt_every_step: false,
            now: 0,
            clock: MonotonicClock::new(),
            frame_windows: Vec::new(),
            frame_objects: 0,
            page_tables: Vec::new(),
            page_table_objects: 0,
            vspaces: Vec::new(),
            vspace_objects: 0,
            // **KT-38** — thế hệ 0 với free-list đủ 256; mọi hart của mô hình **online**.
            asid_generation: 0,
            asid_free: fresh_asid_free(),
            asid_issued: 0,
            asid_exhaustions: 0,
            hart_online: HartMask::for_harts(harts),
            hart_vspace: (0..harts).map(|_| None).collect(),
            flush_log: Vec::new(),
            // **KT-40** — TLB rỗng từng hart; chưa có shootdown/ack nào.
            tlbs: (0..harts).map(|_| HartTlb::new()).collect(),
            tlb_pending: (0..harts).map(|_| None).collect(),
            last_tlb_shootdown: None,
            tlb_shootdown_seq: 0,
            tlb_shootdown_count: 0,
            tlb_msg_total: 0,
            tlb_ack_total: 0,
            tlb_fill_total: 0,
            tlb_wait_timeouts: 0,
        }
    }

    /// Dung lượng arena kernel (byte).
    pub fn arena_capacity(&self) -> usize {
        self.kernel.capacity()
    }

    /// Số byte arena kernel đã cấp.
    pub fn arena_used(&self) -> usize {
        self.kernel.used()
    }

    // ---------------------------------------------------------------- dựng cảnh

    /// Thêm một Untyped gốc. Vùng nhớ thật của Untyped (arena của nó) do objmem cấp với cỡ
    /// `size`; mô hình tính offset arena từ 0.
    ///
    /// **KT-36**: `base` **nay có nghĩa** — nó là đáy cửa sổ PA ảo mà Untyped cấp cho Frame
    /// (`FrameArea`, 1 GiB; `v2/docs/k2-vm.md` §10). Trước KT-36 tham số này bị bỏ qua
    /// ("chỉ để kịch bản ghi nhớ"); object không phải Frame **không** đổi hành vi. Frame lấy
    /// `pa = base + watermark` nên Untyped dùng để retype Frame phải khai `base` **căn theo cỡ
    /// Frame** và **phân biệt** giữa các Untyped (chồng `pa` ⇒ `AlreadyMapped`).
    pub fn add_untyped(&mut self, base: u64, size: usize) -> ObjId {
        assert!(size > 0, "Untyped phải có cỡ dương");
        // **KT-36** — cửa sổ PA của Untyped phải biểu diễn được: `base + 1 GiB` không tràn
        // `u64`. Từ chối **ngay ở constructor** (fail rõ, như `size > 0`) thay vì để mọi lần
        // retype Frame sau đó trả `InvalidArgument`; `FrameArea::carve` vẫn giữ kiểm tra đó như
        // lớp thứ hai cho `FrameArea` dựng trực tiếp.
        assert!(
            base.checked_add(UNTYPED_PA_WINDOW).is_some(),
            "Untyped: base {base:#x} + cửa sổ PA {UNTYPED_PA_WINDOW} tràn u64 (KT-36)"
        );
        let region = self.push_region(size);
        let obj = self.push_kernel(
            ObjType::Untyped,
            Kind::Untyped,
            ChoDat {
                region: Some(region),
                frame_area: Some(FrameArea::new(base)),
                ..ChoDat::default()
            },
        );
        self.frame_windows.push(obj as usize - 1);
        obj
    }

    /// Thêm CNode do harness cấp (init giữ CNode gốc — không cần Retype CNode ở KT-09).
    pub fn add_cnode(&mut self, slots: usize) -> ObjId {
        assert!(slots > 0, "CNode phải có ít nhất một slot");
        self.push_kernel(
            ObjType::CNode,
            Self::fresh_kind(ObjType::CNode, slots),
            ChoDat {
                slots: Some(slots),
                ..ChoDat::default()
            },
        )
    }

    /// Thêm object lõi do harness cấp (SchedControl, TCB khởi tạo, ...) — không tính byte Untyped.
    pub fn add_kernel_object(&mut self, ty: ObjType) -> ObjId {
        assert!(ty.is_core(), "chỉ object lõi mới dựng trực tiếp");
        self.push_kernel(ty, Self::fresh_kind(ty, 0), ChoDat::default())
    }

    /// Cap gốc cho một object (không có cha) — harness đặt vào CNode bằng `put_cap`.
    pub fn root_cap(&mut self, obj: ObjId) -> CapId {
        let ty = self
            .objects
            .get(obj as usize - 1)
            .map(|m| m.ty)
            .expect("root_cap: object phải tồn tại");
        self.push_cap(obj, ty, None, RIGHTS_ALL, None)
    }

    pub fn object_alive(&self, obj: ObjId) -> bool {
        self.objects
            .get(obj as usize - 1)
            .map(|m| m.alive)
            .unwrap_or(false)
    }

    pub fn object_type(&self, obj: ObjId) -> Option<ObjType> {
        self.objects.get(obj as usize - 1).map(|m| m.ty)
    }

    /// `(offset, len)` trong Untyped; `None` với object không cấp từ Untyped.
    ///
    /// **KT-36** — với `Frame`, hai giá trị này là **offset/cỡ trong cửa sổ PA** của Untyped
    /// (`len` = cỡ Frame theo byte), **không** phải byte arena: Frame không tiêu byte arena nên
    /// `check_bytes`/`untyped_allocated` bỏ qua nó (xem `check_frames`).
    pub fn object_bytes(&self, obj: ObjId) -> Option<(usize, usize)> {
        let m = self.objects.get(obj as usize - 1)?;
        m.untyped.map(|_| (m.offset, m.len))
    }

    /// Địa chỉ **thật** của body trong arena objmem (KT-16).
    pub fn object_addr(&self, obj: ObjId) -> Option<u64> {
        self.objects.get(obj as usize - 1).map(|m| m.body.pa())
    }

    /// `(base, capacity)` của vùng nhớ thật của một Untyped (KT-16); `None` nếu không phải
    /// Untyped. Object Retype từ Untyped đó nằm trong `[base, base + capacity)`.
    pub fn untyped_arena(&self, untyped: ObjId) -> Option<(u64, usize)> {
        let m = self.objects.get(untyped as usize - 1)?;
        if m.ty != ObjType::Untyped {
            return None;
        }
        let r = m.region?;
        Some((self.regions[r].base(), self.regions[r].capacity()))
    }

    /// Truy cập body qua `vifast-objmem` (KT-16): object đã finalise ⇒ `Dead`.
    ///
    /// Đây là đường duy nhất lộ body ra ngoài crate mô hình (dạng `&dyn KObj`), dùng cho
    /// test tích hợp; `ObjRef` không mang identity store nên không lộ ref thô.
    pub fn with_object<R>(&self, obj: ObjId, f: impl FnOnce(&dyn KObj) -> R) -> Result<R, Dead> {
        let Some(m) = self.objects.get(obj as usize - 1) else {
            return Err(Dead);
        };
        self.store(m.loc).with(m.body, |k| f(k))
    }

    pub fn untyped_size(&self, obj: ObjId) -> usize {
        let Some(m) = self.objects.get(obj as usize - 1) else {
            return 0;
        };
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region.map(|r| self.regions[r].capacity()).unwrap_or(0)
    }

    pub fn untyped_used(&self, obj: ObjId) -> usize {
        let Some(m) = self.objects.get(obj as usize - 1) else {
            return 0;
        };
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region.map(|r| self.regions[r].used()).unwrap_or(0)
    }

    pub fn untyped_free(&self, obj: ObjId) -> usize {
        let size = self.untyped_size(obj);
        size.saturating_sub(self.untyped_used(obj))
    }

    /// Tổng `len` của object **sống** cấp từ Untyped (không gồm byte đệm căn lề).
    pub fn untyped_allocated(&self, obj: ObjId) -> usize {
        let Some(idx) = self.objects.get(obj as usize - 1).map(|_| obj as usize - 1) else {
            return 0;
        };
        self.objects
            .iter()
            // **KT-36** — Frame không tiêu byte arena ("không cấp byte thật"), nên `len` của nó
            // (cỡ Frame) **không** vào tổng byte đã cấp của Untyped; vùng PA của Frame do
            // `check_frames` kiểm riêng.
            .filter(|m| m.alive && m.untyped == Some(idx) && m.ty != ObjType::Frame)
            .map(|m| m.len)
            .sum()
    }

    /// Byte đệm căn lề nằm giữa watermark và tổng `len` của object sống.
    pub fn untyped_gap(&self, obj: ObjId) -> usize {
        self.untyped_used(obj)
            .saturating_sub(self.untyped_allocated(obj))
    }

    /// Tổng watermark của mọi Untyped còn sống — dùng cho trace/tất định.
    pub fn total_used(&self) -> usize {
        self.objects
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::Untyped)
            .map(|m| m.region.map(|r| self.regions[r].used()).unwrap_or(0))
            .sum()
    }

    // ---------------------------------------------------------------- bất biến

    /// Bất biến tổng byte của một Untyped: không chồng lấn, không vượt biên, không đếm hai lần.
    pub fn check_bytes(&self, untyped: ObjId) -> Result<(), String> {
        let Some(m) = self.objects.get(untyped as usize - 1) else {
            return Err(format!("Untyped {untyped} không tồn tại"));
        };
        if m.ty != ObjType::Untyped {
            return Err(format!("object {untyped} không phải Untyped"));
        }
        let Some(region) = m.region else {
            return Err(format!("object {untyped} không có vùng nhớ"));
        };
        let size = self.regions[region].capacity();
        // Watermark của mô hình = `Arena::used()` của vùng (KT-16).
        let watermark = self.regions[region].used();
        if watermark > size {
            return Err(format!("watermark {watermark} > cỡ {size}"));
        }
        let u_idx = untyped as usize - 1;
        let mut ranges: Vec<(usize, usize, ObjId)> = Vec::new();
        let mut allocated = 0usize;
        for (i, o) in self.objects.iter().enumerate() {
            if !o.alive || o.untyped != Some(u_idx) {
                continue;
            }
            // **KT-36** — Frame cấp từ **cửa sổ PA**, không tiêu byte arena: `offset`/`len` của
            // nó là offset/cỡ trong cửa sổ PA chứ không phải trong arena, nên **không** vào
            // `allocated`/`ranges` của bất biến byte. Vùng PA do `check_frames` kiểm
            // (không chồng, trong cửa sổ, `pa` căn cỡ). Bỏ qua ở đây **không** nới bất biến
            // byte: Frame không chiếm byte nào của arena.
            if o.ty == ObjType::Frame {
                continue;
            }
            let end = o.offset + o.len;
            if end > size {
                return Err(format!(
                    "object {} vượt biên Untyped ({end} > {size})",
                    i + 1
                ));
            }
            // Từng range phải nằm trong vùng đã cấp (0..=watermark), không chỉ trong `size`.
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
        // Không object nào chồng lấn ⇒ tổng `len` không thể vượt watermark; byte đệm căn lề
        // (gap) là phần `watermark - allocated` và **không** thuộc object nào.
        if allocated > watermark {
            return Err(format!("tổng len {allocated} > watermark {watermark}"));
        }
        let gap = watermark - allocated;
        let free = size - watermark;
        // "đã cấp + còn trống == cỡ ban đầu", tách rõ object thật vs byte đệm căn lề.
        if allocated + gap + free != size {
            return Err(format!(
                "tổng byte sai: allocated {allocated} + gap {gap} + free {free} != {size}"
            ));
        }
        Ok(())
    }

    pub fn check_all_bytes(&self) -> Result<(), String> {
        for (i, m) in self.objects.iter().enumerate() {
            if m.alive && m.ty == ObjType::Untyped {
                self.check_bytes((i as u32) + 1)?;
            }
        }
        Ok(())
    }

    /// Finalise TCB (thiết kế §4.3 bảng Finalise; KT-18 DoD 9): gỡ khỏi mọi hàng chờ, chuyển
    /// `Dead` qua một cửa, trả `sched_context` bằng hook rỗng, ghi log.
    ///
    /// `revoke_children` gọi hàm này **trước** `destroy` trong objmem. Gỡ ready queue/refill
    /// queue là hook của KT-19/KT-20 (K1b **chưa có** scheduler).
    /// **KT-36** — finalise một Frame: đánh dấu `live = false` **chỉ khi** bộ đếm mapping đã
    /// bằng 0.
    ///
    /// Bất biến "frame chết ⇒ không còn mapping" (DoD KT-36) được cưỡng chế **ở đây**: còn
    /// mapping sống ⇒ [`KError::HasChildren`] và **không** đổi gì (không im lặng xoá mapping).
    /// Revoke Untyped đã kiểm trước để cả op là atomic; hàm này là cửa thứ hai (defence in
    /// depth) nếu có đường finalise khác trong tương lai.
    pub(super) fn finalise_frame(&mut self, frame: ObjId) -> Result<(), KError> {
        let idx = self.object_index(frame)?;
        let m = &mut self.objects[idx];
        if m.ty != ObjType::Frame {
            return Err(KError::WrongObjectType);
        }
        let Some(f) = m.frame.as_mut() else {
            return Err(KError::WrongObjectType);
        };
        if !f.counters_zero() {
            return Err(KError::HasChildren);
        }
        f.live = false;
        Ok(())
    }

    /// **KT-37** — finalise một PageTable: chỉ khi **tinh khiết** (chưa attach, `mapped == 0`,
    /// mọi PTE rỗng). Còn con/mapping/đang attach ⇒ [`KError::HasChildren`] và **không** đổi gì
    /// (không im lặng bỏ cây/mapping — cùng tinh thần `finalise_frame` của KT-36). Dữ liệu
    /// `Machine::page_tables` chuyển `None` (tombstone) để không rò 4 KiB PTE và để `check_pages`
    /// bắt được bảng chết còn dữ liệu.
    pub(super) fn finalise_page_table(&mut self, pt: ObjId) -> Result<(), KError> {
        let idx = self.object_index(pt)?;
        let m = &self.objects[idx];
        if m.ty != ObjType::PageTable {
            return Err(KError::WrongObjectType);
        }
        let pit = m.pt.ok_or(KError::WrongObjectType)?;
        let Some(d) = self.page_tables.get(pit).and_then(|d| d.as_ref()) else {
            return Err(KError::IllegalOperation);
        };
        if !d.is_pristine() {
            return Err(KError::HasChildren);
        }
        self.page_tables[pit] = None;
        Ok(())
    }

    /// **KT-38** — finalise một VSpace: root **còn sống** phải **tinh khiết** (còn mapping/bảng
    /// con ⇒ [`KError::HasChildren`] và **không** đổi gì — cùng tinh thần `finalise_page_table`);
    /// sau đó phát **một** lệnh [`FlushScope::Asid`] tới `active_on` (đã `⊆` hart online), gỡ
    /// hart khỏi `hart_vspace`, và chuyển dữ liệu `Machine::vspaces` thành tombstone `None`.
    ///
    /// **ASID không quay lại free-list** của thế hệ hiện tại ("không tái dùng trong cùng thế
    /// hệ"): nó chỉ được phát lại khi `generation += 1`. Vì vậy teardown là "giải phóng ASID"
    /// theo nghĩa **thôi sở hữu** + flush theo asid, không phải "trả về free-list".
    pub(super) fn finalise_vspace(&mut self, vs: ObjId) -> Result<(), KError> {
        let idx = self.object_index(vs)?;
        let m = &self.objects[idx];
        if m.ty != ObjType::VSpace {
            return Err(KError::WrongObjectType);
        }
        let sidx = m.space.ok_or(KError::WrongObjectType)?;
        let Some(d) = self.vspaces.get(sidx).and_then(|d| d.as_ref()).copied() else {
            return Err(KError::IllegalOperation);
        };
        // Cửa thứ hai (revoke đã kiểm trước để atomic): không im lặng bỏ cây/mapping.
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
        self.push_flush(FlushScope::Asid, d.generation, Some(d.asid), d.active_on);
        // **KT-40** — teardown VSpace: xoá mọi entry của `(generation, asid)` trên **mọi** hart
        // (broadcast của mô hình). Sau đây không hart nào được dùng translation của VSpace chết.
        for hart in 0..self.tlbs.len() {
            self.tlb_flush_hart_only(hart as u16, d.generation, d.asid, TlbFlushScope::Asid);
        }
        for hart in 0..self.hart_vspace.len() {
            if self.hart_vspace[hart] == Some(vs) {
                self.hart_vspace[hart] = None;
            }
        }
        self.vspaces[sidx] = None;
        Ok(())
    }

    pub fn finalise_tcb(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Dead {
            return Ok(());
        }
        // KT-21 DoD 5: huỷ donation trước khi finalise — không để SC trỏ tới TCB chết.
        self.release_donation_of(tcb)?;
        self.unlink_tcb_queues_keep_committed(tcb);
        let released = self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                let released = t.sched_context.take();
                t.bound_notification = None;
                Ok(released)
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.transition_tcb(tcb, NonReadyTarget::Dead)?;
        self.finalise_log.push(FinaliseEntry {
            tcb,
            state: TcbState::Dead,
            released_sched_context: released,
        });
        // KT-19 DoD 6: `transition_tcb(Dead)` đã gỡ khỏi ready queue; kiểm lại bất biến.
        self.sched_after_op(false);
        Ok(())
    }

    // ---------------------------------------------------------------- nội bộ

    /// Cấp vùng nhớ (arena) cho một Untyped; trả chỉ số trong `regions`.
    ///
    /// `id_base` khác nhau cho từng vùng để `ObjRef` của vùng này không trùng `id` vùng
    /// khác (ref không mang identity store — KT-15/KT-16); tối đa `2^40` object/vùng.
    pub(super) fn push_region(&mut self, size: usize) -> usize {
        let id_base = ((self.regions.len() as u64) + 1) << 40;
        let mem = OwnedMem::with_id_base(size, id_base).expect("cấp vùng nhớ cho Untyped");
        self.regions.push(mem);
        self.regions.len() - 1
    }

    pub(super) fn push_meta(
        &mut self,
        ty: ObjType,
        loc: Loc,
        body: ObjRef<Kind>,
        cho: ChoDat,
    ) -> ObjId {
        if ty == ObjType::Frame {
            self.frame_objects += 1;
        }
        if ty == ObjType::PageTable {
            self.page_table_objects += 1;
        }
        if ty == ObjType::VSpace {
            self.vspace_objects += 1;
        }
        self.objects.push(Meta {
            ty,
            alive: true,
            untyped: cho.untyped,
            offset: cho.offset,
            len: cho.len,
            loc,
            body,
            region: cho.region,
            slots: cho.slots,
            frame: cho.frame,
            frame_area: cho.frame_area,
            pt: cho.pt,
            space: cho.space,
        });
        self.objects.len() as ObjId
    }

    /// Đặt body của object do harness cấp vào arena kernel.
    pub(super) fn push_kernel(&mut self, ty: ObjType, kind: Kind, cho: ChoDat) -> ObjId {
        let body = self
            .kernel
            .alloc_place(kind)
            .expect("arena kernel còn chỗ (tăng bằng Machine::with_arena_bytes)");
        self.push_meta(ty, Loc::Kernel, body, cho)
    }

    pub(super) fn push_cap(
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

    pub(super) fn fresh_kind(ty: ObjType, child_slots: usize) -> Kind {
        match ty {
            ObjType::Untyped => Kind::Untyped,
            // KT-36: Frame chỉ có mốc body — dữ liệu thật ở `Meta::frame`, không cấp vùng nhớ.
            ObjType::Frame => Kind::Frame,
            // KT-37: PageTable cũng chỉ có mốc body — 512 PTE nằm ở `Machine::page_tables`.
            ObjType::PageTable => Kind::PageTable,
            // KT-38: VSpace chỉ có mốc body — metadata (asid/gen/active_on) ở `Machine::vspaces`.
            ObjType::VSpace => Kind::VSpace,
            ObjType::CNode => Kind::CNode {
                slots: vec![None; child_slots.max(1)],
            },
            ObjType::Tcb => Kind::Tcb(Box::new(TcbData {
                state: TcbState::Inactive,
                suspended: false,
                exit_code: None,
                fault_ep: None,
                cspace: None,
                priority: 0,
                max_controlled_priority: 0,
                affinity: AFFINITY_HOME,
                home: 0,
                sched_context: None,
                donated_sc: None,
                bound_notification: None,
                ipc: IpcArea::new(),
                recv_window: None,
                last_ipc_error: None,
                last_fault: None,
                notif: None,
            })),
            ObjType::SchedContext => Kind::SchedContext(Box::new(SchedData {
                bound: None,
                budget: ScBudget::new(),
                donated_to: None,
                donor: None,
            })),
            ObjType::Endpoint => Kind::Endpoint(Box::default()),
            ObjType::Reply => Kind::Reply(Box::new(ReplyData { caller: None })),
            ObjType::Notification => Kind::Notification(Box::default()),
            _ => Kind::Plain,
        }
    }

    pub(super) fn object_index(&self, obj: ObjId) -> Result<usize, KError> {
        if obj == 0 || obj as usize > self.objects.len() {
            return Err(KError::InvalidCapability);
        }
        Ok(obj as usize - 1)
    }

    pub(super) fn meta(&self, obj: ObjId) -> Result<&Meta, KError> {
        Ok(&self.objects[self.object_index(obj)?])
    }

    pub(super) fn store(&self, loc: Loc) -> &OwnedMem {
        match loc {
            Loc::Kernel => &self.kernel,
            Loc::Untyped(i) => &self.regions[i as usize],
        }
    }

    pub(super) fn store_mut(&mut self, loc: Loc) -> &mut OwnedMem {
        match loc {
            Loc::Kernel => &mut self.kernel,
            Loc::Untyped(i) => &mut self.regions[i as usize],
        }
    }

    /// Truy cập body qua store chứa nó; lỗi objmem (`Dead`) quy về `InvalidCapability`
    /// như `object()` cũ (mọi thao tác công khai đều kiểm `cap`/`require_*` trước).
    pub(super) fn body<R>(
        &self,
        obj: ObjId,
        f: impl FnOnce(&Kind) -> Result<R, KError>,
    ) -> Result<R, KError> {
        let (loc, body) = {
            let m = self.meta(obj)?;
            (m.loc, m.body)
        };
        self.store(loc)
            .with(body, f)
            .map_err(|_| KError::InvalidCapability)?
    }

    pub(super) fn body_mut<R>(
        &mut self,
        obj: ObjId,
        f: impl FnOnce(&mut Kind) -> Result<R, KError>,
    ) -> Result<R, KError> {
        let (loc, body) = {
            let m = self.meta(obj)?;
            (m.loc, m.body)
        };
        self.store_mut(loc)
            .with_mut(body, f)
            .map_err(|_| KError::InvalidCapability)?
    }
}
