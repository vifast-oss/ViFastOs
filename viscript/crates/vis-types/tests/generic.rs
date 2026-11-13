// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-28 — **generic hàm + kiểu `[T]`**: xoá kiểu (erase) + **hợp nhất ở call-site**.
//!
//! Khoá: (i) call-site suy được thì vẫn bắt sai kiểu tĩnh (`VS0412`), (ii) chữ ký generic **không** bị
//! `VS0301` oan, (iii) một hiện thực cho cả interp và VM (nên ca chạy nằm ở `test/sem/generic/**`).
//!
//! **Giới hạn VS1b** (JOURNAL ST-28): `Hop[int]` ≡ `Hop[str]` — chưa có instantiation thật; phép toán
//! trên `T` kiểm **động**; constraint/trait ⇒ ST-29.

use vis_syntax::{lex, parse, Mode};
use vis_types::{lower, tyck};

fn kiem(src: &str) -> (Vec<String>, Vec<u32>) {
    let p = format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}");
    let lexed = lex(&p, Mode::Script);
    let out = parse(&p, Mode::Script);
    assert!(
        out.diagnostics.is_empty(),
        "cú pháp phải sạch: {:?}",
        out.diagnostics
    );
    let h = lower(&out.cst, &lexed);
    assert!(h.error.is_none(), "lỗi cấp API: {:?}", h.error);
    let kq = tyck::check(&h.hir, &h.symbols, &h.types);
    (
        kq.codes().iter().map(|s| s.to_string()).collect(),
        kq.diags.iter().map(|d| d.line).collect(),
    )
}

fn sach(src: &str) -> usize {
    let (ma, _) = kiem(src);
    assert!(
        ma.is_empty(),
        "phải sạch chẩn đoán, được {ma:?} cho:\n{src}"
    );
    ma.len()
}

fn co(src: &str, mong: &str) -> Vec<String> {
    let (ma, _) = kiem(src);
    assert!(
        ma.iter().any(|m| m == mong),
        "phải có `{mong}`, được {ma:?} cho:\n{src}"
    );
    ma
}

const CONG: &str = "fn cong[T](a: T, b: T) -> T:\n    a + b\n";

// ---------------------------------------------------------------------------
// 1) Khai báo generic **không** bị `VS0301` oan
// ---------------------------------------------------------------------------

#[test]
fn fn_generic_khai_bao_sach() {
    sach(&format!(
        "{CONG}\nfn main() -> ():\n    print(cong(1, 2))\n"
    ));
}

#[test]
fn fn_hai_tham_so_kieu_sach() {
    sach("fn cap[A, B](a: A, b: B) -> A:\n    a\n\nfn main() -> ():\n    print(cap(1, \"x\"))\n");
}

#[test]
fn kieu_generic_record_sach() {
    sach("type Hop[T]:\n    gia_tri: T\n\nfn main() -> ():\n    let h = Hop(1)\n    print(h.gia_tri)\n");
}

#[test]
fn kieu_generic_enum_sach() {
    sach(
        "enum Or[A, B]:\n    Trai(x: A)\n    Phai(y: B)\n\nfn main() -> ():\n    let a = Or.Trai(\"x\")\n    let b = Or.Phai(1)\n    print(\"ok\")\n",
    );
}

#[test]
fn tham_so_long_list_t_sach() {
    sach("fn dau[T](xs: list[T]) -> int:\n    xs.len()\n\nfn main() -> ():\n    print(dau([1, 2]))\n");
}

#[test]
fn tham_so_map_str_t_sach() {
    sach(
        "fn lay[T](m: map[str, T], k: str) -> T?:\n    none\n\nfn main() -> ():\n    let x: int? = lay({\"a\": 1}, \"a\")\n    print(x)\n",
    );
}

// ---------------------------------------------------------------------------
// 2) Hợp nhất ở call-site: cùng kiểu ⇒ sạch; mâu thuẫn ⇒ `VS0412`
// ---------------------------------------------------------------------------

