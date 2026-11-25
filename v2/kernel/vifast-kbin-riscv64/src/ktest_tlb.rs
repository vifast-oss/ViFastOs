// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **TLB/ASID/shootdown** của ảnh test `vifast-kbin-ktest` (KT-51).
//!
//! Nối sau pha IPI KT-50: `ktest_ipi::secondary_phase` gọi [`secondary_phase`] trên hart thứ cấp,
//! `ktest.rs` gọi [`tlb_phase`] trên BSP sau `ipi_phase()`. Nội dung TAP 25..29 (trap lồng dời sang
//! TAP 30):
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 25 - 100 vong shootdown Ok` | 100 vòng shootdown **thật** giữa 2 hart (`-smp ≥ 2`) hoặc self (`-smp 1`, log `mode=self`): mỗi vòng remap leaf 2 MiB `PROBE_VA → PA(r)` + ghi marker mới, `TlbFlush{asid, Page{va}}` qua IPI KT-50, chờ `ack_mask == active_on`; **không self-target** (`sent = targets−1`, slot yêu cầu của hart gọi luôn 0); hart đích **đọc lại VA** sau flush phải thấy marker mới (bằng chứng dương: thiếu flush ⇒ đọc ra marker cũ) |
//! | `ok 26 - stale mapping caught` | **dương tính thật**: map→PA_A, đọc (nạp TLB), remap→PA_B (marker B) **không** `sfence` ⇒ đọc **phải** ra marker A (TLB đang cache bản cũ); sau `sfence.vma` đúng dạng (`(asid,va)`/`(asid,*)`/`(*,*)`) ⇒ ra marker B. Lặp cho **cả ba** dạng |
//! | `ok 27 - hart ngoai active_on` | (a) hart gọi **ngoài** `active_on` vẫn Ok (`ack_mask == active_on` chỉ gồm hart đích); (b) hart **online nhưng ngoài** `active_on` **không** bị chờ và **không** ack (`acked` của nó không tăng) — `-smp 4` có hart thật như vậy |
//! | `ok 28 - rollover flush` | cấp cạn **256** ASID ⇒ `generation += 1` + **một** `FlushScope::All`: TLB entry cũ của ASID **0** (được phát lại — đúng nguy cơ tái dùng) bị xả; đọc lại VA ra marker mới trên **mọi** hart; `satp_asid` mọi hart = ASID mới |
//! | `ok 29 - timeout khong treo` | shootdown với hart **offline** trong `active_on` ⇒ `Timeout` sau ≥ deadline (`rdtime`), `ack_mask ≠ active_on`, **không treo**; sau đó shootdown bình thường vẫn `Ok` (máy còn dùng được); yêu cầu mồ côi bị xoá |
//!
//! Số đo do chính boot in (`# tlb:`, `# shootdown rounds:`, `# shootdown peer:`, `# stale:`,
//! `# outside:`, `# rollover:`, `# timeout:`, `# tlb asid:`) — `ktest.sh` grep đúng các dòng đó;
//! tài liệu không chép số tay (QD §9 Q12). Xem `v2/docs/k3-riscv64.md` §14.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::smp::{hart_bit, mask_count, HartMask, MAX_HARTS};
use vifast_plat_common::Platform;
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::tlb::{unused_online_harts, FlushScope, TLB_STATS};

use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::smp;
use vifast_plat_riscv64::tlb::{self, ShootdownOutcome};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

// ---------------------------------------------------------------------------------------------
// Hằng số pha đo
// ---------------------------------------------------------------------------------------------

/// Số vòng shootdown của TAP 25 (K3-PLAN §4.1 KT-51: "100 vòng 2 hart `Ok`").
pub const ROUNDS: u64 = 100;
/// Vòng barrier đầu của 100 vòng (`ROUND_BASE + r`, `r = 1..=ROUNDS`).
const ROUND_BASE: u64 = 20;
/// Barrier chốt sau 100 vòng (AP báo đã verify xong vòng cuối).
const ROUND_ROUNDS_DONE: u64 = 129;
/// TAP 27 (a): hart gọi **ngoài** `active_on`.
const ROUND_OUTSIDE_JOIN: u64 = 130;
const ROUND_OUTSIDE_DONE: u64 = 131;
/// TAP 27 (b): hart online **ngoài** `active_on`.
const ROUND_OUTSIDE2_JOIN: u64 = 132;
const ROUND_OUTSIDE2_DONE: u64 = 133;
/// TAP 28: rollover (cạn ASID) — mọi hart online tham gia.
const ROUND_ROLLOVER_JOIN: u64 = 134;
const ROUND_ROLLOVER_DONE: u64 = 135;
/// Barrier cuối: BSP cho AP park.
const ROUND_DONE: u64 = 140;

/// Timeout AP chờ BSP công bố barrier (timebase; 5 s **ảo** — pha đo dài nhưng không treo).
const JOIN_TIMEOUT: u64 = 5 * vifast_plat_riscv64::timer::TIMEBASE_HZ;
/// Timeout một vòng shootdown/chờ ack (2 s ảo).
const ROUND_TIMEOUT: u64 = 2 * vifast_plat_riscv64::timer::TIMEBASE_HZ;
/// Timeout AP phục vụ một yêu cầu (2 s ảo).
const SERVICE_TIMEOUT: u64 = 2 * vifast_plat_riscv64::timer::TIMEBASE_HZ;
/// Timeout của phép đo "timeout phải hết" (100 ms = 1.000.000 tick timebase).
const TIMEOUT_PROBE: u64 = vifast_plat_riscv64::timer::TIMEBASE_HZ / 10;

/// Nền marker (phân biệt với mọi dữ liệu khác trong RAM): `0x4b51_0000_0000_0000 | (round/loại)`.
const MARKER_BASE: u64 = 0x4b51_0000_0000_0000;
/// Marker của vòng `round` (0 = trạng thái đầu pha).
const fn marker(round: u64) -> u64 {
    MARKER_BASE | (round & 0x00ff_ffff)
}
/// Miền marker của phép đo stale (TAP 26).
const STALE_A_BASE: u64 = 0x4b51_a000_0000_0000;
const STALE_B_BASE: u64 = 0x4b51_b000_0000_0000;
/// Miền marker của TAP 27/28 (probe #2).
const SYNC_MARKER: u64 = 0x4b51_c000_0000_0000;

