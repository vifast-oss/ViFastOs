// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-42 — **runner `.scn` cho VM** (model-only): đọc **(không sửa)** năm tệp
//! `v2/test/scn/{22,23,24,25,26}-*.scn`, dựng rig theo kịch bản (harts/caps) và chạy đúng
//! chuỗi bước `vm_*` trên mô hình K2 (Frame/PageTable/VSpace/TLB/fault), so **kết quả từng
//! bước** với kỳ vọng ghi trong tệp (`=> ok` / `=> <mã lỗi>`), 0 lệch.
//!
//! **RED-FIRST (D2-05)**: bản đầu của tệp này chỉ đọc bước rồi `panic!("chưa cài op")` — xem
//! `v2/agent/JOURNAL/kt42-red-scn-runner.log` + `kt42-red-scn-{22,23,24,25,26}.log` (5 ca đỏ,
//! đúng op đầu của từng kịch bản; kịch bản 24 đỏ ở `vm_activate`).
//!
//! Quy ước rig (ghi rõ để review, cùng tinh thần `kt24_scn.rs`/`kt26_scn.rs`):
//!
//! 1. `harts` (top-level) chọn số hart; mặc định 1.
//! 2. Rig chuẩn: CNode 1024 slot; Untyped `uva` (2 MiB) cấp root/PageTable; ba Untyped cửa sổ
//!    PA 1 GiB tại 4/8/16 GiB cấp Frame 4 KiB/2 MiB/1 GiB (`f4`/`f2`/`f1`); hai VSpace
//!    `vs0`/`vs1`, mỗi cái root → mid → leaf (4 KiB ở `0x1000`) + mid 2 MiB ở `root[1]`
//!    (`0x40000000`); `root[2]` để trống cho lá 1 GiB (`0x80000000`); Endpoint `ep0` + Reply
//!    `rep0`; hai TCB `t0` (priority 6, có `fault_ep`) / `t1` (priority 5, procsrv) + SchedContext.
//! 3. `caps` trong tệp là **khai báo tên/kiểu** cho `check_scn.py` (máy kiểm tĩnh); runner dựng
//!    object qua rig chuẩn — **không** đọc `caps` (cùng quy ước kt24/kt25/kt26).
//! 4. Bước `vm_*` là **model API** của K2, **không** phải ABI: `spec/` không đổi (D2-65 Q6);
//!    `vm_expect_*`/`vm_check` là bước **kiểm** (không đổi trạng thái) — sai ⇒ panic.
//!
//! Tệp này **đọc đĩa** nên **không** chạy dưới Miri (isolation) — cùng quy ước `kt24_scn.rs`:
//! Miri của KT-42 chạy `--test kt42_vmcheck` + unit test `check_vm` trong `--lib`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vifast_sim::ipc::{Message, MessageInfo};
use vifast_sim::kobj::{
    FaultAccess, FaultKind, KError, Machine, MapAttrs, MapRights, ObjId, ObjType, SlotRef,
    TcbState, VmFaultReason,
};
use vifast_sim::procsrv_model::ProcsrvModel;

const S_UVA: usize = 0;
const S_UF4: usize = 1;
const S_UF2: usize = 2;
const S_UF1: usize = 3;
const S_EP: usize = 10;
const S_REP: usize = 11;
const S_TCB0: usize = 12;
const S_TCB1: usize = 13;
const S_SC0: usize = 20;
const S_CTL: usize = 40;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Một bước `.scn`: op + tham số `k=v` + kết quả kỳ vọng (`ok` hoặc tên mã lỗi).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Buoc {
    op: String,
    params: BTreeMap<String, String>,
    expect: String,
}

impl Buoc {
    fn p(&self, k: &str) -> &str {
        self.params
            .get(k)
            .map(String::as_str)
            .unwrap_or_else(|| panic!("bước {} thiếu tham số {k}", self.op))
    }

    fn p_u64(&self, k: &str) -> u64 {
        parse_u64(self.p(k))
    }

    fn p_u32(&self, k: &str) -> u32 {
        u32::try_from(self.p_u64(k)).expect("tham số vượt u32")
    }

    fn p_bool(&self, k: &str) -> bool {
        match self.p(k) {
            "true" => true,
            "false" => false,
            v => panic!("tham số {k}={v:?} không phải bool"),
        }
    }
}

fn parse_u64(v: &str) -> u64 {
    if let Some(hex) = v.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).unwrap_or_else(|e| panic!("số hex lạ {v:?}: {e}"))
    } else {
        v.parse()
            .unwrap_or_else(|e| panic!("số thập phân lạ {v:?}: {e}"))
    }
}

