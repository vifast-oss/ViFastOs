// SPDX-License-Identifier: MIT OR Apache-2.0
//! IPI thật + **hộp thư theo hart** (KT-50) — phần *ngữ nghĩa* chạy được trên host (D2-72 Q12).
//!
//! Tệp này **không có `unsafe`** (`vifast-plat-common` là `#![forbid(unsafe_code)]`): asm `ecall`
//! SBI IPI và CSR `sie.SSIE`/`sip.SSIP`/`sstatus.SIE` nằm ở `kernel/vifast-plat-riscv64` (D2-17).
//! Ở đây là những gì phải **đúng trước khi** chạm phần cứng:
//!
//! - **SBI IPI** (EID [`SBI_EXT_IPI`] `0x735049` = "sPI", FID 0 `sbi_send_ipi(hart_mask,
//!   hart_mask_base)` — SBI v2.0 §6.1). `hart_mask_base = 0` ([`IPI_HART_MASK_BASE`]) nên bit
//!   thứ `h` của `hart_mask` **là** hart `h` (không cộng offset), khớp [`crate::smp::hart_bit`].
//! - **ACLINT MSWI là dữ kiện nền tảng, KHÔNG phải đường kernel dùng**: hằng hình học
//!   ([`MSWI_HART_STRIDE`], [`mswi_offset`], [`mswi_address`], [`msip_pending`]) ở đây chỉ để
//!   **host test** + làm rõ vì sao đường thật khác. S-mode **không** đọc được MSWI (PMP firmware
//!   chặn — đo được `load access fault` `scause=5` `tval=0x0200_0004`, `v2/docs/k3-riscv64.md`
//!   §13.4) nên theo **D2-76** kernel **cấm map** CLINT/MSWI: arch đọc/xoá trạng thái ngắt phần
//!   mềm bằng **`sip.SSIP`** (cùng trạng thái theo privileged spec) — xem
//!   [`IpiCounters::record_trap`] cho `spurious`.
//! - **Hộp thư theo hart, coalesce theo kind**: mỗi hart có một hàng chờ dạng **bitmask kind**
//!   ([`IpiMailbox`], `AtomicU64`). Gửi trùng một kind khi kind đó **đang chờ** được **gộp**
//!   (không mất kind, không nhân đôi message, không gửi thêm IPI); kind khác vẫn giữ nguyên bit
//!   riêng — nên `drain` một lần giao **đủ mọi kind** đang chờ. Hàng chờ **cố định 3 bit**
//!   ([`IPI_KIND_COUNT`]) nên đầy **không xảy ra**; đây là điểm khác `vifast-sync::Mailbox` (hàng
//!   MPSC 7 message có payload, đầy ⇒ `Busy`): IPI chỉ cần "đánh thức + kind", coalesce là ngữ
//!   nghĩa đúng của thiết kế §4.17 cho shootdown/wake lặp.
//! - **`Kind`**: [`IpiKind::TlbFlush`] (khung của KT-51 — KT-50 **chưa** làm ASID/`sfence.vma`/
//!   shootdown ack), [`IpiKind::VmKick`], [`IpiKind::Wake`].
//! - **Không self-target** (D2-68): [`delivery_for`] trả [`IpiDelivery::Local`] khi `sender ==
//!   target` ⇒ arch **không** gọi SBI IPI, chỉ xử lý **local** (flush cục bộ + ack ngầm ở KT-51)
//!   và cộng bộ đếm `local`; `sent`/`traps` **không** tăng. Bản self-target là bản **chặt hơn**:
//!   self-IPI làm mutation "bỏ flush cục bộ" sống, tức che lỗi thật (D2-68 đính chính 2026-11-14).
//! - **Bộ đếm** [`IpiCounters`]/[`IPI_STATS`]: `sent` (SBI IPI thật, theo hart gửi), `coalesced`
//!   (post bị gộp), `local` (self-target), `errors` (+ `last_error`), `traps` (ngắt
//!   `scause = (1<<63)|1` đã vào `stvec`), `delivered` (kind đã giao khi drain) + `kinds[3]`,
//!   `spurious` (trap IPI mà MSIP **không** bật). TAP KT-50 đọc đúng các số này.
//! - **Drain cả khi đang chờ khoá** — [`wait_with_drain`]: vòng chờ **gọi drain trước mỗi bước**
//!   rồi mới kiểm predicate, và **luôn** kết thúc khi `now >= deadline`. Đây là bài học D2-68
//!   (KT-40): chờ ack mà không drain hộp thư ⇒ deadlock thật; chờ ack **có** drain + timeout ⇒
//!   hart chậm/chết chỉ làm test đỏ, không treo máy. Hàm thuần (nhận closure `now`/`drain`/`ready`)
//!   nên host test kiểm được cả "có drain" và "timeout phải hết".
//!
//! **Không** thuộc KT-50: ASID/`sfence.vma`/shootdown ack đầy đủ (KT-51), object layer (KT-52),
//! U-mode (KT-53), scheduler/preemption đa hart, hotplug. Xem `v2/docs/k3-riscv64.md` §13.

