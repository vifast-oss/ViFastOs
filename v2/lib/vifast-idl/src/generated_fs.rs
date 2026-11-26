// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ spec/protocols/fs.vidl
// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).
//
// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id
// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).
// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.
// `bytesN` (KT-420): tham số nội tuyến N/8 word MR, little-endian, đệm 0;
// `MessageInfo.length` = số **word MR** của request (`arity`).
#![allow(clippy::all)]

#[rustfmt::skip]
pub const IFACE_NAME: &str = "fs";
#[rustfmt::skip]
pub const IFACE_VERSION: u32 = 1;
#[rustfmt::skip]
pub const METHOD_COUNT: u32 = 10;

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
/// Method `read` — id 1 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_READ: u32 = 1;
/// Method `write` — id 2 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_WRITE: u32 = 2;
/// Method `close` — id 3 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_CLOSE: u32 = 3;
/// Method `mkdir` — id 4 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_MKDIR: u32 = 4;
/// Method `unlink` — id 5 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_UNLINK: u32 = 5;
/// Method `rename` — id 6 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_RENAME: u32 = 6;
/// Method `readdir` — id 7 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_READDIR: u32 = 7;
/// Method `stat` — id 8 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_STAT: u32 = 8;
/// Method `sync` — id 9 (sinh tuần tự từ IDL).
#[rustfmt::skip]
pub const METHOD_SYNC: u32 = 9;

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
        METHOD_OPEN => Some(4),
        METHOD_READ => Some(3),
        METHOD_WRITE => Some(3),
        METHOD_CLOSE => Some(1),
        METHOD_MKDIR => Some(4),
        METHOD_UNLINK => Some(4),
        METHOD_RENAME => Some(4),
        METHOD_READDIR => Some(2),
        METHOD_STAT => Some(1),
        METHOD_SYNC => Some(1),
        _ => None,
    }
}

/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).
#[rustfmt::skip]
pub const fn cap_count(method: u32) -> Option<u8> {
    match method {
        METHOD_OPEN => Some(0),
        METHOD_READ => Some(1),
        METHOD_WRITE => Some(1),
        METHOD_CLOSE => Some(0),
        METHOD_MKDIR => Some(0),
        METHOD_UNLINK => Some(0),
        METHOD_RENAME => Some(1),
        METHOD_READDIR => Some(1),
        METHOD_STAT => Some(1),
        METHOD_SYNC => Some(0),
        _ => None,
    }
}

/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Request {
    Open { path: [u8; 24], len: u32 },
    Read { fd: u32, off: u64, len: u32 },
    Write { fd: u32, off: u64, len: u32 },
    Close { fd: u32 },
    Mkdir { path: [u8; 24], len: u32 },
    Unlink { path: [u8; 24], len: u32 },
    Rename { off1: u32, len1: u32, off2: u32, len2: u32 },
    Readdir { fd: u32, index: u32 },
    Stat { fd: u32 },
    Sync { fd: u32 },
}

/// Client encode `open`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_open(path: [u8; 24], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<24>(&mut mr, 0, &path);
    mr[3] = len as u64;
    (encode_info(METHOD_OPEN, 0, 4), mr)
}

/// Client encode `read`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_read(fd: u32, off: u64, len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = fd as u64;
    mr[1] = off as u64;
    mr[2] = len as u64;
    (encode_info(METHOD_READ, 1, 3), mr)
}

/// Client encode `write`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_write(fd: u32, off: u64, len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = fd as u64;
    mr[1] = off as u64;
    mr[2] = len as u64;
    (encode_info(METHOD_WRITE, 1, 3), mr)
}

/// Client encode `close`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_close(fd: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = fd as u64;
    (encode_info(METHOD_CLOSE, 0, 1), mr)
}

/// Client encode `mkdir`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_mkdir(path: [u8; 24], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<24>(&mut mr, 0, &path);
    mr[3] = len as u64;
    (encode_info(METHOD_MKDIR, 0, 4), mr)
}

/// Client encode `unlink`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_unlink(path: [u8; 24], len: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    put_bytes::<24>(&mut mr, 0, &path);
    mr[3] = len as u64;
    (encode_info(METHOD_UNLINK, 0, 4), mr)
}

