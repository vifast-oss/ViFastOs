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
    /// **KT-329** — ba TCB đích của TAP 11 (`Ready`, cspace `cnode_c`, có SC): `T5` = đường xanh
    /// (a)(b)(c); `T6` = `Resume` **khi cờ còn treo** (đột biến ii); `T7` = chạy lại **không** qua
    /// `Resume` (đột biến iii). `entry?` do **client** ghi (TAP 11), rig không đặt.
    pub tcb_t5: ObjId,
    pub tcb_t6: ObjId,
    pub tcb_t7: ObjId,
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
    // **KT-329** — ba TCB đích của TAP 11: cap đặt **thẳng** trong cspace client (ô 22/24/26, SC
    // ở 23/25/27) để không nới CNode gốc 16 ô. `entry` do **client** ghi ở pha 9b (TAP 11);
    // rig chỉ cấp cspace/SC/priority + `Ready` — đúng khuôn TCB đích của KT-314.
    let mut t11 = [0u32; 3];
    for (i, (tcb_slot, sc_slot)) in [
        (CS_TCB_T5, CS_SC_T5),
        (CS_TCB_T6, CS_SC_T6),
        (CS_TCB_T7, CS_SC_T7),
    ]
    .into_iter()
    .enumerate()
    {
        let tcb_cap = m
            .retype(SlotRef::new(root, RIG_UNTYPED), ObjType::Tcb, 0, 0)
            .expect("rig: retype t11 tcb");
        m.put_cap(cnode_c, tcb_slot, tcb_cap)
            .expect("rig: t11 tcb -> client");
        t11[i] = m.cap_obj(tcb_cap).expect("rig: obj t11 tcb");
        let sc_cap = m
            .retype(SlotRef::new(root, RIG_UNTYPED), ObjType::SchedContext, 0, 0)
            .expect("rig: retype t11 sc");
        m.put_cap(cnode_c, sc_slot, sc_cap)
            .expect("rig: t11 sc -> client");
        m.tcb_configure(
            SlotRef::new(cnode_c, tcb_slot),
            Some(SlotRef::new(cnode_c, CS_SELF)),
        )
        .expect("rig: t11 cspace");
        m.sched_control_configure(
            SlotRef::new(root, RIG_CTL),
            SlotRef::new(cnode_c, sc_slot),
            500,
            5_000,
        )
        .expect("rig: t11 SC");
        m.set_sched_params(
            SlotRef::new(cnode_c, tcb_slot),
            Some(SlotRef::new(cnode_c, sc_slot)),
        )
        .expect("rig: t11 params");
        m.tcb_set_priority(SlotRef::new(cnode_c, tcb_slot), PRIO_CLIENT, 10)
            .expect("rig: t11 priority");
        m.start_tcb(t11[i]).expect("rig: t11 ready");
    }
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
        tcb_t5: t11[0],
        tcb_t6: t11[1],
        tcb_t7: t11[2],
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

// =============================================================================================
// **KT-329** — TAP 11: `Tcb.Suspend(Defer)` + `Tcb.Resume` (đo **độc lập** ở kernel)
// =============================================================================================

/// Kết quả TAP 11 — mọi trường đọc từ **core** (`Machine`), không tin lời chương trình.
pub struct DeferTap {
    said: bool,
    calls: u32,
    state5: Option<TcbState>,
    state6: Option<TcbState>,
    state7: Option<TcbState>,
    entry5: Option<u64>,
    cfg5: u32,
    st5a: Option<TcbState>,
    entry5b: Option<u64>,
    res5: Option<u64>,
    st5b: Option<TcbState>,
    exit5: Option<u64>,
    st5c: Option<TcbState>,
    st6a: Option<TcbState>,
    res6: Option<u64>,
    exit6: Option<u64>,
    st7a: Option<TcbState>,
    exit7: Option<u64>,
    cfg7: u32,
    check_ipc: bool,
    check_caps: bool,
}

