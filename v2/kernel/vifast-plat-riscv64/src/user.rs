// SPDX-License-Identifier: MIT OR Apache-2.0
//! U-mode trên QEMU `virt` (KT-53) — phần **không viết "safe" được** của
//! [`vifast_plat_common::user`]: nạp `init` ELF vào VSpace thật, dựng trapframe U-mode, `sret`
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
    SyscallEnv, FEATURE_DEBUG_HALT, FEATURE_DEBUG_PUT_STR, FEATURE_MCS_YIELD, FEATURE_WAIT_ADDR,
};
use vifast_core::{ObjId, TcbState};
use vifast_elf::ElfError;
use vifast_plat_common::fault::{self, FaultKind, PteFacts, TrapFacts, FAULT_STATS};
use vifast_plat_common::mmu::{PTE_EXEC, PTE_READ, PTE_USER, PTE_WRITE};
use vifast_plat_common::obj::{CapId, MapFlags, ObjError, ObjKind};
use vifast_plat_common::smp::MAX_HARTS;
use vifast_plat_common::trap::{
    tf_reg_offset, Exception, ExceptionKind, TrapFrame, REG_A0, REG_A7, REG_SP, SSTATUS_SIE,
    SSTATUS_SPIE, SSTATUS_SPP, TF_SEPC_OFFSET, TF_SSTATUS_OFFSET,
};
use vifast_plat_common::user::{
    self as abi, SyscallAction, UserRange, UserSnapshot, UserTask, ERR_INVALID_ARGUMENT,
    ERR_NOT_MAPPED,
};
use vifast_plat_common::{Platform, PlatformWriter};

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
pub const USER_STACK_TOP: u64 = 0x4001_0000;
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
    LAST_VSPACE.store(u32::from(loaded.vspace.0), Ordering::Relaxed);
    LAST_ASID.store(u64::from(loaded.asid), Ordering::Relaxed);
    USER_FAULT_RECOVER.store(0, Ordering::Relaxed);
    let current = read_sstatus();
    tf.sstatus = current & !(SSTATUS_SPP | SSTATUS_SPIE | SSTATUS_SIE | SSTATUS_SUM);
    USER_TASK.begin_run(tf.sstatus & SSTATUS_SPP != 0);
    USER_TASK.record_enter_sstatus(tf.sstatus);
    // KT-74: `tp` của lượt resume **luôn** lấy từ core (SetTls/exec) — không giữ giá trị cũ
    // trong trapframe của lần chạy trước.
    crate::user_tls::apply_tls(tf);
    STOP_KIND.store(0, Ordering::Release);
    if !aobj::switch_root(loaded.root, loaded.asid) {
        let _ = aobj::restore_kernel_root();
        stop(3, 0);
        return;
    }
    crate::ipc_buffer::flush_pending(); // KT-282: `satp` vừa trỏ root user — ghi MR4..MR63.
                                        // SAFETY: trapframe do scheduler dựng từ lần nạp/trap trước; `user_enter` chỉ quay lại qua
                                        // `user_exit_resume` sau khi handler gọi `exit_to_kernel`.
    unsafe { user_enter(tf as *const TrapFrame) };
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

/// **KT-74** — `Stop`/`run_task` chuyển sang [`crate::user_run`] cho R1; tái xuất để ảnh test
/// giữ nguyên đường dẫn `crate::user::{Stop, run_task}`.
pub use crate::user_run::{apply_ipc_return, run_task, Stop};
/// **KT-74** — API TLS/`exec` giữ **cùng đường dẫn** `crate::user::*` cho ảnh test; cài đặt nằm ở
/// module riêng [`crate::user_tls`] để giữ luật R1 (≤ 1.500 dòng/tệp).
pub use crate::user_tls::{apply_tls, exec_frame, record_tls_trap, tls_observed, tls_trap_count};

/// Lỗi parse ELF mà `LoadError::Elf` mang — tái xuất để ảnh test so **đúng biến thể** (ca
/// "exec thất bại nguyên tử" của KT-74) mà không phải thêm dependency `vifast-elf` cho kbin.
pub use vifast_elf::ElfError as ElfParseError;

/// Lỗi nạp `init` — đủ để TAP in/log, **không panic** trong đường nạp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// ELF không hợp lệ (`vifast-elf`).
    Elf(ElfError),
    /// Retype/map qua object layer.
    Obj(ObjError),
    /// PA của Frame không nằm trong physmap (RAM 128 MiB).
    Physmap,
    /// Ảnh ELF chạm vùng guard/stack user.
    StackOverlap,
    /// Segment `W+X` (loader kiểm lại ngoài parser).
    Wx,
    /// File offset/độ dài segment ngoài buffer ELF.
    Image,
    /// `satp` không đổi được sang root user.
    Satp,
}

