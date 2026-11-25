// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342b (D2-243)** — miền **TRAP U-MODE** của [`crate::user`]: `on_u_exception` (syscall
//! `scause=8` + fault U-mode), `classify_u_fault` (PTE thật khi `ktest`), `read_sstatus`,
//! `exit_to_kernel` (đường về S-mode sau `Exit`).
//!
//! Tách khỏi `user.rs` cho trần R1 (≤ 1.500 dòng/tệp; mục tiêu `user.rs < 1.100` theo D2-243) —
//! **không** đổi một dòng logic nào. Đường dẫn công khai giữ nguyên nhờ `pub use` ở `user.rs`.

use super::*;
use core::fmt::Write;

/// Trap **từ U-mode** đã decode: syscall (`scause=8`) hoặc fault U-mode (mọi loại khác).
///
/// - `ecall` ⇒ dispatch ABI; `Return` ghi `a0/a1` + `sepc += 4`; `Exit` chuyển trapframe về
///   S-mode (`user_exit_resume`), **không** quay lại U-mode; `Halt` log + SRST.
/// - Fault ⇒ ghi `USER_TASK`/`TRAP_STATS`, in `# u-mode fault:` rồi `PANIC: u-mode fault …` +
///   `DebugHalt(1)` — không treo, không resume.
pub fn on_u_exception(tf: &mut TrapFrame, exc: &Exception) {
    crate::user_tls::restore_kernel_tp();
    let saved = *tf;
    if exc.kind == ExceptionKind::EnvironmentCallFromU {
        // Bằng chứng phần cứng: sstatus **ngay sau trap** từ U-mode (SPP=0, SIE=0, SPIE=0, SUM=0).
        USER_TASK.record_trap_sstatus(read_sstatus());
        USER_TASK.record_ecall(exc.from_supervisor);
        // KT-74: `tp` của U-mode **tại trap** (không phải giá trị kernel tự khai) — evidence
        // `# tls:`. RISC-V: `tp` = `x4`; trap entry đã lưu đủ GPR nên đọc thẳng từ frame.
        crate::user_tls::record_tls_trap(tf);
        let nr = tf.reg(REG_A7);
        let args = [
            tf.arg(0),
            tf.arg(1),
            tf.arg(2),
            tf.arg(3),
            tf.arg(4),
            tf.arg(5),
        ];
        // KT-65: trong pha đa nhiệm, `Yield` (8) là **điểm nhường CPU** của scheduler U-mode
        // (kernel ở S-mode chọn tác vụ kế). Không đi vào core: core vẫn là đường của KT-62 và
        // được kiểm ở ảnh có cùng `Machine`; ở đây `Yield` chỉ trả `a0=0,a1=0` rồi về S-mode.
        if MULTITASK.load(Ordering::Relaxed) != 0 && nr == 8 {
            tf.set_arg(0, 0);
            tf.set_arg(1, 0);
            tf.advance_pc(4);
            record_sys(nr, 0, 0);
            save_tf(tf);
            stop(2, 0);
            crate::TRAP_STATS.record_exception(&saved, exc.kind, true);
            exit_to_kernel(tf);
            return;
        }
        // KT-62: mọi số hiệu `0..15` đi vào `Machine` thật qua `vifast_core::syscall::dispatch`
        // (façade `abi::dispatch` chỉ ghi sổ ABI). Không còn nhánh "3 syscall + NotSupported".
        let Some(tcb) = core_tcb() else {
            {
                let mut w = PlatformWriter(&Riscv64Virt);
                let _ = writeln!(w, "# u-mode syscall: nr={nr} core_tcb=none");
            }
            crate::TRAP_STATS.record_exception(&saved, exc.kind, false);
            vifast_plat_common::Platform::put_str(
                &Riscv64Virt,
                "PANIC: u-mode khong co core tcb\n",
            );
            crate::debug_halt(1)
        };
        let action = dispatch_on_private_stack(tcb, nr, args);
        match action {
            SyscallAction::Return {
                error,
                value,
                info,
                mr,
            } => {
                tf.set_arg(0, error);
                tf.set_arg(1, value);
                tf.set_arg(2, info);
                for (index, word) in mr.iter().enumerate() {
                    tf.set_arg(3 + index, *word);
                }
                // **KT-282** — MR4..MR63 của message vừa nhận vào IPC buffer (đường **nhận**).
                if matches!(nr, 3..=7) {
                    crate::ipc_buffer::apply_received_now(tcb);
                }
                record_sys(nr, error, value);
                if MULTITASK.load(Ordering::Relaxed) == 0 {
                    // Bằng chứng `# sys:` KT-62: số hiệu, mã trả về, TCB + trạng thái **core**.
                    // KT-65: pha đa nhiệm **không** in từng syscall — trap stack per-hart chỉ
                    // 16 KiB (bất biến KT-49); in ở đây làm sâu thêm khung handler không cần thiết.
                    let state = crate::core_bridge::with_machine(|m| m.tcb_state(tcb).ok());
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = writeln!(
                        w,
                        "# sys: nr={nr} err={error} value={value} tcb={tcb} state={} src=core",
                        state.map(crate::user_run::tcb_state_name).unwrap_or("none")
                    );
                }
                tf.advance_pc(4);
                crate::TRAP_STATS.record_exception(&saved, exc.kind, true);
            }
            SyscallAction::Exit { code } => {
                record_sys(nr, 0, code);
                {
                    let state = crate::core_bridge::with_machine(|m| m.tcb_state(tcb).ok());
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = writeln!(
                        w,
                        "# sys: nr={nr} err=0 value={code} tcb={tcb} state={} src=core",
                        state.map(crate::user_run::tcb_state_name).unwrap_or("none")
                    );
                }
                // KT-56/D2-56a: `Exit` là fault kind ABI (đi hàng gửi không Reply — D2-25).
                let kind = FaultKind::Exit { code };
                FAULT_STATS.record(kind);
                // Bằng chứng từ **core**: TCB đã `BlockedOnSend{call:false, fault=Exit}` (hoặc
                // `Inactive` khi không có `fault_ep`) — TAP đọc, không đoán.
                let state = crate::core_bridge::with_machine(|m| m.tcb_state(tcb).ok());
                {
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = write!(
                        w,
                        "# u-mode exit: code={code} faultkind={kind} tcb={tcb} state="
                    );
                    match state {
                        Some(state) => {
                            let _ = writeln!(w, "{}", crate::user_run::tcb_state_name(state));
                        }
                        None => {
                            let _ = writeln!(w, "none");
                        }
                    }
                }
                if MULTITASK.load(Ordering::Relaxed) != 0 {
                    // Trapframe lúc `ecall` (sepc trỏ vào `ecall`, `a0` = mã thoát); scheduler
                    // đọc trạng thái core của TCB để quyết định bước kế.
                    save_tf(tf);
                    stop(1, code);
                }
                exit_to_kernel(tf);
                crate::TRAP_STATS.record_exception(&saved, exc.kind, true);
            }
            SyscallAction::Halt { code } => {
                record_sys(nr, 0, code);
                {
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = writeln!(w, "# user debug_halt: code={code} nr=13");
                }
                crate::TRAP_STATS.record_exception(&saved, exc.kind, true);
                crate::debug_halt(code as usize)
            }
            SyscallAction::Switched { state } => {
                if MULTITASK.load(Ordering::Relaxed) != 0 {
                    // KT-65: tác vụ chặn trong core ⇒ nhường CPU cho scheduler (không PANIC).
                    // Chương trình KT-65 chỉ dùng NBRecv/NBSend nên nhánh này là lưới an toàn.
                    //
                    // **KT-280**: syscall **đã hoàn tất** ở phía core (chặn = chờ message/reply),
                    // nên khi tác vụ được resume nó phải chạy tiếp **sau** `ecall`, không chạy lại
                    // `ecall` (chạy lại sẽ gửi/nhận lần hai — sai ngữ nghĩa và có thể treo).
                    // `sepc += 4` ở đây là nửa thứ nhất của hợp đồng resume; nửa thứ hai là
                    // [`apply_ipc_return`] (nạp `a0..a6` từ vùng nhận IPC của TCB).
                    tf.advance_pc(4);
                    save_tf(tf);
                    stop(3, 0);
                    crate::TRAP_STATS.record_exception(&saved, exc.kind, true);
                    exit_to_kernel(tf);
                    return;
                }
                // KT-62: ảnh U-mode chưa có scheduler; TCB chặn/ngủ mà quay lại U-mode là sai.
                // Dừng có chẩn đoán (TAP kernel đọc trạng thái của nó trước khi tới đây).
                {
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = writeln!(
                        w,
                        "# u-mode switched: nr={nr} tcb={tcb} state={}",
                        crate::user_run::tcb_state_name(state)
                    );
                }
                crate::TRAP_STATS.record_exception(&saved, exc.kind, false);
                vifast_plat_common::Platform::put_str(
                    &Riscv64Virt,
                    "PANIC: u-mode syscall khong the quay lai U-mode\n",
                );
                crate::debug_halt(1)
            }
        }
        return;
    }

    // Fault U-mode: phân loại `FaultKind` từ **trapframe phần cứng** + PTE thật của VSpace
    // (sau khi khôi phục root kernel — root user không có physmap). Ghi bằng chứng TRƯỚC khi
    // quyết định fatal hay resume (deviation test-only: `enter_expect_fault`).
    let recover = USER_FAULT_RECOVER.swap(0, Ordering::SeqCst) == 1;
    USER_TASK.record_fault(saved.scause, saved.stval, saved.sepc);
    let kind = classify_u_fault(&saved, exc);
    if let Some(kind) = kind {
        FAULT_STATS.record(kind);
    }
    crate::TRAP_STATS.record_exception(&saved, exc.kind, recover);
    {
        let mut w = PlatformWriter(&Riscv64Virt);
        let _ = write!(
            w,
            "# u-mode fault: scause=0x{:x} stval=0x{:x} sepc=0x{:x} from_u=1 kernel_addr={} recover={} faultkind=",
            saved.scause,
            saved.stval,
            saved.sepc,
            is_kernel_addr(saved.stval) as u8,
            u8::from(recover)
        );
        match kind {
            Some(kind) => {
                let _ = writeln!(w, "{kind}");
            }
            None => {
                let _ = writeln!(w, "none");
            }
        }
    }
    if recover {
        // Test-only: về S-mode ngay sau trap (không resume instruction lỗi); caller TAP kiểm
        // `FaultKind` + `scause/stval` từ `FAULT_STATS`.
        exit_to_kernel(tf);
        return;
    }
    // KT-70: pha pager — fault VM **không fatal**: giữ trapframe (sepc = instruction lỗi), trả
    // scheduler; pager map trang xong thì `run_task` chạy lại instruction lỗi.
    if pager_faults() {
        if let Some((asid, va, access, reason)) = kind.and_then(|kind| kind.vm()) {
            crate::pager_fault::record(va, asid, access.code() as u32, reason.code() as u32);
            let mut w = PlatformWriter(&Riscv64Virt);
            let _ = writeln!(
                w,
                "# pager: fault kind=Vm va=0x{:x} asid={} access={} reason={} sepc=0x{:x}",
                va,
                asid,
                access.name(),
                reason.name(),
                saved.sepc
            );
            save_tf(tf);
            stop(5, 0);
            crate::TRAP_STATS.record_exception(&saved, exc.kind, true);
            exit_to_kernel(tf);
            return;
        }
    }
    // **KT-411 (vá hồi quy KT-409b)**: module `trap` đã dời khỏi `vifast-plat-common` sang
    // `vifast-plat-riscv64-logic` ở `a2532a67`; tham chiếu cũ còn sót ⇒ `--features user` **không
    // biên dịch được** trên base `e495365f` (6/14 ảnh ktest). Sửa đúng đường dẫn, 0 đổi ngữ nghĩa.
    vifast_plat_riscv64_logic::trap::report_u_fault(&Riscv64Virt, exc, tf)
}

