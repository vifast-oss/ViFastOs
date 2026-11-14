// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-38 — **VSpace + ASID (8 bit = 256/thế hệ) + generation + `active_on: HartMask`**,
//! model-only `[TK]` (K2-PLAN §3 KT-38 + §2 Q4/Q9, D2-65, `v2/docs/k2-vm.md` §12, thiết kế
//! §4.10 `:353-366`).
//!
//! **Tạo VSpace bằng model API `vspace_create`** — `Untyped.Retype(VSpace)` **vẫn** `NotSupported`
//! (spec `reserved`/`unsupported`; conformance + `kt09_untyped` giữ nguyên; K2-PLAN §3 KT-38
//! "tạo VSpace root (model API)").
//!
//! **KT-38 KHÔNG có TLB/shootdown** (KT-40), **không** W^X/protect (KT-39), **không** VM fault
//! (KT-41). Ở đây chỉ có:
//! 1. `VSpace` là object `reserved` id 9 (spec **không** đổi) + **một** root PageTable thật của
//!    mô hình; `page_table_walk(root)` là **oracle** cho `vspace_walk`.
//! 2. ASID 8 bit: free-list 256/thế hệ, **không tái dùng trong cùng thế hệ**, cạn ⇒
//!    `generation += 1` + **một** lệnh flush theo `gen` cho mọi hart online (Q4).
//! 3. `active_on: HartMask` — hart chạy/nghỉ trên VSpace; `active_on ⊆ hart online`; teardown
//!    phát lệnh flush theo `asid` tới `active_on`; hart offline không nhận shootdown.
//! 4. `check_vspace`/`check_asid` sau **mỗi** op; property 10.000 vòng tạo/huỷ VSpace.
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt38-red-compile.log`](../../agent/JOURNAL/kt38-red-compile.log)
//! — API KT-38 chưa có (E0432 `HartMask/FlushCmd/...` + E0599 `vspace_create/vspace_*`) ⇒
//! `could not compile`. KT-38 **không đổi** ngữ nghĩa API cũ (VSpace vẫn `reserved` ⇒ không có ca
//! đỏ ngữ nghĩa; xem `JOURNAL/kt38-journal.md` §1).

use vifast_sim::kobj::{
    AsidState, FlushScope, HartMask, KError, Machine, MapAttrs, MapRights, ObjId, ObjType, SlotRef,
    ASID_COUNT, FLAG_GUEST,
};

const K4: u64 = 1 << 12;
const M2: u64 = 1 << 21;
const G1: u64 = 1 << 30;

// ===========================================================================
// Rig: CNode 1024 slot + Untyped cho root VSpace/PageTable + 3 Untyped Frame
// (4 KiB / 2 MiB / 1 GiB). Frame **không** tiêu byte arena (KT-36).
// ===========================================================================

struct Rig {
    m: Machine,
    c: ObjId,
    /// Untyped chứa root PageTable của VSpace (và PageTable cấp dưới của test).
    uva: ObjId,
    /// Untyped cho Frame 4 KiB (cửa sổ PA 1 GiB ở 4 GiB).
    ufr: ObjId,
    /// Untyped cho Frame 2 MiB (cửa sổ PA 1 GiB ở 8 GiB).
    ufr2: ObjId,
    /// Untyped cho Frame 1 GiB (cửa sổ PA 1 GiB ở 16 GiB).
    ufr1: ObjId,
    sva: SlotRef,
    sfr: SlotRef,
    sfr2: SlotRef,
    sfr1: SlotRef,
    next: usize,
}

impl Rig {
    fn new(harts: u16, va_bytes: usize) -> Self {
        let mut m = Machine::with_harts(harts);
        let c = m.add_cnode(1024);
        let uva = m.add_untyped(0, va_bytes);
        let ufr = m.add_untyped(1 << 32, 4096);
        let ufr2 = m.add_untyped(1 << 33, 4096);
        let ufr1 = m.add_untyped(1 << 34, 4096);
        for (slot, u) in [(0usize, uva), (1, ufr), (2, ufr2), (3, ufr1)] {
            let cap = m.root_cap(u);
            m.put_cap(c, slot, cap).expect("cap Untyped");
        }
        Self {
            m,
            c,
            uva,
            ufr,
            ufr2,
            ufr1,
            sva: SlotRef::new(c, 0),
            sfr: SlotRef::new(c, 1),
            sfr2: SlotRef::new(c, 2),
            sfr1: SlotRef::new(c, 3),
            next: 4,
        }
    }

    fn slot(&mut self) -> usize {
        let s = self.next;
        self.next += 1;
        s
    }

    fn put(&mut self, cap: vifast_sim::kobj::CapId) -> SlotRef {
        let s = self.slot();
        self.put_at(cap, s)
    }

    fn put_at(&mut self, cap: vifast_sim::kobj::CapId, s: usize) -> SlotRef {
        self.m.put_cap(self.c, s, cap).expect("đặt cap vào CNode");
        SlotRef::new(self.c, s)
    }

    /// Tạo VSpace bằng **model API** `vspace_create` + đặt cap vào slot kế; trả `(slot, ObjId)`.
    fn vspace(&mut self) -> (SlotRef, ObjId) {
        let cap = self.m.vspace_create(self.sva).expect("vspace_create");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Như [`Rig::vspace`] nhưng đặt cap vào **slot cố định** (vòng 10.000: slot được giải phóng
    /// sau mỗi `Revoke` nên dùng lại được, không tràn CNode).
    fn vspace_at(&mut self, slot: usize) -> (SlotRef, ObjId) {
        let cap = self.m.vspace_create(self.sva).expect("vspace_create");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put_at(cap, slot), obj)
    }

    /// Retype PageTable (cấp dưới của cây) + đặt cap; trả `(slot, ObjId)`.
    fn tbl(&mut self) -> (SlotRef, ObjId) {
        let cap = self
            .m
            .retype(self.sva, ObjType::PageTable, 0, 0)
            .expect("retype PageTable");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Retype Frame cỡ `size_bits` (12/21/30) từ Untyped tương ứng; trả `(slot, ObjId)`.
    fn frame(&mut self, size_bits: u32) -> (SlotRef, ObjId) {
        let src = match size_bits {
            30 => self.sfr1,
            21 => self.sfr2,
            _ => self.sfr,
        };
        let cap = self
            .m
            .retype(src, ObjType::Frame, size_bits, 0)
            .expect("retype Frame");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Bất biến sau mỗi op: VSpace + ASID + PageTable + Frame + cap + byte.
    fn kiem(&self, nhan: &str) {
        self.m
            .check_vspace()
            .unwrap_or_else(|e| panic!("{nhan}: check_vspace: {e}"));
        self.m
            .check_asid()
            .unwrap_or_else(|e| panic!("{nhan}: check_asid: {e}"));
        self.m
            .check_pages()
            .unwrap_or_else(|e| panic!("{nhan}: check_pages: {e}"));
        self.m
            .check_frames()
            .unwrap_or_else(|e| panic!("{nhan}: check_frames: {e}"));
        self.m
            .check_caps()
            .unwrap_or_else(|e| panic!("{nhan}: check_caps: {e}"));
        self.m
            .check_all_bytes()
            .unwrap_or_else(|e| panic!("{nhan}: check_bytes: {e}"));
        assert_eq!(
            self.m.untyped_used(self.ufr),
            0,
            "{nhan}: Frame không tiêu byte arena"
        );
        assert_eq!(
            self.m.untyped_used(self.ufr2),
            0,
            "{nhan}: Frame 2 MiB không tiêu byte arena"
        );
        assert_eq!(
            self.m.untyped_used(self.ufr1),
            0,
            "{nhan}: Frame 1 GiB không tiêu byte arena"
        );
    }
}

/// Map Frame 4 KiB vào VSpace: dựng chuỗi root → mid (cấp 1) → leaf (cấp 0).
fn cay_4k(r: &mut Rig, vs: ObjId, base: u64) -> (ObjId, ObjId) {
    let root = r.m.vspace_root(vs).expect("root VSpace");
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, base, mid).expect("attach mid");
    let (_, leaf) = r.tbl();
    r.m.page_table_attach(mid, base, leaf).expect("attach leaf");
    (mid, leaf)
}

// ===========================================================================
// 1. Retype VSpace: metadata + root PageTable 4 KiB; ASID đầu tiên; walk oracle 3 cỡ.
// ===========================================================================

#[test]
fn retype_vspace_root_asid_va_walk_oracle() {
    let mut r = Rig::new(1, 512 * 1024);
    assert_eq!(r.m.vspace_count(), 0);
    assert_eq!(
        r.m.asid_state(),
        AsidState {
            generation: 0,
            free: ASID_COUNT,
            issued: 0,
            exhaustions: 0,
        },
        "khởi tạo: free-list đủ 256, chưa cấp ASID nào"
    );
    assert_eq!(r.m.hart_online(), HartMask::from_bits(0b1));

    // Model API: `vspace_create` cấp root 4096 B + ASID; `Retype(VSpace)` **vẫn** `NotSupported`
    // (spec `reserved`, conformance + test cũ `kt09_untyped` giữ nguyên).
    let (_, vs0) = r.vspace();
    let (_, vs12) = r.vspace();
    assert_eq!(
        r.m.retype(r.sva, ObjType::VSpace, 0, 0),
        Err(KError::NotSupported),
        "VSpace vẫn `reserved`/`unsupported` trong spec: ABI Retype không mở"
    );
    assert_eq!(r.m.vspace_count(), 2);
    assert_eq!(
        r.m.page_table_count(),
        2,
        "mỗi VSpace đúng **một** root PageTable"
    );
    r.kiem("retype VSpace");

    // `VSpace.GetAsid` (debug): ASID đầu tiên là 0, kế là 1 — cùng thế hệ 0.
    assert_eq!(r.m.vspace_get_asid(vs0), Ok(0));
    assert_eq!(r.m.vspace_get_asid(vs12), Ok(1));
    let info0 = r.m.vspace_info(vs0).expect("info vs0");
    assert_eq!(info0.generation, 0);
    assert_eq!(info0.active_on, HartMask::EMPTY);
    let info12 = r.m.vspace_info(vs12).expect("info vs12");
    assert_eq!(info12.asid, 1);
    assert_eq!(
        r.m.asid_state(),
        AsidState {
            generation: 0,
            free: ASID_COUNT - 2,
            issued: 2,
            exhaustions: 0,
        }
    );
    assert_eq!(
        r.m.flush_count(),
        0,
        "chưa teardown/cạn ASID ⇒ chưa có lệnh flush"
    );

    // Root là bảng gốc **cấp 2 chưa attach**, rỗng; VSpace tiêu **đúng một trang** arena.
    let root = r.m.vspace_root(vs0).expect("root");
    let pinfo = r.m.page_table_info(root).expect("page_table_info root");
    assert_eq!(pinfo.level, 2);
    assert_eq!(pinfo.attached, None);
    assert_eq!(pinfo.entries, 0);
    assert_eq!(pinfo.mapped, 0);
    assert_eq!(
        r.m.untyped_used(r.uva),
        2 * K4 as usize,
        "VSpace chỉ tiêu 4 KiB cho root; bản thân VSpace 0 byte"
    );
    assert_eq!(r.m.vspace_walk(vs0, 0), Err(KError::NotMapped));
    assert_eq!(r.m.page_table_walk(root, 0), Err(KError::NotMapped));

    // ---- 4 KiB: map qua VSpace, walk qua VSpace == walk qua root (oracle) ----
    let (_, _leaf) = cay_4k(&mut r, vs0, 0);
    let (sf, f4) = r.frame(12);
    let map4 =
        r.m.vspace_map(vs0, K4, sf, MapRights::READ, MapAttrs::WB)
            .expect("map 4 KiB qua VSpace");
    assert_eq!(map4.size_bits, 12);
    assert_eq!(map4.frame, f4);
    assert_eq!(map4.va, K4);
    for va in [K4, K4 + 1, K4 + 0xfff] {
        assert_eq!(
            r.m.vspace_walk(vs0, va),
            r.m.page_table_walk(root, va),
            "4 KiB: vspace_walk ≠ oracle tại va {va:#x}"
        );
    }
    assert_eq!(r.m.vspace_walk(vs0, 0), Err(KError::NotMapped));

    // ---- 2 MiB: lá ở cấp 1 ----
    let root12 = r.m.vspace_root(vs12).expect("root vs12");
    let (_, mid2) = r.tbl();
    r.m.page_table_attach(root12, 0, mid2).expect("attach mid2");
    let (sf2, _f2) = r.frame(21);
    let map2 =
        r.m.vspace_map(
            vs12,
            M2,
            sf2,
            MapRights::READ | MapRights::WRITE,
            MapAttrs::WB,
        )
        .expect("map 2 MiB");
    assert_eq!(map2.size_bits, 21);
    assert_eq!(map2.va, M2);
    for va in [M2, M2 + 1, 3 * M2 / 2] {
        assert_eq!(r.m.vspace_walk(vs12, va), r.m.page_table_walk(root12, va));
    }

    // ---- 1 GiB: lá ngay ở root (cấp 2) ----
    let (sf1, _f1) = r.frame(30);
    let map1 =
        r.m.vspace_map(vs12, G1, sf1, MapRights::READ, MapAttrs::WB)
            .expect("map 1 GiB");
    assert_eq!(map1.size_bits, 30);
    for va in [G1, G1 + K4, G1 + M2, G1 + G1 / 2] {
        assert_eq!(r.m.vspace_walk(vs12, va), r.m.page_table_walk(root12, va));
    }
    r.kiem("walk oracle 3 cỡ");

    // Unmap qua VSpace cũng uỷ quyền `page_table_unmap` (walk hết khớp).
    assert_eq!(
        r.m.vspace_unmap(vs0, K4).map(|m| m.frame),
        Ok(f4),
        "vspace_unmap trả mapping vừa gỡ"
    );
    assert_eq!(r.m.vspace_walk(vs0, K4), Err(KError::NotMapped));
    assert_eq!(r.m.page_table_walk(root, K4), Err(KError::NotMapped));
    r.kiem("unmap");

    // Hết arena ⇒ `NotEnoughMemory`, **atomic**: không cấp ASID (không đốt thế hệ).
    let utiny = r.m.add_untyped(1 << 40, 4096);
    let cap = r.m.root_cap(utiny);
    let s_tiny = r.put(cap);
    let cap_vs = r.m.vspace_create(s_tiny).expect("VSpace từ Untyped 4 KiB");
    let vs_tiny = r.m.cap_info(cap_vs).expect("cap").obj;
    r.put(cap_vs);
    let before = (
        r.m.vspace_count(),
        r.m.page_table_count(),
        r.m.asid_state(),
        r.m.untyped_used(utiny),
        r.m.flush_count(),
    );
    assert_eq!(
        r.m.vspace_create(s_tiny),
        Err(KError::NotEnoughMemory),
        "root 4 KiB không còn chỗ trong Untyped 4 KiB"
    );
    assert_eq!(
        (
            r.m.vspace_count(),
            r.m.page_table_count(),
            r.m.asid_state(),
            r.m.untyped_used(utiny),
            r.m.flush_count(),
        ),
        before,
        "vspace_create lỗi phải atomic (không cấp ASID/thế hệ)"
    );
    let info_tiny = r.m.vspace_info(vs_tiny).expect("info tiny");
    assert_eq!(info_tiny.asid, 2, "ASID kế tiếp của VSpace 4 KiB");
    // FLAG_GUEST của VSpace vẫn NotSupported (D2-32) — KT-38 không mở VSpace khách.
    assert_eq!(
        r.m.retype(r.sva, ObjType::VSpace, 0, FLAG_GUEST),
        Err(KError::NotSupported)
    );
    r.kiem("vspace_create lỗi");
}

// ===========================================================================
// 2. Cấp 257 ASID: cạn ⇒ generation + 1 đúng **một lần**, flush theo `gen` một lần,
//    ASID cũ không tái dùng trong thế hệ cũ; teardown giữ free-list của thế hệ hiện tại.
// ===========================================================================

#[test]
fn cap_257_asid_generation_va_flush_theo_gen() {
    // D2-47(a): stable chạy **đúng 257** (256 ASID/gen 0 + ASID 257 ⇒ gen 1); dưới Miri chỉ 8
    // ASID (không cần đúng 256 để kiểm UB) — ca "cạn ASID ⇒ gen + 1" rẻ chạy dưới Miri nằm ở
    // unit test `kobj::tests::check_asid_can_asid_tang_the_he_mot_lan_va_flush_gen`.
    #[cfg(miri)]
    const SO_ASID: usize = 8;
    #[cfg(not(miri))]
    const SO_ASID: usize = ASID_COUNT;
    let full = SO_ASID == ASID_COUNT;
    #[cfg(miri)]
    let va_bytes = 64 * 1024;
    #[cfg(not(miri))]
    let va_bytes = 2 * 1024 * 1024;
    let mut r = Rig::new(1, va_bytes);
    let mut first_vs = 0;
    let mut seen: Vec<(u64, u16)> = Vec::new();
    for i in 0..SO_ASID {
        let (_, vs) = r.vspace();
        if i == 0 {
            first_vs = vs;
        }
        let info = r.m.vspace_info(vs).expect("info");
        assert_eq!(info.generation, 0, "vòng {i}: còn thế hệ 0");
        assert_eq!(info.asid, i as u16, "vòng {i}: ASID cấp theo thứ tự");
        assert_eq!(r.m.vspace_get_asid(vs), Ok(i as u16));
        seen.push((info.generation, info.asid));
        assert_eq!(r.m.asid_state().free, ASID_COUNT - (i + 1));
        assert_eq!(r.m.asid_state().issued, (i + 1) as u32);
        if i % 32 == 0 {
            r.kiem("cấp ASID");
        }
    }
    assert_eq!(
        r.m.asid_state(),
        AsidState {
            generation: 0,
            free: ASID_COUNT - SO_ASID,
            issued: SO_ASID as u32,
            exhaustions: 0,
        }
    );
    assert_eq!(
        r.m.flush_count(),
        0,
        "chưa cạn ASID ⇒ chưa có lệnh flush theo gen"
    );
    let mut teardown_flushes = SO_ASID as u64;

    // ---- ASID thứ 257 (stable) ⇒ generation += 1 **đúng một lần** + **một** lệnh flush `gen` ----
    if full {
        let (_, vs257) = r.vspace();
        let info = r.m.vspace_info(vs257).expect("info 257");
        assert_eq!(info.generation, 1, "cạn 256 ASID ⇒ thế hệ 1");
        assert_eq!(info.asid, 0, "thế hệ mới nạp lại free-list từ 0");
        seen.push((info.generation, info.asid));
        assert_eq!(
            r.m.asid_state(),
            AsidState {
                generation: 1,
                free: ASID_COUNT - 1,
                issued: 1,
                exhaustions: 1,
            }
        );
        assert_eq!(r.m.flush_count(), 1, "flush toàn bộ **một lần** theo gen");
        let f = r.m.last_flush().expect("lệnh flush gen");
        assert_eq!(f.seq, 1);
        assert_eq!(f.scope, FlushScope::Generation);
        assert_eq!(f.generation, 1);
        assert_eq!(f.asid, None);
        assert_eq!(
            f.harts,
            HartMask::from_bits(0b1),
            "flush gen tới **mọi** hart online (1 hart)"
        );

        // ASID cũ **không** bị đụng: VSpace đầu vẫn (asid 0, thế hệ 0) và **không trùng** trong gen.
        assert_eq!(r.m.vspace_info(first_vs).expect("first").asid, 0);
        assert_eq!(r.m.vspace_info(first_vs).expect("first").generation, 0);
        assert_eq!(seen.len(), ASID_COUNT + 1);
        for i in 0..seen.len() {
            for j in (i + 1)..seen.len() {
                assert_ne!(seen[i], seen[j], "ASID trùng trong một thế hệ: {seen:?}");
            }
        }
        teardown_flushes = ASID_COUNT as u64 + 1;
    }
    r.kiem("sau cấp ASID");

    // ---- Teardown toàn bộ: mỗi VSpace một lệnh flush theo asid; free-list thế hệ hiện tại
    //      **không** đổi (ASID đã cấp về hưu, không quay lại free-list) ----
    r.m.revoke(r.c, Some(0)).expect("revoke Untyped VSpace");
    assert_eq!(r.m.vspace_count(), 0);
    assert_eq!(r.m.page_table_count(), 0);
    assert_eq!(r.m.untyped_used(r.uva), 0, "arena reset sau Revoke");
    assert_eq!(
        r.m.flush_count(),
        teardown_flushes + u64::from(full),
        "mỗi teardown đúng một lệnh `Asid` (+ một lệnh `Generation` nếu cạn ASID)"
    );
    let expected_after_teardown = if full {
        AsidState {
            generation: 1,
            free: ASID_COUNT - 1,
            issued: 1,
            exhaustions: 1,
        }
    } else {
        AsidState {
            generation: 0,
            free: ASID_COUNT - SO_ASID,
            issued: SO_ASID as u32,
            exhaustions: 0,
        }
    };
    assert_eq!(
        r.m.asid_state(),
        expected_after_teardown,
        "teardown không được đụng free-list của thế hệ hiện tại"
    );
    let last = r.m.last_flush().expect("flush cuối");
    assert_eq!(last.scope, FlushScope::Asid);
    assert_eq!(
        last.asid,
        Some(if full { 0 } else { 7 }),
        "ASID của VSpace cuối cùng bị huỷ"
    );
    assert_eq!(last.generation, u64::from(full));
    assert_eq!(last.seq, teardown_flushes + u64::from(full));
    r.kiem("sau teardown");

    // Thế hệ hiện tại **không** phát lại ASID đã cấp ⇒ VSpace mới nhận ASID kế tiếp.
    let (_, vs_new) = r.vspace();
    let info = r.m.vspace_info(vs_new).expect("info mới");
    assert_eq!(info.generation, u64::from(full));
    assert_eq!(
        info.asid,
        if full { 1 } else { SO_ASID as u16 },
        "ASID đã cấp trong thế hệ **không** tái dùng"
    );
    r.kiem("tạo lại sau teardown");
}

// ===========================================================================
// 3. ASID **không tái dùng trong cùng thế hệ**: tạo → huỷ → tạo lại vẫn là ASID kế tiếp;
//    teardown phát lệnh flush theo asid (harts rỗng khi chưa hart nào chạy).
// ===========================================================================

#[test]
fn asid_khong_tai_dung_trong_cung_the_he() {
    let mut r = Rig::new(1, 64 * 1024);
    let mut asids: Vec<u16> = Vec::new();
    for i in 0..6u16 {
        let (_, vs) = r.vspace();
        let info = r.m.vspace_info(vs).expect("info");
        assert_eq!(info.generation, 0, "vòng {i}: vẫn thế hệ 0");
        assert_eq!(
            info.asid, i,
            "vòng {i}: ASID không tái dùng trong cùng thế hệ"
        );
        asids.push(info.asid);
        r.kiem("tạo VSpace");

        r.m.revoke(r.c, Some(0)).expect("revoke Untyped VSpace");
        assert_eq!(r.m.vspace_count(), 0);
        assert_eq!(r.m.page_table_count(), 0);
        assert_eq!(r.m.untyped_used(r.uva), 0);
        let f = r.m.last_flush().expect("flush teardown");
        assert_eq!(f.scope, FlushScope::Asid);
        assert_eq!(f.asid, Some(i), "flush đúng asid của VSpace bị huỷ");
        assert_eq!(f.generation, 0);
        assert_eq!(
            f.harts,
            HartMask::EMPTY,
            "chưa hart nào chạy VSpace ⇒ không shootdown hart nào"
        );
        r.kiem("sau teardown");
    }
    assert_eq!(asids, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(
        r.m.asid_state(),
        AsidState {
            generation: 0,
            free: ASID_COUNT - 6,
            issued: 6,
            exhaustions: 0,
        }
    );
    assert_eq!(r.m.flush_count(), 6);
}

// ===========================================================================
// 4. `active_on`: hart chạy/nghỉ; tối đa một VSpace mỗi hart; hart offline không activate.
// ===========================================================================

#[test]
fn active_on_hart_activate_deactivate() {
    let mut r = Rig::new(4, 64 * 1024);
    let (_, a) = r.vspace();
    let (_, b) = r.vspace();
    assert_eq!(r.m.hart_online(), HartMask::from_bits(0b1111));

    r.m.vspace_activate(a, 0).expect("activate a@0");
    assert_eq!(r.m.vspace_active_on(a), Ok(HartMask::from_bits(0b0001)));
    r.m.vspace_activate(a, 0).expect("activate lặp là no-op");
    assert_eq!(r.m.vspace_active_on(a), Ok(HartMask::from_bits(0b0001)));
    r.m.vspace_activate(a, 2).expect("activate a@2");
    assert_eq!(r.m.vspace_active_on(a), Ok(HartMask::from_bits(0b0101)));
    r.kiem("activate a");

    // Hart 0 chuyển sang VSpace b ⇒ bỏ khỏi `active_on` của a (một ASID hiện hành/hart).
    r.m.vspace_activate(b, 0).expect("activate b@0");
    assert_eq!(r.m.vspace_active_on(a), Ok(HartMask::from_bits(0b0100)));
    assert_eq!(r.m.vspace_active_on(b), Ok(HartMask::from_bits(0b0001)));
    r.kiem("hart chuyển VSpace");

    r.m.vspace_deactivate(a, 2).expect("deactivate a@2");
    assert_eq!(r.m.vspace_active_on(a), Ok(HartMask::EMPTY));
    assert_eq!(
        r.m.vspace_deactivate(a, 2),
        Err(KError::IllegalOperation),
        "hart không chạy VSpace này ⇒ IllegalOperation"
    );
    assert_eq!(
        r.m.vspace_deactivate(b, 1),
        Err(KError::IllegalOperation),
        "hart 1 chưa từng activate"
    );
    r.kiem("deactivate");

    // Hart không có thật / hart offline ⇒ InvalidArgument (bất biến `active_on ⊆ online`).
    assert_eq!(r.m.vspace_activate(a, 4), Err(KError::InvalidArgument));
    assert_eq!(r.m.vspace_activate(a, 99), Err(KError::InvalidArgument));
    assert_eq!(r.m.set_hart_online(4, false), Err(KError::InvalidArgument));
    r.m.set_hart_online(3, false).expect("hart 3 offline");
    assert_eq!(r.m.hart_online(), HartMask::from_bits(0b0111));
    assert_eq!(
        r.m.vspace_activate(a, 3),
        Err(KError::InvalidArgument),
        "hart offline không được activate"
    );
    r.m.set_hart_online(3, true).expect("hart 3 online lại");
    assert_eq!(r.m.hart_online(), HartMask::from_bits(0b1111));
    r.m.vspace_activate(a, 3)
        .expect("activate sau khi online lại");
    assert_eq!(r.m.vspace_active_on(a), Ok(HartMask::from_bits(0b1000)));
    r.kiem("hart online/offline");
}

// ===========================================================================
// 5. Hart offline bị bỏ khỏi `active_on` ⇒ **không** nhận shootdown khi teardown.
// ===========================================================================

#[test]
fn active_on_hart_offline_khong_shootdown() {
    let mut r = Rig::new(4, 64 * 1024);
    let (_, vs) = r.vspace();
    r.m.vspace_activate(vs, 0).expect("activate @0");
    r.m.vspace_activate(vs, 2).expect("activate @2");
    assert_eq!(r.m.vspace_active_on(vs), Ok(HartMask::from_bits(0b0101)));
    r.kiem("2 hart chạy VSpace");

    // Hart 2 offline ⇒ **bỏ khỏi mọi** active_on (không còn chạy VSpace nào).
    r.m.set_hart_online(2, false).expect("hart 2 offline");
    assert_eq!(r.m.vspace_active_on(vs), Ok(HartMask::from_bits(0b0001)));
    assert_eq!(r.m.hart_online(), HartMask::from_bits(0b1011));
    r.kiem("hart 2 offline");

    // Teardown: lệnh flush theo asid chỉ tới hart 0 — **không** gửi shootdown cho hart offline.
    r.m.revoke(r.c, Some(0)).expect("revoke Untyped VSpace");
    assert_eq!(r.m.vspace_count(), 0);
    let f = r.m.last_flush().expect("flush teardown");
    assert_eq!(f.scope, FlushScope::Asid);
    assert_eq!(f.harts, HartMask::from_bits(0b0001));
    assert!(
        !f.harts.contains(2),
        "hart 2 offline không được nhận shootdown: {f:?}"
    );
    for c in r.m.flush_log() {
        assert!(
            c.harts.is_subset_of(HartMask::from_bits(0b1011)),
            "mọi lệnh flush chỉ tới hart online: {c:?}"
        );
    }
    r.kiem("teardown với hart offline");
}

// ===========================================================================
// 6. Lỗi/biên của API VSpace; root không được attach; teardown còn mapping ⇒ HasChildren.
// ===========================================================================

#[test]
fn vspace_loi_va_bien() {
    let mut r = Rig::new(2, 256 * 1024);
    let (_, vs) = r.vspace();
    let root = r.m.vspace_root(vs).expect("root");
    let (_, tbl) = r.tbl();

    // ABI: `Retype(VSpace)` vẫn `NotSupported` (spec `reserved`); tạo bằng model API.
    assert_eq!(
        r.m.retype(r.sva, ObjType::VSpace, 0, 0),
        Err(KError::NotSupported)
    );
    // Object không phải VSpace ⇒ WrongObjectType ở **mọi** cửa VSpace.
    assert_eq!(r.m.vspace_info(tbl), Err(KError::WrongObjectType));
    assert_eq!(r.m.vspace_root(tbl), Err(KError::WrongObjectType));
    assert_eq!(r.m.vspace_get_asid(tbl), Err(KError::WrongObjectType));
    assert_eq!(r.m.vspace_active_on(tbl), Err(KError::WrongObjectType));
    assert_eq!(r.m.vspace_walk(tbl, 0), Err(KError::WrongObjectType));
    assert_eq!(r.m.vspace_activate(tbl, 0), Err(KError::WrongObjectType));
    assert_eq!(r.m.vspace_deactivate(tbl, 0), Err(KError::WrongObjectType));
    assert_eq!(
        r.m.vspace_map(tbl, 0, SlotRef::new(r.c, 1), MapRights::READ, MapAttrs::WB),
        Err(KError::WrongObjectType)
    );

    // Root PageTable của VSpace là object nội bộ: **không** được attach làm bảng con.
    assert_eq!(
        r.m.page_table_attach(tbl, 0, root),
        Err(KError::IllegalOperation),
        "root VSpace phải luôn là bảng gốc"
    );
    assert_eq!(r.m.page_table_info(root).expect("root").attached, None);
    r.kiem("lỗi kiểu + root");

    // VSpace còn mapping ⇒ Revoke Untyped **HasChildren**, không đổi gì (atomic).
    let (mid, _leaf) = cay_4k(&mut r, vs, 0);
    let (sf, f4) = r.frame(12);
    r.m.vspace_map(vs, K4, sf, MapRights::READ, MapAttrs::WB)
        .expect("map trước khi revoke");
    let before = (r.m.vspace_count(), r.m.asid_state(), r.m.flush_count());
    assert_eq!(
        r.m.revoke(r.c, Some(0)),
        Err(KError::HasChildren),
        "VSpace còn mapping không được chết"
    );
    assert_eq!(
        (r.m.vspace_count(), r.m.asid_state(), r.m.flush_count()),
        before,
        "revoke lỗi phải atomic"
    );
    assert_eq!(r.m.vspace_walk(vs, K4).map(|m| m.frame), Ok(f4));
    r.kiem("revoke HasChildren");

    // Gỡ mapping rồi **detach** cây (leaf khỏi mid, mid khỏi root) ⇒ root tinh khiết.
    r.m.vspace_unmap(vs, K4).expect("unmap");
    r.m.page_table_detach(mid, 0).expect("detach leaf");
    r.m.page_table_detach(root, 0).expect("detach mid");
    assert_eq!(r.m.page_table_info(root).expect("root").mapped, 0);
    r.kiem("sau unmap + detach");

    // Teardown đường chính: root tinh khiết ⇒ revoke xoá cả VSpace + root, phát flush asid.
    r.m.revoke(r.c, Some(0)).expect("revoke VSpace rỗng");
    assert_eq!(r.m.vspace_count(), 0);
    assert_eq!(r.m.page_table_count(), 0);
    let f = r.m.last_flush().expect("flush teardown");
    assert_eq!(f.scope, FlushScope::Asid);
    assert_eq!(f.asid, Some(0));
    assert_eq!(r.m.flush_count(), 1);
    // VSpace đã chết: mọi cửa trả InvalidCapability (không trả dữ liệu cũ).
    assert_eq!(r.m.vspace_info(vs), Err(KError::InvalidCapability));
    assert_eq!(r.m.vspace_root(vs), Err(KError::InvalidCapability));
    assert_eq!(r.m.vspace_get_asid(vs), Err(KError::InvalidCapability));
    assert_eq!(r.m.vspace_walk(vs, 0), Err(KError::InvalidCapability));
    assert_eq!(
        r.m.vspace_map(vs, 0, SlotRef::new(r.c, 1), MapRights::READ, MapAttrs::WB),
        Err(KError::InvalidCapability)
    );
    r.kiem("sau teardown");
}

// ===========================================================================
// 7. Property **10.000 thao tác** tạo/huỷ VSpace: ASID không trùng trong thế hệ, `check_vspace`
//    + `check_asid` sau **mỗi** op, không panic, không rò arena.
//
// Chia thành `SO_EPOCH` × `MOI_EPOCH` (mỗi epoch một `Machine` mới) vì `check_*` của harness
// hiện chạy sau **mỗi** public op và cấp phát theo `objects.len()` (append-only: object chết vẫn
// nằm trong bảng). Một `Machine` cho cả 10.000 vòng sẽ là O(n²) ≈ 4 phút (số đo trong
// `JOURNAL/kt38-journal.md` §4); tổng số thao tác tạo/huỷ vẫn đúng **10.000**, mỗi epoch vẫn
// kiểm đủ bất biến ASID/VSpace sau từng op.
// ===========================================================================

#[test]
fn property_10k_tao_huy_vspace() {
    #[cfg(miri)]
    const SO_EPOCH: usize = 1;
    #[cfg(miri)]
    const MOI_EPOCH: usize = 2;
    #[cfg(not(miri))]
    const SO_EPOCH: usize = 40;
    #[cfg(not(miri))]
    const MOI_EPOCH: usize = 250;
    const SLOT_VS: usize = 5;
    // Mỗi epoch một thế hệ 0 (`MOI_EPOCH` = 250 < 256) — op "cạn ASID ⇒ gen+1" có test riêng (257).
    let mut tong = 0usize;
    for epoch in 0..SO_EPOCH {
        let mut r = Rig::new(1, 64 * 1024);
        for i in 0..MOI_EPOCH {
            let (_, vs) = r.vspace_at(SLOT_VS);
            let info = r.m.vspace_info(vs).expect("info");
            // Chuỗi tất định: ASID = i, thế hệ = 0 trong epoch này ⇒ **không** trùng trong thế
            // hệ, kể cả sau khi VSpace trước đã bị huỷ (ASID về hưu cho hết thế hệ).
            assert_eq!(info.asid, i as u16, "epoch {epoch} vòng {i}: asid");
            assert_eq!(info.generation, 0, "epoch {epoch} vòng {i}: thế hệ");
            assert_eq!(info.active_on, HartMask::EMPTY);
            r.m.check_vspace().expect("check_vspace sau tạo");
            r.m.check_asid().expect("check_asid sau tạo");
            // Hart chạy trên VSpace rồi teardown **khi còn active**.
            r.m.vspace_activate(vs, 0).expect("activate");
            assert_eq!(r.m.vspace_active_on(vs), Ok(HartMask::from_bits(0b1)));
            r.m.check_vspace().expect("check_vspace sau activate");
            r.m.revoke(r.c, Some(0)).expect("revoke Untyped VSpace");
            assert_eq!(r.m.vspace_count(), 0);
            assert_eq!(r.m.page_table_count(), 0, "không rò root PageTable");
            r.m.check_vspace().expect("check_vspace sau huỷ");
            r.m.check_asid().expect("check_asid sau huỷ");
            assert_eq!(
                r.m.last_flush().map(|c| (c.scope, c.asid, c.harts)),
                Some((FlushScope::Asid, Some(i as u16), HartMask::from_bits(0b1))),
                "epoch {epoch} vòng {i}: flush theo asid tới hart đang active"
            );
            tong += 1;
        }
        // Cuối epoch: đúng `MOI_EPOCH` ASID đã cấp (về hưu, free-list còn 256 - n), arena sạch.
        assert_eq!(
            r.m.asid_state(),
            AsidState {
                generation: 0,
                free: ASID_COUNT - MOI_EPOCH,
                issued: MOI_EPOCH as u32,
                exhaustions: 0,
            },
            "epoch {epoch}: trạng thái ASID"
        );
        assert_eq!(r.m.flush_count(), MOI_EPOCH as u64);
        assert_eq!(
            r.m.untyped_used(r.uva),
            0,
            "epoch {epoch}: không rò byte arena"
        );
        assert_eq!(r.m.vspace_count(), 0);
        assert_eq!(r.m.page_table_count(), 0);
        r.kiem("cuối epoch");
    }
    assert_eq!(tong, SO_EPOCH * MOI_EPOCH);
    #[cfg(not(miri))]
    assert_eq!(
        tong, 10_000,
        "property: đúng 10.000 thao tác tạo/huỷ VSpace"
    );
}
