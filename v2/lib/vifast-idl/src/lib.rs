// SPDX-License-Identifier: MIT OR Apache-2.0
// **KT-404** (`D2-259`) — crate này **0 `unsafe`** ⇒ khoá bằng máy: mọi `unsafe`
// thêm vào (kể cả trong macro) là lỗi biên dịch, không chỉ là luật cấu trúc R5.
#![forbid(unsafe_code)]
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

/// Interface **socket** — **D2-242 nhóm 6 (6A)**: sinh từ `spec/protocols/socket.vidl`; đa caller
/// theo `handle` (u32) do server cấp; kernel `NetSvc` giữ đơn caller.
#[path = "generated_socket.rs"]
pub mod socket;

/// Interface **meminfo** — **D2-242 nhóm 4 (4C)**: sinh từ `spec/protocols/meminfo.vidl` (lệnh
/// `free`; không thêm syscall/object).
#[path = "generated_meminfo.rs"]
pub mod meminfo;

/// Interface **fsstat** — **D2-242 nhóm 4 (4C)**: sinh từ `spec/protocols/fsstat.vidl` (`df`;
/// thay mục deferred-K7 "STATFS" bằng giao thức, không mở ABI mới).
#[path = "generated_fsstat.rs"]
pub mod fsstat;

// ---------------------------------------------------------------------------------------------
// **KT-420** (D2-262) — bộ **6 `.vidl` chuẩn** của Kiến trúc User Space. `socket` ở trên (đã có
// từ D2-242 nhóm 6); năm module dưới đây là phần mới, sinh từ `spec/protocols/<tên>.vidl`:
//
//   * `ns`   — định danh endpoint (`namesrv`): tên nội tuyến `bytes16`, `lookup` trả cap;
//   * `proc` — tiến trình (`procsrv`): đường dẫn nội tuyến `bytes24` (bố cục `OP_PATH` 24 B);
//   * `fs`   — hệ thống tệp (`fssrv`, KT-422): dữ liệu dài qua buffer cap của người gọi;
//   * `blk`  — thiết bị khối (`blksrv`/`nvmesrv`, KT-422/KT-423b): đơn vị sector;
//   * `term` — terminal (`termsrv`, mốc TUI): byte qua buffer, `put` trả `unit`.
//
// Số method vẫn **sinh tuần tự từ tệp IDL** (D2-102c) — không phải ABI lõi, không đụng
// `spec/abi.toml`. Cú pháp mở rộng tối thiểu (`bytes8..32`, cap `endpoint`/`notification`,
// `reply cap`) được kiểm bằng ca âm ở `v2/test/gen/run_idl_fixtures.sh`.
// ---------------------------------------------------------------------------------------------

/// Interface **ns** — định danh endpoint cho `namesrv` (`KT-420`).
#[path = "generated_ns.rs"]
pub mod ns;

/// Interface **proc** — tiến trình cho `procsrv` (`KT-420`).
#[path = "generated_proc.rs"]
pub mod proc;

/// Interface **fs** — hệ thống tệp cho `fssrv` (`KT-420`, cài thật ở `KT-422`).
#[path = "generated_fs.rs"]
pub mod fs;

/// Interface **blk** — thiết bị khối cho `blksrv`/`nvmesrv` (`KT-420`, cài thật ở `KT-422`).
#[path = "generated_blk.rs"]
pub mod blk;

