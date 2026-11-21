// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-gpu 2D** (virtio 1.0, virtio-pci modern) trên QEMU `virt` — KT-263 (D1 phần 4).
//!
//! # Đường đi
//!
//! `pci::init_for(VirtioKind::Gpu)` (ECAM của KT-72, **không** nhân đôi code PCI) ⇒ BAR modern +
//! bốn capability ⇒ module này lập trình common config, dựng **một** split virtqueue
//! (`controlq` = queue 0 — §5.7.2; `cursorq` = queue 1 **bỏ trống**, không có con trỏ), rồi chạy
//! chuỗi lệnh 2D:
//!
//! ```text
//! GET_DISPLAY_INFO → RESOURCE_CREATE_2D → RESOURCE_ATTACH_BACKING → SET_SCANOUT
//!   → (vẽ vào framebuffer) → TRANSFER_TO_HOST_2D → RESOURCE_FLUSH
//! ```
//!
//! # Bất biến
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1`; **không** `VIRGIL`/`EDID`/`RESOURCE_BLOB`
//!   — quyết định nằm ở `vifast_virtio::gpu::negotiated_features` (test host), không viết tay hai
//!   chỗ (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **fence** → ghi `avail->idx` →
//!   **fence** → notify (MMIO) → poll `used->idx` → **fence** → đọc `used->elem`.
//! - `used->id` **phải** là head 0 (chuỗi hai descriptor driver vừa publish) — lệch ⇒ đếm
//!   `id_mismatch` + lỗi, không im lặng. `used->len` **phải** ≥ cỡ header response.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); response lỗi ⇒ trả **tên**
//!   lỗi (`Unspec`/`InvalidParameter`…) để TAP in ra lý do đo được; poll quá hạn ⇒ `timeout`.
//!
//! # Framebuffer: **300 frame** liền nhau, truy cập qua **physmap**
//!
//! 640×480×4 = 1.228.800 B = **300 trang 4 KiB** — không nhét vừa `.bss` (trần `.data+.bss`
//! 262.144 B, D2-72 Q5) và cũng không nên: framebuffer là vùng DMA **liền** mà thiết bị đọc thẳng.
//! Driver lấy dải PA [`FB_PHYS_BASE`] (2 MiB đầu **sau** vùng probe của ảnh `ktest`, xem
//! `mmu::PROBE2_PA_D`) và truy cập bằng **alias physmap** `PHYSMAP_BASE + PA` (leaf 2 MiB RW NX đã
//! map sẵn cho toàn bộ RAM) ⇒ không cần map thêm trang nào, không đụng `mmu::init`.
//!
//! `RESOURCE_ATTACH_BACKING` gửi **nhiều** `virtio_gpu_mem_entry` (mỗi mảnh [`ATTACH_CHUNK_BYTES`]
//! = 16 trang) chứ không một mảnh khổng lồ: đây là đường scatter-gather thật của spec, và nó
//! chứng minh driver tính đúng `addr`/`length` cho từng mảnh (mảnh cuối **ngắn hơn**, không tràn).
//!
//! `unsafe` (MMIO + `UnsafeCell` vùng DMA) nằm trong `kernel/vifast-plat-*` (D2-17); không lệnh
//! đặc quyền nào ở U-mode.

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::timer::TIMEBASE_HZ;
use vifast_plat_common::Platform;
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::gpu::{
    decode_config, is_error_response, negotiated_features, resource_ok, resp_error_name, CtrlHdr,
    GpuConfig, MemEntry, Rect, ResourceAttachBacking, ResourceCreate2d, ResourceFlush,
    RespDisplayInfo, SetScanout, TransferToHost2d, CTRL_HDR_BYTES, GPU_CONFIG_BYTES,
    MEM_ENTRY_BYTES, MIN_QUEUE_SIZE, QUEUES_REQUIRED, QUEUE_SIZE, Q_CONTROL,
    RESOURCE_ATTACH_BACKING_BYTES, RESP_DISPLAY_INFO_BYTES, VIRTIO_GPU_CMD_GET_DISPLAY_INFO,
    VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, VIRTIO_GPU_RESP_OK_NODATA,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::mmu::{RAM_BYTES, RAM_PHYS_BASE};
use crate::pci::PciReport;
use crate::Riscv64Virt;

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `controlq` trong trang DMA.
const RING_OFF: usize = 0x000;
/// Offset **vùng lệnh** (header + `mem_entry`) trong trang DMA.
const CMD_OFF: usize = 0x200;
/// Cỡ vùng lệnh — đủ cho `RESOURCE_ATTACH_BACKING` + [`MAX_ATTACH_ENTRIES`] mảnh.
const CMD_BYTES: usize = 0x400;
/// Offset **vùng response** trong trang DMA.
const RESP_OFF: usize = 0x600;
/// Cỡ vùng response — đủ cho `virtio_gpu_resp_display_info` (408 B).
const RESP_BYTES: usize = 0x200;

/// Độ phân giải framebuffer driver tạo (pixel). 640×480 là mode mặc định của console QEMU
/// (`-display none` vẫn có console 640×480 — đo được bằng `screendump` trước khi viết driver).
pub const FB_WIDTH: u32 = 640;
/// Cao framebuffer (pixel).
pub const FB_HEIGHT: u32 = 480;
/// Số byte framebuffer = 640 × 480 × 4.
pub const FB_BYTES: usize = 640 * 480 * 4;
/// Số byte **mỗi hàng** của resource = `FB_WIDTH × 4` (pixman của QEMU dùng đúng stride này cho
/// ảnh 640 px 32 bpp; `TRANSFER_TO_HOST_2D.offset` tính theo nó).
pub const FB_STRIDE: usize = FB_WIDTH as usize * 4;
/// Số trang 4 KiB framebuffer chiếm = 300.
pub const FB_PAGES: usize = FB_BYTES / 4096;
/// Base **vật lý** của framebuffer: 2 MiB đầu **sau** vùng probe của ảnh `ktest`
/// (`mmu::PROBE2_PA_D + 2 MiB`), trong RAM `-m 128M`, căn 2 MiB.
pub const FB_PHYS_BASE: u64 = 0x8100_0000;
/// Cỡ **một** mảnh backing gửi trong `RESOURCE_ATTACH_BACKING` = 16 trang 4 KiB.
pub const ATTACH_CHUNK_BYTES: usize = 64 * 1024;
/// Số `virtio_gpu_mem_entry` driver gửi = ⌈[`FB_BYTES`] / [`ATTACH_CHUNK_BYTES`]⌉ = 19.
pub const MAX_ATTACH_ENTRIES: usize = FB_BYTES.div_ceil(ATTACH_CHUNK_BYTES);

/// `resource_id` driver dùng (một resource duy nhất — màn hình chính).
pub const RESOURCE_ID: u32 = 1;
/// `scanout_id` driver dùng (QEMU `-display none`: scanout 0).
pub const SCANOUT_ID: u32 = 0;
/// Thời gian chờ **tối đa** cho một lệnh (tick timebase) — 2 s ảo.
///
/// Lệnh 2D của QEMU xong trong micro-giây; 2 s là dư rất nhiều. Hết hạn ⇒ `Err("timeout")` +
/// TAP đỏ (fail-closed, **không** treo).
pub const GPU_WAIT_TICKS: u64 = 2 * TIMEBASE_HZ;

// `const` assert: framebuffer phải nằm **trong** RAM, **sau** vùng probe của ảnh `ktest`, căn
// trang, và số trang phải khớp `FB_BYTES` — lệch một hằng là **không biên dịch được** thay vì
// DMA ra ngoài RAM (thiết bị ghi vào chỗ khác).
const _: () = assert!(FB_BYTES == FB_WIDTH as usize * FB_HEIGHT as usize * 4);
const _: () = assert!(FB_PAGES * 4096 == FB_BYTES);
const _: () = assert!(FB_PHYS_BASE % 4096 == 0);
const _: () = assert!(FB_PHYS_BASE >= 0x8100_0000); // sau PROBE2_PA_D + 2 MiB
const _: () = assert!(FB_PHYS_BASE + FB_BYTES as u64 <= RAM_PHYS_BASE + RAM_BYTES);
const _: () = assert!(MAX_ATTACH_ENTRIES == 19);
const _: () = assert!(RING_OFF + 0x200 <= CMD_OFF);
const _: () = assert!(CMD_OFF + CMD_BYTES <= RESP_OFF);
const _: () = assert!(RESP_OFF + RESP_BYTES <= DMA_PAGE_BYTES);
const _: () = assert!(RESP_BYTES >= RESP_DISPLAY_INFO_BYTES);
const _: () =
    assert!(RESOURCE_ATTACH_BACKING_BYTES + MAX_ATTACH_ENTRIES * MEM_ENTRY_BYTES <= CMD_BYTES);

// ---------------------------------------------------------------- thanh ghi common config
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

// ---------------------------------------------------------------- bộ đếm (an toàn, atomic)
static COMMANDS: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);
/// Số lần thiết bị **từ chối** một lệnh trong **ca âm đối chứng** (không phải lỗi driver).
static REJECTS: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm để in evidence (đọc một lần, không tách rời từng số).
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Số lệnh đã gửi và nhận response.
    pub commands: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lỗi driver (response lỗi, `used->len`/`used->id` vô lý).
    pub errors: u64,
    /// Số lần poll quá hạn (phải 0 ở đường thành công).
    pub timeouts: u64,
    /// Số lần `used->id` lệch head đã publish (phải 0).
    pub id_mismatch: u64,
    /// Số lần thiết bị **từ chối** lệnh trong ca âm đối chứng (`probe_bogus_scanout`) — **không**
    /// tính là lỗi driver; đây là bằng chứng thiết bị thật sự kiểm `resource_id`.
    pub rejects: u64,
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    Stats {
        commands: COMMANDS.load(Ordering::SeqCst),
        notifies: NOTIFIES.load(Ordering::SeqCst),
        errors: ERRORS.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
        id_mismatch: ID_MISMATCH.load(Ordering::SeqCst),
        rejects: REJECTS.load(Ordering::SeqCst),
    }
}

/// Trạng thái driver — gói trong `UnsafeCell` + `Sync` thủ công: ảnh test chạy **một hart**,
/// driver tự tuần tự hoá (một lệnh một lúc), không có truy cập song song.
struct State {
    ready: bool,
    common: u64,
    notify: u64,
    isr: u64,
    device_cfg: u64,
    notify_mul: u32,
    /// `queue_notify_off` của queue `controlq`.
    notify_off: u16,
    /// Cỡ queue thật của `controlq` (đọc lại từ thiết bị).
    queue_size: u16,
    /// `used->idx` đã xử lý.
    last_used: u16,
    /// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng.
    published: u16,
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
    last_used: 0,
    published: 0,
};

struct Cell<T>(UnsafeCell<T>);
// SAFETY: một hart, truy cập tuần tự trong module này (không chia sẻ giữa các hart).
unsafe impl<T> Sync for Cell<T> {}

static STATE: Cell<State> = Cell(UnsafeCell::new(INIT_STATE));

#[repr(align(4096))]
struct DmaPage([u8; DMA_PAGE_BYTES]);
struct DmaCell(UnsafeCell<DmaPage>);
// SAFETY: như `STATE` — vùng DMA chỉ driver này chạm; thiết bị chạm qua PA (DMA) theo hợp đồng.
unsafe impl Sync for DmaCell {}
static DMA: DmaCell = DmaCell(UnsafeCell::new(DmaPage([0u8; DMA_PAGE_BYTES])));

/// Địa chỉ của trang DMA — identity map nên `VA == PA` (TAP kiểm bằng `walk_sw`).
pub fn dma_page_addr() -> u64 {
    DMA.0.get() as *const _ as u64
}

fn state() -> &'static mut State {
    // SAFETY: một hart + tuần tự (xem `Cell`).
    unsafe { &mut *STATE.0.get() }
}

fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    // SAFETY: một hart + tuần tự; vùng này không ai khác (trừ DMA của thiết bị) chạm.
    unsafe { &mut (*DMA.0.get()).0 }
}

// ---------------------------------------------------------------- MMIO
#[inline]
fn read8(addr: u64) -> u8 {
    // SAFETY: `addr` nằm trong BAR đã gán/map Device RW (kiểm bằng walk trong TAP).
    unsafe { read_volatile(addr as *const u8) }
}
#[inline]
fn read16(addr: u64) -> u16 {
    // SAFETY: như `read8`; bán kính 16 bit căn 2.
    unsafe { read_volatile(addr as *const u16) }
}
#[inline]
fn read32(addr: u64) -> u32 {
    // SAFETY: như `read8`; căn 4.
    unsafe { read_volatile(addr as *const u32) }
}
#[inline]
fn write8(addr: u64, value: u8) {
    // SAFETY: như `read8`; ghi thanh ghi thiết bị đã map.
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

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13: driver phải
/// có barrier trước notify; thiếu ⇒ thiết bị có thể đọc `avail` cũ).
#[inline]
fn fence() {
    // SAFETY: `fence` chỉ là hàng rào thứ tự, không truy cập địa chỉ.
    unsafe { core::arch::asm!("fence rw, rw", options(nostack, preserves_flags)) };
}

// ---------------------------------------------------------------- báo cáo khởi tạo
/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct GpuReport {
    /// Vendor/device (từ PCI config).
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// BAR driver dùng + địa chỉ đã gán + cỡ đo được.
    pub bar_index: u8,
    /// Địa chỉ BAR đã gán.
    pub bar_base: u64,
    /// Cỡ BAR.
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
    /// `queue_notify_off` của `controlq`.
    pub notify_off: u16,
    /// Địa chỉ cửa thông báo `controlq` = notify + off * multiplier.
    pub notify_addr: u64,
    /// `num_queues` thiết bị báo (QEMU: 2 — `controlq` + `cursorq`).
    pub num_queues: u16,
    /// Cỡ queue tối đa thiết bị cho phép.
    pub queue_max: u16,
    /// Cỡ queue driver đặt.
    pub queue_size: u16,
    /// Feature thiết bị (64 bit, đọc hai trang).
    pub device_features: u64,
    /// Feature driver đã ghi (đọc lại).
    pub driver_features: u64,
    /// `device_status` sau DRIVER_OK.
    pub status: u8,
    /// `virtio_gpu_config` đọc từ device config.
    pub config: GpuConfig,
    /// PA bảng descriptor.
    pub desc: u64,
    /// PA khối `avail`.
    pub avail: u64,
    /// PA khối `used`.
    pub used: u64,
    /// PA vùng lệnh.
    pub cmd_pa: u64,
    /// PA vùng response.
    pub resp_pa: u64,
    /// PA framebuffer (đầu dải 300 frame).
    pub fb_pa: u64,
    /// Số byte framebuffer.
    pub fb_bytes: usize,
    /// Số trang 4 KiB framebuffer.
    pub fb_pages: usize,
}

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
pub fn init(pci: &PciReport) -> Result<GpuReport, &'static str> {
    let caps = pci.caps;
    let bar = pci.bar_assigned;
    let common = bar + caps.common.offset as u64;
    let notify = bar + caps.notify.offset as u64;
    let device_cfg = bar + caps.device.offset as u64;
    let isr = bar + caps.isr.offset as u64;
    let notify_mul = caps.notify_off_multiplier;
    let layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    if RING_OFF + layout.total_bytes() > CMD_OFF {
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
    // ---- 4) đàm phán: **chỉ** VERSION_1 (quyết định ở hàm thuần `negotiated_features`).
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
    // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
    write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
    let status = read8(common + C_DEVICE_STATUS);
    if !features_ok(status) {
        return Err("features_ok");
    }
    // ---- 6) queue `controlq` = 0 (queue `cursorq` = 1 **không** dùng — không có con trỏ)
    let num_queues = read16(common + C_NUM_QUEUES);
    if num_queues < QUEUES_REQUIRED {
        return Err("queues");
    }
    write16(common + C_QUEUE_SELECT, Q_CONTROL);
    let queue_max = read16(common + C_QUEUE_SIZE);
    if queue_max < MIN_QUEUE_SIZE || !queue_max.is_power_of_two() {
        return Err("queue_size");
    }
    let queue_size = QUEUE_SIZE.min(queue_max);
    write16(common + C_QUEUE_SIZE, queue_size);
    if read16(common + C_QUEUE_SIZE) != queue_size {
        return Err("queue_size");
    }
    let layout = VqLayout::new(queue_size).ok_or("layout")?;
    let dma = dma_page_addr();
    let base = dma + RING_OFF as u64;
    write64(common + C_QUEUE_DESC, base + layout.desc_off as u64);
    write64(common + C_QUEUE_DRIVER, base + layout.avail_off as u64);
    write64(common + C_QUEUE_DEVICE, base + layout.used_off as u64);
    fence();
    write16(common + C_QUEUE_ENABLE, 1);
    if read16(common + C_QUEUE_ENABLE) != 1 {
        return Err("queue_enable");
    }
    let notify_off = read16(common + C_QUEUE_NOTIFY_OFF);
    let notify_addr = notify + notify_off as u64 * notify_mul as u64;
    // ---- 7) DRIVER_OK
    write8(
        common + C_DEVICE_STATUS,
        ACK | DRIVER | FEATURES_OK | DRIVER_OK,
    );
    let status = read8(common + C_DEVICE_STATUS);
    if !status_sequence_ok(status) {
        return Err("driver_ok");
    }
    // ---- 8) device config: `virtio_gpu_config` (num_scanouts là số đo **từ thiết bị**)
    let mut raw = [0u8; GPU_CONFIG_BYTES];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = read8(device_cfg + index as u64);
    }
    let config = decode_config(&raw).ok_or("config")?;
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
        s.last_used = 0;
        s.published = 0;
    }
    Ok(GpuReport {
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
        notify_off,
        notify_addr,
        num_queues,
        queue_max,
        queue_size,
        device_features,
        driver_features,
        status,
        config,
        desc: base + layout.desc_off as u64,
        avail: base + layout.avail_off as u64,
        used: base + layout.used_off as u64,
        cmd_pa: dma + CMD_OFF as u64,
        resp_pa: dma + RESP_OFF as u64,
        fb_pa: FB_PHYS_BASE,
        fb_bytes: FB_BYTES,
        fb_pages: FB_PAGES,
    })
}

/// Bọc ring của `controlq` thành split queue với cỡ **thật** của nó.
fn queue_with(size: u16) -> Result<SplitQueue<'static>, &'static str> {
    if size < MIN_QUEUE_SIZE {
        return Err("not_ready");
    }
    let layout = VqLayout::new(size).ok_or("not_ready")?;
    SplitQueue::new(&mut page()[RING_OFF..RING_OFF + layout.total_bytes()], size).ok_or("not_ready")
}

/// Gửi **một** lệnh đã nằm sẵn trong vùng lệnh, chờ response, trả `used->len`.
///
/// Chuỗi hai descriptor: `[cmd device-readable] → [resp device-writable]` (§2.6.4). Head luôn là
/// 0 vì driver gửi **tuần tự** và chỉ publish lại sau khi lệnh trước hoàn tất.
fn submit(cmd_len: usize, resp_len: usize) -> Result<u32, &'static str> {
    let size = state().queue_size;
    if size < MIN_QUEUE_SIZE {
        return Err("not_ready");
    }
    if cmd_len == 0 || cmd_len > CMD_BYTES || resp_len == 0 || resp_len > RESP_BYTES {
        return Err("cmd_len");
    }
    let dma = dma_page_addr();
    {
        let mut queue = queue_with(size)?;
        if !queue.set_desc(
            0,
            Desc {
                addr: dma + CMD_OFF as u64,
                len: cmd_len as u32,
                flags: DescFlags::NEXT,
                next: 1,
            },
        ) {
            return Err("desc");
        }
        if !queue.set_desc(
            1,
            Desc {
                addr: dma + RESP_OFF as u64,
                len: resp_len as u32,
                flags: DescFlags::WRITE,
                next: 0,
            },
        ) {
            return Err("desc");
        }
        queue.push_avail(0);
    }
    fence();
    let notify = state().notify;
    let notify_off = state().notify_off;
    let notify_mul = state().notify_mul;
    write16(notify + notify_off as u64 * notify_mul as u64, Q_CONTROL);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
    {
        let s = state();
        s.published = s.published.wrapping_add(1);
    }
    let layout = VqLayout::new(size).ok_or("layout")?;
    let used_idx_addr = dma + RING_OFF as u64 + layout.used_idx_off as u64;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(GPU_WAIT_TICKS);
    loop {
        // SAFETY: `used_idx_addr` nằm trong trang DMA đã bàn giao; đọc volatile để compiler
        // **không** hoist khỏi vòng lặp (bẫy kinh điển của poll DMA).
        let idx = unsafe { read_volatile(used_idx_addr as *const u16) };
        let last = state().last_used;
        if idx != last {
            fence();
            let (id, len) = {
                let queue = queue_with(size)?;
                queue.used_elem(last & (size - 1))
            };
            state().last_used = last.wrapping_add(1);
            if id != 0 {
                ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
                ERRORS.fetch_add(1, Ordering::SeqCst);
                return Err("used_id");
            }
            COMMANDS.fetch_add(1, Ordering::SeqCst);
            return Ok(len);
        }
        if Riscv64Virt.now_ticks() > deadline {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            return Err("timeout");
        }
        core::hint::spin_loop();
    }
}

/// Đọc `type` của response hiện tại trong vùng response.
fn resp_type() -> Option<u32> {
    CtrlHdr::parse(&page()[RESP_OFF..RESP_OFF + CTRL_HDR_BYTES]).map(|hdr| hdr.type_)
}

/// Đòi response là `RESP_OK_NODATA`; response lỗi ⇒ trả **tên** lỗi (D2-102).
fn check_nodata(used_len: u32) -> Result<(), &'static str> {
    if used_len < CTRL_HDR_BYTES as u32 {
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err("resp_len");
    }
    let type_ = resp_type().ok_or("resp_hdr")?;
    if type_ == VIRTIO_GPU_RESP_OK_NODATA {
        return Ok(());
    }
    ERRORS.fetch_add(1, Ordering::SeqCst);
    if is_error_response(type_) {
        return Err(resp_error_name(type_));
    }
    Err("resp_type")
}

/// `GET_DISPLAY_INFO` — đọc mode hiện tại của mọi scanout.
///
/// Trước `SET_SCANOUT` đầu tiên, QEMU trả `enabled = 0` và `width/height = 0` (mode **chưa** được
/// đặt); sau `SET_SCANOUT` nó trả đúng cỡ resource. Đó là phép đo thật của TAP `gpu scanout`.
pub fn get_display_info() -> Result<RespDisplayInfo, &'static str> {
    let cmd = CtrlHdr::new(VIRTIO_GPU_CMD_GET_DISPLAY_INFO).encode();
    page()[CMD_OFF..CMD_OFF + cmd.len()].copy_from_slice(&cmd);
    let used = submit(cmd.len(), RESP_DISPLAY_INFO_BYTES)?;
    if used < RESP_DISPLAY_INFO_BYTES as u32 {
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return Err("resp_len");
    }
    RespDisplayInfo::parse(&page()[RESP_OFF..RESP_OFF + RESP_DISPLAY_INFO_BYTES])
        .ok_or("resp_display")
}

/// `RESOURCE_CREATE_2D` — tạo resource 2D `width × height`, format `B8G8R8A8_UNORM`.
pub fn create_resource(width: u32, height: u32) -> Result<(), &'static str> {
    if !resource_ok(width, height) {
        return Err("resource");
    }
    let cmd = ResourceCreate2d::new(RESOURCE_ID, VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, width, height)
        .encode();
    page()[CMD_OFF..CMD_OFF + cmd.len()].copy_from_slice(&cmd);
    let used = submit(cmd.len(), CTRL_HDR_BYTES)?;
    check_nodata(used)
}

/// `RESOURCE_ATTACH_BACKING` — gắn [`MAX_ATTACH_ENTRIES`] mảnh của dải [`FB_PHYS_BASE`].
///
/// Trả số mảnh đã gửi. Mảnh cuối **ngắn hơn** [`ATTACH_CHUNK_BYTES`] (1.228.800 = 18 × 65.536 +
/// 49.152) — tính bằng `min`, không giả định chia hết.
pub fn attach_backing() -> Result<u32, &'static str> {
    let entries = MAX_ATTACH_ENTRIES;
    let header = ResourceAttachBacking::new(RESOURCE_ID, entries as u32).encode();
    let mut at = CMD_OFF;
    page()[at..at + header.len()].copy_from_slice(&header);
    at += header.len();
    let mut index = 0;
    while index < entries {
        let offset = index * ATTACH_CHUNK_BYTES;
        let length = (FB_BYTES - offset).min(ATTACH_CHUNK_BYTES);
        let entry = MemEntry::new(FB_PHYS_BASE + offset as u64, length as u32).encode();
        page()[at..at + MEM_ENTRY_BYTES].copy_from_slice(&entry);
        at += MEM_ENTRY_BYTES;
        index += 1;
    }
    let used = submit(at - CMD_OFF, CTRL_HDR_BYTES)?;
    check_nodata(used)?;
    Ok(entries as u32)
}

/// `SET_SCANOUT` — gắn [`RESOURCE_ID`] vào [`SCANOUT_ID`], nguồn là `rect`.
pub fn set_scanout(rect: Rect) -> Result<(), &'static str> {
    set_scanout_raw(SCANOUT_ID, RESOURCE_ID, rect)
}

/// `SET_SCANOUT` với `(scanout_id, resource_id)` tuỳ ý — **riêng tư**: chỉ dùng cho ca âm đối
/// chứng bên dưới, không mở đường cho người gọi gắn resource lạ.
fn set_scanout_raw(scanout_id: u32, resource_id: u32, rect: Rect) -> Result<(), &'static str> {
    let cmd = SetScanout::new(scanout_id, resource_id, rect).encode();
    page()[CMD_OFF..CMD_OFF + cmd.len()].copy_from_slice(&cmd);
    let used = submit(cmd.len(), CTRL_HDR_BYTES)?;
    check_nodata(used)
}

/// **Ca âm đối chứng** của TAP `gpu scanout`: gửi `SET_SCANOUT` với `bogus_id` (**không** tồn tại)
/// và trả **tên lỗi** thiết bị báo (`InvalidResourceId` nếu thiết bị kiểm đúng).
///
/// Vì sao cần: `GET_DISPLAY_INFO` của QEMU trả mode **của host** (`req_state`), không phải scanout
/// của khách — nó **không** đổi khi `SET_SCANOUT` (đo được: `1280×800` trước và sau). Nếu thiết bị
/// chấp nhận cả `resource_id` rác thì "SET_SCANOUT thành công" **không** chứng minh được gì; ca âm
/// này chứng minh thiết bị thật sự kiểm id. Bằng chứng **thật** rằng đã vẽ là điểm ảnh trong PPM
/// của runner.
///
/// Trả `Ok("accepted")` khi thiết bị **chấp nhận** id rác — người gọi phải coi đó là TAP đỏ.
pub fn probe_bogus_scanout(bogus_id: u32, rect: Rect) -> Result<&'static str, &'static str> {
    let cmd = SetScanout::new(SCANOUT_ID, bogus_id, rect).encode();
    page()[CMD_OFF..CMD_OFF + cmd.len()].copy_from_slice(&cmd);
    let used = submit(cmd.len(), CTRL_HDR_BYTES)?;
    if used < CTRL_HDR_BYTES as u32 {
        return Err("resp_len");
    }
    let type_ = resp_type().ok_or("resp_hdr")?;
    if is_error_response(type_) {
        REJECTS.fetch_add(1, Ordering::SeqCst);
        return Ok(resp_error_name(type_));
    }
    Ok("accepted")
}

/// `TRANSFER_TO_HOST_2D` — chép `rect` từ backing (guest RAM) sang resource của thiết bị.
///
/// `offset` **không** phải 0 khi `rect` lệch gốc: thiết bị đọc từng hàng tại `offset + stride*h`
/// rồi ghi vào `(rect.y + h) * stride + rect.x * 4` (QEMU `hw/display/virtio-gpu.c`). Vì vậy
/// `offset` = [`Rect::byte_offset`] của `rect` trong backing. Gửi 0 cho rect lệch gốc ⇒ thiết bị
/// chép hàng 0 của framebuffer vào `rect.y` ⇒ nội dung hiện lệch đúng `(rect.x, rect.y)` pixel —
/// lỗi KT-271 đã **đo được** trên PPM (`ViFastOs` hiện ở `(16,16)` thay vì `(8,8)`) trước khi vá.
pub fn transfer_to_host_2d(rect: Rect) -> Result<(), &'static str> {
    let cmd = TransferToHost2d::new(RESOURCE_ID, rect, rect.byte_offset(FB_STRIDE)).encode();
    page()[CMD_OFF..CMD_OFF + cmd.len()].copy_from_slice(&cmd);
    let used = submit(cmd.len(), CTRL_HDR_BYTES)?;
    check_nodata(used)
}

/// `RESOURCE_FLUSH` — bảo thiết bị đẩy `rect` của resource ra màn hình.
pub fn resource_flush(rect: Rect) -> Result<(), &'static str> {
    let cmd = ResourceFlush::new(RESOURCE_ID, rect).encode();
    page()[CMD_OFF..CMD_OFF + cmd.len()].copy_from_slice(&cmd);
    let used = submit(cmd.len(), CTRL_HDR_BYTES)?;
    check_nodata(used)
}

/// Flush **một vùng đã đổi**: `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH` — đúng **hai** lệnh.
///
/// Đây là đường flush theo lô của console (KT-271): console tích vùng bẩn rồi gọi hàm này **một**
/// lần cho cả khối chữ, thay vì hai lệnh cho **mỗi** ký tự. Số lệnh là số **đo được** — bộ đếm
/// [`stats`]`.commands` tăng đúng 2 cho mỗi lần gọi, và ảnh `ktest-gpu` in delta đó ra TAP.
///
/// Không gộp hai lệnh vào một descriptor: spec §5.7.6.5/§5.7.6.6 định nghĩa chúng là hai lệnh
/// riêng, và `submit` của driver gửi **một** lệnh mỗi vòng (head 0) — gộp sẽ phải đổi cả ring.
pub fn flush_rect(rect: Rect) -> Result<(), &'static str> {
    transfer_to_host_2d(rect)?;
    resource_flush(rect)
}

/// Framebuffer driver vẽ — **alias physmap** của dải PA [`FB_PHYS_BASE`] (leaf 2 MiB RW NX đã map
/// sẵn cho toàn bộ RAM), nên `VA = PHYSMAP_BASE + PA` và thiết bị DMA đọc đúng dải PA đó.
pub fn framebuffer() -> &'static mut [u8] {
    let va = crate::mmu::PHYSMAP_BASE + FB_PHYS_BASE;
    // SAFETY: `va` là alias physmap của RAM `[FB_PHYS_BASE, FB_PHYS_BASE + FB_BYTES)` — đã map RW
    // NX bởi `mmu::init` (leaf 2 MiB), nằm ngoài image/arena/vùng probe; một hart, tuần tự.
    unsafe { core::slice::from_raw_parts_mut(va as *mut u8, FB_BYTES) }
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    state().ready
}

/// Địa chỉ ISR (KT-263 chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    state().isr
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều lệnh).
pub fn device_status() -> u8 {
    let common = state().common;
    if common == 0 {
        return 0;
    }
    read8(common + C_DEVICE_STATUS)
}

/// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng (`published / queue_size`).
pub fn published() -> u16 {
    state().published
}

const _: () = {
    // Ring + vùng lệnh + vùng response phải nằm gọn trong **một** trang DMA 4 KiB.
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(l) => l,
        None => panic!("queue size khong hop le"),
    };
    assert!(RING_OFF + layout.end <= CMD_OFF);
    assert!(CMD_OFF + CMD_BYTES <= RESP_OFF);
    assert!(RESP_OFF + RESP_BYTES <= DMA_PAGE_BYTES);
};
