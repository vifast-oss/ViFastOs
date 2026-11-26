// SPDX-License-Identifier: MIT OR Apache-2.0
//! **UART RX IRQ → ring → U-mode đọc** — KT-73 (K6), chỉ ảnh có U-mode (`feature = "user"`).
//!
//! # Đường đi (đóng nợ "shell chưa UART IRQ" của K3.5)
//!
//! 1. **PLIC**: line 10 (UART0) đã bật từ KT-48; module này bật thêm `IER.RX-data-available`
//!    (bit 0) **chỉ trong cửa sổ có người nhận** — không bật thường trực.
//! 2. **Handler** (được `plic::on_external_interrupt` gọi khi `IIR & 0x0f` là `0x04`/`0x0c`):
//!    đọc `RBR` tới khi `LSR.DataReady = 0`, gom theo **slot 24 byte** rồi `Ring::send` vào ring
//!    của KT-69 (header/magic/slot do `vifast_ring` quyết — không có layout thứ hai).
//! 3. **U-mode** mở cùng Frame ở `VA_CON_RING` (kernel map) và `Ring::recv` từng slot, bóc byte
//!    bằng codec [`vifast_virtio::conchan`] dùng chung với kernel ⇒ shell đọc **byte thật** từ
//!    ring, không phải message dòng do kernel cắm sẵn.
//!
//! # Nguồn byte của ca kiểm = loopback 16550 (MCR bit 4)
//!
//! QEMU `virt` không nối bàn phím vào UART khi `-nographic` có log; để ca kiểm **tất định** mà
//! vẫn đi **đúng** đường phần cứng (THR → SOUT/SIN loopback → RBR → `IER` → PLIC → handler), ca
//! kiểm bật loopback của 16550 rồi ghi byte vào `THR`: byte quay lại `RBR` **qua thiết bị**, sinh
//! ngắt RX thật. Đây **không** phải đường tắt phần mềm: nếu `IER`/PLIC/handler sai thì không có
//! byte nào vào ring. Ghi rõ trong `docs/k6-console.md` §4 (nguồn byte) để không ai đọc nhầm là
//! "bàn phím người".
//!
//! # Ca âm
//!
//! - **IRQ không có dữ liệu** (`IIR` báo RX nhưng `LSR` rỗng): handler trả 0 byte, đếm
//!   `empty_polls`, **không** treo, **không** mất byte khác (vòng drain có trần).
//! - **Ring đầy**: `Ring::send` trả [`vifast_ring::ERR_FULL`] (9 = `NotEnoughMemory`) — **không**
//!   ghi đè slot chưa đọc; đếm `drops` + giữ `full_code` **đầu tiên** làm bằng chứng backpressure.
//!
//! `unsafe` (MMIO + physmap volatile) nằm trong `kernel/vifast-plat-*` (D2-17).

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_core::vm::{CapId, MapFlags, ObjKind};
use vifast_plat_common::Platform;
use vifast_ring::{Mem, Ring};
use vifast_virtio::conchan::{CHANNEL_SLOTS, CHUNK_BYTES, SLOT_BYTES};

use crate::mmu;
use crate::obj as aobj;
use crate::Riscv64Virt;

/// VA **cố định** của ring trong VSpace của chương trình U-mode (kernel map; U-mode không tự map).
pub const VA_CON_RING: u64 = 0x4300_0000;
/// Cỡ Frame của ring (một trang 4 KiB).
pub const RING_PAGE_BYTES: usize = 4096;

