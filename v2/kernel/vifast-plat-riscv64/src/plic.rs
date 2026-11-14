// SPDX-License-Identifier: MIT OR Apache-2.0
//! PLIC **cổ điển** trên QEMU `virt` — KT-48 (MMIO/CSR; logic thuần ở
//! `vifast-plat-common::plic`).
//!
//! # Phạm vi
//!
//! - `init`: **probe `ndev`** từ register (ghi 1/đọc lại trên priority, trả 0 — không giả định
//!   95), đặt `threshold = 0`, `priority[UART0_IRQ] = 1`, bật bit enable của context S-mode
//!   `2*hart+1`, kiểm context M-mode **không** bị bật, trả [`PlicReport`] (giá trị **đọc lại** —
//!   bằng chứng cho TAP, không phải hằng số viết tay).
//! - `claim`/`complete`: đọc `claim/complete` của context hiện tại; ghi complete **chỉ** khi
//!   claim ≠ 0 (PLIC spec).
//! - **UART0 IRQ 10**: bật `IER.TX-empty` (mức cao khi THR rỗng), handler claim → đọc `IIR` →
//!   **re-arm** (`IER` 0 → 2 tạo level transition, vì PLIC level-triggered không tự pending lại
//!   khi nguồn còn cao) → complete; đủ [`UART_IRQ_LIMIT`] ngắt thì tắt `IER` + clear enable
//!   (không ngập). Line lạ: đếm + complete (không treo).
//! - Cổng CPU external: `sie.SEIE` **trước** rồi `sstatus.SIE` (đối xứng `timer::enable_timer` —
//!   P2a: trap entry/handler **không** ghi `sstatus`).
//!
//! Không thuộc KT-48: IPI (KT-50), SMP/context hart khác (KT-49), MSI/AIA, EOI khác.
//!
//! `unsafe` (MMIO/CSR) nằm trong danh sách đóng `kernel/vifast-plat-*` của D2-17.

use core::arch::asm;

use vifast_plat_common::plic::{
    claim_offset, context_of_hart, enable_bit, enable_offset, machine_context_of_hart,
    ndev_covers_uart, priority_offset, threshold_offset, uart_limit_reached, ClaimOutcome,
    PLIC_HIGH_WINDOW_BYTES, PLIC_LOW_WINDOW_BYTES, PLIC_MAX_SOURCES, PLIC_STATS, PLIC_THRESHOLD,
    PLIC_UART_PRIORITY, UART_IRQ_LIMIT,
};
use vifast_plat_common::trap::SSTATUS_SIE;
use vifast_plat_common::Platform;

use crate::timer::{read_sie, read_sstatus};
use crate::{Riscv64Virt, PLIC_BASE, UART0_BASE, UART0_IRQ};

/// `sie.SEIE` — supervisor **external** interrupt enable (bit 9). Nguồn PLIC.
pub const SIE_SEIE: u64 = 1 << 9;

/// Thanh ghi 16550A (offset trong trang UART): THR/RBR, IER, IIR/FCR.
const UART_IER: usize = 1;
const UART_IIR: usize = 2;
/// `IER` bit 1 — THR empty (TX-empty) interrupt enable.
pub const UART_IER_TX_EMPTY: u8 = 0x02;
/// `IIR` mã TX-empty (bit 3..1 = 010): nguồn của ngắt UART0 thật trong KT-48. So sánh **4 bit
/// nguồn** (`IIR & 0x0f`) vì bit FIFO 7:6 do firmware quyết định (OpenSBI bật FIFO ⇒ 0xc2).
pub const UART_IIR_THRE: u32 = 0x02;

/// Đọc 32-bit MMIO trong cửa sổ PLIC tại `offset` từ `PLIC_BASE`.
#[inline]
fn plic_read(offset: u64) -> u32 {
    // SAFETY: `PLIC_BASE + offset` nằm trong hai cửa sổ 2 MiB Device RW NX đã map ở KT-48
    // (`mmu::map_mmio`); offset do các hàm const của `plat-common::plic` sinh.
    unsafe { core::ptr::read_volatile((PLIC_BASE as u64 + offset) as *const u32) }
}

