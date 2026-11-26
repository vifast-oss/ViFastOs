// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-413c** — "ai chạy tiếp" của `x86_64` do **`Machine::dispatch(hart)`** quyết định trên
//! **TCB thật** của `vifast-core`, không còn `TASK_SEQUENCE`/hàng đợi tự viết.
//!
//! # Vì sao thay (D2-261)
//!
//! `KT-286..KT-318` dựng một scheduler **song song** trong ảnh: bốn stack 16 KiB, bảng
//! `TASK_SEQUENCE` cố định, hàng đợi `MH_*` + khoá tự viết, `prio_pick`/`sleep_ok` thuần. Nó
//! **đo thật** nhưng là **mô hình thứ hai** của cùng một câu hỏi mà `vifast-core` đã trả lời
//! (`Machine::dispatch`, priority bucket, `block_current`/`notification_signal`, `published`,
//! `check_sched`). Hai mô hình ⇒ hai nguồn sự thật. Task này bỏ mô hình thứ hai: TCB, priority,
//! hàng chờ, ngủ/thức, bất biến đều của **core**; phần còn lại của tệp này chỉ là **kịch bản
//! đo** gọi API công khai của core rồi in bằng chứng.
//!
//! # Cái **mất** và vì sao chấp nhận
//!
//! Bản cũ chuyển ngữ cảnh **phần cứng** (`iretq`, timer preempt cả tác vụ ring 3). Bản này
//! **không** chuyển ngữ cảnh: nó đo **quyết định** của scheduler core (chọn TCB nào, priority,
//! chặn/đánh thức, `published`, bất biến) — đúng phạm vi KT-413c ("thay 'ai chạy tiếp' bằng
//! `Machine::dispatch`"). Vì vậy **không** còn khẳng định "preempt ring 3" trong bằng chứng:
//! TAP 24 đổi nghĩa thành **block/wake trên TCB thật** (`BlockedOnNotification` → `Ready` →
//! được chọn lại). Đây là **hợp đồng mới** đã được `D2-261` duyệt; TAP 22/23/25/29/30 giữ số
//! hiệu, `expected.txt` **không** đổi (vẫn 50 TAP).
//!
//! # Bằng chứng in ra (định dạng D2-261)
//!
//! ```text
//! # sched: tcb=t<ObjId> hart=0 prio=<p> state=Running n=<tick>   (8 dòng, mỗi lượt dispatch)
//! # sched: block tcb=t<ObjId> hart=0 state=BlockedOnNotification
//! # sched: wake tcb=t<ObjId> hart=0 state=Ready
//! # sched: ticks=8 tcbs=4 preempts=<n> wakes=4 viol=0 sleep_viol=0
//! # sched: progress picks=8 distinct=4 delta=8
//! # sched: hart=0 picked=t<ObjId> n=<k>
//! # sched: prio first=t<ObjId> viol=0 ticks=8 picks=[<a>, <b>, <c>, <u>]
//! # sched: sleep wakes=4 viol=0 ticks=8
//! ```
//!
//! Dòng **nhiều hart** của ảnh là `# core smp: …` do `smp_suite::run_x64` in (KT-413b) — tệp
//! này **không** in bản thứ hai (D2-261 mục #8). `CR4.PCIDE` vẫn **mặc định TẮT** (`pcid.rs`).
//!
//! # Vì sao dựng lại `Machine`
//!
//! `coresmp::run` (TAP 48/49) đã để lại 5 TCB mức 1..5 trong model; chen 4 TCB mới vào cùng
//! hàng chờ sẽ làm lượt chọn đầu **không** xác định (TCB mức 5 cũ luôn thắng). Vì vậy pha này
//! gọi `core_bridge::init` với một Untyped **mới** từ arena (đúng khuôn `coresmp`) rồi dựng
//! đúng **bốn** TCB — mọi giá trị sau đó là **hàm thuần của kịch bản**, không phụ thuộc lịch sử
//! model hay thời điểm host. Bằng chứng `# core smp:` của pha trước vẫn nằm nguyên trong log
//! (cùng lần boot) và là dòng nhiều hart mà runner kiểm.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_x86_64::bootmem::BootArena;
use vifast_plat_x86_64::core_bridge;
use vifast_plat_x86_64::TableMemory;

use crate::{check, Console};

