// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-87 (VD-109)** — cache **không che lỗi**: chạy toàn bộ `test/sem/**` + `test/std/**` ở **3 chế
//! độ** (`run` interp; `run --vm` hai lần, lần 2 là cache; `run --vm --no-cache`), so **byte**
//! `(exit, stdout, stderr)`. 520 tệp (≥500). **SỬA KÈM LÝ DO (ST-89)**: VM **đã có host `Ngoai`** ⇒
//! **bỏ danh sách 7 ca loại trừ**; nay so **cả 520 ca × 4 lần**, không còn `VS0604` có chủ đích.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn thu_muc_cache(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-cache-st87-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp cache");
    d
}

fn chay(cache: &Path, f: &str, args: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(goc())
        .env("XDG_CACHE_HOME", cache)
        .arg("run")
        .args(args)
        .arg(f)
        .output()
        .expect("chạy vis");
    (o.status.code().unwrap_or(-1), o.stdout, o.stderr)
}

fn dem_vbc(dir: &Path) -> usize {
    let mut n = 0;
    let Ok(ds) = std::fs::read_dir(dir) else {
        return 0;
    };
    for e in ds.flatten() {
        let p = e.path();
        if p.is_dir() {
            n += dem_vbc(&p);
        } else if p.extension().and_then(|x| x.to_str()) == Some("vbc") {
            n += 1;
        }
    }
    n
}

fn gom(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(ds) = std::fs::read_dir(dir) else {
        return;
    };
    for e in ds.flatten() {
        let p = e.path();
        if p.is_dir() {
            gom(&p, out);
        } else if matches!(p.extension().and_then(|x| x.to_str()), Some("vis" | "vish")) {
            out.push(p);
        }
    }
}

fn ds_tep() -> Vec<PathBuf> {
    let mut ds = Vec::new();
    gom(&goc().join("test/sem"), &mut ds);
    gom(&goc().join("test/std"), &mut ds);
    ds.sort();
    ds
}

/// So một cặp: `la_loi` ⇒ **bỏ stdout** (đúng yêu cầu `.err`), vẫn so exit + stderr.
fn khac(a: &(i32, Vec<u8>, Vec<u8>), b: &(i32, Vec<u8>, Vec<u8>), la_loi: bool) -> bool {
    a.0 != b.0 || a.2 != b.2 || (!la_loi && a.1 != b.1)
}

/// **ST-87 ca (d)** — **520 tệp × 4 lần chạy** (A, B1, B2, C), byte y hệt; **0 lệch cache**.
#[test]
fn ba_che_do_520_tep_khong_lech() {
    let ds = ds_tep();
    assert!(
        ds.len() >= 500,
        "cần ≥500 ca (sem + std), thật {}",
        ds.len()
    );
    let cache = thu_muc_cache("all");
    let goc = goc();
    let mut so = 0usize;
    let mut lech: Vec<String> = Vec::new();
    for p in &ds {
        let rel = p
            .strip_prefix(&goc)
            .expect("dưới gốc")
            .to_string_lossy()
            .to_string();
        let la_loi = goc.join(format!("{rel}.err")).exists();
        let a = chay(&cache, &rel, &[]);
        let b1 = chay(&cache, &rel, &["--vm"]);
        let b2 = chay(&cache, &rel, &["--vm"]);
        let c = chay(&cache, &rel, &["--vm", "--no-cache"]);
        // Cache: B1 = B2 = C **luôn**.
        if khac(&b1, &b2, la_loi) || khac(&b1, &c, la_loi) {
            lech.push(format!(
                "{rel}: B1≠B2 hoặc B1≠C\n B1={b1:?}\n B2={b2:?}\n C ={c:?}"
            ));
            continue;
        }
        if khac(&a, &b1, la_loi) {
            lech.push(format!("{rel}: interp≠VM\n A ={a:?}\n B1={b1:?}"));
        }
        so += 1;
    }
    assert!(
        lech.is_empty(),
        "{} lệch / {} ca so:\n{}",
        lech.len(),
        so,
        lech.join("\n")
    );
    println!(
        "CACHE_KHONG_CHE ca={} so={} cache_dir={}",
        ds.len(),
        so,
        cache.display()
    );
    let _ = std::fs::remove_dir_all(&cache);
}

