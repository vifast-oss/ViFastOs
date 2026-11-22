// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/socket.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "socket";
#[rustfmt::skip]
pub const IFACE_VERSION: u32 = 1;
#[rustfmt::skip]
pub const METHOD_COUNT: u32 = 6;

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

/// Method `open` — id 0 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_OPEN: u32 = 0;
/// Method `connect` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_CONNECT: u32 = 1;
/// Method `send` — id 2 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_SEND: u32 = 2;
/// Method `recv` — id 3 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_RECV: u32 = 3;
/// Method `poll` — id 4 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_POLL: u32 = 4;
/// Method `close` — id 5 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_CLOSE: u32 = 5;

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
        METHOD_OPEN => Some(2),
        METHOD_CONNECT => Some(3),
        METHOD_SEND => Some(2),
        METHOD_RECV => Some(2),
        METHOD_POLL => Some(1),
        METHOD_CLOSE => Some(1),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_OPEN => Some(0),
        METHOD_CONNECT => Some(0),
        METHOD_SEND => Some(1),
        METHOD_RECV => Some(1),
        METHOD_POLL => Some(0),
        METHOD_CLOSE => Some(0),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Open { proto: u32, flags: u32 },
    Connect { handle: u32, ip: u32, port: u32 },
    Send { handle: u32, len: u32 },
    Recv { handle: u32, len: u32 },
    Poll { handle: u32 },
    Close { handle: u32 },
}

/// Client encode `open`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_open(proto: u32, flags: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = proto as u64;
    mr[1] = flags as u64;
    (encode_info(METHOD_OPEN, 0, 2), mr)
}

/// Client encode `connect`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_connect(handle: u32, ip: u32, port: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = handle as u64;
    mr[1] = ip as u64;
    mr[2] = port as u64;
    (encode_info(METHOD_CONNECT, 0, 3), mr)
}

/// Client encode `send`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_send(handle: u32, len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = handle as u64;
    mr[1] = len as u64;
    (encode_info(METHOD_SEND, 1, 2), mr)
}

/// Client encode `recv`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_recv(handle: u32, len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = handle as u64;
    mr[1] = len as u64;
    (encode_info(METHOD_RECV, 1, 2), mr)
}

/// Client encode `poll`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_poll(handle: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = handle as u64;
    (encode_info(METHOD_POLL, 0, 1), mr)
}

/// Client encode `close`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_close(handle: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = handle as u64;
    (encode_info(METHOD_CLOSE, 0, 1), mr)
}

/// Server/client decode request; `None` = label lạ, arity sai, hoặc tham số tràn kiểu.
#[rustfmt::skip]
pub fn decode(label: u32, length: u8, mr: &[u64; 4]) -> Option<Request> {
    if label & REPLY_BIT != 0 {
        return None;
    }
    match label {
        METHOD_OPEN => {
            if length != 2 {
                return None;
            }
            Some(Request::Open { proto: u32_of(mr[0])?, flags: u32_of(mr[1])? })
        }
        METHOD_CONNECT => {
            if length != 3 {
                return None;
            }
            Some(Request::Connect { handle: u32_of(mr[0])?, ip: u32_of(mr[1])?, port: u32_of(mr[2])? })
        }
        METHOD_SEND => {
            if length != 2 {
                return None;
            }
            Some(Request::Send { handle: u32_of(mr[0])?, len: u32_of(mr[1])? })
        }
        METHOD_RECV => {
            if length != 2 {
                return None;
            }
            Some(Request::Recv { handle: u32_of(mr[0])?, len: u32_of(mr[1])? })
        }
        METHOD_POLL => {
            if length != 1 {
                return None;
            }
            Some(Request::Poll { handle: u32_of(mr[0])? })
        }
        METHOD_CLOSE => {
            if length != 1 {
                return None;
            }
            Some(Request::Close { handle: u32_of(mr[0])? })
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

/// Reply của `open`: `returns u32`.
#[rustfmt::skip]
pub fn encode_open_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_OPEN | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_open_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_OPEN | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `connect`: `returns u32`.
#[rustfmt::skip]
pub fn encode_connect_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_CONNECT | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_connect_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_CONNECT | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `send`: `returns u32`.
#[rustfmt::skip]
pub fn encode_send_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_SEND | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_send_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_SEND | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `recv`: `returns u32`.
#[rustfmt::skip]
pub fn encode_recv_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_RECV | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_recv_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_RECV | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `poll`: `returns u32`.
#[rustfmt::skip]
pub fn encode_poll_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_POLL | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_poll_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_POLL | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `close`: `returns u32`.
#[rustfmt::skip]
pub fn encode_close_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_CLOSE | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_close_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_CLOSE | REPLY_BIT || length != 1 {
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
