// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-26 — **`std.*` lõi**: 68 ca `test/std/**` qua binary + lỗi module + differential interp ↔ VM.
//!
//! Kỳ vọng `.out` viết **theo thiết kế** (không chép từ hiện thực); `std` phải chạy **cùng** kết quả
//! trên cả hai đường (VD-55).

use std::path::PathBuf;
use std::process::Command;

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

fn vis_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vis")
}

struct Kq {
    code: i32,
    out: String,
    err: String,
}

fn chay(args: &[&str]) -> Kq {
    let o = Command::new(vis_bin())
        .args(args)
        .output()
        .expect("chạy được `vis`");
    Kq {
        code: o.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&o.stdout).to_string(),
        err: String::from_utf8_lossy(&o.stderr).to_string(),
    }
}

/// Bỏ dòng SPDX đầu của tệp kỳ vọng (QD §9.3).
fn bo_spdx(s: &str) -> String {
    match s.split_once('\n') {
        Some((d, rest)) if d.trim_start().starts_with('#') && d.contains("SPDX") => {
            rest.to_string()
        }
        _ => s.to_string(),
    }
}

fn ds_ca() -> Vec<PathBuf> {
    let dir = goc().join("test/std");
    let mut ds: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("đọc test/std")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "vis").unwrap_or(false))
        .collect();
    ds.sort();
    ds
}

#[test]
fn std_70_ca_qua_binary() {
    let ds = ds_ca();
    assert_eq!(ds.len(), 70, "phải đủ 70 ca `test/std`");
    let mut sai = Vec::new();
    for p in &ds {
        let k = chay(&["run", p.to_str().unwrap()]);
        let want = std::fs::read_to_string(p.with_extension("out"))
            .unwrap_or_else(|e| panic!("thiếu .out cho {}: {e}", p.display()));
        let want = bo_spdx(&want);
        if k.code != 0 || k.out != want {
            sai.push(format!(
                "{}: muốn exit 0 + {:?}, được exit {} + {:?}\n  stderr: {}",
                p.file_name().unwrap().to_string_lossy(),
                want,
                k.code,
                k.out,
                k.err
            ));
        }
    }
    assert!(
        sai.is_empty(),
        "{}/{} ca sai:\n{}",
        sai.len(),
        ds.len(),
        sai.join("\n")
    );
}

/// Differential: **mọi** ca `std` phải cho **cùng** `stdout` + exit trên interp và VM (VD-55).
#[test]
fn std_differential_interp_va_vm() {
    let cache = std::env::temp_dir().join(format!("vis-std-vm-{}", std::process::id()));
    let env_cache = cache.to_string_lossy().to_string();
    let mut sai = Vec::new();
    for p in ds_ca() {
        let a = chay(&["run", p.to_str().unwrap()]);
        let b = Command::new(vis_bin())
            .args(["run", "--vm", p.to_str().unwrap()])
            .env("XDG_CACHE_HOME", &env_cache)
            .output()
            .expect("chạy `vis run --vm`");
        let bout = String::from_utf8_lossy(&b.stdout).to_string();
        let berr = String::from_utf8_lossy(&b.stderr).to_string();
        let bcode = b.status.code().unwrap_or(-1);
        if a.out != bout || a.code != bcode {
            sai.push(format!(
                "{}: interp ({}, {:?}) ≠ vm ({}, {:?})\n  vm stderr: {}",
                p.file_name().unwrap().to_string_lossy(),
                a.code,
                a.out,
                bcode,
                bout,
                berr
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&cache);
    assert!(sai.is_empty(), "{} ca lệch:\n{}", sai.len(), sai.join("\n"));
}

#[test]
fn std_fs_bao_st27() {
    let tmp = std::env::temp_dir().join(format!("vis-std-fs-{}.vis", std::process::id()));
    std::fs::write(&tmp, "use std.fs\nprint(1)\n").expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0701"), "stderr: {}", k.err);
    assert!(k.err.contains("ST-27"), "phải nêu ST-27: {}", k.err);
}

#[test]
fn std_la_bao_vs0701() {
    let tmp = std::env::temp_dir().join(format!("vis-std-la-{}.vis", std::process::id()));
    std::fs::write(&tmp, "use std.khong_co\nprint(1)\n").expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0701"), "stderr: {}", k.err);
}

/// `json.decode` **đã biết nhưng chưa có** ⇒ `VS0301` nói rõ "chưa hỗ trợ (VS1b)".
#[test]
fn json_decode_bao_chua_ho_tro() {
    let tmp = std::env::temp_dir().join(format!("vis-std-json-{}.vis", std::process::id()));
    std::fs::write(&tmp, "use std.json\nprint(json.decode(\"1\"))\n").expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 2, "stderr: {}", k.err);
    assert!(k.err.contains("VS0301"), "stderr: {}", k.err);
    assert!(k.err.contains("chưa hỗ trợ (VS1b)"), "stderr: {}", k.err);
}

/// Sai kiểu **lúc chạy** (tham số `Any`) ⇒ `VS0501` nêu hàm + kiểu nhận được.
#[test]
fn sai_kieu_luc_chay_la_vs0501() {
    let tmp = std::env::temp_dir().join(format!("vis-std-sai-{}.vis", std::process::id()));
    std::fs::write(
        &tmp,
        "use std.list\nprint(list.sort(\"khong phai list\"))\n",
    )
    .expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 1, "stderr: {}", k.err);
    assert!(k.err.contains("VS0501"), "stderr: {}", k.err);
    assert!(k.err.contains("list.sort"), "phải nêu hàm: {}", k.err);
}

/// `tyck` kiểm **số đối số** của `std.*` (VD-43).
#[test]
fn sai_so_doi_so_bi_tyck_bat() {
    let tmp = std::env::temp_dir().join(format!("vis-std-so-{}.vis", std::process::id()));
    std::fs::write(&tmp, "use std.str\nprint(str.upper())\n").expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 2, "stderr: {}", k.err);
    assert!(k.err.contains("VS0401"), "stderr: {}", k.err);
}

/// Không `use` ⇒ **không** gọi được `str.upper` (không rò rỉ phạm vi): tệp đơn lẻ theo VD-45
/// (`Permissive`) ⇒ `VS0501` lúc chạy, **không** im lặng chạy đúng.
#[test]
fn khong_use_thi_khong_goi_duoc_std() {
    let tmp = std::env::temp_dir().join(format!("vis-std-nouse-{}.vis", std::process::id()));
    std::fs::write(&tmp, "print(str.upper(\"a\"))\n").expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 1, "stderr: {}", k.err);
    assert!(k.err.contains("VS0501"), "stderr: {}", k.err);
}

/// `str.repeat` với số lần **âm** ⇒ `VS0501` (ngữ nghĩa kiểm lúc chạy).
#[test]
fn str_repeat_am_la_vs0501() {
    let tmp = std::env::temp_dir().join(format!("vis-std-rep-{}.vis", std::process::id()));
    std::fs::write(&tmp, "use std.str\nprint(str.repeat(\"ab\", 0 - 1))\n").expect("ghi");
    let k = chay(&["run", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(k.code, 1, "stderr: {}", k.err);
    assert!(k.err.contains("VS0501"), "stderr: {}", k.err);
    assert!(k.err.contains("str.repeat"), "phải nêu hàm: {}", k.err);
}
