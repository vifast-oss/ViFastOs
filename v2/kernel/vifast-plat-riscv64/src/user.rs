// SPDX-License-Identifier: MIT OR Apache-2.0
//! U-mode trên QEMU `virt` (KT-53) — phần **không viết "safe" được** của
//! [`vifast_plat_riscv64_logic::user`]: nạp `init` ELF vào VSpace thật, dựng trapframe U-mode, `sret`
//! xuống U-mode, bắt `ecall` (`scause=8`) và đọc bộ nhớ user bằng `sstatus.SUM`.
//!
//! # Cơ chế `sscratch` đã chọn (bắt buộc ghi rõ — KT-53)
//!
//! Từ KT-45/KT-49, **`sscratch` luôn giữ đỉnh trap stack per-hart** (giá trị kernel), cả khi
//! đang chạy trong kernel lẫn khi đang chạy trong **U-mode**:
//!
//! - `sret` xuống U-mode **không** đổi `sscratch`; U-mode không đọc/ghi được CSR supervisor.
//! - Trap từ U-mode: `trap_entry` làm `csrrw sp, sscratch, sp` ⇒ `sp = trap stack top` (kernel),
//!   `sscratch = sp user`; frame lưu `x2` = sp user. Đúng một thanh ghi vừa đổi stack vừa giữ
//!   con trỏ kernel — không cần global/`tp` riêng, không lộ địa chỉ kernel cho U-mode.
//! - Trap từ S-mode (timer/PLIC/IPI) dùng **cùng** cơ chế: `sscratch` vẫn là đỉnh trap stack nên
//!   kernel stack bị ngắt được giữ trong `sscratch`, không phụ thuộc đang ở mode nào.
//! - Đường về S-mode khi `Exit` (`user_exit_resume`) khôi phục ngữ cảnh kernel từ
//!   `USER_KERNEL_CTX` (ra/sp/gp/tp/s0..s11) — **không** dùng stack user.
//!
//! Cơ chế này được chọn vì (1) không thêm CSR/global thứ hai, (2) chạy được cho cả trap từ S-mode
//! lẫn U-mode, (3) `sstatus.SPP` phân biệt nguồn trap nên chính sách syscall/fault tách sạch.
//!
//! # W^X khi nạp ELF
//!
//! `vifast-elf::parse` đã từ chối segment `W+X` và segment `PT_DYNAMIC/PT_INTERP/PT_TLS`; loader
//! ở đây kiểm lại `writable && executable` trước khi map và map **đúng quyền** segment:
//! `.text` → `U|R|X` (NX cho W), `.rodata` → `U|R|NX`, `.data` → `U|R|W|NX`; stack user →
//! `U|R|W|NX`, trang **guard** ngay dưới stack **không map**. Object layer (KT-52, D2-67) còn
//! chặn W^X toàn cục theo Frame nên một segment `R|W|X` không thể lọt.
//!
//! # Cách ly
//!
//! Root VSpace của user được chép 512 entry root kernel (test-only, KT-52) để kernel sống khi
//! `satp` trỏ root user; các leaf kernel giữ `U=0`, nên U-mode đọc `0x8020_0000` ⇒ **load page
//! fault** `scause=13` + `stval=0x8020_0000` (không phải đọc được). Fault U-mode là **fatal**:
//! log `# u-mode fault:` rồi `PANIC: u-mode fault …` + `DebugHalt(1)` (SBI SRST), không treo.
//!
//! Mọi `unsafe` ở tệp này thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17): asm
//! `sret`/`sstatus`/`SUM`, đọc bộ nhớ user và ghi frame qua physmap.

use core::arch::{asm, global_asm};
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_core::syscall::{
    SyscallEnv, VspaceOutcome, FEATURE_DEBUG_HALT, FEATURE_DEBUG_PUT_STR, FEATURE_MCS_YIELD,
    FEATURE_WAIT_ADDR,
};
use vifast_core::ObjId;
use vifast_elf::ElfError;
use vifast_plat_common::fault::{self, FaultKind, PteFacts, TrapFacts, FAULT_STATS};
use vifast_plat_common::obj::{CapId, MapFlags, ObjError, ObjKind};
use vifast_plat_common::smp::MAX_HARTS;
use vifast_plat_riscv64_logic::trap::{
    tf_reg_offset, Exception, ExceptionKind, TrapFrame, REG_A0, REG_A7, REG_SP, SSTATUS_SIE,
    SSTATUS_SPIE, SSTATUS_SPP, TF_SEPC_OFFSET, TF_SSTATUS_OFFSET,
};
use vifast_pte::sv39::{PTE_EXEC, PTE_READ, PTE_USER, PTE_WRITE};
// **KT-409b**: phần ngữ nghĩa U-mode/syscall nay ở `vifast_plat_riscv64_logic::user` (dời khỏi `plat-common`).
use vifast_plat_common::{Platform, PlatformWriter};
pub use vifast_plat_riscv64_logic::user::*;
// **KT-411 (vá hồi quy KT-409b)**: `abi` là bí danh cũ của module `user` (nay ở
// `vifast_plat_riscv64_logic::user`); thiếu bí danh ⇒ `abi::dispatch` không phân giải được ⇒
// `--features user` **không biên dịch được** trên base `e495365f`. 0 đổi ngữ nghĩa.
use vifast_plat_riscv64_logic::user as abi;

