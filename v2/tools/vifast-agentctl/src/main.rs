// SPDX-License-Identifier: MIT OR Apache-2.0
//! CLI mỏng của `vifast-agentctl`. Mặc định **chỉ đọc**; `done`/`block` cần `--write`.
//!
//! **KT-333d (D2-248 mục 2e)** — thêm `lint` (cổng quy trình: QUEUE + STATE) và `sync-queue`
//! (chỉ đồng bộ **số đếm** trong STATE theo QUEUE). Hai lệnh này có `--selftest` riêng (dựng
//! fixture trong bộ nhớ, KHÔNG cần repo thật) để `v2 t0 --selftest` gọi được mà không phụ thuộc
//! nội dung sổ đang thay đổi.

use std::fs;
use std::process::ExitCode;

use vifast_agentctl::{
    lint_queue, lint_state, parse_queue, parse_state, report_json, run_lint, startable,
    sync_state_counts, Error, Issue, PROJECTS,
};

fn queue_path(project: &str, root: &str) -> Result<String, Error> {
    if !PROJECTS.contains(&project) {
        return Err(Error::UnknownProject(project.to_string()));
    }
    Ok(format!("{root}/{project}/agent/QUEUE.md"))
}

fn state_path(project: &str, root: &str) -> String {
    format!("{root}/{project}/agent/STATE.md")
}

fn usage() -> ExitCode {
    eprintln!(
        "dùng: vifast-agentctl <next|list|report|start|done|block|lint|sync-queue> --project v2|viscript [--root .] [--write]"
    );
    eprintln!(
        "      vifast-agentctl lint [--strict] [--strict-state] [--no-state] [--selftest]\n\
         \x20     vifast-agentctl sync-queue [--dry-run] [--selftest]"
    );
    ExitCode::from(2)
}

