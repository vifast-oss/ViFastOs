// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-vm` — máy ảo **VBC1** cho ViScript (ST-20, VD-52/VD-54).
//!
//! # Ngữ nghĩa **trùng** `vis-interp`
//!
//! VM **không** có bảng ngữ nghĩa riêng: toán tử, builtin, phương thức, `Show`, so sánh và vật phẩm
//! `for` đều **gọi** hàm công khai của `vis-interp` (`builtin::{bin,un,call,method}`,
//! `value::{show,eq,cmp,iter_items}`). Nhờ vậy hai đường chạy không thể lệch nhau về ngữ nghĩa.
//!
//! # An toàn với bytecode hỏng
//!
//! Ngăn xếp là `Vec<Value>` **động** — **không** tin `max_stack` trong module (VD-54 (2)); mọi
//! `pop` rỗng ⇒ [`VmError`] ⇒ `VS0501` "bytecode hỏng", **không** panic. Cap khung gọi
//! [`MAX_FRAMES`] = 2.000 (VD-49) và cap bước [`MAX_STEPS`] = 10^7, **cùng mã/cùng thông điệp**
//! với interp.
//!
//! # `steps` khác đơn vị
//!
//! `steps` của VM đếm **lệnh đã thực thi**, của interp đếm **bước ngữ nghĩa** ⇒ test differential
//! **không** so `steps` (chỉ `stdout`, `exit`, `(mã, dòng, cột)` của chẩn đoán).

pub mod dump;
pub mod vm;

pub use vm::{run, Vm, VmError, MAX_FRAMES, MAX_STEPS};
