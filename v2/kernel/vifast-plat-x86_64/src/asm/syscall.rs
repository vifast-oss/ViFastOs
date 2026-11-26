// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `asm/syscall.rs` — **stub syscall `int 0x80`** (KT-270/KT-340c): giữ 15 GPR, chuyển đủ
//! 4 MR + con trỏ frame cho `syscall_dispatch` (`7C-r13`, D2-242), trả `rax:rdx` rồi
//! `iretq`; tách cơ học từ `asm.rs` (KT-401). `vifast_isr_common` ở `asm/trap.rs`.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
core::arch::global_asm!(
    ".section .text",
    // ---- KT-270: stub `int 0x80` (vector 0x80) — đường syscall từ ring 3.
    //
    // `int 0x80` từ CPL3 đẩy `SS/RSP/RFLAGS/CS/RIP` (5 qword); stub đẩy `err = 0`,
    // `vector = 0x80`, rồi **15 GPR** — cùng bố cục frame với exception chung. Handler
    // `syscall_dispatch` nhận **11** tham số SysV (`nr, a0, a1, mi, mr0, mr1, mr2, mr3, cs, rip,
    // frame`; 5 tham số cuối trên stack) và trả `SyscallRet` (`rax:rdx`); stub ghi hai giá trị
    // vào ô `rax`/`rdx` của frame rồi `iretq`.
    //
    // **KT-340c** — đường **duy nhất** của syscall: handler gọi
    // `vifast_core::syscall::dispatch`, nên stub phải chuyển **đủ** MR (`r10`, `r8`, `r9`, `r12`
    // — `[calling.x86_64]` v1.1) và trao **con trỏ frame** cho handler. Handler ghi
    // `MessageInfo` trả về vào ô `r13` (**`7C-r13`**, D2-242) và MR0..MR3 trả về vào
    // `r10/r8/r9/r12` — nhờ vậy ring 3 đọc đúng kênh trả của spec mà stub **không** phải biết
    // syscall nào trả gì (cùng nguyên tắc "một cửa" của D2-79).
    ".p2align 4",
    ".globl vifast_isr_syscall",
    "vifast_isr_syscall:",
    "push 0",
    "push {syscall_vec}",
    "push rax",
    "push rcx",
    "push rdx",
    "push rbx",
    "push rbp",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    // `rbx` = con trỏ frame; mọi tham số đọc **từ bộ nhớ** nên việc ghi đè thanh ghi không ảnh
    // hưởng thứ tự đọc.
    "mov rbx, rsp",
    "mov rdi, [rbx + {sc_rax}]", // nr
    "mov rsi, [rbx + {sc_rdi}]", // a0
    "mov rdx, [rbx + {sc_rsi}]", // a1
    "mov rcx, [rbx + {sc_rdx}]", // mi
    "mov r8,  [rbx + {sc_r10}]", // mr0
    "mov r9,  [rbx + {sc_r8}]",  // mr1
    // Căn stack 16 byte **trước** khi đẩy 5 tham số stack: `and -16` rồi `sub 8` để tổng
    // (pad 8 + 5×8 = 48) vẫn là bội của 16 ⇒ sau `call` callee thấy `rsp % 16 == 8` (SysV).
    "and rsp, -16",
    "sub rsp, 8",
    "push rbx",                     // arg11: con trỏ frame
    "push qword ptr [rbx + {sc_rip}]", // arg10: rip
    "push qword ptr [rbx + {sc_cs}]",  // arg9:  cs
    "push qword ptr [rbx + {sc_r12}]", // arg8:  mr3
    "push qword ptr [rbx + {sc_r9}]",  // arg7:  mr2
    "call syscall_dispatch",
    "add rsp, 48",
    "mov rsp, rbx",
    // Trả kết quả theo `[calling.x86_64]`: `rax` = error, `rdx` = value. `r13`/`r10`/`r8`/`r9`/
    // `r12` do **handler** ghi thẳng vào frame (kênh `MessageInfo` + MR trả về).
    "mov [rsp + {sc_rax}], rax",
    "mov [rsp + {sc_rdx}], rdx",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "pop rdx",
    "pop rcx",
    "pop rax",
    "add rsp, 16",
    "iretq",
    syscall_vec = const crate::exc::VEC_SYSCALL as u32,
    sc_rax = const crate::umode::SYSCALL_FRAME_RAX,
    sc_rdx = const crate::umode::SYSCALL_FRAME_RDX,
    sc_rsi = const crate::umode::SYSCALL_FRAME_RSI,
    sc_rdi = const crate::umode::SYSCALL_FRAME_RDI,
    sc_r8 = const crate::umode::SYSCALL_FRAME_R8,
    sc_r9 = const crate::umode::SYSCALL_FRAME_R9,
    sc_r10 = const crate::umode::SYSCALL_FRAME_R10,
    sc_r12 = const crate::umode::SYSCALL_FRAME_R12,
    sc_rip = const crate::umode::SYSCALL_FRAME_RIP,
    sc_cs = const crate::umode::SYSCALL_FRAME_CS,
);

