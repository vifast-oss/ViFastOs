// SPDX-License-Identifier: MIT OR Apache-2.0
//! SMP dùng chung (KT-49): **bố cục arena theo hart**, [`HartMask`], ngữ nghĩa **SBI HSM**.
//!
//! Tệp này **không có `unsafe`** (`vifast-plat-common` là `#![forbid(unsafe_code)]`): SBI `ecall`,
//! CSR `tp`/`sscratch`, `stvec` theo hart nằm ở `kernel/vifast-plat-riscv64` (D2-17). Ở đây là
//! phần *ngữ nghĩa* chạy được trên host (D2-72 Q12):
//!
//! - **`MAX_HARTS = 8`** `[TK]` (D2-72 Q4) — trần hart của K3; matrix kiểm `1/2/4`;
//! - **bố cục NOLOAD theo hart**: mỗi hart một **record** `guard | stack | trap stack`
//!   ([`HART_GUARD_BYTES`] + [`HART_STACK_BYTES`] + [`HART_TRAP_STACK_BYTES`] =
//!   [`HART_STRIDE`]); guard nằm **ngay dưới** đáy stack của **chính hart đó** nên tràn stack
//!   chạm đúng guard của hart mình (không dùng chung guard như KT-44..48 một hart);
//! - [`HartMask`] — bitmask hart bằng `AtomicU64` (không `unsafe`), dùng cho `hart_online`,
//!   `hart_started`, barrier và kết quả TAP theo hart;
//! - **SBI HSM** (EID [`SBI_EXT_HSM`] `0x48534D`): FID 0 `hart_start`, 1 `hart_stop`, 2
//!   `hart_get_status`; hằng trạng thái [`SBI_HART_STARTED`]..[`SBI_HART_RESUME_PENDING`] và mã
//!   lỗi [`SBI_ERR_INVALID_PARAM`] (`-3`) / [`SBI_ERR_ALREADY_AVAILABLE`] (`-6`);
//! - hàm thuần [`classify_hart_status`] / [`start_decision`] / [`hart_start_ok`] để arch quyết
//!   định **start hay từ chối** — hart không có trong domain/DTB (`-3 INVALID_PARAM`) bị **từ
//!   chối** (log, không panic cả máy), đúng KT-49.
//!
//! **Hart id thật** lấy từ `tp` (SBI truyền `a0 = hartid` cho cả hart boot lẫn hart do
//! `hart_start` đánh thức); mỗi hart có `tp`/`sscratch`/`stvec` riêng. **Không** thuộc KT-49:
//! IPI (KT-50), ASID/TLB shootdown (KT-51), preemption, hotplug; `MAX_HARTS` **không** phải trần
//! cấu hình 64 của QD §3.15 (K3 chỉ cần 8).

use core::sync::atomic::{AtomicU64, Ordering};

/// Số hart tối đa của K3 `[TK]` — D2-72 Q4 (`MAX_HARTS = 8`), matrix kiểm `1/2/4`.
pub const MAX_HARTS: usize = 8;

/// Trang **guard** của một hart (4 KiB) `[TK]` — NONE trong page table, nằm ngay dưới đáy stack
/// của chính hart đó.
pub const HART_GUARD_BYTES: u64 = 0x1000;
/// Stack của một hart (64 KiB) `[TK]` — NOLOAD, `sp` bắt đầu ở đỉnh và mọc xuống guard.
pub const HART_STACK_BYTES: u64 = 0x10000;
/// Trap stack của một hart (16 KiB) `[TK]` — NOLOAD; `trap_entry` đổi `sp` sang đỉnh vùng này
/// qua `sscratch` của hart đang chạy, trapframe 288 B nằm trọn trong vùng.
pub const HART_TRAP_STACK_BYTES: u64 = 0x4000;
/// Bước giữa hai record hart: `guard + stack + trap stack` = 0x1_5000 (84 KiB) `[TK]`.
pub const HART_STRIDE: u64 = HART_GUARD_BYTES + HART_STACK_BYTES + HART_TRAP_STACK_BYTES;
/// Tổng vùng NOLOAD theo hart: `MAX_HARTS × HART_STRIDE` = 0xA_8000 (672 KiB) `[TK]`.
pub const HART_AREA_BYTES: u64 = HART_STRIDE * MAX_HARTS as u64;
/// Offset đáy stack của hart trong record (ngay trên guard).
pub const HART_STACK_OFFSET: u64 = HART_GUARD_BYTES;
/// Offset đáy trap stack của hart trong record (ngay trên stack).
pub const HART_TRAP_STACK_OFFSET: u64 = HART_GUARD_BYTES + HART_STACK_BYTES;

