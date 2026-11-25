// SPDX-License-Identifier: MIT OR Apache-2.0
//! **U-mode (EL0) của ảnh aarch64** — KT-269 (K4' bước 3, B3.7; D2-206).
//!
//! # Đường đi (một chiều, không quay lại kernel bằng `ret`)
//!
//! ```text
//!   high_entry (EL1h, VA nửa cao)
//!     ├─ map trang `.el0` (PA trong ảnh) tại EL0_CODE_VA  = Perms::USER_RX
//!     ├─ map trang stack user (PA trong .bss) tại EL0_STACK_VA = Perms::USER_RW
//!     ├─ vs.activate()  (flush TLB cho VA mới)
//!     ├─ SP_EL0 ← EL0_STACK_TOP, ELR_EL1 ← entry, SPSR_EL1 ← EL0t|DAIF, ISB, eret
//!     ↓
//!   EL0: adr x0, msg; mov x1, len; mov x8, #DebugPutStr; svc #0
//!     ↑ handler (ô 8 = Lower EL AArch64 Sync, EC=0x15): đọc ESR.ISS (imm16) + x8 (số hiệu)
//!       → in chuỗi user qua PL011 → x0 = 0 (error), x1 = len (value) → ELR += 4 → eret
//!     ↓
//!   EL0: mov x8, #Exit; mov x0, #0; svc #0
//!     ↑ handler: TAP 18 (EL0 **đã chạy tiếp** sau `svc` đầu) → ELR ← el0_done, SPSR ← EL1h
//!     ↓
//!   el0_done (EL1h): marker + halt
//! ```
//!
//! # Vì sao `Exit` quay về EL1 bằng `eret` chứ không `ret`
//!
//! `svc` không phải lời gọi hàm: kernel **không** có địa chỉ trả về trên stack của EL0. Đường
//! sản phẩm của một syscall `Exit` là **không trả về EL0**; ở đây (B3.7, chưa có scheduler) nó
//! `eret` về EL1h tại [`el0_done`] để in marker rồi tắt máy — đúng ngữ nghĩa "Exit kết thúc
//! tiến trình", chỉ khác là chưa có procsrv để nhận `FaultKind::Exit`.
//!
//! # Bất biến
//!
//! - **Chỉ một** syscall thật (`DebugPutStr`) + `Exit`; **không** IPC, **không** nhiều process,
//!   **không** preemption (D2-206 giới hạn phạm vi).
//! - Trang mã EL0 map `USER_RX` (không ghi), trang stack `USER_RW` (không exec) — W^X ở mức
//!   trang user; ảnh kernel vẫn map identity `user=false` nên EL0 **không** đọc được kernel.
//! - Số hiệu syscall lấy từ **hằng sinh** `vifast_abi::generated` (`SYS_DEBUG_PUT_STR`,
//!   `SYS_EXIT`) — không chép tay số 12/14 (D2-11). Tên thanh ghi (`x8`, `x0`, `x1`) là chỗ duy
//!   nhất viết tay, đúng `spec/abi.toml [calling.aarch64]`; sai thanh ghi ⇒ handler đọc số hiệu
//!   0 ⇒ TAP 17 `not ok` (đo được, không im lặng).

use core::arch::global_asm;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_core::syscall::SyscallOutcome;

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::bootmem::SharedBootMemory;
use vifast_plat_aarch64::debug_halt;
use vifast_plat_aarch64::fault::{EC_DATA_ABORT_LOWER_EL, EC_INSTRUCTION_ABORT_LOWER_EL};
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::split_mmu::KERNEL_HIGH_BASE;
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};
use vifast_plat_aarch64::umode::{
    is_el0, mode_name, spsr_el0, spsr_el1h, svc_imm, EL0_CODE_VA, EL0_STACK_TOP, EL0_STACK_VA,
    FRAME_X0, FRAME_X1, FRAME_X2, FRAME_X3, FRAME_X4, FRAME_X5, FRAME_X6, FRAME_X8,
};
use vifast_plat_aarch64::vspace::{Frame, VSpace};

use crate::{tap, Console};

/// Ô vector **đồng bộ, Lower EL dùng AArch64** (`0x400`) — nơi `svc` từ EL0 đi vào.
const SLOT_SYNC_LOWER_EL_AARCH64: u64 = 8;
/// Ô vector **đồng bộ, Current EL dùng SPx** (`0x200`) — nơi `svc` từ EL1 đi vào (đột biến (i)).
const SLOT_SYNC_CURRENT_EL_SPX: u64 = 4;

/// Chuỗi EL0 in qua `DebugPutStr` — nằm **cùng trang** với mã EL0 (stub dùng `adr`).
const EL0_MSG: &str = "el0 says hi\n";
/// Độ dài chuỗi — `const` operand cho `mov x1, #len` của stub.
const EL0_MSG_LEN: u64 = EL0_MSG.len() as u64;
// **KT-338b** — trần in byte user + bộ đếm in cũ đã **xoá**: từ khi `svc` đi qua
// `vifast_core::syscall::dispatch`, việc đọc bộ nhớ user nằm ở
// `vifast_plat_aarch64::syscall_env::Aarch64Env` (D2-83: core không đọc user). Không còn đường
// in thứ hai — nếu còn, hai đường có thể lệch nhau mà không ai thấy.

