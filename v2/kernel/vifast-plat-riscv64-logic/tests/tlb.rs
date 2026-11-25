// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test KT-51 — `vifast-plat-common::tlb`: **ASID 8 bit + thế hệ** (cấp phát free-list,
//! **không tái dùng trong cùng thế hệ**, cạn ⇒ `generation += 1` + `rollover`), **`FlushScope`**
//! ba dạng, **ack theo `seq`** với bất biến `Ok ⇒ ack_mask == active_on`, **không self-target**
//! (D2-68), hart **ngoài `active_on` không bị chờ**, và bộ đếm PerHart.
//!
//! Chạy trên host (không asm/CSR/MMIO; D2-72 Q12) và chốt những bất biến mà arch dựa vào. Bằng
//! chứng **thật** trên phần cứng nằm ở TAP 25..29 của
//! `v2/kernel/vifast-kbin-riscv64/src/ktest_tlb.rs` (log `v2/agent/JOURNAL/kt51-ktest-smp*.log`).

use vifast_plat_common::smp::MAX_HARTS;
use vifast_plat_riscv64_logic::tlb::{
    ack_complete, ack_missing, acked_mask, implicit_ack, targets, unused_online_harts,
    AsidAllocator, AsidLease, FlushScope, TlbCounters, TlbFlushRequest, ASID_COUNT, ASID_MASK,
    ASID_NONE, ASID_WORDS, SATP_ASID_FIELD, SATP_ASID_SHIFT,
};
use vifast_pte::sv39::{satp_asid, satp_root, satp_sv39, satp_sv39_asid, SATP_MODE_SV39};

#[test]
fn hang_so_asid_8_bit_dung_d2_65_q4() {
    assert_eq!(ASID_COUNT, 256);
    assert_eq!(ASID_COUNT, 1 << 8);
    assert_eq!(ASID_MASK, 0xff);
    assert_eq!(ASID_WORDS, 4);
    assert!(ASID_NONE as usize >= ASID_COUNT);
    // Trường ASID của satp: bit 59..44, kernel dùng 8 bit thấp.
    assert_eq!(SATP_ASID_SHIFT, 44);
    assert_eq!(SATP_ASID_FIELD, 0x0fff_f000_0000_0000);
    assert_eq!(SATP_ASID_FIELD >> SATP_ASID_SHIFT, 0xffff);
    // `AsidLease::NONE` không hợp lệ, lease thật luôn hợp lệ.
    assert!(!AsidLease::NONE.is_valid());
    assert!(AsidLease {
        asid: 255,
        generation: 9,
        rollover: true
    }
    .is_valid());
}

#[test]
fn cap_asid_khong_trung_trong_cung_the_he() {
    let pool = AsidAllocator::new();
    let mut seen = [false; ASID_COUNT];
    for index in 0..ASID_COUNT {
        let lease = pool.allocate();
        assert_eq!(lease.generation, 0, "chưa cạn ⇒ cùng thế hệ");
        assert!(!lease.rollover, "chưa cạn ⇒ không rollover");
        let asid = lease.asid as usize;
        assert!(asid < ASID_COUNT);
        assert!(!seen[asid], "ASID {asid} bị cấp hai lần trong cùng thế hệ");
        seen[asid] = true;
        // Free-list cấp ASID thấp nhất trước (tất định, dễ đọc log): lần thứ index ra `index`.
        assert_eq!(asid, index);
    }
    let state = pool.state();
    assert_eq!(state.generation, 0);
    assert_eq!(state.free, 0);
    assert_eq!(state.issued, ASID_COUNT as u32);
    assert_eq!(state.exhaustions, 0);
}

#[test]
fn can_asid_tang_the_he_va_bao_rollover() {
    let pool = AsidAllocator::new();
    for _ in 0..ASID_COUNT {
        let _ = pool.allocate();
    }
    // Cạn ⇒ thế hệ +1, nạp lại đủ 256, báo `rollover` để người gọi phát **một** flush toàn bộ.
    let lease = pool.allocate();
    assert!(lease.rollover, "cạn ASID phải báo rollover");
    assert_eq!(lease.generation, 1);
    assert_eq!(lease.asid, 0, "thế hệ mới cấp lại từ ASID 0");
    let state = pool.state();
    assert_eq!(state.generation, 1);
    assert_eq!(state.free, ASID_COUNT - 1);
    assert_eq!(state.issued, 1);
    assert_eq!(state.exhaustions, 1);
    // Trong thế hệ mới vẫn không trùng.
    let mut seen = [false; ASID_COUNT];
    seen[0] = true;
    for _ in 0..(ASID_COUNT - 1) {
        let next = pool.allocate();
        assert_eq!(next.generation, 1);
        assert!(!next.rollover);
        assert!(!seen[next.asid as usize], "trùng ASID trong thế hệ mới");
        seen[next.asid as usize] = true;
    }
}

