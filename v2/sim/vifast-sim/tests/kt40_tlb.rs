// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-40 — **TLB model 16 entry/hart + shootdown `TlbFlush` + ack thế hệ + chống deadlock**
//! (K2-PLAN §3 KT-40 + §2 Q4/Q5; D2-65 Q4/Q5; `v2/docs/k2-vm.md` §14; thiết kế §4.10
//! `:353-366`, §9.5 `:799`).
//!
//! KT-38 đã có **lệnh flush** (`FlushCmd`) và KT-39 đã có `protect`; KT-40 dựng **TLB** thật
//! của mô hình và **tiêu thụ** đường flush đó:
//!
//! 1. mỗi hart một TLB **16 entry** ([`TLB_ENTRIES_PER_HART`]), khoá `(asid, gen, va)`; thay
//!    trang theo **LRU tất định theo chỉ số**; đếm `hit`/`miss`/`fill`/`evict`;
//! 2. `unmap`/`protect` sửa PTE ⇒ **flush cục bộ** + gửi `TlbFlush{asid, va, gen, seq}` vào hộp
//!    thư **mọi hart trong `active_on`** (model của IPI); chỉ trả `Ok` khi **ack đủ**
//!    (`acked_mask == active_on`); ack là **thế hệ** (`fetch_max`, `acked ≠ delivered`);
//! 3. hart chờ ack **phải rút hộp thư của mình** (luật chống deadlock, thiết kế §4.10.3 — đúng
//!    lỗ AP→BSP của v1); test mô phỏng ack đến từ hart **đang chờ**;
//! 4. oracle **TLB-safety**: mọi entry còn sống phải khớp cây PageTable hiện tại ⇒ 0 vi phạm
//!    trên property 10.000 vòng.
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt40-red-compile.log`](../../agent/JOURNAL/kt40-red-compile.log)
//! — API KT-40 chưa có (E0432 `TlbFlushScope/TlbHartInfo/...` + E0599 `tlb_translate/tlb_*`) ⇒
//! `could not compile`.
//!
//! **K2 không phải kernel thật**: không `sfence.vma`, không IPI/TLB phần cứng; "IPI" là một
//! message trong hộp thư mô hình, "ack" là bộ đếm thế hệ trong mô hình (D2-65 Q5).

use vifast_sim::kobj::{
    HartMask, KError, Machine, MapAttrs, MapRights, ObjId, ObjType, SlotRef, TlbFlushScope,
    TLB_ENTRIES_PER_HART,
};
use vifast_sim::Lcg;

const K4: u64 = 1 << 12;

// ===========================================================================
// Rig: CNode 1024 slot + Untyped cho root/thân cây PageTable + Untyped Frame
// (cửa sổ PA 1 GiB; Frame **không** tiêu byte arena — KT-36).
// ===========================================================================

struct Rig {
    m: Machine,
    c: ObjId,
    ufr: ObjId,
    sva: SlotRef,
    next: usize,
}

impl Rig {
    fn new(harts: u16, va_bytes: usize) -> Self {
        let mut m = Machine::with_harts(harts);
        let c = m.add_cnode(1024);
        let uva = m.add_untyped(0, va_bytes);
        let ufr = m.add_untyped(1 << 32, 4096);
        for (slot, u) in [(0usize, uva), (1, ufr)] {
            let cap = m.root_cap(u);
            m.put_cap(c, slot, cap).expect("cap Untyped");
        }
        Self {
            m,
            c,
            ufr,
            sva: SlotRef::new(c, 0),
            next: 2,
        }
    }

    fn slot(&mut self) -> usize {
        let s = self.next;
        self.next += 1;
        s
    }

    fn put(&mut self, cap: vifast_sim::kobj::CapId) -> SlotRef {
        let s = self.slot();
        self.m.put_cap(self.c, s, cap).expect("đặt cap vào CNode");
        SlotRef::new(self.c, s)
    }

    /// Tạo VSpace bằng **model API** `vspace_create` + đặt cap; trả `(slot, ObjId)`.
    fn vspace(&mut self) -> (SlotRef, ObjId) {
        let cap = self.m.vspace_create(self.sva).expect("vspace_create");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Retype Frame 4 KiB + đặt cap; trả `(slot, ObjId)`.
    fn frame(&mut self) -> (SlotRef, ObjId) {
        let cap = self
            .m
            .retype(self.sfr(), ObjType::Frame, 12, 0)
            .expect("retype Frame 4 KiB");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    fn sfr(&self) -> SlotRef {
        SlotRef::new(self.c, 1)
    }

    /// Dựng chuỗi root → mid (cấp 1) → leaf (cấp 0) phủ 2 MiB đầu: đủ cho 512 trang 4 KiB.
    /// Trả `(mid, leaf)` để test teardown **detach** cây (root phải tinh khiết trước Revoke).
    fn cay_4k(&mut self, vs: ObjId) -> (ObjId, ObjId) {
        let root = self.m.vspace_root(vs).expect("root VSpace");
        let (_, mid) = self.tbl();
        self.m.page_table_attach(root, 0, mid).expect("attach mid");
        let (_, leaf) = self.tbl();
        self.m.page_table_attach(mid, 0, leaf).expect("attach leaf");
        (mid, leaf)
    }

    fn tbl(&mut self) -> (SlotRef, ObjId) {
        let cap = self
            .m
            .retype(self.sva, ObjType::PageTable, 0, 0)
            .expect("retype PageTable");
        let obj = self.m.cap_info(cap).expect("cap vừa cấp").obj;
        (self.put(cap), obj)
    }

    /// Cấp Frame 4 KiB mới rồi map vào `va` (quyền `READ | WRITE`, `WB`) qua VSpace.
    fn map_page(&mut self, vs: ObjId, va: u64) -> ObjId {
        let (_, f) = self.frame();
        self.m
            .vspace_map_obj(vs, va, f, MapRights::READ | MapRights::WRITE, MapAttrs::WB)
            .expect("map 4 KiB");
        f
    }

    /// Bất biến sau mỗi op: VSpace/ASID/PageTable/Frame/W^X/TLB/cap/byte.
    fn kiem(&self, nhan: &str) {
        for (ten, kq) in [
            ("check_vspace", self.m.check_vspace()),
            ("check_asid", self.m.check_asid()),
            ("check_pages", self.m.check_pages()),
            ("check_frames", self.m.check_frames()),
            ("check_wx", self.m.check_wx()),
            ("check_tlb", self.m.check_tlb()),
            ("check_caps", self.m.check_caps()),
            ("check_bytes", self.m.check_all_bytes()),
        ] {
            kq.unwrap_or_else(|e| panic!("{nhan}: {ten}: {e}"));
        }
        assert_eq!(
            self.m.untyped_used(self.ufr),
            0,
            "{nhan}: Frame không tiêu byte arena"
        );
    }
}

fn va(i: usize) -> u64 {
    K4 * (i as u64 + 1)
}

// ===========================================================================
// 1. TLB 16 entry/hart, LRU tất định, hit/miss đếm được.
// ===========================================================================

#[test]
fn tlb_16_entry_moi_hart_lru_tat_dinh_hit_miss() {
    assert_eq!(TLB_ENTRIES_PER_HART, 16, "K2-PLAN §3 KT-40: 16 entry/hart");
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    r.m.vspace_activate(vs, 0).expect("activate 0");
    r.m.vspace_activate(vs, 1).expect("activate 1");
    let _ = r.cay_4k(vs);
    for i in 0..18 {
        r.map_page(vs, va(i));
    }
    r.m.set_current_hart(0).expect("hart 0");
    // Nạp 16 entry đầu (mỗi trang một lần ghi TLB) — **miss** hết.
    for i in 0..TLB_ENTRIES_PER_HART {
        let t =
            r.m.tlb_translate(vs, va(i))
                .expect("translate trang đã map");
        assert!(!t.from_tlb, "lần đầu là miss");
        assert_eq!(t.size_bits, 12);
    }
    let info = r.m.tlb_info(0).expect("tlb_info 0");
    assert_eq!(info.capacity, TLB_ENTRIES_PER_HART);
    assert_eq!(info.entries, TLB_ENTRIES_PER_HART, "đầy 16 entry");
    assert_eq!(info.misses, TLB_ENTRIES_PER_HART as u64);
    assert_eq!(info.fills, TLB_ENTRIES_PER_HART as u64);
    assert_eq!(info.hits, 0);
    assert_eq!(r.m.tlb_entries(0), Ok(TLB_ENTRIES_PER_HART));

    // Hit: cùng trang lần hai.
    let t = r.m.tlb_translate(vs, va(0)).expect("hit");
    assert!(t.from_tlb, "entry đã có ⇒ hit");
    assert_eq!(r.m.tlb_info(0).expect("info").hits, 1);

    // LRU tất định: `va(0)` vừa hit thành MRU; nạp trang 17 ⇒ đuổi `va(1)` (LRU cũ nhất),
    // KHÔNG đuổi `va(0)`.
    let t = r.m.tlb_translate(vs, va(16)).expect("trang 17");
    assert!(!t.from_tlb);
    let info = r.m.tlb_info(0).expect("info");
    assert_eq!(info.entries, TLB_ENTRIES_PER_HART, "không vượt 16");
    assert_eq!(info.evictions, 1);
    assert!(
        r.m.tlb_lookup(0, vs, va(1)).expect("lookup").is_none(),
        "LRU phải đuổi trang cũ nhất (va(1)), không phải trang vừa hit"
    );
    assert!(r.m.tlb_lookup(0, vs, va(0)).expect("lookup").is_some());
    assert!(r.m.tlb_lookup(0, vs, va(16)).expect("lookup").is_some());

    // TLB **theo hart**: hart 1 chưa từng dịch trang nào ⇒ rỗng (entry hart 0 không lẫn).
    assert_eq!(r.m.tlb_entries(1), Ok(0));
    r.m.set_current_hart(1).expect("hart 1");
    let t = r.m.tlb_translate(vs, va(0)).expect("miss trên hart 1");
    assert!(!t.from_tlb, "hart 1 phải miss (TLB theo hart)");
    assert_eq!(r.m.tlb_entries(1), Ok(1));
    assert_eq!(r.m.tlb_info(1).expect("info").misses, 1);
    r.kiem("16 entry/LRU");
}

// ===========================================================================
// 2. Shootdown 1/2/4/8 hart: unmap chỉ `Ok` khi **ack đủ** (`acked == active_on`);
//    entry trang bị unmap mất trên **mọi** hart, trang khác còn.
// ===========================================================================

#[test]
fn shootdown_1_2_4_8_hart_ack_du() {
    for n in [1u16, 2, 4, 8] {
        let mut r = Rig::new(n, 2 * 1024 * 1024);
        let (_, vs) = r.vspace();
        for h in 0..n {
            r.m.vspace_activate(vs, h).expect("activate");
        }
        let _ = r.cay_4k(vs);
        let f1 = r.map_page(vs, va(0));
        r.map_page(vs, va(1));
        // Mỗi hart đều có entry cho cả hai trang (chạm từ hart của nó).
        for h in 0..n {
            r.m.set_current_hart(h).expect("hart");
            assert!(r.m.tlb_translate(vs, va(0)).is_ok());
            assert!(r.m.tlb_translate(vs, va(1)).is_ok());
            assert_eq!(r.m.tlb_entries(h), Ok(2));
        }
        // Unmap trên hart 0 ⇒ shootdown tới **mọi** hart trong `active_on` (= 0..n).
        r.m.set_current_hart(0).expect("hart 0");
        let got = r.m.vspace_unmap(vs, va(0)).expect("unmap phải trả Ok");
        assert_eq!(got.frame, f1);
        assert_eq!(got.va, va(0));
        let s =
            r.m.last_tlb_shootdown()
                .unwrap_or_else(|| panic!("{n} hart: phải có shootdown"));
        assert!(s.completed, "{n} hart: shootdown phải hoàn tất");
        assert_eq!(s.targets, vifast_sim::kobj::HartMask::for_harts(n));
        assert_eq!(
            s.acked, s.targets,
            "{n} hart: `Ok` chỉ khi ack_mask == active_on"
        );
        assert_eq!(s.asid, r.m.vspace_get_asid(vs).expect("asid"));
        assert_eq!(s.va, va(0));
        // Trang bị unmap **mất** trên mọi hart; trang khác **còn**.
        for h in 0..n {
            assert!(
                r.m.tlb_lookup(h, vs, va(0)).expect("lookup").is_none(),
                "{n} hart: entry trang bị unmap còn trên hart {h}"
            );
            assert!(
                r.m.tlb_lookup(h, vs, va(1)).expect("lookup").is_some(),
                "{n} hart: entry trang khác bị xoá oan trên hart {h}"
            );
            // `delivered` (đã gửi) == `acked` (đã xử lý) sau khi chờ xong.
            let info = r.m.tlb_info(h).expect("info");
            assert_eq!(
                info.acked, info.delivered,
                "{n} hart: hart {h} còn TlbFlush chưa ack"
            );
        }
        // Dùng entry cũ để dịch ⇒ **hụt** (miss) rồi walk báo `NotMapped` (không trả PA cũ).
        assert_eq!(
            r.m.tlb_translate(vs, va(0)),
            Err(KError::NotMapped),
            "{n} hart: entry TLB cũ phải bị vô hiệu, không dịch lại PA đã thu hồi"
        );
        r.kiem("shootdown đủ ack");
    }
}

// ===========================================================================
// 3. Ack thiếu ⇒ `Ok` **chưa** trả: `begin` chỉ giao message, `wait` mới trả Ok.
// ===========================================================================

#[test]
fn ack_thieu_ok_chua_tra() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    for h in 0..2 {
        r.m.vspace_activate(vs, h).expect("activate");
    }
    let _ = r.cay_4k(vs);
    r.map_page(vs, va(0));
    for h in 0..2 {
        r.m.set_current_hart(h).expect("hart");
        r.m.tlb_translate(vs, va(0)).expect("fill");
    }
    r.m.set_current_hart(0).expect("hart 0");
    let seq = r.m.tlb_shootdown_begin(0, vs, va(0)).expect("begin");
    assert_eq!(seq, 1, "seq shootdown bắt đầu từ 1");
    let p = r.m.tlb_pending(0).expect("pending").expect("đang chờ ack");
    assert!(!p.completed);
    assert_ne!(
        p.acked, p.targets,
        "ack chưa đủ ⇒ `Ok` chưa được trả (begin mới chỉ giao message)"
    );
    assert_eq!(
        p.messages, 1,
        "không gửi cho hart gọi (flush cục bộ + ack ngầm): chỉ hart 1 nhận TlbFlush"
    );
    assert_eq!(r.m.mail_len(0), Ok(0), "hart gọi không tự IPI mình");
    assert_eq!(
        r.m.mail_len(1),
        Ok(1),
        "message nằm trong hộp thư, chưa xử lý"
    );
    // Chờ tới khi đủ ack mới `Ok`.
    r.m.tlb_shootdown_wait(0).expect("wait đủ ack");
    assert!(r.m.tlb_pending(0).expect("pending").is_none());
    let s = r.m.last_tlb_shootdown().expect("shootdown");
    assert!(s.completed);
    assert_eq!(s.seq, seq);
    assert_eq!(s.acked, s.targets);
    assert_eq!(r.m.mail_len(0), Ok(0), "hộp thư đã rút cạn");
    assert_eq!(r.m.mail_len(1), Ok(0));
    // `wait` không còn gì ⇒ no-op (không treo, không lỗi).
    r.m.tlb_shootdown_wait(0).expect("wait lặp là no-op");
    r.m.tlb_shootdown_wait(1)
        .expect("hart 1 không có gì phải chờ");
    r.kiem("ack đủ mới Ok");
}

// ===========================================================================
// 4. Flush theo `(asid, va)` / `asid` / `gen`.
// ===========================================================================

#[test]
fn flush_theo_trang_asid_va_gen() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    for h in 0..2 {
        r.m.vspace_activate(vs, h).expect("activate");
    }
    let _ = r.cay_4k(vs);
    r.map_page(vs, va(0));
    r.map_page(vs, va(1));
    for h in 0..2 {
        r.m.set_current_hart(h).expect("hart");
        r.m.tlb_translate(vs, va(0)).expect("fill 0");
        r.m.tlb_translate(vs, va(1)).expect("fill 1");
    }
    // (a) flush theo **một trang**: chỉ entry phủ `va(0)` của hart 0 mất.
    let n =
        r.m.tlb_flush_local(0, vs, TlbFlushScope::Page { va: va(0) })
            .expect("flush page");
    assert_eq!(n, 1);
    assert!(r.m.tlb_lookup(0, vs, va(0)).expect("lookup").is_none());
    assert!(r.m.tlb_lookup(0, vs, va(1)).expect("lookup").is_some());
    assert!(r.m.tlb_lookup(1, vs, va(0)).expect("lookup").is_some());
    // (b) flush theo **ASID** trên hart 0: cả hai trang của hart 0 mất, hart 1 còn nguyên.
    let n =
        r.m.tlb_flush_local(0, vs, TlbFlushScope::Asid)
            .expect("flush asid");
    assert_eq!(n, 1, "chỉ còn entry trang 1 trên hart 0");
    assert_eq!(r.m.tlb_entries(0), Ok(0));
    assert_eq!(r.m.tlb_entries(1), Ok(2));
    // (c) `vspace_deactivate` = hart rời VSpace ⇒ TLB của chính hart đó bị xoá (Asid);
    // trang bên hart khác không bị đụng.
    r.m.vspace_deactivate(vs, 1).expect("deactivate");
    assert_eq!(r.m.tlb_entries(1), Ok(0));
    assert_eq!(r.m.tlb_entries(0), Ok(0));
    r.kiem("flush trang/asid");

    // (d) **flush theo `gen`** khi cạn ASID: mọi entry của thế hệ cũ mất hiệu lực.
    #[cfg(miri)]
    {
        // Dưới Miri: 257 VSpace quá đắt; ngữ nghĩa `gen` được kiểm ở unit test rẻ trong
        // `src/kobj.rs` (`check_tlb_*`) — không `#[ignore]`, chỉ giảm ca (D2-47(a)).
        return;
    }
    #[cfg(not(miri))]
    {
        let mut r = Rig::new(1, 2 * 1024 * 1024);
        let (_, vs_old) = r.vspace();
        r.m.vspace_activate(vs_old, 0).expect("activate");
        let _ = r.cay_4k(vs_old);
        r.map_page(vs_old, va(0));
        r.m.tlb_translate(vs_old, va(0)).expect("fill thế hệ 0");
        assert_eq!(r.m.tlb_entries(0), Ok(1));
        assert_eq!(r.m.asid_state().generation, 0);
        // Cấp đủ 256 ASID còn lại ⇒ lần kế tiếp cạn ⇒ `generation += 1` + flush theo gen.
        for _ in 0..256 {
            r.vspace();
        }
        assert_eq!(r.m.asid_state().generation, 1, "cạn ASID ⇒ thế hệ mới");
        assert_eq!(
            r.m.tlb_entries(0),
            Ok(0),
            "flush theo `gen` phải vô hiệu **mọi** entry của thế hệ cũ"
        );
        assert!(
            r.m.tlb_lookup(0, vs_old, va(0)).expect("lookup").is_none(),
            "entry thế hệ 0 không được dùng lại sau khi thế hệ tăng"
        );
        // Nạp lại sau flush: trang vẫn còn map ⇒ miss rồi fill lại (gen cũ của VSpace cũ).
        let t = r.m.tlb_translate(vs_old, va(0)).expect("translate lại");
        assert!(!t.from_tlb, "sau flush gen, lần dịch kế là miss");
        assert_eq!(r.m.tlb_entries(0), Ok(1));
        r.kiem("flush gen");
    }
}

