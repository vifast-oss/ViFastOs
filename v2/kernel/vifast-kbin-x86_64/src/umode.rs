// SPDX-License-Identifier: MIT OR Apache-2.0
//! **U-mode (ring 3) của ảnh x86_64** — KT-270 (K5' bước 4; D2-207).
//!
//! # Đường đi (một chiều, không quay lại kernel bằng `ret`)
//!
//! ```text
//!   kmain (CPL0, stack boot)
//!     ├─ `_start` đã dựng cây bảng trang user: PDPT[96] → __pd_user → __pt_user
//!     │    ├─ lá 0 = __user_start | P|US        (mã ring 3: đọc + exec, W^X)
//!     │    └─ lá 1 = __user_stack | P|RW|US|NX  (stack ring 3: đọc + ghi, không exec)
//!     ├─ TSS.RSP0 ← __syscall_stack_top (stack kernel riêng cho đường vào từ CPL3)
//!     ├─ iretq: SS=0x23, RSP=USER_STACK_TOP, RFLAGS=0x202 (IF=1), CS=0x1B, RIP=entry
//!     ↓
//!   ring 3: ghi/đọc stack user → `int 0x80` DebugPutStr(ptr, len)
//!     ↑ stub `vifast_isr_syscall` (crate plat): đẩy frame, gọi `syscall_dispatch`
//!       → in chuỗi user qua COM1 → trả (rax=0, rdx=len) → `iretq` về ring 3
//!     ↓
//!   ring 3: `int 0x80` Exit(0)
//!     ↑ handler: TAP 14 (ring 3 **đã chạy tiếp** sau `int 0x80` đầu) → marker + halt
//! ```
//!
//! # Vì sao `Exit` dừng trong handler chứ không `iretq` về kernel
//!
//! `int 0x80` không phải lời gọi hàm: kernel **không** có địa chỉ trả về trên stack của ring 3.
//! Đường sản phẩm của một syscall `Exit` là **không trả về ring 3**; ở đây (K5' bước 4, chưa có
//! scheduler/procsrv) nó in marker rồi tắt máy — đúng ngữ nghĩa "Exit kết thúc tiến trình", chỉ
//! khác là chưa có procsrv để nhận `FaultKind::Exit`.
//!
//! # Bất biến
//!
//! - **Chỉ một** syscall thật (`DebugPutStr`) + `Exit`; **không** IPC, **không** nhiều process,
//!   **không** preemption (D2-207 giới hạn phạm vi).
//! - Trang mã ring 3 map `US = 1`, **chỉ đọc + exec**; trang stack `US = 1`, đọc + ghi, **NX** —
//!   W^X ở mức trang user; ảnh kernel vẫn map identity `US = 0` nên ring 3 **không** đọc được
//!   kernel.
//! - Số hiệu syscall lấy từ **hằng sinh** `vifast_abi::generated` (`SYS_DEBUG_PUT_STR`,
//!   `SYS_EXIT`) — không chép tay số 12/14 (D2-11). Tên thanh ghi (`rax`, `rdi`, `rsi`, `rdx`) là
//!   chỗ duy nhất viết tay, đúng `spec/abi.toml [calling.x86_64]`; sai thanh ghi ⇒ handler đọc số
//!   hiệu 0 ⇒ TAP 12 `not ok` (đo được, không im lặng).

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::umode::{
    is_user_cs, SyscallRet, USER_CODE_VA, USER_CS, USER_RFLAGS, USER_SS, USER_STACK_TOP,
    USER_STACK_VA,
};

use crate::{check, write_byte, Console};

/// `RSP` **user** của tác vụ ring 3 khi scheduler `iretq` xuống (KT-304) — chừa 16 byte trên
/// đỉnh trang stack user cho bộ đếm [`USER_COUNTER_VA`].
pub(crate) const USER_TASK_RSP: u64 = USER_STACK_TOP - 16;

/// VA **bộ đếm** mà tác vụ ring 3 tự tăng — 8 byte cuối trang stack user (trang `RW`, `US = 1`).
///
/// Kernel **zero** ô này trước khi bật scheduler (`sched::run`) rồi đọc lại sau khi scheduler
/// dừng: `n > 0` là bằng chứng tác vụ ring 3 **đã chạy thật**, không phải hằng. Zero trước là
/// bắt buộc — chương trình ring 3 của KT-270 đã ghi `0x1234` vào đúng ô này, nên không zero thì
/// "chưa chạy" vẫn đọc ra số dương (dương tính giả).
pub(crate) const USER_COUNTER_VA: u64 = USER_STACK_TOP - 8;

// Stub **ring 3** của tác vụ scheduler (KT-304): tăng bộ đếm `USER_COUNTER_VA` trong vòng lặp
// vô hạn; timer preempt nó, scheduler chạy lại nó, và kernel đọc bộ đếm sau khi dừng.
//
// Nằm trong **cùng** section `.user` với stub KT-270 (linker gom `KEEP(*(.user))`, `ASSERT` ≤
// một trang 4 KiB) nên được map ở `USER_CODE_VA` với `US = 1`, chỉ đọc + exec. Không `push`
// (không đụng `rsp` user) — bộ đếm ở cuối trang stack, không chồng lên `rsp`.
core::arch::global_asm!(
    ".section .user, \"ax\"",
    ".p2align 4",
    ".globl __user_sched_entry",
    "__user_sched_entry:",
    "mov rcx, {counter}",
    "1:",
    "inc qword ptr [rcx]",
    "jmp 1b",
    counter = const USER_COUNTER_VA,
);

