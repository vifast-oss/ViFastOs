// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-gpu 2D** (virtio 1.0, virtio-pci modern) trên QEMU `virt` aarch64 — KT-279
//! (K4' bước 3, D1 phần 4 trên aarch64).
//!
//! # Đường đi
//!
//! [`crate::pci::init_for`]`(VirtioKind::Gpu)` (ECAM của KT-273, **không** nhân đôi code PCI) ⇒
//! BAR modern + bốn capability ⇒ tệp này lập trình common config, dựng **một** split virtqueue
//! (`controlq` = queue 0 — §5.7.2; `cursorq` = queue 1 **bỏ trống**, không có con trỏ), rồi chạy
//! chuỗi lệnh 2D:
//!
//! ```text
//! GET_DISPLAY_INFO → RESOURCE_CREATE_2D → RESOURCE_ATTACH_BACKING → SET_SCANOUT
//!   → (vẽ vào framebuffer) → TRANSFER_TO_HOST_2D → RESOURCE_FLUSH
//! ```
//!
//! # Khác khuôn riscv64 ở đúng bốn điểm (đều do kiến trúc, không do phong cách)
//!
//! 1. **Địa chỉ DMA là PA, không phải VA**: sau `activate`, hart 0 chạy ở **alias nửa cao**
//!    (`KERNEL_HIGH_BASE + …`), nên `addr_of!(trang DMA)` **không** phải PA. Thiết bị DMA bằng
//!    PA ⇒ caller đổi bằng [`high_to_pa`] rồi truyền vào [`init`]. Descriptor/`avail`/`used` đều
//!    ghi bằng PA ấy.
//! 2. **Trạng thái driver bằng `Atomic*`, không `UnsafeCell`**: crate này `#![deny(unsafe_code)]`
//!    ngoài [`crate::asm`] (D2-17), nên không có `unsafe impl Sync`/`&mut *cell`. Ảnh boot chạy
//!    **một** hart trên đường driver và driver tự tuần tự hoá (một lệnh một lúc) ⇒
//!    `load`/`store(Relaxed)` là đủ, đúng khuôn `virtio_blk.rs`/`virtio_net.rs`.
//! 3. **Trần poll bằng `CNTPCT_EL0`** (bộ đếm vật lý, luôn có) thay vì timebase của `Platform`:
//!    bước 3 **chưa** có timer per-hart (nợ §7.1 của `k4-aarch64-boot.md`), nhưng bộ đếm đọc
//!    được không cần cấu hình. `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ lùi về trần **số vòng**, ghi rõ.
//! 4. **Cache maintenance trước/sau DMA** (KT-277): vùng DMA nằm trong `.bss` map **Normal
//!    cacheable**, framebuffer nằm trong RAM map **Normal cacheable** — cả hai phải `dc civac`
//!    trước khi bàn giao cho thiết bị và `dc ivac` sau khi thiết bị ghi. Dùng **cùng**
//!    [`crate::cache::HANDOFF_SEQUENCE`]/[`crate::cache::COMPLETE_SEQUENCE`] của KT-277, không
//!    viết bản thứ hai.
//!
//! # Framebuffer: **300 trang ngoài ảnh**, không phải `.bss`
//!
//! 640×480×4 = 1.228.800 B = **300 trang 4 KiB**. `.data+.bss` của ảnh đã ở **225.512 B** /
//! trần **262.144 B** (86,0 %, đo bằng `budget-aarch64.sh` **trước** khi viết tệp này) ⇒ chỉ còn
//! **36.632 B** trống: **không** nhét vừa framebuffer, và cũng **không** nên — framebuffer là
//! vùng DMA **liền** mà thiết bị đọc thẳng, không phải trạng thái tĩnh của kernel.
//!
//! Ba phương án đã **đo** trước khi chọn:
//!
//! | Phương án | Cỡ | Vừa? |
//! |---|---|---|
//! | `.bss` 640×480×4 | 1.228.800 B | **không** (vượt trần 4,7×) |
//! | `.bss` 128×64×4 | 32.768 B | **không** — còn 36.632 B nhưng phải thêm trang DMA 4 KiB + trạng thái ⇒ sát/vượt trần, và 128×64 chỉ in được 15×5 ô |
//! | arena boot ([`crate::bootmem`]) | 44 frame, **38 đã dùng** ⇒ 6 frame = 24 KiB | **không** (thiếu cả cho 32 KiB) |
//! | **RAM ngoài ảnh** (chọn) | 300 trang ở [`FB_PHYS_BASE`] | **vừa** — **0 B** vào `.data+.bss` |
//!
//! Vì vậy framebuffer lấy dải PA [`FB_PHYS_BASE`] = `0x4200_0000` (32 MiB sau gốc RAM, **8 MiB**
//! sau vùng ảnh, căn 2 MiB) và được map vào `VSpace` thành **một cửa sổ 2 MiB** (một bảng mức 3
//! = **1 frame** của arena, xem `main.rs`). Đây đúng khuôn riscv64 (`FB_PHYS_BASE` của KT-263):
//! framebuffer **không** phải `.data+.bss`, nên trần I7 **không** bị nới một byte nào.
//!
//! `RESOURCE_ATTACH_BACKING` gửi **nhiều** `virtio_gpu_mem_entry` (mỗi mảnh [`ATTACH_CHUNK_BYTES`]
//! = 16 trang) chứ không một mảnh khổng lồ: đây là đường scatter-gather thật của spec, và nó
//! chứng minh driver tính đúng `addr`/`length` cho từng mảnh (mảnh cuối **ngắn hơn**, không tràn).
//!
//! # Bất biến
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1`; **không** `VIRGIL`/`EDID`/`RESOURCE_BLOB`
//!   — quyết định nằm ở `vifast_virtio::gpu::negotiated_features` (test host), không viết tay hai
//!   chỗ (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **`dsb sy`** → ghi `avail->idx` →
//!   **`dsb sy`** → notify (MMIO) → poll `used->idx` → **`dsb sy`** → đọc `used->elem`.
//! - `used->id` **phải** là head 0 (chuỗi hai descriptor driver vừa publish) — lệch ⇒ đếm
//!   `id_mismatch` + lỗi, không im lặng. `used->len` **phải** ≥ cỡ header response.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); response lỗi ⇒ trả **tên**
//!   lỗi (`Unspec`/`InvalidParameter`…) để TAP in ra lý do đo được; poll quá hạn ⇒ `timeout`.
//! - `TRANSFER_TO_HOST_2D.offset` = [`Rect::byte_offset`] của rect (KT-271) — **không** gửi 0 cho
//!   rect lệch gốc, và **không** tự tính lại offset bằng công thức thứ hai.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::gpu::{
    decode_config, is_error_response, negotiated_features, resource_ok, resp_error_name, CtrlHdr,
    GpuConfig, MemEntry, Rect, ResourceAttachBacking, ResourceCreate2d, ResourceFlush,
    RespDisplayInfo, SetScanout, TransferToHost2d, BYTES_PER_PIXEL, CTRL_HDR_BYTES,
    GPU_CONFIG_BYTES, MEM_ENTRY_BYTES, MIN_QUEUE_SIZE, QUEUES_REQUIRED, Q_CONTROL,
    RESOURCE_ATTACH_BACKING_BYTES, RESP_DISPLAY_INFO_BYTES, VIRTIO_GPU_CMD_GET_DISPLAY_INFO,
    VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, VIRTIO_GPU_RESP_OK_NODATA,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::asm;
use crate::cache;
use crate::pci::PciReport;

/// Số descriptor của queue `controlq` — **cùng** hằng của crate thuần
/// (`vifast_virtio::gpu::QUEUE_SIZE`), không chép lại số 16 (D2-110B).
pub const QUEUE_SIZE: u16 = vifast_virtio::gpu::QUEUE_SIZE;

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
pub const FB_STRIDE: usize = FB_WIDTH as usize * 4;
/// Số trang 4 KiB framebuffer chiếm = 300.
pub const FB_PAGES: usize = FB_BYTES / 4096;
/// Base **vật lý** của framebuffer: **32 MiB** sau gốc RAM `0x4000_0000`, căn 2 MiB.
///
/// Vì sao 32 MiB: ảnh link tại `0x4008_0000` và dài ~205 KiB ⇒ kết thúc dưới `0x400C_0000`;
/// `0x4200_0000` cách nó **24 MiB**, nằm trong RAM `-m 128M` (`0x4000_0000..0x4800_0000`) và
/// **dưới** vùng QEMU đặt DTB/initrd ở cuối RAM. Căn 2 MiB để **một** bảng mức 3 phủ trọn 300
/// trang (1 frame arena) — xem tài liệu đầu tệp.
pub const FB_PHYS_BASE: u64 = 0x4200_0000;
/// Cỡ **một** mảnh backing gửi trong `RESOURCE_ATTACH_BACKING` = 16 trang 4 KiB.
pub const ATTACH_CHUNK_BYTES: usize = 64 * 1024;
/// Số `virtio_gpu_mem_entry` driver gửi = ⌈[`FB_BYTES`] / [`ATTACH_CHUNK_BYTES`]⌉ = 19.
pub const MAX_ATTACH_ENTRIES: usize = FB_BYTES.div_ceil(ATTACH_CHUNK_BYTES);

/// `resource_id` driver dùng (một resource duy nhất — màn hình chính).
pub const RESOURCE_ID: u32 = 1;
/// `scanout_id` driver dùng (QEMU `-display none`: scanout 0).
pub const SCANOUT_ID: u32 = 0;

/// Số giây trần khi poll `used` (spec §2.7.13: không ngắt thì driver poll, phải có trần).
pub const POLL_TIMEOUT_SECS: u64 = 2;
/// Trần **số vòng** dự phòng khi `CNTFRQ_EL0 = 0` (không có trần thời gian).
pub const POLL_SPIN_LIMIT: u64 = 200_000_000;

// `const` assert: framebuffer phải nằm **trong** RAM, **sau** vùng ảnh, căn 2 MiB, và số trang
// phải khớp `FB_BYTES` — lệch một hằng là **không biên dịch được** thay vì DMA ra ngoài RAM
// (thiết bị ghi vào chỗ khác).
const _: () = assert!(FB_BYTES == FB_WIDTH as usize * FB_HEIGHT as usize * 4);
const _: () = assert!(FB_PAGES * 4096 == FB_BYTES);
const _: () = assert!(FB_STRIDE == FB_WIDTH as usize * BYTES_PER_PIXEL);
const _: () = assert!(FB_PHYS_BASE.is_multiple_of(2 << 20));
const _: () = assert!(FB_PHYS_BASE >= 0x400C_0000); // sau vùng ảnh (~205 KiB từ 0x4008_0000)
const _: () = assert!(FB_PHYS_BASE + FB_BYTES as u64 <= 0x4000_0000 + 0x0800_0000); // RAM -m 128M
const _: () = assert!(FB_BYTES as u64 <= 2 << 20); // một leaf 2 MiB ⇒ một bảng mức 3
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

// ---------------------------------------------------------------- trạng thái (atomic)
static READY: AtomicBool = AtomicBool::new(false);
static COMMON: AtomicU64 = AtomicU64::new(0);
static NOTIFY: AtomicU64 = AtomicU64::new(0);
static DEVICE_CFG: AtomicU64 = AtomicU64::new(0);
static ISR: AtomicU64 = AtomicU64::new(0);
static NOTIFY_MUL: AtomicU64 = AtomicU64::new(0);
static NOTIFY_OFF: AtomicU64 = AtomicU64::new(0);
static QUEUE_SIZE_NOW: AtomicU64 = AtomicU64::new(0);
static LAST_USED: AtomicU64 = AtomicU64::new(0);
static PUBLISHED: AtomicU64 = AtomicU64::new(0);
static DMA_PA: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------- bộ đếm (an toàn, atomic)
static COMMANDS: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);
/// Số lần thiết bị **từ chối** một lệnh trong **ca âm đối chứng** (không phải lỗi driver).
static REJECTS: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------- cache maintenance (KT-277)
//
// Bộ đếm **của chính vòng lặp phát `dc`** trên framebuffer — nhờ vậy `# gpu fb:` in ra số lệnh
// `dc civac` **đã chạy**, không phải một hằng viết tay. Bỏ lời gọi `dc` ⇒ `fb_clean_lines=0` ⇒
// runner ĐỎ (tầng quan sát được trên QEMU TCG; **hiệu ứng** của cache thì TCG không mô hình hoá).
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
    /// Số lần thiết bị **từ chối** lệnh trong ca âm đối chứng (`probe_bogus_scanout`) — **không**
    /// tính là lỗi driver; đây là bằng chứng thiết bị thật sự kiểm `resource_id`.
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
    Stats {
        commands: COMMANDS.load(Ordering::SeqCst),
        notifies: NOTIFIES.load(Ordering::SeqCst),
        errors: ERRORS.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
        id_mismatch: ID_MISMATCH.load(Ordering::SeqCst),
        rejects: REJECTS.load(Ordering::SeqCst),
        fb_clean_lines: FB_CLEAN_LINES.load(Ordering::SeqCst),
        fb_clean_bytes: FB_CLEAN_BYTES.load(Ordering::SeqCst),
        fb_clean_ops: FB_CLEAN_OPS.load(Ordering::SeqCst),
    }
}

