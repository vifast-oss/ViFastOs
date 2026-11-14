// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-72b (VD-103/VD-106)** — cưỡng chế quyền `cap[...]` ở **biên host** + `--grant`.
//!
//! `main` khai `cap` (CST) ⇒ [`Quyen::bat`] bật: `HostFs`/`HostNet` chỉ cho thao tác trong **root**
//! đã khai; ngoài root ⇒ `Err` có dấu [`vis_interp::stdlib::DAU_QUYEN`] ⇒ tầng std đổi thành **trap
//! `VS0604`** (không bắt được), exit 2. **Không** khai `cap` ⇒ `bat=false` ⇒ **không hạn chế**
//! (tương thích 858 test cũ; "siết mặc định" là **nợ**).

use std::path::{Path, PathBuf};

use vis_interp::builtin::{FsHost, MucFs, NetHost};
use vis_interp::stdlib::DAU_QUYEN;
use vis_interp::value::Value;
use vis_syntax::{Kind, Mode, NodeKind};

/// Một `cap` khai trên `main` (đọc từ **CST**).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapKhai {
    pub name: String,
    pub kind: String,
    /// Đường dẫn/host **đã bỏ cú pháp literal** (`p"…"`/`"…"`); rỗng nếu không có `@`.
    pub path: String,
    pub write: bool,
    /// Chỉ số tham số trong `main` (0-based).
    pub pos: u32,
}

/// Một quyền **đã giải** (root canonicalize) — nguồn cho `Value::Cap` + kiểm tra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuyenCap {
    pub pos: u32,
    pub kind: String,
    pub root: String,
    pub write: bool,
    /// **ST-97 (VD-112)**: allowlist lệnh cho `cap[Proc]` (rỗng = không giới hạn).
    pub allow: Vec<String>,
}

/// Tập quyền của lần chạy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quyen {
    pub moc: Vec<QuyenCap>,
    pub bat: bool,
    /// **ST-81 (VD-108)**: `--strict-caps` — mọi truy cập fs/net phải nằm trong `cap` đã khai;
    /// **không** khai `cap` nào mà chạm fs/net ⇒ `VS0604`. Mặc định `false` (tương thích).
    pub siet: bool,
    pub goc: PathBuf,
    /// **ST-97**: `Some(ds)` = có ≥1 `--grant` cho cap `Proc` ⇒ chỉ chạy lệnh khớp `ds`.
    pub allow_proc: Option<Vec<String>>,
}

/// Bỏ `p"…"`/`"…"` để lấy đường dẫn/host **thuần**.
pub fn bo_lit(s: &str) -> String {
    let s = s.trim();
    let inner = if let Some(r) = s.strip_prefix("p\"") {
        r
    } else if let Some(r) = s.strip_prefix('"') {
        r
    } else {
        return s.to_string();
    };
    inner.strip_suffix('"').unwrap_or(inner).to_string()
}

/// **ST-93 (VD-111)**: tách **chữ ký `fn main`** trong `.vish` (khai báo ở cột 1) — CST chế độ
/// Command không có `FnDecl`, nên `--dry`/quyền trước đây **không thấy** `main(p: cap[Proc])`.
fn chu_ky_main_vish(text: &str) -> Option<String> {
    let mut bat = false;
    let mut sig = String::new();
    let mut sau = 0i32;
    for line in text.lines() {
        if !bat {
            if line.starts_with("fn main") {
                bat = true;
                sig.push_str(line);
                sig.push('\n');
            } else {
                continue;
            }
        } else {
            sig.push_str(line);
            sig.push('\n');
        }
        for c in line.chars() {
            match c {
                '(' | '[' => sau += 1,
                ')' | ']' => sau -= 1,
                _ => {}
            }
        }
        if sau == 0 && sig.contains(':') {
            return Some(sig.trim_end().to_string());
        }
    }
    None
}

