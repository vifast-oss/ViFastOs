// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`TLBI` shootdown đa hart** — KT-262 (K4' bước 3, B3.6).
//!
//! # Vì sao phải có shootdown (không phải "cho đủ")
//!
//! `TLBI` chỉ xoá TLB của **PE thực thi nó**. Khi hart 0 sửa/xoá một lá bảng trang, TLB của
//! hart 1 vẫn giữ entry cũ ⇒ hart 1 đọc/ghi theo **bảng trang đã chết**. KT-254 đã ghi đây là
//! **nợ thật** (`VSpace::unmap` chỉ `tlbi vae1` cục bộ); KT-262 trả nợ ấy.
//!
//! # Chuỗi bắt buộc (DDI 0487)
//!
//! ```text
//!   hart sửa bảng trang (hart 0):
//!     dsb ishst        ← chờ mọi STORE vào bảng trang hoàn tất trước khi xoá TLB
//!     tlbi vae1is, VA  ← xoá entry của VA, broadcast inner-shareable
//!     dsb ish          ← chờ TLBI hoàn tất (TLBI là lệnh bất đồng bộ)
//!     isb              ← context synchronization event
//!   hart phụ (trong handler SGI):
//!     tlbi vae1is, VA  ← xoá entry trong TLB **của chính nó**
//!     dsb ish
//!     isb
//!     ack              ← sau khi cả ba bước xong mới báo
//! ```
//!
//! Hart phụ **không** có `dsb ishst`: nó không phải bên ghi bảng trang.
//!
//! # Vì sao vẫn gửi SGI khi `vae1is` đã broadcast
//!
//! `vae1is` broadcast tới mọi PE trong **inner-shareable domain** — nhưng nó **không** cho biết
//! khi nào các PE ấy đã xoá xong, và không tới được PE ngoài domain. SGI + ack biến "tôi đã
//! broadcast" thành "**mọi hart đã xoá xong**" — đó là điều `unmap` cần trước khi trả về.
//! Trên QEMU TCG, `vae1is` **đã** flush TLB của mọi CPU (`tlb_flush_page_by_mmuidx_all_cpus_synced`),
//! nên phần SGI ở đây là **dư thừa về hiệu ứng** trên QEMU — ghi trung thực ở journal
//! (`kt262-mutation.log`), không được gọi là "đã kiểm trên phần cứng".
//!
//! # Bộ đếm ack
//!
//! Một `AtomicU32` **toàn cục** (không phải per-hart): mỗi hart phụ `fetch_add(1)` **sau** khi
//! chuỗi ba bước của nó xong; hart 0 chờ `count == targets.count()` với **trần**
//! [`ACK_WAIT_LIMIT`]. Trần là bắt buộc: chờ vô hạn trên một hart không trả lời là treo im lặng
//! (R5 của thiết kế). Hết trần ⇒ [`Shootdown::timed_out`] = `true`, caller phải xử lý (ảnh: TAP
//! đỏ), **không** được coi là thành công.
//!
//! # Phạm vi
//!
//! Chỉ **một** shootdown tại một thời điểm (không lock, không preemption — kernel này chưa có
//! cả hai). `PENDING_VA` là ô chia sẻ duy nhất giữa hart gọi và handler; nó chỉ đúng vì hart 0
//! **chờ đủ ack** trước khi trả về, nên không có hai shootdown chồng nhau.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::asm;
use crate::gic;
use crate::smp::{valid_hart, MAX_HARTS};

/// Trần vòng chờ ack của một shootdown.
///
/// Đo được trên QEMU TCG (KT-259): một IPI tới hart 1 mất ~1,4×10^4 vòng; shootdown tới **ba**
/// hart phụ cộng thêm ba bước `TLBI` mỗi hart. Trần này rộng hơn **ba bậc độ lớn** so với đo
/// được, và vẫn **hữu hạn** — hết trần là đỏ, không treo.
pub const ACK_WAIT_LIMIT: u32 = 50_000_000;

/// Một bước của chuỗi `TLBI`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `dsb ishst` — chờ store bảng trang hoàn tất (chỉ hart ghi bảng trang).
    DsbIshst,
    /// `tlbi vae1is, <VA>` — xoá entry của VA, broadcast inner-shareable.
    TlbiVae1is,
    /// `dsb ish` — chờ `TLBI` hoàn tất.
    DsbIsh,
    /// `isb` — context synchronization event.
    Isb,
}

impl Step {
    /// Tên **dữ liệu** của bước (in log; đổi tên là đổi thứ người đọc thấy).
    pub const fn name(self) -> &'static str {
        match self {
            Step::DsbIshst => "dsb ishst",
            Step::TlbiVae1is => "tlbi vae1is",
            Step::DsbIsh => "dsb ish",
            Step::Isb => "isb",
        }
    }
}

/// Chuỗi **cục bộ** của hart sửa bảng trang — bốn bước, đúng thứ tự DDI 0487.
pub const LOCAL_SEQUENCE: [Step; 4] = [Step::DsbIshst, Step::TlbiVae1is, Step::DsbIsh, Step::Isb];