// ===========================================================================
// 5. `protect`: quyền mới phải hiện trên **mọi** hart (entry quyền cũ bị vô hiệu).
// ===========================================================================

#[test]
fn protect_cap_nhat_quyen_tren_moi_hart() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    for h in 0..2 {
        r.m.vspace_activate(vs, h).expect("activate");
    }
    let _ = r.cay_4k(vs);
    let (_, f) = r.frame();
    r.m.vspace_map_obj(vs, va(0), f, MapRights::READ, MapAttrs::WB)
        .expect("map READ");
    for h in 0..2 {
        r.m.set_current_hart(h).expect("hart");
        let t = r.m.tlb_translate(vs, va(0)).expect("fill");
        assert_eq!(t.rights, MapRights::READ);
    }
    r.m.set_current_hart(0).expect("hart 0");
    let m =
        r.m.vspace_protect(vs, va(0), MapRights::READ | MapRights::WRITE, MapAttrs::WB)
            .expect("protect W");
    assert_eq!(m.rights, MapRights::READ | MapRights::WRITE);
    let s = r.m.last_tlb_shootdown().expect("shootdown protect");
    assert!(s.completed);
    assert_eq!(s.acked, s.targets);
    // Entry quyền cũ (READ) **không** được dùng lại trên bất kỳ hart nào.
    for h in 0..2 {
        assert!(
            r.m.tlb_lookup(h, vs, va(0)).expect("lookup").is_none(),
            "protect phải vô hiệu entry quyền cũ trên hart {h}"
        );
    }
    // Dịch lại: miss + quyền **mới** (không trả quyền cũ).
    let t = r.m.tlb_translate(vs, va(0)).expect("translate");
    assert!(!t.from_tlb, "entry cũ bị flush ⇒ miss");
    assert_eq!(t.rights, MapRights::READ | MapRights::WRITE);
    r.kiem("protect shootdown");
}

