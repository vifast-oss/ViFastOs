// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Driver `virtio-gpu` 2D** — phần **thuần** dùng chung 3 arch (KT-337, D2-231/D2-239).
//!
//! # Vì sao tệp này tồn tại
//!
//! Trước KT-337, `kernel/vifast-plat-{riscv64,aarch64}/src/virtio_gpu.rs` là **hai bản sao** của
//! cùng một driver: status → feature → queue `controlq` → descriptor → notify → poll → sáu lệnh
//! 2D (`GET_DISPLAY_INFO`, `RESOURCE_CREATE_2D`, `RESOURCE_ATTACH_BACKING`, `SET_SCANOUT`,
//! `TRANSFER_TO_HOST_2D`, `RESOURCE_FLUSH`). Chúng chỉ khác **cách chạm phần cứng** (MMIO, PA,
//! cache maintenance của aarch64) và **đồng hồ**. Tệp này giữ toàn bộ quyết định giao thức; ba
//! `vifast-plat-*` chỉ còn adapter `unsafe` tối giản (D2-17):
//!
//! ```text
//! vifast_virtio::gpudev::Gpu (tệp này)
//!         │  MmioBus (thanh ghi BAR), DmaPool (trang DMA + sync_for_device/cpu), Clock
//!         ▼
//! kernel/vifast-plat-*  — adapter MMIO/DMA/cache (riscv64, aarch64, x86_64)
//! ```
//!
//! # Bất biến giữ nguyên từ KT-263/KT-271/KT-279
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1` — quyết định ở
//!   [`crate::gpu::negotiated_features`] (test host), không viết tay hai chỗ.
//! - Thứ tự spec §2.7: ghi descriptor → `avail->ring`/`idx` → **bàn giao DMA** (aarch64: `dc
//!   civac` + `dsb`) → **fence** → notify → poll `used->idx` (bỏ cache từng dòng) → fence → đọc
//!   `used->elem`.
//! - `used->id` **phải** là head 0; lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - `TRANSFER_TO_HOST_2D.offset` = [`Rect::byte_offset`] của rect trong backing; đặt 0 cho rect
//!   lệch gốc là **lỗi KT-271** — PPM sẽ lệch đúng `(rect.x, rect.y)` pixel.
//! - Fail-closed: thiếu thiết bị/feature/queue ⇒ `Err(stage)`; không PANIC, không treo (có
//!   [`Clock`] trần).
//!
//! `unsafe` **không** có ở đây (`#![forbid(unsafe_code)]` của crate); mọi truy cập thanh ghi/DMA
//! đi qua trait.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::blkdev::{Clock, DEFAULT_SPIN_LIMIT};
pub use crate::dma::{DmaBuf, DmaPool};
use crate::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use crate::gpu::{
    decode_config, is_error_response, negotiated_features, resource_ok, resp_error_name, CtrlHdr,
    GpuConfig, MemEntry, Rect, ResourceAttachBacking, ResourceCreate2d, ResourceFlush,
    RespDisplayInfo, SetScanout, TransferToHost2d, BYTES_PER_PIXEL, CTRL_HDR_BYTES,
    GPU_CONFIG_BYTES, MEM_ENTRY_BYTES, MIN_QUEUE_SIZE, QUEUES_REQUIRED, Q_CONTROL,
    RESOURCE_ATTACH_BACKING_BYTES, RESP_DISPLAY_INFO_BYTES, VIRTIO_GPU_CMD_GET_DISPLAY_INFO,
    VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, VIRTIO_GPU_RESP_OK_NODATA,
};
pub use crate::mmio::MmioBus;
use crate::pcibus::PciReport;
use crate::queue::{Desc, DescFlags, SplitQueue, VqLayout};

