// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-25 — **module đa tệp**: `use ./util`, cấm vòng, `pub`, chẩn đoán đúng tên tệp, hash closure.
//!
//! Chạy **qua binary** `vis` (đỏ trước theo D2-05). Fixture: `test/modules/**`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

fn tep(rel: &str) -> String {
    goc()
        .join("test/modules")
        .join(rel)
        .to_string_lossy()
        .to_string()
}

struct Kq {
    code: i32,
    out: String,
    err: String,
}

fn chay(args: &[&str], extra_env: &[(&str, &str)]) -> Kq {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.args(args);
    for (k, v) in extra_env {
        c.env(k, v);
    }
    let o = c.output().expect("chạy được `vis`");
    Kq {
        code: o.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&o.stdout).to_string(),
        err: String::from_utf8_lossy(&o.stderr).to_string(),
    }
}

fn run(rel: &str) -> Kq {
    let p = tep(rel);
    chay(&["run", &p], &[])
}

#[test]
fn co_ban_pub_fn_va_pub_type() {
    let k = run("co-ban/main.vis");
    assert_eq!(k.code, 0, "stderr: {}", k.err);
    assert_eq!(k.out, "3\n");
}

#[test]
fn hai_tang_noi_dung_dung() {
    let k = run("hai-tang/main.vis");
    assert_eq!(k.code, 0, "stderr: {}", k.err);
    assert_eq!(k.out, "2\n");
}

#[test]
fn vm_va_interp_cung_ket_qua() {
    let p = tep("co-ban/main.vis");
    let a = chay(&["run", &p], &[]);
    let b = chay(
        &["run", "--vm", &p],
        &[("XDG_CACHE_HOME", "/tmp/vis-mod-test-cache")],
    );
    assert_eq!(a.out, b.out);
    assert_eq!(a.code, b.code);
}

#[test]
fn thieu_tep_la_vs0701() {
    let k = run("thieu/main.vis");
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0701"), "stderr: {}", k.err);
    assert!(
        k.err.contains("main.vis:2:1"),
        "phải nêu đúng vị trí: {}",
        k.err
    );
}

#[test]
fn std_fs_bao_st27() {
    // **SỬA KÈM LÝ DO (ST-26)**: `std.fs` cần host ⇒ dời sang **ST-27**; thông điệp nay là "ST-27"
    // (trước ST-26 nó là "ST-26" vì chưa có module std nào).
    let k = run("std/main.vis");
    assert_eq!(k.code, 2);
    assert!(
        k.err.contains("VS0701") && k.err.contains("ST-27"),
        "stderr: {}",
        k.err
    );
}

#[test]
fn pkg_bao_vs2() {
    let k = run("pkg/main.vis");
    assert_eq!(k.code, 2);
    assert!(
        k.err.contains("VS0701") && k.err.contains("VS2"),
        "stderr: {}",
        k.err
    );
}

#[test]
fn use_trung_hai_duong_dan_la_vs0703() {
    let k = run("trung/main.vis");
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0703"), "stderr: {}", k.err);
}

#[test]
fn vong_hai_tep_la_vs0702() {
    let k = run("vong-2/main.vis");
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0702"), "stderr: {}", k.err);
}

#[test]
fn vong_ba_tep_la_vs0702() {
    let k = run("vong-3/main.vis");
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0702"), "stderr: {}", k.err);
    assert!(k.err.contains("c.vis"), "phải nêu tệp đóng vòng: {}", k.err);
}

#[test]
fn loi_trong_tep_phu_bao_dung_ten_tep() {
    let k = run("loi-phu/main.vis");
    assert_eq!(k.code, 2);
    assert!(
        k.err.contains("util.vis:3:18"),
        "chẩn đoán phải trỏ tệp phụ + dòng:cột thật: {}",
        k.err
    );
    assert!(k.err.contains("VS0412"), "stderr: {}", k.err);
}

#[test]
fn khong_pub_thi_khong_thay() {
    let k = run("khong-pub/main.vis");
    assert_eq!(k.code, 2);
    assert!(k.err.contains("VS0301"), "stderr: {}", k.err);
    assert!(k.err.contains("bi_mat"), "phải nêu tên: {}", k.err);
    assert!(k.err.contains("util.vis"), "phải nêu module: {}", k.err);
}