// ---------------------------------------------------------------------------------------------
// Kết quả AP ghi, BSP đọc (sau barrier)
// ---------------------------------------------------------------------------------------------

/// `EXPECT` do BSP công bố **trước** `barrier_release`; AP đọc sau release để biết marker mới.
static EXPECT: AtomicU64 = AtomicU64::new(0);
/// Số lần AP (peer) đọc prime **trước** remap mà không thấy marker cũ — lỗi đo.
static PEER_PRIME_WRONG: AtomicU64 = AtomicU64::new(0);
/// Số lần AP (peer) đọc **sau** flush không thấy marker mới — lỗi đo (mutation bỏ flush).
static PEER_VERIFY_WRONG: AtomicU64 = AtomicU64::new(0);
/// Số lần AP (peer) hết timeout khi chờ ack.
static PEER_SERVICE_TIMEOUT: AtomicU64 = AtomicU64::new(0);
/// Số lần AP verify hỏng ở TAP 27 (hart ngoài `active_on`).
static OUTSIDE_WRONG: AtomicU64 = AtomicU64::new(0);
/// Số lần AP verify hỏng ở TAP 28 (rollover).
static ROLLOVER_WRONG: AtomicU64 = AtomicU64::new(0);
/// Mask hart **phải phục vụ** vòng barrier kế tiếp (D2-78: TAP 27 gỡ hart khỏi `ACTIVE_ON` thật,
/// nên AP không thể đoán bằng `active_on` tại chỗ — BSP công bố mask dịch vụ trước `release`).
static SERVICE_MASK: AtomicU64 = AtomicU64::new(0);
/// Số lần AP hết timeout khi phục vụ một yêu cầu (mọi AP, mọi vòng).
static AP_SERVICE_BAD: AtomicU64 = AtomicU64::new(0);
/// Bitmask hart có `satp.ASID` đúng lease kernel **sau** rollover.
static ASID_OK: HartMask = HartMask::new();
/// Bitmask hart đã hoàn thành pha TLB (tham gia đủ barrier) — chẩn đoán.
static PHASE_OK: HartMask = HartMask::new();

fn now_ticks() -> u64 {
    PLATFORM.now_ticks()
}

/// PA đang được map tại `PROBE_VA` (walk phần mềm), `None` nếu lạ.
fn current_probe_pa() -> Option<u64> {
    mmu::probe_leaf_phys()
}

/// PA đang được map tại `PROBE2_VA` (walk phần mềm).
fn current_probe2_pa() -> Option<u64> {
    mmu::probe2_leaf_phys()
}

/// PA còn lại của cặp C/D.
fn other_probe2_pa(current: u64) -> u64 {
    if current == mmu::PROBE2_PA_C {
        mmu::PROBE2_PA_D
    } else {
        mmu::PROBE2_PA_C
    }
}

/// Hart online đầu tiên khác `boot` (peer của phép đo chéo) — `None` nếu chỉ một hart.
fn find_peer(online: u64, boot: usize) -> Option<usize> {
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        if hart != boot && online & hart_bit(hart) != 0 {
            return Some(hart);
        }
        hart += 1;
    }
    None
}

/// Hart **không online** thấp nhất (< `MAX_HARTS`) — dùng cho phép đo timeout TAP 29.
fn find_offline(online: u64) -> Option<usize> {
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        if online & hart_bit(hart) == 0 {
            return Some(hart);
        }
        hart += 1;
    }
    None
}

/// AP một vòng barrier (đến rồi chờ release).
fn ap_round(round: u64) -> bool {
    smp::barrier_join(round, JOIN_TIMEOUT) && smp::barrier_wait_release(round, JOIN_TIMEOUT)
}

/// BSP mở một vòng barrier tay: công bố vòng, tự đến, chờ đủ `online` arrivals.
///
/// **Chưa** release — BSP còn phải sửa page table/marker rồi mới release, để AP chắc chắn prime
/// (đọc mapping **cũ**) **trước** khi bảng đổi.
fn bsp_open(round: u64, online: u64) -> bool {
    smp::barrier_begin(round, online);
    let _ = smp::barrier_join(round, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived, timeout) = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    !timeout && arrived & online == online
}

/// BSP phát release vòng `join_round` rồi mở vòng `done_round` chờ AP verify xong.
///
/// `done_round` phải là đúng vòng AP chờ sau khi phục vụ (`join_round + 1` theo hằng số) — nếu
/// BSP mở nhầm số, AP chờ tới timeout rồi park (TAP đỏ, không treo).
fn bsp_close(join_round: u64, done_round: u64, online: u64) {
    smp::barrier_release(join_round);
    smp::barrier_begin(done_round, online);
    let _ = smp::barrier_join(done_round, smp::BARRIER_TIMEOUT_TICKS);
    let _ = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    smp::barrier_release(done_round);
}

/// In một dòng `#` từ hart bất kỳ (dùng khoá UART của arch); tự thêm `\n` để log không dính dòng.
fn log(args: core::fmt::Arguments<'_>) {
    vifast_plat_riscv64::debug_lock_do(|| {
        let mut writer = PlatformWriter(&PLATFORM);
        let _ = writeln!(writer, "{}", args);
    });
}

// ---------------------------------------------------------------------------------------------
// Hart thứ cấp
// ---------------------------------------------------------------------------------------------