// ===========================================================================
// 6. Chống deadlock: hart **đang chờ** ack vẫn phải rút hộp thư của mình để trả
//    ack cho hart khác (tái hiện AP→BSP của v1) — không treo, không quá hạn.
// ===========================================================================

#[test]
fn deadlock_ack_tu_hart_dang_cho() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    for h in 0..2 {
        r.m.vspace_activate(vs, h).expect("activate");
    }
    let _ = r.cay_4k(vs);
    r.map_page(vs, va(0));
    r.map_page(vs, va(1));
    for h in 0..2 {
        r.m.set_current_hart(h).expect("hart");
        r.m.tlb_translate(vs, va(0)).expect("fill 0");
        r.m.tlb_translate(vs, va(1)).expect("fill 1");
    }
    // Hart 1 **bắt đầu** shootdown rồi "đang chờ" ack (pending, chưa wait) — mô phỏng
    // một hart bị chặn trong vòng chờ ack (đúng ngữ cảnh AP→BSP).
    let seq1 = r.m.tlb_shootdown_begin(1, vs, va(0)).expect("begin hart 1");
    assert!(r.m.tlb_pending(1).expect("pending").is_some());
    // Hart 0 unmap `va(0)`: vòng chờ của hart 0 phải vừa **rút hộp thư của mình** (để trả ack
    // cho hart 1) vừa cho hart 1 chạy handler (để hart 1 ack mình) ⇒ cả hai hoàn tất.
    r.m.set_current_hart(0).expect("hart 0");
    let s0 = r.m.vspace_unmap(vs, va(0)).expect("unmap không được treo");
    assert_eq!(s0.va, va(0));
    let last = r.m.last_tlb_shootdown().expect("shootdown hart 0");
    assert!(last.completed);
    assert_eq!(last.acked, last.targets, "hart 0 phải nhận đủ ack");
    // Hart 1 (đang chờ) đã được trả ack **trong lúc chờ**, nên `wait` xong ngay.
    r.m.tlb_shootdown_wait(1).expect("hart 1 hết chờ");
    assert!(r.m.tlb_pending(1).expect("pending").is_none());
    let s1 = r.m.last_tlb_shootdown().expect("shootdown hart 1");
    assert!(s1.completed);
    assert_eq!(s1.seq, seq1);
    assert_eq!(
        s1.acked, s1.targets,
        "ack của hart 1 đến từ hart đang chờ (0)"
    );
    // Cả hai hart đều không còn entry cho trang vừa unmap; trang kia còn.
    for h in 0..2 {
        assert!(r.m.tlb_lookup(h, vs, va(0)).expect("lookup").is_none());
        assert!(r.m.tlb_lookup(h, vs, va(1)).expect("lookup").is_some());
        let info = r.m.tlb_info(h).expect("info");
        assert_eq!(info.acked, info.delivered);
    }
    r.kiem("deadlock-order");
}