/// HEAD ngắn của git tại `root` (rỗng nếu không đọc được — lint chỉ so khi có).
fn git_head(root: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn print_issues(issues: &[Issue], errors: usize) -> ExitCode {
    let mut warns = 0usize;
    for i in issues {
        if i.is_error() {
            eprintln!("- {i}");
        } else {
            warns += 1;
            println!("- {i}");
        }
    }
    if errors == 0 {
        println!("vifast-agentctl lint: PASS (0 lỗi, {warns} cảnh báo)");
        ExitCode::SUCCESS
    } else {
        eprintln!("vifast-agentctl lint: FAIL ({errors} lỗi, {warns} cảnh báo)");
        ExitCode::from(1)
    }
}

// ---------------------------------------------------------------- self-test (fixture trong RAM)

const Q_GOOD: &str = "\
| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|
| KT-01 | a | — | 1 h | DONE | x |
| KT-02 | b | KT-01 | 1 h | READY | x |
| KT-03 | c | KT-02 | 1 h | BLOCKED | x |
";

const S_GOOD: &str = "\
# STATE
```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-23 ===
HEAD          : `abc1234` (Chu kỳ 2). Cây sạch.
TASK          : **KT-01..KT-03 DONE** (1 task). READY: KT-02.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
";

fn has_code(issues: &[Issue], code: &str) -> bool {
    issues.iter().any(|i| i.code == code)
}

fn ca(ten: &str, ok: bool, chi_tiet: &str) -> bool {
    println!("{} - {ten}{}", if ok { "ok" } else { "not ok" }, chi_tiet);
    ok
}

/// ≥ 10 ca cho `lint` (fixture RAM — không đọc sổ thật, không phụ thuộc trạng thái repo).
fn lint_selftest() -> ExitCode {
    let mut ca_n = 0usize;
    let mut fails = 0usize;
    let mut run = |ten: &str, ok: bool, chi_tiet: &str| {
        ca_n += 1;
        if !ca(ten, ok, chi_tiet) {
            fails += 1;
        }
    };

    // (1) QUEUE + STATE hợp lệ ⇒ 0 lỗi.
    let (iss, err) = run_lint("v2", Q_GOOD, S_GOOD, Some("abc1234"), false, false, false);
    run(
        "(1) dương: QUEUE + STATE hợp lệ ⇒ 0 lỗi",
        err == 0,
        &format!(" ({iss:?})"),
    );

    // (2) mã sai trên dòng READY ⇒ E_QUEUE_ID.
    let q = Q_GOOD.replace("| KT-02 |", "| KT-2X |");
    let iss = lint_queue("v2", &q);
    run(
        "(2) âm: mã task sai mẫu (READY) ⇒ E_QUEUE_ID",
        has_code(&iss, "E_QUEUE_ID"),
        "",
    );

    // (3) trùng mã ⇒ E_QUEUE_DUP.
    let q = Q_GOOD.replace("| KT-03 | c |", "| KT-01 | c |");
    let iss = lint_queue("v2", &q);
    run(
        "(3) âm: trùng mã ⇒ E_QUEUE_DUP",
        has_code(&iss, "E_QUEUE_DUP"),
        "",
    );

    // (4) trạng thái lạ ⇒ E_QUEUE_STATUS.
    let q = Q_GOOD.replace("| 1 h | READY |", "| 1 h | WIP |");
    let iss = lint_queue("v2", &q);
    run(
        "(4) âm: trạng thái WIP ⇒ E_QUEUE_STATUS",
        has_code(&iss, "E_QUEUE_STATUS"),
        "",
    );

    // (5) READY thiếu dep DONE ⇒ E_QUEUE_READY_DEP.
    let q = Q_GOOD.replace(
        "| KT-01 | a | — | 1 h | DONE |",
        "| KT-01 | a | — | 1 h | REVIEW |",
    );
    let iss = lint_queue("v2", &q);
    run(
        "(5) âm: READY có dep chưa DONE ⇒ E_QUEUE_READY_DEP",
        has_code(&iss, "E_QUEUE_READY_DEP"),
        "",
    );

    // (6) BLOCKED đủ dep DONE ⇒ E_QUEUE_BLOCKED_READY.
    let q = "\
| KT-01 | a | — | 1 h | DONE | x |
| KT-02 | b | KT-01 | 1 h | BLOCKED | x |
";
    let iss = lint_queue("v2", q);
    run(
        "(6) âm: BLOCKED nhưng đủ dep DONE ⇒ E_QUEUE_BLOCKED_READY",
        has_code(&iss, "E_QUEUE_BLOCKED_READY"),
        "",
    );

    // (7) vòng lặp ⇒ E_QUEUE_CYCLE.
    let q = "\
| KT-01 | a | KT-02 | 1 h | READY | x |
| KT-02 | b | KT-01 | 1 h | READY | x |
";
    let iss = lint_queue("v2", q);
    run(
        "(7) âm: vòng lặp phụ thuộc ⇒ E_QUEUE_CYCLE",
        has_code(&iss, "E_QUEUE_CYCLE"),
        "",
    );

    // (8) dòng thiếu cột ⇒ E_QUEUE_COLUMNS.
    let q = "| KT-01 | a | 1 h | DONE |\n";
    let iss = lint_queue("v2", q);
    run(
        "(8) âm: dòng thiếu cột ⇒ E_QUEUE_COLUMNS",
        has_code(&iss, "E_QUEUE_COLUMNS"),
        "",
    );

    // (9) STATE thiếu khối ⇒ E_STATE_BLOCK.
    let iss = lint_state(Q_GOOD, "# STATE\nkhông có khối\n", Some("abc1234"), false);
    run(
        "(9) âm: STATE thiếu khối hiện tại ⇒ E_STATE_BLOCK",
        has_code(&iss, "E_STATE_BLOCK"),
        "",
    );

    // (10) số DONE lệch ⇒ W (mặc định) và E khi --strict-state.
    let s = S_GOOD.replace("(1 task)", "(5 task)");
    let w = lint_state(Q_GOOD, &s, Some("abc1234"), false);
    let e = lint_state(Q_GOOD, &s, Some("abc1234"), true);
    run(
        "(10) lệch số DONE ⇒ W_STATE_COUNT (mặc định) / E_STATE_COUNT (--strict-state)",
        has_code(&w, "W_STATE_COUNT")
            && has_code(&e, "E_STATE_COUNT")
            && !has_code(&w, "E_STATE_COUNT"),
        "",
    );

    // (11) HEAD lệch ⇒ W_STATE_HEAD_DRIFT (mặc định).
    let w = lint_state(Q_GOOD, S_GOOD, Some("def5678"), false);
    run(
        "(11) HEAD lệch ⇒ W_STATE_HEAD_DRIFT (cảnh báo)",
        has_code(&w, "W_STATE_HEAD_DRIFT"),
        "",
    );

    // (12) state đọc được HEAD/khoảng/số đếm (parser).
    let info = parse_state(S_GOOD);
    run(
        "(12) dương: parse_state đọc HEAD + khoảng + (N task)",
        info.head.as_deref() == Some("abc1234")
            && info.range == Some(("KT-01".to_string(), "KT-03".to_string()))
            && info.declared_done == Some(1),
        &format!(" ({info:?})"),
    );

    println!();
    if fails > 0 {
        eprintln!("vifast-agentctl lint --selftest: FAIL ({fails}/{ca_n})");
        return ExitCode::from(1);
    }
    println!("vifast-agentctl lint --selftest: PASS ({ca_n}/{ca_n})");
    ExitCode::SUCCESS
}

/// ≥ 6 ca cho `sync-queue`.
fn sync_selftest() -> ExitCode {
    let mut ca_n = 0usize;
    let mut fails = 0usize;
    let mut run = |ten: &str, ok: bool, chi_tiet: &str| {
        ca_n += 1;
        if !ca(ten, ok, chi_tiet) {
            fails += 1;
        }
    };
    let q = "\
| KT-01 | a | — | 1 h | DONE | x |
| KT-02 | b | KT-01 | 1 h | DONE | x |
| KT-03 | c | KT-02 | 1 h | READY | x |
";
    // (1) khoảng: `(1 task)` → đếm DONE trong KT-01..KT-03 = 2.
    let s = S_GOOD.to_string();
    let out = sync_state_counts(&s, q).expect("phải có mẫu đếm");
    run(
        "(1) khoảng KT-01..KT-03: (1 task) → (2 task)",
        out.contains("(2 task)") && !out.contains("(1 task)"),
        "",
    );
    // (2) không khoảng ⇒ tổng DONE.
    let s2 = S_GOOD.replace("**KT-01..KT-03 DONE** (1 task)", "**TỔNG** (1 task)");
    let out2 = sync_state_counts(&s2, q).expect("phải có mẫu đếm");
    run(
        "(2) không khoảng ⇒ đếm tổng DONE của QUEUE",
        out2.contains("(2 task)"),
        "",
    );
    // (3) đã đúng ⇒ KHÔNG đổi byte nào.
    let s3 = S_GOOD.replace("(1 task)", "(2 task)");
    let out3 = sync_state_counts(&s3, q).expect("phải có mẫu đếm");
    run("(3) đã đúng ⇒ không đổi byte nào", out3 == s3, "");
    // (4) văn xuôi + số khác trên cùng dòng giữ nguyên.
    let s4 = S_GOOD.replace(
        "READY: KT-02.",
        "READY: KT-02; 104 task khác trong lịch sử; 12 BLOCKED.",
    );
    let out4 = sync_state_counts(&s4, q).expect("phải có mẫu đếm");
    run(
        "(4) chỉ sửa '(N task)', giữ mọi số khác trong văn xuôi",
        out4.contains("(2 task)") && out4.contains("104 task khác") && out4.contains("12 BLOCKED"),
        "",
    );
    // (5) dòng HEAD không bị sửa.
    let out5 = sync_state_counts(&s, q).expect("phải có mẫu đếm");
    run(
        "(5) dòng HEAD giữ nguyên (sync-queue chỉ sửa số đếm)",
        out5.contains("HEAD          : `abc1234`"),
        "",
    );
    // (6) không có mẫu `(N task)` ⇒ None (không sửa gì).
    let s6 = S_GOOD.replace("(1 task)", "(một task)");
    run(
        "(6) âm: thiếu mẫu '(N task)' ⇒ None (không sửa gì)",
        sync_state_counts(&s6, q).is_none(),
        "",
    );
    // (7) ngoài khối hiện tại (dòng lịch sử) không bị chạm.
    let s7 = format!("{}\nTASK : lịch sử (99 task) ngoài khối.\n", S_GOOD);
    let out7 = sync_state_counts(&s7, q).expect("phải có mẫu đếm");
    run(
        "(7) dòng 'TASK' NGOÀI khối hiện tại không bị sửa",
        out7.contains("lịch sử (99 task)") && out7.contains("(2 task)"),
        "",
    );

    println!();
    if fails > 0 {
        eprintln!("vifast-agentctl sync-queue --selftest: FAIL ({fails}/{ca_n})");
        return ExitCode::from(1);
    }
    println!("vifast-agentctl sync-queue --selftest: PASS ({ca_n}/{ca_n})");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return usage();
    }
    let cmd = args[0].clone();
    let mut project = String::new();
    let mut root = ".".to_string();
    let mut write = false;
    let mut strict = false;
    let mut strict_state = false;
    let mut no_state = false;
    let mut dry_run = false;
    let mut selftest = false;
    let mut state_override: Option<String> = None;
    let mut queue_override: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--project" if i + 1 < args.len() => {
                project = args[i + 1].clone();
                i += 2;
            }
            "--root" if i + 1 < args.len() => {
                root = args[i + 1].clone();
                i += 2;
            }
            "--state" if i + 1 < args.len() => {
                state_override = Some(args[i + 1].clone());
                i += 2;
            }
            "--queue" if i + 1 < args.len() => {
                queue_override = Some(args[i + 1].clone());
                i += 2;
            }
            "--write" => {
                write = true;
                i += 1;
            }
            "--strict" => {
                strict = true;
                i += 1;
            }
            "--strict-state" => {
                strict_state = true;
                i += 1;
            }
            "--no-state" => {
                no_state = true;
                i += 1;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "--selftest" => {
                selftest = true;
                i += 1;
            }
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }

    // `lint`/`sync-queue` mặc định dự án v2 (cổng quy trình của dòng OS); lệnh khác vẫn bắt buộc.
    if project.is_empty() && (cmd == "lint" || cmd == "sync-queue") {
        project = "v2".to_string();
    }
    if selftest {
        return match cmd.as_str() {
            "lint" => lint_selftest(),
            "sync-queue" => sync_selftest(),
            _ => {
                eprintln!("agentctl: --selftest chỉ có cho 'lint' và 'sync-queue'");
                ExitCode::from(2)
            }
        };
    }
    if project.is_empty() {
        eprintln!("thiếu --project v2|viscript");
        return ExitCode::from(2);
    }

    // Đường dẫn: có thể ép bằng --queue/--state (dùng cho chẩn đoán; mặc định = sổ thật).
    let qpath = match &queue_override {
        Some(p) => p.clone(),
        None => match queue_path(&project, &root) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("agentctl: {e}");
                return ExitCode::from(2);
            }
        },
    };
    let spath = match &state_override {
        Some(p) => p.clone(),
        None => state_path(&project, &root),
    };
    let text = match fs::read_to_string(&qpath) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("agentctl: {}", Error::MissingFile(qpath.clone()));
            return ExitCode::from(1);
        }
    };
    let queue = parse_queue(&qpath, &text);

    match cmd.as_str() {
        "next" => match queue.next() {
            Some((t, note)) => {
                println!("{}", t.id);
                if let Some(n) = note {
                    eprintln!("chú ý: {n}");
                }
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("agentctl: không còn task READY có phụ thuộc DONE");
                ExitCode::from(1)
            }
        },
        "list" => {
            for t in &queue.tasks {
                println!("{:<8} {:<8} {}", t.id, t.status, t.deps);
            }
            ExitCode::SUCCESS
        }
        "report" => {
            println!("{}", report_json(&project, &queue));
            ExitCode::SUCCESS
        }
        "start" => {
            let Some(id) = positional.first() else {
                return usage();
            };
            match queue.find(id) {
                Some(t) if startable(t) => {
                    println!(
                        "bắt đầu {} ({}, {}). Bước tiếp: đọc STATE, mở review, viết test đỏ trước.",
                        t.id, t.status, t.budget
                    );
                    ExitCode::SUCCESS
                }
                Some(t) => {
                    eprintln!(
                        "agentctl: {} đang {} — chỉ start task READY (AGENTS.md §10)",
                        t.id, t.status
                    );
                    ExitCode::from(2)
                }
                None => {
                    eprintln!("agentctl: {}", Error::UnknownTask(id.clone()));
                    ExitCode::from(1)
                }
            }
        }
        "done" | "block" => {
            let Some(id) = positional.first() else {
                return usage();
            };
            if !write {
                eprintln!("agentctl: cần --write (sổ QUEUE thuộc Agent 0 — AGENTS.md §3)");
                return ExitCode::from(2);
            }
            if cmd == "block" {
                if positional.len() < 2 {
                    eprintln!("agentctl: block cần lý do: block <id> <lý do>");
                    return ExitCode::from(2);
                }
                eprintln!("lý do ghi kèm: {}", positional[1]);
            }
            let status = if cmd == "done" { "DONE" } else { "BLOCKED" };
            match queue.set_status(&text, id, status) {
                Ok(out) => match fs::write(&qpath, out) {
                    Ok(()) => {
                        println!("{id} → {status} trong {qpath}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("agentctl: không ghi được {qpath}: {e}");
                        ExitCode::from(1)
                    }
                },
                Err(e) => {
                    eprintln!("agentctl: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "lint" => {
            let state_text = fs::read_to_string(&spath).unwrap_or_default();
            let head = git_head(&root);
            let (issues, errors) = run_lint(
                &project,
                &text,
                &state_text,
                head.as_deref(),
                strict,
                strict_state,
                no_state,
            );
            print_issues(&issues, errors)
        }
        "sync-queue" => {
            let state_text = match fs::read_to_string(&spath) {
                Ok(t) => t,
                Err(_) => {
                    eprintln!("agentctl: {}", Error::MissingFile(spath.clone()));
                    return ExitCode::from(1);
                }
            };
            match sync_state_counts(&state_text, &text) {
                None => {
                    println!(
                        "vifast-agentctl sync-queue: không có mẫu đếm '(N task)' trên dòng TASK — không sửa gì (STATE.md thuộc Agent 0/lead)."
                    );
                    ExitCode::SUCCESS
                }
                Some(new) if new == state_text => {
                    println!("vifast-agentctl sync-queue: số đếm đã khớp QUEUE — không đổi");
                    ExitCode::SUCCESS
                }
                Some(new) => {
                    if dry_run {
                        for (a, b) in state_text.lines().zip(new.lines()) {
                            if a != b {
                                println!("- {a}");
                                println!("+ {b}");
                            }
                        }
                        println!("vifast-agentctl sync-queue: --dry-run — KHÔNG ghi {spath}");
                        ExitCode::SUCCESS
                    } else {
                        match fs::write(&spath, new) {
                            Ok(()) => {
                                println!(
                                    "vifast-agentctl sync-queue: đã đồng bộ số đếm trong {spath} (chỉ dòng TASK)"
                                );
                                ExitCode::SUCCESS
                            }
                            Err(e) => {
                                eprintln!("agentctl: không ghi được {spath}: {e}");
                                ExitCode::from(1)
                            }
                        }
                    }
                }
            }
        }
        _ => usage(),
    }
}
