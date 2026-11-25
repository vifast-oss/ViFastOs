// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-406** — COM1 RX **IRQ thật** trên x86_64: `COM1 (IRQ 4) → IOAPIC → LAPIC → IDT 0x24 →
//! ring KT-69 → shell`.
//!
//! # Vì sao tệp này tồn tại
//!
//! Trước KT-406, shell `product-shell` x86_64 nhận **script do kernel bơm** vào ring bằng
//! `conchan` (nợ KT-73 ghi ở `shellpath.rs`): không byte nào đi qua COM1, nên đường ngắt phần
//! cứng của UART chưa từng chạy trên arch này. Tệp này nối đúng đường:
//!
//! ```text
//!   byte gõ (QEMU stdin) → COM1 RBR → IER.RX-data → IOAPIC pin 4 → LAPIC → IDT 0x24
//!      → vifast_isr_serial → exc_dispatch → drain RBR → conchan::pack → Ring::send
//!      → shell.rs `ring.recv` → `run_line`
//! ```
//!
//! # Bốn mắt xích đều **đọc lại** từ thanh ghi
//!
//! 1. `LAPIC.SVR` bật (`bit 8`) + `TPR = 0` — nếu không, ngắt bị che và **không** có dấu vết;
//! 2. `IOAPIC` redirection pin 4: vector `0x24`, **không** mask, `Destination` = LAPIC ID **của
//!    BSP đọc từ thanh ghi** (không phải hằng 0);
//! 3. `COM1.IER` bit 0 (RX data available) — bật đúng nguồn, không bật tràn lan;
//! 4. `IDT[0x24]` có stub (`STUB_SERIAL`) — thiếu ô thì ngắt rơi vào `#GP`/`#DF`.
//!
//! Mỗi lần ngắt: đọc `RBR` tới khi `LSR.DR = 0` (trần 64 byte), đẩy slot vào ring, rồi **EOI**
//! (`LAPIC_EOI`) — thiếu `EOI` thì LAPIC không chuyển ngắt kế tiếp và phép đo treo thay vì đỏ.
//!
//! # Tại sao `unsafe` không xuất hiện
//!
//! MMIO/I/O đi qua [`crate::asm`] (crate arch, D2-17); bộ nhớ ring đi qua `asm::read_u64`/
//! `asm::write_u64` (volatile). Tệp này là logic + bộ đếm + adapter [`vifast_ring::Mem`].

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::apic;
use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::serial;
use vifast_ring::{Mem, Ring};
use vifast_virtio::conchan::{CHANNEL_SLOTS, CHUNK_BYTES, SLOT_BYTES};

/// **IRQ** của COM1 trên PC: 4 (`isa-serial`, QEMU `q35` route qua IOAPIC pin 4).
pub const COM1_IRQ: u32 = 4;
/// Vector IDT của COM1 RX — khớp [`exc::VEC_SERIAL`].
pub const COM1_VECTOR: u8 = exc::VEC_SERIAL;

/// PA frame ring KT-69 (`0` = chưa gắn).
static RING_PA: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------- bộ đếm

static IRQS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static CHUNKS: AtomicU64 = AtomicU64::new(0);
static DROPS: AtomicU64 = AtomicU64::new(0);
static FULL_CODE: AtomicU64 = AtomicU64::new(0);
static EMPTY_POLLS: AtomicU64 = AtomicU64::new(0);
static NO_RING: AtomicU64 = AtomicU64::new(0);
static LAST_BYTES: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm — in **nguyên khối** trong một dòng `# uart:` (D2-102).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Số lần handler thật sự chạy.
    pub irqs: u64,
    /// Tổng byte đã đẩy vào ring.
    pub bytes: u64,
    /// Số slot `Ring::send` thành công.
    pub chunks: u64,
    /// Số slot bị rơi vì ring đầy.
    pub drops: u64,
    /// Mã lỗi **thật** của lần đầy đầu tiên (9 = `NotEnoughMemory`).
    pub full_code: u64,
    /// Số lần ngắt mà `RBR` rỗng (ca âm).
    pub empty_polls: u64,
    /// Số lần handler chạy khi ring chưa gắn (không panic).
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

// ---------------------------------------------------------------- ring

/// Adapter [`Mem`] trên PA identity-mapped của ring (kernel RAM dưới 1 GiB).
struct PhysMem {
    base: u64,
}

impl Mem for PhysMem {
    fn load(&self, offset: usize) -> u64 {
        asm::read_u64(self.base + offset as u64)
    }
    fn store(&self, offset: usize, value: u64) {
        asm::write_u64(self.base + offset as u64, value);
    }
    fn acquire(&self, offset: usize) -> u64 {
        asm::read_u64(self.base + offset as u64)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        asm::write_u64(self.base + offset as u64, value);
    }
}

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

// ---------------------------------------------------------------- MMIO/IO

/// Đọc thanh ghi LAPIC (MMIO 32-bit) tại `LAPIC_DEFAULT_BASE + offset`.
fn lapic_read(offset: u32) -> u32 {
    asm::read_u32(apic::LAPIC_DEFAULT_BASE + offset as u64)
}