use core::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use vifast_sync::{Msg, PostOutcome, Tier2Mailbox};

use crate::smp::MAX_HARTS;

/// EID **SBI IPI** `0x735049` ("sPI" ASCII) — SBI v2.0 §6.1, KT-50.
pub const SBI_EXT_IPI: u64 = 0x0073_5049;
/// FID 0 — `sbi_send_ipi(hart_mask, hart_mask_base)`.
pub const SBI_IPI_SEND_IPI: u64 = 0;
/// `hart_mask_base` kernel dùng: bit thứ `h` của `hart_mask` = hart `h` (mask tuyệt đối).
pub const IPI_HART_MASK_BASE: u64 = 0;
/// Bit `SSIE` (supervisor **software** interrupt enable) của CSR `sie` — ngắt IPI ở S-mode.
pub const SIE_SSIE: u64 = 1 << 1;
/// Bước giữa hai word MSIP của ACLINT MSWI (dữ kiện nền tảng + host test — **không** dùng để
/// map/đọc trong kernel, D2-76): mỗi hart một word 32-bit.
pub const MSWI_HART_STRIDE: u64 = 4;
/// Cửa sổ ACLINT MSWI (dữ kiện nền tảng): một trang 4 KiB đủ `MAX_HARTS = 8` word = 32 B.
/// Kernel **không map** vùng này (D2-76).
pub const MSWI_WINDOW_BYTES: u64 = 0x1000;
/// Số kind của hộp thư IPI (`TlbFlush`, `VmKick`, `Wake`).
pub const IPI_KIND_COUNT: usize = 3;
/// Mask hợp lệ của [`KindSet`] — bit ngoài dải kind luôn bị bỏ.
pub const IPI_KIND_MASK: u64 = (1u64 << IPI_KIND_COUNT) - 1;
/// Mã lỗi nội bộ: hart đích ngoài `MAX_HARTS` (D2-77 — `post` trả `Invalid`).
pub const IPI_ERR_HART_OUT_OF_RANGE: i64 = -3;
/// Mã lỗi nội bộ: hart đích **hợp lệ nhưng chưa online** (D2-77 — `post` trả `Invalid`, **không**
/// gọi SBI, **không** giữ kind trong hộp thư: không ai drain ⇒ bit mồ côi).
pub const IPI_ERR_HART_OFFLINE: i64 = -4;

/// Loại IPI — hộp thư per-hart **coalesce theo kind** (KT-50).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IpiKind {
    /// Xả TLB cho không gian địa chỉ — KT-50 chỉ đặt **khung** (drain + kind); ASID/`sfence.vma`/
    /// ack shootdown đầy đủ là KT-51.
    TlbFlush,
    /// Đá hart ra khỏi trạng thái guest/VM (H1) — KT-50 mới là kind, chưa có VM.
    VmKick,
    /// Đánh thức hart đang park/chờ.
    Wake,
}

impl IpiKind {
    /// Cả ba kind, theo đúng thứ tự bit ([`IpiKind::index`]).
    pub const ALL: [IpiKind; IPI_KIND_COUNT] = [Self::TlbFlush, Self::VmKick, Self::Wake];

    /// Chỉ số bit trong [`KindSet`] (`0..IPI_KIND_COUNT`).
    pub const fn index(self) -> usize {
        match self {
            Self::TlbFlush => 0,
            Self::VmKick => 1,
            Self::Wake => 2,
        }
    }

    /// Bit của kind trong [`KindSet`]/hộp thư.
    pub const fn bit(self) -> u64 {
        1u64 << self.index()
    }

    /// Kind từ chỉ số bit (`None` nếu ngoài dải).
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::TlbFlush),
            1 => Some(Self::VmKick),
            2 => Some(Self::Wake),
            _ => None,
        }
    }

    /// Tên ổn định cho log/TAP (`TlbFlush`, `VmKick`, `Wake`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::TlbFlush => "TlbFlush",
            Self::VmKick => "VmKick",
            Self::Wake => "Wake",
        }
    }

    /// Kind **chuông** cho một message tầng 2 — ánh xạ **một chiều**, không mất thông tin payload
    /// (payload nằm ở ring tầng 2, không ở bit).
    ///
    /// `TlbFlush`/`VmKick` có bit riêng; năm loại còn lại (`Wake`, `Reschedule`, `Migrate`,
    /// `Stop`, `PowerReq`) đều cần hart đích **chạy** nên dùng bit `Wake` — gộp chuông là đúng
    /// ngữ nghĩa coalesce của tầng 1 (không mất kind nào **của tầng 2**, vì payload ở ring).
    pub const fn for_msg(msg: Msg) -> Self {
        match msg {
            Msg::TlbFlush { .. } => Self::TlbFlush,
            Msg::VmKick { .. } => Self::VmKick,
            Msg::Wake { .. }
            | Msg::Reschedule { .. }
            | Msg::Migrate { .. }
            | Msg::Stop { .. }
            | Msg::PowerReq { .. } => Self::Wake,
        }
    }
}

