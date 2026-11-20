// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Cầu nối core `no_std` vào kernel thật** (KT-61) — `vifast-core::Machine` là `static` **trong
//! ảnh kernel** (`.bss`, đo bởi `v2/test/boot/budget.sh`), không phải stack/heap/arena.
//!
//! # Biên giới D2-83 (một chiều, không nhân đôi chủ sở hữu)
//!
//! - Core sở hữu **Untyped/CNode/MDB/cap/TCB/SchedContext/Endpoint/Reply/Notification/IPC/
//!   WaitAddr** — toàn bộ trạng thái nằm trong `Machine` này.
//! - **`Frame`/`PageTable`/`VSpace` vẫn thuộc object layer** [`crate::obj`]
//!   (`vifast_plat_common::obj`, KT-52). Tệp này **không** tạo/không Retype các loại đó; core trả
//!   `NotSupported` (pin D2-83, TAP `core d2-83 boundary`). Cầu nối duy nhất là **một chiều**:
//!   kernel truyền **nhãn PA/ObjId** của object layer vào làm **khoá** (Untyped boot arena cho
//!   `add_untyped`, `ObjId` Frame làm khoá `WaitAddr`) — core không đọc byte nào của vùng đó và
//!   không giữ body/PTE của Frame.
//! - Vì vậy `Region.base` của core là **nhãn PA** (D2-83), không phải địa chỉ block heap như
//!   `sim::untyped_arena`; TAP không BAO GIỜ dereference nhãn đó.
//!
//! # Static: **`.bss`** + khởi tạo **tại chỗ** (không bản tạm 180 KB)
//!
//! `Machine` có trường khởi tạo **khác 0** (`NODE_NONE = u16::MAX` cho `wait_buckets`, `n_harts`,
//! `published[*].idle`) nên **không** thể là zeroed `.bss` thuần. KT-61 thử hai cách:
//!
//! 1. `static mut MACHINE: Machine = Machine::with_harts(1)` — rơi vào `.data` ⇒ **180.168 B nằm
//!    trong FILE ảnh** (loader copy mỗi boot) — đắt và không cần.
//! 2. `static MaybeUninit<Machine>` + `MACHINE.write(Machine::with_harts(1))` — Rust trả struct lớn
//!    qua `sret` nên bản tạm 180.168 B dựng **trên stack 64 KiB** ⇒ tràn guard page
//!    (`PANIC: guard page fault stval=0x80400000`, log `JOURNAL/kt61-red-before-core-link.log`).
//!
//! Cách chốt: static **`MaybeUninit<Machine>` trong `.bss`** (`NOBITS` — 0 byte trong file, chỉ
//! `memsz`), rồi [`init`] **zero + khởi tạo tại chỗ** qua [`Machine::init_zeroed`] — không bản
//! tạm, không copy 180 KB. `.bss` tăng đúng `size_of::<Machine>()` và **được `budget.sh` cộng
//! đủ** vào ngân sách ảnh (`.data + .sdata + .bss + .sbss` ≤ 262.144 B, D2-72 Q5). TAP `core link`
//! chứng minh static nằm trong vùng ảnh `[KERNEL_PHYS_BASE, guard)`.
//!
//! # Khoá
//!
//! Mọi truy cập đi qua [`with_machine`] (spin lock `AtomicBool`): kernel SMP có nhiều hart nhưng
//! KT-61 chỉ dùng core trên BSP; lock giữ bất biến "một `&mut Machine` tại một thời điểm" nếu
//! K3.5 gọi từ hart khác. Không re-entrant: closure **không** được gọi `with_machine` lồng nhau.
//!
//! `unsafe` ở đây thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17): chỉ `static` + con trỏ tới
//! chính nó, không asm/MMIO mới.

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

use vifast_core::{Footprint, Machine, ObjId};

use crate::mmu;

/// Pin **D2-84**: `size_of::<Machine>()` ≤ **196.608 B** (192 KiB) = trần ảnh 262.144 B trừ
/// 64 KiB headroom. Vượt ⇒ tối ưu cấu trúc, **không** nâng pin.
pub const CORE_PIN_BYTES: usize = 196_608;

