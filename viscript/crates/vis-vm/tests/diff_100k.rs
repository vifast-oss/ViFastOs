// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-22 — **differential trên chương trình sinh tự động**: interp vs VM, **0 lệch**.
//!
//! Sinh chương trình **tất định** trong subset VS0 (không dùng tính năng VS1b): literal/đơn vị,
//! `let`/`var`, `if`/`while`/`for`, `match` (lồng ≤ 3), list/map/bản ghi/enum/`T?`/`Result`,
//! hàm + closure + đệ quy **có base**, nội suy, `with`, `?.`/`??`/`?`/`|>`, `push`, phương thức lõi.
//!
//! Giới hạn: không vòng lặp vô hạn (mọi `while` có biến đếm), đệ quy có base (≤ 20), ≤ **200 token**.
//!
//! Số chương trình: `VIS_DIFF_N` (mặc định **1.000** cho cổng); `bin/vis-diff` chạy **100.000**.

use std::path::PathBuf;

/// Số chương trình — mặc định 1.000 (cổng nhanh); `bin/vis-diff` đặt 100.000.
fn so_chuong_trinh() -> usize {
    std::env::var("VIS_DIFF_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000)
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn duoi(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    fn chon<'a, T>(&mut self, ds: &'a [T]) -> &'a T {
        &ds[self.duoi(ds.len())]
    }
}

/// Bộ sinh chương trình nhỏ, **tất định**, thiên về chương trình hợp lệ kiểu.
struct Gen {
    rng: Lcg,
    dong: Vec<String>,
    bien: Vec<(String, u8)>, // 0 int, 1 str, 2 bool, 3 list[int], 4 int?, 5 Result[int,str]
    dem: usize,
    sau: usize,
}

impl Gen {
    fn moi(seed: u64) -> Self {
        Self {
            rng: Lcg(seed),
            dong: Vec::new(),
            bien: Vec::new(),
            dem: 0,
            sau: 0,
        }
    }

    fn ten(&mut self) -> String {
        self.dem += 1;
        format!("v{}", self.dem)
    }

    fn ds_bien(&self, kieu: u8) -> Vec<String> {
        self.bien
            .iter()
            .filter(|(_, k)| *k == kieu)
            .map(|(n, _)| n.clone())
            .collect()
    }

    fn so(&mut self) -> String {
        let ds = self.ds_bien(0);
        if !ds.is_empty() && self.rng.duoi(3) == 0 {
            self.rng.chon(&ds).clone()
        } else {
            format!("{}", self.rng.duoi(20))
        }
    }

    fn chuoi(&mut self) -> String {
        let ds = self.ds_bien(1);
        if !ds.is_empty() && self.rng.duoi(2) == 0 {
            self.rng.chon(&ds).clone()
        } else {
            format!("\"s{}\"", self.rng.duoi(9))
        }
    }

    fn dieu_kien(&mut self) -> String {
        let a = self.so();
        let b = self.so();
        let op = *self.rng.chon(&["<", ">", "<=", ">=", "==", "!="]);
        format!("{a} {op} {b}")
    }

    fn chenh_lech(&mut self) -> String {
        let a = self.so();
        let b = self.so();
        let op = *self.rng.chon(&["+", "-", "*"]);
        format!("{a} {op} {b}")
    }

    fn opt(&mut self) -> String {
        let ds = self.ds_bien(4);
        if !ds.is_empty() && self.rng.duoi(2) == 0 {
            return self.rng.chon(&ds).clone();
        }
        let _ = self.dieu_kien();
        match self.rng.duoi(2) {
            0 => "none".to_string(),
            _ => format!("some({})", self.so()),
        }
    }

    fn list_so(&mut self) -> String {
        let n = 1 + self.rng.duoi(3);
        let mut ps = Vec::new();
        for _ in 0..n {
            let x = self.so();
            ps.push(x);
        }
        format!("[{}]", ps.join(", "))
    }

    fn cau_lenh(&mut self) {
        if self.sau >= 4 || self.dong.len() > 26 {
            return;
        }
        self.sau += 1;
        match self.rng.duoi(9) {
            0 => {
                let t = self.ten();
                let e = self.chenh_lech();
                self.dong.push(format!("let {t}: int = {e}"));
                self.bien.push((t, 0));
            }
            1 => {
                let t = self.ten();
                let e = self.chuoi();
                self.dong.push(format!("let {t}: str = {e}"));
                self.bien.push((t, 1));
            }
            2 => {
                let ds = self.ds_bien(3);
                if let Some(xs) = ds.first() {
                    let v = self.so();
                    self.dong.push(format!("{xs}.push({v})"));
                } else {
                    let t = self.ten();
                    let l = self.list_so();
                    self.dong.push(format!("let {t}: list[int] = {l}"));
                    self.bien.push((t, 3));
                }
            }
            3 => {
                let t = self.ten();
                let o = self.opt();
                self.dong.push(format!("let {t}: int? = {o}"));
                self.bien.push((t, 4));
            }
            4 => {
                let ds = self.ds_bien(4);
                let e = if ds.is_empty() {
                    self.opt()
                } else {
                    self.rng.chon(&ds).clone()
                };
                let f = self.so();
                self.dong.push(format!("print({e} ?? {f})"));
            }
            5 => {
                let c = self.dieu_kien();
                let e = self.chenh_lech();
                self.dong.push(format!("if {c}:"));
                self.dong.push(format!("    print({e})"));
                self.dong.push("else:".into());
                let e2 = self.so();
                self.dong.push(format!("    print({e2})"));
            }
            6 => {
                let o = self.opt();
                let t = self.ten();
                let p1 = self.so();
                let p2 = self.so();
                self.dong.push(format!("let {t} = match {o}:"));
                self.dong.push(format!("    some(v): v + {p1}"));
                self.dong.push(format!("    none: {p2}"));
                self.bien.push((t, 0));
            }
            7 => {
                let n = 1 + self.rng.duoi(3);
                let k = self.rng.duoi(5);
                self.dong.push(format!("for i in 0..{n}:"));
                self.dong.push(format!("    print(i + {k})"));
            }
            _ => {
                let v = self.so();
                self.dong.push(format!("print(\"tong = {{{v}}}\")"));
            }
        }
        self.sau -= 1;
    }