/// Chuỗi **trong handler SGI** của hart phụ — ba bước (không có `dsb ishst`).
pub const REMOTE_SEQUENCE: [Step; 3] = [Step::TlbiVae1is, Step::DsbIsh, Step::Isb];

/// Thực thi **một** bước của chuỗi cho `va`.
pub fn apply_step(step: Step, va: u64) {
    match step {
        Step::DsbIshst => asm::dsb_ishst(),
        Step::TlbiVae1is => asm::tlbi_vae1is(crate::tlb::tlbi_va_arg(va)),
        Step::DsbIsh => asm::dsb_ish(),
        Step::Isb => asm::isb(),
    }
}

/// Chạy chuỗi **cục bộ** ([`LOCAL_SEQUENCE`]) cho `va`.
pub fn apply_local(va: u64) {
    for step in LOCAL_SEQUENCE {
        apply_step(step, va);
    }
}

/// Chạy chuỗi **của hart phụ** ([`REMOTE_SEQUENCE`]) cho `va` — gọi từ handler SGI.
pub fn apply_remote(va: u64) {
    for step in REMOTE_SEQUENCE {
        apply_step(step, va);
    }
}

// =====================================================================================
// Tập hart đích
// =====================================================================================

/// Tập hart phụ phải nhận SGI shootdown — **mặt nạ bit** `hart id → bit`.
///
/// Mặt nạ (không phải `Vec`) vì `no_std` + không `alloc`, và vì `MAX_HARTS = 8` vừa một `u16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Targets(u16);

impl Targets {
    /// Không hart nào — đường `unmap` khi chưa có hart phụ nào online.
    pub const NONE: Self = Self(0);

    /// Dựng từ mặt nạ; bit ngoài `MAX_HARTS` bị **cắt** (không có hart ấy để gửi).
    pub const fn from_mask(mask: u16) -> Self {
        Self(mask & ((1u16 << MAX_HARTS) - 1))
    }

    /// Mọi hart **online** trừ hart gọi — đường sản phẩm của `unmap`.
    ///
    /// `hart` ngoài `MAX_HARTS` ⇒ không trừ gì (không bịa bit), `online` vẫn bị cắt theo
    /// [`Self::from_mask`].
    pub const fn all_but(hart: usize, online: u16) -> Self {
        let bit = if valid_hart(hart) { 1u16 << hart } else { 0 };
        Self::from_mask(online & !bit)
    }

    /// Mặt nạ bit (để in log / so sánh).
    pub const fn mask(self) -> u16 {
        self.0
    }

    /// Số hart đích = số bit `1` — cũng là số ack phải chờ.
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// `true` ⇔ `hart` nằm trong tập đích.
    pub const fn contains(self, hart: usize) -> bool {
        valid_hart(hart) && (self.0 & (1u16 << hart)) != 0
    }

    /// `true` ⇔ không gửi SGI nào (chờ 0 ack, trả về ngay).
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Duyệt hart đích **tăng dần** — thứ tự gửi SGI là dữ liệu, không phụ thuộc hash.
    pub fn harts(self) -> impl Iterator<Item = usize> {
        (0..MAX_HARTS).filter(move |hart| self.contains(*hart))
    }
}

// =====================================================================================
// Bộ đếm ack
// =====================================================================================

/// Bộ đếm ack **thuần** (`AtomicU32`) — tách khỏi asm để test host khẳng định được.
#[derive(Debug)]
pub struct AckCounter(AtomicU32);

impl AckCounter {
    /// Bộ đếm mới ở 0 (dùng được trong `static`).
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    /// Đưa về 0 trước một shootdown mới.
    pub fn reset(&self) {
        self.0.store(0, Ordering::Release);
    }