/// Đọc danh sách `cap` của `main` từ **CST** (dùng cho cả `--dry` và cưỡng chế).
/// **ST-93**: `.vish` khai báo `fn main` ở cột 1 nằm **ngoài** CST Command ⇒ dựng lại một chương
/// trình Script tí hon từ chữ ký để dùng **cùng** bộ đọc `cap`.
pub fn ds_cap(text: &str, mode: Mode) -> Vec<CapKhai> {
    if mode == Mode::Command {
        if let Some(sig) = chu_ky_main_vish(text) {
            let synth = format!("{sig}\n    let __cap = 1\n");
            return ds_cap(&synth, Mode::Script);
        }
        return Vec::new();
    }
    let parsed = vis_syntax::parse(text, mode);
    let cst = &parsed.cst;
    let mut out = Vec::new();
    for n in cst.root.children() {
        if n.kind != NodeKind::FnDecl {
            continue;
        }
        let ten = n
            .tokens(&cst.tokens)
            .find(|t| t.kind == Kind::Ident)
            .map(|t| t.text.clone());
        if ten.as_deref() != Some("main") {
            continue;
        }
        let Some(ps) = n.child(NodeKind::Params) else {
            continue;
        };
        let params: Vec<&vis_syntax::Node> = ps
            .children()
            .filter(|x| x.kind == NodeKind::Param)
            .collect();
        for (pos, p) in params.iter().enumerate() {
            let Some(tyn) = p.child(NodeKind::Type) else {
                continue;
            };
            let ty = text
                .get(tyn.span.start..tyn.span.end)
                .unwrap_or("")
                .trim()
                .to_string();
            let Some(kind) = ty
                .strip_prefix("cap[")
                .and_then(|r| r.strip_suffix(']'))
                .map(str::trim)
                .filter(|k| !k.is_empty())
            else {
                continue;
            };
            let toks: Vec<&vis_syntax::Token> = p.tokens(&cst.tokens).collect();
            let mut name = String::new();
            for t in &toks {
                if t.kind == Kind::Ident {
                    name = t.text.clone();
                }
                if t.is_punct(":") {
                    break;
                }
            }
            let path = toks
                .iter()
                .find(|t| matches!(t.kind, Kind::Path | Kind::Str))
                .map(|t| bo_lit(&t.text))
                .unwrap_or_default();
            let write = p
                .child(NodeKind::GrantList)
                .map(|g| {
                    g.children()
                        .filter(|x| x.kind == NodeKind::DottedPath)
                        .any(|x| {
                            text.get(x.span.start..x.span.end)
                                .map(|s| s.trim() == "write")
                                .unwrap_or(false)
                        })
                })
                .unwrap_or(false);
            out.push(CapKhai {
                name,
                kind: kind.to_string(),
                path,
                write,
                pos: pos as u32,
            });
        }
        break; // chỉ `main` cấp tệp đầu tiên
    }
    out
}

/// Tính `Quyen` từ `cap` khai + `--grant`; lỗi (grant lạ) ⇒ `Err` cho CLI exit 2.
pub fn tao(
    ds: &[CapKhai],
    grants: &[(String, String)],
    goc: &Path,
    siet: bool,
) -> Result<Quyen, String> {
    for (ten, _) in grants {
        if !ds.iter().any(|c| &c.name == ten) {
            return Err(format!(
                "`--grant {ten}=…`: `main` không khai `cap` tên `{ten}` (không thêm quyền mới)"
            ));
        }
    }
    let mut moc = Vec::new();
    let mut allow_proc: Vec<String> = Vec::new();
    for c in ds {
        // **ST-97 (VD-112)**: `cap[Proc]` nhận **nhiều** `--grant` cùng tên = allowlist lệnh.
        if c.kind == "Proc" {
            let ds_grant: Vec<String> = grants
                .iter()
                .filter(|(n, _)| n == &c.name)
                .map(|(_, v)| v.clone())
                .collect();
            let root = if ds_grant.is_empty() {
                c.path.clone()
            } else {
                ds_grant.join(":")
            };
            allow_proc.extend(ds_grant.iter().cloned());
            moc.push(QuyenCap {
                pos: c.pos,
                kind: c.kind.clone(),
                root,
                write: false,
                allow: ds_grant,
            });
            continue;
        }
        let duong = grants
            .iter()
            .find(|(n, _)| n == &c.name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| c.path.clone());
        let root = match c.kind.as_str() {
            "Dir" | "File" => {
                let p = if Path::new(&duong).is_absolute() {
                    PathBuf::from(&duong)
                } else {
                    goc.join(&duong)
                };
                let canon = std::fs::canonicalize(&p).unwrap_or(p.clone());
                // **ST-97**: `--grant` cho `cap[Dir]` mà trỏ **tệp thực thi** ⇒ nhiều khả năng nhầm với
                // `cap[Proc]`; từ chối rõ (exit 2), không âm thầm nhận làm thư mục.
                if c.kind == "Dir" && la_tep_thuc_thi(&canon) {
                    return Err(format!(
                        "`--grant {}={duong}`: **kiểu không khớp** — `cap[Dir]` nhận **thư mục**, không nhận tệp thực thi; `cap[Proc]` mới nhận lệnh",
                        c.name
                    ));
                }
                canon.to_string_lossy().to_string()
            }
            _ => duong,
        };
        moc.push(QuyenCap {
            pos: c.pos,
            kind: c.kind.clone(),
            root,
            write: c.write && matches!(c.kind.as_str(), "Dir" | "File"),
            allow: Vec::new(),
        });
    }
    Ok(Quyen {
        moc,
        bat: siet || !ds.is_empty(),
        siet,
        goc: goc.to_path_buf(),
        allow_proc: if allow_proc.is_empty() {
            None
        } else {
            Some(allow_proc)
        },
    })
}