/// Phân loại fault U-mode **từ trapframe** (scause/stval/sepc) + PTE thật của VSpace đã nạp.
///
/// `restore_kernel_root` trước khi `walk`: sau `sret` xuống U-mode, root đang hoạt động là root
/// user **không** có physmap ([`crate::obj::copy_kernel_low_root`], KT-53), nên không thể đọc
/// PTE; khôi phục root kernel là an toàn vì fault này không quay lại instruction lỗi.
fn classify_u_fault(tf: &TrapFrame, exc: &Exception) -> Option<FaultKind> {
    #[cfg(feature = "ktest")]
    {
        let restored = aobj::restore_kernel_root();
        let vs = LAST_VSPACE.load(Ordering::Relaxed);
        let pte = if restored && vs != u32::MAX {
            aobj::machine()
                .walk(CapId(vs as u16), tf.stval)
                .ok()
                .map(|w| PteFacts::new(w.valid, w.leaf, w.raw))
        } else {
            None
        };
        let facts = TrapFacts {
            scause: exc.scause,
            stval: exc.stval,
            sepc: exc.sepc,
            from_user: true,
        };
        fault::classify_trap(&facts, LAST_ASID.load(Ordering::Relaxed), pte)
    }
    #[cfg(not(feature = "ktest"))]
    {
        let _ = tf;
        // Không có physmap để walk trong chế độ không-ktest: vẫn phân loại theo **cùng bảng**
        // `scause` (12/13/15 ⇒ `Vm`, còn lại `UserException`), `pte=None` ⇒ `NotMapped`.
        let facts = TrapFacts {
            scause: exc.scause,
            stval: exc.stval,
            sepc: exc.sepc,
            from_user: true,
        };
        fault::classify_trap(&facts, 0, None)
    }
}

