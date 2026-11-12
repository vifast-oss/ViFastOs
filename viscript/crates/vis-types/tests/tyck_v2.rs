// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-12b — 7 lỗi kiểm kiểu do review độc lập bắt (tệp mới, đỏ trước).**
//!
//! Tệp **riêng**: `tests/tyck.rs` + `tests/exhaustive.rs` giữ nguyên (log `st12-red-tyck.log` là bản
//! ghi **lịch sử** của commit `273a921`); mọi ca của ST-12b nằm ở đây.
//!
//! Bảy nhóm, theo thứ tự ưu tiên của Agent 0:
//! 1. `?` chỉ dùng khi hàm trả `U?` / `Result[U,F]` (§4.4)
//! 2. nâng `int → float` **chỉ** cho literal số nguyên (§3.5)
//! 3. gọi **hàm dựng sẵn** phải kiểm chữ ký (`VS0401`/`VS0412`)
//! 4. gán `Field`/`Index`: bản ghi bất biến, `let` bất biến (§3.2/§4.1)
//! 5. `--tên` cho tham số **không** khai `--` ⇒ `VS0401`; trùng tên ⇒ `VS0401`
//! 6. `percent` so sánh `float` **theo tỉ lệ** (§3.1/§3.6)
//! 7. `match` trên `unit`/tuple **hữu hạn** ⇒ không đòi `_` oan (§4.3)

use vis_syntax::{lex, parse, Mode};
use vis_types::{check, lower, CheckOutput, TypeTable};

fn run(src: &str) -> (CheckOutput, TypeTable) {
    let p = parse(src, Mode::Script);
    assert!(
        p.diagnostics.is_empty(),
        "ca phải parse sạch: {:?}\nnguồn: {src}",
        p.diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect::<Vec<_>>()
    );
    let h = lower(&p.cst, &lex(src, Mode::Script));
    assert!(
        h.is_clean(),
        "ca phải hạ sạch: {:?}\nnguồn: {src}",
        h.codes()
    );
    (check(&h.hir, &h.symbols, &h.types), h.types)
}

fn so_ca(src: &str, want: &str) -> Option<String> {
    let (out, table) = run(src);
    let got = if out.is_clean() {
        format!("ok({})", table.show(&out.value_type()))
    } else {
        format!("!{}", out.codes().join(","))
    };
    if got == want {
        if out.types_of_expr.is_empty() {
            return Some(format!(
                "nguồn: {src}\n   muốn: {want}\n   được: {got}\n   nhưng **chưa kiểm biểu thức nào**"
            ));
        }
        return None;
    }
    let diag: Vec<String> = out
        .diags
        .iter()
        .map(|d| format!("{} {}:{} {}", d.code.short, d.line, d.col, d.message))
        .collect();
    Some(format!(
        "nguồn: {src}\n   muốn: {want}\n   được: {got}\n   chẩn đoán: {diag:?}"
    ))
}

fn kiem(ten: &str, cases: &[(&str, &str)]) {
    let mut sai = Vec::new();
    for (src, want) in cases {
        if let Some(m) = so_ca(src, want) {
            sai.push(m);
        }
    }
    assert!(
        sai.is_empty(),
        "{ten}: {}/{} ca sai\n{}",
        sai.len(),
        cases.len(),
        sai.join("\n---\n")
    );
}

/// (1) `?` chỉ dùng khi hàm trả `U?` / `Result[U,F]` — §4.4.
const TRY_KHONG_KHOP: &[(&str, &str)] = &[
    // kỳ vọng: ok(int?) — hàm trả `U?` ⇒ hợp lệ
    ("fn f(x: int?) -> int?:\n    x?\nf(5)", "ok(int?)"),
    // kỳ vọng: !VS0412 — hàm trả `int` (không phải `int?`) ⇒ `?` không được dùng
    ("fn f(x: int?) -> int:\n    x?\nf(5)", "!VS0412"),
    // kỳ vọng: !VS0412 — hàm trả `str`
    ("fn f(x: int?) -> str:\n    x?\nf(5)", "!VS0412"),
    // kỳ vọng: ok(Result[int, str]) — hàm trả `Result` ⇒ hợp lệ
    (
        "fn f(r: Result[int, str]) -> Result[int, str]:\n    let v = r?\n    Ok(v)\nmatch f(Ok(1)):\n    Ok(x): print(x)\n    Err(e): print(e)",
        "ok(())",
    ),
    // kỳ vọng: !VS0412 — `?` trên `Result` trong hàm trả `int?` (§4.4)
    ("fn f(r: Result[int, str]) -> int?:\n    let v = r?\n    Some(v)", "!VS0412"),
    // kỳ vọng: !VS0412 — `?` trên `int` (không optional/result)
    ("fn f(x: int) -> int:\n    x?\nf(1)", "!VS0412"),
];