// ===========================================================================
// 7. Lỗi biên: hart không chạy VSpace, hart/vspace không có thật, `Busy` khi đã có
//    shootdown đang chờ.
// ===========================================================================

#[test]
fn loi_bien_hart_va_vspace() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    let (_, vs2) = r.vspace();
    r.m.vspace_activate(vs, 0).expect("activate");
    let (mid, _leaf) = r.cay_4k(vs);
    r.map_page(vs, va(0));
    // Hart 1 **không** chạy `vs` ⇒ không dịch được (Touch chỉ có nghĩa khi hart nạp ASID).
    r.m.set_current_hart(1).expect("hart 1");
    assert_eq!(
        r.m.tlb_translate(vs, va(0)),
        Err(KError::IllegalOperation),
        "hart không chạy VSpace ⇒ IllegalOperation"
    );
    // VA chưa map ⇒ miss + walk trả NotMapped (không có entry rác).
    r.m.set_current_hart(0).expect("hart 0");
    assert_eq!(r.m.tlb_translate(vs, va(7)), Err(KError::NotMapped));
    let info = r.m.tlb_info(0).expect("info");
    assert_eq!(info.entries, 0, "walk lỗi ⇒ không nạp entry");
    assert_eq!(info.misses, 1);
    // Hart/VSpace không có thật.
    assert_eq!(r.m.tlb_info(2), Err(KError::InvalidArgument));
    assert_eq!(r.m.tlb_entries(99), Err(KError::InvalidArgument));
    assert_eq!(r.m.tlb_lookup(2, vs, va(0)), Err(KError::InvalidArgument));
    assert_eq!(
        r.m.tlb_translate(999, va(0)),
        Err(KError::InvalidCapability),
        "VSpace không tồn tại ⇒ InvalidCapability"
    );
    assert_eq!(
        r.m.tlb_flush_local(2, vs, TlbFlushScope::Asid),
        Err(KError::InvalidArgument)
    );
    assert_eq!(
        r.m.tlb_shootdown_begin(2, vs, va(0)),
        Err(KError::InvalidArgument)
    );
    // `active_on` rỗng ⇒ **không gửi** message nào (DoD), begin/wait vẫn xong ngay.
    let seq =
        r.m.tlb_shootdown_begin(0, vs2, va(0))
            .expect("active_on rỗng vẫn begin được");
    assert_eq!(r.m.mail_len(0), Ok(0), "active_on rỗng ⇒ không gửi gì");
    assert_eq!(r.m.mail_len(1), Ok(0));
    // Đã có shootdown đang chờ ⇒ begin thứ hai `Busy` (không ghi đè trạng thái chờ).
    assert_eq!(r.m.tlb_shootdown_begin(0, vs2, va(0)), Err(KError::Busy));
    r.m.tlb_shootdown_wait(0).expect("wait shootdown rỗng");
    let s = r.m.last_tlb_shootdown().expect("shootdown rỗng");
    assert_eq!(s.seq, seq);
    assert!(s.targets.is_empty(), "active_on rỗng ⇒ targets rỗng");
    assert_eq!(s.acked, s.targets);
    // VSpace đã chết (Revoke Untyped chứa nó ⇒ finalise VSpace) ⇒ thao tác TLB trả lỗi rõ,
    // không panic và không dùng asid/gen của object đã chết.
    let cap = r.m.vspace_create(r.sva).expect("vs tạm");
    let vs3 = r.m.cap_info(cap).expect("cap").obj;
    let _ = r.put(cap);
    // Root của `vs` phải tinh khiết trước khi revoke Untyped (KT-38: `HasChildren`).
    r.m.vspace_unmap(vs, va(0)).expect("unmap trước revoke");
    r.m.page_table_detach(mid, 0).expect("detach leaf");
    let root = r.m.vspace_root(vs).expect("root");
    r.m.page_table_detach(root, 0).expect("detach mid");
    r.m.revoke(r.c, Some(0))
        .expect("Revoke Untyped chứa VSpace");
    assert_eq!(
        r.m.tlb_flush_local(0, vs3, TlbFlushScope::Asid),
        Err(KError::InvalidCapability),
        "VSpace đã chết ⇒ không có asid/gen để flush"
    );
    assert_eq!(
        r.m.tlb_translate(vs3, va(0)),
        Err(KError::InvalidCapability)
    );
    assert_eq!(r.m.tlb_shootdown_wait(1), Ok(()), "không có gì chờ ⇒ no-op");
    r.kiem("lỗi biên");
}