/// `const` chốt: chuỗi trong asm (`.ascii`) và `EL0_MSG` phải **cùng độ dài** — lệch là lỗi biên
/// dịch, không phải sai âm thầm lúc chạy.
const _: () = assert!(EL0_MSG_LEN == 12);

/// Trang stack user — `.bss`, căn 4 KiB; map **cùng PA** ở [`EL0_STACK_VA`] với `USER_RW`.
#[repr(align(4096))]
struct El0Stack([AtomicU64; 512]);

/// Stack user của EL0 (một trang 4 KiB — đủ cho stub, **không** phải stack đầy đủ).
static EL0_STACK: El0Stack = El0Stack([const { AtomicU64::new(0) }; 512]);

/// Số lần `svc` đã vào handler — in bằng chứng **một lần** cho mỗi ca.
///
/// Đột biến (iii) "handler không tăng `ELR_EL1`" làm `svc` lặp vô hạn; đếm để **không** in lặp
/// (log không phình) nhưng vẫn treo có kiểm soát — runner đỏ vì thiếu marker, không phải vì log
/// khổng lồ.
static SVC_COUNT: AtomicU64 = AtomicU64::new(0);

/// **KT-338b** — PA liên kết của `__text_start` (đọc lúc PC còn nửa thấp) — cần để đổi symbol
/// nửa cao thành PA cho các điểm vào pha core.
static TEXT_START_PA: AtomicU64 = AtomicU64::new(0);
/// PA của đầu section `.el0` — mốc để tính VA của từng điểm vào.
static CODE_START_PA: AtomicU64 = AtomicU64::new(0);
/// Đang ở **pha core** (chương trình EL0 thứ hai): `Exit` phải về `core_done`, `Switched` phải
/// giao cho kernel (không phải lỗi).
static IN_CORE_PHASE: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------------------------
// Stub EL0 — section riêng `.el0`, linker đặt **một trang** (xem `linker.ld`)
// ---------------------------------------------------------------------------------------------
//
// Vì sao asm chứ không phải mã Rust: EL0 **không** có `core`/`std`, không có stack khởi tạo,
// không có `panic_handler` — chỉ vài instruction là đủ cho phép đo. Stub **không** tham chiếu
// symbol tuyệt đối nào: `adr` là PC-relative nên chạy đúng ở VA user dù ảnh liên kết ở PA.
//
// Số hiệu syscall lấy từ **hằng sinh** (`vifast_abi::generated`) — `global_asm!` nhận `const`
// operand, nên đổi số trong `spec/abi.toml` là stub đổi theo (D2-11), không có số 12/14 viết tay.
global_asm!(
    ".section .el0, \"ax\"",
    ".p2align 12",
    ".globl __el0_entry",
    "__el0_entry:",
    // x0 = con trỏ chuỗi (cùng trang, PC-relative), x1 = độ dài.
    "adr x0, __el0_msg",
    "mov x1, #{msg_len}",
    // x8 = số hiệu syscall — `[calling.aarch64].syscall_number`.
    "mov x8, #{putstr}",
    "svc #0",
    // Trả về theo quy ước: x0 = error, x1 = value. Giữ lại rồi gọi Exit.
    "mov x9, x0",
    "mov x10, x1",
    "mov x8, #{exit}",
    "mov x0, #0",
    "svc #0",
    // `Exit` không trả về EL0; nếu có thì dừng an toàn thay vì chạy vào rác.
    "1:",
    "b 1b",
    "__el0_msg:",
    ".ascii \"el0 says hi\\n\"",
    "__el0_msg_end:",
    putstr = const vifast_abi::SYS_DEBUG_PUT_STR,
    exit = const vifast_abi::SYS_EXIT,
    msg_len = const EL0_MSG_LEN,
);

