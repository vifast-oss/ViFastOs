// SPDX-License-Identifier: MIT OR Apache-2.0
//! Vùng nhớ căn lề **do objmem sở hữu** + API an toàn cho tầng sim (KT-16).
//!
//! Tầng trên (`vifast-sim`) không được có `unsafe` (R5/D2-17), nên mọi thao tác cần
//! con trỏ thô — `ObjMem::new`, `place`, `destroy`, `reset` — được gói ở đây. Hai kiểu:
//!
//! - [`OwnedArena`]: vùng căn lề + [`Arena`] trần, dùng cho bump allocator của sim
//!   (`Prog::Alloc`).
//! - [`OwnedMem`]: vùng căn lề + [`ObjMem`], dùng cho **object body** trong `kobj::Machine`.
//!
//! Vùng nhớ cấp bằng bộ cấp phát thô có căn lề của `alloc` (`alloc::alloc::alloc` với
//! `Layout` căn [`ARENA_ALIGN`]), **không** `Vec<u8>` (QD §3.5). Tầng trên chỉ thấy địa
//! chỉ số nguyên, không bao giờ thấy `&mut [u8]` của cả vùng.
//!
//! Bất biến: [`HeapBlock`] sở hữu vùng nhớ và chỉ giải phóng ở `Drop`; `Arena`/`ObjMem`
//! giữ con trỏ tới vùng **heap ổn định** đó nên di chuyển `OwnedArena`/`OwnedMem` không
//! làm con trỏ treo. Vùng không bao giờ được cấp lại cho tới khi `Drop`.

use core::alloc::Layout;
use core::ptr::NonNull;

use crate::arena::Arena;
use crate::obj::{KObj, ObjRef, PAddr, BODY_OFFSET};
use crate::store::{Alias, Dead, ObjMem, ObjStore};

/// Căn lề cơ sở của vùng do objmem cấp. Đủ cho mọi căn lề của mô hình sim
/// (`DEFAULT_ALIGN` 16, `CNODE_BASE_BYTES`/trang 4096).
pub const ARENA_ALIGN: usize = 4096;

/// Vùng nhớ thô căn [`ARENA_ALIGN`] do objmem sở hữu.
struct HeapBlock {
    ptr: NonNull<u8>,
    layout: Layout,
    len: usize,
}

impl HeapBlock {
    fn new(cap: usize) -> Option<Self> {
        // `Layout` không cho size 0 khi cấp; vùng "0 byte" vẫn cấp 1 byte nhưng `len` = 0.
        let size = cap.max(1);
        let layout = Layout::from_size_align(size, ARENA_ALIGN).ok()?;
        // SAFETY: `size > 0`; vùng được `Drop` giải phóng đúng `layout`.
        let ptr = unsafe { alloc::alloc::alloc(layout) };
        let ptr = NonNull::new(ptr)?;
        Some(Self {
            ptr,
            layout,
            len: cap,
        })
    }

    fn base(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for HeapBlock {
    fn drop(&mut self) {
        // SAFETY: `ptr` do `alloc` cấp với đúng `layout` này, chỉ giải phóng một lần.
        unsafe { alloc::alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}

impl core::fmt::Debug for HeapBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HeapBlock")
            .field("base", &(self.ptr.as_ptr() as usize))
            .field("len", &self.len)
            .finish()
    }
}

/// Bump allocator thô **an toàn** cho tầng sim: vùng nhớ do crate sở hữu.
pub struct OwnedArena {
    block: HeapBlock,
    arena: Arena,
}

impl OwnedArena {
    /// Cấp vùng `cap` byte (căn [`ARENA_ALIGN`]); `None` khi cấp phát thất bại.
    pub fn new(cap: usize) -> Option<Self> {
        let block = HeapBlock::new(cap)?;
        // SAFETY: `block` sở hữu `base..base + len`, không tham chiếu nào khác.
        let arena = unsafe { Arena::new(block.base(), block.len) };
        Some(Self { block, arena })
    }

    /// Địa chỉ (thật) của đáy vùng.
    #[must_use]
    pub fn base(&self) -> PAddr {
        self.block.base() as PAddr
    }

    /// Số byte đã cấp (gồm padding căn lề).
    #[must_use]
    pub fn used(&self) -> usize {
        self.arena.used()
    }

    /// Tổng số byte của vùng.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.arena.capacity()
    }

    /// Cấp `size` byte căn `align`; trả **offset** so với `base()` (hết chỗ ⇒ `None`).
    ///
    /// `align` ≤ [`ARENA_ALIGN`] (mọi căn lề của mô hình); lớn hơn trả `None` vì đáy vùng
    /// chỉ được bảo đảm căn `ARENA_ALIGN`.
    pub fn alloc_offset(&mut self, size: usize, align: usize) -> Option<usize> {
        if align == 0 || !align.is_power_of_two() || align > ARENA_ALIGN {
            return None;
        }
        let layout = Layout::from_size_align(size, align).ok()?;
        let ptr = self.arena.alloc(layout)?;
        Some(ptr.as_ptr() as usize - self.base() as usize)
    }
}

impl core::fmt::Debug for OwnedArena {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OwnedArena")
            .field("base", &self.base())
            .field("capacity", &self.capacity())
            .field("used", &self.used())
            .finish()
    }
}

