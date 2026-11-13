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

/// Cap **stdin** mỗi stage ngoài (ST-34): 8 MiB. Vượt ⇒ `VS0501` **trước khi spawn**.
pub const MAX_STDIN: usize = 8 * 1024 * 1024;

impl Runner {
    /// Chạy một chương trình ngoài, ghi `stdin` (thread riêng, đóng khi xong), có timeout.
    ///
    /// **Không** deadlock: writer chạy song song, `wait_with_output()` đọc stdout/stderr.
    /// Timeout **10 s/stage** (VD-60); thông điệp nêu **stage nào** (ST-34).
    fn chay_raw(
        &mut self,
        ten: &str,
        args: &[String],
        stdin: &str,
        span: Span,
    ) -> Result<vis_interp::builtin::KetQua, Abort> {
        if stdin.len() > MAX_STDIN {
            return Err(loi(
                &RUNTIME_CODES[0],
                format!(
                    "stdin của stage `{ten}` quá lớn: {} byte (giới hạn {MAX_STDIN} byte = 8 MiB)",
                    stdin.len()
                ),
                span,
            ));
        }
        if self.giai_thich {
            eprintln!("EXEC {ten} {args:?}");
            if !stdin.is_empty() {
                eprintln!("PIPE stdin {ten} ({} byte)", stdin.len());
            }
        }
        let mut cmd = Command::new(ten);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut con = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Err(loi(
                    &RUNTIME_CODES[0],
                    format!("không chạy được chương trình ngoài `{ten}`: {e}"),
                    span,
                ));
            }
        };
        // Ghi stdin ở **thread riêng** rồi **đóng** (drop) — nếu không `cat`/`grep` treo.
        if let Some(mut si) = con.stdin.take() {
            let du_lieu = stdin.as_bytes().to_vec();
            std::thread::spawn(move || {
                use std::io::Write;
                // Lỗi `EPIPE` sau khi tiến trình bị giết (timeout) — **bỏ qua**.
                let _ = si.write_all(&du_lieu);
                let _ = si.flush();
            });
        }
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
                            format!("stage `{ten}` quá thời gian ({} ms)", self.timeout_ms),
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
        Ok(vis_interp::builtin::KetQua {
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            code: out.status.code().unwrap_or(-1),
        })
    }
}

impl vis_interp::builtin::Ngoai for Runner {
    fn chay(
        &mut self,
        ten: &str,
        args: &[String],
        raw: bool,
        stdin: &str,
        span: Span,
    ) -> vis_interp::R<Value> {
        let kq = self.chay_raw(ten, args, stdin, span)?;
        if kq.code != 0 {
            let gon = kq.stderr.trim();
            return Err(loi(
                &RUNTIME_CODES[0],
                if gon.is_empty() {
                    format!("chương trình ngoài `{ten}` thoát mã {}", kq.code)
                } else {
                    format!("chương trình ngoài `{ten}` thoát mã {}: {gon}", kq.code)
                },
                span,
            ));
        }
        if raw {
            return Ok(Value::Str(kq.stdout));
        }
        Ok(tach_dong(&kq.stdout))
    }

    fn chay_day_du(
        &mut self,
        ten: &str,
        args: &[String],
        stdin: &str,
        span: Span,
    ) -> Result<vis_interp::builtin::KetQua, Abort> {
        self.chay_raw(ten, args, stdin, span)
    }
}

/// Tách `stdout` thành `list[str]` — **bỏ** newline cuối, không sinh dòng rỗng thừa.
fn tach_dong(s: &str) -> Value {
    let mut t = s;
    if let Some(x) = t.strip_suffix('\n') {
        t = x;
    }
    let mut dong: Vec<Value> = Vec::new();
    if !t.is_empty() {
        for d in t.split('\n') {
            dong.push(Value::Str(d.to_string()));
        }
    }
    Value::List(dong)
}

/// **Host hệ thống tệp** (ST-33) — `std::fs` **chỉ** ở đây; `vis-interp`/`vis-vm` chỉ thấy trait.
///
/// Chốt an toàn (Agent 0 duyệt): **cho phép** đường dẫn tuyệt đối và `..` (không sandbox — consent
/// thật/`cap[Dir]` là **VS2**); mỗi lần truy cập in `FS …` ra stderr khi `--explain-perms`.
impl vis_interp::builtin::FsHost for Runner {
    fn read(&mut self, p: &str) -> Result<String, String> {
        self.nhat_ky("read", p);
        std::fs::read_to_string(p).map_err(|e| format!("không đọc được `{p}`: {e}"))
    }

    fn write(&mut self, p: &str, s: &str) -> Result<(), String> {
        self.nhat_ky("write", p);
        std::fs::write(p, s).map_err(|e| format!("không ghi được `{p}`: {e}"))
    }

    fn exists(&mut self, p: &str) -> bool {
        self.nhat_ky("exists", p);
        std::path::Path::new(p).exists()
    }

    fn list(&mut self, p: &str) -> Result<Vec<String>, String> {
        self.nhat_ky("list", p);
        let rd = std::fs::read_dir(p).map_err(|e| format!("không đọc được thư mục `{p}`: {e}"))?;
        let mut ten: Vec<String> = Vec::new();
        for e in rd {
            let e = e.map_err(|x| format!("lỗi đọc mục trong `{p}`: {x}"))?;
            ten.push(e.file_name().to_string_lossy().to_string());
        }
        // **Sắp xếp** theo byte — ổn định giữa các hệ tệp; **không** đệ quy.
        ten.sort();
        Ok(ten)
    }
}

impl Runner {
    fn nhat_ky(&self, thao_tac: &str, p: &str) {
        if self.giai_thich {
            eprintln!("FS {thao_tac} {p}");
        }
    }
}