/// Tập kind (bitmask 3 bit) — kết quả `drain` và nội dung chờ của hộp thư.
///
/// Bit ngoài [`IPI_KIND_MASK`] bị **bỏ** (không thể có kind lạ trong hộp thư); `from_bits` là
/// hàm duy nhất dựng từ số nên hộp thư không bao giờ chứa bit lạ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindSet {
    bits: u64,
}

impl KindSet {
    /// Tập rỗng.
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    /// Tập đúng một kind.
    pub const fn from_kind(kind: IpiKind) -> Self {
        Self { bits: kind.bit() }
    }

    /// Tập từ bitmask — bit lạ (ngoài dải kind) bị bỏ.
    pub const fn from_bits(bits: u64) -> Self {
        Self {
            bits: bits & IPI_KIND_MASK,
        }
    }

    /// Bitmask thô (đã lọc dải kind).
    pub const fn bits(&self) -> u64 {
        self.bits
    }

    /// `true` ⇔ rỗng.
    pub const fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Số kind trong tập (`0..=3`).
    pub const fn count(&self) -> usize {
        self.bits.count_ones() as usize
    }

    /// `true` ⇔ tập chứa `kind`.
    pub const fn contains(&self, kind: IpiKind) -> bool {
        self.bits & kind.bit() != 0
    }

    /// Thêm `kind`; trả `true` ⇔ bit **mới được thêm** (trước đó chưa có).
    pub fn insert(&mut self, kind: IpiKind) -> bool {
        let bit = kind.bit();
        let fresh = self.bits & bit == 0;
        self.bits |= bit;
        fresh
    }

    /// Bỏ `kind`; trả `true` ⇔ bit **vừa bị bỏ**.
    pub fn remove(&mut self, kind: IpiKind) -> bool {
        let bit = kind.bit();
        let had = self.bits & bit != 0;
        self.bits &= !bit;
        had
    }

    /// Hợp hai tập.
    pub fn union(self, other: Self) -> Self {
        Self::from_bits(self.bits | other.bits)
    }

    /// Kind đầu tiên trong tập (theo thứ tự bit) — tiện cho log.
    pub const fn first(&self) -> Option<IpiKind> {
        let bit = self.bits & self.bits.wrapping_neg();
        if bit == 0 {
            None
        } else {
            IpiKind::from_index(bit.trailing_zeros() as usize)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hộp thư per-hart (coalesce theo kind)
// ---------------------------------------------------------------------------------------------

/// Ảnh chụp hộp thư IPI của một hart — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IpiBoxSnapshot {
    /// Bitmask kind đang chờ.
    pub pending_bits: u64,
    /// Số lần `post` **đặt bit mới**.
    pub posted: u64,
    /// Số lần `post` bị **gộp** (kind đã chờ sẵn).
    pub coalesced: u64,
    /// Số lần `drain`.
    pub drains: u64,
    /// Tổng số kind đã giao qua `drain`.
    pub delivered: u64,
    /// Số lần `drain` mà hộp thư **rỗng** (không có kind nào).
    pub empty_drains: u64,
}

/// Hộp thư IPI của **một hart**: hàng chờ cố định dạng bitmask kind + bộ đếm.
///
/// Giao thức (một người nhận là chính hart sở hữu hộp thư, nhiều người gửi):
///
/// - `post(kind)` **trước** khi gọi SBI IPI: trả `true` ⇒ kind mới ⇒ caller gửi IPI; `false` ⇒
///   kind đang chờ, IPI đã (hoặc sắp) được gửi ⇒ **không** gửi thêm (coalesce).
/// - `drain()` dùng **một** `swap(0)` ⇒ không thể mất kind nào đã được đặt bit trước đó, mỗi kind
///   được giao **đúng một lần** cho mỗi lần đặt bit.
/// - Ghi `posted`/`coalesced`/`delivered` bằng `Relaxed` (chỉ để đo), còn `pending` dùng
///   `AcqRel`/`Relaxed` đúng cặp `fetch_or` ↔ `swap` để không mất kind.
#[derive(Debug)]
pub struct IpiMailbox {
    pending: AtomicU64,
    posted: AtomicU64,
    coalesced: AtomicU64,
    drains: AtomicU64,
    delivered: AtomicU64,
    empty_drains: AtomicU64,
}

impl IpiMailbox {
    /// Hộp thư rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            pending: AtomicU64::new(0),
            posted: AtomicU64::new(0),
            coalesced: AtomicU64::new(0),
            drains: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            empty_drains: AtomicU64::new(0),
        }
    }

