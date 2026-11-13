// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-51** — stage đầu là **biểu thức đóng** (nợ VSH1b-2): `[1,2] | take 1` **không ngoặc**.
//!
//! Nới **hẹp nhất** (§9.5): chỉ chế độ lệnh, chỉ khi dòng **bắt đầu** bằng `[`, `{`, hoặc **chuỗi**
//! **và** có `|` độ sâu 0. Không nhận số/biến/tên lệnh; bản ghi `{…}` được **bọc list một phần tử**.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-stagedau-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &Path, tag: &str, src: &str, mock: Option<&Path>, vm: bool) -> (i32, String, String) {
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
    if let Some(m) = mock {
        c.arg("--fs-mock").arg(m);
    }
    let o = c.arg(&p).output().expect("chạy");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

const MOCK: &str = r#"{ "dirs": { "./photos": [
  {"name": "a.jpg", "size": "2mb", "modified": "3h", "is_file": true},
  {"name": "b.jpg", "size": "6mb", "modified": "1h", "is_file": true} ] } }"#;

fn mock(d: &Path) -> PathBuf {
    let p = d.join("mock.json");
    std::fs::write(&p, MOCK).expect("ghi mock");
    p
}

#[test]
fn list_literal_dau_pipeline() {
    let d = tam("list");
    let (c, o, e) = chay(&d, "l", "[1, 2] | take 1\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn list_literal_nhieu_stage() {
    let d = tam("list2");
    let (c, o, e) = chay(&d, "l", "[1, 2, 3] | take 2 | take 1\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn list_literal_khong_co_pipe_van_vs0201() {
    let d = tam("nopipe");
    let (c, _, e) = chay(&d, "l", "[1, 2]\n", None, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **Ca bắt buộc (Agent 0)**: `{…}` là ca rủi ro nhất — phải **chạy thật**, không chỉ parse.
#[test]
fn record_literal_dau_where_chay_that() {
    let d = tam("rec");
    let src = "{size: 6mb, modified: 1h, is_file: true} | where size > 1mb\n";
    let (c, o, e) = chay(&d, "l", src, None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("6mb"), "phải in dòng dữ liệu: {o}");
    assert!(o.contains("is_file"), "phải có bảng: {o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn record_literal_where_loc_sach() {
    let d = tam("rec2");
    let src = "{size: 1mb, modified: 1h, is_file: true} | where size > 5mb\n";
    let (c, o, e) = chay(&d, "l", src, None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn record_literal_select_va_take() {
    let d = tam("rec3");
    let (c, o, e) = chay(
        &d,
        "l",
        "{size: 1mb, modified: 1h, is_file: true} | select size\n",
        None,
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("size") && o.contains("1mb"), "{o}");
    let (c2, o2, e2) = chay(
        &d,
        "l2",
        "{size: 1mb, modified: 1h, is_file: true} | take 1\n",
        None,
        false,
    );
    assert_eq!(c2, 0, "stderr: {e2}");
    assert!(o2.contains("1mb"), "{o2}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn string_literal_dau_lines() {
    let d = tam("str");
    let (c, o, e) = chay(&d, "l", "\"a b\" | lines | take 1\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[a b]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn bien_va_so_khong_duoc_nhan() {
    // **Nhập nhằng** với tên lệnh: `x | take 1` vẫn là **lệnh ngoài** `x` (không thành biểu thức).
    let d = tam("x");
    let (c, _, e) = chay(&d, "l", "x | take 1\n", None, false);
    assert_eq!(c, 1, "phải chạy `x` như chương trình ngoài: {e}");
    assert!(e.contains("`x`"), "stderr: {e}");
    // `take 1` (không `|`, không upstream) vẫn là lệnh ⇒ lỗi khe như trước, **không** thành biểu thức.
    let (c2, _, e2) = chay(&d, "l2", "take 1\n", None, false);
    assert_eq!(c2, 2, "stderr: {e2}");
    assert!(e2.contains("VS0602"), "stderr: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lenh_thuong_khong_doi() {
    let d = tam("ls");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "ls ./photos | take 1\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name   size  modified  is_file\na.jpg   2mb        3h  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_ngoac_cu_khong_doi() {
    let d = tam("cu");
    let (c, o, e) = chay(&d, "l", "( [1, 2] ) | take 1\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn golden_ca24_va_ca_a_or_b_khong_doi() {
    // Golden ca 24: shell-let vẫn là **shell-let** (`vis check` xanh).
    let d = tam("golden");
    let p = d.join("x.vish");
    std::fs::write(
        &p,
        "# SPDX-License-Identifier: MIT OR Apache-2.0\nlet x: int = ls | grep foo\n",
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("check")
        .arg(&p)
        .output()
        .expect("chạy check");
    assert_eq!(
        o.status.code().unwrap_or(-1),
        0,
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    // Golden ca `ls | (a | b)`: parse hợp lệ (ST-49), tầng hạ báo **VS0603** như trước.
    let (c2, _, e2) = chay(&d, "l", "ls | (a | b)\n", None, false);
    assert_eq!(c2, 2, "stderr: {e2}");
    assert!(e2.contains("VS0603"), "stderr: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn interp_va_vm_cung_byte() {
    let d = tam("vm");
    for src in [
        "[1, 2, 3] | take 2\n",
        "{size: 6mb, modified: 1h, is_file: true} | where size > 1mb\n",
    ] {
        let (c1, o1, e1) = chay(&d, "i", src, None, false);
        let (c2, o2, e2) = chay(&d, "v", src, None, true);
        assert_eq!(c1, 0, "{e1}");
        assert_eq!(c2, 0, "{e2}");
        assert_eq!(o1.as_bytes(), o2.as_bytes(), "khác byte cho {src}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