/// Chuỗi ring 3 in qua `DebugPutStr` — nằm **cùng trang** với mã ring 3 (stub dùng `lea`).
pub(crate) const USER_MSG: &str = "ring3 says hi\n";
/// Độ dài chuỗi — `const` operand cho `mov rsi, #len` của stub.
pub(crate) const USER_MSG_LEN: u64 = USER_MSG.len() as u64;

/// `const` chốt: chuỗi trong asm (`.ascii`) và [`USER_MSG`] phải **cùng độ dài** — lệch là lỗi
/// biên dịch, không phải sai âm thầm lúc chạy.
const _: () = assert!(USER_MSG_LEN == 14);

/// Trần số byte in từ con trỏ user — chống treo/phình log nếu `len` sai (fail-closed, không tin
/// ring 3). Đột biến (iii) "lùi `rip`" làm `int 0x80` lặp vô hạn; trần này giữ log **hữu hạn**
/// trong khi máy vẫn treo có kiểm soát (runner đỏ vì thiếu marker, không phải vì hết đĩa).
const MAX_USER_PRINT: u64 = 256;

/// Số lần `int 0x80` đã vào handler — in bằng chứng **một lần** cho mỗi ca.
static SYSCALL_COUNT: AtomicU64 = AtomicU64::new(0);

/// Tổng số byte **user** đã in — trần [`MAX_USER_PRINT`] cho **cả** lần boot.
static USER_BYTES_PRINTED: AtomicU64 = AtomicU64::new(0);

extern "C" {
    /// Đầu section `.user` (PA trong ảnh; identity map ⇒ VA == PA).
    static __user_start: u8;
    /// Cuối section `.user`.
    static __user_end: u8;
    /// Điểm vào ring 3 (offset trong section).
    fn __user_entry();
    /// **KT-304**: điểm vào ring 3 của tác vụ scheduler (cùng section `.user`).
    fn __user_sched_entry();
    /// Trang stack user (PA trong `.bss`; map ở [`USER_STACK_VA`] với `US = 1`).
    static __user_stack: u8;
    /// Đỉnh stack kernel riêng cho đường vào từ ring 3 (`TSS.RSP0`).
    static __syscall_stack_top: u8;
}

/// VA của entry **tác vụ ring 3** của scheduler (KT-304) — `USER_CODE_VA + (PA - __user_start)`.
///
/// Cùng phép tính với [`run`] cho `__user_entry`: section `.user` được map **nguyên trang** ở
/// `USER_CODE_VA`, nên offset trong section là offset trong trang. Hàm **không** kiểm bố cục —
/// `run` đã `PANIC` nếu section không vừa một trang; ở đây chỉ cần offset.
pub(crate) fn sched_entry_va() -> u64 {
    let code_start = core::ptr::addr_of!(__user_start) as u64;
    let entry_pa = __user_sched_entry as *const () as u64;
    USER_CODE_VA + entry_pa.saturating_sub(code_start)
}

/// In tối đa [`MAX_USER_PRINT`] byte từ con trỏ **user**; trả số byte đã in.
///
/// Kernel đọc được trang user vì `CR4.SMAP = 0` (chưa bật SMAP) — đo được: nếu SMAP bật, lần đọc
/// đầu fault và handler in `# exc: vec=14` thay vì im lặng.
fn print_user(ptr: u64, len: u64) -> u64 {
    let already = USER_BYTES_PRINTED.load(Ordering::Relaxed);
    let budget = MAX_USER_PRINT.saturating_sub(already);
    let limit = len.min(budget);
    let mut printed = 0;
    while printed < limit {
        let byte = asm::read_u8(ptr + printed);
        write_byte(byte);
        printed += 1;
    }
    USER_BYTES_PRINTED.fetch_add(printed, Ordering::Relaxed);
    printed
}

