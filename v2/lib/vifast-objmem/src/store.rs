// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ObjStore` + `ObjMem`: placement/destroy và truy cập closure (thiết kế `:204-210`).

use core::alloc::Layout;
use core::mem::{align_of, size_of};
use core::ptr;

use crate::arena::Arena;
use crate::obj::{
    body_ptr, header_ptr, KObj, ObjHeader, ObjRef, PAddr, BODY_OFFSET, STATE_FINALISING, STATE_LIVE,
};

/// Tham chiếu không còn trỏ object sống (đã `destroy`, hoặc `id` lệch header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dead;

/// Hai tham chiếu chồng lấn trong `with_many_mut` (trùng `pa`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alias;

/// Hợp đồng truy cập bộ nhớ object (thiết kế `:204-210`).
pub trait ObjStore {
    /// Truy cập chia sẻ body `T` qua closure; ref chết → `Dead`.
    fn with<T: KObj, R>(&self, r: ObjRef<T>, f: impl FnOnce(&T) -> R) -> Result<R, Dead>;

    /// Truy cập độc quyền body `T` qua closure; ref chết → `Dead`.
    fn with_mut<T: KObj, R>(
        &mut self,
        r: ObjRef<T>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<R, Dead>;

    /// Truy cập độc quyền nhiều object; N ≤ 8, trùng từng cặp → `Alias` (KT-15).
    ///
    /// **KT-14 (khung)**: chưa cài đặt — mọi trường hợp trả `Err(Alias)` (fail-closed,
    /// không panic, closure **không** được gọi); ma trận alias `N = 1..8`, `N = 9`
    /// không build, fuzz 10.000 chuỗi thuộc KT-15.
    fn with_many_mut<const N: usize, R>(
        &mut self,
        rs: [ObjRef<dyn KObj>; N],
        f: impl FnOnce([&mut dyn KObj; N]) -> R,
    ) -> Result<R, Alias>;

    /// Đặt `v` (kèm header) tại `pa` đã cấp; trả ref có `id` mới (chỉ Retype gọi).
    ///
    /// # Safety
    /// `pa` phải là vùng đủ chỗ, đã cấp từ [`ObjMem::alloc`] (hoặc tương đương) và
    /// chưa có object sống; `pa + BODY_OFFSET` phải đúng căn lề cho `T`.
    unsafe fn place<T: KObj>(&mut self, pa: PAddr, v: T) -> ObjRef<T>;

    /// Finalise object: `drop_in_place` body rồi đánh dấu header đã chết.
    ///
    /// Gọi hai lần (hoặc trên ref cũ sau khi `pa` được dùng lại) không UB và không
    /// drop lần hai — hàm trả về ngay khi `id`/`state` không khớp.
    ///
    /// # Safety
    /// `r` phải do [`ObjStore::place`] trả về; `pa` của `r` phải còn là vùng nhớ hợp lệ.
    unsafe fn destroy<T: KObj>(&mut self, r: ObjRef<T>);
}

/// Store object trên một vùng Untyped do người gọi cấp.
pub struct ObjMem {
    arena: Arena,
    next_id: u64,
}

impl ObjMem {
    /// # Safety
    /// `base..base + len` phải là vùng nhớ hợp lệ, ghi được, không bị tham chiếu khác
    /// truy cập trong thời gian `ObjMem` sống.
    pub unsafe fn new(base: *mut u8, len: usize) -> Self {
        Self {
            arena: unsafe { Arena::new(base, len) },
            next_id: 0,
        }
    }

    /// Cấp vùng (header + body, đã xét căn lề) cho object `T`; trả `pa` cho `place`.
    ///
    /// Hết chỗ → `None`, không panic. Với `align_of::<T>() > BODY_OFFSET`, block có
    /// padding đầu để `pa + BODY_OFFSET` vẫn đúng căn lề của `T`.
    pub fn alloc<T: KObj>(&mut self) -> Option<PAddr> {
        let (layout, header_offset) = object_layout::<T>()?;
        let base = self.arena.alloc(layout)?;
        let pa = (base.as_ptr() as usize).checked_add(header_offset)?;
        Some(pa as PAddr)
    }

