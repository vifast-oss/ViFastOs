// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-58 (VD-94/VD-99)** — `Ty::Any` **xoá hẳn**: chữ ký `std.*` dùng **biến kiểu** (`Var`/`List`/`Map`).
//!
//! Khoá ba việc:
//! 1. **Không còn** `Ty::Any` — *bảo đảm ở mức biên dịch* (biến thể đã xoá khỏi `enum Ty`; test này chỉ
//!    còn khẳng định **mọi** chỗ trong bảng đều hạ được ra `Type` và `Var` chỉ dùng `0..3`).
//! 2. 16 chữ ký đã đổi có **đúng hình dạng** (`list[T]`, `map[K,V]`, trả `T`/`list[K]`/…).
//! 3. `sang_type()` **tổng** (không `Option`): `Var(i)` → `TypeRef` tên `T`/`K`/`V`.

use vis_types::stdlib::{sig_of, Ty, FUNCS};
use vis_types::Type;

/// Tên biến kiểu theo chỉ số (VD-99 (1): `0=T, 1=K, 2=V`).
const TEN_BIEN: [&str; 3] = ["T", "K", "V"];

fn kiem_var(t: Ty) {
    if let Ty::Var(i) = t {
        assert!((i as usize) < TEN_BIEN.len(), "Ty::Var({i}) ngoài 0..3");
    }
    match t {
        Ty::List(x) => kiem_var(*x),
        Ty::Map(k, v) => {
            kiem_var(*k);
            kiem_var(*v);
        }
        _ => {}
    }
}

#[test]
fn moi_chu_ky_ha_duoc_va_bien_kieu_trong_0_3() {
    for f in &FUNCS {
        for p in f.params {
            kiem_var(*p);
            // `FnKhoa` là **ngoại lệ có ghi** ⇒ hạ thành `{error}` (tầng chạy kiểm, ST-56);
            // còn lại **mọi** tham số phải hạ được (tổng).
            if matches!(p, Ty::FnKhoa) {
                assert_eq!(p.sang_type(), Type::Error);
                continue;
            }
            assert_ne!(
                p.sang_type(),
                Type::Error,
                "{}: tham số không hạ được",
                f.name
            );
        }
        kiem_var(f.ret);
        assert_ne!(
            f.ret.sang_type(),
            Type::Unit,
            "{}: ret hạ thành unit",
            f.name
        );
    }
    // **SỬA KÈM LÝ DO (ST-63, VD-105)**: +5 hàm `std.net.*` (connect, connect_timeout, read, write,
    // close) ⇒ **43** hàm; `MODULES` 7 → **8** (thêm `net`). Không xoá/đổi hàm cũ nào.
    // **SỬA KÈM LÝ DO (ST-64)**: +2 hàm `std.http.*` (`http.get`, `http.get_timeout`) ⇒ **45** hàm;
    // `MODULES` 8 → **9** (thêm `http`). Bỏ `http.body_len` — không cần cho ca cap (xem STATE ST-64).
    assert_eq!(FUNCS.len(), 45, "bảng chữ ký phải đủ 45 hàm");
}

#[test]
fn sang_type_dung_ten_bien_kieu() {
    assert_eq!(
        Ty::Var(0).sang_type(),
        Type::TypeRef {
            name: "T".into(),
            args: Vec::new()
        }
    );
    assert_eq!(
        Ty::List(&Ty::Var(1)).sang_type(),
        Type::List(Box::new(Type::TypeRef {
            name: "K".into(),
            args: Vec::new()
        }))
    );
    // `list[list[T]]` — promotion trong `const FUNCS` đã chứng minh; đây khoá **hình dạng**.
    assert_eq!(
        Ty::List(&Ty::List(&Ty::Var(0))).sang_type(),
        Type::List(Box::new(Type::List(Box::new(Type::TypeRef {
            name: "T".into(),
            args: Vec::new()
        }))))
    );
    assert_eq!(
        Ty::Map(&Ty::Var(1), &Ty::Var(2)).sang_type(),
        Type::Map(
            Box::new(Type::TypeRef {
                name: "K".into(),
                args: Vec::new()
            }),
            Box::new(Type::TypeRef {
                name: "V".into(),
                args: Vec::new()
            })
        )
    );
}

/// 16/38 chữ ký đổi (VD-99 (4)): `std.list` 8 + `std.map` 5 + `std.json` 3.
#[test]
fn muoi_sau_chu_ky_generic_dung_hinh_dang() {
    let t = Ty::Var(0);
    let list_t = Ty::List(&Ty::Var(0));
    let k = Ty::Var(1);
    let v = Ty::Var(2);
    let map_kv = Ty::Map(&Ty::Var(1), &Ty::Var(2));
    let ca: [(&str, &[Ty], Ty); 16] = [
        ("list.sort", &[list_t], list_t),
        ("list.sort_by", &[list_t, Ty::FnKhoa], list_t),
        ("list.reverse", &[list_t], list_t),
        ("list.unique", &[list_t], list_t),
        ("list.concat", &[list_t, list_t], list_t),
        (
            "list.chunk",
            &[list_t, Ty::Int],
            Ty::List(&Ty::List(&Ty::Var(0))),
        ),
        ("list.take", &[list_t, Ty::Int], list_t),
        ("list.drop", &[list_t, Ty::Int], list_t),
        ("map.keys", &[map_kv], Ty::List(&Ty::Var(1))),
        ("map.values", &[map_kv], Ty::List(&Ty::Var(2))),
        ("map.merge", &[map_kv, map_kv], map_kv),
        ("map.get_or", &[map_kv, Ty::Str, v], v),
        ("map.remove", &[map_kv, Ty::Str], map_kv),
        ("json.encode", &[t], Ty::Str),
        ("json.encode_pretty", &[t], Ty::Str),
        ("json.decode", &[Ty::Str], Ty::Var(2)),
    ];
    for (ten, params, ret) in ca {
        let f = sig_of(ten).unwrap_or_else(|| panic!("thiếu chữ ký `{ten}`"));
        assert_eq!(f.params, params, "`{ten}`: tham số sai hình dạng");
        assert_eq!(f.ret, ret, "`{ten}`: kiểu trả sai hình dạng");
    }
    let _ = (k,);
}

/// `list.sort_by` **giữ** `Ty::FnKhoa` (ngoại lệ **có ghi** duy nhất, VD-94 (3)/VD-99 (5)):
/// lambda chưa chú thích là `{error}`; `Type` không có union `int|str` (ST-56) ⇒ tầng chạy kiểm.
#[test]
fn sort_by_giu_ngoai_le_fn_khoa() {
    let f = sig_of("list.sort_by").expect("list.sort_by");
    assert!(matches!(f.params[1], Ty::FnKhoa), "phải giữ `FnKhoa`");
    assert!(
        matches!(f.params[0], Ty::List(_)),
        "tham số 1 phải là `list[T]`"
    );
}
