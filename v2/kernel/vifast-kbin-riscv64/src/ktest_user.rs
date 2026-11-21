// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test U-mode** riscv64 (KT-53) — bin riêng, feature `ktest`, nạp qua OpenSBI.
//!
//! **KT-66: ảnh này là ảnh syscall/core (14 TAP)** — phần **demo** procsrv/shell/con + 3 TAP
//! fault/wx đã tách sang ảnh thứ tư `vifast-kbin-ktest-proc` (`src/ktest_proc.rs`) để ảnh này
//! trả headroom ảnh (~40 KiB) và mỗi ảnh giữ đúng **một** trách nhiệm. Boot kết thúc bằng
//! `panic!` **có chủ đích** ⇒ đúng một dòng `PANIC: KT-66 u-mode pha syscall xong (14 TAP, 0
//! fail)`; **không** còn fault U-mode trong ảnh này.
//!
//! TAP (14 test):
//!
//! 1. `ok 1 - init u-mode` — nạp `init` ELF bằng `vifast-elf` (parse + map `PT_LOAD` theo quyền,
//!    **W^X**: `.text` `U|R|X`, `.rodata` `U|R|NX`, `.data` `U|R|W|NX`), stack user 8 trang +
//!    guard hở; `sret` xuống U-mode (`sstatus.SPP=0`, `sscratch` giữ trap stack per-hart), `init`
//!    chạy thật và in marker qua `ecall`.
//! 2. `ok 2 - syscall debug_put_str` — 2 lần `DebugPutStr` (12) in **đúng byte user**
//!    (`U: init hello from u-mode` / `U: bad syscall err=19 ok`), 51 byte.
//! 3. `ok 3 - syscall exit` — `Exit(42)` (14) QUA CORE: tác vụ sang `Exited`, lưu
//!    `exit_code=42`, **không** quay lại U-mode (entries=1).
//! 4. `ok 4 - syscall la ma loi` — 5 `ecall` ngoài dải ABI ⇒ `RangeError` (19, D2-79),
//!    **không panic**; `init` kiểm `a0==19` rồi mới `Exit(42)`.
//! 5. `ok 5 - syscall debug_get_info` — `DebugGetInfo` (15) qua core: `a1=abi_version`,
//!    `a3=min_compat`, `a4=features`, `a5=num_harts` (D2-29/D2-35); `init` kiểm rồi mới `Exit`.
//! 6. `ok 6 - syscall yield` — `Yield` (8) qua core: một TCB ⇒ round-trip
//!    `Running→Ready→Running`, `a1` = budget còn lại.
//! 7. `ok 7 - syscall 16 duong` — pha **kernel-level**: gọi từng `nr` `0..15` (trừ 14) qua
//!    `vifast_core::syscall::dispatch` trên `Machine` của ảnh; in `# sys: tap=7 nr=…`.
//! 8. `ok 8 - syscall sai cap` — slot rỗng ⇒ 1, Send trên cap TCB ⇒ 2, Call trên cap TCB với
//!    **method chưa cài** (1 `SetFaultEp`) ⇒ 18 (từ KT-290, method 0 của Tcb là `Configure`
//!    **đã cài** nên không còn là "method ngoài danh sách")
//!    (D2-53 Q11).
//! 9. `ok 9 - syscall ipc vong` — Call(0)/Recv(3)/Send trên Reply cap(1) qua dispatch, MR tới
//!    đích, `Reply.caller` sạch.
//! 10. `ok 10 - syscall nb ipc` — NBSend(2)/NBRecv(4)/NBSendRecv(6)/NBSendWait(7).
//! 11. `ok 11 - syscall waitaddr wakeaddr` — WaitAddr(9) chặn đúng khoá, WakeAddr(10) đánh
//!     thức 1, lệch giá trị ⇒ 5, địa chỉ không dịch được ⇒ 11.
//! 12. `ok 12 - syscall clock read` — ClockRead(11) đơn điệu.
//! 13. `ok 13 - syscall yield doi tcb` — Yield(8) với 2 TCB: current đổi `t0 → t1`.
//! 14. `ok 14 - syscall exit khong reply` — `Exit` của `init` vào `fault_ep`: TCB
//!     `BlockedOnSend{call:false, fault=Exit{42}}`, hàng có **fault** (không message), **không**
//!     Reply (D2-25).
//!
//! TAP 15..24 cũ (demo + fault/wx) nằm ở **ảnh `ktest-proc`**, đánh số 1..10 — ánh xạ ghi ở
//! `v2/test/boot/expected.txt` + `v2/docs/k35-procsrv.md` §10.
//!
//! Chương trình `init` do `build.rs` biên dịch từ `init/init_user.rs` (+ `.S`) thành ELF64 tại
//! `0x4000_0000` rồi nhúng qua `include_bytes!` — không loader động, không reloc, không cần
//! toolchain ngoài Rust.
#![no_std]
#![no_main]

// KT-67a: pha invocation tách tệp (R1 ≤ 1.500 dòng).
#[path = "ktest_inv.rs"]
mod ktest_inv;
// KT-67b: pha chuyển cap Frame giữa hai tiến trình U-mode (tách tệp, R1 ≤ 1.500 dòng).
#[path = "ktest_capx.rs"]
mod ktest_capx;
// KT-68: pha capalloc (CNode.*) — tách tệp (R1 ≤ 1.500 dòng).
#[path = "ktest_capa.rs"]
mod ktest_capa;
// KT-69 (K6): IDL + ring chia sẻ (TAP 25..29) — hai tiến trình U-mode thật.
#[path = "ktest_ring.rs"]
mod ktest_ring;
// KT-70 (K6): VFS tầng 1 — pager + page cache, fault→pager, hai client (TAP 30..34).
#[path = "ktest_vfs.rs"]
mod ktest_vfs;

