// SPDX-License-Identifier: MIT OR Apache-2.0
//! Biểu thức theo Phụ lục A.3 + bảng ưu tiên A.5 (ST-05).
//!
//! Đệ quy xuống đúng 12 mức ưu tiên (thấp → cao): `|>` → `or` → `and` → `not` → so sánh
//! (không kết hợp) → `??` (phải) → khoảng (không kết hợp) → `+ -` → `* / %` → một ngôi `-` →
//! `**` (phải) → postfix → primary. `**` cao hơn `-` một ngôi nên `-2 ** 2` = `-(2 ** 2)`;
//! `**` kết hợp phải nên `2 ** 3 ** 2` = `2 ** (3 ** 2)` (A.5, §4.5).

use super::{Elem, Node, NodeKind, PResult, Parser};
use crate::lex::token::Kind;

/// Ba ca lỗi của golden cần hành vi riêng: `50%` (VS0202) và hai toán tử không kết hợp.
impl<'a> Parser<'a> {
    /// `expr := lambda | pipe_expr`.
    pub(crate) fn expr(&mut self) -> PResult<Node> {
        if self.at_lambda() {
            return self.lambda();
        }
        self.pipe_expr()
    }

    /// Lambda: `IDENT => expr`, `( a, b ) => expr`, `() => expr`, `fn(...) -> T: suite`.
    fn at_lambda(&mut self) -> bool {
        if self.at_kw("fn") {
            return true;
        }
        if self.tok().kind == Kind::Ident && self.peek_punct(1, "=>") {
            return true;
        }
        if self.at_punct("(") {
            // Quét tới ')' cân bằng rồi xem token kế có phải `=>`.
            let mut depth = 0usize;
            let mut k = 0usize;
            loop {
                let t = self.tok_at(k);
                if t.kind == Kind::Eof {
                    return false;
                }
                if t.is_punct("(") {
                    depth += 1;
                } else if t.is_punct(")") {
                    depth -= 1;
                    if depth == 0 {
                        return self.peek_punct(k + 1, "=>");
                    }
                } else if t.is_punct(",") || t.kind == Kind::Ident {
                    // hợp lệ trong danh sách tham số lambda
                } else {
                    return false;
                }
                k += 1;
            }
        }
        false
    }

    fn lambda(&mut self) -> PResult<Node> {
        let mut elems: Vec<Elem> = Vec::new();
        if self.at_kw("fn") {
            // `fn(params) -> T: suite`
            elems.push(self.bump()); // fn
            elems.push(self.expect_punct("(")?);
            let mut params = Vec::new();
            while !self.at_punct(")") {
                let mut p = Vec::new();
                p.push(self.expect_ident()?);
                if self.eat_punct(":").is_some() {
                    p.push(Elem::Node(Box::new(self.parse_type()?)));
                }
                params.push(Elem::Node(Box::new(self.node(NodeKind::Param, p))));
                if self.eat_punct(",").is_none() {
                    break;
                }
            }
            elems.push(Elem::Node(Box::new(
                self.node(NodeKind::LambdaParams, params),
            )));
            elems.push(self.expect_punct(")")?);
            if self.eat_punct("->").is_some() {
                elems.push(Elem::Node(Box::new(self.parse_type()?)));
            }
            elems.push(self.expect_punct(":")?);
            elems.push(Elem::Node(Box::new(self.suite()?)));
            return Ok(self.node(NodeKind::Lambda, elems));
        }
        if self.at_punct("(") {
            elems.push(self.bump());
            let mut params = Vec::new();
            while !self.at_punct(")") {
                let mut p = Vec::new();
                p.push(self.expect_ident()?);
                if self.eat_punct(":").is_some() {
                    p.push(Elem::Node(Box::new(self.parse_type()?)));
                }
                params.push(Elem::Node(Box::new(self.node(NodeKind::Param, p))));
                if self.eat_punct(",").is_none() {
                    break;
                }
            }
            elems.push(Elem::Node(Box::new(
                self.node(NodeKind::LambdaParams, params),
            )));
            elems.push(self.expect_punct(")")?);
        } else {
            let mut params = Vec::new();
            let id = self.expect_ident()?;
            params.push(Elem::Node(Box::new(self.node(NodeKind::Param, vec![id]))));
            elems.push(Elem::Node(Box::new(
                self.node(NodeKind::LambdaParams, params),
            )));
        }
        elems.push(self.expect_punct("=>")?);
        let body = self.expr()?;
        elems.push(Elem::Node(Box::new(body)));
        Ok(self.node(NodeKind::Lambda, elems))
    }

