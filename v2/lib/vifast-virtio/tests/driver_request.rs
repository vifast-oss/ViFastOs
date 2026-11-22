// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-81 (§A5a)** — mã **dựng request của driver virtio-blk thật** chạy qua `StrictBlk`.
//!
//! # Vì sao tệp này
//!
//! `tests/strict.rs` chứng minh `StrictBlk` **bắt được** hai lỗi driver v1 — nhưng nó tự dựng
//! `ReqHeader` bằng tay. Nghĩa là nó chứng minh **mô hình**, không chứng minh **driver**. Một driver
//! gửi request sai vẫn có thể qua, vì test không bao giờ chạy mã của driver.
//!
//! Tệp này đóng đúng lỗ ấy: nó gọi [`vifast_virtio::blk::build_request`] — **chính hàm** mà
//! `v2/kernel/vifast-plat-riscv64/src/virtio_blk.rs::submit` gọi (cùng mã, biên dịch vào ảnh) — rồi
//! đưa kết quả cho `StrictBlk` kiểm. Không chép lại công thức nào.
//!
//! # Phủ
//!
//! Đọc / ghi / flush / **nhiều sector** / **sector cuối** / ngoài dải / chưa đàm phán FLUSH /
//! độ dài lệch — mỗi ca một lý do từ chối riêng.
//!
//! # Đột biến (chạy tay, ghi trong `JOURNAL/kt81-thiet-bi-gia.md`)
//!
//! - Tái tạo lỗi v1 #1 trong `negotiated_features` (không đàm phán `VIRTIO_BLK_F_FLUSH`) ⇒ ca
//!   `flush_sau_dam_phan` ĐỎ với `FlushNotNegotiated`.
//! - Tái tạo lỗi v1 #2 trong `build_request` (`data_len` = 0 cho `T_OUT`) ⇒ ca
//!   `ghi_mot_sector` ĐỎ với `LengthMismatch`.

use vifast_virtio::blk::{
    build_request, negotiated_features, ReqHeader, FLUSH_MASK, T_FLUSH, T_IN, T_OUT,
};
use vifast_virtio::strict::{StrictBlk, StrictError};

/// Dung lượng thiết bị dùng trong hầu hết ca (sector 512 B).
const CAP: u64 = 64;
/// Feature thiết bị **có** `VIRTIO_BLK_F_FLUSH` (và `VIRTIO_F_VERSION_1`).
const DEV_FLUSH: u64 = (1u64 << 32) | FLUSH_MASK as u64;
/// Feature thiết bị **không** có `VIRTIO_BLK_F_FLUSH`.
const DEV_NO_FLUSH: u64 = 1u64 << 32;

/// Thiết bị đã `DRIVER_OK`, driver đã đàm phán đúng `negotiated_features(device)`.
fn dev(device_features: u64) -> StrictBlk {
    let mut d = StrictBlk::new(CAP, device_features);
    d.set_driver_features(negotiated_features(device_features));
    d.set_driver_ok();
    d
}

/// Đưa một `BuiltRequest` (do **driver** dựng) qua thiết bị nghiêm ngặt.
fn submit_built(d: &StrictBlk, kind: u32, sector: u64, sectors: u32) -> Result<u8, StrictError> {
    let req = build_request(kind, sector, sectors);
    // Header phải giải mã lại đúng `(kind, sector)` — nếu `encode`/`decode` lệch nhau thì mọi ca
    // dưới đây sẽ "qua" trên một request mà thiết bị thật hiểu khác.
    let decoded = ReqHeader::decode(&req.header);
    assert_eq!(decoded.kind, kind, "header giải mã sai kind");
    assert_eq!(decoded.sector, sector, "header giải mã sai sector");
    d.submit(decoded, u64::from(req.sectors), req.data_len)
}

// ---------------------------------------------------------------- ca dương

#[test]
fn doc_mot_sector() {
    let d = dev(DEV_FLUSH);
    assert_eq!(submit_built(&d, T_IN, 0, 1), Ok(0));
}

#[test]
fn ghi_mot_sector() {
    let d = dev(DEV_FLUSH);
    assert_eq!(submit_built(&d, T_OUT, 0, 1), Ok(0));
}

#[test]
fn flush_sau_dam_phan() {
    let d = dev(DEV_FLUSH);
    assert_eq!(submit_built(&d, T_FLUSH, 0, 0), Ok(0));
}

#[test]
fn doc_nhieu_sector() {
    let d = dev(DEV_FLUSH);
    assert_eq!(submit_built(&d, T_IN, 8, 4), Ok(0));
    assert_eq!(submit_built(&d, T_OUT, 8, 4), Ok(0));
}

#[test]
fn sector_cuoi() {
    let d = dev(DEV_FLUSH);
    // Sector cuối cùng của thiết bị: `CAP - 1`, một sector ⇒ vừa đúng mép.
    assert_eq!(submit_built(&d, T_IN, CAP - 1, 1), Ok(0));
    assert_eq!(submit_built(&d, T_OUT, CAP - 1, 1), Ok(0));
    // Nhiều sector kết thúc đúng mép cũng hợp lệ.
    assert_eq!(submit_built(&d, T_IN, CAP - 4, 4), Ok(0));
}

// ---------------------------------------------------------------- ca âm

