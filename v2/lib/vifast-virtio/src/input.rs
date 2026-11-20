// SPDX-License-Identifier: MIT OR Apache-2.0
//! `virtio_input` — **logic thuần** của thiết bị virtio-input (bàn phím): device id, feature,
//! `virtio_input_config`, `virtio_input_event`, và **bảng chuyển mã phím → ASCII** — KT-255 (D1).
//!
//! # Phạm vi
//!
//! Chỉ phần **quyết định trên byte**: giải mã `virtio_input_config` (select/subsel/size + union),
//! mã hoá/giải mã `virtio_input_event` 8 byte, đọc bitmap `EV_BITS`, và ánh xạ mã phím Linux
//! (`input-event-codes.h`) sang ASCII. Không MMIO, không `unsafe`, không cấp phát ⇒ chạy host test
//! (D2-05: đỏ trước rồi mới xanh). Driver thật ở
//! `kernel/vifast-plat-riscv64/src/virtio_input.rs` chỉ còn phần chạm thanh ghi + ring.
//!
//! # Nguồn
//!
//! Virtio 1.2 §5.8 (Input device): §5.8.3 feature, §5.8.4 `virtio_input_config`, §5.8.5
//! `virtio_input_absinfo`/`virtio_input_devids`, §5.8.6 `virtio_input_event`. Mã phím lấy từ
//! Linux `include/uapi/linux/input-event-codes.h` (cùng bảng QEMU dùng cho `virtio-keyboard`).
//!
//! # Vì sao **không** đàm phán `VIRTIO_INPUT_F_REP`
//!
//! `VIRTIO_INPUT_F_REP` (bit 0) là feature **duy nhất** riêng của thiết bị input: nó cho phép
//! thiết bị báo **tự lặp** bằng `value = 2`. Driver KT-255 chỉ cần "phím xuống → một ký tự", nên
//! **không** đàm phán bit này (D2-110B: chỉ dùng ngữ nghĩa của feature **đã** đàm phán). Nếu một
//! thiết bị vẫn gửi `value = 2`, [`KeyState::feed`] coi như **nhấn** — không im lặng bỏ ký tự.
//!
//! # Vì sao bảng chuyển mã nằm ở crate **thuần**
//!
//! Đây là chỗ dễ sai nhất của cả driver (lệch một phím, quên `Shift`, `KEY_0` nằm **sau**
//! `KEY_9`…). Đặt ở crate `no_std` không `unsafe` nghĩa là **mọi** ca biên chạy được trên host
//! trước khi chạm QEMU; kernel chỉ gọi [`key_to_ascii`]/[`KeyState`].

/// `virtio-keyboard` bản transitional (device id `0x1012`) — KT-255 nhắm id này.
pub const VIRTIO_INPUT_KEYBOARD: u16 = 0x1012;
/// `virtio-keyboard` bản modern-only (`disable-legacy=on`, device id `0x1052`) — cùng driver.
pub const VIRTIO_INPUT_KEYBOARD_MODERN_ONLY: u16 = 0x1052;
/// Device id mà driver KT-255 tìm trên bus PCI (`0x1012`).
pub const DEVICE_ID_INPUT: u16 = VIRTIO_INPUT_KEYBOARD;

/// `VIRTIO_F_VERSION_1` (bit 32) — feature **chung** bắt buộc của mọi thiết bị virtio 1.0.
///
/// Tên ở đây nhắc "input" vì đây là feature duy nhất driver input đàm phán; giá trị **phải** bằng
/// [`crate::features::F_VERSION_1`] (có `const` assert bên dưới — một nguồn, không hai số lệch).
pub const VIRTIO_INPUT_F_VERSION_1: u32 = 32;
/// `VIRTIO_INPUT_F_REP` (bit 0) — thiết bị báo tự lặp (`value = 2`). **Không** đàm phán (xem đầu
/// tệp); hằng số vẫn ở đây để test/evidence đọc được bit và lần mở sau không phải đoán số.
pub const VIRTIO_INPUT_F_REP: u32 = 0;

