// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340d** — hộp thư IPI dùng chung (`vifast_plat_common::ipi::IpiMailbox`) + **TLB shootdown
//! đa hart** trên x86_64 (D2-238, D2-244, D2-247, D2-68, D2-77; đóng lỗi v1 **N5**).
//!
//! # Hai tầng (đính chính kiến trúc của D2-244)
//!
//! `riscv64` (`KT-336c`) và `aarch64` (`KT-338c`) đã nối **tầng 1** của Audit Q1-A: một cơ chế
//! *bitmask kind* (`IpiMailbox` — coalesce theo kind, **không bao giờ đầy**) + `IrqMcsLock`.
//! Tầng 1 **không** mang payload; tầng 2 (`vifast_sync::Mailbox<N>`, Chu kỳ 3/`SMP-1`) mới mang
//! `{hart, tcb}`/`{asid, va}`. Tệp này hoàn tất **3/3 port** của tầng 1.
//!
//! Chia việc ở đây **đúng** theo hai tầng đó:
//!
//! - **Kind** (`TlbFlush`/`VmKick`/`Wake`) quyết định *có gì đang chờ* — bit nằm trong
//!   [`IPI_MAILBOXES`] của hart đích, đặt **trước** khi gửi IPI. Gửi trùng một kind đang chờ thì
//!   **coalesce** (không gửi IPI thứ hai) — ngữ nghĩa D2-68/D2-77 của `riscv64`.
//! - **Payload tối thiểu** (`va`, `mode`, `root`) nằm trong một **ô yêu cầu riêng mỗi hart**
//!   ([`FlushSlot`]) — đúng tinh thần tầng 2 nhưng chỉ đủ cho một yêu cầu đang bay: người gửi
//!   **giữ chỗ** (`claim` CAS) trước khi ghi, hart nhận đọc sau khi `drain`, và **chỉ trả chỗ** khi
//!   đã thấy ACK (hoặc hết trần). Nhờ vậy coalesce của bitmask **không** làm mất VA: hai yêu cầu
//!   khác VA không bao giờ cùng bay tới một hart.
//!
//! # Đường đi (đúng thứ tự, sai thứ tự là mất kind hoặc mất flush)
//!
//! ```text
//!  người gửi (hart A)                       hart đích (hart B)
//!  ─────────────────                        ──────────────────
//!  claim slot[B] (CAS 0→1)
//!  ghi va/mode/root  (Release)
//!  mailbox[B].post(TlbFlush)  ── bit ──┐
//!  ICR fixed-delivery vector 0x30 ─────┼──►  trap 0x30 → drain_current()
//!                                       │      mailbox[B].drain() → KindSet
//!  chờ ACK (wait_ack, có trần)  ◄───────┘      invlpg(va)  (+ observe/chain nếu mode yêu cầu)
//!  release slot[B]                            slot[B].acked = seq  (Release)
//! ```
//!
//! - **Bit vào hộp thư TRƯỚC, IPI sau** ([`post`]): nếu ngược lại, hart đích có thể nhận IPI,
//!   `drain` ra **rỗng** rồi quay lại `hlt` trong khi bit vẫn chưa được đặt — mất đánh thức.
//! - **IPI lỗi ⇒ gỡ bit** (D2-77): [`post`] gọi [`IpiMailbox::unpost`] khi `ICR` không gửi được,
//!   nên không có bit mồ côi (không ai drain).
//! - **Không self-target** (D2-68): [`post`] trả [`PostOutcome::Local`] khi `hart` là hart đang
//!   chạy và **không** gửi IPI; `VSpace::flush_va` flush **cục bộ** rồi mới shootdown các hart khác.
//! - **Hart chưa công bố `LAPIC_ID` ⇒ `Invalid`** (khuôn D2-77 cho "hart chưa online"): không ghi
//!   bit, không gửi, không để lại kind vô chủ.
//!
//! # Chờ ACK **có drain** (bài học D2-68, đúng lỗi v1 **N5**)
//!
//! [`shootdown`] chờ ACK bằng [`wait_with_drain`] của crate dùng chung: **mỗi** bước lặp gọi
//! [`drain_current`] **trước** khi kiểm điều kiện. Đây không phải trang trí: lỗi v1
//! (`docs/v1/KNOWN-ISSUES.md` #5 — "*x86 AP→BSP*") là hart chờ ACK trong vùng `IF = 0` mà **không**
//! xử lý hộp thư của chính nó ⇒ yêu cầu shootdown của hart kia không bao giờ được phục vụ ⇒
//! deadlock (hoặc timeout rồi đọc dữ liệu cũ). Đột biến `mut-no-wait-drain` bỏ đúng lời gọi
//! `drain` đó ⇒ ca `# tlb: chain …` **ĐỎ** (`ack=0`), chứng minh phép chờ **có răng**.
//!
//! # Vì sao **không** bật PCID (nửa còn lại của N5)
//!
//! `CR4.PCIDE = 0` **có kiểm soát**, kèm lý do ghi trong [`PcidStatus`]: khi PCID tắt, entry TLB
//! **không** được gắn thẻ theo `CR3`, nên `invlpg(va)` xoá đúng entry ấy **bất kể** nó được nạp
//! dưới cây bảng trang nào, và một lần ghi `CR3` xoá **toàn bộ** entry không-`G`. Đó là cơ chế
//! **mạnh hơn** (không cần `INVPCID` theo PCID, không có nguy cơ flush nhầm/không flush do thẻ
//! PCID cũ), đúng nhánh "tắt PCID có kiểm soát + ghi lý do" của tiêu chí đóng N5. [`pcid_status`]
//! **đo** `CPUID.01H:ECX.PCID` trên chính ảnh đang chạy (in ở dòng `# ipi: pcid …`) để lựa chọn
//! này là dữ kiện đo được, không phải lời hứa — chạy cả `-cpu qemu64` (không có PCID) lẫn
//! `-cpu max` (có) cho cùng kết quả hành vi.
//!
//! # Phạm vi
//!
//! Có: bitmask + IPI + shootdown theo VA + ACK có trần + drain-khi-chờ + chuỗi hai chiều (để đo
//! đúng lỗi N5) + bộ đếm làm bằng chứng. **Không**: `INVPCID`/PCID, shootdown theo *range*
//! (kernel chưa có range unmap), IPI tới hart **chưa online**, preemption trong handler.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_common::ipi::{
    classify_post, wait_with_drain, IpiKind, KindSet, WaitOutcome, IPI_MAILBOXES,
};

