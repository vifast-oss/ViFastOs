// SPDX-License-Identifier: MIT OR Apache-2.0
//! PLIC **cổ điển** (RISC-V PLIC spec) — phần **logic an toàn** của KT-48, chạy được trên host.
//!
//! QEMU `virt` đặt PLIC tại `0x0c00_0000` (D2-72 Q7) và K3 dùng **PLIC cổ điển**, AIA
//! (APLIC/IMSIC) hoãn tới K4'/K5' (D2-72 Q8). Tệp này chứa **hình học đăng ký** của PLIC spec,
//! quyết định claim/complete và bộ đếm external interrupt; phần MMIO thật
//! (`read_volatile`/`write_volatile`, `csr`) nằm ở `kernel/vifast-plat-riscv64` (D2-17) — nhờ vậy
//! toàn bộ *ngữ nghĩa* ở đây test được trên host (D2-72 Q12, giống `timer`/`trap`/`mmu`).
//!
//! # Bản đồ đăng ký PLIC (spec PLIC §4–§7; QEMU `virt` dùng đúng hình học này)
//!
//! ```text
//!  offset                    nội dung
//!  0x0000 + 4*irq            priority[irq] (IRQ 1..=ndev; 0 = không bao giờ forward)
//!  0x1000 + 4*(irq/32)       pending (chỉ đọc)
//!  0x2000 + 0x80*ctx + 4*w   enable word w của context (mỗi context 0x80 B = 32 word)
//!  0x20_0000 + 0x1000*ctx    context: +0x0 threshold, +0x4 claim/complete (chỉ đọc claim)
//! ```
//!
//! Hart `h` có hai context: **M-mode `2*h`** và **S-mode `2*h+1`** (D2-72 Q7). K3 dùng đúng
//! context **`2*hart+1`**; context M-mode không bao giờ được bật (nevà TAP kiểm `enable_mmode=0`).
//!
//! # Hai cửa sổ mapping 2 MiB (D2-72 Q4 `MAX_HARTS = 8`)
//!
//! - cửa sổ **thấp** `0x0c00_0000..0x0c20_0000`: priority + pending + enable tới context 15
//!   (`0x2000 + 0x80*15 = 0x2780`);
//! - cửa sổ **cao** `0x0c20_0000..0x0c40_0000`: threshold/claim tới context 15
//!   (`0x20_0000 + 0x1000*15 = 0x20f000`).
//!
//! Cả hai map **Device 2 MiB RW NX** trong `mmu` (QEMU `virt` cho phép leaf 2 MiB cho MMIO); nếu
//! `MAX_HARTS` tăng quá [`PLIC_MAPPED_HARTS`] thì phải mở rộng cửa sổ (hằng số ở đây chặn bằng
//! `const` assert + host test).
//!
//! # `ndev` — không giả định 95
//!
//! PLIC spec **không** có thanh ghi `ndev`; QEMU chỉ hiện thực priority register cho các nguồn
//! `1..=ndev` và trả 0 cho nguồn không tồn tại. Vì vậy arch đọc `ndev` bằng **probe
//! write-1/read-back** trên priority (ghi 1, đọc lại, trả về 0), dừng ở nguồn đầu tiên không giữ
//! giá trị — xem `vifast-plat-riscv64::plic::probe_ndev`. Hàm [`ndev_covers_uart`] là cổng kiểm
//! "đọc được ndev và nó phủ IRQ UART" trước khi bật bất cứ thứ gì.
//!
//! # Vì sao phải "re-arm" UART trong handler
//!
//! PLIC cổ điển là **level-triggered** có gateway: sau khi claim, nguồn còn mức cao **không** tự
//! pending lại (gateway chờ nguồn xuống thấp) — nếu handler chỉ claim/complete thì TX-empty (mức
//! cao liên tục khi THR rỗng) chỉ cho **đúng một** ngắt. Vì vậy arch tạo **level transition** bằng
//! cách tắt rồi bật lại `IER.TX-empty` sau complete cho tới khi đủ [`UART_IRQ_LIMIT`] ngắt, rồi
//! tắt hẳn (D2-72: "handler đếm, tắt sau N" — K3-PLAN §4.1 KT-48).
//!
//! **KT-49**: [`PLIC_STATS`] là **PerHart** ([`PerHartPlicStats`]): handler của hart nào cộng bộ
//! đếm của hart đó (`PLIC_STATS.get(hart)`), nên TAP "plic ctx per hart" chứng minh context
//! `2*hart+1` **không lẫn** — cùng một PLIC nhưng mỗi hart claim/complete trên context riêng.
//! `PlicStats` vẫn có `reset()` cho test host.
//!
//! **Không** thuộc KT-48/49: IPI (KT-50), MSI/AIA, hotplug hart.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::smp::MAX_HARTS;