/// Store object **an toàn** cho tầng sim: vùng nhớ + [`ObjMem`] do crate sở hữu.
///
/// `place`/`destroy`/`reset` (unsafe ở [`ObjStore`]/[`ObjMem`]) được gói thành API an
/// toàn: [`OwnedMem::alloc_place`]/[`OwnedMem::alloc_place_span`] cấp đúng layout rồi
/// đặt object, [`OwnedMem::free`] chỉ `destroy` khi ref nằm trong vùng của store này,
/// [`OwnedMem::reset`] chỉ dùng sau khi mọi object đã finalise (hợp đồng của tầng trên).
pub struct OwnedMem {
    block: HeapBlock,
    mem: ObjMem,
}

impl OwnedMem {
    /// Store trên vùng `cap` byte mới (id bắt đầu từ 1).
    pub fn new(cap: usize) -> Option<Self> {
        Self::with_id_base(cap, 0)
    }

    /// Như [`OwnedMem::new`] nhưng `id` của object đầu tiên là `id_base + 1`.
    ///
    /// Cần khi một `Machine` có nhiều store (mỗi Untyped một store): `id` phải **duy nhất
    /// toàn cụm**, nếu không ref của store này có thể trùng `id` ref của store khác
    /// (xem `crate::store::ObjRef` — ref không mang identity store).
    pub fn with_id_base(cap: usize, id_base: u64) -> Option<Self> {
        let block = HeapBlock::new(cap)?;
        // SAFETY: vùng do `block` sở hữu; `ObjMem::new_with_id` chỉ ghi khi `place`.
        let mem = unsafe { ObjMem::new_with_id(block.base(), block.len, id_base) };
        Some(Self { block, mem })
    }

    /// Địa chỉ (thật) của đáy vùng.
    #[must_use]
    pub fn base(&self) -> PAddr {
        self.block.base() as PAddr
    }

    /// Số byte đã cấp cho object (gồm header + padding căn lề).
    #[must_use]
    pub fn used(&self) -> usize {
        self.mem.used()
    }

