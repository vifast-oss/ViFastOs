// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-49** — nhóm `( … )` làm **stage**: **lệnh có cờ**, **pipeline**, **nhiều dòng**.
//!
//! Khoá: `( ls ./photos --long )` ⇒ **6 cột** (`list[FileEntryLong]`), `( ls … | where … )` chạy,
//! newline trong ngoặc **không** kết thúc biểu thức/stage; chẩn đoán vẫn **tuyệt đối**; không phá
//! `( … )` làm **đối số** (ST-46) / stage (ST-37) / shell-let (ST-44).

use std::path::{Path, PathBuf};
use std::process::Command;

fn goc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-nhom-{tag}-{}", std::process::id()));
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
  {"name": "a.jpg", "size": "2mb", "modified": "3h", "is_file": true, "mode": "rw-r--r--", "mtime": "1700000000s"},
  {"name": "b.jpg", "size": "6mb", "modified": "1h", "is_file": true, "mode": "rw-------", "mtime": "1700086400s"} ] } }"#;

fn mock(d: &Path) -> PathBuf {
    let p = d.join("mock.json");
    std::fs::write(&p, MOCK).expect("ghi mock");
    p
}

const BANG_4: &str = "name   size     modified  is_file\na.jpg   2mb  1700000000s  true\nb.jpg   6mb  1700086400s  true\n";
const BANG_6: &str = "name   size     modified  is_file  mode       modified_time\na.jpg   2mb  1700000000s  true     rw-r--r--  2023-11-14 22:13\nb.jpg   6mb  1700086400s  true     rw-------  2023-11-15 22:13\n";
const CHI_B: &str = "name   size     modified  is_file\nb.jpg   6mb  1700086400s  true\n";

#[test]
fn nhom_lenh_co_co_long() {
    let d = tam("long");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "( ls ./photos --long )\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, BANG_6);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_co_pipeline_hai_stage() {
    let d = tam("pipe");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "( ls ./photos | where size > 5mb )\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, CHI_B);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_pipeline_nhieu_dong() {
    let d = tam("pipe2");
    let m = mock(&d);
    let src = "( ls ./photos |\n  where size > 5mb |\n  sort_by size --desc )\n";
    let (c, o, e) = chay(&d, "l", src, Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, CHI_B);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_bieu_thuc_nhieu_dong() {
    let d = tam("expr");
    let (c, o, e) = chay(&d, "l", "( 1 +\n  1 )\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "2\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn chan_doan_tuyet_doi_trong_nhom_nhieu_dong() {
    let d = tam("diag");
    // Dòng 1 SPDX, dòng 2 `( 1 +`, dòng 3 `  0 / 0 )` ⇒ `0` ở **3:3**.
    let (c, _, e) = chay(&d, "l", "( 1 +\n  0 / 0 )\n", None, false);
    assert_eq!(c, 1, "lỗi lúc chạy ⇒ exit 1: {e}");
    assert!(e.contains(":3:3: error VS0502"), "phải tuyệt đối: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_long_nhau_bieu_thuc() {
    let d = tam("nest");
    let (c, o, e) = chay(&d, "l", "( ( 1 + 1 ) * 3 )\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "6\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_long_nhau_lenh() {
    let d = tam("nest2");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "( ( ls ./photos ) | take 1 )\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name   size     modified  is_file\na.jpg   2mb  1700000000s  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn kieu_nhom_long_kiem_tinh_select() {
    let d = tam("kieu");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "( ls ./photos --long ) | select mode\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "mode\nrw-r--r--\nrw-------\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn kieu_nhom_kiem_tinh_where() {
    let d = tam("kieu2");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "( ls ./photos ) | where size > 5mb\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, CHI_B);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_lam_doi_so_st46_khong_doi() {
    let d = tam("arg");
    let (c, o, e) = chay(&d, "l", "( [1, 2] ) | take ( 1 + 1 )\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1, 2]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn shell_let_st44_khong_doi() {
    let d = tam("let");
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
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn interp_va_vm_cung_byte() {
    let d = tam("vm");
    let m = mock(&d);
    let src = "( ls ./photos |\n  where size > 5mb |\n  sort_by size --desc )\n";
    let (c1, o1, e1) = chay(&d, "i", src, Some(&m), false);
    let (c2, o2, e2) = chay(&d, "v", src, Some(&m), true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_rong_la_vs0201() {
    let d = tam("rong");
    let (c, _, e) = chay(&d, "l", "( )\n", None, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// 3 fixture trong `test/cmd/nhom/**`.
#[test]
fn fixture_cmd_nhom() {
    let m = goc().join("test/cmd/nhom/mock.json");
    for (ten, muon) in [("long", BANG_6), ("pipeline", CHI_B), ("nhieu-dong", "2\n")] {
        let p = goc().join("test/cmd/nhom").join(format!("{ten}.vish"));
        let o = Command::new(env!("CARGO_BIN_EXE_vis"))
            .arg("run")
            .arg("--fs-mock")
            .arg(&m)
            .arg(&p)
            .output()
            .expect("chạy fixture");
        assert_eq!(
            o.status.code().unwrap_or(-1),
            0,
            "fixture {ten}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&o.stdout), muon, "fixture {ten}");
    }
    // `( ls ./photos )` vẫn là **4 cột** (đối chứng: không phải mọi nhóm đều 6 cột).
    let p = goc().join("test/cmd/nhom/long.vish");
    let _ = p;
    let d = tam("bon");
    let mm = mock(&d);
    let (c, o, e) = chay(&d, "l", "( ls ./photos )\n", Some(&mm), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, BANG_4);
    let _ = std::fs::remove_dir_all(&d);
}