use core::fmt::Write;

use vifast_core::syscall::{SyscallEnv, SyscallOutcome, REPLY_NONE};
use vifast_core::{
    FaultKind as CoreFaultKind, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState,
};
use vifast_plat_common::fault::{FaultKind, FAULT_STATS};
use vifast_plat_common::user::TaskState;
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::user::{self, USER_BASE, USER_STACK_GUARD_BOTTOM};

use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// ELF64 `init` do `build.rs` sinh (`init/init_user.rs` + `.S` + `.ld`), nhúng vào ảnh test.
static INIT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/init.elf"));

/// Số byte của 2 marker trong `init` — `init` kiểm `a1` sau `DebugPutStr`, host test/runner kiểm
/// lại tổng byte (26 + 25 = 51).
const MARKER_BYTES: u64 = 51;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..37\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    // Bảng trang + arena/guard trước (mmu::init bật satp); ASID lease rồi object layer.
    let mmu = mmu::init();
    let _tlb = vifast_plat_riscv64::tlb::init();
    let obj = vifast_plat_riscv64::obj::init();
    // KT-62: core `no_std` **thật** của ảnh (static trong `.bss`, D2-84) + TCB `init` trên nó.
    let core = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    // KT-66: stack syscall **per-hart** cấp từ arena (một Frame 2 MiB) **trước** `load_init` để
    // Frame 4 KiB của ELF lấy sau; ảnh này không còn `SYSCALL_STACK` 32 KiB trong `.bss`.
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - init u-mode: khong cap duoc stack per-hart\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    let (sys, init_tcb, init_ep) = core_bridge::with_machine(|m| {
        let rig = build_sys_rig(m, core.untyped);
        let (tcb, ep) = build_init_task(m, &rig);
        (rig, tcb, ep)
    });
    user::bind_core_tcb(init_tcb);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let state = core_bridge::with_machine(|m| m.tcb_state(init_tcb).ok());
        let current = core_bridge::with_machine(|m| m.current(0));
        let _ = writeln!(
            w,
            "# user core: static={} pin={} image_cap={} untyped={} harts={} tcb={} ep={} cspace={} state={} current={} check=1",
            core.static_bytes,
            core.pin,
            core_bridge::IMAGE_CAP_BYTES,
            core.arena_bytes,
            vifast_plat_riscv64::smp::hart_count(),
            init_tcb,
            init_ep,
            sys.root,
            state.map(tcb_state_name).unwrap_or("none"),
            (current == Some(init_tcb)) as u8
        );
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# user image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} root=0x{:x} obj_arena=0x{:x}-0x{:x} untyped={}",
            PLATFORM.hart_id(),
            stvec,
            (stvec & 0b11 == 0) as u8,
            mmu.satp,
            mmu.satp >> 60,
            mmu.root,
            obj.arena_base,
            obj.arena_end,
            obj.untypeds
        );
        let _ = writeln!(
            w,
            "# init elf: bytes={} entry=0x40000000 user=[0x{:x},0x{:x}) stack_guard=0x{:x}",
            INIT_ELF.len(),
            USER_BASE,
            vifast_plat_riscv64::user::USER_TOP,
            USER_STACK_GUARD_BOTTOM
        );
    }

    let loaded = match user::load_init(INIT_ELF, obj.untyped0) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - init u-mode: nap ELF that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# init load: elf=1 entry=0x{:x} segs={} text={} ro={} data={} stack={} image_end=0x{:x} guard=0x{:x} guard_none={} wx={} flags={} high={} root=0x{:x} asid={} check={}",
            loaded.entry,
            loaded.segments,
            loaded.text_pages,
            loaded.ro_pages,
            loaded.data_pages,
            loaded.stack_pages,
            loaded.image_end,
            loaded.stack_guard,
            loaded.guard_unmapped as u8,
            loaded.wx_pages,
            loaded.flags_ok as u8,
            loaded.high_half_unmapped as u8,
            loaded.root,
            loaded.asid,
            loaded.check_ok as u8
        );
    }
    if loaded.entry & 0xfff != 0
        || loaded.segments != 3
        || loaded.text_pages != 1
        || loaded.ro_pages != 1
        || loaded.data_pages != 1
        || loaded.stack_pages != 8
        || !loaded.guard_unmapped
        || loaded.wx_pages != 0
        || !loaded.flags_ok
        || !loaded.high_half_unmapped
        || !loaded.check_ok
    {
        PLATFORM.put_str("not ok 1 - init u-mode: plan ELF/mapping khong dung\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ---------------------------------------------------------------- 1..4) lượt U-mode chuẩn
    let snap = match user::enter(&loaded, 0) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - init u-mode: enter that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# init run: entry=0x{:x} u_mode=1 sret_spp={} ecall_spp={} entries={} syscalls={} exits={} exit={}",
            loaded.entry,
            snap.enter_spp,
            snap.ecall_spp,
            snap.entries,
            snap.syscalls,
            snap.exits,
            snap.exit_code
        );
    }
    let mut fails = 0usize;
    // KT-56/D2-56a: `Exit(42)` phải được ghi thành `FaultKind::Exit{42}` **qua ABI** (syscall
    // 14). Kiểm ngay sau lượt U-mode đầu (TAP 6 in sau TAP 5, giữ thứ tự TAP liên tục).
    let exit_kind_ok = FAULT_STATS.last() == Some(FaultKind::Exit { code: 42 });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(w, "# user exitkind: code=42 kind=");
        match FAULT_STATS.last() {
            Some(kind) => {
                let _ = write!(w, "{kind}");
            }
            None => {
                let _ = write!(w, "none");
            }
        }
        let _ = writeln!(w, " ok={}", u8::from(exit_kind_ok));
    }

    // TAP 1: đã hạ sret xuống U-mode thật, mọi ecall đều từ U-mode (SPP=0), và đã exit sạch.
    // Bằng chứng `sstatus` hai đầu: giá trị ghi CSR lúc vào U (SUM/SIE/SPIE/SPP=0) và giá trị
    // phần cứng để lại ở trap `ecall` đầu tiên (SPP=0, SIE=0, SPIE=0, SUM=0).
    const SSTATUS_MASK: u64 = (1 << 8) | (1 << 5) | (1 << 1) | (1 << 18);
    let sstatus_ok = snap.enter_sstatus & SSTATUS_MASK == 0
        && snap.trap_sstatus & SSTATUS_MASK == 0
        && snap.trap_sstatus >> 32 & 0x3 == 2; // UXL=2 (64-bit) — hardware thật
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# u sstatus: enter=0x{:x} trap=0x{:x} spp=0 spie=0 sie=0 sum=0 uxl=2",
            snap.enter_sstatus, snap.trap_sstatus
        );
    }
    let ok1 = snap.entries == 1
        && snap.enter_spp == 0
        && snap.ecalls == 10
        && snap.ecall_spp == 0
        && snap.syscalls == 10
        && snap.state == TaskState::Exited
        && snap.exits == 1
        && sstatus_ok;
    if ok1 {
        PLATFORM.put_str("ok 1 - init u-mode\n");
    } else {
        PLATFORM.put_str("not ok 1 - init u-mode: entries/spp/ecall khong dung\n");
        fails += 1;
    }

    // TAP 2: DebugPutStr in đúng 2 marker, 51 byte, do chính byte user quyết định; cửa sổ đọc
    // user dùng `sstatus.SUM` phải đóng lại (`SUM=0`) sau khi in.
    let sum_off = vifast_plat_riscv64::obj::sum_off();
    let ok2 = snap.put_calls == 2 && snap.put_bytes == MARKER_BYTES && sum_off;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# user put_str: calls={} bytes={} sum_off={}",
            snap.put_calls, snap.put_bytes, sum_off as u8
        );
    }
    if ok2 {
        PLATFORM.put_str("ok 2 - syscall debug_put_str\n");
    } else {
        PLATFORM.put_str("not ok 2 - syscall debug_put_str: so lan/byte khong dung\n");
        fails += 1;
    }

    // TAP 3: Exit(42) — Exited, đúng mã, không quay lại U-mode.
    let ok3 = snap.state == TaskState::Exited
        && snap.exits == 1
        && snap.exit_code == 42
        && snap.entries == 1;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# user exit: code={} state={} exits={}",
            snap.exit_code,
            snap.state.name(),
            snap.exits
        );
    }
    if ok3 {
        PLATFORM.put_str("ok 3 - syscall exit\n");
    } else {
        PLATFORM.put_str("not ok 3 - syscall exit: ma/trang thai khong dung\n");
        fails += 1;
    }

    // TAP 4: syscall lạ (ngoài dải, gồm EID SBI) trả mã lỗi ABI 5, không panic; `init` chỉ
    // Exit(42) khi cả hai lần đều thấy a0=5.
    let ok4 = snap.bad_calls == 5
        && snap.last_bad_nr == 31
        && snap.last_bad_error == 19
        && snap.exit_code == 42;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# user badsys: nr={} err={} calls={} panic=0",
            snap.last_bad_nr, snap.last_bad_error, snap.bad_calls
        );
    }
    if ok4 {
        PLATFORM.put_str("ok 4 - syscall la ma loi\n");
    } else {
        PLATFORM.put_str("not ok 4 - syscall la ma loi: ma loi/so lan khong dung\n");
        fails += 1;
    }

    // ---------------- TAP 5: DebugGetInfo (15) qua core — bằng chứng từ bộ đếm trap handler
    let info_calls = user::sys_calls(15);
    let info_err = user::sys_err(15);
    let info_val = user::sys_val(15);
    let ok5 = info_calls == 1 && info_err == 0 && info_val == vifast_core::syscall::ABI_VERSION;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# user getinfo: nr=15 calls={} err={} abi_version={} num_harts={} exit={}",
            info_calls,
            info_err,
            info_val,
            vifast_plat_riscv64::smp::hart_count(),
            snap.exit_code
        );
    }
    if ok5 {
        PLATFORM.put_str("ok 5 - syscall debug_get_info\n");
    } else {
        PLATFORM.put_str("not ok 5 - syscall debug_get_info: so lan/ma sai\n");
        fails += 1;
    }

    // ---------------- TAP 6: Yield (8) qua core (một TCB ⇒ round-trip Running→Ready→Running)
    let yield_calls = user::sys_calls(8);
    let yield_err = user::sys_err(8);
    let yield_val = user::sys_val(8);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# user yield: nr=8 calls={} err={} budget_left={}",
            yield_calls, yield_err, yield_val
        );
    }
    let ok6 = yield_calls == 1 && yield_err == 0 && yield_val <= 100;
    if ok6 {
        PLATFORM.put_str("ok 6 - syscall yield\n");
    } else {
        PLATFORM.put_str("not ok 6 - syscall yield: so lan/ma/budget sai\n");
        fails += 1;
    }

    if fails > 0 {
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ---------------- TAP 7..13: kernel-level dispatch (cùng `Machine`, TCB riêng)
    let kernel_invariants = kernel_sys_taps(&sys, &mut fails);

    // ---------------- TAP 14: `Exit` của init đi vào core, không Reply
    let mut exit_state = None;
    let mut exit_len = 0usize;
    let mut exit_sender = 0u32;
    let mut exit_fault: Option<CoreFaultKind> = None;
    let mut exit_call = true;
    core_bridge::with_machine(|m| {
        exit_state = m.tcb_state(init_tcb).ok();
        exit_len = m.endpoint_send_len(init_ep).unwrap_or(usize::MAX);
        if let Ok(items) = m.endpoint_send_items(init_ep) {
            let list: [Option<(ObjId, Option<CoreFaultKind>)>; 2] = {
                let mut tmp = [None; 2];
                for (i, item) in items.take(2).enumerate() {
                    tmp[i] = Some((item.0, item.1));
                }
                tmp
            };
            if let Some((sender, fault)) = list[0] {
                exit_sender = sender;
                exit_fault = fault;
            }
        }
        if let Ok(TcbState::BlockedOnSend { call, .. }) = m.tcb_state(init_tcb) {
            exit_call = call;
        }
    });
    let want_state = TcbState::BlockedOnSend {
        ep: init_ep,
        call: false,
        fault: Some(CoreFaultKind::Exit { code: 42 }),
    };
    let exit_calls = user::sys_calls(14);
    let ok14 = exit_state == Some(want_state)
        && exit_sender == init_tcb
        && exit_fault == Some(CoreFaultKind::Exit { code: 42 })
        && exit_len == 1
        && !exit_call
        && exit_calls == 1
        && kernel_invariants
        && snap.exit_code == 42;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# sys: tap=14 nr=14 err=0 code=42 tcb={} state={} call={} fault={} ep_len={} sender={} reply=0",
            init_tcb,
            exit_state.map(tcb_state_name).unwrap_or("none"),
            exit_call as u8,
            match exit_fault {
                Some(CoreFaultKind::Exit { code }) => {
                    if code == 42 {
                        "Exit{42}"
                    } else {
                        "Exit{other}"
                    }
                }
                _ => "none",
            },
            exit_len,
            exit_sender
        );
    }
    if ok14 {
        PLATFORM.put_str("ok 14 - syscall exit khong reply\n");
    } else {
        PLATFORM.put_str("not ok 14 - syscall exit khong reply: state/hang sai\n");
        fails += 1;
    }

    if fails > 0 {
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ---------------- TAP 15..18: KT-67a invocation (D2-97) trên U-mode thật ----------------
    ktest_inv::invocation_phase(sys.root, sys.us, sys.cspace, obj.untyped0, &mut fails);

    // ---------------- TAP 19: KT-67b chuyển cap Frame giữa **hai tiến trình** U-mode --------
    ktest_capx::capx_phase(sys.root, sys.us, sys.ctl, obj.untyped0, &mut fails);

    // ---------------- TAP 20..24: KT-68 capalloc (CNode cấp/thu hồi theo cây MDB) ----------
    ktest_capa::capalloc_phase(sys.root, sys.us, sys.ctl, obj.untyped0, &mut fails);

    // ---------------- TAP 25..29: KT-69 IDL + ring chia sẻ (hai tiến trình U-mode) ----------
    ktest_ring::ring_phase(sys.root, sys.us, sys.ctl, obj.untyped0, &mut fails);

    // ---------------- TAP 30..37: KT-70/KT-150/KT-157 pager + page cache ----------------
    // **KT-157**: trả tài nguyên `init` (VSpace + 12 Frame + 3 PageTable) **trước** pha pager —
    // pha này dựng **năm** VSpace (pager + 4 client U-mode) nên `MAX_OBJECTS = 96` chỉ đủ khi
    // `init` (đã `Exited`, mọi TAP 1..6 đã đo xong) nhường chỗ. Không đổi TAP nào.
    loaded.release();
    ktest_vfs::pager_phase(sys.root, sys.us, sys.ctl, obj.untyped0, &mut fails);

    if fails > 0 {
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ---------------- kết thúc sạch (KT-66): ảnh user chỉ còn 14 TAP syscall/core -------------
    // Demo procsrv/shell/con + fault phase KT-56 đã **tách** sang ảnh `vifast-kbin-ktest-proc`
    // (`src/ktest_proc.rs`) để ảnh này trả headroom: không còn `proc_demo`, 3 ELF demo và
    // 32 KiB stack syscall tĩnh trong `.bss` của ảnh này.
    user::print_stack_evidence();
    PLATFORM.put_str("# ktest-user: taps=37 ok=1\n");
    // PANIC **có chủ đích** (không phải fault): kết thúc boot sạch với đúng **một** dòng PANIC
    // theo mẫu của runner (`expected.txt`: `ktest-user 14 14`). In thẳng + `halt` (không qua
    // `panic!`) để log không có dòng `PANIC at …` thứ hai.
    PLATFORM.put_str(
        "PANIC: KT-70 u-mode pha syscall, invocation, capalloc, IDL va ring va pager xong (37 TAP, 0 fail)\n",
    );
    PLATFORM.halt(1)
}

// =============================================================================================
// KT-62 — rig core cho `init` + TAP kernel-level của bảng 16 syscall
// =============================================================================================

/// Tên trạng thái TCB ổn định cho log/TAP (không in `{:?}` có số hart).
fn tcb_state_name(state: TcbState) -> &'static str {
    match state {
        TcbState::Inactive => "Inactive",
        TcbState::Ready => "Ready",
        TcbState::Running { .. } => "Running",
        TcbState::BlockedOnSend { .. } => "BlockedOnSend",
        TcbState::BlockedOnRecv { .. } => "BlockedOnRecv",
        TcbState::BlockedOnReply { .. } => "BlockedOnReply",
        TcbState::BlockedOnNotification { .. } => "BlockedOnNotification",
        TcbState::BlockedOnAddress { .. } => "BlockedOnAddress",
        TcbState::Sleeping { .. } => "Sleeping",
        TcbState::Stopping => "Stopping",
        TcbState::Dead => "Dead",
    }
}

