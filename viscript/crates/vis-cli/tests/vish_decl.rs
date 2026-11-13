// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-44** — `.vish` có **khai báo kiểu**: vùng item ở đầu tệp (`type`/`enum`/`cmd`/`let` có kiểu)
//! hạ bằng đường script (span tuyệt đối), stage chạy sau như vũng 6; trùng tên ⇒ `VS0302`.
//!
//! Khoá: `cmd f(...) -> list[FileEntry]` cho `f() | where size > 5mb` **kiểm tĩnh** (không `VS0501`);
//! khai báo **sau** stage ⇒ `VS0201` kèm gợi ý; `.vish` **không** khai báo giữ nguyên hành vi vũng 6
//! (4 fixture `test/cmd/fs/**` của ST-42 vẫn xanh); `.vis` **không** bị ảnh hưởng.

use std::path::{Path, PathBuf};
use std::process::Command;

fn goc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-decl-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &Path, tag: &str, ext: &str, src: &str, vm: bool) -> (i32, String, String) {
    let p = d.join(format!("{tag}.{ext}"));
    let spdx = "#"; // `.vis` **và** `.vish` đều dùng `#` cho SPDX (QD §9.3).
    std::fs::write(
        &p,
        format!("{spdx} SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
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

const HAI: &str = "type Hop:\n  size: size\n  ten: str\n\nlet h: Hop = {size: 6mb, ten: \"a\"}\n";

#[test]
fn type_va_let_dung_duoc_truong() {
    let d = tam("hop");
    let (c, o, e) = chay(&d, "hop", "vish", &format!("{HAI}( h.size )\n"), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "6mb\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn enum_trong_vish() {
    let d = tam("enum");
    let src = "enum Mau:\n  Do\n  Xanh\n\nlet m: Mau = Mau.Do\n( str(m) )\n";
    let (c, o, e) = chay(&d, "enum", "vish", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("Do"), "enum: {o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn cmd_tra_record_list_kiem_tinh_qua_khe_ong() {
    let d = tam("cmd");
    let src = "cmd loc() -> list[FileEntry]:\n  [{name: \"a\", size: 6mb, modified: 1h, is_file: true},\n   {name: \"b\", size: 1mb, modified: 2h, is_file: true}]\n\nloc() | where size > 5mb\n";
    let (c, o, e) = chay(&d, "cmd", "vish", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name  size  modified  is_file\na      6mb        1h  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn cmd_rong_ra_rong_dung_render() {
    let d = tam("rong");
    let src = "cmd loc() -> list[FileEntry]:\n  []\n\nloc() | where size > 5mb\n";
    let (c, o, e) = chay(&d, "rong", "vish", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "(rỗng)\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn cmd_co_tham_so_va_let_trong_than() {
    let d = tam("ts");
    let src = "cmd nhan(x: int) -> int:\n  let y: int = x\n  y\n\n( nhan(7) )\n";
    let (c, o, e) = chay(&d, "ts", "vish", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "7\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn type_trung_ten_trong_tep_la_vs0302() {
    let d = tam("dup");
    let src = "type A:\n  x: int\n\ntype A:\n  y: int\n";
    let (c, _, e) = chay(&d, "dup", "vish", src, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0302"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn type_trung_prelude_la_vs0302() {
    let d = tam("prel");
    for ten in ["FileEntry", "ProcEntry"] {
        let src = format!("type {ten}:\n  x: int\n");
        let (c, _, e) = chay(&d, &format!("p{ten}"), "vish", &src, false);
        assert_eq!(c, 2, "{ten}: {e}");
        assert!(e.contains("VS0302"), "{ten} stderr: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khai_bao_sau_stage_la_vs0201() {
    let d = tam("sau");
    let src = "( 1 )\ntype A:\n  x: int\n";
    let (c, _, e) = chay(&d, "sau", "vish", src, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    assert!(e.contains("trước"), "phải kèm gợi ý: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn let_co_kieu_dung_script() {
    let d = tam("let");
    let (c, o, e) = chay(&d, "let", "vish", "let z: int = 5\n( z )\n", false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "5\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn let_shell_van_la_stage() {
    // `let` **không** phải khai báo kiểu khi RHS là **pipeline** (golden `test/cmd/golden.cmd` ca 24).
    let d = tam("shell");
    let (c, _, e) = chay(&d, "shell", "vish", "let x: int = ls | grep foo\n", false);
    // Không có lệnh `ls` trong môi trường test ⇒ lỗi **lúc chạy**, nhưng **parse/lower** phải xanh.
    assert!(
        !e.contains("VS0201") || !e.contains("khai báo phải đứng"),
        "shell-let không được coi là khai báo: {e}"
    );
    let _ = (c, d);
}

#[test]
fn vish_khong_khai_bao_giu_nguyen_vung_6() {
    let d = tam("cu");
    let (c, o, e) = chay(&d, "cu", "vish", "( [1, 2] ) | take 1\n", false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn vis_khong_bi_anh_huong() {
    // `.vis` giữ **script** thuần: `type` + `let` + `print`.
    let d = tam("vis");
    let src = "type P:\n  x: int\n\nlet p: P = {x: 3}\nprint(p.x)\n";
    let (c, o, e) = chay(&d, "vis", "vis", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "3\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_rong_dung_mot_minh_la_vs0201() {
    let d = tam("rong2");
    let (c, _, e) = chay(&d, "rong2", "vish", "( )\n", false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn in_va_vm_cung_byte() {
    let d = tam("vm");
    let src = "cmd loc() -> list[FileEntry]:\n  [{name: \"a\", size: 6mb, modified: 1h, is_file: true}]\n\nloc() | where size > 5mb | sort_by size --desc\n";
    let (c1, o1, e1) = chay(&d, "i", "vish", src, false);
    let (c2, o2, e2) = chay(&d, "v", "vish", src, true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
}

/// 4 fixture `.vish` trong `test/cmd/decl/**`.
#[test]
fn fixture_cmd_decl() {
    let mut ca = 0;
    for (ten, muon) in [
        ("type-let", "6mb\n"),
        (
            "cmd-where",
            "name  size  modified  is_file\na      6mb        1h  true\n",
        ),
        ("cmd-empty", "(rỗng)\n"),
        ("let-typed", "5\n"),
    ] {
        let p = goc().join("test/cmd/decl").join(format!("{ten}.vish"));
        let o = Command::new(env!("CARGO_BIN_EXE_vis"))
            .arg("run")
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

/// **Bắt buộc**: 4 fixture `test/cmd/fs/**` của ST-42 vẫn xanh ⇒ `.vish` không khai báo giữ nguyên.
#[test]
fn fixture_st42_van_xanh() {
    let m = goc().join("test/cmd/fs/mock.json");
    let mut ca = 0;
    for ten in ["ls", "ls-all", "open", "doc672"] {
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
            "fixture fs/{ten}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(!o.stdout.is_empty(), "fixture fs/{ten} phải có đầu ra");
        ca += 1;
    }
    assert_eq!(ca, 4);
}
