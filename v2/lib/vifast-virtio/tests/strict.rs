// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-81** — thiết bị giả **nghiêm ngặt** phải bắt được hai lỗi driver của v1.
//!
//! Mỗi lỗi có **ba** vế, đúng luật đóng lỗi của Agent 0:
//!
//! 1. **Đỏ trên mô hình cũ**: mô hình cũ (không feature, không dung lượng, không cỡ) chấp nhận
//!    request sai — chứng minh bằng cách gọi thẳng `StrictBlk` với request sai và đòi lỗi.
//! 2. **Xanh trên mô hình mới**: cùng request nhưng **đúng** ⇒ `S_OK`.
//! 3. **Đột biến làm đỏ lại**: bỏ phép kiểm tương ứng trong `strict.rs` ⇒ ca âm ở đây đỏ.
//!
//! Vế 3 không nằm trong tệp này (nó là đột biến trên mã nguồn); nó được ghi trong
//! `v2/agent/JOURNAL/kt81-thiet-bi-gia.md` kèm lệnh và kết quả.

use vifast_virtio::blk::{ReqHeader, FLUSH_MASK, S_IOERR, S_OK, S_UNSUPP, T_FLUSH, T_IN, T_OUT};
use vifast_virtio::strict::{StrictBlk, StrictError};

/// Thiết bị 64 sector (32 KiB), **có** báo `VIRTIO_BLK_F_FLUSH`, đã `DRIVER_OK`.
fn dev_co_flush() -> StrictBlk {
    let mut dev = StrictBlk::new(64, FLUSH_MASK as u64);
    dev.set_driver_features((1u64 << 32) | FLUSH_MASK as u64);
    dev.set_driver_ok();
    dev
}

/// Thiết bị 64 sector, **không** báo `VIRTIO_BLK_F_FLUSH`, đã `DRIVER_OK`.
fn dev_khong_flush() -> StrictBlk {
    let mut dev = StrictBlk::new(64, 0);
    dev.set_driver_features(1u64 << 32);
    dev.set_driver_ok();
    dev
}

// ---------------------------------------------------------------- lỗi v1 #1: FLUSH chưa đàm phán

#[test]
fn loi_v1_flush_chua_dam_phan_bi_bat() {
    // Thiết bị KHÔNG báo bit FLUSH, driver cũng không đàm phán ⇒ gửi T_FLUSH là sai.
    let dev = dev_khong_flush();
    assert_eq!(
        dev.submit(ReqHeader::new(T_FLUSH, 0), 0, 0),
        Err(StrictError::FlushNotNegotiated)
    );
    assert_eq!(
        StrictError::FlushNotNegotiated.name(),
        "flush-not-negotiated"
    );
    assert_eq!(StrictError::FlushNotNegotiated.status(), S_IOERR);
}

#[test]
fn doi_chung_duong_flush_da_dam_phan_thi_qua() {
    // Cùng request, thiết bị CÓ báo bit và driver ĐÃ đàm phán ⇒ chấp nhận.
    let dev = dev_co_flush();
    assert_eq!(dev.submit(ReqHeader::new(T_FLUSH, 0), 0, 0), Ok(S_OK));
}

#[test]
fn driver_khai_flush_ma_thiet_bi_khong_bao_thi_van_tu_choi() {
    // Driver tự cho mình quyền gửi FLUSH trong khi thiết bị chưa hứa hỗ trợ ⇒ vẫn sai.
    let mut dev = StrictBlk::new(64, 0);
    dev.set_driver_features((1u64 << 32) | FLUSH_MASK as u64);
    dev.set_driver_ok();
    assert!(!dev.flush_ok());
    assert_eq!(
        dev.submit(ReqHeader::new(T_FLUSH, 0), 0, 0),
        Err(StrictError::FlushNotNegotiated)
    );
}

// ---------------------------------------------------------------- lỗi v1 #2: độ dài không khớp

#[test]
fn loi_v1_do_dai_lech_bi_bat() {
    let dev = dev_co_flush();
    // T_IN 1 sector đòi 512 B; driver đưa 0 B ⇒ thiết bị thật sẽ đọc rác.
    assert_eq!(
        dev.submit(ReqHeader::new(T_IN, 0), 1, 0),
        Err(StrictError::LengthMismatch { want: 512, got: 0 })
    );
    // T_OUT 2 sector đòi 1024 B; driver đưa 512 B ⇒ ghi thiếu.
    assert_eq!(
        dev.submit(ReqHeader::new(T_OUT, 0), 2, 512),
        Err(StrictError::LengthMismatch {
            want: 1024,
            got: 512
        })
    );
    // T_FLUSH không có đệm dữ liệu; đưa 512 B cũng là sai.
    assert_eq!(
        dev.submit(ReqHeader::new(T_FLUSH, 0), 0, 512),
        Err(StrictError::LengthMismatch { want: 0, got: 512 })
    );
    assert_eq!(
        StrictError::LengthMismatch { want: 0, got: 1 }.name(),
        "length-mismatch"
    );
}