/// (2) `int → float` **chỉ** cho literal số nguyên — §3.5.
const NANG_FLOAT: &[(&str, &str)] = &[
    // kỳ vọng: ok(float) — literal int ở vị trí float
    ("let y: float = 1\ny", "ok(float)"),
    // kỳ vọng: ok(float) — literal int trong phép với float
    ("1.5 + 1", "ok(float)"),
    // kỳ vọng: ok(bool) — literal int so với float
    ("1 == 1.5", "ok(bool)"),
    // kỳ vọng: !VS0412 — **biến** int, không phải literal
    ("let x = 1\nlet y: float = x\ny", "!VS0412"),
    // kỳ vọng: !VS0412 — so sánh biến int với float
    ("let x = 1\nx == 1.5", "!VS0412"),
    // kỳ vọng: !VS0412 — cộng biến int với float
    ("let x = 1\nx + 1.5", "!VS0412"),
    // kỳ vọng: !VS0412 — trả biến int ở vị trí float
    ("fn f(x: int) -> float:\n    x\nf(1)", "!VS0412"),
];

/// (3) Hàm dựng sẵn phải được kiểm chữ ký.
const BUILTIN: &[(&str, &str)] = &[
    // kỳ vọng: ok(Result[int, str])
    ("let x: Result[int, str] = Ok(1)\nx", "ok(Result[int, str])"),
    // kỳ vọng: !VS0412 — `Ok` nhận `str` nhưng khai `Result[int, str]`
    ("let x: Result[int, str] = Ok(\"a\")\nx", "!VS0412"),
    // kỳ vọng: !VS0412 — `Some` nhận `str` nhưng khai `int?`
    ("let x: int? = Some(\"a\")\nx", "!VS0412"),
    // kỳ vọng: ok(()) — `print` trả `()`
    ("print(1)", "ok(())"),
    // kỳ vọng: !VS0401 — thiếu đối số
    ("len()", "!VS0401"),
    // kỳ vọng: !VS0401 — thừa đối số
    ("str(1, 2)", "!VS0401"),
    // kỳ vọng: !VS0412 — `len` không nhận `int`
    ("len(1)", "!VS0412"),
];

/// (4) Gán `Field`/`Index`: bản ghi bất biến, `let` bất biến — §3.2/§4.1.
const GAN: &[(&str, &str)] = &[
    // kỳ vọng: !VS0412 — bản ghi bất biến (gợi ý `with`)
    (
        "type U:\n    name: str\nlet u: U = {name: \"a\"}\nu.name = \"b\"",
        "!VS0412",
    ),
    // kỳ vọng: !VS0412 — `let` bất biến
    ("let xs = [1, 2]\nxs[0] = 3", "!VS0412"),
    // kỳ vọng: ok(list[int]) — `var` gán phần tử được
    ("var xs = [1, 2]\nxs[0] = 3\nxs", "ok(list[int])"),
    // kỳ vọng: ok(int) — `var` gán lại
    ("var x = 1\nx = 2\nx", "ok(int)"),
    // kỳ vọng: !VS0412 — gán khác kiểu
    ("var x = 1\nx = \"a\"", "!VS0412"),
];