    /// `pipe_expr := or_expr ('|>' or_expr)*` — kết hợp trái.
    fn pipe_expr(&mut self) -> PResult<Node> {
        let mut lhs = self.or_expr()?;
        while let Some(op) = self.eat_punct("|>") {
            let rhs = self.or_expr()?;
            lhs = self.binop(lhs, op, rhs);
        }
        Ok(lhs)
    }

    fn or_expr(&mut self) -> PResult<Node> {
        let mut lhs = self.and_expr()?;
        while let Some(op) = self.eat_kw("or") {
            let rhs = self.and_expr()?;
            lhs = self.binop(lhs, op, rhs);
        }
        Ok(lhs)
    }

    fn and_expr(&mut self) -> PResult<Node> {
        let mut lhs = self.not_expr()?;
        while let Some(op) = self.eat_kw("and") {
            let rhs = self.not_expr()?;
            lhs = self.binop(lhs, op, rhs);
        }
        Ok(lhs)
    }

    fn not_expr(&mut self) -> PResult<Node> {
        if let Some(op) = self.eat_kw("not") {
            let x = self.not_expr()?;
            return Ok(self.unop(op, x));
        }
        self.cmp_expr()
    }

    /// `cmp_expr := coalesce [cmp_op coalesce]` — **không kết hợp**: `1 < x < 10` là lỗi (A.5).
    fn cmp_expr(&mut self) -> PResult<Node> {
        let lhs = self.coalesce()?;
        if let Some(op) = self.eat_cmp_op() {
            let rhs = self.coalesce()?;
            let n = self.binop(lhs, op, rhs);
            if self.at_cmp_op() {
                return Err(self.err_here(format!(
                    "toán tử so sánh không kết hợp — thêm ngoặc (A.5); thật `{}`",
                    self.tok().text
                )));
            }
            return Ok(n);
        }
        Ok(lhs)
    }

    fn at_cmp_op(&self) -> bool {
        ["==", "!=", "<", "<=", ">", ">="]
            .iter()
            .any(|s| self.at_punct(s))
    }

    fn eat_cmp_op(&mut self) -> Option<Elem> {
        self.eat_any_punct(&["==", "!=", "<", "<=", ">", ">="])
    }

    /// `coalesce := range_expr ('??' range_expr)*` — kết hợp phải (A.5).
    fn coalesce(&mut self) -> PResult<Node> {
        let lhs = self.range_expr()?;
        if let Some(op) = self.eat_punct("??") {
            let rhs = self.coalesce()?;
            return Ok(self.binop(lhs, op, rhs));
        }
        Ok(lhs)
    }

    /// `range_expr := add_expr [('..' | '..=') add_expr]` — không kết hợp.
    fn range_expr(&mut self) -> PResult<Node> {
        let lhs = self.add_expr()?;
        let op = self.eat_any_punct(&["..", "..="]);
        if let Some(op) = op {
            let rhs = self.add_expr()?;
            let n = self.binop(lhs, op, rhs);
            if self.at_punct("..") || self.at_punct("..=") {
                return Err(self.err_here(
                    "toán tử khoảng không kết hợp — thêm ngoặc (A.5); `a .. b .. c` là lỗi",
                ));
            }
            return Ok(n);
        }
        Ok(lhs)
    }

    fn add_expr(&mut self) -> PResult<Node> {
        let mut lhs = self.mul_expr()?;
        loop {
            let op = self.eat_any_punct(&["+", "-"]);
            match op {
                Some(op) => {
                    let rhs = self.mul_expr()?;
                    lhs = self.binop(lhs, op, rhs);
                }
                None => return Ok(lhs),
            }
        }
    }

