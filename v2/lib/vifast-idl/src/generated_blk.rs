// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/blk.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "blk";
#[rustfmt::skip]
pub const IFACE_VERSION: u32 = 1;
#[rustfmt::skip]
pub const METHOD_COUNT: u32 = 5;

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

/// Method `capacity` — id 0 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_CAPACITY: u32 = 0;
/// Method `sector_size` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_SECTOR_SIZE: u32 = 1;
/// Method `read` — id 2 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_READ: u32 = 2;
/// Method `write` — id 3 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_WRITE: u32 = 3;
/// Method `flush` — id 4 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_FLUSH: u32 = 4;

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
        METHOD_CAPACITY => Some(0),
        METHOD_SECTOR_SIZE => Some(0),
        METHOD_READ => Some(2),
        METHOD_WRITE => Some(2),
        METHOD_FLUSH => Some(0),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_CAPACITY => Some(0),
        METHOD_SECTOR_SIZE => Some(0),
        METHOD_READ => Some(1),
        METHOD_WRITE => Some(1),
        METHOD_FLUSH => Some(0),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Capacity,
    SectorSize,
    Read { sector: u64, count: u32 },
    Write { sector: u64, count: u32 },
    Flush,
}

/// Client encode `capacity`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_capacity() -> (u64, [u64; 4]) {
    let mr = [0u64; 4];
    (encode_info(METHOD_CAPACITY, 0, 0), mr)
}

/// Client encode `sector_size`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_sector_size() -> (u64, [u64; 4]) {
    let mr = [0u64; 4];
    (encode_info(METHOD_SECTOR_SIZE, 0, 0), mr)
}

/// Client encode `read`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_read(sector: u64, count: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = sector as u64;
    mr[1] = count as u64;
    (encode_info(METHOD_READ, 1, 2), mr)
}

/// Client encode `write`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_write(sector: u64, count: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = sector as u64;
    mr[1] = count as u64;
    (encode_info(METHOD_WRITE, 1, 2), mr)
}

/// Client encode `flush`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_flush() -> (u64, [u64; 4]) {
    let mr = [0u64; 4];
    (encode_info(METHOD_FLUSH, 0, 0), mr)
}

/// Server/client decode request; `None` = label lạ, arity sai, hoặc tham số tràn kiểu.
#[rustfmt::skip]
pub fn decode(label: u32, length: u8, mr: &[u64; 4]) -> Option<Request> {
    if label & REPLY_BIT != 0 {
        return None;
    }
    match label {
        METHOD_CAPACITY => {
            if length != 0 {
                return None;
            }
            Some(Request::Capacity)
        }
        METHOD_SECTOR_SIZE => {
            if length != 0 {
                return None;
            }
            Some(Request::SectorSize)
        }
        METHOD_READ => {
            if length != 2 {
                return None;
            }
            Some(Request::Read { sector: mr[0], count: u32_of(mr[1])? })
        }
        METHOD_WRITE => {
            if length != 2 {
                return None;
            }
            Some(Request::Write { sector: mr[0], count: u32_of(mr[1])? })
        }
        METHOD_FLUSH => {
            if length != 0 {
                return None;
            }
            Some(Request::Flush)
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

/// Reply của `capacity`: `returns u64`.
#[rustfmt::skip]
pub fn encode_capacity_reply(value: u64) -> (u64, [u64; 4]) {
    (encode_info(METHOD_CAPACITY | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_capacity_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u64> {
    if label != METHOD_CAPACITY | REPLY_BIT || length != 1 {
        return None;
    }
    Some(mr[0])
}

/// Reply của `sector_size`: `returns u32`.
#[rustfmt::skip]
pub fn encode_sector_size_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_SECTOR_SIZE | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_sector_size_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_SECTOR_SIZE | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `read`: `returns u32`.
#[rustfmt::skip]
pub fn encode_read_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_READ | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_read_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_READ | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `write`: `returns u32`.
#[rustfmt::skip]
pub fn encode_write_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_WRITE | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_write_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_WRITE | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `flush`: `returns u32`.
#[rustfmt::skip]
pub fn encode_flush_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_FLUSH | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_flush_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_FLUSH | REPLY_BIT || length != 1 {
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
