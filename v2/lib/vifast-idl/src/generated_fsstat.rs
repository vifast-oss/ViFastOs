// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/fsstat.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "fsstat";
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

/// Method `blocks` — id 0 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_BLOCKS: u32 = 0;
/// Method `blocks_free` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_BLOCKS_FREE: u32 = 1;
/// Method `block_size` — id 2 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_BLOCK_SIZE: u32 = 2;

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
        METHOD_BLOCKS => Some(0),
        METHOD_BLOCKS_FREE => Some(0),
        METHOD_BLOCK_SIZE => Some(0),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_BLOCKS => Some(0),
        METHOD_BLOCKS_FREE => Some(0),
        METHOD_BLOCK_SIZE => Some(0),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Blocks,
    BlocksFree,
    BlockSize,
}

/// Client encode `blocks`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_blocks() -> (u64, [u64; 4]) {
    let mr = [0u64; 4];
    (encode_info(METHOD_BLOCKS, 0, 0), mr)
}

/// Client encode `blocks_free`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_blocks_free() -> (u64, [u64; 4]) {
    let mr = [0u64; 4];
    (encode_info(METHOD_BLOCKS_FREE, 0, 0), mr)
}

/// Client encode `block_size`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_block_size() -> (u64, [u64; 4]) {
    let mr = [0u64; 4];
    (encode_info(METHOD_BLOCK_SIZE, 0, 0), mr)
}

/// Server/client decode request; `None` = label lạ, arity sai, hoặc tham số tràn kiểu.
#[rustfmt::skip]
pub fn decode(label: u32, length: u8, _mr: &[u64; 4]) -> Option<Request> {
    if label & REPLY_BIT != 0 {
        return None;
    }
    match label {
        METHOD_BLOCKS => {
            if length != 0 {
                return None;
            }
            Some(Request::Blocks)
        }
        METHOD_BLOCKS_FREE => {
            if length != 0 {
                return None;
            }
            Some(Request::BlocksFree)
        }
        METHOD_BLOCK_SIZE => {
            if length != 0 {
                return None;
            }
            Some(Request::BlockSize)
        }
        _ => None,
    }
}

/// Reply của `blocks`: `returns u64`.
#[rustfmt::skip]
pub fn encode_blocks_reply(value: u64) -> (u64, [u64; 4]) {
    (encode_info(METHOD_BLOCKS | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_blocks_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u64> {
    if label != METHOD_BLOCKS | REPLY_BIT || length != 1 {
        return None;
    }
    Some(mr[0])
}

/// Reply của `blocks_free`: `returns u64`.
#[rustfmt::skip]
pub fn encode_blocks_free_reply(value: u64) -> (u64, [u64; 4]) {
    (encode_info(METHOD_BLOCKS_FREE | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_blocks_free_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u64> {
    if label != METHOD_BLOCKS_FREE | REPLY_BIT || length != 1 {
        return None;
    }
    Some(mr[0])
}

/// Reply của `block_size`: `returns u64`.
#[rustfmt::skip]
pub fn encode_block_size_reply(value: u64) -> (u64, [u64; 4]) {
    (encode_info(METHOD_BLOCK_SIZE | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_block_size_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u64> {
    if label != METHOD_BLOCK_SIZE | REPLY_BIT || length != 1 {
        return None;
    }
    Some(mr[0])
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
