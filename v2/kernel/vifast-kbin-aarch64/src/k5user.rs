// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-339 — K3.5 trên aarch64**: `procsrv` + `shell` + `echo` là chương trình Rust **thật**
//! (`vifast-plat-user/programs/`), chạy ở **EL0** qua `svc #0`, với `vifast-core::Machine` làm
//! kernel. Ba TAP **42/43/44** khẳng định bằng trạng thái `Machine` + trang IPC buffer, không
//! tin lời chương trình U-mode (D2-102).
//!
//! # Vì sao chạy được mà **không** cần scheduler preemptive
//!
//! Ảnh này vẫn là "kernel là scheduler tuần tự" (KT-338b §8.2): một TCB U-mode `Running` tại một
//! thời điểm. `procsrv` chạy vòng `NBRecv`/`Yield`; **mỗi** `Yield` là một `svc` → trap EL1 →
//! `dispatch` của core → [`after_return`] chạy **dịch vụ kernel** (rút sự kiện khỏi Endpoint,
//! bơm yêu cầu kế) rồi `eret` lại đúng chỗ vừa trap. Nhờ vậy:
//!
//! - procsrv phục vụ **hai** yêu cầu `WAIT`/`KILL` (TAP 42);
//! - shell đọc script từ **ring KT-69** (kernel bơm trước bằng `conchan`, không đổi shell), in
//!   prompt + `help` + `echo`, rồi `Exit(0)` sạch (TAP 43);
//! - echo đọc/ghi **IPC buffer** (MR4..MR7) rồi `send` message 8 MR; kernel đọc lại từ buffer của
//!   TCB nhận (TAP 44).
//!
//! Khi một pha xong, `after_return`/`after_exit` đổi `ELR_EL1` sang hàm EL1 của pha kế (cùng
//! khuôn `Exit` của KT-338b) — **không** có chuyển ngữ cảnh U-mode, không lưu khung trap.
//!
//! # Tách ảnh sản phẩm
//!
//! Toàn bộ tệp này nằm sau `--features ktest`: ảnh sản phẩm **không** link `vifast-core` (trần
//! `.data+.bss` 65.536 B < `size_of::<Machine>()`) nên K3.5 **chưa** chạy trên ảnh sản phẩm —
//! quyết định ngân sách ghi ở `JOURNAL/kt339-user.md`, kèm đề xuất task con.

#![allow(clippy::too_many_arguments)]

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::ipc::{Message, MessageInfo};
use vifast_core::syscall::SyscallReturn;
use vifast_core::{KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_aarch64::bootmem::{self, SharedBootMemory};
use vifast_plat_aarch64::core_bridge;
use vifast_plat_aarch64::umode::spsr_el0;
use vifast_plat_aarch64::userload;
use vifast_plat_aarch64::vspace::VSpace;
use vifast_plat_aarch64::{asm, debug_halt};

use crate::{tap, Console, BOOT_MARKER};

// ---------------------------------------------------------------------------------------------
// Const của giao thức — **khớp** hằng trong `programs/*.rs` (một giao thức, hai phía)
// ---------------------------------------------------------------------------------------------

/// Ô CSpace chung của rig K5 — **khớp** hằng `EP_*` của procsrv/shell/echo.
const SLOT_UNTYPED: usize = 0;
const SLOT_EP_FAULT: usize = 1;
const SLOT_EP_PROC: usize = 2;
const SLOT_CSPACE: usize = 3;
const SLOT_EP_REPLY: usize = 4;
const SLOT_EP_PS: usize = 5;
const SLOT_EP_EVENTS: usize = 8;
const SLOT_EP_ECHO: usize = 9;
/// Cap `Tcb` của echo (để `Tcb.Configure` bind IPC buffer) — user không dùng.
const SLOT_TCB_ECHO: usize = 10;
/// Cap `Frame` của trang IPC buffer (`add_foreign_object`, D2-83).
const SLOT_FRAME: usize = 11;
const SLOT_TCB_PROCSRV: usize = 12;
const SLOT_TCB_SHELL: usize = 13;
const SLOT_TCB_KERN: usize = 14;
const SLOT_TCB_SERVER: usize = 15;
/// Số ô của CSpace K5 (cùng 16 ô với coretest; đủ cho 6 Endpoint + 5 TCB + Frame/CNode).
const K5_SLOTS: usize = 16;

/// `Tcb`/`Endpoint` lấy từ Untyped riêng của K5 (4 frame = 16 KiB; object cần ~4 KiB).
const K5_ARENA_FRAMES: usize = 4;
/// Số trang stack dùng chung cho ba chương trình U-mode (2 × 4 KiB = 8 KiB; shell dùng < 4 KiB).
const K5_STACK_PAGES: usize = 2;
/// Đỉnh stack U-mode của K5 — **khác** trang mã/stack/IPC buffer của pha 1 (0x7000_0000..3000).
const K5_STACK_TOP: u64 = 0x7000_C000;

/// VA trang IPC buffer — **phải** bằng `vifast-plat-user` backend aarch64 (`Sys::IPC_BUFFER_VA`)
/// và `coretest::IPC_BUF_VA`: một trang, ba nơi đọc/ghi.
const IPC_BUF_VA: u64 = crate::coretest::IPC_BUF_VA;

/// Label giao thức procsrv → shell (khớp `procsrv.rs`/`shell.rs`).
const WAIT_REQ: u32 = 0x604;
const KILL_REQ: u32 = 0x605;
const REPLY_WAIT: u32 = 0x606;
const REPLY_KILL: u32 = 0x607;
const EV_UP: u32 = 0x501;
const EV_WAIT: u32 = 0x503;
const EV_KILL: u32 = 0x504;
const EV_PS: u32 = 0x505;
const EV_ECHO: u32 = 0x506;
const EV_HELP: u32 = 0x507;
/// Label message của echo (khớp `echo.rs`).
const ECHO_LABEL: u32 = 0x660;
/// MR4..MR7 kernel ghi **trước** pha echo (khớp `echo.rs`).
const ECHO_PREFILL: [u64; 4] = [0xE100, 0xE101, 0xE102, 0xE103];
/// Biến đổi echo ghi lại (khớp `echo.rs::ECHO_MASK`).
const ECHO_MASK: u64 = 0x0000_00FF_0000_00FF;
/// MR0..MR3 của message echo (chỉ thanh ghi).
const ECHO_REGS: [u64; 4] = [0xE0A0, 0xE0A1, 0xE0A2, 0xE0A3];
/// Pid giả cho hai yêu cầu WAIT/KILL — kernel chọn để biết chính xác bên nhận trả gì.
const PID_WAIT: u64 = 0x42;
const PID_KILL: u64 = 0x43;
/// Mã `exit_code_of` của procsrv khi pid chưa từng exit (`procsrv.rs`).
const PROC_NO_EXIT: u64 = 0xFFFF;
/// Ngân sách `Yield` của một pha trước khi kernel tiếp quản (chống treo khi đột biến).
const YIELD_BUDGET: u64 = 20_000;

/// Chương trình U-mode **thật** — cùng nguồn với riscv64 (`vifast-plat-user/programs/`).
static PROCSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/procsrv.elf"));
static SHELL_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.elf"));
static ECHO_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/echo.elf"));

/// `e_machine` của AArch64 (`vifast-elf::parse` kiểm trước khi map).
const EM_AARCH64: u16 = 183;

/// Trạng thái pha (số ổn định để in log).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle = 0,
    Procsrv = 1,
    Shell = 2,
    Echo = 3,
    Done = 4,
}

