// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-77b** — runtime **shell của ảnh sản phẩm**: `procsrv` + `shell` là chương trình U-mode
//! thật; kernel chỉ đóng vai scheduler tuần tự + thi hành policy (D2-25/QD §1 Q-A), và byte của
//! người dùng đi **từ ngoài** (QEMU stdin → UART RX → ngắt PLIC → ring KT-69 → shell U-mode).
//!
//! # Khác `proc_demo` (ảnh test KT-65) — đúng những gì OD-K6-2 đòi
//!
//! - **Không** script nội bộ: không có `build_script`, không `inject_loopback`, không chạy sẵn
//!   `help/ps/...`. Kịch bản đến từ serial (người dùng/kịch bản của `v2/test/parity/run.sh`).
//! - **Không** TAP, không `captured_*`: ảnh sản phẩm **không** được chứa mã harness (D2-08 +
//!   OD-K6-2 tiêu chí 6, kiểm bằng `v2/agent/JOURNAL/kt77b-harness-scan.sh`).
//! - **Vẫn** phục vụ `PS_REQ`/`wait`/`kill`/fault-forwarding: phần policy này nằm **ở đây**, không
//!   phải trong `proc_demo` — nếu chỉ ảnh test phục vụ `ps` thì đó là "user mode phụ thuộc test
//!   harness" (OD-K6-1). Bố cục cspace/nhãn message dùng chung ở [`crate::proc_abi`].
//!
//! # Vì sao byte vào ring qua **cửa sổ S-mode** chứ không bật ngắt RX thường trực
//!
//! Trap từ U-mode **không** đổi `satp` về root kernel ([`vifast_plat_riscv64::trap_dispatch`]):
//! handler PLIC/UART đọc ring qua **physmap**, mà physmap (root index ≥ 256) **không** map trong
//! VSpace user (`obj::copy_kernel_low_root`). Ngắt RX đến đúng lúc đang chạy U-mode ⇒ handler
//! page-fault (fatal). Vì vậy runtime mở **cửa sổ RX ngắn khi đang ở S-mode** (cùng cơ chế
//! `uart_console::rx_begin`/`rx_end` mà ảnh test dùng), rồi đóng lại trước khi hạ U-mode: byte đến
//! trong lúc cửa sổ đóng nằm ở FIFO 16550 (16 B) và được vét cạn trong `rx_end`/cửa sổ kế. Đây là
//! **nợ K7** đã ghi ở `docs/k6-product-shell.md` §7 (đóng hẳn bằng cách cho trap từ U-mode khôi
//! phục root kernel trước khi dispatch ngắt); nó **không** phải phụ thuộc harness.
#![allow(clippy::too_many_arguments)]

use core::fmt::Write;

use vifast_core::{
    FaultKind, Machine, Message, MessageInfo, NonReadyTarget, ObjId, ObjType, SlotRef, SuspendMode,
    TcbState,
};
use vifast_plat_common::obj::CapId;
use vifast_plat_common::trap::{TrapFrame, REG_SP};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::obj as aobj;
#[cfg(feature = "product-fs")]
use vifast_plat_riscv64::static_slot::StaticSlot;
use vifast_plat_riscv64::user::{self, LoadedInit, Stop};
use vifast_plat_riscv64::{core_bridge, uart_console, Riscv64Virt};

#[cfg(feature = "product-net")]
use crate::net_svc::{NetSvc, Outcome as NetOutcome};
use crate::proc_abi::*;
#[cfg(feature = "product-exec")]
use crate::proc_exec;
/// **KT-77c**: dịch vụ FS của ảnh sản phẩm (chỉ có khi bật `product-fs`).
#[cfg(feature = "product-fs")]
use crate::proc_fs::{FsSvc, FsVol};
#[cfg(feature = "product-net")]
use crate::proc_net;

/// Tham số FS của [`service`]: cặp **(trạng thái giao thức, `Fs` trong ô static)**. `Fs` **không**
/// nằm trong khung stack nào ở đây — nó ở `.bss` qua
/// [`vifast_plat_riscv64::static_slot::StaticSlot`] (xem `proc_fs.rs` đầu tệp).
///
/// Ca âm tiêu chí 5(c) build **không** `product-fs`: kiểu rỗng, không có mã FS nào trong ảnh,
/// request FS của shell không được phục vụ (shell in `fs khong san sang` ⇒ parity FS đỏ).
#[cfg(feature = "product-fs")]
type FsArg<'a> = Option<(&'a mut FsSvc, &'a mut FsVol)>;
#[cfg(not(feature = "product-fs"))]
type FsArg<'a> = Option<&'a mut ()>;

/// Ô `.bss` giữ `Fs` của ảnh sản phẩm (**một** lần dựng cho cả đời ảnh).
#[cfg(feature = "product-fs")]
static FS_SLOT: StaticSlot<FsVol> = StaticSlot::new();

