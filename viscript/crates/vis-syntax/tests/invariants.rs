// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bất biến cấu trúc của lexer trên một **corpus cố định** — bản chạy trong `gate --fast`.
//!
//! Fuzz 10 phút của ST-04 (`src/bin/vis_lex_fuzz.rs`) kiểm đúng bộ bất biến này trên đầu vào
//! sinh ngẫu nhiên; ở đây chạy bản ngắn, tất định, để cổng bắt được hồi quy ngay.

use unicode_normalization::UnicodeNormalization;
use vis_syntax::lex::verify;
use vis_syntax::{lex, Kind, LexOutput, Mode};

/// Đoạn mã đại diện cho từng vùng của SPEC-lex (§2 số, §3 chuỗi, §4 thụt lề, §5 nhập nhằng, §6 lệnh).
const CORPUS: &[&str] = &[
    "",
    "42",
    "1_000_000 0xff 0b101 0b 5kb 5kib 5mib 2h30m 250ms 1d 50pct 12.5pct",
    "1. 1..2 0..=10 1.2..3.4 1e-9 1_ 1__0 9223372036854775808",
    "\"a\\nb\" r\"C:\\raw\" \"a{x:>8}b\" \"{{\" \"{x\"",
    "\"\"\"a\\nb\"\"\" r\"\"\"thô {x}\"\"\" b\"\\x00\\x01\" b\"ố\" p\"./a.txt\"",
    "'a' 'ố' 'ab' '' '",
    "set{1,2} set {1} x??y x?.y f()? a|b a|>b 2**3 x |> f",
    "# c",
    "## doc\nfn f():\n    ## tài liệu trong khối\n    x = 1\n",
    "if x:\n    y\n  z",
    "if x:\n\ty\n",
    "if x:\n    if y:\n        z\n    w\nv",
    "if x:\n    a\n\n# cột 0 vẫn trong khối\n    b\n",
    "(1 +\n 2)\n[1,\n 2]\n{1,\n 2}",
    "x\n|> f\n|> g\ny",
    "x\n\n   |> f",
    "let số_lượng = 1\n",
    "fn tính_tổng(a: int) -> int:\n    return a\n",
    "`~!@#$^&*()[]{}<>?:;,.\\/|=-",
    "ls -5",
    "ls -la --",
    "ls -- -x",
    "echo \"a b\" 'c d'",
    "$HOME $env.PATH $x.y.z",
    "a;b a & a > b a<b",
    "https://x/y#frag",
    "^git",
    "| grep x",
    "a | ^git status",
    "echo\ttiêu",
    "\t\t",
    "\r\n",
    "a\r\nb\r\n",
    "a\rb",
    "é\u{301}",
    "😀",
    "\u{feff}let x = 1",
    "let\u{0}x",
    "x = \"chưa đóng",
    "x = \"{chưa đóng",
    "x = r\"thô {không nội suy}\"",
    "set{set{1}}",
    "0x 0b 0xg 5k 5ki",
    "-9223372036854775808",
    "i64.min",
];

fn check(src: &str, mode: Mode) -> LexOutput {
    let out = lex(src, mode);
    let bad = verify::violations(&out);
    assert!(
        bad.is_empty(),
        "bất biến sai ở mode={mode:?} nguồn={src:?}:\n  {}",
        bad.join("\n  ")
    );
    // Tất định: lex hai lần phải giống hệt (SPEC-lex §0.2).
    let again = lex(src, mode);
    assert_eq!(out, again, "lex không tất định với {src:?}");
    out
}

#[test]
fn corpus_giu_bat_bien_o_ca_hai_che_do() {
    for src in CORPUS {
        for mode in [Mode::Script, Mode::Command] {
            check(src, mode);
        }
    }
}

#[test]
fn nfc_va_nfd_cho_cung_ket_qua_tren_corpus() {
    for src in CORPUS {
        let nfd: String = src.nfd().collect();
        for mode in [Mode::Script, Mode::Command] {
            assert_eq!(
                lex(src, mode),
                lex(&nfd, mode),
                "NFC/NFD lệch ở mode={mode:?} nguồn={src:?}"
            );
        }
    }
}

#[test]
fn moi_ma_loi_tu_vung_deu_phat_duoc() {
    // Mỗi mã VS0101..VS0110 phải có ít nhất một nguồn kích hoạt được (không mã chết).
    let sources: [(Mode, &str, &str); 10] = [
        (Mode::Script, "\"chưa đóng", "VS0101"),
        (Mode::Script, "\"a\\qb\"", "VS0102"),
        (Mode::Script, "`", "VS0103"),
        (Mode::Script, "1_", "VS0104"),
        (Mode::Script, "'ab'", "VS0105"),
        (Mode::Script, "b\"ố\"", "VS0106"),
        (Mode::Script, "x\t= 1", "VS0107"),
        (Mode::Script, "\"{x\"", "VS0108"),
        (Mode::Script, "if x:\n    y\n  z", "VS0109"),
        (Mode::Command, "a^b", "VS0110"),
    ];
    for (mode, src, want) in sources {
        let out = check(src, mode);
        let got: Vec<&str> = out.diagnostics.iter().map(|d| d.code_str()).collect();
        assert!(
            got.contains(&want),
            "nguồn {src:?} không phát {want} (có {got:?})"
        );
    }
}

#[test]
fn span_phu_kin_vung_khong_trong() {
    // Mọi byte nội dung phải nằm trong span của đúng một token (trừ khoảng trắng đã bỏ).
    let src = "let x = 1\nfn f():\n    return x\n";
    let out = lex(src, Mode::Script);
    let mut covered = vec![false; out.text.len()];
    for t in &out.tokens {
        for slot in covered.iter_mut().take(t.span.end).skip(t.span.start) {
            *slot = true;
        }
    }
    for (i, c) in out.text.char_indices() {
        if c.is_whitespace() {
            continue;
        }
        assert!(covered[i], "ký tự {c:?} tại {i} không thuộc token nào");
    }
    assert!(out.tokens.iter().any(|t| t.kind == Kind::Indent));
    assert!(out.tokens.iter().any(|t| t.kind == Kind::Dedent));
}
