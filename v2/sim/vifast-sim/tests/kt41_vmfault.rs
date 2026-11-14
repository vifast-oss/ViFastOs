// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-41 — **VM fault + `FaultKind::Vm` (model-only)** (K2-PLAN §3 KT-41 + §2 Q6; D2-65 Q6;
//! D2-56(a)/(c); `v2/docs/k2-vm.md` §15; thiết kế §4.6 `:321`).
//!
//! K2 **không** có pager/memory server (Q6 — COW/pager là K3): `vm_touch(vspace, va, access)`
//! đi đúng đường fault sẵn có của K1c — `vspace_walk` miss/thiếu quyền ⇒ `FaultKind::Vm
//! {vspace, va, access, reason}` giao tới `fault_ep` của TCB đang chạy; TCB `BlockedOnSend
//! {call: true, fault: Some(Vm ..)}`, procsrv model nhận qua Call+Reply và **chỉ ghi log**
//! (không cấp trang). **Không có `fault_ep`** ⇒ giữ nguyên K1c (`InvalidArgument`, không đổi
//! trạng thái — D2-56(c)).
//!
//! Ba `reason`: `NotMapped` (walk miss), `Permission` (thiếu quyền), `Wx` (truy cập tạo thế
//! W+X: exec trang W, hoặc ghi trang X — luật W^X toàn cục của KT-39/D2-67).
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt41-red-compile.log`](../../agent/JOURNAL/kt41-red-compile.log)
//! — `FaultKind::Vm` chưa có `vspace`/`va`/`reason`, `VmFaultReason`/`Machine::vm_touch`/
//! `Machine::check_fault` chưa tồn tại ⇒ `could not compile`.
//!
//! **Model-only `[TK]`, không phải spec/ABI**: `spec/abi.toml` **không** đổi (D2-65 Q6,
//! D2-56(a) hoãn vào spec ở K3).

use vifast_sim::ipc::{Message, MessageInfo};
use vifast_sim::kobj::{
    FaultAccess, FaultKind, KError, Machine, MapAttrs, MapRights, ObjId, ObjType, SlotRef,
    TcbState, VmFaultReason,
};
use vifast_sim::procsrv_model::{ProcsrvModel, WaitHow};
use vifast_sim::Lcg;

const S_EP: usize = 0;
const S_REP: usize = 1;
const S_UVA: usize = 2;
const S_UFR: usize = 3;
const S_VS: usize = 4;
const S_TCB: usize = 10;
const S_SC: usize = 20;
const S_CTL: usize = 40;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` không cap; `words.len()` phải bằng `info.length()`.
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

/// Quyền + attrs của một mapping — dùng cho mô hình kỳ vọng của property.
#[derive(Clone, Copy)]
struct Page {
    va: u64,
    mapped: Option<(MapRights, MapAttrs)>,
}

// ===========================================================================
// Rig: CNode 1024 slot; 1 Endpoint `fault_ep`; 1 Reply; 3 TCB
// (t0 có fault_ep, t2 **không** fault_ep, t1 là procsrv); VSpace + cây 3 cấp phủ 2 MiB đầu.
// ===========================================================================

struct Rig {
    m: Machine,
    c: ObjId,
    ep: ObjId,
    rep: ObjId,
    vs: ObjId,
    t: [ObjId; 4],
    t_s: [SlotRef; 4],
    procsrv: ProcsrvModel,
    next: usize,
}

