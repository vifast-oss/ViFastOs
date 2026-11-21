// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Preemption x86_64** — KT-286 (K5' bước 8): hai tác vụ kernel + timer chuyển ngữ cảnh.
//!
//! # Đường đi
//!
//! ```text
//!   net::run (sau TAP 21 của SMP)
//!     └─ sched::run(console, 22)
//!          ├─ dựng frame khởi tạo cho A và B trên **stack riêng** của từng tác vụ
//!          ├─ `sti` → `hlt` (chờ tick PIT đầu tiên)
//!          ↓
//!   timer (vector 32) → exc_dispatch → sched::on_tick(frame_rsp)
//!          ├─ tick đầu: lưu `rsp` của **main**, chuyển sang A
//!          ├─ tick kế: lưu `rsp` tác vụ hiện tại, chọn tác vụ kế, nạp `rsp` tác vụ kế
//!          │    (stub chung đổi `rsp` rồi `pop`/`iretq` — xem `vifast_plat_x86_64::asm`)
//!          └─ sau `MAX_SWITCHES` lần: quay về **main**
//!          ↓
//!   main: in `# sched: ticks=<t> tasks=2` + TAP 22/23 → trả về `net::run` → marker + halt
//! ```
//!
//! # Vì sao **không** cần asm riêng cho tác vụ
//!
//! Stub ngắt chung đã là một "context switch" hoàn chỉnh: CPU đẩy `rflags/cs/rip`, stub đẩy
//! `vector` + mã lỗi + 15 GPR, handler chạy, stub `pop`/`iretq`. Chỉ cần **đổi `rsp`** sang frame
//! của tác vụ kế là chuyển được ngữ cảnh — không cần lưu/khôi phục thanh ghi bằng tay, không cần
//! asm mới. Frame khởi tạo (**22** qword — xem `vifast_plat_x86_64::sched`) do
//! [`vifast_plat_x86_64::sched::init_frame_words`] tính,
//! ảnh chỉ ghi từng qword bằng `asm::write_u64` (API an toàn) nên ảnh vẫn **không** `unsafe` (R5).
//!
//! # Vì sao hai tác vụ **không** in gì
//!
//! Mỗi tác vụ chỉ tăng bộ đếm riêng rồi quay vòng. In từ tác vụ sẽ xen ký tự với dòng
//! `# sched: switch …` của handler trên COM1 (bài học KT-259/KT-278: "in trước, đếm sau"), và
//! biến phép đo "hai tác vụ cùng chạy" thành đo tốc độ UART. Bằng chứng "đã chạy" là **bộ đếm**
//! (`runs a=… b=…`) do chính tác vụ tăng, đọc sau khi scheduler dừng.
//!
//! # Phạm vi (D2-212)
//!
//! **Hai** tác vụ kernel + preemption bằng timer. **Không** U-mode task, **không** nhiều hart
//! schedule, **không** priority, **không** sleep/wake. **Không** tuyên bố K5' đạt.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::sched as psched;

use crate::{check, Console};

/// Cỡ stack **riêng** của mỗi tác vụ — 16 KiB (đủ cho handler ngắt chạy trên stack ấy khi
/// preempt; tác vụ chỉ tăng bộ đếm nên dùng rất ít).
const TASK_STACK_BYTES: usize = 16 * 1024;
/// Số qword của stack tác vụ.
const TASK_STACK_QWORDS: usize = TASK_STACK_BYTES / 8;

/// Stack tác vụ: mảng `AtomicU64` (bộ nhớ **ghi được** mà không cần `static mut`/`unsafe`),
/// căn 16 byte để `rsp` sau `iretq` đúng SysV.
#[repr(align(16))]
struct TaskStack([AtomicU64; TASK_STACK_QWORDS]);

/// Stack của tác vụ **A** — vùng `.bss` riêng, `_start` đã zero cả `.bss`.
static TASK_A_STACK: TaskStack = TaskStack([const { AtomicU64::new(0) }; TASK_STACK_QWORDS]);
/// Stack của tác vụ **B** — vùng `.bss` riêng, **không** chồng lên A.
static TASK_B_STACK: TaskStack = TaskStack([const { AtomicU64::new(0) }; TASK_STACK_QWORDS]);

/// Số vòng tác vụ **A** đã chạy — tác vụ tự tăng, scheduler đọc sau khi dừng.
static TASK_A_RUNS: AtomicU64 = AtomicU64::new(0);
/// Số vòng tác vụ **B** đã chạy.
static TASK_B_RUNS: AtomicU64 = AtomicU64::new(0);

