// SPDX-License-Identifier: MIT OR Apache-2.0
//! Object layer **trên QEMU `virt`** (KT-52) — phần không viết "safe" được của
//! [`vifast_plat_common::obj`]:
//!
//! - [`PhysmapMem`] cài [`PhysMem`] bằng **physmap** (`PHYSMAP_BASE + PA`, leaf 2 MiB NX của
//!   KT-46) — object arena và PTE nằm ngoài identity mapping, mọi truy cập đi qua alias này;
//! - [`init`] lấy arena thật **sau `__kernel_end`** (sau arena page table của KT-46, không phải
//!   section ELF nên không vào trần `.data+.bss`, D2-72 Q5) và dựng **2 Untyped boot**:
//!   `U0` = 64 KiB arena + cửa sổ PA 1 GiB căn 2 MiB, `U1` = 4 KiB arena + cửa sổ PA 1 GiB căn
//!   1 GiB (cho Frame 1 GiB — D2-66);
//! - `#[cfg(any(feature = "ktest", feature = "umode"))]`: [`copy_kernel_root`] chép root kernel vào root VSpace (giữ
//!   kernel sống khi đổi `satp`), [`switch_root`]/[`restore_kernel_root`] ghi `satp` + `sfence`,
//!   và [`va_read_u64_sum`]/[`va_write_u64_sum`] đọc/ghi VA **user** từ S-mode qua `sstatus.SUM`
//!   để TAP chứng minh chuyển dịch phần cứng thật (không chỉ walk phần mềm).
//!
//! `unsafe` ở đây thuộc danh sách đóng `kernel/vifast-plat-*` của D2-17: asm `satp`/`sfence`/
//! `sstatus` và truy cập bộ nhớ vật lý. **Không** có logic object ở tệp này.

#[cfg(any(feature = "ktest", feature = "umode"))]
use core::arch::asm;

#[cfg(any(feature = "ktest", feature = "umode"))]
use vifast_plat_common::mmu::satp_sv39_asid;
use vifast_plat_common::mmu::{align_up, satp_root};
use vifast_plat_common::obj::{CapId, FlushRequest, Machine, ObjError, ObjKind, PhysMem};
use vifast_plat_common::Platform;

use crate::{mmu, tlb, Riscv64Virt};

/// Số byte arena object layer (RAM thật sau arena page table KT-46) — `[TK]`, đủ cho 2 Untyped,
/// nhiều PageTable 4 KiB và bảng trung gian của vài VSpace.
///
/// **KT-157**: 64 KiB → 96 KiB. Pha pager dựng **năm** VSpace (pager + 4 client U-mode), mỗi
/// VSpace cần root + L1 + L0 = 3 PageTable 4 KiB ⇒ 60 KiB chỉ riêng bảng trang, cộng metadata
/// CNode/TCB/SC/Endpoint ≈ 10 KiB ⇒ 64 KiB **không đủ** (đo: rig fail `NotEnoughMemory`). Đây là
/// **dung lượng bộ nhớ**, không phải ngưỡng thời gian/seed/`MAX_OBJECTS`; `MAX_OBJECTS = 96`
/// giữ nguyên, `# capa: u0 truoc rig` không nằm trong `expected.txt` nên phép đo không đổi.
pub const OBJ_ARENA_BYTES: u32 = 96 * 1024;

/// Đáy **cửa sổ PA** của `U0`: 32 MiB trong RAM (`-m 128M`), căn **2 MiB** nên cấp được Frame
/// 2 MiB rồi 4 KiB theo thứ tự giảm dần (K2 KT-36: không đệm căn lề ngầm).
pub const OBJ_PA_BASE_U0: u64 = 0x8200_0000;

/// Đáy **cửa sổ PA** của `U1`: đáy RAM `0x8000_0000`, căn **1 GiB** — Frame 1 GiB cần cửa sổ căn
/// 1 GiB; trên `-m 128M` đây là **nhãn** (D2-66), chỉ 4 KiB/2 MiB được ghi thật.
pub const OBJ_PA_BASE_U1: u64 = 0x8000_0000;

/// Báo cáo `init` cho TAP/log — địa chỉ **thật** của lần boot này.
#[derive(Clone, Copy, Debug)]
pub struct ObjReport {
    /// Arena `[base, base+OBJ_ARENA_BYTES)` (PA thật).
    pub arena_base: u64,
    /// Cuối arena.
    pub arena_end: u64,
    /// Số byte arena.
    pub arena_bytes: u32,
    /// Số Untyped đã dựng.
    pub untypeds: usize,
    /// Cap Untyped `U0`.
    pub untyped0: CapId,
    /// Cap Untyped `U1`.
    pub untyped1: CapId,
    /// Cửa sổ PA (1 GiB, D2-66).
    pub window: u64,
    /// Đáy cửa sổ PA của `U0`/`U1`.
    pub pa0: u64,
    pub pa1: u64,
    /// Tự kiểm physmap (`write → read → zero`) ĐẠT.
    pub physmap_ok: bool,
}

