// SPDX-License-Identifier: MIT OR Apache-2.0
//! SMP bring-up trên QEMU `virt` — **KT-49**: SBI HSM `hart_start` + trạng thái **PerHart**.
//!
//! # Vì sao chỉ hart boot chạy `_start`
//!
//! OpenSBI `fw_dynamic` với `-smp N` chỉ nhảy vào S-mode ở **hart boot** (hart 0); các hart còn
//! lại nằm trong firmware ở trạng thái `STOPPED`. Chúng chỉ chạy khi hart boot gọi
//! `sbi_hart_start(hartid, start_addr, opaque)` (EID [`SBI_EXT_HSM`] `0x48534D`, FID 0) — SBI
//! spec: hart được đánh thức vào S-mode với `satp = 0` (Bare), `a0 = hartid`, `a1 = opaque`,
//! ngắt tắt. Vì vậy mỗi hart thứ cấp vào bằng `_start_secondary` (asm trong [`crate`]):
//! `tp ← hartid`, `sp`/`sscratch` ← record của **chính hart đó** trong vùng NOLOAD per-hart,
//! rồi gọi `rust_secondary_main` của ảnh (hàm do bin cung cấp).
//!
//! # Trình tự (BSP)
//!
//! 1. `mmu::init()` xong (mọi hart thứ cấp sẽ `csrw satp` cùng root khi vào).
//! 2. [`init`] probe `sbi_hart_get_status(h)` cho `h = 1..MAX_HARTS`; `STOPPED` ⇒ `hart_start`;
//!    `INVALID_PARAM` (`-3`) ⇒ hart **không có trong DTB/domain** ⇒ **từ chối** + log, **không**
//!    panic cả máy; lỗi khác ⇒ log mã, bỏ qua hart đó.
//! 3. Chờ [`ONLINE`] phủ hết các hart `PRESENT` (timeout), log hart thiếu; trả [`SmpReport`].
//! 4. Hart thứ cấp: adopt `satp` của BSP, cài `stvec`, đánh dấu [`ONLINE`], rồi chờ
//!    [`PHASE`] = [`PHASE_RELEASED`] (park bằng spin; **không** `wfi` vì không có IPI — KT-50).
//!    BSP phát [`release`] khi sẵn sàng; các pha test/barrier dùng [`HartMask`] + [`barrier_*`].
//!
//! [`HartMask`]: vifast_plat_common::smp::HartMask
//!
//! # Không thuộc KT-49
//!
//! IPI (KT-50), ASID/TLB shootdown (KT-51), hotplug/suspend, preemption. `unsafe` (asm/CSR/SBI
//! `ecall`) nằm trong danh sách đóng `kernel/vifast-plat-*` của D2-17.

use core::arch::asm;
use core::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use vifast_plat_common::smp::{
    classify_hart_status, hart_bit, hart_start_ok, mask_count, start_decision, HartMask,
    HartStatus, StartDecision, HART_AREA_BYTES, MAX_HARTS, SBI_EXT_HSM, SBI_HSM_HART_GET_STATUS,
    SBI_HSM_HART_START,
};
use vifast_plat_common::timer::TIMEBASE_HZ;
use vifast_plat_common::Platform;

use crate::mmu;
use crate::timer;
use crate::Riscv64Virt;

/// Token `opaque` truyền qua `sbi_hart_start` — hart thứ cấp kiểm lại để chắc chắn nó được đánh
/// thức bởi kernel này (không phải start lạc). `"KT49"` theo ASCII.
pub const SECONDARY_TOKEN: usize = 0x4b54_3439;

/// Pha **park**: hart thứ cấp chờ BSP phát test (giá trị ban đầu của [`PHASE`]).
pub const PHASE_PARKED: u64 = 0;
/// Pha **released**: BSP cho phép hart thứ cấp vào pha SMP test.
pub const PHASE_RELEASED: u64 = 1;
/// Pha **done**: test xong (BSP sắp halt) — hart thứ cấp park vô hạn.
pub const PHASE_DONE: u64 = 2;

