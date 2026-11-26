// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-56 — pha `FaultKind` thật + W^X trên đường công khai (TAP 38..42 của ảnh `ktest`).
//!
//! # TAP
//!
//! - 38 `faultkind vm load` — probe **load** tới VA chưa map ⇒ trap phần cứng `scause=13`
//!   (`stval` = VA), phân loại `Vm{access=Read, reason=NotMapped}`.
//! - 39 `faultkind vm store` — probe **store** ⇒ `scause=15`, `Vm{access=Write, reason=NotMapped}`.
//! - 40 `faultkind vm insn` — probe **fetch** ⇒ `scause=12`, `Vm{access=Execute, reason=NotMapped}`.
//! - 41 `wx violation qua ABI` — `map` một lượt `W|X` qua đường công khai ⇒ `ObjError::WxViolation`
//!   + mã lỗi ABI **4** (`spec/abi.toml` `[error]`), không ghi PTE, không panic.
//! - 42 `device exec` — nợ **L3 KT-52** (D2-67 "toàn cục"): Frame đã `Device` ở `vs0` **không**
//!   được `RX` ở `vs1`, và `protect` sang `Device+EXEC` bị từ chối; `mapped_device` đối xứng.
//!
//! Trap ở 38..40 là **phần cứng thật** (`crate::probe`, thực thi instruction lỗi trong S-mode rồi
//! resume ngay sau nó) — không tiêm `scause`, không phân loại từ PTE mô phỏng. `reason` lấy từ
//! PTE thật của **root đang chạy** qua `mmu::walk_sw` (hàm thuần đọc bảng thật).
//!
//! Số đo/định danh do boot in (`# faultkind:`); tài liệu không chép tay (QD §9 Q12). Xem
//! `v2/docs/k3-riscv64.md` §19.

use core::fmt::Write;

use vifast_core::vm::{CapId, MapFlags, ObjError, ObjKind};
use vifast_plat_common::fault::{
    classify_trap, FaultAccess, FaultKind, PteFacts, TrapFacts, VmFaultReason,
};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::probe::{self, ProbeKind};
use vifast_plat_riscv64::{mmu, Riscv64Virt};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// VA **chưa map** trong root kernel (root index 1 — vùng user để trống trong ảnh ktest). Dùng
/// cho probe load/store/fetch; `mmu::walk_sw` phải trả invalid.
const FAULT_VA: u64 = 0x5000_0000;
/// VA cho các ca object layer (bảng riêng của VSpace, không phải root kernel).
const MAP_VA: u64 = 0x1000;
/// VA thứ hai trong VSpace khác cho ca Device toàn cục.
const MAP_VA2: u64 = 0x2000;

/// `WxViolation` phải trùng mã 4 của `spec/abi.toml` `[error]` (D2-56a; KT-56 kiểm **qua ABI**).
const ABI_ERR_WX_VIOLATION: u16 = 4;

/// Chạy một probe trap thật + phân loại; trả `true` ⇔ đúng `Vm{access, va=FAULT_VA, NotMapped}`.
fn probe_case(kind: ProbeKind, access: FaultAccess, expect_scause: u64) -> bool {
    let Some(trap) = probe::probe_fault(kind, FAULT_VA) else {
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# faultkind: case={} access={} trap=0 scause=none (KHONG fault — probe sai)",
                kind_name(kind),
                access.name()
            );
        }
        return false;
    };
    let walk = mmu::walk_sw(trap.stval);
    let pte = PteFacts::new(walk.valid, walk.leaf, walk.raw());
    let facts = TrapFacts {
        scause: trap.scause,
        stval: trap.stval,
        sepc: trap.sepc,
        from_user: trap.spp == 0,
    };
    let kind = classify_trap(&facts, 0, Some(pte));
    let want = FaultKind::Vm {
        vspace: 0,
        va: FAULT_VA,
        access,
        reason: VmFaultReason::NotMapped,
    };
    let ok = trap.scause == expect_scause
        && trap.stval == FAULT_VA
        && trap.spp == 1 // trap từ S-mode (probe chạy trong kernel)
        && kind == Some(want);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# faultkind: case={} access={} scause=0x{:x} stval=0x{:x} sepc=0x{:x} spp={} kind=",
            kind_name(kind_of(access)),
            access.name(),
            trap.scause,
            trap.stval,
            trap.sepc,
            trap.spp
        );
        match kind {
            Some(kind) => {
                let _ = write!(w, "{kind}");
            }
            None => {
                let _ = write!(w, "none");
            }
        }
        let _ = writeln!(w, " src=trap walk=0 ok={}", ok as u8);
    }
    ok
}

const fn kind_name(kind: ProbeKind) -> &'static str {
    match kind {
        ProbeKind::Load => "load",
        ProbeKind::Store => "store",
        ProbeKind::Exec => "insn",
    }
}

const fn kind_of(access: FaultAccess) -> ProbeKind {
    match access {
        FaultAccess::Read => ProbeKind::Load,
        FaultAccess::Write => ProbeKind::Store,
        FaultAccess::Execute => ProbeKind::Exec,
    }
}

/// In một dòng evidence + TAP theo `ok`; trả số lỗi (0/1).
fn tap(ok: bool, number: u32, name: &str) -> usize {
    if ok {
        PLATFORM.put_str("ok ");
        print_u64(number as u64);
        PLATFORM.put_str(" - ");
        PLATFORM.put_str(name);
        PLATFORM.put_str("\n");
        0
    } else {
        PLATFORM.put_str("not ok ");
        print_u64(number as u64);
        PLATFORM.put_str(" - ");
        PLATFORM.put_str(name);
        PLATFORM.put_str(": faultkind/W^X khong dat\n");
        1
    }
}