    fn mul_expr(&mut self) -> PResult<Node> {
        let mut lhs = self.unary_expr()?;
        loop {
            // Ca L3 `50%`: `%` không có vế phải ⇒ VS0202 kèm gợi ý `50pct`.
            if self.at_punct("%") && matches!(self.tok_at(1).kind, Kind::Newline | Kind::Eof) {
                return Err(self.err_code(
                    &vis_diag::SYNTAX_CODES[1],
                    "`%` chỉ là modulo; `50%` không phải phần trăm — dùng `50pct`",
                ));
            }
            let op = self.eat_any_punct(&["*", "/", "%"]);
            match op {
                Some(op) => {
                    let rhs = self.unary_expr()?;
                    lhs = self.binop(lhs, op, rhs);
                }
                None => return Ok(lhs),
            }
        }
    }

    /// `unary_expr := '-' unary_expr | power_expr` — một ngôi **thấp hơn** `**`.
    fn unary_expr(&mut self) -> PResult<Node> {
        if let Some(op) = self.eat_punct("-") {
            let x = self.unary_expr()?;
            return Ok(self.unop(op, x));
        }
        self.power_expr()
    }

    /// `power_expr := postfix ['**' unary_expr]` — kết hợp phải.
    fn power_expr(&mut self) -> PResult<Node> {
        let base = self.postfix()?;
        if let Some(op) = self.eat_punct("**") {
            let rhs = self.unary_expr()?;
            return Ok(self.binop(base, op, rhs));
        }
        Ok(base)
    }

    fn binop(&self, lhs: Node, op: Elem, rhs: Node) -> Node {
        let span = crate::lex::token::Span::new(lhs.span.start, rhs.span.end);
        Node::new(
            NodeKind::BinOp,
            span,
            vec![Elem::Node(Box::new(lhs)), op, Elem::Node(Box::new(rhs))],
        )
    }

    fn unop(&self, op: Elem, x: Node) -> Node {
        let span = crate::lex::token::Span::new(
            self.elem_span(&op).map(|s| s.start).unwrap_or(x.span.start),
            x.span.end,
        );
        Node::new(NodeKind::UnOp, span, vec![op, Elem::Node(Box::new(x))])
    }

    /// `postfix := primary postfix_op*` (A.3) — `. ?. () [] ? with`, cộng gọi generic.
    fn postfix(&mut self) -> PResult<Node> {
        let mut base = self.primary()?;
        loop {
            if self.at_punct(".") {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(self.bump());
                // ST-13b (mục 6): sau dấu chấm nhận cả **từ khoá** làm tên trường/phương thức
                // (`m.has(..)`, `x.type`) — từ khoá vẫn là từ khoá ở mọi vị trí khác.
                let name = if self.tok().kind == Kind::Kw {
                    self.bump()
                } else {
                    self.expect_ident()?
                };
                elems.push(name);
                base = self.node(NodeKind::Field, elems);
                continue;
            }
            if self.at_punct("?.") {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(self.bump());
                let name = if self.tok().kind == Kind::Kw {
                    self.bump()
                } else {
                    self.expect_ident()?
                };
                elems.push(name);
                base = self.node(NodeKind::OptField, elems);
                continue;
            }
            // Gọi generic: `json.parse[Config](text)` — `[` types `]` rồi `(`.
            if self.at_punct("[") && self.is_generic_call_ahead() {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(self.bump());
                let mut types = Vec::new();
                types.push(Elem::Node(Box::new(self.parse_type()?)));
                while self.eat_punct(",").is_some() {
                    types.push(Elem::Node(Box::new(self.parse_type()?)));
                }
                elems.push(Elem::Node(Box::new(self.node(NodeKind::Generics, types))));
                elems.push(self.expect_punct("]")?);
                elems.push(Elem::Node(Box::new(self.args()?)));
                base = self.node(NodeKind::GenericCall, elems);
                continue;
            }
            if self.at_punct("(") {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(Elem::Node(Box::new(self.args()?)));
                base = self.node(NodeKind::Call, elems);
                continue;
            }
            if self.at_punct("[") {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(self.bump());
                let idx = self.expr()?;
                elems.push(Elem::Node(Box::new(idx)));
                elems.push(self.expect_punct("]")?);
                base = self.node(NodeKind::Index, elems);
                continue;
            }
            if self.at_punct("?") {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(self.bump());
                base = self.node(NodeKind::TryOp, elems);
                continue;
            }
            if self.at_kw("with") {
                let mut elems = vec![Elem::Node(Box::new(base))];
                elems.push(self.bump());
                if self.tok().kind != Kind::Ident {
                    return Err(self.err_here("`with` phải có ít nhất một trường `ten: giá_trị`"));
                }
                while self.tok().kind == Kind::Ident {
                    let mut f = vec![self.bump()];
                    f.push(self.expect_punct(":")?);
                    // `with` bám chặt hơn `|>` (golden #17): giá trị trường dừng trước `|>`.
                    let v = self.or_expr()?;
                    f.push(Elem::Node(Box::new(v)));
                    elems.push(Elem::Node(Box::new(self.node(NodeKind::WithField, f))));
                    if self.eat_punct(",").is_none() {
                        break;
                    }
                }
                base = self.node(NodeKind::WithOp, elems);
                continue;
            }
            return Ok(base);
        }
    }

