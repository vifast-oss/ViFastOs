// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-81** — thiết bị giả **nghiêm ngặt**: mô hình hoá một `virtio-blk` **thật** từ chối request
//! sai, thay vì im lặng chấp nhận.
//!
//! # Vì sao cần
//!
//! Hai lỗi driver của v1 sống được rất lâu vì **mô hình** (thiết bị giả trong test) chấp nhận mọi
//! thứ: nó không có feature, không có dung lượng, không có cỡ dữ liệu — nên driver gửi gì cũng
//! "chạy". Một thiết bị thật thì không như vậy. Hai lỗi ấy, và cách mô hình dưới đây bắt chúng:
//!
//! | Lỗi v1 | Thiết bị thật làm gì | Mô hình này trả |
//! |---|---|---|
//! | Gửi `T_FLUSH` khi **chưa** đàm phán `VIRTIO_BLK_F_FLUSH` | Hành vi không xác định; spec §5.2.3 nói driver **phải** đàm phán trước | [`StrictError::FlushNotNegotiated`] |
//! | Gửi request mà **độ dài dữ liệu** không khớp độ dài đã khai | Thiết bị đọc/ghi theo `size_max` và số sector ⇒ đọc rác hoặc ghi tràn | [`StrictError::LengthMismatch`] |
//!
//! Cộng thêm hai điều kiện mà một thiết bị thật luôn có và mô hình cũ bỏ qua: sector **ngoài dung
//! lượng** ([`StrictError::OutOfRange`]) và **loại request lạ** ([`StrictError::Unsupported`]).
//!
//! # Nguyên tắc
//!
//! - **Thuần**: không MMIO, không `unsafe`, không cấp phát ⇒ chạy host test.
//! - **Fail-closed**: mọi điều kiện không chắc chắn đều trả lỗi, không "cho qua". Một mô hình dễ
//!   dãi còn tệ hơn không có mô hình, vì nó tạo cảm giác đã kiểm.
//! - **Không thay thế** driver thật: đây là **mô hình để test**, không phải đường chạy sản phẩm.

use crate::blk::{
    flush_negotiated, BlkConfig, ReqHeader, DEFAULT_SECTOR_BYTES, S_IOERR, S_OK, S_UNSUPP, T_FLUSH,
    T_IN, T_OUT,
};

/// Lý do một request bị thiết bị **từ chối**. Mỗi biến thể là một điều kiện mà thiết bị thật kiểm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrictError {
    /// Gửi `T_FLUSH` nhưng `VIRTIO_BLK_F_FLUSH` **chưa** được đàm phán (lỗi driver v1 thứ nhất).
    FlushNotNegotiated,
    /// Độ dài dữ liệu không khớp: `T_IN`/`T_OUT` phải đúng `sectors * 512`, `T_FLUSH` phải **0**
    /// (lỗi driver v1 thứ hai).
    LengthMismatch {
        /// Độ dài mà loại request + số sector **đòi**.
        want: u32,
        /// Độ dài driver thật sự đưa vào descriptor.
        got: u32,
    },
    /// Sector (hoặc dải sector) nằm ngoài dung lượng thiết bị.
    OutOfRange {
        /// Sector bắt đầu.
        sector: u64,
        /// Số sector của request.
        sectors: u64,
        /// Dung lượng thiết bị (sector).
        capacity: u64,
    },
    /// Loại request không phải `T_IN`/`T_OUT`/`T_FLUSH`.
    Unsupported(u32),
    /// Thiết bị chưa `DRIVER_OK` — chưa được nhận request.
    NotReady,
}

impl StrictError {
    /// Tên **chữ** để in vào log (D2-102: không in số trần trong evidence).
    pub const fn name(&self) -> &'static str {
        match self {
            Self::FlushNotNegotiated => "flush-not-negotiated",
            Self::LengthMismatch { .. } => "length-mismatch",
            Self::OutOfRange { .. } => "out-of-range",
            Self::Unsupported(_) => "unsupported-type",
            Self::NotReady => "not-ready",
        }
    }

    /// Mã status mà một thiết bị thật trả cho lỗi này (`S_UNSUPP` cho loại lạ, `S_IOERR` còn lại).
    pub const fn status(&self) -> u8 {
        match self {
            Self::Unsupported(_) => S_UNSUPP,
            _ => S_IOERR,
        }
    }
}

