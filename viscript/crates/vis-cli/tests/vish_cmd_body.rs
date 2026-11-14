// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-48** — thân `cmd` bằng **pipeline lệnh** (nợ VSH1b-2 lớn nhất).
//!
//! Khoá: thân lệnh = **đúng một pipeline**, giá trị cuối là giá trị hàm; thân **script** (ST-44)
//! không đổi; lẫn script+lệnh / thân rỗng / nhiều pipeline ⇒ **`VS0201`**; `--fs-mock` + interp ↔
//! `--vm` **cùng byte**; item **sau** thân lệnh vẫn đúng `dòng:cột`.

/// **SỬA KÈM LÝ DO (ST-50, Agent 0: VS0605 sai sổ — trường là lỗi kiểu)**: mã trường
/// không có nay là **VS0405** (sổ `04xx`); hành vi/ngữ nghĩa không đổi.
use std::path::{Path, PathBuf};
use std::process::Command;

fn goc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-cmdbody-{tag}-{}", std::process::id()));
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

const BANG: &str = "name   size     modified  is_file\na.jpg   2mb  1700000000s  true\nb.jpg   6mb  1700086400s  true\n";

#[test]
fn than_mot_stage_tra_bang() {
    let d = tam("mot");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "cmd loc() -> list[FileEntry]:\n  ls ./photos\n\nloc()\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, BANG);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_nhieu_stage_where_va_sort_by() {
    let d = tam("nhieu");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "cmd loc() -> list[FileEntry]:\n  ls ./photos | where size > 1mb | sort_by size --desc\n\nloc()\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    let dong: Vec<&str> = o.lines().skip(1).collect();
    assert_eq!(
        dong,
        [
            "b.jpg   6mb  1700086400s  true",
            "a.jpg   2mb  1700000000s  true"
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_dung_nhu_stage_khac() {
    let d = tam("stage");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "cmd loc() -> list[FileEntry]:\n  ls ./photos\n\nloc() | take 1\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name   size     modified  is_file\na.jpg   2mb  1700000000s  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn chou_ky_cmd_kiem_tinh_where() {
    let d = tam("sig");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "ok",
        "cmd loc() -> list[FileEntry]:\n  ls ./photos\n\nloc() | where size > 5mb\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("b.jpg") && !o.contains("a.jpg"), "{o}");
    let (c2, _, e2) = chay(
        &d,
        "sai",
        "cmd loc() -> list[FileEntry]:\n  ls ./photos\n\nloc() | where ten\n",
        Some(&m),
        false,
    );
    assert_eq!(c2, 2, "stderr: {e2}");
    assert!(e2.contains("VS0405"), "stderr: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_script_st44_khong_doi() {
    let d = tam("script");
    let (c, o, e) = chay(
        &d,
        "l",
        "cmd a() -> int:\n  1 + 1\n\n( a() )\n",
        None,
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "2\n");
    let (c2, o2, e2) = chay(
        &d,
        "let",
        "cmd nhan(x: int) -> int:\n  let y: int = x\n  y\n\n( nhan(7) )\n",
        None,
        false,
    );
    assert_eq!(c2, 0, "stderr: {e2}");
    assert_eq!(o2, "7\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_rong_la_vs0201() {
    let d = tam("rong");
    let (c, _, e) = chay(&d, "l", "cmd a() -> int:\nprint(1)\n", None, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    assert!(e.contains("rỗng"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **SỬA KÈM LÝ DO (ST-59)**: thân `cmd` = **vùng khai báo** (`let …`) + **một** pipeline cuối nay
/// **hợp lệ** (trước: `VS0201` "lẫn"). Khoá **hành vi mới**: chạy được, in bảng của `ls .`.
#[test]
fn than_khai_bao_roi_lenh_chay_duoc() {
    let d = tam("lan");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "cmd a() -> int:\n  let y = 1\n  ls ./photos\n\n( a() )\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, BANG, "khai báo + pipeline phải trả bảng `ls`");
    // Cùng byte trên đường VM.
    let (cv, ov, ev) = chay(
        &d,
        "l",
        "cmd a() -> int:\n  let y = 1\n  ls ./photos\n\n( a() )\n",
        Some(&m),
        true,
    );
    assert_eq!((cv, ov.as_str()), (0, o.as_str()), "vm stderr: {ev}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-59**: khai báo **sau** pipeline vẫn `VS0201` (chỉ nhận vùng khai báo **đứng trước**).
#[test]
fn than_khai_bao_sau_lenh_van_la_vs0201() {
    let d = tam("lan-sau");
    let (c, _, e) = chay(
        &d,
        "l",
        "cmd a() -> int:\n  ls .\n  let y = 1\n\n( a() )\n",
        None,
        false,
    );
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201") && e.contains("lẫn"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_lan_lenh_va_bieu_thuc_la_vs0201() {
    let d = tam("lan2");
    let (c, _, e) = chay(
        &d,
        "l",
        "cmd a() -> int:\n  ls .\n  1 + 1\n\n( a() )\n",
        None,
        false,
    );
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_hai_pipeline_la_vs0201() {
    let d = tam("hai");
    let m = mock(&d);
    let (c, _, e) = chay(
        &d,
        "l",
        "cmd loc() -> list[FileEntry]:\n  ls ./photos\n  ls ./photos\n\nloc()\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn item_sau_than_lenh_van_dung_dong_cot() {
    let d = tam("sau");
    // `type` **sau** `cmd` thân-lệnh; lỗi cố ý ở dòng 5 (`n int` thiếu `:`) ⇒ phải báo **5:5**.
    let src = "cmd loc() -> list[FileEntry]:\n  ls ./photos\ntype X:\n  n int\nprint(1)\n";
    let (c, _, e) = chay(&d, "l", src, None, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(
        e.contains(":5:5: error VS0201"),
        "phải đúng dòng:cột gốc: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_long_tra_file_entry_long() {
    let d = tam("long");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "cmd loc() -> list[FileEntryLong]:\n  ls ./photos --long\n\nloc() | take 1\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert!(
        o.starts_with("name   size     modified  is_file  mode       modified_time"),
        "{o}"
    );
    assert!(o.contains("rw-r--r--  2023-11-14 22:13"), "{o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_goi_cmd_khac_trong_pipeline() {
    let d = tam("goi");
    let m = mock(&d);
    let src = "cmd loc() -> list[FileEntry]:\n  ls ./photos\n\ncmd mot() -> list[FileEntry]:\n  loc() | take 1\n\nmot()\n";
    let (c, o, e) = chay(&d, "l", src, Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name   size     modified  is_file\na.jpg   2mb  1700000000s  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_lenh_tren_thu_muc_that() {
    let d = tam("that");
    let sub = d.join("that");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("x.txt"), "abc").unwrap();
    let src = format!(
        "cmd loc() -> list[FileEntry]:\n  ls {}\n\nloc()\n",
        sub.display()
    );
    let (c, o, e) = chay(&d, "l", &src, None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("x.txt"), "{o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn interp_va_vm_cung_byte() {
    let d = tam("vm");
    let m = mock(&d);
    let src = "cmd loc() -> list[FileEntry]:\n  ls ./photos | where size > 1mb | sort_by size --desc\n\nloc()\n";
    let (c1, o1, e1) = chay(&d, "i", src, Some(&m), false);
    let (c2, o2, e2) = chay(&d, "v", src, Some(&m), true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fixture_cmd_body() {
    let m = goc().join("test/cmd/body/mock.json");
    for (ten, muon) in [
        ("body1", BANG),
        (
            "body-multi",
            "name   size     modified  is_file\nb.jpg   6mb  1700086400s  true\na.jpg   2mb  1700000000s  true\n",
        ),
        (
            "body-long",
            "name   size     modified  is_file  mode       modified_time\na.jpg   2mb  1700000000s  true     rw-r--r--  2023-11-14 22:13\n",
        ),
    ] {
        let p = goc().join("test/cmd/body").join(format!("{ten}.vish"));
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
}
