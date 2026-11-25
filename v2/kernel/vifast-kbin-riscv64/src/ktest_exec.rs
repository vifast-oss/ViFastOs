// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test `Tcb.SetTls` + `exec(path)`** riscv64 (KT-74) — bin riêng, feature `ktest,user,exec`,
//! nạp qua OpenSBI.
//!
//! # Vì sao tách ảnh
//!
//! D2-109/D2-111: mỗi ảnh test một ngân sách (262.144 B `.data+.bss`), thêm đường mới ⇒ **tách**,
//! không nới trần. Ảnh này tách khỏi `ktest-user`/`ktest-proc` để TAP/`.text` của chúng **không**
//! đổi (hồi quy matrix cũ vẫn so đúng ảnh cũ).
//!
//! # Sáu TAP (kế hoạch `1..6`)
//!
//! 1. `tls dat qua invocation` — chương trình U-mode **A** gọi `Call(cap TCB, MessageInfo{label=15,
//!    length=1}, MR0=tls)` ⇒ invocation `Tcb.SetTls` (method **15** của `[object].Tcb`, số sinh từ
//!    spec); kernel đọc lại `tcb_tls` **và** `x4`/`tp` quan sát **tại trap** (`# tls:`).
//! 2. `tls ca am` — method chưa cài (4) ⇒ 18; arity 0/2 ⇒ 5; cap Untyped + label 15 ⇒ 18; cap TCB
//!    **chỉ đọc** ⇒ 3; và **không** ghi được giá trị trong mọi ca âm.
//! 3. `exec giu pid cspace` — `exec("/b")`: `pid`/TCB/cspace **không** đổi, VSpace/entry **đổi**,
//!    `objects/frames/caps` **bằng nhau** (không rò).
//! 4. `exec chain 3 buoc` — B→C→D→A: **ba** lần `exec` liên tiếp, mỗi bước chạy **thật** (mã thoát
//!    riêng 3/4/1) rồi mới `exec` tiếp; `execs=3`, `recycled` suy ra (D2-106), bộ đếm bằng nhau.
//! 5. `exec that bai nguyen tu` — `path` lạ ⇒ `NotFound`; ELF hỏng ⇒ `Load/Elf`; tiến trình **cũ**
//!    nguyên vẹn (cùng VSpace/entry/bộ đếm, `execs` không tăng).
//! 6. `tls reset sau exec` — sau `exec` sang D (D **không** `SetTls`): `tcb_tls=0` **và** `tp` quan
//!    sát = 0; rồi `SetTls(0xB2)` + resume **không** exec ⇒ `tp` = 0xB2.
//!
//! Bằng chứng `x4`/`tp` là **phần cứng**: trap entry lưu đủ 32 GPR vào trapframe, nên giá trị
//! `tf.x[REG_TP]` tại `ecall` **là** `tp` của U-mode — không có `unsafe`, không có `mv ?, tp`
//! trong chương trình U-mode (giữ luật 0 `unsafe` của D2-93/D2-101).
//!
//! Boot kết thúc bằng **một** `PANIC` có chủ đích (khuôn runner KT-54).

use core::fmt::Write;

use vifast_core::syscall::SyscallEnv;
use vifast_core::{invoke, Machine, ObjId, ObjType, SlotRef};
use vifast_plat_common::obj::ObjCounts;
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::exec::{self, ImageTable};
use vifast_plat_riscv64::user::{self, LoadedInit, Stop};
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Bốn ảnh U-mode của chuỗi `exec` (KT-74) — mỗi bước một ELF **riêng** do `build.rs` sinh.
static ELF_A: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/exec_a.elf"));
static ELF_B: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/exec_b.elf"));
static ELF_C: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/exec_c.elf"));
static ELF_D: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/exec_d.elf"));