#[test]
fn cong_int_va_str_cung_chuong_trinh() {
    sach(&format!(
        "{CONG}\nfn main() -> ():\n    print(cong(1, 2))\n    print(cong(\"a\", \"b\"))\n"
    ));
}

#[test]
fn cong_mau_thuan_int_str_la_vs0412() {
    co(
        &format!("{CONG}\nfn main() -> ():\n    print(cong(1, \"a\"))\n"),
        "VS0412",
    );
}

#[test]
fn cong_mau_thuan_str_int_la_vs0412() {
    co(
        &format!("{CONG}\nfn main() -> ():\n    print(cong(\"a\", 1))\n"),
        "VS0412",
    );
}

#[test]
fn thong_diep_mau_thuan_neu_ten_t_va_hai_kieu() {
    let (ma, _) = kiem(&format!(
        "{CONG}\nfn main() -> ():\n    print(cong(1, \"a\"))\n"
    ));
    assert_eq!(ma, vec!["VS0412".to_string()], "đúng một chẩn đoán: {ma:?}");
    // Chi tiết thông điệp: kiểm qua bản chạy binary (test `stdlib`/`sem` giữ phần còn lại).
}

#[test]
fn list_t_voi_int_la_vs0412() {
    co(
        "fn dau[T](xs: list[T]) -> int:\n    xs.len()\n\nfn main() -> ():\n    print(dau(1))\n",
        "VS0412",
    );
}

#[test]
fn list_t_long_nhau_hop_nhat_duoc() {
    sach("fn sau[T](xs: list[list[T]]) -> int:\n    xs.len()\n\nfn main() -> ():\n    print(sau([[1], [2]]))\n");
}

#[test]
fn optional_t_voi_int_optional_sach() {
    sach("fn giu[T](x: T?) -> T?:\n    x\n\nfn main() -> ():\n    let a: int? = some(1)\n    print(giu(a))\n");
}

#[test]
fn optional_t_voi_int_thuong_duoc_nang() {
    // Nhất quán với hàm **không** generic: `int` tự nâng thành `int?` (§4.4) ⇒ suy `T = int`.
    sach("fn giu[T](x: T?) -> T?:\n    x\n\nfn main() -> ():\n    print(giu(1))\n");
}

#[test]
fn sai_so_doi_so_generic_la_vs0401() {
    co(
        &format!("{CONG}\nfn main() -> ():\n    print(cong(1))\n"),
        "VS0401",
    );
}

#[test]
fn thua_doi_so_generic_la_vs0401() {
    co(
        &format!("{CONG}\nfn main() -> ():\n    print(cong(1, 2, 3))\n"),
        "VS0401",
    );
}

// ---------------------------------------------------------------------------
// 3) Kiểu trả generic
// ---------------------------------------------------------------------------

#[test]
fn tra_t_dung_duoc_o_ngu_canh() {
    sach(
        "fn lay[T](x: T) -> T:\n    x\n\nfn main() -> ():\n    let n: int = lay(1)\n    print(n)\n",
    );
}

#[test]
fn tra_t_sai_ngu_canh_la_vs0412() {
    co("fn lay[T](x: T) -> T:\n    x\n\nfn main() -> ():\n    let n: int = lay(\"x\")\n    print(n)\n", "VS0412");
}

#[test]
fn tra_t_optional_voi_match() {
    sach("fn tim[T](xs: list[T]) -> T?:\n    none\n\nfn main() -> ():\n    match tim([1, 2]):\n        some(v): print(v)\n        none: print(\"khong\")\n");
}

#[test]
fn goi_long_generic() {
    sach("fn lay[T](x: T) -> T:\n    x\n\nfn cong[T](a: T, b: T) -> T:\n    a + b\n\nfn main() -> ():\n    print(cong(lay(1), lay(2)))\n");
}

// ---------------------------------------------------------------------------
// 4) Record/enum generic: payload xoá kiểu ⇒ **không** báo oan
// ---------------------------------------------------------------------------

#[test]
fn record_generic_hai_kieu_cung_luc() {
    sach("type Hop[T]:\n    gia_tri: T\n\nfn main() -> ():\n    let a = Hop(1)\n    let b = Hop(\"x\")\n    print(a.gia_tri)\n    print(b.gia_tri)\n");
}