/// Interface **term** — terminal cho `termsrv` (`KT-420`, mốc TUI).
#[path = "generated_term.rs"]
pub mod term;

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
    #[test]
    fn stub_spec_protocols_sinh_tu_spec_protocols() {
        // **D2-242 nhóm 4C + 6A**: ba IDL mới nằm ở `spec/protocols/` và sinh stub riêng.
        assert_eq!(socket::IFACE_NAME, "socket");
        assert_eq!(socket::METHOD_COUNT, 6);
        assert_eq!(socket::METHOD_OPEN, 0);
        assert_eq!(socket::METHOD_CONNECT, 1);
        assert_eq!(socket::METHOD_SEND, 2);
        assert_eq!(socket::METHOD_RECV, 3);
        assert_eq!(socket::METHOD_POLL, 4);
        assert_eq!(socket::METHOD_CLOSE, 5);
        assert_eq!(socket::cap_count(socket::METHOD_SEND), Some(1));
        assert_eq!(socket::cap_count(socket::METHOD_OPEN), Some(0));
        assert_eq!(socket::arity(socket::METHOD_POLL), Some(1));
        // `handle` là u32 do server cấp — decode giữ đúng kiểu.
        let (_info, mr) = socket::encode_open(0, 0);
        assert_eq!(
            socket::decode(socket::METHOD_OPEN, 2, &mr),
            Some(socket::Request::Open { proto: 0, flags: 0 })
        );

        assert_eq!(meminfo::IFACE_NAME, "meminfo");
        assert_eq!(meminfo::METHOD_COUNT, 2);
        assert_eq!(meminfo::METHOD_TOTAL, 0);
        assert_eq!(meminfo::METHOD_FREE, 1);
        assert_eq!(meminfo::cap_count(meminfo::METHOD_FREE), Some(0));

        assert_eq!(fsstat::IFACE_NAME, "fsstat");
        assert_eq!(fsstat::METHOD_COUNT, 3);
        assert_eq!(fsstat::METHOD_BLOCKS, 0);
        assert_eq!(fsstat::METHOD_BLOCKS_FREE, 1);
        assert_eq!(fsstat::METHOD_BLOCK_SIZE, 2);

        // Tệp sinh phải khai **đúng nguồn** trong `spec/protocols/` (D2-242 nhóm 6).
        assert!(include_str!("generated_socket.rs").contains("spec/protocols/socket.vidl"));
        assert!(include_str!("generated_meminfo.rs").contains("spec/protocols/meminfo.vidl"));
        assert!(include_str!("generated_fsstat.rs").contains("spec/protocols/fsstat.vidl"));
    }

    // =========================================================================================
    // **KT-420** (D2-262) — bộ 6 `.vidl` chuẩn. Test dưới đây là hợp đồng **do máy kiểm** cho
    // `ns`/`proc`/`fs`/`blk`/`term` (socket đã có test ở trên): số method, method id, số **word
    // MR** (`arity`), cap request/reply và roundtrip `bytesN` nội tuyến.
    // =========================================================================================

    /// Số method + id (sinh tuần tự từ IDL) + `arity` (word MR) + cap của 5 interface mới.
    #[test]
    fn sau_giao_thuc_chuan_kt420() {
        // `ns` — 3 method, tên nội tuyến 2 word + `len` ⇒ 3 word; chỉ `register` mang cap request,
        // chỉ `lookup` mang cap reply.
        assert_eq!(ns::IFACE_NAME, "ns");
        assert_eq!(ns::IFACE_VERSION, 1);
        assert_eq!(ns::METHOD_COUNT, 3);
        assert_eq!(ns::METHOD_REGISTER, 0);
        assert_eq!(ns::METHOD_LOOKUP, 1);
        assert_eq!(ns::METHOD_UNREGISTER, 2);
        assert_eq!(ns::arity(ns::METHOD_REGISTER), Some(3));
        assert_eq!(ns::arity(ns::METHOD_LOOKUP), Some(3));
        assert_eq!(ns::cap_count(ns::METHOD_REGISTER), Some(1));
        assert_eq!(ns::cap_count(ns::METHOD_LOOKUP), Some(0));
        assert_eq!(ns::reply_cap_count(ns::METHOD_LOOKUP), Some(1));
        assert_eq!(ns::reply_cap_count(ns::METHOD_REGISTER), Some(0));
        assert_eq!(ns::cap_count(99), None);

        // `proc` — 4 method; `spawn` dùng trọn 4 word (`bytes24` + `len`), không method nào cap.
        assert_eq!(proc::IFACE_NAME, "proc");
        assert_eq!(proc::METHOD_COUNT, 4);
        assert_eq!(proc::METHOD_SPAWN, 0);
        assert_eq!(proc::METHOD_WAIT, 1);
        assert_eq!(proc::METHOD_KILL, 2);
        assert_eq!(proc::METHOD_LIST, 3);
        assert_eq!(proc::arity(proc::METHOD_SPAWN), Some(4));
        assert_eq!(proc::arity(proc::METHOD_WAIT), Some(1));
        assert_eq!(proc::arity(proc::METHOD_KILL), Some(2));
        assert_eq!(proc::arity(proc::METHOD_LIST), Some(1));
        assert_eq!(proc::cap_count(proc::METHOD_SPAWN), Some(0));

        // `fs` — 10 method; cap chỉ ở các method dữ liệu (buffer của người gọi).
        assert_eq!(fs::IFACE_NAME, "fs");
        assert_eq!(fs::METHOD_COUNT, 10);
        assert_eq!(fs::METHOD_OPEN, 0);
        assert_eq!(fs::METHOD_READ, 1);
        assert_eq!(fs::METHOD_WRITE, 2);
        assert_eq!(fs::METHOD_CLOSE, 3);
        assert_eq!(fs::METHOD_MKDIR, 4);
        assert_eq!(fs::METHOD_UNLINK, 5);
        assert_eq!(fs::METHOD_RENAME, 6);
        assert_eq!(fs::METHOD_READDIR, 7);
        assert_eq!(fs::METHOD_STAT, 8);
        assert_eq!(fs::METHOD_SYNC, 9);
        assert_eq!(fs::arity(fs::METHOD_OPEN), Some(4));
        assert_eq!(fs::arity(fs::METHOD_READ), Some(3));
        assert_eq!(fs::arity(fs::METHOD_RENAME), Some(4));
        assert_eq!(fs::arity(fs::METHOD_READDIR), Some(2));
        assert_eq!(fs::arity(fs::METHOD_STAT), Some(1));
        assert_eq!(fs::cap_count(fs::METHOD_READ), Some(1));
        assert_eq!(fs::cap_count(fs::METHOD_RENAME), Some(1));
        assert_eq!(fs::cap_count(fs::METHOD_OPEN), Some(0));

        // `blk` — đơn vị sector; `read`/`write` mang buffer cap.
        assert_eq!(blk::IFACE_NAME, "blk");
        assert_eq!(blk::METHOD_COUNT, 5);
        assert_eq!(blk::METHOD_CAPACITY, 0);
        assert_eq!(blk::METHOD_SECTOR_SIZE, 1);
        assert_eq!(blk::METHOD_READ, 2);
        assert_eq!(blk::METHOD_WRITE, 3);
        assert_eq!(blk::METHOD_FLUSH, 4);
        assert_eq!(blk::arity(blk::METHOD_CAPACITY), Some(0));
        assert_eq!(blk::arity(blk::METHOD_READ), Some(2));
        assert_eq!(blk::cap_count(blk::METHOD_READ), Some(1));
        assert_eq!(blk::cap_count(blk::METHOD_FLUSH), Some(0));

        // `term` — `put` trả `unit` (reply length 0), `get` trả `u32`; cả hai mang buffer cap.
        assert_eq!(term::IFACE_NAME, "term");
        assert_eq!(term::METHOD_COUNT, 2);
        assert_eq!(term::METHOD_PUT, 0);
        assert_eq!(term::METHOD_GET, 1);
        assert_eq!(term::arity(term::METHOD_PUT), Some(1));
        assert_eq!(term::cap_count(term::METHOD_PUT), Some(1));
        assert_eq!(term::cap_count(term::METHOD_GET), Some(1));
        let (info, mr) = term::encode_put_reply();
        assert_eq!((info >> term::INFO_LENGTH_SHIFT) & 0x7F, 0);
        assert_eq!((info >> term::INFO_CAPS_SHIFT) & 0x7, 0);
        assert_eq!(mr, [0u64; 4]);
        assert_eq!(
            term::decode_put_reply(term::METHOD_PUT | term::REPLY_BIT, 0),
            Some(())
        );
    }

    /// `bytesN` nội tuyến: đúng little-endian qua ranh giới word, `len` không cắt ngầm, và
    /// `decode` từ chối arity (word) sai / `u32` tràn kiểu **sau** vùng bytes.
    #[test]
    fn roundtrip_bytes_va_arity_word() {
        // Tên 4 byte ("fsrv") + đệm 0: word 0 = chính nó little-endian, word 1 = 0.
        let mut name = [0u8; 16];
        name[..4].copy_from_slice(b"fsrv");
        let (info, mr) = ns::encode_register(name, 4);
        assert_eq!(
            (info >> ns::INFO_CAPS_SHIFT) & 0x7,
            1,
            "register mang 1 cap endpoint"
        );
        assert_eq!(
            (info >> ns::INFO_LENGTH_SHIFT) & 0x7F,
            3,
            "length = số WORD MR"
        );
        assert_eq!(mr[0], u64::from_le_bytes(*b"fsrv\0\0\0\0"));
        assert_eq!(mr[1], 0);
        assert_eq!(mr[2], 4);
        assert_eq!(
            ns::decode(ns::METHOD_REGISTER, 3, &mr),
            Some(ns::Request::Register { name, len: 4 })
        );
        // Thiếu/thừa **word** đều bị từ chối (D2-104), không cắt ngầm.
        assert_eq!(ns::decode(ns::METHOD_REGISTER, 2, &mr), None);
        assert_eq!(ns::decode(ns::METHOD_REGISTER, 4, &mr), None);
        // `len` nằm ở MR2: tràn u32 ⇒ từ chối.
        let mut wide = mr;
        wide[2] = 1 << 32;
        assert_eq!(ns::decode(ns::METHOD_REGISTER, 3, &wide), None);

        // 16 byte trọn vẹn: kiểm byte-order qua ranh giới hai word.
        let mut name16 = [0u8; 16];
        for (index, byte) in name16.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let (_info, mr) = ns::encode_unregister(name16, 16);
        assert_eq!(mr[0], u64::from_le_bytes([0, 1, 2, 3, 4, 5, 6, 7]));
        assert_eq!(mr[1], u64::from_le_bytes([8, 9, 10, 11, 12, 13, 14, 15]));
        assert_eq!(
            ns::decode(ns::METHOD_UNREGISTER, 3, &mr),
            Some(ns::Request::Unregister {
                name: name16,
                len: 16
            })
        );

        // `fs.open`: `bytes24` chiếm MR0..MR2, `len` ở MR3 ⇒ 4 word đúng trần.
        let mut path = [0u8; 24];
        path[..2].copy_from_slice(b"/a");
        let (info, mr) = fs::encode_open(path, 2);
        assert_eq!((info >> fs::INFO_LENGTH_SHIFT) & 0x7F, 4);
        assert_eq!(
            fs::decode(fs::METHOD_OPEN, 4, &mr),
            Some(fs::Request::Open { path, len: 2 })
        );
        assert_eq!(fs::decode(fs::METHOD_OPEN, 3, &mr), None);

        // `proc.spawn`: `bytes24` + `len` = 4 word; 24 byte đầu là đường dẫn "/hello.elf".
        let mut elf = [0u8; 24];
        elf[..10].copy_from_slice(b"/hello.elf");
        let (info, mr) = proc::encode_spawn(elf, 10);
        assert_eq!((info >> proc::INFO_LENGTH_SHIFT) & 0x7F, 4);
        assert_eq!(
            proc::decode(proc::METHOD_SPAWN, 4, &mr),
            Some(proc::Request::Spawn { path: elf, len: 10 })
        );
    }

    /// `reply cap` (KT-420): reply của `ns.lookup` đặt `caps = 1` trong `MessageInfo`, giữ label
    /// `METHOD | REPLY_BIT`, và `decode_*_reply` vẫn kiểm length chính xác.
    #[test]
    fn reply_cap_cua_lookup() {
        let (info, mr) = ns::encode_lookup_reply(0);
        assert_eq!(
            (info >> ns::INFO_CAPS_SHIFT) & 0x7,
            1,
            "reply mang 1 cap Endpoint"
        );
        assert_eq!(
            info as u32 & ns::LABEL_MASK,
            ns::METHOD_LOOKUP | ns::REPLY_BIT
        );
        assert_eq!(
            ns::decode_lookup_reply(ns::METHOD_LOOKUP | ns::REPLY_BIT, 1, &mr),
            Some(0)
        );
        assert_eq!(
            ns::decode_lookup_reply(ns::METHOD_LOOKUP | ns::REPLY_BIT, 0, &mr),
            None
        );
        assert_eq!(ns::decode_lookup_reply(ns::METHOD_LOOKUP, 1, &mr), None);
        // Reply của method **không** có `reply cap` giữ caps = 0.
        let (info, _mr) = ns::encode_register_reply(0);
        assert_eq!((info >> ns::INFO_CAPS_SHIFT) & 0x7, 0);
    }

    /// 5 tệp sinh mới: khai **đúng nguồn** `spec/protocols/`, không `unsafe`/asm/todo, và không
    /// dùng `unwrap()` trong mã (fail-closed bằng `Option`).
    #[test]
    fn stub_moi_khong_unsafe_va_dung_nguon() {
        let sources = [
            (include_str!("generated_ns.rs"), "spec/protocols/ns.vidl"),
            (
                include_str!("generated_proc.rs"),
                "spec/protocols/proc.vidl",
            ),
            (include_str!("generated_fs.rs"), "spec/protocols/fs.vidl"),
            (include_str!("generated_blk.rs"), "spec/protocols/blk.vidl"),
            (
                include_str!("generated_term.rs"),
                "spec/protocols/term.vidl",
            ),
        ];
        for (text, source) in sources {
            assert!(text.contains(source), "stub phải khai nguồn {source}");
            assert!(text.contains("SINH TỰ ĐỘNG"));
            let code: String = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                !code.contains("unsafe"),
                "stub {source} không được có unsafe"
            );
            assert!(!code.contains("asm!"), "stub {source} không được có asm");
            assert!(!code.contains("todo!("), "stub {source} không được bỏ dở");
            assert!(
                !code.contains("unwrap("),
                "stub {source} không được unwrap (fail-closed)"
            );
        }
    }
}
