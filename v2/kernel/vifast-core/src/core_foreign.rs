// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-67** — object **đại diện** cho object VM của *object layer* (D2-83, cầu nối một chiều).
//!
//! Tách khỏi `machine.rs` để giữ R1 ≤ 1.500 dòng/tệp. Ngữ nghĩa:
//!
//! - core **không** Retype `Frame`/`PageTable`/`VSpace` (pin D2-83 trong `machine.rs`); nó chỉ
//!   giữ **cap + key** cho object mà object layer (`vifast_plat_common::obj`, KT-52) đã cấp.
//! - object đại diện có `untyped: None`, `region: None`, `len: 0` ⇒ **không** tiêu byte arena và
//!   **không** vào `check_bytes`/`check_all_bytes` (core không sở hữu byte của object layer).
//! - `key` do object layer định nghĩa (`(object_id << 16) | cap_index`); core **không** diễn
//!   giải — chỉ trả lại nguyên vẹn cho cầu nối (`vifast_plat_riscv64::mem_bridge`).

use crate::error::{KError, ObjId, ObjType};
use crate::machine::{BodyRef, Machine};
use crate::observer::CoreObserver;
use crate::store::KernelStore;

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    /// Object **đại diện** cho một object VM do object layer sở hữu — xem tài liệu đầu tệp.
    ///
    /// # Lỗi
    /// Loại không phải VM ⇒ panic (lỗi lập trình, như `add_kernel_object`); bảng object **hoặc**
    /// bảng cap đầy ⇒ [`KError::NotEnoughMemory`] (nguyên tử — không đổi trạng thái).
    pub fn add_foreign_object(&mut self, ty: ObjType, key: u64) -> Result<ObjId, KError> {
        assert!(
            matches!(ty, ObjType::Frame | ObjType::PageTable | ObjType::VSpace),
            "add_foreign_object: chỉ nhận Frame/PageTable/VSpace (D2-83)"
        );
        if self.object_count() >= self.objects.len() || self.cap_count() >= self.caps.len() {
            return Err(KError::NotEnoughMemory);
        }
        Ok(self.push_meta(ty, None, key as usize, 0, None, None, 0, 0, BodyRef::NONE))
    }

    /// Key của object đại diện (danh tính bên object layer) để tra ngược Frame/PA.
    pub fn foreign_key(&self, obj: ObjId) -> Result<u64, KError> {
        let m = self.meta(obj)?;
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        if m.untyped.is_some()
            || !matches!(m.ty, ObjType::Frame | ObjType::PageTable | ObjType::VSpace)
        {
            return Err(KError::WrongObjectType);
        }
        Ok(m.offset as u64)
    }

    /// `true` ⇔ còn **cap sống** trỏ object đại diện (caller phải `delete` hết trước khi thu hồi
    /// Frame — dùng để kiểm **trước** khi trả Frame, không để nửa vời).
    pub fn foreign_object_has_caps(&self, obj: ObjId) -> Result<bool, KError> {
        let m = self.meta(obj)?;
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        if m.untyped.is_some() {
            return Err(KError::WrongObjectType);
        }
        Ok(self.caps[..self.n_caps]
            .iter()
            .any(|c| c.alive() && c.obj == obj))
    }

    /// Đánh dấu object đại diện đã chết sau khi object layer đã thu hồi Frame.
    ///
    /// Từ chối khi **còn cap sống** trỏ object (caller phải `delete` cap trước — giữ bất biến
    /// `check_caps`: cap sống không trỏ object chết).
    pub fn kill_foreign_object(&mut self, obj: ObjId) -> Result<(), KError> {
        let m = self.meta(obj)?;
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        if m.untyped.is_some() {
            return Err(KError::WrongObjectType);
        }
        if self.caps[..self.n_caps]
            .iter()
            .any(|c| c.alive() && c.obj == obj)
        {
            return Err(KError::IllegalOperation);
        }
        self.objects[obj as usize - 1].alive = false;
        Ok(())
    }
}