/// Hook arch **test** cho các TAP kernel-level: địa chỉ cố định (7, 64), đồng hồ tăng 10 ns,
/// console đếm byte. Đường U-mode dùng hook thật (`vifast_plat_riscv64::user`), không dùng bản này.
struct TestEnv {
    now: u64,
    addr: Option<(u32, u32)>,
    puts: usize,
}

impl TestEnv {
    const fn new() -> Self {
        Self {
            now: 1_000,
            addr: Some((7, 64)),
            puts: 0,
        }
    }
}

impl SyscallEnv for TestEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        self.puts += 1;
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        self.addr.ok_or(11)
    }
    fn now_ns(&mut self) -> u64 {
        self.now += 10;
        self.now
    }
    fn features(&mut self) -> u64 {
        0b1111
    }

    fn num_harts(&mut self) -> u64 {
        // D2-88(a): số hart **nền tảng** (SBI HSM probe) — TAP 7 pin 1/2/4 theo `-smp`.
        vifast_plat_riscv64::smp::hart_count() as u64
    }
}

/// Bộ định danh của pha syscall (dựng từ **cùng** Untyped của ảnh, sau `init`).
struct SysRig {
    root: ObjId,
    us: SlotRef,
    t0: ObjId,
    t1: ObjId,
    reply: ObjId,
    ntfn: ObjId,
    t0s: SlotRef,
    t1s: SlotRef,
    eps: SlotRef,
    rs: SlotRef,
    ns: SlotRef,
    ctl: SlotRef,
    sc0: SlotRef,
    sc1: SlotRef,
    cspace: SlotRef,
    init_tcb_slot: SlotRef,
    init_ep_slot: SlotRef,
    init_sc_slot: SlotRef,
}

