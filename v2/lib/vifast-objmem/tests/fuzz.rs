// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fuzz **tất định** chuỗi thao tác objmem (KT-15, K1A-PLAN §4 mục 4–5).
//!
//! Mỗi chuỗi: ~32 thao tác `place / destroy / with / with_mut / with_many_mut` trên một
//! `ObjMem` mới; RNG tự viết (xorshift64* — **không** thêm dependency, D2-21).
//! **Không** dùng `reset` trong fuzz: sau `Revoke`/`reset`, vùng bị cấp lại nên byte cũ
//! có thể *tình cờ* trông như header LIVE (bất biến "ref destroy ⇒ Dead" chỉ được bảo
//! đảm khi vùng chưa bị dùng lại — `Arena::reset` đã ghi rõ ref cũ vô hiệu).
//! Sau **mỗi** bước kiểm 4 bất biến:
//! 1. không có hai object **sống** chồng lấn vùng `[pa, pa + 16 + size_of::<T>())`;
//! 2. `arena.used()` khớp tổng block đã cấp (mô hình độc lập: `align <= 16` căn
//!    `max(align, 8)`, block `16 + size`; `align > 16` căn `align`, block `align + size`);
//! 3. mọi ref **sống** truy cập `Ok` và trả đúng giá trị mô hình;
//! 4. mọi ref **đã destroy** trả `Err(Dead)`.
//!
//! `with_many_mut` trên các ref sống rời nhau phải `Ok` và closure ghi được **từng**
//! object (mô hình tăng đúng 1); mảng có ref trùng/ref chết phải `Err(Alias)` và closure
//! **không** được gọi.
//!
//! Hai con số (không nới ngưỡng — K1A-PLAN §4/KT-15):
//! - stable: **10.000** chuỗi (DoD);
//! - dưới Miri: **200** chuỗi (bản rút gọn bắt buộc; xem `kt15-miri*.log`).
//!
//! Không `#[ignore]`, không `#[allow]`.

use core::any::Any;
use core::mem::{align_of, size_of};

use vifast_objmem::{Alias, Dead, KObj, ObjMem, ObjRef, ObjStore, HEADER_SIZE};

/// Số chuỗi của bản đầy đủ (stable) — DoD K1A-PLAN §4/KT-15 mục 4.
const CHUOI_STABLE: u64 = 10_000;
/// Số chuỗi của bản rút gọn dưới Miri — mục 5 (không giảm số stable).
const CHUOI_MIRI: u64 = 200;
/// Số thao tác mỗi chuỗi (~32 theo DoD).
const SO_THAO_TAC: usize = 32;
/// Số slot object tối đa (đủ để chạm trần `N = 8` của `with_many_mut`).
const MAX_OBJ: usize = 16;

/// RNG tất định tự viết — không dependency ngoài (D2-21).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // xorshift không nhận 0; trộn seed để chuỗi kề nhau khác nhau rõ rệt.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn tiep(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Số trong `0..n`.
    fn duoi(&mut self, n: u64) -> u64 {
        self.tiep() % n
    }
}

#[repr(align(64))]
struct Vung([u8; 32 * 1024]);

struct C8 {
    v: u64,
}