// `const` assert: hai hằng phải khớp. Nếu ai đó đổi một bên, ảnh **không biên dịch được** thay vì
// driver đàm phán nhầm trang feature (bit ≥ 32 nằm ở `device_feature_select = 1`).
const _: () = assert!(VIRTIO_INPUT_F_VERSION_1 == crate::features::F_VERSION_1);

/// `select` chưa đặt (spec 1.2 §5.8.4).
pub const CFG_UNSET: u8 = 0x00;
/// `select` = tên thiết bị (`u.string`, không NUL-terminated theo spec; QEMU gửi kèm NUL).
pub const CFG_ID_NAME: u8 = 0x01;
/// `select` = số sê-ri (`u.string`).
pub const CFG_ID_SERIAL: u8 = 0x02;
/// `select` = `virtio_input_devids` (`u.ids`).
pub const CFG_ID_DEVIDS: u8 = 0x03;
/// `select` = bitmap thuộc tính (`u.bitmap`, `subsel` = 0).
pub const CFG_PROP_BITS: u8 = 0x10;
/// `select` = bitmap mã sự kiện của **một** loại (`subsel` = loại: `EV_KEY`…).
pub const CFG_EV_BITS: u8 = 0x11;
/// `select` = `virtio_input_absinfo` của **một** trục (`subsel` = trục).
pub const CFG_ABS_INFO: u8 = 0x12;

/// Offset `select` trong `virtio_input_config` (spec 1.2 §5.8.4).
pub const CFG_SELECT_OFF: usize = 0;
/// Offset `subsel`.
pub const CFG_SUBSEL_OFF: usize = 1;
/// Offset `size` — số byte **có nghĩa** của union.
pub const CFG_SIZE_OFF: usize = 2;
/// Offset union (`string`/`bitmap`/`abs`/`ids`) — sau 5 byte `reserved`.
pub const CFG_U_OFF: usize = 8;
/// Cỡ phần đầu (select/subsel/size/reserved) của `virtio_input_config`.
pub const CFG_HEADER_BYTES: usize = 8;
/// Cỡ union của `virtio_input_config` (`char string[128]` / `u8 bitmap[128]`).
pub const CFG_U_BYTES: usize = 128;
/// Cỡ **đầy đủ** `virtio_input_config` = 8 + 128.
pub const CFG_BYTES: usize = CFG_HEADER_BYTES + CFG_U_BYTES;
/// Cỡ `virtio_input_absinfo` (5 × `__le32`: min/max/fuzz/flat/res).
pub const ABSINFO_BYTES: usize = 20;
/// Cỡ `virtio_input_devids` (4 × `__le16`: bustype/vendor/product/version).
pub const DEVIDS_BYTES: usize = 8;

/// `EV_SYN` — sự kiện đồng bộ (spec 1.2 §5.8.6).
pub const EV_SYN: u16 = 0;
/// `EV_KEY` — phím/nút.
pub const EV_KEY: u16 = 1;
/// `EV_REL` — trục tương đối (chuột).
pub const EV_REL: u16 = 2;
/// `EV_ABS` — trục tuyệt đối (tablet).
pub const EV_ABS: u16 = 3;
/// `SYN_REPORT` — mã của `EV_SYN` kết thúc một lô sự kiện.
pub const SYN_REPORT: u16 = 0;

/// Cỡ `virtio_input_event` (u16 type + u16 code + u32 value).
pub const EVENT_BYTES: usize = 8;

/// Queue `event` (thiết bị → driver) — spec 1.2 §5.8.2.
pub const Q_EVENT: u16 = 0;
/// Queue `status` (driver → thiết bị: LED…) — KT-255 **không** dùng.
pub const Q_STATUS: u16 = 1;
/// Số queue driver **cần**: đúng một (`event`).
pub const QUEUES_REQUIRED: u16 = 1;
/// Cỡ queue driver đặt (luỹ thừa 2). 16 đủ cho một lô `sendkey` + dư, rẻ về `.bss`.
pub const QUEUE_SIZE: u16 = 16;
/// Cỡ queue tối thiểu dùng được (spec: luỹ thừa 2, ≥ 2).
pub const MIN_QUEUE_SIZE: u16 = 2;
/// Số đệm **8 byte** driver arm sẵn trên queue `event`.
///
/// **Vì sao > 1**: QEMU gom sự kiện tới khi thấy `EV_SYN`/`SYN_REPORT` rồi mới lấy **một đệm cho
/// mỗi sự kiện** trong lô; thiếu đệm thì cả lô bị **huỷ** (`virtqueue_unpop`) — mất phím. Một lô
/// `sendkey shift-a` là 5 sự kiện (Shift↓ A↓ A↑ Shift↑ SYN) ⇒ phải arm ≥ 5; 8 là mức rẻ và dư.
pub const EVENT_BUFS: usize = 8;

