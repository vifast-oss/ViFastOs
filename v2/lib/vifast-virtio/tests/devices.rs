// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test cho **driver `virtio-gpu` + `virtio-input` dùng chung 3 arch** (KT-337).
//!
//! ```text
//! cargo test --manifest-path v2/Cargo.toml -p vifast-virtio --test devices
//! ```
//!
//! # Vì sao test này tồn tại
//!
//! Trước KT-337, `virtio_gpu.rs`/`virtio_input.rs` của riscv64 và aarch64 là **hai bản sao** của
//! cùng một driver (status/feature/queue/descriptor/notify/poll/config), chỉ khác cách chạm phần
//! cứng. KT-337 rút quyết định về [`vifast_virtio::gpudev`]/[`vifast_virtio::inputdev`] — crate
//! thuần, test host được (D2-05). Thiết bị giả dưới đây **không** phải QEMU: nó mô phỏng đúng
//! phần giao thức mà driver đọc/ghi, nên một lệch thứ tự hiện ra ngay thay vì chờ một lần soak.
//!
//! # Bốn bất biến được chốt
//!
//! 1. **thứ tự** `sync_for_device` → `fence` → `notify` (aarch64 `dc civac`/`dsb` — KT-277): bỏ
//!    lời gọi sync ⇒ `gpu_submit_syncs_for_device_before_notify` ĐỎ (đột biến (ii) của KT-337);
//! 2. **offset** của `TRANSFER_TO_HOST_2D` = `Rect::byte_offset` (KT-271): đặt `offset = 0` ⇒
//!    `gpu_transfer_uses_rect_byte_offset` ĐỎ (đột biến (i) của KT-337);
//! 3. `RESOURCE_ATTACH_BACKING` gửi **19** mảnh, mảnh cuối **ngắn hơn** (không tràn backing);
//! 4. `virtio-input`: đọc tên/devids/EV_BITS từ **thiết bị**, arm **8** đệm trước notify, nhận
//!    đúng một `EV_KEY KEY_A 1`, và arm lại descriptor **trước** khi trả sự kiện.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use vifast_virtio::blkdev::Clock;
use vifast_virtio::dma::{DmaArena, DmaBuf, DmaPool};
use vifast_virtio::gpu::{
    Rect, ResourceAttachBacking, SetScanout, TransferToHost2d, CTRL_HDR_BYTES,
    RESP_DISPLAY_INFO_BYTES, VIRTIO_GPU_CMD_GET_DISPLAY_INFO,
    VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING, VIRTIO_GPU_CMD_SET_SCANOUT,
    VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D, VIRTIO_GPU_RESP_OK_DISPLAY_INFO, VIRTIO_GPU_RESP_OK_NODATA,
};
use vifast_virtio::gpudev::{Gpu, DMA_PAGE_BYTES, MAX_ATTACH_ENTRIES, RESOURCE_ID, SCANOUT_ID};
use vifast_virtio::input::{InputEvent, EV_KEY, KEY_A};
use vifast_virtio::input::{EVENT_BUFS, EVENT_BYTES};
use vifast_virtio::inputdev::Input;
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
const FB_PA: u64 = 0x8100_0000;
const FB_BYTES: usize = 640 * 480 * 4;

// Thanh ghi common config (spec virtio 1.0 §4.1.4.3) — chép số **có chủ đích**: test phải độc
// lập với hằng private của driver.
const C_DEVICE_FEATURE_SELECT: u64 = 0x00;
const C_DEVICE_FEATURE: u64 = 0x04;
const C_DRIVER_FEATURE_SELECT: u64 = 0x08;
const C_DRIVER_FEATURE: u64 = 0x0c;
const C_NUM_QUEUES: u64 = 0x12;
const C_DEVICE_STATUS: u64 = 0x14;
const C_QUEUE_SELECT: u64 = 0x16;
const C_QUEUE_SIZE: u64 = 0x18;
const C_QUEUE_ENABLE: u64 = 0x1c;
const C_QUEUE_NOTIFY_OFF: u64 = 0x1e;

const F_VERSION_1: u64 = 1 << 32;

fn cap(cap_type: CapType, offset: u32) -> VirtioCap {
    VirtioCap {
        cap_type,
        bar: 4,
        offset,
        length: 0x1000,
    }
}

