// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test demo procsrv/shell** riscv64 (KT-65, **tách ảnh KT-66**) — bin riêng, feature
//! `ktest,user`, nạp qua OpenSBI.
//!
//! **Vì sao tách (KT-66, D2-87 §8.6):** KT-65 nhét demo vào `vifast-kbin-ktest-user` khiến ảnh đó
//! sát trần (239.216/262.144 B): 3 ELF U-mode + `proc_demo` + stack dispatch 32 KiB trong `.bss`.
//! Ảnh này giữ **đúng mười ca cũ 15..24** (đánh số lại **1..10**), trả ~40 KiB headroom cho
//! `ktest-user`; matrix **4 ảnh × smp 1/2/4** = 12 boot (trước KT-66: 9).
//!
//! TAP (10 test — ánh xạ cũ→mới ghi ở `v2/test/boot/expected.txt`):
//!
//! 1. `procsrv/shell/con load u-mode` — 3 ELF Rust riêng nạp bằng `vifast-elf` vào 3 VSpace
//!    (W^X, guard hở) từ Untyped U2 của pha demo.
//! 2. `procsrv recv fault exit` — con `Exit(42)`; core đưa fault `Exit{42}` vào `fault_ep`;
//!    procsrv `NBRecv` rồi kernel đọc `Tcb.last_fault` (D2-25/QD §1 Q-A), TCB con `Inactive`.
//! 3. `shell help echo` — **KT-73**: shell U-mode đọc **byte** từ ring UART (ngắt RX) và in
//!    `# shell: help cmds=6` (không còn message `LINE` do kernel cắm sẵn).
//! 4. `shell ps tu core` — kernel đọc `Machine` khi `PS_REQ`, shell định dạng/in; TAP so từng
//!    dòng `pid=… state=…` với snapshot (không bảng tĩnh).
//! 5. `shell wait ma thoat` — giao thức procsrv `wait(pid)` trả `code=42`.
//! 6. `procsrv kill treo tcb` — `kill(pid)` ⇒ kernel `Tcb.Suspend(Abort)`, `suspended` đọc từ
//!    `Machine` (chưa thu hồi toàn bộ tài nguyên — nợ §8.4).
//! 7. `shell exit va procsrv finalise` — shell `Exit(0)`, procsrv finalise.
//! 8. `u-mode kernel read fault` — U-mode đọc `0x80200000` ⇒ `scause=13`, `FaultKind::Vm`.
//! 9. `faultkind exit` — `Exit(42)` được ghi `FaultKind::Exit{42}` (D2-25/D2-56a).
//! 10. `faultkind wx from u` — in `ok 10` trước, rồi U-mode **thực thi** trang RW NX ⇒
//!     `scause=12`, `FaultKind::Vm{Execute, Wx}`; boot kết thúc bằng `PANIC: u-mode fault …`
//!     (chính sách fatal của K3, runner đòi đúng một dòng PANIC này).
//!
//! Ảnh cũng in **bằng chứng stack syscall per-hart** (KT-66):
//! `# stack: arena=1 … harts=N distinct=1 canary=0` + một dòng `# stack hart=K … low=… canary=0`
//! mỗi hart online (bằng chứng smp 2/4 **không** dùng chung stack).

use crate::proc_demo;
use core::fmt::Write;

