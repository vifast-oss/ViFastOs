// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test cho transport VirtIO-PCI + driver `virtio-blk` dùng chung (KT-334).
//!
//! ```text
//! cargo test --manifest-path v2/Cargo.toml -p vifast-virtio --test transport
//! ```
//!
//! # Vì sao test này tồn tại
//!
//! Trước KT-334, logic driver chỉ chạy được trên QEMU: ba `virtio_blk.rs` của platform trộn
//! `unsafe` MMIO/DMA với quyết định giao thức. Nay quyết định nằm trong crate thuần; test này
//! dựng **thiết bị giả trong bộ nhớ** để chốt từng bất biến mà QEMU không nói rõ:
//!
//! 1. thứ tự trạng thái/feature/queue của §2.4.2, kể cả ca **thiếu** `VIRTIO_F_VERSION_1`;
//! 2. **cache hook**: `sync_for_device` **phải** chạy **trước** notify (KT-277/aarch64) — đột
//!    biến bỏ lời gọi làm test đỏ (đếm lời gọi, không dựa vào hiệu ứng cache mà TCG không mô
//!    hình hoá);
//! 3. descriptor vòng: header → data → status, `used->id` khớp head, byte status do “thiết bị”
//!    ghi;
//! 4. lỗi trạng thái: thiết bị từ chối `FEATURES_OK`, queue quá nhỏ, `config_generation` xé,
//!    `used->id` lệch, hết hạn poll;
//! 5. `DmaPool`/`DmaArena` cấp/thu vùng vật lý liên tục **và** đọc `used->idx`/status từ DMA
//!    (RAM), không phải MMIO.
//!
//! Thiết bị giả **không** phải QEMU: nó chỉ mô phỏng đúng phần giao thức mà driver đọc/ghi, nên
//! một lệch thứ tự sẽ hiện ra ngay ở đây thay vì chờ một lần soak.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use vifast_virtio::blk::{negotiated_features, BlkError, T_FLUSH, T_IN, T_OUT};
use vifast_virtio::blkdev::{
    ascii16, has_magic, hex16, sector0_evidence, Blk, BlkReport, Clock, Transport, DATA_OFF,
    DMA_PAGE_BYTES, QUEUE_SIZE, REQ_OFF, SECTOR0_MAGIC, SECTOR_BYTES, STATUS_OFF,
};
use vifast_virtio::dma::{DmaArena, DmaBuf, DmaPool};
use vifast_virtio::mmio::MmioBus;
use vifast_virtio::pci::{BarKind, CapType, VirtioCap, VirtioCaps};
use vifast_virtio::pcibus::PciReport;
use vifast_virtio::queue::VqLayout;

// ---------------------------------------------------------------- địa chỉ mô phỏng

const BAR: u64 = 0x1000_0000;
const COMMON: u64 = BAR + 0x1000;
const NOTIFY: u64 = BAR + 0x2000;
const DEVICE_CFG: u64 = BAR + 0x3000;
const ISR: u64 = BAR + 0x4000;
const NOTIFY_OFF: u16 = 1;
const NOTIFY_MUL: u32 = 4;
const NOTIFY_ADDR: u64 = NOTIFY + NOTIFY_OFF as u64 * NOTIFY_MUL as u64;
const DMA_BASE_PA: u64 = 0x8000_0000;

// Thanh ghi common config (spec virtio 1.0 §4.1.4.3) — chép số ở đây là **có chủ đích**:
// test phải độc lập với hằng private của driver, nếu không nó chỉ kiểm chính nó.
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

const F_VERSION_1: u64 = 1 << 32;
const F_FLUSH: u64 = 1 << 9;
const CAPACITY_SECTORS: u64 = 8;

/// Báo cáo PCI giả: BAR4 64-bit, bốn capability **cùng** một BAR.
fn pci_report() -> PciReport {
    let cap = |cap_type: CapType, offset: u32| VirtioCap {
        cap_type,
        bar: 4,
        offset,
        length: 0x100,
    };
    PciReport {
        access: 0,
        ecam_base: 0,
        functions_seen: 1,
        bus: 0,
        dev: 6,
        func: 0,
        vendor: 0x1af4,
        device: 0x1042,
        class: 0x0001_0000,
        revision: 1,
        subclass: 0,
        prog_if: 0,
        header_type: 0,
        bar_index: 4,
        bar_kind: BarKind::Memory64,
        bar_size: 0x10_000,
        bar_base: BAR,
        bar_assigned: BAR,
        bar_readback: BAR,
        command: 0x6,
        caps: VirtioCaps {
            common: cap(CapType::Common, 0x1000),
            notify: cap(CapType::Notify, 0x2000),
            isr: cap(CapType::Isr, 0x4000),
            device: cap(CapType::Device, 0x3000),
            notify_off_multiplier: NOTIFY_MUL,
            bar: 4,
        },
        ecam_addr: 0,
        ecam_pa: 0,
        ecam_va: 0,
        other_bars: 1,
        reserved_ranges: 1,
        overlap_checked: true,
        cap_ranges_ok: true,
    }
}

