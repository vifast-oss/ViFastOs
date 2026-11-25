// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-414b** — thư viện dùng chung cho boot stub UEFI aarch64.
//!
//! # Tái dùng, không chép
//!
//! Bốn mô-đun của boot stub UEFI **đã có** ở [`v2/boot/uefi-x86_64`](../../uefi-x86_64) (KT-414)
//! được `#[path]` include **nguyên văn** — cùng một nguồn, không bản sao:
//!
//! - [`efi`]: định nghĩa `repr(C)` tối thiểu của UEFI 2.x (`EFI_SYSTEM_TABLE`,
//!   `EFI_BOOT_SERVICES`, `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`, `EFI_MEMORY_DESCRIPTOR`,
//!   `EFI_CONFIGURATION_TABLE` + GUID ACPI/DTB) — `extern "efiapi"` là **cùng** ABI trên
//!   `aarch64` (AAPCS64), nên không có bản riêng cho arch;
//! - [`memmap`]: **kiểm** bảng nhớ **fail-closed có tên** + thống kê vùng usable + 8 ca tiêm lỗi;
//! - [`handoff`]: bản ghi bàn giao `BootInfo` v1 (`repr(C)`, `rsdp_pa`/`dtb_pa`, bất biến
//!   reserved/copy — D2-260), **tổng quát hoá arch** ở KT-414b (`ARCH_AARCH64` +
//!   `build_for`/`verify_for`; `build`/`verify` giữ nguyên nghĩa x86_64 cho KT-414);
//! - [`console`]: `ConOut->OutputString` trước `ExitBootServices` + `ConOutWriter`
//!   (`fmt::Write` → `ConOut`) để in `BootInfo::write_log` **nguyên văn** của kernel.
//!
//! `bootinfo` cũng **không** phải bản sao: `#[path]` trỏ thẳng vào nguồn sự thật của kernel
//! (`v2/kernel/vifast-plat-common/src/bootinfo.rs`, KT-402) và chỉ shim `crate::smp::MAX_HARTS`.
//! Nhờ vậy tên lỗi, tên trường và dòng log `# bootinfo:` của boot stub **là của kernel**, không
//! thể lệch bản sao — kể cả `BootSource::Uefi` (D2-260).
//!
//! # Phần **riêng** của aarch64
//!
//! - [`semihost`]: ARM semihosting (`SYS_WRITE0`/`SYS_EXIT_EXTENDED`) — sau `ExitBootServices`
//!   `ConOut` của firmware đã chết, nên bằng chứng "đã thoát thật" và **mã thoát QEMU** đi qua
//!   `hlt #0xF000` (QEMU `-semihosting-config`). Đây là chỗ **khác** x86_64 (cổng I/O
//!   `isa-debug-exit`/`isa-debugcon`), nên nó nằm ở crate này chứ không ở `console` dùng chung.
//! - [`dtb_header_ok`]: phép kiểm **thuần** header DTB mà firmware khai (`dtb_pa`), fail-closed.
//!
//! `#![cfg_attr(not(test), no_std)]`: ảnh `.efi` không có `std`; test host (`cargo test --lib`)
//! cần harness `std`.

#![cfg_attr(not(test), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]

mod smp {
    /// Shim cho `crate::smp::MAX_HARTS` mà `bootinfo.rs` dùng. **Phải bằng**
    /// `v2/kernel/vifast-plat-common/src/smp.rs:31` (= 8); test host kiểm chéo bằng
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

// Bốn mô-đun **dùng chung nguyên văn** với `v2/boot/uefi-x86_64` (KT-414) — `#[path]`, không chép.
#[path = "../../uefi-x86_64/src/console.rs"]
pub mod console;
#[path = "../../uefi-x86_64/src/efi.rs"]
pub mod efi;
#[path = "../../uefi-x86_64/src/handoff.rs"]
pub mod handoff;
#[path = "../../uefi-x86_64/src/memmap.rs"]
pub mod memmap;

pub mod semihost;

/// Dựng **`BootInfo` v1 nguồn `Uefi`** (D2-260) từ bảng nhớ đã kiểm — dùng chung cho ảnh `.efi`
/// và test host (một nguồn, không chép logic).
///
/// `BootSource::Uefi` chỉ đòi `n_ram > 0` + mọi vùng RAM hợp lệ (`validate()` fail-closed theo
/// nguồn) — ACPI/DTB chưa parse ở bước stub nên `gic`/`uart`/`pcie_ecam` để `None` là **đúng**,
/// không phải khai man. `n_harts = 1` vì stub chạy trên BSP; SMP đọc DTB/ACPI sau.
pub fn bootinfo_from_memmap(
    st: &memmap::MemMapStats,
) -> Result<bootinfo::BootInfo, bootinfo::BootInfoError> {
    let mut info = bootinfo::BootInfo::new(bootinfo::BootArch::Aarch64, bootinfo::BootSource::Uefi);
    info.n_harts = 1;
    info.hart_ids[0] = 0;
    info.boot_hart = 0;
    for r in st.usable_ram() {
        info.push_ram(r.base, r.size)?;
    }
    info.validate()?;
    Ok(info)
}

/// Phần **thuần** của phép kiểm header DTB mà firmware khai (`dtb_pa`): magic `0xd00dfeed`
/// (big-endian ở offset 0) + `totalsize` trong `[40, MAX_FDT_BYTES]`.
///
/// `dtb_pa == 0` (firmware **không** khai DTB) **không** phải lỗi — AAVMF đo được `dtb_pa=0` vì
/// nó cấp ACPI; firmware cấp FDT thì header **phải** hợp lệ, sai ⇒ `bad-dtb` (fail-closed).
/// Hàm thuần (không `unsafe`) nên test host khoá được; phần đọc PA nằm ở `main.rs`.
pub fn dtb_header_ok(magic: u32, totalsize: u32) -> Result<(), &'static str> {
    if magic != bootinfo::FDT_MAGIC {
        return Err("bad-dtb");
    }
    // Header FDT là 40 byte (devicetree spec §5.2); trần trên chặn `totalsize` rác.
    if totalsize < 40 || totalsize as usize > bootinfo::MAX_FDT_BYTES {
        return Err("bad-dtb");
    }
    Ok(())
}
