// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cmd_lower` — hạ **CST chế độ lệnh** (`.vish`, khối `cmd { … }`) xuống **cùng HIR** của script
//! (ST-18, VD-22 pha 2). Nguyên tắc: `cmd` là **hàm** (`FnKind::Cmd`) nên mọi thứ chạy qua máy interp
//! sẵn có; ở đây chỉ thêm **bảng chữ ký** + **khe ống** + 4 mã `06xx`.

use super::*;
use crate::cmdsig::{hop_nhat_cmd, CmdSig};
use crate::hir::Lit;
use vis_diag::COMMAND_CODES;

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
    fn arg_start(&self, node: &Node) -> Option<usize> {
        // Parser có thể **không** sinh token cho mọi từ (vd `size > 5mb` thành redirect) ⇒ quét
        // thẳng văn bản: hết **tên lệnh**, bỏ khoảng trắng ⇒ đó là đầu vùng đối số.
        let ten = node.tokens(&self.cst.tokens).next()?;
        let mut i = ten.span.end;
        let bytes = self.text.as_bytes();
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        Some(i)
    }

    /// `true` nếu `name` đã **khai** trong phạm vi hiện tại (biến/hàm/const/dựng sẵn).
    fn da_khai(&self, name: &str) -> bool {
        self.symbols
            .syms
            .iter()
            .any(|s| s.name == name && s.kind != SymKind::Generic)
    }

    /// ST-38: **vị từ ngầm `it`** — `where size > 5mb` ⇒ `it => it.size > 5mb`.
    ///
    /// Luật (Agent 0 chốt): chỉ đổi **định danh trần chưa khai báo**, không đổi `it` viết tường minh,
    /// không đổi tên đã khai; không đổi luật nào khác. `where 1` vẫn là **lỗi kiểm tĩnh** (`VS0602`
    /// "đối số sai kiểu" vì `1` không phải hàm) — ghi JOURNAL.
    fn ha_sugar(&mut self, start: usize, pty: Type, rty: Type) -> Option<HExpr> {
        // Kết thúc **stage**: gặp `|`/`;`/`&` ở độ sâu 0, hoặc xuống dòng.
        let mut depth = 0i32;
        let mut end = self.text.len();
        let bytes = self.text.as_bytes();
        let mut i = start;
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b'|' | b';' | b'&' if depth <= 0 => {
                    end = i;
                    break;
                }
                b'\n' if depth <= 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        let text = self.text.get(start..end)?.trim().to_string();
        if text.is_empty() {
            return None;
        }
        let lexed = vis_syntax::lex(&text, vis_syntax::Mode::Script);
        let mut chen: Vec<usize> = Vec::new();
        let toks: Vec<(usize, String)> = lexed
            .tokens
            .iter()
            .map(|t| (t.span.start, t.text.clone()))
            .collect();
        for (i, (off, txt)) in toks.iter().enumerate() {
            if txt != "it" && self.da_khai(txt) {
                continue;
            }
            if txt == "it"
                || txt.is_empty()
                || !txt.starts_with(|c: char| c.is_alphabetic() || c == '_')
            {
                continue;
            }
            if matches!(txt.as_str(), "and" | "or" | "not" | "true" | "false") {
                continue;
            }
            if toks
                .get(i.wrapping_sub(1))
                .map(|(_, p)| p == ".")
                .unwrap_or(false)
            {
                continue;
            }
            if txt == "it" || self.da_khai(txt) {
                continue;
            }
            // `it` tường minh: `it.x` — không đổi; `f(` là lời gọi hàm — không đổi.
            if toks.get(i + 1).map(|(_, p)| p == "(").unwrap_or(false) {
                continue;
            }
            // `time.now()`, `it.is_file()` — tên **module/trường** (theo sau là `.`) ⇒ không đổi.
            if toks.get(i + 1).map(|(_, p)| p == ".").unwrap_or(false) {
                continue;
            }
            chen.push(*off);
        }
        let mut src_body = String::new();
        let mut last = 0usize;
        for off in &chen {
            src_body.push_str(&text[last..*off]);
            src_body.push_str("it.");
            last = *off;
        }
        src_body.push_str(&text[last..]);
        self.tam_n += 1;
        let prefix = format!("let _c{} = it => ", self.tam_n);
        if start < prefix.len() {
            return None;
        }
        let pad = start - prefix.len();
        let src = format!("{prefix}{}{src_body}", " ".repeat(pad));
        if std::env::var("VIS_DBG_SUGAR").is_ok() {
            eprintln!("sugar src={src:?}");
        }
        let lexed2 = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        self.push_frame(FrameKind::Fn, Span::new(start, end));
        let scope = self.scope.clone();
        let mut sub = super::Lowerer::new(
            &parsed.cst,
            &lexed2,
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
            _ => return None,
        };
        match init {
            HExpr::Lambda {
                params, body, span, ..
            } => {
                let mut params = params;
                if let Some(p) = params.first_mut() {
                    p.ty = pty;
                }
                Some(HExpr::Lambda {
                    params,
                    ret: rty,
                    body,
                    span,
                })
            }
            _ => None,
        }
    }

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
    fn vach_text(&self, g: &Node) -> Option<String> {
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

    fn lower_vach(&mut self, g: &Node) -> Option<HExpr> {
        let txt = self.vach_text(g)?;
        // Bọc `let` để **giữ** biểu thức (câu lệnh trần bị bỏ ở cấp tệp) — không chạy tyck ở đây.
        let src = format!("let __vach = {}", txt.trim());
        let lexed = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
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
    fn kieu_text(&self, raw: &str) -> Type {
        let t = raw.trim();
        if let Some(tron) = t.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            let dau = tron.split(',').next().unwrap_or("").trim();
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
        Type::Error
    }
}

fn ten_kieu(t: &Type) -> String {
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
    pub(crate) fn run_shell(&mut self) {
        self.declare_builtins();
        let kids: Vec<Node> = self.cst.root.children().cloned().collect();
        for child in &kids {
            if is_item(child.kind) {
                self.predeclare_item(child);
            }
        }
        for child in &kids {
            if is_item(child.kind) {
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
                        let e = self.boc_print(e, child.span);
                        out.push(HStmt::Expr(e));
                    }
                    self.stmts.extend(out);
                }
                _ => {}
            }
        }
    }

    /// `print(<expr>)` — §9.6: giá trị **cuối pipeline** được hiển thị qua `Render`/`Show`.
    fn boc_print(&mut self, e: HExpr, span: Span) -> HExpr {
        HExpr::Call {
            callee: Box::new(HExpr::Var {
                name: "print".to_string(),
                sym: None,
                span,
            }),
            args: vec![e],
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
                let e = self.boc_print(e, con.span);
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
        for st in &stage {
            let e = self.ha_stage(st, truoc.take(), sigs)?;
            let ty = self.kieu_stage(st, sigs);
            truoc = Some((e, ty));
            ket = truoc.clone();
        }
        ket.map(|(e, _)| e)
    }

    fn kieu_stage(&self, node: &Node, sigs: &[CmdSig]) -> Type {
        // ST-37: stage `( expr )` — suy kiểu **literal** (list/str/int/bool) thay vì `{error}`.
        let head_group = node
            .children()
            .next()
            .filter(|c| c.kind == NodeKind::CmdGroup);
        if let Some(g) = head_group {
            if let Some(txt) = self.vach_text(g) {
                return self.kieu_text(&txt);
            }
        }
        let Some(ten) = self.ten_lenh(node) else {
            return Type::Error;
        };
        sigs.iter()
            .find(|s| s.name == ten)
            .map(|s| s.ret.clone())
            .unwrap_or(Type::Error)
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
        if std::env::var("VIS_DBG_KHE").is_ok() {
            eprintln!(
                "ha_stage kind={:?} kids={:?}",
                node.kind,
                node.children().map(|c| c.kind).collect::<Vec<_>>()
            );
        }
        let head_group = node
            .children()
            .next()
            .filter(|c| c.kind == NodeKind::CmdGroup);
        if let Some(g) = head_group {
            // ST-37: parser **không** gắn node biểu thức vào `CmdGroup` (chỉ giữ token) ⇒ hạ lại
            // **văn bản trong ngoặc** ở chế độ script (biểu thức thuần: literal/phép toán).
            let v = self.lower_vach(g);
            if std::env::var("VIS_DBG_KHE").is_ok() {
                eprintln!("group lowered: {:?}", v.is_some());
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
        let Some(ten) = self.ten_lenh(node) else {
            // Chương trình ngoài (`^git`) — VS0 chưa có tiến trình ⇒ VS0604, không hạ no-op.
            self.diag(
                &COMMAND_CODES[3],
                "chương trình ngoài (`^…`) chưa hỗ trợ ở VS0",
                node.span,
            );
            return None;
        };
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
        // ST-38: **vị từ ngầm `it`** cho `where` (§9.7): `where size > 5mb` ⇒ `it => it.size > 5mb`.
        // Chỉ áp khi có pipe vào và đối số **không** phải closure; luật đổi tên ở [`Self::ha_sugar`].
        if sig.name == "where" {
            if let Some((up, up_ty)) = &truoc {
                let la_closure = node
                    .children()
                    .filter(|c| c.kind != NodeKind::CmdWord)
                    .any(|c| c.kind == NodeKind::CmdClosure);
                if std::env::var("VIS_DBG_SUGAR").is_ok() {
                    eprintln!(
                        "hook where la_closure={la_closure} kids={:?}",
                        node.children().map(|c| c.kind).collect::<Vec<_>>()
                    );
                }
                if !la_closure {
                    let mut bang = Vec::new();
                    let _ = hop_nhat_cmd(&sig.params[0].ty, up_ty, &sig.generics, &mut bang);
                    let fty = super::generic::thay(&sig.params[1].ty, &bang);
                    let (mut pty, rty) = match &fty {
                        Type::Fn(ps, r) if ps.len() == 1 => (ps[0].clone(), (**r).clone()),
                        _ => (Type::Error, Type::Error),
                    };
                    // Chưa suy được `T` (stage trước kiểu `{error}`) ⇒ dùng `{error}` **im lặng**:
                    // để `TypeRef{"T"}` lọt xuống tyck sẽ thành `VS0301` (không có kiểu `T`).
                    if matches!(&pty, Type::TypeRef { args, .. } if args.is_empty()) {
                        pty = Type::Error;
                    }
                    // Vùng văn bản của đối số: từ token đầu **sau tên lệnh** tới hết stage.
                    if let Some(dau) = self.arg_start(node) {
                        let lam = self.ha_sugar(dau, pty, rty);
                        if std::env::var("VIS_DBG_SUGAR").is_ok() {
                            eprintln!("sugar at {dau} -> {:?}", lam.is_some());
                        }
                        if let Some(lam) = lam {
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
                Some(g) if g.kind == NodeKind::CmdClosure => self.ha_closure(&g, &ty),
                Some(g) if g.kind == NodeKind::CmdGroup => self.lower_vach(&g),
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
    fn chuyen_doi(&mut self, node: &Node, ty: &Type) -> Option<HExpr> {
        let raw = self.text_of(node.span).to_string();
        self.chuyen_doi_text(&raw, ty, node.span)
    }

    /// Như [`Self::chuyen_doi`] nhưng nhận **văn bản** (dùng cho token rời của stage).
    fn chuyen_doi_text(&mut self, raw: &str, ty: &Type, span: Span) -> Option<HExpr> {
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
