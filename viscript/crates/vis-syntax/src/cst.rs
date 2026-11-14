// SPDX-License-Identifier: MIT OR Apache-2.0
//! CST **lossless** của ViScript — ST-05 (tài liệu thiết kế Phụ lục A).
//!
//! Cây gồm **node** và **token lá**; mọi token do lexer sinh (kể cả `NEWLINE`/`INDENT`/`DEDENT`
//! và chú thích) đều nằm trong cây theo đúng thứ tự nguồn. Khoảng trắng **không** phải token
//! (SPEC-lex §1 không có kind cho nó): nó là **khoảng trống giữa hai lá liên tiếp**, nên
//! [`Cst::render`] phát lại nguyên văn bằng `text[gap]` + `text[token.span]`. Nhờ vậy CST dùng
//! được cho `vis-fmt` (ST-07): sửa khoảng trắng = sửa gap/độ rộng thụt lề, không mất byte nào.
//!
//! # Bất biến
//!
//! Với mọi nguồn parse **thành công**: `cst.render(&out.text) == out.text` (byte-for-byte, gồm
//! cả chú thích, dòng trống, tab và độ rộng thụt lề gốc).
//!
//! # Vì sao khoảng trắng là gap
//!
//! Lexer **không** sinh token cho khoảng trắng (SPEC-lex §1); nếu CST bịa thêm token giả thì
//! `text[span] == token.text` của ST-04 sẽ sai. Gap là cách duy nhất giữ lossless mà không nới
//! hợp đồng lexer. Chú thích **có** token (`COMMENT`/`DOC_COMMENT`) nên được gắn tường minh vào
//! node sâu nhất chứa nó ([`Cst::attach_comments`]) để tool đọc được mà không phải quét text.

use crate::lex::token::{Span, Token};

/// Loại node của CST (đủ cho biểu thức của Phụ lục A.3 và câu lệnh khai báo A.1/A.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    // Khung tệp / item (A.1)
    Program,
    UseDecl,
    FnDecl,
    CmdDecl,
    ExternDecl,
    TypeDecl,
    EnumDecl,
    AliasDecl,
    NewtypeDecl,
    TraitDecl,
    TraitItem,
    ImplDecl,
    ConstDecl,
    TestDecl,
    TypeField,
    Variant,
    VariantField,
    Param,
    Params,
    Generic,
    Generics,
    Derives,
    GrantList,
    DottedPath,
    // Khối và câu lệnh (A.2)
    Suite,
    LetStmt,
    VarStmt,
    AssignStmt,
    ReturnStmt,
    BreakStmt,
    ContinueStmt,
    ForStmt,
    WhileStmt,
    LoopStmt,
    ScopeStmt,
    IfStmt,
    ElifArm,
    ElseArm,
    MatchStmt,
    MatchArm,
    ExprStmt,
    // Chế độ lệnh (A.7) — CmdCST chưa định kiểu (ST-06)
    CmdShell,
    CmdItems,
    CmdBg,
    CmdPipe,
    CmdStage,
    CmdGroup,
    CmdWord,
    CmdFlag,
    CmdStr,
    CmdVar,
    CmdEnv,
    CmdClosure,
    CmdLet,
    /// ST-44: **khối khai báo** trong `.vish` (raw — hạ bằng đường script ở tiền tố tệp).
    CmdKhai,
    // Kiểu (A.5) và pattern (A.4)
    Type,
    Pattern,
    // Biểu thức (A.3)
    BinOp,
    UnOp,
    Paren,
    Tuple,
    ListLit,
    SetLit,
    MapLit,
    MapEntry,
    RecordLit,
    RecordField,
    Field,
    OptField,
    Call,
    Args,
    NamedArg,
    Index,
    GenericCall,
    TryOp,
    WithOp,
    WithField,
    Ident,
    Literal,
    Lambda,
    LambdaParams,
    IfExpr,
    MatchExpr,
    RefExpr,
    AwaitExpr,
    /// **ST-60**: `spawn f(args)` — tạo tác vụ + đưa vào hàng đợi FIFO (`spawn` là **soft keyword**:
    /// chỉ khi token kế là định danh, nên `s.spawn(1)` vẫn là **phương thức**).
    SpawnExpr,
    CmdBlock,
}