use crate::obj as aobj;
use crate::user_ranges::UserRanges;

pub use crate::init_pages::LoadedInit;
use crate::{debug_put_byte, mmu, Riscv64Virt};

// KT-66: API stack per-hart cho ảnh test (module riêng `crate::user_stack`).
pub use crate::user_stack::{init_syscall_stacks, print_stack_evidence};

/// `sstatus.SUM` — S-mode được đọc/ghi trang `U=1` (privileged spec).
pub(crate) const SSTATUS_SUM: u64 = 1 << 18;

/// `e_machine` của RISC-V ELF64 (EM_RISCV = 243).
pub const EM_RISCV: u16 = 243;

/// Đáy cửa sổ user `[USER_BASE, USER_TOP)` — root index 1 của Sv39 (kernel root entry 1 rỗng).
pub const USER_BASE: u64 = 0x4000_0000;
/// Root index Sv39 của cửa sổ user (`0x4000_0000 >> 30 = 1`).
pub const USER_ROOT_INDEX: usize = (USER_BASE >> 30) as usize;
/// Đỉnh cửa sổ user — hết root index 1.
pub const USER_TOP: u64 = 0x8000_0000;
/// Đỉnh stack user (mọc xuống).
pub const USER_STACK_TOP: u64 = 0x4010_0000;
/// Cỡ stack user (8 trang 4 KiB).
pub const USER_STACK_BYTES: u64 = 0x8000;
/// Trang guard **không map** ngay dưới stack user.
pub const USER_STACK_GUARD_BOTTOM: u64 = USER_STACK_TOP - USER_STACK_BYTES - 0x1000;
/// Đỉnh trang guard (đáy stack).
pub const USER_STACK_GUARD_TOP: u64 = USER_STACK_TOP - USER_STACK_BYTES;

/// Số tác vụ U-mode tối đa mà runtime đa nhiệm hỗ trợ (KT-65: procsrv + shell + con + dự phòng).
/// Mỗi tác vụ có **bảng dải user riêng**: `DebugPutStr` phải kiểm con trỏ theo đúng VSpace đang
/// chạy, không dùng bảng của lượt nạp trước (KT-53 chỉ có một tác vụ nên bảng là static đơn).
/// KT-157: 4 → 6 (pager + 4 client U-mode A/B/C/D + ô 0 dự phòng). Dung lượng bảng dải
/// (test-only), không phải `MAX_TCBS` của core (giữ 64) và không phải ngưỡng thời gian/seed.
pub const MAX_PROCS: usize = 6;

/// Bảng dải `[base,end)` theo **ô tác vụ** (nạp bằng [`load_init_slot`], chọn bằng
/// [`set_active_slot`]).
/// VSpace của lượt U-mode gần nhất (để phân loại fault **sau khi** khôi phục root kernel).
static LAST_VSPACE: AtomicU32 = AtomicU32::new(u32::MAX);
/// ASID nhãn của VSpace đó — nhãn `FaultKind::Vm.vspace`.
static LAST_ASID: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ lượt U-mode hiện tại **được phép** fault để TAP kiểm `FaultKind` (test-only): handler
/// phân loại + ghi bằng chứng rồi về S-mode, **không** PANIC. Mặc định `0` = chính sách K3
/// (fault U-mode ⇒ log + halt). Đây là deviation **chỉ trong ảnh test**, ghi ở §19 docs.
static USER_FAULT_RECOVER: AtomicU32 = AtomicU32::new(0);

static RANGE_SLOTS: [UserRanges; MAX_PROCS] = [const { UserRanges::new() }; MAX_PROCS];
/// Ô bảng dải mà `load_init_slot` kế tiếp ghi vào.
static LOAD_SLOT: AtomicU32 = AtomicU32::new(0);
/// Ô bảng dải/console capture của tác vụ **đang chạy**.
static ACTIVE_SLOT: AtomicU32 = AtomicU32::new(0);

fn slot_index(slot: usize) -> usize {
    if slot < MAX_PROCS {
        slot
    } else {
        0
    }
}

/// Chọn ô bảng dải cho lượt nạp kế tiếp.
pub fn set_load_slot(slot: usize) {
    LOAD_SLOT.store(slot_index(slot) as u32, Ordering::Relaxed);
}

/// Chọn ô bảng dải (và console capture) của tác vụ sắp chạy.
pub fn set_active_slot(slot: usize) {
    ACTIVE_SLOT.store(slot_index(slot) as u32, Ordering::Relaxed);
}

fn active_ranges() -> &'static UserRanges {
    &RANGE_SLOTS[ACTIVE_SLOT.load(Ordering::Relaxed) as usize]
}

pub(crate) fn loading_ranges() -> &'static UserRanges {
    &RANGE_SLOTS[LOAD_SLOT.load(Ordering::Relaxed) as usize]
}

