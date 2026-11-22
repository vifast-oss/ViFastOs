// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338c** (D2-244 tầng 1) — **hộp thư IPI bitmask dùng chung** trên aarch64: `SGI` là *chuông*,
//! `vifast_plat_common::ipi::IpiMailbox` là *bitmask kind*.
//!
//! # Vì sao hai tầng (đính chính kiến trúc của D2-244)
//!
//! `riscv64` đã dùng `IpiMailbox` (bitmask `AtomicU64`/hart, **coalesce theo kind, không bao giờ
//! đầy**) + `Mailbox<N>` MPSC FIFO cho payload. `KT-336c` ghi rõ vì sao **không** thay bitmask
//! bằng FIFO: FIFO **đầy ⇒ `Busy`** ⇒ mất đánh thức. Tầng 1 (đóng Audit Q1-A/`N7`) là **một**
//! cơ chế hộp thư bitmask cho **cả 3 port**; tầng 2 (`SMP-1`, Chu kỳ 3) mới là `Mailbox<N>`
//! mang payload **nằm trên** bitmask.
//!
//! # aarch64: SGI là chuông, bitmask là "kind gì đang chờ"
//!
//! - **Gửi**: [`post`] đặt bit kind vào hộp thư **hart đích trước**, rồi gửi `SGI` **chỉ khi** bit
//!   vừa được đặt (kind đã đang chờ ⇒ **coalesce**, không gửi SGI thứ hai — đúng ngữ nghĩa
//!   D2-68/D2-77 của `riscv64`). SGI **không** mang kind được (chỉ 16 số, và ba số đã dùng cho
//!   ba đường khác nhau), nên **bitmask** là nguồn sự thật về kind.
//! - **Nhận**: [`drain`] `swap` toàn bộ kind của hart đang chạy. Kind nằm trong **bitmask** (không
//!   nằm trong `INTID`) nên đọc/ghi `ICC_IAR1_EL1`/`ICC_EOIR1_EL1` không làm mất kind.
//! - **Không self-target** (D2-68): `post(hart == hart đang chạy, …)` trả [`PostOutcome::Local`] và
//!   **không** gửi SGI — xử lý cục bộ do caller làm.
//!
//! # Bộ đếm là **bằng chứng**, không phải trang trí
//!
//! [`stats`] trả `posted`/`coalesced`/`sent`/`drained`/`empty`. `empty > 0` nghĩa là **một SGI
//! tới mà bitmask rỗng** — mất đánh thức/đột biến, caller phải coi là ĐỎ (ảnh in
//! `# ipi: mailbox empty=…`).

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::ipi::{IpiKind, KindSet, IPI_MAILBOXES};

use crate::gic;

/// SGI dùng làm **chuông** cho từng kind — ba số **khác nhau** đã có từ KT-259/262 (`SGI_IPI`,
/// `SGI_SHOOTDOWN`, `SGI_COMMAND`), nên hộp thư **không** đổi giao thức SGI hiện có; nó thêm
/// tầng bitmask quyết định "kind nào đang chờ" và coalesce lần gửi trùng.
pub const fn sgi_for(kind: IpiKind) -> u32 {
    match kind {
        IpiKind::Wake => gic::SGI_IPI,
        IpiKind::TlbFlush => gic::SGI_SHOOTDOWN,
        IpiKind::VmKick => gic::SGI_COMMAND,
    }
}

/// Kết quả [`post`] — đo được, không đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostOutcome {
    /// Bit vừa được đặt **và** SGI đã gửi.
    Sent,
    /// Bit đã đang chờ ⇒ **coalesce**, không gửi SGI thứ hai.
    Coalesced,
    /// Hart đích ngoài `MAX_HARTS` (không ghi bit, không gửi SGI).
    Invalid,
    /// Hart đích **chính là hart đang chạy** ⇒ xử lý cục bộ, không SGI (D2-68).
    Local,
}

impl PostOutcome {
    /// `true` ⇔ kind **đã** nằm trong hộp thư đích (dù vừa đặt hay đã chờ từ trước).
    pub const fn queued(self) -> bool {
        matches!(self, Self::Sent | Self::Coalesced)
    }

    /// Tên ổn định cho log (`sent`/`coalesced`/`invalid`/`local`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Coalesced => "coalesced",
            Self::Invalid => "invalid",
            Self::Local => "local",
        }
    }
}

/// Bộ đếm hộp thư IPI (toàn cục — mỗi ảnh một thế giới, không chia sẻ giữa hai ảnh).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoorbellStats {
    /// Số lần `post` (mọi hart gửi).
    pub posted: u64,
    /// Số lần bit đã đang chờ ⇒ coalesce.
    pub coalesced: u64,
    /// Số SGI đã gửi được.
    pub sent: u64,
    /// Số lần `drain` **có** kind (kind, không phải lần gọi).
    pub drained: u64,
    /// Số lần `drain` gặp bitmask **rỗng** (SGI mà không có kind ⇒ mất đánh thức/đột biến).
    pub empty: u64,
}

