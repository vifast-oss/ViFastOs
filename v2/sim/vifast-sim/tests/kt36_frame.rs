// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-36 — **Frame 3 cỡ** (4 KiB / 2 MiB / 1 GiB) + ownership/Untyped + `Frame.GetAddress` +
//! bộ đếm W^X (`mapped_w`/`mapped_x`/`maps`), model-only `[TK]` (K2-PLAN §3 KT-36, D2-65
//! Q2/Q3/Q7, `v2/docs/k2-vm.md` §10, thiết kế §4.10 `:353-356`).
//!
//! **KT-36 KHÔNG map Frame vào VSpace/PageTable** — việc đó là KT-37/KT-39. Ở đây chỉ có:
//! 1. `Untyped.Retype(Untyped, Frame, size_bits)` với `size_bits ∈ {12, 21, 30}`; cỡ khác ⇒
//!    `InvalidArgument` (không còn `NotSupported` như thời reserved).
//! 2. `pa` căn đúng cỡ Frame **và** nằm trong cửa sổ PA của Untyped sở hữu; lệch căn ⇒
//!    `Alignment` (không căn lề ngầm — KT-36 DoD "pa lệch 1 ⇒ Alignment").
//! 3. `Frame.GetAddress` (method 2) trả `pa`; frame **không cấp byte thật**: 1 GiB vẫn 0 byte
//!    arena, không `Vec` 1 GiB (rủi ro (a) của KT-36).
//! 4. Frame chết ⇒ **không còn mapping**: bộ đếm `maps/mapped_w/mapped_x` phải bằng 0; nếu
//!    không, `Revoke` **từ chối** bằng `HasChildren` (atomic, không im lặng bỏ mapping).
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt36-red-compile.log`](../../agent/JOURNAL/kt36-red-compile.log)
//! (`FrameMeta`, `UNTYPED_PA_WINDOW`, `Machine::{frame_meta, frame_address, frame_get_address,
//! frame_note_map, frame_note_unmap, frame_count, untyped_frames, untyped_pa_window,
//! check_frames}` chưa có ⇒ E0432/E0599) và ngữ nghĩa
//! [`JOURNAL/kt36-red-semantics.log`](../../agent/JOURNAL/kt36-red-semantics.log) (`Retype` Frame
//! còn trả `NotSupported` cho mọi `size_bits`).
//!
//! Bất biến được kiểm sau **mỗi** thao tác: `check_frames` + `check_caps` + `check_all_bytes`
//! ("check_frames sớm", KT-36 DoD 5 — KT-42 sẽ mở rộng thành `check_vm`).

use vifast_sim::kobj::{FrameMeta, KError, Machine, ObjId, ObjType, SlotRef, UNTYPED_PA_WINDOW};
use vifast_sim::Lcg;

const K4: u64 = 1 << 12;
const M2: u64 = 1 << 21;
const G1: u64 = 1 << 30;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Rig tối thiểu: 1 CNode (`cap_slots` slot), 1 Untyped arena **4096 B** (cố ý nhỏ), cap
/// Untyped ở slot 0; Frame lần lượt vào slot 1, 2, ...
///
/// Arena 4096 B là **cố ý**: Frame không tiêu byte arena, nên cửa sổ PA 1 GiB vẫn dùng được
/// dù Untyped chỉ có 4 KiB — đây là bằng chứng "1 GiB ảo không ăn RSS" (KT-36 rủi ro (a)).
struct Rig {
    m: Machine,
    c: ObjId,
    u: ObjId,
    src: SlotRef,
    ke: usize,
}

impl Rig {
    fn new(base: u64) -> Self {
        Self::new_with(base, 8)
    }

    fn new_with(base: u64, cap_slots: usize) -> Self {
        let mut m = Machine::new();
        let c = m.add_cnode(cap_slots);
        let u = m.add_untyped(base, 4096);
        let cap = m.root_cap(u);
        m.put_cap(c, 0, cap).expect("cap Untyped vào slot 0");
        Self {
            m,
            c,
            u,
            src: slot(c, 0),
            ke: 1,
        }
    }

    /// Retype một Frame rồi đặt cap vào slot kế; trả `(slot, ObjId)`.
    fn frame(&mut self, size_bits: u32) -> Result<(SlotRef, ObjId), KError> {
        let cap = self.m.retype(self.src, ObjType::Frame, size_bits, 0)?;
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        let s = self.ke;
        self.ke += 1;
        self.m.put_cap(self.c, s, cap).expect("đặt cap Frame");
        Ok((slot(self.c, s), obj))
    }

    /// Frame chắc chắn thành công (dùng cho ca không kiểm lỗi).
    fn ok_frame(&mut self, size_bits: u32) -> (SlotRef, ObjId) {
        self.frame(size_bits)
            .unwrap_or_else(|e| panic!("retype Frame size_bits={size_bits}: {e:?}"))
    }

    /// `check_frames` + `check_caps` + `check_bytes` sau mỗi thao tác.
    fn kiem(&self, nhan: &str) {
        self.m
            .check_frames()
            .unwrap_or_else(|e| panic!("{nhan}: check_frames: {e}"));
        self.m
            .check_caps()
            .unwrap_or_else(|e| panic!("{nhan}: check_caps: {e}"));
        self.m
            .check_all_bytes()
            .unwrap_or_else(|e| panic!("{nhan}: check_bytes: {e}"));
    }
}

// ===========================================================================
// 1. `size_bits` lạ ⇒ `InvalidArgument` (biên {0, 1, 11, 13, 20, 22, 29, 31, 64, 1000})
//    — không tạo Frame, không tiêu byte, không đổi watermark.
// ===========================================================================

#[test]
fn retype_frame_size_bits_la_invalid_argument() {
    let mut r = Rig::new(0);
    for bits in [0u32, 1, 11, 13, 20, 22, 29, 31, 32, 64, 255, 256, 1000] {
        assert_eq!(
            r.m.retype(r.src, ObjType::Frame, bits, 0),
            Err(KError::InvalidArgument),
            "size_bits={bits} không thuộc {{12,21,30}} ⇒ InvalidArgument"
        );
        r.kiem(&format!("size_bits={bits}"));
        assert_eq!(r.m.frame_count(), 0, "từ chối không được tạo Frame");
        assert_eq!(r.m.untyped_used(r.u), 0, "từ chối không đốt byte arena");
    }
    // Cỡ hợp lệ vẫn cấp ngay sau các lần từ chối, tại watermark 0.
    let (_, f) = r.frame(12).expect("4 KiB sau các lần từ chối");
    assert_eq!(r.m.frame_address(f).expect("pa"), 0);
    assert_eq!(r.m.untyped_used(r.u), 0, "Frame không tiêu byte arena");
    r.kiem("sau từ chối + 1 frame");
}

// ===========================================================================
// 2. Ba cỡ: retype → `GetAddress` → `FrameMeta` → bộ đếm → huỷ được.
// ===========================================================================

#[test]
fn ba_co_retype_get_address_va_meta() {
    for (bits, base, co) in [(12u32, 0u64, K4), (21, M2, M2), (30, G1, G1)] {
        let mut r = Rig::new(base);
        let (s, f) = r.ok_frame(bits);

        // `Frame.GetAddress` qua cap (ABI method 2) và qua ObjId (model) cùng một `pa`.
        let pa = r.m.frame_get_address(s).expect("GetAddress");
        assert_eq!(pa, base, "pa = base + watermark 0");
        assert_eq!(r.m.frame_address(f).expect("pa theo ObjId"), pa);
        assert_eq!(pa % co, 0, "pa phải căn đúng cỡ Frame {co}");

        let meta: FrameMeta = r.m.frame_meta(f).expect("meta");
        assert_eq!(meta.pa, pa);
        assert_eq!(meta.size_bits, bits as u8);
        assert!(meta.live, "frame mới phải sống");
        assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (0, 0, 0));
        assert_eq!(meta.size_bytes(), co);
        assert!(meta.counters_zero());

        // "map được/huỷ được" của KT-36 = bộ đếm W^X tăng/giảm (map thật là KT-37/39).
        r.m.frame_note_map(f, true, false).expect("note_map W");
        r.m.frame_note_map(f, false, true).expect("note_map X");
        assert_eq!(r.m.frame_meta(f).expect("meta").maps, 2);
        r.m.frame_note_unmap(f, true, false).expect("unmap W");
        r.m.frame_note_unmap(f, false, true).expect("unmap X");

        // Huỷ: revoke Untyped ⇒ Frame chết, bộ đếm sạch.
        r.m.revoke(r.c, Some(0)).expect("revoke Untyped");
        assert!(!r.m.frame_meta(f).expect("meta sau chết").live);
        assert!(r.m.frame_meta(f).expect("meta").counters_zero());
        r.kiem("3 cỡ sau huỷ");
    }
}

