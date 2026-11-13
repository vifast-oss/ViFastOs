// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-39 (VSH1a)** — `sort_by` chế độ shell + cờ `--desc` (VD-59 (3), VD-70).
//!
//! Khoá: khoá `int`/`size`(byte)/`duration`(ns)/`str`(byte UTF-8); **ổn định cả hai chiều** (đảo
//! comparator, không đảo mảng); `--desc` là **cờ** (không bị khe ống chiếm, không bị nuốt vào khoá);
//! `--desc=false`; khoá `bool` ⇒ **`VS0412`**; thiếu đối số/cờ lạ ⇒ **`VS0602`**.

use std::path::PathBuf;
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-sx-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &std::path::Path, tag: &str, src: &str) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vish"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&p)
        .output()
        .expect("chạy");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn ok(tag: &str, src: &str, muon: &str) {
    let d = tam(tag);
    let (c, o, e) = chay(&d, tag, src);
    assert_eq!(c, 0, "[{tag}] stderr: {e}");
    assert_eq!(o, muon, "[{tag}]");
    let _ = std::fs::remove_dir_all(&d);
}

fn loi(tag: &str, src: &str, ma: &str) {
    let d = tam(tag);
    let (c, _, e) = chay(&d, tag, src);
    assert_ne!(c, 0, "[{tag}] phải lỗi, stderr: {e}");
    assert!(e.contains(ma), "[{tag}] cần {ma}, được: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn int_tang_dan() {
    ok("iasc", "( [3, 1, 2] ) | sort_by it => it\n", "[1, 2, 3]\n");
}

#[test]
fn int_giam_dan_bang_co_desc() {
    ok(
        "idesc",
        "( [3, 1, 2] ) | sort_by it => it --desc\n",
        "[3, 2, 1]\n",
    );
}

#[test]
fn desc_bang_false_la_tang_dan() {
    ok(
        "dfalse",
        "( [3, 1, 2] ) | sort_by it => it --desc=false\n",
        "[1, 2, 3]\n",
    );
}

#[test]
fn size_la_khoa_byte() {
    ok(
        "size",
        "( [{size: 6mb, ten: \"b\"}, {size: 1mb, ten: \"s\"}] ) | sort_by size --desc\n",
        "size  ten\n 6mb  b\n 1mb  s\n",
    );
}

#[test]
fn duration_la_khoa_ns() {
    ok(
        "dur",
        "( [{d: 5m, ten: \"x\"}, {d: 1s, ten: \"y\"}] ) | sort_by d\n",
        " d  ten\n1s  y\n5m  x\n",
    );
}

#[test]
fn str_la_khoa_byte_utf8() {
    ok(
        "str",
        "( [\"bb\", \"a\"] ) | sort_by it => it\n",
        "[a, bb]\n",
    );
}

/// Ổn định **tăng**: khoá bằng nhau ⇒ giữ thứ tự gốc (`b` trước `a`).
#[test]
fn on_dinh_khi_khoa_bang_nhau_tang() {
    ok(
        "stab1",
        "( [{size: 1mb, ten: \"b\"}, {size: 1mb, ten: \"a\"}] ) | sort_by size\n",
        "size  ten\n 1mb  b\n 1mb  a\n",
    );
}

/// Ổn định **giảm** trên khoá `size` (ca Agent 0 bắt buộc): `a` trước `b` **dù** `--desc`.
#[test]
fn on_dinh_khi_khoa_bang_nhau_desc_tren_size() {
    ok(
        "stab2",
        "( [{size: 1mb, ten: \"a\"}, {size: 1mb, ten: \"b\"}] ) | sort_by size --desc\n",
        "size  ten\n 1mb  a\n 1mb  b\n",
    );
}

/// `--desc` là **cờ**: không bị khe ống chiếm, không bị nuốt vào khoá (sugar `it.size`).
#[test]
fn desc_khong_bi_nuot_vao_khoa() {
    ok(
        "flag",
        "( [{size: 6mb}, {size: 1mb}] ) | sort_by size --desc\n",
        "size\n 6mb\n 1mb\n",
    );
}

#[test]
fn khoa_bool_la_vs0412() {
    loi("bool", "( [true, false] ) | sort_by it => it\n", "VS0412");
}

#[test]
fn thieu_doi_so_la_vs0602() {
    loi("thieu", "( [3, 1] ) | sort_by\n", "VS0602");
}

#[test]
fn co_la_la_vs0602() {
    loi("co", "( [{size: 1mb}] ) | sort_by size --sai\n", "VS0602");
}