/// Cỡ queue `controlq` driver đặt (khớp [`crate::gpu::QUEUE_SIZE`]).
pub const QUEUE_SIZE: u16 = crate::gpu::QUEUE_SIZE;
/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `controlq` trong trang DMA.
pub const RING_OFF: usize = 0x000;
/// Offset **vùng lệnh** (header + `mem_entry`) trong trang DMA.
pub const CMD_OFF: usize = 0x200;
/// Cỡ vùng lệnh — đủ cho `RESOURCE_ATTACH_BACKING` + [`MAX_ATTACH_ENTRIES`] mảnh.
pub const CMD_BYTES: usize = 0x400;
/// Offset **vùng response** trong trang DMA.
pub const RESP_OFF: usize = 0x600;
/// Cỡ vùng response — đủ cho `virtio_gpu_resp_display_info` (408 B).
pub const RESP_BYTES: usize = 0x200;

/// Độ phân giải framebuffer driver tạo (pixel). 640×480 là mode mặc định của console QEMU
/// (`-display none` vẫn có console 640×480 — đo được bằng `screendump` trước khi viết driver).
pub const FB_WIDTH: u32 = 640;
/// Cao framebuffer (pixel).
pub const FB_HEIGHT: u32 = 480;
/// Số byte framebuffer = 640 × 480 × 4.
pub const FB_BYTES: usize = 640 * 480 * 4;
/// Số byte **mỗi hàng** của resource = `FB_WIDTH × 4` (pixman của QEMU dùng đúng stride này cho
/// ảnh 640 px 32 bpp; `TRANSFER_TO_HOST_2D.offset` tính theo nó).
pub const FB_STRIDE: usize = FB_WIDTH as usize * BYTES_PER_PIXEL;
/// Số trang 4 KiB framebuffer chiếm = 300.
pub const FB_PAGES: usize = FB_BYTES / 4096;
/// Cỡ **một** mảnh backing gửi trong `RESOURCE_ATTACH_BACKING` = 16 trang 4 KiB.
pub const ATTACH_CHUNK_BYTES: usize = 64 * 1024;
/// Số `virtio_gpu_mem_entry` driver gửi = ⌈[`FB_BYTES`] / [`ATTACH_CHUNK_BYTES`]⌉ = 19.
pub const MAX_ATTACH_ENTRIES: usize = FB_BYTES.div_ceil(ATTACH_CHUNK_BYTES);

/// `resource_id` driver dùng (một resource duy nhất — màn hình chính).
pub const RESOURCE_ID: u32 = 1;
/// `scanout_id` driver dùng (QEMU `-display none`: scanout 0).
pub const SCANOUT_ID: u32 = 0;

// Bất biến biên dịch: ring + vùng lệnh + vùng response phải nằm gọn trong **một** trang DMA.
const _: () = assert!(FB_BYTES == FB_WIDTH as usize * FB_HEIGHT as usize * BYTES_PER_PIXEL);
const _: () = assert!(FB_PAGES * 4096 == FB_BYTES);
const _: () = assert!(MAX_ATTACH_ENTRIES == 19);
const _: () = assert!(FB_STRIDE == FB_WIDTH as usize * 4);
const _: () = {
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2 trong [2, 256]"),
    };
    assert!(RING_OFF + layout.end <= CMD_OFF);
    assert!(CMD_OFF + CMD_BYTES <= RESP_OFF);
    assert!(RESP_OFF + RESP_BYTES <= DMA_PAGE_BYTES);
    assert!(RESP_BYTES >= RESP_DISPLAY_INFO_BYTES);
    assert!(RESOURCE_ATTACH_BACKING_BYTES + MAX_ATTACH_ENTRIES * MEM_ENTRY_BYTES <= CMD_BYTES);
};

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

/// Bộ đếm của driver — **ảnh chụp** đọc một lần (D2-102: in từ dữ liệu, không chép số).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// Số lần thiết bị **từ chối** lệnh trong **ca âm đối chứng** (không phải lỗi driver).
    pub rejects: u64,
}

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
    /// PA trang DMA.
    pub dma_pa: u64,
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
    /// PA framebuffer (đầu dải backing).
    pub fb_pa: u64,
    /// Số byte framebuffer.
    pub fb_bytes: usize,
    /// Số trang 4 KiB framebuffer.
    pub fb_pages: usize,
    /// Số byte mỗi hàng framebuffer.
    pub fb_stride: usize,
}

