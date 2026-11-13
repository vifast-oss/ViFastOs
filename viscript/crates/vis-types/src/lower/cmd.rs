// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cmd_lower` — hạ **CST chế độ lệnh** (`.vish`, khối `cmd { … }`) xuống **cùng HIR** của script
//! (ST-18, VD-22 pha 2). Nguyên tắc: `cmd` là **hàm** (`FnKind::Cmd`) nên mọi thứ chạy qua máy interp
//! sẵn có; ở đây chỉ thêm **bảng chữ ký** + **khe ống** + 4 mã `06xx`.

use super::*;
use crate::hir::Lit;
use vis_diag::COMMAND_CODES;

fn tham(ten: &str, ty: Type, mac_dinh: Option<HExpr>) -> HParam {
    HParam {
        name: ten.to_string(),
        span: Span::new(0, 0),
        sym: 0,
        ty,
        named: false,
        is_self: false,
        is_var_self: false,
        default: mac_dinh,
    }
}

fn co(ten: &str, ty: Type, mac_dinh: Option<HExpr>) -> HParam {
    HParam {
        named: true,
        ..tham(ten, ty, mac_dinh)
    }
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

/// Bảng chữ ký **builtin** (VS0): 3 lệnh, không variadic (hạn chế đã biết — `echo` tối đa 3 phần).
fn sig_builtin() -> Vec<CmdSig> {
    let danh_sach: Vec<(&str, Vec<HParam>, Type)> = vec![
        (
            "echo",
            vec![
                tham("a", Type::Str, None),
                tham("b", Type::Str, Some(lit(LitKind::Str, "\"\""))),
                tham("c", Type::Str, Some(lit(LitKind::Str, "\"\""))),
                co("upper", Type::Bool, Some(lit(LitKind::Bool, "false"))),
            ],
            Type::Str,
        ),
        (
            "lines",
            vec![tham("s", Type::Str, None)],
            Type::List(Box::new(Type::Str)),
        ),
        (
            "take",
            vec![
                tham("xs", Type::List(Box::new(Type::Str)), None),
                tham("n", Type::Int, None),
            ],
            Type::List(Box::new(Type::Str)),
        ),
    ];
    danh_sach
        .into_iter()
        .map(|(n, p, r)| CmdSig {
            name: n.to_string(),
            sym: None,
            params: p,
            ret: r,
        })
        .collect()
}

/// Tên kiểu ngắn để đưa vào thông điệp `VS0602`/`VS0603`.
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
#[derive(Clone)]
pub(crate) struct CmdSig {
    pub name: String,
    pub sym: Option<u32>,
    pub params: Vec<HParam>,
    pub ret: Type,
}

impl<'a> Lowerer<'a> {
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
                    params: f.params.clone(),
                    ret: f.ret.clone(),
                }),
                _ => None,
            })
            .collect();
        sigs.extend(sig_builtin());
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
        if let Some(g) = node.child(NodeKind::CmdGroup) {
            if let Some(inner) = g.children().next() {
                if truoc.is_some() {
                    // Stage `( expr )` không có khe ống ⇒ giá trị trước không đi đâu được.
                    self.diag(
                        &COMMAND_CODES[2],
                        "pipeline không nối được: stage dạng `( … )` không nhận giá trị từ pipe",
                        node.span,
                    );
                }
                return Some(self.lower_expr(inner));
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
            self.diag(
                &COMMAND_CODES[0],
                format!("không có lệnh `{ten}` (không có trong bảng chữ ký)"),
                node.span,
            );
            return None;
        };
        // Khe ống (§9.5, ST-18 (a)1): tham số vị trí **đầu tiên** khớp kiểu với giá trị vào —
        // **giữ chỗ trước**, để đối số vị trí còn lại không chiếm mất (`take 1` ⇒ `take(xs=pipe, 1)`).
        let mut vi_tri: Vec<Option<HExpr>> = vec![None; sig.params.len()];
        let mut khe: Option<usize> = None;
        if let Some((up, up_ty)) = &truoc {
            match sig
                .params
                .iter()
                .enumerate()
                .find(|(_, p)| !p.named && p.ty == *up_ty)
            {
                Some((i, _)) => {
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
                    NodeKind::CmdGroup => (false, String::new(), n.span, Some((**n).clone())),
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
                let ten_sach = text.trim_start_matches('-').to_string();
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
                let gt = match gt {
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
            let ty = sig.params[i_vi_tri].ty.clone();
            let expr = match gt {
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
