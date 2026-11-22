// SPDX-License-Identifier: MIT OR Apache-2.0
//! IPI thật trên QEMU `virt` (KT-50): SBI `send_ipi` + ACLINT MSWI + drain hộp thư per-hart.
//!
//! Phần **ngữ nghĩa** (kind, coalesce, `KindSet`, bộ đếm, `wait_with_drain`, quyết định
//! self-target) nằm ở [`vifast_plat_common::ipi`] — test được trên host. Tệp này chỉ là phần
//! **không viết "safe" được** (D2-17, `unsafe` thuộc `kernel/vifast-plat-*`):
//!
//! - **SBI IPI** (`EID = 0x735049`, FID 0) qua `ecall`: gửi `hart_mask` (bit thứ `h` = hart `h`,
//!   `hart_mask_base = 0`) — SBI v2.0 §6.1. OpenSBI đánh thức hart đích bằng IPI M-mode của
//!   firmware rồi bật **SSIP** của S-mode ⇒ hart đích nhận `scause = (1 << 63) | 1`;
//!   `trap_dispatch` decode thành [`vifast_plat_common::trap::Event::Ipi`].
//! - **Trạng thái ngắt phần mềm: `sip.SSIP`** (KHÔNG đọc ACLINT MSWI). Đo trên QEMU 11.1.3 +
//!   OpenSBI v1.8.1 (`-smp 2`): đọc `0x0200_0004` (MSIP hart 1) từ S-mode bị **load access
//!   fault** (`scause = 5`, `tval = 0x0200_0004`) — PMP của firmware không mở ACLINT cho S-mode
//!   (log `v2/agent/JOURNAL/kt50-aclint-pmp.log`). `sip.SSIP` là **cùng trạng thái** theo
//!   privileged spec và S-mode được đọc/ghi: handler đọc SSIP **trước** khi xoá (chứng minh ngắt
//!   có nguồn thật — `spurious` nếu không) rồi `csrc sip, SSIP` để dập ngắt. Hằng ACLINT vẫn ở
//!   `plat-common` làm dữ kiện nền tảng + host test; `mmu::map_mmio` không map CLINT (như KT-47).
//! - **Hộp thư** [`vifast_plat_common::ipi::IPI_MAILBOXES`] theo hart: `post` đặt kind **trước**
//!   khi gửi IPI (không mất kind), gửi trùng ⇒ coalesce (không gửi IPI thứ hai), `drain` một
//!   `swap` giao đủ kind. **Self-target** đi đường `IpiDelivery::Local` — local flush + ack
//!   ngầm, **không** IPI (D2-68), nên `sent`/`traps` không tăng cho hart tự gửi. Hart đích
//!   **hợp lệ nhưng chưa online** ⇒ `Invalid` + log `# ipi: hart N chua online` (**D2-77**):
//!   không gọi SBI, không giữ kind trong hộp thư; SBI lỗi ⇒ **gỡ bit vừa post** (không mồ côi).
//! - **Drain cả khi đang chờ** (D2-68): [`wait_drain`] bọc
//!   [`vifast_plat_common::ipi::wait_with_drain`] — mỗi bước `drain_current()` (drain hộp thư +
//!   xoá `sip.SSIP`) rồi mới kiểm predicate; hết `timeout_ticks` thì trả `false`, **không treo**.
//!
//! **Không** thuộc KT-50: ASID, `sfence.vma`, shootdown ack đầy đủ (KT-51) — [`Kind::TlbFlush`]
//! chỉ là **khung** kind + drain; preemption/scheduler đa hart (K3b). Xem `v2/docs/k3-riscv64.md`
//! §13.
//!
//! [`Kind::TlbFlush`]: vifast_plat_common::ipi::IpiKind::TlbFlush

use core::arch::asm;
use core::fmt::Write;

