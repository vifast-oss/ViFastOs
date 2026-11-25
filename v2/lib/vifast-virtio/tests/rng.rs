// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test cho **`virtio-rng`** (KT-418b) — device id **4**, ring một hàng đợi, cổng **chất lượng
//! entropy** và driver dùng chung 3 arch (`vifast_virtio::rngdev`).
//!
//! ```text
//! cargo test --manifest-path v2/Cargo.toml -p vifast-virtio --test rng
//! ```
//!
//! # Vì sao test này tồn tại
//!
//! `virtio-rng` là **nguồn entropy duy nhất** mà kernel lấy từ phần cứng; một driver "chạy được"
//! nhưng nhận **toàn số 0** từ thiết bị hỏng sẽ đầu độc mọi khoá sinh sau đó mà **không** có triệu
//! chứng. Vì vậy test chốt **ba** lớp:
//!
//! 1. **định danh thiết bị** — device id 4 (`0x1004`/`0x1044`) khớp, id khác **không** khớp;
//! 2. **cổng chất lượng** — `entropy_ok` **từ chối** toàn 0 / toàn `0xff` / một giá trị lặp / chuỗi
//!    có run dài, và **nhận** dãy đếm 0..N (đa dạng) — cổng thuần, test host;
//! 3. **driver fail-closed** — thiết bị trả rác ⇒ `LowEntropy` + đếm `errors`; thiết bị **không**
//!    hoàn tất ⇒ `Timeout` + đếm `timeouts`; gọi trước `init` ⇒ `NotReady`; yêu cầu quá lớn ⇒
//!    `BadLen` **trước** khi chạm ring (không đẩy descriptor rác).
//!
//! Thiết bị giả **không** phải QEMU: nó chỉ mô phỏng đúng phần thanh ghi + trang DMA mà `rngdev`
//! đọc/ghi (cùng khuôn `tests/netdev.rs` của KT-335).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use vifast_virtio::blkdev::Transport;
use vifast_virtio::dma::{DmaBuf, DmaPool};
use vifast_virtio::mmio::MmioBus;
use vifast_virtio::pci::VirtioKind;
use vifast_virtio::queue::VqLayout;
use vifast_virtio::rng::{
    distinct_bytes, entropy_ok, is_virtio_rng, longest_run, RngError, MAX_REQUEST_BYTES,
    MIN_REQUEST_BYTES, QUEUES_REQUIRED, QUEUE_SIZE, Q_REQUEST, VIRTIO_RNG_MODERN,
    VIRTIO_RNG_MODERN_ONLY,
};
use vifast_virtio::rngdev::{Clock, Rng, RNG_BUF_BYTES, RNG_BUF_OFF, RNG_RING_OFF};

// ---------------------------------------------------------------- địa chỉ mô phỏng

const BAR: u64 = 0x1000_0000;
const COMMON: u64 = BAR + 0x1000;
const NOTIFY: u64 = BAR + 0x2000;
const ISR: u64 = BAR + 0x3000;
const NOTIFY_MUL: u32 = 4;
const DMA_BASE_PA: u64 = 0x8000_0000;
const DMA_BYTES: usize = 4096;

// Thanh ghi common config (spec virtio 1.0 §4.1.4.3) — chép số ở đây là **có chủ đích**: test phải
// độc lập với hằng private của driver, nếu không nó chỉ kiểm chính nó.
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
const C_QUEUE_DESC: u64 = 0x20;
const C_QUEUE_DRIVER: u64 = 0x28;
const C_QUEUE_DEVICE: u64 = 0x30;

/// `VIRTIO_F_VERSION_1` (bit 32) — feature bắt buộc của virtio 1.0.
const F_VERSION_1: u32 = 32;

// ---------------------------------------------------------------- thiết bị giả