impl Rig {
    fn new() -> Self {
        let mut m = Machine::new();
        let c = m.add_cnode(1024);
        let uva = m.add_untyped(0, 2 * 1024 * 1024);
        let ufr = m.add_untyped(1 << 32, 4096);
        for (s, u) in [(S_UVA, uva), (S_UFR, ufr)] {
            let cap = m.root_cap(u);
            m.put_cap(c, s, cap).expect("cap Untyped");
        }
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c, S_CTL, ctl_cap).expect("cap SchedControl");
        let ctl_s = slot(c, S_CTL);

        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c, S_EP, ep_cap).expect("cap fault_ep");
        let ep_s = slot(c, S_EP);

        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c, S_REP, rep_cap).expect("cap Reply");
        let rep_s = slot(c, S_REP);

        let vs_cap = m
            .vspace_create(slot(c, S_UVA))
            .expect("vspace_create từ Untyped");
        let vs = m.cap_info(vs_cap).expect("cap VSpace").obj;
        m.put_cap(c, S_VS, vs_cap).expect("cap VSpace vào CNode");

        let mut t = [0u32; 4];
        let mut t_s = [slot(c, S_TCB); 4];
        // t0 (ưu tiên 6) và t2 (4) là tiến trình con **có** fault_ep; t1 (5) procsrv;
        // t3 (3) là tiến trình con **không** fault_ep (ca K1c của D2-56(c)).
        let prio = [6u8, 5, 4, 3];
        for i in 0..4 {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c, S_TCB + i, cap).expect("cap TCB");
            t[i] = obj;
            t_s[i] = slot(c, S_TCB + i);

            let sc = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc);
            m.put_cap(c, S_SC + i, sc_cap).expect("cap SchedContext");
            let sc_slot = slot(c, S_SC + i);
            m.sched_control_configure(ctl_s, sc_slot, 4, 8)
                .expect("SchedControl.Configure");
            m.set_sched_params(t_s[i], Some(sc_slot))
                .expect("set_sched_params (test-only)");
            m.tcb_set_priority(t_s[i], prio[i], 255)
                .expect("Tcb.SetPriority");
        }
        // t0/t2 có `fault_ep`; t1 procsrv và t3 (ca không fault_ep) **không** có. t0/t2/t3
        // `Ready`; t1 để `Inactive` (procsrv được đánh thức bởi `Recv` — `recv` tự
        // `Inactive → Ready`).
        for i in [0usize, 2] {
            m.tcb_set_fault_ep(t_s[i], ep_s).expect("Tcb.SetFaultEp");
        }
        for i in [0usize, 2, 3] {
            m.transition_tcb(t[i], TcbState::Ready)
                .expect("Inactive→Ready");
        }
        let mut procsrv = ProcsrvModel::new(t[1], ep_s, rep_s);
        procsrv.track(t[0], t_s[0]);
        procsrv.track(t[2], t_s[2]);

        let mut r = Self {
            m,
            c,
            ep,
            rep,
            vs,
            t,
            t_s,
            procsrv,
            next: 50,
        };
        r.cay_4k();
        r
    }

    // ------------------------------------------------------------- cây PageTable

    fn slot_moi(&mut self) -> usize {
        let s = self.next;
        self.next += 1;
        s
    }

    fn put(&mut self, cap: vifast_sim::kobj::CapId) -> (SlotRef, ObjId) {
        let s = self.slot_moi();
        self.m.put_cap(self.c, s, cap).expect("đặt cap vào CNode");
        let obj = self.m.cap_info(cap).expect("cap vừa đặt").obj;
        (slot(self.c, s), obj)
    }

    fn tbl(&mut self) -> (SlotRef, ObjId) {
        let cap = self
            .m
            .retype(slot(self.c, S_UVA), ObjType::PageTable, 0, 0)
            .expect("retype PageTable");
        self.put(cap)
    }

    /// Dựng chuỗi root → mid → leaf phủ 2 MiB đầu (đủ cho 512 trang 4 KiB).
    fn cay_4k(&mut self) {
        let root = self.m.vspace_root(self.vs).expect("root VSpace");
        let (_, mid) = self.tbl();
        self.m.page_table_attach(root, 0, mid).expect("attach mid");
        let (_, leaf) = self.tbl();
        self.m.page_table_attach(mid, 0, leaf).expect("attach leaf");
    }

    /// Cấp Frame 4 KiB rồi map vào `va` (qua VSpace, áp luật W^X KT-39).
    fn map(&mut self, va: u64, rights: MapRights, attrs: MapAttrs) -> ObjId {
        let cap = self
            .m
            .retype(slot(self.c, S_UFR), ObjType::Frame, 12, 0)
            .expect("retype Frame 4 KiB");
        let (_, f) = self.put(cap);
        self.m
            .vspace_map_obj(self.vs, va, f, rights, attrs)
            .expect("map 4 KiB vào VSpace");
        f
    }

    // ------------------------------------------------------------- TCB / fault

    /// `dispatch(0)` phải chọn đúng `t[i]` (kiểm mô hình tất định của rig).
    fn chay(&mut self, i: usize) {
        let got = self.m.dispatch(0).expect("dispatch hart 0");
        assert_eq!(
            got,
            Some(self.t[i]),
            "hart 0 phải chọn TCB {i} (kiểm rig tất định)"
        );
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn dang_fault(&self, i: usize, kind: FaultKind) -> bool {
        self.st(i)
            == TcbState::BlockedOnSend {
                ep: self.ep,
                call: true,
                fault: Some(kind),
            }
    }

    /// `vm_touch` trên TCB đang chạy `t[i]` ⇒ fault đúng payload, đúng `fault_ep`.
    fn cham_fault(
        &mut self,
        i: usize,
        va: u64,
        access: FaultAccess,
        reason: VmFaultReason,
    ) -> FaultKind {
        self.chay(i);
        assert_eq!(
            self.m.vm_touch(self.vs, va, access),
            Ok(()),
            "fault phải giao **thành công** qua đường fault sẵn có"
        );
        let kind = FaultKind::Vm {
            vspace: self.vs,
            va,
            access,
            reason,
        };
        assert!(
            self.dang_fault(i, kind),
            "fault sai payload/đích: {:?}",
            self.st(i)
        );
        self.kiem("sau vm_touch fault");
        kind
    }

    /// procsrv `Recv` fault (Call + Reply) và trả kind đã giao.
    fn nhan_fault(&mut self, i: usize, kind: FaultKind) {
        assert_eq!(
            self.procsrv.recv_exit(&mut self.m).expect("recv_exit"),
            None,
            "fault ≠ Exit ⇒ `recv_exit` trả None nhưng đã ghi log"
        );
        assert_eq!(
            self.st(i),
            TcbState::BlockedOnReply { reply: self.rep },
            "fault Call ⇒ caller chờ Reply"
        );
        assert_eq!(
            self.procsrv.last_fault(self.t[i]),
            Some(kind),
            "procsrv model phải **nhận đúng** fault (chỉ ghi log)"
        );
        self.kiem("procsrv nhận fault");
    }

    fn resume(&mut self, i: usize) {
        self.procsrv
            .resume_fault(&mut self.m, self.t[i], msg(0, &[11, 12, 13, 14]))
            .expect("resume_fault (Reply có thanh ghi)");
        assert_eq!(self.st(i), TcbState::Ready, "Reply xong ⇒ Ready");
        assert_eq!(
            self.m.tcb_regs(self.t[i]).expect("regs"),
            [11, 12, 13, 14],
            "Reply ghi thanh ghi mới trước khi Ready (một cửa)"
        );
        self.kiem("sau resume");
    }

    /// Bất biến sau mỗi op: fault không mất (KT-41), Frame/PageTable/W^X/VSpace/ASID/IPC/cap.
    fn kiem(&self, nhan: &str) {
        for (ten, kq) in [
            ("check_fault", self.m.check_fault()),
            ("check_frames", self.m.check_frames()),
            ("check_pages", self.m.check_pages()),
            ("check_wx", self.m.check_wx()),
            ("check_vspace", self.m.check_vspace()),
            ("check_asid", self.m.check_asid()),
            ("check_ipc", self.m.check_ipc()),
            ("check_reply_links", self.m.check_reply_links()),
            ("check_caps", self.m.check_caps()),
            ("check_mailboxes", self.m.check_mailboxes()),
            ("check_bytes", self.m.check_all_bytes()),
        ] {
            kq.unwrap_or_else(|e| panic!("{nhan}: {ten}: {e}"));
        }
    }
}

