// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis` — lệnh dòng ViScript. Vòng 1 (ST-02b) mới có `explain`.

use std::process::ExitCode;

use vis_diag::{all_codes, lookup_str, severity_label, Lang};
use vis_syntax::{parse, Mode};

fn usage() -> ExitCode {
    eprintln!(
        "dùng: vis explain [--lang vi|en|ja] <VSxxxx> | vis explain --list [--lang vi|en|ja] | vis check <tệp…> | vis fmt <tệp> | vis fmt --write <tệp…> | vis fmt --check <tệp…>"
    );
    ExitCode::from(2)
}

/// Ghi ra stdout **một lần**; `BrokenPipe` (vd `vis fmt … | head`) ⇒ im lặng thoát 0, không panic.
/// Lỗi I/O khác ⇒ báo stderr và trả `false` (caller thoát 2).
fn xuat_stdout(s: &str) -> bool {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    match w.write_all(s.as_bytes()).and_then(|()| w.flush()) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(0),
        Err(e) => {
            eprintln!("# không ghi được stdout: {e}");
            false
        }
    }
}

/// Escape ký tự điều khiển để chẩn đoán nằm gọn trên **một** dòng (TAP/stderr).
fn dong_chan_doan(code: &str, line: u32, col: u32, message: &str) -> String {
    format!(
        "{code} {line}:{col} {}",
        message
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
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
    // `.vish` = chế độ lệnh (ST-06, CmdCST); còn lại là script.
    let mode = if path.ends_with(".vish") {
        Mode::Command
    } else {
        Mode::Script
    };
    let out = parse(&src, mode);
    match chon_chan_doan(&out) {
        None => Ok(()),
        Some(d) => Err(dong_chan_doan(d.code_str(), d.line, d.col, &d.message)),
    }
}

fn usage_fmt() -> ExitCode {
    eprintln!("dùng: vis fmt <tệp> | vis fmt --write <tệp…> | vis fmt --check <tệp…>");
    ExitCode::from(2)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FmtMode {
    Stdout,
    Write,
    Check,
}

/// `vis fmt` — chuẩn hoá **thụt lề** (ST-07, VD-39). Thoát 0 (xong / đã đúng), 1 (`--check` có tệp
/// cần sửa), 2 (dùng sai, tệp lỗi cú pháp, `.vish`).
fn run_fmt(args: &[String]) -> ExitCode {
    let (mode, files): (FmtMode, &[String]) = match args.first().map(String::as_str) {
        None => return usage_fmt(),
        Some("--write") => (FmtMode::Write, &args[1..]),
        Some("--check") => (FmtMode::Check, &args[1..]),
        Some(f) if f.starts_with('-') => {
            eprintln!("vis fmt: không hỗ trợ cờ `{f}`");
            return usage_fmt();
        }
        Some(_) => {
            if args.len() != 1 {
                return usage_fmt();
            }
            (FmtMode::Stdout, args)
        }
    };
    if files.is_empty() {
        return usage_fmt();
    }
    let mut ra = String::new();
    if files.iter().any(|f| f.ends_with(".vish")) {
        // `vis fmt` **chưa có đặc tả** định dạng chế độ lệnh (ST-06 đã có CmdCST, nhưng định dạng
        // `.vish` chưa nằm trong phạm vi) — không im lặng bỏ qua, không coi như đã sửa.
        ra.push_str("Bail out! định dạng chế độ lệnh (.vish) chưa có đặc tả — chưa hỗ trợ\n");
        if !xuat_stdout(&ra) {
            return ExitCode::from(2);
        }
        return ExitCode::from(2);
    }
    let mut can_sua = false;
    let mut co_loi = false;
    for path in files {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("# không đọc được: {e}");
                co_loi = true;
                continue;
            }
        };
        match vis_fmt::format(&src) {
            Err(e) => {
                // (d) tệp lỗi: báo lỗi đầu, **không** ghi.
                eprintln!("# {}", dong_chan_doan(e.code, e.line, e.col, &e.message));
                co_loi = true;
            }
            Ok(moi) => match mode {
                FmtMode::Stdout => ra.push_str(&moi),
                FmtMode::Write => {
                    if moi == src {
                        ra.push_str(&format!("giữ nguyên {path}\n"));
                    } else if let Err(e) = std::fs::write(path, &moi) {
                        eprintln!("# không ghi được {path}: {e}");
                        co_loi = true;
                    } else {
                        can_sua = true;
                        ra.push_str(&format!("đã sửa {path}\n"));
                    }
                }
                FmtMode::Check => {
                    if moi != src {
                        can_sua = true;
                        ra.push_str(&format!("{path}\n"));
                    }
                }
            },
        }
    }
    if !xuat_stdout(&ra) {
        return ExitCode::from(2);
    }
    if co_loi {
        return ExitCode::from(2);
    }
    if mode == FmtMode::Check && can_sua {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
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
    let mut tap = String::new();
    tap.push_str("TAP version 13\n");
    tap.push_str(&format!("1..{}\n", args.len()));
    let mut failed = false;
    for (i, path) in args.iter().enumerate() {
        let n = i + 1;
        match check_one(path) {
            Ok(()) => tap.push_str(&format!("ok {n} - {path}\n")),
            Err(msg) => {
                failed = true;
                tap.push_str(&format!("not ok {n} - {path}\n# {msg}\n"));
            }
        }
    }
    if !xuat_stdout(&tap) {
        return ExitCode::from(2);
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
        Some("explain") => {
            let mut lang = Lang::Vi;
            let mut list = false;
            let mut ma: Option<String> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--list" => list = true,
                    "--lang" => match args.get(i + 1) {
                        Some(v) => match Lang::parse(v) {
                            Some(l) => {
                                lang = l;
                                i += 1; // bỏ qua giá trị ngôn ngữ
                            }
                            None => {
                                eprintln!("vis explain: ngôn ngữ không hỗ trợ `{v}` (vi|en|ja)");
                                return ExitCode::from(2);
                            }
                        },
                        None => {
                            eprintln!("vis explain: `--lang` cần giá trị vi|en|ja");
                            return ExitCode::from(2);
                        }
                    },
                    a if a.starts_with('-') => {
                        eprintln!("vis explain: không hỗ trợ cờ `{a}`");
                        return usage();
                    }
                    a => ma = Some(a.to_string()),
                }
                i += 1;
            }
            if list {
                // `explain --list` phủ **cả ba** sổ đã cấp (01xx + 02xx + 03xx, VD-44 #7).
                for c in all_codes() {
                    println!(
                        "{}  {:<8} {}",
                        c.short,
                        severity_label(c.severity, lang),
                        c.message(lang)
                    );
                }
                return ExitCode::SUCCESS;
            }
            match ma {
                None => usage(),
                Some(code) => match lookup_str(&code) {
                    Some(c) => {
                        println!(
                            "{} — {}: {}",
                            c.short,
                            severity_label(c.severity, lang),
                            c.message(lang)
                        );
                        ExitCode::SUCCESS
                    }
                    None => {
                        // VD-20: exit 1 dành cho lỗi từ main; mã sai là **lỗi dùng** ⇒ 2.
                        eprintln!("vis: không có mã {code} trong sổ (SPEC-lex §7)");
                        ExitCode::from(2)
                    }
                },
            }
        }
        Some("check") => run_check(&args[1..]),
        Some("fmt") => run_fmt(&args[1..]),
        _ => usage(),
    }
}
