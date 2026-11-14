// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-75 (VD-104(3))** — bộ hợp chuẩn host: **≥50 ca chạy THẬT trên macOS**, mỗi ca chạy **cả**
//! `vis run` và `vis run --vm --no-cache`, so **byte** stdout + mã thoát (+ stderr chứa nếu khai).
//!
//! Dữ liệu: `test/host/suite.txt` (TSV 5–6 cột, xem header tệp). Mức bằng chứng: **chạy thật trên
//! macOS**; Windows/Linux/wasm **chưa đo**. Không có ca "xanh vì không kiểm gì": mỗi ca phải có
//! stdout khác rỗng hoặc mã thoát khác 0.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn suite_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/host")
}

fn ghi(p: &Path, s: &str) {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).expect("mkdir");
    }
    std::fs::write(p, s).expect("ghi fixture");
}

fn tai_bien(d: &Path) {
    ghi(&d.join("in.txt"), "xin chào");
    ghi(&d.join("empty.txt"), "");
    ghi(&d.join("utf8.txt"), "Xin chào 👋");
    ghi(&d.join("crlf.txt"), "a\r\nb\r\n");
    ghi(&d.join("space name.txt"), "có dấu cách");
    ghi(&d.join("tệp-việt.txt"), "việt");
    ghi(&d.join("big.txt"), &"x".repeat(65536));
    ghi(&d.join("sub/long.txt"), "long");
    for t in ["a.txt", "b.txt", "c.txt"] {
        ghi(&d.join("list").join(t), "x");
    }
    std::fs::create_dir_all(d.join("list/sub")).expect("mkdir list/sub");
    ghi(&d.join("root/in.txt"), "trong");
    ghi(&d.join("ngoai.txt"), "ngoai");
    ghi(&d.join("a/a.txt"), "A");
    ghi(&d.join("b/b.txt"), "B");
}

fn giai(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(x) => {
                out.push('\\');
                out.push(x);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Trả `(chương trình, đuôi, flags, repeat)`.
fn tach(prog: &str, args: &str) -> (String, &'static str, Vec<String>, usize, bool) {
    let (src, duoi) = match prog.split_once(':') {
        Some(("vish", r)) => (r, "vish"),
        Some(("vis", r)) => (r, "vis"),
        _ => (prog, "vis"),
    };
    let mut flags = Vec::new();
    let mut repeat = 1usize;
    let mut vm_chet = false;
    for t in args.split_whitespace() {
        if let Some(n) = t.strip_prefix("repeat=") {
            repeat = n.parse().unwrap_or(1).max(1);
        } else if t == "vm:vs0604" {
            // Phần lớn ca chạy cả hai đường; riêng **Ngoai** (`std.vish` gọi chương trình ngoài)
            // đường `--vm` **không có host Ngoai** ⇒ đúng thiết kế là `VS0604`, exit 1 (ST-27/ST-33).
            // Token này khai kỳ vọng đó để ca vẫn chạy **cả hai** đường thay vì bỏ VM.
            vm_chet = true;
        } else {
            flags.push(t.to_string());
        }
    }
    (giai(src), duoi, flags, repeat, vm_chet)
}

fn chay_mot(d: &Path, src: &str, duoi: &str, flags: &[String], vm: bool) -> (i32, Vec<u8>, String) {
    let ten = format!("c.{}", duoi);
    std::fs::write(d.join(&ten), src).expect("ghi chương trình");
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.current_dir(d).arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    for f in flags {
        c.arg(f);
    }
    let o = c.arg(&ten).output().expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        o.stdout,
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

#[test]
fn host_conformance_suite() {
    let suite = suite_dir().join("suite.txt");
    let text = std::fs::read_to_string(&suite).expect("đọc test/host/suite.txt");
    let host = suite_dir();
    let goc = std::env::temp_dir();
    let pid = std::process::id();
    let dem = AtomicUsize::new(0);
    let mut tong = 0usize;
    let mut so_lan = 0usize;
    let mut hong: Vec<String> = Vec::new();

    for dong in text.lines() {
        let dong = dong.trim_end();
        if dong.is_empty() || dong.starts_with('#') {
            continue;
        }
        let cot: Vec<&str> = dong.split('\t').collect();
        assert!(
            cot.len() >= 5 && cot.len() <= 6,
            "dòng suite phải 5–6 cột: {dong:?}"
        );
        let ten = cot[0];
        let (src0, duoi, flags0, repeat, vm_vs0604) = tach(cot[1], cot[2]);
        let muon_out = giai(cot[3]);
        let muon_ma: i32 = cot[4].parse().unwrap_or_else(|_| panic!("mã thoát: {ten}"));
        let muon_err = cot.get(5).map(|s| giai(s)).unwrap_or_default();
        assert!(
            !muon_out.is_empty() || muon_ma != 0,
            "ca `{ten}` không kiểm gì (stdout rỗng + exit 0)"
        );
        let n = dem.fetch_add(1, Ordering::Relaxed);
        let d = goc.join(format!("vis-host-{pid}-{n}-{ten}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("tmp");
        tai_bien(&d);
        let src = src0.replace("{tmp}", d.to_str().unwrap());
        let flags: Vec<String> = flags0
            .iter()
            .map(|f| {
                f.replace("{tmp}", d.to_str().unwrap())
                    .replace("{host}", host.to_str().unwrap())
            })
            .collect();
        tong += 1;
        for vm in [false, true] {
            for lan in 0..repeat {
                let (ma, out, err) = chay_mot(&d, &src, duoi, &flags, vm);
                so_lan += 1;
                let duong = if vm { "vm" } else { "interp" };
                let (ma_muon, out_muon, err_muon) = if vm && vm_vs0604 {
                    (1, String::new(), "VS0604")
                } else {
                    (muon_ma, muon_out.clone(), muon_err.as_str())
                };
                let ok = ma == ma_muon && out == out_muon.as_bytes();
                let ok_err = err_muon.is_empty() || err.contains(err_muon);
                if !ok || !ok_err {
                    hong.push(format!(
                        "{ten} ({duong}, lần {}/{repeat}): exit {ma} (muốn {ma_muon}), stdout {:?} (muốn {:?}), stderr {:?}",
                        lan + 1,
                        String::from_utf8_lossy(&out),
                        out_muon,
                        err.trim()
                    ));
                }
            }
        }
        if hong.iter().any(|h| h.starts_with(ten)) {
            println!("not ok {tong} - {ten}");
        } else {
            println!("ok {tong} - {ten}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }
    assert!(tong >= 50, "bộ hợp chuẩn host cần ≥50 ca, hiện {tong}");
    assert!(
        hong.is_empty(),
        "{} lỗi / {tong} ca ({so_lan} lần chạy):\n{}",
        hong.len(),
        hong.join("\n")
    );
    println!("1..{tong}");
    println!("# {tong} ca, {so_lan} lần chạy (interp + vm), 0 lệch");
}