/// Offset khối priority trong cửa sổ PLIC (mỗi IRQ 4 B).
pub const PLIC_PRIORITY_BASE: u64 = 0x0000;
/// Offset khối pending (chỉ đọc; mỗi 32 IRQ một word 4 B).
pub const PLIC_PENDING_BASE: u64 = 0x1000;
/// Offset khối enable (mỗi context 0x80 B).
pub const PLIC_ENABLE_BASE: u64 = 0x2000;
/// Khoảng cách enable giữa hai context (0x80 B = 32 word × 4 B).
pub const PLIC_ENABLE_CTX_STRIDE: u64 = 0x80;
/// Offset khối context (threshold/claim).
pub const PLIC_CONTEXT_BASE: u64 = 0x20_0000;
/// Khoảng cách giữa hai context.
pub const PLIC_CONTEXT_STRIDE: u64 = 0x1000;
/// Offset `threshold` trong một context.
pub const PLIC_THRESHOLD_OFFSET: u64 = 0;
/// Offset `claim/complete` trong một context (đọc = claim, ghi = complete).
pub const PLIC_CLAIM_OFFSET: u64 = 4;
/// Nguồn lớn nhất của PLIC spec: id `1..=1023` (1024 thanh ghi priority 4 B = 0x1000 B, id 0
/// dành riêng) — trần probe `ndev`, không giả định 95 (D2-72 Q7).
pub const PLIC_MAX_SOURCES: u32 = 1023;
/// Số nguồn trên một word enable/pending (32).
pub const PLIC_SOURCES_PER_WORD: u32 = 32;
/// Priority đặt cho UART0: **1** (PLIC chỉ forward nguồn có priority > threshold).
pub const PLIC_UART_PRIORITY: u32 = 1;
/// Threshold của context S-mode: **0** (mọi priority > 0 được forward).
pub const PLIC_THRESHOLD: u32 = 0;
/// Số hart tối đa được hai cửa sổ mapping 2 MiB phủ (D2-72 Q4: `MAX_HARTS = 8`).
pub const PLIC_MAPPED_HARTS: usize = 8;
/// Cỡ cửa sổ thấp (priority/pending/enable), 2 MiB.
pub const PLIC_LOW_WINDOW_BYTES: u64 = 0x20_0000;
/// Cỡ cửa sổ cao (context), 2 MiB.
pub const PLIC_HIGH_WINDOW_BYTES: u64 = 0x20_0000;
/// Số context (đánh số `0..PLIC_MAPPED_CONTEXTS`) mà hai cửa sổ 2 MiB phủ: 16 context cho 8
/// hart (M + S mỗi hart). Context S-mode lớn nhất là `PLIC_MAPPED_CONTEXTS - 1 = 15`.
pub const PLIC_MAPPED_CONTEXTS: u32 = (PLIC_MAPPED_HARTS as u32) * 2;
/// Số ngắt UART **thật** cần thu rồi tự tắt (`IER=0`, clear enable) để không ngập — K3-PLAN
/// §4.1 KT-48 "handler đếm, tắt sau N". Đủ chứng minh đường claim/complete lặp lại được.
pub const UART_IRQ_LIMIT: u64 = 8;