/// ELF U-mode do `build.rs` sinh (cùng nguồn với ảnh test: `init/procsrv_user.rs`,
/// `init/shell_user.rs`); `CARGO_FEATURE_USER` bật ⇒ `build.rs` đã sinh hai tệp này.
static PROCSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/procsrv.elf"));
static SHELL_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.elf"));
/// **KT-342** — net server U-mode của ảnh sản phẩm (`product-net`; cnode riêng do
/// [`proc_net::build`] dựng). Không nạp ở ảnh không bật feature.
#[cfg(feature = "product-net")]
static NETSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/netsrv_prod.elf"));

/// Vùng RAM cho Untyped **U2** của pha shell (VSpace/Frame/ring) — ngoài mọi vùng kernel/object
/// layer; PA thật, **không** đụng trần `.data+.bss` (D2-72 Q5). Cùng dải với ảnh test để hai ảnh
/// không lệch bản đồ bộ nhớ.
const PROD_ARENA_BASE: u64 = 0x8100_0000;
const PROD_ARENA_BYTES: u32 = 512 * 1024;
const PROD_PA_BASE: u64 = 0x8120_0000;

/// Chỉ số tác vụ trong bảng TCB kernel dựng (khác `proc_demo`: ảnh sản phẩm **không** nạp con).
const T_PROCSRV: usize = 0;
const T_SHELL: usize = 1;
const T_VICTIM: usize = 2;
const T_KERN: usize = 3;
/// Số TCB kernel dựng.
const N_TCB: usize = 4;

/// Cửa sổ RX mỗi vòng (tick 10 MHz): 2 ms — đủ để byte của một burst 16 B vào ring mà không làm
/// shell chậm thấy được; số này là **thiết kế**, in kèm `const:` trong log (D2-102a).
const RX_WINDOW_TICKS: u64 = 20_000;
/// **D2-102a** — kiểu đầu vào của shell, in kèm nhãn `const:` trong dòng evidence (giá trị cấu
/// hình, không phải số đo): byte đi từ **ngoài** qua UART, kernel không cắm script.
const INPUT_MODE: &str = "serial";
/// Số lần kernel tự bơm payload vào ring (ảnh sản phẩm: không lần nào).
const INJECT_COUNT: u8 = 0;
/// Đường byte của ảnh sản phẩm: ngắt RX của UART → PLIC → ring → U-mode.
const RX_PATH: &str = "uart-rx-irq->plic->ring->u-mode";

/// Watchdog: shell phải thoát hoặc máy phải còn tiến bộ trong 600 s; hết hạn ⇒ PANIC (runner đỏ),
/// không treo im lặng. Dài hơn mọi ngưỡng boot của harness (20 s) nên không ảnh hưởng ca kiểm.
const WATCHDOG_SECS: u64 = 600;

/// Một tác vụ U-mode do runtime này chạy.
struct Task {
    slot: usize,
    loaded: LoadedInit,
    tf: TrapFrame,
    exited: bool,
}

/// Cap/slot kernel dựng cho procsrv/shell — bố cục slot lấy từ [`crate::proc_abi`].
struct Rig {
    tcb: [SlotRef; N_TCB],
    tcb_obj: [ObjId; N_TCB],
    ep_slot: [SlotRef; 7],
    /// **KT-342** — cap/TCB của `netsrv` U-mode + endpoint dịch vụ kernel.
    #[cfg(feature = "product-net")]
    net: proc_net::NetRig,
    /// **KT-342** — endpoint `run` của shell → kernel.
    #[cfg(feature = "product-exec")]
    exec: proc_exec::ExecRig,
}

/// Trạng thái policy thu thập được (evidence + điều kiện dừng), không có phần TAP.
#[derive(Default)]
struct Svc {
    procsrv_up: bool,
    events: usize,
    faults: usize,
    ps_replies: usize,
    ps_n: usize,
    kill_ok: bool,
    shell_exit_core: Option<u64>,
    /// Số reply FS đã gửi (KT-77c) — 0 ở ảnh **không** `product-fs`.
    fs_replies: u64,
    /// **KT-342** — số request `ping` đã phục vụ qua `netsrv` (0 ở ảnh không `product-net`).
    #[cfg(feature = "product-net")]
    net_replies: u64,
    /// **KT-342** — số lần `exec` thành công trên ảnh sản phẩm (0 ở ảnh không `product-exec`).
    #[cfg(feature = "product-exec")]
    execs: u64,
}

fn msg(label: u64, words: &[u64]) -> Message {
    let info = MessageInfo::new(label as u32, 0, false, 0, words.len() as u8)
        .expect("KT-77b: MessageInfo");
    Message::new(info, words).expect("KT-77b: Message")
}

fn fail(platform: &Riscv64Virt, what: &str) -> ! {
    let mut w = PlatformWriter(platform);
    let _ = writeln!(w, "PANIC: product shell: {what}");
    platform.halt(1)
}