/// `1` ⇔ scheduler đang bật (handler timer được phép chuyển tác vụ).
static SCHED_ACTIVE: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ tick đầu đã lưu ngữ cảnh `main` và bắt đầu tác vụ A.
static SCHED_STARTED: AtomicU64 = AtomicU64::new(0);
/// Slot tác vụ **đang chạy** ([`psched::TASK_MAIN`]/[`psched::TASK_A`]/[`psched::TASK_B`]).
static SCHED_CURRENT: AtomicU64 = AtomicU64::new(psched::TASK_MAIN as u64);
/// Số tick timer đã nhận trong lúc scheduler bật.
static SCHED_TICKS: AtomicU64 = AtomicU64::new(0);
/// Số lần **chuyển** tác vụ đã làm.
static SCHED_SWITCHES: AtomicU64 = AtomicU64::new(0);
/// Số hiệu lần chuyển **A→B** đầu tiên (`0` = chưa có).
static SCHED_AB: AtomicU64 = AtomicU64::new(0);
/// Số hiệu lần chuyển **B→A** đầu tiên (`0` = chưa có).
static SCHED_BA: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ đã chuyển về `main`; `sched::run` thấy cờ này thì in tổng kết.
static SCHED_DONE: AtomicU64 = AtomicU64::new(0);
/// `rsp` frame đã lưu của từng slot ngữ cảnh (`main` + hai tác vụ).
static SCHED_RSP: [AtomicU64; psched::TASK_SLOTS] =
    [const { AtomicU64::new(0) }; psched::TASK_SLOTS];

/// Tác vụ **A**: tăng bộ đếm rồi quay vòng — timer sẽ preempt.
extern "C" fn task_a() -> ! {
    loop {
        TASK_A_RUNS.fetch_add(1, Ordering::Relaxed);
        core::hint::spin_loop();
    }
}

/// Tác vụ **B**: cùng khuôn, bộ đếm riêng.
extern "C" fn task_b() -> ! {
    loop {
        TASK_B_RUNS.fetch_add(1, Ordering::Relaxed);
        core::hint::spin_loop();
    }
}

/// Đỉnh stack của `stack` (địa chỉ PA = VA, identity map).
///
/// Lấy địa chỉ **phần tử** `stack.0` (không phải cả struct) để trường mảng là "được dùng" —
/// bộ nhớ ấy do CPU ghi qua `rsp`, Rust không bao giờ đọc từng phần tử.
fn stack_top(stack: &'static TaskStack) -> u64 {
    core::ptr::addr_of!(stack.0) as u64 + TASK_STACK_BYTES as u64
}

/// Ghi frame khởi tạo lên **đỉnh** stack của tác vụ; trả `rsp` của frame.
fn build_frame(stack_top: u64, entry: u64) -> u64 {
    let (words, rsp) = psched::init_frame_words(stack_top, entry, psched::TASK_RFLAGS);
    let mut i = 0;
    while i < psched::FRAME_WORDS {
        asm::write_u64(rsp + (i as u64) * 8, words[i]);
        i += 1;
    }
    rsp
}

/// Đọc `rip` đã lưu trong frame tại `rsp` — giá trị stub sẽ `iretq` tới.
fn frame_rip(rsp: u64) -> u64 {
    asm::read_u64(rsp + exc::FRAME_RIP_OFFSET as u64)
}