impl DeferTap {
    /// (a) sau `Defer` cả ba **vẫn** `Ready`; (b) T5 chạy **đúng một** syscall thành công rồi
    /// `Inactive`, `entry` giữ; (c) `Resume` trả `entry`, T5 chạy lại `Exit(0)`; T6 kiểm đột biến
    /// (ii) (`Resume` khi cờ còn treo phải xoá cờ ⇒ vẫn `Exit(0)`); T7 kiểm đột biến (iii) (điểm
    /// an toàn phải xoá cờ ⇒ chạy lại không qua `Resume` vẫn `Exit(0)`).
    pub fn ok(&self) -> bool {
        self.said
            && self.calls == 3
            && matches!(self.state5, Some(TcbState::Ready))
            && matches!(self.state6, Some(TcbState::Ready))
            && matches!(self.state7, Some(TcbState::Ready))
            && self.entry5.is_some_and(|e| e != 0)
            && self.cfg5 == 1
            && matches!(self.st5a, Some(TcbState::Inactive))
            && self.entry5b == self.entry5
            && self.res5 == self.entry5
            && matches!(self.st5b, Some(TcbState::Running { .. }))
            && self.exit5 == Some(0)
            && self.res6 == self.entry5
            && matches!(self.st6a, Some(TcbState::Running { .. }))
            && self.exit6 == Some(0)
            && matches!(self.st7a, Some(TcbState::Inactive))
            && self.exit7 == Some(0)
            && self.cfg7 == 2
            && self.check_ipc
            && self.check_caps
    }

    /// Dòng chẩn đoán `not ok 11` — in **đủ** trường để đọc log biết hỏng ở đâu.
    pub fn print_fail(&self) {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 11 - ipc: tcb suspend defer + resume: said={} calls={} st5={} st6={} st7={} entry5=0x{:x} cfg5={} st5a={} entry5b=0x{:x} res5=0x{:x} st5b={} exit5={} st5c={} res6=0x{:x} st6a={} exit6={} st7a={} exit7={} cfg7={} check_ipc={} check_caps={}",
            u8::from(self.said),
            self.calls,
            self.state5.map(crate::tcb_state_name).unwrap_or("none"),
            self.state6.map(crate::tcb_state_name).unwrap_or("none"),
            self.state7.map(crate::tcb_state_name).unwrap_or("none"),
            self.entry5.unwrap_or(0),
            self.cfg5,
            self.st5a.map(crate::tcb_state_name).unwrap_or("none"),
            self.entry5b.unwrap_or(0),
            self.res5.unwrap_or(0),
            self.st5b.map(crate::tcb_state_name).unwrap_or("none"),
            self.exit5.map(|c| c as i64).unwrap_or(-1),
            self.st5c.map(crate::tcb_state_name).unwrap_or("none"),
            self.res6.unwrap_or(0),
            self.st6a.map(crate::tcb_state_name).unwrap_or("none"),
            self.exit6.map(|c| c as i64).unwrap_or(-1),
            self.st7a.map(crate::tcb_state_name).unwrap_or("none"),
            self.exit7.map(|c| c as i64).unwrap_or(-1),
            self.cfg7,
            u8::from(self.check_ipc),
            u8::from(self.check_caps)
        );
    }
}

