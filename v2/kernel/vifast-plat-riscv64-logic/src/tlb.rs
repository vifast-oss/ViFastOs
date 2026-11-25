// SPDX-License-Identifier: MIT OR Apache-2.0
//! ASID thật + `TlbFlush`/shootdown (KT-51) — phần **ngữ nghĩa** chạy được trên host (D2-72 Q12).
//!
//! Tệp này **không có `unsafe`** (`vifast-plat-common` là `#![forbid(unsafe_code)]`): `sfence.vma`
//! ba dạng, CSR `satp`, `rdtime`/`csr` nằm ở `kernel/vifast-plat-riscv64` (D2-17). Ở đây là những
//! gì phải **đúng trước khi** chạm phần cứng, port ngữ nghĩa K2 ([`v2/sim/vifast-sim/src/tlb.rs`],
//! [`vspace.rs`]) sang kernel thật:
//!
//! - **ASID 8 bit** (`vifast_pte::ASID_BITS`): **256** ASID mỗi thế hệ (`[`ASID_COUNT`]` đọc từ
//!   crate hình PTE, không chép tay `256`). Cấp phát bằng free-list, **không tái dùng trong cùng
//!   thế hệ**; cạn ⇒ `generation += 1`, nạp lại **đủ** 256 và báo `rollover = true` cho người gọi
//!   để phát **một** flush toàn bộ (Q4). ASID đã `free` **về hưu** cho tới hết thế hệ — đúng
//!   K2-PLAN Q4: hart đã rời VSpace có thể còn entry TLB mang `(gen, asid)` cũ; phát lại ASID
//!   trong cùng thế hệ sẽ khớp nhầm VSpace mới ⇒ lỗi "stale TLB" giả.
//! - **`FlushScope`** ba dạng đúng yêu cầu KT-51: `(asid, va)` một trang, `(asid, *)` một ASID,
//!   `(*, *)` toàn bộ (dùng cho cạn ASID/thế hệ). `code()`/`decode()` là **một** cửa mã hoá cho
//!   slot yêu cầu trong hộp thư (arch không tự bịa mã thứ hai).
//! - **`TlbFlushRequest`** = `{seq, asid, scope}`: thứ mà hộp thư IPI KT-50 (chỉ có **kind**
//!   `TlbFlush`) không mang được, nên arch giữ **một slot yêu cầu/hart** + bit kind để đánh thức.
//! - **Ack theo `seq`**: `ack(seq) = fetch_max` (đơn điệu, không lùi — D2-60/Q5). Bất biến
//!   `Ok ⇒ ack_mask == active_on`; hart gọi nằm trong `active_on` ack **ngầm** ngay sau flush cục
//!   bộ (D2-68: **không** self-target, **không** message cho chính mình).
//! - **Không chờ hart ngoài `active_on`**: [`targets`] chỉ trả hart trong `active_on` khác hart
//!   gọi; [`unused_online_harts`] trả hart online **không** thuộc `active_on` (bằng chứng TAP
//!   "hart ngoài `active_on` không bị chờ").
//! - **Bộ đếm** [`TlbCounters`]/[`TLB_STATS`] PerHart: `sent/received/flushes/acks/timeouts/
//!   errors/page_flushes/asid_flushes/all_flushes/implicit/wrong`. TAP đọc đúng các số này.
//!
//! **Không** thuộc KT-51: object layer (KT-52), U-mode (KT-53), harness đầy đủ (KT-54), conformance
//! (KT-55), `FaultKind`/W^X (KT-56), soak (KT-57), pager/COW, hotplug. Xem
//! `v2/docs/k3-riscv64.md` §14.
//!
//! [`v2/sim/vifast-sim/src/tlb.rs`]: ../../../sim/vifast-sim/src/tlb.rs
//! [`vspace.rs`]: ../../../sim/vifast-sim/src/vspace.rs

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_pte::ASID_BITS;

use vifast_plat_common::smp::{hart_bit, MAX_HARTS};

