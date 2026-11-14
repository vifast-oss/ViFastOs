// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-11 — test ĐỎ TRƯỚC (D2-05)** của `vis-types`: hạ CST → HIR.
//!
//! Tệp này viết **trước** khi hiện thực `lower()` (log đỏ: `viscript/agent/JOURNAL/st11-red-types.log`);
//! kỳ vọng lấy từ **nguồn của Agent 0**, không tự đặt:
//! - **15 khối ```vis đủ thân** trong `docs/viscript/ViScript-thiet-ke.md` (VD-36) — parse sạch
//!   (ST-05) **và** hạ **0 chẩn đoán**, đúng số **item / câu lệnh cấp tệp / khai báo**;
//! - **30 golden biểu thức** trong `viscript/test/expr/golden.expr` (VD-34, Agent 0 soạn, **chỉ
//!   đọc**): 27 ca hợp lệ phải hạ ra đúng **sexp HIR**; 3 ca `!ERR` dừng ở **parse**.
//!
//! # Quy ước **sexp HIR** (dùng cho mọi ca)
//!
//! Cùng quy ước với `golden.expr` (VD-34) — literal giữ **nguyên văn nguồn** (`1`, `"a"`, `50pct`),
//! nhị phân `(op lhs rhs)`, một ngôi `(op x)`, postfix `(field X ten)` / `(?field X ten)` /
//! `(call f a b)` / `(index X i)` / `(try X)` / `(with X ten: v)`, cấu trúc `(list …)`, `(set …)`,
//! `(map k v)`, `(record ten: v)`, `(tuple …)` — **trừ đúng một điểm**:
//!
//! > HIR **không có** nút `paren`: ngoặc tròn là chuyện của CST. Vì vậy `(1 + 2) * 3` ở CST là
//! > `(* (paren (+ 1 2)) 3)` thì ở HIR là `(* (+ 1 2) 3)`.
//!
//! Bộ `30 golden` là kỳ vọng của **Agent 0**; test này chỉ **bỏ lớp `(paren …)`** rồi so với
//! `HExpr::sexp()` của HIR — không chép tay lại kỳ vọng.

use std::path::{Path, PathBuf};

use vis_syntax::{lex, parse, Mode};
use vis_types::{lower, HExpr, HItem, HStmt, HirOutput, LitKind, Type};

// ---------------------------------------------------------------------------
// Hạ một nguồn
// ---------------------------------------------------------------------------

/// Parse + hạ một nguồn **sạch** (assert 0 chẩn đoán parse ở mức test).
fn lower_src(src: &str) -> HirOutput {
    let p = parse(src, Mode::Script);
    assert!(
        p.diagnostics.is_empty(),
        "nguồn phải parse sạch, thật {} chẩn đoán: {:?}",
        p.diagnostics.len(),
        p.diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect::<Vec<_>>()
    );
    lower(&p.cst, &lex(src, Mode::Script))
}

fn lower_and_check(src: &str) -> HirOutput {
    let h = lower_src(src);
    assert!(
        h.is_clean(),
        "hạ phải 0 chẩn đoán, thật {:?}: {:?}",
        h.codes(),
        h.diagnostics
            .iter()
            .map(|d| format!("{} {}:{} {}", d.code.short, d.line, d.col, d.message))
            .collect::<Vec<_>>()
    );
    h
}

// ---------------------------------------------------------------------------
// 1) 15 khối tài liệu đủ thân ⇒ 0 chẩn đoán + đúng số item/khai báo
// ---------------------------------------------------------------------------

/// Tổng số khối ```vis trong tài liệu (ST-05/ST-09 khoá 22).
const TOTAL_VIS_BLOCKS: usize = 22;
/// Số khối **đủ thân** (VD-36).
const FULL_BODIED: usize = 15;
/// Số khối **loại trừ tường minh** (VD-36: 6 khối `...` + khối 1412).
const EXCLUDED_COUNT: usize = 7;

