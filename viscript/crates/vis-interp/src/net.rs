// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-64 (VD-100/VD-105)** — **giao thức HTTP client** nằm trong `vis-interp` (không ở CLI):
//! socket thô qua [`NetHost`], tự dựng/đọc **HTTP/1.1**. Nhờ vậy interp ↔ VM **cùng byte** và codegen
//! **không** phải sửa (`std.http.*` hạ theo tên như mọi `std.*`).
//!
//! Phạm vi vòng 11: **chỉ** `http://`, **chỉ loopback** (`127.0.0.1`/`localhost`); **không** TLS,
//! **không** redirect, **không** keep-alive; phản hồi cap **8 MiB** (mẫu `MAX_STDIN`, ST-34) ⇒ vượt
//! cap là `Err`, không OOM. Body giữ **byte** tới khi chuyển `str` (lossy) — ngôn ngữ chưa có kiểu byte.

use std::time::{Duration, Instant};

use crate::builtin::NetHost;

/// Cap **phản hồi** (8 MiB, mẫu `MAX_STDIN` của ST-34) — vượt ⇒ `Err`, không OOM.
pub const MAX_HTTP: usize = 8 * 1024 * 1024;
/// Cap **header** (64 KiB) — header lớn hơn ⇒ `Err` (không đọc vô hạn).
const MAX_HEADER: usize = 64 * 1024;

/// Tách `http://host[:port][/path][?query]` → `(authority, target)`; chỉ **loopback**.
fn tach_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("chỉ hỗ trợ `http://`, được `{url}`"))?;
    if rest.contains('@') {
        return Err(format!("URL có userinfo không hỗ trợ: `{url}`"));
    }
    let i = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, duong) = (&rest[..i], &rest[i..]);
    if authority.is_empty() {
        return Err(format!("URL thiếu host: `{url}`"));
    }
    let (host, cong) = match authority.rsplit_once(':') {
        Some((h, c)) => (h, c),
        None => (authority, "80"),
    };
    if host != "127.0.0.1" && host != "localhost" {
        return Err(format!(
            "chỉ cho phép loopback (`127.0.0.1`/`localhost`), được `{host}`"
        ));
    }
    cong.parse::<u16>()
        .map_err(|_| format!("cổng `{cong}` không hợp lệ (0..65535)"))?;
    let authority = if cong == "80" {
        host.to_string()
    } else {
        format!("{host}:{cong}")
    };
    let target = if duong.is_empty() {
        "/".to_string()
    } else if duong.starts_with('/') {
        duong.to_string()
    } else {
        format!("/{duong}")
    };
    Ok((authority, target))
}

/// Tìm `\r\n\r\n` trong `du` — vị trí **byte đầu** của dấu phân cách.
fn tim_header(du: &[u8]) -> Option<usize> {
    du.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Đọc status + `Content-Length` từ khối header (không gồm `\r\n\r\n`).
fn doc_header(dau: &[u8]) -> Result<(u16, Option<usize>), String> {
    let text = String::from_utf8_lossy(dau);
    let mut lines = text.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let mut phan = status_line.split_whitespace();
    let _ver = phan.next();
    let status: u16 = phan
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("status line HTTP không hợp lệ: `{status_line}`"))?;
    let mut cl = None;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                cl = Some(
                    v.trim()
                        .parse::<usize>()
                        .map_err(|_| format!("Content-Length không hợp lệ: `{v}`"))?,
                );
            }
        }
    }
    Ok((status, cl))
}

/// Cắt body theo `Content-Length` / EOF; `h` = offset bắt đầu body.
fn hoan_thanh(du: &[u8], h: usize, n: usize) -> Result<String, String> {
    let cuoi = (h + n).min(du.len());
    Ok(String::from_utf8_lossy(&du[h..cuoi]).into_owned())
}