/// Timeout chờ hart online (timebase tick; 2 s ở 10 MHz) — hết hạn thì **log + trả report**,
/// không treo cả máy (KT-49: "hart lỗi ⇒ log, không panic cả máy").
pub const ONLINE_TIMEOUT_TICKS: u64 = 2 * TIMEBASE_HZ;
/// Timeout mặc định cho barrier (1 s) — đủ rộng cho hart chậm, vẫn không treo.
pub const BARRIER_TIMEOUT_TICKS: u64 = TIMEBASE_HZ;

/// Địa chỉ DTB OpenSBI truyền ở `a1` — `_start` lưu vào đây (sau khi zero `.bss`).
///
/// `#[no_mangle]` để asm `la t0, SMP_DTB; sd a1, 0(t0)` ghi được; kiểu `AtomicU64` giữ đúng
/// 8 byte và không cần `static mut`.
#[no_mangle]
pub static SMP_DTB: AtomicU64 = AtomicU64::new(0);

/// `true` ⇔ magic DTB (`0xd00dfeed`, big-endian ở offset 0) nếu `a1` trỏ tới DTB hợp lệ.
///
/// D2-72 Q7: K3 **chỉ kiểm magic** (không parse đầy đủ) — hart count lấy từ `hart_get_status`
/// của SBI HSM (firmware đã biết DTB), không tự đoán.
pub fn dtb_magic_ok(dtb: u64) -> bool {
    if dtb == 0 || dtb % 4 != 0 {
        return false;
    }
    // QEMU `virt` đặt FDT ở **cuối RAM** (vd 0x87e0_0000 với `-m 128M`), ngoài miền kernel
    // identity-map; đọc qua **physmap** (`PHYSMAP_BASE + PA`, KT-46 map toàn RAM 128 MiB) — không
    // thêm mapping mới, không tin `a1` mù. Nếu MMU chưa bật (không xảy ra: `smp::init` chạy sau
    // `mmu::init`) thì `dtb` là PA và đọc trực tiếp.
    let va = if mmu::mmu_enabled() {
        match mmu::physmap_alias(dtb) {
            Some(va) => va,
            None => return false,
        }
    } else {
        dtb
    };
    // SAFETY: `va` nằm trong physmap RAM đã map RW NX (hoặc PA khi MMU tắt) và 4 byte đầu của FDT
    // là header hợp lệ theo D2-72 Q7; chỉ đọc, không ghi.
    let magic = unsafe { core::ptr::read_volatile(va as *const u32) };
    magic == 0xd00d_feedu32.swap_bytes()
}

/// Báo cáo bring-up SMP của lần boot — mọi trường là **giá trị đọc lại** từ SBI/mask.
#[derive(Clone, Copy, Debug)]
pub struct SmpReport {
    /// Hart đang chạy `init` (boot hart — QEMU `virt`: 0).
    pub boot_hart: usize,
    /// Địa chỉ DTB (`a1`) và magic hợp lệ.
    pub dtb: u64,
    pub dtb_magic_ok: bool,
    /// Hart **có** trong domain/DTB (kể cả hart boot) — bitmask.
    pub present_mask: u64,
    /// Hart đã báo online (đã vào S-mode, adopt `satp`, cài `stvec`).
    pub online_mask: u64,
    /// Hart đã gọi `hart_start` **thành công** (mã 0 hoặc -6).
    pub started_mask: u64,
    /// Hart bị **từ chối** vì không có trong DTB/domain (`-3`) hoặc suspend.
    pub absent_mask: u64,
    /// Trạng thái `sbi_hart_get_status` đọc được cho mọi hart (boot hart = 0/Giữ nguyên).
    pub status: [i64; MAX_HARTS],
    /// Mã trả về của `sbi_hart_start` (0 nếu không gọi).
    pub start_code: [i64; MAX_HARTS],
    /// `true` ⇔ hết timeout mà vẫn thiếu hart online.
    pub timed_out: bool,
    /// Số hart online thật (bằng chứng TAP `N harts alive`).
    pub harts: usize,
    /// Vùng NOLOAD per-hart `[base, base + HART_AREA_BYTES)` (địa chỉ linker thật).
    pub hart_area_base: u64,
    /// Bước record per-hart (hằng số `[TK]` + link-time check ở `mmu`).
    pub hart_stride: u64,
}

impl SmpReport {
    /// `true` ⇔ mọi hart `present` đều online và không timeout.
    pub fn is_online(&self) -> bool {
        self.timed_out == false
            && self.online_mask & self.present_mask == self.present_mask
            && mask_count(self.present_mask) as usize == self.harts
    }

