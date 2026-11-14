// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-26 — **Fault + Exit + mô hình procsrv** (K1C-PLAN §5 KT-26; D2-53 Q5/Q10, D2-25, D2-55).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** `FaultKind` có **4 biến thể** (`Exit`, `Vm`,
//! `UnknownSyscall`, `UserException`) nhưng `spec/abi.toml:268-271` **vẫn chỉ có `Exit`**:
//! spec là **tập con** tới K3 (D2-53 **Q5** = (a)), KT-26 **không** sửa `spec/`.
//!
//! Nguồn ngữ nghĩa: thiết kế §4.6 `:321` ("Fault: Call tới fault endpoint **với Reply**; trả lời
//! Reply để tiếp tục, **có thể kèm thanh ghi mới**"), §4.4 `:279` ("Không ô `resume`: message ghi
//! thẳng vào context + IPC buffer rồi mới chuyển Ready. Đường mất message kiểu r319 không biểu
//! diễn được"), §4.4 `:285` (Exit, D2-25), §6.4 (`wait` → `{code, how}`, `kill` =
//! `Suspend(Abort)` + Revoke cap TCB — **Q10** chỉ mô hình).
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt26-red-compile.log`](../../agent/JOURNAL/kt26-red-compile.log)
//! (`E0599`: `Machine::inject_fault`/`tcb_resume_with_regs`/`tcb_regs`/`tcb_last_fault`,
//! `FaultAccess`, `procsrv_model`; `E0433`) và
//! [`JOURNAL/kt26-red-semantics.log`](../../agent/JOURNAL/kt26-red-semantics.log) (probe tạm:
//! `FaultKind` chỉ có `Exit`; fault không có đường Call+Reply; `tcb_resume` **không** ghi thanh ghi
//! và **xoá** `exit_code`).

use vifast_sim::ipc::{Message, MessageInfo, WORDS_IN_REGISTERS};
use vifast_sim::kobj::{
    FaultAccess, FaultKind, KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbState,
    VmFaultReason,
};
use vifast_sim::procsrv_model::{ProcsrvModel, WaitHow, WaitResult};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` **không** cap; `words.len()` phải bằng `info.length()`.
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

fn msg_empty() -> Message {
    msg(0, &[])
}

// ---------------------------------------------------------------------------
// Rig: CNode 64 slot; 1 Endpoint làm `fault_ep`; 1 Reply (procsrv cấp khi Recv);
// 4 TCB (mỗi TCB có SchedContext + priority 5 — test-only D2-50): `t[0]`/`t[2]` là tiến
// trình con (được gắn `fault_ep`), `t[1]` là procsrv, `t[3]` là tiến trình **không** `fault_ep`.
// ---------------------------------------------------------------------------

const S_EP: usize = 0;
const S_REP: usize = 1;
const S_TCB: usize = 10;
const S_SC: usize = 20;
const S_CTL: usize = 40;

struct Rig {
    m: Machine,
    c0: ObjId,
    ep: ObjId,
    ep_s: SlotRef,
    rep: ObjId,
    rep_s: SlotRef,
    t: [ObjId; 4],
    t_s: [SlotRef; 4],
    procsrv: ProcsrvModel,
}

impl Rig {
    fn new() -> Self {
        Self::new_variant(true)
    }

