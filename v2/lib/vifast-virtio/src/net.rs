// SPDX-License-Identifier: MIT OR Apache-2.0
//! `virtio_net` — **logic thuần** device id, feature, `virtio_net_config`, `virtio_net_hdr`
//! (§5.1 Virtio 1.2) — KT-245.
//!
//! # Phạm vi
//!
//! Chỉ phần **quyết định trên byte**: giải mã `virtio_net_config`, chọn bộ feature, mã hoá/giải mã
//! `virtio_net_hdr`, suy độ dài frame từ `used->len`. Không MMIO, không `unsafe`, không cấp phát ⇒
//! chạy host test (D2-05: đỏ trước rồi mới xanh). Driver thật ở
//! `kernel/vifast-plat-riscv64/src/virtio_net.rs` chỉ còn phần chạm thanh ghi.
//!
//! # Vì sao **không** đàm phán `MRG_RXBUF` (D2-110B)
//!
//! `VIRTIO_NET_F_MRG_RXBUF` (bit 15) đổi **ngữ nghĩa** của `virtio_net_hdr`: khi đàm phán, thiết bị
//! có thể dùng nhiều descriptor cho một frame và `num_buffers` mang nghĩa "frame này gồm bao nhiêu
//! buffer". Driver KT-245 dùng **một** descriptor device-writable cho mỗi frame ⇒ **không** đàm phán
//! bit này, và [`negotiated_features`] không bao giờ bật nó kể cả khi thiết bị báo có. Đây đúng
//! luật D2-110B: chỉ dùng lệnh/ngữ nghĩa của feature **đã** đàm phán.
//!
//! # Vì sao `STATUS` có hằng nhưng **không** đàm phán
//!
//! `VIRTIO_NET_F_STATUS` (bit 16) chỉ thêm trường `status` (link up/down) vào config. KT-245 phát
//! hiện "không có mạng" bằng **timeout DHCP/ARP** (fail-closed, KT-246), không bằng bit link ⇒
//! không đàm phán. Hằng số vẫn ở đây để test/evidence đọc được bit và để lần mở sau không phải
//! đoán số.

/// `virtio-net` bản transitional (device id `0x1000`) — KT-245 nhắm id này.
pub const VIRTIO_NET_MODERN: u16 = 0x1000;
/// `virtio-net` bản modern-only (`disable-legacy=on`) — cùng driver.
pub const VIRTIO_NET_MODERN_ONLY: u16 = 0x1041;
/// Device id mà driver KT-245 tìm trên bus PCI (`0x1000`).
pub const DEVICE_ID_NET: u16 = VIRTIO_NET_MODERN;

/// `VIRTIO_NET_F_MAC` (bit 5) — thiết bị cung cấp `mac[6]` trong config.
pub const F_MAC: u32 = 5;
/// `VIRTIO_NET_F_MRG_RXBUF` (bit 15) — **không** đàm phán (xem đầu tệp).
pub const F_MRG_RXBUF: u32 = 15;
/// `VIRTIO_NET_F_STATUS` (bit 16) — có `status` (link) trong config; **không** đàm phán.
pub const F_STATUS: u32 = 16;
/// `VIRTIO_F_VERSION_1` (bit 32) — bắt buộc, xem `features::F_VERSION_1`.
pub const F_VERSION_1: u32 = 32;

/// Queue `rx` (host → guest) — §5.1.2.
pub const Q_RX: u16 = 0;
/// Queue `tx` (guest → host) — §5.1.2.
pub const Q_TX: u16 = 1;
/// Số queue driver **cần**: đúng hai (rx, tx).
pub const QUEUES_REQUIRED: u16 = 2;
/// Cỡ queue driver đặt (luỹ thừa 2). 16 đủ cho một frame đang bay mỗi hướng + dư, rẻ về `.bss`.
pub const QUEUE_SIZE: u16 = 16;
/// Cỡ queue tối thiểu dùng được: một descriptor mỗi hướng.
pub const MIN_QUEUE_SIZE: u16 = 2;

/// Số byte `virtio_net_config` tối thiểu driver đọc: `mac[6]` + `status` + `max_virtqueue_pairs`
/// + `mtu` = 12 (§5.1.4).
pub const NET_CONFIG_BYTES: usize = 12;
/// Cỡ `virtio_net_hdr` bản **legacy** (không `num_buffers`): 10 byte.
pub const HDR_LEGACY_BYTES: usize = 10;
/// Cỡ `virtio_net_hdr` bản **modern** (có `num_buffers`): 12 byte.
pub const HDR_MODERN_BYTES: usize = 12;
/// Cỡ frame Ethernet lớn nhất driver nhận: 14 (header) + 1500 (MTU) = 1514.
pub const MAX_FRAME_BYTES: usize = 1514;
/// Cỡ đệm `rx` một frame = header modern + frame lớn nhất.
pub const RX_BUF_BYTES: usize = HDR_MODERN_BYTES + MAX_FRAME_BYTES;
/// Cỡ đệm `tx` một frame = header modern + frame lớn nhất.
pub const TX_BUF_BYTES: usize = HDR_MODERN_BYTES + MAX_FRAME_BYTES;