/// Số frame 4 KiB cho Untyped của pha này — dựng 4 TCB + 4 SchedContext + 1 SchedControl +
/// 4 Notification + CNode gốc (nhỏ hơn nhiều so với 16 KiB; dư để retype fail-closed).
const ARENA_FRAMES: usize = 4;

/// Số TCB của kịch bản — **bốn** (A, B, C, U) như bản cũ, nhưng là TCB thật của core.
const TCBS: usize = 4;

/// Priority **tĩnh** theo chỉ số TCB: `A=1, B=3, C=2, U=0`. B cao nhất ⇒ lượt chọn đầu **bắt
/// buộc** là B (TAP 29); hoà **không** xảy ra nên FIFO không che mất priority.
const PRIO: [u8; TCBS] = [1, 3, 2, 0];

/// Số lượt `dispatch` của kịch bản (TAP 22 đòi đúng **8** lượt, tổng picks = 8).
const TICKS: u64 = 8;

/// Số lần chặn/đánh thức ở cuối kịch bản (mỗi TCB đúng một lần).
const WAKE_TARGET: u64 = 4;

/// Số hart của mô hình phải đo được (`-smp 4`) — TAP 25 fail-closed nếu khác.
const HARTS: u16 = 4;

/// Kết quả **đo** của kịch bản — in bằng chứng + điều kiện TAP.
#[derive(Clone, Copy, Debug)]
struct Report {
    /// Kịch bản chạy hết **không** lỗi core.
    ok: bool,
    /// `ObjId` của bốn TCB theo chỉ số [`PRIO`].
    ids: [ObjId; TCBS],
    /// TCB được chọn ở lượt đầu.
    first: ObjId,
    /// Chuỗi TCB được chọn, mỗi lượt dispatch một phần tử.
    seq: [ObjId; TICKS as usize],
    /// Số lượt được chọn của từng TCB (chỉ số như [`PRIO`]).
    picks: [u64; TCBS],
    /// Số lần `dispatch` **thay** một TCB đang `Running` (preempt thật của core).
    preempts: u64,
    /// Số TCB đã `BlockedOnNotification` → `Ready` ở bước cuối.
    wakes: u64,
    /// Số lượt chọn **trái** priority kỳ vọng (phải 0).
    viol: u64,
    /// Số lượt chọn một TCB **đang chặn** (phải 0).
    sleep_viol: u64,
    /// Số TCB **khác nhau** đã được chọn.
    distinct: u64,
    /// `BlockedOnNotification` quan sát được trên TCB thật.
    blocked: bool,
    /// `dispatch` kế **bỏ qua** TCB đang chặn.
    skipped: bool,
    /// `notification_signal` đưa TCB về `Ready`.
    woke: bool,
    /// `dispatch` **chọn lại** TCB vừa thức.
    repicked: bool,
    /// `Machine::hart_count()` đo được.
    harts: u16,
    /// `published(h)` khớp `sched` cho **mọi** hart (luật 3 §4.16).
    published_ok: bool,
    /// Bốn bất biến `check_sched`/`check_sc_refs`/`check_budget`/`check_mailboxes`.
    invariants_ok: bool,
    /// Bốn TCB đều `Ready` sau bước đánh thức cuối.
    all_ready: bool,
}

impl Report {
    /// Báo cáo rỗng (fail-closed: mọi TAP đỏ, không panic).
    const fn failed() -> Self {
        Self {
            ok: false,
            ids: [0; TCBS],
            first: 0,
            seq: [0; TICKS as usize],
            picks: [0; TCBS],
            preempts: 0,
            wakes: 0,
            viol: 0,
            sleep_viol: 0,
            distinct: 0,
            blocked: false,
            skipped: false,
            woke: false,
            repicked: false,
            harts: 0,
            published_ok: false,
            invariants_ok: false,
            all_ready: false,
        }
    }

    /// Tổng số lượt chọn — phải bằng [`TICKS`].
    fn picks_total(&self) -> u64 {
        self.picks.iter().sum()
    }
}