/// Dựng cspace giống **nguyên** bố cục [`crate::proc_abi`]: 6 endpoint + cspace + 4 TCB + 4 SC.
///
/// Ngữ nghĩa lấy từ `proc_demo::build_rig` (KT-65) — chỉ khác số TCB (ảnh sản phẩm không nạp con
/// nên không có TCB `child`); slot của procsrv/shell/victim/kern **không đổi**.
fn build_rig(untyped: ObjId) -> Rig {
    core_bridge::with_machine(|m| {
        let root = m.add_cnode(64);
        let us = SlotRef::new(root, 0);
        let uc = m.root_cap(untyped);
        m.put_cap(root, 0, uc).expect("KT-77b: cap untyped");
        // **KT-77c**: thêm `EP_FS` (slot 7) — bố cục procsrv/shell/victim/kern không đổi, nên
        // ảnh test (`proc_demo`) và ảnh sản phẩm vẫn dùng chung `proc_abi`.
        let ep_slots = [
            EP_FAULT, EP_PROC, EP_UART, EP_REPLY, EP_PS, EP_EVENTS, EP_FS,
        ];
        let mut ep_slot = [SlotRef::new(root, 0); 7];
        for (i, slot) in ep_slots.iter().enumerate() {
            let cap = m
                .retype(us, ObjType::Endpoint, 0, 0)
                .expect("KT-77b: retype endpoint");
            m.put_cap(root, *slot, cap).expect("KT-77b: put endpoint");
            ep_slot[i] = SlotRef::new(root, *slot);
        }
        let cs = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cs)
            .expect("KT-77b: cspace cap");
        let ctl_cap = m
            .retype(us, ObjType::SchedControl, 0, 0)
            .expect("KT-77b: retype SchedControl");
        m.put_cap(root, EP_CTL, ctl_cap).expect("KT-77b: cap ctl");
        let ctl = SlotRef::new(root, EP_CTL);
        let cspace = SlotRef::new(root, SLOT_CSPACE);
        let mut tcb = [SlotRef::new(root, 0); N_TCB];
        let mut tcb_obj = [0u32; N_TCB];
        let mut sc = [SlotRef::new(root, 0); N_TCB];
        for i in 0..N_TCB {
            let tcap = m
                .retype(us, ObjType::Tcb, 0, 0)
                .expect("KT-77b: retype TCB");
            m.put_cap(root, TCB_SLOTS[i], tcap)
                .expect("KT-77b: put TCB");
            let scap = m
                .retype(us, ObjType::SchedContext, 0, 0)
                .expect("KT-77b: retype SC");
            m.put_cap(root, SC_SLOTS[i], scap).expect("KT-77b: put SC");
            tcb[i] = SlotRef::new(root, TCB_SLOTS[i]);
            tcb_obj[i] = m.cap_obj(tcap).expect("KT-77b: tcb obj");
            sc[i] = SlotRef::new(root, SC_SLOTS[i]);
        }
        for i in 0..N_TCB {
            m.tcb_configure(tcb[i], Some(cspace))
                .expect("KT-77b: tcb cspace");
            // TCB `kern` là đích *nhận* sự kiện/PS_REQ, không có fault endpoint.
            if i < T_KERN {
                m.tcb_set_fault_ep(tcb[i], ep_slot[0])
                    .expect("KT-77b: fault_ep");
            }
            m.sched_control_configure(ctl, sc[i], 1_000, 10_000)
                .expect("KT-77b: SC");
            m.set_sched_params(tcb[i], Some(sc[i]))
                .expect("KT-77b: sched params");
            m.tcb_set_priority(tcb[i], 5, 10).expect("KT-77b: priority");
            m.start_tcb(tcb_obj[i]).expect("KT-77b: Ready");
        }
        // **KT-342**: hai đường của ảnh sản phẩm — mạng (`netsrv` + cnode riêng) và `run`.
        #[cfg(feature = "product-net")]
        let net = proc_net::build(m, root, us);
        #[cfg(feature = "product-exec")]
        let exec = proc_exec::build(m, root, us);
        Rig {
            tcb,
            tcb_obj,
            ep_slot,
            #[cfg(feature = "product-net")]
            net,
            #[cfg(feature = "product-exec")]
            exec,
        }
    })
}

