// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test cho driver `virtio-net` **dùng chung 3 arch** (`vifast_virtio::netdev`) — KT-335.
//!
//! ```text
//! cargo test --manifest-path v2/Cargo.toml -p vifast-virtio --test netdev
//! ```
//!
//! # Vì sao test này tồn tại
//!
//! Trước KT-335, chuỗi `reset → ACK → DRIVER → feature → FEATURES_OK → hai queue → DRIVER_OK →
//! config` nằm **ba bản** trong `vifast-plat-{riscv64,aarch64,x86_64}/src/virtio_net.rs` và chỉ
//! chạy được trên QEMU. Gộp mà không có phép kiểm host thì mọi lệch thứ tự chỉ hiện ra ở một lần
//! boot dài — đúng lớp lỗi `v2/docs/k6-test-invariants.md` §1 (phép kiểm phụ thuộc môi trường).
//!
//! Test dựng **thiết bị giả trong bộ nhớ** và chốt:
//!
//! 1. đường thành công: mọi trường `NetReport` mà ảnh đang in lấy **từ thiết bị**, không hằng;
//! 2. từng chặng **fail-closed** giữ đúng **tên** chặng cũ (`version1`, `features_ok`,
//!    `queue_size`, `config_generation`, `mac`) — đổi tên là đổi dòng TAP;
//! 3. gọi trước `init` ⇒ `NotReady` (không PANIC);
//! 4. frame sai độ dài ⇒ `BadFrame` **trước** khi chạm ring (không đẩy descriptor rác);
//! 5. thiết bị **không** hoàn tất ⇒ `Timeout` + `stats.timeouts` tăng (fail-closed, không treo).
//!
//! Thiết bị giả **không** phải QEMU: nó chỉ mô phỏng đúng phần thanh ghi mà `netdev` đọc/ghi.

use std::cell::Cell;

use vifast_virtio::blkdev::Transport;
use vifast_virtio::dma::{DmaBuf, DmaPool};
use vifast_virtio::mmio::MmioBus;
use vifast_virtio::net::{
    NetError, F_MAC, F_VERSION_1, HDR_MODERN_BYTES, QUEUES_REQUIRED, QUEUE_SIZE,
};
use vifast_virtio::netdev::{Clock, Net, RX_BUF_OFF, RX_RING_OFF, TX_BUF_OFF};

// ---------------------------------------------------------------- địa chỉ mô phỏng

const BAR: u64 = 0x1000_0000;
const COMMON: u64 = BAR + 0x1000;
const NOTIFY: u64 = BAR + 0x2000;
const DEVICE_CFG: u64 = BAR + 0x3000;
const ISR: u64 = BAR + 0x4000;
const NOTIFY_MUL: u32 = 4;
const DMA_BASE_PA: u64 = 0x8000_0000;
const DMA_BYTES: usize = 4096;

// Thanh ghi common config (spec virtio 1.0 §4.1.4.3) — chép số ở đây là **có chủ đích**: test
// phải độc lập với hằng private của driver, nếu không nó chỉ kiểm chính nó.
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

/// MAC thiết bị giả báo — **từ thiết bị**, không hằng trong driver.
const DEVICE_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0xab, 0xcd, 0xef];

// ---------------------------------------------------------------- thiết bị giả

/// Thiết bị virtio-net giả: chỉ mô phỏng thanh ghi + `device config` mà driver đọc.
struct FakeDev {
    /// Feature thiết bị báo (hai trang 32 bit).
    device_features: u64,
    /// `driver_features` đã ghi (hai trang).
    driver_features: [u32; 2],
    /// Thanh ghi chọn trang feature đang đọc/ghi.
    select: u32,
    /// `device_status`.
    status: u8,
    /// `config_generation` trả về; `generation_tears` ⇒ lần đọc thứ hai khác lần đầu.
    generation: [u8; 2],
    generation_reads: Cell<u8>,
    /// Queue đang chọn + cỡ tối đa từng queue.
    queue_select: u16,
    queue_max: [u16; 2],
    queue_size: [u16; 2],
    /// `false` ⇒ thiết bị **từ chối** `FEATURES_OK` (spec §2.4.2 bước 5).
    accept_features_ok: bool,
    /// `true` ⇒ MAC trả về toàn 0 (thiết bị không cấp MAC thật).
    zero_mac: bool,
    /// `true` ⇒ `config_generation` đổi giữa hai lần đọc config (dữ liệu bị **xé**).
    generation_tears: bool,
    num_queues: u16,
    /// Ba địa chỉ ring thiết bị **nhận** từ driver, theo queue (bằng chứng bố cục trang DMA).
    queue_desc: [u64; 2],
    queue_driver: [u64; 2],
    queue_device: [u64; 2],
}