// ---------------------------------------------------------------------------------------------
// KT-65 — console capture của ảnh test: byte do **U-mode** in ra, không phải kernel in hộ.
// ---------------------------------------------------------------------------------------------

/// Cỡ bộ đệm capture mỗi tác vụ (đủ cho các dòng `# shell:`/`# proc:` của demo).
const CAP_BYTES: usize = 1024;
/// `1` ⇔ đang capture byte `DebugPutStr` theo ô tác vụ đang chạy.
static CAP_ON: AtomicU32 = AtomicU32::new(0);
/// Số byte đã capture theo ô tác vụ.
static CAP_LEN: [AtomicU32; MAX_PROCS] = [const { AtomicU32::new(0) }; MAX_PROCS];
/// Bộ đệm capture theo ô tác vụ.
static mut CAP_BUF: [[u8; CAP_BYTES]; MAX_PROCS] = [[0; CAP_BYTES]; MAX_PROCS];

/// Bắt đầu capture (xoá bộ đệm mọi ô). Chỉ ảnh test dùng.
pub fn capture_begin() {
    for len in CAP_LEN.iter() {
        len.store(0, Ordering::Relaxed);
    }
    CAP_ON.store(1, Ordering::Relaxed);
}

/// Tắt capture.
pub fn capture_end() {
    CAP_ON.store(0, Ordering::Relaxed);
}

fn capture_byte(byte: u8) {
    if CAP_ON.load(Ordering::Relaxed) == 0 {
        return;
    }
    let slot = ACTIVE_SLOT.load(Ordering::Relaxed) as usize;
    let len = CAP_LEN[slot].load(Ordering::Relaxed) as usize;
    if len >= CAP_BYTES {
        return;
    }
    // SAFETY: `CAP_BUF` là static duy nhất; offset `slot * CAP_BYTES + len` đã kiểm
    // `slot < MAX_PROCS`, `len < CAP_BYTES`; chỉ ghi một byte, không tạo tham chiếu `&mut`.
    unsafe {
        core::ptr::addr_of_mut!(CAP_BUF)
            .cast::<u8>()
            .add(slot * CAP_BYTES + len)
            .write_volatile(byte);
    }
    CAP_LEN[slot].store((len + 1) as u32, Ordering::Relaxed);
}

/// Số byte U-mode đã in của ô tác vụ `slot` (chỉ có nghĩa khi capture bật).
pub fn captured_len(slot: usize) -> usize {
    CAP_LEN[slot_index(slot)].load(Ordering::Relaxed) as usize
}

/// `true` ⇔ chuỗi byte `needle` xuất hiện trong output U-mode của ô `slot`.
pub fn captured_contains(slot: usize, needle: &[u8]) -> bool {
    let slot = slot_index(slot);
    let len = captured_len(slot);
    if needle.is_empty() || needle.len() > len {
        return false;
    }
    // SAFETY: cùng static/offset như `capture_byte`; đọc `len` byte đã được ghi trong bộ đệm của
    // ô này, `len <= CAP_BYTES`, chưa có ghi đồng thời (một hart, tuần tự).
    let bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::addr_of!(CAP_BUF)
                .cast::<u8>()
                .add(slot * CAP_BYTES),
            len,
        )
    };
    bytes.windows(needle.len()).any(|window| window == needle)
}

/// Chụp bảng dải của ô `slot` ra đầu ra `out` (tối đa 16 byte) — dùng cho log/TAP.
pub fn captured_prefix(slot: usize, out: &mut [u8]) -> usize {
    let slot = slot_index(slot);
    let len = captured_len(slot).min(out.len());
    // SAFETY: như `captured_contains` — đọc `len` byte từ bộ đệm của ô.
    unsafe {
        core::ptr::copy_nonoverlapping(
            core::ptr::addr_of!(CAP_BUF)
                .cast::<u8>()
                .add(slot * CAP_BYTES),
            out.as_mut_ptr(),
            len,
        );
    }
    len
}

/// `1` ⇔ bật ngữ nghĩa **đa nhiệm** của runtime U-mode (chỉ pha demo KT-65). Mặc định `0` giữ
/// nguyên KT-53/62: `Yield` đi vào core như mọi syscall, `Switched` là lỗi nặng (PANIC).
static MULTITASK: AtomicU32 = AtomicU32::new(0);
/// Loại [`crate::user_run::Stop`] lần gần nhất (0 = chưa có) — `on_u_exception` ghi.
static STOP_KIND: AtomicU32 = AtomicU32::new(0);
/// Mã thoát của `Stop::Exit`/`Stop::Halt`.
static STOP_CODE: AtomicU64 = AtomicU64::new(0);

/// Bật/tắt ngữ nghĩa đa nhiệm của runtime U-mode.
pub fn enable_multitask(on: bool) {
    MULTITASK.store(u32::from(on), Ordering::Relaxed);
}

/// Ghi loại/mã dừng của lượt chạy hiện tại — `on_u_exception` gọi **trước** khi trả về S-mode.
pub(crate) fn stop(kind: u32, code: u64) {
    STOP_CODE.store(code, Ordering::Relaxed);
    STOP_KIND.store(kind, Ordering::Release);
}

