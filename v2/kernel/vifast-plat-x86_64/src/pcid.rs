// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-413b** — cấp **PCID riêng cho từng `VSpace`** + bật `CR4.PCIDE` khi CPU hỗ trợ.
//!
//! # Trạng thái mặc định: **TẮT** (hợp đồng runner hiện hành)
//!
//! `KT-340d` đóng lỗi v1 **N5** bằng nhánh "tắt `PCID`/`PGE` **có kiểm soát** + ghi lý do":
//! `run-x86_64.sh` (dòng 1163-1170) và `soak-x86_64.sh` (dòng 603, khớp **cả dòng** tới `$`) đòi
//! `# ipi: pcid … cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg`. Vì vậy
//! [`enable`] **không** được gọi trên đường boot mặc định: `CR4.PCIDE` vẫn `0`, `CR3[11:0]` vẫn
//! `0`, `INVLPG` + `CR3` reload vẫn là toàn bộ cơ chế xoá TLB — **không** đổi một byte hành vi.
//!
//! # Vì sao phải cấp PCID **riêng** trước khi bật
//!
//! Bật `CR4.PCIDE` mà mọi `VSpace` cùng dùng PCID `0` thì `INVLPG`/`CR3` reload vẫn chỉ chạm
//! **một** không gian TLB: không có lợi ích, và tệ hơn — `CR3` reload **không** xoá entry của
//! PCID khác, nên một `VSpace` "tưởng đã xả TLB" có thể đọc PA cũ (đúng lỗi N5). Vì vậy tệp này
//! cấp **một** PCID cho mỗi `VSpace` ([`alloc_pcid`], gọi từ [`crate::vspace::VSpace::create`])
//! và [`crate::vspace::VSpace::activate`] ghép nó vào `CR3[11:0]` **chỉ khi** [`enabled`].
//!
//! # Cách bật (khi Lead có D2 duyệt đổi kỳ vọng `# ipi: pcid`)
//!
//! 1. Gọi [`enable`] **một lần** trên BSP **trước** khi tạo `VSpace` đầu tiên (sau khi `CPUID`
//!    xác nhận PCID; hàm trả `false` nếu CPU không có ⇒ fail-closed, không `#GP`).
//! 2. `VSpace::activate` tự ghép PCID (không cần sửa gì thêm) — `CR3 = root | pcid`.
//! 3. `VSpace::flush_va` phải chuyển từ `INVLPG` sang `INVPCID` **theo PCID** khi hart đích chạy
//!    `VSpace` khác (nợ riêng: `INVLPG` chỉ xả entry của PCID **hiện hành** trên hart gọi).
//! 4. Đổi kỳ vọng runner: `cr4_pcid=1` + lý do mới (`reason=pcid-per-vspace`), và `soak` phải
//!    khớp dòng mới — **đổi kỳ vọng harness ⇒ cần D2** (AGENTS §4.2), không tự sửa.
//!
//! # Không `unsafe`
//!
//! Mọi lệnh đặc quyền đi qua wrapper an toàn của [`crate::asm`] (D2-17: `unsafe` chỉ trong
//! `asm.rs`); phần **thuần** ([`PcidAlloc`], [`cr3_value`]) chạy được trên host.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::asm;
use crate::control::{CR3_PCID_MASK, CR4_PCIDE};

/// Số PCID của không gian 12 bit (`CR3[11:0]`).
pub const PCID_COUNT: u16 = 4096;
/// PCID dành cho kernel/boot — **không** cấp cho `VSpace` (xem [`PcidAlloc::alloc`]).
pub const PCID_KERNEL: u16 = 0;
/// Số từ 64 bit của bitmap PCID.
const WORDS: usize = PCID_COUNT as usize / 64;

/// Bộ cấp phát PCID **thuần** (không static, không asm) — host-test được.
///
/// Bit `i` của `bits` ⇔ PCID `i` đã cấp. `PCID_KERNEL` (0) **luôn** coi như đã dùng: nó là PCID
/// của ảnh kernel (`CR3` lúc boot), cấp nó cho `VSpace` sẽ làm hai không gian địa chỉ chia sẻ TLB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcidAlloc {
    bits: [u64; WORDS],
}

impl PcidAlloc {
    /// Bộ cấp phát **rỗng** (chưa cấp PCID nào; `PCID_KERNEL` vẫn bị giữ).
    pub const fn new() -> Self {
        Self { bits: [0; WORDS] }
    }

    /// Cấp PCID **chưa dùng** nhỏ nhất (bỏ qua [`PCID_KERNEL`]); `None` khi hết.
    pub fn alloc(&mut self) -> Option<u16> {
        for (i, word) in self.bits.iter_mut().enumerate() {
            let mut free = !*word;
            if i == 0 {
                // PCID 0 = kernel — không bao giờ cấp.
                free &= !1u64;
            }
            if free == 0 {
                continue;
            }
            let bit = free.trailing_zeros() as usize;
            *word |= 1u64 << bit;
            return Some((i * 64 + bit) as u16);
        }
        None
    }

    /// Trả PCID về bộ cấp phát. `PCID_KERNEL` và số ngoài dải là **no-op** (không panic).
    pub fn free(&mut self, pcid: u16) {
        if pcid == PCID_KERNEL || pcid >= PCID_COUNT {
            return;
        }
        let i = pcid as usize;
        self.bits[i / 64] &= !(1u64 << (i % 64));
    }