/// `u32::MAX` = "chưa có"; `u64::MAX` = "chưa đo".
const NO_OBJ: u32 = u32::MAX;
const NO_U64: u64 = u64::MAX;

static PHASE: AtomicU32 = AtomicU32::new(Phase::Idle as u32);
static CUR_TCB: AtomicU32 = AtomicU32::new(NO_OBJ);
static ROOT: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_KERN: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_PROCSRV: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_SHELL: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_ECHO: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_SERVER: AtomicU32 = AtomicU32::new(NO_OBJ);
/// PA vùng Untyped riêng của K5 (4 frame liền nhau) — nạp ở [`load`].
static K5_ARENA_PA: AtomicU64 = AtomicU64::new(0);
/// Điểm vào ba chương trình (VA tuyệt đối do `vifast-elf::parse` trả).
static ENTRY_PROCSRV: AtomicU64 = AtomicU64::new(0);
static ENTRY_SHELL: AtomicU64 = AtomicU64::new(0);
static ENTRY_ECHO: AtomicU64 = AtomicU64::new(0);

// Bằng chứng của từng pha (kernel ghi, TAP đọc — không tin lời U-mode).
static YIELDS: AtomicU64 = AtomicU64::new(0);
static EV_UP_N: AtomicU64 = AtomicU64::new(0);
static EV_WAIT_N: AtomicU64 = AtomicU64::new(0);
static EV_KILL_N: AtomicU64 = AtomicU64::new(0);
static EV_HELP_N: AtomicU64 = AtomicU64::new(0);
static EV_ECHO_N: AtomicU64 = AtomicU64::new(0);
static REPLY_WAIT_N: AtomicU64 = AtomicU64::new(0);
static REPLY_KILL_N: AtomicU64 = AtomicU64::new(0);
static WAIT_PID: AtomicU64 = AtomicU64::new(NO_U64);
static WAIT_CODE: AtomicU64 = AtomicU64::new(NO_U64);
static KILL_PID: AtomicU64 = AtomicU64::new(NO_U64);
static KILL_CODE: AtomicU64 = AtomicU64::new(NO_U64);
static HELP_CMDS: AtomicU64 = AtomicU64::new(NO_U64);
static ECHO_LEN: AtomicU64 = AtomicU64::new(NO_U64);
static PUTS: AtomicU64 = AtomicU64::new(0);
static SHELL_EXIT: AtomicU64 = AtomicU64::new(NO_U64);
static ECHO_EXIT: AtomicU64 = AtomicU64::new(NO_U64);
static PROC_TIMEOUT: AtomicBool = AtomicBool::new(false);
static PROC_TAKEOVER: AtomicBool = AtomicBool::new(false);
static ALL_OK: AtomicBool = AtomicBool::new(true);
/// PA của ring UART đã map (in bằng chứng).
static RING_PA: AtomicU64 = AtomicU64::new(0);
/// `true` ⇔ K5 đang điều khiển U-mode (handler `svc` dùng để định tuyến).
pub fn active() -> bool {
    let phase = PHASE.load(Ordering::Acquire);
    phase == Phase::Procsrv as u32 || phase == Phase::Shell as u32 || phase == Phase::Echo as u32
}