// ---------------------------------------------------------------------------------------------
// **KT-338b** — chương trình EL0 **thứ hai** (pha core): gọi `Tcb.Configure`/`Send`/
// `Tcb.Suspend` qua `svc #0` — toàn bộ đi qua `vifast_core::syscall::dispatch` (không nhánh tắt).
// ---------------------------------------------------------------------------------------------
//
// Quy ước thanh ghi của ảnh (đối xứng `[calling.aarch64]`, ghi rõ trong JOURNAL):
//   * `x8` = số hiệu syscall;
//   * **gửi** (0/1/2): `x0` = cap, `x2` = `MessageInfo`, `x3..x6` = MR0..MR3;
//   * **trả**  (mọi syscall): `x0` = error, `x1` = value, `x2` = `MessageInfo`, `x3..x6` = MR0..MR3.
// Khác riscv64 **một** điểm: riscv64 đặt `MessageInfo` ở `a1` khi gửi; ABI v1.1 của aarch64 khai
// `message_info = "x2"`, `mr_registers = ["x3".."x6"]` nên ảnh này theo **spec** và handler dịch
// sang mảng `args` của core (`args[1] = x2`, `args[2..6] = x3..x6` cho 0/1/2).
//
// `__el0_after_suspend` là **điểm vào sau `Suspend(Defer)`**: kernel chỉ quay lại đây khi
// `Tcb.Resume` giao đúng VA này (D2-83) — nếu `TcbData.entry` sai, EL0 chạy vào `#99` và TAP đỏ.
global_asm!(
    ".section .el0, \"ax\"",
    ".globl __el0_core_entry",
    "__el0_core_entry:",
    // (a) `Tcb.Configure` đổi ưu tiên: Call(cap TCB core, info{label=Configure,len=3},
    //     MR0 = CPTR Frame, MR1 = VA buffer, MR2 = mask priority). Giá trị ưu tiên ở IPC buffer.
    "mov x8, #{sys_call}",
    "mov x0, #{slot_core}",
    "movz x2, #{info_cfg3_hi}, lsl #16",
    "movk x2, #{info_cfg3_lo}",
    "mov x3, #{slot_frame}",
    "movz x4, #{buf_va_hi}, lsl #16",
    "movk x4, #{buf_va_lo}",
    "mov x5, #{mask_prio}",
    "svc #0",
    // (b) `Send` message 8 MR trên Endpoint (server đã Recv-blocked): MR0..MR3 thanh ghi,
    //     MR4..MR7 trong IPC buffer (offset 80).
    "mov x8, #{sys_send}",
    "mov x0, #{slot_ep}",
    "movz x2, #{info_send_hi}, lsl #16",
    "movk x2, #{info_send_lo}",
    "movz x3, #{mr0_hi}, lsl #16",
    "movk x3, #{mr0_lo}",
    "movz x4, #{mr1_hi}, lsl #16",
    "movk x4, #{mr1_lo}",
    "movz x5, #{mr2_hi}, lsl #16",
    "movk x5, #{mr2_lo}",
    "movz x6, #{mr3_hi}, lsl #16",
    "movk x6, #{mr3_lo}",
    "svc #0",
    // (c1) `Tcb.Configure` trên cap **thiếu** RIGHT_WRITE ⇒ core trả 3 (`InsufficientRights`).
    "mov x8, #{sys_call}",
    "mov x0, #{slot_ro}",
    "movz x2, #{info_cfg2_hi}, lsl #16",
    "movk x2, #{info_cfg2_lo}",
    "mov x3, #{slot_frame}",
    "movz x4, #{buf_va_hi}, lsl #16",
    "movk x4, #{buf_va_lo}",
    "svc #0",
    // (c2) `Tcb.Configure(vspace?)` ⇒ arch `Failed(Busy)` ⇒ core rollback + trả 14.
    "mov x8, #{sys_call}",
    "mov x0, #{slot_core}",
    "movz x2, #{info_cfg3_hi}, lsl #16",
    "movk x2, #{info_cfg3_lo}",
    "mov x3, #{slot_frame}",
    "movz x4, #{buf_va_hi}, lsl #16",
    "movk x4, #{buf_va_lo}",
    "mov x5, #{mask_vspace}",
    "svc #0",
    // (d1) `Tcb.Configure(entry?)` = `__el0_after_suspend` (để `Tcb.Resume` có điểm vào).
    "mov x8, #{sys_call}",
    "mov x0, #{slot_core}",
    "movz x2, #{info_cfg3_hi}, lsl #16",
    "movk x2, #{info_cfg3_lo}",
    "mov x3, #{slot_frame}",
    "movz x4, #{buf_va_hi}, lsl #16",
    "movk x4, #{buf_va_lo}",
    "mov x5, #{mask_entry}",
    "svc #0",
    // (d2) `Tcb.Suspend(self, Defer)` — `dispatch` trả `Switched` ở **điểm an toàn** ⇒ kernel
    //      tiếp quản, `Resume` rồi quay lại đúng `__el0_after_suspend`.
    "mov x8, #{sys_call}",
    "mov x0, #{slot_core}",
    "movz x2, #{info_suspend_hi}, lsl #16",
    "movk x2, #{info_suspend_lo}",
    "mov x3, #{defer}",
    "svc #0",
    // Defer **không** treo ⇒ chạy tới đây (SAI): Exit(99) để kernel thấy mã thoát.
    "mov x0, #99",
    "b __el0_exit_now",
    ".globl __el0_after_suspend",
    "__el0_after_suspend:",
    "mov x0, #0",
    "__el0_exit_now:",
    "mov x8, #{sys_exit}",
    "svc #0",
    "2:",
    "b 2b",
    sys_call = const vifast_abi::SYS_CALL,
    sys_send = const vifast_abi::SYS_SEND,
    sys_exit = const vifast_abi::SYS_EXIT,
    slot_core = const crate::coretest::SLOT_CORE as u64,
    slot_ep = const crate::coretest::SLOT_EP as u64,
    slot_frame = const crate::coretest::SLOT_FRAME as u64,
    slot_ro = const crate::coretest::SLOT_TCB_RO as u64,
    buf_va_hi = const (crate::coretest::IPC_BUF_VA >> 16) as u16,
    buf_va_lo = const (crate::coretest::IPC_BUF_VA & 0xFFFF) as u16,
    mr0_hi = const (crate::coretest::MSG_MR0 >> 16) as u16,
    mr0_lo = const (crate::coretest::MSG_MR0 & 0xFFFF) as u16,
    mr1_hi = const (crate::coretest::MSG_MR1 >> 16) as u16,
    mr1_lo = const (crate::coretest::MSG_MR1 & 0xFFFF) as u16,
    mr2_hi = const (crate::coretest::MSG_MR2 >> 16) as u16,
    mr2_lo = const (crate::coretest::MSG_MR2 & 0xFFFF) as u16,
    mr3_hi = const (crate::coretest::MSG_MR3 >> 16) as u16,
    mr3_lo = const (crate::coretest::MSG_MR3 & 0xFFFF) as u16,
    info_cfg3_hi = const (crate::coretest::INFO_CFG3 >> 16) as u16,
    info_cfg3_lo = const (crate::coretest::INFO_CFG3 & 0xFFFF) as u16,
    info_cfg2_hi = const (crate::coretest::INFO_CFG2 >> 16) as u16,
    info_cfg2_lo = const (crate::coretest::INFO_CFG2 & 0xFFFF) as u16,
    info_send_hi = const (crate::coretest::INFO_SEND >> 16) as u16,
    info_send_lo = const (crate::coretest::INFO_SEND & 0xFFFF) as u16,
    info_suspend_hi = const (crate::coretest::INFO_SUSPEND >> 16) as u16,
    info_suspend_lo = const (crate::coretest::INFO_SUSPEND & 0xFFFF) as u16,
    mask_prio = const crate::coretest::MASK_PRIORITY,
    mask_vspace = const crate::coretest::MASK_VSPACE,
    mask_entry = const crate::coretest::MASK_ENTRY,
    defer = const vifast_core::invoke::SUSPEND_MODE_DEFER,
);

