// SPDX-License-Identifier: MPL-2.0
//! **KT-322** — rig của ảnh `ktest-ipc`, tách khỏi `ktest_ipc.rs` để giữ **R1** (≤ 1.500 dòng,
//! AGENTS §4 luật cấu trúc). Đây là **cùng một** rig của KT-280..KT-320: dựng `Machine`, cấp
//! cap cho hai cspace, tạo bốn TCB đích của KT-314/KT-319 và object `VSpace` đại diện của
//! KT-320. **Không** đổi một byte ngữ nghĩa nào — chỉ dời chỗ (hành vi do `expected.txt` +
//! TAP 1..9 chứng minh).
//!
//! Module con của **crate root** (`ktest_ipc.rs` là bin crate root): `use crate::*;` để thấy
//! hằng/`static`/`use` của rig cũ — quyền truy cập private của module cha vẫn đúng vì đây là
//! module **con**.

#![allow(dead_code)]

use crate::*;

/// Rig của pha IPC — mọi thứ thuộc kernel, không phải chương trình.
pub struct Rig {
    pub tcb_c: ObjId,
    pub tcb_s: ObjId,
    pub ep: ObjId,
    pub reply: ObjId,
    pub cnode_c: ObjId,
    pub cnode_s: ObjId,
    /// **KT-311** — CSpace **thứ hai** của client (`cspace?` đổi sang rồi trả về).
    pub cnode_alt: ObjId,
    /// **KT-282** — object `Frame` 4 KiB đã bind làm IPC buffer cho **cả hai** TCB.
    pub ipc_buf: ObjId,
    /// **KT-314** — CNode gốc của rig (đo **độc lập** `Tcb.Resume` bằng model API).
    pub root: ObjId,
    /// **KT-314** — TCB đích (được `Resume` từ U-mode) và TCB đích `entry = 0` (ca âm).
    pub tcb_t: ObjId,
    pub tcb_t2: ObjId,
    /// **KT-320** — object **đại diện** `VSpace` của client trong core (khoá `foreign_key` trỏ
    /// VSpace **thật** của object layer — D2-83). `TcbData.vspace` sau `Configure(vspace?)` phải
    /// **bằng đúng** giá trị này, và `user_run::vspace_applied` phải báo nó đã được tiêu thụ.
    pub vspace_obj: ObjId,
    /// **KT-325** — `VSpace` **thứ hai** (object layer **thật**, root **khác** ảnh đã nạp) +
    /// object đại diện trong core: "VSpace B" của TAP 10 — hai TCB cấu hình **hai** VSpace khác
    /// nhau ⇒ `user_run::vspace_applied(tcb)` phải **khác nhau** theo từng TCB.
    pub vspace_obj2: ObjId,
    /// **KT-319** — TCB đích thứ ba: rig để `Ready`, U-mode `Suspend` nó (không `Resume`).
    pub tcb_t3: ObjId,
    /// **KT-319** — TCB đích thứ tư: rig để **`BlockedOnSend`** trên Endpoint riêng (ca âm
    /// "đường invocation **không** nhận `BlockedOn*`"); kernel đo **độc lập** trạng thái **không**
    /// đổi sau lượt U-mode.
    pub tcb_t4: ObjId,
}

/// Đưa TCB về `Running{hart:0}` — cùng khuôn `ktest_capx::set_running` (chỉ `Inactive` mới
/// `start_tcb`; `Ready` đi thẳng qua `transition_tcb`).
pub fn set_running(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    if !matches!(m.tcb_state(tcb)?, TcbState::Running { .. }) {
        if m.tcb_state(tcb)? == TcbState::Inactive {
            m.start_tcb(tcb)?;
        }
        m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })?;
    }
    Ok(())
}

