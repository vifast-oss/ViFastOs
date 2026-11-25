// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-281** — ảnh test **net server U-mode** riscv64 (N1 phần 3): client U-mode `Call` **chặn**
//! tới **server U-mode** `netsrv`, server gọi **dịch vụ mạng của kernel** (syscall `Call` trên
//! `Endpoint`, driver `virtio_net` chạy trong kernel) rồi `Reply(rtt)` về client.
//!
//! # Vì sao tách ảnh
//!
//! D2-109/D2-111: mỗi ảnh test một ngân sách (262.144 B `.data+.bss`), thêm **đường mới** ⇒ tách
//! ảnh, không nới trần. Ảnh này tách khỏi `ktest-ipc`/`ktest-net` để TAP/`.text` của chúng
//! **không** đổi (hồi quy matrix cũ vẫn so đúng ảnh cũ).
//!
//! # Kiến trúc (driver **ở kernel**, server **chỉ điều phối**)
//!
//! ```text
//! client U-mode (netcli)        server U-mode (netsrv)             kernel (driver)
//!   Call(EP_SRV, Ping(ip)) ──►  Recv(EP_SRV) → ip
//!     [BlockedOnSend{call}]       Call(EP_KERN, NetPing(ip)) ──►  recv(EP_KERN, reply)
//!                                   [BlockedOnSend{call}]          virtio_net::ping(...)
//!                                                                  reply_msg(rtt)
//!   ◄── Send(Reply, rtt) ◄────  resume: a0=0, a3=rtt_ms
//!   resume: a0=0, a3=rtt_ms
//! ```
//!
//! Server U-mode **không** giữ cap `Frame`/BAR, **không** map MMIO: nó chỉ có cap `Endpoint`.
//! Mọi truy cập thanh ghi thiết bị nằm trong `vifast-plat-riscv64::virtio_net` (crate arch,
//! `unsafe` trong danh sách D2-17).
//!
//! # Vì sao `Call` trên Endpoint chứ không phải syscall `NetPing` riêng
//!
//! `spec/abi.toml` cấp đúng **16** syscall `0..15` (dải `16..31` reserved) và luật vòng này **cấm
//! sửa `spec/`** ⇒ không thêm được số 16. Syscall **0 `Call`** trên cap `Endpoint` đã có đúng ngữ
//! nghĩa cần (chặn → kernel xử lý → trả `(error, value)` qua `a0/a1`), đúng khuôn dịch vụ FS của
//! kernel (`proc_fs.rs`). Khảo sát + lý do: `JOURNAL/kt281-netsrv.md` §1.
//!
//! # Ba TAP (kế hoạch `1..3`)
//!
//! 1. `netsrv: server ran` — server U-mode **thật** nhận `Ping(ip)` từ client U-mode, gọi **dịch
//!    vụ kernel** `NetPing` bằng syscall `Call` (KT-306: `net_svc::NetSvc`, **không** phải
//!    `virtio_net::ping` gọi thẳng trong rig), driver ping **thật** (ICMP echo → slirp trả lời),
//!    in `# netsrv: ping 10.0.2.2 ok rtt=<ms>` + `# netping: label=0x291 reply=0 rtt_ms=<ms>`;
//!    kernel đọc **độc lập** message server gửi kernel (`label=0x291 mr=0x0a000202`), `PingStats`
//!    thật (`sent=1 received=1`) và reply trong vùng nhận IPC của server.
//! 2. `netsrv: client got reply` — server `Send` trên cap `Reply` ⇒ client **được resume** với
//!    `a0=0`, `a2` = `MessageInfo{label=0x292,length=1}`, `a3` = RTT; client in
//!    `# netsrv: client got reply=<v>`; ca âm trong cùng boot (`Call` ô rỗng ⇒ 1, `Call` trên cap
//!    `Reply` ⇒ 18).
//! 3. `netping: via ipc` — **`NetPing` là syscall thật**: dịch vụ kernel **kiểm quyền** (caller =
//!    TCB net server, `label = 0x291`, `length = 1`, IP trong chính sách) **trước** khi chạm
//!    driver. Ca âm trong cùng boot: `Call` với **label sai** (`0x999`) ⇒ `InvalidArgument` (5),
//!    **không** gói nào được gửi (`sent` vẫn 1) — server in `# netping: neg label err=5 ok=1`.
//!
//! **Ca âm** (fail-closed): không `-netdev`/`-device` ⇒ `# netsrv: no device` + `not ok 1` +
//! `halt(1)` — runner `net.sh` coi là **đạt** khi ảnh đỏ đúng cách.
//!
//! Boot kết thúc bằng **một** `PANIC` có chủ đích (khuôn runner KT-54).
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_core::{
    KError, Machine, Message, MessageInfo, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState,
};
use vifast_net::addr::MacAddr;
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, LoadedInit, Stop};
use vifast_plat_riscv64::{pci, virtio_net};
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::pci::VirtioKind;

mod net_svc;

