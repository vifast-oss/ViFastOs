// SPDX-License-Identifier: MIT OR Apache-2.0
//! `virtio_rng` — **logic thuần** của `virtio-rng`: device id **4**, feature, chỉ số hàng đợi và
//! **cổng chất lượng entropy** (KT-418b, §5.4 Virtio 1.2).
//!
//! # Phạm vi
//!
//! Chỉ phần **quyết định trên byte**: định danh thiết bị, hằng số hàng đợi, mã lỗi có tên
//! (D2-102) và phép kiểm chất lượng mẫu entropy. Không MMIO, không `unsafe`, không cấp phát ⇒ chạy
//! host test (D2-05: đỏ trước rồi mới xanh). Driver thật ở [`crate::rngdev`] (dùng chung 3 arch);
//! nền tảng chỉ còn adapter MMIO/DMA/đồng hồ.
//!
//! # Vì sao có **cổng chất lượng** (không chỉ "đọc được byte")
//!
//! `virtio-rng` là nguồn entropy **duy nhất** lấy từ phần cứng. Một thiết bị hỏng (hoặc bị cắt
//! nguồn ngẫu nhiên) vẫn "hoàn tất" request và trả **toàn số 0** — driver chỉ kiểm "có byte" sẽ
//! đầu độc mọi khoá sinh sau đó mà **không** có triệu chứng nào ở tầng trên. Vì vậy mẫu phải qua
//! [`entropy_ok`] **trước khi** được tính là entropy: đủ dài, đủ **đa dạng** giá trị byte, và
//! **không** có run dài (nguồn "gần như hằng"). Đây là phép kiểm **thô** có chủ đích — nó không
//! thay thế kiểm thống kê, nó chỉ chặn đúng lớp hỏng mà một driver có thể gặp.
//!
//! # Vì sao **không** có `device config`
//!
//! `virtio-rng` không có trường cấu hình nào (§5.4.4 để trống) ⇒ driver **không** đọc
//! `device_cfg`, không có `config_generation`. Đó là lý do `rngdev::init` ngắn hơn `netdev::init`:
//! không có bước đọc config, không có nguy cơ dữ liệu bị **xé**.

/// `virtio-rng` bản transitional — PCI device id **`0x1005`**, **không** phải `0x1000 + 4`.
///
/// # Vì sao `0x1005` (đo được, không suy từ số hiệu thiết bị)
///
/// Bảng id PCI **legacy** của virtio không cộng đều `0x1000 + device_id`: `0x1004` là
/// `virtio-scsi`, còn **rng** là `0x1005` (balloon `0x1002`, console `0x1003`, 9p `0x1009`).
/// Đo bằng QEMU 11.1.3 (`info pci`): `-device virtio-rng-pci` ⇒ `1af4:1005`;
/// `-device virtio-rng-pci,disable-legacy=on` ⇒ `1af4:1044`. Bản đầu của KT-418b dùng `0x1004`
/// (suy từ "device id 4") và boot thật in `# rng: no device` — xem journal `kt418b-rng-smap.md`
/// §4.1. Đây là lý do hằng số này ghi kèm **bằng chứng đo**, không phải suy luận.
pub const VIRTIO_RNG_MODERN: u16 = 0x1005;
/// `virtio-rng` bản modern-only (`disable-legacy=on`) — PCI device id **`0x1044`** (đo được).
pub const VIRTIO_RNG_MODERN_ONLY: u16 = 0x1044;
/// Device id mà driver tìm trên bus PCI (bản transitional `0x1005`).
pub const DEVICE_ID_RNG: u16 = VIRTIO_RNG_MODERN;

/// `VIRTIO_F_VERSION_1` (bit 32) — bắt buộc, xem [`crate::features::F_VERSION_1`].
pub const F_VERSION_1: u32 = 32;

/// Hàng đợi `requestq` — §5.4.2: **một** hàng đợi duy nhất, chỉ số 0.
pub const Q_REQUEST: u16 = 0;
/// Số hàng đợi driver **cần**: đúng một.
pub const QUEUES_REQUIRED: u16 = 1;
/// Cỡ hàng đợi driver đặt (luỹ thừa 2). 8 đủ cho một request đang bay + dư, rẻ về `.bss`.
pub const QUEUE_SIZE: u16 = 8;
/// Cỡ hàng đợi tối thiểu dùng được: một descriptor.
pub const MIN_QUEUE_SIZE: u16 = 2;

/// Số byte **tối thiểu** của một request: dưới ngưỡng này không đủ mẫu để kết luận chất lượng.
pub const MIN_REQUEST_BYTES: usize = 16;
/// Số byte **tối đa** của một request — trần của đệm DMA một trang.
pub const MAX_REQUEST_BYTES: usize = 256;
/// Số giá trị byte **khác nhau** tối thiểu trong một mẫu hợp lệ.
pub const MIN_DISTINCT_BYTES: u16 = 8;
/// Run (dãy byte giống nhau liên tiếp) **dài nhất** còn chấp nhận được.
pub const MAX_RUN_BYTES: usize = 8;