// ===========================================================================
// 1. `NotMapped`: walk miss ⇒ fault tới đúng `fault_ep`; procsrv **không** cấp trang.
// ===========================================================================

#[test]
fn notmapped_toi_dung_fault_ep_khong_cap_trang() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ | MapRights::WRITE, MapAttrs::WB);
    let frames = r.m.frame_count();
    let va = 0x20_0000 + 0x40; // ngoài 2 MiB đã có bảng ⇒ walk miss
    let kind = r.cham_fault(0, va, FaultAccess::Read, VmFaultReason::NotMapped);
    assert_eq!(r.m.frame_count(), frames, "fault không cấp Frame");

    r.nhan_fault(0, kind);
    assert_eq!(
        r.m.frame_count(),
        frames,
        "procsrv model **không** cấp trang"
    );
    assert_eq!(
        r.m.vspace_walk(r.vs, va),
        Err(KError::NotMapped),
        "trang vẫn chưa được map sau khi procsrv nhận fault (không pager ở K2)"
    );

    // Resume rồi chạm lại: vẫn NotMapped, vẫn không có trang mới.
    r.resume(0);
    let kind2 = r.cham_fault(0, va, FaultAccess::Write, VmFaultReason::NotMapped);
    assert_eq!(r.m.frame_count(), frames);
    r.nhan_fault(0, kind2);
    r.resume(0);
    assert_eq!(r.m.page_table_count(), 3, "không sinh PageTable mới");

    // `va` **không** căn trang: touch là một **byte** trong trang ⇒ payload giữ **đúng** `va`
    // yêu cầu, không trả `Alignment`, không làm tròn về gốc trang.
    let va_odd = 0x7001;
    let k3 = r.cham_fault(0, va_odd, FaultAccess::Read, VmFaultReason::NotMapped);
    assert_eq!(
        r.m.vm_touch(r.vs, va_odd, FaultAccess::Read),
        Err(KError::IllegalOperation),
        "fault đang chờ ⇒ lần chạm nữa không đổi trạng thái (không mất fault)"
    );
    r.nhan_fault(0, k3);
    r.resume(0);
    assert_eq!(
        r.m.frame_count(),
        frames,
        "va lệch căn cũng không cấp trang"
    );
}

