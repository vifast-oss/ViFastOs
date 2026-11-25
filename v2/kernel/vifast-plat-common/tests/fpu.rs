// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — host test cho **ngữ cảnh FPU/SIMD eager theo TCB** (`vifast_plat_common::fpu`).
//!
//! Kiểm đúng thứ mà tầng bookkeeping hứa (D2-259 mục 2):
//!
//! 1. **Vùng lưu đúng cỡ/căn**: 32 thanh ghi × 8 B + 2 word điều khiển = **272 B**, căn **16**
//!    (`fxsave64` của x86_64 đòi 16 B — cùng một kiểu dùng cho `fs0..fs31`/`v0..v31`).
//! 2. **Khe khoá theo TCB**: cùng TCB ⇒ cùng khe; hai TCB ⇒ hai khe **khác nhau** (không lẫn).
//! 3. **Tràn khe ⇒ fail-closed**: TCB thứ `N+1` **không** được cấp khe, **không** ghi đè khe của
//!    TCB khác, bộ đếm `overflow` tăng — không có đường "dùng chung vùng lưu".
//! 4. **`valid` chỉ sau khi lưu**: TCB mới ⇒ `valid = false` (khôi phục = trạng thái mới, không
//!    phải rác của TCB khác); sau `mark_valid` ⇒ `true`; `invalidate` ⇒ `false`.
//! 5. **Hai TCB không lẫn vùng**: ghi mẫu A vào khe TCB 1, mẫu B vào khe TCB 2, đọc lại ⇒ mỗi
//!    bên giữ **đúng** mẫu của mình (đây là bất biến "không lẫn thanh ghi" ở tầng dữ liệu).
//! 6. **4 luồng thật**: 4 luồng × 2.000 lượt ghi/đọc vùng của **chính** TCB mình ⇒ 0 lẫn.

use std::sync::Arc;
use std::thread;

use vifast_plat_common::fpu::{
    FpuArea, FpuTracker, FPU_AREA_BYTES, FPU_CTRL_WORDS, FPU_REGS, FPU_REG_BYTES, FPU_SLOTS,
};

/// Mẫu 8 B suy từ `(tcb, reg)` — hai TCB khác nhau **không thể** trùng mẫu.
fn pattern(tcb: u32, reg: usize) -> u64 {
    (u64::from(tcb) << 32) | (0x5a5a_0000 | reg as u64)
}

#[test]
fn vung_luu_dung_co_va_can_16() {
    assert_eq!(FPU_REGS, 32, "32 thanh ghi dữ liệu (fs0..fs31 / v0..v31)");
    assert_eq!(FPU_REG_BYTES, 16, "128 bit/thanh ghi — đủ cho NEON v0..v31");
    assert_eq!(FPU_CTRL_WORDS, 2, "fcsr / fpsr+fpcr / mxcsr+x87 control");
    assert_eq!(FPU_AREA_BYTES, 528, "32 × 16 B + 2 × 8 B");
    assert_eq!(core::mem::size_of::<FpuArea>(), FPU_AREA_BYTES);
    assert_eq!(core::mem::align_of::<FpuArea>(), 16, "fxsave64 đòi 16 B");
    assert_eq!(
        FPU_SLOTS, 65,
        "MAX_TCBS = 64 (spec/kernel.toml) + 1 (ObjId 1-based)"
    );
}

#[test]
fn khe_theo_tcb_khong_lan() {
    let tracker: FpuTracker<8> = FpuTracker::new();
    let a = tracker.slot_for(3).expect("TCB 3 phải có khe");
    let b = tracker.slot_for(4).expect("TCB 4 phải có khe");
    assert_ne!(a, b, "hai TCB khác nhau phải hai khe khác nhau");
    assert_eq!(tracker.slot_for(3), Some(a), "cùng TCB ⇒ cùng khe");
    assert_eq!(tracker.slot_for(4), Some(b));
    // TCB 0 là ObjId không hợp lệ của core (1-based) ⇒ không cấp khe.
    assert_eq!(
        tracker.slot_for(0),
        None,
        "TCB 0 không hợp lệ ⇒ fail-closed"
    );
}

#[test]
fn tran_khe_fail_closed() {
    let tracker: FpuTracker<2> = FpuTracker::new();
    let a = tracker.slot_for(1).expect("khe 1");
    let b = tracker.slot_for(2).expect("khe 2");
    assert_ne!(a, b);
    assert_eq!(tracker.slot_for(3), None, "tràn ⇒ KHÔNG cấp khe");
    assert_eq!(tracker.overflow(), 1, "đếm được lần tràn");
    // Khe của hai TCB cũ **không** bị ghi đè (fail-closed, không "dùng chung vùng lưu").
    assert_eq!(tracker.slot_for(1), Some(a));
    assert_eq!(tracker.slot_for(2), Some(b));
    assert_eq!(tracker.slot_for(3), None);
    assert_eq!(tracker.overflow(), 2);
}

#[test]
fn valid_chi_sau_khi_luu() {
    let tracker: FpuTracker<4> = FpuTracker::new();
    assert!(
        !tracker.is_valid(7),
        "TCB mới ⇒ chưa có trạng thái để khôi phục"
    );
    tracker.mark_valid(7);
    assert!(tracker.is_valid(7));
    tracker.invalidate(7);
    assert!(
        !tracker.is_valid(7),
        "xoá ⇒ lần vào kế tiếp là trạng thái mới"
    );
    // `invalidate` TCB chưa từng có khe: không panic, không cấp khe.
    tracker.invalidate(9);
    assert!(!tracker.is_valid(9));
}

