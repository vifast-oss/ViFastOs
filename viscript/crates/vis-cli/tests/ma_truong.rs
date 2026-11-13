// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-50** — **một mã** cho "không có trường" (**`VS0405`**, sổ `04xx`), `VS0403` giữ nghĩa cũ
//! ("trường/kiểu không tồn tại **trong khai báo**"), và `where ( x => … )` **kiểm tĩnh**.
//!
//! Khoá: bản ghi `.khong_co` ⇒ `VS0405`; `where` (sugar + lambda trong ngoặc) ⇒ `VS0405`;
//! `where ( x => 1 )` ⇒ `VS0412`; `VS0403` vẫn cho phương thức trên kiểu không phải bản ghi.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-matruong-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &Path, tag: &str, src: &str, mock: Option<&Path>) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vish"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
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

const U: &str = "type U:\n  ten: str\nlet u: U = {ten: \"A\"}\n";

#[test]
fn truong_khong_co_tren_ban_ghi_la_vs0405() {
    let d = tam("rec");
    let (c, _, e) = chay(&d, "l", &format!("{U}( u.khong_co )\n"), None);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0405"), "stderr: {e}");
    assert!(e.contains("khong_co"), "nêu tên trường: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn truong_co_thi_chay_binh_thuong() {
    let d = tam("ok");
    let (c, o, e) = chay(&d, "l", &format!("{U}( u.ten )\n"), None);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "A\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_sugar_truong_khong_co_la_vs0405() {
    let d = tam("sugar");
    let m = mock(&d);
    let (c, _, e) = chay(&d, "l", "ls ./photos | where khong_co\n", Some(&m));
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0405"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_lambda_trong_ngoac_truong_khong_co_la_vs0405() {
    let d = tam("lam");
    let m = mock(&d);
    let (c, _, e) = chay(
        &d,
        "l",
        "ls ./photos | where ( x => x.khong_co )\n",
        Some(&m),
    );
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0405"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_lambda_tra_int_la_vs0412() {
    let d = tam("int");
    let m = mock(&d);
    let (c, _, e) = chay(&d, "l", "ls ./photos | where ( x => 1 )\n", Some(&m));
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(e.contains("bool"), "nêu kiểu cần: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_lambda_trong_ngoac_dung_thi_chay() {
    let d = tam("dung");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "ls ./photos | where ( x => x.size > 5mb )\n",
        Some(&m),
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("b.jpg") && !o.contains("a.jpg"), "{o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_sugar_dung_thi_chay() {
    let d = tam("sugarok");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "ls ./photos | where size > 5mb\n", Some(&m));
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("b.jpg") && !o.contains("a.jpg"), "{o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn select_truong_khong_co_la_vs0405() {
    let d = tam("select");
    let m = mock(&d);
    let (c, _, e) = chay(&d, "l", "ls ./photos | select khong_co\n", Some(&m));
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0405"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn each_truong_khong_co_la_vs0405() {
    let d = tam("each");
    let m = mock(&d);
    let (c, _, e) = chay(&d, "l", "ls ./photos | each x => x.khong_co\n", Some(&m));
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0405"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn phuong_thuc_tren_kieu_khong_phai_ban_ghi_giu_vs0403() {
    // **VS0403** giữ nghĩa cũ: "trường/kiểu/phương thức không tồn tại" — ở đây `int` không có
    // phương thức, **không** phải "trường của bản ghi" ⇒ vẫn `VS0403` (ST-50 không đổi).
    let d = tam("int2");
    let (c, _, e) = chay(&d, "l", "let x: int = 1\n( x.khong_co() )\n", None);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0403"), "stderr: {e}");
    assert!(!e.contains("VS0405"), "không phải lỗi trường bản ghi: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn explain_vs0405_thuoc_so_kieu() {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("explain")
        .arg("VS0405")
        .output()
        .expect("chạy explain");
    let s = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(o.status.code().unwrap_or(-1), 0, "{s}");
    assert!(s.contains("VS0405"), "{s}");
    assert!(s.contains("không có trường"), "{s}");
}

#[test]
fn ma_cu_vs0605_khong_con() {
    // **SỬA KÈM LÝ DO (ST-50, Agent 0: VS0605 sai sổ — trường là lỗi kiểu)**: `VS0605` đã **đổi
    // tên** thành `VS0405` (cùng mã, khác sổ) ⇒ tra mã cũ phải **không** thấy; `all_codes()` vẫn 29
    // (vis-diag khoá riêng).
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("explain")
        .arg("VS0605")
        .output()
        .expect("chạy explain mã cũ");
    assert_ne!(o.status.code().unwrap_or(-1), 0, "VS0605 phải là mã lạ");
    let o2 = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("explain")
        .arg("VS0405")
        .output()
        .expect("chạy explain mã mới");
    assert_eq!(o2.status.code().unwrap_or(-1), 0);
}
