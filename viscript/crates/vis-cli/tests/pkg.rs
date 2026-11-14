// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-70 (VD-101)** — `vis pkg` với **registry local**: manifest tường minh + `vis.lock` +
//! `.vis/store` content-addressed; cài 2 lần **cùng byte**; `install` không cần registry.
//! Hermetic: mỗi test một thư mục tạm **duy nhất** (test chạy song song).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-pkg-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

/// Registry mẫu trong repo (chỉ **đọc**).
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/pkg/registry")
}

fn vis(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("chạy `vis pkg`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn ghi(p: &Path, s: &str) {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).expect("tạo thư mục");
    }
    std::fs::write(p, s).expect("ghi tệp");
}

fn doc(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("đọc {}: {e}", p.display()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::new();
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn sao_chep(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let p = e.path();
        let q = dst.join(e.file_name());
        if p.is_dir() {
            sao_chep(&p, &q);
        } else {
            std::fs::copy(&p, &q).expect("copy");
        }
    }
}

/// Băm **toàn cây** (path tương đối + băm tệp) — đủ để so hai lần cài.
fn cay_hash(dir: &Path) -> String {
    fn walk(base: &Path, d: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(d).expect("read_dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .expect("rel")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = doc(&p);
                out.push((rel, hex(&vis_bytecode::source_hash(&text))));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    let mut s = String::from("pkg1\n");
    for (p, h) in out {
        s.push_str(&p);
        s.push('\n');
        s.push_str(&h);
        s.push('\n');
    }
    hex(&vis_bytecode::source_hash(&s))
}

/// Ca 1 — manifest thiếu `entry`/`files` ⇒ lỗi nêu **tên trường**, exit 2, **không** tạo `vis.lock`.
#[test]
fn manifest_thieu_truong() {
    let d = tmp_dir("manifest");
    let reg = d.join("registry");
    ghi(
        &reg.join("bad/1.0.0/manifest.json"),
        r#"{"name":"bad","version":"1.0.0","files":[]}"#,
    );
    let (code, _out, err) = vis(
        &d,
        &["pkg", "add", "bad", "--registry", reg.to_str().unwrap()],
    );
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("entry"), "phải nêu trường thiếu: {err}");
    assert!(!d.join("vis.lock").exists(), "KHÔNG được ghi vis.lock");
    assert!(!d.join(".vis").exists(), "KHÔNG được tạo .vis");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 2 — `add` (không `@ver`) chọn **cao nhất theo số** (`1.10.0 > 1.9.0`); `@1.0.0` chọn đúng;
/// `list` in `tên ver sha[:12]` sắp theo tên.
#[test]
fn add_roi_list() {
    let d = tmp_dir("list");
    let (code, _out, err) = vis(
        &d,
        &[
            "pkg",
            "add",
            "http",
            "--registry",
            fixture().to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        d.join(".vis/pkg/http/1.10.0/src/main.vis").is_file(),
        "phải chọn 1.10.0 (số), không phải 1.9.0"
    );
    assert!(
        d.join(".vis/pkg/util/0.1.0/src/main.vis").is_file(),
        "dep util"
    );
    let lock = doc(&d.join("vis.lock"));
    assert!(lock.contains("\"name\":\"http\""), "lock: {lock}");
    let (code2, out, err2) = vis(&d, &["pkg", "list"]);
    assert_eq!(code2, 0, "stderr: {err2}");
    let dong: Vec<&str> = out.lines().collect();
    assert_eq!(dong.len(), 2, "list: {out:?}");
    assert!(dong[0].starts_with("http 1.10.0 "), "dòng 1: {:?}", dong[0]);
    assert!(dong[1].starts_with("util 0.1.0 "), "dòng 2: {:?}", dong[1]);
    assert_eq!(dong[0].split(' ').nth(2).unwrap().len(), 12, "sha[:12]");
    // `@1.0.0` chọn đúng version ghi rõ.
    let (code3, _o, e3) = vis(
        &d,
        &[
            "pkg",
            "add",
            "http@1.0.0",
            "--registry",
            fixture().to_str().unwrap(),
        ],
    );
    assert_eq!(code3, 0, "stderr: {e3}");
    assert!(d.join(".vis/pkg/http/1.0.0/src/main.vis").is_file());
    let (_, out2, _) = vis(&d, &["pkg", "list"]);
    assert!(
        out2.lines().any(|l| l.starts_with("http 1.0.0 ")),
        "list: {out2:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 3 — `http` khai `util` ⇒ **cả hai** vào lock + được cài.
#[test]
fn add_keo_phu_thuoc() {
    let d = tmp_dir("deps");
    let reg = d.join("registry");
    sao_chep(&fixture(), &reg);
    let (code, _o, err) = vis(
        &d,
        &[
            "pkg",
            "add",
            "http@1.0.0",
            "--registry",
            reg.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "stderr: {err}");
    assert!(d.join(".vis/pkg/http/1.0.0/src/main.vis").is_file());
    assert!(d.join(".vis/pkg/util/0.1.0/src/main.vis").is_file());
    let lock = doc(&d.join("vis.lock"));
    assert!(lock.contains("\"name\":\"http\""), "lock: {lock}");
    assert!(lock.contains("\"name\":\"util\""), "lock: {lock}");
    let (_, out, _) = vis(&d, &["pkg", "list"]);
    assert_eq!(out.lines().count(), 2, "list: {out:?}");
    assert!(
        doc(&d.join(".vis/pkg/util/0.1.0/src/main.vis")).contains("util_version"),
        "nội dung util"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 4 — xoá **registry**, `vis pkg install` vẫn dựng lại đủ cây (chỉ lock + `.vis/store`).
#[test]
fn install_khong_can_registry() {
    let d = tmp_dir("install");
    let reg = d.join("registry");
    sao_chep(&fixture(), &reg);
    let (code, _o, err) = vis(
        &d,
        &[
            "pkg",
            "add",
            "http@1.0.0",
            "--registry",
            reg.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "stderr: {err}");
    std::fs::remove_dir_all(&reg).expect("xoá registry");
    std::fs::remove_dir_all(d.join(".vis/pkg")).expect("xoá pkg");
    let (code2, _o2, err2) = vis(&d, &["pkg", "install"]);
    assert_eq!(code2, 0, "install phải chạy không cần registry: {err2}");
    assert!(d.join(".vis/pkg/http/1.0.0/src/main.vis").is_file());
    assert!(d.join(".vis/pkg/util/0.1.0/src/main.vis").is_file());
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 5 — e2e: `add` → băm toàn cây → `rm -rf .vis/pkg` → `install` ⇒ băm **bằng nhau**,
/// `vis.lock` **không đổi**, lock **không** chứa đường dẫn tạm.
#[test]
fn cai_hai_lan_cung_byte() {
    let d = tmp_dir("hai-lan");
    let reg = d.join("registry");
    sao_chep(&fixture(), &reg);
    let (code, _o, err) = vis(
        &d,
        &[
            "pkg",
            "add",
            "http@1.0.0",
            "--registry",
            reg.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "stderr: {err}");
    let h1 = cay_hash(&d.join(".vis/pkg"));
    let lock1 = doc(&d.join("vis.lock"));
    std::fs::remove_dir_all(d.join(".vis/pkg")).expect("xoá pkg");
    let (code2, _o2, err2) = vis(&d, &["pkg", "install"]);
    assert_eq!(code2, 0, "install: {err2}");
    let h2 = cay_hash(&d.join(".vis/pkg"));
    assert_eq!(h1, h2, "hai lần cài phải cùng byte");
    assert_eq!(lock1, doc(&d.join("vis.lock")), "vis.lock không được đổi");
    assert!(
        !lock1.contains(d.to_str().unwrap()),
        "lock không được chứa đường dẫn tuyệt đối"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 6 — `vis.lock` JSON hỏng ⇒ exit 2, `.vis/pkg` **giữ nguyên** (không ghi nửa vời).
#[test]
fn lock_hong() {
    let d = tmp_dir("lock-hong");
    let reg = d.join("registry");
    sao_chep(&fixture(), &reg);
    let (code, _o, err) = vis(
        &d,
        &[
            "pkg",
            "add",
            "http@1.0.0",
            "--registry",
            reg.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "stderr: {err}");
    let truoc = cay_hash(&d.join(".vis/pkg"));
    std::fs::write(d.join("vis.lock"), "{ hỏng").expect("ghi lock hỏng");
    let (code2, _o2, err2) = vis(&d, &["pkg", "install"]);
    assert_eq!(code2, 2, "stderr: {err2}");
    assert!(err2.contains("vis.lock"), "phải nêu `vis.lock`: {err2}");
    assert_eq!(
        truoc,
        cay_hash(&d.join(".vis/pkg")),
        "cây .vis/pkg phải giữ nguyên"
    );
    let _ = std::fs::remove_dir_all(&d);
}