/// `(vendor, device)` là virtio-net mà KT-245 chấp nhận (`0x1000`/`0x1041`).
pub const fn is_virtio_net(vendor: u16, device: u16) -> bool {
    vendor == crate::pci::VENDOR_VIRTIO
        && (device == VIRTIO_NET_MODERN || device == VIRTIO_NET_MODERN_ONLY)
}

/// Cỡ `virtio_net_hdr` theo chế độ: 10 byte (legacy) hoặc 12 byte (modern).
pub const fn hdr_bytes(modern: bool) -> usize {
    if modern {
        HDR_MODERN_BYTES
    } else {
        HDR_LEGACY_BYTES
    }
}

/// `virtio_net_config` mà driver đọc (chỉ bốn trường nó dùng — không đoán phần còn lại).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetConfig {
    /// `mac[6]` — đọc **từ thiết bị**, không hardcode.
    pub mac: [u8; 6],
    /// `status` (bit 0 = link up) — chỉ có nghĩa khi đàm phán `STATUS`.
    pub status: u16,
    /// `max_virtqueue_pairs` — driver dùng **một** cặp nên chỉ đọc để làm bằng chứng.
    pub max_virtqueue_pairs: u16,
    /// `mtu` — 0 nghĩa là thiết bị không báo (dùng 1500 của Ethernet).
    pub mtu: u16,
}

impl NetConfig {
    /// `true` ⇔ MAC đọc được **không** phải toàn 0 và không phải broadcast — hai giá trị mà một
    /// thiết bị thật không bao giờ phát. Dùng để fail-closed thay vì in "MAC" rỗng.
    pub const fn mac_hop_le(&self) -> bool {
        let mut any = false;
        let mut all_ff = true;
        let mut index = 0;
        while index < 6 {
            if self.mac[index] != 0 {
                any = true;
            }
            if self.mac[index] != 0xff {
                all_ff = false;
            }
            index += 1;
        }
        any && !all_ff
    }

    /// MTU hiệu dụng: `mtu` thiết bị báo, hoặc 1500 khi thiết bị báo 0.
    pub const fn effective_mtu(&self) -> u16 {
        if self.mtu == 0 {
            1500
        } else {
            self.mtu
        }
    }
}

/// Giải mã `virtio_net_config`. `None` nếu ngắn hơn [`NET_CONFIG_BYTES`] — **không** đoán byte
/// thiếu (fail-closed).
pub fn decode_config(raw: &[u8]) -> Option<NetConfig> {
    if raw.len() < NET_CONFIG_BYTES {
        return None;
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&raw[0..6]);
    Some(NetConfig {
        mac,
        status: u16::from_le_bytes([raw[6], raw[7]]),
        max_virtqueue_pairs: u16::from_le_bytes([raw[8], raw[9]]),
        mtu: u16::from_le_bytes([raw[10], raw[11]]),
    })
}

/// Bộ feature driver **chọn** từ `device_features`: luôn `VIRTIO_F_VERSION_1` (bit 32) + `MAC`
/// (bit 5) **nếu** thiết bị có.
///
/// **Không** bật `MRG_RXBUF`/`STATUS` kể cả khi thiết bị báo có (xem đầu tệp). Hàm là **một** chỗ
/// quyết định để "driver có bit X" và "driver dùng đường của X" không lệch nhau (D2-110B).
pub const fn negotiated_features(device_features: u64) -> u64 {
    let mut features = 1u64 << F_VERSION_1;
    if device_features & (1u64 << F_MAC) != 0 {
        features |= 1u64 << F_MAC;
    }
    features
}

/// `true` ⇔ `driver_features` **đã** có `VIRTIO_NET_F_MAC` (được tin `mac[6]` trong config).
pub const fn mac_negotiated(driver_features: u64) -> bool {
    driver_features & (1u64 << F_MAC) != 0
}

/// `true` ⇔ `driver_features` **đã** có `VIRTIO_NET_F_STATUS` (được tin `status` trong config).
pub const fn status_negotiated(driver_features: u64) -> bool {
    driver_features & (1u64 << F_STATUS) != 0
}

/// `true` ⇔ `driver_features` **đã** có `VIRTIO_NET_F_MRG_RXBUF` (được dùng nhiều buffer/frame).
pub const fn mrg_rxbuf_negotiated(driver_features: u64) -> bool {
    driver_features & (1u64 << F_MRG_RXBUF) != 0
}

/// `virtio_net_hdr` (§5.1.6) — 10 byte legacy, 12 byte modern (thêm `num_buffers`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetHdr {
    /// `flags` (`NEEDS_CSUM` = 1, `DATA_VALID` = 2).
    pub flags: u8,
    /// `gso_type` — driver KT-245 luôn 0 (không GSO).
    pub gso_type: u8,
    /// `hdr_len`.
    pub hdr_len: u16,
    /// `gso_size`.
    pub gso_size: u16,
    /// `csum_start`.
    pub csum_start: u16,
    /// `csum_offset`.
    pub csum_offset: u16,
    /// `num_buffers` — **chỉ** có nghĩa khi modern **và** đàm phán `MRG_RXBUF`; driver ghi 0.
    pub num_buffers: u16,
}

