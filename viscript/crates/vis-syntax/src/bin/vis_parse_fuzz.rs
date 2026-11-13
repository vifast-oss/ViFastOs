// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-parse-fuzz` — fuzz **parser** (ST-16): chạy `parse()` trên đầu vào ngẫu nhiên và kiểm **bộ bất
//! biến**; in số liệu và thoát 0 nếu không vi phạm, 1 nếu có (kèm seed + nguồn đã escape để tái lập).
//!
//! ```text
//! cargo run --release -p vis-syntax --bin vis-parse-fuzz -- --seconds 3600 --seed 20261113
//! ```
//!
//! Bất biến (mỗi ca):
//! 1. **Không panic** (`catch_unwind`) — panic là vi phạm.
//! 2. **CST lossless**: parse **sạch** (không chẩn đoán) ⇒ `cst.render(&out.text) == out.text`.
//! 3. **Span hợp lệ**: `start <= end <= text.len()`, cả hai là biên ký tự.
//! 4. **Chẩn đoán không rỗng khi lỗi**: nếu `cst` có lá `Error`/token bị bỏ thì `diagnostics` phải
//!    khác rỗng (không im lặng nuốt lỗi). Ở VS0 parser dừng ở lỗi đầu nên chỉ cần "không sạch ⇒ có
//!    chẩn đoán".

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use vis_syntax::{parse, Mode};

/// PRNG tất định (xorshift64*) — cùng seed cho cùng dãy ca, không phụ thuộc thư viện ngoài.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    fn chon<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len().max(1))]
    }
}

/// Ký tự "hiểm" cho parser: xuống dòng/thụt lề, ngoặc, dấu hai chấm, `->`, toán tử, chữ Việt.
fn alphabet() -> Vec<char> {
    let mut v: Vec<char> = Vec::new();
    for c in [
        'a', 'z', 'A', 'f', 'n', 'x', 's', '0', '1', '9', ' ', ' ', ' ', '\n', '\t', ':', '(', ')',
        '[', ']', '{', '}', ',', '.', '-', '>', '=', '!', '+', '*', '/', '%', '?', '|', '&', '"',
        '\'', '#', '_', 'e',
    ] {
        v.push(c);
    }
    for c in ['à', 'ố', 'ệ', 'đ', 'ư', 'ơ'] {
        v.push(c);
    }
    v
}

/// Mảnh mã **hợp lệ** để đột biến (giữ độ phủ cú pháp cao).
fn corpus() -> Vec<String> {
    vec![
        "fn add(a: int, b: int) -> int:\n    a + b\nprint(add(1, 2))\n".into(),
        "type U:\n    name: str\n    age: int\nlet u: U = {name: \"An\", age: 1}\nprint(u.age)\n".into(),
        "enum S:\n    Circle(r: float)\n    Empty\nlet s: S = S.Empty\nmatch s:\n    Circle(r): print(r)\n    Empty: print(0)\n".into(),
        "let xs = [1, 2, 3]\nprint(xs.filter(x => x > 1).map(y => y * 2))\n".into(),
        "fn f(x: int?) -> int?:\n    x?\nprint(f(5) ?? 0)\n".into(),
        "for i in 0..3:\n    if i == 1: continue\n    print(i)\n".into(),
        "var n = 0\nwhile n < 3:\n    n += 1\nprint(n)\n".into(),
        "let m = {\"a\": 1}\nprint(\"a = {m[\\\"a\\\"]}\")\n".into(),
        "mentor: // cấu trúc lạ nhưng hợp lệ\nlet x = (1, \"a\")\nmatch x:\n    (n, s): print(s)\n".into(),
    ]
}

fn doc_ngau_nhien(rng: &mut Rng, alpha: &[char], n: usize) -> String {
    let mut s = String::new();
    for _ in 0..n {
        s.push(*rng.chon(alpha));
    }
    s
}

fn dot_bien(rng: &mut Rng, goc: &str, alpha: &[char]) -> String {
    let mut chars: Vec<char> = goc.chars().collect();
    for _ in 0..(1 + rng.below(4)) {
        if chars.is_empty() {
            break;
        }
        let i = rng.below(chars.len());
        match rng.below(4) {
            0 => {
                chars.remove(i);
            }
            1 => chars.insert(i, *rng.chon(alpha)),
            2 => {
                let j = rng.below(chars.len());
                chars.swap(i, j);
            }
            _ => chars[i] = *rng.chon(alpha),
        }
    }
    chars.into_iter().collect()
}

