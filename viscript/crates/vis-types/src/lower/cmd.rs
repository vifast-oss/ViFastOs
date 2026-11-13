// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cmd_lower` — hạ **CST chế độ lệnh** (`.vish`, khối `cmd { … }`) xuống **cùng HIR** của script
//! (ST-18, VD-22 pha 2). Nguyên tắc: `cmd` là **hàm** (`FnKind::Cmd`) nên mọi thứ chạy qua máy interp
//! sẵn có; ở đây chỉ thêm **bảng chữ ký** + **khe ống** + 4 mã `06xx`.

use super::*;
use crate::cmdsig::{hop_nhat_cmd, CmdSig};
use crate::hir::Lit;
use vis_diag::{COMMAND_CODES, TYPE_CODES};

/// Tách **từ** của một stage lệnh, tôn trọng nháy đơn/nháy kép (ST-27).
fn tach_tu(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut trong: Option<char> = None;
    let mut co = false;
    for c in s.chars() {
        match trong {
            Some(q) => {
                if c == q {
                    trong = None;
                } else {
                    cur.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    trong = Some(c);
                    co = true;
                }
                c if c.is_whitespace() => {
                    if co || !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                        co = false;
                    }
                }
                _ => cur.push(c),
            },
        }
    }
    if co || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn lit(kind: LitKind, text: &str) -> HExpr {
    let l = Lit {
        kind,
        text: text.to_string(),
    };
    HExpr::Lit {
        ty: l.ty(),
        lit: l,
        span: Span::new(0, 0),
    }
}

/// Tên kiểu ngắn để đưa vào thông điệp `VS0602`/`VS0603`.
impl<'a> Lowerer<'a> {
    /// Vị trí **bắt đầu** vùng đối số của stage (token đầu sau tên lệnh).
    /// ST-38: hạ **thân closure** `x => <biểu thức>` thành `HExpr::Lambda`.
    ///
    /// Span: parser re-parse thân từ **slice** (span tương đối) ⇒ ở đây lex/parse lại văn bản thân
    /// nhưng **đệm trước bằng đúng số byte** của vị trí thân trong tệp (prefix `let …=` cố định) ⇒
    /// mọi `Span` là **tuyệt đối** (đúng `dòng:cột`, kể cả khi tệp có ký tự nhiều byte trước đó).
    fn ha_closure(&mut self, g: &Node, want: &Type) -> Option<HExpr> {
        let (mut pty, rty) = match want {
            Type::Fn(ps, r) if ps.len() == 1 => (ps[0].clone(), (**r).clone()),
            _ => (Type::Error, Type::Error),
        };
        if matches!(&pty, Type::TypeRef { args, .. } if args.is_empty()) {
            pty = Type::Error;
        }
        let mut rty = rty;
        if matches!(&rty, Type::TypeRef { args, .. } if args.is_empty()) {
            rty = Type::Error;
        }
        // Tên tham số: token đầu; có thể viết liền `x=>`.
        let mut toks: Vec<Span> = Vec::new();
        for e in &g.elems {
            if let Elem::Token(i) = e {
                if let Some(t) = self.tok(*i) {
                    toks.push(t.span);
                }
            }
        }
        let (Some(dau), Some(cuoi)) = (toks.first().copied(), toks.last().copied()) else {
            return None;
        };
        let raw = self.text.get(dau.start..dau.end)?.to_string();
        let ten = raw.trim_end_matches('=').to_string();
        // Vị trí thân: sau token `>`.
        let gt = toks.iter().position(|sp| {
            self.text
                .get(sp.start..sp.end)
                .map(|x| x == ">")
                .unwrap_or(false)
        })?;
        let body_start = toks.get(gt + 1).map(|sp| sp.start)?;
        let body_end = cuoi.end;
        if body_end <= body_start {
            return None;
        }
        let body = self.text.get(body_start..body_end)?.to_string();
        // Tên biến tạm **không** trùng người dùng: bộ hạ con dùng bảng ký hiệu **chung**.
        self.tam_n += 1;
        let prefix = format!("let _c{} = ", self.tam_n);
        if body_start < prefix.len() {
            return None;
        }
        let pad = body_start - prefix.len();
        let src = format!("{prefix}{}{body}", " ".repeat(pad));
        let lexed = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        // ST-46: thân **không** phải biểu thức script (vd `ls ./photos`) ⇒ trả `None` để nơi gọi
        // thử đường **lệnh trong ngoặc** (stage) hoặc báo lỗi rõ (đối số).
        if !parsed.diagnostics.is_empty() {
            return None;
        }
        // Khung riêng cho tham số + bảng **chung** (symbols/types) ⇒ tyck thấy đúng `sym`.
        self.push_frame(FrameKind::Fn, g.span);
        let psym = self.declare(&ten, SymKind::Param, dau, Some(pty.clone()));
        let scope = self.scope.clone();
        let mut sub = super::Lowerer::new(
            &parsed.cst,
            &lexed,
            self.opts.clone(),
            self.symbols,
            self.tbl,
        );
        sub.scope = scope;
        sub.run();
        let out = sub.finish();
        self.pop_frame();
        self.diags.extend(out.diagnostics);
        let init = match out.hir.stmts.first() {
            Some(crate::hir::HStmt::Let { init, .. }) => init.clone(),
            Some(crate::hir::HStmt::Expr(e)) => e.clone(),
            _ => return None,
        };
        Some(HExpr::Lambda {
            params: vec![HParam {
                name: ten,
                span: dau,
                sym: psym,
                ty: pty,
                named: false,
                is_self: false,
                is_var_self: false,
                variadic: false,
                default: None,
            }],
            ret: rty,
            body: crate::hir::HBody::Expr(Box::new(init)),
            span: g.span,
        })
    }

    /// ST-37: hạ **nội dung** stage `( … )` — parser chỉ giữ token nên hạ lại văn bản trong ngoặc
    /// ở chế độ script. Chỉ dùng cho **biểu thức thuần** (literal/phép toán); biến trong nhóm chưa hỗ trợ.
    pub(super) fn vach_text(&self, g: &Node) -> Option<String> {
        let first = g.elems.first().and_then(|e| match e {
            Elem::Token(i) => self.tok(*i).map(|t| t.span),
            _ => None,
        })?;
        let last = g.elems.last().and_then(|e| match e {
            Elem::Token(i) => self.tok(*i).map(|t| t.span),
            _ => None,
        })?;
        self.text.get(first.end..last.start).map(|s| s.to_string())
    }

    pub(super) fn lower_vach(&mut self, g: &Node) -> Option<HExpr> {
        let txt = self.vach_text(g)?;
        let bat_dau = match g.elems.first() {
            Some(Elem::Token(i)) => self.tok(*i).map(|t| t.span.end).unwrap_or(0),
            _ => 0,
        };
        let sau_mo = &self.text[bat_dau.min(self.text.len())..];
        let lead = sau_mo
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        self.lower_bieu_thuc_tai(&txt, bat_dau + lead)
    }