fn pci_report(device: u16) -> PciReport {
    PciReport {
        access: 1,
        ecam_base: 0,
        functions_seen: 2,
        bus: 0,
        dev: 1,
        func: 0,
        vendor: 0x1af4,
        device,
        class: 0x0003_0000,
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

// ---------------------------------------------------------------- thiết bị giả

type Events = Rc<RefCell<Vec<&'static str>>>;

/// Trạng thái chung của thiết bị giả (MMIO + vùng DMA nhìn **cùng** một thế giới).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Gpu,
    Input,
}

struct World {
    kind: Kind,
    events: Events,
    // common config
    status: u8,
    device_feature_select: u32,
    driver_feature_select: u32,
    driver_features: u64,
    device_features: u64,
    queue_select: u16,
    queue_size: u16,
    queue_enable: bool,
    queue_notify_off: u16,
    num_queues: u16,
    // device config (virtio-input)
    config_select: u8,
    config_subsel: u8,
    name: Vec<u8>,
    // "thiết bị" ghi vào DMA
    simulate: bool,
    used_id: u32,
    last_avail: u16,
    pending_events: VecDeque<InputEvent>,
    last_cmd: Option<u32>,
    last_transfer_offset: Option<u64>,
    last_entries: Vec<(u64, u32)>,
    display_queries: u32,
}

impl World {
    fn new(events: Events, device_features: u64, kind: Kind) -> Self {
        let name = b"QEMU Virtio Keyboard\0".to_vec();
        Self {
            kind,
            events,
            status: 0,
            device_feature_select: 0,
            driver_feature_select: 0,
            driver_features: 0,
            device_features,
            queue_select: 0,
            queue_size: 16,
            queue_enable: false,
            queue_notify_off: NOTIFY_OFF,
            num_queues: 2,
            config_select: 0,
            config_subsel: 0,
            name,
            simulate: true,
            used_id: 0,
            last_avail: 0,
            pending_events: VecDeque::new(),
            last_cmd: None,
            last_transfer_offset: None,
            last_entries: Vec::new(),
            display_queries: 0,
        }
    }

    /// View `virtio_input_config` đang chọn: (size, payload).
    fn input_view(&self) -> (u8, Vec<u8>) {
        match (self.config_select, self.config_subsel) {
            (vifast_virtio::input::CFG_ID_NAME, _) => (self.name.len() as u8, self.name.clone()),
            (vifast_virtio::input::CFG_ID_DEVIDS, _) => {
                let mut raw = Vec::new();
                for value in [6u16, 0x627, 1, 1] {
                    raw.extend_from_slice(&value.to_le_bytes());
                }
                (8, raw)
            }
            (vifast_virtio::input::CFG_EV_BITS, 1) => {
                let mut raw = vec![0u8; 128];
                raw[KEY_A as usize / 8] |= 1 << (KEY_A % 8);
                (128, raw)
            }
            _ => (0, Vec::new()),
        }
    }
}

struct FakeBus {
    world: Rc<RefCell<World>>,
}

impl MmioBus for FakeBus {
    fn read8(&self, addr: u64) -> u8 {
        let world = self.world.borrow();
        if (DEVICE_CFG..DEVICE_CFG + 136).contains(&addr) {
            let offset = (addr - DEVICE_CFG) as usize;
            if offset < 8 {
                let (size, _) = world.input_view();
                return match offset {
                    0 => world.config_select,
                    1 => world.config_subsel,
                    2 => size,
                    _ => 0,
                };
            }
            let (_, payload) = world.input_view();
            return payload.get(offset - 8).copied().unwrap_or(0);
        }
        match addr - COMMON {
            C_DEVICE_STATUS => world.status,
            C_QUEUE_SELECT => world.queue_select as u8,
            _ => 0,
        }
    }
    fn read16(&self, addr: u64) -> u16 {
        let world = self.world.borrow();
        match addr - COMMON {
            C_NUM_QUEUES => world.num_queues,
            C_QUEUE_SIZE => world.queue_size,
            C_QUEUE_SELECT => world.queue_select,
            C_QUEUE_ENABLE => world.queue_enable as u16,
            C_QUEUE_NOTIFY_OFF => world.queue_notify_off,
            C_DEVICE_STATUS => world.status as u16,
            _ => 0,
        }
    }
    fn read32(&self, addr: u64) -> u32 {
        let world = self.world.borrow();
        match addr - COMMON {
            C_DEVICE_FEATURE => {
                if world.device_feature_select == 0 {
                    (world.device_features & 0xffff_ffff) as u32
                } else {
                    (world.device_features >> 32) as u32
                }
            }
            C_DRIVER_FEATURE => {
                if world.driver_feature_select == 0 {
                    (world.driver_features & 0xffff_ffff) as u32
                } else {
                    (world.driver_features >> 32) as u32
                }
            }
            _ => 0,
        }
    }
    fn write8(&mut self, addr: u64, value: u8) {
        let mut world = self.world.borrow_mut();
        if (DEVICE_CFG..DEVICE_CFG + 8).contains(&addr) {
            match addr - DEVICE_CFG {
                0 => world.config_select = value,
                1 => world.config_subsel = value,
                _ => {}
            }
            return;
        }
        if addr == COMMON + C_DEVICE_STATUS {
            world.status = value;
        }
    }
    fn write16(&mut self, addr: u64, value: u16) {
        let mut world = self.world.borrow_mut();
        if addr == NOTIFY_ADDR {
            world.events.borrow_mut().push("notify");
            return;
        }
        match addr - COMMON {
            C_QUEUE_SELECT => world.queue_select = value,
            C_QUEUE_SIZE => world.queue_size = value,
            C_QUEUE_ENABLE => world.queue_enable = value == 1,
            _ => {}
        }
    }
    fn write32(&mut self, addr: u64, value: u32) {
        let mut world = self.world.borrow_mut();
        match addr - COMMON {
            C_DEVICE_FEATURE_SELECT => world.device_feature_select = value,
            C_DRIVER_FEATURE_SELECT => world.driver_feature_select = value,
            C_DRIVER_FEATURE => {
                let mask = 0xffff_ffffu64;
                if world.driver_feature_select == 0 {
                    world.driver_features = (world.driver_features & !mask) | value as u64;
                } else {
                    world.driver_features = (world.driver_features & mask) | ((value as u64) << 32);
                }
            }
            _ => {}
        }
    }
    fn write64(&mut self, _addr: u64, _value: u64) {}
}

/// Thiết bị giả + **vùng DMA của nó**: `mem` thuộc `FakeDma` (không `UnsafeCell` — R5 cấm
/// `unsafe` ngoài `kernel/vifast-plat-*`), còn thanh ghi thuộc [`World`] chia sẻ qua `Rc`.
struct FakeDma {
    world: Rc<RefCell<World>>,
    mem: Vec<u8>,
    arena: DmaArena,
}

impl FakeDma {
    fn new(world: Rc<RefCell<World>>) -> Self {
        Self {
            world,
            mem: vec![0u8; DMA_PAGE_BYTES],
            arena: DmaArena::new(DMA_PAGE_BYTES),
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

    /// Thiết bị đọc chuỗi descriptor vừa publish rồi ghi `used` — §2.7.6.
    fn simulate_device(&mut self) {
        let (queue_size, simulate, kind) = {
            let world = self.world.borrow();
            (world.queue_size, world.simulate, world.kind)
        };
        if !simulate || kind != Kind::Gpu {
            return;
        }
        let layout = VqLayout::new(queue_size).expect("layout");
        let avail_idx = self.u16_at(layout.avail_idx_off);
        let last_avail = self.world.borrow().last_avail;
        if avail_idx == last_avail {
            return;
        }
        self.world.borrow_mut().last_avail = avail_idx;
        let slot = (avail_idx.wrapping_sub(1)) & (queue_size - 1);
        let head = self.u16_at(layout.avail_ring_off + slot as usize * 2);
        // Descriptor 0: command (device-readable); descriptor 1: response buffer.
        let at = layout.desc_off + head as usize * 16;
        let cmd_pa = self.u64_at(at);
        let cmd_off = (cmd_pa - DMA_BASE_PA) as usize;
        let kind = self.u32_at(cmd_off);
        let next = self.u16_at(at + 14) as usize;
        let resp_at = layout.desc_off + next * 16;
        let resp_pa = self.u64_at(resp_at);
        let resp_len = self.u32_at(resp_at + 8) as usize;
        let resp_off = (resp_pa - DMA_BASE_PA) as usize;
        self.world.borrow_mut().last_cmd = Some(kind);
        self.world.borrow().events.borrow_mut().push("device_cmd");
        match kind {
            VIRTIO_GPU_CMD_GET_DISPLAY_INFO => {
                self.world.borrow_mut().display_queries += 1;
                for byte in &mut self.mem[resp_off..resp_off + RESP_DISPLAY_INFO_BYTES] {
                    *byte = 0;
                }
                self.set_u32(resp_off, VIRTIO_GPU_RESP_OK_DISPLAY_INFO);
                self.set_u32(resp_off + CTRL_HDR_BYTES + 8, 1280);
                self.set_u32(resp_off + CTRL_HDR_BYTES + 12, 800);
                self.set_u32(resp_off + CTRL_HDR_BYTES + 16, 1);
            }
            VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D => {
                let command = TransferToHost2d::parse(&self.mem[cmd_off..cmd_off + 56])
                    .expect("TRANSFER_TO_HOST_2D");
                self.world.borrow_mut().last_transfer_offset = Some(command.offset);
                self.set_u32(resp_off, VIRTIO_GPU_RESP_OK_NODATA);
            }
            VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING => {
                let command =
                    ResourceAttachBacking::parse(&self.mem[cmd_off..cmd_off + 32]).expect("attach");
                let mut entries = Vec::new();
                for index in 0..command.nr_entries as usize {
                    let at = cmd_off + 32 + index * 16;
                    let addr =
                        u64::from_le_bytes(self.mem[at..at + 8].try_into().expect("addr 8 byte"));
                    let len = u32::from_le_bytes(
                        self.mem[at + 8..at + 12].try_into().expect("len 4 byte"),
                    );
                    entries.push((addr, len));
                }
                self.world.borrow_mut().last_entries = entries;
                self.set_u32(resp_off, VIRTIO_GPU_RESP_OK_NODATA);
            }
            VIRTIO_GPU_CMD_SET_SCANOUT => {
                let command = SetScanout::parse(&self.mem[cmd_off..cmd_off + 48]).expect("scanout");
                // Ca âm đối chứng: resource 999 **không** tồn tại ⇒ thiết bị từ chối.
                if command.resource_id == 999 {
                    self.set_u32(
                        resp_off,
                        vifast_virtio::gpu::VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID,
                    );
                } else {
                    self.set_u32(resp_off, VIRTIO_GPU_RESP_OK_NODATA);
                }
            }
            _ => self.set_u32(resp_off, VIRTIO_GPU_RESP_OK_NODATA),
        }
        let used_len = if kind == VIRTIO_GPU_CMD_GET_DISPLAY_INFO {
            RESP_DISPLAY_INFO_BYTES
        } else {
            CTRL_HDR_BYTES
        };
        let used_idx = self.u16_at(layout.used_idx_off);
        let uslot = (used_idx & (queue_size - 1)) as usize;
        let at = layout.used_ring_off + uslot * 8;
        let used_id = self.world.borrow().used_id;
        self.set_u32(at, used_id);
        self.set_u32(at + 4, used_len.min(resp_len) as u32);
        self.set_u16(layout.used_idx_off, used_idx.wrapping_add(1));
    }

    /// Thiết bị input giao một sự kiện đang chờ vào đệm `used_idx` (async, thấy ở vòng poll).
    fn deliver_pending_event(&mut self) {
        let (queue_size, event) = {
            let mut world = self.world.borrow_mut();
            let Some(event) = world.pending_events.pop_front() else {
                return;
            };
            (world.queue_size, event)
        };
        let layout = VqLayout::new(queue_size).expect("layout");
        let used_idx = self.u16_at(layout.used_idx_off);
        let index = used_idx & (queue_size - 1);
        let buf = vifast_virtio::inputdev::BUF_OFF + index as usize * EVENT_BYTES;
        self.mem[buf..buf + EVENT_BYTES].copy_from_slice(&event.encode());
        let at = layout.used_ring_off + (used_idx & (queue_size - 1)) as usize * 8;
        self.set_u32(at, index as u32);
        self.set_u32(at + 4, EVENT_BYTES as u32);
        self.set_u16(layout.used_idx_off, used_idx.wrapping_add(1));
        self.world.borrow().events.borrow_mut().push("device_event");
    }
}

impl DmaPool for FakeDma {
    fn base_pa(&self) -> u64 {
        DMA_BASE_PA
    }
    fn capacity(&self) -> usize {
        DMA_PAGE_BYTES
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
        let (simulate, kind) = {
            let world = self.world.borrow();
            (world.simulate, world.kind)
        };
        self.world.borrow().events.borrow_mut().push("sync_device");
        if simulate && kind == Kind::Gpu {
            self.simulate_device();
        }
    }
    fn sync_for_cpu(&mut self, _off: usize, len: usize) {
        if len <= 8 {
            let (simulate, pending) = {
                let world = self.world.borrow();
                (world.simulate, !world.pending_events.is_empty())
            };
            self.world.borrow().events.borrow_mut().push("sync_cpu");
            if simulate && pending {
                self.deliver_pending_event();
            }
        }
    }
}

// ---------------------------------------------------------------- đồng hồ giả

struct TestClock {
    now: u64,
    step: u64,
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

struct Rig {
    world: Rc<RefCell<World>>,
    bus: FakeBus,
    dma: FakeDma,
    clock: TestClock,
}

fn rig_for(kind: Kind, device_features: u64) -> Rig {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let world = Rc::new(RefCell::new(World::new(events, device_features, kind)));
    Rig {
        bus: FakeBus {
            world: Rc::clone(&world),
        },
        dma: FakeDma::new(Rc::clone(&world)),
        clock: TestClock::new(1, Some(1_000_000), 0),
        world,
    }
}

fn rig(device_features: u64) -> Rig {
    rig_for(Kind::Gpu, device_features)
}

fn rig_input(device_features: u64) -> Rig {
    rig_for(Kind::Input, device_features)
}

fn boot_gpu(rig: &mut Rig) -> vifast_virtio::gpudev::GpuReport {
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init gpu phải thành công")
}

fn boot_input(rig: &mut Rig) -> (Input, vifast_virtio::inputdev::InputReport) {
    let input = Input::new();
    let report = input
        .init(&mut rig.bus, &mut rig.dma, &pci_report(0x1052), true)
        .expect("init input phải thành công");
    (input, report)
}

fn order(events: &Events, first: &'static str, second: &'static str) -> (usize, usize) {
    let list = events.borrow();
    (
        list.iter().position(|e| *e == first).unwrap_or(usize::MAX),
        list.iter().position(|e| *e == second).unwrap_or(usize::MAX),
    )
}

// ---------------------------------------------------------------- GPU

#[test]
fn gpu_init_negotiates_version1_and_reports_layout() {
    let mut rig = rig(F_VERSION_1 | (1 << 1) | (1 << 3));
    let report = boot_gpu(&mut rig);
    assert_eq!(report.vendor, 0x1af4);
    assert_eq!(report.device, 0x1050);
    assert_eq!(report.queue_max, 16);
    assert_eq!(report.queue_size, vifast_virtio::gpudev::QUEUE_SIZE);
    assert_eq!(report.num_queues, 2);
    assert_eq!(report.status & 0x0f, 0x0f);
    assert_eq!(report.device_features & F_VERSION_1, F_VERSION_1);
    assert_eq!(report.driver_features, 1u64 << 32);
    assert_eq!(report.common, COMMON);
    assert_eq!(report.notify, NOTIFY);
    assert_eq!(report.device_cfg, DEVICE_CFG);
    assert_eq!(report.isr, ISR);
    assert_eq!(report.notify_addr, NOTIFY_ADDR);
    assert_eq!(report.dma_pa, DMA_BASE_PA);
    assert_eq!(report.fb_pa, FB_PA);
    assert_eq!(report.fb_bytes, FB_BYTES);
    assert_eq!(report.fb_pages, FB_BYTES / 4096);
    assert_eq!(report.fb_stride, 640 * 4);
    let layout = VqLayout::new(vifast_virtio::gpudev::QUEUE_SIZE).expect("layout");
    assert_eq!(report.desc, DMA_BASE_PA + layout.desc_off as u64);
    assert_eq!(report.avail, DMA_BASE_PA + layout.avail_off as u64);
    assert_eq!(report.used, DMA_BASE_PA + layout.used_off as u64);
    assert_eq!(report.cmd_pa, DMA_BASE_PA + 0x200);
    assert_eq!(report.resp_pa, DMA_BASE_PA + 0x600);
}

#[test]
fn gpu_display_info_parses_device_mode() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    let info = gpu
        .get_display_info(&mut rig.bus, &mut rig.dma, &mut rig.clock)
        .expect("display info");
    assert_eq!(info.enabled_count(), 1);
    let (index, rect) = info.first_enabled().expect("scanout 0 bật");
    assert_eq!(index, 0);
    assert_eq!((rect.width, rect.height), (1280, 800));
    assert_eq!(rig.world.borrow().display_queries, 1);
}

#[test]
fn gpu_attach_backing_uses_19_entries_and_short_last() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    gpu.create_resource(&mut rig.bus, &mut rig.dma, &mut rig.clock, 640, 480)
        .expect("RESOURCE_CREATE_2D");
    let entries = gpu
        .attach_backing(&mut rig.bus, &mut rig.dma, &mut rig.clock)
        .expect("ATTACH_BACKING");
    assert_eq!(entries as usize, MAX_ATTACH_ENTRIES);
    let world = rig.world.borrow();
    assert_eq!(world.last_entries.len(), MAX_ATTACH_ENTRIES);
    let chunk = vifast_virtio::gpudev::ATTACH_CHUNK_BYTES;
    assert_eq!(world.last_entries[0], (FB_PA, chunk as u32));
    let (last_addr, last_len) = *world.last_entries.last().expect("mảnh cuối");
    assert_eq!(
        last_addr,
        FB_PA + (MAX_ATTACH_ENTRIES - 1) as u64 * chunk as u64
    );
    assert_eq!(
        last_len as usize,
        FB_BYTES - (MAX_ATTACH_ENTRIES - 1) * chunk
    );
    assert!(last_len as usize <= chunk);
    assert_eq!(world.last_cmd, Some(VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING));
}

#[test]
fn gpu_transfer_uses_rect_byte_offset() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    let rect = Rect::new(8, 8, 88, 24);
    gpu.transfer_to_host_2d(&mut rig.bus, &mut rig.dma, &mut rig.clock, rect)
        .expect("TRANSFER_TO_HOST_2D");
    let expected = rect.byte_offset(640 * 4);
    assert_eq!(rig.world.borrow().last_transfer_offset, Some(expected));
    assert_ne!(expected, 0, "rect lệch gốc phải có offset khác 0");
    assert_eq!(
        rig.world.borrow().last_cmd,
        Some(VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D)
    );
}

#[test]
fn gpu_submit_syncs_for_device_before_notify() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    rig.world.borrow_mut().events.borrow_mut().clear();
    gpu.create_resource(&mut rig.bus, &mut rig.dma, &mut rig.clock, 640, 480)
        .expect("create");
    let events = Rc::clone(&rig.world.borrow().events);
    let (sync, notify) = order(&events, "sync_device", "notify");
    assert!(sync < notify, "sync_for_device phải chạy TRƯỚC notify");
}

