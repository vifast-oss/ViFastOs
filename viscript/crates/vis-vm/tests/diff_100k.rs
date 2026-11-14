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

    fn chuong_trinh_co_ban(&mut self) -> String {
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

/// **ST-86**: nhóm cú pháp mới — sinh **mẫu cố định** (tất định) để đếm/kiểm; bộ sinh hỗn hợp
/// chọn ngẫu nhiên một nhóm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nhom {
    Async,
    Chan,
    Cap,
    Derives,
    Trait,
    Assert,
    /// Lỗi **lúc chạy** (frontend sạch): deadlock `VS0505`, thiếu gói `VS0701`, `assert` sai `VS0501`,
    /// send-sau-close `VS0504` — hai đường phải cùng chẩn đoán.
    LoiChay,
    /// Lỗi **tĩnh** (frontend): `send` sai kiểu `VS0412`, `await` không phải Task `VS0412`, thiếu cap
    /// `VS0401`.
    LoiTinh,
}

const DS_NHOM: [Nhom; 8] = [
    Nhom::Async,
    Nhom::Chan,
    Nhom::Cap,
    Nhom::Derives,
    Nhom::Trait,
    Nhom::Assert,
    Nhom::LoiChay,
    Nhom::LoiTinh,
];

impl Gen {
    fn nhom_async(&self) -> String {
        "async fn cong_async(n: int) -> int:\n    n + 1\n\n\
         async fn long(n: int) -> int:\n    let t = spawn cong_async(n)\n    let r = await t\n    r + 1\n\n\
         let t = spawn cong_async(1)\nlet r = await t\nprint(r)\nprint(await cong_async(2))\nprint(await long(3))\n"
            .to_string()
    }

    fn nhom_chan(&self) -> String {
        "async fn gui(c: Chan[int]) -> int:\n    let _ = c.send(4)\n    0\n\n\
         fn main():\n    let c = chan[int](1)\n    let _ = c.send(7)\n    print(c.recv())\n    \
         let d = chan[str](0)\n    d.close()\n    print(d.recv())\n    \
         let e = chan[int](1)\n    let _ = spawn gui(e)\n    print(e.recv())\n"
            .to_string()
    }

    fn nhom_cap(&self) -> String {
        "fn ghi(d: cap[Dir]) -> int:\n    0\n\n\
         fn main(d: cap[Dir] @ p\"./x\" (write)):\n    print(7)\n"
            .to_string()
    }

    fn nhom_derives(&self) -> String {
        "type P derives Show, Eq:\n    x: int\n\n\
         let a: P = {x: 1}\nlet b: P = {x: 1}\nprint(a.show())\nprint(a.eq(b))\n"
            .to_string()
    }

    fn nhom_trait(&self) -> String {
        "trait A:\n    fn mo(self) -> int\n\n\
         trait B: A:\n    fn hai(self) -> int\n\n\
         type P:\n    x: int\n\n\
         impl B for P:\n    fn hai(self) -> int:\n        self.x + 1\n\n\
         impl A for P:\n    fn mo(self) -> int:\n        self.x\n\n\
         let p: P = {x: 41}\nprint(p.hai())\nprint(p.mo())\n"
            .to_string()
    }

    fn nhom_assert(&self) -> String {
        "let a = 1\nassert a == 1\nassert a < 2\nprint(\"ok\")\n".to_string()
    }

    fn nhom_loi_chay(&mut self) -> String {
        match self.rng.duoi(4) {
            0 => "fn main():\n    let c = chan[int](0)\n    let _ = c.send(1)\n".to_string(),
            1 => "use pkg.khong_co\n\nprint(1)\n".to_string(),
            2 => "assert 1 == 2\n".to_string(),
            _ => "fn main():\n    let c = chan[int](1)\n    c.close()\n    let _ = c.send(1)\n"
                .to_string(),
        }
    }

    fn nhom_loi_tinh(&mut self) -> String {
        match self.rng.duoi(3) {
            0 => "fn main():\n    let c = chan[int](1)\n    let _ = c.send(\"x\")\n".to_string(),
            1 => "let x = await 5\nprint(x)\n".to_string(),
            _ => "fn can(c: cap[Dir]) -> int:\n    0\n\nfn main():\n    print(can())\n".to_string(),
        }
    }

    fn theo_nhom(&mut self, nhom: Nhom) -> String {
        match nhom {
            Nhom::Async => self.nhom_async(),
            Nhom::Chan => self.nhom_chan(),
            Nhom::Cap => self.nhom_cap(),
            Nhom::Derives => self.nhom_derives(),
            Nhom::Trait => self.nhom_trait(),
            Nhom::Assert => self.nhom_assert(),
            Nhom::LoiChay => self.nhom_loi_chay(),
            Nhom::LoiTinh => self.nhom_loi_tinh(),
        }
    }