/// Hart thứ cấp ở pha TLB — tham gia **đủ** barrier, là hart đích thật của shootdown khi là peer
/// (hart online nhỏ nhất khác BSP), verify mapping sau flush. Barrier timeout ⇒ park (TAP đỏ, không
/// treo máy).
pub fn secondary_phase(hart: usize) -> ! {
    let online = smp::online_mask();
    let boot = smp::boot_hart();
    let peer = find_peer(online, boot);
    let is_peer = peer == Some(hart);
    // Bật cổng IPI (SSIE + SIE) — trap `scause=(1<<63)|1` sẽ drain + xử lý `TlbFlush`; đường chờ
    // `service_until` cũng tự drain nên phép đo **không phụ thuộc** trap (D2-68).
    let _ = vifast_plat_riscv64::ipi::enable_ipi();

    // --- 100 vòng shootdown trên probe #1: prime trước khi join (BSP chỉ remap sau đủ arrival).
    for round in 1..=ROUNDS {
        let primed = mmu::read_u64(mmu::PROBE_VA);
        if is_peer && primed != marker(round - 1) {
            PEER_PRIME_WRONG.fetch_add(1, Ordering::SeqCst);
        }
        if !ap_round(ROUND_BASE + round) {
            park();
        }
        // **Mọi** hart trong `active_on` phải phục vụ (D2-78: active_on = toàn bộ hart online),
        // nên ack của mỗi AP tăng đúng một mỗi vòng; chỉ peer đọc lại để chứng minh mapping mới.
        let ok = tlb::service_until(SERVICE_TIMEOUT, || tlb::acked_seq(hart) >= round);
        if !ok {
            if is_peer {
                PEER_SERVICE_TIMEOUT.fetch_add(1, Ordering::SeqCst);
            }
            AP_SERVICE_BAD.fetch_add(1, Ordering::SeqCst);
        }
        if is_peer {
            let seen = mmu::read_u64(mmu::PROBE_VA);
            if seen != marker(round) {
                PEER_VERIFY_WRONG.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    if !ap_round(ROUND_ROUNDS_DONE) {
        park();
    }

    // --- TAP 27: (a) hart gọi ngoài `active_on`; (b) hart online ngoài `active_on`.
    if peer.is_some() {
        for (join_round, done_round) in [
            (ROUND_OUTSIDE_JOIN, ROUND_OUTSIDE_DONE),
            (ROUND_OUTSIDE2_JOIN, ROUND_OUTSIDE2_DONE),
        ] {
            // `prev` đọc **trước** join: BSP chỉ post sau khi thấy arrival ⇒ không thể ack sớm
            // chen vào giữa (đọc sau release thì trap IPI có thể đã xử lý ⇒ chờ vô ích).
            let prev = tlb::acked_seq(hart);
            let _primed = mmu::read_u64(mmu::PROBE2_VA);
            if !ap_round(join_round) {
                park();
            }
            // D2-78: hart bị gỡ khỏi `ACTIVE_ON` **không** được phục vụ (và không ack).
            if SERVICE_MASK.load(Ordering::SeqCst) & hart_bit(hart) != 0 {
                let ok = tlb::service_until(SERVICE_TIMEOUT, || tlb::acked_seq(hart) > prev);
                if !ok {
                    AP_SERVICE_BAD.fetch_add(1, Ordering::SeqCst);
                }
                if is_peer {
                    let expect = EXPECT.load(Ordering::SeqCst);
                    let seen = mmu::read_u64(mmu::PROBE2_VA);
                    if !ok || seen != expect {
                        OUTSIDE_WRONG.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
            if !ap_round(done_round) {
                park();
            }
        }
    }

    // --- TAP 28: rollover (cạn ASID) — **mọi** hart online phải flush toàn bộ + adopt ASID mới.
    let prev = tlb::acked_seq(hart);
    let _primed = mmu::read_u64(mmu::PROBE2_VA);
    if !ap_round(ROUND_ROLLOVER_JOIN) {
        park();
    }
    let expect = EXPECT.load(Ordering::SeqCst);
    let ok = tlb::service_until(SERVICE_TIMEOUT, || tlb::acked_seq(hart) > prev);
    let seen = mmu::read_u64(mmu::PROBE2_VA);
    if ok && seen == expect && mmu::current_satp_asid() == tlb::asid() {
        ASID_OK.insert(hart);
    } else {
        ROLLOVER_WRONG.fetch_add(1, Ordering::SeqCst);
    }
    if !ap_round(ROUND_ROLLOVER_DONE) {
        park();
    }

    // --- Chờ BSP kết luận rồi park (park_forever vẫn drain + xử lý TlbFlush nếu có).
    PHASE_OK.insert(hart);
    if !ap_round(ROUND_DONE) {
        park();
    }
    park()
}

fn park() -> ! {
    vifast_plat_riscv64::park_forever()
}

// ---------------------------------------------------------------------------------------------
// BSP: TAP 25 — 100 vòng shootdown
// ---------------------------------------------------------------------------------------------

/// Kết quả 100 vòng (BSP đo + AP ghi).
struct RoundsReport {
    rounds_ok: u64,
    acked_ok: u64,
    sent: u64,
    sent_expected: u64,
    self_slot_wrong: u64,
    local_verify_wrong: u64,
    barrier_bad: u64,
    wait_ticks: u64,
    served: u64,
    targets: u64,
    mode: &'static str,
}

/// BSP chạy 100 vòng: mỗi vòng remap leaf rồi shootdown tới **`active_on()` thật** (`= toàn bộ
/// hart online` — D2-78: API không nhận mask; mọi hart phải phục vụ và ack).
fn bsp_rounds(online: u64, boot: usize, peer: Option<usize>) -> RoundsReport {
    let mut report = RoundsReport {
        rounds_ok: 0,
        acked_ok: 0,
        sent: 0,
        sent_expected: 0,
        self_slot_wrong: 0,
        local_verify_wrong: 0,
        barrier_bad: 0,
        wait_ticks: 0,
        served: 0,
        targets: 0,
        mode: if peer.is_some() { "cross" } else { "self" },
    };
    // D2-78: **không** truyền mask — `shootdown` tự lấy `active_on()`; ở pha này nó = toàn bộ hart
    // online (TAP 27 mới là chỗ gỡ hart khỏi `ACTIVE_ON`).
    let targets = tlb::active_on();
    report.targets = targets;
    let start = now_ticks();
    // Trạng thái đầu: leaf → PA_A, marker 0; xả TLB cục bộ để phép đo bắt đầu từ TLB sạch.
    let _ = tlb::write_phys_u64(mmu::PROBE_PA_A, marker(0));
    if !tlb::remap_probe(mmu::PROBE_PA_A) {
        report.local_verify_wrong += 1;
    }
    tlb::sfence_all();
    let sent_before = TLB_STATS.get(boot).snapshot().sent;
    for round in 1..=ROUNDS {
        // 1) Mở barrier: AP prime mapping **cũ** rồi báo arrival.
        if !bsp_open(ROUND_BASE + round, online) {
            report.barrier_bad += 1;
        }
        // 2) Chuẩn bị mapping mới **sau** arrivals, **trước** release.
        let pa = if round % 2 == 1 {
            mmu::PROBE_PA_B
        } else {
            mmu::PROBE_PA_A
        };
        let _ = tlb::write_phys_u64(pa, marker(round));
        if !tlb::remap_probe(pa) {
            report.local_verify_wrong += 1;
        }
        // 3) Release: AP đi làm việc, BSP shootdown (targets = active_on() bên trong).
        smp::barrier_release(ROUND_BASE + round);
        let outcome = tlb::shootdown(FlushScope::Page { va: mmu::PROBE_VA }, ROUND_TIMEOUT);
        let info = tlb::last_shootdown();
        if outcome == ShootdownOutcome::Completed && info.targets == targets {
            report.rounds_ok += 1;
        }
        if info.acked == targets && info.completed {
            report.acked_ok += 1;
        }
        if tlb::request_seq(boot) != 0 {
            report.self_slot_wrong += 1;
        }
        if mmu::read_u64(mmu::PROBE_VA) != marker(round) {
            report.local_verify_wrong += 1;
        }
    }
    let sent_after = TLB_STATS.get(boot).snapshot().sent;
    report.sent = sent_after - sent_before;
    report.sent_expected = ROUNDS * (mask_count(targets) as u64 - 1);
    report.wait_ticks = now_ticks().wrapping_sub(start);
    // Mọi hart online **khác** BSP phải đã phục vụ đủ 100 yêu cầu (D2-78: active_on = toàn bộ).
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        if hart != boot
            && online & hart_bit(hart) != 0
            && TLB_STATS.get(hart).snapshot().received >= ROUNDS
        {
            report.served += 1;
        }
        hart += 1;
    }
    // Barrier chốt: AP đã verify xong vòng 100.
    let _ = ap_round_close(online);
    report
}

/// BSP chốt vòng sau 100 vòng (AP join `ROUND_ROUNDS_DONE` sau khi verify xong).
fn ap_round_close(online: u64) -> bool {
    smp::barrier_begin(ROUND_ROUNDS_DONE, online);
    let _ = smp::barrier_join(ROUND_ROUNDS_DONE, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived, timeout) = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    smp::barrier_release(ROUND_ROUNDS_DONE);
    !timeout && arrived & online == online
}

// ---------------------------------------------------------------------------------------------
// BSP: TAP 26 — stale mapping (dương tính) với cả ba dạng sfence.vma
// ---------------------------------------------------------------------------------------------

/// Phép đo "TLB thật sự cache": remap **không** sfence ⇒ đọc ra giá trị **cũ**; đúng dạng
/// `sfence.vma` ⇒ đọc ra giá trị **mới**. Lặp cho `(asid,va)`, `(asid,*)`, `(*,*)`.
struct StaleReport {
    forms: u64,
    stale_seen: u64,
    fresh_ok: u64,
    wrong: u64,
    asid: u16,
}

fn bsp_stale(asid: u16) -> StaleReport {
    let mut report = StaleReport {
        forms: 0,
        stale_seen: 0,
        fresh_ok: 0,
        wrong: 0,
        asid,
    };
    let forms = [
        ("Page", FlushScope::Page { va: mmu::PROBE_VA }),
        ("Asid", FlushScope::Asid),
        ("All", FlushScope::All),
    ];
    for (index, (name, scope)) in forms.iter().enumerate() {
        report.forms += 1;
        let marker_a = STALE_A_BASE | index as u64;
        let marker_b = STALE_B_BASE | index as u64;
        // 1) Về PA_A, ghi marker A, xả TLB bằng **đúng dạng** đang đo, đọc (nạp TLB với A).
        let wrote =
            tlb::write_phys_u64(mmu::PROBE_PA_A, marker_a) && tlb::remap_probe(mmu::PROBE_PA_A);
        tlb::flush_scope(*scope, asid);
        let fresh_a = mmu::read_u64(mmu::PROBE_VA) == marker_a;
        if !(wrote && fresh_a) {
            report.wrong += 1;
        }
        // 2) Ghi marker B vào PA_B rồi remap **không** sfence: TLB phải còn translation cũ.
        let wrote_b =
            tlb::write_phys_u64(mmu::PROBE_PA_B, marker_b) && tlb::remap_probe(mmu::PROBE_PA_B);
        let stale = mmu::read_u64(mmu::PROBE_VA);
        let stale_ok = wrote_b && stale == marker_a;
        if stale_ok {
            report.stale_seen += 1;
        } else {
            report.wrong += 1;
        }
        // 3) sfence đúng dạng ⇒ lần đọc này phải thấy marker B.
        tlb::flush_scope(*scope, asid);
        let fresh_b = mmu::read_u64(mmu::PROBE_VA) == marker_b;
        if fresh_b {
            report.fresh_ok += 1;
        } else {
            report.wrong += 1;
        }
        log(format_args!(
            "# stale form: name={} fresh_a={} stale_seen={} fresh_b={} wrong={}",
            name, fresh_a as u8, stale_ok as u8, fresh_b as u8, report.wrong
        ));
    }
    report
}

// ---------------------------------------------------------------------------------------------
// BSP: TAP 27 — hart ngoài active_on
// ---------------------------------------------------------------------------------------------

struct OutsideReport {
    removed: u64,
    removed_count: u64,
    narrowed: u64,
    targets: u64,
    acked: u64,
    caller_in: bool,
    removed_acked_before: u64,
    removed_acked_after: u64,
    restored: bool,
    restored_acked: u64,
    ok: bool,
}

/// Bitmask "hart có ack" trong `mask` (không cần seq cụ thể: TAP so trước/sau).
fn acked_of_mask(mask: u64) -> u64 {
    let mut out = 0u64;
    let mut hart = 0usize;
    while hart < MAX_HARTS {
        if mask & hart_bit(hart) != 0 && tlb::acked_seq(hart) != 0 {
            out |= hart_bit(hart);
        }
        hart += 1;
    }
    out
}

/// **TAP 27 (D2-78)** — trạng thái `active_on` **thật**:
///
/// 1. Gỡ hart **khác** (2 hart phụ ở `-smp 4`, hoặc hart boot khi chỉ 1/2 hart) khỏi `ACTIVE_ON`
///    bằng API test-only, lưu mask cũ. Shootdown (không truyền mask) ⇒ hart bị gỡ **không** ack,
///    **không** bị chờ; `acked == active_on()` mới.
/// 2. Khôi phục `ACTIVE_ON` rồi shootdown lần nữa ⇒ mọi hart online ack (máy trở lại đủ).
fn bsp_outside(online: u64, boot: usize, peer: Option<usize>) -> OutsideReport {
    let mut report = OutsideReport {
        removed: 0,
        removed_count: 0,
        narrowed: 0,
        targets: 0,
        acked: 0,
        caller_in: false,
        removed_acked_before: 0,
        removed_acked_after: 0,
        restored: false,
        restored_acked: 0,
        ok: false,
    };
    let extras = unused_online_harts(hart_bit(boot) | peer.map(hart_bit).unwrap_or(0), online);
    // Ưu tiên gỡ 2 hart phụ (`-smp 4`); nếu không có (1/2 hart) gỡ hart boot để vẫn là "hart
    // ngoài active_on" thật, không phải mask hẹp giả lập.
    report.removed = if extras != 0 { extras } else { hart_bit(boot) };
    report.removed_count = mask_count(report.removed) as u64;
    let old = tlb::active_on();
    let narrowed = old & !report.removed;
    report.narrowed = narrowed;
    report.caller_in = narrowed & hart_bit(boot) != 0;

    if let Some(_peer_hart) = peer {
        // ---------- (1) gỡ hart khỏi ACTIVE_ON (thật), shootdown
        tlb::test_active_on_set(narrowed);
        report.removed_acked_before = acked_of_mask(report.removed);
        SERVICE_MASK.store(narrowed, Ordering::SeqCst);
        let expected = SYNC_MARKER | 0x00a1;
        let opened = bsp_open(ROUND_OUTSIDE_JOIN, online);
        let current = current_probe2_pa().unwrap_or(mmu::PROBE2_PA_C);
        let next = other_probe2_pa(current);
        let prepared = tlb::write_phys_u64(next, expected) && tlb::remap_probe2(next);
        EXPECT.store(expected, Ordering::SeqCst);
        smp::barrier_release(ROUND_OUTSIDE_JOIN);
        let outcome = tlb::shootdown(FlushScope::Page { va: mmu::PROBE2_VA }, ROUND_TIMEOUT);
        let info = tlb::last_shootdown();
        report.targets = info.targets;
        report.acked = info.acked;
        report.removed_acked_after = acked_of_mask(report.removed);
        let narrow_ok = opened
            && prepared
            && outcome == ShootdownOutcome::Completed
            && info.targets == narrowed
            && info.acked == narrowed
            && report.removed_acked_after == report.removed_acked_before;
        bsp_close(ROUND_OUTSIDE_JOIN, ROUND_OUTSIDE_DONE, online);

        // ---------- (2) khôi phục ACTIVE_ON (thật), shootdown lại ⇒ mọi hart online ack
        tlb::test_active_on_set(old);
        SERVICE_MASK.store(old, Ordering::SeqCst);
        let expected2 = SYNC_MARKER | 0x00b2;
        let opened2 = bsp_open(ROUND_OUTSIDE2_JOIN, online);
        let current2 = current_probe2_pa().unwrap_or(mmu::PROBE2_PA_C);
        let next2 = other_probe2_pa(current2);
        let prepared2 = tlb::write_phys_u64(next2, expected2) && tlb::remap_probe2(next2);
        EXPECT.store(expected2, Ordering::SeqCst);
        smp::barrier_release(ROUND_OUTSIDE2_JOIN);
        let outcome2 = tlb::shootdown(FlushScope::Page { va: mmu::PROBE2_VA }, ROUND_TIMEOUT);
        let info2 = tlb::last_shootdown();
        report.restored_acked = info2.acked;
        report.restored = opened2
            && prepared2
            && outcome2 == ShootdownOutcome::Completed
            && info2.targets == old
            && info2.acked == old
            && acked_of_mask(report.removed) == report.removed;
        bsp_close(ROUND_OUTSIDE2_JOIN, ROUND_OUTSIDE2_DONE, online);
        report.ok = narrow_ok && report.restored;
    } else {
        // `-smp 1`: gỡ chính hart boot ⇒ `active_on` rỗng (thật), shootdown không chờ ai; khôi phục
        // rồi shootdown lại ⇒ ack ngầm của boot.
        tlb::test_active_on_set(narrowed);
        report.removed_acked_before = acked_of_mask(report.removed);
        let empty = tlb::shootdown(FlushScope::Page { va: mmu::PROBE2_VA }, ROUND_TIMEOUT);
        let empty_info = tlb::last_shootdown();
        report.targets = empty_info.targets;
        report.acked = empty_info.acked;
        report.removed_acked_after = acked_of_mask(report.removed);
        tlb::test_active_on_set(old);
        let restored = tlb::shootdown(FlushScope::Asid, ROUND_TIMEOUT);
        let restored_info = tlb::last_shootdown();
        report.restored_acked = restored_info.acked;
        report.restored = restored == ShootdownOutcome::Completed
            && restored_info.targets == old
            && restored_info.acked == old;
        report.ok = empty == ShootdownOutcome::Completed
            && empty_info.targets == 0
            && empty_info.acked == 0
            && report.removed_acked_after == report.removed_acked_before
            && report.restored;
    }
    // Luôn trả `ACTIVE_ON` về đúng trạng thái online (kể cả nhánh lỗi).
    tlb::test_active_on_set(old);
    report
}

// ---------------------------------------------------------------------------------------------
// BSP: TAP 28 — rollover (cạn ASID)
// ---------------------------------------------------------------------------------------------

struct RolloverReport {
    generation: u64,
    exhaustions: u64,
    rollover: bool,
    flush_ok: bool,
    asid: u16,
    satp_asid: u16,
    stale_seen: bool,
    fresh_ok: bool,
    free: usize,
    issued: u32,
    asid_ok_mask: u64,
    primed: u64,
    wrong: u64,
}

/// Cấp cạn ASID rồi **một** flush toàn bộ: TLB entry của ASID 0 (được phát lại) phải bị xả.
fn bsp_rollover(online: u64, boot: usize) -> RolloverReport {
    let before = tlb::asid_state();
    let old_marker = SYNC_MARKER | 0x00c2;
    let expect = SYNC_MARKER | 0x00c3;
    // BSP prime TLB với mapping **cũ** (probe #2) trước khi barrier để có entry thật mà xả.
    let current = current_probe2_pa().unwrap_or(mmu::PROBE2_PA_C);
    let _ = tlb::write_phys_u64(current, old_marker);
    tlb::flush_scope(FlushScope::Page { va: mmu::PROBE2_VA }, tlb::asid());
    let primed = mmu::read_u64(mmu::PROBE2_VA);
    // Cấp cạn free-list: lease kế tiếp **phải** rollover (`generation += 1`).
    let mut rollover_lease = None;
    let mut index = 0usize;
    while index <= before.free {
        let lease = tlb::allocate_lease();
        if lease.rollover {
            rollover_lease = Some(lease);
            break;
        }
        index += 1;
    }
    // Barrier: AP prime mapping cũ rồi báo arrival; BSP mới remap + phát flush toàn bộ.
    if !bsp_open(ROUND_ROLLOVER_JOIN, online) {
        // Không release cũng không treo: barrier timeout đã trả về; vẫn tiếp tục đo để TAP đỏ rõ.
    }
    let next = other_probe2_pa(current);
    let prepared = tlb::write_phys_u64(next, expect) && tlb::remap_probe2(next);
    EXPECT.store(expect, Ordering::SeqCst);
    smp::barrier_release(ROUND_ROLLOVER_JOIN);
    let lease = rollover_lease.unwrap_or_default();
    let replaced = tlb::replace_lease(lease, ROUND_TIMEOUT);
    let after = tlb::asid_state();
    let fresh = mmu::read_u64(mmu::PROBE2_VA);
    let asid_ok_self = mmu::current_satp_asid() == tlb::asid();
    if asid_ok_self {
        ASID_OK.insert(boot);
    }
    bsp_close(ROUND_ROLLOVER_JOIN, ROUND_ROLLOVER_DONE, online);
    let mut report = RolloverReport {
        generation: after.generation,
        exhaustions: after.exhaustions,
        rollover: replaced.rollover,
        flush_ok: replaced.flush == Some(ShootdownOutcome::Completed),
        asid: replaced.asid,
        satp_asid: mmu::current_satp_asid(),
        stale_seen: primed == old_marker,
        fresh_ok: fresh == expect && prepared,
        free: after.free,
        issued: after.issued,
        asid_ok_mask: ASID_OK.bits() & online,
        primed,
        wrong: 0,
    };
    if !report.stale_seen || !report.fresh_ok || !report.flush_ok {
        report.wrong += 1;
    }
    if report.generation != before.generation + 1 || report.exhaustions != before.exhaustions + 1 {
        report.wrong += 1;
    }
    if report.satp_asid != report.asid || report.asid != 0 {
        report.wrong += 1;
    }
    if online & !report.asid_ok_mask != 0 {
        // Hart online chưa xác nhận ASID mới (có ý nghĩa khi > 1 hart).
        report.wrong += 1;
    }
    if replaced.flush.is_none() {
        report.wrong += 1;
    }
    report
}

// ---------------------------------------------------------------------------------------------
// BSP: TAP 29 — timeout không treo
// ---------------------------------------------------------------------------------------------

struct TimeoutReport {
    outcome: &'static str,
    narrowed: u64,
    targets: u64,
    acked: u64,
    missing: u64,
    waited: u64,
    timers: u64,
    mailbox_empty: bool,
    recovered: bool,
    wrong: u64,
}

/// **TAP 29 (D2-78)**: đặt `ACTIVE_ON` = `{boot, hart offline}` (trạng thái thật của một VSpace có
/// hart đã chết) ⇒ shootdown phải **Timeout** sau ≥ deadline, không treo; rồi thu hẹp `ACTIVE_ON`
/// về `{boot}` để chứng minh máy còn dùng được, cuối cùng khôi phục `ACTIVE_ON` đầy đủ.
fn bsp_timeout(boot: usize, online: u64) -> TimeoutReport {
    let offline = find_offline(online);
    let mut report = TimeoutReport {
        outcome: "None",
        narrowed: 0,
        targets: 0,
        acked: 0,
        missing: 0,
        waited: 0,
        timers: 0,
        mailbox_empty: false,
        recovered: false,
        wrong: 0,
    };
    let Some(offline_hart) = offline else {
        report.wrong += 1;
        return report;
    };
    let old = tlb::active_on();
    let narrowed = hart_bit(boot) | hart_bit(offline_hart);
    report.narrowed = narrowed;
    tlb::test_active_on_set(narrowed);
    let before = now_ticks();
    let outcome = tlb::shootdown(FlushScope::Page { va: mmu::PROBE2_VA }, TIMEOUT_PROBE);
    let waited = now_ticks().wrapping_sub(before);
    let info = tlb::last_shootdown();
    report.outcome = outcome.name();
    report.targets = info.targets;
    report.acked = info.acked;
    report.missing = info.targets & !info.acked;
    report.waited = waited;
    report.timers = TLB_STATS.get(boot).snapshot().timeouts;
    report.mailbox_empty = vifast_plat_riscv64::ipi::pending_of(offline_hart).is_empty()
        && tlb::request_seq(offline_hart) == 0;
    // Máy còn dùng được: `active_on = {boot}` ⇒ shootdown cục bộ phải Ok ngay.
    tlb::test_active_on_set(hart_bit(boot));
    let recovered = tlb::shootdown(FlushScope::Asid, ROUND_TIMEOUT);
    report.recovered = recovered == ShootdownOutcome::Completed;
    tlb::test_active_on_set(old);
    if report.outcome != "Timeout"
        || report.targets != narrowed
        || report.missing != hart_bit(offline_hart)
        || report.waited < TIMEOUT_PROBE
        || !report.mailbox_empty
        || !report.recovered
        || report.timers == 0
    {
        report.wrong += 1;
    }
    report
}

// ---------------------------------------------------------------------------------------------
// BSP: entry
// ---------------------------------------------------------------------------------------------

/// Pha TLB trên **BSP**: TAP 25..29. Trả số fail (0 = xanh).
///
/// `ktest.rs` đã gọi `tlb::init()` **trước** pha SMP (để hart thứ cấp `adopt_satp` nhận ASID); ở
/// đây chỉ đọc/by chứng, **không** cấp lease lần hai.
pub fn tlb_phase() -> usize {
    let mut fails = 0usize;
    let online = smp::online_mask();
    let boot = PLATFORM.hart_id();
    let peer = find_peer(online, boot);
    let n = mask_count(online) as usize;
    let state = tlb::asid_state();
    let asid = tlb::asid();
    let generation = tlb::generation();

    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tlb: asid={} generation={} rollover=0 satp=0x{:x} satp_asid={} active_on=0x{:x} free={} issued={} exhaustions={}",
            asid,
            generation,
            mmu::satp_now(),
            mmu::current_satp_asid(),
            tlb::active_on(),
            state.free,
            state.issued,
            state.exhaustions
        );
        let _ = writeln!(
            w,
            "# tlb: harts={} boot={} peer={} mode={} rounds={} asid_bits=8 asid_count=256",
            n,
            boot,
            peer.map(|h| h as i64).unwrap_or(-1),
            if peer.is_some() { "cross" } else { "self" },
            ROUNDS
        );
        let _ = writeln!(
            w,
            "# tlb probe: va=0x{:x} pa_a=0x{:x} pa_b=0x{:x} probe2_va=0x{:x} pa_c=0x{:x} pa_d=0x{:x} leaf_a={} leaf_b={} nx=1",
            mmu::PROBE_VA,
            mmu::PROBE_PA_A,
            mmu::PROBE_PA_B,
            mmu::PROBE2_VA,
            mmu::PROBE2_PA_C,
            mmu::PROBE2_PA_D,
            current_probe_pa().map(|_| 1).unwrap_or(0),
            current_probe2_pa().map(|_| 1).unwrap_or(0)
        );
    }

    // ---------------------------------------------------------------- TAP 25: 100 vòng shootdown
    let rounds = bsp_rounds(online, boot, peer);
    {
        let peer_stats = peer
            .map(|h| TLB_STATS.get(h).snapshot())
            .unwrap_or_default();
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# shootdown rounds: mode={} rounds={} ok={} acked_ok={} sent={} sent_expected={} self_slot_wrong={} local_verify_wrong={} barrier_bad={} targets=0x{:x} active_on=0x{:x} wait_ticks={}",
            rounds.mode,
            ROUNDS,
            rounds.rounds_ok,
            rounds.acked_ok,
            rounds.sent,
            rounds.sent_expected,
            rounds.self_slot_wrong,
            rounds.local_verify_wrong,
            rounds.barrier_bad,
            rounds.targets,
            tlb::active_on(),
            rounds.wait_ticks
        );
        let _ = writeln!(
            w,
            "# shootdown all: bad={} served={} expected_served={} online=0x{:x}",
            AP_SERVICE_BAD.load(Ordering::SeqCst),
            rounds.served,
            n.saturating_sub(1),
            online
        );
        let _ = writeln!(
            w,
            "# shootdown peer: mode={} received={} flushes={} page={} asid={} all={} implicit={} prime_wrong={} verify_wrong={} service_timeout={}",
            rounds.mode,
            peer_stats.received,
            peer_stats.flushes,
            peer_stats.page_flushes,
            peer_stats.asid_flushes,
            peer_stats.all_flushes,
            peer_stats.implicit,
            PEER_PRIME_WRONG.load(Ordering::SeqCst),
            PEER_VERIFY_WRONG.load(Ordering::SeqCst),
            PEER_SERVICE_TIMEOUT.load(Ordering::SeqCst)
        );
    }
    let rounds_ok = rounds.rounds_ok == ROUNDS
        && rounds.acked_ok == ROUNDS
        && rounds.sent == rounds.sent_expected
        && rounds.self_slot_wrong == 0
        && rounds.local_verify_wrong == 0
        && rounds.barrier_bad == 0
        && PEER_PRIME_WRONG.load(Ordering::SeqCst) == 0
        && PEER_VERIFY_WRONG.load(Ordering::SeqCst) == 0
        && PEER_SERVICE_TIMEOUT.load(Ordering::SeqCst) == 0
        && AP_SERVICE_BAD.load(Ordering::SeqCst) == 0
        && rounds.served as usize + 1 == n
        && rounds.targets == tlb::active_on();
    if rounds_ok {
        PLATFORM.put_str("ok 25 - 100 vong shootdown Ok\n");
    } else {
        PLATFORM.put_str("not ok 25 - 100 vong shootdown Ok: thieu ack/flush/self-target\n");
        fails += 1;
    }

    // ---------------------------------------------------------------- TAP 26: stale mapping caught
    let stale = bsp_stale(asid);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# stale: asid={} va=0x{:x} forms={} stale_seen={} fresh_ok={} wrong={} note=stale-read-without-sfence-is-positive",
            stale.asid, mmu::PROBE_VA, stale.forms, stale.stale_seen, stale.fresh_ok, stale.wrong
        );
    }
    let stale_ok =
        stale.forms == 3 && stale.stale_seen == 3 && stale.fresh_ok == 3 && stale.wrong == 0;
    if stale_ok {
        PLATFORM.put_str("ok 26 - stale mapping caught\n");
    } else {
        PLATFORM.put_str("not ok 26 - stale mapping caught: TLB khong cache/khong xa dung\n");
        fails += 1;
    }

    // ---------------------------------------------------------------- TAP 27: hart ngoài active_on
    let outside = bsp_outside(online, boot, peer);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# outside: mode={} removed=0x{:x} removed_count={} narrowed=0x{:x} targets=0x{:x} acked=0x{:x} caller_in={} removed_acked_before=0x{:x} removed_acked_after=0x{:x} restored={} restored_acked=0x{:x} outside_wrong={} ok={}",
            if peer.is_some() { "cross" } else { "self" },
            outside.removed,
            outside.removed_count,
            outside.narrowed,
            outside.targets,
            outside.acked,
            outside.caller_in as u8,
            outside.removed_acked_before,
            outside.removed_acked_after,
            outside.restored as u8,
            outside.restored_acked,
            OUTSIDE_WRONG.load(Ordering::SeqCst),
            outside.ok as u8
        );
    }
    let outside_ok = outside.ok && OUTSIDE_WRONG.load(Ordering::SeqCst) == 0;
    if outside_ok {
        PLATFORM.put_str("ok 27 - hart ngoai active_on\n");
    } else {
        PLATFORM.put_str("not ok 27 - hart ngoai active_on: bi cho/ack sai\n");
        fails += 1;
    }

    // ---------------------------------------------------------------- TAP 28: rollover flush
    let rollover = bsp_rollover(online, boot);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# rollover: generation={} exhaustions={} rollover={} flush={} asid={} satp_asid={} stale_seen={} fresh_ok={} free={} issued={} asid_ok=0x{:x} online=0x{:x} primed=0x{:x} wrong={}",
            rollover.generation,
            rollover.exhaustions,
            rollover.rollover as u8,
            if rollover.flush_ok { "Ok" } else { "Fail" },
            rollover.asid,
            rollover.satp_asid,
            rollover.stale_seen as u8,
            rollover.fresh_ok as u8,
            rollover.free,
            rollover.issued,
            rollover.asid_ok_mask,
            online,
            rollover.primed,
            rollover.wrong
        );
    }
    let rollover_ok = rollover.rollover
        && rollover.flush_ok
        && rollover.stale_seen
        && rollover.fresh_ok
        && rollover.wrong == 0
        && ROLLOVER_WRONG.load(Ordering::SeqCst) == 0;
    if rollover_ok {
        PLATFORM.put_str("ok 28 - rollover flush\n");
    } else {
        PLATFORM.put_str("not ok 28 - rollover flush: generation/flush/stale sai\n");
        fails += 1;
    }

    // ---------------------------------------------------------------- TAP 29: timeout không treo
    let timeout = bsp_timeout(boot, online);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timeout: outcome={} narrowed=0x{:x} targets=0x{:x} acked=0x{:x} missing=0x{:x} waited={} probe={} timeouts={} mailbox_empty={} recovered={} wrong={}",
            timeout.outcome,
            timeout.narrowed,
            timeout.targets,
            timeout.acked,
            timeout.missing,
            timeout.waited,
            TIMEOUT_PROBE,
            timeout.timers,
            timeout.mailbox_empty as u8,
            timeout.recovered as u8,
            timeout.wrong
        );
    }
    let timeout_ok = timeout.outcome == "Timeout"
        && timeout.recovered
        && timeout.mailbox_empty
        && timeout.waited >= TIMEOUT_PROBE
        && timeout.wrong == 0;
    if timeout_ok {
        PLATFORM.put_str("ok 29 - timeout khong treo\n");
    } else {
        PLATFORM.put_str("not ok 29 - timeout khong treo: treo/khong ro timeout\n");
        fails += 1;
    }

    // Nhả AP rồi in bằng chứng ASID per-hart.
    smp::barrier_begin(ROUND_DONE, online);
    let _ = smp::barrier_join(ROUND_DONE, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived, timed_out) = smp::barrier_wait_arrivals(smp::BARRIER_TIMEOUT_TICKS);
    smp::barrier_release(ROUND_DONE);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tlb asid: ok=0x{:x} online=0x{:x} phase=0x{:x} arrived=0x{:x} timeout={} satp_asid_boot={} lease_asid={} generation={}",
            ASID_OK.bits() & online,
            online,
            PHASE_OK.bits() & online,
            arrived,
            timed_out as u8,
            mmu::current_satp_asid(),
            tlb::asid(),
            tlb::generation()
        );
    }
    if !timed_out && ASID_OK.bits() & online != online {
        fails += 1;
    }
    fails
}