/// Số ASID mỗi thế hệ: `1 << ASID_BITS` = **256** — `[TK]`, D2-65 Q4 (đọc từ crate hình PTE).
pub const ASID_COUNT: usize = 1 << ASID_BITS;
/// Mặt nạ ASID (bit thấp) — `0xFF` với 8 bit.
pub const ASID_MASK: u64 = ASID_COUNT as u64 - 1;
/// Số word `u64` của free-list/bitmap ASID (`256 / 64 = 4`).
pub const ASID_WORDS: usize = ASID_COUNT.div_ceil(64);
/// Sentinel "chưa cấp / không có ASID" (ASID thật luôn `< ASID_COUNT`).
pub const ASID_NONE: u16 = u16::MAX;

/// Vị trí trường `ASID` trong `satp` (RV64: bit 59..44; kernel dùng 8 bit thấp của trường này).
pub const SATP_ASID_SHIFT: u32 = 44;
/// Mặt nạ trường `ASID` 16 bit của `satp` (dùng để **thay** trường, không đụng MODE/PPN).
pub const SATP_ASID_FIELD: u64 = 0xffffu64 << SATP_ASID_SHIFT;

/// Một **lease** ASID: số ASID + thế hệ lúc cấp + cờ `rollover` (lần cấp này vừa làm `generation
/// += 1`, nên người gọi **phải** phát flush toàn bộ — Q4/D2-65).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsidLease {
    /// ASID đã cấp (`0..ASID_COUNT`).
    pub asid: u16,
    /// Thế hệ lúc cấp.
    pub generation: u64,
    /// `true` ⇔ free-list cạn ở lần cấp này ⇒ thế hệ vừa tăng ⇒ flush toàn bộ.
    pub rollover: bool,
}

impl AsidLease {
    /// Lease rỗng (chưa cấp) — `asid = ASID_NONE`.
    pub const NONE: Self = Self {
        asid: ASID_NONE,
        generation: 0,
        rollover: false,
    };

    /// `true` ⇔ lease có ASID thật.
    pub const fn is_valid(self) -> bool {
        (self.asid as usize) < ASID_COUNT
    }
}

impl Default for AsidLease {
    fn default() -> Self {
        Self::NONE
    }
}

/// Ảnh chụp **trạng thái ASID toàn cục** (debug/TAP) — đúng các trường `AsidState` của K2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AsidState {
    /// Thế hệ hiện tại (`0` lúc khởi tạo; `+1` mỗi lần cạn ASID).
    pub generation: u64,
    /// Số ASID còn cấp được **trong thế hệ hiện tại** (free-list).
    pub free: usize,
    /// Số ASID **đã cấp** trong thế hệ hiện tại (cộng dồn; `free` không trả lại free-list).
    pub issued: u32,
    /// Số lần cạn ASID (⇒ `generation += 1`) từ lúc khởi tạo.
    pub exhaustions: u64,
    /// Số lần `free` hợp lệ (ASID về hưu trong thế hệ).
    pub frees: u64,
}

/// Bộ cấp ASID: free-list 256 bit + thế hệ + bộ đếm, **atomic** và **không `unsafe`**.
///
/// Một kernel có thể có nhiều allocator (mỗi VSpace/K3 chỉ một cái cho kernel VSpace; test dựng
/// instance riêng nên `cargo test` chạy song song không nhiễu nhau). Không cấp phát động: đúng
/// `const`-init cho `static`.
///
/// Khóa bằng một `AtomicBool` CAS (không spin-lock crate ngoài); mọi thao tác cấp/free đều
/// **ngắn** (một `trailing_zeros` + vài `fetch_*`), nên spin là chấp nhận được và **host test
/// kiểm được** (không dựa vào `std::sync`, D2-72 Q12).
#[derive(Debug)]
pub struct AsidAllocator {
    free: [AtomicU64; ASID_WORDS],
    retired: [AtomicU64; ASID_WORDS],
    generation: AtomicU64,
    issued: AtomicU32,
    exhaustions: AtomicU64,
    frees: AtomicU64,
    lock: AtomicBool,
}

impl AsidAllocator {
    /// Free-list của một thế hệ mới: mọi bit bật (đủ 256 ASID), chưa retire ASID nào.
    pub const fn new() -> Self {
        Self {
            free: [const { AtomicU64::new(u64::MAX) }; ASID_WORDS],
            retired: [const { AtomicU64::new(0) }; ASID_WORDS],
            generation: AtomicU64::new(0),
            issued: AtomicU32::new(0),
            exhaustions: AtomicU64::new(0),
            frees: AtomicU64::new(0),
            lock: AtomicBool::new(false),
        }
    }

