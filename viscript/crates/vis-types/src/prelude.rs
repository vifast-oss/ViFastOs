// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Prelude của chế độ lệnh (ST-40a)** — hai bản ghi dựng sẵn, khai **trong Rust** (không phải tệp
//! `.vis` của người dùng), đăng ký **chỉ** khi hạ `.vish`:
//!
//! - `FileEntry { name: str, size: size, modified: duration, is_file: bool }` — cho `walk`/`ls` (ST-42);
//! - `ProcEntry { pid: int, name: str, cpu: percent, mem: size }` — cho `ps` (ST-42);
//! - `FileEntryLong` (ST-46) — **bản ghi riêng** cho `ls --long`: thêm `mode: str`, `modified_time: str`
//!   để bảng mặc định 4 cột **không đổi**.
//!
//! Nhờ vậy `it.size`/`it.cpu` trong vị từ `where`/khoá `sort_by` có **kiểu thật** (`size`, `percent`) thay
//! vì `{error}` ⇒ kiểm tĩnh được. `.vis` **không** thấy hai tên này (khai trùng trong `.vish` ⇒ `VS0302`).

use crate::ty::{FieldDef, RecDef, RecId, Type, TypeTable};
use vis_syntax::Span;

/// Một trường của bản ghi prelude.
pub struct Truong {
    pub ten: &'static str,
    pub ty: Type,
}

/// Bản ghi prelude: tên + danh sách trường (thứ tự khai).
pub struct BanGhi {
    pub ten: &'static str,
    pub truong: Vec<Truong>,
}

fn truong(ten: &'static str, ty: Type) -> Truong {
    Truong { ten, ty }
}

/// Các bản ghi prelude của `.vish` (ST-40a; `FileEntryLong` ST-46).
pub fn bang_ghi() -> Vec<BanGhi> {
    vec![
        BanGhi {
            ten: "FileEntry",
            truong: vec![
                truong("name", Type::Str),
                truong("size", Type::Size),
                truong("modified", Type::Duration),
                truong("is_file", Type::Bool),
            ],
        },
        BanGhi {
            ten: "FileEntryLong",
            truong: vec![
                truong("name", Type::Str),
                truong("size", Type::Size),
                truong("modified", Type::Duration),
                truong("is_file", Type::Bool),
                truong("mode", Type::Str),
                truong("modified_time", Type::Str),
            ],
        },
        BanGhi {
            ten: "ProcEntry",
            truong: vec![
                truong("pid", Type::Int),
                truong("name", Type::Str),
                truong("cpu", Type::Percent),
                truong("mem", Type::Size),
            ],
        },
    ]
}

impl BanGhi {
    /// Đăng ký bản ghi này vào `tbl` và trả `(tên, RecId)`.
    pub fn dang_ky(&self, tbl: &mut TypeTable) -> (String, RecId) {
        let fields = self
            .truong
            .iter()
            .map(|t| FieldDef {
                name: t.ten.to_string(),
                ty: t.ty.clone(),
                span: Span::new(0, 0),
                has_default: false,
            })
            .collect();
        let id = tbl.push_rec(RecDef {
            name: Some(self.ten.to_string()),
            fields,
            span: Span::new(0, 0),
            anonymous: false,
        });
        (self.ten.to_string(), id)
    }
}

/// Đăng ký **cả hai** bản ghi prelude; trả danh sách `(tên, RecId)` theo thứ tự khai.
pub fn dang_ky_vish(tbl: &mut TypeTable) -> Vec<(String, RecId)> {
    bang_ghi().iter().map(|b| b.dang_ky(tbl)).collect()
}
