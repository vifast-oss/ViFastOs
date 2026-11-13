// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-41** — `Render` (§9.6): giá trị **cuối pipeline** của `.vish` ⇒ bảng (`list[bản ghi]`) hoặc
//! khối `khoá  giá trị` (bản ghi đơn); `list` rỗng ⇒ `(rỗng)`; **`print` không đổi**.
//!
//! Độ rộng tính theo **số `char` sau NFC** ⇒ NFD và NFC của cùng chữ tiếng Việt cho **cùng** bảng.

use std::path::PathBuf;
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-rd-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &std::path::Path, tag: &str, src: &str, vm: bool) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vish"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm");
    }
    let o = c.arg(&p).output().expect("chạy");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn xem(tag: &str, src: &str, muon: &str) {
    let d = tam(tag);
    let (c, o, e) = chay(&d, tag, src, false);
    assert_eq!(c, 0, "[{tag}] stderr: {e}");
    assert_eq!(o, muon, "[{tag}]");
    let _ = std::fs::remove_dir_all(&d);
}

const HAI: &str = r#"( [{name: "a", size: 6mb}, {name: "bb", size: 1mb}] )"#;

#[test]
fn bang_co_tieu_de_va_can_phai_so() {
    xem(
        "bang",
        &format!("{HAI} | sort_by size --desc\n"),
        "name  size\na      6mb\nbb     1mb\n",
    );
}

#[test]
fn bang_mot_phan_tu_van_la_bang() {
    xem(
        "mot",
        r#"( [{name: "a", size: 6mb}] )"#,
        "name  size\na      6mb\n",
    );
}

#[test]
fn cot_str_can_trai() {
    xem(
        "trai",
        r#"( [{ten: "x", mo: "ngan"}, {ten: "yy", mo: "dai hon"}] )"#,
        "ten  mo\nx    ngan\nyy   dai hon\n",
    );
}

#[test]
fn list_rong_la_rong() {
    // `(rỗng)` **chỉ** cho list có phần tử là **bản ghi** (Agent 0 chốt); list vô kiểu giữ `[]`.
    xem(
        "rong",
        r#"( [{name: "a", size: 6mb, modified: 1m, is_file: true}] ) | where size > 5gb"#,
        "(rỗng)\n",
    );
    xem("rong2", "( [] ) | where it > 1\n", "[]\n");
}

#[test]
fn ban_ghi_don_la_khoi_khoa_gia_tri() {
    xem(
        "khoi",
        r#"( {name: "a", size: 6mb} )"#,
        "name  a\nsize  6mb\n",
    );
}

#[test]
fn khoi_can_khoa_theo_ten_dai_nhat() {
    xem(
        "khoi2",
        r#"( {modified: 1m, ten: "a"} )"#,
        "modified  1m\nten       a\n",
    );
}

#[test]
fn print_khong_doi() {
    xem("print", "print(\"x\")\nprint([1, 2])\n", "x\n[1, 2]\n");
}

#[test]
fn list_str_cuoi_pipeline_khong_thanh_bang() {
    xem("lstr", "( [\"a\", \"bb\"] ) | get it => it\n", "[a, bb]\n");
}

#[test]
fn list_int_cuoi_pipeline_khong_thanh_bang() {
    xem("lint", "( [1, 2] ) | take 1\n", "[1]\n");
}

/// **NFC vs NFD**: cùng chữ tiếng Việt, hai dạng mã hoá ⇒ **cùng** bảng (cùng độ rộng cột).
#[test]
fn nfd_va_nfc_cung_do_rong() {
    // "ế" dạng NFC (U+1EBF) và NFD ("e" + U+0302 + U+0301).
    let nfc = "( [{ten: \"ế\", mo: \"a\"}, {ten: \"e\", mo: \"bb\"}] )";
    let nfd = "( [{ten: \"e\u{0302}\u{0301}\", mo: \"a\"}, {ten: \"e\", mo: \"bb\"}] )";
    let d = tam("nfd");
    let (c1, o1, e1) = chay(&d, "n1", nfc, false);
    let (c2, o2, e2) = chay(&d, "n2", nfd, false);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1, o2, "NFD và NFC phải cho cùng bảng");
    assert_eq!(o1, "ten  mo\nế    a\ne    bb\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn emoji_mot_char() {
    xem(
        "emoji",
        "( [{ten: \"😀\", mo: \"x\"}, {ten: \"ab\", mo: \"y\"}] )",
        "ten  mo\n😀    x\nab   y\n",
    );
}

#[test]
fn cjk_mot_char() {
    xem(
        "cjk",
        "( [{ten: \"漢字\", mo: \"x\"}, {ten: \"abc\", mo: \"y\"}] )",
        "ten  mo\n漢字   x\nabc  y\n",
    );
}

#[test]
fn tien_viet_nfc_dung_do_rong() {
    xem(
        "vi",
        "( [{ten: \"Việt\", mo: \"a\"}, {ten: \"An\", mo: \"bb\"}] )",
        "ten   mo\nViệt  a\nAn    bb\n",
    );
}

#[test]
fn size_duration_percent_can_phai() {
    xem(
        "donvi",
        "( [{n: \"a\", s: 6mb, d: 1m, p: 60pct}, {n: \"b\", s: 1mb, d: 2m, p: 5pct}] )",
        "n    s   d      p\na  6mb  1m  60pct\nb  1mb  2m   5pct\n",
    );
}

/// **Ca bắt buộc**: bảng ở cuối pipeline **interp và `--vm`** cho cùng chuỗi, **byte-for-byte**.
#[test]
fn bang_ca_hai_duong_giong_nhau_tung_byte() {
    let d = tam("both");
    let src = format!("{HAI} | sort_by size --desc\n");
    let (c1, o1, e1) = chay(&d, "i", &src, false);
    let (c2, o2, e2) = chay(&d, "v", &src, true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes(), "interp vs VM khác byte");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn bang_giu_thu_tu_cot_cua_ban_ghi_dau() {
    xem(
        "thutu",
        r#"( [{b: 1, a: 2}, {b: 3, a: 4}] )"#,
        "b  a\n1  2\n3  4\n",
    );
}
