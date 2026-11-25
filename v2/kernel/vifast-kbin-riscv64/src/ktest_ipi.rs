// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **IPI** của ảnh test `vifast-kbin-ktest` (KT-50) — tách khỏi `ktest_smp.rs` để giữ luật
//! cấu trúc R1 (≤ 1.500 dòng/tệp).
//!
//! Nối sau pha SMP của KT-49: `ktest_smp::secondary_phase` gọi [`secondary_phase`] trên **hart
//! thứ cấp**, `ktest.rs` gọi [`ipi_phase`] trên **BSP** sau `smp_phase()`. Nội dung TAP 20..24:
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 20 - 100 ipi delivered` (hoặc `- 100 ipi traps (self)` ở `-smp 1`) | 100 IPI **thật** qua SBI + `stvec` (`scause=(1<<63)\|1`) tới peer: `sent=100`, `delivered=100`, `traps=100`, `coalesced=0`, `spurious=0`, per-kind `34/33/33` (**không mất, không nhân đôi**). Ở `-smp 1` (không có hart đích) đo 100 IPI **tự gửi** qua SBI `send_ipi_raw` (vẫn là đường SBI/trap thật, log ghi `mode=self-ipi`) |
//! | `ok 21 - ipi drain khi cho ack` | 2 hart **tắt `sstatus.SIE`** (vùng chờ khoá), mỗi hart `post(TlbFlush)` rồi chờ ack bằng `wait_drain` (**drain trong vòng chờ** — D2-68): cả hai xong, `traps_delta = 0` (không nhờ ngắt), và một lần chờ predicate không bao giờ đúng **hết timeout** rồi đi tiếp (không treo) |
//! | `ok 22 - self-target khong gui` | mọi hart `post(chính mình, TlbFlush)`: `sent`/`traps` **không tăng**, `local` +1, `delivered` +1 (local flush + ack ngầm — D2-68) |
//! | `ok 23 - coalesce khong mat kind` | 5 × `TlbFlush` + 1 × `VmKick` tới peer: `coalesced=4`, chỉ **2** IPI (`sent=2`), hộp thư chờ 2 kind, một `drain` giao **đủ** `{TlbFlush, VmKick}` ⇒ gửi trùng **không mất kind** |
//! | `ok 24 - ipi khong nham timer/external` | trong cả pha: `TRAP_STATS.interrupts` tăng **đúng** bằng số trap IPI, `TIMER_STATS.ticks` **0**, `PLIC_STATS.uart_irqs` **0**, `last_scause = 0x8000_0000_0000_0001`, decode 1/5/9 ra `Ipi`/`Timer`/`Interrupt{9}` |
//!
//! Số đo do chính boot in ra (`# ipi:`, `# ipi delivered:`, `# ipi drain:`, `# ipi self:`,
//! `# ipi coalesce:`, `# ipi distinct:`) — `ktest.sh` grep đúng các dòng đó; tài liệu không chép
//! số tay (QD §9 Q12). Xem `v2/docs/k3-riscv64.md` §13.
//!
//! **Không** thuộc KT-50: ASID/`sfence.vma`/shootdown ack đầy đủ (KT-51), object layer (KT-52),
//! U-mode (KT-53), preemption/scheduler đa hart. Ở `-smp 4` chỉ **một** peer (hart online nhỏ
//! nhất khác BSP) làm phép đo chéo — đúng phạm vi "100 IPI 2 hart"; các hart còn lại tham gia
//! barrier + self-target, không tham gia phép đo chéo (log ghi `peer=…`).

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::ipi::{IpiKind, IPI_MAILBOXES, IPI_STATS};
use vifast_plat_common::smp::{mask_count, HartMask, MAX_HARTS};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::ipi::{self, PostOutcome};
use vifast_plat_riscv64::plic::PLIC_STATS;
use vifast_plat_riscv64::smp;
use vifast_plat_riscv64::timer::TIMER_STATS;
use vifast_plat_riscv64::trap::{
    decode_scause, Event, INTERRUPT_SUPERVISOR_EXTERNAL, INTERRUPT_SUPERVISOR_SOFTWARE,
    INTERRUPT_SUPERVISOR_TIMER, SCAUSE_INTERRUPT, TRAP_STATS,
};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

// ---------------------------------------------------------------------------------------------
// Vòng barrier của pha IPI (tiếp số 6 của KT-49)
// ---------------------------------------------------------------------------------------------

/// Chuẩn bị: bật `sie.SSIE` + `sstatus.SIE`, ghi bằng chứng CSR.
const ROUND_SETUP: u64 = 7;
/// 100 IPI chéo: peer chờ trap, BSP gửi tuần tự và đợi từng delivery.
const ROUND_TRAP: u64 = 8;
/// Chờ ack **có drain** khi `SIE = 0` (2 hart chờ nhau).
const ROUND_DRAIN: u64 = 9;
/// Peer báo kết quả pha drain (đến barrier **sau khi** đã lưu số).
const ROUND_DRAIN_RESULT: u64 = 10;
/// Self-target (mọi hart): `sent`/`traps` không tăng, `local` tăng.
const ROUND_SELF: u64 = 11;
/// Peer tắt `SIE` xong, sẵn sàng nhận cụm coalesce (BSP chỉ post **sau** arrival).
const ROUND_COALESCE_READY: u64 = 12;
/// Peer drain cụm coalesce xong, báo kết quả.
const ROUND_COALESCE_RESULT: u64 = 13;