use crate::apic;
use crate::asm;
use crate::control::MSR_IA32_APIC_BASE;
use crate::exc;
use crate::smp;

/// Vector **IPI** của ảnh — vector `0x30` có stub riêng (`KT-278`), dùng chung cho **mọi** kind.
///
/// Trên x86_64, kind **không** được mã hoá trong vector (khác `SGI` aarch64): một vector duy nhất
/// + bitmask là **nguồn sự thật** của kind, nên thêm kind không cần thêm ô `IDT`/vector.
pub const IPI_VECTOR: u8 = exc::VEC_IPI;

/// Số hart tối đa — **phải** khớp `vifast_plat_common::smp::MAX_HARTS` và [`smp::MAX_HARTS`].
pub const MAX_HARTS: usize = smp::MAX_HARTS;

/// `LAPIC_ID` **chưa** được hart nào công bố (sentinel).
pub const NO_LAPIC_ID: u64 = u64::MAX;

/// Trần vòng chờ `ICR.Delivery Status` về `0` — ~100 ms dưới TCG (cùng bậc `smp::IPI_TIMEOUT_US`).
pub const ICR_IDLE_SPINS: u64 = 2_000_000;

/// Trần **số vòng** chờ ACK của một lần shootdown (không phải thời gian — bài học `KT-301`).
///
/// Phải đủ lớn cho một vòng khứ hồi **hai** hart dưới `TCG` bốn vCPU trên máy đang bận, nhưng vẫn
/// hữu hạn để một hart chết chỉ làm ca kiểm **ĐỎ**, không treo máy. `60.000.000` vòng `pause` cách
/// nhau ≈ hàng trăm ms — cùng bậc `smp::SMP_SPIN_LIMIT` (100.000.000) đang dùng cho chờ online.
pub const ACK_SPIN_LIMIT: u64 = 60_000_000;

// ---------------------------------------------------------------------------------------------
// Bảng hart ↔ LAPIC ID (do ảnh công bố; arch crate không tự biết topology)
// ---------------------------------------------------------------------------------------------

/// `LAPIC_ID` của từng hart (`NO_LAPIC_ID` = chưa công bố).
static HART_LAPIC_ID: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(NO_LAPIC_ID) }; MAX_HARTS];
/// Bit `h` ⇔ hart `h` đã công bố `LAPIC_ID` (tức **online** theo nghĩa của tệp này).
static HART_ONLINE: AtomicU32 = AtomicU32::new(0);

/// Công bố `LAPIC_ID` của `hart` — ảnh gọi **một lần** cho mỗi hart (BSP lúc `lapic_init`, AP
/// ngay sau khi đọc/kiểm `LAPIC_ID` của chính nó), **trước** khi bật bit online của hart ấy.
///
/// `false` ⇔ hart ngoài `MAX_HARTS` (không ghi gì — fail-closed).
pub fn set_lapic_id(hart: usize, id: u8) -> bool {
    if hart >= MAX_HARTS {
        return false;
    }
    HART_LAPIC_ID[hart].store(id as u64, Ordering::Release);
    HART_ONLINE.fetch_or(1u32 << hart, Ordering::Release);
    true
}

/// `LAPIC_ID` đã công bố của `hart` (`None` ⇔ ngoài dải hoặc chưa công bố).
pub fn lapic_id_of(hart: usize) -> Option<u8> {
    if hart >= MAX_HARTS {
        return None;
    }
    let id = HART_LAPIC_ID[hart].load(Ordering::Acquire);
    if id == NO_LAPIC_ID {
        None
    } else {
        Some(id as u8)
    }
}

/// Mặt nạ hart đã công bố — bằng chứng `# ipi: … online=0x…`.
pub fn online_mask() -> u32 {
    HART_ONLINE.load(Ordering::Acquire)
}

/// Hart **đang chạy** — đọc `LAPIC_ID` từ MMIO của LAPIC **trên chính hart này** rồi tra bảng.
///
/// Không tin một biến toàn cục "hart hiện tại": cùng lý do `KT-310` (`smp::hart_of_lapic` của ảnh)
/// — một ô `static` dùng chung cho bốn hart là **sai** ngay khi hai hart chạy đồng thời.
pub fn current_hart() -> Option<usize> {
    let id = current_lapic_id()?;
    let mut hart = 0;
    while hart < MAX_HARTS {
        if lapic_id_of(hart) == Some(id) {
            return Some(hart);
        }
        hart += 1;
    }
    None
}

/// `LAPIC_ID` của hart đang chạy (MMIO), `None` nếu LAPIC chưa bật/không map.
fn current_lapic_id() -> Option<u8> {
    let base = lapic_base()?;
    Some(apic::lapic_id(asm::read_u32(base + apic::LAPIC_ID as u64)))
}