// ---------------------------------------------------------------- thanh ghi 16550A (offset)
const UART_RBR: usize = 0;
const UART_THR: usize = 0;
const UART_IER: usize = 1;
const UART_MCR: usize = 4;
const UART_LSR: usize = 5;
/// `IER` bit 0 — RX data available interrupt.
const IER_RX: u8 = 0x01;
/// `MCR` bit 4 — loopback chế độ test của 16550 (SOUT nối SIN).
const MCR_LOOP: u8 = 0x10;
/// `LSR` bit 0 — dữ liệu đọc được.
const LSR_DATA_READY: u8 = 0x01;
/// Mã `IIR & 0x0f`: RX data available.
pub const IIR_RX_AVAILABLE: u32 = 0x04;
/// Mã `IIR & 0x0f`: character timeout (FIFO còn byte).
pub const IIR_RX_TIMEOUT: u32 = 0x0c;
/// Trần byte một lần drain (không bao giờ quay vô hạn trong handler).
const DRAIN_LIMIT: usize = 64;
/// Trần vòng chờ ring nhận đủ byte (tick timebase) — 1 s ảo; hết hạn ⇒ ca kiểm đỏ, không treo.
pub const RX_TIMEOUT_TICKS: u64 = vifast_plat_riscv64_logic::timer::TIMEBASE_HZ;

// ---------------------------------------------------------------- MMIO (UART trang 4 KiB)
#[inline]
fn uart_read(offset: usize) -> u8 {
    // SAFETY: `UART0_BASE + offset` nằm trong trang UART Device đã map (KT-46); offset < 8.
    unsafe { core::ptr::read_volatile((crate::UART0_BASE + offset) as *const u8) }
}
#[inline]
fn uart_write(offset: usize, value: u8) {
    // SAFETY: như `uart_read`; ghi thanh ghi thiết bị đã map.
    unsafe { core::ptr::write_volatile((crate::UART0_BASE + offset) as *mut u8, value) };
}
#[inline]
fn fence() {
    // SAFETY: hàng rào thứ tự bộ nhớ, không truy cập địa chỉ.
    unsafe { core::arch::asm!("fence rw, rw", options(nostack, preserves_flags)) };
}

/// `IIR` có phải nguồn RX (data available / character timeout) không.
pub const fn iir_is_rx(iir: u32) -> bool {
    matches!(iir & 0x0f, IIR_RX_AVAILABLE | IIR_RX_TIMEOUT)
}