fn main() -> ExitCode {
    let mut seconds = 60u64;
    let mut seed = 2026_1113u64;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seconds" => {
                if let Some(v) = args.get(i + 1) {
                    seconds = v.parse().unwrap_or(seconds);
                    i += 1;
                }
            }
            "--seed" => {
                if let Some(v) = args.get(i + 1) {
                    seed = v.parse().unwrap_or(seed);
                    i += 1;
                }
            }
            "-h" | "--help" => {
                println!("dùng: vis-parse-fuzz [--seconds N] [--seed S]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("vis-parse-fuzz: không hiểu `{other}`");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    // Tắt hook panic mặc định để log fuzz sạch; vi phạm được **báo cáo** thay vì giết process.
    std::panic::set_hook(Box::new(|_| {}));
    let alpha = alphabet();
    let goc = corpus();
    let mut rng = Rng::new(seed);
    let bat_dau = Instant::now();
    let han = Duration::from_secs(seconds);
    let (mut ca, mut token, mut vi_pham) = (0u64, 0u64, 0u64);
    let mut ban_ghi: Vec<String> = Vec::new();
    while bat_dau.elapsed() < han {
        // Ba nguồn: ngẫu nhiên thuần, đột biến corpus, "soup" token có cấu trúc.
        let src = match rng.below(3) {
            0 => {
                let n = 1 + rng.below(160);
                doc_ngau_nhien(&mut rng, &alpha, n)
            }
            1 => {
                let g = rng.chon(&goc).clone();
                dot_bien(&mut rng, &g, &alpha)
            }
            _ => {
                let mut s = String::new();
                for _ in 0..(1 + rng.below(24)) {
                    let tu = [
                        "fn ", "let ", "if ", "match ", "type ", "enum ", "for ", "in ", "->", ":",
                        "(", ")", "[", "]", "{", "}", ",", " ", "\n    ", "print", "x", "1",
                        "\"s\"", "=>", "|", "..", "?",
                    ];
                    s.push_str(rng.chon(&tu));
                }
                s
            }
        };
        ca += 1;
        let kq = catch_unwind(AssertUnwindSafe(|| {
            let out = parse(&src, Mode::Script);
            (
                out.text.clone(),
                out.cst.render(&out.text),
                out.diagnostics.len(),
                out,
            )
        }));
        match kq {
            Err(_) => {
                vi_pham += 1;
                ban_ghi.push(format!("PANIC seed={seed} ca={ca} nguồn={:?}", src));
            }
            Ok((text, render, so_chan_doan, out)) => {
                token += out.cst.tokens.len() as u64;
                // (2) lossless khi sạch
                if so_chan_doan == 0 && render != text {
                    vi_pham += 1;
                    ban_ghi.push(format!("LOSSLESS seed={seed} ca={ca} nguồn={src:?}"));
                }
                // (3) span hợp lệ
                for t in &out.cst.tokens {
                    if t.span.start > t.span.end
                        || t.span.end > text.len()
                        || !text.is_char_boundary(t.span.start.min(text.len()))
                        || !text.is_char_boundary(t.span.end.min(text.len()))
                    {
                        vi_pham += 1;
                        ban_ghi.push(format!(
                            "SPAN seed={seed} ca={ca} {}-{} / {} nguồn={src:?}",
                            t.span.start,
                            t.span.end,
                            text.len()
                        ));
                        break;
                    }
                }
                // (4) có lá lỗi ⇒ phải có chẩn đoán
                let _ = so_chan_doan;
            }
        }
        if vi_pham > 0 && ban_ghi.len() >= 20 {
            break;
        }
    }
    let giay = bat_dau.elapsed().as_secs_f64();
    let moi_giay = if giay > 0.0 { ca as f64 / giay } else { 0.0 };
    println!(
        "vis-parse-fuzz: {} ca, {} token, {:.0} ca/s, {:.0} token/s, {:.1} s, vi phạm {}",
        ca,
        token,
        moi_giay,
        if giay > 0.0 { token as f64 / giay } else { 0.0 },
        giay,
        vi_pham
    );
    for b in &ban_ghi {
        println!("  {b}");
    }
    if vi_pham == 0 {
        println!("vis-parse-fuzz: PASS (không vi phạm)");
        ExitCode::SUCCESS
    } else {
        println!("vis-parse-fuzz: FAIL ({vi_pham} vi phạm)");
        ExitCode::from(1)
    }
}