/// Context **S-mode** của hart `hart`: `2*hart + 1` (D2-72 Q7).
pub const fn context_of_hart(hart: usize) -> u32 {
    hart as u32 * 2 + 1
}

/// Context **M-mode** của hart `hart`: `2*hart` — **không** dùng ở K3, chỉ để kiểm "không bật
/// nhầm context M-mode" (TAP `enable_mmode=0`).
pub const fn machine_context_of_hart(hart: usize) -> u32 {
    hart as u32 * 2
}

/// Offset thanh ghi priority của `irq` (IRQ 0 không tồn tại trong PLIC).
pub const fn priority_offset(irq: u32) -> u64 {
    PLIC_PRIORITY_BASE + irq as u64 * 4
}

/// Word enable chứa `irq` (32 IRQ/word).
pub const fn enable_word_index(irq: u32) -> u32 {
    irq / PLIC_SOURCES_PER_WORD
}

/// Bit của `irq` trong word enable.
pub const fn enable_bit(irq: u32) -> u32 {
    1u32 << (irq % PLIC_SOURCES_PER_WORD)
}

/// Offset word enable của `irq` cho `context`.
pub const fn enable_offset(context: u32, irq: u32) -> u64 {
    PLIC_ENABLE_BASE + context as u64 * PLIC_ENABLE_CTX_STRIDE + enable_word_index(irq) as u64 * 4
}

/// Offset khối threshold/claim của `context`.
pub const fn context_offset(context: u32) -> u64 {
    PLIC_CONTEXT_BASE + context as u64 * PLIC_CONTEXT_STRIDE
}

/// Offset thanh ghi `threshold` của `context`.
pub const fn threshold_offset(context: u32) -> u64 {
    context_offset(context) + PLIC_THRESHOLD_OFFSET
}

/// Offset thanh ghi `claim/complete` của `context`.
pub const fn claim_offset(context: u32) -> u64 {
    context_offset(context) + PLIC_CLAIM_OFFSET
}

/// `true` ⇔ `ndev` (số nguồn đọc từ register) phủ `irq` (hợp lệ `1..=ndev`).
pub const fn ndev_covers_uart(ndev: u32, irq: u32) -> bool {
    irq >= 1 && irq <= ndev
}

/// `true` ⇔ đã thu đủ [`UART_IRQ_LIMIT`] ngắt UART ⇒ handler phải tắt `IER` + enable để dừng.
pub const fn uart_limit_reached(uart_irqs: u64) -> bool {
    uart_irqs >= UART_IRQ_LIMIT
}

/// Kết quả phân loại một lần **claim**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// Claim trả 0: PLIC không có ngắt đang chờ (spurious — **không** complete theo spec).
    Spurious,
    /// Claim đúng IRQ UART0 đã bật — đường xử lý TX-empty.
    Uart,
    /// Claim một line khác — `line` là IRQ thật đã claim; handler đếm "IRQ giả/ngoài dự kiến"
    /// rồi **complete** để không treo (K3-PLAN §4.1 KT-48).
    Unexpected(u32),
}

/// Phân loại `claim` so với `uart_irq` — hàm thuần, host test phủ cả ba nhánh.
pub const fn classify_claim(claim: u32, uart_irq: u32) -> ClaimOutcome {
    if claim == 0 {
        ClaimOutcome::Spurious
    } else if claim == uart_irq {
        ClaimOutcome::Uart
    } else {
        ClaimOutcome::Unexpected(claim)
    }
}