// Bất biến **lúc biên dịch** của hai hằng trên: arm quá cỡ queue là publish tràn ring; arm < 5 là
// mất lô `sendkey shift-a` của QEMU (5 sự kiện). Đổi một hằng mà vi phạm ⇒ ảnh không biên dịch.
const _: () = assert!(EVENT_BUFS >= 5);
const _: () = assert!(EVENT_BUFS as u16 <= QUEUE_SIZE);

// ---------------------------------------------------------------- mã phím (Linux input-event-codes.h)
/// `KEY_ESC`.
pub const KEY_ESC: u16 = 1;
/// `KEY_1` (hàng số bắt đầu ở 2).
pub const KEY_1: u16 = 2;
/// `KEY_9`.
pub const KEY_9: u16 = 10;
/// `KEY_0` — **sau** `KEY_9` trong bảng Linux (bẫy hay gặp khi viết bảng chuyển mã).
pub const KEY_0: u16 = 11;
/// `KEY_MINUS`.
pub const KEY_MINUS: u16 = 12;
/// `KEY_EQUAL`.
pub const KEY_EQUAL: u16 = 13;
/// `KEY_BACKSPACE`.
pub const KEY_BACKSPACE: u16 = 14;
/// `KEY_TAB`.
pub const KEY_TAB: u16 = 15;
/// `KEY_Q` (hàng chữ trên bắt đầu ở 16).
pub const KEY_Q: u16 = 16;
/// `KEY_P` — cuối dải `KEY_Q..=KEY_P` (16..=25).
pub const KEY_P: u16 = 25;
/// `KEY_LEFTBRACE`.
pub const KEY_LEFTBRACE: u16 = 26;
/// `KEY_RIGHTBRACE`.
pub const KEY_RIGHTBRACE: u16 = 27;
/// `KEY_ENTER`.
pub const KEY_ENTER: u16 = 28;
/// `KEY_LEFTCTRL`.
pub const KEY_LEFTCTRL: u16 = 29;
/// `KEY_A` — đầu dải `KEY_A..=KEY_L` (30..=38).
pub const KEY_A: u16 = 30;
/// `KEY_L` — cuối dải `KEY_A..=KEY_L`.
pub const KEY_L: u16 = 38;
/// `KEY_SEMICOLON`.
pub const KEY_SEMICOLON: u16 = 39;
/// `KEY_APOSTROPHE`.
pub const KEY_APOSTROPHE: u16 = 40;
/// `KEY_GRAVE`.
pub const KEY_GRAVE: u16 = 41;
/// `KEY_LEFTSHIFT`.
pub const KEY_LEFTSHIFT: u16 = 42;
/// `KEY_BACKSLASH`.
pub const KEY_BACKSLASH: u16 = 43;
/// `KEY_Z` — đầu dải `KEY_Z..=KEY_M` (44..=50).
pub const KEY_Z: u16 = 44;
/// `KEY_M` — cuối dải `KEY_Z..=KEY_M`.
pub const KEY_M: u16 = 50;
/// `KEY_COMMA`.
pub const KEY_COMMA: u16 = 51;
/// `KEY_DOT`.
pub const KEY_DOT: u16 = 52;
/// `KEY_SLASH`.
pub const KEY_SLASH: u16 = 53;
/// `KEY_RIGHTSHIFT`.
pub const KEY_RIGHTSHIFT: u16 = 54;
/// `KEY_LEFTALT`.
pub const KEY_LEFTALT: u16 = 56;
/// `KEY_SPACE`.
pub const KEY_SPACE: u16 = 57;
/// `KEY_CAPSLOCK`.
pub const KEY_CAPSLOCK: u16 = 58;
/// `KEY_F1` — phím **không** sinh ASCII (dùng cho ca "mã lạ" của TAP).
pub const KEY_F1: u16 = 59;
/// `KEY_UP` — phím **không** sinh ASCII.
pub const KEY_UP: u16 = 103;
/// `KEY_LEFTMETA`.
pub const KEY_LEFTMETA: u16 = 125;

