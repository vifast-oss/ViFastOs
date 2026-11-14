// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **SMP** của ảnh test `vifast-kbin-ktest` (KT-49) — tách khỏi `ktest.rs` để giữ luật cấu
//! trúc R1 (≤ 1.500 dòng/tệp).
//!
//! Được `ktest.rs` gọi bằng [`smp_phase`] trên **BSP** (hart 0) sau các TAP KT-45..KT-48; đồng
//! thời cung cấp `rust_secondary_main` mà asm `_start_secondary` của `vifast-plat-riscv64` gọi cho
//! hart thứ cấp (SBI HSM). Nội dung: bring-up HSM, barrier có timeout, TAP 15..19 (harts alive,
//! stack/guard riêng mỗi hart, timer PerHart, PLIC context `2*h+1`, barrier/chậm-timeout).
//!
//! Số liệu/bằng chứng do boot in ra (`# smp:`, `# hart h alive`, `# stack hart=h`, `# timer
//! hart=h`, `# plic hart=h`, `# barrier:`) — runner `ktest.sh` grep đúng các dòng đó; tài liệu
//! không chép số tay (QD §9 Q12). Xem `v2/docs/k3-riscv64.md` §12.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::mmu::{PTE_EXEC, PTE_READ, PTE_WRITE};
use vifast_plat_common::plic::{PLIC_STATS, UART_IRQ_LIMIT};
use vifast_plat_common::smp::{mask_count, HartMask, MAX_HARTS};
use vifast_plat_common::timer::{TICK_TICKS, TIMEBASE_HZ, TIMER_STATS};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::plic;
use vifast_plat_riscv64::smp;
use vifast_plat_riscv64::timer;
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

// ---------------------------------------------------------------------------------------------
// KT-49 — trạng thái SMP của ảnh test
// ---------------------------------------------------------------------------------------------

/// Vòng barrier: alive → guard xong → timer xong → PLIC xong → barrier thường → barrier timeout.
const ROUND_ALIVE: u64 = 1;
const ROUND_GUARD: u64 = 2;
const ROUND_TIMER: u64 = 3;
const ROUND_PLIC: u64 = 4;
const ROUND_BARRIER: u64 = 5;
const ROUND_TIMEOUT: u64 = 6;
/// Hart "chậm" cho test barrier (nếu online) và thời gian trễ **đúng 5 ms** (`TICK_TICKS/2` = nửa
/// tick 10 ms) — chứng minh barrier chờ hart chậm mà **không treo**. (Bản trước dùng
/// `TICK_TICKS/20` = 0,5 ms — reviewer-kt49 bắt lỗi đơn vị; 0,5 ms nằm trong nhiễu icount nên
/// không chứng minh được "hart chậm".)
const SLOW_HART: usize = 1;
const SLOW_DELAY_TICKS: u64 = TICK_TICKS / 2;

