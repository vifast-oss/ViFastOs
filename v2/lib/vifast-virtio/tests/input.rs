// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-255 — test host cho **logic thuần** của virtio-input (bàn phím): không phần cứng, không
//! `unsafe`, không cấp phát.
//!
//! Năm nhóm: (A) `virtio_input_event` 8 byte — encode/parse + ca âm cắt cụt; (B)
//! `virtio_input_config` — offset theo spec 1.2 §5.8.4, `size`/`subsel`, ca âm header ngắn;
//! (C) **bảng chuyển mã phím → ASCII** `key_to_ascii` + máy trạng thái Shift `KeyState`;
//! (D) feature/device id/queue; (E) bitmap `EV_BITS` + `virtio_input_devids`.
//!
//! Mỗi ca âm khẳng định **đúng** lý do từ chối (không chỉ "trả None") để log đỏ tái lập được.

use vifast_virtio::features::F_VERSION_1;
use vifast_virtio::input::{
    decode_devids, decode_name, event_len, is_virtio_input, key_bit, key_to_ascii,
    negotiated_features, InputConfigHeader, InputDevIds, InputError, InputEvent, KeyState,
    ABSINFO_BYTES, CFG_BYTES, CFG_EV_BITS, CFG_HEADER_BYTES, CFG_ID_DEVIDS, CFG_ID_NAME,
    CFG_SELECT_OFF, CFG_SIZE_OFF, CFG_SUBSEL_OFF, CFG_U_BYTES, CFG_U_OFF, DEVICE_ID_INPUT,
    DEVIDS_BYTES, EVENT_BUFS, EVENT_BYTES, EV_ABS, EV_KEY, EV_REL, EV_SYN, KEY_0, KEY_1, KEY_9,
    KEY_A, KEY_APOSTROPHE, KEY_BACKSLASH, KEY_BACKSPACE, KEY_COMMA, KEY_DOT, KEY_ENTER, KEY_EQUAL,
    KEY_ESC, KEY_F1, KEY_GRAVE, KEY_L, KEY_LEFTBRACE, KEY_LEFTSHIFT, KEY_M, KEY_MINUS, KEY_P,
    KEY_Q, KEY_RIGHTBRACE, KEY_RIGHTSHIFT, KEY_SEMICOLON, KEY_SLASH, KEY_SPACE, KEY_TAB, KEY_UP,
    KEY_Z, MIN_QUEUE_SIZE, QUEUES_REQUIRED, QUEUE_SIZE, Q_EVENT, SYN_REPORT, VIRTIO_INPUT_F_REP,
    VIRTIO_INPUT_F_VERSION_1, VIRTIO_INPUT_KEYBOARD, VIRTIO_INPUT_KEYBOARD_MODERN_ONLY,
};
use vifast_virtio::pci::{VirtioKind, VENDOR_VIRTIO};

// ---------------------------------------------------------------- (A) virtio_input_event

#[test]
fn event_encode_parse_roundtrip() {
    let event = InputEvent::new(EV_KEY, KEY_A, 1);
    let raw = event.encode();
    assert_eq!(raw.len(), EVENT_BYTES);
    assert_eq!(InputEvent::parse(&raw), Some(event));
    // Ba trường **khác nhau** để hoán vị offset là test đỏ.
    let other = InputEvent::new(EV_REL, 0x1234, 0xdead_beef);
    assert_eq!(InputEvent::parse(&other.encode()), Some(other));
}

#[test]
fn event_parse_dung_offset_little_endian() {
    // `type` u16 @0, `code` u16 @2, `value` u32 @4 — spec 1.2 §5.8.6.
    let raw = [0x01, 0x00, 0x1e, 0x00, 0x01, 0x00, 0x00, 0x00];
    let event = InputEvent::parse(&raw).expect("8 byte phải giải mã được");
    assert_eq!(event.event_type, EV_KEY);
    assert_eq!(event.code, KEY_A);
    assert_eq!(event.value, 1);
}

#[test]
fn event_parse_ngan_hon_8_byte_bi_tu_choi() {
    let raw = InputEvent::new(EV_KEY, KEY_A, 1).encode();
    for len in 0..EVENT_BYTES {
        assert_eq!(
            InputEvent::parse(&raw[..len]),
            None,
            "event {len} byte phải bị từ chối"
        );
    }
    assert!(InputEvent::parse(&raw).is_some());
}