// ---------------------------------------------------------------- bộ đếm
static IRQS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static DROPS: AtomicU64 = AtomicU64::new(0);
static FULL_CODE: AtomicU64 = AtomicU64::new(0);
static EMPTY_POLLS: AtomicU64 = AtomicU64::new(0);
static CHUNKS: AtomicU64 = AtomicU64::new(0);
static INJECTED: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static LAST_IRQ_BYTES: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm — in nguyên khối (D2-102: in từ biến đo, không chép số).
#[derive(Clone, Copy, Debug, Default)]
pub struct ConStats {
    /// Số ngắt RX đã xử lý.
    pub irqs: u64,
    /// Tổng byte đã đẩy **vào ring**.
    pub bytes: u64,
    /// Số slot bị rơi vì ring đầy (backpressure).
    pub drops: u64,
    /// Mã lỗi **thật** của lần `send` đầy đầu tiên (phải 9 = `NotEnoughMemory`).
    pub full_code: u64,
    /// Số lần handler thấy nguồn RX nhưng `LSR` rỗng (ca âm "IRQ không có dữ liệu").
    pub empty_polls: u64,
    /// Số slot đã gửi.
    pub chunks: u64,
    /// Số byte đã ghi vào `THR` ở chế độ loopback (nguồn của ca kiểm).
    pub injected: u64,
    /// Số lần chờ ring nhận đủ byte quá hạn (phải 0).
    pub timeouts: u64,
    /// Số byte **của ngắt gần nhất**.
    pub last_irq_bytes: u64,
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> ConStats {
    ConStats {
        irqs: IRQS.load(Ordering::SeqCst),
        bytes: BYTES.load(Ordering::SeqCst),
        drops: DROPS.load(Ordering::SeqCst),
        full_code: FULL_CODE.load(Ordering::SeqCst),
        empty_polls: EMPTY_POLLS.load(Ordering::SeqCst),
        chunks: CHUNKS.load(Ordering::SeqCst),
        injected: INJECTED.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
        last_irq_bytes: LAST_IRQ_BYTES.load(Ordering::SeqCst),
    }
}

// ---------------------------------------------------------------- trạng thái ring
/// Adapter [`Mem`] trên **physmap** của Frame (kernel đọc/ghi byte của ring khi chưa map vào
/// U-mode, và handler ghi slot). Acquire/Release = `fence` + đọc/ghi thường (RISC-V).
struct PhysMem {
    base: u64,
}

impl Mem for PhysMem {
    fn load(&self, offset: usize) -> u64 {
        // SAFETY: `base` là alias physmap của Frame 4 KiB do object layer cấp; offset < 4096.
        unsafe { core::ptr::read_volatile((self.base + offset as u64) as *const u64) }
    }
    fn store(&self, offset: usize, value: u64) {
        // SAFETY: như `load`.
        unsafe { core::ptr::write_volatile((self.base + offset as u64) as *mut u64, value) };
    }
    fn acquire(&self, offset: usize) -> u64 {
        fence();
        self.load(offset)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        self.store(offset, value);
        fence();
    }
}

/// Trạng thái ring (một Frame, một hart).
struct State {
    ready: bool,
    rx_on: bool,
    pa: u64,
    phys: u64,
    /// `head` lúc init (bằng chứng ring tiến từ 0).
    head0: u64,
}

const INIT_STATE: State = State {
    ready: false,
    rx_on: false,
    pa: 0,
    phys: 0,
    head0: 0,
};

struct Cell<T>(core::cell::UnsafeCell<T>);
// SAFETY: ảnh test chạy một hart cho pha này; truy cập tuần tự trong module.
unsafe impl<T> Sync for Cell<T> {}

static STATE: Cell<State> = Cell(core::cell::UnsafeCell::new(INIT_STATE));

fn state() -> &'static mut State {
    // SAFETY: một hart + tuần tự (xem `Cell`).
    unsafe { &mut *STATE.0.get() }
}

/// Báo cáo khởi tạo ring — PA/VA/kích thước **đọc lại** từ object layer, không hằng.
#[derive(Clone, Copy, Debug)]
pub struct RingReport {
    /// PA của Frame ring (object layer cấp).
    pub pa: u64,
    /// Alias physmap kernel dùng để ghi header/slot.
    pub phys: u64,
    /// VA trong VSpace của chương trình U-mode.
    pub va: u64,
    /// Số slot (`const:` trong evidence — giá trị cấu hình, không phải số đo).
    pub slots: u32,
    /// Byte mỗi slot.
    pub slot_bytes: u32,
    /// `head`/`tail` đọc lại ngay sau `init` (phải 0/0).
    pub head: u64,
    /// `tail` sau `init`.
    pub tail: u64,
    /// Hình học `Ring::open` + `check` đọc lại **thành công** (bằng chứng header hợp lệ).
    pub check: bool,
}

/// Cấp Frame 4 KiB từ `untyped`, khởi tạo header ring của KT-69, trả PA/physmap.
///
/// Lỗi trả tên chặng (`&'static str`) để TAP in lý do đo được thay vì PANIC im lặng.
pub fn init_ring(untyped: CapId) -> Result<RingReport, &'static str> {
    let m = aobj::machine();
    let frame = m
        .retype(untyped, ObjKind::Frame, 12)
        .map_err(|_| "retype")?;
    let pa = m.frame_pa(frame).map_err(|_| "frame_pa")?;
    let phys = mmu::physmap_alias(pa).ok_or("physmap")?;
    let ring = Ring::new(PhysMem { base: phys });
    let geometry = ring.init(CHANNEL_SLOTS, SLOT_BYTES).map_err(|_| "init")?;
    let snapshot = ring.stats();
    let check = ring.open().is_ok() && ring.check();
    // Lưu cap Frame để `attach` map **đúng** Frame này (không cấp Frame thứ hai).
    // SAFETY: ghi một lần, trước khi bất kỳ ai đọc (`ready` chỉ bật sau đó); một hart.
    unsafe { *FRAME_CAP.0.get() = Some(frame) };
    let s = state();
    s.ready = true;
    s.pa = pa;
    s.phys = phys;
    s.head0 = snapshot.head;
    Ok(RingReport {
        pa,
        phys,
        va: VA_CON_RING,
        slots: geometry.size,
        slot_bytes: geometry.slot_bytes,
        head: snapshot.head,
        tail: snapshot.tail,
        check,
    })
}

