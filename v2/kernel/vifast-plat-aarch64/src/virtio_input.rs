// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-input` trên aarch64 — KT-337 (trước: driver đầy đủ KT-283).
//!
//! Quyết định giao thức (status/feature/queue, descriptor, arm đệm, notify, poll `used`, đọc
//! `virtio_input_config`) nằm ở [`vifast_virtio::inputdev`] — crate **thuần**, test host đầy đủ.
//! Tệp này giữ đúng những gì thuộc nền tảng aarch64:
//!
//! 1. **PA của DMA**: trang DMA là frame động identity-map (KT-338a); caller truyền thẳng PA vào
//!    [`init`] (không còn đổi alias nửa cao như thời `.bss`).
//! 2. **Cache maintenance** (KT-277): [`Dma`] cài `sync_for_device` = `dc civac` từng dòng
//!    (`cache::HANDOFF_SEQUENCE`, bỏ bước `Notify` — transport đã ghi cửa thông báo) và
//!    `sync_for_cpu` = `dc ivac`; bộ đếm là số lệnh `dc` **đã chạy** (bỏ ⇒ runner ĐỎ).
//! 3. **Đồng hồ**: `CNTPCT_EL0`; `CNTFRQ_EL0 = 0` ⇒ [`ticks_for_secs`] trả `None`, người gọi lùi
//!    về trần số vòng hoặc bỏ chờ.
//!
//! aarch64 bật **cả hai** queue (`event` + `status`) như KT-283 — `with_status_queue = true` —
//! nên `num_queues`/`status_queue_idle` của TAP không đổi.

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_virtio::blkdev::Clock;
use vifast_virtio::dma::DmaBuf;
use vifast_virtio::input::InputEvent;
pub use vifast_virtio::inputdev::{DmaPool, Input, InputReport, MmioBus};

use crate::asm;
use crate::cache;
use crate::pci::PciReport;

pub use vifast_virtio::inputdev::{EV_BITS_BYTES, NAME_MAX_BYTES};

pub use vifast_virtio::input::{EVENT_BUFS, EVENT_BYTES, QUEUE_SIZE};
pub use vifast_virtio::inputdev::{BUF_OFF, DMA_PAGE_BYTES, RING_OFF, STATUS_RING_OFF};

/// Số giây trần khi poll `used` (spec §2.7.13: không ngắt thì driver poll, phải có trần).
pub const POLL_TIMEOUT_SECS: u64 = 2;
/// Trần **số vòng** dự phòng khi `CNTFRQ_EL0 = 0` (không có trần thời gian).
pub const POLL_SPIN_LIMIT: u64 = 200_000_000;

const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2"),
    };
    assert!(RING_OFF + layout.end <= STATUS_RING_OFF);
    assert!(STATUS_RING_OFF + layout.end <= BUF_OFF);
    assert!(BUF_OFF + EVENT_BUFS * EVENT_BYTES <= DMA_PAGE_BYTES);
};

// ---------------------------------------------------------------- bộ đếm cache maintenance
//
// Bộ đếm **của chính vòng lặp phát `dc`** — nhờ vậy `# input stats: … clean_lines=` là số lệnh
// `dc` **đã chạy**, không phải một hằng viết tay.
static CLEAN_LINES: AtomicU64 = AtomicU64::new(0);
static CLEAN_BYTES: AtomicU64 = AtomicU64::new(0);
static CLEAN_OPS: AtomicU64 = AtomicU64::new(0);
static INVAL_LINES: AtomicU64 = AtomicU64::new(0);
static INVAL_OPS: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm để in evidence (đọc một lần, không tách rời từng số).
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Số sự kiện đã nhận (mọi loại).
    pub events: u64,
    /// Số `EV_SYN`/`SYN_REPORT` (mỗi lô QEMU kết thúc bằng một cái).
    pub syncs: u64,
    /// Số `EV_KEY` có `value != 0` (nhấn).
    pub key_downs: u64,
    /// Số `EV_KEY` có `value == 0` (nhả).
    pub key_ups: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lỗi driver (`used->len`/`used->id` vô lý, sự kiện không giải mã được).
    pub errors: u64,
    /// Số lần poll quá hạn (phải 0 ở đường thành công).
    pub timeouts: u64,
    /// Số lần `used->id` lệch descriptor đã arm (phải 0).
    pub id_mismatch: u64,
    /// Số dòng đã `dc civac` trước khi bàn giao ring.
    pub clean_lines: u64,
    /// Số byte đã làm sạch.
    pub clean_bytes: u64,
    /// Số lần chạy chuỗi bàn giao (mỗi lần notify).
    pub clean_ops: u64,
    /// Số dòng đã `dc ivac` sau khi thiết bị ghi.
    pub inval_lines: u64,
    /// Số lần chạy chuỗi hoàn tất.
    pub inval_ops: u64,
}