// ===========================================================================
// 2. `Permission`: thiếu quyền thật (không phải W+X).
// ===========================================================================

#[test]
fn permission_ghi_trang_read_va_doc_trang_write_only() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ, MapAttrs::WB); // READ-only
    r.map(0x2000, MapRights::WRITE, MapAttrs::WB); // WRITE-only

    // Ghi vào mapping READ ⇒ Permission.
    let k1 = r.cham_fault(0, 0x1000, FaultAccess::Write, VmFaultReason::Permission);
    r.nhan_fault(0, k1);
    r.resume(0);
    // Đọc mapping WRITE-only (không có bit READ) ⇒ Permission.
    let k2 = r.cham_fault(0, 0x2000, FaultAccess::Read, VmFaultReason::Permission);
    r.nhan_fault(0, k2);
    r.resume(0);
    // Exec trang READ-only **không** EXEC ⇒ Permission (chưa phải W^X — không có W).
    let k3 = r.cham_fault(0, 0x1000, FaultAccess::Execute, VmFaultReason::Permission);
    r.nhan_fault(0, k3);
    r.resume(0);

    // Quyền đủ ⇒ **không** fault, giữ Running.
    r.chay(0);
    assert_eq!(r.m.vm_touch(r.vs, 0x1000, FaultAccess::Read), Ok(()));
    assert_eq!(r.m.vm_touch(r.vs, 0x2000, FaultAccess::Write), Ok(()));
    assert_eq!(r.st(0), TcbState::Running { hart: 0 });
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0), "không sinh fault thừa");
    r.kiem("permission");
}

// ===========================================================================
// 3. `Wx`: truy cập tạo thế W+X (luật W^X toàn cục KT-39/D2-67 đã cấm map W+X).
// ===========================================================================

#[test]
fn wx_exec_trang_write_va_ghi_trang_exec() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ | MapRights::WRITE, MapAttrs::WB); // W, không X
    r.map(
        0x2000,
        MapRights::READ,
        MapAttrs::WB | MapAttrs::EXEC, // X, không W
    );
    r.map(0x3000, MapRights::WRITE, MapAttrs::WB | MapAttrs::NX); // W + NX

    // Exec trang W ⇒ W+X trong sử dụng ⇒ Wx (không phải Permission).
    let k1 = r.cham_fault(0, 0x1000, FaultAccess::Execute, VmFaultReason::Wx);
    r.nhan_fault(0, k1);
    r.resume(0);
    // Ghi trang X ⇒ W+X ⇒ Wx.
    let k2 = r.cham_fault(0, 0x2000, FaultAccess::Write, VmFaultReason::Wx);
    r.nhan_fault(0, k2);
    r.resume(0);
    // Trang W + NX: exec vẫn là W^X ⇒ **Wx** (Wx thắng Permission/NX).
    let k3 = r.cham_fault(0, 0x3000, FaultAccess::Execute, VmFaultReason::Wx);
    r.nhan_fault(0, k3);
    r.resume(0);

    // Trang X: đọc/exec **hợp lệ**; trang W: đọc/ghi **hợp lệ**.
    r.chay(0);
    assert_eq!(r.m.vm_touch(r.vs, 0x2000, FaultAccess::Read), Ok(()));
    assert_eq!(r.m.vm_touch(r.vs, 0x2000, FaultAccess::Execute), Ok(()));
    assert_eq!(r.m.vm_touch(r.vs, 0x1000, FaultAccess::Read), Ok(()));
    assert_eq!(r.m.vm_touch(r.vs, 0x1000, FaultAccess::Write), Ok(()));
    assert_eq!(r.st(0), TcbState::Running { hart: 0 });
    r.kiem("wx");

    // Bất biến W^X của Frame vẫn đúng: không Frame nào vừa W vừa X.
    assert_eq!(r.m.check_wx(), Ok(()));
}

