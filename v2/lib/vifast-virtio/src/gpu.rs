// SPDX-License-Identifier: MIT OR Apache-2.0
//! `virtio_gpu` — **logic thuần** của thiết bị virtio-gpu 2D: device id, feature, cấu trúc lệnh
//! (§5.7 Virtio 1.2), **font bitmap 8×8** cho ASCII 32..126, và hàm **vẽ** thuần trên framebuffer
//! — KT-263 (D1 phần 4).
//!
//! # Phạm vi
//!
//! Chỉ phần **quyết định trên byte**: mã hoá/giải mã `virtio_gpu_ctrl_hdr` và sáu lệnh 2D mà
//! driver dùng (`GET_DISPLAY_INFO`, `RESOURCE_CREATE_2D`, `RESOURCE_ATTACH_BACKING`,
//! `SET_SCANOUT`, `TRANSFER_TO_HOST_2D`, `RESOURCE_FLUSH`), giải mã `virtio_gpu_resp_display_info`,
//! bảng font 8×8, và hai hàm vẽ `fill_rect`/`draw_text`. Không MMIO, không `unsafe`, không cấp
//! phát ⇒ chạy host test (D2-05: đỏ trước rồi mới xanh). Driver thật ở
//! `kernel/vifast-plat-riscv64/src/virtio_gpu.rs` chỉ còn phần chạm thanh ghi + ring + DMA.
//!
//! # Nguồn
//!
//! Virtio 1.2 §5.7 (GPU device): §5.7.2 virtqueue, §5.7.3 feature, §5.7.4 `virtio_gpu_config`,
//! §5.7.6 lệnh/response. Offset từng trường **theo đúng thứ tự khai báo C của spec** — mỗi
//! struct ở đây có `const` assert cỡ, nên lệch một trường là **không biên dịch được** thay vì gửi
//! lệnh sai cho thiết bị.
//!
//! # Vì sao **không** đàm phán feature nào của GPU
//!
//! `VIRTIO_GPU_F_VIRGL` (bit 0) mở đường 3D; `EDID` (1), `RESOURCE_UUID` (2), `RESOURCE_BLOB` (3),
//! `CONTEXT_INIT` (4) đều là năng lực **thêm** mà driver 2D này không dùng. Driver chỉ đàm phán
//! `VIRTIO_F_VERSION_1` (bit 32) — D2-110B: chỉ dùng ngữ nghĩa của feature **đã** đàm phán. Hằng
//! số của chúng vẫn ở đây để test/evidence đọc được bit và lần mở 3D sau không phải đoán số.
//!
//! # Vì sao font nằm ở crate **thuần**
//!
//! Bảng 95 glyph × 8 byte là chỗ dễ sai **im lặng** nhất của cả driver (lệch một bit ⇒ chữ sai
//! nhưng lệnh vẫn "thành công"). Đặt ở crate `no_std` không `unsafe` nghĩa là **mọi** ca biên
//! (glyph rỗng, glyph ngoài ASCII, vẽ sát biên) chạy được trên host trước khi chạm QEMU; kernel
//! chỉ gọi [`glyph`]/[`draw_text`].

/// `virtio-gpu` bản modern-only (device id `0x1050`) — KT-263 nhắm id này.
pub const VIRTIO_GPU_MODERN: u16 = 0x1050;
/// `virtio-gpu` bản transitional (device id `0x1010`) — cùng driver.
pub const VIRTIO_GPU_TRANSITIONAL: u16 = 0x1010;
/// Device id mà driver KT-263 tìm trên bus PCI (`0x1050`).
pub const DEVICE_ID_GPU: u16 = VIRTIO_GPU_MODERN;

/// `VIRTIO_F_VERSION_1` (bit 32) — feature **chung** bắt buộc của mọi thiết bị virtio 1.0.
///
/// Tên ở đây nhắc "gpu" vì đây là feature **duy nhất** driver GPU đàm phán; giá trị **phải** bằng
/// [`crate::features::F_VERSION_1`] (có `const` assert bên dưới — một nguồn, không hai số lệch).
pub const VIRTIO_GPU_F_VERSION_1: u32 = 32;
/// `VIRTIO_GPU_F_VIRGL` (bit 0) — 3D/virgl. **Không** đàm phán (xem đầu tệp).
pub const VIRTIO_GPU_F_VIRGL: u32 = 0;
/// `VIRTIO_GPU_F_EDID` (bit 1) — lệnh `GET_EDID`. **Không** đàm phán.
pub const VIRTIO_GPU_F_EDID: u32 = 1;
/// `VIRTIO_GPU_F_RESOURCE_UUID` (bit 2) — `RESOURCE_ASSIGN_UUID`. **Không** đàm phán.
pub const VIRTIO_GPU_F_RESOURCE_UUID: u32 = 2;
/// `VIRTIO_GPU_F_RESOURCE_BLOB` (bit 3) — resource blob. **Không** đàm phán.
pub const VIRTIO_GPU_F_RESOURCE_BLOB: u32 = 3;
/// `VIRTIO_GPU_F_CONTEXT_INIT` (bit 4) — context init. **Không** đàm phán.
pub const VIRTIO_GPU_F_CONTEXT_INIT: u32 = 4;

// `const` assert: hai hằng phải khớp. Nếu ai đó đổi một bên, ảnh **không biên dịch được** thay vì
// driver đàm phán nhầm trang feature (bit ≥ 32 nằm ở `device_feature_select = 1`).
const _: () = assert!(VIRTIO_GPU_F_VERSION_1 == crate::features::F_VERSION_1);

/// Queue `controlq` (driver → thiết bị, có response) — §5.7.2.
pub const Q_CONTROL: u16 = 0;
/// Queue `cursorq` (con trỏ chuột) — §5.7.2. Driver KT-263 **không** dùng (không có con trỏ).
pub const Q_CURSOR: u16 = 1;
/// Số queue driver **cần**: đúng một (`controlq`).
pub const QUEUES_REQUIRED: u16 = 1;
/// Cỡ queue driver đặt (luỹ thừa 2). 16 đủ cho một lệnh đang bay + dư, rẻ về `.bss`.
pub const QUEUE_SIZE: u16 = 16;
/// Cỡ queue tối thiểu dùng được (spec: luỹ thừa 2, ≥ 2).
pub const MIN_QUEUE_SIZE: u16 = 2;

/// Cỡ `virtio_gpu_config` (§5.7.4): bốn `__le32`.
pub const GPU_CONFIG_BYTES: usize = 16;
/// `VIRTIO_GPU_MAX_SCANOUTS` (§5.7.6.1) — số phần tử `pmodes` của `resp_display_info`.
pub const MAX_SCANOUTS: usize = 16;