/// Trần **ảnh link** (QD §3.5 / D2-85a): `.data+.sdata+.bss+.sbss` ≤ **262.144 B** — ngưỡng
/// **khác** pin trên, `budget.sh` đo trên ELF thật (không đọc lẫn).
pub const IMAGE_CAP_BYTES: usize = 262_144;

/// Cỡ vùng RAM **nhãn** cấp cho Untyped boot của core (bắt đầu ngay sau arena object layer).
/// Core chỉ kế toán byte (D2-83) — K3.5 sẽ dùng làm arena slot; KT-61 không dereference.
pub const CORE_ARENA_BYTES: usize = 64 * 1024;

/// `size_of::<Machine>()` — in ra trong TAP/báo cáo (D2-84/D2-85a), cùng `budget.sh` cho ảnh.
pub const CORE_STATIC_BYTES: usize = core::mem::size_of::<Machine>();

/// Model core duy nhất của ảnh — `MaybeUninit` ⇒ **`.bss`** (NOBITS: 0 byte trong file, chỉ
/// `memsz`); [`init`] zero + khởi tạo tại chỗ bằng [`Machine::init_zeroed`] (không bản tạm 180 KB,
/// không copy file). `.data+.bss` của ảnh vẫn được `budget.sh` cộng đủ `size_of::<Machine>()`.
static mut MACHINE: MaybeUninit<Machine> = MaybeUninit::uninit();

/// Khoá một-người-ghi cho [`MACHINE`] (xem đầu tệp).
static LOCKED: AtomicBool = AtomicBool::new(false);

/// Địa chỉ của `Machine` — TAP dùng để chứng minh static nằm trong **vùng ảnh kernel**.
pub fn machine_addr() -> u64 {
    core::ptr::addr_of!(MACHINE) as u64
}

/// `size_of::<Machine>()` (byte tĩnh model chiếm trong `.bss`).
pub fn machine_bytes() -> usize {
    CORE_STATIC_BYTES
}

/// Báo cáo [`init`] cho TAP/log ảnh sản phẩm — số **thật** của lần boot này.
#[derive(Clone, Copy, Debug)]
pub struct CoreReport {
    /// Địa chỉ `static MACHINE` trong ảnh.
    pub addr: u64,
    /// `size_of::<Machine>()`.
    pub static_bytes: usize,
    /// Pin D2-84.
    pub pin: usize,
    /// Đáy Untyped boot (nhãn PA, D2-83).
    pub base: u64,
    /// Cỡ Untyped boot.
    pub arena_bytes: usize,
    /// Object Untyped của core.
    pub untyped: ObjId,
    /// Số hart **nền tảng** đã truyền vào `Machine::init_zeroed` (D2-88(a)).
    pub harts: usize,
    /// Số object trong bảng core.
    pub objects: usize,
    /// Số cap sống.
    pub caps: usize,
    /// `check_caps` ĐẠT.
    pub check_caps: bool,
    /// `check_bytes(untyped)` ĐẠT.
    pub check_bytes: bool,
    /// `check_all_bytes` ĐẠT.
    pub check_all_bytes: bool,
    /// Bảng phân bố byte (D2-84/D2-85c).
    pub footprint: Footprint,
    /// `true` ⇔ static nằm trong `[KERNEL_PHYS_BASE, guard_start)` (vùng ảnh, không phải arena).
    pub in_image: bool,
}

impl CoreReport {
    /// `true` ⇔ mọi bất biến tầng 1 xanh + pin D2-84 + static trong ảnh.
    pub fn healthy(&self) -> bool {
        self.static_bytes <= self.pin
            && self.in_image
            && self.check_caps
            && self.check_bytes
            && self.check_all_bytes
    }
}

fn lock() {
    while LOCKED
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
}

fn unlock() {
    LOCKED.store(false, Ordering::Release);
}

