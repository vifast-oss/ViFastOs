// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-27 — **chương trình ngoài** (`/bin/echo`…), variadic, `--no-extern`, timeout.
//!
//! Hermetic: mọi đường dẫn tuyệt đối **được kiểm tồn tại** trước khi dùng (`assert!`) — không `PATH`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    // Duy nhất **mỗi lần gọi**: test chạy song song, trùng tên thư mục sẽ xoá lẫn nhau.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-ext-{}-{}-{}", tag, std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn vis(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn co(duong: &str) {
    assert!(
        Path::new(duong).exists(),
        "thiếu chương trình hermetic `{duong}` — test phải đỏ, không fallback PATH"
    );
}

fn chay_vish(noi_dung: &str, env: &[(&str, &str)]) -> (i32, String, String) {
    let d = tmp_dir("run");
    let p = d.join("t.vish");
    std::fs::write(&p, noi_dung).expect("ghi kịch bản");
    let (c, o, e) = vis(&["run", p.to_str().unwrap()], env);
    let _ = std::fs::remove_dir_all(&d);
    (c, o, e)
}

#[test]
fn duong_dan_hermetic_co_that() {
    for p in [
        "/bin/echo",
        "/usr/bin/printf",
        "/usr/bin/true",
        "/usr/bin/false",
        "/bin/cat",
    ] {
        co(p);
    }
}

#[test]
fn echo_ra_list_str() {
    co("/bin/echo");
    let (c, o, e) = chay_vish("/bin/echo hi\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[hi]\n");
}

#[test]
fn printf_nhieu_dong_bo_newline_cuoi() {
    co("/usr/bin/printf");
    let (c, o, e) = chay_vish("/usr/bin/printf \"a\\nb\\n\"\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[a, b]\n", "phải tách dòng và **bỏ** newline cuối");
}

#[test]
fn echo_raw_giu_newline() {
    co("/bin/echo");
    let (c, o, e) = chay_vish("/bin/echo hi --raw\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o, "hi\n\n",
        "--raw ⇒ `str` nguyên văn (kể cả newline) rồi `print` thêm `\\n`"
    );
}

#[test]
fn true_khong_in_gi() {
    co("/usr/bin/true");
    let (c, o, e) = chay_vish("/usr/bin/true\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[]\n");
}

#[test]
fn false_thoat_ma_1_la_vs0501() {
    co("/usr/bin/false");
    let (c, _, e) = chay_vish("/usr/bin/false\n", &[]);
    assert_eq!(c, 1, "stderr: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(e.contains("thoát mã 1"), "phải nêu mã thoát: {e}");
}

#[test]
fn cat_doc_stdin_rong_la_rong() {
    co("/bin/cat");
    let (c, o, e) = chay_vish("/bin/cat\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[]\n", "stdin đóng ⇒ rỗng");
}

#[test]
fn stderr_khong_tron_stdout() {
    // `/bin/cat /khong/co` ⇒ exit 1 + stderr thật, stdout **rỗng**.
    co("/bin/cat");
    let (c, o, e) = chay_vish("/bin/cat /khong/co/that\n", &[]);
    assert_eq!(c, 1);
    assert_eq!(o, "", "stdout KHÔNG được trộn stderr");
    assert!(e.contains("VS0501"), "stderr: {e}");
}

#[test]
fn no_extern_la_vs0601() {
    let d = tmp_dir("noext");
    let p = d.join("t.vish");
    std::fs::write(&p, "/bin/echo hi\n").expect("ghi");
    let (c, _, e) = vis(&["check", "--no-extern", p.to_str().unwrap()], &[]);
    assert_eq!(c, 1, "stderr: {e}");
    let (c2, o2, e2) = vis(&["check", p.to_str().unwrap()], &[]);
    assert_eq!(c2, 0, "mặc định phải xanh cho lệnh lạ: {o2} {e2}");
    assert!(o2.contains("ok 1"), "TAP: {o2}");
    // Chạy với `--no-extern` ⇒ `VS0601`.
    let (c3, _, e3) = vis(&["run", "--no-extern", p.to_str().unwrap()], &[]);
    assert_eq!(c3, 2, "stderr: {e3}");
    assert!(e3.contains("VS0601"), "stderr: {e3}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn explain_perms_in_nhat_ky() {
    co("/bin/echo");
    let (c, o, e) = chay_vish("/bin/echo hi\n", &[("VIS_EXPLAIN", "1")]);
    assert_eq!(c, 0);
    assert_eq!(o, "[hi]\n");
    // Không bật cờ thì **không** có dòng EXEC.
    assert!(!e.contains("EXEC"), "mặc định không in nhật ký: {e}");
    let d = tmp_dir("perms");
    let p = d.join("t.vish");
    std::fs::write(&p, "/bin/echo hi\n").expect("ghi");
    let (_, _, e2) = vis(&["run", "--explain-perms", p.to_str().unwrap()], &[]);
    assert!(e2.contains("EXEC /bin/echo"), "phải in nhật ký quyền: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn timeout_la_vs0501() {
    // `/bin/sleep 5` ⇒ chờ; timeout 50 ms ⇒ VS0501 "quá thời gian".
    co("/bin/sleep");
    let (c, _, e) = chay_vish("/bin/sleep 5\n", &[("VIS_CMD_TIMEOUT_MS", "50")]);
    assert_eq!(c, 1, "stderr: {e}");
    assert!(
        e.contains("VS0501") && e.contains("thời gian"),
        "stderr: {e}"
    );
}

#[test]
fn variadic_gom_da_so_con_lai() {
    // `cmd x(a: str, ...rest: str)` gọi `x "a" "b" "c"` ⇒ rest = ["b","c"].
    let d = tmp_dir("var");
    let p = d.join("t.vis");
    std::fs::write(
        &p,
        "cmd x(a: str, ...rest: str) -> list[str]:\n    rest\n\nprint(x(\"a\", \"b\", \"c\"))\n",
    )
    .expect("ghi");
    let (c, o, e) = vis(&["run", p.to_str().unwrap()], &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[b, c]\n", "rest phải là [\"b\", \"c\"]");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn echo_trong_vish_van_chay() {
    // Lệnh **nội bộ** (`echo`) không bị đổi nghĩa bởi chương trình ngoài.
    let (c, o, e) = chay_vish("echo xin_chao\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "xin_chao\n");
}
