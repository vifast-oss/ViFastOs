// SPDX-License-Identifier: MIT OR Apache-2.0
//! **TAP 34 — khoá MCS bọc ngắt: loại trừ thật + ngắt bị hoãn thật** (KT-336a).
//!
//! Hai phép đo **độc lập**, mỗi phép trả lời một câu hỏi mà `run-x86_64.sh` đọc từ log:
//!
//! 1. **Loại trừ hai ngữ cảnh thật** (`# lock: mcs …`): BSP giữ [`TEST_LOCK`], rồi gửi IPI tới
//!    **hart phụ 1**. Handler IPI của hart 1 gọi [`on_ipi`] → `try_lock_irqsave` trên **cùng**
//!    khoá. Nếu khoá loại trừ đúng, hart 1 **không** lấy được (`denied=1`, `overlap=0`); nếu
//!    không, nó vào vùng găng khi BSP đang ở trong (`in_cs > 1`, `overlap > 0`) ⇒ TAP 34 ĐỎ.
//!    Bất biến **không phụ thuộc thứ tự tick**: BSP chờ **biến đếm** `PROBE_DONE` (trần số vòng),
//!    không chờ thời gian (bài học KT-308/KT-313).
//!
//! 2. **Ngắt bị hoãn** (`# lock: irq …`): bật ngắt, lấy khoá (guard `cli`), gửi **IPI cho chính
//!    BSP**, quay một cửa sổ vòng. Trong vùng găng `IF = 0` ⇒ handler **không thể** chạy ⇒
//!    `during == before`. Nhả khoá (guard khôi phục `IF = 1`) ⇒ IPI đang chờ **được giao** ⇒
//!    `after > before`. Nếu `lock` **không** tắt ngắt, handler chạy ngay trong vùng găng ⇒
//!    `during > before` ⇒ TAP 34 ĐỎ (đột biến `mut-no-irq`).
//!
//! # Vì sao phép đo này **không** phải "đo thời gian"
//!
//! - "Không giao được khi `IF = 0`" là **bảo đảm của CPU** (SDM: `IF` che mọi ngắt che được),
//!   không phải "thường đúng"; cửa sổ vòng chỉ để **cho LAPIC cơ hội** giao nếu `IF` sai.
//! - "Đã giao sau khi nhả" là **biến đếm** `after > before`, chờ tới trần số vòng — chứng minh
//!   IPI **thật sự đã được xếp** (không phải ta quên gửi).
//!
//! # Đột biến
//!
//! - `mut-no-irq`: `IrqState` không `cli` ⇒ `inside_if = 1`/`deferred = 0` ⇒ ĐỎ.
//! - `mut-leak-unlock`: guard bỏ nhánh nhả khoá ⇒ lần lấy kế tiếp chạm trần
//!   `lock_irqsave_bounded` ⇒ in `# lock: BAD acquire` ⇒ TAP 34 ĐỎ (**không** treo).
//! - `mut-no-lock`: bỏ phần MCS ⇒ hart 1 vào vùng găng khi BSP đang giữ ⇒ `overlap > 0` ⇒ ĐỎ.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::irq_lock::IrqMcsLock;
use vifast_plat_x86_64::smp as psmp;

use crate::{asm, check, Console};

/// Trần **số vòng** chờ sự kiện từ hart khác — hết trần ⇒ TAP ĐỎ, **không treo**.
const WAIT_SPIN_LIMIT: u64 = 50_000_000;
/// Cửa sổ vòng trong vùng găng: đủ để LAPIC giao IPI nếu `IF` sai; `IF = 0` thì không thể giao.
const IRQ_WINDOW_SPINS: u64 = 2_000_000;
/// Trần của lần lấy khoá test — deadlock thành `None` có chẩn đoán, không chờ vô hạn.
const ACQUIRE_SPIN_LIMIT: u32 = 50_000_000;

/// Khoá **của riêng ca kiểm** — không dùng chung với COM1/scheduler để phép đo độc lập.
static TEST_LOCK: IrqMcsLock<()> = IrqMcsLock::new(());
/// `1` ⇔ IPI kế tiếp tới hart phụ phải thử lấy [`TEST_LOCK`].
static PROBE_ARMED: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ hart phụ **đã** thử và bị từ chối (bằng chứng loại trừ).
static PROBE_DENIED: AtomicU64 = AtomicU64::new(0);
/// Số lần hart phụ **lấy được** khoá trong khi BSP đang giữ — phải là `0`.
static PROBE_BAD: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ handler IPI của hart phụ đã chạy xong phép thử (biến đếm để BSP chờ).
static PROBE_DONE: AtomicU64 = AtomicU64::new(0);