// ===========================================================================
// 3. Biên `pa`: 0 / 2^12 / 2^21 / 2^30 (KT-36 DoD 2).
// ===========================================================================

#[test]
fn bien_pa_0_2mu12_2mu21_2mu30() {
    // Biên 0 và 2^12: hai Frame 4 KiB liên tiếp trong cùng cửa sổ.
    let mut r = Rig::new(0);
    let (_, f0) = r.ok_frame(12);
    let (_, f1) = r.ok_frame(12);
    assert_eq!(r.m.frame_address(f0).expect("pa"), 0, "biên 0");
    assert_eq!(r.m.frame_address(f1).expect("pa"), K4, "biên 2^12");
    r.kiem("biên 0/2^12");

    let mut r2 = Rig::new(M2);
    let (_, f2) = r2.ok_frame(21);
    assert_eq!(r2.m.frame_address(f2).expect("pa"), M2, "biên 2^21");
    r2.kiem("biên 2^21");

    let mut r3 = Rig::new(G1);
    let (_, f3) = r3.ok_frame(30);
    assert_eq!(r3.m.frame_address(f3).expect("pa"), G1, "biên 2^30");
    r3.kiem("biên 2^30");
}

// ===========================================================================
// 4. `pa` lệch 1 ⇒ `Alignment` (KT-36 DoD 3) — **không** căn lề ngầm, atomic.
// ===========================================================================