use vifast_core::{Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::fault::{FaultAccess, FaultKind, VmFaultReason, FAULT_STATS};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::user::TaskState;
use vifast_plat_riscv64::user::{self, LoadedInit, USER_BASE, USER_STACK_GUARD_BOTTOM};
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// ELF64 `init` do `build.rs` sinh (`init/init_user.rs` + `.S` + `.ld`), nhúng vào ảnh test.
static INIT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/init.elf"));

/// Rig **tối thiểu** cho `init` (không cần TCB/Endpoint/Reply của pha syscall KT-62): cspace
/// 16 slot, Untyped ở slot 0, `SchedControl` slot 1, cap cspace slot 9, TCB/EP/SC `init`
/// slot 10/11/12.
struct Rig {
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    cspace: SlotRef,
    init_tcb_slot: SlotRef,
    init_ep_slot: SlotRef,
    init_sc_slot: SlotRef,
}

fn build_rig(m: &mut Machine, u: ObjId) -> Rig {
    let root = m.add_cnode(16);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("rig: cap Untyped");
    let ctl_cap = m
        .retype(us, ObjType::SchedControl, 0, 0)
        .expect("rig: SchedControl");
    m.put_cap(root, 1, ctl_cap).expect("rig: cap SchedControl");
    let cs = m.root_cap(root);
    m.put_cap(root, 9, cs).expect("rig: cap cspace");
    Rig {
        root,
        us,
        ctl: SlotRef::new(root, 1),
        cspace: SlotRef::new(root, 9),
        init_tcb_slot: SlotRef::new(root, 10),
        init_ep_slot: SlotRef::new(root, 11),
        init_sc_slot: SlotRef::new(root, 12),
    }
}

/// Tạo TCB `init` trên core: cspace + `fault_ep` + SC + priority, `Ready` rồi `dispatch(0)` ⇒
/// TCB **đang chạy** trên hart 0 (giống KT-62/H; demo cần `init` chạy thật để có `exit_kind_ok`).
fn build_init_task(m: &mut Machine, rig: &Rig) -> (ObjId, ObjId) {
    let tcb = m
        .retype(rig.us, ObjType::Tcb, 0, 0)
        .expect("init: retype TCB");
    let ep = m
        .retype(rig.us, ObjType::Endpoint, 0, 0)
        .expect("init: retype Endpoint");
    let sc = m
        .retype(rig.us, ObjType::SchedContext, 0, 0)
        .expect("init: retype SC");
    let (tcb_cap, ep_cap, sc_cap) = (tcb, ep, sc);
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

pub fn run() -> ! {
    PLATFORM.put_str("TAP version 13\n1..10\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu = mmu::init();
    let _tlb = vifast_plat_riscv64::tlb::init();
    let obj = vifast_plat_riscv64::obj::init();
    let core = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    // KT-66: stack syscall per-hart cấp từ arena **trước** `load_init` (Frame 2 MiB lấy trước
    // các Frame 4 KiB của ELF; không đụng `.data+.bss`).
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - procsrv/shell/con load u-mode: khong cap duoc stack\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    let (init_tcb, init_ep, cspace) = core_bridge::with_machine(|m| {
        let rig = build_rig(m, core.untyped);
        let (tcb, ep) = build_init_task(m, &rig);
        (tcb, ep, rig.root)
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
            cspace,
            state.map(tcb_state_name).unwrap_or("none"),
            (current == Some(init_tcb)) as u8
        );
    }
    let _ = (mmu, stvec, USER_BASE, USER_STACK_GUARD_BOTTOM);
    let loaded = match user::load_init(INIT_ELF, obj.untyped0) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - procsrv/shell/con load u-mode: init ELF that bai kind={} ({:?})",
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
            loaded.entry, loaded.segments, loaded.text_pages, loaded.ro_pages, loaded.data_pages,
            loaded.stack_pages, loaded.image_end, loaded.stack_guard, loaded.guard_unmapped as u8,
            loaded.wx_pages, loaded.flags_ok as u8, loaded.high_half_unmapped as u8, loaded.root,
            loaded.asid, loaded.check_ok as u8
        );
    }
    if loaded.wx_pages != 0 || !loaded.flags_ok || !loaded.high_half_unmapped || !loaded.check_ok {
        PLATFORM.put_str("not ok 1 - procsrv/shell/con load u-mode: plan init ELF sai\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    // `init` chạy một lượt thật (10 syscall, `Exit(42)`) — nguồn của `exit_kind_ok` + `loaded`
    // cho fault phase (TAP 8..10), đúng như KT-62/H.
    let snap = match user::enter(&loaded, 0) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - procsrv/shell/con load u-mode: init enter kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let exit_kind_ok = FAULT_STATS.last() == Some(FaultKind::Exit { code: 42 });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# init run: entry=0x{:x} u_mode=1 sret_spp={} ecall_spp={} entries={} syscalls={} exits={} exit={}",
            loaded.entry, snap.enter_spp, snap.ecall_spp, snap.entries, snap.syscalls, snap.exits,
            snap.exit_code
        );
        let _ = writeln!(
            w,
            "# user exit: code={} state={} exits={}",
            snap.exit_code,
            snap.state.name(),
            snap.exits
        );
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
    // KT-73: **PLIC + UART RX IRQ** cho đường "bàn phím → ring → shell". `plic::init` probe
    // `ndev` từ register, đặt threshold/priority/enable cho context của hart này; cổng external
    // chỉ mở trong từng cửa sổ bơm byte (`uart_console::inject_loopback`), không mở thường trực.
    let plic = vifast_plat_riscv64::plic::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# proc plic: base=0x{:x} ndev={} hart={} ctx={} threshold={} priority_uart={} \
             enable_ctx={} enable_mmode={} claim_idle={} const:uart_irq=10",
            plic.base,
            plic.ndev,
            plic.hart,
            plic.context,
            plic.threshold,
            plic.priority_uart,
            plic.enable_ctx as u8,
            plic.enable_mmode as u8,
            plic.claim_idle
        );
    }
    // Demo procsrv/shell/con (TAP 1..7) rồi continuation fault/wx (TAP 8..10, PANIC fatal).
    proc_demo::run(
        &PLATFORM,
        obj.untyped0,
        core.untyped,
        loaded,
        exit_kind_ok,
        fault_phase,
    );
}

