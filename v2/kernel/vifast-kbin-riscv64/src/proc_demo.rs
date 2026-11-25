// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-65 — demo end-to-end trên QEMU: **procsrv + shell + tiến trình con** đều là chương trình
//! U-mode thật (ELF riêng nạp bằng `vifast-elf`), core `Machine` thật giữ TCB/cap/IPC.
//!
//! # Vai trò của kernel trong pha demo (không có scheduler U-mode trong core)
//!
//! 1. **Scheduler tuần tự**: `user::run_task` hạ `sret` xuống một tác vụ; `Yield`/`Exit` trả về
//!    S-mode; kernel chọn tác vụ kế (round-robin). Mỗi lượt có đúng một TCB `Running{hart:0}`
//!    theo bất biến core — chuyển trạng thái qua `transition_tcb`.
//! 2. **Chuyển tiếp fault (D2-25/QD §1 Q-A)**: khi con `Exit`, core đưa fault `Exit{code}` vào
//!    `fault_ep`; lượt `NBRecv` của procsrv làm core ghi `Tcb.last_fault` của procsrv và đưa TCB
//!    con về `Inactive`. ABI `Recv` hiện chỉ trả `MessageInfo`+MR (chưa có trường fault), nên
//!    kernel đọc `last_fault` rồi gửi một **message chuyển tiếp** vào `fault_ep` bằng đúng
//!    `NBSend` — procsrv nhận bằng `NBRecv` của mình (không thêm syscall; xem
//!    `docs/k35-procsrv.md` §2 ghi rõ đây là cầu nối của arch, nợ ABI).
//! 3. **UART RX IRQ → ring → shell (KT-73)**: kernel bơm byte của script vào **UART thật** qua
//!    loopback 16550 (`MCR` bit 4) trong cửa sổ ngắn; byte quay lại `RBR` sinh **ngắt RX**, handler
//!    đẩy vào ring KT-69, **shell U-mode** đọc byte từ ring và ghép dòng. Không còn message
//!    `LINE` do kernel cắm sẵn — đây là đường IRQ → ring → U-mode thật (nợ K3.5 đã đóng).
//! 4. **`ps`**: kernel đọc trạng thái TCB **từ `Machine`** khi shell gửi `PS_REQ`, trả bằng
//!    message; shell tự định dạng/in — không có bảng tĩnh nào.
//! 5. **`kill`**: procsrv quyết định theo giao thức; kernel thi hành `Tcb.Suspend(Abort)` khi
//!    nhận sự kiện `EV_KILL` của procsrv (chưa có syscall invocation TCB cho user — nợ D2-86).
//!
//! Mọi output `# proc:`/`# shell:` do **chương trình U-mode** in qua `DebugPutStr`; kernel
//! capture byte theo ô tác vụ (`user::captured_contains`) để TAP kiểm chữ thật, không tin lời.
// Module chỉ có trong ảnh test `vifast-kbin-ktest-user` (feature `ktest,user`).
#![allow(clippy::too_many_arguments)]

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_core::{
    FaultKind, Machine, Message, MessageInfo, NonReadyTarget, ObjId, ObjType, SlotRef, SuspendMode,
    TcbState,
};
use vifast_plat_common::obj::CapId;
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, LoadedInit, Stop, USER_BASE};
use vifast_plat_riscv64::{core_bridge, uart_console, Riscv64Virt};

// KT-77b: bố cục cspace/nhãn message **một nguồn** cho cả ảnh test và ảnh sản phẩm.
use crate::proc_abi::*;

/// ELF U-mode do `build.rs` sinh (KT-65).
static PROCSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/procsrv.elf"));
static SHELL_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.elf"));
static CHILD_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/child.elf"));

/// Chỉ số tác vụ trong mảng `tasks` của ảnh test (ánh xạ `proc_abi::SLOT_*`).
const T_PROCSRV: usize = 0;
const T_SHELL: usize = 1;
const T_CHILD: usize = 2;
const T_VICTIM: usize = 3;
const T_KERN: usize = 4;

/// Vùng RAM cho Untyped **U2** của riêng pha demo (nạp 3 VSpace + Frame). Nằm trên RAM 128 MiB,
/// ngoài mọi vùng kernel/object layer đã dùng; không đụng ngưỡng `.data+.bss` (đây là PA thật,
/// không phải section ảnh).
const DEMO_ARENA_BASE: u64 = 0x8100_0000;
const DEMO_ARENA_BYTES: u32 = 512 * 1024;
const DEMO_PA_BASE: u64 = 0x8120_0000;

/// Một tác vụ U-mode của demo.
struct Task {
    slot: usize,
    loaded: LoadedInit,
    tf: TrapFrame,
    started: bool,
    exited: bool,
}