/// Địa chỉ MMIO của LAPIC **của hart đang chạy** từ `IA32_APIC_BASE`; `None` nếu APIC tắt.
fn lapic_base() -> Option<u64> {
    let msr = asm::read_msr(MSR_IA32_APIC_BASE);
    if msr & apic::APIC_BASE_ENABLE == 0 {
        return None;
    }
    let base = msr & apic::APIC_BASE_ADDR_MASK;
    if base == 0 {
        None
    } else {
        Some(base)
    }
}

// ---------------------------------------------------------------------------------------------
// Bộ đếm — bằng chứng, không phải trang trí
// ---------------------------------------------------------------------------------------------

/// Ảnh chụp bộ đếm IPI (toàn ảnh) — in ở dòng `# ipi: mailbox …`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IpiStats {
    /// Số lần `post` (mọi hart gửi, mọi kind).
    pub posted: u64,
    /// Số lần bit đã chờ sẵn ⇒ **coalesce** (không gửi IPI thứ hai).
    pub coalesced: u64,
    /// Số IPI **đã gửi được** (`ICR` fixed-delivery).
    pub sent: u64,
    /// Số lần `drain` **có** kind (kind, không phải lần gọi).
    pub drained: u64,
    /// Số lần `drain` gặp bitmask **rỗng** (`> 0` là mất đánh thức / đột biến — phải là `0`).
    pub empty: u64,
    /// Số ACK đã ghi (mỗi yêu cầu `TlbFlush` được phục vụ xong).
    pub acked: u64,
    /// Số lần IPI **không** gửi được (hoặc hart đích chưa công bố) ⇒ bit đã được gỡ (D2-77).
    pub invalid: u64,
    /// Số lần `post` self-target xử lý **cục bộ** (D2-68 — không IPI).
    pub local: u64,
    /// Số lần chờ ACK hết trần (mỗi **hart đích** hết trần tính một lần).
    pub timeouts: u64,
    /// Số lần một yêu cầu bị **từ chối** vì ô của hart đích còn bận (chưa ACK).
    pub busy: u64,
}

macro_rules! counter {
    ($name:ident) => {
        static $name: AtomicU64 = AtomicU64::new(0);
    };
}

counter!(POSTED);
counter!(COALESCED);
counter!(SENT);
counter!(DRAINED);
counter!(EMPTY);
counter!(ACKED);
counter!(INVALID);
counter!(LOCAL);
counter!(TIMEOUTS);
counter!(BUSY);

/// Ảnh chụp bộ đếm (một lần đọc mỗi ô — đủ cho log bằng chứng).
pub fn stats() -> IpiStats {
    IpiStats {
        posted: POSTED.load(Ordering::Relaxed),
        coalesced: COALESCED.load(Ordering::Relaxed),
        sent: SENT.load(Ordering::Relaxed),
        drained: DRAINED.load(Ordering::Relaxed),
        empty: EMPTY.load(Ordering::Relaxed),
        acked: ACKED.load(Ordering::Relaxed),
        invalid: INVALID.load(Ordering::Relaxed),
        local: LOCAL.load(Ordering::Relaxed),
        timeouts: TIMEOUTS.load(Ordering::Relaxed),
        busy: BUSY.load(Ordering::Relaxed),
    }
}

/// Xoá bộ đếm — **chỉ** cho test (ảnh không gọi: bộ đếm là bằng chứng tích luỹ của cả boot).
pub fn reset_stats() {
    for slot in [
        &POSTED, &COALESCED, &SENT, &DRAINED, &EMPTY, &ACKED, &INVALID, &LOCAL, &TIMEOUTS, &BUSY,
    ] {
        slot.store(0, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------------------------
// Gửi IPI (ICR fixed-delivery) — không self-target, lỗi ⇒ gỡ bit
// ---------------------------------------------------------------------------------------------

/// Kết quả một lần [`post`] — đo được, không đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostOutcome {
    /// Bit vừa đặt **và** IPI đã gửi xong.
    Sent,
    /// Kind đã đang chờ ⇒ **coalesce**, không gửi IPI thứ hai.
    Coalesced,
    /// Hart đích **chính là hart đang chạy** ⇒ xử lý cục bộ, không IPI (D2-68).
    Local,
    /// Hart ngoài `MAX_HARTS`, chưa công bố `LAPIC_ID`, hoặc IPI không gửi được (bit đã gỡ).
    Invalid,
}

impl PostOutcome {
    /// `true` ⇔ kind **đã** nằm trong hộp thư đích (dù vừa đặt hay đã chờ từ trước).
    pub const fn queued(self) -> bool {
        matches!(self, Self::Sent | Self::Coalesced)
    }

    /// Tên ổn định cho log/TAP (`sent`/`coalesced`/`local`/`invalid`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Coalesced => "coalesced",
            Self::Local => "local",
            Self::Invalid => "invalid",
        }
    }
}

/// Gửi **một** IPI fixed-delivery tới `dest` (APIC ID); `false` ⇔ `ICR` không rảnh hoặc lệnh
/// không hoàn tất trong [`ICR_IDLE_SPINS`] vòng.
///
/// Thứ tự theo SDM Vol 3A §10.6.1: chờ `ICR.Delivery Status = 0` **trước** khi ghi, ghi `HIGH`
/// (destination) rồi `LOW` (delivery mode + vector), và chờ `0` **sau** để lệnh thực sự đi.
pub fn send_ipi(dest: u8) -> bool {
    let Some(base) = lapic_base() else {
        return false;
    };
    if !wait_icr_idle(base) {
        return false;
    }
    asm::write_u32(base + smp::LAPIC_ICR_HIGH as u64, smp::icr_high(dest));
    asm::write_u32(
        base + smp::LAPIC_ICR_LOW as u64,
        smp::icr_fixed_low(IPI_VECTOR),
    );
    wait_icr_idle(base)
}