/// Cỡ `virtio_gpu_ctrl_hdr` = 4 + 4 + 8 + 4 + 4 (padding).
pub const CTRL_HDR_BYTES: usize = 24;
/// Cỡ `virtio_gpu_rect` = 4 × `__le32`.
pub const RECT_BYTES: usize = 16;
/// Cỡ `virtio_gpu_resource_create_2d` = header + 4 × `__le32`.
pub const RESOURCE_CREATE_2D_BYTES: usize = CTRL_HDR_BYTES + 16;
/// Cỡ `virtio_gpu_set_scanout` = header + rect + 2 × `__le32`.
pub const SET_SCANOUT_BYTES: usize = CTRL_HDR_BYTES + RECT_BYTES + 8;
/// Cỡ `virtio_gpu_transfer_to_host_2d` = header + rect + `__le64` + `__le32` + padding.
pub const TRANSFER_TO_HOST_2D_BYTES: usize = CTRL_HDR_BYTES + RECT_BYTES + 8 + 4 + 4;
/// Cỡ `virtio_gpu_resource_flush` = header + rect + `__le32` + padding.
pub const RESOURCE_FLUSH_BYTES: usize = CTRL_HDR_BYTES + RECT_BYTES + 4 + 4;
/// Cỡ `virtio_gpu_resource_attach_backing` = header + 2 × `__le32`.
pub const RESOURCE_ATTACH_BACKING_BYTES: usize = CTRL_HDR_BYTES + 8;
/// Cỡ `virtio_gpu_mem_entry` = `__le64 addr` + `__le32 length` + `__le32 padding`.
pub const MEM_ENTRY_BYTES: usize = 16;
/// Cỡ `virtio_gpu_display_one` = rect + `enabled` + `flags`.
pub const DISPLAY_ONE_BYTES: usize = RECT_BYTES + 8;
/// Cỡ `virtio_gpu_resp_display_info` = header + [`MAX_SCANOUTS`] × [`DISPLAY_ONE_BYTES`].
pub const RESP_DISPLAY_INFO_BYTES: usize = CTRL_HDR_BYTES + MAX_SCANOUTS * DISPLAY_ONE_BYTES;

// `const` assert cỡ từng struct theo spec — lệch một trường là **không biên dịch được**.
const _: () = assert!(CTRL_HDR_BYTES == 24);
const _: () = assert!(RECT_BYTES == 16);
const _: () = assert!(RESOURCE_CREATE_2D_BYTES == 40);
const _: () = assert!(SET_SCANOUT_BYTES == 48);
const _: () = assert!(TRANSFER_TO_HOST_2D_BYTES == 56);
const _: () = assert!(RESOURCE_FLUSH_BYTES == 48);
const _: () = assert!(RESOURCE_ATTACH_BACKING_BYTES == 32);
const _: () = assert!(MEM_ENTRY_BYTES == 16);
const _: () = assert!(DISPLAY_ONE_BYTES == 24);
const _: () = assert!(RESP_DISPLAY_INFO_BYTES == 408);

// ---------------------------------------------------------------- loại lệnh (§5.7.6.1)
/// `GET_DISPLAY_INFO` — đọc mode hiện tại của từng scanout.
pub const VIRTIO_GPU_CMD_GET_DISPLAY_INFO: u32 = 0x0100;
/// `RESOURCE_CREATE_2D` — tạo resource 2D rỗng (chưa có backing).
pub const VIRTIO_GPU_CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
/// `RESOURCE_UNREF` — giải phóng resource.
pub const VIRTIO_GPU_CMD_RESOURCE_UNREF: u32 = 0x0102;
/// `SET_SCANOUT` — gắn resource vào một scanout (hoặc tắt khi `resource_id = 0`).
pub const VIRTIO_GPU_CMD_SET_SCANOUT: u32 = 0x0103;
/// `RESOURCE_FLUSH` — bảo thiết bị đẩy vùng resource ra màn hình.
pub const VIRTIO_GPU_CMD_RESOURCE_FLUSH: u32 = 0x0104;
/// `TRANSFER_TO_HOST_2D` — chép từ backing (guest RAM) sang resource của thiết bị.
pub const VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
/// `RESOURCE_ATTACH_BACKING` — gắn danh sách `virtio_gpu_mem_entry` làm backing.
pub const VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;
/// `RESOURCE_DETACH_BACKING` — gỡ backing.
pub const VIRTIO_GPU_CMD_RESOURCE_DETACH_BACKING: u32 = 0x0107;
/// `GET_CAPSET_INFO` — chỉ có nghĩa khi đàm phán virgl; driver **không** gửi.
pub const VIRTIO_GPU_CMD_GET_CAPSET_INFO: u32 = 0x0108;
/// `GET_CAPSET` — như trên.
pub const VIRTIO_GPU_CMD_GET_CAPSET: u32 = 0x0109;
/// `GET_EDID` — cần feature `EDID`; driver **không** gửi.
pub const VIRTIO_GPU_CMD_GET_EDID: u32 = 0x010a;
/// `UPDATE_CURSOR` — queue `cursorq`; driver **không** dùng.
pub const VIRTIO_GPU_CMD_UPDATE_CURSOR: u32 = 0x0300;
/// `MOVE_CURSOR` — queue `cursorq`; driver **không** dùng.
pub const VIRTIO_GPU_CMD_MOVE_CURSOR: u32 = 0x0301;

/// `RESP_OK_NODATA` — lệnh thành công, không có dữ liệu trả về.
pub const VIRTIO_GPU_RESP_OK_NODATA: u32 = 0x1100;
/// `RESP_OK_DISPLAY_INFO` — trả `virtio_gpu_resp_display_info`.
pub const VIRTIO_GPU_RESP_OK_DISPLAY_INFO: u32 = 0x1101;
/// `RESP_ERR_UNSPEC` — lỗi không xác định.
pub const VIRTIO_GPU_RESP_ERR_UNSPEC: u32 = 0x1200;
/// `RESP_ERR_OUT_OF_MEMORY` — thiết bị hết bộ nhớ.
pub const VIRTIO_GPU_RESP_ERR_OUT_OF_MEMORY: u32 = 0x1201;
/// `RESP_ERR_INVALID_SCANOUT_ID` — `scanout_id` không hợp lệ.
pub const VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID: u32 = 0x1202;
/// `RESP_ERR_INVALID_RESOURCE_ID` — `resource_id` không tồn tại.
pub const VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID: u32 = 0x1203;
/// `RESP_ERR_INVALID_CONTEXT_ID` — `ctx_id` không hợp lệ.
pub const VIRTIO_GPU_RESP_ERR_INVALID_CONTEXT_ID: u32 = 0x1204;
/// `RESP_ERR_INVALID_PARAMETER` — tham số sai (kể cả `format` lạ).
pub const VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER: u32 = 0x1205;

/// `VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM` — byte trong bộ nhớ là B,G,R,A (little-endian `0xAARRGGBB`).
pub const VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM: u32 = 1;
/// `VIRTIO_GPU_FORMAT_B8G8R8X8_UNORM` — như trên nhưng byte A không có nghĩa.
pub const VIRTIO_GPU_FORMAT_B8G8R8X8_UNORM: u32 = 2;

/// `true` ⇔ `(vendor, device)` là virtio-gpu mà KT-263 chấp nhận (`0x1050`/`0x1010`).
pub const fn is_virtio_gpu(vendor: u16, device: u16) -> bool {
    vendor == crate::pci::VENDOR_VIRTIO
        && (device == VIRTIO_GPU_MODERN || device == VIRTIO_GPU_TRANSITIONAL)
}

/// Bộ feature driver **chọn** từ `device_features`: **chỉ** `VIRTIO_F_VERSION_1`.
///
/// Hàm là **một** chỗ quyết định để "driver có bit X" và "driver dùng đường của X" không lệch
/// nhau (D2-110B). Không bao giờ bật `VIRGL`/`EDID`/`RESOURCE_UUID`/`RESOURCE_BLOB`/`CONTEXT_INIT`
/// kể cả khi thiết bị báo có.
pub const fn negotiated_features(device_features: u64) -> u64 {
    let _ = device_features;
    1u64 << VIRTIO_GPU_F_VERSION_1
}