/// Bộ nhớ/trạng thái kernel thu thập trong pha demo.
struct Demo {
    uart_idx: usize,
    fault_seen: usize,
    child_code: Option<u64>,
    child_state: Option<TcbState>,
    shell_exit_state: Option<TcbState>,
    shell_exit_code: Option<u64>,
    ps_replies: usize,
    ps_entries: usize,
    ps_words: [u64; 2],
    /// **KT-119**: trạng thái **thật** của từng TCB tại lúc `PS_REQ` — `(id, state.index(), suspended)`.
    /// Test TAP 4 dùng **nguồn này** để dựng dòng kỳ vọng, **không** giải mã lại `ps_words` (giải mã
    /// lại là dùng chung công thức với shell ⇒ đột biến chỗ đóng gói vẫn xanh; reviewer đã dựng lại).
    ps_real: [(u64, u8, bool); 4],
    ps_n: usize,
    wait_code: Option<u64>,
    wait_pid: Option<u64>,
    kill_code: Option<u64>,
    kill_pid: Option<u64>,
    victim_suspended: bool,
    kill_suspend_ok: bool,
    events: usize,
    /// Byte script đã bơm **qua UART thật** (kể cả `\n`) — đối chiếu `uart_console::stats().bytes`.
    uart_bytes: usize,
    /// Số dòng script đã bơm.
    uart_lines: usize,
    /// Mọi lần bơm đều qua loopback + ngắt RX thật + ring.
    uart_inject_ok: bool,
    procsrv_started: bool,
    shell_started: bool,
    child_started: bool,
}

impl Demo {
    fn new() -> Self {
        Self {
            uart_idx: 0,
            fault_seen: 0,
            child_code: None,
            child_state: None,
            shell_exit_state: None,
            shell_exit_code: None,
            ps_replies: 0,
            ps_entries: 0,
            ps_words: [0; 2],
            ps_real: [(0, 0, false); 4],
            ps_n: 0,
            wait_code: None,
            wait_pid: None,
            kill_code: None,
            kill_pid: None,
            victim_suspended: false,
            kill_suspend_ok: false,
            events: 0,
            uart_bytes: 0,
            uart_lines: 0,
            uart_inject_ok: true,
            procsrv_started: false,
            shell_started: false,
            child_started: false,
        }
    }
}

/// Cap/slot của rig core.
struct Rig {
    tcb: [SlotRef; 5],
    tcb_obj: [ObjId; 5],
    /// Object của endpoint theo **đúng** cspace slot 1..8 (fault, proc, uart, reply, ps, events).
    /// KT-73: đường `LINE` qua `uart_ep` đã bỏ (shell đọc byte từ ring) ⇒ driver demo không đọc
    /// `_ep` nữa, nhưng **vẫn giữ** endpoint trong cspace để bố cục slot của chương trình U-mode
    /// không đổi (EP_PROC/EP_REPLY/EP_PS/EP_EVENTS vẫn dùng).
    _ep: [ObjId; 6],
    ep_slot: [SlotRef; 6],
}

fn msg(label: u64, words: &[u64]) -> Message {
    let info =
        MessageInfo::new(label as u32, 0, false, 0, words.len() as u8).expect("KT-65: MessageInfo");
    Message::new(info, words).expect("KT-65: Message")
}

fn build_rig(untyped: ObjId) -> Rig {
    core_bridge::with_machine(|m| {
        let root = m.add_cnode(64);
        let us = SlotRef::new(root, 0);
        let uc = m.root_cap(untyped);
        m.put_cap(root, 0, uc).expect("KT-65: cap untyped");
        let mut ep = [0u32; 6];
        let ep_slots = [EP_FAULT, EP_PROC, EP_UART, EP_REPLY, EP_PS, EP_EVENTS];
        let mut ep_slot = [SlotRef::new(root, 0); 6];
        for (i, slot) in ep_slots.iter().enumerate() {
            let cap = m
                .retype(us, ObjType::Endpoint, 0, 0)
                .expect("KT-65: retype endpoint");
            m.put_cap(root, *slot, cap).expect("KT-65: put endpoint");
            ep[i] = m.cap_obj(cap).expect("KT-65: endpoint obj");
            ep_slot[i] = SlotRef::new(root, *slot);
        }
        // Cap CSpace của chính CNode (để cấu hình TCB) + SchedControl.
        let cs = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cs).expect("KT-65: cspace cap");
        let ctl_cap = m
            .retype(us, ObjType::SchedControl, 0, 0)
            .expect("KT-65: retype SchedControl");
        m.put_cap(root, EP_CTL, ctl_cap).expect("KT-65: cap ctl");
        let ctl = SlotRef::new(root, EP_CTL);
        let cspace = SlotRef::new(root, SLOT_CSPACE);
        let mut tcb = [SlotRef::new(root, 0); 5];
        let mut tcb_obj = [0u32; 5];
        let mut sc = [SlotRef::new(root, 0); 5];
        for i in 0..5 {
            let tcap = m.retype(us, ObjType::Tcb, 0, 0).expect("KT-65: retype TCB");
            m.put_cap(root, TCB_SLOTS[i], tcap).expect("KT-65: put TCB");
            let scap = m
                .retype(us, ObjType::SchedContext, 0, 0)
                .expect("KT-65: retype SC");
            m.put_cap(root, SC_SLOTS[i], scap).expect("KT-65: put SC");
            tcb[i] = SlotRef::new(root, TCB_SLOTS[i]);
            tcb_obj[i] = m.cap_obj(tcap).expect("KT-65: tcb obj");
            sc[i] = SlotRef::new(root, SC_SLOTS[i]);
        }
        for i in 0..5 {
            m.tcb_configure(tcb[i], Some(cspace))
                .expect("KT-65: tcb cspace");
            if i < 4 {
                m.tcb_set_fault_ep(tcb[i], ep_slot[0])
                    .expect("KT-65: fault_ep");
            }
            m.sched_control_configure(ctl, sc[i], 1_000, 10_000)
                .expect("KT-65: SC");
            m.set_sched_params(tcb[i], Some(sc[i]))
                .expect("KT-65: sched params");
            m.tcb_set_priority(tcb[i], 5, 10).expect("KT-65: priority");
            m.start_tcb(tcb_obj[i]).expect("KT-65: Ready");
        }
        Rig {
            tcb,
            tcb_obj,
            _ep: ep,
            ep_slot,
        }
    })
}

