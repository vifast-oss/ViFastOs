// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-blk** (virtio 1.0, virtio-pci modern) trên x86_64 — KT-288 (K5' bước 9).
//!
//! # Đường đi
//!
//! [`crate::pci::init_for`]`(VirtioKind::Blk)` (ECAM/legacy của KT-274, **không** nhân đôi code
//! PCI) ⇒ BAR firmware đã gán + bốn capability ⇒ tệp này lập trình common config, dựng **split
//! virtqueue** trong **một trang DMA 4 KiB** căn lề, rồi gửi request `IN`/`OUT`/`FLUSH` và **poll
//! `used`** (bước này chưa dùng ngắt).
//!
//! # Khác khuôn aarch64 (KT-273) ở đúng ba điểm, đều do kiến trúc
//!
//! 1. **DMA bằng VA = PA**: x86_64 chạy identity map 1 GiB ([`crate::boot::IDENTITY_MAP_BYTES`])
//!    nên `addr_of!(trang DMA)` **là** PA — không có alias nửa cao, không cần `high_to_pa`.
//! 2. **BAR do firmware gán**: SeaBIOS đã enumerate và gán BAR trước khi ảnh chạy, nên driver
//!    **đọc** rồi kiểm BAR nằm trong cửa sổ MMIO đã map ([`crate::boot::mmio_mapped`]) — không
//!    ghi đè địa chỉ firmware đã cấp (xem đầu [`crate::pci`]). "Leaf riêng" vì vậy là **bất
//!    biến kiểm được** ([`bar_disjoint`]): BAR của blk phải rời BAR của virtio-net, không đè.
//! 3. **Đồng hồ là tham số**, không phải singleton: bước này chạy sau `cli` nên không dùng được
//!    tick PIT→IOAPIC→LAPIC; đồng hồ là PIT kênh 2 ([`crate::timebase::PitClock`]) do ảnh sở
//!    hữu. Mọi hàm chờ nhận `now: &mut dyn FnMut() -> u64` (µs) — driver **không** giữ trạng
//!    thái thời gian, và test host không cần đồng hồ thật (cùng khuôn [`crate::virtio_net`]).
//!
//! # Bất biến
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1` (bit 32) + `VIRTIO_BLK_F_FLUSH` **nếu**
//!   thiết bị có — quyết định nằm ở `vifast_virtio::blk::negotiated_features` (test host).
//! - Thứ tự spec §2.7: ghi descriptor → `avail->ring` → **fence** → `avail->idx` → **fence** →
//!   notify (MMIO) → poll `used->idx` → **fence** → đọc `used->elem`/status.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - Request dựng bằng `vifast_virtio::blk::build_request` — **cùng** mã mà test host kiểm
//!   (`v2/lib/vifast-virtio/tests/driver_request.rs`), không ghép header bằng tay.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); LBA ngoài dung lượng ⇒
//!   `OutOfRange` **trước** khi chạm ring; status lạ ⇒ `Io` + đếm `errors`.
//!
//! # Vì sao tệp này có `unsafe`
//!
//! MMIO của BAR virtio + vùng DMA (`UnsafeCell` + `Sync` thủ công) là truy cập con trỏ thô, đúng
//! danh sách đóng D2-17 (`kernel/vifast-plat-*`) và **y hệt** khuôn
//! `kernel/vifast-plat-x86_64/src/virtio_net.rs`. Crate vẫn `#![deny(unsafe_code)]`; chỉ tệp này
//! (và [`crate::asm`]) `#![allow(unsafe_code)]`.

#![allow(unsafe_code)] // D2-17: `kernel/vifast-plat-*`; MMIO + vùng DMA.

use core::cell::UnsafeCell;
use core::sync::atomic::{fence, Ordering};

use vifast_virtio::blk::{
    build_request, decode_config, flush_negotiated, negotiated_features, status_to_result,
    BlkError, BLK_CONFIG_BYTES, T_FLUSH, T_IN, T_OUT,
};
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::asm;
use crate::pci::PciReport;

/// Số descriptor của queue (luỹ thừa 2). 8 đủ cho một request 3 descriptor và rẻ về `.bss`.
pub const QUEUE_SIZE: u16 = 8;
/// Cỡ mỗi sector của `virtio_blk` và của `vifast_fs::BLOCK_SIZE`.
pub const SECTOR_BYTES: usize = 512;
/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset vùng request header trong trang DMA.
pub const REQ_OFF: usize = 0x200;
/// Offset vùng đệm dữ liệu 512 B.
pub const DATA_OFF: usize = 0x220;
/// Offset byte status (thiết bị ghi đè).
pub const STATUS_OFF: usize = 0x420;