#[test]
fn gpu_used_id_mismatch_is_reported() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    rig.world.borrow_mut().used_id = 7;
    let error = gpu
        .create_resource(&mut rig.bus, &mut rig.dma, &mut rig.clock, 640, 480)
        .expect_err("used->id lệch phải là lỗi");
    assert_eq!(error, "used_id");
    assert_eq!(gpu.stats().id_mismatch, 1);
    assert_eq!(gpu.stats().errors, 1);
}

#[test]
fn gpu_deadline_timeout_stops_poll() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    rig.world.borrow_mut().simulate = false;
    rig.clock = TestClock::new(1, Some(4), 0);
    let error = gpu
        .create_resource(&mut rig.bus, &mut rig.dma, &mut rig.clock, 640, 480)
        .expect_err("hết hạn phải là timeout");
    assert_eq!(error, "timeout");
    assert_eq!(gpu.stats().timeouts, 1);
}

#[test]
fn gpu_probe_bogus_scanout_returns_device_error_name() {
    let mut rig = rig(F_VERSION_1);
    let gpu = Gpu::new();
    gpu.init(
        &mut rig.bus,
        &mut rig.dma,
        &mut rig.clock,
        &pci_report(0x1050),
        FB_PA,
    )
    .expect("init");
    let full = Rect::full(640, 480);
    gpu.set_scanout(&mut rig.bus, &mut rig.dma, &mut rig.clock, full)
        .expect("SET_SCANOUT");
    assert_eq!(
        rig.world.borrow().last_cmd,
        Some(VIRTIO_GPU_CMD_SET_SCANOUT)
    );
    let name = gpu
        .probe_bogus_scanout(&mut rig.bus, &mut rig.dma, &mut rig.clock, 999, full)
        .expect("probe");
    assert_eq!(name, "InvalidResourceId");
    assert_eq!(SCANOUT_ID, 0);
    assert_eq!(RESOURCE_ID, 1);
}