    /// Hạ **văn bản biểu thức** ở vị trí tuyệt đối `bat_dau` (ST-46) — dùng chung cho `( … )`
    /// (stage, đối số, lồng nhau).
    pub(super) fn lower_bieu_thuc_tai(&mut self, txt: &str, bat_dau: usize) -> Option<HExpr> {
        let txt = txt.to_string();
        // ST-46: **đệm byte** để span của biểu thức là **tuyệt đối** trong tệp (chẩn đoán lúc chạy
        // báo đúng `dòng:cột`); đệm bằng **dấu cách** trong cùng dòng ⇒ không sinh `Indent`/`Dedent`.
        let dau = "let __vach = ";
        let pad = bat_dau.saturating_sub(dau.len());
        // Bọc `let` để **giữ** biểu thức (câu lệnh trần bị bỏ ở cấp tệp) — không chạy tyck ở đây.
        let src = format!("{dau}{}{}", " ".repeat(pad), txt.trim());
        let lexed = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        // ST-46: thân **không** phải biểu thức script (vd `ls ./photos`) ⇒ trả `None` để nơi gọi
        // thử đường **lệnh trong ngoặc** (stage) hoặc báo lỗi rõ (đối số).
        if !parsed.diagnostics.is_empty() {
            return None;
        }
        // ST-38: hạ **vào bảng chung** (ST-25) — nếu hạ bằng bảng mới, `RecId` của bản ghi ẩn danh
        // trong nhóm không tồn tại ở bảng chính ⇒ tyck báo sai kiểu (`record#0` vs `record#1`).
        let h = crate::lower_into(
            self.symbols,
            self.tbl,
            &parsed.cst,
            &lexed,
            self.opts.clone(),
        );
        match h.hir.stmts.first().cloned()? {
            HStmt::Let { init, .. } => Some(init),
            HStmt::Expr(e) => Some(e),
            _ => None,
        }
    }

    /// ST-37: kiểu **literal** suy từ văn bản (`[1, 2]` ⇒ `list[int]`, `"x"` ⇒ `str`, …).
    /// Chỉ để chọn khe ống / buộc `T`; `{error}` nếu không chắc (tyck/lúc chạy kiểm sau).
    pub(super) fn kieu_text(&self, raw: &str) -> Type {
        let t = raw.trim();
        if let Some(tron) = t.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            let dau = super::chieu::tach_phay(tron)
                .first()
                .copied()
                .unwrap_or("")
                .trim();
            return Type::List(Box::new(if dau.is_empty() {
                Type::Error
            } else {
                self.kieu_text(dau)
            }));
        }
        if t.starts_with('"') {
            return Type::Str;
        }
        if t == "true" || t == "false" {
            return Type::Bool;
        }
        if t.parse::<i64>().is_ok() {
            return Type::Int;
        }
        if t.parse::<f64>().is_ok() {
            return Type::Float;
        }
        // ST-40a: literal có **đơn vị** — `5mb`/`2kb` (size), `3ms`/`1s`/`2m` (duration), `50pct` (percent).
        let so = t.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
        if let Ok(_n) = so.parse::<f64>() {
            if t.ends_with('%') || t.ends_with("pct") {
                return Type::Percent;
            }
            let duoi = &t[so.len()..];
            match duoi {
                "b" | "kb" | "mb" | "gb" | "tb" => return Type::Size,
                "ms" | "s" | "m" | "h" | "d" => return Type::Duration,
                _ => {}
            }
        }
        // ST-40a: literal bản ghi `{…}` khớp **đúng bộ trường** của một bản ghi prelude ⇒ kiểu đó
        // (cho `it` có kiểu thật khi pipeline bắt đầu bằng list literal bản ghi).
        if t.starts_with('{') && t.ends_with('}') {
            let trong = &t[1..t.len() - 1];
            let mut ten_truong: Vec<String> = Vec::new();
            let mut duoc = true;
            for f in super::chieu::tach_phay(trong) {
                match f.split_once(':') {
                    Some((k, _)) if !k.trim().is_empty() => {
                        ten_truong.push(k.trim().trim_matches('"').to_string())
                    }
                    _ => {
                        duoc = false;
                        break;
                    }
                }
            }
            if duoc {
                for (_, id) in &self.prelude_recs {
                    if let Some(rec) = self.tbl.rec(*id) {
                        let mut a: Vec<String> =
                            rec.fields.iter().map(|f| f.name.clone()).collect();
                        let mut b = ten_truong.clone();
                        a.sort();
                        b.sort();
                        if a == b {
                            return Type::Record(*id);
                        }
                    }
                }
            }
        }
        // ST-40a: `ten(...)` gọi lệnh `cmd` đã khai ⇒ kiểu ra của chữ ký (generic chưa suy ⇒ `{error}`).
        if let Some(goi) = t.strip_suffix(')') {
            if let Some((ten, _)) = goi.split_once('(') {
                let ten = ten.trim();
                if !ten.is_empty() {
                    if let Some(sig) = self.cmd_sigs.iter().find(|s| s.name == ten) {
                        return if sig.generics.is_empty() {
                            sig.ret.clone()
                        } else {
                            Type::Error
                        };
                    }
                }
            }
        }
        // ST-40a: tên biến/tham số **đã khai** với kiểu tường minh.
        if let Some(sym) = self.symbols.syms.iter().find(|x| x.name == t) {
            if let Some(ty) = &sym.ty {
                return ty.clone();
            }
        }
        Type::Error
    }
}

