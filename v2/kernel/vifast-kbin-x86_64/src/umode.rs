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

use vifast_core::syscall::SyscallOutcome;

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::bootmem::BootArena;
use vifast_plat_x86_64::core_bridge;
use vifast_plat_x86_64::format::Perms;
use vifast_plat_x86_64::mmu::TableMemory;
use vifast_plat_x86_64::syscall_env::{self, X86_64Env};
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::umode::{
    is_user_cs, SyscallRet, SYSCALL_FRAME_R10, SYSCALL_FRAME_R12, SYSCALL_FRAME_R13,
    SYSCALL_FRAME_R8, SYSCALL_FRAME_R9, USER_CS, USER_RFLAGS, USER_SS, USER_STACK_TOP,
    USER_STACK_VA, VSPACE_CODE_VA, VSPACE_PEER_VA, VSPACE_PROBE_VA, VSPACE_STACK_TOP,
    VSPACE_STACK_VA,
};
use vifast_plat_x86_64::vspace::VSpace;

use crate::{check, Console};

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

/// Số lần `int 0x80` đã vào handler — in bằng chứng **một lần** cho mỗi ca.
///
/// **KT-340c** — trần `MAX_USER_PRINT`/bộ đếm `USER_BYTES_PRINTED` đã **xoá**: việc đọc bộ nhớ
/// user chuyển xuống `vifast_plat_x86_64::syscall_env::X86_64Env::put_str` (D2-83: core không đọc
/// bộ nhớ user), và ngân sách `DebugPutStr` do **core** kiểm (`MAX_DEBUG_PUT`). Không còn đường
/// in thứ hai — nếu còn, hai đường có thể lệch nhau mà không ai thấy.
static SYSCALL_COUNT: AtomicU64 = AtomicU64::new(0);

extern "C" {
    /// Đầu section `.user` (PA trong ảnh; identity map ⇒ VA == PA).
    static __user_start: u8;
    /// Cuối section `.user`.
    static __user_end: u8;
    /// Điểm vào ring 3 (offset trong section).
    fn __user_entry();
    /// **KT-304**: điểm vào ring 3 của tác vụ scheduler (cùng section `.user`).
    fn __user_sched_entry();
    /// **KT-340b**: nhãn **ngay sau** lệnh ghi vào `VSPACE_PEER_VA` — handler resume về đây sau
    /// khi bắt đúng `#PF` cô lập (VA = `VSPACE_CODE_VA + (nhãn - __user_start)`).
    fn __user_after_peer();
    /// **KT-418c**: nhãn **đầu** vòng quay CPL3 — chỉ tick rơi **trong** `[spin, resumed)` mới
    /// được tính/đổi `RIP`; tick rơi chỗ khác trong ring 3 resume nguyên (không phá ca `#PF`).
    fn __user_cpl3_spin();
    /// **KT-418c**: nhãn **ngay sau** vòng quay CPL3 — handler timer `iretq` về đây khi tick đầu
    /// tiên cắt ngang ring 3 (cùng công thức VA với `__user_after_peer`).
    fn __user_cpl3_resumed();
    /// Trang stack user (PA trong `.bss`; map ở [`USER_STACK_VA`] với `US = 1`).
    static __user_stack: u8;
    /// Đỉnh stack kernel riêng cho đường vào từ ring 3 (`TSS.RSP0`).
    static __syscall_stack_top: u8;
}

// ---------------------------------------------------------------------------------------------
// **KT-340b** — tác vụ ring 3 chạy trong `VSpace` **động** (bảng trang riêng)
// ---------------------------------------------------------------------------------------------

/// `CR3` của ảnh trước khi hạ xuống ring 3 — handler `Exit` trả về cây gốc trước khi chạy tiếp
/// PCI/net (TAP 15..) để các pha sau **không** phụ thuộc VSpace của tác vụ.
static VSPACE_BOOT_ROOT: AtomicU64 = AtomicU64::new(0);

/// PA frame "vùng của tác vụ khác" — sau fault phải **nguyên vẹn** (bằng chứng cô lập).
static VSPACE_PEER_PA: AtomicU64 = AtomicU64::new(0);