// ---------------------------------------------------------------- MMIO (qua `asm`)
#[inline]
fn read8(addr: u64) -> u8 {
    asm::mmio_read_u8(addr)
}
#[inline]
fn read16(addr: u64) -> u16 {
    asm::mmio_read_u16(addr)
}
#[inline]
fn read32(addr: u64) -> u32 {
    asm::mmio_read_u32(addr)
}
#[inline]
fn write8(addr: u64, value: u8) {
    asm::mmio_write_u8(addr, value);
}
#[inline]
fn write16(addr: u64, value: u16) {
    asm::mmio_write_u16(addr, value);
}
#[inline]
fn write32(addr: u64, value: u32) {
    asm::mmio_write_u32(addr, value);
}
#[inline]
fn write64(addr: u64, value: u64) {
    asm::mmio_write_u64(addr, value);
}

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13).
///
/// `dsb sy` (không phải `dmb`): DMA của thiết bị nằm **ngoài** domain shareability của CPU nên
/// cần barrier **đầy đủ** — cùng lý do `fence rw, rw` bên riscv64.
#[inline]
fn fence() {
    asm::dsb_sy();
}

/// Trang DMA của ảnh (4 KiB căn lề) — vùng virtqueue + lệnh + response.
fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    asm::dma_page_gpu()
}

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

