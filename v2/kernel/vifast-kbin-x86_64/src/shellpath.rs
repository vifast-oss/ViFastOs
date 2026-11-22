// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-341** — shell **sản phẩm dùng chung** (`vifast-plat-user/programs/shell.rs`) trên x86_64:
//! ELF thật, CPL3, mọi `int 0x80` đi qua **một** `vifast_core::syscall::dispatch` (`7C-r13`).
//!
//! # Vì sao tệp này tồn tại
//!
//! `KT-340c` đã nối `Machine` + IPC ring 3 nhưng chỉ trong ảnh **kiểm** (`ktest`), và `Machine`
//! nằm trong `.bss` (184.792 B) — vượt trần `.data+.bss` ảnh sản phẩm 65.536 B. `KT-341` đưa
//! `Machine` ra section NOLOAD `.corespace` (xem `core_bridge.rs`/`linker.ld`) rồi dựng **cùng**
//! đường shell cho **cả hai** ảnh:
//!
//! - ảnh **sản phẩm** (`--features product-shell`): `product::boot` gọi [`start_product`]; shell
//!   chạy CPL3 từ ELF, đọc ring KT-69 (kernel bơm script bằng `conchan`), in prompt/help/echo,
//!   `Exit(0)` ⇒ kernel in `# core:` + `# shell:` rồi marker + halt. **0 TAP** (D2-08).
//! - ảnh **kiểm** (`--features ktest`): sau TAP 38..43, `umode` gọi [`start_ktest`]; shell chạy
//!   **cùng** mã, kernel đếm byte `DebugPutStr` để phát **TAP 44/45** (prompt + lệnh nội trú),
//!   rồi trả về chuỗi boot cũ (`net::run` TAP 15..).
//!
//! # Vì sao **không** cần scheduler preemptive
//!
//! `shell` là chương trình Rust thật nhưng chỉ một TCB U-mode `Running` tại một thời điểm. Mỗi
//! `Yield` là một `int 0x80` → `dispatch` của core → trả `Return` (không `Switched`) → `iretq` lại
//! đúng chỗ. `Exit` là syscall của core ⇒ kernel tiếp quản (như `coretest::finish`). Cùng mô
//! hình "kernel là scheduler tuần tự" mà KT-339 dùng cho aarch64.
//!
//! # Bằng chứng (kernel đếm, không tin lời U-mode — D2-102)
//!
//! `syscall_env::put_str` đếm **byte thật** in ra COM1 (puts/bytes + bốn mẫu chuỗi). TAP 44 đòi
//! prompt `shell>` + `# shell: exit code=0` + `Exit` trả 0 + TCB `Inactive`; TAP 45 đòi
//! `# shell: help cmds=6` + `# shell: echo same-source`. Nếu `int 0x80` của core bị tắt (đột biến
//! `mut-no-int80`), không byte nào được in ⇒ cả hai TAP ĐỎ.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::{KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::bootmem::BootArena;
use vifast_plat_x86_64::core_bridge;
use vifast_plat_x86_64::format::Perms;
use vifast_plat_x86_64::mmu::TableMemory;
use vifast_plat_x86_64::syscall_env::{self, X86_64Env};
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::umode::{
    is_user_cs, SyscallRet, SYSCALL_FRAME_R10, SYSCALL_FRAME_R12, SYSCALL_FRAME_R13,
    SYSCALL_FRAME_R8, SYSCALL_FRAME_R9, USER_RFLAGS,
};
use vifast_plat_x86_64::userload;
use vifast_plat_x86_64::vspace::VSpace;

use crate::Console;

/// Ô CSpace dùng chung của rig — **khớp** hằng `EP_*` của `shell.rs` (một giao thức, hai phía).
const SLOT_UNTYPED: usize = 0;
const SLOT_EP_PROC: usize = 2;
const SLOT_CSPACE: usize = 3;
const SLOT_EP_REPLY: usize = 4;
const SLOT_EP_PS: usize = 5;
const SLOT_EP_EVENTS: usize = 8;
/// Cap `Frame` của trang IPC buffer (`add_foreign_object`, D2-83).
const SLOT_FRAME: usize = 11;
/// Cap `Tcb` của shell (kernel giữ; U-mode không dùng).
const SLOT_TCB_SHELL: usize = 12;
/// Số ô CSpace của rig (cùng 16 ô với `coretest`).
const SHELL_SLOTS: usize = 16;
/// Số frame Untyped của rig **sản phẩm** (TCB + Endpoint + CNode ≈ vài KiB).
const SHELL_ARENA_FRAMES: usize = 4;

