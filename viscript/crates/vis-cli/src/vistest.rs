// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`vis test`** (ST-30) — chạy bộ `test/sem/**` (và mọi thư mục được trỏ) in **TAP**.
//!
//! Quy ước bộ ca (giống harness `crates/vis-cli/tests/sem.rs`):
//! - `<tên>.vis`/`.vish` **không** có `<tên>.err` ⇒ phải chạy **exit 0** và `stdout` **byte-khớp** `.out`
//!   (bỏ dòng SPDX đầu của `.out`).
//! - có `<tên>.err` ⇒ phải **đỏ** đúng mã trong `.err` (`VS05xx` ⇒ exit 1, còn lại ⇒ exit 2).
//!
//! `--vm` chạy **đường VM** (`--no-cache`); không có `--vm` thì chạy interp (mặc định `vis run`, VD-56).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use vis_syntax::{Kind, Mode, NodeKind, Span};

fn usage() -> ExitCode {
    eprintln!("dùng: vis test [--vm] [--strict-caps] [thư mục|tệp]");
    ExitCode::from(2)
}

/// Bỏ dòng SPDX đầu của tệp kỳ vọng (QD §9.3 — fixture `.out` có header).
fn bo_spdx(s: &str) -> &str {
    match s.split_once('\n') {
        Some((d, rest)) if d.trim_start().starts_with('#') && d.contains("SPDX") => rest,
        _ => s,
    }
}

/// Tìm `test/sem` từ cwd leo lên (chạy được cả trong `crates/vis-cli` và ở gốc repo).
fn tim_mac_dinh() -> Option<PathBuf> {
    let mut d = std::env::current_dir().ok()?;
    loop {
        for ten in ["test/sem", "viscript/test/sem"] {
            let p = d.join(ten);
            if p.is_dir() {
                return Some(p);
            }
        }
        if !d.pop() {
            return None;
        }
    }
}

fn gom(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut ds: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    ds.sort();
    for p in ds {
        if p.is_dir() {
            gom(&p, out);
        } else if matches!(
            p.extension().and_then(|x| x.to_str()),
            Some("vis") | Some("vish")
        ) {
            out.push(p);
        }
    }
}

/// **ST-79 (VD-108)**: tên + span của mọi `test fn` trong tệp, theo thứ tự khai báo (CST).
fn ds_test(text: &str) -> Vec<(String, Span, Span)> {
    let out = vis_syntax::parse(text, Mode::Script);
    let cst = &out.cst;
    let mut ds = Vec::new();
    for n in cst.root.children() {
        if n.kind != NodeKind::TestDecl {
            continue;
        }
        let toks: Vec<&vis_syntax::Token> = n.tokens(&cst.tokens).collect();
        let mut idents = toks.iter().filter(|t| t.kind == Kind::Ident);
        let Some(test_tok) = idents.next() else {
            continue;
        };
        let Some(name_tok) = idents.next() else {
            continue;
        };
        ds.push((name_tok.text.clone(), test_tok.span, name_tok.span));
    }
    ds
}

/// Đổi `test fn <tên>` thành `fn main` (chỉ tên đang chọn); giữ nguyên phần còn lại.
fn thanh_main(text: &str, test_span: Span, name_span: Span) -> String {
    let mut s = String::with_capacity(text.len());
    s.push_str(&text[..test_span.start]);
    s.push_str("fn main");
    s.push_str(&text[name_span.end..]);
    s
}

fn chay(ca: &Path, vm: bool, strict: bool) -> (i32, String, String) {
    let mut c = Command::new(std::env::current_exe().unwrap_or_else(|_| PathBuf::from("vis")));
    c.arg("run").arg("--tap");
    if strict {
        c.arg("--strict-caps");
    }
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    c.arg(ca);
    match c.output() {
        Ok(o) => (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
        ),
        Err(e) => (-1, String::new(), format!("không chạy được: {e}")),
    }
}

/// Mã chẩn đoán đầu trong stderr: `(mã, dòng, cột, thông điệp)` — dòng stderr có dạng
/// `tệp:dòng:cột: error VSxxxx: thông điệp`.
fn chan_doan(stderr: &str) -> Option<(String, u32, u32, String)> {
    for l in stderr.lines() {
        if let Some(i) = l.find("VS") {
            let ma: String = l[i..].chars().take(6).collect();
            // Vị trí: hai trường số ngay trước `: error`.
            let truoc = l[..i]
                .trim_end_matches("error")
                .trim_end_matches(':')
                .trim_end();
            let mut so: Vec<u32> = truoc
                .rsplit(':')
                .take(2)
                .filter_map(|x| x.trim().parse::<u32>().ok())
                .collect();
            so.reverse();
            let (dong, cot) = match so.as_slice() {
                [d, c] => (*d, *c),
                [d] => (*d, 1),
                _ => (1, 1),
            };
            let msg = l[i..]
                .split_once(':')
                .map(|(_, m)| m.trim().to_string())
                .unwrap_or_default();
            return Some((ma, dong, cot, msg));
        }
    }
    None
}

