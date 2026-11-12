// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-lex-fuzz` — fuzz 10 phút của ST-04 (QD §7.5), chạy tay, **không** phải một phần
//! `gate --fast` (fuzz dài là tiêu chí task, không phải cổng thường ngày — QUEUE Khối D).
//!
//! ```text
//! cargo run --release -p vis-syntax --bin vis-lex-fuzz -- --seconds 600 --seed 20261112
//! ```
//!
//! Sinh đầu vào theo ba nguồn: chuỗi ngẫu nhiên từ bảng ký tự "hiểm", đột biến của một corpus
//! mã thật, và dạng NFD của cả hai (để kiểm NFC idempotent). Mỗi ca chạy **cả hai chế độ** và
//! kiểm bộ bất biến của [`vis_syntax::lex::verify`]; ca hỏng được in kèm seed + nguồn đã escape
//! để tái lập được, rồi thoát mã 1.

use std::collections::BTreeSet;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use unicode_normalization::UnicodeNormalization;
use vis_syntax::lex::verify;
use vis_syntax::{lex, Mode};

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
}

/// Ký tự "hiểm": metachar, dấu nháy, tiền tố chuỗi/số, chữ Việt, dấu tổ hợp, điều khiển.
fn alphabet() -> Vec<char> {
    let mut v: Vec<char> = Vec::new();
    for c in [
        'a', 'z', 'A', 'Z', '_', '0', '1', '7', '9', 'x', 'b', 'r', 'p', 'e', 's', 'm', 'h', 'd',
        'k', 'i', 'c', 't', 'u', 'w', 'n', 'g', 'f', 'E', '+', '-', '*', '/', '%', '.', ',', ':',
        ';', '(', ')', '[', ']', '{', '}', '<', '>', '=', '!', '?', '&', '|', '^', '~', '@', '#',
        '$', '\\', '"', '\'', '`', ' ', '\t', '\n', '\r', '\u{0}', '\u{7f}', '\u{feff}',
    ] {
        v.push(c);
    }
    for c in ['à', 'ố', 'ệ', 'ự', 'ỹ', 'đ', 'Đ', 'ư', 'ơ', 'ă', 'â'] {
        v.push(c);
    }
    for c in [
        '\u{300}', '\u{301}', '\u{302}', '\u{303}', '\u{306}', '\u{31b}', '\u{323}',
    ] {
        v.push(c);
    }
    v.push('😀');
    v.push('中');
    v
}

/// Corpus mã thật — nguồn đột biến (giữ được "hình dạng" cú pháp).
const CORPUS: &[&str] = &[
    "let x = 0xff + 2h30m",
    "if x:\n    y = 5kb\nelse:\n    y = 250ms\n",
    "fn f(a: int) -> int:\n    return a * 2\n",
    "set{1, 2} == set {3}",
    "\"a{x:>8}b\" p\"./a.txt\" r\"C:\\\\raw\"",
    "b\"\\x00\\x01\" \"\"\"a\\nb\"\"\" r\"\"\"thô\"\"\"",
    "'ố' 'a' 'ab'",
    "x??y x?.y f()? a|b a|>b 2**3 x |> f",
    "1..2 0..=10 1.2..3.4 1e-9 50pct 12.5pct",
    "## doc\n# c\nlet _tmp = 1_000\n",
    "echo \"a b\" 'c d' | grep -- -x",
    "$HOME $env.PATH",
    "ls -la; a & b > c < d",
    "https://x/y#frag",
    "^git status",
    "if x:\n\ty\n",
    "x\n|> f\ny",
];