/// Khối loại trừ — (dòng mở ```vis, lý do). Trùng `docs/viscript/doc-examples-allow.txt`
/// (7 mục, mỗi mục **thật đỏ** khi parse) + `tests/doc_fixtures.rs` của ST-05.
const EXCLUDED: [(usize, &str); EXCLUDED_COUNT] = [
    (
        305,
        "`...` ở 312-315 (`for i in 0..10: ...`) — QD §7.4 cấm `...` trong fixture",
    ),
    (
        432,
        "`...` ở 436 (`fn max_by[...] -> T?: ...`) — thân hàm lược",
    ),
    (445, "`...` ở 450 (`fn show(self) -> str: ...`) — thân hàm lược"),
    (610, "`...` ở 613 (`pub cmd ls(...): ...`) — thân hàm lược"),
    (
        662,
        "`...` ở 663 (`extern cmd git(..., ...rest: str)`) — không có token `...` trong SPEC-lex §1",
    ),
    (737, "`...` ở 739 (`fn main(...): ...`) — thân hàm lược"),
    (
        1412,
        "`Entry(trường: …)` ở 1429 + `assert` ở 1438 — hai lỗ hổng tài liệu (VD-36), không tự thêm cú pháp",
    ),
];

/// (dòng mở khối, số item, số câu lệnh cấp tệp, số **khai báo** ngoài 8 tên dựng sẵn).
///
/// Số khai báo = ký hiệu `Symbols` **trừ** `SymKind::Builtin`: item cấp tệp + tham số + `let`/`var`
/// + binding pattern + generic + nhãn `scope`. Đếm tay từ nguồn khối, không lấy từ log.
const DOC_BLOCKS: [(usize, usize, usize, usize); FULL_BODIED] = [
    // **SỬA KÈM LÝ DO (ST-57)**: `derives Eq, Show` **sinh** `impl` ⇒ +1 item (`HImpl`) và +5
    // khai báo (`User.show` + `self`, `User.eq` + `self` + `other`).
    // **ST-60c**: `enum Shape derives Show` nay **sinh** `impl` thật ⇒ +1 item, +2 sym (show + self).
    (214, 6, 0, 11), // type User derives Eq/Show + enum Shape derives Show + alias + newtype
    (238, 1, 0, 4),  // fn total + R + rows + r (lambda)
    (290, 0, 5, 6),  // x, count, a, b, name, age
    (325, 0, 1, 4),  // r (arm1), r (arm2 = or-pattern dùng chung), w, h
    (342, 0, 3, 3),  // e, domain, mail (if let)
    (406, 1, 0, 5),  // greet + name + times + loud + s
    (422, 0, 1, 3),  // f + x (lambda) + y
    (473, 1, 0, 4),  // read_config + dir + text + cfg
    (488, 0, 1, 2),  // cfg + err (else)
    (535, 0, 3, 2),  // a, b
    (549, 0, 2, 2),  // counter, inc
    (571, 0, 0, 0),  // chỉ có `#!` + `#|` (chú thích)
    (684, 1, 0, 5),  // backup_big + ctx + dir + files + f
    (698, 1, 0, 7),  // fetch_all + net + urls + s (scope) + tasks + u + t
    (1459, 1, 0, 4), // tính_tổng + danh_sách + tổng + số
];

fn doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/viscript/ViScript-thiet-ke.md")
}

