// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hạ **CST → HIR** + giải tên cho chế độ script (ST-11).
//!
//! # Hợp đồng (VD-44)
//!
//! 1. **Permissive ở tầng hạ**: biểu diễn được **mọi** thứ trong Phụ lục A ở mức cú pháp —
//!    `float`, `cap[Dir]`, `ref(0)`, `async fn`, `fn f[T](…)`, `derives`, `scope s`,
//!    `cmd { … }`, `await`… Hạ **không** báo "tính năng chưa hỗ trợ"; giới hạn VS0 chỉ báo ở
//!    tầng kiểm kiểu (ST-12). 15 khối ```vis đủ thân của tài liệu phải hạ **0 chẩn đoán**.
//! 2. **Giải tên chỉ cho tên GIÁ TRỊ** (biến, tham số, hàm, bản ghi/enum đã khai). Khai lại
//!    **cùng khung** ⇒ `VS0302`. Tên **kiểu** chưa phân giải giữ dạng [`Type::TypeRef`] — ST-11
//!    **không** báo `VS0301` cho kiểu (ST-12 làm).
//! 3. **Không prelude**: đúng 8 tên giá trị dựng sẵn [`BUILTIN_VALUE_NAMES`] (khoá bằng test).
//! 4. `VS0301` (**tên giá trị chưa khai**): mặc định [`NamePolicy::Permissive`] (VD-45) **chỉ**
//!    báo khi **gán** cho tên chưa khai — chỗ chắc chắn sai; [`NamePolicy::Strict`] báo cho **mọi**
//!    lần dùng chưa phân giải và sẽ là mặc định từ ST-13/14. Lý do: 30 golden biểu thức và nhiều
//!    khối tài liệu dùng tên tự do (`a`, `b`, `xs`, `user`, `json`…) — xem `JOURNAL` ST-11.
//! 5. **Không panic** trên dữ liệu người dùng (CST của tệp parse lỗi, 7 khối loại trừ, rác):
//!    thiếu node ⇒ [`HExpr::Error`]/bỏ qua, không `unwrap`/`todo!`/`unimplemented!`.
//! 6. `Mode::Command` **không** hạ bằng `lower` (VD-22 — `cmd_lower` là VS1): trả
//!    [`LowerError::CommandMode`] tường minh trong [`HirOutput::error`], không panic.

use vis_diag::NAME_CODES;
use vis_syntax::{Cst, Diagnostic, Elem, Kind, LexOutput, Node, NodeKind, Span, Token};

use crate::cmdsig::SigSource;
use crate::hir::{
    BinOp, FnKind, HAlias, HConst, HEnum, HExpr, HField, HFn, HImpl, HItem, HParam, HRec, HSig,
    HStmt, HTrait, HUse, HVariant, Hir, LitKind, UnOp,
};

mod generic;
mod trait_impl;
use crate::ty::{EnumDef, EnumId, FieldDef, RecDef, RecId, Type, TypeTable, VariantDef};
pub use trait_impl::TraitSig;

mod chieu;
mod cmd;
mod expr;
mod khai;
mod pat;
mod stmt;
mod sugar;
mod than;
pub use than::is_builtin_value;
pub(crate) use than::{lower_nhom, lower_than, ThanCtx};

/// **8 tên giá trị dựng sẵn** của VS0 — danh sách **khoá bằng test** (VD-44 #3: thêm/bớt phải
/// qua Agent 0). `true`/`false`/`none` là **literal**, không phải tên.
pub const BUILTIN_VALUE_NAMES: [&str; 11] = [
    "print", "len", "str", "echo", "lines", "take", "Ok", "Err", "Some", "None", "ref",
];

mod options;
pub use options::{LowerOptions, NamePolicy};

/// Lỗi **cấp API** (không phải chẩn đoán ngôn ngữ): đầu vào ngoài phạm vi `lower`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowerError {
    /// CST sinh từ `parse(.., Mode::Command)` — cần `cmd_lower` (VD-22, VS1).
    CommandMode,
}

impl LowerError {
    /// Thông điệp ổn định cho người dùng/log.
    pub const fn message(self) -> &'static str {
        match self {
            LowerError::CommandMode => {
                "chế độ lệnh (.vish) cần `cmd_lower` (VD-22, VS1) — `lower` chỉ hạ chế độ script"
            }
        }
    }
}

/// Kết quả một lượt hạ.
#[derive(Debug, Clone)]
pub struct HirOutput {
    pub hir: Hir,
    /// Chẩn đoán **ngôn ngữ** (`VS03xx`) — không chứa lỗi cấp API.
    pub diagnostics: Vec<Diagnostic>,
    pub symbols: Symbols,
    pub types: TypeTable,
    /// Lỗi cấp API (vd hạ CST chế độ lệnh); `None` khi hạ bình thường.
    pub error: Option<LowerError>,
}

impl HirOutput {
    /// `true` nếu không có chẩn đoán và không có lỗi cấp API.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty() && self.error.is_none()
    }

    /// Mã chẩn đoán theo thứ tự (tiện cho log/test).
    pub fn codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.short).collect()
    }
}

/// Loại khung tên (phạm vi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    File,
    Fn,
    Block,
}

/// Một khung tên. `parent` trỏ khung ngoài.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HFrame {
    pub parent: Option<u32>,
    pub span: Span,
    pub kind: FrameKind,
}

/// Loại khai báo trong bảng ký hiệu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymKind {
    /// Tên giá trị dựng sẵn ([`BUILTIN_VALUE_NAMES`]).
    Builtin,
    /// Khai báo cấp tệp (fn/type/enum/alias/const/…) hoặc phương thức.
    Item,
    Param,
    /// `var` (biến đổi được).
    Var,
    PatBind,
    ElseBind,
    /// Tham số kiểu của hàm (`fn f[T]`).
    Generic,
    /// Nhãn `scope s`.
    ScopeBind,
}

/// Một ký hiệu: tên + loại + khung + vị trí (+ kiểu nếu biết).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HSym {
    pub name: String,
    pub kind: SymKind,
    pub frame: u32,
    pub span: Span,
    pub ty: Option<Type>,
}

/// Thông tin **module** (ST-25): một tệp nguồn trong đơn vị biên dịch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModInfo {
    /// Tên hiển thị (tên tệp, không kèm thư mục).
    pub name: String,
    /// Đường dẫn tương đối tới tệp chính (ổn định khi đổi `cwd`).
    pub path: String,
}

/// Bảng ký hiệu của một lượt hạ: các khung + ký hiệu theo thứ tự khai báo.
///
/// ST-25 thêm ba mảng **song song** với `syms` để kiểm tầm nhìn `pub`/`use` **ở tầng giải tên**
/// (`mod_of` = module sở hữu, `pub_of` = có `pub` hay không) và `mods` = danh sách module.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Symbols {
    pub frames: Vec<HFrame>,
    pub syms: Vec<HSym>,
    /// Module sở hữu từng ký hiệu; `u16::MAX` = dựng sẵn (luôn thấy).
    pub mod_of: Vec<u16>,
    /// Ký hiệu có `pub` hay không (chỉ có nghĩa với item cấp tệp).
    pub pub_of: Vec<bool>,
    /// Các module của đơn vị biên dịch (chỉ số = `mod_of`).
    pub mods: Vec<ModInfo>,
}