/// Chữ ký mà runner ghi vào **8 byte đầu** của sector 0 của đĩa tạm.
///
/// Đây là **hợp đồng với runner** (`run-x86_64.sh` tạo đĩa bằng `printf 'VIFASTOS-KT288-S0'`):
/// ảnh in `magic=1` chỉ khi 8 byte đầu khớp đúng chữ ký này, nên "đọc được sector 0" không thể
/// là đọc ra số 0 rồi tự khen. Test host `magic_is_prefix_of_runner_signature` chốt hợp đồng.
pub const SECTOR0_MAGIC: [u8; 8] = *b"VIFASTOS";

/// Trần poll `used` — 2 s (cùng ngân sách khuôn aarch64 KT-273 / riscv64 KT-72).
pub const POLL_TIMEOUT_US: u64 = 2_000_000;

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

// ---------------------------------------------------------------- bộ đếm (an toàn, atomic)
static REQUESTS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static READS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static WRITES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static FLUSHES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static COMPLETIONS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static ERRORS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static NOTIFIES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static ID_MISMATCH: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static TIMEOUTS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Ảnh chụp bộ đếm để in evidence (đọc một lần, không tách rời từng số).
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Tổng request đã phát (kể cả lỗi).
    pub requests: u64,
    /// Request đọc.
    pub reads: u64,
    /// Request ghi.
    pub writes: u64,
    /// Request flush.
    pub flushes: u64,
    /// Request hoàn tất với status OK.
    pub completions: u64,
    /// Request lỗi (status ≠ 0 hoặc lỗi driver sau khi phát).
    pub errors: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lần `used->id` lệch head (phải 0).
    pub id_mismatch: u64,
    /// Số lần poll quá hạn (phải 0).
    pub timeouts: u64,
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    Stats {
        requests: REQUESTS.load(Ordering::SeqCst),
        reads: READS.load(Ordering::SeqCst),
        writes: WRITES.load(Ordering::SeqCst),
        flushes: FLUSHES.load(Ordering::SeqCst),
        completions: COMPLETIONS.load(Ordering::SeqCst),
        errors: ERRORS.load(Ordering::SeqCst),
        notifies: NOTIFIES.load(Ordering::SeqCst),
        id_mismatch: ID_MISMATCH.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
    }
}

/// Trạng thái driver — gói trong `UnsafeCell` + `Sync` thủ công: ảnh chạy **một** CPU trên đường
/// driver và driver tự tuần tự hoá (một request một lúc), không có truy cập song song.
struct State {
    ready: bool,
    common: u64,
    notify: u64,
    isr: u64,
    device_cfg: u64,
    notify_mul: u32,
    /// `queue_notify_off` của queue 0.
    notify_off: u16,
    /// Cỡ queue thật (đọc lại từ thiết bị).
    queue_size: u16,
    /// Dung lượng theo sector 512 B (đọc từ `virtio_blk_config`).
    capacity: u64,
    /// `true` ⇔ đã đàm phán `VIRTIO_BLK_F_FLUSH` (được phép gửi `T_FLUSH`).
    flush_ok: bool,
    /// `used->idx` đã xử lý.
    last_used: u16,
}

const INIT_STATE: State = State {
    ready: false,
    common: 0,
    notify: 0,
    isr: 0,
    device_cfg: 0,
    notify_mul: 0,
    notify_off: 0,
    queue_size: 0,
    capacity: 0,
    flush_ok: false,
    last_used: 0,
};

struct Cell<T>(UnsafeCell<T>);
// SAFETY: một CPU, truy cập tuần tự trong module này (không chia sẻ giữa các CPU).
unsafe impl<T> Sync for Cell<T> {}

