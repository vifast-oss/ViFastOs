// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-11 — test ĐỎ TRƯỚC (D2-05)** của phần **giải tên** (`lower`).
//!
//! Khoá đúng hợp đồng VD-44 #2/#3:
//! - khai lại **cùng khung** ⇒ `VS0302` (kể cả tham số trùng, `let` trùng, item trùng);
//! - **shadowing** ở khung trong là hợp lệ, kiểu được phép khác (§4.1);
//! - tên **giá trị** chưa khai ⇒ `VS0301` — mặc định chỉ ở chỗ chắc chắn sai (**gán** cho tên
//!   chưa khai), và ở **mọi** lần dùng khi bật `NamePolicy::Strict`;
//! - tên **kiểu** chưa khai **không** báo `VS0301` ở ST-11 (ST-12 làm) — giữ `Type::TypeRef`;
//! - **8 tên giá trị dựng sẵn** khoá bằng test (VD-44 #3: thêm/bớt phải qua Agent 0).

use vis_syntax::{lex, parse, Mode};
use vis_types::{
    lower_with, HExpr, HItem, HPat, HStmt, HirOutput, LowerOptions, SymKind, BUILTIN_VALUE_NAMES,
};

fn lower_src(src: &str, opts: LowerOptions) -> HirOutput {
    let p = parse(src, Mode::Script);
    assert!(
        p.diagnostics.is_empty(),
        "nguồn phải parse sạch: {:?}",
        p.diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect::<Vec<_>>()
    );
    lower_with(&p.cst, &lex(src, Mode::Script), opts)
}

fn free(src: &str) -> HirOutput {
    lower_src(src, LowerOptions::default())
}

fn strict(src: &str) -> HirOutput {
    lower_src(src, LowerOptions::strict())
}

// ---------------------------------------------------------------------------
// VS0302 — khai lại cùng khung
// ---------------------------------------------------------------------------

#[test]
fn vs0302_let_trung_cung_khung() {
    let h = free("let x = 1\nlet x = 2\n");
    assert_eq!(h.codes(), ["VS0302"], "hai `let x` cùng khung phải VS0302");
    assert!(
        h.diagnostics[0].message.contains("x"),
        "thông điệp phải nêu tên: {}",
        h.diagnostics[0].message
    );
    assert!(
        h.diagnostics[0].message.contains("trùng"),
        "thông điệp phải nói 'trùng': {}",
        h.diagnostics[0].message
    );
    // Vị trí: báo chỗ khai **thứ hai**.
    assert_eq!(h.diagnostics[0].line, 2);
}

#[test]
fn vs0302_tham_so_trung_va_item_trung() {
    let h = free("fn f(a: int, a: int) -> int:\n    a\n");
    assert_eq!(h.codes(), ["VS0302"], "hai tham số cùng tên ⇒ VS0302");

    let h = free("fn f() -> int:\n    1\n\nfn f() -> int:\n    2\n");
    assert_eq!(h.codes(), ["VS0302"], "hai `fn f` cùng tệp ⇒ VS0302");

    let h = free("fn f() -> int:\n    1\n\nlet f = 2\n");
    assert_eq!(
        h.codes(),
        ["VS0302"],
        "tên hàm và `let` cùng khung ⇒ VS0302 (§4.1 'cùng tên trong cùng scope')"
    );
}

// ---------------------------------------------------------------------------
// Shadowing hợp lệ + khung
// ---------------------------------------------------------------------------