static ALIVE: HartMask = HartMask::new();
static STACK_OK: HartMask = HartMask::new();
static GUARD_OK: HartMask = HartMask::new();
static TIMER_OK: HartMask = HartMask::new();
static PLIC_OK: HartMask = HartMask::new();
static GUARD_DONE: HartMask = HartMask::new();
/// Lượt guard hiện tại (BSP đặt; AP chờ `TURN == hart` rồi tự chạm guard của mình).
static GUARD_TURN: AtomicU64 = AtomicU64::new(u64::MAX);
/// Kết quả timer per-hart (ghi bởi chính hart, đọc bởi BSP sau barrier).
static TIMER_TICKS: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static TIMER_FIRST: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static TIMER_LATE: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static TIMER_MISSED: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static TIMER_NONMONO: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// Kết quả PLIC per-hart (ctx/mmode/threshold/priority/enable round-trip).
static PLIC_CTX: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static PLIC_MCTX: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static PLIC_THRESHOLD: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
static PLIC_PRIORITY: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static PLIC_FLAGS: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// Thời điểm tới barrier của từng hart + cờ hart chậm đã trễ thật.
static ARRIVE_TIME: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
static SLOW_DELAYED: AtomicU64 = AtomicU64::new(0);
/// Kết quả barrier của BSP (arrival/vòng 5, timeout/vòng 6, thời gian chờ).
static BARRIER5_ARRIVED: AtomicU64 = AtomicU64::new(0);
static BARRIER5_MISSING: AtomicU64 = AtomicU64::new(0);
static BARRIER5_TIMEOUT: AtomicU64 = AtomicU64::new(0);
static BARRIER5_WAIT: AtomicU64 = AtomicU64::new(0);
static BARRIER6_ARRIVED: AtomicU64 = AtomicU64::new(0);
static BARRIER6_MISSING: AtomicU64 = AtomicU64::new(0);
static BARRIER6_TIMEOUT: AtomicU64 = AtomicU64::new(0);
static BARRIER6_WAIT: AtomicU64 = AtomicU64::new(0);
static BARRIER6_EXPECTED_MISSING: AtomicU64 = AtomicU64::new(0);

/// `rdtime` hiện tại (timebase) cho đo/barrier.
fn now_ticks_test() -> u64 {
    PLATFORM.now_ticks()
}

/// Mỗi hart in **một dòng** alive (id + ticket + địa chỉ vùng riêng), dưới khoá UART của arch.
fn alive_phase() -> u64 {
    let hart = PLATFORM.hart_id();
    let ticket = smp::take_ticket(hart);
    let (stack_bottom, stack_top) = mmu::stack_range_of(hart);
    let (trap_bottom, trap_top) = mmu::trap_stack_range_of(hart);
    let (guard_start, guard_end) = mmu::guard_range_of(hart);
    vifast_plat_riscv64::debug_lock_do(|| {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# hart {} alive ticket={} tp={} stack=0x{:x}-0x{:x} trap=0x{:x}-0x{:x} guard=0x{:x}-0x{:x}",
            hart,
            ticket,
            PLATFORM.hart_id(),
            stack_bottom,
            stack_top,
            trap_bottom,
            trap_top,
            guard_start,
            guard_end
        );
    });
    ALIVE.insert(hart);
    ticket
}

/// BSP chạy một vòng barrier: công bố vòng, tự đến, chờ đủ arrival (timeout), rồi phát release.
fn bsp_barrier(round: u64, expected: u64, timeout_ticks: u64) -> (u64, bool, u64) {
    smp::barrier_begin(round, expected);
    let start = now_ticks_test();
    let _ = smp::barrier_join(round, timeout_ticks);
    let (arrived, timed_out) = smp::barrier_wait_arrivals(timeout_ticks);
    smp::barrier_release(round);
    (arrived, timed_out, now_ticks_test().wrapping_sub(start))
}

/// Hart thứ cấp chạy một vòng barrier: chờ vòng được công bố, đến, chờ release.
fn ap_barrier(round: u64, timeout_ticks: u64) -> bool {
    smp::barrier_join(round, timeout_ticks) && smp::barrier_wait_release(round, timeout_ticks)
}

