// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-objmem` — bộ nhớ object trong Untyped (KT-14, D2-14, QD §3.5).
//!
//! Object nằm ngay trong vùng nhớ người gọi cấp (Untyped): `[ObjHeader 16 B][body T]`,
//! body tại `pa + BODY_OFFSET`. Crate **không** cấp phát toàn cục và **không** dùng
//! `Vec<u8>`; [`Arena`] là bump allocator thô trên vùng do người gọi cấp, trả `None`
//! khi hết chỗ (không panic).
//!
//! Truy cập object **chỉ** qua closure ([`ObjStore::with`], [`ObjStore::with_mut`]):
//! crate không bao giờ tạo tham chiếu tới cả vùng nhớ, chỉ tới header 16 B hoặc tới
//! body của đúng một object. `ObjRef` mang `id` không tái dùng; ref có `id` lệch
//! header hoặc ref đã `destroy` trả [`Dead`].
//!
//! `#![no_std]` cho build bare-metal; test dùng `std`.
//!
//! Ngoài phạm vi KT-14: `with_many_mut` mới là **khung** fail-closed (kiểm trùng `pa`,
//! còn lại trả `Alias`; chưa dựng `[&mut dyn KObj; N]` — phần đầy đủ thuộc KT-15) và
//! fuzz/alias từng cặp (KT-15).
//!
//! Ghi chú thiết kế:
//! - `with`/`with_mut` loại trừ truy cập đồng thời bằng `&self`/`&mut self`; KT-14 chưa
//!   cần `UnsafeCell` (QD §3.5 nhắc `UnsafeCell` cho đường nhiều object). Đường
//!   `with_many_mut`/KT-15 sẽ dùng con trỏ thô + `UnsafeCell` khi dựng `[&mut dyn KObj; N]`.
//! - `ObjRef` **không** mang identity của store: KT-16 phải bảo đảm một store/cụm hoặc
//!   `id` duy nhất toàn cục (KT-14 đếm `next_id` từ 1 cho mỗi `ObjMem`; phần `cluster_id`
//!   trong `id` thuộc K1b).
#![cfg_attr(not(test), no_std)]

mod arena;
mod obj;
mod store;

pub use arena::Arena;
pub use obj::{
    KObj, ObjHeader, ObjRef, PAddr, BODY_OFFSET, HEADER_SIZE, STATE_FINALISING, STATE_LIVE,
};
pub use store::{Alias, Dead, ObjMem, ObjStore};