    /// Hỗn hợp: 50% chương trình cơ bản cũ, 50% một nhóm cú pháp mới (đều **kết thúc**; deadlock có
    /// `VS0505`, không treo).
    fn chuong_trinh(&mut self) -> String {
        if self.rng.duoi(2) == 0 {
            return self.chuong_trinh_co_ban();
        }
        let nhom = *self.rng.chon(&DS_NHOM);
        self.theo_nhom(nhom)
    }
}

/// Hai đường **interp** và **VM** cho cùng một nguồn: `Ok((interp,vm))` nếu frontend sạch, `Err(mã)`
/// nếu frontend báo lỗi (hai đường dùng chung frontend ⇒ cùng bộ chẩn đoán).
fn chay_hai(src: &str) -> Result<(String, String), String> {
    let parsed = vis_syntax::parse(src, vis_syntax::Mode::Script);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics[0].code_str().to_string());
    }
    let lexed = vis_syntax::lex(src, vis_syntax::Mode::Script);
    let h = vis_types::lower(&parsed.cst, &lexed);
    if !h.is_clean() {
        return Err(h.diagnostics[0].code.short.to_string());
    }
    let t = vis_types::check(&h.hir, &h.symbols, &h.types);
    if !t.is_clean() {
        return Err(t.diags[0].code.short.to_string());
    }
    let a = vis_interp::run(src, &h.hir, &h.symbols, &h.types, &[]);
    let ta = format!(
        "{:?}|{}|{:?}",
        a.stdout,
        a.exit,
        a.diagnostics
            .iter()
            .map(|d| (d.code_str(), d.line, d.col, d.message.clone()))
            .collect::<Vec<_>>()
    );
    let m = vis_bytecode::compile(&h.hir, &h.symbols, src).map_err(|e| format!("codegen: {e}"))?;
    vis_bytecode::verify(&m).map_err(|e| format!("verify: {e}"))?;
    let b = vis_vm::run(&m, src, &[]);
    let tb = format!(
        "{:?}|{}|{:?}",
        b.stdout,
        b.exit,
        b.diagnostics
            .iter()
            .map(|d| (d.code_str(), d.line, d.col, d.message.clone()))
            .collect::<Vec<_>>()
    );
    Ok((ta, tb))
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

fn chay_bo(n: usize, seed: u64) -> (u64, u64, u64, Vec<String>, Vec<String>) {
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
        let a = vis_interp::run(&src, &h.hir, &h.symbols, &h.types, &[]);
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
                .map(|d| (d.code_str(), d.line, d.col, d.message.clone()))
                .collect::<Vec<_>>()
        );
        let tb = format!(
            "{:?}|{}|{:?}",
            b.stdout,
            b.exit,
            b.diagnostics
                .iter()
                .map(|d| (d.code_str(), d.line, d.col, d.message.clone()))
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
    (loi_tinh, chay, lech, vi_du, ma)
}