fn parse_rights(v: &str) -> MapRights {
    let mut out = MapRights::NONE;
    for part in v.split('|') {
        out = out
            | match part.trim() {
                "READ" => MapRights::READ,
                "WRITE" => MapRights::WRITE,
                "NONE" => MapRights::NONE,
                other => panic!("quyền lạ {other:?}"),
            };
    }
    out
}

fn parse_attrs(v: &str) -> MapAttrs {
    let mut out = MapAttrs::NONE;
    for part in v.split('|') {
        out = out
            | match part.trim() {
                "WB" => MapAttrs::WB,
                "WC" => MapAttrs::WC,
                "UC" => MapAttrs::UC,
                "DEVICE" => MapAttrs::DEVICE,
                "NX" => MapAttrs::NX,
                "EXEC" => MapAttrs::EXEC,
                "NONE" => MapAttrs::NONE,
                other => panic!("thuộc tính lạ {other:?}"),
            };
    }
    out
}

fn parse_access(v: &str) -> FaultAccess {
    match v {
        "Read" => FaultAccess::Read,
        "Write" => FaultAccess::Write,
        "Execute" => FaultAccess::Execute,
        other => panic!("access lạ {other:?}"),
    }
}

fn parse_reason(v: &str) -> VmFaultReason {
    match v {
        "NotMapped" => VmFaultReason::NotMapped,
        "Permission" => VmFaultReason::Permission,
        "Wx" => VmFaultReason::Wx,
        other => panic!("reason lạ {other:?}"),
    }
}

fn scn_path(ten: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/scn")
        .join(ten)
}

/// Đọc tệp `.scn`: `harts` + mọi chuỗi có `=>` (theo thứ tự xuất hiện — `setup` trước,
/// `thread` sau, đúng thứ tự chạy). Đọc tệp là nguồn kỳ vọng duy nhất — không chép tay.
fn doc(ten: &str) -> (u16, Vec<Buoc>) {
    let path = scn_path(ten);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    assert!(
        text.contains("version = 1"),
        "{ten}: fixture phải có version = 1"
    );
    let harts = text
        .lines()
        .find(|l| l.trim_start().starts_with("harts"))
        .and_then(|l| l.split_once('='))
        .map(|(_, v)| v.trim().parse().expect("harts phải là số"))
        .unwrap_or(1);
    let mut steps = Vec::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        let inner = &after[..end];
        rest = &after[end + 1..];
        let Some((body, expect)) = inner.rsplit_once("=>") else {
            continue;
        };
        let mut toks = body.split_whitespace();
        let op = toks.next().expect("bước rỗng").to_string();
        let mut params = BTreeMap::new();
        for tok in toks {
            let (k, v) = tok
                .split_once('=')
                .unwrap_or_else(|| panic!("{ten}: tham số không phải k=v: {tok:?}"));
            assert!(!params.contains_key(k), "{ten}: tham số {k} lặp");
            params.insert(k.to_string(), v.to_string());
        }
        steps.push(Buoc {
            op,
            params,
            expect: expect.trim().to_string(),
        });
    }
    assert!(!steps.is_empty(), "{ten}: không có bước nào");
    (harts, steps)
}

/// Rig chuẩn của 5 kịch bản VM (xem quy ước ở doc module).
struct Rig {
    m: Machine,
    vs: [ObjId; 2],
    f4: ObjId,
    f2: ObjId,
    f1: ObjId,
    t: [ObjId; 2],
    ep: ObjId,
    rep: ObjId,
    procsrv: ProcsrvModel,
}

