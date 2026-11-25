// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **conformance 5 `.scn` × sim/QEMU** của ảnh test `vifast-kbin-ktest` (KT-55).
//!
//! Nối sau pha object layer KT-52 (`ktest_obj::obj_phase`); TAP **37**
//! (`ok 37 - conformance 5/5`); trap lồng dời sang **43** (KT-56 thêm TAP 38..42).
//!
//! # Vì sao 5 ca + tập bước rút gọn
//!
//! Nguồn sự thật là `v2/test/scn/{22..26}-*.scn` (D2-69). Crate `vifast-scenario` **đọc** đúng các
//! tệp đó (nhúng bằng `include_str!`), chiếu sang tập bước mà **cả** sim host và kernel quan sát
//! được bằng trạng thái thật; lý do từng bước bị loại ghi ở doc crate:
//!
//! - Frame 1 GiB **chưa** cho map (KT-52 L7 / D2-66) ⇒ bỏ bước `frame=f1` / `va=0x80000000`;
//! - mô hình TLB per-hart (`vm_tlb_fill`/`vm_tlb_expect`) là model-only — TLB thật (stale→fresh,
//!   3 dạng `sfence.vma`) đã có TAP 25..29 của KT-51;
//! - `vm_dispatch`/`vm_expect_state`/`vm_procsrv_*` cần TCB/procsrv — K3 **chưa** có (Q13).
//!
//! # Định dạng dòng (chuẩn hoá, `vifast-traceformat::TraceLine`)
//!
//! ```text
//! # trace: <scn> <step> <op> <k=v>… => <kết quả>
//! ```
//!
//! Chỉ trường **quan sát được**: `va` (địa chỉ ảo — không phải PA), `frame` (tên), `rights`/`attrs`
//! (chuỗi của tệp), `size` (12/21), `maps`/`w`/`x`, `n`, `reason`, `targets`/`acked`/`completed`.
//! **Không** in PA tuyệt đối, thời gian, tick hay hart. `v2/test/conformance/trace_compare.py` so
//! từng dòng với sim; `check_scn.py --trace` kiểm chiều sim theo đúng hợp đồng này.
//!
//! # Red-first (D2-05) và lỗi platform do conformance phát hiện
//!
//! Trước KT-55 kernel **không** in `# trace:` nào ⇒ so trace rỗng (log `kt55-red-diff.log`), đỏ.
//! Khi chạy thật, ca `vm-wx` lộ **lỗi platform**: `Machine::map`/`protect` cho phép **một** mapping
//! `W|X`, tức tự tạo trạng thái vi phạm bất biến W^X toàn cục của `check_objmem` (D2-67) mà K2 từ
//! chối `WxViolation`. Sửa ở `obj/vspace.rs` + test host `wx_ca_3_cung_map_w_x_bi_tu_choi_atomic`
//! (đỏ trước, xanh sau) — không nới ngưỡng, không đổi `spec/`.
//!
//! Số đo do boot in (`# conf:` + `# trace:`) — `ktest.sh` grep đúng các dòng đó; tài liệu không chép
//! số tay (QD §9 Q12). Xem `v2/docs/k3-riscv64.md` §18.

use core::fmt::Write;

use vifast_plat_common::fault::{classify_trap, FaultKind, PteFacts, TrapFacts};
use vifast_plat_common::obj::{CapId, Machine, MapFlags, ObjError, ObjKind};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::obj::{self as aobj, PhysmapMem};
use vifast_plat_riscv64::probe::{self, ProbeKind};
use vifast_plat_riscv64::timer::TIMEBASE_HZ;
use vifast_plat_riscv64::tlb::FlushScope;
use vifast_plat_riscv64::tlb::ShootdownOutcome;
use vifast_plat_riscv64::{mmu, smp, tlb, Riscv64Virt};
use vifast_scenario::{IrStep, Op, RawStep, MAX_STEPS};
use vifast_traceformat::TraceLine;