/// Chờ `ICR.Delivery Status` về `0` trong trần [`ICR_IDLE_SPINS`].
fn wait_icr_idle(base: u64) -> bool {
    let mut spins = 0u64;
    while spins < ICR_IDLE_SPINS {
        let low = asm::read_u32(base + smp::LAPIC_ICR_LOW as u64);
        if !smp::icr_delivery_pending(low) {
            return true;
        }
        spins += 1;
        if spins & 0x3F == 0 {
            core::hint::spin_loop();
        }
    }
    false
}

/// Đặt `kind` cho `hart` **rồi** gửi IPI khi cần — **bit trước, chuông sau**.
///
/// Thứ tự là **hợp đồng**: bit phải nằm trong hộp thư **trước** khi IPI có thể tới (nếu ngược lại,
/// hart đích `drain` ra rỗng rồi `hlt` ⇒ mất kind — xem đầu tệp). Khi IPI **không** gửi được, bit
/// vừa đặt được **gỡ** ngay (D2-77) để không có kind mồ côi.
pub fn post(hart: usize, kind: IpiKind) -> PostOutcome {
    POSTED.fetch_add(1, Ordering::Relaxed);
    let Some(mailbox) = IPI_MAILBOXES.try_get(hart) else {
        INVALID.fetch_add(1, Ordering::Relaxed);
        return PostOutcome::Invalid;
    };
    // D2-68 — **không self-target**: hart đang chạy thì không gửi IPI cho chính nó.
    if current_hart() == Some(hart) {
        LOCAL.fetch_add(1, Ordering::Relaxed);
        return PostOutcome::Local;
    }
    // D2-77 — hart chưa công bố `LAPIC_ID` (chưa online) ⇒ `Invalid`, **không** giữ kind.
    let Some(dest) = lapic_id_of(hart) else {
        INVALID.fetch_add(1, Ordering::Relaxed);
        return PostOutcome::Invalid;
    };
    let fresh = mailbox.post(kind);
    if !fresh {
        COALESCED.fetch_add(1, Ordering::Relaxed);
        return PostOutcome::Coalesced;
    }
    if send_ipi(dest) {
        SENT.fetch_add(1, Ordering::Relaxed);
        PostOutcome::Sent
    } else {
        // IPI không đi được ⇒ **gỡ bit vừa đặt** (D2-77: bit mồ côi không ai drain).
        let _ = mailbox.unpost(kind);
        INVALID.fetch_add(1, Ordering::Relaxed);
        PostOutcome::Invalid
    }
}

/// Quyết định gửi của `send_ipi` theo `online_mask` (hàm thuần của crate dùng chung) — dùng cho
/// **ca âm** ở test host: một hart chưa online **không** nhận `TlbFlush`.
pub const fn post_decision(sender: usize, target: usize, online_mask: u32) -> u8 {
    match classify_post(sender, target, online_mask as u64) {
        vifast_plat_common::ipi::PostDecision::InvalidHart => 0,
        vifast_plat_common::ipi::PostDecision::Offline => 1,
        vifast_plat_common::ipi::PostDecision::Local => 2,
        vifast_plat_common::ipi::PostDecision::Remote => 3,
    }
}

// ---------------------------------------------------------------------------------------------
// Ô yêu cầu shootdown (payload tối thiểu của hart đích)
// ---------------------------------------------------------------------------------------------

/// Số byte một trang 4 KiB — đơn vị của shootdown theo **range** (KT-415).
pub const PAGE_BYTES: u64 = 4096;

/// Bit [`FlushMode`]: hart nhận **đọc lại** `va` sau khi `invlpg` và ghi vào `obs`.
pub const MODE_OBSERVE: u64 = 1 << 0;
/// Bit [`FlushMode`]: hart nhận **nối** một yêu cầu nữa tới `chain` hart cho `chain_va` (đo đúng
/// lỗi v1 **N5**: hart đang chờ ACK phải xử lý hộp thư của chính nó).
pub const MODE_CHAIN: u64 = 1 << 1;
/// Bit [`FlushMode`]: hart nhận **ghi** `remap_val` vào ô bảng trang `remap_pa` **trước** khi
/// `invlpg` (đường "hart khác sửa bảng trang" — AP→BSP).
pub const MODE_REMAP: u64 = 1 << 2;

/// Giá trị `chain` "không nối" (hart `0` là BSP nên `0` **không** dùng làm sentinel).
pub const NO_CHAIN: u64 = u64::MAX;

/// Yêu cầu gửi kèm **một** `TlbFlush` (payload tầng 2 tối thiểu, xem đầu tệp).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlushRequest {
    /// Số thứ tự do người gửi cấp; ACK phải mang **đúng** số này.
    pub seq: u64,
    /// VA cần xoá khỏi TLB của hart đích.
    pub va: u64,
    /// Độ dài **range** (byte) bắt đầu từ `va`; `0` ⇔ chỉ một trang (KT-415).
    pub len: u64,
    /// Tổ hợp bit [`MODE_OBSERVE`]/[`MODE_CHAIN`]/[`MODE_REMAP`].
    pub mode: u64,
    /// `CR3` (PA bảng gốc) hart đích phải **dùng** khi phục vụ (`0` = không đổi `CR3`).
    pub root: u64,
    /// Ô bảng trang (PA) để ghi khi có [`MODE_REMAP`] (`0` = không ghi).
    pub remap_pa: u64,
    /// Giá trị ghi vào `remap_pa` khi có [`MODE_REMAP`].
    pub remap_val: u64,
    /// Hart nhận yêu cầu **nối** (khi có [`MODE_CHAIN`]); [`NO_CHAIN`] = không nối.
    pub chain: u64,
    /// `seq` của yêu cầu nối.
    pub chain_seq: u64,
    /// `va` của yêu cầu nối.
    pub chain_va: u64,
}

