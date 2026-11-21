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
//! # KT-304 — tác vụ **ring 3** trong scheduler
//!
//! Thêm tác vụ **U** chạy ở **CPL3** (dùng `umode` sẵn có của KT-270): frame khởi tạo 22 qword
//! với `cs = 0x1B`, `ss = 0x23`, `rsp` = user rsp, nằm trên **stack kernel riêng** của U. Trước
//! lần `iretq` đầu tiên, `TSS.RSP0` được nạp **đỉnh stack kernel ấy** — nếu không, tick timer từ
//! CPL3 sẽ đẩy frame vào stack syscall mà `main` đang chạy và ghi đè frame của `main` (đúng lớp
//! lỗi boot 410 của KT-298).
//!
//! Thứ tự [`psched::TASK_SEQUENCE`]: `A→B→A→B→A→B→U→A→U→main` — U bị preempt **hai** lần
//! (`U→A`, `U→main`) và được chạy lại **một** lần (`A→U`). Bằng chứng: bộ đếm user do chính
//! tác vụ ring 3 tăng (`n > 0`), `cs` của frame lúc preempt (`ring=3` ở **cả hai** lần), số lần
//! preempt/resume, và `last >= first` (bộ đếm **không giảm**).
//!
//! **KT-308** — KT-304 đòi thêm `last > first` ("bộ đếm tăng giữa hai lần preempt"). Đó là
//! **giả định sai**: tick là PIT 100 Hz (10 ms); nếu vCPU bị host deschedule ≥ 10 ms đúng lúc
//! `iretq` xuống CPL3 thì tick kế đã pending ⇒ CPU nhận ngắt **trước** lệnh user đầu tiên ⇒
//! 0 vòng. Race xảy ra được ở **cả hai** cửa sổ: soak KT-308 bắt boot 331 (`first > 0`,
//! `last == first` — 0 vòng ở cửa sổ **sau**) và boot 231 (`first == 0`, `last > first` — 0 vòng
//! ở cửa sổ **đầu**). Vì vậy **không** cửa sổ nào được đòi có tiến bộ; bất biến thật là "tác vụ
//! chạy **ít nhất một** cửa sổ" (`n > 0`) + bộ đếm **không giảm** (`last >= first`) + `cs = 0x1B`
//! ở **cả hai** lần preempt + `resumed ≥ 1`. Không nới ngưỡng nào khác.
//!
//! # Phạm vi (D2-212, D2-219)
//!
//! **Ba** tác vụ (hai kernel + một ring 3) + preemption bằng timer. **Không** nhiều tác vụ ring
//! 3, **không** IPC giữa chúng, **không** nhiều hart schedule, **không** priority, **không**
//! sleep/wake. **Không** tuyên bố K5' đạt.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::sched as psched;
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::umode::USER_CS;

use crate::{check, umode, Console};

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
/// **KT-304** — stack **kernel** riêng của tác vụ **ring 3** (U). Đây là nơi frame khởi tạo 22
/// qword nằm và là `TSS.RSP0` khi tác vụ chạy ở CPL3: timer ngắt từ ring 3 đẩy frame vào đây,
/// **không** đụng stack syscall mà `main` đang chạy.
static TASK_U_STACK: TaskStack = TaskStack([const { AtomicU64::new(0) }; TASK_STACK_QWORDS]);

/// Số vòng tác vụ **A** đã chạy — tác vụ tự tăng, scheduler đọc sau khi dừng.
static TASK_A_RUNS: AtomicU64 = AtomicU64::new(0);
/// Số vòng tác vụ **B** đã chạy.
static TASK_B_RUNS: AtomicU64 = AtomicU64::new(0);