/// Gửi GET + đọc phản hồi trên tay đã nối. Trả **body** (chuỗi lossy) hoặc `Err` (HTTP/thời gian).
fn doc_http(
    net: &mut dyn NetHost,
    tay: &str,
    authority: &str,
    target: &str,
    timeout: u64,
) -> Result<String, String> {
    let req = format!("GET {target} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    let han = Instant::now() + Duration::from_millis(timeout.max(1));
    net.write(tay, req.as_bytes())?;
    let mut du: Vec<u8> = Vec::new();
    let mut header_end: Option<usize> = None;
    let mut cl: Option<usize> = None;
    loop {
        if let (Some(h), Some(n)) = (header_end, cl) {
            if du.len() >= h + n {
                return hoan_thanh(&du, h, n);
            }
        }
        let con = han.saturating_duration_since(Instant::now());
        if con.is_zero() {
            return Err(format!(
                "quá thời gian chờ HTTP từ `{authority}` sau {} ms",
                timeout.max(1)
            ));
        }
        let doc = net.read(tay, 16 * 1024, con.as_millis().max(1) as u64)?;
        if doc.is_empty() {
            // EOF: header phải đủ; nếu có Content-Length mà thiếu body ⇒ `Err` (không treo).
            let h = header_end
                .ok_or_else(|| "phản hồi HTTP không đủ header (kết nối đóng sớm)".to_string())?;
            return match cl {
                Some(n) if du.len() < h + n => Err(format!(
                    "Content-Length {n} nhưng mới đọc {} byte (kết nối đóng sớm)",
                    du.len() - h
                )),
                Some(n) => hoan_thanh(&du, h, n),
                None => hoan_thanh(&du, h, du.len() - h),
            };
        }
        du.extend_from_slice(&doc);
        if header_end.is_none() {
            if let Some(i) = tim_header(&du) {
                let h = i + 4;
                let (status, l) = doc_header(&du[..i])?;
                if !(200..300).contains(&status) {
                    return Err(format!("HTTP {status}"));
                }
                header_end = Some(h);
                cl = l;
                if let Some(n) = cl {
                    if n > MAX_HTTP {
                        return Err(format!("phản hồi vượt giới hạn {MAX_HTTP} byte (8 MiB)"));
                    }
                }
            } else if du.len() > MAX_HEADER {
                return Err(format!(
                    "phản hồi HTTP header quá lớn (> {MAX_HEADER} byte)"
                ));
            }
        }
        if let Some(h) = header_end {
            if du.len().saturating_sub(h) > MAX_HTTP {
                return Err(format!("phản hồi vượt giới hạn {MAX_HTTP} byte (8 MiB)"));
            }
        }
    }
}

/// `http.get`/`get_timeout` — nối qua `NetHost`, chạy giao thức, **đóng tay** (best-effort).
pub fn http_get(net: &mut dyn NetHost, url: &str, timeout_ms: u64) -> Result<String, String> {
    let (authority, target) = tach_url(url)?;
    let timeout = timeout_ms.max(1);
    let tay = net.connect(&authority, timeout)?;
    net.ghi_nhat_ky(&format!("NET http GET {authority}{target}"));
    let kq = doc_http(net, &tay, &authority, &target, timeout);
    let _ = net.close(&tay);
    kq
}

#[cfg(test)]
mod tests {
    use super::{doc_header, tach_url};

    #[test]
    fn tach_url_dung_va_chan_ngoai_loopback() {
        assert_eq!(
            tach_url("http://127.0.0.1:8080/a?b=1").unwrap(),
            ("127.0.0.1:8080".to_string(), "/a?b=1".to_string())
        );
        assert_eq!(
            tach_url("http://localhost").unwrap(),
            ("localhost".to_string(), "/".to_string())
        );
        assert_eq!(tach_url("http://127.0.0.1:81").unwrap().1, "/".to_string());
        assert!(tach_url("https://127.0.0.1/").is_err());
        assert!(tach_url("http://example.com/").is_err());
        assert!(tach_url("http://127.0.0.1:99999/").is_err());
    }

    #[test]
    fn doc_header_doc_status_va_content_length() {
        assert_eq!(
            doc_header(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nX: y").unwrap(),
            (200, Some(9))
        );
        assert_eq!(
            doc_header(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0").unwrap(),
            (404, Some(0))
        );
        assert_eq!(doc_header(b"HTTP/1.1 204 No Content").unwrap(), (204, None));
        assert!(doc_header(b"rac").is_err());
        assert!(doc_header(b"HTTP/1.1 200 OK\r\nContent-Length: x").is_err());
    }
}