/// **KT-329** — chạy TAP 11: (a) đọc trạng thái **ngay** (T5/T6/T7 do client `Suspend(Defer)` ở
/// lượt 9; các lượt 10..12 không chạm chúng) + `entry`; (b) chạy T5 **một** lượt ⇒ đếm
/// `Tcb.Configure` thành công (phải **1**) rồi mới dừng ở điểm an toàn; (c) `Resume` (12) trả
/// `entry` và chạy tiếp ⇒ `Exit(0)`. T6/T7 là hai đường **đột biến** (ii)/(iii) — xem
/// [`DeferTap::ok`].
pub fn run_defer_tap(client: &vifast_plat_riscv64::user::LoadedInit, rig: &Rig) -> DeferTap {
    let said = crate::user::captured_contains(SLOT_CLIENT, b"# ipc: defer t5=0 t6=0 t7=0 entry=0x");
    // Không có lượt nào khác gọi `Defer` trong ảnh này ⇒ bộ đếm toàn cục = số `Defer` thành công
    // của lượt 9 (3). TAP 11 yêu cầu **đúng** 3.
    let calls = vifast_core::invoke::stats().deferred;
    let state5 = state_of(rig.tcb_t5);
    let state6 = state_of(rig.tcb_t6);
    let state7 = state_of(rig.tcb_t7);
    let entry5 = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_t5).ok());
    // (b) T5: đúng **một** `Tcb.Configure` thành công rồi điểm an toàn treo (`Inactive`).
    let cfg_before_t5 = vifast_core::invoke::stats().configure;
    let mut tf_t5 = TrapFrame::zeroed();
    tf_t5.x[REG_SP] = user::USER_STACK_TOP;
    tf_t5.sepc = entry5.unwrap_or(0);
    let _stop5a = run_turn(client, &mut tf_t5, rig.tcb_t5, SLOT_CLIENT);
    let cfg5 = vifast_core::invoke::stats()
        .configure
        .saturating_sub(cfg_before_t5);
    let st5a = state_of(rig.tcb_t5);
    let entry5b = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_t5).ok());
    // (c) `Resume` (12) — đường khôi phục duy nhất (spec **không** có `Restart`).
    let res5 = core_bridge::with_machine(|m| {
        m.tcb_resume_invocation(SlotRef::new(rig.cnode_c, CS_TCB_T5), 0)
            .ok()
    });
    let st5b = state_of(rig.tcb_t5);
    let stop5b = run_turn(client, &mut tf_t5, rig.tcb_t5, SLOT_CLIENT);
    let exit5 = exit_code(stop5b);
    let st5c = state_of(rig.tcb_t5);
    // T6 — `Resume` **trước khi** chạy (cờ còn treo): phải xoá cờ ⇒ hai syscall ⇒ `Exit(0)`.
    let entry6 = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_t6).ok());
    let res6 = core_bridge::with_machine(|m| {
        m.tcb_resume_invocation(SlotRef::new(rig.cnode_c, CS_TCB_T6), 0)
            .ok()
    });
    let st6a = state_of(rig.tcb_t6);
    let mut tf_t6 = TrapFrame::zeroed();
    tf_t6.x[REG_SP] = user::USER_STACK_TOP;
    tf_t6.sepc = entry6.unwrap_or(0);
    let stop6 = run_turn(client, &mut tf_t6, rig.tcb_t6, SLOT_CLIENT);
    let exit6 = exit_code(stop6);
    // T7 — **không** `Resume`: điểm an toàn phải xoá cờ; chạy lại bằng `set_running` của rig ⇒
    // syscall 2 chạy được ⇒ `Exit(0)`. Đột biến "bỏ xoá cờ" ⇒ treo lại ⇒ `exit7 = None`.
    let entry7 = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_t7).ok());
    let cfg_before_t7 = vifast_core::invoke::stats().configure;
    let mut tf_t7 = TrapFrame::zeroed();
    tf_t7.x[REG_SP] = user::USER_STACK_TOP;
    tf_t7.sepc = entry7.unwrap_or(0);
    let _stop7a = run_turn(client, &mut tf_t7, rig.tcb_t7, SLOT_CLIENT);
    let st7a = state_of(rig.tcb_t7);
    let stop7b = run_turn(client, &mut tf_t7, rig.tcb_t7, SLOT_CLIENT);
    let exit7 = exit_code(stop7b);
    let cfg7 = vifast_core::invoke::stats()
        .configure
        .saturating_sub(cfg_before_t7);
    let (check_ipc, check_caps) =
        core_bridge::with_machine(|m| (m.check_ipc().is_ok(), m.check_caps().is_ok()));
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc defer: state5={} state6={} state7={} calls={} cfg5={} st5a={} entry5=0x{:x} res5=0x{:x} st5b={} exit5={} st5c={} st6a={} res6=0x{:x} exit6={} st7a={} exit7={} cfg7={} stop5a={} stop7a={}",
            state5.map(crate::tcb_state_name).unwrap_or("none"),
            state6.map(crate::tcb_state_name).unwrap_or("none"),
            state7.map(crate::tcb_state_name).unwrap_or("none"),
            calls,
            cfg5,
            st5a.map(crate::tcb_state_name).unwrap_or("none"),
            entry5.unwrap_or(0),
            res5.unwrap_or(0),
            st5b.map(crate::tcb_state_name).unwrap_or("none"),
            exit5.map(|c| c as i64).unwrap_or(-1),
            st5c.map(crate::tcb_state_name).unwrap_or("none"),
            st6a.map(crate::tcb_state_name).unwrap_or("none"),
            res6.unwrap_or(0),
            exit6.map(|c| c as i64).unwrap_or(-1),
            st7a.map(crate::tcb_state_name).unwrap_or("none"),
            exit7.map(|c| c as i64).unwrap_or(-1),
            cfg7,
            crate::stop_name(_stop5a),
            crate::stop_name(_stop7a)
        );
    }
    DeferTap {
        said,
        calls,
        state5,
        state6,
        state7,
        entry5,
        cfg5,
        st5a,
        entry5b,
        res5,
        st5b,
        exit5,
        st5c,
        st6a,
        res6,
        exit6,
        st7a,
        exit7,
        cfg7,
        check_ipc,
        check_caps,
    }
}

/// Mã `Stop::Exit` (nếu lượt chạy kết thúc bằng `Exit`).
fn exit_code(stop: Option<Stop>) -> Option<u64> {
    match stop {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    }
}
