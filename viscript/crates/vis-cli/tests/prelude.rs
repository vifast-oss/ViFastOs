// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-40a** — prelude bản ghi `FileEntry`/`ProcEntry` cho `.vish`: `it` có **kiểu thật** nên
//! `where`/`sort_by` kiểm tĩnh được (`VS0605` trường lạ, `VS0412` sai kiểu), vẫn chạy **thật** trên
//! cả interp và VM. `.vis` **không** thấy hai tên này.

use std::path::PathBuf;
use std::process::Command;

const FE: &str = r#"( [{name: "a", size: 6mb, modified: 1m, is_file: true}, {name: "b", size: 1mb, modified: 2m, is_file: true}] )"#;
const PS: &str = r#"( [{pid: 1, name: "a", cpu: 60pct, mem: 10mb}, {pid: 2, name: "b", cpu: 20pct, mem: 5mb}] )"#;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-pl-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &std::path::Path, tag: &str, duoi: &str, src: &str, vm: bool) -> (i32, String, String) {
    let p = d.join(format!("{tag}.{duoi}"));
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
    let (c, o, e) = chay(&d, tag, "vish", src, false);
    assert_eq!(c, 0, "[{tag}] stderr: {e}");
    assert_eq!(o, muon, "[{tag}]");
    let _ = std::fs::remove_dir_all(&d);
}

fn loi(tag: &str, src: &str, ma: &str) {
    let d = tam(tag);
    let (c, _, e) = chay(&d, tag, "vish", src, false);
    assert_ne!(c, 0, "[{tag}] phải lỗi, stderr: {e}");
    assert!(e.contains(ma), "[{tag}] cần {ma}, được: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_size_tren_file_entry_chay_that() {
    ok(
        "wsize",
        &format!("{FE} | where size > 5mb\n"),
        "[{name: a, size: 6mb, modified: 1m, is_file: true}]\n",
    );
}

/// **Ca bắt buộc (Agent 0)**: chạy thật **cả** interp **và** `--vm`, stdout **giống nhau**.
#[test]
fn where_size_chay_that_ca_hai_duong() {
    let d = tam("both");
    let src = format!("{FE} | where size > 5mb\n");
    let (c1, o1, e1) = chay(&d, "i", "vish", &src, false);
    let (c2, o2, e2) = chay(&d, "v", "vish", &src, true);
    assert_eq!(c1, 0, "interp stderr: {e1}");
    assert_eq!(c2, 0, "vm stderr: {e2}");
    assert_eq!(o1, o2, "interp và VM phải cùng stdout");
    assert_eq!(o1, "[{name: a, size: 6mb, modified: 1m, is_file: true}]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn it_viet_tuong_minh_tuong_duong() {
    ok(
        "wexpl",
        &format!("{FE} | where it.size > 5mb\n"),
        "[{name: a, size: 6mb, modified: 1m, is_file: true}]\n",
    );
}

#[test]
fn truong_la_la_vs0605() {
    loi("khong", &format!("{FE} | where khong_co > 1\n"), "VS0605");
}

#[test]
fn so_sanh_sai_kieu_la_vs0412() {
    loi("saikieu", &format!("{FE} | where size > \"x\"\n"), "VS0412");
}

#[test]
fn so_sanh_cung_don_vi_size_thi_ok() {
    ok(
        "donvi",
        &format!("{FE} | where size > 5kb\n"),
        "[{name: a, size: 6mb, modified: 1m, is_file: true}, {name: b, size: 1mb, modified: 2m, is_file: true}]\n",
    );
}

#[test]
fn duration_so_sanh_duoc() {
    ok(
        "dur",
        &format!("{FE} | where modified > 90s\n"),
        "[{name: b, size: 1mb, modified: 2m, is_file: true}]\n",
    );
}

#[test]
fn proc_cpu_la_percent() {
    ok(
        "cpu",
        &format!("{PS} | where cpu > 50pct\n"),
        "[{pid: 1, name: a, cpu: 60pct, mem: 10mb}]\n",
    );
}

#[test]
fn proc_cpu_sai_don_vi_la_vs0412() {
    loi("cpu2", &format!("{PS} | where cpu > 50\n"), "VS0412");
}

#[test]
fn sort_by_khoa_co_kieu_that() {
    ok(
        "sort",
        &format!("{FE} | sort_by size --desc\n"),
        "[{name: a, size: 6mb, modified: 1m, is_file: true}, {name: b, size: 1mb, modified: 2m, is_file: true}]\n",
    );
}

#[test]
fn sort_by_khoa_la_la_vs0605() {
    loi("sort2", &format!("{FE} | sort_by khong_co\n"), "VS0605");
}

/// Không suy được kiểu (`list[int]`) ⇒ **kiểm động**, không báo oan.
#[test]
fn khong_suy_duoc_kieu_thi_khong_bao_oan() {
    ok("dynamic", "( [1, 2] ) | where it > 1\n", "[2]\n");
}

/// `.vis` **không** thấy prelude: người dùng khai `type FileEntry` của riêng mình vẫn chạy.
#[test]
fn vis_khong_thay_prelude() {
    let d = tam("hid");
    let (c, o, e) = chay(
        &d,
        "u",
        "vis",
        "type FileEntry:\n    ten: str\n\nlet f: FileEntry = {ten: \"x\"}\nprint(f.ten)\n",
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "x\n");
    let _ = std::fs::remove_dir_all(&d);
}