use vifast_plat_common::ipi::{
    classify_post, deadline_after, wait_with_drain, IpiKind, KindSet, PostDecision, WaitOutcome,
    IPI_ERR_HART_OFFLINE, IPI_ERR_HART_OUT_OF_RANGE, IPI_MAILBOXES, IPI_STATS, SBI_EXT_IPI,
    SBI_IPI_SEND_IPI, SIE_SSIE,
};
use vifast_plat_common::smp::{hart_bit, valid_hart};
use vifast_plat_common::trap::SSTATUS_SIE;
use vifast_plat_common::{Platform, PlatformWriter};

use crate::smp;
use crate::timer;
use crate::Riscv64Virt;

/// Kết quả một lần `post` — TAP đọc được đường đi (remote/coalesce/local/lỗi).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostOutcome {
    /// Đã gửi SBI IPI (kind mới, hart đích khác hart gửi).
    Remote,
    /// Kind đã đang chờ ở hart đích ⇒ **gộp**, không gửi IPI thứ hai (không mất kind).
    Coalesced,
    /// Self-target ⇒ local flush + ack ngầm, **không** IPI (D2-68).
    Local,
    /// Hart đích không hợp lệ hoặc SBI trả lỗi — đếm `errors`, không panic.
    Invalid,
}

/// Hart đang chạy (`tp` do `_start`/`_start_secondary` đặt từ `a0`).
fn current_hart() -> usize {
    Riscv64Virt.hart_id()
}

// ---------------------------------------------------------------------------------------------
// SBI IPI
// ---------------------------------------------------------------------------------------------

/// `sbi_send_ipi(hart_mask, hart_mask_base)` — trả mã SBI (`0` = thành công).
///
/// `a0` = `hart_mask`, `a1` = `hart_mask_base` (0 ⇒ bit thứ `h` là hart `h`); SBI trả
/// `(a0 = error, a1 = value)` nên `a1` phải khai báo `inout` (firmware được phép ghi).
fn sbi_send_ipi(hart_mask: u64) -> i64 {
    let mut error: u64 = 0;
    let base: u64 = vifast_plat_common::ipi::IPI_HART_MASK_BASE;
    let mut value: u64 = 0;
    // SAFETY: `ecall` SBI IPI hợp lệ (OpenSBI v1.8.1 có extension `ipi`); SBI bảo toàn mọi thanh
    // ghi trừ `a0`/`a1`; `ecall` có thể ghi bộ nhớ phía firmware nên **không** `nomem`.
    unsafe {
        asm!(
            "ecall",
            in("a7") SBI_EXT_IPI,
            in("a6") SBI_IPI_SEND_IPI,
            inout("a0") hart_mask => error,
            inout("a1") base => value,
            options(nostack),
        );
    }
    let _ = value;
    error as i64
}

/// Gửi IPI **thẳng** tới `hart_mask` qua SBI, cộng `sent` của hart hiện tại (đường test/raw —
/// không qua hộp thư). Trả mã SBI.
///
/// Dùng cho phép đo "SBI IPI + trap thật" ở cấu hình **1 hart** (không có hart đích để chéo);
/// đường sản phẩm luôn đi [`post`] để giữ coalesce + cấm self-target (D2-68).
pub fn send_ipi_raw(hart_mask: u64) -> i64 {
    let code = sbi_send_ipi(hart_mask);
    if code == 0 {
        IPI_STATS.get(current_hart()).record_sent();
    } else {
        IPI_STATS.get(current_hart()).record_error(code);
    }
    code
}

/// Gửi IPI tới một hart qua SBI (không qua hộp thư) — tiện cho test/log.
pub fn send_ipi(hart: usize) -> i64 {
    if !valid_hart(hart) {
        return -3;
    }
    send_ipi_raw(hart_bit(hart))
}

// ---------------------------------------------------------------------------------------------
// Trạng thái ngắt phần mềm của **hart đang chạy** — CSR `sip.SSIP`
// ---------------------------------------------------------------------------------------------
//
// Vì sao **không** đọc ACLINT MSWI: đo trên QEMU 11.1.3 + OpenSBI v1.8.1 (`-smp 2`), đọc
// `0x0200_0004` từ S-mode bị **load access fault** (`scause=5`, `stval=0x0200_0004`) — PMP của
// firmware không mở ACLINT cho S-mode. `sip.SSIP` (bit 1) là **cùng một trạng thái** MSIP theo
// privileged spec và S-mode được phép đọc/ghi: đọc ⇒ biết ngắt phần mềm có nguồn thật (`spurious`
// nếu không), ghi 0 ⇒ dập ngắt trước `sret`. Hằng ACLINT/`mswi_*` vẫn ở `plat-common` làm dữ kiện
// nền tảng + test host; arch dùng CSR. Deviation ghi ở `v2/docs/k3-riscv64.md` §13.4.