impl FakeDev {
    fn new() -> Self {
        Self {
            device_features: (1u64 << F_VERSION_1) | (1u64 << F_MAC),
            driver_features: [0; 2],
            select: 0,
            status: 0,
            generation: [7, 7],
            generation_reads: Cell::new(0),
            queue_select: 0,
            queue_max: [QUEUE_SIZE, QUEUE_SIZE],
            queue_size: [0; 2],
            accept_features_ok: true,
            zero_mac: false,
            generation_tears: false,
            num_queues: QUEUES_REQUIRED,
            queue_desc: [0; 2],
            queue_driver: [0; 2],
            queue_device: [0; 2],
        }
    }

    fn read8(&self, addr: u64) -> u8 {
        // `device config` nằm ở một capability **khác** (BAR + 0x3000), không phải common config.
        if addr >= DEVICE_CFG {
            return self.device_config_byte(addr - DEVICE_CFG);
        }
        let off = addr - COMMON;
        match off {
            C_DEVICE_STATUS => self.status,
            C_CONFIG_GENERATION => {
                let index = usize::from(self.generation_reads.get().min(1));
                self.generation_reads
                    .set(self.generation_reads.get().saturating_add(1));
                if self.generation_tears && index == 1 {
                    self.generation[1].wrapping_add(1)
                } else {
                    self.generation[index]
                }
            }
            _ => self.device_config_byte(off),
        }
    }

    fn read16(&self, addr: u64) -> u16 {
        match addr - COMMON {
            C_NUM_QUEUES => self.num_queues,
            C_QUEUE_SIZE => self.queue_max[self.queue_select as usize],
            C_QUEUE_ENABLE => 1,
            C_QUEUE_NOTIFY_OFF => self.queue_select,
            _ => 0,
        }
    }

    fn read32(&self, addr: u64) -> u32 {
        match addr - COMMON {
            C_DEVICE_FEATURE => {
                if self.select == 0 {
                    self.device_features as u32
                } else {
                    (self.device_features >> 32) as u32
                }
            }
            C_DRIVER_FEATURE => self.driver_features[self.select.min(1) as usize],
            _ => 0,
        }
    }

    fn write8(&mut self, addr: u64, value: u8) {
        if addr - COMMON == C_DEVICE_STATUS {
            // Viết 0 = reset (spec §2.4.2 bước 1); ghi `FEATURES_OK` có thể bị **từ chối**.
            if value & 0x08 != 0 && !self.accept_features_ok {
                self.status = value & !0x08;
            } else {
                self.status = value;
            }
        }
    }

    fn write16(&mut self, addr: u64, value: u16) {
        match addr - COMMON {
            C_QUEUE_SELECT => self.queue_select = value,
            C_QUEUE_SIZE => self.queue_size[self.queue_select as usize] = value,
            _ => {}
        }
    }

    fn write32(&mut self, addr: u64, value: u32) {
        match addr - COMMON {
            C_DEVICE_FEATURE_SELECT | C_DRIVER_FEATURE_SELECT => self.select = value,
            C_DRIVER_FEATURE => self.driver_features[self.select.min(1) as usize] = value,
            _ => {}
        }
    }

    fn write64(&mut self, addr: u64, value: u64) {
        let slot = self.queue_select as usize;
        match addr - COMMON {
            C_QUEUE_DESC => self.queue_desc[slot] = value,
            C_QUEUE_DRIVER => self.queue_driver[slot] = value,
            C_QUEUE_DEVICE => self.queue_device[slot] = value,
            _ => {}
        }
    }

    fn device_config_byte(&self, index: u64) -> u8 {
        if self.zero_mac && index < 6 {
            return 0;
        }
        match index {
            0..=5 => DEVICE_MAC[index as usize],
            6..=7 => 0,
            8..=9 => 1, // max_virtqueue_pairs (byte thấp)
            10 => 0xdc, // mtu = 1500 (0x05dc) little-endian
            11 => 0x05,
            _ => 0,
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
    fn fence(&self) {}
}

/// Pool DMA 4 KiB phẳng (đúng một vùng như adapter thật) + đọc 16 bit "volatile" từ RAM.
struct FakeDma {
    mem: Vec<u8>,
}

impl FakeDma {
    fn new() -> Self {
        Self {
            mem: vec![0u8; DMA_BYTES],
        }
    }
}

impl DmaPool for FakeDma {
    fn base_pa(&self) -> u64 {
        DMA_BASE_PA
    }
    fn capacity(&self) -> usize {
        DMA_BYTES
    }
    fn bytes(&mut self) -> &mut [u8] {
        &mut self.mem
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        if len == 0 || len > DMA_BYTES || align == 0 || !align.is_power_of_two() {
            return None;
        }
        Some(DmaBuf {
            off: 0,
            len,
            pa: DMA_BASE_PA,
        })
    }
    fn free(&mut self, _buf: DmaBuf) -> bool {
        true
    }
    fn read_u16(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.mem[off], self.mem[off + 1]])
    }
    fn read_u8(&self, off: usize) -> u8 {
        self.mem[off]
    }
    fn sync_for_device(&mut self, _off: usize, _len: usize) {}
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {}
}

