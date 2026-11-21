// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Hai hàm map cửa sổ vào `VSpace` của ảnh aarch64** — KT-273/275/279/283.
//!
//! Vì sao tách tệp: `main.rs` đã sát trần **R1** (1.500 dòng/tệp trong `kernel/`), và hai hàm
//! này là **thuần** (không trạng thái, không `unsafe`) nên tách được mà không đổi ngữ nghĩa.
//! Chúng dùng chung cho ECAM (VA riêng, PA không identity được), cửa sổ BAR của bốn loại thiết bị
//! virtio, framebuffer GPU, và UART/GIC/GICR.
//!
//! Cả hai đều **fail-closed**: căn lề sai ⇒ [`MapError::Unaligned`], không map lệch trang; lỗi
//! bảng trang được trả nguyên cho người gọi (không nuốt).

use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::mmu::{MapError, TableMemory};
use vifast_plat_aarch64::vspace::{Frame, VSpace};

/// Map cửa sổ `[pa_base, pa_base+bytes)` vào `[va_base, va_base+bytes)` bằng trang 4 KiB —
/// KT-273 dùng cho ECAM (VA riêng, PA không identity được) và cửa sổ BAR (VA == PA).
///
/// `bytes` phải là bội của 4 KiB và `pa_base`/`va_base` phải căn 4 KiB; lệch ⇒ `Unaligned`
/// (không map lệch trang).
pub fn map_window_into<M: TableMemory>(
    vs: &mut VSpace<M>,
    va_base: u64,
    pa_base: u64,
    bytes: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    if bytes == 0 || bytes & 0xFFF != 0 || va_base & 0xFFF != 0 || pa_base & 0xFFF != 0 {
        return Err(MapError::Unaligned);
    }
    let pages = bytes / 0x1000;
    let mut index = 0;
    while index < pages {
        let frame = Frame::new(pa_base + index * 0x1000).ok_or(MapError::Unaligned)?;
        vs.map_obj_with(va_base + index * 0x1000, frame, perms, attr)?;
        index += 1;
    }
    Ok(pages as usize)
}

/// Map **identity** `[start, end)` vào `VSpace` bằng trang 4 KiB (làm tròn ra ngoài) — dùng cho
/// UART/GIC trong VSpace của ảnh (kernel vẫn cần chúng sau khi `TTBR0` đổi).
pub fn map_identity_into<M: TableMemory>(
    vs: &mut VSpace<M>,
    start: u64,
    end: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    let first = start & !0xFFF;
    let last = (end + 0xFFF) & !0xFFF;
    let mut pages = 0;
    let mut pa = first;
    while pa < last {
        let frame = Frame::new(pa).ok_or(MapError::Unaligned)?;
        vs.map_obj_with(pa, frame, perms, attr)?;
        pages += 1;
        pa += 0x1000;
    }
    Ok(pages)
}