    /// Đặt `kind` vào hàng chờ; trả `true` ⇔ **bit mới** (cần gửi IPI), `false` ⇔ đã có (gộp).
    ///
    /// Không bao giờ làm mất kind: `fetch_or` giữ mọi bit đã đặt; gửi trùng chỉ tăng `coalesced`.
    pub fn post(&self, kind: IpiKind) -> bool {
        let previous = self.pending.fetch_or(kind.bit(), Ordering::AcqRel);
        if previous & kind.bit() != 0 {
            self.coalesced.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            self.posted.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    /// **Gỡ** `kind` vừa post nhưng IPI gửi thất bại (D2-77): không để bit mồ côi trong hộp thư
    /// (hart đích không nhận được IPI thì bit sẽ nằm lại tới lần drain tình cờ).
    ///
    /// Giảm `posted` tương ứng để bất biến `posted = delivered + pending` vẫn đúng; trả `true` ⇔
    /// bit **đã được gỡ** (nếu ai đó đã drain mất thì `false`).
    pub fn unpost(&self, kind: IpiKind) -> bool {
        let previous = self.pending.fetch_and(!kind.bit(), Ordering::AcqRel);
        let removed = previous & kind.bit() != 0;
        if removed {
            self.posted.fetch_sub(1, Ordering::Relaxed);
        }
        removed
    }

    /// Rút **toàn bộ** kind đang chờ bằng một `swap(0)`; cộng `delivered` theo số kind.
    pub fn drain(&self) -> KindSet {
        let bits = self.pending.swap(0, Ordering::AcqRel);
        let kinds = KindSet::from_bits(bits);
        self.drains.fetch_add(1, Ordering::Relaxed);
        self.delivered
            .fetch_add(kinds.count() as u64, Ordering::Relaxed);
        if kinds.is_empty() {
            self.empty_drains.fetch_add(1, Ordering::Relaxed);
        }
        kinds
    }

    /// Tập kind đang chờ (không rút).
    pub fn pending(&self) -> KindSet {
        KindSet::from_bits(self.pending.load(Ordering::Acquire))
    }

    /// `true` ⇔ không có kind nào đang chờ.
    pub fn is_empty(&self) -> bool {
        self.pending.load(Ordering::Acquire) == 0
    }

    /// Ảnh chụp bộ đếm hộp thư.
    pub fn snapshot(&self) -> IpiBoxSnapshot {
        IpiBoxSnapshot {
            pending_bits: self.pending.load(Ordering::Acquire),
            posted: self.posted.load(Ordering::Relaxed),
            coalesced: self.coalesced.load(Ordering::Relaxed),
            drains: self.drains.load(Ordering::Relaxed),
            delivered: self.delivered.load(Ordering::Relaxed),
            empty_drains: self.empty_drains.load(Ordering::Relaxed),
        }
    }
}

impl Default for IpiMailbox {
    fn default() -> Self {
        Self::new()
    }
}

/// Hộp thư IPI của **mọi hart** (`MAX_HARTS` slot, `const`-init — không cấp phát).
#[derive(Debug)]
pub struct PerHartIpiMailboxes {
    slots: [IpiMailbox; MAX_HARTS],
}

impl PerHartIpiMailboxes {
    /// Hộp thư rỗng cho mọi hart.
    pub const fn new() -> Self {
        Self {
            slots: [const { IpiMailbox::new() }; MAX_HARTS],
        }
    }

    /// Hộp thư của `hart`; hart ngoài dải ⇒ slot 0 (không bao giờ xảy ra với hart thật).
    pub fn get(&self, hart: usize) -> &IpiMailbox {
        &self.slots[if hart < MAX_HARTS { hart } else { 0 }]
    }

    /// Hộp thư của `hart` nếu hợp lệ.
    pub fn try_get(&self, hart: usize) -> Option<&IpiMailbox> {
        self.slots.get(hart)
    }

    /// Tổng số kind đang chờ trên mọi hart — bằng chứng "không còn kind treo" sau pha IPI.
    pub fn total_pending(&self) -> usize {
        self.slots.iter().map(|slot| slot.pending().count()).sum()
    }
}

impl Default for PerHartIpiMailboxes {
    fn default() -> Self {
        Self::new()
    }
}

/// Hộp thư IPI của ảnh kernel — một slot mỗi hart.
pub static IPI_MAILBOXES: PerHartIpiMailboxes = PerHartIpiMailboxes::new();

// ---------------------------------------------------------------------------------------------
// Tầng 2: ring payload `Mailbox<N>` **trên** chuông `IpiMailbox` (KT-415, D2-259 mục 6)
// ---------------------------------------------------------------------------------------------

/// Kết quả một lần [`HartTier2::send`] — **hai** tầng, **hai** kết cục đo được.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier2Send {
    /// Kết cục tầng 2: message vào ring (`Delivered`) hay ring đầy ⇒ `OVERFLOW` (`Overflow`).
    pub outcome: PostOutcome,
    /// `true` ⇔ lần này đặt **bit mới** ở chuông tầng 1 (cần gửi IPI); `false` ⇔ bit đã chờ
    /// (gộp — IPI đã/sắp gửi, **không** mất wake).
    pub doorbell: bool,
}

/// Kết quả một lần [`HartTier2::drain`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HartDrain {
    /// Tập kind đã rút khỏi chuông tầng 1 (rút **trước** ring).
    pub kinds: KindSet,
    /// Số message đã rút khỏi ring tầng 2.
    pub messages: usize,
    /// `true` ⇔ cờ `OVERFLOW` đang bật ⇒ người nhận **phải** full-rescan/full-flush.
    pub overflowed: bool,
}