/// Map Frame ring vào VSpace `vspace` tại [`VA_CON_RING`] (R|W|U, không exec — W^X).
///
/// Kernel map hộ: U-mode **không** có invocation `Frame.Map` cho cap lạ, và ring là kênh do
/// kernel làm producer — không cần chuyển cap. `false` nếu object layer từ chối.
pub fn attach(vspace: CapId) -> bool {
    let s = state();
    if !s.ready {
        return false;
    }
    let m = aobj::machine();
    // Cap Frame đã cấp nằm trong `untyped` của pha; map lại đúng Frame bằng cap đã lưu.
    let flags = MapFlags {
        read: true,
        write: true,
        exec: false,
        user: true,
        device: false,
    };
    matches!(
        FRAME_CAP.with(|cap| m.map(vspace, VA_CON_RING, *cap, flags)),
        Some(Ok(()))
    )
}

/// Cap Frame ring (một phần tử; `with` để không cần `static mut`).
struct FrameCap(core::cell::UnsafeCell<Option<CapId>>);
// SAFETY: một hart, chỉ ghi một lần lúc `init_ring`, đọc sau đó.
unsafe impl Sync for FrameCap {}
static FRAME_CAP: FrameCap = FrameCap(core::cell::UnsafeCell::new(None));

impl FrameCap {
    fn with<R>(&self, f: impl FnOnce(&CapId) -> R) -> Option<R> {
        // SAFETY: xem `Sync` ở trên; không có mutation đồng thời.
        unsafe { (*self.0.get()).as_ref().map(f) }
    }
}

/// `true` ⇔ ring đã init và map được vào VSpace U-mode (TAP dùng).
pub fn ready() -> bool {
    state().ready
}

/// PA của Frame ring (0 nếu chưa init).
pub fn ring_pa() -> u64 {
    state().pa
}

