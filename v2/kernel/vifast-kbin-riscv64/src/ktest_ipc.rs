// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-280** — ảnh test **IPC từ U-mode** riscv64: client U-mode `Call` **chặn** tới **server
//! U-mode**, server `Recv` + `Reply`, client được **resume** với message trả lời.
//!
//! # Vì sao tách ảnh
//!
//! D2-109/D2-111: mỗi ảnh test một ngân sách (262.144 B `.data+.bss`), thêm **đường mới** ⇒ tách
//! ảnh, không nới trần. Ảnh này tách khỏi `ktest-user`/`ktest-proc` để TAP/`.text` của chúng
//! **không** đổi (hồi quy matrix cũ vẫn so đúng ảnh cũ).
//!
//! # Vì sao cần (khảo sát KT-280 §1 của `JOURNAL/kt280-ipc.md`)
//!
//! K3 đã có IPC **trong core** (TAP 9 của `ktest-user` gọi `Call/Recv/Send` **từ kernel**) và
//! U-mode đã có từ KT-53, nhưng **chưa** có đường "tác vụ U-mode chặn trong IPC rồi được resume":
//! mọi `call()` của chương trình U-mode đều nhắm cap **không phải Endpoint** (invocation ⇒ trả về
//! ngay), còn IPC U-mode hiện có là `NBSend`+`NBRecv` polling. Ba mảnh còn thiếu — và là toàn bộ
//! phạm vi KT-280:
//!
//! 1. `sepc += 4` khi tác vụ chặn trong đa nhiệm (`plat-riscv64/src/user.rs`, nhánh `Switched`);
//! 2. nạp `a0..a6` từ `Machine::tcb_ipc_area` khi resume (`user::apply_ipc_return`);
//! 3. scheduler giữ tác vụ đang chặn **sống** và resume nó (ảnh này).
//!
//! # Hai TAP (kế hoạch `1..2`)
//!
//! 1. `ipc: umode call` — client U-mode gọi `Call(EP, {label=0x280,length=1}, MR0=0x1234)`:
//!    core đưa TCB client sang `BlockedOnReply`, message **nằm trong hàng Endpoint**
//!    (`ep_len=1`, `ep_mr=0x1234`), rồi server U-mode `Recv` **nhận đúng** message đó (label/MR
//!    đọc từ `tcb_ipc_area` của server) và in `# ipc: server ran nr=1`.
//! 2. `ipc: server replied` — server `Send` trên cap `Reply` ⇒ client **được resume** với
//!    `a0=0`, `a2` = `MessageInfo{label=0x281,length=2}`, `a3=0x5a5a`, `a4=1` (đọc từ vùng nhận
//!    IPC của client, không phải kernel tự khai) ⇒ client in `# ipc: umode call ok reply=23130`;
//!    `Reply.caller` sạch, `check_ipc`/`check_caps` xanh, hai tác vụ `Exit(0)`.
//!
//! **Ca âm** (trong cùng boot, chương trình client tự kiểm): `Call` trên ô rỗng ⇒ 1
//! (`InvalidCapability`); `Call` trên cap `Reply` (không phải Endpoint) ⇒ 18 (`NotSupported`).
//!
//! Boot kết thúc bằng **một** `PANIC` có chủ đích (khuôn runner KT-54).
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_core::{KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::trap::{TrapFrame, REG_SP};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::user::{self, LoadedInit, Stop};
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Hai chương trình U-mode **riêng** do `build.rs` sinh — hai tiến trình, hai VSpace, hai cspace.
static CLIENT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ipc_client.elf"));
static SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ipc_server.elf"));

/// Ô tác vụ (`user::MAX_PROCS`) của hai tiến trình — dùng cho bảng dải VA + capture output.
const SLOT_CLIENT: usize = 1;
const SLOT_SERVER: usize = 2;

/// Ô cspace **của từng tiến trình** (kernel dựng; chương trình U-mode hardcode cùng số).
const CS_SELF: usize = 0;
const CS_EP: usize = 1;
const CS_REPLY: usize = 2;

/// Ô trong CNode gốc của rig.
const RIG_UNTYPED: usize = 0;
const RIG_TCB_C: usize = 1;
const RIG_TCB_S: usize = 2;
const RIG_EP: usize = 3;
const RIG_REPLY: usize = 4;
const RIG_CTL: usize = 5;
const RIG_SC_C: usize = 6;
const RIG_SC_S: usize = 7;
const RIG_CSPACE: usize = 8;

/// Label/MR **hợp đồng** giữa hai chương trình U-mode (không phải ABI) — khớp
/// `init/ipc_client_user.rs` + `init/ipc_server_user.rs`.
const LABEL_REQ: u32 = 0x280;
const LABEL_REPLY: u32 = 0x281;
const ARG: u64 = 0x1234;
const REPLY_VALUE: u64 = 0x5a5a;

/// Ưu tiên: server **cao hơn** client để sau `Reply` core chọn **server** chạy tiếp (server còn
/// một syscall `Exit`), rồi kernel mới resume client. Không phụ thuộc thứ tự FIFO.
const PRIO_CLIENT: u8 = 3;
const PRIO_SERVER: u8 = 5;

/// Rig của pha IPC — mọi thứ thuộc kernel, không phải chương trình.
struct Rig {
    tcb_c: ObjId,
    tcb_s: ObjId,
    ep: ObjId,
    reply: ObjId,
    cnode_c: ObjId,
    cnode_s: ObjId,
}

/// Đưa TCB về `Running{hart:0}` — cùng khuôn `ktest_capx::set_running` (chỉ `Inactive` mới
/// `start_tcb`; `Ready` đi thẳng qua `transition_tcb`).
fn set_running(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    if !matches!(m.tcb_state(tcb)?, TcbState::Running { .. }) {
        if m.tcb_state(tcb)? == TcbState::Inactive {
            m.start_tcb(tcb)?;
        }
        m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })?;
    }
    Ok(())
}

