// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter mỏng** của `virtio-gpu` 2D trên aarch64 — KT-337 (trước: driver đầy đủ KT-279).
//!
//! Quyết định giao thức (status/feature/queue, descriptor, notify, poll, sáu lệnh 2D, offset
//! `TRANSFER_TO_HOST_2D`) nằm ở [`vifast_virtio::gpudev`] — crate **thuần**, test host đầy đủ.
//! Tệp này giữ đúng những gì thuộc nền tảng aarch64:
//!
//! 1. **PA của DMA**: sau `activate`, con trỏ trang DMA là alias **nửa cao**; thiết bị DMA bằng
//!    PA ⇒ [`high_to_pa`] đổi trước khi truyền vào [`init`].
//! 2. **Cache maintenance** (KT-277): [`Dma`] cài `sync_for_device` = `dc civac` từng dòng
//!    (`cache::HANDOFF_SEQUENCE`, bỏ bước `Notify` vì [`vifast_virtio::mmio::MmioBus::notify`]
//!    của transport đã làm) và `sync_for_cpu` = `dc ivac`; **framebuffer** (Normal cacheable) được
//!    làm sạch **đúng các hàng của rect** trước `TRANSFER_TO_HOST_2D`.
//! 3. **Đồng hồ**: `CNTPCT_EL0`; `CNTFRQ_EL0 = 0` ⇒ lùi về trần **số vòng**.
//!
//! Bộ đếm cache (cho dòng `# gpu stats: … fb_clean_*`) đếm **chính các lệnh `dc` đã chạy** —
//! bỏ lời gọi `dc` ⇒ `fb_clean_ops=0` ⇒ runner ĐỎ (tầng quan sát được trên QEMU TCG; hiệu ứng
//! cache thì TCG không mô hình hoá — ghi trung thực ở `v2/docs/k4-aarch64-boot.md` §11).

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_virtio::blkdev::Clock;
use vifast_virtio::gpudev::{DmaBuf, DmaPool, Gpu, MmioBus};

use crate::asm;
use crate::cache;
use crate::pci::PciReport;

pub use vifast_virtio::gpudev::{
    GpuReport, ATTACH_CHUNK_BYTES, CMD_BYTES, CMD_OFF, DMA_PAGE_BYTES, FB_BYTES, FB_HEIGHT,
    FB_PAGES, FB_STRIDE, FB_WIDTH, MAX_ATTACH_ENTRIES, QUEUE_SIZE, RESOURCE_ID, RESP_BYTES,
    RESP_OFF, RING_OFF, SCANOUT_ID,
};

/// Số giây trần khi poll `used` (spec §2.7.13: không ngắt thì driver poll, phải có trần).
pub const POLL_TIMEOUT_SECS: u64 = 2;
/// Trần **số vòng** dự phòng khi `CNTFRQ_EL0 = 0` (không có trần thời gian).
pub const POLL_SPIN_LIMIT: u64 = 200_000_000;
/// Base **vật lý** của framebuffer: 2 MiB đầu **sau** vùng ảnh (~205 KiB từ `0x4008_0000`),
/// trong RAM `-m 128M`, căn 2 MiB.
pub const FB_PHYS_BASE: u64 = 0x4200_0000;

const _: () = assert!(FB_STRIDE == FB_WIDTH as usize * 4);
const _: () = assert!(FB_PHYS_BASE.is_multiple_of(2 << 20));
const _: () = assert!(FB_PHYS_BASE >= 0x400C_0000); // sau vùng ảnh (~205 KiB từ 0x4008_0000)
const _: () = assert!(FB_PHYS_BASE + FB_BYTES as u64 <= 0x4000_0000 + 0x0800_0000); // RAM -m 128M
const _: () = assert!(FB_BYTES as u64 <= 2 << 20); // một leaf 2 MiB ⇒ một bảng mức 3
const _: () = {
    let layout = match vifast_virtio::queue::VqLayout::new(QUEUE_SIZE) {
        Some(layout) => layout,
        None => panic!("QUEUE_SIZE phải là luỹ thừa 2"),
    };
    assert!(RING_OFF + layout.end <= CMD_OFF);
    assert!(CMD_OFF + CMD_BYTES <= RESP_OFF);
    assert!(RESP_OFF + RESP_BYTES <= DMA_PAGE_BYTES);
};

