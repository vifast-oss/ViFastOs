// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-70 (VD-101)** — `vis pkg`: registry **local** (không mạng), cài vào `.vis/pkg/<tên>/<ver>/`.
//!
//! Mô hình: `manifest.json` của mỗi gói khai **tường minh** `files[{path,sha256}]` (không glob,
//! không `read_dir` khi cài); nội dung được **chuẩn hoá** (bỏ BOM, CRLF→LF, đúng một LF cuối) **khi
//! ingest** ⇒ băm ổn định. `vis.lock` ghi **tất cả** gói đã cài (khoá sắp theo tên, không timestamp,
//! không đường dẫn tuyệt đối). `install` **chỉ** đọc `vis.lock` + kho blob `.vis/store/<sha256>` ⇒
//! không cần registry.
//!
//! Băm: `file_sha = SHA256(nội dung chuẩn hoá)`;
//! `tree_sha = SHA256("vispkg1\n" + Σ sorted(path+"\n"+file_sha+"\n"))` — dùng
//! [`vis_bytecode::source_hash`] (SHA-256 thật, không thêm crate).

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use vis_interp::stdlib::json;
use vis_interp::value::Value;

/// Chạy `vis pkg …`; lỗi ⇒ `vis pkg: …` + exit **2** (mẫu `vis cache`, không mã mới).
pub fn run(args: &[String]) -> ExitCode {
    match chay(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vis pkg: {e}");
            ExitCode::from(2)
        }
    }
}

fn dung() -> &'static str {
    "dùng: vis pkg add <tên>[@<ver>] [--registry <dir>] | vis pkg install | vis pkg list"
}

fn chay(args: &[String]) -> Result<(), String> {
    let Some(lenh) = args.first() else {
        return Err(dung().to_string());
    };
    match lenh.as_str() {
        "add" => lenh_add(&args[1..]),
        "install" => lenh_install(&args[1..]),
        "list" => lenh_list(&args[1..]),
        khac => Err(format!("lệnh lạ `{khac}` — {}", dung())),
    }
}

/// Băm hex của một chuỗi (SHA-256 thật, qua `vis-bytecode`).
fn bam(s: &str) -> String {
    let h = vis_bytecode::source_hash(s);
    let mut out = String::with_capacity(64);
    for b in h {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Chuẩn hoá **khi ingest**: bỏ BOM, CRLF→LF, đúng **một** LF cuối; từ chối non-UTF-8.
fn chuan_hoa(bytes: &[u8]) -> Result<String, String> {
    let s = std::str::from_utf8(bytes).map_err(|e| format!("tệp không phải UTF-8: {e}"))?;
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    let s = s.replace("\r\n", "\n");
    let s = s.trim_end_matches('\n');
    if s.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{s}\n"))
    }
}

/// `tree_sha` trên danh sách `(path, file_sha)` — sắp theo **byte** của path.
fn bam_cay(files: &[(String, String)]) -> String {
    let mut ds = files.to_vec();
    ds.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut s = String::from("vispkg1\n");
    for (p, h) in ds {
        s.push_str(&p);
        s.push('\n');
        s.push_str(&h);
        s.push('\n');
    }
    bam(&s)
}

// ----- JSON tiện ích -----

fn lay<'a>(kv: &'a [(Value, Value)], k: &str) -> Option<&'a Value> {
    kv.iter().find_map(|(kk, v)| match kk {
        Value::Str(s) if s == k => Some(v),
        _ => None,
    })
}

fn chuoi(v: &Value) -> Option<&str> {
    match v {
        Value::Str(s) => Some(s),
        _ => None,
    }
}

fn lay_chuoi(kv: &[(Value, Value)], k: &str) -> Option<String> {
    lay(kv, k).and_then(chuoi).map(str::to_string)
}