// ---------------------------------------------------------------- producer
/// Đẩy `bytes` vào ring theo slot [`CHUNK_BYTES`]; trả `(slot_ok, byte_ok)`.
///
/// Ring đầy ⇒ `send` trả 9, **không** ghi đè, đếm `drops` + giữ mã lần đầu — đây là backpressure
/// thật của kênh (không phải "bỏ im lặng").
fn push(bytes: &[u8]) -> (usize, usize) {
    let s = state();
    if !s.ready {
        return (0, 0);
    }
    let ring = Ring::new(PhysMem { base: s.phys });
    let mut sent = 0usize;
    let mut bytes_ok = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let end = (index + CHUNK_BYTES).min(bytes.len());
        let (words, count) = vifast_virtio::conchan::pack(&bytes[index..end]);
        match ring.send(&words) {
            Ok(()) => {
                sent += 1;
                bytes_ok += count;
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
    (sent, bytes_ok)
}

/// Drain `RBR` tới khi `LSR` hết dữ liệu (trần [`DRAIN_LIMIT`] byte); trả số byte đã đọc.
///
/// Dùng **cả** trong handler và trong bước "tắt nguồn rồi vét cạn" (byte đến muộn không mất).
fn drain_rbr() -> u32 {
    let mut raw = [0u8; CHUNK_BYTES];
    let mut count = 0usize;
    let mut total = 0u32;
    while (total as usize) < DRAIN_LIMIT && uart_read(UART_LSR) & LSR_DATA_READY != 0 {
        let byte = uart_read(UART_RBR);
        raw[count] = byte;
        count += 1;
        total += 1;
        if count == CHUNK_BYTES {
            let (_, ok) = push(&raw[..count]);
            BYTES.fetch_add(ok as u64, Ordering::SeqCst);
            count = 0;
        }
    }
    if count > 0 {
        let (_, ok) = push(&raw[..count]);
        BYTES.fetch_add(ok as u64, Ordering::SeqCst);
    }
    total
}

/// Handler cho một ngắt RX: trả số byte đã đọc (0 = "nguồn RX nhưng không có dữ liệu" — ca âm).
///
/// Được [`crate::plic::on_external_interrupt`] gọi khi `iir_is_rx` **và** RX đang bật.
pub fn on_rx_irq() -> u32 {
    if !state().rx_on {
        return 0;
    }
    IRQS.fetch_add(1, Ordering::SeqCst);
    let bytes = drain_rbr();
    LAST_IRQ_BYTES.store(u64::from(bytes), Ordering::SeqCst);
    if bytes == 0 {
        EMPTY_POLLS.fetch_add(1, Ordering::SeqCst);
    }
    bytes
}

/// `true` ⇔ đường RX đang bật (handler chỉ xử lý RX khi bật — giữ nguyên hành vi TX của KT-48).
pub fn rx_enabled() -> bool {
    state().rx_on
}

/// Bật `IER.RX` + priority/enable PLIC line 10 + cổng external. Trả `(ier_on, plic_on)`
/// **đọc lại từ register**.
pub fn rx_begin() -> (bool, bool) {
    let s = state();
    if !s.ready {
        return (false, false);
    }
    s.rx_on = true;
    uart_write(UART_IER, IER_RX);
    fence();
    let ier_on = uart_read(UART_IER) & IER_RX != 0;
    let plic_on = crate::plic::enable_irq(crate::UART0_IRQ);
    let gate_on = crate::plic::enable_external();
    (ier_on && plic_on && gate_on, plic_on)
}

/// Tắt `IER.RX`, đóng cổng external, tắt enable PLIC; **vét cạn** `RBR` trước khi tắt hẳn nguồn
/// (byte đến muộn vẫn vào ring — không mất byte). Trả `(bytes_late, ier_off)`.
pub fn rx_end() -> (u32, bool) {
    let s = state();
    if !s.ready {
        return (0, false);
    }
    // Cổng CPU trước (không còn ngắt mới), rồi nguồn PLIC, rồi `IER`.
    crate::plic::disable_external();
    let plic_off = crate::plic::disable_irq(crate::UART0_IRQ);
    uart_write(UART_IER, 0);
    fence();
    let ier_off = uart_read(UART_IER) & IER_RX == 0;
    let late = drain_rbr();
    s.rx_on = false;
    (late, ier_off && plic_off)
}

// ---------------------------------------------------------------- loopback (nguồn byte ca kiểm)
/// `MCR` loopback **bật** — đọc lại bit 4.
fn loopback_on() -> bool {
    uart_write(UART_MCR, MCR_LOOP);
    fence();
    uart_read(UART_MCR) & MCR_LOOP != 0
}

/// `MCR` loopback **tắt** (về 0) — đọc lại bit 4 đã 0.
fn loopback_off() -> bool {
    uart_write(UART_MCR, 0);
    fence();
    uart_read(UART_MCR) & MCR_LOOP == 0
}

/// `true` ⇔ loopback 16550 đang bật (đọc register — bằng chứng ca kiểm).
pub fn loopback_active() -> bool {
    uart_read(UART_MCR) & MCR_LOOP != 0
}

/// Ghi `bytes` vào `THR` (chỉ dùng khi loopback bật: byte quay lại `RBR` qua thiết bị).
pub fn inject_thr(bytes: &[u8]) -> usize {
    for byte in bytes {
        uart_write(UART_THR, *byte);
        INJECTED.fetch_add(1, Ordering::SeqCst);
    }
    fence();
    bytes.len()
}

/// Đường **đầy đủ** của ca kiểm: bật RX → bật loopback → ghi `bytes` vào `THR` → chờ ring nhận
/// đủ (`head` tiến `>= bytes`) → tắt loopback → vét cạn → tắt RX.
///
/// Trả `(ok, bytes_in_ring, ticks)`; `ok == false` nghĩa là thiếu byte hoặc quá hạn — TAP đỏ, và
/// mọi bộ đếm vẫn in ra để chẩn đoán.
pub fn inject_loopback(bytes: &[u8]) -> (bool, u64, u64) {
    let s = state();
    if !s.ready {
        return (false, 0, 0);
    }
    let head_before = Ring::new(PhysMem { base: s.phys }).stats().head;
    let want = vifast_virtio::conchan::chunks_for(bytes.len()) as u64;
    let (rx_on, _) = rx_begin();
    let loop_on = loopback_on();
    let written = inject_thr(bytes) as u64;
    let start = Riscv64Virt.now_ticks();
    let mut head = head_before;
    while head < head_before + want {
        if Riscv64Virt.now_ticks().wrapping_sub(start) > RX_TIMEOUT_TICKS {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            break;
        }
        head = Ring::new(PhysMem { base: s.phys }).stats().head;
        core::hint::spin_loop();
    }
    let loop_off = loopback_off();
    let (late, rx_off) = rx_end();
    let elapsed = Riscv64Virt.now_ticks().wrapping_sub(start);
    let head = Ring::new(PhysMem { base: s.phys }).stats().head;
    let ok = rx_on
        && loop_on
        && loop_off
        && rx_off
        && written == bytes.len() as u64
        && head >= head_before + want
        && late == 0;
    (ok, head - head_before, elapsed)
}

/// Đẩy `bytes` vào ring **từ đường kernel** (không qua UART) — dùng cho ca kiểm backpressure
/// (ring đầy) và ca âm của TAP; trả `(slot_ok, byte_ok)`.
pub fn produce(bytes: &[u8]) -> (usize, usize) {
    let (slots, bytes_ok) = push(bytes);
    BYTES.fetch_add(bytes_ok as u64, Ordering::SeqCst);
    (slots, bytes_ok)
}

/// Kernel đóng vai **consumer** (sau khi chương trình U-mode đã ngừng): đọc ring qua physmap,
/// trả số byte đã chép vào `out` — dùng để đối chiếu thứ tự byte khi không còn U-mode.
pub fn consume(out: &mut [u8]) -> usize {
    let s = state();
    if !s.ready {
        return 0;
    }
    let ring = Ring::new(PhysMem { base: s.phys });
    let mut words = [0u64; 4];
    let mut slot = [0u8; CHUNK_BYTES];
    let mut copied = 0usize;
    while ring.recv(&mut words).is_ok() {
        let count = vifast_virtio::conchan::unpack(&words, &mut slot);
        if count == 0 {
            break;
        }
        let room = out.len().saturating_sub(copied);
        let take = count.min(room);
        out[copied..copied + take].copy_from_slice(&slot[..take]);
        copied += take;
        if take < count {
            break;
        }
    }
    copied
}

/// `true` ⇔ ring header hợp lệ (magic/version/hình học/`head >= tail`).
pub fn ring_check() -> bool {
    let s = state();
    if !s.ready {
        return false;
    }
    Ring::new(PhysMem { base: s.phys }).check()
}

/// `(head, tail, used, seq)` của ring — bằng chứng thứ tự/không mất byte.
pub fn ring_stats() -> (u64, u64, u64, u64) {
    let s = state();
    if !s.ready {
        return (0, 0, 0, 0);
    }
    let stats = Ring::new(PhysMem { base: s.phys }).stats();
    (stats.head, stats.tail, stats.used, stats.seq)
}

/// `head` lúc init — chứng minh ring tiến từ 0 của lần boot này.
pub fn head_at_init() -> u64 {
    state().head0
}
