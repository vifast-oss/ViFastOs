// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342b (D2-243)** — miền **BỘ NHỚ USER / `SyscallEnv`** của [`crate::user`]: lớp đọc bộ nhớ
//! user (`UserMemAccess`, `sstatus.SUM`), `user_range_mapped`, cài đặt [`vifast_core::syscall::SyscallEnv`]
//! (DebugPutStr/WaitAddr/invoke/IPC buffer/TLS-VSpace hook) và `with_kernel_root`.
//!
//! Tách khỏi `user.rs` cho trần R1 (≤ 1.500 dòng/tệp; mục tiêu `user.rs < 1.100` theo D2-243) —
//! **không** đổi một dòng logic nào. Đường dẫn công khai giữ nguyên nhờ `pub use` ở `user.rs`.

use super::*;

/// Lớp đọc bộ nhớ user của syscall `DebugPutStr`: kiểm dải user + PTE (`U|R`), đọc bằng
/// `sstatus.SUM`, in **raw** từng byte ra UART.
pub(crate) struct UserMemAccess;

/// `[va, va+len)` nằm trong một dải **đã map** của tác vụ đang chạy (một nguồn: `active_ranges`)
/// — cho `ipc_buffer::read_header` (KT-67b) và `put_str`.
pub(crate) fn user_range_mapped(va: u64, len: u64) -> bool {
    active_ranges().contains(va, len)
}

impl SyscallEnv for UserMemAccess {
    /// **KT-314** — `Tcb.Resume`: giữ `entry` để `run_task` nạp `sepc` (D2-83: core không ghi CSR).
    fn tcb_resume(&mut self, tcb: ObjId, entry: u64) -> bool {
        crate::user_run::arm_resume_pc(tcb, entry);
        true
    }

    /// **KT-320/KT-325 + D2-242 nhóm 3** — giữ danh tính `VSpace` để `run_task` tiêu thụ
    /// (D2-83); `0` = xoá; bảng khe **tràn** ⇒ `Failed(Busy)` 14 (core rollback).
    fn tcb_vspace(&mut self, tcb: ObjId, vspace: ObjId) -> VspaceOutcome {
        match crate::user_run::arm_vspace(tcb, vspace) {
            true => VspaceOutcome::Applied,
            false => VspaceOutcome::Failed(vifast_core::KError::Busy),
        }
    }

    /// KT-62: `DebugPutStr` (12) — cùng đường đọc user như KT-53 (dải đã map + `sstatus.SUM`).
    fn put_str(&mut self, ptr: u64, len: u64) -> Result<u64, u64> {
        let range = UserRange::new(USER_BASE, USER_TOP);
        if !range.contains(ptr, len) {
            return Err(ERR_INVALID_ARGUMENT);
        }
        if len == 0 {
            return Ok(0);
        }
        // Sau khi `satp` trỏ root user, kernel **không** có physmap để `walk` PTE (xem
        // `obj::copy_kernel_low_root`). Loader ghi bảng dải `[base,end)` của **mọi** trang đã map
        // U|R (segment + stack); kiểm trọn dải trước khi đọc ⇒ con trỏ xấu nhận `NotMapped`,
        // không biến thành page fault S-mode (fatal).
        if !active_ranges().contains(ptr, len) {
            return Err(ERR_NOT_MAPPED);
        }
        // Đọc/ghép từng khối 64 B trong cửa sổ SUM, tắt SUM trước khi in (không giữ SUM lâu hơn
        // cần thiết và không để rò bit SUM sang đường kernel khác).
        let mut copied = 0u64;
        let mut buf = [0u8; 64];
        while copied < len {
            let n = core::cmp::min(64u64, len - copied) as usize;
            aobj::set_sum(true);
            // SAFETY: mọi trang đã kiểm leaf `U|R`; đọc 1 byte trong [0, 4096) của trang đó;
            // SUM bật nên S-mode đọc được trang U; tắt ngay sau vòng.
            unsafe {
                for (i, slot) in buf.iter_mut().enumerate().take(n) {
                    let addr = ptr + copied + i as u64;
                    *slot = core::ptr::read_volatile(addr as *const u8);
                }
            }
            aobj::set_sum(false);
            for byte in buf.iter().take(n) {
                debug_put_byte(*byte);
                capture_byte(*byte);
            }
            copied += n as u64;
        }
        Ok(len)
    }