/// **ST-97**: tệp thực thi (Unix) — dùng để bắt `--grant` Proc gán nhầm cho `cap[Dir]`.
#[cfg(unix)]
fn la_tep_thuc_thi(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn la_tep_thuc_thi(_p: &Path) -> bool {
    false
}

impl Quyen {
    /// **ST-97**: allowlist `cap[Proc]` (nếu người dùng đã `--grant`).
    pub fn allow_proc(&self) -> Option<Vec<String>> {
        self.allow_proc.clone()
    }

    /// Giá trị `Value::Cap` cho từng tham số (đưa vào interp/VM).
    pub fn gia_tri(&self) -> Vec<(u32, Value)> {
        self.moc
            .iter()
            .map(|c| {
                (
                    c.pos,
                    Value::Cap {
                        kind: c.kind.clone(),
                        root: c.root.clone(),
                        write: c.write,
                    },
                )
            })
            .collect()
    }

    fn tuyet_doi(&self, p: &str) -> PathBuf {
        let p = Path::new(p);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.goc.join(p)
        }
    }

    fn chuan(p: &Path) -> PathBuf {
        if let Ok(c) = std::fs::canonicalize(p) {
            return c;
        }
        // Tệp **chưa tồn tại** (ghi): canonicalize thư mục cha rồi ghép tên.
        if let (Some(cha), Some(ten)) = (p.parent(), p.file_name()) {
            if let Ok(c) = std::fs::canonicalize(cha) {
                return c.join(ten);
            }
        }
        p.to_path_buf()
    }

    /// Kiểm thao tác hệ tệp; `can_write=false` ⇒ chỉ cần root, `true` ⇒ root phải có `(write)`.
    pub fn kiem_fs(&self, p: &str, can_write: bool) -> Result<(), String> {
        if !self.bat {
            return Ok(());
        }
        let c = Self::chuan(&self.tuyet_doi(p));
        let mut trong_root = false;
        for q in &self.moc {
            if q.kind != "Dir" && q.kind != "File" {
                continue;
            }
            let r = Self::chuan(Path::new(&q.root));
            if c.starts_with(&r) {
                trong_root = true;
                if !can_write || q.write {
                    return Ok(());
                }
            }
        }
        let co_fs = self.moc.iter().any(|q| q.kind == "Dir" || q.kind == "File");
        let msg = if self.siet && !co_fs {
            // **ST-81**: strict + **không** khai cap fs ⇒ nêu rõ **cần khai** gì.
            format!("[quyền] cần khai `cap[Dir]` cho `{p}`")
        } else if trong_root {
            format!("[quyền] thiếu `(write)` trên `cap[Dir]` cho `{p}`")
        } else {
            format!("[quyền] thiếu `cap[Dir]` cho `{p}`")
        };
        Err(format!("{DAU_QUYEN}{msg}"))
    }

    /// **ST-93 (VD-111)**: `--strict-caps` + **không** khai `cap[Proc]` ⇒ **không** cho chạy tiến
    /// trình ngoài. Mặc định (`siet=false`) vẫn cho (tương thích).
    pub fn cho_phep_proc(&self) -> bool {
        !self.siet || self.moc.iter().any(|c| c.kind == "Proc")
    }

    /// Kiểm thao tác mạng: host khớp `cap[Net]` đã khai **hoặc** loopback khi có `cap[Net]`.
    pub fn kiem_net(&self, dia_chi: &str) -> Result<(), String> {
        if !self.bat {
            return Ok(());
        }
        let host = dia_chi.rsplit_once(':').map(|(h, _)| h).unwrap_or(dia_chi);
        let co_net = self.moc.iter().any(|q| q.kind == "Net");
        let khop = self.moc.iter().any(|q| {
            q.kind == "Net" && (q.root == host || (q.root.is_empty() && host == "localhost"))
        });
        let loopback = host == "127.0.0.1" || host == "localhost";
        if khop || (loopback && co_net) {
            Ok(())
        } else if self.siet && !co_net {
            // **ST-81**: strict + không khai `cap[Net]` ⇒ nêu rõ cần khai.
            Err(format!(
                "{DAU_QUYEN}[quyền] cần khai `cap[Net]` cho `{dia_chi}`"
            ))
        } else {
            Err(format!(
                "{DAU_QUYEN}[quyền] thiếu `cap[Net]` cho `{dia_chi}` (không khai host này)"
            ))
        }
    }
}

