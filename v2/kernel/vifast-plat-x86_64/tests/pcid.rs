// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho bộ cấp phát **PCID** — KT-413b (`D2-259` mục 2, nợ `KT-413` §5.2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test pcid
//! ```
//!
//! Phần **thuần** của `pcid.rs` (bitmap + ghép `CR3`) chạy được trên host; phần asm
//! (`enable`/`cpuid`) chỉ chạy trong ảnh — host stub **panic** nên test này **không** gọi.
//!
//! Bất biến chốt:
//!
//! 1. **PCID 0 là của kernel** — không bao giờ cấp cho `VSpace` (cấp nó = chia sẻ TLB với ảnh).
//! 2. **Mỗi `VSpace` một PCID** — cấp N lần ra N số **đôi một khác nhau**; trả rồi cấp lại đúng số
//!    vừa trả (không rò, không nhảy cóc vô hạn).
//! 3. **Hết PCID thì fail-closed** — `None` (caller dùng `PCID_KERNEL`), không panic, không cấp
//!    trùng.
//! 4. **`CR3` chỉ mang PCID khi `CR4.PCIDE = 1`** — mặc định tắt ⇒ `cr3_value(root, pcid, false)`
//!    **đúng bằng** `root` (0 đổi hành vi, 0 đổi dòng `# ipi: pcid` của runner).

use vifast_plat_x86_64::pcid::{self, PcidAlloc, PCID_COUNT, PCID_KERNEL};

/// **Ca 1** — PCID 0 (kernel) **không** được cấp; số đầu tiên là 1.
#[test]
fn kernel_pcid_is_never_allocated() {
    let mut a = PcidAlloc::new();
    assert!(a.is_used(PCID_KERNEL), "PCID 0 thuộc kernel");
    assert_eq!(a.used(), 0, "chưa cấp PCID nào cho VSpace");
    assert_eq!(a.alloc(), Some(1), "PCID đầu tiên cho VSpace là 1");
    assert!(a.is_used(1));
    assert!(!a.is_used(2));
    assert_eq!(a.used(), 1);
}

/// **Ca 2** — cấp N lần ra N PCID **đôi một khác nhau**; trả rồi cấp lại đúng số vừa trả.
#[test]
fn allocations_are_unique_and_reusable() {
    let mut a = PcidAlloc::new();
    let mut seen = std::vec::Vec::new();
    for _ in 0..64 {
        let p = a.alloc().expect("còn PCID");
        assert!(!seen.contains(&p), "PCID {p} bị cấp hai lần");
        seen.push(p);
    }
    assert_eq!(a.used(), 64);
    // Trả một số ở **giữa** dải rồi cấp lại: phải nhận đúng số nhỏ nhất vừa trả.
    let freed = seen[10];
    a.free(freed);
    assert!(!a.is_used(freed));
    assert_eq!(a.used(), 63);
    assert_eq!(a.alloc(), Some(freed), "cấp lại đúng PCID vừa trả");
    assert_eq!(a.used(), 64);
    // Trả PCID kernel / ngoài dải: no-op, không panic, không đổi bộ đếm.
    a.free(PCID_KERNEL);
    a.free(PCID_COUNT);
    a.free(u16::MAX);
    assert_eq!(a.used(), 64);
    assert!(a.is_used(PCID_KERNEL));
    assert!(!a.is_used(PCID_COUNT));
}

/// **Ca 3** — hết PCID thì `None` (fail-closed), không panic, không cấp trùng.
#[test]
fn exhaustion_is_fail_closed() {
    let mut a = PcidAlloc::new();
    // 4095 PCID cấp được (1..=4095); PCID 0 là của kernel.
    for i in 1..PCID_COUNT {
        assert_eq!(a.alloc(), Some(i), "PCID kế tiếp phải là {i}");
    }
    assert_eq!(a.used(), (PCID_COUNT - 1) as usize);
    assert_eq!(a.alloc(), None, "hết PCID ⇒ None, không cấp trùng");
    assert_eq!(a.alloc(), None);
    // Trả một số ⇒ cấp lại được đúng số ấy.
    a.free(1234);
    assert_eq!(a.alloc(), Some(1234));
    assert_eq!(a.alloc(), None);
}

/// **Ca 4** — `CR3` chỉ mang PCID khi `CR4.PCIDE = 1`; mặc định tắt ⇒ **đúng bằng** `root`.
#[test]
fn cr3_carries_pcid_only_when_enabled() {
    let root = 0x0000_0000_0023_7000u64;
    // Tắt (mặc định của ảnh): `CR3[11:0]` phải là 0 — đúng hợp đồng `cr4_pcid=0` của runner.
    assert_eq!(pcid::cr3_value(root, 7, false), root);
    assert_eq!(pcid::cr3_value(root, PCID_KERNEL, false), root);
    // Bật: `CR3[11:0]` = PCID, `CR3[63:12]` = root.
    assert_eq!(pcid::cr3_value(root, 7, true), root | 7);
    assert_eq!(pcid::cr3_value(root, 0xFFF, true), root | 0xFFF);
    // `root` lỡ có bit thấp (không được xảy ra) vẫn bị **che** — không ghi rác vào trường PCID.
    assert_eq!(pcid::cr3_value(root | 0xABC, 7, true), root | 7);
    assert_eq!(pcid::cr3_value(root | 0xABC, 7, false), root);
}

/// **Ca 5** — bitmap **toàn ảnh**: cấp/trả qua static, đếm đúng, `exhausted() == 0`.
///
/// Gộp **một** test vì static dùng chung giữa các test chạy song song.
#[test]
fn global_allocator_round_trip() {
    assert_eq!(pcid::exhausted(), 0, "chưa lần nào hết PCID");
    let before = pcid::used_pcids();
    let a = pcid::alloc_pcid().expect("còn PCID");
    let b = pcid::alloc_pcid().expect("còn PCID");
    assert_ne!(a, b, "hai VSpace phải hai PCID khác nhau");
    assert_ne!(a, PCID_KERNEL);
    assert_ne!(b, PCID_KERNEL);
    assert_eq!(pcid::used_pcids(), before + 2);
    pcid::free_pcid(a);
    assert_eq!(pcid::used_pcids(), before + 1);
    // Trả lại số vừa cấp ⇒ lần cấp kế tiếp nhận đúng nó (nhỏ nhất còn trống).
    assert_eq!(pcid::alloc_pcid(), Some(a));
    pcid::free_pcid(a);
    pcid::free_pcid(b);
    assert_eq!(pcid::used_pcids(), before);
    // Trả PCID kernel / ngoài dải: no-op.
    pcid::free_pcid(PCID_KERNEL);
    pcid::free_pcid(PCID_COUNT);
    assert_eq!(pcid::used_pcids(), before);
    assert_eq!(pcid::exhausted(), 0);
}

/// **Ca 6** — **mặc định TẮT**: `enabled()` phải `false` (hợp đồng `# ipi: pcid … cr4_pcid=0`).
///
/// Đây là ca **hợp đồng runner**: bật PCID mà không đổi kỳ vọng `run-x86_64.sh`/`soak-x86_64.sh`
/// là làm đỏ cổng — nên đường boot mặc định **không** được gọi `enable()`.
#[test]
fn pcid_is_disabled_by_default() {
    assert!(
        !pcid::enabled(),
        "PCID phải TẮT mặc định (runner đòi cr4_pcid=0)"
    );
}