#[test]
fn pa_lech_1_la_alignment() {
    for bits in [12u32, 21, 30] {
        let mut r = Rig::new(1);
        assert_eq!(
            r.m.retype(r.src, ObjType::Frame, bits, 0),
            Err(KError::Alignment),
            "size_bits={bits}: pa = base + 0 = 1 ⇒ lệch căn ⇒ Alignment"
        );
        r.kiem("pa lệch 1");
        assert_eq!(r.m.frame_count(), 0, "Alignment không được tạo Frame");
        assert_eq!(r.m.untyped_used(r.u), 0);
    }
    // base 0x1001: `pa` lệch 1 với mọi cỡ ≥ 4 KiB.
    let mut r = Rig::new(0x1001);
    assert_eq!(
        r.m.retype(r.src, ObjType::Frame, 21, 0),
        Err(KError::Alignment)
    );

    // Cửa sổ trộn cỡ **tăng dần**: 4 KiB ở 0 ⇒ watermark 4096; 2 MiB tại `pa = 4096` lệch
    // căn 2 MiB ⇒ `Alignment` (mô hình **không** tự đệm căn lề; thứ tự giảm dần mới hợp lệ).
    let mut r2 = Rig::new(0);
    let (_, f4k) = r2.ok_frame(12);
    assert_eq!(r2.m.frame_address(f4k).expect("pa"), 0);
    assert_eq!(
        r2.m.retype(r2.src, ObjType::Frame, 21, 0),
        Err(KError::Alignment),
        "pa kế = 4096 (lệch căn 2 MiB) ⇒ Alignment, không đệm ngầm"
    );
    // Watermark **không** đổi ⇒ frame 4 KiB kế tiếp vẫn ở 2^12 (bằng chứng atomic).
    let (_, f4k_b) = r2.ok_frame(12);
    assert_eq!(r2.m.frame_address(f4k_b).expect("pa"), K4);
    r2.kiem("Alignment atomic");
}

// ===========================================================================
// 5. 1 GiB **không** ăn byte thật; cửa sổ PA đầy ⇒ `NotEnoughMemory` (KT-36 DoD 4).
// ===========================================================================

