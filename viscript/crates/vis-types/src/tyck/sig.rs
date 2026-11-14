// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-60b**: `FnSig` — chữ ký đầy đủ của hàm khai trong tệp (tách khỏi `expr.rs` cho luật **R1**
//! ≤1.500 dòng/tệp). Nội dung/ngữ nghĩa **không** đổi.

use crate::ty::Type;

/// Chữ ký một hàm **khai trong tệp** (giữ tên tham số + có giá trị mặc định hay không).
#[derive(Debug, Clone)]
pub(crate) struct FnSig {
    pub name: String,
    /// **ST-60**: `async fn` — gọi **không** chạy thân, trả `Task[ret]`.
    pub is_async: bool,
    /// `(tên, kiểu, có_mặc_định, khai `--`)` — kiểu đã **xoá generic** (ST-28), dùng cho thân hàm.
    pub params: Vec<(String, Type, bool, bool)>,
    pub ret: Type,
    /// Tham số cuối là `...rest` ⇒ nhận **thêm** đối số vị trí (ST-27).
    pub variadic: bool,
    /// Tham số kiểu của hàm (`fn f[T]`) — ST-28.
    pub generics: Vec<String>,
    /// Kiểu tham số **nguyên bản** (còn `TypeRef{"T"}`) để **hợp nhất** ở call-site.
    pub raw_params: Vec<Type>,
    /// Kiểu trả **nguyên bản** (còn `T`) — thay bằng kiểu đã suy sau khi hợp nhất.
    pub raw_ret: Type,
}

impl FnSig {
    /// Kiểu trả đã suy (dùng sau `hop_nhat`) — chuyển từ `expr.rs` (ST-61, giữ R1).
    pub(crate) fn ret_raw(&self) -> Type {
        self.raw_ret.clone()
    }
}