/// Số IPI của phép đo "100 ipi delivered" (K3-PLAN §4.1 KT-50).
pub const IPI_TARGET: u64 = 100;
/// Timeout **mỗi** delivery khi BSP gửi 100 IPI (100 ms — dư cho QEMU icount chia lượt vCPU).
const IPI_DELIVERY_TIMEOUT: u64 = vifast_plat_riscv64::timer::TIMEBASE_HZ / 10;
/// Timeout của barrier `ROUND_TRAP` ở phía **AP** — pha 1 có thể kéo dài hơn barrier thường (1 s)
/// vì 100 delivery tuần tự; 10 s = trần 100 × [`IPI_DELIVERY_TIMEOUT`] để AP **không** rời pha giữa
/// chừng (rời sớm sẽ drain hộp thư ngoài phép đo ⇒ `delivered > traps`, sai ngữ nghĩa).
const IPI_TRAP_PHASE_TIMEOUT: u64 = 10 * vifast_plat_riscv64::timer::TIMEBASE_HZ;
/// Timeout của vòng chờ ack có drain (100 ms).
const IPI_DRAIN_TIMEOUT: u64 = vifast_plat_riscv64::timer::TIMEBASE_HZ / 10;
/// Timeout của phép đo "chờ predicate không bao giờ đúng" (50 ms) — phải **hết** rồi đi tiếp.
const IPI_TIMEOUT_PROBE: u64 = vifast_plat_riscv64::timer::TIMEBASE_HZ / 20;

// ---------------------------------------------------------------------------------------------
// Kết quả từng hart (AP ghi, BSP đọc sau barrier)
// ---------------------------------------------------------------------------------------------

/// `sie.SSIE` + `sstatus.SIE` bật được trên hart nào.
static SETUP_SIE_OK: HartMask = HartMask::new();
/// Kết quả pha drain: `ok`/`traps_delta`/`wait_ticks` từng hart.
static DRAIN_OK: HartMask = HartMask::new();
static DRAIN_TRAPS_DELTA: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static DRAIN_WAIT: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// Kết quả self-target từng hart.
static SELF_OK: HartMask = HartMask::new();
static SELF_SENT_DELTA: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static SELF_TRAPS_DELTA: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// Kết quả coalesce.
static COALESCE_OK: HartMask = HartMask::new();
static COALESCE_BITS: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static COALESCE_DELIVERED: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static COALESCE_SENT_DELTA: AtomicU64 = AtomicU64::new(0);
static COALESCE_COALESCED_DELTA: AtomicU64 = AtomicU64::new(0);
static COALESCE_PENDING: AtomicU64 = AtomicU64::new(0);

// Kết quả pha 1 (BSP đo) — 100 IPI.
static TRAP_SENT: AtomicU64 = AtomicU64::new(0);
static TRAP_COALESCED: AtomicU64 = AtomicU64::new(0);
static TRAP_DELIVERED: AtomicU64 = AtomicU64::new(0);
static TRAP_TRAPS: AtomicU64 = AtomicU64::new(0);
static TRAP_SPURIOUS: AtomicU64 = AtomicU64::new(0);
static TRAP_TIMEOUT: AtomicU64 = AtomicU64::new(0);
static TRAP_KINDS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static TRAP_ARRIVED: AtomicU64 = AtomicU64::new(0);
static TRAP_BARRIER_TIMEOUT: AtomicU64 = AtomicU64::new(0);
static TRAP_WAIT_TICKS: AtomicU64 = AtomicU64::new(0);

/// Kết quả "timeout phải hết".
static TIMEOUT_ENDED: AtomicU64 = AtomicU64::new(0);
static TIMEOUT_WAIT: AtomicU64 = AtomicU64::new(0);
static TIMEOUT_DRAINS: AtomicU64 = AtomicU64::new(0);

fn now_ticks() -> u64 {
    PLATFORM.now_ticks()
}

/// Spin tới khi `pred()` đúng hoặc quá `timeout_ticks` (timebase).
fn spin_until(timeout_ticks: u64, mut pred: impl FnMut() -> bool) -> bool {
    let start = now_ticks();
    loop {
        if pred() {
            return true;
        }
        if now_ticks().wrapping_sub(start) > timeout_ticks {
            return false;
        }
        core::hint::spin_loop();
    }
}

/// Hart online đầu tiên khác `boot` (peer của phép đo chéo) — `None` nếu không có.
fn find_peer(online: u64, boot: usize) -> Option<usize> {
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        if hart != boot && online & (1u64 << hart) != 0 {
            return Some(hart);
        }
        hart += 1;
    }
    None
}