/// `true` ⇔ `hart` là hart id hợp lệ của K3 (`< MAX_HARTS`).
pub const fn valid_hart(hart: usize) -> bool {
    hart < MAX_HARTS
}

/// Bit của `hart` trong [`HartMask`]; hart ngoài dải ⇒ 0 (không bao giờ hợp lệ).
pub const fn hart_bit(hart: usize) -> u64 {
    if valid_hart(hart) {
        1u64 << hart
    } else {
        0
    }
}

/// Offset record của `hart` trong vùng NOLOAD theo hart (hart ngoài dải ⇒ `MAX_HARTS` record —
/// caller phải kiểm [`valid_hart`] trước; hàm này chỉ dùng cho hart hợp lệ).
pub const fn hart_record_offset(hart: usize) -> u64 {
    hart as u64 * HART_STRIDE
}

/// Offset **guard** của `hart` (đầu record).
pub const fn guard_offset(hart: usize) -> u64 {
    hart_record_offset(hart)
}

/// Offset **đáy stack** của `hart` (ngay trên guard).
pub const fn stack_bottom_offset(hart: usize) -> u64 {
    hart_record_offset(hart) + HART_STACK_OFFSET
}

/// Offset **đỉnh stack** của `hart` (`sp` ban đầu).
pub const fn stack_top_offset(hart: usize) -> u64 {
    hart_record_offset(hart) + HART_STACK_OFFSET + HART_STACK_BYTES
}

/// Offset **đáy trap stack** của `hart`.
pub const fn trap_bottom_offset(hart: usize) -> u64 {
    hart_record_offset(hart) + HART_TRAP_STACK_OFFSET
}

/// Offset **đỉnh trap stack** của `hart` (`sscratch` ban đầu).
pub const fn trap_top_offset(hart: usize) -> u64 {
    hart_record_offset(hart) + HART_TRAP_STACK_OFFSET + HART_TRAP_STACK_BYTES
}

/// `true` ⇔ `offset` (tính từ đầu vùng NOLOAD) rơi vào **record** của một hart
/// (`< HART_AREA_BYTES`). Dùng cho kiểm tra chéo linker ↔ hằng số.
pub const fn in_hart_area(offset: u64) -> bool {
    offset < HART_AREA_BYTES
}

/// `true` ⇔ `offset` rơi vào **guard** của hart nào đó.
pub const fn is_guard_offset(offset: u64) -> bool {
    in_hart_area(offset) && offset % HART_STRIDE < HART_GUARD_BYTES
}

/// Hart **sở hữu** `offset` (nếu thuộc vùng NOLOAD theo hart) — dùng để nói "fault thuộc hart nào".
pub const fn hart_of_offset(offset: u64) -> Option<usize> {
    if in_hart_area(offset) {
        Some((offset / HART_STRIDE) as usize)
    } else {
        None
    }
}

/// Bitmask hart — `AtomicU64`, `MAX_HARTS = 8 < 64` nên một word là đủ.
///
/// Không phải khoá: mỗi hart chỉ set/clear bit của mình (thường bằng `fetch_or`), đọc bằng
/// `load(Ordering::SeqCst)` khi barrier. `unsafe` **không** cần vì `AtomicU64` là API an toàn.
#[derive(Debug)]
pub struct HartMask {
    bits: AtomicU64,
}