#[test]
fn check_bao_dung_tep_va_dong_cot() {
    let p = tep("loi-phu/main.vis");
    let k = chay(&["check", &p], &[]);
    assert_eq!(k.code, 1, "stderr: {}", k.err);
    assert!(k.out.contains("not ok 1"), "TAP: {}", k.out);
    assert!(
        k.out.contains("util.vis:3:18"),
        "TAP phải nêu tệp phụ: {}",
        k.out
    );
}

#[test]
fn dump_deps_in_cay() {
    let p = tep("hai-tang/main.vis");
    let k = chay(&["run", "--dump-deps", &p], &[]);
    assert_eq!(k.code, 0, "stderr: {}", k.err);
    assert!(k.err.contains("main.vis -> a.vis"), "cây: {}", k.err);
    assert!(k.err.contains("a.vis -> b.vis"), "cây: {}", k.err);
}

#[test]
fn tep_chinh_khong_ton_tai() {
    let k = chay(&["run", "/tmp/khong-co-tep-nay.vis"], &[]);
    assert_eq!(k.code, 2);
    assert!(
        k.err.contains("không phải tệp") || k.err.contains("không đọc được"),
        "stderr: {}",
        k.err
    );
}

/// ST-25 mục 4: sửa **tệp phụ thuộc** ⇒ khoá cache **đổi** (chạy lại phải biên dịch lại, không trúng).
#[test]
fn cache_doi_khi_sua_phu_thuoc() {
    let tmp = std::env::temp_dir().join(format!("vis-mod-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    let main = tmp.join("main.vis");
    let util = tmp.join("util.vis");
    std::fs::write(&main, "use ./util\nprint(g())\n").expect("ghi main");
    std::fs::write(&util, "pub fn g() -> int:\n    10\n").expect("ghi util");
    let cache = tmp.join("cache");
    let env = [("XDG_CACHE_HOME", cache.to_str().unwrap())];
    let m = main.to_string_lossy().to_string();
    let a1 = chay(&["run", "--vm", &m], &env);
    assert_eq!(a1.out, "10\n", "stderr: {}", a1.err);
    // Lần 2 **trúng** cache (vẫn 10).
    let a2 = chay(&["run", "--vm", &m], &env);
    assert_eq!(a2.out, "10\n");
    // Sửa phụ thuộc ⇒ khoá đổi ⇒ phải thấy 11.
    std::fs::write(&util, "pub fn g() -> int:\n    11\n").expect("sửa util");
    let a3 = chay(&["run", "--vm", &m], &env);
    assert_eq!(
        a3.out, "11\n",
        "hash closure không gồm phụ thuộc? stderr: {}",
        a3.err
    );
    // Đếm tệp `.vbc`: phải có **2** (khoá cũ + khoá mới) ⇒ chứng minh khoá đổi.
    let mut n = 0;
    fn dem(p: &Path, n: &mut usize) {
        if let Ok(ds) = std::fs::read_dir(p) {
            for e in ds.flatten() {
                let q = e.path();
                if q.is_dir() {
                    dem(&q, n);
                } else if q.extension().map(|x| x == "vbc").unwrap_or(false) {
                    *n += 1;
                }
            }
        }
    }
    dem(&cache, &mut n);
    assert_eq!(
        n, 2,
        "phải có 2 khoá cache (trước/sau khi sửa phụ thuộc), thật {n}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Cache **không** được làm sai kết quả khi bytecode trên đĩa bị hỏng (luôn qua `load_verified`).
#[test]
fn cache_hong_thi_bien_dich_lai() {
    let tmp = std::env::temp_dir().join(format!("vis-mod-hong-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    let main = tmp.join("main.vis");
    std::fs::write(&main, "print(1 + 1)\n").expect("ghi");
    let cache = tmp.join("cache");
    let env = [("XDG_CACHE_HOME", cache.to_str().unwrap())];
    let m = main.to_string_lossy().to_string();
    assert_eq!(chay(&["run", "--vm", &m], &env).out, "2\n");
    // Phá mọi tệp `.vbc`.
    fn pha(p: &Path) {
        if let Ok(ds) = std::fs::read_dir(p) {
            for e in ds.flatten() {
                let q = e.path();
                if q.is_dir() {
                    pha(&q);
                } else if q.extension().map(|x| x == "vbc").unwrap_or(false) {
                    let _ = std::fs::write(&q, b"VBC1 rac");
                }
            }
        }
    }
    pha(&cache);
    let k = chay(&["run", "--vm", &m], &env);
    assert_eq!(k.code, 0, "phải biên dịch lại rồi chạy: {}", k.err);
    assert_eq!(k.out, "2\n");
    let _ = std::fs::remove_dir_all(&tmp);
}