/// Ghi 32-bit MMIO trong cửa sổ PLIC.
#[inline]
fn plic_write(offset: u64, value: u32) {
    // SAFETY: như `plic_read`; chỉ ghi trong cửa sổ PLIC đã map.
    unsafe { core::ptr::write_volatile((PLIC_BASE as u64 + offset) as *mut u32, value) };
}

/// Đọc thanh ghi 8-bit của UART0 (`UART0_BASE` map 4 KiB Device từ KT-46).
#[inline]
fn uart_read(offset: usize) -> u8 {
    // SAFETY: `UART0_BASE + offset` nằm trong trang UART Device đã map; offset < 8.
    unsafe { core::ptr::read_volatile((UART0_BASE + offset) as *const u8) }
}

/// Ghi thanh ghi 8-bit của UART0.
#[inline]
fn uart_write(offset: usize, value: u8) {
    // SAFETY: như `uart_read`; chỉ ghi trong trang UART.
    unsafe { core::ptr::write_volatile((UART0_BASE + offset) as *mut u8, value) };
}

/// Hàng rào bộ nhớ cho MMIO: bảo đảm thứ tự các ghi/đọc thiết bị trước khi phụ thuộc vào nhau
/// (vd enable bit PLIC xong mới mở cổng `sstatus.SIE`; claim xong mới complete).
#[inline]
fn fence() {
    // SAFETY: `fence` chỉ là hàng rào thứ tự bộ nhớ, không truy cập địa chỉ nào.
    unsafe { asm!("fence") };
}

/// Hart hiện tại lấy từ `tp` (đặt bởi `_start`) qua [`Platform::hart_id`].
fn current_hart() -> usize {
    Riscv64Virt.hart_id()
}

/// Context S-mode `2*hart+1` của hart đang chạy (D2-72 Q7).
pub fn current_context() -> u32 {
    context_of_hart(current_hart())
}

/// Báo cáo PLIC của lần boot — mọi trường là giá trị **đọc lại** từ register.
#[derive(Clone, Copy, Debug)]
pub struct PlicReport {
    /// `PLIC_BASE` (dữ kiện nền tảng, D2-72 Q7).
    pub base: u64,
    /// Hart đang chạy.
    pub hart: usize,
    /// Context S-mode `2*hart+1`.
    pub context: u32,
    /// Context M-mode `2*hart` — kiểm **không** bật (phải là 0).
    pub machine_context: u32,
    /// Số nguồn đọc được bằng probe priority (không giả định 95).
    pub ndev: u32,
    /// `threshold` đọc lại của context S-mode (phải 0).
    pub threshold: u32,
    /// `priority[UART0_IRQ]` đọc lại (phải 1).
    pub priority_uart: u32,
    /// Bit enable của `UART0_IRQ` trong context S-mode đọc lại (phải bật).
    pub enable_ctx: bool,
    /// Bit enable của `UART0_IRQ` trong context M-mode đọc lại (phải **tắt**).
    pub enable_mmode: bool,
    /// Claim đọc lúc chưa bật nguồn (phải 0 — không có ngắt chờ).
    pub claim_idle: u32,
    /// Địa chỉ đăng ký enable đã dùng (bằng chứng offset).
    pub enable_address: u64,
    /// Địa chỉ context (threshold/claim) đã dùng.
    pub context_address: u64,
    /// Cửa sổ mapping thấp (2 MiB) `[start, end)`.
    pub low_window: (u64, u64),
    /// Cửa sổ mapping cao (2 MiB) `[start, end)`.
    pub high_window: (u64, u64),
}

impl PlicReport {
    /// `true` ⇔ cấu hình đúng hợp đồng KT-48 (D2-72 Q7/Q8): `ndev` phủ UART, context đúng hart,
    /// threshold 0, priority 1, enable đúng context S-mode, **không** context M-mode, claim rỗng.
    pub fn is_configured(&self) -> bool {
        self.ndev > UART0_IRQ
            && self.context == context_of_hart(self.hart)
            && self.machine_context == machine_context_of_hart(self.hart)
            && self.threshold == PLIC_THRESHOLD
            && self.priority_uart == PLIC_UART_PRIORITY
            && self.enable_ctx
            && !self.enable_mmode
            && self.claim_idle == 0
    }
}