use net_svc::{NetSvc, Outcome, LABEL_BAD, LABEL_NETPING};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Hai chương trình U-mode **riêng** do `build.rs` sinh — hai tiến trình, hai VSpace, hai cspace.
static CLIENT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/netcli.elf"));
static SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/netsrv.elf"));

/// Ô tác vụ (`user::MAX_PROCS`) của hai tiến trình — dùng cho bảng dải VA + capture output.
const SLOT_CLIENT: usize = 1;
const SLOT_SERVER: usize = 2;

/// Ô cspace **của từng tiến trình** (kernel dựng; chương trình U-mode hardcode cùng số).
const C_SELF: usize = 0;
const C_EP_SRV: usize = 1;
const C_REPLY: usize = 2;
/// **KT-306** — client giữ **một bản** cap `EP_KERN` **chỉ để kiểm quyền**: nó gọi dịch vụ với
/// request **hợp lệ** (`label = 0x291`) nhưng **không phải** net server ⇒ dịch vụ phải trả
/// `InsufficientRights` (3). Không có bản cap này thì nhánh kiểm `caller` không có phép kiểm nào.
const C_EP_KERN: usize = 3;
const C_EMPTY: usize = 9;

const S_SELF: usize = 0;
const S_EP_SRV: usize = 1;
const S_REPLY: usize = 2;
const S_EP_KERN: usize = 3;

const K_SELF: usize = 0;
const K_EP_KERN: usize = 1;
const K_REPLY: usize = 2;

/// Ô trong CNode gốc của rig.
const RIG_UNTYPED: usize = 0;
const RIG_TCB_C: usize = 1;
const RIG_TCB_S: usize = 2;
const RIG_TCB_K: usize = 3;
const RIG_EP_SRV: usize = 4;
const RIG_EP_KERN: usize = 5;
const RIG_REPLY_CLIENT: usize = 6;
const RIG_REPLY_KERN: usize = 7;
const RIG_CTL: usize = 8;
const RIG_SC_C: usize = 9;
const RIG_SC_S: usize = 10;
const RIG_SC_K: usize = 11;
const RIG_CSPACE: usize = 12;

/// Label/MR **hợp đồng** giữa ba bên (không phải ABI) — khớp `init/netcli_user.rs` +
/// `init/netsrv_user.rs`.
const LABEL_REQ: u32 = 0x290;
const LABEL_KERN: u32 = 0x291;
const LABEL_REPLY: u32 = 0x292;
/// IP cần ping, mã hoá `(a<<24)|(b<<16)|(c<<8)|d` — gateway slirp `10.0.2.2` (dữ kiện nền tảng
/// QEMU, không phải số đo).
const TARGET_IP: u64 = 0x0A00_0202;

/// Ưu tiên: server **cao hơn** client để sau `Reply` core chọn **server** chạy tiếp (server còn
/// một syscall `Exit`), rồi kernel mới resume client. Kernel thấp hơn cả hai — nó chỉ chạy xen
/// giữa hai lượt U-mode, không tranh CPU.
const PRIO_CLIENT: u8 = 3;
const PRIO_SERVER: u8 = 5;
const PRIO_KERN: u8 = 2;

/// Rig của pha netsrv — mọi thứ thuộc kernel, không phải chương trình.
struct Rig {
    tcb_c: ObjId,
    tcb_s: ObjId,
    tcb_k: ObjId,
    ep_srv: ObjId,
    ep_kern: ObjId,
    reply_client: ObjId,
    reply_kern: ObjId,
    cnode_c: ObjId,
    cnode_s: ObjId,
    cnode_k: ObjId,
}

/// Đưa TCB về `Running{hart:0}` — cùng khuôn `ktest_ipc::set_running` (chỉ `Inactive` mới
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