/// Dựng CNode 32 slot + 2 TCB + Endpoint/Reply/Notification + SchedControl + 2 SC (từ Untyped).
///
/// TCB kernel-level để **Inactive** (TAP 7 tự đưa về `Running`); slot dành cho `init`:
/// 10 = TCB, 11 = `fault_ep`, 12 = SC.
fn build_sys_rig(m: &mut Machine, u: ObjId) -> SysRig {
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("rig: cap Untyped");
    let types = [
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::Notification,
        ObjType::SchedControl,
        ObjType::SchedContext,
        ObjType::SchedContext,
    ];
    let mut obj = [0u32; 8];
    for (i, ty) in types.iter().enumerate() {
        let cap = m.retype(us, *ty, 0, 0).expect("rig: retype object lõi");
        m.put_cap(root, 1 + i, cap).expect("rig: đặt cap");
        obj[i] = m.cap_obj(cap).expect("rig: cap_obj");
    }
    let cs = m.root_cap(root);
    m.put_cap(root, 9, cs).expect("rig: cap cspace");
    let rig = SysRig {
        root,
        us,
        t0: obj[0],
        t1: obj[1],
        reply: obj[3],
        ntfn: obj[4],
        t0s: SlotRef::new(root, 1),
        t1s: SlotRef::new(root, 2),
        eps: SlotRef::new(root, 3),
        rs: SlotRef::new(root, 4),
        ns: SlotRef::new(root, 5),
        ctl: SlotRef::new(root, 6),
        sc0: SlotRef::new(root, 7),
        sc1: SlotRef::new(root, 8),
        cspace: SlotRef::new(root, 9),
        init_tcb_slot: SlotRef::new(root, 10),
        init_ep_slot: SlotRef::new(root, 11),
        init_sc_slot: SlotRef::new(root, 12),
    };
    // Cấu hình 2 TCB kernel-level (vẫn Inactive): cspace + SC + priority.
    for (slot, sc) in [(rig.t0s, rig.sc0), (rig.t1s, rig.sc1)] {
        m.tcb_configure(slot, Some(rig.cspace))
            .expect("rig: cspace");
        m.sched_control_configure(rig.ctl, sc, 500, 5_000)
            .expect("rig: SC");
        m.set_sched_params(slot, Some(sc)).expect("rig: params");
        m.tcb_set_priority(slot, 3, 10).expect("rig: priority");
    }
    rig
}