/// **KT-304** — số lần tác vụ **ring 3** bị tick preempt (phải ≥ 2: lần đầu rời U, lần cuối về
/// `main`).
static U_PREEMPTED: AtomicU64 = AtomicU64::new(0);
/// **KT-304** — số lần tác vụ ring 3 được `schedule()` chọn **lại** sau khi đã bị preempt.
static U_RESUMED: AtomicU64 = AtomicU64::new(0);
/// **KT-304** — `cs` của frame lúc preempt **cuối** tác vụ ring 3 (`0x1B` = ring 3 thật).
static U_PREEMPT_CS: AtomicU64 = AtomicU64::new(0);
/// **KT-308** — `cs` của frame lúc preempt **đầu** tác vụ ring 3. Phải kiểm **cả hai** lần
/// preempt: một lần ở CPL3 không chứng minh lần còn lại cũng ở CPL3.
static U_PREEMPT_CS_FIRST: AtomicU64 = AtomicU64::new(0);
/// **KT-304** — bộ đếm user tại lần preempt **đầu** (để so với lần cuối: `last >= first` ⇒ bộ
/// đếm **không giảm**). **KT-308**: `first` **không** được đòi `> 0` — cửa sổ đầu cũng có thể
/// 0 vòng dưới host deschedule (boot 231 của soak KT-308).
static U_COUNT_FIRST: AtomicU64 = AtomicU64::new(0);
/// **KT-304** — bộ đếm user tại lần preempt **cuối**.
static U_COUNT_LAST: AtomicU64 = AtomicU64::new(0);

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