impl HartMask {
    /// Mask rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            bits: AtomicU64::new(0),
        }
    }

    /// Mask từ bit sẵn có (vd `expected` dựng bằng [`hart_bit`]) — `const` cho test.
    pub const fn from_bits(bits: u64) -> Self {
        Self {
            bits: AtomicU64::new(bits),
        }
    }

    /// Set bit của `hart`; trả `true` ⇔ bit **vừa được set** (hart mới online), `false` nếu đã
    /// set từ trước hoặc hart ngoài dải.
    pub fn insert(&self, hart: usize) -> bool {
        if !valid_hart(hart) {
            return false;
        }
        let bit = hart_bit(hart);
        let previous = self.bits.fetch_or(bit, Ordering::SeqCst);
        previous & bit == 0
    }

    /// Clear bit của `hart`; trả `true` ⇔ bit **vừa được clear**.
    pub fn remove(&self, hart: usize) -> bool {
        if !valid_hart(hart) {
            return false;
        }
        let bit = hart_bit(hart);
        let previous = self.bits.fetch_and(!bit, Ordering::SeqCst);
        previous & bit != 0
    }

    /// `true` ⇔ bit của `hart` đang set.
    pub fn contains(&self, hart: usize) -> bool {
        valid_hart(hart) && self.bits.load(Ordering::SeqCst) & hart_bit(hart) != 0
    }

    /// Set/clear bit theo `on`; trả `true` ⇔ trạng thái **đổi**.
    pub fn set(&self, hart: usize, on: bool) -> bool {
        if on {
            self.insert(hart)
        } else {
            self.remove(hart)
        }
    }

    /// Toàn bộ bit (ảnh chụp).
    pub fn bits(&self) -> u64 {
        self.bits.load(Ordering::SeqCst)
    }

    /// Số hart đang set.
    pub fn count(&self) -> u32 {
        self.bits().count_ones()
    }

    /// `true` ⇔ mask rỗng.
    pub fn is_empty(&self) -> bool {
        self.bits() == 0
    }

    /// Clear toàn bộ (dùng cho barrier/khởi động lại trong test — ảnh thật không reset).
    pub fn clear(&self) {
        self.bits.store(0, Ordering::SeqCst);
    }

    /// Hart id thấp nhất đang set (nếu có).
    pub fn lowest(&self) -> Option<usize> {
        let bits = self.bits();
        if bits == 0 {
            None
        } else {
            Some(bits.trailing_zeros() as usize)
        }
    }

    /// Hart id cao nhất đang set (nếu có).
    pub fn highest(&self) -> Option<usize> {
        let bits = self.bits();
        if bits == 0 {
            None
        } else {
            Some(63 - bits.leading_zeros() as usize)
        }
    }
}

impl Default for HartMask {
    fn default() -> Self {
        Self::new()
    }
}

/// Mask của các hart `0..count` — "N hart đầu tiên" (matrix `1/2/4`).
pub const fn range_mask(count: usize) -> u64 {
    if count >= MAX_HARTS {
        (1u64 << MAX_HARTS) - 1
    } else {
        (1u64 << count) - 1
    }
}

/// Mask của một slice hart (bỏ hart ngoài dải).
pub fn mask_of(harts: &[usize]) -> u64 {
    let mut mask = 0u64;
    for &hart in harts {
        mask |= hart_bit(hart);
    }
    mask
}

/// Số hart trong mask (pure, dùng cho TAP/log không cần `HartMask`).
pub const fn mask_count(mask: u64) -> u32 {
    mask.count_ones()
}

// ---------------------------------------------------------------------------------------------
// SBI HSM (SBI v2.0 §8): EID 0x48534D, FID 0/1/2
// ---------------------------------------------------------------------------------------------