// ---------------------------------------------------------------- thiết bị giả (MMIO)

/// Nhật ký sự kiện chia sẻ giữa "thiết bị" (MMIO) và "vùng DMA" — chốt **thứ tự**.
type Events = Rc<RefCell<Vec<&'static str>>>;

/// Config space + common config của một thiết bị virtio-blk giả.
struct FakeDev {
    status: u8,
    device_feature_select: u32,
    driver_feature_select: u32,
    driver_features: u64,
    device_features: u64,
    queue_select: u16,
    queue_size: u16,
    queue_enable: bool,
    queue_desc: u64,
    queue_driver: u64,
    queue_device: u64,
    queue_notify_off: u16,
    num_queues: u16,
    /// Thiết bị **từ chối** `FEATURES_OK` (giữ ACK|DRIVER) — ca âm §2.4.2 bước 5.
    reject_features: bool,
    /// `config_generation` đổi giữa hai lần đọc (config bị xé).
    torn_generation: bool,
    generation_reads: Cell<u32>,
    config: [u8; 24],
    notifies: u32,
    last_notify_queue: u16,
    events: Events,
}

impl FakeDev {
    fn new(events: Events) -> Self {
        let mut config = [0u8; 24];
        config[0..8].copy_from_slice(&CAPACITY_SECTORS.to_le_bytes());
        config[8..12].copy_from_slice(&0u32.to_le_bytes()); // size_max
        config[12..16].copy_from_slice(&1u32.to_le_bytes()); // seg_max
        config[20..24].copy_from_slice(&512u32.to_le_bytes()); // blk_size
        Self {
            status: 0,
            device_feature_select: 0,
            driver_feature_select: 0,
            driver_features: 0,
            device_features: F_VERSION_1 | F_FLUSH,
            queue_select: 0,
            queue_size: 16,
            queue_enable: false,
            queue_desc: 0,
            queue_driver: 0,
            queue_device: 0,
            queue_notify_off: NOTIFY_OFF,
            num_queues: 2,
            reject_features: false,
            torn_generation: false,
            generation_reads: Cell::new(0),
            config,
            notifies: 0,
            last_notify_queue: 0,
            events,
        }
    }

    fn read8(&self, addr: u64) -> u8 {
        if (DEVICE_CFG..DEVICE_CFG + 24).contains(&addr) {
            return self.config[(addr - DEVICE_CFG) as usize];
        }
        let off = addr - COMMON;
        match off {
            C_DEVICE_STATUS => self.status,
            C_CONFIG_GENERATION => {
                let n = self.generation_reads.get();
                self.generation_reads.set(n + 1);
                if self.torn_generation && n >= 1 {
                    4
                } else {
                    3
                }
            }
            C_QUEUE_NOTIFY_OFF => self.queue_notify_off as u8,
            _ => 0,
        }
    }

    fn read16(&self, addr: u64) -> u16 {
        let off = addr - COMMON;
        match off {
            C_NUM_QUEUES => self.num_queues,
            C_QUEUE_SIZE => self.queue_size,
            C_QUEUE_SELECT => self.queue_select,
            C_QUEUE_ENABLE => self.queue_enable as u16,
            C_QUEUE_NOTIFY_OFF => self.queue_notify_off,
            C_DEVICE_STATUS => self.status as u16,
            _ => 0,
        }
    }

    fn read32(&self, addr: u64) -> u32 {
        let off = addr - COMMON;
        match off {
            C_DEVICE_FEATURE => {
                if self.device_feature_select == 0 {
                    (self.device_features & 0xffff_ffff) as u32
                } else {
                    (self.device_features >> 32) as u32
                }
            }
            C_DRIVER_FEATURE => {
                if self.driver_feature_select == 0 {
                    (self.driver_features & 0xffff_ffff) as u32
                } else {
                    (self.driver_features >> 32) as u32
                }
            }
            _ => 0,
        }
    }