    /// `procsrv_has_sc = false` ⇒ procsrv **passive** (không `SchedContext` riêng) để kiểm
    /// donation đi theo đường fault `Call`→`Reply` (KT-21).
    fn new_variant(procsrv_has_sc: bool) -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(64);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c0, S_CTL, ctl_cap).expect("cap SchedControl");
        let ctl_s = slot(c0, S_CTL);

        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, S_EP, ep_cap).expect("cap fault_ep");
        let ep_s = slot(c0, S_EP);

        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, S_REP, rep_cap).expect("cap Reply");
        let rep_s = slot(c0, S_REP);

        let mut t = [0u32; 4];
        let mut t_s = [slot(c0, S_TCB); 4];
        for (i, obj) in t.iter_mut().enumerate() {
            *obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(*obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            t_s[i] = slot(c0, S_TCB + i);

            if i != 1 || procsrv_has_sc {
                let sc = m.add_kernel_object(ObjType::SchedContext);
                let sc_cap = m.root_cap(sc);
                m.put_cap(c0, S_SC + i, sc_cap).expect("cap SchedContext");
                let sc_slot = slot(c0, S_SC + i);
                m.sched_control_configure(ctl_s, sc_slot, 4, 8)
                    .expect("SchedControl.Configure");
                m.set_sched_params(t_s[i], Some(sc_slot))
                    .expect("set_sched_params (test-only)");
            }
            m.tcb_set_priority(t_s[i], 5, 255).expect("Tcb.SetPriority");
            m.transition_tcb(*obj, TcbState::Ready)
                .expect("Inactive→Ready");
        }
        // `fault_ep` cho hai tiến trình con; procsrv/`t[3]` **không** cần.
        for i in [0usize, 2] {
            m.tcb_set_fault_ep(t_s[i], ep_s).expect("Tcb.SetFaultEp");
        }
        let mut procsrv = ProcsrvModel::new(t[1], ep_s, rep_s);
        for i in [0usize, 2, 3] {
            procsrv.track(t[i], t_s[i]);
        }
        Self {
            m,
            c0,
            ep,
            ep_s,
            rep,
            rep_s,
            t,
            t_s,
            procsrv,
        }
    }

    fn kiem(&self, buoc: &str) {
        if let Err(e) = self.m.check_sched() {
            panic!("check_sched sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_mailboxes() {
            panic!("check_mailboxes sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_ipc() {
            panic!("check_ipc sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_caps() {
            panic!("check_caps sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_all_bytes() {
            panic!("check_all_bytes sau {buoc}: {e}");
        }
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn cho_fault(&mut self, i: usize, kind: FaultKind, buoc: &str) {
        self.m.inject_fault(self.t[i], kind).expect("inject_fault");
        assert_eq!(
            self.st(i),
            TcbState::BlockedOnSend {
                ep: self.ep,
                call: true,
                fault: Some(kind)
            },
            "{buoc}: fault ≠ Exit phải đi **Call** tới fault_ep"
        );
        self.kiem(buoc);
    }

    /// procsrv `Recv` (có Reply cap) để nhận fault.
    fn recv_fault(&mut self, buoc: &str) {
        self.procsrv
            .recv_exit(&mut self.m)
            .unwrap_or_else(|e| panic!("recv_exit sau {buoc}: {e:?}"));
        self.kiem(buoc);
    }

    /// procsrv `Reply` với thanh ghi mới cho caller.
    fn reply_regs(&mut self, i: usize, words: &[u64], buoc: &str) {
        let m = msg(0, words);
        self.m
            .reply_msg(self.t[1], self.rep_s, m)
            .unwrap_or_else(|e| panic!("procsrv Reply sau {buoc}: {e:?}"));
        self.kiem(buoc);
        let _ = i;
    }
}

// ===========================================================================
// 1. `FaultKind` — 4 biến thể model-only + `name`/`render`/`is_exit`/`exit_code`.
// ===========================================================================

#[test]
fn fault_kind_bon_bien_the_model_only() {
    let exit = FaultKind::Exit { code: 9 };
    // **KT-41/D2-65 Q6** — `Vm` mang `vspace`/`va`/`reason`; payload fault vẫn là model-only.
    let vm = FaultKind::Vm {
        vspace: 1,
        va: 0xDEAD_0000,
        access: FaultAccess::Write,
        reason: VmFaultReason::NotMapped,
    };
    let unk = FaultKind::UnknownSyscall { nr: 77 };
    let usr = FaultKind::UserException {
        ip: 0x4000,
        kind: 3,
    };
    assert_eq!(exit.name(), "Exit");
    assert_eq!(vm.name(), "Vm");
    assert_eq!(unk.name(), "UnknownSyscall");
    assert_eq!(usr.name(), "UserException");
    assert!(exit.is_exit(), "chỉ Exit mới đi Send không Reply (D2-25)");
    assert!(!vm.is_exit());
    assert!(!unk.is_exit());
    assert!(!usr.is_exit());
    assert_eq!(exit.exit_code(), Some(9));
    assert_eq!(vm.exit_code(), None);

    // `render` phải phân biệt được các biến thể (dùng cho `expect_states`/trace).
    assert_eq!(exit.render(), "Exit{9}");
    assert!(vm.render().contains("va=3735879680"));
    assert!(vm.render().contains("access=Write"));
    assert!(vm.render().contains("reason=NotMapped"));
    assert!(unk.render().contains("nr=77"));
    assert!(usr.render().contains("ip=16384"));
    assert!(usr.render().contains("kind=3"));

    // Trạng thái chặn render đúng `FaultKind` mới (không rơi về "none").
    let st = TcbState::BlockedOnSend {
        ep: 7,
        call: true,
        fault: Some(unk),
    };
    assert!(st.render().contains("UnknownSyscall{nr=77}"));

    // `FaultAccess` có tên ổn định.
    assert_eq!(FaultAccess::Read.name(), "Read");
    assert_eq!(FaultAccess::Write.name(), "Write");
    assert_eq!(FaultAccess::Execute.name(), "Execute");
}

// ===========================================================================
// 2. Fault ≠ Exit đi **Call + Reply**; Reply ghi thanh ghi **trước** `Ready` (r319).
// ===========================================================================

#[test]
fn fault_unknown_syscall_qua_call_roi_reply_ghi_thanh_ghi() {
    let mut r = Rig::new();
    let kind = FaultKind::UnknownSyscall { nr: 77 };
    r.cho_fault(0, kind, "inject UnknownSyscall");
    // Fault đi **Call** ⇒ nằm trong hàng gửi, chờ procsrv Recv (không phải Send).
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));

    r.recv_fault("procsrv Recv");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnReply { reply: r.rep },
        "fault Call ⇒ caller chờ Reply"
    );
    assert_eq!(
        r.procsrv.last_fault(r.t[0]),
        Some(kind),
        "procsrv phải biết (tcb, fault) vừa nhận"
    );
    assert_eq!(r.m.ipc_sent(), 0, "fault không phải message người dùng");

    r.reply_regs(0, &[0x11, 0x22, 0x33, 0x44], "Reply thanh ghi");
    assert_eq!(r.st(0), TcbState::Ready, "Reply ⇒ caller Ready");
    // **Ghi trước Ready**: đọc ngay sau `reply` phải thấy thanh ghi mới (chống r319).
    assert_eq!(
        r.m.tcb_regs(r.t[0]).expect("tcb_regs"),
        [0x11, 0x22, 0x33, 0x44]
    );
    assert_eq!(
        r.m.tcb_ipc_info(r.t[0])
            .expect("ipc")
            .expect("đã nhận")
            .length(),
        4
    );
    assert_eq!(r.m.ipc_sent(), 1, "chỉ Reply là message người dùng");
    assert_eq!(r.m.ipc_delivered(), 1);
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(0),
        "không phát lại fault cũ"
    );
    r.kiem("sau Reply");
}