/// Trích **tại chỗ** mọi khối ```vis của tài liệu (không chép tay nội dung).
fn doc_blocks() -> Vec<(usize, String)> {
    let path = doc_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    let mut out = Vec::new();
    let mut it = text.lines().enumerate();
    while let Some((idx, line)) = it.next() {
        if line.trim_end() == "```vis" {
            let mut body = String::new();
            for (_, l) in it.by_ref() {
                if l.trim_end() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            out.push((idx + 1, body));
        }
    }
    out
}

#[test]
fn khoi_tai_lieu_22_15_7() {
    let blocks = doc_blocks();
    assert_eq!(
        blocks.len(),
        TOTAL_VIS_BLOCKS,
        "tài liệu phải có đúng {TOTAL_VIS_BLOCKS} khối ```vis (VD-36); thêm/bớt là đỏ"
    );
    let excluded: Vec<usize> = blocks
        .iter()
        .map(|(l, _)| *l)
        .filter(|l| EXCLUDED.iter().any(|(x, _)| x == l))
        .collect();
    assert_eq!(
        excluded,
        EXCLUDED.iter().map(|(l, _)| *l).collect::<Vec<_>>(),
        "danh sách khối loại trừ phải khớp đúng dòng"
    );
    assert_eq!(blocks.len() - EXCLUDED.len(), FULL_BODIED);
}

#[test]
fn ha_15_khoi_tai_lieu_0_chan_doan_va_dung_so_khai_bao() {
    let blocks = doc_blocks();
    let mut checked = 0usize;
    for (line, body) in &blocks {
        if EXCLUDED.iter().any(|(l, _)| l == line) {
            continue;
        }
        let want = DOC_BLOCKS
            .iter()
            .find(|(l, _, _, _)| l == line)
            .unwrap_or_else(|| panic!("khối dòng {line} chưa có số kỳ vọng trong DOC_BLOCKS"));
        let p = parse(body, Mode::Script);
        assert!(
            p.diagnostics.is_empty(),
            "khối {line} phải parse sạch (ST-05), thật {:?}",
            p.diagnostics
                .iter()
                .map(|d| d.code_str())
                .collect::<Vec<_>>()
        );
        // Bất biến CST lossless vẫn giữ (không nới hợp đồng ST-05).
        assert_eq!(p.cst.render(&p.text), p.text, "khối {line} mất lossless");
        let h = lower(&p.cst, &lex(body, Mode::Script));
        assert!(
            h.is_clean(),
            "khối {line} phải hạ **0 chẩn đoán** (VD-44 #1), thật {:?}: {:?}",
            h.codes(),
            h.diagnostics
                .iter()
                .map(|d| format!("{} {}:{} {}", d.code.short, d.line, d.col, d.message))
                .collect::<Vec<_>>()
        );
        let decls = h
            .symbols
            .syms
            .iter()
            .filter(|s| s.kind != vis_types::SymKind::Builtin)
            .count();
        assert_eq!(
            (h.hir.items.len(), h.hir.stmts.len(), decls),
            (want.1, want.2, want.3),
            "khối {line}: (item, câu lệnh, khai báo) sai"
        );
        checked += 1;
    }
    assert_eq!(checked, FULL_BODIED, "phải hạ đủ {FULL_BODIED} khối");
}

// ---------------------------------------------------------------------------
// 2) 30 golden biểu thức ⇒ sexp HIR (bỏ lớp `paren` của CST)
// ---------------------------------------------------------------------------

const GOLDEN_CASES: usize = 30;
const GOLDEN_OK: usize = 27;
const GOLDEN_ERR: usize = 3;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/expr/golden.expr")
}

#[derive(Debug, Clone, PartialEq)]
enum Sexp {
    Atom(String),
    List(Vec<Sexp>),
}

fn parse_sexp(s: &str) -> Sexp {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    fn go(b: &[char], i: &mut usize) -> Sexp {
        while *i < b.len() && b[*i].is_whitespace() {
            *i += 1;
        }
        if *i >= b.len() {
            return Sexp::Atom(String::new());
        }
        if b[*i] == '(' {
            *i += 1;
            let mut v = Vec::new();
            loop {
                while *i < b.len() && b[*i].is_whitespace() {
                    *i += 1;
                }
                if *i >= b.len() {
                    break;
                }
                if b[*i] == ')' {
                    *i += 1;
                    break;
                }
                v.push(go(b, i));
            }
            return Sexp::List(v);
        }
        let start = *i;
        if b[*i] == '"' {
            *i += 1;
            while *i < b.len() && b[*i] != '"' {
                *i += 1;
            }
            *i = (*i + 1).min(b.len());
        } else {
            while *i < b.len() && !b[*i].is_whitespace() && b[*i] != '(' && b[*i] != ')' {
                *i += 1;
            }
        }
        Sexp::Atom(b[start..*i].iter().collect())
    }
    go(&b, &mut i)
}

