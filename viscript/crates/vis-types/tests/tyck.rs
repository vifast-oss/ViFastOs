// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-12 — test ĐỎ TRƯỚC (D2-05)** của kiểm kiểu VS0 (`crates/vis-types/src/tyck`).
//!
//! # Quy ước (để review được)
//!
//! Mỗi ca là **một chương trình nhỏ, tự chứa** (khai đủ `fn`/`let`/`type`/`enum`) + kỳ vọng ghi
//! ngay **trên** ca:
//!
//! ```text
//! // kỳ vọng: ok(int)      ⇒ hạ + kiểm **0 chẩn đoán**, `value_type()` in ra đúng `int`
//! // kỳ vọng: !VS0412      ⇒ **đúng một** chẩn đoán mang mã `VS0412`
//! ```
//!
//! `ok(<kiểu>)` in theo `TypeTable::show` (bản ghi/enum ra **tên**, còn lại theo `Type::name`:
//! `int`, `int?`, `list[int]`, `map[str, int]`, `Result[int, str]`, `()`, `{error}`).
//! `value_type()` = kiểu của **biểu thức cấp tệp cuối** (không có ⇒ `()`).
//!
//! Ca nào cố ý dùng tên **giá trị** tự do thì ghi lý do trong comment (VD-45: Permissive).

use vis_syntax::{lex, parse, Mode};
use vis_types::{check, lower, CheckOutput, TypeTable};

/// Hạ + kiểm một nguồn; ca phải **parse sạch** và **hạ sạch** (chẩn đoán chỉ được đến từ tyck).
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
        "ca phải **hạ** sạch (chẩn đoán hạ: {:?})\nnguồn: {src}",
        h.codes()
    );
    let out = check(&h.hir, &h.symbols, &h.types);
    (out, h.types)
}