    fn lock(&self) {
        while self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    /// Cấp một ASID. Free-list rỗng ⇒ `generation += 1`, nạp lại **đủ** 256, trả lease với
    /// `rollover = true` (người gọi **phải** phát flush toàn bộ — Q4). Không bao giờ thất bại.
    pub fn allocate(&self) -> AsidLease {
        self.lock();
        let mut generation = self.generation.load(Ordering::Relaxed);
        let mut rollover = false;
        // Tìm bit free thấp nhất; cạn ⇒ sang thế hệ mới rồi lấy ASID 0.
        let mut found = self.find_free();
        if found.is_none() {
            generation += 1;
            self.generation.store(generation, Ordering::Relaxed);
            for word in &self.free {
                word.store(u64::MAX, Ordering::Relaxed);
            }
            for word in &self.retired {
                word.store(0, Ordering::Relaxed);
            }
            self.issued.store(0, Ordering::Relaxed);
            self.exhaustions.fetch_add(1, Ordering::Relaxed);
            rollover = true;
            found = self.find_free();
        }
        let asid = found.unwrap_or(0);
        // `find_free` đã clear bit trong free-list ⇒ ASID này không thể cấp lại trong thế hệ.
        self.issued.fetch_add(1, Ordering::Relaxed);
        let lease = AsidLease {
            asid: asid as u16,
            generation,
            rollover,
        };
        self.unlock();
        lease
    }

    /// Clear + trả bit free thấp nhất (đã giữ khoá).
    fn find_free(&self) -> Option<usize> {
        for (index, word) in self.free.iter().enumerate() {
            let bits = word.load(Ordering::Relaxed);
            if bits != 0 {
                let offset = bits.trailing_zeros() as usize;
                word.fetch_and(!(1u64 << offset), Ordering::Relaxed);
                return Some(index * 64 + offset);
            }
        }
        None
    }

    /// **Free** một lease: ASID **về hưu** trong thế hệ hiện tại (không trả free-list — Q4).
    ///
    /// Trả `false` nếu lease sai thế hệ, ASID ngoài dải, hoặc đã free trước đó (double-free bị
    /// bắt, không âm thầm đếm hai lần).
    pub fn free(&self, lease: AsidLease) -> bool {
        if !lease.is_valid() {
            return false;
        }
        self.lock();
        let ok = if lease.generation != self.generation.load(Ordering::Relaxed) {
            false
        } else {
            let word = lease.asid as usize / 64;
            let bit = 1u64 << (lease.asid as usize % 64);
            let previous = self.retired[word].fetch_or(bit, Ordering::Relaxed);
            previous & bit == 0
        };
        if ok {
            self.frees.fetch_add(1, Ordering::Relaxed);
        }
        self.unlock();
        ok
    }

    /// Ảnh chụp trạng thái (TAP/debug).
    pub fn state(&self) -> AsidState {
        let mut free = 0usize;
        for word in &self.free {
            free += word.load(Ordering::Relaxed).count_ones() as usize;
        }
        AsidState {
            generation: self.generation.load(Ordering::Relaxed),
            free,
            issued: self.issued.load(Ordering::Relaxed),
            exhaustions: self.exhaustions.load(Ordering::Relaxed),
            frees: self.frees.load(Ordering::Relaxed),
        }
    }

    /// Thế hệ hiện tại (không lấy khoá — chỉ đọc).
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

impl Default for AsidAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Phạm vi một lệnh flush TLB — đúng **ba** dạng của KT-51/Q4.
///
/// Tương ứng ba dạng `sfence.vma` của privileged spec:
/// [`FlushScope::Page`] ⇒ `sfence.vma va, asid`; [`FlushScope::Asid`] ⇒ `sfence.vma zero, asid`;
/// [`FlushScope::All`] ⇒ `sfence.vma zero, zero` (flush mọi ASID — dùng khi cạn ASID/thế hệ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlushScope {
    /// Một trang: mọi entry của `(asid)` **phủ** `va` (kể cả entry 2 MiB/1 GiB chứa `va`).
    Page {
        /// VA nằm trong vùng cần xả (không cần căn lề — hardware tự phủ trang chứa nó).
        va: u64,
    },
    /// Một ASID: mọi entry của ASID đó (teardown VSpace).
    Asid,
    /// Toàn bộ: mọi ASID (cạn ASID/thế hệ — bắt buộc khi `generation += 1`).
    All,
}

impl FlushScope {
    /// Mã `Page`.
    pub const PAGE_CODE: u64 = 0;
    /// Mã `Asid`.
    pub const ASID_CODE: u64 = 1;
    /// Mã `All`.
    pub const ALL_CODE: u64 = 2;

