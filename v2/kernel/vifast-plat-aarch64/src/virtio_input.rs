// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-input** (bàn phím, virtio 1.0, virtio-pci modern) trên QEMU `virt` aarch64 —
//! KT-283 (K4' bước 3, D1 phần 3 trên aarch64).
//!
//! # Đường đi
//!
//! [`crate::pci::init_for`]`(VirtioKind::Input)` (ECAM của KT-273, **không** nhân đôi code PCI) ⇒
//! BAR modern + bốn capability ⇒ tệp này lập trình common config, dựng **hai** split virtqueue
//! (`event` = queue 0 — §5.8.2, `status` = queue 1), đọc `virtio_input_config` (tên/danh
//! tính/bitmap `EV_BITS`), rồi `poll_event`/`poll_event_wait` và **poll `used`** (chưa dùng ngắt).
//!
//! # Khác khuôn riscv64 (KT-255) ở đúng bốn điểm — đều do kiến trúc, không do phong cách
//!
//! 1. **Địa chỉ DMA là PA, không phải VA**: sau `activate`, hart 0 chạy ở **alias nửa cao**
//!    (`KERNEL_HIGH_BASE + …`), nên `addr_of!(trang DMA)` **không** phải PA. Thiết bị DMA bằng
//!    PA ⇒ caller đổi bằng [`high_to_pa`] rồi truyền vào [`init`]. Descriptor/`avail`/`used` đều
//!    ghi bằng PA ấy.
//! 2. **Trạng thái driver bằng `Atomic*`, không `UnsafeCell`**: crate này `#![deny(unsafe_code)]`
//!    ngoài [`crate::asm`] (D2-17), nên không có `unsafe impl Sync`/`&mut *cell`. Ảnh boot chạy
//!    **một** hart trên đường driver và driver tự tuần tự hoá (một sự kiện một lúc) ⇒
//!    `load`/`store(Relaxed)` là đủ, đúng khuôn `virtio_blk.rs`/`virtio_net.rs`/`virtio_gpu.rs`.
//! 3. **Trần poll bằng `CNTPCT_EL0`** (bộ đếm vật lý, luôn có) thay vì timebase của `Platform`:
//!    bước 3 **chưa** có timer per-hart (nợ §7.1 của `k4-aarch64-boot.md`), nhưng bộ đếm đọc
//!    được không cần cấu hình. `CNTFRQ_EL0 = 0` (firmware lạ) ⇒ lùi về trần **số vòng**, ghi rõ.
//! 4. **Cache maintenance trước/sau DMA** (KT-277): vùng DMA nằm trong `.bss` map **Normal
//!    cacheable**, còn thiết bị đọc/ghi bằng DMA **không** đi qua cache của CPU. Dùng **cùng**
//!    [`cache::HANDOFF_SEQUENCE`]/[`cache::COMPLETE_SEQUENCE`] của KT-277, không viết bản thứ hai.
//!
//! # Hai queue `event`/`status` — và vì sao `status` **không** arm đệm
//!
//! Spec 1.2 §5.8.2: thiết bị input có **đúng hai** queue — `event` (thiết bị → driver: phím) và
//! `status` (driver → thiết bị: LED). Driver KT-283 **đọc** phím nên chỉ `event` cần đệm; nhưng
//! một thiết bị virtio 1.0 **đòi** driver đặt `queue_size` + địa chỉ cho **mọi** queue trước
//! `DRIVER_OK` nếu driver muốn dùng chúng — và bật `status` là cách chứng minh driver hiểu
//! topology hai queue thay vì coi thiết bị chỉ có một. `status` **không** có đệm ⇒ thiết bị
//! không bao giờ ghi vào ring ấy (không có descriptor nào để ghi), nên bật nó **không** tốn
//! thêm một byte DMA nào ngoài ring 430 B trong **cùng** trang 4 KiB.
//!
//! # Bất biến
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1`; **không** `VIRTIO_INPUT_F_REP` — quyết
//!   định nằm ở `vifast_virtio::input::negotiated_features` (test host), không viết tay hai chỗ
//!   (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **`dsb sy`** → ghi `avail->idx` →
//!   **`dsb sy`** → notify (MMIO) → poll `used->idx` → **`dsb sy`** → đọc `used->elem`.
//! - `used->id` **phải** là descriptor đã arm (< `queue_size`) — lệch ⇒ đếm `id_mismatch` + lỗi,
//!   không im lặng. `used->len` **phải** đúng 8 (`event_len`) — thiết bị ghi một
//!   `virtio_input_event` vào mỗi đệm 8 byte.
//! - **Arm nhiều đệm** ([`EVENT_BUFS`]): QEMU gom sự kiện tới `EV_SYN`/`SYN_REPORT` rồi mới lấy
//!   **một đệm cho mỗi sự kiện**; thiếu đệm thì **cả lô** bị huỷ ⇒ mất phím. Một lô
//!   `sendkey shift-a` là 5 sự kiện.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); config thiếu/không khớp
//!   `select`/`subsel` ⇒ `Err`; `used->len` vô lý ⇒ `Io` + đếm `errors`.
//!
//! # Bảng chuyển mã: **không** viết lại
//!
//! Mã phím → ASCII là của `vifast_virtio::input` ([`vifast_virtio::input::key_to_ascii`] /
//! [`vifast_virtio::input::KeyState`], 16 test host bên riscv64). Tệp này chỉ đọc
//! `virtio_input_event` 8 byte và trả [`InputEvent`] — người gọi (ảnh) dùng `KeyState`.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::input::{
    decode_devids, decode_name, event_len, key_bit, negotiated_features, InputConfigHeader,
    InputDevIds, InputError, InputEvent, CFG_EV_BITS, CFG_HEADER_BYTES, CFG_ID_DEVIDS, CFG_ID_NAME,
    CFG_SELECT_OFF, CFG_SUBSEL_OFF, CFG_U_BYTES, CFG_U_OFF, EVENT_BYTES, EV_KEY, KEY_A,
    MIN_QUEUE_SIZE, QUEUES_REQUIRED, Q_EVENT, Q_STATUS,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::asm;
use crate::cache;
use crate::pci::PciReport;

/// Số descriptor của queue `event` — **cùng** hằng của crate thuần
/// (`vifast_virtio::input::QUEUE_SIZE`), không chép lại số 16 (D2-110B).
pub const QUEUE_SIZE: u16 = vifast_virtio::input::QUEUE_SIZE;

/// Số đệm **8 byte** driver arm sẵn trên queue `event` — cùng hằng của crate thuần.
pub const EVENT_BUFS: usize = vifast_virtio::input::EVENT_BUFS;

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `event` trong trang DMA.
pub const RING_OFF: usize = 0x000;
/// Offset ring `status` trong trang DMA (queue 1 — bật nhưng **không** arm đệm).
pub const STATUS_RING_OFF: usize = 0x200;
/// Offset vùng đệm sự kiện (mỗi đệm [`EVENT_BYTES`] byte, device-writable).
pub const BUF_OFF: usize = 0x400;
/// Cỡ tối đa của một `virtio_input_config` đọc được (union 128 byte).
pub const NAME_MAX_BYTES: usize = CFG_U_BYTES;
/// Cỡ bitmap `EV_BITS` tối đa (union 128 byte).
pub const EV_BITS_BYTES: usize = CFG_U_BYTES;

/// Số giây trần khi poll `used` (spec §2.7.13: không ngắt thì driver poll, phải có trần).
pub const POLL_TIMEOUT_SECS: u64 = 2;
/// Trần **số vòng** dự phòng khi `CNTFRQ_EL0 = 0` (không có trần thời gian).
pub const POLL_SPIN_LIMIT: u64 = 200_000_000;

// `const` assert: hai ring + vùng đệm phải nằm gọn trong **một** trang DMA 4 KiB, và hai ring
// phải **rời nhau** — lệch một hằng là **không biên dịch được** thay vì driver thứ hai ghi đè
// `avail->idx` của driver thứ nhất (hỏng im lặng, chỉ lộ khi thứ tự gọi đổi).
const _: () = {
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(l) => l,
        None => panic!("queue size khong hop le"),
    };
    assert!(RING_OFF + layout.end <= STATUS_RING_OFF);
    assert!(STATUS_RING_OFF + layout.end <= BUF_OFF);
    assert!(BUF_OFF + EVENT_BUFS * EVENT_BYTES <= DMA_PAGE_BYTES);
    assert!(EVENT_BUFS as u16 <= QUEUE_SIZE);
};

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
static STATUS_QUEUE_SIZE: AtomicU64 = AtomicU64::new(0);
static STATUS_NOTIFY_OFF: AtomicU64 = AtomicU64::new(0);
static LAST_USED: AtomicU64 = AtomicU64::new(0);
static PUBLISHED: AtomicU64 = AtomicU64::new(0);
static DMA_PA: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------- bộ đếm (an toàn, atomic)
static EVENTS: AtomicU64 = AtomicU64::new(0);
static SYNCS: AtomicU64 = AtomicU64::new(0);
static KEY_DOWNS: AtomicU64 = AtomicU64::new(0);
static KEY_UPS: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);
/// Số dòng đã `dc civac` **trước** khi bàn giao ring (tổng qua mọi lần notify).
static CLEAN_LINES: AtomicU64 = AtomicU64::new(0);
/// Số byte đã làm sạch (tổng) — bằng chứng vùng DMA **không** rỗng.
static CLEAN_BYTES: AtomicU64 = AtomicU64::new(0);
/// Số lần chạy [`cache::HANDOFF_SEQUENCE`] (mỗi lần notify một lần).
static CLEAN_OPS: AtomicU64 = AtomicU64::new(0);
/// Số dòng đã `dc ivac` **sau** khi thiết bị báo hoàn tất (tổng).
static INVAL_LINES: AtomicU64 = AtomicU64::new(0);
/// Số lần chạy [`cache::COMPLETE_SEQUENCE`].
static INVAL_OPS: AtomicU64 = AtomicU64::new(0);

/// Ảnh chụp bộ đếm để in evidence (đọc một lần, không tách rời từng số).
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Số sự kiện đã nhận (mọi loại).
    pub events: u64,
    /// Số `EV_SYN`/`SYN_REPORT` (mỗi lô QEMU kết thúc bằng một cái).
    pub syncs: u64,
    /// Số `EV_KEY` có `value != 0` (nhấn).
    pub key_downs: u64,
    /// Số `EV_KEY` có `value == 0` (nhả).
    pub key_ups: u64,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lỗi driver (`used->len`/`used->id` vô lý, sự kiện không giải mã được).
    pub errors: u64,
    /// Số lần poll quá hạn (phải 0 ở đường thành công).
    pub timeouts: u64,
    /// Số lần `used->id` lệch descriptor đã arm (phải 0).
    pub id_mismatch: u64,
    /// Số dòng đã `dc civac` trước khi bàn giao ring.
    pub clean_lines: u64,
    /// Số byte đã làm sạch.
    pub clean_bytes: u64,
    /// Số lần chạy chuỗi bàn giao (mỗi lần notify).
    pub clean_ops: u64,
    /// Số dòng đã `dc ivac` sau khi thiết bị ghi.
    pub inval_lines: u64,
    /// Số lần chạy chuỗi hoàn tất.
    pub inval_ops: u64,
}

impl Stats {
    /// `true` ⇔ đường đi sạch: có sự kiện, không lỗi/timeout/lệch id, và **đã** làm sạch vùng DMA
    /// ít nhất một lần (bỏ `dc civac` ⇒ `false`).
    pub const fn ok(self) -> bool {
        self.events > 0
            && self.errors == 0
            && self.timeouts == 0
            && self.id_mismatch == 0
            && self.clean_ops > 0
            && self.clean_lines > 0
            && self.clean_bytes > 0
            && self.inval_ops > 0
            && self.inval_lines > 0
    }
}

/// Đọc ảnh chụp bộ đếm.
pub fn stats() -> Stats {
    Stats {
        events: EVENTS.load(Ordering::SeqCst),
        syncs: SYNCS.load(Ordering::SeqCst),
        key_downs: KEY_DOWNS.load(Ordering::SeqCst),
        key_ups: KEY_UPS.load(Ordering::SeqCst),
        notifies: NOTIFIES.load(Ordering::SeqCst),
        errors: ERRORS.load(Ordering::SeqCst),
        timeouts: TIMEOUTS.load(Ordering::SeqCst),
        id_mismatch: ID_MISMATCH.load(Ordering::SeqCst),
        clean_lines: CLEAN_LINES.load(Ordering::SeqCst),
        clean_bytes: CLEAN_BYTES.load(Ordering::SeqCst),
        clean_ops: CLEAN_OPS.load(Ordering::SeqCst),
        inval_lines: INVAL_LINES.load(Ordering::SeqCst),
        inval_ops: INVAL_OPS.load(Ordering::SeqCst),
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

/// Trang DMA của ảnh (4 KiB căn lề) — hai virtqueue + vùng đệm sự kiện.
fn page() -> &'static mut [u8; DMA_PAGE_BYTES] {
    asm::dma_page_input()
}

/// Đổi một địa chỉ **nửa cao** của ảnh về **PA** liên kết.
///
/// Sau khi nhảy nửa cao, `adrp` cho alias `KERNEL_HIGH_BASE + (pa - text_start)`; PA thật là
/// `text_start_pa + (va - KERNEL_HIGH_BASE)`. Chỉ đúng cho địa chỉ **trong ảnh** — trang DMA
/// nằm trong `.bss` nên thuộc trường hợp này (cùng công thức `virtio_blk`/`virtio_gpu`).
pub const fn high_to_pa(va: u64, text_start_pa: u64) -> u64 {
    text_start_pa + (va - crate::split_mmu::KERNEL_HIGH_BASE)
}

/// Bộ đếm vật lý hiện tại (`CNTPCT_EL0`).
pub fn now_ticks() -> u64 {
    asm::read_cntpct_el0()
}

/// Số tick `CNTPCT_EL0` của `secs` giây; `None` nếu `CNTFRQ_EL0 = 0` (firmware không khai tần số).
pub fn ticks_for_secs(secs: u64) -> Option<u64> {
    let freq = asm::read_cntfrq_el0();
    if freq == 0 {
        None
    } else {
        Some(secs.saturating_mul(freq))
    }
}

/// Hạn chót poll: `Some(deadline)` nếu có tần số bộ đếm, `None` nếu `CNTFRQ_EL0 = 0`.
fn poll_deadline() -> Option<u64> {
    ticks_for_secs(POLL_TIMEOUT_SECS).map(|ticks| now_ticks().wrapping_add(ticks))
}

// ---------------------------------------------------------------- báo cáo khởi tạo
/// Mọi số đo của lần khởi tạo driver — **đọc lại từ thiết bị**, in nguyên văn vào TAP.
#[derive(Clone, Copy, Debug)]
pub struct InputReport {
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
    /// `queue_notify_off` của queue `event`.
    pub notify_off: u16,
    /// Địa chỉ cửa thông báo `event` = notify + off * multiplier.
    pub notify_addr: u64,
    /// `num_queues` thiết bị báo (QEMU: 2 — `event` + `status`).
    pub num_queues: u16,
    /// Cỡ queue tối đa thiết bị cho phép (queue `event`).
    pub queue_max: u16,
    /// Cỡ queue driver đặt cho `event`.
    pub queue_size: u16,
    /// Cỡ queue driver đặt cho `status` (0 nếu thiết bị không có queue thứ hai).
    pub status_queue_size: u16,
    /// `queue_notify_off` của queue `status`.
    pub status_notify_off: u16,
    /// Feature thiết bị (64 bit, đọc hai trang).
    pub device_features: u64,
    /// Feature driver đã ghi (đọc lại).
    pub driver_features: u64,
    /// `device_status` sau DRIVER_OK.
    pub status: u8,
    /// `config_generation` đọc **sau** khi đọc config (chỉ để làm bằng chứng — với input, mỗi lần
    /// ghi `select`/`subsel` **đổi** generation nên **không** dùng để kiểm tính nguyên vẹn).
    pub config_generation: u8,
    /// Tên thiết bị đọc từ `select = ID_NAME` (đã bỏ NUL cuối).
    pub name: [u8; NAME_MAX_BYTES],
    /// Số byte tên có nghĩa.
    pub name_len: u8,
    /// Danh tính thiết bị (`select = ID_DEVIDS`).
    pub devids: InputDevIds,
    /// Bitmap `EV_BITS` với `subsel = EV_KEY`.
    pub ev_bits: [u8; EV_BITS_BYTES],
    /// Số byte bitmap có nghĩa.
    pub ev_bits_len: u8,
    /// `true` ⇔ bitmap `EV_BITS` bật bit `KEY_A` (thiết bị **có** phím ta sắp bơm).
    pub key_a_supported: bool,
    /// PA trang DMA.
    pub dma_pa: u64,
    /// PA bảng descriptor của queue `event`.
    pub desc: u64,
    /// PA khối `avail` của queue `event`.
    pub avail: u64,
    /// PA khối `used` của queue `event`.
    pub used: u64,
    /// PA vùng đệm sự kiện.
    pub buf_pa: u64,
    /// Số đệm đã arm lúc init.
    pub armed: u16,
}

impl InputReport {
    /// Tên thiết bị dạng `&str` (rỗng nếu thiết bị không có tên — nhưng `init` đã fail-closed).
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len as usize]).unwrap_or("?")
    }
}

/// Ghi `select`/`subsel` vào device config rồi đọc lại 8 byte đầu.
///
/// Thiết bị **phải** trả đúng cặp `(select, subsel)` đã ghi — nếu không, ta đang đọc một view khác
/// (QEMU trả toàn 0 khi không có config khớp) ⇒ fail-closed thay vì dùng dữ liệu rác.
fn select_config(
    device_cfg: u64,
    select: u8,
    subsel: u8,
) -> Result<InputConfigHeader, &'static str> {
    write8(device_cfg + CFG_SELECT_OFF as u64, select);
    write8(device_cfg + CFG_SUBSEL_OFF as u64, subsel);
    let mut raw = [0u8; CFG_HEADER_BYTES];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = read8(device_cfg + index as u64);
    }
    let header = InputConfigHeader::parse(&raw).ok_or("config")?;
    if header.select != select || header.subsel != subsel {
        return Err("config_select");
    }
    Ok(header)
}

/// Đọc `n` byte union (offset 8) của view đang chọn vào `out`; trả số byte đã đọc.
fn read_config_payload(device_cfg: u64, header: &InputConfigHeader, out: &mut [u8]) -> usize {
    let count = header.payload_bytes().min(out.len());
    for (index, byte) in out.iter_mut().take(count).enumerate() {
        *byte = read8(device_cfg + CFG_U_OFF as u64 + index as u64);
    }
    count
}

/// Khởi tạo driver từ kết quả PCI. Lỗi trả **tên chặng** (`&'static str`) để TAP in ra lý do
/// đo được — không PANIC im lặng.
///
/// `dma_pa` là **PA** của trang DMA (caller đổi alias nửa cao → PA bằng [`high_to_pa`]); phải
/// căn 4 KiB vì descriptor/`avail`/`used` đều suy từ nó bằng phép cộng offset.
pub fn init(pci: &PciReport, dma_pa: u64) -> Result<InputReport, &'static str> {
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
    if RING_OFF + layout.total_bytes() > STATUS_RING_OFF
        || STATUS_RING_OFF + layout.total_bytes() > BUF_OFF
        || BUF_OFF + EVENT_BUFS * EVENT_BYTES > DMA_PAGE_BYTES
    {
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
    // ---- 6) hai queue: `event` = 0 (đọc phím) và `status` = 1 (LED — bật, không arm đệm).
    let num_queues = read16(common + C_NUM_QUEUES);
    if num_queues < QUEUES_REQUIRED {
        return Err("queues");
    }
    // Spec §5.8.2: thiết bị input **có** hai queue. Thiếu queue thứ hai ⇒ thiết bị không đúng
    // hợp đồng ⇒ fail-closed thay vì lặng lẽ bỏ qua (một driver "một queue" sẽ bỏ LED mà không
    // ai biết).
    if num_queues < 2 {
        return Err("queues2");
    }
    write16(common + C_QUEUE_SELECT, Q_EVENT);
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
    // ---- queue `status` (1): cùng cỡ, ring riêng trong **cùng** trang DMA.
    write16(common + C_QUEUE_SELECT, Q_STATUS);
    let status_queue_max = read16(common + C_QUEUE_SIZE);
    if status_queue_max < MIN_QUEUE_SIZE || !status_queue_max.is_power_of_two() {
        return Err("status_queue_size");
    }
    let status_queue_size = QUEUE_SIZE.min(status_queue_max);
    write16(common + C_QUEUE_SIZE, status_queue_size);
    if read16(common + C_QUEUE_SIZE) != status_queue_size {
        return Err("status_queue_size");
    }
    let status_layout = VqLayout::new(status_queue_size).ok_or("layout")?;
    let status_base = dma_pa + STATUS_RING_OFF as u64;
    write64(
        common + C_QUEUE_DESC,
        status_base + status_layout.desc_off as u64,
    );
    write64(
        common + C_QUEUE_DRIVER,
        status_base + status_layout.avail_off as u64,
    );
    write64(
        common + C_QUEUE_DEVICE,
        status_base + status_layout.used_off as u64,
    );
    fence();
    write16(common + C_QUEUE_ENABLE, 1);
    if read16(common + C_QUEUE_ENABLE) != 1 {
        return Err("status_queue_enable");
    }
    let status_notify_off = read16(common + C_QUEUE_NOTIFY_OFF);
    // ---- 7) DRIVER_OK
    write8(
        common + C_DEVICE_STATUS,
        ACK | DRIVER | FEATURES_OK | DRIVER_OK,
    );
    let status = read8(common + C_DEVICE_STATUS);
    if !status_sequence_ok(status) {
        return Err("driver_ok");
    }
    // ---- 8) device config: tên + danh tính + bitmap EV_BITS (đọc **từ thiết bị**)
    let name_header = select_config(device_cfg, CFG_ID_NAME, 0)?;
    let mut name = [0u8; NAME_MAX_BYTES];
    let payload_len = read_config_payload(device_cfg, &name_header, &mut name);
    // `size` của QEMU **gồm** NUL cuối (`sizeof("QEMU Virtio Keyboard")` = 21) ⇒ `name_len` phải
    // là số byte **có nghĩa** sau khi bỏ NUL, không phải `size` — nếu không, `name()` trả chuỗi có
    // NUL và dòng evidence `# input name:` bị cắt ở NUL (đo được bên riscv64 khi dựng KT-255).
    let name_len = decode_name(&name[..payload_len]).ok_or("name")?.len();
    let ids_header = select_config(device_cfg, CFG_ID_DEVIDS, 0)?;
    let mut ids_raw = [0u8; 8];
    let ids_len = read_config_payload(device_cfg, &ids_header, &mut ids_raw);
    let devids = decode_devids(&ids_raw[..ids_len]).ok_or("devids")?;
    let ev_header = select_config(device_cfg, CFG_EV_BITS, EV_KEY as u8)?;
    let mut ev_bits = [0u8; EV_BITS_BYTES];
    let ev_bits_len = read_config_payload(device_cfg, &ev_header, &mut ev_bits);
    let key_a_supported = key_bit(&ev_bits[..ev_bits_len], KEY_A);
    let config_generation = read8(common + C_CONFIG_GENERATION);
    // ---- 9) lưu trạng thái (khối riêng: không giữ `&mut` qua lời gọi `arm_all`)
    DMA_PA.store(dma_pa, Ordering::Relaxed);
    COMMON.store(common, Ordering::Relaxed);
    NOTIFY.store(notify, Ordering::Relaxed);
    DEVICE_CFG.store(device_cfg, Ordering::Relaxed);
    ISR.store(isr, Ordering::Relaxed);
    NOTIFY_MUL.store(notify_mul as u64, Ordering::Relaxed);
    NOTIFY_OFF.store(notify_off as u64, Ordering::Relaxed);
    QUEUE_SIZE_NOW.store(queue_size as u64, Ordering::Relaxed);
    STATUS_QUEUE_SIZE.store(status_queue_size as u64, Ordering::Relaxed);
    STATUS_NOTIFY_OFF.store(status_notify_off as u64, Ordering::Relaxed);
    LAST_USED.store(0, Ordering::Relaxed);
    PUBLISHED.store(0, Ordering::Relaxed);
    READY.store(true, Ordering::Relaxed);
    // ---- 10) arm **nhiều** đệm ngay: sự kiện host gửi tới phải có đệm trước khi người gọi dùng
    // driver (QEMU huỷ **cả lô** nếu thiếu đệm — xem đầu tệp).
    let armed = arm_all()?;
    Ok(InputReport {
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
        status_queue_size,
        status_notify_off,
        device_features,
        driver_features,
        status,
        config_generation,
        name,
        name_len: name_len as u8,
        devids,
        ev_bits,
        ev_bits_len: ev_bits_len as u8,
        key_a_supported,
        dma_pa,
        desc: desc_pa,
        avail: avail_pa,
        used: used_pa,
        buf_pa: dma_pa + BUF_OFF as u64,
        armed,
    })
}

