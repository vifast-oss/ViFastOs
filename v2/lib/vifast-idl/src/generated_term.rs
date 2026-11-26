// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/term.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "term";
#[rustfmt::skip]
pub const IFACE_VERSION: u32 = 1;
#[rustfmt::skip]
pub const METHOD_COUNT: u32 = 2;

/// Bit reply trong `label` 20 bit (bit 19) — reply của một method cùng số method.
#[rustfmt::skip]
pub const REPLY_BIT: u32 = 0x8_0000;
/// Mặt nạ `label` 20 bit (`spec/abi.toml` `[message_info].label_bits = 20`).
#[rustfmt::skip]
pub const LABEL_MASK: u32 = 0xF_FFFF;
/// Dịch bit `caps` của `MessageInfo` (spec `[message_info].caps:20..22`).
#[rustfmt::skip]
pub const INFO_CAPS_SHIFT: u32 = 20;
/// Dịch bit `length` của `MessageInfo` (spec `[message_info].length:28..34`).
#[rustfmt::skip]
pub const INFO_LENGTH_SHIFT: u32 = 28;

/// Method `put` — id 0 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_PUT: u32 = 0;
/// Method `get` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_GET: u32 = 1;

/// Đóng gói `MessageInfo` 35 bit từ `label`/`caps`/`length` (không đổi layout ABI).
#[rustfmt::skip]
pub const fn encode_info(label: u32, caps: u8, length: u8) -> u64 {
    (label as u64 & LABEL_MASK as u64)
        | ((caps as u64 & 0x7) << INFO_CAPS_SHIFT)
        | ((length as u64 & 0x7F) << INFO_LENGTH_SHIFT)
}

/// Số scalar (MR) của từng method — decode kiểm **chính xác** (D2-104: thiếu/thừa ⇒ lỗi).
#[rustfmt::skip]
pub const fn arity(method: u32) -> Option<u8> {
    match method {
        METHOD_PUT => Some(1),
        METHOD_GET => Some(1),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_PUT => Some(1),
        METHOD_GET => Some(1),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Put { len: u32 },
    Get { len: u32 },
}

/// Client encode `put`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_put(len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = len as u64;
    (encode_info(METHOD_PUT, 1, 1), mr)
}

/// Client encode `get`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_get(len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = len as u64;
    (encode_info(METHOD_GET, 1, 1), mr)
}

/// Server/client decode request; `None` = label lạ, arity sai, hoặc tham số tràn kiểu.
#[rustfmt::skip]
pub fn decode(label: u32, length: u8, mr: &[u64; 4]) -> Option<Request> {
    if label & REPLY_BIT != 0 {
        return None;
    }
    match label {
        METHOD_PUT => {
            if length != 1 {
                return None;
            }
            Some(Request::Put { len: u32_of(mr[0])? })
        }
        METHOD_GET => {
            if length != 1 {
                return None;
            }
            Some(Request::Get { len: u32_of(mr[0])? })
        }
        _ => None,
    }
}

/// `u32` không mất bit: giá trị > `u32::MAX` ⇒ `None` (fail-closed, không cắt ngầm).
fn u32_of(value: u64) -> Option<u32> {
    if value <= u32::MAX as u64 {
        Some(value as u32)
    } else {
        None
    }
}

/// Reply của `put`: `returns unit`.
#[rustfmt::skip]
pub fn encode_put_reply() -> (u64, [u64; 4]) {
    (encode_info(METHOD_PUT | REPLY_BIT, 0, 0), [0u64; 4])
}

#[rustfmt::skip]
pub fn decode_put_reply(label: u32, length: u8) -> Option<()> {
    if label == METHOD_PUT | REPLY_BIT && length == 0 {
        Some(())
    } else {
        None
    }
}

/// Reply của `get`: `returns u32`.
#[rustfmt::skip]
pub fn encode_get_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_GET | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_get_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_GET | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply chung (mọi method): trả `(method, value)`; `length = 0` cho `unit`.
#[rustfmt::skip]
pub fn decode_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<(u32, u64)> {
    if label & REPLY_BIT == 0 {
        return None;
    }
    let method = label & !REPLY_BIT;
    let want = arity(method)?;
    match want {
        0 => {
            if length == 0 {
                Some((method, 0))
            } else {
                None
            }
        }
        _ => {
            if length == 1 {
                Some((method, mr[0]))
            } else {
                None
            }
        }
    }
}
