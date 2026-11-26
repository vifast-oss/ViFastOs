// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/proc.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
// `bytesN` (KT-420): tham số nội tuyến N/8 word MR, little-endian, đệm 0;
// `MessageInfo.length` = số **word MR** của request (`arity`).
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "proc";
#[rustfmt::skip]
pub const IFACE_VERSION: u32 = 1;
#[rustfmt::skip]
pub const METHOD_COUNT: u32 = 4;

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

/// Method `spawn` — id 0 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_SPAWN: u32 = 0;
/// Method `wait` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_WAIT: u32 = 1;
/// Method `kill` — id 2 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_KILL: u32 = 2;
/// Method `list` — id 3 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_LIST: u32 = 3;

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
        METHOD_SPAWN => Some(4),
        METHOD_WAIT => Some(1),
        METHOD_KILL => Some(2),
        METHOD_LIST => Some(1),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_SPAWN => Some(0),
        METHOD_WAIT => Some(0),
        METHOD_KILL => Some(0),
        METHOD_LIST => Some(0),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Spawn { path: [u8; 24], len: u32 },
    Wait { pid: u32 },
    Kill { pid: u32, how: u32 },
    List { cursor: u32 },
}

/// Client encode `spawn`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_spawn(path: [u8; 24], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<24>(&mut mr, 0, &path);
    mr[3] = len as u64;
    (encode_info(METHOD_SPAWN, 0, 4), mr)
}

/// Client encode `wait`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_wait(pid: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = pid as u64;
    (encode_info(METHOD_WAIT, 0, 1), mr)
}

/// Client encode `kill`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_kill(pid: u32, how: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = pid as u64;
    mr[1] = how as u64;
    (encode_info(METHOD_KILL, 0, 2), mr)
}

/// Client encode `list`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_list(cursor: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = cursor as u64;
    (encode_info(METHOD_LIST, 0, 1), mr)
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
        METHOD_SPAWN => {
            if length != 4 {
                return None;
            }
            Some(Request::Spawn { path: get_bytes::<24>(mr, 0), len: u32_of(mr[3])? })
        }
        METHOD_WAIT => {
            if length != 1 {
                return None;
            }
            Some(Request::Wait { pid: u32_of(mr[0])? })
        }
        METHOD_KILL => {
            if length != 2 {
                return None;
            }
            Some(Request::Kill { pid: u32_of(mr[0])?, how: u32_of(mr[1])? })
        }
        METHOD_LIST => {
            if length != 1 {
                return None;
            }
            Some(Request::List { cursor: u32_of(mr[0])? })
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

/// Reply của `spawn`: `returns u32`.
#[rustfmt::skip]
pub fn encode_spawn_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_SPAWN | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_spawn_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_SPAWN | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `wait`: `returns u32`.
#[rustfmt::skip]
pub fn encode_wait_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_WAIT | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_wait_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_WAIT | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `kill`: `returns u32`.
#[rustfmt::skip]
pub fn encode_kill_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_KILL | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_kill_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_KILL | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `list`: `returns u64`.
#[rustfmt::skip]
pub fn encode_list_reply(value: u64) -> (u64, [u64; 4]) {
    (encode_info(METHOD_LIST | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_list_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u64> {
    if label != METHOD_LIST | REPLY_BIT || length != 1 {
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