/// **Hộp thư tầng 2 của một hart**: ring payload [`Tier2Mailbox<N>`] + chuông [`IpiMailbox`].
///
/// # Giao thức (bắt buộc, đây là chỗ "0 mất wake" được bảo đảm)
///
/// - [`send`](Self::send): ghi ring **trước** (hoặc bật `OVERFLOW`), rồi **luôn** `post` chuông
///   tầng 1 — kể cả khi ring đầy. Vì vậy hart đích **luôn** bị đánh thức; nếu message rơi khỏi
///   ring thì cờ `OVERFLOW` nói cho nó biết phải quét lại toàn phần.
/// - [`drain`](Self::drain): rút chuông **trước** (xoá bit), rồi rút ring + đọc-và-xoá cờ. Một
///   `send` chen giữa hai bước để lại bit chuông cho lượt sau ⇒ không có khe mất wake.
///
/// `HartTier2` **sở hữu** chuông của nó (không dùng chung `IPI_MAILBOXES`) để chỉ **một** người
/// rút — hai người rút cùng một chuông sẽ chia mất kind.
#[derive(Debug)]
pub struct HartTier2<const N: usize> {
    ring: Tier2Mailbox<N>,
    doorbell: IpiMailbox,
}

impl<const N: usize> HartTier2<N> {
    /// Hộp thư rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            ring: Tier2Mailbox::new(),
            doorbell: IpiMailbox::new(),
        }
    }

    /// Gửi `msg` với kind chuông suy ra từ chính message ([`IpiKind::for_msg`]).
    pub fn send(&self, msg: Msg) -> Tier2Send {
        self.send_as(IpiKind::for_msg(msg), msg)
    }

    /// Gửi `msg` với kind chuông **chỉ định** (dùng khi caller cần bit khác ánh xạ mặc định).
    ///
    /// Thứ tự **bắt buộc**: ring trước, chuông sau — chuông là `Release`/`AcqRel` nên người nhận
    /// thấy message (hoặc cờ) trước khi thấy bit.
    pub fn send_as(&self, kind: IpiKind, msg: Msg) -> Tier2Send {
        let outcome = self.ring.post(msg);
        // **Đột biến** `mut-tier2-no-doorbell-on-overflow`: ring đầy ⇒ **không** rung chuông ⇒
        // hart đích không bao giờ bị đánh thức cho message đã rơi ⇒ **mất wake**. Chỉ dùng cho
        // kiểm (D2-05); nhánh sản phẩm **luôn** rung chuông.
        #[cfg(feature = "mut-tier2-no-doorbell-on-overflow")]
        {
            if outcome.overflowed() {
                return Tier2Send {
                    outcome,
                    doorbell: false,
                };
            }
        }
        let doorbell = self.doorbell.post(kind);
        Tier2Send { outcome, doorbell }
    }

    /// Rút chuông **trước**, rồi rút ring + cờ; gọi `f` cho từng message.
    pub fn drain<F: FnMut(Msg)>(&self, f: F) -> HartDrain {
        let kinds = self.doorbell.drain();
        let d = self.ring.take(f);
        // **Đột biến** `mut-tier2-ignore-overflow`: người nhận **bỏ qua** cờ `OVERFLOW` ⇒ message
        // đã rơi khỏi ring không được cứu bằng full-rescan ⇒ **mất wake**. Chỉ dùng cho kiểm.
        #[cfg(feature = "mut-tier2-ignore-overflow")]
        let overflowed = {
            let _ = d.overflowed;
            false
        };
        #[cfg(not(feature = "mut-tier2-ignore-overflow"))]
        let overflowed = d.overflowed;
        HartDrain {
            kinds,
            messages: d.messages,
            overflowed,
        }
    }

    /// Ring payload (đọc bộ đếm/chẩn đoán).
    pub fn ring(&self) -> &Tier2Mailbox<N> {
        &self.ring
    }

    /// Chuông tầng 1 của hart này.
    pub fn doorbell(&self) -> &IpiMailbox {
        &self.doorbell
    }
}

impl<const N: usize> Default for HartTier2<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Hộp thư **tầng 2** của mọi hart (`MAX_HARTS` slot, `const`-init — không cấp phát).
#[derive(Debug)]
pub struct PerHartTier2<const N: usize> {
    slots: [HartTier2<N>; MAX_HARTS],
}

impl<const N: usize> PerHartTier2<N> {
    /// Hộp thư rỗng cho mọi hart.
    pub const fn new() -> Self {
        Self {
            slots: [const { HartTier2::new() }; MAX_HARTS],
        }
    }

    /// Hộp thư của `hart`; hart ngoài dải ⇒ slot 0 (cùng quy ước [`PerHartIpiMailboxes::get`]).
    pub fn get(&self, hart: usize) -> &HartTier2<N> {
        &self.slots[if hart < MAX_HARTS { hart } else { 0 }]
    }