// ---------------------------------------------------------------- input

#[test]
fn input_init_reads_config_and_arms_all_buffers() {
    let mut rig = rig_input(F_VERSION_1);
    let (_input, report) = boot_input(&mut rig);
    assert_eq!(report.name(), "QEMU Virtio Keyboard");
    assert_eq!(report.name_len as usize, 20);
    assert_eq!(report.devids.bustype, 6);
    assert_eq!(report.devids.vendor, 0x627);
    assert_eq!(report.devids.product, 1);
    assert_eq!(report.devids.version, 1);
    assert!(report.key_a_supported);
    assert_eq!(report.num_queues, 2);
    assert_eq!(report.queue_size, vifast_virtio::input::QUEUE_SIZE);
    assert_eq!(report.status & 0x0f, 0x0f);
    assert_eq!(report.armed as usize, EVENT_BUFS);
    assert_eq!(report.dma_pa, DMA_BASE_PA);
    assert_eq!(
        report.buf_pa,
        DMA_BASE_PA + vifast_virtio::inputdev::BUF_OFF as u64
    );
    let layout = VqLayout::new(vifast_virtio::input::QUEUE_SIZE).expect("layout");
    assert_eq!(rig.dma.u16_at(layout.avail_idx_off) as usize, EVENT_BUFS);
}