    /// Hart **thiếu** (present nhưng chưa online) — log khi timeout.
    pub fn missing_mask(&self) -> u64 {
        self.present_mask & !self.online_mask
    }
}

// ---------------------------------------------------------------------------------------------
// Trạng thái toàn cục (atomic — nhiều hart đọc/ghi)
// ---------------------------------------------------------------------------------------------

/// Bitmask hart **có** trong domain/DTB (probe `hart_get_status`) — gồm hart boot.
static PRESENT: HartMask = HartMask::new();
/// Bitmask hart đã báo online (mỗi hart tự set bit của mình trong [`secondary_setup`]).
static ONLINE: HartMask = HartMask::new();
/// Bitmask hart đã `hart_start` thành công.
static STARTED: HartMask = HartMask::new();
/// Bitmask hart bị từ chối (không có trong DTB/domain).
static ABSENT: HartMask = HartMask::new();
/// Mã `sbi_hart_get_status` theo hart (`-99` = chưa probe).
static STATUS: [AtomicI64; MAX_HARTS] = [const { AtomicI64::new(-99) }; MAX_HARTS];
/// Mã `sbi_hart_start` theo hart.
static START_CODE: [AtomicI64; MAX_HARTS] = [const { AtomicI64::new(0) }; MAX_HARTS];
/// Ticket tới (0..N-1) của từng hart — "mỗi hart in id + ticket".
static TICKET: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
static NEXT_TICKET: AtomicU64 = AtomicU64::new(0);
/// Địa chỉ trapframe của trap gần nhất **theo hart** (bằng chứng `.trap_stack` riêng mỗi hart).
static LAST_FRAME: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// Pha SMP (xem [`PHASE_PARKED`]/[`PHASE_RELEASED`]/[`PHASE_DONE`]).
static PHASE: AtomicU64 = AtomicU64::new(PHASE_PARKED);
/// Hart boot (hart chạy `rust_main`).
static BOOT_HART: AtomicU64 = AtomicU64::new(0);

// Barrier tập trung một vòng: BSP đặt `round`/`expected`, mọi hart set bit `arrived`, BSP thấy đủ
// thì set `released`; AP chờ `released`.
static BARRIER_ROUND: AtomicU64 = AtomicU64::new(0);
static BARRIER_EXPECTED: AtomicU64 = AtomicU64::new(0);
static BARRIER_RELEASED: AtomicU64 = AtomicU64::new(0);
static BARRIER_ARRIVED: HartMask = HartMask::new();

/// Hart đang chạy ([`Riscv64Virt::hart_id`] đọc `tp` — mỗi hart một `tp`).
pub fn current_hart() -> usize {
    Riscv64Virt.hart_id()
}

/// `rdtime` hiện tại (timebase) — timeout của bring-up/barrier.
fn now() -> u64 {
    timer::now_ticks()
}

/// Spin tới khi `pred()` đúng hoặc quá `timeout_ticks`; trả `true` ⇔ đúng hạn.
fn spin_until(timeout_ticks: u64, mut pred: impl FnMut() -> bool) -> bool {
    let start = now();
    loop {
        if pred() {
            return true;
        }
        if now().wrapping_sub(start) > timeout_ticks {
            return false;
        }
        core::hint::spin_loop();
    }
}