/// Ảnh chụp bộ đếm external interrupt — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlicSnapshot {
    /// Số trap external (`scause = (1<<63)|9`) đã vào handler (kể cả spurious).
    pub traps: u64,
    /// Số lần đọc claim thành công (claim ≠ 0).
    pub claims: u64,
    /// Số lần ghi complete (phải bằng `claims` ⇒ cân bằng).
    pub completions: u64,
    /// Số ngắt UART0 thật đã xử lý.
    pub uart_irqs: u64,
    /// Số claim trả 0 (spurious — không complete).
    pub spurious: u64,
    /// Số claim line **khác** UART0 (đã complete để không treo).
    pub unexpected: u64,
    /// IRQ của lần claim gần nhất (0 nếu spurious).
    pub last_irq: u64,
    /// Mã nguồn `IIR` đọc trong handler UART lần gần nhất (`IIR & 0x0f`; 0x2 = TX-empty).
    /// Chỉ giữ 4 bit nguồn: bit FIFO 7:6 phụ thuộc firmware có bật FIFO hay không (OpenSBI bật ⇒
    /// giá trị thô 0xc2), không phải nguồn ngắt.
    pub last_iir: u64,
    /// 1 ⇔ handler đã tắt `IER` sau khi đủ [`UART_IRQ_LIMIT`] ngắt.
    pub uart_ier_off: u64,
}

impl PlicSnapshot {
    /// Claim/complete **cân bằng** (DoD KT-48).
    pub const fn balanced(&self) -> bool {
        self.claims == self.completions
    }

    /// Số "IRQ giả": spurious + line lạ (DoD KT-48: phải bằng 0).
    pub const fn fake_irqs(&self) -> u64 {
        self.spurious + self.unexpected
    }
}

/// Bộ đếm PLIC toàn cục (một hart ở KT-48; PerHart là KT-49).
#[derive(Debug)]
pub struct PlicStats {
    traps: AtomicU64,
    claims: AtomicU64,
    completions: AtomicU64,
    uart_irqs: AtomicU64,
    spurious: AtomicU64,
    unexpected: AtomicU64,
    last_irq: AtomicU64,
    last_iir: AtomicU64,
    uart_ier_off: AtomicU64,
}

impl PlicStats {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            traps: AtomicU64::new(0),
            claims: AtomicU64::new(0),
            completions: AtomicU64::new(0),
            uart_irqs: AtomicU64::new(0),
            spurious: AtomicU64::new(0),
            unexpected: AtomicU64::new(0),
            last_irq: AtomicU64::new(0),
            last_iir: AtomicU64::new(0),
            uart_ier_off: AtomicU64::new(0),
        }
    }

    /// Ghi một lần claim: cộng `traps`, phân loại, cộng `claims` (claim ≠ 0) hoặc `spurious`.
    ///
    /// Trả [`ClaimOutcome`] để arch quyết định xử lý (UART ⇒ re-arm; line lạ ⇒ complete).
    pub fn begin_claim(&self, claim: u32, uart_irq: u32) -> ClaimOutcome {
        let outcome = classify_claim(claim, uart_irq);
        self.traps.fetch_add(1, Ordering::Relaxed);
        self.last_irq.store(claim as u64, Ordering::Relaxed);
        match outcome {
            ClaimOutcome::Spurious => {
                self.spurious.fetch_add(1, Ordering::Relaxed);
            }
            ClaimOutcome::Uart => {
                self.claims.fetch_add(1, Ordering::Relaxed);
                self.uart_irqs.fetch_add(1, Ordering::Relaxed);
            }
            ClaimOutcome::Unexpected(_) => {
                self.claims.fetch_add(1, Ordering::Relaxed);
                self.unexpected.fetch_add(1, Ordering::Relaxed);
            }
        }
        outcome
    }

    /// Ghi **mã nguồn** `IIR` (`IIR & 0x0f`) đọc trong handler — bằng chứng ngắt đến từ
    /// TX-empty (`0x2`), ổn định dù firmware bật/tắt FIFO (bit 7:6 của IIR).
    pub fn note_iir(&self, iir: u32) {
        self.last_iir.store((iir & 0x0f) as u64, Ordering::Relaxed);
    }

    /// Ghi một lần complete — handler **luôn** complete sau khi claim ≠ 0 (cân bằng).
    pub fn note_complete(&self) {
        self.completions.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi mốc handler đã tắt `IER` sau [`UART_IRQ_LIMIT`] ngắt.
    pub fn note_uart_ier_off(&self) {
        self.uart_ier_off.store(1, Ordering::Relaxed);
    }

    /// Đọc nhất quán (một hart; từng trường `Relaxed` đủ cho test KT-48).
    pub fn snapshot(&self) -> PlicSnapshot {
        PlicSnapshot {
            traps: self.traps.load(Ordering::Relaxed),
            claims: self.claims.load(Ordering::Relaxed),
            completions: self.completions.load(Ordering::Relaxed),
            uart_irqs: self.uart_irqs.load(Ordering::Relaxed),
            spurious: self.spurious.load(Ordering::Relaxed),
            unexpected: self.unexpected.load(Ordering::Relaxed),
            last_irq: self.last_irq.load(Ordering::Relaxed),
            last_iir: self.last_iir.load(Ordering::Relaxed),
            uart_ier_off: self.uart_ier_off.load(Ordering::Relaxed),
        }
    }

    /// Xoá bộ đếm — chỉ dùng trong test host (ảnh thật không reset).
    pub fn reset(&self) {
        for field in [
            &self.traps,
            &self.claims,
            &self.completions,
            &self.uart_irqs,
            &self.spurious,
            &self.unexpected,
            &self.last_irq,
            &self.last_iir,
            &self.uart_ier_off,
        ] {
            field.store(0, Ordering::Relaxed);
        }
    }
}