impl FlushRequest {
    /// Yêu cầu "chỉ xoá `va`" — dạng **sản phẩm** (unmap/`INVLPG` một VA).
    pub const fn flush(seq: u64, va: u64) -> Self {
        Self {
            seq,
            va,
            len: 0,
            mode: 0,
            root: 0,
            remap_pa: 0,
            remap_val: 0,
            chain: NO_CHAIN,
            chain_seq: 0,
            chain_va: 0,
        }
    }

    /// Yêu cầu "xoá **mọi trang** trong `[va, va+len)`" — dạng **range** (KT-415).
    ///
    /// `len == 0` ⇒ đúng một trang (tương thích [`Self::flush`]). Một yêu cầu cho cả range thay
    /// vì `N` yêu cầu đơn: hart đích xoá cả range trong **một** lần phục vụ, một ACK.
    pub const fn flush_range(seq: u64, va: u64, len: u64) -> Self {
        let mut req = Self::flush(seq, va);
        req.len = len;
        req
    }

    /// Số trang của yêu cầu (`len == 0` ⇒ 1).
    pub const fn pages(&self) -> u64 {
        if self.len == 0 {
            1
        } else {
            self.len.div_ceil(PAGE_BYTES)
        }
    }

    /// Yêu cầu "xoá rồi **đọc lại** `va`" — dạng chứng minh mapping mới đã có hiệu lực.
    pub const fn observe(seq: u64, va: u64) -> Self {
        let mut req = Self::flush(seq, va);
        req.mode = MODE_OBSERVE;
        req
    }

    /// `true` ⇔ yêu cầu có bit `bit`.
    pub const fn has(&self, bit: u64) -> bool {
        self.mode & bit != 0
    }
}

/// Ô yêu cầu của **một** hart — người gửi **giữ chỗ** bằng `claim` trước khi ghi.
///
/// `claim = 0` ⇔ ô rảnh; `1` ⇔ đang có yêu cầu bay tới hart này. Chỗ chỉ được trả **sau** khi
/// người gửi thấy ACK (hoặc hết trần) — nhờ vậy coalesce của bitmask không bao giờ tráo VA.
#[derive(Debug)]
struct FlushSlot {
    claim: AtomicU32,
    seq: AtomicU64,
    va: AtomicU64,
    len: AtomicU64,
    mode: AtomicU64,
    root: AtomicU64,
    remap_pa: AtomicU64,
    remap_val: AtomicU64,
    chain: AtomicU64,
    chain_seq: AtomicU64,
    chain_va: AtomicU64,
    obs: AtomicU64,
    acked: AtomicU64,
}

impl FlushSlot {
    const fn new() -> Self {
        Self {
            claim: AtomicU32::new(0),
            seq: AtomicU64::new(0),
            va: AtomicU64::new(0),
            len: AtomicU64::new(0),
            mode: AtomicU64::new(0),
            root: AtomicU64::new(0),
            remap_pa: AtomicU64::new(0),
            remap_val: AtomicU64::new(0),
            chain: AtomicU64::new(NO_CHAIN),
            chain_seq: AtomicU64::new(0),
            chain_va: AtomicU64::new(0),
            obs: AtomicU64::new(0),
            acked: AtomicU64::new(0),
        }
    }
}

/// **Nguồn `seq` DUY NHẤT** của toàn ảnh — mọi yêu cầu `TlbFlush` (từ `VSpace`, từ test) lấy số
/// từ đây.
///
/// Vì sao **một** nguồn: `ACK` là "ô đã phục vụ tới `seq` ấy". Hai bộ đếm độc lập (ví dụ `VSpace`
/// đếm 1,2,… và một đường khác cũng đếm 1,2,…) sinh ra **trùng `seq`** ⇒ một ACK **cũ** thoả phép
/// chờ của yêu cầu **mới** ⇒ người gửi chạy trước khi hart kia kịp `invlpg` ⇒ `post` kế tiếp thấy
/// bit còn treo (coalesce) và **mất một yêu cầu**. Đo được: bản đầu của `KT-340d` dùng hai bộ đếm
/// và boot GPU (`kt337-gpu`) bắt `coalesced=1 sent=5` ⇒ TAP 37 ĐỎ **không tất định**. Một nguồn
/// `seq` là điều kiện của giao thức, không phải chi tiết cài đặt.
static SEQ_SOURCE: AtomicU64 = AtomicU64::new(0);

/// `seq` kế tiếp (luôn **khác 0** — `0` là "ô rỗng").
pub fn next_seq() -> u64 {
    SEQ_SOURCE.fetch_add(1, Ordering::AcqRel) + 1
}

/// Ô yêu cầu của **mọi** hart (`MAX_HARTS` slot, `const`-init — không cấp phát).
static FLUSH_SLOTS: [FlushSlot; MAX_HARTS] = [const { FlushSlot::new() }; MAX_HARTS];

/// Ảnh chụp ô yêu cầu (bằng chứng `# tlb:`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlushSnapshot {
    /// `seq` đang treo (`0` = chưa có yêu cầu).
    pub seq: u64,
    /// `seq` mà hart nhận đã ACK — phải **bằng** `seq` khi thành công.
    pub acked: u64,
    /// Giá trị hart nhận **đọc lại** `va` sau `invlpg` (chỉ có nghĩa với [`MODE_OBSERVE`]).
    pub obs: u64,
    /// `1` ⇔ ô đang bị giữ chỗ.
    pub claimed: u32,
}