    /// `true` ⇔ `pcid` đang được cấp (`PCID_KERNEL` luôn `true` — nó thuộc kernel).
    pub fn is_used(&self, pcid: u16) -> bool {
        if pcid >= PCID_COUNT {
            return false;
        }
        if pcid == PCID_KERNEL {
            return true;
        }
        let i = pcid as usize;
        self.bits[i / 64] & (1u64 << (i % 64)) != 0
    }

    /// Số PCID đã cấp cho `VSpace` (không tính [`PCID_KERNEL`]).
    pub fn used(&self) -> usize {
        self.bits
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let w = if i == 0 { *w & !1u64 } else { *w };
                w.count_ones() as usize
            })
            .sum()
    }
}

impl Default for PcidAlloc {
    fn default() -> Self {
        Self::new()
    }
}

/// Bitmap PCID **toàn ảnh** — mỗi `VSpace` một bit (xem [`alloc_pcid`]).
static PCID_BITS: [AtomicU64; WORDS] = [const { AtomicU64::new(0) }; WORDS];
/// `true` ⇔ `CR4.PCIDE` đã bật (mặc định **`false`** — xem đầu tệp).
static PCID_ENABLED: AtomicBool = AtomicBool::new(false);
/// Số lần [`alloc_pcid`] **hết** PCID (bằng chứng: phải `0` trong thực tế).
static PCID_EXHAUSTED: AtomicU64 = AtomicU64::new(0);

/// Cấp **một** PCID cho `VSpace` mới; `None` khi hết (caller dùng [`PCID_KERNEL`] — an toàn,
/// chỉ mất lợi ích TLB; [`exhausted`] đếm để phép kiểm bắt được).
pub fn alloc_pcid() -> Option<u16> {
    for (i, word) in PCID_BITS.iter().enumerate() {
        let mut cur = word.load(Ordering::Acquire);
        loop {
            let mut free = !cur;
            if i == 0 {
                free &= !1u64;
            }
            if free == 0 {
                break;
            }
            let bit = free.trailing_zeros() as usize;
            let next = cur | (1u64 << bit);
            match word.compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return Some((i * 64 + bit) as u16),
                Err(seen) => cur = seen,
            }
        }
    }
    PCID_EXHAUSTED.fetch_add(1, Ordering::Relaxed);
    None
}

/// Trả PCID về bitmap toàn ảnh (`PCID_KERNEL`/ngoài dải = no-op).
pub fn free_pcid(pcid: u16) {
    if pcid == PCID_KERNEL || pcid >= PCID_COUNT {
        return;
    }
    let i = pcid as usize;
    PCID_BITS[i / 64].fetch_and(!(1u64 << (i % 64)), Ordering::AcqRel);
}

/// Số PCID đã cấp cho `VSpace` (không tính [`PCID_KERNEL`]).
pub fn used_pcids() -> usize {
    PCID_BITS
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let w = w.load(Ordering::Acquire);
            let w = if i == 0 { w & !1u64 } else { w };
            w.count_ones() as usize
        })
        .sum()
}

/// Số lần cấp PCID thất bại vì **hết** (phải `0`).
pub fn exhausted() -> u64 {
    PCID_EXHAUSTED.load(Ordering::Relaxed)
}

/// `true` ⇔ `CR4.PCIDE` đã bật (mặc định **`false`**).
pub fn enabled() -> bool {
    PCID_ENABLED.load(Ordering::Acquire)
}

/// Bật `CR4.PCIDE` **khi CPU hỗ trợ**; trả `false` (không đổi gì) khi CPU **không** có PCID.
///
/// **Không** gọi trên đường boot mặc định — xem đầu tệp (hợp đồng `# ipi: pcid` của runner).
/// Idempotent: gọi lại khi đã bật trả `true` ngay.
pub fn enable() -> bool {
    if PCID_ENABLED.load(Ordering::Acquire) {
        return true;
    }
    let (_, ecx, _) = asm::cpuid_leaf1();
    if ecx & (1 << 17) == 0 {
        // CPU không có PCID: ghi `CR4.PCIDE` sẽ `#GP` ⇒ fail-closed, không đụng `CR4`.
        return false;
    }
    let cr4 = asm::read_cr4();
    asm::write_cr4(cr4 | CR4_PCIDE);
    // Bật `PCIDE` **không** xả TLB; nạp lại `CR3` (PCID 0) để trạng thái TLB xác định.
    asm::write_cr3(asm::read_cr3());
    PCID_ENABLED.store(true, Ordering::Release);
    true
}

/// Giá trị ghi vào `CR3` cho `(root, pcid)`: `root | pcid` **chỉ khi** PCID đang bật.
///
/// Khi `CR4.PCIDE = 0`, `CR3[11:0]` phải là `0` (bit PCID chỉ có nghĩa khi `PCIDE = 1`) — hàm
/// này giữ đúng bất biến ấy, nên `activate` dùng nó cho **cả hai** trạng thái.
pub fn cr3_value(root: u64, pcid: u16, pcid_enabled: bool) -> u64 {
    let root = root & !CR3_PCID_MASK;
    if pcid_enabled {
        root | (pcid as u64 & CR3_PCID_MASK)
    } else {
        root
    }
}

// Bất biến hằng số — sai một trong số này thì `CR3`/bitmap không còn khớp nhau.
const _: () = {
    assert!(PCID_COUNT == 4096);
    assert!(PCID_COUNT as usize == WORDS * 64);
    assert!(PCID_KERNEL == 0);
    assert!(CR3_PCID_MASK == 0xFFF);
    assert!(CR4_PCIDE == 1 << 17);
};