    /// Một hart phụ báo xong; trả số ack **sau** khi cộng.
    pub fn ack(&self) -> u32 {
        self.0.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Số ack hiện tại.
    pub fn count(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }

    /// Chờ tới khi `count >= want`; trả số vòng đã chờ, `None` nếu quá `limit`.
    ///
    /// `want == 0` trả `Some(0)` ngay — đường `Targets::NONE` không chờ vô ích.
    pub fn wait(&self, want: u32, limit: u32) -> Option<u32> {
        let mut spins = 0u32;
        while spins < limit {
            if self.count() >= want {
                return Some(spins);
            }
            spins += 1;
            core::hint::spin_loop();
        }
        None
    }
}

impl Default for AckCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Kết quả **đo được** của một shootdown — caller quyết định đỏ/xanh từ đây, không đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shootdown {
    /// VA đã xoá khỏi TLB.
    pub va: u64,
    /// Mặt nạ hart đích.
    pub targets: u16,
    /// Số ack phải có (= `targets.count()`).
    pub expected: u32,
    /// Số ack nhận được.
    pub acks: u32,
    /// Số vòng hart gọi đã chờ (hoặc [`ACK_WAIT_LIMIT`] khi hết trần).
    pub waited: u32,
    /// `true` ⇔ hết trần trước khi đủ ack.
    pub timed_out: bool,
    /// Mặt nạ hart mà `send_sgi` **thất bại** (0 = mọi SGI đã gửi được).
    pub send_failed: u16,
}

impl Shootdown {
    /// `true` ⇔ **mọi** hart đích đã ack, không timeout, không lỗi gửi.
    ///
    /// Ack **thừa** cũng là sai: mỗi hart phụ chỉ được ack đúng một lần cho một shootdown.
    pub const fn ok(&self) -> bool {
        !self.timed_out && self.send_failed == 0 && self.acks == self.expected
    }
}

// =====================================================================================
// Trạng thái chia sẻ giữa hart gọi và handler SGI
// =====================================================================================

/// VA của shootdown đang chạy — handler SGI đọc để biết phải xoá entry nào.
///
/// Chỉ **một** shootdown tại một thời điểm: hart gọi chờ đủ ack trước khi trả về, nên không có
/// hai giá trị chồng nhau. Ghi `Release` / đọc `Acquire` để giá trị thấy được trước khi SGI tới.
static PENDING_VA: AtomicU64 = AtomicU64::new(0);

/// Bộ đếm ack của shootdown đang chạy.
static ACKS: AckCounter = AckCounter::new();

/// Gửi SGI tới từng hart đích rồi chờ đủ ack — phần **thuần** của shootdown.
///
/// `send(hart)` trả `true` ⇔ SGI đã gửi được. Hàm này **không** chạm asm/MMIO, nên test host
/// gọi được với bộ gửi giả (xem `tests/shootdown.rs`) — đó là cách khẳng định giao thức ack mà
/// không cần QEMU.
///
/// Trả `(acks, send_failed, waited)`; `waited == None` ⇔ hết trần [`ACK_WAIT_LIMIT`].
pub fn collect_acks<F>(targets: Targets, acks: &AckCounter, mut send: F) -> (u32, u16, Option<u32>)
where
    F: FnMut(usize) -> bool,
{
    let mut send_failed = 0u16;
    for hart in targets.harts() {
        if !send(hart) {
            send_failed |= 1u16 << hart;
        }
    }
    let waited = acks.wait(targets.count(), ACK_WAIT_LIMIT);
    (acks.count(), send_failed, waited)
}

/// **Shootdown thật**: chuỗi cục bộ → SGI tới mọi hart đích → chờ đủ ack.
///
/// Thứ tự **bắt buộc**:
///
/// 1. `ACKS.reset()` + công bố `PENDING_VA` (Release);
/// 2. [`apply_local`] — `dsb ishst` → `tlbi vae1is` → `dsb ish` → `isb`;
/// 3. gửi `SGI_SHOOTDOWN` tới từng hart đích (hart phụ chạy [`apply_remote`] trong handler);
/// 4. chờ `acks == targets.count()` với trần [`ACK_WAIT_LIMIT`].
///
/// Bước 2 đứng **trước** bước 3 có chủ ý: `dsb ishst` của bước 2 cũng là hàng rào cho store
/// `PENDING_VA`, nên hart phụ đọc được VA đúng trước khi SGI tới.
pub fn invalidate(va: u64, targets: Targets) -> Shootdown {
    ACKS.reset();
    PENDING_VA.store(va, Ordering::Release);
    apply_local(va);
    // **KT-338c** (D2-244 tầng 1): chuông đi qua **hộp thư IPI bitmask dùng chung** — bit
    // `TlbFlush` được đặt vào hộp thư hart đích **trước** khi SGI gửi (coalesce nếu kind đã
    // chờ). Hộp thư tự gỡ bit nếu SGI không gửi được (D2-77), nên `send_failed` vẫn là số đo
    // thật.
    let (acks, send_failed, waited) = collect_acks(targets, &ACKS, |hart| {
        crate::ipi::post(hart, vifast_plat_common::ipi::IpiKind::TlbFlush).queued()
    });
    Shootdown {
        va,
        targets: targets.mask(),
        expected: targets.count(),
        acks,
        waited: waited.unwrap_or(ACK_WAIT_LIMIT),
        timed_out: waited.is_none(),
        send_failed,
    }
}

/// Handler SGI phía hart phụ: nếu `intid` là `SGI_SHOOTDOWN` thì chạy [`apply_remote`] cho VA
/// đang chờ rồi **ack**; trả `true` ⇔ đã xử lý.
///
/// **Ack sau khi chuỗi xong** (không phải trước): ack là lời hứa "TLB của tôi đã sạch", nên nó
/// phải đứng sau `dsb ish`/`isb`. Đột biến "ack trước khi TLBI" làm TAP 13 đỏ trên phần cứng
/// thật; trên QEMU TCG nó **không** quan sát được (xem journal).
pub fn handle_ipi(intid: u64) -> bool {
    if intid != gic::SGI_SHOOTDOWN as u64 {
        return false;
    }
    let va = PENDING_VA.load(Ordering::Acquire);
    apply_remote(va);
    ACKS.ack();
    true
}
