// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-53** — `select` (bản ghi ẩn danh) đi `to json` + **hợp nhất** hai `sort_by`.
//!
//! Khoá: `to json` là **stage lệnh** (không cần `use std.json`), `--pretty` ⇒ encode_pretty;
//! JSON của bản ghi (kể cả **ẩn danh**) theo **khoá trường + thứ tự**; shell `sort_by` khoá
//! `str/size/duration` ổn định + `--desc`; `std.list.sort_by` cũ (`fn(int)->int`) không đổi;
//! khoá `bool` ⇒ `VS0412`; interp ↔ `--vm` **cùng byte**.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-json-sort-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmd/json")
}

fn chay(d: &Path, tag: &str, src: &str, vm: bool, mock: Option<&Path>) -> (i32, String, String) {
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

fn chay_vis(d: &Path, tag: &str, src: &str) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vis"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&p)
        .output()
        .expect("chạy");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn mock_fixture() -> PathBuf {
    fixture_dir().join("mock.json")
}

const PIC: &str = "[{pid: 7, name: \"x\"}, {pid: 3, name: \"y\"}]";

#[test]
fn fixture_pipeline_dai_to_json() {
    // **Ca bắt buộc (Agent 0)**: `( ls ) | where | select | to json` với `--fs-mock`,
    // chạy **cả hai đường** cùng byte.
    let f = fixture_dir().join("json-pipe.vish");
    let m = mock_fixture();
    let mut outs = Vec::new();
    for vm in [false, true] {
        let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
        c.arg("run");
        if vm {
            c.arg("--vm");
        }
        c.arg("--fs-mock").arg(&m).arg(&f);
        let o = c.output().expect("chạy fixture");
        assert_eq!(
            o.status.code().unwrap_or(-1),
            0,
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
        outs.push(o.stdout);
    }
    assert_eq!(outs[0], outs[1], "interp và --vm phải cùng byte");
    assert_eq!(
        String::from_utf8_lossy(&outs[0]),
        "[{\"name\":\"b.jpg\",\"size\":6000000},{\"name\":\"c.jpg\",\"size\":8000000}]\n"
    );
}

#[test]
fn fixture_to_json_pretty() {
    let f = fixture_dir().join("json-pretty.vish");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&f)
        .output()
        .expect("chạy");
    assert_eq!(
        o.status.code().unwrap_or(-1),
        0,
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&o.stdout), "[\n  3,\n  1\n]\n");
}