/// `true` ⇔ `(vendor, device)` là virtio-keyboard mà KT-255 chấp nhận (`0x1012`/`0x1052`).
pub const fn is_virtio_input(vendor: u16, device: u16) -> bool {
    vendor == crate::pci::VENDOR_VIRTIO
        && (device == VIRTIO_INPUT_KEYBOARD || device == VIRTIO_INPUT_KEYBOARD_MODERN_ONLY)
}

/// `true` ⇔ `code` là phím Shift (trái **hoặc** phải) — phím **đổi trạng thái**, không sinh ký tự.
pub const fn is_shift(code: u16) -> bool {
    code == KEY_LEFTSHIFT || code == KEY_RIGHTSHIFT
}

/// Bộ feature driver **chọn** từ `device_features`: **chỉ** `VIRTIO_F_VERSION_1` (bit 32).
///
/// **Không** bật `VIRTIO_INPUT_F_REP` kể cả khi thiết bị báo có (xem đầu tệp). Hàm là **một** chỗ
/// quyết định để "driver có bit X" và "driver dùng đường của X" không lệch nhau (D2-110B).
/// Trả `0` khi thiết bị **không** có `VERSION_1` ⇒ người gọi fail-closed.
pub const fn negotiated_features(device_features: u64) -> u64 {
    if device_features & (1u64 << VIRTIO_INPUT_F_VERSION_1) != 0 {
        1u64 << VIRTIO_INPUT_F_VERSION_1
    } else {
        0
    }
}

/// `virtio_input_event` (§5.8.6) — 8 byte, little-endian.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputEvent {
    /// `type` (`EV_SYN`/`EV_KEY`/`EV_REL`/`EV_ABS`…). Tên `event_type` vì `type` là từ khoá Rust.
    pub event_type: u16,
    /// `code` — mã phím/trục (bảng Linux).
    pub code: u16,
    /// `value` — 1 = nhấn, 0 = nhả, 2 = tự lặp (chỉ khi đàm phán `REP`).
    pub value: u32,
}

impl InputEvent {
    /// Dựng một sự kiện.
    pub const fn new(event_type: u16, code: u16, value: u32) -> Self {
        Self {
            event_type,
            code,
            value,
        }
    }

    /// Mã hoá little-endian thành 8 byte.
    pub const fn encode(&self) -> [u8; EVENT_BYTES] {
        let kind = self.event_type.to_le_bytes();
        let code = self.code.to_le_bytes();
        let value = self.value.to_le_bytes();
        [
            kind[0], kind[1], code[0], code[1], value[0], value[1], value[2], value[3],
        ]
    }

    /// Giải mã. `None` nếu `raw` ngắn hơn [`EVENT_BYTES`] — **không** đọc tràn, không đoán.
    pub fn parse(raw: &[u8]) -> Option<Self> {
        if raw.len() < EVENT_BYTES {
            return None;
        }
        Some(Self {
            event_type: u16::from_le_bytes([raw[0], raw[1]]),
            code: u16::from_le_bytes([raw[2], raw[3]]),
            value: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        })
    }

    /// `true` ⇔ là sự kiện phím/nút.
    pub const fn is_key(&self) -> bool {
        self.event_type == EV_KEY
    }

    /// `true` ⇔ là sự kiện đồng bộ.
    pub const fn is_syn(&self) -> bool {
        self.event_type == EV_SYN
    }

    /// `true` ⇔ phím **đang xuống** (`value != 0`, gồm cả tự lặp `2`).
    pub const fn is_key_down(&self) -> bool {
        self.is_key() && self.value != 0
    }