    /// Hộp thư của `hart` nếu hợp lệ.
    pub fn try_get(&self, hart: usize) -> Option<&HartTier2<N>> {
        self.slots.get(hart)
    }

    /// Tổng số message tầng 2 đang chờ trên mọi hart.
    pub fn total_pending(&self) -> usize {
        self.slots.iter().map(|slot| slot.ring.pending()).sum()
    }
}

impl<const N: usize> Default for PerHartTier2<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Hộp thư **tầng 2** của ảnh kernel — một slot mỗi hart, ring 8 message (bằng sức chứa
/// `HartMailbox` của core, `crate::mailbox`).
pub static TIER2_MAILBOXES: PerHartTier2<8> = PerHartTier2::new();

// ---------------------------------------------------------------------------------------------
// Bộ đếm (sent/delivered/coalesced/spurious)
// ---------------------------------------------------------------------------------------------

/// Ảnh chụp bộ đếm IPI của **một hart** — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IpiSnapshot {
    /// Số SBI `send_ipi` **thật** đã gửi (không tính self-target).
    pub sent: u64,
    /// Số `post` bị gộp vì kind đang chờ.
    pub coalesced: u64,
    /// Số `post` self-target xử lý **local** (không IPI — D2-68).
    pub local: u64,
    /// Số lỗi SBI (hoặc tham số) khi gửi.
    pub errors: u64,
    /// Mã lỗi gần nhất (`0` = chưa có).
    pub last_error: i64,
    /// Số trap IPI (`scause = (1<<63)|1`) đã vào `stvec` trên hart này.
    pub traps: u64,
    /// Số kind đã giao cho hart này (qua trap handler hoặc drain khi chờ).
    pub delivered: u64,
    /// Số trap "spurious": trap IPI mà MSIP **không** bật (không có ngắt phần mềm thật).
    pub spurious: u64,
    /// Số kind đã giao theo từng kind (`TlbFlush`, `VmKick`, `Wake`) — coalesce không làm mất kind.
    pub kinds: [u64; IPI_KIND_COUNT],
}

/// Bộ đếm IPI của **một hart** — `sent/coalesced/local/errors` theo **hart gửi**,
/// `traps/delivered/spurious` theo **hart nhận** (chính là hart sở hữu hộp thư).
#[derive(Debug)]
pub struct IpiCounters {
    sent: AtomicU64,
    coalesced: AtomicU64,
    local: AtomicU64,
    errors: AtomicU64,
    last_error: AtomicI64,
    traps: AtomicU64,
    delivered: AtomicU64,
    spurious: AtomicU64,
    kinds: [AtomicU64; IPI_KIND_COUNT],
}