/// Probe số nguồn: PLIC spec **không** có thanh ghi `ndev`; QEMU chỉ hiện thực priority cho
/// nguồn `1..=ndev` và trả 0 cho nguồn không tồn tại. Ghi `1` rồi đọc lại, trả priority về `0`
/// ngay; dừng ở nguồn đầu tiên **không** giữ giá trị ⇒ `ndev` (không giả định 95, D2-72 Q7).
///
/// K3 **sở hữu** PLIC (chưa nguồn nào dùng priority khác 0) nên probe trả mọi priority đã ghi về
/// 0 là an toàn; `init` đặt lại priority UART = 1 sau probe. **D2-75** (2026-11-15): probe được
/// phép ở **cả ảnh sản phẩm** với điều kiện **không phá trạng thái** — mỗi nguồn write-1 →
/// read-back → ghi 0 → đọc lại 0, panic nếu khôi phục thất bại.
pub fn probe_ndev() -> u32 {
    let mut ndev = 0u32;
    let mut irq = 1u32;
    while irq <= PLIC_MAX_SOURCES {
        // D2-75: probe **chẩn đoán không phá huỷ** — write-1 → read-back → ghi 0 → đọc lại 0.
        // Khôi phục thất bại ⇒ panic (không bao giờ chạy tiếp với priority rác).
        plic_write(priority_offset(irq), PLIC_UART_PRIORITY);
        let readback = plic_read(priority_offset(irq));
        plic_write(priority_offset(irq), 0);
        let restored = plic_read(priority_offset(irq));
        if restored != 0 {
            panic!(
                "plic: probe khong khoi phuc priority irq {} = {}",
                irq, restored
            );
        }
        if readback != PLIC_UART_PRIORITY {
            break;
        }
        ndev = irq;
        irq += 1;
    }
    fence();
    ndev
}

/// Khởi tạo PLIC cho hart hiện tại: probe `ndev`, threshold 0, priority IRQ 10 = 1, enable IRQ 10
/// ở context S-mode; trả các giá trị **đọc lại** cho TAP. Panic nếu `ndev` không phủ UART0.
pub fn init() -> PlicReport {
    let hart = current_hart();
    let context = context_of_hart(hart);
    let machine_context = machine_context_of_hart(hart);
    let ndev = probe_ndev();
    if !ndev_covers_uart(ndev, UART0_IRQ) {
        // Không có nguồn UART trên PLIC ⇒ mọi test IRQ sẽ vô nghĩa; dừng sạch thay vì treo.
        panic!("plic: ndev {} khong phu UART0_IRQ {}", ndev, UART0_IRQ);
    }

    plic_write(threshold_offset(context), PLIC_THRESHOLD);
    let threshold = plic_read(threshold_offset(context));
    plic_write(priority_offset(UART0_IRQ), PLIC_UART_PRIORITY);
    let priority_uart = plic_read(priority_offset(UART0_IRQ));

    let enable_address = PLIC_BASE as u64 + enable_offset(context, UART0_IRQ);
    let enable_word = plic_read(enable_offset(context, UART0_IRQ));
    plic_write(
        enable_offset(context, UART0_IRQ),
        enable_word | enable_bit(UART0_IRQ),
    );
    fence();
    let enable_ctx = plic_read(enable_offset(context, UART0_IRQ)) & enable_bit(UART0_IRQ) != 0;
    let enable_mmode =
        plic_read(enable_offset(machine_context, UART0_IRQ)) & enable_bit(UART0_IRQ) != 0;
    let context_address = PLIC_BASE as u64 + claim_offset(context);
    // Claim đọc lúc chưa bật nguồn: phải 0 (không "nuốt" ngắt nào) — bằng chứng TAP.
    let claim_idle = plic_read(claim_offset(context));

    PlicReport {
        base: PLIC_BASE as u64,
        hart,
        context,
        machine_context,
        ndev,
        threshold,
        priority_uart,
        enable_ctx,
        enable_mmode,
        claim_idle,
        enable_address,
        context_address,
        low_window: (PLIC_BASE as u64, PLIC_BASE as u64 + PLIC_LOW_WINDOW_BYTES),
        high_window: (
            PLIC_BASE as u64 + PLIC_LOW_WINDOW_BYTES,
            PLIC_BASE as u64 + PLIC_LOW_WINDOW_BYTES + PLIC_HIGH_WINDOW_BYTES,
        ),
    }
}

