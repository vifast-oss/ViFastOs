// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-263 — test host cho **logic thuần** của virtio-gpu 2D (không phần cứng, không `unsafe`).
//!
//! Bốn nhóm: (A) device id/feature/`virtio_gpu_config`; (B) **mã hoá byte-exact** sáu lệnh 2D và
//! giải mã `resp_display_info`; (C) **font 8×8** ASCII 32..126; (D) **vẽ** `fill_rect`/`draw_text`
//! kèm **ca âm** (rect vượt biên, `stride` sai, đệm ngắn, glyph ngoài ASCII).
//!
//! Mọi kỳ vọng byte ở đây là **offset theo spec 1.2 §5.7**, không phải "chép lại output của code":
//! một trường lệch chỗ ⇒ test đỏ trước khi lệnh sai tới QEMU.

use vifast_virtio::gpu::{
    bgra, decode_config, draw_text, fb_bytes, fill_rect, get_pixel, glyph, glyph_pixel,
    glyph_supported, is_error_response, is_ok_response, is_virtio_gpu, negotiated_features,
    pages_for, put_pixel, resource_ok, resp_error_name, text_height, text_width, CtrlHdr,
    DisplayOne, DrawError, GpuConfig, MemEntry, Rect, ResourceAttachBacking, ResourceCreate2d,
    ResourceFlush, RespDisplayInfo, SetScanout, TransferToHost2d, BYTES_PER_PIXEL, COLOR_BLACK,
    COLOR_BLUE, COLOR_GREEN, COLOR_RED, COLOR_WHITE, CTRL_HDR_BYTES, FONT_COUNT, FONT_FIRST,
    FONT_LAST, GLYPH_BYTES, GLYPH_H, GLYPH_W, GPU_CONFIG_BYTES, MAX_SCANOUTS, MEM_ENTRY_BYTES,
    Q_CONTROL, Q_CURSOR, RECT_BYTES, RESOURCE_ATTACH_BACKING_BYTES, RESOURCE_CREATE_2D_BYTES,
    RESOURCE_FLUSH_BYTES, RESP_DISPLAY_INFO_BYTES, SET_SCANOUT_BYTES, TRANSFER_TO_HOST_2D_BYTES,
    VIRTIO_GPU_CMD_GET_DISPLAY_INFO, VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING,
    VIRTIO_GPU_CMD_RESOURCE_CREATE_2D, VIRTIO_GPU_CMD_RESOURCE_FLUSH, VIRTIO_GPU_CMD_SET_SCANOUT,
    VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D, VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
    VIRTIO_GPU_F_CONTEXT_INIT, VIRTIO_GPU_F_EDID, VIRTIO_GPU_F_RESOURCE_BLOB,
    VIRTIO_GPU_F_RESOURCE_UUID, VIRTIO_GPU_F_VIRGL, VIRTIO_GPU_MODERN,
    VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER, VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID,
    VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID, VIRTIO_GPU_RESP_ERR_OUT_OF_MEMORY,
    VIRTIO_GPU_RESP_ERR_UNSPEC, VIRTIO_GPU_RESP_OK_DISPLAY_INFO, VIRTIO_GPU_RESP_OK_NODATA,
    VIRTIO_GPU_TRANSITIONAL,
};
use vifast_virtio::pci::{VirtioKind, VENDOR_VIRTIO};

// ---------------------------------------------------------------- A) device id / feature / config

#[test]
fn device_id_va_kind_gpu() {
    assert_eq!(VIRTIO_GPU_MODERN, 0x1050);
    assert_eq!(VIRTIO_GPU_TRANSITIONAL, 0x1010);
    assert!(is_virtio_gpu(VENDOR_VIRTIO, 0x1050));
    assert!(is_virtio_gpu(VENDOR_VIRTIO, 0x1010));
    // Ca âm: vendor lạ, device của thiết bị khác, device rác.
    assert!(!is_virtio_gpu(0x1234, 0x1050));
    assert!(!is_virtio_gpu(VENDOR_VIRTIO, 0x1000));
    assert!(!is_virtio_gpu(VENDOR_VIRTIO, 0x1012));
    assert!(!is_virtio_gpu(VENDOR_VIRTIO, 0x0000));
    assert_eq!(VirtioKind::Gpu.name(), "gpu");
    assert!(VirtioKind::Gpu.matches(VENDOR_VIRTIO, 0x1050));
    assert!(VirtioKind::Gpu.matches(VENDOR_VIRTIO, 0x1010));
    assert!(!VirtioKind::Gpu.matches(VENDOR_VIRTIO, 0x1041));
    // Không loại nào khác nhận id của gpu (một id không thuộc hai driver).
    for kind in [
        VirtioKind::Blk,
        VirtioKind::Console,
        VirtioKind::Net,
        VirtioKind::Input,
    ] {
        assert!(!kind.matches(VENDOR_VIRTIO, 0x1050), "{kind:?}");
        assert!(!kind.matches(VENDOR_VIRTIO, 0x1010), "{kind:?}");
    }
}

