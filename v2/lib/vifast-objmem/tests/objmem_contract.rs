// SPDX-License-Identifier: MIT OR Apache-2.0
//! Contract test **công khai** của `vifast-objmem` (KT-14) — viết trước khi cài đặt.
//!
//! Bám DoD K1A-PLAN §4/KT-14: header 16 B, arena căn lề 8/16/64 + hết chỗ `None`,
//! `place`/`with`/`with_mut`/`destroy`, ref sau `destroy` → `Dead`, `id` không tái
//! dùng, destroy hai lần không UB/không drop lần hai, object căn lề 64.

use core::alloc::Layout;
use core::mem::{align_of, size_of};
use core::sync::atomic::{AtomicUsize, Ordering};

use vifast_objmem::{Arena, Dead, KObj, ObjMem, ObjRef, ObjStore, PAddr, BODY_OFFSET, HEADER_SIZE};

#[repr(align(64))]
struct Aligned([u8; 8192]);

impl Aligned {
    fn new() -> Self {
        Self([0u8; 8192])
    }
}

fn arena(buf: &mut Aligned) -> Arena {
    unsafe { Arena::new(buf.0.as_mut_ptr(), buf.0.len()) }
}

fn store(buf: &mut Aligned) -> ObjMem {
    unsafe { ObjMem::new(buf.0.as_mut_ptr(), buf.0.len()) }
}

struct Counter {
    n: u64,
}

impl KObj for Counter {
    fn kind(&self) -> u8 {
        42
    }
}

fn with_sig<S: ObjStore>(s: &S, r: ObjRef<Counter>) -> Result<u64, Dead> {
    s.with(r, |c| c.n)
}

fn with_mut_sig<S: ObjStore>(s: &mut S, r: ObjRef<Counter>) -> Result<u64, Dead> {
    s.with_mut(r, |c| c.n)
}

unsafe fn place_sig<S: ObjStore>(s: &mut S, pa: PAddr, v: Counter) -> ObjRef<Counter> {
    unsafe { s.place(pa, v) }
}

unsafe fn destroy_sig<S: ObjStore>(s: &mut S, r: ObjRef<Counter>) {
    unsafe { s.destroy(r) }
}

#[test]
fn header_16b_va_objref_16b() {
    assert_eq!(HEADER_SIZE, 16);
    assert_eq!(BODY_OFFSET, 16);
    assert_eq!(size_of::<vifast_objmem::ObjHeader>(), 16);
    assert_eq!(align_of::<vifast_objmem::ObjHeader>(), 8);
    assert_eq!(size_of::<ObjRef<Counter>>(), 16);
    assert_eq!(size_of::<ObjRef<dyn KObj>>(), 16);
}

#[test]
fn arena_can_le_8_16_64_va_het_cho_none() {
    let mut buf = Aligned::new();
    let mut a = arena(&mut buf);

    // Đẩy cursor lệch để kiểm căn lề thật, không chỉ block đầu.
    assert!(a.alloc(Layout::from_size_align(3, 1).unwrap()).is_some());
    assert!(a.alloc(Layout::from_size_align(5, 1).unwrap()).is_some());
    for align in [8usize, 16, 64] {
        let p = a
            .alloc(Layout::from_size_align(24, align).unwrap())
            .expect("còn chỗ");
        assert_eq!(p.as_ptr() as usize % align, 0, "align {align}");
    }

    // Hết chỗ: None, không panic.
    assert!(a
        .alloc(Layout::from_size_align(1_000_000, 8).unwrap())
        .is_none());
}

#[test]
fn arena_day_dung_64_byte_thi_tra_none() {
    let mut buf = Aligned::new();
    let mut a = unsafe { Arena::new(buf.0.as_mut_ptr(), 64) };
    assert_eq!(a.capacity(), 64);
    assert!(a.alloc(Layout::from_size_align(64, 8).unwrap()).is_some());
    assert!(a.alloc(Layout::from_size_align(1, 8).unwrap()).is_none());
}