/// Loại dừng lần gần nhất (số nguyên thô — `crate::user_run` dịch thành `Stop`).
pub(crate) fn stop_kind() -> u32 {
    STOP_KIND.load(Ordering::Acquire)
}

/// Mã thoát lần gần nhất.
pub(crate) fn stop_code() -> u64 {
    STOP_CODE.load(Ordering::Relaxed)
}

use crate::tf_store::save as save_tf;
/// Bản sao trapframe của tác vụ vừa dừng (`Stop::Yield`/`Stop::Exit`).
pub use crate::tf_store::saved as saved_tf;

/// KT-70: API pager-fault ở [`crate::pager_fault`] (tách để giữ luật R1); `user` giữ đường dẫn cũ.
pub use crate::pager_fault::{
    pager_fault, pager_fault_count, pager_faults, set_pager_faults, PagerFault,
};

/// Chụp bảng dải VA của **ô đang nạp** — `exec` lưu trước khi gọi loader và khôi phục nếu loader
/// lỗi **muộn** (sau `reset()`), để ảnh cũ không chạy với bảng dải rác (D2-108B).
pub(crate) fn ranges_save() -> UserRanges {
    let saved = UserRanges::new();
    loading_ranges().snapshot_into(&saved);
    saved
}

/// Khôi phục bảng dải VA đã chụp bằng [`ranges_save`].
pub(crate) fn ranges_restore(saved: &UserRanges) {
    saved.snapshot_into(loading_ranges());
}

/// `pub(crate)` — hạ `sret` xuống `loaded` với trapframe `tf` và **trả về** khi tác vụ dừng
/// (`Yield`/`Exit`/`Halt`/`Fault`). Loại/mã dừng nằm ở [`stop_kind`]/[`stop_code`];
/// [`crate::user_run::run_task`] là cái bọc công khai (tách để giữ R1).
pub(crate) fn enter_loaded(loaded: &LoadedInit, tf: &mut TrapFrame) {
    enter_loaded_with_root(loaded, tf, None)
}

/// **KT-320** — như [`enter_loaded`], nhưng `satp` lấy root của **VSpace đã cấu hình** cho TCB
/// (`Some`) thay vì của ảnh nạp (`None`). Caller đã giải `ObjId → (root_pa, asid)` qua object
/// layer (D2-83: core **không** sở hữu PTE/PA) — xem `user_run::run_task`.
pub(crate) fn enter_loaded_with_root(
    loaded: &LoadedInit,
    tf: &mut TrapFrame,
    root: Option<(u64, u16)>,
) {
    let (root_pa, asid) = root.unwrap_or((loaded.root, loaded.asid));
    LAST_VSPACE.store(u32::from(loaded.vspace.0), Ordering::Relaxed);
    LAST_ASID.store(u64::from(asid), Ordering::Relaxed);
    USER_FAULT_RECOVER.store(0, Ordering::Relaxed);
    let current = read_sstatus();
    tf.sstatus = current & !(SSTATUS_SPP | SSTATUS_SPIE | SSTATUS_SIE | SSTATUS_SUM);
    USER_TASK.begin_run(tf.sstatus & SSTATUS_SPP != 0);
    USER_TASK.record_enter_sstatus(tf.sstatus);
    // KT-74: `tp` của lượt resume **luôn** lấy từ core (SetTls/exec) — không giữ giá trị cũ
    // trong trapframe của lần chạy trước.
    crate::user_tls::apply_tls(tf);
    STOP_KIND.store(0, Ordering::Release);
    if !aobj::switch_root(root_pa, asid) {
        let _ = aobj::restore_kernel_root();
        stop(3, 0);
        return;
    }
    crate::ipc_buffer::flush_pending(); // KT-282: `satp` vừa trỏ root user — ghi MR4..MR63.
                                        // SAFETY: trapframe do scheduler dựng từ lần nạp/trap trước; `user_enter` chỉ quay lại qua
                                        // `user_exit_resume` sau khi handler gọi `exit_to_kernel`.
                                        // **KT-418** — **một cửa** của ngữ cảnh FPU/SIMD: khôi phục `f0..f31`+`fcsr` của **đúng**
                                        // TCB này trước `sret` (khe chưa hợp lệ ⇒ `FS = Initial`, trạng thái mới). Đặt **sau**
                                        // `switch_root` để mọi đường vào U-mode (kể cả `vspace?` của KT-320) đi qua đúng một lần.
    crate::fpu::restore_current();
    unsafe { user_enter(tf as *const TrapFrame) };
    // **KT-418** — tác vụ vừa dừng: lưu `f0..f31`+`fcsr` vào khe của TCB **nếu** phần cứng báo
    // bẩn (`sstatus.FS = Dirty`); sạch ⇒ bỏ lưu (theo dõi bẩn miễn phí bằng phần cứng).
    crate::fpu::save_current();
    // KT-65: trả về S-mode với `satp` kernel để scheduler/physmap chạy.
    let _ = aobj::restore_kernel_root();
}

/// Trạng thái tác vụ user (TCB-like tối thiểu) — TAP đọc sau mỗi lượt.
pub static USER_TASK: UserTask = UserTask::new();