impl Symbols {
    /// Thêm một module, trả id.
    pub fn add_mod(&mut self, m: ModInfo) -> u16 {
        let id = self.mods.len() as u16;
        self.mods.push(m);
        id
    }
}

impl Symbols {
    pub fn get(&self, id: u32) -> Option<&HSym> {
        self.syms.get(id as usize)
    }

    /// Tên của một ký hiệu.
    pub fn name_of(&self, id: u32) -> Option<&str> {
        self.get(id).map(|s| s.name.as_str())
    }

    /// Ký hiệu theo tên trong **một** khung (lần khai báo cuối trong khung đó).
    pub fn in_frame(&self, frame: u32, name: &str) -> Option<&HSym> {
        self.syms
            .iter()
            .rev()
            .find(|s| s.frame == frame && s.name == name)
    }

    /// Mọi ký hiệu mang `name` (theo thứ tự khai báo).
    pub fn all_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a HSym> {
        self.syms.iter().filter(move |s| s.name == name)
    }

    /// Ký hiệu theo tên + loại (tiện cho test khung).
    pub fn kind_of(&self, id: u32) -> Option<SymKind> {
        self.get(id).map(|s| s.kind)
    }
}

/// Hạ một CST chế độ script với tuỳ chọn mặc định ([`LowerOptions::default`]).
pub fn lower(cst: &Cst, lex: &LexOutput) -> HirOutput {
    lower_with(cst, lex, LowerOptions::default())
}

/// Hạ một CST chế độ script với tuỳ chọn cho trước (bảng ký hiệu/kiểu **mới**).
pub fn lower_with(cst: &Cst, lex: &LexOutput, opts: LowerOptions) -> HirOutput {
    let mut symbols = Symbols::default();
    let mut types = TypeTable::new();
    lower_into(&mut symbols, &mut types, cst, lex, opts)
}

/// Hạ với **nguồn chữ ký lệnh** tự chọn (ST-37): `vis-std`/prelude (ST-40/41) cắm `SigSource`
/// riêng mà **không** phải sửa `lower`/`tyck`. Nguồn mặc định là 3 builtin (`BuiltinSigs`).
pub fn lower_with_sigs(
    cst: &Cst,
    lex: &LexOutput,
    opts: LowerOptions,
    src: &dyn SigSource,
) -> HirOutput {
    let mut symbols = Symbols::default();
    let mut types = TypeTable::new();
    let mut l = Lowerer::new(cst, lex, opts, &mut symbols, &mut types);
    l.cmd_sigs = src.sigs().to_vec();
    l.run();
    l.finish()
}

/// Hạ một tệp **vào bảng chung** (ST-25): ký hiệu và kiểu được **append** ⇒ id ổn định giữa các tệp.
///
/// `symbols.mods` do nơi gọi khai bằng [`Symbols::add_mod`] trước khi hạ tệp tương ứng; tên item
/// cấp tệp của tệp mới bắt đầu từ `symbols.syms.len()`.
pub fn lower_into(
    symbols: &mut Symbols,
    types: &mut TypeTable,
    cst: &Cst,
    lex: &LexOutput,
    opts: LowerOptions,
) -> HirOutput {
    let mut l = Lowerer::new(cst, lex, opts, symbols, types);
    l.run();
    l.finish()
}

/// Template biến thể enum generic (ST-35): `(tên, tham số kiểu, [(biến thể, kiểu trường)])`.
type EnumTmpl = (String, Vec<String>, Vec<(String, Vec<Type>)>);

struct Lowerer<'a> {
    cst: &'a Cst,
    text: &'a str,
    opts: LowerOptions,
    diags: Vec<Diagnostic>,
    error: Option<LowerError>,
    symbols: &'a mut Symbols,
    /// Bảng kiểu **chung** của đơn vị biên dịch (ST-25: id ổn định, append theo tệp).
    tbl: &'a mut TypeTable,
    /// Chồng khung hiện tại; đáy luôn là khung tệp `0`.
    scope: Vec<u32>,
    items: Vec<HItem>,
    stmts: Vec<HStmt>,
    /// Tham số kiểu đang có hiệu lực (`type Hop[T]`, `enum Or[A, B]`) — ST-28: xoá kiểu.
    generic_scope: Vec<String>,
    impls: Vec<(Type, String, u32)>,
    /// ST-29: trait nội tệp để kiểm `impl` khớp trait; **ST-57**: kèm **siêu trait**.
    traits: Vec<TraitSig>,
    /// **ST-57**: trait `pub` nhập từ module khác (mồ côi đa tệp + siêu trait).
    traits_ngoai: Vec<TraitSig>,
    /// **ST-57**: `(kiểu, trait)` của mọi `impl Trait for Type` trong tệp (kiểm siêu trait **một tầng**).
    impl_pairs: Vec<(Type, String, Span)>,
    /// **ST-57**: tên kiểu **nhập** từ module khác — mồ côi **thật** = trait nhập *và* kiểu nhập.
    kieu_ngoai: Vec<String>,
    /// **ST-57**: `impl` **sinh** từ `derives` (nối cuối `items` ở cuối `run()`).
    derived: Vec<HItem>,
    /// ST-29: kiểu tĩnh của biến (chỉ từ **chú thích** tường minh: tham số + `let`/`var`).
    type_env: std::collections::HashMap<u32, Type>,
    /// ST-29: tên kiểu của `impl` đang hạ (để tái dùng sym đã khai ở lượt thu thập).
    cur_impl: Option<String>,
    /// ST-35: **template** trường của `type T[..]` — kiểu còn **ký hiệu** `TypeRef{"T"}` để thay khi
    /// instantiation (`Hop[int]`), thay vì xoá kiểu như ST-28.
    rec_tmpl: Vec<(String, Vec<String>, Vec<Type>)>,
    /// ST-35: template biến thể của `enum E[..]` — `(tên enum, tham số, [(biến thể, kiểu trường)])`.
    enum_tmpl: Vec<EnumTmpl>,
    /// ST-35: cache instantiation `(tên gốc, đối số) → id`.
    inst_cache: Vec<((String, Vec<Type>), u32)>,
    /// ST-35: `true` khi đang dựng **template** ⇒ tham số kiểu giữ dạng `TypeRef{"T"}`.
    sym_generic: bool,
    /// ST-37: chữ ký lệnh chế độ shell (builtin + `cmd`) — **một** nguồn cho khe ống và kiểm gọi.
    cmd_sigs: Vec<crate::cmdsig::CmdSig>,
    /// ST-38: bộ đếm tên tạm cho **sugar `it`** (tránh trùng `__c` giữa nhiều vị từ trong tệp).
    tam_n: u32,
    /// ST-46: `RecId` của `FileEntryLong` (prelude) — `ls --long` trả `list` kiểu này.
    file_long: Option<RecId>,
    /// ST-41: kiểu (tĩnh) của **stage cuối** vừa hạ — để `boc_render` biết phần tử có phải **bản ghi**
    /// (⇒ bảng / `(rỗng)`) hay không (`list[str]`/`list[int]` rỗng giữ `[]`).
    last_stage_ty: Type,
    /// ST-40a: bản ghi **prelude** của `.vish` (`FileEntry`, `ProcEntry`) — để `kieu_text` nhận literal
    /// bản ghi có đúng bộ trường và cho `it` **kiểu thật**.
    prelude_recs: Vec<(String, RecId)>,
    rec_names: Vec<(String, RecId)>,
    enum_names: Vec<(String, EnumId)>,
    /// Tên biến thể trần → (enum, chỉ số biến thể).
    variant_index: Vec<(String, EnumId, usize)>,
    aliases: Vec<(String, Type)>,
    /// Tên item cấp tệp → chỉ số ký hiệu (khai ở lượt 1).
    item_syms: Vec<(String, u32)>,
    /// Đang hạ vế sau của or-pattern: khai tên đã có thì **dùng lại**, không báo `VS0302`.
    suppress_dup: bool,
    /// Đang hạ **đích gán**: hoãn `VS0301` của [`Lowerer::lower_var_use`] để `lower_assign` báo
    /// **một** lần ở gốc lvalue (tránh 2 chẩn đoán cho cùng một `x = 1`).
    suppress_name_diag: bool,
}