// ---------------------------------------------------------------- bộ đếm cache framebuffer

static FB_CLEAN_LINES: AtomicU64 = AtomicU64::new(0);
static FB_CLEAN_BYTES: AtomicU64 = AtomicU64::new(0);
static FB_CLEAN_OPS: AtomicU64 = AtomicU64::new(0);

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
    /// Số lần thiết bị **từ chối** lệnh trong ca âm đối chứng.
    pub rejects: u64,
    /// Số dòng framebuffer đã `dc civac` trước `TRANSFER_TO_HOST_2D` (tổng qua mọi lần flush).
    pub fb_clean_lines: u64,
    /// Số byte framebuffer đã làm sạch (tổng).
    pub fb_clean_bytes: u64,
    /// Số lần chạy vòng làm sạch framebuffer (mỗi lần flush một lần).
    pub fb_clean_ops: u64,
}

impl Stats {
    /// `true` ⇔ đường đi sạch: có lệnh, không lỗi/timeout/lệch id, và **đã** làm sạch framebuffer
    /// ít nhất một lần (bỏ `dc` ⇒ `false`).
    pub const fn ok(self) -> bool {
        self.commands > 0
            && self.errors == 0
            && self.timeouts == 0
            && self.id_mismatch == 0
            && self.fb_clean_ops > 0
            && self.fb_clean_lines > 0
            && self.fb_clean_bytes > 0
    }
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    let dev = GPU.stats();
    Stats {
        commands: dev.commands,
        notifies: dev.notifies,
        errors: dev.errors,
        timeouts: dev.timeouts,
        id_mismatch: dev.id_mismatch,
        rejects: dev.rejects,
        fb_clean_lines: FB_CLEAN_LINES.load(Ordering::SeqCst),
        fb_clean_bytes: FB_CLEAN_BYTES.load(Ordering::SeqCst),
        fb_clean_ops: FB_CLEAN_OPS.load(Ordering::SeqCst),
    }
}

// ---------------------------------------------------------------- PA của trang DMA

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết.
///
/// Sau khi nhảy nửa cao, `adrp` cho alias `KERNEL_HIGH_BASE + (pa - text_start)`; PA thật là
/// `text_start_pa + (va - KERNEL_HIGH_BASE)`. Chỉ đúng cho địa chỉ **trong ảnh** — trang DMA
/// nằm trong `.bss` nên thuộc trường hợp này.
pub const fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - crate::split_mmu::KERNEL_HIGH_BASE)
}

/// Bộ đếm vật lý hiện tại (`CNTPCT_EL0`).
pub fn now_ticks() -> u64 {
    asm::read_cntpct_el0()
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

/// Pool DMA một vùng: trang `.bss` của ảnh, PA do caller truyền (alias nửa cao → PA).
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
        asm::dma_page_gpu()
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
        // `dc civac` từng dòng (ghi bản bẩn ra PoC + bỏ bản sao) → `dsb ish` → `dsb sy`.
        // Bước `Notify` của chuỗi do transport làm qua `MmioBus::notify` — adapter **không** ghi
        // cửa thông báo lần thứ hai.
        let addr = self.base + off as u64;
        for step in cache::HANDOFF_SEQUENCE {
            if matches!(step, cache::HandoffStep::Notify) {
                continue;
            }
            cache::apply_handoff(step, addr, len as u64, 0);
        }
    }
    fn sync_for_cpu(&mut self, off: usize, len: usize) {
        let addr = self.base + off as u64;
        if len <= 8 {
            // Một dòng chứa `used->idx` trong vòng poll: mọi dòng đã **sạch** từ lần handoff, nên
            // `dc ivac` chỉ bỏ bản sao, không mất dữ liệu chưa ghi.
            cache::invalidate_line(addr);
            return;
        }
        for step in cache::COMPLETE_SEQUENCE {
            cache::apply_complete(step, addr, len as u64);
        }
    }
}