/// VA trang **IPC buffer** — phải bằng backend x86_64 của shim (`Sys::IPC_BUFFER_VA`).
const IPC_BUF_VA: u64 = 0x7000_9000;
/// Đỉnh stack U-mode; hai trang mọc xuống (`0x7000_E000..0x7001_0000`).
const SHELL_STACK_TOP: u64 = 0x7001_0000;
/// Số trang stack (8 KiB).
const SHELL_STACK_PAGES: usize = 2;
/// Script kernel bơm vào ring KT-69 (cùng nội dung aarch64 KT-339): prompt + hai lệnh nội trú +
/// `exit` sạch. `help` in `cmds=6`, `echo` in đúng text — không cần UART RX thật (nợ KT-73).
const SCRIPT: &[u8] = b"help\necho same-source\nexit\n";
/// Ngân sách `Yield` trước khi kernel tiếp quản (chống treo khi đột biến).
const YIELD_BUDGET: u64 = 20_000;
/// `e_machine` x86-64.
const EM_X86_64: u16 = 62;

/// Chương trình U-mode **cùng nguồn** với riscv64/aarch64 (`vifast-plat-user/programs/shell.rs`).
static SHELL_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.elf"));

/// `u32::MAX` = chưa có; `u64::MAX` = chưa đo.
const NO_TCB: u32 = u32::MAX;
const NO_U64: u64 = u64::MAX;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static SHELL_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
static ROOT: AtomicU32 = AtomicU32::new(NO_TCB);
/// `CR3` gốc của ảnh (khôi phục khi `Exit`).
static BOOT_ROOT: AtomicU64 = AtomicU64::new(0);
/// `CR3` của `VSpace` shell (bằng chứng `# shell: vspace`).
static SHELL_ROOT: AtomicU64 = AtomicU64::new(0);
/// PA frame IPC buffer.
static BUF_PA: AtomicU64 = AtomicU64::new(0);
/// PA frame ring KT-69.
static RING_PA: AtomicU64 = AtomicU64::new(0);
/// Mã thoát của shell (`Exit`).
static EXIT_CODE: AtomicU64 = AtomicU64::new(NO_U64);
/// Số `Yield` shell đã gọi.
static YIELDS: AtomicU64 = AtomicU64::new(0);
/// Số `int 0x80` đã vào dispatcher (bằng chứng CPL3 thật).
static SYSCALLS: AtomicU64 = AtomicU64::new(0);
/// `true` ⇔ đã thấy ít nhất một `cs = 0x1B` (thật sự từ CPL3).
static SAW_USER_CS: AtomicBool = AtomicBool::new(false);