pub fn run(args: &[String]) -> ExitCode {
    let mut vm = false;
    let mut strict = false;
    let mut duong: Option<PathBuf> = None;
    for a in args {
        match a.as_str() {
            "--vm" => vm = true,
            "--strict-caps" => strict = true,
            "-h" | "--help" => return usage(),
            s if s.starts_with('-') => {
                eprintln!("vis test: không hỗ trợ cờ `{s}`");
                return usage();
            }
            s => duong = Some(PathBuf::from(s)),
        }
    }
    let goc = match duong {
        Some(p) => {
            if !p.exists() {
                eprintln!("vis test: không có đường dẫn `{}`", p.display());
                return ExitCode::from(2);
            }
            p
        }
        None => {
            match tim_mac_dinh() {
                Some(p) => p,
                None => {
                    eprintln!("vis test: không tìm thấy `test/sem` (chạy ở gốc repo hoặc truyền đường dẫn)");
                    return ExitCode::from(2);
                }
            }
        }
    };
    let mut files: Vec<PathBuf> = Vec::new();
    if goc.is_dir() {
        gom(&goc, &mut files);
    } else {
        files.push(goc.clone());
    }
    // **ST-79 (VD-108)**: tệp có `test fn` ⇒ **mỗi test một ca TAP** (chạy bằng bản sao đổi
    // `test fn <tên>` → `fn main`); tệp thường giữ nguyên hành vi fixture `.out`/`.err`.
    let mut ca: Vec<(PathBuf, Option<(String, String)>)> = Vec::new();
    for p in files {
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let tests = ds_test(&text);
        if tests.is_empty() {
            ca.push((p, None));
        } else {
            for (ten, test_span, name_span) in tests {
                let src = thanh_main(&text, test_span, name_span);
                ca.push((p.clone(), Some((ten, src))));
            }
        }
    }
    let mut tap = String::new();
    tap.push_str("TAP version 13\n");
    tap.push_str(&format!("1..{}\n", ca.len()));
    let mut hong = 0usize;
    for (i, (p, bai_test)) in ca.iter().enumerate() {
        let n = i + 1;
        let ten = p
            .strip_prefix(&goc)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        if let Some((ten_test, src)) = bai_test {
            // Ghi bản sao tạm, chạy `vis run` (interp hoặc VM), TAP theo **tên test**.
            let d = std::env::temp_dir().join(format!("vis-test-{}-{}", std::process::id(), i));
            let _ = std::fs::remove_dir_all(&d);
            let _ = std::fs::create_dir_all(&d);
            let tep = d.join("t.vis");
            if let Err(e) = std::fs::write(&tep, src) {
                hong += 1;
                tap.push_str(&format!(
                    "not ok {n} - {ten_test}\n# không ghi được bản sao: {e}\n"
                ));
                continue;
            }
            let mut c =
                Command::new(std::env::current_exe().unwrap_or_else(|_| PathBuf::from("vis")));
            c.arg("run");
            if strict {
                c.arg("--strict-caps");
            }
            if vm {
                c.arg("--vm").arg("--no-cache");
            }
            c.arg(&tep);
            let (code, _out, stderr) = match c.output() {
                Ok(o) => (
                    o.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&o.stdout).to_string(),
                    String::from_utf8_lossy(&o.stderr).to_string(),
                ),
                Err(e) => (-1, String::new(), format!("không chạy được: {e}")),
            };
            let _ = std::fs::remove_dir_all(&d);
            if code == 0 {
                tap.push_str(&format!("ok {n} - {ten_test}\n"));
            } else {
                hong += 1;
                tap.push_str(&format!("not ok {n} - {ten_test}\n"));
                let (ma, dong, cot, msg) =
                    chan_doan(&stderr).unwrap_or(("VS?".into(), 1, 1, String::new()));
                tap.push_str(&format!("# {ten_test}:{dong}:{cot}: {ma} {msg}\n"));
            }
            continue;
        }
        let err = p.with_extension("err");
        let (code, out, stderr) = chay(p, vm, strict);
        if err.exists() {
            let mong = std::fs::read_to_string(&err).unwrap_or_default();
            // `.err` có thể có header SPDX: lấy **dòng đầu không phải chú thích**.
            let mong = mong
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .unwrap_or("");
            let muon = if mong.starts_with("VS05") { 1 } else { 2 };
            // ST-30b: hai đường **đã đồng bộ** chẩn đoán ⇒ `--vm` khoá **đúng mã** như interp.
            let ok = code == muon && stderr.contains(mong);
            if ok {
                tap.push_str(&format!("ok {n} - {ten}\n"));
            } else {
                hong += 1;
                tap.push_str(&format!("not ok {n} - {ten}\n"));
                let (ma, dong, cot, msg) =
                    chan_doan(&stderr).unwrap_or((mong.to_string(), 1, 1, String::new()));
                tap.push_str(&format!("# {ten}:{dong}:{cot}: {ma} {msg}\n"));
            }
            continue;
        }
        let want_path = p.with_extension("out");
        let want = std::fs::read_to_string(&want_path).unwrap_or_default();
        let want = bo_spdx(&want);
        if code == 0 && out == want {
            tap.push_str(&format!("ok {n} - {ten}\n"));
        } else {
            hong += 1;
            tap.push_str(&format!("not ok {n} - {ten}\n"));
            if code != 0 {
                let (ma, dong, cot, msg) =
                    chan_doan(&stderr).unwrap_or(("VS?".into(), 1, 1, String::new()));
                tap.push_str(&format!("# {ten}:{dong}:{cot}: {ma} {msg}\n"));
            } else {
                tap.push_str(&format!("# {ten}:1:1: VS0000 stdout khác kỳ vọng\n"));
            }
        }
    }
    print!("{tap}");
    if hong > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `vis test` chỉ dùng ở chế độ script/lệnh — giữ tham chiếu để `Mode` không bị bỏ (tài liệu hoá).
#[allow(dead_code)]
fn _mode(_: Mode) {}