/// Bọc `FsHost` để **kiểm quyền trước** mọi thao tác.
pub struct FsGuard<'a> {
    pub trong: &'a mut dyn FsHost,
    pub quyen: &'a Quyen,
}

impl FsHost for FsGuard<'_> {
    fn read(&mut self, p: &str) -> Result<String, String> {
        self.quyen.kiem_fs(p, false)?;
        self.trong.read(p)
    }
    fn write(&mut self, p: &str, s: &str) -> Result<(), String> {
        self.quyen.kiem_fs(p, true)?;
        self.trong.write(p, s)
    }
    /// **ST-83**: ngoài quyền ⇒ **trap `VS0604`** (không còn `false` im lặng).
    fn exists(&mut self, p: &str) -> bool {
        self.exists_quyen(p).unwrap_or(false)
    }
    fn exists_quyen(&mut self, p: &str) -> Result<bool, String> {
        self.quyen.kiem_fs(p, false)?;
        Ok(self.trong.exists(p))
    }
    fn list(&mut self, p: &str) -> Result<Vec<String>, String> {
        self.quyen.kiem_fs(p, false)?;
        self.trong.list(p)
    }
    fn list_entries(&mut self, p: &str) -> Result<Vec<MucFs>, String> {
        self.quyen.kiem_fs(p, false)?;
        self.trong.list_entries(p)
    }
}

/// Bọc `NetHost` để kiểm `cap[Net]`.
pub struct NetGuard<'a> {
    pub trong: &'a mut dyn NetHost,
    pub quyen: &'a Quyen,
}

impl NetHost for NetGuard<'_> {
    fn connect(&mut self, dia_chi: &str, timeout_ms: u64) -> Result<String, String> {
        self.quyen.kiem_net(dia_chi)?;
        self.trong.connect(dia_chi, timeout_ms)
    }
    fn listen(&mut self, dia_chi: &str) -> Result<String, String> {
        self.quyen.kiem_net(dia_chi)?;
        self.trong.listen(dia_chi)
    }
    fn local_addr(&mut self, tay: &str) -> Result<String, String> {
        self.trong.local_addr(tay)
    }
    fn accept(&mut self, tay: &str, timeout_ms: u64) -> Result<String, String> {
        self.trong.accept(tay, timeout_ms)
    }
    fn read(&mut self, tay: &str, toi_da: usize, timeout_ms: u64) -> Result<Vec<u8>, String> {
        self.trong.read(tay, toi_da, timeout_ms)
    }
    fn write(&mut self, tay: &str, du_lieu: &[u8]) -> Result<(), String> {
        self.trong.write(tay, du_lieu)
    }
    fn close(&mut self, tay: &str) -> Result<(), String> {
        self.trong.close(tay)
    }
    fn ghi_nhat_ky(&mut self, dong: &str) {
        self.trong.ghi_nhat_ky(dong);
    }
}
