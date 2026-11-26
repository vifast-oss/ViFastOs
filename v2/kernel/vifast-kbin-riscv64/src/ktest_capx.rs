// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-67b** — TAP 19: **chuyển cap Frame giữa HAI tiến trình U-mode** (D2-98).
//!
//! Hai **ELF riêng**, hai `load_init_slot`, hai VSpace, hai TCB, **hai CNode/cspace riêng**:
//!
//! * `memsrv.elf` (TCB A) giữ cap `Untyped` **trong cspace của A**, tự `Untyped.Retype` ra
//!   `Frame` (slot 5), rồi `NBSend` sang Endpoint **mang cap** (`MessageInfo.caps = 1` + CPTR
//!   nguồn trong **header IPC buffer** — spec `abi.toml` dòng 58);
//! * `client.elf` (TCB B) **không** có cap Frame nào lúc spawn. `Recv` xong, kernel cài cap con
//!   vào **receive window trong cspace B** (4 CPTR do chính B ghi vào header); B dùng **cap nhận
//!   được** để `Frame.Map` vào VSpace B, ghi/đọc 8 B thật, `Frame.Unmap`, rồi map lại + ghi giá
//!   trị thứ hai.
//!
//! Bằng chứng kernel đo (không tin lời chương trình):
//! 1. **trước** `Recv`: `cap_at(cspace B, 4) = SlotEmpty`, `walk(vspace B, VA)` invalid;
//! 2. **sau**: cap Frame trong cspace B (đúng `obj` của Frame A, `rights = RIGHTS_ALL & !GRANT`),
//!    `walk(vspace B, VA)` valid + PA thật, kernel đọc **physmap** tại PA = giá trị B ghi;
//! 3. `cap_transfer_installed` delta = **1**, `cap_transfer_failed` delta = **1** (cap thiếu
//!    `RIGHT_GRANT` bị bỏ **mềm** — message vẫn giao, 0 cap; D2-53 Q6), message `caps = 0`
//!    không cài cap nào (đối chứng);
//! 4. ca âm: `caps = 1` + CPTR nguồn rỗng ⇒ `InvalidCapability` **1** (kernel đọc CPTR từ header).
//!
//! Policy chuyển cap của ABI là **copy** (thiết kế §4.6/KT-63), không phải move: TAP đo thêm
//! `src_alive=1` (memsrv **vẫn** giữ cap nguồn) để ghi rõ policy — muốn “chuyển hết” phải có
//! `CNode.Delete` (method 4, ngoài danh sách trắng invocation — nợ §5 của `k6-memsrv.md`).

use core::fmt::Write;

use vifast_core::vm::{CapId, ObjKind};
use vifast_core::{
    KError, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL, RIGHT_GRANT,
};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, Stop};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Hai chương trình U-mode **riêng** do `build.rs` sinh (`init/memsrv_user.rs`,
/// `init/client_user.rs`) — hai tiến trình, không phải hai vai của một ảnh.
static MEMSRV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/memsrv.elf"));
static CLIENT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/client.elf"));

/// CSpace **của memsrv** (kernel dựng lúc spawn).
const A_EP: usize = 1;
const A_VSPACE: usize = 2;
const A_UNTYPED: usize = 3;
const A_FRAME: usize = 5;
const A_FRAME_NO_GRANT: usize = 6;
/// CSpace **của client** (kernel dựng lúc spawn) — `B_RECV` = ô đầu receive window.
const B_EP: usize = 1;
const B_VSPACE: usize = 2;
const B_RECV: usize = 4;
/// Slot tạm của rig trong cspace gốc (13..19 đã dùng ở pha KT-67a).
const TMP_FRAME: usize = 20;
const TMP_TCB_A: usize = 21;
const TMP_SC_A: usize = 22;
const TMP_TCB_B: usize = 23;
const TMP_SC_B: usize = 24;

/// VA client map Frame (VSpace của **client**).
const VA_MAP: u64 = 0x4100_0000;
/// Giá trị client ghi lần hai (kernel đọc physmap phải thấy đúng).
const MAGIC2: u64 = 0x8877_6655_4433_2211;
/// Nửa byte thấp của quyền: `R|W|G` = quyền của cap nhận (bỏ `RIGHT_GRANT` khỏi `RIGHTS_ALL`).
const RECV_RIGHTS: u8 = RIGHTS_ALL & !RIGHT_GRANT;

/// Rig của pha capx — mọi thứ thuộc kernel, không phải chương trình.
struct Rig {
    tcb_a: ObjId,
    tcb_b: ObjId,
    cnode_a: ObjId,
    cnode_b: ObjId,
}

/// Đưa TCB từ `Inactive` về `Running{hart:0}` — cùng khuôn `ktest_inv::set_running`.
fn set_running(m: &mut vifast_core::Machine, tcb: ObjId) -> Result<(), KError> {
    if !matches!(m.tcb_state(tcb)?, TcbState::Running { .. }) {
        if m.tcb_state(tcb)? == TcbState::Inactive {
            m.start_tcb(tcb)?;
        }
        m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })?;
    }
    Ok(())
}

