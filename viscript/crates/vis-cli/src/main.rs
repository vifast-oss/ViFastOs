// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis` — lệnh dòng ViScript. Vòng 1 (ST-02b) mới có `explain`.

use std::process::ExitCode;

use vis_diag::{lookup_str, Severity, CODES, SYNTAX_CODES};

fn usage() -> ExitCode {
    eprintln!("dùng: vis explain <VSxxxx> | vis explain --list");
    ExitCode::from(2)
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
        _ => usage(),
    }
}