    fn write8(&mut self, addr: u64, value: u8) {
        let off = addr - COMMON;
        if off == C_DEVICE_STATUS {
            self.status = if self.reject_features && value & 0x08 != 0 {
                value & !0x08
            } else {
                value
            };
        }
    }

    fn write16(&mut self, addr: u64, value: u16) {
        if addr == NOTIFY_ADDR {
            self.notifies += 1;
            self.last_notify_queue = value;
            self.events.borrow_mut().push("notify");
            return;
        }
        let off = addr - COMMON;
        match off {
            C_QUEUE_SELECT => self.queue_select = value,
            C_QUEUE_SIZE => self.queue_size = value,
            C_QUEUE_ENABLE => self.queue_enable = value == 1,
            _ => {}
        }
    }

    fn write32(&mut self, addr: u64, value: u32) {
        let off = addr - COMMON;
        match off {
            C_DEVICE_FEATURE_SELECT => self.device_feature_select = value,
            C_DRIVER_FEATURE_SELECT => self.driver_feature_select = value,
            C_DRIVER_FEATURE => {
                let mask = 0xffff_ffffu64;
                if self.driver_feature_select == 0 {
                    self.driver_features = (self.driver_features & !mask) | value as u64;
                } else {
                    self.driver_features = (self.driver_features & mask) | ((value as u64) << 32);
                }
            }
            _ => {}
        }
    }

    fn write64(&mut self, addr: u64, value: u64) {
        let off = addr - COMMON;
        match off {
            C_QUEUE_DESC => self.queue_desc = value,
            C_QUEUE_DRIVER => self.queue_driver = value,
            C_QUEUE_DEVICE => self.queue_device = value,
            _ => {}
        }
    }
}

/// `MmioBus` trên thiết bị giả.
struct FakeBus {
    dev: FakeDev,
}

impl MmioBus for FakeBus {
    fn read8(&self, addr: u64) -> u8 {
        self.dev.read8(addr)
    }
    fn read16(&self, addr: u64) -> u16 {
        self.dev.read16(addr)
    }
    fn read32(&self, addr: u64) -> u32 {
        self.dev.read32(addr)
    }
    fn write8(&mut self, addr: u64, value: u8) {
        self.dev.write8(addr, value);
    }
    fn write16(&mut self, addr: u64, value: u16) {
        self.dev.write16(addr, value);
    }
    fn write32(&mut self, addr: u64, value: u32) {
        self.dev.write32(addr, value);
    }
    fn write64(&mut self, addr: u64, value: u64) {
        self.dev.write64(addr, value);
    }
}

// ---------------------------------------------------------------- vùng DMA + "thiết bị" hoàn tất

/// Pool DMA 4 KiB + mô phỏng thiết bị đọc ring/ghi `used` **trong** `sync_for_device`.
///
/// Chọn chỗ này vì đúng ngữ nghĩa hook: `sync_for_device` là thời điểm driver **bàn giao** vùng
/// DMA. Nhờ vậy test chứng minh được cả hai: (a) hook được gọi trước notify (đếm + thứ tự);
/// (b) nội dung ring/request là thứ thiết bị thật sẽ đọc.
struct FakeDma {
    mem: Vec<u8>,
    arena: DmaArena,
    /// `false` ⇒ thiết bị **không** bao giờ hoàn tất (đo timeout/spin-limit).
    simulate: bool,
    /// Byte status "thiết bị" ghi (0 = OK, 1 = IOERR).
    status_byte: u8,
    /// `used->id` "thiết bị" ghi (0 = đúng head; khác 0 = ca lỗi).
    used_id: u32,
    last_avail: u16,
    device_syncs: u32,
    cpu_polls: u32,
    cpu_full: u32,
    last_kind: Option<u32>,
    last_sector: Option<u64>,
    last_desc_len: Option<u32>,
    last_write: Option<[u8; SECTOR_BYTES]>,
    events: Events,
}

/// Byte thứ `i` của sector mà thiết bị giả trả về — **hàm**, không mảng chép tay.
fn data_pattern(sector: u64, i: usize) -> u8 {
    (sector as u8).wrapping_mul(31).wrapping_add(i as u8)
}