#[test]
fn fault_vm_va_user_exception_cung_duong_call_reply() {
    for (i, kind) in [
        FaultKind::Vm {
            vspace: 1,
            va: 0x1_0000,
            access: FaultAccess::Execute,
            reason: VmFaultReason::Permission,
        },
        FaultKind::UserException {
            ip: 0x8000,
            kind: 4,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut r = Rig::new();
        r.cho_fault(0, kind, "inject");
        r.recv_fault("procsrv Recv");
        assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });
        // 6 MR: 4 vào "thanh ghi", 2 vào "IPC buffer model" — cùng `ipc::transfer`.
        r.reply_regs(0, &[10, 20, 30, 40, 50, 60], "Reply 6 MR");
        assert_eq!(r.st(0), TcbState::Ready);
        assert_eq!(
            r.m.tcb_regs(r.t[0]).expect("regs"),
            [10, 20, 30, 40],
            "case {i}: MR0..MR3"
        );
        assert_eq!(
            r.m.tcb_ipc_buffer(r.t[0]).expect("buffer")[..2],
            [50, 60],
            "case {i}: MR4..MR5 vào buffer"
        );
        r.kiem("case");
    }
}

// ===========================================================================
// 3. `tcb_resume_with_regs` — một cửa ghi thanh ghi **trước** khi `Ready`, không phát lại IPC.
// ===========================================================================

#[test]
fn resume_with_regs_khi_dang_cho_reply_ghi_thanh_ghi_truoc_ready() {
    let mut r = Rig::new();
    let kind = FaultKind::Vm {
        vspace: 1,
        va: 0x40,
        access: FaultAccess::Read,
        reason: VmFaultReason::NotMapped,
    };
    r.cho_fault(0, kind, "inject Vm");
    r.recv_fault("procsrv Recv");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });

    r.m.tcb_resume_with_regs(r.t_s[0], msg(0, &[7, 8, 9, 10]))
        .expect("tcb_resume_with_regs");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [7, 8, 9, 10]);
    assert_eq!(
        r.m.tcb_info(r.t_s[0]).expect("info").priority,
        5,
        "TCB quay lại **đúng priority** (bucket 5)"
    );
    assert!(r.m.ready_contains(0, r.t[0]), "Ready lại trong ready queue");
    assert_eq!(
        r.m.ipc_sent(),
        1,
        "chỉ message thanh ghi — không phát lại Call"
    );
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0), "hàng gửi rỗng");
    assert_eq!(r.m.endpoint_recv_waiter(r.ep), Ok(None));
    r.kiem("resume_with_regs");
}

