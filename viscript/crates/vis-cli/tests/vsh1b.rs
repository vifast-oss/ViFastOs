// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-46** — ba nợ VSH1b: `ls --long` thật (cột `mode`/`modified_time`), `( … )` làm **đối số**
//! (biểu thức), và chẩn đoán **tuyệt đối** trong `( … )`.
//!
//! Khoá: `--long` ⇒ **6 cột** (`mode` `rw-r--r--`, `modified_time` **UTC** `YYYY-MM-DD HH:MM`), bảng
//! mặc định **4 cột không đổi**; `take ( 1 + 1 )`/`echo ( "a" )`/`( ls … )` chạy; lỗi lúc chạy trong
//! `( … )` báo `dòng:cột` **tuyệt đối**; shell-let ST-44 **không** đổi.

use std::path::{Path, PathBuf};
use std::process::Command;

fn goc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-vsh1b-{tag}-{}", std::process::id()));
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

fn bang_mac_dinh() -> &'static str {
    "name   size     modified  is_file\na.jpg   2mb  1700000000s  true\nb.jpg   6mb  1700086400s  true\n"
}

const BANG_LONG: &str = "name   size     modified  is_file  mode       modified_time\na.jpg   2mb  1700000000s  true     rw-r--r--  2023-11-14 22:13\nb.jpg   6mb  1700086400s  true     rw-------  2023-11-15 22:13\n";