    /// WaitAddr/WakeAddr (9/10): dịch VA user ⇒ **nhãn** `(ppn, offset)` cho core (D2-83 — core
    /// chỉ giữ khoá, không sở hữu Frame/PTE).
    ///
    /// Đang chạy với `satp` = root user ([`crate::obj::copy_kernel_low_root`]) nên **không** có
    /// physmap để walk: tạm khôi phục root kernel, walk PTE thật, rồi trả lại root user trước khi
    /// về U-mode. Lỗi ⇒ `NotMapped` (11) / `InvalidArgument` (5), không panic.
    fn resolve_addr(&mut self, va: u64) -> Result<(u32, u32), u64> {
        let range = UserRange::new(USER_BASE, USER_TOP);
        if !range.contains_addr(va) {
            return Err(ERR_INVALID_ARGUMENT);
        }
        let vs = LAST_VSPACE.load(Ordering::Relaxed);
        if vs == u32::MAX {
            return Err(ERR_NOT_MAPPED);
        }
        let vspace = CapId(vs as u16);
        let (root, asid) = {
            let m = aobj::machine();
            (m.vspace_root_pa(vspace), m.vspace_asid(vspace))
        };
        let (Ok(root), Ok(asid)) = (root, asid) else {
            return Err(ERR_NOT_MAPPED);
        };
        if !aobj::restore_kernel_root() {
            return Err(ERR_NOT_MAPPED);
        }
        let walk = aobj::machine().walk(vspace, va);
        let back = aobj::switch_root(root, asid);
        if !back {
            // F7 reviewer-kt62: từ đây `satp` = root kernel mà đường về U-mode vẫn `sret` ⇒ sai
            // nặng (user chạy với root kernel). Dừng có chẩn đoán, không trả lỗi mềm.
            vifast_plat_common::Platform::put_str(
                &Riscv64Virt,
                "PANIC: u-mode resolve_addr khong tra duoc satp user\n",
            );
            crate::debug_halt(1)
        }
        let Ok(info) = walk else {
            return Err(ERR_NOT_MAPPED);
        };
        if !info.valid || !info.leaf || info.level != 0 {
            return Err(ERR_NOT_MAPPED);
        }
        Ok(((info.phys >> 12) as u32, (va & 0xfff) as u32))
    }

    /// `ClockRead` (11) — đồng hồ đơn điệu của nền tảng: **`rdtime`** (`timer::now_ticks`, ns)
    /// — D2-88(b); SBI TIME chỉ dùng cho **ngắt** timer, không phải clock read.
    fn now_ns(&mut self) -> u64 {
        Riscv64Virt.now_ns()
    }

    /// `DebugGetInfo` (15) — bitmap tính năng **runtime** của ảnh này (bit 0..3, xem
    /// `vifast_core::syscall::FEATURE_*`).
    fn features(&mut self) -> u64 {
        FEATURE_DEBUG_PUT_STR | FEATURE_DEBUG_HALT | FEATURE_WAIT_ADDR | FEATURE_MCS_YIELD
    }

    /// `DebugGetInfo.num_harts` — **số hart nền tảng** (D2-88(a)): probe SBI HSM nếu SMP chưa
    /// bring-up; `-smp 1/2/4` ⇒ 1/2/4. Không phải số hart model core đang chạy.
    fn num_harts(&mut self) -> u64 {
        crate::smp::hart_count() as u64
    }