#[test]
fn bong_ten_trong_khoi_hop_le_va_phan_giai_dung() {
    // §4.1: `let x = 1` ngoài, `let x = "a"` trong khối là hợp lệ, **kiểu khác được phép**.
    let h = free("let x = 1\nif true:\n    let x = \"a\"\n    x\n");
    assert!(
        h.is_clean(),
        "shadowing ở khối trong phải hợp lệ, thật {:?}",
        h.codes()
    );
    let xs: Vec<&vis_types::HSym> = h.symbols.all_named("x").collect();
    assert_eq!(xs.len(), 2, "phải có 2 khai báo `x` (ngoài + trong)");
    assert_ne!(
        xs[0].frame, xs[1].frame,
        "hai `x` phải ở hai khung khác nhau"
    );
    // `x` trong khối phải phân giải về khai báo **trong** (frame sâu hơn).
    let HStmt::If { then, .. } = &h.hir.stmts[1] else {
        panic!("câu lệnh thứ hai phải là `if`");
    };
    let HStmt::Expr(HExpr::Var { sym, .. }) = &then[1] else {
        panic!("thân `if` phải có biểu thức `x`");
    };
    let sym = h.symbols.get(sym.expect("phải phân giải")).unwrap();
    assert_eq!(sym.name, "x");
    assert_eq!(sym.frame, xs[1].frame, "phải trỏ khai báo trong khối");
}

#[test]
fn tham_so_thay_trong_than_ham() {
    let h = free("fn f(a: int) -> int:\n    a\n");
    let HItem::Fn(f) = &h.hir.items[0] else {
        panic!("phải là hàm");
    };
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].ty, vis_types::Type::Int);
    let HStmt::Expr(HExpr::Var { name, sym, .. }) = &f.body[0] else {
        panic!("thân hàm phải là biểu thức `a`");
    };
    assert_eq!(name, "a");
    let s = h.symbols.get(sym.expect("phải phân giải")).unwrap();
    assert_eq!(s.kind, SymKind::Param);
    assert_eq!(
        s.frame,
        h.symbols.get(f.params[0].sym).unwrap().frame,
        "biến `a` phải trỏ đúng ký hiệu tham số"
    );
}

#[test]
fn let_trong_khoi_khong_ro_ra_ngoai() {
    // Ra khỏi khối thì `y` không còn: Strict mode báo VS0301 ⇒ chứng minh khung đã đóng.
    let h = strict("if true:\n    let y = 1\ny\n");
    assert_eq!(h.codes(), ["VS0301"], "sau khối, `y` phải không phân giải");
    assert!(h.diagnostics[0].message.contains("y"));
    // Trong khối thì phân giải được (Free mode ⇒ 0 chẩn đoán).
    let h = free("if true:\n    let y = 1\n    y\n");
    assert!(
        h.is_clean(),
        "trong khối `y` phải phân giải: {:?}",
        h.codes()
    );
}

#[test]
fn ham_goi_truoc_va_sau_khai_bao() {
    // §14.3: phân giải **hai lượt** (khai báo trước, thân sau).
    let h = free("fn g() -> int:\n    f()\n\nfn f() -> int:\n    1\n");
    assert!(
        h.is_clean(),
        "hàm gọi hàm khai **sau** phải phân giải: {:?}",
        h.codes()
    );
    let HItem::Fn(g) = &h.hir.items[0] else {
        panic!("item đầu phải là `g`");
    };
    let HStmt::Expr(HExpr::Call { callee, .. }) = &g.body[0] else {
        panic!("thân `g` phải là lời gọi");
    };
    let HExpr::Var { name, sym, .. } = &**callee else {
        panic!("callee phải là tên");
    };
    assert_eq!(name, "f");
    let s = h.symbols.get(sym.expect("phải phân giải")).unwrap();
    assert_eq!(s.kind, SymKind::Item, "`f` phải trỏ khai báo cấp tệp");

    // Khai trước rồi gọi sau — cùng kết quả.
    let h = free("fn f() -> int:\n    1\n\nfn g() -> int:\n    f()\n");
    assert!(h.is_clean());
    let HItem::Fn(g) = &h.hir.items[1] else {
        panic!("item thứ hai phải là `g`");
    };
    let HStmt::Expr(HExpr::Call { callee, .. }) = &g.body[0] else {
        panic!("thân `g` phải là lời gọi");
    };
    assert!(matches!(&**callee, HExpr::Var { sym: Some(_), .. }));
}

