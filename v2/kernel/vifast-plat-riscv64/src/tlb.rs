// SPDX-License-Identifier: MIT OR Apache-2.0
//! ASID thật + `sfence.vma` + shootdown ack trên QEMU `virt` (KT-51).
//!
//! Phần **ngữ nghĩa** (ASID allocator/thế hệ, `FlushScope`, bất biến ack, bộ đếm, "hart ngoài
//! `active_on` không bị chờ") nằm ở [`vifast_plat_riscv64_logic::tlb`] — test được trên host. Tệp này là
//! phần **không viết "safe" được** (D2-17, `unsafe` thuộc `kernel/vifast-plat-*`):
//!
//! - **`sfence.vma` ba dạng** đúng `rs1`/`rs2` của privileged spec:
//!   [`sfence_page`] = `sfence.vma va, asid` (rs1 = VA, rs2 = ASID), [`sfence_asid`] =
//!   `sfence.vma zero, asid` (rs1 = `x0` ⇒ mọi VA của ASID), [`sfence_all`] = `sfence.vma`
//!   (rs1 = rs2 = `x0` ⇒ mọi ASID). Không dùng lẫn tham số: `sfence.vma` lấy **rs1 = địa chỉ**,
//!   **rs2 = ASID** (đảo thứ tự là lỗi im lặng).
//! - **`satp.ASID`**: [`mmu::set_satp_asid`] ghi lại `satp` với ASID của lease kernel (giữ MODE/PPN);
//!   **không** `sfence` ở đây vì đổi thẻ ASID không làm sai translation — entry cũ mang thẻ cũ
//!   không khớp thẻ mới; chỉ khi ASID được **phát lại** ở thế hệ mới mới cần flush toàn bộ (và
//!   `generation += 1` luôn phát `FlushScope::All`). Ghi chú này là lý do kỹ thuật, không phải
//!   bỏ sót.
//! - **Slot yêu cầu/hart**: hộp thư IPI KT-50 chỉ mang **kind** (`TlbFlush`), nên mỗi hart có một
//!   slot `{seq, asid, scope, va}` (4 `AtomicU64`). Người gửi ghi `asid/scope/va` → ghi `seq`
//!   (Release) → `post(TlbFlush)` (AcqRel) → IPI; người nhận `drain` thấy kind, đọc `seq`
//!   (Acquire) rồi đọc các trường ⇒ **không** bao giờ thấy slot nửa vời. Gửi trùng/coalesce không
//!   mất yêu cầu: slot giữ **seq mới nhất**, ack `fetch_max` nên `ack >= seq` là đủ.
//! - **Ack ngầm cho hart gọi** (D2-68): hart gọi **luôn** flush cục bộ trước (page table của chính
//!   nó vừa đổi) rồi — nếu nằm trong `active_on` — tự ghi `acked_seq = seq` **không** gửi message
//!   cho chính mình. `targets()` của `plat-common` loại hart gọi khỏi danh sách gửi, nên
//!   **không self-target**; bản self-target sẽ làm mutation "bỏ flush cục bộ" sống (D2-68).
//! - **Chờ ack**: mỗi bước **drain hộp thư của chính hart** (kể cả khi `SIE = 0` — luật chống
//!   deadlock D2-68/§4.10.3) rồi kiểm `ack_mask == active_on`; hết deadline (`rdtime`) ⇒
//!   `TimedOut` + đếm `timeouts`, **không treo**. `ack_mask` chỉ tính hart trong `active_on`
//!   (hart online ngoài `active_on` **không** bị chờ), và **luôn** kiểm lại `acked == active_on`
//!   trước khi trả `Completed`.
//! - **Cạn ASID**: [`replace_lease`] nhận lease `rollover` ⇒ `generation += 1` **và** phát **một**
//!   `FlushScope::All` tới `active_on` (Q4: "cạn ⇒ generation+=1 + flush toàn bộ").
//!
//! Đăng ký hart vào `active_on`: [`init`] (BSP) và [`activate`] (hart thứ cấp sau `adopt_satp`).
//!
//! **Không** thuộc KT-51: object layer/Retype (KT-52), U-mode (KT-53), harness đầy đủ (KT-54),
//! conformance (KT-55), `FaultKind`/W^X (KT-56), soak (KT-57), pager/COW, nhiều VSpace thật.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_plat_common::ipi::{IpiKind, KindSet};
use vifast_plat_common::smp::{hart_bit, mask_count, HartMask, MAX_HARTS};
use vifast_pte::sv39::satp_asid;
#[cfg(feature = "ktest")]
use vifast_pte::sv39::{PTE_EXEC, PTE_READ, PTE_WRITE};
// **KT-409b**: phần ngữ nghĩa TLB/ASID nay ở `vifast_plat_riscv64_logic::tlb` (dời khỏi `plat-common`).
use vifast_plat_common::Platform;
pub use vifast_plat_riscv64_logic::tlb::*;

