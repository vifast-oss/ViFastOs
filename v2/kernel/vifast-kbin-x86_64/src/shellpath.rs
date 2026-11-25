// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-341 / KT-341b** — shell **sản phẩm dùng chung** (`vifast-plat-user/programs/shell.rs`) +
//! **procsrv** (`…/procsrv.rs`) trên x86_64: ELF thật, CPL3, mọi `int 0x80` đi qua **một**
//! `vifast_core::syscall::dispatch` (`7C-r13`).
//!
//! # Vì sao tệp này tồn tại
//!
//! `KT-340c` đã nối `Machine` + IPC ring 3 nhưng chỉ trong ảnh **kiểm** (`ktest`), và `Machine`
//! nằm trong `.bss` (184.792 B) — vượt trần `.data+.bss` ảnh sản phẩm tối giản 65.536 B. `KT-341`
//! đưa `Machine` ra section NOLOAD `.corespace` (xem `core_bridge.rs`/`linker.ld`) rồi dựng **cùng**
//! đường shell cho **cả hai** ảnh:
//!
//! - ảnh **sản phẩm** (`--features product-shell`): `product::boot` gọi [`start_product`]; shell
//!   chạy CPL3 từ ELF, đọc ring KT-69 (kernel bơm script bằng `conchan`), in prompt/help/echo, rồi
//!   `ps`/`wait`/`kill` (**KT-341b**), `Exit(0)` ⇒ kernel in `# core:` + `# shell:` rồi marker +
//!   halt. **0 TAP** (D2-08).
//! - ảnh **kiểm** (`--features ktest`): sau TAP 38..43, `umode` gọi [`start_ktest`]; shell chạy
//!   **cùng** mã, kernel đếm byte `DebugPutStr` để phát **TAP 44..47** (prompt + lệnh nội trú +
//!   `ps` + `wait`/`kill`), rồi trả về chuỗi boot cũ (`net::run` TAP 15..).
//!
//! # **KT-341b** — hai tác vụ U-mode, kernel vẫn là scheduler **tuần tự**
//!
//! Ảnh sản phẩm không có scheduler preemptive trong core (Chu kỳ 3). `shell` và `procsrv` là **hai**
//! TCB `Running` luân phiên: mỗi lần tác vụ đang chạy gọi `Yield`, kernel **lưu nguyên khung**
//! `int 0x80` của nó ([`save_ctx`]) rồi nạp khung của tác vụ kia ([`load_ctx`]) + `CR3` của
//! `VSpace` tương ứng, trước `iretq` của stub asm. Đây **không** phải scheduler mới: là cùng mô
//! hình "kernel đóng vai scheduler tuần tự" mà `KT-339` dùng cho aarch64 — chỉ khác arch giữ khung
//! ở đâu (x86_64: 22 qword của stub `int 0x80`, xem [`FRAME_WORDS`]).
//!
//! Dịch vụ **chính sách** (D2-93: kernel đọc `Machine`, không tin lời U-mode):
//! - `ps` (label `PS_REQ` trên Endpoint 5) ⇒ kernel đọc **trạng thái TCB thật** rồi trả `PS_REPLY`
//!   trên Endpoint 4 (cùng giao thức riscv64, `proc_abi`/`shell.rs`);
//! - `wait`/`kill` (Endpoint 2) ⇒ **procsrv** U-mode trả lời trên Endpoint 4; `kill` còn gửi
//!   `EV_KILL` trên Endpoint 8 ⇒ kernel thi hành `Tcb.Suspend` trên TCB `victim`.
//!
//! # Bằng chứng (kernel đếm, không tin lời U-mode — D2-102)
//!
//! `syscall_env::put_str` đếm **byte thật** in ra COM1 (puts/bytes + bảy mẫu chuỗi). TAP 44 đòi
//! prompt `shell>` + `# shell: exit code=0` + `Exit` trả 0 + TCB `Inactive`; TAP 45 đòi
//! `# shell: help cmds=6` + `# shell: echo same-source`; TAP 46 đòi `# shell: ps n=3` (kernel trả
//! lời `PS_REQ` từ trạng thái `Machine`); TAP 47 đòi `# shell: wait …` + `# shell: kill …` **và**
//! `victim` đã `suspended=1` trong `Machine`. Nếu `int 0x80` của core bị tắt (đột biến
//! `mut-no-int80`), không byte nào được in ⇒ các TAP ĐỎ.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::{
    KError, Machine, Message, MessageInfo, NonReadyTarget, ObjId, ObjType, SlotRef, SuspendMode,
    TcbState,
};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::bootmem::BootArena;
use vifast_plat_x86_64::core_bridge;
use vifast_plat_x86_64::format::Perms;
use vifast_plat_x86_64::mmu::TableMemory;
use vifast_plat_x86_64::syscall_env::{self, X86_64Env};
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::umode::{
    is_user_cs, SyscallRet, SYSCALL_FRAME_R10, SYSCALL_FRAME_R12, SYSCALL_FRAME_R13,
    SYSCALL_FRAME_R8, SYSCALL_FRAME_R9, USER_CS, USER_RFLAGS, USER_SS,
};
use vifast_plat_x86_64::userload;
use vifast_plat_x86_64::vspace::VSpace;

use crate::Console;

// ---------------------------------------------------------------------------------------------
// Bố cục CSpace — **khớp hằng số của `shell.rs`/`procsrv.rs`** (một giao thức, hai phía).
// ---------------------------------------------------------------------------------------------
const SLOT_UNTYPED: usize = 0;
const SLOT_EP_PROC: usize = 2;
const SLOT_CSPACE: usize = 3;
const SLOT_EP_REPLY: usize = 4;
const SLOT_EP_PS: usize = 5;
const SLOT_EP_EVENTS: usize = 8;
/// Cap `Frame` của trang IPC buffer (`add_foreign_object`, D2-83).
const SLOT_FRAME: usize = 11;
/// Cap `Tcb` của shell/procsrv (kernel giữ; U-mode không dùng).
const SLOT_TCB_SHELL: usize = 12;
const SLOT_TCB_PROCSRV: usize = 12;
/// Số ô CSpace của rig (cùng 16 ô với `coretest`).
const SHELL_SLOTS: usize = 16;
const PROCSRV_SLOTS: usize = 16;

/// CSpace **của kernel** — nơi giữ cap Endpoint/TCB để kernel *tự* nhận `PS_REQ`/`EV_*` và trả
/// `PS_REPLY` (D2-93: policy ở kernel, cùng bố cục riscv64 `proc_rt`).
const K_SLOT_UNTYPED: usize = 0;
const K_SLOT_EP_PROC: usize = 1;
const K_SLOT_EP_REPLY: usize = 2;
const K_SLOT_EP_PS: usize = 3;
const K_SLOT_EP_EVENTS: usize = 4;
const K_SLOT_TCB_VICTIM: usize = 13;
const K_SLOT_TCB_KERN: usize = 14;
const K_SLOT_CSPACE: usize = 15;
/// **KT-341b** — `SchedControl` + `SchedContext` của shell/procsrv (cùng mức ưu tiên ⇒
/// `yield` = round-robin FIFO của scheduler core, không cần scheduler thứ hai).
const K_SLOT_SCHED_CONTROL: usize = 11;
const K_SLOT_SC_SHELL: usize = 9;
const K_SLOT_SC_PROCSRV: usize = 10;
const KERNEL_SLOTS: usize = 16;
/// Mức ưu tiên **bằng nhau** của shell/procsrv (mô hình MCS: `yield` đưa TCB xuống cuối hàng
/// cùng mức ⇒ hai tác vụ luân phiên; kernel/victim mức thấp hơn nên không bao giờ được chọn).
const TASK_PRIORITY: u8 = 10;
/// Ngân sách/chu kỳ của `SchedContext` (đơn vị mô hình; đủ cho phép đo vài trăm syscall).
const TASK_BUDGET: u64 = 10_000;
const TASK_PERIOD: u64 = 100_000;