/// Busy-wait `ticks` đơn vị timebase (hart "chậm" cho test barrier) — không `wfi`.
pub fn delay_ticks(ticks: u64) {
    let start = now();
    while now().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------------------------
// SBI HSM
// ---------------------------------------------------------------------------------------------

/// Gọi SBI HSM: trả `(error, value)` — `a0`/`a1` theo SBI spec.
fn sbi_hsm(fid: u64, a0: u64, a1: u64, a2: u64) -> (i64, i64) {
    let mut error: u64 = 0;
    let mut value: u64 = 0;
    // SAFETY: `ecall` SBI HSM hợp lệ (OpenSBI v1.8.1 có extension `hsm` — log firmware xác nhận);
    // SBI bảo toàn mọi thanh ghi trừ `a0`/`a1`; `a2` là tham số thứ ba (opaque). `ecall` có thể
    // ghi bộ nhớ phía firmware nên **không** `nomem`.
    unsafe {
        asm!(
            "ecall",
            in("a7") SBI_EXT_HSM,
            in("a6") fid,
            inout("a0") a0 => error,
            inout("a1") a1 => value,
            in("a2") a2,
            options(nostack),
        );
    }
    (error as i64, value as i64)
}

/// `sbi_hart_start(hartid, start_addr, opaque)` — trả mã SBI (`0` thành công).
pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> i64 {
    sbi_hsm(
        SBI_HSM_HART_START,
        hartid as u64,
        start_addr as u64,
        opaque as u64,
    )
    .0
}

/// `sbi_hart_get_status(hartid)` — trả mã trạng thái SBI HSM (`1` = `STOPPED`, `-3` = không có).
pub fn hart_status(hartid: usize) -> i64 {
    let (error, value) = sbi_hsm(SBI_HSM_HART_GET_STATUS, hartid as u64, 0, 0);
    if error == 0 {
        value
    } else {
        error
    }
}

/// Địa chỉ entry cho hart thứ cấp (`_start_secondary`, 4-byte aligned trong `.text.entry`).
pub fn secondary_entry() -> usize {
    extern "C" {
        fn _start_secondary();
    }
    _start_secondary as *const () as usize
}

// ---------------------------------------------------------------------------------------------
// Bring-up (BSP) + vào S-mode (AP)
// ---------------------------------------------------------------------------------------------

/// Probe + đánh thức toàn bộ hart `STOPPED` trong domain/DTB, chờ online (timeout), trả report.
///
/// Gọi trên **BSP** sau `mmu::init()` (hart thứ cấp adopt `satp` khi vào) và **trước** mọi test
/// cần SMP. Hart không có trong DTB (`-3`) bị **từ chối** + log — không panic cả máy.
pub fn init() -> SmpReport {
    let boot = current_hart();
    BOOT_HART.store(boot as u64, Ordering::SeqCst);
    let dtb = SMP_DTB.load(Ordering::SeqCst);
    let dtb_ok = dtb_magic_ok(dtb);
    PRESENT.insert(boot);
    // Hart boot đã chạy trong S-mode ⇒ coi như online ngay (hart khác tự set khi vào được).
    ONLINE.insert(boot);
    STATUS[boot].store(0, Ordering::SeqCst); // boot hart đang STARTED
    for hart in 0..MAX_HARTS {
        if hart == boot {
            continue;
        }
        let status = hart_status(hart);
        STATUS[hart].store(status, Ordering::SeqCst);
        match start_decision(status) {
            StartDecision::Start => {
                let code = hart_start(hart, secondary_entry(), SECONDARY_TOKEN);
                START_CODE[hart].store(code, Ordering::SeqCst);
                if hart_start_ok(code) {
                    STARTED.insert(hart);
                    PRESENT.insert(hart);
                }
                // Lỗi `hart_start` không panic: hart đó coi như "có trong DTB nhưng không lên";
                // vẫn đưa vào PRESENT để report/TAP thấy thiếu online (đỏ rõ ràng, không treo).
                else {
                    PRESENT.insert(hart);
                }
            }
            StartDecision::AlreadyStarted => {
                PRESENT.insert(hart);
            }
            StartDecision::Reject => {
                ABSENT.insert(hart);
            }
            StartDecision::Error(_code) => {
                // Lỗi SBI khác: bỏ hart đó (không start), log mã qua report; không panic.
            }
        }
    }
    let expected = PRESENT.bits();
    let timed_out = !spin_until(ONLINE_TIMEOUT_TICKS, || {
        ONLINE.bits() & expected == expected
    });
    SmpReport {
        boot_hart: boot,
        dtb,
        dtb_magic_ok: dtb_ok,
        present_mask: PRESENT.bits(),
        online_mask: ONLINE.bits(),
        started_mask: STARTED.bits(),
        absent_mask: ABSENT.bits(),
        status: read_statuses(),
        start_code: read_start_codes(),
        timed_out,
        harts: ONLINE.count() as usize,
        hart_area_base: mmu::hart_area_base(),
        hart_stride: mmu::hart_stride(),
    }
}

fn read_statuses() -> [i64; MAX_HARTS] {
    let mut out = [0i64; MAX_HARTS];
    for (hart, slot) in out.iter_mut().enumerate() {
        *slot = STATUS[hart].load(Ordering::SeqCst);
    }
    out
}

fn read_start_codes() -> [i64; MAX_HARTS] {
    let mut out = [0i64; MAX_HARTS];
    for (hart, slot) in out.iter_mut().enumerate() {
        *slot = START_CODE[hart].load(Ordering::SeqCst);
    }
    out
}

/// Chuẩn bị hart **thứ cấp** khi đã vào `rust_secondary_main`: kiểm `opaque`, adopt `satp` của
/// BSP, cài `stvec`, đánh dấu online. Trả `false` nếu `opaque` sai (hart sẽ park, không panic).
pub fn secondary_setup(opaque: usize) -> bool {
    let hart = current_hart();
    let token_ok = opaque == SECONDARY_TOKEN;
    let satp_ok = mmu::adopt_satp();
    crate::install_trap_vector();
    if token_ok && satp_ok {
        ONLINE.insert(hart);
        true
    } else {
        false
    }
}

/// Hart thứ cấp chờ BSP phát pha SMP (`PHASE_RELEASED`) — trả `true` nếu kịp, `false` nếu timeout
/// (khi đó caller ghi lỗi rồi park; **không** treo cả máy).
pub fn wait_release(timeout_ticks: u64) -> bool {
    spin_until(timeout_ticks, || {
        PHASE.load(Ordering::SeqCst) != PHASE_PARKED
    })
}

/// BSP phát pha SMP: mọi hart thứ cấp ra khỏi park.
pub fn release() {
    PHASE.store(PHASE_RELEASED, Ordering::SeqCst);
}

/// BSP kết thúc pha SMP; hart thứ cấp park vô hạn.
pub fn finish() {
    PHASE.store(PHASE_DONE, Ordering::SeqCst);
}

/// Pha hiện tại (0 park / 1 released / 2 done).
pub fn phase() -> u64 {
    PHASE.load(Ordering::SeqCst)
}

/// Lấy ticket online (0..N-1) cho `hart` — in ra `# hart h alive ticket=T`; `u64::MAX` nếu lỗi.
pub fn take_ticket(hart: usize) -> u64 {
    if hart >= MAX_HARTS {
        return u64::MAX;
    }
    let ticket = NEXT_TICKET.fetch_add(1, Ordering::SeqCst);
    TICKET[hart].store(ticket, Ordering::SeqCst);
    ticket
}

/// Ticket đã cấp cho `hart` (`u64::MAX` = chưa).
pub fn ticket_of(hart: usize) -> u64 {
    if hart < MAX_HARTS {
        TICKET[hart].load(Ordering::SeqCst)
    } else {
        u64::MAX
    }
}

/// Ghi địa chỉ trapframe của trap gần nhất trên `hart` — `trap_dispatch` gọi ở mọi trap; TAP
/// "stack/guard" kiểm frame nằm trong `.trap_stack` **của chính hart đó**.
pub fn note_trap_frame(hart: usize, frame: usize) {
    if hart < MAX_HARTS {
        LAST_FRAME[hart].store(frame as u64, Ordering::SeqCst);
    }
}

/// Trapframe gần nhất của `hart` (0 = chưa có trap).
pub fn last_trap_frame(hart: usize) -> u64 {
    if hart < MAX_HARTS {
        LAST_FRAME[hart].load(Ordering::SeqCst)
    } else {
        0
    }
}

/// `true` ⇔ `hart` đã online (bitmask).
pub fn is_online(hart: usize) -> bool {
    ONLINE.contains(hart)
}

/// Bitmask đọc lại (cho TAP/log).
pub fn online_mask() -> u64 {
    ONLINE.bits()
}

/// Bitmask hart có trong DTB/domain.
pub fn present_mask() -> u64 {
    PRESENT.bits()
}

/// Bitmask hart bị từ chối (không có trong DTB/domain).
pub fn absent_mask() -> u64 {
    ABSENT.bits()
}

/// Bitmask hart `hart_start` thành công.
pub fn started_mask() -> u64 {
    STARTED.bits()
}

/// Mã `hart_get_status` của `hart` (`-99` = chưa probe).
pub fn status_of(hart: usize) -> i64 {
    if hart < MAX_HARTS {
        STATUS[hart].load(Ordering::SeqCst)
    } else {
        -99
    }
}

/// Mã `hart_start` của `hart`.
pub fn start_code_of(hart: usize) -> i64 {
    if hart < MAX_HARTS {
        START_CODE[hart].load(Ordering::SeqCst)
    } else {
        0
    }
}

/// `true` ⇔ `hart` có trong DTB/domain (theo mask probe).
pub fn is_present(hart: usize) -> bool {
    PRESENT.contains(hart)
}

/// Mô tả trạng thái SBI của `hart` (dùng cho log `# smp:`).
pub fn status_name(hart: usize) -> &'static str {
    match classify_hart_status(status_of(hart)) {
        HartStatus::Started => "started",
        HartStatus::Stopped => "stopped",
        HartStatus::Pending => "pending",
        HartStatus::Suspended => "suspended",
        HartStatus::Invalid => "invalid",
        HartStatus::Error(_) => "error",
    }
}