impl FakeDma {
    fn new(events: Events) -> Self {
        Self {
            mem: vec![0u8; DMA_PAGE_BYTES],
            arena: DmaArena::new(DMA_PAGE_BYTES),
            simulate: true,
            status_byte: 0,
            used_id: 0,
            last_avail: 0,
            device_syncs: 0,
            cpu_polls: 0,
            cpu_full: 0,
            last_kind: None,
            last_sector: None,
            last_desc_len: None,
            last_write: None,
            events,
        }
    }

    fn u16_at(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.mem[off], self.mem[off + 1]])
    }

    fn u32_at(&self, off: usize) -> u32 {
        u32::from_le_bytes([
            self.mem[off],
            self.mem[off + 1],
            self.mem[off + 2],
            self.mem[off + 3],
        ])
    }

    fn u64_at(&self, off: usize) -> u64 {
        u64::from_le_bytes(self.mem[off..off + 8].try_into().expect("8 byte"))
    }

    fn set_u16(&mut self, off: usize, value: u16) {
        self.mem[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn set_u32(&mut self, off: usize, value: u32) {
        self.mem[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn sim_off(pa: u64) -> usize {
        (pa - DMA_BASE_PA) as usize
    }

    /// Thiết bị đọc chuỗi descriptor rồi ghi `used` — mô phỏng §2.7.6.
    fn simulate_device(&mut self) {
        let layout = VqLayout::new(QUEUE_SIZE).expect("layout");
        let avail_idx = self.u16_at(layout.avail_idx_off);
        if avail_idx == self.last_avail {
            return;
        }
        self.last_avail = avail_idx;
        let slot = (avail_idx.wrapping_sub(1)) & (QUEUE_SIZE - 1);
        let head = self.u16_at(layout.avail_ring_off + slot as usize * 2);
        // Descriptor đầu = request header.
        let mut index = head;
        let mut first = true;
        let mut last_data: Option<(usize, u32, bool)> = None;
        let mut status_at = 0usize;
        loop {
            let at = layout.desc_off + index as usize * 16;
            let addr = Self::sim_off(self.u64_at(at));
            let len = self.u32_at(at + 8);
            let flags = self.u16_at(at + 12);
            let next = self.u16_at(at + 14);
            if first {
                self.last_kind = Some(self.u32_at(addr));
                self.last_sector = Some(self.u64_at(addr + 8));
                first = false;
            } else if flags & 0b10 == 0 {
                // Descriptor device-readable cuối cùng (header/OUT data): ghi nhận độ dài.
                last_data = Some((addr, len, false));
            } else {
                // Device-writable: data (IN) hoặc status (len 1).
                if len == 1 {
                    status_at = addr;
                } else {
                    last_data = Some((addr, len, true));
                }
            }
            if flags & 1 == 0 {
                break;
            }
            index = next;
        }
        if let Some((addr, len, writable)) = last_data {
            self.last_desc_len = Some(len);
            if writable {
                let sector = self.last_sector.unwrap_or(0);
                for i in 0..len as usize {
                    self.mem[addr + i] = data_pattern(sector, i);
                }
            } else {
                let mut buf = [0u8; SECTOR_BYTES];
                buf.copy_from_slice(&self.mem[addr..addr + SECTOR_BYTES]);
                self.last_write = Some(buf);
            }
        }
        self.mem[status_at] = self.status_byte;
        // `used->ring[slot] = (id, len)` rồi `used->idx += 1` (thứ tự thiết bị).
        let used_idx = self.u16_at(layout.used_idx_off);
        let uslot = (used_idx & (QUEUE_SIZE - 1)) as usize;
        let at = layout.used_ring_off + uslot * 8;
        self.set_u32(at, self.used_id);
        self.set_u32(at + 4, SECTOR_BYTES as u32);
        self.set_u16(layout.used_idx_off, used_idx.wrapping_add(1));
    }
}

impl DmaPool for FakeDma {
    fn base_pa(&self) -> u64 {
        DMA_BASE_PA
    }
    fn capacity(&self) -> usize {
        self.mem.len()
    }
    fn bytes(&mut self) -> &mut [u8] {
        &mut self.mem
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        self.arena.alloc(DMA_BASE_PA, len, align)
    }
    fn free(&mut self, buf: DmaBuf) -> bool {
        self.arena.free(buf)
    }
    fn read_u16(&self, off: usize) -> u16 {
        self.u16_at(off)
    }
    fn read_u8(&self, off: usize) -> u8 {
        self.mem[off]
    }
    fn sync_for_device(&mut self, _off: usize, _len: usize) {
        self.device_syncs += 1;
        self.events.borrow_mut().push("sync_device");
        if self.simulate {
            self.simulate_device();
        }
    }
    fn sync_for_cpu(&mut self, _off: usize, len: usize) {
        if len <= 8 {
            self.cpu_polls += 1;
        } else {
            self.cpu_full += 1;
        }
    }
}

// ---------------------------------------------------------------- đồng hồ giả

/// Đồng hồ điều khiển được: mỗi lần đọc tăng `step` tick.
struct TestClock {
    now: u64,
    step: u64,
    /// Trần tương đối so với lúc gọi `deadline`; `None` ⇒ dùng `spin_limit`.
    timeout: Option<u64>,
    spin_limit: u64,
}

impl TestClock {
    fn new(step: u64, timeout: Option<u64>, spin_limit: u64) -> Self {
        Self {
            now: 0,
            step,
            timeout,
            spin_limit,
        }
    }
}

impl Clock for TestClock {
    fn now(&mut self) -> u64 {
        self.now = self.now.wrapping_add(self.step);
        self.now
    }
    fn deadline(&mut self) -> Option<u64> {
        self.timeout.map(|ticks| self.now.wrapping_add(ticks))
    }
    fn spin_limit(&self) -> u64 {
        self.spin_limit
    }
}

// ---------------------------------------------------------------- khung chạy

struct Device {
    blk: Blk,
    bus: FakeBus,
    dma: FakeDma,
    report: BlkReport,
    clock: TestClock,
}

fn boot(dev: FakeDev, mut dma: FakeDma) -> Device {
    let blk = Blk::new();
    let transport = Transport::from_report(&pci_report());
    let mut bus = FakeBus { dev };
    let report = blk
        .init(&mut bus, &transport, &mut dma)
        .expect("init phải thành công với thiết bị giả hợp lệ");
    Device {
        blk,
        bus,
        dma,
        report,
        clock: TestClock::new(1, Some(1_000_000), 0),
    }
}

fn new_events() -> Events {
    Rc::new(RefCell::new(Vec::new()))
}

fn valid_device(events: &Events) -> Device {
    boot(
        FakeDev::new(Rc::clone(events)),
        FakeDma::new(Rc::clone(events)),
    )
}

fn event_order(events: &Events) -> (usize, usize) {
    let list = events.borrow();
    let sync = list
        .iter()
        .position(|e| *e == "sync_device")
        .unwrap_or(usize::MAX);
    let notify = list
        .iter()
        .position(|e| *e == "notify")
        .unwrap_or(usize::MAX);
    (sync, notify)
}

// ---------------------------------------------------------------- 1. init: feature/queue/config

#[test]
fn init_negotiates_version1_and_reads_config() {
    let events = new_events();
    let device = valid_device(&events);
    let report = device.report;
    assert_eq!(report.vendor, 0x1af4);
    assert_eq!(report.device, 0x1042);
    assert_eq!(report.queue_max, 16);
    assert_eq!(report.queue_size, QUEUE_SIZE);
    assert_eq!(report.num_queues, 2);
    assert!(report.flush_ok);
    assert_eq!(report.device_features, F_VERSION_1 | F_FLUSH);
    assert_eq!(
        report.driver_features,
        negotiated_features(report.device_features)
    );
    assert_eq!(report.status & 0x0f, 0x0f);
    assert_eq!(report.capacity, CAPACITY_SECTORS);
    assert_eq!(report.blk_size, 512);
    assert_eq!(report.seg_max, 1);
    assert_eq!(report.config_generation, 3);
    assert_eq!(report.notify_off, NOTIFY_OFF);
    assert_eq!(report.isr, ISR);
    assert_eq!(report.common, COMMON);
    assert_eq!(report.notify, NOTIFY);
    assert_eq!(report.device_cfg, DEVICE_CFG);
    assert_eq!(report.notify_mul, NOTIFY_MUL);
    assert_eq!(report.notify_addr, NOTIFY_ADDR);
    let layout = VqLayout::new(QUEUE_SIZE).expect("layout");
    assert_eq!(report.dma_pa, DMA_BASE_PA);
    assert_eq!(report.desc, DMA_BASE_PA + layout.desc_off as u64);
    assert_eq!(report.avail, DMA_BASE_PA + layout.avail_off as u64);
    assert_eq!(report.used, DMA_BASE_PA + layout.used_off as u64);
    assert_eq!(report.req_pa, DMA_BASE_PA + REQ_OFF as u64);
    assert_eq!(report.data_pa, DMA_BASE_PA + DATA_OFF as u64);
    assert_eq!(report.status_pa, DMA_BASE_PA + STATUS_OFF as u64);
}

#[test]
fn init_rejects_device_without_version1() {
    let events = new_events();
    let mut dev = FakeDev::new(Rc::clone(&events));
    // Thiết bị legacy: **không** có bit 32 ⇒ driver phải từ chối, không chạy tiếp.
    dev.device_features = F_FLUSH;
    let mut dma = FakeDma::new(Rc::clone(&events));
    let blk = Blk::new();
    let transport = Transport::from_report(&pci_report());
    let mut bus = FakeBus { dev };
    assert_eq!(
        blk.init(&mut bus, &transport, &mut dma),
        Err("version1"),
        "thiếu VIRTIO_F_VERSION_1 phải bị từ chối"
    );
}

#[test]
fn init_rejects_device_that_drops_features_ok() {
    let events = new_events();
    let mut dev = FakeDev::new(Rc::clone(&events));
    dev.reject_features = true;
    let mut dma = FakeDma::new(Rc::clone(&events));
    let blk = Blk::new();
    let transport = Transport::from_report(&pci_report());
    let mut bus = FakeBus { dev };
    assert_eq!(
        blk.init(&mut bus, &transport, &mut dma),
        Err("features_ok"),
        "thiết bị bỏ FEATURES_OK phải làm init fail-closed"
    );
}

#[test]
fn init_rejects_torn_config_generation() {
    let events = new_events();
    let mut dev = FakeDev::new(Rc::clone(&events));
    dev.torn_generation = true;
    let mut dma = FakeDma::new(Rc::clone(&events));
    let blk = Blk::new();
    let transport = Transport::from_report(&pci_report());
    let mut bus = FakeBus { dev };
    assert_eq!(
        blk.init(&mut bus, &transport, &mut dma),
        Err("config_generation"),
        "config_generation đổi giữa hai lần đọc ⇒ không dùng capacity xé"
    );
}

#[test]
fn init_rejects_queue_too_small_for_three_descriptor_chain() {
    let events = new_events();
    let mut dev = FakeDev::new(Rc::clone(&events));
    dev.queue_size = 2; // chain header→data→status cần 3
    let mut dma = FakeDma::new(Rc::clone(&events));
    let blk = Blk::new();
    let transport = Transport::from_report(&pci_report());
    let mut bus = FakeBus { dev };
    assert_eq!(
        blk.init(&mut bus, &transport, &mut dma),
        Err("queue_size"),
        "queue_size = 2 từng làm set_desc(2) bị bỏ im lặng (D2-110)"
    );
}

// ---------------------------------------------------------------- 2. cache hook trước notify

#[test]
fn submit_syncs_for_device_before_notify() {
    let events = new_events();
    let mut device = valid_device(&events);
    let mut out = [0u8; SECTOR_BYTES];
    device
        .blk
        .read_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            0,
            &mut out,
        )
        .expect("đọc sector 0");
    assert_eq!(
        device.dma.device_syncs, 1,
        "sync_for_device phải chạy đúng một lần/request"
    );
    assert_eq!(
        device.bus.dev.notifies, 1,
        "notify phải chạy đúng một lần/request"
    );
    assert_eq!(device.bus.dev.last_notify_queue, 0);
    let (sync, notify) = event_order(&events);
    assert!(
        sync < notify,
        "sync_for_device phải chạy TRƯỚC notify (sync={sync}, notify={notify})"
    );
    // Cache sau khi thiết bị báo xong: ít nhất một lời gọi toàn trang.
    assert!(
        device.dma.cpu_full >= 1,
        "sync_for_cpu(toàn trang) phải chạy sau hoàn tất"
    );
    assert!(
        device.dma.cpu_polls >= 1,
        "phải có ít nhất một lần bỏ cache dòng used->idx"
    );
}

// ---------------------------------------------------------------- 3. descriptor vòng + dữ liệu

#[test]
fn read_sector_roundtrip_through_ring() {
    let events = new_events();
    let mut device = valid_device(&events);
    let mut out = [0u8; SECTOR_BYTES];
    device
        .blk
        .read_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            3,
            &mut out,
        )
        .expect("đọc sector 3");
    assert_eq!(device.dma.last_kind, Some(T_IN));
    assert_eq!(device.dma.last_sector, Some(3));
    assert_eq!(device.dma.last_desc_len, Some(SECTOR_BYTES as u32));
    for (i, byte) in out.iter().enumerate() {
        assert_eq!(*byte, data_pattern(3, i), "byte {i} của sector 3");
    }
    let stats = device.blk.stats();
    assert_eq!(stats.requests, 1);
    assert_eq!(stats.reads, 1);
    assert_eq!(stats.completions, 1);
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.id_mismatch, 0);
    assert_eq!(stats.timeouts, 0);
}

