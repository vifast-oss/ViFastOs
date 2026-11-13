// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-40b** — `get`/`select`: chiếu trường bằng **sugar tên** (VD-71/73), `it` kiểu thật từ prelude.
//! `get` ⇒ `list[K]`; `select` ⇒ `list[bản ghi ẩn danh]` giữ **đúng thứ tự đối số**; trường lạ ⇒
//! **`VS0605`**; `it` không phải bản ghi ⇒ **kiểm động**; chạy **cả hai đường** interp + `--vm`.

use std::path::PathBuf;
use std::process::Command;

const FE: &str = r#"( [{name: "a", size: 6mb, modified: 1m, is_file: true}, {name: "b", size: 1mb, modified: 2m, is_file: true}] )"#;
const PS: &str = r#"( [{pid: 1, name: "a", cpu: 60pct, mem: 10mb}, {pid: 2, name: "b", cpu: 20pct, mem: 5mb}] )"#;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-gs-{tag}-{}", std::process::id()));
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

fn ok(tag: &str, src: &str, muon: &str) {
    let d = tam(tag);
    let (c, o, e) = chay(&d, tag, src, false);
    assert_eq!(c, 0, "[{tag}] stderr: {e}");
    assert_eq!(o, muon, "[{tag}]");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn get_ten_truong() {
    ok("gname", &format!("{FE} | get name\n"), "[a, b]\n");
}

#[test]
fn get_it_tuong_minh_tuong_duong() {
    ok("gname2", &format!("{FE} | get it.name\n"), "[a, b]\n");
}

#[test]
fn get_gia_tri_co_don_vi() {
    ok("gsize", &format!("{FE} | get size\n"), "[6mb, 1mb]\n");
}

#[test]
fn get_noi_tiep_duoc() {
    ok("gchain", &format!("{FE} | get name | count\n"), "2\n");
}

#[test]
fn select_giu_thu_tu_doi_so() {
    ok(
        "sel",
        &format!("{PS} | select pid, name\n"),
        "pid  name\n  1  a\n  2  b\n",
    );
}

#[test]
fn select_dao_thu_tu_doi_so() {
    ok(
        "sel2",
        &format!("{PS} | select name, pid\n"),
        "name  pid\na       1\nb       2\n",
    );
}

#[test]
fn select_ba_truong() {
    ok(
        "sel3",
        &format!("{PS} | select pid, cpu, mem\n"),
        "pid    cpu   mem\n  1  60pct  10mb\n  2  20pct   5mb\n",
    );
}

#[test]
fn get_truong_la_la_vs0605() {
    let d = tam("bad");
    let (c, _, e) = chay(&d, "bad", &format!("{FE} | get khong_co\n"), false);
    assert_ne!(c, 0, "phải lỗi");
    assert!(e.contains("VS0605"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn select_truong_la_la_vs0605() {
    let d = tam("bad2");
    let (c, _, e) = chay(
        &d,
        "bad2",
        &format!("{FE} | select size, khong_co\n"),
        false,
    );
    assert_ne!(c, 0, "phải lỗi");
    assert!(e.contains("VS0605"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn get_khong_doi_so_la_vs0602() {
    let d = tam("noarg");
    let (c, _, e) = chay(&d, "noarg", &format!("{FE} | get\n"), false);
    assert_ne!(c, 0, "phải lỗi");
    assert!(e.contains("VS0602"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `it` **không** phải bản ghi ⇒ **kiểm động**: không `VS0605` oan (Agent 0 chốt).
#[test]
fn kieu_khong_phai_ban_ghi_thi_kiem_dong() {
    let d = tam("dyn");
    let (c, _, e) = chay(&d, "dyn", "( [1, 2] ) | select a\n", false);
    assert_ne!(c, 0, "vẫn lỗi (lúc chạy)");
    assert!(!e.contains("VS0605"), "không được VS0605 oan: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn get_tren_list_rong_la_rong() {
    ok("empty", "( [] ) | get name\n", "[]\n");
}

/// **Ca bắt buộc**: cả interp và `--vm` cho **cùng** stdout.
#[test]
fn get_select_chay_ca_hai_duong() {
    let d = tam("both");
    let src = format!("{FE} | get name\n{PS} | select pid, name\n");
    let (c1, o1, e1) = chay(&d, "i", &src, false);
    let (c2, o2, e2) = chay(&d, "v", &src, true);
    assert_eq!(c1, 0, "interp: {e1}");
    assert_eq!(c2, 0, "vm: {e2}");
    // **SỬA KÈM LÝ DO (ST-41)**: `select` trả `list[bản ghi]` ⇒ giá trị cuối pipeline in **bảng**
    // (`Render`, §9.6), không còn `Show` `[{pid: 1, …}]`.
    assert_eq!(o1, "[a, b]\npid  name\n  1  a\n  2  b\n");
    assert_eq!(o1, o2, "interp và VM phải cùng stdout");
    let _ = std::fs::remove_dir_all(&d);
}