#[test]
fn arena_used_va_reset() {
    let mut buf = Aligned::new();
    let mut a = arena(&mut buf);
    assert_eq!(a.used(), 0);
    assert_eq!(a.capacity(), buf.0.len());

    let p = a.alloc(Layout::from_size_align(32, 16).unwrap()).unwrap();
    assert!(a.used() >= 32);
    assert_eq!(a.used() % 16, 0);

    unsafe { a.reset() };
    assert_eq!(a.used(), 0);
    let q = a.alloc(Layout::from_size_align(32, 16).unwrap()).unwrap();
    assert_eq!(p.as_ptr(), q.as_ptr());
}

#[test]
fn chu_ky_objstore_dung_thiet_ke() {
    let mut buf = Aligned::new();
    let mut s = store(&mut buf);
    let pa = s.alloc::<Counter>().expect("còn chỗ");
    let r = unsafe { place_sig(&mut s, pa, Counter { n: 11 }) };
    assert_eq!(with_sig(&s, r), Ok(11));
    assert_eq!(with_mut_sig(&mut s, r), Ok(11));
    unsafe { destroy_sig(&mut s, r) };
    assert_eq!(with_sig(&s, r), Err(Dead));
}

#[test]
fn ref_sau_destroy_tra_dead_va_khong_goi_closure() {
    let mut buf = Aligned::new();
    let mut s = store(&mut buf);
    let pa = s.alloc::<Counter>().unwrap();
    let r = unsafe { s.place(pa, Counter { n: 5 }) };
    unsafe { s.destroy(r) };

    let mut called = false;
    let kq = s.with(r, |_| {
        called = true;
    });
    assert_eq!(kq, Err(Dead));
    assert!(!called);

    let mut called = false;
    let kq = s.with_mut(r, |_| {
        called = true;
    });
    assert_eq!(kq, Err(Dead));
    assert!(!called);
}

static DROPS: AtomicUsize = AtomicUsize::new(0);

struct DropOnce;

impl KObj for DropOnce {
    fn kind(&self) -> u8 {
        43
    }
}

impl Drop for DropOnce {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn destroy_drop_dung_mot_lan_hai_lan_khong_ub() {
    let mut buf = Aligned::new();
    let mut s = store(&mut buf);
    let pa = s.alloc::<DropOnce>().unwrap();
    let r = unsafe { s.place(pa, DropOnce) };

    unsafe { s.destroy(r) };
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(s.with(r, |_| 1u8), Err(Dead));

    // Lần hai: không UB, không drop lại.
    unsafe { s.destroy(r) };
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
}

#[test]
fn id_khong_tai_dung_ba_vong() {
    let mut buf = Aligned::new();
    let mut s = store(&mut buf);
    let mut ids = [0u64; 3];
    for (i, id) in ids.iter_mut().enumerate() {
        let pa = s.alloc::<Counter>().unwrap();
        let r = unsafe { s.place(pa, Counter { n: i as u64 }) };
        *id = r.id();
        unsafe { s.destroy(r) };
    }
    assert!(ids[0] < ids[1], "{ids:?}");
    assert!(ids[1] < ids[2], "{ids:?}");
    assert!(ids[0] != 0);
}

#[test]
fn ref_cu_sau_khi_pa_duoc_dung_lai_tra_dead() {
    let mut buf = Aligned::new();
    let mut s = store(&mut buf);
    let pa = s.alloc::<Counter>().unwrap();

    let old = unsafe { s.place(pa, Counter { n: 1 }) };
    unsafe { s.destroy(old) };
    let new = unsafe { s.place(pa, Counter { n: 2 }) };

    assert_eq!(s.with(old, |c| c.n), Err(Dead));
    assert_eq!(s.with(new, |c| c.n), Ok(2));
}

#[repr(align(64))]
struct Align64 {
    x: u64,
}

impl KObj for Align64 {
    fn kind(&self) -> u8 {
        44
    }
}

#[test]
fn object_can_le_64_body_can_le() {
    let mut buf = Aligned::new();
    let mut s = store(&mut buf);
    let pa = s.alloc::<Align64>().unwrap();
    let r = unsafe { s.place(pa, Align64 { x: 9 }) };
    assert_eq!((r.pa() as usize + BODY_OFFSET) % 64, 0);
    assert_eq!(s.with(r, |o| o.x), Ok(9));
}
