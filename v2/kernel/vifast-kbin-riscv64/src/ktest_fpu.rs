// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — pha **FPU/SIMD eager theo TCB** trên ảnh `vifast-kbin-ktest-user` (TAP 38).
//!
//! # Đo cái gì
//!
//! Hai tác vụ U-mode **riêng** (`fpu.elf` nạp **hai** lần vào hai VSpace) chạy xen kẽ qua
//! `Yield`; mỗi tác vụ nạp `f0..f31` bằng mẫu **riêng của nó** (`(task << 32) | (0x100 + reg)`)
//! rồi sau **mỗi** lần được resume kiểm lại **toàn bộ** 32 thanh ghi. Tác vụ nào thấy lệch ⇒
//! `Exit(1)`; cả hai `Exit(0)` ⇔ **không lẫn thanh ghi**.
//!
//! # Vì sao đây là ca **đỏ** khi chưa có KT-418
//!
//! Trapframe 288 B **không** chứa `f0..f31`; trước KT-418 `sstatus.FS = 0` nên lệnh FP ở U-mode
//! trap `illegal instruction` (tác vụ chết ngay lệnh `fmv.d.x` đầu tiên). Sau KT-418: `FS` được
//! bật, ngữ cảnh lưu/khôi phục theo **TCB** ⇒ mỗi tác vụ đọc lại đúng mẫu của mình.
//!
//! # Bằng chứng `# fpu:`
//!
//! Kernel in bộ đếm **thật** của `vifast_plat_riscv64::fpu::stats()`: `saves` (lưu vì bẩn),
//! `skipped` (bỏ lưu vì sạch), `restores` (khôi phục), `fresh` (trạng thái mới), `overflow`
//! (tràn khe — phải `0`), cộng **hai khe khác nhau** của hai TCB (bất biến "không lẫn vùng lưu").
//!
//! Không `unsafe` trong tệp này (R5): mọi truy cập core/object layer qua API an toàn.

use core::fmt::Write;