/// Đồng hồ giả: mỗi lần đọc tiến 1 ms (µs) ⇒ vòng poll kết thúc sau `timeout / 1000` vòng.
struct StepClock {
    now: Cell<u64>,
}

impl Clock for StepClock {
    fn now(&mut self) -> u64 {
        let value = self.now.get();
        self.now.set(value + 1_000);
        value
    }
}

/// Transport trỏ vào thiết bị giả (cùng BAR, bốn capability).
fn transport() -> Transport {
    Transport {
        vendor: 0x1af4,
        device: 0x1000,
        bus: 0,
        dev: 6,
        func: 0,
        bar_index: 4,
        bar_base: BAR,
        bar_size: 0x4000,
        common: COMMON,
        notify: NOTIFY,
        device_cfg: DEVICE_CFG,
        isr: ISR,
        notify_mul: NOTIFY_MUL,
    }
}

// ================================================================ 1. đường thành công

#[test]
fn init_reads_everything_from_device() {
    let mut dev = FakeDev::new();
    dev.device_features |= 1u64 << F_VERSION_1;
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    let report = net
        .init(&mut bus, &transport(), &mut dma)
        .expect("init xong");

    // Cỡ queue driver **đặt** = min(QUEUE_SIZE, max của thiết bị) và **đọc lại** được.
    assert_eq!(report.queue_size_rx, QUEUE_SIZE);
    assert_eq!(report.queue_size_tx, QUEUE_SIZE);
    assert_eq!(report.num_queues, QUEUES_REQUIRED);
    // MAC lấy **từ `device config`**, không hằng trong driver.
    assert_eq!(report.config.mac, DEVICE_MAC);
    assert_eq!(net.mac(), DEVICE_MAC);
    // Cửa thông báo = notify + queue_notify_off × multiplier (spec §4.1.4.5).
    assert_eq!(report.notify_addr_rx, NOTIFY); // queue_notify_off = 0
    assert_eq!(report.notify_addr_tx, NOTIFY + NOTIFY_MUL as u64); // queue_notify_off = 1
                                                                   // Địa chỉ ring/đệm suy từ **PA của pool**, không tự đoán.
    assert_eq!(report.dma_pa, DMA_BASE_PA);
    assert_eq!(report.desc_rx, DMA_BASE_PA + RX_RING_OFF as u64);
    assert_eq!(report.rx_buf_pa, DMA_BASE_PA + RX_BUF_OFF as u64);
    assert_eq!(report.tx_buf_pa, DMA_BASE_PA + TX_BUF_OFF as u64);
    // Feature đã đàm phán: VERSION_1 + MAC, **không** bit nào khác.
    assert_eq!(
        report.driver_features,
        (1u64 << F_VERSION_1) | (1u64 << F_MAC)
    );
    assert!(report.mac_ok);
    assert!(net.ready());
    // Ba địa chỉ ring thiết bị **nhận** phải khớp báo cáo ⇒ bố cục trang DMA (rx ring → tx ring →
    // đệm) không bị xê dịch khi gộp ba bản driver (TAP của ba arch phụ thuộc bố cục này).
    assert_eq!(bus.dev.queue_desc[0], report.desc_rx);
    assert_eq!(bus.dev.queue_driver[0], report.avail_rx);
    assert_eq!(bus.dev.queue_device[0], report.used_rx);
    assert_eq!(bus.dev.queue_desc[1], report.desc_tx);
    assert_eq!(bus.dev.queue_driver[1], report.avail_tx);
    assert_eq!(bus.dev.queue_device[1], report.used_tx);
    // `rx` đã được arm ngay trong init ⇒ đã có đúng **một** notify.
    assert_eq!(net.stats().notifies, 1);
}

// ================================================================ 2. thiếu VERSION_1

#[test]
fn missing_version1_is_rejected() {
    let mut dev = FakeDev::new();
    dev.device_features = 1u64 << F_MAC; // thiếu VIRTIO_F_VERSION_1 (bắt buộc)
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    assert_eq!(
        net.init(&mut bus, &transport(), &mut dma).err(),
        Some("version1"),
        "thiết bị legacy phải bị từ chối ở đúng chặng version1"
    );
    assert!(!net.ready());
}

