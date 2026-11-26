// SPDX-License-Identifier: MIT OR Apache-2.0
// **KT-420** (D2-262) — crate này **0 `unsafe`** ⇒ khoá bằng máy (mọi `unsafe` thêm vào, kể cả
// trong macro, là lỗi biên dịch — không chỉ là luật cấu trúc R5/D2-17).
#![forbid(unsafe_code)]
//! `vifast-namesrv` — **dịch vụ định danh endpoint** của Kiến trúc User Space (`KT-420`).
//!
//! # Vì sao có crate này
//!
//! `KT-420b` bỏ 17 hằng `EP_*` hardcode: mỗi tiến trình chỉ nhận **một** cap `NS_EP` lúc boot rồi
//! **tra cứu** endpoint của dịch vụ khác theo **tên**. Hợp đồng dây là
//! [`spec/protocols/ns.vidl`](../../../../spec/protocols/ns.vidl) (3 method: `register`,
//! `lookup`, `unregister`); stub typed do `vifast-idl` sinh. Crate này là **lõi kiểm được trên
//! host** của dịch vụ đó:
//!
//! | Phần | Ở đây | Ghi chú |
//! |---|---|---|
//! | bảng đăng ký có biên | [`NameService`] | `MAX_ENTRIES` ô; đầy ⇒ `Busy` (14), **không ghi đè** |
//! | kiểm tên | [`valid_name`] | ASCII `[a-z0-9_-]`, `1..=16` byte — đúng `bytes16` của `ns.vidl` |
//! | chuyển `ns.vidl` | [`NameService::handle`] | `ns::Request` đã decode ⇒ [`Response`] (status + cap reply) |
//! | vòng IPC của kernel | **không** ở đây | `KT-420b` nối `Recv`/`Reply` + cấp `NS_EP` |
//!
//! # Vì sao **không** phải `[[bin]]` ở bước này
//!
//! Chương trình U-mode cần bố cục CSpace (`NS_EP`, slot reply, badge) do kernel dựng — đó là
//! `KT-420b` (chủ sở hữu `spec/abi.toml`). Ở `KT-420` crate là **thư viện** `no_std` để `cargo
//! test` chạy trên host và `KT-420b` chỉ việc gọi [`NameService::handle`] trong vòng `Recv`.
//!
//! # Mã trạng thái: **không bịa số**
//!
//! Mọi mã trong [`status`] là **mã có tên của bảng `spec/abi.toml` `[error]`** (sinh vào
//! `vifast_abi::ERRORS`); test host `ma_trang_thai_khop_bang_abi` đối chiếu từng mã với bảng đó
//! ⇒ đổi số trong spec mà quên crate này là **ĐỎ**, không phải "niềm tin".
//!
//! # Fail-closed
//!
//! Mọi đầu vào sai (tên rỗng/quá dài/ký tự lạ, `len` vượt `NAME_MAX`, `register` thiếu cap, bảng
//! đầy) đều trả **mã lỗi có tên** và **không** thay đổi bảng — không panic, không cấp phát, không
//! im lặng ghi đè ô đã có.

#![cfg_attr(not(test), no_std)]

use vifast_idl::ns;

/// Số byte tối đa của một tên — trùng `bytes16` (2 word MR) của `ns.vidl`.
pub const NAME_MAX: usize = 16;

/// Số ô của bảng đăng ký (biên cứng, không cấp phát): đầy ⇒ [`status::FULL`], không ghi đè.
pub const MAX_ENTRIES: usize = 8;

/// Mã trạng thái trả cho client — **mã có tên** của `spec/abi.toml` `[error]`.
///
/// `OK = 0` không nằm trong bảng (bảng bắt đầu từ 1); các mã còn lại chép **đúng số** của bảng và
/// được test host đối chiếu với `vifast_abi::ERRORS` (xem `ma_trang_thai_khop_bang_abi`).
pub mod status {
    /// Thành công (`register`/`lookup`/`unregister` không lỗi).
    pub const OK: u32 = 0;
    /// `InvalidCapability` (1): `register` khai 1 cap nhưng tầng IPC không cài cap nào.
    pub const INVALID_CAP: u32 = 1;
    /// `InvalidArgument` (5): tên rỗng/quá `NAME_MAX`/có byte ngoài `[a-z0-9_-]` hoặc `len` sai.
    pub const INVALID_NAME: u32 = 5;
    /// `SlotOccupied` (7): tên đã đăng ký (không ghi đè cap cũ — tránh đánh tráo dịch vụ).
    pub const OCCUPIED: u32 = 7;
    /// `SlotEmpty` (8): tên chưa đăng ký (`lookup`/`unregister`).
    pub const EMPTY: u32 = 8;
    /// `Busy` (14): bảng đầy (đã đủ [`MAX_ENTRIES`] tên).
    pub const FULL: u32 = 14;
}

