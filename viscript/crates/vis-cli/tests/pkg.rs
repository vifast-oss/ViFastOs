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

// ===== ST-71 (VD-101 (5)): `use pkg.*` + kiểm sha lúc chạy =====

/// Cài `http@ver` từ fixture vào `d` (cwd = `d`).
fn cai_http(d: &Path, ver: &str) {
    let reg = fixture();
    let (code, _o, err) = vis(
        d,
        &[
            "pkg",
            "add",
            &format!("http@{ver}"),
            "--registry",
            reg.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "cài http@{ver}: {err}");
}

/// Ghi một gói tối giản vào registry tạm (nội dung đã chuẩn hoá — một LF cuối).
fn ghi_goi_vong(reg: &Path, ten: &str, dep: &str) {
    let src = format!(
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n\nuse pkg.{dep}\n\npub fn f{ten}() -> int:\n    1\n"
    );
    let sha = hex(&vis_bytecode::source_hash(&src));
    let goc = reg.join(ten).join("1.0.0");
    ghi(&goc.join("src/main.vis"), &src);
    let man = format!(
        r#"{{"name":"{ten}","version":"1.0.0","entry":"src/main.vis","deps":{{"{dep}":"1.0.0"}},"files":[{{"path":"src/main.vis","sha256":"{sha}"}}]}}"#
    );
    ghi(&goc.join("manifest.json"), &man);
}

/// Ca 7 (CLI) — `use pkg.http` chạy thật; interp và VM (`--no-cache`) cùng byte.
#[test]
fn use_pkg_chay_that() {
    let d = tmp_dir("use-chay");
    cai_http(&d, "1.0.0");
    ghi(
        &d.join("app.vis"),
        "use pkg.http\n\nprint(http_version())\n",
    );
    for args in [
        vec!["run", "app.vis"],
        vec!["run", "--vm", "--no-cache", "app.vis"],
    ] {
        let (code, out, err) = vis(&d, &args);
        assert_eq!(code, 0, "{args:?} stderr: {err}");
        assert_eq!(out, "100\n", "{args:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 8 (CLI) — sửa tay tệp trong `.vis/pkg` ⇒ `VS0701`, exit 2, không in kết quả.
#[test]
fn pkg_bi_sua_bao_vs0701() {
    let d = tmp_dir("use-sua");
    cai_http(&d, "1.0.0");
    ghi(
        &d.join("app.vis"),
        "use pkg.http\n\nprint(http_version())\n",
    );
    let tep = d.join(".vis/pkg/http/1.0.0/src/main.vis");
    let cu = doc(&tep);
    std::fs::write(&tep, format!("{cu}\n# sửa tay\n")).expect("sửa tệp gói");
    let (code, out, err) = vis(&d, &["run", "app.vis"]);
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("VS0701"), "stderr: {err}");
    assert!(err.contains("bị sửa"), "phải nói gói bị sửa: {err}");
    assert_eq!(out, "", "không được in kết quả");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 9 (CLI) — chưa cài gói ⇒ lỗi rõ **nêu tên** + gợi ý `vis pkg add`; cả khi lock có gói khác.
#[test]
fn pkg_chua_cai() {
    let d = tmp_dir("use-chua-cai");
    ghi(&d.join("app.vis"), "use pkg.http\n\nprint(1)\n");
    let (code, _out, err) = vis(&d, &["run", "app.vis"]);
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("chưa cài gói `http`"), "stderr: {err}");
    assert!(err.contains("vis pkg add http"), "stderr: {err}");
    // Lock **có** gói khác nhưng thiếu `http` ⇒ cùng thông điệp.
    let reg = fixture();
    let (c0, _o0, e0) = vis(
        &d,
        &["pkg", "add", "util", "--registry", reg.to_str().unwrap()],
    );
    assert_eq!(c0, 0, "cài util: {e0}");
    let (code2, _o2, err2) = vis(&d, &["run", "app.vis"]);
    assert_eq!(code2, 2, "stderr: {err2}");
    assert!(err2.contains("chưa cài gói `http`"), "stderr: {err2}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 10 (CLI) — hai gói cùng `src/main.vis` ⇒ `rel` khác nhau (`pkg/<tên>/<ver>/…`), `--dump-deps`
/// in cả hai, chạy đúng.
#[test]
fn hai_goi_cung_ten_tep() {
    let d = tmp_dir("use-hai-goi");
    cai_http(&d, "1.10.0");
    let (c, _o, e) = vis(
        &d,
        &[
            "pkg",
            "add",
            "util",
            "--registry",
            fixture().to_str().unwrap(),
        ],
    );
    assert_eq!(c, 0, "cài util: {e}");
    ghi(
        &d.join("app.vis"),
        "use pkg.http\nuse pkg.util\n\nprint(http_version() + util_version())\n",
    );
    let (code, out, err) = vis(&d, &["run", "--dump-deps", "app.vis"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "1101\n");
    assert!(
        err.contains("pkg/http/1.10.0/src/main.vis"),
        "dump-deps thiếu rel http: {err}"
    );
    assert!(
        err.contains("pkg/util/0.1.0/src/main.vis"),
        "dump-deps thiếu rel util: {err}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 11 (CLI) — vòng qua gói A↔B ⇒ `VS0702`, không treo.
#[test]
fn pkg_vong_lap() {
    let d = tmp_dir("use-vong");
    let reg = d.join("registry");
    ghi_goi_vong(&reg, "a", "b");
    ghi_goi_vong(&reg, "b", "a");
    let (code, _o, err) = vis(
        &d,
        &["pkg", "add", "a@1.0.0", "--registry", reg.to_str().unwrap()],
    );
    assert_eq!(code, 0, "cài a: {err}");
    ghi(&d.join("app.vis"), "use pkg.a\n\nprint(fa())\n");
    let (code2, _out, err2) = vis(&d, &["run", "app.vis"]);
    assert_eq!(code2, 2, "stderr: {err2}");
    assert!(err2.contains("VS0702"), "stderr: {err2}");
    assert!(err2.contains("vòng phụ thuộc"), "stderr: {err2}");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------
// **ST-90 (VD-110)** — `entry` trong `vis.lock` + `vis pkg remove`
// ---------------------------------------------------------------------------

fn add_fixture(d: &Path, spec: &str) -> (i32, String, String) {
    vis(
        d,
        &[
            "pkg",
            "add",
            spec,
            "--registry",
            fixture().to_str().unwrap(),
        ],
    )
}

fn chuong_trinh_use(d: &Path, ten: &str) -> PathBuf {
    let p = d.join("main.vis");
    ghi(&p, &format!("use pkg.{ten}\nprint(1)\n"));
    p
}

/// (a) cài rồi ⇒ `vis.lock` có `entry` đúng đường dẫn trong gói.
#[test]
fn lock_co_entry_sau_khi_cai() {
    let d = tmp_dir("entry-co");
    let (c, _o, e) = add_fixture(&d, "http@1.0.0");
    assert_eq!(c, 0, "stderr: {e}");
    let lock = doc(&d.join("vis.lock"));
    assert!(lock.contains("\"entry\":\"src/main.vis\""), "lock: {lock}");
    assert_eq!(lock.matches("\"entry\"").count(), 2, "http + util: {lock}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) lock có `entry` **sai** ⇒ `VS0701` nêu **tên tệp**, không chạy.
#[test]
fn entry_sai_bao_vs0701_neu_ten() {
    let d = tmp_dir("entry-sai");
    let (c, _o, e) = add_fixture(&d, "http@1.0.0");
    assert_eq!(c, 0, "stderr: {e}");
    let p = d.join("vis.lock");
    let lock = doc(&p).replace("\"entry\":\"src/main.vis\"", "\"entry\":\"src/khong.vis\"");
    ghi(&p, &lock);
    let f = chuong_trinh_use(&d, "http");
    let (c2, o2, e2) = vis(&d, &["check", f.to_str().unwrap()]);
    assert_eq!(c2, 1, "stderr: {e2}");
    assert!(
        o2.contains("VS0701") && o2.contains("src/khong.vis"),
        "phải nêu entry sai: {o2}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) lock **cũ** (không `entry`) ⇒ fallback quy ước, vẫn chạy.
#[test]
fn lock_cu_khong_entry_van_chay() {
    let d = tmp_dir("entry-cu");
    let (c, _o, e) = add_fixture(&d, "http@1.0.0");
    assert_eq!(c, 0, "stderr: {e}");
    let p = d.join("vis.lock");
    let lock = doc(&p).replace("\"entry\":\"src/main.vis\",", "");
    ghi(&p, &lock);
    assert!(!lock.contains("\"entry\""), "lock cũ: {lock}");
    let f = chuong_trinh_use(&d, "http");
    let (c2, _o2, e2) = vis(&d, &["check", f.to_str().unwrap()]);
    assert_eq!(c2, 0, "fallback quy ước phải chạy: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `pkg remove` xoá lock + `.vis/pkg` + blob **không còn dùng**; chạy lại `use pkg.x` ⇒ `VS0701`.
#[test]
fn remove_xoa_lock_store_va_bao_chua_cai() {
    let d = tmp_dir("remove");
    let (c, _o, e) = add_fixture(&d, "http@1.0.0");
    assert_eq!(c, 0, "stderr: {e}");
    let lock = doc(&d.join("vis.lock"));
    let http_sha = {
        // sha của tệp http trong lock
        let i = lock.find("\"name\":\"http\"").expect("http");
        let j = lock[i..].find("\"files\"").expect("files") + i;
        let k = lock[j..].find("\"sha256\":\"").expect("sha") + j + 10;
        lock[k..k + 64].to_string()
    };
    assert!(d.join(".vis/store").join(&http_sha).is_file());
    let (c2, _o2, e2) = vis(&d, &["pkg", "remove", "http"]);
    assert_eq!(c2, 0, "stderr: {e2}");
    let lock2 = doc(&d.join("vis.lock"));
    assert!(!lock2.contains("\"name\":\"http\""), "lock: {lock2}");
    assert!(!d.join(".vis/pkg/http").exists());
    assert!(
        !d.join(".vis/store").join(&http_sha).exists(),
        "blob http phải xoá"
    );
    let f = chuong_trinh_use(&d, "http");
    let (c3, o3, e3) = vis(&d, &["check", f.to_str().unwrap()]);
    assert_eq!(c3, 1, "{e3}");
    assert!(o3.contains("chưa cài gói `http`"), "{o3}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) tên lạ ⇒ exit **2**, lock **không đổi**.
#[test]
fn remove_ten_la_exit_2_lock_khong_doi() {
    let d = tmp_dir("remove-la");
    let (c, _o, e) = add_fixture(&d, "http@1.0.0");
    assert_eq!(c, 0, "stderr: {e}");
    let truoc = doc(&d.join("vis.lock"));
    let (c2, _o2, e2) = vis(&d, &["pkg", "remove", "khong_co_goi_nay"]);
    assert_eq!(c2, 2, "stderr: {e2}");
    assert!(e2.contains("chưa cài gói"), "{e2}");
    assert_eq!(doc(&d.join("vis.lock")), truoc, "lock không được đổi");
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) hai gói **cùng nội dung** (cùng blob store): remove một ⇒ blob còn, gói kia vẫn chạy.
#[test]
fn remove_khong_xoa_blob_con_dung() {
    let d = tmp_dir("remove-chung");
    let reg = d.join("registry");
    let noi_dung = "print(\"chung\")\n";
    let sha = hex(&vis_bytecode::source_hash(noi_dung));
    for ten in ["alpha", "beta"] {
        ghi(
            &reg.join(ten).join("1.0.0").join("manifest.json"),
            &format!(
                r#"{{"name":"{ten}","version":"1.0.0","entry":"src/main.vis","files":[{{"path":"src/main.vis","sha256":"{sha}"}}]}}"#
            ),
        );
        ghi(&reg.join(ten).join("1.0.0").join("src/main.vis"), noi_dung);
    }
    for ten in ["alpha", "beta"] {
        let (c, _o, e) = vis(
            &d,
            &["pkg", "add", ten, "--registry", reg.to_str().unwrap()],
        );
        assert_eq!(c, 0, "add {ten}: {e}");
    }
    assert_eq!(
        std::fs::read_dir(d.join(".vis/store")).unwrap().count(),
        1,
        "hai gói cùng nội dung ⇒ một blob"
    );
    let (c, _o, e) = vis(&d, &["pkg", "remove", "alpha"]);
    assert_eq!(c, 0, "remove: {e}");
    assert!(
        d.join(".vis/store").join(&sha).is_file(),
        "blob còn dùng bởi beta — không được xoá"
    );
    let f = chuong_trinh_use(&d, "beta");
    let (c2, _o2, e2) = vis(&d, &["check", f.to_str().unwrap()]);
    assert_eq!(c2, 0, "beta phải còn chạy: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (g) manifest **không** có `entry` ⇒ `--entry` chỉ định; không có `--entry` ⇒ lỗi nêu `entry`.
#[test]
fn entry_tuy_chon_khi_manifest_thieu() {
    let d = tmp_dir("entry-tuy-chon");
    let reg = d.join("registry");
    let noi_dung = "print(\"noent\")\n";
    let sha = hex(&vis_bytecode::source_hash(noi_dung));
    ghi(
        &reg.join("noent").join("1.0.0").join("manifest.json"),
        &format!(
            r#"{{"name":"noent","version":"1.0.0","files":[{{"path":"src/main.vis","sha256":"{sha}"}}]}}"#
        ),
    );
    ghi(
        &reg.join("noent").join("1.0.0").join("src/main.vis"),
        noi_dung,
    );
    let (c0, _o0, e0) = vis(
        &d,
        &["pkg", "add", "noent", "--registry", reg.to_str().unwrap()],
    );
    assert_eq!(c0, 2, "thiếu entry phải lỗi: {e0}");
    assert!(e0.contains("entry"), "{e0}");
    let (c, _o, e) = vis(
        &d,
        &[
            "pkg",
            "add",
            "noent",
            "--entry",
            "src/main.vis",
            "--registry",
            reg.to_str().unwrap(),
        ],
    );
    assert_eq!(c, 0, "có --entry phải cài được: {e}");
    let lock = doc(&d.join("vis.lock"));
    assert!(lock.contains("\"entry\":\"src/main.vis\""), "lock: {lock}");
    let _ = std::fs::remove_dir_all(&d);
}