    /// Tổng số byte của vùng.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.mem.capacity()
    }

    /// `pa` có nằm trong vùng của store này không (dùng để chặn ref store khác).
    #[must_use]
    pub fn owns(&self, pa: PAddr) -> bool {
        self.mem.chua_header(pa)
    }

    /// Cấp block theo layout của `T` rồi đặt `v`; trả ref sống.
    pub fn alloc_place<T: KObj>(&mut self, v: T) -> Option<ObjRef<T>> {
        let pa = self.mem.alloc::<T>()?;
        // SAFETY: `pa` vừa cấp cho đúng `T` từ arena của store này, chưa có object sống.
        Some(unsafe { self.mem.place(pa, v) })
    }

    /// Cấp **một block `size` byte căn `align`** (kích thước logic của object, ví dụ 512 B
    /// của TCB) rồi đặt `v` ở đầu block; trả `(ref, offset so với base)`.
    ///
    /// Dùng cho Untyped: watermark của mô hình chính là `Arena::used()` của vùng, nên
    /// `offset` trả về đúng bằng watermark cũ; `size` giữ nguyên cỡ logic của spec (không
    /// phải `size_of::<T>()`).
    ///
    /// **Hợp đồng căn lề (KT-17, follow-up KT-16)**: `align` hiệu dụng (sau khi lấy max với
    /// căn của body/header) phải `≤` [`ARENA_ALIGN`]. Đáy vùng chỉ được bảo đảm căn
    /// [`ARENA_ALIGN`] nên căn lớn hơn **không thể** bảo đảm địa chỉ tuyệt đối; khi đó trả
    /// `None` (fail-closed, không đốt byte) thay vì trả `pa` sai căn — vi phạm hợp đồng
    /// [`Layout`]. Đây là giới hạn của arena của mô hình, không phải thay đổi ABI.
    pub fn alloc_place_span<T: KObj>(
        &mut self,
        v: T,
        size: usize,
        align: usize,
    ) -> Option<(ObjRef<T>, usize)> {
        let align_t = core::mem::align_of::<T>();
        // Body nằm ở `pa + 16`; cần `align_t` chia hết `BODY_OFFSET` để đúng căn lề.
        if align_t > BODY_OFFSET || !BODY_OFFSET.is_multiple_of(align_t) {
            return None;
        }
        let can = BODY_OFFSET.checked_add(core::mem::size_of::<T>())?;
        let block = size.max(can);
        let align = align
            .max(align_t)
            .max(core::mem::align_of::<crate::ObjHeader>());
        if align > ARENA_ALIGN {
            return None;
        }
        let layout = Layout::from_size_align(block, align).ok()?;
        let pa = self.mem.alloc_block(layout)?;
        let offset = usize::try_from(pa.checked_sub(self.base())?).ok()?;
        // SAFETY: block vừa cấp từ arena của store này, đủ chỗ cho header + body và body
        // đúng căn lề (`align ≥ align_t`, `BODY_OFFSET % align_t == 0`).
        let r = unsafe { self.mem.place(pa, v) };
        Some((r, offset))
    }

    /// Finalise object nếu ref nằm trong vùng của store này; ref ngoài vùng là **no-op**
    /// (guard [`ObjMem::chua_header`], KT-15/KT-16).
    pub fn free<T: KObj>(&mut self, r: ObjRef<T>) {
        if self.mem.chua_header(r.pa()) {
            // SAFETY: `pa` nằm trong vùng của store này; `destroy` tự kiểm `id`/`state`
            // nên gọi hai lần hoặc ref cũ là no-op.
            unsafe { self.mem.destroy(r) }
        }
    }

    /// Đưa watermark về 0 sau khi **mọi** object trong vùng đã finalise (Untyped `Revoke`).
    ///
    /// Hợp đồng của tầng trên: chỉ gọi khi đã `free` mọi body trong vùng; mọi `ObjRef` cũ
    /// trở nên vô hiệu (`id` không tái dùng nên ref cũ không trỏ nhầm object mới).
    pub fn reset(&mut self) {
        // SAFETY: hợp đồng của hàm — mọi object trong vùng đã `destroy`.
        unsafe { self.mem.reset() }
    }
}

impl core::fmt::Debug for OwnedMem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OwnedMem")
            .field("base", &self.base())
            .field("capacity", &self.capacity())
            .field("used", &self.used())
            .finish()
    }
}

impl ObjStore for OwnedMem {
    fn with<T: KObj, R>(&self, r: ObjRef<T>, f: impl FnOnce(&T) -> R) -> Result<R, Dead> {
        self.mem.with(r, f)
    }