/// Trạng thái driver — mọi trường đổi được là `Atomic*` để ảnh boot khai `static GPU: Gpu` mà
/// **không** cần `UnsafeCell`/`unsafe` ở platform.
pub struct Gpu {
    common: AtomicU64,
    notify: AtomicU64,
    device_cfg: AtomicU64,
    isr: AtomicU64,
    notify_mul: AtomicU64,
    notify_off: AtomicU64,
    queue_size: AtomicU64,
    dma_off: AtomicU64,
    dma_pa: AtomicU64,
    fb_pa: AtomicU64,
    last_used: AtomicU64,
    published: AtomicU64,
    ready: AtomicBool,
    commands: AtomicU64,
    notifies: AtomicU64,
    errors: AtomicU64,
    timeouts: AtomicU64,
    id_mismatch: AtomicU64,
    rejects: AtomicU64,
}

impl Default for Gpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Gpu {
    /// Trạng thái rỗng (chưa init).
    pub const fn new() -> Self {
        Self {
            common: AtomicU64::new(0),
            notify: AtomicU64::new(0),
            device_cfg: AtomicU64::new(0),
            isr: AtomicU64::new(0),
            notify_mul: AtomicU64::new(0),
            notify_off: AtomicU64::new(0),
            queue_size: AtomicU64::new(0),
            dma_off: AtomicU64::new(0),
            dma_pa: AtomicU64::new(0),
            fb_pa: AtomicU64::new(0),
            last_used: AtomicU64::new(0),
            published: AtomicU64::new(0),
            ready: AtomicBool::new(false),
            commands: AtomicU64::new(0),
            notifies: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            id_mismatch: AtomicU64::new(0),
            rejects: AtomicU64::new(0),
        }
    }

    /// Ảnh chụp bộ đếm.
    pub fn stats(&self) -> Stats {
        Stats {
            commands: self.commands.load(Ordering::Relaxed),
            notifies: self.notifies.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            id_mismatch: self.id_mismatch.load(Ordering::Relaxed),
            rejects: self.rejects.load(Ordering::Relaxed),
        }
    }

    /// `true` ⇔ driver đã init thành công.
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Địa chỉ ISR (chẩn đoán; K6 chưa dùng ngắt).
    pub fn isr_addr(&self) -> u64 {
        self.isr.load(Ordering::Relaxed)
    }

    /// Địa chỉ `device_cfg` (chẩn đoán).
    pub fn device_cfg_addr(&self) -> u64 {
        self.device_cfg.load(Ordering::Relaxed)
    }

    /// PA trang DMA đã cấp.
    pub fn dma_pa(&self) -> u64 {
        self.dma_pa.load(Ordering::Relaxed)
    }

    /// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng.
    pub fn published(&self) -> u16 {
        self.published.load(Ordering::Relaxed) as u16
    }

    /// Đọc `device_status` hiện tại.
    pub fn device_status<B: MmioBus>(&self, bus: &mut B) -> u8 {
        let common = self.common.load(Ordering::Relaxed);
        if common == 0 {
            return 0;
        }
        bus.read8(common + C_DEVICE_STATUS)
    }