/// Bật bit enable của `irq` cho context S-mode hart hiện tại; `true` ⇔ **đọc lại** thấy bit.
///
/// Đây là cài đặt `Platform::enable_irq` (KT-48); priority/threshold do [`init`] đặt trước.
pub fn enable_irq(irq: u32) -> bool {
    if irq == 0 || irq > PLIC_MAX_SOURCES {
        return false;
    }
    let offset = enable_offset(current_context(), irq);
    let word = plic_read(offset);
    plic_write(offset, word | enable_bit(irq));
    fence();
    plic_read(offset) & enable_bit(irq) != 0
}

/// Tắt bit enable của `irq`; `true` ⇔ **đọc lại** thấy bit đã về 0.
pub fn disable_irq(irq: u32) -> bool {
    if irq == 0 || irq > PLIC_MAX_SOURCES {
        return false;
    }
    let offset = enable_offset(current_context(), irq);
    let word = plic_read(offset);
    plic_write(offset, word & !enable_bit(irq));
    fence();
    plic_read(offset) & enable_bit(irq) == 0
}

/// `true` ⇔ bit enable của `irq` trong context S-mode hart hiện tại đang bật (đọc register).
pub fn irq_enabled(irq: u32) -> bool {
    if irq == 0 || irq > PLIC_MAX_SOURCES {
        return false;
    }
    plic_read(enable_offset(current_context(), irq)) & enable_bit(irq) != 0
}

/// Đọc `claim/complete` của context hiện tại: IRQ đang chờ, hoặc 0 nếu không có.
pub fn claim() -> u32 {
    plic_read(claim_offset(current_context()))
}

/// Ghi complete cho `irq` (chỉ gọi với `irq != 0` — PLIC spec).
pub fn complete(irq: u32) {
    if irq != 0 {
        plic_write(claim_offset(current_context()), irq);
        fence();
    }
}

/// Bật cổng CPU cho ngắt external: `sie.SEIE = 1` **rồi** `sstatus.SIE = 1` (nguồn trước, cổng
/// sau — đối xứng `timer::enable_timer`, P2a). Trả `true` ⇔ đọc lại thấy **cả hai**.
pub fn enable_external() -> bool {
    // SAFETY: ghi hai CSR ngắt của hart hiện tại; giá trị hằng, không chạm bộ nhớ.
    unsafe {
        asm!(
            "csrs sie, {seie}",
            "csrs sstatus, {sie}",
            seie = in(reg) SIE_SEIE,
            sie = in(reg) SSTATUS_SIE,
            options(nomem, nostack, preserves_flags),
        );
    }
    read_sie() & SIE_SEIE != 0 && read_sstatus() & SSTATUS_SIE != 0
}

/// Tắt cổng CPU external: `sstatus.SIE = 0` **rồi** `sie.SEIE = 0` (đảo thứ tự [`enable_external`]).
///
/// `sstatus.SIE` là cổng **dùng chung** mọi nguồn S-mode; KT-48 gọi hàm này **trước** khi bật
/// timer (`timer::init` sẽ bật lại SIE). Khi có SMP/preemption (KT-49+) cần gate theo hart/nguồn
/// thay vì tắt SIE toàn cục như ở đây.
pub fn disable_external() {
    // SAFETY: ghi hai CSR ngắt của hart hiện tại; giá trị hằng, không chạm bộ nhớ.
    unsafe {
        asm!(
            "csrc sstatus, {sie}",
            "csrc sie, {seie}",
            sie = in(reg) SSTATUS_SIE,
            seie = in(reg) SIE_SEIE,
            options(nomem, nostack, preserves_flags),
        );
    }
}