#[test]
fn free_ve_huu_trong_the_he_khong_trai_lai_free_list() {
    let pool = AsidAllocator::new();
    let lease = pool.allocate();
    assert_eq!(lease.asid, 0);
    assert!(pool.free(lease), "free lease hợp lệ phải trả true");
    assert!(!pool.free(lease), "double-free phải bị bắt (false)");
    let state = pool.state();
    assert_eq!(state.frees, 1);
    assert_eq!(
        state.free,
        ASID_COUNT - 1,
        "free không trả ASID về free-list"
    );
    // ASID 0 vẫn không được cấp lại; lần cấp kế là ASID 1.
    let next = pool.allocate();
    assert_eq!(next.asid, 1);
    assert_eq!(pool.state().issued, 2);
}

#[test]
fn free_sai_the_he_bi_tu_choi() {
    let pool = AsidAllocator::new();
    let old = pool.allocate();
    for _ in 0..(ASID_COUNT - 1) {
        let _ = pool.allocate();
    }
    let rollover = pool.allocate();
    assert!(rollover.rollover);
    assert_eq!(rollover.generation, 1);
    assert!(
        !pool.free(old),
        "lease của thế hệ cũ không được free trong thế hệ mới"
    );
    assert_eq!(pool.state().frees, 0);
}

#[test]
fn satp_giu_mode_ppn_khi_doi_asid() {
    let root = 0x804a_8000u64;
    let base = satp_sv39(root);
    assert_eq!(base >> 60, SATP_MODE_SV39);
    for asid in [0u16, 1, 7, 255] {
        let satp = satp_sv39_asid(root, asid);
        assert_eq!(satp >> 60, SATP_MODE_SV39, "MODE không đổi khi đổi ASID");
        assert_eq!(satp_root(satp), root, "PPN không đổi khi đổi ASID");
        assert_eq!(satp_asid(satp), asid);
        assert_eq!(
            satp & !SATP_ASID_FIELD,
            base & !SATP_ASID_FIELD,
            "chỉ trường ASID thay đổi"
        );
    }
    // ASID vượt 8 bit bị cắt (kernel chỉ dùng 8 bit).
    assert_eq!(satp_asid(satp_sv39_asid(root, 0x1ff)), 0xff);
    // ASID 0 cho satp y hệt KT-46 (không phá ảnh đang chạy).
    assert_eq!(satp_sv39_asid(root, 0), base);
}

#[test]
fn flush_scope_ba_dang_ma_hoa_giai_ma() {
    assert_eq!(FlushScope::PAGE_CODE, 0);
    assert_eq!(FlushScope::ASID_CODE, 1);
    assert_eq!(FlushScope::ALL_CODE, 2);
    let page = FlushScope::Page { va: 0x8080_0000 };
    assert_eq!(page.code(), 0);
    assert_eq!(page.va(), 0x8080_0000);
    assert_eq!(page.name(), "Page");
    assert_eq!(FlushScope::Asid.code(), 1);
    assert_eq!(FlushScope::Asid.name(), "Asid");
    assert_eq!(FlushScope::Asid.va(), 0);
    assert_eq!(FlushScope::All.code(), 2);
    assert_eq!(FlushScope::All.name(), "All");
    assert_eq!(
        FlushScope::decode(page.code(), page.va()),
        Some(page),
        "decode(encode(x)) == x cho Page"
    );
    assert_eq!(
        FlushScope::decode(FlushScope::ASID_CODE, 0),
        Some(FlushScope::Asid)
    );
    assert_eq!(
        FlushScope::decode(FlushScope::ALL_CODE, 0),
        Some(FlushScope::All)
    );
    assert_eq!(FlushScope::decode(3, 0), None, "mã lạ không được flush bừa");
    let request = TlbFlushRequest {
        seq: 7,
        asid: 3,
        scope: page,
    };
    assert_eq!(request.scope, page);
}

