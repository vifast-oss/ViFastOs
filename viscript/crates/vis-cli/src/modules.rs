// SPDX-License-Identifier: MIT OR Apache-2.0
//! Nạp **đa tệp** (ST-25, §7.10): giải `use`, cấm vòng, hợp nhất HIR, giữ `dòng:cột` theo **tệp thật**.
//!
//! Mô hình: mỗi tệp là **một module**; `use ./util` nhập item `pub` của `util.vis` cùng thư mục;
//! `use std.x` ⇒ `VS0701` ("std chưa có — ST-26"), `use pkg.x` ⇒ `VS0701` ("gói ngoài chưa hỗ trợ").
//!
//! Thứ tự **tất định**: hậu duyệt DFS theo thứ tự `use` trong nguồn (không phụ thuộc `read_dir`), mỗi
//! tệp một lần, tệp chính **cuối**. Hậu duyệt ⇒ khi hạ một tệp, mọi kiểu `pub` của phụ thuộc đã nằm
//! trong bảng kiểu chung nên `RecId`/`EnumId` **ổn định** giữa các tệp.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use vis_diag::MODULE_CODES;
use vis_syntax::{Diagnostic, Mode, NodeKind, Span};
use vis_types::{
    check, lower_into, HItem, HStmt, Hir, LowerOptions, ModInfo, NamePolicy, Symbols, TraitSig,
    Type, TypeTable,
};

/// Một tệp đã nạp.
#[derive(Debug, Clone)]
pub struct FileUnit {
    /// Đường dẫn tuyệt đối (đã `canonicalize`) — dùng cho `--dump-deps`/test.
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Đường dẫn tương đối so với **tệp chính** (ổn định khi đổi `cwd`).
    pub rel: String,
    pub text: String,
}

/// Kết quả nạp một đơn vị biên dịch.
pub struct Loaded {
    /// Đường dẫn tuyệt đối của tệp chính.
    #[allow(dead_code)]
    pub main: PathBuf,
    /// Tệp theo **hậu duyệt** (phụ thuộc trước, tệp chính cuối).
    pub files: Vec<FileUnit>,
    pub hir: Hir,
    pub symbols: Symbols,
    pub types: TypeTable,
    /// Chẩn đoán **tĩnh** kèm tên tệp: `(rel, diag)`.
    pub diags: Vec<(String, Diagnostic)>,
    /// Cây phụ thuộc cho `--dump-deps`: `rel → [rel phụ thuộc trực tiếp]`.
    pub deps: Vec<(String, Vec<String>)>,
}

impl Loaded {
    /// Tổng byte của cả đơn vị (dùng cho log/đo).
    #[allow(dead_code)]
    pub fn tong_byte(&self) -> usize {
        self.files.iter().map(|f| f.text.len()).sum()
    }

    /// Tệp chính (phần tử cuối của `files`, thứ tự hậu duyệt).
    #[allow(dead_code)]
    pub fn tep_chinh(&self) -> Option<&FileUnit> {
        self.files.last()
    }
}

/// Lỗi **cấp API** khi nạp (không phải chẩn đoán ngôn ngữ).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Không phải tệp.
    NotFile(String),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::NotFile(p) => write!(f, "không phải tệp: `{p}`"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mau {
    DangTham,
    Xong,
}

fn chuan_hoa(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn rel_cua(main_dir: &Path, p: &Path) -> String {
    let c = chuan_hoa(p);
    let goc = chuan_hoa(main_dir);
    match c.strip_prefix(&goc) {
        Ok(r) => r.to_string_lossy().replace('\\', "/"),
        Err(_) => c
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    }
}

/// Ứng viên tệp theo chế độ: `.vish` trước ở chế độ lệnh, `.vis` trước ở chế độ script.
fn ung_vien(goc: &Path, ten: &str, mode: Mode) -> Vec<PathBuf> {
    let base = goc.join(ten);
    if base.extension().is_some() {
        return vec![base];
    }
    let (a, b) = if mode == Mode::Command {
        ("vish", "vis")
    } else {
        ("vis", "vish")
    };
    vec![base.with_extension(a), base.with_extension(b)]
}

/// Vị trí `dòng:cột` (cột theo scalar, tab = 4 như `lower`).
pub fn vi_tri(text: &str, span: Span) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, c) in text.char_indices() {
        if i >= span.start {
            break;
        }
        match c {
            '\n' => {
                line += 1;
                col = 1;
            }
            '\t' => col += 4,
            _ => col += 1,
        }
    }
    (line, col)
}