    /// Nhìn trước: `[` … `]` rồi `(` ⇒ danh sách kiểu của gọi generic (ghi chú 4 A.5).
    fn is_generic_call_ahead(&self) -> bool {
        let mut depth = 0usize;
        let mut k = 0usize;
        loop {
            let t = self.tok_at(k);
            if t.kind == Kind::Eof {
                return false;
            }
            if t.is_punct("[") {
                depth += 1;
            } else if t.is_punct("]") {
                depth -= 1;
                if depth == 0 {
                    return self.peek_punct(k + 1, "(");
                }
            } else if t.is_punct("(")
                || t.is_punct(")")
                || t.is_punct("{")
                || t.is_punct("}")
                || t.is_punct(";")
            {
                return false;
            }
            k += 1;
        }
    }

    /// `args := positional_args? named_args?` — `--ten` là tham số có tên (A.3).
    ///
    /// Trả **một** node `Args` chứa `(` · (đối số | `,`) · `)`: dấu phẩy là lá token để CST
    /// lossless; `sexp` của `Call` duyệt node con nên không thấy dấu phẩy.
    fn args(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_punct("(")?];
        let mut seen_named = false;
        while !self.at_punct(")") {
            if self.at_punct("-") && self.peek_punct(1, "-") {
                seen_named = true;
                let mut na = vec![self.bump(), self.bump()];
                na.push(self.expect_ident()?);
                if self.at_punct(":") {
                    na.push(self.bump());
                    let v = self.expr()?;
                    na.push(Elem::Node(Box::new(v)));
                }
                elems.push(Elem::Node(Box::new(self.node(NodeKind::NamedArg, na))));
            } else {
                if seen_named {
                    return Err(self.err_here(
                        "tham số có tên phải đứng **sau** tham số vị trí (A.3: `positional_args? named_args?`)",
                    ));
                }
                let a = self.expr()?;
                elems.push(Elem::Node(Box::new(a)));
            }
            if self.at_punct(",") {
                elems.push(self.bump());
            } else {
                break;
            }
        }
        elems.push(self.expect_punct(")")?);
        Ok(self.node(NodeKind::Args, elems))
    }

    /// `primary := literal | IDENT | ref(...) | () | (expr) | tuple | list | map | set | record
    /// | cmd {...} | if_expr | match_expr | await postfix` (A.3).
    fn primary(&mut self) -> PResult<Node> {
        let t = self.tok();
        match t.kind {
            Kind::Int
            | Kind::Float
            | Kind::Char
            | Kind::Str
            | Kind::RawStr
            | Kind::MultiStr
            | Kind::RawMulti
            | Kind::Bytes
            | Kind::Path
            | Kind::Size
            | Kind::Duration
            | Kind::Percent => {
                let e = self.bump();
                return Ok(self.node(NodeKind::Literal, vec![e]));
            }
            Kind::SetLbrace => {
                let mut elems = vec![self.bump()];
                let mut kids = Vec::new();
                while !self.at_punct("}") {
                    kids.push(self.expr()?);
                    if self.eat_punct(",").is_none() {
                        break;
                    }
                }
                elems.extend(kids.into_iter().map(|k| Elem::Node(Box::new(k))));
                elems.push(self.expect_punct("}")?);
                return Ok(self.node(NodeKind::SetLit, elems));
            }
            // **ST-60**: `spawn f(args)` — **soft keyword**: `spawn` + token kế là **định danh**.
            // `s.spawn(1)` giữ nguyên là **phương thức** (sau `.` là Ident, không vào nhánh này).
            Kind::Ident if t.text == "spawn" && self.tok_at(1).kind == Kind::Ident => {
                let mut elems = vec![self.bump()];
                let ten = self.bump();
                elems.push(Elem::Node(Box::new(self.node(NodeKind::Ident, vec![ten]))));
                let a = self.args()?;
                elems.push(Elem::Node(Box::new(a)));
                return Ok(self.node(NodeKind::SpawnExpr, elems));
            }
            Kind::Ident => {
                let e = self.bump();
                return Ok(self.node(NodeKind::Ident, vec![e]));
            }
            Kind::Kw if t.text == "true" || t.text == "false" || t.text == "none" => {
                let e = self.bump();
                return Ok(self.node(NodeKind::Literal, vec![e]));
            }
            Kind::Kw if t.text == "self" => {
                let e = self.bump();
                return Ok(self.node(NodeKind::Ident, vec![e]));
            }
            Kind::Kw if t.text == "ref" => {
                let mut elems = vec![self.bump()];
                elems.push(self.expect_punct("(")?);
                let x = self.expr()?;
                elems.push(Elem::Node(Box::new(x)));
                elems.push(self.expect_punct(")")?);
                return Ok(self.node(NodeKind::RefExpr, elems));
            }
            Kind::Kw if t.text == "await" => {
                let mut elems = vec![self.bump()];
                let x = self.postfix()?;
                elems.push(Elem::Node(Box::new(x)));
                return Ok(self.node(NodeKind::AwaitExpr, elems));
            }
            Kind::Kw if t.text == "if" => return self.if_expr(),
            Kind::Kw if t.text == "match" => return self.match_expr(),
            Kind::Kw if t.text == "cmd" && self.peek_punct(1, "{") => return self.cmd_block(),
            Kind::Kw if t.text == "fn" => return self.lambda(),
            Kind::Punct if t.text == "(" => return self.paren_or_tuple(),
            Kind::Punct if t.text == "[" => return self.list_lit(),
            Kind::Punct if t.text == "{" => return self.map_or_record(),
            _ => {}
        }
        Err(self.err_here(format!(
            "mong biểu thức, thật `{}` ({})",
            t.text,
            t.kind.name()
        )))
    }

    fn paren_or_tuple(&mut self) -> PResult<Node> {
        let mut elems = vec![self.bump()];
        if self.at_punct(")") {
            elems.push(self.bump());
            return Ok(self.node(NodeKind::Tuple, elems)); // unit `()`
        }
        let first = self.expr()?;
        if self.at_punct(",") {
            let mut kids = vec![first];
            while self.eat_punct(",").is_some() {
                if self.at_punct(")") {
                    break;
                }
                kids.push(self.expr()?);
            }
            if kids.len() < 2 {
                return Err(
                    self.err_here("tuple phải có ≥2 phần tử (dùng `()` cho unit, `(x)` cho nhóm)")
                );
            }
            let mut e = vec![elems.remove(0)];
            e.extend(kids.into_iter().map(|k| Elem::Node(Box::new(k))));
            e.push(self.expect_punct(")")?);
            return Ok(self.node(NodeKind::Tuple, e));
        }
        elems.push(Elem::Node(Box::new(first)));
        elems.push(self.expect_punct(")")?);
        Ok(self.node(NodeKind::Paren, elems))
    }

    fn list_lit(&mut self) -> PResult<Node> {
        let mut elems = vec![self.bump()];
        while !self.at_punct("]") {
            let e = self.expr()?;
            elems.push(Elem::Node(Box::new(e)));
            if self.eat_punct(",").is_none() {
                break;
            }
        }
        elems.push(self.expect_punct("]")?);
        Ok(self.node(NodeKind::ListLit, elems))
    }

    /// `{ IDENT ':'` ⇒ bản ghi; ngược lại map (ghi chú 4 A.5).
    fn map_or_record(&mut self) -> PResult<Node> {
        let record = self.tok_at(1).kind == Kind::Ident && self.peek_punct(2, ":");
        let mut elems = vec![self.bump()];
        while !self.at_punct("}") {
            if record {
                let mut f = vec![self.expect_ident()?];
                f.push(self.expect_punct(":")?);
                let v = self.expr()?;
                f.push(Elem::Node(Box::new(v)));
                elems.push(Elem::Node(Box::new(self.node(NodeKind::RecordField, f))));
            } else {
                let key = self.expr()?;
                let m = vec![
                    Elem::Node(Box::new(key)),
                    self.expect_punct(":")?,
                    Elem::Node(Box::new(self.expr()?)),
                ];
                elems.push(Elem::Node(Box::new(self.node(NodeKind::MapEntry, m))));
            }
            if self.eat_punct(",").is_none() {
                break;
            }
        }
        elems.push(self.expect_punct("}")?);
        let kind = if record {
            NodeKind::RecordLit
        } else {
            NodeKind::MapLit
        };
        Ok(self.node(kind, elems))
    }

    /// `if_expr := 'if' if_cond ':' suite elif_arm* else_arm` — **else bắt buộc** (A.3).
    fn if_expr(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("if")?];
        let cond = self.if_cond()?;
        elems.push(Elem::Node(Box::new(cond)));
        elems.push(self.expect_punct(":")?);
        let then = self.suite()?;
        elems.push(Elem::Node(Box::new(then)));
        while self.at_kw("elif") {
            let mut arm = vec![self.bump()];
            let c = self.if_cond()?;
            arm.push(Elem::Node(Box::new(c)));
            arm.push(self.expect_punct(":")?);
            arm.push(Elem::Node(Box::new(self.suite()?)));
            elems.push(Elem::Node(Box::new(self.node(NodeKind::ElifArm, arm))));
        }
        if self.at_kw("else") {
            let mut arm = vec![self.bump()];
            arm.push(self.expect_punct(":")?);
            arm.push(Elem::Node(Box::new(self.suite()?)));
            elems.push(Elem::Node(Box::new(self.node(NodeKind::ElseArm, arm))));
        } else {
            return Err(self.err_here("`if` dùng như biểu thức phải có `else` (A.3)"));
        }
        Ok(self.node(NodeKind::IfExpr, elems))
    }

    /// `match_expr := 'match' expr ':' NEWLINE INDENT match_arm+ DEDENT` (dạng biểu thức).
    fn match_expr(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("match")?];
        let subject = self.expr()?;
        elems.push(Elem::Node(Box::new(subject)));
        elems.push(self.expect_punct(":")?);
        elems.push(Elem::Node(Box::new(self.match_body()?)));
        Ok(self.node(NodeKind::MatchExpr, elems))
    }

    /// `cmd '{' pipeline '}'` — ST-05 giữ **nguyên token** pipeline; `CmdCST` đầy đủ là ST-06.
    fn cmd_block(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("cmd")?, self.expect_punct("{")?];
        let mut depth = 1usize;
        while depth > 0 {
            if self.at_eof() {
                return Err(self.err_here("`cmd {` chưa đóng `}`"));
            }
            if self.at_punct("{") {
                depth += 1;
            } else if self.at_punct("}") {
                depth -= 1;
            }
            elems.push(self.bump());
        }
        Ok(self.node(NodeKind::CmdBlock, elems))
    }

    /// `if_cond := 'let' pattern '=' expr | expr`.
    pub(crate) fn if_cond(&mut self) -> PResult<Node> {
        if self.at_kw("let") {
            let mut elems = vec![self.bump()];
            let p = self.parse_pattern()?;
            elems.push(Elem::Node(Box::new(p)));
            elems.push(self.expect_punct("=")?);
            let e = self.expr()?;
            elems.push(Elem::Node(Box::new(e)));
            return Ok(self.node(NodeKind::Pattern, elems));
        }
        self.expr()
    }
}