pub(super) fn ten_kieu(t: &Type) -> String {
    match t {
        Type::Int => "int".into(),
        Type::Float => "float".into(),
        Type::Size => "size".into(),
        Type::Duration => "duration".into(),
        Type::Percent => "percent".into(),
        Type::Str => "str".into(),
        Type::Bool => "bool".into(),
        Type::Bytes => "bytes".into(),
        Type::Path => "path".into(),
        Type::Char => "char".into(),
        Type::Unit => "unit".into(),
        Type::List(x) => format!("list[{}]", ten_kieu(x)),
        Type::Map(k, v) => format!("map[{}, {}]", ten_kieu(k), ten_kieu(v)),
        Type::Opt(x) => format!("{}?", ten_kieu(x)),
        Type::Result(a, b) => format!("Result[{}, {}]", ten_kieu(a), ten_kieu(b)),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Một chữ ký lệnh dùng lúc hạ: tên, `sym` (cmd trong tệp) hoặc `None` (builtin), tham số, kiểu ra.
impl<'a> Lowerer<'a> {
    /// Dựng call **chương trình ngoài** `exec.<tên>` (ST-27): từ thô của stage, bỏ tên lệnh; cờ
    /// `--raw` của Vis ⇒ trả `str` thay vì `list[str]`.
    fn ngoai_call(&mut self, ten: &str, node: &Node, stdin: Option<HExpr>) -> HExpr {
        let raw = self
            .text
            .get(node.span.start..node.span.end)
            .unwrap_or("")
            .trim()
            .to_string();
        let mut tu = tach_tu(&raw);
        if !tu.is_empty() {
            tu.remove(0);
        }
        let mut co_raw = false;
        if tu.last().map(|s| s.as_str()) == Some("--raw") {
            tu.pop();
            co_raw = true;
        }
        let items: Vec<HExpr> = tu
            .iter()
            .map(|w| lit(LitKind::Str, &format!("{w:?}")))
            .collect();
        let ds = HExpr::List {
            items,
            elem: Type::Str,
            span: node.span,
        };
        let name = if co_raw {
            format!("exec.raw.{ten}")
        } else {
            format!("exec.{ten}")
        };
        // ST-34: đối số thứ 2 = **stdin** (giá trị từ pipe) — `builtin::call` quy đổi (một quy tắc).
        let vao = stdin.unwrap_or_else(|| {
            let mut rong = lit(LitKind::Str, "\"\"");
            if let HExpr::Lit { span, .. } = &mut rong {
                *span = node.span;
            }
            rong
        });
        HExpr::Call {
            callee: Box::new(HExpr::Var {
                name,
                sym: None,
                span: node.span,
            }),
            args: vec![ds, vao],
            named: Vec::new(),
            span: node.span,
        }
    }

    /// Chạy hạ chế độ lệnh: item (`cmd`/`fn`/`extern`) trước, rồi từng dòng lệnh thành `HStmt`.
    /// Chạy các dòng lệnh của CST chế độ lệnh; **bỏ** mọi node có `span.start < tu` (ST-44: dòng
    /// đã hạ trong **vùng khai báo** ở tiền tố tệp).
    pub(crate) fn run_shell_tu(&mut self, tu: usize) {
        self.declare_builtins();
        let kids: Vec<Node> = self.cst.root.children().cloned().collect();
        for child in &kids {
            if is_item(child.kind) && child.span.start >= tu {
                self.predeclare_item(child);
            }
        }
        for child in &kids {
            if is_item(child.kind) && child.span.start >= tu {
                if let Some(it) = self.build_item(child) {
                    self.items.push(it);
                }
            }
        }
        let mut sigs: Vec<CmdSig> = self
            .items
            .iter()
            .filter_map(|it| match it {
                HItem::Fn(f) if f.kind == FnKind::Cmd => Some(CmdSig {
                    name: f.name.clone(),
                    sym: Some(f.sym),
                    generics: f.generics.clone(),
                    params: f.params.clone(),
                    ret: f.ret.clone(),
                }),
                _ => None,
            })
            .collect();
        sigs.extend(self.cmd_sigs.clone());
        for child in &kids {
            if child.span.start < tu {
                continue;
            }
            match child.kind {
                NodeKind::CmdItems => self.ha_dong(child, &sigs),
                NodeKind::CmdBg => {
                    self.diag(
                        &COMMAND_CODES[3],
                        "chạy nền `&` chưa hỗ trợ ở VS0 (chưa có tiến trình)",
                        child.span,
                    );
                }
                k if !is_item(k) => {
                    let mut out = Vec::new();
                    if let Some(e) = self.ha_pipeline(child, &sigs) {
                        // ST-46: dòng **chỉ** gọi `print(…)` ⇒ không bọc `render` (không in thêm `()`).
                        let la_print = matches!(
                            &e,
                            HExpr::Call { callee, .. }
                                if matches!(&**callee, HExpr::Var { name, .. } if name == "print")
                        );
                        let e = if la_print {
                            e
                        } else {
                            self.boc_render(e, child.span)
                        };
                        out.push(HStmt::Expr(e));
                    }
                    self.stmts.extend(out);
                }
                _ => {}
            }
        }
    }

    /// `render(<expr>)` (ST-41) — §9.6: giá trị **cuối pipeline** hiển thị qua **`Render`**; `print`
    /// của người dùng **không** đổi (tên builtin riêng, chỉ dùng cho giá trị cuối).
    fn boc_render(&mut self, e: HExpr, span: Span) -> HExpr {
        let co_ban_ghi =
            matches!(&self.last_stage_ty, Type::List(el) if matches!(el.as_ref(), Type::Record(_)));
        HExpr::Call {
            callee: Box::new(HExpr::Var {
                name: "render".to_string(),
                sym: None,
                span,
            }),
            args: vec![
                e,
                HExpr::Lit {
                    ty: Type::Bool,
                    lit: Lit {
                        kind: LitKind::Bool,
                        text: co_ban_ghi.to_string(),
                    },
                    span,
                },
            ],
            named: Vec::new(),
            span,
        }
    }

    /// Một dòng lệnh (`items`): mỗi pipeline/bg là một câu lệnh, **theo thứ tự**.
    fn ha_dong(&mut self, node: &Node, sigs: &[CmdSig]) {
        for con in node.children() {
            if con.kind == NodeKind::CmdBg {
                // (a)4 — VS0 chưa có tiến trình: báo đúng **VS0604**, không hạ thành no-op.
                self.diag(
                    &COMMAND_CODES[3],
                    "chạy nền `&` chưa hỗ trợ ở VS0 (chưa có tiến trình)",
                    con.span,
                );
                continue;
            }
            if let Some(e) = self.ha_pipeline(con, sigs) {
                let e = self.boc_render(e, con.span);
                self.stmts.push(HStmt::Expr(e));
            }
        }
    }

    /// `pipeline := stage ('|' stage)*` ⇒ **lồng call** theo khe ống (§9.5, ST-18 (a)).
    fn ha_pipeline(&mut self, node: &Node, sigs: &[CmdSig]) -> Option<HExpr> {
        let stage: Vec<&Node> = node
            .children()
            .filter(|n| n.kind == NodeKind::CmdStage)
            .collect();
        if stage.is_empty() {
            return None;
        }
        let mut truoc: Option<(HExpr, Type)> = None;
        let mut ket: Option<(HExpr, Type)> = None;
        self.last_stage_ty = Type::Error;
        for st in &stage {
            let e = self.ha_stage(st, truoc.take(), sigs)?;
            let ty = self.kieu_stage(st, sigs);
            truoc = Some((e, ty));
            ket = truoc.clone();
        }
        // ST-41: hook `where`/`sort_by`/`get`/`select` đã đặt kiểu **chính xác** (giữ nguyên); nếu chưa
        // (stage thường/group) ⇒ dùng `kieu_stage` (literal bản ghi ⇒ `list[record#N]`).
        if matches!(self.last_stage_ty, Type::Error) {
            if let Some(last) = stage.last() {
                self.last_stage_ty = self.kieu_stage(last, sigs);
            }
        }
        ket.map(|(e, _)| e)
    }

    fn kieu_stage(&self, node: &Node, sigs: &[CmdSig]) -> Type {
        // ST-37: stage `( expr )` — suy kiểu **literal** (list/str/int/bool) thay vì `{error}`.
        // ST-46: **chỉ** khi phần tử **đầu** của stage là nhóm thì mới là stage-biểu-thức;
        // `take ( … )`/`f()` có nhóm là **đối số** nên không rơi vào nhánh này.
        let head_group = match node.elems.first() {
            Some(Elem::Node(g)) if g.kind == NodeKind::CmdGroup => Some(&**g),
            _ => None,
        };
        if let Some(g) = head_group {
            if let Some(txt) = self.vach_text(g) {
                if !txt.trim().is_empty() {
                    return self.kieu_text(&txt);
                }
            }
        }
        let Some(ten) = self.ten_lenh(node) else {
            return Type::Error;
        };
        let mut r = sigs
            .iter()
            .find(|s| s.name == ten)
            .map(|s| s.ret.clone())
            .unwrap_or(Type::Error);
        // ST-46: `ls --long` ⇒ **bản ghi riêng** `FileEntryLong` (6 trường) cho kiểm tĩnh hạ nguồn.
        let stage_txt = self.text_of(node.span);
        if ten == "ls" && stage_txt.contains("--long") {
            if let Some(id) = self.file_long {
                r = Type::List(Box::new(Type::Record(id)));
            }
        }
        r
    }

    /// Tên lệnh = **phần tử đầu** của stage (token `Word` hoặc node `CmdWord`).
    fn ten_lenh(&self, node: &Node) -> Option<String> {
        match node.elems.first()? {
            Elem::Token(i) => {
                let t = self.tok(*i)?;
                if t.kind == Kind::Word || t.kind == Kind::Ident {
                    Some(t.text.clone())
                } else {
                    None
                }
            }
            Elem::Node(n) => n.tokens(&self.cst.tokens).next().map(|t| t.text.clone()),
        }
    }

    /// Một stage: `(group expr)` hoặc `(cmd|ext tên)` + đối số, có thể nhận giá trị từ pipe.
    fn ha_stage(
        &mut self,
        node: &Node,
        truoc: Option<(HExpr, Type)>,
        sigs: &[CmdSig],
    ) -> Option<HExpr> {
        // ST-44: nhóm `()` **rỗng đứng một mình** (không phải `f()`) ⇒ lỗi rõ, không coi là lệnh `(`.
        if let Some(Elem::Node(g)) = node.elems.first() {
            if g.kind == NodeKind::CmdGroup
                && node.elems.len() == 1
                && self
                    .vach_text(g)
                    .map(|t| t.trim().is_empty())
                    .unwrap_or(false)
            {
                self.diag(
                    &vis_diag::SYNTAX_CODES[0],
                    "`( … )` rỗng không phải biểu thức (chỉ `f()` mới hợp lệ làm lời gọi)",
                    g.span,
                );
                return None;
            }
        }
        // ST-34: `| complete` — stage **giả**, chỉ dùng sau chương trình ngoài; nó **đổi tên** callee
        // của stage trước thành `exec.complete.<ten>` (không thêm biến thể HIR ⇒ codegen không đổi).
        if self.ten_lenh(node).as_deref() == Some("complete") {
            if node.child(NodeKind::CmdGroup).is_some() || node.elems.len() > 1 {
                // `complete` không nhận đối số.
                self.diag(&COMMAND_CODES[1], "`complete` không nhận đối số", node.span);
                return None;
            }
            let Some((up, _)) = truoc else {
                self.diag(
                    &COMMAND_CODES[1],
                    "complete chỉ dùng sau chương trình ngoài",
                    node.span,
                );
                return None;
            };
            if let HExpr::Call {
                callee,
                args,
                named,
                span,
            } = up
            {
                if let HExpr::Var { name, sym, .. } = *callee {
                    if let Some(con) = name.strip_prefix("exec.") {
                        return Some(HExpr::Call {
                            callee: Box::new(HExpr::Var {
                                name: format!("exec.complete.{con}"),
                                sym,
                                span,
                            }),
                            args,
                            named,
                            span,
                        });
                    }
                }
            }
            self.diag(
                &COMMAND_CODES[1],
                "complete chỉ dùng sau chương trình ngoài",
                node.span,
            );
            return None;
        }
        // ST-46: chỉ khi **phần tử đầu** là nhóm ⇒ stage-biểu-thức (`( ls … )`); nhóm làm **đối số**
        // (`take ( … )`) do vòng đối số xử lý.
        let head_group = match node.elems.first() {
            Some(Elem::Node(g)) if g.kind == NodeKind::CmdGroup => Some(&**g),
            _ => None,
        };
        if let Some(g) = head_group {
            // ST-37: parser **không** gắn node biểu thức vào `CmdGroup` (chỉ giữ token) ⇒ hạ lại
            // **văn bản trong ngoặc** ở chế độ script (biểu thức thuần: literal/phép toán).
            // ST-46: không phải biểu thức ⇒ thử **một lệnh** trong ngoặc (`( ls ./photos )`).
            let v = self.lower_vach(g).or_else(|| {
                if truoc.is_none() {
                    self.lower_nhom_lenh(g, sigs)
                } else {
                    None
                }
            });
            if v.is_none() {
                if let Some(e) = truoc.as_ref() {
                    let _ = e;
                    self.diag(
                        &vis_diag::SYNTAX_CODES[0],
                        "trong `( … )`: không phải biểu thức hay lệnh chạy được",
                        g.span,
                    );
                }
            }
            if let Some(e) = v {
                if truoc.is_some() {
                    // Stage `( expr )` không có khe ống ⇒ giá trị trước không đi đâu được.
                    self.diag(
                        &COMMAND_CODES[2],
                        "pipeline không nối được: stage dạng `( … )` không nhận giá trị từ pipe",
                        node.span,
                    );
                }
                return Some(e);
            }
        }
        if let Some(ten) = self.ten_lenh(node) {
            if crate::lower::is_builtin_value(&ten) && !sigs.iter().any(|s| s.name == ten) {
                let nhom: Vec<&Node> = node
                    .children()
                    .filter(|c| c.kind == NodeKind::CmdGroup)
                    .collect();
                let cac_phan_tu = node.elems.len();
                if !nhom.is_empty() && cac_phan_tu == nhom.len() + 1 {
                    let mut args = Vec::new();
                    for g in nhom {
                        args.push(self.lower_vach(g)?);
                    }
                    return Some(HExpr::Call {
                        callee: Box::new(HExpr::Var {
                            name: ten,
                            sym: None,
                            span: node.span,
                        }),
                        args,
                        named: Vec::new(),
                        span: node.span,
                    });
                }
            }
        }
        let Some(ten) = self.ten_lenh(node) else {
            // Chương trình ngoài (`^git`) — VS0 chưa có tiến trình ⇒ VS0604, không hạ no-op.
            self.diag(
                &COMMAND_CODES[3],
                "chương trình ngoài (`^…`) chưa hỗ trợ ở VS0",
                node.span,
            );
            return None;
        };
        if std::env::var("VIS_DBG44").is_ok() {
            eprintln!(
                "stage ten={ten:?} sigs={:?} tu_child={:?}",
                sigs.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                node.span
            );
        }
        let Some(sig) = sigs.iter().find(|s| s.name == ten).cloned() else {
            // ST-27: lệnh **lạ** ⇒ chương trình ngoài (§9.3) — trừ khi host tắt (`--no-extern`).
            if !self.opts.cho_ngoai {
                self.diag(
                    &COMMAND_CODES[0],
                    format!(
                        "không có lệnh `{ten}` trong bảng chữ ký và chương trình ngoài đang tắt (`--no-extern`)"
                    ),
                    node.span,
                );
                return None;
            }
            return Some(self.ngoai_call(&ten, node, truoc.map(|(e, _)| e)));
        };
        // ST-40b: `get name` / `select pid, name` — chiếu trường của `it` (VD-71/73).
        if sig.name == "get" || sig.name == "select" {
            if let Some((up, up_ty)) = &truoc {
                let la_closure = node.children().any(|c| c.kind == NodeKind::CmdClosure);
                if !la_closure {
                    let mut bang = Vec::new();
                    let _ = hop_nhat_cmd(&sig.params[0].ty, up_ty, &sig.generics, &mut bang);
                    let elem = super::generic::thay(
                        &Type::TypeRef {
                            name: "T".into(),
                            args: Vec::new(),
                        },
                        &bang,
                    );
                    if let Some(dau) = self.arg_start(node) {
                        let het = self.stage_end(dau);
                        if let Some(lam) = self.ha_chieu(&sig.name, dau, het, &elem, node.span) {
                            let kieu_ra = match &lam {
                                HExpr::Lambda { ret, .. } => ret.clone(),
                                _ => Type::Error,
                            };
                            self.last_stage_ty = Type::List(Box::new(kieu_ra));
                            return Some(HExpr::Call {
                                callee: Box::new(HExpr::Var {
                                    name: sig.name.clone(),
                                    sym: sig.sym,
                                    span: node.span,
                                }),
                                args: vec![up.clone(), lam],
                                named: Vec::new(),
                                span: node.span,
                            });
                        }
                    }
                }
            }
        }
        // ST-39: khoá ngầm cho `sort_by` (§9.7): `sort_by modified --desc` ⇒ `it => it.modified`, `desc=true`.
        if sig.name == "sort_by" {
            if let Some((up, up_ty)) = &truoc {
                let la_closure = node.children().any(|c| c.kind == NodeKind::CmdClosure);
                if !la_closure {
                    let mut bang = Vec::new();
                    let _ = hop_nhat_cmd(&sig.params[0].ty, up_ty, &sig.generics, &mut bang);
                    let fty = super::generic::thay(&sig.params[1].ty, &bang);
                    let (mut pty, mut rty) = match &fty {
                        Type::Fn(ps, r) if ps.len() == 1 => (ps[0].clone(), (**r).clone()),
                        _ => (Type::Error, Type::Error),
                    };
                    if matches!(&pty, Type::TypeRef { args, .. } if args.is_empty()) {
                        pty = Type::Error;
                    }
                    if matches!(&rty, Type::TypeRef { args, .. } if args.is_empty()) {
                        rty = Type::Error;
                    }
                    if let Some(dau) = self.arg_start(node) {
                        let het_stage = self.stage_end(dau);
                        let vung = self.text.get(dau..het_stage).unwrap_or("").to_string();
                        let body_end = dau + vung.find("--").unwrap_or(vung.len());
                        let co_text = &vung[vung.find("--").unwrap_or(vung.len())..];
                        if let Some(lam) = self.ha_sugar(dau, Some(body_end), pty.clone(), rty) {
                            self.last_stage_ty = Type::List(Box::new(pty.clone()));
                            let mut args = vec![up.clone(), lam];
                            let mut loi = false;
                            for raw in co_text.split_whitespace() {
                                if !raw.starts_with("--") {
                                    continue;
                                }
                                let (ten, gt) = raw
                                    .trim_start_matches('-')
                                    .split_once('=')
                                    .map(|(a, b)| (a.to_string(), Some(b.to_string())))
                                    .unwrap_or((raw.trim_start_matches('-').to_string(), None));
                                if ten != "desc" {
                                    self.diag(
                                        &COMMAND_CODES[1],
                                        format!("`sort_by` không có cờ `--{ten}`"),
                                        node.span,
                                    );
                                    loi = true;
                                    continue;
                                }
                                let b = gt.map(|v| v == "true").unwrap_or(true);
                                args.push(HExpr::Lit {
                                    ty: Type::Bool,
                                    lit: Lit {
                                        kind: LitKind::Bool,
                                        text: b.to_string(),
                                    },
                                    span: node.span,
                                });
                            }
                            if !loi {
                                return Some(HExpr::Call {
                                    callee: Box::new(HExpr::Var {
                                        name: sig.name.clone(),
                                        sym: sig.sym,
                                        span: node.span,
                                    }),
                                    args,
                                    named: Vec::new(),
                                    span: node.span,
                                });
                            }
                            return None;
                        }
                    }
                }
            }
        }
        // ST-38: **vị từ ngầm `it`** cho `where` (§9.7): `where size > 5mb` ⇒ `it => it.size > 5mb`.
        // Chỉ áp khi có pipe vào và đối số **không** phải closure; luật đổi tên ở [`Self::ha_sugar`].
        if sig.name == "where" {
            if let Some((up, up_ty)) = &truoc {
                let la_closure = node
                    .children()
                    .filter(|c| c.kind != NodeKind::CmdWord)
                    .any(|c| c.kind == NodeKind::CmdClosure);
                if !la_closure {
                    let mut bang = Vec::new();
                    let _ = hop_nhat_cmd(&sig.params[0].ty, up_ty, &sig.generics, &mut bang);
                    let fty = super::generic::thay(&sig.params[1].ty, &bang);
                    let (mut pty, mut rty) = match &fty {
                        Type::Fn(ps, r) if ps.len() == 1 => (ps[0].clone(), (**r).clone()),
                        _ => (Type::Error, Type::Error),
                    };
                    // Chưa suy được `T` (stage trước kiểu `{error}`) ⇒ dùng `{error}` **im lặng**:
                    // để `TypeRef{"T"}` lọt xuống tyck sẽ thành `VS0301` (không có kiểu `T`).
                    if matches!(&pty, Type::TypeRef { args, .. } if args.is_empty()) {
                        pty = Type::Error;
                    }
                    if matches!(&rty, Type::TypeRef { args, .. } if args.is_empty()) {
                        rty = Type::Error;
                    }
                    // ST-46: `where ( x => x )` — nhóm chứa **lambda** ⇒ hạ thẳng thành biểu thức
                    // (không đưa qua sugar `it`, tránh đổi tên `x` thành `it.x`).
                    if let Some(g) = node.children().find(|c| c.kind == NodeKind::CmdGroup) {
                        let txt = self.vach_text(g).unwrap_or_default();
                        if txt.contains("=>") {
                            if let Some(lam) = self.lower_vach(g) {
                                self.last_stage_ty = Type::List(Box::new(pty));
                                return Some(HExpr::Call {
                                    callee: Box::new(HExpr::Var {
                                        name: sig.name.clone(),
                                        sym: sig.sym,
                                        span: node.span,
                                    }),
                                    args: vec![up.clone(), lam],
                                    named: Vec::new(),
                                    span: node.span,
                                });
                            }
                        }
                    }
                    // Vùng văn bản của đối số: từ token đầu **sau tên lệnh** tới hết stage.
                    if let Some(dau) = self.arg_start(node) {
                        let pty_hint = pty.clone();
                        let lam = self.ha_sugar(dau, None, pty, rty);
                        if let Some(lam) = lam {
                            self.last_stage_ty = Type::List(Box::new(pty_hint));
                            return Some(HExpr::Call {
                                callee: Box::new(HExpr::Var {
                                    name: sig.name.clone(),
                                    sym: sig.sym,
                                    span: node.span,
                                }),
                                args: vec![up.clone(), lam],
                                named: Vec::new(),
                                span: node.span,
                            });
                        }
                    }
                }
            }
        }
        // Khe ống (§9.5, ST-18 (a)1): tham số vị trí **đầu tiên** khớp kiểu với giá trị vào —
        // **giữ chỗ trước**, để đối số vị trí còn lại không chiếm mất (`take 1` ⇒ `take(xs=pipe, 1)`).
        let mut vi_tri: Vec<Option<HExpr>> = vec![None; sig.params.len()];
        let mut khe: Option<usize> = None;
        let mut bang: Vec<(String, Type)> = Vec::new();
        if let Some((up, up_ty)) = &truoc {
            // ST-37 (§9.5): khe chọn theo **khả năng nhận** — hợp nhất một chiều để buộc `T`.
            // - `up_ty = {error}` (stage nhóm không suy được) ⇒ chọn **vị trí đầu** (an toàn).
            // - KHÔNG đặt khe vào tham số `--cờ`; không đè bind tường minh (khe giữ chỗ trước).
            let mut chon: Option<usize> = None;
            if matches!(up_ty, Type::Error) {
                chon = sig.params.iter().position(|p| !p.named);
            } else {
                for (i, p) in sig.params.iter().enumerate() {
                    if p.named {
                        continue;
                    }
                    let mut b = bang.clone();
                    if hop_nhat_cmd(&p.ty, up_ty, &sig.generics, &mut b) {
                        chon = Some(i);
                        bang = b;
                        break;
                    }
                }
            }
            match chon {
                Some(i) => {
                    vi_tri[i] = Some(up.clone());
                    khe = Some(i);
                }
                None => {
                    self.diag(
                        &COMMAND_CODES[2],
                        format!(
                            "pipeline không nối được: `{ten}` không có khe nhận `{}`",
                            ten_kieu(up_ty)
                        ),
                        node.span,
                    );
                    return None;
                }
            }
        }
        // Đối số tường minh — duyệt **elems** của stage (token lẫn node, theo đúng thứ tự nguồn).
        let mut theo_ten: Vec<(String, HExpr)> = Vec::new();
        let mut i_vi_tri = 0usize;
        let mut bo_qua = 1usize; // phần tử đầu là tên lệnh
        for e in &node.elems {
            if bo_qua > 0 {
                bo_qua -= 1;
                continue;
            }
            let (la_co, text, con_span, gt) = match e {
                Elem::Token(i) => {
                    let Some(t) = self.tok(*i) else { continue };
                    if t.kind == Kind::Comment {
                        continue;
                    }
                    let la = t.text.starts_with('-') && t.text != "-" && t.text != "--";
                    (la, t.text.clone(), t.span, None)
                }
                Elem::Node(n) => match n.kind {
                    NodeKind::CmdFlag => {
                        let ten_co = n
                            .tokens(&self.cst.tokens)
                            .next()
                            .map(|t| t.text.clone())
                            .unwrap_or_default();
                        (true, ten_co, n.span, n.children().next().cloned())
                    }
                    NodeKind::CmdGroup | NodeKind::CmdClosure => {
                        (false, String::new(), n.span, Some((**n).clone()))
                    }
                    NodeKind::CmdVar | NodeKind::CmdEnv => {
                        self.diag(
                            &COMMAND_CODES[3],
                            "biến `$x`/`$env.X` chưa hỗ trợ ở VS0",
                            n.span,
                        );
                        return None;
                    }
                    _ => (
                        false,
                        n.tokens(&self.cst.tokens)
                            .map(|t| t.text.clone())
                            .collect::<String>(),
                        n.span,
                        None,
                    ),
                },
            };
            // `--x` có thể đến dưới dạng **token Word** hoặc node `CmdWord` ⇒ nhận theo văn bản.
            // Ngoại lệ VD-40 #2: `-5` là **số** nếu tham số kế tiếp kiểu số.
            let so_am = text
                .strip_prefix('-')
                .and_then(|r| r.parse::<f64>().ok())
                .is_some();
            let kieu_ke = {
                let mut k = i_vi_tri;
                while k < sig.params.len() && (sig.params[k].named || Some(k) == khe) {
                    k += 1;
                }
                sig.params.get(k).map(|p| p.ty.clone())
            };
            let la_co = (la_co
                || (text.starts_with('-') && text != "-" && text != "--" && gt.is_none()))
                && !(so_am
                    && matches!(
                        kieu_ke,
                        Some(Type::Int)
                            | Some(Type::Float)
                            | Some(Type::Size)
                            | Some(Type::Percent)
                            | Some(Type::Duration)
                    ));
            if la_co {
                // ST-27: `--x=v` — tên cờ và giá trị tách bằng `=`, **không** lấy khoảng trắng.
                let (ten_sach, gt_bang) = match text.trim_start_matches('-').split_once('=') {
                    Some((n, v)) => (n.to_string(), Some(v.to_string())),
                    None => (text.trim_start_matches('-').to_string(), None),
                };
                if text == "--" {
                    continue; // `--` kết thúc cờ (VD-40 #4)
                }
                let Some(p) = sig
                    .params
                    .iter()
                    .find(|p| p.named && p.name == ten_sach)
                    .cloned()
                else {
                    self.diag(
                        &COMMAND_CODES[1],
                        format!("`{ten}` không có cờ `{text}`"),
                        con_span,
                    );
                    return None;
                };
                let gt = if let Some(v) = gt_bang {
                    // `--x=v` ⇒ giá trị `v` (đã bỏ khoảng trắng quanh `=`), đổi theo kiểu cờ.
                    match self.chuyen_doi_text(&v, &p.ty, con_span) {
                        Some(e) => e,
                        None => {
                            self.diag(
                                &COMMAND_CODES[1],
                                format!(
                                    "cờ `--{ten_sach}={v}` sai kiểu (cần `{}`)",
                                    ten_kieu(&p.ty)
                                ),
                                con_span,
                            );
                            return None;
                        }
                    }
                } else {
                    match gt {
                        Some(v) if v.kind == NodeKind::CmdGroup => match v.children().next() {
                            Some(x) => self.lower_expr(x),
                            None => continue,
                        },
                        Some(v) => match self.chuyen_doi(&v, &p.ty) {
                            Some(x) => x,
                            None => {
                                self.diag(
                                    &COMMAND_CODES[1],
                                    format!("cờ `{text}` sai kiểu (cần `{}`)", ten_kieu(&p.ty)),
                                    con_span,
                                );
                                return None;
                            }
                        },
                        None if p.ty == Type::Bool => HExpr::Lit {
                            lit: Lit {
                                kind: LitKind::Bool,
                                text: "true".into(),
                            },
                            ty: Type::Bool,
                            span: con_span,
                        },
                        None => {
                            self.diag(
                                &COMMAND_CODES[1],
                                format!("cờ `{text}` cần giá trị nhưng chưa hỗ trợ ở VS0"),
                                con_span,
                            );
                            return None;
                        }
                    }
                };
                theo_ten.push((ten_sach, gt));
                continue;
            }
            // ST-44: `f()` — nhóm **rỗng** là dấu **lời gọi không đối số**, không phải đối số.
            // Nhóm **không rỗng** làm đối số vẫn là **nợ VSH1b** ⇒ giữ `VS0603` như trước.
            if let Some(g) = gt.as_ref() {
                if g.kind == NodeKind::CmdGroup
                    && self
                        .vach_text(g)
                        .map(|t| t.trim().is_empty())
                        .unwrap_or(false)
                {
                    continue; // `f()` — lời gọi **không đối số**
                }
            }
            // Đối số vị trí (bỏ qua ô **khe ống** đã giữ chỗ).
            while i_vi_tri < sig.params.len()
                && (sig.params[i_vi_tri].named || Some(i_vi_tri) == khe)
            {
                i_vi_tri += 1;
            }
            if i_vi_tri >= sig.params.len() {
                self.diag(&COMMAND_CODES[1], format!("`{ten}` thừa đối số"), con_span);
                return None;
            }
            // ST-37: thay tham số kiểu đã suy (từ khe hoặc từ đối số trước) trước khi kiểm đối số.
            let ty = super::generic::thay(&sig.params[i_vi_tri].ty, &bang);
            // ST-45: **`each` không kiểm kiểu trả** (giống `for` trong Rust) — khai kiểu trả lambda là
            // `{error}` để tyck **không** `coerce` về `()`; thân vẫn được kiểm (field/kiểu/cú pháp).
            let ty = if sig.name == "each" {
                match ty {
                    Type::Fn(ps, _) => Type::Fn(ps, Box::new(Type::Error)),
                    other => other,
                }
            } else {
                ty
            };
            // ST-37: buộc `T` từ **kiểu suy được của đối số** (literal) trước khi chuyển.
            let suy = match gt.as_ref() {
                Some(g) if g.kind == NodeKind::CmdGroup => self
                    .vach_text(g)
                    .map(|t| self.kieu_text(&t))
                    .unwrap_or(Type::Error),
                _ => self.kieu_text(&text),
            };
            let _ = hop_nhat_cmd(&ty, &suy, &sig.generics, &mut bang);
            let expr = match gt {
                Some(g) if g.kind == NodeKind::CmdClosure => {
                    let lam = self.ha_closure(&g, &ty);
                    // ST-39: `.vish` **không** qua tyck ⇒ kiểm khoá ở đây khi biết kiểu: thân chỉ là
                    // chính tham số (`x => x`) ⇒ khoá = kiểu tham số; `bool`/`map`/`record`/`list`
                    // không so được ⇒ `VS0412`.
                    if sig.name == "sort_by" {
                        if let (Some(HExpr::Lambda { params, body, .. }), Type::Fn(_, _)) =
                            (lam.as_ref(), &ty)
                        {
                            let khoa = match (body, params.first()) {
                                (crate::hir::HBody::Expr(e), Some(p)) => match e.as_ref() {
                                    HExpr::Var { name, .. } if *name == p.name => {
                                        Some(p.ty.clone())
                                    }
                                    _ => None,
                                },
                                _ => None,
                            };
                            if let Some(k) = khoa {
                                if matches!(
                                    &k,
                                    Type::Bool | Type::Map(..) | Type::Record(..) | Type::List(..)
                                ) {
                                    self.diag(
                                        &TYPE_CODES[3],
                                        format!("`sort_by`: khoá không so được: `{}`", k.name()),
                                        g.span,
                                    );
                                }
                            }
                        }
                    }
                    lam
                }
                Some(g) if g.kind == NodeKind::CmdGroup => {
                    // ST-46: `( … )` ở **vị trí đối số** = **biểu thức** (bỏ `VS0603`); kiểm kiểu
                    // **tĩnh** khi `kieu_text` suy được (`1 + 1`/lambda ⇒ `{error}` ⇒ kiểm động).
                    let txt = self.vach_text(&g).unwrap_or_default();
                    let suy = self.kieu_text(&txt);
                    let khop = matches!(suy, Type::Error) || matches!(ty, Type::Error) || suy == ty;
                    if !khop {
                        self.diag(
                            &TYPE_CODES[3],
                            format!(
                                "`( … )` làm đối số: cần `{}`, được `{}`",
                                ten_kieu(&ty),
                                ten_kieu(&suy)
                            ),
                            g.span,
                        );
                        return None;
                    }
                    self.lower_vach(&g)
                }
                Some(g) => {
                    let inner = g.children().next().cloned();
                    match inner {
                        Some(inner) => {
                            let raw = self.text_of(inner.span).to_string();
                            self.chuyen_doi_text(&raw, &ty, inner.span)
                        }
                        None => None,
                    }
                }
                None => {
                    let raw = if text.is_empty() {
                        self.text_of(con_span).to_string()
                    } else {
                        text.clone()
                    };
                    self.chuyen_doi_text(&raw, &ty, con_span)
                }
            };
            let Some(expr) = expr else {
                self.diag(
                    &COMMAND_CODES[1],
                    format!(
                        "`{ten}`: đối số `{}` sai kiểu (cần `{}`)",
                        text,
                        ten_kieu(&ty)
                    ),
                    con_span,
                );
                return None;
            };
            vi_tri[i_vi_tri] = Some(expr);
            i_vi_tri += 1;
        }
        // Builtin: interp nhận **đối số vị trí** (không có binding theo tên) ⇒ đặt cờ vào đúng ô,
        // điền default cho các ô trống phía trước.
        if sig.sym.is_none() {
            for (ten_co, gt) in &theo_ten {
                if let Some(i) = sig.params.iter().position(|p| p.named && &p.name == ten_co) {
                    for (j, o) in vi_tri.iter_mut().enumerate().take(i) {
                        if o.is_none() {
                            *o = sig.params[j].default.clone();
                        }
                    }
                    vi_tri[i] = Some(gt.clone());
                }
            }
            theo_ten.clear();
            // ST-42: điền default cho **mọi** ô còn trống (kể cả ô **sau** cờ, vd `long` sau `--all`),
            // nếu không `day_du` sai ⇒ cờ bị **bỏ** khỏi call.
            for (j, o) in vi_tri.iter_mut().enumerate() {
                if o.is_none() {
                    *o = sig.params[j].default.clone();
                }
            }
            let day_du = vi_tri.iter().all(|x| x.is_some());
            if day_du {
                let args: Vec<HExpr> = vi_tri.into_iter().flatten().collect();
                return Some(HExpr::Call {
                    callee: Box::new(HExpr::Var {
                        name: sig.name.clone(),
                        sym: None,
                        span: node.span,
                    }),
                    args,
                    named: Vec::new(),
                    span: node.span,
                });
            }
        }
        // Thiếu tham số **bắt buộc** (không default) ⇒ VS0602.
        let mut args: Vec<HExpr> = Vec::new();
        for (i, p) in sig.params.iter().enumerate() {
            if p.named {
                continue;
            }
            match &vi_tri[i] {
                Some(e) => args.push(e.clone()),
                None if p.default.is_none() => {
                    self.diag(
                        &COMMAND_CODES[1],
                        format!("`{ten}` thiếu đối số `{}`", p.name),
                        node.span,
                    );
                    return None;
                }
                None => {
                    // Default ở giữa: truyền theo tên để interp tự điền default.
                    break;
                }
            }
        }
        let callee = match sig.sym {
            Some(sym) => HExpr::Var {
                name: sig.name.clone(),
                sym: Some(sym),
                span: node.span,
            },
            None => HExpr::Var {
                name: sig.name.clone(),
                sym: None,
                span: node.span,
            },
        };
        Some(HExpr::Call {
            callee: Box::new(callee),
            args,
            named: theo_ten,
            span: node.span,
        })
    }

    /// Chuyển một **từ trần**/chuỗi thành literal **đúng kiểu tham số** (pha 2 của VD-22).
    /// ST-46: `( <lệnh> … )` — nhóm làm **stage** nhưng nội dung là **một lệnh** (không `|`/`;`).
    /// Parse nội dung bằng `Mode::Command` **không đệm** rồi cộng bù `at` cho span (chẩn đoán tuyệt đối).
    pub(super) fn lower_nhom_lenh(&mut self, g: &Node, sigs: &[CmdSig]) -> Option<HExpr> {
        let text = self.vach_text(g)?;
        let bat_dau = match g.elems.first() {
            Some(Elem::Token(i)) => self.tok(*i).map(|t| t.span.end).unwrap_or(0),
            _ => 0,
        };
        let sau = &self.text[bat_dau.min(self.text.len())..];
        let lead = sau.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let at = bat_dau + lead;
        let inner = text.trim();
        let parsed = vis_syntax::parse(inner, vis_syntax::lex::Mode::Command);
        if !parsed.diagnostics.is_empty() {
            return None;
        }
        // Tệp **một dòng** ⇒ root KHÔNG có `CmdItems` (VD-40 #1): pipeline nằm thẳng ở root.
        let dong = parsed
            .cst
            .root
            .children()
            .find(|n| n.kind == NodeKind::CmdItems)
            .cloned()
            .or_else(|| {
                parsed
                    .cst
                    .root
                    .children()
                    .find(|n| n.kind == NodeKind::CmdPipe)
                    .cloned()
            })?;
        let stage = dong.children().find(|n| n.kind == NodeKind::CmdStage)?;
        let doan = |sp: Span| inner.get(sp.start..sp.end).unwrap_or("").to_string();
        let ten = match stage.elems.first()? {
            Elem::Token(i) => parsed.cst.tokens.get(*i as usize)?.text.clone(),
            Elem::Node(n) => n.tokens(&parsed.cst.tokens).next()?.text.clone(),
        };
        let sig = sigs.iter().find(|s| s.name == ten)?.clone();
        if !sig.generics.is_empty() {
            return None;
        }
        let mut args: Vec<Option<HExpr>> = sig.params.iter().map(|p| p.default.clone()).collect();
        let mut vi_tri = 0usize;
        for e in &stage.elems[1..] {
            if let Elem::Node(n) = e {
                if n.kind == NodeKind::CmdGroup {
                    let raw = doan(n.span);
                    let inner_nhom = raw.strip_prefix('(')?.strip_suffix(')')?;
                    while vi_tri < sig.params.len() && sig.params[vi_tri].named {
                        vi_tri += 1;
                    }
                    if vi_tri >= sig.params.len() {
                        return None;
                    }
                    let rel = n.span.start + 1 + (raw.len() - raw.trim_start().len());
                    let e = self.lower_bieu_thuc_tai(inner_nhom, at + rel)?;
                    args[vi_tri] = Some(e);
                    vi_tri += 1;
                    continue;
                }
            }
            let (la_co, raw, sp) = match e {
                Elem::Token(i) => {
                    let t = parsed.cst.tokens.get(*i as usize)?;
                    (
                        t.text.starts_with('-') && t.text != "-",
                        t.text.clone(),
                        t.span,
                    )
                }
                Elem::Node(n) if n.kind == NodeKind::CmdFlag => {
                    let t = n.tokens(&parsed.cst.tokens).next()?;
                    (true, t.text.clone(), n.span)
                }
                Elem::Node(n) => (false, doan(n.span), n.span),
            };
            let sp = Span::new(sp.start + at, sp.end + at);
            if la_co {
                let ten_co = raw.trim_start_matches('-');
                if let Some(i) = sig.params.iter().position(|p| p.named && p.name == ten_co) {
                    args[i] = Some(HExpr::Lit {
                        ty: Type::Bool,
                        lit: Lit {
                            kind: LitKind::Bool,
                            text: "true".into(),
                        },
                        span: sp,
                    });
                }
                continue;
            }
            while vi_tri < sig.params.len() && sig.params[vi_tri].named {
                vi_tri += 1;
            }
            if vi_tri >= sig.params.len() {
                return None;
            }
            let ty = sig.params[vi_tri].ty.clone();
            let e = self.chuyen_doi_text(&raw, &ty, sp)?;
            args[vi_tri] = Some(e);
            vi_tri += 1;
        }
        let mut ds = Vec::new();
        for a in args {
            ds.push(a?);
        }
        Some(HExpr::Call {
            callee: Box::new(HExpr::Var {
                name: ten,
                sym: None,
                span: g.span,
            }),
            args: ds,
            named: Vec::new(),
            span: g.span,
        })
    }

    pub(super) fn chuyen_doi(&mut self, node: &Node, ty: &Type) -> Option<HExpr> {
        let raw = self.text_of(node.span).to_string();
        self.chuyen_doi_text(&raw, ty, node.span)
    }

    /// Như [`Self::chuyen_doi`] nhưng nhận **văn bản** (dùng cho token rời của stage).
    pub(super) fn chuyen_doi_text(&mut self, raw: &str, ty: &Type, span: Span) -> Option<HExpr> {
        let text = raw.trim();
        let (kind, ty_lit) = match ty {
            Type::Str => {
                let s = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
                    text.to_string()
                } else {
                    format!("\"{text}\"")
                };
                return Some(HExpr::Lit {
                    lit: Lit {
                        kind: LitKind::Str,
                        text: s,
                    },
                    ty: Type::Str,
                    span,
                });
            }
            Type::Int => (LitKind::Int, Type::Int),
            Type::Float => (LitKind::Float, Type::Float),
            Type::Size => (LitKind::Size, Type::Size),
            Type::Bool => (LitKind::Bool, Type::Bool),
            _ => return None,
        };
        let hople = match kind {
            LitKind::Int => text.parse::<i64>().is_ok(),
            LitKind::Float => text.parse::<f64>().is_ok(),
            LitKind::Bool => text == "true" || text == "false",
            LitKind::Size => vis_syntax::lex::lit::size_parts(text).is_some(),
            _ => false,
        };
        if !hople {
            return None;
        }
        Some(HExpr::Lit {
            lit: Lit {
                kind,
                text: text.to_string(),
            },
            ty: ty_lit,
            span,
        })
    }
}