fn span_cua(it: &HItem) -> Span {
    match it {
        HItem::Fn(f) => f.span,
        HItem::Rec(r) => r.span,
        HItem::Enum(e) => e.span,
        HItem::Alias(a) => a.span,
        HItem::Extern(e) => e.span,
        HItem::Const(c) => c.span,
        HItem::Use(u) => u.span,
        HItem::Trait(t) => t.span,
        HItem::Impl(i) => i.span,
    }
}

struct Nap {
    mode: Mode,
    main: PathBuf,
    main_dir: PathBuf,
    symbols: Symbols,
    types: TypeTable,
    files: Vec<FileUnit>,
    items: Vec<HItem>,
    item_rel: Vec<String>,
    stmts: Vec<HStmt>,
    span: Span,
    mod_cua_rel: BTreeMap<String, u16>,
    diags: Vec<(String, Diagnostic)>,
    deps: Vec<(String, Vec<String>)>,
    mau: BTreeMap<String, Mau>,
    pub_types: BTreeMap<String, Vec<(String, Type)>>,
    /// **ST-57 (VD-98 (1))**: `trait` `pub` của từng tệp — để tệp `use` nó kiểm `impl`/siêu trait.
    pub_traits: BTreeMap<String, Vec<TraitSig>>,
    cho_ngoai: bool,
}