// Bất biến hằng số: đổi `PROBE*_VA`/marker phải giữ ba dạng Sv39 và các miền marker tách nhau.
const _: () = {
    assert!(mmu::PROBE_VA & ((1 << 21) - 1) == 0);
    assert!(mmu::PROBE2_VA & ((1 << 21) - 1) == 0);
    assert!(mmu::PROBE_VA >> 39 == 0);
    assert!(mmu::PROBE2_VA >> 39 == 0);
    assert!(mmu::PROBE_VA != mmu::PROBE2_VA);
    assert!(mmu::PROBE_PA_A != mmu::PROBE_PA_B);
    assert!(mmu::PROBE2_PA_C != mmu::PROBE2_PA_D);
    assert!(session_marker_ok());
};

/// Marker các pha không lẫn nhau (hàm `const` phụ để `assert!` gọn): mọi nền marker chỉ khác nhau
/// ở nibble cao và đều có 24 bit thấp = 0 để `marker(round)` / marker pha không đè nhau.
const fn session_marker_ok() -> bool {
    let all = [MARKER_BASE, STALE_A_BASE, STALE_B_BASE, SYNC_MARKER];
    let mut i = 0;
    while i < all.len() {
        if all[i] & 0x00ff_ffff != 0 {
            return false;
        }
        let mut j = i + 1;
        while j < all.len() {
            if all[i] == all[j] {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}