/// Ảnh chụp ô yêu cầu của `hart`.
pub fn flush_snapshot(hart: usize) -> FlushSnapshot {
    let Some(slot) = FLUSH_SLOTS.get(hart) else {
        return FlushSnapshot::default();
    };
    FlushSnapshot {
        seq: slot.seq.load(Ordering::Acquire),
        acked: slot.acked.load(Ordering::Acquire),
        obs: slot.obs.load(Ordering::Acquire),
        claimed: slot.claim.load(Ordering::Acquire),
    }
}

/// `true` ⇔ `bit` nằm trong `mode` (hàm thuần, test host được).
pub const fn mode_has(mode: u64, bit: u64) -> bool {
    mode & bit != 0
}

/// `true` ⇔ ACK đã tới: `acked == seq` và `seq != 0` (hàm thuần).
///
/// `0` **không** phải số thứ tự hợp lệ (ô rỗng + `acked = 0` **không** được coi là "đã ACK").
pub const fn ack_ok(acked: u64, seq: u64) -> bool {
    seq != 0 && acked == seq
}

/// Hart **đích** của một shootdown: mọi hart trong `active_on` **trừ** hart `me` (D2-68 —
/// không self-target; hart gọi flush **cục bộ** ngay).
pub const fn shootdown_targets(active_on: u32, me: usize) -> u32 {
    if me >= MAX_HARTS {
        return active_on;
    }
    active_on & !(1u32 << me)
}

// ---------------------------------------------------------------------------------------------
// Ghi yêu cầu + gửi + chờ ACK
// ---------------------------------------------------------------------------------------------

/// Kết quả phục vụ **một** hart đích.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AckOutcome {
    /// ACK đúng `seq` trong trần ⇒ hart đích đã `invlpg` (và đọc lại nếu yêu cầu).
    Acked,
    /// Hết trần [`ACK_SPIN_LIMIT`] trước khi thấy ACK — **không** treo.
    Timeout,
    /// Ô của hart đích còn bận (một yêu cầu khác chưa ACK) — người gửi **không** ghi đè.
    Busy,
    /// Không gửi được (hart chưa công bố `LAPIC_ID` / `ICR` lỗi / self-target).
    NotSent,
}

impl AckOutcome {
    /// Tên ổn định cho log bằng chứng (`acked`/`timeout`/`busy`/`not_sent`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Acked => "acked",
            Self::Timeout => "timeout",
            Self::Busy => "busy",
            Self::NotSent => "not_sent",
        }
    }
}

/// Ghi `req` vào ô của `hart` rồi `post(TlbFlush)`. Trả `false` ⇔ ô **bận** hoặc `hart` không
/// nhận được (khi đó **không** có bit nào được đặt — người gửi kiểm lại bằng [`AckOutcome`]).
///
/// Người gọi **phải** [`release_slot`] khi xong (kể cả nhánh lỗi) — nếu không, mọi yêu cầu sau
/// tới hart ấy bị từ chối (`Busy`), đúng fail-closed.
pub fn issue(hart: usize, req: &FlushRequest) -> bool {
    let Some(slot) = FLUSH_SLOTS.get(hart) else {
        return false;
    };
    if slot
        .claim
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        BUSY.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    // Xoá ACK cũ **trước** khi ghi yêu cầu mới: một ACK sót lại của yêu cầu trước không bao giờ
    // được thoả phép chờ của yêu cầu này (chuông vừa rung xong ⇒ `acked` cũ vẫn nằm đó).
    slot.acked.store(0, Ordering::Release);
    slot.seq.store(req.seq, Ordering::Release);
    slot.va.store(req.va, Ordering::Release);
    slot.len.store(req.len, Ordering::Release);
    slot.mode.store(req.mode, Ordering::Release);
    slot.root.store(req.root, Ordering::Release);
    slot.remap_pa.store(req.remap_pa, Ordering::Release);
    slot.remap_val.store(req.remap_val, Ordering::Release);
    slot.chain.store(req.chain, Ordering::Release);
    slot.chain_seq.store(req.chain_seq, Ordering::Release);
    slot.chain_va.store(req.chain_va, Ordering::Release);
    if post(hart, IpiKind::TlbFlush).queued() {
        true
    } else {
        // Không đặt được bit nào ⇒ trả chỗ ngay (không có ai sẽ ACK).
        release_slot(hart);
        false
    }
}

/// Trả chỗ ô yêu cầu của `hart` (sau khi ACK hoặc hết trần).
pub fn release_slot(hart: usize) {
    if let Some(slot) = FLUSH_SLOTS.get(hart) {
        slot.claim.store(0, Ordering::Release);
    }
}

/// Chờ ACK của một hart — **có drain** hộp thư của chính mình trong lúc chờ.
///
/// Đây là bài học D2-68 và là **chính** lỗi v1 N5: hart chờ ACK mà không xử lý hộp thư của mình
/// thì yêu cầu của hart kia không bao giờ được phục vụ. Hàm dùng [`wait_with_drain`] của crate
/// dùng chung nên ngữ nghĩa "drain trước mỗi bước, hết trần là dừng" là **một** bản duy nhất cho
/// ba port. `drain` bị tắt **chỉ** ở đột biến `mut-no-wait-drain`.
pub fn wait_ack(hart: usize, seq: u64) -> AckOutcome {
    let Some(slot) = FLUSH_SLOTS.get(hart) else {
        return AckOutcome::NotSent;
    };
    if seq == 0 {
        return AckOutcome::NotSent;
    }
    let mut spins = 0u64;
    let outcome = wait_with_drain(
        ACK_SPIN_LIMIT,
        || {
            if spins & 0x3F == 0 {
                core::hint::spin_loop();
            }
            spins = spins.saturating_add(1);
            spins
        },
        || {
            // **N5/D2-68** — phục vụ hộp thư của chính mình trước khi kiểm ACK.
            #[cfg(not(feature = "mut-no-wait-drain"))]
            {
                let _ = drain_waiting();
            }
        },
        || ack_ok(slot.acked.load(Ordering::Acquire), seq),
    );
    match outcome {
        WaitOutcome::Ready => AckOutcome::Acked,
        WaitOutcome::TimedOut => {
            TIMEOUTS.fetch_add(1, Ordering::Relaxed);
            AckOutcome::Timeout
        }
    }
}