/// Cách thiết bị giả trả entropy — mỗi biến thể là một ca của cổng chất lượng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fill {
    /// Dãy giả ngẫu nhiên (LCG) — **phải** qua cổng.
    Random,
    /// Toàn 0 — thiết bị hỏng; **phải** bị từ chối.
    Zeros,
    /// Một giá trị lặp — **phải** bị từ chối.
    Constant,
    /// **Không** bao giờ hoàn tất — ca timeout.
    Never,
}

struct FakeDev {
    device_features: u64,
    driver_features: [u32; 2],
    select: u32,
    status: u8,
    queue_select: u16,
    queue_max: [u16; 1],
    queue_size: [u16; 1],
    num_queues: u16,
    queue_desc: [u64; 1],
    queue_driver: [u64; 1],
    queue_device: [u64; 1],
}

impl FakeDev {
    fn new() -> Self {
        Self {
            device_features: 1u64 << F_VERSION_1,
            driver_features: [0; 2],
            select: 0,
            status: 0,
            queue_select: 0,
            queue_max: [QUEUE_SIZE],
            queue_size: [0; 1],
            num_queues: QUEUES_REQUIRED,
            queue_desc: [0; 1],
            queue_driver: [0; 1],
            queue_device: [0; 1],
        }
    }

    fn read8(&self, addr: u64) -> u8 {
        match addr - COMMON {
            C_DEVICE_STATUS => self.status,
            _ => 0,
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
            self.status = value;
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
}

struct FakeBus {
    dev: FakeDev,
    doorbells: Rc<Cell<u32>>,
}

impl FakeBus {
    fn new(dev: FakeDev) -> Self {
        Self {
            dev,
            doorbells: Rc::new(Cell::new(0)),
        }
    }
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
    fn notify(&mut self, _addr: u64, _queue: u16) {
        self.doorbells.set(self.doorbells.get().wrapping_add(1));
    }
}

/// Trạng thái **thiết bị** trên trang DMA: hoàn tất request khi driver poll `used->idx`.
struct DeviceState {
    fill: Fill,
    /// `used->idx` thiết bị đã ghi.
    used: u16,
    /// Số request thiết bị đã phục vụ (bằng chứng driver thật sự đẩy descriptor).
    served: u32,
    /// Byte entropy thiết bị đã ghi (bằng chứng độ dài khớp descriptor).
    bytes: u64,
    /// Trạng thái LCG — tất định để test tái lập được.
    lcg: u64,
}

struct FakeDma {
    mem: RefCell<Vec<u8>>,
    device: RefCell<DeviceState>,
}

impl FakeDma {
    fn new(fill: Fill) -> Self {
        Self {
            mem: RefCell::new(vec![0u8; DMA_BYTES]),
            device: RefCell::new(DeviceState {
                fill,
                used: 0,
                served: 0,
                bytes: 0,
                lcg: 0x2545_f491_4f6c_dd1d,
            }),
        }
    }

    fn layout() -> VqLayout {
        VqLayout::new(QUEUE_SIZE).expect("queue size hợp lệ")
    }

    /// Thiết bị phục vụ **mọi** request đang chờ (avail > used) — đúng một descriptor mỗi request.
    fn serve_pending(&self) {
        let layout = Self::layout();
        let mut device = self.device.borrow_mut();
        if device.fill == Fill::Never {
            return;
        }
        let mem = self.mem.borrow();
        let avail = u16::from_le_bytes([
            mem[RNG_RING_OFF + layout.avail_idx_off],
            mem[RNG_RING_OFF + layout.avail_idx_off + 1],
        ]);
        drop(mem);
        while device.used != avail {
            let slot = device.used & (QUEUE_SIZE - 1);
            let head = {
                let mem = self.mem.borrow();
                let at = RNG_RING_OFF + layout.avail_ring_off + slot as usize * 2;
                u16::from_le_bytes([mem[at], mem[at + 1]])
            };
            // Descriptor head phải là descriptor 0 và device-writable (spec §2.6.4).
            let (addr, len, flags) = {
                let mem = self.mem.borrow();
                let at = RNG_RING_OFF + layout.desc_off + head as usize * 16;
                (
                    u64::from_le_bytes(mem[at..at + 8].try_into().unwrap()),
                    u32::from_le_bytes(mem[at + 8..at + 12].try_into().unwrap()),
                    u16::from_le_bytes(mem[at + 12..at + 14].try_into().unwrap()),
                )
            };
            assert_eq!(head, 0, "driver phải dùng descriptor 0");
            assert_eq!(flags & 0x2, 0x2, "đệm entropy phải device-writable");
            let off = (addr - DMA_BASE_PA) as usize;
            assert_eq!(off, RNG_BUF_OFF, "đệm entropy phải ở RNG_BUF_OFF");
            assert!(len as usize <= RNG_BUF_BYTES, "đệm vượt vùng DMA");
            {
                let mut mem = self.mem.borrow_mut();
                for index in 0..len as usize {
                    let byte = match device.fill {
                        Fill::Random => {
                            device.lcg = device
                                .lcg
                                .wrapping_mul(6364136223846793005)
                                .wrapping_add(1442695040888963407);
                            (device.lcg >> 33) as u8
                        }
                        Fill::Zeros => 0,
                        Fill::Constant => 0xaa,
                        Fill::Never => unreachable!(),
                    };
                    mem[off + index] = byte;
                }
                // `used->ring[slot] = (id, len)` rồi tăng `used->idx` (thứ tự spec §2.7.8).
                let at = RNG_RING_OFF + layout.used_ring_off + slot as usize * 8;
                mem[at..at + 4].copy_from_slice(&u32::from(head).to_le_bytes());
                mem[at + 4..at + 8].copy_from_slice(&len.to_le_bytes());
                let next = device.used.wrapping_add(1);
                let iat = RNG_RING_OFF + layout.used_idx_off;
                mem[iat..iat + 2].copy_from_slice(&next.to_le_bytes());
            }
            device.used = device.used.wrapping_add(1);
            device.served += 1;
            device.bytes += u64::from(len);
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
        self.mem.get_mut()
    }
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf> {
        if len == 0 || align == 0 || !align.is_power_of_two() || len > DMA_BYTES {
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
    fn sync_for_device(&mut self, _off: usize, _len: usize) {}
    fn sync_for_cpu(&mut self, _off: usize, _len: usize) {}
    fn read_u16(&self, off: usize) -> u16 {
        // Poll `used->idx` = chỗ thiết bị giả hoàn tất request (đúng thứ tự spec).
        if off == RNG_RING_OFF + Self::layout().used_idx_off {
            self.serve_pending();
        }
        let mem = self.mem.borrow();
        u16::from_le_bytes([mem[off], mem[off + 1]])
    }
    fn read_u8(&self, off: usize) -> u8 {
        self.mem.borrow()[off]
    }
}

/// Đồng hồ giả: mỗi lần đọc tăng `step` — đủ để ca timeout kết thúc nhanh, tất định.
struct FakeClock {
    now: u64,
    step: u64,
}

impl Clock for FakeClock {
    fn now(&mut self) -> u64 {
        self.now = self.now.wrapping_add(self.step);
        self.now
    }
}

fn transport() -> Transport {
    Transport {
        vendor: 0x1af4,
        device: VIRTIO_RNG_MODERN,
        bus: 0,
        dev: 3,
        func: 0,
        bar_index: 4,
        bar_base: BAR,
        bar_size: 0x4000,
        common: COMMON,
        notify: NOTIFY,
        device_cfg: ISR,
        isr: ISR,
        notify_mul: NOTIFY_MUL,
    }
}

// ---------------------------------------------------------------- 1) định danh thiết bị

#[test]
fn device_id_4_va_hai_ban_deu_khop() {
    // **Đo được** bằng QEMU 11.1.3 (`info pci`), không suy từ "device id 4": bảng id PCI legacy
    // của virtio **không** cộng đều `0x1000 + device_id` (`0x1004` là virtio-scsi).
    assert_eq!(
        VIRTIO_RNG_MODERN, 0x1005,
        "virtio-rng transitional = 0x1005 (đo QEMU)"
    );
    assert_eq!(
        VIRTIO_RNG_MODERN_ONLY, 0x1044,
        "virtio-rng modern-only = 0x1044 (đo QEMU)"
    );
    assert!(is_virtio_rng(0x1af4, VIRTIO_RNG_MODERN));
    assert!(is_virtio_rng(0x1af4, VIRTIO_RNG_MODERN_ONLY));
    // Vendor sai ⇒ không khớp, kể cả đúng device id.
    assert!(!is_virtio_rng(0x1234, VIRTIO_RNG_MODERN));
    // Các thiết bị khác **không** bị nhận nhầm là rng.
    // `0x1004` = virtio-scsi: **không** được nhận nhầm là rng (đây là bẫy đã mắc ở bản đầu).
    for other in [
        0x1000u16, 0x1001, 0x1002, 0x1003, 0x1004, 0x1012, 0x1050, 0x1041,
    ] {
        assert!(
            !is_virtio_rng(0x1af4, other),
            "id 0x{other:04x} không phải rng"
        );
    }
    // `VirtioKind::Rng` là **một** nguồn: quét PCI dùng nó, không chép số lần hai.
    assert!(VirtioKind::Rng.matches(0x1af4, VIRTIO_RNG_MODERN));
    assert!(VirtioKind::Rng.matches(0x1af4, VIRTIO_RNG_MODERN_ONLY));
    assert_eq!(VirtioKind::Rng.name(), "rng");
    // Queue: đúng **một** hàng đợi, chỉ số 0 (spec §5.4.2).
    assert_eq!(Q_REQUEST, 0);
    assert_eq!(QUEUES_REQUIRED, 1);
    // Hằng số phải tự nhất quán — kiểm ở **compile time** (clippy: không `assert!` hằng).
    const _: () = assert!(QUEUE_SIZE.is_power_of_two() && QUEUE_SIZE >= 2);
    const _: () = assert!(MIN_REQUEST_BYTES >= 16 && MAX_REQUEST_BYTES >= MIN_REQUEST_BYTES);
}

// ---------------------------------------------------------------- 2) cổng chất lượng

#[test]
fn cong_chat_luong_tu_choi_entropy_rac() {
    // Toàn 0 / toàn 0xff / một giá trị lặp ⇒ **từ chối** (thiết bị hỏng hoặc bị cắt nguồn).
    assert!(!entropy_ok(&[0u8; 64]));
    assert!(!entropy_ok(&[0xffu8; 64]));
    assert!(!entropy_ok(&[0xaau8; 64]));
    // Run dài dù có vài giá trị khác ⇒ vẫn từ chối (nguồn "gần như hằng").
    let mut run = [0u8; 64];
    run[0] = 1;
    run[1] = 2;
    run[2] = 3;
    run[3] = 4;
    run[4] = 5;
    run[5] = 6;
    run[6] = 7;
    run[7] = 8;
    assert!(!entropy_ok(&run), "run 56 byte giống nhau phải bị từ chối");
    // Quá ngắn ⇒ không đủ mẫu để kết luận ⇒ từ chối (fail-closed).
    assert!(!entropy_ok(&[1, 2, 3, 4]));
    // Dãy đếm 0..63: đa dạng, không run ⇒ **nhận**.
    let mut counter = [0u8; 64];
    for (index, byte) in counter.iter_mut().enumerate() {
        *byte = index as u8;
    }
    assert!(entropy_ok(&counter));
    assert_eq!(distinct_bytes(&counter), 64);
    assert_eq!(longest_run(&counter), 1);
    assert_eq!(distinct_bytes(&[0u8; 64]), 1);
    assert_eq!(longest_run(&[0u8; 64]), 64);
}

// ---------------------------------------------------------------- 3) driver

#[test]
fn init_va_fill_lay_entropy_tu_thiet_bi() {
    let mut bus = FakeBus::new(FakeDev::new());
    let mut dma = FakeDma::new(Fill::Random);
    let rng = Rng::new();
    let report = rng
        .init(&mut bus, &transport(), &mut dma)
        .expect("init phải xanh");
    assert_eq!(report.device_id, VIRTIO_RNG_MODERN);
    assert!(report.version1, "VERSION_1 phải đàm phán được");
    assert_eq!(report.queue_size, QUEUE_SIZE);
    assert_eq!(report.requests, 0);

    let mut clock = FakeClock { now: 0, step: 1 };
    let mut out = [0u8; 64];
    let got = rng
        .fill(&mut bus, &mut dma, &mut out, 1_000, &mut clock)
        .expect("fill phải xanh");
    assert_eq!(got, 64, "thiết bị trả đủ số byte yêu cầu");
    assert!(
        entropy_ok(&out),
        "entropy thiết bị trả phải qua cổng chất lượng"
    );
    let report = rng.report();
    assert_eq!(report.requests, 1);
    assert_eq!(report.bytes, 64);
    assert_eq!(report.notifies, 1, "mỗi request đúng một doorbell");
    assert_eq!(report.errors, 0);
    assert_eq!(report.timeouts, 0);
    assert!(report.entropy_ok);
    assert_eq!(report.distinct, distinct_bytes(&out));
    assert_eq!(report.longest_run, longest_run(&out));
    // Thiết bị **thật sự** nhận request (không phải driver tự bịa byte).
    assert_eq!(dma.device.borrow().served, 1);
    assert_eq!(dma.device.borrow().bytes, 64);
}

#[test]
fn thiet_bi_tra_toan_0_thi_fill_do() {
    let mut bus = FakeBus::new(FakeDev::new());
    let mut dma = FakeDma::new(Fill::Zeros);
    let rng = Rng::new();
    rng.init(&mut bus, &transport(), &mut dma)
        .expect("init phải xanh");
    let mut clock = FakeClock { now: 0, step: 1 };
    let mut out = [0u8; 64];
    let err = rng
        .fill(&mut bus, &mut dma, &mut out, 1_000, &mut clock)
        .expect_err("entropy toàn 0 phải bị từ chối");
    assert_eq!(err, RngError::LowEntropy);
    assert_eq!(err.name(), "low_entropy");
    let report = rng.report();
    assert_eq!(report.errors, 1, "đếm lỗi, không im lặng");
    assert!(!report.entropy_ok);
    assert_eq!(report.bytes, 0, "byte rác **không** được tính là đã lấy");
}

#[test]
fn thiet_bi_mot_gia_tri_lap_thi_fill_do() {
    let mut bus = FakeBus::new(FakeDev::new());
    let mut dma = FakeDma::new(Fill::Constant);
    let rng = Rng::new();
    rng.init(&mut bus, &transport(), &mut dma)
        .expect("init phải xanh");
    let mut clock = FakeClock { now: 0, step: 1 };
    let mut out = [0u8; 64];
    assert_eq!(
        rng.fill(&mut bus, &mut dma, &mut out, 1_000, &mut clock),
        Err(RngError::LowEntropy)
    );
}

#[test]
fn thiet_bi_khong_hoan_tat_thi_timeout() {
    let mut bus = FakeBus::new(FakeDev::new());
    let mut dma = FakeDma::new(Fill::Never);
    let rng = Rng::new();
    rng.init(&mut bus, &transport(), &mut dma)
        .expect("init phải xanh");
    // Đồng hồ nhảy 100 mỗi lần đọc ⇒ vượt hạn 1.000 sau ~11 vòng, không treo test.
    let mut clock = FakeClock { now: 0, step: 100 };
    let mut out = [0u8; 64];
    let err = rng
        .fill(&mut bus, &mut dma, &mut out, 1_000, &mut clock)
        .expect_err("thiết bị không hoàn tất phải timeout");
    assert_eq!(err, RngError::Timeout);
    assert_eq!(err.name(), "timeout");
    assert_eq!(rng.report().timeouts, 1);
    assert_eq!(rng.report().bytes, 0);
}

#[test]
fn goi_truoc_init_va_yeu_cau_sai_do_dai() {
    let mut bus = FakeBus::new(FakeDev::new());
    let mut dma = FakeDma::new(Fill::Random);
    let rng = Rng::new();
    let mut clock = FakeClock { now: 0, step: 1 };
    let mut out = [0u8; 64];
    assert_eq!(
        rng.fill(&mut bus, &mut dma, &mut out, 1_000, &mut clock),
        Err(RngError::NotReady),
        "chưa init ⇒ NotReady, không PANIC"
    );
    rng.init(&mut bus, &transport(), &mut dma)
        .expect("init phải xanh");
    // Quá lớn / quá nhỏ ⇒ BadLen **trước** khi chạm ring (không đẩy descriptor rác).
    let mut big = [0u8; MAX_REQUEST_BYTES + 1];
    assert_eq!(
        rng.fill(&mut bus, &mut dma, &mut big, 1_000, &mut clock),
        Err(RngError::BadLen)
    );
    let mut small = [0u8; MIN_REQUEST_BYTES - 1];
    assert_eq!(
        rng.fill(&mut bus, &mut dma, &mut small, 1_000, &mut clock),
        Err(RngError::BadLen)
    );
    assert_eq!(
        rng.report().requests,
        0,
        "yêu cầu sai không được tính là request"
    );
    assert_eq!(dma.device.borrow().served, 0, "không descriptor nào bị đẩy");
}

#[test]
fn thiet_bi_khong_co_version_1_thi_init_do() {
    let mut dev = FakeDev::new();
    dev.device_features = 0; // thiết bị không báo VERSION_1
    let mut bus = FakeBus::new(dev);
    let mut dma = FakeDma::new(Fill::Random);
    let rng = Rng::new();
    assert_eq!(
        rng.init(&mut bus, &transport(), &mut dma),
        Err(RngError::Version1)
    );
    assert_eq!(RngError::Version1.name(), "version1");
}

#[test]
fn thiet_bi_thieu_queue_thi_init_do() {
    let mut dev = FakeDev::new();
    dev.num_queues = 0;
    let mut bus = FakeBus::new(dev);
    let mut dma = FakeDma::new(Fill::Random);
    let rng = Rng::new();
    assert_eq!(
        rng.init(&mut bus, &transport(), &mut dma),
        Err(RngError::Queues)
    );
    assert_eq!(RngError::Queues.name(), "queues");
}

#[test]
fn hai_lan_fill_lien_tiep_khong_lap_byte() {
    let mut bus = FakeBus::new(FakeDev::new());
    let mut dma = FakeDma::new(Fill::Random);
    let rng = Rng::new();
    rng.init(&mut bus, &transport(), &mut dma)
        .expect("init phải xanh");
    let mut clock = FakeClock { now: 0, step: 1 };
    let mut first = [0u8; 64];
    let mut second = [0u8; 64];
    rng.fill(&mut bus, &mut dma, &mut first, 1_000, &mut clock)
        .expect("fill 1");
    rng.fill(&mut bus, &mut dma, &mut second, 1_000, &mut clock)
        .expect("fill 2");
    assert_ne!(first, second, "hai request phải là hai mẫu khác nhau");
    assert_eq!(rng.report().requests, 2);
    assert_eq!(rng.report().bytes, 128);
    assert_eq!(rng.report().notifies, 2);
}