static PLATFORM: Riscv64Virt = Riscv64Virt;

type Mac = Machine<PhysmapMem>;

/// 5 cặp `(id, tag, nội dung tệp)` — `include_str!` chính fixture của K2 (không chép tay, không
/// sửa): `.scn` đổi là kernel đổi theo ở lần build kế.
const SCNS: [(u8, &str, &str); 5] = [
    (
        22,
        "vm-map",
        include_str!("../../../test/scn/22-frame-map-unmap-walk.scn"),
    ),
    (
        23,
        "vm-wx",
        include_str!("../../../test/scn/23-wx-violation-counters.scn"),
    ),
    (
        24,
        "vm-tlb",
        include_str!("../../../test/scn/24-shootdown-2-hart.scn"),
    ),
    (
        25,
        "vm-fault",
        include_str!("../../../test/scn/25-vm-fault-notmapped-resume.scn"),
    ),
    (
        26,
        "vm-shared",
        include_str!("../../../test/scn/26-shared-readonly-frame.scn"),
    ),
];

/// Timeout một shootdown (2 s ảo — cùng KT-51; hết hạn ⇒ `not ok`, không treo máy).
const SHOOTDOWN_TIMEOUT: u64 = 2 * TIMEBASE_HZ;

/// Rig conformance: 2 VSpace + đúng 3 Frame (4 KiB/2 MiB/1 GiB) — khớp rig của sim và
/// `vm_expect_frames n=3` của các kịch bản.
struct Rig {
    vs: [CapId; 2],
    f4: CapId,
    f2: CapId,
    f1: CapId,
}

impl Rig {
    fn vspace(&self, name: &str) -> Option<CapId> {
        match name {
            "vs0" => Some(self.vs[0]),
            "vs1" => Some(self.vs[1]),
            _ => None,
        }
    }

    fn frame(&self, name: &str) -> Option<CapId> {
        match name {
            "f4" => Some(self.f4),
            "f2" => Some(self.f2),
            "f1" => Some(self.f1),
            _ => None,
        }
    }
}

/// Dựng rig từ arena boot: `aobj::init()` (idempotent) rồi Retype 2 VSpace + 3 Frame đúng cỡ.
fn build_rig() -> Option<Rig> {
    let report = aobj::init();
    let m = aobj::machine();
    let vs0 = match m.retype(report.untyped0, ObjKind::VSpace, 0) {
        Ok(c) => c,
        Err(e) => {
            report_rig_fail("vspace0", e);
            return None;
        }
    };
    let vs1 = match m.retype(report.untyped0, ObjKind::VSpace, 0) {
        Ok(c) => c,
        Err(e) => {
            report_rig_fail("vspace1", e);
            return None;
        }
    };
    // PA của Untyped cấp theo watermark tăng dần + căn cỡ **tại thời điểm cấp** ⇒ cấp Frame
    // lớn trước (2 MiB rồi 4 KiB), nếu không 2 MiB sẽ lệch watermark và trả `Alignment`
    // (hành vi `pa_alloc` của KT-52 — không đổi, chỉ dùng đúng thứ tự).
    let f2 = match m.retype(report.untyped0, ObjKind::Frame, 21) {
        Ok(c) => c,
        Err(e) => {
            report_rig_fail("frame2m", e);
            return None;
        }
    };
    let f4 = match m.retype(report.untyped0, ObjKind::Frame, 12) {
        Ok(c) => c,
        Err(e) => {
            report_rig_fail("frame4k", e);
            return None;
        }
    };
    // Frame thứ ba: **4 KiB** (không phải 1 GiB) — mọi bước `frame=f1` đã bị tập con
    // conformance loại (KT-52 L7: 1 GiB chưa cho map), và Frame 1 GiB từ cửa sổ PA của u1
    // (0x8000_0000) **chồng** cửa sổ u0 (0x8200_0000…) khi cùng sống ⇒ `AlreadyMapped`. Cỡ Frame
    // 1 GiB vẫn được KT-52 TAP 32 chứng minh; ở đây chỉ cần **đếm** `n=3` khớp sim.
    let f1 = match m.retype(report.untyped0, ObjKind::Frame, 12) {
        Ok(c) => c,
        Err(e) => {
            report_rig_fail("frame3", e);
            return None;
        }
    };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# conf rig: arena=0x{:x} vs=2 frames=3 obj={} caps={}",
            report.arena_base,
            m.counts().objects,
            m.counts().caps
        );
    }
    Some(Rig {
        vs: [vs0, vs1],
        f4,
        f2,
        f1,
    })
}

