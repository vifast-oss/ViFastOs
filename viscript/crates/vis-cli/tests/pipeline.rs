// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-34** — `| complete` + **pipe→stdin** cho chương trình ngoài.
//!
//! Khoá: `a | b` ghi **stdout của `a`** vào **stdin** của `b` (đóng stdin sau khi ghi); `complete` cho
//! record `{stdout: list[str], stderr: list[str], code: int}` và **exit≠0 không còn là lỗi**;
//! `complete` trên stage **Vis** ⇒ `VS0602`; pipe vẫn là **giá trị** (không đổi HIR/codegen).
//! Hermetic: chỉ dùng `/bin/echo`, `/usr/bin/printf`, `/bin/cat`, `/usr/bin/grep` — **assert tồn tại**.

use std::path::PathBuf;
use std::process::Command;

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

/// **Assert tồn tại** chương trình ngoài dùng trong test (như ST-27) — thiếu thì đỏ ngay, không flake.
fn co(ten: &str) {
    assert!(
        std::path::Path::new(ten).exists(),
        "thiếu chương trình ngoài `{ten}` (test giả định POSIX có nó)"
    );
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-pipe-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &std::path::Path, tag: &str, src: &str, env: &[(&str, &str)]) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vish"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run").arg(&p);
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

// ---------- hermetic: assert tồn tại ----------

#[test]
fn chuong_trinh_ngoai_co_that() {
    for t in [
        "/bin/echo",
        "/usr/bin/printf",
        "/bin/cat",
        "/usr/bin/grep",
        "/bin/sleep",
    ] {
        co(t);
    }
}

// ---------- pipe → stdin ----------

#[test]
fn echo_qua_cat_nhan_stdin() {
    let d = tam("cat");
    let (c, o, e) = chay(&d, "cat", "/bin/echo a | /bin/cat\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[a]\n", "cat phải nhận stdin từ echo");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn echo_qua_grep_tim_thay() {
    let d = tam("grep");
    let (c, o, e) = chay(&d, "grep", "/bin/echo a | /usr/bin/grep a\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[a]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn grep_khong_khop_van_la_loi_khi_khong_complete() {
    let d = tam("grep0");
    let (c, _, e) = chay(&d, "grep0", "/bin/echo x | /usr/bin/grep z\n", &[]);
    assert_eq!(c, 1, "exit≠0 ⇒ VS0501 như VD-60");
    assert!(
        e.contains("VS0501") && e.contains("thoát mã 1"),
        "stderr: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- complete ----------

#[test]
fn complete_grep_khong_khop_het_la_loi() {
    let d = tam("cmp");
    let (c, o, e) = chay(&d, "cmp", "/bin/echo x | /usr/bin/grep z | complete\n", &[]);
    assert_eq!(c, 0, "complete ⇒ **không** còn VS0501: {e}");
    assert_eq!(o, "{stdout: [], stderr: [], code: 1}\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn complete_grep_khop_code_0() {
    let d = tam("cmp2");
    let (c, o, _) = chay(
        &d,
        "cmp2",
        "/bin/echo x | /usr/bin/grep x | complete\n",
        &[],
    );
    assert_eq!(c, 0);
    assert_eq!(o, "{stdout: [x], stderr: [], code: 0}\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn complete_giu_stderr_va_code_khac_0() {
    let d = tam("cmp3");
    // `grep` với cờ lạ ⇒ exit 2 + stderr; `complete` giữ **cả hai**, không thành VS0501.
    let (c, o, e) = chay(&d, "cmp3", "/usr/bin/grep --khong-co | complete\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert!(
        o.contains("stderr: [grep:"),
        "stderr phải có trong record: {o}"
    );
    assert!(o.contains("code: 2"), "code 2: {o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn complete_tren_stage_vis_la_vs0602() {
    let d = tam("vis");
    let (c, _, e) = chay(&d, "vis", "echo x | complete\n", &[]);
    assert_eq!(c, 2);
    assert!(e.contains("VS0602"), "stderr: {e}");
    assert!(
        e.contains("complete chỉ dùng sau chương trình ngoài"),
        "stderr: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn complete_co_doi_so_la_vs0602() {
    let d = tam("vis2");
    let (c, _, e) = chay(&d, "vis2", "/bin/echo a | complete 1\n", &[]);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0602"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- quy tắc nối stdin: str nguyên văn vs list[str] ----------

#[test]
fn printf_khong_newline_qua_cat_khong_them_dong_rong() {
    let d = tam("printf");
    let (c, o, e) = chay(&d, "printf", "/usr/bin/printf \"x\" | /bin/cat\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[x]\n", "**một** dòng, không thừa dòng rỗng");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn printf_hai_dong_qua_cat_ra_hai_dong() {
    let d = tam("printf2");
    let (c, o, _) = chay(&d, "printf2", "/usr/bin/printf \"a\\nb\" | /bin/cat\n", &[]);
    assert_eq!(c, 0);
    assert_eq!(o, "[a, b]\n", "hai dòng, không dòng rỗng");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn list_nhieu_dong_qua_grep_loc_dung_dong() {
    let d = tam("list");
    let (c, o, _) = chay(
        &d,
        "list",
        "/usr/bin/printf \"a\\nb\" | /usr/bin/grep b\n",
        &[],
    );
    assert_eq!(c, 0);
    assert_eq!(o, "[b]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn raw_str_di_nguyen_van_khong_them_newline() {
    let d = tam("raw");
    // `--raw` ⇒ `str` nguyên văn ⇒ **không** nối thêm `\n`; `cat` in đúng một dòng.
    let (c, o, e) = chay(&d, "raw", "/usr/bin/printf x --raw | /bin/cat\n", &[]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[x]\n");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- timeout nêu **stage** ----------

#[test]
fn timeout_neu_ro_stage() {
    let d = tam("to");
    let (c, _, e) = chay(
        &d,
        "to",
        "/bin/sleep 5 | /bin/cat\n",
        &[("VIS_CMD_TIMEOUT_MS", "50")],
    );
    assert_eq!(c, 1, "stderr: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(
        e.contains("stage `/bin/sleep`"),
        "thông điệp timeout phải nêu stage: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- fixture `.vish` trong `test/cmd/pipe/**` ----------

#[test]
fn fixture_vish_trong_test_cmd_pipe() {
    let mut ca = 0;
    for (ten, muon) in [
        ("cat", "[a]\n"),
        ("grep", "[a]\n"),
        ("printf", "[x]\n"),
        ("complete", "{stdout: [], stderr: [], code: 1}\n"),
    ] {
        let p = goc().join("test/cmd/pipe").join(format!("{ten}.vish"));
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
    assert_eq!(ca, 4, "phải chạy đủ 4 fixture `.vish`");
}

// ---------- VM: không host ⇒ VS0604 (đường bytecode) ----------

#[test]
fn vm_khong_host_bao_vs0604() {
    let d = tam("vm");
    let p = d.join("vm.vish");
    std::fs::write(
        &p,
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n/bin/echo a | /bin/cat\n",
    )
    .expect("ghi");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(["run", "--vm", "--no-cache"])
        .arg(&p)
        .output()
        .expect("chạy --vm");
    let e = String::from_utf8_lossy(&o.stderr).to_string();
    assert_eq!(o.status.code().unwrap_or(-1), 1, "stderr: {e}");
    assert!(
        e.contains("VS0604") && e.contains("không có host"),
        "stderr: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