impl IpiCounters {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            coalesced: AtomicU64::new(0),
            local: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            last_error: AtomicI64::new(0),
            traps: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            spurious: AtomicU64::new(0),
            kinds: [const { AtomicU64::new(0) }; IPI_KIND_COUNT],
        }
    }

    /// Xoá bộ đếm — chỉ dùng trong test host (ảnh thật không reset).
    pub fn reset(&self) {
        for field in [
            &self.sent,
            &self.coalesced,
            &self.local,
            &self.errors,
            &self.traps,
            &self.delivered,
            &self.spurious,
        ] {
            field.store(0, Ordering::Relaxed);
        }
        for slot in &self.kinds {
            slot.store(0, Ordering::Relaxed);
        }
        self.last_error.store(0, Ordering::Relaxed);
    }

    /// Ghi một SBI `send_ipi` **thành công**.
    pub fn record_sent(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một `post` bị **gộp** (kind đang chờ).
    pub fn record_coalesced(&self) {
        self.coalesced.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một `post` **self-target** xử lý local (local flush + ack ngầm — D2-68).
    pub fn record_local(&self, kind: IpiKind) {
        self.local.fetch_add(1, Ordering::Relaxed);
        self.kinds[kind.index()].fetch_add(1, Ordering::Relaxed);
        self.delivered.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi một lỗi gửi (mã SBI hoặc `-3` tham số).
    pub fn record_error(&self, code: i64) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        self.last_error.store(code, Ordering::Relaxed);
    }

    /// Ghi một trap IPI đã vào `stvec`; `mswi_pending` = MSIP đọc **trước khi xoá**.
    ///
    /// `false` ⇒ ngắt phần mềm không có nguồn thật ⇒ cộng `spurious` (TAP KT-50 đòi `0`).
    pub fn record_trap(&self, mswi_pending: bool) {
        self.traps.fetch_add(1, Ordering::Relaxed);
        if !mswi_pending {
            self.spurious.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Ghi một lần `drain` giao `kinds` (trap handler **hoặc** vòng chờ có drain).
    pub fn record_drain(&self, kinds: KindSet) {
        self.delivered
            .fetch_add(kinds.count() as u64, Ordering::Relaxed);
        for kind in IpiKind::ALL {
            if kinds.contains(kind) {
                self.kinds[kind.index()].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Ảnh chụp bộ đếm.
    pub fn snapshot(&self) -> IpiSnapshot {
        let mut kinds = [0u64; IPI_KIND_COUNT];
        for (index, slot) in self.kinds.iter().enumerate() {
            kinds[index] = slot.load(Ordering::Relaxed);
        }
        IpiSnapshot {
            sent: self.sent.load(Ordering::Relaxed),
            coalesced: self.coalesced.load(Ordering::Relaxed),
            local: self.local.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            last_error: self.last_error.load(Ordering::Relaxed),
            traps: self.traps.load(Ordering::Relaxed),
            delivered: self.delivered.load(Ordering::Relaxed),
            spurious: self.spurious.load(Ordering::Relaxed),
            kinds,
        }
    }
}

impl Default for IpiCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm IPI **theo hart** — mỗi hart một [`IpiCounters`].
#[derive(Debug)]
pub struct PerHartIpiStats {
    slots: [IpiCounters; MAX_HARTS],
}

impl PerHartIpiStats {
    /// Bộ đếm rỗng cho mọi hart.
    pub const fn new() -> Self {
        Self {
            slots: [const { IpiCounters::new() }; MAX_HARTS],
        }
    }

    /// Bộ đếm của `hart`; hart ngoài dải ⇒ slot 0.
    pub fn get(&self, hart: usize) -> &IpiCounters {
        &self.slots[if hart < MAX_HARTS { hart } else { 0 }]
    }

    /// Bộ đếm của `hart` nếu hợp lệ.
    pub fn try_get(&self, hart: usize) -> Option<&IpiCounters> {
        self.slots.get(hart)
    }

    /// Tổng `sent` trên mọi hart.
    pub fn total_sent(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| slot.snapshot().sent)
            .sum::<u64>()
    }

    /// Tổng `delivered` trên mọi hart.
    pub fn total_delivered(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| slot.snapshot().delivered)
            .sum::<u64>()
    }

    /// Tổng `traps` trên mọi hart.
    pub fn total_traps(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| slot.snapshot().traps)
            .sum::<u64>()
    }

    /// Tổng `spurious` trên mọi hart.
    pub fn total_spurious(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| slot.snapshot().spurious)
            .sum::<u64>()
    }
}

impl Default for PerHartIpiStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm IPI toàn cục của ảnh kernel (PerHart).
pub static IPI_STATS: PerHartIpiStats = PerHartIpiStats::new();

// ---------------------------------------------------------------------------------------------
// ACLINT MSWI + quyết định gửi (không self-target — D2-68)
// ---------------------------------------------------------------------------------------------

/// `true` ⇔ word MSIP nghĩa "có ngắt phần mềm đang chờ" (bit 0 — ACLINT MSWI).
pub const fn msip_pending(word: u32) -> bool {
    word & 1 != 0
}

/// Offset word MSIP của `hart` trong khối MSWI: `hart * 4`.
pub const fn mswi_offset(hart: usize) -> u64 {
    hart as u64 * MSWI_HART_STRIDE
}

/// Địa chỉ tuyệt đối word MSIP của `hart` khi ACLINT đặt tại `clint_base` (D2-72 Q7).
pub const fn mswi_address(clint_base: u64, hart: usize) -> u64 {
    clint_base + mswi_offset(hart)
}

/// Đường đi của một `post`: **local** (không IPI) hay **remote** (gửi SBI IPI).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpiDelivery {
    /// Gửi chính hart mình ⇒ local flush + ack ngầm, **không** IPI (D2-68) ⇒ `sent` không tăng.
    Local,
    /// Hart khác ⇒ đặt kind vào hộp thư đích rồi gọi SBI IPI.
    Remote,
}

/// Quyết định đường gửi: `sender == target` ⇒ [`IpiDelivery::Local`] (D2-68), còn lại `Remote`.
///
/// Hàm thuần để host test chốt "không self-target" mà không cần QEMU: kiểm `sender == target`
/// **trước** mọi thứ khác nên không có nhánh nào gọi SBI IPI cho chính mình.
pub const fn delivery_for(sender: usize, target: usize) -> IpiDelivery {
    if sender == target {
        IpiDelivery::Local
    } else {
        IpiDelivery::Remote
    }
}

/// Quyết định **đầy đủ** của một `post` (KT-50 + **D2-77**) — hàm thuần, host test được.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostDecision {
    /// Hart đích ngoài `0..MAX_HARTS` ⇒ `Invalid` + [`IPI_ERR_HART_OUT_OF_RANGE`].
    InvalidHart,
    /// Hart hợp lệ nhưng **chưa online** ⇒ `Invalid` + [`IPI_ERR_HART_OFFLINE`] + log; **không**
    /// gọi SBI, **không** giữ kind trong hộp thư (không ai drain — D2-77).
    Offline,
    /// Gửi chính hart mình ⇒ local flush + ack ngầm, **không** IPI (D2-68).
    Local,
    /// Hart khác **và** online ⇒ đặt kind vào hộp thư rồi gọi SBI IPI.
    Remote,
}