#[test]
fn write_sector_sends_header_and_payload() {
    let events = new_events();
    let mut device = valid_device(&events);
    let mut data = [0u8; SECTOR_BYTES];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    device
        .blk
        .write_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            7,
            &data,
        )
        .expect("ghi sector 7");
    assert_eq!(device.dma.last_kind, Some(T_OUT));
    assert_eq!(device.dma.last_sector, Some(7));
    assert_eq!(device.dma.last_write, Some(data));
    let stats = device.blk.stats();
    assert_eq!(stats.writes, 1);
    assert_eq!(stats.completions, 1);
    assert_eq!(stats.errors, 0);
}

#[test]
fn used_id_mismatch_is_an_error() {
    let events = new_events();
    let mut dma = FakeDma::new(Rc::clone(&events));
    dma.used_id = 1; // thiết bị trả head sai
    let mut device = boot(FakeDev::new(Rc::clone(&events)), dma);
    let mut out = [0u8; SECTOR_BYTES];
    assert_eq!(
        device.blk.read_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            0,
            &mut out
        ),
        Err(BlkError::Io)
    );
    let stats = device.blk.stats();
    assert_eq!(stats.id_mismatch, 1);
    assert_eq!(stats.errors, 1);
    assert_eq!(stats.completions, 0);
}

