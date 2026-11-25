// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-402** (D2-259) — đọc **DTB thật** mà QEMU `virt` aarch64 truyền trong `x0` và parse
//! thành [`BootInfo`] dùng chung.
//!
//! Phần **thuần** (đi cây FDT, kiểm biên, lỗi có tên) nằm ở
//! [`vifast_plat_common::bootinfo`]; tệp này chỉ là lớp **chạm bộ nhớ** (đọc header ở PA, kiểm
//! magic/trần trước khi tạo slice). Không có bản sao thứ hai của logic FDT.
//!
//! # Vì sao `dtb` có thể bằng 0 (và ta vẫn tìm được DTB thật)
//!
//! QEMU `virt` chỉ ghi **bootloader** (đặt `x0 = dtb`) khi boot ảnh Linux `Image`; với ảnh **ELF**
//! (`hw/arm/boot.c: arm_setup_direct_kernel_boot`), `is_linux = 0` nên **không** có bootloader và
//! `x0 = 0`. Nhưng QEMU vẫn sinh DTB (`virt.c: get_dtb = machvirt_dtb`) và đặt nó ở
//! **`loader_start`** (base RAM, `VIRT_MEM.base` = `0x4000_0000`) khi còn chỗ trước ảnh.
//!
//! Vì vậy [`detect`] xử lý cả hai đường: `dtb != 0` (firmware/bootloader tương lai) → dùng thẳng;
//! `dtb == 0` → **quét có biên** `[DTB_BASE_PA, __text_start)` tìm magic FDT thật (không đoán
//! địa chỉ con trỏ, không rơi về hằng số GIC/UART). Không tìm thấy ⇒ lỗi có tên (fail-closed).
//!
//! # Nơi gọi
//!
//! Gọi ở đầu ảnh `kmain(dtb)` — **trước** `boot_mmu`/`mmu_bringup`, lúc paging còn tắt nên mọi
//! địa chỉ là PA đọc trực tiếp.

#![allow(unsafe_code)] // D2-17: crate `kernel/vifast-plat-*`; chỉ phần đọc PA cần `unsafe`.

use vifast_plat_common::bootinfo::{
    parse_fdt, BootArch, BootInfo, BootInfoError, FDT_MAGIC, MAX_FDT_BYTES,
};

/// Base RAM của `qemu-system-aarch64 -M virt` (`virt.c: memmap[VIRT_MEM].base`), đồng thời là
/// `loader_start` — nơi QEMU đặt DTB khi boot ảnh **ELF** (xem đầu tệp). Đây là **hợp đồng đường
/// boot** (cùng loại với `BOOT_LOAD_ADDR` của x86), không phải địa chỉ thiết bị cần driver.
pub const DTB_BASE_PA: u64 = 0x4000_0000;

/// Trần cửa sổ quét DTB (16 MiB) — chặn quét vô hạn nếu layout ảnh đổi bất thường.
pub const DTB_SCAN_LIMIT: u64 = 16 << 20;

extern "C" {
    /// Đầu ảnh kernel (linker.ld) — mốc trên của cửa sổ quét DTB.
    static __text_start: u8;
}

/// Đọc DTB ở `dtb_pa` và trả [`BootInfo`] cho `aarch64`.
///
/// Lỗi **có tên** khi: con trỏ 0/lệch căn, magic sai, `totalsize` vượt trần/ngoài dữ liệu, hoặc
/// bất kỳ lỗi cấu trúc nào từ parser chung (thiếu `/memory`, GIC, UART, ECAM…).
pub fn detect(dtb_pa: u64) -> Result<BootInfo, BootInfoError> {
    let dtb_pa = locate_dtb(dtb_pa)?;
    if !dtb_pa.is_multiple_of(8) {
        return Err(BootInfoError::BadOffset { what: "dtb-pa" });
    }
    let mut header = [0u8; 40];
    // SAFETY: `dtb_pa` do QEMU truyền trong `x0`; đọc 40 byte header FDT từ RAM vật lý (paging
    // tắt tại điểm gọi). Không ghi.
    unsafe {
        core::ptr::copy_nonoverlapping(dtb_pa as *const u8, header.as_mut_ptr(), header.len());
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
    // SAFETY: `totalsize` đã kiểm `>= 40` và `<= MAX_FDT_BYTES`; DTB nằm trong RAM vật lý.
    // Parser chung kiểm tiếp mọi offset/khối trước khi đọc.
    let bytes = unsafe { core::slice::from_raw_parts(dtb_pa as *const u8, totalsize) };
    parse_fdt(bytes, BootArch::Aarch64)
}

/// Xác định PA của DTB: dùng thẳng con trỏ firmware nếu có; nếu `x0 == 0` (QEMU boot ELF) thì
/// quét **có biên** `[DTB_BASE_PA, __text_start)` tìm magic FDT thật.
fn locate_dtb(dtb_pa: u64) -> Result<u64, BootInfoError> {
    if dtb_pa != 0 {
        return Ok(dtb_pa);
    }
    // SAFETY: đọc **địa chỉ** symbol linker (`__text_start`), không dereference. Tại điểm gọi
    // paging tắt nên giá trị là PA liên kết (0x4008_0000 theo linker.ld).
    let text_start = core::ptr::addr_of!(__text_start) as u64;
    if text_start <= DTB_BASE_PA || text_start - DTB_BASE_PA > DTB_SCAN_LIMIT {
        return Err(BootInfoError::MissingTable("dtb"));
    }
    let mut pa = DTB_BASE_PA;
    while pa + 8 <= text_start {
        // SAFETY: `pa` nằm trong `[DTB_BASE_PA, __text_start)` — vùng RAM thấp QEMU đã cấp cho
        // máy (base RAM) và chưa bị ảnh chiếm; chỉ đọc từng u32.
        let word = unsafe { core::ptr::read_volatile(pa as *const u32) };
        if word == FDT_MAGIC.swap_bytes() {
            return Ok(pa);
        }
        pa += 4;
    }
    Err(BootInfoError::MissingTable("dtb"))
}

/// In `# bootinfo:` từ kết quả [`detect`] (hoặc dòng `fail-closed` **có tên** khi bảng hỏng).
///
/// Trả `true` ⇔ `BootInfo` hợp lệ. Trả `false` ⇒ **caller phải halt** (fail-closed, D2-259):
/// không có giá trị mặc định nào được dùng thay bảng hỏng.
pub fn write_log<W: core::fmt::Write>(info: &Result<BootInfo, BootInfoError>, w: &mut W) -> bool {
    match info {
        Ok(info) => info.write_log(w).is_ok(),
        // QEMU `virt` boot ảnh **ELF** không truyền DTB và **không cài** DTB (xem đầu tệp:
        // `fdt_size` 1 MiB > khoảng trống trước ảnh nên `arm_load_dtb` trả 0). "Không có bảng"
        // khác "bảng hỏng": ghi rõ `absent` rồi để đường boot cũ chạy tiếp — **không** im lặng
        // và **không** bịa giá trị. Bảng **có** mà hỏng thì vẫn fail-closed (nhánh dưới).
        Err(BootInfoError::MissingTable("dtb")) => {
            let _ = core::fmt::Write::write_fmt(
                w,
                format_args!(
                    "# bootinfo: absent src=dtb (QEMU ELF direct boot không cài DTB; xem bootinfo.rs)\n"
                ),
            );
            true
        }
        Err(e) => {
            let _ =
                core::fmt::Write::write_fmt(w, format_args!("# bootinfo: fail-closed err={e}\n"));
            false
        }
    }
}
