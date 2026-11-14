// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-87b (VD-109)** — fast-path cache một tệp là **dead code** (đọc khoá v1, ghi khoá v2) ⇒ **bỏ**
//! fast-path; đường duy nhất `khoa_multi` (hit thật, bỏ codegen). Khoá: (rel, text, mode, deps).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn thu_muc(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-fastpath-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn chay(cache: &Path, f: &Path, args: &[&str]) -> (i32, Vec<u8>, Vec<u8>, f64) {
    let t = Instant::now();
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(goc())
        .env("XDG_CACHE_HOME", cache)
        .arg("run")
        .args(args)
        .arg(f)
        .output()
        .expect("chạy vis");
    (
        o.status.code().unwrap_or(-1),
        o.stdout,
        o.stderr,
        t.elapsed().as_secs_f64(),
    )
}

fn dem_vbc(dir: &Path) -> usize {
    tim_vbc(dir).len()
}

fn tim_vbc(dir: &Path) -> Vec<PathBuf> {
    let mut ds = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return ds;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            ds.extend(tim_vbc(&p));
        } else if p.extension().and_then(|x| x.to_str()) == Some("vbc") {
            ds.push(p);
        }
    }
    ds
}

/// **ST-87b** — không còn nhánh chết: `main.rs` **không** đọc khoá v1, `cache.rs` **không** còn
/// `khoa` v1; chỉ còn `khoa_multi`. (Ca đỏ trước khi sửa.)
#[test]
fn khong_con_fast_path_khoa_v1() {
    let main = std::fs::read_to_string(goc().join("crates/vis-cli/src/main.rs")).expect("main");
    let cache = std::fs::read_to_string(goc().join("crates/vis-cli/src/cache.rs")).expect("cache");
    assert!(
        !main.contains("cache::khoa("),
        "main.rs còn đọc khoá v1 (fast-path chết)"
    );
    assert!(
        !cache.contains("pub fn khoa("),
        "cache.rs còn `khoa` v1 không ai ghi"
    );
    assert!(main.contains("cache::khoa_multi("), "phải dùng khoá multi");
}

/// **ST-87b ca (a)** — một tệp **một module**: hai lần `--vm --tap` **byte y hệt**, lần 2 **~12–20 ms**
/// (không còn marker `(cache)` vì đường duy nhất là `khoa_multi`; cache hit bỏ codegen).
#[test]
fn mot_module_lan_hai_byte_y_het_va_nhanh() {
    let d = thu_muc("a");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let f = d.join("a.vis");
    std::fs::write(&f, "print(1 + 2)\n").expect("ghi");
    let (c1, o1, e1, t1) = chay(&cache, &f, &["--vm", "--tap"]);
    let (c2, o2, e2, t2) = chay(&cache, &f, &["--vm", "--tap"]);
    assert_eq!((c1, &o1, &e1), (c2, &o2, &e2), "hai lần phải byte y hệt");
    assert_eq!(o1, b"3\n");
    assert!(
        !e1.windows(7).any(|w| w == b"(cache)"),
        "đã bỏ fast-path: {e1:?}"
    );
    assert!(
        t2 < 0.10,
        "lần 2 phải ~12–20 ms, thật {:.1} ms (lần 1 {:.1} ms)",
        t2 * 1000.0,
        t1 * 1000.0
    );
    assert!(dem_vbc(&cache) >= 1, "phải có .vbc");
    println!(
        "FASTPATH_MOT_MODULE lan1={:.1}ms lan2={:.1}ms",
        t1 * 1000.0,
        t2 * 1000.0
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-87b ca (b)** — `use ./x`: cache hit đúng như cũ (không hồi quy); chạy lần 2 **không ghi lại**
/// `.vbc` (mtime không đổi) và byte y hệt.
#[test]
fn module_cache_hit_khong_hoi_quy() {
    let d = thu_muc("b");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let x = d.join("x.vis");
    let main = d.join("main.vis");
    std::fs::write(&x, "pub fn gia_tri() -> int:\n    1\n").expect("ghi x");
    std::fs::write(&main, "use ./x\nprint(gia_tri())\n").expect("ghi main");
    let (_, o1, e1, t1) = chay(&cache, &main, &["--vm"]);
    assert_eq!(o1, b"1\n", "{e1:?}");
    assert_eq!(dem_vbc(&cache), 1, "một khoá multi");
    let vbc1 = tim_vbc(&cache);
    assert_eq!(vbc1.len(), 1);
    let mtime1 = std::fs::metadata(&vbc1[0]).unwrap().modified().unwrap();
    let (_, o2, e2, t2) = chay(&cache, &main, &["--vm"]);
    assert_eq!(o1, o2, "{e2:?}");
    assert_eq!(dem_vbc(&cache), 1);
    let vbc2 = tim_vbc(&cache);
    assert_eq!(vbc2.len(), 1);
    let mtime2 = std::fs::metadata(&vbc2[0]).unwrap().modified().unwrap();
    assert_eq!(mtime1, mtime2, "cache hit không được ghi lại .vbc");
    assert!(
        t2 <= t1 + 0.20,
        "lần 2 không chậm hơn: {:.1} vs {:.1} ms",
        t2 * 1000.0,
        t1 * 1000.0
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-87b ca (c)** — sửa **tệp chính** ⇒ khoá đổi ⇒ cache miss (kết quả mới).
#[test]
fn sua_tep_chinh_cache_miss() {
    let d = thu_muc("c");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let f = d.join("a.vis");
    std::fs::write(&f, "print(1)\n").expect("ghi");
    assert_eq!(chay(&cache, &f, &["--vm"]).1, b"1\n");
    std::fs::write(&f, "print(2)\n").expect("sửa");
    assert_eq!(chay(&cache, &f, &["--vm"]).1, b"2\n");
    assert_eq!(dem_vbc(&cache), 2, "khoá chính đổi ⇒ thêm .vbc");
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-87b ca (d)** — sửa **tệp phụ** (`./x`) ⇒ khoá multi đổi ⇒ cache miss.
#[test]
fn sua_tep_phu_cache_miss() {
    let d = thu_muc("d");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let x = d.join("x.vis");
    let main = d.join("main.vis");
    std::fs::write(&x, "pub fn gia_tri() -> int:\n    1\n").expect("ghi x");
    std::fs::write(&main, "use ./x\nprint(gia_tri())\n").expect("ghi main");
    assert_eq!(chay(&cache, &main, &["--vm"]).1, b"1\n");
    std::fs::write(&x, "pub fn gia_tri() -> int:\n    3\n").expect("sửa x");
    assert_eq!(chay(&cache, &main, &["--vm"]).1, b"3\n");
    assert_eq!(dem_vbc(&cache), 2, "khoá phụ đổi ⇒ thêm .vbc");
    let _ = std::fs::remove_dir_all(&d);
}