/// In lý do dựng rig thất bại (giữ chữ ký `?` gọn ở trên; chỉ dùng khi lỗi).
fn report_rig_fail(what: &str, e: ObjError) {
    let mut w = PlatformWriter(&PLATFORM);
    let _ = writeln!(
        w,
        "not ok - conf rig: retype {what} loi {}",
        aobj::error_name(e)
    );
}

fn drain(m: &mut Mac) -> usize {
    m.drain_flushes(aobj::run_flush)
}

/// Quyền của `.scn` (`READ|WRITE`, …) → `MapFlags`; `attrs` `WB` không phải Device.
fn flags_of(rights: &str, attrs: &str) -> MapFlags {
    let mut flags = MapFlags {
        read: false,
        write: false,
        exec: false,
        user: false,
        device: false,
    };
    for part in rights.split('|') {
        match part {
            "READ" => flags.read = true,
            "WRITE" => flags.write = true,
            "EXEC" => flags.exec = true,
            _ => {}
        }
    }
    // `.scn` tách **quyền dữ liệu** (`rights`) khỏi **thuộc tính** (`attrs`): `attrs` mang
    // `EXEC`/`NX` (và `DEVICE`). K2 (`parse_attrs`) coi `EXEC` là bit thực thi ⇒ kernel phải đọc
    // cùng nghĩa, nếu không `READ attrs=WB|EXEC` bị map thành R|NX và W^X không bao giờ chạy.
    for part in attrs.split('|') {
        match part {
            "EXEC" => flags.exec = true,
            "NX" => flags.exec = false,
            "DEVICE" => flags.device = true,
            _ => {}
        }
    }
    flags
}

/// `size_bits` của leaf từ cấp dừng (`12 + 9·level`) — cùng công thức `vifast-pte`.
fn size_bits_of_level(level: u8) -> u64 {
    12 + 9 * u64::from(level)
}

fn parse_u64(v: &str) -> u64 {
    if let Some(hex) = v.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).unwrap_or(0)
    } else {
        v.parse().unwrap_or(0)
    }
}

/// Ghi một trường của bước: `va` luôn in hex, còn lại in chuỗi như tệp.
fn field_param(line: &mut TraceLine<192>, step: &IrStep<'_>, key: &str) {
    if let Some(v) = step.get(key) {
        if key == "va" {
            line.field_hex(key, parse_u64(v));
        } else {
            line.field_str(key, v);
        }
    }
}

/// In `# trace:` của bước; trả `false` nếu bộ đệm tràn (ca đỏ).
fn emit(line: &TraceLine<192>) -> bool {
    if line.truncated() {
        return false;
    }
    PLATFORM.put_str(line.as_str());
    true
}