/// Shootdown **một** VA tới mọi hart trong `targets` (`bit h` = hart `h`), rồi chờ **đủ** ACK.
///
/// Trả `(số hart đích, số ACK)` — ca kiểm đòi hai số **bằng nhau** (và bằng `targets.count_ones()`),
/// nên "gửi xong" không bao giờ bị nhầm với "hart kia đã xoá TLB".
///
/// `mode` cho biết hart nhận có **đọc lại** VA ([`MODE_OBSERVE`]) hay không; `seq` do người gọi cấp
/// (một bộ đếm tăng dần, **khác 0**).
pub fn shootdown(targets: u32, va: u64, mode: u64, seq: u64) -> (u32, u32) {
    shootdown_range(targets, va, 0, mode, seq)
}

/// Shootdown **một range** `[va, va+len)` tới mọi hart trong `targets`, rồi chờ **đủ** ACK.
///
/// **KT-415**: một yêu cầu cho cả range (một bit chuông, một ACK mỗi hart) thay vì `N` yêu cầu
/// đơn — dùng cho teardown vùng/`VSpace::free_range`. `len == 0` ⇒ đúng một trang, cùng ngữ nghĩa
/// [`shootdown`]. `MODE_OBSERVE` chỉ đọc lại **VA đầu** (đọc cả range là việc của caller).
pub fn shootdown_range(targets: u32, va: u64, len: u64, mode: u64, seq: u64) -> (u32, u32) {
    let mut acked = 0u32;
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        let bit = 1u32 << hart;
        if targets & bit == 0 {
            hart += 1;
            continue;
        }
        let req = FlushRequest {
            seq,
            va,
            len,
            mode,
            root: 0,
            remap_pa: 0,
            remap_val: 0,
            chain: NO_CHAIN,
            chain_seq: 0,
            chain_va: 0,
        };
        if issue(hart, &req) {
            if wait_ack(hart, seq) == AckOutcome::Acked {
                acked += 1;
            }
            release_slot(hart);
        }
        hart += 1;
    }
    (targets.count_ones(), acked)
}

// ---------------------------------------------------------------------------------------------
// Phục vụ phía hart nhận — chạy trong handler vector 0x30
// ---------------------------------------------------------------------------------------------

/// `drain` hộp thư của hart **đang chạy** và **phục vụ** mọi kind nhận được; trả `KindSet` vừa rút.
///
/// Gọi từ handler ngắt vector [`IPI_VECTOR`] (`#  trap` của ảnh) **và** từ vòng chờ ACK (bài học
/// D2-68: chờ ở đâu thì phục vụ ở đó). Với `TlbFlush`: đổi `CR3` nếu yêu cầu chỉ định gốc khác,
/// ghi ô bảng trang nếu có [`MODE_REMAP`], `invlpg(va)`, đọc lại nếu có [`MODE_OBSERVE`], nối yêu
/// cầu nếu có [`MODE_CHAIN`], **rồi** mới ghi ACK (`acked = seq`) — thứ tự này để người gửi không
/// bao giờ thấy ACK trước khi việc xoá TLB xảy ra.
pub fn drain_current() -> KindSet {
    drain_inner(true)
}

/// `drain` trong **vòng chờ** (bài học D2-68) — khác [`drain_current`] **duy nhất** ở chỗ **không**
/// đếm `empty`.
///
/// Vì sao: vòng chờ ACK gọi `drain` **mỗi** bước lặp, nên phần lớn lần gọi gặp hộp thư rỗng — đó là
/// trạng thái **bình thường**, không phải "IPI tới mà không có kind". `empty > 0` chỉ có nghĩa khi
/// nó đi kèm một IPI **thật** (handler), nên chỉ [`drain_current`] (đường handler) mới cộng.
pub fn drain_waiting() -> KindSet {
    drain_inner(false)
}

