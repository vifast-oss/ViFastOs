// SPDX-License-Identifier: MIT OR Apache-2.0
//! `with_many_mut`: bảng tĩnh `kind → constructor` + kiểm alias **từng cặp** (KT-15, D2-48).
//!
//! Thiết kế `:207-208` cố định hai thứ mà KT-15 **không** được đổi:
//! - `ObjRef<dyn KObj>` vẫn **16 B** và **không** mang vtable (D2-48 chọn (a));
//! - closure nhận đúng `[&mut dyn KObj; N]`.
//!
//! Vì vậy trait object được dựng từ bảng cố định `kind → constructor`:
//! `place::<T>` đăng ký `ctor::<T>` (một hàm cụ thể cho mỗi `T`) vào ô `header.kind`.
//! Vtable do **compiler** sinh từ `impl KObj for T` khi `*mut T` được ép thành
//! `*mut dyn KObj` trong `ctor::<T>` — không có vtable viết tay, không đổi layout.
//! `header.kind` là nguồn duy nhất để tra hàm dựng: `kind` lạ hoặc `pa` ngoài arena ⇒
//! [`Alias`]. `ObjRef` 16 B **không** mang identity store, nên ref của store khác chỉ bị
//! chặn khi `pa` ngoài arena hoặc `id`/`kind`/`state` lệch — đó là **hợp đồng** của
//! `place`/`reset`, không phải bất biến cưỡng chế được (KT-16 thêm identity cụm).
//!
//! Thứ tự kiểm trong [`with_many_mut`] (mọi bước **trước** khi tạo tham chiếu):
//! 1. `const { assert!(N <= MAX_MANY) }` — trần `N ≤ 8` cưỡng chế **lúc biên dịch**
//!    (N = 9 không build; xem `tests/compile-fail/n9.rs`).
//! 2. `pa` phải là đầu header 16 B **trong** arena của store này (chặn `pa` giả/ngoài
//!    vùng trước khi đọc); ref sống: `header.id == r.id` và `state == STATE_LIVE`;
//!    `kind` phải có trong bảng.
//! 3. Từng cặp: trùng `pa` hoặc trùng `id`, hoặc hai vùng
//!    `[pa, pa + HEADER_SIZE + size_of::<T>())` chồng lấn ⇒ [`Alias`], closure **không**
//!    được gọi.
//! 4. Chỉ khi 1–3 xong mới dựng `[&mut dyn KObj; N]` từ con trỏ thô.
//!
//! Không `#[allow]`, không `#[ignore]`: mọi test alias chạy dưới Miri (D2-14).
//!
//! Ghi chú: vùng Untyped nằm **ngoài** `ObjMem` (người gọi cấp) và mọi truy cập đi qua
//! con trỏ thô dựng từ `pa`, nên đường nhiều object không cần `UnsafeCell`; `&mut self`
//! của `with_many_mut` đã loại trừ `with`/`with_mut` đồng thời. Ref không mang identity
//! store (KT-16 phải bảo đảm id duy nhất toàn cụm) — xem `lib.rs`.

use core::any::TypeId;
use core::mem::size_of;
use core::ptr;

use crate::obj::{body_ptr, header_ptr, KObj, ObjRef, PAddr, HEADER_SIZE, STATE_LIVE};
use crate::store::{Alias, ObjMem};

/// Số object tối đa của một lần `with_many_mut` (thiết kế `:208`; K1A-PLAN §4/KT-15).
pub const MAX_MANY: usize = 8;

/// Số ô của bảng `kind → constructor` (`header.kind: u8`).
pub const MAX_KINDS: usize = u8::MAX as usize + 1;

/// Hàm dựng `*mut dyn KObj` cho body của object đã `place` với đúng `T`.
///
/// Vtable do compiler sinh từ `impl KObj for T`; bảng chỉ giữ **một hàm cho mỗi kind**.
type Ctor = unsafe fn(PAddr) -> *mut dyn KObj;

/// `ctor::<T>`: `pa + BODY_OFFSET` → `*mut T` → `*mut dyn KObj`.
///
/// # Safety
/// `pa` phải là object **đang sống** đã `place::<T>` (bảng chỉ được tra theo
/// `header.kind` của chính object đó, và `place` là nơi duy nhất đăng ký kind).
unsafe fn ctor<T: KObj>(pa: PAddr) -> *mut dyn KObj {
    // SAFETY: xem điều kiện của hàm; `body_ptr` trả con trỏ tới body đã khởi tạo `T`.
    let p: *mut T = unsafe { body_ptr::<T>(pa) };
    // Ép `*mut T` → `*mut dyn KObj` (unsizing, compiler sinh vtable).
    p
}