/// ELF **hỏng** cho ca "exec thất bại": 64 byte 0 — `vifast_elf::parse` từ chối magic ⇒
/// `LoadError::Elf`, **trước** khi cấp bất cứ Frame nào (đường nguyên tử rẻ nhất).
static ELF_BAD: [u8; 64] = [0u8; 64];

/// Giá trị TLS mà bước A đặt (hằng **đầu vào** của ca kiểm, khớp `exec_a_user.rs`).
const TLS_A: u64 = 0x7431_0001;
/// Giá trị TLS đặt **sau** `exec` trong ca 6 (bằng chứng reset ⇒ `SetTls` lại).
const TLS_AFTER: u64 = 0xb2;

/// Ô tác vụ (= `pid`) của tiến trình được `exec` — **một** tiến trình, giữ nguyên qua mọi bước.
const PID: usize = 0;

/// Rig tối thiểu: CNode 16 slot, cap TCB ở slot **1** (CPTR mà chương trình U-mode dùng),
/// cap cspace ở slot 9, cap Untyped ở slot 0, bản sao TCB **chỉ đọc** ở slot 11.
struct Rig {
    tcb: ObjId,
    root: ObjId,
    tcb_slot: SlotRef,
    ro_slot: SlotRef,
    untyped_slot: SlotRef,
}

fn build_rig(m: &mut Machine, u: ObjId) -> Rig {
    let root = m.add_cnode(16);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("rig: cap Untyped");
    let cs = m.root_cap(root);
    m.put_cap(root, 9, cs).expect("rig: cap cspace");
    let tcb_cap = m.retype(us, ObjType::Tcb, 0, 0).expect("rig: TCB");
    m.put_cap(root, 1, tcb_cap).expect("rig: cap TCB");
    m.tcb_configure(SlotRef::new(root, 1), Some(SlotRef::new(root, 9)))
        .expect("rig: cspace");
    // Ca quyền âm: bản sao TCB **chỉ đọc** (mint `RIGHT_READ`) ⇒ `SetTls` phải trả 3.
    m.mint(
        SlotRef::new(root, 1),
        SlotRef::new(root, 11),
        vifast_core::RIGHT_READ,
        None,
    )
    .expect("rig: TCB chi doc");
    let tcb = m.cap_obj(tcb_cap).expect("rig: obj TCB");
    Rig {
        tcb,
        root,
        tcb_slot: SlotRef::new(root, 1),
        ro_slot: SlotRef::new(root, 11),
        untyped_slot: SlotRef::new(root, 0),
    }
}

/// Hook arch tối thiểu cho ca âm **gọi thẳng** `invoke::dispatch` (method `Tcb.SetTls` không
/// dùng hook nào — D2-83 chỉ VM cần) — không cần U-mode cho các ca lỗi.
struct NoEnv;

impl SyscallEnv for NoEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(11)
    }
    fn now_ns(&mut self) -> u64 {
        0
    }
    fn features(&mut self) -> u64 {
        0
    }
    fn num_harts(&mut self) -> u64 {
        1
    }
    fn invoke(&mut self, _op: invoke::InvokeOp) -> Result<invoke::InvokeOutcome, u64> {
        Err(18)
    }
    fn release_object(&mut self, _key: u64) {}
    fn ipc_header(&mut self, _tcb: ObjId) -> Option<vifast_core::ipc::IpcHeader> {
        None
    }
}

/// `MessageInfo`: `length` ở bit 28.., `label` ở bit 0..19, `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

/// Gọi invocation trên cap `slot` với `label`/`length`/`mrs` — trả `(a0, a1)`.
fn invoke_on(
    m: &mut Machine,
    tcb: ObjId,
    slot: SlotRef,
    label: u32,
    len: u8,
    mrs: [u64; 4],
) -> Result<(u64, u64), u64> {
    invoke::dispatch(m, tcb, slot, info(label, len), &mrs, &mut NoEnv)
}