    /// Mã hoá để đặt vào slot yêu cầu (một word).
    pub const fn code(self) -> u64 {
        match self {
            Self::Page { .. } => Self::PAGE_CODE,
            Self::Asid => Self::ASID_CODE,
            Self::All => Self::ALL_CODE,
        }
    }

    /// Giải mã `(code, va)` — `None` nếu mã lạ (slot hỏng ⇒ **không** flush bừa).
    pub const fn decode(code: u64, va: u64) -> Option<Self> {
        match code {
            Self::PAGE_CODE => Some(Self::Page { va }),
            Self::ASID_CODE => Some(Self::Asid),
            Self::ALL_CODE => Some(Self::All),
            _ => None,
        }
    }

    /// VA của phạm vi (`Page` ⇒ VA; hai dạng còn lại ⇒ 0).
    pub const fn va(self) -> u64 {
        match self {
            Self::Page { va } => va,
            Self::Asid | Self::All => 0,
        }
    }

    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Page { .. } => "Page",
            Self::Asid => "Asid",
            Self::All => "All",
        }
    }
}

/// Yêu cầu `TlbFlush` đầy đủ (slot yêu cầu/hart + bit kind `TlbFlush` của hộp thư IPI KT-50).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TlbFlushRequest {
    /// Số hiệu shootdown toàn cục (1-based, tăng đơn điệu) — dùng cho ack `fetch_max`.
    pub seq: u64,
    /// ASID mà lệnh áp dụng (hart nhận **adopt** ASID này trước khi flush).
    pub asid: u16,
    /// Phạm vi flush.
    pub scope: FlushScope,
}

/// Ảnh chụp một shootdown (đang chờ hoặc đã xong) — nguồn của bất biến
/// `Ok ⇒ acked == active_on`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TlbShootdownInfo {
    /// Số hiệu shootdown toàn cục.
    pub seq: u64,
    /// ASID lúc phát.
    pub asid: u16,
    /// Tên phạm vi (`Page`/`Asid`/`All`).
    pub scope: &'static str,
    /// Tập hart **phải** ack (`active_on`).
    pub targets: u64,
    /// Tập hart **đã** ack (snapshot cuối).
    pub acked: u64,
    /// `true` ⇔ `acked == targets` (Ok thật).
    pub completed: bool,
    /// Thời gian chờ (timebase tick).
    pub wait_ticks: u64,
    /// Số vòng drain của vòng chờ.
    pub rounds: u64,
}

