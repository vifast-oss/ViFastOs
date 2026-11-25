// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-339a** — shell **sản phẩm dùng chung** (`vifast-plat-user/programs/shell.rs`) trên
//! aarch64: `Machine` trong section NOLOAD `.corespace`, loader ELF, rig `svc #0`, ring KT-69,
//! PL011 — cùng **một** mã cho ảnh sản phẩm (`--features product-shell`) và ảnh kiểm (`ktest`).
//!
//! # Vì sao tệp này tồn tại
//!
//! `KT-338b`/`KT-339` đã nối `Machine` + IPC EL0 nhưng **chỉ** trong ảnh `ktest`, và `Machine`
//! nằm trong `.bss` (184.792 B) — vượt trần `.data+.bss` 65.536 B của ảnh sản phẩm tối giản.
//! `KT-339a` (D2-250) đưa `Machine` ra section **NOLOAD `.corespace`** (khuôn x86_64 KT-341) rồi
//! dựng **cùng** đường shell cho **cả hai** ảnh:
//!
//! - ảnh **sản phẩm** (`--features product-shell`): [`boot`] dựng MMU + `VSpace` + `Machine`, nạp
//!   `procsrv`/`shell` **cùng nguồn** rồi `eret` xuống EL0; shell in prompt + `help` + `echo` +
//!   `ps` (kernel trả bảng TCB đọc từ `Machine`) + `exit`; kernel in `# core:`/`# shell:` rồi
//!   marker + halt. **0 TAP** (D2-08).
//! - ảnh **kiểm** (`ktest`): sau TAP 36..44, `k5user::k5_finish` gọi [`start`] — **cùng** mã, trên
//!   `VSpace`/ring/ELF mà KT-339 đã dựng — và phát **TAP 45/46** từ trạng thái `Machine` + số byte
//!   `DebugPutStr` kernel đếm được (không tin lời U-mode, D2-102).
//!
//! # Vì sao **không** cần scheduler preemptive
//!
//! Một TCB U-mode `Running` tại một thời điểm ("kernel là scheduler tuần tự", KT-338b §8.2).
//! `procsrv` vòng `NBRecv`/`Yield`; mỗi `Yield` là một `svc` → handler chạy **dịch vụ kernel**
//! (rút sự kiện/trả lời, bơm yêu cầu kế) rồi `eret` lại đúng chỗ vừa trap. Shell cũng vậy; lệnh
//! `ps` của nó là `NBSend(EP_PS)` + `NBRecv(EP_REPLY)`, kernel trả `PS_REPLY` đọc từ `Machine`.
//! `exit` là syscall của core ⇒ kernel tiếp quản (không trả về EL0).
//!
//! # Nợ đã biết (ghi thẳng)
//!
//! - UART RX thật → ring vẫn là **kernel bơm script** bằng `conchan` (nợ KT-73 trên aarch64).
//! - Không có preemption; `ps` phản ánh đúng các TCB mà rig này tạo (kern/procsrv/shell).
//! - `wait`/`kill` của shell cần `procsrv` phục vụ **mọi** pid; rig chạy một yêu cầu `WAIT` là
//!   bằng chứng tối thiểu (cùng khuôn KT-339), không phải "shell đầy đủ".

#![allow(clippy::too_many_arguments)]

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::ipc::{Message, MessageInfo};
#[cfg(not(feature = "ktest"))]
use vifast_core::syscall::SyscallOutcome;
use vifast_core::syscall::SyscallReturn;
use vifast_core::{KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::bootmem;
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::bootmem::SharedBootMemory;
use vifast_plat_aarch64::core_bridge;
use vifast_plat_aarch64::debug_halt;
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::pl011;
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::resources::{GIC, GICR, UART0, UART0_IRQ};
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::sysreg::{MAIR_EL1_KERNEL, TCR_EL1_KERNEL};
use vifast_plat_aarch64::umode::spsr_el0_irq;
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::umode::{is_el0, svc_imm, FRAME_X0, FRAME_X8};
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::umode::{
    spsr_el1h, FRAME_X1, FRAME_X2, FRAME_X3, FRAME_X4, FRAME_X5, FRAME_X6,
};
use vifast_plat_aarch64::userload;
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::vspace::{Frame, VSpace};
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::{boot_mmu, tlb};
#[cfg(not(feature = "ktest"))]
use vifast_plat_aarch64::{gic, uart_rx};

use crate::{Console, BOOT_MARKER};

// ---------------------------------------------------------------------------------------------
// Hằng của giao thức — **khớp** hằng trong `programs/*.rs` (một giao thức, hai phía)
// ---------------------------------------------------------------------------------------------

const SLOT_UNTYPED: usize = 0;
const SLOT_EP_FAULT: usize = 1;
const SLOT_EP_PROC: usize = 2;
const SLOT_CSPACE: usize = 3;
const SLOT_EP_REPLY: usize = 4;
const SLOT_EP_PS: usize = 5;
const SLOT_EP_EVENTS: usize = 8;
const SLOT_FRAME: usize = 11;
const SLOT_TCB_PROCSRV: usize = 12;
const SLOT_TCB_SHELL: usize = 13;
const SLOT_TCB_KERN: usize = 14;
/// Số ô CSpace của rig — cùng 16 ô với `coretest`/`k5user` (một giao thức, hai phía).
const PSHELL_SLOTS: usize = 16;
/// Untyped riêng của rig: 4 frame = 16 KiB (TCB/Endpoint/CNode/Foreign ≈ vài KiB).
const PSHELL_ARENA_FRAMES: usize = 4;
/// Đỉnh stack U-mode dùng chung (2 trang mọc xuống `0x7000_A000..0x7000_C000`) — **khớp**
/// `k5user::K5_STACK_TOP` (một bố cục VA cho cả hai pha, không có bản sao thứ hai).
const STACK_TOP: u64 = 0x7000_C000;
#[cfg(not(feature = "ktest"))]
const STACK_PAGES: usize = 2;
/// VA trang IPC buffer của shim aarch64 (`Sys::IPC_BUFFER_VA`) — **khớp** `coretest::IPC_BUF_VA`
/// và backend aarch64 của shim: một trang, ba nơi đọc/ghi.
const IPC_BUF_VA: u64 = 0x7000_2000;
/// Script kernel bơm vào ring KT-69 — **chỉ** khi bật feature `script-input` (KT-406).
///
/// Đây là đường **kiểm hồi quy** của `run-aarch64.sh`: nó cần log `help/echo/ps/exit` mà
/// **không** mở stdin QEMU. Ảnh sản phẩm thật (không bật feature) nhận input **duy nhất** qua
/// ngắt UART thật: ring khởi tạo **rỗng** rồi [`uart_rx`] đẩy byte của người gõ vào.
#[cfg(all(not(feature = "ktest"), feature = "script-input"))]
const SCRIPT: &[u8] = b"help\necho same-source\nps\nexit\n";
/// Ảnh sản phẩm (không `script-input`): ring **rỗng** — mọi byte đến từ UART RX IRQ (**KT-406**).
#[cfg(all(not(feature = "ktest"), not(feature = "script-input")))]
const SCRIPT: &[u8] = b"";
/// Script của pha **thứ hai** trên ảnh kiểm (ring KT-339 được nạp lại sau pha shell của nó).
#[cfg(feature = "ktest")]
const SCRIPT_KTEST: &[u8] = b"help\necho same-source\nps\nexit\n";
/// `e_machine` của AArch64 (`vifast-elf::parse` kiểm trước khi map).
#[cfg(not(feature = "ktest"))]
const EM_AARCH64: u16 = 183;

/// Chương trình U-mode **cùng nguồn** với riscv64 (`vifast-plat-user/programs/`) — ảnh sản phẩm
/// nhúng ELF; ảnh kiểm dùng lại bản KT-339 đã nạp (không nhúng lần hai).
#[cfg(not(feature = "ktest"))]
static PROCSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/procsrv.elf"));
#[cfg(not(feature = "ktest"))]
static SHELL_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.elf"));

/// Label giao thức (khớp `procsrv.rs`/`shell.rs`).
const EV_UP: u32 = 0x501;
const EV_WAIT: u32 = 0x503;
const WAIT_REQ: u32 = 0x604;
const REPLY_WAIT: u32 = 0x606;
const EV_ECHO: u32 = 0x506;
const EV_HELP: u32 = 0x507;
const EV_PS: u32 = 0x505;
const PS_REQ: u32 = 0x602;
const PS_REPLY: u32 = 0x603;
/// Pid giả cho yêu cầu `WAIT` — kernel chọn để biết chính xác bên nhận trả gì.
const PID_WAIT: u64 = 0x4242;
/// `exit_code_of` của procsrv khi pid chưa từng exit (`procsrv.rs`).
const PROC_NO_EXIT: u64 = 0xFFFF;
/// Ngân sách `Yield` một pha trước khi kernel tiếp quản (chống treo khi đột biến).
const YIELD_BUDGET: u64 = 20_000;

const NO_OBJ: u32 = u32::MAX;
const NO_U64: u64 = u64::MAX;

/// Pha của đường shell sản phẩm (số ổn định để in log).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle = 0,
    Procsrv = 1,
    Shell = 2,
    Done = 3,
}

