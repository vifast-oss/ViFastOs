// SPDX-License-Identifier: MIT OR Apache-2.0
//! **TAP — khoá MCS bọc lưu/tắt `sstatus.SIE`**: loại trừ hai ngữ cảnh thật + ngắt bị hoãn thật
//! (KT-336c, D2-243; cùng khuôn `KT-336a` cho `x86_64`).
//!
//! Hai phép đo **độc lập**, mỗi phép trả lời một câu hỏi mà `ktest.sh` đọc từ log:
//!
//! 1. **Loại trừ hai ngữ cảnh** (`# lock: mcs …`): BSP lấy [`TEST_LOCK`] (guard đã tắt `SIE`), rồi
//!    **bật lại `SIE` một cách cố ý** trong vùng găng và tự gửi IPI cho chính BSP. Ngữ cảnh thứ
//!    hai là **trap handler thật** (`stvec` → `trap_dispatch` → `ipi::on_ipi_interrupt`) — đúng
//!    cặp "luồng thường vs handler ngắt" của D2-238. Handler gọi [`on_ipi`] →
//!    `try_lock_irqsave` trên **cùng** khoá: khoá loại trừ đúng ⇒ `denied=1`, `overlap=0`; nếu
//!    không ⇒ nó vào vùng găng khi BSP đang trong ⇒ `in_cs > 1`, `overlap > 0` ⇒ TAP ĐỎ.
//!    Bất biến **không phụ thuộc thứ tự tick**: BSP chờ **biến đếm** `PROBE_DONE` (trần số vòng),
//!    không chờ thời gian (`k6-test-invariants.md` §1).
//!
//! 2. **Ngắt bị hoãn** (`# lock: irq …`): bật nguồn ngắt mềm (`sie.SSIE`) + `sstatus.SIE`, lấy
//!    khoá (guard tắt `SIE`), tự gửi IPI, quay một cửa sổ vòng có trần. Trong vùng găng `SIE = 0`
//!    ⇒ handler **không thể** chạy ⇒ `during == before` và `if_inside = 0`. Nhả khoá (guard khôi
//!    phục **đúng** `SIE` đã lưu = 1) ⇒ IPI đang chờ **được giao** ⇒ `after > before`. Nếu `lock`
//!    **không** tắt ngắt, handler chạy ngay trong vùng găng ⇒ `during > before`, `inside_if = 1`
//!    ⇒ TAP ĐỎ (đột biến `mut-no-irq`).
//!
//! # Vì sao đây **không** phải "đo thời gian"
//!
//! - "Không giao được khi `SIE = 0`" là **bảo đảm của kiến trúc** (privileged spec: `sstatus.SIE`
//!   che mọi ngắt S-mode), không phải "thường đúng"; cửa sổ vòng chỉ để **cho CPU cơ hội** giao
//!   nếu `SIE` sai.
//! - "Đã giao sau khi nhả" là **biến đếm** `after > before`, chờ tới trần số vòng — chứng minh IPI
//!   **thật sự đã được xếp** (không phải ta quên gửi), và hết trần ⇒ ĐỎ, **không treo**.
//!
//! # Đột biến (cả hai phải ĐỎ — D2-05)
//!
//! - `mut-no-irq`: `IrqState` không `csrc sstatus,SIE` ⇒ `if_inside = 1`/`deferred = 0` ⇒ ĐỎ.
//! - `mut-no-lock`: bỏ phần MCS ⇒ handler vào vùng găng khi BSP đang giữ ⇒ `overlap > 0` ⇒ ĐỎ.
//! - `mut-leak-unlock`: guard bỏ nhánh nhả khoá ⇒ lần lấy kế tiếp chạm trần
//!   `lock_irqsave_bounded` ⇒ in `# lock: BAD acquire` ⇒ TAP ĐỎ (**không** treo).

use core::fmt::Write;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;

use vifast_plat_common::ipi::IPI_STATS;
use vifast_plat_common::smp::hart_bit;
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::ipi;
use vifast_plat_riscv64::irq_lock::{irq_enabled, IrqMcsLock};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Trần **số vòng** chờ sự kiện (trap handler chạy) — hết trần ⇒ TAP ĐỎ, **không treo**.
const WAIT_SPIN_LIMIT: u64 = 50_000_000;
/// Cửa sổ vòng trong vùng găng: đủ để CPU giao IPI nếu `SIE` sai; `SIE = 0` thì không thể giao.
const IRQ_WINDOW_SPINS: u64 = 500_000;
/// Trần của lần lấy khoá test — deadlock thành `None` có chẩn đoán, không chờ vô hạn.
const ACQUIRE_SPIN_LIMIT: u32 = 50_000_000;