/// Seed ghi vào frame peer trước khi chạy: ghi từ ring 3 vào vùng này **phải** fault, nên đọc lại
/// sau vẫn đúng seed. `0` không dùng (0 là "chưa cấp").
const PEER_SEED: u64 = 0x5A5A_0000_0000_5A5A;
/// Dữ liệu hai frame của phép đo `INVLPG` (khác nhau để phân biệt mapping cũ/mới).
const PROBE_A_WORD: u64 = 0x1111_2222_3333_4444;
const PROBE_B_WORD: u64 = 0x5555_6666_7777_8888;

/// Cấp **một frame dữ liệu** từ arena bảng trang (identity map nên kernel đọc/ghi được bằng PA).
fn alloc_frame(console: &mut Console, arena: &mut BootArena, what: &str) -> u64 {
    match arena.alloc_frame() {
        Some(pa) => pa,
        None => {
            let _ = writeln!(
                console,
                "PANIC: vspace: arena bảng trang hết frame khi cấp '{what}' (đã dùng {} / {} frame)",
                arena.used(),
                arena.frames(),
            );
            asm::halt(3)
        }
    }
}

/// Map một trang vào `vs`; lỗi ⇒ `PANIC` (fail-loud, không bỏ qua mapping).
fn map_or_panic(
    console: &mut Console,
    vs: &mut VSpace<BootArena>,
    va: u64,
    pa: u64,
    perms: Perms,
    what: &str,
) {
    if let Err(err) = vs.map(va, pa, perms) {
        let _ = writeln!(
            console,
            "PANIC: vspace: map {what} va=0x{va:x} pa=0x{pa:x} lỗi {err:?}"
        );
        asm::halt(3)
    }
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

    // -----------------------------------------------------------------------------------------
    // **KT-340b** — `VSpace` **động**: tác vụ ring 3 chạy trong bảng trang **riêng** của nó
    // (PML4/PDPT/cây user cấp từ arena ngoài `.bss`), kernel/MMIO **chia sẻ** từ ảnh. Tác vụ
    // **thứ hai** (`VSpace` B) map `VSPACE_PEER_VA` → frame peer; tác vụ A **không** map VA ấy ⇒
    // ghi từ ring 3 xuống đó là `#PF` thật.
    // -----------------------------------------------------------------------------------------
    let arena = match BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) {
        Some(arena) => arena,
        None => {
            let _ = writeln!(console, "PANIC: vspace: arena bảng trang không hợp lệ");
            asm::halt(3)
        }
    };
    let mut data = arena;
    let peer_pa = alloc_frame(console, &mut data, "peer");
    let probe_a_pa = alloc_frame(console, &mut data, "probe-a");
    let probe_b_pa = alloc_frame(console, &mut data, "probe-b");
    asm::write_u64(peer_pa, PEER_SEED);
    asm::write_u64(probe_a_pa, PROBE_A_WORD);
    asm::write_u64(probe_b_pa, PROBE_B_WORD);
    VSPACE_PEER_PA.store(peer_pa, Ordering::Relaxed);

    let mut a = match VSpace::create(1, arena) {
        Ok(vspace) => vspace,
        Err(err) => {
            let _ = writeln!(console, "PANIC: vspace: tạo VSpace A lỗi {err:?}");
            asm::halt(3)
        }
    };
    let mut b = match VSpace::create(2, arena) {
        Ok(vspace) => vspace,
        Err(err) => {
            let _ = writeln!(console, "PANIC: vspace: tạo VSpace B lỗi {err:?}");
            asm::halt(3)
        }
    };
    // A: mã (RX) + stack (RW NX) + trang probe. **Không** map `VSPACE_PEER_VA` — đó là vùng của
    // tác vụ khác.
    map_or_panic(
        console,
        &mut a,
        VSPACE_CODE_VA,
        code_start,
        Perms::USER_RX,
        "A/code",
    );
    map_or_panic(
        console,
        &mut a,
        VSPACE_STACK_VA,
        stack_pa,
        Perms::USER_RW,
        "A/stack",
    );
    map_or_panic(
        console,
        &mut a,
        VSPACE_PROBE_VA,
        probe_a_pa,
        Perms::USER_RW,
        "A/probe",
    );
    // B: cùng VA trang mã (cùng PA) + **vùng riêng** ở `VSPACE_PEER_VA` → frame peer.
    map_or_panic(
        console,
        &mut b,
        VSPACE_CODE_VA,
        code_start,
        Perms::USER_RX,
        "B/code",
    );
    map_or_panic(
        console,
        &mut b,
        VSPACE_PEER_VA,
        peer_pa,
        Perms::USER_RW,
        "B/peer",
    );

    // Bằng chứng **đọc từ bảng trang** (không phải hằng mong đợi).
    const PTE_P_BIT: u64 = 1 << 0;
    const PTE_RW_BIT: u64 = 1 << 1;
    const PTE_US_BIT: u64 = 1 << 2;
    const PTE_NX_BIT: u64 = 1u64 << 63;
    const PTE_ADDR_BITS: u64 = 0x000F_FFFF_FFFF_F000;
    let a_code = a.leaf(VSPACE_CODE_VA).unwrap_or(0);
    let a_stack = a.leaf(VSPACE_STACK_VA).unwrap_or(0);
    let b_peer = b.leaf(VSPACE_PEER_VA).unwrap_or(0);
    let a_peer = a.leaf(VSPACE_PEER_VA);
    let b_peer_pa = b.translate(VSPACE_PEER_VA).map(|t| t.pa).unwrap_or(0);
    let code_ok = a_code & PTE_P_BIT != 0
        && a_code & PTE_RW_BIT == 0
        && a_code & PTE_US_BIT != 0
        && a_code & PTE_NX_BIT == 0;
    let stack_ok = a_stack & PTE_P_BIT != 0
        && a_stack & PTE_RW_BIT != 0
        && a_stack & PTE_US_BIT != 0
        && a_stack & PTE_NX_BIT != 0;
    let b_ok = b_peer & PTE_P_BIT != 0
        && b_peer & PTE_US_BIT != 0
        && b_peer & PTE_ADDR_BITS == peer_pa
        && b_peer_pa == peer_pa;
    let iso_ok = a_peer.is_none();
    let _ = writeln!(
        console,
        "# vspace: a=1 b=2 a_root=0x{:x} b_root=0x{:x} boot_root=0x{:x} frames={}/{}",
        a.root(),
        b.root(),
        a.boot_root(),
        arena.used(),
        arena.frames(),
    );
    let _ = writeln!(
        console,
        "# vspace: code_va=0x{VSPACE_CODE_VA:x} stack_va=0x{VSPACE_STACK_VA:x} peer_va=0x{VSPACE_PEER_VA:x} probe_va=0x{VSPACE_PROBE_VA:x} code_pa=0x{code_start:x} stack_pa=0x{stack_pa:x} peer_pa=0x{peer_pa:x}"
    );
    let _ = writeln!(
        console,
        "# vspace: leaves a_code=0x{a_code:016x} a_stack=0x{a_stack:016x} b_peer=0x{b_peer:016x} a_peer=absent:{} code_ok={} stack_ok={} b_ok={} iso_ok={}",
        a_peer.is_none() as u8,
        code_ok as u8,
        stack_ok as u8,
        b_ok as u8,
        iso_ok as u8,
    );
    if !(code_ok && stack_ok && b_ok && iso_ok) {
        let _ = writeln!(
            console,
            "PANIC: vspace: quyền/cô lập sai (code_ok={code_ok} stack_ok={stack_ok} b_ok={b_ok} iso_ok={iso_ok})"
        );
        asm::halt(3)
    }

    // **Đột biến (iii)** — "bảng trang dùng chung" (như trước KT-340b): A map **luôn**
    // `VSPACE_PEER_VA` → frame của tác vụ B, **sau** phép kiểm `iso_ok` ở trên (để phép kiểm
    // đứng trước vẫn xanh) ⇒ lệnh ghi từ ring 3 **không** fault ⇒ `vspace_ok = 0` ⇒ TAP 14 ĐỎ.
    // Chỉ tồn tại khi bật feature đột biến.
    #[cfg(feature = "mut-shared-peer")]
    map_or_panic(
        console,
        &mut a,
        VSPACE_PEER_VA,
        peer_pa,
        Perms::USER_RW,
        "mut/peer",
    );

    // ---- **INVLPG**: đổi mapping của **cùng** một VA rồi đọc lại. `before` nạp entry TLB của
    // frame A; sau `unmap` + `map` sang frame B, `after` **phải** là dữ liệu của B. Thiếu
    // `INVLPG` (đột biến `no-invlpg`) ⇒ `after` = dữ liệu cũ của A ⇒ PANIC (ĐỎ).
    a.activate();
    VSPACE_BOOT_ROOT.store(a.boot_root(), Ordering::Relaxed);
    // **KT-418b** — `VSPACE_PROBE_VA` là trang `US=1` trong bảng trang đang hoạt động; với
    // `CR4.SMAP` bật, CPL0 chỉ đọc được khi `RFLAGS.AC = 1` ⇒ bọc bằng guard (tự `clac`).
    let before = {
        let _user = asm::smap::UserAccess::begin();
        asm::read_u64(VSPACE_PROBE_VA)
    };
    if let Err(err) = a.unmap(VSPACE_PROBE_VA) {
        let _ = writeln!(console, "PANIC: vspace: unmap probe lỗi {err:?}");
        asm::halt(3)
    }
    map_or_panic(
        console,
        &mut a,
        VSPACE_PROBE_VA,
        probe_b_pa,
        Perms::USER_RW,
        "A/probe-b",
    );
    let after = {
        let _user = asm::smap::UserAccess::begin();
        asm::read_u64(VSPACE_PROBE_VA)
    };
    let probe_ok = before == PROBE_A_WORD && after == PROBE_B_WORD;
    crate::vspace_set_probe_ok(probe_ok);
    let _ = writeln!(
        console,
        "# vspace: remap va=0x{:x} before=0x{before:016x} after=0x{after:016x} expect=(0x{PROBE_A_WORD:016x},0x{PROBE_B_WORD:016x}) invlpg_ok={}",
        VSPACE_PROBE_VA,
        probe_ok as u8,
    );
    if !probe_ok {
        let _ = writeln!(
            console,
            "PANIC: vspace: INVLPG không hiệu lực — cùng VA vẫn đọc ra dữ liệu cũ (before=0x{before:016x} after=0x{after:016x})"
        );
        asm::halt(3)
    }

    let entry = VSPACE_CODE_VA + (entry_pa - code_start);
    let resume = VSPACE_CODE_VA + (__user_after_peer as *const () as u64 - code_start);
    let cpl3_spin = VSPACE_CODE_VA + (__user_cpl3_spin as *const () as u64 - code_start);
    let cpl3_resume = VSPACE_CODE_VA + (__user_cpl3_resumed as *const () as u64 - code_start);
    let _ = writeln!(
        console,
        "# umode: iretq entry=0x{entry:x} sp=0x{VSPACE_STACK_TOP:x} rflags=0x{USER_RFLAGS:x} cs=0x{USER_CS:x} ss=0x{USER_SS:x} code_va=0x{VSPACE_CODE_VA:x} code_pa=0x{code_start:x} code_bytes={code_bytes} stack_va=0x{VSPACE_STACK_VA:x} stack_pa=0x{stack_pa:x} syscall_stack=0x{syscall_stack_top:x}"
    );
    let _ = USER_STACK_TOP; // đường **tĩnh** (KT-270/scheduler) không đổi — xem TAP 22..25.
    let _ = USER_STACK_VA;
    // Stack kernel **riêng** cho đường vào từ CPL3: `kmain` không quay lại sau `iretq`, nhưng
    // dùng stack boot làm `RSP0` sẽ để handler đè lên frame còn sống của `kmain`.
    //
    // **KT-278**: TSS là per-hart — ring 3 chỉ chạy trên **hart 0** (BSP), nên chỉ đổi `RSP0` của
    // slot 0. Ba hart phụ không bao giờ xuống CPL3 ở bước này.
    tss::set_rsp0(0, syscall_stack_top);
    // **KT-418** — cứng hóa `SMEP`/`SMAP` + **ca âm phần cứng** (TAP 50) trên trang mã ring 3
    // vừa map `US=1` trong `VSpace` **đang hoạt động** (đúng bảng trang mà ring 3 sẽ chạy).
    crate::sec_probe::run(console, 50, VSPACE_CODE_VA);
    // ---- **KT-418c** — arm ca cô lập **SAU** `sec_probe::run` (lỗi `-cpu max` của KT-418b).
    //
    // `sec_probe::run` gọi `exc_arm(VEC_PF, …)` cho **hai** ca âm `SMEP`/`SMAP`, mà `exc_arm` **xoá
    // `EXPECT_CR2`** (đúng thiết kế KT-418: nhánh `CR2` chỉ dành cho ca cô lập `VSpace`). Arming ca
    // cô lập **trước** các phép đo đó ⇒ trên CPU **có** `SMEP`/`SMAP` (`-cpu max`) `EXPECT_CR2 = 0`
    // khi `#PF` thật của ring 3 tới ⇒ handler rơi vào nhánh `# exc:` chung rồi `halt(2)`: boot chết
    // ở TAP 12. Trên `qemu64` (không SMEP/SMAP) hai phép đo bị bỏ qua nên lỗi **không lộ**.
    crate::exc_arm_user_pf(VSPACE_PEER_VA, resume);
    if !crate::user_pf_armed(VSPACE_PEER_VA) {
        let _ = writeln!(
            console,
            "# vspace: BAD arm cr2=0x{VSPACE_PEER_VA:x} (EXPECT_CR2/EXPECT_VEC không đúng ngay trước iretq — KT-418c)"
        );
        asm::halt(3)
    }
    // **KT-418c** — arm phủ preempt CPL3: vòng quay ring 3 chờ tick PIT, handler `iretq` về nhãn
    // `cpl3_resume` rồi `int 0x80` kế tiếp in `# sched: cpl3 preempted=… resumed=1 cs=0x…`.
    crate::cpl3_arm(cpl3_spin, cpl3_resume);
    // **KT-418** — **một cửa** của ngữ cảnh FPU/SIMD: bật `CR0.EM=0`/`CR0.MP=1` +
    // `CR4.OSFXSR`/`OSXMMEXCPT`, gắn TCB đang chạy rồi khôi phục `xmm0..xmm15`/`st0..st7` +
    // `mxcsr` của **đúng** TCB đó trước `iretq` (khe chưa hợp lệ ⇒ trạng thái mới).
    let _ = vifast_plat_x86_64::asm::fpu::enable_fpu();
    if let Some(tcb) = crate::coretest::current_tcb() {
        vifast_plat_x86_64::asm::fpu::bind_current(tcb);
    }
    vifast_plat_x86_64::asm::fpu::restore_current();
    asm::enter_user(entry, VSPACE_STACK_TOP, USER_RFLAGS)
}

