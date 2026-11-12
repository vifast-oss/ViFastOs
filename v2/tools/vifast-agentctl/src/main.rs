// SPDX-License-Identifier: MIT OR Apache-2.0
//! CLI mỏng của `vifast-agentctl`. Mặc định **chỉ đọc**; `done`/`block` cần `--write`.

use std::fs;
use std::process::ExitCode;

use vifast_agentctl::{parse_queue, report_json, Error, PROJECTS};

fn queue_path(project: &str, root: &str) -> Result<String, Error> {
    if !PROJECTS.contains(&project) {
        return Err(Error::UnknownProject(project.to_string()));
    }
    Ok(format!("{root}/{project}/agent/QUEUE.md"))
}

fn load(project: &str, root: &str) -> Result<(String, String), Error> {
    let path = queue_path(project, root)?;
    let text = fs::read_to_string(&path).map_err(|_| Error::MissingFile(path.clone()))?;
    Ok((path, text))
}

fn usage() -> ExitCode {
    eprintln!(
        "dùng: vifast-agentctl <next|list|report|start|done|block> --project v2|viscript [--root .] [--write]"
    );
    ExitCode::from(2)
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
            "--write" => {
                write = true;
                i += 1;
            }
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }
    if project.is_empty() {
        eprintln!("thiếu --project v2|viscript");
        return ExitCode::from(2);
    }

    let (path, text) = match load(&project, &root) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("agentctl: {e}");
            return ExitCode::from(1);
        }
    };
    let queue = parse_queue(&path, &text);

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
                Some(t) => {
                    println!(
                        "bắt đầu {} ({}, {}). Bước tiếp: đọc STATE, mở review, viết test đỏ trước.",
                        t.id, t.status, t.budget
                    );
                    ExitCode::SUCCESS
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
            let status = if cmd == "done" { "DONE" } else { "BLOCKED" };
            match queue.set_status(&text, id, status) {
                Ok(out) => match fs::write(&path, out) {
                    Ok(()) => {
                        println!("{id} → {status} trong {path}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("agentctl: không ghi được {path}: {e}");
                        ExitCode::from(1)
                    }
                },
                Err(e) => {
                    eprintln!("agentctl: {e}");
                    ExitCode::from(1)
                }
            }
        }
        _ => usage(),
    }
}
