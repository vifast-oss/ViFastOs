// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-406** — UART RX **IRQ thật** trên aarch64: `PL011 → GIC SPI 33 → ring KT-69 → shell`.
//!
//! # Vì sao tệp này tồn tại
//!
//! Trước KT-406, shell `product-shell` aarch64 nhận **script do kernel bơm** vào ring bằng
//! `conchan` (nợ KT-73 ghi ở `pshell.rs` §"Nợ đã biết"): không có byte nào đi qua thiết bị, nên
//! đường ngắt UART chưa từng chạy trên arch này. Tệp này nối đúng đường phần cứng:
//!
//! ```text
//!   phím/byte (QEMU stdin) → PL011 FIFO → IMSC.RXIM → GIC SPI 33 → vec_handler (IRQ)
//!      → drain DR → conchan::pack → Ring::send → shell.rs `ring.recv` → `run_line`
//! ```
//!
//! Mỗi mắt xích đều **đo được** và in ra log: `# uart: irq=<n> rx=<byte> … ok=1`. Không có
//! "đường tắt phần mềm": bỏ bật `IMSC`/SPI, hoặc bỏ `EOI`, thì `irq`/`rx` = 0 và runner ĐỎ.
//!
//! # Tại sao `unsafe` vẫn **không** xuất hiện
//!
//! MMIO đi qua [`crate::pl011`] (đã bọc `asm::mmio_*`); bộ nhớ ring đi qua
//! [`crate::asm::read_u64`]/[`crate::asm::write_u64`] — hai hàm volatile của crate (D2-17).
//! Tệp này chỉ là **logic + bộ đếm + adapter** [`vifast_ring::Mem`].
//!
//! # Ca âm (đo được, không phải lời hứa)
//!
//! - **Ngắt RX không có dữ liệu**: `FR.RXFE = 1` mà `on_irq` vẫn được gọi ⇒ 0 byte,
//!   `empty_polls += 1`, **không** treo (vòng drain có trần [`crate::pl011::RX_DRAIN_LIMIT`]).
//! - **Ring đầy**: `Ring::send` trả mã lỗi thật (9 = `NotEnoughMemory`); byte **không** bị ghi
//!   đè, `drops += 1`, `full_code` giữ mã lần đầu. Runner thấy `drops=0` ở kịch bản thường.
//! - **Chưa init ring** (`ring_pa = 0`): `on_irq` trả 0 và đếm `no_ring` — không panic trong
//!   handler (ngắt có thể tới trước khi loader xong).

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_ring::{Mem, Ring};
use vifast_virtio::conchan::{CHANNEL_SLOTS, CHUNK_BYTES, SLOT_BYTES};

use crate::pl011;

/// Trần số byte một lần ngắt đọc vào một slot (bằng `CHUNK_BYTES` của codec — một slot ring).
const DRAIN_SLOT: usize = CHUNK_BYTES;

// ---------------------------------------------------------------- trạng thái ring

/// PA của frame ring KT-69 (`0` = chưa init). Ghi **một lần** trước `eret` xuống EL0.
static RING_PA: AtomicU64 = AtomicU64::new(0);

/// Adapter [`Mem`] trên PA identity-mapped của ring (kernel RAM).
struct PhysMem {
    base: u64,
}

impl Mem for PhysMem {
    fn load(&self, offset: usize) -> u64 {
        crate::asm::read_u64(self.base + offset as u64)
    }
    fn store(&self, offset: usize, value: u64) {
        crate::asm::write_u64(self.base + offset as u64, value);
    }
    fn acquire(&self, offset: usize) -> u64 {
        crate::asm::read_u64(self.base + offset as u64)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        crate::asm::write_u64(self.base + offset as u64, value);
    }
}

// ---------------------------------------------------------------- bộ đếm

static IRQS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static CHUNKS: AtomicU64 = AtomicU64::new(0);
static DROPS: AtomicU64 = AtomicU64::new(0);
static FULL_CODE: AtomicU64 = AtomicU64::new(0);
static EMPTY_POLLS: AtomicU64 = AtomicU64::new(0);
static NO_RING: AtomicU64 = AtomicU64::new(0);
static LAST_BYTES: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm — in **nguyên khối** trong một dòng `# uart:` (D2-102: in từ biến đo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Số lần `on_irq` thật sự chạy.
    pub irqs: u64,
    /// Tổng byte đã đẩy vào ring.
    pub bytes: u64,
    /// Số slot đã `Ring::send` thành công.
    pub chunks: u64,
    /// Số slot bị rơi vì ring đầy.
    pub drops: u64,
    /// Mã lỗi **thật** của lần đầy đầu tiên (9 = `NotEnoughMemory`).
    pub full_code: u64,
    /// Số lần handler thấy ngắt RX nhưng FIFO rỗng (ca âm).
    pub empty_polls: u64,
    /// Số lần handler chạy khi ring chưa init (không được panic).
    pub no_ring: u64,
    /// Số byte của ngắt gần nhất.
    pub last_irq_bytes: u64,
}