/// Số frame Untyped của rig (TCB + Endpoint ≈ 3 KiB; giữ 4 frame như KT-341).
const SHELL_ARENA_FRAMES: usize = 4;

/// VA trang **IPC buffer** — phải bằng backend x86_64 của shim (`Sys::IPC_BUFFER_VA`). **Cùng**
/// một VA cho cả hai tác vụ (mỗi `VSpace` map frame của chính nó) ⇒ hook `ipc_buffer_va` của
/// `syscall_env` chỉ cần **một** giá trị, đúng ngữ nghĩa "mỗi tiến trình một buffer".
const IPC_BUF_VA: u64 = 0x7000_9000;
/// Đỉnh stack U-mode của shell; hai trang mọc xuống (`0x7000_E000..0x7001_0000`).
const SHELL_STACK_TOP: u64 = 0x7001_0000;
/// Đỉnh stack U-mode của procsrv (cửa sổ 2 MiB kế: `shell` ở `0x7000_0000`, `procsrv` ở
/// `0x7020_0000` — xem `build.rs`/`userload::USER_WINDOW_BASE`).
const PROCSRV_STACK_TOP: u64 = 0x7021_0000;
/// Số trang stack (8 KiB).
const SHELL_STACK_PAGES: usize = 2;
/// Ngân sách `Yield` trước khi kernel tiếp quản (chống treo khi đột biến).
const YIELD_BUDGET: u64 = 20_000;
/// `e_machine` x86-64.
const EM_X86_64: u16 = 62;

/// Số tác vụ U-mode của rig (scheduler tuần tự: đúng **một** `Running` tại một thời điểm).
const NTASKS: usize = 2;
const T_SHELL: usize = 0;
const T_PROCSRV: usize = 1;

/// Số qword của khung `int 0x80`: 15 GPR + `vector` + `err` + `rip`/`cs`/`rflags`/`rsp`/`ss`.
/// Khớp **tuyệt đối** [`vifast_plat_x86_64::umode`] (`SYSCALL_FRAME_*`) — test host của crate arch
/// chốt từng offset.
const FRAME_WORDS: usize = 22;
/// Ô `rdx` (value trả về) trong khung.
const W_RDX: usize = 12;
/// Ô `rax` (error trả về) trong khung.
const W_RAX: usize = 14;
/// Ô `rip` (CPU đẩy — lệnh kế tiếp `int 0x80`).
const W_RIP: usize = 17;
/// Ô `cs` (CPU đẩy — `0x1B` nếu vào từ ring 3).
const W_CS: usize = 18;
/// Ô `rflags` (CPU đẩy).
const W_RFLAGS: usize = 19;
/// Ô `rsp` user (CPU đẩy khi đổi CPL).
const W_RSP: usize = 20;
/// Ô `ss` user (CPU đẩy khi đổi CPL).
const W_SS: usize = 21;

/// Chương trình U-mode **cùng nguồn** với riscv64/aarch64 (`vifast-plat-user/programs/`).
static SHELL_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.elf"));
static PROCSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/procsrv.elf"));

/// Hằng giao thức `proc_abi`/`shell.rs` (một giao thức, hai phía — không chép số *khác* nguồn).
const PS_REQ: u64 = 0x602;
const PS_REPLY: u64 = 0x603;
/// `EV_KILL` — procsrv báo kernel thi hành `Tcb.Suspend` (cùng số với `procsrv.rs`).
const EV_KILL: u64 = 0x504;

/// `u32::MAX` = chưa có; `u64::MAX` = chưa đo.
const NO_TCB: u32 = u32::MAX;
const NO_U64: u64 = u64::MAX;

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Tác vụ **đang chạy** (0 = shell, 1 = procsrv).
static CUR: AtomicU32 = AtomicU32::new(0);
/// Object `Tcb` của từng tác vụ (chỉ số theo [`T_SHELL`]/[`T_PROCSRV`]).
static TCB_OBJ: [AtomicU32; NTASKS] = [const { AtomicU32::new(NO_TCB) }; NTASKS];
/// `CR3` (`PML4`) của từng tác vụ.
static TASK_ROOT: [AtomicU64; NTASKS] = [const { AtomicU64::new(0) }; NTASKS];
/// Khung `int 0x80` đã lưu của từng tác vụ — `NTASKS × FRAME_WORDS` ô (không `unsafe`: mỗi ô một
/// `AtomicU64`, cùng khuôn "trạng thái nằm ở static có ghi chú" của `syscall_env`).
static CTX: [AtomicU64; NTASKS * FRAME_WORDS] = [const { AtomicU64::new(0) }; NTASKS * FRAME_WORDS];
/// Tác vụ đã `Exit` (không bao giờ được chọn để chạy lại).
static EXITED: [AtomicBool; NTASKS] = [const { AtomicBool::new(false) }; NTASKS];
/// Số lần đổi tác vụ (bằng chứng `# proc: switch`).
static SWITCHES: AtomicU64 = AtomicU64::new(0);
/// Số `PS_REPLY` kernel đã gửi (bằng chứng `ps`).
static PS_REPLIES: AtomicU64 = AtomicU64::new(0);
/// `true` ⇔ kernel đã nhận `EV_KILL` và `Tcb.Suspend(victim)` thành công.
static KILL_OK: AtomicBool = AtomicBool::new(false);
/// Object `Tcb` của `victim` (tác vụ **không** chạy — mục tiêu của `kill <pid>`).
static VICTIM_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// Object `Tcb` của kernel (người gửi `PS_REPLY`, người nhận `PS_REQ`/`EV_*`).
static KERNEL_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// CSpace của kernel (cap Endpoint/TCB để kernel tự phục vụ giao thức).
static KERNEL_ROOT: AtomicU32 = AtomicU32::new(NO_TCB);
/// `CR3` gốc của ảnh (khôi phục khi `Exit`).
static BOOT_ROOT: AtomicU64 = AtomicU64::new(0);
/// Mã thoát của shell (`Exit`).
static EXIT_CODE: AtomicU64 = AtomicU64::new(NO_U64);
/// Số `Yield` đã gọi (cả hai tác vụ).
static YIELDS: AtomicU64 = AtomicU64::new(0);
/// Số `int 0x80` đã vào dispatcher (bằng chứng CPL3 thật).
static SYSCALLS: AtomicU64 = AtomicU64::new(0);
/// `true` ⇔ đã thấy ít nhất một `cs = 0x1B` (thật sự từ CPL3).
static SAW_USER_CS: AtomicBool = AtomicBool::new(false);

/// `true` ⇔ rig shell/procsrv đã dựng (handler `int 0x80` định tuyến sang [`dispatch`]).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// TCB hiện hành của tác vụ `task` (`None` ⇔ chưa dựng / đã `Exit`).
pub fn current_tcb_of(task: usize) -> Option<ObjId> {
    if !active() || task >= NTASKS {
        return None;
    }
    let tcb = TCB_OBJ[task].load(Ordering::Acquire);
    (tcb != NO_TCB).then_some(tcb)
}

/// TCB của tác vụ **đang chạy**.
pub fn current_tcb() -> Option<ObjId> {
    current_tcb_of(CUR.load(Ordering::Acquire) as usize)
}

// ---------------------------------------------------------------------------------------------
// Khung `int 0x80` của từng tác vụ — lưu/nạp **nguyên** 22 qword (đổi tác vụ đúng nghĩa)
// ---------------------------------------------------------------------------------------------

/// Đọc ô `word` của khung đã lưu của `task`.
fn ctx_get(task: usize, word: usize) -> u64 {
    CTX[task * FRAME_WORDS + word].load(Ordering::Acquire)
}

/// Ghi ô `word` của khung đã lưu của `task`.
fn ctx_set(task: usize, word: usize, value: u64) {
    CTX[task * FRAME_WORDS + word].store(value, Ordering::Release);
}