#[test]
fn mot_gib_khong_an_byte_that() {
    let mut r = Rig::new(0);
    let arena_truoc = r.m.arena_used();
    let byte_truoc = r.m.untyped_used(r.u);
    let tong_truoc = r.m.total_used();

    let (_, f) = r.ok_frame(30);

    assert_eq!(
        r.m.untyped_used(r.u),
        byte_truoc,
        "Frame không tiêu byte Untyped"
    );
    assert_eq!(r.m.total_used(), tong_truoc, "không đụng watermark nào");
    let delta = r.m.arena_used() - arena_truoc;
    assert!(
        delta < 4096,
        "1 GiB Frame chỉ tốn metadata (< 1 trang), không `Vec` 1 GiB (delta = {delta} B)"
    );
    assert_eq!(r.m.frame_meta(f).expect("meta").size_bytes(), G1);

    // Cửa sổ PA của Untyped đúng 1 GiB ⇒ đã dùng hết, Frame kế tiếp không còn chỗ.
    assert_eq!(r.m.untyped_pa_window(r.u), Some((0, G1)));
    assert_eq!(
        r.m.retype(r.src, ObjType::Frame, 12, 0),
        Err(KError::NotEnoughMemory),
        "cửa sổ PA đã đầy ⇒ NotEnoughMemory (không tràn, không panic)"
    );
    r.kiem("1 GiB đầy cửa sổ");

    // Thu hồi rồi cấp lại: watermark cửa sổ về 0 (như revoke Untyped của KT-09).
    r.m.revoke(r.c, Some(0)).expect("revoke");
    let (_, f2) = r.ok_frame(12);
    assert_eq!(r.m.frame_address(f2).expect("pa"), 0);
}

// ===========================================================================
// 6. Trộn cỡ **giảm dần** trong một cửa sổ: không chồng, `pa` đúng vị trí.
// ===========================================================================

#[test]
fn tron_co_giam_dan_trong_mot_cua_so() {
    // 2 MiB ở 0, rồi 4 KiB ở 2^21 (giảm dần ⇒ căn lề tự nhiên, không đệm).
    let mut r = Rig::new(0);
    let (_, fm) = r.ok_frame(21);
    let (_, fk) = r.ok_frame(12);
    let (_, fk2) = r.ok_frame(12);
    assert_eq!(r.m.frame_address(fm).expect("pa"), 0);
    assert_eq!(r.m.frame_address(fk).expect("pa"), M2);
    assert_eq!(r.m.frame_address(fk2).expect("pa"), M2 + K4);
    r.kiem("2 MiB ↓ 4 KiB");
    assert_eq!(
        r.m.untyped_pa_window(r.u),
        Some((0, UNTYPED_PA_WINDOW)),
        "cửa sổ PA = 1 GiB `[TK]` (k2-vm.md §10)"
    );
}

// ===========================================================================
// 7. Frame chết ⇒ bộ đếm sạch; Revoke khi còn mapping ⇒ `HasChildren` **atomic**.
// ===========================================================================

#[test]
fn frame_chet_sach_sau_revoke() {
    let mut r = Rig::new(0);
    let (_, fa) = r.ok_frame(21);
    let (_, fb) = r.ok_frame(12);
    // Bộ đếm khác 0 **rồi về 0** trước khi chết.
    r.m.frame_note_map(fb, true, false).expect("map W");
    r.m.frame_note_unmap(fb, true, false).expect("unmap W");
    r.m.revoke(r.c, Some(0)).expect("revoke Untyped");
    for (f, nhan) in [(fa, "2 MiB"), (fb, "4 KiB")] {
        let meta = r.m.frame_meta(f).expect("meta sau chết");
        assert!(!meta.live, "{nhan}: phải chết");
        assert_eq!(
            (meta.maps, meta.mapped_w, meta.mapped_x),
            (0, 0, 0),
            "{nhan}: frame chết phải không còn mapping"
        );
        assert!(meta.counters_zero());
    }
    assert_eq!(r.m.frame_count(), 0);
    r.kiem("frame chết sạch");
}

