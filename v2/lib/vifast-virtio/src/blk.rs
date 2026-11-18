// SPDX-License-Identifier: MIT OR Apache-2.0
//! `virtio_blk` — request header, `virtio_blk_config`, mã status (§5.2 Virtio 1.0), phần **thuần**.
//!
//! Đơn vị của thiết bị là **sector 512 B** (giống `vifast_fs::BLOCK_SIZE`): `read` = `IN`,
//! `write` = `OUT`, `flush` = `FLUSH` (không có đệm dữ liệu, chỉ header + status).

/// Loại request `IN` (đọc thiết bị → bộ nhớ).
pub const T_IN: u32 = 0;
/// Loại request `OUT` (ghi bộ nhớ → thiết bị).
pub const T_OUT: u32 = 1;
/// Loại request `FLUSH` — đẩy cache ghi của thiết bị xuống phương tiện (§5.2.6).
pub const T_FLUSH: u32 = 4;

/// Status `OK` — request hoàn tất.
pub const S_OK: u8 = 0;
/// Status `IOERR` — lỗi I/O (gồm sector ngoài dung lượng).
pub const S_IOERR: u8 = 1;
/// Status `UNSUPP` — thiết bị không hỗ trợ loại request.
pub const S_UNSUPP: u8 = 2;

/// Cỡ header request (§5.2.6): `type` 4 + `reserved` 4 + `sector` 8.
pub const REQ_HEADER_BYTES: usize = 16;
/// Cỡ `virtio_blk_config` tối thiểu mà driver đọc: capacity 8 + size_max 4 + seg_max 4 +
/// geometry 4 + blk_size 4 = 24.
pub const BLK_CONFIG_BYTES: usize = 24;

/// Cỡ sector mặc định khi `blk_size` = 0 (thiết bị cũ).
pub const DEFAULT_SECTOR_BYTES: u32 = 512;

/// `VIRTIO_BLK_F_FLUSH` = **feature bit 9** (§5.2.3): thiết bị có cache ghi và hỗ trợ request
/// `FLUSH`. Driver **phải** đàm phán bit này trước khi gửi `T_FLUSH`; thiếu đàm phán mà vẫn gửi
/// là vi phạm spec (thiết bị thật có thể trả lỗi/undefined).
pub const F_FLUSH: u32 = 9;
/// Mask của [`F_FLUSH`] trong 32 bit feature thấp (`0x200`).
pub const FLUSH_MASK: u32 = 1 << F_FLUSH;

/// Bộ feature driver **chọn** từ `device_features`: luôn `VIRTIO_F_VERSION_1` (bit 32; init đã
/// kiểm thiết bị có) + `VIRTIO_BLK_F_FLUSH` **nếu** thiết bị có.
///
/// Vì sao hàm thuần ở đây: đàm phán feature là quyết định (không phải MMIO) ⇒ test host được, và
/// nó là **một** chỗ duy nhất để "driver gửi FLUSH" và "driver đã đàm phán FLUSH" không lệch nhau.
pub const fn negotiated_features(device_features: u64) -> u64 {
    let mut features = 1u64 << 32;
    if device_features & FLUSH_MASK as u64 != 0 {
        features |= FLUSH_MASK as u64;
    }
    features
}

/// `true` ⇔ `driver_features` **đã** có `VIRTIO_BLK_F_FLUSH` (được phép gửi `T_FLUSH`).
pub const fn flush_negotiated(driver_features: u64) -> bool {
    driver_features & FLUSH_MASK as u64 != 0
}

/// Header của một request virtio-blk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReqHeader {
    /// `T_IN`/`T_OUT`/`T_FLUSH`.
    pub kind: u32,
    /// Sector bắt đầu (tính theo 512 B — đơn vị của spec, không phải `blk_size`).
    pub sector: u64,
}

impl ReqHeader {
    /// Header cho một request.
    pub const fn new(kind: u32, sector: u64) -> Self {
        Self { kind, sector }
    }

    /// `true` ⇔ đây là `FLUSH` (không có đệm dữ liệu).
    pub const fn is_flush(&self) -> bool {
        self.kind == T_FLUSH
    }