impl Rig {
    fn new(harts: u16) -> Self {
        let mut m = Machine::with_harts(harts);
        let c = m.add_cnode(1024);
        let uva = m.add_untyped(0, 2 * 1024 * 1024);
        let uf4 = m.add_untyped(1 << 32, 4096);
        let uf2 = m.add_untyped(1 << 33, 4096);
        let uf1 = m.add_untyped(1 << 34, 4096);
        for (s, u) in [(S_UVA, uva), (S_UF4, uf4), (S_UF2, uf2), (S_UF1, uf1)] {
            let cap = m.root_cap(u);
            m.put_cap(c, s, cap).expect("cap Untyped");
        }
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c, S_CTL, ctl_cap).expect("cap SchedControl");
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c, S_EP, ep_cap).expect("cap fault_ep");
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c, S_REP, rep_cap).expect("cap Reply");

        let mut vs = [0u32; 2];
        for (i, v) in vs.iter_mut().enumerate() {
            let cap = m
                .vspace_create(slot(c, S_UVA))
                .unwrap_or_else(|e| panic!("vspace_create vs{i}: {e:?}"));
            *v = m.cap_info(cap).expect("cap VSpace").obj;
        }

        // Frame 3 cỡ (mỗi cỡ một cửa sổ PA riêng): chưa mapping.
        let f4 = rig_frame(&mut m, c, S_UF4, 12);
        let f2 = rig_frame(&mut m, c, S_UF2, 21);
        let f1 = rig_frame(&mut m, c, S_UF1, 30);

        // Cây PageTable cho mỗi VSpace: root[0] → mid → leaf (4 KiB), root[1] → mid 2 MiB.
        let mut next = 50usize;
        for v in vs {
            let root = m.vspace_root(v).expect("root VSpace");
            for (va, co_leaf) in [(0u64, true), (0x4000_0000, false)] {
                let cap = m
                    .retype(slot(c, S_UVA), ObjType::PageTable, 0, 0)
                    .expect("retype PageTable cấp 1");
                let mid = m.cap_info(cap).expect("cap").obj;
                m.put_cap(c, next, cap).expect("cap PageTable");
                next += 1;
                m.page_table_attach(root, va, mid)
                    .expect("attach mid vào root");
                if co_leaf {
                    let cap = m
                        .retype(slot(c, S_UVA), ObjType::PageTable, 0, 0)
                        .expect("retype PageTable cấp 0");
                    let leaf = m.cap_info(cap).expect("cap").obj;
                    m.put_cap(c, next, cap).expect("cap PageTable");
                    next += 1;
                    m.page_table_attach(mid, 0, leaf)
                        .expect("attach leaf vào mid");
                }
            }
        }

        let mut t = [0u32; 2];
        let mut t_s = [slot(c, S_TCB0), slot(c, S_TCB1)];
        for i in 0..2 {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c, S_TCB0 + i, cap).expect("cap TCB");
            t[i] = obj;
            t_s[i] = slot(c, S_TCB0 + i);
            let sc = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc);
            m.put_cap(c, S_SC0 + i, sc_cap).expect("cap SchedContext");
            m.sched_control_configure(slot(c, S_CTL), slot(c, S_SC0 + i), 4, 8)
                .expect("SchedControl.Configure");
            m.set_sched_params(t_s[i], Some(slot(c, S_SC0 + i)))
                .expect("set_sched_params");
            m.tcb_set_priority(t_s[i], if i == 0 { 6 } else { 5 }, 255)
                .expect("Tcb.SetPriority");
        }
        m.tcb_set_fault_ep(t_s[0], slot(c, S_EP))
            .expect("Tcb.SetFaultEp t0");
        m.transition_tcb(t[0], TcbState::Ready)
            .expect("t0 Inactive→Ready");
        let mut procsrv = ProcsrvModel::new(t[1], slot(c, S_EP), slot(c, S_REP));
        procsrv.track(t[0], t_s[0]);

        let _ = next;
        Self {
            m,
            vs,
            f4,
            f2,
            f1,
            t,
            ep,
            rep,
            procsrv,
        }
    }

    fn obj(&self, ten: &str) -> ObjId {
        match ten {
            "vs0" => self.vs[0],
            "vs1" => self.vs[1],
            "f4" => self.f4,
            "f2" => self.f2,
            "f1" => self.f1,
            "t0" => self.t[0],
            "t1" => self.t[1],
            "ep0" => self.ep,
            "rep0" => self.rep,
            other => panic!("tên object lạ {other:?}"),
        }
    }

    fn expect_ok(&self, b: &Buoc, r: Result<(), KError>) {
        match r {
            Ok(()) => assert_eq!(b.expect, "ok", "{}: kỳ vọng {}", b.op, b.expect),
            Err(e) => assert_eq!(
                b.expect,
                e.name(),
                "{}: kết quả {:?} ≠ kỳ vọng {}",
                b.op,
                e,
                b.expect
            ),
        }
    }

    fn expect_ok_string(&self, b: &Buoc, r: Result<(), String>) {
        match r {
            Ok(()) => assert_eq!(b.expect, "ok", "{}: kỳ vọng {}", b.op, b.expect),
            Err(e) => panic!("{}: bất biến vỡ: {e} (kỳ vọng {})", b.op, b.expect),
        }
    }

    fn chay(&mut self, b: &Buoc) {
        match b.op.as_str() {
            "vm_activate" => {
                let vs = self.obj(b.p("vs"));
                let r = self.m.vspace_activate(vs, b.p_u32("hart") as u16);
                self.expect_ok(b, r);
            }
            "vm_map" => {
                let vs = self.obj(b.p("vs"));
                let f = self.obj(b.p("frame"));
                let r = self
                    .m
                    .vspace_map_obj(
                        vs,
                        b.p_u64("va"),
                        f,
                        parse_rights(b.p("rights")),
                        parse_attrs(b.p("attrs")),
                    )
                    .map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_unmap" => {
                self.m
                    .set_current_hart(b.p_u32("hart") as u16)
                    .expect("set_current_hart");
                let vs = self.obj(b.p("vs"));
                let r = self.m.vspace_unmap(vs, b.p_u64("va")).map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_protect" => {
                self.m
                    .set_current_hart(b.p_u32("hart") as u16)
                    .expect("set_current_hart");
                let vs = self.obj(b.p("vs"));
                let r = self
                    .m
                    .vspace_protect(
                        vs,
                        b.p_u64("va"),
                        parse_rights(b.p("rights")),
                        parse_attrs(b.p("attrs")),
                    )
                    .map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_walk" => {
                let vs = self.obj(b.p("vs"));
                let r = self.m.vspace_walk(vs, b.p_u64("va")).map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_expect_map" => {
                let vs = self.obj(b.p("vs"));
                let f = self.obj(b.p("frame"));
                let va = b.p_u64("va");
                let m = self
                    .m
                    .vspace_walk(vs, va)
                    .unwrap_or_else(|e| panic!("{}: walk {va:#x}: {e:?}", b.op));
                assert_eq!(m.frame, f, "{}: frame sai", b.op);
                assert_eq!(
                    u32::from(m.size_bits),
                    b.p_u32("size"),
                    "{}: size_bits sai",
                    b.op
                );
                assert_eq!(
                    m.pa,
                    self.m.frame_address(f).expect("frame_address"),
                    "{}: pa sai",
                    b.op
                );
                self.expect_ok(b, Ok(()));
            }
            "vm_expect_counters" => {
                let f = self.obj(b.p("frame"));
                let meta = self.m.frame_meta(f).expect("frame_meta");
                assert_eq!(meta.maps, b.p_u32("maps"), "{}: maps sai", b.op);
                assert_eq!(meta.mapped_w, b.p_u32("w"), "{}: mapped_w sai", b.op);
                assert_eq!(meta.mapped_x, b.p_u32("x"), "{}: mapped_x sai", b.op);
                self.expect_ok(b, Ok(()));
            }
            "vm_expect_frames" => {
                assert_eq!(
                    self.m.frame_count(),
                    b.p("n").parse::<usize>().expect("n"),
                    "{}: frame_count sai",
                    b.op
                );
                self.expect_ok(b, Ok(()));
            }
            "vm_tlb_fill" => {
                let vs = self.obj(b.p("vs"));
                self.m
                    .set_current_hart(b.p_u32("hart") as u16)
                    .expect("set_current_hart");
                let r = self.m.tlb_translate(vs, b.p_u64("va")).map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_tlb_expect" => {
                let vs = self.obj(b.p("vs"));
                let present = self
                    .m
                    .tlb_lookup(b.p_u32("hart") as u16, vs, b.p_u64("va"))
                    .expect("tlb_lookup")
                    .is_some();
                assert_eq!(present, b.p_bool("present"), "{}: present sai", b.op);
                self.expect_ok(b, Ok(()));
            }
            "vm_shootdown_begin" => {
                let vs = self.obj(b.p("vs"));
                let r = self
                    .m
                    .tlb_shootdown_begin(b.p_u32("hart") as u16, vs, b.p_u64("va"))
                    .map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_shootdown_wait" => {
                let r = self.m.tlb_shootdown_wait(b.p_u32("hart") as u16);
                self.expect_ok(b, r);
            }
            "vm_expect_pending" => {
                let info = self
                    .m
                    .tlb_pending(b.p_u32("hart") as u16)
                    .expect("tlb_pending")
                    .expect("phải có shootdown đang chờ");
                kiem_shootdown(b, &info);
            }
            "vm_expect_shootdown" => {
                let info = self
                    .m
                    .last_tlb_shootdown()
                    .expect("phải có shootdown vừa xong");
                kiem_shootdown(b, &info);
            }
            "vm_dispatch" => {
                let hart = b.p_u32("hart") as u16;
                let got = self.m.dispatch(hart).expect("dispatch");
                assert_eq!(
                    got,
                    Some(self.obj(b.p("tcb"))),
                    "{}: dispatch chọn sai TCB",
                    b.op
                );
                self.expect_ok(b, Ok(()));
            }
            "vm_touch" => {
                let vs = self.obj(b.p("vs"));
                self.m
                    .set_current_hart(b.p_u32("hart") as u16)
                    .expect("set_current_hart");
                let r = self
                    .m
                    .vm_touch(vs, b.p_u64("va"), parse_access(b.p("access")));
                self.expect_ok(b, r);
            }
            "vm_expect_fault" => {
                let tcb = self.obj(b.p("tcb"));
                let vs = self.obj(b.p("vs"));
                let want = TcbState::BlockedOnSend {
                    ep: self.ep,
                    call: true,
                    fault: Some(FaultKind::Vm {
                        vspace: vs,
                        va: b.p_u64("va"),
                        access: parse_access(b.p("access")),
                        reason: parse_reason(b.p("reason")),
                    }),
                };
                assert_eq!(
                    self.m.tcb_state(tcb).expect("tcb_state"),
                    want,
                    "{}: fault sai payload/đích",
                    b.op
                );
                self.expect_ok(b, Ok(()));
            }
            "vm_expect_state" => {
                let st = self.m.tcb_state(self.obj(b.p("tcb"))).expect("tcb_state");
                let ok = match b.p("state") {
                    "Ready" => st == TcbState::Ready,
                    "Running" => matches!(st, TcbState::Running { .. }),
                    "BlockedOnReply" => {
                        matches!(st, TcbState::BlockedOnReply { reply } if reply == self.rep)
                    }
                    "BlockedOnSend" => matches!(st, TcbState::BlockedOnSend { .. }),
                    other => panic!("trạng thái lạ {other:?}"),
                };
                assert!(
                    ok,
                    "{}: trạng thái {:?} không khớp {}",
                    b.op,
                    st,
                    b.p("state")
                );
                self.expect_ok(b, Ok(()));
            }
            "vm_procsrv_recv" => {
                let r = self.procsrv.recv_exit(&mut self.m).map(|_| ());
                self.expect_ok(b, r);
            }
            "vm_procsrv_resume" => {
                let tcb = self.obj(b.p("tcb"));
                let info = MessageInfo::new(0, 0, false, 0, 0).expect("MessageInfo rỗng");
                let msg = Message::new(info, &[]).expect("Message rỗng");
                let r = self.procsrv.resume_fault(&mut self.m, tcb, msg);
                self.expect_ok(b, r);
            }
            "vm_check" => {
                let r = self.m.check_vm();
                self.expect_ok_string(b, r);
            }
            other => panic!("KT-42: runner chưa cài op {other:?}"),
        }
    }
}