static POSTED: AtomicU64 = AtomicU64::new(0);
static COALESCED: AtomicU64 = AtomicU64::new(0);
static SENT: AtomicU64 = AtomicU64::new(0);
static DRAINED: AtomicU64 = AtomicU64::new(0);
static EMPTY: AtomicU64 = AtomicU64::new(0);

/// Hart **đang chạy** theo `MPIDR_EL1` (cùng định nghĩa với `smp::hart_index`).
fn current_hart() -> Option<usize> {
    crate::smp::hart_index(crate::asm::read_mpidr_el1())
}

/// Đặt `kind` cho `hart` **và** gửi `SGI` khi cần — hộp thư **không** self-target (D2-68).
///
/// Thứ tự **bắt buộc**: bit vào hộp thư **trước**, SGI **sau**. Nếu ngược lại, hart đích có thể
/// nhận SGI, drain ra rỗng (mất kind) rồi quay lại `wfe` — đúng lỗi "mất đánh thức" mà tầng
/// bitmask sinh ra để chặn.
pub fn post(hart: usize, kind: IpiKind) -> PostOutcome {
    POSTED.fetch_add(1, Ordering::Relaxed);
    if current_hart() == Some(hart) {
        return PostOutcome::Local;
    }
    let Some(mailbox) = IPI_MAILBOXES.try_get(hart) else {
        return PostOutcome::Invalid;
    };
    // `post` trả `true` ⇔ bit **vừa** được đặt (kind chưa chờ) ⇒ cần SGI.
    let fresh = mailbox.post(kind);
    if !fresh {
        COALESCED.fetch_add(1, Ordering::Relaxed);
        return PostOutcome::Coalesced;
    }
    match gic::send_sgi(hart, sgi_for(kind)) {
        Ok(()) => {
            SENT.fetch_add(1, Ordering::Relaxed);
            PostOutcome::Sent
        }
        Err(_) => {
            // Không gửi được SGI ⇒ **gỡ bit vừa đặt** (D2-77: bit mồ côi không ai drain).
            let _ = mailbox.unpost(kind);
            PostOutcome::Invalid
        }
    }
}

/// Đặt `kind` cho `hart` **không** gửi SGI (dùng khi chuông đã được gửi bằng đường khác).
pub fn post_bit_only(hart: usize, kind: IpiKind) -> bool {
    POSTED.fetch_add(1, Ordering::Relaxed);
    match IPI_MAILBOXES.try_get(hart) {
        Some(mailbox) => {
            let fresh = mailbox.post(kind);
            if !fresh {
                COALESCED.fetch_add(1, Ordering::Relaxed);
            }
            fresh
        }
        None => false,
    }
}

/// `swap` toàn bộ kind của hart **đang chạy** — gọi trong handler SGI.
///
/// `intid` chỉ dùng cho **chẩn đoán**: bitmask là nguồn sự thật của kind, `INTID` là chuông.
pub fn drain_current() -> KindSet {
    let Some(hart) = current_hart() else {
        EMPTY.fetch_add(1, Ordering::Relaxed);
        return KindSet::empty();
    };
    let kinds = IPI_MAILBOXES.get(hart).drain();
    if kinds.is_empty() {
        EMPTY.fetch_add(1, Ordering::Relaxed);
    } else {
        DRAINED.fetch_add(1, Ordering::Relaxed);
    }
    kinds
}

/// Drain hộp thư của hart `hart` (dùng trong test/đo, không phải handler).
pub fn drain_hart(hart: usize) -> KindSet {
    let kinds = IPI_MAILBOXES.get(hart).drain();
    if kinds.is_empty() {
        EMPTY.fetch_add(1, Ordering::Relaxed);
    } else {
        DRAINED.fetch_add(1, Ordering::Relaxed);
    }
    kinds
}

/// Ảnh chụp bộ đếm — in `# ipi: mailbox …` để runner/harness đối chiếu.
pub fn stats() -> DoorbellStats {
    DoorbellStats {
        posted: POSTED.load(Ordering::Relaxed),
        coalesced: COALESCED.load(Ordering::Relaxed),
        sent: SENT.load(Ordering::Relaxed),
        drained: DRAINED.load(Ordering::Relaxed),
        empty: EMPTY.load(Ordering::Relaxed),
    }
}

/// Xoá bộ đếm (dùng cho test host; ảnh không gọi).
pub fn reset_stats() {
    POSTED.store(0, Ordering::Relaxed);
    COALESCED.store(0, Ordering::Relaxed);
    SENT.store(0, Ordering::Relaxed);
    DRAINED.store(0, Ordering::Relaxed);
    EMPTY.store(0, Ordering::Relaxed);
}

/// Bitmask kind đang chờ của `hart` — không drain (đọc để in bằng chứng).
pub fn pending(hart: usize) -> KindSet {
    match IPI_MAILBOXES.try_get(hart) {
        Some(mailbox) => mailbox.pending(),
        None => KindSet::empty(),
    }
}