/// TCB **core** của `init` (KT-62) — `ecall` từ U-mode dispatch vào `Machine` thật của ảnh.
/// `u32::MAX` = chưa gắn (ảnh không dựng core ⇒ handler coi là lỗi nội bộ và halt, không "đường
/// tạm" nào được dựng lại).
static CORE_TCB: AtomicU32 = AtomicU32::new(u32::MAX);

/// Gắn TCB core cho tác vụ U-mode của ảnh này (KT-62) — gọi **trước** [`enter`].
pub fn bind_core_tcb(tcb: ObjId) {
    CORE_TCB.store(tcb, Ordering::Release);
    // **KT-418** — cùng một nguồn "TCB đang chạy" cho ngữ cảnh FPU/SIMD (module `fpu` biên dịch
    // được cả ở ảnh **không** bật feature `user`, nên nó giữ con trỏ riêng thay vì đọc `CORE_TCB`).
    crate::fpu::bind_current(tcb);
}

/// Số syscall tối đa ghi sổ theo số hiệu (ABI `0..15`) — KT-62.
pub const SYS_MAX: usize = 16;
/// Số lần gọi theo số hiệu (KT-62) — TAP ảnh `ktest-user` đọc sau lượt U-mode.
static SYS_CALLS: [AtomicU64; SYS_MAX] = [const { AtomicU64::new(0) }; SYS_MAX];
/// `a0` (error) lần gần nhất theo số hiệu.
static SYS_ERR: [AtomicU64; SYS_MAX] = [const { AtomicU64::new(0) }; SYS_MAX];
/// `a1` (value) lần gần nhất theo số hiệu.
static SYS_VAL: [AtomicU64; SYS_MAX] = [const { AtomicU64::new(0) }; SYS_MAX];

/// Ghi sổ **một** syscall đã dispatch (KT-62) — bằng chứng `# sys:`/TAP, không đổi ngữ nghĩa.
pub fn record_sys(nr: u64, error: u64, value: u64) {
    if let Ok(index) = usize::try_from(nr) {
        if index < SYS_MAX {
            SYS_CALLS[index].fetch_add(1, Ordering::Relaxed);
            SYS_ERR[index].store(error, Ordering::Relaxed);
            SYS_VAL[index].store(value, Ordering::Relaxed);
        }
    }
}

/// Số lần syscall `nr` đã dispatch.
pub fn sys_calls(nr: usize) -> u64 {
    SYS_CALLS
        .get(nr)
        .map(|v| v.load(Ordering::Relaxed))
        .unwrap_or(0)
}

/// `a0` lần gần nhất của syscall `nr`.
pub fn sys_err(nr: usize) -> u64 {
    SYS_ERR
        .get(nr)
        .map(|v| v.load(Ordering::Relaxed))
        .unwrap_or(0)
}

/// `a1` lần gần nhất của syscall `nr`.
pub fn sys_val(nr: usize) -> u64 {
    SYS_VAL
        .get(nr)
        .map(|v| v.load(Ordering::Relaxed))
        .unwrap_or(0)
}

/// TCB core đã gắn (KT-62).
pub fn core_tcb() -> Option<ObjId> {
    let value = CORE_TCB.load(Ordering::Acquire);
    if value == u32::MAX {
        None
    } else {
        Some(value)
    }
}

pub use crate::user_run::{apply_ipc_return, run_task, Stop};
/// **KT-74** — `Stop`/`run_task` chuyển sang [`crate::user_run`] cho R1; tái xuất để ảnh test
/// giữ nguyên đường dẫn `crate::user::{Stop, run_task}`.
use crate::user_run::{is_kernel_addr, kernel_ctx_sp};
/// **KT-74** — API TLS/`exec` giữ **cùng đường dẫn** `crate::user::*` cho ảnh test; cài đặt nằm ở
/// module riêng [`crate::user_tls`] để giữ luật R1 (≤ 1.500 dòng/tệp).
pub use crate::user_tls::{apply_tls, exec_frame, record_tls_trap, tls_observed, tls_trap_count};

/// Lỗi parse ELF mà `LoadError::Elf` mang — tái xuất để ảnh test so **đúng biến thể** (ca
/// "exec thất bại nguyên tử" của KT-74) mà không phải thêm dependency `vifast-elf` cho kbin.
pub use vifast_elf::ElfError as ElfParseError;

// ---------------------------------------------------------------------------------------------
// **KT-342b (D2-243)** — `user.rs` đã tách theo **miền** để giữ trần R1 (≤ 1.500 dòng/tệp; mục
// tiêu < 1.100). Đường dẫn **công khai không đổi** (`crate::user::load_init`, `crate::user::LoadError`,
// `crate::user::UserMemAccess`, `crate::user::on_u_exception`, …) nhờ các `pub use` dưới đây:
// mô-đun con `use super::*` nên vẫn thấy hằng/trạng thái private của tệp này như trước.
// ---------------------------------------------------------------------------------------------

pub mod load;
pub mod mem;
pub mod trap;