    /// `true` ⇔ phím **vừa nhả** (`value == 0`).
    pub const fn is_key_up(&self) -> bool {
        self.is_key() && self.value == 0
    }
}

/// Phần đầu `virtio_input_config` mà driver **ghi** (select/subsel) rồi **đọc lại** (size).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputConfigHeader {
    /// `select` — loại thông tin muốn đọc.
    pub select: u8,
    /// `subsel` — chỉ số phụ (loại event/trục), 0 nếu không dùng.
    pub subsel: u8,
    /// `size` — số byte **có nghĩa** của union (thiết bị ghi).
    pub size: u8,
}

impl InputConfigHeader {
    /// Dựng header để **ghi** vào device config (5 byte `reserved` = 0).
    pub const fn new(select: u8, subsel: u8) -> Self {
        Self {
            select,
            subsel,
            size: 0,
        }
    }

    /// Mã hoá 8 byte (select/subsel/size + 5 byte `reserved` = 0).
    pub const fn encode(&self) -> [u8; CFG_HEADER_BYTES] {
        [self.select, self.subsel, self.size, 0, 0, 0, 0, 0]
    }

    /// Giải mã 8 byte đầu. `None` nếu ngắn hơn [`CFG_HEADER_BYTES`].
    pub fn parse(raw: &[u8]) -> Option<Self> {
        if raw.len() < CFG_HEADER_BYTES {
            return None;
        }
        Some(Self {
            select: raw[CFG_SELECT_OFF],
            subsel: raw[CFG_SUBSEL_OFF],
            size: raw[CFG_SIZE_OFF],
        })
    }

    /// Số byte union **có nghĩa** = `min(size, 128)` — `size` là `u8` nên có thể vượt union; driver
    /// **không** đọc quá union (spec: `size` ≤ 128).
    pub const fn payload_bytes(&self) -> usize {
        if self.size as usize > CFG_U_BYTES {
            CFG_U_BYTES
        } else {
            self.size as usize
        }
    }
}

/// `virtio_input_devids` (§5.8.5) — danh tính thiết bị, đọc bằng `select = ID_DEVIDS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputDevIds {
    /// `bustype` (`BUS_VIRTUAL` = 0x06 trên Linux; QEMU gửi 0).
    pub bustype: u16,
    /// `vendor`.
    pub vendor: u16,
    /// `product`.
    pub product: u16,
    /// `version`.
    pub version: u16,
}

/// Giải mã `virtio_input_devids` (8 byte). `None` nếu ngắn hơn [`DEVIDS_BYTES`].
pub fn decode_devids(raw: &[u8]) -> Option<InputDevIds> {
    if raw.len() < DEVIDS_BYTES {
        return None;
    }
    Some(InputDevIds {
        bustype: u16::from_le_bytes([raw[0], raw[1]]),
        vendor: u16::from_le_bytes([raw[2], raw[3]]),
        product: u16::from_le_bytes([raw[4], raw[5]]),
        version: u16::from_le_bytes([raw[6], raw[7]]),
    })
}