static PHASE: AtomicU32 = AtomicU32::new(Phase::Idle as u32);
static CUR_TCB: AtomicU32 = AtomicU32::new(NO_OBJ);
static ROOT: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_KERN: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_PROCSRV: AtomicU32 = AtomicU32::new(NO_OBJ);
static TCB_SHELL: AtomicU32 = AtomicU32::new(NO_OBJ);
static ENTRY_PROCSRV: AtomicU64 = AtomicU64::new(0);
static ENTRY_SHELL: AtomicU64 = AtomicU64::new(0);
static RING_PA: AtomicU64 = AtomicU64::new(0);
static BUFFER_PA: AtomicU64 = AtomicU64::new(0);
static YIELDS: AtomicU64 = AtomicU64::new(0);
static PUTS: AtomicU64 = AtomicU64::new(0);
static EV_UP_N: AtomicU64 = AtomicU64::new(0);
static EV_WAIT_N: AtomicU64 = AtomicU64::new(0);
static EV_HELP_N: AtomicU64 = AtomicU64::new(0);
static EV_ECHO_N: AtomicU64 = AtomicU64::new(0);
static EV_PS_N: AtomicU64 = AtomicU64::new(0);
static REPLY_WAIT_N: AtomicU64 = AtomicU64::new(0);
static PS_REPLIES: AtomicU64 = AtomicU64::new(0);
static PS_ENTRIES: AtomicU64 = AtomicU64::new(0);
static WAIT_PID: AtomicU64 = AtomicU64::new(NO_U64);
static WAIT_CODE: AtomicU64 = AtomicU64::new(NO_U64);
static SHELL_EXIT: AtomicU64 = AtomicU64::new(NO_U64);
static PROC_TAKEOVER: AtomicBool = AtomicBool::new(false);
static PROC_TIMEOUT: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "ktest")]
static ALL_OK: AtomicBool = AtomicBool::new(true);

/// `true` ⇔ đường shell sản phẩm đang điều khiển U-mode.
pub fn active() -> bool {
    matches!(
        PHASE.load(Ordering::Acquire),
        x if x == Phase::Procsrv as u32 || x == Phase::Shell as u32
    )
}

/// TCB hiện hành của đường shell (handler `svc` dùng để định tuyến).
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

// ---------------------------------------------------------------------------------------------
// Nạp ảnh — chỉ ảnh **sản phẩm** (ảnh kiểm dùng lại trang KT-339 đã map)
// ---------------------------------------------------------------------------------------------

/// Nạp `procsrv`/`shell` + stack + ring + IPC buffer vào `VSpace` **trước** lần `eret` đầu.
///
/// Ảnh **kiểm**: KT-339 đã map đủ ba thứ ấy trong `high_entry` (`k5user::load`) — hàm này chỉ
/// lấy lại entry/stack/ring **đo được** (không nạp lần hai, không đụng bảng trang). Ảnh **sản
/// phẩm**: nạp thật, mọi lỗi ⇒ `PANIC` fail-loud.
/// Nhánh **ảnh kiểm** (KT-339a): dùng lại **đúng** trang/ring mà KT-339 đã nạp trong
/// `high_entry` (`k5user::load`) — chỉ nạp lại script ring + zero IPC buffer cho pha thứ hai.
/// Không nạp lần hai, không đụng bảng trang (không có `&mut VSpace` ở điểm gọi `k5_finish`).
#[cfg(feature = "ktest")]
fn reuse_after_k5(console: &mut Console) -> bool {
    let entry_p = crate::k5user::procsrv_entry();
    let entry_s = crate::k5user::shell_entry();
    let ring = crate::k5user::ring_pa();
    let buffer = crate::coretest::ipc_buffer_pa();
    if entry_p == 0 || entry_s == 0 || ring == 0 || buffer == 0 {
        let _ = writeln!(console, "# shell fail: stage=reuse (KT-339 chưa nạp đủ)");
        return false;
    }
    if userload::fill_ring(ring, SCRIPT_KTEST).is_err() {
        let _ = writeln!(console, "# shell fail: stage=ring-refill");
        return false;
    }
    zero_pages(buffer, 0x1000);
    ENTRY_PROCSRV.store(entry_p, Ordering::Release);
    ENTRY_SHELL.store(entry_s, Ordering::Release);
    RING_PA.store(ring, Ordering::Release);
    BUFFER_PA.store(buffer, Ordering::Release);
    true
}