/// Chạy một ca: `Some((số bước trace, số lỗi))`; `None` ⇔ ca bị bỏ vì smp chưa đủ hart.
fn run_case(id: u8, tag: &str, text: &str, m: &mut Mac, rig: &Rig) -> Option<(usize, usize)> {
    if id == 24 && smp::online_mask().count_ones() < 2 {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# conf: scn=24 name=vm-tlb steps=0 skipped=smp{}",
            smp::online_mask().count_ones()
        );
        return None;
    }
    let mut raw = [RawStep::EMPTY; MAX_STEPS];
    let n_raw = vifast_scenario::parse(text, &mut raw);
    let mut ir = [IrStep::EMPTY; MAX_STEPS];
    let n_ir = vifast_scenario::project(id, &raw[..n_raw], &mut ir);

    // Ca 24 mô hình `active_on = {0,1}` (2 hart, D2-78): đặt **trạng thái thật** rồi khôi phục.
    let prev_active = tlb::active_on();
    if id == 24 {
        tlb::test_active_on_set(0b11);
    }

    let mut fails = 0usize;
    for step in &ir[..n_ir] {
        let mut line: TraceLine<192> = TraceLine::new();
        line.start(id, step.step, step.op_name());
        let mut result: &'static str = "ok";
        match step.op {
            Op::Map => {
                let vs = step.get("vs").and_then(|v| rig.vspace(v));
                let fr = step.get("frame").and_then(|v| rig.frame(v));
                match (vs, fr) {
                    (Some(vs), Some(fr)) => {
                        let flags = flags_of(
                            step.get("rights").unwrap_or(""),
                            step.get("attrs").unwrap_or(""),
                        );
                        let r = m.map(vs, parse_u64(step.get("va").unwrap_or("0x0")), fr, flags);
                        drain(m);
                        result = match r {
                            Ok(()) => "ok",
                            Err(e) => aobj::error_name(e),
                        };
                    }
                    _ => {
                        fails += 1;
                        result = "BadObject";
                    }
                }
                for key in ["va", "frame", "rights", "attrs"] {
                    field_param(&mut line, step, key);
                }
            }
            Op::Unmap => {
                let vs = step.get("vs").and_then(|v| rig.vspace(v));
                match vs {
                    Some(vs) => {
                        let r = m.unmap(vs, parse_u64(step.get("va").unwrap_or("0x0")));
                        drain(m);
                        result = match r {
                            Ok(()) => "ok",
                            Err(e) => aobj::error_name(e),
                        };
                    }
                    None => {
                        fails += 1;
                        result = "BadObject";
                    }
                }
                field_param(&mut line, step, "va");
            }
            Op::Protect => {
                let vs = step.get("vs").and_then(|v| rig.vspace(v));
                match vs {
                    Some(vs) => {
                        let flags = flags_of(
                            step.get("rights").unwrap_or(""),
                            step.get("attrs").unwrap_or(""),
                        );
                        let r = m.protect(vs, parse_u64(step.get("va").unwrap_or("0x0")), flags);
                        drain(m);
                        result = match r {
                            Ok(()) => "ok",
                            Err(e) => aobj::error_name(e),
                        };
                    }
                    None => {
                        fails += 1;
                        result = "BadObject";
                    }
                }
                for key in ["va", "rights", "attrs"] {
                    field_param(&mut line, step, key);
                }
            }
            Op::Walk => {
                let vs = step.get("vs").and_then(|v| rig.vspace(v));
                let va = parse_u64(step.get("va").unwrap_or("0x0"));
                let present = matches!(vs.map(|vs| m.walk(vs, va)), Some(Ok(info)) if info.valid && info.leaf);
                result = if present { "ok" } else { "NotMapped" };
                line.field_hex("va", va);
            }
            Op::ExpectMap => {
                let va = parse_u64(step.get("va").unwrap_or("0x0"));
                let vs = step.get("vs").and_then(|v| rig.vspace(v));
                let fr = step.get("frame").and_then(|v| rig.frame(v));
                let want = parse_u64(step.get("size").unwrap_or("12"));
                line.field_hex("va", va)
                    .field_str("frame", step.get("frame").unwrap_or("?"));
                match (vs.map(|vs| m.walk(vs, va)), fr) {
                    (Some(Ok(info)), Some(fr)) if info.valid && info.leaf => {
                        let got = size_bits_of_level(info.level);
                        let pa_ok = m.frame_pa(fr).map(|pa| pa == info.phys).unwrap_or(false);
                        line.field_u64("size", got);
                        if got != want || !pa_ok {
                            fails += 1;
                            result = "Mismatch";
                        }
                    }
                    _ => {
                        line.field_u64("size", 0);
                        fails += 1;
                        result = "NotMapped";
                    }
                }
            }
            Op::Counters => {
                let fr = step.get("frame").and_then(|v| rig.frame(v));
                let (maps, w, x) = fr
                    .and_then(|fr| m.frame_counts(fr).ok())
                    .unwrap_or((0, 0, 0));
                line.field_str("frame", step.get("frame").unwrap_or("?"))
                    .field_u64("maps", u64::from(maps))
                    .field_u64("w", u64::from(w))
                    .field_u64("x", u64::from(x));
                let want = (
                    parse_u64(step.get("maps").unwrap_or("0")),
                    parse_u64(step.get("w").unwrap_or("0")),
                    parse_u64(step.get("x").unwrap_or("0")),
                );
                if (u64::from(maps), u64::from(w), u64::from(x)) != want {
                    fails += 1;
                    result = "Mismatch";
                }
            }
            Op::Frames => {
                let got = m.counts().frames as u64;
                line.field_u64("n", got);
                if got != parse_u64(step.get("n").unwrap_or("0")) {
                    fails += 1;
                    result = "Mismatch";
                }
            }
            Op::Check => match m.check_objmem() {
                Ok(_) => {}
                Err(e) => {
                    fails += 1;
                    result = aobj::error_name(e);
                }
            },
            Op::Activate => {
                let hart = parse_u64(step.get("hart").unwrap_or("0")) as usize;
                if !tlb::activate(hart) {
                    fails += 1;
                    result = "Inactive";
                }
                field_param(&mut line, step, "vs");
            }
            Op::Shootdown => {
                let va = parse_u64(step.get("va").unwrap_or("0x0"));
                let outcome = tlb::shootdown(FlushScope::Page { va }, SHOOTDOWN_TIMEOUT);
                let info = tlb::last_shootdown();
                result = match outcome {
                    ShootdownOutcome::Completed => "ok",
                    _ => outcome.name(),
                };
                line.field_hex("va", va)
                    .field_hex("targets", info.targets)
                    .field_hex("acked", info.acked)
                    .field_u64("completed", u64::from(info.completed));
            }
            Op::Fault => {
                let va = parse_u64(step.get("va").unwrap_or("0x0"));
                let access = step.get("access").unwrap_or("Read");
                let want_reason = step.get("reason").unwrap_or("ok");
                let vs = step.get("vs").and_then(|v| rig.vspace(v));
                match vs {
                    Some(vs) => match probe_vm_fault(m, vs, va, access, want_reason) {
                        Ok(kind) => {
                            let got = match kind {
                                FaultKind::Vm { reason, .. } => reason.name(),
                                _ => "NotVm",
                            };
                            line.field_hex("va", va)
                                .field_str("access", access)
                                .field_str("reason", got);
                            // Kỳ vọng của bước `vm_expect_fault` là `reason=<…>`; `expect` của
                            // tệp là `ok` (fault đúng payload) nên so với `reason`, không `ok`.
                            if got != want_reason {
                                fails += 1;
                                result = "Mismatch";
                            }
                        }
                        Err(why) => {
                            line.field_hex("va", va)
                                .field_str("access", access)
                                .field_str("reason", "NoTrap");
                            fails += 1;
                            result = why;
                        }
                    },
                    None => {
                        line.field_hex("va", va)
                            .field_str("access", access)
                            .field_str("reason", "BadObject");
                        fails += 1;
                        result = "BadObject";
                    }
                }
            }
        }
        if result != step.expect {
            fails += 1;
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok - conf {tag} b{} {}: quan sát {:?} ≠ kỳ vọng {:?}",
                step.step,
                step.op_name(),
                result,
                step.expect
            );
        }
        line.result(result);
        if !emit(&line) {
            fails += 1;
        }
    }

    if id == 24 {
        tlb::test_active_on_set(prev_active);
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# conf: scn={id} name={tag} steps={n_ir} ok={}",
            u8::from(fails == 0)
        );
    }
    Some((n_ir, fails))
}