extern "C" {
    /// Đầu section `.el0` (PA trong ảnh; đọc lúc PC ở nửa cao ⇒ phải đổi về PA).
    static __el0_start: u8;
    /// Cuối section `.el0`.
    static __el0_end: u8;
    /// Điểm vào EL0 (offset trong section).
    fn __el0_entry();
    /// **KT-338b** — điểm vào **pha core** (chương trình EL0 thứ hai).
    fn __el0_core_entry();
    /// **KT-338b** — điểm vào sau `Tcb.Suspend(Defer)` + `Tcb.Resume`.
    fn __el0_after_suspend();
}

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết.
///
/// Sau khi nhảy nửa cao, `adrp` cho alias `KERNEL_HIGH_BASE + (pa - text_start)`; PA thật là
/// `text_start_pa + (va - KERNEL_HIGH_BASE)`. Chỉ đúng cho địa chỉ **trong ảnh** — mọi symbol
/// dùng ở đây (`.el0`, `.bss`) đều nằm trong `[__text_start, __kernel_end)`.
fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - KERNEL_HIGH_BASE)
}

/// **Chạy EL0** — map trang mã + stack user rồi `eret` xuống EL0. **Không trả về.**
///
/// `text_start_pa` = PA liên kết của `__text_start` (đọc lúc PC còn ở nửa thấp, xem
/// `TEXT_START_PA` của ảnh). Mọi lỗi bảng trang là `PANIC` (fail-loud), không im lặng bỏ trang.
pub fn run(console: &mut Console, vs: &mut VSpace<SharedBootMemory>, text_start_pa: u64) -> ! {
    let code_start_pa = high_to_pa(core::ptr::addr_of!(__el0_start) as u64, text_start_pa);
    // **KT-338b** — nhớ PA mốc để `run_core_phase` tính VA của điểm vào thứ hai.
    TEXT_START_PA.store(text_start_pa, Ordering::Release);
    CODE_START_PA.store(code_start_pa, Ordering::Release);
    let code_end_pa = high_to_pa(core::ptr::addr_of!(__el0_end) as u64, text_start_pa);
    let entry_pa = high_to_pa(__el0_entry as *const () as u64, text_start_pa);
    let stack_pa = high_to_pa(core::ptr::addr_of!(EL0_STACK.0) as u64, text_start_pa);
    let code_bytes = code_end_pa.saturating_sub(code_start_pa);

    // Bất biến bố cục: section `.el0` phải vừa **một** Frame 4 KiB và entry phải nằm trong nó.
    if code_bytes == 0 || code_bytes > 0x1000 || entry_pa < code_start_pa || entry_pa >= code_end_pa
    {
        let _ = writeln!(
            console,
            "PANIC: umode: section .el0 không hợp lệ (start=0x{code_start_pa:x} end=0x{code_end_pa:x} bytes={code_bytes} entry=0x{entry_pa:x})"
        );
        debug_halt(3)
    }

    let code_frame = match Frame::new(code_start_pa) {
        Some(frame) => frame,
        None => {
            let _ = writeln!(console, "PANIC: umode: PA mã EL0 không căn 4 KiB");
            debug_halt(3)
        }
    };
    let stack_frame = match Frame::new(stack_pa) {
        Some(frame) => frame,
        None => {
            let _ = writeln!(console, "PANIC: umode: PA stack EL0 không căn 4 KiB");
            debug_halt(3)
        }
    };
    // Trang mã: **chỉ đọc + exec** ở EL0 (W^X); trang stack: đọc + ghi, **không** exec.
    if let Err(error) = vs.map_obj_with(EL0_CODE_VA, code_frame, Perms::USER_RX, MemAttr::Normal) {
        let _ = writeln!(console, "PANIC: umode: map mã EL0 lỗi {error:?}");
        debug_halt(3)
    }
    if let Err(error) = vs.map_obj_with(EL0_STACK_VA, stack_frame, Perms::USER_RW, MemAttr::Normal)
    {
        let _ = writeln!(console, "PANIC: umode: map stack EL0 lỗi {error:?}");
        debug_halt(3)
    }
    // VA mới chưa từng được truy cập nên TLB không có entry cũ; vẫn `activate` để chắc chắn
    // `TTBR0` đang là bảng vừa map (và để chuỗi TLBI của B3.6 chạy trên đường này).
    if let Err(error) = vs.activate() {
        let _ = writeln!(console, "PANIC: umode: activate lỗi {error:?}");
        debug_halt(3)
    }

    let entry = EL0_CODE_VA + (entry_pa - code_start_pa);
    let spsr = spsr_el0();
    let _ = writeln!(
        console,
        "# umode: eret entry=0x{entry:x} sp=0x{EL0_STACK_TOP:x} spsr=0x{spsr:x} mode={} code_va=0x{EL0_CODE_VA:x} code_pa=0x{code_start_pa:x} code_bytes={code_bytes} stack_va=0x{EL0_STACK_VA:x} stack_pa=0x{stack_pa:x}",
        mode_name(spsr),
    );
    // **KT-418** — **một cửa** của ngữ cảnh FPU/SIMD: gắn TCB đang chạy rồi khôi phục
    // `v0..v31`+`fpsr`/`fpcr` của **đúng** TCB đó trước `eret` (khe chưa hợp lệ ⇒ trạng thái mới).
    if let Some(tcb) = crate::coretest::current_tcb() {
        vifast_plat_aarch64::asm::fpu::bind_current(tcb);
    }
    vifast_plat_aarch64::asm::fpu::restore_current();
    asm::eret_to_el0(entry, EL0_STACK_TOP, spsr)
}