#[test]
fn resume_with_regs_khi_fault_chua_giao_huy_message() {
    let mut r = Rig::new();
    r.cho_fault(0, FaultKind::UnknownSyscall { nr: 5 }, "inject");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1), "fault còn trong hàng");
    r.m.tcb_resume_with_regs(r.t_s[0], msg(0, &[1, 2, 3, 4]))
        .expect("resume khi fault chưa giao");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(0),
        "resume huỷ message fault đang chờ (không để message của TCB Ready)"
    );
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [1, 2, 3, 4]);
    assert_eq!(r.m.ipc_sent(), 1);
    r.kiem("resume trước khi giao");
}

#[test]
fn resume_with_regs_giu_exit_code_va_khong_phat_lai() {
    let mut r = Rig::new();
    r.m.exit(r.t[0], 7).expect("Exit");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 7 })
        }
    );
    r.m.tcb_suspend(r.t_s[0], SuspendMode::Abort)
        .expect("Suspend(Abort)");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.m.tcb_read_state(r.t_s[0]).expect("read").exit_code,
        Some(7),
        "Suspend(Abort) huỷ message Exit nhưng **giữ** exit_code (D2-25)"
    );
    r.m.tcb_resume_with_regs(r.t_s[0], msg(0, &[0xAA, 0xBB]))
        .expect("resume_with_regs");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.m.tcb_read_state(r.t_s[0]).expect("read").exit_code,
        Some(7),
        "`tcb_resume_with_regs` **giữ** exit_code (khác `Tcb.Resume` của KT-18/25)"
    );
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [0xAA, 0xBB, 0, 0]);
    assert_eq!(r.m.ipc_sent(), 1);
    r.kiem("resume giữ exit_code");
}

// ===========================================================================
// 4. `Exit` đủ D2-25 — 4 nhánh (kiểm lại từ KT-18 bằng test của KT-26).
// ===========================================================================

#[test]
fn exit_co_fault_ep_thanh_blocked_on_send_call_false() {
    let mut r = Rig::new();
    r.m.exit(r.t[0], 7).expect("Exit");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 7 })
        },
        "Exit đi Send (call=false), không Reply (D2-25)"
    );
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    r.kiem("Exit có fault_ep");
}

#[test]
fn exit_procsrv_recv_xong_moi_inactive() {
    let mut r = Rig::new();
    r.m.exit(r.t[0], 3).expect("Exit");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    // `.scn` 20: **trước** khi procsrv Recv, TCB còn `BlockedOnSend` (kỳ vọng nêu trong tệp).
    assert!(matches!(r.st(0), TcbState::BlockedOnSend { .. }));
    r.recv_fault("procsrv Recv Exit");
    assert_eq!(r.st(0), TcbState::Inactive, "Recv xong mới Inactive");
    assert_eq!(
        r.m.tcb_read_state(r.t_s[0]).expect("read").exit_code,
        Some(3),
        "exit_code vẫn đọc được sau khi thành Inactive"
    );
    r.kiem("sau Recv Exit");
}

#[test]
fn exit_khong_fault_ep_inactive_ngay() {
    let mut r = Rig::new();
    // `t[3]` không có `fault_ep` — dùng TCB thứ tư chưa gắn.
    r.m.exit(r.t[3], 5).expect("Exit không fault_ep");
    assert_eq!(
        r.st(3),
        TcbState::Inactive,
        "không fault_ep ⇒ Inactive ngay"
    );
    assert_eq!(
        r.m.tcb_read_state(r.t_s[3]).expect("read").exit_code,
        Some(5)
    );
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0), "không sinh message");
    r.kiem("Exit không fault_ep");
}

#[test]
fn exit_suspend_abort_huy_message_giu_code() {
    let mut r = Rig::new();
    r.m.exit(r.t[0], 9).expect("Exit");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    r.m.tcb_suspend(r.t_s[0], SuspendMode::Abort)
        .expect("Suspend(Abort)");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0), "message Exit bị huỷ");
    assert_eq!(r.st(0), TcbState::Ready);
    let snap = r.m.tcb_read_state(r.t_s[0]).expect("read");
    assert!(snap.suspended);
    assert_eq!(
        snap.exit_code,
        Some(9),
        "exit_code vẫn đọc được (D2-25 kiểm **cả hai** chiều)"
    );
    r.kiem("Suspend Abort khi chờ Exit");
}

// ===========================================================================
// 5. Đường lỗi/biên của `inject_fault` (không giao nửa chừng).
// ===========================================================================

