// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-42** — `ls`/`open` qua `FsHost` + `--fs-mock` (ST-40a `FileEntry`, `Render` ST-41).
//!
//! Khoá: `ls` ⇒ `list[FileEntry]` **sắp theo tên byte**, `size` = **byte**, `modified` = **duration**;
//! `--all` gồm `.`; `--long` no-op; `open` ⇒ `str`, lỗi ⇒ **`VS0501`** nêu đường dẫn; khối 672 chạy
//! **end-to-end** với mock và **interp ↔ `--vm` cùng byte**; không mock ⇒ hệ tệp thật.

use std::path::{Path, PathBuf};
use std::process::Command;

fn goc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-ls-{tag}-{}", std::process::id()));
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
  {"name": "b.jpg", "size": "6mb", "modified": "2h", "is_file": true},
  {"name": "a.jpg", "size": "1mb", "modified": "3h", "is_file": true},
  {"name": ".hidden", "size": "1kb", "modified": "1h", "is_file": true} ] },
  "files": { "./notes.txt": "xin chào" } }"#;

fn mock(d: &Path) -> PathBuf {
    let p = d.join("mock.json");
    std::fs::write(&p, MOCK).expect("ghi mock");
    p
}

const BANG_LS: &str =
    "name   size  modified  is_file\na.jpg   1mb        3h  true\nb.jpg   6mb        2h  true\n";