#[test]
fn event_phan_loai_key_va_syn() {
    let down = InputEvent::new(EV_KEY, KEY_A, 1);
    let up = InputEvent::new(EV_KEY, KEY_A, 0);
    let syn = InputEvent::new(EV_SYN, SYN_REPORT, 0);
    assert!(down.is_key() && down.is_key_down() && !down.is_key_up());
    assert!(up.is_key() && up.is_key_up() && !up.is_key_down());
    assert!(syn.is_syn() && !syn.is_key());
    // Ca âm: EV_REL/EV_ABS **không** phải phím.
    assert!(!InputEvent::new(EV_REL, 0, 1).is_key());
    assert!(!InputEvent::new(EV_ABS, 0, 1).is_key());
}

#[test]
fn event_len_chi_nhan_dung_8_byte() {
    assert_eq!(event_len(EVENT_BYTES as u32), Ok(EVENT_BYTES));
    // Ca âm: thiếu/thừa byte đều là lỗi I/O — **không** kẹp im lặng.
    for bad in [0u32, 1, 7, 9, 16, 4096] {
        assert_eq!(event_len(bad), Err(InputError::Io), "used->len={bad}");
    }
}

// ---------------------------------------------------------------- (B) virtio_input_config

#[test]
fn config_header_doc_dung_offset_spec() {
    // select @0, subsel @1, size @2, reserved[5] @3..8, union @8 — spec 1.2 §5.8.4.
    let mut raw = [0u8; CFG_HEADER_BYTES];
    raw[CFG_SELECT_OFF] = CFG_ID_NAME;
    raw[CFG_SUBSEL_OFF] = 0;
    raw[CFG_SIZE_OFF] = 21;
    let header = InputConfigHeader::parse(&raw).expect("8 byte header phải giải mã được");
    assert_eq!(header.select, CFG_ID_NAME);
    assert_eq!(header.subsel, 0);
    assert_eq!(header.size, 21);
    assert_eq!(header.payload_bytes(), 21);
}

#[test]
fn config_header_ngan_hon_8_byte_bi_tu_choi() {
    let raw = [0u8; CFG_HEADER_BYTES];
    for len in 0..CFG_HEADER_BYTES {
        assert_eq!(
            InputConfigHeader::parse(&raw[..len]),
            None,
            "header {len} byte phải bị từ chối"
        );
    }
}

#[test]
fn config_header_size_vuot_union_bi_kep_ve_128() {
    // `size` là u8 nên tối đa 255, nhưng union chỉ 128 byte ⇒ driver **không** đọc quá union.
    let mut raw = [0u8; CFG_HEADER_BYTES];
    raw[CFG_SIZE_OFF] = 255;
    let header = InputConfigHeader::parse(&raw).expect("header hợp lệ");
    assert_eq!(header.payload_bytes(), CFG_U_BYTES);
    assert_eq!(CFG_U_BYTES, 128);
}

#[test]
fn config_hang_so_offset_khop_spec() {
    assert_eq!(CFG_SELECT_OFF, 0);
    assert_eq!(CFG_SUBSEL_OFF, 1);
    assert_eq!(CFG_SIZE_OFF, 2);
    assert_eq!(CFG_U_OFF, 8);
    assert_eq!(CFG_HEADER_BYTES, 8);
    assert_eq!(CFG_BYTES, 136);
    assert_eq!(ABSINFO_BYTES, 20);
    assert_eq!(DEVIDS_BYTES, 8);
    assert_eq!(CFG_U_OFF + CFG_U_BYTES, CFG_BYTES);
}

#[test]
fn config_header_encode_ghi_dung_ba_byte() {
    let header = InputConfigHeader {
        select: CFG_EV_BITS,
        subsel: EV_KEY as u8,
        size: 44,
    };
    let raw = header.encode();
    assert_eq!(raw[CFG_SELECT_OFF], CFG_EV_BITS);
    assert_eq!(raw[CFG_SUBSEL_OFF], EV_KEY as u8);
    assert_eq!(raw[CFG_SIZE_OFF], 44);
    // 5 byte `reserved` phải bằng 0 (spec: driver ghi 0).
    assert_eq!(&raw[3..8], &[0u8; 5]);
    assert_eq!(InputConfigHeader::parse(&raw), Some(header));
}

