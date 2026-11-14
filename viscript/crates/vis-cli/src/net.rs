// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-63/ST-65 (VD-100/VD-105)** — `NetHost` của CLI: **TCP thật**, chỉ **loopback** trong vòng 11.
//!
//! Handle là chuỗi **mờ**: `"C<n>"` cho kết nối, `"L<n>"` cho listener — chương trình ngôn ngữ không
//! thấy địa chỉ/socket. Coi **cả** `WouldBlock` (macOS trả os error 35 cho `set_read_timeout`) **và**
//! `TimedOut` là **hết giờ**. `accept` timeout 0 ⇒ **chặn**; ngược lại poll non-blocking tới hạn.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use vis_interp::builtin::NetHost;

/// Host mạng của CLI. `--explain-perms` ⇒ in `NET …` ra **stderr** (không trộn stdout).
pub struct Net {
    giai_thich: bool,
    tay: Vec<(String, TcpStream)>,
    tiep: u32,
    /// **ST-65**: listener đang mở — handle `"L<n>"` (tách khỏi `"C<n>"` của kết nối).
    nghe: Vec<(String, TcpListener)>,
    tiep_nghe: u32,
}

impl Net {
    pub fn moi(giai_thich: bool) -> Self {
        Self {
            giai_thich,
            tay: Vec::new(),
            tiep: 0,
            nghe: Vec::new(),
            tiep_nghe: 0,
        }
    }

    fn nhat_ky(&self, thao_tac: &str, p: &str) {
        if self.giai_thich {
            eprintln!("NET {thao_tac} {p}");
        }
    }

    fn tim(&mut self, tay: &str) -> Result<&mut TcpStream, String> {
        self.tay
            .iter_mut()
            .find(|(t, _)| t == tay)
            .map(|(_, s)| s)
            .ok_or_else(|| format!("tay mạng `{tay}` không tồn tại (chưa nối hoặc đã `close`)"))
    }

    fn giu(&mut self, s: TcpStream) -> String {
        self.tiep += 1;
        let tay = format!("C{}", self.tiep);
        self.tay.push((tay.clone(), s));
        tay
    }
}

/// Chỉ **loopback** (VD-100/105 (4)): `127.0.0.1`/`localhost`; host khác ⇒ `Err`.
fn chi_loopback(dia_chi: &str) -> Result<SocketAddr, String> {
    let (host, cong) = dia_chi
        .rsplit_once(':')
        .ok_or_else(|| format!("địa chỉ `{dia_chi}` phải có dạng `host:port`"))?;
    if host != "127.0.0.1" && host != "localhost" {
        return Err(format!(
            "chỉ cho phép loopback (`127.0.0.1`/`localhost`) trong vòng này, được `{host}`"
        ));
    }
    if cong.parse::<u16>().is_err() {
        return Err(format!("cổng `{cong}` không hợp lệ (0..65535)"));
    }
    let mut ds = dia_chi
        .to_socket_addrs()
        .map_err(|e| format!("địa chỉ `{dia_chi}` không phân giải được: {e}"))?;
    ds.next()
        .ok_or_else(|| format!("địa chỉ `{dia_chi}` không có kết quả phân giải"))
}