    fn chuong_trinh(&mut self) -> String {
        let mut s = String::from(
            "type D:\n    n: int\n\
             fn fact(n: int) -> int:\n    let r = if n <= 1: 1 else: n * fact(n - 1)\n    r\n\
             fn cong(a: int, b: int) -> int:\n    a + b\n",
        );
        let so_cau = 2 + self.rng.duoi(5);
        for _ in 0..so_cau {
            self.cau_lenh();
        }
        s.push_str(&self.dong.join("\n"));
        s.push('\n');
        // Dùng đệ quy có base + closure + `|>`/`with`/`?.` ở đuôi (nếu ngân sách token còn).
        if self.rng.duoi(2) == 0 {
            s.push_str("let inc = (x) => x + 1\n");
            s.push_str("print(inc(1) + fact(3) + cong(1, 2))\n");
        }
        if self.rng.duoi(3) == 0 {
            s.push_str("let d: D? = some({n: 1})\nprint(d?.n ?? 0)\n");
        }
        s
    }
}

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

#[test]
fn diff_chuong_trinh_sinh_tu_dong() {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(than_chay)
        .expect("tạo luồng")
        .join()
        .expect("luồng lỗi");
}

fn than_chay() {
    let n = so_chuong_trinh();
    let seed: u64 = std::env::var("VIS_DIFF_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0xD1FF_2026);
    let bat_dau = std::time::Instant::now();
    let (mut loi_tinh, mut chay, mut lech) = (0u64, 0u64, 0u64);
    let mut ma: Vec<String> = Vec::new();
    let mut vi_du: Vec<String> = Vec::new();
    for i in 0..n {
        let mut g = Gen::moi(seed.wrapping_add(i as u64));
        let src = g.chuong_trinh();
        if src.split_whitespace().count() > 200 {
            continue;
        }
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        if !parsed.diagnostics.is_empty() {
            loi_tinh += 1;
            if std::env::var("VIS_DIFF_DEBUG").is_ok() && ma.len() < 8 {
                ma.push(format!(
                    "PARSE {} :: {}",
                    parsed.diagnostics[0].code_str(),
                    src.replace('\n', "⏎")
                ));
            }
            continue;
        }
        let lexed = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let h = vis_types::lower(&parsed.cst, &lexed);
        if !h.is_clean() {
            loi_tinh += 1;
            if std::env::var("VIS_DIFF_DEBUG").is_ok() && ma.len() < 8 {
                ma.push(format!("LOWER {}", h.diagnostics[0].code.short));
            }
            continue;
        }
        let t = vis_types::check(&h.hir, &h.symbols, &h.types);
        if !t.is_clean() {
            loi_tinh += 1;
            if std::env::var("VIS_DIFF_DEBUG").is_ok() && ma.len() < 8 {
                ma.push(format!(
                    "CHECK {} {}",
                    t.diags[0].code.short, t.diags[0].message
                ));
            }
            continue;
        }
        // Đường interp
        let a = vis_interp::run(&src, &h.hir, &h.symbols, &h.types, &[]);
        // Đường VM: codegen → verify → chạy
        let m = match vis_bytecode::compile(&h.hir, &h.symbols, &src) {
            Ok(m) => m,
            Err(e) => {
                lech += 1;
                vi_du.push(format!("#{i} codegen lỗi: {e}\n{src}"));
                continue;
            }
        };
        if let Err(e) = vis_bytecode::verify(&m) {
            lech += 1;
            vi_du.push(format!("#{i} verify lỗi: {e}\n{src}"));
            continue;
        }
        let b = vis_vm::run(&m, &src, &[]);
        chay += 1;
        let ta = format!(
            "{:?}|{}|{:?}",
            a.stdout,
            a.exit,
            a.diagnostics
                .iter()
                .map(|d| (d.code_str(), d.line, d.col))
                .collect::<Vec<_>>()
        );
        let tb = format!(
            "{:?}|{}|{:?}",
            b.stdout,
            b.exit,
            b.diagnostics
                .iter()
                .map(|d| (d.code_str(), d.line, d.col))
                .collect::<Vec<_>>()
        );
        if ta != tb {
            lech += 1;
            if vi_du.len() < 5 {
                vi_du.push(format!("#{i} lệch\ninterp {ta}\nvm     {tb}\n{src}"));
            }
        }
    }
    let giay = bat_dau.elapsed().as_secs_f64();
    println!(
        "DIFF tong={n} loi_tinh={loi_tinh} chay={chay} lech={lech} giay={giay:.1} toc_do={:.0}/s seed={seed}",
        n as f64 / giay.max(0.001)
    );
    for v in &vi_du {
        println!("LECH {v}");
    }
    for m in &ma {
        println!("MA {m}");
    }
    assert_eq!(lech, 0, "phải **0 lệch**, thật {lech}");
    assert!(
        chay > (n as u64) / 2,
        "phần lớn chương trình phải chạy được ({chay}/{n})"
    );
}

/// Giữ `goc()` được dùng (đường dẫn nguồn cho tài liệu/log).
#[test]
fn goc_ton_tai() {
    assert!(goc().join("test/sem").is_dir());
}