/// Giải mã **tên** thiết bị (`select = ID_NAME`): bỏ NUL cuối, đòi ASCII in được.
///
/// `None` nếu rỗng, toàn NUL, hoặc có byte ngoài `0x20..=0x7e` — fail-closed: một "tên" chứa byte
/// điều khiển không phải tên, và in nó ra log là làm hỏng bằng chứng.
pub fn decode_name(raw: &[u8]) -> Option<&str> {
    let mut end = raw.len();
    while end > 0 && raw[end - 1] == 0 {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let bytes = &raw[..end];
    for &byte in bytes {
        if !(0x20..=0x7e).contains(&byte) {
            return None;
        }
    }
    core::str::from_utf8(bytes).ok()
}

/// `true` ⇔ bit `code` bật trong bitmap `EV_BITS` (byte `code/8`, bit `code%8`).
///
/// Bitmap ngắn hơn chỉ số ⇒ `false` (không đọc tràn). Dùng để chứng minh thiết bị **có** phím ta
/// sắp bơm, thay vì tin lời khai.
pub fn key_bit(bitmap: &[u8], code: u16) -> bool {
    let index = code as usize / 8;
    if index >= bitmap.len() {
        return false;
    }
    bitmap[index] & (1 << (code % 8)) != 0
}

/// Độ dài một sự kiện từ `used->len` của queue `event`: **phải đúng** [`EVENT_BYTES`].
///
/// Thiết bị ghi một `virtio_input_event` vào mỗi đệm 8 byte ⇒ `len` khác 8 là bất thường; driver
/// **không** kẹp im lặng mà trả [`InputError::Io`] (D2-110: dùng `qsize`/`len` thật, không đoán).
pub const fn event_len(used_len: u32) -> Result<usize, InputError> {
    if used_len == EVENT_BYTES as u32 {
        Ok(EVENT_BYTES)
    } else {
        Err(InputError::Io)
    }
}

/// Bảng ký tự **Shift** của hàng số `KEY_1..KEY_9` (US layout).
const SHIFTED_DIGITS: [u8; 9] = *b"!@#$%^&*(";

/// Ba hàng chữ cái của bàn phím US, **đúng thứ tự mã** của bảng Linux.
///
/// **Bẫy đã tự gặp (KT-255)**: mã phím **không** theo thứ tự alphabet — `KEY_Q..=KEY_P` là
/// `q w e r t y u i o p` (QWERTY), `KEY_A..=KEY_L` là `a s d f g h j k l`, `KEY_Z..=KEY_M` là
/// `z x c v b n m`. Và `KEY_A..=KEY_Z` **không** phải một dải liên tiếp: 30..=50 còn chứa
/// `KEY_SEMICOLON` (39), `KEY_APOSTROPHE` (40), `KEY_GRAVE` (41), `KEY_LEFTSHIFT` (42),
/// `KEY_BACKSLASH` (43). Hai bản sai đầu tiên (`base + (code - first)`) cho `w`→`r`, `;`→`j`,
/// `M`(77)→`M` — test host bắt **trước** khi chạm QEMU (D2-05).
const ROW_TOP: [u8; 10] = *b"qwertyuiop";
const ROW_HOME: [u8; 9] = *b"asdfghjkl";
const ROW_BOTTOM: [u8; 7] = *b"zxcvbnm";

/// Dịch một chữ thường sang chữ hoa khi `shift` (chênh 32 — chỉ đúng cho ASCII chữ cái).
const fn letter(lower: u8, shift: bool) -> u8 {
    if shift {
        lower - 32
    } else {
        lower
    }
}

/// **Bảng chuyển mã phím → ASCII** (layout US, không dead-key, không compose).
///
/// `shift` là trạng thái Shift **tại lúc phím xuống** (driver theo dõi bằng [`KeyState`]).
/// `None` ⇔ phím **không** sinh byte ASCII (F1, mũi tên, Ctrl/Alt/Meta, mã ngoài bảng) — ca âm
/// bắt buộc của bảng này.
///
/// Quy ước có chủ đích: `Enter` → `\n` (0x0a), `Backspace` → 0x08, `Tab` → `\t`, `Esc` → 0x1b,
/// `Space` → `' '` (Shift **không** đổi). Đây là các mã ASCII điều khiển chuẩn, không phải "ký tự
/// in được" — nhưng chúng là byte ASCII thật mà một shell cần.
pub const fn key_to_ascii(code: u16, shift: bool) -> Option<u8> {
    // Chữ cái: **ba** hàng rời, tra bảng theo đúng thứ tự mã (xem `ROW_*`).
    if code >= KEY_Q && code <= KEY_P {
        return Some(letter(ROW_TOP[(code - KEY_Q) as usize], shift));
    }
    if code >= KEY_A && code <= KEY_L {
        return Some(letter(ROW_HOME[(code - KEY_A) as usize], shift));
    }
    if code >= KEY_Z && code <= KEY_M {
        return Some(letter(ROW_BOTTOM[(code - KEY_Z) as usize], shift));
    }
    // Hàng số: `KEY_1..=KEY_9` = 2..=10; `KEY_0` = 11 nằm **sau** `KEY_9`.
    if code >= KEY_1 && code <= KEY_9 {
        let index = (code - KEY_1) as usize;
        return Some(if shift {
            SHIFTED_DIGITS[index]
        } else {
            b'1' + index as u8
        });
    }
    match code {
        KEY_0 => Some(if shift { b')' } else { b'0' }),
        KEY_SPACE => Some(b' '),
        KEY_ENTER => Some(b'\n'),
        KEY_BACKSPACE => Some(0x08),
        KEY_TAB => Some(b'\t'),
        KEY_ESC => Some(0x1b),
        KEY_MINUS => Some(if shift { b'_' } else { b'-' }),
        KEY_EQUAL => Some(if shift { b'+' } else { b'=' }),
        KEY_LEFTBRACE => Some(if shift { b'{' } else { b'[' }),
        KEY_RIGHTBRACE => Some(if shift { b'}' } else { b']' }),
        KEY_SEMICOLON => Some(if shift { b':' } else { b';' }),
        KEY_APOSTROPHE => Some(if shift { b'"' } else { b'\'' }),
        KEY_GRAVE => Some(if shift { b'~' } else { b'`' }),
        KEY_BACKSLASH => Some(if shift { b'|' } else { b'\\' }),
        KEY_COMMA => Some(if shift { b'<' } else { b',' }),
        KEY_DOT => Some(if shift { b'>' } else { b'.' }),
        KEY_SLASH => Some(if shift { b'?' } else { b'/' }),
        _ => None,
    }
}

/// Máy trạng thái **Shift** + chuyển mã: nuốt sự kiện, trả ký tự khi có phím sinh ASCII.
///
/// Vì sao cần trạng thái: `virtio_input_event` **không** mang cờ Shift; thiết bị chỉ gửi
/// `EV_KEY KEY_LEFTSHIFT 1` rồi `EV_KEY KEY_A 1`. Driver phải nhớ Shift giữa hai sự kiện — và
/// phải nhớ **theo đúng thứ tự** (Shift↑ trước A↓ ⇒ 'a').
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyState {
    shift: bool,
}

impl KeyState {
    /// Trạng thái đầu: Shift **không** bật.
    pub const fn new() -> Self {
        Self { shift: false }
    }

    /// `true` ⇔ Shift đang giữ.
    pub const fn shift(&self) -> bool {
        self.shift
    }

    /// Nuốt **một** sự kiện; trả byte ASCII khi sự kiện là **phím xuống** của phím có ASCII.
    ///
    /// - `EV_SYN`/`EV_REL`/`EV_ABS` ⇒ `None` (không đổi trạng thái).
    /// - Shift xuống/nhả ⇒ cập nhật trạng thái, `None`.
    /// - Phím nhả (`value == 0`) ⇒ `None`.
    /// - Phím xuống (`value != 0`, gồm tự lặp `2`) ⇒ [`key_to_ascii`] với Shift hiện tại.
    pub fn feed(&mut self, event: InputEvent) -> Option<u8> {
        if !event.is_key() {
            return None;
        }
        if is_shift(event.code) {
            self.shift = event.value != 0;
            return None;
        }
        if event.value == 0 {
            return None;
        }
        key_to_ascii(event.code, self.shift)
    }
}

/// Lỗi của driver virtio-input — có **tên** để in evidence (D2-102), không in số trần.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    /// Chưa init / chưa `DRIVER_OK`.
    NotReady,
    /// Không thấy thiết bị virtio-input trên bus (fail-closed, **không** PANIC).
    NoDevice,
    /// Sự kiện sai cỡ (`used->len` ≠ 8) hoặc không giải mã được.
    BadEvent,
    /// `used->len`/`used->id` vô lý.
    Io,
    /// Poll `used` quá hạn (không có ngắt ở KT-255).
    Timeout,
    /// Queue đầy (chưa kịp thu hồi descriptor).
    QueueFull,
}

impl InputError {
    /// Tên để in evidence.
    pub const fn name(self) -> &'static str {
        match self {
            InputError::NotReady => "NotReady",
            InputError::NoDevice => "NoDevice",
            InputError::BadEvent => "BadEvent",
            InputError::Io => "Io",
            InputError::Timeout => "Timeout",
            InputError::QueueFull => "QueueFull",
        }
    }
}