/// `virtio_gpu_config` (§5.7.4) — bốn trường `__le32`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuConfig {
    /// `events_read` — bit sự kiện thiết bị đã báo (đọc-xoá).
    pub events_read: u32,
    /// `events_clear` — ghi bit để xoá sự kiện.
    pub events_clear: u32,
    /// `num_scanouts` — số scanout thiết bị hỗ trợ (QEMU mặc định 1).
    pub num_scanouts: u32,
    /// `num_capsets` — số capset (chỉ có nghĩa với virgl).
    pub num_capsets: u32,
}

/// Giải mã `virtio_gpu_config`. `None` nếu ngắn hơn [`GPU_CONFIG_BYTES`] — **không** đoán byte
/// thiếu (fail-closed).
pub fn decode_config(raw: &[u8]) -> Option<GpuConfig> {
    if raw.len() < GPU_CONFIG_BYTES {
        return None;
    }
    Some(GpuConfig {
        events_read: rd32(raw, 0),
        events_clear: rd32(raw, 4),
        num_scanouts: rd32(raw, 8),
        num_capsets: rd32(raw, 12),
    })
}

/// `virtio_gpu_ctrl_hdr` (§5.7.6.1) — 24 byte, mở đầu **mọi** lệnh và response.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CtrlHdr {
    /// `type` — `VIRTIO_GPU_CMD_*` (lệnh) hoặc `VIRTIO_GPU_RESP_*` (response).
    pub type_: u32,
    /// `flags` — driver KT-263 luôn 0 (không `FENCE`).
    pub flags: u32,
    /// `fence_id` — 0 khi không yêu cầu fence.
    pub fence_id: u64,
    /// `ctx_id` — 0 cho lệnh 2D (không có context 3D).
    pub ctx_id: u32,
}

impl CtrlHdr {
    /// Header cho lệnh `type_`, mọi trường còn lại 0 (dạng driver KT-263 gửi).
    pub const fn new(type_: u32) -> Self {
        Self {
            type_,
            flags: 0,
            fence_id: 0,
            ctx_id: 0,
        }
    }

    /// Mã hoá little-endian đúng 24 byte.
    pub fn encode(&self) -> [u8; CTRL_HDR_BYTES] {
        let mut out = [0u8; CTRL_HDR_BYTES];
        wr32(&mut out, 0, self.type_);
        wr32(&mut out, 4, self.flags);
        wr64(&mut out, 8, self.fence_id);
        wr32(&mut out, 16, self.ctx_id);
        // 20..24 là `padding` — spec không định nghĩa, ghi 0.
        out
    }

    /// Giải mã. `None` nếu `bytes` ngắn hơn 24 — **không** đọc tràn.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < CTRL_HDR_BYTES {
            return None;
        }
        Some(Self {
            type_: rd32(bytes, 0),
            flags: rd32(bytes, 4),
            fence_id: rd64(bytes, 8),
            ctx_id: rd32(bytes, 16),
        })
    }
}

/// `virtio_gpu_rect` (§5.7.6.1) — vùng chữ nhật trong không gian resource.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    /// Góc trái.
    pub x: u32,
    /// Góc trên.
    pub y: u32,
    /// Rộng (pixel).
    pub width: u32,
    /// Cao (pixel).
    pub height: u32,
}

impl Rect {
    /// Hình chữ nhật `(x, y, width, height)`.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Toàn bộ resource `width × height` (gốc 0,0) — dạng driver dùng cho transfer/flush.
    pub const fn full(width: u32, height: u32) -> Self {
        Self::new(0, 0, width, height)
    }

    /// `true` ⇔ không có pixel nào (rộng hoặc cao bằng 0).
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// `x + width` (bão hoà — dùng để kiểm biên, không để tính địa chỉ).
    pub const fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    /// `y + height` (bão hoà).
    pub const fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }

    /// Offset **byte** của pixel đầu `rect` trong backing có `stride` byte/hàng.
    ///
    /// Đây là giá trị `virtio_gpu_transfer_to_host_2d.offset` phải mang khi `rect` **không** bắt
    /// đầu ở gốc resource: thiết bị đọc từng hàng tại `offset + stride * h` rồi ghi vào
    /// `(rect.y + h) * stride + rect.x * 4` (QEMU `hw/display/virtio-gpu.c`). Gửi `0` cho một
    /// rect lệch gốc ⇒ thiết bị chép **hàng 0** của backing vào `rect.y` ⇒ nội dung hiện lệch
    /// đúng `(rect.x, rect.y)` pixel — lỗi KT-271 đã đo được trên PPM trước khi vá.
    pub const fn byte_offset(&self, stride: usize) -> u64 {
        (self.y as u64) * (stride as u64) + (self.x as u64) * (BYTES_PER_PIXEL as u64)
    }

    /// Mã hoá little-endian đúng 16 byte.
    pub fn encode(&self) -> [u8; RECT_BYTES] {
        let mut out = [0u8; RECT_BYTES];
        wr32(&mut out, 0, self.x);
        wr32(&mut out, 4, self.y);
        wr32(&mut out, 8, self.width);
        wr32(&mut out, 12, self.height);
        out
    }

    /// Giải mã. `None` nếu `bytes` ngắn hơn 16.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < RECT_BYTES {
            return None;
        }
        Some(Self {
            x: rd32(bytes, 0),
            y: rd32(bytes, 4),
            width: rd32(bytes, 8),
            height: rd32(bytes, 12),
        })
    }
}

/// `virtio_gpu_resource_create_2d` (§5.7.6.3) — 40 byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceCreate2d {
    /// Header `type = VIRTIO_GPU_CMD_RESOURCE_CREATE_2D`.
    pub hdr: CtrlHdr,
    /// `resource_id` — driver chọn (KT-263 dùng 1).
    pub resource_id: u32,
    /// `format` — [`VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM`].
    pub format: u32,
    /// Rộng (pixel).
    pub width: u32,
    /// Cao (pixel).
    pub height: u32,
}

impl ResourceCreate2d {
    /// Lệnh tạo resource 2D `resource_id` cỡ `width × height`, format `format`.
    pub const fn new(resource_id: u32, format: u32, width: u32, height: u32) -> Self {
        Self {
            hdr: CtrlHdr::new(VIRTIO_GPU_CMD_RESOURCE_CREATE_2D),
            resource_id,
            format,
            width,
            height,
        }
    }

    /// Mã hoá little-endian đúng 40 byte.
    pub fn encode(&self) -> [u8; RESOURCE_CREATE_2D_BYTES] {
        let mut out = [0u8; RESOURCE_CREATE_2D_BYTES];
        out[0..CTRL_HDR_BYTES].copy_from_slice(&self.hdr.encode());
        wr32(&mut out, CTRL_HDR_BYTES, self.resource_id);
        wr32(&mut out, CTRL_HDR_BYTES + 4, self.format);
        wr32(&mut out, CTRL_HDR_BYTES + 8, self.width);
        wr32(&mut out, CTRL_HDR_BYTES + 12, self.height);
        out
    }

    /// Giải mã. `None` nếu ngắn hơn 40.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < RESOURCE_CREATE_2D_BYTES {
            return None;
        }
        Some(Self {
            hdr: CtrlHdr::parse(bytes)?,
            resource_id: rd32(bytes, CTRL_HDR_BYTES),
            format: rd32(bytes, CTRL_HDR_BYTES + 4),
            width: rd32(bytes, CTRL_HDR_BYTES + 8),
            height: rd32(bytes, CTRL_HDR_BYTES + 12),
        })
    }
}