/// Tạo TCB `init` trên core: cspace + `fault_ep` + SC + priority, `Ready` rồi `dispatch(0)` ⇒
/// TCB **đang chạy** trên hart 0 (bằng chứng `# user core: current=1`).
fn build_init_task(m: &mut Machine, rig: &SysRig) -> (ObjId, ObjId) {
    let tcb = m
        .retype(rig.us, ObjType::Tcb, 0, 0)
        .expect("init: retype TCB");
    let ep = m
        .retype(rig.us, ObjType::Endpoint, 0, 0)
        .expect("init: retype Endpoint");
    let sc = m
        .retype(rig.us, ObjType::SchedContext, 0, 0)
        .expect("init: retype SC");
    let tcb_cap = tcb;
    let ep_cap = ep;
    let sc_cap = sc;
    m.put_cap(rig.root, 10, tcb_cap).expect("init: cap TCB");
    m.put_cap(rig.root, 11, ep_cap).expect("init: cap EP");
    m.put_cap(rig.root, 12, sc_cap).expect("init: cap SC");
    let tcb_obj = m.cap_obj(tcb_cap).expect("init: obj TCB");
    let ep_obj = m.cap_obj(ep_cap).expect("init: obj EP");
    m.tcb_configure(rig.init_tcb_slot, Some(rig.cspace))
        .expect("init: cspace");
    m.tcb_set_fault_ep(rig.init_tcb_slot, rig.init_ep_slot)
        .expect("init: fault_ep");
    m.sched_control_configure(rig.ctl, rig.init_sc_slot, 100, 1_000)
        .expect("init: SC");
    m.set_sched_params(rig.init_tcb_slot, Some(rig.init_sc_slot))
        .expect("init: params");
    m.tcb_set_priority(rig.init_tcb_slot, 3, 10)
        .expect("init: priority");
    m.start_tcb(tcb_obj).expect("init: Ready");
    let got = m.dispatch(0).expect("init: dispatch");
    debug_assert_eq!(got, Some(tcb_obj), "init phải là TCB chạy trên hart 0");
    (tcb_obj, ep_obj)
}