/// Bọc ring của queue `event` thành split queue với cỡ **thật** của nó.
fn queue_with(size: u16) -> Result<SplitQueue<'static>, InputError> {
    if size < MIN_QUEUE_SIZE {
        return Err(InputError::NotReady);
    }
    let layout = VqLayout::new(size).ok_or(InputError::NotReady)?;
    SplitQueue::new(&mut page()[RING_OFF..RING_OFF + layout.total_bytes()], size)
        .ok_or(InputError::NotReady)
}

/// Bọc ring của queue `status` (bật nhưng **không** arm đệm — chỉ để chứng minh topology).
fn status_queue_with(size: u16) -> Result<SplitQueue<'static>, InputError> {
    if size < MIN_QUEUE_SIZE {
        return Err(InputError::NotReady);
    }
    let layout = VqLayout::new(size).ok_or(InputError::NotReady)?;
    SplitQueue::new(
        &mut page()[STATUS_RING_OFF..STATUS_RING_OFF + layout.total_bytes()],
        size,
    )
    .ok_or(InputError::NotReady)
}

/// Bàn giao vùng DMA cho thiết bị theo [`cache::HANDOFF_SEQUENCE`] rồi notify queue `event`.
///
/// Thứ tự là **dữ liệu** (test host khẳng định), không phải ba dòng rời có thể xoá lẫn nhau:
/// `dc civac` từng dòng (ghi bản bẩn ra PoC + bỏ bản sao) → `dsb ish` → `dsb sy` → notify.
fn handoff_and_notify() {
    let dma_pa = DMA_PA.load(Ordering::Relaxed);
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
}