/// Phân loại `post` theo `online_mask` (bit `h` = hart `h` online) — **kiểm online trước** mọi
/// thứ khác (D2-77): hart chưa chạy thì kind nằm lại vô chủ và có thể bị giao nhầm sau khi hart
/// start. `sender` luôn online; self-target vẫn đi đường [`PostDecision::Local`].
pub const fn classify_post(sender: usize, target: usize, online_mask: u64) -> PostDecision {
    if target >= MAX_HARTS {
        PostDecision::InvalidHart
    } else if online_mask & (1u64 << target) == 0 {
        PostDecision::Offline
    } else if sender == target {
        PostDecision::Local
    } else {
        PostDecision::Remote
    }
}

// ---------------------------------------------------------------------------------------------
// Chờ **có drain** (bài học D2-68) + timeout luôn kết thúc
// ---------------------------------------------------------------------------------------------

/// Kết quả một vòng chờ có drain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitOutcome {
    /// `ready()` đúng — đã xong (drain vẫn được gọi ở bước cuối trước khi kiểm).
    Ready,
    /// Quá `deadline` mà `ready()` chưa đúng — **không** treo.
    TimedOut,
}

/// Deadline tuyệt đối từ `now` + `timeout_ticks` (bão hoà, không tràn).
pub const fn deadline_after(now: u64, timeout_ticks: u64) -> u64 {
    now.saturating_add(timeout_ticks)
}

/// Vòng chờ **có drain** (KT-50/D2-68): mỗi bước gọi `drain()` **trước** rồi kiểm `ready()`;
/// dừng khi `ready()` đúng hoặc `now() >= deadline`.
///
/// Vì sao **drain trước** mỗi bước: khi hart đang chờ trong vùng khoá/`SIE = 0`, trap IPI
/// **không** vào được ⇒ message nằm trong hộp thư sẽ không bao giờ được xử lý nếu vòng chờ chỉ
/// nhìn cờ/ack. Đây đúng là lỗi KT-40 (D2-68): chờ ack mà không drain ⇒ **deadlock**. Drain trong
/// vòng chờ làm tiến trình phụ thuộc **dữ liệu** (kind được giao) thay vì phụ thuộc **ngắt**.
///
/// Hàm thuần (nhận closure `now`/`drain`/`ready`) nên host test kiểm được cả "drain có được gọi"
/// và "timeout phải hết" mà không cần phần cứng.
pub fn wait_with_drain<N, D, R>(
    deadline: u64,
    mut now: N,
    mut drain: D,
    mut ready: R,
) -> WaitOutcome
where
    N: FnMut() -> u64,
    D: FnMut(),
    R: FnMut() -> bool,
{
    loop {
        drain();
        if ready() {
            return WaitOutcome::Ready;
        }
        if now() >= deadline {
            return WaitOutcome::TimedOut;
        }
        core::hint::spin_loop();
    }
}

// Bất biến hằng số: sai một trong số này thì mask/hộp thư không còn khớp `scause`/SBI.
const _: () = {
    assert!(SBI_EXT_IPI == 0x735049);
    assert!(IPI_HART_MASK_BASE == 0);
    assert!(MAX_HARTS <= 64); // bitmask hart một word u64 (SBI hart_mask)
    assert!(IPI_KIND_COUNT == 3);
    assert!(IPI_KIND_MASK == 0b111);
    assert!(MSWI_HART_STRIDE == 4);
    assert!(MSWI_WINDOW_BYTES >= MSWI_HART_STRIDE * MAX_HARTS as u64);
    assert!(mswi_address(0x0200_0000, 0) == 0x0200_0000);
    assert!(mswi_address(0x0200_0000, 7) == 0x0200_001c);
    assert!(msip_pending(1));
    assert!(!msip_pending(0));
    assert!(IpiKind::TlbFlush.bit() == 1);
    assert!(IpiKind::VmKick.bit() == 2);
    assert!(IpiKind::Wake.bit() == 4);
    assert!(KindSet::empty().is_empty());
    assert!(KindSet::from_kind(IpiKind::Wake).bits() == 4);
    assert!(KindSet::from_bits(u64::MAX).bits() == IPI_KIND_MASK);
    assert!(matches!(IpiKind::from_index(0), Some(IpiKind::TlbFlush)));
    assert!(IpiKind::from_index(3).is_none());
    assert!(deadline_after(u64::MAX - 1, 10) == u64::MAX);
    assert!(matches!(
        classify_post(0, MAX_HARTS, 1),
        PostDecision::InvalidHart
    ));
    assert!(matches!(classify_post(0, 1, 0b1), PostDecision::Offline));
    assert!(matches!(classify_post(0, 0, 0b1), PostDecision::Local));
    assert!(matches!(classify_post(0, 1, 0b11), PostDecision::Remote));
    assert!(IPI_ERR_HART_OFFLINE < IPI_ERR_HART_OUT_OF_RANGE);
};