#[test]
fn fault_khong_co_fault_ep_bi_tu_choi() {
    let mut r = Rig::new();
    let truoc = r.st(3);
    assert_eq!(
        r.m.inject_fault(r.t[3], FaultKind::UnknownSyscall { nr: 1 }),
        Err(KError::InvalidArgument),
        "KT-26 chỉ mô hình fault **có** fault_ep (không fault_ep để K3)"
    );
    assert_eq!(r.st(3), truoc, "lỗi không đổi trạng thái");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0));
    assert_eq!(r.m.ipc_sent(), 0);
    r.kiem("fault không fault_ep");
}

#[test]
fn fault_ep_chet_invalid_capability_khong_doi_trang_thai() {
    let mut r = Rig::new();
    r.m.delete(r.ep_s).expect("Delete cap fault_ep");
    assert_eq!(
        r.m.inject_fault(
            r.t[0],
            FaultKind::Vm {
                vspace: 1,
                va: 1,
                access: FaultAccess::Read,
                reason: VmFaultReason::NotMapped,
            }
        ),
        Err(KError::InvalidCapability)
    );
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.m.ipc_sent(), 0);
    r.kiem("fault_ep chết");
}

#[test]
fn fault_call_gap_recv_khong_co_reply_bi_cancelled() {
    let mut r = Rig::new();
    // procsrv Recv **không** cấp Reply ⇒ fault không thể được trả lời.
    r.m.recv(r.t[1], r.ep_s, None).expect("Recv không Reply");
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnRecv {
            ep: r.ep,
            reply: None
        }
    );
    r.m.inject_fault(r.t[0], FaultKind::UnknownSyscall { nr: 3 })
        .expect("inject");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "không có Reply ⇒ caller không bị treo vĩnh viễn (Cancelled, như Q7(b))"
    );
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
    assert_eq!(r.st(1), TcbState::Ready, "procsrv được đánh thức");
    assert_eq!(
        r.m.tcb_last_fault(r.t[1]).expect("last"),
        Some((r.t[0], FaultKind::UnknownSyscall { nr: 3 }))
    );
    r.kiem("fault Call không Reply");
}

#[test]
fn fault_khi_hang_gui_day_busy_khong_doi_gi() {
    let mut r = Rig::new();
    // Làm đầy hàng gửi của `fault_ep` bằng NBSend (không chặn) từ `t[2]`.
    for i in 0..vifast_sim::mailbox::MAILBOX_CAP {
        r.m.nb_send(r.t[2], r.ep_s, msg(i as u32, &[i as u64]))
            .expect("NBSend đầy hàng");
    }
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(vifast_sim::mailbox::MAILBOX_CAP)
    );
    let truoc = r.st(0);
    assert_eq!(
        r.m.inject_fault(r.t[0], FaultKind::UnknownSyscall { nr: 1 }),
        Err(KError::Busy),
        "hàng đầy ⇒ Busy **trước** khi đổi trạng thái (không mất nửa chừng)"
    );
    assert_eq!(r.st(0), truoc);
    r.kiem("hàng đầy");
}

// ===========================================================================
// 6. Mô hình procsrv `{code, how}` — 4 nhánh (Q10; DoD 5).
// ===========================================================================

#[test]
fn procsrv_wait_exited_khi_exit_co_va_khong_fault_ep() {
    // (a) Exit có fault_ep: procsrv Recv rồi `wait` ⇒ Exited{code}.
    let mut r = Rig::new();
    r.m.exit(r.t[0], 42).expect("Exit");
    let got = r.procsrv.recv_exit(&mut r.m).expect("recv_exit");
    assert_eq!(got, Some((r.t[0], 42)));
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[0]).expect("wait"),
        WaitResult {
            code: 42,
            how: WaitHow::Exited
        }
    );
    // (b) Exit **không** fault_ep: TCB Inactive ngay; `wait` đọc `exit_code` ⇒ Exited.
    r.m.exit(r.t[3], 8).expect("Exit không fault_ep");
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[3]).expect("wait"),
        WaitResult {
            code: 8,
            how: WaitHow::Exited
        }
    );
    r.kiem("wait Exited");
}

#[test]
fn procsrv_wait_cancelled_khi_abort_truoc_khi_giao() {
    let mut r = Rig::new();
    r.m.exit(r.t[0], 11).expect("Exit");
    r.m.tcb_suspend(r.t_s[0], SuspendMode::Abort)
        .expect("Abort trước khi procsrv Recv");
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[0]).expect("wait"),
        WaitResult {
            code: 11,
            how: WaitHow::Cancelled
        },
        "Abort huỷ message Exit nhưng exit_code vẫn đọc được ⇒ Cancelled"
    );
    r.kiem("wait Cancelled");
}