impl NodeKind {
    /// Tên ngắn (log/test).
    pub const fn name(self) -> &'static str {
        match self {
            NodeKind::Program => "Program",
            NodeKind::UseDecl => "use",
            NodeKind::FnDecl => "fn",
            NodeKind::CmdDecl => "cmd",
            NodeKind::ExternDecl => "extern",
            NodeKind::TypeDecl => "type",
            NodeKind::EnumDecl => "enum",
            NodeKind::AliasDecl => "alias",
            NodeKind::NewtypeDecl => "newtype",
            NodeKind::TraitDecl => "trait",
            NodeKind::TraitItem => "trait_item",
            NodeKind::ImplDecl => "impl",
            NodeKind::ConstDecl => "const",
            NodeKind::TestDecl => "test_fn",
            NodeKind::TypeField => "type_field",
            NodeKind::Variant => "variant",
            NodeKind::VariantField => "variant_field",
            NodeKind::Param => "param",
            NodeKind::Params => "params",
            NodeKind::Generic => "generic",
            NodeKind::Generics => "generics",
            NodeKind::Derives => "derives",
            NodeKind::GrantList => "grants",
            NodeKind::DottedPath => "path",
            NodeKind::Suite => "suite",
            NodeKind::LetStmt => "let",
            NodeKind::VarStmt => "var",
            NodeKind::AssignStmt => "assign",
            NodeKind::ReturnStmt => "return",
            NodeKind::BreakStmt => "break",
            NodeKind::ContinueStmt => "continue",
            NodeKind::ForStmt => "for",
            NodeKind::WhileStmt => "while",
            NodeKind::LoopStmt => "loop",
            NodeKind::ScopeStmt => "scope",
            NodeKind::IfStmt => "if",
            NodeKind::ElifArm => "elif",
            NodeKind::ElseArm => "else",
            NodeKind::MatchStmt => "match",
            NodeKind::MatchArm => "arm",
            NodeKind::ExprStmt => "expr_stmt",
            NodeKind::Type => "type",
            NodeKind::Pattern => "pat",
            NodeKind::BinOp => "binop",
            NodeKind::UnOp => "unop",
            NodeKind::Paren => "paren",
            NodeKind::Tuple => "tuple",
            NodeKind::ListLit => "list",
            NodeKind::SetLit => "set",
            NodeKind::MapLit => "map",
            NodeKind::MapEntry => "entry",
            NodeKind::RecordLit => "record",
            NodeKind::RecordField => "record_field",
            NodeKind::Field => "field",
            NodeKind::OptField => "?field",
            NodeKind::Call => "call",
            NodeKind::Args => "args",
            NodeKind::NamedArg => "named_arg",
            NodeKind::Index => "index",
            NodeKind::GenericCall => "generic_call",
            NodeKind::TryOp => "try",
            NodeKind::WithOp => "with",
            NodeKind::WithField => "with_field",
            NodeKind::Ident => "ident",
            NodeKind::Literal => "literal",
            NodeKind::Lambda => "lambda",
            NodeKind::LambdaParams => "lambda_params",
            NodeKind::IfExpr => "if_expr",
            NodeKind::MatchExpr => "match_expr",
            NodeKind::RefExpr => "ref",
            NodeKind::AwaitExpr => "await",
            NodeKind::SpawnExpr => "spawn",
            NodeKind::CmdBlock => "cmd_block",
            NodeKind::CmdShell => "shell",
            NodeKind::CmdItems => "items",
            NodeKind::CmdBg => "bg",
            NodeKind::CmdPipe => "pipe",
            NodeKind::CmdStage => "stage",
            NodeKind::CmdGroup => "group",
            NodeKind::CmdWord => "word",
            NodeKind::CmdFlag => "flag",
            NodeKind::CmdStr => "str",
            NodeKind::CmdVar => "var",
            NodeKind::CmdEnv => "env",
            NodeKind::CmdClosure => "closure",
            NodeKind::CmdLet => "let_cmd",
            NodeKind::CmdKhai => "cmd_khai",
        }
    }
}