pub(crate) use load::write_page;
pub use load::{load_init, load_init_slot, load_init_slot_with_stack, LoadError, USER_STACK_PAGES};
pub(crate) use mem::{user_range_mapped, UserMemAccess};
pub use trap::on_u_exception;
pub(crate) use trap::read_sstatus;

/// Hạ `sret` xuống U-mode tại `entry` với `sp = USER_STACK_TOP`; trả về **chỉ khi** `init` gọi
/// `Exit` (đường `user_exit_resume` khôi phục ngữ cảnh kernel).
///
/// `mode` được đặt vào `a0` — chương trình `init` dùng để chọn lượt chạy (0 = syscall, 1 = cố
/// đọc địa chỉ kernel cho TAP cách ly).
pub fn enter(loaded: &LoadedInit, mode: u64) -> Result<UserSnapshot, LoadError> {
    enter_with(loaded, mode, false)
}

/// Như [`enter`] nhưng **cho phép một fault** U-mode (TAP `faultkind vm load`): handler phân loại
/// `FaultKind` + ghi bằng chứng rồi trả về S-mode. Chỉ ảnh test dùng; chính sách mặc định của K3
/// vẫn là fatal (`enter`).
pub fn enter_expect_fault(loaded: &LoadedInit, mode: u64) -> Result<UserSnapshot, LoadError> {
    enter_with(loaded, mode, true)
}

fn enter_with(
    loaded: &LoadedInit,
    mode: u64,
    expect_fault: bool,
) -> Result<UserSnapshot, LoadError> {
    LAST_VSPACE.store(u32::from(loaded.vspace.0), Ordering::Relaxed);
    LAST_ASID.store(u64::from(loaded.asid), Ordering::Relaxed);
    USER_FAULT_RECOVER.store(u32::from(expect_fault), Ordering::Relaxed);
    let mut tf = TrapFrame::zeroed();
    tf.x[REG_SP] = USER_STACK_TOP;
    // `a0` = mode; các thanh ghi khác 0 (không lộ địa chỉ kernel cho user).
    tf.x[REG_A0] = mode;
    tf.sepc = loaded.entry;
    // KT-74: TLS từ core (mặc định 0 khi chưa `SetTls`).
    crate::user_tls::apply_tls(&mut tf);
    let current = read_sstatus();
    // U-mode: SPP=0; tắt SIE/SPIE/SUM (không cho ngắt S-mode chen vào U-phase; syscall không
    // cần ngắt). Giữ nguyên UXL/FS và các bit WARL khác của sstatus.
    tf.sstatus = current & !(SSTATUS_SPP | SSTATUS_SPIE | SSTATUS_SIE | SSTATUS_SUM);
    USER_TASK.begin_run(tf.sstatus & SSTATUS_SPP != 0);
    USER_TASK.record_enter_sstatus(tf.sstatus);
    if !aobj::switch_root(loaded.root, loaded.asid) {
        let _ = aobj::restore_kernel_root();
        return Err(LoadError::Satp);
    }
    // SAFETY: trapframe hợp lệ (sepc trong segment X, sp trong stack đã map U); `user_enter`
    // lưu ngữ cảnh kernel, nạp CSR + 31 thanh ghi từ frame rồi `sret`. Chỉ quay lại qua
    // `user_exit_resume` (Exit) hoặc không quay lại (fault ⇒ halt).
    unsafe { user_enter(&tf as *const TrapFrame) };
    // KT-65: `Exit`/fault trả về S-mode **vẫn đang** `satp` = root user (không có physmap).
    // Khôi phục root kernel trước khi phần còn lại của kernel chạm object layer/physmap.
    let _ = aobj::restore_kernel_root();
    Ok(USER_TASK.snapshot())
}

// ---------------------------------------------------------------------------------------------
// KT-65 — **stack riêng cho dispatch syscall** (không dùng trap stack 16 KiB của KT-49)
// ---------------------------------------------------------------------------------------------

// KT-66: stack per-hart + canary/nước thấp nằm ở module riêng `user_stack` (R1 ≤ 1.500 dòng);
// phần dưới chỉ còn scratch một-lượt + đổi `sp` sang block của hart hiện tại.
#[repr(C)]
struct SavedSp([u64; 2]);

/// Hart đang chạy dispatch — asm đọc lại giá trị này ở cả hai phía `call`.
static mut STACK_HART: usize = 0;
/// `sp`/`ra` kernel lưu tạm **theo hart** trong lúc chạy dispatch trên stack riêng.
static mut SYSCALL_SAVED_SP: [SavedSp; MAX_HARTS] = [const { SavedSp([0; 2]) }; MAX_HARTS];
/// Tham số/`tcb` của lượt dispatch đang chạy trên stack riêng (một hart, không lồng nhau).
static mut SYSCALL_TCB: u32 = 0;
static mut SYSCALL_NR: u64 = 0;
static mut SYSCALL_ARGS: [u64; 6] = [0; 6];
/// Kết quả dispatch (kiểu `Copy`).
static mut SYSCALL_ACTION: MaybeUninit<SyscallAction> = MaybeUninit::uninit();