// ===========================================================================
// 4. Không `fault_ep` ⇒ K1c (`InvalidArgument`, không đổi trạng thái — D2-56(c)).
// ===========================================================================

#[test]
fn khong_fault_ep_giu_hanh_vi_k1c() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ, MapAttrs::WB);
    // Đẩy t0/t2 vào fault đang chờ để `dispatch` chọn t3 (không có fault_ep).
    let k0 = r.cham_fault(0, 0x9000, FaultAccess::Read, VmFaultReason::NotMapped);
    let k2 = r.cham_fault(2, 0x9000, FaultAccess::Write, VmFaultReason::NotMapped);
    let ipc0 = r.m.ipc_sent();
    let hang0 = r.m.endpoint_send_len(r.ep).expect("hàng");

    r.chay(3);
    assert_eq!(
        r.m.vm_touch(r.vs, 0x9000, FaultAccess::Write),
        Err(KError::InvalidArgument),
        "không fault_ep ⇒ K1c: `InvalidArgument` (D2-56(c)), không kill/không đổi trạng thái"
    );
    assert_eq!(
        r.st(3),
        TcbState::Running { hart: 0 },
        "lỗi không đổi trạng thái TCB"
    );
    assert_eq!(r.m.ipc_sent(), ipc0, "không sinh message");
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(hang0),
        "fault cũ của t0 còn nguyên, không bị mất"
    );

    // Quyền đủ thì vẫn chạm được bình thường dù không có `fault_ep`.
    assert_eq!(r.m.vm_touch(r.vs, 0x1000, FaultAccess::Read), Ok(()));
    assert_eq!(r.st(3), TcbState::Running { hart: 0 });
    r.kiem("không fault_ep");

    // t0/t2 vẫn giữ fault cũ; procsrv nhận đủ và resume.
    r.nhan_fault(0, k0);
    r.resume(0);
    r.nhan_fault(2, k2);
    r.resume(2);
    r.kiem("không fault_ep — thu hồi fault cũ");
}

// ===========================================================================
// 5. Resume sau fault trả **đúng** trạng thái (Ready + regs), rồi fault lại được.
// ===========================================================================

#[test]
fn resume_sau_fault_tra_lai_dung_trang_thai() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ | MapRights::WRITE, MapAttrs::WB);

    let k1 = r.cham_fault(0, 0x5000, FaultAccess::Read, VmFaultReason::NotMapped);
    r.nhan_fault(0, k1);
    // Fault-resume một cửa: ghi thanh ghi **trước** Ready, không phát lại IPC.
    r.resume(0);
    assert_eq!(r.m.tcb_last_fault(r.t[0]).expect("last_fault"), None);
    assert!(r
        .m
        .tcb_read_state(r.t_s[0])
        .expect("read")
        .exit_code
        .is_none());

    // Chạy lại: quyền đủ ⇒ Ok, vẫn Running (đúng trạng thái trước fault).
    r.chay(0);
    assert_eq!(r.st(0), TcbState::Running { hart: 0 });
    assert_eq!(r.m.vm_touch(r.vs, 0x1000, FaultAccess::Write), Ok(()));
    assert_eq!(r.st(0), TcbState::Running { hart: 0 });

    // Fault lần hai với **va khác** ⇒ payload mới, không dính fault cũ/W^X.
    let k2 = r.cham_fault(0, 0x6000, FaultAccess::Execute, VmFaultReason::NotMapped);
    assert_ne!(k1, k2, "hai fault khác payload");
    r.nhan_fault(0, k2);
    r.resume(0);
    assert_eq!(r.st(0), TcbState::Ready);
    r.kiem("resume hai lần");
}