fn set_line(lines: &mut [([u8; 24], usize); 6], idx: &mut usize, text: &[u8]) {
    if *idx >= lines.len() {
        return;
    }
    let n = text.len().min(24);
    lines[*idx].0[..n].copy_from_slice(&text[..n]);
    lines[*idx].1 = n;
    *idx += 1;
}

fn decimal(out: &mut [u8], mut value: u64) -> usize {
    let mut tmp = [0u8; 20];
    let mut i = tmp.len();
    if value == 0 {
        i -= 1;
        tmp[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        tmp[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    let n = tmp.len() - i;
    out[..n].copy_from_slice(&tmp[i..]);
    n
}

/// Xây script UART **theo pid thật** của con/victim (pid là `ObjId` của core).
fn build_script(rig: &Rig) -> ([([u8; 24], usize); 6], usize) {
    let mut lines = [([0u8; 24], 0); 6];
    let mut idx = 0usize;
    set_line(&mut lines, &mut idx, b"help");
    set_line(&mut lines, &mut idx, b"echo shell-that");
    set_line(&mut lines, &mut idx, b"ps");
    let mut wait = [0u8; 24];
    wait[..5].copy_from_slice(b"wait ");
    let n = decimal(&mut wait[5..], u64::from(rig.tcb_obj[T_CHILD]));
    set_line(&mut lines, &mut idx, &wait[..5 + n]);
    let mut kill = [0u8; 24];
    kill[..5].copy_from_slice(b"kill ");
    let n = decimal(&mut kill[5..], u64::from(rig.tcb_obj[T_VICTIM]));
    set_line(&mut lines, &mut idx, &kill[..5 + n]);
    set_line(&mut lines, &mut idx, b"exit");
    (lines, idx)
}

/// Dịch vụ kernel giữa hai lượt U-mode (một lần khoá `Machine`).
fn service(m: &mut Machine, rig: &Rig, demo: &mut Demo, script: &[([u8; 24], usize)]) {
    // 1) Sự kiện từ procsrv (kênh IPC `events_ep`).
    while let Ok(Some(message)) = m.nb_recv(rig.tcb_obj[T_KERN], rig.ep_slot[5]) {
        demo.events += 1;
        let label = u64::from(message.info.label());
        match label {
            EV_UP => {
                demo.procsrv_started = true;
            }
            EV_FAULT => {
                demo.fault_seen += 1;
            }
            EV_WAIT => {
                demo.wait_pid = Some(message.mr[0]);
                demo.wait_code = Some(message.mr[1]);
            }
            EV_KILL => {
                demo.kill_pid = Some(message.mr[0]);
                demo.kill_code = Some(message.mr[1]);
                let suspend = m.tcb_suspend(rig.tcb[T_VICTIM], SuspendMode::Abort);
                demo.kill_suspend_ok = suspend.is_ok();
                demo.victim_suspended = suspend.is_ok()
                    && m.tcb_info_obj(rig.tcb_obj[T_VICTIM])
                        .map(|info| info.suspended && info.state == TcbState::Ready)
                        .unwrap_or(false);
            }
            EV_PS => {
                demo.ps_entries = message.mr[0] as usize;
            }
            EV_ECHO | EV_HELP => {}
            _ => {}
        }
    }
    // 2) Fault `Exit` vừa được procsrv `NBRecv` thụ lý (core đã ghi `last_fault` + `Inactive`).
    if let Ok(Some((pid, kind))) = m.take_last_fault(rig.tcb_obj[T_PROCSRV]) {
        let state = m.tcb_state(pid).ok();
        if let FaultKind::Exit { code } = kind {
            let words = [u64::from(pid), code, 0, 0];
            let _ = m.nb_send(rig.tcb_obj[T_KERN], rig.ep_slot[0], msg(FAULT_EXIT, &words));
            if pid == rig.tcb_obj[T_CHILD] {
                demo.child_code = Some(code);
                demo.child_state = state;
            } else if pid == rig.tcb_obj[T_SHELL] {
                demo.shell_exit_code = Some(code);
                demo.shell_exit_state = state;
            }
        }
    }
    // 3) `ps`: shell yêu cầu, kernel đọc `Machine` và trả message (shell tự in).
    while let Ok(Some(request)) = m.nb_recv(rig.tcb_obj[T_KERN], rig.ep_slot[4]) {
        if u64::from(request.info.label()) == PS_REQ {
            let ids = [
                rig.tcb_obj[T_PROCSRV],
                rig.tcb_obj[T_SHELL],
                rig.tcb_obj[T_CHILD],
                rig.tcb_obj[T_VICTIM],
            ];
            let mut entries = [0u64; 4];
            let mut n = 0usize;
            for id in ids {
                let Ok(state) = m.tcb_state(id) else { continue };
                let suspended = m
                    .tcb_info_obj(id)
                    .map(|info| info.suspended)
                    .unwrap_or(false);
                let entry =
                    (u64::from(id) << 5) | ((state.index() as u64) << 1) | u64::from(suspended);
                if n < entries.len() {
                    entries[n] = entry;
                    demo.ps_real[n] = (u64::from(id), state.index() as u8, suspended);
                    n += 1;
                }
            }
            let words = [
                entries[0] | (entries[1] << 16),
                entries[2] | (entries[3] << 16),
            ];
            let _ = m.nb_send(rig.tcb_obj[T_KERN], rig.ep_slot[3], msg(PS_REPLY, &words));
            demo.ps_words = words;
            demo.ps_n = n;
            demo.ps_replies += 1;
        }
    }
    // 4) **KT-73** UART thật → ring → shell: bơm **một dòng** script mỗi lượt (giữ thứ tự) khi
    // ring đang **rỗng** (shell đã tiêu thụ dòng trước), qua loopback 16550 + ngắt RX thật.
    let shell_alive = m
        .tcb_state(rig.tcb_obj[T_SHELL])
        .map(|state| state.is_live())
        .unwrap_or(false);
    let (_, _, ring_used, _) = uart_console::ring_stats();
    if shell_alive && ring_used == 0 && demo.uart_idx < script.len() {
        let (bytes, len) = script[demo.uart_idx];
        let mut line = [0u8; 25];
        line[..len].copy_from_slice(&bytes[..len]);
        line[len] = b'\n';
        let (ok, _chunks, _ticks) = uart_console::inject_loopback(&line[..len + 1]);
        demo.uart_inject_ok &= ok;
        demo.uart_bytes += len + 1;
        demo.uart_lines += 1;
        demo.uart_idx += 1;
    }
}

/// Trạng thái điều phối của demo để **không** nằm trong khung stack bị ghi đè (chẩn đoán KT-65:
/// biến cục bộ `steps`/`tcb_index` từng bị zero sau lượt U-mode đầu của shell — xem
/// `docs/k35-procsrv.md` §6). Dùng static để vòng lặp có nguồn sự thật ngoài stack.
static STEP_NO: AtomicU64 = AtomicU64::new(0);
static TURN_NO: AtomicU64 = AtomicU64::new(0);
static CUR_TASK: AtomicU64 = AtomicU64::new(0);

fn task_running(m: &mut Machine, tcb: ObjId) -> bool {
    m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
        .is_ok()
}

fn task_ready(m: &mut Machine, tcb: ObjId) {
    // **D2-187/D2-194** — không còn cửa "đặt `Ready` vô điều kiện": chọn cửa khớp trạng thái
    // nguồn. `wake` là API **test-only** (D2-144) và đúng ngữ nghĩa cho `BlockedOn*`/`Sleeping`.
    let _ = match m.tcb_state(tcb) {
        Ok(TcbState::Inactive) => m.start_tcb(tcb),
        Ok(TcbState::Ready) => Ok(()),
        Ok(TcbState::Running { .. }) => m.tcb_yield(tcb),
        _ => m.wake(tcb),
    };
}

/// Đệm nhỏ dựng dòng `pid=… state=…` để đối chiếu output shell (không cấp phát).
struct FixedBuf {
    b: [u8; 64],
    n: usize,
}

impl FixedBuf {
    fn new() -> Self {
        Self { b: [0; 64], n: 0 }
    }
    fn push(&mut self, t: &[u8]) {
        for byte in t {
            if self.n < self.b.len() {
                self.b[self.n] = *byte;
                self.n += 1;
            }
        }
    }
    fn num(&mut self, mut v: u64) {
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        if v == 0 {
            i -= 1;
            tmp[i] = b'0';
        }
        while v > 0 {
            i -= 1;
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        self.push(&tmp[i..]);
    }
    fn as_bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

/// Tên trạng thái theo **chỉ số `TcbState::index()`** — khớp `state_name` của shell.
fn state_label(code: u8) -> &'static [u8] {
    match code {
        0 => b"Inactive",
        1 => b"Ready",
        2 => b"Running",
        3 => b"BlockedOnSend",
        4 => b"BlockedOnRecv",
        5 => b"BlockedOnReply",
        6 => b"BlockedOnNotification",
        7 => b"BlockedOnAddress",
        8 => b"Sleeping",
        9 => b"Stopping",
        10 => b"Dead",
        _ => b"Unknown",
    }
}

fn tcb_name(state: TcbState) -> &'static str {
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

/// Chạy pha demo, in TAP 1..7 rồi giao quyền cho `cont` (`-> !`). Ảnh test kết thúc bằng
/// continuation (TAP 22..24 + PANIC fatal của KT-56) nên **không quay lại khung gọi** — tránh
/// đường trả về của khung `rust_main` sau nhiều lượt `user_enter` (đã thấy `ra=0` khi trả về).
pub fn run(
    platform: &Riscv64Virt,
    load_untyped: CapId,
    core_untyped: ObjId,
    init_loaded: LoadedInit,
    init_exit_kind_ok: bool,
    cont: fn(LoadedInit, bool) -> !,
) -> ! {
    // --- nạp 3 chương trình U-mode vào 3 VSpace thật (Untyped U2 của pha demo).
    // U2 **của object layer** cấp Frame/PageTable cho 3 VSpace; core rig dùng Untyped riêng
    // (`core_untyped`) như KT-61/KT-62.
    let u2 = aobj::machine()
        .add_untyped(DEMO_ARENA_BASE, DEMO_ARENA_BYTES, DEMO_PA_BASE)
        .expect("KT-65: add_untyped U2");
    // KT-73: Frame ring UART (4 KiB) từ cùng Untyped; header/layout là của `vifast_ring` (KT-69).
    let con_ring = match uart_console::init_ring(u2) {
        Ok(report) => report,
        Err(stage) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "not ok 1 - procsrv/shell/con load u-mode: ring stage={stage}"
            );
            platform.put_str("# ktest halt\n");
            platform.halt(1)
        }
    };
    let procsrv = match user::load_init_slot(PROCSRV_ELF, load_untyped, SLOT_PROCSRV) {
        Ok(loaded) => loaded,
        Err(e) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(w, "not ok 1 - procsrv/shell/con load: {} {:?}", e.name(), e);
            platform.put_str("# ktest halt\n");
            platform.halt(1);
        }
    };
    let shell = match user::load_init_slot(SHELL_ELF, load_untyped, SLOT_SHELL) {
        Ok(loaded) => loaded,
        Err(e) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "not ok 1 - procsrv/shell/con load: shell {} {:?}",
                e.name(),
                e
            );
            platform.put_str("# ktest halt\n");
            platform.halt(1);
        }
    };
    // **KT-401** — `child.elf` mang fixture 27 KiB (bằng chứng biên ELF) ⇒ cấp **1** trang stack
    // thay vì 8 (`KT-157` đã dùng 1 trang cho client): tiến trình con chỉ `put`+`exit`, và
    // arena object 96 ô của ảnh test không đủ nếu giữ 8 trang (đo được: `Obj(NoSlots)`).
    let child = match user::load_init_slot_with_stack(CHILD_ELF, load_untyped, SLOT_CHILD, 1) {
        Ok(loaded) => loaded,
        Err(e) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "not ok 1 - procsrv/shell/con load: child {} {:?}",
                e.name(),
                e
            );
            platform.put_str("# ktest halt\n");
            platform.halt(1);
        }
    };
    // **KT-401** — bằng chứng biên ELF user riscv64: ảnh `child.elf` mang fixture 32 KiB
    // (xem `init/child_user.rs`) ⇒ `image_end` **vượt** cửa sổ cũ: guard cũ `0x4000_7000`
    // (ảnh chỉ ~28 KiB) và `MAX_ELF_BYTES` cũ 24 KiB. Nạp được với guard mới `0x400f_7000`
    // + `MAX_INIT_PAGES = 32`; đây là điều kiện **bắt buộc** của TAP 1 (`child_big=1`).
    const OLD_GUARD_BOTTOM: u64 = 0x4000_7000;
    const OLD_ELF_BYTES: u64 = 24 * 1024;
    let child_big =
        child.image_end > OLD_GUARD_BOTTOM && child.image_end - USER_BASE > OLD_ELF_BYTES;
    let load_ok = child_big
        && procsrv.check_ok
        && procsrv.wx_pages == 0
        && procsrv.flags_ok
        && procsrv.high_half_unmapped
        && shell.check_ok
        && shell.wx_pages == 0
        && shell.flags_ok
        && shell.high_half_unmapped
        && child.check_ok
        && child.wx_pages == 0
        && child.flags_ok
        && child.high_half_unmapped;
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# proc: load procsrv=[0x{:x},0x{:x}) asid={} segs={} stack={} ok={} shell=[0x{:x},0x{:x}) asid={} segs={} stack={} ok={} child=[0x{:x},0x{:x}) asid={} segs={} stack={} ok={} child_big={}",
            procsrv.entry,
            procsrv.image_end,
            procsrv.asid,
            procsrv.segments,
            procsrv.stack_pages,
            procsrv.check_ok as u8,
            shell.entry,
            shell.image_end,
            shell.asid,
            shell.segments,
            shell.stack_pages,
            shell.check_ok as u8,
            child.entry,
            child.image_end,
            child.asid,
            child.segments,
            child.stack_pages,
            child.check_ok as u8,
            child_big as u8,
        );
    }

    // KT-73: map Frame ring UART vào VSpace **của shell** — shell đọc byte từ đây (kernel làm
    // producer qua ngắt RX; shell không có invocation `Frame.Map` cho cap lạ nên kernel map hộ).
    let shell_ring = uart_console::attach(shell.vspace);
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# proc ring: pa=0x{:x} phys=0x{:x} va=0x{:x} const:slots={} const:slot_bytes={} \
             head={} tail={} check={} mapped_shell={} const:capacity={}",
            con_ring.pa,
            con_ring.phys,
            con_ring.va,
            con_ring.slots,
            con_ring.slot_bytes,
            con_ring.head,
            con_ring.tail,
            con_ring.check as u8,
            shell_ring as u8,
            vifast_virtio::conchan::channel_bytes()
        );
    }
    // --- rig core: cspace dùng chung + 6 endpoint + 5 TCB + 5 SC + SchedControl.
    let rig = build_rig(core_untyped);
    let (lines, n_lines) = build_script(&rig);
    let mut demo = Demo::new();
    let mut tasks = [
        Task {
            slot: SLOT_PROCSRV,
            loaded: procsrv,
            tf: TrapFrame::zeroed(),
            started: false,
            exited: false,
        },
        Task {
            slot: SLOT_SHELL,
            loaded: shell,
            tf: TrapFrame::zeroed(),
            started: false,
            exited: false,
        },
        Task {
            slot: SLOT_CHILD,
            loaded: child,
            tf: TrapFrame::zeroed(),
            started: false,
            exited: false,
        },
    ];
    for task in tasks.iter_mut() {
        task.tf.x[REG_SP] = user::USER_STACK_TOP;
        task.tf.sepc = task.loaded.entry;
    }
    let mut script = [([0u8; 24], 0usize); 6];
    script[..n_lines].copy_from_slice(&lines[..n_lines]);

    user::enable_multitask(true);
    user::capture_begin();
    const ORDER: [usize; 3] = [T_PROCSRV, T_SHELL, T_CHILD];
    STEP_NO.store(0, Ordering::Relaxed);
    TURN_NO.store(0, Ordering::Relaxed);
    let mut running = true;
    while running && STEP_NO.load(Ordering::Relaxed) < 240 {
        let _ = STEP_NO.fetch_add(1, Ordering::Relaxed) + 1;
        let turn = TURN_NO.fetch_add(1, Ordering::Relaxed);
        CUR_TASK.store(
            ORDER[(turn as usize) % ORDER.len()] as u64,
            Ordering::Relaxed,
        );
        let tcb_index = CUR_TASK.load(Ordering::Relaxed) as usize;
        if tasks[tcb_index].exited {
            continue;
        }
        user::set_active_slot(tasks[CUR_TASK.load(Ordering::Relaxed) as usize].slot);
        user::bind_core_tcb(rig.tcb_obj[CUR_TASK.load(Ordering::Relaxed) as usize]);
        let entered = core_bridge::with_machine(|m| {
            task_running(m, rig.tcb_obj[CUR_TASK.load(Ordering::Relaxed) as usize])
        });
        if !entered {
            break;
        }
        tasks[CUR_TASK.load(Ordering::Relaxed) as usize].started = true;
        let stop = user::run_task(
            &tasks[CUR_TASK.load(Ordering::Relaxed) as usize].loaded,
            &mut tasks[CUR_TASK.load(Ordering::Relaxed) as usize].tf,
        );
        let saved = user::saved_tf();
        tasks[CUR_TASK.load(Ordering::Relaxed) as usize].tf = saved;
        match stop {
            Stop::Yield => {
                core_bridge::with_machine(|m| {
                    task_ready(m, rig.tcb_obj[CUR_TASK.load(Ordering::Relaxed) as usize])
                });
            }
            Stop::Exit { .. } => {
                tasks[tcb_index].exited = true;
                if tcb_index == T_CHILD {
                    demo.child_started = true;
                }
            }
            Stop::Fault => {
                // KT-70: pha proc không bật fault→pager ⇒ fault VM là lỗi (dừng tác vụ).
                tasks[tcb_index].exited = true;
            }
            Stop::Blocked(_) | Stop::Halt { .. } => {
                tasks[tcb_index].exited = true;
            }
        }
        if tasks[T_PROCSRV].started {
            demo.procsrv_started =
                demo.procsrv_started || user::captured_contains(SLOT_PROCSRV, b"# proc: up");
        }
        if tasks[T_SHELL].started {
            demo.shell_started =
                demo.shell_started || user::captured_contains(SLOT_SHELL, b"# shell: up");
        }
        if tasks[T_CHILD].started {
            demo.child_started = true;
        }
        core_bridge::with_machine(|m| service(m, &rig, &mut demo, &script));
        // Điều kiện dừng: shell đã Exit, procsrv đã nhận **cả** Exit của con và của shell.
        let shell_state = core_bridge::with_machine(|m| m.tcb_state(rig.tcb_obj[T_SHELL]).ok());
        running = !(tasks[T_SHELL].exited
            && demo.fault_seen >= 2
            && demo.child_code == Some(42)
            && demo.shell_exit_code == Some(0)
            && shell_state == Some(TcbState::Inactive));
        // Không còn tác vụ nào chạy được mà chưa xong ⇒ thoát để TAP đỏ có chẩn đoán.
        if tasks.iter().all(|task| task.exited) && running {
            break;
        }
    }
    user::enable_multitask(false);
    user::capture_end();

    // ---------------------------------------------------------------- TAP 1..7
    let mut ok_all = true;
    let mut w = PlatformWriter(platform);
    // 15 — nạp + chạy U-mode thật cả ba chương trình.
    let ok15 =
        load_ok && tasks[T_PROCSRV].started && tasks[T_SHELL].started && tasks[T_CHILD].started;
    if ok15 {
        let _ = writeln!(w, "ok 1 - procsrv/shell/con load u-mode");
    } else {
        let _ = writeln!(
            w,
            "not ok 1 - procsrv/shell/con load u-mode: load/start sai"
        );
        ok_all = false;
    }
    // 16 — procsrv nhận Exit của con qua core; TCB con `Inactive`.
    let ok16 = demo.child_code == Some(42)
        && demo.child_state == Some(TcbState::Inactive)
        && demo.fault_seen >= 1
        && user::captured_contains(SLOT_PROCSRV, b"# proc: recv exit pid=")
        && user::captured_contains(SLOT_PROCSRV, b" code=42");
    {
        let _ = writeln!(
            w,
            "# proc: exit recv code={} state={} faults={} captured={}",
            demo.child_code.map(|c| c as i64).unwrap_or(-1),
            demo.child_state.map(tcb_name).unwrap_or("none"),
            demo.fault_seen,
            user::captured_contains(SLOT_PROCSRV, b"# proc: recv exit pid=") as u8
        );
    }
    if ok16 {
        let _ = writeln!(w, "ok 2 - procsrv recv fault exit");
    } else {
        let _ = writeln!(w, "not ok 2 - procsrv recv fault exit: core/chuoi sai");
        ok_all = false;
    }
    // 17 — help + echo do shell U-mode in, **và** byte tới shell qua đúng đường UART thật:
    // ngắt RX (PLIC) → handler → ring KT-69 → U-mode đọc. Điều kiện đo được: số byte vào ring
    // bằng số byte script đã bơm, có ngắt, không rơi slot, ring đã được tiêu thụ hết.
    let con_stats = uart_console::stats();
    let (ring_head, ring_tail, ring_used, _) = uart_console::ring_stats();
    let ok_uart_path = demo.uart_inject_ok
        && demo.uart_bytes > 0
        && con_stats.irqs > 0
        && con_stats.bytes == demo.uart_bytes as u64
        && con_stats.drops == 0
        && con_stats.timeouts == 0
        && ring_used == 0
        && ring_tail == ring_head;
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# con uart: irqs={} bytes={} const:script_bytes={} lines={} inject_ok={} drops={} \
             timeouts={} head={} tail={} used={} empty_polls={} const:va=0x{:x} ring_ok={}",
            con_stats.irqs,
            con_stats.bytes,
            demo.uart_bytes,
            demo.uart_lines,
            demo.uart_inject_ok as u8,
            con_stats.drops,
            con_stats.timeouts,
            ring_head,
            ring_tail,
            ring_used,
            con_stats.empty_polls,
            con_ring.va,
            uart_console::ring_check() as u8
        );
    }
    let ok17 = user::captured_contains(SLOT_SHELL, b"# shell: help cmds=6")
        && user::captured_contains(SLOT_SHELL, b"# shell: echo shell-that")
        && ok_uart_path;
    if ok17 {
        let _ = writeln!(w, "ok 3 - shell help echo");
    } else {
        let _ = writeln!(
            w,
            "not ok 3 - shell help echo: thieu output U-mode hoac duong UART->ring->shell sai"
        );
        ok_all = false;
    }
    // 18 — ps từ core: snapshot kernel lúc `PS_REQ` phải khớp **từng dòng** shell in.
    let mut snapshot_ok = demo.ps_n == 4;
    if snapshot_ok {
        // **KT-119**: dựng dòng kỳ vọng từ `ps_real` — trạng thái **thật** đọc bằng
        // `m.tcb_state(id)` lúc `PS_REQ` — **không** giải mã lại `ps_words`. Trước đây test giải mã
        // `(entry >> 1) & 0x0F` y như shell, nên đột biến chỗ **đóng gói** (`state << 1` →
        // `state << 2`) làm shell in sai trạng thái mà test **vẫn xanh** (reviewer đã dựng lại:
        // shell in `state=Running` cho TCB đang `Ready`, `snapshot_match=1`).
        for (pid, state, suspended) in demo.ps_real[..demo.ps_n].iter() {
            let mut line = FixedBuf::new();
            line.push(b"  pid=");
            line.num(*pid);
            line.push(b" state=");
            line.push(state_label(*state));
            if *suspended {
                line.push(b" suspended");
            }
            if !user::captured_contains(SLOT_SHELL, line.as_bytes()) {
                snapshot_ok = false;
            }
        }
    }
    let ok18 = demo.ps_replies == 1
        && demo.ps_entries == 4
        && snapshot_ok
        && user::captured_contains(SLOT_SHELL, b"# shell: ps n=4");
    {
        let _ = writeln!(
            w,
            "# shell: ps replies={} entries={} snapshot_match={} captured={}",
            demo.ps_replies,
            demo.ps_entries,
            snapshot_ok as u8,
            user::captured_contains(SLOT_SHELL, b"# shell: ps n=") as u8
        );
    }
    if ok18 {
        let _ = writeln!(w, "ok 4 - shell ps tu core");
    } else {
        let _ = writeln!(w, "not ok 4 - shell ps tu core: snapshot/chuoi sai");
        ok_all = false;
    }
    // 19 — wait: giao thức procsrv trả mã thoát của con.
    let ok19 = demo.wait_pid == Some(u64::from(rig.tcb_obj[T_CHILD]))
        && demo.wait_code == Some(42)
        && user::captured_contains(SLOT_SHELL, b"# shell: wait pid=")
        && user::captured_contains(SLOT_SHELL, b" code=42");
    {
        let _ = writeln!(
            w,
            "# shell: wait pid={} code={} captured={}",
            demo.wait_pid.map(|p| p as i64).unwrap_or(-1),
            demo.wait_code.map(|c| c as i64).unwrap_or(-1),
            user::captured_contains(SLOT_SHELL, b"# shell: wait pid=") as u8
        );
    }
    if ok19 {
        let _ = writeln!(w, "ok 5 - shell wait ma thoat");
    } else {
        let _ = writeln!(w, "not ok 5 - shell wait ma thoat: giao thuc sai");
        ok_all = false;
    }
    // 20 — kill: procsrv quyết định, core treo TCB victim thật (`suspended=1`).
    let ok20 = demo.kill_pid == Some(u64::from(rig.tcb_obj[T_VICTIM]))
        && demo.kill_code == Some(0)
        && demo.kill_suspend_ok
        && demo.victim_suspended
        && user::captured_contains(SLOT_SHELL, b"# shell: kill pid=")
        && user::captured_contains(SLOT_SHELL, b" code=0");
    {
        let _ = writeln!(
            w,
            "# shell: kill pid={} code={} suspend_ok={} victim_suspended={} captured={}",
            demo.kill_pid.map(|p| p as i64).unwrap_or(-1),
            demo.kill_code.map(|c| c as i64).unwrap_or(-1),
            demo.kill_suspend_ok as u8,
            demo.victim_suspended as u8,
            user::captured_contains(SLOT_SHELL, b"# shell: kill pid=") as u8
        );
    }
    if ok20 {
        let _ = writeln!(w, "ok 6 - procsrv kill treo tcb");
    } else {
        let _ = writeln!(w, "not ok 6 - procsrv kill treo tcb: suspended sai");
        ok_all = false;
    }
    // 21 — shell Exit: core chuyển Inactive sau khi procsrv Recv (D2-25).
    let ok21 = demo.shell_exit_code == Some(0)
        && demo.shell_exit_state == Some(TcbState::Inactive)
        && demo.fault_seen >= 2
        && user::captured_contains(SLOT_SHELL, b"# shell: exit code=0")
        && user::captured_contains(SLOT_PROCSRV, b" code=0");
    {
        let shell_state = core_bridge::with_machine(|m| m.tcb_state(rig.tcb_obj[T_SHELL]).ok());
        let _ = writeln!(
            w,
            "# shell: exit code={} state={} faults={} steps={} captured={}",
            demo.shell_exit_code.map(|c| c as i64).unwrap_or(-1),
            shell_state.map(tcb_name).unwrap_or("none"),
            demo.fault_seen,
            STEP_NO.load(Ordering::Relaxed),
            user::captured_contains(SLOT_SHELL, b"# shell: exit code=0") as u8
        );
    }
    if ok21 {
        let _ = writeln!(w, "ok 7 - shell exit va procsrv finalise");
    } else {
        let _ = writeln!(
            w,
            "not ok 7 - shell exit va procsrv finalise: state/chuoi sai"
        );
        ok_all = false;
    }
    {
        // Bất biến core sau pha demo (cùng bộ kiểm của KT-62).
        let invariants = core_bridge::with_machine(|m| {
            m.check_caps().is_ok()
                && m.check_bytes(core_untyped).is_ok()
                && m.check_all_bytes().is_ok()
                && m.check_sched().is_ok()
                && m.check_sc_refs().is_ok()
                && m.check_budget().is_ok()
                && m.check_mailboxes().is_ok()
                && m.check_ipc().is_ok()
                && m.check_reply_links().is_ok()
                && m.check_notifications().is_ok()
                && m.check_waitaddr().is_ok()
                && m.check_fault().is_ok()
        });
        let _ = writeln!(
            w,
            "# proc: sched steps={} loads=3 events={} faults={} ps={} invariant={}",
            STEP_NO.load(Ordering::Relaxed),
            demo.events,
            demo.fault_seen,
            demo.ps_entries,
            invariants as u8
        );
        ok_all &= invariants;
    }
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(w, "# proc: sched done ok={}", ok_all as u8);
    }
    // Không trả về khung gọi: continuation chạy TAP 22..24 (fault/KT-56) rồi kết thúc boot.
    cont(init_loaded, init_exit_kind_ok)
}