use crate::ipi;
use crate::mmu;
use crate::timer;
use crate::Riscv64Virt;

// ---------------------------------------------------------------------------------------------
// sfence.vma — ba dạng (rs1 = VA, rs2 = ASID)
// ---------------------------------------------------------------------------------------------

/// `sfence.vma va, asid` — xả mọi entry **phủ** `va` của ASID đó (kể cả leaf 2 MiB/1 GiB).
///
/// `va` nằm ở **rs1**, ASID ở **rs2** — đúng thứ tự của privileged spec; đảo lại là lỗi im lặng
/// (flush nhầm trang/asid). `va = 0` với rs1 **khác** `x0` vẫn là "trang chứa địa chỉ 0" — dùng
/// [`sfence_asid`] khi muốn xả cả ASID.
pub fn sfence_page(asid: u16, va: u64) {
    // SAFETY: `sfence.vma` không truy cập bộ nhớ của kernel, chỉ xả TLB của hart hiện tại;
    // `rs1 = va` (thanh ghi tổng quát, không phải `x0`), `rs2 = asid`.
    unsafe {
        asm!(
            "sfence.vma {va}, {asid}",
            va = in(reg) va,
            asid = in(reg) asid as u64,
            options(nostack, preserves_flags),
        );
    }
}

/// `sfence.vma zero, asid` — xả **mọi** entry của một ASID (teardown VSpace).
pub fn sfence_asid(asid: u16) {
    // SAFETY: rs1 = `x0` (mọi VA), rs2 = ASID; chỉ xả TLB của hart hiện tại.
    unsafe {
        asm!(
            "sfence.vma zero, {asid}",
            asid = in(reg) asid as u64,
            options(nostack, preserves_flags),
        );
    }
}

/// `sfence.vma` — rs1 = rs2 = `x0`: xả **toàn bộ** TLB của hart hiện tại (mọi ASID).
pub fn sfence_all() {
    // SAFETY: dạng `sfence.vma zero, zero`; chỉ xả TLB của hart hiện tại.
    unsafe {
        asm!("sfence.vma", options(nostack, preserves_flags));
    }
}

/// Flush cục bộ theo phạm vi (dùng chung cho hart gọi và hart nhận yêu cầu).
pub fn flush_scope(scope: FlushScope, asid: u16) {
    match scope {
        FlushScope::Page { va } => sfence_page(asid, va),
        FlushScope::Asid => sfence_asid(asid),
        FlushScope::All => sfence_all(),
        // **KT-415** — range: `sfence.vma va, asid` cho **từng trang** (một yêu cầu, một ACK).
        FlushScope::Range { start, len } => {
            let pages = if len == 0 {
                1
            } else {
                len.div_ceil(FlushScope::PAGE_BYTES)
            };
            for page in 0..pages {
                sfence_page(asid, start + page * FlushScope::PAGE_BYTES);
            }
        }
    }
}

/// Flush cục bộ **tính vào bộ đếm** của hart đang chạy (`flushes` + `page/asid/all`).
fn record_local_flush(scope: FlushScope) {
    TLB_STATS.get(current_hart()).record_flush(scope);
}

/// Hart đang chạy (`tp` do `_start`/`_start_secondary` đặt từ `a0`).
fn current_hart() -> usize {
    Riscv64Virt.hart_id()
}