/// Dựng CNode 16 slot + 2 TCB + Endpoint + Reply + SchedControl + 2 SC + 2 cspace con.
///
/// Bố cục cspace con (giống nhau hai bên để chương trình U-mode hardcode một bảng):
/// `0` = cap `CNode` của chính nó, `1` = `Endpoint`, `2` = `Reply`; ô `9` để **rỗng** (ca âm
/// "Call trên ô rỗng").
fn build_rig(m: &mut Machine, u: ObjId) -> Rig {
    let root = m.add_cnode(16);
    let us = SlotRef::new(root, RIG_UNTYPED);
    let uc = m.root_cap(u);
    m.put_cap(root, RIG_UNTYPED, uc).expect("rig: cap Untyped");
    let types = [
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::SchedControl,
        ObjType::SchedContext,
        ObjType::SchedContext,
    ];
    let mut obj = [0u32; 7];
    for (i, ty) in types.iter().enumerate() {
        let cap = m.retype(us, *ty, 0, 0).expect("rig: retype object lõi");
        m.put_cap(root, RIG_TCB_C + i, cap).expect("rig: đặt cap");
        obj[i] = m.cap_obj(cap).expect("rig: cap_obj");
    }
    let cs = m.root_cap(root);
    m.put_cap(root, RIG_CSPACE, cs).expect("rig: cap cspace");
    let cnode_c = m.add_cnode(16);
    let cnode_s = m.add_cnode(16);
    let self_c = m.root_cap(cnode_c);
    m.put_cap(cnode_c, CS_SELF, self_c).expect("rig: self c");
    let self_s = m.root_cap(cnode_s);
    m.put_cap(cnode_s, CS_SELF, self_s).expect("rig: self s");
    // Endpoint dùng chung: cap ở **cả hai** cspace (client gửi, server nhận).
    m.copy(SlotRef::new(root, RIG_EP), SlotRef::new(cnode_c, CS_EP))
        .expect("rig: ep -> client");
    m.copy(SlotRef::new(root, RIG_EP), SlotRef::new(cnode_s, CS_EP))
        .expect("rig: ep -> server");
    // Reply: server cần để `Recv(ep, reply)`; client giữ một bản để ca âm "Call trên cap Reply".
    m.copy(
        SlotRef::new(root, RIG_REPLY),
        SlotRef::new(cnode_s, CS_REPLY),
    )
    .expect("rig: reply -> server");
    m.copy(
        SlotRef::new(root, RIG_REPLY),
        SlotRef::new(cnode_c, CS_REPLY),
    )
    .expect("rig: reply -> client");
    // Cấu hình 2 TCB: cspace riêng + SC + priority (server cao hơn — xem `PRIO_SERVER`).
    for (tcb_slot, cnode, sc, prio) in [
        (RIG_TCB_C, cnode_c, RIG_SC_C, PRIO_CLIENT),
        (RIG_TCB_S, cnode_s, RIG_SC_S, PRIO_SERVER),
    ] {
        m.tcb_configure(
            SlotRef::new(root, tcb_slot),
            Some(SlotRef::new(cnode, CS_SELF)),
        )
        .expect("rig: cspace");
        m.sched_control_configure(
            SlotRef::new(root, RIG_CTL),
            SlotRef::new(root, sc),
            500,
            5_000,
        )
        .expect("rig: SC");
        m.set_sched_params(SlotRef::new(root, tcb_slot), Some(SlotRef::new(root, sc)))
            .expect("rig: params");
        m.tcb_set_priority(SlotRef::new(root, tcb_slot), prio, 10)
            .expect("rig: priority");
    }
    Rig {
        tcb_c: obj[0],
        tcb_s: obj[1],
        ep: obj[2],
        reply: obj[3],
        cnode_c,
        cnode_s,
    }
}