/// Dịch vụ kernel giữa hai lượt U-mode — **cùng giao thức** với `proc_demo::service` (KT-65/KT-73),
/// bỏ đúng phần harness: bơm script và đếm `captured_*`.
fn service(m: &mut Machine, rig: &Rig, svc: &mut Svc, fs: &mut FsArg<'_>) {
    // 1) Sự kiện procsrv → kernel.
    while let Ok(Some(message)) = m.nb_recv(rig.tcb_obj[T_KERN], rig.ep_slot[5]) {
        svc.events += 1;
        match u64::from(message.info.label()) {
            EV_UP => svc.procsrv_up = true,
            EV_FAULT => svc.faults += 1,
            EV_KILL => {
                // `kill(pid)`: procsrv quyết định theo giao thức, kernel thi hành `Tcb.Suspend`.
                let suspend = m.tcb_suspend(rig.tcb[T_VICTIM], SuspendMode::Abort);
                svc.kill_ok = suspend.is_ok();
            }
            _ => {}
        }
    }
    // 2) Fault `Exit` procsrv vừa `NBRecv` — core đã ghi `last_fault` + `Inactive` (D2-25);
    //    kernel đọc rồi chuyển tiếp một message vào fault_ep (cầu nối arch, nợ ABI).
    if let Ok(Some((pid, kind))) = m.take_last_fault(rig.tcb_obj[T_PROCSRV]) {
        if let FaultKind::Exit { code } = kind {
            let words = [u64::from(pid), code, 0, 0];
            let _ = m.nb_send(rig.tcb_obj[T_KERN], rig.ep_slot[0], msg(FAULT_EXIT, &words));
            if pid == rig.tcb_obj[T_SHELL] {
                svc.shell_exit_core = Some(code);
            }
        }
    }
    // 3) `ps`: shell gửi `PS_REQ`, kernel đọc trạng thái TCB **từ `Machine`** rồi trả message —
    //    shell tự định dạng/in (không bảng tĩnh, không harness).
    while let Ok(Some(request)) = m.nb_recv(rig.tcb_obj[T_KERN], rig.ep_slot[4]) {
        if u64::from(request.info.label()) == PS_REQ {
            let ids = [
                rig.tcb_obj[T_PROCSRV],
                rig.tcb_obj[T_SHELL],
                rig.tcb_obj[T_VICTIM],
            ];
            let mut entries = [0u64; 3];
            let mut n = 0usize;
            for id in ids {
                let Ok(state) = m.tcb_state(id) else { continue };
                let suspended = m
                    .tcb_info_obj(id)
                    .map(|info| info.suspended)
                    .unwrap_or(false);
                entries[n] =
                    (u64::from(id) << 5) | ((state.index() as u64) << 1) | u64::from(suspended);
                n += 1;
            }
            let words = [entries[0] | (entries[1] << 16), entries[2]];
            let _ = m.nb_send(rig.tcb_obj[T_KERN], rig.ep_slot[3], msg(PS_REPLY, &words));
            svc.ps_replies += 1;
            svc.ps_n = n;
        }
    }
    // 4) **KT-77c** — `ls`/`cat`/ghi/`sync`: shell gửi `FS_REQ` (4 MR) trên endpoint slot 7,
    //    kernel FS (`proc_fs::FsSvc`) trả **một** reply `FS_REPLY` trên endpoint reply. Một request
    //    một reply nên shell không cần hàng đợi, và ngữ nghĩa `Yield`/`NBRecv` không đổi.
    #[cfg(feature = "product-fs")]
    {
        if let Some((state, volume)) = fs.as_mut() {
            while let Ok(Some(request)) = m.nb_recv(rig.tcb_obj[T_KERN], rig.ep_slot[6]) {
                if u64::from(request.info.label()) != FS_REQ {
                    continue;
                }
                let words = request.words();
                let head = [
                    words.first().copied().unwrap_or(0),
                    words.get(1).copied().unwrap_or(0),
                    words.get(2).copied().unwrap_or(0),
                    words.get(3).copied().unwrap_or(0),
                ];
                let reply = state.request(volume, head);
                let _ = m.nb_send(rig.tcb_obj[T_KERN], rig.ep_slot[3], msg(FS_REPLY, &reply));
                svc.fs_replies += 1;
            }
        } else {
            // Ảnh **không** FS: rút request khỏi hàng đợi để endpoint không đầy, **và trả lời ngay**
            // một `FS_REPLY` lỗi. **KT-123** (review chéo): trước đây chỉ rút mà **không** trả lời,
            // nên shell quay `FS_REPLY_BUDGET` (hai mươi nghìn) vòng `Yield` rồi mới bỏ cuộc — đo
            // được **~54 giây mỗi lệnh** (`ls` cũ ~50 giây). Nay shell nhận lỗi ở vòng đầu ⇒ in
            // `fs khong san sang` rồi trả prompt ngay. Parity FS vẫn **đỏ** (ca âm 5(c)) vì không
            // có dữ liệu thật — chỉ nhanh hơn, không nới phép kiểm nào.
            while let Ok(Some(request)) = m.nb_recv(rig.tcb_obj[T_KERN], rig.ep_slot[6]) {
                if u64::from(request.info.label()) != FS_REQ {
                    continue;
                }
                let _ = m.nb_send(
                    rig.tcb_obj[T_KERN],
                    rig.ep_slot[3],
                    msg(FS_REPLY, &[ST_NOFS, 0, 0, 0]),
                );
                svc.fs_replies += 1;
            }
        }
    }
}

/// Hạ **một** tác vụ xuống U-mode đúng một lượt; trả `None` nếu không chuyển được TCB sang
/// `Running`. `tcb_obj` là object TCB của tác vụ — nhận tường minh để `netsrv` (TCB thứ năm của
/// ảnh sản phẩm) dùng **cùng** đường chạy với procsrv/shell (KT-342).
///
/// `Stop::Blocked` **không** đánh dấu `exited`: đó là trạng thái bình thường của server U-mode
/// đang `Recv` chờ request (KT-342), không phải kết thúc tác vụ.
fn turn(task: &mut Task, tcb_obj: ObjId) -> Option<Stop> {
    user::set_active_slot(task.slot);
    user::bind_core_tcb(tcb_obj);
    let entered = core_bridge::with_machine(|m| {
        m.transition_tcb(tcb_obj, NonReadyTarget::Running { hart: 0 })
            .is_ok()
    });
    if !entered {
        return None;
    }
    let stop = user::run_task(&task.loaded, &mut task.tf);
    task.tf = user::saved_tf();
    match stop {
        Stop::Yield => {
            core_bridge::with_machine(|m| {
                let _ = m.tcb_yield(tcb_obj);
            });
        }
        Stop::Blocked(_) => {}
        _ => task.exited = true,
    }
    Some(stop)
}