// ---------------------------------------------------------------- adapter đồng hồ

struct Tick;

impl Clock for Tick {
    fn now(&mut self) -> u64 {
        now_ticks()
    }
    fn deadline(&mut self) -> Option<u64> {
        let freq = asm::read_cntfrq_el0();
        if freq == 0 {
            None
        } else {
            Some(now_ticks().wrapping_add(POLL_TIMEOUT_SECS.wrapping_mul(freq)))
        }
    }
    fn spin_limit(&self) -> u64 {
        POLL_SPIN_LIMIT
    }
}

// ---------------------------------------------------------------- hình học framebuffer (thuần)

/// Offset **byte** của từng hàng của `rect` trong framebuffer — dùng cho vòng `dc civac` theo
/// hàng (mỗi hàng một lần, không làm sạch cả khung cho một rect nhỏ).
///
/// Hàng `h` bắt đầu ở `rect.byte_offset(FB_STRIDE) + h × FB_STRIDE` — **cùng** công thức
/// [`Rect::byte_offset`] mà `TRANSFER_TO_HOST_2D.offset` dùng, nên vùng làm sạch và vùng thiết bị
/// đọc không thể lệch nhau.
pub fn rect_rows(rect: vifast_virtio::gpu::Rect, stride: usize) -> impl Iterator<Item = u64> {
    let base = rect.byte_offset(stride);
    let step = stride as u64;
    (0..rect.height as u64).map(move |row| base + row * step)
}

/// Số dòng cache mà [`clean_framebuffer_rect`] phát ra cho `rect` — **đúng** con số vòng lặp
/// chạy, không phải ước lượng (test host đối chiếu với `# gpu stats:` của ảnh).
pub fn clean_rect_lines(rect: vifast_virtio::gpu::Rect, line: u64) -> u64 {
    if rect.is_empty() || line == 0 {
        return 0;
    }
    let row_bytes = rect.width as u64 * vifast_virtio::gpu::BYTES_PER_PIXEL as u64;
    let mut total = 0;
    for offset in rect_rows(rect, FB_STRIDE) {
        total += cache::line_count(FB_PHYS_BASE + offset, row_bytes, line);
    }
    total
}

/// `dc civac` từng hàng của `rect` trong framebuffer rồi `dsb ish`/`dsb sy` — **bàn giao** vùng
/// đã vẽ cho thiết bị trước `TRANSFER_TO_HOST_2D`.
///
/// Vì sao cần: framebuffer map **Normal cacheable** (đúng: nó là RAM), còn thiết bị đọc bằng DMA
/// **không** đi qua cache của CPU. Không làm sạch ⇒ thiết bị có thể đọc bản cũ trong RAM và PPM
/// sẽ không có gì (trên phần cứng thật; QEMU TCG không mô hình hoá cache nên **hiệu ứng** không
/// quan sát được — tầng quan sát được là bộ đếm `fb_clean_lines`).
///
/// Trả số dòng đã làm sạch.
pub fn clean_framebuffer_rect(rect: vifast_virtio::gpu::Rect) -> u64 {
    if rect.is_empty() {
        return 0;
    }
    let row_bytes = rect.width as u64 * vifast_virtio::gpu::BYTES_PER_PIXEL as u64;
    let mut lines = 0;
    for offset in rect_rows(rect, FB_STRIDE) {
        lines += cache::clean_invalidate_lines(FB_PHYS_BASE + offset, row_bytes);
    }
    asm::dsb_ish();
    asm::dsb_sy();
    FB_CLEAN_LINES.fetch_add(lines, Ordering::SeqCst);
    FB_CLEAN_BYTES.fetch_add(
        rect.width as u64 * rect.height as u64 * vifast_virtio::gpu::BYTES_PER_PIXEL as u64,
        Ordering::SeqCst,
    );
    FB_CLEAN_OPS.fetch_add(1, Ordering::SeqCst);
    lines
}

// ---------------------------------------------------------------- trạng thái + API cũ

static GPU: Gpu = Gpu::new();