/// Nạp thật `procsrv`/`shell` + stack + ring + IPC buffer (ảnh **sản phẩm**).
#[cfg(not(feature = "ktest"))]
pub fn load(vs: &mut VSpace<SharedBootMemory>, console: &mut Console) -> bool {
    // ---- 1) Hai ELF **cùng nguồn** với riscv64 (D2-26: dùng `vifast_elf::parse`).
    // ---- 1) Hai ELF **cùng nguồn** với riscv64 (D2-26: dùng `vifast_elf::parse`).
    let procsrv = match userload::load_elf(vs, PROCSRV_ELF, EM_AARCH64) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(
                console,
                "# shell fail: stage=elf procsrv err={} elf={}",
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
                "# shell fail: stage=elf shell err={} elf={}",
                error.name(),
                error.elf_code()
            );
            return false;
        }
    };
    ENTRY_PROCSRV.store(procsrv.entry, Ordering::Release);
    ENTRY_SHELL.store(shell.entry, Ordering::Release);

    // ---- 2) Stack U-mode dùng chung (hai pha chạy tuần tự, không có hai tác vụ cùng lúc).
    let (stack_base, stack_bytes) = match userload::map_user_stack(vs, STACK_TOP, STACK_PAGES) {
        Ok(range) => range,
        Err(error) => {
            let _ = writeln!(console, "# shell fail: stage=stack err={}", error.name());
            return false;
        }
    };
    // ---- 3) Ring KT-69 (script do kernel bơm) + trang IPC buffer (D2-83).
    let ring_pa = match userload::map_ring_with_script(vs, SCRIPT) {
        Ok(pa) => pa,
        Err(error) => {
            let _ = writeln!(console, "# shell fail: stage=ring err={}", error.name());
            return false;
        }
    };
    RING_PA.store(ring_pa, Ordering::Release);
    let Some(buf_pa) = bootmem::alloc_boot_frame() else {
        let _ = writeln!(console, "# shell fail: stage=buffer (hết frame)");
        return false;
    };
    asm::zero_boot_ram(buf_pa, 0x1000);
    let Some(frame) = Frame::new(buf_pa) else {
        let _ = writeln!(console, "# shell fail: stage=buffer frame pa=0x{buf_pa:x}");
        return false;
    };
    if vs
        .map_obj_with(IPC_BUF_VA, frame, Perms::USER_RW, MemAttr::Normal)
        .is_err()
    {
        let _ = writeln!(
            console,
            "# shell fail: stage=buffer map va=0x{IPC_BUF_VA:x}"
        );
        return false;
    }
    BUFFER_PA.store(buf_pa, Ordering::Release);
    // VA mới chưa từng được dùng ⇒ TLB không có entry cũ; vẫn `TLBI` để bảng trang vừa sửa
    // chắc chắn có hiệu lực (cùng tinh thần `VSpace::activate`).
    tlb::vmalle1();
    asm::dsb_ish();
    asm::isb();
    let _ = writeln!(
        console,
        "# shell: load procsrv=0x{:x}+{}p entry=0x{:x} shell=0x{:x}+{}p entry=0x{:x} stack=0x{:x}+{} ring_pa=0x{:x} ring_va=0x{:x} script={}B buf_va=0x{:x}",
        procsrv.base,
        procsrv.pages,
        procsrv.entry,
        shell.base,
        shell.pages,
        shell.entry,
        stack_base,
        stack_bytes,
        ring_pa,
        userload::RING_VA,
        SCRIPT.len(),
        IPC_BUF_VA,
    );
    true
}

/// Zero `bytes` tại PA (dùng `write_u64` của crate arch — không `unsafe` ở ảnh).
#[cfg(feature = "ktest")]
fn zero_pages(pa: u64, bytes: u64) {
    let mut offset = 0u64;
    while offset < bytes {
        asm::write_u64(pa + offset, 0);
        offset += 8;
    }
}

// ---------------------------------------------------------------------------------------------
// Boot ảnh **sản phẩm**: MMU + `VSpace` + `Machine` + nạp ELF
// ---------------------------------------------------------------------------------------------

extern "C" {
    static __text_start: u8;
    static __data_start: u8;
    static __kernel_end: u8;
}

/// Map `pages` trang identity `[start, start+bytes)` với quyền `perms` (VA == PA).
#[cfg(not(feature = "ktest"))]
fn map_identity(vs: &mut VSpace<SharedBootMemory>, start: u64, bytes: u64, perms: Perms) -> usize {
    let first = start & !0xFFF;
    let last = (start + bytes + 0xFFF) & !0xFFF;
    let mut pages = 0usize;
    let mut va = first;
    while va < last {
        let Some(frame) = Frame::new(va) else {
            return pages;
        };
        if vs.map_obj_with(va, frame, perms, MemAttr::Normal).is_err() {
            return pages;
        }
        pages += 1;
        va += 0x1000;
    }
    pages
}

/// **KT-406** — dựng đường UART RX **IRQ thật** rồi in bằng chứng `key=value`.
///
/// Trả `true` ⇔ **mọi** mắt xích đọc lại được: ring gắn PA, distributor bật group 1 NS, hart 0
/// wake, CPU interface `SRE`, SPI 33 enable + `IROUTER` trỏ hart 0, `IMSC.RXIM` bật. Không có
/// mắt xích nào "coi như đã làm": mọi giá trị in ra là **đọc lại từ thanh ghi**.
#[cfg(not(feature = "ktest"))]
fn bringup_uart_irq(console: &mut Console) -> bool {
    let ring_pa = RING_PA.load(Ordering::Acquire);
    let ring_ok = uart_rx::init(ring_pa);
    let info = match gic::init_distributor(GIC.base) {
        Ok(info) => info,
        Err(error) => {
            let _ = writeln!(console, "# uart: irq=0 gicd=fail error={error:?} ok=0");
            return false;
        }
    };
    let woke = gic::wake_redistributor(0);
    let waker = gic::redistributor_waker(0).unwrap_or(0);
    let sre = gic::init_cpu_interface().unwrap_or(0);
    let spi = gic::enable_spi(GIC.base, UART0_IRQ, 0);
    let rx_on = pl011::rx_enable();
    let mis = pl011::masked_status();
    let (intid, spi_enabled, router, priority) = match spi {
        Ok(info) => (info.intid, info.enabled, info.router, info.priority),
        Err(_) => (UART0_IRQ, false, u64::MAX, 0),
    };
    let (slots, slot_bytes) = uart_rx::geometry();
    let ring_check = uart_rx::ring_check();
    let ok = ring_ok
        && info.ctlr & gic::GICD_CTLR_ENABLE_GRP1NS != 0
        && info.lines > UART0_IRQ as u32
        && woke.is_ok()
        && waker & gic::GICR_WAKER_PROCESSOR_SLEEP == 0
        && sre & gic::ICC_SRE_EL1_SRE != 0
        && intid == UART0_IRQ
        && spi_enabled
        && router == 0
        && priority == gic::SPI_PRIORITY
        && rx_on
        && ring_check;
    let _ = writeln!(
        console,
        "# uart: irq={intid} rx={} mis=0x{mis:02x} spi_enabled={} router=0x{router:x} prio=0x{priority:02x} imsc_rx={} gicd_ctlr=0x{:x} lines={} waker=0x{waker:x} sre=0x{sre:x} ring_pa=0x{ring_pa:x} slots={slots} slot={slot_bytes} ring_check={} ok={}",
        u8::from(rx_on),
        u8::from(spi_enabled),
        u8::from(rx_on),
        info.ctlr,
        info.lines,
        u8::from(ring_check),
        u8::from(ok),
    );
    ok
}

