// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-35** — generic **thật**: `Hop[int] ≠ Hop[str]` (nợ VD-61). **25 ca**.
//!
//! Khoá: mỗi tổ hợp tham số kiểu là **một** kiểu riêng (instantiation), trường/biến thể được **thay**
//! `T`; hàm generic **hợp nhất liên kết** các vị trí. Tầng này chỉ nói về **tyck** (không chạy).

use vis_syntax::Mode;
use vis_types::{lower, tyck};

/// Hạ + tyck; trả danh sách mã lỗi (rỗng = sạch).
fn ma(src: &str) -> Vec<String> {
    let lexed = vis_syntax::lex(src, Mode::Script);
    let out = vis_syntax::parse(src, Mode::Script);
    let h = lower(&out.cst, &lexed);
    assert!(h.error.is_none(), "lỗi cấp API: {:?}", h.error);
    let k = tyck::check(&h.hir, &h.symbols, &h.types);
    k.diags.iter().map(|d| d.code.short.to_string()).collect()
}

fn sach(src: &str) {
    let m = ma(src);
    assert!(m.is_empty(), "phải sạch, được {m:?} — nguồn:\n{src}");
}

fn co(src: &str, mong: &str) {
    let m = ma(src);
    assert!(
        m.iter().any(|x| x == mong),
        "phải có {mong}, được {m:?} — nguồn:\n{src}"
    );
}

const HOP: &str = "type Hop[T]:\n    gia_tri: T\n\n";

// ---------- 1–5: instantiation bản ghi ----------

#[test]
fn hop_int_sach() {
    sach(&format!("{HOP}let a: Hop[int] = {{gia_tri: 1}}\n"));
}

#[test]
fn hop_str_sach() {
    sach(&format!("{HOP}let b: Hop[str] = {{gia_tri: \"x\"}}\n"));
}

#[test]
fn hop_int_nhan_chuoi_la_vs0412() {
    co(
        &format!("{HOP}let c: Hop[int] = {{gia_tri: \"x\"}}\n"),
        "VS0412",
    );
}

#[test]
fn gan_cheo_hai_instantiation_la_vs0412() {
    // **Chính nợ VD-61**: `Hop[int]` và `Hop[str]` là hai kiểu **khác nhau**.
    co(
        &format!("{HOP}let a: Hop[int] = {{gia_tri: 1}}\nlet b: Hop[str] = a\n"),
        "VS0412",
    );
}

#[test]
fn gan_cung_instantiation_sach() {
    // Chiều **ngược lại**: cùng kiểu ⇒ **sạch** (instantiation không phải "khác gì cũng đỏ").
    sach(&format!(
        "{HOP}let a: Hop[int] = {{gia_tri: 1}}\nlet lai: Hop[int] = a\n"
    ));
}

// ---------- 6–10: trường được **thay** `T` ----------

#[test]
fn truong_hop_int_la_int() {
    sach(&format!(
        "{HOP}let a: Hop[int] = {{gia_tri: 1}}\nlet n: int = a.gia_tri\n"
    ));
}

#[test]
fn truong_hop_int_khong_phai_str() {
    co(
        &format!("{HOP}let a: Hop[int] = {{gia_tri: 1}}\nlet s: str = a.gia_tri\n"),
        "VS0412",
    );
}

#[test]
fn hai_instantiation_cung_ton_tai() {
    sach(&format!(
        "{HOP}let a: Hop[int] = {{gia_tri: 1}}\nlet b: Hop[str] = {{gia_tri: \"x\"}}\nprint(a.gia_tri)\nprint(b.gia_tri)\n"
    ));
}

#[test]
fn generic_long_trong_generic() {
    sach(&format!(
        "{HOP}let a: Hop[list[int]] = {{gia_tri: [1, 2]}}\n"
    ));
}

#[test]
fn generic_long_sai_phan_tu_la_vs0412() {
    co(
        &format!("{HOP}let a: Hop[list[int]] = {{gia_tri: [\"x\"]}}\n"),
        "VS0412",
    );
}

