// SPDX-License-Identifier: MIT OR Apache-2.0
//! CLI của sim: in trace + hash (KT-08); `--check` chạy hai lần cùng seed để chứng minh tất định.

use std::process::ExitCode;

use vifast_sim::Harness;
use vifast_traceformat::{Record, KIND_IDLE, KIND_RUN};

/// Header dump `VFT1` (D2-40): magic 4 B + version u32 + count u32 + FNV-1a 64 của bản ghi.
const MAGIC: [u8; 4] = *b"VFT1";
const VERSION: u32 = 1;

fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Ghi dump nhị phân: header `VFT1` + bản ghi 32 B. `t_ns` = chỉ số step (thời gian logic của
/// sim; QEMU `pmemsave` cho `t_ns` thật) — cùng layout nên `vifast-trace` đọc chung.
fn write_trace_bin(path: &str, sim: &Harness) -> std::io::Result<()> {
    let mut body = Vec::new();
    for ev in sim.events() {
        let kind = match ev.kind {
            "run" => KIND_RUN,
            "idle" => KIND_IDLE,
            _ => 0,
        };
        let rec = Record::new(
            ev.step,
            0,
            kind,
            ev.task.unwrap_or(u32::MAX),
            ev.used as u64,
            ev.value,
        );
        body.extend_from_slice(&rec.to_bytes());
    }
    let mut out = Vec::with_capacity(20 + body.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&((body.len() / 32) as u32).to_le_bytes());
    out.extend_from_slice(&fnv1a64(&body).to_le_bytes());
    out.extend_from_slice(&body);
    std::fs::write(path, out)
}

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
    let mut trace_bin: Option<String> = None;
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
            "--trace-bin" if i + 1 < args.len() => {
                trace_bin = Some(args[i + 1].clone());
                i += 2;
            }
            other => {
                eprintln!("vifast-sim: tham số lạ {other}");
                return ExitCode::from(2);
            }
        }
    }

    let mut a = Harness::new(seed, cap, tasks);
    a.run(steps);
    if let Some(path) = trace_bin.as_deref() {
        if let Err(exc) = write_trace_bin(path, &a) {
            eprintln!("vifast-sim: không ghi được {path}: {exc}");
            return ExitCode::from(2);
        }
        println!(
            "# trace-bin={path} records={} header=VFT1v{VERSION}",
            a.events().len()
        );
    }
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
    for line in a.dump(steps) {
        println!("{line}");
    }
    println!("# trace-hash={:016x}", a.dump_hash(steps));
    ExitCode::SUCCESS
}