/// Chỉ số của `id` trong bảng TCB; `None` ⇔ không phải TCB của kịch bản.
fn index_of(ids: &[ObjId; TCBS], id: ObjId) -> Option<usize> {
    let mut i = 0;
    while i < TCBS {
        if ids[i] == id {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Priority đọc từ bảng [`PRIO`] theo `ObjId` (`0` khi không khớp — không được xảy ra).
fn prio_of(ids: &[ObjId; TCBS], id: ObjId) -> u8 {
    match index_of(ids, id) {
        Some(i) => PRIO[i],
        None => 0,
    }
}

/// `true` ⇔ mọi TCB của kịch bản đang `Ready`.
fn all_ready(m: &Machine, ids: &[ObjId; TCBS]) -> bool {
    let mut i = 0;
    while i < TCBS {
        if m.tcb_state(ids[i]) != Ok(TcbState::Ready) {
            return false;
        }
        i += 1;
    }
    true
}

/// Chỉ số TCB mà `dispatch` **phải** chọn: priority cao nhất trong các TCB `Ready` **và**
/// TCB đang `Running` (vì `dispatch` đưa `Running` về hàng trước khi chọn). `None` ⇔ hàng rỗng.
///
/// Đây là **kỳ vọng độc lập** với `dispatch` — nếu `dispatch` chọn khác thì `viol` tăng và
/// TAP 29 đỏ; nhờ vậy phép kiểm priority **không** phải "đọc lại chính scheduler".
fn expected_pick(m: &Machine, ids: &[ObjId; TCBS], cur: Option<ObjId>) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut i = 0;
    while i < TCBS {
        let state = m.tcb_state(ids[i]).ok()?;
        let candidate = state == TcbState::Ready
            || (matches!(state, TcbState::Running { .. }) && cur == Some(ids[i]));
        if candidate {
            match best {
                Some(b) if PRIO[b] >= PRIO[i] => {}
                _ => best = Some(i),
            }
        }
        i += 1;
    }
    best
}

/// Một lượt `dispatch(0)` **có kiểm**: đọc trạng thái **trước**, gọi core, ghi số đo.
///
/// Trả `ObjId` được chọn; lỗi core ⇒ `KError` (caller fail-closed, **không** panic).
fn dispatch_tick(m: &mut Machine, rep: &mut Report) -> Result<ObjId, KError> {
    let cur = m.current(0);
    let cur_running = match cur {
        Some(t) => matches!(m.tcb_state(t)?, TcbState::Running { .. }),
        None => false,
    };
    let expected = expected_pick(m, &rep.ids, cur);
    // TCB đang chặn — `dispatch` **không** được chọn; đếm vi phạm như một bất biến của kịch bản.
    let mut blocked = [false; TCBS];
    for i in 0..TCBS {
        blocked[i] = !matches!(
            m.tcb_state(rep.ids[i])?,
            TcbState::Ready | TcbState::Running { .. }
        );
    }
    let picked = m.dispatch(0)?.ok_or(KError::IllegalOperation)?;
    let tick = rep.picks_total() as usize;
    if tick < TICKS as usize {
        rep.seq[tick] = picked;
    }
    if let Some(i) = index_of(&rep.ids, picked) {
        rep.picks[i] += 1;
        if blocked[i] {
            rep.sleep_viol += 1;
        }
    }
    if expected.map(|i| rep.ids[i]) != Some(picked) {
        rep.viol += 1;
    }
    if cur_running {
        rep.preempts += 1;
    }
    Ok(picked)
}

/// Thân kịch bản — chạy **trong** khoá của [`core_bridge`] (không lồng khoá, không in).
fn run_locked(m: &mut Machine, untyped: ObjId) -> Result<Report, KError> {
    let mut rep = Report::failed();

    // ---- 1. CNode gốc + 4 TCB + 4 SchedContext + 1 SchedControl + 4 Notification.
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(untyped);
    m.put_cap(root, 0, uc)?;
    let mut tcb_slot = [SlotRef::new(root, 0); TCBS];
    for (i, slot) in tcb_slot.iter_mut().enumerate() {
        let cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, 1 + i, cap)?;
        *slot = SlotRef::new(root, 1 + i);
        rep.ids[i] = m.cap_obj(cap)?;
    }
    let mut sc_slot = [SlotRef::new(root, 0); TCBS];
    for (i, slot) in sc_slot.iter_mut().enumerate() {
        let cap = m.retype(us, ObjType::SchedContext, 0, 0)?;
        let n = 1 + TCBS + i;
        m.put_cap(root, n, cap)?;
        *slot = SlotRef::new(root, n);
    }
    let ctl_cap = m.retype(us, ObjType::SchedControl, 0, 0)?;
    let ctl_n = 1 + 2 * TCBS;
    m.put_cap(root, ctl_n, ctl_cap)?;
    let ctl = SlotRef::new(root, ctl_n);
    let mut ntfn = [0u32; TCBS];
    for (i, id) in ntfn.iter_mut().enumerate() {
        let cap = m.retype(us, ObjType::Notification, 0, 0)?;
        let n = 2 + 2 * TCBS + i;
        m.put_cap(root, n, cap)?;
        *id = m.cap_obj(cap)?;
    }

    // ---- 2. Cấu hình SC + priority + affinity + home rồi `start` (Ready).
    // Thứ tự **bắt buộc**: `SetAffinity` trước `SetHome` (home phải nằm trong affinity).
    for i in 0..TCBS {
        m.sched_control_configure(ctl, sc_slot[i], 8, 16)?;
        m.set_sched_params(tcb_slot[i], Some(sc_slot[i]))?;
        m.tcb_set_priority(tcb_slot[i], PRIO[i], 10)?;
        m.tcb_set_affinity(tcb_slot[i], 0xF)?;
        m.tcb_set_home(tcb_slot[i], 0)?;
        m.start_tcb(rep.ids[i])?;
    }

    // ---- 3. Tám lượt dispatch: priority (B), preempt, chặn/bỏ qua/đánh thức/chọn lại, rồi
    //         A và U để cả bốn TCB đều được chọn. Mọi bước là **hàm thuần của kịch bản**:
    //         không timer, không host deschedule ⇒ kết quả không phụ thuộc thời điểm.
    let b = 1usize;
    let c = 2usize;
    let a = 0usize;
    let u = 3usize;
    rep.first = dispatch_tick(m, &mut rep)?; // t1: B (priority cao nhất)
    dispatch_tick(m, &mut rep)?; //            t2: B — `dispatch` preempt `Running` rồi chọn lại
    m.notification_wait(rep.ids[b], ntfn[b])?; // B chặn trên notification **thật** của core
    rep.blocked = m.tcb_state(rep.ids[b])? == TcbState::BlockedOnNotification { ntfn: ntfn[b] };
    let skip = dispatch_tick(m, &mut rep)?; //   t3: C — B đang chặn nên bị bỏ qua
    rep.skipped = skip != rep.ids[b];
    dispatch_tick(m, &mut rep)?; //             t4: C — preempt rồi chọn lại
    m.notification_signal(ntfn[b], 0x42)?; //   đánh thức B (đường sản phẩm, không `wake()` test)
    rep.woke = m.tcb_state(rep.ids[b])? == TcbState::Ready;
    let back = dispatch_tick(m, &mut rep)?; //   t5: B — preempt C, chọn lại TCB vừa thức
    rep.repicked = back == rep.ids[b];
    m.notification_wait(rep.ids[b], ntfn[b])?; // B chặn lại
    dispatch_tick(m, &mut rep)?; //             t6: C
    m.notification_wait(rep.ids[c], ntfn[c])?; // C chặn
    dispatch_tick(m, &mut rep)?; //             t7: A
    m.notification_wait(rep.ids[a], ntfn[a])?; // A chặn
    dispatch_tick(m, &mut rep)?; //             t8: U
    m.notification_wait(rep.ids[u], ntfn[u])?; // U chặn
                                               // ---- 4. Đánh thức **cả bốn**: mỗi TCB một Notification riêng (`Wait` thứ hai trên cùng
                                               //         một Notification là `Busy` — đúng luật core), rồi đếm số TCB về `Ready`.
    let mut wakes = 0u64;
    for i in 0..TCBS {
        m.notification_signal(ntfn[i], 1)?;
        if m.tcb_state(rep.ids[i])? == TcbState::Ready {
            wakes += 1;
        }
    }
    rep.wakes = wakes;
    rep.all_ready = all_ready(m, &rep.ids);

    // ---- 5. Bất biến + `published` của **mọi** hart (luật 3 §4.16) — đo trên model.
    rep.harts = m.hart_count();
    let mut published_ok = rep.harts == HARTS;
    for h in 0..rep.harts {
        let Ok(p) = m.published(h) else {
            published_ok = false;
            continue;
        };
        if p.idle != m.sched_idle(h) || p.cur_prio != m.sched_cur_prio(h) {
            published_ok = false;
        }
    }
    rep.published_ok = published_ok;
    rep.invariants_ok = m.check_sched().is_ok()
        && m.check_sc_refs().is_ok()
        && m.check_budget().is_ok()
        && m.check_mailboxes().is_ok();
    rep.distinct = rep.picks.iter().filter(|n| **n > 0).count() as u64;
    rep.ok = true;
    Ok(rep)
}

/// Bản chụp cho [`report`] — `run` ghi, `report` (gọi sau TAP 28) đọc.
static DONE: AtomicBool = AtomicBool::new(false);
static FIRST: AtomicU32 = AtomicU32::new(0);
static IDS: [AtomicU32; TCBS] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static PICKS: [AtomicU64; TCBS] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
static PREEMPTS: AtomicU64 = AtomicU64::new(0);
static WAKES_MEASURED: AtomicU64 = AtomicU64::new(0);
static VIOL: AtomicU64 = AtomicU64::new(0);
static SLEEP_VIOL: AtomicU64 = AtomicU64::new(0);

/// Chạy pha scheduler: dựng **Machine mới** với Untyped riêng rồi đo kịch bản dispatch.
///
/// Gọi từ [`crate::net::run`] tại chỗ cũ của `sched::run` (TAP 22..25), **sau** `coresmp`/TAP
/// 48/49 và **trước** `blk`/`com`/`net` DHCP. `tap_first` = 22 (TAP 22..25).
pub fn run(console: &mut Console, tap_first: usize) {
    let Some(mut arena) = BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) else {
        let _ = writeln!(console, "PANIC: dispatch: arena không hợp lệ");
        crate::asm::halt(3)
    };
    let mut base = 0u64;
    for index in 0..ARENA_FRAMES {
        let Some(pa) = arena.alloc_frame() else {
            let _ = writeln!(
                console,
                "PANIC: dispatch: arena hết frame (đã dùng {} / {})",
                arena.used(),
                arena.frames()
            );
            crate::asm::halt(3)
        };
        if index == 0 {
            base = pa;
        }
    }
    let report = core_bridge::init(
        base,
        ARENA_FRAMES * 0x1000,
        crate::smp::online_count().max(1),
    );
    let rep = if report.healthy() {
        core_bridge::with_machine(|m| run_locked(m, report.untyped))
            .unwrap_or_else(|_| Report::failed())
    } else {
        Report::failed()
    };

    // ---- Ghi bản chụp cho `report` (TAP 29/30 in sau TAP 28, đúng thứ tự log).
    if rep.ok {
        DONE.store(true, Ordering::Release);
        FIRST.store(rep.first, Ordering::Release);
        for i in 0..TCBS {
            IDS[i].store(rep.ids[i], Ordering::Release);
            PICKS[i].store(rep.picks[i], Ordering::Release);
        }
        TICK_COUNT.store(rep.picks_total(), Ordering::Release);
        PREEMPTS.store(rep.preempts, Ordering::Release);
        WAKES_MEASURED.store(rep.wakes, Ordering::Release);
        VIOL.store(rep.viol, Ordering::Release);
        SLEEP_VIOL.store(rep.sleep_viol, Ordering::Release);
    }

    // ---- Bằng chứng: mỗi lượt dispatch một dòng `tcb=… hart=… prio=… state=Running`.
    for (i, id) in rep.seq.iter().enumerate() {
        let state = if rep.ok { "Running" } else { "None" };
        let _ = writeln!(
            console,
            "# sched: tcb=t{id} hart=0 prio={} state={state} n={}",
            prio_of(&rep.ids, *id),
            i + 1
        );
    }
    if rep.blocked {
        let _ = writeln!(
            console,
            "# sched: block tcb=t{} hart=0 state=BlockedOnNotification",
            rep.ids[1]
        );
    }
    if rep.woke {
        let _ = writeln!(
            console,
            "# sched: wake tcb=t{} hart=0 state=Ready",
            rep.ids[1]
        );
    }
    let _ = writeln!(
        console,
        "# sched: ticks={} tcbs={TCBS} preempts={} wakes={} viol={} sleep_viol={}",
        rep.picks_total(),
        rep.preempts,
        rep.wakes,
        rep.viol,
        rep.sleep_viol
    );
    let _ = writeln!(
        console,
        "# sched: progress picks={} distinct={} delta={}",
        rep.picks_total(),
        rep.distinct,
        rep.picks_total()
    );
    let _ = writeln!(
        console,
        "# sched: hart=0 picked=t{} n={}",
        rep.first,
        rep.picks.get(1).copied().unwrap_or(0)
    );

    // ---- TAP 22..25 (số hiệu giữ nguyên hợp đồng runner; `expected.txt` không đổi).
    check(
        console,
        tap_first,
        "sched: two tasks ran",
        rep.ok && rep.distinct == TCBS as u64 && rep.picks_total() == TICKS,
    );
    check(
        console,
        tap_first + 1,
        "sched: preempted",
        rep.ok && rep.preempts >= 2 && rep.distinct >= 2,
    );
    check(
        console,
        tap_first + 2,
        "sched: block wake",
        rep.ok && rep.blocked && rep.skipped && rep.woke && rep.repicked,
    );
    check(
        console,
        tap_first + 3,
        "sched: multi-hart",
        rep.ok && rep.harts == HARTS && rep.published_ok && rep.invariants_ok,
    );
}

