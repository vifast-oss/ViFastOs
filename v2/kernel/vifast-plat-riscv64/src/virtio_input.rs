// SPDX-License-Identifier: MIT OR Apache-2.0
//! Driver **virtio-input** (bàn phím, virtio 1.0, virtio-pci modern) trên QEMU `virt` — KT-255 (D1).
//!
//! # Đường đi
//!
//! `pci::init_for(VirtioKind::Input)` (ECAM của KT-72, **không** nhân đôi code PCI) ⇒ BAR modern +
//! bốn capability ⇒ module này lập trình common config, dựng **một** split virtqueue (`event` =
//! queue 0 — §5.8.2), đọc `virtio_input_config` (tên/danh tính/bitmap `EV_BITS`), rồi
//! `poll_event`/`poll_event_wait` và **poll `used`** (KT-255 chưa dùng ngắt).
//!
//! # Bất biến
//!
//! - Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1`; **không** `VIRTIO_INPUT_F_REP` — quyết
//!   định nằm ở `vifast_virtio::input::negotiated_features` (test host), không viết tay hai chỗ
//!   (D2-110B).
//! - Thứ tự spec §2.7: ghi descriptor → ghi `avail->ring` → **fence** → ghi `avail->idx` →
//!   **fence** → notify (MMIO) → poll `used->idx` → **fence** → đọc `used->elem`.
//! - `used->id` **phải** là descriptor đã arm (< `queue_size`) — lệch ⇒ đếm `id_mismatch` + lỗi,
//!   không im lặng. `used->len` **phải** đúng 8 (`event_len`) — thiết bị ghi một
//!   `virtio_input_event` vào mỗi đệm 8 byte.
//! - **Arm nhiều đệm** ([`EVENT_BUFS`]): QEMU gom sự kiện tới `EV_SYN`/`SYN_REPORT` rồi mới lấy
//!   **một đệm cho mỗi sự kiện**; thiếu đệm thì **cả lô** bị huỷ ⇒ mất phím. Một lô
//!   `sendkey shift-a` là 5 sự kiện.
//! - **Fail-closed**: thiếu thiết bị ⇒ `Err` (không PANIC, không treo); config thiếu/không khớp
//!   `select`/`subsel` ⇒ `Err`; `used->len` vô lý ⇒ `Io` + đếm `errors`.
//!
//! `unsafe` (MMIO + `UnsafeCell` vùng DMA) nằm trong `kernel/vifast-plat-*` (D2-17); không lệnh
//! đặc quyền nào ở U-mode.

use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_common::Platform;
use vifast_virtio::features::{
    features_ok, status_sequence_ok, ACK, DRIVER, DRIVER_OK, FEATURES_OK, F_VERSION_1,
};
use vifast_virtio::input::{
    decode_devids, decode_name, event_len, key_bit, negotiated_features, InputConfigHeader,
    InputDevIds, InputError, InputEvent, CFG_EV_BITS, CFG_HEADER_BYTES, CFG_ID_DEVIDS, CFG_ID_NAME,
    CFG_SELECT_OFF, CFG_SUBSEL_OFF, CFG_U_BYTES, CFG_U_OFF, EVENT_BUFS, EVENT_BYTES, EV_KEY, KEY_A,
    MIN_QUEUE_SIZE, QUEUES_REQUIRED, QUEUE_SIZE, Q_EVENT,
};
use vifast_virtio::queue::{Desc, DescFlags, SplitQueue, VqLayout};

use crate::pci::PciReport;
use crate::Riscv64Virt;

/// Cỡ trang DMA (căn lề cứng của virtqueue).
pub const DMA_PAGE_BYTES: usize = 4096;
/// Offset ring `event` trong trang DMA.
const RING_OFF: usize = 0x000;
/// Offset vùng đệm sự kiện (mỗi đệm [`EVENT_BYTES`] byte, device-writable).
const BUF_OFF: usize = 0x400;
/// Cỡ tối đa của một `virtio_input_config` đọc được (union 128 byte).
pub const NAME_MAX_BYTES: usize = CFG_U_BYTES;
/// Cỡ bitmap `EV_BITS` tối đa (union 128 byte).
pub const EV_BITS_BYTES: usize = CFG_U_BYTES;

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
static EVENTS: AtomicU64 = AtomicU64::new(0);
static SYNCS: AtomicU64 = AtomicU64::new(0);
static KEY_DOWNS: AtomicU64 = AtomicU64::new(0);
static KEY_UPS: AtomicU64 = AtomicU64::new(0);
static NOTIFIES: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static ID_MISMATCH: AtomicU64 = AtomicU64::new(0);

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
    }
}