// Handler **syscall** của ảnh — định nghĩa ở `vifast-kbin-x86_64` (`#[no_mangle]`).
//
// Stub `vifast_isr_syscall` gọi hàm này với **11** tham số SysV (6 thanh ghi + 5 trên stack) và
// nhận `SyscallRet` ở `rax:rdx`. Khai báo ở đây (crate plat) để stub asm gọi được mà ảnh **không**
// cần `unsafe` (R5). **KT-340c** thêm `mr2`/`mr3` (đủ 4 MR của `[calling.x86_64]`) và `frame` để
// handler ghi `MessageInfo` trả về vào `r13` + MR trả về vào `r10/r8/r9/r12` (D2-242 `7C-r13`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
#[allow(dead_code)] // chỉ được gọi từ stub asm (`vifast_isr_syscall`), không từ Rust.
extern "C" {
    fn syscall_dispatch(
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
    ) -> crate::umode::SyscallRet;
}

// =============================================================================================
// **KT-417c** — stub `syscall`/`sysret` (đường thứ **hai**, song song `int 0x80`)
// =============================================================================================
//
// # Khác gì stub `int 0x80` ở trên
//
// `int 0x80` để **CPU** làm ba việc: đẩy `SS/RSP/RFLAGS/CS/RIP`, nạp `CS`/`SS` từ gate IDT, và
// đổi `RSP` sang `TSS.RSP0`. `syscall` **không** làm việc nào: nó chỉ đặt
// `RIP = LSTAR`, `CS = STAR[47:32]`, `R11 = RFLAGS`, `RCX = RIP kế tiếp` — `RSP` và `GS.base`
// vẫn là của ring 3. Vì vậy stub này phải **tự**:
//
// 1. `swapgs` ⇒ `GS.base` = vùng scratch của hart (xem `syscall_fast::GS_SCRATCH`);
// 2. lưu `RSP` user vào `gs:[8]`, nạp `RSP` kernel từ `gs:[0]`;
// 3. dựng frame **đúng bố cục** `SYSCALL_FRAME_*` (err + vector + 15 GPR) để `syscall_dispatch`
//    dùng chung một hợp đồng byte-level với `int 0x80` (`7C-r13`, D2-242) — **không** có hợp
//    đồng thứ hai;
// 4. trước khi ra: đọc lại `RCX` (RIP trả) và `R11` (RFLAGS trả) từ frame, trả `RSP` user, rồi
//    `swapgs` **đúng một lần** và `sysretq`.
//
// # `vector = SYSCALL_FAST_VEC` (0x100)
//
// Ô `vector` của frame là **dấu đường vào**: `int 0x80` đẩy `0x80`, stub này đẩy `0x100` (không
// phải vector hợp lệ). Nhờ vậy handler in được dòng bằng chứng riêng cho `syscall` mà
// `syscall_dispatch` **không** phải thêm tham số thứ 12.
//
// # Vì sao `and rsp, -16` + `sub rsp, 8` rồi 5 `push`
//
// Y hệt stub `int 0x80`: SysV đòi `rsp % 16 == 0` **trước** `call` ⇒ callee thấy
// `rsp % 16 == 8`. Cùng chuỗi lệnh ⇒ cùng kết quả, không phải nghĩ lại.
//
// # `sysretq` chỉ an toàn khi
//
// `RCX` canonical (RIP user) và `R11` giữ `IF = 1`. Stub **khôi phục nguyên** hai giá trị CPU đã
// lưu lúc vào (không tự đặt lại), nên `IF` của ring 3 đúng như trước `syscall`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
core::arch::global_asm!(
    ".section .text",
    ".p2align 4",
    ".globl vifast_isr_syscall_fast",
    "vifast_isr_syscall_fast:",
    // (1) GS: ring 3 (0) → scratch của hart. Đúng **một** `swapgs` ở đây.
    "swapgs",
    // (2) Giữ `RSP` user rồi sang stack kernel của hart (per-hart — `syscall` không có `TSS.RSP0`).
    "mov gs:[{gs_user_rsp}], rsp",
    "mov rsp, gs:[{gs_rsp0}]",
    // (3) Frame cùng bố cục `int 0x80`; `err = 0`, `vector = 0x100` (dấu `syscall`).
    "push 0",
    "push {fast_vec}",
    "push rax",
    "push rcx", // CPU đã đặt RIP trả về vào RCX — giữ trong frame
    "push rdx",
    "push rbx",
    "push rbp",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11", // CPU đã đặt RFLAGS vào R11 — giữ trong frame (sysretq cần lại)
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rbx, rsp",
    // Tham số 1..6 (SysV): nr, a0, a1, mi, mr0, mr1 — đọc **từ bộ nhớ** như stub `int 0x80`.
    "mov rdi, [rbx + {sc_rax}]", // nr
    "mov rsi, [rbx + {sc_rdi}]", // a0
    "mov rdx, [rbx + {sc_rsi}]", // a1
    "mov rcx, [rbx + {sc_rdx}]", // mi
    "mov r8,  [rbx + {sc_r10}]", // mr0
    "mov r9,  [rbx + {sc_r8}]",  // mr1
    // Tham số 7..11 trên stack: mr2, mr3, cs, rip, frame.
    // `cs` = selector mà `SYSRET` **sẽ** nạp (`STAR[63:48] + 16`, RPL 3) — hằng suy từ GDT.
    "and rsp, -16",
    "sub rsp, 8",
    "push rbx",                        // arg11: con trỏ frame
    "push qword ptr [rbx + {sc_rcx}]", // arg10: rip trả về (RCX của CPU)
    "push {user_cs}",                  // arg9:  cs (selector ring 3 của SYSRET)
    "push qword ptr [rbx + {sc_r12}]", // arg8:  mr3
    "push qword ptr [rbx + {sc_r9}]",  // arg7:  mr2
    "call syscall_dispatch",
    "add rsp, 48",
    "mov rsp, rbx",
    // Kết quả SysV: rax = error, rdx = value — ghi vào frame như đường `int 0x80`.
    "mov [rsp + {sc_rax}], rax",
    "mov [rsp + {sc_rdx}], rdx",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "pop rdx",
    "pop rcx",
    "pop rax",
    "add rsp, 16",
    // Frame base = rsp − 136 (15 GPR × 8 + err 8 + vector 8). Đọc **trước** khi trả `RSP`:
    // `sysretq` cần `RCX` = RIP trả về và `R11` = RFLAGS trả về (cặp CPU đã lưu lúc vào).
    "lea rcx, [rsp - 136]",
    "mov r11, [rcx + {sc_r11}]",
    "mov rcx, [rcx + {sc_rcx}]",
    // Trả `RSP` user **trong khi** `GS.base` còn là scratch, rồi `swapgs` **đúng một lần**.
    "mov rsp, gs:[{gs_user_rsp}]",
    "swapgs",
    "sysretq",
    fast_vec = const crate::syscall_fast::SYSCALL_FAST_VEC,
    user_cs = const crate::syscall_fast::sysret_cs() as u32,
    gs_rsp0 = const crate::syscall_fast::GS_OFF_RSP0,
    gs_user_rsp = const crate::syscall_fast::GS_OFF_USER_RSP,
    sc_rax = const crate::umode::SYSCALL_FRAME_RAX,
    sc_rdx = const crate::umode::SYSCALL_FRAME_RDX,
    sc_rsi = const crate::umode::SYSCALL_FRAME_RSI,
    sc_rdi = const crate::umode::SYSCALL_FRAME_RDI,
    sc_r8 = const crate::umode::SYSCALL_FRAME_R8,
    sc_r9 = const crate::umode::SYSCALL_FRAME_R9,
    sc_r10 = const crate::umode::SYSCALL_FRAME_R10,
    sc_r11 = const crate::umode::SYSCALL_FRAME_R11,
    sc_r12 = const crate::umode::SYSCALL_FRAME_R12,
    sc_rcx = const crate::umode::SYSCALL_FRAME_RCX,
);