/// Bỏ **đúng một** lớp `(paren X)` ⇒ `X` (HIR không có nút `paren`).
fn elide_paren(s: Sexp) -> Sexp {
    match s {
        Sexp::List(v) => {
            if v.len() == 2 && v[0] == Sexp::Atom("paren".into()) {
                elide_paren(v[1].clone())
            } else {
                Sexp::List(v.into_iter().map(elide_paren).collect())
            }
        }
        a => a,
    }
}

fn render_sexp(s: &Sexp) -> String {
    match s {
        Sexp::Atom(a) => a.clone(),
        Sexp::List(v) => {
            let inner: Vec<String> = v.iter().map(render_sexp).collect();
            format!("({})", inner.join(" "))
        }
    }
}

/// Đọc `golden.expr` (của Agent 0, **chỉ đọc**): `(ca hợp lệ, ca !ERR)`.
///
/// Phần tử: `(dòng, nguồn, kỳ vọng)` — kỳ vọng là sexp với ca hợp lệ, là **chuỗi gợi ý** (có thể
/// rỗng) với ca `!ERR`.
type GoldenCase = (usize, String, String);

fn golden_cases() -> (Vec<GoldenCase>, Vec<GoldenCase>) {
    let path = golden_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    let mut ok = Vec::new();
    let mut err = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let (src, rhs) = line
            .split_once(" => ")
            .unwrap_or_else(|| panic!("golden.expr:{line_no}: thiếu ' => '"));
        let src = src.trim().to_string();
        let rhs = rhs.trim();
        match rhs.strip_prefix("!ERR") {
            Some(rest) => err.push((line_no, src, rest.trim().trim_matches('"').to_string())),
            None => ok.push((line_no, src, rhs.to_string())),
        }
    }
    assert_eq!(
        ok.len() + err.len(),
        GOLDEN_CASES,
        "golden.expr phải đúng {GOLDEN_CASES} ca"
    );
    (ok, err)
}