/// Ghi thanh ghi LAPIC (MMIO 32-bit).
fn lapic_write(offset: u32, value: u32) {
    asm::write_u32(apic::LAPIC_DEFAULT_BASE + offset as u64, value);
}

/// Đọc thanh ghi IOAPIC qua `IOREGSEL`/`IOWIN`.
fn ioapic_read(reg: u32) -> u32 {
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_REGSEL as u64, reg);
    asm::read_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_WIN as u64)
}

/// Ghi thanh ghi IOAPIC.
fn ioapic_write(reg: u32, value: u32) {
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_REGSEL as u64, reg);
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_WIN as u64, value);
}

/// Đọc thanh ghi COM1 (offset trong bảng 16550).
fn com_in(offset: u16) -> u8 {
    asm::inb(serial::COM1_BASE + offset)
}

/// Ghi thanh ghi COM1.
fn com_out(offset: u16, value: u8) {
    asm::outb(serial::COM1_BASE + offset, value);
}

// ---------------------------------------------------------------- khởi tạo

/// Báo cáo khởi tạo — mọi trường **đọc lại** từ thanh ghi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// `IA32_APIC_BASE` đọc lại (bit 11 = APIC bật).
    pub apic_base_msr: u64,
    /// `LAPIC_ID` thô.
    pub lapic_id_raw: u32,
    /// `LAPIC.SVR` đọc lại.
    pub svr: u32,
    /// `LAPIC.TPR` đọc lại.
    pub tpr: u32,
    /// `IOAPICID` thô.
    pub ioapic_id_raw: u32,
    /// `IOAPICVER` thô.
    pub ioapic_version: u32,
    /// Số redirection entry.
    pub ioapic_entries: u32,
    /// Word thấp redirection pin 4 (sau khi mở mask).
    pub redir_low: u32,
    /// Word cao redirection pin 4 (**Destination** = LAPIC ID BSP).
    pub redir_high: u32,
    /// `COM1.IER` đọc lại.
    pub ier: u8,
    /// `COM1.LSR` đọc lại ngay sau khi bật.
    pub lsr: u8,
    /// `true` ⇔ ring gắn PA hợp lệ.
    pub ring_ok: bool,
}

impl Report {
    /// `true` ⇔ mọi mắt xích đọc lại đúng: APIC bật, SVR bật, TPR 0, IOAPIC có pin 4 không mask
    /// + vector `0x24` + Destination = ID BSP, IER bit 0 bật, ring có PA.
    pub fn ok(&self) -> bool {
        self.apic_base_msr & apic::APIC_BASE_ENABLE != 0
            && self.svr & apic::SVR_APIC_ENABLE != 0
            && (self.svr & apic::SVR_VECTOR_MASK) == exc::VEC_SPURIOUS as u32
            && self.tpr == 0
            && self.ioapic_entries > COM1_IRQ
            && !apic::ioapic_redir_is_masked(self.redir_low)
            && (self.redir_low & apic::LVT_VECTOR_MASK) == COM1_VECTOR as u32
            && (self.redir_high >> 24) == apic::lapic_id(self.lapic_id_raw) as u32
            && self.ier & serial::IER_RX != 0
            && self.ring_ok
    }
}