/// Một ô bảng: hàm dựng + kích thước body (dùng cho kiểm chồng lấn từng cặp).
#[derive(Clone, Copy)]
pub(crate) struct Entry {
    ctor: Ctor,
    size: usize,
    /// Kiểu đã đăng ký — chỉ để `debug_assert` va chạm `kind` (không ảnh hưởng layout).
    type_id: TypeId,
}

/// Bảng cố định `kind → constructor`, `no_std`, không cấp phát.
pub(crate) struct KindTable {
    slots: [Option<Entry>; MAX_KINDS],
}

impl core::fmt::Debug for KindTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let da = self.slots.iter().flatten().count();
        f.debug_struct("KindTable")
            .field("slots", &MAX_KINDS)
            .field("da_dang_ky", &da)
            .finish()
    }
}

impl KindTable {
    pub(crate) const fn new() -> Self {
        Self {
            slots: [None; MAX_KINDS],
        }
    }

    /// Đăng ký `ctor::<T>` cho `kind` (gọi từ `place::<T>`, `kind` = `v.kind()`).
    ///
    /// Hai kiểu khác nhau dùng **cùng** `kind` là lỗi người dùng: ô bị ghi đè bởi lần
    /// `place` sau. `header.kind` vẫn là nguồn tra duy nhất nên không có đường đoán.
    pub(crate) fn register<T: KObj>(&mut self, kind: u8) {
        let type_id = TypeId::of::<T>();
        if let Some(cu) = self.slots[kind as usize] {
            // Hai kiểu khác nhau dùng chung `kind` là vi phạm hợp đồng `place`: tra bảng
            // sẽ dựng sai vtable cho một trong hai. Chỉ panic ở build debug.
            debug_assert!(
                cu.type_id == type_id,
                "vifast-objmem: hai kiểu khác nhau dùng chung kind {kind} (hợp đồng place)"
            );
        }
        self.slots[kind as usize] = Some(Entry {
            ctor: ctor::<T>,
            size: size_of::<T>(),
            type_id,
        });
    }

    fn get(&self, kind: u8) -> Option<Entry> {
        self.slots[kind as usize]
    }
}

/// Vùng byte mà một object sống chiếm: `[pa, pa + HEADER_SIZE + size_of::<T>())`.
///
/// `len` tính cả header; vùng **không** gồm padding căn lề trước header (phần đó không
/// thuộc object). Object `size_of::<T>() == 0` vẫn có vùng 16 B của header.
#[derive(Clone, Copy)]
struct Vung {
    pa: PAddr,
    len: u64,
}

impl Vung {
    fn cua(pa: PAddr, size: usize) -> Option<Self> {
        let len = (HEADER_SIZE as u64).checked_add(size as u64)?;
        Some(Self { pa, len })
    }
}

/// Hai vùng có giao nhau không; `None` khi cộng tràn `u64` (coi như không xác định).
fn chong_lan(a: Vung, b: Vung) -> Option<bool> {
    let a_end = a.pa.checked_add(a.len)?;
    let b_end = b.pa.checked_add(b.len)?;
    Some(a.pa < b_end && b.pa < a_end)
}

/// Kiểm alias từng cặp trên `(ref, vùng)` đã tra bảng — **thuần**, không truy cập bộ nhớ.
fn kiem_alias<const N: usize>(rs: &[ObjRef<dyn KObj>; N], vung: &[Vung; N]) -> Result<(), Alias> {
    for i in 0..N {
        for j in (i + 1)..N {
            // Cùng `pa` (kể cả khác `id`) hoặc cùng `id` (khác `pa`) ⇒ không thể là hai
            // object sống phân biệt (id duy nhất, không tái dùng).
            if rs[i].pa() == rs[j].pa() || rs[i].id() == rs[j].id() {
                return Err(Alias);
            }
            match chong_lan(vung[i], vung[j]) {
                Some(false) => {}
                // Chồng lấn, hoặc tràn số ⇒ fail-closed, giống nhau về kết quả.
                _ => return Err(Alias),
            }
        }
    }
    Ok(())
}