fn rig_frame(m: &mut Machine, c: ObjId, src: usize, size_bits: u32) -> ObjId {
    let cap = m
        .retype(slot(c, src), ObjType::Frame, size_bits, 0)
        .unwrap_or_else(|e| panic!("retype Frame {size_bits}: {e:?}"));
    let f = m.cap_info(cap).expect("cap Frame").obj;
    // Cap Frame không dùng tiếp; đặt vào slot tạm để `check_caps` vẫn đối xứng.
    let s = 100 + size_bits as usize;
    m.put_cap(c, s, cap).expect("cap Frame vào CNode");
    f
}

fn kiem_shootdown(b: &Buoc, info: &vifast_sim::tlb::TlbShootdownInfo) {
    assert_eq!(
        info.targets.bits(),
        b.p_u64("targets"),
        "{}: targets sai",
        b.op
    );
    assert_eq!(info.acked.bits(), b.p_u64("acked"), "{}: acked sai", b.op);
    assert_eq!(
        u64::from(info.messages),
        b.p_u64("messages"),
        "{}: messages sai",
        b.op
    );
    assert_eq!(
        info.completed,
        b.p_bool("completed"),
        "{}: completed sai",
        b.op
    );
    assert_eq!(b.expect, "ok", "{}: kỳ vọng {}", b.op, b.expect);
}

fn run_scn(ten: &str) {
    let (harts, steps) = doc(ten);
    let mut r = Rig::new(harts);
    for step in &steps {
        r.chay(step);
    }
    println!("# SCN OK: {ten} (harts={harts}, {} bước)", steps.len());
}

#[test]
fn scn22_map_unmap_walk_bo_cuc() {
    run_scn("22-frame-map-unmap-walk.scn");
}

#[test]
fn scn23_wx_violation() {
    run_scn("23-wx-violation-counters.scn");
}

#[test]
fn scn24_shootdown_2_hart() {
    run_scn("24-shootdown-2-hart.scn");
}

#[test]
fn scn25_vm_fault_resume() {
    run_scn("25-vm-fault-notmapped-resume.scn");
}

#[test]
fn scn26_shared_readonly() {
    run_scn("26-shared-readonly-frame.scn");
}