/// (5) `--tên` cho tham số không khai `--` ⇒ `VS0401`.
const TEN_THAM_SO: &[(&str, &str)] = &[
    // kỳ vọng: !VS0401 — `a` là tham số **vị trí**, không nhận `--a`
    (
        "fn f(a: int, --b: int = 1) -> int:\n    a + b\nf(1, --a: 2)",
        "!VS0401",
    ),
    // kỳ vọng: !VS0401 — đối số tên **lặp**
    (
        "fn f(--a: int, --b: int = 1) -> int:\n    a\nf(--a: 1, --a: 2)",
        "!VS0401",
    ),
    // kỳ vọng: ok(int) — tham số tên hợp lệ
    (
        "fn f(a: int, --b: int = 1) -> int:\n    a + b\nf(1, --b: 2)",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — tham số tên có mặc định
    ("fn f(--a: int = 1) -> int:\n    a\nf(--a: 3)", "ok(int)"),
];

/// (6) `percent` so sánh `float` theo tỉ lệ — §3.1/§3.6.
const PERCENT_FLOAT: &[(&str, &str)] = &[
    // kỳ vọng: ok(bool) — `50pct == 0.5`
    ("50pct == 0.5", "ok(bool)"),
    // kỳ vọng: ok(bool) — `25pct < 0.3`
    ("25pct < 0.3", "ok(bool)"),
    // kỳ vọng: ok(bool) — `0.5 < 50pct`
    ("0.5 < 50pct", "ok(bool)"),
    // kỳ vọng: !VS0412 — cộng `percent` với `float` **không** được
    ("50pct + 0.5", "!VS0412"),
];

/// (7) `match` trên `unit`/tuple là **hữu hạn** — §4.3.
const MATCH_HUU_HAN: &[(&str, &str)] = &[
    // kỳ vọng: ok(()) — `unit` chỉ có một giá trị `()`
    ("let u = ()\nmatch u:\n    (): print(0)", "ok(())"),
    // kỳ vọng: ok(()) — tuple 2 thành phần, pattern `(a, b)` phủ hết
    ("let t = (1, 2)\nmatch t:\n    (a, b): print(a)", "ok(())"),
    // kỳ vọng: ok(int) — `match` biểu thức trên tuple
    (
        "let t = (1, 2)\nlet v = match t:\n    (a, b): a\nv",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — pattern chỉ phủ một phần (`1`, b)
    ("let t = (1, 2)\nmatch t:\n    (1, b): print(b)", "!VS0402"),
];

/// (8) Phủ thêm: `|>`, `?.`, `str[i]`, `let`/`var`, builtin, `percent`, tuple.
const PHU: &[(&str, &str)] = &[
    // kỳ vọng: ok(int)
    ("fn inc(x: int) -> int:\n    x + 1\n3 |> inc", "ok(int)"),
    // kỳ vọng: !VS0412 — `|>` với đối số sai kiểu
    ("fn inc(x: int) -> int:\n    x + 1\n\"a\" |> inc", "!VS0412"),
    // kỳ vọng: ok(int?) — `?.` trên `U?`
    (
        "type U:\n    age: int\nlet u: U? = {age: 20}\nu?.age",
        "ok(int?)",
    ),
    // kỳ vọng: !VS0412 — `.` trên `U?` phải dùng `?.`
    (
        "type U:\n    age: int\nlet u: U? = {age: 20}\nu.age",
        "!VS0412",
    ),
    // kỳ vọng: !VS0412 — cấm `s[i]` trên `str` (§4.5)
    ("let s = \"abc\"\ns[0]", "!VS0412"),
    // kỳ vọng: ok(int)
    ("let s = \"abc\"\ns.len()", "ok(int)"),
];

/// (9) **VD-48** — phép của VS0 + thông điệp cho kiểu ngoài VS0.
const PHEP_VS0: &[(&str, &str)] = &[
    // kỳ vọng: ok(size) — `size % int` giữ đơn vị
    ("5mb % 2", "ok(size)"),
    // kỳ vọng: ok(duration) — `duration % int` giữ đơn vị
    ("250ms % 2", "ok(duration)"),
    // kỳ vọng: ok(percent) — `percent / int` ⇒ percent
    ("50pct / 2", "ok(percent)"),
    // kỳ vọng: ok(float) — `percent / percent` ⇒ **float** (VD-48b)
    ("50pct / 25pct", "ok(float)"),
    // kỳ vọng: ok(percent) — `percent + percent` ⇒ percent
    ("50pct + 10pct", "ok(percent)"),
];

#[test]
fn phep_cua_vs0_theo_vd48() {
    kiem("phép VS0", PHEP_VS0);
}

/// VD-48(a): kiểu ngoài VS0 vẫn `VS0301` **nhưng** thông điệp nêu "VS0/vis-std".
#[test]
fn kieu_ngoai_vs0_bao_vs0301_ro_rang() {
    for src in [
        "let x: AnyError = none\nx",
        "fn f(c: cap[Dir]) -> int:\n    1\nf(1)",
        "let j: Json = none\nj",
    ] {
        let (out, _) = run(src);
        assert!(
            !out.codes().is_empty() && out.codes().iter().all(|c| *c == "VS0301"),
            "mọi chẩn đoán phải là VS0301: {:?}\nnguồn: {src}",
            out.codes()
        );
        assert!(
            out.diags
                .iter()
                .any(|d| d.message.contains("VS0") && d.message.contains("vis-std")),
            "phải có thông điệp nêu VS0/vis-std: {:?}\nnguồn: {src}",
            out.diags
                .iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn try_chi_dung_khi_ham_tra_optional_hoac_result() {
    kiem("`?`", TRY_KHONG_KHOP);
}

#[test]
fn nang_int_float_chi_cho_literal() {
    kiem("nâng float", NANG_FLOAT);
}

#[test]
fn builtin_co_kiem_chu_ky() {
    kiem("builtin", BUILTIN);
}

#[test]
fn gan_field_index_kiem_bat_bien() {
    kiem("gán", GAN);
}

#[test]
fn ten_tham_so_phai_khai_dau_hai_gach() {
    kiem("tham số tên", TEN_THAM_SO);
}

#[test]
fn percent_so_sanh_float_theo_ti_le() {
    kiem("percent/float", PERCENT_FLOAT);
}

#[test]
fn match_unit_tuple_la_huu_han() {
    kiem("match hữu hạn", MATCH_HUU_HAN);
}

#[test]
fn phu_them_bieu_thuc_va_kieu() {
    kiem("phủ", PHU);
}