impl Stats {
    /// `true` ⇔ đường đi sạch: có sự kiện, không lỗi/timeout/lệch id, và **đã** làm sạch vùng DMA
    /// ít nhất một lần (bỏ `dc civac` ⇒ `false`).
    pub const fn ok(self) -> bool {
        self.events > 0
            && self.errors == 0
            && self.timeouts == 0
            && self.id_mismatch == 0
            && self.clean_ops > 0
            && self.clean_lines > 0
            && self.clean_bytes > 0
            && self.inval_ops > 0
            && self.inval_lines > 0
    }
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    let dev = INPUT.stats();
    Stats {
        events: dev.events,
        syncs: dev.syncs,
        key_downs: dev.key_downs,
        key_ups: dev.key_ups,
        notifies: dev.notifies,
        errors: dev.errors,
        timeouts: dev.timeouts,
        id_mismatch: dev.id_mismatch,
        clean_lines: CLEAN_LINES.load(Ordering::SeqCst),
        clean_bytes: CLEAN_BYTES.load(Ordering::SeqCst),
        clean_ops: CLEAN_OPS.load(Ordering::SeqCst),
        inval_lines: INVAL_LINES.load(Ordering::SeqCst),
        inval_ops: INVAL_OPS.load(Ordering::SeqCst),
    }
}

// ---------------------------------------------------------------- PA của trang DMA

/// PA trang DMA — đặt lúc [`init`] (caller truyền thẳng PA; trang là frame động identity-map).
static DMA_PA: AtomicU64 = AtomicU64::new(0);

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết.
pub const fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - crate::split_mmu::KERNEL_HIGH_BASE)
}

/// Bộ đếm vật lý hiện tại (`CNTPCT_EL0`).
pub fn now_ticks() -> u64 {
    asm::read_cntpct_el0()
}

/// Số tick của `secs` giây — `None` nếu `CNTFRQ_EL0 = 0` (không có tần số ⇒ không có mốc).
pub fn ticks_for_secs(secs: u64) -> Option<u64> {
    let freq = asm::read_cntfrq_el0();
    if freq == 0 {
        None
    } else {
        Some(secs.wrapping_mul(freq))
    }
}

// ---------------------------------------------------------------- adapter MMIO

struct Mmio;

impl MmioBus for Mmio {
    fn read8(&self, addr: u64) -> u8 {
        asm::mmio_read_u8(addr)
    }
    fn read16(&self, addr: u64) -> u16 {
        asm::mmio_read_u16(addr)
    }
    fn read32(&self, addr: u64) -> u32 {
        asm::mmio_read_u32(addr)
    }
    fn write8(&mut self, addr: u64, value: u8) {
        asm::mmio_write_u8(addr, value);
    }
    fn write16(&mut self, addr: u64, value: u16) {
        asm::mmio_write_u16(addr, value);
    }
    fn write32(&mut self, addr: u64, value: u32) {
        asm::mmio_write_u32(addr, value);
    }
    fn write64(&mut self, addr: u64, value: u64) {
        asm::mmio_write_u64(addr, value);
    }
    fn fence(&self) {
        // `dsb sy`: DMA của thiết bị nằm **ngoài** domain shareability của CPU.
        asm::dsb_sy();
    }
}

// ---------------------------------------------------------------- adapter DMA + cache

/// Pool DMA một vùng: frame động identity-map ngoài ảnh (KT-338a), PA do caller truyền thẳng.
struct Dma {
    base: u64,
}