/// In số thập phân không cần `alloc`.
fn print_u64(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        PLATFORM.put_str(s);
    }
}

/// Dựng VSpace + Frame 4 KiB từ Untyped boot (`aobj::init()` idempotent).
fn rig() -> Option<(CapId, CapId, CapId)> {
    let report = aobj::init();
    let m = aobj::machine();
    let vs0 = m.retype(report.untyped0, ObjKind::VSpace, 0).ok()?;
    let vs1 = m.retype(report.untyped0, ObjKind::VSpace, 0).ok()?;
    let fr = m.retype(report.untyped0, ObjKind::Frame, 12).ok()?;
    Some((vs0, vs1, fr))
}

/// TAP 41 — một lượt `map` `W|X` qua đường công khai ⇒ `WxViolation` + mã ABI 4, atomic.
fn tap_wx_abi(m: &mut vifast_core::vm::Machine<aobj::PhysmapMem>) -> usize {
    let Some((vs, _vs1, fr)) = rig() else {
        PLATFORM.put_str("not ok 41 - wx violation qua ABI: retype rig that bai\n");
        return 1;
    };
    let wx = MapFlags {
        read: true,
        write: true,
        exec: true,
        user: false,
        device: false,
    };
    let before = m.counts();
    let err = m.map(vs, MAP_VA, fr, wx).err();
    let counts_ok = m.counts() == before;
    let pte_invalid = m.walk(vs, MAP_VA).map(|w| !w.valid).unwrap_or(false);
    let counts = m.frame_counts(fr).unwrap_or((1, 1, 1));
    let abi = err.and_then(ObjError::abi_code);
    let ok = err == Some(ObjError::WxViolation)
        && abi == Some(ABI_ERR_WX_VIOLATION)
        && counts_ok
        && pte_invalid
        && counts == (0, 0, 0);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# faultkind: case=wx-abi map=W|X err={} abi={} atomic={} pte=0 counts={}/{}/{} panic=0 ok={}",
            err.map(ObjError::code_name).unwrap_or("none"),
            abi.map(|v| v as u64).unwrap_or(0),
            counts_ok as u8,
            counts.0,
            counts.1,
            counts.2,
            ok as u8
        );
    }
    let _ = m.revoke(vs);
    let _ = m.revoke(fr);
    tap(ok, 41, "wx violation qua ABI")
}

/// TAP 42 — `Device` toàn cục (nợ L3 KT-52): Device ở vs0 chặn RX ở vs1; protect Device+EXEC bị
/// từ chối; `mapped_device` đối xứng; dọn sạch.
fn tap_device_exec(m: &mut vifast_core::vm::Machine<aobj::PhysmapMem>) -> usize {
    let Some((vs0, vs1, fr)) = rig() else {
        PLATFORM.put_str("not ok 42 - device exec: retype rig that bai\n");
        return 1;
    };
    let mut ok = true;
    ok &= m.map(vs0, MAP_VA, fr, MapFlags::DEVICE_RW).is_ok();
    ok &= m.frame_device(fr).unwrap_or(0) == 1;
    let err_map = m.map(vs1, MAP_VA2, fr, MapFlags::RX).err();
    ok &= err_map == Some(ObjError::WxViolation);
    ok &= m.walk(vs1, MAP_VA2).map(|w| !w.valid).unwrap_or(false);
    let dev_exec = MapFlags {
        read: true,
        write: false,
        exec: true,
        user: false,
        device: true,
    };
    let err_protect = m.protect(vs0, MAP_VA, dev_exec).err();
    ok &= err_protect == Some(ObjError::WxViolation);
    ok &= m.frame_counts(fr).unwrap_or((0, 0, 0)) == (1, 1, 0);
    let abi = err_map.and_then(ObjError::abi_code);
    ok &= abi == Some(ABI_ERR_WX_VIOLATION);
    let check_ok = m.check_objmem().is_ok();
    ok &= check_ok;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# faultkind: case=device-exec device_count=1 map_rx={} protect_dev_rx={} abi={} atomic={} check={} ok={}",
            err_map.map(ObjError::code_name).unwrap_or("none"),
            err_protect.map(ObjError::code_name).unwrap_or("none"),
            abi.map(|v| v as u64).unwrap_or(0),
            (err_map == Some(ObjError::WxViolation) && err_protect == Some(ObjError::WxViolation)) as u8,
            check_ok as u8,
            ok as u8
        );
    }
    let _ = m.unmap(vs0, MAP_VA);
    let _ = m.revoke(vs0);
    let _ = m.revoke(vs1);
    let _ = m.revoke(fr);
    tap(ok, 42, "device exec")
}

/// Pha `FaultKind`/W^X — trả số lỗi (0 = sạch); in TAP 38..42.
pub fn fault_phase() -> usize {
    let m = aobj::machine();
    let mut fails = 0usize;

    // 38..40 — trap **phần cứng thật** (probe S-mode load/store/fetch).
    fails += tap(
        probe_case(ProbeKind::Load, FaultAccess::Read, 13),
        38,
        "faultkind vm load",
    );
    fails += tap(
        probe_case(ProbeKind::Store, FaultAccess::Write, 15),
        39,
        "faultkind vm store",
    );
    fails += tap(
        probe_case(ProbeKind::Exec, FaultAccess::Execute, 12),
        40,
        "faultkind vm insn",
    );

    // 41..42 — W^X qua ABI/đường công khai + nợ L3 (Device toàn cục).
    fails += tap_wx_abi(m);
    fails += tap_device_exec(m);
    fails
}