/// Một ô của bảng: tên đã đăng ký + **CPTR** nơi cap `Endpoint` của dịch vụ đang nằm.
///
/// Bảng **không** giữ cap thật (cap chỉ sống trong CSpace); `cap_slot` là chỉ số ô mà tầng IPC đã
/// cài cap nhận được. `KT-420b` sẽ đối chiếu badge người gọi trước khi `reply` cap đó.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    name: [u8; NAME_MAX],
    len: u8,
    cap_slot: u32,
}

impl Entry {
    /// Ô rỗng (dùng cho khởi tạo `const`).
    pub const EMPTY: Entry = Entry {
        name: [0; NAME_MAX],
        len: 0,
        cap_slot: 0,
    };

    /// Tên của ô (`&[]` nếu ô rỗng).
    pub fn name(&self) -> &[u8] {
        &self.name[..self.len as usize]
    }

    /// CPTR cap `Endpoint` đã đăng ký cho tên này.
    pub fn cap_slot(&self) -> u32 {
        self.cap_slot
    }
}

/// Kết quả một lời gọi `handle`: `status` trả cho client + cap (nếu reply mang cap).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    /// Mã trạng thái (0 = OK) — xem [`status`].
    pub status: u32,
    /// CPTR cap cần đưa vào **reply** (`lookup` thành công); `None` = reply không mang cap.
    pub cap_slot: Option<u32>,
}

impl Response {
    /// Reply không mang cap.
    pub const fn plain(status: u32) -> Self {
        Self {
            status,
            cap_slot: None,
        }
    }

    /// Reply mang cap (chỉ `lookup` thành công dùng).
    pub const fn with_cap(status: u32, cap_slot: u32) -> Self {
        Self {
            status,
            cap_slot: Some(cap_slot),
        }
    }
}

/// Bảng đăng ký tên → endpoint, biên cứng [`MAX_ENTRIES`], không cấp phát.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameService {
    entries: [Entry; MAX_ENTRIES],
    len: usize,
}

impl Default for NameService {
    fn default() -> Self {
        Self::new()
    }
}

impl NameService {
    /// Dịch vụ rỗng.
    pub const fn new() -> Self {
        Self {
            entries: [Entry::EMPTY; MAX_ENTRIES],
            len: 0,
        }
    }

    /// Số tên đã đăng ký.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Bảng rỗng?
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Ô thứ `index` (chỉ số trong bảng, không phải CPTR).
    pub fn entry(&self, index: usize) -> Option<&Entry> {
        if index < self.len {
            Some(&self.entries[index])
        } else {
            None
        }
    }