#[test]
fn decode_name_bo_nul_cuoi() {
    // QEMU gửi `size = sizeof("QEMU Virtio Keyboard")` = 21 ⇒ 20 ký tự + NUL.
    let mut raw = [0u8; 21];
    raw[..20].copy_from_slice(b"QEMU Virtio Keyboard");
    assert_eq!(decode_name(&raw), Some("QEMU Virtio Keyboard"));
    // Không có NUL cuối vẫn đọc được (thiết bị khác có thể không gửi NUL).
    assert_eq!(decode_name(b"keyboard"), Some("keyboard"));
}

#[test]
fn decode_name_rong_hoac_khong_in_duoc_bi_tu_choi() {
    assert_eq!(decode_name(b""), None, "tên rỗng phải bị từ chối");
    assert_eq!(decode_name(&[0u8; 8]), None, "toàn NUL phải bị từ chối");
    // Byte điều khiển / không in được ⇒ không phải tên hợp lệ.
    assert_eq!(decode_name(b"key\x01board"), None);
    assert_eq!(decode_name(&[0xff, 0xfe]), None);
    // Khoảng trắng hai đầu **không** bị cắt (tên là dữ liệu của thiết bị, không phải để làm đẹp).
    assert_eq!(decode_name(b" k "), Some(" k "));
}

// ---------------------------------------------------------------- (C) bảng chuyển mã

#[test]
fn key_to_ascii_chu_thuong_khong_shift() {
    assert_eq!(key_to_ascii(KEY_A, false), Some(b'a'));
    assert_eq!(key_to_ascii(KEY_Q, false), Some(b'q'));
    assert_eq!(key_to_ascii(KEY_Z, false), Some(b'z'));
    // **Ba** hàng rời, thứ tự **QWERTY** (bảng Linux không có `KEY_A..=KEY_Z` liền và không
    // theo alphabet): mỗi hàng phải khớp **nguyên văn** chuỗi chữ thường.
    let rows = [
        (KEY_Q, KEY_P, "qwertyuiop"),
        (KEY_A, KEY_L, "asdfghjkl"),
        (KEY_Z, KEY_M, "zxcvbnm"),
    ];
    for (first, last, want) in rows {
        assert_eq!((last - first) as usize + 1, want.len(), "cỡ hàng");
        for (index, ch) in want.bytes().enumerate() {
            assert_eq!(
                key_to_ascii(first + index as u16, false),
                Some(ch),
                "KEY code {}",
                first + index as u16
            );
        }
    }
    // 26 chữ cái phải phủ **đúng** 26 mã (không dư, không thiếu).
    let mut letters = 0u16;
    for code in 0..=0xff {
        if key_to_ascii(code, false).is_some_and(|b| b.is_ascii_lowercase()) {
            letters += 1;
        }
    }
    assert_eq!(letters, 26);
}

#[test]
fn key_to_ascii_chu_hoa_khi_shift() {
    assert_eq!(key_to_ascii(KEY_A, true), Some(b'A'));
    assert_eq!(key_to_ascii(KEY_Z, true), Some(b'Z'));
    let rows = [
        (KEY_Q, KEY_P, "QWERTYUIOP"),
        (KEY_A, KEY_L, "ASDFGHJKL"),
        (KEY_Z, KEY_M, "ZXCVBNM"),
    ];
    for (first, last, want) in rows {
        assert_eq!((last - first) as usize + 1, want.len(), "cỡ hàng");
        for (index, ch) in want.bytes().enumerate() {
            assert_eq!(
                key_to_ascii(first + index as u16, true),
                Some(ch),
                "KEY code {}",
                first + index as u16
            );
        }
    }
    // Ca âm của bẫy KT-255: `;` (39) và `'` (40) nằm **giữa** dải 30..=50 nhưng **không** phải
    // chữ cái — bản đầu trả 'j'/'k' cho hai mã này.
    assert_eq!(key_to_ascii(KEY_SEMICOLON, false), Some(b';'));
    assert_eq!(key_to_ascii(KEY_APOSTROPHE, false), Some(b'\''));
    assert_eq!(key_to_ascii(KEY_GRAVE, false), Some(b'`'));
    assert_eq!(key_to_ascii(KEY_BACKSLASH, false), Some(b'\\'));
}

