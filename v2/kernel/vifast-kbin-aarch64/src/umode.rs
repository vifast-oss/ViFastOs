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
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::bootmem::SharedBootMemory;
use vifast_plat_aarch64::fault::{EC_DATA_ABORT_LOWER_EL, EC_INSTRUCTION_ABORT_LOWER_EL};
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::split_mmu::KERNEL_HIGH_BASE;
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};
use vifast_plat_aarch64::umode::{
    is_el0, mode_name, spsr_el0, spsr_el1h, svc_imm, EL0_CODE_VA, EL0_STACK_TOP, EL0_STACK_VA,
    FRAME_X0, FRAME_X1, FRAME_X8,
};
use vifast_plat_aarch64::vspace::{Frame, VSpace};
use vifast_plat_aarch64::{debug_halt, pl011};

use crate::{tap, Console, BOOT_MARKER};

/// Ô vector **đồng bộ, Lower EL dùng AArch64** (`0x400`) — nơi `svc` từ EL0 đi vào.
const SLOT_SYNC_LOWER_EL_AARCH64: u64 = 8;
/// Ô vector **đồng bộ, Current EL dùng SPx** (`0x200`) — nơi `svc` từ EL1 đi vào (đột biến (i)).
const SLOT_SYNC_CURRENT_EL_SPX: u64 = 4;

/// Chuỗi EL0 in qua `DebugPutStr` — nằm **cùng trang** với mã EL0 (stub dùng `adr`).
const EL0_MSG: &str = "el0 says hi\n";
/// Độ dài chuỗi — `const` operand cho `mov x1, #len` của stub.
const EL0_MSG_LEN: u64 = EL0_MSG.len() as u64;
/// Trần số byte in từ con trỏ user — chống treo/treo log nếu `len` sai (fail-closed, không tin EL0).
const MAX_USER_PRINT: u64 = 256;

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

/// Tổng số byte **user** đã in — trần [`MAX_USER_PRINT`] cho **cả** lần boot.
///
/// Đột biến (iii) "handler không tăng `ELR_EL1`" làm `svc` lặp vô hạn; nếu mỗi vòng lại in
/// chuỗi user thì log phình tới hàng trăm MB. Trần này giữ log **hữu hạn** trong khi máy vẫn
/// treo có kiểm soát (runner đỏ vì thiếu marker, không phải vì hết đĩa).
static USER_BYTES_PRINTED: AtomicU64 = AtomicU64::new(0);

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

extern "C" {
    /// Đầu section `.el0` (PA trong ảnh; đọc lúc PC ở nửa cao ⇒ phải đổi về PA).
    static __el0_start: u8;
    /// Cuối section `.el0`.
    static __el0_end: u8;
    /// Điểm vào EL0 (offset trong section).
    fn __el0_entry();
}

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết.
///
/// Sau khi nhảy nửa cao, `adrp` cho alias `KERNEL_HIGH_BASE + (pa - text_start)`; PA thật là
/// `text_start_pa + (va - KERNEL_HIGH_BASE)`. Chỉ đúng cho địa chỉ **trong ảnh** — mọi symbol
/// dùng ở đây (`.el0`, `.bss`) đều nằm trong `[__text_start, __kernel_end)`.
fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - KERNEL_HIGH_BASE)
}

/// In tối đa [`MAX_USER_PRINT`] byte từ con trỏ **user**; trả số byte đã in.
///
/// Kernel đọc được trang user vì `SCTLR_EL1.SPAN = 1` (PAN không bật khi vào exception) — đo
/// được: nếu PAN bật, lần đọc đầu fault và handler in `# umode: fault` thay vì im lặng.
fn print_user(ptr: u64, len: u64) -> u64 {
    let already = USER_BYTES_PRINTED.load(Ordering::Relaxed);
    let budget = MAX_USER_PRINT.saturating_sub(already);
    let limit = len.min(budget);
    let mut printed = 0;
    while printed < limit {
        let byte = asm::read_u8(ptr + printed);
        let _ = pl011::write_byte(byte);
        printed += 1;
    }
    USER_BYTES_PRINTED.fetch_add(printed, Ordering::Relaxed);
    printed
}