impl Default for TlbShootdownInfo {
    fn default() -> Self {
        Self {
            seq: 0,
            asid: ASID_NONE,
            scope: "None",
            targets: 0,
            acked: 0,
            completed: true,
            wait_ticks: 0,
            rounds: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Bất biến ack (hàm thuần — host test)
// ---------------------------------------------------------------------------------------------

/// Bit các hart đã ack `seq` (`acked_seq[hart] >= seq`) — **không** xét `active_on`.
pub fn acked_mask(acked_seq: &[u64; MAX_HARTS], seq: u64) -> u64 {
    let mut mask = 0u64;
    for (hart, &acked) in acked_seq.iter().enumerate() {
        if acked >= seq {
            mask |= hart_bit(hart);
        }
    }
    mask
}

/// Bit các hart **phải** ack: `active_on` trừ hart gọi (không self-target — D2-68).
pub const fn targets(active_on: u64, caller: usize) -> u64 {
    active_on & !hart_bit(caller)
}

/// Bit hart gọi **nếu** nó thuộc `active_on` — ack **ngầm** sau flush cục bộ (D2-68).
pub const fn implicit_ack(active_on: u64, caller: usize) -> u64 {
    active_on & hart_bit(caller)
}

/// Hart **online nhưng ngoài** `active_on` — không được chờ, không được fail shootdown.
pub const fn unused_online_harts(active_on: u64, online: u64) -> u64 {
    online & !active_on
}

/// `true` ⇔ mọi hart trong `active_on` đã ack (`ack_complete` là bất biến `Ok`).
pub const fn ack_complete(acked: u64, active_on: u64) -> bool {
    acked & active_on == active_on
}

/// Số hart **còn thiếu** ack.
pub const fn ack_missing(acked: u64, active_on: u64) -> u64 {
    active_on & !acked
}

// ---------------------------------------------------------------------------------------------
// Bộ đếm (PerHart)
// ---------------------------------------------------------------------------------------------

/// Ảnh chụp bộ đếm TLB của **một hart** — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TlbSnapshot {
    /// Số yêu cầu `TlbFlush` gửi **remote** (post + SBI IPI, không tính ngầm/self).
    pub sent: u64,
    /// Số yêu cầu `TlbFlush` đã xử lý (drain + flush + ack).
    pub received: u64,
    /// Số lần `sfence.vma` thật do flush (cục bộ của hart gọi hoặc do yêu cầu).
    pub flushes: u64,
    /// Số lần ack (tăng `acked_seq`), kể cả ack ngầm của hart gọi.
    pub acks: u64,
    /// Số shootdown hết timeout trên hart này.
    pub timeouts: u64,
    /// Số lỗi (hart đích offline, slot hỏng, trùng shootdown…).
    pub errors: u64,
    /// Số flush phạm vi `Page`.
    pub page_flushes: u64,
    /// Số flush phạm vi `Asid`.
    pub asid_flushes: u64,
    /// Số flush phạm vi `All`.
    pub all_flushes: u64,
    /// Số ack **ngầm** (hart gọi nằm trong `active_on` — D2-68).
    pub implicit: u64,
    /// Số lần kiểm bằng chứng thất bại (TAP: stale mapping còn sống, hart ngoài bị chờ…).
    pub wrong: u64,
}

impl TlbSnapshot {
    /// Tổng số flush mọi phạm vi (= `page + asid + all`).
    pub const fn flush_total(self) -> u64 {
        self.page_flushes + self.asid_flushes + self.all_flushes
    }
}

/// Bộ đếm TLB của **một hart**: `sent` theo hart gửi, `received/flushes/acks` theo hart nhận.
#[derive(Debug)]
pub struct TlbCounters {
    sent: AtomicU64,
    received: AtomicU64,
    flushes: AtomicU64,
    acks: AtomicU64,
    timeouts: AtomicU64,
    errors: AtomicU64,
    page_flushes: AtomicU64,
    asid_flushes: AtomicU64,
    all_flushes: AtomicU64,
    implicit: AtomicU64,
    wrong: AtomicU64,
}

impl TlbCounters {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
            flushes: AtomicU64::new(0),
            acks: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            page_flushes: AtomicU64::new(0),
            asid_flushes: AtomicU64::new(0),
            all_flushes: AtomicU64::new(0),
            implicit: AtomicU64::new(0),
            wrong: AtomicU64::new(0),
        }
    }