/// Lưu **nguyên** khung `int 0x80` của `task` (đọc từ con trỏ frame của stub asm).
fn save_ctx(task: usize, frame: u64) {
    let mut word = 0usize;
    while word < FRAME_WORDS {
        ctx_set(task, word, asm::read_u64(frame + (word as u64) * 8));
        word += 1;
    }
}

/// Nạp khung đã lưu của `task` vào frame mà stub asm sắp `iretq` — mọi GPR + `rip`/`cs`/`rflags`/
/// `rsp`/`ss` đổi cùng lúc, nên `iretq` trả về **đúng** tác vụ kia.
fn load_ctx(task: usize, frame: u64) {
    let mut word = 0usize;
    while word < FRAME_WORDS {
        asm::write_u64(frame + (word as u64) * 8, ctx_get(task, word));
        word += 1;
    }
}

/// Khung **đầu tiên** của một tác vụ chưa từng chạy: `iretq` xuống `entry` với `RSP = stack_top`,
/// `CS/SS` ring 3 thật, `RFLAGS.IF = 1` (giống `asm::enter_user`).
fn init_ctx(task: usize, entry: u64, stack_top: u64) {
    let mut word = 0usize;
    while word < FRAME_WORDS {
        ctx_set(task, word, 0);
        word += 1;
    }
    ctx_set(task, W_RIP, entry);
    ctx_set(task, W_CS, USER_CS as u64);
    ctx_set(task, W_SS, USER_SS as u64);
    ctx_set(task, W_RFLAGS, USER_RFLAGS);
    ctx_set(task, W_RSP, stack_top);
}

// ---------------------------------------------------------------------------------------------
// Cấp phát / ring
// ---------------------------------------------------------------------------------------------

/// **KT-406** — script kernel bơm vào ring khi bật feature `script-input` **hoặc** ảnh kiểm
/// (`ktest`: TAP 44..47 cần lệnh mẫu như trước, không được đổi hành vi).
///
/// Đó là đường **kiểm hồi quy** của `run-x86_64.sh` (không mở stdin QEMU). Ảnh sản phẩm thật
/// (không bật feature) khởi tạo ring **rỗng**; byte **duy nhất** tới shell là từ ngắt COM1 RX
/// (`comirq::on_irq` → ring) — "script bơm kernel" không còn là đường vào của sản phẩm.
#[cfg(any(feature = "script-input", feature = "ktest"))]
fn script_for_this_image(buf: &mut [u8; 128], victim: u64) -> usize {
    build_script(buf, victim)
}

/// **KT-406** — ảnh sản phẩm thật: ring **rỗng**, input từ UART RX IRQ.
#[cfg(not(any(feature = "script-input", feature = "ktest")))]
fn script_for_this_image(_buf: &mut [u8; 128], _victim: u64) -> usize {
    0
}

/// Cấp một frame hoặc in `PANIC` + halt (fail-loud, không đoán).
fn alloc(console: &mut Console, arena: &mut BootArena, what: &str) -> u64 {
    match arena.alloc_frame() {
        Some(pa) => pa,
        None => {
            let _ = writeln!(
                console,
                "PANIC: shellpath: arena hết frame khi cấp '{what}' (đã dùng {} / {})",
                arena.used(),
                arena.frames(),
            );
            asm::halt(3)
        }
    }
}

/// Zero một frame 4 KiB (arena NOLOAD **không** được `.bss` zero hộ).
fn zero_frame(pa: u64) {
    let mut offset = 0u64;
    while offset < 4096 {
        asm::write_u64(pa + offset, 0);
        offset += 8;
    }
}

/// `vifast_ring::Mem` trên **PA identity** của arena (kernel ghi ring trước khi `iretq`).
struct PhysMem {
    base: u64,
}

impl vifast_ring::Mem for PhysMem {
    fn load(&self, offset: usize) -> u64 {
        asm::read_u64(self.base + offset as u64)
    }
    fn store(&self, offset: usize, value: u64) {
        asm::write_u64(self.base + offset as u64, value);
    }
    fn acquire(&self, offset: usize) -> u64 {
        asm::read_u64(self.base + offset as u64)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        asm::write_u64(self.base + offset as u64, value);
    }
}

/// Cấp frame ring KT-69, khởi tạo hình học 32×32 rồi bơm `script` bằng **cùng** codec `conchan`
/// mà `shell.rs` đọc (một định dạng, hai phía).
fn make_ring(console: &mut Console, arena: &mut BootArena, script: &[u8]) -> u64 {
    let pa = alloc(console, arena, "ring");
    zero_frame(pa);
    let ring = vifast_ring::Ring::new(PhysMem { base: pa });
    if ring
        .init(
            vifast_virtio::conchan::CHANNEL_SLOTS,
            vifast_virtio::conchan::SLOT_BYTES,
        )
        .is_err()
    {
        let _ = writeln!(console, "PANIC: shellpath: ring init lỗi");
        asm::halt(3)
    }
    let mut start = 0usize;
    while start < script.len() {
        let mut slot = [0u8; vifast_virtio::conchan::CHUNK_BYTES];
        let mut used = 0usize;
        while start < script.len() && used < slot.len() {
            slot[used] = script[start];
            used += 1;
            start += 1;
        }
        let (words, _) = vifast_virtio::conchan::pack(&slot[..used]);
        if ring.send(&words).is_err() {
            let _ = writeln!(console, "PANIC: shellpath: ring send lỗi");
            asm::halt(3)
        }
    }
    pa
}

/// Ghi `s` vào `buf` tại `len` (không tràn) — dựng script có `pid` động.
fn push_bytes(buf: &mut [u8], len: &mut usize, s: &[u8]) {
    for byte in s {
        if *len < buf.len() {
            buf[*len] = *byte;
            *len += 1;
        }
    }
}