#[test]
fn feature_chi_version_1() {
    // Thiết bị báo **mọi** feature GPU ⇒ driver vẫn chỉ chọn VERSION_1 (bit 32).
    let all = (1u64 << VIRTIO_GPU_F_VIRGL)
        | (1u64 << VIRTIO_GPU_F_EDID)
        | (1u64 << VIRTIO_GPU_F_RESOURCE_UUID)
        | (1u64 << VIRTIO_GPU_F_RESOURCE_BLOB)
        | (1u64 << VIRTIO_GPU_F_CONTEXT_INIT)
        | (1u64 << 32);
    assert_eq!(negotiated_features(all), 1u64 << 32);
    assert_eq!(negotiated_features(0), 1u64 << 32);
    // Bit 0 (VIRGL) **không** được bật dù thiết bị có.
    assert_eq!(negotiated_features(all) & 1, 0);
}

#[test]
fn gpu_config_giai_ma() {
    let mut raw = [0u8; GPU_CONFIG_BYTES];
    raw[0..4].copy_from_slice(&0x0000_0003u32.to_le_bytes());
    raw[4..8].copy_from_slice(&0x0000_0001u32.to_le_bytes());
    raw[8..12].copy_from_slice(&1u32.to_le_bytes());
    raw[12..16].copy_from_slice(&0u32.to_le_bytes());
    let config = decode_config(&raw).expect("16 byte phải giải mã được");
    assert_eq!(
        config,
        GpuConfig {
            events_read: 3,
            events_clear: 1,
            num_scanouts: 1,
            num_capsets: 0,
        }
    );
    // Ca âm: thiếu một byte ⇒ `None`, không đoán.
    assert_eq!(decode_config(&raw[..GPU_CONFIG_BYTES - 1]), None);
    assert_eq!(decode_config(&[]), None);
}

// ---------------------------------------------------------------- B) mã hoá lệnh byte-exact

#[test]
fn ctrl_hdr_encode_byte_exact() {
    let hdr = CtrlHdr {
        type_: 0x0105,
        flags: 1,
        fence_id: 0x1122_3344_5566_7788,
        ctx_id: 2,
    };
    let bytes = hdr.encode();
    assert_eq!(bytes.len(), CTRL_HDR_BYTES);
    assert_eq!(bytes.len(), 24);
    assert_eq!(&bytes[0..4], &[0x05, 0x01, 0x00, 0x00], "type @0");
    assert_eq!(&bytes[4..8], &[0x01, 0x00, 0x00, 0x00], "flags @4");
    assert_eq!(
        &bytes[8..16],
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
        "fence_id @8"
    );
    assert_eq!(&bytes[16..20], &[0x02, 0x00, 0x00, 0x00], "ctx_id @16");
    assert_eq!(&bytes[20..24], &[0, 0, 0, 0], "padding @20");
}

#[test]
fn ctrl_hdr_parse_roundtrip_va_ngan() {
    let hdr = CtrlHdr {
        type_: VIRTIO_GPU_RESP_OK_DISPLAY_INFO,
        flags: 0,
        fence_id: 0xdead_beef,
        ctx_id: 7,
    };
    assert_eq!(CtrlHdr::parse(&hdr.encode()), Some(hdr));
    assert_eq!(CtrlHdr::parse(&[0u8; CTRL_HDR_BYTES - 1]), None);
    assert_eq!(CtrlHdr::parse(&[]), None);
    assert_eq!(CtrlHdr::new(0x0100).type_, 0x0100);
    assert_eq!(CtrlHdr::new(0x0100).flags, 0);
    assert_eq!(CtrlHdr::new(0x0100).fence_id, 0);
    assert_eq!(CtrlHdr::new(0x0100).ctx_id, 0);
}

#[test]
fn rect_encode_byte_exact() {
    let rect = Rect::new(3, 5, 640, 480);
    let bytes = rect.encode();
    assert_eq!(bytes.len(), RECT_BYTES);
    assert_eq!(&bytes[0..4], &[0x03, 0x00, 0x00, 0x00], "x @0");
    assert_eq!(&bytes[4..8], &[0x05, 0x00, 0x00, 0x00], "y @4");
    assert_eq!(&bytes[8..12], &[0x80, 0x02, 0x00, 0x00], "width @8");
    assert_eq!(&bytes[12..16], &[0xe0, 0x01, 0x00, 0x00], "height @12");
    assert_eq!(Rect::parse(&bytes), Some(rect));
    assert_eq!(Rect::parse(&bytes[..RECT_BYTES - 1]), None);
}