#[test]
fn for_bind_if_let_match_arm_va_lambda() {
    // `for i in …` — `i` là khai báo trong khung của vòng lặp.
    let h = free("for i in 0..10:\n    i\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::For { pat, body, .. } = &h.hir.stmts[0] else {
        panic!("phải là `for`");
    };
    let HPat::Bind { name, sym, .. } = pat else {
        panic!("pattern vòng lặp phải là bind");
    };
    assert_eq!(name, "i");
    let HStmt::Expr(HExpr::Var { sym: used, .. }) = &body[0] else {
        panic!("thân vòng lặp phải là `i`");
    };
    assert_eq!(used, &Some(*sym), "`i` trong thân phải trỏ bind của `for`");

    // `if let mail = …` — `mail` chỉ sống trong nhánh `then`.
    let h = free("if let mail = user.email:\n    send(mail)\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::If { cond, then, .. } = &h.hir.stmts[0] else {
        panic!("phải là `if`");
    };
    let vis_types::HCond::Let { pat, .. } = cond else {
        panic!("`if let` phải là HCond::Let");
    };
    let HPat::Bind { name, .. } = pat else {
        panic!("pattern `if let` phải là bind");
    };
    assert_eq!(name, "mail");
    let HStmt::Expr(HExpr::Call { args, .. }) = &then[0] else {
        panic!("thân phải là lời gọi");
    };
    assert!(
        matches!(&args[0], HExpr::Var { sym: Some(_), .. }),
        "`mail` trong thân phải phân giải"
    );

    // arm `match` — binding sống trong arm.
    let h = free("match s:\n    Circle(r): r\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::Match { arms, .. } = &h.hir.stmts[0] else {
        panic!("phải là `match`");
    };
    let HStmt::Expr(HExpr::Var { name, sym, .. }) = &arms[0].body[0] else {
        panic!("thân arm phải là `r`");
    };
    assert_eq!(name, "r");
    let s = h.symbols.get(sym.expect("phải phân giải")).unwrap();
    assert_eq!(s.kind, SymKind::PatBind);

    // lambda `(x) => x + 1`
    let h = free("let f = (x) => x + 1\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::Let {
        init: HExpr::Lambda { params, body, .. },
        ..
    } = &h.hir.stmts[0]
    else {
        panic!("phải là `let` với lambda");
    };
    assert_eq!(params.len(), 1);
    let vis_types::HBody::Expr(e) = body else {
        panic!("lambda một biểu thức");
    };
    let HExpr::Bin { lhs, .. } = &**e else {
        panic!("thân lambda phải là phép cộng");
    };
    assert!(matches!(
        &**lhs,
        HExpr::Var { name, sym: Some(_), .. } if name == "x"
    ));
}

#[test]
fn nhan_scope_la_khai_bao() {
    let h = free("scope s:\n    s.spawn(1)\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::Scope { label, body, .. } = &h.hir.stmts[0] else {
        panic!("phải là `scope`");
    };
    assert_eq!(label.as_deref(), Some("s"));
    let HStmt::Expr(HExpr::Call { callee, .. }) = &body[0] else {
        panic!("thân phải là lời gọi `s.spawn(1)`");
    };
    let HExpr::Field { base, name, .. } = &**callee else {
        panic!("callee phải là truy cập trường");
    };
    assert_eq!(name, "spawn");
    let HExpr::Var { sym, .. } = &**base else {
        panic!("`s` phải là tên");
    };
    let s = h.symbols.get(sym.expect("phải phân giải")).unwrap();
    assert_eq!(s.kind, SymKind::ScopeBind);
}

// ---------------------------------------------------------------------------
// VS0301 — tên giá trị chưa khai
// ---------------------------------------------------------------------------