use vifast_core::vm::CapId;
use vifast_core::{Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::trap::{TrapFrame, REG_A0, REG_SP};
use vifast_plat_riscv64::user::{self, Stop};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Chương trình U-mode **dấu phẩy động** do `build.rs` sinh (`init/fpu_user.rs` + `.S`).
static FPU_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fpu.elf"));

/// Ô bảng dải user của pha này (`MAX_PROCS = 6`; 0/3 đã dùng ở KT-68, 1/2 ở KT-67a/67b).
const LOAD_SLOT: usize = 4;

/// Số bước round-robin tối đa — mỗi tác vụ `Yield` 8 lần rồi `Exit`, nên 2 × (8 + 1) = 18 bước
/// là đủ; trần 200 để máy không treo nếu một tác vụ fault (TAP đỏ, không treo).
const MAX_STEPS: usize = 200;

/// Một tác vụ U-mode giữ trapframe để chạy tiếp sau `Yield`.
struct Task {
    tf: TrapFrame,
    tcb: ObjId,
    /// Task id đặt vào `a0` **một lần** ở lượt đầu (chương trình dùng nó làm mẫu FPU).
    id: u64,
    started: bool,
    exited: bool,
    code: u64,
}

impl Task {
    fn new(tcb: ObjId, id: u64, entry: u64) -> Self {
        let mut tf = TrapFrame::zeroed();
        tf.x[REG_SP] = user::USER_STACK_TOP;
        tf.sepc = entry;
        Self {
            tf,
            tcb,
            id,
            started: false,
            exited: false,
            code: u64::MAX,
        }
    }
}

/// Đưa TCB `Inactive/Ready` → `Running{hart:0}` (cùng khuôn `ktest_capa`).
fn set_running(m: &mut Machine, tcb: ObjId) -> bool {
    if matches!(m.tcb_state(tcb), Ok(TcbState::Running { .. })) {
        return true;
    }
    if m.tcb_state(tcb) == Ok(TcbState::Inactive) && m.start_tcb(tcb).is_err() {
        return false;
    }
    m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
        .is_ok()
}

/// Đưa TCB về `Ready` sau `Yield` (cùng khuôn `ktest_capa`).
fn task_ready(m: &mut Machine, tcb: ObjId) {
    let _ = match m.tcb_state(tcb) {
        Ok(TcbState::Inactive) => m.start_tcb(tcb),
        Ok(TcbState::Ready) => Ok(()),
        Ok(TcbState::Running { .. }) => m.tcb_yield(tcb),
        _ => m.wake(tcb),
    };
}

/// Rig của pha: hai TCB + hai SC trong **một** CNode riêng (không đụng slot của pha trước).
struct Rig {
    tcb_a: ObjId,
    tcb_b: ObjId,
}

/// Dựng rig: CNode 16 slot (slot 0 = self cap, 1/2 = TCB, 3/4 = SC) + nạp `fpu.elf` **hai** lần.
fn build_rig(
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    stage: &mut &'static str,
) -> Option<(Rig, user::LoadedInit, user::LoadedInit)> {
    *stage = "load a";
    let loaded_a = match user::load_init_slot(FPU_ELF, load_untyped, LOAD_SLOT) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "load b";
    let loaded_b = match user::load_init_slot(FPU_ELF, load_untyped, LOAD_SLOT) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "core rig";
    let rig = core_bridge::with_machine(|m| -> Option<Rig> {
        let cnode = m.add_cnode(16);
        let self_cap = m.root_cap(cnode);
        m.put_cap(cnode, 0, self_cap).ok()?;
        let mut tcbs = [0u32; 2];
        for (index, tcb_slot) in [1usize, 2].into_iter().enumerate() {
            let tcb_cap = m.retype(us, ObjType::Tcb, 0, 0).ok()?;
            let tcb = m.cap_obj(tcb_cap).ok()?;
            m.put_cap(cnode, tcb_slot, tcb_cap).ok()?;
            let sc_cap = m.retype(us, ObjType::SchedContext, 0, 0).ok()?;
            m.put_cap(cnode, tcb_slot + 2, sc_cap).ok()?;
            m.tcb_configure(SlotRef::new(cnode, tcb_slot), Some(SlotRef::new(cnode, 0)))
                .ok()?;
            m.sched_control_configure(ctl, SlotRef::new(cnode, tcb_slot + 2), 500, 5_000)
                .ok()?;
            m.set_sched_params(
                SlotRef::new(cnode, tcb_slot),
                Some(SlotRef::new(cnode, tcb_slot + 2)),
            )
            .ok()?;
            m.tcb_set_priority(SlotRef::new(cnode, tcb_slot), 3, 10)
                .ok()?;
            tcbs[index] = tcb;
        }
        Some(Rig {
            tcb_a: tcbs[0],
            tcb_b: tcbs[1],
        })
    });
    rig.map(|rig| (rig, loaded_a, loaded_b))
}

/// Pha FPU: TAP 38. Trả số fail (0 = xanh).
pub fn fpu_phase(_root: ObjId, us: SlotRef, ctl: SlotRef, load_untyped: CapId, fails: &mut usize) {
    let mut stage = "start";
    let Some((rig, loaded_a, loaded_b)) = build_rig(us, ctl, load_untyped, &mut stage) else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(w, "# fpu: rig FAIL stage={stage}");
        let _ = writeln!(w, "not ok 38 - fpu hai tac vu khong lan thanh ghi");
        *fails += 1;
        return;
    };
    let mut tasks = [
        Task::new(rig.tcb_a, 1, loaded_a.entry),
        Task::new(rig.tcb_b, 2, loaded_b.entry),
    ];
    let loads = [&loaded_a, &loaded_b];
    user::enable_multitask(true);
    let mut steps = 0usize;
    while steps < MAX_STEPS && !tasks.iter().all(|task| task.exited) {
        for (index, task) in tasks.iter_mut().enumerate() {
            if task.exited {
                continue;
            }
            steps += 1;
            user::set_active_slot(LOAD_SLOT);
            user::bind_core_tcb(task.tcb);
            if !core_bridge::with_machine(|m| set_running(m, task.tcb)) {
                task.exited = true;
                continue;
            }
            if !task.started {
                // Task id đi vào `a0` của trapframe **một lần** — chương trình dùng nó làm mẫu
                // FPU; các lượt resume sau giữ nguyên `a0` do chương trình không đọc lại.
                task.tf.x[REG_A0] = task.id;
                task.started = true;
            }
            let stop = user::run_task(loads[index], &mut task.tf);
            task.tf = user::saved_tf();
            match stop {
                Stop::Yield => core_bridge::with_machine(|m| task_ready(m, task.tcb)),
                Stop::Exit { code } => {
                    task.exited = true;
                    task.code = code;
                }
                Stop::Halt { code } => {
                    task.exited = true;
                    task.code = 0x8000_0000 | code;
                }
                Stop::Fault => {
                    task.exited = true;
                    task.code = 0x4000_0000;
                }
                Stop::Blocked(_) => {
                    task.exited = true;
                    task.code = 0x2000_0000;
                }
            }
        }
    }
    user::enable_multitask(false);

    let stats = vifast_plat_riscv64::fpu::stats();
    let slot_a = vifast_plat_riscv64::fpu::slot_of(rig.tcb_a);
    let slot_b = vifast_plat_riscv64::fpu::slot_of(rig.tcb_b);
    let valid_a = vifast_plat_riscv64::fpu::is_valid(rig.tcb_a);
    let valid_b = vifast_plat_riscv64::fpu::is_valid(rig.tcb_b);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# fpu: tcb_a={} tcb_b={} slot_a={} slot_b={} valid_a={} valid_b={} saves={} skipped={} restores={} fresh={} overflow={} steps={} code_a={} code_b={}",
            rig.tcb_a,
            rig.tcb_b,
            slot_a.map(|s| s as i64).unwrap_or(-1),
            slot_b.map(|s| s as i64).unwrap_or(-1),
            u8::from(valid_a),
            u8::from(valid_b),
            stats.saves,
            stats.skipped,
            stats.restores,
            stats.fresh,
            stats.overflow,
            steps,
            tasks[0].code,
            tasks[1].code,
        );
    }
    // Điều kiện xanh: **cả hai** tác vụ `Exit(0)` (mẫu FPU của mình còn nguyên sau mỗi lần
    // resume), hai khe **khác nhau** (không dùng chung vùng lưu), 0 tràn khe, và **có** ít nhất
    // một lần lưu + một lần khôi phục thật (nếu không, phép kiểm rỗng).
    let ok = tasks[0].code == 0
        && tasks[1].code == 0
        && slot_a.is_some()
        && slot_b.is_some()
        && slot_a != slot_b
        && stats.overflow == 0
        && stats.saves >= 1
        && stats.restores >= 1;
    {
        let mut w = PlatformWriter(&PLATFORM);
        if ok {
            let _ = writeln!(w, "ok 38 - fpu hai tac vu khong lan thanh ghi");
        } else {
            let _ = writeln!(w, "not ok 38 - fpu hai tac vu khong lan thanh ghi");
            *fails += 1;
        }
    }
    loaded_a.release();
    loaded_b.release();
}