/// In bằng chứng **priority** + **sleep/wake** và phát TAP 29/30 (gọi sau TAP 28).
///
/// Pha không chạy lại ở đây — chỉ đọc bản chụp của [`run`] rồi kiểm **lại** trạng thái cuối
/// của bốn TCB trên model (`all_ready`), nên TAP 30 vẫn là phép đo, không phải đọc hằng.
pub fn report(console: &mut Console, tap_first: usize) {
    let done = DONE.load(Ordering::Acquire);
    let first = FIRST.load(Ordering::Acquire);
    let ticks = TICK_COUNT.load(Ordering::Acquire);
    let preempts = PREEMPTS.load(Ordering::Acquire);
    let wakes = WAKES_MEASURED.load(Ordering::Acquire);
    let viol = VIOL.load(Ordering::Acquire);
    let sleep_viol = SLEEP_VIOL.load(Ordering::Acquire);
    let picks = [
        PICKS[0].load(Ordering::Acquire),
        PICKS[1].load(Ordering::Acquire),
        PICKS[2].load(Ordering::Acquire),
        PICKS[3].load(Ordering::Acquire),
    ];
    let ids = [
        IDS[0].load(Ordering::Acquire),
        IDS[1].load(Ordering::Acquire),
        IDS[2].load(Ordering::Acquire),
        IDS[3].load(Ordering::Acquire),
    ];
    let _ = writeln!(
        console,
        "# sched: prio first=t{first} viol={viol} ticks={ticks} preempts={preempts} picks=[{}, {}, {}, {}]",
        picks[0], picks[1], picks[2], picks[3]
    );
    let _ = writeln!(
        console,
        "# sched: sleep wakes={wakes} viol={sleep_viol} ticks={ticks}"
    );
    // TAP 29: priority **tĩnh** — lượt đầu là B (3 > 2 > 1 > 0), **không** lượt nào trái kỳ
    // vọng độc lập [`expected_pick`], và đủ 8 lượt. Đột biến `mut-dispatch-no-prio` (bỏ xếp
    // bucket theo priority trong `dispatch_step`) chọn FIFO ⇒ `first` = A ⇒ ĐỎ.
    let priority_ok = done
        && ticks == TICKS
        && viol == 0
        && ids[1] != 0
        && first == ids[1]
        && picks.iter().all(|n| *n >= 1);
    check(console, tap_first, "sched: priority", priority_ok);
    // TAP 30: sleep/wake — **cả bốn** TCB chặn rồi thức (`wakes == 4`), **không** lượt chọn
    // nào rơi vào TCB đang chặn, và trạng thái cuối là `Ready` (đọc lại model, không tin hằng).
    let ready_now = if done {
        core_bridge::with_machine(|m| all_ready(m, &ids))
    } else {
        false
    };
    check(
        console,
        tap_first + 1,
        "sched: sleep wake",
        done && wakes == WAKE_TARGET && sleep_viol == 0 && ready_now,
    );
}
