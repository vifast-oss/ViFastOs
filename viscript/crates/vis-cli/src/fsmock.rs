// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`--fs-mock`** (ST-42) — `FsHost` **tất định** đọc từ một tệp kịch bản JSON, để test `ls`/`open`
//! **hermetic** (không phụ thuộc đĩa). Định dạng:
//!
//! ```json
//! { "dirs": { "./photos": [
//!     {"name": "a.jpg", "size": "2mb", "modified": "3h", "is_file": true} ] },
//!   "files": { "./x.txt": "nội dung" } }
//! ```
//!
//! `size` = literal `size` (bảng của lexer: `2mb` = **2 000 000** byte, `2mib` = 2 097 152) hoặc số
//! byte; `modified` = literal `duration` **từ epoch** (tất định) ⇒ `sort_by modified --desc` = mới nhất trước.

use vis_interp::builtin::{FsHost, MucFs};
use vis_interp::value::Value;

/// FsHost giả lập theo kịch bản.
#[derive(Debug, Clone, Default)]
pub struct FsMock {
    dirs: Vec<(String, Vec<MucFs>)>,
    files: Vec<(String, String)>,
}

fn lay(m: &Value, ten: &str) -> Option<Value> {
    if let Value::Map(entries) = m {
        entries
            .iter()
            .find(|(k, _)| matches!(k, Value::Str(s) if s == ten))
            .map(|(_, v)| v.clone())
    } else {
        None
    }
}

fn chuoi(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// `rw-r--r--`/`-rw-r--r--` (hoặc `755` hệ 8) → bit POSIX; `-`/rỗng ⇒ `0`.
fn mode_tu_chuoi(s: &str) -> u32 {
    let t = s.trim().trim_start_matches('-');
    if t.is_empty() || t == "-" {
        return 0;
    }
    if t.chars().all(|c| ('0'..='7').contains(&c)) {
        return u32::from_str_radix(t, 8).unwrap_or(0);
    }
    let mut mode = 0u32;
    let bits = [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ];
    for (i, c) in t.chars().take(9).enumerate() {
        if c != '-' {
            mode |= bits[i];
        }
    }
    mode
}

impl FsMock {
    /// Nạp kịch bản từ chuỗi JSON. `Err` nêu rõ lỗi định dạng.
    pub fn tu_json(s: &str) -> Result<Self, String> {
        let v = vis_interp::json::decode(s)
            .map_err(|e| format!("kịch bản `--fs-mock` không hợp lệ: {e}"))?;
        let mut mock = FsMock::default();
        if let Some(dirs) = lay(&v, "dirs") {
            let Value::Map(ds) = dirs else {
                return Err("`--fs-mock`: `dirs` phải là object".into());
            };
            for (k, dsach) in ds {
                let Some(p) = chuoi(&k) else { continue };
                let Value::List(items) = dsach else {
                    return Err(format!("`--fs-mock`: `dirs[{p}]` phải là mảng"));
                };
                let mut muc = Vec::new();
                for it in items {
                    let Some(ten) = lay(&it, "name").and_then(|x| chuoi(&x)) else {
                        return Err(format!("`--fs-mock`: mục trong `{p}` thiếu `name`"));
                    };
                    let size = match lay(&it, "size") {
                        Some(Value::Int(n)) => n,
                        Some(Value::Size(n)) => n,
                        Some(Value::Str(s)) => {
                            vis_interp::value::parse_size(&s).ok_or_else(|| {
                                format!("`--fs-mock`: `size` của `{ten}` không hợp lệ: {s}")
                            })?
                        }
                        _ => 0,
                    };
                    let modified = match lay(&it, "modified") {
                        Some(Value::Duration(d)) => d,
                        Some(Value::Int(n)) => n,
                        Some(Value::Str(s)) => {
                            vis_interp::value::parse_duration(&s).ok_or_else(|| {
                                format!("`--fs-mock`: `modified` của `{ten}` không hợp lệ: {s}")
                            })?
                        }
                        _ => 0,
                    };
                    let is_file = matches!(lay(&it, "is_file"), Some(Value::Bool(true)));
                    // ST-46: `mode` = chuỗi `rw-r--r--` (hoặc số), `mtime` = duration từ epoch.
                    let mode = match lay(&it, "mode") {
                        Some(Value::Int(n)) => n.max(0) as u32,
                        Some(Value::Str(s)) => mode_tu_chuoi(&s),
                        _ => 0,
                    };
                    let modified_ns = match lay(&it, "mtime") {
                        Some(Value::Duration(d)) => d,
                        Some(Value::Int(n)) => n,
                        Some(Value::Str(s)) => {
                            vis_interp::value::parse_duration(&s).ok_or_else(|| {
                                format!("`--fs-mock`: `mtime` của `{ten}` không hợp lệ: {s}")
                            })?
                        }
                        _ => modified,
                    };
                    muc.push(MucFs {
                        name: ten,
                        size: size.max(0) as u64,
                        modified_ns,
                        is_file,
                        mode,
                    });
                }
                muc.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
                mock.dirs.push((p, muc));
            }
        }
        if let Some(files) = lay(&v, "files") {
            let Value::Map(fs) = files else {
                return Err("`--fs-mock`: `files` phải là object".into());
            };
            for (k, v) in fs {
                let Some(p) = chuoi(&k) else { continue };
                let Some(nd) = chuoi(&v) else {
                    return Err(format!("`--fs-mock`: `files[{p}]` phải là chuỗi"));
                };
                mock.files.push((p, nd));
            }
        }
        Ok(mock)
    }
}

impl FsHost for FsMock {
    fn read(&mut self, p: &str) -> Result<String, String> {
        self.files
            .iter()
            .find(|(k, _)| k == p)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| format!("không đọc được tệp `{p}`: không có trong kịch bản `--fs-mock`"))
    }

    fn write(&mut self, p: &str, s: &str) -> Result<(), String> {
        match self.files.iter_mut().find(|(k, _)| k == p) {
            Some((_, v)) => *v = s.to_string(),
            None => self.files.push((p.to_string(), s.to_string())),
        }
        Ok(())
    }

    fn exists(&mut self, p: &str) -> bool {
        self.files.iter().any(|(k, _)| k == p) || self.dirs.iter().any(|(k, _)| k == p)
    }

    fn list(&mut self, p: &str) -> Result<Vec<String>, String> {
        self.list_entries(p)
            .map(|m| m.into_iter().map(|x| x.name).collect())
    }

    fn list_entries(&mut self, p: &str) -> Result<Vec<MucFs>, String> {
        self.dirs
            .iter()
            .find(|(k, _)| k == p)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                format!("không đọc được thư mục `{p}`: không có trong kịch bản `--fs-mock`")
            })
    }
}