/// Một phần tử con của node: token lá (chỉ số trong `LexOutput.tokens`) hoặc node con.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elem {
    Token(u32),
    Node(Box<Node>),
}

/// Node CST: loại + span byte (trong văn bản đã NFC) + danh sách con **theo đúng thứ tự nguồn**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
    pub elems: Vec<Elem>,
}

impl Node {
    pub fn new(kind: NodeKind, span: Span, elems: Vec<Elem>) -> Self {
        Self { kind, span, elems }
    }

    /// Node con đầu tiên có `kind` cho trước (tiện cho test/harness đi cây).
    pub fn child(&self, kind: NodeKind) -> Option<&Node> {
        self.elems.iter().find_map(|e| match e {
            Elem::Node(n) if n.kind == kind => Some(&**n),
            _ => None,
        })
    }

    /// Duyệt các node con (bỏ qua lá).
    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.elems.iter().filter_map(|e| match e {
            Elem::Node(n) => Some(&**n),
            Elem::Token(_) => None,
        })
    }

    /// Các cặp (chỉ số token, văn bản) của riêng node này **không** tính node con.
    pub fn tokens<'a>(&'a self, toks: &'a [Token]) -> impl Iterator<Item = &'a Token> {
        self.elems.iter().filter_map(move |e| match e {
            Elem::Token(i) => toks.get(*i as usize),
            Elem::Node(_) => None,
        })
    }

    /// Tìm node sâu nhất chứa `span` (dùng để gắn chú thích).
    fn deepest_containing(&mut self, span: Span) -> &mut Node {
        let child_idx = self.elems.iter().position(|e| match e {
            Elem::Node(n) => n.span.start <= span.start && span.end <= n.span.end,
            Elem::Token(_) => false,
        });
        match child_idx {
            Some(i) => match &mut self.elems[i] {
                Elem::Node(n) => n.deepest_containing(span),
                Elem::Token(_) => unreachable!(),
            },
            None => self,
        }
    }

    /// Chèn một lá token vào node sao cho thứ tự span vẫn tăng (chú thích nằm giữa hai con).
    fn insert_leaf(&mut self, idx: u32, span: Span, toks: &[Token]) {
        let pos = self
            .elems
            .iter()
            .position(|e| match e {
                Elem::Token(i) => toks[*i as usize].span.start > span.start,
                Elem::Node(n) => n.span.start > span.start,
            })
            .unwrap_or(self.elems.len());
        self.elems.insert(pos, Elem::Token(idx));
    }
}

/// Cây CST lossless + bảng token gốc (để render/đối chiếu span).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cst {
    pub root: Node,
    pub tokens: Vec<Token>,
    /// Sexp của **CmdCST** (`Mode::Command`) — `None` với chế độ script. ST-06 pha B dựng trong
    /// lúc parse (kèm `root` là cây node) để `vis check`/golden đối chiếu; quy ước ở đầu
    /// `viscript/test/cmd/golden.cmd`.
    pub cmd_sexp: Option<String>,
}

impl Cst {
    pub fn new(root: Node, tokens: Vec<Token>) -> Self {
        Self {
            root,
            tokens,
            cmd_sexp: None,
        }
    }

    /// Gắn sexp CmdCST do parser dựng (ST-06).
    pub fn set_cmd_sexp(&mut self, sexp: String) {
        self.cmd_sexp = Some(sexp);
    }