// ================================================================ 3. thiết bị từ chối FEATURES_OK

#[test]
fn features_ok_rejection_is_reported() {
    let mut dev = FakeDev::new();
    dev.accept_features_ok = false;
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    assert_eq!(
        net.init(&mut bus, &transport(), &mut dma).err(),
        Some("features_ok"),
        "§2.4.2 bước 5: thiết bị từ chối FEATURES_OK ⇒ fail-closed"
    );
    assert!(!net.ready());
}

// ================================================================ 4. queue quá nhỏ

#[test]
fn tiny_queue_is_rejected() {
    let mut dev = FakeDev::new();
    dev.queue_max = [1, 1]; // < MIN_QUEUE_SIZE (2) — không dựng được ring
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    assert_eq!(
        net.init(&mut bus, &transport(), &mut dma).err(),
        Some("queue_size")
    );
    assert!(!net.ready());
}

// ================================================================ 5. config_generation xé

#[test]
fn torn_config_generation_is_rejected() {
    let mut dev = FakeDev::new();
    dev.generation_tears = true;
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    assert_eq!(
        net.init(&mut bus, &transport(), &mut dma).err(),
        Some("config_generation"),
        "đọc config mà generation đổi ⇒ dữ liệu xé ⇒ fail-closed (spec §2.4.4)"
    );
}

// ================================================================ 6. MAC vô nghĩa

#[test]
fn zero_mac_is_rejected() {
    let mut dev = FakeDev::new();
    dev.zero_mac = true; // MAC toàn 0 — thiết bị thật không bao giờ phát
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    assert_eq!(
        net.init(&mut bus, &transport(), &mut dma).err(),
        Some("mac")
    );
    assert!(!net.ready());
}

// ================================================================ 7–9. chưa init / frame sai / timeout

#[test]
fn calls_before_init_fail_closed() {
    let net = Net::new();
    let mut bus = FakeBus {
        dev: FakeDev::new(),
    };
    let mut dma = FakeDma::new();
    let mut clock = StepClock { now: Cell::new(0) };
    let mut frame = [0u8; 64];
    assert_eq!(
        net.send_frame(&mut bus, &mut dma, &mut clock, &frame, 1_000),
        Err(NetError::NotReady)
    );
    assert_eq!(
        net.recv_frame_wait(&mut bus, &mut dma, &mut clock, &mut frame, 1_000),
        None
    );
    assert_eq!(net.device_status(&mut bus), 0);
    assert_eq!(net.config_generation(&mut bus), 0);
    assert_eq!(net.dma_pa(), 0);
    assert_eq!(net.isr_addr(), 0);
}

#[test]
fn oversized_frame_is_rejected_before_ring() {
    let mut dev = FakeDev::new();
    dev.device_features |= 1u64 << F_VERSION_1;
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    net.init(&mut bus, &transport(), &mut dma)
        .expect("init xong");
    let before = net.stats();
    // 1515 byte > MAX_FRAME_BYTES (1514) ⇒ từ chối **trước** khi chạm ring.
    let frame = [0u8; 1515];
    let mut clock = StepClock { now: Cell::new(0) };
    assert_eq!(
        net.send_frame(&mut bus, &mut dma, &mut clock, &frame, 1_000),
        Err(NetError::BadFrame)
    );
    let after = net.stats();
    assert_eq!(after.errors, before.errors + 1, "đếm lỗi, không im lặng");
    assert_eq!(after.tx_frames, before.tx_frames, "không frame nào ra dây");
    assert_eq!(
        after.notifies, before.notifies,
        "không notify ⇒ thiết bị không thấy descriptor rác"
    );
}

#[test]
fn unfinished_tx_times_out() {
    let mut dev = FakeDev::new();
    dev.device_features |= 1u64 << F_VERSION_1;
    // Thiết bị **không** hoàn tất `tx` (`used->idx` luôn 0) ⇒ driver phải timeout, không treo.
    let mut bus = FakeBus { dev };
    let mut dma = FakeDma::new();
    let net = Net::new();
    net.init(&mut bus, &transport(), &mut dma)
        .expect("init xong");
    let before = net.stats();
    let frame = vec![0x11u8; HDR_MODERN_BYTES + 60];
    let mut clock = StepClock { now: Cell::new(0) };
    assert_eq!(
        net.send_frame(&mut bus, &mut dma, &mut clock, &frame, 10_000),
        Err(NetError::Timeout)
    );
    let after = net.stats();
    assert_eq!(after.timeouts, before.timeouts + 1);
    assert_eq!(after.errors, before.errors + 1);
    assert_eq!(
        after.tx_frames, before.tx_frames,
        "không đếm frame chưa xong"
    );
}