    /// Mã hoá 16 byte little-endian.
    pub fn encode(&self) -> [u8; REQ_HEADER_BYTES] {
        let mut out = [0u8; REQ_HEADER_BYTES];
        out[0..4].copy_from_slice(&self.kind.to_le_bytes());
        // `reserved` (byte 4..8) phải 0 — để nguyên 0.
        out[8..16].copy_from_slice(&self.sector.to_le_bytes());
        out
    }

    /// Giải mã lại 16 byte (kiểm bằng chứng: request trên ring = request mong đợi).
    pub fn decode(bytes: &[u8]) -> Self {
        let mut raw = [0u8; REQ_HEADER_BYTES];
        let n = bytes.len().min(REQ_HEADER_BYTES);
        raw[..n].copy_from_slice(&bytes[..n]);
        Self {
            kind: u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]),
            sector: u64::from_le_bytes(raw[8..16].try_into().unwrap_or([0; 8])),
        }
    }
}

/// Một request virtio-blk **đã dựng xong** — kết quả của [`build_request`].
///
/// # Vì sao trả **cấu trúc** chứ không chỉ 16 byte header
///
/// Một request không chỉ là header. Thiết bị thật kiểm **cả** độ dài đệm và dải sector; driver
/// dựng header đúng mà khai độ dài sai thì vẫn hỏng. Gom cả ba thứ vào **một** giá trị để "cái
/// driver gửi" và "cái test kiểm" là **một** — không có đường thứ hai để lệch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltRequest {
    /// 16 byte header little-endian, đã mã hoá, sẵn sàng chép vào ring.
    pub header: [u8; REQ_HEADER_BYTES],
    /// Loại request (`T_IN`/`T_OUT`/`T_FLUSH`).
    pub kind: u32,
    /// Sector bắt đầu (đơn vị 512 B của spec).
    pub sector: u64,
    /// Số sector của request (`0` cho `FLUSH`).
    pub sectors: u32,
    /// Độ dài đệm dữ liệu mà request **đòi** (`0` cho `FLUSH`).
    pub data_len: u32,
    /// Chỉ số descriptor của status trong chuỗi (`1` cho `FLUSH`, `2` cho `IN`/`OUT`).
    pub status_index: u16,
}

/// **Dựng một request virtio-blk** từ `(loại, sector, số sector)`.
///
/// # Đây là mã của driver **thật**
///
/// `v2/kernel/vifast-plat-riscv64/src/virtio_blk.rs` (`submit`) gọi **đúng hàm này** — không chép
/// lại công thức ở đâu khác. Nhờ vậy test host (`tests/driver_request.rs`) chạy **cùng** mã được
/// biên dịch vào ảnh, và `StrictBlk` kiểm đúng cái driver thật sự gửi.
///
/// `data_len` tính từ **số sector**, không nhận từ người gọi: nếu người gọi tự khai độ dài thì
/// chính chỗ khai ấy là nơi lỗi v1 #2 sinh ra.
pub fn build_request(kind: u32, sector: u64, sectors: u32) -> BuiltRequest {
    let data_len = if kind == T_FLUSH {
        0
    } else {
        sectors.saturating_mul(DEFAULT_SECTOR_BYTES)
    };
    BuiltRequest {
        header: ReqHeader::new(kind, sector).encode(),
        kind,
        sector,
        sectors,
        data_len,
        status_index: if kind == T_FLUSH { 1 } else { 2 },
    }
}

/// `virtio_blk_config` mà driver đọc (chỉ các trường nó dùng — không đoán phần còn lại).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlkConfig {
    /// Dung lượng theo sector 512 B — đọc **từ thiết bị**, không hardcode.
    pub capacity_sectors: u64,
    /// Cỡ request tối đa thiết bị nhận.
    pub size_max: u32,
    /// Số segment tối đa của một request.
    pub seg_max: u32,
    /// `blk_size` thiết bị báo (0 ⇒ 512).
    pub blk_size: u32,
}