/// Khoá **của riêng ca kiểm** — không dùng chung với `core_bridge`/`COM1` để phép đo độc lập.
static TEST_LOCK: IrqMcsLock<()> = IrqMcsLock::new(());
/// `1` ⇔ trap IPI kế tiếp của BSP phải thử lấy [`TEST_LOCK`].
static PROBE_ARMED: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ ngữ cảnh thứ hai **đã** thử và bị từ chối (bằng chứng loại trừ).
static PROBE_DENIED: AtomicU64 = AtomicU64::new(0);
/// Số lần ngữ cảnh thứ hai **lấy được** khoá trong khi BSP đang giữ — phải là `0`.
static PROBE_BAD: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ handler IPI đã chạy xong phép thử (biến đếm để BSP chờ).
static PROBE_DONE: AtomicU64 = AtomicU64::new(0);

/// Handler IPI gọi **trong ngắt** (qua `trap_dispatch` của arch). Chỉ làm việc khi
/// [`PROBE_ARMED`] = `1`; ngoài ra là no-op (IPI của TAP 20..24 và IPI tự gửi của phép đo ngắt
/// đều đi qua đây).
pub(crate) fn on_ipi() {
    if PROBE_ARMED.load(Ordering::Acquire) == 0 {
        return;
    }
    match TEST_LOCK.try_lock_irqsave() {
        Some(guard) => {
            // Khoá **không** loại trừ: ngữ cảnh thứ hai vào được khi BSP đang giữ.
            PROBE_BAD.fetch_add(1, Ordering::AcqRel);
            drop(guard);
        }
        None => PROBE_DENIED.store(1, Ordering::Release),
    }
    PROBE_DONE.store(1, Ordering::Release);
}

/// Quay tới khi `cond()` đúng hoặc hết `limit` vòng; trả kết quả **kiểm lại** sau khi hết trần.
fn spin_until(mut cond: impl FnMut() -> bool, limit: u64) -> bool {
    let mut n = 0u64;
    while n < limit {
        if cond() {
            return true;
        }
        n += 1;
        core::hint::spin_loop();
    }
    cond()
}

/// Số trap IPI **đã vào `stvec`** trên hart đang chạy (bộ đếm thật của arch, không phải biến của
/// phép kiểm).
fn ipi_traps() -> u64 {
    let hart = PLATFORM.hart_id();
    IPI_STATS.get(hart).snapshot().traps
}