/// Truy cập 8 B vật lý qua physmap của kernel (D2-17: unsafe chỉ ở `kernel/vifast-plat-*`).
pub struct PhysmapMem;

impl PhysMem for PhysmapMem {
    fn read_u64(&self, pa: u64) -> Option<u64> {
        let va = mmu::physmap_alias(pa)?;
        // SAFETY: `va` là alias physmap của PA trong RAM; object layer chỉ đọc PTE/Frame mà nó
        // vừa cấp (PA đã kiểm căn lề/cửa sổ) nên 8 B nằm trong RAM đã map.
        Some(unsafe { core::ptr::read_volatile(va as *const u64) })
    }

    fn write_u64(&mut self, pa: u64, value: u64) -> bool {
        let Some(va) = mmu::physmap_alias(pa) else {
            return false;
        };
        // SAFETY: như `read_u64` — ghi 8 B vào RAM đã map qua physmap (NX không cấm ghi dữ liệu).
        unsafe { core::ptr::write_volatile(va as *mut u64, value) };
        true
    }
}

/// Một `Machine` duy nhất cho cả ảnh (single-hart object ops ở KT-52; K3b mới cần khoá).
static mut MACHINE: Machine<PhysmapMem> = Machine::new(PhysmapMem);

/// Truy cập object layer đã `init`. `&mut` vì mọi thao tác Retype/Map/Revoke đổi trạng thái.
///
/// # Panic
/// `init()` chưa chạy thì vẫn trả `Machine` rỗng (không panic) — TAP sẽ thấy `NoSlots`; các
/// caller trong repo luôn gọi [`init`] trước.
pub fn machine() -> &'static mut Machine<PhysmapMem> {
    // SAFETY: `MACHINE` là static duy nhất, được `init` khởi tạo trước mọi lần dùng; KT-52 chỉ
    // chạy object op trên một hart (BSP). Không tạo hai `&mut` cùng lúc vì mọi hàm nhận `&mut`
    // tuần tự. `addr_of_mut!` tránh `static_mut_refs`.
    unsafe { &mut *core::ptr::addr_of_mut!(MACHINE) }
}

/// Dựng 2 Untyped boot trên RAM thật và tự kiểm physmap. **Idempotent** (reset trước khi dựng).
///
/// # Panic
/// Arena/layout/physmap hỏng ⇒ panic qua `PanicHandler` của ảnh (không chạy với object layer
/// dở); thông điệp ổn định để runner đỏ.
pub fn init() -> ObjReport {
    let (_, arena_end) = mmu::arena_range();
    let base = align_up(arena_end, 4096);
    let m = machine();
    m.reset();
    let u0 = m
        .add_untyped(base, OBJ_ARENA_BYTES, OBJ_PA_BASE_U0)
        .unwrap_or_else(|e| panic!("obj: untyped U0 loi {:?}", e));
    let u1 = m
        .add_untyped(base + OBJ_ARENA_BYTES as u64, 4096, OBJ_PA_BASE_U1)
        .unwrap_or_else(|e| panic!("obj: untyped U1 loi {:?}", e));
    // Tự kiểm physmap trên 8 B cuối arena (ngoài watermark; chưa tiêu byte nào) rồi trả 0.
    let probe = base + OBJ_ARENA_BYTES as u64 - 8;
    let marker = 0x4b54_3532_4f42_4a21u64; // "KT52OBJ!"
    let physmap_ok = m.write_phys_u64(probe, marker)
        && m.read_phys_u64(probe) == Some(marker)
        && m.write_phys_u64(probe, 0)
        && m.read_phys_u64(probe) == Some(0);
    if !physmap_ok {
        panic!("obj: physmap khong doc/ghi duoc");
    }
    let report = ObjReport {
        arena_base: base,
        arena_end: base + OBJ_ARENA_BYTES as u64,
        arena_bytes: OBJ_ARENA_BYTES,
        untypeds: 2,
        untyped0: u0,
        untyped1: u1,
        window: vifast_plat_common::obj::UNTYPED_PA_WINDOW,
        pa0: OBJ_PA_BASE_U0,
        pa1: OBJ_PA_BASE_U1,
        physmap_ok,
    };
    // TAP/log cần: object layer dựng xong phải sạch (không object mồ côi, byte khớp).
    if m.check_objmem().is_err() {
        panic!("obj: check_objmem sau init that bai");
    }
    report
}