/// `true` ⇔ cả `sie.SEIE` và `sstatus.SIE` đang bật (bằng chứng "cổng external đã mở").
pub fn external_gate_enabled() -> bool {
    read_sie() & SIE_SEIE != 0 && read_sstatus() & SSTATUS_SIE != 0
}

/// Bật UART `IER.TX-empty` (mức cao khi THR rỗng). Ghi **đúng** bit TX-empty để tắt mọi nguồn
/// RX/line khác (K3 chưa có shell; tránh "IRQ giả" từ RX). `true` ⇔ đọc lại thấy bit.
pub fn uart_enable_tx_irq() -> bool {
    uart_write(UART_IER, UART_IER_TX_EMPTY);
    fence();
    uart_read(UART_IER) & UART_IER_TX_EMPTY != 0
}

/// Tắt UART `IER.TX-empty`; `true` ⇔ đọc lại thấy bit đã về 0.
pub fn uart_disable_tx_irq() -> bool {
    uart_write(UART_IER, 0);
    fence();
    uart_read(UART_IER) & UART_IER_TX_EMPTY == 0
}

/// `true` ⇔ `IER.TX-empty` đang bật (đọc register).
pub fn uart_tx_irq_enabled() -> bool {
    uart_read(UART_IER) & UART_IER_TX_EMPTY != 0
}

/// Tạo **level transition** cho nguồn TX-empty: `IER` 2 → 0 → 2. PLIC cổ điển level-triggered
/// không tự pending lại khi nguồn còn mức cao sau claim, nên handler phải hạ rồi nâng mức để có
/// ngắt kế (xem `plat-common::plic`).
fn uart_rearm_tx_irq() {
    uart_write(UART_IER, 0);
    fence();
    uart_write(UART_IER, UART_IER_TX_EMPTY);
    fence();
}

/// Xử lý **một** ngắt external (`scause = (1<<63)|9`) — đường duy nhất handler C gọi.
///
/// 1. **claim** của context `2*hart+1`; claim = 0 ⇒ spurious (đếm, **không** complete).
/// 2. line `10` (UART): đọc `IIR`, đếm; chưa đủ [`UART_IRQ_LIMIT`] ⇒ complete rồi re-arm
///    (`IER` 0→2) để có ngắt kế; đủ N ⇒ tắt `IER` + clear enable (không ngập) rồi complete.
/// 3. line khác: đếm "IRQ ngoài dự kiến" rồi **complete** — không treo (K3-PLAN §4.1 KT-48).
///
/// Không in trong handler (in làm trễ/ngập log); TAP đọc [`PLIC_STATS`].
pub fn on_external_interrupt() -> ClaimOutcome {
    let context = current_context();
    let claim = plic_read(claim_offset(context));
    let outcome = PLIC_STATS.begin_claim(claim, UART0_IRQ);
    match outcome {
        ClaimOutcome::Spurious => {}
        ClaimOutcome::Uart => {
            let iir = uart_read(UART_IIR) as u32;
            // `IIR & 0x0f` = mã nguồn (0x2 = TX-empty); bit 7:6 là trạng thái FIFO của firmware.
            PLIC_STATS.note_iir(iir);
            if uart_limit_reached(PLIC_STATS.snapshot().uart_irqs) {
                let _ = uart_disable_tx_irq();
                let _ = disable_irq(UART0_IRQ);
                PLIC_STATS.note_uart_ier_off();
                plic_write(claim_offset(context), claim);
                fence();
                PLIC_STATS.note_complete();
            } else {
                plic_write(claim_offset(context), claim);
                fence();
                PLIC_STATS.note_complete();
                uart_rearm_tx_irq();
            }
        }
        ClaimOutcome::Unexpected(line) => {
            plic_write(claim_offset(context), line);
            fence();
            PLIC_STATS.note_complete();
        }
    }
    outcome
}

/// `true` ⇔ đủ [`UART_IRQ_LIMIT`] ngắt UART đã xử lý (TAP/product chờ mốc này).
pub fn uart_irqs_done() -> bool {
    PLIC_STATS.snapshot().uart_irqs >= UART_IRQ_LIMIT
}