/// **Trap phần cứng thật** cho một bước `vm_expect_fault` (KT-56 — thay phân loại PTE của KT-55).
///
/// Cơ chế: nếu VA đã có leaf trong rig VSpace, **scratch-map** đúng PA đó vào root kernel tại
/// **đúng VA kịch bản** với cờ làm phép truy cập `access` fault (bảng cấp 0 dự phòng — xem
/// `vifast_plat_riscv64::mmu::map_scratch_4k`); nếu chưa map thì để nguyên. Sau đó một
/// instruction load/store/jump thật của `crate::probe` gây trap; `scause/stval/sepc` lấy từ
/// trapframe phần cứng, `reason` lấy từ PTE **đã cấu hình cho phép thử** (đúng luật `D2-56a`).
///
/// Trả `Err(mã)` khi không có trap (kỳ vọng sai — ca đỏ, không im lặng) hoặc trap trả biến thể
/// không phải `Vm`.
fn probe_vm_fault(
    m: &Mac,
    vs: CapId,
    va: u64,
    access: &str,
    want_reason: &str,
) -> Result<FaultKind, &'static str> {
    let walk = m.walk(vs, va).ok();
    let mapped = walk.filter(|w| w.valid && w.leaf);
    let (r, w, x) = scratch_rights(access, want_reason);
    let scratch = match mapped {
        Some(info) => mmu::map_scratch_4k(va, info.phys, r, w, x),
        None => false,
    };
    // **Đọc leaf thật** từ bảng trang đang hoạt động (root kernel) — không dựng PTE bằng tay:
    // `reason` phải suy từ entry phần cứng sẽ dùng, không phải từ biến `(r,w,x)` của test.
    // Làm **trước** khi gỡ scratch để entry còn đó.
    let pte = {
        let walk_sw = mmu::walk_sw(va);
        Some(PteFacts::new(walk_sw.valid, walk_sw.leaf, walk_sw.raw()))
    };
    let probe_kind = match access {
        "Write" => ProbeKind::Store,
        "Execute" => ProbeKind::Exec,
        _ => ProbeKind::Load,
    };
    let trap = probe::probe_fault(probe_kind, va);
    if scratch {
        let _ = mmu::unmap_scratch_4k(va);
    }
    let Some(trap) = trap else {
        emit_faultprobe(va, access, 0, 0, 0, "NoTrap", scratch, mapped.is_some(), 0);
        return Err("NoTrap");
    };
    let facts = TrapFacts {
        scause: trap.scause,
        stval: trap.stval,
        sepc: trap.sepc,
        from_user: trap.spp == 0,
    };
    let kind = classify_trap(&facts, u64::from(m.vspace_asid(vs).unwrap_or(0)), pte);
    let pte_raw = pte.map(|p| p.raw).unwrap_or(0);
    let reason = match kind {
        Some(FaultKind::Vm { reason, .. }) => reason.name(),
        Some(FaultKind::UserException { .. }) => "UserException",
        _ => "NotVm",
    };
    emit_faultprobe(
        va,
        access,
        trap.scause,
        trap.stval,
        trap.sepc,
        reason,
        scratch,
        mapped.is_some(),
        pte_raw,
    );
    match kind {
        Some(kind @ FaultKind::Vm { .. }) => {
            // D2-56a: `va` của `FaultKind` phải là `stval` của trap.
            let ok_stval = matches!(kind, FaultKind::Vm { va: got, .. } if got == trap.stval);
            if !ok_stval || trap.stval != va {
                return Err("StvalMismatch");
            }
            Ok(kind)
        }
        Some(_) => Err("NotVm"),
        None => Err("NoFault"),
    }
}