/// `virtio_gpu_set_scanout` (§5.7.6.4) — 48 byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SetScanout {
    /// Header `type = VIRTIO_GPU_CMD_SET_SCANOUT`.
    pub hdr: CtrlHdr,
    /// Vùng nguồn trong resource.
    pub r: Rect,
    /// `scanout_id` — chỉ số màn hình (QEMU `-display none`: 0).
    pub scanout_id: u32,
    /// `resource_id` — 0 nghĩa là **tắt** scanout.
    pub resource_id: u32,
}

impl SetScanout {
    /// Lệnh gắn `resource_id` vào `scanout_id`, nguồn là `r`.
    pub const fn new(scanout_id: u32, resource_id: u32, r: Rect) -> Self {
        Self {
            hdr: CtrlHdr::new(VIRTIO_GPU_CMD_SET_SCANOUT),
            r,
            scanout_id,
            resource_id,
        }
    }

    /// Mã hoá little-endian đúng 48 byte.
    pub fn encode(&self) -> [u8; SET_SCANOUT_BYTES] {
        let mut out = [0u8; SET_SCANOUT_BYTES];
        out[0..CTRL_HDR_BYTES].copy_from_slice(&self.hdr.encode());
        out[CTRL_HDR_BYTES..CTRL_HDR_BYTES + RECT_BYTES].copy_from_slice(&self.r.encode());
        wr32(&mut out, CTRL_HDR_BYTES + RECT_BYTES, self.scanout_id);
        wr32(&mut out, CTRL_HDR_BYTES + RECT_BYTES + 4, self.resource_id);
        out
    }

    /// Giải mã. `None` nếu ngắn hơn 48.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < SET_SCANOUT_BYTES {
            return None;
        }
        Some(Self {
            hdr: CtrlHdr::parse(bytes)?,
            r: Rect::parse(&bytes[CTRL_HDR_BYTES..])?,
            scanout_id: rd32(bytes, CTRL_HDR_BYTES + RECT_BYTES),
            resource_id: rd32(bytes, CTRL_HDR_BYTES + RECT_BYTES + 4),
        })
    }
}

/// `virtio_gpu_transfer_to_host_2d` (§5.7.6.5) — 56 byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransferToHost2d {
    /// Header `type = VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D`.
    pub hdr: CtrlHdr,
    /// Vùng cần chép.
    pub r: Rect,
    /// `offset` — offset byte trong backing (driver dùng 0).
    pub offset: u64,
    /// `resource_id` đích.
    pub resource_id: u32,
}

impl TransferToHost2d {
    /// Lệnh chép `r` từ backing sang resource `resource_id`, bắt đầu ở `offset`.
    pub const fn new(resource_id: u32, r: Rect, offset: u64) -> Self {
        Self {
            hdr: CtrlHdr::new(VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D),
            r,
            offset,
            resource_id,
        }
    }

    /// Mã hoá little-endian đúng 56 byte.
    pub fn encode(&self) -> [u8; TRANSFER_TO_HOST_2D_BYTES] {
        let mut out = [0u8; TRANSFER_TO_HOST_2D_BYTES];
        out[0..CTRL_HDR_BYTES].copy_from_slice(&self.hdr.encode());
        out[CTRL_HDR_BYTES..CTRL_HDR_BYTES + RECT_BYTES].copy_from_slice(&self.r.encode());
        wr64(&mut out, CTRL_HDR_BYTES + RECT_BYTES, self.offset);
        wr32(&mut out, CTRL_HDR_BYTES + RECT_BYTES + 8, self.resource_id);
        // 4 byte `padding` cuối — ghi 0.
        out
    }

    /// Giải mã. `None` nếu ngắn hơn 56.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < TRANSFER_TO_HOST_2D_BYTES {
            return None;
        }
        Some(Self {
            hdr: CtrlHdr::parse(bytes)?,
            r: Rect::parse(&bytes[CTRL_HDR_BYTES..])?,
            offset: rd64(bytes, CTRL_HDR_BYTES + RECT_BYTES),
            resource_id: rd32(bytes, CTRL_HDR_BYTES + RECT_BYTES + 8),
        })
    }
}

/// `virtio_gpu_resource_flush` (§5.7.6.6) — 48 byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceFlush {
    /// Header `type = VIRTIO_GPU_CMD_RESOURCE_FLUSH`.
    pub hdr: CtrlHdr,
    /// Vùng cần đẩy ra màn hình.
    pub r: Rect,
    /// `resource_id` nguồn.
    pub resource_id: u32,
}

impl ResourceFlush {
    /// Lệnh đẩy `r` của resource `resource_id` ra màn hình.
    pub const fn new(resource_id: u32, r: Rect) -> Self {
        Self {
            hdr: CtrlHdr::new(VIRTIO_GPU_CMD_RESOURCE_FLUSH),
            r,
            resource_id,
        }
    }

    /// Mã hoá little-endian đúng 48 byte.
    pub fn encode(&self) -> [u8; RESOURCE_FLUSH_BYTES] {
        let mut out = [0u8; RESOURCE_FLUSH_BYTES];
        out[0..CTRL_HDR_BYTES].copy_from_slice(&self.hdr.encode());
        out[CTRL_HDR_BYTES..CTRL_HDR_BYTES + RECT_BYTES].copy_from_slice(&self.r.encode());
        wr32(&mut out, CTRL_HDR_BYTES + RECT_BYTES, self.resource_id);
        // 4 byte `padding` cuối — ghi 0.
        out
    }

    /// Giải mã. `None` nếu ngắn hơn 48.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < RESOURCE_FLUSH_BYTES {
            return None;
        }
        Some(Self {
            hdr: CtrlHdr::parse(bytes)?,
            r: Rect::parse(&bytes[CTRL_HDR_BYTES..])?,
            resource_id: rd32(bytes, CTRL_HDR_BYTES + RECT_BYTES),
        })
    }
}

/// `virtio_gpu_resource_attach_backing` (§5.7.6.7) — 32 byte, theo sau là `nr_entries` ×
/// [`MemEntry`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceAttachBacking {
    /// Header `type = VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING`.
    pub hdr: CtrlHdr,
    /// `resource_id` được gắn backing.
    pub resource_id: u32,
    /// `nr_entries` — số [`MemEntry`] theo sau header.
    pub nr_entries: u32,
}

impl ResourceAttachBacking {
    /// Lệnh gắn `nr_entries` mảnh backing vào `resource_id`.
    pub const fn new(resource_id: u32, nr_entries: u32) -> Self {
        Self {
            hdr: CtrlHdr::new(VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING),
            resource_id,
            nr_entries,
        }
    }

    /// Mã hoá little-endian đúng 32 byte.
    pub fn encode(&self) -> [u8; RESOURCE_ATTACH_BACKING_BYTES] {
        let mut out = [0u8; RESOURCE_ATTACH_BACKING_BYTES];
        out[0..CTRL_HDR_BYTES].copy_from_slice(&self.hdr.encode());
        wr32(&mut out, CTRL_HDR_BYTES, self.resource_id);
        wr32(&mut out, CTRL_HDR_BYTES + 4, self.nr_entries);
        out
    }

    /// Giải mã. `None` nếu ngắn hơn 32.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < RESOURCE_ATTACH_BACKING_BYTES {
            return None;
        }
        Some(Self {
            hdr: CtrlHdr::parse(bytes)?,
            resource_id: rd32(bytes, CTRL_HDR_BYTES),
            nr_entries: rd32(bytes, CTRL_HDR_BYTES + 4),
        })
    }
}