/// Cài đặt `ObjStore::with_many_mut` cho [`ObjMem`] (KT-15, D2-48).
///
/// # Panics
/// Không panic trong đường chạy bình thường; `N > 8` là **lỗi biên dịch**.
pub(crate) fn with_many_mut<const N: usize, R>(
    mem: &ObjMem,
    rs: [ObjRef<dyn KObj>; N],
    f: impl FnOnce([&mut dyn KObj; N]) -> R,
) -> Result<R, Alias> {
    const { assert!(N <= MAX_MANY, "with_many_mut: N <= 8") };

    let mut ctors: [Option<Ctor>; N] = [None; N];
    let mut vung: [Vung; N] = [Vung { pa: 0, len: 0 }; N];

    // Bước 2: ref phải sống và kind phải có trong bảng. Chỉ đọc header 16 B, chưa tạo
    // tham chiếu tới body.
    for (i, r) in rs.iter().enumerate() {
        // Chặn `pa` giả/ngoài arena **trước** khi đọc bộ nhớ (KT-15).
        if !mem.chua_header(r.pa()) {
            return Err(Alias);
        }
        // SAFETY: `r` do `place` trả về (hợp đồng store); header nằm ở `pa` trong arena,
        // còn hợp lệ.
        let header = unsafe { ptr::read(header_ptr(r.pa())) };
        if header.id() != r.id() || header.state() != STATE_LIVE {
            return Err(Alias);
        }
        let entry = mem.bang().get(header.kind()).ok_or(Alias)?;
        let v = Vung::cua(r.pa(), entry.size).ok_or(Alias)?;
        ctors[i] = Some(entry.ctor);
        vung[i] = v;
    }

    // Bước 3: từng cặp, trước khi dựng bất kỳ `&mut` nào.
    kiem_alias(&rs, &vung)?;

    // Bước 4: dựng `[&mut dyn KObj; N]`. Bất biến: header LIVE + đúng `id` + kind khớp
    // `ctor` (đăng ký ở `place`), và các vùng đôi một rời nhau ⇒ không có hai `&mut`
    // trỏ cùng byte.
    let refs: [&mut dyn KObj; N] = core::array::from_fn(|i| {
        let c = match ctors[i] {
            Some(c) => c,
            // Không xảy ra: mọi `ctors[i]` đã gán ở bước 2.
            None => unreachable!(),
        };
        // SAFETY: xem bất biến ngay trên; `ctor::<T>` chỉ đòi `pa` của object `T` sống.
        unsafe { &mut *c(rs[i].pa()) }
    });

    Ok(f(refs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ObjStore;
    use core::any::Any;

    struct Dem {
        n: u64,
    }

    impl KObj for Dem {
        fn kind(&self) -> u8 {
            21
        }

        fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
            Some(self)
        }
    }

    struct Khac {
        n: u64,
    }

    impl KObj for Khac {
        fn kind(&self) -> u8 {
            22
        }

        fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
            Some(self)
        }
    }

    #[repr(align(64))]
    struct Buf([u8; 8192]);

    fn store(buf: &mut Buf) -> ObjMem {
        // SAFETY: `buf.0` hợp lệ, chỉ dùng qua store trong test này.
        unsafe { ObjMem::new(buf.0.as_mut_ptr(), buf.0.len()) }
    }

    fn place<T: KObj>(s: &mut ObjMem, v: T) -> ObjRef<T> {
        let pa = s.alloc::<T>().expect("còn chỗ");
        // SAFETY: `pa` vừa cấp cho `T`.
        unsafe { s.place(pa, v) }
    }

    #[test]
    fn trung_pa_khac_id_tra_alias_va_khong_dung_object() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let a = place(&mut s, Dem { n: 1 });
        // Ref giả: cùng `pa`, `id` lệch 1 — chỉ trong crate mới dựng được.
        let gia = ObjRef::<dyn KObj>::new(a.pa(), a.id() + 1);
        let mut da_goi = false;
        let kq = s.with_many_mut([a.into_dyn(), gia], |mut rs| {
            da_goi = true;
            for x in rs.iter_mut() {
                if let Some(any) = x.as_any_mut() {
                    if let Some(d) = any.downcast_mut::<Dem>() {
                        d.n = 99;
                    }
                }
            }
        });
        assert_eq!(kq, Err(Alias));
        assert!(!da_goi, "closure không được gọi khi có alias");
        assert_eq!(s.with(a, |d| d.n), Ok(1), "object sống không được đụng");
    }

    /// Body 16 B có **layout y hệt `ObjHeader`** — để dựng `pa + 16` giả một header
    /// `{kind đã đăng ký, LIVE, id = X}` và buộc nhánh **chồng vùng** (không phải kiểm
    /// sống/kind) phải bắt.
    #[repr(C)]
    struct Gia {
        kind: u8,
        state: u8,
        _pad: u16,
        id: u64,
    }

    impl KObj for Gia {
        fn kind(&self) -> u8 {
            33
        }

        fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
            Some(self)
        }
    }

    #[test]
    fn ref_lech_vao_trong_header_tra_alias_fail_closed() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let a = place(&mut s, Dem { n: 1 });
        // `pa + 8` rơi vào header của `a`: byte "kind" đọc được là byte của `_pad`/`id`,
        // nên **có thể** tình cờ là kind đã đăng ký; id/state thì thường lệch. Test chỉ
        // khẳng định fail-closed (`Alias`, không gọi closure), **không** khẳng định nó
        // chết ở nhánh nào. Nhánh chồng vùng thuần có test end-to-end ngay dưới.
        let gia = ObjRef::<dyn KObj>::new(a.pa() + 8, a.id());
        let mut da_goi = false;
        let kq = s.with_many_mut([a.into_dyn(), gia], |_| da_goi = true);
        assert_eq!(kq, Err(Alias));
        assert!(!da_goi);
        assert_eq!(s.with(a, |d| d.n), Ok(1));
    }

    #[test]
    fn chong_vung_that_su_bat_boi_kiem_tung_cap() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        // Body của `Gia` được đặt ở `pa + 16` với bytes đúng `{kind: 33, state: LIVE,
        // pad: 0, id: 777}` ⇒ ref giả `(pa + 16, 777)` **qua được** kiểm sống + tra bảng
        // (kind 33 do `place::<Gia>` đăng ký), nên chỉ còn `kiem_alias` chặn được.
        let a = place(
            &mut s,
            Gia {
                kind: 33,
                state: STATE_LIVE,
                _pad: 0,
                id: 777,
            },
        );
        let gia = ObjRef::<dyn KObj>::new(a.pa() + HEADER_SIZE as u64, 777);
        let mut da_goi = false;
        let kq = s.with_many_mut([a.into_dyn(), gia], |_| da_goi = true);
        assert_eq!(
            kq,
            Err(Alias),
            "hai vùng [pa, pa+32) và [pa+16, pa+48) chồng nhau ⇒ Alias"
        );
        assert!(!da_goi, "closure không được gọi khi chồng vùng");
        assert_eq!(s.with(a, |g| g.id), Ok(777));
    }

    #[test]
    fn chong_lan_tung_cap_vung() {
        let v = |pa, size| Vung::cua(pa, size).expect("không tràn");
        // [100, 116) và [108, 124): chồng một phần.
        assert_eq!(chong_lan(v(100, 0), v(108, 0)), Some(true));
        // Kề nhau: [100, 116) và [116, 132): không chồng.
        assert_eq!(chong_lan(v(100, 0), v(116, 0)), Some(false));
        // Lồng nhau hoàn toàn.
        assert_eq!(chong_lan(v(100, 64), v(104, 0)), Some(true));
        // Cách xa.
        assert_eq!(chong_lan(v(0, 8), v(4096, 8)), Some(false));
        // Tràn `u64` ⇒ `None` (caller coi là fail-closed).
        assert_eq!(
            chong_lan(
                Vung {
                    pa: u64::MAX - 1,
                    len: 8
                },
                v(0, 0)
            ),
            None
        );
        // Bảng: kind đã đăng ký có kích thước đúng, kind lạ không có.
        let mut t = KindTable::new();
        t.register::<Dem>(21);
        t.register::<Khac>(22);
        assert_eq!(t.get(21).map(|e| e.size), Some(size_of::<Dem>()));
        assert_eq!(t.get(22).map(|e| e.size), Some(size_of::<Khac>()));
        assert!(t.get(23).is_none());
    }

    #[test]
    fn kiem_alias_bat_ca_cap_khong_ke_nhau() {
        // 3 vùng: [0,16) [16,32) [24,40) ⇒ cặp (0, 2) chồng nhưng (0,1) và (1,2)? (1,2) chồng.
        let rs = [
            ObjRef::<dyn KObj>::new(0, 1),
            ObjRef::<dyn KObj>::new(16, 2),
            ObjRef::<dyn KObj>::new(24, 3),
        ];
        let vung = [
            Vung::cua(0, 0).unwrap(),
            Vung::cua(16, 0).unwrap(),
            Vung::cua(24, 0).unwrap(),
        ];
        assert_eq!(kiem_alias(&rs, &vung), Err(Alias));
        // Chỉ cặp (0, 2) chồng nếu đổi vùng 3 ra xa: [0,16) [16,32) [64,80).
        let vung2 = [
            Vung::cua(0, 0).unwrap(),
            Vung::cua(16, 0).unwrap(),
            Vung::cua(64, 0).unwrap(),
        ];
        let rs2 = [
            ObjRef::<dyn KObj>::new(0, 1),
            ObjRef::<dyn KObj>::new(16, 2),
            ObjRef::<dyn KObj>::new(64, 3),
        ];
        assert_eq!(kiem_alias(&rs2, &vung2), Ok(()));
        // Cùng `id`, khác `pa`, vùng rời nhau ⇒ vẫn `Alias` (`id` không tái dùng).
        let rs3 = [
            ObjRef::<dyn KObj>::new(0, 9),
            ObjRef::<dyn KObj>::new(64, 9),
        ];
        let vung3 = [Vung::cua(0, 0).unwrap(), Vung::cua(64, 0).unwrap()];
        assert_eq!(kiem_alias(&rs3, &vung3), Err(Alias));
    }

    #[test]
    fn ref_cua_store_khac_bi_chan_boi_guard_arena() {
        let mut buf_a = Buf([0u8; 8192]);
        let mut buf_b = Buf([0u8; 8192]);
        let mut a = store(&mut buf_a);
        let mut b = store(&mut buf_b);
        let r = place(&mut a, Dem { n: 1 });
        // `ObjRef` 16 B không mang identity store; guard `chua_header` chặn vì `pa` ngoài
        // arena của `b` (nếu hai arena chồng nhau thì đây là vi phạm hợp đồng `ObjMem::new`).
        let mut da_goi = false;
        assert_eq!(
            b.with_many_mut([r.into_dyn()], |_| da_goi = true),
            Err(Alias)
        );
        assert!(!da_goi, "closure không được gọi");
        assert_eq!(a.with(r, |d| d.n), Ok(1), "store A không bị đụng");
    }

    #[test]
    fn hai_kind_khac_nhau_ok_va_ghi_duoc_tung_object() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let a = place(&mut s, Dem { n: 0 });
        let b = place(&mut s, Khac { n: 0 });
        let kq = s.with_many_mut([a.into_dyn(), b.into_dyn()], |mut rs| {
            for x in rs.iter_mut() {
                let any = x.as_any_mut().expect("as_any_mut");
                if let Some(d) = any.downcast_mut::<Dem>() {
                    d.n += 1;
                } else if let Some(k) = any.downcast_mut::<Khac>() {
                    k.n += 1;
                }
            }
        });
        assert_eq!(kq, Ok(()));
        assert_eq!(s.with(a, |d| d.n), Ok(1));
        assert_eq!(s.with(b, |k| k.n), Ok(1));
    }

    /// KT-16: bảng `kind → ctor` giữ nguyên cấu trúc, **không phình quá 8 KiB**.
    /// 256 ô × `Entry { ctor, size, type_id }` = 8.192 B đúng trần.
    #[test]
    fn bang_kind_giu_nguyen_va_khong_phinh_qua_8_kib() {
        let n = core::mem::size_of::<KindTable>();
        assert!(
            n <= 8 * 1024,
            "bảng kind→ctor phình: {n} B > 8 KiB (256 ô × {} B)",
            core::mem::size_of::<Option<Entry>>()
        );
        assert_eq!(n, MAX_KINDS * core::mem::size_of::<Option<Entry>>());
        // Đo được trên 64-bit: 256 ô × 32 B = 8.192 B = trần 8 KiB (không phình).
        println!(
            "# KT-16: bảng kind→ctor = {n} B ({} B/ô)",
            core::mem::size_of::<Option<Entry>>()
        );
    }

    #[test]
    fn khong_ref_nao_thi_ok_rong() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        assert_eq!(s.with_many_mut([], |rs| rs.len()), Ok(0));
    }

    #[test]
    fn pa_dung_lai_sau_reset_van_bat_duoc_ref_cu() {
        let mut buf = Buf([0u8; 8192]);
        let mut s = store(&mut buf);
        let a = place(&mut s, Dem { n: 1 });
        let pa_a = a.pa();
        // SAFETY: `a` do `place` trả về và đã destroy trước khi `reset`.
        unsafe {
            s.destroy(a);
            s.reset();
        }
        let b = place(&mut s, Dem { n: 2 });
        assert_eq!(b.pa(), pa_a, "sau reset, arena cấp lại đúng `pa` cũ");
        assert_ne!(a.id(), b.id(), "id không tái dùng");
        // Ref cũ trỏ đúng `pa` nhưng sai `id` ⇒ fail-closed, closure không gọi.
        let mut da_goi = false;
        assert_eq!(
            s.with_many_mut([a.into_dyn(), b.into_dyn()], |_| da_goi = true),
            Err(Alias)
        );
        assert!(!da_goi);
        assert_eq!(s.with_many_mut([b.into_dyn()], |rs| rs.len()), Ok(1));
        assert_eq!(s.with(b, |d| d.n), Ok(2));
    }
}