#[test]
fn gan_cho_ten_chua_khai_bao_vs0301_mac_dinh() {
    // VD-44 #2: gán cho tên chưa khai là lỗi **chắc chắn** ⇒ báo cả ở chế độ mặc định.
    let h = free("x = 1\n");
    assert_eq!(h.codes(), ["VS0301"]);
    assert!(h.diagnostics[0].message.contains("x"));
    assert!(
        h.diagnostics[0].message.contains("let") || h.diagnostics[0].message.contains("khai"),
        "phải gợi ý khai báo: {}",
        h.diagnostics[0].message
    );
    // Gán cho `var`/`let` đã khai ⇒ **không** phải lỗi tên (mutability là chuyện của ST-12).
    assert!(free("var x = 1\nx = 2\n").is_clean());
    assert!(free("var x = 1\nx += 2\n").is_clean());
}

#[test]
fn ten_la_bao_vs0301_o_che_do_chat() {
    let h = strict("let y = z + 1\n");
    assert_eq!(h.codes(), ["VS0301"], "Strict: `z` chưa khai ⇒ VS0301");
    assert!(h.diagnostics[0].message.contains("z"));
    // Mặc định (`Permissive`): tên tự do **không** báo — 30 golden + 15 khối tài liệu dựa vào điều này.
    assert!(free("let y = z + 1\n").is_clean());
}