// ---------- 11–15: enum generic ----------

const OR: &str = "enum Or[A, B]:\n    Trai(x: A)\n    Phai(y: B)\n\n";

#[test]
fn or_int_str_literal_sach() {
    sach(&format!("{OR}let x: Or[int, str] = Or.Trai(1)\n"));
}

#[test]
fn or_sai_dau_bien_the_la_vs0412() {
    co(&format!("{OR}let y: Or[str, int] = Or.Trai(1)\n"), "VS0412");
}

#[test]
fn or_gan_cheo_instantiation_la_vs0412() {
    co(
        &format!("{OR}let x: Or[int, str] = Or.Trai(1)\nlet y: Or[str, int] = x\n"),
        "VS0412",
    );
}

#[test]
fn or_match_thieu_nhanh_van_vs0402() {
    co(
        &format!(
            "{OR}fn mo(o: Or[int, str]) -> ():\n    match o:\n        Trai(x): print(x)\n\nmo(Or.Trai(1))\n"
        ),
        "VS0402",
    );
}

#[test]
fn or_match_du_nhanh_sach() {
    sach(&format!(
        "{OR}fn mo(o: Or[int, str]) -> ():\n    match o:\n        Trai(x): print(x)\n        Phai(y): print(y)\n\nmo(Or.Trai(1))\nmo(Or.Phai(\"s\"))\n"
    ));
}

// ---------- 16–21: hàm generic hợp nhất **liên kết** ----------

const F: &str = "fn f[T](x: T, h: Hop[T]) -> T:\n    x\n\n";

#[test]
fn ham_lien_ket_sach() {
    sach(&format!("{HOP}{F}print(f(1, {{gia_tri: 2}}))\n"));
}

#[test]
fn ham_lien_ket_lech_la_vs0412() {
    co(
        &format!("{HOP}{F}print(f(1, {{gia_tri: \"s\"}}))\n"),
        "VS0412",
    );
}

#[test]
fn ham_lien_ket_t_la_str_sach() {
    sach(&format!("{HOP}{F}print(f(\"a\", {{gia_tri: \"b\"}}))\n"));
}

#[test]
fn ham_hai_tham_so_cung_t_lech_la_vs0412() {
    // `fn g[T](a: T, b: T)` — hai vị trí **cùng** `T` ⇒ phải khớp.
    co(
        "fn g[T](a: T, b: T) -> T:\n    a\n\nprint(g(1, \"x\"))\n",
        "VS0412",
    );
}

#[test]
fn ham_tra_ve_t_giu_kieu_sach() {
    sach(&format!("{HOP}{F}let n: int = f(1, {{gia_tri: 2}})\n"));
}

#[test]
fn ham_tra_ve_t_sai_ngu_canh_la_vs0412() {
    co(
        &format!("{HOP}{F}let s: str = f(1, {{gia_tri: 2}})\n"),
        "VS0412",
    );
}

// ---------- 22–25: biên ----------

#[test]
fn hop_bool_khac_hop_int() {
    co(
        &format!("{HOP}let a: Hop[int] = {{gia_tri: 1}}\nlet b: Hop[bool] = a\n"),
        "VS0412",
    );
}

#[test]
fn or_cung_hai_dau_sach() {
    sach(&format!("{OR}let x: Or[int, int] = Or.Trai(1)\n"));
}

#[test]
fn or_list_sai_phan_tu_la_vs0412() {
    co(
        "enum Or[A]:\n    Mot(x: A)\n\nlet x: Or[list[int]] = Or.Mot([\"s\"])\n",
        "VS0412",
    );
}

#[test]
fn tham_so_hop_lech_instantiation_la_vs0412() {
    co(
        &format!("{HOP}{F}let b: Hop[str] = {{gia_tri: \"x\"}}\nprint(f(1, b))\n"),
        "VS0412",
    );
}