/// Đăng descriptor `index` (đệm 8 byte device-writable) và publish — gọi lúc init và sau mỗi sự
/// kiện nhận.
fn arm_one(index: u16) -> bool {
    let size = QUEUE_SIZE_NOW.load(Ordering::Relaxed) as u16;
    if size < MIN_QUEUE_SIZE || index >= size {
        return false;
    }
    let dma_pa = DMA_PA.load(Ordering::Relaxed);
    {
        let Ok(mut queue) = queue_with(size) else {
            return false;
        };
        if !queue.set_desc(
            index,
            Desc {
                addr: dma_pa + BUF_OFF as u64 + index as u64 * EVENT_BYTES as u64,
                len: EVENT_BYTES as u32,
                flags: DescFlags::WRITE,
                next: 0,
            },
        ) {
            return false;
        }
        queue.push_avail(index);
    }
    handoff_and_notify();
    PUBLISHED.fetch_add(1, Ordering::SeqCst);
    true
}

/// Arm [`EVENT_BUFS`] đệm đầu tiên rồi notify **một** lần (init).
fn arm_all() -> Result<u16, &'static str> {
    let size = QUEUE_SIZE_NOW.load(Ordering::Relaxed) as u16;
    if size < EVENT_BUFS as u16 {
        return Err("arm");
    }
    let dma_pa = DMA_PA.load(Ordering::Relaxed);
    {
        let mut queue = queue_with(size).map_err(|_| "arm")?;
        for index in 0..EVENT_BUFS as u16 {
            if !queue.set_desc(
                index,
                Desc {
                    addr: dma_pa + BUF_OFF as u64 + index as u64 * EVENT_BYTES as u64,
                    len: EVENT_BYTES as u32,
                    flags: DescFlags::WRITE,
                    next: 0,
                },
            ) {
                return Err("arm");
            }
            queue.push_avail(index);
        }
    }
    handoff_and_notify();
    PUBLISHED.store(EVENT_BUFS as u64, Ordering::SeqCst);
    Ok(EVENT_BUFS as u16)
}