#[test]
fn ten_kieu_chua_khai_khong_bao_o_st11() {
    // VD-44 #2: tên KIỂU để ST-12; ST-11 chỉ giữ `TypeRef`.
    let h = strict("let x: Config = y\nlet z: map[K, V] = w\n");
    // `y`/`w` là tên **giá trị** chưa khai ⇒ Strict báo đúng 2 lần (không phải vì `Config`/`K`/`V`).
    assert_eq!(h.codes(), ["VS0301", "VS0301"]);
    let named: Vec<bool> = h
        .diagnostics
        .iter()
        .map(|d| d.message.contains("`y`") || d.message.contains("`w`"))
        .collect();
    assert_eq!(
        named,
        [true, true],
        "chẩn đoán phải nêu tên **giá trị**, không phải tên kiểu: {:?}",
        h.diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
    // Tên kiểu chưa khai vẫn **không** sinh chẩn đoán nào ở ST-11 (kể cả `cap[Dir]`).
    assert!(
        strict("let c: cap[Dir] = d\n").codes() == ["VS0301"],
        "chỉ `d` bị báo"
    );
}

/// **VD-45**: `Permissive` là mặc định ⇒ **đọc** tên tự do được phép, nhưng **gán** cho tên chưa
/// khai vẫn `VS0301`. Cặp ca này chứng minh "permissive" **không** tắt kiểm tra tên.
#[test]
fn permissive_van_bao_vs0301_khi_gan_ten_la() {
    // (a) đọc tên tự do: hợp lệ (đây là lý do chọn Permissive — 15 khối + 30 golden).
    let h = free("let y = z + 1\n");
    assert!(
        h.is_clean(),
        "đọc `z` tự do phải được phép: {:?}",
        h.codes()
    );
    // (b) GÁN cho tên chưa khai: `VS0301` ở **cả** hai dạng gán.
    for src in ["z = 1\n", "z += 1\n"] {
        let h = free(src);
        assert_eq!(h.codes(), ["VS0301"], "`{src}` phải VS0301 ở Permissive");
        assert!(
            h.diagnostics[0].message.contains('z'),
            "thông điệp phải nêu tên: {}",
            h.diagnostics[0].message
        );
    }
    // (c) gán cho tên **đã khai** thì không phải lỗi tên.
    assert!(free("var z = 0\nz = 1\n").is_clean());
    assert!(free("var z = 0\nz += 1\n").is_clean());
    // (d) đích gán phức hợp (`a.b = 1`) cũng phải phân giải được gốc.
    assert_eq!(free("a.b = 1\n").codes(), ["VS0301"]);
}

/// **VD-45**: `Strict` báo `VS0301` cho **mọi** lần **đọc** tên tự do — mặc định từ ST-13/14.
#[test]
fn strict_bao_ca_ten_tu_do_khi_doc() {
    let h = strict("let y = z + 1\n");
    assert_eq!(h.codes(), ["VS0301"], "Strict: đọc `z` phải VS0301");
    assert!(h.diagnostics[0].message.contains('z'));
    let h = strict("let a = b\nlet c = d\n");
    assert_eq!(
        h.codes(),
        ["VS0301", "VS0301"],
        "mỗi tên tự do một chẩn đoán"
    );
    // Strict **không** báo cho tên đã khai / tên dựng sẵn.
    assert!(strict("let z = 1\nlet y = z + 1\n").is_clean());
    assert!(strict("print(1)\n").is_clean());
}

/// §4.1 (reviewer3 MAJOR 1): `let` phải hạ **giá trị trước** rồi mới khai tên.
#[test]
fn let_ha_gia_tri_truoc_khi_khai_ten() {
    // (a) cấp tệp: `let x = x + 1` — RHS **không** được trỏ khai báo mới ⇒ `sym = None`.
    let h = free("let x = x + 1\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::Let { pat, init, .. } = &h.hir.stmts[0] else {
        panic!("phải là `let`");
    };
    let HPat::Bind {
        name, sym: new_sym, ..
    } = pat
    else {
        panic!("pattern phải là bind");
    };
    assert_eq!(name, "x");
    let HExpr::Bin { lhs, .. } = init else {
        panic!("RHS phải là phép cộng");
    };
    let HExpr::Var { name, sym, .. } = &**lhs else {
        panic!("vế trái RHS phải là tên");
    };
    assert_eq!(name, "x");
    assert_eq!(
        *sym, None,
        "RHS `x` phải KHÔNG trỏ khai báo mới (hạ giá trị trước khi khai tên, §4.1)"
    );
    assert_ne!(*sym, Some(*new_sym), "RHS không được trỏ chính `x` mới");

    // (b) shadow trong khối: RHS trỏ `x` **khung ngoài**, thân khối trỏ `x` mới.
    let h = free("let x = 1\nif true:\n    let x = x + 1\n    x\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let xs: Vec<&vis_types::HSym> = h.symbols.all_named("x").collect();
    assert_eq!(xs.len(), 2, "phải có 2 khai báo `x`");
    let outer_frame = xs[0].frame;
    let inner_frame = xs[1].frame;
    assert_ne!(outer_frame, inner_frame, "hai `x` phải ở hai khung");
    let HStmt::If { then, .. } = &h.hir.stmts[1] else {
        panic!("câu lệnh thứ hai phải là `if`");
    };
    let HStmt::Let { init, .. } = &then[0] else {
        panic!("thân `if` phải bắt đầu bằng `let`");
    };
    let HExpr::Bin { lhs, .. } = init else {
        panic!("RHS phải là phép cộng");
    };
    let HExpr::Var { sym, .. } = &**lhs else {
        panic!("RHS phải là tên");
    };
    assert_eq!(
        h.symbols
            .get(sym.expect("RHS phải phân giải về `x` ngoài"))
            .unwrap()
            .frame,
        outer_frame,
        "RHS phải trỏ `x` khung NGOÀI (không phải khai báo mới trong khối)"
    );
    let HStmt::Expr(HExpr::Var { sym: body_sym, .. }) = &then[1] else {
        panic!("biểu thức cuối khối phải là `x`");
    };
    assert_eq!(
        h.symbols
            .get(body_sym.expect("phải phân giải"))
            .unwrap()
            .frame,
        inner_frame,
        "thân khối phải trỏ `x` MỚI trong khối"
    );
}

/// VD-45 (reviewer3 MAJOR 2): mỗi đích gán chưa khai chỉ báo `VS0301` **một lần**, ở **cả** hai policy.
#[test]
fn gan_ten_la_chi_mot_vs0301_moi_dang() {
    for src in ["x = 1\n", "a.b = 1\n", "xs[0] = 1\n"] {
        let h = free(src);
        assert_eq!(
            h.codes(),
            ["VS0301"],
            "Permissive `{src}`: phải **đúng một** VS0301, thật {:?}",
            h.codes()
        );
        let h = strict(src);
        assert_eq!(
            h.codes(),
            ["VS0301"],
            "Strict `{src}`: phải **đúng một** VS0301 (không nhân đôi), thật {:?}",
            h.codes()
        );
        assert!(h.diagnostics[0].message.contains('x') || h.diagnostics[0].message.contains('a'));
    }
    // Gán đích phức hợp mà **gốc đã khai** ⇒ không chẩn đoán.
    assert!(free("var a = 1\na.b = 2\n").is_clean());
    assert!(strict("var xs = [1]\nxs[0] = 2\n").is_clean());
}

// ---------------------------------------------------------------------------
// 8 tên giá trị dựng sẵn (VD-44 #3) — khoá danh sách
// ---------------------------------------------------------------------------

#[test]
fn danh_sach_builtin_khoa_dung_8_ten() {
    assert_eq!(
        BUILTIN_VALUE_NAMES,
        ["print", "len", "str", "Ok", "Err", "Some", "None", "ref"],
        "VD-44 #3: 8 tên giá trị dựng sẵn — thêm/bớt phải qua Agent 0"
    );
    for n in BUILTIN_VALUE_NAMES {
        assert!(vis_types::is_builtin_value(n), "{n} phải là builtin");
        assert!(vis_types::BUILTIN_VALUE_NAMES.contains(&n));
    }
    assert!(!vis_types::is_builtin_value("println"));
    assert!(!vis_types::is_builtin_value("true"), "true là literal");
    // Chứng minh `lower` **dùng** danh sách: `print(1)` ⇒ `sym` trỏ ký hiệu `Builtin`.
    let h = free("print(1)\n");
    assert!(h.is_clean(), "{:?}", h.codes());
    let HStmt::Expr(HExpr::Call { callee, .. }) = &h.hir.stmts[0] else {
        panic!("phải là lời gọi");
    };
    let HExpr::Var { name, sym, .. } = &**callee else {
        panic!("callee phải là tên");
    };
    assert_eq!(name, "print");
    let sym = sym.expect("`print` phải phân giải");
    assert_eq!(
        h.symbols.kind_of(sym),
        Some(SymKind::Builtin),
        "`print` phải trỏ ký hiệu dựng sẵn (không phải `sym = None`)"
    );
    // `true`/`false`/`none` **không** phải tên ⇒ không có ký hiệu Builtin.
    assert!(!BUILTIN_VALUE_NAMES.contains(&"true"));
    assert!(!BUILTIN_VALUE_NAMES.contains(&"none"));
}

#[test]
fn builtin_phan_giai_duoc_o_moi_che_do() {
    let h = strict("print(1)\nlet x = str(2)\nlet a = Ok(1)\n");
    assert!(
        h.codes().iter().all(|c| *c != "VS0301"),
        "tên dựng sẵn không được báo VS0301: {:?}",
        h.codes()
    );
    // Có ký hiệu `SymKind::Builtin` trong khung tệp cho từng tên.
    for n in BUILTIN_VALUE_NAMES {
        assert_eq!(
            h.symbols.in_frame(0, n).map(|s| s.kind),
            Some(SymKind::Builtin),
            "{n} phải có ký hiệu Builtin ở khung tệp"
        );
    }
    // Nhưng tên **không** dựng sẵn vẫn báo.
    let h = strict("send(1)\n");
    assert_eq!(h.codes(), ["VS0301"]);
}
