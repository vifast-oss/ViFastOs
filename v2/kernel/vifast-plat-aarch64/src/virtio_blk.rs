// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-blk** (virtio 1.0, virtio-pci modern) trên QEMU `virt` aarch64 — KT-273
//! (K4' bước 3).
//!
//! # Đường đi
//!
//! [`crate::pci::init`] (ECAM qua physmap) ⇒ BAR modern + bốn capability ⇒ tệp này lập trình
//! thanh ghi common config, dựng **split virtqueue** trong **một trang DMA 4 KiB** căn lề
//! ([`crate::asm::dma_page`]), rồi gửi request `IN`/`OUT`/`FLUSH` và **poll `used`** (bước 3
//! chưa dùng ngắt).
//!
//! # Khác riscv64 ở đúng ba điểm (đều do kiến trúc, không do phong cách)
//!
//! 1. **Địa chỉ DMA là PA, không phải VA**: sau `activate`, hart 0 chạy ở **alias nửa cao**
//!    (`KERNEL_HIGH_BASE + …`), nên `addr_of!(trang DMA)` **không** phải PA. Thiết bị DMA bằng
//!    PA ⇒ caller đổi bằng [`high_to_pa`] rồi truyền vào [`init`]. Descriptor/`avail`/`used` đều
//!    ghi bằng PA ấy.
//! 2. **Trạng thái driver bằng `Atomic*`, không `UnsafeCell`**: crate này `#![deny(unsafe_code)]`
//!    ngoài [`crate::asm`] (D2-17), nên không có `unsafe impl Sync`/`&mut *cell`. Ảnh boot chạy
//!    **một** hart trên đường driver và driver tự tuần tự hoá (một request một lúc) ⇒
//!    `load`/`store(Relaxed)` là đủ, đúng khuôn `bootmem.rs` (KT-249).
//! 3. **Trần poll bằng `CNTPCT_EL0`** (bộ đếm vật lý, luôn có) thay vì timebase của `Platform`:
//!    bước 3 **chưa** có timer per-hart (nợ §7.1 của `k4-aarch64-boot.md`), nhưng bộ đếm đọc
//!    được không cần cấu hình. `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ lùi về trần **số vòng**, ghi rõ.
//!
//! # Bất biến
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1` (bit 32) + `VIRTIO_BLK_F_FLUSH` **nếu**
//!   thiết bị có — quyết định nằm ở `vifast_virtio::blk::negotiated_features` (test host).
//! - Thứ tự spec §2.7: ghi descriptor → `avail->ring` → **`dsb sy`** → `avail->idx` → **`dsb sy`**
//!   → notify (MMIO) → poll `used->idx` → **`dsb sy`** → đọc `used->elem`/status.
//! - `used->id` **phải** khớp head descriptor (0) — lệch ⇒ đếm `id_mismatch` + lỗi, không im lặng.
//! - Request dựng bằng `vifast_virtio::blk::build_request` — **cùng** mã mà test host kiểm
//!   (`v2/lib/vifast-virtio/tests/driver_request.rs`), không ghép header bằng tay.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_virtio::blk::{
    build_request, decode_config, flush_negotiated, negotiated_features, status_to_result,
    BlkError, BLK_CONFIG_BYTES, T_FLUSH, T_IN, T_OUT,
};
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::asm;
use crate::cache;
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
/// Đây là **hợp đồng với runner** (`run-aarch64.sh` tạo đĩa bằng `printf 'VIFASTOS-KT273-S0'`):
/// ảnh in `magic=1` chỉ khi 8 byte đầu khớp đúng chữ ký này, nên "đọc được sector 0" không thể
/// là đọc ra số 0 rồi tự khen.
pub const SECTOR0_MAGIC: [u8; 8] = *b"VIFASTOS";

/// Số giây trần khi poll `used` (spec §2.7.13: không ngắt thì driver poll, phải có trần).
pub const POLL_TIMEOUT_SECS: u64 = 2;

/// Trần **số vòng** dự phòng khi `CNTFRQ_EL0 = 0` (không có trần thời gian).
pub const POLL_SPIN_LIMIT: u64 = 200_000_000;

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

// ---------------------------------------------------------------- trạng thái (atomic)
static READY: AtomicBool = AtomicBool::new(false);
static COMMON: AtomicU64 = AtomicU64::new(0);
static NOTIFY: AtomicU64 = AtomicU64::new(0);
static DEVICE_CFG: AtomicU64 = AtomicU64::new(0);
static ISR: AtomicU64 = AtomicU64::new(0);
static NOTIFY_MUL: AtomicU64 = AtomicU64::new(0);
static NOTIFY_OFF: AtomicU64 = AtomicU64::new(0);
static QUEUE_SIZE_NOW: AtomicU64 = AtomicU64::new(0);
static CAPACITY: AtomicU64 = AtomicU64::new(0);
static LAST_USED: AtomicU64 = AtomicU64::new(0);
static FLUSH_OK: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------- bộ đếm (an toàn, atomic)
static REQUESTS: AtomicU64 = AtomicU64::new(0);
static READS: AtomicU64 = AtomicU64::new(0);
static WRITES: AtomicU64 = AtomicU64::new(0);
static FLUSHES: AtomicU64 = AtomicU64::new(0);
static COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------- cache maintenance (KT-277)
//
// Bộ đếm **của chính vòng lặp phát `dc`** — nhờ vậy `# dma: … lines=` là số lệnh `dc` **đã
// chạy**, không phải một hằng viết tay. Bỏ lời gọi `dc civac` ⇒ `lines=0` ⇒ runner ĐỎ (tầng
// quan sát được trên QEMU TCG; **hiệu ứng** của cache thì TCG không mô hình hoá — ghi trung thực
// ở `v2/docs/k4-aarch64-boot.md` §11).
static CLEAN_LINES: AtomicU64 = AtomicU64::new(0);
static CLEAN_BYTES: AtomicU64 = AtomicU64::new(0);
static CLEAN_OPS: AtomicU64 = AtomicU64::new(0);
static INVAL_LINES: AtomicU64 = AtomicU64::new(0);
static INVAL_BYTES: AtomicU64 = AtomicU64::new(0);
static INVAL_OPS: AtomicU64 = AtomicU64::new(0);
static POLL_INVAL: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm cache maintenance — in vào dòng `# dma:` của ảnh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Số dòng đã `dc civac` **trước** khi bàn giao ring (tổng qua mọi request).
    pub clean_lines: u64,
    /// Số byte vùng DMA đã làm sạch (tổng, mỗi request một lần toàn trang).
    pub clean_bytes: u64,
    /// Số lần chạy chuỗi bàn giao (mỗi request một lần).
    pub clean_ops: u64,
    /// Số dòng đã `dc ivac` **sau** khi thiết bị báo hoàn tất.
    pub inval_lines: u64,
    /// Số byte vùng DMA đã bỏ cache sau khi thiết bị ghi.
    pub inval_bytes: u64,
    /// Số lần chạy chuỗi hoàn tất.
    pub inval_ops: u64,
    /// Số lần `dc ivac` một dòng trong vòng poll `used->idx`.
    pub poll_inval: u64,
}