/// Nhận **một** sự kiện nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có sự kiện.
///
/// `used->len` khác 8 hoặc `used->id` ngoài queue ⇒ đếm `errors`/`id_mismatch`, **không** trả dữ
/// liệu rác; descriptor vẫn được arm lại để lần sau nhận được.
pub fn poll_event() -> Option<InputEvent> {
    if !READY.load(Ordering::Relaxed) {
        return None;
    }
    let size = QUEUE_SIZE_NOW.load(Ordering::Relaxed) as u16;
    if size < MIN_QUEUE_SIZE {
        return None;
    }
    let dma_pa = DMA_PA.load(Ordering::Relaxed);
    let layout = VqLayout::new(size)?;
    let used_idx_addr = dma_pa + RING_OFF as u64 + layout.used_idx_off as u64;
    // **KT-277**: `dc ivac` dòng chứa `used->idx` **trước** mỗi lần đọc. Thiết bị ghi qua DMA
    // (không đi qua cache của CPU), nên đọc mà không bỏ cache có thể thấy mãi giá trị cũ và poll
    // treo tới hết trần. An toàn: `HANDOFF_SEQUENCE` vừa `dc civac` toàn trang nên mọi dòng đều
    // **sạch** — `dc ivac` chỉ bỏ bản sao đã sạch, không mất dữ liệu chưa ghi.
    cache::invalidate_line(used_idx_addr);
    let idx = asm::mmio_read_u16(used_idx_addr);
    let last = LAST_USED.load(Ordering::Relaxed) as u16;
    if idx == last {
        return None;
    }
    // Thiết bị đã ghi xong — bỏ cache **trước** khi đọc `used->elem` và đệm sự kiện.
    let mut inval_lines = 0u64;
    for step in cache::COMPLETE_SEQUENCE {
        inval_lines += cache::apply_complete(step, dma_pa, DMA_PAGE_BYTES as u64);
    }
    INVAL_LINES.fetch_add(inval_lines, Ordering::SeqCst);
    INVAL_OPS.fetch_add(1, Ordering::SeqCst);
    let (id, len) = {
        let queue = queue_with(size).ok()?;
        queue.used_elem(last & (size - 1))
    };
    LAST_USED.store(last.wrapping_add(1) as u64, Ordering::Relaxed);
    if id as u16 >= size {
        ID_MISMATCH.fetch_add(1, Ordering::SeqCst);
        ERRORS.fetch_add(1, Ordering::SeqCst);
        return None;
    }
    let index = id as u16;
    let event = match event_len(len) {
        Ok(count) => {
            let start = BUF_OFF + index as usize * EVENT_BYTES;
            InputEvent::parse(&page()[start..start + count])
        }
        Err(_) => None,
    };
    // Arm lại **trước** khi trả: cửa sổ không có đệm là nhỏ nhất (QEMU cần đệm cho **mọi** sự
    // kiện của lô kế tiếp).
    if !arm_one(index) {
        ERRORS.fetch_add(1, Ordering::SeqCst);
    }
    let event = match event {
        Some(event) => event,
        None => {
            ERRORS.fetch_add(1, Ordering::SeqCst);
            return None;
        }
    };
    EVENTS.fetch_add(1, Ordering::SeqCst);
    if event.is_syn() {
        SYNCS.fetch_add(1, Ordering::SeqCst);
    } else if event.is_key_down() {
        KEY_DOWNS.fetch_add(1, Ordering::SeqCst);
    } else if event.is_key_up() {
        KEY_UPS.fetch_add(1, Ordering::SeqCst);
    }
    Some(event)
}