/// **Handler `svc`** — trả `Some(n)` nếu trap thuộc đường U-mode (đã resume hoặc đã dừng).
///
/// Ba nhánh, phân loại bằng **ô vector + `ESR_EL1.EC`** (không suy diễn từ chỗ khác):
///
/// 1. ô 8 + `EC = 0x15` ⇒ `svc` **từ EL0** — đường sản phẩm;
/// 2. ô 4 + `EC = 0x15` ⇒ `svc` **từ EL1** — đột biến (i) `SPSR_EL1` để EL1h: `eret` không hạ
///    đặc quyền, mã EL0 chạy ở EL1. In `# umode: svc from EL1` rồi dừng (ĐỎ, không im lặng);
/// 3. ô 8 + `EC = 0x20/0x24` ⇒ **fault từ EL0** — đột biến (ii) trang không `user=true`: in
///    `# umode: fault` kèm `ESR`/`FAR` rồi dừng (ĐỎ).
pub fn handle(slot: u64, esr_raw: u64, far: u64, elr: u64, spsr: u64, frame: u64) -> Option<u64> {
    let esr = Esr::from_bits(esr_raw);
    let ec = esr.ec_raw();

    if slot == SLOT_SYNC_LOWER_EL_AARCH64 && esr.ec() == Some(ExceptionClass::Svc64) {
        return Some(handle_svc(slot, esr, elr, spsr, frame));
    }
    if slot == SLOT_SYNC_CURRENT_EL_SPX && esr.ec() == Some(ExceptionClass::Svc64) {
        let mut console = Console;
        let _ = writeln!(
            console,
            "# umode: svc from EL1 (spsr=0x{spsr:x} mode={}) — SPSR_EL1 phải là EL0t (M=0b0000)",
            mode_name(spsr),
        );
        debug_halt(4)
    }
    if slot == SLOT_SYNC_LOWER_EL_AARCH64
        && (ec == EC_INSTRUCTION_ABORT_LOWER_EL || ec == EC_DATA_ABORT_LOWER_EL)
    {
        let mut console = Console;
        let _ = writeln!(
            console,
            "# umode: fault ec=0x{ec:02x} esr=0x{esr_raw:016x} far=0x{far:x} elr=0x{elr:x} spsr=0x{spsr:x} mode={}",
            mode_name(spsr),
        );
        debug_halt(4)
    }
    None
}

