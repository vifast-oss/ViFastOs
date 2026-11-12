// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **alias từng cặp** của `with_many_mut` (KT-15, thiết kế `:207-208`, D2-48).
//!
//! Viết **trước** khi cài đặt (test đỏ trước): KT-14 để lại khung fail-closed luôn
//! `Err(Alias)`, nên các ca "N ref rời nhau ⇒ `Ok` + closure ghi được từng object"
//! đỏ cho tới khi có bảng tĩnh `kind → constructor` (D2-48).
//!
//! Phạm vi (DoD K1A-PLAN §4/KT-15):
//! 1. `N = 1..=8`, các ref **rời nhau** ⇒ `Ok`, closure ghi được **từng** object.
//! 2. Mọi kiểu trùng lặp trong `[ObjRef<dyn KObj>; N]` (cùng `pa`, lặp lại ref) ⇒
//!    `Err(Alias)` và closure **không** được gọi; object không bị đụng.
//! 3. Ba `kind` khác nhau trong một lần gọi ⇒ đúng bảng `kind → constructor`.
//! 4. Ref đã `destroy` trong mảng ⇒ fail-closed `Err(Alias)`, không truy cập.
//!
//! Ca dựng ref **giả** (cùng `pa` khác `id`, chồng vùng một phần) nằm trong unit test
//! của `src/many.rs` vì `ObjRef::new` là `pub(crate)` — ngoài crate không dựng được.

use core::any::Any;

use vifast_objmem::{Alias, KObj, ObjMem, ObjRef, ObjStore};

/// Vùng Untyped cho mỗi ca (đủ cho 8 object + căn lề 64).
#[repr(align(64))]
struct Vung([u8; 16 * 1024]);

impl Vung {
    fn moi() -> Self {
        Self([0u8; 16 * 1024])
    }
}

fn store(v: &mut Vung) -> ObjMem {
    // SAFETY: `v.0` là vùng nhớ hợp lệ, chỉ dùng qua `ObjMem` trong test này.
    unsafe { ObjMem::new(v.0.as_mut_ptr(), v.0.len()) }
}

struct Counter {
    n: u64,
}

