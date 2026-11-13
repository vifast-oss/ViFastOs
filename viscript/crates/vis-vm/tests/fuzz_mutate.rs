// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-21 — **fuzz đột biến bytecode** (VD-52 #3, §17).
//!
//! Nguồn: **303 module** sinh từ ca sem không lỗi (ST-38: 355 ca − 52 `.err`); mỗi vòng: bit-flip / byte-flip / cắt cụt / chèn.
//! Bất biến (mọi vòng):
//! 1. `decode` **không panic**;
//! 2. `verify` **không panic**;
//! 3. nếu `verify` OK ⇒ VM chạy **không panic** (bắt bằng `catch_unwind`), **kết thúc** (cap 10^7
//!    lệnh), và **độ sâu ngăn xếp ≤ `need`** mà verifier tính (cộng dồn theo hàm vì VM dùng **một**
//!    ngăn xếp giá trị cho mọi khung).
//!
//! Chạy 10 phút: `VBC_FUZZ_PHUT=10 cargo test -p vis-vm --test fuzz_mutate -- --nocapture`.

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

use vis_bytecode::{decode, encode, need_stack, verify, Module};

/// Số phút fuzz (mặc định ngắn để cổng nhanh; 10 phút cho log ST-21).
fn so_phut() -> f64 {
    std::env::var("VBC_FUZZ_PHUT")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.02)
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
}

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

/// 303 module sinh từ ca sem không `.err` (đúng bộ với differential; ST-38: 355 − 52).
fn module_goc() -> Vec<(String, Vec<u8>)> {
    let mut ds = Vec::new();
    let dir = goc().join("test/sem");
    let mut tep = Vec::new();
    for nhom in std::fs::read_dir(&dir).expect("đọc test/sem") {
        let p = nhom.expect("entry").path();
        if !p.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&p).expect("đọc nhóm") {
            let q = f.expect("entry").path();
            if q.extension()
                .map(|x| x == "vis" || x == "vish")
                .unwrap_or(false)
                && !q.with_extension("err").exists()
            {
                tep.push(q);
            }
        }
    }
    tep.sort();
    for p in tep {
        let text = std::fs::read_to_string(&p).expect("đọc");
        let mode = if p.extension().map(|x| x == "vish").unwrap_or(false) {
            vis_syntax::Mode::Command
        } else {
            vis_syntax::Mode::Script
        };
        let parsed = vis_syntax::parse(&text, mode);
        let lexed = vis_syntax::lex(&text, mode);
        let h = vis_types::lower(&parsed.cst, &lexed);
        if let Ok(m) = vis_bytecode::compile(&h.hir, &h.symbols, &text) {
            ds.push((text, encode(&m)));
        }
    }
    ds
}

/// `need` của module = **max** theo hàm (VM cho **mỗi khung một** ngăn xếp giá trị).
fn need_cua_module(m: &Module) -> usize {
    m.funcs
        .iter()
        .enumerate()
        .map(|(i, f)| need_stack(&f.code, i as u32).unwrap_or(0) as usize)
        .max()
        .unwrap_or(0)
}

fn dot_bien(rng: &mut Lcg, goc: &[u8]) -> Vec<u8> {
    let mut b = goc.to_vec();
    match rng.duoi(4) {
        // bit-flip
        0 if !b.is_empty() => {
            let i = rng.duoi(b.len());
            b[i] ^= 1u8 << rng.duoi(8);
        }
        // byte-flip
        1 if !b.is_empty() => {
            let i = rng.duoi(b.len());
            b[i] = rng.next() as u8;
        }
        // cắt cụt
        2 => {
            let n = rng.duoi(b.len());
            b.truncate(n);
        }
        // chèn 1..8 byte
        _ => {
            let i = rng.duoi(b.len() + 1);
            let k = 1 + rng.duoi(8);
            let them: Vec<u8> = (0..k).map(|_| rng.next() as u8).collect();
            for (j, x) in them.into_iter().enumerate() {
                b.insert(i + j, x);
            }
        }
    }
    b
}

#[test]
fn fuzz_dot_bien_bytecode() {
    // VM đệ quy theo khung (cap 2.000) ⇒ cần stack lớn như `vis run`.
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(than_fuzz)
        .expect("tạo luồng")
        .join()
        .expect("luồng lỗi");
}

fn than_fuzz() {
    let goc = module_goc();
    assert_eq!(
        goc.len(),
        303,
        "phải đủ 303 module nguồn (ST-38: 355 ca − 52 `.err`)"
    );
    let phut = so_phut();
    let bat_dau = std::time::Instant::now();
    let han = std::time::Duration::from_secs_f64(phut * 60.0);
    let mut rng = Lcg(0x5EED_2026_1121);
    let (mut tong, mut tu_choi_decode, mut tu_choi_verify, mut chay_duoc, mut vi_pham) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut chi_tiet: Vec<String> = Vec::new();
    while bat_dau.elapsed() < han {
        let (ten, bytes) = &goc[rng.duoi(goc.len())];
        let dot = dot_bien(&mut rng, bytes);
        tong += 1;
        // 1) decode không panic
        let m = match std::panic::catch_unwind(|| decode(&dot)) {
            Ok(Ok(m)) => m,
            Ok(Err(_)) => {
                tu_choi_decode += 1;
                continue;
            }
            Err(_) => {
                vi_pham += 1;
                chi_tiet.push(format!("{ten}: decode PANIC"));
                continue;
            }
        };
        // 2) verify không panic
        match std::panic::catch_unwind(AssertUnwindSafe(|| verify(&m))) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                tu_choi_verify += 1;
                continue;
            }
            Err(_) => {
                vi_pham += 1;
                chi_tiet.push(format!("{ten}: verify PANIC"));
                continue;
            }
        }
        // 3) verify OK ⇒ VM không panic + không vượt `need`
        let need = need_cua_module(&m);
        let kq = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let mut vm = vis_vm::Vm::new(&m, "");
            let out = vm.run(&[]);
            (out.steps, vm.max_stack_used())
        }));
        match kq {
            Ok((buoc, dinh)) => {
                chay_duoc += 1;
                if dinh > need {
                    vi_pham += 1;
                    chi_tiet.push(format!(
                        "{ten}: ngăn xếp {dinh} > need {need} (steps {buoc})"
                    ));
                }
            }
            Err(_) => {
                vi_pham += 1;
                chi_tiet.push(format!("{ten}: VM PANIC"));
            }
        }
    }
    println!("FUZZ phut={phut} tong={tong} decode_tu_choi={tu_choi_decode} verify_tu_choi={tu_choi_verify} chay_duoc={chay_duoc} vi_pham={vi_pham} giay={:.1}",
        bat_dau.elapsed().as_secs_f64());
    for l in chi_tiet.iter().take(10) {
        println!("VI_PHAM {l}");
    }
    assert_eq!(vi_pham, 0, "phải **0** vi phạm, thật {vi_pham}");
}