/// **Chạy EL0** — map trang mã + stack user rồi `eret` xuống EL0. **Không trả về.**
///
/// `text_start_pa` = PA liên kết của `__text_start` (đọc lúc PC còn ở nửa thấp, xem
/// `TEXT_START_PA` của ảnh). Mọi lỗi bảng trang là `PANIC` (fail-loud), không im lặng bỏ trang.
pub fn run(console: &mut Console, vs: &mut VSpace<SharedBootMemory>, text_start_pa: u64) -> ! {
    let code_start_pa = high_to_pa(core::ptr::addr_of!(__el0_start) as u64, text_start_pa);
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
    let imm = svc_imm(esr.iss());
    let nr = asm::frame_get(frame, FRAME_X8);
    let count = SVC_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

    // TAP 16: lần `svc` **đầu tiên** — đã thật sự vào EL0 (SPSR M=0) và trap đi vào ô
    // "Lower EL, AArch64, Sync" (ô 8). In **một** lần, không in lại khi `svc` lặp.
    if count == 1 {
        let entered = is_el0(spsr) && slot == SLOT_SYNC_LOWER_EL_AARCH64;
        tap(&mut console, 16, "umode: el0 entered", entered);
    }

    if nr == vifast_abi::SYS_DEBUG_PUT_STR {
        let ptr = asm::frame_get(frame, FRAME_X0);
        let len = asm::frame_get(frame, FRAME_X1);
        let printed = print_user(ptr, len);
        // Trả kết quả theo **đúng** quy ước: x0 = error (0 = OK), x1 = value.
        asm::frame_set(frame, FRAME_X0, 0);
        asm::frame_set(frame, FRAME_X1, len);
        if count == 1 {
            tap(&mut console, 17, "umode: svc handled", printed == len);
            let _ = writeln!(console, "# umode: el0 ran svc={imm} ret={len}");
            let _ = writeln!(
                console,
                "# umode: svc nr={nr} imm={imm} elr=0x{elr:x} ptr=0x{ptr:x} len={len} printed={printed} error=0 ret={len}"
            );
        }
        // **AArch64 `svc`**: `ELR_EL1` đã là địa chỉ instruction **kế tiếp** (DDI 0487 §D17.2.37:
        // preferred return address của SVC là instruction sau SVC — khác `brk`, nơi ELR trỏ vào
        // chính instruction). Vì vậy handler **không** cộng 4; cộng 4 là bỏ một instruction của
        // EL0 (đo được: bỏ `mov x9, x0`). Hệ quả cho đột biến (iii): "không tăng `ELR`" là hành
        // vi **đúng** trên AArch64, không phải đột biến; đột biến tương đương là **lùi** `ELR`
        // (`elr - 4` ⇒ `svc` thứ hai đọc `x0 = 0` ⇒ data abort EL1; lùi về đầu stub ⇒ lặp vô
        // hạn) — log ở `v2/docs/k4-aarch64-boot.md` §9.4.
        return 1;
    }

    if nr == vifast_abi::SYS_EXIT {
        // TAP 18: EL0 **đã chạy tiếp** sau `svc` đầu và gọi `Exit` — bằng chứng "returned to el0".
        let returned = is_el0(spsr) && slot == SLOT_SYNC_LOWER_EL_AARCH64;
        tap(&mut console, 18, "umode: returned to el0", returned);
        let code = asm::frame_get(frame, FRAME_X0);
        let _ = writeln!(console, "# umode: exit code={code} nr={nr} imm={imm}");
        // `Exit` không trả về EL0: `eret` về EL1h tại `el0_done` (in marker + halt).
        asm::write_elr_el1(el0_done as *const () as u64);
        asm::write_spsr_el1(spsr_el1h());
        return 1;
    }

    // Số hiệu lạ (chưa cài): in **một** lần rồi resume — không treo máy vì một syscall lạ, và
    // TAP 17 đã `not ok` nếu `DebugPutStr` không tới (đo được, không im lặng).
    if count <= 2 {
        let _ = writeln!(
            console,
            "# umode: svc nr={nr} imm={imm} — không phải DebugPutStr ({})/Exit ({})",
            vifast_abi::SYS_DEBUG_PUT_STR,
            vifast_abi::SYS_EXIT,
        );
    }
    1
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
#[no_mangle]
pub extern "C" fn el0_done() -> ! {
    let mut console = Console;
    let _ = writeln!(console, "# umode: done");
    if !crate::blk::run(&mut console) {
        debug_halt(5)
    }
    if !crate::net::run(&mut console) {
        debug_halt(5)
    }
    let _ = pl011::write_str(BOOT_MARKER);
    let _ = writeln!(console, "# viFastOs aarch64: halt");
    debug_halt(0)
}
