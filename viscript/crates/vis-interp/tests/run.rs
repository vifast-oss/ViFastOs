// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-13 — test ĐỎ TRƯỚC (D2-05)** của bộ thông dịch VS0.
//!
//! # Quy ước
//!
//! Mỗi ca là **một chương trình tự chứa** + kỳ vọng ghi **trên** ca:
//!
//! ```text
//! // kỳ vọng: out("3\n")     ⇒ chạy sạch, `stdout` đúng, và `steps > 0` (chống xanh-giả)
//! // kỳ vọng: !VS0502        ⇒ **đúng một** chẩn đoán runtime mang mã đó
//! ```
//!
//! Mọi ca phải **parse + hạ + kiểm kiểu sạch** trước khi chạy (tyck ST-12 là cổng của pipeline);
//! riêng test `phuong_thuc_ngoai_tap_bao_vs0501` cố ý **bỏ** tyck để kiểm phòng thủ của interp.

use vis_interp::{run, RunOutput};
use vis_syntax::{lex, parse, Mode};
use vis_types::{check, lower};

fn run_src(src: &str) -> RunOutput {
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
    let t = check(&h.hir, &h.symbols, &h.types);
    assert!(
        t.is_clean(),
        "ca phải **kiểm kiểu** sạch: {:?}\nnguồn: {src}",
        t.diags
            .iter()
            .map(|d| format!("{} {}:{} {}", d.code.short, d.line, d.col, d.message))
            .collect::<Vec<_>>()
    );
    run(src, &h.hir, &h.symbols, &h.types, &[])
}