/// **KT-342** — một lượt của `netsrv` **chỉ khi** TCB của nó `Ready` (đang chờ request đã có
/// message) hoặc `Running`; mọi trạng thái chặn khác ⇒ bỏ lượt, **không** lỗi.
#[cfg(all(feature = "product-net", not(feature = "mut-no-netsrv")))]
fn net_turn(rig: &Rig, task: &mut Task) -> Option<Stop> {
    let ready = core_bridge::with_machine(|m| {
        matches!(
            m.tcb_state(rig.net.tcb_obj),
            Ok(TcbState::Ready) | Ok(TcbState::Running { .. })
        )
    });
    if !ready {
        return None;
    }
    // **KT-342**: `netsrv` là tác vụ **duy nhất** dùng syscall **chặn** (`Recv`/`Call`), nên mỗi
    // lượt resume phải lấy `a0..a6` từ **vùng nhận IPC** của core — cùng hợp đồng
    // `user_run::apply_ipc_return` mà `ktest_netsrv` gọi trước mỗi lượt server. Không có message
    // (lượt đầu, chưa ai gọi) ⇒ hàm trả `false` và **không** chạm trapframe.
    let _ = user::apply_ipc_return(&mut task.tf, rig.net.tcb_obj);
    turn(task, rig.net.tcb_obj)
}

/// **KT-342** — phục vụ một request `ping` đang chờ trên `EP_NET_KERN` (nếu có): `NetSvc` kiểm
/// quyền rồi chạm driver **một** lần; trả `true` ⇔ vừa phục vụ xong một request.
#[cfg(feature = "product-net")]
fn net_service(
    platform: &Riscv64Virt,
    rig: &Rig,
    svc: &mut Svc,
    netsvc: &mut NetSvc,
    out: &mut NetOutcome,
) -> bool {
    let pending = core_bridge::with_machine(|m| proc_net::pending(m, rig.net.ep_kern_obj));
    if !pending {
        return false;
    }
    let outcome = core_bridge::with_machine(|m| netsvc.service(m));
    let mut w = PlatformWriter(platform);
    let _ = writeln!(
        w,
        "# product net: served={} denied={} failed={} err={} rtt_ms={} sent={} received={} label=0x{:x} caller={}",
        netsvc.served,
        netsvc.denied,
        netsvc.failed,
        netsvc.last_err,
        netsvc.last_rtt_ms,
        netsvc.sent,
        netsvc.received,
        netsvc.last_label,
        netsvc.last_caller
    );
    // **KT-342**: chuyển **kết quả** của `netsrv` cho shell trên `EP_REPLY` (đường kernel→U-mode
    // đã dùng cho FS, D2-83): shell chỉ đọc **kết quả**, không giữ cap của driver/`NetSvc`.
    // `netsrv` trả lời `Call` của kernel (bằng chứng `# netsrv: ping …`) rồi thoát; message này
    // là bản sao kết quả **kernel đã nhận**, không phải shell tự tin lời chương trình.
    let fwd = core_bridge::with_machine(|m| {
        m.nb_send(
            rig.tcb_obj[T_KERN],
            rig.ep_slot[3],
            msg(NET_REPLY, &[netsvc.last_rtt_ms, netsvc.last_err]),
        )
    });
    if fwd.is_err() {
        return false;
    }
    svc.net_replies += 1;
    *out = outcome;
    true
}

/// Cửa sổ RX **ở S-mode**: mở nguồn (IER.RX + PLIC line 10 + cổng external), chờ ngắt thật đẩy
/// byte vào ring tới khi ring có dữ liệu hoặc hết [`RX_WINDOW_TICKS`], rồi đóng và vét cạn `RBR`.
///
/// Trả `(ngắt_mới, byte_vét_thêm)` — số đo, không phải hằng.
fn rx_window(platform: &Riscv64Virt) -> (u64, u32) {
    let before = uart_console::stats().irqs;
    let (_, _) = uart_console::rx_begin();
    let start = platform.now_ticks();
    while uart_console::ring_stats().2 == 0
        && platform.now_ticks().wrapping_sub(start) < RX_WINDOW_TICKS
    {
        core::hint::spin_loop();
    }
    let (late, _) = uart_console::rx_end();
    (uart_console::stats().irqs.saturating_sub(before), late)
}