/// `true` ⇔ `sip.SSIP` của hart đang chạy đang bật (ngắt phần mềm có nguồn thật).
pub fn ipi_pending_current() -> bool {
    let value: u64;
    // SAFETY: chỉ đọc CSR `sip`, không tác dụng phụ.
    unsafe { asm!("csrr {}, sip", out(reg) value, options(nomem, nostack)) };
    value & SIE_SSIE != 0
}

/// Xoá `sip.SSIP` của hart đang chạy (dập ngắt phần mềm trước khi `sret`).
pub fn clear_ipi_current() {
    // SAFETY: ghi bit SSIP của `sip` trên hart hiện tại — privileged spec cho S-mode ghi SSIP.
    unsafe { asm!("csrc sip, {}", in(reg) SIE_SSIE, options(nomem, nostack)) };
}

// ---------------------------------------------------------------------------------------------
// Cổng ngắt phần mềm (CSR per hart)
// ---------------------------------------------------------------------------------------------

/// Bật cổng IPI: `sie.SSIE = 1` + `sstatus.SIE = 1`; trả `true` ⇔ đọc lại thấy cả hai.
///
/// Chỉ gọi ở **ngữ cảnh chủ động** (không trong trap handler) — P2a của KT-45: handler không ghi
/// `sstatus`; việc bật/tắt cổng là của code thường.
pub fn enable_ipi() -> bool {
    // SAFETY: ghi CSR `sie`/`sstatus` của hart hiện tại; giá trị chỉ gồm bit enable ngắt phần mềm.
    unsafe {
        asm!("csrs sie, {}", in(reg) SIE_SSIE, options(nomem, nostack));
        asm!("csrs sstatus, {}", in(reg) SSTATUS_SIE, options(nomem, nostack));
    }
    timer::read_sie() & SIE_SSIE != 0 && timer::read_sstatus() & SSTATUS_SIE != 0
}

/// Tắt cổng IPI (`sie.SSIE = 0`) — `sstatus.SIE` do người gọi quyết định (timer/PLIC dùng chung).
///
/// Cặp đối xứng của [`enable_ipi`]; KT-50 dùng `set_global_irq` cho vùng chờ khoá, KT-51 sẽ dùng
/// `disable_ipi` khi tắt hẳn đường IPI lúc teardown.
pub fn disable_ipi() {
    // SAFETY: clear bit SSIE của `sie` trên hart hiện tại.
    unsafe { asm!("csrc sie, {}", in(reg) SIE_SSIE, options(nomem, nostack)) };
}