#[test]
fn key_to_ascii_so_va_ky_tu_shift_cua_so() {
    // Hàng số: 1..9 rồi 0 (KEY_0 = 11 nằm **sau** KEY_9 = 10).
    assert_eq!(key_to_ascii(KEY_1, false), Some(b'1'));
    assert_eq!(key_to_ascii(KEY_9, false), Some(b'9'));
    assert_eq!(key_to_ascii(KEY_0, false), Some(b'0'));
    // Shift của hàng số là ký tự US: ! @ # $ % ^ & * ( )
    let shifted = *b"!@#$%^&*()";
    for (index, want) in shifted.iter().enumerate() {
        let code = if index == 9 {
            KEY_0
        } else {
            KEY_1 + index as u16
        };
        assert_eq!(key_to_ascii(code, true), Some(*want), "KEY code {code}");
    }
}

#[test]
fn key_to_ascii_dau_cau_co_ban() {
    let cases = [
        (KEY_MINUS, b'-', b'_'),
        (KEY_EQUAL, b'=', b'+'),
        (KEY_LEFTBRACE, b'[', b'{'),
        (KEY_RIGHTBRACE, b']', b'}'),
        (KEY_SEMICOLON, b';', b':'),
        (KEY_APOSTROPHE, b'\'', b'"'),
        (KEY_GRAVE, b'`', b'~'),
        (KEY_BACKSLASH, b'\\', b'|'),
        (KEY_COMMA, b',', b'<'),
        (KEY_DOT, b'.', b'>'),
        (KEY_SLASH, b'/', b'?'),
    ];
    for (code, plain, shifted) in cases {
        assert_eq!(key_to_ascii(code, false), Some(plain), "code {code} thường");
        assert_eq!(key_to_ascii(code, true), Some(shifted), "code {code} shift");
    }
}

#[test]
fn key_to_ascii_dieu_khien_ascii() {
    assert_eq!(key_to_ascii(KEY_ENTER, false), Some(b'\n'));
    assert_eq!(key_to_ascii(KEY_BACKSPACE, false), Some(0x08));
    assert_eq!(key_to_ascii(KEY_TAB, false), Some(b'\t'));
    assert_eq!(key_to_ascii(KEY_ESC, false), Some(0x1b));
    // Space **không** đổi khi Shift (Shift+Space vẫn là space).
    assert_eq!(key_to_ascii(KEY_SPACE, false), Some(b' '));
    assert_eq!(key_to_ascii(KEY_SPACE, true), Some(b' '));
}

#[test]
fn key_to_ascii_ma_la_tra_none() {
    // Ca âm: mã 0 (KEY_RESERVED), phím không sinh ASCII (F1/mũi tên), mã ngoài bảng, mã rác.
    for code in [0u16, KEY_F1, KEY_UP, 0x00ff, 0x02ff, 0xffff] {
        assert_eq!(key_to_ascii(code, false), None, "code {code} thường");
        assert_eq!(key_to_ascii(code, true), None, "code {code} shift");
    }
    // Shift **không** làm một mã lạ thành ASCII.
    assert_eq!(key_to_ascii(KEY_LEFTSHIFT, true), None);
    assert_eq!(key_to_ascii(KEY_RIGHTSHIFT, true), None);
}

#[test]
fn key_state_theo_doi_shift_tra_ascii_dung() {
    let mut state = KeyState::new();
    assert!(!state.shift());
    // `sendkey a`: A xuống, A lên.
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 1)), Some(b'a'));
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 0)), None);
    // `sendkey shift-a`: Shift xuống, A xuống, A lên, Shift lên.
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 1)), None);
    assert!(state.shift());
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 1)), Some(b'A'));
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 0)), None);
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 0)), None);
    assert!(!state.shift());
    // Sau khi nhả Shift, 'a' lại là chữ thường.
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 1)), Some(b'a'));
}

#[test]
fn key_state_bo_qua_syn_va_khong_co_ban_phim() {
    let mut state = KeyState::new();
    // EV_SYN/SYN_REPORT không sinh ký tự và không đổi trạng thái.
    assert_eq!(state.feed(InputEvent::new(EV_SYN, SYN_REPORT, 0)), None);
    assert!(!state.shift());
    // EV_REL/EV_ABS cũng bị bỏ qua (bàn phím không phát, nhưng driver phải chịu được).
    assert_eq!(state.feed(InputEvent::new(EV_REL, 0, 1)), None);
    assert_eq!(state.feed(InputEvent::new(EV_ABS, 0, 1)), None);
    // Phím không có ASCII ⇒ None nhưng **không** đổi shift.
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_F1, 1)), None);
    assert!(!state.shift());
}

