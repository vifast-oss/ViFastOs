// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-objmem` — bộ nhớ object trong Untyped (KT-14, D2-14, QD §3.5).
//!
//! Object nằm ngay trong vùng nhớ người gọi cấp (Untyped): `[ObjHeader 16 B][body T]`,
//! body tại `pa + BODY_OFFSET`. Crate **không** cấp phát toàn cục và **không** dùng
//! `Vec<u8>`; [`Arena`] là bump allocator thô trên vùng do người gọi cấp, trả `None`
//! khi hết chỗ (không panic).
//!
//! Truy cập object **chỉ** qua closure ([`ObjStore::with`], [`ObjStore::with_mut`],
//! [`ObjStore::with_many_mut`]): crate không bao giờ tạo tham chiếu tới cả vùng nhớ, chỉ
//! tới header 16 B hoặc tới body của đúng một object. `ObjRef` mang `id` không tái dùng;
//! ref có `id` lệch header hoặc ref đã `destroy` trả [`Dead`] (`with`/`with_mut`) hoặc
//! [`Alias`] (`with_many_mut` — chữ ký thiết kế chỉ có một kiểu lỗi).
//!
//! `#![no_std]` cho build bare-metal; test dùng `std`.
//!
//! `with_many_mut` (KT-15, D2-48): `ObjRef<dyn KObj>` vẫn 16 B; trait object được dựng
//! từ bảng cố định `kind → constructor` do `place::<T>` đăng ký. Alias được kiểm **từng
//! cặp** trước khi tạo bất kỳ tham chiếu nào: `pa` ngoài arena, ref chết (`id` lệch/
//! `state` ≠ LIVE), `kind` lạ, trùng `pa`/`id`, chồng vùng ⇒ `Alias`, closure không gọi;
//! `N > 8` không build. Fuzz chuỗi thao tác ở `tests/fuzz.rs` (10.000 chuỗi stable,
//! 200 chuỗi dưới Miri).
//!
//! Tầng sim (KT-16) dùng vùng nhớ **do objmem sở hữu** ([`OwnedArena`] / [`OwnedMem`],
//! feature `heap`): object body nằm trong `ObjMem` của từng Untyped, watermark của mô hình
//! chính là `Arena::used()`. `with`/`with_mut`/`with_many_mut` đều guard `chua_header` trước
//! khi đọc header (KT-15/KT-16), nên ref của store khác trả `Dead`/`Alias` thay vì đọc bừa.
//!
//! **Hợp đồng không cưỡng chế được**: `ObjRef` không mang identity store, nên ref chỉ
//! được dùng với store sinh ra nó; `with_many_mut` chặn được `pa` ngoài vùng arena và
//! `id`/`kind`/`state` lệch, còn ref của store khác nằm trong cùng vùng nhớ là trách
//! nhiệm người gọi (KT-16 thêm identity cụm). `reset` làm mọi ref cũ vô hiệu.
//!
//! Ghi chú thiết kế:
//! - `with`/`with_mut` loại trừ truy cập đồng thời bằng `&self`/`&mut self`; `with_many_mut`
//!   kiểm alias từng cặp rồi dựng `[&mut dyn KObj; N]` bằng con trỏ thô. Vùng Untyped nằm
//!   ngoài `ObjMem` và mọi truy cập đi qua con trỏ dựng từ `pa`, nên **không** cần
//!   `UnsafeCell`; `&mut self` đã loại trừ `with`/`with_mut` đồng thời.
//! - `ObjRef` **không** mang identity của store: KT-16 phải bảo đảm một store/cụm hoặc
//!   `id` duy nhất toàn cục (KT-14 đếm `next_id` từ 1 cho mỗi `ObjMem`; phần `cluster_id`
//!   trong `id` thuộc K1b). `with_many_mut` có guard `chua_header` (KT-15) để ref của
//!   store khác bị chặn khi `pa` ngoài arena; guard chưa áp cho `with`/`with_mut` (KT-16).
#![cfg_attr(not(test), no_std)]

#[cfg(any(feature = "heap", test))]
extern crate alloc;

mod arena;
mod many;
mod obj;
#[cfg(any(feature = "heap", test))]
mod owned;
mod store;

pub use arena::Arena;
pub use many::{MAX_KINDS, MAX_MANY};
pub use obj::{
    KObj, ObjHeader, ObjRef, PAddr, BODY_OFFSET, HEADER_SIZE, STATE_FINALISING, STATE_LIVE,
};
#[cfg(any(feature = "heap", test))]
pub use owned::{OwnedArena, OwnedMem, ARENA_ALIGN};
pub use store::{Alias, Dead, ObjMem, ObjStore};
