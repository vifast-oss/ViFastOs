// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-414** — thư viện dùng chung cho boot stub UEFI x86_64.
//!
//! Ba phần, mỗi phần một tệp:
//!
//! - [`efi`]: định nghĩa `repr(C)` tối thiểu của UEFI 2.x (`EFI_SYSTEM_TABLE`,
//!   `EFI_BOOT_SERVICES`, `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`, `EFI_MEMORY_DESCRIPTOR`) — chỉ phần
//!   đụng tới, không kéo crate ngoài (D2-21);
//! - [`memmap`]: **kiểm** bảng nhớ **fail-closed có tên** + thống kê vùng usable;
//! - [`handoff`]: bản ghi bàn giao `BootInfo` v1 (nháp, xem README §5).
//!
//! `bootinfo` **không** phải bản sao: `#[path]` trỏ thẳng vào nguồn sự thật của kernel
//! (`v2/kernel/vifast-plat-common/src/bootinfo.rs`, KT-402) và chỉ shim
//! `crate::smp::MAX_HARTS`. Nhờ vậy tên lỗi, tên trường và dòng log `# bootinfo:` của boot stub
//! **là của kernel**, không thể lệch bản sao.
//!
//! `#![cfg_attr(not(test), no_std)]`: ảnh `.efi` không có `std`; test host (`cargo test --lib`)
//! cần harness `std`.

#![cfg_attr(not(test), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]

mod smp {
    /// Shim cho `crate::smp::MAX_HARTS` mà `bootinfo.rs` dùng. **Phải bằng**
    /// `v2/kernel/vifast-plat-common/src/smp.rs:31` (= 8); `tests/canonical.rs` kiểm chéo bằng
    /// `include_str!` nên lệch là đỏ ngay.
    pub const MAX_HARTS: usize = 8;
}

// `-D warnings` của `v2 t0` áp lên thân `bootinfo.rs` **của kernel** (ta include, không sửa):
// hai lint dưới đây là của bản kernel hiện tại, ngoài quyền task này (0 chạm kernel/plat-*).
// File gốc vẫn sạch dưới clippy của workspace kernel; ở đây chỉ cần không biến chúng thành đỏ
// giả cho boot stub.
#[allow(clippy::manual_is_multiple_of, clippy::needless_range_loop)]
#[path = "../../../kernel/vifast-plat-common/src/bootinfo.rs"]
pub mod bootinfo;

pub mod console;
pub mod efi;
pub mod handoff;
pub mod memmap;