#[test]
fn revoke_bi_tu_choi_khi_con_mapping() {
    let mut r = Rig::new(0);
    let (sf, f) = r.ok_frame(12);
    r.m.frame_note_map(f, true, false).expect("map W");
    r.m.frame_note_map(f, false, true).expect("map X");

    let err =
        r.m.revoke(r.c, Some(0))
            .expect_err("còn mapping ⇒ không được chết");
    assert_eq!(
        err,
        KError::HasChildren,
        "frame còn mapping sống ⇒ lỗi bất biến rõ (không im lặng bỏ mapping)"
    );
    // Atomic: chưa có gì chết, bộ đếm còn nguyên, cap còn dùng được.
    assert!(r.m.frame_meta(f).expect("meta").live);
    assert_eq!(r.m.frame_meta(f).expect("meta").maps, 2);
    assert_eq!(r.m.frame_count(), 1);
    assert_eq!(r.m.frame_get_address(sf).expect("cap còn sống"), 0);
    r.kiem("revoke bị từ chối");

    // Gỡ mapping rồi mới chết được.
    r.m.frame_note_unmap(f, true, false).expect("unmap W");
    r.m.frame_note_unmap(f, false, true).expect("unmap X");
    r.m.revoke(r.c, Some(0))
        .expect("revoke sau khi sạch mapping");
    assert!(!r.m.frame_meta(f).expect("meta").live);
    r.kiem("revoke sau khi sạch");
}

// ===========================================================================
// 8. Bất biến bộ đếm W^X (Q3): `mapped_w + mapped_x ≤ maps`, không trừ âm, lỗi rõ.
// ===========================================================================

#[test]
fn bat_bien_bo_dem_map() {
    let mut r = Rig::new(0);
    let (sf, f) = r.ok_frame(12);

    // Không có mapping mà unmap ⇒ IllegalOperation (không trừ âm).
    assert_eq!(
        r.m.frame_note_unmap(f, true, false),
        Err(KError::IllegalOperation)
    );
    assert_eq!(
        r.m.frame_note_unmap(f, false, true),
        Err(KError::IllegalOperation)
    );
    // Một mapping vừa W vừa X: **luật** W^X là KT-39 (KT-36 cố ý chưa áp), nên bất biến ở đây
    // là **từng bộ đếm ≤ `maps`** — `w + x ≤ maps` chỉ đúng sau khi KT-39 cấm mapping W+X.
    r.m.frame_note_map(f, true, true).expect("map W+X");
    let meta = r.m.frame_meta(f).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 1, 1));
    assert!(meta.mapped_w <= meta.maps && meta.mapped_x <= meta.maps);
    r.m.frame_note_map(f, true, false).expect("map W thứ hai");
    r.m.frame_note_unmap(f, true, true).expect("unmap W+X");
    let meta = r.m.frame_meta(f).expect("meta");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 1, 0));
    assert_eq!(
        r.m.frame_note_unmap(f, false, true),
        Err(KError::IllegalOperation),
        "mapped_x = 0 ⇒ không được trừ"
    );

    // Sai loại object ⇒ `WrongObjectType`; cap không phải Frame ⇒ `WrongObjectType`.
    let ep = r.m.add_kernel_object(ObjType::Endpoint);
    assert_eq!(
        r.m.frame_meta(ep).map(|_| ()).unwrap_err(),
        KError::WrongObjectType,
        "Endpoint không phải Frame"
    );
    let cap_ep = r.m.root_cap(ep);
    r.m.put_cap(r.c, 7, cap_ep).expect("cap Endpoint slot 7");
    assert_eq!(
        r.m.frame_get_address(slot(r.c, 7)),
        Err(KError::WrongObjectType)
    );
    // Cap Untyped ở slot 0 ⇒ cũng `WrongObjectType`; slot rỗng ⇒ `SlotEmpty`;
    // slot ngoài dải ⇒ `RangeError`.
    assert_eq!(r.m.frame_get_address(r.src), Err(KError::WrongObjectType));
    assert_eq!(r.m.frame_get_address(slot(r.c, 6)), Err(KError::SlotEmpty));
    assert_eq!(
        r.m.frame_get_address(slot(r.c, 99)),
        Err(KError::RangeError)
    );

    // Gỡ mapping còn lại (W) rồi mới xoá được — "frame chết phải sạch mapping".
    r.m.frame_note_unmap(f, true, false).expect("unmap W cuối");
    assert!(r.m.frame_meta(f).expect("meta").counters_zero());
    // Frame chết rồi: op ghi/đọc ⇒ `InvalidCapability` (không dùng object chết).
    r.m.revoke(r.c, Some(0)).expect("revoke");
    assert_eq!(
        r.m.frame_note_map(f, true, false),
        Err(KError::InvalidCapability)
    );
    assert_eq!(r.m.frame_address(f), Err(KError::InvalidCapability));
    // Cap Frame đã bị `Revoke` xoá khỏi slot ⇒ `GetAddress` qua slot thấy ô rỗng.
    assert_eq!(r.m.frame_get_address(sf), Err(KError::SlotEmpty));
    r.kiem("bất biến bộ đếm");
}