    /// Số byte đã cấp từ vùng Untyped (gồm padding căn lề).
    #[must_use]
    pub fn used(&self) -> usize {
        self.arena.used()
    }

    /// Tổng số byte của vùng Untyped được cấp lúc `new`.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.arena.capacity()
    }

    /// Đưa watermark về 0 sau khi mọi object đã finalise (Untyped `Revoke`).
    ///
    /// # Safety
    /// Chỉ gọi khi mọi object trong vùng đã `destroy`; mọi `ObjRef` cũ trở nên vô hiệu.
    pub unsafe fn reset(&mut self) {
        unsafe { self.arena.reset() }
    }
}

impl ObjStore for ObjMem {
    fn with<T: KObj, R>(&self, r: ObjRef<T>, f: impl FnOnce(&T) -> R) -> Result<R, Dead> {
        // Đọc bản sao header, không giữ tham chiếu sống qua lúc tạo `&T`.
        // SAFETY: `r` do `place` trả về và vùng nhớ còn hợp lệ (hợp đồng store).
        let header = unsafe { ptr::read(header_ptr(r.pa())) };
        if header.id != r.id() || header.state != STATE_LIVE {
            return Err(Dead);
        }
        // SAFETY: header LIVE + khớp id ⇒ body đã được `place` khởi tạo đúng `T`;
        // `&self` loại trừ `with_mut` đồng thời trên cùng store.
        let body = unsafe { &*body_ptr::<T>(r.pa()) };
        Ok(f(body))
    }

