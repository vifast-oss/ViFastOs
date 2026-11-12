// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ObjStore` + `ObjMem`: placement/destroy và truy cập closure (thiết kế `:204-210`).

use core::alloc::Layout;
use core::mem::{align_of, size_of};
use core::ptr;

use crate::arena::Arena;
use crate::many::{self, KindTable};
use crate::obj::{
    body_ptr, header_ptr, KObj, ObjHeader, ObjRef, PAddr, BODY_OFFSET, HEADER_SIZE,
    STATE_FINALISING, STATE_LIVE,
};

/// Tham chiếu không còn trỏ object sống (đã `destroy`, hoặc `id` lệch header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dead;

/// Hai tham chiếu chồng lấn, hoặc đầu vào không dựng được tham chiếu an toàn, trong
/// `with_many_mut`.
///
/// Kết quả của các kiểm **làm được** mà không cần identity store: `pa` ngoài vùng arena
/// của store, ref có `id` lệch header hoặc `state` ≠ LIVE, `kind` chưa đăng ký, trùng
/// `pa`/`id`, chồng vùng. Closure **không** được gọi trong mọi trường hợp `Err(Alias)`.
///
/// `ObjRef` 16 B **không** mang identity store, nên "ref của store khác" chỉ bị chặn
/// khi `pa` rơi ngoài arena hoặc `id`/`kind`/`state` lệch; đó là **hợp đồng** của
/// [`ObjStore`] (xem `place`), không phải bất biến cưỡng chế được.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alias;

/// Hợp đồng truy cập bộ nhớ object (thiết kế `:204-210`).
///
/// Hợp đồng chung: `ObjRef` phải do [`ObjStore::place`] của **cùng store này** trả về và
/// còn sống; [`ObjMem::reset`] làm mọi ref cũ vô hiệu. Ref chết (`id` lệch header hoặc
/// `state` ≠ LIVE) ⇒ `with`/`with_mut` trả [`Dead`], `with_many_mut` trả [`Alias`] cho
/// **cùng điều kiện** (chữ ký thiết kế `:207-208` chỉ có một kiểu lỗi). `ObjRef` không
/// mang identity store nên hợp đồng này **không** được cưỡng chế hoàn toàn.
pub trait ObjStore {
    /// Truy cập chia sẻ body `T` qua closure; ref chết → `Dead`.
    fn with<T: KObj, R>(&self, r: ObjRef<T>, f: impl FnOnce(&T) -> R) -> Result<R, Dead>;

    /// Truy cập độc quyền body `T` qua closure; ref chết → `Dead`.
    fn with_mut<T: KObj, R>(
        &mut self,
        r: ObjRef<T>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<R, Dead>;

    /// Truy cập độc quyền nhiều object; `N ≤ 8`, trùng/chồng từng cặp → `Alias`.
    ///
    /// Nguồn vtable là bảng tĩnh `kind → constructor` (D2-48): `ObjRef` vẫn 16 B.
    /// `pa` ngoài vùng arena, ref chết (`id` lệch/`state` ≠ LIVE), `kind` lạ, trùng
    /// `pa`/`id`, chồng vùng đều trả `Err(Alias)` và closure **không** được gọi.
    /// `N > 8` **không build** (`const { assert!(N <= MAX_MANY) }`).
    ///
    /// Hợp đồng (không cưỡng chế được — xem [`ObjStore`]): mọi `r` phải do store này cấp
    /// và kiểu của object phải khớp `kind` (điều kiện của `place`); `reset` làm ref cũ
    /// vô hiệu.
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
    ///
    /// Ngoài ra `T` phải là kiểu **duy nhất** dùng `v.kind()` trong store này: bảng
    /// `kind → constructor` (D2-48) giả định `kind` xác định đúng một `T`. Hai kiểu khác
    /// nhau dùng chung `kind` sẽ khiến `with_many_mut` dựng `&mut dyn KObj` sai vtable.
    /// Ref trả về chỉ dùng với **store này** (`ObjRef` không mang identity store), và
    /// [`ObjMem::reset`] làm mọi ref cũ vô hiệu.
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
    /// Bảng `kind → constructor` do `place::<T>` đăng ký (D2-48).
    kinds: KindTable,
}

impl ObjMem {
    /// # Safety
    /// `base..base + len` phải là vùng nhớ hợp lệ, ghi được, không bị tham chiếu khác
    /// truy cập trong thời gian `ObjMem` sống.
    pub unsafe fn new(base: *mut u8, len: usize) -> Self {
        Self {
            arena: unsafe { Arena::new(base, len) },
            next_id: 0,
            kinds: KindTable::new(),
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

    /// Bảng `kind → constructor` của store (KT-15 dùng để dựng `dyn KObj`).
    pub(crate) fn bang(&self) -> &KindTable {
        &self.kinds
    }

    /// `pa` có phải đầu một header 16 B **bên trong** vùng Untyped của store này không.
    ///
    /// Chặn `pa` giả/ngoài arena trước khi `ptr::read` header (KT-15); `reset` không làm
    /// ref cũ trỏ ra ngoài vùng, trạng thái "đã revoke" do kiểm `id`/`state` xử lý.
    pub(crate) fn chua_header(&self, pa: PAddr) -> bool {
        pa.is_multiple_of(align_of::<ObjHeader>() as u64) && self.arena.chua(pa, HEADER_SIZE)
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
        // KT-15: khung fail-closed của KT-14 đã thay bằng cài đặt thật (D2-48).
        many::with_many_mut(self, rs, f)
    }

    unsafe fn place<T: KObj>(&mut self, pa: PAddr, v: T) -> ObjRef<T> {
        self.next_id += 1;
        let kind = v.kind();
        let header = ObjHeader {
            kind,
            state: STATE_LIVE,
            _pad: 0,
            id: self.next_id,
        };
        // D2-48: chỉ `place::<T>` biết `T`, nên đây là nơi duy nhất đăng ký constructor.
        self.kinds.register::<T>(kind);
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
    fn with_many_mut_hai_ref_roi_nhau_tra_ok() {
        // KT-15 thay khung fail-closed của KT-14: hai ref rời nhau giờ phải `Ok`.
        // Ma trận alias đầy đủ ở `tests/alias.rs`; nhánh forged ở `many.rs::tests`.
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let a = place(&mut s, Counter { n: 1 });
        let b = place(&mut s, Counter { n: 2 });
        assert_ne!(a.pa(), b.pa());

        let ra = ObjRef::<dyn KObj>::new(a.pa(), a.id());
        let rb = ObjRef::<dyn KObj>::new(b.pa(), b.id());
        let mut called = false;
        let kq = s.with_many_mut([ra, rb], |rs| {
            called = true;
            rs.len()
        });
        assert_eq!(kq, Ok(2));
        assert!(called, "closure phải được gọi khi mọi ref rời nhau");

        // Cùng `pa` (kể cả khác id) vẫn `Alias` — kiểm nhánh trùng địa chỉ.
        let mut called = false;
        let rc = ObjRef::<dyn KObj>::new(a.pa(), a.id() + 1);
        let kq = s.with_many_mut([ra, rc], |_| {
            called = true;
        });
        assert_eq!(kq, Err(Alias));
        assert!(!called);
    }
}