/// EID **SBI HSM** `0x48534D` ("HSM" ASCII) — D2-72 Q4/KT-49.
pub const SBI_EXT_HSM: u64 = 0x4853_4D;
/// FID 0 — `sbi_hart_start(hartid, start_addr, opaque)`.
pub const SBI_HSM_HART_START: u64 = 0;
/// FID 1 — `sbi_hart_stop()` (không dùng ở K3).
pub const SBI_HSM_HART_STOP: u64 = 1;
/// FID 2 — `sbi_hart_get_status(hartid)`.
pub const SBI_HSM_HART_GET_STATUS: u64 = 2;

/// SBI thành công (`SBI_SUCCESS = 0`).
pub const SBI_SUCCESS: i64 = 0;
/// SBI lỗi chung (`SBI_ERR_FAILED = -1`).
pub const SBI_ERR_FAILED: i64 = -1;
/// SBI không hỗ trợ (`SBI_ERR_NOT_SUPPORTED = -2`).
pub const SBI_ERR_NOT_SUPPORTED: i64 = -2;
/// SBI tham số sai (`SBI_ERR_INVALID_PARAM = -3`) — `hart_get_status` trả mã này cho hart
/// **không có** trong domain/DTB ⇒ KT-49 từ chối hart đó (log, không panic).
pub const SBI_ERR_INVALID_PARAM: i64 = -3;
/// SBI đã sẵn sàng (`SBI_ERR_ALREADY_AVAILABLE = -6`) — `hart_start` lại hart đã start.
pub const SBI_ERR_ALREADY_AVAILABLE: i64 = -6;

/// Trạng thái hart **STARTED** (SBI HSM `sbi_hart_get_status`).
pub const SBI_HART_STARTED: i64 = 0;
/// Trạng thái hart **STOPPED** — hart có trong domain nhưng chưa được đánh thức.
pub const SBI_HART_STOPPED: i64 = 1;
/// Trạng thái hart **START_PENDING**.
pub const SBI_HART_START_PENDING: i64 = 2;
/// Trạng thái hart **STOP_PENDING**.
pub const SBI_HART_STOP_PENDING: i64 = 3;
/// Trạng thái hart **SUSPENDED**.
pub const SBI_HART_SUSPENDED: i64 = 4;
/// Trạng thái hart **SUSPEND_PENDING**.
pub const SBI_HART_SUSPEND_PENDING: i64 = 5;
/// Trạng thái hart **RESUME_PENDING**.
pub const SBI_HART_RESUME_PENDING: i64 = 6;

/// Trạng thái hart đã phân loại từ mã `sbi_hart_get_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HartStatus {
    /// Hart đang chạy (S-mode đã vào).
    Started,
    /// Hart có trong domain/DTB nhưng đang dừng — **start được** bằng HSM.
    Stopped,
    /// Hart đang chuyển trạng thái (`START_PENDING`/`STOP_PENDING`) — không start lại.
    Pending,
    /// Hart đang suspend — K3 không đánh thức (không hotplug).
    Suspended,
    /// Hart **không có** trong domain/DTB (`SBI_ERR_INVALID_PARAM`) ⇒ từ chối.
    Invalid,
    /// Lỗi SBI khác (`SBI_ERR_FAILED`, …) — từ chối + log mã lỗi, không panic.
    Error(i64),
}

/// Phân loại mã `sbi_hart_get_status` (hàm thuần, host test được).
pub const fn classify_hart_status(status: i64) -> HartStatus {
    match status {
        SBI_HART_STARTED => HartStatus::Started,
        SBI_HART_STOPPED => HartStatus::Stopped,
        SBI_HART_START_PENDING | SBI_HART_STOP_PENDING => HartStatus::Pending,
        SBI_HART_SUSPENDED | SBI_HART_SUSPEND_PENDING | SBI_HART_RESUME_PENDING => {
            HartStatus::Suspended
        }
        SBI_ERR_INVALID_PARAM => HartStatus::Invalid,
        other => HartStatus::Error(other),
    }
}