/// Quyền của **scratch leaf** để phép thử `access` fault đúng `want_reason`:
/// `NotMapped` không map; `Permission` thiếu đúng quyền (và không tạo W^X); `Wx` tạo thế W+X.
fn scratch_rights(access: &str, want_reason: &str) -> (bool, bool, bool) {
    match (access, want_reason) {
        ("Write", "Permission") => (true, false, false),
        ("Write", "Wx") => (true, false, true),
        ("Execute", "Permission") => (true, false, false),
        ("Execute", "Wx") => (true, true, false),
        ("Read", "Permission") => (false, false, true),
        _ => (true, false, false),
    }
}

/// In evidence `# faultprobe:` — chứng minh trap đến từ **phần cứng** (scause/stval/sepc thật).
/// 9 tham số là 9 trường **phần cứng** của một trap (không gộp struct: chỉ dùng một lần ở đây).
#[allow(clippy::too_many_arguments)]
fn emit_faultprobe(
    va: u64,
    access: &str,
    scause: u64,
    stval: u64,
    sepc: u64,
    reason: &str,
    scratch: bool,
    mapped: bool,
    pte_raw: u64,
) {
    let mut w = PlatformWriter(&PLATFORM);
    let _ = writeln!(
        w,
        "# faultprobe: va=0x{va:x} access={access} scause=0x{scause:x} stval=0x{stval:x} sepc=0x{sepc:x} pte=0x{pte_raw:x} reason={reason} mapped={} scratch={} src=trap",
        u8::from(mapped),
        u8::from(scratch)
    );
}

