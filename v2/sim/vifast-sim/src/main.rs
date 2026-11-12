// SPDX-License-Identifier: MIT OR Apache-2.0
//! CLI của sim: in trace + hash (KT-08); `--check` chạy hai lần cùng seed để chứng minh tất định.

use std::process::ExitCode;

use vifast_sim::Harness;

fn parse_u64(s: &str) -> Option<u64> {
    s.parse::<u64>().ok()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 0u64;
    let mut steps = 64u64;
    let mut tasks = 4u32;
    let mut cap = 4096usize;
    let mut check = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" if i + 1 < args.len() => {
                seed = parse_u64(&args[i + 1]).unwrap_or(0);
                i += 2;
            }
            "--steps" if i + 1 < args.len() => {
                steps = parse_u64(&args[i + 1]).unwrap_or(64);
                i += 2;
            }
            "--tasks" if i + 1 < args.len() => {
                tasks = parse_u64(&args[i + 1]).unwrap_or(4) as u32;
                i += 2;
            }
            "--cap" if i + 1 < args.len() => {
                cap = parse_u64(&args[i + 1]).unwrap_or(4096) as usize;
                i += 2;
            }
            "--check" => {
                check = true;
                i += 1;
            }
            other => {
                eprintln!("vifast-sim: tham số lạ {other}");
                return ExitCode::from(2);
            }
        }
    }

    let mut a = Harness::new(seed, cap, tasks);
    a.run(steps);
    if check {
        let mut b = Harness::new(seed, cap, tasks);
        b.run(steps);
        if a.trace_lines() != b.trace_lines() || a.hash() != b.hash() {
            eprintln!("vifast-sim: KHÔNG tất định ở seed {seed}");
            return ExitCode::from(1);
        }
        println!(
            "# determinism OK seed={seed} steps={steps} tasks={tasks} hash={:016x}",
            a.hash()
        );
        return ExitCode::SUCCESS;
    }
    for line in a.trace_lines() {
        println!("{line}");
    }
    println!("# trace-hash={:016x}", a.hash());
    ExitCode::SUCCESS
}
