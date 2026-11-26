// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/ns.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
// `reply cap` (KT-420): reply mang cap — tầng IPC cài vào receive window của người gọi;
// `MessageInfo.caps` của reply = `reply_cap_count(method)`; cap KHÔNG nằm trong MR.
// `bytesN` (KT-420): tham số nội tuyến N/8 word MR, little-endian, đệm 0;
// `MessageInfo.length` = số **word MR** của request (`arity`).
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "ns";
#[rustfmt::skip]
pub const IFACE_VERSION: u32 = 1;
#[rustfmt::skip]
pub const METHOD_COUNT: u32 = 3;

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

/// Method `register` — id 0 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_REGISTER: u32 = 0;
/// Method `lookup` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_LOOKUP: u32 = 1;
/// Method `unregister` — id 2 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_UNREGISTER: u32 = 2;

/// Đóng gói `MessageInfo` 35 bit từ `label`/`caps`/`length` (không đổi layout ABI).
#[rustfmt::skip]
pub const fn encode_info(label: u32, caps: u8, length: u8) -> u64 {
    (label as u64 & LABEL_MASK as u64)
        | ((caps as u64 & 0x7) << INFO_CAPS_SHIFT)
        | ((length as u64 & 0x7F) << INFO_LENGTH_SHIFT)
}

/// Số **word MR** của request từng method (bytesN chiếm N/8 word) — decode kiểm **chính xác** (D2-104: thiếu/thừa ⇒ lỗi).
#[rustfmt::skip]
pub const fn arity(method: u32) -> Option<u8> {
    match method {
        METHOD_REGISTER => Some(3),
        METHOD_LOOKUP => Some(3),
        METHOD_UNREGISTER => Some(3),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_REGISTER => Some(1),
        METHOD_LOOKUP => Some(0),
        METHOD_UNREGISTER => Some(0),
        _ => None,
    }
}

/// Số cap trong **reply** của từng method (0 hoặc 1) — KT-420 (`ns.lookup`).
#[rustfmt::skip]
pub const fn reply_cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_REGISTER => Some(0),
        METHOD_LOOKUP => Some(1),
        METHOD_UNREGISTER => Some(0),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Register { name: [u8; 16], len: u32 },
    Lookup { name: [u8; 16], len: u32 },
    Unregister { name: [u8; 16], len: u32 },
}

/// Client encode `register`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_register(name: [u8; 16], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<16>(&mut mr, 0, &name);
    mr[2] = len as u64;
    (encode_info(METHOD_REGISTER, 1, 3), mr)
}

/// Client encode `lookup`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_lookup(name: [u8; 16], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<16>(&mut mr, 0, &name);
    mr[2] = len as u64;
    (encode_info(METHOD_LOOKUP, 0, 3), mr)
}

/// Client encode `unregister`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_unregister(name: [u8; 16], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<16>(&mut mr, 0, &name);
    mr[2] = len as u64;
    (encode_info(METHOD_UNREGISTER, 0, 3), mr)
}

/// Đóng gói `bytes` (N byte, little-endian, đệm 0) vào `mr[at..at + N/8]`.
#[rustfmt::skip]
pub fn put_bytes<const N: usize>(mr: &mut [u64; 4], at: usize, bytes: &[u8; N]) {
    let mut i = 0;
    while i < N / 8 {
        let mut word = [0u8; 8];
        let mut j = 0;
        while j < 8 {
            word[j] = bytes[i * 8 + j];
            j += 1;
        }
        mr[at + i] = u64::from_le_bytes(word);
        i += 1;
    }
}

/// Giải nén `N` byte từ `mr[at..at + N/8]` (little-endian).
#[rustfmt::skip]
pub fn get_bytes<const N: usize>(mr: &[u64; 4], at: usize) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N / 8 {
        let word = mr[at + i].to_le_bytes();
        let mut j = 0;
        while j < 8 {
            out[i * 8 + j] = word[j];
            j += 1;
        }
        i += 1;
    }
    out
}

/// Server/client decode request; `None` = label lạ, arity sai, hoặc tham số tràn kiểu.
#[rustfmt::skip]
pub fn decode(label: u32, length: u8, mr: &[u64; 4]) -> Option<Request> {
    if label & REPLY_BIT != 0 {
        return None;
    }
    match label {
        METHOD_REGISTER => {
            if length != 3 {
                return None;
            }
            Some(Request::Register { name: get_bytes::<16>(mr, 0), len: u32_of(mr[2])? })
        }
        METHOD_LOOKUP => {
            if length != 3 {
                return None;
            }
            Some(Request::Lookup { name: get_bytes::<16>(mr, 0), len: u32_of(mr[2])? })
        }
        METHOD_UNREGISTER => {
            if length != 3 {
                return None;
            }
            Some(Request::Unregister { name: get_bytes::<16>(mr, 0), len: u32_of(mr[2])? })
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

/// Reply của `register`: `returns u32`.
#[rustfmt::skip]
pub fn encode_register_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_REGISTER | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_register_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_REGISTER | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `lookup`: `returns u32`.
#[rustfmt::skip]
pub fn encode_lookup_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_LOOKUP | REPLY_BIT, 1, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_lookup_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_LOOKUP | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `unregister`: `returns u32`.
#[rustfmt::skip]
pub fn encode_unregister_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_UNREGISTER | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_unregister_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_UNREGISTER | REPLY_BIT || length != 1 {
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