// ===========================================================================
// 6. Fault **không mất** khi TCB đang chờ (đúng C2/D2-63): Reply đang giữ caller
//    ⇒ `Recv` lần hai bị `Busy`, fault thứ hai còn nguyên trong hàng, procsrv nhận
//    lại được sau khi Reply.
// ===========================================================================

#[test]
fn fault_khong_mat_khi_tcb_dang_cho_c2() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ, MapAttrs::WB);

    // t0 fault (chưa ai Recv) ⇒ message nằm trong hàng.
    let k0 = r.cham_fault(0, 0x7000, FaultAccess::Read, VmFaultReason::NotMapped);
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    // t2 cũng fault trong lúc t0 còn chờ (t1 procsrv Inactive ⇒ dispatch chọn t2).
    let k2 = r.cham_fault(2, 0x8000, FaultAccess::Write, VmFaultReason::NotMapped);
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(2), "hai fault cùng hàng");

    // procsrv Recv t0; Reply đang giữ caller=t0 ⇒ Recv lần hai bị **Busy** (C2).
    r.nhan_fault(0, k0);
    assert_eq!(
        r.procsrv.recv_exit(&mut r.m),
        Err(KError::Busy),
        "C2/D2-63: không tái dùng Reply đang giữ caller"
    );
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(1),
        "fault của t2 **không** mất khi t0 đang chờ Reply"
    );
    assert!(r.dang_fault(2, k2), "t2 vẫn BlockedOnSend mang fault");
    assert_eq!(r.m.check_fault(), Ok(()));

    // Reply cho t0 giải phóng Reply ⇒ nhận được fault của t2, payload nguyên vẹn.
    r.resume(0);
    r.nhan_fault(2, k2);
    r.resume(2);
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0));
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.st(2), TcbState::Ready);
    r.kiem("C2 không mất fault");
}

// ===========================================================================
// 7. Không hồi quy: `FaultKind::Exit` cũ vẫn chạy và `inject_fault(Exit)` vẫn bị từ chối.
// ===========================================================================

#[test]
fn fault_exit_cu_khong_hoi_quy() {
    let mut r = Rig::new();
    r.m.exit(r.t[0], 9).expect("Exit trên TCB có fault_ep");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 9 })
        },
        "Exit đi hàng gửi **không** Reply (D2-25)"
    );
    r.kiem("Exit chờ giao");
    assert_eq!(
        r.procsrv.recv_exit(&mut r.m).expect("Recv Exit"),
        Some((r.t[0], 9))
    );
    assert_eq!(r.st(0), TcbState::Inactive);
    assert_eq!(
        r.procsrv.wait(&r.m, r.t[0]).expect("wait"),
        vifast_sim::procsrv_model::WaitResult {
            code: 9,
            how: WaitHow::Exited
        }
    );
    assert_eq!(
        r.m.inject_fault(r.t[2], FaultKind::Exit { code: 1 }),
        Err(KError::InvalidArgument),
        "`inject_fault` không nhận Exit (K1c giữ nguyên)"
    );
    r.kiem("Exit");
}

// ===========================================================================
// 8. Lỗi biên của `vm_touch`: không TCB đang chạy, VSpace không có thật, VA ngoài dải.
// ===========================================================================

#[test]
fn vm_touch_loi_bien() {
    let mut r = Rig::new();
    r.map(0x1000, MapRights::READ, MapAttrs::WB);
    // Chưa `dispatch` ⇒ không có TCB đang chạy ⇒ `IllegalOperation` (không fault bừa).
    assert_eq!(
        r.m.vm_touch(r.vs, 0x1000, FaultAccess::Read),
        Err(KError::IllegalOperation)
    );
    r.chay(0);
    // VA ngoài 39 bit ⇒ RangeError của walk (không phải fault).
    assert_eq!(
        r.m.vm_touch(r.vs, 1u64 << 39, FaultAccess::Read),
        Err(KError::RangeError)
    );
    // VSpace không tồn tại ⇒ InvalidCapability (không đổi trạng thái TCB).
    assert_eq!(
        r.m.vm_touch(9999, 0x1000, FaultAccess::Read),
        Err(KError::InvalidCapability)
    );
    assert_eq!(r.st(0), TcbState::Running { hart: 0 });
    r.kiem("lỗi biên");
}