// ===========================================================================
// 7b. Hart **gọi không thuộc `active_on`** (unmap VSpace của hart khác): chỉ local flush +
//     shootdown các hart trong mask; ack vẫn đủ và entry của hart đích bị xoá.
// ===========================================================================

#[test]
fn hart_goi_ngoai_active_on_van_shootdown_dung() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace(); // chạy trên hart 1
    let (_, vs2) = r.vspace(); // chạy trên hart 0 (hart gọi)
    r.m.vspace_activate(vs, 1).expect("activate hart 1");
    r.m.vspace_activate(vs2, 0).expect("activate hart 0");
    let _ = r.cay_4k(vs);
    r.map_page(vs, va(0));
    r.m.set_current_hart(1).expect("hart 1");
    r.m.tlb_translate(vs, va(0)).expect("fill trên hart 1");
    assert_eq!(r.m.tlb_entries(1), Ok(1));
    assert_eq!(
        r.m.vspace_active_on(vs),
        Ok(HartMask::from_bits(0b10)),
        "chỉ hart 1 chạy `vs`"
    );
    // Hart 0 gọi unmap cho VSpace **không** chạy trên nó (kernel op hộ): local flush hart 0
    // (không có entry) + `TlbFlush` tới đúng active_on = {1}.
    r.m.set_current_hart(0).expect("hart 0");
    let s =
        r.m.vspace_unmap(vs, va(0))
            .expect("unmap từ hart ngoài active_on");
    assert_eq!(s.va, va(0));
    let info = r.m.last_tlb_shootdown().expect("shootdown");
    assert!(info.completed);
    assert_eq!(info.targets, HartMask::from_bits(0b10));
    assert_eq!(info.acked, info.targets, "chỉ cần ack của hart 1");
    assert_eq!(
        info.messages, 1,
        "một message cho hart 1, không cho hart gọi"
    );
    assert!(r.m.tlb_lookup(1, vs, va(0)).expect("lookup").is_none());
    r.kiem("hart ngoài active_on");
}