/// Chạy shell sản phẩm tới khi shell `Exit` (rồi `halt(0)`) hoặc một bước hỏng (PANIC + halt(1)).
pub fn run(platform: &Riscv64Virt, load_untyped: CapId, core_untyped: ObjId) -> ! {
    if !user::init_syscall_stacks() {
        fail(platform, "khong cap duoc stack syscall per-hart");
    }
    let u2 = match aobj::machine().add_untyped(PROD_ARENA_BASE, PROD_ARENA_BYTES, PROD_PA_BASE) {
        Ok(cap) => cap,
        Err(_) => fail(platform, "add_untyped U2"),
    };
    let ring = match uart_console::init_ring(u2) {
        Ok(report) => report,
        Err(stage) => fail(platform, stage),
    };
    let procsrv = match user::load_init_slot(PROCSRV_ELF, load_untyped, SLOT_PROCSRV) {
        Ok(loaded) => loaded,
        Err(e) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "# product load: procsrv that bai kind={} ({:?})",
                e.name(),
                e
            );
            fail(platform, "load procsrv");
        }
    };
    let shell = match user::load_init_slot(SHELL_ELF, load_untyped, SLOT_SHELL) {
        Ok(loaded) => loaded,
        Err(e) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "# product load: shell that bai kind={} ({:?})",
                e.name(),
                e
            );
            fail(platform, "load shell");
        }
    };
    let load_ok = procsrv.check_ok
        && procsrv.wx_pages == 0
        && procsrv.flags_ok
        && procsrv.high_half_unmapped
        && shell.check_ok
        && shell.wx_pages == 0
        && shell.flags_ok
        && shell.high_half_unmapped;
    let ring_mapped = uart_console::attach(shell.vspace);
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# product load: procsrv=[0x{:x},0x{:x}) asid={} segs={} stack={} ok={} shell=[0x{:x},0x{:x}) asid={} segs={} stack={} ok={} wx={} high_unmapped={}",
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
            shell.wx_pages,
            shell.high_half_unmapped as u8,
        );
        let _ = writeln!(
            w,
            "# product ring: pa=0x{:x} va=0x{:x} slots={} slot_bytes={} check={} mapped_shell={}",
            ring.pa, ring.va, ring.slots, ring.slot_bytes, ring.check as u8, ring_mapped as u8
        );
    }
    if !load_ok || !ring.check || !ring_mapped {
        fail(platform, "ke hoach nap shell sai (W^X/cờ/ring)");
    }
    // **KT-342**: mở mạng **trước** khi hạ U-mode (PCI ECAM/BAR/DMA + DHCP + ARP đều chạm MMIO ở
    // S-mode) — cùng lý do `proc_fs::open`; lỗi ⇒ PANIC có tên (fail-closed, không chạy mạng dở).
    #[cfg(feature = "product-net")]
    let net_setup = match proc_net::open() {
        Ok(setup) => setup,
        Err(why) => fail(platform, why),
    };
    #[cfg(feature = "product-net")]
    proc_net::report(platform, &net_setup);

    let rig = build_rig(core_untyped);
    // **KT-342**: dịch vụ `NetPing` của kernel (KT-306, `net_svc::NetSvc`) — caller duy nhất là
    // `netsrv` U-mode vừa dựng; chính sách IP lấy từ DHCP/ARP thật (không hằng của bài test).
    #[cfg(feature = "product-net")]
    let mut netsvc = NetSvc::new(
        rig.tcb_obj[T_KERN],
        rig.net.ep_kern,
        rig.net.reply_kern,
        rig.net.reply_kern_obj,
        rig.net.tcb_obj,
        net_setup.config.ip,
        net_setup.mac,
        net_setup.config.gw,
        net_setup.gw_mac,
    );
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# product shell: harts={} procsrv=0x{:x} shell=0x{:x} victim=0x{:x} kern=0x{:x} const:ep_proc={} const:ep_ps={} const:ep_events={} const:slot_cspace={} const:input_mode={} const:inject={}",
            vifast_plat_riscv64::smp::hart_count(),
            rig.tcb_obj[T_PROCSRV],
            rig.tcb_obj[T_SHELL],
            rig.tcb_obj[T_VICTIM],
            rig.tcb_obj[T_KERN],
            EP_PROC,
            EP_PS,
            EP_EVENTS,
            SLOT_CSPACE,
            INPUT_MODE,
            INJECT_COUNT
        );
        let _ = writeln!(
            w,
            "# product rx: const:window_ticks={} const:watchdog_s={} const:rx_path={}",
            RX_WINDOW_TICKS, WATCHDOG_SECS, RX_PATH
        );
    }
    // **KT-77c**: mở FS **trước** khi hạ U-mode (driver PCI/virtio-blk chạm MMIO ở S-mode).
    // `medium=virtio-blk` khi có `-drive` (bền qua reboot); `medium=ram` (bay hơi) khi không có
    // đĩa — cả hai đường đều in một dòng log, không im lặng và không treo.
    // `open` dựng `Fs` **trong ô `.bss`** (`FS_SLOT`) và trả **tham chiếu** — không bản `Fs` nào
    // nằm trong khung này (tránh tràn guard page 64 KiB; xem `proc_fs.rs` đầu tệp).
    #[cfg(feature = "product-fs")]
    let mut fs_open = crate::proc_fs::open(platform, &FS_SLOT);
    #[cfg(feature = "product-fs")]
    let mut fs_vol = fs_open.fs.take();
    #[cfg(feature = "product-fs")]
    let mut fs_state = fs_open.svc;
    #[cfg(not(feature = "product-fs"))]
    let mut fs_none: FsArg = None;

    // **KT-342**: nạp `netsrv` U-mode vào ô [`SLOT_NETSRV`] (cùng loader `vifast-elf` + W^X của
    // shell/procsrv; cnode riêng đã dựng trong `build_rig`).
    #[cfg(feature = "product-net")]
    let netsrv = match user::load_init_slot(NETSRV_ELF, load_untyped, SLOT_NETSRV) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "# product load: netsrv that bai kind={} ({:?})",
                error.name(),
                error
            );
            fail(platform, "load netsrv")
        }
    };

    let mut tasks = [
        Task {
            slot: SLOT_PROCSRV,
            loaded: procsrv,
            tf: TrapFrame::zeroed(),
            exited: false,
        },
        Task {
            slot: SLOT_SHELL,
            loaded: shell,
            tf: TrapFrame::zeroed(),
            exited: false,
        },
    ];
    for task in tasks.iter_mut() {
        task.tf.x[REG_SP] = user::USER_STACK_TOP;
        task.tf.sepc = task.loaded.entry;
    }
    #[cfg(feature = "product-net")]
    let mut net_task = Task {
        slot: SLOT_NETSRV,
        loaded: netsrv,
        tf: TrapFrame::zeroed(),
        exited: false,
    };
    #[cfg(feature = "product-net")]
    {
        net_task.tf.x[REG_SP] = user::USER_STACK_TOP;
        net_task.tf.sepc = net_task.loaded.entry;
    }
    #[cfg(feature = "product-net")]
    let mut net_exit = false;
    user::enable_multitask(true);
    let mut svc = Svc::default();
    let start = platform.now_ticks();
    let mut windows = 0u64;
    let mut irqs = 0u64;
    let mut late_bytes = 0u32;
    let mut shell_code: Option<u64> = None;
    loop {
        // 1) Cửa sổ RX ở S-mode (ngắt thật), rồi đóng trước khi hạ U-mode — xem đầu tệp.
        let (irq, late) = rx_window(platform);
        windows += 1;
        irqs += irq;
        late_bytes += late;
        // 2) Mỗi vòng đúng một lượt procsrv + một lượt shell (round-robin, không scheduler core).
        for idx in [T_PROCSRV, T_SHELL] {
            if idx == T_SHELL && shell_code.is_some() {
                continue;
            }
            let task = &mut tasks[idx];
            if task.exited {
                continue;
            }
            match turn(task, rig.tcb_obj[idx]) {
                Some(Stop::Yield) => {}
                Some(Stop::Exit { code }) => {
                    if idx == T_SHELL {
                        shell_code = Some(code);
                    } else {
                        fail(
                            platform,
                            "procsrv thoat bat thuong (shell khong con nguoi phuc vu)",
                        );
                    }
                }
                Some(Stop::Fault) => fail(platform, "u-mode fault trong pha shell"),
                Some(Stop::Halt { .. }) | Some(Stop::Blocked(_)) => {
                    fail(platform, "tac vu dung ngoai y muon")
                }
                None => fail(platform, "khong chuyen duoc tcb sang Running"),
            }
            core_bridge::with_machine(|m| {
                #[cfg(feature = "product-fs")]
                {
                    let mut arg: FsArg = fs_vol.as_deref_mut().map(|v| (&mut fs_state, v));
                    service(m, &rig, &mut svc, &mut arg);
                }
                #[cfg(not(feature = "product-fs"))]
                {
                    let _ = &mut fs_none;
                    service(m, &rig, &mut svc, &mut fs_none);
                }
            });
            // **KT-342**: `run <path>` của shell ⇒ kernel-mediated `exec` (D2-96). Thành công thì
            // trapframe của ô shell đã trỏ ảnh mới; lỗi thì kernel trả `EXEC_REPLY` cho shell.
            #[cfg(feature = "product-exec")]
            if idx == T_SHELL {
                let task = &mut tasks[T_SHELL];
                let outcome = core_bridge::with_machine(|m| {
                    proc_exec::service(
                        m,
                        &rig.exec,
                        rig.tcb_obj[T_KERN],
                        &mut task.loaded,
                        SLOT_SHELL,
                        rig.tcb[T_SHELL],
                        &mut task.tf,
                        load_untyped,
                        rig.ep_slot[3],
                    )
                });
                match outcome {
                    proc_exec::Outcome::Empty => {}
                    proc_exec::Outcome::Executed(report) => {
                        svc.execs += 1;
                        let mut w = PlatformWriter(platform);
                        let _ = writeln!(
                            w,
                            "# product exec: path={} pid={} tcb={} old_entry=0x{:x} new_entry=0x{:x} old_vspace={} new_vspace={} freed_frames={} new_pages={} frames={}->{} objects={}->{} caps={}->{} check={}",
                            proc_exec::HELLO_PATH,
                            report.pid,
                            report.tcb,
                            report.old_entry,
                            report.new_entry,
                            report.old_vspace,
                            report.new_vspace,
                            report.freed_frames,
                            report.new_pages,
                            report.before.frames,
                            report.after.frames,
                            report.before.objects,
                            report.after.objects,
                            report.before.caps,
                            report.after.caps,
                            report.check_ok as u8
                        );
                    }
                    proc_exec::Outcome::Failed(code) => {
                        let mut w = PlatformWriter(platform);
                        let _ = writeln!(w, "# product exec: err={code}");
                    }
                    proc_exec::Outcome::Error => fail(platform, "exec: loi IPC noi bo"),
                }
            }
        }
        // **KT-342**: một lượt `netsrv` (chỉ khi TCB `Ready`) + phục vụ request đang chờ trên
        // `EP_NET_KERN` — `NetSvc` kiểm quyền rồi chạm driver **một** lần.
        #[cfg(all(feature = "product-net", not(feature = "mut-no-netsrv")))]
        if shell_code.is_none() && !net_task.exited {
            match net_turn(&rig, &mut net_task) {
                Some(Stop::Exit { code }) if code == 0 => net_exit = true,
                Some(Stop::Exit { .. }) => fail(platform, "netsrv thoat loi"),
                Some(Stop::Fault) | Some(Stop::Halt { .. }) => {
                    fail(platform, "netsrv dung ngoai y muon")
                }
                Some(Stop::Yield) | Some(Stop::Blocked(_)) | None => {}
            }
            let mut outcome = NetOutcome::Empty;
            let _ = net_service(platform, &rig, &mut svc, &mut netsvc, &mut outcome);
        }
        if let Some(code) = shell_code {
            // Cho procsrv nhận `FAULT_EXIT` của shell và in `# proc: recv exit` trước khi tắt:
            // core chỉ ghi `Tcb.last_fault` **sau** lượt `NBRecv` của procsrv (D2-25), nên cần
            // một lượt để nhận fault + một lượt để nhận message chuyển tiếp.
            // Cho procsrv hoàn tất giao thức: nhận fault `Exit` của shell rồi nhận message
            // chuyển tiếp (`FAULT_EXIT`) — đúng thứ tự đo được: lượt 1 xếp hàng, lượt 2 mới
            // thấy `last_fault` (log `kt77b-smoke3.log`), nên chạy tới khi `faults >= 1`.
            for _ in 0..6 {
                if !tasks[T_PROCSRV].exited {
                    let task = &mut tasks[T_PROCSRV];
                    let _ = turn(task, rig.tcb_obj[T_PROCSRV]);
                }
                core_bridge::with_machine(|m| {
                    #[cfg(feature = "product-fs")]
                    {
                        let mut arg: FsArg = fs_vol.as_deref_mut().map(|v| (&mut fs_state, v));
                        service(m, &rig, &mut svc, &mut arg);
                    }
                    #[cfg(not(feature = "product-fs"))]
                    {
                        let _ = &mut fs_none;
                        service(m, &rig, &mut svc, &mut fs_none);
                    }
                });
                if svc.faults >= 1 && svc.shell_exit_core.is_some() {
                    break;
                }
            }
            // **KT-77c**: tổng kết dịch vụ FS (số đo, không phải hằng) — in **trước** dòng halt để
            // `grep '# product fs:'` thấy cả lúc mở volume lẫn lúc kết thúc.
            #[cfg(feature = "product-fs")]
            fs_state.report(platform, svc.fs_replies);
            let (head, tail, used, _) = uart_console::ring_stats();
            let stats = uart_console::stats();
            let mut w = PlatformWriter(platform);
            let _ = writeln!(
                w,
                "# product halt: shell_exit={} fault_seen={} events={} ps_replies={} ps_n={} kill_suspend={} proc_up={} fs_replies={} rx_windows={} rx_irqs={} rx_bytes={} rx_late={} ring_used={} ring_head={} ring_tail={} ring_ok={}",
                code,
                svc.faults,
                svc.events,
                svc.ps_replies,
                svc.ps_n,
                svc.kill_ok as u8,
                svc.procsrv_up as u8,
                svc.fs_replies,
                windows,
                irqs,
                stats.bytes,
                late_bytes,
                used,
                head,
                tail,
                uart_console::ring_check() as u8
            );
            #[cfg(feature = "product-net")]
            {
                let mut w = PlatformWriter(platform);
                let _ = writeln!(
                    w,
                    "# product halt net: replies={} net_exit={}",
                    svc.net_replies, net_exit as u8
                );
            }
            #[cfg(feature = "product-exec")]
            {
                let mut w = PlatformWriter(platform);
                let _ = writeln!(w, "# product halt exec: execs={}", svc.execs);
            }
            platform.halt(0)
        }
        if platform.now_ticks().wrapping_sub(start) > WATCHDOG_SECS * platform.timebase_hz() {
            fail(platform, "watchdog: shell khong thoat");
        }
    }
}
