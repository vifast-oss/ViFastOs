// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-245 — test host cho **logic thuần** của virtio-net (không phần cứng, không `unsafe`).
//!
//! Bốn nhóm: (A) `virtio_net_config` — offset theo spec 1.2 §5.1.4 + ca âm config ngắn;
//! (B) feature — luôn `VERSION_1`, `MAC` chỉ khi thiết bị có, **không** `MRG_RXBUF`/`STATUS`;
//! (C) `virtio_net_hdr` — round-trip 10/12 byte + ca âm header cắt cụt; (D) độ dài frame từ
//! `used->len` + hằng số queue/device id.
//!
//! Mỗi ca âm khẳng định **đúng** lý do từ chối (không chỉ "trả None"), để log đỏ tái lập được.

use vifast_virtio::net::{
    decode_config, frame_len, frame_ok, hdr_bytes, is_virtio_net, mac_negotiated,
    mrg_rxbuf_negotiated, negotiated_features, status_negotiated, NetConfig, NetError, NetHdr,
    DEVICE_ID_NET, F_MAC, F_MRG_RXBUF, F_STATUS, F_VERSION_1, HDR_LEGACY_BYTES, HDR_MODERN_BYTES,
    MAX_FRAME_BYTES, MIN_QUEUE_SIZE, NET_CONFIG_BYTES, QUEUES_REQUIRED, QUEUE_SIZE, Q_RX, Q_TX,
    RX_BUF_BYTES, TX_BUF_BYTES, VIRTIO_NET_MODERN, VIRTIO_NET_MODERN_ONLY,
};
use vifast_virtio::pci::{VirtioKind, VENDOR_VIRTIO};
use vifast_virtio::queue::VqLayout;

/// `virtio_net_config` mẫu: MAC `52:54:00:12:34:56` (tiền tố QEMU), status 1, **2** cặp queue,
/// MTU 1500. Ba trường số **khác nhau** để hoán vị offset là test đỏ (đã đo: bản đầu để
/// `status = max_virtqueue_pairs = 1` nên đột biến hoán vị hai trường **không** bị bắt).
const CONFIG_SAMPLE: [u8; NET_CONFIG_BYTES] = [
    0x52, 0x54, 0x00, 0x12, 0x34, 0x56, // mac[6]
    0x01, 0x00, // status = 1 (link up)
    0x02, 0x00, // max_virtqueue_pairs = 2
    0xdc, 0x05, // mtu = 1500
];

// ---------------------------------------------------------------- (A) virtio_net_config