    /// Vị trí của `name` trong bảng.
    fn find(&self, name: &[u8]) -> Option<usize> {
        let mut index = 0;
        while index < self.len {
            if self.entries[index].name() == name {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    /// Đăng ký `name` cho cap `Endpoint` đang ở CPTR `cap_slot`.
    ///
    /// Trả [`status::OK`], hoặc mã lỗi có tên: [`status::INVALID_NAME`], [`status::OCCUPIED`],
    /// [`status::FULL`]. **Không** ghi đè tên đã có (chống đánh tráo dịch vụ) và **không** đổi
    /// bảng khi lỗi.
    pub fn register(&mut self, name: &[u8], cap_slot: u32) -> u32 {
        if !valid_name(name) {
            return status::INVALID_NAME;
        }
        if self.find(name).is_some() {
            return status::OCCUPIED;
        }
        if self.len == MAX_ENTRIES {
            return status::FULL;
        }
        let mut slot_name = [0u8; NAME_MAX];
        let mut index = 0;
        while index < name.len() {
            slot_name[index] = name[index];
            index += 1;
        }
        self.entries[self.len] = Entry {
            name: slot_name,
            len: name.len() as u8,
            cap_slot,
        };
        self.len += 1;
        status::OK
    }

    /// Tra cứu `name`; `Ok(cap_slot)` hoặc `Err(mã lỗi có tên)`.
    pub fn lookup(&self, name: &[u8]) -> Result<u32, u32> {
        if !valid_name(name) {
            return Err(status::INVALID_NAME);
        }
        match self.find(name) {
            Some(index) => Ok(self.entries[index].cap_slot),
            None => Err(status::EMPTY),
        }
    }

    /// Bỏ đăng ký `name` (dồn ô trống để `len` luôn là số ô đang dùng).
    ///
    /// Trả [`status::OK`] hoặc [`status::EMPTY`] (kể cả khi gọi hai lần) / [`status::INVALID_NAME`].
    pub fn unregister(&mut self, name: &[u8]) -> u32 {
        let mut index = match self.find(name) {
            Some(index) => index,
            None => {
                return if valid_name(name) {
                    status::EMPTY
                } else {
                    status::INVALID_NAME
                }
            }
        };
        // Dồn các ô sau lên một bậc (bảng nhỏ, không cần danh sách liên kết).
        while index + 1 < self.len {
            self.entries[index] = self.entries[index + 1];
            index += 1;
        }
        self.len -= 1;
        self.entries[self.len] = Entry::EMPTY;
        status::OK
    }

    /// Chuyển một request **đã decode** từ `ns.vidl` thành reply của dịch vụ.
    ///
    /// `cap_slot` = CPTR mà tầng IPC đã cài cap nhận được của message (chỉ `register` dùng);
    /// `None` nghĩa là message không mang cap — `register` khi đó trả [`status::INVALID_CAP`]
    /// thay vì đăng ký một endpoint không tồn tại.
    pub fn handle(&mut self, request: &ns::Request, cap_slot: Option<u32>) -> Response {
        match *request {
            ns::Request::Register { name, len } => {
                let name = match checked_name(&name, len) {
                    Some(name) => name,
                    None => return Response::plain(status::INVALID_NAME),
                };
                let slot = match cap_slot {
                    Some(slot) => slot,
                    None => return Response::plain(status::INVALID_CAP),
                };
                Response::plain(self.register(name, slot))
            }
            ns::Request::Lookup { name, len } => {
                let name = match checked_name(&name, len) {
                    Some(name) => name,
                    None => return Response::plain(status::INVALID_NAME),
                };
                match self.lookup(name) {
                    Ok(slot) => Response::with_cap(status::OK, slot),
                    Err(code) => Response::plain(code),
                }
            }
            ns::Request::Unregister { name, len } => {
                let name = match checked_name(&name, len) {
                    Some(name) => name,
                    None => return Response::plain(status::INVALID_NAME),
                };
                Response::plain(self.unregister(name))
            }
        }
    }
}

/// Tên hợp lệ: `1..=NAME_MAX` byte, mỗi byte thuộc `[a-z0-9_-]`.
///
/// Tên ngắn, không dấu cách, không NUL — nhờ vậy `ns.vidl` nhét vừa `bytes16` và không cần buffer
/// cap cho mỗi lần tra cứu (xem `spec/protocols/ns.vidl`).
pub fn valid_name(name: &[u8]) -> bool {
    if name.is_empty() || name.len() > NAME_MAX {
        return false;
    }
    let mut index = 0;
    while index < name.len() {
        let byte = name[index];
        let lowercase = byte.is_ascii_lowercase();
        let digit = byte.is_ascii_digit();
        if !(lowercase || digit || byte == b'_' || byte == b'-') {
            return false;
        }
        index += 1;
    }
    true
}

/// Lấy `name[..len]` từ field cố định `bytes16`; `None` nếu `len = 0`/`len > NAME_MAX`.
fn checked_name(field: &[u8; NAME_MAX], len: u32) -> Option<&[u8]> {
    let len = usize::try_from(len).ok()?;
    if len == 0 || len > NAME_MAX {
        return None;
    }
    Some(&field[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16 byte mẫu, đệm 0 — dựng field `bytes16` của `ns.vidl` từ một tên ngắn.
    fn field(name: &[u8]) -> [u8; NAME_MAX] {
        let mut out = [0u8; NAME_MAX];
        out[..name.len()].copy_from_slice(name);
        out
    }

    /// Đăng ký rồi tra cứu trả **đúng** CPTR đã đăng ký; tên thứ hai không lẫn.
    #[test]
    fn dang_ky_va_tra_cuu_tra_dung_slot() {
        let mut service = NameService::new();
        assert_eq!(service.register(b"fssrv", 7), status::OK);
        assert_eq!(service.register(b"netsrv", 9), status::OK);
        assert_eq!(service.len(), 2);
        assert_eq!(service.lookup(b"fssrv"), Ok(7));
        assert_eq!(service.lookup(b"netsrv"), Ok(9));
        assert_eq!(service.lookup(b"fssrv1"), Err(status::EMPTY));
        assert_eq!(service.entry(0).map(Entry::name), Some(&b"fssrv"[..]));
        assert_eq!(service.entry(1).map(Entry::cap_slot), Some(9));
        assert_eq!(service.entry(2), None);
    }

    /// Tên trùng ⇒ `SlotOccupied` và **không** ghi đè cap cũ (chống đánh tráo dịch vụ).
    #[test]
    fn dang_ky_trung_khong_ghi_de() {
        let mut service = NameService::new();
        assert_eq!(service.register(b"fssrv", 7), status::OK);
        assert_eq!(service.register(b"fssrv", 8), status::OCCUPIED);
        assert_eq!(service.len(), 1);
        assert_eq!(service.lookup(b"fssrv"), Ok(7), "cap cũ phải giữ nguyên");
    }

    /// Bảng đầy ⇒ `Busy`; mọi tên cũ vẫn tra được (fail-closed, không ghi đè ô cuối).
    #[test]
    fn bang_day_tra_busy_va_giu_du_lieu() {
        let mut service = NameService::new();
        let names: [&[u8]; MAX_ENTRIES] = [b"s0", b"s1", b"s2", b"s3", b"s4", b"s5", b"s6", b"s7"];
        for (index, name) in names.iter().enumerate() {
            assert_eq!(service.register(name, index as u32), status::OK);
        }
        assert_eq!(service.len(), MAX_ENTRIES);
        assert_eq!(service.register(b"s8", 99), status::FULL);
        assert_eq!(service.len(), MAX_ENTRIES);
        assert_eq!(service.lookup(b"s7"), Ok(7));
        assert_eq!(service.lookup(b"s8"), Err(status::EMPTY));
    }

    /// Tên sai (rỗng/quá dài/ký tự lạ) ⇒ `InvalidArgument`, bảng **không** đổi; lookup cũng vậy.
    #[test]
    fn ten_sai_bi_tu_choi_fail_closed() {
        let mut service = NameService::new();
        assert_eq!(service.register(b"", 1), status::INVALID_NAME);
        assert_eq!(
            service.register(b"name-qua-dai-16byte", 1),
            status::INVALID_NAME
        );
        assert_eq!(service.register(b"co dau cach", 1), status::INVALID_NAME);
        assert_eq!(service.register(b"HOA", 1), status::INVALID_NAME);
        assert_eq!(service.register(b"nul\0", 1), status::INVALID_NAME);
        assert_eq!(service.len(), 0);
        assert_eq!(service.lookup(b"HOA"), Err(status::INVALID_NAME));
        assert_eq!(service.lookup(b""), Err(status::INVALID_NAME));
    }

    /// `unregister` dồn ô và cho phép đăng ký lại tên bằng slot **mới**; gọi hai lần ⇒ `SlotEmpty`.
    #[test]
    fn huy_dang_ky_va_dang_ky_lai() {
        let mut service = NameService::new();
        assert_eq!(service.register(b"a", 1), status::OK);
        assert_eq!(service.register(b"b", 2), status::OK);
        assert_eq!(service.register(b"c", 3), status::OK);
        assert_eq!(service.unregister(b"b"), status::OK);
        assert_eq!(service.len(), 2);
        assert_eq!(service.lookup(b"b"), Err(status::EMPTY));
        assert_eq!(
            service.lookup(b"c"),
            Ok(3),
            "ô sau phải dồn lên, cap không mất"
        );
        assert_eq!(service.unregister(b"b"), status::EMPTY);
        assert_eq!(service.register(b"b", 4), status::OK);
        assert_eq!(service.lookup(b"b"), Ok(4));
        assert_eq!(service.lookup(b"a"), Ok(1));
    }

    /// Đường **typed** `ns.vidl`: encode → decode → `handle` → encode reply → decode reply.
    #[test]
    fn handle_qua_stub_ns_vidl() {
        let mut service = NameService::new();
        let name = b"procsrv";
        let (info, mr) = ns::encode_register(field(name), name.len() as u32);
        assert_eq!(
            (info >> ns::INFO_CAPS_SHIFT) & 0x7,
            1,
            "register mang 1 cap"
        );
        let request = ns::decode(ns::METHOD_REGISTER, 3, &mr).expect("decode register");
        let response = service.handle(&request, Some(5));
        assert_eq!(response, Response::plain(status::OK));

        let (_info, mr) = ns::encode_lookup(field(name), name.len() as u32);
        let request = ns::decode(ns::METHOD_LOOKUP, 3, &mr).expect("decode lookup");
        let response = service.handle(&request, None);
        assert_eq!(response, Response::with_cap(status::OK, 5));
        // Reply của lookup mang **đúng** 1 cap và label reply — tầng IPC dùng để cài cap.
        let (reply_info, _reply_mr) = ns::encode_lookup_reply(response.status);
        assert_eq!((reply_info >> ns::INFO_CAPS_SHIFT) & 0x7, 1);
        assert_eq!(
            reply_info as u32 & ns::LABEL_MASK,
            ns::METHOD_LOOKUP | ns::REPLY_BIT
        );

        let (_info, mr) = ns::encode_unregister(field(name), name.len() as u32);
        let request = ns::decode(ns::METHOD_UNREGISTER, 3, &mr).expect("decode unregister");
        assert_eq!(service.handle(&request, None), Response::plain(status::OK));
        assert!(service.is_empty());
    }

    /// `register` khai cap nhưng tầng IPC không cài cap ⇒ `InvalidCapability`, **không** thêm ô.
    #[test]
    fn register_thieu_cap_bi_tu_choi() {
        let mut service = NameService::new();
        let name = b"fssrv";
        let (_info, mr) = ns::encode_register(field(name), name.len() as u32);
        let request = ns::decode(ns::METHOD_REGISTER, 3, &mr).expect("decode register");
        assert_eq!(
            service.handle(&request, None),
            Response::plain(status::INVALID_CAP)
        );
        assert_eq!(service.len(), 0);
        assert_eq!(service.lookup(name), Err(status::EMPTY));
    }

    /// `len` vượt `NAME_MAX` (kể cả `u32::MAX`) ⇒ `InvalidArgument`; không đọc ngoài field.
    #[test]
    fn len_vuot_name_max_bi_tu_choi() {
        let mut service = NameService::new();
        let (_info, mr) = ns::encode_register(field(b"fssrv"), 99);
        let request = ns::decode(ns::METHOD_REGISTER, 3, &mr).expect("decode register");
        assert_eq!(
            service.handle(&request, Some(1)),
            Response::plain(status::INVALID_NAME)
        );
        assert_eq!(service.len(), 0);
        let request = ns::Request::Lookup {
            name: field(b"fssrv"),
            len: u32::MAX,
        };
        assert_eq!(
            service.handle(&request, None),
            Response::plain(status::INVALID_NAME)
        );
    }

    /// Mã trạng thái phải **khớp bảng `spec/abi.toml` `[error]`** (sinh vào `vifast_abi::ERRORS`):
    /// không được bịa số. Đây là nguồn độc lập — sửa spec mà quên crate này là ĐỎ.
    #[test]
    fn ma_trang_thai_khop_bang_abi() {
        fn code(name: &str) -> u32 {
            let mut index = 0;
            while index < vifast_abi::ERRORS.len() {
                let (abi_name, abi_code) = vifast_abi::ERRORS[index];
                if abi_name.as_bytes() == name.as_bytes() {
                    return u32::from(abi_code);
                }
                index += 1;
            }
            panic!("spec/abi.toml [error] thiếu mã {name}");
        }
        assert_eq!(status::INVALID_CAP, code("InvalidCapability"));
        assert_eq!(status::INVALID_NAME, code("InvalidArgument"));
        assert_eq!(status::OCCUPIED, code("SlotOccupied"));
        assert_eq!(status::EMPTY, code("SlotEmpty"));
        assert_eq!(status::FULL, code("Busy"));
        assert_ne!(status::OK, status::FULL);
    }

    /// Bảng không cấp phát động và biên `MAX_ENTRIES` là **hằng của dịch vụ**, không phụ thuộc
    /// số lần gọi: `size_of` bảng giữ nguyên sau nhiều lần đăng ký/huỷ.
    #[test]
    fn bang_khong_cap_phat_va_bien_giu_nguyen() {
        let before = core::mem::size_of::<NameService>();
        let mut service = NameService::new();
        for _ in 0..(MAX_ENTRIES * 4) {
            assert_eq!(service.register(b"loop", 1), status::OK);
            assert_eq!(service.unregister(b"loop"), status::OK);
        }
        assert_eq!(core::mem::size_of::<NameService>(), before);
        assert_eq!(service.len(), 0);
    }
}
