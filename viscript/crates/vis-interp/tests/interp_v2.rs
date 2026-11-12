// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-13b — 10 mục review ST-13** (tệp mới; `tests/run.rs` + `tests/show.rs` của ST-13 giữ nguyên
//! để log đỏ `st13-red-run.log` vẫn khớp lịch sử của commit `5e9a962`).
//!
//! Mỗi ca ghi rõ **kỳ vọng** (stdout + mã lỗi nếu có) và **mục review** tương ứng.

use vis_syntax::{lex, parse, Mode};
use vis_types::{check, lower};

/// Chạy một chương trình: trả (stdout, mã lỗi đầu tiên nếu có, exit).
fn chay(src: &str) -> (String, Option<String>, i32) {
    let p = parse(src, Mode::Script);
    if !p.diagnostics.is_empty() {
        return (String::new(), Some(p.diagnostics[0].code.short.into()), 2);
    }
    let h = lower(&p.cst, &lex(src, Mode::Script));
    if !h.is_clean() {
        return (String::new(), Some(h.diagnostics[0].code.short.into()), 2);
    }
    let t = check(&h.hir, &h.symbols, &h.types);
    if !t.is_clean() {
        return (String::new(), Some(t.diags[0].code.short.into()), 2);
    }
    let out = vis_interp::run(src, &h.hir, &h.symbols, &h.types, &[]);
    let ma = out.diagnostics.first().map(|d| d.code.short.to_string());
    (out.stdout, ma, out.exit)
}

fn kiem(ten: &str, src: &str, stdout: &str, ma: Option<&str>) {
    let (got, got_ma, _exit) = chay(src);
    let m = ma.map(|s| s.to_string());
    assert_eq!(
        (got.as_str(), got_ma.as_deref()),
        (stdout, m.as_deref()),
        "{ten}: nguồn:\n{src}"
    );
}

// ---------------------------------------------------------------- mục 2
#[test]
fn range_khong_tran_so() {
    // mục 2: `..=` với `i64::MAX` từng panic "attempt to add with overflow".
    kiem(
        "range-	max",
        "for i in 9223372036854775807..=9223372036854775807:\n    print(i)\n",
        "9223372036854775807\n",
        None,
    );
    kiem(
        "range-bien",
        "var n = 0\nfor i in 9223372036854775800..=9223372036854775807:\n    n += 1\nprint(n)\n",
        "8\n",
        None,
    );
    kiem(
        "range-cat-cuoi",
        "for i in 9223372036854775805..=9223372036854775807:\n    print(i)\n",
        "9223372036854775805\n9223372036854775806\n9223372036854775807\n",
        None,
    );
}

// ---------------------------------------------------------------- mục 3
#[test]
fn than_vong_lap_bo_gia_tri() {
    // mục 3: giá trị cuối của thân vòng lặp bị **bỏ**, không được ép/không lỗi.
    kiem("for", "for i in 0..1:\n    1\nprint(2)\n", "2\n", None);
    kiem("while", "while false:\n    1\nprint(2)\n", "2\n", None);
}

// ---------------------------------------------------------------- mục 5
#[test]
fn so_hoc_doi_xung_voi_don_vi() {
    // mục 5: các phép mà **tyck** cho qua thì runtime phải chạy (đối xứng).
    kiem("size*percent", "print(5mb * 50pct)\n", "2500kb\n", None);
    kiem("percent*size", "print(50pct * 5mb)\n", "2500kb\n", None);
    kiem("percent/percent", "print(50pct / 50pct)\n", "1\n", None);
    kiem("size%size", "print(5mb % 3mb)\n", "2mb\n", None);
    kiem("duration%duration", "print(5s % 2s)\n", "1s\n", None);
    kiem("percent%percent", "print(50pct % 20pct)\n", "10pct\n", None);
}

// ---------------------------------------------------------------- mục 6
#[test]
fn phuong_thuc_kiem_kieu_va_has() {
    // mục 6: `filter`/`each` phải nhận **hàm**; `sum` chỉ trên list số.
    kiem("filter-so", "print([1, 2].filter(1))\n", "", Some("VS0412"));
    kiem("each-so", "print([1, 2].each(1))\n", "", Some("VS0412"));
    kiem(
        "sum-chuoi",
        "print([\"a\", \"b\"].sum())\n",
        "",
        Some("VS0412"),
    );
    // `has` là **từ khoá** ⇒ phải gọi được sau dấu chấm (mục 6).
    kiem(
        "map.has",
        "let m = {\"a\": 1}\nprint(m.has(\"b\"))\n",
        "false\n",
        None,
    );
}

// ---------------------------------------------------------------- mục 7
#[test]
fn vi_tri_tab_va_utf8() {
    // mục 7: tab = **4 cột**; cột đếm theo **Unicode scalar** (emoji = 1).
    // Tab **giữa dòng** = 4 cột (tab ở đầu dòng là VS0107 — lexer không cho thụt lề bằng tab).
    let src_tab = "let xs = [1]\nprint(  \txs[9])\n";
    let p = parse(src_tab, Mode::Script);
    // `	` sinh **VS0107 là Warning** (không phải Error) ⇒ vẫn hạ/chạy được.
    assert!(
        p.diagnostics.iter().all(|d| d.code.short == "VS0107"),
        "ca tab chỉ được có cảnh báo VS0107: {:?}",
        p.diagnostics
            .iter()
            .map(|d| d.code.short)
            .collect::<Vec<_>>()
    );
    let h = lower(&p.cst, &lex(src_tab, Mode::Script));
    let o = vis_interp::run(src_tab, &h.hir, &h.symbols, &h.types, &[]);
    let d = o.diagnostics[0].clone();
    assert_eq!(d.code.short, "VS0503");
    // "print(" = 7 cột, 2 dấu cách ⇒ 9, tab ⇒ +4 = 13 ⇒ `xs` ở cột 13.
    assert_eq!((d.line, d.col), (2, 13), "tab giữa dòng = 4 cột");
    // tiếng Việt + emoji: cột theo scalar, không theo byte.
    let src = "let s = \"ạ😀\"\nprint(s.len() + 1)\n";
    let (out, ma, _) = chay(src);
    assert_eq!(ma, None);
    assert_eq!(out, "3\n");
}

// ---------------------------------------------------------------- mục 8
#[test]
fn break_continue_va_try_ngoai_ngu_canh() {
    // mục 8: ngoài vòng lặp/hàm ⇒ **VS0501**, không được nuốt.
    kiem("break", "break\n", "", Some("VS0501"));
    kiem("continue", "continue\n", "", Some("VS0501"));
    kiem("try", "let x: int? = 5\nprint(x?)\n", "", Some("VS0501"));
}

// ---------------------------------------------------------------- mục 9
#[test]
fn show_don_vi_va_char() {
    // mục 9: `w` phải có trong bảng đơn vị; `char` xuống dòng phải **escape**.
    kiem("tuan", "print(1w)\n", "1w\n", None);
    kiem("kb-uu-tien-thap-phan", "print(1024kb)\n", "1024kb\n", None);
    kiem("kib", "print(1kib + 1kb)\n", "2024b\n", None);
}