/// Hạn chót poll: `Some(deadline)` nếu có tần số bộ đếm, `None` nếu `CNTFRQ_EL0 = 0`.
fn poll_deadline() -> Option<u64> {
    let freq = asm::read_cntfrq_el0();
    if freq == 0 {
        None
    } else {
        Some(now_ticks().wrapping_add(POLL_TIMEOUT_SECS * freq))
    }
}

// ---------------------------------------------------------------- hình học framebuffer (thuần)

/// Offset **byte** của từng hàng của `rect` trong framebuffer — dùng cho vòng `dc civac` theo
/// hàng (mỗi hàng một lần, không làm sạch cả khung cho một rect nhỏ).
///
/// Hàng `h` bắt đầu ở `rect.byte_offset(FB_STRIDE) + h × FB_STRIDE` — **cùng** công thức
/// [`Rect::byte_offset`] mà `TRANSFER_TO_HOST_2D.offset` dùng, nên vùng làm sạch và vùng thiết bị
/// đọc không thể lệch nhau.
pub fn rect_rows(rect: Rect, stride: usize) -> impl Iterator<Item = u64> {
    let base = rect.byte_offset(stride);
    let step = stride as u64;
    (0..rect.height as u64).map(move |row| base + row * step)
}

/// Số dòng cache mà [`clean_framebuffer_rect`] phát ra cho `rect` — **đúng** con số vòng lặp
/// chạy, không phải ước lượng (test host đối chiếu với `# gpu fb:` của ảnh).
pub fn clean_rect_lines(rect: Rect, line: u64) -> u64 {
    if rect.is_empty() || line == 0 {
        return 0;
    }
    let row_bytes = rect.width as u64 * BYTES_PER_PIXEL as u64;
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
pub fn clean_framebuffer_rect(rect: Rect) -> u64 {
    if rect.is_empty() {
        return 0;
    }
    let row_bytes = rect.width as u64 * BYTES_PER_PIXEL as u64;
    let mut lines = 0;
    for offset in rect_rows(rect, FB_STRIDE) {
        lines += cache::clean_invalidate_lines(FB_PHYS_BASE + offset, row_bytes);
    }
    asm::dsb_ish();
    asm::dsb_sy();
    FB_CLEAN_LINES.fetch_add(lines, Ordering::SeqCst);
    FB_CLEAN_BYTES.fetch_add(rect.width as u64 * rect.height as u64 * 4, Ordering::SeqCst);
    FB_CLEAN_OPS.fetch_add(1, Ordering::SeqCst);
    lines
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
    /// Địa chỉ BAR đã gán (PA = VA, identity).
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
    /// PA framebuffer (đầu dải 300 trang).
    pub fb_pa: u64,
    /// Số byte framebuffer.
    pub fb_bytes: usize,
    /// Số trang 4 KiB framebuffer.
    pub fb_pages: usize,
}

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
///
/// `dma_pa` là **PA** của trang DMA (caller đổi alias nửa cao → PA bằng [`high_to_pa`]); phải
/// căn 4 KiB vì descriptor/`avail`/`used` đều suy từ nó bằng phép cộng offset.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<GpuReport, &'static str> {
    if dma_pa & 0xfff != 0 {
        return Err("dma_align");
    }
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
    let base = dma_pa + RING_OFF as u64;
    let desc_pa = base + layout.desc_off as u64;
    let avail_pa = base + layout.avail_off as u64;
    let used_pa = base + layout.used_off as u64;
    write64(common + C_QUEUE_DESC, desc_pa);
    write64(common + C_QUEUE_DRIVER, avail_pa);
    write64(common + C_QUEUE_DEVICE, used_pa);
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
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    COMMON.store(common, Ordering::Relaxed);
    NOTIFY.store(notify, Ordering::Relaxed);
    DEVICE_CFG.store(device_cfg, Ordering::Relaxed);
    ISR.store(isr, Ordering::Relaxed);
    NOTIFY_MUL.store(notify_mul as u64, Ordering::Relaxed);
    NOTIFY_OFF.store(notify_off as u64, Ordering::Relaxed);
    QUEUE_SIZE_NOW.store(queue_size as u64, Ordering::Relaxed);
    LAST_USED.store(0, Ordering::Relaxed);
    PUBLISHED.store(0, Ordering::Relaxed);
    READY.store(true, Ordering::Relaxed);
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
        dma_pa,
        desc: desc_pa,
        avail: avail_pa,
        used: used_pa,
        cmd_pa: dma_pa + CMD_OFF as u64,
        resp_pa: dma_pa + RESP_OFF as u64,
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
    if !READY.load(Ordering::Relaxed) {
        return Err("not_ready");
    }
    let size = QUEUE_SIZE_NOW.load(Ordering::Relaxed) as u16;
    if size < MIN_QUEUE_SIZE {
        return Err("not_ready");
    }
    if cmd_len == 0 || cmd_len > CMD_BYTES || resp_len == 0 || resp_len > RESP_BYTES {
        return Err("cmd_len");
    }
    let dma_pa = DMA_PA.load(Ordering::Relaxed);
    {
        let mut queue = queue_with(size)?;
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
    // ---- **KT-277**: bàn giao vùng DMA theo `cache::HANDOFF_SEQUENCE`:
    // `dc civac` từng dòng (ghi bản bẩn ra PoC + bỏ bản sao) → `dsb ish` → `dsb sy` → notify.
    let notify = NOTIFY.load(Ordering::Relaxed);
    let notify_off = NOTIFY_OFF.load(Ordering::Relaxed) as u16;
    let notify_mul = NOTIFY_MUL.load(Ordering::Relaxed) as u32;
    let notify_addr = notify + notify_off as u64 * notify_mul as u64;
    for step in cache::HANDOFF_SEQUENCE {
        cache::apply_handoff(step, dma_pa, DMA_PAGE_BYTES as u64, notify_addr);
    }
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
    PUBLISHED.fetch_add(1, Ordering::Relaxed);

    // ---- poll used (một chiều) — offset lấy từ **qsize thật** của queue.
    let layout = VqLayout::new(size).ok_or("layout")?;
    let used_idx_addr = dma_pa + RING_OFF as u64 + layout.used_idx_off as u64;
    let deadline = poll_deadline();
    let mut spins = 0u64;
    loop {
        // **KT-277**: `dc ivac` dòng chứa `used->idx` **trước** mỗi lần đọc (thiết bị ghi qua DMA
        // không đi qua cache của CPU). An toàn: `HANDOFF_SEQUENCE` vừa `dc civac` toàn trang nên
        // mọi dòng đều **sạch** — `dc ivac` chỉ bỏ bản sao đã sạch.
        cache::invalidate_line(used_idx_addr);
        let idx = asm::mmio_read_u16(used_idx_addr);
        let last = LAST_USED.load(Ordering::Relaxed) as u16;
        if idx != last {
            // Thiết bị đã ghi xong — bỏ cache **trước** khi đọc `used->elem` và response.
            for step in cache::COMPLETE_SEQUENCE {
                cache::apply_complete(step, dma_pa, DMA_PAGE_BYTES as u64);
            }
            let (id, len) = {
                let queue = queue_with(size)?;
                queue.used_elem(last & (size - 1))
            };
            LAST_USED.store(last.wrapping_add(1) as u64, Ordering::Relaxed);
            if id != 0 {
                ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
                ERRORS.fetch_add(1, Ordering::SeqCst);
                return Err("used_id");
            }
            COMMANDS.fetch_add(1, Ordering::SeqCst);
            return Ok(len);
        }
        match deadline {
            Some(limit) => {
                if now_ticks() > limit {
                    TIMEOUTS.fetch_add(1, Ordering::SeqCst);
                    return Err("timeout");
                }
            }
            None => {
                spins += 1;
                if spins > POLL_SPIN_LIMIT {
                    TIMEOUTS.fetch_add(1, Ordering::SeqCst);
                    return Err("timeout");
                }
            }
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
/// đặt); sau `SET_SCANOUT` nó trả đúng cỡ resource. Đó là phép đo thật của TAP `gpu display`.
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
/// của khách — nó **không** đổi khi `SET_SCANOUT`. Nếu thiết bị chấp nhận cả `resource_id` rác thì
/// "SET_SCANOUT thành công" **không** chứng minh được gì; ca âm này chứng minh thiết bị thật sự
/// kiểm id. Bằng chứng **thật** rằng đã vẽ là điểm ảnh trong PPM của runner.
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
/// lỗi KT-271 đã **đo được** trên PPM trước khi vá.
///
/// **Trước** khi gửi lệnh, làm sạch cache của đúng vùng `rect` (xem
/// [`clean_framebuffer_rect`]) — thiết bị đọc bằng DMA, không qua cache của CPU.
pub fn transfer_to_host_2d(rect: Rect) -> Result<(), &'static str> {
    clean_framebuffer_rect(rect);
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
/// [`stats`]`.commands` tăng đúng 2 cho mỗi lần gọi, và ảnh in delta đó ra TAP.
pub fn flush_rect(rect: Rect) -> Result<(), &'static str> {
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
    READY.load(Ordering::Relaxed)
}

/// Địa chỉ ISR (chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    ISR.load(Ordering::Relaxed)
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều lệnh).
pub fn device_status() -> u8 {
    let common = COMMON.load(Ordering::Relaxed);
    if common == 0 {
        return 0;
    }
    read8(common + C_DEVICE_STATUS)
}

/// Số lệnh đã publish (một chiều) — bằng chứng ring quay vòng (`published / queue_size`).
pub fn published() -> u16 {
    PUBLISHED.load(Ordering::Relaxed) as u16
}

/// Địa chỉ `device_cfg` (chẩn đoán).
pub fn device_cfg_addr() -> u64 {
    DEVICE_CFG.load(Ordering::Relaxed)
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
