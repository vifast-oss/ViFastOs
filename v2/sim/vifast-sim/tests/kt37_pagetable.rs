// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-37 — **PageTable 3 cấp** (root/cấp 1/cấp 0) + `map`/`unmap` + rights/attrs + 7 mã lỗi,
//! model-only `[TK]` (K2-PLAN §3 KT-37, D2-65 Q2/Q3/Q7, `v2/docs/k2-vm.md` §11, thiết kế
//! §4.10 `:353-366`).
//!
//! **KT-37 KHÔNG có VSpace/ASID/TLB** (KT-38/40), **không** áp luật W^X (KT-39) và **không**
//! có VM fault (KT-41). Ở đây chỉ có cây PageTable **theo mô hình**:
//! 1. `PageTable` là object (retype từ Untyped, 4 KiB) và cây 3 cấp dùng `vifast-pte`
//!    (`walk_shape` 9/9/9/12, `MAX_MAP_DEPTH`).
//! 2. Lá chọn theo **cỡ Frame**: 4 KiB ở cấp 0, 2 MiB ở cấp 1, 1 GiB ở cấp 2. Bảng cấp trên
//!    phải **đã map** (attach) trước, thiếu ⇒ `MissingPageTable`.
//! 3. `map` lưu **rights/attrs của từng mapping** (không suy từ cap) và cộng
//!    `maps/mapped_w/mapped_x` của Frame qua `frame_note_map`.
//! 4. `unmap`/walk ngược VA→PA khớp 100%; `check_pages` kiểm cây + bộ đếm sau **mỗi** op.
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt37-red-compile.log`](../../agent/JOURNAL/kt37-red-compile.log)
//! — API chưa có ⇒ E0432/E0599; [`JOURNAL/kt37-red-semantics.log`](../../agent/JOURNAL/kt37-red-semantics.log)
//! — `Retype(Untyped, PageTable)` còn `NotSupported` (reserved), map/unmap chưa có.

use std::collections::BTreeMap;

use vifast_sim::kobj::{
    CapId, KError, Machine, MapAttrs, MapRights, Mapping, ObjId, ObjType, SlotRef,
};
use vifast_sim::Lcg;

const K4: u64 = 1 << 12;
const M2: u64 = 1 << 21;
const G1: u64 = 1 << 30;
const VA_LIMIT: u64 = 1 << 39;

// ===========================================================================
// Rig: CNode 128 slot + Untyped cho PageTable (64 KiB) + Untyped cho Frame (4 KiB, cửa sổ PA
// 1 GiB ở base 4 GiB). Frame **không** tiêu byte arena (KT-36) nên 4 KiB là đủ.
// ===========================================================================

struct Rig {
    m: Machine,
    c: ObjId,
    upt: ObjId,
    ufr: ObjId,
    ufr1: ObjId,
    spt: SlotRef,
    sfr: SlotRef,
    sfr1: SlotRef,
    next: usize,
}

impl Rig {
    fn new() -> Self {
        let mut m = Machine::new();
        let c = m.add_cnode(128);
        let upt = m.add_untyped(0, 256 * 1024);
        let ufr = m.add_untyped(1 << 32, 4096);
        // Cửa sổ PA 1 GiB chỉ chứa **một** Frame 1 GiB ⇒ Untyped riêng cho cỡ 30.
        let ufr1 = m.add_untyped(1 << 33, 4096);
        let cpt = m.root_cap(upt);
        m.put_cap(c, 0, cpt).expect("cap Untyped PageTable");
        let cfr = m.root_cap(ufr);
        m.put_cap(c, 1, cfr).expect("cap Untyped Frame");
        let cfr1 = m.root_cap(ufr1);
        m.put_cap(c, 2, cfr1).expect("cap Untyped Frame 1G");
        Self {
            m,
            c,
            upt,
            ufr,
            ufr1,
            spt: SlotRef::new(c, 0),
            sfr: SlotRef::new(c, 1),
            sfr1: SlotRef::new(c, 2),
            next: 3,
        }
    }

    fn slot(&mut self) -> usize {
        let s = self.next;
        self.next += 1;
        s
    }

    fn put(&mut self, cap: CapId) -> SlotRef {
        let s = self.slot();
        self.m.put_cap(self.c, s, cap).expect("đặt cap vào CNode");
        SlotRef::new(self.c, s)
    }

    /// Retype một PageTable + đặt cap; trả `(slot, obj)`.
    fn tbl(&mut self) -> (SlotRef, ObjId) {
        let cap = self
            .m
            .retype(self.spt, ObjType::PageTable, 0, 0)
            .expect("retype PageTable");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Retype một Frame cỡ `size_bits` + đặt cap; trả `(slot, obj)`.
    fn frame(&mut self, size_bits: u32) -> (SlotRef, ObjId) {
        let src = if size_bits == 30 { self.sfr1 } else { self.sfr };
        let cap = self
            .m
            .retype(src, ObjType::Frame, size_bits, 0)
            .expect("retype Frame");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Bất biến sau mỗi op: PageTable tree + Frame + cap + byte.
    fn kiem(&self, nhan: &str) {
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
            self.m.untyped_used(self.ufr1),
            0,
            "{nhan}: Frame 1 GiB không tiêu byte arena"
        );
    }
}

// ---------------------------------------------------------------------------
// Dựng cây
// ---------------------------------------------------------------------------

/// Cây 4 KiB: root → mid (cấp 1, attach tại `base` 1 GiB-căn) → leaf (cấp 0, attach tại `base`
/// 2 MiB-căn). Trả `(root, mid, leaf)`.
fn cay_4k(r: &mut Rig, base: u64) -> (ObjId, ObjId, ObjId) {
    let (_, root) = r.tbl();
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, base, mid)
        .expect("attach mid vào root");
    let (_, leaf) = r.tbl();
    r.m.page_table_attach(mid, base, leaf)
        .expect("attach leaf vào mid");
    (root, mid, leaf)
}

/// Cây 2 MiB: root → mid attach tại `base` 1 GiB-căn.
fn cay_2m(r: &mut Rig, base: u64) -> (ObjId, ObjId) {
    let (_, root) = r.tbl();
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, base, mid)
        .expect("attach mid vào root");
    (root, mid)
}

fn map4(r: &mut Rig, root: ObjId, va: u64, f: SlotRef) -> Mapping {
    r.m.page_table_map(root, va, f, MapRights::READ, MapAttrs::WB)
        .expect("map 4 KiB")
}

// ===========================================================================
// 1. Retype PageTable: 4 KiB, size_bits 0/12, cỡ lạ ⇒ InvalidArgument, atomic.
// ===========================================================================

#[test]
fn retype_page_table_size_bits_va_byte() {
    let mut r = Rig::new();
    assert_eq!(r.m.page_table_count(), 0);
    assert_eq!(r.m.untyped_used(r.upt), 0);

    for bits in [1u32, 11, 13, 21, 30, 31, 64, 1000] {
        assert_eq!(
            r.m.retype(r.spt, ObjType::PageTable, bits, 0),
            Err(KError::InvalidArgument),
            "PageTable size_bits={bits} phải bị từ chối"
        );
        assert_eq!(r.m.page_table_count(), 0, "từ chối không được tạo object");
        assert_eq!(r.m.untyped_used(r.upt), 0, "từ chối không đốt byte arena");
    }

    // size_bits = 0 và 12 (4096 B) đều hợp lệ; mỗi bảng tiêu đúng 4096 B arena.
    let (_, a) = r.tbl();
    assert_eq!(r.m.page_table_count(), 1);
    assert_eq!(r.m.untyped_used(r.upt), K4 as usize);
    let cap =
        r.m.retype(r.spt, ObjType::PageTable, 12, 0)
            .expect("4096 B");
    let b = r.m.cap_info(cap).expect("cap").obj;
    r.put(cap);
    assert_eq!(r.m.page_table_count(), 2);
    assert_eq!(r.m.untyped_used(r.upt), 2 * K4 as usize);
    r.kiem("sau retype");

    // Bảng mới: level 2 (root), chưa attach, rỗng.
    for pt in [a, b] {
        let info = r.m.page_table_info(pt).expect("info");
        assert_eq!(info.level, 2);
        assert_eq!(info.attached, None);
        assert_eq!(info.entries, 0);
        assert_eq!(info.mapped, 0);
    }

    // Cap không phải Untyped ⇒ WrongObjectType (không tạo object).
    let (fs, _) = r.frame(12);
    assert_eq!(
        r.m.retype(fs, ObjType::PageTable, 0, 0),
        Err(KError::WrongObjectType)
    );
    assert_eq!(r.m.page_table_count(), 2);
    r.kiem("sau cap sai");
}

// ===========================================================================
// 2. Map/walk/unmap đủ 3 cỡ: lá đúng cấp, pa đúng, rights/attrs giữ nguyên.
// ===========================================================================

#[test]
fn map_walk_unmap_du_ba_co() {
    // --- 4 KiB: lá cấp 0 ---
    let mut r = Rig::new();
    let (root, _mid, leaf) = cay_4k(&mut r, 0);
    let (f4, o4) = r.frame(12);
    let pa4 = r.m.frame_address(o4).expect("pa 4K");
    let m4 = map4(&mut r, root, 0x1000, f4);
    assert_eq!(m4.va, 0x1000);
    assert_eq!(m4.pa, pa4);
    assert_eq!(m4.size_bits, 12);
    assert_eq!(m4.rights, MapRights::READ);
    assert_eq!(m4.attrs, MapAttrs::WB);
    // Walk tại base và giữa trang đều trả cùng mapping (lá 4 KiB).
    assert_eq!(r.m.page_table_walk(root, 0x1000).expect("walk base"), m4);
    assert_eq!(r.m.page_table_walk(root, 0x1abc).expect("walk trong"), m4);
    let info = r.m.page_table_info(leaf).expect("info leaf");
    assert_eq!(info.level, 0);
    assert_eq!(info.attached, Some((_mid, 0)));
    assert_eq!(info.entries, 1);
    assert_eq!(info.mapped, 1);
    r.kiem("4K map");
    let um = r.m.page_table_unmap(root, 0x1000).expect("unmap 4K");
    assert_eq!(um, m4);
    assert_eq!(r.m.page_table_walk(root, 0x1000), Err(KError::NotMapped));
    assert_eq!(r.m.frame_meta(o4).expect("meta").maps, 0);
    r.kiem("4K unmap");

    // --- 2 MiB: lá cấp 1 ---
    let mut r = Rig::new();
    let (root, mid) = cay_2m(&mut r, 0);
    let (f2, o2) = r.frame(21);
    let pa2 = r.m.frame_address(o2).expect("pa 2M");
    let m2 =
        r.m.page_table_map(
            root,
            0x200000,
            f2,
            MapRights::READ | MapRights::WRITE,
            MapAttrs::WC,
        )
        .expect("map 2 MiB");
    assert_eq!(m2.va, 0x200000);
    assert_eq!(m2.pa, pa2);
    assert_eq!(m2.size_bits, 21);
    assert_eq!(r.m.page_table_walk(root, 0x200000).expect("walk 2M"), m2);
    // Địa chỉ bên trong trang lớn vẫn trả mapping 2 MiB.
    assert_eq!(
        r.m.page_table_walk(root, 0x200000 + 0x1234)
            .expect("walk trong 2M"),
        m2
    );
    let info = r.m.page_table_info(mid).expect("info mid");
    assert_eq!(info.level, 1);
    assert_eq!(info.entries, 1);
    assert_eq!(info.mapped, 1);
    let meta = r.m.frame_meta(o2).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 1, 0));
    r.kiem("2M map");
    assert_eq!(r.m.page_table_unmap(root, 0x200000).expect("unmap 2M"), m2);
    r.kiem("2M unmap");

    // --- 1 GiB: lá ở root (cấp 2) ---
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (f1, o1) = r.frame(30);
    let pa1 = r.m.frame_address(o1).expect("pa 1G");
    let m1 =
        r.m.page_table_map(root, G1, f1, MapRights::READ, MapAttrs::WB | MapAttrs::EXEC)
            .expect("map 1 GiB");
    assert_eq!(m1.va, G1);
    assert_eq!(m1.pa, pa1);
    assert_eq!(m1.size_bits, 30);
    assert_eq!(r.m.page_table_walk(root, G1 + 0x3000).expect("walk 1G"), m1);
    let info = r.m.page_table_info(root).expect("info root");
    assert_eq!(info.level, 2);
    assert_eq!(info.entries, 1);
    assert_eq!(info.mapped, 1);
    let meta = r.m.frame_meta(o1).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 0, 1));
    r.kiem("1G map");
    assert_eq!(r.m.page_table_unmap(root, G1).expect("unmap 1G"), m1);
    r.kiem("1G unmap");
}

// ===========================================================================
// 3. Bảng chân trị 7 mã lỗi × 3 cỡ (mỗi ca kiểm atomic: lỗi không đổi cây/bộ đếm).
// ===========================================================================

#[test]
fn bay_ma_loi_nhan_ba_co() {
    // ---------------------------------------------------------------- MissingPageTable
    // 4 KiB: thiếu mid, rồi thiếu leaf (có mid).
    let mut r = Rig::new();
    let (f4, o4) = r.frame(12);
    let (_, root) = r.tbl();
    assert_eq!(
        r.m.page_table_map(root, 0, f4, MapRights::READ, MapAttrs::WB),
        Err(KError::MissingPageTable),
        "4K: root không có mid"
    );
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, 0, mid).expect("attach mid");
    assert_eq!(
        r.m.page_table_map(root, 0, f4, MapRights::READ, MapAttrs::WB),
        Err(KError::MissingPageTable),
        "4K: mid không có leaf"
    );
    assert_eq!(r.m.frame_meta(o4).expect("meta").maps, 0, "lỗi atomic");
    r.kiem("4K missing");

    // 2 MiB: thiếu mid.
    let mut r = Rig::new();
    let (f2, o2) = r.frame(21);
    let (_, root) = r.tbl();
    assert_eq!(
        r.m.page_table_map(root, 0, f2, MapRights::READ, MapAttrs::WB),
        Err(KError::MissingPageTable),
        "2M: root không có mid"
    );
    assert_eq!(r.m.frame_meta(o2).expect("meta").maps, 0);
    r.kiem("2M missing");

    // 1 GiB: truyền **bảng cấp 1** làm root ⇒ thiếu bảng gốc cấp 2.
    let mut r = Rig::new();
    let (f1, o1) = r.frame(30);
    let (root, mid) = cay_2m(&mut r, 0);
    assert_eq!(
        r.m.page_table_map(mid, 0, f1, MapRights::READ, MapAttrs::WB),
        Err(KError::MissingPageTable),
        "1G: map trên bảng cấp 1 (không phải gốc)"
    );
    assert_eq!(r.m.frame_meta(o1).expect("meta").maps, 0);
    r.kiem("1G missing");
    let _ = root;

    // ---------------------------------------------------------------- AlreadyMapped
    // 4 KiB: trùng đúng VA.
    let mut r = Rig::new();
    let (root, _m, _l) = cay_4k(&mut r, 0);
    let (f4a, o4a) = r.frame(12);
    let (f4b, _o4b) = r.frame(12);
    map4(&mut r, root, 0x2000, f4a);
    assert_eq!(
        r.m.page_table_map(root, 0x2000, f4b, MapRights::READ, MapAttrs::WB),
        Err(KError::AlreadyMapped)
    );
    assert_eq!(r.m.frame_meta(o4a).expect("meta").maps, 1, "không cộng đôi");
    r.kiem("4K already");

    // 2 MiB: trùng VA và 4 KiB nằm trong trang 2 MiB đã map.
    let mut r = Rig::new();
    let (root, _mid) = cay_2m(&mut r, 0);
    let (f2, _o2) = r.frame(21);
    let (f2b, _o2b) = r.frame(21);
    r.m.page_table_map(root, 0, f2, MapRights::READ, MapAttrs::WB)
        .expect("map 2M");
    assert_eq!(
        r.m.page_table_map(root, 0, f2b, MapRights::READ, MapAttrs::WB),
        Err(KError::AlreadyMapped)
    );
    let (f4, _o4) = r.frame(12);
    assert_eq!(
        r.m.page_table_map(root, 0x1000, f4, MapRights::READ, MapAttrs::WB),
        Err(KError::AlreadyMapped),
        "4K trong trang 2M đã map"
    );
    r.kiem("2M already");

    // 1 GiB: trùng VA, 2 MiB và 4 KiB nằm trong trang 1 GiB đã map.
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (f1, _o1) = r.frame(30);
    r.m.page_table_map(root, 0, f1, MapRights::READ, MapAttrs::WB)
        .expect("map 1G");
    assert_eq!(
        r.m.page_table_map(root, 0, f1, MapRights::READ, MapAttrs::WB),
        Err(KError::AlreadyMapped),
        "map lại cùng Frame tại cùng VA"
    );
    let (f2, _) = r.frame(21);
    assert_eq!(
        r.m.page_table_map(root, 0x200000, f2, MapRights::READ, MapAttrs::WB),
        Err(KError::AlreadyMapped),
        "2M trong trang 1G đã map"
    );
    let (f4, _) = r.frame(12);
    assert_eq!(
        r.m.page_table_map(root, 0x1000, f4, MapRights::READ, MapAttrs::WB),
        Err(KError::AlreadyMapped),
        "4K trong trang 1G đã map"
    );
    r.kiem("1G already");

    // ---------------------------------------------------------------- NotMapped
    // 4 KiB.
    let mut r = Rig::new();
    let (root, _m, _l) = cay_4k(&mut r, 0);
    assert_eq!(r.m.page_table_unmap(root, 0x1000), Err(KError::NotMapped));
    // 2 MiB.
    let mut r = Rig::new();
    let (root, _mid) = cay_2m(&mut r, 0);
    assert_eq!(r.m.page_table_unmap(root, 0), Err(KError::NotMapped));
    // 1 GiB.
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    assert_eq!(r.m.page_table_unmap(root, G1), Err(KError::NotMapped));
    r.kiem("NotMapped");

    // ---------------------------------------------------------------- Alignment
    // 4 KiB: lệch 1.
    let mut r = Rig::new();
    let (root, _m, _l) = cay_4k(&mut r, 0);
    let (f4, o4) = r.frame(12);
    assert_eq!(
        r.m.page_table_map(root, 0x1001, f4, MapRights::READ, MapAttrs::WB),
        Err(KError::Alignment)
    );
    assert_eq!(r.m.frame_meta(o4).expect("meta").maps, 0);
    r.kiem("4K alignment");
    // 2 MiB: lệch 1.
    let mut r = Rig::new();
    let (root, _mid) = cay_2m(&mut r, 0);
    let (f2, _o2) = r.frame(21);
    assert_eq!(
        r.m.page_table_map(root, 1, f2, MapRights::READ, MapAttrs::WB),
        Err(KError::Alignment)
    );
    r.kiem("2M alignment");
    // 1 GiB: lệch 1.
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (f1, _o1) = r.frame(30);
    assert_eq!(
        r.m.page_table_map(root, 1, f1, MapRights::READ, MapAttrs::WB),
        Err(KError::Alignment)
    );
    r.kiem("1G alignment");

    // ---------------------------------------------------------------- RangeError
    for bits in [12u32, 21, 30] {
        let mut r = Rig::new();
        let (fs, _o) = r.frame(bits);
        let (_, root) = r.tbl();
        assert_eq!(
            r.m.page_table_map(root, VA_LIMIT, fs, MapRights::READ, MapAttrs::WB),
            Err(KError::RangeError),
            "size_bits={bits}: va ngoài VA_BITS"
        );
        assert_eq!(
            r.m.page_table_map(root, u64::MAX, fs, MapRights::READ, MapAttrs::WB),
            Err(KError::RangeError)
        );
        assert_eq!(
            r.m.page_table_unmap(root, VA_LIMIT),
            Err(KError::RangeError)
        );
        r.kiem("range");
    }

    // ---------------------------------------------------------------- HasChildren
    // 4 KiB: leaf table còn mapping ⇒ detach HasChildren; revoke Untyped bảng ⇒ HasChildren.
    let mut r = Rig::new();
    let (root, mid, leaf) = cay_4k(&mut r, 0);
    let (f4, o4) = r.frame(12);
    let m4 = map4(&mut r, root, 0x1000, f4);
    assert_eq!(
        r.m.page_table_detach(mid, 0),
        Err(KError::HasChildren),
        "detach leaf còn mapping"
    );
    assert_eq!(
        r.m.revoke(r.c, Some(0)),
        Err(KError::HasChildren),
        "revoke Untyped bảng khi còn mapping"
    );
    assert_eq!(r.m.page_table_walk(root, 0x1000).expect("vẫn còn"), m4);
    assert_eq!(r.m.frame_meta(o4).expect("meta").maps, 1, "atomic");
    assert_eq!(r.m.page_table_info(leaf).expect("info").mapped, 1);
    r.kiem("4K HasChildren");
    // Gỡ mapping rồi detach/revoke mới được.
    r.m.page_table_unmap(root, 0x1000).expect("unmap");
    r.m.page_table_detach(mid, 0).expect("detach leaf");
    r.m.page_table_detach(root, 0).expect("detach mid");
    r.m.revoke(r.c, Some(0)).expect("revoke Untyped bảng");
    assert_eq!(r.m.page_table_count(), 0);
    r.kiem("4K revoke sạch");

    // 2 MiB: mid còn mapping ⇒ revoke HasChildren.
    let mut r = Rig::new();
    let (root, _mid) = cay_2m(&mut r, 0);
    let (f2, o2) = r.frame(21);
    let m2 =
        r.m.page_table_map(root, 0, f2, MapRights::READ, MapAttrs::WB)
            .expect("map 2M");
    assert_eq!(r.m.revoke(r.c, Some(0)), Err(KError::HasChildren));
    assert_eq!(r.m.page_table_walk(root, 0).expect("vẫn còn"), m2);
    assert_eq!(r.m.frame_meta(o2).expect("meta").maps, 1);
    r.kiem("2M HasChildren");

    // 1 GiB: root còn mapping ⇒ revoke HasChildren.
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (f1, o1) = r.frame(30);
    let m1 =
        r.m.page_table_map(root, 0, f1, MapRights::READ, MapAttrs::WB)
            .expect("map 1G");
    assert_eq!(r.m.revoke(r.c, Some(0)), Err(KError::HasChildren));
    assert_eq!(r.m.page_table_walk(root, 0).expect("vẫn còn"), m1);
    assert_eq!(r.m.frame_meta(o1).expect("meta").maps, 1);
    r.kiem("1G HasChildren");
}

// ===========================================================================
// 4. Attach/detach: cấp trên phải tồn tại, cấp cháu không cắt ngang, detach đúng.
// ===========================================================================

#[test]
fn attach_detach_va_has_children() {
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, 0, mid).expect("attach mid");
    let info = r.m.page_table_info(mid).expect("info");
    assert_eq!(info.level, 1);
    assert_eq!(info.attached, Some((root, 0)));

    // Attach lại chính mid ⇒ AlreadyMapped (bảng đã nằm trong cây).
    assert_eq!(
        r.m.page_table_attach(root, 0, mid),
        Err(KError::AlreadyMapped)
    );
    // Detach cũng đòi `va` đúng gốc vùng con phủ (1 GiB ở cấp này) ⇒ Alignment.
    assert_eq!(r.m.page_table_detach(root, 0x1000), Err(KError::Alignment));
    assert_eq!(
        r.m.page_table_detach(root, VA_LIMIT),
        Err(KError::RangeError)
    );
    // Cháu lệch căn 2 MiB ⇒ Alignment.
    let (_, leaf) = r.tbl();
    assert_eq!(
        r.m.page_table_attach(mid, 0x1000, leaf),
        Err(KError::Alignment)
    );
    // Ngoài VA_BITS ⇒ RangeError.
    assert_eq!(
        r.m.page_table_attach(mid, VA_LIMIT, leaf),
        Err(KError::RangeError)
    );
    // Đích không có cấp dưới (bảng lá cấp 0) ⇒ InvalidArgument.
    r.m.page_table_attach(mid, 0, leaf).expect("attach leaf");
    // Ô đã có bảng con ⇒ AlreadyMapped.
    let (_, leaf2) = r.tbl();
    assert_eq!(
        r.m.page_table_attach(mid, 0, leaf2),
        Err(KError::AlreadyMapped)
    );
    let (_, x) = r.tbl();
    assert_eq!(
        r.m.page_table_attach(leaf, 0, x),
        Err(KError::InvalidArgument)
    );
    // Tự attach chính nó ⇒ InvalidArgument.
    let (_, y) = r.tbl();
    assert_eq!(r.m.page_table_attach(y, 0, y), Err(KError::InvalidArgument));
    // Bảng **đã có entry** không được làm con (dựng cây 4 KiB ở VA 0 của root2).
    let (_, root2) = r.tbl();
    let (_, mid2) = r.tbl();
    r.m.page_table_attach(root2, 0, mid2).expect("attach mid2");
    let (_, leaf2) = r.tbl();
    r.m.page_table_attach(mid2, 0, leaf2).expect("attach leaf2");
    let (f4b, _) = r.frame(12);
    r.m.page_table_map(root2, 0x1000, f4b, MapRights::READ, MapAttrs::WB)
        .expect("map 4K vào root2");
    let (_, root3) = r.tbl();
    assert_eq!(
        r.m.page_table_attach(root3, 0, root2),
        Err(KError::InvalidArgument),
        "bảng đã có entry không được attach"
    );
    r.kiem("attach errors");

    // Detach bảng rỗng: leaf trả về làm root, mid detach được.
    let got = r.m.page_table_detach(mid, 0).expect("detach leaf");
    assert_eq!(got, leaf);
    let info = r.m.page_table_info(leaf).expect("info sau detach");
    assert_eq!(info.level, 2);
    assert_eq!(info.attached, None);
    // Detach lần hai ⇒ NotMapped.
    assert_eq!(r.m.page_table_detach(mid, 0), Err(KError::NotMapped));
    // Detach ô chứa **lá** (không phải bảng) ⇒ NotMapped: lá 1 GiB nằm ngay ở root.
    let (_, root_1g) = r.tbl();
    let (f1, _) = r.frame(30);
    r.m.page_table_map(root_1g, 0, f1, MapRights::READ, MapAttrs::WB)
        .expect("map 1G");
    assert_eq!(r.m.page_table_detach(root_1g, 0), Err(KError::NotMapped));
    // Detach bảng lá còn con ⇒ HasChildren, atomic.
    let (root4, mid4, _l) = cay_4k(&mut r, G1);
    let (f4, _) = r.frame(12);
    map4(&mut r, root4, G1 + 0x1000, f4);
    let info_before = r.m.page_table_info(mid4).expect("info");
    assert_eq!(r.m.page_table_detach(mid4, G1), Err(KError::HasChildren));
    let info_after = r.m.page_table_info(mid4).expect("info");
    assert_eq!(info_before.entries, info_after.entries);
    assert_eq!(info_before.mapped, info_after.mapped);
    r.kiem("detach");
}

// ===========================================================================
// 5. Rights/attrs lưu theo **từng mapping**, không suy từ cap; chia sẻ Frame 2 root.
// ===========================================================================

#[test]
fn rights_attrs_theo_mapping_va_chia_se_frame() {
    let mut r = Rig::new();
    let (root_a, _ma, _la) = cay_4k(&mut r, 0);
    let (root_b, _mb, _lb) = cay_4k(&mut r, G1);
    let (fa, oa) = r.frame(12);
    // Cap có RIGHTS_ALL — mapping chỉ xin WRITE + attrs UC|EXEC ⇒ phải giữ đúng yêu cầu.
    let ma =
        r.m.page_table_map(
            root_a,
            0x1000,
            fa,
            MapRights::WRITE,
            MapAttrs::UC | MapAttrs::EXEC,
        )
        .expect("map WRITE/UC/EXEC");
    assert_eq!(ma.rights, MapRights::WRITE);
    assert_eq!(ma.attrs, MapAttrs::UC | MapAttrs::EXEC);
    let meta = r.m.frame_meta(oa).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 1, 1));

    // Cùng Frame map root B với READ, toàn bộ attrs: bộ đếm **toàn cục** cộng đúng.
    let mb =
        r.m.page_table_map(
            root_b,
            G1 + 0x1000,
            fa,
            MapRights::READ,
            MapAttrs::WB
                | MapAttrs::WC
                | MapAttrs::UC
                | MapAttrs::DEVICE
                | MapAttrs::NX
                | MapAttrs::EXEC,
        )
        .expect("map READ/all-attrs");
    let meta = r.m.frame_meta(oa).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (2, 1, 2));
    assert_eq!(r.m.page_table_walk(root_a, 0x1000).expect("walk A"), ma);
    assert_eq!(
        r.m.page_table_walk(root_b, G1 + 0x1000).expect("walk B"),
        mb
    );
    assert_ne!(ma.rights, mb.rights);
    assert_ne!(ma.attrs, mb.attrs);
    r.kiem("shared frame");

    // Unmap một root: bộ đếm còn đúng phần còn lại, root kia không đổi.
    r.m.page_table_unmap(root_a, 0x1000).expect("unmap A");
    let meta = r.m.frame_meta(oa).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 0, 1));
    assert_eq!(
        r.m.page_table_walk(root_b, G1 + 0x1000).expect("walk B"),
        mb
    );
    r.m.page_table_unmap(root_b, G1 + 0x1000).expect("unmap B");
    assert_eq!(r.m.frame_meta(oa).expect("meta").maps, 0);
    r.kiem("shared frame xong");
}

// ===========================================================================
// 6. Unmap đúng **gốc** mapping; địa chỉ bên trong trang lớn ⇒ Alignment (không xoá ngầm).
// ===========================================================================

#[test]
fn unmap_dung_goc_va_alignment() {
    // 4 KiB.
    let mut r = Rig::new();
    let (root, _m, _l) = cay_4k(&mut r, 0);
    let (f4, _o4) = r.frame(12);
    let m4 = map4(&mut r, root, 0x1000, f4);
    assert_eq!(
        r.m.page_table_unmap(root, 0x1001),
        Err(KError::Alignment),
        "unmap lệch base 4K"
    );
    assert_eq!(r.m.page_table_walk(root, 0x1000).expect("vẫn còn"), m4);
    r.m.page_table_unmap(root, 0x1000).expect("unmap đúng base");
    assert_eq!(r.m.page_table_unmap(root, 0x1000), Err(KError::NotMapped));
    r.kiem("unmap 4K");

    // 2 MiB.
    let mut r = Rig::new();
    let (root, _mid) = cay_2m(&mut r, 0);
    let (f2, _o2) = r.frame(21);
    let m2 =
        r.m.page_table_map(root, 0, f2, MapRights::READ, MapAttrs::WB)
            .expect("map 2M");
    assert_eq!(
        r.m.page_table_unmap(root, 0x1000),
        Err(KError::Alignment),
        "unmap giữa trang 2M"
    );
    assert_eq!(r.m.page_table_walk(root, 0).expect("vẫn còn"), m2);
    assert_eq!(r.m.page_table_unmap(root, M2), Err(KError::NotMapped));
    r.m.page_table_unmap(root, 0).expect("unmap đúng base 2M");
    r.kiem("unmap 2M");

    // 1 GiB.
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (f1, _o1) = r.frame(30);
    let m1 =
        r.m.page_table_map(root, G1, f1, MapRights::READ, MapAttrs::WB)
            .expect("map 1G");
    assert_eq!(
        r.m.page_table_unmap(root, G1 + 0x1000),
        Err(KError::Alignment),
        "unmap giữa trang 1G"
    );
    assert_eq!(r.m.page_table_walk(root, G1).expect("vẫn còn"), m1);
    r.m.page_table_unmap(root, G1).expect("unmap đúng base 1G");
    r.kiem("unmap 1G");
}

// ===========================================================================
// 7. Property 10.000 op: map/unmap ngẫu nhiên, walk ngược VA→PA khớp 100% sau mỗi op.
// ===========================================================================

#[test]
fn property_10k_op_walk_nguoc_khop_100() {
    #[cfg(miri)]
    const N_OPS: usize = 20;
    #[cfg(not(miri))]
    const N_OPS: usize = 10_000;

    let mut r = Rig::new();
    // Cây: root → M1 (VA 0) + M2 (VA 1G); M1 → L0 (0), L1 (2M); M2 → L2 (1G+4M), L3 (1G+6M).
    let (_, root) = r.tbl();
    let (_, m1) = r.tbl();
    r.m.page_table_attach(root, 0, m1).expect("attach M1");
    let (_, m2) = r.tbl();
    r.m.page_table_attach(root, G1, m2).expect("attach M2");
    for (parent, va) in [(m1, 0), (m1, 2 * M2), (m2, G1 + 4 * M2), (m2, G1 + 6 * M2)] {
        let (_, leaf) = r.tbl();
        r.m.page_table_attach(parent, va, leaf)
            .expect("attach leaf table");
    }

    // Cấp Frame theo thứ tự **giảm dần** trong cùng cửa sổ PA (KT-36: không căn lề ngầm):
    // 2 MiB trước, 4 KiB sau; 1 GiB ở Untyped riêng (một cửa sổ chỉ chứa một Frame 1 GiB).
    let mut f2 = Vec::new();
    for _ in 0..4 {
        f2.push(r.frame(21));
    }
    let mut f4 = Vec::new();
    for _ in 0..10 {
        f4.push(r.frame(12));
    }
    let f1 = [r.frame(30)];
    r.kiem("dựng cây");

    // Slot theo cỡ (mỗi slot là base mapping; không chồng nhau giữa các cỡ).
    let slots4: [u64; 8] = [
        0x1000,
        0x2000,
        2 * M2 + 0x1000,
        2 * M2 + 0x2000,
        G1 + 4 * M2 + 0x1000,
        G1 + 4 * M2 + 0x2000,
        G1 + 6 * M2 + 0x1000,
        G1 + 6 * M2 + 0x2000,
    ];
    let slots2: [u64; 4] = [4 * M2, 6 * M2, G1, G1 + 2 * M2];
    let slots1: [u64; 2] = [2 * G1, 3 * G1];

    let mut oracle: BTreeMap<u64, Mapping> = BTreeMap::new();
    // Sổ phụ để kiểm bộ đếm Frame **toàn cục** theo mapping sống.
    let mut dem: BTreeMap<ObjId, (u32, u32, u32)> = BTreeMap::new();
    let mut rng = Lcg::new(37);
    let mut thong_ke = [0u32; 6]; // map ok / map err / unmap ok / unmap err / walk spot ok / walk full

    for step in 0..N_OPS {
        let do_map = rng.below(2) == 0;
        let co = rng.below(10);
        let (bits, slots): (u32, &[u64]) = if co < 6 {
            (12, &slots4)
        } else if co < 9 {
            (21, &slots2)
        } else {
            (30, &slots1)
        };
        let va = slots[rng.below(slots.len() as u64) as usize];
        let size = 1u64 << bits;

        if do_map {
            let (fs, obj) = match bits {
                12 => f4[rng.below(f4.len() as u64) as usize],
                21 => f2[rng.below(f2.len() as u64) as usize],
                _ => f1[rng.below(f1.len() as u64) as usize],
            };
            let rights = match rng.below(3) {
                0 => MapRights::READ,
                1 => MapRights::READ | MapRights::WRITE,
                _ => MapRights::WRITE,
            };
            let attrs = MapAttrs::WB
                | if rng.below(2) == 0 {
                    MapAttrs::NX
                } else {
                    MapAttrs::EXEC
                }
                | if rng.below(2) == 0 {
                    MapAttrs::WC
                } else {
                    MapAttrs::UC
                };
            let chong = oracle
                .values()
                .any(|m| m.va < va + size && va < m.va + (1u64 << m.size_bits));
            let got = r.m.page_table_map(root, va, fs, rights, attrs);
            if chong {
                assert_eq!(
                    got,
                    Err(KError::AlreadyMapped),
                    "step {step}: chồng mapping phải AlreadyMapped"
                );
                thong_ke[1] += 1;
            } else {
                let m = got.unwrap_or_else(|e| panic!("step {step}: map lỗi {e:?}"));
                assert_eq!(m.va, va);
                assert_eq!(m.size_bits, bits as u8);
                assert_eq!(m.frame, obj);
                assert_eq!(m.pa, r.m.frame_address(obj).expect("pa"));
                assert_eq!(m.rights, rights);
                assert_eq!(m.attrs, attrs);
                assert!(oracle.insert(va, m).is_none());
                let d = dem.entry(obj).or_insert((0, 0, 0));
                d.0 += 1;
                d.1 += u32::from(rights.contains(MapRights::WRITE));
                d.2 += u32::from(attrs.contains(MapAttrs::EXEC));
                thong_ke[0] += 1;
            }
        } else {
            // Kỳ vọng: mapping phủ `va` (nếu có). Chỉ unmap được khi `va` đúng **gốc**.
            let phu = oracle
                .values()
                .find(|m| m.va <= va && va < m.va + (1u64 << m.size_bits))
                .copied();
            let got = r.m.page_table_unmap(root, va);
            match phu {
                None => {
                    assert_eq!(got, Err(KError::NotMapped), "step {step}: chưa map");
                    thong_ke[3] += 1;
                }
                Some(m) if m.va != va => {
                    assert_eq!(
                        got,
                        Err(KError::Alignment),
                        "step {step}: unmap giữa trang lớn phải Alignment"
                    );
                    thong_ke[3] += 1;
                }
                Some(m) => {
                    assert_eq!(got, Ok(m), "step {step}: unmap trả mapping đúng");
                    assert!(oracle.remove(&va).is_some());
                    let d = dem.get_mut(&m.frame).expect("đếm");
                    d.0 -= 1;
                    d.1 -= u32::from(m.rights.contains(MapRights::WRITE));
                    d.2 -= u32::from(m.attrs.contains(MapAttrs::EXEC));
                    thong_ke[2] += 1;
                }
            }
        }

        // Bất biến cây/bộ đếm **sau mỗi op** (check_pages tự chạy trong sched_after_op; gọi
        // tay thêm để chứng minh checker chạy được ở mọi trạng thái).
        r.kiem(&format!("step {step}"));
        assert_eq!(
            r.m.page_table_info(root).expect("info root").mapped as usize,
            oracle.len(),
            "step {step}: số lá phải bằng oracle"
        );
        // Bộ đếm Frame toàn cục khớp sổ phụ.
        for &(_, obj) in f4.iter().chain(f2.iter()).chain(f1.iter()) {
            let meta = r.m.frame_meta(obj).expect("meta");
            let (maps, w, x) = dem.get(&obj).copied().unwrap_or((0, 0, 0));
            assert_eq!(
                (meta.maps, meta.mapped_w, meta.mapped_x),
                (maps, w, x),
                "step {step}: bộ đếm Frame {obj} lệch"
            );
        }

        // Spot-check walk ngược trên một mapping ngẫu nhiên (base + giữa trang).
        if !oracle.is_empty() {
            let i = rng.below(oracle.len() as u64) as usize;
            let m = *oracle.values().nth(i).expect("mapping");
            assert_eq!(r.m.page_table_walk(root, m.va).expect("walk base"), m);
            let giua = m.va + (1u64 << m.size_bits) / 2;
            assert_eq!(r.m.page_table_walk(root, giua).expect("walk giữa"), m);
            thong_ke[4] += 1;
        }
    }

    // Sau 10.000 op: kiểm **toàn bộ** mapping sống bằng walk ngược, cả base và byte cuối.
    for m in oracle.values() {
        assert_eq!(r.m.page_table_walk(root, m.va).expect("walk base"), *m);
        let cuoi = m.va + (1u64 << m.size_bits) - 1;
        assert_eq!(r.m.page_table_walk(root, cuoi).expect("walk cuối"), *m);
        thong_ke[5] += 1;
    }
    // Các VA chắc chắn trống trong cây (root entry chưa dùng) ⇒ NotMapped.
    for va in [4 * G1 + 0x1000, 5 * G1, 10 * G1] {
        assert!(
            matches!(
                r.m.page_table_walk(root, va),
                Err(KError::NotMapped) | Err(KError::Alignment)
            ),
            "VA trống phải NotMapped/Alignment"
        );
    }
    // Không rò Frame/PageTable: mọi op đều đi qua check; số mapping khớp oracle.
    assert_eq!(
        r.m.page_table_info(root).expect("info").mapped as usize,
        oracle.len()
    );

    // In thống kê để log cổng có bằng chứng (không assert số cụ thể — chỉ cần đủ 6 loại).
    println!(
        "KT-37 property: {N_OPS} op, map_ok {}, map_err {}, unmap_ok {}, unmap_err {}, spot {}, full {}",
        thong_ke[0], thong_ke[1], thong_ke[2], thong_ke[3], thong_ke[4], thong_ke[5]
    );
    assert!(thong_ke[0] > 0 && thong_ke[1] > 0 && thong_ke[2] > 0 && thong_ke[3] > 0);
}

// ===========================================================================
// 8. Không rò PageTable/Frame: 1.000 vòng tạo → map → unmap → detach → revoke.
// ===========================================================================

#[test]
fn khong_ro_pagetable_frame_1000_vong() {
    #[cfg(miri)]
    const VONG: usize = 1;
    #[cfg(not(miri))]
    const VONG: usize = 100;
    #[cfg(miri)]
    const MOI_VONG: usize = 2;
    #[cfg(not(miri))]
    const MOI_VONG: usize = 10;
    let mut tong = 0usize;
    for vong in 0..VONG {
        let mut r = Rig::new();
        for _ in 0..MOI_VONG {
            let (root, mid, _leaf) = cay_4k(&mut r, 0);
            let (fs, _obj) = r.frame(12);
            r.m.page_table_map(root, 0x1000, fs, MapRights::READ, MapAttrs::WB)
                .expect("map");
            r.m.page_table_unmap(root, 0x1000).expect("unmap");
            r.m.page_table_detach(mid, 0).expect("detach leaf");
            r.m.page_table_detach(root, 0).expect("detach mid");
            r.kiem("vòng");
            tong += 1;
        }
        // Revoke Untyped bảng + Untyped Frame: mọi object chết, bộ đếm về 0, arena reset.
        r.m.revoke(r.c, Some(0)).expect("revoke bảng");
        r.m.revoke(r.c, Some(1)).expect("revoke frame");
        assert_eq!(r.m.page_table_count(), 0, "vòng {vong}: rò PageTable");
        assert_eq!(r.m.frame_count(), 0, "vòng {vong}: rò Frame");
        assert_eq!(r.m.untyped_used(r.upt), 0, "arena bảng chưa reset");
        r.kiem("revoke");
    }
    assert_eq!(tong, VONG * MOI_VONG);
}

// ===========================================================================
// 9. Lỗi cap/đối tượng của đường `map` qua cap (không phải 7 mã VM, nhưng là cửa ABI).
// ===========================================================================

#[test]
fn map_qua_cap_loi_co_ban() {
    let mut r = Rig::new();
    let (_, root) = r.tbl();
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, 0, mid).expect("attach");
    let (_, leaf) = r.tbl();
    r.m.page_table_attach(mid, 0, leaf).expect("attach leaf");
    let (f4, o4) = r.frame(12);
    let (tbl_slot, _) = r.tbl();

    // Slot rỗng ⇒ SlotEmpty.
    let empty = SlotRef::new(r.c, 120);
    assert_eq!(
        r.m.page_table_map(root, 0x1000, empty, MapRights::READ, MapAttrs::WB),
        Err(KError::SlotEmpty)
    );
    // Cap trỏ PageTable ⇒ WrongObjectType.
    assert_eq!(
        r.m.page_table_map(root, 0x1000, tbl_slot, MapRights::READ, MapAttrs::WB),
        Err(KError::WrongObjectType)
    );
    // Root không phải PageTable ⇒ WrongObjectType.
    assert_eq!(
        r.m.page_table_map_obj(o4, 0x1000, o4, MapRights::READ, MapAttrs::WB),
        Err(KError::WrongObjectType)
    );
    // Root đã chết (revoke) ⇒ InvalidCapability. Cây phải **tinh khiết** trước khi revoke:
    // gỡ leaf khỏi mid, mid khỏi root (không im lặng bỏ bảng — KT-37 DoD).
    r.m.page_table_detach(mid, 0).expect("detach leaf");
    r.m.page_table_detach(root, 0).expect("detach mid");
    r.m.revoke(r.c, Some(0)).expect("revoke bảng rỗng");
    assert_eq!(
        r.m.page_table_map(root, 0x1000, f4, MapRights::READ, MapAttrs::WB),
        Err(KError::InvalidCapability)
    );
    let _ = r.m.frame_meta(o4);
    r.kiem("cap errors");
}

// ===========================================================================
// 10. `page_table_pte_raw`: layout PTE mô hình (valid/table/rights/attrs/frame).
// ===========================================================================

#[test]
fn pte_layout_valid_table_quyen_attrs_frame() {
    let mut r = Rig::new();
    let (root, mid, leaf) = cay_4k(&mut r, 0);
    let (f4, o4) = r.frame(12);
    map4(&mut r, root, 0x1000, f4);

    // Root entry 0: valid + table, trỏ `mid`.
    let p = r.m.page_table_pte_raw(root, 0).expect("pte root");
    assert_eq!(p & 1, 1, "valid");
    assert_eq!((p >> 1) & 1, 1, "table");
    assert_eq!((p >> 10) as u32, mid, "chỉ số bảng con");
    // Mid entry 0: table, trỏ `leaf` (bảng lá).
    let p = r.m.page_table_pte_raw(mid, 0).expect("pte mid");
    assert_eq!((p >> 10) as u32, leaf);
    // Leaf entry 1: không table; rights READ = bit 2; attrs WB = bit 4; frame ở bit 10.
    let p = r.m.page_table_pte_raw(leaf, 1).expect("pte leaf");
    assert_eq!(p & 1, 1);
    assert_eq!((p >> 1) & 1, 0, "lá");
    assert_eq!((p >> 2) & 3, 1, "READ");
    assert_eq!((p >> 4) & 0x3f, 1, "WB");
    assert_eq!((p >> 10) as u32, o4);
    // Entry rỗng = 0.
    assert_eq!(r.m.page_table_pte_raw(leaf, 0).expect("pte rỗng"), 0);
    assert_eq!(r.m.page_table_pte_raw(leaf, 512), Err(KError::RangeError));
    r.kiem("pte layout");
}