    /// Ghi một yêu cầu gửi remote (post + SBI IPI).
    pub fn record_sent(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một yêu cầu đã nhận + đã flush + đã ack.
    pub fn record_received(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một lần `sfence.vma` thật, theo phạm vi.
    pub fn record_flush(&self, scope: FlushScope) {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        match scope {
            FlushScope::Page { .. } => self.page_flushes.fetch_add(1, Ordering::Relaxed),
            FlushScope::Asid => self.asid_flushes.fetch_add(1, Ordering::Relaxed),
            FlushScope::All => self.all_flushes.fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Ghi một ack **tường minh** (do yêu cầu).
    pub fn record_ack(&self) {
        self.acks.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một ack **ngầm** (hart gọi trong `active_on` — D2-68).
    pub fn record_implicit(&self) {
        self.acks.fetch_add(1, Ordering::Relaxed);
        self.implicit.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một timeout.
    pub fn record_timeout(&self) {
        self.timeouts.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một lỗi.
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một bằng chứng sai (TAP).
    pub fn record_wrong(&self) {
        self.wrong.fetch_add(1, Ordering::Relaxed);
    }

    /// Ảnh chụp bộ đếm.
    pub fn snapshot(&self) -> TlbSnapshot {
        TlbSnapshot {
            sent: self.sent.load(Ordering::Relaxed),
            received: self.received.load(Ordering::Relaxed),
            flushes: self.flushes.load(Ordering::Relaxed),
            acks: self.acks.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            page_flushes: self.page_flushes.load(Ordering::Relaxed),
            asid_flushes: self.asid_flushes.load(Ordering::Relaxed),
            all_flushes: self.all_flushes.load(Ordering::Relaxed),
            implicit: self.implicit.load(Ordering::Relaxed),
            wrong: self.wrong.load(Ordering::Relaxed),
        }
    }
}

impl Default for TlbCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm TLB **theo hart** — mỗi hart một [`TlbCounters`].
#[derive(Debug)]
pub struct PerHartTlbStats {
    slots: [TlbCounters; MAX_HARTS],
}

impl PerHartTlbStats {
    /// Bộ đếm rỗng cho mọi hart.
    pub const fn new() -> Self {
        Self {
            slots: [const { TlbCounters::new() }; MAX_HARTS],
        }
    }

    /// Bộ đếm của `hart`; hart ngoài dải ⇒ slot 0 (không bao giờ xảy ra với hart thật).
    pub fn get(&self, hart: usize) -> &TlbCounters {
        &self.slots[if hart < MAX_HARTS { hart } else { 0 }]
    }

    /// Bộ đếm của `hart` nếu hợp lệ.
    pub fn try_get(&self, hart: usize) -> Option<&TlbCounters> {
        self.slots.get(hart)
    }

    /// Tổng số yêu cầu đã gửi remote trên mọi hart.
    pub fn total_sent(&self) -> u64 {
        self.slots.iter().map(|slot| slot.snapshot().sent).sum()
    }

    /// Tổng số yêu cầu đã xử lý trên mọi hart.
    pub fn total_received(&self) -> u64 {
        self.slots.iter().map(|slot| slot.snapshot().received).sum()
    }

    /// Tổng số timeout trên mọi hart.
    pub fn total_timeouts(&self) -> u64 {
        self.slots.iter().map(|slot| slot.snapshot().timeouts).sum()
    }
}

impl Default for PerHartTlbStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm TLB toàn cục của ảnh kernel (PerHart).
pub static TLB_STATS: PerHartTlbStats = PerHartTlbStats::new();

// Bất biến hằng số: sai một trong số này thì satp/hộp thư/ack không còn khớp nhau.
const _: () = {
    assert!(ASID_BITS == 8);
    assert!(ASID_COUNT == 256);
    assert!(ASID_MASK == 0xff);
    assert!((ASID_COUNT & 63) == 0); // mọi word free-list đầy 64 bit
    assert!(ASID_WORDS == 4);
    assert!((ASID_NONE as usize) >= ASID_COUNT);
    assert!(SATP_ASID_SHIFT == 44);
    assert!(SATP_ASID_FIELD == 0x0fff_f000_0000_0000);
    assert!(SATP_ASID_FIELD >> SATP_ASID_SHIFT == 0xffff);
    assert!(!AsidLease::NONE.is_valid());
    assert!(AsidLease {
        asid: 0,
        generation: 0,
        rollover: false
    }
    .is_valid());
    assert!(AsidLease {
        asid: 255,
        generation: 7,
        rollover: true
    }
    .is_valid());
    assert!(FlushScope::PAGE_CODE == 0);
    assert!(FlushScope::ASID_CODE == 1);
    assert!(FlushScope::ALL_CODE == 2);
    assert!(matches!(
        FlushScope::decode(FlushScope::PAGE_CODE, 0x1000),
        Some(FlushScope::Page { va }) if va == 0x1000
    ));
    assert!(matches!(
        FlushScope::decode(FlushScope::ASID_CODE, 0),
        Some(FlushScope::Asid)
    ));
    assert!(matches!(
        FlushScope::decode(FlushScope::ALL_CODE, 0),
        Some(FlushScope::All)
    ));
    assert!(FlushScope::decode(3, 0).is_none());
    assert!(targets(0b11, 0) == 0b10);
    assert!(implicit_ack(0b11, 0) == 0b1);
    assert!(implicit_ack(0b10, 0) == 0);
    assert!(unused_online_harts(0b11, 0b1111) == 0b1100);
    assert!(ack_complete(0b11, 0b11));
    assert!(!ack_complete(0b01, 0b11));
    assert!(ack_missing(0b01, 0b11) == 0b10);
};