/// In nguồn dạng escape được (ASCII, giữ nguyên dấu tiếng Việt).
fn escape(src: &str) -> String {
    let mut out = String::new();
    for c in src.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{feff}' => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

/// Một ca hỏng: in đủ để tái lập rồi thoát.
fn fail(seed: u64, case: u64, mode: Mode, src: &str, bad: &[String]) -> ExitCode {
    println!("FUZZ HỎNG — seed={seed} ca={case} mode={mode:?}");
    println!("nguồn: \"{}\"", escape(src));
    println!("byte : {:?}", src.as_bytes());
    for b in bad {
        println!("  - {b}");
    }
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let mut seconds = 600u64;
    let mut seed = 20261112u64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seconds" => {
                if let Some(v) = args.next().and_then(|v| v.parse().ok()) {
                    seconds = v;
                }
            }
            "--seed" => {
                if let Some(v) = args.next().and_then(|v| v.parse().ok()) {
                    seed = v;
                }
            }
            "-h" | "--help" => {
                println!("dùng: vis-lex-fuzz [--seconds N] [--seed N]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("tham số lạ: {other}");
                return ExitCode::from(2);
            }
        }
    }

    let alpha = alphabet();
    let mut rng = Rng::new(seed);
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let started = Instant::now();
    let mut cases: u64 = 0;
    let mut tokens: u64 = 0;
    let mut codes = [0u64; 10];
    let mut seen_kinds: BTreeSet<&'static str> = BTreeSet::new();
    let mut last_report = Instant::now();

    // (không in số vi phạm của đầu vào rỗng — dễ đọc nhầm thành "0 bất biến được kiểm".)
    println!(
        "vis-lex-fuzz: seed={seed} thời lượng={seconds}s chế độ=script+lệnh \
         kiểm: span/vị trí/NFC idempotent/INDENT-DEDENT/mã chẩn đoán"
    );

    while Instant::now() < deadline {
        for _ in 0..256 {
            cases += 1;
            // 0: ngẫu nhiên, 1: đột biến corpus, 2: NFD của ca 0/1.
            let kind = rng.below(3);
            let base = if kind == 1 {
                let idx = rng.below(CORPUS.len());
                CORPUS[idx].to_string()
            } else {
                let len = rng.below(48);
                (0..len).map(|_| alpha[rng.below(alpha.len())]).collect()
            };
            let src = if kind == 2 {
                base.nfd().collect::<String>()
            } else if kind == 1 {
                mutate(&mut rng, &base, &alpha)
            } else {
                base
            };

            for mode in [Mode::Script, Mode::Command] {
                let out = lex(&src, mode);
                tokens += out.tokens.len() as u64;
                for t in &out.tokens {
                    seen_kinds.insert(t.kind.name());
                }
                for d in &out.diagnostics {
                    let idx = (d.code.id as usize).saturating_sub(101);
                    if let Some(slot) = codes.get_mut(idx) {
                        *slot += 1;
                    }
                }
                let bad = verify::violations(&out);
                if !bad.is_empty() {
                    return fail(seed, cases, mode, &src, &bad);
                }
                // NFC idempotent: dạng NFD của cùng văn bản phải ra cùng kết quả.
                let nfd: String = src.nfd().collect();
                let other = lex(&nfd, mode);
                if other != out {
                    return fail(
                        seed,
                        cases,
                        mode,
                        &src,
                        &[format!("NFC không idempotent (NFD cho {other:?})")],
                    );
                }
            }
        }
        if last_report.elapsed() >= Duration::from_secs(15) {
            last_report = Instant::now();
            let rate = cases as f64 / started.elapsed().as_secs_f64();
            println!(
                "  {:>5}s  ca={cases}  token={tokens}  {rate:.0} ca/s",
                started.elapsed().as_secs()
            );
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "vis-lex-fuzz: XONG seed={seed} ca={cases} token={tokens} {:.0} ca/s trong {elapsed:.1}s",
        cases as f64 / elapsed
    );
    let names = [
        "VS0101", "VS0102", "VS0103", "VS0104", "VS0105", "VS0106", "VS0107", "VS0108", "VS0109",
        "VS0110",
    ];
    let covered: Vec<String> = names
        .iter()
        .zip(codes)
        .map(|(n, c)| format!("{n}={c}"))
        .collect();
    println!("phủ mã lỗi: {}", covered.join(" "));
    println!(
        "phủ kind: {}/{} — {}",
        seen_kinds.len(),
        vis_syntax::Kind::ALL.len(),
        seen_kinds.iter().copied().collect::<Vec<_>>().join(" ")
    );
    ExitCode::SUCCESS
}

/// Đột biến một ca corpus: chèn/xoá/thay một đoạn ngắn ở biên ký tự.
fn mutate(rng: &mut Rng, base: &str, alpha: &[char]) -> String {
    let mut chars: Vec<char> = base.chars().collect();
    let rounds = 1 + rng.below(4);
    for _ in 0..rounds {
        if chars.is_empty() {
            chars.push(alpha[rng.below(alpha.len())]);
        }
        let at = rng.below(chars.len());
        match rng.below(3) {
            0 => {
                let n = 1 + rng.below(4);
                for _ in 0..n {
                    let c = alpha[rng.below(alpha.len())];
                    chars.insert(at, c);
                }
            }
            1 => {
                let n = (1 + rng.below(3)).min(chars.len() - at);
                chars.drain(at..at + n);
            }
            _ => {
                chars[at] = alpha[rng.below(alpha.len())];
            }
        }
    }
    chars.into_iter().collect()
}