impl NetHdr {
    /// Header rỗng (mọi trường 0) — dạng driver KT-245 gửi cho frame không checksum offload.
    pub const fn zero() -> Self {
        Self {
            flags: 0,
            gso_type: 0,
            hdr_len: 0,
            gso_size: 0,
            csum_start: 0,
            csum_offset: 0,
            num_buffers: 0,
        }
    }

    /// Mã hoá little-endian. **10 byte đầu** luôn có nghĩa; hai byte cuối (`num_buffers`) chỉ có
    /// nghĩa khi `modern` — khi `modern == false` chúng được ghi 0 và người gọi chỉ dùng 10 byte.
    pub fn encode(&self, modern: bool) -> [u8; HDR_MODERN_BYTES] {
        let mut out = [0u8; HDR_MODERN_BYTES];
        out[0] = self.flags;
        out[1] = self.gso_type;
        out[2..4].copy_from_slice(&self.hdr_len.to_le_bytes());
        out[4..6].copy_from_slice(&self.gso_size.to_le_bytes());
        out[6..8].copy_from_slice(&self.csum_start.to_le_bytes());
        out[8..10].copy_from_slice(&self.csum_offset.to_le_bytes());
        if modern {
            out[10..12].copy_from_slice(&self.num_buffers.to_le_bytes());
        }
        out
    }

    /// Giải mã. `None` nếu `bytes` ngắn hơn cỡ header của chế độ (`10` legacy / `12` modern) —
    /// **không** đọc tràn, không đoán `num_buffers`.
    pub fn parse(bytes: &[u8], modern: bool) -> Option<Self> {
        let need = hdr_bytes(modern);
        if bytes.len() < need {
            return None;
        }
        let num_buffers = if modern {
            u16::from_le_bytes([bytes[10], bytes[11]])
        } else {
            0
        };
        Some(Self {
            flags: bytes[0],
            gso_type: bytes[1],
            hdr_len: u16::from_le_bytes([bytes[2], bytes[3]]),
            gso_size: u16::from_le_bytes([bytes[4], bytes[5]]),
            csum_start: u16::from_le_bytes([bytes[6], bytes[7]]),
            csum_offset: u16::from_le_bytes([bytes[8], bytes[9]]),
            num_buffers,
        })
    }
}

/// Lỗi của driver virtio-net — có **tên** để in evidence (D2-102), không in số trần.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetError {
    /// Chưa init / chưa `DRIVER_OK`.
    NotReady,
    /// Không thấy thiết bị virtio-net trên bus (fail-closed, **không** PANIC).
    NoDevice,
    /// Frame dài hơn [`MAX_FRAME_BYTES`] hoặc ngắn hơn header Ethernet tối thiểu.
    BadFrame,
    /// `used->len` vô lý (nhỏ hơn header, hoặc lớn hơn đệm đã đưa).
    Io,
    /// Poll `used` quá hạn (không có ngắt ở KT-245).
    Timeout,
    /// Queue đầy (chưa kịp thu hồi descriptor).
    QueueFull,
}

impl NetError {
    /// Tên để in evidence.
    pub const fn name(self) -> &'static str {
        match self {
            NetError::NotReady => "NotReady",
            NetError::NoDevice => "NoDevice",
            NetError::BadFrame => "BadFrame",
            NetError::Io => "Io",
            NetError::Timeout => "Timeout",
            NetError::QueueFull => "QueueFull",
        }
    }
}

/// Độ dài **frame** (không kể header) suy từ `used->len` của một descriptor `rx`.
///
/// `used->len` là số byte thiết bị đã ghi vào đệm device-writable = `hdr + frame`. Hàm từ chối
/// (không kẹp im lặng): `used_len` nhỏ hơn header ⇒ [`NetError::Io`]; frame vượt
/// [`MAX_FRAME_BYTES`] ⇒ [`NetError::TooLarge`]… (xem [`NetError`]).
pub const fn frame_len(used_len: u32, modern: bool) -> Result<usize, NetError> {
    let hdr = hdr_bytes(modern) as u32;
    if used_len < hdr {
        return Err(NetError::Io);
    }
    let frame = (used_len - hdr) as usize;
    if frame > MAX_FRAME_BYTES {
        return Err(NetError::Io);
    }
    Ok(frame)
}

/// `true` ⇔ `frame` là frame Ethernet **hợp lệ về độ dài** để đưa vào descriptor `tx`:
/// `14 ≤ len ≤ MAX_FRAME_BYTES` (14 = 6 đích + 6 nguồn + 2 ethertype).
pub const fn frame_ok(frame: &[u8]) -> bool {
    frame.len() >= 14 && frame.len() <= MAX_FRAME_BYTES
}