#[test]
fn key_state_autorepeat_value_2_la_nhan() {
    // Không đàm phán `VIRTIO_INPUT_F_REP` nên thiết bị **không** gửi value=2; nếu có thì driver
    // coi như nhấn (value != 0) — không im lặng bỏ ký tự.
    let mut state = KeyState::new();
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 2)), Some(b'a'));
}

#[test]
fn key_state_shift_phai_cung_bat_shift() {
    let mut state = KeyState::new();
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_RIGHTSHIFT, 1)), None);
    assert!(state.shift());
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_A, 1)), Some(b'A'));
    assert_eq!(state.feed(InputEvent::new(EV_KEY, KEY_RIGHTSHIFT, 0)), None);
    assert!(!state.shift());
}

#[test]
fn key_state_chuoi_qemu_that_sendkey_a_va_shift_a() {
    // Đúng chuỗi QEMU phát cho `sendkey a` rồi `sendkey shift-a` (kèm EV_SYN/SYN_REPORT cuối
    // mỗi lô — `virtio_input_handle_sync` của QEMU).
    let events = [
        InputEvent::new(EV_KEY, KEY_A, 1),
        InputEvent::new(EV_KEY, KEY_A, 0),
        InputEvent::new(EV_SYN, SYN_REPORT, 0),
        InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 1),
        InputEvent::new(EV_KEY, KEY_A, 1),
        InputEvent::new(EV_KEY, KEY_A, 0),
        InputEvent::new(EV_KEY, KEY_LEFTSHIFT, 0),
        InputEvent::new(EV_SYN, SYN_REPORT, 0),
    ];
    let mut state = KeyState::new();
    let mut typed = [0u8; 8];
    let mut count = 0usize;
    for event in events {
        if let Some(ascii) = state.feed(event) {
            typed[count] = ascii;
            count += 1;
        }
    }
    assert_eq!(count, 2);
    assert_eq!(&typed[..count], b"aA");
}

// ---------------------------------------------------------------- (D) feature/device/queue

#[test]
fn negotiated_features_luon_version1_va_khong_rep() {
    // Thiết bị báo có REP (bit 0) + VERSION_1 (bit 32) ⇒ driver **chỉ** lấy VERSION_1.
    let device = (1u64 << VIRTIO_INPUT_F_VERSION_1) | (1u64 << VIRTIO_INPUT_F_REP);
    let driver = negotiated_features(device);
    assert_eq!(driver, 1u64 << F_VERSION_1);
    assert_eq!(driver & (1u64 << VIRTIO_INPUT_F_REP), 0);
    // Thiết bị **không** có VERSION_1 ⇒ driver vẫn trả 0 (người gọi fail-closed).
    assert_eq!(negotiated_features(0), 0);
    // Hằng của module phải **khớp** hằng chung (một nguồn, không hai số lệch).
    assert_eq!(VIRTIO_INPUT_F_VERSION_1, F_VERSION_1);
}

#[test]
fn device_id_va_kind_pci_khop() {
    assert_eq!(DEVICE_ID_INPUT, 0x1012);
    assert_eq!(VIRTIO_INPUT_KEYBOARD, 0x1012);
    assert_eq!(VIRTIO_INPUT_KEYBOARD_MODERN_ONLY, 0x1052);
    assert!(is_virtio_input(VENDOR_VIRTIO, VIRTIO_INPUT_KEYBOARD));
    assert!(is_virtio_input(
        VENDOR_VIRTIO,
        VIRTIO_INPUT_KEYBOARD_MODERN_ONLY
    ));
    // Ca âm: blk/net/console **không** phải input; vendor lạ **không** phải input.
    assert!(!is_virtio_input(VENDOR_VIRTIO, 0x1001));
    assert!(!is_virtio_input(VENDOR_VIRTIO, 0x1000));
    assert!(!is_virtio_input(VENDOR_VIRTIO, 0x1003));
    assert!(!is_virtio_input(0x1234, VIRTIO_INPUT_KEYBOARD));
    // `VirtioKind::Input` phải khớp **cùng** tập id (một chỗ quyết định).
    assert!(VirtioKind::Input.matches(VENDOR_VIRTIO, VIRTIO_INPUT_KEYBOARD));
    assert!(VirtioKind::Input.matches(VENDOR_VIRTIO, VIRTIO_INPUT_KEYBOARD_MODERN_ONLY));
    assert!(!VirtioKind::Input.matches(VENDOR_VIRTIO, 0x1001));
    assert_eq!(VirtioKind::Input.name(), "input");
}