impl Default for PlicStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm PLIC **theo hart** (KT-49) — mỗi hart một [`PlicStats`] riêng.
///
/// Handler external chạy trên hart nào thì cộng bộ đếm của hart đó, nên `traps/claims/completions/
/// uart_irqs` **không lẫn** giữa các hart; TAP "plic ctx per hart" đọc từng slot.
#[derive(Debug)]
pub struct PerHartPlicStats {
    slots: [PlicStats; MAX_HARTS],
}

impl PerHartPlicStats {
    /// Bộ đếm rỗng cho mọi hart — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            slots: [const { PlicStats::new() }; MAX_HARTS],
        }
    }

    /// Bộ đếm của `hart`; hart ngoài dải ⇒ slot 0 (không bao giờ xảy ra với trap thật).
    pub fn get(&self, hart: usize) -> &PlicStats {
        &self.slots[if hart < MAX_HARTS { hart } else { 0 }]
    }

    /// Tổng `claims` trên mọi hart (bằng chứng gộp cho log/TAP).
    pub fn total_claims(&self) -> u64 {
        self.slots.iter().map(|slot| slot.snapshot().claims).sum()
    }

    /// Tổng `uart_irqs` trên mọi hart.
    pub fn total_uart_irqs(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| slot.snapshot().uart_irqs)
            .sum()
    }
}

impl Default for PerHartPlicStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm PLIC **theo hart** của ảnh kernel (KT-49; KT-48 là một hart toàn cục).
pub static PLIC_STATS: PerHartPlicStats = PerHartPlicStats::new();

// Hai cửa sổ 2 MiB phải phủ hết context của `MAX_HARTS = 8`: enable của context 15 ở 0x2780
// (cửa sổ thấp) và claim của context 15 ở 0x20f000 (cửa sổ cao). Nếu ai đổi cửa sổ/hart thì
// build dừng ở đây thay vì fault MMIO lúc chạy (rủi ro "claim sai context" của K3-PLAN §4.1).
const _: () = {
    assert!(PLIC_MAPPED_HARTS >= 1);
    assert!(PLIC_MAX_SOURCES == 1023);
    assert!(PLIC_SOURCES_PER_WORD == 32);
    assert!(UART_IRQ_LIMIT >= 1 && UART_IRQ_LIMIT <= 64);
    assert!(PLIC_UART_PRIORITY > PLIC_THRESHOLD);
    assert!(enable_offset(PLIC_MAPPED_CONTEXTS - 1, PLIC_MAX_SOURCES) < PLIC_LOW_WINDOW_BYTES);
    assert!(
        context_offset(PLIC_MAPPED_CONTEXTS - 1) + PLIC_CONTEXT_STRIDE - PLIC_CONTEXT_BASE
            <= PLIC_HIGH_WINDOW_BYTES
    );
};
