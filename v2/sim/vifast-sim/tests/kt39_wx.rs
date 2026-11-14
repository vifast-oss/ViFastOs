// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-39 — **W^X toàn cục theo Frame vật lý** + `protect` + bất biến/bộ đếm, model-only `[TK]`
//! (K2-PLAN §3 KT-39 + §2 Q3/Q7, **D2-65**, D2-67; `v2/docs/k2-vm.md` §13; thiết kế §4.10
//! `:353-356`).
//!
//! **K2 không phải kernel thật**: bộ đếm nằm trên `FrameMeta` của mô hình; không PTE/TLB/MMU
//! thật, không `sfence.vma`. KT-39 **không** làm TLB/shootdown (KT-40), VM fault (KT-41),
//! `.scn`/`check_vm` (KT-42), COW.
//!
//! Luật (Q3 + D2-67):
//! 1. `mapped_w`/`mapped_x` đếm **mọi mapping sống** của Frame, **toàn cục mọi VSpace/PageTable**;
//!    tăng ở map, giảm ở unmap/protect, kiểm lại từ cây ở `wx_recount`.
//! 2. Map tạo ra `w > 0 && x > 0` (kể cả W+X trong **một** lời gọi) ⇒ `WxViolation` **trước mọi
//!    thay đổi** (atomic); **không** tự unmap/hạ quyền — JIT phải unmap W rồi map X.
//! 3. `protect` tính lại hai bộ đếm; chuyển W→X/X→W là **yêu cầu tường minh** được phép nếu kết
//!    quả thoả bất biến; vi phạm ⇒ `WxViolation` atomic.
//! 4. `Device + EXEC` ⇒ `WxViolation` (D2-67); `NX` + `EXEC` cùng lúc ⇒ `InvalidArgument`;
//!    `WB/WC/UC/Device` phải **đúng một** giá trị (0 hoặc >1 ⇒ `InvalidArgument`).
//! 5. Shared read-only: cùng Frame map READ vào 2 VSpace — bộ đếm toàn cục + `maps` tăng đúng,
//!    **không** cấp bản sao Frame; unmap một bên còn đúng phần còn lại.
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt39-red-compile.log`](../../agent/JOURNAL/kt39-red-compile.log)
//! — API KT-39 chưa có (E0432 `WxRecount`, E0599 `check_wx`/`wx_recount`/`page_table_protect`/
//! `vspace_protect`) ⇒ `could not compile`. Đỏ **ngữ nghĩa** (map W sau X thành công) có ở
//! [`JOURNAL/kt39-red-semantic.log`](../../agent/JOURNAL/kt39-red-semantic.log) sau khi API có.

use std::collections::BTreeMap;

use vifast_sim::kobj::{
    CapId, KError, Machine, MapAttrs, MapRights, ObjId, ObjType, SlotRef, WxRecount,
};
use vifast_sim::Lcg;

const K4: u64 = 1 << 12;
const M2: u64 = 1 << 21;
const G1: u64 = 1 << 30;

// ===========================================================================
// Rig: CNode 1024 slot + Untyped cho root/cây PageTable + 3 Untyped cửa sổ PA Frame
// (4 KiB / 2 MiB / 1 GiB). Frame **không** tiêu byte arena (KT-36).
// ===========================================================================

struct Rig {
    m: Machine,
    c: ObjId,
    /// Untyped chứa root VSpace + PageTable của cây (một cửa sổ PA riêng, không dùng cho Frame).
    uva: ObjId,
    /// Untyped cho Frame 4 KiB (cửa sổ PA 1 GiB ở 4 GiB).
    ufr: ObjId,
    /// Untyped cho Frame 2 MiB (cửa sổ PA 1 GiB ở 8 GiB).
    ufr2: ObjId,
    /// Untyped cho Frame 1 GiB (cửa sổ PA 1 GiB ở 16 GiB) — một Frame 1 GiB chiếm trọn cửa sổ.
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

    fn put(&mut self, cap: CapId) -> SlotRef {
        let s = self.slot();
        self.put_at(cap, s)
    }

    fn put_at(&mut self, cap: CapId, s: usize) -> SlotRef {
        self.m.put_cap(self.c, s, cap).expect("đặt cap vào CNode");
        SlotRef::new(self.c, s)
    }

    /// Tạo VSpace bằng **model API** `vspace_create` + đặt cap vào slot kế; trả `(slot, ObjId)`.
    fn vspace(&mut self) -> (SlotRef, ObjId) {
        let cap = self.m.vspace_create(self.sva).expect("vspace_create");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Như [`Rig::vspace`] nhưng đặt cap vào slot cố định (vòng tạo/huỷ dùng lại slot).
    fn vspace_at(&mut self, slot: usize) -> (SlotRef, ObjId) {
        let cap = self.m.vspace_create(self.sva).expect("vspace_create");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put_at(cap, slot), obj)
    }

    /// Retype PageTable cấp dưới + đặt cap; trả `(slot, ObjId)`.
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

    /// Như [`Rig::frame`] nhưng cap Frame vào **slot cố định** (vòng tạo/huỷ).
    fn frame_at(&mut self, size_bits: u32, slot: usize) -> (SlotRef, ObjId) {
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
        (self.put_at(cap, slot), obj)
    }

    /// Bất biến sau mỗi op (KT-36/37/38/39): W^X + cây + Frame + VSpace + cap + byte.
    fn kiem(&self, nhan: &str) {
        self.m
            .check_wx()
            .unwrap_or_else(|e| panic!("{nhan}: check_wx: {e}"));
        self.m
            .check_pages()
            .unwrap_or_else(|e| panic!("{nhan}: check_pages: {e}"));
        self.m
            .check_frames()
            .unwrap_or_else(|e| panic!("{nhan}: check_frames: {e}"));
        self.m
            .check_vspace()
            .unwrap_or_else(|e| panic!("{nhan}: check_vspace: {e}"));
        self.m
            .check_asid()
            .unwrap_or_else(|e| panic!("{nhan}: check_asid: {e}"));
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

/// Dựng chuỗi root → mid (cấp 1) → leaf (cấp 0) cho vùng 4 KiB của `vs` tại `base`
/// (`base` phải căn 1 GiB cho mid và căn 2 MiB cho leaf; test dùng `base = 0`).
fn cay_4k(r: &mut Rig, vs: ObjId, base: u64) -> (ObjId, ObjId) {
    let root = r.m.vspace_root(vs).expect("root VSpace");
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, base, mid).expect("attach mid");
    let (_, leaf) = r.tbl();
    r.m.page_table_attach(mid, base, leaf).expect("attach leaf");
    (mid, leaf)
}

/// `(maps, mapped_w, mapped_x)` của Frame theo `FrameMeta`.
fn dem(r: &Rig, f: ObjId) -> (u32, u32, u32) {
    let meta = r.m.frame_meta(f).expect("frame_meta");
    (meta.maps, meta.mapped_w, meta.mapped_x)
}

// ===========================================================================
// 1. Map W sau X ⇒ `WxViolation` **atomic**, không hạ ngầm mapping X; đọc-only chia sẻ được;
//    JIT đúng thứ tự (unmap W rồi map X) thành công.
// ===========================================================================

#[test]
fn map_w_sau_x_bi_tu_choi_atomic_khong_ha_ngam() {
    let mut r = Rig::new(1, 512 * 1024);
    let (_, vs) = r.vspace();
    let (_, leaf) = cay_4k(&mut r, vs, 0);
    let (sf, f) = r.frame(12);
    let _root = r.m.vspace_root(vs).expect("root");

    // X trước: READ + WB|EXEC (một kiểu bộ nhớ + EXEC) ⇒ hợp lệ, w=0 x=1.
    let mx =
        r.m.vspace_map(vs, K4, sf, MapRights::READ, MapAttrs::WB | MapAttrs::EXEC)
            .expect("map X");
    assert_eq!(
        (mx.rights, mx.attrs),
        (MapRights::READ, MapAttrs::WB | MapAttrs::EXEC)
    );
    assert_eq!(dem(&r, f), (1, 0, 1));
    let info_truoc = r.m.page_table_info(leaf).expect("info leaf");
    r.kiem("map X");

    // W sau X ⇒ WxViolation **trước mọi thay đổi**: không PTE mới, bộ đếm giữ nguyên.
    assert_eq!(
        r.m.vspace_map(
            vs,
            2 * K4,
            sf,
            MapRights::READ | MapRights::WRITE,
            MapAttrs::WB
        ),
        Err(KError::WxViolation),
        "map WRITE khi mapped_x > 0 phải WxViolation"
    );
    assert_eq!(dem(&r, f), (1, 0, 1), "WxViolation không đổi bộ đếm");
    assert_eq!(
        r.m.vspace_walk(vs, 2 * K4),
        Err(KError::NotMapped),
        "WxViolation không được để lại PTE dở"
    );
    assert_eq!(
        r.m.vspace_walk(vs, K4),
        Ok(mx),
        "mapping X cũ **vẫn** còn (không bị hạ ngầm)"
    );
    let info_sau = r.m.page_table_info(leaf).expect("info leaf");
    assert_eq!(
        (info_truoc.entries, info_truoc.mapped),
        (info_sau.entries, info_sau.mapped),
        "cây PageTable không đổi sau WxViolation"
    );
    r.kiem("W sau X bị từ chối");

    // READ-only cùng Frame vẫn được (không phải W, không phải X) — chia sẻ đọc.
    r.m.vspace_map(vs, 3 * K4, sf, MapRights::READ, MapAttrs::WB)
        .expect("map READ chia sẻ");
    assert_eq!(dem(&r, f), (2, 0, 1));
    r.kiem("READ chia sẻ");

    // W+X trong **một** lời gọi cũng bị từ chối (tự nó tạo w>0 ∧ x>0).
    assert_eq!(
        r.m.vspace_map(
            vs,
            4 * K4,
            sf,
            MapRights::READ | MapRights::WRITE,
            MapAttrs::WB | MapAttrs::EXEC
        ),
        Err(KError::WxViolation)
    );
    assert_eq!(dem(&r, f), (2, 0, 1));
    assert_eq!(r.m.vspace_walk(vs, 4 * K4), Err(KError::NotMapped));
    r.kiem("W+X cùng lời gọi");

    // JIT đúng thứ tự: unmap X ⇒ w=0 x=0 cho mapping còn lại; map W được.
    r.m.vspace_unmap(vs, K4).expect("unmap X");
    assert_eq!(dem(&r, f), (1, 0, 0));
    let mw =
        r.m.vspace_map(vs, K4, sf, MapRights::READ | MapRights::WRITE, MapAttrs::WB)
            .expect("map W sau khi unmap X");
    assert_eq!(
        (mw.rights, mw.attrs),
        (MapRights::READ | MapRights::WRITE, MapAttrs::WB)
    );
    assert_eq!(dem(&r, f), (2, 1, 0));
    assert_eq!(
        r.m.vspace_map(
            vs,
            2 * K4,
            sf,
            MapRights::READ,
            MapAttrs::WB | MapAttrs::EXEC
        ),
        Err(KError::WxViolation),
        "map X khi mapped_w > 0 phải WxViolation"
    );
    assert_eq!(dem(&r, f), (2, 1, 0));
    assert_eq!(r.m.vspace_walk(vs, 2 * K4), Err(KError::NotMapped));
    r.kiem("W rồi X bị từ chối");
}

// ===========================================================================
// 2. Map X sau W ⇒ `WxViolation` atomic; thứ tự JIT ngược lại; `protect` W→X tường minh.
// ===========================================================================

#[test]
fn map_x_sau_w_bi_tu_choi_atomic_va_protect_tuong_minh() {
    let mut r = Rig::new(1, 512 * 1024);
    let (_, vs) = r.vspace();
    let (_, _leaf) = cay_4k(&mut r, vs, 0);
    let (sf, f) = r.frame(12);

    let mw =
        r.m.vspace_map(vs, K4, sf, MapRights::READ | MapRights::WRITE, MapAttrs::WB)
            .expect("map W");
    assert_eq!(
        (mw.rights, mw.attrs),
        (MapRights::READ | MapRights::WRITE, MapAttrs::WB)
    );
    assert_eq!(dem(&r, f), (1, 1, 0));
    r.kiem("map W");

    assert_eq!(
        r.m.vspace_map(
            vs,
            2 * K4,
            sf,
            MapRights::READ,
            MapAttrs::UC | MapAttrs::EXEC
        ),
        Err(KError::WxViolation),
        "map EXEC khi mapped_w > 0 phải WxViolation"
    );
    assert_eq!(dem(&r, f), (1, 1, 0));
    assert_eq!(r.m.vspace_walk(vs, 2 * K4), Err(KError::NotMapped));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(mw), "mapping W cũ vẫn còn");
    r.kiem("X sau W bị từ chối");

    // `protect` tường minh W→X trên **cùng** mapping: delta đúng, kết quả không vi phạm.
    let mp =
        r.m.vspace_protect(vs, K4, MapRights::READ, MapAttrs::UC | MapAttrs::EXEC)
            .expect("protect W→X tường minh");
    assert_eq!(
        (mp.rights, mp.attrs),
        (MapRights::READ, MapAttrs::UC | MapAttrs::EXEC)
    );
    assert_eq!(dem(&r, f), (1, 0, 1));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(mp));
    r.kiem("protect W→X");

    // Sau khi X: map W bị từ chối; protect X→W tường minh được.
    assert_eq!(
        r.m.vspace_map(vs, 2 * K4, sf, MapRights::WRITE, MapAttrs::WC),
        Err(KError::WxViolation)
    );
    assert_eq!(dem(&r, f), (1, 0, 1));
    let mw2 =
        r.m.vspace_protect(vs, K4, MapRights::READ | MapRights::WRITE, MapAttrs::WC)
            .expect("protect X→W tường minh");
    assert_eq!(dem(&r, f), (1, 1, 0));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(mw2));
    r.kiem("protect X→W");

    // unmap giảm đúng bộ đếm.
    r.m.vspace_unmap(vs, K4).expect("unmap");
    assert_eq!(dem(&r, f), (0, 0, 0));
    r.kiem("unmap");
}

// ===========================================================================
// 3. Tổ hợp attrs: đúng **một** kiểu bộ nhớ; `NX`+`EXEC` ⇒ InvalidArgument;
//    `Device`+`EXEC` ⇒ WxViolation (D2-67); mọi lỗi atomic.
// ===========================================================================

#[test]
fn attr_khong_hop_le_va_device_exec() {
    let mut r = Rig::new(1, 512 * 1024);
    let (_, vs) = r.vspace();
    let (_, _leaf) = cay_4k(&mut r, vs, 0);
    let (sf, f) = r.frame(12);

    for (attrs, loi) in [
        (MapAttrs::NONE, KError::InvalidArgument),
        (MapAttrs::WB | MapAttrs::WC, KError::InvalidArgument),
        (
            MapAttrs::WB | MapAttrs::DEVICE | MapAttrs::NX | MapAttrs::EXEC,
            KError::InvalidArgument,
        ),
        (MapAttrs::DEVICE | MapAttrs::EXEC, KError::WxViolation),
    ] {
        assert_eq!(
            r.m.vspace_map(vs, K4, sf, MapRights::READ, attrs),
            Err(loi),
            "attrs {attrs:?} phải {loi:?}"
        );
        assert_eq!(dem(&r, f), (0, 0, 0), "lỗi attrs không đổi bộ đếm");
        assert_eq!(r.m.vspace_walk(vs, K4), Err(KError::NotMapped));
        r.kiem("attrs lỗi");
    }

    // Hợp lệ: một kiểu bộ nhớ + `NX` (không EXEC).
    let mn =
        r.m.vspace_map(vs, K4, sf, MapRights::READ, MapAttrs::DEVICE | MapAttrs::NX)
            .expect("Device+NX hợp lệ");
    assert_eq!(
        (mn.rights, mn.attrs),
        (MapRights::READ, MapAttrs::DEVICE | MapAttrs::NX)
    );
    assert_eq!(dem(&r, f), (1, 0, 0));
    r.m.vspace_unmap(vs, K4).expect("unmap");
    // Hợp lệ: một kiểu bộ nhớ + `EXEC` (không NX, không Device).
    let mx =
        r.m.vspace_map(vs, K4, sf, MapRights::READ, MapAttrs::UC | MapAttrs::EXEC)
            .expect("UC+EXEC hợp lệ");
    assert_eq!(dem(&r, f), (1, 0, 1));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(mx));
    r.kiem("attrs hợp lệ");
}

// ===========================================================================
// 4. `protect`: tính lại hai bộ đếm, vi phạm ⇒ WxViolation atomic, lỗi khác giữ nguyên cây.
// ===========================================================================

#[test]
fn protect_tinh_lai_dem_va_atomic() {
    let mut r = Rig::new(1, 512 * 1024);
    let (_, vs) = r.vspace();
    let (_, _leaf) = cay_4k(&mut r, vs, 0);
    let (sf, f) = r.frame(12);

    r.m.vspace_map(vs, K4, sf, MapRights::READ | MapRights::WRITE, MapAttrs::WB)
        .expect("map W");
    assert_eq!(dem(&r, f), (1, 1, 0));

    // W → READ: giảm mapped_w đúng 1.
    let p1 =
        r.m.vspace_protect(vs, K4, MapRights::READ, MapAttrs::WB)
            .expect("protect READ");
    assert_eq!((p1.rights, p1.attrs), (MapRights::READ, MapAttrs::WB));
    assert_eq!(dem(&r, f), (1, 0, 0));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(p1));
    r.kiem("protect READ");

    // READ → EXEC: tăng mapped_x đúng 1.
    let p2 =
        r.m.vspace_protect(vs, K4, MapRights::READ, MapAttrs::UC | MapAttrs::EXEC)
            .expect("protect EXEC");
    assert_eq!(dem(&r, f), (1, 0, 1));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(p2));
    r.kiem("protect EXEC");

    // EXEC + WRITE ⇒ WxViolation **atomic**: PTE/counters không đổi.
    assert_eq!(
        r.m.vspace_protect(
            vs,
            K4,
            MapRights::READ | MapRights::WRITE,
            MapAttrs::WB | MapAttrs::EXEC
        ),
        Err(KError::WxViolation)
    );
    assert_eq!(dem(&r, f), (1, 0, 1));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(p2), "protect lỗi không đổi PTE");
    r.kiem("protect vi phạm");

    // Attr không hợp lệ ⇒ InvalidArgument, không đổi gì.
    assert_eq!(
        r.m.vspace_protect(vs, K4, MapRights::READ, MapAttrs::WB | MapAttrs::WC),
        Err(KError::InvalidArgument)
    );
    assert_eq!(dem(&r, f), (1, 0, 1));
    assert_eq!(r.m.vspace_walk(vs, K4), Ok(p2));
    r.kiem("protect attr lỗi");

    // Chưa map ⇒ NotMapped; va bên trong trang lớn ⇒ Alignment (không sửa ngầm).
    assert_eq!(
        r.m.vspace_protect(vs, 2 * K4, MapRights::READ, MapAttrs::WB),
        Err(KError::NotMapped)
    );
    r.kiem("protect NotMapped");

    // EXEC → WRITE tường minh: w=1, x=0 — hợp lệ.
    let p3 =
        r.m.vspace_protect(vs, K4, MapRights::WRITE, MapAttrs::WB)
            .expect("protect EXEC→WRITE");
    assert_eq!((p3.rights, p3.attrs), (MapRights::WRITE, MapAttrs::WB));
    assert_eq!(dem(&r, f), (1, 1, 0));
    r.kiem("protect EXEC→WRITE");
}

// ===========================================================================
// 5. Ba cỡ Frame (4 KiB / 2 MiB / 1 GiB): W^X, protect trên lá lớn, alignment.
// ===========================================================================

#[test]
fn ba_co_frame_wx_va_protect() {
    let mut r = Rig::new(1, 512 * 1024);
    let (_, vs) = r.vspace();
    // mid phủ 1 GiB tại 0 + leaf 4 KiB phủ 2 MiB tại 0; 2 MiB dùng mid trực tiếp.
    let (_, mid) = r.tbl();
    let root = r.m.vspace_root(vs).expect("root");
    r.m.page_table_attach(root, 0, mid).expect("attach mid");
    let (_, leaf) = r.tbl();
    r.m.page_table_attach(mid, 0, leaf).expect("attach leaf");

    // ---- 4 KiB ----
    let (sf4, f4) = r.frame(12);
    r.m.vspace_map(
        vs,
        K4,
        sf4,
        MapRights::READ | MapRights::WRITE,
        MapAttrs::WB,
    )
    .expect("map W 4K");
    assert_eq!(dem(&r, f4), (1, 1, 0));
    assert_eq!(
        r.m.vspace_map(
            vs,
            2 * K4,
            sf4,
            MapRights::READ,
            MapAttrs::WB | MapAttrs::EXEC
        ),
        Err(KError::WxViolation)
    );
    assert_eq!(dem(&r, f4), (1, 1, 0));
    r.kiem("4K W rồi X");

    // ---- 2 MiB ----
    let (sf2, f2) = r.frame(21);
    let va2 = 2 * M2;
    r.m.vspace_map(
        vs,
        va2,
        sf2,
        MapRights::READ | MapRights::WRITE,
        MapAttrs::WC,
    )
    .expect("map W 2M");
    assert_eq!(dem(&r, f2), (1, 1, 0));
    assert_eq!(
        r.m.vspace_map(
            vs,
            3 * M2,
            sf2,
            MapRights::READ,
            MapAttrs::WC | MapAttrs::EXEC
        ),
        Err(KError::WxViolation)
    );
    // protect địa chỉ bên trong trang 2 MiB ⇒ Alignment (không xoá ngầm cả trang).
    assert_eq!(
        r.m.vspace_protect(vs, va2 + K4, MapRights::READ, MapAttrs::WC),
        Err(KError::Alignment)
    );
    assert_eq!(dem(&r, f2), (1, 1, 0));
    let m2 = r.m.vspace_walk(vs, va2 + 0x1234).expect("walk trong 2M");
    assert_eq!(m2.size_bits, 21);
    r.kiem("2M W rồi X");

    // ---- 1 GiB ----
    let (sf1, f1) = r.frame(30);
    r.m.vspace_map(
        vs,
        G1,
        sf1,
        MapRights::READ | MapRights::WRITE,
        MapAttrs::WB,
    )
    .expect("map W 1G");
    assert_eq!(dem(&r, f1), (1, 1, 0));
    assert_eq!(
        r.m.vspace_map(
            vs,
            2 * G1,
            sf1,
            MapRights::READ,
            MapAttrs::WB | MapAttrs::EXEC
        ),
        Err(KError::WxViolation)
    );
    assert_eq!(dem(&r, f1), (1, 1, 0));
    r.kiem("1G W rồi X");

    // JIT cho từng cỡ: unmap W rồi map X; protect X→W tường minh trên lá lớn.
    for (va, sf, f) in [(K4, sf4, f4), (va2, sf2, f2), (G1, sf1, f1)] {
        let removed = r.m.vspace_unmap(vs, va).expect("unmap W");
        assert!(removed.rights.contains(MapRights::WRITE));
        assert_eq!(dem(&r, f), (0, 0, 0));
        r.m.vspace_map(vs, va, sf, MapRights::READ, MapAttrs::WB | MapAttrs::EXEC)
            .expect("map X sau unmap");
        assert_eq!(dem(&r, f), (1, 0, 1));
        let p =
            r.m.vspace_protect(vs, va, MapRights::READ | MapRights::WRITE, MapAttrs::WB)
                .expect("protect X→W lá lớn");
        let co_ky = if va == G1 {
            30
        } else if va == va2 {
            21
        } else {
            12
        };
        assert_eq!(p.size_bits, co_ky);
        assert_eq!(dem(&r, f), (1, 1, 0));
        r.kiem("JIT từng cỡ");
    }
}

// ===========================================================================
// 6. Shared read-only: **một** Frame trong **hai** VSpace, không cấp bản sao; bộ đếm toàn cục;
//    unmap một bên còn đúng phần còn lại; W^X vẫn đúng toàn cục.
// ===========================================================================

#[test]
fn shared_read_only_hai_vspace_khong_ban_sao() {
    let mut r = Rig::new(1, 1024 * 1024);
    let (_, vs_a) = r.vspace();
    let (_, vs_b) = r.vspace();
    cay_4k(&mut r, vs_a, 0);
    cay_4k(&mut r, vs_b, 0);
    let (sf, f) = r.frame(12);
    let pa = r.m.frame_address(f).expect("pa");
    assert_eq!(r.m.frame_count(), 1);
    assert_eq!(r.m.page_table_count(), 6, "2 root + 2×(mid+leaf)");
    let pt_truoc = r.m.page_table_count();

    // READ vào cả hai VSpace — **cùng** Frame, không bản sao.
    let ma =
        r.m.vspace_map(vs_a, K4, sf, MapRights::READ, MapAttrs::WB)
            .expect("map READ vs A");
    let mb =
        r.m.vspace_map(vs_b, K4, sf, MapRights::READ, MapAttrs::WB)
            .expect("map READ vs B");
    assert_eq!(dem(&r, f), (2, 0, 0));
    assert_eq!(ma.frame, mb.frame);
    assert_eq!(ma.pa, mb.pa);
    assert_eq!(ma.pa, pa);
    assert_eq!(r.m.frame_count(), 1, "không cấp Frame thứ hai");
    assert_eq!(r.m.page_table_count(), pt_truoc, "không cấp bảng sao");
    assert_eq!(r.m.vspace_walk(vs_a, K4), Ok(ma));
    assert_eq!(r.m.vspace_walk(vs_b, K4), Ok(mb));
    r.kiem("shared RO 2 VSpace");

    // Unmap một bên: bộ đếm còn đúng phần còn lại, bên kia không đổi.
    r.m.vspace_unmap(vs_a, K4).expect("unmap A");
    assert_eq!(dem(&r, f), (1, 0, 0));
    assert_eq!(r.m.vspace_walk(vs_a, K4), Err(KError::NotMapped));
    assert_eq!(r.m.vspace_walk(vs_b, K4), Ok(mb));
    r.kiem("unmap một VSpace");

    // Map W bên A (thêm mapping) rồi map X bên B ⇒ WxViolation toàn cục; mapping B còn nguyên.
    let mw =
        r.m.vspace_map(
            vs_a,
            K4,
            sf,
            MapRights::READ | MapRights::WRITE,
            MapAttrs::WB,
        )
        .expect("map W vs A");
    assert_eq!(dem(&r, f), (2, 1, 0));
    assert_eq!(
        r.m.vspace_map(
            vs_b,
            2 * K4,
            sf,
            MapRights::READ,
            MapAttrs::WB | MapAttrs::EXEC
        ),
        Err(KError::WxViolation),
        "W^X là bất biến toàn cục, không theo VSpace"
    );
    assert_eq!(dem(&r, f), (2, 1, 0));
    assert_eq!(r.m.vspace_walk(vs_a, K4), Ok(mw));
    assert_eq!(r.m.vspace_walk(vs_b, K4), Ok(mb));
    r.kiem("W ở A, X ở B bị từ chối");

    // JIT: unmap W ở A ⇒ bên B còn READ; map X bên B được.
    r.m.vspace_unmap(vs_a, K4).expect("unmap W A");
    assert_eq!(dem(&r, f), (1, 0, 0));
    let mx =
        r.m.vspace_map(
            vs_b,
            2 * K4,
            sf,
            MapRights::READ,
            MapAttrs::WB | MapAttrs::EXEC,
        )
        .expect("map X B");
    assert_eq!(dem(&r, f), (2, 0, 1));
    assert_eq!(r.m.vspace_walk(vs_b, K4), Ok(mb));
    assert_eq!(r.m.vspace_walk(vs_b, 2 * K4), Ok(mx));
    r.kiem("shared RO + X");

    // Dọn: 0 mapping, 0 Frame sống sau revoke.
    r.m.vspace_unmap(vs_b, K4).expect("unmap B K4");
    r.m.vspace_unmap(vs_b, 2 * K4).expect("unmap B 2K4");
    assert_eq!(dem(&r, f), (0, 0, 0));
    r.m.revoke(r.c, Some(1)).expect("revoke Untyped Frame");
    assert_eq!(r.m.frame_count(), 0);
    r.kiem("dọn shared RO");
}

// ===========================================================================
// 7. Property **10.000** thao tác map/unmap/protect trên 3 cỡ Frame, 1 VSpace: mỗi op so
//    kết quả với oracle (W^X toàn cục), bộ đếm sau mỗi op, `wx_recount` định kỳ.
// ===========================================================================

#[test]
fn property_10k_map_unmap_protect() {
    #[cfg(miri)]
    const SO_OP: usize = 60;
    #[cfg(not(miri))]
    const SO_OP: usize = 10_000;

    let mut r = Rig::new(1, 1024 * 1024);
    let (_, vs) = r.vspace();
    let root = r.m.vspace_root(vs).expect("root");
    // mid phủ 1 GiB tại 0; leaf 4 KiB phủ 2 MiB tại 0 (dưới mid).
    let (_, mid) = r.tbl();
    r.m.page_table_attach(root, 0, mid).expect("attach mid");
    let (_, _leaf) = r.tbl();
    r.m.page_table_attach(mid, 0, _leaf).expect("attach leaf");

    // 4 KiB: 16 ô; 2 MiB: 4 ô (từ 4 MiB để không đè leaf 4 KiB); 1 GiB: 2 ô.
    let mut slots: Vec<(u64, u8)> = Vec::new();
    for i in 0..16u64 {
        slots.push((K4 * (i + 1), 12));
    }
    for i in 0..4u64 {
        slots.push(((2 + i) * M2, 21));
    }
    for i in 0..2u64 {
        slots.push(((1 + i) * G1, 30));
    }
    let mut f4 = Vec::new();
    for _ in 0..6 {
        f4.push(r.frame(12).1);
    }
    let mut f2 = Vec::new();
    for _ in 0..4 {
        f2.push(r.frame(21).1);
    }
    let f1 = r.frame(30).1;

    // Oracle: va → (frame, rights, attrs, size_bits); bộ đếm kỳ vọng theo Frame.
    let mut oracle: BTreeMap<u64, (ObjId, MapRights, MapAttrs, u8)> = BTreeMap::new();
    let mut dem_ky: BTreeMap<ObjId, (u32, u32, u32)> = BTreeMap::new();
    for f in f4.iter().chain(f2.iter()).chain(std::iter::once(&f1)) {
        dem_ky.insert(*f, (0, 0, 0));
    }

    let mem_types = [MapAttrs::WB, MapAttrs::WC, MapAttrs::UC];
    let mut rng = Lcg::new(0x39_00_39);
    let mut so_map = 0usize;
    let mut so_unmap = 0usize;
    let mut so_protect = 0usize;
    let mut so_wx = 0usize;
    let mut so_am = 0usize;

    for step in 0..SO_OP {
        let chon = rng.below(100);
        if chon < 52 {
            // ----- map -----
            let (va, bits) = slots[rng.below(slots.len() as u64) as usize];
            let f = match bits {
                12 => f4[rng.below(f4.len() as u64) as usize],
                21 => f2[rng.below(f2.len() as u64) as usize],
                _ => f1,
            };
            let rights = match rng.below(3) {
                0 => MapRights::READ,
                1 => MapRights::READ | MapRights::WRITE,
                _ => MapRights::WRITE,
            };
            let mem = mem_types[rng.below(mem_types.len() as u64) as usize];
            let attrs = if rng.below(2) == 0 {
                mem | MapAttrs::NX
            } else {
                mem | MapAttrs::EXEC
            };
            let (maps, w, x) = *dem_ky.get(&f).expect("frame trong sổ");
            let w2 = w + u32::from(rights.contains(MapRights::WRITE));
            let x2 = x + u32::from(attrs.contains(MapAttrs::EXEC));
            let ky_vong = if oracle.contains_key(&va) {
                so_am += 1;
                Err(KError::AlreadyMapped)
            } else if w2 > 0 && x2 > 0 {
                so_wx += 1;
                Err(KError::WxViolation)
            } else {
                Ok(())
            };
            // Dùng đường `ObjId` (`vspace_map_obj` → cùng cửa `page_table_map_obj` policy);
            // cap Frame của từng Frame nằm trong CNode của rig và không cần cho oracle.
            let got = r.m.vspace_map_obj(vs, va, f, rights, attrs);
            match ky_vong {
                Ok(()) => {
                    let m = got.unwrap_or_else(|e| panic!("step {step}: map lỗi {e:?}"));
                    assert_eq!(m.va, va, "step {step}: va");
                    assert_eq!(m.frame, f, "step {step}: frame");
                    assert_eq!(m.size_bits, bits, "step {step}: cỡ");
                    assert_eq!(m.rights, rights, "step {step}: rights");
                    assert_eq!(m.attrs, attrs, "step {step}: attrs");
                    oracle.insert(va, (f, rights, attrs, bits));
                    let d = dem_ky.get_mut(&f).expect("frame trong sổ");
                    d.0 += 1;
                    d.1 += u32::from(rights.contains(MapRights::WRITE));
                    d.2 += u32::from(attrs.contains(MapAttrs::EXEC));
                    so_map += 1;
                }
                Err(e) => {
                    assert_eq!(got, Err(e), "step {step}: map phải {e:?}");
                    let d = dem_ky.get(&f).expect("frame trong sổ");
                    assert_eq!(*d, (maps, w, x), "step {step}: lỗi map đổi bộ đếm");
                }
            }
        } else if chon < 78 {
            // ----- unmap -----
            let (va, _) = slots[rng.below(slots.len() as u64) as usize];
            let cu = oracle.get(&va).copied();
            let got = r.m.vspace_unmap(vs, va);
            match cu {
                Some((f, _, _, _)) => {
                    let m = got.unwrap_or_else(|e| panic!("step {step}: unmap lỗi {e:?}"));
                    assert_eq!(m.va, va);
                    oracle.remove(&va);
                    let d = dem_ky.get_mut(&f).expect("frame trong sổ");
                    d.0 -= 1;
                    d.1 -= u32::from(m.rights.contains(MapRights::WRITE));
                    d.2 -= u32::from(m.attrs.contains(MapAttrs::EXEC));
                    so_unmap += 1;
                }
                None => {
                    assert_eq!(got, Err(KError::NotMapped), "step {step}: unmap chưa map");
                }
            }
        } else {
            // ----- protect -----
            let va = if oracle.is_empty() {
                continue;
            } else {
                let keys: Vec<u64> = oracle.keys().copied().collect();
                keys[rng.below(keys.len() as u64) as usize]
            };
            let (f, old_r, old_a, bits) = *oracle.get(&va).expect("va trong oracle");
            let rights = match rng.below(3) {
                0 => MapRights::READ,
                1 => MapRights::READ | MapRights::WRITE,
                _ => MapRights::WRITE,
            };
            let mem = mem_types[rng.below(mem_types.len() as u64) as usize];
            let attrs = if rng.below(2) == 0 {
                mem | MapAttrs::NX
            } else {
                mem | MapAttrs::EXEC
            };
            let (maps, w, x) = *dem_ky.get(&f).expect("frame trong sổ");
            let w2 = w - u32::from(old_r.contains(MapRights::WRITE))
                + u32::from(rights.contains(MapRights::WRITE));
            let x2 = x - u32::from(old_a.contains(MapAttrs::EXEC))
                + u32::from(attrs.contains(MapAttrs::EXEC));
            let ky_vong = if w2 > 0 && x2 > 0 {
                so_wx += 1;
                Err(KError::WxViolation)
            } else {
                Ok(())
            };
            let got = r.m.vspace_protect(vs, va, rights, attrs);
            match ky_vong {
                Ok(()) => {
                    let m = got.unwrap_or_else(|e| panic!("step {step}: protect lỗi {e:?}"));
                    assert_eq!(m.va, va);
                    assert_eq!(m.frame, f);
                    assert_eq!(m.size_bits, bits);
                    assert_eq!(m.rights, rights);
                    assert_eq!(m.attrs, attrs);
                    oracle.insert(va, (f, rights, attrs, bits));
                    let d = dem_ky.get_mut(&f).expect("frame trong sổ");
                    assert_eq!(d.0, maps);
                    *d = (maps, w2, x2);
                    so_protect += 1;
                }
                Err(e) => {
                    assert_eq!(got, Err(e), "step {step}: protect phải {e:?}");
                    assert_eq!(
                        *dem_ky.get(&f).expect("frame trong sổ"),
                        (maps, w, x),
                        "step {step}: lỗi protect đổi bộ đếm"
                    );
                    assert_eq!(
                        *oracle.get(&va).expect("mapping còn"),
                        (f, old_r, old_a, bits),
                        "step {step}: lỗi protect đổi mapping"
                    );
                }
            }
        }

        // Bộ đếm thật khớp sổ kỳ vọng sau **mỗi** op.
        for (f, ky) in &dem_ky {
            assert_eq!(dem(&r, *f), *ky, "step {step}: bộ đếm Frame {f}");
        }
        // Bất biến chạy sẵn trong `sched_after_op`; gọi tường minh định kỳ để chốt.
        if step % 64 == 0 || step + 1 == SO_OP {
            r.m.check_wx().expect("check_wx");
            r.m.check_pages().expect("check_pages");
            assert_eq!(
                r.m.wx_recount(),
                Ok(recount(&dem_ky)),
                "step {step}: recount"
            );
        }
    }

    // Thống kê có ích để log (không phải ngưỡng): phải có đủ 3 loại op + ca WxViolation.
    #[cfg(not(miri))]
    {
        assert!(so_map > 0 && so_unmap > 0 && so_protect > 0);
        assert!(so_am > 0, "property phải có ca AlreadyMapped");
        assert!(so_wx > 0, "property phải có ca WxViolation");
    }
    let _ = (so_map, so_unmap, so_protect, so_am, so_wx);

    // Dọn: unmap hết ⇒ 0 mapping, đếm lại từ cây = 0.
    let vas: Vec<u64> = oracle.keys().copied().collect();
    for va in vas {
        r.m.vspace_unmap(vs, va).expect("unmap dọn");
    }
    for f in f4.iter().chain(f2.iter()).chain(std::iter::once(&f1)) {
        assert_eq!(dem(&r, *f), (0, 0, 0));
    }
    assert_eq!(
        r.m.wx_recount(),
        Ok(WxRecount {
            frames: 0,
            maps: 0,
            mapped_w: 0,
            mapped_x: 0,
        })
    );
    r.kiem("property xong");
}

/// Tổng hợp `wx_recount` kỳ vọng từ sổ bộ đếm (chỉ Frame có mapping sống).
fn recount(dem_ky: &BTreeMap<ObjId, (u32, u32, u32)>) -> WxRecount {
    let mut out = WxRecount::default();
    for (maps, w, x) in dem_ky.values() {
        if *maps > 0 {
            out.frames += 1;
            out.maps += u64::from(*maps);
            out.mapped_w += u64::from(*w);
            out.mapped_x += u64::from(*x);
        }
    }
    out
}

// ===========================================================================
// 8. 1.000 vòng tạo/huỷ (VSpace + Frame 1 GiB, không bảng trung gian): không rò Frame/
//    PageTable, bộ đếm về 0 sau mỗi vòng, `check_wx`/`wx_recount` sạch.
// ===========================================================================

#[test]
fn vong_1000_tao_huy_khong_ro() {
    #[cfg(miri)]
    const VONG: usize = 5;
    #[cfg(not(miri))]
    const VONG: usize = 1_000;
    const SLOT_VS: usize = 5;
    const SLOT_F: usize = 6;

    let mut r = Rig::new(1, 64 * 1024);
    for i in 0..VONG {
        let (_, vs) = r.vspace_at(SLOT_VS);
        let root = r.m.vspace_root(vs).expect("root");
        let (fc, f) = r.frame_at(30, SLOT_F);
        let m =
            r.m.vspace_map(vs, G1, fc, MapRights::READ | MapRights::WRITE, MapAttrs::WB)
                .expect("map 1G");
        assert_eq!(m.size_bits, 30);
        assert_eq!(dem(&r, f), (1, 1, 0), "vòng {i}: bộ đếm sau map");
        // W^X trước teardown: map X cùng Frame bị từ chối, counters giữ nguyên.
        assert_eq!(
            r.m.vspace_map(
                vs,
                2 * G1,
                fc,
                MapRights::READ,
                MapAttrs::WB | MapAttrs::EXEC
            ),
            Err(KError::WxViolation),
            "vòng {i}: W+X phải bị từ chối"
        );
        assert_eq!(dem(&r, f), (1, 1, 0), "vòng {i}: WxViolation không đổi");
        r.m.vspace_unmap(vs, G1).expect("unmap 1G");
        assert_eq!(dem(&r, f), (0, 0, 0), "vòng {i}: bộ đếm sau unmap");
        assert_eq!(r.m.vspace_walk(vs, G1), Err(KError::NotMapped));
        assert_eq!(r.m.page_table_walk(root, G1), Err(KError::NotMapped));
        // Teardown sạch: root tinh khiết + Frame 0 mapping ⇒ revoke Untyped xoá cả cây.
        r.m.revoke(r.c, Some(0)).expect("revoke Untyped VSpace");
        r.m.revoke(r.c, Some(3)).expect("revoke Untyped Frame");
        assert_eq!(r.m.vspace_count(), 0, "vòng {i}: rò VSpace");
        assert_eq!(r.m.page_table_count(), 0, "vòng {i}: rò PageTable");
        assert_eq!(r.m.frame_count(), 0, "vòng {i}: rò Frame");
        assert_eq!(
            r.m.wx_recount(),
            Ok(WxRecount::default()),
            "vòng {i}: recount"
        );
        if i % 50 == 0 || i + 1 == VONG {
            r.kiem("vòng tạo/huỷ");
        }
    }
    assert_eq!(
        r.m.untyped_used(r.uva),
        0,
        "không rò byte arena bảng/VSpace"
    );
}