// ---------------------------------------------------------------------------------------------
// Trạng thái lease kernel + active_on
// ---------------------------------------------------------------------------------------------

/// Bộ cấp ASID của **kernel VSpace** (K3: đúng một VSpace; K3b tách allocator/VSpace).
static ASIDS: AsidAllocator = AsidAllocator::new();
/// ASID của lease kernel hiện tại (`ASID_NONE` = chưa `init`).
static LEASE_ASID: AtomicU64 = AtomicU64::new(ASID_NONE as u64);
/// Thế hệ của lease kernel hiện tại.
static LEASE_GEN: AtomicU64 = AtomicU64::new(0);
/// Tập hart đang nạp ASID kernel (`⊆` hart online) — phạm vi `active_on` của shootdown.
static ACTIVE_ON: HartMask = HartMask::new();
/// Số hiệu shootdown toàn cục (1-based, tăng đơn điệu).
static SEQ: AtomicU64 = AtomicU64::new(0);
/// Chỉ **một** shootdown tại một thời điểm (K3 chưa có nhiều VSpace song song): hai shootdown
/// chồng nhau sẽ dùng chung `seq`/slot đang ghi — khoá này biến lỗi thành `Busy` thay vì sai số.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Báo cáo `init`: số đo thật do lần boot sinh ra (không viết tay trong tài liệu).
#[derive(Clone, Copy, Debug)]
pub struct TlbInitReport {
    /// ASID của lease kernel.
    pub asid: u16,
    /// Thế hệ của lease.
    pub generation: u64,
    /// Lease đầu tiên có làm tăng thế hệ không (không bao giờ ở boot sạch; có thì phải đã flush).
    pub rollover: bool,
    /// `satp` sau khi ghi ASID (đọc lại từ CSR).
    pub satp: u64,
    /// ASID đọc lại từ `satp` (phải bằng `asid`).
    pub satp_asid: u16,
    /// Tập hart `active_on` tại thời điểm init.
    pub active_on: u64,
    /// Trạng thái allocator (gen/free/issued/exhaustions).
    pub state: AsidState,
}

/// Kết quả một shootdown — TAP đọc để phân biệt Ok / timeout / đang bận.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShootdownOutcome {
    /// Đủ ack (`ack_mask == active_on`) — bất biến Q5.
    Completed,
    /// Hết deadline mà vẫn thiếu ack (hart chậm/chết/offline) — **không treo**.
    TimedOut,
    /// Đã có shootdown khác đang chạy (K3 chưa cần hai shootdown song song).
    Busy,
}

impl ShootdownOutcome {
    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Completed => "Ok",
            Self::TimedOut => "Timeout",
            Self::Busy => "Busy",
        }
    }
}

/// Kết quả [`replace_lease`]: lease mới + (nếu `rollover`) kết quả flush toàn bộ.
#[derive(Clone, Copy, Debug)]
pub struct ReplaceOutcome {
    /// ASID đang dùng sau khi thay.
    pub asid: u16,
    /// Thế hệ sau khi thay.
    pub generation: u64,
    /// `true` ⇔ lease mới làm `generation += 1` ⇒ đã phát flush toàn bộ.
    pub rollover: bool,
    /// Kết quả flush toàn bộ (`None` nếu không rollover).
    pub flush: Option<ShootdownOutcome>,
}

/// Một slot yêu cầu `TlbFlush` của hart — 4 word, `const`-init (không cấp phát).
#[derive(Debug)]
struct FlushSlot {
    seq: AtomicU64,
    asid: AtomicU64,
    scope: AtomicU64,
    va: AtomicU64,
    /// Độ dài range (KT-415); `0` với mọi phạm vi cũ.
    len: AtomicU64,
}

impl FlushSlot {
    const fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
            asid: AtomicU64::new(0),
            scope: AtomicU64::new(0),
            va: AtomicU64::new(0),
            len: AtomicU64::new(0),
        }
    }
}

/// Slot yêu cầu `TlbFlush` theo hart.
static SLOTS: [FlushSlot; MAX_HARTS] = [const { FlushSlot::new() }; MAX_HARTS];
/// `fetch_max` của các `ack` đã xử lý theo hart (`acked_seq[h] < seq` ⇔ chưa ack seq đó).
static ACKED: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];

