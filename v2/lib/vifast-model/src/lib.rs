// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-model` — mô hình Untyped/CNode **tái sử dụng** của dòng OS (KT-11).
//!
//! Crate này không phải test-only: sim dùng làm mô hình tham chiếu để so vi sai
//! (`tests/kt11_diff_sim.rs`, 10.000 seed) và kernel K1a dùng lại phần CSpace/Untyped.
//! Phạm vi có chủ ý: **không** MDB đầy đủ 48 B của production, **không** Frame/VSpace/IPC
//! (K2/K1b) — chỉ Untyped (watermark) + CNode (Copy/Mint/Move/Delete/Revoke) + cây dẫn xuất.
//!
//! Nguồn: `spec/abi.toml`, thiết kế §4.2/§4.3, D2-11/D2-35; quy ước F3 (review KT-10) ghi ở
//! [`Machine`]. Mã lỗi lấy từ [`vifast_abi`] nên không chép tay bảng ABI.
//!
//! ```ignore
//! use vifast_model::{Machine, ObjType, SlotRef};
//! let mut m = Machine::new();
//! let c = m.add_cnode(8);
//! let u = m.add_untyped(0x1000, 4096);
//! let cap = m.root_cap(u);
//! m.put_cap(c, 0, cap).unwrap();
//! let tcb = m.retype(SlotRef::new(c, 0), ObjType::Tcb, 0, 0).unwrap();
//! m.put_cap(c, 1, tcb).unwrap();
//! m.check_bytes(u).unwrap();
//! m.check_caps().unwrap();
//! ```

#![forbid(unsafe_code)]

mod cap;
mod error;
mod machine;
mod object;

pub use cap::{
    CapId, CapInfo, ObjId, SlotRef, RIGHTS_ALL, RIGHT_GRANT, RIGHT_GRANT_REPLY, RIGHT_READ,
    RIGHT_WRITE,
};
pub use error::{KError, CORE_OBJECTS, ERRORS};
pub use machine::Machine;
pub use object::{
    ObjType, CNODE_BASE_BYTES, DEFAULT_ALIGN, FLAG_GUEST, SLOT_BYTES, UNTYPED_MIN_BYTES,
};
pub use vifast_abi::RESERVED_OBJECTS;