// ---------------------------------------------------------------- 4. flush + lỗi trạng thái

#[test]
fn flush_requires_negotiated_feature() {
    let events = new_events();
    let mut dev = FakeDev::new(Rc::clone(&events));
    dev.device_features = F_VERSION_1; // không có VIRTIO_BLK_F_FLUSH
    let mut device = boot(dev, FakeDma::new(Rc::clone(&events)));
    assert!(!device.report.flush_ok);
    assert_eq!(
        device
            .blk
            .flush(&mut device.bus, &mut device.dma, &mut device.clock),
        Err(BlkError::Unsupported),
        "không đàm phán FLUSH thì không được gửi T_FLUSH (spec §5.2.3)"
    );
    assert_eq!(device.blk.stats().flushes, 0);
    assert_eq!(device.bus.dev.notifies, 0);
    assert_eq!(device.dma.device_syncs, 0);
}

#[test]
fn flush_works_when_negotiated() {
    let events = new_events();
    let mut device = valid_device(&events);
    device
        .blk
        .flush(&mut device.bus, &mut device.dma, &mut device.clock)
        .expect("FLUSH đã đàm phán");
    assert_eq!(device.dma.last_kind, Some(T_FLUSH));
    assert_eq!(device.blk.stats().flushes, 1);
    assert_eq!(device.blk.stats().completions, 1);
}