/// `virtio_gpu_mem_entry` (§5.7.6.7) — một mảnh backing: PA + cỡ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemEntry {
    /// Địa chỉ **vật lý** của mảnh (thiết bị DMA thẳng vào đây).
    pub addr: u64,
    /// Cỡ mảnh (byte).
    pub length: u32,
}

impl MemEntry {
    /// Mảnh `[addr, addr + length)`.
    pub const fn new(addr: u64, length: u32) -> Self {
        Self { addr, length }
    }

    /// Mã hoá little-endian đúng 16 byte.
    pub fn encode(&self) -> [u8; MEM_ENTRY_BYTES] {
        let mut out = [0u8; MEM_ENTRY_BYTES];
        wr64(&mut out, 0, self.addr);
        wr32(&mut out, 8, self.length);
        // 4 byte `padding` cuối — ghi 0.
        out
    }

    /// Giải mã. `None` nếu ngắn hơn 16.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < MEM_ENTRY_BYTES {
            return None;
        }
        Some(Self {
            addr: rd64(bytes, 0),
            length: rd32(bytes, 8),
        })
    }
}

/// `virtio_gpu_display_one` (§5.7.6.1) — một phần tử `pmodes`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayOne {
    /// Vùng hiển thị.
    pub r: Rect,
    /// `enabled` — 1 ⇔ scanout này đang bật (đã `SET_SCANOUT`).
    pub enabled: u32,
    /// `flags` — spec chưa định nghĩa, driver chỉ in ra.
    pub flags: u32,
}

impl DisplayOne {
    /// Giải mã từ 24 byte.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < DISPLAY_ONE_BYTES {
            return None;
        }
        Some(Self {
            r: Rect::parse(bytes)?,
            enabled: rd32(bytes, RECT_BYTES),
            flags: rd32(bytes, RECT_BYTES + 4),
        })
    }

    /// Mã hoá little-endian đúng 24 byte (chỉ dùng cho test).
    pub fn encode(&self) -> [u8; DISPLAY_ONE_BYTES] {
        let mut out = [0u8; DISPLAY_ONE_BYTES];
        out[0..RECT_BYTES].copy_from_slice(&self.r.encode());
        wr32(&mut out, RECT_BYTES, self.enabled);
        wr32(&mut out, RECT_BYTES + 4, self.flags);
        out
    }
}

/// `virtio_gpu_resp_display_info` (§5.7.6.1) — 408 byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RespDisplayInfo {
    /// Header — `type` phải là [`VIRTIO_GPU_RESP_OK_DISPLAY_INFO`].
    pub hdr: CtrlHdr,
    /// `pmodes[VIRTIO_GPU_MAX_SCANOUTS]`.
    pub pmodes: [DisplayOne; MAX_SCANOUTS],
}

impl Default for RespDisplayInfo {
    fn default() -> Self {
        Self {
            hdr: CtrlHdr::default(),
            pmodes: [DisplayOne::default(); MAX_SCANOUTS],
        }
    }
}

impl RespDisplayInfo {
    /// Giải mã. `None` nếu ngắn hơn 408 **hoặc** `type` không phải `RESP_OK_DISPLAY_INFO` — một
    /// response lỗi không được đọc như bảng mode (fail-closed).
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < RESP_DISPLAY_INFO_BYTES {
            return None;
        }
        let hdr = CtrlHdr::parse(bytes)?;
        if hdr.type_ != VIRTIO_GPU_RESP_OK_DISPLAY_INFO {
            return None;
        }
        let mut pmodes = [DisplayOne::default(); MAX_SCANOUTS];
        let mut index = 0;
        while index < MAX_SCANOUTS {
            let at = CTRL_HDR_BYTES + index * DISPLAY_ONE_BYTES;
            pmodes[index] = DisplayOne::parse(&bytes[at..])?;
            index += 1;
        }
        Some(Self { hdr, pmodes })
    }

    /// Mã hoá little-endian đúng 408 byte (chỉ dùng cho test).
    pub fn encode(&self) -> [u8; RESP_DISPLAY_INFO_BYTES] {
        let mut out = [0u8; RESP_DISPLAY_INFO_BYTES];
        out[0..CTRL_HDR_BYTES].copy_from_slice(&self.hdr.encode());
        let mut index = 0;
        while index < MAX_SCANOUTS {
            let at = CTRL_HDR_BYTES + index * DISPLAY_ONE_BYTES;
            out[at..at + DISPLAY_ONE_BYTES].copy_from_slice(&self.pmodes[index].encode());
            index += 1;
        }
        out
    }

    /// Số scanout đang bật (`enabled != 0`).
    pub fn enabled_count(&self) -> usize {
        let mut count = 0;
        let mut index = 0;
        while index < MAX_SCANOUTS {
            if self.pmodes[index].enabled != 0 {
                count += 1;
            }
            index += 1;
        }
        count
    }

    /// Scanout **đầu tiên** đang bật: `(chỉ số, rect)`. `None` ⇔ chưa `SET_SCANOUT` lần nào.
    pub fn first_enabled(&self) -> Option<(usize, Rect)> {
        let mut index = 0;
        while index < MAX_SCANOUTS {
            if self.pmodes[index].enabled != 0 {
                return Some((index, self.pmodes[index].r));
            }
            index += 1;
        }
        None
    }
}

/// `true` ⇔ `type_` là response **thành công** (`0x1100..=0x11ff`).
pub const fn is_ok_response(type_: u32) -> bool {
    type_ >= 0x1100 && type_ <= 0x11ff
}

/// `true` ⇔ `type_` là response **lỗi** (`0x1200..=0x12ff`).
pub const fn is_error_response(type_: u32) -> bool {
    type_ >= 0x1200 && type_ <= 0x12ff
}

/// Tên response lỗi để in evidence (D2-102: không in số trần).
pub const fn resp_error_name(type_: u32) -> &'static str {
    match type_ {
        VIRTIO_GPU_RESP_ERR_UNSPEC => "Unspec",
        VIRTIO_GPU_RESP_ERR_OUT_OF_MEMORY => "OutOfMemory",
        VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID => "InvalidScanoutId",
        VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID => "InvalidResourceId",
        VIRTIO_GPU_RESP_ERR_INVALID_CONTEXT_ID => "InvalidContextId",
        VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER => "InvalidParameter",
        _ => "Unknown",
    }
}

/// Trần cạnh resource driver chấp nhận (pixel) — chặn `width * height * 4` tràn `usize` và chặn
/// cấu hình vô lý trước khi gửi lệnh cho thiết bị.
pub const MAX_RESOURCE_DIM: u32 = 4096;

/// `true` ⇔ `(width, height)` là cỡ resource hợp lệ: khác 0 và ≤ [`MAX_RESOURCE_DIM`].
pub const fn resource_ok(width: u32, height: u32) -> bool {
    width > 0 && height > 0 && width <= MAX_RESOURCE_DIM && height <= MAX_RESOURCE_DIM
}

/// Số byte framebuffer của resource `width × height` (4 byte/pixel). `None` nếu tràn `usize`.
pub const fn fb_bytes(width: u32, height: u32) -> Option<usize> {
    // `?` trên `Option` chưa ổn định trong `const fn` ⇒ dùng `match` (cùng ngữ nghĩa).
    let pixels = match (width as usize).checked_mul(height as usize) {
        Some(value) => value,
        None => return None,
    };
    pixels.checked_mul(BYTES_PER_PIXEL)
}