/// Chạm thử **stack + trap stack + guard của chính `hart`** — trả `true` nếu mọi thứ đúng.
///
/// - stack/trap stack RW NX, ghi/đọc marker khớp;
/// - guard NONE ở cả identity, nửa cao, physmap và **thuộc đúng hart** (`guard_owner`);
/// - `probe_guard_load` gây page fault `scause=13` **thật**, handler resume **một** instruction,
///   trapframe nằm trong `.trap_stack` **của hart này** (chứng minh `sscratch` per-hart).
fn guard_probe_own_hart(hart: usize) -> bool {
    let (stack_bottom, stack_top) = mmu::stack_range_of(hart);
    let (trap_bottom, trap_top) = mmu::trap_stack_range_of(hart);
    let (guard_start, guard_end) = mmu::guard_range_of(hart);
    let marker = 0x4b34_9000_0000_0000u64 | hart as u64;
    mmu::write_u64(stack_bottom, marker);
    let stack_rw = mmu::read_u64(stack_bottom) == marker;
    mmu::write_u64(trap_bottom, marker ^ 0x5a);
    let trap_rw = mmu::read_u64(trap_bottom) == marker ^ 0x5a;
    let stack_walk = mmu::walk_sw(stack_bottom);
    let trap_walk = mmu::walk_sw(trap_bottom);
    let rw_nx = |walk: vifast_plat_common::mmu::Walk| {
        walk.valid
            && walk.leaf
            && walk.level == 0
            && walk.raw() & PTE_EXEC == 0
            && walk.raw() & (PTE_READ | PTE_WRITE) == (PTE_READ | PTE_WRITE)
    };
    let guard_none = !mmu::walk_sw(guard_start).valid
        && mmu::high_alias(guard_start)
            .map(|va| !mmu::walk_sw(va).valid)
            .unwrap_or(false)
        && mmu::physmap_alias(guard_start)
            .map(|va| !mmu::walk_sw(va).valid)
            .unwrap_or(false)
        && mmu::guard_owner(guard_start) == Some(hart)
        && guard_end == guard_start + 0x1000;
    let before = vifast_plat_common::trap::GUARD_STATS.snapshot();
    let faulted = mmu::probe_guard_load(guard_start);
    let after = vifast_plat_common::trap::GUARD_STATS.snapshot();
    let frame = smp::last_trap_frame(hart);
    let frame_ok = frame >= trap_bottom
        && frame < trap_top
        && frame + vifast_plat_common::trap::TF_SIZE as u64 <= trap_top
        && frame >= stack_top
        && frame >= trap_bottom;
    let fault_ok = after.load_faults == before.load_faults + 1
        && after.recovered == before.recovered + 1
        && after.last_scause == 13
        && after.last_stval == guard_start
        && !mmu::guard_recovery_armed();
    let ok = stack_rw
        && trap_rw
        && rw_nx(stack_walk)
        && rw_nx(trap_walk)
        && guard_none
        && frame_ok
        && fault_ok;
    let _ = faulted; // giá trị chỉ để giữ instruction `ld` gây fault
    if ok {
        STACK_OK.insert(hart);
        GUARD_OK.insert(hart);
    }
    ok
}

/// AP chờ tới lượt (`GUARD_TURN == hart`), tự chạm guard **của mình**, đánh dấu xong.
///
/// Nếu hết timeout mà chưa tới lượt, AP không làm gì và đi tiếp (BSP sẽ thấy bit thiếu ⇒ TAP đỏ,
/// nhưng máy không treo).
fn wait_guard_turn(hart: usize) {
    let start = now_ticks_test();
    while now_ticks_test().wrapping_sub(start) <= smp::BARRIER_TIMEOUT_TICKS {
        if GUARD_TURN.load(Ordering::SeqCst) == hart as u64 {
            let _ = guard_probe_own_hart(hart);
            GUARD_DONE.insert(hart);
            return;
        }
        core::hint::spin_loop();
    }
}

/// BSP điều phối lượt guard: đặt `GUARD_TURN = h` rồi tự chạm guard hart mình hoặc chờ AP xong.
fn bsp_guard_round(online: u64) {
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) == 0 {
            continue;
        }
        GUARD_TURN.store(hart as u64, Ordering::SeqCst);
        if hart == PLATFORM.hart_id() {
            let _ = guard_probe_own_hart(hart);
            GUARD_DONE.insert(hart);
        } else {
            let start = now_ticks_test();
            while !GUARD_DONE.contains(hart)
                && now_ticks_test().wrapping_sub(start) <= smp::BARRIER_TIMEOUT_TICKS
            {
                core::hint::spin_loop();
            }
        }
    }
}