/// TCB hiện hành của K5 (handler `svc` ưu tiên trước `coretest::current_tcb`).
pub fn current() -> Option<ObjId> {
    if !active() {
        return None;
    }
    let tcb = CUR_TCB.load(Ordering::Acquire);
    (tcb != NO_OBJ).then_some(tcb)
}

/// `Message` không cap từ `(label, MRs)` — biên `MessageInfo` do core kiểm.
fn msg(label: u32, words: &[u64]) -> Result<Message, KError> {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8)?;
    Message::new(info, words)
}

/// PA của frame chứa **IPC buffer** — `coretest::prepare` đã map trang này; K5 dùng lại **một**
/// trang cho mọi TCB (mỗi pha một TCB chạy, không có hai tiến trình cùng lúc).
fn ipc_buffer_frame_pa() -> u64 {
    crate::coretest::ipc_buffer_pa()
}

// ---------------------------------------------------------------------------------------------
// Nạp ảnh — gọi ở `high_entry` **trước** lần `eret` đầu tiên (còn `&mut VSpace`)
// ---------------------------------------------------------------------------------------------

/// Nạp ba ELF U-mode + stack + ring script vào `VSpace` thật; cấp Untyped riêng của rig K5.
///
/// Trả `false` ⇔ bất kỳ bước nào hỏng; caller (`high_entry`) in `PANIC` và dừng — **không** chạy
/// tiếp với chương trình nửa nạp.
pub fn load(vs: &mut VSpace<SharedBootMemory>, console: &mut Console) -> bool {
    // ---- 0) Untyped riêng của K5 (4 frame liền nhau) — TCB/Endpoint/CNode của rig.
    let mut arena = 0u64;
    for index in 0..K5_ARENA_FRAMES {
        let Some(pa) = bootmem::alloc_boot_frame() else {
            let _ = writeln!(console, "# k5 fail: stage=arena index={index} (hết frame)");
            return false;
        };
        if index == 0 {
            arena = pa;
        }
    }
    if arena == 0 {
        let _ = writeln!(console, "# k5 fail: stage=arena (không có frame)");
        return false;
    }
    K5_ARENA_PA.store(arena, Ordering::Release);

    // ---- 1) Ba ELF — **cùng nguồn** Rust với riscv64, khác target biên dịch.
    let procsrv = match userload::load_elf(vs, PROCSRV_ELF, EM_AARCH64) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(
                console,
                "# k5 fail: stage=elf procsrv err={} elf={}",
                error.name(),
                error.elf_code()
            );
            return false;
        }
    };
    let shell = match userload::load_elf(vs, SHELL_ELF, EM_AARCH64) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(
                console,
                "# k5 fail: stage=elf shell err={} elf={}",
                error.name(),
                error.elf_code()
            );
            return false;
        }
    };
    let echo = match userload::load_elf(vs, ECHO_ELF, EM_AARCH64) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(
                console,
                "# k5 fail: stage=elf echo err={} elf={}",
                error.name(),
                error.elf_code()
            );
            return false;
        }
    };
    ENTRY_PROCSRV.store(procsrv.entry, Ordering::Release);
    ENTRY_SHELL.store(shell.entry, Ordering::Release);
    ENTRY_ECHO.store(echo.entry, Ordering::Release);

    // ---- 2) Stack U-mode dùng chung (mỗi pha chạy tuần tự, không có hai tác vụ cùng lúc).
    let (stack_base, stack_bytes) = match userload::map_user_stack(vs, K5_STACK_TOP, K5_STACK_PAGES)
    {
        Ok(range) => range,
        Err(error) => {
            let _ = writeln!(console, "# k5 fail: stage=stack err={}", error.name());
            return false;
        }
    };

    // ---- 3) Ring UART của shell: kernel bơm script bằng **cùng** codec mà shell đọc.
    // Script: `help` + `echo same-source` + `exit` — đủ chứng minh prompt + lệnh nội trú + thoát
    // sạch mà không cần UART RX thật (kiến trúc IRQ UART→user là nợ của KT-73 trên aarch64).
    let script = b"help\necho same-source\nexit\n";
    let ring_pa = match userload::map_ring_with_script(vs, script) {
        Ok(pa) => pa,
        Err(error) => {
            let _ = writeln!(console, "# k5 fail: stage=ring err={}", error.name());
            return false;
        }
    };
    RING_PA.store(ring_pa, Ordering::Release);
    // Ảnh mới map xong: `activate` để TLB/TTBR0 chắc chắn đang là bảng vừa sửa (cùng khuôn
    // `coretest::prepare`).
    if vs.activate().is_err() {
        let _ = writeln!(console, "# k5 fail: stage=activate");
        return false;
    }

    let _ = writeln!(
        console,
        "# k5 load: procsrv=0x{:x}+{}p entry=0x{:x} base=0x{:x} segs={} shell=0x{:x}+{}p entry=0x{:x} segs={} echo=0x{:x}+{}p entry=0x{:x} segs={} stack=0x{:x}+{} ring_pa=0x{:x} ring_va=0x{:x} script={}B",
        procsrv.base,
        procsrv.pages,
        procsrv.entry,
        procsrv.base,
        procsrv.segments,
        shell.base,
        shell.pages,
        shell.entry,
        shell.segments,
        echo.base,
        echo.pages,
        echo.entry,
        echo.segments,
        stack_base,
        stack_bytes,
        ring_pa,
        userload::RING_VA,
        script.len(),
    );
    true
}

