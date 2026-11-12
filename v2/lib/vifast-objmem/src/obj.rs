// SPDX-License-Identifier: MIT OR Apache-2.0
//! Header 16 B, `ObjRef` và trait `KObj` (thiết kế `:198-214`, QD §3.5).

use core::marker::PhantomData;

/// Địa chỉ vật lý của object trong Untyped (sim: offset trong vùng được cấp).
pub type PAddr = u64;

/// Kích thước `ObjHeader` — 16 B, **không** có trường `refs` (QD §3.5).
pub const HEADER_SIZE: usize = 16;

/// Body của object nằm ngay sau header: `pa + BODY_OFFSET`.
pub const BODY_OFFSET: usize = HEADER_SIZE;

/// `state` của object còn sống.
pub const STATE_LIVE: u8 = 0;
/// `state` sau `destroy` (vòng đời do cap + finalise quyết định — QD §3.5).
pub const STATE_FINALISING: u8 = 1;

/// Header của object kernel: 16 B, `#[repr(C)]`, **không** có `refs`.
///
/// Trường để `pub(crate)` để ngoài crate không dựng được header giả; đọc qua accessor.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjHeader {
    pub(crate) kind: u8,
    pub(crate) state: u8,
    pub(crate) _pad: u16,
    pub(crate) id: u64,
}

impl ObjHeader {
    /// Mã loại object (khớp `[object]` trong `spec/abi.toml`).
    #[must_use]
    pub fn kind(&self) -> u8 {
        self.kind
    }

    /// `STATE_LIVE` hoặc `STATE_FINALISING`.
    #[must_use]
    pub fn state(&self) -> u8 {
        self.state
    }

    /// `id` duy nhất, không tái dùng.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// Object kernel đặt trong arena qua [`crate::ObjStore::place`].
///
/// Chỉ cần `kind()` (object-safe để `[&mut dyn KObj; N]` của `with_many_mut` dùng được);
/// `'static` chặn object mượn dữ liệu ngắn hơn vòng đời của vùng Untyped.
///
/// `as_any_mut` (mặc định `None`) là đường **duy nhất** để closure của `with_many_mut`
/// ghi theo kiểu cụ thể qua `&mut dyn KObj` mà không phải đổi chữ ký thiết kế
/// `:207-208` (D2-48 giữ `ObjRef` 16 B). Object nào muốn nhận ghi kiểu cụ thể thì
/// override trả `Some(self)`; object không cần thì giữ mặc định (KT-14 vẫn hợp lệ).
pub trait KObj: 'static {
    /// Mã loại object; ghi vào [`ObjHeader`] lúc `place`.
    ///
    /// Bảng `kind → constructor` (D2-48) dùng giá trị này để tra vtable, nên trong một
    /// store **một `kind` chỉ được ứng với một kiểu** (điều kiện của `place`).
    fn kind(&self) -> u8;

    /// Truy cập kiểu cụ thể từ `&mut dyn KObj`; mặc định không hỗ trợ.
    ///
    /// Cần cho `with_many_mut`: closure chỉ nhận `[&mut dyn KObj; N]` (thiết kế
    /// `:207-208`) nên không thể gọi trực tiếp field của `T`. Trả `Some(self)` để
    /// cho phép `downcast_mut::<T>()`.
    fn as_any_mut(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}

/// Tham chiếu tới một object: `pa` + `id`; **không** mang quyền truy cập.
///
/// Layout theo thiết kế: `{ pa, id, PhantomData }`, 16 B. `T: ?Sized` để
/// `ObjRef<dyn KObj>` dùng được trong chữ ký `with_many_mut`; `id` không tái dùng nên
/// ref cũ không bao giờ trỏ nhầm object mới dùng lại `pa`.
#[repr(C)]
pub struct ObjRef<T: ?Sized> {
    pa: PAddr,
    id: u64,
    _t: PhantomData<T>,
}

impl<T: ?Sized> ObjRef<T> {
    /// Chỉ crate dựng được; người dùng nhận ref từ `place`.
    pub(crate) fn new(pa: PAddr, id: u64) -> Self {
        Self {
            pa,
            id,
            _t: PhantomData,
        }
    }

