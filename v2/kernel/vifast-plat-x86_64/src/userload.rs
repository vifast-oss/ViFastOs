// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-341** — nạp ELF U-mode cho x86_64 (khuôn `vifast-plat-aarch64::userload`, KT-339).
//!
//! # Vì sao cần
//!
//! `KT-340c` chỉ chạy chương trình ring 3 dựng bằng `global_asm!` (một trang `.user`). Shell dùng
//! chung `vifast-plat-user/programs/shell.rs` là chương trình Rust **thật**, nhiều segment
//! (`.text`/`.rodata`/`.data`/`.bss`) — phải nạp **ELF64** do `build.rs` biên dịch cho
//! `x86_64-unknown-none`, đúng khuôn `vifast-elf::{parse, LoadSegment}` mà riscv64/aarch64 dùng
//! (D2-26: **không** có parser ELF thứ hai, **không** đọc đĩa — ảnh nhúng `include_bytes!`).
//!
//! # Bất biến
//!
//! - **W^X**: `PF_X` ⇒ [`Perms::USER_RX`], `PF_W` ⇒ [`Perms::USER_RW`], còn lại
//!   [`Perms::USER_RO`]; segment `W+X` bị `vifast-elf::parse` từ chối trước khi tới đây.
//! - **Zero trước khi copy**: frame lấy từ arena boot không được `.bss` zero hộ, nên mỗi trang
//!   được zero tường minh rồi mới copy `filesz`; phần `.bss` (memsz > filesz) vì thế đúng là 0.
//! - **Fail-closed**: thiếu frame / parse lỗi / map lỗi ⇒ `Err(LoadError)`; ảnh in `PANIC` rồi
//!   dừng — không chạy tiếp với chương trình nửa nạp.
//! - **Đăng ký dải user**: `SyscallEnv::put_str` của x86_64 chỉ đọc trong dải **đã đăng ký**
//!   (fail-closed); loader đăng ký `[base, end)` của ảnh đã map, stack và ring. Không mở cả
//!   không gian địa chỉ.
//!
//! `unsafe` **không** có ở đây: mọi phép đọc/ghi đi qua [`crate::asm`] (module duy nhất của crate
//! được `allow(unsafe_code)`, D2-17).

use crate::asm;
use crate::bootmem::{BootArena, FRAME_BYTES};
use crate::format::Perms;
use crate::mmu::TableMemory;
use crate::syscall_env;
use crate::vspace::VSpace;
use vifast_elf::ElfError;

/// Cửa sổ VA dành cho chương trình U-mode nạp từ ELF.
///
/// `0x7000_0000` nằm dưới `VSPACE_BASE` (64 GiB) và trong nửa thấp 39-bit canonical; identity map
/// của ảnh không phủ vùng này (nên kernel không vô tình đọc/ghi cùng PA), MMIO/ECAM cũng không
/// giao. Ba ảnh chương trình được link cách nhau 2 MiB (`shell` 0x7000_0000, `procsrv`
/// 0x7020_0000) để một segment mọc thêm không đè ảnh kế.
pub const USER_WINDOW_BASE: u64 = 0x7000_0000;
/// Đỉnh cửa sổ — dưới mọi cửa sổ MMIO của ảnh.
pub const USER_WINDOW_END: u64 = 0x7800_0000;

/// VA ring KT-69 mà shell dùng chung mọi arch (`shell.rs::VA_CON_RING`).
pub const RING_VA: u64 = 0x4300_0000;

/// Lỗi nạp — ảnh in tên ổn định, **không** đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// `vifast-elf::parse` từ chối (mã gốc giữ trong [`LoadError::Elf`]).
    Elf(ElfError),
    /// Arena boot hết frame (không nới trần `.data+.bss`; arena nằm ngoài ảnh).
    NoFrame,
    /// `VSpace::map` từ chối (trùng VA, quyền, canonical, …).
    Map,
    /// Bảng dải user đầy (nới `MAX_USER_RANGES` nếu thật sự cần).
    TooManyRanges,
}

impl LoadError {
    /// Tên ổn định để in log (`# k5 fail: stage=…`).
    pub const fn name(self) -> &'static str {
        match self {
            LoadError::Elf(_) => "elf",
            LoadError::NoFrame => "no-frame",
            LoadError::Map => "map",
            LoadError::TooManyRanges => "ranges",
        }
    }

    /// Mã `ElfError` khi lỗi đến từ parser (0 với lỗi khác) — bằng chứng máy kiểm được.
    pub const fn elf_code(self) -> u8 {
        match self {
            LoadError::Elf(error) => elf_error_code(error),
            _ => 0,
        }
    }
}

/// Mã số ổn định của [`ElfError`] (in vào log; không phụ thuộc `Debug` của crate khác).
const fn elf_error_code(error: ElfError) -> u8 {
    match error {
        ElfError::TooSmall => 1,
        ElfError::BadMagic => 2,
        ElfError::Not64Bit => 3,
        ElfError::NotLittleEndian => 4,
        ElfError::BadIdentVersion => 5,
        ElfError::NotExecutable => 6,
        ElfError::BadVersion => 7,
        ElfError::BadHeaderSize => 8,
        ElfError::BadProgramHeaderSize => 9,
        ElfError::TooManySegments => 10,
        ElfError::HeaderOutOfBounds => 11,
        ElfError::BadProgramHeader => 12,
        ElfError::BadAlignment => 13,
        ElfError::Misaligned => 14,
        ElfError::FileRange => 15,
        ElfError::MemorySmallerThanFile => 16,
        ElfError::Overflow => 17,
        ElfError::OutsideUserRange => 18,
        ElfError::BadUserRange => 19,
        ElfError::WrongMachine => 20,
        ElfError::UnsupportedSegment => 21,
        ElfError::NotReadable => 22,
        ElfError::WriteAndExecute => 23,
        ElfError::Overlap => 24,
        ElfError::NoSegments => 25,
        ElfError::BadEntry => 26,
    }
}

