// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-67 (VD-102)** — khung LSP trên stdio: `Content-Length: N\r\n\r\n<N byte>`.
//!
//! Chỉ header `Content-Length` là **bắt buộc**; `Content-Type` và header lạ được **bỏ qua**.
//! stdout chỉ chứa frame — không bao giờ `println!` ở tầng này.

use std::io::{BufRead, Write};

/// Cap body (16 MiB) — chặn `Content-Length` khổng lồ làm OOM (không phải giới hạn LSP).
const MAX_BODY: usize = 16 * 1024 * 1024;

/// Đọc frame LSP từ `BufRead`. `Ok(None)` = **EOF sạch** (không còn byte nào ở đầu frame mới).
pub struct Doc<R> {
    r: R,
}

impl<R: BufRead> Doc<R> {
    pub fn moi(r: R) -> Self {
        Self { r }
    }

    pub fn doc_frame(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut content_length: Option<usize> = None;
        let mut dong = String::new();
        loop {
            dong.clear();
            let n = self
                .r
                .read_line(&mut dong)
                .map_err(|e| format!("lỗi đọc header LSP: {e}"))?;
            if n == 0 {
                // EOF: sạch nếu chưa đọc header nào; ngược lại là header cụt.
                if content_length.is_none() && dong.is_empty() {
                    return Ok(None);
                }
                return Err("EOF giữa header LSP".to_string());
            }
            let t = dong.trim_end_matches(['\r', '\n']);
            if t.is_empty() {
                break;
            }
            let Some((k, v)) = t.split_once(':') else {
                continue; // header lạ không có `:` — bỏ qua
            };
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_length = Some(
                    v.trim()
                        .parse::<usize>()
                        .map_err(|_| format!("Content-Length không hợp lệ: `{}`", v.trim()))?,
                );
            }
        }
        let n = content_length.ok_or("frame LSP thiếu `Content-Length`")?;
        if n > MAX_BODY {
            return Err(format!("frame LSP quá lớn: {n} byte (cap {MAX_BODY})"));
        }
        let mut body = vec![0u8; n];
        self.r
            .read_exact(&mut body)
            .map_err(|e| format!("lỗi đọc body LSP ({n} byte): {e}"))?;
        Ok(Some(body))
    }
}

/// Ghi một frame LSP (header + body), **flush** ngay.
pub fn ghi<W: Write>(w: &mut W, body: &[u8]) -> Result<(), String> {
    write!(w, "Content-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("lỗi ghi header LSP: {e}"))?;
    w.write_all(body)
        .and_then(|()| w.flush())
        .map_err(|e| format!("lỗi ghi body LSP: {e}"))
}