/// Dispatch một syscall U-mode trên **stack riêng của hart hiện tại** (xem
/// [`crate::user_stack::init_syscall_stacks`]). Sau khi về, kiểm canary đáy block ngay (tràn ⇒
/// `canary=1`).
fn dispatch_on_private_stack(tcb: ObjId, nr: u64, args: [u64; 6]) -> SyscallAction {
    let hart = crate::smp::current_hart().min(MAX_HARTS - 1);
    let top = crate::user_stack::syscall_stack_top(hart);
    if top == 0 {
        let mut w = PlatformWriter(&Riscv64Virt);
        let _ = writeln!(w, "# stack: chua cap cho hart={hart} (init_syscall_stacks)");
        vifast_plat_common::Platform::put_str(&Riscv64Virt, "PANIC: syscall stack chua cap\n");
        crate::debug_halt(1)
    }
    // SAFETY: ghi tham số + hart vào static trước khi đổi stack; một hart, không lồng nhau.
    unsafe {
        core::ptr::write(core::ptr::addr_of_mut!(SYSCALL_TCB), tcb);
        core::ptr::write(core::ptr::addr_of_mut!(SYSCALL_NR), nr);
        core::ptr::write(core::ptr::addr_of_mut!(SYSCALL_ARGS), args);
        core::ptr::write(core::ptr::addr_of_mut!(STACK_HART), hart);
    }
    // SAFETY: đổi `sp` sang block **của hart này** (16 B aligned, NOLOAD identity), lưu `sp`/`ra`
    // vào `SYSCALL_SAVED_SP[hart]`, `call` hàm Rust (giữ callee-saved theo ABI), rồi khôi phục
    // `ra`/`sp`. Chỉ dùng `t0/t1` (caller-saved, đọc lại `STACK_HART` sau `call` — không giữ giá
    // trị trong thanh ghi qua lời gọi); `clobber_abi("C")` khai báo đúng các thanh ghi bị clobber.
    unsafe {
        asm!(
            "la t0, {hart}",
            "ld t0, 0(t0)",
            "slli t0, t0, 4",
            "la t1, {saved}",
            "add t0, t0, t1",
            "sd sp, 0(t0)",
            "sd ra, 8(t0)",
            "mv sp, {top}",
            "call {f}",
            "la t0, {hart}",
            "ld t0, 0(t0)",
            "slli t0, t0, 4",
            "la t1, {saved}",
            "add t0, t0, t1",
            "ld ra, 8(t0)",
            "ld sp, 0(t0)",
            hart = sym STACK_HART,
            saved = sym SYSCALL_SAVED_SP,
            top = in(reg) top,
            f = sym syscall_on_private_stack,
            clobber_abi("C"),
            options(nostack),
        );
        crate::user_stack::check_stack_canary(hart);
        (*core::ptr::addr_of!(SYSCALL_ACTION)).assume_init()
    }
}

/// Chạy trên stack riêng: gọi đúng façade [`abi::dispatch`] như trước.
extern "C" fn syscall_on_private_stack() {
    // SAFETY: các static chỉ được ghi trước khi chuyển stack và đọc trong cùng lượt, một hart;
    // dùng `ptr::read`/`ptr::write` để **không** tạo tham chiếu tới `static mut` (Rust 2024).
    let (tcb, nr, args) = unsafe {
        (
            core::ptr::read(core::ptr::addr_of!(SYSCALL_TCB)),
            core::ptr::read(core::ptr::addr_of!(SYSCALL_NR)),
            core::ptr::read(core::ptr::addr_of!(SYSCALL_ARGS)),
        )
    };
    let action = crate::core_bridge::with_machine(|m| {
        abi::dispatch(m, tcb, 0, nr, args, &mut UserMemAccess, &USER_TASK)
    });
    // SAFETY: một người ghi (hart hiện tại), chưa ai đọc cho tới khi quay lại stack cũ.
    unsafe {
        core::ptr::write(
            core::ptr::addr_of_mut!(SYSCALL_ACTION),
            MaybeUninit::new(action),
        );
    }
}

// Ngữ cảnh kernel (ra/sp/gp/tp/s0..s11) được `user_enter` lưu và `user_exit_resume` khôi phục.
extern "C" {
    static USER_KERNEL_CTX: [u64; 16];
    /// Hạ `sret` xuống U-mode theo trapframe tại `a0`; chỉ quay lại qua [`user_exit_resume`].
    fn user_enter(frame: *const TrapFrame);
    /// Đường về S-mode sau `Exit`: khôi phục ngữ cảnh kernel rồi `ret` về caller Rust.
    fn user_exit_resume();
}