/// `true` ⇔ `satp` hiện tại đúng root + ASID kernel (dùng sau khi khôi phục `satp`).
pub fn kernel_root_active() -> bool {
    let satp = mmu::satp_now();
    satp_root(satp) == mmu::root_phys() && mmu::mmu_enabled()
}

// ---------------------------------------------------------------------------------------------
// Chỉ ảnh test: đổi `satp` sang root VSpace, chép root kernel, đọc/ghi VA user qua SUM
// ---------------------------------------------------------------------------------------------

/// Chép **512 entry root** của kernel (root đang hoạt động) vào root PageTable của VSpace.
///
/// Nhờ vậy khi `satp` trỏ root VSpace, kernel identity/nửa cao/physmap/UART/PLIC vẫn map y hệt
/// (dùng **chung** bảng cấp dưới) nên kernel tiếp tục chạy; chỉ nhánh root mà VSpace tự dùng
/// (vd. VA `0x4000_0000`, root index 1) là của object layer. Trả `false` nếu PA đích không
/// thuộc RAM/không căn 4 KiB.
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn copy_kernel_root(dst_pa: u64) -> bool {
    if dst_pa & 0xfff != 0 {
        return false;
    }
    let Some(dst) = mmu::physmap_alias(dst_pa) else {
        return false;
    };
    let src = mmu::root_phys();
    if src == 0 {
        return false;
    }
    for i in 0..512usize {
        // SAFETY: `src` là root page table 4 KiB do `mmu::init` cấp trong arena identity-mapped;
        // `dst` là alias physmap của bảng 4 KiB vừa retype từ Untyped (PA đã kiểm qua physmap).
        // Chỉ đọc/ghi 8 B trong hai bảng 4 KiB.
        unsafe {
            let entry = core::ptr::read_volatile((src as *const u64).add(i));
            core::ptr::write_volatile((dst as *mut u64).add(i), entry);
        }
    }
    true
}

/// Chép **nửa thấp** root kernel (index `0..256`, trừ `skip_index`) vào root VSpace user —
/// **không** chép nửa cao (kernel high alias + physmap).
///
/// KT-53: root VSpace user chỉ giữ mapping **cần cho kernel chạy tại chỗ** khi `satp` trỏ root
/// user: root index 0 (UART/PLIC MMIO) và root index 2 (kernel image + NOLOAD/arena) — đều `U=0`.
/// **Nửa cao** (root index ≥ 256: kernel high alias `0xFFFF_FFC0_4000_0000`, physmap
/// `0xFFFF_FFC0_0000_0000`) **không** được chép, nên U-mode không có đường PTE nào tới đó (kể cả
/// nếu một leaf kernel bị đặt `U=1` do lỗi tương lai, high alias vẫn không tồn tại trong VSpace
/// user). `skip_index` = root index của cửa sổ user (KT-53: `1` cho `0x4000_0000..0x8000_0000`)
/// để VSpace tự dựng bảng cấp dưới.
///
/// Hệ quả: sau khi đổi `satp` sang root user, kernel **không** được chạm physmap/high alias;
/// `user::UserMemAccess` vì vậy kiểm con trỏ bằng **bảng dải user ghi lúc nạp** thay vì `walk`
/// (walk đọc PTE qua physmap). Trả `false` nếu PA đích không thuộc RAM/không căn 4 KiB.
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn copy_kernel_low_root(dst_pa: u64, skip_index: usize) -> bool {
    if dst_pa & 0xfff != 0 || skip_index >= 256 {
        return false;
    }
    let Some(dst) = mmu::physmap_alias(dst_pa) else {
        return false;
    };
    let src = mmu::root_phys();
    if src == 0 {
        return false;
    }
    for i in 0..256usize {
        if i == skip_index {
            continue;
        }
        // SAFETY: `src` là root kernel 4 KiB; `dst` là alias physmap của root VSpace vừa retype
        // (PA đã kiểm qua physmap). Chỉ đọc/ghi 8 B trong hai bảng 4 KiB.
        unsafe {
            let entry = core::ptr::read_volatile((src as *const u64).add(i));
            core::ptr::write_volatile((dst as *mut u64).add(i), entry);
        }
    }
    true
}