// ===========================================================================
// 9. Frame thuộc **Untyped đã retype**; hai cửa sổ chồng nhau ⇒ `AlreadyMapped`.
// ===========================================================================

#[test]
fn frame_thuoc_untyped_va_khong_chong_pa() {
    // 16 slot: 1 Untyped nguồn + 3 Frame + 2 Untyped phụ, không đụng nhau.
    let mut r = Rig::new_with(0, 16);
    let (_, f) = r.ok_frame(12);
    assert_eq!(r.m.untyped_frames(r.u), vec![f], "Frame thuộc Untyped");
    assert!(r.m.untyped_frames(1).is_empty(), "CNode không có Frame");
    assert_eq!(r.m.untyped_pa_window(r.u), Some((0, UNTYPED_PA_WINDOW)));
    assert_eq!(r.m.untyped_pa_window(f), None, "Frame không phải Untyped");

    // Untyped thứ hai **cùng** base 0 ⇒ cửa sổ PA chồng Untyped đầu: Frame 4 KiB tại pa 0
    // sẽ chồng Frame sống f ⇒ `AlreadyMapped` ("không hai Frame sống cùng pa").
    let u2 = r.m.add_untyped(0, 4096);
    let cap2 = r.m.root_cap(u2);
    r.m.put_cap(r.c, 10, cap2).expect("cap Untyped 2");
    assert_eq!(
        r.m.retype(slot(r.c, 10), ObjType::Frame, 12, 0),
        Err(KError::AlreadyMapped),
        "hai Frame sống không được chồng pa"
    );
    assert_eq!(r.m.frame_count(), 1);
    r.kiem("chồng cửa sổ PA bị từ chối");

    // Untyped thứ ba ở cửa sổ kế (base = 1 GiB) ⇒ cấp được, pa rời nhau.
    let u3 = r.m.add_untyped(G1, 4096);
    let cap3 = r.m.root_cap(u3);
    r.m.put_cap(r.c, 11, cap3).expect("cap Untyped 3");
    let cap =
        r.m.retype(slot(r.c, 11), ObjType::Frame, 12, 0)
            .expect("4 KiB ở cửa sổ 2");
    let g = r.m.cap_info(cap).expect("cap").obj;
    r.m.put_cap(r.c, 12, cap).expect("cap Frame cửa sổ 2");
    assert_eq!(r.m.frame_address(g).expect("pa"), G1);
    r.kiem("hai cửa sổ rời nhau");
}

// ===========================================================================
// 10. 10.000 lần retype + huỷ (KT-36 DoD 4) — biên cửa sổ, không rò, không ăn RSS.
//
// Làm theo **vòng**: mỗi vòng cấp `MOI_VONG` Frame 4 KiB rồi `Revoke` Untyped (mọi Frame chết,
// watermark cửa sổ về 0). Tổng = 10.000 cấp + 10.000 huỷ, mà số object sống luôn ≤ `MOI_VONG`
// nên bất biến sau **mỗi** op vẫn kiểm được với chi phí O(MOI_VONG) (kiểm 10.000 object sống
// cùng lúc sẽ là O(n²) — không cần cho DoD và làm cổng chậm vô ích).
// ===========================================================================

#[cfg(not(miri))]
const VONG_10K: usize = 100;
#[cfg(miri)]
const VONG_10K: usize = 2;
#[cfg(not(miri))]
const MOI_VONG: usize = 100;
#[cfg(miri)]
const MOI_VONG: usize = 32;