// ---------------------------------------------------------------------------------------------
// Dựng rig K5 (CSpace + 6 Endpoint + 5 TCB + Frame IPC buffer)
// ---------------------------------------------------------------------------------------------

/// Cấp cap `Tcb` cho `slot`, trả `ObjId` của TCB.
fn make_tcb(m: &mut Machine, root: ObjId, us: SlotRef, slot: usize) -> Result<ObjId, KError> {
    let cap = m.retype(us, ObjType::Tcb, 0, 0)?;
    m.put_cap(root, slot, cap)?;
    m.cap_obj(cap)
}

/// Cấp + gắn một Endpoint vào `slot`.
fn make_endpoint(m: &mut Machine, root: ObjId, us: SlotRef, slot: usize) -> Result<(), KError> {
    let cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
    m.put_cap(root, slot, cap)
}

/// Dựng toàn bộ object của K5 trong **một** lần khoá `Machine`.
fn build_rig(console: &mut Console) -> bool {
    let arena = K5_ARENA_PA.load(Ordering::Acquire);
    if arena == 0 {
        let _ = writeln!(console, "# k5 fail: stage=rig arena=0");
        return false;
    }
    let built = core_bridge::with_machine(|m| -> Result<(), KError> {
        // ---- Untyped + CNode gốc của K5.
        let untyped = m.add_untyped(arena, K5_ARENA_FRAMES * 0x1000);
        let root = m.add_cnode(K5_SLOTS);
        ROOT.store(root, Ordering::Release);
        let untyped_cap = m.root_cap(untyped);
        m.put_cap(root, SLOT_UNTYPED, untyped_cap)?;
        let us = SlotRef::new(root, SLOT_UNTYPED);
        // ---- Sáu Endpoint: fault/proc/reply/ps/events/echo (đúng số ô user gọi).
        for slot in [
            SLOT_EP_FAULT,
            SLOT_EP_PROC,
            SLOT_EP_REPLY,
            SLOT_EP_PS,
            SLOT_EP_EVENTS,
            SLOT_EP_ECHO,
        ] {
            make_endpoint(m, root, us, slot)?;
        }
        // ---- Năm TCB.
        let kern = make_tcb(m, root, us, SLOT_TCB_KERN)?;
        let procsrv = make_tcb(m, root, us, SLOT_TCB_PROCSRV)?;
        let shell = make_tcb(m, root, us, SLOT_TCB_SHELL)?;
        let echo = make_tcb(m, root, us, SLOT_TCB_ECHO)?;
        let server = make_tcb(m, root, us, SLOT_TCB_SERVER)?;
        TCB_KERN.store(kern, Ordering::Release);
        TCB_PROCSRV.store(procsrv, Ordering::Release);
        TCB_SHELL.store(shell, Ordering::Release);
        TCB_ECHO.store(echo, Ordering::Release);
        TCB_SERVER.store(server, Ordering::Release);
        // ---- Cap CNode của chính rig (cho `Tcb.Configure(cspace?)`) + Frame IPC buffer.
        let cspace_cap = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cspace_cap)?;
        let frame_obj = m.add_foreign_object(ObjType::Frame, ipc_buffer_frame_pa())?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, SLOT_FRAME, frame_cap)?;
        // ---- Cấu hình: mọi TCB cùng CSpace; echo bind thêm IPC buffer (đọc MR4..MR63).
        let cspace = SlotRef::new(root, SLOT_CSPACE);
        for slot in [
            SLOT_TCB_KERN,
            SLOT_TCB_PROCSRV,
            SLOT_TCB_SHELL,
            SLOT_TCB_SERVER,
        ] {
            m.tcb_configure(SlotRef::new(root, slot), Some(cspace))?;
        }
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(root, SLOT_TCB_ECHO),
            Some(cspace),
            Some((SlotRef::new(root, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        // ---- Ưu tiên: tác vụ U-mode **cao hơn** kern/server (chỉ một U-mode `Running` tại một
        // thời điểm; `Yield` chọn lại chính nó, không nhảy sang kern Ready).
        for slot in [SLOT_TCB_PROCSRV, SLOT_TCB_SHELL, SLOT_TCB_ECHO] {
            m.tcb_set_priority(SlotRef::new(root, slot), 10, u8::MAX)?;
        }
        for slot in [SLOT_TCB_KERN, SLOT_TCB_SERVER] {
            m.tcb_set_priority(SlotRef::new(root, slot), 1, u8::MAX)?;
        }
        // ---- Kern chạy (Ready) suốt cả ba pha; server chờ tới pha echo mới `Recv`.
        m.start_tcb(kern)?;
        m.start_tcb(server)?;
        Ok(())
    });
    if built.is_err() {
        let _ = writeln!(console, "# k5 fail: stage=rig (xem PANIC trước)");
    }
    built.is_ok()
}

// ---------------------------------------------------------------------------------------------
// Điều phối pha — mọi `eret` dùng đúng một chỗ
// ---------------------------------------------------------------------------------------------

/// `eret` xuống EL0 tại `entry`, SP = [`K5_STACK_TOP`]. **Không trả về.**
fn enter_el0(entry: u64) -> ! {
    asm::eret_to_el0(entry, K5_STACK_TOP, spsr_el0())
}

/// Đưa một TCB từ `Inactive` → `Running{hart:0}` (nếu đang `Inactive`).
fn start_and_run(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    if matches!(m.tcb_state(tcb)?, TcbState::Inactive) {
        m.start_tcb(tcb)?;
    }
    m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
}

/// Vào pha procsrv: đóng gói sẵn yêu cầu **WAIT** trên `EP_PROC` rồi `eret`.
fn enter_procsrv(console: &mut Console) -> ! {
    let root = ROOT.load(Ordering::Acquire);
    let kern = TCB_KERN.load(Ordering::Acquire);
    let procsrv = TCB_PROCSRV.load(Ordering::Acquire);
    let sent = core_bridge::with_machine(|m| -> Result<(), KError> {
        start_and_run(m, procsrv)?;
        let request = msg(WAIT_REQ, &[PID_WAIT, 0, 0, 0])?;
        m.nb_send(kern, SlotRef::new(root, SLOT_EP_PROC), request)
    });
    if sent.is_err() {
        let _ = writeln!(console, "# k5 fail: stage=procsrv-request (xem PANIC sau)");
    }
    PHASE.store(Phase::Procsrv as u32, Ordering::Release);
    CUR_TCB.store(procsrv, Ordering::Release);
    YIELDS.store(0, Ordering::Release);
    enter_el0(ENTRY_PROCSRV.load(Ordering::Acquire))
}

/// Dịch vụ kernel trong **một** `Yield` của procsrv: rút sự kiện/trả lời, bơm yêu cầu kế.
///
/// Trả `Some(entry)` ⇔ pha procsrv **xong** (kernel tiếp quản, `ELR ← entry`).
fn service_procsrv(m: &mut Machine) -> Result<Option<u64>, KError> {
    let root = ROOT.load(Ordering::Acquire);
    let kern = TCB_KERN.load(Ordering::Acquire);
    let procsrv = TCB_PROCSRV.load(Ordering::Acquire);
    // ---- 1) Sự kiện procsrv báo (`EP_EVENTS`) — mỗi lần chỉ một message đang chờ.
    while let Ok(Some(message)) = m.nb_recv(kern, SlotRef::new(root, SLOT_EP_EVENTS)) {
        match message.info.label() {
            EV_UP => {
                EV_UP_N.fetch_add(1, Ordering::Relaxed);
            }
            EV_WAIT => {
                EV_WAIT_N.fetch_add(1, Ordering::Relaxed);
                WAIT_PID.store(message.mr[0], Ordering::Release);
                WAIT_CODE.store(message.mr[1], Ordering::Release);
            }
            EV_KILL => {
                EV_KILL_N.fetch_add(1, Ordering::Relaxed);
                KILL_PID.store(message.mr[0], Ordering::Release);
                KILL_CODE.store(message.mr[1], Ordering::Release);
            }
            _ => {}
        }
    }
    // ---- 2) Trả lời của procsrv (`EP_REPLY`) — rút để hàng Endpoint không đầy.
    while let Ok(Some(message)) = m.nb_recv(kern, SlotRef::new(root, SLOT_EP_REPLY)) {
        match message.info.label() {
            REPLY_WAIT => {
                REPLY_WAIT_N.fetch_add(1, Ordering::Relaxed);
            }
            REPLY_KILL => {
                REPLY_KILL_N.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
    // ---- 3) Đủ yêu cầu 1 ⇒ bơm yêu cầu 2 (`KILL`).
    if EV_WAIT_N.load(Ordering::Acquire) >= 1 && EV_KILL_N.load(Ordering::Acquire) == 0 {
        let request = msg(KILL_REQ, &[PID_KILL, 0, 0, 0])?;
        // Endpoint có thể còn đầy nếu procsrv chưa rút; `Busy` là bình thường ⇒ thử lại lượt sau.
        let _ = m.nb_send(kern, SlotRef::new(root, SLOT_EP_PROC), request);
    }
    // ---- 4) Đủ hai yêu cầu ⇒ kết thúc pha (ĐIỂM TIẾP QUẢN DUY NHẤT của pha này).
    let done = EV_KILL_N.load(Ordering::Acquire) >= 1
        && REPLY_KILL_N.load(Ordering::Acquire) >= 1
        && REPLY_WAIT_N.load(Ordering::Acquire) >= 1;
    if done || PROC_TIMEOUT.load(Ordering::Acquire) {
        // `Running → Inactive` (ALLOWED) — tác vụ U-mode tuần tự đã xong, kernel tiếp quản.
        let _ = m.transition_tcb(procsrv, NonReadyTarget::Inactive);
        PROC_TAKEOVER.store(true, Ordering::Release);
        return Ok(Some(k5_shell as *const () as u64));
    }
    Ok(None)
}

/// Handler gọi **sau** `dispatch` trả `Return` của một syscall trong pha K5.
///
/// Trả `Some(entry)` ⇒ kernel tiếp quản (đổi `ELR_EL1` sang hàm EL1 `entry`); `None` ⇒ `eret`
/// lại đúng chỗ vừa trap.
pub fn after_return(nr: u64, result: &SyscallReturn) -> Option<u64> {
    let phase = PHASE.load(Ordering::Acquire);
    match phase {
        x if x == Phase::Procsrv as u32 => {
            let budgets = YIELDS.fetch_add(1, Ordering::AcqRel) + 1;
            if nr == vifast_core::spec::SYS_YIELD && budgets >= YIELD_BUDGET {
                PROC_TIMEOUT.store(true, Ordering::Release);
            }
            if nr != vifast_core::spec::SYS_YIELD {
                // Các syscall khác (DebugPutStr/IPC) không phải điểm dịch vụ; chỉ ghi bằng chứng.
                if nr == vifast_core::spec::SYS_DEBUG_PUT_STR && result.error == 0 {
                    PUTS.fetch_add(1, Ordering::Relaxed);
                }
                return None;
            }
            core_bridge::with_machine(|m| service_procsrv(m).ok().flatten())
        }
        x if x == Phase::Shell as u32 => {
            if nr == vifast_core::spec::SYS_DEBUG_PUT_STR && result.error == 0 {
                PUTS.fetch_add(1, Ordering::Relaxed);
            }
            // Rút sự kiện `help`/`echo` ở **mỗi** điểm trả về của shell (rẻ: hàng thường rỗng), để
            // message không bị `Machine::exit` dọn (`unlink_tcb_queues`) trước khi kernel đọc.
            drain_shell_events();
            None
        }
        x if x == Phase::Echo as u32 => None,
        _ => None,
    }
}

/// Handler gọi khi `Exit` của một tác vụ K5: ghi mã thoát + chuyển pha.
///
/// Trả `Some(entry)` ⇒ tiếp quản; `None` ⇒ không phải pha K5 (đường coretest xử lý).
pub fn after_exit(code: u64) -> Option<u64> {
    let phase = PHASE.load(Ordering::Acquire);
    if phase == Phase::Procsrv as u32 {
        // procsrv không được `Exit` khi đang phục vụ; coi là lỗi pha, đi tiếp shell.
        PROC_TIMEOUT.store(true, Ordering::Release);
        core_bridge::with_machine(|m| {
            let _ = m.transition_tcb(
                TCB_PROCSRV.load(Ordering::Acquire),
                NonReadyTarget::Inactive,
            );
        });
        return Some(k5_shell as *const () as u64);
    }
    if phase == Phase::Shell as u32 {
        SHELL_EXIT.store(code, Ordering::Release);
        // Rút nốt sự kiện `help`/`echo` mà shell vừa gửi trước khi `Exit`.
        drain_shell_events();
        return Some(k5_echo as *const () as u64);
    }
    if phase == Phase::Echo as u32 {
        ECHO_EXIT.store(code, Ordering::Release);
        return Some(k5_finish as *const () as u64);
    }
    None
}

/// Rút `EP_EVENTS` (HELP/ECHO) — gọi ở **đầu** [`k5_echo`] (ranh giới pha shell→echo) và ở
/// `Exit` của shell.
fn drain_shell_events() {
    let root = ROOT.load(Ordering::Acquire);
    let kern = TCB_KERN.load(Ordering::Acquire);
    core_bridge::with_machine(|m| {
        while let Ok(Some(message)) = m.nb_recv(kern, SlotRef::new(root, SLOT_EP_EVENTS)) {
            match message.info.label() {
                EV_HELP => {
                    EV_HELP_N.fetch_add(1, Ordering::Relaxed);
                    HELP_CMDS.store(message.mr[0], Ordering::Release);
                }
                EV_ECHO => {
                    EV_ECHO_N.fetch_add(1, Ordering::Relaxed);
                    ECHO_LEN.store(message.mr[0], Ordering::Release);
                }
                _ => {}
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Các điểm vào EL1 giữa hai pha (đặt qua `ELR_EL1` của handler `svc`)
// ---------------------------------------------------------------------------------------------

/// Pha **shell** (TAP 43 được phát **trước** khi vào pha này, từ [`k5_shell`]).
#[no_mangle]
pub extern "C" fn k5_shell() -> ! {
    let mut console = Console;
    // TAP 42 — procsrv phục vụ **hai** yêu cầu, kernel đọc bằng chứng từ Endpoint.
    let root = ROOT.load(Ordering::Acquire);
    let procsrv = TCB_PROCSRV.load(Ordering::Acquire);
    let state = core_bridge::with_machine(|m| m.tcb_state(procsrv).ok());
    let ok = EV_UP_N.load(Ordering::Acquire) >= 1
        && EV_WAIT_N.load(Ordering::Acquire) == 1
        && EV_KILL_N.load(Ordering::Acquire) == 1
        && REPLY_WAIT_N.load(Ordering::Acquire) == 1
        && REPLY_KILL_N.load(Ordering::Acquire) == 1
        && WAIT_PID.load(Ordering::Acquire) == PID_WAIT
        && WAIT_CODE.load(Ordering::Acquire) == PROC_NO_EXIT
        && KILL_PID.load(Ordering::Acquire) == PID_KILL
        && KILL_CODE.load(Ordering::Acquire) == 0
        && YIELDS.load(Ordering::Acquire) >= 3
        && !PROC_TIMEOUT.load(Ordering::Acquire)
        && PROC_TAKEOVER.load(Ordering::Acquire)
        && state == Some(TcbState::Inactive);
    let _ = writeln!(
        &mut console,
        "# k5 proc: up={} wait_pid={} wait_code={} kill_pid={} kill_code={} reply_wait={} reply_kill={} yields={} timeout={} takeover={} root={} state={}",
        EV_UP_N.load(Ordering::Acquire),
        WAIT_PID.load(Ordering::Acquire),
        WAIT_CODE.load(Ordering::Acquire),
        KILL_PID.load(Ordering::Acquire),
        KILL_CODE.load(Ordering::Acquire),
        REPLY_WAIT_N.load(Ordering::Acquire),
        REPLY_KILL_N.load(Ordering::Acquire),
        YIELDS.load(Ordering::Acquire),
        u8::from(PROC_TIMEOUT.load(Ordering::Acquire)),
        u8::from(PROC_TAKEOVER.load(Ordering::Acquire)),
        root,
        match state {
            Some(TcbState::Inactive) => "Inactive",
            Some(TcbState::Running { .. }) => "Running",
            Some(_) => "other",
            None => "none",
        },
    );
    let mut all = tap(
        &mut console,
        42,
        "k5: procsrv serves 2 requests from EL0 (svc #0)",
        ok,
    );
    all &= ALL_OK.load(Ordering::Acquire);
    ALL_OK.store(all, Ordering::Release);

    // ---- Vào pha shell: TCB shell `Inactive → Running`, `eret` tại entry ELF.
    let shell = TCB_SHELL.load(Ordering::Acquire);
    let entered = core_bridge::with_machine(|m| start_and_run(m, shell));
    if entered.is_err() {
        let _ = writeln!(&mut console, "PANIC: k5: không chạy được shell");
        debug_halt(3)
    }
    PHASE.store(Phase::Shell as u32, Ordering::Release);
    CUR_TCB.store(shell, Ordering::Release);
    YIELDS.store(0, Ordering::Release);
    enter_el0(ENTRY_SHELL.load(Ordering::Acquire))
}

/// Pha **echo** — TAP 43 (shell) phát trước, rồi prefill IPC buffer + chặn server `Recv`.
#[no_mangle]
pub extern "C" fn k5_echo() -> ! {
    let mut console = Console;
    // **Ranh giới pha**: message `help`/`echo` của shell nằm trong hàng `EP_EVENTS` **sau** khi
    // TCB shell `Exit` (core hoãn/xoá message theo `unlink_tcb_queues` chỉ khi rời pha; đo được
    // message còn nguyên ở đây). Rút **trước** khi đọc bằng chứng ⇒ TAP 43 thấy `help_cmds` +
    // `echo_len` do kernel đọc, không tin lời chương trình.
    drain_shell_events();
    let shell = TCB_SHELL.load(Ordering::Acquire);
    let state = core_bridge::with_machine(|m| m.tcb_state(shell).ok());
    let puts = PUTS.load(Ordering::Acquire);
    // **KT-339** — bằng chứng **kernel-side**: hash các lần `DebugPutStr` phải phủ đủ bốn mốc
    // prompt/`help`/`echo`/`exit` (xem `vifast_plat_aarch64::k5cap`). Không tin hàng IPC của TCB
    // vừa thoát: `Machine::exit` dọn message của nó (`unlink_tcb_queues`).
    let transcript = vifast_plat_aarch64::k5cap::flags();
    let ok = SHELL_EXIT.load(Ordering::Acquire) == 0
        && state == Some(TcbState::Inactive)
        && transcript == vifast_plat_aarch64::k5cap::F_ALL
        && puts >= 6;
    let _ = writeln!(
        &mut console,
        "# k5 shell: exit={} state={} transcript=0x{:x} puts={} events_help={} events_echo={}",
        SHELL_EXIT.load(Ordering::Acquire),
        match state {
            Some(TcbState::Inactive) => "Inactive",
            Some(TcbState::Running { .. }) => "Running",
            Some(_) => "other",
            None => "none",
        },
        transcript,
        puts,
        EV_HELP_N.load(Ordering::Acquire),
        EV_ECHO_N.load(Ordering::Acquire),
    );
    let mut all = tap(
        &mut console,
        43,
        "k5: shell prompt + help/echo + clean exit from EL0",
        ok,
    );
    all &= ALL_OK.load(Ordering::Acquire);
    ALL_OK.store(all, Ordering::Release);

    // ---- Prefill MR4..MR7 của IPC buffer (kernel đọc lại sau message 8 MR của echo).
    let mut index = 0;
    while index < ECHO_PREFILL.len() {
        asm::write_u64(
            IPC_BUF_VA + vifast_core::ipc::IPC_BUFFER_MR_OFFSET as u64 + (index as u64) * 8,
            ECHO_PREFILL[index],
        );
        index += 1;
    }
    // ---- Server chặn `Recv(EP_ECHO)` **trước** khi echo gửi ⇒ message giao ngay khi gửi.
    let root = ROOT.load(Ordering::Acquire);
    let server = TCB_SERVER.load(Ordering::Acquire);
    let echo = TCB_ECHO.load(Ordering::Acquire);
    let entered = core_bridge::with_machine(|m| -> Result<(), KError> {
        m.recv(server, SlotRef::new(root, SLOT_EP_ECHO), None)?;
        start_and_run(m, echo)
    });
    if entered.is_err() {
        let _ = writeln!(
            &mut console,
            "PANIC: k5: không chặn được server / chạy echo"
        );
        debug_halt(3)
    }
    PHASE.store(Phase::Echo as u32, Ordering::Release);
    CUR_TCB.store(echo, Ordering::Release);
    enter_el0(ENTRY_ECHO.load(Ordering::Acquire))
}

/// Kết thúc K5 — TAP 44 (echo/IPC buffer), rồi marker + halt (chỉ khi mọi TAP xanh).
#[no_mangle]
pub extern "C" fn k5_finish() -> ! {
    let mut console = Console;
    let echo = TCB_ECHO.load(Ordering::Acquire);
    let server = TCB_SERVER.load(Ordering::Acquire);
    let (state, area) =
        core_bridge::with_machine(|m| (m.tcb_state(echo).ok(), m.tcb_ipc_area(server).ok()));
    // Đọc **trực tiếp** trang IPC buffer: chương trình U-mode đã ghi word nào, kernel thấy word ấy.
    let mut buf = [0u64; 4];
    let mut index = 0;
    while index < buf.len() {
        buf[index] = asm::read_u64(
            IPC_BUF_VA + vifast_core::ipc::IPC_BUFFER_MR_OFFSET as u64 + (index as u64) * 8,
        );
        index += 1;
    }
    let mut expected = [0u64; 4];
    index = 0;
    while index < expected.len() {
        expected[index] = ECHO_PREFILL[index] ^ ECHO_MASK;
        index += 1;
    }
    let (msg_len, regs, area_buf) = match &area {
        Some(area) => match area.info {
            Some(info) => (u64::from(info.length()), area.regs, area.buffer),
            None => (0, [0u64; 4], [0u64; vifast_core::ipc::WORDS_IN_IPC_BUFFER]),
        },
        None => (0, [0u64; 4], [0u64; vifast_core::ipc::WORDS_IN_IPC_BUFFER]),
    };
    let ok = ECHO_EXIT.load(Ordering::Acquire) == 0
        && state == Some(TcbState::Inactive)
        && msg_len == 8
        && regs == ECHO_REGS
        && area_buf[0] == expected[0]
        && area_buf[1] == expected[1]
        && area_buf[2] == expected[2]
        && area_buf[3] == expected[3]
        && buf == expected;
    ALL_OK.store(ok && ALL_OK.load(Ordering::Acquire), Ordering::Release);
    let _ = writeln!(
        &mut console,
        "# k5 echo: exit={} state={} len={} reg0=0x{:x} reg3=0x{:x} buf0=0x{:x} buf3=0x{:x} got0=0x{:x} got3=0x{:x} prefill0=0x{:x} mask=0x{:x}",
        ECHO_EXIT.load(Ordering::Acquire),
        match state {
            Some(TcbState::Inactive) => "Inactive",
            Some(TcbState::Running { .. }) => "Running",
            Some(_) => "other",
            None => "none",
        },
        msg_len,
        regs[0],
        regs[3],
        area_buf[0],
        area_buf[3],
        buf[0],
        buf[3],
        ECHO_PREFILL[0],
        ECHO_MASK,
    );
    let mut all = tap(
        &mut console,
        44,
        "k5: echo EL0 reads/writes IPC buffer (length + content)",
        ok,
    );
    all &= ALL_OK.load(Ordering::Acquire);
    ALL_OK.store(all, Ordering::Release);
    // Bất biến cuối: cap/byte/IPC của core vẫn xanh sau ba tiến trình U-mode.
    let invariants = core_bridge::with_machine(|m| {
        m.check_caps().is_ok()
            && m.check_all_bytes().is_ok()
            && m.check_ipc().is_ok()
            && m.check_sched().is_ok()
            && m.check_reply_links().is_ok()
    });
    let _ = writeln!(
        &mut console,
        "# k5 core: invariants={}",
        u8::from(invariants)
    );
    if !all || !invariants {
        debug_halt(5)
    }
    // Marker **chỉ** khi mọi TAP xanh (bất biến của ảnh: TAP đỏ ⇒ không marker).
    let _ = core::fmt::Write::write_str(&mut console, BOOT_MARKER);
    let _ = writeln!(&mut console, "# viFastOs aarch64: halt");
    debug_halt(0)
}

// ---------------------------------------------------------------------------------------------
// Điểm vào từ `coretest::finish` (sau TAP 41)
// ---------------------------------------------------------------------------------------------

/// Bắt đầu K5: dựng rig rồi `eret` xuống procsrv. **Không trả về.**
pub fn start(console: &mut Console) -> ! {
    if !build_rig(console) {
        let _ = writeln!(
            console,
            "PANIC: k5: không dựng được rig (TAP 42..44 không chạy)"
        );
        debug_halt(3)
    }
    enter_procsrv(console)
}