/// Trạng thái driver — gói trong `UnsafeCell` + `Sync` thủ công: ảnh test chạy **một hart**,
/// driver tự tuần tự hoá (một sự kiện một lúc), không có truy cập song song.
struct State {
    ready: bool,
    common: u64,
    notify: u64,
    isr: u64,
    device_cfg: u64,
    notify_mul: u32,
    /// `queue_notify_off` của queue `event`.
    notify_off: u16,
    /// Cỡ queue thật của queue `event` (đọc lại từ thiết bị).
    queue_size: u16,
    /// `used->idx` đã xử lý.
    last_used: u16,
    /// Số descriptor đã publish (một chiều) — bằng chứng ring quay vòng.
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
pub struct InputReport {
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
    /// `queue_notify_off` của queue `event`.
    pub notify_off: u16,
    /// Địa chỉ cửa thông báo `event` = notify + off * multiplier.
    pub notify_addr: u64,
    /// `num_queues` thiết bị báo (QEMU: 2 — `event` + `status`).
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
    /// Địa chỉ (PA) bảng descriptor.
    pub desc: u64,
    /// PA khối `avail`.
    pub avail: u64,
    /// PA khối `used`.
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
pub fn init(pci: &PciReport) -> Result<InputReport, &'static str> {
    let caps = pci.caps;
    let bar = pci.bar_assigned;
    let common = bar + caps.common.offset as u64;
    let notify = bar + caps.notify.offset as u64;
    let device_cfg = bar + caps.device.offset as u64;
    let isr = bar + caps.isr.offset as u64;
    let notify_mul = caps.notify_off_multiplier;
    let layout = VqLayout::new(QUEUE_SIZE).ok_or("layout")?;
    if RING_OFF + layout.total_bytes() > BUF_OFF
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
    // ---- 6) queue `event` = 0 (queue `status` = 1 **không** dùng — KT-255 chỉ đọc phím)
    let num_queues = read16(common + C_NUM_QUEUES);
    if num_queues < QUEUES_REQUIRED {
        return Err("queues");
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
    // ---- 8) device config: tên + danh tính + bitmap EV_BITS (đọc **từ thiết bị**)
    let name_header = select_config(device_cfg, CFG_ID_NAME, 0)?;
    let mut name = [0u8; NAME_MAX_BYTES];
    let payload_len = read_config_payload(device_cfg, &name_header, &mut name);
    // `size` của QEMU **gồm** NUL cuối (`sizeof("QEMU Virtio Keyboard")` = 21) ⇒ `name_len` phải là
    // số byte **có nghĩa** sau khi bỏ NUL, không phải `size` — nếu không, `name()` trả chuỗi có NUL
    // và dòng evidence `# input name:` bị cắt ở NUL (đo được ở lần chạy đầu: `od -c` thấy `\0`).
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
    // ---- 9) lưu trạng thái (khối riêng: không giữ `&mut State` qua lời gọi `arm_all`)
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
        desc: base + layout.desc_off as u64,
        avail: base + layout.avail_off as u64,
        used: base + layout.used_off as u64,
        buf_pa: dma + BUF_OFF as u64,
        armed,
    })
}

/// Bọc ring của queue `event` thành split queue với cỡ **thật** của nó.
///
/// Nhận `size` từ người gọi (không tự đọc trạng thái) để không có hai `&mut State` sống cùng lúc.
fn queue_with(size: u16) -> Result<SplitQueue<'static>, InputError> {
    if size < MIN_QUEUE_SIZE {
        return Err(InputError::NotReady);
    }
    let layout = VqLayout::new(size).ok_or(InputError::NotReady)?;
    SplitQueue::new(&mut page()[RING_OFF..RING_OFF + layout.total_bytes()], size)
        .ok_or(InputError::NotReady)
}