/// **Chạy ring 3** — in bằng chứng, đặt `TSS.RSP0` sang stack syscall riêng rồi `iretq` xuống
/// CPL3. **Không trả về.**
///
/// Mọi lỗi bố cục section là `PANIC` (fail-loud), không im lặng bỏ trang.
pub fn run(console: &mut Console) -> ! {
    let code_start = core::ptr::addr_of!(__user_start) as u64;
    let code_end = core::ptr::addr_of!(__user_end) as u64;
    let entry_pa = __user_entry as *const () as u64;
    let stack_pa = core::ptr::addr_of!(__user_stack) as u64;
    let syscall_stack_top = core::ptr::addr_of!(__syscall_stack_top) as u64;
    let code_bytes = code_end.saturating_sub(code_start);

    // Bất biến bố cục: section `.user` phải vừa **một** Frame 4 KiB và entry phải nằm trong nó.
    if code_bytes == 0 || code_bytes > 0x1000 || entry_pa < code_start || entry_pa >= code_end {
        let _ = writeln!(
            console,
            "PANIC: umode: section .user không hợp lệ (start=0x{code_start:x} end=0x{code_end:x} bytes={code_bytes} entry=0x{entry_pa:x})"
        );
        asm::halt(3)
    }

    let entry = USER_CODE_VA + (entry_pa - code_start);
    let _ = writeln!(
        console,
        "# umode: iretq entry=0x{entry:x} sp=0x{USER_STACK_TOP:x} rflags=0x{USER_RFLAGS:x} cs=0x{USER_CS:x} ss=0x{USER_SS:x} code_va=0x{USER_CODE_VA:x} code_pa=0x{code_start:x} code_bytes={code_bytes} stack_va=0x{USER_STACK_VA:x} stack_pa=0x{stack_pa:x} syscall_stack=0x{syscall_stack_top:x}"
    );
    // Stack kernel **riêng** cho đường vào từ CPL3: `kmain` không quay lại sau `iretq`, nhưng
    // dùng stack boot làm `RSP0` sẽ để handler đè lên frame còn sống của `kmain`.
    //
    // **KT-278**: TSS là per-hart — ring 3 chỉ chạy trên **hart 0** (BSP), nên chỉ đổi `RSP0` của
    // slot 0. Ba hart phụ không bao giờ xuống CPL3 ở bước này.
    tss::set_rsp0(0, syscall_stack_top);
    asm::enter_user(entry, USER_STACK_TOP, USER_RFLAGS)
}

/// Handler **`int 0x80`** — stub asm (`vifast_isr_syscall`) gọi hàm này.
///
/// Tham số theo SysV: `nr` = `rax`, `a0`/`a1` = `rdi`/`rsi`, `mi` = `rdx`, `mr0`/`mr1` =
/// `r10`/`r8`, `cs`/`rip` = frame của CPU (để chứng minh **đã vào từ ring 3**). Trả
/// [`SyscallRet`] (`rax` = error, `rdx` = value) — stub ghi vào frame trước `iretq`.
///
/// Phân phối theo **số hiệu** (không theo bộ đếm) để một `int 0x80` bị lặp vẫn là `DebugPutStr`,
/// không bị nhầm thành `Exit` — nhờ vậy đột biến "lùi `rip`" (iii) thật sự lặp thay vì thoát sớm.
#[no_mangle]
pub extern "C" fn syscall_dispatch(
    nr: u64,
    a0: u64,
    a1: u64,
    _mi: u64,
    _mr0: u64,
    _mr1: u64,
    cs: u64,
    rip: u64,
) -> SyscallRet {
    let mut console = Console;
    let count = SYSCALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let user = is_user_cs(cs);

    // TAP 12: lần `int 0x80` **đầu tiên** — đã thật sự vào từ CPL3 (`cs = 0x1B`). In **một** lần,
    // không in lại khi `int 0x80` lặp.
    if count == 1 {
        check(&mut console, 12, "umode: ring3 entered", user);
        let _ = writeln!(
            console,
            "# umode: int80 nr={nr} cs=0x{cs:x} rip=0x{rip:x} user={}",
            user as u8,
        );
    }

    if nr == vifast_abi::SYS_DEBUG_PUT_STR {
        let printed = print_user(a0, a1);
        let ret = SyscallRet::ok(a1);
        if count == 1 {
            check(&mut console, 13, "umode: syscall handled", printed == a1);
            let _ = writeln!(console, "# umode: ring3 ran nr={nr} ret={}", ret.value);
            let _ = writeln!(
                console,
                "# umode: int80 ret error={} value={} printed={printed} ptr=0x{a0:x} len={a1}",
                ret.error, ret.value,
            );
        }
        return ret;
    }

    if nr == vifast_abi::SYS_EXIT {
        // TAP 14: ring 3 **đã chạy tiếp** sau `int 0x80` đầu và gọi `Exit` — bằng chứng
        // "returned to ring3".
        check(&mut console, 14, "umode: returned to ring3", user);
        let _ = writeln!(console, "# umode: exit code={a0} nr={nr}");
        let _ = writeln!(console, "# umode: done");
        // **KT-274** (D2-208): chạy tiếp phần **PCI + virtio-net + ARP** (TAP 15..17) rồi in
        // marker + tắt máy. Chưa có procsrv/scheduler nên đây là chỗ duy nhất kernel còn chạy
        // được sau `Exit`; thứ tự TAP 1..14 của các bước trước **không đổi**.
        crate::net::run(&mut console, 15)
    }

    // Số hiệu lạ (chưa cài): in **một** lần rồi trả `NotSupported` (18) — không treo máy vì một
    // syscall lạ, và TAP 12/13 đã `not ok` nếu `DebugPutStr` không tới (đo được, không im lặng).
    if count <= 2 {
        let _ = writeln!(
            console,
            "# umode: int80 nr={nr} — không phải DebugPutStr ({})/Exit ({})",
            vifast_abi::SYS_DEBUG_PUT_STR,
            vifast_abi::SYS_EXIT,
        );
    }
    SyscallRet::err(18)
}