impl Nap {
    fn bao(&mut self, rel: &str, code: &'static vis_diag::Code, msg: String, span: Span) {
        let text = self
            .files
            .iter()
            .find(|f| f.rel == rel)
            .map(|f| f.text.clone())
            .or_else(|| {
                if rel == rel_cua(&self.main_dir, &self.main) {
                    std::fs::read_to_string(&self.main).ok()
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let (line, col) = vi_tri(&text, span);
        self.diags.push((
            rel.to_string(),
            Diagnostic {
                code,
                message: msg,
                span,
                line,
                col,
            },
        ));
    }

    /// Giải một `use` thành đường dẫn tệp (hoặc lỗi `07xx`).
    fn giai_use(
        &self,
        seg: &[String],
        tu_dir: &Path,
    ) -> Result<PathBuf, (&'static vis_diag::Code, String)> {
        let dau = seg.first().map(String::as_str).unwrap_or("");
        if dau == "std" {
            return Err((&MODULE_CODES[0], "`std` chưa có — ST-26".to_string()));
        }
        if dau == "pkg" {
            return Err((
                &MODULE_CODES[0],
                "gói ngoài chưa hỗ trợ (VS2): `pkg.*` hoãn".to_string(),
            ));
        }
        let ten = seg.join("/");
        for c in ung_vien(tu_dir, &ten, self.mode) {
            if c.is_file() {
                return Ok(chuan_hoa(&c));
            }
        }
        Err((
            &MODULE_CODES[0],
            format!("không thấy tệp module `{ten}` (đã thử .vis/.vish)"),
        ))
    }

    fn tham(&mut self, path: &Path) {
        let path = chuan_hoa(path);
        let rel = rel_cua(&self.main_dir, &path);
        if self.mau.contains_key(&rel) {
            return;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.diags.push((
                    rel.clone(),
                    Diagnostic {
                        code: &MODULE_CODES[0],
                        message: format!("không đọc được tệp: {e}"),
                        span: Span::new(0, 0),
                        line: 1,
                        col: 1,
                    },
                ));
                return;
            }
        };
        let mode = if path.extension().map(|x| x == "vish").unwrap_or(false) {
            Mode::Command
        } else {
            self.mode
        };
        let parsed = vis_syntax::parse(&text, mode);
        let lexed = vis_syntax::lex(&text, mode);
        if !parsed.diagnostics.is_empty() {
            for d in &parsed.diagnostics {
                self.diags.push((rel.clone(), d.clone()));
            }
            self.mau.insert(rel, Mau::Xong);
            return;
        }
        let mod_id = match self.mod_cua_rel.get(&rel) {
            Some(id) => *id,
            None => {
                let id = self.symbols.add_mod(ModInfo {
                    name: path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    path: rel.clone(),
                });
                self.mod_cua_rel.insert(rel.clone(), id);
                id
            }
        };
        self.mau.insert(rel.clone(), Mau::DangTham);
        // 1) Giải `use` từ CST (chưa cần hạ).
        let mut ds: Vec<(Vec<String>, Span)> = Vec::new();
        for n in parsed.cst.root.children() {
            if n.kind != NodeKind::UseDecl {
                continue;
            }
            let Some(dp) = n.child(NodeKind::DottedPath) else {
                continue;
            };
            let raw = text
                .get(dp.span.start..dp.span.end)
                .unwrap_or("")
                .trim()
                .to_string();
            let seg: Vec<String> = if raw.contains('/') {
                vec![raw]
            } else {
                raw.split('.').map(str::to_string).collect()
            };
            ds.push((seg, n.span));
        }
        let tu_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.main_dir.clone());
        let mut con: Vec<String> = Vec::new();
        let mut da_co: Vec<String> = Vec::new();
        let mut std_mods: Vec<String> = Vec::new();
        for (seg, span) in ds {
            // ST-26: `use std.<module>` — module **trong Rust**, không phải tệp.
            if seg.first().map(String::as_str) == Some("std") {
                let m2 = seg.get(1).cloned().unwrap_or_default();
                if vis_types::stdlib::module_ho_tro(&m2) {
                    if std_mods.contains(&m2) {
                        self.bao(
                            &rel,
                            &MODULE_CODES[2],
                            format!("`use std.{m2}` trùng"),
                            span,
                        );
                    } else {
                        std_mods.push(m2);
                    }
                } else if m2 == "fs" {
                    self.bao(
                        &rel,
                        &MODULE_CODES[0],
                        "`std.fs` chưa có — ST-27 (cần host)".to_string(),
                        span,
                    );
                } else if vis_types::stdlib::MODULES_HOAN.contains(&m2.as_str()) {
                    self.bao(
                        &rel,
                        &MODULE_CODES[0],
                        format!("`std.{m2}` chưa hỗ trợ (VS1b)"),
                        span,
                    );
                } else {
                    self.bao(
                        &rel,
                        &MODULE_CODES[0],
                        format!(
                            "không có module `std.{m2}` (có: str/list/map/path/time/json/fs/net/http)"
                        ),
                        span,
                    );
                }
                continue;
            }
            match self.giai_use(&seg, &tu_dir) {
                Ok(dich) => {
                    let d_rel = rel_cua(&self.main_dir, &dich);
                    if da_co.contains(&d_rel) {
                        self.bao(
                            &rel,
                            &MODULE_CODES[2],
                            format!("`use` trùng module `{d_rel}` (một module, nhiều mục `use`)"),
                            span,
                        );
                        continue;
                    }
                    da_co.push(d_rel.clone());
                    if self.mau.get(&d_rel) == Some(&Mau::DangTham) {
                        self.bao(
                            &rel,
                            &MODULE_CODES[1],
                            format!("vòng phụ thuộc: `{rel}` → `{d_rel}`"),
                            span,
                        );
                        continue;
                    }
                    con.push(d_rel.clone());
                    self.tham(&dich);
                }
                Err((code, msg)) => self.bao(&rel, code, msg, span),
            }
        }
        self.deps.push((rel.clone(), con.clone()));
        // 2) Hạ tệp **vào bảng chung** (phụ thuộc đã hạ xong ⇒ kiểu `pub` sẵn sàng).
        let imports: Vec<u16> = con
            .iter()
            .filter_map(|r| self.mod_cua_rel.get(r).copied())
            .collect();
        let mut imported_types: Vec<(String, Type)> = Vec::new();
        let mut imported_traits: Vec<TraitSig> = Vec::new();
        for r in &con {
            if let Some(ds) = self.pub_types.get(r) {
                imported_types.extend(ds.iter().cloned());
            }
            if let Some(ts) = self.pub_traits.get(r) {
                imported_traits.extend(ts.iter().cloned());
            }
        }
        // Chính sách tên: tệp **không** `use` ai và không bị `use` thì giữ **Permissive** như trước
        // (VD-45: tên tự do ⇒ lỗi runtime `VS0501`); tệp trong module dùng **Strict** để bắt
        // tầm nhìn `pub`/`use` ngay khi biên dịch.
        let trong_module = !con.is_empty() || self.mod_cua_rel.len() > 1;
        let names = if trong_module {
            NamePolicy::Strict
        } else {
            NamePolicy::Permissive
        };
        let opts = LowerOptions {
            names,
            mod_id,
            imports,
            imported_types,
            trait_ngoai: imported_traits,
            std_mods,
            cho_ngoai: self.cho_ngoai,
        };
        let out = lower_into(
            &mut self.symbols,
            &mut self.types,
            &parsed.cst,
            &lexed,
            opts,
        );
        for d in &out.diagnostics {
            self.diags.push((rel.clone(), d.clone()));
        }
        let mut pubt: Vec<(String, Type)> = Vec::new();
        let mut pubt_trait: Vec<TraitSig> = Vec::new();
        for it in &out.hir.items {
            let la_pub =
                |sym: u32, s: &Symbols| s.pub_of.get(sym as usize).copied().unwrap_or(false);
            if let HItem::Trait(t) = it {
                if la_pub(t.sym, &self.symbols) {
                    pubt_trait.push(TraitSig {
                        name: t.name.clone(),
                        supertraits: t.supertraits.clone(),
                        methods: t
                            .methods
                            .iter()
                            .map(|m| {
                                (
                                    m.name.clone(),
                                    m.params.len(),
                                    m.ret.clone(),
                                    m.params.iter().map(|p| p.ty.clone()).collect(),
                                )
                            })
                            .collect(),
                    });
                }
            }
            match it {
                HItem::Rec(r) if la_pub(r.sym, &self.symbols) => {
                    pubt.push((r.name.clone(), Type::Record(r.id)));
                }
                HItem::Enum(e) if la_pub(e.sym, &self.symbols) => {
                    pubt.push((e.name.clone(), Type::Enum(e.id)));
                }
                HItem::Alias(a) if la_pub(a.sym, &self.symbols) => {
                    pubt.push((a.name.clone(), a.ty.clone()));
                }
                _ => {}
            }
        }
        self.pub_types.insert(rel.clone(), pubt);
        self.pub_traits.insert(rel.clone(), pubt_trait);
        for it in out.hir.items {
            self.item_rel.push(rel.clone());
            self.items.push(it);
        }
        if rel == rel_cua(&self.main_dir, &self.main) {
            self.stmts = out.hir.stmts;
            self.span = out.hir.span;
        }
        self.files.push(FileUnit {
            path: path.clone(),
            rel: rel.clone(),
            text,
        });
        self.mau.insert(rel, Mau::Xong);
    }
}

/// Nạp `main` + toàn bộ phụ thuộc (ST-25).
#[allow(dead_code)] // giữ API `load` (không cờ) cho test/đọc; sản phẩm dùng `load_with`.
pub fn load(main: &str) -> Result<Loaded, LoadError> {
    load_with(main, true)
}

/// Như [`load`] nhưng `cho_ngoai = false` ⇒ lệnh lạ báo `VS0601` (`vis check --no-extern`).
pub fn load_with(main: &str, cho_ngoai: bool) -> Result<Loaded, LoadError> {
    load_full(main, cho_ngoai, true)
}

/// Như [`load_with`] nhưng `kiem_kieu = false` ⇒ **chỉ cú pháp/hạ**, bỏ chẩn đoán `tyck`
/// (`vis check --syntax`, dùng cho cổng "khối mã trong tài liệu **parse** được" — QD §7.4).
pub fn load_full(main: &str, cho_ngoai: bool, kiem_kieu: bool) -> Result<Loaded, LoadError> {
    let main_path = PathBuf::from(main);
    if !main_path.is_file() {
        return Err(LoadError::NotFile(main.to_string()));
    }
    let mode = if main.ends_with(".vish") {
        Mode::Command
    } else {
        Mode::Script
    };
    let main_dir = main_path
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut n = Nap {
        mode,
        main: chuan_hoa(&main_path),
        main_dir: main_dir.clone(),
        symbols: Symbols::default(),
        types: TypeTable::new(),
        files: Vec::new(),
        items: Vec::new(),
        item_rel: Vec::new(),
        stmts: Vec::new(),
        span: Span::new(0, 0),
        mod_cua_rel: BTreeMap::new(),
        diags: Vec::new(),
        deps: Vec::new(),
        mau: BTreeMap::new(),
        pub_types: BTreeMap::new(),
        pub_traits: BTreeMap::new(),
        cho_ngoai,
    };
    n.tham(&main_path);
    let rel_chinh = rel_cua(&main_dir, &n.main);
    let hir = Hir {
        items: std::mem::take(&mut n.items),
        stmts: std::mem::take(&mut n.stmts),
        span: n.span,
    };
    // `check` **một** lần trên HIR hợp nhất; quy chẩn đoán về tệp có item chứa span.
    let kq = if kiem_kieu {
        check(&hir, &n.symbols, &n.types)
    } else {
        vis_types::tyck::CheckOutput {
            diags: Vec::new(),
            types_of_expr: Vec::new(),
            file_expr_types: Vec::new(),
            item_types: Vec::new(),
        }
    };
    let item_rel = n.item_rel.clone();
    for d in &kq.diags {
        let rel = hir
            .items
            .iter()
            .enumerate()
            .find(|(_, it)| {
                let s = span_cua(it);
                d.span.start >= s.start && d.span.start <= s.end
            })
            .map(|(i, _)| {
                item_rel
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| rel_chinh.clone())
            })
            .unwrap_or_else(|| rel_chinh.clone());
        // Tyck không có văn bản ⇒ tự tính `dòng:cột` theo **tệp được gán** (ST-25 mục 3).
        let mut d2 = d.clone();
        if let Some(f) = n.files.iter().find(|f| f.rel == rel) {
            let (line, col) = vi_tri(&f.text, d2.span);
            d2.line = line;
            d2.col = col;
        }
        n.diags.push((rel, d2));
    }
    Ok(Loaded {
        main: n.main,
        files: n.files,
        hir,
        symbols: n.symbols,
        types: n.types,
        diags: n.diags,
        deps: n.deps,
    })
}

/// In cây phụ thuộc (cho `vis run --dump-deps`), thứ tự hậu duyệt đã sắp.
pub fn dump_deps(l: &Loaded) -> String {
    let mut s = String::new();
    for (rel, con) in &l.deps {
        if con.is_empty() {
            s.push_str(&format!("{rel}\n"));
        } else {
            s.push_str(&format!("{rel} -> {}\n", con.join(", ")));
        }
    }
    s
}