/// Chạy **một lượt** tác vụ U-mode: gắn TCB core + đưa `Running` + `sret`; trả `Stop` và cập nhật
/// `tf` bằng trapframe **đã lưu** (KT-280: lượt sau resume từ đúng chỗ dừng).
fn run_turn(loaded: &LoadedInit, tf: &mut TrapFrame, tcb: ObjId, slot: usize) -> Option<Stop> {
    user::set_active_slot(slot);
    user::bind_core_tcb(tcb);
    if core_bridge::with_machine(|m| set_running(m, tcb).is_ok()) {
        let stop = user::run_task(loaded, tf);
        *tf = user::saved_tf();
        Some(stop)
    } else {
        None
    }
}

/// Trạng thái core của TCB (đọc sau mỗi lượt).
fn state_of(tcb: ObjId) -> Option<TcbState> {
    core_bridge::with_machine(|m| m.tcb_state(tcb).ok())
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..2\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let _mmu = vifast_plat_riscv64::mmu::init();
    let _tlb = vifast_plat_riscv64::tlb::init();
    let obj = vifast_plat_riscv64::obj::init();
    let core = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - ipc: umode call: khong cap duoc stack syscall\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    // Đa nhiệm bật để `Exit`/`Yield` **trả về kernel** (như `ktest-proc`), và để nhánh
    // `Switched` (tác vụ chặn) nhường CPU thay vì PANIC.
    user::enable_multitask(true);
    user::capture_begin();

    // ---- nạp HAI chương trình U-mode vào hai VSpace (cùng Untyped của ảnh test).
    let client = match user::load_init_slot(CLIENT_ELF, obj.untyped0, SLOT_CLIENT) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - ipc: umode call: nap client that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let server = match user::load_init_slot(SERVER_ELF, obj.untyped0, SLOT_SERVER) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - ipc: umode call: nap server that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let rig = core_bridge::with_machine(|m| build_rig(m, core.untyped));
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc rig: stvec=0x{:x} untyped={} core_untyped={} cspace_c={} cspace_s={} tcb_c={} tcb_s={} ep={} reply={}",
            stvec,
            obj.untyped0.0,
            core.untyped,
            rig.cnode_c,
            rig.cnode_s,
            rig.tcb_c,
            rig.tcb_s,
            rig.ep,
            rig.reply
        );
    }

    // ================================================================ lượt 1: client `Call`
    let mut tf_c = TrapFrame::zeroed();
    tf_c.x[REG_SP] = user::USER_STACK_TOP;
    tf_c.sepc = client.entry;
    let stop1 = run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT);
    let state1 = state_of(rig.tcb_c);
    // seL4: `Call` khi **chưa** có người nhận ⇒ caller chặn ở **Send** (`call = true`), message
    // nằm trong hàng Endpoint. Chuyển sang `BlockedOnReply` chỉ khi server `Recv` lấy message.
    let blocked_send = matches!(state1, Some(TcbState::BlockedOnSend { call: true, .. }));
    let (ep_len, ep_mr) = core_bridge::with_machine(|m| {
        (
            m.endpoint_send_len(rig.ep).ok(),
            m.endpoint_last_mr(rig.ep).ok().flatten(),
        )
    });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc call: blocked={} state={} call=1 ep_len={} ep_mr=0x{:x} label=0x{:x} stop={}",
            u8::from(blocked_send),
            state1.map(tcb_state_name).unwrap_or("none"),
            ep_len.unwrap_or(usize::MAX),
            ep_mr.unwrap_or(0),
            LABEL_REQ,
            stop_name(stop1)
        );
    }

    // ================================================================ lượt 2: server `Recv` + `Yield`
    let mut tf_s = TrapFrame::zeroed();
    tf_s.x[REG_SP] = user::USER_STACK_TOP;
    tf_s.sepc = server.entry;
    let stop2 = run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER);
    let state_s2 = state_of(rig.tcb_s);
    // **Bằng chứng trực tiếp** của hợp đồng seL4: sau `Recv` của server, caller chuyển
    // `BlockedOnSend{call}` → `BlockedOnReply` (server đang giữ `Reply.caller`).
    let state_c2 = state_of(rig.tcb_c);
    let blocked_reply = matches!(state_c2, Some(TcbState::BlockedOnReply { .. }));
    // Message **server nhận được** — đọc từ vùng nhận IPC của server (core sở hữu, D2-83).
    let (srv_label, srv_mr) = core_bridge::with_machine(|m| {
        m.tcb_ipc_area(rig.tcb_s)
            .ok()
            .and_then(|area| {
                area.info
                    .map(|info| (info.label(), area.mr(0).unwrap_or(0)))
            })
            .unwrap_or((u32::MAX, 0))
    });
    let caller_mid = core_bridge::with_machine(|m| m.reply_caller(rig.reply).ok().flatten());
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc server: recv={} label=0x{:x} mr=0x{:x} client={} caller={} server={} stop={}",
            u8::from(srv_label == LABEL_REQ && srv_mr == ARG),
            srv_label,
            srv_mr,
            state_c2.map(tcb_state_name).unwrap_or("none"),
            caller_mid.map(|t| t as u64).unwrap_or(0),
            state_s2.map(tcb_state_name).unwrap_or("none"),
            stop_name(stop2)
        );
    }

    // ================================================================ lượt 3: server `Reply` + `Exit`
    let stop3 = run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER);
    let state_s3 = state_of(rig.tcb_s);
    let state_c3 = state_of(rig.tcb_c);
    let reply_caller = core_bridge::with_machine(|m| m.reply_caller(rig.reply).ok().flatten());
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc reply: server_exit={} client={} reply_caller={} server={} stop={}",
            match stop3 {
                Some(Stop::Exit { code }) => code as i64,
                _ => -1,
            },
            state_c3.map(tcb_state_name).unwrap_or("none"),
            reply_caller.map(|t| t as u64).unwrap_or(0),
            state_s3.map(tcb_state_name).unwrap_or("none"),
            stop_name(stop3)
        );
    }

    // ================================================================ lượt 4: resume client
    // Nửa thứ hai của hợp đồng resume: `a0..a6` lấy từ vùng nhận IPC của client (message server
    // vừa giao). Không có message ⇒ `false` và client **không** được resume (đỏ có chẩn đoán).
    let applied = user::apply_ipc_return(&mut tf_c, rig.tcb_c);
    let stop4 = if applied {
        run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT)
    } else {
        None
    };
    let state_c4 = state_of(rig.tcb_c);
    let state_s4 = state_of(rig.tcb_s);
    // Message **client nhận được** — kernel đọc độc lập từ vùng nhận IPC của client (không tin
    // lời chương trình): label/MR phải đúng hợp đồng với server.
    let (cli_label, cli_mr0, cli_mr1) = core_bridge::with_machine(|m| {
        m.tcb_ipc_area(rig.tcb_c)
            .ok()
            .and_then(|area| {
                area.info.map(|info| {
                    (
                        info.label(),
                        area.mr(0).unwrap_or(0),
                        area.mr(1).unwrap_or(0),
                    )
                })
            })
            .unwrap_or((u32::MAX, 0, 0))
    });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc client: reply_label=0x{:x} reply_mr0=0x{:x} reply_mr1={} const:label_reply=0x{:x} const:reply_value=0x{:x}",
            cli_label, cli_mr0, cli_mr1, LABEL_REPLY, REPLY_VALUE
        );
    }
    user::capture_end();

    // ================================================================ TAP 1..2
    let client_said_call = user::captured_contains(SLOT_CLIENT, b"# ipc: umode call ok reply=");
    let client_said_neg = user::captured_contains(SLOT_CLIENT, b"# ipc: neg empty=1 reply=18 ok=1");
    let server_said_ran = user::captured_contains(SLOT_SERVER, b"# ipc: server ran nr=1");
    let (check_ipc, check_caps) =
        core_bridge::with_machine(|m| (m.check_ipc().is_ok(), m.check_caps().is_ok()));
    let reply_clean = reply_caller.is_none()
        && core_bridge::with_machine(|m| m.reply_caller(rig.reply).ok().flatten()).is_none();
    let client_exit = match stop4 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    let server_exit = match stop3 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    let mut fails = 0usize;
    // TAP 1 — `Call` U-mode **chặn** đúng, message tới server U-mode, caller `BlockedOnReply`.
    let ok1 = blocked_send
        && ep_len == Some(1)
        && ep_mr == Some(ARG)
        && srv_label == LABEL_REQ
        && srv_mr == ARG
        && blocked_reply
        && server_said_ran;
    if ok1 {
        PLATFORM.put_str("ok 1 - ipc: umode call\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 1 - ipc: umode call: blocked_send={} ep_len={} ep_mr=0x{:x} srv_label=0x{:x} srv_mr=0x{:x} blocked_reply={} server_ran={}",
            u8::from(blocked_send),
            ep_len.map(|n| n as u64).unwrap_or(u64::MAX),
            ep_mr.unwrap_or(0),
            srv_label,
            srv_mr,
            u8::from(blocked_reply),
            u8::from(server_said_ran)
        );
        fails += 1;
    }
    // TAP 2 — server `Reply` ⇒ client **resume** với message trả lời; ca âm đúng mã.
    let ok2 = applied
        && client_exit == Some(0)
        && server_exit == Some(0)
        && client_said_call
        && client_said_neg
        && cli_label == LABEL_REPLY
        && cli_mr0 == REPLY_VALUE
        && cli_mr1 == 1
        && reply_clean
        && check_ipc
        && check_caps;
    if ok2 {
        PLATFORM.put_str("ok 2 - ipc: server replied\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 2 - ipc: server replied: applied={} client_exit={} server_exit={} said_call={} said_neg={} reply_label=0x{:x} reply_mr0=0x{:x} reply_mr1={} reply_clean={} check_ipc={} check_caps={}",
            u8::from(applied),
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(client_said_call),
            u8::from(client_said_neg),
            cli_label,
            cli_mr0,
            cli_mr1,
            u8::from(reply_clean),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc final: client_exit={} server_exit={} reply_clean={} check_ipc={} check_caps={} capture_client={} capture_server={} client={} server={}",
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(reply_clean),
            u8::from(check_ipc),
            u8::from(check_caps),
            u8::from(client_said_call),
            u8::from(server_said_ran),
            state_c4.map(tcb_state_name).unwrap_or("none"),
            state_s4.map(tcb_state_name).unwrap_or("none")
        );
    }
    if fails == 0 {
        PLATFORM.put_str("PANIC: KT-280 ipc u-mode xong (2 TAP, 0 fail)\n");
        PLATFORM.halt(0)
    } else {
        PLATFORM.put_str("PANIC: KT-280 ipc u-mode CO LOI\n");
        PLATFORM.halt(1)
    }
}

/// Tên trạng thái TCB cho log (cùng bảng với `ktest_user`/`ktest_proc`).
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

/// Tên `Stop` cho log.
fn stop_name(stop: Option<Stop>) -> &'static str {
    match stop {
        Some(Stop::Exit { .. }) => "Exit",
        Some(Stop::Yield) => "Yield",
        Some(Stop::Blocked(_)) => "Blocked",
        Some(Stop::Halt { .. }) => "Halt",
        Some(Stop::Fault) => "Fault",
        None => "None",
    }
}

/// Entry hart **thứ cấp** (matrix `-smp 2/4`): adopt `satp`/`stvec` của BSP rồi park — KT-280
/// không cần hart phụ (hai tác vụ U-mode trên một hart), nhưng linker đòi symbol cho mọi ảnh.
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