// Ảnh chụp shootdown gần nhất (TAP đọc; không cần khoá vì chỉ ghi 1 hart/lần rồi in).
static LAST_SEQ: AtomicU64 = AtomicU64::new(0);
static LAST_ASID: AtomicU64 = AtomicU64::new(ASID_NONE as u64);
static LAST_SCOPE: AtomicU64 = AtomicU64::new(0);
static LAST_TARGETS: AtomicU64 = AtomicU64::new(0);
static LAST_ACKED: AtomicU64 = AtomicU64::new(0);
static LAST_COMPLETED: AtomicU64 = AtomicU64::new(1);
static LAST_WAIT: AtomicU64 = AtomicU64::new(0);
static LAST_ROUNDS: AtomicU64 = AtomicU64::new(0);
static SHOOTDOWNS: AtomicU64 = AtomicU64::new(0);

/// ASID của lease kernel hiện tại (`ASID_NONE` nếu chưa [`init`]).
pub fn asid() -> u16 {
    LEASE_ASID.load(Ordering::SeqCst) as u16
}

/// Thế hệ của lease kernel hiện tại.
pub fn generation() -> u64 {
    LEASE_GEN.load(Ordering::SeqCst)
}

/// Tập hart đang nạp ASID kernel (`active_on`).
pub fn active_on() -> u64 {
    ACTIVE_ON.bits()
}

/// Hart có đang nạp ASID kernel không.
pub fn is_active(hart: usize) -> bool {
    ACTIVE_ON.contains(hart)
}

/// Lease kernel hiện tại.
pub fn lease() -> AsidLease {
    AsidLease {
        asid: asid(),
        generation: generation(),
        rollover: false,
    }
}

/// Trạng thái allocator ASID (gen/free/issued/exhaustions/frees).
pub fn asid_state() -> AsidState {
    ASIDS.state()
}

/// Số hiệu shootdown gần nhất đã phát.
pub fn last_seq() -> u64 {
    SEQ.load(Ordering::SeqCst)
}

/// Tổng số shootdown đã phát.
pub fn shootdown_count() -> u64 {
    SHOOTDOWNS.load(Ordering::SeqCst)
}

/// `acked_seq` của `hart` (0 = chưa ack gì; TAP in bằng chứng).
pub fn acked_seq(hart: usize) -> u64 {
    if hart < MAX_HARTS {
        ACKED[hart].load(Ordering::SeqCst)
    } else {
        0
    }
}

/// `seq` đang nằm trong slot yêu cầu của `hart` (`0` = chưa có yêu cầu nào).
///
/// TAP dùng để chứng minh **không self-target**: slot của hart gọi luôn `0`.
pub fn request_seq(hart: usize) -> u64 {
    if hart < MAX_HARTS {
        SLOTS[hart].seq.load(Ordering::Acquire)
    } else {
        0
    }
}

/// Mask hart đã ack `seq` (chỉ đọc; không bao gồm logic `active_on`).
pub fn ack_mask(seq: u64) -> u64 {
    let mut acked = [0u64; MAX_HARTS];
    for (hart, slot) in acked.iter_mut().enumerate() {
        *slot = ACKED[hart].load(Ordering::SeqCst);
    }
    acked_mask(&acked, seq)
}

/// Ảnh chụp shootdown **gần nhất**.
pub fn last_shootdown() -> TlbShootdownInfo {
    let scope = FlushScope::decode(LAST_SCOPE.load(Ordering::SeqCst), 0, 0)
        .map(|scope| scope.name())
        .unwrap_or("Invalid");
    TlbShootdownInfo {
        seq: LAST_SEQ.load(Ordering::SeqCst),
        asid: LAST_ASID.load(Ordering::SeqCst) as u16,
        scope,
        targets: LAST_TARGETS.load(Ordering::SeqCst),
        acked: LAST_ACKED.load(Ordering::SeqCst),
        completed: LAST_COMPLETED.load(Ordering::SeqCst) != 0,
        wait_ticks: LAST_WAIT.load(Ordering::SeqCst),
        rounds: LAST_ROUNDS.load(Ordering::SeqCst),
    }
}