/// Handler IPI gọi **trong ngắt** của hart đích — phép thử loại trừ chéo hart.
///
/// Chỉ làm việc khi [`PROBE_ARMED`] = `1`; ngoài ra là no-op (IPI của TAP 20 và IPI tự gửi của
/// phép đo ngắt đều đi qua đây).
pub(crate) fn on_ipi() {
    if PROBE_ARMED.load(Ordering::Acquire) == 0 {
        return;
    }
    match TEST_LOCK.try_lock_irqsave() {
        Some(guard) => {
            // Khoá **không** loại trừ: hart phụ vào được khi BSP đang giữ.
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

/// In hai dòng `# lock:` + phát TAP `tap` (`lock: mcs irq guard`).
pub(crate) fn report(console: &mut Console, tap: usize) {
    // ---- 1. Loại trừ hai ngữ cảnh: BSP giữ khoá, hart 1 thử lấy (trong handler IPI).
    PROBE_ARMED.store(1, Ordering::SeqCst);
    PROBE_DENIED.store(0, Ordering::SeqCst);
    PROBE_BAD.store(0, Ordering::SeqCst);
    PROBE_DONE.store(0, Ordering::SeqCst);
    // Đột biến (iii): chỉ bỏ khoá **trong phạm vi** phép thử để không làm gãy scheduler/COM1
    // của cả boot — TAP 34 vẫn là ca phải ĐỎ và `overlap` đo đúng "hai ngữ cảnh cùng vào".
    #[cfg(feature = "mut-no-lock")]
    vifast_plat_x86_64::irq_lock::mut_arm_no_lock();
    let held = TEST_LOCK.lock_irqsave_bounded(ACQUIRE_SPIN_LIMIT);
    let ap_id = psmp::apic_id_for_hart(1).unwrap_or(1);
    let sent = crate::smp::send_test_ipi(ap_id);
    let waited = spin_until(|| PROBE_DONE.load(Ordering::Acquire) == 1, WAIT_SPIN_LIMIT);
    let denied = PROBE_DENIED.load(Ordering::Acquire) == 1;
    let bad = PROBE_BAD.load(Ordering::Acquire);
    drop(held);
    PROBE_ARMED.store(0, Ordering::SeqCst);
    #[cfg(feature = "mut-no-lock")]
    vifast_plat_x86_64::irq_lock::mut_disarm_no_lock();
    let excl_ok =
        sent && waited && denied && bad == 0 && TEST_LOCK.overlap() == 0 && TEST_LOCK.in_cs() == 0;

    // ---- 2. Ngắt bị hoãn: IPI cho chính BSP, gửi **trong** vùng găng `IF = 0`.
    asm::sti();
    let if_before = asm::interrupts_enabled();
    let base = crate::smp::ipi_count();
    let mut inside_if = true;
    let mut during = base;
    let mut deferred = false;
    let mut self_sent = false;
    {
        let guard = TEST_LOCK.lock_irqsave_bounded(ACQUIRE_SPIN_LIMIT);
        if guard.is_some() {
            // Guard đã `cli` ⇒ `IF` phải tắt **trong** vùng găng.
            inside_if = asm::interrupts_enabled();
            let bsp_id = crate::smp::bsp_lapic_id() as u8;
            self_sent = crate::smp::send_test_ipi(bsp_id);
            let mut n = 0u64;
            while n < IRQ_WINDOW_SPINS {
                n += 1;
                core::hint::spin_loop();
            }
            during = crate::smp::ipi_count();
            deferred = during == base;
        }
    }
    // Guard đã khôi phục `IF = 1`; IPI đang chờ phải được giao **bây giờ** (chờ bằng biến đếm).
    let delivered = spin_until(|| crate::smp::ipi_count() > base, WAIT_SPIN_LIMIT);
    let after = crate::smp::ipi_count();
    asm::cli();
    let irq_ok = if_before && !inside_if && self_sent && deferred && delivered;

    // ---- 3. Đột biến (ii): guard bỏ nhánh nhả khoá ⇒ lần lấy kế tiếp **phải** chạm trần.
    #[allow(unused_variables)]
    let leak_ok = true;
    #[cfg(feature = "mut-leak-unlock")]
    let leak_ok = {
        vifast_plat_x86_64::irq_lock::mut_arm_leak();
        let leak = TEST_LOCK.lock_irqsave_bounded(ACQUIRE_SPIN_LIMIT);
        drop(leak); // `Drop` bỏ nhả khoá theo mutation
        let again = TEST_LOCK.lock_irqsave_bounded(8);
        let caught = again.is_none();
        if caught {
            let _ = writeln!(
                console,
                "# lock: BAD acquire (đột biến quên unlock — trần chờ đã bắt, không treo)"
            );
        } else {
            let _ = writeln!(console, "# lock: BAD mutate (quên unlock KHÔNG bị bắt)");
        }
        caught
    };

    let mcs_ok = excl_ok && TEST_LOCK.overlap() == 0 && TEST_LOCK.in_cs() == 0;
    let _ = writeln!(
        console,
        "# lock: mcs acquired={} contended={} denied={} overlap={} in_cs={} ok={}",
        TEST_LOCK.acquired(),
        TEST_LOCK.contended(),
        denied as u8,
        TEST_LOCK.overlap(),
        TEST_LOCK.in_cs(),
        mcs_ok as u8,
    );
    let _ = writeln!(
        console,
        "# lock: irq deferred={} delivered={} before={} during={} after={} if_before={} if_inside={} ok={}",
        deferred as u8,
        delivered as u8,
        base,
        during,
        after,
        if_before as u8,
        inside_if as u8,
        irq_ok as u8,
    );
    // Ở build đột biến, TAP **phải ĐỎ** (đó là mục đích của đột biến); `leak_ok` chỉ là chẩn
    // đoán "trần chờ đã bắt được".
    let ok = mcs_ok && irq_ok && leak_ok && !cfg!(feature = "mut-leak-unlock");
    check(console, tap, "lock: mcs irq guard", ok);
}