    /// **Invocation** (KT-67a, D2-97): core đã kiểm cap/quyền/loại; object layer thi hành
    /// (`Frame`/`PageTable`/`VSpace` — D2-83). Lỗi trả **mã ABI** đúng nguồn (object layer hoặc
    /// core), không bọc mã mới.
    ///
    /// **F1 (review W8a)** — mã lấy từ [`MemError::to_abi_code_for_invoke`] (**toàn phần**): lỗi
    /// nội bộ `NoSlots`/`Corrupt` đi qua cầu bằng ánh xạ tường minh (`NotEnoughMemory` 9 /
    /// `NotSupported` 18). Trước bản vá, mã được lấy bằng `abi_code()` rồi **mặc định 5**
    /// (`InvalidArgument`) — biến hai lỗi này thành "tham số sai" trong khi tham số đúng.
    fn invoke(
        &mut self,
        op: vifast_core::invoke::InvokeOp,
    ) -> Result<vifast_core::invoke::InvokeOutcome, u64> {
        // Trong syscall U-mode, `satp` vẫn trỏ **root user** (không có physmap — xem
        // `resolve_addr`): object layer chỉ chạm được sau khi khôi phục root kernel, rồi phải
        // trả lại root user trước khi `sret` về U-mode. Cùng khuôn `resolve_addr`.
        with_kernel_root(|| crate::mem_bridge::invoke(op))
            .and_then(|result| result.map_err(|error| u64::from(error.to_abi_code_for_invoke())))
    }

    /// Rollback object vừa retype khi core không dựng nổi object đại diện (bảng cap đầy).
    fn release_object(&mut self, key: u64) {
        let _ = with_kernel_root(|| crate::mem_bridge::release_key(key));
    }

    /// **KT-67b** — header **IPC buffer** của TCB gọi (D2-98/spec dòng 58): magic +
    /// `MessageInfo` bản ghi + receive window 4 CPTR + 4 CPTR nguồn cap. Đọc qua
    /// [`crate::ipc_buffer::read_header`] (trang `USER_IPC_BUFFER_VA`, SUM, chỉ trong syscall
    /// U-mode). `None` = chưa có lượt U-mode / chưa map / magic 0 (ảnh cũ giữ nguyên hành vi).
    fn ipc_header(&mut self, _tcb: ObjId) -> Option<vifast_core::ipc::IpcHeader> {
        if LAST_VSPACE.load(Ordering::Relaxed) == u32::MAX {
            return None;
        }
        crate::ipc_buffer::read_header()
    }

    /// **KT-282** — MR4..MR63 trong IPC buffer (D2-83); `va` do core truyền, `None` ⇔ chưa map.
    fn ipc_buffer_words(&mut self, _tcb: ObjId, va: u64) -> Option<[u64; 60]> {
        crate::ipc_buffer::read_words(va)
    }
}

/// Chạy `f` trên **root kernel** rồi trả lại đúng root user (`LAST_VSPACE`/`LAST_ASID`) — bắt
/// buộc cho mọi thao tác object layer phát sinh từ syscall U-mode (D2-83): root user không map
/// nửa cao/physmap (KT-53), nên `PhysMem` chỉ chạm được khi `satp` = root kernel.
///
/// Trả `Err(11)` (`NotMapped`) khi chưa có lượt U-mode; nếu không trả lại được root user thì
/// dừng có chẩn đoán (chạy tiếp sẽ `sret` với root kernel — sai nặng, như `resolve_addr`).
fn with_kernel_root<R>(f: impl FnOnce() -> R) -> Result<R, u64> {
    let vs = LAST_VSPACE.load(Ordering::Relaxed);
    if vs == u32::MAX {
        return Err(ERR_NOT_MAPPED);
    }
    let vspace = CapId(vs as u16);
    let (root, asid) = {
        let m = aobj::machine();
        (m.vspace_root_pa(vspace), m.vspace_asid(vspace))
    };
    let (Ok(root), Ok(asid)) = (root, asid) else {
        return Err(ERR_NOT_MAPPED);
    };
    if !aobj::restore_kernel_root() {
        return Err(ERR_NOT_MAPPED);
    }
    let out = f();
    if !aobj::switch_root(root, asid) {
        vifast_plat_common::Platform::put_str(
            &Riscv64Virt,
            "PANIC: u-mode invocation khong tra duoc satp user\n",
        );
        crate::debug_halt(1)
    }
    Ok(out)
}