/// Chạy **một** lượt ảnh hiện tại tới khi `Exit` (đa nhiệm: `Exit` trả về kernel) và trả mã thoát.
///
/// `exec_frame` dựng trapframe **mới** (reset `sepc`/`sp`/thanh ghi, `tp` = TLS của TCB) — nên
/// đây cũng là bằng chứng "reset đúng" sau `exec`.
fn run_once(loaded: &LoadedInit) -> Option<u64> {
    user::set_active_slot(PID);
    let mut tf = user::exec_frame(loaded);
    match user::run_task(loaded, &mut tf) {
        Stop::Exit { code } => Some(code),
        _ => None,
    }
}

fn counts_line(w: &mut PlatformWriter<'_, Riscv64Virt>, c: ObjCounts) {
    let _ = write!(
        w,
        "objects={} frames={} caps={} vspaces={} page_tables={} mappings={}",
        c.objects, c.frames, c.caps, c.vspaces, c.page_tables, c.mappings
    );
}

pub fn run() -> ! {
    PLATFORM.put_str("TAP version 13\n1..6\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let _mmu = mmu_init();
    let _tlb = vifast_plat_riscv64::tlb::init();
    let obj = vifast_plat_riscv64::obj::init();
    let core = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - tls dat qua invocation: khong cap duoc stack\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    // Đa nhiệm bật để `Exit` **trả về kernel** (như `ktest-proc`), nên một bước `exec` chạy thật
    // rồi mới chuyển ảnh.
    user::enable_multitask(true);
    let rig = core_bridge::with_machine(|m| build_rig(m, core.untyped));
    user::bind_core_tcb(rig.tcb);
    let mut table = ImageTable::new();
    let added = table.add("/a", ELF_A)
        && table.add("/b", ELF_B)
        && table.add("/c", ELF_C)
        && table.add("/d", ELF_D)
        && table.add("/bad", &ELF_BAD);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# exec rig: stvec=0x{:x} untyped={} core_untyped={} images={} cspace={} tcb={} tcb_cptr={} tls_const=0x{:x}",
            stvec,
            obj.untyped0.0,
            core.untyped,
            table.len(),
            rig.root,
            rig.tcb,
            rig.tcb_slot.slot,
            TLS_A
        );
    }
    if !added {
        PLATFORM.put_str("not ok 1 - tls dat qua invocation: bang anh exec day/trung path\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    let mut fails = 0usize;

    // ------------------------------------------------ nạp ảnh đầu (A) — chưa phải `exec`
    let mut loaded = match user::load_init_slot(ELF_A, obj.untyped0, PID) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - tls dat qua invocation: nap A that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    user::set_active_slot(PID);
    let counts0 = exec::counts();

    // ------------------------------------------------ TAP 1: SetTls qua invocation từ U-mode
    // **Hai lượt** — đúng ngữ nghĩa "giá trị có hiệu lực ở lần resume **kế tiếp**" (cùng hợp đồng
    // với `Tcb.Configure` của core): lượt 1 A gọi `SetTls` (lúc đó `tp` vẫn là 0 vì đã nạp vào
    // thanh ghi **trước** khi `SetTls` chạy); lượt 2 resume ⇒ trapframe mới lấy `tp` **từ core**
    // ⇒ trap của A thấy `tp = TLS_A`. Đây là toàn bộ đường ABI: U-mode → syscall 0 → invocation →
    // core → trapframe → thanh ghi user.
    let exit_a = run_once(&loaded);
    let observed_first = user::tls_observed();
    let exit_a2 = run_once(&loaded);
    let tls_core = core_bridge::with_machine(|m| m.tcb_tls(rig.tcb).ok());
    let observed = user::tls_observed();
    let set_tls_stats = invoke::stats().set_tls;
    let sys0 = (user::sys_calls(0), user::sys_err(0), user::sys_val(0));
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tls: set=0x{:x} observed_first={} observed=0x{:x} method={} const:method_source=spec traps={} stats={} sys0_calls={} sys0_err={} exit={} exit2={} tcb=0x{:x} tcb_tls=0x{:x} ok={}",
            TLS_A,
            observed_first,
            observed,
            invoke::TCB_SET_TLS,
            user::tls_trap_count(),
            set_tls_stats,
            sys0.0,
            sys0.1,
            exit_a.unwrap_or(u64::MAX),
            exit_a2.unwrap_or(u64::MAX),
            rig.tcb,
            tls_core.unwrap_or(0),
            u8::from(
                exit_a == Some(1)
                    && exit_a2 == Some(1)
                    && tls_core == Some(TLS_A)
                    && observed_first == 0
                    && observed == TLS_A
                    && set_tls_stats >= 1
                    && sys0.1 == 0
            )
        );
    }
    if exit_a == Some(1)
        && exit_a2 == Some(1)
        && tls_core == Some(TLS_A)
        && observed_first == 0
        && observed == TLS_A
        && set_tls_stats >= 1
    {
        PLATFORM.put_str("ok 1 - tls dat qua invocation\n");
    } else {
        PLATFORM.put_str("not ok 1 - tls dat qua invocation\n");
        fails += 1;
    }

    // ------------------------------------------------ TAP 2: ca âm (method/arity/cap/quyền)
    let neg = core_bridge::with_machine(|m| {
        let method_bad = invoke_on(m, rig.tcb, rig.tcb_slot, 4, 1, [1, 0, 0, 0]);
        let arity0 = invoke_on(
            m,
            rig.tcb,
            rig.tcb_slot,
            invoke::TCB_SET_TLS,
            0,
            [1, 0, 0, 0],
        );
        let arity2 = invoke_on(
            m,
            rig.tcb,
            rig.tcb_slot,
            invoke::TCB_SET_TLS,
            2,
            [1, 0, 0, 0],
        );
        let cap_untyped = invoke_on(
            m,
            rig.tcb,
            rig.untyped_slot,
            invoke::TCB_SET_TLS,
            1,
            [1, 0, 0, 0],
        );
        let cap_ro = invoke_on(
            m,
            rig.tcb,
            rig.ro_slot,
            invoke::TCB_SET_TLS,
            1,
            [1, 0, 0, 0],
        );
        let tls_after = m.tcb_tls(rig.tcb).ok();
        (method_bad, arity0, arity2, cap_untyped, cap_ro, tls_after)
    });
    let code = |r: Result<(u64, u64), u64>| r.err().unwrap_or(0);
    let neg_ok = code(neg.0) == 18
        && code(neg.1) == 5
        && code(neg.2) == 5
        && code(neg.3) == 18
        && code(neg.4) == 3
        && neg.5 == Some(TLS_A);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tls neg: method_bad={} arity0={} arity2={} cap_untyped={} cap_ro={} tls_giu=0x{:x} set_tls_unchanged={} ok={}",
            code(neg.0),
            code(neg.1),
            code(neg.2),
            code(neg.3),
            code(neg.4),
            neg.5.unwrap_or(0),
            u8::from(neg.5 == Some(TLS_A)),
            u8::from(neg_ok)
        );
    }
    if neg_ok {
        PLATFORM.put_str("ok 2 - tls ca am\n");
    } else {
        PLATFORM.put_str("not ok 2 - tls ca am\n");
        fails += 1
    }

    // ------------------------------------------------ TAP 3: exec A→B giữ pid/cspace
    let cspace_before = core_bridge::with_machine(|m| m.tcb_info_obj(rig.tcb).ok());
    let step1 = core_bridge::with_machine(|m| {
        exec::exec(
            m,
            &table,
            &mut loaded,
            PID,
            "/b",
            obj.untyped0,
            rig.tcb_slot,
        )
    });
    let cspace_after = core_bridge::with_machine(|m| m.tcb_info_obj(rig.tcb).ok());
    let exit_b = run_once(&loaded);
    let cspace_same =
        cspace_before.as_ref().map(|i| i.cspace) == cspace_after.as_ref().map(|i| i.cspace);
    let r1 = match &step1 {
        Ok(report) => *report,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# exec: err={}", error.name());
            PLATFORM.put_str("not ok 3 - exec giu pid cspace\n");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    // `entry` của cả bốn ảnh **giống nhau** (`init.ld` link tại `USER_BASE`), nên "reset sepc"
    // được chứng minh bằng (a) VSpace **đổi** (map lại toàn bộ ảnh), (b) ảnh B **chạy thật** với
    // mã thoát riêng (2), và (c) ảnh mới **khác byte** với ảnh cũ (đo `len` từ `include_bytes`).
    // Ghi rõ `const:elf_linked_at_base=1` để người đọc không hiểu `entry` bằng nhau là lỗi.
    let img_a = table.find("/a").map(|i| i.len()).unwrap_or(0);
    let img_b = table.find("/b").map(|i| i.len()).unwrap_or(0);
    let img_differ = img_a != img_b && img_a > 0 && img_b > 0;
    let exec_ok = r1.no_leak()
        && cspace_same
        && r1.pid as usize == PID
        && r1.old_vspace != r1.new_vspace
        && img_differ
        && exit_b == Some(2);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# exec: step=1 pid={} tcb={} cspace_same={} old_vspace={} new_vspace={} vspace_changed={} old_entry=0x{:x} new_entry=0x{:x} const:elf_linked_at_base=1 img_differ={} img_a={} img_b={} freed={} new_pages={} exit={} ",
            r1.pid,
            rig.tcb,
            u8::from(cspace_same),
            r1.old_vspace,
            r1.new_vspace,
            u8::from(r1.old_vspace != r1.new_vspace),
            r1.old_entry,
            r1.new_entry,
            u8::from(img_differ),
            img_a,
            img_b,
            r1.freed_frames,
            r1.new_pages,
            exit_b.unwrap_or(u64::MAX)
        );
        counts_line(&mut w, r1.before);
        let _ = write!(w, " -> ");
        counts_line(&mut w, r1.after);
        let _ = writeln!(
            w,
            " same={} check={} ok={}",
            u8::from(r1.no_leak()),
            u8::from(r1.check_ok),
            u8::from(exec_ok)
        );
    }
    if exec_ok {
        PLATFORM.put_str("ok 3 - exec giu pid cspace\n");
    } else {
        PLATFORM.put_str("not ok 3 - exec giu pid cspace\n");
        fails += 1
    }

    // ------------------------------------------------ TAP 4: chuỗi 3 lần exec liên tiếp B→C→D→A
    let chain_start = exec::counts();
    let mut chain_execs = 0u32;
    let mut chain_recycled = 0usize;
    let mut chain_freed = 0usize;
    let mut chain_ok = true;
    let mut chain_exits = [0u64; 3];
    let mut chain_same = true;
    for (index, (path, want)) in [("/c", 3u64), ("/d", 4u64), ("/a", 1u64)]
        .iter()
        .enumerate()
    {
        let step = core_bridge::with_machine(|m| {
            exec::exec(
                m,
                &table,
                &mut loaded,
                PID,
                path,
                obj.untyped0,
                rig.tcb_slot,
            )
        });
        match step {
            Ok(report) => {
                chain_execs += 1;
                chain_recycled += report.recycled();
                chain_freed += report.freed_frames;
                chain_same &= report.no_leak();
                let got = run_once(&loaded);
                chain_exits[index] = got.unwrap_or(u64::MAX);
                chain_ok &= got == Some(*want) && report.pid as usize == PID;
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# exec chain step: i={} path={} pid={} tcb={} freed={} recycled={} new_vspace={} new_entry=0x{:x} exit={} frames={}->{} same={} check={}",
                    index + 1,
                    path,
                    report.pid,
                    rig.tcb,
                    report.freed_frames,
                    report.recycled(),
                    report.new_vspace,
                    report.new_entry,
                    got.unwrap_or(u64::MAX),
                    report.before.frames,
                    report.after.frames,
                    u8::from(report.no_leak()),
                    u8::from(report.check_ok)
                );
            }
            Err(error) => {
                chain_ok = false;
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# exec chain step: i={} path={} err={}",
                    index + 1,
                    path,
                    error.name()
                );
            }
        }
    }
    let chain_end = exec::counts();
    let chain_no_leak = chain_same
        && chain_start.objects == chain_end.objects
        && chain_start.frames == chain_end.frames
        && chain_start.caps == chain_end.caps;
    let chain_evidence_ok = chain_ok && chain_execs == 3 && chain_no_leak && chain_recycled > 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# exec chain: execs={} pid={} tcb={} exits={},{},{} recycled={} freed={} ",
            chain_execs,
            PID,
            rig.tcb,
            chain_exits[0],
            chain_exits[1],
            chain_exits[2],
            chain_recycled,
            chain_freed
        );
        counts_line(&mut w, chain_start);
        let _ = write!(w, " -> ");
        counts_line(&mut w, chain_end);
        let _ = writeln!(
            w,
            " frames_same={} objects_same={} caps_same={} total={} ok={}",
            u8::from(chain_start.frames == chain_end.frames),
            u8::from(chain_start.objects == chain_end.objects),
            u8::from(chain_start.caps == chain_end.caps),
            exec::exec_total(),
            u8::from(chain_evidence_ok)
        );
    }
    if chain_evidence_ok {
        PLATFORM.put_str("ok 4 - exec chain 3 buoc\n");
    } else {
        PLATFORM.put_str("not ok 4 - exec chain 3 buoc\n");
        fails += 1
    }

    // ------------------------------------------------ TAP 5: thất bại ⇒ tiến trình cũ nguyên vẹn
    let before_bad = exec::counts();
    let vspace_before = loaded.vspace;
    let entry_before = loaded.entry;
    let execs_before = exec::exec_total();
    let bad_path = core_bridge::with_machine(|m| {
        exec::exec(
            m,
            &table,
            &mut loaded,
            PID,
            "/khong-co",
            obj.untyped0,
            rig.tcb_slot,
        )
    })
    .err();
    let bad_elf = core_bridge::with_machine(|m| {
        exec::exec(
            m,
            &table,
            &mut loaded,
            PID,
            "/bad",
            obj.untyped0,
            rig.tcb_slot,
        )
    })
    .err();
    let after_bad = exec::counts();
    let atomic_ok = bad_path == Some(exec::ExecError::NotFound)
        && bad_elf
            == Some(exec::ExecError::Load(
                vifast_plat_riscv64::user::LoadError::Elf(
                    vifast_plat_riscv64::user::ElfParseError::BadMagic,
                ),
            ))
        && loaded.vspace == vspace_before
        && loaded.entry == entry_before
        && before_bad.objects == after_bad.objects
        && before_bad.frames == after_bad.frames
        && before_bad.caps == after_bad.caps
        && exec::exec_total() == execs_before;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# exec atomic: path_err={} elf_err={} vspace_same={} entry_same={} ",
            bad_path.map(|e| e.name()).unwrap_or("none"),
            bad_elf.map(|e| e.name()).unwrap_or("none"),
            u8::from(loaded.vspace == vspace_before),
            u8::from(loaded.entry == entry_before)
        );
        counts_line(&mut w, before_bad);
        let _ = write!(w, " -> ");
        counts_line(&mut w, after_bad);
        let _ = writeln!(
            w,
            " frames_same={} objects_same={} caps_same={} execs_total={} ok={}",
            u8::from(before_bad.frames == after_bad.frames),
            u8::from(before_bad.objects == after_bad.objects),
            u8::from(before_bad.caps == after_bad.caps),
            exec::exec_total(),
            u8::from(atomic_ok)
        );
    }
    if atomic_ok {
        PLATFORM.put_str("ok 5 - exec that bai nguyen tu\n");
    } else {
        PLATFORM.put_str("not ok 5 - exec that bai nguyen tu\n");
        fails += 1
    }

    // ------------------------------------------------ TAP 6: TLS reset sau exec + SetTls lại
    let to_d = core_bridge::with_machine(|m| {
        exec::exec(
            m,
            &table,
            &mut loaded,
            PID,
            "/d",
            obj.untyped0,
            rig.tcb_slot,
        )
    });
    let tls_after_exec = core_bridge::with_machine(|m| m.tcb_tls(rig.tcb).ok());
    let exit_d = run_once(&loaded);
    let observed_reset = user::tls_observed();
    // `SetTls` **không** qua invocation: đường core trực tiếp (đã kiểm invocation ở TAP 1) để ca
    // này cô lập đúng "TLS được nạp lại khi resume".
    let set_after = core_bridge::with_machine(|m| m.tcb_set_tls(rig.tcb_slot, TLS_AFTER).ok());
    let exit_d2 = run_once(&loaded);
    let observed_after = user::tls_observed();
    let tls_now = core_bridge::with_machine(|m| m.tcb_tls(rig.tcb).ok());
    let reset_ok = to_d.is_ok()
        && tls_after_exec == Some(0)
        && observed_reset == 0
        && exit_d == Some(4)
        && set_after.is_some()
        && exit_d2 == Some(4)
        && observed_after == TLS_AFTER
        && tls_now == Some(TLS_AFTER);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tls reset: after_exec=0x{:x} observed={} exit1={} set=0x{:x} observed2=0x{:x} exit2={} tcb_tls=0x{:x} execs_total={} ok={}",
            tls_after_exec.unwrap_or(u64::MAX),
            observed_reset,
            exit_d.unwrap_or(u64::MAX),
            TLS_AFTER,
            observed_after,
            exit_d2.unwrap_or(u64::MAX),
            tls_now.unwrap_or(0),
            exec::exec_total(),
            u8::from(reset_ok)
        );
    }
    if reset_ok {
        PLATFORM.put_str("ok 6 - tls reset sau exec\n");
    } else {
        PLATFORM.put_str("not ok 6 - tls reset sau exec\n");
        fails += 1
    }

    // Giải phóng ảnh cuối (giữ object layer sạch như các ảnh test khác) rồi kết thúc.
    loaded.release();
    let final_counts = exec::counts();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(w, "# exec final: c0 ");
        counts_line(&mut w, counts0);
        let _ = write!(w, " -> ");
        counts_line(&mut w, final_counts);
        let _ = writeln!(
            w,
            " releases_clean={} fail={} execs_total={}",
            // D6 (reviewer): `counts0` đo **sau** khi nạp ảnh A nên so với nó là `≤` yếu; phép đo
            // đúng là "sau `release()` **không còn** Frame/mapping nào" ⇒ so với **không**.
            u8::from(final_counts.frames == 0 && final_counts.mappings == 0),
            fails,
            exec::exec_total()
        );
    }
    if fails == 0 {
        PLATFORM.put_str("PANIC: KT-74 tls va exec xong (6 TAP, 0 fail)\n");
        PLATFORM.halt(0)
    } else {
        PLATFORM.put_str("PANIC: KT-74 tls va exec CO LOI\n");
        PLATFORM.halt(1)
    }
}

/// `mmu::init` (KT-46) — bọc để `# exec rig:` in `stvec`/satp **gián tiếp** (không đọc CSR trong
/// crate bin: `csrr` chỉ được phép ở `kernel/vifast-plat-*`, D2-17/R5).
fn mmu_init() -> vifast_plat_riscv64::mmu::MmuReport {
    vifast_plat_riscv64::mmu::init()
}