impl KObj for C8 {
    fn kind(&self) -> u8 {
        1
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

struct C16 {
    a: u64,
    b: u64,
}

impl KObj for C16 {
    fn kind(&self) -> u8 {
        2
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

#[repr(align(64))]
struct A64 {
    v: u64,
}

impl KObj for A64 {
    fn kind(&self) -> u8 {
        3
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

/// Object kích thước 0: block vẫn 16 B (chỉ header), kiểm nhánh `size_of::<T>() == 0`.
struct Z;

impl KObj for Z {
    fn kind(&self) -> u8 {
        4
    }
}

/// Ref cụ thể của một slot (giữ kiểu để gọi `with`/`with_mut`/`destroy` có kiểm tra).
#[derive(Clone, Copy, Debug)]
enum R {
    C8(ObjRef<C8>),
    C16(ObjRef<C16>),
    A64(ObjRef<A64>),
    Z(ObjRef<Z>),
}

impl R {
    fn dyn_ref(self) -> ObjRef<dyn KObj> {
        match self {
            R::C8(x) => x.into_dyn(),
            R::C16(x) => x.into_dyn(),
            R::A64(x) => x.into_dyn(),
            R::Z(x) => x.into_dyn(),
        }
    }

    fn pa(self) -> u64 {
        match self {
            R::C8(x) => x.pa(),
            R::C16(x) => x.pa(),
            R::A64(x) => x.pa(),
            R::Z(x) => x.pa(),
        }
    }

    /// Độ dài vùng object (header + body), không gồm padding căn lề trước header.
    fn vung_len(self) -> u64 {
        match self {
            R::C8(_) => (HEADER_SIZE + size_of::<C8>()) as u64,
            R::C16(_) => (HEADER_SIZE + size_of::<C16>()) as u64,
            R::A64(_) => (HEADER_SIZE + size_of::<A64>()) as u64,
            R::Z(_) => (HEADER_SIZE + size_of::<Z>()) as u64,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BanGhi {
    r: R,
    song: bool,
    val: u64,
}

fn doc(s: &ObjMem, r: R) -> Result<u64, Dead> {
    match r {
        R::C8(x) => s.with(x, |o| o.v),
        R::C16(x) => s.with(x, |o| o.a),
        R::A64(x) => s.with(x, |o| o.v),
        R::Z(x) => s.with(x, |o| {
            let _ = o.kind();
            0
        }),
    }
}

fn ghi(s: &mut ObjMem, r: R, v: u64) -> Result<(), Dead> {
    match r {
        R::C8(x) => s.with_mut(x, |o| o.v = v),
        R::C16(x) => s.with_mut(x, |o| {
            o.a = v;
            o.b = v.wrapping_mul(2);
        }),
        R::A64(x) => s.with_mut(x, |o| o.v = v),
        R::Z(x) => s.with_mut(x, |_| {}),
    }
}

/// Ghi dấu qua `&mut dyn KObj` (đường `as_any_mut`, không đụng field trực tiếp).
fn bum(x: &mut dyn KObj) {
    let any = x.as_any_mut().expect("object fuzz hỗ trợ as_any_mut");
    if let Some(o) = any.downcast_mut::<C8>() {
        o.v += 1;
    } else if let Some(o) = any.downcast_mut::<C16>() {
        o.a += 1;
        o.b += 2;
    } else if let Some(o) = any.downcast_mut::<A64>() {
        o.v += 1;
    } else {
        panic!("kind không hỗ trợ nhiều-object");
    }
}

fn ghi_nhieu<const N: usize>(s: &mut ObjMem, rs: [ObjRef<dyn KObj>; N]) -> Result<(), Alias> {
    s.with_many_mut(rs, |mut refs| {
        for x in refs.iter_mut() {
            bum(&mut **x);
        }
    })
}

/// Hai ref trùng/ref chết ⇒ `Alias`, closure không gọi.
fn mong_alias(s: &mut ObjMem, a: ObjRef<dyn KObj>, b: ObjRef<dyn KObj>) {
    let mut da_goi = false;
    let kq = s.with_many_mut([a, b], |_| da_goi = true);
    assert_eq!(kq, Err(Alias), "trùng/ref chết phải Alias");
    assert!(!da_goi, "closure không được gọi khi Alias");
}

fn lam_tron(x: usize, a: usize) -> usize {
    (x + a - 1) & !(a - 1)
}

/// Căn lề của `ObjHeader` (`#[repr(C)]` với `u64 id`) — mô hình dùng hằng này chứ
/// không gọi vào crate, để phép kiểm `arena.used()` là phép tính **độc lập**.
const HEADER_ALIGN: usize = 8;

/// Mô hình `arena.used()` sau khi cấp một object `(size, align)` — độc lập với `Arena`:
/// `align <= 16` ⇒ căn `max(align, HEADER_ALIGN)`, block `16 + size`; `align > 16` ⇒
/// căn `align`, block `align + size` (phần `align - 16` byte đầu là padding trước header).
fn cap_nhat_used(cursor: usize, size: usize, align: usize) -> usize {
    if align > HEADER_SIZE {
        lam_tron(cursor, align) + align + size
    } else {
        lam_tron(cursor, align.max(HEADER_ALIGN)) + HEADER_SIZE + size
    }
}

fn chong_lan(a: BanGhi, b: BanGhi) -> bool {
    let (a0, a1) = (a.r.pa(), a.r.pa() + a.r.vung_len());
    let (b0, b1) = (b.r.pa(), b.r.pa() + b.r.vung_len());
    a0 < b1 && b0 < a1
}

/// Bốn bất biến, kiểm sau **mỗi** bước.
fn kiem_bat_bien(
    s: &ObjMem,
    recs: &[Option<BanGhi>; MAX_OBJ],
    used: usize,
    seed: u64,
    buoc: usize,
) {
    assert_eq!(s.used(), used, "arena.used() phải khớp tổng block đã cấp");
    assert!(s.used() <= s.capacity(), "used vượt capacity");
    // 1) không object sống nào chồng lấn.
    for i in 0..MAX_OBJ {
        for j in (i + 1)..MAX_OBJ {
            if let (Some(a), Some(b)) = (recs[i], recs[j]) {
                if a.song && b.song {
                    assert!(!chong_lan(a, b), "object sống {i} và {j} chồng lấn");
                }
            }
        }
    }
    // 3) + 4) ref sống `Ok` đúng giá trị; ref destroy `Dead`.
    for (i, rec) in recs.iter().enumerate() {
        if let Some(b) = rec {
            if b.song {
                assert_eq!(
                    doc(s, b.r),
                    Ok(b.val),
                    "ref sống {i} phải Ok đúng giá trị ({b:?}, seed={seed}, buoc={buoc})"
                );
            } else {
                assert_eq!(
                    doc(s, b.r),
                    Err(Dead),
                    "ref đã destroy {i} phải Dead ({b:?}, seed={seed}, buoc={buoc}, used={})",
                    s.used()
                );
            }
        }
    }
}

fn chay_mot_chuoi(seed: u64) {
    let mut vung = Vung([0u8; 32 * 1024]);
    // SAFETY: vùng nhớ hợp lệ, chỉ dùng qua `ObjMem` trong chuỗi này.
    let mut s = unsafe { ObjMem::new(vung.0.as_mut_ptr(), vung.0.len()) };
    let mut rng = Rng::new(seed);
    let mut recs: [Option<BanGhi>; MAX_OBJ] = [None; MAX_OBJ];
    let mut used = 0usize;

    for _buoc in 0..SO_THAO_TAC {
        let slot_trong = recs.iter().position(Option::is_none);
        let op = rng.duoi(100);

        if op < 25 {
            // place: chọn kiểu theo RNG, cấp phát tới khi hết chỗ (None ⇒ bỏ qua).
            if let Some(i) = slot_trong {
                match rng.duoi(4) {
                    0 => {
                        if let Some(pa) = s.alloc::<C8>() {
                            // SAFETY: `pa` vừa cấp cho `C8`.
                            let r = unsafe { s.place(pa, C8 { v: 0 }) };
                            used = cap_nhat_used(used, size_of::<C8>(), align_of::<C8>());
                            recs[i] = Some(BanGhi {
                                r: R::C8(r),
                                song: true,
                                val: 0,
                            });
                        }
                    }
                    1 => {
                        if let Some(pa) = s.alloc::<C16>() {
                            // SAFETY: `pa` vừa cấp cho `C16`.
                            let r = unsafe { s.place(pa, C16 { a: 0, b: 0 }) };
                            used = cap_nhat_used(used, size_of::<C16>(), align_of::<C16>());
                            recs[i] = Some(BanGhi {
                                r: R::C16(r),
                                song: true,
                                val: 0,
                            });
                        }
                    }
                    2 => {
                        if let Some(pa) = s.alloc::<A64>() {
                            // SAFETY: `pa` vừa cấp cho `A64`.
                            let r = unsafe { s.place(pa, A64 { v: 0 }) };
                            used = cap_nhat_used(used, size_of::<A64>(), align_of::<A64>());
                            recs[i] = Some(BanGhi {
                                r: R::A64(r),
                                song: true,
                                val: 0,
                            });
                        }
                    }
                    _ => {
                        if let Some(pa) = s.alloc::<Z>() {
                            // SAFETY: `pa` vừa cấp cho `Z`.
                            let r = unsafe { s.place(pa, Z) };
                            used = cap_nhat_used(used, size_of::<Z>(), align_of::<Z>());
                            recs[i] = Some(BanGhi {
                                r: R::Z(r),
                                song: true,
                                val: 0,
                            });
                        }
                    }
                }
            }
        } else if op < 45 {
            // destroy: chọn một slot sống.
            let live: [usize; MAX_OBJ] = {
                let mut ds = [0usize; MAX_OBJ];
                let mut n = 0;
                for (i, r) in recs.iter().enumerate() {
                    if r.is_some_and(|b| b.song) {
                        ds[n] = i;
                        n += 1;
                    }
                }
                ds
            };
            let dem = recs.iter().filter(|r| r.is_some_and(|b| b.song)).count();
            if dem > 0 {
                let i = live[rng.duoi(dem as u64) as usize];
                let b = recs[i].expect("slot sống");
                match b.r {
                    // SAFETY: ref do `place` trả về, vùng còn hợp lệ.
                    R::C8(x) => unsafe { s.destroy(x) },
                    R::C16(x) => unsafe { s.destroy(x) },
                    R::A64(x) => unsafe { s.destroy(x) },
                    R::Z(x) => unsafe { s.destroy(x) },
                }
                recs[i] = Some(BanGhi { song: false, ..b });
            }
        } else if op < 60 {
            // with: đọc ref sống (đúng giá trị) hoặc ref chết (Dead).
            let i = rng.duoi(MAX_OBJ as u64) as usize;
            if let Some(b) = recs[i] {
                if b.song {
                    assert_eq!(doc(&s, b.r), Ok(b.val));
                } else {
                    assert_eq!(doc(&s, b.r), Err(Dead));
                }
            }
        } else if op < 75 {
            // with_mut: ghi giá trị mới rồi cập nhật mô hình.
            let live: [usize; MAX_OBJ] = {
                let mut ds = [0usize; MAX_OBJ];
                let mut n = 0;
                for (i, r) in recs.iter().enumerate() {
                    if r.is_some_and(|b| b.song) {
                        ds[n] = i;
                        n += 1;
                    }
                }
                ds
            };
            let dem = recs.iter().filter(|r| r.is_some_and(|b| b.song)).count();
            if dem > 0 {
                let i = live[rng.duoi(dem as u64) as usize];
                let b = recs[i].expect("slot sống");
                let v = rng.duoi(1_000);
                assert_eq!(ghi(&mut s, b.r, v), Ok(()));
                let val = if matches!(b.r, R::Z(_)) { 0 } else { v };
                recs[i] = Some(BanGhi { val, ..b });
            }
        } else if op < 95 {
            // with_many_mut: chọn k = 2..=4 object sống rời nhau (khác kiểu tuỳ RNG).
            let mut chon = [0usize; 4];
            let mut n = 0usize;
            let k = 2 + rng.duoi(3) as usize;
            let mut thu = 0;
            while n < k && thu < 64 {
                let i = rng.duoi(MAX_OBJ as u64) as usize;
                if let Some(b) = recs[i] {
                    if b.song && !matches!(b.r, R::Z(_)) && !chon[..n].contains(&i) {
                        chon[n] = i;
                        n += 1;
                    }
                }
                thu += 1;
            }
            if n == k {
                let ds: [ObjRef<dyn KObj>; 4] = [
                    recs[chon[0]].unwrap().r.dyn_ref(),
                    recs[chon[1]].unwrap().r.dyn_ref(),
                    if k > 2 {
                        recs[chon[2]].unwrap().r.dyn_ref()
                    } else {
                        recs[chon[0]].unwrap().r.dyn_ref()
                    },
                    if k > 3 {
                        recs[chon[3]].unwrap().r.dyn_ref()
                    } else {
                        recs[chon[0]].unwrap().r.dyn_ref()
                    },
                ];
                let kq = match k {
                    2 => ghi_nhieu(&mut s, [ds[0], ds[1]]),
                    3 => ghi_nhieu(&mut s, [ds[0], ds[1], ds[2]]),
                    _ => ghi_nhieu(&mut s, [ds[0], ds[1], ds[2], ds[3]]),
                };
                assert_eq!(kq, Ok(()), "ref rời nhau phải Ok");
                for &i in &chon[..k] {
                    let b = recs[i].unwrap();
                    recs[i] = Some(BanGhi {
                        val: b.val + 1,
                        ..b
                    });
                }
            }
        } else {
            // Nhánh fail-closed: mảng có ref trùng (cùng `pa`) hoặc ref đã destroy.
            let live: [usize; MAX_OBJ] = {
                let mut ds = [0usize; MAX_OBJ];
                let mut n = 0;
                for (i, r) in recs.iter().enumerate() {
                    if r.is_some_and(|b| b.song) {
                        ds[n] = i;
                        n += 1;
                    }
                }
                ds
            };
            let dem = recs.iter().filter(|r| r.is_some_and(|b| b.song)).count();
            let da_chet: [usize; MAX_OBJ] = {
                let mut ds = [0usize; MAX_OBJ];
                let mut n = 0;
                for (i, r) in recs.iter().enumerate() {
                    if r.is_some_and(|b| !b.song) {
                        ds[n] = i;
                        n += 1;
                    }
                }
                ds
            };
            let co_chet = recs.iter().filter(|r| r.is_some_and(|b| !b.song)).count();
            if dem >= 1 {
                let i = live[rng.duoi(dem as u64) as usize];
                let a = recs[i].unwrap().r.dyn_ref();
                if co_chet > 0 && rng.duoi(2) == 0 {
                    let j = da_chet[rng.duoi(co_chet as u64) as usize];
                    let b = recs[j].unwrap().r.dyn_ref();
                    mong_alias(&mut s, a, b);
                } else {
                    // Cùng ref ⇒ cùng `pa` ⇒ Alias.
                    mong_alias(&mut s, a, a);
                }
            }
        }

        kiem_bat_bien(&s, &recs, used, seed, _buoc);
    }
}

#[test]
fn fuzz_chuoi_thao_tac_tat_dinh() {
    let so_chuoi = if cfg!(miri) { CHUOI_MIRI } else { CHUOI_STABLE };
    const { assert!(CHUOI_MIRI < CHUOI_STABLE, "bản Miri phải là bản rút gọn") };
    for seed in 0..so_chuoi {
        chay_mot_chuoi(seed);
    }
}