/// Thiết bị `virtio-blk` giả **nghiêm ngặt**.
///
/// Trạng thái đủ để quyết định một request có hợp lệ hay không: dung lượng, `size_max`, feature
/// thiết bị báo, feature driver **đã** đàm phán, và cờ `DRIVER_OK`.
#[derive(Clone, Copy, Debug)]
pub struct StrictBlk {
    config: BlkConfig,
    device_features: u64,
    driver_features: u64,
    ready: bool,
}

impl StrictBlk {
    /// Thiết bị với dung lượng + feature cho trước; **chưa** sẵn sàng (phải `set_driver_ok`).
    pub const fn new(capacity_sectors: u64, device_features: u64) -> Self {
        Self {
            config: BlkConfig {
                capacity_sectors,
                size_max: 1 << 20,
                seg_max: 1,
                blk_size: DEFAULT_SECTOR_BYTES,
            },
            device_features,
            driver_features: 0,
            ready: false,
        }
    }

    /// Ghi bộ feature driver **đã** đàm phán (đúng giá trị đã ghi vào `driver_features`).
    pub fn set_driver_features(&mut self, features: u64) {
        self.driver_features = features;
    }

    /// Đánh dấu thiết bị đã `DRIVER_OK`.
    pub fn set_driver_ok(&mut self) {
        self.ready = true;
    }

    /// `virtio_blk_config` mà driver đọc.
    pub const fn config(&self) -> BlkConfig {
        self.config
    }

    /// `true` ⇔ driver đã đàm phán `VIRTIO_BLK_F_FLUSH` **và** thiết bị có bit ấy.
    ///
    /// Kiểm **cả hai** phía: driver khai có nhưng thiết bị không báo ⇒ cũng là sai (driver không
    /// được tự cho mình quyền gửi `FLUSH` mà thiết bị chưa hứa hỗ trợ).
    pub const fn flush_ok(&self) -> bool {
        flush_negotiated(self.driver_features)
            && self.device_features & crate::blk::FLUSH_MASK as u64 != 0
    }

    /// Độ dài dữ liệu mà `header` **đòi**, tính từ loại request + số sector.
    ///
    /// `T_FLUSH` = 0; `T_IN`/`T_OUT` = `sectors * 512`. Loại lạ ⇒ `None` (không có độ dài hợp lệ).
    pub const fn expected_len(header: ReqHeader, sectors: u64) -> Option<u32> {
        match header.kind {
            T_FLUSH => Some(0),
            T_IN | T_OUT => {
                let bytes = sectors * DEFAULT_SECTOR_BYTES as u64;
                if bytes > u32::MAX as u64 {
                    None
                } else {
                    Some(bytes as u32)
                }
            }
            _ => None,
        }
    }

    /// **Kiểm một request** như thiết bị thật kiểm. `Ok(status)` = chấp nhận; `Err` = từ chối.
    ///
    /// Thứ tự kiểm **cố định** (và được test): sẵn sàng → loại → đàm phán FLUSH → độ dài → dải
    /// sector. Thứ tự cố định để hai lần chạy cùng một request sai luôn cho **cùng** lý do — nếu
    /// không, log đỏ không tái lập được và người đọc phải đoán.
    pub fn submit(
        &self,
        header: ReqHeader,
        sectors: u64,
        data_len: u32,
    ) -> Result<u8, StrictError> {
        if !self.ready {
            return Err(StrictError::NotReady);
        }
        let want = match Self::expected_len(header, sectors) {
            Some(n) => n,
            None => return Err(StrictError::Unsupported(header.kind)),
        };
        // Lỗi v1 #1: FLUSH khi chưa đàm phán.
        if header.is_flush() && !self.flush_ok() {
            return Err(StrictError::FlushNotNegotiated);
        }
        // Lỗi v1 #2: độ dài dữ liệu không khớp.
        if data_len != want {
            return Err(StrictError::LengthMismatch {
                want,
                got: data_len,
            });
        }
        // Dải sector phải nằm trong dung lượng (FLUSH không có sector).
        if !header.is_flush() {
            let end = header.sector.saturating_add(sectors);
            if end > self.config.capacity_sectors {
                return Err(StrictError::OutOfRange {
                    sector: header.sector,
                    sectors,
                    capacity: self.config.capacity_sectors,
                });
            }
        }
        Ok(S_OK)
    }
}