/// Ghi số thập phân của `value` vào `buf` tại `len`.
fn push_u64(buf: &mut [u8], len: &mut usize, value: u64) {
    let mut digits = [0u8; 20];
    let mut count = 0usize;
    let mut left = value;
    loop {
        digits[count] = b'0' + (left % 10) as u8;
        count += 1;
        left /= 10;
        if left == 0 || count == digits.len() {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        if *len < buf.len() {
            buf[*len] = digits[count];
            *len += 1;
        }
    }
}

/// Script kernel bơm vào ring KT-69 (**nợ KT-73** giữ nguyên: kernel bơm, chưa có UART RX thật):
/// prompt + hai lệnh nội trú + **`ps`/`wait`/`kill`** (KT-341b) + `exit` sạch. `pid` là `ObjId`
/// thật của TCB `victim` (đọc từ `Machine` **sau** khi dựng rig — không đoán số).
fn build_script(buf: &mut [u8; 128], victim: u64) -> usize {
    let mut len = 0usize;
    push_bytes(buf, &mut len, b"help\necho same-source\nps\nwait ");
    push_u64(buf, &mut len, victim);
    push_bytes(buf, &mut len, b"\nkill ");
    push_u64(buf, &mut len, victim);
    push_bytes(buf, &mut len, b"\nexit\n");
    len
}

// ---------------------------------------------------------------------------------------------
// Dựng rig + nạp ELF
// ---------------------------------------------------------------------------------------------

/// Giữ cap/TCB của rig sau khi dựng (một lần `with_machine`).
struct Rig {
    shell_tcb: ObjId,
    procsrv_tcb: ObjId,
    kernel_tcb: ObjId,
    victim: ObjId,
    /// CSpace **của kernel** (cap Endpoint/TCB để kernel tự phục vụ giao thức).
    kernel_root: ObjId,
}

/// Dựng **ba** CSpace (kernel/shell/procsrv) + Endpoint dùng chung + 4 TCB.
///
/// Endpoint được cấp **một lần** trong CSpace của kernel rồi `CNode.Copy` sang hai CSpace tác vụ —
/// nhờ vậy cả ba bên nói **cùng** một đối tượng Endpoint (một giao thức), không phải ba bản sao
/// không ai nối với ai.
fn build_rig(console: &mut Console, untyped: ObjId, buf_shell: u64, buf_procsrv: u64) -> Rig {
    let built = core_bridge::with_machine(|m: &mut Machine| -> Result<Rig, KError> {
        // ---- 1) CSpace của kernel + Endpoint dùng chung.
        let kroot = m.add_cnode(KERNEL_SLOTS);
        let kc = m.root_cap(kroot);
        m.put_cap(kroot, K_SLOT_CSPACE, kc)?;
        let uc = m.root_cap(untyped);
        m.put_cap(kroot, K_SLOT_UNTYPED, uc)?;
        let us = SlotRef::new(kroot, K_SLOT_UNTYPED);
        for slot in [
            K_SLOT_EP_PROC,
            K_SLOT_EP_REPLY,
            K_SLOT_EP_PS,
            K_SLOT_EP_EVENTS,
        ] {
            let cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
            m.put_cap(kroot, slot, cap)?;
        }
        // ---- 2) TCB của kernel + `victim` (victim **không** bao giờ chạy: mục tiêu của `kill`).
        for slot in [K_SLOT_TCB_KERN, K_SLOT_TCB_VICTIM] {
            let cap = m.retype(us, ObjType::Tcb, 0, 0)?;
            m.put_cap(kroot, slot, cap)?;
        }
        let kernel_root = SlotRef::new(kroot, K_SLOT_CSPACE);
        let kernel_tcb_slot = SlotRef::new(kroot, K_SLOT_TCB_KERN);
        let victim_slot = SlotRef::new(kroot, K_SLOT_TCB_VICTIM);
        m.tcb_configure(kernel_tcb_slot, Some(kernel_root))?;
        m.tcb_configure(victim_slot, Some(kernel_root))?;
        m.tcb_set_priority(kernel_tcb_slot, 5, u8::MAX)?;
        m.tcb_set_priority(victim_slot, 3, u8::MAX)?;
        let kernel_tcb = m.cap_obj_at(kroot, K_SLOT_TCB_KERN)?;
        let victim = m.cap_obj_at(kroot, K_SLOT_TCB_VICTIM)?;
        m.start_tcb(kernel_tcb)?;
        m.start_tcb(victim)?;
        // **KT-341b** — `SchedControl` + `SchedContext`: `sched_pick_on` chỉ chọn TCB **có ngân
        // sách** (`tcb_has_budget` ⇒ cần `funding_sc`); thiếu SC thì `SYS_YIELD` không chọn được
        // ai và trả `Switched` không có đích. Cùng khuôn `proc_rt::build_rig` riscv64.
        let sched_control = m.retype(us, ObjType::SchedControl, 0, 0)?;
        m.put_cap(kroot, K_SLOT_SCHED_CONTROL, sched_control)?;
        for (sc_slot, sc) in [
            (K_SLOT_SC_SHELL, m.retype(us, ObjType::SchedContext, 0, 0)?),
            (
                K_SLOT_SC_PROCSRV,
                m.retype(us, ObjType::SchedContext, 0, 0)?,
            ),
        ] {
            m.put_cap(kroot, sc_slot, sc)?;
            m.sched_control_configure(
                SlotRef::new(kroot, K_SLOT_SCHED_CONTROL),
                SlotRef::new(kroot, sc_slot),
                TASK_BUDGET,
                TASK_PERIOD,
            )?;
        }

        // ---- 3) CSpace của **shell**: copy 4 Endpoint + Frame IPC buffer + TCB.
        let sroot = m.add_cnode(SHELL_SLOTS);
        let sc = m.root_cap(sroot);
        m.put_cap(sroot, SLOT_CSPACE, sc)?;
        let suc = m.root_cap(untyped);
        m.put_cap(sroot, SLOT_UNTYPED, suc)?;
        for (k_slot, s_slot) in [
            (K_SLOT_EP_PROC, SLOT_EP_PROC),
            (K_SLOT_EP_REPLY, SLOT_EP_REPLY),
            (K_SLOT_EP_PS, SLOT_EP_PS),
            (K_SLOT_EP_EVENTS, SLOT_EP_EVENTS),
        ] {
            m.copy(SlotRef::new(kroot, k_slot), SlotRef::new(sroot, s_slot))?;
        }
        let frame = m.add_foreign_object(ObjType::Frame, buf_shell)?;
        let fcap = m.root_cap(frame);
        m.put_cap(sroot, SLOT_FRAME, fcap)?;
        let s_tcb = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(sroot, SLOT_TCB_SHELL, s_tcb)?;
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(sroot, SLOT_TCB_SHELL),
            Some(SlotRef::new(sroot, SLOT_CSPACE)),
            Some((SlotRef::new(sroot, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        m.tcb_set_priority(SlotRef::new(sroot, SLOT_TCB_SHELL), TASK_PRIORITY, u8::MAX)?;
        m.set_sched_params(
            SlotRef::new(sroot, SLOT_TCB_SHELL),
            Some(SlotRef::new(kroot, K_SLOT_SC_SHELL)),
        )?;
        let shell_tcb = m.cap_obj_at(sroot, SLOT_TCB_SHELL)?;
        m.start_tcb(shell_tcb)?;

        // ---- 4) CSpace của **procsrv**: cùng bốn Endpoint (2/4/8), Frame IPC buffer, TCB.
        let proot = m.add_cnode(PROCSRV_SLOTS);
        let pc = m.root_cap(proot);
        m.put_cap(proot, SLOT_CSPACE, pc)?;
        for (k_slot, p_slot) in [
            (K_SLOT_EP_PROC, SLOT_EP_PROC),
            (K_SLOT_EP_REPLY, SLOT_EP_REPLY),
            (K_SLOT_EP_EVENTS, SLOT_EP_EVENTS),
        ] {
            m.copy(SlotRef::new(kroot, k_slot), SlotRef::new(proot, p_slot))?;
        }
        let pframe = m.add_foreign_object(ObjType::Frame, buf_procsrv)?;
        let pfcap = m.root_cap(pframe);
        m.put_cap(proot, SLOT_FRAME, pfcap)?;
        let p_tcb = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(proot, SLOT_TCB_PROCSRV, p_tcb)?;
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(proot, SLOT_TCB_PROCSRV),
            Some(SlotRef::new(proot, SLOT_CSPACE)),
            Some((SlotRef::new(proot, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        m.tcb_set_priority(
            SlotRef::new(proot, SLOT_TCB_PROCSRV),
            TASK_PRIORITY,
            u8::MAX,
        )?;
        m.set_sched_params(
            SlotRef::new(proot, SLOT_TCB_PROCSRV),
            Some(SlotRef::new(kroot, K_SLOT_SC_PROCSRV)),
        )?;
        let procsrv_tcb = m.cap_obj_at(proot, SLOT_TCB_PROCSRV)?;
        m.start_tcb(procsrv_tcb)?;

        Ok(Rig {
            shell_tcb,
            procsrv_tcb,
            kernel_tcb,
            victim,
            kernel_root: kroot,
        })
    });
    match built {
        Ok(rig) => rig,
        Err(error) => {
            let _ = writeln!(console, "PANIC: shellpath: dựng rig lỗi {error:?}");
            asm::halt(3)
        }
    }
}

/// Nạp một chương trình U-mode vào `VSpace` **riêng** + stack + trang IPC buffer, rồi đăng ký dải
/// user cho hook `DebugPutStr` (fail-closed).
fn load_task(
    console: &mut Console,
    arena: &mut BootArena,
    bytes: &[u8],
    what: &str,
    stack_top: u64,
    buf_pa: u64,
) -> (VSpace<BootArena>, userload::Loaded) {
    let mut vs = match VSpace::create(1, *arena) {
        Ok(vs) => vs,
        Err(error) => {
            let _ = writeln!(console, "PANIC: shellpath: tạo VSpace {what} lỗi {error:?}");
            asm::halt(3)
        }
    };
    let loaded = match userload::load_elf(&mut vs, arena, bytes, EM_X86_64) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: shellpath: nạp ELF {what} lỗi {} elf={}",
                error.name(),
                error.elf_code()
            );
            asm::halt(3)
        }
    };
    if let Err(error) = userload::map_user_stack(&mut vs, arena, stack_top, SHELL_STACK_PAGES) {
        let _ = writeln!(
            console,
            "PANIC: shellpath: map stack {what} lỗi {}",
            error.name()
        );
        asm::halt(3)
    }
    if let Err(error) = vs.map(IPC_BUF_VA, buf_pa, Perms::USER_RW) {
        let _ = writeln!(
            console,
            "PANIC: shellpath: map ipc-buffer {what} lỗi {error:?}"
        );
        asm::halt(3)
    }
    if !syscall_env::register_user_range(IPC_BUF_VA, 0x1000) {
        let _ = writeln!(console, "PANIC: shellpath: đăng ký ipc-buffer {what} lỗi");
        asm::halt(3)
    }
    (vs, loaded)
}

/// Dựng Untyped của rig: ảnh **sản phẩm** tự `core_bridge::init`; ảnh **kiểm** dùng lại
/// `Machine`/Untyped của `KT-340c` (không init lại — sẽ xoá rig cũ và mọi bằng chứng TAP 38..43).
fn machine_untyped(console: &mut Console, arena: &mut BootArena, fresh: bool) -> (ObjId, bool) {
    let mut base = 0u64;
    for index in 0..SHELL_ARENA_FRAMES {
        let pa = alloc(console, arena, "untyped");
        if index == 0 {
            base = pa;
        }
    }
    if fresh {
        let report = core_bridge::init(base, SHELL_ARENA_FRAMES * 4096, 1);
        let _ = writeln!(
            console,
            "# core: addr=0x{:x} size={} pin={} in_image={} harts={} objects={} caps={} healthy={}",
            report.addr,
            report.static_bytes,
            report.pin,
            u8::from(report.in_image),
            report.harts,
            report.objects,
            report.caps,
            u8::from(report.healthy()),
        );
        if !report.healthy() {
            let _ = writeln!(console, "PANIC: shellpath: Machine không healthy");
            asm::halt(3)
        }
        (report.untyped, true)
    } else {
        // Ảnh kiểm: `Machine` của KT-340c đã init — **không** init lại (sẽ xoá rig + bằng chứng
        // TAP 38..43); cấp Untyped **riêng** cho rig shell từ 4 frame mới, cùng khuôn aarch64.
        let untyped = core_bridge::with_machine(|m| m.add_untyped(base, SHELL_ARENA_FRAMES * 4096));
        (untyped, false)
    }
}

/// Dựng rig hai tác vụ + nạp **hai** ELF, rồi `iretq` xuống CPL3 tại shell. **Không trả về**:
/// `Exit`/lỗi dừng trong [`dispatch`] (kernel tiếp quản).
fn prepare_and_run(console: &mut Console, fresh: bool) -> ! {
    core_bridge::set_image_bounds(
        core::ptr::addr_of!(__text_start) as u64,
        core::ptr::addr_of!(__kernel_end) as u64,
    );
    let Some(mut arena) = BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) else {
        let _ = writeln!(console, "PANIC: shellpath: arena không hợp lệ");
        asm::halt(3)
    };
    // Hai trang IPC buffer **riêng** (cùng VA, hai `VSpace` khác nhau) — zero tường minh vì arena
    // NOLOAD không được `.bss` zero hộ.
    let buf_shell = alloc(console, &mut arena, "ipc-buffer-shell");
    zero_frame(buf_shell);
    let buf_procsrv = alloc(console, &mut arena, "ipc-buffer-procsrv");
    zero_frame(buf_procsrv);
    syscall_env::set_ipc_buffer_va(Some(IPC_BUF_VA));
    syscall_env::reset_user_ranges();

    // ---- 1) Nạp **hai** chương trình U-mode vào hai `VSpace` riêng (W^X theo segment).
    let (mut vs_shell, loaded_shell) = load_task(
        console,
        &mut arena,
        SHELL_ELF,
        "shell",
        SHELL_STACK_TOP,
        buf_shell,
    );
    let (vs_proc, loaded_proc) = load_task(
        console,
        &mut arena,
        PROCSRV_ELF,
        "procsrv",
        PROCSRV_STACK_TOP,
        buf_procsrv,
    );
    let shell_root = vs_shell.root();
    let proc_root = vs_proc.root();

    // ---- 2) Ring KT-69 cho shell. Script có **pid thật** của `victim` ⇒ dựng rig trước.
    let untyped = {
        let (untyped, _fresh) = machine_untyped(console, &mut arena, fresh);
        untyped
    };
    let rig = build_rig(console, untyped, buf_shell, buf_procsrv);
    let mut script = [0u8; 128];
    let script_len = script_for_this_image(&mut script, u64::from(rig.victim));
    let ring_pa = make_ring(console, &mut arena, &script[..script_len]);
    if let Err(error) = vs_shell.map(userload::RING_VA, ring_pa, Perms::USER_RW) {
        let _ = writeln!(console, "PANIC: shellpath: map ring lỗi {error:?}");
        asm::halt(3)
    }
    if !syscall_env::register_user_range(userload::RING_VA, 0x1000) {
        let _ = writeln!(console, "PANIC: shellpath: đăng ký ring lỗi");
        asm::halt(3)
    }

    TCB_OBJ[T_SHELL].store(rig.shell_tcb, Ordering::Release);
    TCB_OBJ[T_PROCSRV].store(rig.procsrv_tcb, Ordering::Release);
    TASK_ROOT[T_SHELL].store(shell_root, Ordering::Release);
    TASK_ROOT[T_PROCSRV].store(proc_root, Ordering::Release);
    VICTIM_TCB.store(rig.victim, Ordering::Release);
    KERNEL_TCB.store(rig.kernel_tcb, Ordering::Release);
    KERNEL_ROOT.store(rig.kernel_root, Ordering::Release);
    init_ctx(T_SHELL, loaded_shell.entry, SHELL_STACK_TOP);
    init_ctx(T_PROCSRV, loaded_proc.entry, PROCSRV_STACK_TOP);

    let _ = writeln!(
        console,
        "# shell: load entry=0x{:x} base=0x{:x}+{}p segs={} stack_top=0x{:x} buf_va=0x{:x} ring_va=0x{:x} script={}B root=0x{:x}",
        loaded_shell.entry,
        loaded_shell.base,
        loaded_shell.pages,
        loaded_shell.segments,
        SHELL_STACK_TOP,
        IPC_BUF_VA,
        userload::RING_VA,
        script_len,
        shell_root,
    );
    let _ = writeln!(
        console,
        "# proc: load entry=0x{:x} base=0x{:x}+{}p segs={} stack_top=0x{:x} buf_va=0x{:x} root=0x{:x} procsrv={} victim={} kernel={}",
        loaded_proc.entry,
        loaded_proc.base,
        loaded_proc.pages,
        loaded_proc.segments,
        PROCSRV_STACK_TOP,
        IPC_BUF_VA,
        proc_root,
        rig.procsrv_tcb,
        rig.victim,
        rig.kernel_tcb,
    );

    // ---- 3) Kernel chặn IRQ legacy **trước khi bật `IF`** (SeaBIOS để lại 8259 PIC: IRQ0/PIT
    // ánh xạ vector 8 — trùng ô IDT của `#DF`; `iretq` đặt `RFLAGS.IF = 1` ⇒ 1/6 boot flake).
    asm::outb(0x21, 0xFF);
    asm::outb(0xA1, 0xFF);

    // ---- 3b) **KT-406** — COM1 RX **IRQ thật**: LAPIC/SVR + IOAPIC pin 4 + IER.RX. Chỉ ảnh
    // **sản phẩm có shell** (ảnh kiểm giữ đường TAP; ảnh tối giản không có U-mode). Bằng chứng
    // in từ **thanh ghi đọc lại**, không phải hằng mong đợi.
    #[cfg(all(feature = "product-shell", not(feature = "ktest")))]
    {
        let uart = crate::comirq::init(ring_pa);
        let (slots, slot_bytes) = crate::comirq::geometry();
        let _ = writeln!(
            console,
            "# uart: rx on={} irq={} vector=0x{:02x} lapic_id={} svr=0x{:03x} tpr={} ioapic_id={} ioapic_ver=0x{:02x} entries={} redir_low=0x{:04x} redir_high=0x{:08x} dest={} ier=0x{:02x} lsr=0x{:02x} ring_pa=0x{:x} slots={} slot={} ring_check={} ok={}",
            u8::from(uart.ier & vifast_plat_x86_64::serial::IER_RX != 0),
            crate::comirq::COM1_IRQ,
            crate::comirq::COM1_VECTOR,
            vifast_plat_x86_64::apic::lapic_id(uart.lapic_id_raw),
            uart.svr,
            uart.tpr,
            vifast_plat_x86_64::apic::ioapic_id(uart.ioapic_id_raw),
            uart.ioapic_version,
            uart.ioapic_entries,
            uart.redir_low & 0xFFFF,
            uart.redir_high,
            uart.redir_high >> 24,
            uart.ier,
            uart.lsr,
            ring_pa,
            slots,
            slot_bytes,
            u8::from(crate::comirq::ring_check()),
            u8::from(uart.ok()),
        );
        if !uart.ok() {
            let _ = writeln!(
                console,
                "PANIC: shellpath: UART RX IRQ không sẵn sàng (KT-406)"
            );
            asm::halt(6)
        }
    }

    // ---- 4) Shell `Running` trước, procsrv `Ready` (đổi tác vụ ở [`switch_to`] khi `Yield`).
    let started = core_bridge::with_machine(|m| {
        m.transition_tcb(rig.shell_tcb, NonReadyTarget::Running { hart: 0 })
            .is_ok()
    });
    if !started {
        let _ = writeln!(
            console,
            "PANIC: shellpath: không chuyển được shell sang Running"
        );
        asm::halt(3)
    }
    // ---- 5) Vào ring 3: `CR3` shell + `TSS.RSP0` + `iretq` (không trả).
    let boot_root = asm::read_cr3() & !0xFFF;
    BOOT_ROOT.store(boot_root, Ordering::Release);
    tss::set_rsp0(0, core::ptr::addr_of!(__syscall_stack_top) as u64);
    vs_shell.activate();
    syscall_env::capture_reset();
    ACTIVE.store(true, Ordering::Release);
    let _ = writeln!(
        console,
        "# shell: enter cpl=3 entry=0x{:x} rsp=0x{:x} cr3=0x{:x} tcb={} untyped={} fresh={} tasks={}",
        loaded_shell.entry,
        SHELL_STACK_TOP,
        shell_root,
        rig.shell_tcb,
        untyped,
        u8::from(fresh),
        NTASKS,
    );
    asm::enter_user(loaded_shell.entry, SHELL_STACK_TOP, USER_RFLAGS)
}

/// Ảnh **sản phẩm** (`--features product-shell`): `Machine` mới + shell/procsrv thật, rồi marker
/// + halt.
pub fn start_product(console: &mut Console) -> ! {
    prepare_and_run(console, true)
}

/// Ảnh **kiểm** (`--features ktest`): dùng lại `Machine` của KT-340c, chạy shell + procsrv
/// (TAP 44..47) rồi trả chuỗi boot về `net::run`.
pub fn start_ktest(console: &mut Console) -> ! {
    prepare_and_run(console, false)
}

// ---------------------------------------------------------------------------------------------
// Dịch vụ kernel (policy — D2-93: đọc `Machine`, không tin lời U-mode)
// ---------------------------------------------------------------------------------------------

/// Dựng `Message` 4 MR (cùng định dạng `MessageInfo`/`Message` của core — không tự bịa layout).
fn msg(label: u64, words: &[u64]) -> Message {
    let info = MessageInfo::new(label as u32, 0, false, 0, words.len() as u8)
        .expect("shellpath: MessageInfo");
    Message::new(info, words).expect("shellpath: Message")
}

/// Trả `PS_REPLY` cho shell: đọc **trạng thái TCB thật** trong `Machine` rồi đóng gói
/// `(id << 5) | (state.index() << 1) | suspended` (cùng bố cục `shell.rs::cmd_ps`/riscv64).
fn service_ps(console: &mut Console) {
    let kernel = KERNEL_TCB.load(Ordering::Acquire);
    let kroot = KERNEL_ROOT.load(Ordering::Acquire);
    if kernel == NO_TCB || kroot == 0 {
        return;
    }
    loop {
        let request = core_bridge::with_machine(|m| {
            match m.nb_recv(kernel, SlotRef::new(kroot, K_SLOT_EP_PS)) {
                Ok(Some(message)) => Some(message),
                _ => None,
            }
        });
        let Some(request) = request else { return };
        if u64::from(request.info.label()) != PS_REQ {
            continue;
        }
        let entries = core_bridge::with_machine(|m| {
            let mut entries = [0u64; 3];
            let mut count = 0usize;
            for id in [
                TCB_OBJ[T_PROCSRV].load(Ordering::Acquire),
                TCB_OBJ[T_SHELL].load(Ordering::Acquire),
                VICTIM_TCB.load(Ordering::Acquire),
            ] {
                if id == NO_TCB {
                    continue;
                }
                let Ok(state) = m.tcb_state(id) else { continue };
                let suspended = m
                    .tcb_info_obj(id)
                    .map(|info| info.suspended)
                    .unwrap_or(false);
                entries[count] =
                    (u64::from(id) << 5) | ((state.index() as u64) << 1) | u64::from(suspended);
                count += 1;
                if count == entries.len() {
                    break;
                }
            }
            (count, entries)
        });
        let (count, entries) = entries;
        let words = [entries[0] | (entries[1] << 16), entries[2]];
        core_bridge::with_machine(|m| {
            let _ = m.nb_send(
                kernel,
                SlotRef::new(kroot, K_SLOT_EP_REPLY),
                msg(PS_REPLY, &words),
            );
        });
        PS_REPLIES.fetch_add(1, Ordering::Release);
        let _ = writeln!(
            console,
            "# core: ps entries={count} replies={} (kernel đọc TcbState từ Machine)",
            PS_REPLIES.load(Ordering::Acquire)
        );
    }
}

/// Nhận sự kiện `EV_*` (procsrv/shell → Endpoint 8); `EV_KILL` ⇒ kernel thi hành
/// `Tcb.Suspend(victim)` — phần **chính sách** của `kill`, đúng khuôn riscv64 `proc_rt::service`.
fn service_events(console: &mut Console) {
    let kernel = KERNEL_TCB.load(Ordering::Acquire);
    let kroot = KERNEL_ROOT.load(Ordering::Acquire);
    if kernel == NO_TCB || kroot == 0 {
        return;
    }
    loop {
        let label = core_bridge::with_machine(|m| {
            match m.nb_recv(kernel, SlotRef::new(kroot, K_SLOT_EP_EVENTS)) {
                Ok(Some(message)) => Some(u64::from(message.info.label())),
                _ => None,
            }
        });
        let Some(label) = label else { return };
        if label != EV_KILL {
            continue;
        }
        let victim = VICTIM_TCB.load(Ordering::Acquire);
        if victim == NO_TCB {
            continue;
        }
        let ok = core_bridge::with_machine(|m| {
            m.tcb_suspend(SlotRef::new(kroot, K_SLOT_TCB_VICTIM), SuspendMode::Abort)
                .is_ok()
        });
        KILL_OK.store(ok, Ordering::Release);
        let _ = writeln!(
            console,
            "# proc: victim pid={victim} suspend={} (kernel thi hành Tcb.Suspend sau EV_KILL)",
            u8::from(ok)
        );
    }
}

/// Dịch vụ **giữa hai lượt U-mode**: `ps` (kernel trả lời) + `EV_KILL` (kernel thi hành).
fn service(console: &mut Console) {
    service_events(console);
    service_ps(console);
}

// ---------------------------------------------------------------------------------------------
// Đổi tác vụ — kernel là scheduler **tuần tự** (một `Running` tại một thời điểm)
// ---------------------------------------------------------------------------------------------

/// Lưu khung `int 0x80` của `cur` (kèm hai giá trị trả của syscall vừa xong), nạp khung của
/// `next` vào frame mà stub sắp `iretq`, đổi `CR3` sang `VSpace` của `next`, rồi trả `(rax, rdx)`
/// **của `next`** để stub ghi đúng hai ô đó (stub ghi đè sau khi handler trả về — xem `asm.rs`).
///
/// Trạng thái TCB **không** bị đụng ở đây: khi `SyscallOutcome::Switched`, scheduler của core **đã**
/// đưa `cur` về `Ready` và tác vụ kế sang `Running` (`yield_current`/`dispatch`); shellpath chỉ đổi
/// **ngữ cảnh phần cứng** (GPR + `RIP`/`RSP` + `CR3`) cho khớp với quyết định ấy.
fn switch_frame(
    cur: usize,
    next: usize,
    frame: u64,
    result: &vifast_core::syscall::SyscallReturn,
) -> SyscallRet {
    write_return_frame(frame, result);
    save_ctx(cur, frame);
    ctx_set(cur, W_RAX, result.error);
    ctx_set(cur, W_RDX, result.value);
    load_ctx(next, frame);
    asm::write_cr3(TASK_ROOT[next].load(Ordering::Acquire));
    CUR.store(next as u32, Ordering::Release);
    SWITCHES.fetch_add(1, Ordering::Release);
    SyscallRet {
        error: ctx_get(next, W_RAX),
        value: ctx_get(next, W_RDX),
    }
}

/// Tác vụ kế mà **scheduler core** vừa đặt `Running` (sau `SYS_YIELD`): shell và procsrv cùng
/// mức ưu tiên + `SchedContext` có ngân sách ⇒ `yield` đưa TCB xuống cuối hàng FIFO cùng mức, tác
/// vụ kia lên đầu hàng — round-robin của **chính** scheduler core, không scheduler thứ hai.
fn next_running(cur: usize) -> Option<usize> {
    core_bridge::with_machine(|m| {
        let mut next = None;
        let mut task = 0usize;
        while task < NTASKS {
            if task != cur && !EXITED[task].load(Ordering::Acquire) {
                let tcb = TCB_OBJ[task].load(Ordering::Acquire);
                if tcb != NO_TCB && matches!(m.tcb_state(tcb), Ok(TcbState::Running { .. })) {
                    next = Some(task);
                }
            }
            task += 1;
        }
        next
    })
}

/// procsrv `Exit` (panic U-mode): không có `Switched` từ scheduler nên kernel **tự** đưa shell về
/// `Running` rồi đổi ngữ cảnh — phép đo shell tiếp tục, TAP 46/47 sẽ ĐỎ vì thiếu reply thật.
fn hand_back_to_shell(console: &mut Console, cur: usize, frame: u64) -> SyscallRet {
    let shell = TCB_OBJ[T_SHELL].load(Ordering::Acquire);
    let ok = core_bridge::with_machine(|m| {
        m.transition_tcb(shell, NonReadyTarget::Running { hart: 0 })
            .is_ok()
    });
    if !ok {
        let _ = writeln!(
            console,
            "PANIC: shellpath: procsrv Exit mà không đưa được shell về Running"
        );
        asm::halt(5)
    }
    switch_frame(
        cur,
        T_SHELL,
        frame,
        &vifast_core::syscall::SyscallReturn::new(0, 0),
    )
}

/// Ghi `MessageInfo`/MR trả về vào frame theo `7C-r13` (D2-242): `r13` = info, `r12` = MR3,
/// `r10/r8/r9` = MR0..2. Một chỗ duy nhất — dispatcher của ảnh sản phẩm và của `umode` (ktest)
/// đều đi qua đây.
pub fn write_return_frame(frame: u64, result: &vifast_core::syscall::SyscallReturn) {
    asm::write_u64(frame + SYSCALL_FRAME_R13 as u64, result.info);
    asm::write_u64(frame + SYSCALL_FRAME_R12 as u64, result.mr[3]);
    asm::write_u64(frame + SYSCALL_FRAME_R10 as u64, result.mr[0]);
    asm::write_u64(frame + SYSCALL_FRAME_R8 as u64, result.mr[1]);
    asm::write_u64(frame + SYSCALL_FRAME_R9 as u64, result.mr[2]);
}

/// `int 0x80` của shell/procsrv — **một** đường qua `vifast_core::syscall::dispatch` (D2-79). Trả
/// `SyscallRet` cho stub asm (rax/rdx); `MessageInfo`/MR ghi thẳng vào frame (`7C-r13`).
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dispatch(
    nr: u64,
    a0: u64,
    a1: u64,
    mi: u64,
    mr0: u64,
    mr1: u64,
    mr2: u64,
    mr3: u64,
    cs: u64,
    _rip: u64,
    frame: u64,
) -> SyscallRet {
    let mut console = Console;
    let user = is_user_cs(cs);
    if user {
        SAW_USER_CS.store(true, Ordering::Release);
    }
    SYSCALLS.fetch_add(1, Ordering::Release);
    let cur = CUR.load(Ordering::Acquire) as usize;

    // **Đột biến (ii)** — tắt đường `DebugPutStr` của **core** trên ảnh: shell không in được
    // prompt/help ⇒ TAP 44..47 **ĐỎ**. `Exit` vẫn đi qua core để phép đo kết thúc (nếu chặn cả
    // Exit thì ảnh treo, không còn "đỏ có chẩn đoán").
    #[cfg(feature = "mut-no-int80")]
    if nr == vifast_abi::SYS_DEBUG_PUT_STR {
        return SyscallRet {
            error: 0,
            value: a1,
        };
    }

    let Some(tcb) = current_tcb_of(cur) else {
        let _ = writeln!(console, "PANIC: shellpath: int80 nr={nr} không có TCB");
        asm::halt(1)
    };
    if nr == vifast_abi::SYS_YIELD {
        let yields = YIELDS.fetch_add(1, Ordering::Release) + 1;
        if yields > YIELD_BUDGET {
            let _ = writeln!(console, "# shell fail: yield budget hết (yields={yields})");
            #[cfg(feature = "ktest")]
            crate::check(
                &mut console,
                44,
                "shell: prompt from CPL3 via int 0x80",
                false,
            );
            asm::halt(4)
        }
    }
    // `[calling.x86_64]` — ba syscall gửi có `args[1]` = `MessageInfo` (`rdx`), `args[2..6]` =
    // MR0..3 (`r10/r8/r9/r12`); còn lại `args[0..5] = rdi, rsi, rdx, r10, r8, r9`.
    let args = match nr {
        vifast_abi::SYS_CALL | vifast_abi::SYS_SEND | vifast_abi::SYS_NB_SEND => {
            [a0, mi, mr0, mr1, mr2, mr3]
        }
        _ => [a0, a1, mi, mr0, mr1, mr2],
    };
    let outcome = core_bridge::with_machine(|m| {
        vifast_core::syscall::dispatch(m, tcb, 0, nr, args, &mut X86_64Env)
    });
    match outcome {
        vifast_core::syscall::SyscallOutcome::Return(result) => {
            if !user {
                let _ = writeln!(console, "PANIC: shellpath: int80 nr={nr} không từ CPL3");
                asm::halt(1)
            }
            write_return_frame(frame, &result);
            // Dịch vụ kernel **trước** khi trả về U-mode: shell vừa `NBSend(PS_REQ)` thì nhận
            // `PS_REPLY` ngay ở lượt `NBRecv` kế (không cần thêm vòng).
            service(&mut console);
            SyscallRet {
                error: result.error,
                value: result.value,
            }
        }
        vifast_core::syscall::SyscallOutcome::Exit { code } => {
            if cur != T_SHELL {
                // procsrv `Exit` (panic U-mode): đánh dấu chết rồi trả lượt cho shell — phép đo
                // shell vẫn tiếp tục và TAP 46/47 sẽ ĐỎ vì thiếu reply thật.
                EXITED[cur].store(true, Ordering::Release);
                return hand_back_to_shell(&mut console, cur, frame);
            }
            finish(&mut console, code)
        }
        vifast_core::syscall::SyscallOutcome::Switched { state } => {
            // Scheduler core đã chọn tác vụ kế (round-robin cùng mức ưu tiên): đổi **ngữ cảnh
            // phần cứng** sang tác vụ ấy. `Yield` trả `(0, 0)` — cả `shell.rs` lẫn `procsrv.rs`
            // đều bỏ qua giá trị trả của `Yield` (chỉ dùng làm điểm nhường).
            let Some(next) = next_running(cur) else {
                let _ = writeln!(
                    console,
                    "PANIC: shellpath: Switched (nr={nr}) mà không tác vụ nào Running (state=0x{:x})",
                    state.index()
                );
                asm::halt(2)
            };
            let result = vifast_core::syscall::SyscallReturn::new(0, 0);
            switch_frame(cur, next, frame, &result)
        }
        vifast_core::syscall::SyscallOutcome::Halt { code } => {
            let _ = writeln!(console, "# shell: halt code={code} nr={nr}");
            asm::halt(code as u32)
        }
    }
}

/// **Ảnh sản phẩm** — stub asm `int 0x80` gọi **đúng** tên symbol `syscall_dispatch` (cùng tên
/// với handler ảnh kiểm; hai ảnh không bao giờ link cùng lúc). Mọi syscall đi qua [`dispatch`].
#[cfg(all(feature = "product-shell", not(feature = "ktest")))]
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn syscall_dispatch(
    nr: u64,
    a0: u64,
    a1: u64,
    mi: u64,
    mr0: u64,
    mr1: u64,
    mr2: u64,
    mr3: u64,
    cs: u64,
    rip: u64,
    frame: u64,
) -> SyscallRet {
    dispatch(nr, a0, a1, mi, mr0, mr1, mr2, mr3, cs, rip, frame)
}

/// Kết thúc pha shell: khôi phục `CR3`, đo bằng chứng kernel, in `# shell:` rồi cfg-branch
/// (sản phẩm: marker + halt; ktest: TAP 44..47 + `net::run`).
fn finish(console: &mut Console, code: u64) -> ! {
    // Trả `CR3` về cây gốc **trước** khi hoạt động tiếp (cùng bài học `KT-340b`).
    let boot_root = BOOT_ROOT.load(Ordering::Acquire);
    if boot_root != 0 {
        asm::write_cr3(boot_root);
    }
    EXIT_CODE.store(code, Ordering::Release);
    let shell = TCB_OBJ[T_SHELL].load(Ordering::Acquire);
    let (state, objects, caps, victim_suspended) = core_bridge::with_machine(|m| {
        (
            m.tcb_state(shell).ok(),
            m.object_count(),
            m.live_cap_count(),
            m.tcb_info_obj(VICTIM_TCB.load(Ordering::Acquire))
                .map(|info| info.suspended)
                .unwrap_or(false),
        )
    });
    let capture = syscall_env::capture_report();
    let state_code = state.map(|s| match s {
        TcbState::Inactive => 0u64,
        TcbState::Dead => 10,
        _ => 2,
    });
    let kill_ok = KILL_OK.load(Ordering::Acquire);
    KILL_OK.store(kill_ok, Ordering::Release);
    let _ = writeln!(
        console,
        "# shell: exit code={code} state={} puts={} bytes={} hash=0x{:016x} prompt={} help={} echo={} exit_line={} ps={} wait={} kill={} syscalls={} yields={} user_cs={} switches={} ps_replies={} victim_suspended={} kill_ok={} objects={objects} caps={caps}",
        state_code.unwrap_or(99),
        capture.puts,
        capture.bytes,
        capture.hash,
        u8::from(capture.prompt),
        u8::from(capture.help),
        u8::from(capture.echo),
        u8::from(capture.exit_line),
        u8::from(capture.ps),
        u8::from(capture.wait),
        u8::from(capture.kill),
        SYSCALLS.load(Ordering::Acquire),
        YIELDS.load(Ordering::Acquire),
        u8::from(SAW_USER_CS.load(Ordering::Acquire)),
        SWITCHES.load(Ordering::Acquire),
        PS_REPLIES.load(Ordering::Acquire),
        u8::from(victim_suspended),
        u8::from(kill_ok),
    );
    ACTIVE.store(false, Ordering::Release);

    #[cfg(feature = "ktest")]
    {
        let exited = state == Some(TcbState::Inactive);
        crate::check(
            console,
            44,
            "shell: prompt + clean Exit from CPL3 through core int 0x80",
            code == 0
                && exited
                && SAW_USER_CS.load(Ordering::Acquire)
                && capture.puts > 0
                && capture.prompt
                && capture.exit_line,
        );
        crate::check(
            console,
            45,
            "shell: help + echo builtin (core syscalls, no ktest code in ELF)",
            capture.help && capture.echo,
        );
        crate::check(
            console,
            46,
            "shell: ps served by kernel EP_PS from Machine TcbState (n=3)",
            capture.ps && PS_REPLIES.load(Ordering::Acquire) >= 1,
        );
        crate::check(
            console,
            47,
            "shell: wait + kill via procsrv EP_PROC, victim suspended by kernel",
            capture.wait && capture.kill && kill_ok && victim_suspended,
        );
        crate::net::run(console, 15)
    }
    #[cfg(all(feature = "product-shell", not(feature = "ktest")))]
    {
        let _ = capture;
        // **KT-406** — bằng chứng UART RX **IRQ thật** của lần boot này (bộ đếm handler, không
        // phải số viết tay): `irq>0` + `rx>0` ⇒ byte đã qua COM1 → IOAPIC → handler → ring;
        // `drops=0` ⇒ không mất byte; `head>0` ⇒ shell **đã tiêu thụ** ring.
        {
            let uart = crate::comirq::stats();
            let (head, tail, used, _seq) = crate::comirq::ring_stats();
            let uart_ok = uart.irqs > 0 && uart.bytes > 0 && uart.drops == 0 && head > 0;
            let _ = writeln!(
                console,
                "# uart: irq={} rx={} chunks={} drops={} empty={} no_ring={} last={} head={head} tail={tail} used={used} ok={}",
                uart.irqs,
                uart.bytes,
                uart.chunks,
                uart.drops,
                uart.empty_polls,
                uart.no_ring,
                uart.last_irq_bytes,
                u8::from(uart_ok),
            );
        }
        let _ = writeln!(console, "{}", crate::BOOT_MARKER.trim_end());
        let _ = writeln!(console, "{}", crate::HALT_LINE);
        asm::halt(0)
    }
    #[cfg(not(any(feature = "ktest", feature = "product-shell")))]
    {
        let _ = code;
        asm::halt(0)
    }
}

extern "C" {
    static __text_start: u8;
    static __kernel_end: u8;
    static __syscall_stack_top: u8;
}

const _: () = assert!(IPC_BUF_VA & 0xFFF == 0);
const _: () = assert!(SHELL_STACK_TOP & 0xFFF == 0);
const _: () = assert!(PROCSRV_STACK_TOP & 0xFFF == 0);
const _: () = assert!(SHELL_STACK_TOP - (SHELL_STACK_PAGES as u64) * 0x1000 > IPC_BUF_VA);
const _: () = assert!(PROCSRV_STACK_TOP - (SHELL_STACK_PAGES as u64) * 0x1000 > IPC_BUF_VA);