impl BlkConfig {
    /// Dung lượng theo byte = `capacity_sectors * 512` (công thức, D2-106).
    pub const fn capacity_bytes(&self) -> u64 {
        self.capacity_sectors * DEFAULT_SECTOR_BYTES as u64
    }

    /// `true` ⇔ thiết bị dùng sector 512 B (driver K6 chỉ hỗ trợ cỡ này).
    pub const fn sector_512(&self) -> bool {
        self.blk_size == DEFAULT_SECTOR_BYTES
    }
}

/// Giải mã `virtio_blk_config`. `None` nếu `blk_size` khác 512 (driver **không** tự đổi cỡ
/// sector: `vifast_fs` cố định 512 B).
pub fn decode_config(raw: &[u8]) -> Option<BlkConfig> {
    if raw.len() < BLK_CONFIG_BYTES {
        return None;
    }
    let blk_size = u32::from_le_bytes(raw[20..24].try_into().ok()?);
    let blk_size = if blk_size == 0 {
        DEFAULT_SECTOR_BYTES
    } else {
        blk_size
    };
    if blk_size != DEFAULT_SECTOR_BYTES {
        return None;
    }
    Some(BlkConfig {
        capacity_sectors: u64::from_le_bytes(raw[0..8].try_into().ok()?),
        size_max: u32::from_le_bytes(raw[8..12].try_into().ok()?),
        seg_max: u32::from_le_bytes(raw[12..16].try_into().ok()?),
        blk_size,
    })
}

/// Mã status thiết bị ghi vào byte cuối của request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlkStatus {
    /// `OK`.
    Ok = S_OK,
    /// `IOERR`.
    IoErr = S_IOERR,
    /// `UNSUPP`.
    Unsupported = S_UNSUPP,
    /// Mã lạ (spec: "khác 0/1/2" ⇒ coi là lỗi I/O).
    Unknown = 3,
}

impl BlkStatus {
    /// Giải mã byte status; mọi giá trị ∉ {0,1,2} ⇒ [`BlkStatus::Unknown`].
    pub const fn from_u8(value: u8) -> Self {
        match value {
            S_OK => BlkStatus::Ok,
            S_IOERR => BlkStatus::IoErr,
            S_UNSUPP => BlkStatus::Unsupported,
            _ => BlkStatus::Unknown,
        }
    }
}

/// Lỗi của driver virtio-blk — có **tên** để in evidence (D2-102), và ánh xạ duy nhất sang
/// `vifast-fs` ở tầng gọi ([`BlkError::to_fs_name`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlkError {
    /// Thiết bị chưa sẵn sàng (status chưa DRIVER_OK / queue chưa bật).
    NotReady,
    /// Thiết bị trả `IOERR` hoặc mã lạ.
    Io,
    /// Thiết bị không hỗ trợ loại request (`UNSUPP`).
    Unsupported,
    /// LBA ngoài dung lượng — **kiểm trước** khi lập request, không để thiết bị treo.
    OutOfRange,
    /// Poll `used` quá hạn (không có ngắt/timeout thật ở K6).
    Timeout,
}

impl BlkError {
    /// Tên để in evidence.
    pub const fn name(self) -> &'static str {
        match self {
            BlkError::NotReady => "NotReady",
            BlkError::Io => "Io",
            BlkError::Unsupported => "Unsupported",
            BlkError::OutOfRange => "OutOfRange",
            BlkError::Timeout => "Timeout",
        }
    }

    /// Tên biến thể `vifast_fs::FsError` tương ứng — **một** chỗ ánh xạ, để log và FS không lệch.
    /// (Không import `vifast-fs` ở đây: crate này không phụ thuộc FS.)
    pub const fn to_fs_name(self) -> &'static str {
        match self {
            BlkError::OutOfRange => "Invalid",
            _ => "Io",
        }
    }
}

/// Ánh xạ byte status của thiết bị thành kết quả.
pub const fn status_to_result(status: u8) -> Result<(), BlkError> {
    match status {
        S_OK => Ok(()),
        S_UNSUPP => Err(BlkError::Unsupported),
        _ => Err(BlkError::Io),
    }
}