/// Bit của hart trong mask — tiện cho ktest (không cần import `hart_bit`).
pub const fn bit(hart: usize) -> u64 {
    hart_bit(hart)
}

/// Số hart online hiện tại (`ONLINE.count()`).
pub fn online_count() -> usize {
    ONLINE.count() as usize
}

/// Số byte vùng NOLOAD per-hart (`MAX_HARTS × HART_STRIDE`) — log/budget evidence.
pub const fn hart_area_bytes() -> u64 {
    HART_AREA_BYTES
}

// ---------------------------------------------------------------------------------------------
// Barrier tập trung — "hart chậm không treo"
// ---------------------------------------------------------------------------------------------

/// BSP bắt đầu vòng barrier `round` với `expected` (bitmask) — xoá arrival cũ, đặt kỳ vọng.
pub fn barrier_begin(round: u64, expected: u64) {
    BARRIER_ARRIVED.clear();
    BARRIER_EXPECTED.store(expected, Ordering::SeqCst);
    // `released` **không** reset: nó tăng đơn điệu theo vòng (`release(round)`), nên hart còn kẹt
    // ở vòng trước vẫn thấy release cũ của mình; reset về 0 sẽ làm hart đó chờ vô ích (đua giữa
    // "BSP sang vòng mới" và "AP chưa kịp đọc release cũ") — bug đo được ở lần boot smp=4.
    BARRIER_ROUND.store(round, Ordering::SeqCst);
}