#[test]
fn hang_so_queue_dung_duoc() {
    assert_eq!(Q_EVENT, 0);
    assert_eq!(QUEUES_REQUIRED, 1);
    assert!(QUEUE_SIZE.is_power_of_two());
    // Số đệm arm phải ≤ cỡ queue (không publish quá ring) và ≥ 5 (một lô `sendkey shift-a`
    // của QEMU là 5 event: Shift↓ A↓ A↑ Shift↑ SYN). Bất biến **lúc biên dịch** nằm ở
    // `input.rs` (`const _`); ở đây kiểm giá trị cụ thể để đổi hằng là test đỏ.
    assert_eq!(EVENT_BUFS, 8);
    assert_eq!(QUEUE_SIZE, 16);
    assert_eq!(MIN_QUEUE_SIZE, 2);
    assert_eq!(QUEUE_SIZE - MIN_QUEUE_SIZE, 14);
}

#[test]
fn loi_co_ten_de_in_evidence() {
    // D2-102: evidence in **tên**, không in số trần.
    let all = [
        InputError::NotReady,
        InputError::NoDevice,
        InputError::BadEvent,
        InputError::Io,
        InputError::Timeout,
        InputError::QueueFull,
    ];
    for error in all {
        assert!(!error.name().is_empty(), "{error:?} phải có tên");
        assert!(
            error.name().chars().all(|c| c.is_ascii_alphabetic()),
            "tên {} phải là chữ",
            error.name()
        );
    }
    for (index, a) in all.iter().enumerate() {
        for b in &all[index + 1..] {
            assert_ne!(a.name(), b.name(), "hai lỗi trùng tên");
        }
    }
}

// ---------------------------------------------------------------- (E) bitmap + devids

#[test]
fn key_bit_doc_bitmap_ev_bits() {
    // Bitmap `EV_BITS` với subsel = EV_KEY: bit `code` nằm ở byte `code/8`, bit `code%8`.
    let mut bitmap = [0u8; 44];
    bitmap[KEY_A as usize / 8] |= 1 << (KEY_A % 8);
    bitmap[KEY_LEFTSHIFT as usize / 8] |= 1 << (KEY_LEFTSHIFT % 8);
    assert!(key_bit(&bitmap, KEY_A));
    assert!(key_bit(&bitmap, KEY_LEFTSHIFT));
    // Ca âm: phím không có trong bitmap ⇒ false; bitmap ngắn hơn chỉ số ⇒ false (không tràn).
    assert!(!key_bit(&bitmap, KEY_Q));
    assert!(!key_bit(&bitmap, 0x2ff));
    assert!(!key_bit(&[], KEY_A));
    assert!(!key_bit(&bitmap[..2], KEY_A));
}

#[test]
fn decode_devids_dung_offset() {
    // bustype @0, vendor @2, product @4, version @6 — mỗi trường u16 LE (spec 1.2 §5.8.4).
    let raw = [0x00, 0x00, 0x27, 0x06, 0x01, 0x00, 0x01, 0x00];
    let ids = decode_devids(&raw).expect("8 byte devids phải giải mã được");
    assert_eq!(
        ids,
        InputDevIds {
            bustype: 0,
            vendor: 0x0627,
            product: 0x0001,
            version: 0x0001,
        }
    );
    // Ca âm: thiếu byte ⇒ None (không đoán).
    for len in 0..DEVIDS_BYTES {
        assert_eq!(decode_devids(&raw[..len]), None, "devids {len} byte");
    }
}

#[test]
fn hang_so_select_khop_spec() {
    use vifast_virtio::input::{CFG_ABS_INFO, CFG_ID_SERIAL, CFG_PROP_BITS, CFG_UNSET};
    assert_eq!(CFG_UNSET, 0x00);
    assert_eq!(CFG_ID_NAME, 0x01);
    assert_eq!(CFG_ID_SERIAL, 0x02);
    assert_eq!(CFG_ID_DEVIDS, 0x03);
    assert_eq!(CFG_PROP_BITS, 0x10);
    assert_eq!(CFG_EV_BITS, 0x11);
    assert_eq!(CFG_ABS_INFO, 0x12);
    // Bốn loại event mà driver phân biệt (spec 1.2 §5.8.6).
    assert_eq!((EV_SYN, EV_KEY, EV_REL, EV_ABS), (0, 1, 2, 3));
    assert_eq!(SYN_REPORT, 0);
}