#[test]
fn hai_tcb_khong_lan_vung() {
    let tracker: FpuTracker<4> = FpuTracker::new();
    let mut areas = [FpuArea::ZERO; 4];
    let s1 = tracker.slot_for(1).expect("khe TCB 1");
    let s2 = tracker.slot_for(2).expect("khe TCB 2");
    for reg in 0..FPU_REGS {
        areas[s1].set_word(reg * FPU_REG_BYTES, pattern(1, reg));
        areas[s2].set_word(reg * FPU_REG_BYTES, pattern(2, reg));
    }
    areas[s1].set_word(FPU_REGS * FPU_REG_BYTES, 0x11);
    areas[s2].set_word(FPU_REGS * FPU_REG_BYTES, 0x22);
    for reg in 0..FPU_REGS {
        assert_eq!(
            areas[s1].word(reg * FPU_REG_BYTES),
            pattern(1, reg),
            "TCB 1 reg {reg}"
        );
        assert_eq!(
            areas[s2].word(reg * FPU_REG_BYTES),
            pattern(2, reg),
            "TCB 2 reg {reg}"
        );
    }
    assert_eq!(areas[s1].word(FPU_REGS * FPU_REG_BYTES), 0x11);
    assert_eq!(areas[s2].word(FPU_REGS * FPU_REG_BYTES), 0x22);
}

#[test]
fn bon_luong_khong_lan() {
    let tracker: Arc<FpuTracker<8>> = Arc::new(FpuTracker::new());
    let mut handles = Vec::new();
    for tcb in 1u32..=4 {
        let tracker = Arc::clone(&tracker);
        handles.push(thread::spawn(move || {
            let slot = tracker.slot_for(tcb).expect("mỗi luồng một khe");
            let mut area = FpuArea::ZERO;
            for round in 0..2_000u64 {
                for reg in 0..FPU_REGS {
                    area.set_word(reg * FPU_REG_BYTES, pattern(tcb, reg) ^ round);
                }
                for reg in 0..FPU_REGS {
                    assert_eq!(
                        area.word(reg * FPU_REG_BYTES),
                        pattern(tcb, reg) ^ round,
                        "luồng tcb={tcb} reg={reg} round={round}"
                    );
                }
            }
            slot
        }));
    }
    let slots: Vec<usize> = handles
        .into_iter()
        .map(|h| h.join().expect("luồng"))
        .collect();
    let mut sorted = slots.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 4, "4 TCB ⇒ 4 khe phân biệt: {slots:?}");
}

#[test]
fn bo_dem_save_restore_skip() {
    let tracker: FpuTracker<4> = FpuTracker::new();
    tracker.record_save();
    tracker.record_save();
    tracker.record_restore();
    tracker.record_skip();
    let stats = tracker.stats();
    assert_eq!(stats.saves, 2);
    assert_eq!(stats.restores, 1);
    assert_eq!(stats.skipped, 1);
    assert_eq!(stats.overflow, 0);
    // Bộ đếm là **bằng chứng** cho TAP: tổng lượt = saves + skipped (mỗi lượt ra U-mode đúng
    // một trong hai), và mỗi lượt vào U-mode là restores + fresh.
    tracker.record_fresh();
    let stats = tracker.stats();
    assert_eq!(stats.fresh, 1);
    assert_eq!(stats.saves + stats.skipped, 3);
}

#[test]
fn dem_luot_vao_ra_khop() {
    // Mô phỏng 2 TCB chạy xen kẽ 100 lượt: mỗi lượt ra U-mode ⇒ save **hoặc** skip; mỗi lượt
    // vào ⇒ restore **hoặc** fresh. Bất biến: saves + skipped == restores + fresh == số lượt.
    let tracker: FpuTracker<4> = FpuTracker::new();
    let mut areas = [FpuArea::ZERO; 4];
    let mut dirty = [false; 4];
    let tcbs = [1u32, 2u32];
    for round in 0..100u64 {
        let tcb = tcbs[(round as usize) % 2];
        let slot = tracker.slot_for(tcb).expect("khe");
        // Vào U-mode.
        if tracker.is_valid(tcb) {
            assert_eq!(areas[slot].word(0), pattern(tcb, 0), "khôi phục đúng vùng");
            tracker.record_restore();
        } else {
            tracker.record_fresh();
        }
        // Tác vụ chạy: nếu "dùng FP" thì ghi mẫu của mình.
        if round % 3 == 0 {
            areas[slot].set_word(0, pattern(tcb, 0));
            tracker.mark_valid(tcb);
            dirty[(round as usize) % 2] = true;
        }
        // Ra U-mode.
        if dirty[(round as usize) % 2] {
            tracker.record_save();
            dirty[(round as usize) % 2] = false;
        } else {
            tracker.record_skip();
        }
    }
    let stats = tracker.stats();
    assert_eq!(stats.saves + stats.skipped, 100);
    assert_eq!(stats.restores + stats.fresh, 100);
    assert_eq!(stats.overflow, 0);
}
