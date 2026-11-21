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

/// Cận dưới/cận trên của stack **riêng** của `slot` — chỉ có nghĩa với hai **tác vụ**.
///
/// `TASK_A`/`TASK_B` chạy trên stack 16 KiB do chính ảnh khai, nên cận là **chắc chắn**.
///
/// `TASK_MAIN` trả `None` — **có chủ đích**: `main` không có một stack cố định. Đường boot bắt
/// đầu trên `__stack_top` (64 KiB của linker), nhưng sau `umode::run` (`iretq` xuống ring 3 rồi
/// `int 0x80` quay lại) toàn bộ phần còn lại — kể cả `net::run` → `sched::run` — chạy trên
/// **stack syscall** (`__syscall_stack_top`, 32 KiB). Đo được: frame của `main` nằm ở `0x128970`
/// trong `[0x122000, 0x12a000)`, **dưới** `__stack_bottom` (`0x139000`) — nên lấy stack boot làm
/// cận là **dương tính giả** (đã bắt được ngay lượt boot đầu của bản sửa).
///
/// Frame của `main` do **CPU đẩy** tại chỗ ngắt, nên nó hợp lệ theo định nghĩa; lớp kiểm còn lại
/// ([`psched::switch_frame_ok`]) vẫn soi nội dung. Slot lạ ⇒ `None` và bị [`frame_ok`] từ chối.
fn slot_stack(slot: usize) -> Option<(u64, u64)> {
    match slot {
        psched::TASK_A => {
            let base = core::ptr::addr_of!(TASK_A_STACK.0) as u64;
            Some((base, base + TASK_STACK_BYTES as u64))
        }
        psched::TASK_B => {
            let base = core::ptr::addr_of!(TASK_B_STACK.0) as u64;
            Some((base, base + TASK_STACK_BYTES as u64))
        }
        _ => None,
    }
}

/// `true` ⇔ frame tại `rsp` **đủ** để `iretq` vào `slot` — kiểm **trước** khi stub đổi `rsp`
/// (KT-298, yêu cầu (b) của task).
///
/// Hai lớp:
///
/// 1. **Nội dung frame** — [`psched::switch_frame_ok`] kiểm `rip`/`cs`/`rflags`/`rsp` (hàm thuần,
///    có test host).
/// 2. **Vị trí frame** — `rsp` phải nằm trong stack **của chính slot ấy**. Đây là lớp bắt đúng
///    lỗi của boot 410: một frame của tác vụ bị đem `iretq` trên stack của slot khác (hoặc một
///    `rsp` rác) ⇒ hai ngữ cảnh chồng lên nhau. Không có lớp này, `#GP` chỉ hiện ra **sau** khi
///    stack đã bị ghi đè, không truy được về nguồn.
fn frame_ok(rsp: u64, slot: usize) -> bool {
    if slot != psched::TASK_MAIN && slot != psched::TASK_A && slot != psched::TASK_B {
        return false;
    }
    if let Some((bottom, top)) = slot_stack(slot) {
        if rsp < bottom || rsp >= top {
            return false;
        }
    }
    let rip = asm::read_u64(rsp + exc::FRAME_RIP_OFFSET as u64);
    let cs = asm::read_u64(rsp + exc::FRAME_CS_OFFSET as u64);
    let rflags = asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64);
    // Hai ô `rsp`/`ss` mà `iretq` của QEMU pop thêm nằm ngay sau `rflags` (frame 22 qword).
    let task_rsp = asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 8);
    psched::switch_frame_ok(rip, cs, rflags, task_rsp)
}

/// Frame của `slot` **không** qua được [`frame_ok`]: in chẩn đoán rồi **halt** — thà đỏ một boot
/// còn hơn `iretq` vào một frame rác rồi `#GP` storm (bài học boot 410).
///
/// **Chỉ đọc** các trường của frame khi `rsp` nằm trong stack của `slot`: đọc một `rsp` rác sẽ
/// sinh `#PF` **ngay trong đường chẩn đoán** — đo được với đột biến `bad-rsp`
/// (`# exc: cr2=0x700000000088`), tức là mất luôn dòng chẩn đoán đáng giá nhất.
fn bad_frame(slot: usize, rsp: u64) -> ! {
    let mut console = Console;
    let _ = writeln!(console, "# sched: BAD frame slot={slot} rsp=0x{rsp:x}");
    if let Some((bottom, top)) = slot_stack(slot) {
        if rsp >= bottom && rsp < top {
            let _ = writeln!(
                console,
                "# sched: BAD frame fields rip=0x{:x} cs=0x{:x} rflags=0x{:x} task_rsp=0x{:x}",
                asm::read_u64(rsp + exc::FRAME_RIP_OFFSET as u64),
                asm::read_u64(rsp + exc::FRAME_CS_OFFSET as u64),
                asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64),
                asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 8),
            );
        }
    }
    asm::halt(3)
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
        // **KT-298** — kiểm frame **trước** khi stub `mov rsp, rdx` (yêu cầu (b) của task).
        if !frame_ok(rsp, psched::TASK_A) {
            bad_frame(psched::TASK_A, rsp);
        }
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
    // thay vì frame đã lưu. KT-286 ghi kỳ vọng "`#PF` rồi `#DF`"; **KT-298** đặt đột biến **sau**
    // phép kiểm frame nên nó bị bắt sớm: `# sched: BAD frame …` + `halt` (ĐỎ có chẩn đoán thay
    // vì đỏ bằng triple-fault) — xem ghi chú feature trong `Cargo.toml`.
    let rsp = if cfg!(feature = "bad-rsp") {
        crate::PF_UNMAPPED_ADDR
    } else {
        SCHED_RSP[next].load(Ordering::Acquire)
    };
    // **KT-298** — cùng phép kiểm cho mọi lần chuyển (kể cả lần cuối về `main`, nơi frame nằm
    // trên stack của đường boot/U-mode — đúng chỗ boot 410 bị hỏng).
    if !frame_ok(rsp, next) {
        bad_frame(next, rsp);
    }
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