/// So một ca; trả `None` nếu đúng, `Some(mô tả)` nếu sai.
fn so_ca(src: &str, want: &str) -> Option<String> {
    let (out, table) = run(src);
    let got = if out.is_clean() {
        format!("ok({})", table.show(&out.value_type()))
    } else {
        format!("!{}", out.codes().join(","))
    };
    if got == want {
        // `ok(())` không được "xanh" nhờ **không kiểm gì** (khung rỗng của pha đỏ).
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

fn kiem_nhom(ten: &str, cases: &[(&str, &str)]) {
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

// ---------------------------------------------------------------------------
// 1) Literal + chú thích kiểu (10 ca)
// ---------------------------------------------------------------------------

const LITERAL: &[(&str, &str)] = &[
    // kỳ vọng: ok(int)
    ("1", "ok(int)"),
    // kỳ vọng: ok(float)
    ("1.5", "ok(float)"),
    // kỳ vọng: ok(str)
    ("\"a\"", "ok(str)"),
    // kỳ vọng: ok(bool)
    ("true", "ok(bool)"),
    // kỳ vọng: ok(size)
    ("5mb", "ok(size)"),
    // kỳ vọng: ok(duration)
    ("250ms", "ok(duration)"),
    // kỳ vọng: ok(percent)
    ("50pct", "ok(percent)"),
    // kỳ vọng: ok(bytes)
    ("b\"a\"", "ok(bytes)"),
    // kỳ vọng: ok(path)
    ("p\"./x\"", "ok(path)"),
    // kỳ vọng: ok(char)
    ("'a'", "ok(char)"),
];

// ---------------------------------------------------------------------------
// 2) Số học theo đơn vị (22 ca) — VD-43 #2
// ---------------------------------------------------------------------------

const SO_HOC: &[(&str, &str)] = &[
    // kỳ vọng: ok(int)
    ("1 + 2", "ok(int)"),
    // kỳ vọng: ok(float)
    ("1.5 + 2.5", "ok(float)"),
    // kỳ vọng: ok(size)
    ("5mb + 5mb", "ok(size)"),
    // kỳ vọng: ok(size)
    ("5mb - 1kb", "ok(size)"),
    // kỳ vọng: ok(duration)
    ("250ms + 1s", "ok(duration)"),
    // kỳ vọng: ok(percent)
    ("50pct + 10pct", "ok(percent)"),
    // kỳ vọng: !VS0412 — trộn hai đơn vị
    ("5mb + 1s", "!VS0412"),
    // kỳ vọng: !VS0412 — trộn int với đơn vị
    ("1 + 5mb", "!VS0412"),
    // kỳ vọng: ok(size)
    ("5mb * 2", "ok(size)"),
    // kỳ vọng: ok(size)
    ("2 * 5mb", "ok(size)"),
    // kỳ vọng: ok(size)
    ("5mb / 2", "ok(size)"),
    // kỳ vọng: ok(size)
    ("5mb * 50pct", "ok(size)"),
    // kỳ vọng: ok(size)
    ("50pct * 5mb", "ok(size)"),
    // kỳ vọng: ok(duration)
    ("250ms * 2", "ok(duration)"),
    // kỳ vọng: ok(float) — size / size
    ("5mb / 5mb", "ok(float)"),
    // kỳ vọng: ok(float) — duration / duration
    ("250ms / 250ms", "ok(float)"),
    // kỳ vọng: ok(int) — chia 0 là **runtime**, không phải lỗi kiểu
    ("1 / 0", "ok(int)"),
    // kỳ vọng: ok(int)
    ("2 ** 3", "ok(int)"),
    // kỳ vọng: ok(int) — mũ âm là runtime
    ("2 ** -1", "ok(int)"),
    // kỳ vọng: !VS0412
    ("5mb ** 2", "!VS0412"),
    // kỳ vọng: !VS0412 — không có `str + str`
    ("\"a\" + \"b\"", "!VS0412"),
    // kỳ vọng: ok(int)
    ("1 % 2", "ok(int)"),
    // kỳ vọng: ok(size)
    ("5mb % 2", "ok(size)"),
    // kỳ vọng: !VS0412
    ("50pct % 2", "!VS0412"),
    // kỳ vọng: ok(float) — literal int ở vị trí float (§3.5)
    ("1.5 + 1", "ok(float)"),
];

// ---------------------------------------------------------------------------
// 3) So sánh + logic + `??` (16 ca) — VD-43 #3/#4
// ---------------------------------------------------------------------------

const SO_SANH_LOGIC: &[(&str, &str)] = &[
    // kỳ vọng: ok(bool)
    ("1 == 2", "ok(bool)"),
    // kỳ vọng: ok(bool)
    ("1 < 2", "ok(bool)"),
    // kỳ vọng: ok(bool) — so sánh số: int với float được (tỉ lệ)
    ("1 == 1.5", "ok(bool)"),
    // kỳ vọng: !VS0412 — khác đơn vị
    ("5mb < 1s", "!VS0412"),
    // kỳ vọng: ok(bool)
    ("\"a\" < \"b\"", "ok(bool)"),
    // kỳ vọng: !VS0412 — không so chuỗi với số
    ("1 == \"a\"", "!VS0412"),
    // kỳ vọng: ok(bool)
    ("true == false", "ok(bool)"),
    // kỳ vọng: ok(bool)
    ("true and false", "ok(bool)"),
    // kỳ vọng: !VS0412
    ("1 and true", "!VS0412"),
    // kỳ vọng: ok(bool)
    ("not true", "ok(bool)"),
    // kỳ vọng: !VS0412
    ("not 1", "!VS0412"),
    // kỳ vọng: !VS0412
    ("1 or 2", "!VS0412"),
    // kỳ vọng: ok(bool)
    ("true or false and true", "ok(bool)"),
    // kỳ vọng: ok(int)
    ("let x: int? = 5\nx ?? 0", "ok(int)"),
    // kỳ vọng: !VS0412 — vế phải khác kiểu đã bỏ Option
    ("let x: int? = 5\nx ?? \"a\"", "!VS0412"),
    // kỳ vọng: !VS0412 — `??` cần vế trái là `T?`
    ("let x: int = 5\nx ?? 0", "!VS0412"),
];

// ---------------------------------------------------------------------------
// 4) Kiến trúc: list/map/record/enum/`T?`/trường (17 ca) — VD-43 #1/#6
// ---------------------------------------------------------------------------

const KIEN_TRUC: &[(&str, &str)] = &[
    // kỳ vọng: ok(list[int])
    ("[1, 2]", "ok(list[int])"),
    // kỳ vọng: !VS0412 — phần tử khác kiểu
    ("[1, \"a\"]", "!VS0412"),
    // kỳ vọng: ok(map[str, int])
    ("{\"a\": 1}", "ok(map[str, int])"),
    // kỳ vọng: !VS0412
    ("{\"a\": 1, \"b\": \"x\"}", "!VS0412"),
    // kỳ vọng: ok(list[int]) — `[]` lấy kiểu từ chú thích
    ("let x: list[int] = []\nx", "ok(list[int])"),
    // kỳ vọng: ok(User) — bản ghi ẩn danh so khớp **cấu trúc** với `type User`
    (
        "type User:\n    name: str\n    age: int\nlet u: User = {name: \"An\", age: 20}\nu",
        "ok(User)",
    ),
    // kỳ vọng: !VS0412 — trường sai kiểu
    (
        "type User:\n    name: str\nlet u: User = {name: 1}\nu",
        "!VS0412",
    ),
    // kỳ vọng: !VS0403 — `with` cập nhật trường **không tồn tại**
    (
        "type User:\n    name: str\nlet u: User = {name: \"An\"}\nu with age: 1",
        "!VS0403",
    ),
    // kỳ vọng: !VS0412 — `with` sai kiểu trường
    (
        "type User:\n    name: str\nlet u: User = {name: \"An\"}\nu with name: 1",
        "!VS0412",
    ),
    // kỳ vọng: ok(Shape) — enum literal có đối số
    (
        "enum Shape:\n    Circle(r: float)\n    Empty\nShape.Circle(1.0)",
        "ok(Shape)",
    ),
    // kỳ vọng: !VS0412 — đối số biến thể sai kiểu
    (
        "enum Shape:\n    Circle(r: float)\nShape.Circle(\"a\")",
        "!VS0412",
    ),
    // kỳ vọng: !VS0401 — sai số đối số biến thể
    (
        "enum Shape:\n    Circle(r: float)\nShape.Circle(1.0, 2.0)",
        "!VS0401",
    ),
    // kỳ vọng: ok(Shape) — biến thể không đối số
    (
        "enum Shape:\n    Circle(r: float)\n    Empty\nlet s: Shape = Shape.Empty\ns",
        "ok(Shape)",
    ),
    // kỳ vọng: ok(str) — truy cập trường
    (
        "type User:\n    name: str\nlet u: User = {name: \"A\"}\nu.name",
        "ok(str)",
    ),
    // **SỬA KÈM LÝ DO (ST-50, Agent 0: VS0605 sai sổ — trường là lỗi kiểu)**: truy cập trường
    // không có trên **bản ghi** ⇒ **`VS0405`** (một mã duy nhất); trước là `VS0403`.
    (
        "type User:\n    name: str\nlet u: User = {name: \"A\"}\nu.age",
        "!VS0405",
    ),
    // kỳ vọng: !VS0412 — `.` trên `T?` phải dùng `?.`
    ("let x: int? = 5\nx.foo", "!VS0412"),
    // kỳ vọng: !VS0301 — tên **kiểu** không tồn tại (ST-12 phân giải `TypeRef`)
    ("let x: Unknown = 1\nx", "!VS0301"),
];

// ---------------------------------------------------------------------------
// 5) Hàm: số đối số, kiểu đối số, `?`, tham số mặc định, lambda (20 ca)
// ---------------------------------------------------------------------------

const HAM: &[(&str, &str)] = &[
    // kỳ vọng: ok(int)
    ("fn f(a: int) -> int:\n    a\nf(1)", "ok(int)"),
    // kỳ vọng: !VS0412
    ("fn f(a: int) -> int:\n    a\nf(\"a\")", "!VS0412"),
    // kỳ vọng: !VS0401 — thiếu đối số
    ("fn f(a: int, b: int) -> int:\n    a\nf(1)", "!VS0401"),
    // kỳ vọng: !VS0401 — thừa đối số
    ("fn f(a: int) -> int:\n    a\nf(1, 2)", "!VS0401"),
    // kỳ vọng: !VS0412 — `return` sai kiểu
    ("fn f(a: int) -> int:\n    \"a\"\nf(1)", "!VS0412"),
    // kỳ vọng: !VS0412
    ("fn f() -> str:\n    1\nf()", "!VS0412"),
    // kỳ vọng: ok(int) — tham số có tên `--b`
    ("fn g(a: int, --b: int = 1) -> int:\n    a\ng(1, --b: 2)", "ok(int)"),
    // kỳ vọng: !VS0412 — đối số có tên sai kiểu
    (
        "fn g(a: int, --b: int = 1) -> int:\n    a\ng(1, --b: \"x\")",
        "!VS0412",
    ),
    // kỳ vọng: !VS0401 — thiếu tham số bắt buộc
    ("fn g(a: int, --b: int = 1) -> int:\n    a\ng()", "!VS0401"),
    // kỳ vọng: !VS0401 — lambda **một** đối số, gọi **hai** đối số
    ("let f = (x) => x\nf(1, 2)", "!VS0401"),
    // kỳ vọng: ok({error}) — parser VS0 **chưa** nhận kiểu cho tham số lambda (`(x: int) =>` ⇒
    // `VS0201`), nên tham số là `{error}` (ST-11) và kiểu gọi cũng `{error}`; suy kiểu lambda
    // hai chiều thuộc ST-14. Ca này khoá **hành vi hiện tại**, ghi ở JOURNAL.
    ("let f = (x) => x\nf(1)", "ok({error})"),
    // kỳ vọng: ok(int) — truyền hàm làm đối số
    (
        "fn apply(f: fn(int) -> int, x: int) -> int:\n    f(x)\nfn inc(y: int) -> int:\n    y + 1\napply(inc, 1)",
        "ok(int)",
    ),
    // kỳ vọng: !VS0412 — đối số hàm sai kiểu
    (
        "fn apply(f: fn(int) -> int, x: int) -> int:\n    f(x)\napply(1, 1)",
        "!VS0412",
    ),
    // kỳ vọng: ok(int?) — tự nâng `T → T?` ở **trả về**
    ("fn f(x: int) -> int?:\n    x\nf(1)", "ok(int?)"),
    // kỳ vọng: !VS0412 — `?` trên kiểu không phải `T?`
    ("fn f(x: int) -> int:\n    x?\nf(1)", "!VS0412"),
    // kỳ vọng: !VS0412 — **SỬA KỲ VỌNG CŨ KÈM LÝ DO (ST-12b, Agent 0 cho phép)**: `?` trên `T?`
    // chỉ dùng được khi hàm trả `U?` (§4.4); ca này từng khoá **hành vi sai** (`ok(int)`).
    ("fn f(x: int?) -> int:\n    x?\nf(5)", "!VS0412"),
    // kỳ vọng: !VS0412 — `?` cho `int` nhưng hàm trả `str`
    ("fn f(x: int?) -> str:\n    x?\nf(5)", "!VS0412"),
    // kỳ vọng: ok(int?) — gọi hàm trả `T?`
    ("fn f(x: int?) -> int?:\n    x\nf(5)", "ok(int?)"),
    // kỳ vọng: ok(int) — tham số mặc định dùng được
    ("fn f(a: int = 1) -> int:\n    a\nf()", "ok(int)"),
    // kỳ vọng: !VS0412 — giá trị mặc định sai kiểu
    ("fn f(a: int = \"x\") -> int:\n    a\nf()", "!VS0412"),
];

#[test]
fn literal_va_chu_thich_kieu() {
    kiem_nhom("literal", LITERAL);
}

#[test]
fn so_hoc_theo_don_vi() {
    kiem_nhom("số học", SO_HOC);
}

#[test]
fn so_sanh_logic_va_coalesce() {
    kiem_nhom("so sánh/logic", SO_SANH_LOGIC);
}

#[test]
fn kien_truc_list_map_record_enum() {
    kiem_nhom("kiến trúc", KIEN_TRUC);
}

#[test]
fn ham_doi_so_va_optional() {
    kiem_nhom("hàm", HAM);
}

/// Tổng số ca phải ≥ 80 (tiêu chí ST-12) — giảm là đỏ.
#[test]
fn du_80_ca() {
    let n = LITERAL.len() + SO_HOC.len() + SO_SANH_LOGIC.len() + KIEN_TRUC.len() + HAM.len();
    assert!(n >= 80, "bảng ca phải ≥ 80, thật {n}");
}