#[test]
fn muoi_nghin_frame_retype_va_huy() {
    // Mỗi vòng một `Machine` mới: bất biến chạy **sau mỗi op** của mô hình có chi phí
    // O(số object/cap **đã từng** cấp), nên dồn 10.000 Frame vào **một** mô hình sẽ là O(n²)
    // (~56 s) mà không kiểm thêm được gì. Số object sống mỗi vòng ≤ `MOI_VONG`.
    let mut tong = 0usize;
    for vong in 0..VONG_10K {
        let mut m = Machine::new();
        let c0 = m.add_cnode(2);
        let u0 = m.add_untyped(0, 4096);
        let cap = m.root_cap(u0);
        m.put_cap(c0, 0, cap).expect("cap Untyped");
        let src = slot(c0, 0);
        let mut objs = Vec::with_capacity(MOI_VONG);
        for i in 0..MOI_VONG {
            let cap = m
                .retype(src, ObjType::Frame, 12, 0)
                .unwrap_or_else(|e| panic!("vòng {vong} retype #{i}: {e:?}"));
            let obj = m.cap_info(cap).expect("cap").obj;
            // `pa` tuyến tính trong cửa sổ, đúng căn 4 KiB.
            assert_eq!(
                m.frame_address(obj).expect("pa"),
                (i as u64) * K4,
                "vòng {vong}: pa của frame #{i} phải tuyến tính"
            );
            // Cap vào slot 1 rồi xoá ngay: object **vẫn sống** (chủ sở hữu là Untyped), cây cap
            // không phình và `check_caps` vẫn đúng.
            m.put_cap(c0, 1, cap).expect("đặt cap Frame");
            m.delete(slot(c0, 1)).expect("xoá cap Frame");
            objs.push(obj);
            tong += 1;
        }
        assert_eq!(m.frame_count(), MOI_VONG);
        assert_eq!(m.untyped_used(u0), 0, "Frame vẫn 0 byte arena");
        m.check_frames()
            .unwrap_or_else(|e| panic!("vòng {vong}: check_frames: {e}"));
        m.check_caps()
            .unwrap_or_else(|e| panic!("vòng {vong}: check_caps: {e}"));
        m.check_all_bytes()
            .unwrap_or_else(|e| panic!("vòng {vong}: check_bytes: {e}"));

        // Huỷ: mọi Frame chết, bộ đếm sạch, watermark cửa sổ về 0.
        m.revoke(c0, Some(0)).expect("revoke Untyped");
        assert_eq!(m.frame_count(), 0);
        for f in &objs {
            let meta = m.frame_meta(*f).expect("meta");
            assert!(!meta.live);
            assert!(meta.counters_zero(), "frame chết phải sạch mapping");
        }
        // Sau huỷ, cấp lại bắt đầu ở `pa = 0` (watermark cửa sổ đã reset).
        let cap = m.retype(src, ObjType::Frame, 12, 0).expect("cấp lại");
        let obj = m.cap_info(cap).expect("cap").obj;
        assert_eq!(m.frame_address(obj).expect("pa"), 0, "vòng {vong}");
        m.check_frames().expect("sau huỷ: check_frames");
    }
    assert_eq!(tong, VONG_10K * MOI_VONG, "đủ 10.000 lần retype + huỷ");
}

// ===========================================================================
// 11. Property **10.000 seed** — `check_frames` không thấy Frame mồ côi/chồng pa (DoD 5).
// ===========================================================================

#[cfg(not(miri))]
const PROP_SEEDS: u64 = 10_000;
#[cfg(miri)]
const PROP_SEEDS: u64 = 5;
#[cfg(not(miri))]
const PROP_OPS: u64 = 8;
#[cfg(miri)]
const PROP_OPS: u64 = 4;

/// Trạng thái fuzz: mô hình + **sổ mapping đã tạo qua hook** (để `unmap` luôn dùng đúng bộ cờ
/// của lần `map` tương ứng — mô hình không tự nhớ quyền của mapping; KT-37 sẽ truyền quyền thật).
struct Fuzz {
    m: Machine,
    c: ObjId,
    unt: Vec<(SlotRef, ObjId)>,
    tu: usize,
    maps: Vec<(ObjId, bool, bool)>,
    lcg: Lcg,
}

impl Fuzz {
    fn new(seed: u64) -> Self {
        let mut m = Machine::with_arena_bytes(64 << 10);
        let c = m.add_cnode(64);
        let mut unt = Vec::new();
        for (i, base) in [0u64, G1, 2 * G1].iter().enumerate() {
            let u = m.add_untyped(*base, 4096);
            let cap = m.root_cap(u);
            m.put_cap(c, i, cap).expect("cap Untyped");
            unt.push((slot(c, i), u));
        }
        Self {
            m,
            c,
            unt,
            tu: 3,
            maps: Vec::new(),
            lcg: Lcg::new(seed),
        }
    }

