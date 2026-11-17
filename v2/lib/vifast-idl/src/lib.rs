// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-idl` — **stub IDL tối thiểu** của ViFastOs v2 (KT-69).
//!
//! Nguồn: [`idl/ring.vidl`](../idl/ring.vidl) — một interface, N method (tham số vô hướng + tối
//! đa **một** `Frame`/buffer cap), kiểu trả vô hướng. `src/generated.rs` do
//! `v2/tools/vifast-gen/gen_idl.py` sinh; `./v2/bin/vifast-idl --check` nằm trong `v2 check` nên
//! sửa tay bị bắt (cùng luật D2-11 với `spec/abi.toml`).
//!
//! # Vì sao **không** nằm trong `spec/abi.toml`
//!
//! `spec/abi.toml` là nguồn sự thật của **ABI lõi** (16 syscall, mã lỗi, object, chữ ký method
//! kernel). Interface dịch vụ như `ringchan` không phải ABI lõi, và KT-69 **không** mở `spec/`
//! (D2-99). Vì vậy method id sinh **tuần tự từ tệp IDL** — generator ghi rõ nguồn số trong tệp
//! sinh và trong `v2/docs/k6-idl.md`; không có chỗ nào giả vờ đó là số spec (D2-102c).
//!
//! # Stub sinh ra **thuần** encode/decode
//!
//! Không `unsafe`, không `ecall`, không phụ thuộc crate: encode `(MessageInfo, MR0..MR3)` và
//! decode ngược lại, kiểm **arity chính xác** (D2-104) và tràn kiểu (u32 > `u32::MAX` ⇒ từ chối).
//! Chương trình U-mode ghép stub với shim `vifast_user_abi` để gửi/nhận qua Endpoint; nhờ tách
//! như vậy stub test được trên host và không kéo asm vào lib.
//!
//! Cap (`Frame`/buffer) **không** đi trong MR: `MessageInfo.caps` + CPTR trong header IPC buffer
//! (`spec/abi.toml` dòng 58, KT-67b). Stub chỉ mang `caps` count trong `MessageInfo`.
#![cfg_attr(not(test), no_std)]

mod generated;

pub use generated::*;

/// Interface **pager** (KT-70) — sinh từ `idl/pager.vidl`; tên `encode_*`/`decode` riêng module.
#[path = "generated_pager.rs"]
pub mod pager;

#[cfg(test)]
mod tests {
    use super::*;