/// Chạy `f` với **`&mut Machine` duy nhất** của ảnh (khoá spin; không re-entrant).
///
/// # Panic
/// `init()` chưa chạy thì `MACHINE` chưa được ghi — mọi caller trong repo luôn gọi [`init`] trước
/// (ảnh sản phẩm và ảnh test đều `init` ngay sau object layer). Debug build có assert.
pub fn with_machine<R>(f: impl FnOnce(&mut Machine) -> R) -> R {
    lock();
    debug_assert!(
        MACHINE_INIT.load(Ordering::Acquire),
        "core_bridge: with_machine trước init"
    );
    // SAFETY: `MACHINE` là static duy nhất; `init` đã zero + `init_zeroed` (bit-valid `Machine`)
    // trước mọi lần đọc, và lock bảo đảm chỉ một `&mut` tại một thời điểm. Không có đường nào khác
    // chạm static này.
    let m = unsafe { &mut *core::ptr::addr_of_mut!(MACHINE).cast::<Machine>() };
    let r = f(m);
    unlock();
    r
}

/// Đánh dấu `init` đã ghi model (debug assert của [`with_machine`]).
static MACHINE_INIT: AtomicBool = AtomicBool::new(false);

/// Ghi model core vào `.bss` và dựng **Untyped boot** từ vùng `[base, base+bytes)`.
///
/// `base` là **nhãn PA** (D2-83): KT-61 truyền đáy vùng RAM ngay sau arena object layer; core chỉ
/// kế toán byte, không đọc/ghi vùng này. Idempotent: gọi lại ghi model mới từ đầu (ảnh test gọi
/// một lần).
pub fn init(base: u64, bytes: usize) -> CoreReport {
    lock();
    debug_assert!(bytes > 0, "core_bridge: Untyped boot phải có cỡ dương");
    // SAFETY: static `.bss` (MaybeUninit); `write_bytes` ghi 0 toàn bộ **không đọc** giá trị cũ,
    // rồi `init_zeroed` đặt các sentinel khác 0. `Machine` không có `Drop`/con trỏ nên zero là một
    // giá trị bit-valid (enumerate của nó đều nhận 0). Lock đang giữ ⇒ không ai đọc chen giữa.
    let raw = core::ptr::addr_of_mut!(MACHINE).cast::<Machine>();
    unsafe { core::ptr::write_bytes(raw, 0, 1) };
    let m = unsafe { &mut *raw };
    // D2-88(a): `Machine` chạy **số hart nền tảng thật** (SBI HSM/DTB), không hardcode 1; TAP
    // pin trên `-smp 1/2/4` thấy 1/2/4.
    let harts = crate::smp::hart_count();
    m.init_zeroed(u16::try_from(harts).unwrap_or(1));
    MACHINE_INIT.store(true, Ordering::Release);
    let untyped = m.add_untyped(base, bytes);
    let check_caps = m.check_caps().is_ok();
    let check_bytes = m.check_bytes(untyped).is_ok();
    let check_all_bytes = m.check_all_bytes().is_ok();
    let report = CoreReport {
        addr: machine_addr(),
        static_bytes: CORE_STATIC_BYTES,
        pin: CORE_PIN_BYTES,
        base,
        arena_bytes: bytes,
        untyped,
        harts,
        objects: m.object_count(),
        caps: m.live_cap_count(),
        check_caps,
        check_bytes,
        check_all_bytes,
        footprint: Machine::footprint(),
        in_image: in_image_region(machine_addr(), CORE_STATIC_BYTES),
    };
    unlock();
    report
}

/// `true` ⇔ `[addr, addr+bytes)` nằm trong **vùng ảnh kernel** `[KERNEL_PHYS_BASE, guard_start)`
/// (trước vùng NOLOAD của per-hart); đây là bằng chứng "static nằm trong ảnh", không phải arena.
fn in_image_region(addr: u64, bytes: usize) -> bool {
    let (guard_start, _) = mmu::guard_range_of(0);
    let end = addr.saturating_add(bytes as u64);
    addr >= mmu::KERNEL_PHYS_BASE && end <= guard_start && end > addr
}