    /// Đặt cap vừa Retype vào ô trống đầu tiên (mô hình bắt cap con **phải** nằm trong slot).
    fn dat_cap(&mut self, cap: u32) {
        let mut s = self.tu;
        loop {
            match self.m.cap_at(self.c, s) {
                Err(KError::SlotEmpty) => {
                    self.m.put_cap(self.c, s, cap).expect("đặt cap Frame");
                    self.tu = s + 1;
                    return;
                }
                Ok(_) => s += 1,
                Err(e) => panic!("CNode hết slot cho cap Frame: {e:?}"),
            }
        }
    }

    /// Một bước ngẫu nhiên tất định; trả "dấu vết" ngắn để kiểm tính tất định.
    fn buoc(&mut self) -> u64 {
        let ui = self.lcg.below(self.unt.len() as u64) as usize;
        let (src, u) = self.unt[ui];
        let chon = self.lcg.below(100);
        let loai = self.lcg.below(100);
        let mut dau = 0u64;
        match chon {
            0..=44 => {
                // Retype theo cỡ ngẫu nhiên; `Alignment`/`NotEnoughMemory`/`AlreadyMapped` là
                // **hợp lệ** với mô hình (thứ tự cấp + cửa sổ + chồng pa), không được panic.
                let bits = [12u32, 21, 30][(loai % 3) as usize];
                if let Ok(cap) = self.m.retype(src, ObjType::Frame, bits, 0) {
                    dau = self.m.cap_info(cap).expect("cap").obj as u64;
                    self.dat_cap(cap);
                }
            }
            45..=64 => {
                let fr = self.m.untyped_frames(u);
                if !fr.is_empty() {
                    let f = fr[(loai as usize) % fr.len()];
                    let (w, x) = (loai.is_multiple_of(2), loai.is_multiple_of(3));
                    if self.m.frame_note_map(f, w, x).is_ok() {
                        self.maps.push((f, w, x));
                    }
                    dau = f as u64;
                }
            }
            65..=84 => {
                if !self.maps.is_empty() {
                    let k = (loai as usize) % self.maps.len();
                    let (f, w, x) = self.maps[k];
                    if self.m.frame_note_unmap(f, w, x).is_ok() {
                        self.maps.remove(k);
                    }
                    dau = f as u64;
                }
            }
            _ => {
                // Revoke Untyped: chỉ thành công khi mọi Frame trong đó sạch mapping.
                if self.m.revoke(self.c, Some(src.slot)).is_ok() {
                    // Frame của Untyped vừa revoke đã chết ⇒ không còn mapping sống trong sổ.
                    let con_song = self.m.untyped_frames(u);
                    self.maps.retain(|(f, _, _)| !con_song.contains(f));
                }
                dau = self.m.frame_count() as u64;
            }
        }
        // Bất biến sau **mỗi** bước.
        self.m
            .check_frames()
            .unwrap_or_else(|e| panic!("check_frames: {e}"));
        self.m
            .check_caps()
            .unwrap_or_else(|e| panic!("check_caps: {e}"));
        self.m
            .check_all_bytes()
            .unwrap_or_else(|e| panic!("check_bytes: {e}"));
        self.m.frame_count() as u64 + dau
    }
}

#[test]
fn check_frames_khong_mo_coi_10k_seed() {
    for seed in 0..PROP_SEEDS {
        let mut f = Fuzz::new(seed);
        let mut vet = Vec::with_capacity(PROP_OPS as usize);
        for buoc in 0..PROP_OPS {
            vet.push(f.buoc());
            // Bất biến được kiểm trong `Fuzz::buoc`; panic ở đó sẽ kèm số bước.
            let _ = buoc;
        }
        // Cùng seed ⇒ cùng vết (mô hình tất định).
        let mut g = Fuzz::new(seed);
        let mut vet2 = Vec::with_capacity(PROP_OPS as usize);
        for _ in 0..PROP_OPS {
            vet2.push(g.buoc());
        }
        assert_eq!(vet, vet2, "seed {seed}: hai lượt phải cùng vết");
    }
}