/// Đưa TCB về `Running{0}` (TAP kernel-level gọi sau mỗi syscall làm nó chặn).
fn make_running(m: &mut Machine, tcb: ObjId) {
    if m.tcb_state(tcb) != Ok(TcbState::Running { hart: 0 }) {
        let _ = m.wake(tcb);
        if m.tcb_state(tcb) == Ok(TcbState::Ready) {
            let _ = m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 });
        }
    }
}

fn args6(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> [u64; 6] {
    [a0, a1, a2, a3, a4, a5]
}

/// `MessageInfo` 35 bit: label 0, caps 0, no_reply 0, extra 0, length `len`.
fn mk_info(len: u8) -> u64 {
    u64::from(len) << 28
}

fn outcome_class(outcome: &SyscallOutcome) -> &'static str {
    match outcome {
        SyscallOutcome::Return(r) if r.error == 0 => "ok",
        SyscallOutcome::Return(_) => "err",
        SyscallOutcome::Halt { .. } => "halt",
        SyscallOutcome::Exit { .. } => "exit",
        SyscallOutcome::Switched { .. } => "blocked",
    }
}

fn outcome_err(outcome: &SyscallOutcome) -> u64 {
    match outcome {
        SyscallOutcome::Return(r) => r.error,
        _ => 0,
    }
}