/// **Continuation không trả về** sau pha demo KT-65: TAP 8..10 (fault U-mode KT-56) rồi
/// `PANIC: u-mode fault …` (chính sách fatal của K3). Tách hàm để `proc_demo::run` gọi thẳng
/// thay vì trả về qua khung gọi sâu (đã quan sát `ra=0` khi trả về sau nhiều lượt `user_enter`).
fn fault_phase(loaded: LoadedInit, exit_kind_ok: bool) -> ! {
    let mut fails = 0usize;
    // KT-66: bằng chứng stack syscall per-hart **sau** khi demo đã chạy nhiều lượt dispatch.
    user::print_stack_evidence();
    // ------------------------- 8) U-mode đọc địa chỉ kernel — trap THẬT, phân loại FaultKind
    // KT-56: lượt này **được phép fault** (`enter_expect_fault`) để kiểm phân loại rồi về S-mode;
    // chính sách mặc định của K3 (fault ⇒ log + halt) giữ nguyên cho lượt 7.
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# u-mode read: hart={} addr=0x80200000 expect=scause=0xd stval=0x80200000 recover=1",
            PLATFORM.hart_id()
        );
    }
    let fault_snap = match user::enter_expect_fault(&loaded, 1) {
        // Không được tới đây: đọc địa chỉ kernel mà không fault nghĩa là leaf kernel bị `U=1`.
        Ok(snap) => snap,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 8 - u-mode kernel read fault: enter loi kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let want5 = FaultKind::Vm {
        vspace: u64::from(loaded.asid),
        va: 0x8020_0000,
        access: FaultAccess::Read,
        reason: VmFaultReason::Permission,
    };
    let ok5 = fault_snap.state == TaskState::Faulted
        && fault_snap.fault_scause == 0xd
        && fault_snap.fault_stval == 0x8020_0000
        && FAULT_STATS.last() == Some(want5);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# faultkind: case=u-kernel-read scause=0x{:x} stval=0x{:x} recover=1 kind=",
            fault_snap.fault_scause, fault_snap.fault_stval
        );
        match FAULT_STATS.last() {
            Some(kind) => {
                let _ = write!(w, "{kind}");
            }
            None => {
                let _ = write!(w, "none");
            }
        }
        let _ = writeln!(w, " ok={}", u8::from(ok5));
    }
    if ok5 {
        PLATFORM.put_str("ok 8 - u-mode kernel read fault\n");
    } else {
        PLATFORM.put_str("not ok 8 - u-mode kernel read fault: FaultKind/scause sai\n");
        fails += 1;
    }

    // ------------------- 9) `Exit` mang `FaultKind::Exit` (D2-25/D2-56a) — qua syscall 14 thật
    if exit_kind_ok {
        PLATFORM.put_str("ok 9 - faultkind exit\n");
    } else {
        PLATFORM.put_str("not ok 9 - faultkind exit: khong ghi Exit{{42}}\n");
        fails += 1;
    }

    if fails > 0 {
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ------------------- 10) W^X **từ U-mode**: thực thi trang `.data` (RW NX) ⇒ fault thật
    // In `ok 10` TRƯỚC khi kích hoạt: boot kết thúc bằng `PANIC: u-mode fault scause=0xc …` +
    // SBI SRST (chính sách K3: log + halt), không có `# ktest halt`. Runner đòi đúng dòng PANIC
    // này và đúng `faultkind=…reason=Wx` trong dòng `# u-mode fault:`.
    PLATFORM.put_str("ok 10 - faultkind wx from u\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# u-mode wx: hart={} exec=data_page expect=scause=0xc reason=Wx fatal=1",
            PLATFORM.hart_id()
        );
    }
    match user::enter(&loaded, 2) {
        // Không được tới đây: thực thi trang RW NX mà không fault nghĩa là W^X bị phá.
        Ok(_) => {
            PLATFORM.put_str("not ok 10 - faultkind wx from u: khong fault\n");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 10 - faultkind wx from u: enter loi kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    }
}