impl CacheStats {
    /// `true` ⇔ **cả hai** chiều đã chạy ít nhất một lần và mỗi lần làm sạch đều có dòng thật.
    ///
    /// Đây là điều kiện của `ok=1` trong dòng `# dma:`: một ảnh không có đĩa (không request nào)
    /// hoặc một ảnh bỏ bước `dc` đều cho `false`.
    pub const fn ok(self) -> bool {
        self.clean_ops > 0
            && self.clean_lines > 0
            && self.inval_ops > 0
            && self.inval_lines > 0
            && self.clean_bytes > 0
            && self.inval_bytes > 0
    }
}

/// Đọc ảnh chụp bộ đếm cache maintenance.
pub fn cache_stats() -> CacheStats {
    CacheStats {
        clean_lines: CLEAN_LINES.load(Ordering::SeqCst),
        clean_bytes: CLEAN_BYTES.load(Ordering::SeqCst),
        clean_ops: CLEAN_OPS.load(Ordering::SeqCst),
        inval_lines: INVAL_LINES.load(Ordering::SeqCst),
        inval_bytes: INVAL_BYTES.load(Ordering::SeqCst),
        inval_ops: INVAL_OPS.load(Ordering::SeqCst),
        poll_inval: POLL_INVAL.load(Ordering::SeqCst),
    }
}

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