/// Xử lý một `svc` từ EL0 — phân phối theo **số hiệu syscall** (`x8`), không theo thứ tự gọi.
///
/// Phân phối theo `nr` (không theo bộ đếm) để một `svc` bị lặp vẫn là `DebugPutStr`, không bị
/// nhầm thành `Exit` — nhờ vậy đột biến "lùi `ELR`" (iii) thật sự lặp thay vì thoát sớm.
fn handle_svc(slot: u64, esr: Esr, elr: u64, spsr: u64, frame: u64) -> u64 {
    let mut console = Console;
    // **KT-339a** — đột biến (ii): bỏ nhánh `svc #0` của core trong pha shell sản phẩm ⇒ shell
    // **không** in prompt/help/ps ⇒ TAP 45/46 ĐỎ, log sản phẩm thiếu `shell>`.
    if crate::pshell::svc_blocked() {
        let _ = writeln!(&mut console, "PANIC: shell: svc blocked (mut-no-svc)");
        debug_halt(5)
    }
    let imm = svc_imm(esr.iss());
    let nr = asm::frame_get(frame, FRAME_X8);
    let count = SVC_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let core_phase = IN_CORE_PHASE.load(Ordering::Acquire);

    // TAP 16: lần `svc` **đầu tiên** — đã thật sự vào EL0 (SPSR M=0) và trap đi vào ô
    // "Lower EL, AArch64, Sync" (ô 8). In **một** lần, không in lại khi `svc` lặp.
    if count == 1 {
        let entered = is_el0(spsr) && slot == SLOT_SYNC_LOWER_EL_AARCH64;
        tap(&mut console, 16, "umode: el0 entered", entered);
    }

    // **KT-338b** — tham số syscall theo `[calling.aarch64]`: `x0`/`x1` = 2 tham số,
    // `x2` = `MessageInfo`, `x3..x6` = MR0..MR3.
    let x0 = asm::frame_get(frame, FRAME_X0);
    let x1 = asm::frame_get(frame, FRAME_X1);
    let x2 = asm::frame_get(frame, FRAME_X2);
    let x3 = asm::frame_get(frame, FRAME_X3);
    let x4 = asm::frame_get(frame, FRAME_X4);
    let x5 = asm::frame_get(frame, FRAME_X5);
    let x6 = asm::frame_get(frame, FRAME_X6);
    // `vifast_core::syscall::dispatch` nhận `args[0..2]` = 2 tham số đầu và `args[1]` là
    // `MessageInfo` cho các syscall **gửi** (0/1/2) — nên ba syscall ấy dịch `x2..x6` vào
    // `args[1..6]`; các syscall còn lại giữ `x0..x5` (Recv: `args[1]` = cap Reply).
    let args = match nr {
        vifast_abi::SYS_CALL | vifast_abi::SYS_SEND | vifast_abi::SYS_NB_SEND => {
            [x0, x2, x3, x4, x5, x6]
        }
        _ => [x0, x1, x2, x3, x4, x5],
    };

    // **KT-339** — pha K5 đặt TCB hiện hành riêng (`procsrv`/`shell`/`echo`); ngoài K5 vẫn là
    // TCB của coretest (lượt 1 + pha core KT-338b).
    let Some(tcb) = crate::k5user::current()
        .or_else(crate::pshell::current)
        .or_else(crate::coretest::current_tcb)
    else {
        let _ = writeln!(
            console,
            "PANIC: umode: không có TCB core cho svc nr={nr} (coretest::prepare chưa chạy)"
        );
        debug_halt(1)
    };
    // **KT-338b** — đường **duy nhất** của `svc`: `vifast_core::syscall::dispatch` trên `Machine`
    // thật, dưới `core_bridge::with_machine` (khoá `IrqMcsLock` tắt `DAIF.I`). Không nhánh tắt,
    // không xử lý syscall nào ngoài core.
    let outcome = vifast_plat_aarch64::core_bridge::with_machine(|m| {
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
            if nr == vifast_abi::SYS_DEBUG_PUT_STR && !core_phase && count == 1 {
                tap(
                    &mut console,
                    17,
                    "umode: svc handled",
                    result.error == 0 && result.value == x1,
                );
                let _ = writeln!(console, "# umode: el0 ran svc={imm} ret={}", result.value);
                let _ = writeln!(
                    console,
                    "# umode: svc nr={nr} imm={imm} elr=0x{elr:x} ptr=0x{x0:x} len={x1} printed={} error={} ret={}",
                    result.value, result.error, result.value
                );
            }
            // **KT-338b** — ghi bằng chứng của pha core (kernel đọc lại, không tin chương trình).
            if core_phase {
                crate::coretest::record_syscall(result.error, result.value);
            }
            // Trả theo **đúng** `[calling.aarch64]`: `x0` = error, `x1` = value, `x2` =
            // `MessageInfo`, `x3..x6` = MR0..MR3.
            asm::frame_set(frame, FRAME_X0, result.error);
            asm::frame_set(frame, FRAME_X1, result.value);
            asm::frame_set(frame, FRAME_X2, result.info);
            for (index, value) in result.mr.iter().enumerate() {
                asm::frame_set(frame, FRAME_X3 + index, *value);
            }
            // **KT-339** — dịch vụ kernel giữa hai `svc` của pha K5. `Some(entry)` ⇔ pha xong ⇒
            // kernel tiếp quản tại hàm EL1 `entry` (cùng khuôn `Exit` của KT-338b).
            if let Some(entry) = crate::k5user::after_return(nr, &result) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
            } else if let Some(entry) = crate::pshell::after_return(nr, &result, &mut console) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
            }
            // **AArch64 `svc`**: `ELR_EL1` đã là instruction **kế tiếp** (DDI 0487 §D17.2.37) nên
            // handler **không** cộng 4 — cùng lý do đã ghi ở KT-269.
            1
        }
        SyscallOutcome::Exit { code } => {
            // **KT-418** — tác vụ dừng: lưu `v0..v31`+`fpsr`/`fpcr` vào khe của TCB (eager; aarch64
            // không có cờ bẩn phần cứng tương đương `sstatus.FS`).
            vifast_plat_aarch64::asm::fpu::save_current();
            // **KT-339** — `Exit` của tác vụ K5: ghi mã thoát, chuyển pha, kernel tiếp quản.
            if let Some(entry) = crate::k5user::after_exit(code) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
                return 1;
            }
            if let Some(entry) = crate::pshell::after_exit(code) {
                asm::write_elr_el1(entry);
                asm::write_spsr_el1(spsr_el1h());
                return 1;
            }
            if core_phase {
                crate::coretest::record_exit(code);
                asm::write_elr_el1(core_done as *const () as u64);
            } else {
                // TAP 18: EL0 **đã chạy tiếp** sau `svc` đầu và gọi `Exit`.
                let returned = is_el0(spsr) && slot == SLOT_SYNC_LOWER_EL_AARCH64;
                tap(&mut console, 18, "umode: returned to el0", returned);
                let _ = writeln!(console, "# umode: exit code={code} nr={nr} imm={imm}");
                // `Exit` không trả về EL0: `eret` về EL1h tại `el0_done` (in marker + halt).
                asm::write_elr_el1(el0_done as *const () as u64);
            }
            asm::write_spsr_el1(spsr_el1h());
            1
        }
        SyscallOutcome::Halt { code } => {
            let _ = writeln!(console, "# umode: halt code={code} nr={nr} imm={imm}");
            debug_halt(code as usize)
        }
        SyscallOutcome::Switched { state } => {
            // **KT-339** — pha K5 là "kernel scheduler tuần tự": `Yield` của tác vụ U-mode duy
            // nhất `Running` phải quay lại chính nó. Nếu core trả `Switched` (ví dụ đột biến ưu
            // tiên làm TCB khác được chọn), arch **không** có scheduler để chuyển ngữ cảnh ⇒ coi
            // như `Return` và để `after_return` quyết định; TAP/bằng chứng sẽ lộ bất thường.
            if crate::k5user::active() {
                asm::frame_set(frame, FRAME_X0, 0);
                asm::frame_set(frame, FRAME_X1, 0);
                asm::frame_set(frame, FRAME_X2, 0);
                for index in 0..4 {
                    asm::frame_set(frame, FRAME_X3 + index, 0);
                }
                if let Some(entry) =
                    crate::k5user::after_return(nr, &vifast_core::syscall::SyscallReturn::new(0, 0))
                {
                    asm::write_elr_el1(entry);
                    asm::write_spsr_el1(spsr_el1h());
                }
                return 1;
            }
            // **KT-339a** — đường shell sản phẩm: "kernel là scheduler tuần tự" ⇒ `Switched` coi
            // như `Return` để hook dịch vụ kernel quyết định (cùng khuôn `k5user`).
            if crate::pshell::active() {
                asm::frame_set(frame, FRAME_X0, 0);
                asm::frame_set(frame, FRAME_X1, 0);
                asm::frame_set(frame, FRAME_X2, 0);
                for index in 0..4 {
                    asm::frame_set(frame, FRAME_X3 + index, 0);
                }
                if let Some(entry) = crate::pshell::after_return(
                    nr,
                    &vifast_core::syscall::SyscallReturn::new(0, 0),
                    &mut console,
                ) {
                    asm::write_elr_el1(entry);
                    asm::write_spsr_el1(spsr_el1h());
                }
                return 1;
            }
            if core_phase {
                // **Điểm an toàn `Tcb.Suspend(Defer)`** (KT-329): TCB vừa bị treo ⇒ kernel
                // `Resume` rồi quay lại EL0 tại `TcbData.entry`. Ghi `(0, 0)` cho syscall này để
                // bộ đếm khớp thứ tự; TAP của ca (d) đọc `SWITCHED`/`SUSPEND_STATE` độc lập.
                crate::coretest::record_syscall(0, 0);
                let _ = writeln!(
                    console,
                    "# core switch: nr={nr} state=0x{:x} — điểm an toàn, kernel tiếp quản",
                    state_code(state)
                );
                asm::write_elr_el1(core_switched as *const () as u64);
                asm::write_spsr_el1(spsr_el1h());
                return 1;
            }
            let _ = writeln!(
                console,
                "PANIC: umode: svc nr={nr} trả Switched ngoài pha core (state=0x{:x})",
                state_code(state)
            );
            debug_halt(4)
        }
    }
}