/// Nhận **một** sự kiện, chờ tối đa `timeout_ticks` tick `CNTPCT_EL0`. `None` = hết hạn/chưa có.
///
/// `timeout_ticks` là tick của **bộ đếm vật lý** (xem [`ticks_for_secs`]); `0` ⇒ dùng trần mặc
/// định [`POLL_TIMEOUT_SECS`]. `CNTFRQ_EL0 = 0` (firmware không khai tần số) ⇒ lùi về trần **số
/// vòng** [`POLL_SPIN_LIMIT`] — ghi rõ, không im lặng chờ vô hạn.
pub fn poll_event_wait(timeout_ticks: u64) -> Option<InputEvent> {
    let deadline = if timeout_ticks == 0 {
        poll_deadline()
    } else {
        Some(now_ticks().wrapping_add(timeout_ticks))
    };
    let mut spins = 0u64;
    loop {
        if let Some(event) = poll_event() {
            return Some(event);
        }
        let expired = match deadline {
            Some(limit) => now_ticks() > limit,
            None => spins >= POLL_SPIN_LIMIT,
        };
        if expired {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            return None;
        }
        spins = spins.wrapping_add(1);
        core::hint::spin_loop();
    }
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

/// Địa chỉ ISR (chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    ISR.load(Ordering::Relaxed)
}