/// `(vendor, device)` là virtio-rng mà KT-418b chấp nhận (`0x1004`/`0x1044`).
pub const fn is_virtio_rng(vendor: u16, device: u16) -> bool {
    vendor == crate::pci::VENDOR_VIRTIO
        && (device == VIRTIO_RNG_MODERN || device == VIRTIO_RNG_MODERN_ONLY)
}

/// Mã lỗi driver — **tên** để in evidence, không in số trần (D2-102).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RngError {
    /// Gọi `fill` trước `init`.
    NotReady,
    /// Không tìm thấy thiết bị trên bus.
    NoDevice,
    /// Thiết bị không báo `VIRTIO_F_VERSION_1`.
    Version1,
    /// Đàm phán feature đọc lại **không** khớp điều driver ghi.
    Features,
    /// Thiết bị **từ chối** `FEATURES_OK` (§2.4.2 bước 5).
    FeaturesOk,
    /// Thiết bị báo ít hơn một hàng đợi.
    Queues,
    /// `queue_size` thiết bị báo không dùng được (không luỹ thừa 2 / quá nhỏ).
    QueueSize,
    /// Ghi `queue_enable` xong đọc lại **không** thấy bật.
    QueueEnable,
    /// `device_status` sau `DRIVER_OK` không đúng chuỗi bit.
    DriverOk,
    /// Độ dài yêu cầu ngoài `[MIN_REQUEST_BYTES, MAX_REQUEST_BYTES]`.
    BadLen,
    /// Thiết bị **không** hoàn tất trong hạn.
    Timeout,
    /// `used->id`/`used->len` vô lý (thiết bị trả lời sai giao thức).
    Io,
    /// Mẫu trả về **không** qua [`entropy_ok`] — fail-closed, không nhận entropy rác.
    LowEntropy,
}

impl RngError {
    /// Tên để in evidence (D2-102).
    pub const fn name(self) -> &'static str {
        match self {
            RngError::NotReady => "not_ready",
            RngError::NoDevice => "no_device",
            RngError::Version1 => "version1",
            RngError::Features => "features",
            RngError::FeaturesOk => "features_ok",
            RngError::Queues => "queues",
            RngError::QueueSize => "queue_size",
            RngError::QueueEnable => "queue_enable",
            RngError::DriverOk => "driver_ok",
            RngError::BadLen => "bad_len",
            RngError::Timeout => "timeout",
            RngError::Io => "io",
            RngError::LowEntropy => "low_entropy",
        }
    }
}

/// Số giá trị byte **khác nhau** trong `bytes` (0..=256) — không cấp phát: bitmap 256 bit.
pub fn distinct_bytes(bytes: &[u8]) -> u16 {
    let mut seen = [0u64; 4];
    for byte in bytes {
        let index = usize::from(*byte);
        seen[index / 64] |= 1u64 << (index % 64);
    }
    seen.iter().map(|word| word.count_ones() as u16).sum()
}

/// Độ dài **run dài nhất** (dãy byte giống nhau liên tiếp) trong `bytes`; 0 khi rỗng.
pub fn longest_run(bytes: &[u8]) -> usize {
    let mut best = 0usize;
    let mut run = 0usize;
    let mut previous: Option<u8> = None;
    for byte in bytes {
        if previous == Some(*byte) {
            run += 1;
        } else {
            run = 1;
            previous = Some(*byte);
        }
        if run > best {
            best = run;
        }
    }
    best
}

/// `true` ⇔ mẫu `bytes` **đủ** để coi là entropy: đủ dài, đủ đa dạng, không run dài.
///
/// Fail-closed: mẫu rỗng/ngắn, toàn một giá trị, hoặc "gần như hằng" đều **không** qua. Đây là
/// cổng **thô** chặn lớp hỏng "thiết bị trả toàn 0" — không phải kiểm thống kê đầy đủ.
pub fn entropy_ok(bytes: &[u8]) -> bool {
    bytes.len() >= MIN_REQUEST_BYTES
        && distinct_bytes(bytes) >= MIN_DISTINCT_BYTES
        && longest_run(bytes) <= MAX_RUN_BYTES
}

/// Ảnh chụp bộ đếm driver — đọc **một lần**, in từ dữ liệu (D2-102).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RngReport {
    /// Device id đọc từ PCI config.
    pub device_id: u16,
    /// `VIRTIO_F_VERSION_1` đã đàm phán.
    pub version1: bool,
    /// Cỡ hàng đợi driver đặt (thiết bị đã nhận).
    pub queue_size: u16,
    /// Số request **thành công**.
    pub requests: u64,
    /// Số byte entropy **đã nhận** (chỉ request thành công).
    pub bytes: u64,
    /// Số giá trị byte khác nhau của mẫu **gần nhất**.
    pub distinct: u16,
    /// Run dài nhất của mẫu **gần nhất**.
    pub longest_run: usize,
    /// Mẫu gần nhất có qua [`entropy_ok`] không.
    pub entropy_ok: bool,
    /// Số lần ghi cửa thông báo (doorbell).
    pub notifies: u64,
    /// Số lần poll quá hạn (phải 0 ở đường thành công).
    pub timeouts: u64,
    /// Số lỗi driver (mẫu rác, `used` vô lý, thiết bị trả lời sai).
    pub errors: u64,
}