#[test]
fn procsrv_wait_faulted_va_resume_fault() {
    let mut r = Rig::new();
    let kind = FaultKind::Vm {
        vspace: 1,
        va: 0x9000,
        access: FaultAccess::Write,
        reason: VmFaultReason::NotMapped,
    };
    r.cho_fault(0, kind, "inject Vm");
    assert_eq!(r.procsrv.recv_exit(&mut r.m).expect("recv"), None);
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[0]).expect("wait"),
        WaitResult {
            code: 0,
            how: WaitHow::Faulted
        },
        "fault ≠ Exit ⇒ Faulted"
    );
    assert_eq!(r.procsrv.last_fault(r.t[0]), Some(kind));
    // procsrv quyết định sửa rồi cho chạy tiếp bằng thanh ghi mới.
    r.procsrv
        .resume_fault(&mut r.m, r.t[0], msg(0, &[5, 6, 7, 8]))
        .expect("resume_fault");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [5, 6, 7, 8]);
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[0]),
        Err(KError::Busy),
        "sau khi sửa, tiến trình **chạy lại** nên wait chưa trả"
    );
    r.kiem("wait Faulted + resume");
}

#[test]
fn procsrv_kill_la_suspend_abort_cong_revoke_cap() {
    let mut r = Rig::new();
    r.procsrv.kill(&mut r.m, r.t[0]).expect("kill");
    assert_eq!(r.st(0), TcbState::Ready);
    assert!(
        r.m.tcb_read_state(r.t_s[0]).expect("read").suspended,
        "kill = Suspend(Abort) ⇒ treo"
    );
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[0]).expect("wait"),
        WaitResult {
            code: 0,
            how: WaitHow::Killed
        }
    );
    r.kiem("kill");
}

// ===========================================================================
// 6b. Ba nhánh phụ reviewer yêu cầu phủ (không sửa lõi): Revoke cap dẫn xuất khi kill,
//     nhánh `Sleeping` của `tcb_resume_with_regs`, donation đi theo fault Call→Reply.
// ===========================================================================

#[test]
fn procsrv_kill_revoke_xoa_cap_dan_xuat() {
    let mut r = Rig::new();
    // Cap **dẫn xuất** từ cap TCB gốc (slot 30) — `kill` phải Revoke nó (thiết kế §6.4).
    let con = r.m.copy(r.t_s[0], slot(r.c0, 30)).expect("copy cap TCB");
    let _ = con;
    assert!(
        r.m.cap_at(r.c0, 30).is_ok(),
        "cap dẫn xuất phải tồn tại trước kill"
    );
    r.procsrv.kill(&mut r.m, r.t[0]).expect("kill");
    assert_eq!(
        r.m.cap_at(r.c0, 30),
        Err(KError::SlotEmpty),
        "kill = Revoke cap TCB ⇒ cap dẫn xuất bị xoá"
    );
    assert!(
        r.m.cap_at(r.c0, S_TCB).is_ok(),
        "Revoke **không** xoá chính cap TCB (chỉ con/cháu)"
    );
    r.kiem("kill revoke");
}

#[test]
fn resume_with_regs_giu_sleeping_va_ghi_thanh_ghi() {
    let mut r = Rig::new();
    r.m.transition_tcb(r.t[0], TcbState::Running { hart: 0 })
        .expect("Ready→Running");
    r.m.transition_tcb(
        r.t[0],
        TcbState::Sleeping {
            reason: vifast_sim::kobj::SleepReason::Budget,
        },
    )
    .expect("Running→Sleeping{Budget}");
    assert!(matches!(r.st(0), TcbState::Sleeping { .. }));
    r.m.tcb_resume_with_regs(r.t_s[0], msg(0, &[9, 8, 7, 6]))
        .expect("resume_with_regs trên Sleeping");
    assert_eq!(
        r.st(0),
        TcbState::Sleeping {
            reason: vifast_sim::kobj::SleepReason::Budget
        },
        "giữ nguyên Sleeping (bảng §4.5 `:309-310`), không phát lại gì"
    );
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [9, 8, 7, 6]);
    assert_eq!(r.m.ipc_sent(), 1, "chỉ message thanh ghi");
    assert!(
        !r.m.ready_contains(0, r.t[0]),
        "Sleeping **không** vào ready queue"
    );
    r.kiem("resume Sleeping");
}