/// Thân chung của hai đường drain: rút kind, rồi **phục vụ** `TlbFlush`.
fn drain_inner(count_empty: bool) -> KindSet {
    let Some(hart) = current_hart() else {
        if count_empty {
            EMPTY.fetch_add(1, Ordering::Relaxed);
        }
        return KindSet::empty();
    };
    let kinds = IPI_MAILBOXES.get(hart).drain();
    if kinds.is_empty() {
        if count_empty {
            EMPTY.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        DRAINED.fetch_add(1, Ordering::Relaxed);
    }
    if kinds.contains(IpiKind::TlbFlush) {
        service_flush(hart);
    }
    kinds
}

/// Phục vụ **một** yêu cầu `TlbFlush` của `hart` (đọc ô, xoá TLB, ACK).
fn service_flush(hart: usize) {
    let Some(slot) = FLUSH_SLOTS.get(hart) else {
        return;
    };
    let seq = slot.seq.load(Ordering::Acquire);
    if seq == 0 {
        return;
    }
    let root = slot.root.load(Ordering::Acquire);
    if root != 0 && asm::read_cr3() & !0xFFF != root & !0xFFF {
        asm::write_cr3(root);
    }
    let mode = slot.mode.load(Ordering::Acquire);
    if mode_has(mode, MODE_REMAP) {
        let pa = slot.remap_pa.load(Ordering::Acquire);
        if pa != 0 {
            asm::write_u64(pa, slot.remap_val.load(Ordering::Acquire));
        }
    }
    let va = slot.va.load(Ordering::Acquire);
    let len = slot.len.load(Ordering::Acquire);
    // **Đột biến (ii)** (`mut-no-ipi-invlpg`): bỏ **đúng** lệnh `invlpg` của handler ⇒ hart nhận
    // vẫn ACK nhưng TLB còn cũ ⇒ ca `# tlb: shootdown` ĐỎ. Chỉ dùng cho đột biến.
    #[cfg(not(feature = "mut-no-ipi-invlpg"))]
    {
        // **KT-415** — range: `len == 0` ⇒ đúng một trang (đường cũ giữ nguyên từng lệnh).
        let pages = if len == 0 {
            1
        } else {
            len.div_ceil(PAGE_BYTES)
        };
        for page in 0..pages {
            asm::invlpg(va + page * PAGE_BYTES);
        }
    }
    let obs = if mode_has(mode, MODE_OBSERVE) {
        asm::read_u64(va)
    } else {
        0
    };
    slot.obs.store(obs, Ordering::Release);
    if mode_has(mode, MODE_CHAIN) {
        let chain = slot.chain.load(Ordering::Acquire);
        if chain != NO_CHAIN && chain < MAX_HARTS as u64 {
            let req = FlushRequest {
                seq: slot.chain_seq.load(Ordering::Acquire),
                va: slot.chain_va.load(Ordering::Acquire),
                len: 0,
                mode: MODE_OBSERVE,
                root: 0,
                remap_pa: 0,
                remap_val: 0,
                chain: NO_CHAIN,
                chain_seq: 0,
                chain_va: 0,
            };
            // Nối **trước** khi ACK: người gửi chỉ thấy ACK khi yêu cầu nối đã nằm trong hộp thư
            // của hart kia (nếu không, phép đo "hart đang chờ ACK có xử lý hộp thư" không tất định).
            let _ = issue(chain as usize, &req);
        }
    }
    slot.acked.store(seq, Ordering::Release);
    ACKED.fetch_add(1, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------------------------
// PCID / PGE — nhánh "tắt có kiểm soát + ghi lý do" của N5
// ---------------------------------------------------------------------------------------------

/// Trạng thái **PCID**/**PGE** của ảnh — in ở dòng `# ipi: pcid …` (D2-80: bằng chứng neo ảnh).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PcidStatus {
    /// `CPUID.01H:ECX[17]` — CPU **có** PCID.
    pub cpuid_pcid: bool,
    /// `CPUID.01H:EDX[13]` — CPU **có** PGE (global pages).
    pub cpuid_pge: bool,
    /// `CR4.PCIDE` hiện tại (ảnh **giữ** `0` — xem lý do ở đầu tệp).
    pub cr4_pcid: bool,
    /// `CR4.PGE` hiện tại (ảnh **giữ** `0`: mọi trang đều là entry không-`G`).
    pub cr4_pge: bool,
}

impl PcidStatus {
    /// `true` ⇔ lựa chọn "tắt PCID/PGE có kiểm soát" đang **đúng** như thiết kế: cả hai đều tắt,
    /// bất kể CPU có hỗ trợ hay không ⇒ `INVLPG` + `CR3` là toàn bộ cơ chế xoá TLB.
    pub const fn full_flush_ok(&self) -> bool {
        !self.cr4_pcid && !self.cr4_pge
    }
}

/// Đo `CPUID.01H` và đọc `CR4` để chốt lựa chọn PCID/PGE (xem [`PcidStatus`]).
pub fn pcid_status() -> PcidStatus {
    let (_, ecx, edx) = cpuid_leaf1();
    let cr4 = asm::read_cr4();
    PcidStatus {
        cpuid_pcid: ecx & (1 << 17) != 0,
        cpuid_pge: edx & (1 << 13) != 0,
        cr4_pcid: cr4 & crate::control::CR4_PCIDE != 0,
        cr4_pge: cr4 & crate::control::CR4_PGE != 0,
    }
}

/// `CPUID.01H` → `(eax, ecx, edx)` bằng `asm!` trong [`crate::asm`] (tệp này **không** `unsafe`).
fn cpuid_leaf1() -> (u32, u32, u32) {
    asm::cpuid_leaf1()
}

// Bất biến hằng số — sai một trong số này thì hộp thư/vector không còn khớp `IDT`/`ICR`.
const _: () = {
    assert!(IPI_VECTOR == 0x30);
    assert!(MAX_HARTS == 8);
    assert!(MAX_HARTS <= 32); // `active_on`/`online_mask` là `u32`
    assert!(MODE_OBSERVE != 0 && MODE_CHAIN != 0 && MODE_REMAP != 0);
    assert!(MODE_OBSERVE & MODE_CHAIN == 0 && MODE_CHAIN & MODE_REMAP == 0);
    assert!(NO_CHAIN >= MAX_HARTS as u64);
    assert!(ack_ok(7, 7) && !ack_ok(0, 0) && !ack_ok(6, 7));
    assert!(mode_has(MODE_OBSERVE | MODE_CHAIN, MODE_CHAIN));
    assert!(shootdown_targets(0b1111, 0) == 0b1110);
    assert!(shootdown_targets(0b1111, 3) == 0b0111);
    assert!(shootdown_targets(0b1111, 8) == 0b1111);
    assert!(ACK_SPIN_LIMIT > 0);
    assert!(ICR_IDLE_SPINS > 0);
};