/// Dựng CNode 32 slot + 3 TCB + 2 Endpoint + 2 Reply + SchedControl + 3 SC + 3 cspace con.
///
/// Bố cục cspace con (xem bảng ở đầu `init/netcli_user.rs` / `init/netsrv_user.rs`):
/// - client: `0` self, `1` `EP_SRV`, `2` `Reply` (ca âm), `9` **rỗng** (ca âm);
/// - server: `0` self, `1` `EP_SRV` (`Recv`), `2` `Reply` (trả client), `3` `EP_KERN` (`Call`);
/// - kernel: `0` self, `1` `EP_KERN` (`recv`), `2` `Reply` (trả server).
fn build_rig(m: &mut Machine, u: ObjId) -> Rig {
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, RIG_UNTYPED);
    let uc = m.root_cap(u);
    m.put_cap(root, RIG_UNTYPED, uc).expect("rig: cap Untyped");
    let types = [
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::Reply,
        ObjType::SchedControl,
        ObjType::SchedContext,
        ObjType::SchedContext,
        ObjType::SchedContext,
    ];
    let mut obj = [0u32; 11];
    for (i, ty) in types.iter().enumerate() {
        let cap = m.retype(us, *ty, 0, 0).expect("rig: retype object lõi");
        m.put_cap(root, RIG_TCB_C + i, cap).expect("rig: đặt cap");
        obj[i] = m.cap_obj(cap).expect("rig: cap_obj");
    }
    let cs = m.root_cap(root);
    m.put_cap(root, RIG_CSPACE, cs).expect("rig: cap cspace");
    let cnode_c = m.add_cnode(16);
    let cnode_s = m.add_cnode(16);
    let cnode_k = m.add_cnode(16);
    for (cnode, self_slot) in [(cnode_c, C_SELF), (cnode_s, S_SELF), (cnode_k, K_SELF)] {
        let self_cap = m.root_cap(cnode);
        m.put_cap(cnode, self_slot, self_cap).expect("rig: self");
    }
    // Endpoint `EP_SRV`: client gửi (`Call`), server nhận (`Recv`).
    m.copy(
        SlotRef::new(root, RIG_EP_SRV),
        SlotRef::new(cnode_c, C_EP_SRV),
    )
    .expect("rig: ep_srv -> client");
    m.copy(
        SlotRef::new(root, RIG_EP_SRV),
        SlotRef::new(cnode_s, S_EP_SRV),
    )
    .expect("rig: ep_srv -> server");
    // Endpoint `EP_KERN`: server gửi (`Call`), kernel nhận (`recv`).
    m.copy(
        SlotRef::new(root, RIG_EP_KERN),
        SlotRef::new(cnode_s, S_EP_KERN),
    )
    .expect("rig: ep_kern -> server");
    m.copy(
        SlotRef::new(root, RIG_EP_KERN),
        SlotRef::new(cnode_k, K_EP_KERN),
    )
    .expect("rig: ep_kern -> kernel");
    // **KT-306**: client giữ **một bản** `EP_KERN` để kiểm nhánh **quyền** của dịch vụ (request
    // hợp lệ nhưng caller không phải net server ⇒ `InsufficientRights` 3).
    m.copy(
        SlotRef::new(root, RIG_EP_KERN),
        SlotRef::new(cnode_c, C_EP_KERN),
    )
    .expect("rig: ep_kern -> client (kiem quyen)");
    // Reply trả client: server giữ để `Send`; client giữ một bản cho ca âm "Call trên cap Reply".
    m.copy(
        SlotRef::new(root, RIG_REPLY_CLIENT),
        SlotRef::new(cnode_s, S_REPLY),
    )
    .expect("rig: reply -> server");
    m.copy(
        SlotRef::new(root, RIG_REPLY_CLIENT),
        SlotRef::new(cnode_c, C_REPLY),
    )
    .expect("rig: reply -> client");
    // Reply trả server: kernel giữ để `reply_msg` sau khi chạy driver.
    m.copy(
        SlotRef::new(root, RIG_REPLY_KERN),
        SlotRef::new(cnode_k, K_REPLY),
    )
    .expect("rig: reply -> kernel");
    // Cấu hình 3 TCB: cspace riêng + SC + priority.
    for (tcb_slot, cnode, self_slot, sc, prio) in [
        (RIG_TCB_C, cnode_c, C_SELF, RIG_SC_C, PRIO_CLIENT),
        (RIG_TCB_S, cnode_s, S_SELF, RIG_SC_S, PRIO_SERVER),
        (RIG_TCB_K, cnode_k, K_SELF, RIG_SC_K, PRIO_KERN),
    ] {
        m.tcb_configure(
            SlotRef::new(root, tcb_slot),
            Some(SlotRef::new(cnode, self_slot)),
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
        tcb_k: obj[2],
        ep_srv: obj[3],
        ep_kern: obj[4],
        reply_client: obj[5],
        reply_kern: obj[6],
        cnode_c,
        cnode_s,
        cnode_k,
    }
}

/// Chạy **một lượt** tác vụ U-mode: gắn TCB core + đưa `Running` + `sret`; trả `Stop` và cập nhật
/// `tf` bằng trapframe **đã lưu** (lượt sau resume từ đúng chỗ dừng).
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

/// Message `label` + `words` — cùng khuôn `proc_rt::msg`.
fn msg(label: u32, words: &[u64]) -> Message {
    let info =
        MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("KT-281: MessageInfo");
    Message::new(info, words).expect("KT-281: Message")
}

/// `(label, MR0)` của message **vừa nhận** trong vùng nhận IPC của `tcb` — kernel đọc **độc lập**,
/// không tin lời chương trình (D2-83: core sở hữu message, arch chỉ đọc).
fn received(tcb: ObjId) -> (u32, u64) {
    core_bridge::with_machine(|m| {
        m.tcb_ipc_area(tcb)
            .ok()
            .and_then(|area| {
                area.info
                    .map(|info| (info.label(), area.mr(0).unwrap_or(0)))
            })
            .unwrap_or((u32::MAX, 0))
    })
}

/// Tên trạng thái TCB cho log (cùng bảng với `ktest_ipc`/`ktest_user`).
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

