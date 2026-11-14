// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-63 (VD-100/VD-105)** — `NetHost` của CLI: **TCP thật**, chỉ **loopback** trong vòng 11.
//!
//! Handle là chuỗi **mờ** (`"C<n>"`) — chương trình ngôn ngữ không thấy địa chỉ/socket. Coi **cả**
//! `WouldBlock` (macOS trả os error 35 cho `set_read_timeout`) **và** `TimedOut` là **hết giờ**.
//! `listen`/`accept` để **ST-65** (ở đây trả `Err` rõ ràng, không giả vờ đã có server).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use vis_interp::builtin::NetHost;

/// Host mạng của CLI. `--explain-perms` ⇒ in `NET …` ra **stderr** (không trộn stdout).
pub struct Net {
    giai_thich: bool,
    tay: Vec<(String, TcpStream)>,
    tiep: u32,
}

impl Net {
    pub fn moi(giai_thich: bool) -> Self {
        Self {
            giai_thich,
            tay: Vec::new(),
            tiep: 0,
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

    fn listen(&mut self, _dia_chi: &str) -> Result<String, String> {
        Err("`net.listen` chưa hỗ trợ ở ST-63 (để ST-65: server loopback)".to_string())
    }

    fn local_addr(&mut self, tay: &str) -> Result<String, String> {
        let s = self.tim(tay)?;
        let a = s
            .local_addr()
            .map_err(|e| format!("không lấy được địa chỉ local của `{tay}`: {e}"))?;
        Ok(a.to_string())
    }

    fn accept(&mut self, _tay: &str, _timeout_ms: u64) -> Result<String, String> {
        Err("`net.accept` chưa hỗ trợ ở ST-63 (để ST-65: server loopback)".to_string())
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
        let i =
            self.tay.iter().position(|(t, _)| t == tay).ok_or_else(|| {
                format!("tay mạng `{tay}` không tồn tại (chưa nối hoặc đã `close`)")
            })?;
        self.tay.remove(i);
        Ok(())
    }

    /// **ST-64**: nhật ký tầng giao thức (HTTP) khi `--explain-perms`.
    fn ghi_nhat_ky(&mut self, dong: &str) {
        if self.giai_thich {
            eprintln!("{dong}");
        }
    }
}