/// Bật cổng IPI và ghi bằng chứng — chạy trên **mọi** hart.
fn enable_and_note(hart: usize) -> bool {
    let on = ipi::enable_ipi();
    if on {
        SETUP_SIE_OK.insert(hart);
    }
    on
}

/// BSP: chuẩn bị rồi một vòng barrier thường (`begin/join/wait/release`).
fn bsp_simple_round(round: u64, online: u64) -> (u64, bool) {
    let (arrived, timed_out, _) =
        crate::ktest_smp::bsp_barrier(round, online, smp::BARRIER_TIMEOUT_TICKS);
    (arrived, timed_out)
}

// ---------------------------------------------------------------------------------------------
// Pha 1 — 100 IPI delivered
// ---------------------------------------------------------------------------------------------

/// BSP: gửi `IPI_TARGET` IPI tới `peer`, **tuần tự** (đợi delivery trước khi gửi tiếp để không
/// coalesce nhầm) với kind luân phiên `TlbFlush/VmKick/Wake` ⇒ per-kind `34/33/33`.
fn bsp_trap_phase(peer: usize) {
    let phase_start = now_ticks();
    let before_sent = IPI_STATS.get(PLATFORM.hart_id()).snapshot().sent;
    let before_peer = IPI_STATS.get(peer).snapshot();
    let mut coalesced = 0u64;
    let mut timed_out = false;
    let mut index = 0u64;
    while index < IPI_TARGET {
        let kind = IpiKind::ALL[(index as usize) % IpiKind::ALL.len()];
        let outcome = ipi::post(peer, kind);
        match outcome {
            PostOutcome::Remote => {}
            PostOutcome::Coalesced => coalesced += 1,
            // Self-target không xảy ra (peer != boot); `Invalid` là lỗi thật ⇒ TAP đỏ.
            _ => {
                timed_out = true;
                break;
            }
        }
        let target = before_peer.delivered + index + 1;
        if !spin_until(IPI_DELIVERY_TIMEOUT, || {
            IPI_STATS.get(peer).snapshot().delivered >= target
        }) {
            timed_out = true;
            break;
        }
        index += 1;
    }
    let after_peer = IPI_STATS.get(peer).snapshot();
    TRAP_SENT.store(
        IPI_STATS.get(PLATFORM.hart_id()).snapshot().sent - before_sent,
        Ordering::SeqCst,
    );
    TRAP_COALESCED.store(coalesced, Ordering::SeqCst);
    TRAP_DELIVERED.store(
        after_peer.delivered - before_peer.delivered,
        Ordering::SeqCst,
    );
    TRAP_TRAPS.store(after_peer.traps - before_peer.traps, Ordering::SeqCst);
    TRAP_SPURIOUS.store(after_peer.spurious, Ordering::SeqCst);
    TRAP_TIMEOUT.store(timed_out as u64, Ordering::SeqCst);
    for (slot, kind) in TRAP_KINDS.iter().zip(IpiKind::ALL) {
        slot.store(after_peer.kinds[kind.index()], Ordering::SeqCst);
    }
    TRAP_WAIT_TICKS.store(now_ticks().wrapping_sub(phase_start), Ordering::SeqCst);
}

