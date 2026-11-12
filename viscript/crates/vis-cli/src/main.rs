// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis` — lệnh dòng ViScript. Vòng 1 (ST-02b) mới có `explain`.

use std::process::ExitCode;

use vis_diag::{lookup_str, Severity, CODES, SYNTAX_CODES};
use vis_syntax::{parse, Mode};

fn usage() -> ExitCode {
    eprintln!("dùng: vis explain <VSxxxx> | vis explain --list | vis check <tệp…>");
    ExitCode::from(2)
}

fn usage_check() -> ExitCode {
    eprintln!("dùng: vis check <tệp…>");
    ExitCode::from(2)
}

/// Chọn chẩn đoán **theo tầng** (VD-38 mục 8): có lỗi **từ vựng** (`01xx`) thì in cái đầu tiên
/// thuộc `01xx` (chuỗi token không còn tin được ⇒ lỗi parser chỉ là hệ quả); không có `01xx` mới
/// in cái đầu tiên thuộc `02xx` (cú pháp). Không dùng "vị trí sớm nhất": `let a = 1 let b = "x`
/// có VS0201 ở vị trí sớm hơn nhưng phải báo VS0101.
fn chon_chan_doan(out: &vis_syntax::ParseOutput) -> Option<&vis_syntax::Diagnostic> {
    let tang = |d: &&vis_syntax::Diagnostic, block: u16| d.code.id / 100 == block;
    out.diagnostics
        .iter()
        .find(|d| tang(d, 1))
        .or_else(|| out.diagnostics.iter().find(|d| tang(d, 2)))
        .or_else(|| out.diagnostics.first())
}

/// Kết quả kiểm **một** tệp: `Ok(())` sạch, `Err(<mã> <dòng>:<cột> <thông điệp>)` hoặc
/// `Err(không đọc được: …)`. Chỉ trả **một** chẩn đoán (lọc tầng như [`chon_chan_doan`]).
fn check_one(path: &str) -> Result<(), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("không đọc được: {e}"))?;
    let out = parse(&src, Mode::Script);
    match chon_chan_doan(&out) {
        None => Ok(()),
        Some(d) => Err(format!(
            "{} {}:{} {}",
            d.code_str(),
            d.line,
            d.col,
            // TAP là **một dòng** mỗi chẩn đoán: escape ký tự điều khiển (message có thể chứa
            // token NEWLINE/TAB thật, vd `mong xuống dòng…, thật `\n``).
            d.message
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t")
        )),
    }
}

/// `vis check <tệp…>` — in TAP 13. Thoát 0 (sạch) / 1 (có tệp hỏng) / 2 (dùng sai, `.vish`).
fn run_check(args: &[String]) -> ExitCode {
    if args.is_empty() {
        return usage_check();
    }
    if let Some(bad) = args.iter().find(|a| a.starts_with('-')) {
        eprintln!("vis check: không hỗ trợ cờ `{bad}`");
        return usage_check();
    }
    if args.iter().any(|a| a.ends_with(".vish")) {
        // Chế độ lệnh là ST-06: **không** im lặng bỏ qua, không giả vờ `ok`.
        println!("TAP version 13");
        println!("Bail out! chế độ lệnh (.vish) thuộc ST-06 — chưa hỗ trợ");
        return ExitCode::from(2);
    }
    println!("TAP version 13");
    println!("1..{}", args.len());
    let mut failed = false;
    for (i, path) in args.iter().enumerate() {
        let n = i + 1;
        match check_one(path) {
            Ok(()) => println!("ok {n} - {path}"),
            Err(msg) => {
                failed = true;
                println!("not ok {n} - {path}");
                println!("# {msg}");
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("explain") => match args.get(1).map(String::as_str) {
            None => usage(),
            Some("--list") => {
                for c in CODES.iter().chain(SYNTAX_CODES.iter()) {
                    let tag = match c.severity {
                        Severity::Error => "lỗi",
                        Severity::Warning => "cảnh báo",
                    };
                    println!("{}  {:<8} {}", c.short, tag, c.message_vi);
                }
                ExitCode::SUCCESS
            }
            Some(code) => match lookup_str(code) {
                Some(c) => {
                    let tag = match c.severity {
                        Severity::Error => "lỗi",
                        Severity::Warning => "cảnh báo",
                    };
                    println!("{} — {}: {}", c.short, tag, c.message_vi);
                    ExitCode::SUCCESS
                }
                None => {
                    // VD-20: exit 1 dành cho lỗi từ main; mã sai là **lỗi dùng** ⇒ 2.
                    eprintln!("vis: không có mã {code} trong sổ (SPEC-lex §7)");
                    ExitCode::from(2)
                }
            },
        },
        Some("check") => run_check(&args[1..]),
        _ => usage(),
    }
}