/// Pha conformance — trả số lỗi (0 = sạch); tự in TAP 37 khi sạch.
pub fn conf_phase() -> usize {
    // D2-80: định danh **ảnh** (16 hex FNV-1a của `.text` đã nạp) — runner bắt buộc đối chiếu với
    // tệp ảnh; log không có/lệch dòng này bị từ chối (chống ghép trace từ nguồn khác).
    vifast_plat_riscv64::report_image_digest();
    let mut fails = 0usize;
    let mut cases = 0usize;
    let mut steps = 0usize;
    for (id, tag, text) in SCNS {
        // Mỗi ca một rig **mới** (như sim dựng `Rig::new` cho từng tệp): trạng thái mapping/đếm
        // của ca trước không được rò sang ca sau (nếu không ca 25/26 kế thừa mapping của ca 23).
        let Some(rig) = build_rig() else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "not ok - conf {tag}: dựng rig (VSpace/Frame) thất bại");
            fails += 1;
            continue;
        };
        let m = aobj::machine();
        match run_case(id, tag, text, &mut *m, &rig) {
            Some((n, f)) => {
                steps += n;
                fails += f;
                if f == 0 {
                    cases += 1;
                }
            }
            None => {
                // Ca 24 bỏ trên smp<2: tính là đạt có điều kiện; log smp≥2 là nguồn so trace.
                cases += 1;
            }
        }
    }
    if fails == 0 && cases == 5 {
        // D2-80/DoD: `-smp 1` không chạy ca 24 (cần 2 hart thật) ⇒ nhãn TAP phải nói đúng
        // "4/5 + 1 skipped", không được claim 5/5 khi một ca vắng.
        if smp::online_mask().count_ones() >= 2 {
            PLATFORM.put_str("ok 37 - conformance 5/5\n");
        } else {
            PLATFORM.put_str("ok 37 - conformance 4/5 + 1 skipped\n");
        }
        0
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 37 - conformance {cases}/5: {fails} lỗi, {steps} bước trace"
        );
        fails.max(1)
    }
}