impl<'a> Lowerer<'a> {
    fn new(
        cst: &'a Cst,
        lex: &'a LexOutput,
        opts: LowerOptions,
        symbols: &'a mut Symbols,
        tbl: &'a mut TypeTable,
    ) -> Self {
        // ST-25: kiểu **công khai** của các module đã `use` được nhập vào bảng tên của tệp này.
        let mut rec_names: Vec<(String, RecId)> = Vec::new();
        let mut enum_names: Vec<(String, EnumId)> = Vec::new();
        let mut aliases: Vec<(String, Type)> = Vec::new();
        for (n, t) in &opts.imported_types {
            match t {
                Type::Record(id) => rec_names.push((n.clone(), *id)),
                Type::Enum(id) => enum_names.push((n.clone(), *id)),
                other => aliases.push((n.clone(), other.clone())),
            }
        }
        Self {
            generic_scope: Vec::new(),
            impls: Vec::new(),
            traits: Vec::new(),
            traits_ngoai: opts.trait_ngoai.clone(),
            impl_pairs: Vec::new(),
            kieu_ngoai: opts.imported_types.iter().map(|(n, _)| n.clone()).collect(),
            derived: Vec::new(),
            rec_tmpl: Vec::new(),
            enum_tmpl: Vec::new(),
            inst_cache: Vec::new(),
            sym_generic: false,
            cmd_sigs: crate::cmdsig::BuiltinSigs::mac_dinh().sigs().to_vec(),
            tam_n: 0,
            file_long: None,
            prelude_recs: Vec::new(),
            last_stage_ty: Type::Error,
            type_env: std::collections::HashMap::new(),
            cur_impl: None,
            cst,
            text: &lex.text,
            opts,
            diags: Vec::new(),
            error: None,
            symbols,
            tbl,
            scope: Vec::new(),
            items: Vec::new(),
            stmts: Vec::new(),
            rec_names,
            enum_names,
            variant_index: Vec::new(),
            aliases,
            item_syms: Vec::new(),
            suppress_dup: false,
            suppress_name_diag: false,
        }
    }

    fn run(&mut self) {
        // ST-25: đơn vị biên dịch **dùng chung** khung gốc `0` và bảng ký hiệu — tệp thứ hai trở đi
        // **không** tạo khung mới, để mọi item cấp tệp nằm cùng một phạm vi (tầm nhìn lọc bằng `use`).
        if self.symbols.frames.is_empty() {
            self.symbols.frames.push(HFrame {
                parent: None,
                span: self.cst.root.span,
                kind: FrameKind::File,
            });
        }
        self.scope.push(0);
        if self.command_mode() {
            // ST-18 (VD-22 pha 2): `.vish`/`cmd { … }` hạ xuống **cùng HIR** — `cmd_lower`.
            // ST-40a: prelude bản ghi **chỉ** cho chế độ lệnh, đăng ký **trước** khi hạ item để
            // `type FileEntry` của người dùng trùng tên bị `VS0302`.
            let prelude = self.dang_ky_prelude();
            // ST-44: **vùng khai báo** ở đầu tệp (type/enum/cmd/let…) hạ trước, vào **cùng**
            // bảng ký hiệu/kiểu; stage chạy sau với mốc tách để **bỏ** các dòng đã hạ.
            let (moc, co_khai) = khai::tach(self.text);
            // ST-48: thân `cmd` **lệnh** thay bằng `()` trong bản sao tiền tố (giữ nguyên byte).
            let ds_than = khai::than_cmd(self.text);
            let tien_to = khai::filler(self.text, &ds_than);
            if co_khai && moc > 0 {
                self.ha_tien_to_khai(moc, &prelude, &tien_to);
                self.ha_than_cmd(&ds_than);
            } else if let Some((line, col)) = khai::khai_sau_stage(self.text, 0) {
                // Tệp **không** mở đầu bằng khai báo nhưng có khai báo ở cột 1 ⇒ lỗi rõ.
                let sp = Lowerer::span_dong_cot(self.text, line, col);
                self.diag(
                    &vis_diag::SYNTAX_CODES[0],
                    "khai báo phải đứng **trước** stage đầu tiên (`.vish`: item ở đầu tệp, rồi mới\
                     tới pipeline)",
                    sp,
                );
            }
            self.run_shell_tu(moc);
            return;
        }
        self.declare_builtins();
        // Lượt 1 — "khai báo trước" (§14.3): tên item + id kiểu, để thân hàm gọi được hàm khai
        // sau nó và `type` tham chiếu được kiểu khai sau.
        for child in self.cst.root.children() {
            if is_item(child.kind) {
                self.predeclare_item(child);
            }
        }
        // ST-29: **lượt 1b** — thu thập `trait`/`impl` + sym phương thức (sau khi mọi `type`/`enum`
        // đã khai ⇒ `impl Show for Hop` phân giải được kể cả khi `type Hop` viết sau).
        for child in self.cst.root.children() {
            match child.kind {
                NodeKind::TraitDecl => self.collect_trait(child),
                NodeKind::ImplDecl => self.collect_impl(child),
                _ => {}
            }
        }
        // **ST-57 (VD-93/VD-98 (3))**: lượt **1c** — `derives Show/Eq` sinh `impl` **thật** **trước**
        // mọi lượt hạ thân (câu `p.show()` trước khai báo vẫn phân giải).
        // **ST-60c (B3)**: lượt 1c chạy **trước** khi kiểm siêu trait ⇒ `impl` **sinh từ `derives`**
        // đã nằm trong `impl_pairs` (trước: `impl TraitCon for T` bị `VS0403` oan).
        self.sinh_derives();
        // **ST-57 (VD-98 (2))**: siêu trait kiểm **một tầng**, **sau** 1b+1c ⇒ thứ tự `impl` tự do.
        self.kiem_sieu_trait();
        // Lượt 2 — thân item + câu lệnh cấp tệp, **theo đúng thứ tự nguồn**.
        for child in self.cst.root.children() {
            if is_item(child.kind) {
                if let Some(it) = self.build_item(child) {
                    self.items.push(it);
                }
            } else if let Some(s) = self.lower_stmt(child) {
                self.stmts.push(s);
            }
        }
        // **ST-57**: `impl` sinh từ `derives` nối **cuối** (sym/bảng `impls` đã có từ lượt 1c).
        self.items.append(&mut self.derived);
    }

