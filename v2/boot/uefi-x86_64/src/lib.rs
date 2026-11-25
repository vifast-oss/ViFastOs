// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-414** — thư viện dùng chung cho boot stub UEFI x86_64.
//!
//! Ba phần, mỗi phần một tệp:
//!
//! - [`efi`]: định nghĩa `repr(C)` tối thiểu của UEFI 2.x (`EFI_SYSTEM_TABLE`,
//!   `EFI_BOOT_SERVICES`, `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`, `EFI_MEMORY_DESCRIPTOR`,
//!   `EFI_CONFIGURATION_TABLE` + GUID ACPI/DTB) — chỉ phần đụng tới, không kéo crate ngoài
//!   (D2-21);
//! - [`memmap`]: **kiểm** bảng nhớ **fail-closed có tên** + thống kê vùng usable;
//! - [`handoff`]: bản ghi bàn giao `BootInfo` v1 (`repr(C)`, `rsdp_pa`/`dtb_pa`, bất biến
//!   reserved/copy trang bàn giao — D2-260).
//!
//! `bootinfo` **không** phải bản sao: `#[path]` trỏ thẳng vào nguồn sự thật của kernel
//! (`v2/kernel/vifast-plat-common/src/bootinfo.rs`, KT-402) và chỉ shim
//! `crate::smp::MAX_HARTS`. Nhờ vậy tên lỗi, tên trường và dòng log `# bootinfo:` của boot stub
//! **là của kernel**, không thể lệch bản sao — kể cả `BootSource::Uefi` (D2-260).
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

/// Dựng **`BootInfo` v1 nguồn `Uefi`** (D2-260) từ bảng nhớ đã kiểm — dùng chung cho ảnh `.efi`
/// và test host (một nguồn, không chép logic).
///
/// `BootSource::Uefi` chỉ đòi `n_ram > 0` + mọi vùng RAM hợp lệ (`validate()` fail-closed theo
/// nguồn) — ACPI/DTB chưa parse ở bước stub nên `apic`/`uart`/`pcie_ecam` để `None` là **đúng**,
/// không phải khai man. `n_harts = 1` vì stub chạy trên BSP; SMP đọc ACPI sau.
pub fn bootinfo_from_memmap(
    st: &memmap::MemMapStats,
) -> Result<bootinfo::BootInfo, bootinfo::BootInfoError> {
    let mut info = bootinfo::BootInfo::new(bootinfo::BootArch::X86_64, bootinfo::BootSource::Uefi);
    info.n_harts = 1;
    info.hart_ids[0] = 0;
    info.boot_hart = 0;
    for r in st.usable_ram() {
        info.push_ram(r.base, r.size)?;
    }
    info.validate()?;
    Ok(info)
}