    /// Gắn **chú thích** (`COMMENT`/`DOC_COMMENT`) vào node sâu nhất chứa nó.
    ///
    /// Khoảng trắng không cần bước này (đã là gap); chú thích có token nên phải có mặt tường
    /// minh trong cây để tool đọc được. Gọi đúng một lần sau khi parse xong.
    pub fn attach_comments(&mut self) {
        let comments: Vec<u32> = self
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                matches!(
                    t.kind,
                    crate::lex::token::Kind::Comment | crate::lex::token::Kind::DocComment
                )
            })
            .map(|(i, _)| i as u32)
            .collect();
        // Bỏ qua chú thích đã có trong cây (parser đã bỏ chú thích khỏi luồng cú pháp, nhưng
        // hàm này phải idempotent để test/harness dựng cây tay cũng gọi được).
        let existing: std::collections::HashSet<u32> = self.leaves().into_iter().collect();
        for idx in comments {
            if existing.contains(&idx) {
                continue;
            }
            let span = self.tokens[idx as usize].span;
            let toks = &self.tokens;
            let node = self.root.deepest_containing(span);
            node.insert_leaf(idx, span, toks);
        }
    }

    /// Các lá token theo **đúng thứ tự nguồn** (duyệt trước-sau, trái sang phải).
    pub fn leaves(&self) -> Vec<u32> {
        let mut out = Vec::new();
        fn go(n: &Node, out: &mut Vec<u32>) {
            for e in &n.elems {
                match e {
                    Elem::Token(i) => out.push(*i),
                    Elem::Node(c) => go(c, out),
                }
            }
        }
        go(&self.root, &mut out);
        out
    }

    /// Render **nguyên văn**: gap `text[prev_end..token.start]` + `text[token.span]`, cuối cùng là
    /// phần đuôi `text[last_end..]`. Phải bằng `LexOutput.text` với nguồn parse thành công.
    pub fn render(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() + 16);
        let mut last = 0usize;
        for idx in self.leaves() {
            let t = &self.tokens[idx as usize];
            let start = t.span.start.min(text.len());
            let end = t.span.end.min(text.len());
            if start > last && text.is_char_boundary(last) && text.is_char_boundary(start) {
                out.push_str(&text[last..start]);
            }
            if text.is_char_boundary(start) && text.is_char_boundary(end) {
                out.push_str(&text[start..end]);
            }
            last = end;
        }
        if last < text.len() && text.is_char_boundary(last) {
            out.push_str(&text[last..]);
        }
        out
    }

    /// Sexp của **CmdCST** (chế độ lệnh) — quy ước ở đầu `viscript/test/cmd/golden.cmd` (ST-06).
    ///
    /// **Pha A chưa hiện thực** (D2-05): trả chuỗi đánh dấu để harness `tests/cmd.rs` đỏ; pha B
    /// mới dựng CmdCST từ `parse(Mode::Command)`.
    pub fn sexp_cmd(&self) -> String {
        self.cmd_sexp
            .clone()
            .unwrap_or_else(|| "# không phải CmdCST (Mode::Script)".to_string())
    }

    /// Sexp của **biểu thức gốc** (chương trình chỉ có đúng một `ExprStmt`) — định dạng quy ước
    /// ở đầu `viscript/test/expr/golden.expr` (VD-34).
    ///
    /// `None` nếu tệp không phải "một biểu thức" (harness `tests/expr.rs` dùng để báo lỗi rõ).
    pub fn sexp_expr(&self) -> Option<String> {
        let stmt = self
            .root
            .children()
            .find(|n| n.kind == NodeKind::ExprStmt)?;
        let expr = stmt.children().next()?;
        Some(self.sexp(expr))
    }

    /// Sexp của một node (quy ước VD-34).
    pub fn sexp(&self, node: &Node) -> String {
        match node.kind {
            NodeKind::Ident | NodeKind::Literal => {
                let txt = node
                    .tokens(&self.tokens)
                    .map(|t| t.text.as_str())
                    .collect::<String>();
                txt
            }
            NodeKind::BinOp => {
                let mut it = node.elems.iter();
                let lhs = next_node(&mut it, self);
                let op = next_token(&mut it, self);
                let rhs = next_node(&mut it, self);
                format!("({op} {lhs} {rhs})")
            }
            NodeKind::UnOp => {
                let mut it = node.elems.iter();
                let op = next_token(&mut it, self);
                let x = next_node(&mut it, self);
                format!("({op} {x})")
            }
            NodeKind::Paren => {
                let inner = node
                    .children()
                    .next()
                    .map(|n| self.sexp(n))
                    .unwrap_or_default();
                format!("(paren {inner})")
            }
            NodeKind::Field | NodeKind::OptField => {
                let base = node
                    .children()
                    .next()
                    .map(|n| self.sexp(n))
                    .unwrap_or_default();
                let name = node
                    .elems
                    .iter()
                    .rev()
                    .find_map(|e| match e {
                        Elem::Token(i) => Some(self.tokens[*i as usize].text.clone()),
                        Elem::Node(_) => None,
                    })
                    .unwrap_or_default();
                let tag = if node.kind == NodeKind::Field {
                    "field"
                } else {
                    "?field"
                };
                format!("({tag} {base} {name})")
            }
            NodeKind::Call => {
                let mut out = String::from("(call");
                for c in node.children() {
                    match c.kind {
                        NodeKind::Args => {
                            for a in c.children() {
                                out.push(' ');
                                out.push_str(&self.sexp(a));
                            }
                        }
                        _ => {
                            out.push(' ');
                            out.push_str(&self.sexp(c));
                        }
                    }
                }
                out.push(')');
                out
            }
            NodeKind::GenericCall => {
                // `json.parse[Config](text)` — sexp theo dạng gọi hàm thường (test golden không dùng).
                let mut out = String::from("(call");
                for c in node.children() {
                    match c.kind {
                        NodeKind::Args => {
                            for a in c.children() {
                                out.push(' ');
                                out.push_str(&self.sexp(a));
                            }
                        }
                        _ => {
                            out.push(' ');
                            out.push_str(&self.sexp(c));
                        }
                    }
                }
                out.push(')');
                out
            }
            NodeKind::Index => {
                let mut it = node.children();
                let base = it.next().map(|n| self.sexp(n)).unwrap_or_default();
                let idx = it.next().map(|n| self.sexp(n)).unwrap_or_default();
                format!("(index {base} {idx})")
            }
            NodeKind::TryOp => {
                let inner = node
                    .children()
                    .next()
                    .map(|n| self.sexp(n))
                    .unwrap_or_default();
                format!("(try {inner})")
            }
            NodeKind::WithOp => {
                let mut it = node.children();
                let base = it.next().map(|n| self.sexp(n)).unwrap_or_default();
                let mut out = format!("(with {base}");
                for f in it {
                    out.push(' ');
                    out.push_str(&self.sexp(f));
                }
                out.push(')');
                out
            }
            NodeKind::WithField => {
                let name = node
                    .elems
                    .iter()
                    .find_map(|e| match e {
                        Elem::Token(i) => Some(self.tokens[*i as usize].text.clone()),
                        Elem::Node(_) => None,
                    })
                    .unwrap_or_default();
                let val = node
                    .children()
                    .next()
                    .map(|n| self.sexp(n))
                    .unwrap_or_default();
                format!("{name}: {val}")
            }
            NodeKind::ListLit | NodeKind::SetLit => {
                let tag = if node.kind == NodeKind::ListLit {
                    "list"
                } else {
                    "set"
                };
                let mut out = format!("({tag}");
                for c in node.children() {
                    out.push(' ');
                    out.push_str(&self.sexp(c));
                }
                out.push(')');
                out
            }
            NodeKind::MapLit => {
                let mut out = String::from("(map");
                for c in node.children() {
                    out.push(' ');
                    out.push_str(&self.sexp(c));
                }
                out.push(')');
                out
            }
            NodeKind::MapEntry => {
                let mut it = node.children();
                let k = it.next().map(|n| self.sexp(n)).unwrap_or_default();
                let v = it.next().map(|n| self.sexp(n)).unwrap_or_default();
                format!("{k} {v}")
            }
            NodeKind::RecordLit => {
                let mut out = String::from("(record");
                for c in node.children() {
                    out.push(' ');
                    out.push_str(&self.sexp(c));
                }
                out.push(')');
                out
            }
            NodeKind::RecordField => {
                let name = node
                    .elems
                    .iter()
                    .find_map(|e| match e {
                        Elem::Token(i) => Some(self.tokens[*i as usize].text.clone()),
                        Elem::Node(_) => None,
                    })
                    .unwrap_or_default();
                match node.children().next() {
                    Some(v) => format!("{name}: {}", self.sexp(v)),
                    None => name,
                }
            }
            NodeKind::Tuple => {
                let mut out = String::from("(tuple");
                for c in node.children() {
                    out.push(' ');
                    out.push_str(&self.sexp(c));
                }
                out.push(')');
                out
            }
            _ => {
                // Loại không có quy ước sexp trong golden (if/lambda/match/câu lệnh): in tên node
                // + con để log vẫn đọc được; golden không dùng tới.
                let mut out = format!("({}", node.kind.name());
                for c in node.children() {
                    out.push(' ');
                    out.push_str(&self.sexp(c));
                }
                out.push(')');
                out
            }
        }
    }
}