#[test]
fn long_co_du_sau_cot() {
    let d = tam("long");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "ls ./photos --long\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, BANG_LONG);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn long_mode_dung_quyen() {
    let d = tam("mode");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "ls ./photos --long | get mode\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[rw-r--r--, rw-------]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn long_modified_time_utc() {
    let d = tam("time");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "ls ./photos --long | get modified_time\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[2023-11-14 22:13, 2023-11-15 22:13]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khong_long_giu_bang_4_cot() {
    let d = tam("ngan");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "ls ./photos\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, bang_mac_dinh());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn long_where_va_sort_by_van_chay() {
    let d = tam("where");
    let m = mock(&d);
    let (c, o, e) = chay(
        &d,
        "l",
        "ls ./photos --long | where size > 1mb | sort_by size --desc\n",
        Some(&m),
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("b.jpg") && o.contains("rw-------"), "{o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn long_va_khong_long_cung_byte_interp_vm() {
    let d = tam("vm");
    let m = mock(&d);
    let src = "ls ./photos --long\n";
    let (c1, o1, e1) = chay(&d, "i", src, Some(&m), false);
    let (c2, o2, e2) = chay(&d, "v", src, Some(&m), true);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1.as_bytes(), o2.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
}

/// **Ca bắt buộc** (Agent 0): thư mục **thật** trong `/tmp` ⇒ `mode` đọc được quyền thật (khác `-`)
/// và `modified_time` khớp `mtime` của tệp vừa tạo (sai số ≤ 60 s).
#[test]
fn long_tren_thu_muc_that() {
    let d = tam("that");
    let sub = d.join("that");
    std::fs::create_dir_all(&sub).unwrap();
    let f = sub.join("x.txt");
    std::fs::write(&f, "abc").unwrap();
    let mtime = std::fs::metadata(&f).unwrap().modified().unwrap();
    let giay = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let (c, o, e) = chay(
        &d,
        "l",
        &format!("ls {} --long\n", sub.display()),
        None,
        false,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert!(o.contains("mode"), "phải có cột mode: {o}");
    assert!(!o.contains("  -  "), "mode phải đọc được quyền thật: {o}");
    let dong = o.lines().nth(1).unwrap_or("");
    assert!(dong.contains("x.txt"), "{o}");
    let cot: Vec<&str> = dong.split_whitespace().collect();
    // `modified_time` là **hai** token `YYYY-MM-DD` + `HH:MM`.
    let i_ngay = cot
        .iter()
        .position(|x| x.len() == 10 && x.as_bytes()[4] == b'-' && x.as_bytes()[7] == b'-')
        .expect("phải có cột ngày YYYY-MM-DD");
    let mt = format!("{} {}", cot[i_ngay], cot[i_ngay + 1]);
    let (nam, thang, ngay) = (
        mt[0..4].parse::<i64>().unwrap_or(0),
        mt[5..7].parse::<i64>().unwrap_or(0),
        mt[8..10].parse::<i64>().unwrap_or(0),
    );
    let (gio, phut) = (
        mt[11..13].parse::<i64>().unwrap_or(0),
        mt[14..16].parse::<i64>().unwrap_or(0),
    );
    let epoch_phut = giay / 60;
    // Đổi `YYYY-MM-DD HH:MM` ngược về phút epoch để so (civil algorithm).
    let y = nam - (thang <= 2) as i64;
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (thang + 9) % 12;
    let doy = (153 * mp + 2) / 5 + ngay - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let ngay_epoch = era * 146_097 + doe - 719_468;
    let phut_epoch = ngay_epoch * 1440 + gio * 60 + phut;
    assert!(
        (phut_epoch - epoch_phut).abs() <= 1,
        "modified_time lệch quá 60 s: {mt} vs epoch {epoch_phut}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_lam_doi_so_take() {
    let d = tam("take");
    let (c, o, e) = chay(&d, "l", "( [1, 2] ) | take ( 1 + 1 )\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1, 2]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_lam_doi_so_echo() {
    let d = tam("echo");
    let (c, o, e) = chay(&d, "l", "echo ( \"a\" )\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "a\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_lam_doi_so_sai_kieu_la_vs0412() {
    let d = tam("sai");
    let (c, _, e) = chay(&d, "l", "( [1, 2] ) | take ( \"x\" )\n", None, false);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_lam_stage_van_chay() {
    let d = tam("stage");
    let m = mock(&d);
    let (c, o, e) = chay(&d, "l", "( ls ./photos )\n", Some(&m), false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, bang_mac_dinh());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_long_nhau() {
    let d = tam("nested");
    let (c, o, e) = chay(&d, "l", "( ( 1 + 1 ) * 3 )\n", None, false);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "6\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// Lỗi **lúc chạy** trong `( … )` ⇒ `dòng:cột` **tuyệt đối** (không phải vị trí tương đối trong ngoặc).
#[test]
fn chan_doan_tuyet_doi_trong_nhom() {
    let d = tam("diag");
    let src = "print(1)\n( 1 / 0 )\n";
    let (c, _, e) = chay(&d, "l", src, None, false);
    // Lỗi **lúc chạy** ⇒ exit **1** (exit 2 dành cho lỗi biên dịch).
    assert_eq!(c, 1, "stderr: {e}");
    // SPDX là dòng 1, `print(1)` dòng 2, `( 1 / 0 )` dòng 3; `1` ở **cột 3** của tệp.
    assert!(e.contains(":3:3: error VS0502"), "phải tuyệt đối: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn shell_let_st44_khong_doi() {
    // `let` typed nhưng RHS là **lệnh** ⇒ vẫn là shell-let (không phải khai báo) ⇒ `vis check` xanh.
    let d = tam("let");
    let p = d.join("x.vish");
    std::fs::write(
        &p,
        "# SPDX-License-Identifier: MIT OR Apache-2.0\nlet x: int = ls . | take 1\n",
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

/// 4 fixture `.vish` trong `test/cmd/long/**`.
#[test]
fn fixture_cmd_long() {
    let m = goc().join("test/cmd/long/mock.json");
    let mut ca = 0;
    for (ten, muon) in [
        ("long", BANG_LONG),
        (
            "long-where",
            "name   size     modified  is_file  mode       modified_time\nb.jpg   6mb  1700086400s  true     rw-------  2023-11-15 22:13\na.jpg   2mb  1700000000s  true     rw-r--r--  2023-11-14 22:13\n",
        ),
        ("group-arg", "[1, 2]\na\n"),
        ("group-stage", bang_mac_dinh()),
    ] {
        let p = goc().join("test/cmd/long").join(format!("{ten}.vish"));
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