#[test]
fn doc_qua_mep_bi_tu_choi() {
    let d = dev(DEV_FLUSH);
    // Bắt đầu đúng ở `CAP` ⇒ ngoài dải.
    assert_eq!(
        submit_built(&d, T_IN, CAP, 1),
        Err(StrictError::OutOfRange {
            sector: CAP,
            sectors: 1,
            capacity: CAP,
        })
    );
    // Kết thúc vượt mép một sector ⇒ ngoài dải.
    assert_eq!(
        submit_built(&d, T_OUT, CAP - 1, 2),
        Err(StrictError::OutOfRange {
            sector: CAP - 1,
            sectors: 2,
            capacity: CAP,
        })
    );
}

#[test]
fn flush_khi_chua_dam_phan_bi_tu_choi() {
    // Lỗi v1 #1: thiết bị **không** báo FLUSH ⇒ driver không đàm phán ⇒ gửi `T_FLUSH` là sai.
    let d = dev(DEV_NO_FLUSH);
    assert_eq!(
        submit_built(&d, T_FLUSH, 0, 0),
        Err(StrictError::FlushNotNegotiated)
    );
}

#[test]
fn do_dai_lech_bi_tu_choi() {
    let d = dev(DEV_FLUSH);
    // Lỗi v1 #2: driver khai độ dài **khác** độ dài mà `(loại, số sector)` đòi.
    let req = build_request(T_OUT, 0, 1);
    assert_eq!(
        d.submit(ReqHeader::decode(&req.header), 1, 0),
        Err(StrictError::LengthMismatch { want: 512, got: 0 })
    );
    // Chiều ngược lại: khai nhiều hơn số sector đòi.
    assert_eq!(
        d.submit(ReqHeader::decode(&req.header), 1, 1024),
        Err(StrictError::LengthMismatch {
            want: 512,
            got: 1024,
        })
    );
    // FLUSH mà có đệm dữ liệu cũng sai.
    let f = build_request(T_FLUSH, 0, 0);
    assert_eq!(
        d.submit(ReqHeader::decode(&f.header), 0, 512),
        Err(StrictError::LengthMismatch { want: 0, got: 512 })
    );
}

#[test]
fn loai_la_bi_tu_choi() {
    let d = dev(DEV_FLUSH);
    // `T_GET_ID` (8) không phải loại driver K6 dùng ⇒ thiết bị từ chối, không đoán.
    assert_eq!(submit_built(&d, 8, 0, 0), Err(StrictError::Unsupported(8)));
}

// ---------------------------------------------------------------- hình dạng request

#[test]
fn hinh_dang_request_dung() {
    // `FLUSH`: không đệm, status ở descriptor 1.
    let f = build_request(T_FLUSH, 0, 0);
    assert_eq!(f.data_len, 0);
    assert_eq!(f.status_index, 1);
    // `IN`/`OUT`: đệm = số sector × 512, status ở descriptor 2.
    for kind in [T_IN, T_OUT] {
        let r = build_request(kind, 3, 1);
        assert_eq!(r.data_len, 512);
        assert_eq!(r.status_index, 2);
        let r4 = build_request(kind, 3, 4);
        assert_eq!(r4.data_len, 2048);
        assert_eq!(r4.status_index, 2);
    }
}

// ---------------------------------------------------------------- cấu trúc: driver dùng chung mã

/// Nguồn driver **thật** — nhúng lúc biên dịch để ca này không thể "trôi" khỏi mã đang có.
///
/// **KT-334** — sau khi gộp transport, mã dựng request + vòng queue nằm trong
/// [`vifast_virtio::blkdev`] (logíc dùng chung 3 arch), còn `vifast-plat-*/virtio_blk.rs` chỉ
/// còn adapter MMIO/DMA. Vì vậy ca này kiểm **cả hai**: (a) `blkdev` là nơi dựng request, và
/// (b) tệp adapter của riscv64 **không** quay lại tự dựng header (không sinh nguồn thứ hai).
const DRIVER_SRC: &str = include_str!("../src/blkdev.rs");

/// Adapter của một nền tảng (chỉ còn MMIO/DMA/clock) — phải **không** chứa `ReqHeader::new(`.
const ADAPTER_SRC: &str = include_str!("../../../kernel/vifast-plat-riscv64/src/virtio_blk.rs");

#[test]
fn driver_that_dung_build_request() {
    // Nếu driver quay lại tự dựng header (`ReqHeader::new`) thì test host ở trên không còn chứng
    // minh gì về driver nữa — đúng lỗ mà §A5a đóng. Ca này giữ cho hai bên là **một**.
    assert!(
        DRIVER_SRC.contains("build_request("),
        "driver virtio_blk KHÔNG gọi build_request ⇒ test host không còn kiểm mã driver"
    );
    assert!(
        !DRIVER_SRC.contains("ReqHeader::new("),
        "blkdev tự dựng header (ReqHeader::new) ⇒ lại có hai nguồn"
    );
    // **KT-334** — adapter nền tảng **không** được dựng lại header/độ dài: nếu có, hai nguồn
    // lại xuất hiện ở tầng plat và test host ở trên không còn chứng minh gì về mã đang chạy.
    assert!(
        !ADAPTER_SRC.contains("ReqHeader::new("),
        "adapter virtio_blk của plat tự dựng header (ReqHeader::new) ⇒ lại có hai nguồn"
    );
    // Độ dài descriptor phải lấy từ request đã dựng, không viết `SECTOR_BYTES` lần thứ hai.
    assert!(
        DRIVER_SRC.contains("len: req.data_len"),
        "descriptor dữ liệu không dùng req.data_len ⇒ độ dài có thể lệch header"
    );
}
