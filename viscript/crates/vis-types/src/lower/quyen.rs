// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-72 (VD-106 (2))** — kiểm `@ p"…"`/`@ "host"` + `(write)` trên tham số `cap[...]`.
//!
//! `@` chỉ hợp lệ: `cap[Dir]`/`cap[File]` + **path literal**, `cap[Net]` + **str literal**.
//! `(write)` chỉ hợp lệ với `cap[Dir]`/`cap[File]` và chỉ quyền `write`.

use vis_diag::TYPE_CODES;
use vis_syntax::{Kind, Node, NodeKind, Token};

use crate::ty::Type;

impl<'a> super::Lowerer<'a> {
    /// **ST-72 (VD-106 (2))**: kiểm `@` + `(write)` trên tham số theo loại `cap[...]`.
    ///
    /// `@` chỉ hợp lệ: `cap[Dir]`/`cap[File]` + **path literal**, `cap[Net]` + **str literal**.
    /// `(write)` chỉ hợp lệ với `cap[Dir]`/`cap[File]` và chỉ quyền `write`.
    pub(super) fn kiem_cap_param(&mut self, p: &Node, ty: &Type) {
        const LOAI: &vis_diag::Code = &TYPE_CODES[4]; // VS0412
                                                      // Parser **tiêu** token `@` (không đưa vào CST) nhưng **giữ** token literal ngay sau nó
                                                      // ⇒ nhận diện `@` bằng literal đường dẫn/chuỗi **trực tiếp** của `Param`.
        let toks: Vec<&Token> = p.tokens(&self.cst.tokens).collect();
        let at_lit: Option<Kind> = toks
            .iter()
            .find(|t| matches!(t.kind, Kind::Path | Kind::Str))
            .map(|t| t.kind);
        let grants: Vec<String> = p
            .child(NodeKind::GrantList)
            .map(|g| {
                g.children()
                    .filter(|n| n.kind == NodeKind::DottedPath)
                    .map(|n| self.dotted_text(n))
                    .collect()
            })
            .unwrap_or_default();
        let cap_kind: Option<String> = match ty {
            Type::TypeRef { name, args } if name == "cap" && args.len() == 1 => match &args[0] {
                Type::TypeRef { name, args } if args.is_empty() => Some(name.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(lit) = at_lit {
            match (cap_kind.as_deref(), lit) {
                (Some("Dir") | Some("File"), Kind::Path) | (Some("Net"), Kind::Str) => {}
                (Some("Dir") | Some("File"), _) => self.diag(
                    LOAI,
                    "`@` với `cap[Dir]`/`cap[File]` cần literal đường dẫn `p\"…\"`",
                    p.span,
                ),
                (Some("Net"), _) => self.diag(
                    LOAI,
                    "`@` với `cap[Net]` cần literal host `\"…\"`",
                    p.span,
                ),
                _ => self.diag(
                    LOAI,
                    "`@` chỉ hợp lệ trên tham số `cap[Dir]`/`cap[File]` (đường dẫn) hoặc `cap[Net]` (host)",
                    p.span,
                ),
            }
        }
        if !grants.is_empty() {
            match cap_kind.as_deref() {
                Some("Dir") | Some("File") => {
                    for g in &grants {
                        if g != "write" {
                            self.diag(
                                LOAI,
                                format!("quyền `{g}` không hỗ trợ cho `cap` — chỉ `(write)`"),
                                p.span,
                            );
                        }
                    }
                }
                _ => self.diag(
                    LOAI,
                    "danh sách quyền `(...)` chỉ hợp lệ với `cap[Dir]`/`cap[File]`",
                    p.span,
                ),
            }
        }
    }
}