#[test]
fn out_of_range_does_not_submit() {
    let events = new_events();
    let mut device = valid_device(&events);
    let mut out = [0u8; SECTOR_BYTES];
    assert_eq!(
        device.blk.read_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            CAPACITY_SECTORS as u32,
            &mut out
        ),
        Err(BlkError::OutOfRange)
    );
    let stats = device.blk.stats();
    assert_eq!(
        stats.requests, 0,
        "kiểm dải phải chạy TRƯỚC khi phát request"
    );
    assert_eq!(stats.completions, 0);
    assert_eq!(device.bus.dev.notifies, 0);
}

#[test]
fn poll_times_out_at_deadline() {
    let events = new_events();
    let mut dma = FakeDma::new(Rc::clone(&events));
    dma.simulate = false; // thiết bị không bao giờ hoàn tất
    let mut device = boot(FakeDev::new(Rc::clone(&events)), dma);
    device.clock = TestClock::new(5, Some(10), 0);
    let mut out = [0u8; SECTOR_BYTES];
    assert_eq!(
        device.blk.read_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            0,
            &mut out
        ),
        Err(BlkError::Timeout)
    );
    let stats = device.blk.stats();
    assert_eq!(stats.timeouts, 1);
    assert_eq!(stats.errors, 1);
}