/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct BlkReport {
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
    /// `queue_notify_off` của queue 0.
    pub notify_off: u16,
    /// Địa chỉ cửa thông báo = notify + notify_off * multiplier.
    pub notify_addr: u64,
    /// `num_queues` thiết bị báo.
    pub num_queues: u16,
    /// Cỡ queue thiết bị cho phép tối đa.
    pub queue_max: u16,
    /// Cỡ queue driver đặt (= [`QUEUE_SIZE`] nếu thiết bị cho phép).
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
    /// PA trang DMA (thiết bị DMA vào đây).
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

/// Hàng rào thứ tự bộ nhớ trước/sau khi bàn giao ring cho thiết bị (spec §2.7.13: driver phải
/// có barrier trước notify; thiếu ⇒ thiết bị có thể đọc `avail` cũ).
///
/// `dsb sy` (không phải `dmb`): DMA của thiết bị nằm **ngoài** domain shareability của CPU nên
/// cần barrier **đầy đủ** — cùng lý do `fence rw, rw` bên riscv64.
#[inline]
fn fence() {
    asm::dsb_sy();
}

/// Trang DMA của ảnh (4 KiB căn lề) — vùng virtqueue + request + data + status.
fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    asm::dma_page()
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

// ---------------------------------------------------------------- khởi tạo

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
///
/// `dma_pa` là **PA** của trang DMA (caller đổi alias nửa cao → PA bằng [`high_to_pa`]); phải
/// căn 4 KiB vì descriptor/`avail`/`used` đều suy từ nó bằng phép cộng offset.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<BlkReport, &'static str> {
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
    let desc_pa = dma_pa + layout.desc_off as u64;
    let avail_pa = dma_pa + layout.avail_off as u64;
    let used_pa = dma_pa + layout.used_off as u64;
    let req_pa = dma_pa + REQ_OFF as u64;
    let data_pa = dma_pa + DATA_OFF as u64;
    let status_pa = dma_pa + STATUS_OFF as u64;
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
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    COMMON.store(common, Ordering::Relaxed);
    NOTIFY.store(notify, Ordering::Relaxed);
    DEVICE_CFG.store(device_cfg, Ordering::Relaxed);
    ISR.store(isr, Ordering::Relaxed);
    NOTIFY_MUL.store(notify_mul as u64, Ordering::Relaxed);
    NOTIFY_OFF.store(notify_off as u64, Ordering::Relaxed);
    QUEUE_SIZE_NOW.store(queue_size as u64, Ordering::Relaxed);
    CAPACITY.store(capacity, Ordering::Relaxed);
    LAST_USED.store(0, Ordering::Relaxed);
    FLUSH_OK.store(flush_ok, Ordering::Relaxed);
    READY.store(true, Ordering::Relaxed);

    Ok(BlkReport {
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
        flush_ok,
        status,
        config_generation,
        capacity,
        size_max: config.size_max,
        seg_max: config.seg_max,
        blk_size: config.blk_size,
        dma_pa,
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
    CAPACITY.load(Ordering::Relaxed)
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
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
) -> Result<(), BlkError> {
    if !READY.load(Ordering::Relaxed) {
        return Err(BlkError::NotReady);
    }
    let dma_pa = DMA_PA.load(Ordering::Relaxed);
    let req_pa = dma_pa + REQ_OFF as u64;
    let data_pa = dma_pa + DATA_OFF as u64;
    let status_pa = dma_pa + STATUS_OFF as u64;
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
    let qsize = QUEUE_SIZE_NOW.load(Ordering::Relaxed) as u16;
    let status_index = req.status_index;
    {
        let mut q = queue_with(qsize)?;
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
    // ---- **KT-277**: bàn giao vùng DMA cho thiết bị theo [`cache::HANDOFF_SEQUENCE`]:
    // `dc civac` từng dòng (ghi bản bẩn ra PoC + bỏ bản sao) → `dsb ish` → `dsb sy` → notify.
    // Thứ tự là **dữ liệu** (test host khẳng định), không phải ba dòng rời có thể xoá lẫn nhau.
    let notify = NOTIFY.load(Ordering::Relaxed);
    let notify_off = NOTIFY_OFF.load(Ordering::Relaxed) as u16;
    let notify_mul = NOTIFY_MUL.load(Ordering::Relaxed) as u32;
    let notify_addr = notify + notify_off as u64 * notify_mul as u64;
    let mut clean_lines = 0u64;
    for step in cache::HANDOFF_SEQUENCE {
        clean_lines += cache::apply_handoff(step, dma_pa, DMA_PAGE_BYTES as u64, notify_addr);
    }
    CLEAN_LINES.fetch_add(clean_lines, Ordering::SeqCst);
    CLEAN_BYTES.fetch_add(DMA_PAGE_BYTES as u64, Ordering::SeqCst);
    CLEAN_OPS.fetch_add(1, Ordering::SeqCst);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);

    // ---- poll used (một chiều) — offset lấy từ **qsize thật** của queue.
    let used_pa = dma_pa + VqLayout::new(qsize).ok_or(BlkError::NotReady)?.used_off as u64;
    let used_idx_addr = used_pa + 2;
    let deadline = poll_deadline();
    let mut spins = 0u64;
    loop {
        // **KT-277**: `dc ivac` dòng chứa `used->idx` **trước** mỗi lần đọc. Thiết bị ghi qua
        // DMA (không đi qua cache của CPU), nên đọc mà không bỏ cache có thể thấy mãi giá trị cũ
        // và poll treo tới hết trần. An toàn: `HANDOFF_SEQUENCE` vừa `dc civac` toàn trang nên
        // mọi dòng đều **sạch** — `dc ivac` chỉ bỏ bản sao đã sạch, không mất dữ liệu chưa ghi.
        cache::invalidate_line(used_idx_addr);
        POLL_INVAL.fetch_add(1, Ordering::SeqCst);
        let idx = asm::mmio_read_u16(used_idx_addr);
        let last = LAST_USED.load(Ordering::Relaxed) as u16;
        if idx != last {
            // **KT-277**: thiết bị đã ghi xong — bỏ cache **trước** khi đọc `used->elem`,
            // byte status và đệm dữ liệu, theo [`cache::COMPLETE_SEQUENCE`].
            let mut inval_lines = 0u64;
            for step in cache::COMPLETE_SEQUENCE {
                inval_lines += cache::apply_complete(step, dma_pa, DMA_PAGE_BYTES as u64);
            }
            INVAL_LINES.fetch_add(inval_lines, Ordering::SeqCst);
            INVAL_BYTES.fetch_add(DMA_PAGE_BYTES as u64, Ordering::SeqCst);
            INVAL_OPS.fetch_add(1, Ordering::SeqCst);
            let element = {
                let q = queue_with(qsize)?;
                q.used_elem(last & (qsize - 1))
            };
            LAST_USED.store(last.wrapping_add(1) as u64, Ordering::Relaxed);
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
        match deadline {
            Some(limit) => {
                if now_ticks() > limit {
                    TIMEOUTS.fetch_add(1, Ordering::SeqCst);
                    ERRORS.fetch_add(1, Ordering::SeqCst);
                    return Err(BlkError::Timeout);
                }
            }
            None => {
                spins += 1;
                if spins > POLL_SPIN_LIMIT {
                    TIMEOUTS.fetch_add(1, Ordering::SeqCst);
                    ERRORS.fetch_add(1, Ordering::SeqCst);
                    return Err(BlkError::Timeout);
                }
            }
        }
        core::hint::spin_loop();
    }
}

/// PA của trang DMA — đặt lúc [`init`] (caller đã đổi alias nửa cao → PA).
static DMA_PA: AtomicU64 = AtomicU64::new(0);

/// Đọc một sector, **kiểm dải trước** (không phát request ngoài dung lượng).
pub fn read_sector(lba: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    if lba as u64 >= capacity_sectors() {
        return Err(BlkError::OutOfRange);
    }
    READS.fetch_add(1, Ordering::SeqCst);
    submit(T_IN, lba as u64, None, Some(out))
}

/// Ghi một sector, kiểm dải trước.
pub fn write_sector(lba: u32, data: &[u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    if lba as u64 >= capacity_sectors() {
        return Err(BlkError::OutOfRange);
    }
    WRITES.fetch_add(1, Ordering::SeqCst);
    submit(T_OUT, lba as u64, Some(data), None)
}

/// Đẩy cache ghi của thiết bị xuống phương tiện (`VIRTIO_BLK_T_FLUSH`) — **thật**, không no-op.
pub fn flush() -> Result<(), BlkError> {
    if !READY.load(Ordering::Relaxed) {
        return Err(BlkError::NotReady);
    }
    // **Không** gửi lệnh của feature chưa đàm phán.
    if !FLUSH_OK.load(Ordering::Relaxed) {
        return Err(BlkError::Unsupported);
    }
    FLUSHES.fetch_add(1, Ordering::SeqCst);
    submit(T_FLUSH, 0, None, None)
}

/// **Chỉ cho ca âm của TAP**: phát request đọc **vượt** kiểm dải để chứng minh thiết bị trả
/// `IOERR` thật (và không treo). Không dùng trong đường FS.
pub fn read_sector_force(lba: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), BlkError> {
    READS.fetch_add(1, Ordering::SeqCst);
    submit(T_IN, lba as u64, None, Some(out))
}

/// Địa chỉ ISR (đọc 1 byte để lấy lý do ngắt — bước 3 chưa dùng ngắt, chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    ISR.load(Ordering::Relaxed)
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều request).
pub fn device_status() -> u8 {
    let common = COMMON.load(Ordering::Relaxed);
    if common == 0 {
        return 0;
    }
    read8(common + C_DEVICE_STATUS)
}

/// Đọc `config_generation` hiện tại.
pub fn config_generation() -> u8 {
    let common = COMMON.load(Ordering::Relaxed);
    if common == 0 {
        return 0;
    }
    read8(common + C_CONFIG_GENERATION)
}

/// Địa chỉ `device_cfg` (chẩn đoán).
pub fn device_cfg_addr() -> u64 {
    DEVICE_CFG.load(Ordering::Relaxed)
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
};