#[test]
fn ls_mock_ra_bang() {
    let d = tam("ls");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "ls", "ls ./photos\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, BANG_LS);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ls_sap_theo_ten_byte() {
    let d = tam("sort");
    let mut js = String::from(r#"{ "dirs": { "./x": ["#);
    for n in ["zz", "aa", "mm"] {
        js.push_str(&format!(
            r#"{{"name": "{n}", "size": 1, "modified": "1h", "is_file": true}},"#
        ));
    }
    js.pop();
    js.push_str("] } }");
    let p = d.join("m.json");
    std::fs::write(&p, js).unwrap();
    let (c, o, e) = chay(&d, "sort", "ls ./x | get name\n", Some(&p), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[aa, mm, zz]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ls_all_gom_ten_cham() {
    let d = tam("all");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "all", "ls ./photos --all | get name\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[.hidden, a.jpg, b.jpg]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ls_long_la_co_hop_le_va_no_op() {
    let d = tam("long");
    let m = mock(&d);
    let (c1, o1, e1) = chay(&d, "k", "ls ./photos\n", Some(&m), false);
    let (c2, o2, e2) = chay(&d, "l", "ls ./photos --long\n", Some(&m), false);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1, o2, "`--long` chưa đổi hành vi (nợ ghi JOURNAL)");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ls_khong_doi_so_dung_cham() {
    let d = tam("dot");
    let p = d.join("m.json");
    std::fs::write(
        &p,
        r#"{ "dirs": { ".": [{"name": "x", "size": 1, "modified": "1h", "is_file": true}] } }"#,
    )
    .unwrap();
    let (c, o, e) = chay(&d, "dot", "ls | get name\n", Some(&p), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[x]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn size_la_byte_theo_bang_lexer() {
    // `2mb` = 2 000 000 (bảng lexer), `2mib` = 2 097 152.
    let d = tam("mb");
    let p = d.join("m.json");
    std::fs::write(
        &p,
        r#"{ "dirs": { "./x": [{"name": "dec", "size": "2mb", "modified": 0, "is_file": true},
                               {"name": "bin", "size": "2mib", "modified": 0, "is_file": true}] } }"#,
    )
    .unwrap();
    // `dec` (`2mb`) nằm **đúng** 2 000 000 byte; `bin` (`2mib`) = 2 097 152 byte.
    let (c1, o1, e1) = chay(
        &d,
        "mb1",
        "ls ./x | where size > 1999999b | where size < 2000001b | get name\n",
        Some(&p),
        false,
    );
    assert_eq!(c1, 0, "stderr: {e1}");
    assert_eq!(o1, "[dec]\n", "2mb phải là 2 000 000 byte");
    let (c2, o2, e2) = chay(
        &d,
        "mb2",
        "ls ./x | where size > 2000000b | get name\n",
        Some(&p),
        false,
    );
    assert_eq!(c2, 0, "stderr: {e2}");
    assert_eq!(o2, "[bin]\n", "2mib phải là 2 097 152 byte");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn modified_la_duration() {
    let d = tam("mod");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "mod", "ls ./photos | get modified\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[3h, 2h]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn open_mock_doc_noi_dung() {
    let d = tam("open");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "open", "open ./notes.txt\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "xin chào\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn open_thieu_tep_la_vs0501() {
    let d = tam("openbad");
    let m = mock(&d);
    let (c, _, e) = chay(&d, "ob", "open ./khong-co.txt\n", Some(&m), false);
    assert_ne!(c, 0);
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(e.contains("./khong-co.txt"), "phải nêu đường dẫn: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ls_thu_muc_khong_ton_tai_la_vs0501() {
    let d = tam("lsbad");
    let m = mock(&d);
    let (c, _, e) = chay(&d, "lb", "ls ./khong-co\n", Some(&m), false);
    assert_ne!(c, 0);
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(e.contains("./khong-co"), "phải nêu đường dẫn: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **Khối 672 end-to-end**: `ls ./photos | where size > 5mb | sort_by modified --desc | take 10`.
#[test]
fn khoi_672_end_to_end() {
    let d = tam("672");
    let m = mock(&d);
    let src = "ls ./photos | where size > 5mb | sort_by modified --desc | take 10\n";
    let (c, o, e) = chay(&d, "672", src, Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name   size  modified  is_file\nb.jpg   6mb        2h  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Khối 672 **interp ↔ `--vm` cùng byte**.
#[test]
fn khoi_672_interp_va_vm_cung_byte() {
    let d = tam("672vm");
    let m = mock(&d);
    let src = "ls ./photos | where size > 5mb | sort_by modified --desc | take 10\n";
    let (c1, o1, e1) = chay(&d, "i", src, Some(&m), false);
    let (c2, o2, e2) = chay(&d, "v", src, Some(&m), true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
}

/// Không `--fs-mock` ⇒ **hệ tệp thật** (thư mục trong `/tmp`).
#[test]
fn khong_mock_thi_dung_fs_that() {
    let d = tam("real");
    let sub = d.join("thu-muc");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("b.txt"), "bb").unwrap();
    std::fs::write(sub.join("a.txt"), "a").unwrap();
    let src = format!("ls {}\n", sub.display());
    let (c, o, e) = chay(&d, "real", &src, None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.starts_with("name"), "phải là bảng: {o}");
    let a = o.find("a.txt").unwrap();
    let b = o.find("b.txt").unwrap();
    assert!(a < b, "sắp theo tên: {o}");
    let (c2, o2, e2) = chay(
        &d,
        "realopen",
        &format!("open {}\n", sub.join("a.txt").display()),
        None,
        false,
    );
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o2, "a\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// 4 fixture `.vish` trong `test/cmd/fs/**` chạy với `--fs-mock`.
#[test]
fn fixture_cmd_fs() {
    let m = goc().join("test/cmd/fs/mock.json");
    let mut ca = 0;
    for (ten, muon) in [
        ("ls", BANG_LS),
        (
            "ls-all",
            "name     size  modified  is_file\n.hidden   1kb        1h  true\na.jpg     1mb        3h  true\nb.jpg     6mb        2h  true\n",
        ),
        ("open", "xin chào\n"),
        ("doc672", "name   size  modified  is_file\nb.jpg   6mb        2h  true\n"),
    ] {
        let p = goc().join("test/cmd/fs").join(format!("{ten}.vish"));
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
        ca += 1;
    }
    assert_eq!(ca, 4);
}