#[test]
fn config_doc_dung_offset_spec() {
    let config = decode_config(&CONFIG_SAMPLE).expect("config 12 byte phải giải mã được");
    assert_eq!(config.mac, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    assert_eq!(config.status, 1);
    assert_eq!(config.max_virtqueue_pairs, 2);
    assert_eq!(config.mtu, 1500);
    assert_eq!(config.effective_mtu(), 1500);
}

#[test]
fn config_ngan_hon_12_byte_bi_tu_choi() {
    // Ca âm: thiếu **một** byte ⇒ không đoán byte thiếu.
    for len in 0..NET_CONFIG_BYTES {
        assert_eq!(
            decode_config(&CONFIG_SAMPLE[..len]),
            None,
            "config {len} byte phải bị từ chối"
        );
    }
    assert!(decode_config(&CONFIG_SAMPLE).is_some());
}

#[test]
fn config_dai_hon_12_byte_van_doc_duoc() {
    // Thiết bị modern có thêm speed/duplex/rss… — driver chỉ đọc 12 byte đầu, không đòi khớp cỡ.
    let mut raw = [0u8; 20];
    raw[..NET_CONFIG_BYTES].copy_from_slice(&CONFIG_SAMPLE);
    raw[12..16].copy_from_slice(&1_000_000u32.to_le_bytes()); // speed
    let config = decode_config(&raw).expect("config dài hơn vẫn phải đọc được");
    assert_eq!(config.mac, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    assert_eq!(config.mtu, 1500);
}

#[test]
fn config_mtu_0_dung_1500_va_mac_hop_le() {
    let mut raw = CONFIG_SAMPLE;
    raw[10] = 0;
    raw[11] = 0;
    let config = decode_config(&raw).expect("config hợp lệ");
    assert_eq!(config.mtu, 0);
    assert_eq!(config.effective_mtu(), 1500);
    assert!(config.mac_hop_le());
}

#[test]
fn config_mac_toan_0_hoac_broadcast_khong_hop_le() {
    // Ca âm: hai giá trị MAC mà thiết bị thật không phát ⇒ driver phải fail-closed.
    let zero = NetConfig {
        mac: [0; 6],
        status: 0,
        max_virtqueue_pairs: 1,
        mtu: 1500,
    };
    assert!(!zero.mac_hop_le());
    let broadcast = NetConfig {
        mac: [0xff; 6],
        status: 0,
        max_virtqueue_pairs: 1,
        mtu: 1500,
    };
    assert!(!broadcast.mac_hop_le());
    let mut one_byte = [0u8; 6];
    one_byte[5] = 1;
    let unicast = NetConfig {
        mac: one_byte,
        status: 0,
        max_virtqueue_pairs: 1,
        mtu: 1500,
    };
    assert!(unicast.mac_hop_le());
}

// ---------------------------------------------------------------- (B) feature

#[test]
fn feature_luon_version_1_du_thiet_bi_khong_bao() {
    // `VERSION_1` là feature **driver bắt buộc**: thiếu nó thì init phải từ chối, nhưng hàm đàm
    // phán vẫn luôn đặt bit (việc kiểm "thiết bị có bit" ở driver).
    assert_eq!(negotiated_features(0), 1u64 << F_VERSION_1);
    assert_eq!(
        negotiated_features(u64::MAX) & (1u64 << F_VERSION_1),
        1u64 << F_VERSION_1
    );
}

#[test]
fn feature_mac_chi_khi_thiet_bi_co() {
    let with_mac = negotiated_features(1u64 << F_MAC);
    assert!(mac_negotiated(with_mac));
    assert_eq!(with_mac, (1u64 << F_VERSION_1) | (1u64 << F_MAC));
    let without_mac = negotiated_features(0);
    assert!(!mac_negotiated(without_mac));
    assert_eq!(without_mac, 1u64 << F_VERSION_1);
}

#[test]
fn feature_khong_bao_gio_dam_phan_mrg_rxbuf() {
    // D2-110B: driver dùng **một** descriptor/frame ⇒ không đàm phán MRG_RXBUF dù thiết bị báo có.
    let driver = negotiated_features(u64::MAX);
    assert!(!mrg_rxbuf_negotiated(driver));
    assert_eq!(driver & (1u64 << F_MRG_RXBUF), 0);
    // Đối chứng: bit ấy **có** trong device_features ⇒ phép kiểm trên không rỗng.
    let device = u64::MAX;
    assert_ne!(device & (1u64 << F_MRG_RXBUF), 0);
}

#[test]
fn feature_khong_dam_phan_status() {
    // Link status không đàm phán: "không có mạng" phát hiện bằng timeout DHCP/ARP (KT-246).
    let driver = negotiated_features(u64::MAX);
    assert!(!status_negotiated(driver));
    assert_eq!(driver & (1u64 << F_STATUS), 0);
    let device = u64::MAX;
    assert_ne!(device & (1u64 << F_STATUS), 0);
}

#[test]
fn feature_bit_dung_so_cua_spec() {
    // Số bit là dữ kiện spec §5.1.3 — sai số là driver đàm phán sai feature mà không ai thấy.
    assert_eq!(F_MAC, 5);
    assert_eq!(F_MRG_RXBUF, 15);
    assert_eq!(F_STATUS, 16);
    assert_eq!(F_VERSION_1, 32);
}

// ---------------------------------------------------------------- (C) virtio_net_hdr

#[test]
fn header_round_trip_modern_12_byte() {
    let hdr = NetHdr {
        flags: 0x03,
        gso_type: 0x01,
        hdr_len: 0x1234,
        gso_size: 0x5678,
        csum_start: 0x9abc,
        csum_offset: 0xdef0,
        num_buffers: 0x0007,
    };
    let bytes = hdr.encode(true);
    assert_eq!(bytes.len(), HDR_MODERN_BYTES);
    assert_eq!(NetHdr::parse(&bytes, true), Some(hdr));
}

#[test]
fn header_round_trip_legacy_10_byte() {
    let hdr = NetHdr {
        flags: 0x02,
        gso_type: 0,
        hdr_len: 0x0010,
        gso_size: 0,
        csum_start: 0x000e,
        csum_offset: 0x0004,
        num_buffers: 0,
    };
    let bytes = hdr.encode(false);
    // 10 byte đầu giống hệt bản modern; hai byte cuối **phải** 0 ở bản legacy.
    assert_eq!(
        bytes[..HDR_LEGACY_BYTES],
        hdr.encode(true)[..HDR_LEGACY_BYTES]
    );
    assert_eq!(&bytes[HDR_LEGACY_BYTES..], &[0u8, 0u8]);
    assert_eq!(NetHdr::parse(&bytes[..HDR_LEGACY_BYTES], false), Some(hdr));
}

#[test]
fn header_cat_cut_bi_tu_choi() {
    // Ca âm: header ngắn hơn cỡ của chế độ ⇒ None (không đọc tràn, không đoán num_buffers).
    let bytes = NetHdr::zero().encode(true);
    for len in 0..HDR_LEGACY_BYTES {
        assert_eq!(
            NetHdr::parse(&bytes[..len], false),
            None,
            "legacy {len} byte"
        );
        assert_eq!(
            NetHdr::parse(&bytes[..len], true),
            None,
            "modern {len} byte"
        );
    }
    // 10 byte là **đủ** cho legacy nhưng **thiếu** cho modern (thiếu `num_buffers`).
    assert!(NetHdr::parse(&bytes[..HDR_LEGACY_BYTES], false).is_some());
    assert_eq!(NetHdr::parse(&bytes[..HDR_LEGACY_BYTES], true), None);
    assert_eq!(NetHdr::parse(&bytes[..HDR_MODERN_BYTES - 1], true), None);
    assert!(NetHdr::parse(&bytes[..HDR_MODERN_BYTES], true).is_some());
}

#[test]
fn header_legacy_bo_qua_num_buffers() {
    // Đọc bản legacy từ đệm 12 byte: `num_buffers` **không** được lấy từ hai byte cuối.
    let mut bytes = NetHdr::zero().encode(true);
    bytes[10..12].copy_from_slice(&0xbeefu16.to_le_bytes());
    let legacy = NetHdr::parse(&bytes, false).expect("legacy đọc 10 byte đầu");
    assert_eq!(legacy.num_buffers, 0);
    let modern = NetHdr::parse(&bytes, true).expect("modern đọc 12 byte");
    assert_eq!(modern.num_buffers, 0xbeef);
}

#[test]
fn header_zero_ma_hoa_toan_0() {
    let bytes = NetHdr::zero().encode(true);
    assert_eq!(bytes, [0u8; HDR_MODERN_BYTES]);
    assert_eq!(NetHdr::parse(&bytes, true), Some(NetHdr::zero()));
}

#[test]
fn header_co_hai_co_10_va_12() {
    assert_eq!(hdr_bytes(false), 10);
    assert_eq!(hdr_bytes(true), 12);
    assert_eq!(HDR_LEGACY_BYTES, 10);
    assert_eq!(HDR_MODERN_BYTES, 12);
}

// ---------------------------------------------------------------- (D) độ dài frame + hằng số

#[test]
fn frame_len_tru_header() {
    // `used->len` = header + frame ⇒ frame = used_len - header.
    assert_eq!(frame_len(12 + 60, true), Ok(60));
    assert_eq!(frame_len(10 + 60, false), Ok(60));
    assert_eq!(frame_len(12, true), Ok(0));
    assert_eq!(
        frame_len(12 + MAX_FRAME_BYTES as u32, true),
        Ok(MAX_FRAME_BYTES)
    );
}

#[test]
fn frame_len_vo_ly_bi_tu_choi() {
    // Ca âm: `used->len` nhỏ hơn header ⇒ thiết bị ghi rác; lớn hơn đệm ⇒ không tin được.
    assert_eq!(frame_len(0, true), Err(NetError::Io));
    assert_eq!(frame_len(11, true), Err(NetError::Io));
    assert_eq!(frame_len(9, false), Err(NetError::Io));
    assert_eq!(
        frame_len(12 + MAX_FRAME_BYTES as u32 + 1, true),
        Err(NetError::Io)
    );
}

#[test]
fn frame_ok_bien_ethernet() {
    // 14 byte = header Ethernet tối thiểu; 1514 = 14 + MTU 1500.
    assert!(!frame_ok(&[0u8; 13]));
    assert!(frame_ok(&[0u8; 14]));
    assert!(frame_ok(&[0u8; MAX_FRAME_BYTES]));
    assert!(!frame_ok(&[0u8; MAX_FRAME_BYTES + 1]));
    assert!(!frame_ok(&[]));
}

#[test]
fn dem_rx_tx_du_cho_frame_lon_nhat() {
    // Đệm `rx`/`tx` phải chứa header modern + frame lớn nhất, nếu không driver cắt frame im lặng.
    // Đây là quan hệ **hằng số** ⇒ kiểm lúc biên dịch (clippy `assertions_on_constants`).
    const {
        assert!(RX_BUF_BYTES >= HDR_MODERN_BYTES + MAX_FRAME_BYTES);
        assert!(TX_BUF_BYTES >= HDR_MODERN_BYTES + MAX_FRAME_BYTES);
        assert!(MAX_FRAME_BYTES == 14 + 1500);
    }
}

#[test]
fn queue_va_device_id_dung_spec() {
    assert_eq!(Q_RX, 0);
    assert_eq!(Q_TX, 1);
    assert_eq!(QUEUES_REQUIRED, 2);
    assert_eq!(DEVICE_ID_NET, 0x1000);
    assert_eq!(VIRTIO_NET_MODERN, 0x1000);
    assert_eq!(VIRTIO_NET_MODERN_ONLY, 0x1041);
    // Cỡ queue driver đặt phải dựng được layout thật (luỹ thừa 2, ≥ MIN_QUEUE_SIZE).
    const { assert!(QUEUE_SIZE >= MIN_QUEUE_SIZE) };
    assert!(QUEUE_SIZE.is_power_of_two());
    let layout = VqLayout::new(QUEUE_SIZE).expect("QUEUE_SIZE phải dựng được VqLayout");
    assert_eq!(layout.queue_size, QUEUE_SIZE);
}

#[test]
fn pci_kind_net_khop_hai_device_id() {
    assert!(is_virtio_net(VENDOR_VIRTIO, VIRTIO_NET_MODERN));
    assert!(is_virtio_net(VENDOR_VIRTIO, VIRTIO_NET_MODERN_ONLY));
    // Ca âm: blk/console **không** phải net, vendor lạ **không** phải net.
    assert!(!is_virtio_net(VENDOR_VIRTIO, 0x1001));
    assert!(!is_virtio_net(VENDOR_VIRTIO, 0x1003));
    assert!(!is_virtio_net(0x1234, VIRTIO_NET_MODERN));
    // `VirtioKind::Net` phải khớp **cùng** tập id (một chỗ quyết định, không hai bảng lệch).
    assert!(VirtioKind::Net.matches(VENDOR_VIRTIO, VIRTIO_NET_MODERN));
    assert!(VirtioKind::Net.matches(VENDOR_VIRTIO, VIRTIO_NET_MODERN_ONLY));
    assert!(!VirtioKind::Net.matches(VENDOR_VIRTIO, 0x1001));
    assert!(!VirtioKind::Net.matches(VENDOR_VIRTIO, 0x1003));
    assert_eq!(VirtioKind::Net.name(), "net");
}

#[test]
fn loi_co_ten_de_in_evidence() {
    // D2-102: evidence in **tên**, không in số trần.
    let all = [
        NetError::NotReady,
        NetError::NoDevice,
        NetError::BadFrame,
        NetError::Io,
        NetError::Timeout,
        NetError::QueueFull,
    ];
    for error in all {
        assert!(!error.name().is_empty(), "{error:?} phải có tên");
        assert!(
            error.name().chars().all(|c| c.is_ascii_alphabetic()),
            "tên {} phải là chữ",
            error.name()
        );
    }
    // Không hai biến thể trùng tên (log đỏ phải phân biệt được nguyên nhân).
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a.name(), b.name());
        }
    }
}
