// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-345b** — host test cho **vòng đời TCB spawn→destroy** mà lệnh `loop` dựa vào.
//!
//! `LoopSvc` (`loop_svc.rs`) mỗi vòng: `Retype(Tcb)` + `Retype(SchedContext)` từ **một Untyped
//! 4 KiB riêng**, `Configure`/`Start`, rồi `Delete` hai cap + `Revoke` Untyped đó; `ok` chỉ được
//! tính khi `Machine::total_used()` **trước = sau**. Nếu `Revoke` không finalise/không trả byte
//! (ví dụ bỏ bước `reset` arena hoặc bỏ `finalise_tcb`), `loop` sẽ báo `LOOP BAD`/rò rỉ.
//!
//! Ca này khoá đúng bất biến đó ở mức core (chạy host, không QEMU):
//!
//! 1. Untyped **con** 4 KiB cấp từ Untyped gốc, TCB + SC cấp từ con ⇒ `total_used()` tăng;
//! 2. `Delete` cap + `Revoke` con ⇒ byte về 0, TCB/SC bị finalise, `total_used()` **bằng** ban đầu;
//! 3. lặp lại lần hai ⇒ vẫn bằng (không tích luỹ giữa các vòng — đúng `loop 6`).
//!
//! Không sửa `vifast-core` cho test này; chỉ dùng API công khai.
#![cfg(test)]

extern crate std;

use crate::error::{ObjId, ObjType, SlotRef};
use crate::machine::Machine;

/// Dựng cảnh tối thiểu: CNode 16 ô + Untyped gốc 64 KiB (như `core_bridge::CORE_ARENA_BYTES`)
/// + một `SchedControl` (ô 7) + **một** arena con 4 KiB (ô 8) — đúng `loop_svc::build` (arena
/// dựng **một lần** lúc boot, không dựng lại mỗi vòng).
fn rig() -> (Machine, ObjId) {
    let mut m = Machine::new();
    let u = m.add_untyped(0x8200_0000, 64 * 1024);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 1, u_cap).expect("cap untyped");
    let ctl = m
        .retype(SlotRef::new(c, 1), ObjType::SchedControl, 0, 0)
        .expect("retype SchedControl");
    m.put_cap(c, 7, ctl).expect("put ctl");
    let arena_cap = m
        .retype(SlotRef::new(c, 1), ObjType::Untyped, 12, 0)
        .expect("retype arena 4 KiB");
    m.put_cap(c, 8, arena_cap).expect("put arena");
    (m, c)
}

/// Một vòng spawn→destroy **trong** arena có sẵn (ô 8); trả `total_used()` sau vòng.
fn round(m: &mut Machine, c: ObjId) -> usize {
    let arena = SlotRef::new(c, 8);
    let tcb_cap = m.retype(arena, ObjType::Tcb, 0, 0).expect("retype Tcb");
    m.put_cap(c, 9, tcb_cap).expect("put Tcb");
    let tcb = SlotRef::new(c, 9);
    let sc_cap = m
        .retype(arena, ObjType::SchedContext, 0, 0)
        .expect("retype SC");
    m.put_cap(c, 10, sc_cap).expect("put SC");
    let sc = SlotRef::new(c, 10);
    // Cấu hình + Start như `loop_svc::round`.
    m.sched_control_configure(SlotRef::new(c, 7), sc, 1_000, 10_000)
        .expect("SC budget");
    m.set_sched_params(tcb, Some(sc)).expect("sched params");
    m.tcb_configure(tcb, Some(SlotRef::new(c, 0)))
        .expect("cspace");
    let tcb_obj = m.cap_obj(tcb_cap).expect("tcb obj");
    m.start_tcb(tcb_obj).expect("Ready");
    // Destroy: xoá cap rồi Revoke arena ⇒ finalise Tcb/SC + watermark 0.
    m.delete(tcb).expect("delete Tcb cap");
    m.delete(sc).expect("delete SC cap");
    m.revoke(c, Some(8)).expect("revoke arena");
    m.total_used()
}

/// **Ca chính** — `total_used()` trước = sau **hai** vòng liên tiếp; byte không tích luỹ.
#[test]
fn loop_spawn_destroy_khong_ro_ri_qua_hai_vong() {
    let (mut m, c) = rig();
    // `before` = watermark sau khi dựng cảnh (arena + SchedControl đã tiêu byte) — mốc so sánh.
    let before = m.total_used();
    assert!(before > 0, "cảnh đã cấp arena/SchedControl ⇒ watermark > 0");
    for round_index in 0..2 {
        assert_eq!(
            round(&mut m, c),
            before,
            "vòng {round_index}: spawn→destroy phải trả byte về đúng ban đầu"
        );
    }
    // Arena (cap ở ô 8) vẫn sống — `loop_svc` giữ nó cho các vòng sau.
    assert!(
        m.cnode_slot(c, 8).ok().flatten().is_some(),
        "cap arena 4 KiB phải còn sau các vòng"
    );
}

/// **Ca đối chứng** — nếu **bỏ** bước `Revoke` thì `total_used()` **không** về mốc ban đầu
/// (chứng minh phép đo của ca chính không tự nhiên xanh).
#[test]
fn khong_revoke_thi_used_tang_va_ca_chinh_se_do() {
    let (mut m, c) = rig();
    let before = m.total_used();
    let arena = SlotRef::new(c, 8);
    let cap = m.retype(arena, ObjType::Tcb, 0, 0).expect("retype Tcb");
    m.put_cap(c, 9, cap).expect("put Tcb");
    // KHÔNG Delete/Revoke: object vẫn nằm dưới watermark ⇒ `used > before`.
    assert!(
        m.total_used() > before,
        "không revoke thì watermark phải > mốc — nếu bằng thì phép đo rò rỉ vô nghĩa"
    );
}