/// Client encode `rename`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_rename(off1: u32, len1: u32, off2: u32, len2: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = off1 as u64;
    mr[1] = len1 as u64;
    mr[2] = off2 as u64;
    mr[3] = len2 as u64;
    (encode_info(METHOD_RENAME, 1, 4), mr)
}

/// Client encode `readdir`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_readdir(fd: u32, index: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = fd as u64;
    mr[1] = index as u64;
    (encode_info(METHOD_READDIR, 1, 2), mr)
}

/// Client encode `stat`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_stat(fd: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = fd as u64;
    (encode_info(METHOD_STAT, 1, 1), mr)
}

/// Client encode `sync`: trả `(MessageInfo, MR0..MR3)`.
#[rustfmt::skip]
pub fn encode_sync(fd: u32) -> (u64, [u64; 4]) {
    let mut mr = [0u64; 4];
    mr[0] = fd as u64;
    (encode_info(METHOD_SYNC, 0, 1), mr)
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
        METHOD_OPEN => {
            if length != 4 {
                return None;
            }
            Some(Request::Open { path: get_bytes::<24>(mr, 0), len: u32_of(mr[3])? })
        }
        METHOD_READ => {
            if length != 3 {
                return None;
            }
            Some(Request::Read { fd: u32_of(mr[0])?, off: mr[1], len: u32_of(mr[2])? })
        }
        METHOD_WRITE => {
            if length != 3 {
                return None;
            }
            Some(Request::Write { fd: u32_of(mr[0])?, off: mr[1], len: u32_of(mr[2])? })
        }
        METHOD_CLOSE => {
            if length != 1 {
                return None;
            }
            Some(Request::Close { fd: u32_of(mr[0])? })
        }
        METHOD_MKDIR => {
            if length != 4 {
                return None;
            }
            Some(Request::Mkdir { path: get_bytes::<24>(mr, 0), len: u32_of(mr[3])? })
        }
        METHOD_UNLINK => {
            if length != 4 {
                return None;
            }
            Some(Request::Unlink { path: get_bytes::<24>(mr, 0), len: u32_of(mr[3])? })
        }
        METHOD_RENAME => {
            if length != 4 {
                return None;
            }
            Some(Request::Rename { off1: u32_of(mr[0])?, len1: u32_of(mr[1])?, off2: u32_of(mr[2])?, len2: u32_of(mr[3])? })
        }
        METHOD_READDIR => {
            if length != 2 {
                return None;
            }
            Some(Request::Readdir { fd: u32_of(mr[0])?, index: u32_of(mr[1])? })
        }
        METHOD_STAT => {
            if length != 1 {
                return None;
            }
            Some(Request::Stat { fd: u32_of(mr[0])? })
        }
        METHOD_SYNC => {
            if length != 1 {
                return None;
            }
            Some(Request::Sync { fd: u32_of(mr[0])? })
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

/// Reply của `mkdir`: `returns u32`.
#[rustfmt::skip]
pub fn encode_mkdir_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_MKDIR | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_mkdir_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_MKDIR | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `unlink`: `returns u32`.
#[rustfmt::skip]
pub fn encode_unlink_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_UNLINK | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_unlink_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_UNLINK | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `rename`: `returns u32`.
#[rustfmt::skip]
pub fn encode_rename_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_RENAME | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_rename_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_RENAME | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `readdir`: `returns u32`.
#[rustfmt::skip]
pub fn encode_readdir_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_READDIR | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_readdir_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_READDIR | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `stat`: `returns u32`.
#[rustfmt::skip]
pub fn encode_stat_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_STAT | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_stat_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_STAT | REPLY_BIT || length != 1 {
        return None;
    }
    u32_of(mr[0])
}

/// Reply của `sync`: `returns u32`.
#[rustfmt::skip]
pub fn encode_sync_reply(value: u32) -> (u64, [u64; 4]) {
    (encode_info(METHOD_SYNC | REPLY_BIT, 0, 1), [value as u64, 0, 0, 0])
}

#[rustfmt::skip]
pub fn decode_sync_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<u32> {
    if label != METHOD_SYNC | REPLY_BIT || length != 1 {
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