    /// Khởi tạo driver: status/feature/queue/config trên BAR PCI + vùng DMA đã cấp.
    ///
    /// `fb_pa` là PA của dải backing 640×480×4 (platform chọn — riscv64 `0x8100_0000`, aarch64
    /// `0x4200_0000`, x86_64 `0x0200_0000`), phải nằm trong RAM của ảnh và căn 4 KiB.
    ///
    /// Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do đo được — không PANIC im lặng.
    pub fn init<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        pci: &PciReport,
        fb_pa: u64,
    ) -> Result<GpuReport, &'static str> {
        let _ = clock;
        if fb_pa & 0xfff != 0 {
            return Err("fb_align");
        }
        let layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
        if RING_OFF + layout.total_bytes() > CMD_OFF {
            return Err("layout");
        }
        let page: DmaBuf = dma
            .alloc(DMA_PAGE_BYTES, DMA_PAGE_BYTES)
            .ok_or("dma_align")?;
        if page.len < DMA_PAGE_BYTES || page.pa & 0xfff != 0 || page.off & 0xfff != 0 {
            return Err("dma_align");
        }
        // Vùng DMA sạch trước khi bàn giao (kể cả khi init gọi lại).
        dma.zero(page);
        let bar = pci.bar_base;
        let caps = pci.caps;
        let common = bar + caps.common.offset as u64;
        let notify = bar + caps.notify.offset as u64;
        let device_cfg = bar + caps.device.offset as u64;
        let isr = bar + caps.isr.offset as u64;
        let notify_mul = caps.notify_off_multiplier;

        // ---- 1) reset (§2.4.2 bước 1)
        bus.write8(common + C_DEVICE_STATUS, 0);
        if bus.read8(common + C_DEVICE_STATUS) != 0 {
            return Err("reset");
        }
        // ---- 2) ACK + DRIVER
        bus.write8(common + C_DEVICE_STATUS, ACK);
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER);
        // ---- 3) feature thiết bị (hai trang 32 bit)
        bus.write32(common + C_DEVICE_FEATURE_SELECT, 0);
        let low = bus.read32(common + C_DEVICE_FEATURE) as u64;
        bus.write32(common + C_DEVICE_FEATURE_SELECT, 1);
        let high = bus.read32(common + C_DEVICE_FEATURE) as u64;
        let device_features = (high << 32) | low;
        if device_features & (1u64 << F_VERSION_1) == 0 {
            return Err("version1");
        }
        // ---- 4) đàm phán: **chỉ** VERSION_1 (quyết định ở hàm thuần `negotiated_features`).
        let wanted = negotiated_features(device_features);
        if wanted & (1u64 << F_VERSION_1) == 0 {
            return Err("version1");
        }
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 0);
        bus.write32(common + C_DRIVER_FEATURE, (wanted & 0xffff_ffff) as u32);
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 1);
        bus.write32(common + C_DRIVER_FEATURE, (wanted >> 32) as u32);
        // đọc lại cả hai trang
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 0);
        let drv_low = bus.read32(common + C_DRIVER_FEATURE) as u64;
        bus.write32(common + C_DRIVER_FEATURE_SELECT, 1);
        let drv_high = bus.read32(common + C_DRIVER_FEATURE) as u64;
        let driver_features = (drv_high << 32) | drv_low;
        if driver_features != wanted {
            return Err("features");
        }
        // ---- 5) FEATURES_OK + đọc lại (§2.4.2 bước 5: thiết bị có thể từ chối)
        bus.write8(common + C_DEVICE_STATUS, ACK | DRIVER | FEATURES_OK);
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !features_ok(status) {
            return Err("features_ok");
        }
        // ---- 6) queue `controlq` = 0 (queue `cursorq` = 1 **không** dùng — không có con trỏ)
        let num_queues = bus.read16(common + C_NUM_QUEUES);
        if num_queues < QUEUES_REQUIRED {
            return Err("queues");
        }
        bus.write16(common + C_QUEUE_SELECT, Q_CONTROL);
        let queue_max = bus.read16(common + C_QUEUE_SIZE);
        if queue_max < MIN_QUEUE_SIZE || !queue_max.is_power_of_two() {
            return Err("queue_size");
        }
        let queue_size = QUEUE_SIZE.min(queue_max);
        bus.write16(common + C_QUEUE_SIZE, queue_size);
        if bus.read16(common + C_QUEUE_SIZE) != queue_size {
            return Err("queue_size");
        }
        let layout = VqLayout::new(queue_size).ok_or("layout")?;
        let base = page.pa + RING_OFF as u64;
        let desc_pa = base + layout.desc_off as u64;
        let avail_pa = base + layout.avail_off as u64;
        let used_pa = base + layout.used_off as u64;
        bus.write64(common + C_QUEUE_DESC, desc_pa);
        bus.write64(common + C_QUEUE_DRIVER, avail_pa);
        bus.write64(common + C_QUEUE_DEVICE, used_pa);
        bus.fence();
        bus.write16(common + C_QUEUE_ENABLE, 1);
        if bus.read16(common + C_QUEUE_ENABLE) != 1 {
            return Err("queue_enable");
        }
        let notify_off = bus.read16(common + C_QUEUE_NOTIFY_OFF);
        let notify_addr = notify + notify_off as u64 * notify_mul as u64;
        // ---- 7) DRIVER_OK
        bus.write8(
            common + C_DEVICE_STATUS,
            ACK | DRIVER | FEATURES_OK | DRIVER_OK,
        );
        let status = bus.read8(common + C_DEVICE_STATUS);
        if !status_sequence_ok(status) {
            return Err("driver_ok");
        }
        // ---- 8) device config: `virtio_gpu_config` (num_scanouts là số đo **từ thiết bị**)
        let mut raw = [0u8; GPU_CONFIG_BYTES];
        bus.read_bytes(device_cfg, &mut raw);
        let config = decode_config(&raw).ok_or("config")?;
        // ---- 9) lưu trạng thái
        self.common.store(common, Ordering::Relaxed);
        self.notify.store(notify, Ordering::Relaxed);
        self.device_cfg.store(device_cfg, Ordering::Relaxed);
        self.isr.store(isr, Ordering::Relaxed);
        self.notify_mul.store(notify_mul as u64, Ordering::Relaxed);
        self.notify_off.store(notify_off as u64, Ordering::Relaxed);
        self.queue_size.store(queue_size as u64, Ordering::Relaxed);
        self.dma_off.store(page.off as u64, Ordering::Relaxed);
        self.dma_pa.store(page.pa, Ordering::Relaxed);
        self.fb_pa.store(fb_pa, Ordering::Relaxed);
        self.last_used.store(0, Ordering::Relaxed);
        self.published.store(0, Ordering::Relaxed);
        self.ready.store(true, Ordering::Relaxed);
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
            dma_pa: page.pa,
            desc: desc_pa,
            avail: avail_pa,
            used: used_pa,
            cmd_pa: page.pa + CMD_OFF as u64,
            resp_pa: page.pa + RESP_OFF as u64,
            fb_pa,
            fb_bytes: FB_BYTES,
            fb_pages: FB_PAGES,
            fb_stride: FB_STRIDE,
        })
    }

    /// Gửi **một** lệnh đã nằm sẵn trong vùng lệnh, chờ response, trả `used->len`.
    ///
    /// Chuỗi hai descriptor: `[cmd device-readable] → [resp device-writable]` (§2.6.4). Head luôn
    /// là 0 vì driver gửi **tuần tự** và chỉ publish lại sau khi lệnh trước hoàn tất.
    fn submit<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        cmd_len: usize,
        resp_len: usize,
    ) -> Result<u32, &'static str> {
        if !self.ready.load(Ordering::Relaxed) {
            return Err("not_ready");
        }
        let size = self.queue_size.load(Ordering::Relaxed) as u16;
        if size < MIN_QUEUE_SIZE {
            return Err("not_ready");
        }
        if cmd_len == 0 || cmd_len > CMD_BYTES || resp_len == 0 || resp_len > RESP_BYTES {
            return Err("cmd_len");
        }
        let layout = VqLayout::new(size).ok_or("layout")?;
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let dma_pa = self.dma_pa.load(Ordering::Relaxed);
        {
            let mem =
                &mut dma.bytes()[dma_off + RING_OFF..dma_off + RING_OFF + layout.total_bytes()];
            let mut queue = SplitQueue::new(mem, size).ok_or("desc")?;
            if !queue.set_desc(
                0,
                Desc {
                    addr: dma_pa + CMD_OFF as u64,
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
                    addr: dma_pa + RESP_OFF as u64,
                    len: resp_len as u32,
                    flags: DescFlags::WRITE,
                    next: 0,
                },
            ) {
                return Err("desc");
            }
            queue.push_avail(0);
        }
        // **Bàn giao DMA cho thiết bị TRƯỚC khi báo** (aarch64: `dc civac` + `dsb`, KT-277).
        // Thiếu lời gọi này ⇒ test host `gpu_submit_syncs_for_device_before_notify` ĐỎ, và trên
        // aarch64 thiết bị có thể đọc bản cũ trong RAM.
        dma.sync_for_device(dma_off, DMA_PAGE_BYTES);
        bus.fence();
        let notify = self.notify.load(Ordering::Relaxed);
        let notify_off = self.notify_off.load(Ordering::Relaxed);
        let notify_mul = self.notify_mul.load(Ordering::Relaxed);
        bus.notify(notify + notify_off * notify_mul, Q_CONTROL);
        self.notifies.fetch_add(1, Ordering::Relaxed);
        self.published.fetch_add(1, Ordering::Relaxed);

        // ---- poll used (một chiều) — offset lấy từ **queue_size thật** của queue.
        let used_idx_off = dma_off + RING_OFF + layout.used_idx_off;
        let deadline = clock.deadline();
        let spin_limit = match clock.spin_limit() {
            0 => DEFAULT_SPIN_LIMIT,
            limit => limit,
        };
        let mut spins = 0u64;
        loop {
            // **Cache**: bỏ dòng chứa `used->idx` trước mỗi lần đọc (aarch64 `dc ivac`).
            dma.sync_for_cpu(used_idx_off, 2);
            let idx = dma.read_u16(used_idx_off);
            let last = self.last_used.load(Ordering::Relaxed) as u16;
            if idx != last {
                // Thiết bị đã ghi xong — bỏ cache **trước** khi đọc `used->elem`/response.
                dma.sync_for_cpu(dma_off, DMA_PAGE_BYTES);
                bus.fence();
                let (id, len) = {
                    let mem = &mut dma.bytes()
                        [dma_off + RING_OFF..dma_off + RING_OFF + layout.total_bytes()];
                    let queue = SplitQueue::new(mem, size).ok_or("not_ready")?;
                    queue.used_elem(last & (size - 1))
                };
                self.last_used
                    .store(last.wrapping_add(1) as u64, Ordering::Relaxed);
                if id != 0 {
                    self.id_mismatch.fetch_add(1, Ordering::Relaxed);
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    return Err("used_id");
                }
                self.commands.fetch_add(1, Ordering::Relaxed);
                return Ok(len);
            }
            match deadline {
                Some(limit) => {
                    if clock.now() > limit {
                        self.timeouts.fetch_add(1, Ordering::Relaxed);
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        return Err("timeout");
                    }
                }
                None => {
                    spins = spins.wrapping_add(1);
                    if spins > spin_limit {
                        self.timeouts.fetch_add(1, Ordering::Relaxed);
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        return Err("timeout");
                    }
                }
            }
            core::hint::spin_loop();
        }
    }

    fn write_cmd<D: DmaPool>(&self, dma: &mut D, cmd: &[u8]) {
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        dma.bytes()[dma_off + CMD_OFF..dma_off + CMD_OFF + cmd.len()].copy_from_slice(cmd);
    }

    fn resp_type<D: DmaPool>(&self, dma: &mut D) -> Option<u32> {
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let at = dma_off + RESP_OFF;
        CtrlHdr::parse(&dma.bytes()[at..at + CTRL_HDR_BYTES]).map(|hdr| hdr.type_)
    }

    /// Đòi response là `RESP_OK_NODATA`; response lỗi ⇒ trả **tên** lỗi (D2-102).
    fn check_nodata<D: DmaPool>(&self, dma: &mut D, used_len: u32) -> Result<(), &'static str> {
        if used_len < CTRL_HDR_BYTES as u32 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err("resp_len");
        }
        let type_ = self.resp_type(dma).ok_or("resp_hdr")?;
        if type_ == VIRTIO_GPU_RESP_OK_NODATA {
            return Ok(());
        }
        self.errors.fetch_add(1, Ordering::Relaxed);
        if is_error_response(type_) {
            return Err(resp_error_name(type_));
        }
        Err("resp_type")
    }

    /// `GET_DISPLAY_INFO` — đọc mode hiện tại của mọi scanout.
    ///
    /// Trước `SET_SCANOUT` đầu tiên, QEMU trả `enabled = 0` và `width/height = 0` (mode **chưa**
    /// được đặt); sau `SET_SCANOUT` nó trả mode **của host** (`req_state`) — không phải scanout
    /// của khách. Đó là số đo thật của TAP `gpu display info`.
    pub fn get_display_info<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
    ) -> Result<RespDisplayInfo, &'static str> {
        let cmd = CtrlHdr::new(VIRTIO_GPU_CMD_GET_DISPLAY_INFO).encode();
        self.write_cmd(dma, &cmd);
        let used = self.submit(bus, dma, clock, cmd.len(), RESP_DISPLAY_INFO_BYTES)?;
        if used < RESP_DISPLAY_INFO_BYTES as u32 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err("resp_len");
        }
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let at = dma_off + RESP_OFF;
        let bytes = &dma.bytes()[at..at + RESP_DISPLAY_INFO_BYTES];
        let mut raw = [0u8; RESP_DISPLAY_INFO_BYTES];
        raw.copy_from_slice(bytes);
        RespDisplayInfo::parse(&raw).ok_or("resp_display")
    }

    /// `RESOURCE_CREATE_2D` — tạo resource 2D `width × height`, format `B8G8R8A8_UNORM`.
    pub fn create_resource<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        width: u32,
        height: u32,
    ) -> Result<(), &'static str> {
        if !resource_ok(width, height) {
            return Err("resource");
        }
        let cmd =
            ResourceCreate2d::new(RESOURCE_ID, VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, width, height)
                .encode();
        self.write_cmd(dma, &cmd);
        let used = self.submit(bus, dma, clock, cmd.len(), CTRL_HDR_BYTES)?;
        self.check_nodata(dma, used)
    }

    /// `RESOURCE_ATTACH_BACKING` — gắn [`MAX_ATTACH_ENTRIES`] mảnh của dải PA framebuffer.
    ///
    /// Trả số mảnh đã gửi. Mảnh cuối **ngắn hơn** [`ATTACH_CHUNK_BYTES`] (1.228.800 = 18 × 65.536
    /// + 49.152) — tính bằng `min`, không giả định chia hết.
    pub fn attach_backing<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
    ) -> Result<u32, &'static str> {
        let fb_pa = self.fb_pa.load(Ordering::Relaxed);
        let entries = MAX_ATTACH_ENTRIES;
        let header = ResourceAttachBacking::new(RESOURCE_ID, entries as u32).encode();
        let dma_off = self.dma_off.load(Ordering::Relaxed) as usize;
        let mut at = dma_off + CMD_OFF;
        dma.bytes()[at..at + header.len()].copy_from_slice(&header);
        at += header.len();
        let mut index = 0;
        while index < entries {
            let offset = index * ATTACH_CHUNK_BYTES;
            let length = (FB_BYTES - offset).min(ATTACH_CHUNK_BYTES);
            let entry = MemEntry::new(fb_pa + offset as u64, length as u32).encode();
            dma.bytes()[at..at + MEM_ENTRY_BYTES].copy_from_slice(&entry);
            at += MEM_ENTRY_BYTES;
            index += 1;
        }
        let cmd_len = at - (dma_off + CMD_OFF);
        let used = self.submit(bus, dma, clock, cmd_len, CTRL_HDR_BYTES)?;
        self.check_nodata(dma, used)?;
        Ok(entries as u32)
    }

    /// `SET_SCANOUT` — gắn [`RESOURCE_ID`] vào [`SCANOUT_ID`], nguồn là `rect`.
    pub fn set_scanout<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        rect: Rect,
    ) -> Result<(), &'static str> {
        let cmd = SetScanout::new(SCANOUT_ID, RESOURCE_ID, rect).encode();
        self.write_cmd(dma, &cmd);
        let used = self.submit(bus, dma, clock, cmd.len(), CTRL_HDR_BYTES)?;
        self.check_nodata(dma, used)
    }

    /// **Ca âm đối chứng**: gửi `SET_SCANOUT` với `bogus_id` (**không** tồn tại) và trả **tên
    /// lỗi** thiết bị báo (`InvalidResourceId` nếu thiết bị kiểm đúng).
    ///
    /// Vì sao cần: `GET_DISPLAY_INFO` của QEMU trả mode **của host**, không phải scanout của
    /// khách — nó **không** đổi khi `SET_SCANOUT`. Nếu thiết bị chấp nhận cả `resource_id` rác
    /// thì "SET_SCANOUT thành công" **không** chứng minh được gì; ca âm này chứng minh thiết bị
    /// thật sự kiểm id. Bằng chứng **thật** rằng đã vẽ là điểm ảnh trong PPM của runner.
    ///
    /// Trả `Ok("accepted")` khi thiết bị **chấp nhận** id rác — người gọi phải coi đó là TAP đỏ.
    pub fn probe_bogus_scanout<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        bogus_id: u32,
        rect: Rect,
    ) -> Result<&'static str, &'static str> {
        let cmd = SetScanout::new(SCANOUT_ID, bogus_id, rect).encode();
        self.write_cmd(dma, &cmd);
        let used = self.submit(bus, dma, clock, cmd.len(), CTRL_HDR_BYTES)?;
        if used < CTRL_HDR_BYTES as u32 {
            return Err("resp_len");
        }
        let type_ = self.resp_type(dma).ok_or("resp_hdr")?;
        if is_error_response(type_) {
            self.rejects.fetch_add(1, Ordering::Relaxed);
            return Ok(resp_error_name(type_));
        }
        Ok("accepted")
    }

    /// `TRANSFER_TO_HOST_2D` — chép `rect` từ backing (guest RAM) sang resource của thiết bị.
    ///
    /// `offset` **không** phải 0 khi `rect` lệch gốc: thiết bị đọc từng hàng tại `offset +
    /// stride*h` rồi ghi vào `(rect.y + h) * stride + rect.x * 4` (QEMU `hw/display/virtio-gpu.c`).
    /// Vì vậy `offset` = [`Rect::byte_offset`] của `rect` trong backing. Gửi 0 cho rect lệch gốc
    /// ⇒ thiết bị chép hàng 0 của framebuffer vào `rect.y` ⇒ nội dung hiện lệch đúng
    /// `(rect.x, rect.y)` pixel — lỗi KT-271 đã **đo được** trên PPM trước khi vá.
    ///
    /// **Một chỗ duy nhất** tính offset: test host `gpu_transfer_uses_rect_byte_offset` đỏ nếu
    /// đổi thành 0.
    pub fn transfer_to_host_2d<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        rect: Rect,
    ) -> Result<(), &'static str> {
        let offset = rect.byte_offset(FB_STRIDE);
        self.transfer_to_host_2d_with_offset(bus, dma, clock, rect, offset)
    }

    /// `TRANSFER_TO_HOST_2D` với `offset` tường minh — **chỉ** dùng cho adapter cần tính offset
    /// theo stride khác (không có trong driver này) hoặc test. Đường sản phẩm dùng
    /// [`Gpu::transfer_to_host_2d`].
    pub fn transfer_to_host_2d_with_offset<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        rect: Rect,
        offset: u64,
    ) -> Result<(), &'static str> {
        let cmd = TransferToHost2d::new(RESOURCE_ID, rect, offset).encode();
        self.write_cmd(dma, &cmd);
        let used = self.submit(bus, dma, clock, cmd.len(), CTRL_HDR_BYTES)?;
        self.check_nodata(dma, used)
    }

    /// `RESOURCE_FLUSH` — bảo thiết bị đẩy `rect` của resource ra màn hình.
    pub fn resource_flush<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        rect: Rect,
    ) -> Result<(), &'static str> {
        let cmd = ResourceFlush::new(RESOURCE_ID, rect).encode();
        self.write_cmd(dma, &cmd);
        let used = self.submit(bus, dma, clock, cmd.len(), CTRL_HDR_BYTES)?;
        self.check_nodata(dma, used)
    }

    /// Flush **một vùng đã đổi**: `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH` — đúng **hai** lệnh.
    ///
    /// Đây là đường flush theo lô của console (KT-271): console tích vùng bẩn rồi gọi hàm này
    /// **một** lần cho cả khối chữ, thay vì hai lệnh cho **mỗi** ký tự. Số lệnh là số **đo được**
    /// — [`Gpu::stats`]`.commands` tăng đúng 2 cho mỗi lần gọi.
    pub fn flush_rect<B: MmioBus, D: DmaPool, C: Clock>(
        &self,
        bus: &mut B,
        dma: &mut D,
        clock: &mut C,
        rect: Rect,
    ) -> Result<(), &'static str> {
        self.transfer_to_host_2d(bus, dma, clock, rect)?;
        self.resource_flush(bus, dma, clock, rect)
    }
}
