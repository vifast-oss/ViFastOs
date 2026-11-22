// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338a** — vùng RAM boot **ngoài ảnh** của aarch64: arena bảng trang + frame DMA động.
//!
//! Tách khỏi `main.rs` để giữ luật **R1** (tệp trong `kernel/` ≤ 1.500 dòng) — cùng lý do các
//! mô-đun `mapwin`/`blk`/`net`/`gpu`/`input` của KT-273..KT-305.
//!
//! # Bố cục
//!
//! ```text
//!   __kernel_end (căn 4 KiB) ──► arena bảng trang: BOOT_ARENA_FRAMES × 4 KiB
//!                                (bump allocator; 4 frame cuối cấp cho DMA lúc driver init)
//! ```
//!
//! Vùng này **không** thuộc section nào của ELF: trước KT-338a arena 44 frame + 4 trang DMA
//! `static` chiếm **196.608 B** trong `.bss`; sau KT-338a `.data+.bss` của ảnh còn **30.632 B**
//! (đo bằng `v2/test/boot/budget-aarch64.sh`, journal `kt338a-bss.md`) — trần I7 **không** nới.
//!
//! # Vì sao **ba** hàm tách rời
//!
//! `prepare` chạy khi **MMU tắt** (PA == VA); `map_boot_table` map identity vào bảng boot **trước**
//! khi bật MMU; `map_vspace` map identity vào `VSpace` **trước** `activate`. Cả ba đều bắt buộc:
//!
//! - thiếu bảng boot ⇒ ghi entry bảng trang (qua VA thấp của arena) fault ngay giữa
//!   `map_high`/`map_phys`, trước cả `high_entry`;
//! - thiếu `VSpace` ⇒ fault ở lần cấp bảng trang sau `activate` **và** ở mọi lần `DmaPool` đọc
//!   hay phát `dc civac` trên **PA** của trang DMA (identity mới làm `PA == VA` đúng).
//!
//! Đây **không** phải "map thêm cho chắc": mỗi mapping ứng với một đường truy cập thật, và phép
//! đo `identity=1` của `input::map_window` walk chính bảng trang `VSpace` để chứng minh.
//! `MapError` không bị nuốt — hàm in `PANIC` rồi `debug_halt` (R5: sai phải dừng, không im lặng).

use core::fmt::Write;

use vifast_plat_aarch64::bootmem;
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::mmu::TableMemory;
use vifast_plat_aarch64::split_mmu::SplitMmu;
use vifast_plat_aarch64::vspace::VSpace;
use vifast_plat_aarch64::{asm, boot_mmu, debug_halt};

use crate::Console;

/// Dựng vùng RAM boot ngay sau `kernel_end`: zero toàn vùng rồi công bố PA arena; trả **PA**.
///
/// Vì sao `__kernel_end`: linker đặt cả `.smp_stacks` (NOLOAD) **trong** `[__text_start,
/// __kernel_end)` nên đây là biên đầu tiên chắc chắn không còn ai dùng; QEMU `virt -m 128M` có
/// RAM phẳng `0x4000_0000..0x4800_0000` và nạp ảnh ở `0x4008_0000`, nên vùng 256 KiB này nằm trọn
/// trong RAM. `kernel_end` được căn lên 4 KiB (linker không hứa căn trang).
///
/// Zero **tường minh** vì `.bss` được `_start` zero hộ còn RAM sau ảnh thì không; `PageTable`
/// cũng zero từng frame khi cấp và `asm::dma_page_*` zero frame DMA lúc cấp, nhưng zero cả vùng ở
/// đây để bất biến "frame cấp lần đầu phải sạch" không phụ thuộc đường cấp nào.
pub fn prepare(kernel_end: u64) -> u64 {
    let ram_pa = (kernel_end + 0xFFF) & !0xFFF;
    asm::zero_boot_ram(ram_pa, bootmem::boot_ram_bytes());
    bootmem::publish_arena(ram_pa);
    ram_pa
}

/// Map **identity** vùng RAM boot vào bảng **nửa thấp** của bảng boot; lỗi ⇒ `PANIC` + dừng.
pub fn map_boot_table<M: TableMemory>(
    mmu: &mut SplitMmu<M>,
    console: &mut Console,
    ram_pa: u64,
) -> usize {
    match boot_mmu::map_identity(
        mmu.low_mut(),
        ram_pa,
        ram_pa + bootmem::BOOT_RAM_BYTES as u64,
        Perms::KERNEL_RW,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map vùng RAM boot lỗi {error:?}");
            debug_halt(3)
        }
    }
}

/// Map **identity** vùng RAM boot vào bảng **nửa thấp** của `VSpace` (trước `activate`); lỗi ⇒
/// `PANIC` + dừng.
pub fn map_vspace<M: TableMemory>(vs: &mut VSpace<M>, console: &mut Console, ram_pa: u64) -> usize {
    match crate::mapwin::map_identity_into(
        vs,
        ram_pa,
        ram_pa + bootmem::BOOT_RAM_BYTES as u64,
        Perms::KERNEL_RW,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map vùng RAM boot lỗi {error:?}");
            debug_halt(3)
        }
    }
}