/// **KT-304** — ghi frame khởi tạo **ring 3** lên đỉnh stack **kernel** của tác vụ U; trả `rsp`
/// của frame (nằm trên stack kernel, không phải stack user).
fn build_user_frame(kernel_stack_top: u64, entry: u64) -> u64 {
    let (words, rsp) = psched::init_user_frame_words(
        kernel_stack_top,
        entry,
        vifast_plat_x86_64::umode::USER_RFLAGS,
        umode::USER_TASK_RSP,
    );
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
        // **KT-304** — tác vụ ring 3 chạy trên **stack kernel riêng** của nó (frame do CPU đẩy
        // khi ngắt từ CPL3 nằm ở đây, không phải trên stack user).
        psched::TASK_U => {
            let base = core::ptr::addr_of!(TASK_U_STACK.0) as u64;
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
    if slot != psched::TASK_MAIN
        && slot != psched::TASK_A
        && slot != psched::TASK_B
        && slot != psched::TASK_U
    {
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
    let ss = asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 16);
    // **Đột biến (ii)** (`--features reject-user-cs`): từ chối **mọi** frame ring 3 ⇒ lần chuyển
    // sang tác vụ U in `# sched: BAD frame` rồi halt (ĐỎ, không marker). Chứng minh nhánh ring 3
    // của `switch_frame_ok` là **có răng**, không phải nhánh chết.
    if cfg!(feature = "reject-user-cs") && cs == USER_CS as u64 {
        return false;
    }
    psched::switch_frame_ok(rip, cs, rflags, task_rsp, ss)
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
                "# sched: BAD frame fields rip=0x{:x} cs=0x{:x} rflags=0x{:x} task_rsp=0x{:x} ss=0x{:x}",
                asm::read_u64(rsp + exc::FRAME_RIP_OFFSET as u64),
                asm::read_u64(rsp + exc::FRAME_CS_OFFSET as u64),
                asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64),
                asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 8),
                asm::read_u64(rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 16),
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

    // ---- **KT-304**: tác vụ **ring 3** — đo preempt/resume **trước** khi đổi ngữ cảnh.
    //
    // `frame_rsp` là frame do CPU đẩy khi ngắt từ CPL3 (trên stack kernel riêng của U), nên ô
    // `cs` của nó là **bằng chứng đã thật sự ở ring 3** lúc bị preempt. Bộ đếm user đọc được vì
    // kernel đọc thẳng trang stack user (`CR4.SMAP = 0`) — cùng đường `DebugPutStr` của KT-270.
    if current == psched::TASK_U {
        let cs = asm::read_u64(frame_rsp + exc::FRAME_CS_OFFSET as u64);
        let count = asm::read_u64(umode::USER_COUNTER_VA);
        if U_PREEMPTED.load(Ordering::Acquire) == 0 {
            // **KT-308** — lưu `cs` của lần preempt **đầu**: TAP 24 đòi ring 3 ở **cả hai** lần.
            U_PREEMPT_CS_FIRST.store(cs, Ordering::Release);
            U_COUNT_FIRST.store(count, Ordering::Release);
        }
        U_PREEMPT_CS.store(cs, Ordering::Release);
        U_COUNT_LAST.store(count, Ordering::Release);
        U_PREEMPTED.fetch_add(1, Ordering::Relaxed);
    }
    // `schedule()` chọn **lại** U sau khi nó đã bị preempt ⇒ đây là lần **chạy lại** ở ring 3.
    if next == psched::TASK_U && U_PREEMPTED.load(Ordering::Acquire) > 0 {
        U_RESUMED.fetch_add(1, Ordering::Relaxed);
    }

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
    // **KT-304**: tác vụ ring 3 — frame khởi tạo nằm trên **stack kernel riêng** của U, `cs`/
    // `ss`/`rsp` là của ring 3 (xem `psched::init_user_frame_words`).
    let u_kernel_top = stack_top(&TASK_U_STACK);
    let u_rsp = build_user_frame(u_kernel_top, umode::sched_entry_va());
    SCHED_RSP[psched::TASK_A].store(a_rsp, Ordering::Release);
    SCHED_RSP[psched::TASK_B].store(b_rsp, Ordering::Release);
    SCHED_RSP[psched::TASK_U].store(u_rsp, Ordering::Release);

    // Bộ đếm user: **zero trước** khi chạy. Chương trình ring 3 của KT-270 đã ghi `0x1234` vào
    // đúng ô này; không zero thì "tác vụ chưa chạy" vẫn đọc ra số dương (dương tính giả).
    asm::write_u64(umode::USER_COUNTER_VA, 0);

    // `TSS.RSP0` cho đường vào từ CPL3: stack kernel **riêng** của tác vụ ring 3 — **không**
    // phải stack syscall mà `main` đang chạy (frame của `main` đã lưu ở `SCHED_RSP[TASK_MAIN]`;
    // dùng chung stack sẽ ghi đè nó — đúng lớp lỗi boot 410 của KT-298).
    //
    // **Đột biến (i)** (`--features no-rsp0`): nạp `RSP0 = 0` ⇒ tick đầu tiên từ CPL3 đẩy frame
    // vào địa chỉ 0 ⇒ `#DF` (ĐỎ, không marker).
    if cfg!(feature = "no-rsp0") {
        tss::set_rsp0(0, 0);
    } else {
        tss::set_rsp0(0, u_kernel_top);
    }
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
    // **KT-304**: bằng chứng tác vụ ring 3 — bộ đếm do **chính tác vụ** tăng, `cs` của frame
    // lúc preempt, số lần preempt/resume.
    let u_runs = asm::read_u64(umode::USER_COUNTER_VA);
    let u_preempted = U_PREEMPTED.load(Ordering::Relaxed);
    let u_resumed = U_RESUMED.load(Ordering::Relaxed);
    let u_cs_first = U_PREEMPT_CS_FIRST.load(Ordering::Relaxed);
    let u_cs_last = U_PREEMPT_CS.load(Ordering::Relaxed);
    let u_first = U_COUNT_FIRST.load(Ordering::Relaxed);
    let u_last = U_COUNT_LAST.load(Ordering::Relaxed);
    // **KT-308** — ring 3 ở **cả hai** lần preempt; `last >= first` là bất biến thật (bộ đếm
    // đơn điệu). `last > first` **và** `first > 0` **không** được đòi: tick là PIT 100 Hz; nếu
    // vCPU bị host deschedule ≥ 10 ms đúng lúc `iretq` xuống CPL3 thì tick kế đã pending ⇒ CPU
    // nhận ngắt **trước** lệnh user đầu tiên ⇒ 0 vòng. Race xảy ra được ở **cả hai** cửa sổ:
    // soak KT-308 bắt boot 331 (`first > 0`, `last == first`) và boot 231 (`first == 0`,
    // `last > first`). Bất biến thật: `n > 0` (chạy **ít nhất một** cửa sổ) + `last >= first`.
    let u_ring3 = u_cs_first == USER_CS as u64 && u_cs_last == USER_CS as u64;
    let u_delta = u_last as i128 - u_first as i128;
    let _ = writeln!(console, "# sched: ticks={ticks} tasks=3");
    let _ = writeln!(
        console,
        "# sched: runs a={a} b={b} switches={switches} ab={ab} ba={ba} a_rsp=0x{a_rsp:x} b_rsp=0x{b_rsp:x} u_rsp=0x{u_rsp:x}"
    );
    let _ = writeln!(
        console,
        "# sched: umode task ran n={u_runs} ring={} preempted={u_preempted} resumed={u_resumed}",
        if u_ring3 { 3 } else { 0 },
    );
    let _ = writeln!(
        console,
        "# sched: umode counts first={u_first} last={u_last} cs=0x{u_cs_last:x} kernel_stack=0x{u_kernel_top:x}"
    );
    // **KT-308** — dòng bằng chứng định lượng: `last >= first` / `delta >= 0` (bộ đếm **không
    // giảm**). `delta` in **có dấu** để ca âm "bộ đếm giảm" (`last < first`) đọc được trực tiếp.
    // `first`/`last` vẫn in để người đọc thấy cửa sổ nào 0 vòng — **không** phải điều kiện.
    let _ = writeln!(
        console,
        "# sched: umode progress first={u_first} last={u_last} delta={u_delta}"
    );

    // TAP 22: **cả hai** tác vụ kernel đã chạy thật (bộ đếm do chính tác vụ tăng, không phải hằng).
    check(console, tap_first, "sched: two tasks ran", a > 0 && b > 0);
    // TAP 23: đã **preempt** — có chuyển tác vụ theo **cả hai** chiều, và có ≥ 2 tick.
    check(
        console,
        tap_first + 1,
        "sched: preempted",
        switches >= 2 && ab >= 1 && ba >= 1 && ticks >= 2,
    );
    // TAP 24 (**KT-304**, sửa **KT-308**): tác vụ **ring 3** — đã chạy thật (`n > 0`), bị
    // preempt khi đang ở CPL3 **cả hai** lần (`cs = 0x1B` ở lần đầu **và** lần cuối), được
    // `schedule()` chọn **lại** (`resumed ≥ 1`) và bộ đếm **không giảm** giữa hai lần preempt
    // (`last >= first`).
    //
    // **KT-308** — bỏ `last > first` của KT-304 **và** không thay bằng `first > 0`: cả hai đều
    // đòi một **cửa sổ cụ thể** có tiến bộ, điều **không** được bảo đảm (PIT 100 Hz; host
    // deschedule ≥ 10 ms đúng lúc `iretq` ⇒ tick kế pending ⇒ 0 vòng). Hai ảnh ngược đo được:
    // boot 331 (`first > 0`, `last == first`) và boot 231 (`first == 0`, `last > first`). Bất
    // biến thật là "tác vụ chạy **ít nhất một** cửa sổ" (`n > 0`) + bộ đếm **không giảm**
    // (`last >= first`) + `cs = 0x1B` **cả hai** lần + `resumed ≥ 1` — không nới ngưỡng nào.
    check(
        console,
        tap_first + 2,
        "sched: umode task",
        u_runs > 0 && u_ring3 && u_preempted >= 2 && u_resumed >= 1 && u_last >= u_first,
    );
}