/// Đọc `sstatus` (giữ UXL/FS; loader tự xoá SPP/SPIE/SIE/SUM).
pub(crate) fn read_sstatus() -> u64 {
    let value: u64;
    // SAFETY: chỉ đọc CSR `sstatus` của hart hiện tại.
    unsafe { asm!("csrr {}, sstatus", out(reg) value, options(nomem, nostack)) };
    value
}

/// Chuyển trapframe về S-mode tại [`user_exit_resume`] — **không** quay lại U-mode sau `Exit`.
///
/// Ghi CSR `sstatus.SPP=1` (vì asm resume của `trap_entry` **không** ghi `sstatus` — P2a KT-45),
/// xoá SPIE/SIE/SUM; đặt `sepc`/`sp` kernel để asm resume `sret` về S-mode.
fn exit_to_kernel(tf: &mut TrapFrame) {
    tf.sstatus = (tf.sstatus | SSTATUS_SPP) & !(SSTATUS_SPIE | SSTATUS_SIE | SSTATUS_SUM);
    let resume: unsafe extern "C" fn() = user_exit_resume;
    tf.sepc = resume as usize as u64;
    tf.x[REG_SP] = kernel_ctx_sp();
    // SAFETY: chỉ sửa bit CSR `sstatus`; SIE đang 0 trong trap handler nên sret chưa bật ngắt.
    unsafe {
        asm!(
            "csrs sstatus, {spp}",
            "csrc sstatus, {clear}",
            spp = in(reg) SSTATUS_SPP,
            clear = in(reg) (SSTATUS_SPIE | SSTATUS_SIE | SSTATUS_SUM),
            options(nostack, preserves_flags),
        );
    }
}