#[test]
fn rect_tien_ich_bien() {
    let full = Rect::full(640, 480);
    assert_eq!(full, Rect::new(0, 0, 640, 480));
    assert_eq!(full.right(), 640);
    assert_eq!(full.bottom(), 480);
    assert!(!full.is_empty());
    assert!(Rect::new(0, 0, 0, 10).is_empty());
    assert!(Rect::new(0, 0, 10, 0).is_empty());
    // Bão hoà: không tràn `u32` khi cộng.
    assert_eq!(Rect::new(u32::MAX, 0, 10, 10).right(), u32::MAX);
    assert_eq!(Rect::new(0, u32::MAX, 10, 10).bottom(), u32::MAX);
}

#[test]
fn resource_create_2d_encode_byte_exact() {
    let cmd = ResourceCreate2d::new(1, VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, 640, 480);
    let bytes = cmd.encode();
    assert_eq!(bytes.len(), RESOURCE_CREATE_2D_BYTES);
    assert_eq!(bytes.len(), 40);
    assert_eq!(
        &bytes[0..4],
        &VIRTIO_GPU_CMD_RESOURCE_CREATE_2D.to_le_bytes(),
        "type @0"
    );
    assert_eq!(&bytes[4..24], &[0u8; 20], "flags/fence/ctx/padding = 0");
    assert_eq!(&bytes[24..28], &[0x01, 0x00, 0x00, 0x00], "resource_id @24");
    assert_eq!(&bytes[28..32], &[0x01, 0x00, 0x00, 0x00], "format @28");
    assert_eq!(&bytes[32..36], &[0x80, 0x02, 0x00, 0x00], "width @32");
    assert_eq!(&bytes[36..40], &[0xe0, 0x01, 0x00, 0x00], "height @36");
    assert_eq!(ResourceCreate2d::parse(&bytes), Some(cmd));
    assert_eq!(ResourceCreate2d::parse(&bytes[..39]), None);
}

#[test]
fn set_scanout_encode_byte_exact() {
    let cmd = SetScanout::new(0, 1, Rect::full(640, 480));
    let bytes = cmd.encode();
    assert_eq!(bytes.len(), SET_SCANOUT_BYTES);
    assert_eq!(bytes.len(), 48);
    assert_eq!(&bytes[0..4], &VIRTIO_GPU_CMD_SET_SCANOUT.to_le_bytes());
    assert_eq!(&bytes[24..40], &Rect::full(640, 480).encode(), "rect @24");
    assert_eq!(&bytes[40..44], &[0x00, 0x00, 0x00, 0x00], "scanout_id @40");
    assert_eq!(&bytes[44..48], &[0x01, 0x00, 0x00, 0x00], "resource_id @44");
    assert_eq!(SetScanout::parse(&bytes), Some(cmd));
    assert_eq!(SetScanout::parse(&bytes[..47]), None);
    // `resource_id = 0` là **tắt** scanout — vẫn mã hoá được, không bị coi là lỗi.
    let off = SetScanout::new(0, 0, Rect::full(640, 480));
    assert_eq!(SetScanout::parse(&off.encode()), Some(off));
}