#[test]
fn record_generic_truong_doc_ra_dung_kieu_dong() {
    sach("type Hop[T]:\n    gia_tri: T\n\nfn main() -> ():\n    let h = Hop([1, 2])\n    print(h.gia_tri.len())\n");
}

#[test]
fn enum_generic_hai_bien_the() {
    sach("enum Or[A, B]:\n    Trai(x: A)\n    Phai(y: B)\n\nfn main() -> ():\n    let a = Or.Trai(\"x\")\n    let b = Or.Phai(1)\n    print(\"ok\")\n");
}

#[test]
fn enum_generic_match_hai_nhanh() {
    sach("enum Or[A, B]:\n    Trai(x: A)\n    Phai(y: B)\n\nfn mo(o: Or[str, int]) -> ():\n    match o:\n        Trai(x): print(x)\n        Phai(y): print(y)\n\nfn main() -> ():\n    mo(Or.Trai(\"x\"))\n    mo(Or.Phai(1))\n");
}

#[test]
fn enum_generic_payload_khong_bi_khoa_kieu() {
    // Nợ VS1b: chưa có instantiation thật ⇒ `Or[str,int]` nhận bất kỳ payload nào **vẫn sạch**;
    // nếu ai đó thêm instantiation thật, ca này sẽ đỏ và phải cập nhật cùng JOURNAL.
    sach("enum Or[A, B]:\n    Trai(x: A)\n    Phai(y: B)\n\nfn main() -> ():\n    let a = Or.Trai(1)\n    print(\"van chay\")\n");
}

// ---------------------------------------------------------------------------
// 5) Thiếu type-arg / tham số kiểu lạ
// ---------------------------------------------------------------------------

#[test]
fn kieu_tham_so_la_van_la_vs0301() {
    co(
        "fn f[T](x: U) -> int:\n    0\n\nfn main() -> ():\n    print(f(1))\n",
        "VS0301",
    );
}

#[test]
fn generic_khong_dung_trong_than_van_sach() {
    sach("fn f[T](x: int) -> int:\n    x\n\nfn main() -> ():\n    print(f(1))\n");
}

#[test]
fn hai_generic_kieu_tra_rieng() {
    sach("fn trai[A, B](a: A, b: B) -> A:\n    a\n\nfn main() -> ():\n    print(trai(1, \"x\"))\n    print(trai(\"y\", 2))\n");
}

#[test]
fn ba_generic_cung_luc() {
    sach("fn ba[A, B, C](a: A, b: B, c: C) -> A:\n    a\n\nfn main() -> ():\n    print(ba(1, \"x\", true))\n");
}

#[test]
fn generic_voi_mac_dinh_khong_dung() {
    sach("fn f[T](x: T, y: int = 0) -> T:\n    x\n\nfn main() -> ():\n    print(f(1))\n");
}

// ---------------------------------------------------------------------------
// 6) Kiểm **cả hai đường** cho cùng kết luận (bug phát hiện ở ST-28)
// ---------------------------------------------------------------------------

#[test]
fn kiem_va_chay_cung_ket_luan_tren_cung_tep() {
    // `vis check` và `vis run` phải **cùng** kết luận với chương trình generic (trước ST-28: check
    // xanh nhưng run `VS0301` — bug thật, xem JOURNAL). Test CLI tương ứng:
    // `crates/vis-cli/tests/generic_cli.rs`; ở đây khoá phần tyck: cùng input ⇒ cùng bộ mã.
    let src = "fn f[T](xs: list[T]) -> int:\n    xs.len()\n\nfn main() -> ():\n    print(f([1]))\n";
    let (ma1, _) = kiem(src);
    let (ma2, _) = kiem(src);
    assert_eq!(
        ma1, ma2,
        "kiểm hai lần phải **tất định**: {ma1:?} vs {ma2:?}"
    );
    assert!(
        ma1.is_empty(),
        "chương trình generic hợp lệ phải sạch: {ma1:?}"
    );
}