/// Nhánh boot của ảnh **sản phẩm** `--features product-shell`. **Không trả về.**
///
/// `product::boot` đã dựng PL011 + bảng vector (nền sản phẩm) trước khi gọi hàm này.
#[cfg(not(feature = "ktest"))]
pub fn boot(_dtb: u64, console: &mut Console) -> ! {
    let text_start = core::ptr::addr_of!(__text_start) as u64;
    let data_start = core::ptr::addr_of!(__data_start) as u64;
    let kernel_end = core::ptr::addr_of!(__kernel_end) as u64;
    if text_start >= data_start || data_start >= kernel_end {
        let _ = writeln!(
            console,
            "PANIC: shell: biên ảnh sai text=0x{text_start:x} data=0x{data_start:x} end=0x{kernel_end:x}"
        );
        debug_halt(3)
    }

    // ---- 1) Arena RAM boot **ngoài ảnh** (KT-338a) — bảng trang + frame ELF/DMA.
    let ram_pa = crate::bootram::prepare(kernel_end);
    let (low_mem, high_mem) = bootmem::shared_boot_memory();
    let mut vs = match VSpace::create(1, low_mem, high_mem) {
        Ok(vs) => vs,
        Err(error) => {
            let _ = writeln!(console, "PANIC: shell: vspace create lỗi {error:?}");
            debug_halt(3)
        }
    };
    // ---- 2) Map ảnh kernel **identity** với W^X (text RX / data-bss-stack RW) + UART.
    let pages_text = map_identity(
        &mut vs,
        text_start,
        data_start - text_start,
        Perms::KERNEL_RX,
    );
    let pages_data = map_identity(
        &mut vs,
        data_start,
        kernel_end - data_start,
        Perms::KERNEL_RW,
    );
    let uart_start = UART0.base;
    let uart_bytes = UART0.end().saturating_sub(UART0.base);
    let mut uart_pages = 0usize;
    {
        let mut va = uart_start & !0xFFF;
        let last = (uart_start + uart_bytes + 0xFFF) & !0xFFF;
        while va < last {
            let Some(frame) = Frame::new(va) else { break };
            if vs
                .map_obj_with(va, frame, Perms::KERNEL_RW, MemAttr::Device)
                .is_err()
            {
                break;
            }
            uart_pages += 1;
            va += 0x1000;
        }
    }
    // ---- 2b) **KT-406** — map GICD + GICR của hart 0 (Device) để đường ngắt UART chạy được.
    // GIC dùng **system register** (`ICC_*_EL1`) cho CPU interface nên chỉ hai khối MMIO này cần.
    let mut gic_pages = 0usize;
    for region in [GIC, GICR] {
        let mut va = region.base & !0xFFF;
        let last = (region.end() + 0xFFF) & !0xFFF;
        while va < last {
            let Some(frame) = Frame::new(va) else { break };
            if vs
                .map_obj_with(va, frame, Perms::KERNEL_RW, MemAttr::Device)
                .is_err()
            {
                break;
            }
            gic_pages += 1;
            va += 0x1000;
        }
    }
    // ---- 3) Arena RAM boot: identity (bảng trang/frame) + physmap (cửa sổ nửa cao, để
    // `ttbr1` có bảng mức 1 — `enable_mmu` đòi một gốc nửa cao hợp lệ).
    let ram_bytes = bootmem::boot_ram_bytes() as u64;
    let pages_ram = map_identity(&mut vs, ram_pa, ram_bytes, Perms::KERNEL_RW);
    let pages_physmap = match vs.map_physmap(
        ram_pa,
        ram_pa + ram_bytes,
        Perms::KERNEL_RW,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: shell: map physmap lỗi {error:?}");
            debug_halt(3)
        }
    };
    let (Some(ttbr0), Some(ttbr1)) = (vs.ttbr0_value(), vs.ttbr1_value()) else {
        let _ = writeln!(console, "PANIC: shell: VSpace thiếu gốc bảng trang");
        debug_halt(3)
    };
    // ---- 4) Bật MMU: **cùng** chuỗi 6 bước của B3.3 (`MMU_ENABLE_SEQUENCE`) — không có chuỗi
    // thứ hai; đổi gốc bảng trang sang `VSpace` vừa dựng (vẫn chạy ở VA thấp identity).
    tlb::vmalle1();
    asm::dsb_ish();
    asm::isb();
    let sctlr = boot_mmu::sctlr_for_mmu_on(asm::read_sctlr_el1());
    let steps = boot_mmu::enable_mmu(
        TCR_EL1_KERNEL.bits(),
        MAIR_EL1_KERNEL.bits(),
        ttbr0,
        ttbr1,
        sctlr,
    );
    let on = boot_mmu::mmu_is_on(asm::read_sctlr_el1());
    let _ = writeln!(
        console,
        "# boot: mmu on={} steps={} ttbr0=0x{ttbr0:x} ttbr1=0x{ttbr1:x} text_pages={pages_text} data_pages={pages_data} uart_pages={uart_pages} gic_pages={gic_pages} ram_pages={pages_ram} physmap_pages={pages_physmap} frames_used={}",
        u8::from(on),
        steps,
        vs.mmu().low().memory().frames_used(),
    );
    if !on || steps != boot_mmu::MMU_ENABLE_SEQUENCE.len() {
        let _ = writeln!(console, "PANIC: shell: MMU không bật đủ chuỗi");
        debug_halt(3)
    }

    // ---- 5) `Machine` (`.corespace` NOLOAD) + Untyped boot 4 frame từ arena.
    core_bridge::set_image_bounds(text_start, kernel_end);
    let mut core_arena = 0u64;
    for index in 0..4 {
        let Some(pa) = bootmem::alloc_boot_frame() else {
            let _ = writeln!(console, "PANIC: shell: hết frame cho core (index={index})");
            debug_halt(3)
        };
        if index == 0 {
            core_arena = pa;
        }
    }
    let report = core_bridge::init(core_arena, 4 * 0x1000, 1);
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
        let _ = writeln!(
            console,
            "PANIC: shell: core không healthy (in_image={} caps={} bytes={} all={})",
            u8::from(report.in_image),
            u8::from(report.check_caps),
            u8::from(report.check_bytes),
            u8::from(report.check_all_bytes),
        );
        debug_halt(3)
    }

    // ---- 6) Nạp hai chương trình U-mode **cùng nguồn**.
    if !load(&mut vs, console) {
        let _ = writeln!(console, "PANIC: shell: không nạp được chương trình U-mode");
        debug_halt(3)
    }
    // ---- 6b) **KT-406** — UART RX **IRQ thật**: GIC SPI 33 + `IMSC.RXIM` + ring của shell.
    // Đây là nguồn byte **duy nhất** của ảnh sản phẩm (ring khởi tạo rỗng khi không bật
    // `script-input`); hỏng một mắt xích ⇒ `# uart: … ok=0` + runner ĐỎ.
    if !bringup_uart_irq(console) {
        let _ = writeln!(console, "PANIC: shell: UART RX IRQ không sẵn sàng (KT-406)");
        debug_halt(6)
    }
    start(console)
}