/// `true` ⇔ rig shell đã dựng (handler `int 0x80` định tuyến sang [`dispatch`]).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// TCB hiện hành của shell (`None` ⇔ chưa dựng / đã Exit).
pub fn current_tcb() -> Option<ObjId> {
    if !active() {
        return None;
    }
    let tcb = SHELL_TCB.load(Ordering::Acquire);
    (tcb != NO_TCB).then_some(tcb)
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

/// Cấp frame ring KT-69, khởi tạo hình học 32×32 rồi bơm [`SCRIPT`] bằng **cùng** codec
/// `conchan` mà `shell.rs` đọc (một định dạng, hai phía).
fn make_ring(console: &mut Console, arena: &mut BootArena) -> u64 {
    let pa = alloc(console, arena, "ring");
    let mut offset = 0u64;
    while offset < 4096 {
        asm::write_u64(pa + offset, 0);
        offset += 8;
    }
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
    while start < SCRIPT.len() {
        let mut slot = [0u8; vifast_virtio::conchan::CHUNK_BYTES];
        let mut used = 0usize;
        while start < SCRIPT.len() && used < slot.len() {
            slot[used] = SCRIPT[start];
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

/// Dựng rig (CSpace + Endpoint + TCB + Frame IPC buffer), nạp ELF shell, map stack/ring/buffer,
/// rồi `iretq` xuống CPL3. **Không trả về**: `Exit`/lỗi dừng trong [`dispatch`] (kernel tiếp quản).
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
    let (untyped, fresh_machine) = machine_untyped(console, &mut arena, fresh);

    // ---- Frame IPC buffer (zero toàn bộ: shell chỉ dùng MR0..MR3 trên thanh ghi, không header).
    let buf_pa = alloc(console, &mut arena, "ipc-buffer");
    let mut offset = 0u64;
    while offset < 4096 {
        asm::write_u64(buf_pa + offset, 0);
        offset += 8;
    }
    BUF_PA.store(buf_pa, Ordering::Release);
    syscall_env::set_ipc_buffer_va(Some(IPC_BUF_VA));
    syscall_env::reset_user_ranges();

    // ---- `VSpace` **riêng** + nạp ELF shell thật + stack + ring.
    let mut vs = match VSpace::create(4, arena) {
        Ok(vs) => vs,
        Err(error) => {
            let _ = writeln!(console, "PANIC: shellpath: tạo VSpace lỗi {error:?}");
            asm::halt(3)
        }
    };
    let loaded = match userload::load_elf(&mut vs, &mut arena, SHELL_ELF, EM_X86_64) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: shellpath: nạp ELF lỗi {} elf={}",
                error.name(),
                error.elf_code()
            );
            asm::halt(3)
        }
    };
    if let Err(error) =
        userload::map_user_stack(&mut vs, &mut arena, SHELL_STACK_TOP, SHELL_STACK_PAGES)
    {
        let _ = writeln!(console, "PANIC: shellpath: map stack lỗi {}", error.name());
        asm::halt(3)
    }
    if let Err(error) = vs.map(IPC_BUF_VA, buf_pa, Perms::USER_RW) {
        let _ = writeln!(console, "PANIC: shellpath: map ipc-buffer lỗi {error:?}");
        asm::halt(3)
    }
    if !syscall_env::register_user_range(IPC_BUF_VA, 0x1000) {
        let _ = writeln!(console, "PANIC: shellpath: đăng ký ipc-buffer lỗi");
        asm::halt(3)
    }
    let ring_pa = make_ring(console, &mut arena);
    RING_PA.store(ring_pa, Ordering::Release);
    if let Err(error) = vs.map(userload::RING_VA, ring_pa, Perms::USER_RW) {
        let _ = writeln!(console, "PANIC: shellpath: map ring lỗi {error:?}");
        asm::halt(3)
    }
    if !syscall_env::register_user_range(userload::RING_VA, 0x1000) {
        let _ = writeln!(console, "PANIC: shellpath: đăng ký ring lỗi");
        asm::halt(3)
    }
    let shell_root = vs.root();
    SHELL_ROOT.store(shell_root, Ordering::Release);
    let _ = writeln!(
        console,
        "# shell: load entry=0x{:x} base=0x{:x}+{}p segs={} stack_top=0x{:x} buf_va=0x{:x} ring_va=0x{:x} script={}B root=0x{:x}",
        loaded.entry,
        loaded.base,
        loaded.pages,
        loaded.segments,
        SHELL_STACK_TOP,
        IPC_BUF_VA,
        userload::RING_VA,
        SCRIPT.len(),
        shell_root,
    );

    // ---- Rig trên `Machine` (một `with_machine`; không in bên trong khoá).
    let built = core_bridge::with_machine(|m: &mut Machine| -> Result<(), KError> {
        let root = m.add_cnode(SHELL_SLOTS);
        ROOT.store(root, Ordering::Release);
        let uc = m.root_cap(untyped);
        m.put_cap(root, SLOT_UNTYPED, uc)?;
        let us = SlotRef::new(root, SLOT_UNTYPED);
        for slot in [SLOT_EP_PROC, SLOT_EP_REPLY, SLOT_EP_PS, SLOT_EP_EVENTS] {
            let cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
            m.put_cap(root, slot, cap)?;
        }
        let cspace_cap = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cspace_cap)?;
        let frame_obj = m.add_foreign_object(ObjType::Frame, buf_pa)?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, SLOT_FRAME, frame_cap)?;
        let tcb_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_TCB_SHELL, tcb_cap)?;
        let shell = m.cap_obj(tcb_cap)?;
        SHELL_TCB.store(shell, Ordering::Release);
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(root, SLOT_TCB_SHELL),
            Some(SlotRef::new(root, SLOT_CSPACE)),
            Some((SlotRef::new(root, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        m.tcb_set_priority(SlotRef::new(root, SLOT_TCB_SHELL), 10, u8::MAX)?;
        m.start_tcb(shell)?;
        m.transition_tcb(shell, NonReadyTarget::Running { hart: 0 })?;
        Ok(())
    });
    if built.is_err() {
        let _ = writeln!(console, "PANIC: shellpath: dựng rig lỗi");
        asm::halt(3)
    }

    // ---- Vào ring 3: `CR3` shell + `TSS.RSP0` + `iretq` (không trả).
    let boot_root = asm::read_cr3() & !0xFFF;
    BOOT_ROOT.store(boot_root, Ordering::Release);
    tss::set_rsp0(0, core::ptr::addr_of!(__syscall_stack_top) as u64);
    vs.activate();
    syscall_env::capture_reset();
    ACTIVE.store(true, Ordering::Release);
    let _ = writeln!(
        console,
        "# shell: enter cpl=3 entry=0x{:x} rsp=0x{:x} cr3=0x{:x} tcb={} untyped={} fresh={}",
        loaded.entry,
        SHELL_STACK_TOP,
        shell_root,
        SHELL_TCB.load(Ordering::Acquire),
        untyped,
        u8::from(fresh_machine),
    );
    asm::enter_user(loaded.entry, SHELL_STACK_TOP, USER_RFLAGS)
}