impl LoadError {
    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Elf(_) => "Elf",
            Self::Obj(_) => "Obj",
            Self::Physmap => "Physmap",
            Self::StackOverlap => "StackOverlap",
            Self::Wx => "Wx",
            Self::Image => "Image",
            Self::Satp => "Satp",
        }
    }
}

/// Nạp `init` ELF vào một VSpace mới: parse `vifast-elf`, map segment **theo quyền**, map stack
/// user + để hở trang guard, rồi `check_objmem`.
///
/// `untyped` là cap Untyped chứa arena/PA cho VSpace/Frame (KT-52).
pub fn load_init(bytes: &[u8], untyped: CapId) -> Result<LoadedInit, LoadError> {
    load_init_slot(bytes, untyped, 0)
}

/// Như [`load_init`] nhưng ghi bảng dải user vào **ô tác vụ** `slot` (KT-65: nhiều chương trình
/// U-mode trong cùng ảnh test, mỗi chương trình một VSpace).
pub fn load_init_slot(bytes: &[u8], untyped: CapId, slot: usize) -> Result<LoadedInit, LoadError> {
    load_init_slot_with_stack(bytes, untyped, slot, USER_STACK_PAGES)
}

/// Số trang stack user mặc định (8 trang = 32 KiB).
pub const USER_STACK_PAGES: usize = (USER_STACK_BYTES / 4096) as usize;