    fn finish(self) -> HirOutput {
        let symbols = self.symbols.clone();
        let types = self.tbl.clone();
        HirOutput {
            hir: Hir {
                items: self.items,
                stmts: self.stmts,
                span: self.cst.root.span,
            },
            diagnostics: self.diags,
            symbols,
            types,
            error: self.error,
        }
    }

    // ----- tiện ích đọc CST -----

    fn tok(&self, i: u32) -> Option<&'a Token> {
        self.cst.tokens.get(i as usize)
    }

    fn text_of(&self, span: Span) -> &'a str {
        self.text.get(span.start..span.end).unwrap_or("")
    }

    /// Tên khai báo = token `Ident` đầu tiên của node (bỏ qua `pub`/`async`/từ khoá).
    fn decl_name(&self, node: &Node) -> Option<String> {
        node.tokens(&self.cst.tokens)
            .find(|t| t.kind == Kind::Ident)
            .map(|t| t.text.clone())
    }

    /// Token `Ident` đầu tiên + vị trí (dùng cho `var`/`scope`/pattern).
    fn first_ident(&self, node: &Node) -> Option<(String, Span)> {
        node.tokens(&self.cst.tokens)
            .find(|t| t.kind == Kind::Ident)
            .map(|t| (t.text.clone(), t.span))
    }

    fn has_token(&self, node: &Node, kw: &str) -> bool {
        node.tokens(&self.cst.tokens)
            .any(|t| t.kind == Kind::Kw && t.text == kw)
    }

    /// Vị trí `dòng:cột` (1-based; cột theo scalar, tab = 4 — SPEC-lex §4.7).
    fn pos(&self, span: Span) -> (u32, u32) {
        let end = span.start.min(self.text.len());
        let head = self.text.get(..end).unwrap_or("");
        let mut line = 1u32;
        let mut col = 1u32;
        for ch in head.chars() {
            match ch {
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

    fn diag(&mut self, code: &'static vis_diag::Code, msg: impl Into<String>, span: Span) {
        let (line, col) = self.pos(span);
        self.diags.push(Diagnostic {
            code,
            message: msg.into(),
            span,
            line,
            col,
        });
    }

    fn command_mode(&self) -> bool {
        self.cst.cmd_sexp.is_some()
            || self
                .cst
                .root
                .children()
                .any(|n| n.kind == NodeKind::CmdShell || n.kind == NodeKind::CmdItems)
    }

    // ----- khung và ký hiệu -----

    fn push_frame(&mut self, kind: FrameKind, span: Span) {
        let parent = self.scope.last().copied();
        let id = self.symbols.frames.len() as u32;
        self.symbols.frames.push(HFrame { parent, span, kind });
        self.scope.push(id);
    }

    fn pop_frame(&mut self) {
        if self.scope.len() > 1 {
            self.scope.pop();
        }
    }

    fn current_frame(&self) -> u32 {
        self.scope.last().copied().unwrap_or(0)
    }

    fn declare_builtins(&mut self) {
        if self.symbols.syms.iter().any(|s| s.kind == SymKind::Builtin) {
            return;
        }
        for name in BUILTIN_VALUE_NAMES {
            self.symbols.syms.push(HSym {
                name: name.to_string(),
                kind: SymKind::Builtin,
                frame: 0,
                span: Span::new(0, 0),
                ty: None,
            });
            // Dựng sẵn **luôn** thấy, không phụ thuộc module.
            self.symbols.mod_of.push(u16::MAX);
            self.symbols.pub_of.push(true);
        }
    }

    /// Khai một tên trong khung hiện tại; khai lại **cùng khung** ⇒ `VS0302` (VD-44 #2).
    fn declare(&mut self, name: &str, kind: SymKind, span: Span, ty: Option<Type>) -> u32 {
        let frame = self.current_frame();
        if let Some(prev) = self.symbols.in_frame(frame, name) {
            let (pl, pc) = self.pos(prev.span);
            let msg = format!("khai báo trùng `{name}` trong cùng phạm vi (đã khai ở {pl}:{pc})");
            self.diag(&NAME_CODES[1], msg, span);
        }
        let id = self.symbols.syms.len() as u32;
        self.symbols.syms.push(HSym {
            name: name.to_string(),
            kind,
            frame,
            span,
            ty,
        });
        self.symbols.mod_of.push(self.opts.mod_id);
        self.symbols.pub_of.push(false);
        id
    }

    /// Khai tên cho **vế sau** của or-pattern: đã có trong khung ⇒ dùng lại chỉ số, **không**
    /// báo `VS0302` (các vế or-pattern dùng chung một binding — §4.3).
    fn declare_or_reuse(&mut self, name: &str, span: Span) -> u32 {
        let frame = self.current_frame();
        if let Some(i) = self
            .symbols
            .syms
            .iter()
            .rposition(|s| s.frame == frame && s.name == name)
        {
            return i as u32;
        }
        self.declare(name, SymKind::PatBind, span, None)
    }

    /// Giải tên **giá trị** từ khung trong ra ngoài. `None` = chưa phân giải (tên tự do).
    ///
    /// ST-25: tên thuộc **module khác** chỉ thấy khi tệp hiện tại `use` module đó **và** mục có `pub`
    /// (mô hình "chỉ `pub`", §7.10). Tên dựng sẵn (`mod_of == u16::MAX`) luôn thấy.
    fn resolve(&self, name: &str) -> Option<u32> {
        for frame in self.scope.iter().rev() {
            for (i, s) in self.symbols.syms.iter().enumerate().rev() {
                if s.frame == *frame && s.name == name && self.duoc_thay(i) {
                    return Some(i as u32);
                }
            }
        }
        None
    }

    /// Ký hiệu thứ `i` có nằm trong tầm nhìn của tệp đang hạ không.
    fn duoc_thay(&self, i: usize) -> bool {
        let m = match self.symbols.mod_of.get(i) {
            Some(m) => *m,
            // Bảng cũ/không đồng bộ: coi như cùng module (an toàn cho API một tệp).
            None => return true,
        };
        if m == u16::MAX || m == self.opts.mod_id {
            return true;
        }
        self.opts.imports.contains(&m) && self.symbols.pub_of.get(i).copied().unwrap_or(false)
    }

    /// Tên module sở hữu ký hiệu đầu tiên mang `name` (dùng cho thông điệp `VS0301`).
    fn module_cua_ten(&self, name: &str) -> Option<&str> {
        let i = self.symbols.syms.iter().rposition(|s| s.name == name)?;
        let m = *self.symbols.mod_of.get(i)?;
        if m == u16::MAX {
            return None;
        }
        self.symbols.mods.get(m as usize).map(|x| x.name.as_str())
    }

    /// Hạ **một lần dùng tên giá trị**: phân giải + `VS0301` theo chính sách.
    fn lower_var_use(&mut self, name: &str, span: Span) -> HExpr {
        match self.resolve(name) {
            Some(sym) => HExpr::Var {
                name: name.to_string(),
                sym: Some(sym),
                span,
            },
            None => {
                if self.opts.names == NamePolicy::Strict && !self.suppress_name_diag {
                    // ST-25: nếu tên **có** ở module khác thì nêu tên module và lý do bị chặn.
                    let msg = match self.module_cua_ten(name) {
                        Some(m) => format!(
                            "không thấy tên `{name}`: module `{m}` chưa được `use` hoặc mục không `pub`"
                        ),
                        None => format!("tên không tồn tại: `{name}`"),
                    };
                    self.diag(&NAME_CODES[0], msg, span);
                }
                HExpr::Var {
                    name: name.to_string(),
                    sym: None,
                    span,
                }
            }
        }
    }

    fn item_sym(&self, name: &str) -> u32 {
        self.item_syms
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, id)| *id)
            .unwrap_or(0)
    }

    // ----- lượt 1: khai báo trước -----

    fn predeclare_item(&mut self, node: &Node) {
        match node.kind {
            NodeKind::TypeDecl => {
                let Some(name) = self.decl_name(node) else {
                    return;
                };
                let id = self.declare(&name, SymKind::Item, node.span, None);
                if self.has_token(node, "pub") {
                    if let Some(p) = self.symbols.pub_of.get_mut(id as usize) {
                        *p = true;
                    }
                }
                self.item_syms.push((name.clone(), id));
                let rid = RecId(self.tbl.recs().len() as u32);
                self.tbl.push_rec(RecDef {
                    name: Some(name.clone()),
                    fields: Vec::new(),
                    span: node.span,
                    anonymous: false,
                });
                self.rec_names.push((name, rid));
            }
            NodeKind::EnumDecl => {
                let Some(name) = self.decl_name(node) else {
                    return;
                };
                let id = self.declare(&name, SymKind::Item, node.span, None);
                if self.has_token(node, "pub") {
                    if let Some(p) = self.symbols.pub_of.get_mut(id as usize) {
                        *p = true;
                    }
                }
                self.item_syms.push((name.clone(), id));
                let eid = EnumId(self.tbl.enums().len() as u32);
                self.tbl.push_enum(EnumDef {
                    name: name.clone(),
                    variants: Vec::new(),
                    span: node.span,
                });
                self.enum_names.push((name, eid));
                let mut idx = 0usize;
                for v in node.children().filter(|n| n.kind == NodeKind::Variant) {
                    if let Some(vn) = self.decl_name(v) {
                        self.variant_index.push((vn, eid, idx));
                        idx += 1;
                    }
                }
            }
            NodeKind::FnDecl
            | NodeKind::CmdDecl
            | NodeKind::TestDecl
            | NodeKind::TraitDecl
            | NodeKind::ExternDecl
            | NodeKind::AliasDecl
            | NodeKind::NewtypeDecl
            | NodeKind::ConstDecl => {
                if let Some(name) = self.decl_name(node) {
                    let id = self.declare(&name, SymKind::Item, node.span, None);
                    // ST-25: `pub` quyết định tầm nhìn ra module khác (`let`/`const` **chưa** hỗ trợ
                    // `pub` — nợ VS1b, ghi JOURNAL).
                    if self.has_token(node, "pub") {
                        if let Some(p) = self.symbols.pub_of.get_mut(id as usize) {
                            *p = true;
                        }
                    }
                    self.item_syms.push((name, id));
                }
            }
            _ => {}
        }
    }

    // ----- lượt 2: dựng item -----

    fn build_item(&mut self, node: &Node) -> Option<HItem> {
        match node.kind {
            NodeKind::FnDecl => Some(HItem::Fn(self.lower_fn(node, FnKind::Fn))),
            NodeKind::CmdDecl => Some(HItem::Fn(self.lower_fn(node, FnKind::Cmd))),
            NodeKind::TestDecl => Some(HItem::Fn(self.lower_fn(node, FnKind::Test))),
            NodeKind::ExternDecl => Some(HItem::Extern(self.lower_extern(node))),
            NodeKind::TypeDecl => self.lower_rec(node).map(HItem::Rec),
            NodeKind::EnumDecl => self.lower_enum(node).map(HItem::Enum),
            NodeKind::AliasDecl | NodeKind::NewtypeDecl => self.lower_alias(node),
            NodeKind::ConstDecl => self.lower_const(node),
            NodeKind::UseDecl => {
                // ST-25: đường dẫn **tệp tương đối** (`./util`, `./a/b`) giữ nguyên trong **một**
                // đoạn (có `/`); đường dẫn chấm (`std.fs`, `pkg.http`) tách theo `.`.
                let path = node
                    .child(NodeKind::DottedPath)
                    .map(|p| self.text_of(p.span).trim().to_string())
                    .map(|raw| {
                        if raw.contains('/') {
                            vec![raw]
                        } else {
                            raw.split('.').map(str::to_string).collect()
                        }
                    })
                    .unwrap_or_default();
                Some(HItem::Use(HUse {
                    path,
                    span: node.span,
                }))
            }
            NodeKind::TraitDecl => self.lower_trait(node),
            NodeKind::ImplDecl => self.lower_impl(node),
            _ => None,
        }
    }

    fn generics_of(&self, node: &Node) -> Vec<String> {
        node.child(NodeKind::Generics)
            .map(|g| {
                g.children()
                    .filter(|n| n.kind == NodeKind::Generic)
                    .filter_map(|g| self.decl_name(g))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn lower_fn(&mut self, node: &Node, kind: FnKind) -> HFn {
        let name = self.decl_name(node).unwrap_or_default();
        let generics = self.generics_of(node);
        let is_pub = self.has_token(node, "pub");
        let is_async = self.has_token(node, "async");
        self.push_frame(FrameKind::Fn, node.span);
        let sym = match kind {
            FnKind::Fn | FnKind::Cmd | FnKind::Test => self.item_sym(&name),
            FnKind::ImplMethod => {
                // ST-29: sym đã khai ở **lượt 1b** (để call-site trỏ tới trước khi thân hạ).
                let tim = self.cur_impl.clone().and_then(|t| {
                    self.impls
                        .iter()
                        .find(|(ty, mn, _)| {
                            self.ten_kieu(ty).as_deref() == Some(t.as_str()) && mn == &name
                        })
                        .map(|(_, _, sym)| *sym)
                });
                tim.unwrap_or_else(|| self.declare(&name, SymKind::Item, node.span, None))
            }
            FnKind::TraitMethod => self.declare(&name, SymKind::Item, node.span, None),
        };
        for g in &generics {
            self.declare(g, SymKind::Generic, node.span, None);
        }
        // ST-35: tham số kiểu của **chính hàm** phải có hiệu lực khi hạ chữ ký — nhờ vậy `Hop[T]`
        // trong chú thích giữ được **ký hiệu** `T` (không xoá thành `{error}`) để tyck hợp nhất.
        let luu_gen = std::mem::replace(&mut self.generic_scope, generics.clone());
        // ST-35: trong **chữ ký** hàm generic, `T` giữ dạng **ký hiệu** (không xoá thành `{error}`) để
        // call-site hợp nhất được; thân hàm vẫn dùng bản đã xoá (tyck `erase_gen`).
        let luu_sym = std::mem::replace(&mut self.sym_generic, !generics.is_empty());
        let params = self.lower_params(node);
        let ret = node
            .child(NodeKind::Type)
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Unit);
        self.sym_generic = luu_sym;
        self.generic_scope = luu_gen;
        let body = node
            .child(NodeKind::Suite)
            .map(|s| self.lower_suite(s))
            .unwrap_or_default();
        self.pop_frame();
        HFn {
            name,
            span: node.span,
            sym,
            kind,
            is_pub,
            is_async,
            generics,
            params,
            ret,
            body,
        }
    }

    fn lower_params(&mut self, node: &Node) -> Vec<HParam> {
        let Some(params) = node.child(NodeKind::Params) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in params.children().filter(|n| n.kind == NodeKind::Param) {
            let is_self = self.has_token(p, "self");
            let is_var_self = is_self && self.has_token(p, "var");
            let named = p
                .tokens(&self.cst.tokens)
                .take(2)
                .filter(|t| t.is_punct("-"))
                .count()
                == 2;
            let name = match self.first_ident(p) {
                Some((n, _)) => n,
                None if is_self => "self".to_string(),
                None => continue,
            };
            // ST-27: `...rest: str` — lexer không có `...` ⇒ ba token `..` `.` `rest`.
            let variadic = p
                .tokens(&self.cst.tokens)
                .take(2)
                .filter(|t| t.is_punct("..") || t.is_punct("."))
                .count()
                >= 2
                && p.tokens(&self.cst.tokens).any(|t| t.is_punct(".."));
            let ty = match p.child(NodeKind::Type) {
                Some(t) => self.resolve_type(t),
                None => Type::Error,
            };
            // Giá trị mặc định: node con không phải `Type`/`GrantList` (đối số `@path` là token).
            let default = p
                .children()
                .filter(|c| c.kind != NodeKind::Type && c.kind != NodeKind::GrantList)
                .last()
                .map(|c| self.lower_expr(c));
            // ST-27: tham số variadic nhận **list** phần tử (vd `...rest: str` ⇒ `list[str]`).
            let ty = if variadic {
                Type::List(Box::new(ty))
            } else {
                ty
            };
            let sym = self.declare(&name, SymKind::Param, p.span, Some(ty.clone()));
            // ST-29: kiểu tĩnh của tham số (từ **chú thích**) để dispatch phương thức impl.
            if !matches!(ty, Type::Error) {
                self.type_env.insert(sym, ty.clone());
            }
            out.push(HParam {
                name,
                span: p.span,
                sym,
                ty,
                named,
                is_self,
                is_var_self,
                default,
                variadic,
            });
        }
        out
    }

    fn lower_extern(&mut self, node: &Node) -> HSig {
        let name = self.decl_name(node).unwrap_or_default();
        let sym = self.item_sym(&name);
        self.push_frame(FrameKind::Fn, node.span);
        let params = self.lower_params(node);
        self.pop_frame();
        HSig {
            name,
            span: node.span,
            sym,
            params,
        }
    }

    fn field_of(&mut self, f: &Node) -> HField {
        let name = self.decl_name(f).unwrap_or_default();
        let ty = match f.child(NodeKind::Type) {
            Some(t) => self.resolve_type(t),
            None => Type::Error,
        };
        let default = f
            .children()
            .filter(|c| c.kind != NodeKind::Type)
            .last()
            .map(|c| self.lower_expr(c));
        let has_default = default.is_some();
        HField {
            name,
            span: f.span,
            ty,
            default,
            has_default,
        }
    }

    fn lower_rec(&mut self, node: &Node) -> Option<HRec> {
        let luu_generic = std::mem::take(&mut self.generic_scope);
        self.generic_scope = self.generics_of(node);
        let name = self.decl_name(node)?;
        let sym = self.item_sym(&name);
        let derives = self.derives_of(node);
        let fields: Vec<HField> = node
            .children()
            .filter(|n| n.kind == NodeKind::TypeField)
            .map(|f| self.field_of(f))
            .collect();
        let field_tys: Vec<Type> = fields.iter().map(|f| f.ty.clone()).collect();
        self.kiem_derives(node, &derives, &field_tys);
        // ST-35: dựng **template** (kiểu còn ký hiệu `T`) để instantiation `Hop[int]` thay được.
        if !self.generic_scope.is_empty() {
            self.dung_template_rec(node, &name);
        }
        let id = self
            .rec_names
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, id)| *id)?;
        if let Some(def) = self.tbl.rec_mut(id) {
            def.fields = fields
                .iter()
                .map(|f| FieldDef {
                    name: f.name.clone(),
                    ty: f.ty.clone(),
                    span: f.span,
                    has_default: f.has_default,
                })
                .collect();
        }
        self.generic_scope = luu_generic;
        Some(HRec {
            id,
            name,
            span: node.span,
            sym,
            generics: self.generics_of(node),
            fields,
            derives,
        })
    }

    fn lower_enum(&mut self, node: &Node) -> Option<HEnum> {
        let luu_generic = std::mem::take(&mut self.generic_scope);
        self.generic_scope = self.generics_of(node);
        let name = self.decl_name(node)?;
        let sym = self.item_sym(&name);
        let derives = self.derives_of(node);
        let mut variants: Vec<HVariant> = Vec::new();
        for v in node.children().filter(|n| n.kind == NodeKind::Variant) {
            let vname = self.decl_name(v)?;
            let fields: Vec<HField> = v
                .children()
                .filter(|n| n.kind == NodeKind::VariantField)
                .map(|f| self.field_of(f))
                .collect();
            variants.push(HVariant {
                name: vname,
                span: v.span,
                fields,
            });
        }
        let mut vt: Vec<Type> = Vec::new();
        for v in &variants {
            for f in &v.fields {
                vt.push(f.ty.clone());
            }
        }
        self.kiem_derives(node, &derives, &vt);
        // ST-35: template biến thể (kiểu còn ký hiệu `T`) cho instantiation `Or[int, str]`.
        if !self.generic_scope.is_empty() {
            self.dung_template_enum(node, &name);
        }
        let id = self
            .enum_names
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, id)| *id)?;
        if let Some(def) = self.tbl.enum_mut(id) {
            def.variants = variants
                .iter()
                .map(|v| VariantDef {
                    name: v.name.clone(),
                    fields: v
                        .fields
                        .iter()
                        .map(|f| FieldDef {
                            name: f.name.clone(),
                            ty: f.ty.clone(),
                            span: f.span,
                            has_default: f.has_default,
                        })
                        .collect(),
                    span: v.span,
                })
                .collect();
        }
        self.generic_scope = luu_generic;
        Some(HEnum {
            id,
            name,
            span: node.span,
            sym,
            generics: self.generics_of(node),
            variants,
            derives,
        })
    }

    fn derives_of(&self, node: &Node) -> Vec<String> {
        node.child(NodeKind::Derives)
            .map(|d| {
                d.children()
                    .filter(|n| n.kind == NodeKind::DottedPath)
                    .map(|p| self.dotted_text(p))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn lower_alias(&mut self, node: &Node) -> Option<HItem> {
        let is_newtype = node.kind == NodeKind::NewtypeDecl;
        let name = self.decl_name(node)?;
        let sym = self.item_sym(&name);
        let ty = match node.child(NodeKind::Type) {
            Some(t) => self.resolve_type(t),
            None => Type::Error,
        };
        // `alias` = tên khác của cùng kiểu; `newtype` = kiểu mới (ST-12 phân giải tiếp).
        if !is_newtype {
            self.aliases.push((name.clone(), ty.clone()));
        }
        Some(HItem::Alias(HAlias {
            name,
            span: node.span,
            sym,
            ty,
            is_newtype,
        }))
    }

    fn lower_const(&mut self, node: &Node) -> Option<HItem> {
        let name = self.decl_name(node)?;
        let sym = self.item_sym(&name);
        let ty = match node.child(NodeKind::Type) {
            Some(t) => self.resolve_type(t),
            None => Type::Error,
        };
        let value = node
            .children()
            .filter(|c| c.kind != NodeKind::Type)
            .last()
            .map(|c| self.lower_expr(c))
            .unwrap_or(HExpr::Error { span: node.span });
        Some(HItem::Const(HConst {
            name,
            span: node.span,
            sym,
            ty,
            value,
        }))
    }

    fn lower_trait(&mut self, node: &Node) -> Option<HItem> {
        let name = self.decl_name(node)?;
        let sym = self.item_sym(&name);
        let supertraits = node
            .children()
            .filter(|n| n.kind == NodeKind::DottedPath)
            .map(|p| self.dotted_text(p))
            .collect();
        let mut methods = Vec::new();
        for m in node.children().filter(|n| n.kind == NodeKind::TraitItem) {
            methods.push(self.lower_fn(m, FnKind::TraitMethod));
        }
        Some(HItem::Trait(HTrait {
            name,
            span: node.span,
            sym,
            supertraits,
            methods,
        }))
    }

    fn lower_impl(&mut self, node: &Node) -> Option<HItem> {
        let trait_name = node
            .child(NodeKind::DottedPath)
            .map(|p| self.dotted_text(p));
        let ty = node
            .child(NodeKind::Type)
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Error);
        let mut methods = Vec::new();
        let truct = self.ten_kieu(&ty);
        self.cur_impl = truct.clone();
        for m in node.children().filter(|n| n.kind == NodeKind::FnDecl) {
            methods.push(self.lower_fn(m, FnKind::ImplMethod));
        }
        self.cur_impl = None;
        self.kiem_impl_khop(node, &trait_name, &ty, &methods);
        Some(HItem::Impl(HImpl {
            trait_name,
            ty,
            span: node.span,
            methods,
        }))
    }

    // ----- kiểu -----

    /// Phân giải một nút `Type` (VD-44 #2: **không** báo `VS0301` cho kiểu).
    /// Kiểu của **một đối số generic**: giữ **ký hiệu** `T` khi `T` là tham số kiểu đang có hiệu lực
    /// (ST-35) — nhờ vậy `Hop[T]` trong chữ ký hàm hợp nhất được với `Hop[int]` ở call-site.
    fn resolve_arg_type(&mut self, node: &Node) -> Type {
        // Token đầu (dù nằm trong `elems` hay trong node con) — đủ để nhận ra `T`.
        let dau = node
            .tokens(&self.cst.tokens)
            .next()
            .filter(|t| t.kind == Kind::Ident)
            .map(|t| t.text.clone())
            .or_else(|| {
                let txt = self
                    .text
                    .get(node.span.start..node.span.end)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                txt.chars()
                    .next()
                    .filter(|c| c.is_alphabetic() || *c == '_')
                    .filter(|_| txt.chars().all(|c| c.is_alphanumeric() || c == '_'))
                    .map(|_| txt)
            });
        if let Some(t) = dau {
            if self.generic_scope.iter().any(|g| g == &t) {
                return Type::TypeRef {
                    name: t,
                    args: Vec::new(),
                };
            }
        }
        self.resolve_type(node)
    }

    fn resolve_type(&mut self, node: &Node) -> Type {
        if let Some(Elem::Token(i)) = node.elems.last() {
            if self.tok(*i).map(|t| t.is_punct("?")).unwrap_or(false) {
                let inner = self.resolve_type_body(node);
                if inner.is_opt() {
                    // `T??` là lỗi kiểu (§4.4) — ST-12 báo; ST-11 giữ `{error}`.
                    return Type::Error;
                }
                return Type::Opt(Box::new(inner));
            }
        }
        self.resolve_type_body(node)
    }

    fn resolve_type_body(&mut self, node: &Node) -> Type {
        let kids: Vec<&Node> = node.children().collect();
        let Some(first) = kids.first().copied() else {
            return Type::Error;
        };
        if first.kind == NodeKind::DottedPath {
            let name = self.dotted_text(first);
            let args: Vec<Type> = kids
                .get(1)
                .filter(|n| n.kind == NodeKind::Generics)
                .map(|g| {
                    g.children()
                        .filter(|n| n.kind == NodeKind::Type)
                        .map(|t| self.resolve_arg_type(t))
                        .collect()
                })
                .unwrap_or_default();
            return self.named_type(&name, args);
        }
        let head = node
            .tokens(&self.cst.tokens)
            .next()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        match head.as_str() {
            "ref" | "weak" => {
                let inner = kids
                    .iter()
                    .find(|n| n.kind == NodeKind::Type)
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Type::Error);
                Type::TypeRef {
                    name: head,
                    args: vec![inner],
                }
            }
            "cap" => {
                let inner = kids
                    .iter()
                    .find(|n| n.kind == NodeKind::DottedPath)
                    .map(|p| Type::TypeRef {
                        name: self.dotted_text(p),
                        args: Vec::new(),
                    })
                    .unwrap_or(Type::Error);
                Type::TypeRef {
                    name: "cap".into(),
                    args: vec![inner],
                }
            }
            "fn" => {
                let mut types: Vec<Type> = kids
                    .iter()
                    .filter(|n| n.kind == NodeKind::Type)
                    .map(|t| self.resolve_type(t))
                    .collect();
                let ret = types.pop().unwrap_or(Type::Unit);
                Type::Fn(types.into_boxed_slice(), Box::new(ret))
            }
            "(" => {
                let mut types: Vec<Type> = kids
                    .iter()
                    .filter(|n| n.kind == NodeKind::Type)
                    .map(|t| self.resolve_type(t))
                    .collect();
                match types.len() {
                    0 => Type::Unit,
                    1 => types.remove(0),
                    _ => Type::TypeRef {
                        name: "tuple".into(),
                        args: types,
                    },
                }
            }
            "{" => {
                let mut fields = Vec::new();
                for f in kids.iter().filter(|n| n.kind == NodeKind::TypeField) {
                    let hf = self.field_of(f);
                    fields.push(FieldDef {
                        name: hf.name,
                        ty: hf.ty,
                        span: hf.span,
                        has_default: hf.has_default,
                    });
                }
                let id = RecId(self.tbl.recs().len() as u32);
                self.tbl.push_rec(RecDef {
                    name: None,
                    fields,
                    span: node.span,
                    anonymous: true,
                });
                Type::Record(id)
            }
            _ => Type::Error,
        }
    }

    fn named_type(&mut self, name: &str, args: Vec<Type>) -> Type {
        // ST-28: tham số kiểu của `type`/`enum` đang khai ⇒ **xoá kiểu** (`{error}`) —
        // chưa có instantiation thật (nợ VS1b), nhờ vậy trường/biến thể generic không bị `VS0301`.
        if args.is_empty() && self.generic_scope.iter().any(|g| g == name) {
            // ST-35: khi dựng **template** (hoặc trong chú thích hàm) giữ **ký hiệu** `T`;
            // ngoài ra vẫn xoá kiểu như ST-28 (thân hàm dựa vào `{error}` để im lặng).
            return if self.sym_generic {
                Type::TypeRef {
                    name: name.to_string(),
                    args: Vec::new(),
                }
            } else {
                Type::Error
            };
        }
        match (name, args.len()) {
            ("list", 1) => {
                let mut it = args.into_iter();
                return Type::List(Box::new(it.next().unwrap_or(Type::Error)));
            }
            ("map", 2) => {
                let mut it = args.into_iter();
                let k = it.next().unwrap_or(Type::Error);
                let v = it.next().unwrap_or(Type::Error);
                return Type::Map(Box::new(k), Box::new(v));
            }
            ("Result" | "result", 2) => {
                let mut it = args.into_iter();
                let a = it.next().unwrap_or(Type::Error);
                let b = it.next().unwrap_or(Type::Error);
                return Type::Result(Box::new(a), Box::new(b));
            }
            (_, 0) => {}
            _ => {
                // ST-28: `Or[str, int]` / `Hop[T]` — enum/record **generic** đã khai ⇒ bỏ đối số
                // kiểu (chưa có instantiation thật, nợ VS1b) và trả thẳng `Enum`/`Record`.
                // ST-35: còn tham số **ký hiệu** (vd `Hop[T]` trong chữ ký hàm) ⇒ giữ `TypeRef`
                // để hợp nhất ở call-site; ngược lại ⇒ **instantiation thật** (id riêng).
                let co_ky_hieu = args.iter().any(|a| {
                    matches!(a, Type::TypeRef { args, .. } if args.is_empty())
                        || matches!(a, Type::Error)
                });
                if co_ky_hieu {
                    return Type::TypeRef {
                        name: name.to_string(),
                        args,
                    };
                }
                let enum_base = self
                    .enum_names
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, id)| *id);
                if let Some(id) = enum_base {
                    return self
                        .inst_enum(name, id, args)
                        .map(Type::Enum)
                        .unwrap_or(Type::Enum(id));
                }
                let rec_base = self
                    .rec_names
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, id)| *id);
                if let Some(id) = rec_base {
                    return self
                        .inst_rec(name, id, args)
                        .map(Type::Record)
                        .unwrap_or(Type::Record(id));
                }
                return Type::TypeRef {
                    name: name.to_string(),
                    args,
                };
            }
        }
        if let Some(t) = Type::builtin(name) {
            return t;
        }
        if let Some((_, id)) = self.rec_names.iter().find(|(n, _)| n == name) {
            return Type::Record(*id);
        }
        if let Some((_, id)) = self.enum_names.iter().find(|(n, _)| n == name) {
            return Type::Enum(*id);
        }
        if let Some((_, t)) = self.aliases.iter().find(|(n, _)| n == name) {
            return t.clone();
        }
        Type::TypeRef {
            name: name.to_string(),
            args: Vec::new(),
        }
    }

    fn dotted_text(&self, node: &Node) -> String {
        node.tokens(&self.cst.tokens)
            .filter(|t| t.kind == Kind::Ident)
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

fn is_item(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::UseDecl
            | NodeKind::FnDecl
            | NodeKind::CmdDecl
            | NodeKind::ExternDecl
            | NodeKind::TypeDecl
            | NodeKind::EnumDecl
            | NodeKind::AliasDecl
            | NodeKind::NewtypeDecl
            | NodeKind::TraitDecl
            | NodeKind::ImplDecl
            | NodeKind::ConstDecl
            | NodeKind::TestDecl
    )
}

/// Loại literal theo kind token (SPEC-lex §1).
pub(crate) fn lit_kind(kind: Kind) -> Option<LitKind> {
    Some(match kind {
        Kind::Int => LitKind::Int,
        Kind::Float => LitKind::Float,
        Kind::Size => LitKind::Size,
        Kind::Duration => LitKind::Duration,
        Kind::Percent => LitKind::Percent,
        Kind::Char => LitKind::Char,
        Kind::Str | Kind::RawStr | Kind::MultiStr | Kind::RawMulti => LitKind::Str,
        Kind::Bytes => LitKind::Bytes,
        Kind::Path => LitKind::Path,
        _ => return None,
    })
}

/// Toán tử nhị phân từ chữ viết token.
pub(crate) fn bin_op(text: &str) -> Option<BinOp> {
    BinOp::parse(text)
}

/// Toán tử một ngôi từ chữ viết token.
pub(crate) fn un_op(text: &str) -> Option<UnOp> {
    match text {
        "-" => Some(UnOp::Neg),
        "not" => Some(UnOp::Not),
        _ => None,
    }
}
