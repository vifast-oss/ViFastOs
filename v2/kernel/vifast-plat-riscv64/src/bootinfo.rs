// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-402** (D2-259) — đọc **DTB thật** ở địa chỉ vật lý QEMU/OpenSBI truyền (`a1`) và parse
//! thành [`BootInfo`] dùng chung.
//!
//! Phần **thuần** (đi cây FDT, kiểm biên, lỗi có tên) nằm ở
//! [`vifast_plat_common::bootinfo`]; tệp này chỉ là lớp **chạm bộ nhớ**: đọc 40 byte header ở
//! `dtb_pa`, kiểm magic/trần `totalsize` **trước khi** tạo slice, rồi gọi parser chung. Không có
//! bản sao thứ hai của logic FDT ở đây.
//!
//! # Nơi gọi
//!
//! Phải gọi **trước** `mmu::init()` (lúc đó paging tắt nên `dtb_pa` là PA đọc trực tiếp). Nếu
//! MMU đã bật, dùng `physmap` (`PHYSMAP_BASE + PA`) — cùng quy ước với [`crate::smp::dtb_magic_ok`].

use vifast_plat_common::bootinfo::{
    parse_fdt, BootArch, BootInfo, BootInfoError, FDT_MAGIC, MAX_FDT_BYTES,
};

/// Đọc DTB ở `dtb_pa` và trả [`BootInfo`] cho `riscv64`.
///
/// Lỗi **có tên** khi: con trỏ 0/lệch căn, magic sai, `totalsize` vượt trần/ngoài dữ liệu, hoặc
/// bất kỳ lỗi cấu trúc nào từ parser chung (thiếu `/memory`, PLIC, UART, ECAM…).
pub fn detect(dtb_pa: u64) -> Result<BootInfo, BootInfoError> {
    let pa = translate_pa(dtb_pa)?;
    let mut header = [0u8; 40];
    // SAFETY: `pa` trỏ tới DTB do OpenSBI/QEMU truyền trong `a1`; đọc 40 byte header (đúng cỡ
    // `struct fdt_header`) từ RAM vật lý. Không ghi, không giữ con trỏ sau khi trả.
    unsafe {
        core::ptr::copy_nonoverlapping(pa as *const u8, header.as_mut_ptr(), header.len());
    }
    let magic = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    if magic != FDT_MAGIC {
        return Err(BootInfoError::BadMagic {
            what: "dtb",
            got: magic,
        });
    }
    let totalsize = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if totalsize < header.len() || totalsize > MAX_FDT_BYTES {
        return Err(BootInfoError::Truncated {
            needed: totalsize,
            have: MAX_FDT_BYTES,
        });
    }
    // SAFETY: DTB nằm trong RAM vật lý QEMU; `totalsize` đã kiểm `>= 40` và `<= MAX_FDT_BYTES`
    // (trần 1 MiB) nên slice nằm trong vùng DTB hợp lệ. Parser chung kiểm tiếp mọi offset.
    let bytes = unsafe { core::slice::from_raw_parts(pa as *const u8, totalsize) };
    parse_fdt(bytes, BootArch::Riscv64)
}

/// PA/VA của DTB theo trạng thái MMU: chưa bật ⇒ PA đọc trực tiếp; đã bật ⇒ qua physmap.
fn translate_pa(dtb_pa: u64) -> Result<u64, BootInfoError> {
    if dtb_pa == 0 {
        return Err(BootInfoError::NullPointer);
    }
    if !dtb_pa.is_multiple_of(8) {
        return Err(BootInfoError::BadOffset { what: "dtb-pa" });
    }
    if crate::mmu::mmu_enabled() {
        crate::mmu::physmap_alias(dtb_pa).ok_or(BootInfoError::BadOffset {
            what: "dtb-physmap",
        })
    } else {
        Ok(dtb_pa)
    }
}

/// In `# bootinfo:` từ kết quả [`detect`] (hoặc dòng `fail-closed` **có tên** khi bảng hỏng).
///
/// Trả `true` ⇔ `BootInfo` hợp lệ. Trả `false` ⇒ **caller phải halt** (fail-closed, D2-259):
/// không có giá trị mặc định nào được dùng thay bảng hỏng.
pub fn write_log<W: core::fmt::Write>(info: &Result<BootInfo, BootInfoError>, w: &mut W) -> bool {
    match info {
        Ok(info) => info.write_log(w).is_ok(),
        Err(e) => {
            let _ =
                core::fmt::Write::write_fmt(w, format_args!("# bootinfo: fail-closed err={e}\n"));
            false
        }
    }
}