impl DmaPool for Dma {
    fn base_pa(&self) -> u64 {
        self.base
    }
    fn capacity(&self) -> usize {
        DMA_PAGE_BYTES
    }
    fn bytes(&mut self) -> &mut [u8] {
        asm::dma_page_input()
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        if len == 0 || len > DMA_PAGE_BYTES || align == 0 || !align.is_power_of_two() {
            return None;
        }
        if align > DMA_PAGE_BYTES || !DMA_PAGE_BYTES.is_multiple_of(align) || self.base & 0xfff != 0
        {
            return None;
        }
        Some(DmaBuf {
            off: 0,
            len,
            pa: self.base,
        })
    }
    fn free(&mut self, _buf: DmaBuf) -> bool {
        true
    }
    fn read_u16(&self, off: usize) -> u16 {
        asm::mmio_read_u16(self.base + off as u64)
    }
    fn read_u8(&self, off: usize) -> u8 {
        asm::mmio_read_u8(self.base + off as u64)
    }
    fn sync_for_device(&mut self, off: usize, len: usize) {
        // `dc civac` từng dòng → `dsb ish` → `dsb sy`; bước `Notify` do transport làm.
        let addr = self.base + off as u64;
        let mut clean_lines = 0u64;
        for step in cache::HANDOFF_SEQUENCE {
            if matches!(step, cache::HandoffStep::Notify) {
                continue;
            }
            clean_lines += cache::apply_handoff(step, addr, len as u64, 0);
        }
        CLEAN_LINES.fetch_add(clean_lines, Ordering::SeqCst);
        CLEAN_BYTES.fetch_add(len as u64, Ordering::SeqCst);
        CLEAN_OPS.fetch_add(1, Ordering::SeqCst);
    }
    fn sync_for_cpu(&mut self, off: usize, len: usize) {
        let addr = self.base + off as u64;
        if len <= 8 {
            // Một dòng chứa `used->idx` trong vòng poll: mọi dòng đã **sạch** từ lần handoff, nên
            // `dc ivac` chỉ bỏ bản sao, không mất dữ liệu chưa ghi.
            cache::invalidate_line(addr);
            return;
        }
        let mut inval_lines = 0u64;
        for step in cache::COMPLETE_SEQUENCE {
            inval_lines += cache::apply_complete(step, addr, len as u64);
        }
        INVAL_LINES.fetch_add(inval_lines, Ordering::SeqCst);
        INVAL_OPS.fetch_add(1, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

struct Tick {
    deadline: Option<u64>,
}

impl Tick {
    fn new() -> Self {
        Self {
            deadline: ticks_for_secs(POLL_TIMEOUT_SECS).map(|t| now_ticks().wrapping_add(t)),
        }
    }
}

impl Clock for Tick {
    fn now(&mut self) -> u64 {
        now_ticks()
    }
    fn deadline(&mut self) -> Option<u64> {
        self.deadline
    }
    fn spin_limit(&self) -> u64 {
        POLL_SPIN_LIMIT
    }
}

// ---------------------------------------------------------------- trạng thái + API cũ

static INPUT: Input = Input::new();

/// Khởi tạo driver từ kết quả PCI. `dma_pa` là **PA** của trang DMA — phải căn 4 KiB.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<InputReport, &'static str> {
    if dma_pa & 0xfff != 0 {
        return Err("dma_align");
    }
    let mut dma = Dma { base: dma_pa };
    let report = INPUT.init(&mut Mmio, &mut dma, pci, true)?;
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    Ok(report)
}

/// Nhận **một** sự kiện nếu thiết bị đã hoàn tất (không chờ).
pub fn poll_event() -> Option<InputEvent> {
    INPUT.poll_event(&mut Mmio, &mut Dma { base: dma_pa() }, &mut Tick::new())
}

/// Nhận **một** sự kiện, chờ tối đa `timeout_ticks` tick `CNTPCT_EL0`.
///
/// `timeout_ticks = 0` ⇒ chỉ thử một lần (không chờ). Trần **số vòng** của adapter vẫn áp khi
/// `CNTFRQ_EL0 = 0`.
pub fn poll_event_wait(timeout_ticks: u64) -> Option<InputEvent> {
    INPUT.poll_event_wait(
        &mut Mmio,
        &mut Dma { base: dma_pa() },
        &mut Tick::new(),
        timeout_ticks,
    )
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    INPUT.ready()
}

/// Địa chỉ ISR (chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    INPUT.isr_addr()
}

/// PA trang DMA đã cấp (bằng chứng `identity` của vùng DMA).
pub fn dma_pa() -> u64 {
    DMA_PA.load(Ordering::Relaxed)
}

/// Đọc thanh ghi `device_status` hiện tại.
pub fn device_status() -> u8 {
    INPUT.device_status(&mut Mmio)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    INPUT.config_generation(&mut Mmio)
}

/// Số descriptor đã publish (một chiều) — bằng chứng ring quay vòng.
pub fn published() -> u16 {
    INPUT.published()
}

/// Cỡ queue `event` driver đã đặt.
pub fn queue_size() -> u16 {
    INPUT.queue_size()
}

/// Cỡ queue `status` driver đã đặt.
pub fn status_queue_size() -> u16 {
    INPUT.status_queue_size()
}

/// `queue_notify_off` của queue `status` (bằng chứng driver đọc **cả hai** queue).
pub fn status_notify_off() -> u16 {
    INPUT.status_notify_off()
}

/// `true` ⇔ ring `status` đã bật và **không** có descriptor nào được publish (không arm đệm).
pub fn status_queue_idle() -> bool {
    INPUT.status_queue_idle(&mut Dma { base: dma_pa() })
}