/// Tên [`Outcome`] cho log (KT-306).
fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Empty => "Empty",
        Outcome::Served => "Served",
        Outcome::Denied => "Denied",
        Outcome::Failed => "Failed",
        Outcome::Error => "Error",
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

/// In MAC dạng `aa:bb:cc:dd:ee:ff` (không cấp phát).
fn put_mac(w: &mut PlatformWriter<'_, Riscv64Virt>, mac: [u8; 6]) {
    let _ = write!(
        w,
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..3\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu_report = vifast_plat_riscv64::mmu::init();
    let _tlb = vifast_plat_riscv64::tlb::init();

    // ------------------------------------------------ 0) thiết bị mạng (fail-closed)
    // Ca âm **cố định** (D2-196 mục (3), cùng khuôn `ktest_net`): không có `-netdev`/`-device`
    // ⇒ in `# netsrv: no device`, TAP 1 **đỏ**, `halt(1)`. Runner `net.sh` coi là **đạt** khi ảnh
    // đỏ đúng cách; ảnh PASS giả ⇒ runner **đỏ**.
    let pci_report = match pci::init_for(VirtioKind::Net) {
        Ok(report) => report,
        Err(fail) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# netsrv pci: err={} stage={}",
                fail.error.name(),
                fail.stage
            );
            let _ = writeln!(w, "# netsrv: no device");
            let _ = writeln!(
                w,
                "not ok 1 - netsrv: server ran: khong tim thay thiet bi mang"
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let net = match virtio_net::init(&pci_report) {
        Ok(report) => report,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# netsrv net: init err={:?}", error);
            let _ = writeln!(w, "# netsrv: no device");
            let _ = writeln!(w, "not ok 1 - netsrv: server ran: init driver that bai");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let our_mac = MacAddr::new(virtio_net::mac());
    // DHCP + ARP gateway: cùng hai bước `ktest_net` làm trước khi ping (driver **ở kernel**).
    let config = match virtio_net::dhcp_acquire(virtio_net::NET_STEP_TIMEOUT_TICKS) {
        Ok(config) => config,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# netsrv dhcp: err={:?}", error);
            let _ = writeln!(w, "# netsrv: no link");
            let _ = writeln!(
                w,
                "not ok 1 - netsrv: server ran: khong lay duoc dia chi DHCP"
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let gw_mac =
        match virtio_net::arp_resolve(config.ip, config.gw, virtio_net::NET_STEP_TIMEOUT_TICKS) {
            Ok(mac) => mac,
            Err(error) => {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(w, "# netsrv arp: err={:?}", error);
                let _ = writeln!(w, "# netsrv: no link");
                let _ = writeln!(
                    w,
                    "not ok 1 - netsrv: server ran: khong resolve duoc gateway"
                );
                PLATFORM.put_str("# ktest halt\n");
                PLATFORM.halt(1)
            }
        };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netsrv image: hart={} stvec=0x{:x} satp=0x{:x} mode={} tables={} used={} \
             const:target=10.0.2.2",
            PLATFORM.hart_id(),
            stvec,
            mmu_report.satp,
            mmu_report.satp >> 60,
            mmu_report.tables,
            mmu_report.used
        );
        let _ = write!(w, "# netsrv net: mac=");
        put_mac(&mut w, net.config.mac);
        let _ = writeln!(
            w,
            " dhcp ip={} gw={} mask={} server={}",
            config.ip, config.gw, config.mask, config.server
        );
        let _ = write!(w, "# netsrv arp: gw=");
        put_mac(&mut w, gw_mac.0);
        let _ = writeln!(w);
    }

    // ------------------------------------------------ 1) core + U-mode
    let obj = vifast_plat_riscv64::obj::init();
    let core = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - netsrv: server ran: khong cap duoc stack syscall\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    // Đa nhiệm bật để `Exit`/`Yield` **trả về kernel** (như `ktest-ipc`), và để nhánh `Switched`
    // (tác vụ chặn) nhường CPU thay vì PANIC.
    user::enable_multitask(true);
    user::capture_begin();

    let client = match user::load_init_slot(CLIENT_ELF, obj.untyped0, SLOT_CLIENT) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - netsrv: server ran: nap netcli that bai kind={} ({:?})",
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
                "not ok 1 - netsrv: server ran: nap netsrv that bai kind={} ({:?})",
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
            "# netsrv rig: untyped={} core_untyped={} cspace_c={} cspace_s={} cspace_k={} \
             tcb_c={} tcb_s={} tcb_k={} ep_srv={} ep_kern={} reply_client={} reply_kern={}",
            obj.untyped0.0,
            core.untyped,
            rig.cnode_c,
            rig.cnode_s,
            rig.cnode_k,
            rig.tcb_c,
            rig.tcb_s,
            rig.tcb_k,
            rig.ep_srv,
            rig.ep_kern,
            rig.reply_client,
            rig.reply_kern
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
            m.endpoint_send_len(rig.ep_srv).ok(),
            m.endpoint_last_mr(rig.ep_srv).ok().flatten(),
        )
    });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netsrv call: blocked={} state={} call=1 ep_len={} ep_mr=0x{:x} label=0x{:x} stop={}",
            u8::from(blocked_send),
            state1.map(tcb_state_name).unwrap_or("none"),
            ep_len.unwrap_or(usize::MAX),
            ep_mr.unwrap_or(0),
            LABEL_REQ,
            stop_name(stop1)
        );
    }

    // ================================================================ lượt 2: server `Recv` + `Call` kernel
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
    let (srv_label, srv_mr) = received(rig.tcb_s);
    let caller_mid = core_bridge::with_machine(|m| m.reply_caller(rig.reply_client).ok().flatten());
    // Server đã `Call` kernel ⇒ chặn ở Send trên `EP_KERN`, message `NetPing(ip)` nằm trong hàng.
    let srv_blocked_kern = matches!(state_s2, Some(TcbState::BlockedOnSend { call: true, .. }));
    let (kern_len, kern_mr) = core_bridge::with_machine(|m| {
        (
            m.endpoint_send_len(rig.ep_kern).ok(),
            m.endpoint_last_mr(rig.ep_kern).ok().flatten(),
        )
    });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netsrv server: recv={} label=0x{:x} mr=0x{:x} client={} caller={} server={} \
             kern_blocked={} kern_len={} kern_mr=0x{:x} stop={}",
            u8::from(srv_label == LABEL_REQ && srv_mr == TARGET_IP),
            srv_label,
            srv_mr,
            state_c2.map(tcb_state_name).unwrap_or("none"),
            caller_mid.map(|t| t as u64).unwrap_or(0),
            state_s2.map(tcb_state_name).unwrap_or("none"),
            u8::from(srv_blocked_kern),
            kern_len.unwrap_or(usize::MAX),
            kern_mr.unwrap_or(0),
            stop_name(stop2)
        );
    }

    // ================================================================ dịch vụ kernel #1 — `NetPing`
    // **KT-306**: `NetPing` **không** còn là `virtio_net::ping` gọi thẳng trong rig — nó là dịch
    // vụ kernel (`net_svc::NetSvc`) trên `EP_KERN`, có **kiểm quyền** trước khi chạm driver.
    let mut svc = NetSvc::new(
        rig.tcb_k,
        SlotRef::new(rig.cnode_k, K_EP_KERN),
        SlotRef::new(rig.cnode_k, K_REPLY),
        rig.reply_kern,
        rig.tcb_s,
        config.ip,
        our_mac,
        config.gw,
        gw_mac,
    );
    let outcome1 = core_bridge::with_machine(|m| svc.service(m));
    // Ảnh chụp **sau** lượt #1 (lượt #2 sẽ ghi đè các trường `last_*`).
    let s1_served = svc.served;
    let s1_denied = svc.denied;
    let s1_failed = svc.failed;
    let s1_label = svc.last_label;
    let s1_len = svc.last_length;
    let s1_ip = svc.last_ip;
    let s1_caller = svc.last_caller;
    let s1_err = svc.last_err;
    let s1_sent = svc.sent;
    let s1_received = svc.received;
    let srv_state_kern = state_of(rig.tcb_s);
    // Kernel đọc **độc lập** vùng nhận IPC của server: reply phải đúng label `NetPing` + MR0 = RTT.
    let (srv_reply_label, srv_reply_mr) = received(rig.tcb_s);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netping kern: n=1 outcome={} served={} denied={} failed={} label=0x{:x} len={} \
             mr=0x{:x} caller={} err={} rtt_ms={} sent={} received={} reply_label=0x{:x} \
             reply_mr={} server={}",
            outcome_name(outcome1),
            svc.served,
            svc.denied,
            svc.failed,
            svc.last_label,
            svc.last_length,
            svc.last_ip,
            svc.last_caller,
            svc.last_err,
            svc.last_rtt_ms,
            svc.sent,
            svc.received,
            srv_reply_label,
            srv_reply_mr,
            srv_state_kern.map(tcb_state_name).unwrap_or("none")
        );
    }
    let rtt_ms = svc.last_rtt_ms;

    // ================================================================ lượt 3: server resume + `Call` label SAI
    // Nửa thứ hai của hợp đồng resume: `a0..a6` lấy từ vùng nhận IPC của server (message dịch vụ
    // vừa giao). Không có message ⇒ `false` và server **không** được resume (đỏ có chẩn đoán).
    let applied_s = user::apply_ipc_return(&mut tf_s, rig.tcb_s);
    let stop3 = if applied_s {
        run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER)
    } else {
        None
    };
    let state_s3 = state_of(rig.tcb_s);
    // Ca âm: server `Call` lại dịch vụ với **label sai** ⇒ chặn ở Send trên `EP_KERN`.
    let srv_blocked_bad = matches!(state_s3, Some(TcbState::BlockedOnSend { call: true, .. }));
    let (bad_len, bad_mr) = core_bridge::with_machine(|m| {
        (
            m.endpoint_send_len(rig.ep_kern).ok(),
            m.endpoint_last_mr(rig.ep_kern).ok().flatten(),
        )
    });

    // ================================================================ dịch vụ kernel #2 — label sai ⇒ TỪ CHỐI
    let outcome2 = core_bridge::with_machine(|m| svc.service(m));
    let s2_served = svc.served;
    let s2_denied = svc.denied;
    let s2_failed = svc.failed;
    let s2_label = svc.last_label;
    let s2_len = svc.last_length;
    let s2_ip = svc.last_ip;
    let s2_caller = svc.last_caller;
    let s2_err = svc.last_err;
    let s2_sent = svc.sent;
    let s2_received = svc.received;
    let (srv_deny_label, srv_deny_mr) = received(rig.tcb_s);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netping kern: n=2 outcome={} served={} denied={} failed={} label=0x{:x} len={} \
             mr=0x{:x} caller={} err={} rtt_ms={} sent={} received={} reply_label=0x{:x} \
             reply_mr={} server={} stop={}",
            outcome_name(outcome2),
            svc.served,
            svc.denied,
            svc.failed,
            svc.last_label,
            svc.last_length,
            svc.last_ip,
            svc.last_caller,
            svc.last_err,
            svc.last_rtt_ms,
            svc.sent,
            svc.received,
            srv_deny_label,
            srv_deny_mr,
            state_of(rig.tcb_s).map(tcb_state_name).unwrap_or("none"),
            stop_name(stop3)
        );
    }

    // ================================================================ lượt 4: server resume + `Reply` client
    let applied_s2 = user::apply_ipc_return(&mut tf_s, rig.tcb_s);
    let stop4 = if applied_s2 {
        run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER)
    } else {
        None
    };
    let state_s4 = state_of(rig.tcb_s);
    let state_c4 = state_of(rig.tcb_c);
    let reply_caller =
        core_bridge::with_machine(|m| m.reply_caller(rig.reply_client).ok().flatten());
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netsrv reply: applied={} server_exit={} client={} reply_caller={} server={} stop={}",
            u8::from(applied_s && applied_s2),
            match stop4 {
                Some(Stop::Exit { code }) => code as i64,
                _ => -1,
            },
            state_c4.map(tcb_state_name).unwrap_or("none"),
            reply_caller.map(|t| t as u64).unwrap_or(0),
            state_s4.map(tcb_state_name).unwrap_or("none"),
            stop_name(stop4)
        );
    }

    // ================================================================ lượt 5: resume client
    let applied_c = user::apply_ipc_return(&mut tf_c, rig.tcb_c);
    let stop5 = if applied_c {
        run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT)
    } else {
        None
    };
    // Message **client nhận được** từ server — kernel đọc **độc lập** từ vùng nhận IPC của client
    // (không tin lời chương trình): label/MR phải đúng hợp đồng. Đọc **trước** ca âm quyền bên
    // dưới (lượt kế sẽ ghi đè vùng nhận IPC của client bằng reply `InsufficientRights`).
    let (cli_label, cli_mr0) = received(rig.tcb_c);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netsrv client: reply_label=0x{:x} reply_mr0={} const:label_reply=0x{:x} \
             const:rtt_ms={}",
            cli_label, cli_mr0, LABEL_REPLY, rtt_ms
        );
    }
    // **KT-306** — client gọi dịch vụ với request **hợp lệ** nhưng nó **không phải** net server
    // ⇒ chặn ở Send trên `EP_KERN` (dịch vụ sẽ từ chối ở lượt kế).
    let cli_blocked_kern = matches!(
        state_of(rig.tcb_c),
        Some(TcbState::BlockedOnSend { call: true, .. })
    );

    // ================================================================ dịch vụ kernel #3 — SAI CALLER
    let outcome3 = core_bridge::with_machine(|m| svc.service(m));
    let s3_served = svc.served;
    let s3_denied = svc.denied;
    let s3_failed = svc.failed;
    let s3_label = svc.last_label;
    let s3_len = svc.last_length;
    let s3_ip = svc.last_ip;
    let s3_caller = svc.last_caller;
    let s3_err = svc.last_err;
    let s3_sent = svc.sent;
    let s3_received = svc.received;
    let (cli_deny_label, cli_deny_mr) = received(rig.tcb_c);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netping kern: n=3 outcome={} served={} denied={} failed={} label=0x{:x} len={} \
             mr=0x{:x} caller={} err={} rtt_ms={} sent={} received={} reply_label=0x{:x} \
             reply_mr={} server={}",
            outcome_name(outcome3),
            s3_served,
            s3_denied,
            s3_failed,
            s3_label,
            s3_len,
            s3_ip,
            s3_caller,
            s3_err,
            svc.last_rtt_ms,
            s3_sent,
            s3_received,
            cli_deny_label,
            cli_deny_mr,
            state_of(rig.tcb_s).map(tcb_state_name).unwrap_or("none")
        );
    }

    // ================================================================ lượt 6: resume client (in ca âm quyền)
    let applied_c2 = user::apply_ipc_return(&mut tf_c, rig.tcb_c);
    let stop6 = if applied_c2 {
        run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT)
    } else {
        None
    };
    let state_c5 = state_of(rig.tcb_c);
    let state_s5 = state_of(rig.tcb_s);
    user::capture_end();

    // ================================================================ TAP 1..3
    let server_said_ping = user::captured_contains(SLOT_SERVER, b"# netsrv: ping 10.0.2.2 ok rtt=");
    let server_said_netping =
        user::captured_contains(SLOT_SERVER, b"# netping: label=0x291 reply=0 rtt_ms=");
    let server_said_neg_label =
        user::captured_contains(SLOT_SERVER, b"# netping: neg label err=5 ok=1");
    let client_said_reply = user::captured_contains(SLOT_CLIENT, b"# netsrv: client got reply=");
    let client_said_neg =
        user::captured_contains(SLOT_CLIENT, b"# netsrv: neg empty=1 reply=18 ok=1");
    let client_said_neg_caller =
        user::captured_contains(SLOT_CLIENT, b"# netping: neg caller err=3 ok=1");
    let (check_ipc, check_caps) =
        core_bridge::with_machine(|m| (m.check_ipc().is_ok(), m.check_caps().is_ok()));
    let reply_clean = reply_caller.is_none()
        && core_bridge::with_machine(|m| m.reply_caller(rig.reply_client).ok().flatten()).is_none()
        && core_bridge::with_machine(|m| m.reply_caller(rig.reply_kern).ok().flatten()).is_none();
    let client_exit = match stop6 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    let server_exit = match stop4 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    let mut fails = 0usize;
    // TAP 1 — server U-mode **thật** nhận `Ping(ip)`, gọi **dịch vụ kernel** `NetPing` bằng syscall
    // `Call` (KT-306: `net_svc::NetSvc`, không phải `virtio_net::ping` gọi thẳng trong rig), driver
    // ping **thật**; kernel đọc độc lập message + `PingStats` + reply trong vùng nhận IPC của server.
    let ok1 = blocked_send
        && ep_len == Some(1)
        && ep_mr == Some(TARGET_IP)
        && srv_label == LABEL_REQ
        && srv_mr == TARGET_IP
        && blocked_reply
        && srv_blocked_kern
        && kern_len == Some(1)
        && kern_mr == Some(TARGET_IP)
        && outcome1 == Outcome::Served
        && s1_served == 1
        && s1_denied == 0
        && s1_failed == 0
        && s1_label == LABEL_NETPING
        && s1_len == 1
        && s1_ip == TARGET_IP
        && s1_caller == u64::from(rig.tcb_s)
        && s1_err == 0
        && s1_sent == 1
        && s1_received == 1
        && srv_reply_label == LABEL_NETPING
        && srv_reply_mr == rtt_ms
        && server_said_ping
        && server_said_netping;
    if ok1 {
        PLATFORM.put_str("ok 1 - netsrv: server ran\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 1 - netsrv: server ran: blocked_send={} ep_len={} ep_mr=0x{:x} \
             srv_label=0x{:x} srv_mr=0x{:x} blocked_reply={} srv_blocked_kern={} kern_len={} \
             kern_mr=0x{:x} outcome1={} served={} denied={} failed={} label=0x{:x} len={} \
             ip=0x{:x} caller={} err={} sent={} received={} reply_label=0x{:x} reply_mr={} \
             rtt_ms={} said_ping={} said_netping={}",
            u8::from(blocked_send),
            ep_len.map(|n| n as u64).unwrap_or(u64::MAX),
            ep_mr.unwrap_or(0),
            srv_label,
            srv_mr,
            u8::from(blocked_reply),
            u8::from(srv_blocked_kern),
            kern_len.map(|n| n as u64).unwrap_or(u64::MAX),
            kern_mr.unwrap_or(0),
            outcome_name(outcome1),
            s1_served,
            s1_denied,
            s1_failed,
            s1_label,
            s1_len,
            s1_ip,
            s1_caller,
            s1_err,
            s1_sent,
            s1_received,
            srv_reply_label,
            srv_reply_mr,
            rtt_ms,
            u8::from(server_said_ping),
            u8::from(server_said_netping)
        );
        fails += 1;
    }
    // TAP 2 — server `Reply` ⇒ client **resume** với `Reply(rtt)`; ca âm đúng mã.
    let ok2 = applied_s
        && applied_s2
        && applied_c
        && applied_c2
        && client_exit == Some(0)
        && server_exit == Some(0)
        && client_said_reply
        && client_said_neg
        && cli_label == LABEL_REPLY
        && cli_mr0 == rtt_ms
        && reply_clean
        && check_ipc
        && check_caps;
    if ok2 {
        PLATFORM.put_str("ok 2 - netsrv: client got reply\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 2 - netsrv: client got reply: applied_s={} applied_s2={} applied_c={} \
             applied_c2={} \
             client_exit={} server_exit={} said_reply={} said_neg={} reply_label=0x{:x} \
             reply_mr0={} rtt_ms={} reply_clean={} check_ipc={} check_caps={}",
            u8::from(applied_s),
            u8::from(applied_s2),
            u8::from(applied_c),
            u8::from(applied_c2),
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(client_said_reply),
            u8::from(client_said_neg),
            cli_label,
            cli_mr0,
            rtt_ms,
            u8::from(reply_clean),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    // TAP 3 — **`NetPing` là syscall thật** (KT-306): dịch vụ kernel **kiểm quyền** rồi mới chạm
    // driver. Ca âm trong cùng boot: `Call` với **label sai** ⇒ `InvalidArgument` (5) và **không**
    // gói nào được gửi (`sent` vẫn 1). Đây là phép kiểm **quyền**, không phải phép kiểm định dạng.
    let ok3 = outcome2 == Outcome::Denied
        && s2_served == 1
        && s2_denied == 1
        && s2_failed == 0
        && s2_label == LABEL_BAD
        && s2_len == 1
        && s2_ip == TARGET_IP
        && s2_caller == u64::from(rig.tcb_s)
        && s2_err == u64::from(KError::InvalidArgument.id())
        && s2_sent == 1
        && s2_received == 1
        && srv_blocked_bad
        && bad_len == Some(1)
        && bad_mr == Some(TARGET_IP)
        && srv_deny_label == LABEL_BAD
        && srv_deny_mr == u64::from(KError::InvalidArgument.id())
        && server_said_neg_label
        // **Quyền**: request **hợp lệ** từ một TCB **không phải** net server ⇒ `InsufficientRights`
        // (3), vẫn **không** gói nào (`sent` = 1). Đây là nhánh kiểm `caller` của dịch vụ.
        && outcome3 == Outcome::Denied
        && s3_served == 1
        && s3_denied == 2
        && s3_failed == 0
        && s3_label == LABEL_NETPING
        && s3_len == 1
        && s3_ip == TARGET_IP
        && s3_caller == u64::from(rig.tcb_c)
        && s3_err == u64::from(KError::InsufficientRights.id())
        && s3_sent == 1
        && s3_received == 1
        && cli_blocked_kern
        && cli_deny_label == LABEL_NETPING
        && cli_deny_mr == u64::from(KError::InsufficientRights.id())
        && client_said_neg_caller;
    if ok3 {
        PLATFORM.put_str("ok 3 - netping: via ipc\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 3 - netping: via ipc: outcome2={} served={} denied={} failed={} \
             label=0x{:x} len={} ip=0x{:x} caller={} err={} sent={} received={} \
             srv_blocked_bad={} bad_len={} bad_mr=0x{:x} deny_label=0x{:x} deny_mr={} \
             said_neg_label={} outcome3={} s3_served={} s3_denied={} s3_label=0x{:x} \
             s3_caller={} s3_err={} s3_sent={} cli_blocked_kern={} cli_deny_label=0x{:x} \
             cli_deny_mr={} said_neg_caller={}",
            outcome_name(outcome2),
            s2_served,
            s2_denied,
            s2_failed,
            s2_label,
            s2_len,
            s2_ip,
            s2_caller,
            s2_err,
            s2_sent,
            s2_received,
            u8::from(srv_blocked_bad),
            bad_len.map(|n| n as u64).unwrap_or(u64::MAX),
            bad_mr.unwrap_or(0),
            srv_deny_label,
            srv_deny_mr,
            u8::from(server_said_neg_label),
            outcome_name(outcome3),
            s3_served,
            s3_denied,
            s3_label,
            s3_caller,
            s3_err,
            s3_sent,
            u8::from(cli_blocked_kern),
            cli_deny_label,
            cli_deny_mr,
            u8::from(client_said_neg_caller)
        );
        fails += 1;
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# netsrv final: client_exit={} server_exit={} reply_clean={} check_ipc={} \
             check_caps={} capture_client={} capture_server={} served={} denied={} sent={} \
             received={} client={} server={}",
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(reply_clean),
            u8::from(check_ipc),
            u8::from(check_caps),
            u8::from(client_said_reply),
            u8::from(server_said_ping),
            s3_served,
            s3_denied,
            s3_sent,
            s3_received,
            state_c5.map(tcb_state_name).unwrap_or("none"),
            state_s5.map(tcb_state_name).unwrap_or("none")
        );
    }
    if fails == 0 {
        PLATFORM.put_str("PANIC: KT-306 netping xong (3 TAP, 0 fail)\n");
        PLATFORM.halt(0)
    } else {
        PLATFORM.put_str("PANIC: KT-306 netping CO LOI\n");
        PLATFORM.halt(1)
    }
}

/// Entry hart **thứ cấp** (matrix `-smp 2/4`): adopt `satp`/`stvec` của BSP rồi park — KT-281
/// không cần hart phụ (ba tác vụ trên một hart), nhưng linker đòi symbol cho mọi ảnh.
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