// ---------------------------------------------------------------------------------------------
// Init / activate / lease
// ---------------------------------------------------------------------------------------------

/// Cấp ASID đầu tiên cho kernel VSpace, ghi `satp.ASID`, đăng ký hart hiện tại vào `active_on`.
///
/// Gọi trên **BSP** sau `mmu::init()` và **trước** `smp::init()` để hart thứ cấp `adopt_satp`
/// nhận luôn `satp` có ASID đúng. Lease đầu tiên không bao giờ `rollover`; nếu có (allocator đã
/// cạn từ trước — không xảy ra ở boot) thì flush toàn bộ ngay để giữ bất biến "thế hệ mới ⇒ TLB
/// sạch".
pub fn init() -> TlbInitReport {
    let hart = current_hart();
    let lease = ASIDS.allocate();
    LEASE_ASID.store(lease.asid as u64, Ordering::SeqCst);
    LEASE_GEN.store(lease.generation, Ordering::SeqCst);
    ACTIVE_ON.insert(hart);
    if lease.rollover {
        sfence_all();
        record_local_flush(FlushScope::All);
    }
    // Ghi `satp` với ASID; TAP đọc lại `satp_asid` để kết luận ghi được hay không.
    let _ = mmu::set_satp_asid(lease.asid);
    TlbInitReport {
        asid: lease.asid,
        generation: lease.generation,
        rollover: lease.rollover,
        satp: mmu::satp_now(),
        satp_asid: mmu::current_satp_asid(),
        active_on: active_on(),
        state: ASIDS.state(),
    }
}

/// Hart thứ cấp **tham gia** `active_on` sau khi `adopt_satp` (SBI HSM đánh thức với `satp = 0`).
///
/// Trả `true` ⇔ hart đã nạp đúng `satp.ASID` của lease kernel (nếu lệch, ghi lại rồi kiểm).
pub fn activate(hart: usize) -> bool {
    let asid = asid();
    if asid == ASID_NONE {
        return false;
    }
    if mmu::current_satp_asid() != asid && !mmu::set_satp_asid(asid) {
        return false;
    }
    ACTIVE_ON.insert(hart);
    mmu::current_satp_asid() == asid
}

/// Cấp thêm một lease (dùng cho test cạn ASID và cho K3b khi tạo VSpace mới).
pub fn allocate_lease() -> AsidLease {
    ASIDS.allocate()
}

/// Free một lease (ASID về hưu trong thế hệ — không trả free-list; Q4).
pub fn free_lease(lease: AsidLease) -> bool {
    ASIDS.free(lease)
}

/// Thay lease kernel bằng `lease`, áp ASID lên hart hiện tại và — nếu `rollover` — phát **một**
/// `FlushScope::All` tới `active_on` (cạn ASID ⇒ `generation += 1` + flush toàn bộ, Q4).
///
/// `timeout_ticks` tính bằng timebase (10 MHz) cho lần flush toàn bộ.
pub fn replace_lease(lease: AsidLease, timeout_ticks: u64) -> ReplaceOutcome {
    LEASE_ASID.store(lease.asid as u64, Ordering::SeqCst);
    LEASE_GEN.store(lease.generation, Ordering::SeqCst);
    let _ = mmu::set_satp_asid(lease.asid);
    let flush = if lease.rollover {
        Some(shootdown(FlushScope::All, timeout_ticks))
    } else {
        None
    };
    ReplaceOutcome {
        asid: lease.asid,
        generation: lease.generation,
        rollover: lease.rollover,
        flush,
    }
}

// ---------------------------------------------------------------------------------------------
// Gửi yêu cầu + xử lý yêu cầu
// ---------------------------------------------------------------------------------------------

