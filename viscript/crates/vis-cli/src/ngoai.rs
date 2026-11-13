// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Host chương trình ngoài** (ST-27) — chỉ ở `vis-cli`; `vis-interp` chỉ thấy `trait Ngoai`.
//!
//! Chạy bằng [`std::process::Command`] (mảng đối số, **không** `sh -c`), env kế thừa, timeout (mặc
//! định 10 s), cap đồng thời = 1 (VS0/VS1 chưa có job). `exit != 0` ⇒ `VS0501` kèm **stderr** của
//! tiến trình (không trộn stdout). Mỗi lần chạy in một dòng `EXEC …` khi `--explain-perms`.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use vis_diag::RUNTIME_CODES;
use vis_interp::{Abort, Trap, Value};
use vis_syntax::Span;

/// Runner cho chương trình ngoài.
pub struct Runner {
    /// In `EXEC …` ra stderr trước mỗi lần chạy (nhật ký quyền).
    pub giai_thich: bool,
    /// Bao nhiêu mili-giây thì bỏ cuộc (mặc định 10 000).
    pub timeout_ms: u64,
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            giai_thich: false,
            timeout_ms: std::env::var("VIS_CMD_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10_000),
        }
    }
}

fn loi(ma: &'static vis_diag::Code, msg: impl Into<String>, span: Span) -> Abort {
    Abort::Trap(Trap::new(ma, msg, span))
}

impl vis_interp::builtin::Ngoai for Runner {
    fn chay(&mut self, ten: &str, args: &[String], raw: bool, span: Span) -> vis_interp::R<Value> {
        // Ở VS0/VS1 lệnh ngoài **chỉ** trong chế độ lệnh (`.vish`); ở script là lỗi rõ ràng.
        if self.giai_thich {
            eprintln!("EXEC {ten} {args:?}");
        }
        let mut cmd = Command::new(ten);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut con = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                // Lỗi **lúc chạy** (không tìm thấy/không chạy được) ⇒ `VS0501` như mọi lỗi runtime.
                return Err(loi(
                    &RUNTIME_CODES[0],
                    format!("không chạy được chương trình ngoài `{ten}`: {e}"),
                    span,
                ));
            }
        };
        // Chờ **có timeout** — cap đồng thời = 1 nên chỉ một tiến trình tại một thời điểm.
        let han = Instant::now() + Duration::from_millis(self.timeout_ms);
        loop {
            match con.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if Instant::now() >= han {
                        let _ = con.kill();
                        let _ = con.wait();
                        return Err(loi(
                            &RUNTIME_CODES[0],
                            format!(
                                "chương trình ngoài `{ten}` quá thời gian ({} ms)",
                                self.timeout_ms
                            ),
                            span,
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => {
                    return Err(loi(
                        &RUNTIME_CODES[0],
                        format!("lỗi chờ `{ten}`: {e}"),
                        span,
                    ))
                }
            }
        }
        let out = con.wait_with_output().map_err(|e| {
            Abort::Trap(Trap::new(
                &RUNTIME_CODES[0],
                format!("lỗi đọc kết quả `{ten}`: {e}"),
                span,
            ))
        })?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(-1);
        if code != 0 {
            let gon = stderr.trim();
            return Err(loi(
                &RUNTIME_CODES[0],
                if gon.is_empty() {
                    format!("chương trình ngoài `{ten}` thoát mã {code}")
                } else {
                    format!("chương trình ngoài `{ten}` thoát mã {code}: {gon}")
                },
                span,
            ));
        }
        if raw {
            return Ok(Value::Str(stdout));
        }
        // Mặc định: tách dòng, **bỏ** newline cuối.
        let mut dong: Vec<Value> = Vec::new();
        let mut t = stdout.as_str();
        if let Some(x) = t.strip_suffix('\n') {
            t = x;
        }
        if !t.is_empty() {
            for d in t.split('\n') {
                dong.push(Value::Str(d.to_string()));
            }
        }
        Ok(Value::List(dong))
    }
}