/// Một hart **đến** barrier `round`: chờ BSP công bố vòng (nếu cần), set bit arrival.
///
/// Trả `false` nếu `round` chưa được công bố trong timeout (hart đến sai vòng).
pub fn barrier_join(round: u64, timeout_ticks: u64) -> bool {
    let hart = current_hart();
    if !spin_until(timeout_ticks, || {
        BARRIER_ROUND.load(Ordering::SeqCst) == round
    }) {
        return false;
    }
    BARRIER_ARRIVED.insert(hart);
    true
}

/// Đọc arrival hiện tại (BSP chờ đủ `expected`).
pub fn barrier_arrivals() -> u64 {
    BARRIER_ARRIVED.bits()
}

/// BSP chờ đủ arrival (hoặc timeout); trả `(arrived_mask, timed_out)`.
pub fn barrier_wait_arrivals(timeout_ticks: u64) -> (u64, bool) {
    let expected = BARRIER_EXPECTED.load(Ordering::SeqCst);
    let ok = spin_until(timeout_ticks, || {
        BARRIER_ARRIVED.bits() & expected == expected
    });
    (BARRIER_ARRIVED.bits(), !ok)
}

/// BSP phát barrier vòng `round` — mọi hart đang chờ được đi tiếp.
pub fn barrier_release(round: u64) {
    BARRIER_RELEASED.store(round, Ordering::SeqCst);
}

/// Hart chờ vòng `round` được phát; `false` nếu timeout.
pub fn barrier_wait_release(round: u64, timeout_ticks: u64) -> bool {
    spin_until(timeout_ticks, || {
        BARRIER_RELEASED.load(Ordering::SeqCst) >= round
    })
}

/// Kỳ vọng đã đặt cho vòng barrier hiện tại (BSP).
pub fn barrier_expected() -> u64 {
    BARRIER_EXPECTED.load(Ordering::SeqCst)
}