    /// Địa chỉ object (header) trong arena.
    #[must_use]
    pub fn pa(&self) -> PAddr {
        self.pa
    }

    /// `id` của object; khác header là `Dead` khi truy cập.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl<T: KObj> ObjRef<T> {
    /// Chuyển sang `ObjRef<dyn KObj>` — vẫn **16 B** (D2-48 giữ layout ref).
    ///
    /// Đây là đường duy nhất để người ngoài crate dựng `[ObjRef<dyn KObj>; N]` cho
    /// `with_many_mut` (constructor của `ObjRef` là `pub(crate)`); phép chuyển chỉ
    /// đổi kiểu tĩnh, **không** mang vtable — vtable do bảng `kind → constructor`
    /// của store tra lúc truy cập (D2-48).
    #[must_use]
    pub fn into_dyn(self) -> ObjRef<dyn KObj> {
        ObjRef {
            pa: self.pa,
            id: self.id,
            _t: PhantomData::<dyn KObj>,
        }
    }

    /// Bản mượn của [`ObjRef::into_dyn`].
    #[must_use]
    pub fn as_dyn(&self) -> ObjRef<dyn KObj> {
        self.into_dyn()
    }
}

impl<T: ?Sized> Copy for ObjRef<T> {}

impl<T: ?Sized> Clone for ObjRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> PartialEq for ObjRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.pa == other.pa && self.id == other.id
    }
}

impl<T: ?Sized> Eq for ObjRef<T> {}

impl<T: ?Sized> core::fmt::Debug for ObjRef<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ObjRef")
            .field("pa", &self.pa)
            .field("id", &self.id)
            .finish()
    }
}

/// Con trỏ tới header của object tại `pa`.
///
/// # Safety
/// `pa` phải trỏ tới object đã `place` (vùng nhớ còn hợp lệ).
pub(crate) unsafe fn header_ptr(pa: PAddr) -> *mut ObjHeader {
    pa as *mut ObjHeader
}

/// Con trỏ tới body `T` của object tại `pa`.
///
/// # Safety
/// `pa + BODY_OFFSET` phải đúng căn lề cho `T` và nằm trong vùng đã `place`.
pub(crate) unsafe fn body_ptr<T>(pa: PAddr) -> *mut T {
    // SAFETY: caller bảo đảm vùng nhớ hợp lệ và căn lề (xem `ObjStore::place`).
    unsafe { (pa as *mut u8).add(BODY_OFFSET).cast::<T>() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, needs_drop, offset_of, size_of};

    struct Dummy;

    impl KObj for Dummy {
        fn kind(&self) -> u8 {
            7
        }
    }

    #[test]
    fn header_dung_16_byte_khong_refs() {
        assert_eq!(size_of::<ObjHeader>(), 16);
        assert_eq!(align_of::<ObjHeader>(), align_of::<u64>());
        assert_eq!(offset_of!(ObjHeader, kind), 0);
        assert_eq!(offset_of!(ObjHeader, state), 1);
        assert_eq!(offset_of!(ObjHeader, _pad), 2);
        assert_eq!(offset_of!(ObjHeader, id), 8);
        assert!(!needs_drop::<ObjHeader>());
        assert_eq!(HEADER_SIZE, size_of::<ObjHeader>());
        assert_eq!(BODY_OFFSET, HEADER_SIZE);
    }

    #[test]
    fn objref_dung_16_byte_va_phantomdata_dyn() {
        assert_eq!(size_of::<ObjRef<Dummy>>(), 16);
        assert_eq!(size_of::<ObjRef<dyn KObj>>(), 16);

        let r = ObjRef::<Dummy>::new(0x1000, 9);
        let copy = r;
        assert_eq!(r.pa(), 0x1000);
        assert_eq!(r.id(), 9);
        assert_eq!(r, copy);
        assert_eq!(format!("{r:?}"), "ObjRef { pa: 4096, id: 9 }");
    }
}