/// Chạy một chương trình U-mode tới `Exit`/`Yield` và trả `Stop`.
fn run_elf(loaded: &user::LoadedInit, tcb: ObjId, slot: usize) -> Option<Stop> {
    user::set_active_slot(slot);
    user::bind_core_tcb(tcb);
    let entered = core_bridge::with_machine(|m| set_running(m, tcb).is_ok());
    if !entered {
        return None;
    }
    let mut tf = TrapFrame::zeroed();
    tf.x[REG_SP] = user::USER_STACK_TOP;
    tf.sepc = loaded.entry;
    Some(user::run_task(loaded, &mut tf))
}

/// TAP 19 — chuyển cap Frame giữa hai tiến trình U-mode + client map/ghi/đọc thật.
///
/// Trả `true` ⇔ mọi bất biến xanh (TAP đã in bởi hàm này).
pub fn capx_phase(
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    fails: &mut usize,
) -> bool {
    let fail = |fails: &mut usize, reason: &str| {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(w, "not ok 19 - ipc capx hai tien trinh: {reason}");
        *fails += 1;
    };
    // ---- nạp HAI chương trình riêng vào hai VSpace (cùng Untyped của ảnh test).
    let memsrv = match user::load_init_slot(MEMSRV_ELF, load_untyped, 1) {
        Ok(loaded) => loaded,
        Err(_) => {
            fail(fails, "nap memsrv that bai");
            return false;
        }
    };
    let client = match user::load_init_slot(CLIENT_ELF, load_untyped, 2) {
        Ok(loaded) => loaded,
        Err(_) => {
            fail(fails, "nap client that bai");
            return false;
        }
    };
    // ---- key object layer của hai VSpace + Frame "thiếu GRANT" (object layer sở hữu — D2-83).
    let keys = {
        let m = aobj::machine();
        let no_grant = m.retype(load_untyped, ObjKind::Frame, 12).ok();
        let key_of = |cap: CapId| m.cap_key(cap).ok().map(|id| mem_bridge::make_key(cap, id));
        match (
            key_of(memsrv.vspace),
            key_of(client.vspace),
            no_grant.and_then(key_of),
        ) {
            (Some(a), Some(b), Some(no_grant_key)) => Some((a, b, no_grant_key)),
            _ => None,
        }
    };
    let Some((vspace_key_a, vspace_key_b, no_grant_key)) = keys else {
        fail(fails, "key object layer sai");
        return false;
    };
    let rig = core_bridge::with_machine(|m| -> Result<Rig, KError> {
        let cnode_a = m.add_cnode(16);
        let cnode_b = m.add_cnode(16);
        let self_a = m.root_cap(cnode_a);
        m.put_cap(cnode_a, 0, self_a)?;
        let self_b = m.root_cap(cnode_b);
        m.put_cap(cnode_b, 0, self_b)?;
        // memsrv giữ cap Untyped **trong cspace của A** để tự Retype (D2-97/D2-99).
        m.copy(SlotRef::new(root, 0), SlotRef::new(cnode_a, A_UNTYPED))?;
        // Endpoint dùng chung: cap ở cspace A + cap ở cspace B.
        let ep_cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
        m.put_cap(cnode_a, A_EP, ep_cap)?;
        m.copy(SlotRef::new(cnode_a, A_EP), SlotRef::new(cnode_b, B_EP))?;
        // Cap VSpace **của từng tiến trình** nằm trong cspace của chính nó.
        let vsa = m.add_foreign_object(ObjType::VSpace, vspace_key_a)?;
        let vsa_cap = m.root_cap(vsa);
        m.put_cap(cnode_a, A_VSPACE, vsa_cap)?;
        let vsb = m.add_foreign_object(ObjType::VSpace, vspace_key_b)?;
        let vsb_cap = m.root_cap(vsb);
        m.put_cap(cnode_b, B_VSPACE, vsb_cap)?;
        // Frame **thiếu `RIGHT_GRANT`** trong cspace A (đối chứng chuyển mềm, D2-53 Q6).
        let frame_obj = m.add_foreign_object(ObjType::Frame, no_grant_key)?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, TMP_FRAME, frame_cap)?;
        m.copy_rights(
            SlotRef::new(root, TMP_FRAME),
            SlotRef::new(cnode_a, A_FRAME_NO_GRANT),
            Some(RECV_RIGHTS),
        )?;
        m.delete(SlotRef::new(root, TMP_FRAME))?;
        // TCB + SchedContext cho hai tiến trình, cspace riêng từng bên.
        let tcb_a_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        let tcb_a = m.cap_obj(tcb_a_cap)?;
        m.put_cap(root, TMP_TCB_A, tcb_a_cap)?;
        let sc_a = m.retype(us, ObjType::SchedContext, 0, 0)?;
        m.put_cap(root, TMP_SC_A, sc_a)?;
        m.tcb_configure(
            SlotRef::new(root, TMP_TCB_A),
            Some(SlotRef::new(cnode_a, 0)),
        )?;
        m.sched_control_configure(ctl, SlotRef::new(root, TMP_SC_A), 500, 5_000)?;
        m.set_sched_params(
            SlotRef::new(root, TMP_TCB_A),
            Some(SlotRef::new(root, TMP_SC_A)),
        )?;
        m.tcb_set_priority(SlotRef::new(root, TMP_TCB_A), 3, 10)?;
        let tcb_b_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        let tcb_b = m.cap_obj(tcb_b_cap)?;
        m.put_cap(root, TMP_TCB_B, tcb_b_cap)?;
        let sc_b = m.retype(us, ObjType::SchedContext, 0, 0)?;
        m.put_cap(root, TMP_SC_B, sc_b)?;
        m.tcb_configure(
            SlotRef::new(root, TMP_TCB_B),
            Some(SlotRef::new(cnode_b, 0)),
        )?;
        m.sched_control_configure(ctl, SlotRef::new(root, TMP_SC_B), 500, 5_000)?;
        m.set_sched_params(
            SlotRef::new(root, TMP_TCB_B),
            Some(SlotRef::new(root, TMP_SC_B)),
        )?;
        m.tcb_set_priority(SlotRef::new(root, TMP_TCB_B), 3, 10)?;
        Ok(Rig {
            tcb_a,
            tcb_b,
            cnode_a,
            cnode_b,
        })
    });
    let rig = match rig {
        Ok(rig) => rig,
        Err(_) => {
            fail(fails, "dung rig core that bai");
            return false;
        }
    };
    // Mốc bộ đếm **trước** pha (TAP không phụ thuộc các pha trước).
    let (installed_before, failed_before) =
        core_bridge::with_machine(|m| (m.cap_transfer_installed(), m.cap_transfer_failed()));
    // Cho phép `Exit`/`Yield` dừng lượt U-mode (không PANIC) — chỉ trong pha này.
    user::enable_multitask(true);
    // ---- A: memsrv tự Retype + gửi cap; kết thúc `Exit(0)`.
    let stop_a = run_elf(&memsrv, rig.tcb_a, 1);
    // memsrv kết thúc bằng `Yield` (không `Exit`: Exit huỷ message đang chờ — D2-25); đưa TCB A
    // về `Ready` để hart 0 không còn `Running` nào trước khi chạy B.
    let a_ready = matches!(stop_a, Some(Stop::Yield))
        && core_bridge::with_machine(|m| m.tcb_yield(rig.tcb_a).is_ok());
    if !a_ready {
        fail(fails, "memsrv khong Yield");
        user::enable_multitask(false);
        return false;
    }
    // ---- **Trước** Recv của client: cspace B **chưa** có cap Frame, VSpace B chưa map VA.
    let (pre_b4_empty, pre_b5_empty, src_after_send) = core_bridge::with_machine(|m| {
        let b4 = m.cap_at(rig.cnode_b, B_RECV);
        let b5 = m.cap_at(rig.cnode_b, B_RECV + 1);
        let src = m
            .cap_at(rig.cnode_a, A_FRAME)
            .ok()
            .and_then(|cap| m.cap_info(cap).ok());
        (
            b4 == Err(KError::SlotEmpty),
            b5 == Err(KError::SlotEmpty),
            src.map(|info| (info.obj, info.ty, info.alive)),
        )
    });
    let pre_walk = aobj::machine()
        .walk(client.vspace, VA_MAP)
        .map(|walk| walk.valid)
        .unwrap_or(true);
    // ---- B: client Recv (kernel cài cap vào window **của B**) → Frame.Map → ghi/đọc.
    let stop_b = run_elf(&client, rig.tcb_b, 2);
    user::enable_multitask(false);
    if !matches!(stop_b, Some(Stop::Exit { code: 0 })) {
        fail(fails, "client khong Exit(0)");
        return false;
    }
    // ---- đo **sau**: cap trong cspace B, Frame A, quyền, PA qua physmap, PTE của VSpace B.
    let (b4, src5, installed, failed) = core_bridge::with_machine(|m| {
        let b4 = m
            .cap_at(rig.cnode_b, B_RECV)
            .ok()
            .and_then(|cap| m.cap_info(cap).ok())
            .map(|info| (info.obj, info.ty, info.rights, info.alive));
        let src5 = m
            .cap_at(rig.cnode_a, A_FRAME)
            .ok()
            .and_then(|cap| m.cap_info(cap).ok())
            .map(|info| (info.obj, info.rights, info.alive));
        (
            b4,
            src5,
            m.cap_transfer_installed().saturating_sub(installed_before),
            m.cap_transfer_failed().saturating_sub(failed_before),
        )
    });
    let (frame_obj, b4_rights, b4_alive) = match b4 {
        Some((obj, ObjType::Frame, rights, alive)) => (obj, rights, alive),
        _ => (0, 0, false),
    };
    let (src_obj, src_rights, src_alive) = src5.unwrap_or((0, 0, false));
    let resolved = mem_bridge::resolve(frame_obj);
    let (frame_pa, frame_bits) = resolved.map(|(_, pa, bits)| (pa, bits)).unwrap_or((0, 0));
    let phys = resolved.and_then(|(_, pa, _)| aobj::machine().read_phys_u64(pa));
    let walk_after = aobj::machine().walk(client.vspace, VA_MAP).ok();
    let (walk_valid, walk_pa) = walk_after
        .map(|walk| (walk.valid && walk.leaf && walk.level == 0, walk.phys))
        .unwrap_or((false, 0));
    let caps_ok = core_bridge::with_machine(|m| m.check_caps().is_ok());
    let ipc_ok = core_bridge::with_machine(|m| m.check_ipc().is_ok());
    let objmem_ok = aobj::machine().check_objmem().is_ok();
    // Dọn mapping của client (giữ cap để `check_caps` vẫn kiểm được bất biến của cấp cap).
    let _ = aobj::machine().unmap(client.vspace, VA_MAP);
    {
        // F3 (reviewer): in **giá trị thật** của phép đo trước, không dùng chuỗi literal —
        // cùng chuẩn `# mem:`/`# inv:` của các pha trước. Policy in kèm vì là **ngữ nghĩa**
        // (KT-63 copy), không phải số đo.
        let mut w = PlatformWriter(&PLATFORM);
        let cspace_tag = |empty: bool| if empty { "empty" } else { "filled" };
        let (src_ty, src_is_alive) = src_after_send
            .map(|(_, ty, alive)| (ty, alive))
            .unwrap_or((ObjType::Untyped, false));
        let _ = writeln!(
            w,
            "# capx: pre cspace_b4={} cspace_b5={} walk={} a5={} src_alive={} policy=copy",
            cspace_tag(pre_b4_empty),
            cspace_tag(pre_b5_empty),
            u8::from(pre_walk),
            if src_ty == ObjType::Frame {
                "Frame"
            } else {
                "?"
            },
            u8::from(src_is_alive)
        );
        let _ = writeln!(
            w,
            "# capx: recv slot={} ty={:?} rights=0x{:x} obj={} from_obj={} from_rights=0x{:x} installed={} failed={}",
            B_RECV,
            if frame_obj == 0 {
                ObjType::Untyped
            } else {
                ObjType::Frame
            },
            b4_rights,
            frame_obj,
            src_obj,
            src_rights,
            installed,
            failed
        );
        let _ = writeln!(
            w,
            "# capx: map va=0x{:x} pa=0x{:x} bits={} walk={} walk_pa=0x{:x} phys=0x{:x} caps={} ipc={} obj={} src_alive={}",
            VA_MAP,
            frame_pa,
            frame_bits,
            u8::from(walk_valid),
            walk_pa,
            phys.unwrap_or(0),
            caps_ok as u8,
            ipc_ok as u8,
            objmem_ok as u8,
            src_alive as u8
        );
    }
    let ok = pre_b4_empty
        && pre_b5_empty
        && !pre_walk
        && src_after_send
            .map(|(obj, ty, alive)| ty == ObjType::Frame && alive && obj != 0)
            .unwrap_or(false)
        && frame_obj != 0
        && b4_rights == RECV_RIGHTS
        && b4_alive
        && src_obj == frame_obj
        && src_rights == RIGHTS_ALL
        && src_alive
        && installed == 1
        && failed == 1
        && resolved.is_some()
        && frame_bits == 12
        && phys == Some(MAGIC2)
        && walk_valid
        && walk_pa == frame_pa
        && caps_ok
        && ipc_ok
        && objmem_ok;
    // **KT-68** — trả `VSpace`/PageTable của **hai** chương trình về object layer sau khi đã đo
    // xong TAP 19 (arena 64 KiB; pha capalloc cần PageTable cho hai chương trình khác).
    // KT-69: xoá cap tạm trong cspace gốc (20..24) — cspace gốc chỉ 32 slot.
    core_bridge::with_machine(|m| {
        for slot in TMP_FRAME..=TMP_SC_B {
            let _ = m.delete(SlotRef::new(root, slot));
        }
    });
    memsrv.release();
    client.release();
    if ok {
        PLATFORM.put_str("ok 19 - ipc capx hai tien trinh\n");
    } else {
        fail(fails, "cap/PA/walk/pte sai");
    }
    ok
}