/// Handler **`int 0x80`** — stub asm (`vifast_isr_syscall`) gọi hàm này.
///
/// # Tham số (SysV, **11** tham số — KT-340c)
///
/// `nr` = `rax`, `a0`/`a1` = `rdi`/`rsi`, `mi` = `rdx`, `mr0..mr3` = `r10`/`r8`/`r9`/`r12`,
/// `cs`/`rip` = frame CPU (chứng minh **đã vào từ ring 3**), `frame` = con trỏ frame trong stub
/// (handler ghi `MessageInfo` trả về vào **`r13`** + MR vào `r10/r8/r9/r12` — `7C-r13`, D2-242).
/// Trả [`SyscallRet`] (`rax` = error, `rdx` = value) — stub ghi hai giá trị vào frame trước
/// `iretq`.
///
/// # Một cửa duy nhất (KT-340c, D2-79)
///
/// **Mọi** số hiệu `0..15` đi qua `vifast_core::syscall::dispatch` trên `Machine` thật, dưới
/// `core_bridge::with_machine` (khoá `IrqMcsLock` tắt `RFLAGS.IF`). Không còn bảng syscall riêng
/// của ảnh: `DebugPutStr`/`Exit` là syscall của **core** như mọi syscall khác — đó là phần
/// "gỡ state machine trùng lặp" của `KT-340c` (xem journal để biết phần scheduler còn lại + task
/// con đề xuất).
///
/// Phân phối theo **số hiệu** (không theo bộ đếm) để một `int 0x80` bị lặp vẫn là `DebugPutStr`,
/// không bị nhầm thành `Exit`.
#[no_mangle]
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
    // **KT-341** — pha shell (sau TAP 38..43) có dispatcher riêng nhưng **vẫn** một
    // `vifast_core::syscall::dispatch`/`Machine` (D2-79): định tuyến theo pha, không bảng thứ hai.
    if crate::shellpath::active() {
        return crate::shellpath::dispatch(nr, a0, a1, mi, mr0, mr1, mr2, mr3, cs, rip, frame);
    }
    let mut console = Console;
    let count = SYSCALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let user = is_user_cs(cs);
    // ---- **KT-418c** — khoá phủ preempt ring-3: `int 0x80` **từ CPL3** sau khi tick đã cắt ngang
    // CPL3 chính là bằng chứng đã **resume** đúng nhãn ring-3 ⇒ in **một** dòng
    // `# sched: cpl3 preempted=<n> resumed=1 cs=0x<cs>` (`cs` đo từ frame, không hardcode).
    if user && crate::cpl3_preempt_pending() {
        let _ = crate::cpl3_report(&mut console);
    }

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

    let Some(tcb) = crate::coretest::current_tcb() else {
        let _ = writeln!(
            console,
            "PANIC: umode: không có TCB core cho int80 nr={nr} (coretest::prepare chưa chạy)"
        );
        asm::halt(1)
    };
    // **`[calling.x86_64]`** — ba syscall **gửi** có `args[1]` = `MessageInfo` (`rdx`) và
    // `args[2..6]` = MR0..MR3 (`r10`, `r8`, `r9`, `r12`); các syscall còn lại giữ
    // `args[0..5]` = `rdi, rsi, rdx, r10, r8, r9` (Recv: `args[1]` = cap Reply).
    let args = match nr {
        vifast_abi::SYS_CALL | vifast_abi::SYS_SEND | vifast_abi::SYS_NB_SEND => {
            [a0, mi, mr0, mr1, mr2, mr3]
        }
        _ => [a0, a1, mi, mr0, mr1, mr2],
    };
    // **Đường duy nhất**: `dispatch` của core trên `Machine` thật, dưới khoá bọc tắt ngắt.
    let outcome = core_bridge::with_machine(|m| {
        vifast_core::syscall::dispatch(m, tcb, 0, nr, args, &mut X86_64Env)
    });

    match outcome {
        SyscallOutcome::Return(result) => {
            let in_core = crate::coretest::in_core_phase();
            if count == 1 && nr == vifast_abi::SYS_DEBUG_PUT_STR {
                check(
                    &mut console,
                    13,
                    "umode: syscall handled",
                    result.error == 0 && result.value == a1,
                );
                let _ = writeln!(console, "# umode: ring3 ran nr={nr} ret={}", result.value);
                let _ = writeln!(
                    console,
                    "# umode: int80 ret error={} value={} ptr=0x{a0:x} len={a1}",
                    result.error, result.value,
                );
            }
            if in_core {
                crate::coretest::record_syscall(result.error, result.value);
            }
            // **`7C-r13`** (D2-242): kênh trả của `MessageInfo` là `r13`; MR0..MR3 là
            // `r10/r8/r9/r12`. Đột biến `mut-info-r12` **đảo** hai chỗ này ⇒ chương trình ring 3
            // đọc `r13` không thấy info ⇒ TAP 41 ĐỎ.
            #[cfg(feature = "mut-info-r12")]
            {
                asm::write_u64(frame + SYSCALL_FRAME_R12 as u64, result.info);
                asm::write_u64(frame + SYSCALL_FRAME_R13 as u64, result.mr[3]);
            }
            #[cfg(not(feature = "mut-info-r12"))]
            {
                asm::write_u64(frame + SYSCALL_FRAME_R13 as u64, result.info);
                asm::write_u64(frame + SYSCALL_FRAME_R12 as u64, result.mr[3]);
            }
            asm::write_u64(frame + SYSCALL_FRAME_R10 as u64, result.mr[0]);
            asm::write_u64(frame + SYSCALL_FRAME_R8 as u64, result.mr[1]);
            asm::write_u64(frame + SYSCALL_FRAME_R9 as u64, result.mr[2]);
            SyscallRet {
                error: result.error,
                value: result.value,
            }
        }
        SyscallOutcome::Exit { code } => {
            // **KT-418** — tác vụ dừng: lưu `xmm0..xmm15`/`st0..st7` + `mxcsr` vào khe của TCB
            // (eager — x86_64 không có bit bẩn phần cứng tương đương `sstatus.FS`).
            vifast_plat_x86_64::asm::fpu::save_current();
            // **KT-340b** — tác vụ chạy trong `VSpace` **động**: trả `CR3` về cây gốc của ảnh
            // **trước** khi chạy tiếp PCI/net (TAP 15..), để các pha sau không phụ thuộc bảng
            // trang của tác vụ; rồi kiểm bằng chứng cô lập (fault thật + peer nguyên vẹn +
            // `INVLPG`).
            let boot_root = VSPACE_BOOT_ROOT.load(Ordering::Relaxed);
            if boot_root != 0 {
                asm::write_cr3(boot_root);
            }
            if crate::coretest::in_core_phase() {
                // Pha core: ghi mã thoát (kernel đối chiếu), đo toàn bộ bằng chứng + TAP 38..43,
                // rồi **chạy tiếp** chuỗi boot (TAP 15.. như trước KT-340c).
                crate::coretest::record_exit(code);
                let _ = writeln!(console, "# core: exit code=0x{code:x}");
                crate::coretest::finish(&mut console);
                // **KT-341** — shell sản phẩm dùng chung chạy **sau** TAP 38..43 trên cùng
                // `Machine`; pha này phát TAP 44/45 rồi trả chuỗi boot về `net::run`.
                crate::shellpath::start_ktest(&mut console)
            }
            let peer_pa = VSPACE_PEER_PA.load(Ordering::Relaxed);
            let peer_unchanged = peer_pa != 0 && asm::read_u64(peer_pa) == PEER_SEED;
            let vspace_ok = crate::vspace_ok() && peer_unchanged;
            let _ = writeln!(
                console,
                "# vspace: iso ok={} fault={} peer_unchanged={} peer_pa=0x{peer_pa:x} seed=0x{PEER_SEED:016x} boot_root=0x{boot_root:x}",
                vspace_ok as u8,
                crate::vspace_fault_seen() as u8,
                peer_unchanged as u8,
            );
            // TAP 14: ring 3 **đã chạy tiếp** sau `int 0x80` đầu và gọi `Exit` — bằng chứng
            // "returned to ring3". **KT-340b** siết thêm: tác vụ phải chạy trong `VSpace` **riêng**
            // và lệnh ghi vào vùng của tác vụ khác phải **fault** (nếu không, bảng trang dùng
            // chung ⇒ `vspace_ok = 0` ⇒ TAP 14 ĐỎ).
            check(
                &mut console,
                14,
                "umode: returned to ring3",
                user && vspace_ok,
            );
            // **KT-418c** — đã arm phủ preempt CPL3 mà **không** tick nào cắt ngang (PIT chết,
            // `IF = 0`, hoặc vòng quay kết thúc trước tick) ⇒ in lỗi **có tên** để ca kiểm trượt
            // vì thiếu bằng chứng (không im lặng, không xanh oan).
            if crate::cpl3_armed_but_no_tick() {
                let _ = writeln!(
                    console,
                    "# sched: cpl3 preempted=0 resumed=0 cs=0x0 (BAD: không tick nào cắt ngang CPL3 — KT-418c)"
                );
            }
            let _ = writeln!(console, "# umode: exit code={a0} nr={nr}");
            let _ = writeln!(console, "# umode: done");
            // **KT-340c** — chạy **pha core** (chương trình ring 3 thứ hai): mọi `int 0x80` của
            // pha ấy đi qua `vifast_core::syscall::dispatch`; `Exit` của pha ấy quay lại đây với
            // `in_core_phase = true` (nhánh trên) rồi mới sang PCI/net.
            crate::coretest::start_core_phase(&mut console, boot_root)
        }
        SyscallOutcome::Halt { code } => {
            let _ = writeln!(console, "# umode: halt code={code} nr={nr}");
            asm::halt(code as u32)
        }
        SyscallOutcome::Switched { state } => {
            if !crate::coretest::in_core_phase() {
                let _ = writeln!(
                    console,
                    "PANIC: umode: Switched ngoài pha core (nr={nr} state=0x{:x})",
                    crate::coretest::state_code(state)
                );
                asm::halt(2)
            }
            // Điểm an toàn `Tcb.Suspend(Defer)` (KT-329): kernel `Resume` rồi quay lại ring 3 tại
            // `TcbData.entry` do core giao. Ghi `RIP` của **frame** (stub không đụng ô này) ⇒
            // `iretq` nhảy đúng điểm vào; `RSP` user giữ nguyên frame đã đẩy của `int 0x80`.
            let Some(entry) = crate::coretest::resume_after_switch(&mut console, state) else {
                asm::halt(4)
            };
            asm::write_u64(
                frame + vifast_plat_x86_64::umode::SYSCALL_FRAME_RIP as u64,
                entry,
            );
            SyscallRet::ok(0)
        }
    }
}