    fn with_mut<T: KObj, R>(
        &mut self,
        r: ObjRef<T>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<R, Dead> {
        self.mem.with_mut(r, f)
    }

    fn with_many_mut<const N: usize, R>(
        &mut self,
        rs: [ObjRef<dyn KObj>; N],
        f: impl FnOnce([&mut dyn KObj; N]) -> R,
    ) -> Result<R, Alias> {
        self.mem.with_many_mut(rs, f)
    }

    unsafe fn place<T: KObj>(&mut self, pa: PAddr, v: T) -> ObjRef<T> {
        // SAFETY: điều kiện của `place` do caller của impl này bảo đảm.
        unsafe { self.mem.place(pa, v) }
    }

    unsafe fn destroy<T: KObj>(&mut self, r: ObjRef<T>) {
        // SAFETY: điều kiện của `destroy` do caller bảo đảm.
        unsafe { self.mem.destroy(r) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    struct Dem {
        n: u64,
    }

    impl KObj for Dem {
        fn kind(&self) -> u8 {
            51
        }
    }

    #[test]
    fn owned_arena_va_mem_can_le_va_dung_lai_duoc_sau_reset() {
        let mut arena = OwnedArena::new(4096).expect("cấp vùng");
        assert_eq!(arena.capacity(), 4096);
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.base() % ARENA_ALIGN as u64, 0);
        let a = arena.alloc_offset(64, 16).expect("còn chỗ");
        let b = arena.alloc_offset(64, 64).expect("còn chỗ");
        assert_eq!((a, b), (0, 64));
        assert_eq!(arena.used(), 128);
        assert_eq!(arena.alloc_offset(8192, 16), None);

        let mut mem = OwnedMem::new(4096).expect("cấp vùng");
        let (r, off) = mem
            .alloc_place_span(Dem { n: 7 }, 512, 16)
            .expect("cấp object");
        assert_eq!(off, 0);
        assert_eq!(mem.used(), 512, "block giữ cỡ logic của spec");
        assert!(mem.owns(r.pa()));
        assert_eq!(mem.with(r, |d| d.n), Ok(7));
        assert_eq!(size_of::<Dem>(), 8);
        assert!(align_of::<Dem>() <= 16);
        mem.free(r);
        assert_eq!(mem.with(r, |d| d.n), Err(Dead));
        mem.reset();
        assert_eq!(mem.used(), 0);
        assert_eq!(mem.alloc_place(Dem { n: 9 }).map(|r| r.id()), Some(2));
    }

    #[test]
    fn id_base_tao_id_khong_trung_giua_hai_store() {
        let mut a = OwnedMem::with_id_base(1024, 0).expect("store A");
        let mut b = OwnedMem::with_id_base(1024, 1 << 40).expect("store B");
        let ra = a.alloc_place(Dem { n: 1 }).expect("A");
        let rb = b.alloc_place(Dem { n: 2 }).expect("B");
        assert_eq!(ra.id(), 1);
        assert_eq!(rb.id(), (1 << 40) + 1);
        assert_ne!(ra.id(), rb.id());
    }

    #[test]
    fn free_ref_cua_store_khac_la_no_op() {
        let mut a = OwnedMem::new(1024).expect("A");
        let mut b = OwnedMem::new(1024).expect("B");
        let ra = a.alloc_place(Dem { n: 3 }).expect("A");
        assert!(!b.owns(ra.pa()));
        b.free(ra);
        assert_eq!(a.with(ra, |d| d.n), Ok(3), "store A không bị đụng");
    }

    /// KT-17 (follow-up KT-16): đáy vùng chỉ căn `ARENA_ALIGN` ⇒ `align` lớn hơn phải
    /// **fail-closed** (`None`, không đốt byte), không được trả `pa` sai căn tuyệt đối.
    #[test]
    fn alloc_place_span_tu_choi_align_lon_hon_arena_align() {
        let mut mem = OwnedMem::new(1 << 16).expect("cấp vùng");
        assert!(mem.alloc_place_span(Dem { n: 1 }, 8192, 8192).is_none());
        assert_eq!(mem.used(), 0, "từ chối phải không đốt byte");

        // Biên đúng ARENA_ALIGN: được nhận, `pa` căn tuyệt đối (đáy vùng căn ARENA_ALIGN).
        let (r, off) = mem
            .alloc_place_span(Dem { n: 2 }, ARENA_ALIGN, ARENA_ALIGN)
            .expect("align = ARENA_ALIGN đúng biên");
        assert_eq!(off, 0);
        assert_eq!(r.pa() % ARENA_ALIGN as u64, 0);
        assert_eq!(mem.used(), ARENA_ALIGN);
        mem.free(r);
    }
}