/// Kết quả **một vòng đời TCB thật** của ảnh sản phẩm (soak 1.000 boot kiểm core trên kernel
/// sản phẩm, không chỉ ảnh test).
#[derive(Clone, Copy, Debug)]
pub struct BootSmoke {
    /// Retype TCB từ Untyped ĐẠT.
    pub retype: bool,
    /// `Tcb.Configure` ĐẠT.
    pub configure: bool,
    /// `Resume` (sau `Suspend`) ĐẠT.
    pub resume: bool,
    /// `Suspend(Abort)` ĐẠT.
    pub suspend: bool,
    /// Trạng thái cuối là `Ready` + `suspended = false`.
    pub state_ready: bool,
    /// Mọi bất biến tầng 1 + scheduler xanh sau khi revoke.
    pub checks: bool,
    /// Số object trong bảng core.
    pub objects: usize,
    /// Số cap sống sau revoke (đúng 1: cap Untyped).
    pub caps: usize,
    /// Byte Untyped đã dùng sau revoke (đúng 0).
    pub used: usize,
}

/// Chạy **một vòng đời TCB** trên `Machine` thật của ảnh sản phẩm: `Retype` TCB từ Untyped →
/// `Configure` → `Ready` → `Suspend(Abort)` → `Resume` → `revoke` cây Untyped (byte về 0).
///
/// Không dùng API test-only; không in TAP (ảnh sản phẩm chỉ `ok 1 - k3 boot`). Lỗi từng bước
/// được trả về qua cờ — ảnh sản phẩm `PANIC` nếu bất kỳ cờ nào sai (soak đỏ, không giả xanh).
pub fn boot_smoke(untyped: ObjId) -> BootSmoke {
    with_machine(|m| {
        let root = m.add_cnode(16);
        let us = vifast_core::SlotRef::new(root, 0);
        let uc = m.root_cap(untyped);
        let put = m.put_cap(root, 0, uc).is_ok();
        let mut retype = false;
        let mut configure = false;
        let mut suspend = false;
        let mut resume = false;
        let mut state_ready = false;
        let mut tcb = 0u32;
        if put {
            if let Ok(cap) = m.retype(us, vifast_core::ObjType::Tcb, 0, 0) {
                if m.put_cap(root, 1, cap).is_ok() {
                    if let Ok(obj) = m.cap_obj(cap) {
                        tcb = obj;
                        retype = true;
                    }
                }
            }
        }
        let ts = vifast_core::SlotRef::new(root, 1);
        if retype {
            configure = m.tcb_configure(ts, None).is_ok()
                && m.tcb_set_priority(ts, 4, 8).is_ok()
                && m.start_tcb(tcb).is_ok();
            let torn = configure && m.tcb_suspend(ts, vifast_core::SuspendMode::Abort).is_ok();
            let treo = torn && m.tcb_read_state(ts).map(|s| s.suspended).unwrap_or(false);
            let go = treo && m.tcb_resume(ts).is_ok();
            let info = m.tcb_info(ts).ok();
            state_ready = go
                && info
                    .map(|i| i.state == vifast_core::TcbState::Ready && !i.suspended)
                    .unwrap_or(false);
            suspend = treo;
            resume = go;
        }
        // Trả toàn bộ byte/cap: delete cap cspace (không có) + revoke cây Untyped.
        let _ = m.delete(ts);
        let _ = m.revoke(root, Some(0));
        let checks = m.check_caps().is_ok()
            && m.check_bytes(untyped).is_ok()
            && m.check_all_bytes().is_ok()
            && m.check_sched().is_ok()
            && m.check_sc_refs().is_ok()
            && m.check_budget().is_ok();
        BootSmoke {
            retype,
            configure,
            suspend,
            resume,
            state_ready,
            checks,
            objects: m.object_count(),
            caps: m.live_cap_count(),
            used: m.untyped_used(untyped),
        }
    })
}