/// BSP ở `-smp 1`: 100 IPI **tự gửi** qua SBI (`send_ipi_raw`) — đo đúng đường SBI IPI + trap
/// `scause=(1<<63)|1` + drain khi **không có** hart đích. Log ghi `mode=self-ipi`, không giả 2 hart.
fn bsp_self_ipi_phase() {
    let hart = PLATFORM.hart_id();
    let before = IPI_STATS.get(hart).snapshot();
    let mut ok = true;
    for index in 0..IPI_TARGET {
        if ipi::send_ipi_raw(1u64 << hart) != 0 {
            ok = false;
            break;
        }
        let target = before.traps + index + 1;
        if !spin_until(IPI_DELIVERY_TIMEOUT, || {
            IPI_STATS.get(hart).snapshot().traps >= target
        }) {
            ok = false;
            break;
        }
    }
    let after = IPI_STATS.get(hart).snapshot();
    TRAP_SENT.store(after.sent - before.sent, Ordering::SeqCst);
    TRAP_COALESCED.store(after.coalesced - before.coalesced, Ordering::SeqCst);
    TRAP_DELIVERED.store(after.delivered - before.delivered, Ordering::SeqCst);
    TRAP_TRAPS.store(after.traps - before.traps, Ordering::SeqCst);
    TRAP_SPURIOUS.store(after.spurious, Ordering::SeqCst);
    TRAP_TIMEOUT.store((!ok) as u64, Ordering::SeqCst);
    for (slot, kind) in TRAP_KINDS.iter().zip(IpiKind::ALL) {
        slot.store(after.kinds[kind.index()], Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------------------------
// Pha 2 — drain khi chờ ack (SIE = 0)
// ---------------------------------------------------------------------------------------------

/// Một hart: tắt `SIE`, post `TlbFlush` cho `target`, rồi **chờ bằng `wait_drain`** (drain trong
/// vòng chờ — D2-68) tới khi hộp thư của chính mình giao thêm ≥ 1 kind; ghi kết quả.
///
/// `traps_delta` phải **0**: kind được giao bởi **drain trong vòng chờ**, không nhờ trap (`SIE`
/// tắt thì trap không vào được — đây đúng là kịch bản deadlock của D2-68 nếu thiếu drain).
fn drain_side(hart: usize, target: usize) -> bool {
    ipi::set_global_irq(false); // vùng "đang giữ khoá": trap IPI không vào
    let before = IPI_STATS.get(hart).snapshot();
    let start = now_ticks();
    let posted = ipi::post(target, IpiKind::TlbFlush) == PostOutcome::Remote;
    let target_delivered = before.delivered + 1;
    let drained = ipi::wait_drain(IPI_DRAIN_TIMEOUT, || {
        IPI_STATS.get(hart).snapshot().delivered >= target_delivered
    });
    let waited = now_ticks().wrapping_sub(start);
    let after = IPI_STATS.get(hart).snapshot();
    ipi::set_global_irq(true);
    let traps_delta = after.traps - before.traps;
    let ok = posted && drained && traps_delta == 0;
    DRAIN_OK.set(hart, ok);
    DRAIN_TRAPS_DELTA[hart].store(traps_delta, Ordering::SeqCst);
    DRAIN_WAIT[hart].store(waited, Ordering::SeqCst);
    ok
}

/// Phép đo "timeout phải hết": chờ predicate **không bao giờ đúng** với timeout 50 ms; phải trả
/// `false` sau ≥ timeout, không treo. Đồng thời kiểm vòng chờ **có drain** (số `drain` của hộp
/// thư tăng) — chứng minh không phải "ngủ đông chờ ngắt".
fn timeout_probe(hart: usize) {
    ipi::set_global_irq(false);
    let drains_before = IPI_MAILBOXES.get(hart).snapshot().drains;
    let start = now_ticks();
    let ready = ipi::wait_drain(IPI_TIMEOUT_PROBE, || false);
    let waited = now_ticks().wrapping_sub(start);
    let drains_after = IPI_MAILBOXES.get(hart).snapshot().drains;
    ipi::set_global_irq(true);
    let ended = !ready && waited >= IPI_TIMEOUT_PROBE;
    TIMEOUT_ENDED.store(ended as u64, Ordering::SeqCst);
    TIMEOUT_WAIT.store(waited, Ordering::SeqCst);
    TIMEOUT_DRAINS.store(drains_after - drains_before, Ordering::SeqCst);
}

// ---------------------------------------------------------------------------------------------
// Pha 3 — self-target
// ---------------------------------------------------------------------------------------------

/// Mỗi hart tự gửi `TlbFlush` cho chính mình: đường **local**, `sent`/`traps` không tăng.
fn self_target_side(hart: usize) -> bool {
    let before = IPI_STATS.get(hart).snapshot();
    let outcome = ipi::post(hart, IpiKind::TlbFlush);
    let after = IPI_STATS.get(hart).snapshot();
    let kind_index = IpiKind::TlbFlush.index();
    let ok = outcome == PostOutcome::Local
        && after.sent == before.sent
        && after.traps == before.traps
        && after.local == before.local + 1
        && after.delivered == before.delivered + 1
        && after.kinds[kind_index] == before.kinds[kind_index] + 1;
    SELF_SENT_DELTA[hart].store(after.sent - before.sent, Ordering::SeqCst);
    SELF_TRAPS_DELTA[hart].store(after.traps - before.traps, Ordering::SeqCst);
    SELF_OK.set(hart, ok);
    ok
}

// ---------------------------------------------------------------------------------------------
// Pha 4 — coalesce
// ---------------------------------------------------------------------------------------------

/// BSP post cụm coalesce tới `peer` (5 × `TlbFlush` + 1 × `VmKick`) rồi kiểm **trước khi** peer
/// drain: chỉ 2 IPI, `coalesced = 4`, hộp thư đích chờ đúng 2 kind.
fn coalesce_bsp(peer: usize) -> bool {
    let before = IPI_STATS.get(PLATFORM.hart_id()).snapshot();
    let mut remote = 0u64;
    for _ in 0..5 {
        if ipi::post(peer, IpiKind::TlbFlush) == PostOutcome::Remote {
            remote += 1;
        }
    }
    for _ in 0..1 {
        if ipi::post(peer, IpiKind::VmKick) == PostOutcome::Remote {
            remote += 1;
        }
    }
    let after = IPI_STATS.get(PLATFORM.hart_id()).snapshot();
    let pending = IPI_MAILBOXES.get(peer).pending();
    let want = IpiKind::TlbFlush.bit() | IpiKind::VmKick.bit();
    let sent_delta = after.sent - before.sent;
    let coalesced_delta = after.coalesced - before.coalesced;
    COALESCE_SENT_DELTA.store(sent_delta, Ordering::SeqCst);
    COALESCE_COALESCED_DELTA.store(coalesced_delta, Ordering::SeqCst);
    COALESCE_PENDING.store(pending.bits(), Ordering::SeqCst);
    remote == 2 && sent_delta == 2 && coalesced_delta == 4 && pending.bits() == want
}

/// BSP ở `-smp 1`: kiểm coalesce **trên hộp thư tại chỗ** (không có hart đích để gửi IPI) rồi
/// drain bằng `drain_current`; bất biến yêu cầu giống hệt đường chéo.
fn coalesce_local(boot: usize) -> bool {
    let mailbox = IPI_MAILBOXES.get(boot);
    let before = mailbox.snapshot();
    let mut fresh = 0u64;
    for _ in 0..5 {
        if mailbox.post(IpiKind::TlbFlush) {
            fresh += 1;
        }
    }
    fresh += mailbox.post(IpiKind::VmKick) as u64;
    let pending = mailbox.pending();
    let want = IpiKind::TlbFlush.bit() | IpiKind::VmKick.bit();
    let drained = ipi::drain_current();
    let after = mailbox.snapshot();
    let coalesced_delta = after.coalesced - before.coalesced;
    COALESCE_SENT_DELTA.store(0, Ordering::SeqCst);
    COALESCE_COALESCED_DELTA.store(coalesced_delta, Ordering::SeqCst);
    COALESCE_PENDING.store(pending.bits(), Ordering::SeqCst);
    COALESCE_BITS[boot].store(drained.bits(), Ordering::SeqCst);
    COALESCE_DELIVERED[boot].store(drained.count() as u64, Ordering::SeqCst);
    let ok = fresh == 2 && coalesced_delta == 4 && pending.bits() == want && drained.bits() == want;
    if ok {
        COALESCE_OK.insert(boot);
    }
    ok
}

// ---------------------------------------------------------------------------------------------
// Entry: AP
// ---------------------------------------------------------------------------------------------

/// Entry hart **thứ cấp** sau pha SMP — tham gia pha IPI rồi park. Mọi bước đều có timeout để
/// hart lỗi **không treo** cả máy (TAP đỏ thay vì treo).
pub fn secondary_phase(hart: usize) -> ! {
    let online = smp::online_mask();
    let boot = smp::boot_hart();
    let is_peer = find_peer(online, boot) == Some(hart);
    if !enable_and_note(hart) {
        park();
    } // vào pha IPI
      // R7: chuẩn bị xong.
    if !crate::ktest_smp::ap_barrier(ROUND_SETUP, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    } // xong R7
      // R8: với peer, `barrier_wait_release` là vùng spin có `SIE = 1` để IPI trap của BSP drain.
    if !crate::ktest_smp::ap_barrier(ROUND_TRAP, IPI_TRAP_PHASE_TIMEOUT) {
        // Rời pha giữa chừng sẽ drain ngoài phép đo ⇒ park để TAP đỏ rõ ràng, không làm bẩn số.
        park();
    } // xong R8
      // R9: peer `join` (báo SIE đã tắt) → post + **chờ ack bằng drain** → `wait_release`. Việc nằm
      // GIỮA join và wait_release: BSP chỉ post sau khi thấy arrival, nên kind nằm trong hộp thư tới
      // khi vòng chờ của peer drain (nếu làm sau `wait_release` thì hai bên chờ nhau vô ích ⇒ timeout). // chờ R9
      // Tắt `sstatus.SIE` **trước** khi join R9: BSP chỉ post sau khi thấy arrival, nên post của
      // BSP không thể trap trên hart này (nếu trap, kind bị drain ngoài vòng chờ ⇒ phép đo vô nghĩa).
    ipi::set_global_irq(false);
    if !smp::barrier_join(ROUND_DRAIN, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    } // đã join R9
    if is_peer {
        let _ = drain_side(hart, boot);
    } // xong drain
    if !smp::barrier_wait_release(ROUND_DRAIN, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    }
    if !crate::ktest_smp::ap_barrier(ROUND_DRAIN_RESULT, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    }
    // R11: self-target trên **mọi** hart, đến barrier sau khi lưu kết quả.
    let _ = self_target_side(hart);
    if !crate::ktest_smp::ap_barrier(ROUND_SELF, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    }
    // R12: tắt SIE và báo sẵn sàng; BSP chỉ post sau arrival (không trap nào chen vào).
    ipi::set_global_irq(false);
    if !crate::ktest_smp::ap_barrier(ROUND_COALESCE_READY, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    }
    if is_peer {
        // R13: drain cụm coalesce bằng đường thật (không nhờ trap), lưu kết quả, báo BSP.
        let kinds = ipi::drain_current();
        let want = IpiKind::TlbFlush.bit() | IpiKind::VmKick.bit();
        COALESCE_BITS[hart].store(kinds.bits(), Ordering::SeqCst);
        COALESCE_DELIVERED[hart].store(kinds.count() as u64, Ordering::SeqCst);
        if kinds.bits() == want && kinds.count() == 2 {
            COALESCE_OK.insert(hart);
        }
    }
    ipi::set_global_irq(true);
    if !crate::ktest_smp::ap_barrier(ROUND_COALESCE_RESULT, smp::BARRIER_TIMEOUT_TICKS) {
        park();
    }
    // KT-51: pha TLB/ASID/shootdown (TAP 25..29) — AP tham gia barrier + là hart đích thật.
    crate::ktest_tlb::secondary_phase(hart)
}

fn park() -> ! {
    vifast_plat_riscv64::park_forever()
}

// ---------------------------------------------------------------------------------------------
// Entry: BSP
// ---------------------------------------------------------------------------------------------

/// Pha IPI trên **BSP**: TAP 20..24. Trả số fail (0 = xanh).
pub fn ipi_phase() -> usize {
    let mut fails = 0usize;
    let online = smp::online_mask();
    let boot = PLATFORM.hart_id();
    let n = mask_count(online) as usize;
    let peer = find_peer(online, boot);

    // Snapshot đầu pha cho TAP 24 (không nhầm timer/external).
    let traps_before = TRAP_STATS.snapshot();
    let ipi_traps_before = IPI_STATS.total_traps();
    let mut timer_before = [0u64; MAX_HARTS];
    let mut plic_before = [0u64; MAX_HARTS];
    for hart in 0..MAX_HARTS {
        timer_before[hart] = TIMER_STATS.get(hart).snapshot().ticks;
        plic_before[hart] = PLIC_STATS.get(hart).snapshot().uart_irqs;
    }

    // --- R7: setup (SIE, SSIE) trên mọi hart.
    let setup_ok = enable_and_note(boot);
    let (setup_arrived, setup_timeout) = bsp_simple_round(ROUND_SETUP, online);
    let setup_ok = setup_ok
        && !setup_timeout
        && setup_arrived & online == online
        && SETUP_SIE_OK.bits() & online == online;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipi: harts={} boot={} peer={} mode={} target={} sie_ok=0x{:x} setup_ok={}",
            n,
            boot,
            peer.map(|h| h as i64).unwrap_or(-1),
            if n >= 2 { "cross" } else { "self-ipi" },
            IPI_TARGET,
            SETUP_SIE_OK.bits() & online,
            setup_ok as u8
        );
    }

    // --- R8: 100 IPI (manual: post nằm GIỮA wait_arrivals và release).
    smp::barrier_begin(ROUND_TRAP, online);
    let _ = smp::barrier_join(ROUND_TRAP, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived8, timeout8) = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    if let Some(peer_hart) = peer {
        bsp_trap_phase(peer_hart);
    } else {
        bsp_self_ipi_phase();
    }
    smp::barrier_release(ROUND_TRAP);
    TRAP_ARRIVED.store(arrived8, Ordering::SeqCst);
    TRAP_BARRIER_TIMEOUT.store(timeout8 as u64, Ordering::SeqCst);

    // --- R9: drain khi chờ ack (SIE = 0 hai phía). BSP tắt SIE TRƯỚC khi publish round để IPI
    // của peer không trap vào BSP (nếu trap, kind bị drain ngoài vòng chờ ⇒ phép đo vô nghĩa).
    ipi::set_global_irq(false);
    smp::barrier_begin(ROUND_DRAIN, online);
    let _ = smp::barrier_join(ROUND_DRAIN, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived9, timeout9) = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    let mut drain_pair_ok = true;
    if let Some(peer_hart) = peer {
        drain_pair_ok = drain_side(boot, peer_hart);
    }
    smp::barrier_release(ROUND_DRAIN);
    ipi::set_global_irq(true);
    let _ = (arrived9, timeout9);
    // Phép đo timeout: predicate không bao giờ đúng ⇒ phải **hết** 50 ms rồi đi tiếp.
    timeout_probe(boot);

    // --- R10: peer báo kết quả drain.
    let (arrived10, timeout10) = bsp_simple_round(ROUND_DRAIN_RESULT, online);
    let mut drain_ok = drain_pair_ok && !timeout10 && arrived10 & online == online;
    if let Some(peer_hart) = peer {
        drain_ok = drain_ok
            && DRAIN_OK.contains(boot)
            && DRAIN_OK.contains(peer_hart)
            && DRAIN_TRAPS_DELTA[boot].load(Ordering::SeqCst) == 0
            && DRAIN_TRAPS_DELTA[peer_hart].load(Ordering::SeqCst) == 0;
    }
    let timeout_ok = TIMEOUT_ENDED.load(Ordering::SeqCst) == 1;

    // --- R11: self-target trên mọi hart (đến barrier sau khi lưu kết quả).
    let _ = self_target_side(boot);
    let (arrived11, timeout11) = bsp_simple_round(ROUND_SELF, online);
    let self_ok = !timeout11
        && arrived11 & online == online
        && SELF_OK.bits() & online == online
        && (0..MAX_HARTS).all(|hart| {
            online & (1u64 << hart) == 0
                || (SELF_SENT_DELTA[hart].load(Ordering::SeqCst) == 0
                    && SELF_TRAPS_DELTA[hart].load(Ordering::SeqCst) == 0)
        });

    // --- R12: peer tắt SIE xong; BSP post cụm coalesce (chỉ **sau** arrival).
    smp::barrier_begin(ROUND_COALESCE_READY, online);
    let _ = smp::barrier_join(ROUND_COALESCE_READY, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived12, timeout12) = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    let coalesce_post_ok = if let Some(peer_hart) = peer {
        coalesce_bsp(peer_hart)
    } else {
        coalesce_local(boot)
    };
    smp::barrier_release(ROUND_COALESCE_READY);

    // --- R13: peer drain xong, báo kết quả.
    let (arrived13, timeout13) = bsp_simple_round(ROUND_COALESCE_RESULT, online);
    let want = IpiKind::TlbFlush.bit() | IpiKind::VmKick.bit();
    let mut coalesce_ok = coalesce_post_ok
        && !timeout12
        && !timeout13
        && arrived12 & online == online
        && arrived13 & online == online
        && COALESCE_PENDING.load(Ordering::SeqCst) == want
        && COALESCE_COALESCED_DELTA.load(Ordering::SeqCst) == 4;
    if let Some(peer_hart) = peer {
        coalesce_ok = coalesce_ok
            && COALESCE_OK.contains(peer_hart)
            && COALESCE_BITS[peer_hart].load(Ordering::SeqCst) == want
            && COALESCE_DELIVERED[peer_hart].load(Ordering::SeqCst) == 2
            && COALESCE_SENT_DELTA.load(Ordering::SeqCst) == 2;
    } else {
        coalesce_ok = coalesce_ok
            && COALESCE_OK.contains(boot)
            && COALESCE_BITS[boot].load(Ordering::SeqCst) == want
            && COALESCE_DELIVERED[boot].load(Ordering::SeqCst) == 2;
    }

    // --- TAP 20: 100 IPI (không mất/không nhân đôi).
    let trap_ok = TRAP_BARRIER_TIMEOUT.load(Ordering::SeqCst) == 0
        && TRAP_ARRIVED.load(Ordering::SeqCst) & online == online
        && TRAP_TIMEOUT.load(Ordering::SeqCst) == 0
        && TRAP_COALESCED.load(Ordering::SeqCst) == 0
        && TRAP_SPURIOUS.load(Ordering::SeqCst) == 0
        && TRAP_SENT.load(Ordering::SeqCst) == IPI_TARGET
        && TRAP_TRAPS.load(Ordering::SeqCst) == IPI_TARGET
        && if peer.is_some() {
            TRAP_DELIVERED.load(Ordering::SeqCst) == IPI_TARGET
                && TRAP_KINDS[0].load(Ordering::SeqCst) == 34
                && TRAP_KINDS[1].load(Ordering::SeqCst) == 33
                && TRAP_KINDS[2].load(Ordering::SeqCst) == 33
        } else {
            TRAP_DELIVERED.load(Ordering::SeqCst) == 0
        };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipi delivered: mode={} target={} sent={} delivered={} traps={} coalesced={} spurious={} kinds={}/{}/{} timeout={}",
            if peer.is_some() { "cross" } else { "self-ipi" },
            IPI_TARGET,
            TRAP_SENT.load(Ordering::SeqCst),
            TRAP_DELIVERED.load(Ordering::SeqCst),
            TRAP_TRAPS.load(Ordering::SeqCst),
            TRAP_COALESCED.load(Ordering::SeqCst),
            TRAP_SPURIOUS.load(Ordering::SeqCst),
            TRAP_KINDS[0].load(Ordering::SeqCst),
            TRAP_KINDS[1].load(Ordering::SeqCst),
            TRAP_KINDS[2].load(Ordering::SeqCst),
            TRAP_TIMEOUT.load(Ordering::SeqCst)
        );
        let _ = writeln!(
            w,
            "# ipi phase1 wait_ticks={}",
            TRAP_WAIT_TICKS.load(Ordering::SeqCst)
        );
    }
    if trap_ok {
        // D5: `-smp 1` đo **IPI tự gửi** (100 trap, `delivered=0`) nên tên TAP phải nói rõ
        // `traps (self)` — không để đọc thành "100 kind được giao chéo" trên máy một hart.
        if peer.is_some() {
            PLATFORM.put_str("ok 20 - 100 ipi delivered\n");
        } else {
            PLATFORM.put_str("ok 20 - 100 ipi traps (self)\n");
        }
    } else {
        PLATFORM.put_str("not ok 20 - 100 ipi delivered/traps: mat/nhan doi/khong dung kind\n");
        fails += 1;
    }

    // --- TAP 21: drain khi chờ ack + timeout phải hết.
    let drain_tap_ok = if peer.is_some() {
        drain_ok && timeout_ok
    } else {
        // Không có peer: vẫn chứng minh **drain trong vòng chờ** (số drain tăng) + timeout hết.
        timeout_ok && TIMEOUT_DRAINS.load(Ordering::SeqCst) > 0
    };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipi drain: mode={} mutual={} boot_ok={} peer_ok={} traps_delta={}/{} wait_ticks={}/{} timeout_ended={} timeout_wait={} timeout_drains={}",
            if peer.is_some() { "cross" } else { "self-wait" },
            peer.is_some() as u8,
            DRAIN_OK.contains(boot) as u8,
            peer.map(|h| DRAIN_OK.contains(h) as u8).unwrap_or(0),
            DRAIN_TRAPS_DELTA[boot].load(Ordering::SeqCst),
            peer.map(|h| DRAIN_TRAPS_DELTA[h].load(Ordering::SeqCst))
                .unwrap_or(0),
            DRAIN_WAIT[boot].load(Ordering::SeqCst),
            peer.map(|h| DRAIN_WAIT[h].load(Ordering::SeqCst)).unwrap_or(0),
            TIMEOUT_ENDED.load(Ordering::SeqCst),
            TIMEOUT_WAIT.load(Ordering::SeqCst),
            TIMEOUT_DRAINS.load(Ordering::SeqCst)
        );
    }
    if drain_tap_ok {
        PLATFORM.put_str("ok 21 - ipi drain khi cho ack\n");
    } else {
        PLATFORM.put_str("not ok 21 - ipi drain khi cho ack: drain khong tien/timeout khong het\n");
        fails += 1;
    }

    // --- TAP 22: self-target không gửi IPI.
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipi self: ok=0x{:x} local={} sent_delta={} traps_delta={} kinds_tlb={}",
            SELF_OK.bits() & online,
            IPI_STATS.get(boot).snapshot().local,
            SELF_SENT_DELTA[boot].load(Ordering::SeqCst),
            SELF_TRAPS_DELTA[boot].load(Ordering::SeqCst),
            IPI_STATS.get(boot).snapshot().kinds[IpiKind::TlbFlush.index()]
        );
    }
    if self_ok {
        PLATFORM.put_str("ok 22 - self-target khong gui\n");
    } else {
        PLATFORM.put_str("not ok 22 - self-target khong gui: co IPI cho chinh minh\n");
        fails += 1;
    }

    // --- TAP 23: coalesce không mất kind.
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipi coalesce: mode={} posts=6 coalesced={} sent_delta={} pending=0x{:x} peer_bits=0x{:x} peer_delivered={} ok={}",
            if peer.is_some() { "cross" } else { "mailbox" },
            COALESCE_COALESCED_DELTA.load(Ordering::SeqCst),
            COALESCE_SENT_DELTA.load(Ordering::SeqCst),
            COALESCE_PENDING.load(Ordering::SeqCst),
            peer.map(|h| COALESCE_BITS[h].load(Ordering::SeqCst))
                .unwrap_or(COALESCE_BITS[boot].load(Ordering::SeqCst)),
            peer.map(|h| COALESCE_DELIVERED[h].load(Ordering::SeqCst))
                .unwrap_or(COALESCE_DELIVERED[boot].load(Ordering::SeqCst)),
            coalesce_ok as u8
        );
    }
    if coalesce_ok {
        PLATFORM.put_str("ok 23 - coalesce khong mat kind\n");
    } else {
        PLATFORM.put_str("not ok 23 - coalesce khong mat kind: gop lam mat/thua kind\n");
        fails += 1;
    }

    // --- TAP 24: không nhầm timer/external.
    let traps_after = TRAP_STATS.snapshot();
    let ipi_traps = IPI_STATS.total_traps() - ipi_traps_before;
    let mut timer_delta = 0u64;
    let mut plic_delta = 0u64;
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) != 0 {
            timer_delta += TIMER_STATS.get(hart).snapshot().ticks - timer_before[hart];
            plic_delta += PLIC_STATS.get(hart).snapshot().uart_irqs - plic_before[hart];
        }
    }
    let interrupts_delta = traps_after.interrupts - traps_before.interrupts;
    let last_ipi = traps_after.last_scause == (SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_SOFTWARE)
        && traps_after.last_code == INTERRUPT_SUPERVISOR_SOFTWARE;
    let decode_ok = decode_scause(SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_SOFTWARE) == Event::Ipi
        && decode_scause(SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_TIMER) == Event::Timer
        && decode_scause(SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_EXTERNAL)
            == Event::Interrupt {
                code: INTERRUPT_SUPERVISOR_EXTERNAL,
            };
    let distinct_ok = interrupts_delta == ipi_traps
        && timer_delta == 0
        && plic_delta == 0
        && last_ipi
        && decode_ok;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipi distinct: ipi_traps={} interrupts_delta={} timer_delta={} plic_delta={} last_scause=0x{:x} last_code={} decode_ok={} ok={}",
            ipi_traps,
            interrupts_delta,
            timer_delta,
            plic_delta,
            traps_after.last_scause,
            traps_after.last_code,
            decode_ok as u8,
            distinct_ok as u8
        );
    }
    if distinct_ok {
        PLATFORM.put_str("ok 24 - ipi khong nham timer/external\n");
    } else {
        PLATFORM.put_str("not ok 24 - ipi khong nham timer/external: lan ngat khac loai\n");
        fails += 1;
    }

    fails
}