/// Ảnh **sản phẩm** (`--features product-shell`): `Machine` mới + shell thật, rồi marker + halt.
pub fn start_product(console: &mut Console) -> ! {
    prepare_and_run(console, true)
}

/// Ảnh **kiểm** (`--features ktest`): dùng lại `Machine` của KT-340c, chạy shell (TAP 44/45) rồi
/// trả chuỗi boot về `net::run`.
pub fn start_ktest(console: &mut Console) -> ! {
    prepare_and_run(console, false)
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

/// `int 0x80` của shell — **một** đường qua `vifast_core::syscall::dispatch` (D2-79). Trả
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

    // **Đột biến (ii)** — tắt đường `DebugPutStr` của **core** trên ảnh: shell không in được
    // prompt/help ⇒ TAP 44/45 **ĐỎ**. `Exit` vẫn đi qua core để phép đo kết thúc (nếu chặn cả
    // Exit thì ảnh treo, không còn "đỏ có chẩn đoán").
    #[cfg(feature = "mut-no-int80")]
    if nr == vifast_abi::SYS_DEBUG_PUT_STR {
        return SyscallRet {
            error: 0,
            value: a1,
        };
    }

    let Some(tcb) = current_tcb() else {
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
            SyscallRet {
                error: result.error,
                value: result.value,
            }
        }
        vifast_core::syscall::SyscallOutcome::Exit { code } => finish(&mut console, code),
        vifast_core::syscall::SyscallOutcome::Switched { .. } => {
            let _ = writeln!(
                console,
                "PANIC: shellpath: Switched ngoài scheduler (nr={nr})"
            );
            asm::halt(2)
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
/// (sản phẩm: marker + halt; ktest: TAP 44/45 + `net::run`).
fn finish(console: &mut Console, code: u64) -> ! {
    // Trả `CR3` về cây gốc **trước** khi hoạt động tiếp (cùng bài học `KT-340b`).
    let boot_root = BOOT_ROOT.load(Ordering::Acquire);
    if boot_root != 0 {
        asm::write_cr3(boot_root);
    }
    EXIT_CODE.store(code, Ordering::Release);
    let (state, objects, caps) = core_bridge::with_machine(|m| {
        (
            m.tcb_state(SHELL_TCB.load(Ordering::Acquire)).ok(),
            m.object_count(),
            m.live_cap_count(),
        )
    });
    let capture = syscall_env::capture_report();
    let state_code = state.map(|s| match s {
        TcbState::Inactive => 0u64,
        TcbState::Dead => 10,
        _ => 2,
    });
    let _ = writeln!(
        console,
        "# shell: exit code={code} state={} puts={} bytes={} hash=0x{:016x} prompt={} help={} echo={} exit_line={} syscalls={} yields={} user_cs={} cr3_boot=0x{boot_root:x} cr3_shell=0x{:x} ring_pa=0x{:x} buf_pa=0x{:x} objects={objects} caps={caps}",
        state_code.unwrap_or(99),
        capture.puts,
        capture.bytes,
        capture.hash,
        u8::from(capture.prompt),
        u8::from(capture.help),
        u8::from(capture.echo),
        u8::from(capture.exit_line),
        SYSCALLS.load(Ordering::Acquire),
        YIELDS.load(Ordering::Acquire),
        u8::from(SAW_USER_CS.load(Ordering::Acquire)),
        SHELL_ROOT.load(Ordering::Acquire),
        RING_PA.load(Ordering::Acquire),
        BUF_PA.load(Ordering::Acquire),
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
            "shell: help + echo builtin (16 core syscalls, no ktest code in ELF)",
            capture.help && capture.echo,
        );
        crate::net::run(console, 15)
    }
    #[cfg(all(feature = "product-shell", not(feature = "ktest")))]
    {
        let _ = capture;
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
const _: () = assert!(SHELL_STACK_TOP - (SHELL_STACK_PAGES as u64) * 0x1000 > IPC_BUF_VA);
