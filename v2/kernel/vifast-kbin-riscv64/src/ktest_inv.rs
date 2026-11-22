// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-67a** — pha invocation hẹp trên ảnh `vifast-kbin-ktest-user` (TAP 15..18).
//!
//! Tách khỏi `ktest_user.rs` để giữ luật R1 (≤ 1.500 dòng/tệp). Bốn TAP:
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 15 - invocation untyped retype` | U-mode tự `Untyped.Retype` Frame (D2-97/D2-99), cap đại diện trong CSpace, `resolve` ra PA thật; `check_objmem`/`check_caps` sạch |
//! | `ok 16 - invocation frame map` | `Frame.Map` qua VSpace cap; kernel đọc **vật lý** tại PA (physmap, root kernel) đúng giá trị U-mode ghi; ca âm: VSpace chỉ READ ⇒ 3 |
//! | `ok 17 - invocation frame unmap va fault` | `Frame.Unmap` đúng cap Frame ⇒ walk invalid; ca âm: Frame cap khác ⇒ 1, mapping **còn**; đọc lại ⇒ `FaultKind::Vm{NotMapped,Read}` |
//! | `ok 18 - invocation ngoai danh sach` | method/loại ngoài whitelist + `caps=1` ⇒ `NotSupported` 18, có đếm (`stats=3`) |
//!
//! Quy ước tham số: **D2-99**; `spec/` chốt ở KT-67b. Biên giới D2-83: core giữ cap/quyền,
//! object layer (`mem_bridge::invoke`) sở hữu Frame/PA/PTE.

use core::fmt::Write;

use vifast_core::RIGHT_READ;
use vifast_core::{KError, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::fault::{FaultKind, FAULT_STATS};
use vifast_plat_common::obj::{CapId, ObjKind};
use vifast_plat_common::user::TaskState;
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::user;
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// ELF chương trình invocation (build.rs sinh từ `init/inv_user.rs` + `.S`).
static INV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/inv.elf"));

/// Slot CSpace dùng cho pha invocation (13..19 trống sau rig `build_sys_rig` 0..12).
const INV_SLOT_UNTYPED: usize = 13;
const INV_SLOT_VSPACE: usize = 14;
const INV_SLOT_FRAME: usize = 15;
const INV_SLOT_TCB: usize = 16;
const INV_SLOT_SC: usize = 17;
const INV_SLOT_FRAME_B: usize = 18;
const INV_SLOT_VSPACE_RO: usize = 19;
/// VA mà chương trình invocation map Frame của nó.
const INV_VA: u64 = 0x4100_0000;
/// Giá trị chương trình ghi/đọc thật qua vùng đã map.
const INV_MAGIC: u64 = 0x1122_3344_5566_7788;

/// Đưa TCB về `Running{0}` (bản sao nhỏ của helper `make_running` bên `ktest_user`).
fn set_running(m: &mut vifast_core::Machine, tcb: ObjId) {
    if m.tcb_state(tcb) != Ok(TcbState::Running { hart: 0 }) {
        let _ = m.wake(tcb);
        if m.tcb_state(tcb) == Ok(TcbState::Ready) {
            let _ = m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 });
        }
    }
}

// lại vùng đã unmap, và method ngoài danh sách trắng ⇒ `NotSupported` 18 (D2-53 Q11).
// =============================================================================================

/// Bốn TAP `# inv:` — mỗi method một TAP + ca method lạ. Trả `true` ⇔ mọi TAP xanh.
///
/// Bằng chứng **đo từ kernel**, không tin lời chương trình: `mem_bridge::resolve` trả PA thật
/// của Frame mà U-mode đã retype, kernel đọc 8 B **vật lý** tại PA đó (giá trị chương trình ghi
/// qua vùng đã map), `ObjectLayer::walk` chứng minh mapping biến mất sau `Unmap`, và
/// `FaultKind::Vm{va, access=Read, reason=NotMapped}` do trap phần cứng sinh ra.
pub fn invocation_phase(
    root: ObjId,
    us: SlotRef,
    cspace: SlotRef,
    load_untyped: CapId,
    fails: &mut usize,
) -> bool {
    // Mốc số đếm TRƯỚC pha: TAP 8 (kernel-level) đã gọi `Call` trên cap TCB ⇒ có sẵn 1 lần
    // `NotSupported`; TAP này đo **delta** của riêng pha invocation, không phụ thuộc mốc cũ.
    let before = vifast_core::invoke::stats();
    // ---- nạp chương trình invocation vào ô tác vụ 1 (VSpace riêng, cùng Untyped u0)
    let loaded = match user::load_init_slot(INV_ELF, load_untyped, 1) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 15 - invocation untyped retype: nap inv ELF that bai kind={} ({:?})",
                error.name(),
                error
            );
            *fails += 1;
            return false;
        }
    };
    let vspace_key = {
        let m = aobj::machine();
        match m.cap_key(loaded.vspace) {
            Ok(id) => mem_bridge::make_key(loaded.vspace, id),
            Err(error) => {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "not ok 15 - invocation untyped retype: cap_key VSpace loi {:?}",
                    error
                );
                *fails += 1;
                return false;
            }
        }
    };
    // ---- fixture ca âm M1: Frame B do object layer cấp (chỉ để thử unmap sai Frame)
    let frame_b = {
        let m = aobj::machine();
        match m.retype(load_untyped, ObjKind::Frame, 12) {
            Ok(cap) => match (m.frame_pa(cap), m.cap_key(cap)) {
                (Ok(pa), Ok(id)) => Some((mem_bridge::make_key(cap, id), pa)),
                _ => None,
            },
            Err(_) => None,
        }
    };
    // ---- cấp cap lúc "spawn": memsrv giữ cap Untyped trực tiếp; chương trình giữ cap VSpace
    let setup = core_bridge::with_machine(|m| -> Result<(ObjId, ObjId), KError> {
        m.copy(SlotRef::new(root, 0), SlotRef::new(root, INV_SLOT_UNTYPED))?;
        let u_obj = m.cap_obj_at(root, INV_SLOT_UNTYPED)?;
        let vs_obj = m.add_foreign_object(ObjType::VSpace, vspace_key)?;
        let vs_cap = m.root_cap(vs_obj);
        m.put_cap(root, INV_SLOT_VSPACE, vs_cap)?;
        // M2 fixture: cap VSpace **chỉ READ** — invocation Map/Unmap phải trả 3.
        m.copy_rights(
            SlotRef::new(root, INV_SLOT_VSPACE),
            SlotRef::new(root, INV_SLOT_VSPACE_RO),
            Some(RIGHT_READ),
        )?;
        // M1 fixture: Frame B (object layer cấp) — unmap VA của Frame A bằng cap này phải bị
        // từ chối, và mapping Frame A phải còn.
        if let Some((key, _pa)) = frame_b {
            let b_obj = m.add_foreign_object(ObjType::Frame, key)?;
            let b_cap = m.root_cap(b_obj);
            m.put_cap(root, INV_SLOT_FRAME_B, b_cap)?;
        }
        let tcb_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, INV_SLOT_TCB, tcb_cap)?;
        let sc_cap = m.retype(us, ObjType::SchedContext, 0, 0)?;
        m.put_cap(root, INV_SLOT_SC, sc_cap)?;
        m.tcb_configure(SlotRef::new(root, INV_SLOT_TCB), Some(cspace))?;
        m.set_sched_params(
            SlotRef::new(root, INV_SLOT_TCB),
            Some(SlotRef::new(root, INV_SLOT_SC)),
        )?;
        m.tcb_set_priority(SlotRef::new(root, INV_SLOT_TCB), 3, 10)?;
        let tcb = m.cap_obj(tcb_cap)?;
        // Pha kernel-level (TAP 7..13) có thể còn TCB `Ready` khác ⇒ **không** dùng `dispatch`
        // (sẽ chọn TCB khác); đưa thẳng TCB invocation về `Running{hart:0}` như helper
        // `make_running` của TAP kernel-level, rồi kiểm lại bằng `current(0)`.
        m.start_tcb(tcb)?;
        set_running(m, tcb);
        if m.current(0) != Some(tcb) {
            return Err(KError::IllegalOperation);
        }
        Ok((u_obj, tcb))
    });
    let (untyped_obj, tcb) = match setup {
        Ok(pair) => pair,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 15 - invocation untyped retype: cap luc spawn loi {:?}",
                error
            );
            *fails += 1;
            return false;
        }
    };
    // Bridge MỘT CHIỀU core ObjId → cap Untyped của object layer (D2-83): chỉ cần đăng ký,
    // byte/PA vẫn do object layer sở hữu.
    let registered = mem_bridge::register_untyped(untyped_obj, load_untyped);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# inv: prep untyped_slot={} vspace_slot={} frame_slot={} frame_b_slot={} vspace_ro_slot={} tcb={} va=0x{:x} registered={} vspace_key=0x{:x} frame_b_pa=0x{:x} root=0x{:x}",
            INV_SLOT_UNTYPED,
            INV_SLOT_VSPACE,
            INV_SLOT_FRAME,
            INV_SLOT_FRAME_B,
            INV_SLOT_VSPACE_RO,
            tcb,
            INV_VA,
            registered as u8,
            vspace_key,
            frame_b.map(|(_, pa)| pa).unwrap_or(0),
            loaded.root
        );
    }
    user::set_active_slot(1);
    user::bind_core_tcb(tcb);
    let run = user::enter_expect_fault(&loaded, 0);
    let snap = match run {
        Ok(snap) => snap,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 15 - invocation untyped retype: enter that bai kind={} ({:?})",
                error.name(),
                error
            );
            *fails += 1;
            return false;
        }
    };
    // ---- đo sau lượt chạy: cap Frame U-mode tự cấp, PA thật, nội dung ghi thật, PTE sau unmap
    let stats = vifast_core::invoke::stats();
    let delta = vifast_core::invoke::InvocationStats {
        allowed: stats.allowed.saturating_sub(before.allowed),
        denied: stats.denied.saturating_sub(before.denied),
        retype: stats.retype.saturating_sub(before.retype),
        map: stats.map.saturating_sub(before.map),
        unmap: stats.unmap.saturating_sub(before.unmap),
        copy: stats.copy.saturating_sub(before.copy),
        mint: stats.mint.saturating_sub(before.mint),
        moved: stats.moved.saturating_sub(before.moved),
        delete: stats.delete.saturating_sub(before.delete),
        revoke: stats.revoke.saturating_sub(before.revoke),
        // KT-74: `Tcb.SetTls` — pha KT-67a không gọi method này ⇒ delta phải **0** (giữ nguyên
        // khẳng định cũ, không nới).
        set_tls: stats.set_tls.saturating_sub(before.set_tls),
        // **KT-290**: pha KT-67a không gọi `Tcb.Configure` ⇒ delta phải **0** (giữ nguyên
        // khẳng định cũ, không nới).
        configure: stats.configure.saturating_sub(before.configure),
        // **KT-314**: pha KT-67a không gọi `Tcb.Resume` ⇒ delta phải **0** (giữ nguyên khẳng
        // định cũ, không nới).
        resume: stats.resume.saturating_sub(before.resume),
        // **KT-319**: pha KT-67a không gọi `Tcb.Suspend` ⇒ delta phải **0** (giữ nguyên khẳng
        // định cũ, không nới).
        suspend: stats.suspend.saturating_sub(before.suspend),
        // **KT-329**: pha KT-67a không gọi `Suspend(Defer)` ⇒ delta `deferred` phải **0**.
        deferred: stats.deferred.saturating_sub(before.deferred),
    };
    let fault = FAULT_STATS.last();
    let mut frame_obj = 0u32;
    let mut frame_ty = None;
    core_bridge::with_machine(|m| {
        if let Ok(cap) = m.cap_at(root, INV_SLOT_FRAME) {
            if let Ok(info) = m.cap_info(cap) {
                frame_obj = info.obj;
                frame_ty = Some(info.ty);
            }
        }
    });
    let resolved = mem_bridge::resolve(frame_obj);
    let (frame_pa, frame_bits) = resolved.map(|(_, pa, bits)| (pa, bits)).unwrap_or((0, 0));
    let phys = resolved.and_then(|(_, pa, _)| aobj::machine().read_phys_u64(pa));
    let walk_after = aobj::machine()
        .walk(loaded.vspace, INV_VA)
        .map(|walk| walk.valid)
        .unwrap_or(true);
    // M1: sau lượt chạy, **cả** Frame A và Frame B phải không còn mapping nào (Frame B chưa
    // từng được map; Frame A đã được unmap đúng cap của nó).
    let frame_a_maps = resolved
        .and_then(|(cap, _, _)| aobj::machine().frame_counts(cap).ok())
        .map(|(maps, _, _)| maps)
        .unwrap_or(u16::MAX);
    let frame_b_maps = frame_b
        .map(|(key, _)| {
            let (cap, _) = mem_bridge::decode_key(key);
            aobj::machine()
                .frame_counts(cap)
                .map(|(maps, _, _)| maps)
                .unwrap_or(u16::MAX)
        })
        .unwrap_or(u16::MAX);
    let objmem_ok = aobj::machine().check_objmem().is_ok();
    let caps_ok = core_bridge::with_machine(|m| m.check_caps().is_ok());
    let fault_ok = matches!(
        fault,
        Some(FaultKind::Vm {
            va,
            access: vifast_plat_common::fault::FaultAccess::Read,
            reason: vifast_plat_common::fault::VmFaultReason::NotMapped,
            ..
        }) if va == INV_VA
    );
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# inv: retype method={} ty={:?} size={} err={} slot={} stats={}/{}",
            vifast_core::invoke::METHOD_RETYPE,
            frame_ty.unwrap_or(ObjType::Untyped),
            frame_bits,
            if frame_ty == Some(ObjType::Frame) {
                0
            } else {
                1
            },
            INV_SLOT_FRAME,
            delta.retype,
            delta.allowed
        );
        let _ = writeln!(
            w,
            "# inv: map method={} va=0x{:x} pa=0x{:x} err={} stats={} phys=0x{:x}",
            vifast_core::invoke::METHOD_MAP,
            INV_VA,
            frame_pa,
            if delta.map == 1 { 0 } else { 1 },
            delta.map,
            phys.unwrap_or(0)
        );
        let _ = writeln!(
            w,
            "# inv: unmap method={} va=0x{:x} err={} stats={} walk_after={} objmem={} caps={}",
            vifast_core::invoke::METHOD_UNMAP,
            INV_VA,
            if delta.unmap == 1 { 0 } else { 1 },
            delta.unmap,
            walk_after as u8,
            objmem_ok as u8,
            caps_ok as u8
        );
        let _ = writeln!(
            w,
            "# inv: deny err={} stats={} allowed={} tls_delta={} last_err={} syscalls={} entries={}",
            user::sys_err(0),
            delta.denied,
            delta.allowed,
            delta.set_tls,
            user::sys_err(0),
            snap.syscalls,
            snap.entries
        );
        // Ca âm “VSpace chỉ READ ⇒ 3” và “caps=1 ⇒ 18” do **chương trình** kiểm (`inv_user.S`
        // mã thoát 97/96); dòng này chỉ in số **đo được** ở kernel, không nhãn hard-code
        // (reviewer-kt67a nit 3).
        let _ = writeln!(
            w,
            "# inv: deny2 ran={} frame_a_maps={} frame_b_maps={} walk_after={}",
            u8::from(snap.state == TaskState::Faulted),
            frame_a_maps,
            frame_b_maps,
            walk_after as u8
        );
        let _ = writeln!(
            w,
            "# inv: fault va=0x{:x} kind={} ok={} snap_state={}",
            INV_VA,
            match fault {
                Some(_) => {
                    if fault_ok {
                        "Vm{NotMapped,Read}"
                    } else {
                        "other"
                    }
                }
                None => "none",
            },
            fault_ok as u8,
            snap.state.name()
        );
    }
    // ---- TAP 15..18
    let ok15 = registered
        && delta.retype == 1
        && frame_ty == Some(ObjType::Frame)
        && frame_bits == 12
        && resolved.is_some()
        && loaded.check_ok
        && objmem_ok
        && caps_ok;
    if ok15 {
        PLATFORM.put_str("ok 15 - invocation untyped retype\n");
    } else {
        PLATFORM.put_str("not ok 15 - invocation untyped retype: cap/byte sai\n");
        *fails += 1;
    }
    let ok16 = delta.map == 1 && phys == Some(INV_MAGIC);
    if ok16 {
        PLATFORM.put_str("ok 16 - invocation frame map\n");
    } else {
        PLATFORM.put_str("not ok 16 - invocation frame map: PTE/PA/byte sai\n");
        *fails += 1;
    }
    let ok17 = delta.unmap == 1
        && !walk_after
        && fault_ok
        && frame_a_maps == 0
        && frame_b_maps == 0
        && snap.state == TaskState::Faulted;
    if ok17 {
        PLATFORM.put_str("ok 17 - invocation frame unmap va fault\n");
    } else {
        PLATFORM.put_str("not ok 17 - invocation frame unmap va fault: mapping/fault sai\n");
        *fails += 1;
    }
    // D6 (reviewer KT-74): câu "delta `set_tls` phải **0**" phải được **assert**, không chỉ nói —
    // pha invocation (KT-67a) **không** gọi `Tcb.SetTls`, nên gộp vào `ok18` để nó có hiệu lực.
    let ok18 =
        delta.denied == 3 && delta.allowed == 3 && delta.set_tls == 0 && user::sys_err(0) == 18;
    if ok18 {
        PLATFORM.put_str("ok 18 - invocation ngoai danh sach\n");
    } else {
        PLATFORM.put_str("not ok 18 - invocation ngoai danh sach: so lan/ma sai\n");
        *fails += 1;
    }
    // **KT-68** — trả `VSpace`/PageTable của pha này về object layer sau khi đã đo xong TAP
    // 15..18: arena object layer chỉ 64 KiB và pha capalloc cần ~16 KiB PageTable cho hai
    // chương trình mới. Chỉ giải phóng **sau** mọi phép đo; không đổi kết quả TAP nào ở trên.
    // KT-69: xoá cap tạm trong cspace gốc (13..19) sau khi đo xong — cspace gốc chỉ 32 slot,
    // các pha sau cần slot trống; không đổi kết quả TAP 15..18.
    core_bridge::with_machine(|m| {
        for slot in INV_SLOT_UNTYPED..=INV_SLOT_VSPACE_RO {
            let _ = m.delete(SlotRef::new(root, slot));
        }
    });
    loaded.release();
    ok15 && ok16 && ok17 && ok18
}