    /// Tệp sinh phải tự khai nguồn số + **không** có `unsafe`/syscall/todo (KT-69).
    #[test]
    fn generated_khong_unsafe_va_khai_nguon_so() {
        let text = include_str!("generated.rs");
        assert!(text.contains("SINH TỰ ĐỘNG"), "thiếu header sinh tự động");
        assert!(
            text.contains("spec/abi.toml` KHÔNG có bảng method"),
            "thiếu ghi chú nguồn số method (D2-102c)"
        );
        // Chỉ xét **mã**, không xét chú thích (header nói về chữ "unsafe").
        let code: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!code.contains("unsafe"), "stub sinh KHÔNG được có unsafe");
        assert!(!code.contains("asm!"), "stub sinh KHÔNG được có asm");
        assert!(!code.contains("todo!("), "stub sinh không được bỏ dở");
        assert!(!code.contains("unimplemented!("));
    }

    #[test]
    fn hang_interface_va_method_dung_idl() {
        assert_eq!(IFACE_NAME, "ringchan");
        assert_eq!(IFACE_VERSION, 1);
        assert_eq!(METHOD_COUNT, 3);
        // Method id tuần tự từ IDL, không phải số ABI lõi.
        assert_eq!(METHOD_ATTACH, 0);
        assert_eq!(METHOD_SYNC, 1);
        assert_eq!(METHOD_FINISH, 2);
        assert_eq!(arity(METHOD_ATTACH), Some(1));
        assert_eq!(arity(METHOD_SYNC), Some(2));
        assert_eq!(arity(METHOD_FINISH), Some(3));
        assert_eq!(cap_count(METHOD_ATTACH), Some(1));
        assert_eq!(cap_count(METHOD_SYNC), Some(0));
        assert_eq!(cap_count(METHOD_FINISH), Some(0));
        assert_eq!(cap_count(99), None);
        // Reply bit nằm trong 20 bit label, trên các method id nhỏ.
        assert_eq!(REPLY_BIT & !LABEL_MASK, 0);
        // Bit reply là bit 19 của label; method id nhỏ hơn 2^19. Clippy thấy hằng nên cho phép
        // lint "assertion hằng" — đây là **pin hợp đồng** có chủ đích của stub sinh.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(METHOD_COUNT <= REPLY_BIT);
        }
    }

    /// Encode/decode roundtrip **mọi** method + kiểm `length`/`caps` trong `MessageInfo`.
    #[test]
    fn roundtrip_moi_method() {
        let (info, mr) = encode_attach(8);
        assert_eq!(info & 0xF_FFFF, u64::from(METHOD_ATTACH));
        assert_eq!((info >> INFO_CAPS_SHIFT) & 0x7, 1, "attach mang 1 cap");
        assert_eq!((info >> INFO_LENGTH_SHIFT) & 0x7F, 1);
        assert_eq!(
            decode(info as u32 & LABEL_MASK, 1, &mr),
            Some(Request::Attach { slots: 8 })
        );

        let (info, mr) = encode_sync(3, 0x1122_3344_5566_7788);
        assert_eq!((info >> INFO_CAPS_SHIFT) & 0x7, 0);
        assert_eq!(
            decode(METHOD_SYNC, 2, &mr),
            Some(Request::Sync {
                phase: 3,
                count: 0x1122_3344_5566_7788
            })
        );

        let (info, mr) = encode_finish(0, 40, 39);
        assert_eq!((info >> INFO_LENGTH_SHIFT) & 0x7F, 3);
        assert_eq!(
            decode(METHOD_FINISH, 3, &mr),
            Some(Request::Finish {
                err: 0,
                sent: 40,
                got: 39
            })
        );
    }

    /// Arity **chính xác** (D2-104) + label reply không decode như request + label lạ.
    #[test]
    fn decode_tu_choi_arity_va_label_sai() {
        let (_, mr) = encode_sync(1, 2);
        assert_eq!(
            decode(METHOD_SYNC, 1, &mr),
            None,
            "thiếu MR phải bị từ chối"
        );
        assert_eq!(decode(METHOD_SYNC, 3, &mr), None, "thừa MR phải bị từ chối");
        assert_eq!(decode(METHOD_SYNC | REPLY_BIT, 2, &mr), None);
        assert_eq!(decode(77, 0, &mr), None);
        // u32 tràn kiểu: MR0 = 2^32 ⇒ từ chối, không cắt ngầm.
        let mut wide = [0u64; 4];
        wide[0] = 1 << 32;
        assert_eq!(decode(METHOD_ATTACH, 1, &wide), None);
        assert_eq!(decode(METHOD_FINISH, 3, &wide), None);
    }

    /// Reply: encode/decode đúng kiểu trả, sai label/length bị từ chối.
    #[test]
    fn reply_roundtrip() {
        let (info, mr) = encode_attach_reply(0);
        assert_eq!(info as u32 & LABEL_MASK, METHOD_ATTACH | REPLY_BIT);
        assert_eq!(
            decode_attach_reply(METHOD_ATTACH | REPLY_BIT, 1, &mr),
            Some(0)
        );
        assert_eq!(decode_attach_reply(METHOD_ATTACH, 1, &mr), None);

        let (info, mr) = encode_finish_reply(0xdead_beef);
        assert_eq!(
            decode_finish_reply(info as u32 & LABEL_MASK, 1, &mr),
            Some(0xdead_beef)
        );
        assert_eq!(decode_finish_reply(METHOD_FINISH | REPLY_BIT, 0, &mr), None);
        assert_eq!(
            decode_reply(METHOD_FINISH | REPLY_BIT, 1, &mr),
            Some((METHOD_FINISH, 0xdead_beef))
        );
    }

    /// KT-70: interface `pager` — arity chính xác, roundtrip fault/served, reply.
    #[test]
    fn pager_stub_arity_va_roundtrip() {
        use pager::{
            decode, encode_fault, encode_finish, encode_ready, encode_served, encode_stop, Request,
            METHOD_FAULT, METHOD_FINISH, METHOD_READY, METHOD_SERVED, METHOD_STOP, REPLY_BIT,
        };
        assert_eq!(pager::IFACE_NAME, "pager");
        assert_eq!(pager::IFACE_VERSION, 1);
        assert_eq!(pager::METHOD_COUNT, 5);
        assert_eq!(METHOD_FAULT, 0);
        assert_eq!(METHOD_SERVED, 1);
        assert_eq!(METHOD_READY, 2);
        assert_eq!(METHOD_FINISH, 3);
        assert_eq!(METHOD_STOP, 4);
        assert_eq!(pager::arity(METHOD_FAULT), Some(4));
        assert_eq!(pager::arity(METHOD_SERVED), Some(4));
        assert_eq!(pager::arity(METHOD_READY), Some(0));
        assert_eq!(pager::arity(METHOD_FINISH), Some(3));
        assert_eq!(pager::arity(METHOD_STOP), Some(0));
        assert_eq!(pager::cap_count(METHOD_FAULT), Some(0));

        let (info, mr) = encode_fault(1, 0, 4, 0x4200_0000);
        assert_eq!((info >> pager::INFO_LENGTH_SHIFT) & 0x7F, 4);
        assert_eq!(
            decode(METHOD_FAULT, 4, &mr),
            Some(Request::Fault {
                client: 1,
                file: 0,
                page: 4,
                va: 0x4200_0000
            })
        );
        // Thiếu/thừa/tràn u32 đều bị từ chối (D2-104).
        assert_eq!(decode(METHOD_FAULT, 3, &mr), None);
        assert_eq!(decode(METHOD_FAULT, 5, &mr), None);
        let mut wide = mr;
        wide[0] = 1 << 32;
        assert_eq!(decode(METHOD_FAULT, 4, &wide), None);

        let (info, mr) = encode_served(1, 4, 2, 5);
        assert_eq!(
            decode(METHOD_SERVED, 4, &mr),
            Some(Request::Served {
                client: 1,
                page: 4,
                slot: 2,
                status: 5
            })
        );
        let _ = info;
        assert_eq!(decode(METHOD_SERVED | REPLY_BIT, 4, &mr), None);
        let (info, mr) = encode_ready();
        assert_eq!(decode(METHOD_READY, 0, &mr), Some(Request::Ready));
        let _ = decode(METHOD_READY, 0, &mr);
        let _ = info;
        let (info, mr) = encode_finish(11, 2, 8);
        assert_eq!(
            decode(METHOD_FINISH, 3, &mr),
            Some(Request::Finish {
                faults: 11,
                hits: 2,
                misses: 8
            })
        );
        let _ = decode(METHOD_FINISH, 3, &mr);
        let _ = info;
        let (info, mr) = encode_stop();
        assert_eq!(decode(METHOD_STOP, 0, &mr), Some(Request::Stop));
        let _ = decode(METHOD_STOP, 0, &mr);
        let _ = info;
    }

    /// **KT-107**: hằng dịch bit của `MessageInfo` phải khớp **`spec/abi.toml`**, không phải khớp
    /// chính chúng. Trước đây `roundtrip_moi_method` giải mã bằng **đúng** `INFO_CAPS_SHIFT` mà
    /// encoder sinh ra ⇒ đổi hằng số (ví dụ `20` → `21`) thì test **vẫn xanh** (đã chứng minh bằng
    /// đột biến), tức wire format sai mà unit test không bắt. Test này đọc **nguyên văn** dòng
    /// `layout` trong spec và đối chiếu ⇒ nguồn độc lập.
    #[test]
    fn info_shift_khop_spec() {
        let spec = include_str!("../../../../spec/abi.toml");
        let layout = spec
            .lines()
            .find(|line| line.trim_start().starts_with("layout ="))
            .expect("spec/abi.toml phải có dòng layout");
        // Dạng: layout = "label:0..19, caps:20..22, no_reply:23, ... length:28..34"
        let field = |name: &str| -> u32 {
            let start = layout
                .find(&format!("{name}:"))
                .unwrap_or_else(|| panic!("spec thiếu trường {name}"));
            let rest = &layout[start + name.len() + 1..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().expect("spec: số bit không hợp lệ")
        };
        assert_eq!(field("caps"), INFO_CAPS_SHIFT, "caps phải khớp spec");
        assert_eq!(field("length"), INFO_LENGTH_SHIFT, "length phải khớp spec");
        assert_eq!(field("label"), 0, "label bắt đầu ở bit 0");
    }

    /// Tệp sinh của **cả hai** interface đều không `unsafe`/asm (D2-101 tinh thần).
    #[test]
    fn generated_pager_khong_unsafe() {
        let text = include_str!("generated_pager.rs");
        assert!(text.contains("SINH TỰ ĐỘNG"));
        let code: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!code.contains("unsafe"));
        assert!(!code.contains("asm!"));
    }
}