/// 27 ca **hợp lệ**: hạ rồi so `HExpr::sexp()` với kỳ vọng của Agent 0 (bỏ lớp `paren`).
#[test]
fn golden_27_bieu_thuc_ha_ra_sexp_hir() {
    let (cases, errs) = golden_cases();
    assert_eq!(cases.len(), GOLDEN_OK, "phải có đúng {GOLDEN_OK} ca hợp lệ");
    assert_eq!(
        errs.len(),
        GOLDEN_ERR,
        "phải có đúng {GOLDEN_ERR} ca `!ERR`"
    );
    for (line_no, src, rhs) in cases {
        let want = render_sexp(&elide_paren(parse_sexp(&rhs)));
        let p = parse(&src, Mode::Script);
        assert!(
            p.diagnostics.is_empty(),
            "golden.expr:{line_no} `{src}` phải parse sạch khi hạ"
        );
        let h = lower(&p.cst, &lex(&src, Mode::Script));
        assert!(
            h.is_clean(),
            "golden.expr:{line_no} `{src}` hạ phải 0 chẩn đoán, thật {:?}",
            h.codes()
        );
        let got = h
            .hir
            .stmts
            .iter()
            .find_map(|s| match s {
                HStmt::Expr(e) => Some(e.sexp()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("golden.expr:{line_no} `{src}` không ra biểu thức"));
        assert_eq!(got, want, "golden.expr:{line_no} `{src}`: sexp HIR sai");
    }
}

/// 3 ca `!ERR` (VD-44 #4): **dừng ở parse** — không lower, không đòi chẩn đoán hạ.
///
/// Test này **cố ý xanh trước khi hiện thực `lower`** (nó chỉ chạm parser của ST-05); ghi rõ ở
/// đầu log đỏ để không ai tưởng nó là ca hạ.
#[test]
fn ba_ca_err_dung_o_parse_khong_lower() {
    let (_, errs) = golden_cases();
    assert_eq!(errs.len(), GOLDEN_ERR);
    for (line_no, src, hint) in errs {
        let p = parse(&src, Mode::Script);
        assert_eq!(
            p.diagnostics.len(),
            1,
            "golden.expr:{line_no} `{src}` phải đúng **1** lỗi parse (VD-35)"
        );
        if !hint.is_empty() {
            assert!(
                p.diagnostics[0].message.contains(&hint),
                "golden.expr:{line_no}: thông điệp `{}` phải chứa `{hint}`",
                p.diagnostics[0].message
            );
        }
        // Không gọi `lower` cho ca `!ERR` — nếu parser hỏng thì hạ là vô nghĩa.
    }
}

// ---------------------------------------------------------------------------
// 3) Điền kiểu literal (VD-43) + `T?` + `none`
// ---------------------------------------------------------------------------

fn expr_of(src: &str) -> HExpr {
    let h = lower_and_check(src);
    match h.hir.stmts.as_slice() {
        [HStmt::Expr(e)] => e.clone(),
        other => panic!("`{src}` phải là đúng một biểu thức, thật {other:?}"),
    }
}

#[test]
fn literal_duoc_dien_kieu() {
    let cases: [(&str, LitKind, Type); 12] = [
        ("1", LitKind::Int, Type::Int),
        ("1.5", LitKind::Float, Type::Float),
        ("5mb", LitKind::Size, Type::Size),
        ("250ms", LitKind::Duration, Type::Duration),
        ("50pct", LitKind::Percent, Type::Percent),
        ("\"a\"", LitKind::Str, Type::Str),
        ("r\"a\"", LitKind::Str, Type::Str),
        ("b\"a\"", LitKind::Bytes, Type::Bytes),
        ("p\"./x\"", LitKind::Path, Type::Path),
        ("'a'", LitKind::Char, Type::Char),
        ("true", LitKind::Bool, Type::Bool),
        ("false", LitKind::Bool, Type::Bool),
    ];
    for (src, kind, ty) in cases {
        match expr_of(src) {
            HExpr::Lit {
                lit,
                ty: got,
                span: _,
            } => {
                assert_eq!(lit.kind, kind, "`{src}`: LitKind sai");
                assert_eq!(got, ty, "`{src}`: kiểu literal sai");
                assert_eq!(lit.text, src, "`{src}`: phải giữ nguyên văn");
            }
            other => panic!("`{src}` phải là literal, thật {other:?}"),
        }
    }
}

#[test]
fn kieu_cau_truc_va_option() {
    // list[int]
    match expr_of("[1, 2, 3]") {
        HExpr::List { items, elem, .. } => {
            assert_eq!(items.len(), 3);
            assert_eq!(elem, Type::Int);
        }
        other => panic!("phải là list, thật {other:?}"),
    }
    // map[str, int]
    match expr_of("{\"a\": 1}") {
        HExpr::Map { key, value, .. } => {
            assert_eq!((key, value), (Type::Str, Type::Int));
        }
        other => panic!("phải là map, thật {other:?}"),
    }
    // bản ghi ẩn danh: Record(RecId) có trường + kiểu
    let h = lower_and_check("{name: \"An\", age: 20}");
    let HStmt::Expr(HExpr::Record { fields, ty, .. }) = &h.hir.stmts[0] else {
        panic!("phải là record");
    };
    assert_eq!(fields.len(), 2);
    let Type::Record(id) = ty else {
        panic!("record literal phải có kiểu Record(RecId), thật {ty:?}");
    };
    let def = h.types.rec(*id).expect("phải có RecDef");
    assert_eq!(
        def.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["name", "age"]
    );
    assert_eq!(def.fields[0].ty, Type::Str);
    assert_eq!(def.fields[1].ty, Type::Int);
    assert!(def.anonymous);
    // `none` ⇒ Opt(None); `xs[0]?` ⇒ Try
    match expr_of("none") {
        HExpr::Opt {
            value: None, ty, ..
        } => assert!(ty.is_opt(), "`none` phải có kiểu `T?`"),
        other => panic!("`none` phải là Opt(None), thật {other:?}"),
    }
    match expr_of("xs[0]?") {
        HExpr::Try { expr, .. } => assert!(matches!(*expr, HExpr::Index { .. })),
        other => panic!("phải là Try(Index), thật {other:?}"),
    }
    // `T?` trong chú thích kiểu
    let h = lower_and_check("let x: int? = 5\n");
    let HStmt::Let { ty, .. } = &h.hir.stmts[0] else {
        panic!("phải là let");
    };
    assert_eq!(ty.clone(), Some(Type::Opt(Box::new(Type::Int))));
}

#[test]
fn kieu_hien_thi_on_dinh_va_type_ref() {
    // VD-44 #2: tên kiểu chưa khai giữ dạng `TypeRef`, KHÔNG báo chẩn đoán.
    let h = lower_and_check("let x: Config = y\nlet z: cap[Dir] = w\nlet f: fn(int) -> bool = g\nlet t: list[map[str, int]] = h\n");
    let tys: Vec<Option<Type>> = h
        .hir
        .stmts
        .iter()
        .map(|s| match s {
            HStmt::Let { ty, .. } => ty.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(
        tys[0],
        Some(Type::TypeRef {
            name: "Config".into(),
            args: vec![]
        })
    );
    assert_eq!(
        tys[1],
        Some(Type::TypeRef {
            name: "cap".into(),
            args: vec![Type::TypeRef {
                name: "Dir".into(),
                args: vec![]
            }]
        })
    );
    assert_eq!(
        tys[2],
        Some(Type::Fn(
            vec![Type::Int].into_boxed_slice(),
            Box::new(Type::Bool)
        ))
    );
    assert_eq!(
        tys[3],
        Some(Type::List(Box::new(Type::Map(
            Box::new(Type::Str),
            Box::new(Type::Int)
        ))))
    );
    // Display ổn định
    assert_eq!(Type::Opt(Box::new(Type::Int)).to_string(), "int?");
    assert_eq!(tys[3].as_ref().unwrap().to_string(), "list[map[str, int]]");
    // `type User` khai trong tệp ⇒ Record(RecId), tên hiển thị theo bảng kiểu
    let h = lower_and_check("type User:\n    name: str\nlet u: User = v\n");
    let HItem::Rec(rec) = &h.hir.items[0] else {
        panic!("phải là bản ghi");
    };
    assert_eq!(h.types.show(&Type::Record(rec.id)), "User");
    assert_eq!(h.types.rec(rec.id).unwrap().fields[0].name, "name");
}

// ---------------------------------------------------------------------------
// 4) Không panic trên khối loại trừ + rác; chế độ lệnh trả lỗi tường minh
// ---------------------------------------------------------------------------

/// Không panic trên 7 khối loại trừ + rác, **và** chứng minh `lower` thật sự chạy.
///
/// Ca đối chứng ở cuối là điều kiện để test này **đỏ khi `lower` còn là khung rỗng**: nếu chỉ
/// khẳng định "không panic" thì một hàm rỗng cũng xanh (test hình thức) — D2-05 không chấp nhận.
#[test]
fn khong_panic_tren_7_khoi_loai_tru_va_rac() {
    let blocks = doc_blocks();
    for (line, body) in &blocks {
        if !EXCLUDED.iter().any(|(l, _)| l == line) {
            continue;
        }
        // Parse lỗi là chuyện của ST-05 — chỉ cần `lower` **không panic** (VD-44 #6).
        let p = parse(body, Mode::Script);
        let h = lower(&p.cst, &lex(body, Mode::Script));
        let _ = (h.hir.items.len(), h.hir.stmts.len(), h.codes());
    }
    for junk in [
        "",
        "let x =",
        "((((",
        "fn f(:\n",
        "match x:\n",
        "let x = \"chưa đóng\n",
        "if",
        "type T:\n",
        "enum E:\n",
        "    V(\n",
        "\u{0}\u{1}",
    ] {
        let p = parse(junk, Mode::Script);
        let h = lower(&p.cst, &lex(junk, Mode::Script));
        // Không panic là tiêu chí; chẩn đoán (nếu có) phải là mã đã cấp.
        for d in &h.diagnostics {
            assert!(
                vis_diag::lookup(d.code.id).is_some(),
                "chẩn đoán phải dùng mã đã cấp: {}",
                d.code.short
            );
        }
    }
    // Ca ĐỐI CHỨNG: nguồn hợp lệ phải hạ ra đúng 1 câu lệnh ⇒ chứng minh `lower` chạy thật.
    let h = lower_and_check("let x = 1\n");
    assert_eq!(
        h.hir.stmts.len(),
        1,
        "ca đối chứng: `lower` phải hạ được `let x = 1` (không phải khung rỗng)"
    );
    match &h.hir.stmts[0] {
        HStmt::Let {
            pat: vis_types::HPat::Bind { name, .. },
            ..
        } => assert_eq!(name, "x"),
        other => panic!("ca đối chứng: phải là `let x`, thật {other:?}"),
    }
}

#[test]
fn cst_che_do_lenh_ha_that_qua_cmd_lower() {
    // **SỬA KÈM LÝ DO (ST-18)**: VD-22 pha 2 đã hiện thực (`cmd_lower`) ⇒ `lower` trên CST chế độ
    // lệnh **hạ thật** (không còn `LowerError::CommandMode`). Giữ nguyên tính chất "không panic";
    // ca lệnh ngoài bảng VS0 (`ls`) nay cho **chẩn đoán `VS0601`** thay vì lỗi cấp API.
    let src = "echo \"a\"\n";
    let p = parse(src, Mode::Command);
    let h = lower(&p.cst, &lex(src, Mode::Command));
    assert!(h.error.is_none(), "không còn lỗi cấp API: {:?}", h.error);
    assert_eq!(h.hir.stmts.len(), 1, "một dòng lệnh ⇒ một `HStmt`");

    let p2 = parse("ls | grep foo\n", Mode::Command);
    let h2 = lower(&p2.cst, &lex("ls | grep foo\n", Mode::Command));
    assert!(
        h2.error.is_none(),
        "không panic/lỗi cấp API: {:?}",
        h2.error
    );
    // **SỬA KÈM LÝ DO (ST-27)**: lệnh lạ ⇒ **chương trình ngoài** (§9.3) — hạ thành call
    // `exec.<tên>`, **không** còn `VS0601`; lỗi (nếu có) chỉ đến lúc chạy.
    assert!(
        h2.diagnostics.is_empty(),
        "lệnh lạ không còn chẩn đoán tĩnh: {:?}",
        h2.diagnostics
    );
    // Nợ VS1b (JOURNAL ST-27): **stage ngoài đứng sau pipe** — giá trị pipe chưa nối vào `stdin`
    // của tiến trình kế (thiết kế §9.5 dùng `stream[bytes]`), nên HIR chỉ còn stage cuối.
    let s2 = format!("{:?}", h2.hir);
    assert!(
        s2.contains("exec.grep"),
        "phải hạ stage cuối thành `exec.<tên>`: {s2}"
    );
}

// ---------------------------------------------------------------------------
// 5) Đường dẫn tài liệu phải tồn tại (test không im lặng bỏ qua)
// ---------------------------------------------------------------------------

#[test]
fn doc_va_golden_ton_tai() {
    assert!(Path::new(&doc_path()).is_file(), "thiếu tài liệu thiết kế");
    assert!(Path::new(&golden_path()).is_file(), "thiếu golden.expr");
}