#[test]
fn input_poll_delivers_key_event_and_rearms() {
    let mut rig = rig_input(F_VERSION_1);
    let (input, _report) = boot_input(&mut rig);
    rig.world
        .borrow_mut()
        .pending_events
        .push_back(InputEvent::new(EV_KEY, KEY_A, 1));
    let event = input
        .poll_event(&mut rig.bus, &mut rig.dma, &mut rig.clock)
        .expect("phải nhận EV_KEY KEY_A 1");
    assert_eq!(event.event_type, EV_KEY);
    assert_eq!(event.code, KEY_A);
    assert_eq!(event.value, 1);
    assert!(event.is_key_down());
    let stats = input.stats();
    assert_eq!(stats.events, 1);
    assert_eq!(stats.key_downs, 1);
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.id_mismatch, 0);
    assert!(input.published() as usize > EVENT_BUFS);
}

#[test]
fn input_submit_syncs_for_device_before_notify() {
    let mut rig = rig_input(F_VERSION_1);
    let events = Rc::clone(&rig.world.borrow().events);
    boot_input(&mut rig);
    let (sync, notify) = order(&events, "sync_device", "notify");
    assert!(sync < notify, "sync_for_device phải chạy TRƯỚC notify");
    assert_eq!(
        rig.world.borrow().last_cmd,
        None,
        "input không gửi lệnh controlq"
    );
}

#[test]
fn input_status_queue_is_idle() {
    let mut rig = rig_input(F_VERSION_1);
    let (input, report) = boot_input(&mut rig);
    assert_eq!(report.status_queue_size, vifast_virtio::input::QUEUE_SIZE);
    assert!(input.status_queue_idle(&mut rig.dma));
}