/// Số ổn định của `TcbState` cho log (một nguồn: `coretest::state_code`).
fn state_code(state: vifast_core::TcbState) -> u64 {
    crate::coretest::state_code(state)
}

// ---------------------------------------------------------------------------------------------
// **KT-338b** — pha core: chương trình EL0 thứ hai + hai điểm về của kernel
// ---------------------------------------------------------------------------------------------

/// VA user của điểm vào thứ hai trong `.el0` (`__el0_after_suspend`) — kernel đặt vào
/// `Tcb.Configure(entry?)` để `Tcb.Resume` giao lại **đúng** giá trị đó (D2-83).
pub fn core_after_suspend_va() -> u64 {
    let text_start = TEXT_START_PA.load(Ordering::Acquire);
    let code_start = CODE_START_PA.load(Ordering::Acquire);
    if text_start == 0 || code_start == 0 {
        return 0;
    }
    let pa = high_to_pa(__el0_after_suspend as *const () as u64, text_start);
    EL0_CODE_VA + pa.saturating_sub(code_start)
}

/// `SPSR_EL1` để `eret` xuống **EL0t** ở pha core — cùng `spsr_el0` nhưng đọc lại từ nguồn duy
/// nhất của `vifast-plat-aarch64::umode` (không chép bit tay).
pub fn spsr_el0_phase() -> u64 {
    spsr_el0()
}