/// Số trang 4 KiB cần cho `bytes` byte (làm tròn lên).
pub const fn pages_for(bytes: usize) -> usize {
    bytes.div_ceil(4096)
}

// ---------------------------------------------------------------- font 8×8
/// Byte đầu tiên của bảng font (dấu cách).
pub const FONT_FIRST: u8 = 32;
/// Byte cuối cùng của bảng font (`~`).
pub const FONT_LAST: u8 = 126;
/// Số glyph trong bảng = 126 − 32 + 1.
pub const FONT_COUNT: usize = (FONT_LAST - FONT_FIRST) as usize + 1;
/// Số byte mỗi glyph (8 hàng).
pub const GLYPH_BYTES: usize = 8;
/// Rộng một glyph (pixel).
pub const GLYPH_W: usize = 8;
/// Cao một glyph (pixel).
pub const GLYPH_H: usize = 8;

// `const` assert: bảng phải phủ **đúng** ASCII in được; thiếu một glyph là chữ trên màn hình
// mất ký tự mà không ai thấy.
const _: () = assert!(FONT_COUNT == 95);

/// Bảng font 8×8 cho ASCII 32..126 — **public domain**.
///
/// Nguồn: `font8x8_basic` của Daniel Hepper (`github.com/dhepper/font8x8`), bản thân nó dựa trên
/// font VGA 8×8 public domain của Marcel Sondaar/IBM. Mỗi glyph là 8 **hàng**, mỗi hàng một byte,
/// **bit 0 = pixel trái nhất** (đúng thứ tự của bảng gốc) — [`draw_text`] đọc theo quy ước đó.
pub const FONT_8X8: [[u8; GLYPH_BYTES]; FONT_COUNT] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 0x20 ' '
    [0x18, 0x3c, 0x3c, 0x18, 0x18, 0x00, 0x18, 0x00], // 0x21 '!'
    [0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 0x22 '"'
    [0x36, 0x36, 0x7f, 0x36, 0x7f, 0x36, 0x36, 0x00], // 0x23 '#'
    [0x0c, 0x3e, 0x03, 0x1e, 0x30, 0x1f, 0x0c, 0x00], // 0x24 '$'
    [0x00, 0x63, 0x33, 0x18, 0x0c, 0x66, 0x63, 0x00], // 0x25 '%'
    [0x1c, 0x36, 0x1c, 0x6e, 0x3b, 0x33, 0x6e, 0x00], // 0x26 '&'
    [0x06, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00], // 0x27 '\''
    [0x18, 0x0c, 0x06, 0x06, 0x06, 0x0c, 0x18, 0x00], // 0x28 '('
    [0x06, 0x0c, 0x18, 0x18, 0x18, 0x0c, 0x06, 0x00], // 0x29 ')'
    [0x00, 0x66, 0x3c, 0xff, 0x3c, 0x66, 0x00, 0x00], // 0x2a '*'
    [0x00, 0x0c, 0x0c, 0x3f, 0x0c, 0x0c, 0x00, 0x00], // 0x2b '+'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c, 0x06], // 0x2c ','
    [0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0x00], // 0x2d '-'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c, 0x00], // 0x2e '.'
    [0x60, 0x30, 0x18, 0x0c, 0x06, 0x03, 0x01, 0x00], // 0x2f '/'
    [0x3e, 0x63, 0x73, 0x7b, 0x6f, 0x67, 0x3e, 0x00], // 0x30 '0'
    [0x0c, 0x0e, 0x0c, 0x0c, 0x0c, 0x0c, 0x3f, 0x00], // 0x31 '1'
    [0x1e, 0x33, 0x30, 0x1c, 0x06, 0x33, 0x3f, 0x00], // 0x32 '2'
    [0x1e, 0x33, 0x30, 0x1c, 0x30, 0x33, 0x1e, 0x00], // 0x33 '3'
    [0x38, 0x3c, 0x36, 0x33, 0x7f, 0x30, 0x78, 0x00], // 0x34 '4'
    [0x3f, 0x03, 0x1f, 0x30, 0x30, 0x33, 0x1e, 0x00], // 0x35 '5'
    [0x1c, 0x06, 0x03, 0x1f, 0x33, 0x33, 0x1e, 0x00], // 0x36 '6'
    [0x3f, 0x33, 0x30, 0x18, 0x0c, 0x0c, 0x0c, 0x00], // 0x37 '7'
    [0x1e, 0x33, 0x33, 0x1e, 0x33, 0x33, 0x1e, 0x00], // 0x38 '8'
    [0x1e, 0x33, 0x33, 0x3e, 0x30, 0x18, 0x0e, 0x00], // 0x39 '9'
    [0x00, 0x0c, 0x0c, 0x00, 0x00, 0x0c, 0x0c, 0x00], // 0x3a ':'
    [0x00, 0x0c, 0x0c, 0x00, 0x00, 0x0c, 0x0c, 0x06], // 0x3b ';'
    [0x18, 0x0c, 0x06, 0x03, 0x06, 0x0c, 0x18, 0x00], // 0x3c '<'
    [0x00, 0x00, 0x3f, 0x00, 0x00, 0x3f, 0x00, 0x00], // 0x3d '='
    [0x06, 0x0c, 0x18, 0x30, 0x18, 0x0c, 0x06, 0x00], // 0x3e '>'
    [0x1e, 0x33, 0x30, 0x18, 0x0c, 0x00, 0x0c, 0x00], // 0x3f '?'
    [0x3e, 0x63, 0x7b, 0x7b, 0x7b, 0x03, 0x1e, 0x00], // 0x40 '@'
    [0x0c, 0x1e, 0x33, 0x33, 0x3f, 0x33, 0x33, 0x00], // 0x41 'A'
    [0x3f, 0x66, 0x66, 0x3e, 0x66, 0x66, 0x3f, 0x00], // 0x42 'B'
    [0x3c, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3c, 0x00], // 0x43 'C'
    [0x1f, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1f, 0x00], // 0x44 'D'
    [0x7f, 0x46, 0x16, 0x1e, 0x16, 0x46, 0x7f, 0x00], // 0x45 'E'
    [0x7f, 0x46, 0x16, 0x1e, 0x16, 0x06, 0x0f, 0x00], // 0x46 'F'
    [0x3c, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7c, 0x00], // 0x47 'G'
    [0x33, 0x33, 0x33, 0x3f, 0x33, 0x33, 0x33, 0x00], // 0x48 'H'
    [0x1e, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x1e, 0x00], // 0x49 'I'
    [0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1e, 0x00], // 0x4a 'J'
    [0x67, 0x66, 0x36, 0x1e, 0x36, 0x66, 0x67, 0x00], // 0x4b 'K'
    [0x0f, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7f, 0x00], // 0x4c 'L'
    [0x63, 0x77, 0x7f, 0x7f, 0x6b, 0x63, 0x63, 0x00], // 0x4d 'M'
    [0x63, 0x67, 0x6f, 0x7b, 0x73, 0x63, 0x63, 0x00], // 0x4e 'N'
    [0x1c, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1c, 0x00], // 0x4f 'O'
    [0x3f, 0x66, 0x66, 0x3e, 0x06, 0x06, 0x0f, 0x00], // 0x50 'P'
    [0x1e, 0x33, 0x33, 0x33, 0x3b, 0x1e, 0x38, 0x00], // 0x51 'Q'
    [0x3f, 0x66, 0x66, 0x3e, 0x36, 0x66, 0x67, 0x00], // 0x52 'R'
    [0x1e, 0x33, 0x07, 0x0e, 0x38, 0x33, 0x1e, 0x00], // 0x53 'S'
    [0x3f, 0x2d, 0x0c, 0x0c, 0x0c, 0x0c, 0x1e, 0x00], // 0x54 'T'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3f, 0x00], // 0x55 'U'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x1e, 0x0c, 0x00], // 0x56 'V'
    [0x63, 0x63, 0x63, 0x6b, 0x7f, 0x77, 0x63, 0x00], // 0x57 'W'
    [0x63, 0x63, 0x36, 0x1c, 0x1c, 0x36, 0x63, 0x00], // 0x58 'X'
    [0x33, 0x33, 0x33, 0x1e, 0x0c, 0x0c, 0x1e, 0x00], // 0x59 'Y'
    [0x7f, 0x63, 0x31, 0x18, 0x4c, 0x66, 0x7f, 0x00], // 0x5a 'Z'
    [0x1e, 0x06, 0x06, 0x06, 0x06, 0x06, 0x1e, 0x00], // 0x5b '['
    [0x03, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x40, 0x00], // 0x5c '\\'
    [0x1e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1e, 0x00], // 0x5d ']'
    [0x08, 0x1c, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00], // 0x5e '^'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff], // 0x5f '_'
    [0x0c, 0x0c, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00], // 0x60 '`'
    [0x00, 0x00, 0x1e, 0x30, 0x3e, 0x33, 0x6e, 0x00], // 0x61 'a'
    [0x07, 0x06, 0x06, 0x3e, 0x66, 0x66, 0x3b, 0x00], // 0x62 'b'
    [0x00, 0x00, 0x1e, 0x33, 0x03, 0x33, 0x1e, 0x00], // 0x63 'c'
    [0x38, 0x30, 0x30, 0x3e, 0x33, 0x33, 0x6e, 0x00], // 0x64 'd'
    [0x00, 0x00, 0x1e, 0x33, 0x3f, 0x03, 0x1e, 0x00], // 0x65 'e'
    [0x1c, 0x36, 0x06, 0x0f, 0x06, 0x06, 0x0f, 0x00], // 0x66 'f'
    [0x00, 0x00, 0x6e, 0x33, 0x33, 0x3e, 0x30, 0x1f], // 0x67 'g'
    [0x07, 0x06, 0x36, 0x6e, 0x66, 0x66, 0x67, 0x00], // 0x68 'h'
    [0x0c, 0x00, 0x0e, 0x0c, 0x0c, 0x0c, 0x1e, 0x00], // 0x69 'i'
    [0x30, 0x00, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1e], // 0x6a 'j'
    [0x07, 0x06, 0x66, 0x36, 0x1e, 0x36, 0x67, 0x00], // 0x6b 'k'
    [0x0e, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x1e, 0x00], // 0x6c 'l'
    [0x00, 0x00, 0x33, 0x7f, 0x7f, 0x6b, 0x63, 0x00], // 0x6d 'm'
    [0x00, 0x00, 0x1f, 0x33, 0x33, 0x33, 0x33, 0x00], // 0x6e 'n'
    [0x00, 0x00, 0x1e, 0x33, 0x33, 0x33, 0x1e, 0x00], // 0x6f 'o'
    [0x00, 0x00, 0x3b, 0x66, 0x66, 0x3e, 0x06, 0x0f], // 0x70 'p'
    [0x00, 0x00, 0x6e, 0x33, 0x33, 0x3e, 0x30, 0x78], // 0x71 'q'
    [0x00, 0x00, 0x3b, 0x6e, 0x66, 0x06, 0x0f, 0x00], // 0x72 'r'
    [0x00, 0x00, 0x3e, 0x03, 0x1e, 0x30, 0x1f, 0x00], // 0x73 's'
    [0x08, 0x0c, 0x3e, 0x0c, 0x0c, 0x2c, 0x18, 0x00], // 0x74 't'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x33, 0x6e, 0x00], // 0x75 'u'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x1e, 0x0c, 0x00], // 0x76 'v'
    [0x00, 0x00, 0x63, 0x6b, 0x7f, 0x7f, 0x36, 0x00], // 0x77 'w'
    [0x00, 0x00, 0x63, 0x36, 0x1c, 0x36, 0x63, 0x00], // 0x78 'x'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x3e, 0x30, 0x1f], // 0x79 'y'
    [0x00, 0x00, 0x3f, 0x19, 0x0c, 0x26, 0x3f, 0x00], // 0x7a 'z'
    [0x38, 0x0c, 0x0c, 0x07, 0x0c, 0x0c, 0x38, 0x00], // 0x7b '{'
    [0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x18, 0x00], // 0x7c '|'
    [0x07, 0x0c, 0x0c, 0x38, 0x0c, 0x0c, 0x07, 0x00], // 0x7d '}'
    [0x6e, 0x3b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 0x7e '~'
];

/// Glyph 8×8 của `c`. Byte ngoài `32..=126` trả glyph **rỗng** (8 byte 0) — không panic, không
/// đọc ngoài bảng; người gọi kiểm [`glyph_supported`] nếu cần phân biệt "dấu cách" với "không có".
pub const fn glyph(c: u8) -> [u8; GLYPH_BYTES] {
    if c < FONT_FIRST || c > FONT_LAST {
        return [0u8; GLYPH_BYTES];
    }
    FONT_8X8[(c - FONT_FIRST) as usize]
}

/// `true` ⇔ `c` có glyph **thật** trong bảng (32..=126).
pub const fn glyph_supported(c: u8) -> bool {
    c >= FONT_FIRST && c <= FONT_LAST
}

/// `true` ⇔ pixel `(col, row)` của glyph `bits` được bật (bit 0 = trái nhất).
pub const fn glyph_pixel(bits: &[u8; GLYPH_BYTES], col: usize, row: usize) -> bool {
    if col >= GLYPH_W || row >= GLYPH_H {
        return false;
    }
    bits[row] >> col & 1 != 0
}

// ---------------------------------------------------------------- vẽ trên framebuffer
/// Số byte mỗi pixel của [`VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM`].
pub const BYTES_PER_PIXEL: usize = 4;

/// Màu `B8G8R8A8_UNORM` dạng `u32` little-endian = `0xAARRGGBB`.
///
/// Vì byte trong bộ nhớ là `B, G, R, A` nên `u32` đọc little-endian có `R` ở byte thứ ba — đúng
/// `0xAARRGGBB`. Đây là chỗ dễ đảo R/B nhất của cả driver, nên nó là **một** hàm có test.
pub const fn bgra(r: u8, g: u8, b: u8) -> u32 {
    ((0xffu32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Đỏ đặc (`(255, 0, 0)` trong PPM).
pub const COLOR_RED: u32 = bgra(255, 0, 0);
/// Trắng đặc.
pub const COLOR_WHITE: u32 = bgra(255, 255, 255);
/// Đen đặc.
pub const COLOR_BLACK: u32 = bgra(0, 0, 0);
/// Xanh lá đặc.
pub const COLOR_GREEN: u32 = bgra(0, 255, 0);
/// Xanh dương đặc.
pub const COLOR_BLUE: u32 = bgra(0, 0, 255);

/// Lỗi của phép vẽ — có **tên** để in evidence (D2-102), không in số trần.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawError {
    /// `stride < width * 4` — mỗi hàng không đủ pixel.
    BadStride,
    /// `fb.len() < stride * height` — vùng đệm ngắn hơn khai báo.
    BadBuffer,
    /// Hình chữ nhật (hoặc khối chữ) vượt ra ngoài `width × height`.
    OutOfBounds,
    /// `width * height * 4` hoặc `stride * height` tràn `usize`.
    Overflow,
}

impl DrawError {
    /// Tên để in evidence.
    pub const fn name(self) -> &'static str {
        match self {
            DrawError::BadStride => "BadStride",
            DrawError::BadBuffer => "BadBuffer",
            DrawError::OutOfBounds => "OutOfBounds",
            DrawError::Overflow => "Overflow",
        }
    }
}

/// Kiểm `(fb, w, h, stride)` và một hình chữ nhật; trả `(x, y, rộng, cao)` đã đổi sang `usize`.
///
/// **Từ chối**, không kẹp: một `rect` vượt biên mà bị kẹp im lặng sẽ vẽ ra ngoài vùng đã khai và
/// ghi đè pixel của hàng khác (đúng lỗi cần chặn trước khi chạm QEMU).
pub(crate) fn check_rect(
    len: usize,
    width: usize,
    height: usize,
    stride: usize,
    rect: Rect,
) -> Result<(usize, usize, usize, usize), DrawError> {
    if width == 0 || height == 0 {
        return Err(DrawError::OutOfBounds);
    }
    let min_stride = width
        .checked_mul(BYTES_PER_PIXEL)
        .ok_or(DrawError::Overflow)?;
    if stride < min_stride {
        return Err(DrawError::BadStride);
    }
    let need = stride.checked_mul(height).ok_or(DrawError::Overflow)?;
    if len < need {
        return Err(DrawError::BadBuffer);
    }
    let x = rect.x as usize;
    let y = rect.y as usize;
    let rw = rect.width as usize;
    let rh = rect.height as usize;
    let right = x.checked_add(rw).ok_or(DrawError::Overflow)?;
    let bottom = y.checked_add(rh).ok_or(DrawError::Overflow)?;
    if right > width || bottom > height {
        return Err(DrawError::OutOfBounds);
    }
    Ok((x, y, rw, rh))
}

/// Ghi **một** pixel `color` tại `(x, y)` — kiểm biên, không ghi tràn.
pub fn put_pixel(
    fb: &mut [u8],
    width: usize,
    height: usize,
    stride: usize,
    x: usize,
    y: usize,
    color: u32,
) -> Result<(), DrawError> {
    let rect = Rect::new(x as u32, y as u32, 1, 1);
    check_rect(fb.len(), width, height, stride, rect)?;
    let at = y * stride + x * BYTES_PER_PIXEL;
    fb[at..at + BYTES_PER_PIXEL].copy_from_slice(&color.to_le_bytes());
    Ok(())
}

/// Đọc **một** pixel `(x, y)` dạng `0xAARRGGBB`. `None` nếu ngoài biên hoặc đệm ngắn.
pub fn get_pixel(
    fb: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    x: usize,
    y: usize,
) -> Option<u32> {
    if x >= width || y >= height || stride < width * BYTES_PER_PIXEL {
        return None;
    }
    let at = y.checked_mul(stride)?.checked_add(x * BYTES_PER_PIXEL)?;
    if at + BYTES_PER_PIXEL > fb.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        fb[at],
        fb[at + 1],
        fb[at + 2],
        fb[at + 3],
    ]))
}