impl KObj for Counter {
    fn kind(&self) -> u8 {
        7
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

struct Blob {
    a: u64,
    b: u64,
}

impl KObj for Blob {
    fn kind(&self) -> u8 {
        9
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

#[repr(align(64))]
struct Can64 {
    n: u64,
}

impl KObj for Can64 {
    fn kind(&self) -> u8 {
        11
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

fn place<T: KObj>(s: &mut ObjMem, v: T) -> ObjRef<T> {
    let pa = s.alloc::<T>().expect("vùng còn chỗ");
    // SAFETY: `pa` vừa cấp cho `T`, chưa có object sống.
    unsafe { s.place(pa, v) }
}

/// Ép `ObjRef<T>` (16 B) thành `ObjRef<dyn KObj>` (vẫn 16 B — D2-48 giữ layout).
fn dyn_ref<T: KObj>(r: ObjRef<T>) -> ObjRef<dyn KObj> {
    r.into_dyn()
}

/// Mảng `[ObjRef<dyn KObj>; N]` từ slice — const generic suy ra từ ngữ cảnh gọi.
fn mang<const N: usize>(src: &[ObjRef<dyn KObj>]) -> [ObjRef<dyn KObj>; N] {
    assert!(src.len() >= N, "cần ít nhất {N} ref");
    core::array::from_fn(|i| src[i])
}

/// Ghi dấu qua `&mut dyn KObj`: chỉ đường `as_any_mut` mới chạm được field cụ thể.
fn ghi(x: &mut dyn KObj) {
    let any = x.as_any_mut().expect("object test hỗ trợ as_any_mut");
    if let Some(c) = any.downcast_mut::<Counter>() {
        c.n += 1;
    } else if let Some(b) = any.downcast_mut::<Blob>() {
        b.a += 1;
        b.b += 10;
    } else if let Some(c) = any.downcast_mut::<Can64>() {
        c.n += 1;
    } else {
        panic!("kind lạ");
    }
}

/// Gọi `with_many_mut` với `N` là hằng của **từng nhánh** (không có cách biến N runtime).
fn goi<const N: usize>(s: &mut ObjMem, rs: &[ObjRef<dyn KObj>]) -> Result<usize, Alias> {
    s.with_many_mut(mang::<N>(rs), |mut refs| {
        for x in refs.iter_mut() {
            ghi(&mut **x);
        }
        refs.len()
    })
}

/// Dispatcher 1..=8: mỗi `N` gọi đúng nhánh const generic.
fn goi_n(s: &mut ObjMem, n: usize, rs: &[ObjRef<dyn KObj>]) -> Result<usize, Alias> {
    match n {
        1 => goi::<1>(s, rs),
        2 => goi::<2>(s, rs),
        3 => goi::<3>(s, rs),
        4 => goi::<4>(s, rs),
        5 => goi::<5>(s, rs),
        6 => goi::<6>(s, rs),
        7 => goi::<7>(s, rs),
        8 => goi::<8>(s, rs),
        _ => panic!("N = {n} ngoài 1..=8"),
    }
}

/// Như [`goi`] nhưng khẳng định `Alias` + closure **không** được gọi.
fn goi_mong_alias<const N: usize>(s: &mut ObjMem, rs: &[ObjRef<dyn KObj>]) {
    let mut da_goi = false;
    let kq = s.with_many_mut(mang::<N>(rs), |refs| {
        da_goi = true;
        refs.len()
    });
    assert_eq!(kq, Err(Alias), "N = {N}: phải Alias");
    assert!(!da_goi, "N = {N}: closure không được gọi khi có alias");
}

fn goi_n_mong_alias(s: &mut ObjMem, n: usize, rs: &[ObjRef<dyn KObj>]) {
    match n {
        1 => goi_mong_alias::<1>(s, rs),
        2 => goi_mong_alias::<2>(s, rs),
        3 => goi_mong_alias::<3>(s, rs),
        4 => goi_mong_alias::<4>(s, rs),
        5 => goi_mong_alias::<5>(s, rs),
        6 => goi_mong_alias::<6>(s, rs),
        7 => goi_mong_alias::<7>(s, rs),
        8 => goi_mong_alias::<8>(s, rs),
        _ => panic!("N = {n} ngoài 1..=8"),
    }
}

#[test]
fn n1_den_n8_ref_roi_nhau_tra_ok_va_ghi_duoc_tung_object() {
    for n in 1..=8usize {
        let mut v = Vung::moi();
        let mut s = store(&mut v);
        let mut cu: Vec<ObjRef<Counter>> = Vec::new();
        for _ in 0..n {
            cu.push(place(&mut s, Counter { n: 0 }));
        }
        let rs: Vec<ObjRef<dyn KObj>> = cu.iter().copied().map(dyn_ref).collect();

        assert_eq!(
            goi_n(&mut s, n, &rs),
            Ok(n),
            "N = {n}: các ref rời nhau ⇒ Ok"
        );
        for (i, r) in cu.iter().enumerate() {
            assert_eq!(
                s.with(*r, |c| c.n),
                Ok(1),
                "N = {n}: object {i} phải được ghi"
            );
        }
    }
}

#[test]
fn trung_pa_moi_n_tra_alias_va_khong_dung_object() {
    for n in 2..=8usize {
        let mut v = Vung::moi();
        let mut s = store(&mut v);
        let mut cu: Vec<ObjRef<Counter>> = Vec::new();
        for _ in 0..n {
            cu.push(place(&mut s, Counter { n: 0 }));
        }
        let mut rs: Vec<ObjRef<dyn KObj>> = cu.iter().copied().map(dyn_ref).collect();
        // Lặp ref cuối = ref đầu ⇒ cùng `pa` + cùng `id`.
        rs[n - 1] = rs[0];
        goi_n_mong_alias(&mut s, n, &rs);
        // Vị trí giữa mảng cũng phải bị bắt (kiểm từng cặp, không chỉ cặp kề).
        rs[n - 1] = rs[n / 2];
        goi_n_mong_alias(&mut s, n, &rs);
        for (i, r) in cu.iter().enumerate() {
            assert_eq!(
                s.with(*r, |c| c.n),
                Ok(0),
                "N = {n}: object {i} không được đụng"
            );
        }
    }
}

#[test]
fn moi_ref_deu_trung_nhau_tra_alias() {
    // N = 1 không có cặp nào để trùng (một ref luôn hợp lệ); ca trùng bắt đầu từ N = 2.
    for n in 2..=8usize {
        let mut v = Vung::moi();
        let mut s = store(&mut v);
        let a = place(&mut s, Counter { n: 0 });
        let da: ObjRef<dyn KObj> = dyn_ref(a);
        let rs = [da; 8];
        goi_n_mong_alias(&mut s, n, &rs[..n]);
        assert_eq!(s.with(a, |c| c.n), Ok(0));
    }
}

#[test]
fn ba_kind_khac_nhau_trong_mot_lan_goi_ok() {
    let mut v = Vung::moi();
    let mut s = store(&mut v);
    let c = place(&mut s, Counter { n: 0 });
    let b = place(&mut s, Blob { a: 0, b: 0 });
    let a = place(&mut s, Can64 { n: 0 });
    let rs = [dyn_ref(c), dyn_ref(b), dyn_ref(a)];

    let kq = s.with_many_mut(mang::<3>(&rs), |mut refs| {
        // Bảng `kind → constructor` phải dựng đúng vtable cho từng `kind`.
        assert_eq!(refs[0].kind(), 7);
        assert_eq!(refs[1].kind(), 9);
        assert_eq!(refs[2].kind(), 11);
        for x in refs.iter_mut() {
            ghi(&mut **x);
        }
    });
    assert_eq!(kq, Ok(()));
    assert_eq!(s.with(c, |x| x.n), Ok(1));
    assert_eq!(s.with(b, |x| (x.a, x.b)), Ok((1, 10)));
    assert_eq!(s.with(a, |x| x.n), Ok(1));
}

#[test]
fn ref_da_destroy_trong_mang_tra_alias_fail_closed() {
    let mut v = Vung::moi();
    let mut s = store(&mut v);
    let a = place(&mut s, Counter { n: 0 });
    let b = place(&mut s, Counter { n: 0 });
    // SAFETY: `a` do `place` trả về, vùng còn hợp lệ.
    unsafe { s.destroy(a) };
    let rs = [dyn_ref(a), dyn_ref(b)];
    goi_n_mong_alias(&mut s, 2, &rs);
    assert_eq!(s.with(b, |c| c.n), Ok(0), "object sống không được đụng");
}

#[test]
fn goi_lien_tiep_ghi_don_dung_tren_tung_object() {
    let mut v = Vung::moi();
    let mut s = store(&mut v);
    let a = place(&mut s, Counter { n: 0 });
    let b = place(&mut s, Counter { n: 0 });
    let rs = [dyn_ref(a), dyn_ref(b)];
    for lan in 1..=3u64 {
        assert_eq!(goi_n(&mut s, 2, &rs), Ok(2));
        assert_eq!(s.with(a, |c| c.n), Ok(lan));
        assert_eq!(s.with(b, |c| c.n), Ok(lan));
    }
}

/// Object kích thước 0 — kiểm nhánh `size_of::<T>() == 0` (block chỉ có header 16 B).
struct Rong;

impl KObj for Rong {
    fn kind(&self) -> u8 {
        13
    }
}

#[test]
fn zst_cung_object_thuong_trong_mot_lan_goi_ok() {
    let mut v = Vung::moi();
    let mut s = store(&mut v);
    let z = place(&mut s, Rong);
    let c = place(&mut s, Counter { n: 0 });
    let rs = [dyn_ref(z), dyn_ref(c)];

    let kq = s.with_many_mut(mang::<2>(&rs), |refs| {
        assert_eq!(refs[0].kind(), 13);
        assert_eq!(refs[1].kind(), 7);
        ghi(&mut *refs[1]);
    });
    assert_eq!(kq, Ok(()));
    assert_eq!(s.with(z, |_| ()), Ok(()), "ZST vẫn sống sau lần gọi");
    assert_eq!(s.with(c, |x| x.n), Ok(1));
}

#[test]
fn ba_kind_khac_nhau_ma_co_ref_trung_tra_alias() {
    let mut v = Vung::moi();
    let mut s = store(&mut v);
    let c = place(&mut s, Counter { n: 0 });
    let b = place(&mut s, Blob { a: 0, b: 0 });
    let a = place(&mut s, Can64 { n: 0 });

    // Lặp ref của Blob ở vị trí cuối: mảng trộn 3 kind vẫn phải bắt từng cặp.
    let rs = [dyn_ref(c), dyn_ref(b), dyn_ref(b)];
    goi_n_mong_alias(&mut s, 3, &rs);
    assert_eq!(s.with(c, |x| x.n), Ok(0));
    assert_eq!(s.with(b, |x| (x.a, x.b)), Ok((0, 0)));
    assert_eq!(s.with(a, |x| x.n), Ok(0));
}