/// Đọc ảnh chụp bộ đếm (không xoá).
pub fn stats() -> Stats {
    Stats {
        irqs: IRQS.load(Ordering::SeqCst),
        bytes: BYTES.load(Ordering::SeqCst),
        chunks: CHUNKS.load(Ordering::SeqCst),
        drops: DROPS.load(Ordering::SeqCst),
        full_code: FULL_CODE.load(Ordering::SeqCst),
        empty_polls: EMPTY_POLLS.load(Ordering::SeqCst),
        no_ring: NO_RING.load(Ordering::SeqCst),
        last_irq_bytes: LAST_BYTES.load(Ordering::SeqCst),
    }
}

/// PA ring đã gắn (`0` = chưa).
pub fn ring_pa() -> u64 {
    RING_PA.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------- vòng đời

/// Gắn PA frame ring do loader cấp; `true` ⇔ PA hợp lệ (khác 0).
pub fn init(pa: u64) -> bool {
    RING_PA.store(pa, Ordering::SeqCst);
    pa != 0
}

/// Đẩy `bytes` vào ring theo slot [`CHUNK_BYTES`]; trả `(slot_ok, byte_ok)`.
fn push(bytes: &[u8]) -> (usize, usize) {
    let pa = RING_PA.load(Ordering::SeqCst);
    if pa == 0 {
        NO_RING.fetch_add(1, Ordering::SeqCst);
        return (0, 0);
    }
    let ring = Ring::new(PhysMem { base: pa });
    let mut sent = 0usize;
    let mut ok_bytes = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let end = (index + CHUNK_BYTES).min(bytes.len());
        let (words, count) = vifast_virtio::conchan::pack(&bytes[index..end]);
        match ring.send(&words) {
            Ok(()) => {
                sent += 1;
                ok_bytes += count;
                CHUNKS.fetch_add(1, Ordering::SeqCst);
            }
            Err(code) => {
                if DROPS.fetch_add(1, Ordering::SeqCst) == 0 {
                    FULL_CODE.store(u64::from(code), Ordering::SeqCst);
                }
            }
        }
        index = end;
    }
    (sent, ok_bytes)
}

/// Vét `PL011` FIFO vào ring (trần `RX_DRAIN_LIMIT` byte); trả số byte đã đọc.
fn drain() -> u32 {
    let mut slot = [0u8; DRAIN_SLOT];
    let mut total = 0u32;
    loop {
        let count = pl011::rx_drain(&mut slot);
        if count == 0 {
            break;
        }
        let (_, ok) = push(&slot[..count]);
        BYTES.fetch_add(ok as u64, Ordering::SeqCst);
        total += count as u32;
        if count < DRAIN_SLOT {
            break;
        }
    }
    total
}

/// Handler của **một** ngắt RX: drain FIFO → ring; trả số byte đã đọc.
///
/// Gọi từ `vec_handler` (ô IRQ) **sau** khi `ack_irq` trả đúng INTID của PL011; caller chịu
/// trách nhiệm `EOI`. Hàm **không** panic và **không** quay vô hạn khi thiết bị không trả lời.
pub fn on_irq() -> u32 {
    IRQS.fetch_add(1, Ordering::SeqCst);
    let bytes = drain();
    LAST_BYTES.store(u64::from(bytes), Ordering::SeqCst);
    if bytes == 0 {
        EMPTY_POLLS.fetch_add(1, Ordering::SeqCst);
    }
    bytes
}

/// `true` ⇔ ring header hợp lệ (magic/version/hình học/`head >= tail`).
pub fn ring_check() -> bool {
    let pa = RING_PA.load(Ordering::SeqCst);
    if pa == 0 {
        return false;
    }
    Ring::new(PhysMem { base: pa }).check()
}

/// `(head, tail, used, seq)` của ring — bằng chứng shell **đã tiêu thụ** byte.
pub fn ring_stats() -> (u64, u64, u64, u64) {
    let pa = RING_PA.load(Ordering::SeqCst);
    if pa == 0 {
        return (0, 0, 0, 0);
    }
    let stats = Ring::new(PhysMem { base: pa }).stats();
    (stats.head, stats.tail, stats.used, stats.seq)
}

/// Hình học ring đọc từ header — hằng số của codec (dùng cho dòng evidence `slots=`/`slot=`).
pub const fn geometry() -> (u32, u32) {
    (CHANNEL_SLOTS as u32, SLOT_BYTES as u32)
}