#[test]
fn fault_donation_di_theo_call_va_tra_lai_khi_reply() {
    // procsrv **passive** (không SC riêng) ⇒ SC của client phải đi theo fault Call (KT-21).
    let mut r = Rig::new_variant(false);
    assert_eq!(r.m.donated_sc(r.t[1]).expect("donated_sc"), None);
    let sc_client =
        r.m.tcb_info(r.t_s[0])
            .expect("info")
            .sched_context
            .expect("client có SC");
    r.m.recv(r.t[1], r.ep_s, Some(r.rep_s))
        .expect("procsrv Recv");
    r.m.inject_fault(
        r.t[0],
        FaultKind::Vm {
            vspace: 1,
            va: 0x7777,
            access: FaultAccess::Read,
            reason: VmFaultReason::NotMapped,
        },
    )
    .expect("inject Vm");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });
    assert_eq!(
        r.m.donated_sc(r.t[1]).expect("donated_sc"),
        Some(sc_client),
        "fault Call mang SC của client sang procsrv passive"
    );
    r.reply_regs(0, &[1, 2, 3, 4], "Reply trả donation");
    assert_eq!(
        r.m.donated_sc(r.t[1]).expect("donated_sc"),
        None,
        "Reply trả SC về client (không rò)"
    );
    assert_eq!(
        r.m.tcb_info(r.t_s[0]).expect("info").sched_context,
        Some(sc_client)
    );
    r.kiem("donation fault");
}

// ===========================================================================
// 7. Fuzz 1.000 seed × 64 op trộn exit/fault/recv/reply/suspend/resume/kill —
//    bất biến sau **mỗi** bước; cùng seed ⇒ cùng trace.
// ===========================================================================

const SEEDS: u64 = if cfg!(miri) { 3 } else { 1_000 };
const OPS: usize = if cfg!(miri) { 8 } else { 64 };

fn mot_seed(seed: u64) -> Vec<String> {
    let mut r = Rig::new();
    let mut rng = Lcg::new(seed);
    let mut trace = Vec::with_capacity(OPS);
    for step in 0..OPS {
        let i = (rng.below(2) as usize) * 2; // 0 hoặc 2 (hai tiến trình con)
        let op = rng.below(8);
        let kq: Result<(), KError> = match op {
            0 => {
                let kind = match rng.below(3) {
                    0 => FaultKind::Vm {
                        vspace: 1,
                        va: rng.next_u64() & 0xFFFF,
                        access: match rng.below(3) {
                            0 => FaultAccess::Read,
                            1 => FaultAccess::Write,
                            _ => FaultAccess::Execute,
                        },
                        reason: VmFaultReason::NotMapped,
                    },
                    1 => FaultKind::UnknownSyscall { nr: rng.below(200) },
                    _ => FaultKind::UserException {
                        ip: rng.below(0x10000),
                        kind: rng.below(4),
                    },
                };
                r.m.inject_fault(r.t[i], kind)
            }
            1 => r.m.exit(r.t[i], rng.below(100)),
            2 => r.procsrv.recv_exit(&mut r.m).map(|_| ()),
            3 => {
                let w = [rng.next_u64(), rng.next_u64()];
                r.m.tcb_resume_with_regs(r.t_s[i], msg(0, &w))
            }
            4 => r.m.tcb_suspend(r.t_s[i], SuspendMode::Abort),
            5 => r.m.tcb_resume(r.t_s[i]),
            6 => r.procsrv.kill(&mut r.m, r.t[i]),
            7 => {
                // Kích hoạt lại một TCB đã chết (controller, không phải syscall) để vòng fuzz
                // không bão hoà ở trạng thái `Inactive`; `tcb_configure` xoá exit_code.
                if r.st(i) == TcbState::Inactive {
                    r.m.transition_tcb(r.t[i], TcbState::Ready)
                } else {
                    Ok(())
                }
            }
            _ => unreachable!(),
        };
        // Bất biến sau **mỗi** bước (KT-26 DoD 7).
        r.kiem(&format!("seed {seed} bước {step}"));
        trace.push(format!(
            "{step}:op={op}:kq={}:st={}:sent={}:delivered={}:pending={}:dropped={}",
            match kq {
                Ok(()) => "ok".to_string(),
                Err(e) => e.name().to_string(),
            },
            r.st(i).render(),
            r.m.ipc_sent(),
            r.m.ipc_delivered(),
            r.m.ipc_pending(),
            r.m.ipc_dropped()
        ));
    }
    trace
}