/// PA trang DMA (bằng chứng `identity` của vùng DMA).
pub fn dma_pa() -> u64 {
    DMA_PA.load(Ordering::Relaxed)
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều sự kiện).
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

/// Số descriptor đã publish (một chiều) — bằng chứng ring quay vòng (`published / queue_size`).
pub fn published() -> u16 {
    PUBLISHED.load(Ordering::Relaxed) as u16
}

/// Cỡ queue `event` driver đã đặt (0 nếu chưa init).
pub fn queue_size() -> u16 {
    QUEUE_SIZE_NOW.load(Ordering::Relaxed) as u16
}

/// Cỡ queue `status` driver đã đặt (0 nếu chưa init).
pub fn status_queue_size() -> u16 {
    STATUS_QUEUE_SIZE.load(Ordering::Relaxed) as u16
}

/// `queue_notify_off` của queue `status` (bằng chứng driver đọc **cả hai** queue).
pub fn status_notify_off() -> u16 {
    STATUS_NOTIFY_OFF.load(Ordering::Relaxed) as u16
}

/// `true` ⇔ ring `status` đã bật và **không** có descriptor nào được publish (không arm đệm).
///
/// Đây là bất biến của thiết kế: driver chỉ **đọc** phím, nên queue `status` (LED) được bật cho
/// đúng topology nhưng không bao giờ có đệm — thiết bị không thể ghi vào ring ấy.
pub fn status_queue_idle() -> bool {
    let size = STATUS_QUEUE_SIZE.load(Ordering::Relaxed) as u16;
    if size < MIN_QUEUE_SIZE {
        return false;
    }
    match status_queue_with(size) {
        Ok(queue) => queue.avail_idx() == 0,
        Err(_) => false,
    }
}