#[test]
fn transfer_to_host_2d_encode_byte_exact() {
    let cmd = TransferToHost2d::new(1, Rect::full(640, 480), 0);
    let bytes = cmd.encode();
    assert_eq!(bytes.len(), TRANSFER_TO_HOST_2D_BYTES);
    assert_eq!(bytes.len(), 56);
    assert_eq!(
        &bytes[0..4],
        &VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D.to_le_bytes()
    );
    assert_eq!(&bytes[24..40], &Rect::full(640, 480).encode(), "rect @24");
    assert_eq!(&bytes[40..48], &[0u8; 8], "offset @40");
    assert_eq!(&bytes[48..52], &[0x01, 0x00, 0x00, 0x00], "resource_id @48");
    assert_eq!(&bytes[52..56], &[0u8; 4], "padding @52");
    assert_eq!(TransferToHost2d::parse(&bytes), Some(cmd));
    assert_eq!(TransferToHost2d::parse(&bytes[..55]), None);
    // Offset khác 0 phải nằm đúng 8 byte @40.
    let shifted = TransferToHost2d::new(2, Rect::new(1, 2, 3, 4), 0x0102_0304_0506_0708);
    let raw = shifted.encode();
    assert_eq!(
        &raw[40..48],
        &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    assert_eq!(TransferToHost2d::parse(&raw), Some(shifted));
}

#[test]
fn resource_flush_encode_byte_exact() {
    let cmd = ResourceFlush::new(1, Rect::full(640, 480));
    let bytes = cmd.encode();
    assert_eq!(bytes.len(), RESOURCE_FLUSH_BYTES);
    assert_eq!(bytes.len(), 48);
    assert_eq!(&bytes[0..4], &VIRTIO_GPU_CMD_RESOURCE_FLUSH.to_le_bytes());
    assert_eq!(&bytes[24..40], &Rect::full(640, 480).encode(), "rect @24");
    assert_eq!(&bytes[40..44], &[0x01, 0x00, 0x00, 0x00], "resource_id @40");
    assert_eq!(&bytes[44..48], &[0u8; 4], "padding @44");
    assert_eq!(ResourceFlush::parse(&bytes), Some(cmd));
    assert_eq!(ResourceFlush::parse(&bytes[..47]), None);
}

#[test]
fn attach_backing_va_mem_entry_byte_exact() {
    let cmd = ResourceAttachBacking::new(1, 19);
    let bytes = cmd.encode();
    assert_eq!(bytes.len(), RESOURCE_ATTACH_BACKING_BYTES);
    assert_eq!(bytes.len(), 32);
    assert_eq!(
        &bytes[0..4],
        &VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING.to_le_bytes()
    );
    assert_eq!(&bytes[24..28], &[0x01, 0x00, 0x00, 0x00], "resource_id @24");
    assert_eq!(&bytes[28..32], &[0x13, 0x00, 0x00, 0x00], "nr_entries @28");
    assert_eq!(ResourceAttachBacking::parse(&bytes), Some(cmd));
    assert_eq!(ResourceAttachBacking::parse(&bytes[..31]), None);

    let entry = MemEntry::new(0x8100_0000, 0x0001_0000);
    let raw = entry.encode();
    assert_eq!(raw.len(), MEM_ENTRY_BYTES);
    assert_eq!(raw.len(), 16);
    assert_eq!(&raw[0..8], &[0x00, 0x00, 0x00, 0x81, 0, 0, 0, 0], "addr @0");
    assert_eq!(&raw[8..12], &[0x00, 0x00, 0x01, 0x00], "length @8");
    assert_eq!(&raw[12..16], &[0u8; 4], "padding @12");
    assert_eq!(MemEntry::parse(&raw), Some(entry));
    assert_eq!(MemEntry::parse(&raw[..15]), None);
}

#[test]
fn resp_display_info_giai_ma_408_byte() {
    let mut info = RespDisplayInfo {
        hdr: CtrlHdr::new(VIRTIO_GPU_RESP_OK_DISPLAY_INFO),
        ..Default::default()
    };
    info.pmodes[0] = DisplayOne {
        r: Rect::full(640, 480),
        enabled: 1,
        flags: 0,
    };
    info.pmodes[1] = DisplayOne {
        r: Rect::full(800, 600),
        enabled: 0,
        flags: 0,
    };
    let raw = info.encode();
    assert_eq!(raw.len(), RESP_DISPLAY_INFO_BYTES);
    assert_eq!(raw.len(), 408);
    // `pmodes[0].r.width` ở 24 + 8 = 32; `enabled` ở 24 + 16 = 40.
    assert_eq!(&raw[32..36], &[0x80, 0x02, 0x00, 0x00], "pmodes[0].r.width");
    assert_eq!(&raw[40..44], &[0x01, 0x00, 0x00, 0x00], "pmodes[0].enabled");
    // `pmodes[1]` bắt đầu ở 24 + 24 = 48.
    assert_eq!(&raw[56..60], &[0x20, 0x03, 0x00, 0x00], "pmodes[1].r.width");
    assert_eq!(&raw[64..68], &[0u8; 4], "pmodes[1].enabled");
    let parsed = RespDisplayInfo::parse(&raw).expect("408 byte hợp lệ");
    assert_eq!(parsed, info);
    assert_eq!(parsed.enabled_count(), 1);
    assert_eq!(parsed.first_enabled(), Some((0, Rect::full(640, 480))));
}

#[test]
fn resp_display_info_tu_choi_type_sai_va_ngan() {
    let info = RespDisplayInfo {
        hdr: CtrlHdr::new(VIRTIO_GPU_RESP_OK_DISPLAY_INFO),
        ..Default::default()
    };
    let good = info.encode();
    // Ca âm 1: response **lỗi** không được đọc như bảng mode.
    let mut bad = good;
    bad[0..4].copy_from_slice(&VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER.to_le_bytes());
    assert_eq!(RespDisplayInfo::parse(&bad), None);
    // Ca âm 2: thiếu một byte.
    assert_eq!(
        RespDisplayInfo::parse(&good[..RESP_DISPLAY_INFO_BYTES - 1]),
        None
    );
    // Ca âm 3: rỗng.
    assert_eq!(RespDisplayInfo::parse(&[]), None);
    // Ca âm 4: `OK_NODATA` (24 byte) không phải bảng mode.
    let mut nodata = [0u8; RESP_DISPLAY_INFO_BYTES];
    nodata[0..4].copy_from_slice(&VIRTIO_GPU_RESP_OK_NODATA.to_le_bytes());
    assert_eq!(RespDisplayInfo::parse(&nodata), None);
}

#[test]
fn display_info_chua_bat_scanout() {
    let mut info = RespDisplayInfo {
        hdr: CtrlHdr::new(VIRTIO_GPU_RESP_OK_DISPLAY_INFO),
        ..Default::default()
    };
    assert_eq!(info.enabled_count(), 0);
    assert_eq!(info.first_enabled(), None);
    // Bật scanout **cuối** (15) — `first_enabled` phải trả đúng chỉ số, không chỉ 0.
    info.pmodes[MAX_SCANOUTS - 1] = DisplayOne {
        r: Rect::full(320, 200),
        enabled: 1,
        flags: 0,
    };
    assert_eq!(info.enabled_count(), 1);
    assert_eq!(
        info.first_enabled(),
        Some((MAX_SCANOUTS - 1, Rect::full(320, 200)))
    );
}

#[test]
fn ma_loi_response_co_ten() {
    assert!(is_ok_response(VIRTIO_GPU_RESP_OK_NODATA));
    assert!(is_ok_response(VIRTIO_GPU_RESP_OK_DISPLAY_INFO));
    assert!(!is_ok_response(VIRTIO_GPU_RESP_ERR_UNSPEC));
    assert!(is_error_response(VIRTIO_GPU_RESP_ERR_UNSPEC));
    assert!(is_error_response(VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER));
    assert!(!is_error_response(VIRTIO_GPU_RESP_OK_NODATA));
    assert_eq!(resp_error_name(VIRTIO_GPU_RESP_ERR_UNSPEC), "Unspec");
    assert_eq!(
        resp_error_name(VIRTIO_GPU_RESP_ERR_OUT_OF_MEMORY),
        "OutOfMemory"
    );
    assert_eq!(
        resp_error_name(VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID),
        "InvalidScanoutId"
    );
    assert_eq!(
        resp_error_name(VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID),
        "InvalidResourceId"
    );
    assert_eq!(resp_error_name(0x9999), "Unknown");
}

#[test]
fn hang_so_queue_va_loai_lenh() {
    assert_eq!(Q_CONTROL, 0);
    assert_eq!(Q_CURSOR, 1);
    assert_eq!(VIRTIO_GPU_CMD_GET_DISPLAY_INFO, 0x0100);
    assert_eq!(VIRTIO_GPU_CMD_RESOURCE_CREATE_2D, 0x0101);
    assert_eq!(VIRTIO_GPU_CMD_SET_SCANOUT, 0x0103);
    assert_eq!(VIRTIO_GPU_CMD_RESOURCE_FLUSH, 0x0104);
    assert_eq!(VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D, 0x0105);
    assert_eq!(VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING, 0x0106);
    assert_eq!(VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM, 1);
    assert_eq!(VIRTIO_GPU_RESP_OK_NODATA, 0x1100);
    assert_eq!(VIRTIO_GPU_RESP_OK_DISPLAY_INFO, 0x1101);
}

// ---------------------------------------------------------------- C) font 8×8

#[test]
fn font_phu_dung_ascii_in_duoc() {
    assert_eq!(FONT_FIRST, 32);
    assert_eq!(FONT_LAST, 126);
    assert_eq!(FONT_COUNT, 95);
    assert_eq!(GLYPH_BYTES, 8);
    assert_eq!(GLYPH_W, 8);
    assert_eq!(GLYPH_H, 8);
    for code in FONT_FIRST..=FONT_LAST {
        assert!(glyph_supported(code), "0x{code:02x} phải có glyph");
    }
    // Ca âm: ngoài khoảng in được.
    assert!(!glyph_supported(0));
    assert!(!glyph_supported(31));
    assert!(!glyph_supported(127));
    assert!(!glyph_supported(255));
}

#[test]
fn font_glyph_byte_exact() {
    // Dấu cách: **toàn 0** (không phải "không có glyph").
    assert_eq!(glyph(b' '), [0u8; 8]);
    // 'A' — đọc từ bảng public domain, kiểm từng byte (bit 0 = pixel trái).
    assert_eq!(
        glyph(b'A'),
        [0x0c, 0x1e, 0x33, 0x33, 0x3f, 0x33, 0x33, 0x00]
    );
    // 'V' và 'i' — hai glyph của chữ "ViFast" trên màn hình.
    assert_eq!(
        glyph(b'V'),
        [0x33, 0x33, 0x33, 0x33, 0x33, 0x1e, 0x0c, 0x00]
    );
    assert_eq!(
        glyph(b'i'),
        [0x0c, 0x00, 0x0e, 0x0c, 0x0c, 0x0c, 0x1e, 0x00]
    );
    // Glyph ngoài ASCII ⇒ rỗng, **không** panic, **không** đọc ngoài bảng.
    assert_eq!(glyph(0), [0u8; 8]);
    assert_eq!(glyph(127), [0u8; 8]);
    assert_eq!(glyph(255), [0u8; 8]);
}

#[test]
fn font_moi_glyph_8_byte_va_khac_nhau() {
    // Mọi glyph đúng 8 byte (kiểu mảng bảo đảm) và **chỉ** dấu cách rỗng — một glyph rỗng lẫn
    // trong bảng nghĩa là chữ trên màn hình mất ký tự mà lệnh vẫn "thành công".
    for code in FONT_FIRST..=FONT_LAST {
        let bits = glyph(code);
        let empty = bits.iter().all(|byte| *byte == 0);
        assert_eq!(
            empty,
            code == b' ',
            "glyph 0x{code:02x} ({:?}) rỗng ngoài dự kiến",
            code as char
        );
    }
    // Chữ "ViFast" phải có **sáu** glyph khác nhau (không phải cùng một ô).
    let text = b"ViFast";
    for i in 0..text.len() {
        for j in (i + 1)..text.len() {
            if text[i] != text[j] {
                assert_ne!(glyph(text[i]), glyph(text[j]), "{} vs {}", text[i], text[j]);
            }
        }
    }
}

#[test]
fn font_pixel_doc_dung_chieu_bit() {
    // 'A' hàng 0 = 0x0c = 0b0000_1100 ⇒ bit 2 và bit 3 bật (cột 2, 3 tính từ trái).
    let a = glyph(b'A');
    assert!(!glyph_pixel(&a, 0, 0));
    assert!(!glyph_pixel(&a, 1, 0));
    assert!(glyph_pixel(&a, 2, 0));
    assert!(glyph_pixel(&a, 3, 0));
    assert!(!glyph_pixel(&a, 4, 0));
    // Hàng 4 = 0x3f ⇒ sáu pixel trái bật.
    for col in 0..6 {
        assert!(glyph_pixel(&a, col, 4), "cột {col}");
    }
    assert!(!glyph_pixel(&a, 6, 4));
    assert!(!glyph_pixel(&a, 7, 4));
    // Ngoài biên ⇒ false, không panic.
    assert!(!glyph_pixel(&a, 8, 0));
    assert!(!glyph_pixel(&a, 0, 8));
}

// ---------------------------------------------------------------- D) vẽ

#[test]
fn bgra_thu_tu_byte_trong_bo_nho() {
    // B8G8R8A8_UNORM: byte 0 = B, 1 = G, 2 = R, 3 = A ⇒ `u32` LE = 0xAARRGGBB.
    assert_eq!(bgra(255, 0, 0), 0xffff_0000);
    assert_eq!(bgra(255, 0, 0).to_le_bytes(), [0x00, 0x00, 0xff, 0xff]);
    assert_eq!(bgra(0, 255, 0).to_le_bytes(), [0x00, 0xff, 0x00, 0xff]);
    assert_eq!(bgra(0, 0, 255).to_le_bytes(), [0xff, 0x00, 0x00, 0xff]);
    assert_eq!(COLOR_RED, bgra(255, 0, 0));
    assert_eq!(COLOR_WHITE, 0xffff_ffff);
    assert_eq!(COLOR_BLACK, 0xff00_0000);
    assert_eq!(COLOR_GREEN, bgra(0, 255, 0));
    assert_eq!(COLOR_BLUE, bgra(0, 0, 255));
    assert_eq!(BYTES_PER_PIXEL, 4);
}

#[test]
fn fill_rect_to_dung_vung() {
    let (w, h) = (16usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    fill_rect(&mut fb, w, h, stride, Rect::new(4, 2, 6, 3), COLOR_RED).expect("hợp lệ");
    for y in 0..h {
        for x in 0..w {
            let inside = (4..10).contains(&x) && (2..5).contains(&y);
            let want = if inside { COLOR_RED } else { 0 };
            assert_eq!(get_pixel(&fb, w, h, stride, x, y), Some(want), "({x},{y})");
        }
    }
    // Tô đè bằng màu khác — lần hai ghi đúng, không cộng dồn.
    fill_rect(&mut fb, w, h, stride, Rect::new(4, 2, 6, 3), COLOR_BLUE).expect("hợp lệ");
    assert_eq!(get_pixel(&fb, w, h, stride, 4, 2), Some(COLOR_BLUE));
}

#[test]
fn fill_rect_vuot_bien_bi_tu_choi() {
    let (w, h) = (16usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    // Tràn phải.
    assert_eq!(
        fill_rect(&mut fb, w, h, stride, Rect::new(10, 0, 7, 1), COLOR_RED),
        Err(DrawError::OutOfBounds)
    );
    // Tràn dưới.
    assert_eq!(
        fill_rect(&mut fb, w, h, stride, Rect::new(0, 6, 1, 3), COLOR_RED),
        Err(DrawError::OutOfBounds)
    );
    // Gốc ngoài khung.
    assert_eq!(
        fill_rect(&mut fb, w, h, stride, Rect::new(16, 0, 1, 1), COLOR_RED),
        Err(DrawError::OutOfBounds)
    );
    // Tràn `u32` khi cộng `x + width` ⇒ vẫn là OutOfBounds, không phải panic.
    assert_eq!(
        fill_rect(
            &mut fb,
            w,
            h,
            stride,
            Rect::new(u32::MAX, 0, 2, 1),
            COLOR_RED
        ),
        Err(DrawError::OutOfBounds)
    );
    // Không pixel nào bị ghi sau các ca bị từ chối.
    assert!(fb.iter().all(|byte| *byte == 0));
    assert_eq!(DrawError::OutOfBounds.name(), "OutOfBounds");
}

#[test]
fn fill_rect_stride_sai_va_dem_ngan() {
    let (w, h) = (16usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    // `stride` nhỏ hơn `width * 4` ⇒ mỗi hàng không đủ pixel.
    assert_eq!(
        fill_rect(&mut fb, w, h, stride - 1, Rect::new(0, 0, 1, 1), COLOR_RED),
        Err(DrawError::BadStride)
    );
    assert_eq!(DrawError::BadStride.name(), "BadStride");
    // Đệm ngắn hơn `stride * height`.
    let mut short = vec![0u8; stride * h - 1];
    assert_eq!(
        fill_rect(&mut short, w, h, stride, Rect::new(0, 0, 1, 1), COLOR_RED),
        Err(DrawError::BadBuffer)
    );
    assert_eq!(DrawError::BadBuffer.name(), "BadBuffer");
    // `width`/`height` bằng 0 ⇒ không có khung để vẽ.
    assert_eq!(
        fill_rect(&mut fb, 0, h, stride, Rect::new(0, 0, 1, 1), COLOR_RED),
        Err(DrawError::OutOfBounds)
    );
    // `stride * height` tràn `usize` ⇒ Overflow, không panic.
    assert_eq!(
        fill_rect(&mut fb, w, h, usize::MAX, Rect::new(0, 0, 1, 1), COLOR_RED),
        Err(DrawError::Overflow)
    );
    assert_eq!(DrawError::Overflow.name(), "Overflow");
}

#[test]
fn fill_rect_rong_la_khong_lam_gi() {
    let (w, h) = (8usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    // Rect rộng 0 / cao 0: hợp lệ về biên, không ghi pixel nào.
    assert_eq!(
        fill_rect(&mut fb, w, h, stride, Rect::new(2, 2, 0, 4), COLOR_RED),
        Ok(())
    );
    assert_eq!(
        fill_rect(&mut fb, w, h, stride, Rect::new(2, 2, 4, 0), COLOR_RED),
        Ok(())
    );
    assert!(fb.iter().all(|byte| *byte == 0));
}

#[test]
fn draw_text_ve_dung_glyph() {
    let (w, h) = (8usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    draw_text(&mut fb, w, h, stride, 0, 0, "A", COLOR_WHITE, COLOR_BLACK).expect("vừa khung");
    let bits = glyph(b'A');
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            let want = if glyph_pixel(&bits, col, row) {
                COLOR_WHITE
            } else {
                COLOR_BLACK
            };
            assert_eq!(
                get_pixel(&fb, w, h, stride, col, row),
                Some(want),
                "({col},{row})"
            );
        }
    }
    assert_eq!(text_width("A"), 8);
    assert_eq!(text_height(), 8);
}

#[test]
fn draw_text_nhieu_ky_tu_va_le_phai() {
    let (w, h) = (48usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    // "ViFast" = 6 ký tự = 48 pixel ⇒ vừa khít khung 48.
    assert_eq!(text_width("ViFast"), 48);
    draw_text(
        &mut fb,
        w,
        h,
        stride,
        0,
        0,
        "ViFast",
        COLOR_WHITE,
        COLOR_BLACK,
    )
    .expect("vừa khít");
    // Cột 0..8 là 'V', cột 8..16 là 'i' — kiểm chéo hai glyph liền nhau.
    let v = glyph(b'V');
    let i = glyph(b'i');
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            let want_v = if glyph_pixel(&v, col, row) {
                COLOR_WHITE
            } else {
                COLOR_BLACK
            };
            assert_eq!(get_pixel(&fb, w, h, stride, col, row), Some(want_v));
            let want_i = if glyph_pixel(&i, col, row) {
                COLOR_WHITE
            } else {
                COLOR_BLACK
            };
            assert_eq!(get_pixel(&fb, w, h, stride, 8 + col, row), Some(want_i));
        }
    }
}

#[test]
fn draw_text_vuot_bien_bi_tu_choi() {
    let (w, h) = (16usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    // Ba ký tự = 24 pixel > 16.
    assert_eq!(
        draw_text(&mut fb, w, h, stride, 0, 0, "abc", COLOR_WHITE, COLOR_BLACK),
        Err(DrawError::OutOfBounds)
    );
    // Lệch phải.
    assert_eq!(
        draw_text(&mut fb, w, h, stride, 9, 0, "a", COLOR_WHITE, COLOR_BLACK),
        Err(DrawError::OutOfBounds)
    );
    // Lệch dưới (cao 8, y = 1).
    assert_eq!(
        draw_text(&mut fb, w, h, stride, 0, 1, "a", COLOR_WHITE, COLOR_BLACK),
        Err(DrawError::OutOfBounds)
    );
    // `stride` sai.
    assert_eq!(
        draw_text(
            &mut fb,
            w,
            h,
            stride - 1,
            0,
            0,
            "a",
            COLOR_WHITE,
            COLOR_BLACK
        ),
        Err(DrawError::BadStride)
    );
    assert!(fb.iter().all(|byte| *byte == 0));
}

#[test]
fn draw_text_byte_khong_glyph_thanh_o_nen() {
    let (w, h) = (8usize, 8usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    // 0x7f không có glyph ⇒ cả ô là `bg`, **không** panic, **không** lệch cột.
    draw_text(
        &mut fb,
        w,
        h,
        stride,
        0,
        0,
        "\u{7f}",
        COLOR_WHITE,
        COLOR_BLACK,
    )
    .expect("vừa khung");
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            assert_eq!(
                get_pixel(&fb, w, h, stride, col, row),
                Some(COLOR_BLACK),
                "({col},{row})"
            );
        }
    }
    // Chuỗi rỗng: không ghi gì, vẫn Ok.
    assert_eq!(
        draw_text(&mut fb, w, h, stride, 0, 0, "", COLOR_WHITE, COLOR_BLACK),
        Ok(())
    );
}

#[test]
fn put_pixel_va_get_pixel_bien() {
    let (w, h) = (4usize, 3usize);
    let stride = w * BYTES_PER_PIXEL;
    let mut fb = vec![0u8; stride * h];
    put_pixel(&mut fb, w, h, stride, 3, 2, COLOR_GREEN).expect("trong biên");
    assert_eq!(get_pixel(&fb, w, h, stride, 3, 2), Some(COLOR_GREEN));
    assert_eq!(get_pixel(&fb, w, h, stride, 0, 0), Some(0));
    // Ngoài biên ⇒ lỗi / None, không ghi tràn.
    assert_eq!(
        put_pixel(&mut fb, w, h, stride, 4, 0, COLOR_RED),
        Err(DrawError::OutOfBounds)
    );
    assert_eq!(
        put_pixel(&mut fb, w, h, stride, 0, 3, COLOR_RED),
        Err(DrawError::OutOfBounds)
    );
    assert_eq!(get_pixel(&fb, w, h, stride, 4, 0), None);
    assert_eq!(get_pixel(&fb, w, h, stride, 0, 3), None);
    // `stride` nhỏ hơn `width * 4` ⇒ `get_pixel` trả None thay vì đọc sai hàng.
    assert_eq!(get_pixel(&fb, w, h, stride - 1, 0, 0), None);
}

#[test]
fn kich_thuoc_framebuffer_va_so_trang() {
    // 640×480×4 = 1.228.800 byte = 300 trang 4 KiB (đúng số frame driver cấp).
    assert_eq!(fb_bytes(640, 480), Some(1_228_800));
    assert_eq!(pages_for(1_228_800), 300);
    // 320×200×4 = 256.000 byte = 63 trang (làm tròn lên).
    assert_eq!(fb_bytes(320, 200), Some(256_000));
    assert_eq!(pages_for(256_000), 63);
    // Trang lẻ: 1 byte vẫn cần 1 trang; 4096 byte đúng 1 trang.
    assert_eq!(pages_for(1), 1);
    assert_eq!(pages_for(4096), 1);
    assert_eq!(pages_for(4097), 2);
    // Tràn `usize` ⇒ None, không panic.
    assert_eq!(fb_bytes(u32::MAX, u32::MAX), None);
}

#[test]
fn resource_ok_chan_cau_hinh_vo_ly() {
    assert!(resource_ok(640, 480));
    assert!(resource_ok(1, 1));
    assert!(resource_ok(4096, 4096));
    assert!(!resource_ok(0, 480));
    assert!(!resource_ok(640, 0));
    assert!(!resource_ok(4097, 480));
    assert!(!resource_ok(640, 4097));
}