#[test]
fn fuzz_1000_seed_64_op_bat_bien_va_tat_dinh() {
    for seed in 0..SEEDS {
        let a = mot_seed(seed);
        let b = mot_seed(seed);
        assert_eq!(a, b, "seed {seed}: cùng seed phải cùng trace");
        assert_eq!(a.len(), OPS);
    }
}

// ===========================================================================
// 8. `Step::{Fault, ReplyFault, ResumeWithRegs}` của `scenario.rs` (dữ liệu kịch bản).
// ===========================================================================

#[test]
fn scenario_step_fault_replyfault_resumewithregs() {
    use vifast_sim::scenario::{run_sequential, Msg, ScenarioStep, Step};

    let mut m = Machine::new();
    let c0 = m.add_cnode(32);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep_cap = m.root_cap(ep);
    m.put_cap(c0, 0, ep_cap).expect("cap ep");
    let rep = m.add_kernel_object(ObjType::Reply);
    let rep_cap = m.root_cap(rep);
    m.put_cap(c0, 1, rep_cap).expect("cap rep");
    let ep_s = slot(c0, 0);
    let rep_s = slot(c0, 1);
    let app = m.add_kernel_object(ObjType::Tcb);
    let app_cap = m.root_cap(app);
    m.put_cap(c0, 10, app_cap).expect("cap app");
    let app_s = slot(c0, 10);
    let srv = m.add_kernel_object(ObjType::Tcb);
    let srv_cap = m.root_cap(srv);
    m.put_cap(c0, 11, srv_cap).expect("cap srv");
    m.transition_tcb(app, TcbState::Ready).expect("Ready app");
    m.transition_tcb(srv, TcbState::Ready).expect("Ready srv");
    m.tcb_set_fault_ep(app_s, ep_s).expect("SetFaultEp");

    let kind = FaultKind::UnknownSyscall { nr: 12 };
    let info = MessageInfo::new(0, 0, false, 0, 2).expect("info");
    let regs = Msg::new(info, &[0x55, 0x66]).expect("Msg");
    let steps = vec![
        ScenarioStep {
            thread: "app",
            step: Step::Fault { tcb: app, kind },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnSend {
                ep,
                call: true,
                fault: Some(kind),
            }),
        },
        ScenarioStep {
            thread: "procsrv",
            step: Step::Recv {
                tcb: srv,
                ep: ep_s,
                reply: Some(rep_s),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "app",
            step: Step::ResumeWithRegs {
                tcb: app_s,
                msg: regs,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "app",
            step: Step::Fault { tcb: app, kind },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnSend {
                ep,
                call: true,
                fault: Some(kind),
            }),
        },
        ScenarioStep {
            thread: "procsrv",
            step: Step::Recv {
                tcb: srv,
                ep: ep_s,
                reply: Some(rep_s),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "procsrv",
            step: Step::ReplyFault {
                tcb: srv,
                reply: rep_s,
                msg: regs,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
    ];
    let report = run_sequential(&mut m, &steps).expect("scenario Step fault phải chạy được");
    assert_eq!(report.events, 6);
    assert_eq!(m.tcb_regs(app).expect("regs"), [0x55, 0x66, 0, 0]);
    assert_eq!(m.ipc_sent(), 2, "mỗi lần resume = một message thanh ghi");
    assert_eq!(m.endpoint_send_len(ep), Ok(0));
}

#[test]
fn msg_rong_khong_dung_duoc_cho_resume_thanh_ghi() {
    // `tcb_resume_with_regs` là model API cho **thanh ghi**; message rỗng vẫn hợp lệ (0 MR) và
    // **xoá sạch** vùng nhận cũ (không lộ dữ liệu message trước).
    let mut r = Rig::new();
    r.cho_fault(0, FaultKind::UnknownSyscall { nr: 2 }, "inject");
    r.recv_fault("recv");
    r.m.reply_msg(r.t[1], r.rep_s, msg(0, &[1, 2, 3, 4]))
        .expect("Reply có dữ liệu");
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [1, 2, 3, 4]);
    // Fault mới rồi resume bằng message rỗng: vùng nhận phải sạch.
    r.m.inject_fault(r.t[0], FaultKind::UnknownSyscall { nr: 3 })
        .expect("inject lần 2");
    r.m.tcb_resume_with_regs(r.t_s[0], msg_empty())
        .expect("resume rỗng");
    assert_eq!(r.m.tcb_regs(r.t[0]).expect("regs"), [0, 0, 0, 0]);
    assert_eq!(
        r.m.tcb_ipc_info(r.t[0])
            .expect("info")
            .expect("some")
            .length(),
        0
    );
    assert_eq!(WORDS_IN_REGISTERS, 4);
    r.kiem("resume rỗng");
}