/// Ghi yêu cầu vào slot của `hart` rồi `post(TlbFlush)` — **không** self-target ở tầng này
/// (`targets()`/`implicit_ack()` của `plat-common` đã loại hart gọi).
fn post_request(hart: usize, seq: u64, asid: u16, scope: FlushScope) -> ipi::PostOutcome {
    if hart >= MAX_HARTS {
        return ipi::PostOutcome::Invalid;
    }
    // Ghi tham số trước, `seq` sau (Release) — người nhận đọc `seq` (Acquire) rồi mới đọc tham
    // số nên không bao giờ thấy nửa vời. `va`/`scope` là dữ liệu thuần, `seq` là cờ "đã sẵn".
    SLOTS[hart].asid.store(asid as u64, Ordering::Relaxed);
    SLOTS[hart].scope.store(scope.code(), Ordering::Relaxed);
    SLOTS[hart].va.store(scope.va(), Ordering::Relaxed);
    SLOTS[hart].len.store(scope.range_len(), Ordering::Relaxed);
    SLOTS[hart].seq.store(seq, Ordering::Release);
    let outcome = ipi::post(hart, IpiKind::TlbFlush);
    if outcome == ipi::PostOutcome::Invalid {
        // D2-77: không giữ yêu cầu mồ côi khi hart đích offline/không post được.
        SLOTS[hart].seq.store(0, Ordering::Release);
    }
    outcome
}

/// Xử lý yêu cầu `TlbFlush` của **hart đang chạy** nếu có (gọi từ mọi đường `drain`).
///
/// Trả `true` ⇔ vừa xử lý một yêu cầu **mới** (đã flush + ack). Hart nhận **adopt** ASID trong
/// yêu cầu trước khi flush (giữ mọi hart cùng thế hệ ASID — nếu thiếu bước này, một hart còn thẻ
/// ASID cũ sẽ không bị `sfence (asid mới, va)` xả ⇒ stale TLB thật).
pub fn handle_kinds(kinds: KindSet) -> bool {
    if !kinds.contains(IpiKind::TlbFlush) {
        return false;
    }
    let hart = current_hart();
    if hart >= MAX_HARTS {
        return false;
    }
    let seq = SLOTS[hart].seq.load(Ordering::Acquire);
    if seq == 0 {
        return false;
    }
    if seq <= ACKED[hart].load(Ordering::Acquire) {
        // Yêu cầu cũ/trùng (coalesce hoặc đã xử lý): không flush lại, không ack lùi.
        return false;
    }
    let asid = SLOTS[hart].asid.load(Ordering::Relaxed) as u16;
    let code = SLOTS[hart].scope.load(Ordering::Relaxed);
    let va = SLOTS[hart].va.load(Ordering::Relaxed);
    let len = SLOTS[hart].len.load(Ordering::Relaxed);
    let Some(scope) = FlushScope::decode(code, va, len) else {
        // Slot hỏng: không flush bừa, nhưng phải ack để người gửi không treo; đếm lỗi.
        TLB_STATS.get(hart).record_error();
        ACKED[hart].fetch_max(seq, Ordering::AcqRel);
        return false;
    };
    let request = TlbFlushRequest { seq, asid, scope };
    if request.asid != mmu::current_satp_asid() {
        // Không panic nếu ghi hụt: flush vẫn chạy theo ASID trong yêu cầu, bộ đếm ghi lỗi.
        if !mmu::set_satp_asid(request.asid) {
            TLB_STATS.get(hart).record_error();
        }
    }
    flush_scope(request.scope, request.asid);
    record_local_flush(request.scope);
    TLB_STATS.get(hart).record_received();
    ACKED[hart].fetch_max(seq, Ordering::AcqRel);
    TLB_STATS.get(hart).record_ack();
    true
}

/// **Chỉ ảnh test** (D2-78): đặt lại `ACTIVE_ON` và trả mask **mới**; người gọi tự lưu mask cũ để
/// khôi phục. Đây là **trạng thái thật** của kernel VSpace (không phải mask truyền vào shootdown) —
/// dùng để chứng minh hart đã gỡ khỏi `active_on` không nhận/không ack và shootdown không chờ nó.
#[cfg(feature = "ktest")]
pub fn test_active_on_set(mask: u64) -> u64 {
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        ACTIVE_ON.set(hart, mask & hart_bit(hart) != 0);
        hart += 1;
    }
    ACTIVE_ON.bits()
}