/// Khởi tạo driver từ kết quả PCI. `dma_pa` là **PA** của trang DMA — phải căn 4 KiB.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<GpuReport, &'static str> {
    if dma_pa & 0xfff != 0 {
        return Err("dma_align");
    }
    let mut dma = Dma { base: dma_pa };
    GPU.init(&mut Mmio, &mut dma, &mut Tick, pci, FB_PHYS_BASE)
}

/// `GET_DISPLAY_INFO` — mode **thiết bị báo**.
pub fn get_display_info() -> Result<vifast_virtio::gpu::RespDisplayInfo, &'static str> {
    GPU.get_display_info(&mut Mmio, &mut Dma { base: dma_pa() }, &mut Tick)
}

/// `RESOURCE_CREATE_2D` — tạo resource 2D.
pub fn create_resource(width: u32, height: u32) -> Result<(), &'static str> {
    GPU.create_resource(
        &mut Mmio,
        &mut Dma { base: dma_pa() },
        &mut Tick,
        width,
        height,
    )
}

/// `RESOURCE_ATTACH_BACKING` — gắn 19 mảnh của dải framebuffer.
pub fn attach_backing() -> Result<u32, &'static str> {
    GPU.attach_backing(&mut Mmio, &mut Dma { base: dma_pa() }, &mut Tick)
}

/// `SET_SCANOUT` — gắn resource vào scanout.
pub fn set_scanout(rect: vifast_virtio::gpu::Rect) -> Result<(), &'static str> {
    GPU.set_scanout(&mut Mmio, &mut Dma { base: dma_pa() }, &mut Tick, rect)
}

/// Ca âm đối chứng: `SET_SCANOUT` với resource không tồn tại.
pub fn probe_bogus_scanout(
    bogus_id: u32,
    rect: vifast_virtio::gpu::Rect,
) -> Result<&'static str, &'static str> {
    GPU.probe_bogus_scanout(
        &mut Mmio,
        &mut Dma { base: dma_pa() },
        &mut Tick,
        bogus_id,
        rect,
    )
}

/// `TRANSFER_TO_HOST_2D` — làm sạch **đúng các hàng của rect** rồi gửi lệnh; offset do crate
/// thuần tính bằng `Rect::byte_offset` (một chỗ duy nhất).
pub fn transfer_to_host_2d(rect: vifast_virtio::gpu::Rect) -> Result<(), &'static str> {
    clean_framebuffer_rect(rect);
    GPU.transfer_to_host_2d(&mut Mmio, &mut Dma { base: dma_pa() }, &mut Tick, rect)
}

/// `RESOURCE_FLUSH` — đẩy vùng ra màn hình.
pub fn resource_flush(rect: vifast_virtio::gpu::Rect) -> Result<(), &'static str> {
    GPU.resource_flush(&mut Mmio, &mut Dma { base: dma_pa() }, &mut Tick, rect)
}

/// Flush một vùng đã đổi: `TRANSFER_TO_HOST_2D` (có làm sạch cache) + `RESOURCE_FLUSH`.
pub fn flush_rect(rect: vifast_virtio::gpu::Rect) -> Result<(), &'static str> {
    transfer_to_host_2d(rect)?;
    resource_flush(rect)
}

/// Framebuffer driver vẽ — dải PA [`FB_PHYS_BASE`] map **identity** (`VA == PA`) trong `VSpace`
/// của ảnh, nên thiết bị DMA đọc đúng dải PA đó.
pub fn framebuffer() -> &'static mut [u8] {
    asm::gpu_framebuffer(FB_PHYS_BASE, FB_BYTES)
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    GPU.ready()
}

/// Địa chỉ ISR (chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    GPU.isr_addr()
}

/// PA trang DMA đã cấp (bằng chứng `identity` của vùng DMA).
pub fn dma_pa() -> u64 {
    GPU.dma_pa()
}

/// Đọc thanh ghi `device_status` hiện tại.
pub fn device_status() -> u8 {
    GPU.device_status(&mut Mmio)
}

/// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng.
pub fn published() -> u16 {
    GPU.published()
}