/// Dựng CNode 16 slot + 2 TCB + Endpoint + Reply + SchedControl + 2 SC + 2 cspace con + **Frame
/// 4 KiB cho IPC buffer** (KT-282).
///
/// Bố cục cspace con (giống nhau hai bên để chương trình U-mode hardcode một bảng):
/// `0` = cap `CNode` của chính nó, `1` = `Endpoint`, `2` = `Reply`; ô `9` để **rỗng** (ca âm
/// "Call trên ô rỗng").
pub fn build_rig(m: &mut Machine, u: ObjId, load_untyped: CapId, client_vspace: CapId) -> Rig {
    let root = m.add_cnode(16);
    let us = SlotRef::new(root, RIG_UNTYPED);
    let uc = m.root_cap(u);
    m.put_cap(root, RIG_UNTYPED, uc).expect("rig: cap Untyped");
    // **KT-314** — bảng `(ô, loại)` **tường minh** (trước KT-314 là `RIG_TCB_C + i`, chỉ đúng vì
    // các ô 1..7 liền nhau; thêm TCB đích ở ô 10..13 thì phải khai rõ để không đè `RIG_CSPACE`=8
    // / `RIG_IPC_BUF`=9).
    let objs: [(usize, ObjType); 13] = [
        (RIG_TCB_C, ObjType::Tcb),
        (RIG_TCB_S, ObjType::Tcb),
        (RIG_EP, ObjType::Endpoint),
        (RIG_REPLY, ObjType::Reply),
        (RIG_CTL, ObjType::SchedControl),
        (RIG_SC_C, ObjType::SchedContext),
        (RIG_SC_S, ObjType::SchedContext),
        // **KT-314** — hai TCB đích + hai `SchedContext` (KT-20: `Running ⇒ còn budget`).
        (RIG_TCB_T, ObjType::Tcb),
        (RIG_TCB_T2, ObjType::Tcb),
        (RIG_SC_T, ObjType::SchedContext),
        (RIG_SC_T2, ObjType::SchedContext),
        // **KT-319** — TCB đích thứ ba (`Ready`) + `SchedContext` của nó.
        (RIG_TCB_T3, ObjType::Tcb),
        (RIG_SC_T3, ObjType::SchedContext),
    ];
    let mut obj = [0u32; 13];
    for (i, (slot, ty)) in objs.iter().enumerate() {
        let cap = m.retype(us, *ty, 0, 0).expect("rig: retype object lõi");
        m.put_cap(root, *slot, cap).expect("rig: đặt cap");
        obj[i] = m.cap_obj(cap).expect("rig: cap_obj");
    }
    let cs = m.root_cap(root);
    m.put_cap(root, RIG_CSPACE, cs).expect("rig: cap cspace");
    // **KT-282** — Frame 4 KiB cho IPC buffer: object layer cấp (D2-83), core giữ **đại diện**
    // (`add_foreign_object`) + cap gốc; `Tcb.Configure` bind cap đó + VA của buffer.
    let ipc_buf = {
        let frame_key = {
            let am = aobj::machine();
            let cap = am
                .retype(load_untyped, ObjKind::Frame, 12)
                .expect("rig: frame 4 KiB");
            let id = am.cap_key(cap).expect("rig: frame key");
            mem_bridge::make_key(cap, id)
        };
        let frame_obj = m
            .add_foreign_object(ObjType::Frame, frame_key)
            .expect("rig: đại diện Frame");
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, RIG_IPC_BUF, frame_cap)
            .expect("rig: cap ipc buffer");
        frame_obj
    };
    // **KT-314** — 32 ô (16 ô cũ + cap `Tcb` server/đích/đích-2 ở ô 14..16).
    let cnode_c = m.add_cnode(32);
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
    // **KT-290** — client giữ cap `Tcb` của chính nó + cap `Frame` IPC buffer để **tự** gọi
    // `Tcb.Configure` qua invocation (rig **không** bind hộ client nữa).
    m.copy(SlotRef::new(root, RIG_TCB_C), SlotRef::new(cnode_c, CS_TCB))
        .expect("rig: tcb -> client");
    m.copy(
        SlotRef::new(root, RIG_IPC_BUF),
        SlotRef::new(cnode_c, CS_FRAME),
    )
    .expect("rig: frame -> client");
    // **KT-309** — hai cap `Frame` làm **nguồn** extra cap: một cap **có** `RIGHT_GRANT` (đường
    // xanh) và một cap **thiếu** `RIGHT_GRANT` (ca âm (i) ⇒ `InsufficientRights` 3).
    m.copy(
        SlotRef::new(root, RIG_IPC_BUF),
        SlotRef::new(cnode_c, CS_FRAME_GRANT),
    )
    .expect("rig: frame grant -> client");
    m.copy_rights(
        SlotRef::new(root, RIG_IPC_BUF),
        SlotRef::new(cnode_c, CS_FRAME_NO_GRANT),
        Some(RIGHTS_ALL & !RIGHT_GRANT),
    )
    .expect("rig: frame no-grant -> client");
    // **KT-311** — CSpace **thứ hai** (`cnode_alt`) cho tham số `cspace?`:
    //   * ô 0 = cap tới **CSpace gốc** (`cnode_c`) — client trả `cspace` về;
    //   * ô 3 = cap `Tcb` của client — gọi `Configure` khi CSpace đang là `cnode_alt`;
    //   * ô 8 = cap `Reply` — **chỉ có** ở đây ⇒ `Call` trên ô 8 trả 18 khi CSpace đã đổi,
    //     1 khi chưa (chứng minh `cspace?` thật sự đổi CSpace, không chỉ ghi trường).
    let cnode_alt = m.add_cnode(16);
    let back_cap = m.root_cap(cnode_c);
    m.put_cap(cnode_alt, CS_ALT_BACK, back_cap)
        .expect("rig: cnode_alt -> cnode_c");
    m.copy(
        SlotRef::new(root, RIG_TCB_C),
        SlotRef::new(cnode_alt, CS_ALT_TCB),
    )
    .expect("rig: tcb -> cnode_alt");
    m.copy(
        SlotRef::new(root, RIG_REPLY),
        SlotRef::new(cnode_alt, CS_ALT_ONLY),
    )
    .expect("rig: reply -> cnode_alt");
    // Ô 4 = cap `Frame` IPC buffer (client truyền `SLOT_FRAME` = 4 làm `ipc_buffer?` **cả khi**
    // CSpace đang là `cnode_alt` — CPTR giải trong CSpace **caller**, không phải CSpace gốc).
    m.copy(
        SlotRef::new(root, RIG_IPC_BUF),
        SlotRef::new(cnode_alt, CS_FRAME),
    )
    .expect("rig: frame -> cnode_alt");
    let alt_cap = m.root_cap(cnode_alt);
    m.put_cap(cnode_c, CS_ALT_CNODE, alt_cap)
        .expect("rig: cnode_alt -> client");
    // **KT-314** — cap `Tcb` của **server** (đã `Exit`) + **hai TCB đích** vào cspace client:
    // client tự gọi `Tcb.Resume` (method 12) trên chúng.
    m.copy(
        SlotRef::new(root, RIG_TCB_S),
        SlotRef::new(cnode_c, CS_TCB_S),
    )
    .expect("rig: tcb server -> client");
    m.copy(
        SlotRef::new(root, RIG_TCB_T),
        SlotRef::new(cnode_c, CS_TCB_T),
    )
    .expect("rig: tcb dich -> client");
    m.copy(
        SlotRef::new(root, RIG_TCB_T2),
        SlotRef::new(cnode_c, CS_TCB_T2),
    )
    .expect("rig: tcb dich 2 -> client");
    // **KT-320** — object **đại diện** `VSpace` của client trong core + cap ở ô `CS_VSPACE` của
    // cspace client (giá trị `MR1` của `Tcb.Configure` tham số `vspace?`).
    //
    // D2-83: core **không** sở hữu PTE/PA — nó giữ **danh tính** object qua `foreign_key`
    // (`(object_id << 16) | cap_index` của object layer, cùng khuôn `Frame.Map`), nên
    // `user_run::resolve_vspace` giải ra đúng `(root_pa, asid)` của VSpace **thật** mà loader đã
    // dựng cho client (`LoadedInit::root`/`::asid` chính là `vspace_root_pa`/`vspace_asid`).
    let vspace_obj = {
        let id = aobj::machine()
            .cap_key(client_vspace)
            .expect("rig: cap_key vspace client");
        let key = mem_bridge::make_key(client_vspace, id);
        let obj = m
            .add_foreign_object(ObjType::VSpace, key)
            .expect("rig: vspace dai dien");
        let cap = m.root_cap(obj);
        m.put_cap(cnode_c, CS_VSPACE, cap)
            .expect("rig: vspace -> client");
        obj
    };
    // **KT-325** — **VSpace thứ hai** (object layer **thật**): root PageTable 4 KiB **mới** (chưa
    // map ảnh, không chép nửa thấp root kernel — chỉ dùng để **giải danh tính**, không bao giờ
    // `sret` vào nó). Object đại diện trong core + cap ở ô `CS_VSPACE2` của cspace client: TAP 10
    // cấu hình nó cho TCB thứ hai; arch giải ra root **khác** `loaded.root` ⇒ `match=0`, **từ
    // chối** vào U-mode (fail-closed) — nhưng **bản ghi** `vspace_applied` của TCB đó vẫn là object
    // này, **không** phải giá trị của TCB thứ nhất.
    let vspace_obj2 = {
        let am = aobj::machine();
        let cap = am
            .retype(load_untyped, ObjKind::VSpace, 0)
            .expect("rig: vspace thu hai");
        let id = am.cap_key(cap).expect("rig: cap_key vspace thu hai");
        let key = mem_bridge::make_key(cap, id);
        let obj = m
            .add_foreign_object(ObjType::VSpace, key)
            .expect("rig: vspace dai dien 2");
        let cap_core = m.root_cap(obj);
        m.put_cap(cnode_c, CS_VSPACE2, cap_core)
            .expect("rig: vspace 2 -> client");
        obj
    };
    // **KT-319** — cap `Tcb` đích thứ ba (đủ quyền — đường xanh `Suspend`) + bản **chỉ đọc**
    // (mint `RIGHT_READ` từ chính cap đó — ca âm quyền 3).
    m.copy(
        SlotRef::new(root, RIG_TCB_T3),
        SlotRef::new(cnode_c, CS_TCB_T3),
    )
    .expect("rig: tcb dich 3 -> client");
    m.copy_rights(
        SlotRef::new(root, RIG_TCB_T3),
        SlotRef::new(cnode_c, CS_TCB_RO),
        Some(vifast_core::RIGHT_READ),
    )
    .expect("rig: tcb dich 3 ro -> client");
    // **KT-319** — TCB đích thứ tư, **`BlockedOnSend`** trên một `Endpoint` **riêng**: bảng 11×11
    // **cho** `BlockedOnSend → Inactive`, nên ca âm này chỉ bị chặn bởi **kiểm trạng thái** của
    // đường invocation (mutation "bỏ kiểm trạng thái" ⇒ treo tác vụ đang chờ IPC ⇒ TAP 8 đỏ).
    // Cap đặt **thẳng** vào cspace client (ô 19) để không phải nới CNode gốc 16 ô.
    let t4_cap = m
        .retype(SlotRef::new(root, RIG_UNTYPED), ObjType::Tcb, 0, 0)
        .expect("rig: retype dich 4");
    m.put_cap(cnode_c, CS_TCB_T4, t4_cap)
        .expect("rig: tcb dich 4 -> client");
    let tcb_t4 = m.cap_obj(t4_cap).expect("rig: obj dich 4");
    let ep2_cap = m
        .retype(SlotRef::new(root, RIG_UNTYPED), ObjType::Endpoint, 0, 0)
        .expect("rig: retype ep2");
    m.put_cap(cnode_s, CS_SRV_EP2, ep2_cap)
        .expect("rig: ep2 -> server");
    let ep2 = m.cap_obj(ep2_cap).expect("rig: obj ep2");
    m.transition_tcb(
        tcb_t4,
        NonReadyTarget::BlockedOnSend {
            ep: ep2,
            call: false,
            fault: None,
        },
    )
    .expect("rig: dich 4 blocked on send");
    // Cấu hình 2 TCB: cspace riêng + SC + priority (server cao hơn).
    // **KT-290**: client **không** được rig bind IPC buffer — chính U-mode bind qua
    // `Tcb.Configure` invocation (TAP 4). Server vẫn do rig bind (server không làm Configure).
    m.tcb_configure(
        SlotRef::new(root, RIG_TCB_C),
        Some(SlotRef::new(cnode_c, CS_SELF)),
    )
    .expect("rig: cspace client");
    m.tcb_configure_with_ipc_buffer(
        SlotRef::new(root, RIG_TCB_S),
        Some(SlotRef::new(cnode_s, CS_SELF)),
        Some((
            SlotRef::new(root, RIG_IPC_BUF),
            vifast_plat_riscv64::ipc_buffer::USER_IPC_BUFFER_VA,
        )),
    )
    .expect("rig: cspace + ipc buffer server");
    // **KT-314** — hai TCB đích: cspace (để `Tcb.Configure` giải CPTR trong CSpace **caller** và
    // để syscall của đích chạy được) + SC (KT-20) + priority. `entry` **không** đặt ở đây: đích
    // thứ nhất nhận `entry?` từ **chính client** (đường xanh), đích thứ hai giữ `0` (ca âm).
    for slot in [RIG_TCB_T, RIG_TCB_T2, RIG_TCB_T3] {
        m.tcb_configure(
            SlotRef::new(root, slot),
            Some(SlotRef::new(cnode_c, CS_SELF)),
        )
        .expect("rig: cspace dich");
    }
    for (tcb_slot, sc, prio) in [
        (RIG_TCB_C, RIG_SC_C, PRIO_CLIENT),
        (RIG_TCB_S, RIG_SC_S, PRIO_SERVER),
        (RIG_TCB_T, RIG_SC_T, PRIO_CLIENT),
        (RIG_TCB_T2, RIG_SC_T2, PRIO_CLIENT),
        (RIG_TCB_T3, RIG_SC_T3, PRIO_CLIENT),
    ] {
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
    // **KT-319** — đích thứ ba `Ready` **trước** lượt U-mode: đường xanh `Suspend` trên `Ready`
    // (đích thứ nhất, `Inactive`, sẽ `Resume`; đích thứ ba không bao giờ chạy). `entry` để `0` —
    // nó **không** được `Resume`, và `Suspend` không đọc `entry`.
    m.start_tcb(obj[11]).expect("rig: dich 3 ready");
    Rig {
        tcb_c: obj[0],
        tcb_s: obj[1],
        ep: obj[2],
        reply: obj[3],
        cnode_c,
        cnode_s,
        cnode_alt,
        ipc_buf,
        root,
        tcb_t: obj[7],
        tcb_t2: obj[8],
        vspace_obj,
        vspace_obj2,
        tcb_t3: obj[11],
        tcb_t4,
    }
}