/// Ảnh đã nạp — số đo **thật** để TAP/báo cáo in, không phải lời khai của chương trình.
#[derive(Clone, Copy, Debug)]
pub struct Loaded {
    /// Điểm vào (VA tuyệt đối, `ET_EXEC`).
    pub entry: u64,
    /// VA thấp nhất của segment (đã căn trang).
    pub base: u64,
    /// VA cao nhất (đã căn trang) — `end - base` = cửa sổ ảnh.
    pub end: u64,
    /// Số `PT_LOAD`.
    pub segments: usize,
    /// Số frame 4 KiB đã cấp + map.
    pub pages: usize,
}

/// Quyền của một segment theo cờ `vifast-elf` (W^X: không bao giờ `W|X`).
const fn perms_for(exec: bool, write: bool) -> Perms {
    if exec {
        Perms::USER_RX
    } else if write {
        Perms::USER_RW
    } else {
        Perms::USER_RO
    }
}

/// Zero một frame 4 KiB qua `asm::write_u64` (arena NOLOAD **không** được `.bss` zero hộ).
fn zero_frame(pa: u64) {
    let mut offset = 0u64;
    while offset < FRAME_BYTES {
        asm::write_u64(pa + offset, 0);
        offset += 8;
    }
}

/// Nạp **một** ELF64 `ET_EXEC` vào `vs` theo đúng VA liên kết của nó.
///
/// `bytes` là toàn bộ tệp (ảnh nhúng bằng `include_bytes!`); `machine` là `e_machine` mà kiến trúc
/// chấp nhận (x86-64 = 62) — hàm **không** tự đoán kiến trúc.
pub fn load_elf<M: TableMemory>(
    vs: &mut VSpace<M>,
    arena: &mut BootArena,
    bytes: &[u8],
    machine: u16,
) -> Result<Loaded, LoadError> {
    let image = vifast_elf::parse(bytes, machine, USER_WINDOW_BASE, USER_WINDOW_END)
        .map_err(LoadError::Elf)?;
    let mut pages = 0usize;
    let mut base = u64::MAX;
    let mut end = 0u64;
    for segment in image.segments() {
        let seg_start = segment.vaddr & !(FRAME_BYTES - 1);
        let seg_end = segment.memory_end().ok_or(LoadError::NoFrame)?;
        if seg_start < base {
            base = seg_start;
        }
        if seg_end > end {
            end = seg_end;
        }
        let file_start = segment.vaddr;
        let file_end = segment.vaddr.saturating_add(segment.file_size as u64);
        let mut va = seg_start;
        while va < seg_end {
            let Some(pa) = arena.alloc_frame() else {
                return Err(LoadError::NoFrame);
            };
            zero_frame(pa);
            // Copy phần giao giữa [filesz) và trang này — `.bss` ở lại 0.
            let copy_start = va.max(file_start);
            let copy_end = (va + FRAME_BYTES).min(file_end);
            if copy_end > copy_start {
                let mut offset = 0u64;
                while copy_start + offset < copy_end {
                    let file_index =
                        segment.file_offset as u64 + (copy_start - file_start) + offset;
                    let Some(byte) = bytes.get(file_index as usize) else {
                        return Err(LoadError::Elf(ElfError::FileRange));
                    };
                    asm::write_u8(pa + (copy_start - va) + offset, *byte);
                    offset += 1;
                }
            }
            let perms = perms_for(segment.executable(), segment.writable());
            vs.map(va, pa, perms).map_err(|_| LoadError::Map)?;
            pages += 1;
            va += FRAME_BYTES;
        }
    }
    if pages == 0 || base == u64::MAX || end <= base {
        return Err(LoadError::Elf(ElfError::NoSegments));
    }
    if !syscall_env::register_user_range(base, end - base) {
        return Err(LoadError::TooManyRanges);
    }
    Ok(Loaded {
        entry: image.entry,
        base,
        end,
        segments: image.segment_count(),
        pages,
    })
}

/// Cấp + map **stack user** `pages` trang, kết thúc tại `top` (mọc xuống).
///
/// Trả `(base, bytes)` và đăng ký dải để `DebugPutStr` chấp nhận chuỗi trên stack.
pub fn map_user_stack<M: TableMemory>(
    vs: &mut VSpace<M>,
    arena: &mut BootArena,
    top: u64,
    pages: usize,
) -> Result<(u64, u64), LoadError> {
    let bytes = (pages as u64) * FRAME_BYTES;
    let base = top.checked_sub(bytes).ok_or(LoadError::NoFrame)?;
    let mut index = 0usize;
    while index < pages {
        let va = base + (index as u64) * FRAME_BYTES;
        let Some(pa) = arena.alloc_frame() else {
            return Err(LoadError::NoFrame);
        };
        zero_frame(pa);
        vs.map(va, pa, Perms::USER_RW).map_err(|_| LoadError::Map)?;
        index += 1;
    }
    if !syscall_env::register_user_range(base, bytes) {
        return Err(LoadError::TooManyRanges);
    }
    Ok((base, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quyền theo segment **không** bao giờ sinh W|X.
    #[test]
    fn quyen_wx() {
        assert_eq!(perms_for(true, false), Perms::USER_RX);
        assert_eq!(perms_for(false, true), Perms::USER_RW);
        assert_eq!(perms_for(false, false), Perms::USER_RO);
        let wx = perms_for(true, true);
        assert!(wx.exec && !wx.write, "exec thắng — không có W|X");
    }
}