// ===========================================================================
// 7c. AP→BSP **tương hỗ** không dựa self-target: 2 hart cùng chờ ack của nhau; mỗi hart chỉ
//     tiến khi **tự rút hộp thư của mình** (`tlb_shootdown_poll`) — thiếu bước đó là deadlock.
// ===========================================================================

#[test]
fn deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu() {
    let mut r = Rig::new(2, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    for h in 0..2 {
        r.m.vspace_activate(vs, h).expect("activate");
    }
    let _ = r.cay_4k(vs);
    r.map_page(vs, va(0));
    r.map_page(vs, va(1));
    for h in 0..2 {
        r.m.set_current_hart(h).expect("hart");
        r.m.tlb_translate(vs, va(0)).expect("fill 0");
        r.m.tlb_translate(vs, va(1)).expect("fill 1");
    }
    // Hai hart **cùng** mở shootdown rồi cùng chờ ack của nhau (không hart nào chạy handler
    // của hart kia giúp — mô phỏng đúng cảnh AP↔BSP cùng chờ).
    r.m.tlb_shootdown_begin(0, vs, va(0)).expect("begin hart 0");
    r.m.tlb_shootdown_begin(1, vs, va(1)).expect("begin hart 1");
    assert!(r.m.tlb_pending(0).expect("pending").is_some());
    assert!(r.m.tlb_pending(1).expect("pending").is_some());
    // Vòng tự phục vụ: mỗi hart rút hộp thư **của chính nó**; ack chéo chỉ xuất hiện khi
    // hart kia tự phục vụ. `poll` **không** cho hart khác "chạy" hộ.
    assert_eq!(
        r.m.tlb_shootdown_poll(1),
        Ok(false),
        "hart 1 tự ack mình nhưng còn chờ ack của hart 0"
    );
    assert_eq!(
        r.m.tlb_shootdown_poll(0),
        Ok(true),
        "hart 0 rút hộp thư của mình (ack hart 1) + đã được hart 1 ack ⇒ xong"
    );
    assert_eq!(
        r.m.tlb_shootdown_poll(1),
        Ok(true),
        "hart 1 nhận ack của hart 0 ⇒ xong"
    );
    assert!(r.m.tlb_pending(0).expect("pending").is_none());
    assert!(r.m.tlb_pending(1).expect("pending").is_none());
    for h in 0..2 {
        assert!(r.m.tlb_lookup(h, vs, va(0)).expect("lookup").is_none());
        assert!(r.m.tlb_lookup(h, vs, va(1)).expect("lookup").is_none());
        let info = r.m.tlb_info(h).expect("info");
        assert_eq!(info.acked, info.delivered);
    }
    r.kiem("mutual poll AP→BSP");
}

// ===========================================================================
// 8. Property 10.000 vòng: map/unmap/protect/translate/activate/deactivate ngẫu
//    nhiên tất định; bất biến TLB-safety **0 vi phạm** sau mỗi vòng.
// ===========================================================================

#[test]
fn property_10k_tlb_shootdown() {
    #[cfg(miri)]
    let (harts, iters) = (2u16, 150usize);
    #[cfg(not(miri))]
    let (harts, iters) = (4u16, 10_000usize);
    const PAGES: usize = 16;
    let mut r = Rig::new(harts, 2 * 1024 * 1024);
    let (_, vs) = r.vspace();
    for h in 0..harts {
        r.m.vspace_activate(vs, h).expect("activate");
    }
    let _ = r.cay_4k(vs);
    let mut frames = Vec::new();
    for i in 0..PAGES {
        frames.push(r.map_page(vs, va(i)));
    }
    let mut mapped = [true; PAGES];
    let mut writable = [true; PAGES];
    let mut rng = Lcg::new(40);
    let mut hits = 0u64;
    let mut misses = 0u64;
    let mut unmaps = 0u64;
    for vong in 0..iters {
        let h = rng.below(harts as u64) as u16;
        r.m.set_current_hart(h).expect("hart");
        match rng.below(9) {
            0 | 1 => {
                let i = rng.below(PAGES as u64) as usize;
                let got = r.m.tlb_translate(vs, va(i));
                if mapped[i] {
                    let t = got.expect("đang map ⇒ translate Ok");
                    assert_eq!(t.rights.contains(MapRights::WRITE), writable[i]);
                    if t.from_tlb {
                        hits += 1;
                    } else {
                        misses += 1;
                    }
                } else {
                    assert_eq!(got, Err(KError::NotMapped));
                    misses += 1;
                }
            }
            2 => {
                let i = rng.below(PAGES as u64) as usize;
                if mapped[i] {
                    let m = r.m.vspace_unmap(vs, va(i)).expect("unmap");
                    assert_eq!(m.va, va(i));
                    mapped[i] = false;
                    unmaps += 1;
                    for hh in 0..harts {
                        assert!(
                            r.m.tlb_lookup(hh, vs, va(i)).expect("lookup").is_none(),
                            "vòng {vong}: entry trang bị unmap còn trên hart {hh}"
                        );
                    }
                } else {
                    assert_eq!(r.m.vspace_unmap(vs, va(i)), Err(KError::NotMapped));
                }
            }
            3 => {
                let i = rng.below(PAGES as u64) as usize;
                if !mapped[i] {
                    let rights = if writable[i] {
                        MapRights::READ | MapRights::WRITE
                    } else {
                        MapRights::READ
                    };
                    r.m.vspace_map_obj(vs, va(i), frames[i], rights, MapAttrs::WB)
                        .expect("map lại");
                    mapped[i] = true;
                }
            }
            4 | 5 => {
                let i = rng.below(PAGES as u64) as usize;
                if mapped[i] {
                    let new_w = !writable[i];
                    let rights = if new_w {
                        MapRights::READ | MapRights::WRITE
                    } else {
                        MapRights::READ
                    };
                    r.m.vspace_protect(vs, va(i), rights, MapAttrs::WB)
                        .expect("protect");
                    writable[i] = new_w;
                    let t = r.m.tlb_translate(vs, va(i)).expect("translate sau protect");
                    assert_eq!(t.rights, rights, "vòng {vong}: quyền cũ còn trong TLB");
                }
            }
            6 => {
                r.m.vspace_deactivate(vs, h).expect("deactivate");
                r.m.vspace_activate(vs, h).expect("activate lại");
            }
            7 => {
                r.m.tlb_flush_local(h, vs, TlbFlushScope::Asid)
                    .expect("flush asid");
            }
            _ => {
                for _ in 0..3 {
                    let i = rng.below(PAGES as u64) as usize;
                    let _ = r.m.tlb_translate(vs, va(i));
                }
            }
        }
        r.m.check_tlb()
            .unwrap_or_else(|e| panic!("vòng {vong}: check_tlb: {e}"));
        assert_eq!(
            r.m.tlb_violations(),
            0,
            "vòng {vong}: phải 0 vi phạm TLB-safety"
        );
        let info = r.m.tlb_info(h).expect("info");
        assert!(info.entries <= TLB_ENTRIES_PER_HART);
        assert!(info.acked <= info.delivered, "acked != delivered");
        assert!(r.m.tlb_entries(h).expect("entries") <= TLB_ENTRIES_PER_HART);
    }
    // Dưới Miri cấu hình tự giảm (2 hart × 150 vòng) nên "phải có hit" không bảo đảm — ca
    // hit/miss đầy đủ đã có ở test 1; ở đây chỉ cần chứng minh có dịch (không phải property rỗng).
    #[cfg(not(miri))]
    assert!(hits > 0, "property phải có hit");
    assert!(hits + misses > 0, "property phải có ít nhất một lần dịch");
    assert!(misses > 0, "property phải có miss");
    assert!(unmaps > 0, "property phải có unmap/shootdown");
    assert!(
        r.m.tlb_shootdown_count() >= unmaps,
        "mỗi unmap (và protect) là một shootdown"
    );
    // Không còn shootdown treo; mọi message đã ack; dọn sạch rồi TLB rỗng.
    for h in 0..harts {
        assert!(r.m.tlb_pending(h).expect("pending").is_none());
        let info = r.m.tlb_info(h).expect("info");
        assert_eq!(info.acked, info.delivered);
        assert_eq!(r.m.mail_len(h), Ok(0), "hộp thư hart {h} phải rỗng");
    }
    for (i, &con_map) in mapped.iter().enumerate() {
        if con_map {
            r.m.vspace_unmap(vs, va(i)).expect("unmap dọn");
        }
    }
    for h in 0..harts {
        assert_eq!(r.m.tlb_entries(h), Ok(0), "hart {h} còn entry sau khi dọn");
    }
    r.kiem("property 10k");
}