/// Như [`load_init_slot`] nhưng chọn số trang stack — KT-157 dùng 1 trang cho client C/D.
pub fn load_init_slot_with_stack(
    bytes: &[u8],
    untyped: CapId,
    slot: usize,
    stack_pages: usize,
) -> Result<LoadedInit, LoadError> {
    set_load_slot(slot);
    let image = vifast_elf::parse(bytes, EM_RISCV, USER_BASE, USER_TOP).map_err(LoadError::Elf)?;
    // Ảnh không được chạm guard/stack user; guard nằm ngay dưới stack.
    let mut image_end = USER_BASE;
    for segment in image.segments() {
        let end = segment.memory_end().ok_or(LoadError::Image)?;
        if end > image_end {
            image_end = end;
        }
        if segment.writable() && segment.executable() {
            return Err(LoadError::Wx);
        }
    }
    if image_end > USER_STACK_GUARD_BOTTOM {
        return Err(LoadError::StackOverlap);
    }

    let m = aobj::machine();
    let vspace = m
        .retype(untyped, ObjKind::VSpace, 0)
        .map_err(LoadError::Obj)?;
    // KT-74: danh sách trang của lần nạp **này** — khai báo ngay sau VSpace để **mọi** đường
    // lỗi phía dưới dùng được `bail!` (trả lại Frame/VSpace, không mồ côi).
    let mut init_pages = crate::init_pages::InitPages::new();
    // KT-74 (atomic của `exec`): từ đây **mọi** đường lỗi phải trả Frame/VSpace vừa cấp — nếu
    // không, `exec` hỏng để lại tài nguyên mồ côi và phép đo "objects/frames/caps bằng nhau"
    // của `exec` sẽ sai. `parse` + kiểm W^X/overlap ở **trên** đã chạy trước khi cấp bất cứ thứ
    // gì, nên lỗi hình dạng ELF không bao giờ tới đây; các lỗi còn lại (Obj/Physmap/Image) đều
    // đi qua `bail!`.
    macro_rules! bail {
        ($pages:expr, $e:expr) => {{
            // `m` đang giữ ⇒ dùng `release_with` (không gọi `aobj::machine()` lần hai). Truyền
            // `$pages` qua **đối số macro** vì macro hygiene: `init_pages` khai báo sau macro.
            crate::init_pages::release_with(m, vspace, &$pages);
            return Err($e);
        }};
    }
    let root = match m.vspace_root_pa(vspace) {
        Ok(root) => root,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    let asid = match m.vspace_asid(vspace) {
        Ok(asid) => asid,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    // Chỉ chép **nửa thấp** root kernel (index 0..256 trừ cửa sổ user index 1) để kernel sống
    // khi `satp` trỏ root user; **không** chép nửa cao (high alias/physmap) — U-mode không có
    // đường PTE nào tới đó, và syscall không được dùng physmap sau khi đổi `satp`.
    if !aobj::copy_kernel_low_root(root, USER_ROOT_INDEX) {
        bail!(init_pages, LoadError::Physmap);
    }
    loading_ranges().reset();

    let mut text_pages = 0usize;
    let mut ro_pages = 0usize;
    let mut data_pages = 0usize;
    for segment in image.segments() {
        let flags = MapFlags {
            read: true,
            write: segment.writable(),
            exec: segment.executable(),
            user: true,
            device: false,
        };
        let first_page = segment.vaddr & !0xfff;
        let pages = segment.page_count();
        let file_end = segment.vaddr + segment.file_size as u64;
        for index in 0..pages {
            let va = first_page + (index as u64) * 4096;
            let frame = match m.retype(untyped, ObjKind::Frame, 12) {
                Ok(frame) => frame,
                Err(e) => bail!(init_pages, LoadError::Obj(e)),
            };
            let pa = match m.frame_pa(frame) {
                Ok(pa) => pa,
                Err(e) => bail!(init_pages, LoadError::Obj(e)),
            };
            // Phần file rơi vào trang này (phần còn lại zero-fill).
            let page_end = va + 4096;
            let copy_start = va.max(segment.vaddr);
            let copy_end = page_end.min(file_end);
            let (file_start, copy_len) = if copy_end > copy_start {
                let offset = segment.file_offset as u64 + (copy_start - segment.vaddr);
                let len = copy_end - copy_start;
                let start = usize::try_from(offset).map_err(|_| LoadError::Image)?;
                let end = usize::try_from(offset + len).map_err(|_| LoadError::Image)?;
                if end > bytes.len() {
                    bail!(init_pages, LoadError::Image);
                }
                (start, len as usize)
            } else {
                (0, 0)
            };
            let data = &bytes[file_start..file_start + copy_len];
            if !write_page(pa, data) {
                bail!(init_pages, LoadError::Physmap);
            }
            if let Err(e) = m.map(vspace, va, frame, flags) {
                bail!(init_pages, LoadError::Obj(e));
            }
            if !init_pages.push(va, frame) {
                bail!(init_pages, LoadError::Image);
            }
            if index == 0 && !loading_ranges().push(va, va + (pages as u64) * 4096) {
                bail!(init_pages, LoadError::Image);
            }
            if segment.executable() {
                text_pages += 1;
            } else if segment.writable() {
                data_pages += 1;
            } else {
                ro_pages += 1;
            }
        }
    }

    // Stack user: `stack_pages` trang RW NX; trang ngay dưới (guard) **không map**.
    let stack_pages =
        match crate::init_pages::map_user_stack(m, vspace, untyped, stack_pages, &mut init_pages) {
            Ok(pages) => pages,
            Err(e) => bail!(init_pages, e),
        };

    // **KT-67b** — trang **IPC buffer** (một Frame 4 KiB, `R|W|U|NX`, zero):
    // `spec/abi.toml` dòng 58 chốt header IPC buffer chứa receive window (CPTR, ≤4 cap); shim
    // U-mode + kernel đọc/ghi cùng VA cố định này. Nằm trong cùng bảng cấp 1 với ảnh/stack
    // (0x4002_0000 ⇒ level-1 index 0) nên **không** cần bảng trang mới.
    let ipc_frame = match m.retype(untyped, ObjKind::Frame, 12) {
        Ok(frame) => frame,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    let ipc_pa = match m.frame_pa(ipc_frame) {
        Ok(pa) => pa,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    if !write_page(ipc_pa, &[]) {
        bail!(init_pages, LoadError::Physmap);
    }
    let ipc_va = crate::ipc_buffer::USER_IPC_BUFFER_VA;
    if let Err(e) = m.map(vspace, ipc_va, ipc_frame, MapFlags::RW_NX_USER) {
        bail!(init_pages, LoadError::Obj(e));
    }
    if !init_pages.push(ipc_va, ipc_frame) {
        bail!(init_pages, LoadError::Image);
    }
    if !loading_ranges().push(ipc_va, ipc_va + 4096) {
        bail!(init_pages, LoadError::Image);
    }

    // Đi lại PTE thật để chứng minh quyền leaf (không tin cờ lúc map): `W+X` = 0, text có X,
    // data có W, mọi leaf user có `U`; đây là bằng chứng `# init load: … wx=0 flags=1`.
    let mut wx_pages = 0usize;
    let mut flags_ok = true;
    for segment in image.segments() {
        let mut va = segment.vaddr & !0xfff;
        for _ in 0..segment.page_count() {
            let walk = match m.walk(vspace, va) {
                Ok(walk) => walk,
                Err(e) => bail!(init_pages, LoadError::Obj(e)),
            };
            let raw = walk.raw;
            let read = raw & PTE_READ != 0;
            let write = raw & PTE_WRITE != 0;
            let exec = raw & PTE_EXEC != 0;
            let user = raw & PTE_USER != 0;
            if write && exec {
                wx_pages += 1;
            }
            let shape_ok = walk.valid && walk.leaf && walk.level == 0;
            let expected = shape_ok
                && read
                && user
                && write == segment.writable()
                && exec == segment.executable();
            if !expected {
                flags_ok = false;
            }
            va += 4096;
        }
    }

    // Nửa cao phải **không** map: walk high alias kernel + physmap đều invalid.
    let high_alias_off = m
        .walk(vspace, mmu::KERNEL_HIGH_BASE)
        .map(|walk| !walk.valid)
        .unwrap_or(false);
    let physmap_off = m
        .walk(vspace, mmu::PHYSMAP_BASE + mmu::RAM_PHYS_BASE)
        .map(|walk| !walk.valid)
        .unwrap_or(false);
    let high_half_unmapped = high_alias_off && physmap_off;
    let guard_unmapped = m
        .walk(vspace, USER_STACK_GUARD_BOTTOM)
        .map(|walk| !walk.valid)
        .unwrap_or(false);

    let flush = m.drain_flushes(aobj::run_flush);
    let check_ok = m.check_objmem().is_ok();
    let _ = flush;
    Ok(LoadedInit {
        vspace,
        root,
        asid,
        entry: image.entry,
        segments: image.segment_count(),
        text_pages,
        ro_pages,
        data_pages,
        stack_pages,
        image_end,
        stack_guard: USER_STACK_GUARD_BOTTOM,
        check_ok,
        wx_pages,
        flags_ok,
        high_half_unmapped,
        guard_unmapped,
        pages: init_pages,
    })
}

/// Ghi một trang 4 KiB: zero toàn trang rồi chép `data` (≤ 4096) vào đầu trang.
///
/// # Safety
/// Logic: `pa` do object layer cấp trong cửa sổ PA của Untyped, `mmu::physmap_alias` chỉ trả
/// `Some` khi PA nằm trong RAM 128 MiB đã map — nên 4 KiB ghi là hợp lệ.
pub(crate) fn write_page(pa: u64, data: &[u8]) -> bool {
    if data.len() > 4096 {
        return false;
    }
    let Some(va) = mmu::physmap_alias(pa) else {
        return false;
    };
    // SAFETY: `va` là alias physmap của RAM thật (xem trên); ghi 4 KiB + chép `data`.
    unsafe {
        core::ptr::write_bytes(va as *mut u8, 0, 4096);
        if !data.is_empty() {
            core::ptr::copy_nonoverlapping(data.as_ptr(), va as *mut u8, data.len());
        }
    }
    true
}

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
    vifast_plat_common::trap::report_u_fault(&Riscv64Virt, exc, tf)
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

/// Địa chỉ trong vùng ảnh kernel (identity 2 MiB đầu) — dùng cho bằng chứng "kernel_addr" của
/// fault cách ly. Không phải kiểm an ninh (an ninh do PTE `U=0` quyết định).
fn is_kernel_addr(addr: u64) -> bool {
    (mmu::KERNEL_PHYS_BASE..mmu::KERNEL_PHYS_BASE + 0x0020_0000).contains(&addr)
}

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
    fn invoke(
        &mut self,
        op: vifast_core::invoke::InvokeOp,
    ) -> Result<vifast_core::invoke::InvokeOutcome, u64> {
        // Trong syscall U-mode, `satp` vẫn trỏ **root user** (không có physmap — xem
        // `resolve_addr`): object layer chỉ chạm được sau khi khôi phục root kernel, rồi phải
        // trả lại root user trước khi `sret` về U-mode. Cùng khuôn `resolve_addr`.
        with_kernel_root(|| crate::mem_bridge::invoke(op)).and_then(|result| {
            result.map_err(|error| {
                error
                    .abi_code()
                    .map(u64::from)
                    .unwrap_or(u64::from(vifast_core::KError::InvalidArgument.id()))
            })
        })
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

/// `sp` kernel đã lưu lúc vào U-mode (dùng làm `sp` tạm trước khi `user_exit_resume` nạp lại).
fn kernel_ctx_sp() -> u64 {
    // SAFETY: `USER_KERNEL_CTX` do asm định nghĩa trong `.bss`; chỉ đọc phần tử `sp`. Một hart ở
    // KT-53 nên không có ghi đồng thời.
    unsafe { USER_KERNEL_CTX[1] }
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