// ===========================================================================
// 9. Property 10.000 vòng (Miri: 150): fault ngẫu nhiên tất định trên 4 loại mapping +
//    2 khoảng trống ⇒ 3 reason đều xuất hiện; fault/resume không rò, bất biến sau mỗi vòng.
// ===========================================================================

// Miri: 40 vòng (property nặng 11 checker/vòng; KT-40 dùng 150 vòng cho 1 checker).
const VONG: usize = if cfg!(miri) { 40 } else { 10_000 };

fn ky_vong(page: Option<(MapRights, MapAttrs)>, access: FaultAccess) -> Option<VmFaultReason> {
    let Some((r, a)) = page else {
        return Some(VmFaultReason::NotMapped);
    };
    let x = a.contains(MapAttrs::EXEC) && !a.contains(MapAttrs::NX);
    match access {
        FaultAccess::Read => (!r.contains(MapRights::READ)).then_some(VmFaultReason::Permission),
        FaultAccess::Write => {
            if r.contains(MapRights::WRITE) {
                None
            } else if x {
                Some(VmFaultReason::Wx)
            } else {
                Some(VmFaultReason::Permission)
            }
        }
        FaultAccess::Execute => {
            if x {
                None
            } else if r.contains(MapRights::WRITE) {
                Some(VmFaultReason::Wx)
            } else {
                Some(VmFaultReason::Permission)
            }
        }
    }
}

#[test]
fn property_10k_fault_resume() {
    let mut r = Rig::new();
    let pages = [
        Page {
            va: 0x1000,
            mapped: Some((MapRights::READ | MapRights::WRITE, MapAttrs::WB)),
        },
        Page {
            va: 0x2000,
            mapped: Some((MapRights::READ, MapAttrs::WB)),
        },
        Page {
            va: 0x3000,
            mapped: Some((MapRights::WRITE, MapAttrs::WB)),
        },
        Page {
            va: 0x4000,
            mapped: Some((MapRights::READ, MapAttrs::WB | MapAttrs::EXEC)),
        },
        Page {
            va: 0x5000,
            mapped: None,
        },
        Page {
            va: 0x20_0000,
            mapped: None,
        },
    ];
    for p in &pages {
        if let Some((rights, attrs)) = p.mapped {
            r.map(p.va, rights, attrs);
        }
    }
    let frames = r.m.frame_count();
    let tables = r.m.page_table_count();

    let mut rng = Lcg::new(41);
    let (mut c_not, mut c_perm, mut c_wx, mut c_ok) = (0u64, 0u64, 0u64, 0u64);
    for vong in 0..VONG {
        let p = pages[(rng.below(pages.len() as u64)) as usize];
        let access = match rng.below(3) {
            0 => FaultAccess::Read,
            1 => FaultAccess::Write,
            _ => FaultAccess::Execute,
        };
        let expected = ky_vong(p.mapped, access);
        r.chay(0);
        match expected {
            None => {
                assert_eq!(
                    r.m.vm_touch(r.vs, p.va, access),
                    Ok(()),
                    "vòng {vong}: quyền đủ phải Ok"
                );
                assert_eq!(r.st(0), TcbState::Running { hart: 0 });
                c_ok += 1;
            }
            Some(reason) => {
                let kind = r.cham_fault(0, p.va, access, reason);
                match reason {
                    VmFaultReason::NotMapped => c_not += 1,
                    VmFaultReason::Permission => c_perm += 1,
                    VmFaultReason::Wx => c_wx += 1,
                }
                r.nhan_fault(0, kind);
                r.resume(0);
            }
        }
        assert_eq!(r.m.frame_count(), frames, "vòng {vong}: không rò Frame");
        assert_eq!(
            r.m.page_table_count(),
            tables,
            "vòng {vong}: không rò PageTable"
        );
        r.kiem(&format!("property vòng {vong}"));
    }
    assert!(
        c_not > 0 && c_perm > 0 && c_wx > 0,
        "phải phủ đủ 3 reason: NotMapped={c_not} Permission={c_perm} Wx={c_wx}"
    );
    assert!(c_ok > 0, "phải có ca thành công");
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(0),
        "không còn fault tồn đọng sau khi resume hết"
    );
    assert!(matches!(
        r.st(0),
        TcbState::Ready | TcbState::Running { .. }
    ));
    r.kiem("property kết thúc");
}