/// Tô đặc `rect` bằng `color` trên framebuffer `fb` (`width × height`, `stride` byte/hàng).
///
/// Từ chối (không kẹp) khi `stride` sai, đệm ngắn, hoặc `rect` vượt biên — xem [`DrawError`].
pub fn fill_rect(
    fb: &mut [u8],
    width: usize,
    height: usize,
    stride: usize,
    rect: Rect,
    color: u32,
) -> Result<(), DrawError> {
    let (x, y, rw, rh) = check_rect(fb.len(), width, height, stride, rect)?;
    let pixel = color.to_le_bytes();
    let mut row = 0;
    while row < rh {
        let base = (y + row) * stride + x * BYTES_PER_PIXEL;
        let mut col = 0;
        while col < rw {
            let at = base + col * BYTES_PER_PIXEL;
            fb[at..at + BYTES_PER_PIXEL].copy_from_slice(&pixel);
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

/// Rộng (pixel) của chuỗi `s` khi vẽ bằng font 8×8 = `8 × số byte`.
pub fn text_width(s: &str) -> usize {
    s.len() * GLYPH_W
}

/// Cao (pixel) của một dòng chữ = [`GLYPH_H`].
pub const fn text_height() -> usize {
    GLYPH_H
}

/// Vẽ chuỗi `s` tại `(x, y)`: mỗi glyph 8×8, pixel bật = `fg`, pixel tắt = `bg`.
///
/// Từ chối (không kẹp) khi khối chữ `8 × s.len()` × `8` vượt biên, `stride` sai, hoặc đệm ngắn.
/// Byte **không** có glyph (ngoài 32..=126) vẽ thành ô `bg` — không panic, không bỏ lệch cột.
///
/// **Chín tham số là chủ ý**: hình học framebuffer (`fb`, `width`, `height`, `stride`) phải do
/// người gọi truyền **tường minh** — gói vào struct sẽ giấu mất `stride ≠ width * 4` (đúng lớp
/// lỗi mà [`DrawError::BadStride`] chặn). Cùng khuôn `#[allow]` đã dùng ở
/// `kernel/vifast-plat-riscv64/src/virtio_net.rs` cho hàm dựng frame.
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    fb: &mut [u8],
    width: usize,
    height: usize,
    stride: usize,
    x: usize,
    y: usize,
    s: &str,
    fg: u32,
    bg: u32,
) -> Result<(), DrawError> {
    let count = s.len();
    let block_w = count.checked_mul(GLYPH_W).ok_or(DrawError::Overflow)?;
    let right = x.checked_add(block_w).ok_or(DrawError::Overflow)?;
    let bottom = y.checked_add(GLYPH_H).ok_or(DrawError::Overflow)?;
    if right > width || bottom > height {
        return Err(DrawError::OutOfBounds);
    }
    // Kiểm stride/đệm **một lần** bằng cả khối chữ (không kiểm lại từng pixel).
    check_rect(
        fb.len(),
        width,
        height,
        stride,
        Rect::new(x as u32, y as u32, block_w as u32, GLYPH_H as u32),
    )?;
    let fg_bytes = fg.to_le_bytes();
    let bg_bytes = bg.to_le_bytes();
    for (index, byte) in s.as_bytes().iter().enumerate() {
        let bits = glyph(*byte);
        let gx = x + index * GLYPH_W;
        let mut row = 0;
        while row < GLYPH_H {
            let base = (y + row) * stride + gx * BYTES_PER_PIXEL;
            let mut col = 0;
            while col < GLYPH_W {
                let at = base + col * BYTES_PER_PIXEL;
                let pixel = if glyph_pixel(&bits, col, row) {
                    fg_bytes
                } else {
                    bg_bytes
                };
                fb[at..at + BYTES_PER_PIXEL].copy_from_slice(&pixel);
                col += 1;
            }
            row += 1;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- đọc/ghi little-endian
fn rd32(bytes: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn rd64(bytes: &[u8], off: usize) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[off..off + 8]);
    u64::from_le_bytes(raw)
}

fn wr32(bytes: &mut [u8], off: usize, value: u32) {
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn wr64(bytes: &mut [u8], off: usize, value: u64) {
    bytes[off..off + 8].copy_from_slice(&value.to_le_bytes());
}