/// In hai dòng `# lock:` + phát TAP (`lock: mcs irq guard`) nếu cả hai phép đo xanh.
pub(crate) fn lock_phase(fails: &mut usize, tap: usize) -> bool {
    let hart = PLATFORM.hart_id();
    // Ngữ cảnh thứ hai phải là **ngắt thật**: cài callback vào handler IPI của arch (hook
    // test-only, `#[cfg(feature = "ktest")]` — ảnh sản phẩm không có nhánh này).
    ipi::set_ipi_test_hook(on_ipi);

    // ---- 1. Loại trừ hai ngữ cảnh: BSP giữ khoá, **handler ngắt** thử lấy (trap thật của arch).
    PROBE_ARMED.store(1, Ordering::SeqCst);
    PROBE_DENIED.store(0, Ordering::SeqCst);
    PROBE_BAD.store(0, Ordering::SeqCst);
    PROBE_DONE.store(0, Ordering::SeqCst);
    // Đột biến (ii): chỉ bỏ khoá **trong phạm vi** phép thử để không làm gãy `core_bridge` của
    // cả boot — TAP vẫn là ca phải ĐỎ và `overlap` đo đúng "hai ngữ cảnh cùng vào".
    #[cfg(feature = "mut-no-lock")]
    vifast_plat_riscv64::irq_lock::mut_arm_no_lock();
    let base_a = ipi_traps();
    let held = TEST_LOCK.lock_irqsave_bounded(ACQUIRE_SPIN_LIMIT);
    // Bật lại `SIE` **cố ý** trong vùng găng: đây là ngữ cảnh mà D2-238 lo — handler chạy trên
    // chính hart đang giữ khoá. `SSIE` phải bật để ngắt phần mềm có đường vào `stvec`.
    let armed_ipi = ipi::enable_ipi();
    let sent = armed_ipi && ipi::send_ipi_raw(hart_bit(hart)) == 0;
    let waited = spin_until(|| PROBE_DONE.load(Ordering::Acquire) == 1, WAIT_SPIN_LIMIT);
    let denied = PROBE_DENIED.load(Ordering::Acquire) == 1;
    let bad = PROBE_BAD.load(Ordering::Acquire);
    ipi::set_global_irq(false);
    drop(held);
    PROBE_ARMED.store(0, Ordering::SeqCst);
    #[cfg(feature = "mut-no-lock")]
    vifast_plat_riscv64::irq_lock::mut_disarm_no_lock();
    let excl_ok = sent
        && waited
        && denied
        && bad == 0
        && ipi_traps() > base_a
        && TEST_LOCK.overlap() == 0
        && TEST_LOCK.in_cs() == 0;

    // ---- 2. Ngắt bị hoãn: tự gửi IPI **trong** vùng găng `SIE = 0`.
    let enabled = ipi::enable_ipi();
    let if_before = enabled && irq_enabled();
    let base = ipi_traps();
    let mut inside_if = true;
    let mut during = base;
    let mut deferred = false;
    let mut self_sent = false;
    {
        let guard = TEST_LOCK.lock_irqsave_bounded(ACQUIRE_SPIN_LIMIT);
        if guard.is_some() {
            // Guard đã tắt `SIE` ⇒ phải tắt **trong** vùng găng.
            inside_if = irq_enabled();
            self_sent = ipi::send_ipi_raw(hart_bit(hart)) == 0;
            let mut n = 0u64;
            while n < IRQ_WINDOW_SPINS {
                n += 1;
                core::hint::spin_loop();
            }
            during = ipi_traps();
            deferred = during == base;
        }
    }
    // Guard đã khôi phục `SIE = 1`; IPI đang chờ phải được giao **bây giờ** (chờ bằng biến đếm).
    let delivered = spin_until(|| ipi_traps() > base, WAIT_SPIN_LIMIT);
    let after = ipi_traps();
    ipi::set_global_irq(false);
    let irq_ok = if_before && !inside_if && self_sent && deferred && delivered;

    // ---- 3. Đột biến (iii): guard bỏ nhánh nhả khoá ⇒ lần lấy kế tiếp **phải** chạm trần.
    #[allow(unused_variables)]
    let leak_ok = true;
    #[cfg(feature = "mut-leak-unlock")]
    let leak_ok = {
        vifast_plat_riscv64::irq_lock::mut_arm_leak();
        let leak = TEST_LOCK.lock_irqsave_bounded(ACQUIRE_SPIN_LIMIT);
        drop(leak); // `Drop` bỏ nhả khoá theo mutation
        let again = TEST_LOCK.lock_irqsave_bounded(8);
        let caught = again.is_none();
        {
            let mut w = PlatformWriter(&PLATFORM);
            if caught {
                let _ = writeln!(
                    w,
                    "# lock: BAD acquire (đột biến quên unlock — trần chờ đã bắt, không treo)"
                );
            } else {
                let _ = writeln!(w, "# lock: BAD mutate (quên unlock KHÔNG bị bắt)");
            }
        }
        caught
    };

    let overlap = TEST_LOCK.overlap();
    let mcs_ok = excl_ok && overlap == 0 && TEST_LOCK.in_cs() == 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# lock: mcs acquired={} contended={} denied={} overlap={} in_cs={} ok={}",
            TEST_LOCK.acquired(),
            TEST_LOCK.contended(),
            denied as u8,
            overlap,
            TEST_LOCK.in_cs(),
            mcs_ok as u8,
        );
        let _ = writeln!(
            w,
            "# lock: irq deferred={} delivered={} before={} during={} after={} if_before={} if_inside={} overlap={} ok={}",
            deferred as u8,
            delivered as u8,
            base,
            during,
            after,
            if_before as u8,
            inside_if as u8,
            overlap,
            irq_ok as u8,
        );
    }
    // Ở build đột biến, TAP **phải ĐỎ** (đó là mục đích của đột biến); `leak_ok` chỉ là chẩn
    // đoán "trần chờ đã bắt được".
    let ok = mcs_ok && irq_ok && leak_ok && !cfg!(feature = "mut-leak-unlock");
    if ok {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(w, "ok {} - lock: mcs irq guard", tap);
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok {} - lock: mcs irq guard: khoá/ngắt không đúng bất biến",
            tap
        );
        *fails += 1;
    }
    ok
}