/// Handler timer gọi khi scheduler bật: lưu ngữ cảnh tác vụ hiện tại, chọn tác vụ kế, trả
/// `(rsp, rip)` của frame tác vụ kế — hoặc `None` nếu tick này **không** chuyển tác vụ.
///
/// `frame_rsp` là `rsp` của frame ngắt trên stack tác vụ đang chạy (stub chung truyền vào).
pub(crate) fn on_tick(frame_rsp: u64) -> Option<(u64, u64)> {
    if SCHED_ACTIVE.load(Ordering::Acquire) == 0 {
        return None;
    }
    // Đã chuyển về `main`: các tick còn lại (nếu có) không được kéo scheduler chạy lại.
    if SCHED_DONE.load(Ordering::Acquire) != 0 {
        return None;
    }
    SCHED_TICKS.fetch_add(1, Ordering::Relaxed);

    // ---- Tick đầu: lưu ngữ cảnh `main` rồi bắt đầu tác vụ A.
    if SCHED_STARTED.load(Ordering::Acquire) == 0 {
        SCHED_STARTED.store(1, Ordering::Release);
        SCHED_RSP[psched::TASK_MAIN].store(frame_rsp, Ordering::Release);
        SCHED_CURRENT.store(psched::TASK_A as u64, Ordering::Release);
        let rsp = SCHED_RSP[psched::TASK_A].load(Ordering::Acquire);
        return Some((rsp, frame_rip(rsp)));
    }

    // **Đột biến (i)** (`--features no-preempt`): sau tick đầu **không** gọi `schedule()` nữa ⇒
    // chỉ tác vụ A chạy, `main` không bao giờ được nhường lại ⇒ không có tổng kết/marker (ĐỎ).
    if cfg!(feature = "no-preempt") {
        return None;
    }

    // ---- Lưu ngữ cảnh tác vụ hiện tại, chọn tác vụ kế.
    let current = SCHED_CURRENT.load(Ordering::Acquire) as usize;
    SCHED_RSP[current].store(frame_rsp, Ordering::Release);
    let switches = SCHED_SWITCHES.load(Ordering::Acquire) + 1;
    SCHED_SWITCHES.store(switches, Ordering::Release);
    let next = psched::next_task(current, switches, psched::MAX_SWITCHES);

    // In **một lần** cho mỗi chiều: đủ bằng chứng, không nghẽn COM1 (mỗi tick một dòng sẽ biến
    // phép đo thành đo tốc độ UART — bài học KT-266).
    if current == psched::TASK_A && next == psched::TASK_B && SCHED_AB.load(Ordering::Relaxed) == 0
    {
        SCHED_AB.store(switches, Ordering::Release);
        let mut console = Console;
        let _ = writeln!(console, "# sched: switch a→b n={switches}");
    }
    if current == psched::TASK_B && next == psched::TASK_A && SCHED_BA.load(Ordering::Relaxed) == 0
    {
        SCHED_BA.store(switches, Ordering::Release);
        let mut console = Console;
        let _ = writeln!(console, "# sched: switch b→a n={switches}");
    }

    SCHED_CURRENT.store(next as u64, Ordering::Release);
    if next == psched::TASK_MAIN {
        SCHED_DONE.store(1, Ordering::Release);
    }

    // **Đột biến (ii)** (`--features bad-rsp`): nạp `rsp` của tác vụ kế bằng địa chỉ **chưa map**
    // thay vì frame đã lưu ⇒ stub `pop` từ đó ⇒ `#PF` (rồi `#DF` vì đẩy frame `#PF` cũng fault).
    if cfg!(feature = "bad-rsp") {
        return Some((crate::PF_UNMAPPED_ADDR, 0));
    }
    let rsp = SCHED_RSP[next].load(Ordering::Acquire);
    Some((rsp, frame_rip(rsp)))
}

/// Chạy scheduler: dựng hai tác vụ, bật ngắt, chờ `main` được nhường lại, in tổng kết + TAP.
///
/// Gọi từ [`crate::net::run`] **sau** TAP 21 (SMP) và **trước** marker — thứ tự TAP 1..21
/// **không đổi**.
pub fn run(console: &mut Console, tap_first: usize) {
    let a_rsp = build_frame(stack_top(&TASK_A_STACK), task_a as *const () as u64);
    let b_rsp = build_frame(stack_top(&TASK_B_STACK), task_b as *const () as u64);
    SCHED_RSP[psched::TASK_A].store(a_rsp, Ordering::Release);
    SCHED_RSP[psched::TASK_B].store(b_rsp, Ordering::Release);
    SCHED_ACTIVE.store(1, Ordering::Release);

    // Ngắt đã tắt từ `kmain` (sau khi đo timer); bật lại để PIT preempt `main` đang `hlt`.
    asm::sti();
    while SCHED_DONE.load(Ordering::Acquire) == 0 {
        asm::hlt();
    }
    asm::cli();
    SCHED_ACTIVE.store(0, Ordering::Release);

    let ticks = SCHED_TICKS.load(Ordering::Relaxed);
    let a = TASK_A_RUNS.load(Ordering::Relaxed);
    let b = TASK_B_RUNS.load(Ordering::Relaxed);
    let switches = SCHED_SWITCHES.load(Ordering::Relaxed);
    let ab = SCHED_AB.load(Ordering::Relaxed);
    let ba = SCHED_BA.load(Ordering::Relaxed);
    let _ = writeln!(console, "# sched: ticks={ticks} tasks=2");
    let _ = writeln!(
        console,
        "# sched: runs a={a} b={b} switches={switches} ab={ab} ba={ba} a_rsp=0x{a_rsp:x} b_rsp=0x{b_rsp:x}"
    );

    // TAP 22: **cả hai** tác vụ đã chạy thật (bộ đếm do chính tác vụ tăng, không phải hằng).
    check(console, tap_first, "sched: two tasks ran", a > 0 && b > 0);
    // TAP 23: đã **preempt** — có chuyển tác vụ theo **cả hai** chiều, và có ≥ 2 tick.
    check(
        console,
        tap_first + 1,
        "sched: preempted",
        switches >= 2 && ab >= 1 && ba >= 1 && ticks >= 2,
    );
}