/// Vòng phục vụ yêu cầu: mỗi bước **drain** hộp thư của hart hiện tại (xử lý `TlbFlush` nếu có)
/// rồi kiểm `ready`; hết `timeout_ticks` trả `false` (**không treo**).
pub fn service_until(timeout_ticks: u64, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = timer::now_ticks().saturating_add(timeout_ticks);
    loop {
        let _ = ipi::drain_current();
        if ready() {
            return true;
        }
        if timer::now_ticks() >= deadline {
            return false;
        }
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------------------------
// Shootdown
// ---------------------------------------------------------------------------------------------

/// Shootdown một phạm vi tới **`active_on()` của kernel VSpace** — **D2-78**: API **không** nhận
/// mask từ ngoài, nên không thể chờ (hoặc bỏ chờ) sai tập hart; tập hart luôn là trạng thái thật
/// đang nạp ASID.
///
/// Trình tự: flush cục bộ hart gọi → ack ngầm (nếu hart gọi thuộc `active_on`) → gửi `TlbFlush` cho
/// các hart **khác** trong `active_on` → chờ `ack_mask == active_on` (drain mỗi bước, timeout bằng
/// `rdtime`). Trả [`ShootdownOutcome::Completed`] **chỉ khi** `acked == active_on` (bất biến Q5);
/// timeout thì trả `TimedOut` + đếm, không panic/không treo.
pub fn shootdown(scope: FlushScope, timeout_ticks: u64) -> ShootdownOutcome {
    let me = current_hart();
    let active_on_mask = active_on();
    if IN_FLIGHT
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        TLB_STATS.get(me).record_error();
        return ShootdownOutcome::Busy;
    }
    // `IN_FLIGHT` chỉ được đặt ở đây và **chỉ** nhả ở cuối hàm (một đường thoát duy nhất: vòng chờ
    // luôn thoát bằng `Completed`/`TimedOut`). Kernel `no_std` + `panic = abort`: panic giữa đường là
    // halt toàn máy (không có unwinding để kẹt khoá); `Busy` **không** đặt cờ và đường thành công
    // luôn đi qua phần nhả cờ dưới cuối hàm.
    let seq = SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    SHOOTDOWNS.fetch_add(1, Ordering::SeqCst);
    let asid = asid();
    let counters = TLB_STATS.get(me);
    let start = timer::now_ticks();

    // 1) Hart gọi **luôn** flush cục bộ (page table của chính nó vừa đổi), rồi ack ngầm nếu thuộc
    //    `active_on` — D2-68: **không** gửi message cho chính mình.
    flush_scope(scope, asid);
    counters.record_flush(scope);
    if implicit_ack(active_on_mask, me) != 0 {
        ACKED[me].store(seq, Ordering::SeqCst);
        counters.record_implicit();
    }

    // 2) Gửi yêu cầu cho các hart **khác** trong `active_on` (không self-target).
    let remote = targets(active_on_mask, me);
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        if remote & hart_bit(hart) != 0 {
            match post_request(hart, seq, asid, scope) {
                ipi::PostOutcome::Remote => counters.record_sent(),
                // Coalesce: kind `TlbFlush` đã chờ sẵn (yêu cầu trước chưa drain xong) — slot giữ
                // seq **mới nhất**, không mất yêu cầu; không gửi IPI thứ hai.
                ipi::PostOutcome::Coalesced => {}
                // Offline/không hợp lệ: không ai ack ⇒ để vòng chờ timeout thay vì giả xong.
                ipi::PostOutcome::Invalid | ipi::PostOutcome::Local => {
                    counters.record_error();
                }
            }
        }
        hart += 1;
    }

    // 3) Chờ đủ ack — **drain mỗi bước** (luật chống deadlock D2-68), deadline bằng `rdtime`.
    let deadline = start.saturating_add(timeout_ticks);
    let mut rounds = 0u64;
    let acked;
    let completed;
    loop {
        let _ = ipi::drain_current();
        rounds += 1;
        let mask = ack_mask(seq);
        if ack_complete(mask, active_on_mask) {
            acked = mask;
            completed = true;
            break;
        }
        if timer::now_ticks() >= deadline {
            acked = mask;
            completed = false;
            break;
        }
        core::hint::spin_loop();
    }
    let waited = timer::now_ticks().wrapping_sub(start);
    if completed {
        // Bất biến chốt: `Ok ⇒ acked == active_on` (kiểm lại lần nữa, không tin một lần đọc).
        debug_assert!(acked & active_on_mask == active_on_mask);
    } else {
        counters.record_timeout();
    }
    LAST_SEQ.store(seq, Ordering::SeqCst);
    LAST_ASID.store(asid as u64, Ordering::SeqCst);
    LAST_SCOPE.store(scope.code(), Ordering::SeqCst);
    LAST_TARGETS.store(active_on_mask, Ordering::SeqCst);
    LAST_ACKED.store(acked, Ordering::SeqCst);
    LAST_COMPLETED.store(completed as u64, Ordering::SeqCst);
    LAST_WAIT.store(waited, Ordering::SeqCst);
    LAST_ROUNDS.store(rounds, Ordering::SeqCst);
    IN_FLIGHT.store(false, Ordering::Release);
    if completed {
        ShootdownOutcome::Completed
    } else {
        ShootdownOutcome::TimedOut
    }
}