#[test]
fn select_ban_ghi_an_danh_ra_json_dung_khoa_thu_tu() {
    let d = tam("select");
    let src = format!("( {PIC} ) | select pid, name | to json\n");
    let (c, o, e) = chay(&d, "l", &src, false, None);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "[{\"pid\":7,\"name\":\"x\"},{\"pid\":3,\"name\":\"y\"}]\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn literal_ban_ghi_an_danh_ra_object() {
    let d = tam("obj");
    let (c, o, e) = chay(
        &d,
        "l",
        "( {pid: 1, name: \"a\"} ) | to json\n",
        false,
        None,
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "{\"pid\":1,\"name\":\"a\"}\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn to_json_khong_can_use_std_json() {
    // Không hề có `use std.json` ⇒ shell sugar tự phân giải `std.json.encode`.
    let d = tam("use");
    let (c, o, e) = chay(&d, "l", "( [1, 2] ) | to json\n", false, None);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1,2]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn to_json_thieu_pipe_bao_vs0602_ro() {
    let d = tam("nopipe");
    let (c, _, e) = chay(&d, "l", "to json\n", false, None);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0602"), "stderr: {e}");
    assert!(e.contains("cần giá trị từ pipe"), "thông điệp rõ: {e}");
    assert!(e.contains("to json"), "nêu tên: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn to_json_co_khong_hop_le_bao_vs0602() {
    let d = tam("flag");
    let (c, _, e) = chay(&d, "l", "( [1] ) | to json --prettyy\n", false, None);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0602"), "stderr: {e}");
    assert!(e.contains("--prettyy"), "nêu cờ: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fixture_sort_shell_khoa_str_desc() {
    let f = fixture_dir().join("sort-shell.vish");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&f)
        .output()
        .expect("chạy");
    assert_eq!(
        o.status.code().unwrap_or(-1),
        0,
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let s = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(s.find("b").unwrap() < s.find("a").unwrap(), "desc: {s}");
}

#[test]
fn sort_by_khoa_size_va_duration_on_dinh() {
    let d = tam("size");
    let src = "( [ {size: 6mb, ten: \"a\"}, {size: 2mb, ten: \"b\"}, {size: 6mb, ten: \"c\"} ] ) | sort_by size\n";
    let (c, o, e) = chay(&d, "l", src, false, None);
    assert_eq!(c, 0, "stderr: {e}");
    let b = o.find("b").expect("b");
    let a = o.find("a").expect("a");
    let cc = o.find("c").expect("c");
    assert!(
        b < a && a < cc,
        "asc, ổn định (a trước c vì khoá bằng): {o}"
    );
    let src2 = "( [ {size: 6mb, ten: \"a\"}, {size: 2mb, ten: \"b\"}, {size: 6mb, ten: \"c\"} ] ) | sort_by size --desc\n";
    let (c2, o2, e2) = chay(&d, "l2", src2, false, None);
    assert_eq!(c2, 0, "stderr: {e2}");
    let a2 = o2.find("a").expect("a");
    let c2pos = o2.find("c").expect("c");
    assert!(a2 < c2pos, "desc vẫn ổn định: {o2}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn sort_by_khoa_bool_la_vs0412() {
    let d = tam("bool");
    let src = "( [ {n: 1}, {n: 2} ] ) | sort_by ( x => x.n > 1 )\n";
    let (c, _, e) = chay(&d, "l", src, false, None);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(e.contains("bool"), "nêu kiểu: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn std_list_sort_by_cu_van_chay() {
    let d = tam("std");
    let src = "use std.list\nprint(list.sort_by([3, 1, 2], x => 0 - x))\n";
    let (c, o, e) = chay_vis(&d, "l", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[3, 2, 1]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn round_trip_encode_decode_map_list_int_str() {
    let d = tam("rt");
    for (literal, want) in [
        ("{a: 1, b: 2}", "{\"a\":1,\"b\":2}"),
        ("[1, 2, 3]", "[1,2,3]"),
        ("42", "42"),
        ("\"xin chào\"", "\"xin chào\""),
    ] {
        let src = format!(
            "use std.json\nfn main() -> ():\n    let s = json.encode({literal})\n    print(s)\n    match json.decode(s):\n        ok(v):\n            print(json.encode(v))\n        err(e):\n            print(\"E\")\n            print(e)\n"
        );
        let (c, o, e) = chay_vis(&d, "rt", &src);
        assert_eq!(c, 0, "stderr: {e}");
        assert_eq!(o, format!("{want}\n{want}\n"), "round-trip {literal}: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn json_encode_list_ban_ghi_co_ten_va_an_danh() {
    let d = tam("enc");
    let src = "use std.json\ntype P:\n  pid: int\n  name: str\nlet p: P = {pid: 1, name: \"a\"}\nlet q = {pid: 2, name: \"b\"}\nprint(json.encode(p))\nprint(json.encode([p, q]))\n";
    let (c, o, e) = chay_vis(&d, "l", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o,
        "{\"pid\":1,\"name\":\"a\"}\n[{\"pid\":1,\"name\":\"a\"},{\"pid\":2,\"name\":\"b\"}]\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn interp_va_vm_cung_byte_cho_sort_va_json() {
    let d = tam("vm");
    for src in [
        "( [ {ten: \"b\", n: 1}, {ten: \"a\", n: 2} ] ) | sort_by ten --desc\n",
        "( [ {pid: 1, name: \"a\"} ] ) | select pid, name | to json --pretty\n",
    ] {
        let (c1, o1, e1) = chay(&d, "i", src, false, None);
        let (c2, o2, e2) = chay(&d, "v", src, true, None);
        assert_eq!(c1, 0, "{e1}");
        assert_eq!(c2, 0, "{e2}");
        assert_eq!(o1.as_bytes(), o2.as_bytes(), "khác byte cho {src}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