/// Đăng descriptor `index` (đệm 8 byte device-writable) và publish — gọi lúc init và sau mỗi sự
/// kiện nhận.
fn arm_one(index: u16) -> bool {
    let size = state().queue_size;
    if size < MIN_QUEUE_SIZE || index >= size {
        return false;
    }
    let dma = dma_page_addr();
    {
        let Ok(mut queue) = queue_with(size) else {
            return false;
        };
        if !queue.set_desc(
            index,
            Desc {
                addr: dma + BUF_OFF as u64 + index as u64 * EVENT_BYTES as u64,
                len: EVENT_BYTES as u32,
                flags: DescFlags::WRITE,
                next: 0,
            },
        ) {
            return false;
        }
        queue.push_avail(index);
    }
    fence();
    let notify = state().notify;
    let notify_off = state().notify_off;
    let notify_mul = state().notify_mul;
    write16(notify + notify_off as u64 * notify_mul as u64, Q_EVENT);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
    // Một `&mut State` duy nhất: `state().published = state().published + 1` tạo **hai** borrow
    // mutable của cùng một `static` trong một biểu thức (aliasing — UB dù compiler không bắt).
    let s = state();
    s.published = s.published.wrapping_add(1);
    true
}

/// Arm [`EVENT_BUFS`] đệm đầu tiên rồi notify **một** lần (init).
fn arm_all() -> Result<u16, &'static str> {
    let size = state().queue_size;
    if size < EVENT_BUFS as u16 {
        return Err("arm");
    }
    let dma = dma_page_addr();
    {
        let mut queue = queue_with(size).map_err(|_| "arm")?;
        for index in 0..EVENT_BUFS as u16 {
            if !queue.set_desc(
                index,
                Desc {
                    addr: dma + BUF_OFF as u64 + index as u64 * EVENT_BYTES as u64,
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
    fence();
    let notify = state().notify;
    let notify_off = state().notify_off;
    let notify_mul = state().notify_mul;
    write16(notify + notify_off as u64 * notify_mul as u64, Q_EVENT);
    NOTIFIES.fetch_add(1, Ordering::SeqCst);
    state().published = EVENT_BUFS as u16;
    Ok(EVENT_BUFS as u16)
}

/// Nhận **một** sự kiện nếu thiết bị đã hoàn tất (không chờ). `None` = chưa có sự kiện.
///
/// `used->len` khác 8 hoặc `used->id` ngoài queue ⇒ đếm `errors`/`id_mismatch`, **không** trả dữ
/// liệu rác; descriptor vẫn được arm lại để lần sau nhận được.
pub fn poll_event() -> Option<InputEvent> {
    if !state().ready {
        return None;
    }
    let size = state().queue_size;
    if size < MIN_QUEUE_SIZE {
        return None;
    }
    let dma = dma_page_addr();
    let used_idx_addr = dma + RING_OFF as u64 + VqLayout::new(size)?.used_idx_off as u64;
    // SAFETY: `used_idx_addr` nằm trong trang DMA đã bàn giao; đọc volatile để compiler **không**
    // hoist khỏi vòng lặp (bẫy kinh điển của poll DMA).
    let idx = unsafe { read_volatile(used_idx_addr as *const u16) };
    let last = state().last_used;
    if idx == last {
        return None;
    }
    fence();
    let (id, len) = {
        let queue = queue_with(size).ok()?;
        queue.used_elem(last & (size - 1))
    };
    state().last_used = last.wrapping_add(1);
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

/// Nhận **một** sự kiện, chờ tối đa `timeout_ticks` tick timebase. `None` = hết hạn / chưa có.
pub fn poll_event_wait(timeout_ticks: u64) -> Option<InputEvent> {
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    loop {
        if let Some(event) = poll_event() {
            return Some(event);
        }
        if Riscv64Virt.now_ticks() > deadline {
            TIMEOUTS.fetch_add(1, Ordering::SeqCst);
            return None;
        }
        core::hint::spin_loop();
    }
}

/// `true` ⇔ driver đã init thành công.
pub fn ready() -> bool {
    state().ready
}

/// Địa chỉ ISR (KT-255 chưa dùng ngắt — chỉ in chẩn đoán).
pub fn isr_addr() -> u64 {
    state().isr
}

/// Đọc thanh ghi `device_status` hiện tại (bằng chứng driver còn DRIVER_OK sau nhiều sự kiện).
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

/// Số descriptor đã publish (một chiều) — bằng chứng ring quay vòng (`published / queue_size`).
pub fn published() -> u16 {
    state().published
}

const _: () = {
    // Ring + vùng đệm phải nằm gọn trong **một** trang DMA 4 KiB.
    let layout = match VqLayout::new(QUEUE_SIZE) {
        Some(l) => l,
        None => panic!("queue size khong hop le"),
    };
    assert!(RING_OFF + layout.end <= BUF_OFF);
    assert!(BUF_OFF + EVENT_BUFS * EVENT_BYTES <= DMA_PAGE_BYTES);
    assert!(EVENT_BUFS as u16 <= QUEUE_SIZE);
};
