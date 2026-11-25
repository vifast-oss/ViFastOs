// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-console** (virtio 1.0, virtio-pci modern) — KT-73 (K6), chỉ ảnh `ktest-con`.
//!
//! # Đường đi
//!
//! `pci::init_for(VirtioKind::Console)` (ECAM của KT-72, **không** nhân đôi code PCI) ⇒ BAR
//! modern + bốn capability ⇒ module này lập trình common config, dựng **hai** split virtqueue
//! (`rx` = queue 0, `tx` = queue 1 — chế độ một port khi **không** đàm phán
//! `VIRTIO_CONSOLE_F_MULTIPORT`, [`vifast_virtio::console`]), rồi `write`/`read` byte và **poll
//! `used`** (K6 chưa dùng ngắt).
//!
//! # Bất biến (D2-110B)
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1`; **không** bit `MULTIPORT`/`SIZE` ⇒
//!   **không** gửi lệnh control nào, **không** đọc `virtio_console_config`. Mọi truy cập queue
//!   qua [`vifast_virtio::console::queue_index`] (port 0); port > 0 trả `None` ⇒ fail-closed.
//! - Console **không** có byte status như virtio-blk. **`used->len` chỉ có nghĩa cho `rx`** (thiết
//!   bị ghi vào buffer device-writable — §2.6.8/§5.3.6): driver kiểm `len ≤ đệm` và trả số byte.
//!   Với `tx` **không** có buffer device-writable ⇒ `len` **không** mang nghĩa "đã nhận bao nhiêu
//!   byte" (QEMU ghi 0); driver chỉ kiểm `len ≤ số byte đã đưa` ([`vifast_virtio::console::tx_used_len_sane`])
//!   và lấy bằng chứng phân phối từ **host** (tệp `.out` của chardev), **không** đòi `len == want`.
//! - `rx` arm **trước** (một descriptor device-writable 24 B) để byte host gửi tới không bị rơi;
//!   `read` xong arm lại ngay.
//!
//! `unsafe` (MMIO + `UnsafeCell` vùng DMA) nằm trong `kernel/vifast-plat-*` (D2-17).

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::Platform;
use vifast_plat_riscv64_logic::timer::TIMEBASE_HZ;
use vifast_virtio::console::{
    negotiated_features, queue_index, rx_outcome, tx_used_len_sane, ConsoleError, MIN_QUEUE_SIZE,
    QUEUES_REQUIRED, Q_RX, Q_TX,
};
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::pci::PciReport;
use crate::Riscv64Virt;

/// Số descriptor mỗi queue (luỹ thừa 2) — console dùng **một** descriptor mỗi lần.
pub const QUEUE_SIZE: u16 = 8;
/// Cỡ trang DMA (ring + đệm).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `rx` trong trang DMA.
const RX_RING_OFF: usize = 0x000;
/// Offset ring `tx` trong trang DMA.
const TX_RING_OFF: usize = 0x200;
/// Offset đệm `rx` (device-writable).
const RX_BUF_OFF: usize = 0x400;
/// Offset đệm `tx` (driver-writable).
const TX_BUF_OFF: usize = 0x440;
/// Cỡ đệm mỗi chiều = [`vifast_virtio::console::TX_CHUNK_BYTES`].
const BUF_BYTES: usize = vifast_virtio::console::TX_CHUNK_BYTES;
/// Timeout poll `used` (tick timebase) — 2 s ảo (cùng quy ước KT-72).
const POLL_TIMEOUT_TICKS: u64 = 2 * TIMEBASE_HZ;

// ---------------------------------------------------------------- thanh ghi common config
const C_DEVICE_FEATURE_SELECT: u64 = 0x00;
const C_DEVICE_FEATURE: u64 = 0x04;
const C_DRIVER_FEATURE_SELECT: u64 = 0x08;
const C_DRIVER_FEATURE: u64 = 0x0c;
const C_NUM_QUEUES: u64 = 0x12;
const C_DEVICE_STATUS: u64 = 0x14;
const C_CONFIG_GENERATION: u64 = 0x15;
const C_QUEUE_SELECT: u64 = 0x16;
const C_QUEUE_SIZE: u64 = 0x18;
const C_QUEUE_ENABLE: u64 = 0x1c;
const C_QUEUE_NOTIFY_OFF: u64 = 0x1e;
const C_QUEUE_DESC: u64 = 0x20;
const C_QUEUE_DRIVER: u64 = 0x28;
const C_QUEUE_DEVICE: u64 = 0x30;

// ---------------------------------------------------------------- bộ đếm
static WRITES: AtomicU64 = AtomicU64::new(0);
static WRITE_BYTES: AtomicU64 = AtomicU64::new(0);
static READS: AtomicU64 = AtomicU64::new(0);
static READ_BYTES: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);
static LAST_TX_LEN: AtomicU64 = AtomicU64::new(0);
static LAST_RX_LEN: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm driver.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Số lần `write` phát.
    pub writes: u64,
    /// Tổng byte đã ghi (thiết bị xác nhận).
    pub write_bytes: u64,
    /// Số lần `read` trả byte.
    pub reads: u64,
    /// Tổng byte đã đọc.
    pub read_bytes: u64,
    /// Số lần `read` không có dữ liệu (mã `Busy`).
    pub busy: u64,
    /// Số lỗi I/O.
    pub errors: u64,
    /// Số lần ghi doorbell.
    pub notifies: u64,
    /// Số lần poll quá hạn.
    pub timeouts: u64,
    /// Số lần `used->id` lệch head (phải 0).
    pub id_mismatch: u64,
    /// `used->len` của lần `tx` **cuối** (QEMU: 0 — không phải số byte, xem `tx_used_len_sane`).
    pub last_tx_len: u64,
    /// `used->len` của lần `rx` **cuối** (số byte thiết bị đã ghi vào đệm — có nghĩa).
    pub last_rx_len: u64,
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    Stats {
        writes: WRITES.load(Ordering::SeqCst),
        write_bytes: WRITE_BYTES.load(Ordering::SeqCst),
        reads: READS.load(Ordering::SeqCst),
        read_bytes: READ_BYTES.load(Ordering::SeqCst),
        busy: BUSY.load(Ordering::SeqCst),
        errors: ERRORS.load(Ordering::SeqCst),
        notifies: NOTIFIES.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
        id_mismatch: ID_MISMATCH.load(Ordering::SeqCst),
        last_tx_len: LAST_TX_LEN.load(Ordering::SeqCst),
        last_rx_len: LAST_RX_LEN.load(Ordering::SeqCst),
    }
}

/// Trạng thái một queue (rx hoặc tx).
#[derive(Clone, Copy, Debug, Default)]
pub struct QueueState {
    /// `queue_size` thiết bị cho phép (đọc).
    pub max: u16,
    /// `queue_size` driver đặt (đã ghi + đọc lại).
    pub size: u16,
    /// `queue_notify_off` của queue.
    pub notify_off: u16,
    /// `last_used` driver đã tiêu thụ.
    pub last_used: u16,
    /// Số descriptor đã publish (một chiều).
    pub head: u16,
}

struct State {
    ready: bool,
    common: u64,
    notify: u64,
    device: u64,
    isr: u64,
    notify_mul: u32,
    rx: QueueState,
    tx: QueueState,
    /// `true` ⇔ descriptor `rx` đang chờ thiết bị ghi (đã publish).
    rx_armed: bool,
}

const INIT_STATE: State = State {
    ready: false,
    common: 0,
    notify: 0,
    device: 0,
    isr: 0,
    notify_mul: 0,
    rx: QueueState {
        max: 0,
        size: 0,
        notify_off: 0,
        last_used: 0,
        head: 0,
    },
    tx: QueueState {
        max: 0,
        size: 0,
        notify_off: 0,
        last_used: 0,
        head: 0,
    },
    rx_armed: false,
};

struct Cell<T>(UnsafeCell<T>);
// SAFETY: một hart, truy cập tuần tự trong module này.
unsafe impl<T> Sync for Cell<T> {}

static STATE: Cell<State> = Cell(UnsafeCell::new(INIT_STATE));

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: như KT-72: vùng DMA chỉ driver này chạm; thiết bị chạm qua PA (DMA) theo hợp đồng.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

fn state() -> &'static mut State {
    // SAFETY: một hart + tuần tự (xem `Cell`).
    unsafe { &mut *STATE.0.get() }
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một hart + tuần tự; vùng này không ai khác (trừ DMA của thiết bị) chạm.
    unsafe { &mut (*DMA.0.get()).0 }
}

/// PA của trang DMA (identity map nên `VA == PA`).
pub fn dma_page_addr() -> u64 {
    DMA.0.get() as *const _ as u64
}

// ---------------------------------------------------------------- MMIO
#[inline]
fn read8(addr: u64) -> u8 {
    // SAFETY: `addr` nằm trong BAR đã gán/map Device RW.
    unsafe { read_volatile(addr as *const u8) }
}
#[inline]
fn read16(addr: u64) -> u16 {
    // SAFETY: như `read8`; căn 2.
    unsafe { read_volatile(addr as *const u16) }
}
#[inline]
fn read32(addr: u64) -> u32 {
    // SAFETY: như `read8`; căn 4.
    unsafe { read_volatile(addr as *const u32) }
}
#[inline]
fn write8(addr: u64, value: u8) {
    // SAFETY: ghi thanh ghi thiết bị đã map.
    unsafe { write_volatile(addr as *mut u8, value) };
}
#[inline]
fn write16(addr: u64, value: u16) {
    // SAFETY: như `write8`.
    unsafe { write_volatile(addr as *mut u16, value) };
}
#[inline]
fn write32(addr: u64, value: u32) {
    // SAFETY: như `write8`.
    unsafe { write_volatile(addr as *mut u32, value) };
}
#[inline]
fn write64(addr: u64, value: u64) {
    // SAFETY: như `write8`.
    unsafe { write_volatile(addr as *mut u64, value) };
}
#[inline]
fn fence() {
    // SAFETY: hàng rào thứ tự bộ nhớ.
    unsafe { core::arch::asm!("fence rw, rw", options(nostack, preserves_flags)) };
}

/// Báo cáo khởi tạo — mọi trường **đọc lại từ thiết bị**.
#[derive(Clone, Copy, Debug)]
pub struct ConsoleReport {
    /// Vendor/device đọc từ PCI config.
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// BAR driver dùng.
    pub bar_index: u8,
    /// Địa chỉ BAR đã gán.
    pub bar_base: u64,
    /// Cỡ BAR đo được.
    pub bar_size: u64,
    /// Địa chỉ common config.
    pub common: u64,
    /// Địa chỉ notify.
    pub notify: u64,
    /// Địa chỉ device config.
    pub device_cfg: u64,
    /// Địa chỉ ISR.
    pub isr: u64,
    /// `notify_off_multiplier`.
    pub notify_mul: u32,
    /// `num_queues` thiết bị báo.
    pub num_queues: u16,
    /// Queue `rx` (0).
    pub rx: QueueState,
    /// Queue `tx` (1).
    pub tx: QueueState,
    /// Feature thiết bị đọc hai trang.
    pub device_features: u64,
    /// Feature driver đã ghi + đọc lại.
    pub driver_features: u64,
    /// `true` ⇔ **có** đàm phán `MULTIPORT` (phải `false` — driver K6 không dùng control queue).
    pub multiport: bool,
    /// `device_status` sau `DRIVER_OK`.
    pub status: u8,
    /// `config_generation` đọc sau khi init (bằng chứng spec §2.4.4).
    pub config_generation: u8,
    /// PA bảng descriptor `rx`.
    pub rx_desc_pa: u64,
    /// PA đệm `rx`.
    pub rx_buf_pa: u64,
    /// PA bảng descriptor `tx`.
    pub tx_desc_pa: u64,
    /// PA đệm `tx`.
    pub tx_buf_pa: u64,
}

/// Khởi tạo driver từ kết quả PCI; lỗi trả **tên chặng** để TAP in lý do đo được.
pub fn init(pci: &PciReport) -> Result<ConsoleReport, &'static str> {
    let caps = pci.caps;
    let bar = pci.bar_assigned;
    let common = bar + caps.common.offset as u64;
    let notify = bar + caps.notify.offset as u64;
    let device_cfg = bar + caps.device.offset as u64;
    let isr = bar + caps.isr.offset as u64;
    let notify_mul = caps.notify_off_multiplier;
    let rx_layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    let tx_layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    if RX_RING_OFF + rx_layout.total_bytes() > TX_RING_OFF
        || TX_RING_OFF + tx_layout.total_bytes() > RX_BUF_OFF
        || RX_BUF_OFF + BUF_BYTES > TX_BUF_OFF
        || TX_BUF_OFF + BUF_BYTES > DMA_PAGE_BYTES
    {
        return Err("layout");
    }
    for byte in page().iter_mut() {
        *byte = 0;
    }

    // ---- 1) reset + ACK + DRIVER (§2.4.2)
    write8(common + C_DEVICE_STATUS, 0);
    if read8(common + C_DEVICE_STATUS) != 0 {
        return Err("reset");
    }
    write8(common + C_DEVICE_STATUS, ACK);
    write8(common + C_DEVICE_STATUS, ACK | DRIVER);
    // ---- 2) feature thiết bị + đàm phán **duy nhất** VERSION_1
    write32(common + C_DEVICE_FEATURE_SELECT, 0);
    let low = read32(common + C_DEVICE_FEATURE) as u64;
    write32(common + C_DEVICE_FEATURE_SELECT, 1);
    let high = read32(common + C_DEVICE_FEATURE) as u64;
    let device_features = (high << 32) | low;
    if device_features & (1u64 << F_VERSION_1) == 0 {
        return Err("version1");
    }
    let wanted = negotiated_features(device_features);
    write32(common + C_DRIVER_FEATURE_SELECT, 0);
    write32(common + C_DRIVER_FEATURE, (wanted & 0xffff_ffff) as u32);
    write32(common + C_DRIVER_FEATURE_SELECT, 1);
    write32(common + C_DRIVER_FEATURE, (wanted >> 32) as u32);
    write32(common + C_DRIVER_FEATURE_SELECT, 0);
    let drv_low = read32(common + C_DRIVER_FEATURE) as u64;
    write32(common + C_DRIVER_FEATURE_SELECT, 1);
    let drv_high = read32(common + C_DRIVER_FEATURE) as u64;
    let driver_features = (drv_high << 32) | drv_low;
    if driver_features != wanted {
        return Err("features");
    }
    let multiport = vifast_virtio::console::multiport_negotiated(driver_features);
    if multiport {
        return Err("multiport");
    }
    // ---- 3) FEATURES_OK + đọc lại
    write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
    if !features_ok(read8(common + C_DEVICE_STATUS)) {
        return Err("features_ok");
    }
    // ---- 4) hai queue: rx (0) + tx (1). Chỉ số lấy từ hàm thuần (port 0, MULTIPORT không đàm
    // phán) — không viết tay 0/1 ở hai chỗ.
    let num_queues = read16(common + C_NUM_QUEUES);
    if num_queues < QUEUES_REQUIRED {
        return Err("queues");
    }
    let dma = dma_page_addr();
    let setup = |index: u16, ring_off: usize| -> Result<QueueState, &'static str> {
        write16(common + C_QUEUE_SELECT, index);
        let max = read16(common + C_QUEUE_SIZE);
        if max < MIN_QUEUE_SIZE || !max.is_power_of_two() {
            return Err("queue_size");
        }
        let size = QUEUE_SIZE.min(max);
        write16(common + C_QUEUE_SIZE, size);
        if read16(common + C_QUEUE_SIZE) != size {
            return Err("queue_size");
        }
        let layout = VqLayout::new(size).ok_or("layout")?;
        let base = dma + ring_off as u64;
        write64(common + C_QUEUE_DESC, base + layout.desc_off as u64);
        write64(common + C_QUEUE_DRIVER, base + layout.avail_off as u64);
        write64(common + C_QUEUE_DEVICE, base + layout.used_off as u64);
        fence();
        write16(common + C_QUEUE_ENABLE, 1);
        if read16(common + C_QUEUE_ENABLE) != 1 {
            return Err("queue_enable");
        }
        Ok(QueueState {
            max,
            size,
            notify_off: read16(common + C_QUEUE_NOTIFY_OFF),
            last_used: 0,
            head: 0,
        })
    };
    let rx_index = queue_index(0, false, driver_features).ok_or("multiport")?;
    let tx_index = queue_index(0, true, driver_features).ok_or("multiport")?;
    if rx_index != Q_RX || tx_index != Q_TX {
        return Err("queue_index");
    }
    let rx = setup(rx_index, RX_RING_OFF)?;
    let tx = setup(tx_index, TX_RING_OFF)?;
    // ---- 5) DRIVER_OK
    write8(
        common + C_DEVICE_STATUS,
        ACK | DRIVER | FEATURES_OK | DRIVER_OK,
    );
    let status = read8(common + C_DEVICE_STATUS);
    if !status_sequence_ok(status) {
        return Err("driver_ok");
    }
    let config_generation = read8(common + C_CONFIG_GENERATION);
    // ---- 6) lưu trạng thái rồi arm `rx` **trước** để byte host tới không rơi
    let s = state();
    s.ready = true;
    s.common = common;
    s.notify = notify;
    s.device = device_cfg;
    s.isr = isr;
    s.notify_mul = notify_mul;
    s.rx = rx;
    s.tx = tx;
    s.rx_armed = false;
    if !arm_rx() {
        return Err("arm_rx");
    }
    let ring_base = |off: usize| dma + off as u64;
    let layout = VqLayout::new(rx.size).ok_or("layout")?;
    let tx_layout = VqLayout::new(tx.size).ok_or("layout")?;
    Ok(ConsoleReport {
        vendor: pci.vendor,
        device: pci.device,
        bar_index: pci.bar_index,
        bar_base: bar,
        bar_size: pci.bar_size,
        common,
        notify,
        device_cfg,
        isr,
        notify_mul,
        num_queues,
        rx,
        tx,
        device_features,
        driver_features,
        multiport,
        status,
        config_generation,
        rx_desc_pa: ring_base(RX_RING_OFF) + layout.desc_off as u64,
        rx_buf_pa: dma + RX_BUF_OFF as u64,
        tx_desc_pa: ring_base(TX_RING_OFF) + tx_layout.desc_off as u64,
        tx_buf_pa: dma + TX_BUF_OFF as u64,
    })
}

/// Queue `rx`/`tx` đang dùng (từ trạng thái — **không** hardcode lại cỡ).
fn queue_with(ring_off: usize, size: u16) -> Result<SplitQueue<'static>, ConsoleError> {
    let page = page();
    let slice = &mut page[ring_off..];
    SplitQueue::new(slice, size).ok_or(ConsoleError::NotReady)
}

/// Ghi doorbell của queue `index` (dùng `notify_off` đã cache + multiplier).
fn notify_queue(index: u16) {
    let s = state();
    let off = if index == Q_TX {
        s.tx.notify_off
    } else {
        s.rx.notify_off
    };
    write16(s.notify + off as u64 * s.notify_mul as u64, index);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
}

/// Arm **một** descriptor `rx` 24 B (device-writable) rồi publish + notify.
fn arm_rx() -> bool {
    let s = state();
    let size = s.rx.size;
    if size == 0 || s.rx_armed {
        return false;
    }
    let head = s.rx.head;
    let ok = {
        let mut queue = match queue_with(RX_RING_OFF, size) {
            Ok(queue) => queue,
            Err(_) => return false,
        };
        if !queue.set_desc(
            head % size,
            Desc {
                addr: dma_page_addr() + RX_BUF_OFF as u64,
                len: BUF_BYTES as u32,
                flags: DescFlags::WRITE,
                next: 0,
            },
        ) {
            return false;
        }
        queue.push_avail(head % size);
        true
    };
    if !ok {
        return false;
    }
    fence();
    notify_queue(Q_RX);
    s.rx.head = s.rx.head.wrapping_add(1);
    s.rx_armed = true;
    true
}

/// Một lần đọc `used`: `Some((id, len))` nếu thiết bị đã hoàn tất thêm phần tử, `None` nếu chưa.
fn poll_used_once(ring_off: usize, size: u16, last_used: u16) -> Option<(u32, u32)> {
    let dma = dma_page_addr();
    let layout = VqLayout::new(size)?;
    let used_idx_addr = dma + ring_off as u64 + layout.used_idx_off as u64;
    // SAFETY: `used_idx_addr` nằm trong trang DMA đã bàn giao; đọc volatile để không bị hoist.
    let idx = unsafe { read_volatile(used_idx_addr as *const u16) };
    if idx == last_used {
        return None;
    }
    fence();
    let queue = queue_with(ring_off, size).ok()?;
    Some(queue.used_elem(last_used & (size - 1)))
}

/// Poll `used` của một queue tới khi tiến khỏi `last_used`; trả `(id, len)`; `None` = quá hạn.
fn poll_used(ring_off: usize, size: u16, last_used: u16) -> Option<(u32, u32)> {
    let dma = dma_page_addr();
    let layout = VqLayout::new(size)?;
    let used_idx_addr = dma + ring_off as u64 + layout.used_idx_off as u64;
    let start = Riscv64Virt.now_ticks();
    let deadline = start.wrapping_add(POLL_TIMEOUT_TICKS);
    loop {
        // SAFETY: `used_idx_addr` nằm trong trang DMA đã bàn giao; đọc volatile để không bị hoist.
        let idx = unsafe { read_volatile(used_idx_addr as *const u16) };
        if idx != last_used {
            fence();
            let element = {
                let queue = queue_with(ring_off, size).ok()?;
                queue.used_elem(last_used & (size - 1))
            };
            return Some(element);
        }
        if Riscv64Virt.now_ticks() > deadline {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            return None;
        }
        core::hint::spin_loop();
    }
}

/// Ghi `bytes` (≤ [`BUF_BYTES`]) qua queue `tx`; thiết bị phải xác nhận đúng số byte.
pub fn write(bytes: &[u8]) -> Result<usize, ConsoleError> {
    let s = state();
    if !s.ready {
        return Err(ConsoleError::NotReady);
    }
    if bytes.is_empty() || bytes.len() > BUF_BYTES {
        return Err(ConsoleError::TooLarge);
    }
    let count = bytes.len();
    page()[TX_BUF_OFF..TX_BUF_OFF + count].copy_from_slice(bytes);
    let size = s.tx.size;
    let slot = s.tx.head % size;
    {
        let mut queue = queue_with(TX_RING_OFF, size)?;
        if !queue.set_desc(
            slot,
            Desc {
                addr: dma_page_addr() + TX_BUF_OFF as u64,
                len: count as u32,
                flags: DescFlags::NONE,
                next: 0,
            },
        ) {
            return Err(ConsoleError::NotReady);
        }
        queue.push_avail(slot);
    }
    fence();
    notify_queue(Q_TX);
    WRITES.fetch_add(1, Ordering::SeqCst);
    s.tx.head = s.tx.head.wrapping_add(1);
    let last = s.tx.last_used;
    let Some((id, len)) = poll_used(TX_RING_OFF, size, last) else {
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err(ConsoleError::Io);
    };
    if id != u32::from(slot) {
        ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err(ConsoleError::Io);
    }
    s.tx.last_used = s.tx.last_used.wrapping_add(1);
    // `used->len` của tx **không** mang nghĩa số byte đã nhận (không có buffer device-writable,
    // §2.6.8): chỉ kiểm "không vượt số byte đã đưa"; bằng chứng phân phối lấy từ phía host.
    if !tx_used_len_sane(len, count as u32) {
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err(ConsoleError::Io);
    }
    LAST_TX_LEN.store(u64::from(len), Ordering::SeqCst);
    WRITE_BYTES.fetch_add(count as u64, Ordering::SeqCst);
    Ok(count)
}

/// Đọc **một** descriptor `rx` đã hoàn tất, chờ tối đa `timeout_ticks` (`0` = thử một lần).
///
/// `Err(Busy)` = chưa có dữ liệu hoặc thiết bị hoàn tất descriptor rỗng — **không** chặn mãi
/// (ca âm "không có dữ liệu"); `rx` được arm lại ngay sau mỗi lần hoàn tất.
pub fn read_wait(out: &mut [u8; BUF_BYTES], timeout_ticks: u64) -> Result<usize, ConsoleError> {
    let s = state();
    if !s.ready {
        return Err(ConsoleError::NotReady);
    }
    if !s.rx_armed {
        return Err(ConsoleError::Busy);
    }
    let size = s.rx.size;
    let last = s.rx.last_used;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let element = loop {
        match poll_used_once(RX_RING_OFF, size, last) {
            Some(element) => break element,
            None => {
                if timeout_ticks == 0 || Riscv64Virt.now_ticks() > deadline {
                    BUSY.fetch_add(1, Ordering::SeqCst);
                    return Err(ConsoleError::Busy);
                }
                core::hint::spin_loop();
            }
        }
    };
    let (id, len) = element;
    if id != u32::from((s.rx.head.wrapping_sub(1)) % size) {
        ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err(ConsoleError::Io);
    }
    s.rx.last_used = s.rx.last_used.wrapping_add(1);
    s.rx_armed = false;
    let count = match rx_outcome(len, BUF_BYTES as u32) {
        Ok(count) => count as usize,
        Err(ConsoleError::Busy) => {
            // Thiết bị hoàn tất descriptor **rỗng**: arm lại rồi trả `Busy` (không phải lỗi).
            let _ = arm_rx();
            BUSY.fetch_add(1, Ordering::SeqCst);
            return Err(ConsoleError::Busy);
        }
        Err(error) => {
            ERRORS.fetch_add(1, Ordering::SeqCst);
            let _ = arm_rx();
            return Err(error);
        }
    };
    out[..count].copy_from_slice(&page()[RX_BUF_OFF..RX_BUF_OFF + count]);
    LAST_RX_LEN.store(u64::from(len), Ordering::SeqCst);
    let _ = arm_rx();
    READS.fetch_add(1, Ordering::SeqCst);
    READ_BYTES.fetch_add(count as u64, Ordering::SeqCst);
    Ok(count)
}

/// Như [`read_wait`] nhưng **không chờ** (một lần thử) — dùng cho ca âm "không có dữ liệu".
pub fn read_try(out: &mut [u8; BUF_BYTES]) -> Result<usize, ConsoleError> {
    read_wait(out, 0)
}

/// Đọc có chờ 2 s ảo (cùng hạn `poll_used` của KT-72).
pub fn read(out: &mut [u8; BUF_BYTES]) -> Result<usize, ConsoleError> {
    read_wait(out, POLL_TIMEOUT_TICKS)
}

/// `true` ⇔ driver sẵn sàng.
pub fn ready() -> bool {
    state().ready
}

/// `device_status` hiện tại (bằng chứng driver còn `DRIVER_OK`).
pub fn device_status() -> u8 {
    let s = state();
    if s.common == 0 {
        return 0;
    }
    read8(s.common + C_DEVICE_STATUS)
}

/// `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    let s = state();
    if s.common == 0 {
        return 0;
    }
    read8(s.common + C_CONFIG_GENERATION)
}

/// Địa chỉ ISR (chỉ đọc chẩn đoán — K6 chưa dùng ngắt console).
pub fn isr_addr() -> u64 {
    state().isr
}

/// `(rx_armed, rx_head, tx_head)` — bằng chứng arm/tiêu thụ queue.
pub fn queue_state() -> (bool, u16, u16) {
    let s = state();
    (s.rx_armed, s.rx.head, s.tx.head)
}

const _: () = {
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("queue size khong hop le"),
    };
    assert!(RX_RING_OFF + layout.end <= TX_RING_OFF);
    assert!(TX_RING_OFF + layout.end <= RX_BUF_OFF);
    assert!(RX_BUF_OFF + BUF_BYTES <= TX_BUF_OFF);
    assert!(TX_BUF_OFF + BUF_BYTES <= DMA_PAGE_BYTES);
};