/// Mỗi hart arm SBI TIME **của mình**, chờ đủ 100 tick trong slot PerHart của hart đó rồi dừng.
fn timer_phase(hart: usize) {
    let arm = timer::init();
    let start = now_ticks_test();
    let mut ok = false;
    while now_ticks_test().wrapping_sub(start) <= 2 * TIMEBASE_HZ {
        if TIMER_STATS.get(hart).snapshot().ticks >= 100 {
            ok = true;
            break;
        }
        core::hint::spin_loop();
    }
    PLATFORM.set_timer_deadline(u64::MAX);
    PLATFORM.disable_timer();
    let snap = TIMER_STATS.get(hart).snapshot();
    TIMER_TICKS[hart].store(snap.ticks, Ordering::SeqCst);
    TIMER_FIRST[hart].store(snap.first, Ordering::SeqCst);
    TIMER_LATE[hart].store(snap.late, Ordering::SeqCst);
    TIMER_MISSED[hart].store(snap.missed, Ordering::SeqCst);
    TIMER_NONMONO[hart].store(snap.non_monotonic, Ordering::SeqCst);
    // Slot phải là **của chính hart này**: `first` = `rdtime` lúc hart này arm, `armed` vẫn bật,
    // `ticks` đủ 100 ⇒ handler timer trên hart này đã cộng slot này (không lẫn hart khác).
    let own_slot = snap.first == arm.now && snap.armed && snap.ticks >= 100;
    if ok && own_slot && snap.non_monotonic == 0 {
        TIMER_OK.insert(hart);
    }
}

/// Mỗi hart khởi tạo PLIC **context của mình** (`2*hart+1`) và ghi lại bằng chứng.
fn plic_phase(hart: usize) {
    let report = plic::init_hart();
    PLIC_CTX[hart].store(report.context as u64, Ordering::SeqCst);
    PLIC_MCTX[hart].store(report.machine_context as u64, Ordering::SeqCst);
    PLIC_THRESHOLD[hart].store(report.threshold as u64, Ordering::SeqCst);
    PLIC_PRIORITY[hart].store(report.priority_uart as u64, Ordering::SeqCst);
    let flags = report.enable_on as u64
        | ((report.enable_off as u64) << 1)
        | ((!report.enabled_mmode as u64) << 2);
    PLIC_FLAGS[hart].store(flags, Ordering::SeqCst);
    if report.is_configured() {
        PLIC_OK.insert(hart);
    }
}

/// Hart thứ cấp: park sau khi xong — không in, không giữ khoá.
fn park_hart() -> ! {
    vifast_plat_riscv64::park_forever()
}

/// Entry hart **thứ cấp** (SBI HSM `hart_start` gọi `_start_secondary` với `a0 = hartid`,
/// `a1 = opaque`). Adopt `satp` của BSP, cài `stvec`, báo online, rồi chạy đúng chuỗi pha SMP mà
/// BSP điều phối; mọi bước đều có timeout để hart lỗi **không treo** cả máy.
#[no_mangle]
pub extern "C" fn rust_secondary_main(hart: usize, opaque: usize) -> ! {
    if !smp::secondary_setup(opaque) {
        park_hart()
    }
    if !smp::wait_release(smp::ONLINE_TIMEOUT_TICKS) {
        park_hart()
    }
    alive_phase();
    let _ = ap_barrier(ROUND_ALIVE, smp::BARRIER_TIMEOUT_TICKS);
    wait_guard_turn(hart);
    let _ = ap_barrier(ROUND_GUARD, smp::BARRIER_TIMEOUT_TICKS);
    timer_phase(hart);
    let _ = ap_barrier(ROUND_TIMER, smp::BARRIER_TIMEOUT_TICKS);
    plic_phase(hart);
    let _ = ap_barrier(ROUND_PLIC, smp::BARRIER_TIMEOUT_TICKS);
    if hart == SLOW_HART {
        smp::delay_ticks(SLOW_DELAY_TICKS);
        SLOW_DELAYED.store(1, Ordering::SeqCst);
    }
    ARRIVE_TIME[hart].store(now_ticks_test(), Ordering::SeqCst);
    let _ = ap_barrier(ROUND_BARRIER, smp::BARRIER_TIMEOUT_TICKS);
    let _ = ap_barrier(ROUND_TIMEOUT, smp::BARRIER_TIMEOUT_TICKS);
    park_hart()
}