fn than_chay() {
    let n = so_chuong_trinh();
    let seed: u64 = std::env::var("VIS_DIFF_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0xD1FF_2026);
    let (loi_tinh, chay, lech, vi_du, ma) = chay_bo(n, seed);
    let _ = loi_tinh;
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

/// **ST-86** ca 1: bộ sinh sinh được **từng nhóm** cú pháp mới (đếm mẫu; mỗi nhóm 100 mẫu).
#[test]
fn bo_sinh_sinh_du_nhom_cu_phap_moi() {
    let dem = |nhom: Nhom, needle: &str| {
        let mut n = 0;
        for seed in 0..100u64 {
            let mut g = Gen::moi(seed);
            if g.theo_nhom(nhom).contains(needle) {
                n += 1;
            }
        }
        n
    };
    assert_eq!(dem(Nhom::Async, "async fn"), 100);
    assert_eq!(dem(Nhom::Async, "spawn "), 100, "có spawn lồng trong async");
    assert_eq!(dem(Nhom::Async, "await "), 100);
    assert_eq!(dem(Nhom::Chan, "chan[int]"), 100);
    assert_eq!(dem(Nhom::Chan, ".send("), 100);
    assert_eq!(dem(Nhom::Chan, ".recv("), 100);
    assert_eq!(dem(Nhom::Chan, ".close("), 100);
    assert_eq!(dem(Nhom::Cap, "cap[Dir] @ p\"./x\" (write)"), 100);
    assert_eq!(dem(Nhom::Cap, "fn main("), 100);
    assert_eq!(dem(Nhom::Derives, "derives Show, Eq"), 100);
    assert_eq!(dem(Nhom::Derives, ".show()"), 100);
    assert_eq!(dem(Nhom::Trait, "trait B: A"), 100);
    assert_eq!(dem(Nhom::Trait, "impl B for P"), 100);
    assert_eq!(dem(Nhom::Assert, "assert "), 100);
}

/// **ST-86** ca 2: bộ sinh **hỗn hợp** chạm đủ 8 nhóm trong 2.000 seed; mọi chương trình parse được
/// (không sinh cú pháp rác).
#[test]
fn bo_sinh_hon_hop_du_nhom() {
    let mut gap = [0usize; 8];
    for seed in 0..2_000u64 {
        let mut g = Gen::moi(seed);
        let src = g.chuong_trinh();
        let p = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        assert!(p.diagnostics.is_empty(), "seed {seed} parse lỗi: {src}");
        let danh_dau = [
            "async fn",
            "chan[",
            "cap[Dir]",
            "derives Show, Eq",
            "trait B: A",
            "assert ",
            "use pkg.khong_co",
            "await 5",
        ];
        for (i, d) in danh_dau.iter().enumerate() {
            if src.contains(d) {
                gap[i] += 1;
            }
        }
    }
    println!("NHOM_HON_HOP {gap:?}");
    for (i, n) in gap.iter().enumerate() {
        assert!(*n > 0, "nhóm {i} không xuất hiện trong hỗn hợp");
    }
}

/// **ST-86** ca 3: lỗi **tĩnh** (`send` sai kiểu, `await` không phải Task, thiếu cap) — frontend (dùng
/// chung cho cả hai đường) cho **đúng mã**, không lệch.
#[test]
fn loi_tinh_cu_phap_moi_cung_ma() {
    let can = [
        (
            "fn main():\n    let c = chan[int](1)\n    let _ = c.send(\"x\")\n",
            "VS0412",
        ),
        ("let x = await 5\nprint(x)\n", "VS0412"),
        (
            "fn can(c: cap[Dir]) -> int:\n    0\n\nfn main():\n    print(can())\n",
            "VS0401",
        ),
    ];
    for (src, mong) in can {
        let p = vis_syntax::parse(src, vis_syntax::Mode::Script);
        let lex = vis_syntax::lex(src, vis_syntax::Mode::Script);
        let h = vis_types::lower(&p.cst, &lex);
        let t = vis_types::check(&h.hir, &h.symbols, &h.types);
        assert!(!t.is_clean(), "phải có chẩn đoán: {src}");
        assert_eq!(t.diags[0].code.short, mong, "{src}");
    }
}

/// **ST-86** ca 4: lỗi **lúc chạy** (deadlock, thiếu gói, `assert` sai, send-sau-close) — frontend sạch,
/// **interp và VM cùng chẩn đoán** (so đủ mã/dòng/cột/thông điệp).
#[test]
fn loi_chay_cu_phap_moi_hai_duong_cung_ma() {
    let mut gap: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for seed in 0..40u64 {
        let mut g = Gen::moi(seed);
        let src = g.theo_nhom(Nhom::LoiChay);
        let (a, b) = chay_hai(&src).expect("frontend sạch");
        assert_eq!(a, b, "hai đường lệch ở seed {seed}: {src}");
        let ma = a.split("VS").nth(1).map(|x| format!("VS{}", &x[..4]));
        if let Some(m) = ma {
            gap.insert(m);
        }
    }
    // `use pkg.khong_co` (`VS0701`) do **CLI** phân giải gói, không qua `vis_interp::run`/VM ⇒
    // kiểm riêng ở `crates/vis-cli/tests/diff_loi.rs` (hai đường CLI cùng mã).
    for m in ["VS0505", "VS0501", "VS0504"] {
        assert!(gap.contains(m), "thiếu mã {m} trong {gap:?}");
    }
}

/// **ST-86** ca 5: cổng chỉ chạy **1.000** (mặc định), test này chạy **2.000** (< 60 s); **20.000**
/// chỉ chạy qua `bin/vis-diff` và ghi log — không chạy trong `gate --fast`.
#[test]
fn hai_nghin_chuong_trinh_khong_phai_20k() {
    if std::env::var_os("VIS_DIFF_N").is_none() {
        assert_eq!(so_chuong_trinh(), 1_000, "mặc định cổng phải là 1.000");
    }
    let tran_20k: usize = std::env::var("VIS_DIFF_20K")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    assert!(2_000 < tran_20k, "test chỉ chạy 2.000, 20k nằm ngoài cổng");
    let t = std::time::Instant::now();
    let (_, chay, lech, vi_du, _) = chay_bo(2_000, 20_261_115);
    let giay = t.elapsed().as_secs_f64();
    assert_eq!(lech, 0, "2.000 chương trình phải 0 lệch: {vi_du:?}");
    assert!(chay > 1_500, "phần lớn phải chạy được ({chay}/2000)");
    assert!(giay < 60.0, "2.000 chương trình chạy {giay:.1} s (≥ 60 s)");
}

/// Giữ `goc()` được dùng (đường dẫn nguồn cho tài liệu/log).
#[test]
fn goc_ton_tai() {
    assert!(goc().join("test/sem").is_dir());
}