/// Bật **COM1 RX IRQ** thật: LAPIC → IOAPIC pin 4 → IER → (IDT đã có stub từ `idt::install`).
///
/// Gọi **một lần** trước khi vào CPL3 (script hoặc UART — cùng đường ngắt).
pub fn init(ring_pa: u64) -> Report {
    RING_PA.store(ring_pa, Ordering::SeqCst);
    // 1) Che 8259 PIC — hai nguồn ngắt cùng lúc thì vector IRQ4 legacy (0x0C) rơi vào ô IDT
    //    của exception; đây là bước `qg` của KT-266, giữ nguyên ngữ nghĩa.
    asm::outb(0x21, 0xFF);
    asm::outb(0xA1, 0xFF);
    // 2) Bật APIC toàn cục nếu firmware chưa bật; địa chỉ base phải là mặc định (ảnh map
    //    3..4 GiB nên chỉ hỗ trợ base này — không map thêm trang MMIO thứ hai).
    let mut msr = asm::read_msr(0x1B); // IA32_APIC_BASE
    msr |= apic::APIC_BASE_ENABLE;
    asm::write_msr(0x1B, msr);
    let apic_base_msr = asm::read_msr(0x1B);
    let base = apic_base_msr & apic::APIC_BASE_ADDR_MASK;
    let _ = base; // `LAPIC_DEFAULT_BASE` của QEMU `q35`; đọc lại để log nếu muốn.
                  // 3) LAPIC: SVR (spurious + enable), TPR = 0, mask LINT0/LINT1.
    let lapic_id_raw = lapic_read(apic::LAPIC_ID);
    lapic_write(apic::LAPIC_SVR, apic::svr_value(exc::VEC_SPURIOUS));
    lapic_write(apic::LAPIC_TPR, 0);
    lapic_write(apic::LAPIC_LVT_LINT0, apic::LVT_MASKED);
    lapic_write(apic::LAPIC_LVT_LINT1, apic::LVT_MASKED);
    let svr = lapic_read(apic::LAPIC_SVR);
    let tpr = lapic_read(apic::LAPIC_TPR);
    // 4) IOAPIC: đọc ID/version, **mask toàn bộ**, rồi mở **đúng** pin 4.
    let ioapic_id_raw = ioapic_read(apic::IOAPIC_ID_REG);
    let ioapic_version = ioapic_read(apic::IOAPIC_VER_REG);
    let ioapic_entries = apic::ioapic_redirection_count(ioapic_version);
    let mut pin = 0u32;
    while pin < ioapic_entries {
        if let Some(reg) = apic::ioapic_redtbl_reg(pin) {
            let low = ioapic_read(reg);
            ioapic_write(reg, apic::ioapic_redir_low_masked(low));
            ioapic_write(reg + 1, 0);
        }
        pin += 1;
    }
    let dest = apic::lapic_id(lapic_id_raw) as u32;
    if let Some(reg) = apic::ioapic_redtbl_reg(COM1_IRQ) {
        // Thứ tự **an toàn**: ghi Destination (word cao) **trước** khi mở mask, rồi word thấp
        // (vector + không mask). ISA IRQ 4 là **edge**, polarity **high** → bit 13/15 = 0.
        ioapic_write(reg + 1, dest << 24);
        ioapic_write(reg, COM1_VECTOR as u32);
    }
    let redir_low = apic::ioapic_redtbl_reg(COM1_IRQ)
        .map(ioapic_read)
        .unwrap_or(0);
    let redir_high = apic::ioapic_redtbl_reg(COM1_IRQ)
        .map(|reg| ioapic_read(reg + 1))
        .unwrap_or(0);
    // 5) COM1: bật **đúng** IER.RX (bit 0), giữ các bit đã có (không ghi đè 0), đọc lại.
    com_out(serial::REG_IER, com_in(serial::REG_IER) | serial::IER_RX);
    let ier = com_in(serial::REG_IER);
    let lsr = com_in(serial::REG_LSR);
    Report {
        apic_base_msr,
        lapic_id_raw,
        svr,
        tpr,
        ioapic_id_raw,
        ioapic_version,
        ioapic_entries,
        redir_low,
        redir_high,
        ier,
        lsr,
        ring_ok: ring_pa != 0,
    }
}

/// Vét `RBR` tới khi `LSR.DR = 0` (trần [`serial::RX_DRAIN_LIMIT`] byte); trả số byte đã đọc.
fn drain() -> u32 {
    let mut slot = [0u8; CHUNK_BYTES];
    let mut total = 0u32;
    while total < serial::RX_DRAIN_LIMIT && com_in(serial::REG_LSR) & serial::LSR_DR != 0 {
        slot[(total as usize) % CHUNK_BYTES] = com_in(serial::REG_DATA);
        total += 1;
        if total as usize % CHUNK_BYTES == 0 {
            let count = CHUNK_BYTES;
            let (_, ok) = push(&slot[..count]);
            BYTES.fetch_add(ok as u64, Ordering::SeqCst);
        }
    }
    let tail = (total as usize) % CHUNK_BYTES;
    if tail != 0 {
        let (_, ok) = push(&slot[..tail]);
        BYTES.fetch_add(ok as u64, Ordering::SeqCst);
    }
    total
}

/// Handler của **một** ngắt COM1 RX: drain `RBR` → ring, rồi `EOI`. Trả số byte đã đọc.
///
/// Gọi từ `exc_dispatch` (vector [`COM1_VECTOR`]) **rồi** resume. Không bao giờ panic/quay vô
/// hạn khi thiết bị không trả lời (trần drain).
pub fn on_irq() -> u32 {
    IRQS.fetch_add(1, Ordering::SeqCst);
    let bytes = drain();
    LAST_BYTES.store(u64::from(bytes), Ordering::SeqCst);
    if bytes == 0 {
        EMPTY_POLLS.fetch_add(1, Ordering::SeqCst);
    }
    // EOI **sau** khi đã lấy hết byte: LAPIC chỉ chuyển ngắt kế tiếp khi nhận EOI.
    lapic_write(apic::LAPIC_EOI, 0);
    bytes
}

/// `true` ⇔ ring header hợp lệ.
pub fn ring_check() -> bool {
    let pa = RING_PA.load(Ordering::SeqCst);
    if pa == 0 {
        return false;
    }
    Ring::new(PhysMem { base: pa }).check()
}

/// `(head, tail, used, seq)` — bằng chứng shell **đã tiêu thụ** byte.
pub fn ring_stats() -> (u64, u64, u64, u64) {
    let pa = RING_PA.load(Ordering::SeqCst);
    if pa == 0 {
        return (0, 0, 0, 0);
    }
    let stats = Ring::new(PhysMem { base: pa }).stats();
    (stats.head, stats.tail, stats.used, stats.seq)
}

/// Hình học ring (hằng codec) — dùng cho dòng evidence `slots=`/`slot=`.
pub const fn geometry() -> (u32, u32) {
    (CHANNEL_SLOTS as u32, SLOT_BYTES as u32)
}