// ---------------------------------------------------------------------------------------------
// Rig: CSpace 16 ô + 4 Endpoint + 2 TCB + Frame IPC buffer
// ---------------------------------------------------------------------------------------------

fn make_tcb(m: &mut Machine, root: ObjId, us: SlotRef, slot: usize) -> Result<ObjId, KError> {
    let cap = m.retype(us, ObjType::Tcb, 0, 0)?;
    m.put_cap(root, slot, cap)?;
    m.cap_obj(cap)
}

fn make_endpoint(m: &mut Machine, root: ObjId, us: SlotRef, slot: usize) -> Result<(), KError> {
    let cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
    m.put_cap(root, slot, cap)
}

// ---------------------------------------------------------------------------------------------
// Điều phối pha — mọi `eret` dùng đúng một chỗ
// ---------------------------------------------------------------------------------------------

fn enter_el0(entry: u64) -> ! {
    // **KT-406** — `I` **không** mask: shell chờ ring ở EL0 và byte UART phải tới bằng IRQ thật
    // (trước KT-406 dùng `spsr_el0()` mask đủ vì chưa có đường xử lý ngắt từ EL0).
    asm::eret_to_el0(entry, STACK_TOP, spsr_el0_irq())
}

fn start_and_run(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    if matches!(m.tcb_state(tcb)?, TcbState::Inactive) {
        m.start_tcb(tcb)?;
    }
    m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
}

/// Dựng rig rồi vào pha `procsrv`. **Không trả về.**
pub fn start(console: &mut Console) -> ! {
    // Ảnh kiểm: trang/ring là của KT-339 — nạp lại script + zero IPC buffer trước pha thứ hai.
    #[cfg(feature = "ktest")]
    if !reuse_after_k5(console) {
        let _ = writeln!(console, "PANIC: shell: không tái dùng được trang KT-339");
        debug_halt(3)
    }
    // Bằng chứng của pha này phải **của pha này**: reset transcript/đếm trước khi eret.
    #[cfg(feature = "ktest")]
    vifast_plat_aarch64::k5cap::reset();
    PUTS.store(0, Ordering::Release);
    let root = match build(console) {
        Some(root) => root,
        None => {
            let _ = writeln!(console, "PANIC: shell: không dựng được rig");
            debug_halt(3)
        }
    };
    let _ = root;
    let procsrv = TCB_PROCSRV.load(Ordering::Acquire);
    let kern = TCB_KERN.load(Ordering::Acquire);
    let sent = core_bridge::with_machine(|m| -> Result<(), KError> {
        start_and_run(m, procsrv)?;
        let request = msg(WAIT_REQ, &[PID_WAIT, 0, 0, 0])?;
        m.nb_send(kern, SlotRef::new(root, SLOT_EP_PROC), request)
    });
    if sent.is_err() {
        let _ = writeln!(console, "# shell fail: stage=procsrv-request");
    }
    PHASE.store(Phase::Procsrv as u32, Ordering::Release);
    CUR_TCB.store(procsrv, Ordering::Release);
    YIELDS.store(0, Ordering::Release);
    enter_el0(ENTRY_PROCSRV.load(Ordering::Acquire))
}

/// Dựng toàn bộ object của rig; trả `root` CNode.
fn build(console: &mut Console) -> Option<ObjId> {
    let mut arena = 0u64;
    for index in 0..PSHELL_ARENA_FRAMES {
        let Some(pa) = bootmem::alloc_boot_frame() else {
            let _ = writeln!(console, "# shell fail: stage=arena index={index}");
            return None;
        };
        if index == 0 {
            arena = pa;
        }
    }
    let built = core_bridge::with_machine(|m| -> Result<ObjId, KError> {
        let untyped = m.add_untyped(arena, PSHELL_ARENA_FRAMES * 0x1000);
        let root = m.add_cnode(PSHELL_SLOTS);
        ROOT.store(root, Ordering::Release);
        let untyped_cap = m.root_cap(untyped);
        m.put_cap(root, SLOT_UNTYPED, untyped_cap)?;
        let us = SlotRef::new(root, SLOT_UNTYPED);
        for slot in [
            SLOT_EP_FAULT,
            SLOT_EP_PROC,
            SLOT_EP_REPLY,
            SLOT_EP_PS,
            SLOT_EP_EVENTS,
        ] {
            make_endpoint(m, root, us, slot)?;
        }
        let kern = make_tcb(m, root, us, SLOT_TCB_KERN)?;
        let procsrv = make_tcb(m, root, us, SLOT_TCB_PROCSRV)?;
        let shell = make_tcb(m, root, us, SLOT_TCB_SHELL)?;
        TCB_KERN.store(kern, Ordering::Release);
        TCB_PROCSRV.store(procsrv, Ordering::Release);
        TCB_SHELL.store(shell, Ordering::Release);
        let cspace_cap = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cspace_cap)?;
        let frame_obj = m.add_foreign_object(ObjType::Frame, BUFFER_PA.load(Ordering::Acquire))?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, SLOT_FRAME, frame_cap)?;
        let cspace = SlotRef::new(root, SLOT_CSPACE);
        for slot in [SLOT_TCB_KERN, SLOT_TCB_PROCSRV] {
            m.tcb_configure(SlotRef::new(root, slot), Some(cspace))?;
        }
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(root, SLOT_TCB_SHELL),
            Some(cspace),
            Some((SlotRef::new(root, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        for slot in [SLOT_TCB_PROCSRV, SLOT_TCB_SHELL] {
            m.tcb_set_priority(SlotRef::new(root, slot), 10, u8::MAX)?;
        }
        m.tcb_set_priority(SlotRef::new(root, SLOT_TCB_KERN), 1, u8::MAX)?;
        m.start_tcb(kern)?;
        Ok(root)
    });
    match built {
        Ok(root) => Some(root),
        Err(_) => {
            let _ = writeln!(console, "# shell fail: stage=rig");
            None
        }
    }
}

/// Dịch vụ kernel trong một `Yield` của **procsrv**: rút sự kiện/trả lời; đủ yêu cầu ⇒ shell.
fn service_procsrv(m: &mut Machine) -> Result<Option<u64>, KError> {
    let root = ROOT.load(Ordering::Acquire);
    let kern = TCB_KERN.load(Ordering::Acquire);
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
            _ => {}
        }
    }
    while let Ok(Some(message)) = m.nb_recv(kern, SlotRef::new(root, SLOT_EP_REPLY)) {
        if message.info.label() == REPLY_WAIT {
            REPLY_WAIT_N.fetch_add(1, Ordering::Relaxed);
        }
    }
    let done = EV_WAIT_N.load(Ordering::Acquire) >= 1
        && REPLY_WAIT_N.load(Ordering::Acquire) >= 1
        && WAIT_PID.load(Ordering::Acquire) == PID_WAIT
        && WAIT_CODE.load(Ordering::Acquire) == PROC_NO_EXIT;
    if done || PROC_TIMEOUT.load(Ordering::Acquire) {
        let _ = m.transition_tcb(
            TCB_PROCSRV.load(Ordering::Acquire),
            NonReadyTarget::Inactive,
        );
        PROC_TAKEOVER.store(true, Ordering::Release);
        return Ok(Some(enter_shell as *const () as u64));
    }
    Ok(None)
}