/// Pha SMP trên **BSP**: bring-up SBI HSM + 5 TAP (15..19). Trả số fail (0 = xanh).
pub fn smp_phase() -> usize {
    let mut fails = 0usize;
    let boot = PLATFORM.hart_id();
    // --- Bring-up: hart 0 đang chạy; probe + HSM start hart 1..N-1 (hart ngoài DTB bị từ chối).
    let report = smp::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# smp: boot={} harts={} present=0x{:x} online=0x{:x} started=0x{:x} absent=0x{:x} rejected={} timeout={} dtb=0x{:x} dtb_magic={} base=0x{:x} stride=0x{:x} area=0x{:x}",
            report.boot_hart,
            report.harts,
            report.present_mask,
            report.online_mask,
            report.started_mask,
            report.absent_mask,
            mask_count(report.absent_mask),
            report.timed_out as u8,
            report.dtb,
            report.dtb_magic_ok as u8,
            report.hart_area_base,
            report.hart_stride,
            smp::hart_area_bytes()
        );
    }
    // Log từng hart bị từ chối (hart vượt DTB) và từng hart không lên online.
    for hart in 0..MAX_HARTS {
        if report.absent_mask & (1u64 << hart) != 0 {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# smp: hart {} status={} ngoai DTB — tu choi start (khong panic ca may)",
                hart, report.status[hart]
            );
        } else if report.present_mask & (1u64 << hart) != 0
            && report.online_mask & (1u64 << hart) == 0
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# smp: hart {} KHONG online status={} start_code={}",
                hart, report.status[hart], report.start_code[hart]
            );
        } else if report.status[hart] != -99 && report.status[hart] < 0 {
            // Lỗi SBI khác `-3` (vd `SBI_ERR_FAILED = -1`): log rồi bỏ hart, **không panic cả máy**
            // — yêu cầu KT-49 "hart lỗi ⇒ log".
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# smp: hart {} status={} loi SBI — bo qua (khong panic ca may)",
                hart, report.status[hart]
            );
        }
    }
    // Cho hart thứ cấp ra khỏi park; mọi hart (kể cả BSP) in dòng alive + lấy ticket.
    smp::release();
    alive_phase();
    let online = report.online_mask;
    let (arrived, timed_out, _) = bsp_barrier(ROUND_ALIVE, online, smp::BARRIER_TIMEOUT_TICKS);
    // --- TAP 15: N hart alive, ticket là hoán vị 0..N-1, mỗi hart in đúng id của mình.
    let n = mask_count(online) as usize;
    let mut tickets_ok = true;
    let mut seen = 0u64;
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) == 0 {
            continue;
        }
        let ticket = smp::ticket_of(hart);
        if ticket >= MAX_HARTS as u64 || seen & (1u64 << ticket) != 0 {
            tickets_ok = false;
        } else {
            seen |= 1u64 << ticket;
        }
    }
    let tickets_perm = tickets_ok && seen == (1u64 << n) - 1 && n > 0;
    let alive_ok = report.is_online()
        && report.dtb_magic_ok
        && n == mask_count(report.present_mask) as usize
        && ALIVE.bits() == online
        && !timed_out
        && arrived & online == online
        && tickets_perm
        && (report.absent_mask & report.present_mask == 0);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# harts alive: n={} mask=0x{:x} alive=0x{:x} tickets={} arrived=0x{:x} timeout={}",
            n,
            online,
            ALIVE.bits(),
            tickets_perm as u8,
            arrived,
            timed_out as u8
        );
    }
    if alive_ok {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(w, "ok 15 - harts alive {}", n);
    } else {
        PLATFORM.put_str("not ok 15 - harts alive: thieu hart/ticket khong phai hoan vi\n");
        fails += 1;
    }
    if PLATFORM.hart_id() != boot || boot != report.boot_hart {
        fails += 1;
    }

    // --- TAP 16: stack + guard riêng mỗi hart; mỗi hart tự chạm guard của mình (fault đúng hart).
    bsp_guard_round(online);
    let (guard_arrived, guard_timeout, _) =
        bsp_barrier(ROUND_GUARD, online, smp::BARRIER_TIMEOUT_TICKS);
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) == 0 {
            continue;
        }
        let (stack_bottom, stack_top) = mmu::stack_range_of(hart);
        let (trap_bottom, trap_top) = mmu::trap_stack_range_of(hart);
        let (guard_start, guard_end) = mmu::guard_range_of(hart);
        let frame = smp::last_trap_frame(hart);
        let frame_trap = frame >= trap_bottom && frame < trap_top;
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# stack hart={} stack=0x{:x}-0x{:x} trap=0x{:x}-0x{:x} guard=0x{:x}-0x{:x} owner={} frame=0x{:x} frame_trap={} rw_nx={} guard_none={}",
            hart,
            stack_bottom,
            stack_top,
            trap_bottom,
            trap_top,
            guard_start,
            guard_end,
            mmu::guard_owner(guard_start).unwrap_or(usize::MAX),
            frame,
            frame_trap as u8,
            STACK_OK.contains(hart) as u8,
            GUARD_OK.contains(hart) as u8
        );
    }
    let mut distinct = [0u64; MAX_HARTS];
    let mut distinct_len = 0usize;
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) != 0 {
            distinct[distinct_len] = mmu::guard_range_of(hart).0;
            distinct_len += 1;
        }
    }
    let mut guards_distinct = true;
    for index in 0..distinct_len {
        for other in index + 1..distinct_len {
            if distinct[index] == distinct[other] {
                guards_distinct = false;
            }
        }
    }
    let stack_ok = STACK_OK.bits() & online == online
        && GUARD_OK.bits() & online == online
        && guard_arrived & online == online
        && !guard_timeout
        && guards_distinct;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# per-hart stack/guard: stack_ok=0x{:x} guard_ok=0x{:x} distinct={} owner_ok={}",
            STACK_OK.bits() & online,
            GUARD_OK.bits() & online,
            guards_distinct as u8,
            GUARD_OK.bits() & online == online
        );
    }
    if stack_ok {
        PLATFORM.put_str(
            "ok 16 - per-hart stack/guard
",
        );
    } else {
        PLATFORM.put_str("not ok 16 - per-hart stack/guard: stack/guard khong dung hart\n");
        fails += 1;
    }

    // --- TAP 17: mỗi hart 100 tick trong slot PerHart của chính nó (không lẫn).
    timer_phase(boot);
    let (timer_arrived, timer_timeout, _) =
        bsp_barrier(ROUND_TIMER, online, smp::BARRIER_TIMEOUT_TICKS);
    let mut timer_ok = timer_arrived & online == online && !timer_timeout;
    let mut total_ticks = 0u64;
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) == 0 {
            continue;
        }
        let ticks = TIMER_TICKS[hart].load(Ordering::SeqCst);
        total_ticks += ticks;
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer hart={} ticks={} first={} late={} missed={} non_monotonic={} ok={}",
            hart,
            ticks,
            TIMER_FIRST[hart].load(Ordering::SeqCst),
            TIMER_LATE[hart].load(Ordering::SeqCst),
            TIMER_MISSED[hart].load(Ordering::SeqCst),
            TIMER_NONMONO[hart].load(Ordering::SeqCst),
            TIMER_OK.contains(hart) as u8
        );
        if !TIMER_OK.contains(hart) || ticks < 100 {
            timer_ok = false;
        }
    }
    // Slot PerHart của BSP đã bị `timer_phase` reset/arm lại ⇒ số tick trước đó không làm bẩn
    // hart khác; tổng tick mỗi hart ≥ 100 là bằng chứng "không lẫn".
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# per-hart timer: ok=0x{:x} total_ticks={} arrived=0x{:x} timeout={}",
            TIMER_OK.bits() & online,
            total_ticks,
            timer_arrived,
            timer_timeout as u8
        );
    }
    if timer_ok {
        PLATFORM.put_str(
            "ok 17 - per-hart timer
",
        );
    } else {
        PLATFORM.put_str("not ok 17 - per-hart timer: thieu tick/khong dung slot\n");
        fails += 1;
    }

    // --- TAP 18: PLIC context `2*h+1` theo từng hart; M-mode `2*h` không bật; bộ đếm không lẫn.
    plic_phase(boot);
    let (plic_arrived, plic_timeout, _) =
        bsp_barrier(ROUND_PLIC, online, smp::BARRIER_TIMEOUT_TICKS);
    let mut plic_ok = plic_arrived & online == online && !plic_timeout;
    let mut contexts_distinct = true;
    for hart in 0..MAX_HARTS {
        if online & (1u64 << hart) == 0 {
            continue;
        }
        let ctx = PLIC_CTX[hart].load(Ordering::SeqCst);
        let mctx = PLIC_MCTX[hart].load(Ordering::SeqCst);
        let expected_ctx = (hart as u64) * 2 + 1;
        let expected_mctx = (hart as u64) * 2;
        let flags = PLIC_FLAGS[hart].load(Ordering::SeqCst);
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# plic hart={} ctx={} mmode_ctx={} threshold={} priority_uart={} enable_on={} enable_off={} mmode_off={} ok={}",
            hart,
            ctx,
            mctx,
            PLIC_THRESHOLD[hart].load(Ordering::SeqCst),
            PLIC_PRIORITY[hart].load(Ordering::SeqCst),
            flags & 1,
            (flags >> 1) & 1,
            (flags >> 2) & 1,
            PLIC_OK.contains(hart) as u8
        );
        if ctx != expected_ctx || mctx != expected_mctx || !PLIC_OK.contains(hart) {
            plic_ok = false;
        }
        for other in hart + 1..MAX_HARTS {
            if online & (1u64 << other) != 0 && PLIC_CTX[other].load(Ordering::SeqCst) == ctx {
                contexts_distinct = false;
            }
        }
    }
    // Bộ đếm PerHart: hart **boot** (hart chạy KT-48) giữ 8 IRQ UART; các hart khác **không** claim
    // external nào. Dùng `boot` chứ không hardcode hart 0 — OpenSBI có thể chọn boot hart khác 0
    // (đo được ở ảnh sản phẩm `-smp 4` không icount: `Domain0 Boot HART: 2`).
    let uart_boot = PLIC_STATS.get(boot).snapshot().uart_irqs;
    let mut plic_stats_clean = uart_boot == UART_IRQ_LIMIT;
    for hart in 0..MAX_HARTS {
        if hart != boot
            && online & (1u64 << hart) != 0
            && PLIC_STATS.get(hart).snapshot().traps != 0
        {
            plic_stats_clean = false;
        }
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# per-hart plic: ok=0x{:x} distinct={} uart_irqs_boot={} stats_clean={} arrived=0x{:x} timeout={}",
            PLIC_OK.bits() & online,
            contexts_distinct as u8,
            uart_boot,
            plic_stats_clean as u8,
            plic_arrived,
            plic_timeout as u8
        );
    }
    if plic_ok && contexts_distinct && plic_stats_clean {
        PLATFORM.put_str(
            "ok 18 - plic ctx per hart
",
        );
    } else {
        PLATFORM.put_str("not ok 18 - plic ctx per hart: context/PerHart khong dung\n");
        fails += 1;
    }

    // --- TAP 19: barrier (hart chậm vẫn qua) + vòng có hart **không tồn tại** timeout, không treo.
    ARRIVE_TIME[boot].store(now_ticks_test(), Ordering::SeqCst);
    let (arrived5, timed5, wait5) = bsp_barrier(ROUND_BARRIER, online, smp::BARRIER_TIMEOUT_TICKS);
    BARRIER5_ARRIVED.store(arrived5, Ordering::SeqCst);
    BARRIER5_MISSING.store(online & !arrived5, Ordering::SeqCst);
    BARRIER5_TIMEOUT.store(timed5 as u64, Ordering::SeqCst);
    BARRIER5_WAIT.store(wait5, Ordering::SeqCst);
    // Vòng 6: kỳ vọng gồm một bit hart **không online** ⇒ barrier phải timeout ngắn rồi đi tiếp.
    let absent_bit = {
        let mut mask = 0u64;
        for hart in 0..MAX_HARTS {
            if online & (1u64 << hart) == 0 {
                mask = 1u64 << hart;
                break;
            }
        }
        mask
    };
    let expected6 = online | absent_bit;
    // 200 ms (20 tick) — đủ để mọi hart online kịp join kể cả khi QEMU icount chia lượt vCPU
    // thô (đo được: hart chậm có thể tới muộn ~100 ms ảo), vẫn là timeout **ngắn có biên** so với
    // 1 s của barrier thường; hart "không tồn tại" thì không bao giờ tới, BSP phải đi tiếp.
    let timeout6_ticks = TICK_TICKS * 20;
    smp::barrier_begin(ROUND_TIMEOUT, expected6);
    let start6 = now_ticks_test();
    let _ = smp::barrier_join(ROUND_TIMEOUT, smp::BARRIER_TIMEOUT_TICKS);
    let (arrived6, timed6) = smp::barrier_wait_arrivals(timeout6_ticks);
    smp::barrier_release(ROUND_TIMEOUT);
    let wait6 = now_ticks_test().wrapping_sub(start6);
    BARRIER6_ARRIVED.store(arrived6, Ordering::SeqCst);
    BARRIER6_MISSING.store(expected6 & !arrived6, Ordering::SeqCst);
    BARRIER6_TIMEOUT.store(timed6 as u64, Ordering::SeqCst);
    BARRIER6_WAIT.store(wait6, Ordering::SeqCst);
    BARRIER6_EXPECTED_MISSING.store(absent_bit, Ordering::SeqCst);
    let slow_ok = online & (1u64 << SLOW_HART) == 0 || SLOW_DELAYED.load(Ordering::SeqCst) == 1;
    let barrier_ok = arrived5 & online == online
        && !timed5
        && wait5 < TIMEBASE_HZ
        && slow_ok
        && timed6
        && BARRIER6_MISSING.load(Ordering::SeqCst) == absent_bit
        && wait6 < TIMEBASE_HZ;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# barrier: round=5 arrived=0x{:x} missing=0x{:x} timeout={} wait_ticks={} slow_hart={} slow_delayed={}",
            arrived5,
            online & !arrived5,
            timed5 as u8,
            wait5,
            SLOW_HART,
            SLOW_DELAYED.load(Ordering::SeqCst)
        );
        let _ = writeln!(
            w,
            "# barrier timeout: round=6 expected_missing=0x{:x} arrived=0x{:x} missing=0x{:x} timeout={} wait_ticks={} (may khong treo)",
            absent_bit,
            arrived6,
            BARRIER6_MISSING.load(Ordering::SeqCst),
            timed6 as u8,
            wait6
        );
    }
    if barrier_ok {
        PLATFORM.put_str("ok 19 - barrier\n");
    } else {
        PLATFORM.put_str("not ok 19 - barrier: thieu arrival/timeout khong dung\n");
        fails += 1;
    }

    smp::finish();
    fails
}