/// Số hart **còn thiếu** ack của lần shootdown gần nhất (chẩn đoán).
pub fn last_missing() -> u64 {
    let info = last_shootdown();
    ack_missing(info.acked, info.targets)
}

/// Số hart trong `active_on` (tiện log/TAP).
pub fn active_count() -> usize {
    mask_count(active_on()) as usize
}

/// Đọc `satp` của hart hiện tại (wrapper cho TAP; `mmu` cũng có).
pub fn satp_asid_now() -> u16 {
    satp_asid(mmu::satp_now())
}

/// `true` ⇔ lease kernel đã init và không phải `ASID_NONE`.
pub fn initialized() -> bool {
    asid() != ASID_NONE
}

/// Chỉ dùng nội bộ: đọc leaf PTE level-1 của VA thử nghiệm (ktest) — khai báo ở `mmu`.
#[cfg(feature = "ktest")]
pub use crate::mmu::{
    probe2_leaf_phys, probe_leaf_phys, remap_probe, remap_probe2, PROBE2_PA_C, PROBE2_PA_D,
    PROBE2_VA, PROBE_PA_A, PROBE_PA_B, PROBE_VA,
};

/// Đọc 8 B ở PA qua physmap (bằng chứng dữ liệu thật, không qua VA đang thử).
#[cfg(feature = "ktest")]
pub fn read_phys_u64(pa: u64) -> Option<u64> {
    let va = mmu::physmap_alias(pa)?;
    Some(mmu::read_u64(va))
}

/// Ghi 8 B ở PA qua physmap (ghi marker vào PA **mới** trước khi remap để kiểm TLB).
#[cfg(feature = "ktest")]
pub fn write_phys_u64(pa: u64, value: u64) -> bool {
    match mmu::physmap_alias(pa) {
        Some(va) => {
            mmu::write_u64(va, value);
            true
        }
        None => false,
    }
}

/// `true` ⇔ PTE của VA thử nghiệm là leaf 4 KiB/2 MiB không exec (bằng chứng dựng bảng).
#[cfg(feature = "ktest")]
pub fn probe_leaf_ok(pa: u64) -> bool {
    let walk = mmu::walk_sw(PROBE_VA);
    walk.valid
        && walk.leaf
        && walk.level == 1
        && walk.phys == pa
        && walk.raw() & PTE_EXEC == 0
        && walk.raw() & (PTE_READ | PTE_WRITE) == (PTE_READ | PTE_WRITE)
}

const _: () = {
    assert!(MAX_HARTS <= 64);
    assert!(ASID_NONE as usize >= 256);
    // Mã ba dạng flush phải là 0/1/2 (slot dùng chung mã với `plat-common`).
    assert!(FlushScope::PAGE_CODE == 0);
    assert!(FlushScope::ASID_CODE == 1);
    assert!(FlushScope::ALL_CODE == 2);
    assert!(FlushScope::RANGE_CODE == 3);
};