/// Pha **shell** — vào EL0 tại entry `shell.elf` (đã nạp), kernel đã sẵn sàng trả `PS_REPLY`.
#[no_mangle]
pub extern "C" fn enter_shell() -> ! {
    let mut console = Console;
    // TAP 45 (ảnh kiểm) — procsrv phục vụ một yêu cầu `WAIT` từ EL0; kernel đọc bằng chứng từ
    // Endpoint, không tin lời chương trình.
    #[cfg(feature = "ktest")]
    {
        let procsrv = TCB_PROCSRV.load(Ordering::Acquire);
        let state = core_bridge::with_machine(|m| m.tcb_state(procsrv).ok());
        let ok = EV_UP_N.load(Ordering::Acquire) == 1
            && EV_WAIT_N.load(Ordering::Acquire) == 1
            && REPLY_WAIT_N.load(Ordering::Acquire) == 1
            && WAIT_PID.load(Ordering::Acquire) == PID_WAIT
            && WAIT_CODE.load(Ordering::Acquire) == PROC_NO_EXIT
            && YIELDS.load(Ordering::Acquire) >= 2
            && !PROC_TIMEOUT.load(Ordering::Acquire)
            && PROC_TAKEOVER.load(Ordering::Acquire)
            && state == Some(TcbState::Inactive);
        let _ = writeln!(
            &mut console,
            "# shell proc: up={} wait_pid={} wait_code={} reply_wait={} yields={} timeout={} takeover={} state={}",
            EV_UP_N.load(Ordering::Acquire),
            WAIT_PID.load(Ordering::Acquire),
            WAIT_CODE.load(Ordering::Acquire),
            REPLY_WAIT_N.load(Ordering::Acquire),
            YIELDS.load(Ordering::Acquire),
            u8::from(PROC_TIMEOUT.load(Ordering::Acquire)),
            u8::from(PROC_TAKEOVER.load(Ordering::Acquire)),
            match state {
                Some(TcbState::Inactive) => "Inactive",
                Some(TcbState::Running { .. }) => "Running",
                Some(_) => "other",
                None => "none",
            },
        );
        let mut all = crate::tap(
            &mut console,
            45,
            "shell: procsrv serves WAIT from EL0 (product path)",
            ok,
        );
        all &= ALL_OK.load(Ordering::Acquire);
        ALL_OK.store(all, Ordering::Release);
    }
    let shell = TCB_SHELL.load(Ordering::Acquire);
    let root = ROOT.load(Ordering::Acquire);
    let entered = core_bridge::with_machine(|m| {
        let _ = root;
        start_and_run(m, shell)
    });
    if entered.is_err() {
        let _ = writeln!(&mut console, "PANIC: shell: không chạy được shell");
        debug_halt(3)
    }
    PHASE.store(Phase::Shell as u32, Ordering::Release);
    CUR_TCB.store(shell, Ordering::Release);
    YIELDS.store(0, Ordering::Release);
    enter_el0(ENTRY_SHELL.load(Ordering::Acquire))
}