/// `eret` xuống EL0 tại điểm vào **pha core**. **Không trả về.**
pub fn run_core_phase(console: &mut Console) -> ! {
    let text_start = TEXT_START_PA.load(Ordering::Acquire);
    let code_start = CODE_START_PA.load(Ordering::Acquire);
    if text_start == 0 || code_start == 0 {
        let _ = writeln!(console, "PANIC: umode: chưa có PA mốc cho pha core");
        debug_halt(3)
    }
    let entry_pa = high_to_pa(__el0_core_entry as *const () as u64, text_start);
    let entry = EL0_CODE_VA + entry_pa.saturating_sub(code_start);
    IN_CORE_PHASE.store(true, Ordering::Release);
    let _ = writeln!(
        console,
        "# core phase: eret entry=0x{entry:x} sp=0x{EL0_STACK_TOP:x} mode=EL0t nr_syscalls={}",
        crate::coretest::CORE_SYSCALL_RECORDS
    );
    asm::eret_to_el0(entry, EL0_STACK_TOP, spsr_el0())
}

/// Điểm về **EL1h** sau khi `dispatch` trả `Switched` ở pha core: `Tcb.Resume` rồi quay lại EL0
/// tại điểm vào core giao. **Không trả về.**
#[no_mangle]
pub extern "C" fn core_switched() -> ! {
    let mut console = Console;
    crate::coretest::resume_after_switch(&mut console)
}

/// Điểm về **EL1h** sau `Exit` của pha core: đo bằng chứng, phát TAP 36..41, marker + halt.
#[no_mangle]
pub extern "C" fn core_done() -> ! {
    let mut console = Console;
    crate::coretest::finish(&mut console)
}

/// Điểm về của `Exit` — chạy ở **EL1h** sau `eret` của handler, in marker rồi tắt máy.
///
/// Đây là "kết thúc tiến trình" của B3.7: chưa có procsrv/scheduler nên kernel tự in marker
/// (marker **chỉ** được in khi mọi ca TAP đã xanh — nếu in sớm, một ảnh hỏng vẫn có marker).
///
/// **KT-273**: trước marker, chạy ba phép đo PCI/virtio-blk (TAP 19/20/21). Chúng phải ở **đây**
/// (không phải `high_entry`) để số hiệu TAP tuần tự sau `ok 18`; `VSpace` vẫn đang hoạt động nên
/// MMIO tới ECAM/BAR đi qua bảng trang đã map trong `high_entry`.
///
/// **KT-275**: sau `blk`, chạy bốn phép đo mạng (TAP 22/23/24/25) — cùng lý do thứ tự TAP.
/// **KT-277**: sau PCI, chạy hai phép đo **W^X** (TAP 22) và **cache maintenance trước DMA**
/// (TAP 23). Phép đo W^X phải ở đây vì nó chỉ có nghĩa khi bảng trang **đang hoạt động** là bảng
/// của `VSpace` (đã `activate` trong `high_entry`); phép đo DMA đọc bộ đếm mà `blk::run` vừa
/// sinh ra.
#[no_mangle]
pub extern "C" fn el0_done() -> ! {
    let mut console = Console;
    let _ = writeln!(console, "# umode: done");
    let blk_ok = crate::blk::run(&mut console);
    if !crate::net::run(&mut console) {
        debug_halt(5)
    }
    if !crate::wxprobe::run(&mut console, blk_ok) {
        debug_halt(5)
    }
    // **KT-279**: virtio-gpu 2D + console trên framebuffer — TAP 28/29/30, sau `wxprobe` để số
    // hiệu TAP tuần tự. Không có `-device virtio-gpu-pci` ⇒ `# gpu: no device` + ba TAP đỏ ⇒
    // dừng **trước** marker (ca âm của runner).
    if !crate::gpu::run(&mut console) {
        debug_halt(5)
    }
    // **KT-283**: virtio-input (bàn phím) — TAP 31/32, sau `gpu` để số hiệu TAP tuần tự. Không có
    // `-device virtio-keyboard-pci` ⇒ `# input: no device` + hai TAP đỏ ⇒ dừng **trước** marker
    // (ca âm của runner). Có thiết bị nhưng không có phím ⇒ TAP 32 đỏ sau `INPUT_WAIT_SECS`.
    if !crate::input::run(&mut console) {
        debug_halt(5)
    }
    // **KT-303** — TAP 35 (DNS) đỏ ⇒ **không** in marker (bất biến "TAP đỏ ⇒ không marker"),
    // nhưng các bước `wx`/`gpu`/`input` đã chạy và in TAP của chúng.
    if crate::net::dns_red() {
        debug_halt(5)
    }
    // **KT-338b** — pha core: chương trình EL0 **thứ hai** gọi `Tcb.Configure`/`Send`/
    // `Tcb.Suspend` qua `svc #0` → `vifast_core::syscall::dispatch`. TAP 36..41 phát ở
    // `coretest::finish` (đường `Exit` của pha core), rồi marker + halt. Từ đây **không** dùng
    // `el0_done` nữa: mọi TAP 1..35 đã xanh trước khi vào pha này.
    crate::coretest::start(&mut console)
}