static STATE: Cell<State> = Cell(UnsafeCell::new(INIT_STATE));

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: như `STATE` — vùng DMA chỉ driver này chạm; thiết bị chạm qua PA (DMA) theo hợp đồng.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA`.
pub fn dma_page_addr() -> u64 {
    DMA.0.get() as *const _ as u64
}

fn state() -> &'static mut State {
    // SAFETY: một CPU + tuần tự (xem `Cell`).
    unsafe { &mut *STATE.0.get() }
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một CPU + tuần tự; vùng này không ai khác (trừ DMA của thiết bị) chạm.
    unsafe { &mut (*DMA.0.get()).0 }
}

// ---------------------------------------------------------------- MMIO
#[inline]
fn read8(addr: u64) -> u8 {
    asm::read_u8(addr)
}
#[inline]
fn read16(addr: u64) -> u16 {
    asm::read_u16(addr)
}
#[inline]
fn read32(addr: u64) -> u32 {
    asm::read_u32(addr)
}
#[inline]
fn write8(addr: u64, value: u8) {
    asm::write_u8(addr, value);
}
#[inline]
fn write16(addr: u64, value: u16) {
    asm::write_u16(addr, value);
}
#[inline]
fn write32(addr: u64, value: u32) {
    asm::write_u32(addr, value);
}
#[inline]
fn write64(addr: u64, value: u64) {
    asm::write_u64(addr, value);
}

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13: driver phải
/// có barrier trước notify; thiếu ⇒ thiết bị có thể đọc `avail` cũ).
#[inline]
fn barrier() {
    fence(Ordering::SeqCst);
}

// ---------------------------------------------------------------- BAR rời nhau (leaf riêng)

/// `true` ⇔ hai vùng BAR **rời nhau** (không chồng byte nào) và cả hai đều hợp lệ.
///
/// Trên x86_64 firmware gán BAR, nên "leaf riêng" không phải một hằng địa chỉ như aarch64 mà là
/// **bất biến kiểm được**: BAR của blk không được trùng/đè BAR của virtio-net. Hàm này là chỗ
/// duy nhất quyết định điều đó (ảnh in `bar_distinct=1`, test host chốt cả ca âm).
///
/// Fail-closed: BAR chưa gán (`0`), cỡ `0`, hoặc phép cộng tràn 64 bit ⇒ `false` — không có
/// trường hợp "không chứng minh được nên coi là đúng".
pub const fn bar_disjoint(a_base: u64, a_size: u64, b_base: u64, b_size: u64) -> bool {
    if a_base == 0 || b_base == 0 || a_size == 0 || b_size == 0 {
        return false;
    }
    let Some(a_end) = a_base.checked_add(a_size) else {
        return false;
    };
    let Some(b_end) = b_base.checked_add(b_size) else {
        return false;
    };
    a_end <= b_base || b_end <= a_base
}

// ---------------------------------------------------------------- báo cáo khởi tạo
/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct BlkReport {
    /// Vendor/device (từ PCI config).
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// Bus/device/function.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Chỉ số BAR driver dùng.
    pub bar_index: u8,
    /// Địa chỉ BAR firmware đã gán.
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
    /// `notify_off_multiplier` đọc từ capability.
    pub notify_mul: u32,
    /// `queue_notify_off` của queue 0.
    pub notify_off: u16,
    /// `num_queues` thiết bị báo.
    pub num_queues: u16,
    /// Cỡ queue tối đa thiết bị cho phép.
    pub queue_max: u16,
    /// Cỡ queue driver đặt.
    pub queue_size: u16,
    /// Feature thiết bị (64 bit, đọc hai trang).
    pub device_features: u64,
    /// Feature driver đã ghi (đọc lại).
    pub driver_features: u64,
    /// `true` ⇔ đàm phán được `VIRTIO_BLK_F_FLUSH` (bit 9) — điều kiện để gửi `T_FLUSH`.
    pub flush_ok: bool,
    /// `device_status` sau DRIVER_OK.
    pub status: u8,
    /// `config_generation` đọc **sau** khi đọc config (spec: đọc lại để phát hiện đổi).
    pub config_generation: u8,
    /// Dung lượng theo **sector 512 B** — đọc từ `virtio_blk_config`.
    pub capacity: u64,
    /// `size_max` thiết bị.
    pub size_max: u32,
    /// `seg_max` thiết bị.
    pub seg_max: u32,
    /// `blk_size` thiết bị.
    pub blk_size: u32,
    /// PA trang DMA (identity map ⇒ VA == PA).
    pub dma_pa: u64,
    /// PA bảng descriptor.
    pub desc: u64,
    /// PA khối `avail`.
    pub avail: u64,
    /// PA khối `used`.
    pub used: u64,
    /// PA vùng request header (DMA).
    pub req_pa: u64,
    /// PA vùng dữ liệu 512 B (DMA).
    pub data_pa: u64,
    /// PA byte status (DMA).
    pub status_pa: u64,
}

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
pub fn init(pci: &PciReport) -> Result<BlkReport, &'static str> {
    let caps = pci.caps;
    let bar = pci.bar_base;
    let common = bar + caps.common.offset as u64;
    let notify = bar + caps.notify.offset as u64;
    let device_cfg = bar + caps.device.offset as u64;
    let isr = bar + caps.isr.offset as u64;
    let notify_mul = caps.notify_off_multiplier;
    let layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    if layout.total_bytes() > DMA_PAGE_BYTES {
        return Err("layout");
    }
    // Vùng DMA sạch trước khi bàn giao (kể cả khi init gọi lại).
    for byte in page().iter_mut() {
        *byte = 0;
    }

    // ---- 1) reset (§2.4.2 bước 1)
    write8(common + C_DEVICE_STATUS, 0);
    if read8(common + C_DEVICE_STATUS) != 0 {
        return Err("reset");
    }
    // ---- 2) ACK + DRIVER
    write8(common + C_DEVICE_STATUS, ACK);
    write8(common + C_DEVICE_STATUS, ACK | DRIVER);
    // ---- 3) feature thiết bị (hai trang 32 bit)
    write32(common + C_DEVICE_FEATURE_SELECT, 0);
    let low = read32(common + C_DEVICE_FEATURE) as u64;
    write32(common + C_DEVICE_FEATURE_SELECT, 1);
    let high = read32(common + C_DEVICE_FEATURE) as u64;
    let device_features = (high << 32) | low;
    if device_features & (1u64 << F_VERSION_1) == 0 {
        return Err("version1");
    }
    // ---- 4) đàm phán: VERSION_1 bắt buộc + `VIRTIO_BLK_F_FLUSH` **nếu thiết bị có** (gửi
    // `T_FLUSH` mà không đàm phán là vi phạm spec §5.2.3). Quyết định ở hàm thuần (test host).
    let wanted = negotiated_features(device_features);
    if wanted & (1u64 << F_VERSION_1) == 0 {
        return Err("version1");
    }
    write32(common + C_DRIVER_FEATURE_SELECT, 0);
    write32(common + C_DRIVER_FEATURE, (wanted & 0xffff_ffff) as u32);
    write32(common + C_DRIVER_FEATURE_SELECT, 1);
    write32(common + C_DRIVER_FEATURE, (wanted >> 32) as u32);
    // đọc lại cả hai trang
    write32(common + C_DRIVER_FEATURE_SELECT, 0);
    let drv_low = read32(common + C_DRIVER_FEATURE) as u64;
    write32(common + C_DRIVER_FEATURE_SELECT, 1);
    let drv_high = read32(common + C_DRIVER_FEATURE) as u64;
    let driver_features = (drv_high << 32) | drv_low;
    if driver_features != wanted {
        return Err("features");
    }
    let flush_ok = flush_negotiated(driver_features);
    // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
    write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
    let status = read8(common + C_DEVICE_STATUS);
    if !features_ok(status) {
        return Err("features_ok");
    }
    // ---- 6) queue 0
    let num_queues = read16(common + C_NUM_QUEUES);
    if num_queues == 0 {
        return Err("queues");
    }
    write16(common + C_QUEUE_SELECT, 0);
    let queue_max = read16(common + C_QUEUE_SIZE);
    // Chain của driver dùng 3 descriptor (header + data + status; FLUSH dùng 2) ⇒ queue **phải**
    // chứa được 3; `queue_size = 2` từng làm `set_desc(2)` bị bỏ im lặng (D2-110 medium).
    if queue_max < 3 || !queue_max.is_power_of_two() {
        return Err("queue_size");
    }
    let queue_size = QUEUE_SIZE.min(queue_max);
    write16(common + C_QUEUE_SIZE, queue_size);
    if read16(common + C_QUEUE_SIZE) != queue_size {
        return Err("queue_size");
    }
    let dma = dma_page_addr();
    let desc_pa = dma + layout.desc_off as u64;
    let avail_pa = dma + layout.avail_off as u64;
    let used_pa = dma + layout.used_off as u64;
    let req_pa = dma + REQ_OFF as u64;
    let data_pa = dma + DATA_OFF as u64;
    let status_pa = dma + STATUS_OFF as u64;
    write64(common + C_QUEUE_DESC, desc_pa);
    write64(common + C_QUEUE_DRIVER, avail_pa);
    write64(common + C_QUEUE_DEVICE, used_pa);
    barrier();
    write16(common + C_QUEUE_ENABLE, 1);
    if read16(common + C_QUEUE_ENABLE) != 1 {
        return Err("queue_enable");
    }
    let notify_off = read16(common + C_QUEUE_NOTIFY_OFF);
    // ---- 7) DRIVER_OK
    write8(
        common + C_DEVICE_STATUS,
        ACK | DRIVER | FEATURES_OK | DRIVER_OK,
    );
    let status = read8(common + C_DEVICE_STATUS);
    if !status_sequence_ok(status) {
        return Err("driver_ok");
    }
    // ---- 8) device config: capacity/size_max/seg_max/blk_size đọc từ thiết bị
    let generation_before = read8(common + C_CONFIG_GENERATION);
    let mut raw = [0u8; BLK_CONFIG_BYTES];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = read8(device_cfg + index as u64);
    }
    let config_generation = read8(common + C_CONFIG_GENERATION);
    // Spec §2.4.4: nếu `config_generation` đổi trong lúc đọc config thì dữ liệu có thể **xé** ⇒
    // fail-closed (không dùng capacity đo được nửa vời).
    if config_generation != generation_before {
        return Err("config_generation");
    }
    let config = decode_config(&raw).ok_or("config")?;
    if config.capacity_sectors == 0 {
        return Err("capacity");
    }
    let capacity = config.capacity_sectors;
    // ---- 9) lưu trạng thái
    {
        let s = state();
        s.ready = true;
        s.common = common;
        s.notify = notify;
        s.isr = isr;
        s.device_cfg = device_cfg;
        s.notify_mul = notify_mul;
        s.notify_off = notify_off;
        s.queue_size = queue_size;
        s.capacity = capacity;
        s.flush_ok = flush_ok;
        s.last_used = 0;
    }
    Ok(BlkReport {
        vendor: pci.vendor,
        device: pci.device,
        bus: pci.bus,
        dev: pci.dev,
        func: pci.func,
        bar_index: pci.bar_index,
        bar_base: bar,
        bar_size: pci.bar_size,
        common,
        notify,
        device_cfg,
        isr,
        notify_mul,
        notify_off,
        num_queues,
        queue_max,
        queue_size,
        device_features,
        driver_features,
        flush_ok,
        status,
        config_generation,
        capacity,
        size_max: config.size_max,
        seg_max: config.seg_max,
        blk_size: config.blk_size,
        dma_pa: dma,
        desc: desc_pa,
        avail: avail_pa,
        used: used_pa,
        req_pa,
        data_pa,
        status_pa,
    })
}

/// Dung lượng thiết bị theo sector 512 B (0 nếu chưa init).
pub fn capacity_sectors() -> u64 {
    state().capacity
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    state().ready
}

/// Bọc vùng DMA thành split queue với `size` (lấy từ trạng thái, **không** hardcode lại).
fn queue_with(size: u16) -> Result<SplitQueue<'static>, BlkError> {
    SplitQueue::new(&mut page()[..], size).ok_or(BlkError::NotReady)
}

/// Gửi một request và chờ `used` — điểm duy nhất phát request (mọi lối vào đi qua đây để bộ đếm
/// và thứ tự fence không bị bỏ sót).
fn submit(
    kind: u32,
    sector: u64,
    data_in: Option<&[u8; SECTOR_BYTES]>,
    data_out: Option<&mut [u8; SECTOR_BYTES]>,
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    if !state().ready {
        return Err(BlkError::NotReady);
    }
    let dma = dma_page_addr();
    let req_pa = dma + REQ_OFF as u64;
    let data_pa = dma + DATA_OFF as u64;
    let status_pa = dma + STATUS_OFF as u64;
    // Dữ liệu vào (OUT) chép **vào** vùng DMA; FLUSH không có dữ liệu.
    if let Some(bytes) = data_in {
        let dst = &mut page()[DATA_OFF..DATA_OFF + SECTOR_BYTES];
        dst.copy_from_slice(bytes);
    }
    // Header + status khởi tạo (status = 0xff để phát hiện thiết bị không ghi).
    let req = build_request(kind, sector, 1);
    page()[REQ_OFF..REQ_OFF + 16].copy_from_slice(&req.header);
    page()[STATUS_OFF] = 0xff;

    REQUESTS.fetch_add(1, Ordering::SeqCst);
    let size = state().queue_size;
    let notify = state().notify;
    let notify_off = state().notify_off;
    let notify_mul = state().notify_mul;
    let status_index = req.status_index;
    {
        let mut q = queue_with(size)?;
        if !q.set_desc(
            0,
            Desc {
                addr: req_pa,
                len: 16,
                flags: DescFlags::NEXT,
                next: 1,
            },
        ) {
            return Err(BlkError::NotReady);
        }
        if kind != T_FLUSH {
            let flags = if kind == T_IN {
                // device-writable
                DescFlags(DescFlags::NEXT.bits() | DescFlags::WRITE.bits())
            } else {
                DescFlags::NEXT
            };
            if !q.set_desc(
                1,
                Desc {
                    addr: data_pa,
                    // Độ dài descriptor lấy từ **cùng** giá trị đã dựng request — không viết
                    // `SECTOR_BYTES` lần thứ hai (đó đúng là chỗ lỗi v1 #2 sinh ra).
                    len: req.data_len,
                    flags,
                    next: 2,
                },
            ) {
                return Err(BlkError::NotReady);
            }
        }
        if !q.set_desc(
            status_index,
            Desc {
                addr: status_pa,
                len: 1,
                flags: DescFlags::WRITE,
                next: 0,
            },
        ) {
            return Err(BlkError::NotReady);
        }
        // avail -> fence -> idx: `push_avail` ghi ring rồi idx; fence **sau** để thiết bị chỉ
        // thấy idx khi ring đã xong. (Spec §2.7.13.)
        q.push_avail(0);
    }
    barrier();
    write16(notify + notify_off as u64 * notify_mul as u64, 0);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);

    // ---- poll used (một chiều) — offset lấy từ **size thật** của queue.
    let used_idx_addr = dma + VqLayout::new(size).ok_or(BlkError::NotReady)?.used_idx_off as u64;
    let deadline = now().saturating_add(POLL_TIMEOUT_US);
    loop {
        // `asm::read_u16` là đọc `volatile` — compiler **không** hoist khỏi vòng lặp (bẫy kinh
        // điển của poll DMA).
        let idx = read16(used_idx_addr);
        let last = state().last_used;
        if idx != last {
            barrier();
            let element = queue_with(size)?.used_elem(last & (size - 1));
            state().last_used = last.wrapping_add(1);
            if element.0 != 0 {
                ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
                ERRORS.fetch_add(1, Ordering::SeqCst);
                return Err(BlkError::Io);
            }
            let status = page()[STATUS_OFF];
            match status_to_result(status) {
                Ok(()) => {
                    COMPLETIONS.fetch_add(1, Ordering::SeqCst);
                    if let Some(out) = data_out {
                        let src = &page()[DATA_OFF..DATA_OFF + SECTOR_BYTES];
                        out.copy_from_slice(src);
                    }
                    return Ok(());
                }
                Err(error) => {
                    ERRORS.fetch_add(1, Ordering::SeqCst);
                    return Err(error);
                }
            }
        }
        if now() >= deadline {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            ERRORS.fetch_add(1, Ordering::SeqCst);
            return Err(BlkError::Timeout);
        }
        core::hint::spin_loop();
    }
}

/// Đọc một sector, **kiểm dải trước** (không phát request ngoài dung lượng).
pub fn read_sector(
    lba: u32,
    out: &mut [u8; SECTOR_BYTES],
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    if lba as u64 >= capacity_sectors() {
        return Err(BlkError::OutOfRange);
    }
    READS.fetch_add(1, Ordering::SeqCst);
    submit(T_IN, lba as u64, None, Some(out), now)
}

/// Ghi một sector, kiểm dải trước.
pub fn write_sector(
    lba: u32,
    data: &[u8; SECTOR_BYTES],
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    if lba as u64 >= capacity_sectors() {
        return Err(BlkError::OutOfRange);
    }
    WRITES.fetch_add(1, Ordering::SeqCst);
    submit(T_OUT, lba as u64, Some(data), None, now)
}

/// Đẩy cache ghi của thiết bị xuống phương tiện (`VIRTIO_BLK_T_FLUSH`) — **thật**, không no-op.
pub fn flush(now: &mut dyn FnMut() -> u64) -> Result<(), BlkError> {
    if !state().ready {
        return Err(BlkError::NotReady);
    }
    // **Không** gửi lệnh của feature chưa đàm phán.
    if !state().flush_ok {
        return Err(BlkError::Unsupported);
    }
    FLUSHES.fetch_add(1, Ordering::SeqCst);
    submit(T_FLUSH, 0, None, None, now)
}

/// **Chỉ cho ca âm của TAP**: phát request đọc **vượt** kiểm dải để chứng minh thiết bị trả
/// `IOERR` thật (và không treo). Không dùng trong đường FS.
pub fn read_sector_force(
    lba: u32,
    out: &mut [u8; SECTOR_BYTES],
    now: &mut dyn FnMut() -> u64,
) -> Result<(), BlkError> {
    READS.fetch_add(1, Ordering::SeqCst);
    submit(T_IN, lba as u64, None, Some(out), now)
}

/// Địa chỉ ISR (đọc 1 byte để lấy lý do ngắt — bước này chưa dùng ngắt, chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    state().isr
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều request).
pub fn device_status() -> u8 {
    let common = state().common;
    if common == 0 {
        return 0;
    }
    read8(common + C_DEVICE_STATUS)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    let common = state().common;
    if common == 0 {
        return 0;
    }
    read8(common + C_CONFIG_GENERATION)
}

/// Địa chỉ `device_cfg` (chẩn đoán).
pub fn device_cfg_addr() -> u64 {
    state().device_cfg
}

// ---------------------------------------------------------------- bằng chứng sector 0 (thuần)

/// 16 byte đầu → **32 ký tự hex thường** (không dấu cách) — đúng thứ runner grep.
pub fn hex16(bytes: &[u8; 16]) -> [u8; 32] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 16 {
        out[i * 2] = DIGITS[(bytes[i] >> 4) as usize];
        out[i * 2 + 1] = DIGITS[(bytes[i] & 0xf) as usize];
        i += 1;
    }
    out
}

/// 16 byte → ASCII **chỉ khi mọi byte in được** (`0x20..=0x7e`); ngược lại `None`.
///
/// Vì sao không thay byte không in được bằng `'.'`: log là **bằng chứng**, không phải bản xem
/// trước — một chuỗi "trông như chữ" từ đĩa nhị phân sẽ đánh lừa người đọc.
pub fn ascii16(bytes: &[u8; 16]) -> Option<[u8; 16]> {
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        let byte = bytes[i];
        if !(0x20..=0x7e).contains(&byte) {
            return None;
        }
        out[i] = byte;
        i += 1;
    }
    Some(out)
}

/// `true` ⇔ 8 byte đầu khớp [`SECTOR0_MAGIC`] (lát ngắn hơn chữ ký ⇒ `false`, không đọc ngoài).
pub fn has_magic(bytes: &[u8]) -> bool {
    if bytes.len() < SECTOR0_MAGIC.len() {
        return false;
    }
    let mut i = 0;
    while i < SECTOR0_MAGIC.len() {
        if bytes[i] != SECTOR0_MAGIC[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Ba trường bằng chứng của 16 byte đầu sector 0 — **một** giá trị để log và test không lệch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sector0 {
    /// Hex 32 ký tự.
    pub hex: [u8; 32],
    /// ASCII nếu mọi byte in được.
    pub ascii: Option<[u8; 16]>,
    /// `true` ⇔ 8 byte đầu khớp chữ ký.
    pub magic: bool,
}

/// Dựng bằng chứng sector 0 từ 16 byte đầu.
pub fn sector0_evidence(bytes: &[u8; 16]) -> Sector0 {
    Sector0 {
        hex: hex16(bytes),
        ascii: ascii16(bytes),
        magic: has_magic(bytes),
    }
}

const _: () = {
    // `LAYOUT` của queue phải nằm gọn **trước** vùng request trong cùng trang DMA.
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(l) => l,
        None => panic!("queue size khong hop le"),
    };
    assert!(layout.end <= REQ_OFF);
    assert!(REQ_OFF + 16 <= DATA_OFF);
    assert!(DATA_OFF + SECTOR_BYTES <= STATUS_OFF);
    assert!(STATUS_OFF < DMA_PAGE_BYTES);
    // Chữ ký phải là tiền tố của chuỗi 17 byte mà runner ghi vào đĩa tạm.
    assert!(SECTOR0_MAGIC[0] == b'V' && SECTOR0_MAGIC[7] == b'S');
};