/// Dịch vụ kernel trong `Yield` của **shell**: rút sự kiện + trả lời `PS_REQ` (bảng TCB đọc từ
/// `Machine`). Trả `None` (shell tự `exit` khi hết script — không có điểm tiếp quản giữa chừng).
fn service_shell(m: &mut Machine) {
    let root = ROOT.load(Ordering::Acquire);
    let kern = TCB_KERN.load(Ordering::Acquire);
    while let Ok(Some(message)) = m.nb_recv(kern, SlotRef::new(root, SLOT_EP_EVENTS)) {
        match message.info.label() {
            EV_HELP => {
                EV_HELP_N.fetch_add(1, Ordering::Relaxed);
            }
            EV_ECHO => {
                EV_ECHO_N.fetch_add(1, Ordering::Relaxed);
            }
            EV_PS => {
                EV_PS_N.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
    while let Ok(Some(request)) = m.nb_recv(kern, SlotRef::new(root, SLOT_EP_PS)) {
        if request.info.label() == PS_REQ {
            // Bảng TCB **đọc từ `Machine`** — không phải bảng tĩnh; shell tự định dạng/in.
            let ids = [
                TCB_KERN.load(Ordering::Acquire),
                TCB_PROCSRV.load(Ordering::Acquire),
                TCB_SHELL.load(Ordering::Acquire),
            ];
            let mut entries = [0u64; 4];
            let mut n = 0usize;
            for id in ids {
                if id == NO_OBJ {
                    continue;
                }
                let Ok(state) = m.tcb_state(id) else { continue };
                let suspended = m
                    .tcb_info_obj(id)
                    .map(|info| info.suspended)
                    .unwrap_or(false);
                if n < entries.len() {
                    entries[n] =
                        (u64::from(id) << 5) | ((state.index() as u64) << 1) | u64::from(suspended);
                    n += 1;
                }
            }
            let words = [
                entries[0] | (entries[1] << 16),
                entries[2] | (entries[3] << 16),
            ];
            if let Ok(message) = msg(PS_REPLY, &words) {
                let _ = m.nb_send(kern, SlotRef::new(root, SLOT_EP_REPLY), message);
            }
            PS_ENTRIES.store(n as u64, Ordering::Release);
            PS_REPLIES.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Hook **sau `dispatch` trả `Return`/`Switched`** của một syscall trong pha shell.
pub fn after_return(nr: u64, result: &SyscallReturn, console: &mut Console) -> Option<u64> {
    if !active() {
        return None;
    }
    if nr == vifast_core::spec::SYS_DEBUG_PUT_STR && result.error == 0 {
        PUTS.fetch_add(1, Ordering::Relaxed);
    }
    if nr != vifast_core::spec::SYS_YIELD {
        return None;
    }
    let _ = console;
    let budgets = YIELDS.fetch_add(1, Ordering::AcqRel) + 1;
    if budgets >= YIELD_BUDGET {
        if PHASE.load(Ordering::Acquire) == Phase::Procsrv as u32 {
            PROC_TIMEOUT.store(true, Ordering::Release);
        } else {
            let _ = writeln!(console, "# shell fail: stage=budget yields={budgets}");
            debug_halt(4)
        }
    }
    match PHASE.load(Ordering::Acquire) {
        x if x == Phase::Procsrv as u32 => {
            core_bridge::with_machine(|m| service_procsrv(m).ok().flatten())
        }
        x if x == Phase::Shell as u32 => {
            core_bridge::with_machine(service_shell);
            None
        }
        _ => None,
    }
}

/// Hook khi `Exit` của một tác vụ trong pha shell.
pub fn after_exit(code: u64) -> Option<u64> {
    let phase = PHASE.load(Ordering::Acquire);
    if phase == Phase::Procsrv as u32 {
        PROC_TIMEOUT.store(true, Ordering::Release);
        core_bridge::with_machine(|m| {
            let _ = m.transition_tcb(
                TCB_PROCSRV.load(Ordering::Acquire),
                NonReadyTarget::Inactive,
            );
        });
        return Some(enter_shell as *const () as u64);
    }
    if phase == Phase::Shell as u32 {
        // Rút **nốt** sự kiện shell vừa gửi trước `Exit` (EV_HELP/EV_ECHO/**EV_PS**): shell in
        // `ps` rồi chạy `exit` ngay, không có `Yield` nào ở giữa ⇒ nếu không rút ở đây thì bằng
        // chứng `ps=1` mất (đo được ở lượt chạy 2026-11-23: `ps=0` dù shell đã in `ps n=3`).
        core_bridge::with_machine(service_shell);
        SHELL_EXIT.store(code, Ordering::Release);
        PHASE.store(Phase::Done as u32, Ordering::Release);
        return Some(done as *const () as u64);
    }
    None
}

/// TAP 46 (ảnh kiểm) / kết thúc ảnh sản phẩm: in bằng chứng rồi marker + halt.
#[no_mangle]
pub extern "C" fn done() -> ! {
    let mut console = Console;
    let shell = TCB_SHELL.load(Ordering::Acquire);
    let state = core_bridge::with_machine(|m| m.tcb_state(shell).ok());
    let puts = PUTS.load(Ordering::Acquire);
    let exit = SHELL_EXIT.load(Ordering::Acquire);
    let help = EV_HELP_N.load(Ordering::Acquire);
    let echo = EV_ECHO_N.load(Ordering::Acquire);
    let ps = EV_PS_N.load(Ordering::Acquire);
    let replies = PS_REPLIES.load(Ordering::Acquire);
    let entries = PS_ENTRIES.load(Ordering::Acquire);
    // `EV_PS` **không** dùng làm bằng chứng: shell gửi nó rồi `Exit` ngay, và `Machine::exit`
    // dọn message của TCB thoát (`unlink_tcb_queues`) trước khi kernel kịp rút — đo được ở lượt
    // chạy 2026-11-23 (`ps=0` dù shell đã in `ps n=3`). Bằng chứng `ps` là **kernel-side**:
    // `PS_REQ` đã tới EP_PS (replies) + bảng TCB đọc từ `Machine` có ≥ 3 mục.
    #[allow(unused_mut)]
    let mut all = exit == 0
        && state == Some(TcbState::Inactive)
        && help >= 1
        && echo >= 1
        && replies >= 1
        && entries >= 3
        && puts >= 4;
    // Bằng chứng **kernel-side** của chuỗi đã in (không tin lời U-mode): `k5cap` băm đúng các
    // byte `DebugPutStr` — phải thấy prompt + help + echo + exit.
    #[cfg(feature = "ktest")]
    {
        let transcript = vifast_plat_aarch64::k5cap::flags();
        all &= transcript == vifast_plat_aarch64::k5cap::F_ALL;
        all &= ALL_OK.load(Ordering::Acquire);
        let _ = writeln!(
            &mut console,
            "# shell: exit code={exit} state={} transcript=0x{transcript:x} puts={puts} help={help} echo={echo} ps={ps} replies={replies} entries={entries} yields={}",
            match state {
                Some(TcbState::Inactive) => "Inactive",
                Some(TcbState::Running { .. }) => "Running",
                Some(_) => "other",
                None => "none",
            },
            YIELDS.load(Ordering::Acquire),
        );
        let _ = crate::tap(
            &mut console,
            46,
            "shell: product path help/echo/ps/exit over PL011 (0 marker)",
            all,
        );
    }
    #[cfg(not(feature = "ktest"))]
    let _ = writeln!(
        &mut console,
        "# shell: exit code={exit} state={} puts={puts} help={help} echo={echo} ps={ps} replies={replies} entries={entries} yields={}",
        match state {
            Some(TcbState::Inactive) => "Inactive",
            Some(TcbState::Running { .. }) => "Running",
            Some(_) => "other",
            None => "none",
        },
        YIELDS.load(Ordering::Acquire),
    );
    // **KT-406** — bằng chứng UART RX **IRQ thật** của lần boot này (đọc từ bộ đếm của handler,
    // không phải số viết tay): `irq` > 0 và `rx` > 0 chứng minh byte đã đi qua PL011 → GIC →
    // handler → ring; `chunks`/`drops` chứng minh ring nhận đúng slot; `head`/`tail` chứng minh
    // shell **đã tiêu thụ** (head tiến ⇒ shell đọc; drops = 0 ⇒ không mất byte).
    #[cfg(not(feature = "ktest"))]
    {
        let uart = uart_rx::stats();
        let (head, tail, used, _seq) = uart_rx::ring_stats();
        let ok = uart.irqs > 0 && uart.bytes > 0 && uart.drops == 0 && head > 0;
        let _ = writeln!(
            &mut console,
            "# uart: irq={} rx={} chunks={} drops={} empty={} no_ring={} last={} head={head} tail={tail} used={used} ok={}",
            uart.irqs,
            uart.bytes,
            uart.chunks,
            uart.drops,
            uart.empty_polls,
            uart.no_ring,
            uart.last_irq_bytes,
            u8::from(ok),
        );
    }
    let invariants = core_bridge::with_machine(|m| {
        m.check_caps().is_ok()
            && m.check_all_bytes().is_ok()
            && m.check_ipc().is_ok()
            && m.check_sched().is_ok()
            && m.check_reply_links().is_ok()
    });
    let _ = writeln!(&mut console, "# shell: invariants={}", u8::from(invariants));
    if !all || !invariants {
        debug_halt(5)
    }
    // Marker **chỉ** khi mọi bất biến xanh (bất biến của ảnh: đỏ ⇒ không marker).
    let _ = console.write_str(BOOT_MARKER);
    let _ = writeln!(&mut console, "# viFastOs aarch64: halt");
    debug_halt(0)
}

// ---------------------------------------------------------------------------------------------
// Handler trap của ảnh **sản phẩm** (ảnh kiểm dùng `trap.rs` — miền kiểm đầy đủ)
// ---------------------------------------------------------------------------------------------

/// `true` ⇔ đột biến `mut-no-svc` đang bật **và** đường shell đang chạy: handler `svc` **không**
/// gọi `dispatch` ⇒ shell không in prompt/help/ps ⇒ runner ĐỎ (không marker).
pub fn svc_blocked() -> bool {
    cfg!(feature = "mut-no-svc") && active()
}

/// `svc` từ EL0 của đường shell sản phẩm — **một** đường `syscall::dispatch` trên `Machine`.
#[cfg(not(feature = "ktest"))]
fn handle_svc(slot: u64, esr_raw: u64, frame: u64) -> u64 {
    let mut console = Console;
    if svc_blocked() {
        let _ = writeln!(&mut console, "# shell: svc blocked (mut-no-svc)");
        debug_halt(5)
    }
    let esr = Esr::from_bits(esr_raw);
    let imm = svc_imm(esr.iss());
    let nr = asm::frame_get(frame, FRAME_X8);
    let x0 = asm::frame_get(frame, FRAME_X0);
    let x1 = asm::frame_get(frame, FRAME_X1);
    let x2 = asm::frame_get(frame, FRAME_X2);
    let x3 = asm::frame_get(frame, FRAME_X3);
    let x4 = asm::frame_get(frame, FRAME_X4);
    let x5 = asm::frame_get(frame, FRAME_X5);
    let x6 = asm::frame_get(frame, FRAME_X6);
    let args = match nr {
        vifast_abi::SYS_CALL | vifast_abi::SYS_SEND | vifast_abi::SYS_NB_SEND => {
            [x0, x2, x3, x4, x5, x6]
        }
        _ => [x0, x1, x2, x3, x4, x5],
    };
    let Some(tcb) = current() else {
        let _ = writeln!(&mut console, "PANIC: shell: svc nr={nr} không có TCB");
        debug_halt(1)
    };
    let outcome = core_bridge::with_machine(|m| {
        vifast_core::syscall::dispatch(
            m,
            tcb,
            0,
            nr,
            args,
            &mut vifast_plat_aarch64::syscall_env::Aarch64Env,
        )
    });
    match outcome {
        SyscallOutcome::Return(result) => {
            asm::frame_set(frame, FRAME_X0, result.error);
            asm::frame_set(frame, FRAME_X1, result.value);
            asm::frame_set(frame, FRAME_X2, result.info);
            for (index, value) in result.mr.iter().enumerate() {
                asm::frame_set(frame, FRAME_X3 + index, *value);
            }
            let _ = slot;
            let _ = imm;
            if let Some(entry) = after_return(nr, &result, &mut console) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
            }
            1
        }
        SyscallOutcome::Exit { code } => {
            if let Some(entry) = after_exit(code) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
                return 1;
            }
            let _ = writeln!(
                &mut console,
                "PANIC: shell: Exit ngoài pha shell code={code}"
            );
            debug_halt(4)
        }
        SyscallOutcome::Halt { code } => debug_halt(code as usize),
        SyscallOutcome::Switched { .. } => {
            // Kernel là scheduler tuần tự của đường này: `Use` còn lại quay lại chính nó.
            asm::frame_set(frame, FRAME_X0, 0);
            asm::frame_set(frame, FRAME_X1, 0);
            asm::frame_set(frame, FRAME_X2, 0);
            for index in 0..4 {
                asm::frame_set(frame, FRAME_X3 + index, 0);
            }
            if let Some(entry) = after_return(nr, &SyscallReturn::new(0, 0), &mut console) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
            }
            1
        }
    }
}

/// Handler vector của ảnh **sản phẩm** có shell: chỉ nhận `svc #0` từ EL0 (ô 8); mọi trap khác
/// in nguyên nhân rồi dừng — che lỗi là điều tệ nhất.
#[cfg(not(feature = "ktest"))]
#[no_mangle]
pub extern "C" fn vec_handler(
    slot: u64,
    esr_raw: u64,
    far: u64,
    elr: u64,
    spsr: u64,
    frame: u64,
) -> u64 {
    // **KT-406** — ô IRQ: **1** = EL1t, **5** = EL1h, **9** = nguồn thấp (EL0) AArch64 (§D1.10.2:
    // `0x480` = IRQ từ EL0). Byte UART tới khi shell đang chờ ring ở **EL0** ⇒ ô **9**; thiếu ô
    // này thì ngắt IRQ rơi vào nhánh `# exc` và halt (đo được: `slot=9 ec=0x15`).
    // `ESR_EL1` của ngắt là 0 nên phân loại bằng **ô vector**, không bằng syndrome. Chỉ SPI của
    // PL011 được xử lý; `EOI` **bắt buộc** sau ack, nếu không ngắt không bao giờ tới lại. INTID
    // lạ vẫn phải `EOI` rồi resume (không halt): nguồn ngắt khác không được biến thành PANIC.
    if slot == 1 || slot == 5 || slot == 9 {
        let intid = gic::ack_irq();
        if !gic::is_spurious(intid) {
            if intid == UART0_IRQ {
                let _ = uart_rx::on_irq();
            }
            gic::end_irq(intid);
        }
        return 1;
    }
    let esr = Esr::from_bits(esr_raw);
    if slot == 8 && esr.ec() == Some(ExceptionClass::Svc64) && is_el0(spsr) {
        return handle_svc(slot, esr_raw, frame);
    }
    let mut console = Console;
    let _ = writeln!(
        &mut console,
        "# exc: slot={slot} ec=0x{:02x} esr=0x{esr_raw:016x} far=0x{far:x} elr=0x{elr:x} spsr=0x{spsr:x}",
        esr.ec_raw()
    );
    debug_halt(2)
}