/// Quyết định của BSP cho một hart **không phải hart boot** sau khi `hart_get_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartDecision {
    /// Gọi `sbi_hart_start` (hart `STOPPED`).
    Start,
    /// Hart đã/bắt đầu chạy — không start lại, chỉ chờ online.
    AlreadyStarted,
    /// Không start: hart **không có** trong DTB/domain (`Invalid`) hoặc suspend.
    Reject,
    /// Lỗi SBI khác — log mã lỗi, không panic.
    Error(i64),
}

/// Quyết định start từ trạng thái: chỉ `STOPPED` mới gọi `hart_start`; `INVALID_PARAM` ⇒
/// [`StartDecision::Reject`] (hart vượt DTB — KT-49 từ chối, không panic cả máy).
pub const fn start_decision(status: i64) -> StartDecision {
    match classify_hart_status(status) {
        HartStatus::Stopped => StartDecision::Start,
        HartStatus::Started | HartStatus::Pending => StartDecision::AlreadyStarted,
        HartStatus::Suspended => StartDecision::Reject,
        HartStatus::Invalid => StartDecision::Reject,
        HartStatus::Error(code) => StartDecision::Error(code),
    }
}

/// `true` ⇔ mã trả về của `sbi_hart_start` được coi là **thành công** (0 hoặc "-6 đã sẵn sàng").
pub const fn hart_start_ok(code: i64) -> bool {
    code == SBI_SUCCESS || code == SBI_ERR_ALREADY_AVAILABLE
}

/// `true` ⇔ mã trả về của `sbi_hart_get_status` biểu diễn **hart có trong máy** (mọi mã ≥ 0).
pub const fn hart_present(status: i64) -> bool {
    status >= 0
}

// Bất biến bố cục: sai một trong số này thì địa chỉ stack/guard tính từ linker sẽ lệch, và
// guard của hart này có thể rơi vào stack của hart khác (rủi ro "stack AP chồng" của K3-PLAN).
const _: () = {
    assert!(MAX_HARTS == 8);
    assert!(MAX_HARTS < 64); // HartMask một word u64
    assert!(HART_GUARD_BYTES == 0x1000);
    assert!(HART_STACK_BYTES == 0x10000);
    assert!(HART_TRAP_STACK_BYTES == 0x4000);
    assert!(HART_STRIDE == 0x15000);
    assert!(HART_STRIDE % 0x1000 == 0); // mọi record bắt đầu ở biên trang
    assert!(HART_AREA_BYTES == 0xA8000);
    assert!(HART_AREA_BYTES <= 2 * 1024 * 1024); // vùng NOLOAD nằm trong 2 MiB sau ALIGN(2M)
    assert!(stack_bottom_offset(0) == HART_GUARD_BYTES);
    assert!(stack_top_offset(0) == 0x11000);
    assert!(trap_top_offset(0) == 0x15000);
    assert!(trap_top_offset(MAX_HARTS - 1) == HART_AREA_BYTES);
    assert!(guard_offset(1) == HART_STRIDE);
    assert!(is_guard_offset(0));
    assert!(is_guard_offset(HART_GUARD_BYTES - 1));
    assert!(!is_guard_offset(HART_GUARD_BYTES));
    assert!(!is_guard_offset(HART_GUARD_BYTES));
    assert!(is_guard_offset(HART_STRIDE)); // guard của hart 1
    assert!(matches!(hart_of_offset(0), Some(0)));
    assert!(matches!(hart_of_offset(HART_STRIDE), Some(1)));
    assert!(matches!(hart_of_offset(HART_AREA_BYTES), None));
    assert!(range_mask(1) == 0b1);
    assert!(range_mask(4) == 0b1111);
    assert!(range_mask(MAX_HARTS) == (1u64 << MAX_HARTS) - 1);
};