fn la_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Đường dẫn tương đối an toàn: chỉ component `Normal`, không `..`/tuyệt đối/`\`.
fn kiem_path(p: &str) -> Result<(), String> {
    if p.is_empty() || p.contains('\\') {
        return Err(format!("đường dẫn `{p}` không hợp lệ"));
    }
    let mut co = false;
    for c in Path::new(p).components() {
        match c {
            Component::Normal(_) => co = true,
            _ => return Err(format!("đường dẫn `{p}` phải tương đối, không `..`/gốc")),
        }
    }
    if co {
        Ok(())
    } else {
        Err(format!("đường dẫn `{p}` không hợp lệ"))
    }
}

// ----- manifest -----

struct TepManifest {
    path: String,
    sha256: String,
}

struct Manifest {
    name: String,
    version: String,
    deps: Vec<(String, String)>,
    files: Vec<TepManifest>,
}

fn doc_manifest(duong: &Path) -> Result<Manifest, String> {
    let bytes = std::fs::read(duong)
        .map_err(|e| format!("không đọc được manifest `{}`: {e}", duong.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| format!("manifest `{}` không phải UTF-8: {e}", duong.display()))?;
    let v = vis_interp::json::decode(text)
        .map_err(|e| format!("manifest `{}` hỏng: {e}", duong.display()))?;
    let Value::Map(kv) = &v else {
        return Err(format!("manifest `{}` phải là object", duong.display()));
    };
    let name = lay_chuoi(kv, "name")
        .ok_or_else(|| format!("manifest `{}` thiếu trường `name`", duong.display()))?;
    let version = lay_chuoi(kv, "version")
        .ok_or_else(|| format!("manifest `{}` thiếu trường `version`", duong.display()))?;
    let entry = lay_chuoi(kv, "entry")
        .ok_or_else(|| format!("manifest `{}` thiếu trường `entry`", duong.display()))?;
    kiem_path(&entry)?;
    let fs_v = lay(kv, "files")
        .ok_or_else(|| format!("manifest `{}` thiếu trường `files`", duong.display()))?;
    let Value::List(ds) = fs_v else {
        return Err(format!(
            "manifest `{}`: `files` phải là mảng",
            duong.display()
        ));
    };
    let mut files = Vec::new();
    let mut da = BTreeSet::new();
    for it in ds {
        let Value::Map(f) = it else {
            return Err(format!(
                "manifest `{}`: mỗi `files[]` phải là object",
                duong.display()
            ));
        };
        let path = lay_chuoi(f, "path")
            .ok_or_else(|| format!("manifest `{}`: `files[]` thiếu `path`", duong.display()))?;
        kiem_path(&path)?;
        let sha = lay_chuoi(f, "sha256")
            .ok_or_else(|| format!("manifest `{}`: `files[]` thiếu `sha256`", duong.display()))?;
        if !la_hex64(&sha) {
            return Err(format!(
                "manifest `{}`: `sha256` của `{path}` phải 64 chữ số hex",
                duong.display()
            ));
        }
        if !da.insert(path.clone()) {
            return Err(format!(
                "manifest `{}`: trùng `files[].path` `{path}`",
                duong.display()
            ));
        }
        files.push(TepManifest {
            path,
            sha256: sha.to_ascii_lowercase(),
        });
    }
    if files.is_empty() {
        return Err(format!("manifest `{}`: `files` rỗng", duong.display()));
    }
    if !files.iter().any(|f| f.path == entry) {
        return Err(format!(
            "manifest `{}`: `entry` `{entry}` không nằm trong `files`",
            duong.display()
        ));
    }
    let mut deps = Vec::new();
    if let Some(dv) = lay(kv, "deps") {
        let Value::Map(dm) = dv else {
            return Err(format!(
                "manifest `{}`: `deps` phải là object",
                duong.display()
            ));
        };
        for (k, v) in dm {
            let Some(dn) = chuoi(k) else {
                return Err(format!(
                    "manifest `{}`: tên dep phải là chuỗi",
                    duong.display()
                ));
            };
            let Some(dver) = chuoi(v) else {
                return Err(format!(
                    "manifest `{}`: version của dep `{dn}` phải là chuỗi",
                    duong.display()
                ));
            };
            so_phien_ban(dver)?;
            deps.push((dn.to_string(), dver.to_string()));
        }
    }
    Ok(Manifest {
        name,
        version,
        deps,
        files,
    })
}

// ----- phiên bản theo **số** -----

fn so_phien_ban(v: &str) -> Result<Vec<u64>, String> {
    if v.is_empty() {
        return Err("version rỗng".to_string());
    }
    let mut ds = Vec::new();
    for p in v.split('.') {
        ds.push(
            p.parse::<u64>()
                .map_err(|_| format!("version `{v}` không phải số (phần `{p}`)"))?,
        );
    }
    Ok(ds)
}

fn cmp_phien_ban(a: &str, b: &str) -> Result<std::cmp::Ordering, String> {
    let mut x = so_phien_ban(a)?;
    let mut y = so_phien_ban(b)?;
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    Ok(x.cmp(&y))
}

/// Chọn version cao nhất trong `registry/<tên>/*` (nếu không ghi `@ver`).
fn chon_cao_nhat(reg: &Path, ten: &str) -> Result<String, String> {
    let dir = reg.join(ten);
    let ds = std::fs::read_dir(&dir).map_err(|e| {
        format!(
            "không thấy gói `{ten}` trong registry `{}`: {e}",
            reg.display()
        )
    })?;
    let mut tot: Option<String> = None;
    for it in ds {
        let it = it.map_err(|e| format!("lỗi đọc registry: {e}"))?;
        if !it.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let Some(v) = it.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if so_phien_ban(&v).is_err() {
            continue;
        }
        if !reg.join(ten).join(&v).join("manifest.json").is_file() {
            continue;
        }
        let tot_hon = match &tot {
            None => true,
            Some(t) => cmp_phien_ban(&v, t)? == std::cmp::Ordering::Greater,
        };
        if tot_hon {
            tot = Some(v);
        }
    }
    tot.ok_or_else(|| {
        format!(
            "gói `{ten}` không có version hợp lệ trong registry `{}`",
            reg.display()
        )
    })
}

// ----- gom closure để cài -----

struct TepCai {
    path: String,
    sha256: String,
    noi_dung: String,
}

struct GoiCai {
    name: String,
    version: String,
    sha256: String,
    files: Vec<TepCai>,
}

fn gom_goi(
    reg: &Path,
    name: &str,
    version: &str,
    ds: &mut Vec<GoiCai>,
    da: &mut BTreeSet<(String, String)>,
) -> Result<(), String> {
    if !da.insert((name.to_string(), version.to_string())) {
        return Ok(());
    }
    let dir = reg.join(name).join(version);
    let m = doc_manifest(&dir.join("manifest.json"))?;
    if m.name != name || m.version != version {
        return Err(format!(
            "manifest `{}` khai `{}@{}` nhưng thư mục là `{name}@{version}`",
            dir.display(),
            m.name,
            m.version
        ));
    }
    let mut files = Vec::new();
    for f in &m.files {
        let full = dir.join(&f.path);
        let meta = std::fs::symlink_metadata(&full)
            .map_err(|e| format!("không thấy `{}`: {e}", full.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("`{}` là symlink — không nhận", full.display()));
        }
        if !meta.file_type().is_file() {
            return Err(format!("`{}` không phải tệp thường", full.display()));
        }
        let bytes =
            std::fs::read(&full).map_err(|e| format!("lỗi đọc `{}`: {e}", full.display()))?;
        let noi_dung = chuan_hoa(&bytes).map_err(|e| format!("`{}`: {e}", full.display()))?;
        let sha = bam(&noi_dung);
        if sha != f.sha256 {
            return Err(format!(
                "`{name}@{version}`: băm `{}` lệch — manifest `{}`, nội dung thật `{sha}`",
                f.path, f.sha256
            ));
        }
        files.push(TepCai {
            path: f.path.clone(),
            sha256: sha,
            noi_dung,
        });
    }
    files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    let cap: Vec<(String, String)> = files
        .iter()
        .map(|f| (f.path.clone(), f.sha256.clone()))
        .collect();
    let sha256 = bam_cay(&cap);
    ds.push(GoiCai {
        name: m.name.clone(),
        version: m.version.clone(),
        sha256,
        files,
    });
    for (dn, dv) in &m.deps {
        gom_goi(reg, dn, dv, ds, da)?;
    }
    Ok(())
}

// ----- vis.lock -----

struct TepKhoa {
    path: String,
    sha256: String,
}

struct GoiKhoa {
    name: String,
    version: String,
    sha256: String,
    files: Vec<TepKhoa>,
}

fn doc_khoa(duong: &Path, bat_buoc: bool) -> Result<Vec<GoiKhoa>, String> {
    let bytes = match std::fs::read(duong) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !bat_buoc => return Ok(Vec::new()),
        Err(e) => return Err(format!("không đọc được `{}`: {e}", duong.display())),
    };
    let text =
        std::str::from_utf8(&bytes).map_err(|e| format!("`vis.lock` không phải UTF-8: {e}"))?;
    let v = vis_interp::json::decode(text).map_err(|e| format!("`vis.lock` hỏng: {e}"))?;
    let Value::Map(kv) = &v else {
        return Err("`vis.lock` hỏng: phải là object".to_string());
    };
    match lay(kv, "lock") {
        Some(Value::Int(1)) => {}
        _ => return Err("`vis.lock` hỏng: thiếu `lock` = 1".to_string()),
    }
    let Some(Value::List(ps)) = lay(kv, "packages") else {
        return Err("`vis.lock` hỏng: thiếu `packages`".to_string());
    };
    let mut out = Vec::new();
    for p in ps {
        let Value::Map(p) = p else {
            return Err("`vis.lock` hỏng: `packages[]` phải là object".to_string());
        };
        let name = lay_chuoi(p, "name")
            .ok_or_else(|| "`vis.lock` hỏng: package thiếu `name`".to_string())?;
        let version = lay_chuoi(p, "version")
            .ok_or_else(|| "`vis.lock` hỏng: package thiếu `version`".to_string())?;
        so_phien_ban(&version)?;
        let sha256 = lay_chuoi(p, "sha256")
            .ok_or_else(|| "`vis.lock` hỏng: package thiếu `sha256`".to_string())?;
        if !la_hex64(&sha256) {
            return Err("`vis.lock` hỏng: `sha256` package không phải 64 hex".to_string());
        }
        let Some(Value::List(fs)) = lay(p, "files") else {
            return Err("`vis.lock` hỏng: package thiếu `files`".to_string());
        };
        let mut files = Vec::new();
        for f in fs {
            let Value::Map(f) = f else {
                return Err("`vis.lock` hỏng: `files[]` phải là object".to_string());
            };
            let path = lay_chuoi(f, "path")
                .ok_or_else(|| "`vis.lock` hỏng: file thiếu `path`".to_string())?;
            kiem_path(&path)?;
            let sha = lay_chuoi(f, "sha256")
                .ok_or_else(|| "`vis.lock` hỏng: file thiếu `sha256`".to_string())?;
            if !la_hex64(&sha) {
                return Err("`vis.lock` hỏng: `sha256` file không phải 64 hex".to_string());
            }
            files.push(TepKhoa {
                path,
                sha256: sha.to_ascii_lowercase(),
            });
        }
        files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        out.push(GoiKhoa {
            name,
            version,
            sha256: sha256.to_ascii_lowercase(),
            files,
        });
    }
    out.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    Ok(out)
}

fn gia_tri_khoa(goi: &[GoiKhoa]) -> Value {
    let ps: Vec<Value> = goi
        .iter()
        .map(|p| {
            let fs: Vec<Value> = p
                .files
                .iter()
                .map(|f| {
                    Value::Map(vec![
                        (Value::Str("path".into()), Value::Str(f.path.clone())),
                        (Value::Str("sha256".into()), Value::Str(f.sha256.clone())),
                    ])
                })
                .collect();
            Value::Map(vec![
                (Value::Str("name".into()), Value::Str(p.name.clone())),
                (Value::Str("version".into()), Value::Str(p.version.clone())),
                (Value::Str("sha256".into()), Value::Str(p.sha256.clone())),
                (Value::Str("files".into()), Value::List(fs)),
            ])
        })
        .collect();
    Value::Map(vec![
        (Value::Str("lock".into()), Value::Int(1)),
        (Value::Str("packages".into()), Value::List(ps)),
    ])
}

fn ghi_atomic(p: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)
            .map_err(|e| format!("không tạo được thư mục `{}`: {e}", d.display()))?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("lỗi ghi `{}`: {e}", tmp.display()))?;
    std::fs::rename(&tmp, p).map_err(|e| format!("lỗi đổi tên `{}`: {e}", p.display()))
}

fn ghi_lock(p: &Path, goi: &[GoiKhoa]) -> Result<(), String> {
    let mut s = String::new();
    json(&gia_tri_khoa(goi), false, &mut s);
    s.push('\n');
    ghi_atomic(p, s.as_bytes())
}

// ----- lệnh -----

/// Tách `tên[@version]`.
fn tach_ten(spec: &str) -> Result<(String, Option<String>), String> {
    match spec.rsplit_once('@') {
        Some((ten, ver)) => {
            if ten.is_empty() || ver.is_empty() {
                return Err(format!("tên gói `{spec}` không hợp lệ (dạng `tên[@ver]`)"));
            }
            so_phien_ban(ver)?;
            Ok((ten.to_string(), Some(ver.to_string())))
        }
        None => Ok((spec.to_string(), None)),
    }
}

fn tuy_chon(
    args: &[String],
    cho_registry: bool,
) -> Result<(Option<String>, Option<PathBuf>), String> {
    let mut ten = None;
    let mut reg = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--registry" if cho_registry => {
                i += 1;
                let Some(v) = args.get(i) else {
                    return Err("thiếu giá trị sau `--registry`".to_string());
                };
                reg = Some(PathBuf::from(v));
            }
            a if a.starts_with('-') => return Err(format!("cờ lạ `{a}`")),
            a => {
                if ten.is_some() {
                    return Err(format!("thừa đối số `{a}`"));
                }
                ten = Some(a.to_string());
            }
        }
        i += 1;
    }
    Ok((ten, reg))
}

fn thu_muc_registry(chon: Option<PathBuf>) -> PathBuf {
    chon.or_else(|| std::env::var_os("VIS_REGISTRY").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("registry"))
}

fn cwd() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("không lấy được thư mục hiện tại: {e}"))
}

/// `vis pkg add <tên>[@<ver>] [--registry <dir>]`.
fn lenh_add(args: &[String]) -> Result<(), String> {
    let (ten, reg) = tuy_chon(args, true)?;
    let Some(spec) = ten else {
        return Err(format!("thiếu tên gói — {}", dung()));
    };
    let (ten, ver) = tach_ten(&spec)?;
    let reg = thu_muc_registry(reg);
    let ver = match ver {
        Some(v) => v,
        None => chon_cao_nhat(&reg, &ten)?,
    };
    let cwd = cwd()?;
    // Đọc lock cũ **trước** khi ghi gì (lock hỏng ⇒ dừng, không nửa vời).
    let mut khoa = doc_khoa(&cwd.join("vis.lock"), false)?;
    let mut ds = Vec::new();
    let mut da = BTreeSet::new();
    gom_goi(&reg, &ten, &ver, &mut ds, &mut da)?;
    ds.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    // 1) blob nội dung (content-addressed) — chỉ sau khi **mọi** manifest hợp lệ.
    for g in &ds {
        for f in &g.files {
            ghi_atomic(
                &cwd.join(".vis/store").join(&f.sha256),
                f.noi_dung.as_bytes(),
            )?;
        }
    }
    // 2) cây cài đặt: xoá đích rồi ghi từng tệp theo thứ tự sort (không còn tệp thừa).
    for g in &ds {
        let dich = cwd.join(".vis/pkg").join(&g.name).join(&g.version);
        let _ = std::fs::remove_dir_all(&dich);
        for f in &g.files {
            let p = dich.join(&f.path);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)
                    .map_err(|e| format!("không tạo được `{}`: {e}", d.display()))?;
            }
            std::fs::write(&p, f.noi_dung.as_bytes())
                .map_err(|e| format!("lỗi ghi `{}`: {e}", p.display()))?;
        }
    }
    // 3) lock cuối cùng (merge theo tên; gói mới thắng).
    for g in ds {
        khoa.retain(|x| x.name != g.name);
        khoa.push(GoiKhoa {
            name: g.name,
            version: g.version,
            sha256: g.sha256,
            files: g
                .files
                .iter()
                .map(|f| TepKhoa {
                    path: f.path.clone(),
                    sha256: f.sha256.clone(),
                })
                .collect(),
        });
    }
    khoa.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    ghi_lock(&cwd.join("vis.lock"), &khoa)
}

/// `vis pkg install` — **chỉ** đọc `vis.lock` + `.vis/store`; không mở registry.
fn lenh_install(args: &[String]) -> Result<(), String> {
    let (ten, reg) = tuy_chon(args, false)?;
    if ten.is_some() {
        return Err(format!("`install` không nhận đối số — {}", dung()));
    }
    if reg.is_some() {
        return Err("`install` không nhận `--registry`".to_string());
    }
    let cwd = cwd()?;
    let khoa = doc_khoa(&cwd.join("vis.lock"), true)?;
    // Kiểm **toàn bộ** blob trước khi chạm `.vis/pkg` (lock hỏng/thiếu blob ⇒ cây giữ nguyên).
    struct GoiLai {
        name: String,
        version: String,
        files: Vec<(String, String)>,
    }
    let mut noi_dung: Vec<GoiLai> = Vec::new();
    for g in &khoa {
        let mut fs = Vec::new();
        for f in &g.files {
            let blob = cwd.join(".vis/store").join(&f.sha256);
            let bytes = std::fs::read(&blob).map_err(|_| {
                format!(
                    "thiếu blob `{}` cho `{}@{}` — chạy `vis pkg add`",
                    f.sha256, g.name, g.version
                )
            })?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|e| format!("blob `{}` không phải UTF-8: {e}", f.sha256))?;
            if bam(text) != f.sha256 {
                return Err(format!("blob `{}` hỏng (băm lệch)", f.sha256));
            }
            fs.push((f.path.clone(), text.to_string()));
        }
        noi_dung.push(GoiLai {
            name: g.name.clone(),
            version: g.version.clone(),
            files: fs,
        });
    }
    for g in noi_dung {
        let dich = cwd.join(".vis/pkg").join(&g.name).join(&g.version);
        let _ = std::fs::remove_dir_all(&dich);
        for (path, text) in g.files {
            let p = dich.join(&path);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)
                    .map_err(|e| format!("không tạo được `{}`: {e}", d.display()))?;
            }
            std::fs::write(&p, text.as_bytes())
                .map_err(|e| format!("lỗi ghi `{}`: {e}", p.display()))?;
        }
    }
    Ok(())
}

/// `vis pkg list` — `tên ver sha256[:12]`, sắp theo tên.
fn lenh_list(args: &[String]) -> Result<(), String> {
    let (ten, reg) = tuy_chon(args, false)?;
    if ten.is_some() || reg.is_some() {
        return Err(format!("`list` không nhận đối số — {}", dung()));
    }
    let cwd = cwd()?;
    let khoa = doc_khoa(&cwd.join("vis.lock"), true)?;
    for p in &khoa {
        println!(
            "{} {} {}",
            p.name,
            p.version,
            &p.sha256[..12.min(p.sha256.len())]
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bam, bam_cay, chuan_hoa, cmp_phien_ban, so_phien_ban};

    /// CRLF/BOM khác nhau nhưng chuẩn hoá ⇒ **cùng** băm; đổi 1 byte ⇒ đổi băm; ghim 1 hex.
    #[test]
    fn bam_chuan_hoa_crlf_bom() {
        let a = chuan_hoa(b"x").unwrap();
        let b = chuan_hoa(b"x\r\n").unwrap();
        let c = chuan_hoa(b"\xef\xbb\xbfx\r\n").unwrap();
        assert_eq!(a, "x\n");
        assert_eq!(a, b);
        assert_eq!(a, c);
        let fs = bam(&a);
        assert_eq!(
            fs,
            "73cb3858a687a8494ca3323053016282f3dad39d42cf62ca4e79dda2aac7d9ac"
        );
        assert_eq!(
            bam_cay(&[("a.vis".to_string(), fs.clone())]),
            "510fb9cc152028b1f61fe4934b1a7a8555b1d0787cce5e70d590a8b4539ff392"
        );
        let khac = bam(&chuan_hoa(b"y").unwrap());
        assert_ne!(fs, khac);
    }

    /// So version theo **số**: `1.10.0 > 1.9.0`; `1.0` = `1.0.0`.
    #[test]
    fn so_phien_ban_theo_so() {
        assert_eq!(
            cmp_phien_ban("1.10.0", "1.9.0").unwrap(),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            cmp_phien_ban("1.0.0", "1.0").unwrap(),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            cmp_phien_ban("1.9.0", "1.10.0").unwrap(),
            std::cmp::Ordering::Less
        );
        assert!(so_phien_ban("1.x").is_err());
        assert!(so_phien_ban("").is_err());
    }
}