#[test]
fn poll_falls_back_to_spin_limit_without_clock() {
    let events = new_events();
    let mut dma = FakeDma::new(Rc::clone(&events));
    dma.simulate = false;
    let mut device = boot(FakeDev::new(Rc::clone(&events)), dma);
    device.clock = TestClock::new(0, None, 3);
    let mut out = [0u8; SECTOR_BYTES];
    assert_eq!(
        device.blk.read_sector(
            &mut device.bus,
            &mut device.dma,
            &mut device.clock,
            0,
            &mut out
        ),
        Err(BlkError::Timeout),
        "không có trần thời gian ⇒ trần số vòng"
    );
    assert_eq!(device.blk.stats().timeouts, 1);
}

#[test]
fn device_status_and_config_generation_read_live() {
    let events = new_events();
    let mut device = valid_device(&events);
    device.bus.dev.status = 0x0f;
    assert_eq!(device.blk.device_status(&mut device.bus), 0x0f);
    assert_eq!(device.blk.config_generation(&mut device.bus), 3);
    assert_eq!(device.blk.capacity_sectors(), CAPACITY_SECTORS);
    assert!(device.blk.ready());
    assert_eq!(device.blk.dma_pa(), DMA_BASE_PA);
}

// ---------------------------------------------------------------- 5. DMA pool: cấp/thu + đọc RAM

#[test]
fn dma_arena_allocates_contiguous_aligned_and_frees_lifo() {
    let mut arena = DmaArena::new(0x2000);
    let first = arena.alloc(0x8000_0000, 16, 16).expect("cấp 16 B");
    assert_eq!(first.off, 0);
    assert_eq!(first.pa, 0x8000_0000);
    let second = arena
        .alloc(0x8000_0000, 4096, 4096)
        .expect("cấp 4 KiB căn 4 KiB");
    assert_eq!(second.off, 4096, "vùng kế phải căn lên 4 KiB");
    assert_eq!(second.pa, 0x8000_1000);
    assert!(!arena.free(first), "chỉ thu hồi được vùng đỉnh (LIFO)");
    assert!(arena.free(second));
    // Vùng `first` nằm dưới `second` và không còn là đỉnh (cursor = 4096) ⇒ chưa thu được;
    // `reset` mới là đường thu toàn bộ của arena bump.
    assert!(!arena.free(first));
    assert_eq!(arena.used(), 4096);
    arena.reset();
    assert_eq!(arena.used(), 0);
    assert!(arena.alloc(0x8000_0000, 4096, 512).is_some());
    assert!(
        arena.alloc(0x8000_0000, 4096, 3).is_none(),
        "align không luỹ thừa 2"
    );
}

// ---------------------------------------------------------------- 6. bằng chứng sector 0 (magic)

#[test]
fn sector0_magic_requires_exact_prefix() {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(b"VIFASTOS");
    bytes[8..16].copy_from_slice(b"-KT334-S");
    assert!(has_magic(&bytes, &SECTOR0_MAGIC));
    let evidence = sector0_evidence(&bytes, &SECTOR0_MAGIC);
    assert!(evidence.magic);
    assert_eq!(&evidence.hex[..16], b"5649464153544f53");
    assert_eq!(evidence.ascii, Some(*b"VIFASTOS-KT334-S"));
    // Lệch **một** byte ⇒ không phải chữ ký (không "gần đúng").
    bytes[7] ^= 0x01;
    assert!(!has_magic(&bytes, &SECTOR0_MAGIC));
    assert!(!sector0_evidence(&bytes, &SECTOR0_MAGIC).magic);
    // Lát ngắn hơn chữ ký ⇒ false, không đọc ngoài.
    assert!(!has_magic(&bytes[..4], &SECTOR0_MAGIC));
}

#[test]
fn hex_and_ascii_evidence_are_exact() {
    let mut bytes = [0u8; 16];
    bytes[0] = 0xde;
    bytes[1] = 0xad;
    bytes[15] = 0x01;
    let hex = hex16(&bytes);
    assert_eq!(&hex[0..4], b"dead");
    assert_eq!(&hex[30..32], b"01");
    assert_eq!(
        ascii16(&bytes),
        None,
        "byte 0xde không in được ⇒ không bịa '.'"
    );
    let text = *b"VIFASTOS-KT334-S";
    assert_eq!(ascii16(&text), Some(text));
}