/// Bật/tắt `sstatus.SIE` (dùng cho vùng "đang giữ khoá ⇒ không nhận trap" của test drain).
pub fn set_global_irq(on: bool) {
    // SAFETY: ghi CSR `sstatus` của hart hiện tại, chỉ bit SIE.
    unsafe {
        if on {
            asm!("csrs sstatus, {}", in(reg) SSTATUS_SIE, options(nomem, nostack));
        } else {
            asm!("csrc sstatus, {}", in(reg) SSTATUS_SIE, options(nomem, nostack));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Post / drain / handler
// ---------------------------------------------------------------------------------------------

/// Ghi nhận kind đã xử lý **local** cho đường self-target (bộ đếm `local` + `kinds`).
///
/// Đây **không** phải đường shootdown: KT-51 (D2-68) làm local flush + ack ngầm trong
/// `crate::tlb::shootdown` (không self-target), còn hàm này chỉ giữ ngữ nghĩa "post kind cho chính
/// mình không sinh IPI" mà TAP 22 kiểm; nó **không** thay `sfence.vma`/ack của shootdown.
fn local_flush(kind: IpiKind) {
    IPI_STATS.get(current_hart()).record_local(kind);
}

/// Gửi `kind` tới `target` — **không self-target** (D2-68).
///
/// 1. `target` ngoài dải ⇒ `Invalid` (`errors` + [`IPI_ERR_HART_OUT_OF_RANGE`]), không panic.
/// 2. `target` **hợp lệ nhưng chưa online** ⇒ `Invalid` + log `# ipi: hart N chua online`
///    ([`IPI_ERR_HART_OFFLINE`]); **không** gọi SBI, **không** đặt kind vào hộp thư (**D2-77**:
///    không ai drain ⇒ bit mồ côi, có thể giao nhầm sau khi hart start).
/// 3. `target == hart hiện tại` ⇒ local flush + ack ngầm, **không** SBI IPI (D2-68) ⇒
///    `sent`/`traps` **không** tăng.
/// 4. Ngược lại: đặt kind vào **hộp thư đích trước**; kind mới ⇒ gửi SBI IPI (một hart) và cộng
///    `sent`; kind đang chờ ⇒ **coalesce**, không gửi IPI thứ hai. SBI lỗi ⇒ **gỡ bit vừa post**
///    ([`vifast_plat_common::ipi::IpiMailbox::unpost`]) + `errors` — không để kind mồ côi.
pub fn post(target: usize, kind: IpiKind) -> PostOutcome {
    let sender = current_hart();
    match classify_post(sender, target, smp::online_mask()) {
        PostDecision::InvalidHart => {
            IPI_STATS
                .get(sender)
                .record_error(IPI_ERR_HART_OUT_OF_RANGE);
            PostOutcome::Invalid
        }
        PostDecision::Offline => {
            log_offline(target);
            IPI_STATS.get(sender).record_error(IPI_ERR_HART_OFFLINE);
            PostOutcome::Invalid
        }
        PostDecision::Local => {
            local_flush(kind);
            PostOutcome::Local
        }
        PostDecision::Remote => {
            if !IPI_MAILBOXES.get(target).post(kind) {
                IPI_STATS.get(sender).record_coalesced();
                return PostOutcome::Coalesced;
            }
            let code = sbi_send_ipi(hart_bit(target));
            if code == 0 {
                IPI_STATS.get(sender).record_sent();
                PostOutcome::Remote
            } else {
                // D2-77: IPI không gửi được thì bit vừa post phải được gỡ — nếu để lại, kind nằm
                // trong hộp thư mà hart đích không có tín hiệu nào để drain.
                let _ = IPI_MAILBOXES.get(target).unpost(kind);
                IPI_STATS.get(sender).record_error(code);
                PostOutcome::Invalid
            }
        }
    }
}

/// Log khi `post` tới hart **hợp lệ nhưng chưa online** (D2-77) — một dòng `#` để runner/harness
/// không nhầm là TAP; dùng khoá UART của arch vì AP cũng có thể gọi.
fn log_offline(target: usize) {
    crate::debug_lock_do(|| {
        let mut writer = PlatformWriter(&Riscv64Virt);
        let _ = writeln!(
            writer,
            "# ipi: hart {} chua online, khong gui (D2-77)",
            target
        );
    });
}

/// Xoá `sip.SSIP` rồi `swap` toàn bộ kind của hộp thư **hart đang chạy**, ghi `delivered` +
/// `kinds`. Kind nằm trong hộp thư (không phải trong SSIP) nên xoá SSIP trước không làm mất kind.
fn drain_hart_current() -> KindSet {
    let hart = current_hart();
    clear_ipi_current();
    let kinds = IPI_MAILBOXES.get(hart).drain();
    IPI_STATS.get(hart).record_drain(kinds);
    // KT-51: mọi đường drain (trap handler, vòng chờ khoá, `park_forever`) đều **xử lý yêu cầu
    // `TlbFlush`** nếu có — kind `TlbFlush` chỉ là "đánh thức", phạm vi flush nằm ở slot yêu cầu
    // của hart (`crate::tlb`). Nếu không xử lý ở đây, hart park hoặc đang chờ khoá sẽ bỏ yêu cầu và
    // shooter sẽ timeout oan.
    let _ = crate::tlb::handle_kinds(kinds);
    kinds
}

/// Drain hộp thư của **hart đang chạy** — gọi được cả trong trap handler lẫn vòng chờ khoá.
pub fn drain_current() -> KindSet {
    drain_hart_current()
}

// ---------------------------------------------------------------------------------------------
// Hook **test-only** (KT-336c) — ca kiểm khoá cần chạy **trong ngữ cảnh ngắt thật**
// ---------------------------------------------------------------------------------------------

/// `0` = chưa cài; khác `0` = địa chỉ `fn()` do ảnh `ktest` cài ([`set_ipi_test_hook`]).
///
/// Vì sao cần hook: `trap_dispatch` của arch gọi [`on_ipi_interrupt`] trực tiếp — kbin (nơi có
/// `ktest`) **không** có cửa nào chen vào ngữ cảnh ngắt. Ca kiểm `KT-336c` phải chạy `try_lock`
/// **trong handler** mới chứng minh được "luồng thường vs ngắt" của D2-238. Hook chỉ tồn tại khi
/// bật feature `ktest` ⇒ ảnh sản phẩm **không** có nhánh/chuỗi nào (D2-08).
#[cfg(feature = "ktest")]
static IPI_TEST_HOOK: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Cài callback chạy **trong** ngữ cảnh ngắt IPI (ảnh `ktest`; xem [`IPI_TEST_HOOK`]).
#[cfg(feature = "ktest")]
pub fn set_ipi_test_hook(f: fn()) {
    IPI_TEST_HOOK.store(f as usize, core::sync::atomic::Ordering::Release);
}

#[cfg(feature = "ktest")]
fn run_ipi_test_hook() {
    let p = IPI_TEST_HOOK.load(core::sync::atomic::Ordering::Acquire);
    if p != 0 {
        // SAFETY: giá trị duy nhất được ghi vào static này là con trỏ hàm từ
        // `set_ipi_test_hook`; địa chỉ hàm nằm trong `.text` của ảnh nên sống suốt thời gian boot.
        // `fn()` là con trỏ hàm không bắt ngữ cảnh, truyền qua `usize` (cùng kiến trúc 64-bit).
        let f: fn() = unsafe { core::mem::transmute::<usize, fn()>(p) };
        f();
    }
}

/// Handler IPI (`Event::Ipi`, `scause = (1<<63)|1`): đọc `sip.SSIP` trước khi xoá (trap "spurious"
/// nếu không có ngắt thật), rồi drain hộp thư của hart đang chạy. Trả kind đã giao (log/TAP).
pub fn on_ipi_interrupt() -> KindSet {
    let pending = ipi_pending_current();
    IPI_STATS.get(current_hart()).record_trap(pending);
    // KT-336c: hook test-only chạy **trước** drain — vùng găng của ca kiểm không dính TLB/kind.
    #[cfg(feature = "ktest")]
    run_ipi_test_hook();
    drain_hart_current()
}

/// Số kind đang chờ trong hộp thư của `hart` (không drain).
pub fn pending_of(hart: usize) -> KindSet {
    IPI_MAILBOXES.get(hart).pending()
}

/// Wrapper [`wait_with_drain`] cho ngữ cảnh arch: `timeout_ticks` tính bằng **timebase** (10 MHz,
/// D2-72 Q6); mỗi bước drain hộp thư của hart đang chạy rồi kiểm `ready`.
///
/// Trả `false` ⇔ hết timeout (hart kia chậm/chết) — **không treo** máy, để TAP đỏ rõ ràng.
pub fn wait_drain(timeout_ticks: u64, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = deadline_after(timer::now_ticks(), timeout_ticks);
    let outcome = wait_with_drain(
        deadline,
        timer::now_ticks,
        || {
            let _ = drain_current();
        },
        &mut ready,
    );
    outcome == WaitOutcome::Ready
}