fn next_node<'a, I>(it: &mut I, cst: &'a Cst) -> String
where
    I: Iterator<Item = &'a Elem>,
{
    for e in it.by_ref() {
        if let Elem::Node(n) = e {
            return cst.sexp(n);
        }
    }
    String::new()
}

fn next_token<'a, I>(it: &mut I, cst: &'a Cst) -> String
where
    I: Iterator<Item = &'a Elem>,
{
    for e in it.by_ref() {
        if let Elem::Token(i) = e {
            return cst.tokens[*i as usize].text.clone();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex::{lex, Mode};

    fn build(src: &str) -> (Cst, String) {
        let out = lex(src, Mode::Script);
        let elems = out
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind != crate::lex::token::Kind::Eof)
            .map(|(i, _)| Elem::Token(i as u32))
            .collect();
        let span = Span::new(0, out.text.len());
        let cst = Cst::new(Node::new(NodeKind::Program, span, elems), out.tokens);
        (cst, out.text)
    }

    #[test]
    fn render_tra_nguyen_van() {
        for src in [
            "let x = 1\n",
            "# chú thích\nlet y = 2  # đuôi\n",
            "if x:\n\tprint(x)\n",
            "a |> b\n  |> c\n",
            "",
        ] {
            let (cst, text) = build(src);
            assert_eq!(cst.render(&text), text, "render phải lossless: {src:?}");
        }
    }

    #[test]
    fn comment_duoc_gan_vao_cay() {
        let (mut cst, text) = build("# mở đầu\nlet x = 1 # đuôi\n");
        cst.attach_comments();
        assert_eq!(cst.render(&text), text);
        let comments = cst
            .leaves()
            .into_iter()
            .filter(|i| {
                matches!(
                    cst.tokens[*i as usize].kind,
                    crate::lex::token::Kind::Comment
                )
            })
            .count();
        assert_eq!(comments, 2);
    }

    #[test]
    fn sexp_literal_va_nhi_phan() {
        // Dựng tay một BinOp(+, Literal(1), Literal(2)) để khoá định dạng.
        let toks = vec![
            Token::new(crate::lex::token::Kind::Int, "1", Span::new(0, 1), 1, 1),
            Token::new(crate::lex::token::Kind::Punct, "+", Span::new(2, 3), 1, 3),
            Token::new(crate::lex::token::Kind::Int, "2", Span::new(4, 5), 1, 5),
            Token::new(crate::lex::token::Kind::Eof, "", Span::new(5, 5), 1, 6),
        ];
        let lhs = Node::new(NodeKind::Literal, Span::new(0, 1), vec![Elem::Token(0)]);
        let rhs = Node::new(NodeKind::Literal, Span::new(4, 5), vec![Elem::Token(2)]);
        let bin = Node::new(
            NodeKind::BinOp,
            Span::new(0, 5),
            vec![
                Elem::Node(Box::new(lhs)),
                Elem::Token(1),
                Elem::Node(Box::new(rhs)),
            ],
        );
        let stmt = Node::new(
            NodeKind::ExprStmt,
            Span::new(0, 5),
            vec![Elem::Node(Box::new(bin))],
        );
        let root = Node::new(
            NodeKind::Program,
            Span::new(0, 5),
            vec![Elem::Node(Box::new(stmt))],
        );
        let cst = Cst::new(root, toks);
        assert_eq!(cst.sexp_expr().as_deref(), Some("(+ 1 2)"));
    }
}
