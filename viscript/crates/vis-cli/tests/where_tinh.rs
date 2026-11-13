// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-45** — `where` kiểm tĩnh đầy đủ (thân phải `bool`), `each` **không** kiểm kiểu trả.
//!
//! Khoá: `where size` / `where it.size` / `where 1` / `where x => x` trên `list[FileEntry]` ⇒ **`VS0412`**
//! nêu **kiểu thật**; `where size > 5mb` sạch; `each` trả `int`/bản ghi ⇒ sạch nhưng **không nuốt** lỗi
//! trường (`VS0403`/`VS0405`); `it` **không biết kiểu** ⇒ **không** lỗi tĩnh (giữ `VS0501` lúc chạy);
//! `.vish` **khai báo** (ST-44) + `cmd` trả `list[FileEntry]` cũng kiểm tĩnh.

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
    let d = std::env::temp_dir().join(format!("vis-where-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &Path, tag: &str, src: &str, vm: bool) -> (i32, String, String) {
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

const FE: &str = "( [{name: \"a\", size: 6mb, modified: 1h, is_file: true}] )";

#[test]
fn where_size_la_vs0412() {
    let d = tam("size");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | where size\n"), false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(
        e.contains("bool") && e.contains("size"),
        "nêu kiểu thật: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_it_size_la_vs0412() {
    let d = tam("itsize");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | where it.size\n"), false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_1_la_vs0412() {
    let d = tam("mot");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | where 1\n"), false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(e.contains("int"), "nêu kiểu thật: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_closure_khong_it_la_vs0412() {
    let d = tam("cl");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | where x => x\n"), false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(e.contains("FileEntry"), "nêu kiểu thật: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_so_sanh_bool_thi_sach() {
    let d = tam("ok");
    let src = "( [{name: \"a\", size: 6mb, modified: 1h, is_file: true}, {name: \"b\", size: 1mb, modified: 2h, is_file: true}] ) | where size > 5mb\n";
    let (c, o, e) = chay(&d, "w", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name  size  modified  is_file\na      6mb        1h  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_tren_kieu_khong_biet_khong_loi_tinh() {
    let d = tam("any");
    // `( [] )` ⇒ element **không suy được** ⇒ tyck **không** được đỏ oan (kiểm động giữ nguyên).
    let (c, o, e) = chay(&d, "w", "( [] ) | where size\n", false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn each_tra_int_sach() {
    let d = tam("eint");
    let (c, o, e) = chay(&d, "w", "( [1, 2] ) | each x => x + 1\n", false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "()\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn each_tra_ban_ghi_sach() {
    let d = tam("erec");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | each x => x\n"), false);
    assert_eq!(c, 0, "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn each_van_bao_truong_khong_co() {
    let d = tam("efield");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | each x => x.khong_co\n"), false);
    assert_eq!(c, 2, "stderr: {e}");
    // **SỬA KÈM LÝ DO (ST-50, Agent 0: VS0605 sai sổ — trường là lỗi kiểu)**: closure tường minh
    // nay cũng ra **`VS0405`** (một mã duy nhất cho "không có trường"); trước là `VS0403`.
    assert!(e.contains("VS0405"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_viet_tat_truong_khong_co_la_vs0405() {
    let d = tam("sugar");
    let (c, _, e) = chay(&d, "w", &format!("{FE} | where khong_co\n"), false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0405"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **Bắt buộc** (Agent 0 chốt): `.vish` **khai báo** + `cmd` trả `list[FileEntry]` ⇒ kiểm tĩnh **thật**.
#[test]
fn khai_bao_cmd_where_size_la_vs0412() {
    let d = tam("decl");
    let src = "cmd loc() -> list[FileEntry]:\n  [{name: \"a\", size: 6mb, modified: 1h, is_file: true}]\n\nloc() | where size\n";
    let (c, _, e) = chay(&d, "w", src, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(e.contains("size"), "nêu kiểu thật: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khai_bao_cmd_where_bool_thi_chay_duoc() {
    let d = tam("declok");
    let src = "cmd loc() -> list[FileEntry]:\n  [{name: \"a\", size: 6mb, modified: 1h, is_file: true}]\n\nloc() | where size > 5mb\n";
    let (c, o, e) = chay(&d, "w", src, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "name  size  modified  is_file\na      6mb        1h  true\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_interp_va_vm_cung_byte() {
    let d = tam("vm");
    let src = "( [{name: \"a\", size: 6mb, modified: 1h, is_file: true}] ) | where size > 5mb\n";
    let (c1, o1, e1) = chay(&d, "i", src, false);
    let (c2, o2, e2) = chay(&d, "v", src, true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
}

/// 3 fixture `.vish` trong `test/cmd/where/**`.
#[test]
fn fixture_cmd_where() {
    let mut ca = 0;
    for (ten, muon) in [
        (
            "where-ok",
            "name  size  modified  is_file\na      6mb        1h  true\n",
        ),
        ("each-kieu", "()\n"),
        ("each-in", "a\nb\n()\n"),
    ] {
        let p = goc().join("test/cmd/where").join(format!("{ten}.vish"));
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
    assert_eq!(ca, 3);
}