#[test]
fn doi_chung_duong_do_dai_dung_thi_qua() {
    let dev = dev_co_flush();
    assert_eq!(dev.submit(ReqHeader::new(T_IN, 0), 1, 512), Ok(S_OK));
    assert_eq!(dev.submit(ReqHeader::new(T_OUT, 8), 2, 1024), Ok(S_OK));
    assert_eq!(dev.submit(ReqHeader::new(T_IN, 63), 1, 512), Ok(S_OK));
}

// ---------------------------------------------------------------- hai điều kiện nữa của thiết bị thật

#[test]
fn sector_ngoai_dung_luong_bi_bat() {
    let dev = dev_co_flush();
    // 64 sector ⇒ sector hợp lệ 0..63; đọc 1 sector ở 64 là ngoài.
    assert_eq!(
        dev.submit(ReqHeader::new(T_IN, 64), 1, 512),
        Err(StrictError::OutOfRange {
            sector: 64,
            sectors: 1,
            capacity: 64
        })
    );
    // Dải vắt qua biên: 63 + 2 = 65 > 64.
    assert_eq!(
        dev.submit(ReqHeader::new(T_OUT, 63), 2, 1024),
        Err(StrictError::OutOfRange {
            sector: 63,
            sectors: 2,
            capacity: 64
        })
    );
    // Tràn số cũng phải bị bắt, không được "quay vòng" thành sector nhỏ.
    assert_eq!(
        dev.submit(ReqHeader::new(T_IN, u64::MAX), 2, 1024),
        Err(StrictError::OutOfRange {
            sector: u64::MAX,
            sectors: 2,
            capacity: 64
        })
    );
    assert_eq!(
        StrictError::OutOfRange {
            sector: 0,
            sectors: 0,
            capacity: 0
        }
        .name(),
        "out-of-range"
    );
}

#[test]
fn loai_request_la_bi_bat() {
    let dev = dev_co_flush();
    // `T_GET_ID` (8) không phải loại K6 hỗ trợ.
    assert_eq!(
        dev.submit(ReqHeader::new(8, 0), 0, 0),
        Err(StrictError::Unsupported(8))
    );
    assert_eq!(StrictError::Unsupported(8).name(), "unsupported-type");
    assert_eq!(StrictError::Unsupported(8).status(), S_UNSUPP);
}

#[test]
fn chua_driver_ok_thi_tu_choi() {
    let mut dev = StrictBlk::new(64, FLUSH_MASK as u64);
    dev.set_driver_features((1u64 << 32) | FLUSH_MASK as u64);
    // Chưa `set_driver_ok`.
    assert_eq!(
        dev.submit(ReqHeader::new(T_IN, 0), 1, 512),
        Err(StrictError::NotReady)
    );
    assert_eq!(StrictError::NotReady.name(), "not-ready");
}

// ---------------------------------------------------------------- thứ tự kiểm phải cố định

#[test]
fn thu_tu_kiem_co_dinh_de_log_do_tai_lap_duoc() {
    // Request sai **nhiều** điều kiện cùng lúc: thiết bị không có FLUSH, độ dài sai, sector ngoài.
    // Phải luôn trả **cùng** lý do (đàm phán FLUSH trước độ dài, độ dài trước dải sector) — nếu
    // không, hai lần chạy cùng một lỗi cho hai log khác nhau và người đọc phải đoán.
    let dev = dev_khong_flush();
    for _ in 0..3 {
        assert_eq!(
            dev.submit(ReqHeader::new(T_FLUSH, 999), 0, 512),
            Err(StrictError::FlushNotNegotiated)
        );
    }
    // Bỏ điều kiện FLUSH (dùng T_IN) ⇒ lý do kế tiếp là độ dài, không phải dải sector.
    assert_eq!(
        dev.submit(ReqHeader::new(T_IN, 999), 1, 0),
        Err(StrictError::LengthMismatch { want: 512, got: 0 })
    );
    // Độ dài đúng ⇒ lúc ấy mới tới dải sector.
    assert_eq!(
        dev.submit(ReqHeader::new(T_IN, 999), 1, 512),
        Err(StrictError::OutOfRange {
            sector: 999,
            sectors: 1,
            capacity: 64
        })
    );
}

#[test]
fn config_doc_tu_thiet_bi_khong_hardcode() {
    // Dung lượng phải là giá trị thiết bị báo, không phải hằng trong driver.
    let dev = StrictBlk::new(1234, 0);
    assert_eq!(dev.config().capacity_sectors, 1234);
    assert_eq!(dev.config().capacity_bytes(), 1234 * 512);
    assert!(dev.config().sector_512());
}