#[test]
fn ack_mask_chi_dung_hart_da_ack_du_seq() {
    let mut acked = [0u64; MAX_HARTS];
    acked[0] = 5;
    acked[2] = 7;
    acked[3] = 4;
    assert_eq!(acked_mask(&acked, 5), 0b101, "hart 2 (7≥5), hart 0 (5≥5)");
    assert_eq!(acked_mask(&acked, 6), 0b100, "chỉ hart 2 ack đủ seq 6");
    assert_eq!(acked_mask(&acked, 8), 0, "chưa ai ack seq 8");
    assert_eq!(ack_missing(0b101, 0b111), 0b010);
    assert!(ack_complete(0b101, 0b101));
    assert!(!ack_complete(0b101, 0b111));
}

#[test]
fn khong_self_target_va_ack_ngam_dung_d2_68() {
    // Hart gọi trong `active_on`: phải **bị loại** khỏi targets (không tự IPI) và ack **ngầm**.
    assert_eq!(targets(0b0111, 1), 0b0101);
    assert_eq!(implicit_ack(0b0111, 1), 0b0010);
    // Hart gọi **ngoài** `active_on`: không target, **không** ack ngầm.
    assert_eq!(targets(0b0110, 0), 0b0110);
    assert_eq!(implicit_ack(0b0110, 0), 0);
    // Active_on rỗng ⇒ không chờ ai, ack_complete luôn đúng.
    assert_eq!(targets(0, 0), 0);
    assert!(ack_complete(0, 0));
    // Hart online ngoài active_on: không bị chờ.
    assert_eq!(unused_online_harts(0b0011, 0b1111), 0b1100);
    // Tập targets và implicit không bao giờ giao nhau (không đếm hai lần một hart).
    for caller in 0..MAX_HARTS {
        for active in 0u64..8 {
            let active = active << 1;
            assert_eq!(targets(active, caller) & implicit_ack(active, caller), 0);
            // Hợp của hai tập luôn nằm trong active_on (hart ngoài active_on không bị chờ).
            assert_eq!(
                (targets(active, caller) | implicit_ack(active, caller)) & !active,
                0
            );
        }
    }
}

#[test]
fn bo_dem_tlb_per_hart_khong_lan() {
    let counters = TlbCounters::new();
    counters.record_sent();
    counters.record_sent();
    counters.record_received();
    counters.record_flush(FlushScope::Page { va: 0x1000 });
    counters.record_flush(FlushScope::Asid);
    counters.record_flush(FlushScope::All);
    counters.record_ack();
    counters.record_implicit();
    counters.record_timeout();
    counters.record_error();
    counters.record_wrong();
    let snap = counters.snapshot();
    assert_eq!(snap.sent, 2);
    assert_eq!(snap.received, 1);
    assert_eq!(snap.flushes, 3);
    assert_eq!(snap.flush_total(), 3, "tổng = page + asid + all");
    assert_eq!(snap.page_flushes, 1);
    assert_eq!(snap.asid_flushes, 1);
    assert_eq!(snap.all_flushes, 1);
    assert_eq!(snap.acks, 2, "ack tường minh + ack ngầm");
    assert_eq!(snap.implicit, 1);
    assert_eq!(snap.timeouts, 1);
    assert_eq!(snap.errors, 1);
    assert_eq!(snap.wrong, 1);
    let fresh = TlbCounters::new();
    assert_eq!(fresh.snapshot(), Default::default());
}

#[test]
fn thu_tu_cap_asid_va_generation_tat_dinh() {
    // Cùng một dãy thao tác ⇒ cùng kết quả (không phụ thuộc thời gian/thread).
    let run = || {
        let pool = AsidAllocator::new();
        let mut leases = [AsidLease::NONE; 8];
        for slot in leases.iter_mut() {
            *slot = pool.allocate();
        }
        let freed: [bool; 8] = core::array::from_fn(|i| pool.free(leases[i]));
        let next = pool.allocate();
        let state = pool.state();
        (leases, freed, next, state)
    };
    let first = run();
    let second = run();
    assert_eq!(first, second);
    assert_eq!(first.1, [true; 8]);
    assert_eq!(first.2.asid, 8, "8 ASID đã về hưu ⇒ ASID kế là 8");
    assert_eq!(first.3.issued, 9);
    assert_eq!(first.3.frees, 8);
    assert_eq!(first.3.free, ASID_COUNT - 9);
}