fn so_ca(src: &str, want: &str) -> Option<String> {
    let out = run_src(src);
    let got = if out.is_clean() {
        format!("out({:?})", out.stdout)
    } else {
        format!("!{}", out.codes().join(","))
    };
    if got != want {
        let diag: Vec<String> = out
            .diagnostics
            .iter()
            .map(|d| format!("{} {}:{} {}", d.code.short, d.line, d.col, d.message))
            .collect();
        return Some(format!(
            "nguồn: {src}\n   muốn: {want}\n   được: {got}\n   chẩn đoán: {diag:?}"
        ));
    }
    // Chống xanh-giả: `out("")` cũng phải **chạy thật** ít nhất một bước.
    if out.steps == 0 {
        return Some(format!(
            "nguồn: {src}\n   muốn: {want}\n   được: {got}\n   nhưng **chưa chạy bước nào**"
        ));
    }
    None
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

// ---------------------------------------------------------------------------
// 1) Số học + in ấn (21 ca)
// ---------------------------------------------------------------------------

const SO_HOC: &[(&str, &str)] = &[
    // kỳ vọng: out("3\n")
    ("print(1 + 2)", "out(\"3\\n\")"),
    // kỳ vọng: out("3\n")
    ("print(7 / 2)", "out(\"3\\n\")"),
    // kỳ vọng: out("-3\n")
    ("print(-7 / 2)", "out(\"-3\\n\")"),
    // kỳ vọng: out("-1\n")
    ("print(-7 % 2)", "out(\"-1\\n\")"),
    // kỳ vọng: out("1024\n")
    ("print(2 ** 10)", "out(\"1024\\n\")"),
    // kỳ vọng: out("512\n")
    ("print(2 ** 3 ** 2)", "out(\"512\\n\")"),
    // kỳ vọng: out("3.5\n")
    ("print(1.5 + 2)", "out(\"3.5\\n\")"),
    // kỳ vọng: out("10mb\n")
    ("print(5mb + 5mb)", "out(\"10mb\\n\")"),
    // kỳ vọng: out("1\n")
    ("print(5mb / 5mb)", "out(\"1\\n\")"),
    // kỳ vọng: out("1s\n")
    ("print(250ms + 750ms)", "out(\"1s\\n\")"),
    // kỳ vọng: out("60pct\n")
    ("print(50pct + 10pct)", "out(\"60pct\\n\")"),
    // kỳ vọng: out("100\n")
    ("print(50pct * 200)", "out(\"100\\n\")"),
    // kỳ vọng: !VS0502
    ("print(1 / 0)", "!VS0502"),
    // kỳ vọng: !VS0502
    ("print(1 % 0)", "!VS0502"),
    // kỳ vọng: !VS0501 — mũ âm
    ("print(2 ** -1)", "!VS0501"),
    // kỳ vọng: !VS0501 — tràn i64
    ("print(9223372036854775807 + 1)", "!VS0501"),
    // kỳ vọng: out("false\n")
    ("print(true and false)", "out(\"false\\n\")"),
    // kỳ vọng: out("true\n")
    ("print(not false)", "out(\"true\\n\")"),
    // kỳ vọng: out("true\n")
    ("print(1 < 2)", "out(\"true\\n\")"),
    // kỳ vọng: out("true\n")
    ("print(50pct < 60pct)", "out(\"true\\n\")"),
    // kỳ vọng: out("1\n") — **ST-56 (VD-92)**: `ref` hạ **identity** (trước là `!VS0501`)
    ("print(ref(1))", "out(\"1\\n\")"),
];

// ---------------------------------------------------------------------------
// 2) Điều khiển + khai báo (12 ca)
// ---------------------------------------------------------------------------

const DIEU_KHIEN: &[(&str, &str)] = &[
    // kỳ vọng: out("2\n")
    ("var x = 1\nx = x + 1\nprint(x)", "out(\"2\\n\")"),
    // kỳ vọng: out("d\n")
    (
        "let x = 1\nif x > 0:\n    print(\"d\")\nelse:\n    print(\"a\")",
        "out(\"d\\n\")",
    ),
    // kỳ vọng: out("0\n1\n2\n")
    ("for i in 0..3:\n    print(i)", "out(\"0\\n1\\n2\\n\")"),
    // kỳ vọng: out("0\n1\n2\n")
    ("for i in 0..=2:\n    print(i)", "out(\"0\\n1\\n2\\n\")"),
    // kỳ vọng: out("10\n")
    (
        "var s = 0\nfor i in 1..=4:\n    s = s + i\nprint(s)",
        "out(\"10\\n\")",
    ),
    // kỳ vọng: out("3\n")
    (
        "var i = 0\nwhile i < 3:\n    i = i + 1\nprint(i)",
        "out(\"3\\n\")",
    ),
    // kỳ vọng: out("2\n")
    (
        "var i = 0\nloop:\n    i += 1\n    if i == 2: break\nprint(i)",
        "out(\"2\\n\")",
    ),
    // kỳ vọng: out("4\n")
    (
        "var s = 0\nfor i in 0..4:\n    if i == 2: continue\n    s += i\nprint(s)",
        "out(\"4\\n\")",
    ),
    // kỳ vọng: out("a\n1\n") — bóng tên theo khối
    (
        "let x = 1\nif true:\n    let x = \"a\"\n    print(x)\nprint(x)",
        "out(\"a\\n1\\n\")",
    ),
    // kỳ vọng: out("3\n") — pattern tuple
    ("let (a, b) = (1, 2)\nprint(a + b)", "out(\"3\\n\")"),
    // kỳ vọng: out("1\n") — `if` biểu thức
    ("let x = if true: 1 else: 2\nprint(x)", "out(\"1\\n\")"),
    // kỳ vọng: out("b\n") — `elif` trong `if` biểu thức
    (
        "let s = if 1 > 2: \"a\" elif 1 == 1: \"b\" else: \"c\"\nprint(s)",
        "out(\"b\\n\")",
    ),
];

// ---------------------------------------------------------------------------
// 3) Hàm, closure, đệ quy, tham số tên/mặc định (12 ca)
// ---------------------------------------------------------------------------

const HAM: &[(&str, &str)] = &[
    // kỳ vọng: out("5\n")
    ("fn add(a: int, b: int) -> int:\n    a + b\nprint(add(2, 3))", "out(\"5\\n\")"),
    // kỳ vọng: out("11\n") — tham số mặc định
    (
        "fn f(a: int, --b: int = 10) -> int:\n    a + b\nprint(f(1))",
        "out(\"11\\n\")",
    ),
    // kỳ vọng: out("6\n") — tham số tên
    (
        "fn f(a: int, --b: int = 10) -> int:\n    a + b\nprint(f(1, --b: 5))",
        "out(\"6\\n\")",
    ),
    // kỳ vọng: out("120\n") — đệ quy (buộc `if` thành biểu thức vì `if` cuối thân là **câu lệnh**)
    (
        "fn fact(n: int) -> int:\n    let r = if n <= 1: 1 else: n * fact(n - 1)\n    r\nprint(fact(5))",
        "out(\"120\\n\")",
    ),
    // kỳ vọng: out("42\n") — lambda + gọi
    ("let inc = (x) => x + 1\nprint(inc(41))", "out(\"42\\n\")"),
    // kỳ vọng: out("11\n") — closure bắt môi trường
    ("let n = 10\nlet f = () => n + 1\nprint(f())", "out(\"11\\n\")"),
    // kỳ vọng: out("42\n") — truyền hàm làm đối số
    (
        "fn apply(f: fn(int) -> int, x: int) -> int:\n    f(x)\nfn inc(y: int) -> int:\n    y + 1\nprint(apply(inc, 41))",
        "out(\"42\\n\")",
    ),
    // kỳ vọng: out("1\n") — `return` sớm
    (
        "fn f(n: int) -> int:\n    if n > 0: return 1\n    2\nprint(f(5))",
        "out(\"1\\n\")",
    ),
    // kỳ vọng: out("3\n") — hàm bậc cao
    (
        "fn twice(f: fn(int) -> int, x: int) -> int:\n    f(f(x))\nfn inc(y: int) -> int:\n    y + 1\nprint(twice(inc, 1))",
        "out(\"3\\n\")",
    ),
    // kỳ vọng: out("12\n")
    (
        "fn pick(a: int, --b: int = 2) -> int:\n    a * b\nprint(pick(3, --b: 4))",
        "out(\"12\\n\")",
    ),
    // kỳ vọng: out("6\n")
    (
        "fn pick(a: int, --b: int = 2) -> int:\n    a * b\nprint(pick(3))",
        "out(\"6\\n\")",
    ),
    // kỳ vọng: out("hi\n") — `main` được chạy nếu có
    ("fn main():\n    print(\"hi\")", "out(\"hi\\n\")"),
];

// ---------------------------------------------------------------------------
// 4) Tập hợp + phương thức lõi (17 ca)
// ---------------------------------------------------------------------------

const TAP_HOP: &[(&str, &str)] = &[
    // kỳ vọng: out("3\n")
    ("let xs = [1, 2, 3]\nprint(xs.len())", "out(\"3\\n\")"),
    // kỳ vọng: out("1\n")
    ("let xs = [1, 2, 3]\nprint(xs[0])", "out(\"1\\n\")"),
    // kỳ vọng: !VS0503 — chỉ số âm
    ("let xs = [1, 2, 3]\nprint(xs[-1])", "!VS0503"),
    // kỳ vọng: !VS0503 — ngoài phạm vi
    ("let xs = [1, 2, 3]\nprint(xs[5])", "!VS0503"),
    // kỳ vọng: out("0\n") — `get` trả `T?` + `??`
    ("let xs = [1, 2, 3]\nprint(xs.get(9) ?? 0)", "out(\"0\\n\")"),
    // kỳ vọng: out("6\n")
    ("let xs = [1, 2, 3]\nprint(xs.sum())", "out(\"6\\n\")"),
    // kỳ vọng: out("[2, 4, 6]\n")
    (
        "let xs = [1, 2, 3]\nprint(xs.map(x => x * 2))",
        "out(\"[2, 4, 6]\\n\")",
    ),
    // kỳ vọng: out("[2, 3]\n")
    (
        "let xs = [1, 2, 3]\nprint(xs.filter(x => x > 1))",
        "out(\"[2, 3]\\n\")",
    ),
    // kỳ vọng: out("1\n2\n3\n")
    (
        "let xs = [1, 2, 3]\nxs.each(x => print(x))",
        "out(\"1\\n2\\n3\\n\")",
    ),
    // kỳ vọng: out("1-2\n")
    ("print([1, 2].join(\"-\"))", "out(\"1-2\\n\")"),
    // kỳ vọng: out("ABC\n")
    ("print(\"abc\".upper())", "out(\"ABC\\n\")"),
    // kỳ vọng: out("abc\n")
    ("print(\"ABC\".lower())", "out(\"abc\\n\")"),
    // kỳ vọng: out("[a, b]\n")
    ("print(\"a,b\".split(\",\"))", "out(\"[a, b]\\n\")"),
    // kỳ vọng: out("5\n") — `len` theo Unicode scalar
    ("print(\"héllo\".len())", "out(\"5\\n\")"),
    // kỳ vọng: out("true\n")
    ("print(\"abc\".contains(\"bc\"))", "out(\"true\\n\")"),
    // kỳ vọng: out("3\n") — `len` toàn cục
    ("print(len(\"héllo\"))", "out(\"5\\n\")"),
    // kỳ vọng: out("2\n") — map
    (
        "let m = {\"a\": 1, \"b\": 2}\nprint(m.len())",
        "out(\"2\\n\")",
    ),
];

// ---------------------------------------------------------------------------
// 5) Bản ghi / enum / `T?` / `Result` / match (12 ca)
// ---------------------------------------------------------------------------

const KIEU: &[(&str, &str)] = &[
    // kỳ vọng: out("20\n")
    (
        "type User:\n    name: str\n    age: int\nlet u: User = {name: \"An\", age: 20}\nprint(u.age)",
        "out(\"20\\n\")",
    ),
    // kỳ vọng: out("21\n") — `with`
    (
        "type User:\n    name: str\n    age: int\nlet u: User = {name: \"An\", age: 20}\nprint((u with age: 21).age)",
        "out(\"21\\n\")",
    ),
    // kỳ vọng: out("2\n") — enum + match
    (
        "enum Shape:\n    Circle(r: float)\n    Empty\nlet s: Shape = Shape.Circle(2.0)\nmatch s:\n    Circle(r): print(r)\n    Empty: print(0)",
        "out(\"2\\n\")",
    ),
    // kỳ vọng: out("nhỏ\n") — guard + nhánh dự phòng
    (
        "enum Shape:\n    Circle(r: float)\n    Rect(w: float, h: float)\nlet s: Shape = Shape.Circle(2.0)\nmatch s:\n    Circle(r) if r > 100.0: print(\"lớn\")\n    Circle(r): print(\"nhỏ\")\n    Rect(w, h): print(\"vuông\")",
        "out(\"nhỏ\\n\")",
    ),
    // kỳ vọng: out("cả hai\n") — or-pattern
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nmatch x:\n    A | B: print(\"cả hai\")",
        "out(\"cả hai\\n\")",
    ),
    // kỳ vọng: out("0\n") — `T?` + match some/none
    (
        "let x: int? = none\nmatch x:\n    some(v): print(v)\n    none: print(0)",
        "out(\"0\\n\")",
    ),
    // kỳ vọng: out("5\n") — `??` trên `some`
    ("let x: int? = 5\nprint(x ?? 0)", "out(\"5\\n\")"),
    // kỳ vọng: out("7\n") — `??` trên `none`
    ("let y: int? = none\nprint(y ?? 7)", "out(\"7\\n\")"),
    // kỳ vọng: out("âm\n") — `?` trên Result (trả sớm `Err`)
    (
        "fn f(n: int) -> Result[int, str]:\n    let r = if n > 0: Ok(n) else: Err(\"âm\")\n    r\nfn g(n: int) -> Result[int, str]:\n    let v = f(n)?\n    Ok(v * 2)\nmatch g(-1):\n    Ok(x): print(x)\n    Err(e): print(e)",
        "out(\"âm\\n\")",
    ),
    // kỳ vọng: out("7\n") — `?` trên `T?`
    (
        "fn first(xs: list[int]) -> int?:\n    let a = xs.first()?\n    a\nprint(first([7]) ?? 0)",
        "out(\"7\\n\")",
    ),
    // kỳ vọng: out("4\n") — `|>`
    ("fn inc(x: int) -> int:\n    x + 1\nprint(3 |> inc)", "out(\"4\\n\")"),
    // kỳ vọng: out("true\n") — `try` giữ giá trị ngoài `Result`
    (
        "fn f() -> Result[int, str]:\n    Ok(1)\nmatch f():\n    Ok(v): print(v == 1)\n    Err(e): print(false)",
        "out(\"true\\n\")",
    ),
];

// ---------------------------------------------------------------------------
// 6) Giá trị của khối theo VD-47: `if`/`match` câu lệnh **cuối** phải cho giá trị.
// ---------------------------------------------------------------------------

const GIA_TRI_DUOI: &[(&str, &str)] = &[
    // kỳ vọng: out("1\n") — `if` câu lệnh cuối thân hàm cho giá trị (§4/§6, VD-47)
    (
        "fn f(x: int) -> int:\n    if x > 0: 1 else: 2\nprint(f(1))",
        "out(\"1\\n\")",
    ),
    // kỳ vọng: out("2\n")
    (
        "fn f(x: int) -> int:\n    if x > 0: 1 else: 2\nprint(f(-1))",
        "out(\"2\\n\")",
    ),
    // kỳ vọng: out("10\n") — `match` câu lệnh cuối cho giá trị
    (
        "fn f(x: int) -> int:\n    match x:\n        1: 10\n        _: 20\nprint(f(1))",
        "out(\"10\\n\")",
    ),
    // kỳ vọng: out("()\n") — `if` **không** `else` ở đuôi ⇒ `()`
    (
        "fn f(x: int):\n    if x > 0: 1\nprint(f(1))",
        "out(\"()\\n\")",
    ),
    // kỳ vọng: out("10\n") — `if` lồng ở đuôi khối con
    (
        "fn f(x: int) -> int:\n    if x > 0:\n        if x > 1: 10 else: 20\n    else: 30\nprint(f(5))",
        "out(\"10\\n\")",
    ),
];

#[test]
fn gia_tri_cua_khoi_theo_vd47() {
    kiem("giá trị đuôi", GIA_TRI_DUOI);
}

#[test]
fn so_hoc_va_don_vi() {
    kiem("số học", SO_HOC);
}

#[test]
fn dieu_khien_va_khai_bao() {
    kiem("điều khiển", DIEU_KHIEN);
}

#[test]
fn ham_closure_va_tham_so() {
    kiem("hàm", HAM);
}

#[test]
fn tap_hop_va_phuong_thuc() {
    kiem("tập hợp", TAP_HOP);
}

#[test]
fn kieu_du_lieu_va_match() {
    kiem("kiểu", KIEU);
}

/// `RusOutput` in `line:col` **thật** từ `span` + văn bản (VD-46 bắt buộc ở tầng này).
#[test]
fn chan_doan_runtime_co_dong_cot_that() {
    let src = "let xs = [1]\nprint(xs[3])";
    let out = run_src(src);
    assert_eq!(out.codes(), ["VS0503"]);
    let d = &out.diagnostics[0];
    assert_eq!((d.line, d.col), (2, 7), "dòng 2, cột 7 — chỗ `xs[3]`");
    assert!(d.message.contains("ngoài phạm vi"), "{}", d.message);
    // Lỗi chia 0 ở dòng 1.
    let out = run_src("print(1 / 0)");
    let d = &out.diagnostics[0];
    assert_eq!((d.line, d.col), (1, 7), "cột 7 — chỗ `1 / 0`");
}

/// Phòng thủ: phương thức **ngoài tập đã khoá** ⇒ `VS0501`, **không panic**. Ca này **bỏ** tyck
/// (pipeline thật chặn ở tyck bằng `VS0403`) — kiểm hành vi của riêng interp.
#[test]
fn phuong_thuc_ngoai_tap_bao_vs0501() {
    let src = "let xs = [1]\nprint(xs.sort())";
    let p = parse(src, Mode::Script);
    let h = lower(&p.cst, &lex(src, Mode::Script));
    let out = run(src, &h.hir, &h.symbols, &h.types, &[]);
    assert_eq!(out.codes(), ["VS0501"], "{:?}", out.diagnostics);
    assert!(
        out.diagnostics[0].message.contains("chưa hỗ trợ"),
        "{:?}",
        out.diagnostics[0].message
    );
}

/// Tiêu chí ST-13: ≥ **60** ca — giảm là đỏ.
#[test]
fn du_60_ca() {
    let n = SO_HOC.len()
        + DIEU_KHIEN.len()
        + HAM.len()
        + TAP_HOP.len()
        + KIEU.len()
        + GIA_TRI_DUOI.len();
    assert!(n >= 60, "bảng ca phải ≥ 60, thật {n}");
}