/// Ghi `satp = Sv39(root_pa, asid)` cho hart hiện tại + `sfence.vma` toàn bộ.
///
/// Chỉ dùng khi root đích đã có đủ mapping kernel ([`copy_kernel_root`]) — nếu không, lệnh kế
/// tiếp có thể page-fault. Trả `true` ⇔ `satp` đọc lại đúng.
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn switch_root(root_pa: u64, asid: u16) -> bool {
    if root_pa & 0xfff != 0 {
        return false;
    }
    let satp = satp_sv39_asid(root_pa, asid);
    // SAFETY: root đã được chép đủ mapping kernel + mapping object cần dùng; `satp` mode 8;
    // `sfence.vma` xoá TLB của hart hiện tại (một asm block, không thao tác bộ nhớ chen giữa).
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
            options(nostack, preserves_flags),
        );
    }
    mmu::satp_now() == satp
}

/// Khôi phục `satp` kernel (root + ASID lease của KT-51) rồi `sfence.vma` toàn bộ.
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn restore_kernel_root() -> bool {
    let ok = mmu::set_satp_asid(tlb::asid());
    tlb::sfence_all();
    ok && kernel_root_active()
}

/// Bật/tắt `sstatus.SUM` (S-mode được đọc/ghi trang `U=1`; privileged spec).
///
/// `pub(crate)` từ KT-53 để `user::UserMemAccess` đọc byte user của syscall `DebugPutStr` bằng
/// cùng cơ chế đã kiểm ở TAP 35 (không mở `unsafe` ra ngoài crate arch, D2-17).
#[cfg(any(feature = "ktest", feature = "umode"))]
pub(crate) fn set_sum(on: bool) {
    const SUM: u64 = 1 << 18;
    // SAFETY: chỉ đổi bit SUM của CSR `sstatus` của hart hiện tại, không đụng bộ nhớ.
    unsafe {
        if on {
            asm!("csrs sstatus, {b}", b = in(reg) SUM, options(nostack, preserves_flags));
        } else {
            asm!("csrc sstatus, {b}", b = in(reg) SUM, options(nostack, preserves_flags));
        }
    }
}

/// Đọc 8 B tại VA **user** từ S-mode với `SUM=1`, rồi tắt SUM.
///
/// # Safety (logic)
/// Caller phải bảo đảm `va` vừa map leaf `U|R` (TAP làm vậy trước khi gọi; sai ⇒ page fault
/// thật, đúng chính sách fatal của KT-45).
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn va_read_u64_sum(va: u64) -> Option<u64> {
    if va & 7 != 0 {
        return None;
    }
    set_sum(true);
    // SAFETY: caller đã map `va` U|R|A|D; đọc 8 B trong leaf; SUM bật ở trên.
    let value = unsafe { core::ptr::read_volatile(va as *const u64) };
    set_sum(false);
    Some(value)
}

/// Ghi 8 B tại VA **user** từ S-mode với `SUM=1`, rồi tắt SUM.
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn va_write_u64_sum(va: u64, value: u64) -> bool {
    if va & 7 != 0 {
        return false;
    }
    set_sum(true);
    // SAFETY: như `va_read_u64_sum`; leaf phải có W (TAP map RW trước).
    unsafe { core::ptr::write_volatile(va as *mut u64, value) };
    set_sum(false);
    true
}

/// `true` ⇔ `sstatus.SUM` đang tắt (không rò bit SUM sang phần còn lại của boot).
#[cfg(any(feature = "ktest", feature = "umode"))]
pub fn sum_off() -> bool {
    let bits: u64;
    // SAFETY: `csrr sstatus` chỉ đọc CSR.
    unsafe { asm!("csrr {}, sstatus", out(reg) bits, options(nomem, nostack)) };
    bits & (1 << 18) == 0
}

/// `true` ⇔ object layer sạch (`check_objmem`) — TAP/log của ảnh sản phẩm.
pub fn healthy() -> bool {
    machine().check_objmem().is_ok()
}

/// Tên lỗi để in log (không kéo trait `Debug` vào `no_std` log).
pub fn error_name(e: ObjError) -> &'static str {
    e.code_name()
}

/// Tên kind để in log.
pub fn kind_name(k: ObjKind) -> &'static str {
    k.name()
}

/// Thực thi một yêu cầu flush bằng `sfence.vma` thật (KT-51 §14.2).
pub fn run_flush(req: FlushRequest) {
    match req {
        FlushRequest::Page { asid, va } => tlb::sfence_page(asid, va),
        FlushRequest::Asid { asid } => tlb::sfence_asid(asid),
        FlushRequest::All => tlb::sfence_all(),
    }
}

/// Tick hiện tại — TAP "kernel còn sống sau khi khôi phục `satp`" (rdtime của [`Platform`]).
pub fn platform_ticks() -> u64 {
    Riscv64Virt.now_ticks()
}