// Trapframe x0..x31 + sepc/sstatus/scause/stval (288 B) — asm nạp `sepc/sstatus` và 31 thanh ghi
// (x2 = sp CUỐI CÙNG qua `ld sp, {x2}(a0)`), phần còn lại của frame không bị asm đọc.
global_asm!(
    ".section .bss",
    ".balign 8",
    ".globl USER_KERNEL_CTX",
    "USER_KERNEL_CTX:",
    ".space 128",
    // ------------------------------------------------------------------ vào U-mode
    ".section .text",
    ".balign 4",
    ".globl user_enter",
    "user_enter:",
    // a0 = &TrapFrame. Lưu ngữ cảnh kernel (ra/sp/gp/tp/s0..s11) vào ctx tĩnh trước khi đụng
    // bất cứ thanh ghi callee-saved nào.
    "la t0, USER_KERNEL_CTX",
    "sd ra, 0(t0)",
    "sd sp, 8(t0)",
    "sd gp, 16(t0)",
    "sd tp, 24(t0)",
    "sd s0, 32(t0)",
    "sd s1, 40(t0)",
    "sd s2, 48(t0)",
    "sd s3, 56(t0)",
    "sd s4, 64(t0)",
    "sd s5, 72(t0)",
    "sd s6, 80(t0)",
    "sd s7, 88(t0)",
    "sd s8, 96(t0)",
    "sd s9, 104(t0)",
    "sd s10, 112(t0)",
    "sd s11, 120(t0)",
    // CSR từ frame (a0 làm base). t0 (x5) được khôi phục sau các lệnh csrw.
    "ld t0, {sstatus}(a0)",
    "csrw sstatus, t0",
    "ld t0, {sepc}(a0)",
    "csrw sepc, t0",
    // Khôi phục 31 thanh ghi từ frame; x2 (sp) và x10 (a0) cuối cùng.
    "ld ra, {x1}(a0)",
    "ld gp, {x3}(a0)",
    "ld tp, {x4}(a0)",
    "ld t0, {x5}(a0)",
    "ld t1, {x6}(a0)",
    "ld t2, {x7}(a0)",
    "ld s0, {x8}(a0)",
    "ld s1, {x9}(a0)",
    "ld a1, {x11}(a0)",
    "ld a2, {x12}(a0)",
    "ld a3, {x13}(a0)",
    "ld a4, {x14}(a0)",
    "ld a5, {x15}(a0)",
    "ld a6, {x16}(a0)",
    "ld a7, {x17}(a0)",
    "ld s2, {x18}(a0)",
    "ld s3, {x19}(a0)",
    "ld s4, {x20}(a0)",
    "ld s5, {x21}(a0)",
    "ld s6, {x22}(a0)",
    "ld s7, {x23}(a0)",
    "ld s8, {x24}(a0)",
    "ld s9, {x25}(a0)",
    "ld s10, {x26}(a0)",
    "ld s11, {x27}(a0)",
    "ld t3, {x28}(a0)",
    "ld t4, {x29}(a0)",
    "ld t5, {x30}(a0)",
    "ld t6, {x31}(a0)",
    "ld sp, {x2}(a0)",
    "ld a0, {x10}(a0)",
    "sret",
    // ------------------------------------------------------------------ về S-mode sau Exit
    ".balign 4",
    ".globl user_exit_resume",
    "user_exit_resume:",
    // `sscratch` vẫn là đỉnh trap stack per-hart (trap_entry vừa đặt lại); khôi phục ngữ cảnh
    // kernel rồi `ret` về caller Rust của `user_enter` (a0/ra không cần đúng ABI cho caller).
    "la t0, USER_KERNEL_CTX",
    "ld ra, 0(t0)",
    "ld gp, 16(t0)",
    "ld tp, 24(t0)",
    "ld s0, 32(t0)",
    "ld s1, 40(t0)",
    "ld s2, 48(t0)",
    "ld s3, 56(t0)",
    "ld s4, 64(t0)",
    "ld s5, 72(t0)",
    "ld s6, 80(t0)",
    "ld s7, 88(t0)",
    "ld s8, 96(t0)",
    "ld s9, 104(t0)",
    "ld s10, 112(t0)",
    "ld s11, 120(t0)",
    "ld sp, 8(t0)",
    "ret",
    sstatus = const TF_SSTATUS_OFFSET,
    sepc = const TF_SEPC_OFFSET,
    x1 = const tf_reg_offset(1),
    x2 = const tf_reg_offset(2),
    x3 = const tf_reg_offset(3),
    x4 = const tf_reg_offset(4),
    x5 = const tf_reg_offset(5),
    x6 = const tf_reg_offset(6),
    x7 = const tf_reg_offset(7),
    x8 = const tf_reg_offset(8),
    x9 = const tf_reg_offset(9),
    x10 = const tf_reg_offset(10),
    x11 = const tf_reg_offset(11),
    x12 = const tf_reg_offset(12),
    x13 = const tf_reg_offset(13),
    x14 = const tf_reg_offset(14),
    x15 = const tf_reg_offset(15),
    x16 = const tf_reg_offset(16),
    x17 = const tf_reg_offset(17),
    x18 = const tf_reg_offset(18),
    x19 = const tf_reg_offset(19),
    x20 = const tf_reg_offset(20),
    x21 = const tf_reg_offset(21),
    x22 = const tf_reg_offset(22),
    x23 = const tf_reg_offset(23),
    x24 = const tf_reg_offset(24),
    x25 = const tf_reg_offset(25),
    x26 = const tf_reg_offset(26),
    x27 = const tf_reg_offset(27),
    x28 = const tf_reg_offset(28),
    x29 = const tf_reg_offset(29),
    x30 = const tf_reg_offset(30),
    x31 = const tf_reg_offset(31),
);