    fn with_mut<T: KObj, R>(
        &mut self,
        r: ObjRef<T>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<R, Dead> {
        // SAFETY: như `with`.
        let header = unsafe { ptr::read(header_ptr(r.pa())) };
        if header.id != r.id() || header.state != STATE_LIVE {
            return Err(Dead);
        }
        // SAFETY: như `with`; `&mut self` loại trừ mọi truy cập đồng thời.
        let body = unsafe { &mut *body_ptr::<T>(r.pa()) };
        Ok(f(body))
    }

    fn with_many_mut<const N: usize, R>(
        &mut self,
        rs: [ObjRef<dyn KObj>; N],
        f: impl FnOnce([&mut dyn KObj; N]) -> R,
    ) -> Result<R, Alias> {
        // KT-14 mới là khung: kiểm trùng `pa` (đúng ngữ nghĩa `Alias`); dựng
        // `[&mut dyn KObj; N]` cần vtable theo `kind` — phần đầy đủ thuộc KT-15.
        // Trả `Alias` an toàn, không panic và không tạo tham chiếu nào.
        for (i, a) in rs.iter().enumerate() {
            for b in rs.iter().skip(i + 1) {
                if a.pa() == b.pa() {
                    return Err(Alias);
                }
            }
        }
        let _ = f;
        Err(Alias)
    }

    unsafe fn place<T: KObj>(&mut self, pa: PAddr, v: T) -> ObjRef<T> {
        self.next_id += 1;
        let header = ObjHeader {
            kind: v.kind(),
            state: STATE_LIVE,
            _pad: 0,
            id: self.next_id,
        };
        // SAFETY: hợp đồng `place` — `pa` đã cấp, đủ chỗ và đúng căn lề cho `T`.
        unsafe {
            ptr::write(header_ptr(pa), header);
            ptr::write(body_ptr::<T>(pa), v);
        }
        ObjRef::new(pa, header.id)
    }

    unsafe fn destroy<T: KObj>(&mut self, r: ObjRef<T>) {
        let pa = r.pa();
        // SAFETY: hợp đồng `destroy` — `pa` của `r` còn là vùng nhớ hợp lệ.
        let mut header = unsafe { ptr::read(header_ptr(pa)) };
        // Đã destroy (hoặc `pa` đã được dùng lại cho object khác) ⇒ no-op.
        if header.id != r.id() || header.state != STATE_LIVE {
            return;
        }
        header.state = STATE_FINALISING;
        // SAFETY: đánh dấu header trước để lần gọi hai là no-op; body đã khởi tạo.
        unsafe {
            ptr::write(header_ptr(pa), header);
            ptr::drop_in_place(body_ptr::<T>(pa));
        }
    }
}

/// Layout của một block object: `(layout, header_offset)`.
///
/// `header_offset` là vị trí header so với đầu block; body luôn ở `pa + BODY_OFFSET`.
fn object_layout<T: KObj>() -> Option<(Layout, usize)> {
    let size = size_of::<T>();
    let align = align_of::<T>();
    let header_align = align_of::<ObjHeader>();
    if align > BODY_OFFSET {
        // Chừa `align` byte đầu để header rơi vào `base + align - BODY_OFFSET`;
        // khi đó body = base + align, đúng căn lề của T.
        let layout = Layout::from_size_align(align.checked_add(size)?, align).ok()?;
        Some((layout, align - BODY_OFFSET))
    } else {
        let layout =
            Layout::from_size_align(BODY_OFFSET.checked_add(size)?, align.max(header_align))
                .ok()?;
        Some((layout, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::ObjRef;

    struct Counter {
        n: u64,
    }

    impl KObj for Counter {
        fn kind(&self) -> u8 {
            42
        }
    }

    #[repr(align(64))]
    struct Buf([u8; 8192]);

    fn store(buf: &mut Buf) -> ObjMem {
        unsafe { ObjMem::new(buf.0.as_mut_ptr(), buf.0.len()) }
    }

    fn place<T: KObj>(s: &mut ObjMem, v: T) -> ObjRef<T> {
        let pa = s.alloc::<T>().expect("còn chỗ");
        unsafe { s.place(pa, v) }
    }

    #[test]
    fn with_va_with_mut_chay_closure() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let r = place(&mut s, Counter { n: 1 });
        assert_eq!(s.with(r, |c| c.n), Ok(1));
        assert_eq!(
            s.with_mut(r, |c| {
                c.n = 7;
                c.n
            }),
            Ok(7)
        );
        assert_eq!(s.with(r, |c| c.n), Ok(7));
    }

    #[test]
    fn ref_id_lech_header_tra_dead() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let r = place(&mut s, Counter { n: 1 });
        let lech = ObjRef::<Counter>::new(r.pa(), r.id() + 1);
        assert_eq!(s.with(lech, |c| c.n), Err(Dead));
        assert_eq!(
            s.with_mut(lech, |c| {
                c.n = 9;
            }),
            Err(Dead)
        );
        // Ref gốc vẫn sống, object không bị đụng.
        assert_eq!(s.with(r, |c| c.n), Ok(1));
    }

    #[test]
    fn with_many_mut_khung_fail_closed_tra_alias() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let a = place(&mut s, Counter { n: 1 });
        let b = place(&mut s, Counter { n: 2 });
        assert_ne!(a.pa(), b.pa());

        // KT-14: khung chưa cài đặt ⇒ fail-closed kể cả khi hai ref rời nhau.
        let ra = ObjRef::<dyn KObj>::new(a.pa(), a.id());
        let rb = ObjRef::<dyn KObj>::new(b.pa(), b.id());
        let mut called = false;
        let kq = s.with_many_mut([ra, rb], |_| {
            called = true;
        });
        assert_eq!(kq, Err(Alias), "KT-14: khung luôn fail-closed");
        assert!(!called, "closure không được gọi khi chưa cài đặt");

        // Cùng `pa` (kể cả khác id) cũng `Alias` — kiểm nhánh trùng địa chỉ.
        let rc = ObjRef::<dyn KObj>::new(a.pa(), a.id() + 1);
        let kq = s.with_many_mut([ra, rc], |_| {
            called = true;
        });
        assert_eq!(kq, Err(Alias));
        assert!(!called);
    }
}