/// Chạy **một lượt** tác vụ U-mode: gắn TCB core + đưa `Running` + `sret`; trả `Stop` và cập nhật
/// `tf` bằng trapframe **đã lưu** (KT-280: lượt sau resume từ đúng chỗ dừng).
pub fn run_turn(loaded: &LoadedInit, tf: &mut TrapFrame, tcb: ObjId, slot: usize) -> Option<Stop> {
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
pub fn state_of(tcb: ObjId) -> Option<TcbState> {
    core_bridge::with_machine(|m| m.tcb_state(tcb).ok())
}

/// **KT-282** — `(length, MR4, MR_cuối)` của message **vừa nhận** ở TCB (đọc từ `tcb_ipc_area`
/// của core — kernel không tin lời chương trình). `MR4` = word 0 của vùng buffer; `MR_cuối` =
/// `MR(length-1)` ⇒ vòng 8 MR cho MR7, vòng 64 MR cho MR63 (biên trên của vùng 60 word).
pub fn area_of(tcb: ObjId) -> Option<(u8, u64, u64)> {
    core_bridge::with_machine(|m| {
        m.tcb_ipc_area(tcb).ok().and_then(|area| {
            area.info.map(|info| {
                let len = info.length() as usize;
                let last = if len == 0 {
                    0
                } else {
                    area.mr(len - 1).unwrap_or(0)
                };
                (info.length(), area.mr(4).unwrap_or(0), last)
            })
        })
    })
}