/// **Cả** `WouldBlock` (macOS) **và** `TimedOut` là **hết giờ** (VD-100 (4)).
fn la_het_gio(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

/// **ST-65**: `accept` trên listener; `timeout_ms = 0` ⇒ **chặn** tới khi có kết nối; ngược lại
/// poll **non-blocking** mỗi 5 ms tới hạn (macOS trả `WouldBlock` — cùng logic ST-63).
fn accept_tren(l: &TcpListener, timeout_ms: u64) -> Result<TcpStream, String> {
    if timeout_ms == 0 {
        let _ = l.set_nonblocking(false);
        let (s, _) = l.accept().map_err(|e| format!("lỗi `accept`: {e}"))?;
        let _ = s.set_nonblocking(false);
        return Ok(s);
    }
    let han = Instant::now() + Duration::from_millis(timeout_ms);
    l.set_nonblocking(true)
        .map_err(|e| format!("lỗi đặt non-blocking cho listener: {e}"))?;
    loop {
        match l.accept() {
            Ok((s, _)) => {
                let _ = l.set_nonblocking(false);
                let _ = s.set_nonblocking(false);
                return Ok(s);
            }
            Err(e) if la_het_gio(&e) => {
                if Instant::now() >= han {
                    let _ = l.set_nonblocking(false);
                    return Err(format!("quá thời gian chờ kết nối sau {timeout_ms} ms"));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                let _ = l.set_nonblocking(false);
                return Err(format!("lỗi `accept`: {e}"));
            }
        }
    }
}

impl NetHost for Net {
    fn connect(&mut self, dia_chi: &str, timeout_ms: u64) -> Result<String, String> {
        self.nhat_ky("connect", dia_chi);
        let addr = chi_loopback(dia_chi)?;
        let s = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms.max(1)))
            .map_err(|e| {
                if la_het_gio(&e) {
                    format!("quá thời gian chờ nối tới `{dia_chi}`")
                } else if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    format!("bị từ chối kết nối tới `{dia_chi}`: {e}")
                } else {
                    format!("lỗi nối tới `{dia_chi}`: {e}")
                }
            })?;
        let _ = s.set_nodelay(true);
        Ok(self.giu(s))
    }

    fn listen(&mut self, dia_chi: &str) -> Result<String, String> {
        let addr = chi_loopback(dia_chi)?;
        self.nhat_ky("listen", dia_chi);
        let l = TcpListener::bind(addr).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                format!("cổng `{dia_chi}` đang bận (đã có tiến trình listen)")
            } else {
                format!("lỗi `listen` trên `{dia_chi}`: {e}")
            }
        })?;
        self.tiep_nghe += 1;
        let tay = format!("L{}", self.tiep_nghe);
        self.nghe.push((tay.clone(), l));
        Ok(tay)
    }

    fn local_addr(&mut self, tay: &str) -> Result<String, String> {
        // Ưu tiên listener (ST-65), sau đó kết nối (ST-63) — cùng một tên hàm ngôn ngữ.
        if let Some((_, l)) = self.nghe.iter().find(|(t, _)| t == tay) {
            return l
                .local_addr()
                .map(|a| a.to_string())
                .map_err(|e| format!("không lấy được địa chỉ local của `{tay}`: {e}"));
        }
        let s = self.tim(tay)?;
        let a = s
            .local_addr()
            .map_err(|e| format!("không lấy được địa chỉ local của `{tay}`: {e}"))?;
        Ok(a.to_string())
    }

    fn accept(&mut self, tay: &str, timeout_ms: u64) -> Result<String, String> {
        let i = self
            .nghe
            .iter()
            .position(|(t, _)| t == tay)
            .ok_or_else(|| {
                format!("tay listener `{tay}` không tồn tại (đã close hoặc chưa listen)")
            })?;
        self.nhat_ky("accept", tay);
        // Lách borrow: tạm rút listener ra, chờ, rồi trả lại (không đổi thứ tự handle).
        let (ten, l) = self.nghe.remove(i);
        let kq = accept_tren(&l, timeout_ms);
        self.nghe.push((ten, l));
        let s = kq?;
        Ok(self.giu(s))
    }

    fn read(&mut self, tay: &str, toi_da: usize, timeout_ms: u64) -> Result<Vec<u8>, String> {
        if toi_da == 0 {
            return Ok(Vec::new());
        }
        let s = self.tim(tay)?;
        s.set_read_timeout(Some(Duration::from_millis(timeout_ms.max(1))))
            .map_err(|e| format!("không đặt được timeout đọc cho `{tay}`: {e}"))?;
        let mut buf = vec![0u8; toi_da];
        match s.read(&mut buf) {
            Ok(0) => Ok(Vec::new()),
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(e) if la_het_gio(&e) => Err(format!(
                "quá thời gian chờ dữ liệu từ `{tay}` sau {timeout_ms} ms"
            )),
            Err(e) => Err(format!("lỗi đọc từ `{tay}`: {e}")),
        }
    }

    fn write(&mut self, tay: &str, du_lieu: &[u8]) -> Result<(), String> {
        let s = self.tim(tay)?;
        s.write_all(du_lieu)
            .and_then(|()| s.flush())
            .map_err(|e| format!("lỗi ghi tới `{tay}`: {e}"))
    }

    fn close(&mut self, tay: &str) -> Result<(), String> {
        // **ST-65**: `close` đóng được **cả** kết nối (`C…`) lẫn listener (`L…`); `close_listen`
        // của ngôn ngữ cũng đi qua đây.
        if let Some(i) = self.tay.iter().position(|(t, _)| t == tay) {
            self.tay.remove(i);
            return Ok(());
        }
        if let Some(i) = self.nghe.iter().position(|(t, _)| t == tay) {
            self.nghe.remove(i);
            return Ok(());
        }
        Err(format!(
            "tay mạng `{tay}` không tồn tại (chưa nối/listen hoặc đã `close`)"
        ))
    }

    /// **ST-64**: nhật ký tầng giao thức (HTTP) khi `--explain-perms`.
    fn ghi_nhat_ky(&mut self, dong: &str) {
        if self.giai_thich {
            eprintln!("{dong}");
        }
    }
}