/// **ST-87 ca (a)** — hai lần `--vm` cùng tệp: byte y hệt; lần 2 **không chậm hơn** (đo, ngưỡng lỏng).
#[test]
fn hai_lan_vm_cache_byte_y_het() {
    let d = thu_muc_cache("a");
    let f = d.join("a.vis");
    std::fs::write(&f, "print(1 + 2)\n").expect("ghi");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let t1 = Instant::now();
    let a = chay(&cache, f.to_str().unwrap(), &["--vm"]);
    let d1 = t1.elapsed();
    let t2 = Instant::now();
    let b = chay(&cache, f.to_str().unwrap(), &["--vm"]);
    let d2 = t2.elapsed();
    assert_eq!(a, b, "hai lần --vm phải byte y hệt");
    assert_eq!(a.1, b"3\n");
    assert!(
        d2.as_secs_f64() <= d1.as_secs_f64() + 0.20,
        "lần 2 phải nhanh hơn/không chậm hơn đáng kể: lần 1 {d1:?}, lần 2 {d2:?}"
    );
    let so_vbc = dem_vbc(&cache);
    assert!(so_vbc >= 1, "phải có cache .vbc");
    println!("CACHE_HAI_LAN lan1={d1:?} lan2={d2:?} vbc={so_vbc}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-87 ca (b)** — cùng nội dung, `.vis` vs `.vish`: **khoá cache tách theo mode** (2 tệp `.vbc`),
/// và nội dung có ngữ nghĩa khác (`echo`) cho kết quả khác, không lẫn cache.
#[test]
fn mode_vis_vish_khong_lan_cache() {
    let d = thu_muc_cache("b");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let vis = d.join("a.vis");
    let vish = d.join("a.vish");
    let src = "let x = 1\nprint(x)\n";
    std::fs::write(&vis, src).expect("ghi vis");
    std::fs::write(&vish, src).expect("ghi vish");
    let av = chay(&cache, vis.to_str().unwrap(), &["--vm"]);
    let bv = chay(&cache, vish.to_str().unwrap(), &["--vm"]);
    assert_eq!(av.1, b"1\n");
    assert_eq!(bv.1, b"1\n");
    let so_vbc = dem_vbc(&cache);
    assert_eq!(so_vbc, 2, "mode phải là một phần khoá cache (.vis ≠ .vish)");
    // Ngữ nghĩa khác: `echo "a"` chạy ở `.vish`, lỗi ở `.vis`; cache của `.vish` không được dùng.
    let vm = d.join("b.vish");
    let sm = d.join("b.vis");
    std::fs::write(&vm, "echo \"a\"\n").expect("ghi");
    std::fs::write(&sm, "echo \"a\"\n").expect("ghi");
    let r1 = chay(&cache, vm.to_str().unwrap(), &["--vm"]);
    let r2 = chay(&cache, sm.to_str().unwrap(), &["--vm"]);
    let r3 = chay(&cache, vm.to_str().unwrap(), &["--vm"]);
    assert_eq!(r1.1, b"a\n", "vish phải chạy echo: {r1:?}");
    assert_eq!(r2.0, 2, "vis không có echo (lỗi frontend): {r2:?}");
    assert_eq!(r1, r3, "chạy lại vish không được lẫn cache vis");
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-87 ca (c)** — đổi module `use ./x`: cache cũ **không** được dùng (kết quả đổi theo).
#[test]
fn module_doi_cache_khong_dung_lai() {
    let d = thu_muc_cache("c");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let x = d.join("x.vis");
    let main = d.join("main.vis");
    std::fs::write(&x, "pub fn gia_tri() -> int:\n    1\n").expect("ghi x");
    std::fs::write(&main, "use ./x\nprint(gia_tri())\n").expect("ghi main");
    let r1 = chay(&cache, main.to_str().unwrap(), &["--vm"]);
    assert_eq!(r1.1, b"1\n", "lần 1: {r1:?}");
    std::fs::write(&x, "pub fn gia_tri() -> int:\n    2\n").expect("sửa x");
    let r2 = chay(&cache, main.to_str().unwrap(), &["--vm"]);
    assert_eq!(r2.1, b"2\n", "đổi module phải đổi kết quả: {r2:?}");
    let r3 = chay(&cache, main.to_str().unwrap(), &["--vm", "--no-cache"]);
    assert_eq!(r3.1, b"2\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-87** — `--strict-caps` không bị cache che: cùng bytecode cache, bật strict ⇒ `VS0604` **cả hai
/// lần**; tắt strict ⇒ chạy bình thường. Kết luận: dùng chung cache **an toàn** vì quyền kiểm ở runtime.
#[test]
fn strict_caps_khong_bi_cache_che() {
    let d = thu_muc_cache("d");
    let cache = d.join("cache");
    std::fs::create_dir_all(&cache).expect("cache");
    let f = d.join("a.vis");
    std::fs::write(
        &f,
        "use std.fs\nfn main():\n    print(fs.exists(\"khong.txt\"))\n",
    )
    .expect("ghi");
    let thuong1 = chay(&cache, f.to_str().unwrap(), &["--vm"]);
    let thuong2 = chay(&cache, f.to_str().unwrap(), &["--vm"]);
    assert_eq!(thuong1, thuong2);
    assert_eq!(thuong1.1, b"false\n");
    let strict1 = chay(&cache, f.to_str().unwrap(), &["--vm", "--strict-caps"]);
    let strict2 = chay(&cache, f.to_str().unwrap(), &["--vm", "--strict-caps"]);
    assert_eq!(strict1, strict2, "strict phải ổn định qua cache");
    assert!(
        String::from_utf8_lossy(&strict1.2).contains("VS0604"),
        "{strict1:?}"
    );
    assert_ne!(thuong1.0, strict1.0, "strict phải khác exit");
    let _ = std::fs::remove_dir_all(&d);
}