/// TAP **7..13** — bảng 16 syscall + nhóm cap/IPC/NB/WaitAddr/Clock/Yield trên `Machine` thật.
///
/// Trả `true` ⇔ mọi bất biến core còn xanh sau pha (TAP 14 đưa vào điều kiện).
fn kernel_sys_taps(rig: &SysRig, fails: &mut usize) -> bool {
    let mut env = TestEnv::new();
    let mut len = 0usize;
    let mut invariants = false;
    core_bridge::with_machine(|m| {
        // ---------------------------------------------------------------- TAP 7: 15 số hiệu
        m.set_addr_word(7, 64, 0x1234).expect("tap7: addr word");
        m.notification_signal(rig.ntfn, 0x1).expect("tap7: signal");
        let make_running_locked = |m: &mut Machine| make_running(m, rig.t0);
        let plan: [(u64, [u64; 6], &str); 15] = [
            (
                0,
                args6(rig.eps.slot as u64, mk_info(1), 0xaa, 0, 0, 0),
                "blocked",
            ),
            (
                1,
                args6(rig.eps.slot as u64, mk_info(0), 0, 0, 0, 0),
                "blocked",
            ),
            (
                2,
                args6(rig.eps.slot as u64, mk_info(1), 0x55, 0, 0, 0),
                "ok",
            ),
            (3, args6(rig.eps.slot as u64, REPLY_NONE, 0, 0, 0, 0), "ok"),
            (4, args6(rig.eps.slot as u64, 0, 0, 0, 0, 0), "err"),
            (
                5,
                args6(rig.rs.slot as u64, rig.eps.slot as u64, mk_info(0), 0, 0, 0),
                "blocked",
            ),
            (
                6,
                args6(
                    rig.eps.slot as u64,
                    rig.eps.slot as u64,
                    REPLY_NONE,
                    mk_info(1),
                    0x66,
                    0,
                ),
                "ok",
            ),
            (
                7,
                args6(rig.eps.slot as u64, rig.ns.slot as u64, mk_info(0), 0, 0, 0),
                "ok",
            ),
            (8, args6(0, 0, 0, 0, 0, 0), "ok"),
            (9, args6(0x4000_0000, 0x1234, 0, 0, 0, 0), "blocked"),
            (10, args6(0x4000_0000, 1, 0, 0, 0, 0), "ok"),
            (11, args6(0, 0, 0, 0, 0, 0), "ok"),
            (12, args6(0x4000_0000, 8, 0, 0, 0, 0), "ok"),
            (13, args6(5, 0, 0, 0, 0, 0), "halt"),
            (15, args6(0, 0, 0, 0, 0, 0), "ok"),
        ];
        make_running_locked(m);
        let mut ok7 = true;
        let mut calls7 = 0usize;
        for (nr, args, want) in plan {
            let outcome = vifast_core::syscall::dispatch(m, rig.t0, 0, nr, args, &mut env);
            let class = outcome_class(&outcome);
            let code = outcome_err(&outcome);
            let state = m.tcb_state(rig.t0).unwrap_or(TcbState::Dead);
            if class != want {
                ok7 = false;
            }
            if nr == 4 && code != 14 {
                ok7 = false;
            }
            if nr == 15 {
                match outcome {
                    SyscallOutcome::Return(r) => {
                        // D2-88(a): `num_harts` = số hart **nền tảng** (1/2/4 theo `-smp`).
                        if r.value != vifast_core::syscall::ABI_VERSION
                            || r.mr[2] != vifast_plat_riscv64::smp::hart_count() as u64
                            || r.mr[3] != 0
                        {
                            ok7 = false;
                        }
                    }
                    _ => ok7 = false,
                }
            }
            calls7 += 1;
            {
                let mut w = PlatformWriter(&PLATFORM);
                if nr == 15 {
                    let _ = writeln!(
                        w,
                        "# sys: tap=7 nr=15 out={} err={} tcb={} state={} src=core harts={}",
                        class,
                        code,
                        rig.t0,
                        tcb_state_name(state),
                        vifast_plat_riscv64::smp::hart_count()
                    );
                } else {
                    let _ = writeln!(
                        w,
                        "# sys: tap=7 nr={} out={} err={} tcb={} state={} src=core",
                        nr,
                        class,
                        code,
                        rig.t0,
                        tcb_state_name(state)
                    );
                }
            }
            make_running_locked(m);
        }
        ok7 &= calls7 == 15;
        // ---------------------------------------------------------------- TAP 8: cap sai
        let e_empty = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            1,
            args6(31, mk_info(0), 0, 0, 0, 0),
            &mut env,
        ));
        let e_tcb = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            1,
            args6(rig.t0s.slot as u64, mk_info(0), 0, 0, 0, 0),
            &mut env,
        ));
        // **KT-290**: `Call` trên cap TCB với `label = 1` (`SetFaultEp`, **chưa cài**) ⇒ 18.
        // Trước KT-290 ca này dùng `label = 0`; từ KT-290 `(Tcb, 0)` là `Tcb.Configure` **đã
        // cài** nên `label = 0` không còn là "method ngoài danh sách" (arity 0 ⇒ 5).
        let e_call = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            0,
            args6(rig.t0s.slot as u64, 1, 0, 0, 0, 0),
            &mut env,
        ));
        let e_recv = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            3,
            args6(rig.t0s.slot as u64, REPLY_NONE, 0, 0, 0, 0),
            &mut env,
        ));
        let ok8 = e_empty == 1 && e_tcb == 2 && e_call == 18 && e_recv == 2;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: tap=8 empty={} wrong_type={} call_nonep={} recv_wrong={} tcb={} state=Running",
                e_empty, e_tcb, e_call, e_recv, rig.t0
            );
        }
        make_running_locked(m);
        // ---------------------------------------------------------------- TAP 9: vòng IPC
        make_running(m, rig.t1);
        let _ = vifast_core::syscall::dispatch(
            m,
            rig.t1,
            0,
            3,
            args6(rig.eps.slot as u64, rig.rs.slot as u64, 0, 0, 0, 0),
            &mut env,
        );
        let call_out = vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            0,
            args6(rig.eps.slot as u64, mk_info(2), 0x11, 0x22, 0, 0),
            &mut env,
        );
        let block9 = matches!(
            call_out,
            SyscallOutcome::Switched {
                state: TcbState::BlockedOnReply { .. }
            }
        );
        let area = m.tcb_ipc_area(rig.t1).ok();
        let mr9 = area
            .and_then(|a| a.info.map(|_| (a.mr(0), a.mr(1))))
            .unwrap_or((None, None));
        make_running(m, rig.t1);
        let reply_out = vifast_core::syscall::dispatch(
            m,
            rig.t1,
            0,
            1,
            args6(rig.rs.slot as u64, mk_info(1), 0x33, 0, 0, 0),
            &mut env,
        );
        let reply_ok = !matches!(reply_out, SyscallOutcome::Return(ref r) if r.error != 0);
        let reply_clean = m.reply_caller(rig.reply).ok().flatten().is_none();
        let caller_ready = matches!(
            m.tcb_state(rig.t0),
            Ok(TcbState::Ready) | Ok(TcbState::Running { .. })
        );
        let ok9 =
            block9 && mr9 == (Some(0x11), Some(0x22)) && reply_ok && reply_clean && caller_ready;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: tap=9 call=1 recv=1 reply=1 mr=0x11/0x22 blocked={} reply_caller=0 caller={} state_ok={}",
                block9 as u8,
                tcb_state_name(m.tcb_state(rig.t0).unwrap_or(TcbState::Dead)),
                ok9 as u8
            );
        }
        // ---------------------------------------------------------------- TAP 10: nhóm NB
        make_running_locked(m);
        let nb_send = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            2,
            args6(rig.eps.slot as u64, mk_info(1), 0x55, 0, 0, 0),
            &mut env,
        ));
        let nb_get = vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            4,
            args6(rig.eps.slot as u64, 0, 0, 0, 0, 0),
            &mut env,
        );
        let nb_mr = match nb_get {
            SyscallOutcome::Return(r) => r.mr[0],
            _ => 0,
        };
        let nb_busy = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            4,
            args6(rig.eps.slot as u64, 0, 0, 0, 0, 0),
            &mut env,
        ));
        let nbsr = vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            6,
            args6(
                rig.eps.slot as u64,
                rig.eps.slot as u64,
                REPLY_NONE,
                mk_info(1),
                0x66,
                0,
            ),
            &mut env,
        );
        let nbsr_mr = match nbsr {
            SyscallOutcome::Return(r) => r.mr[0],
            _ => 0,
        };
        m.notification_signal(rig.ntfn, 0x2).expect("tap10: signal");
        let nbw = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            7,
            args6(rig.eps.slot as u64, rig.ns.slot as u64, mk_info(0), 0, 0, 0),
            &mut env,
        ));
        // Payload Notification (F1 reviewer-kt62): `word`/`badge` phải tới user cho cả 3 đường
        // 3 Recv, 4 NBRecv, 7 NBSendWait — không được trả `ipc_area` cũ (stale) hay `value=0`.
        let mut notif_ok = true;
        let mut notif_seen = [0u64; 3];
        for (index, (nr, badge, with_send)) in
            [(4u64, 153u64, false), (3, 171, false), (7, 205, true)]
                .iter()
                .enumerate()
        {
            m.notification_signal(rig.ntfn, *badge)
                .expect("tap10: notif");
            let a = if *with_send {
                args6(rig.eps.slot as u64, rig.ns.slot as u64, mk_info(0), 0, 0, 0)
            } else if *nr == 3 {
                args6(rig.ns.slot as u64, REPLY_NONE, 0, 0, 0, 0)
            } else {
                args6(rig.ns.slot as u64, 0, 0, 0, 0, 0)
            };
            match vifast_core::syscall::dispatch(m, rig.t0, 0, *nr, a, &mut env) {
                SyscallOutcome::Return(r) => {
                    if r.error != 0 || r.value != *badge || r.mr[0] != *badge || r.mr[1] != *badge {
                        notif_ok = false;
                    }
                    notif_seen[index] = r.value;
                }
                _ => notif_ok = false,
            }
            make_running_locked(m);
        }
        let ok10 = nb_send == 0
            && nb_mr == 0x55
            && nb_busy == 14
            && nbsr_mr == 0x66
            && nbw == 0
            && notif_ok;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: tap=10 nb_send={} nb_recv_mr=0x{:x} busy={} nbsr_mr=0x{:x} nbwait={} notif=0x{:x}/0x{:x}/0x{:x} ok={}",
                nb_send,
                nb_mr,
                nb_busy,
                nbsr_mr,
                nbw,
                notif_seen[0],
                notif_seen[1],
                notif_seen[2],
                ok10 as u8
            );
        }
        // ---------------------------------------------------------------- TAP 11: WaitAddr
        make_running_locked(m);
        let wait_out = vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            9,
            args6(0x4000_0000, 0x1234, 0, 0, 0, 0),
            &mut env,
        );
        let wait_block = matches!(
            wait_out,
            SyscallOutcome::Switched {
                state: TcbState::BlockedOnAddress { .. }
            }
        );
        let wake_out = vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            10,
            args6(0x4000_0000, 1, 0, 0, 0, 0),
            &mut env,
        );
        let woke = match wake_out {
            SyscallOutcome::Return(r) => r.value,
            _ => 0,
        };
        let after_wake = m.tcb_state(rig.t0) == Ok(TcbState::Ready);
        make_running_locked(m);
        let mismatch = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            9,
            args6(0x4000_0000, 0x9999, 0, 0, 0, 0),
            &mut env,
        ));
        env.addr = None;
        let notmapped = outcome_err(&vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            9,
            args6(0xdead_0000, 0, 0, 0, 0, 0),
            &mut env,
        ));
        env.addr = Some((7, 64));
        let ok11 = wait_block && woke == 1 && after_wake && mismatch == 5 && notmapped == 11;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: tap=11 wait={} wake={} mismatch={} notmapped={} state=Ready ok={}",
                wait_block as u8, woke, mismatch, notmapped, ok11 as u8
            );
        }
        // ---------------------------------------------------------------- TAP 12: ClockRead
        make_running_locked(m);
        let c1 = match vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            11,
            args6(0, 0, 0, 0, 0, 0),
            &mut env,
        ) {
            SyscallOutcome::Return(r) => r.value,
            _ => 0,
        };
        let c2 = match vifast_core::syscall::dispatch(
            m,
            rig.t0,
            0,
            11,
            args6(0, 0, 0, 0, 0, 0),
            &mut env,
        ) {
            SyscallOutcome::Return(r) => r.value,
            _ => 0,
        };
        let ok12 = c1 > 0 && c2 > c1;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: tap=12 nr=11 clock1={c1} clock2={c2} mono={}",
                ok12 as u8
            );
        }
        // ---------------------------------------------------------------- TAP 13: Yield đổi TCB
        make_running(m, rig.t1);
        make_running(m, rig.t0);
        let before13 = m.current(0);
        let yield_out =
            vifast_core::syscall::dispatch(m, rig.t0, 0, 8, args6(0, 0, 0, 0, 0, 0), &mut env);
        let after13 = m.current(0);
        let ok13 = before13 == Some(rig.t0)
            && after13 == Some(rig.t1)
            && matches!(yield_out, SyscallOutcome::Switched { .. })
            && m.tcb_state(rig.t0) == Ok(TcbState::Ready);
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: tap=13 nr=8 tcb_before={} tcb_after={} state_before=Running state_after={} ok={}",
                before13.unwrap_or(0),
                after13.unwrap_or(0),
                tcb_state_name(m.tcb_state(rig.t0).unwrap_or(TcbState::Dead)),
                ok13 as u8
            );
        }
        // ---------------------------------------------------------------- in TAP + bất biến
        let taps: [(bool, &str); 7] = [
            (ok7, "syscall 16 duong"),
            (ok8, "syscall sai cap"),
            (ok9, "syscall ipc vong"),
            (ok10, "syscall nb va notification"),
            (ok11, "syscall waitaddr wakeaddr"),
            (ok12, "syscall clock read"),
            (ok13, "syscall yield doi tcb"),
        ];
        for (index, (ok, name)) in taps.iter().enumerate() {
            let number = 7 + index;
            let mut w = PlatformWriter(&PLATFORM);
            if *ok {
                let _ = writeln!(w, "ok {number} - {name}");
            } else {
                let _ = writeln!(w, "not ok {number} - {name}: kiem sai");
                *fails += 1;
            }
            len += 1;
        }
        invariants = m.check_caps().is_ok()
            && m.check_all_bytes().is_ok()
            && m.check_sched().is_ok()
            && m.check_sc_refs().is_ok()
            && m.check_budget().is_ok()
            && m.check_mailboxes().is_ok()
            && m.check_ipc().is_ok()
            && m.check_reply_links().is_ok()
            && m.check_notifications().is_ok()
            && m.check_waitaddr().is_ok()
            && m.check_fault().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# sys: kernel phase invariants={} caps={} objects={}",
                invariants as u8,
                m.live_cap_count(),
                m.object_count()
            );
        }
    });
    invariants
}

/// Entry hart **thứ cấp** (KT-49): ảnh user chỉ cần symbol để link; không gọi `smp::init` nên
/// không hart thứ cấp nào chạy (nếu có, vào đây rồi park — không panic).
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
