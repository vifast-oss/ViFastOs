// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Nạp ELF U-mode cho aarch64** (KT-339) — phần `unsafe` duy nhất của đường nạp nằm ở
//! [`crate::asm`] (D2-17: `kernel/vifast-plat-*` là danh sách đóng), tệp này chỉ **quyết định**
//! (`vifast-elf::parse`) và gọi API an toàn của [`crate::vspace::VSpace`].
//!
//! # Vì sao cần (và vì sao không chép loader riscv64)
//!
//! `KT-338b` chỉ map **một** trang mã EL0 dựng bằng `global_asm!` — đủ cho TAP 16/17/18 và pha
//! core, **không** đủ cho chương trình Rust thật (`procsrv`/`shell`: nhiều segment, `.rodata`,
//! `.bss`, stack). `KT-339` nạp **ELF64 thật** do `vifast-plat-user/programs/` sinh ra cho
//! `aarch64-unknown-none`, với đúng khuôn `vifast-elf::{parse, LoadSegment}` mà riscv64 dùng
//! (D2-26) — **không** có parser ELF thứ hai, **không** đọc đĩa: ảnh kiểm `include_bytes!`.
//!
//! # Bất biến
//!
//! - **W^X**: segment `PF_X` ⇒ `USER_RX`, `PF_W` ⇒ `USER_RW`, còn lại ⇒ `USER_RO`; segment
//!   `W+X` bị `vifast-elf::parse` từ chối (`ElfError::WriteAndExecute`) trước khi tới đây.
//! - **Zero trước khi copy**: frame lấy từ arena boot **không** được `_start` zero hộ (arena nằm
//!   ngoài ảnh — KT-338a), nên mỗi trang được `zero_boot_ram` rồi mới copy phần `filesz`; phần
//!   `.bss` (memsz > filesz) vì vậy đúng là **0**.
//! - **Fail-closed**: thiếu frame / parse lỗi / map lỗi ⇒ `Err(LoadError)`; ảnh in `PANIC` và
//!   dừng, **không** chạy tiếp với chương trình nửa nạp.
//! - **Đăng ký dải user**: `SyscallEnv::put_str` (và các hook đọc bộ nhớ user khác) chỉ đọc trong
//!   dải **đã đăng ký**; loader đăng ký `[base, end)` của ảnh đã map để chuỗi `.rodata` in được,
//!   nhưng vẫn **không** mở cả không gian địa chỉ (giữ tinh thần fail-closed của KT-338b).

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::mmu::TableMemory;
use vifast_elf::ElfError;

use crate::asm;
use crate::bootmem;
use crate::format::{MemAttr, Perms};
use crate::split_mmu::PHYSMAP_BASE;
use crate::vspace::{Frame, VSpace};

/// Cửa sổ VA **nửa thấp** dành cho chương trình U-mode nạp từ ELF.
///
/// `0x7000_0000` là [`crate::umode::EL0_CODE_VA`] (pha 1 + pha core của KT-338b); các ảnh K5
/// được link ở `0x7100_0000`/`0x7120_0000`/`0x7140_0000` — cùng cửa sổ, không đè trang nào của
/// pha trước.
pub const USER_WINDOW_BASE: u64 = 0x7000_0000;
/// Đỉnh cửa sổ — dưới mọi cửa sổ MMIO của ảnh (`0x0800_0000`/`0x8000_0000`… không giao).
pub const USER_WINDOW_END: u64 = 0x7800_0000;

/// Số dải user tối đa được đăng ký (3 ảnh ELF + stack + ring + dự phòng).
pub const MAX_USER_RANGES: usize = 8;

/// Dải `[base, base+len)` đã map cho U-mode — nguồn sự thật cho hook đọc bộ nhớ user.
static RANGE_BASE: [AtomicU64; MAX_USER_RANGES] = [const { AtomicU64::new(0) }; MAX_USER_RANGES];
static RANGE_LEN: [AtomicU64; MAX_USER_RANGES] = [const { AtomicU64::new(0) }; MAX_USER_RANGES];
/// Số dải đã đăng ký (cấp phát tuyến tính, chỉ hart 0 lúc nạp).
static RANGE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Lỗi nạp — ảnh in tên, **không** đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// `vifast-elf::parse` từ chối (mã gốc giữ trong [`LoadError::elf`]).
    Elf(ElfError),
    /// Arena boot hết frame (không nới trần `.data+.bss`; arena nằm ngoài ảnh).
    NoFrame,
    /// PA của frame không căn 4 KiB (không thể xảy ra với `bootmem`; kiểm để fail-closed).
    BadFrame,
    /// `VSpace::map_obj_with` từ chối (trùng VA, quyền, …).
    Map,
    /// Hết ô đăng ký dải (nới [`MAX_USER_RANGES`] nếu thật sự cần).
    TooManyRanges,
    /// `vifast-ring` từ chối hình học slot (cấu hình sai của kernel, không phải lỗi user).
    Ring,
}

impl LoadError {
    /// Tên ổn định để in log (`# k5 fail: stage=…`).
    pub const fn name(self) -> &'static str {
        match self {
            LoadError::Elf(_) => "elf",
            LoadError::NoFrame => "no-frame",
            LoadError::BadFrame => "bad-frame",
            LoadError::Map => "map",
            LoadError::TooManyRanges => "ranges",
            LoadError::Ring => "ring",
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

/// `true` ⇔ `[va, va+len)` nằm trọn trong **một** dải user đã đăng ký.
pub fn registered_range_ok(va: u64, len: u64) -> bool {
    let count = RANGE_COUNT.load(Ordering::Acquire) as usize;
    let Some(stop) = va.checked_add(len) else {
        return false;
    };
    let mut index = 0;
    while index < count && index < MAX_USER_RANGES {
        let base = RANGE_BASE[index].load(Ordering::Acquire);
        let range_len = RANGE_LEN[index].load(Ordering::Acquire);
        if base != 0 && va >= base && stop <= base.saturating_add(range_len) {
            return true;
        }
        index += 1;
    }
    false
}

/// Đăng ký dải user `[base, base+len)` (gọi bởi loader/stack/ring). Trả `false` nếu bảng đầy.
pub fn register_range(base: u64, len: u64) -> bool {
    let index = RANGE_COUNT.load(Ordering::Acquire) as usize;
    if index >= MAX_USER_RANGES {
        return false;
    }
    RANGE_BASE[index].store(base, Ordering::Release);
    RANGE_LEN[index].store(len, Ordering::Release);
    RANGE_COUNT.store((index + 1) as u32, Ordering::Release);
    true
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

/// Nạp **một** ELF64 `ET_EXEC` vào `vs` theo đúng VA liên kết của nó.
///
/// `bytes` là toàn bộ tệp (ảnh kiểm nhúng bằng `include_bytes!`); `machine` là `e_machine` mà
/// kiến trúc chấp nhận (AArch64 = 183) — hàm **không** tự đoán kiến trúc.
pub fn load_elf<M: TableMemory>(
    vs: &mut VSpace<M>,
    bytes: &[u8],
    machine: u16,
) -> Result<Loaded, LoadError> {
    let image = vifast_elf::parse(bytes, machine, USER_WINDOW_BASE, USER_WINDOW_END)
        .map_err(LoadError::Elf)?;
    let mut pages = 0usize;
    let mut base = u64::MAX;
    let mut end = 0u64;
    for segment in image.segments() {
        let seg_start = segment.vaddr & !0xFFF;
        let seg_end = segment.memory_end().ok_or(LoadError::BadFrame)?;
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
            let page_end = va + 0x1000;
            let Some(pa) = bootmem::alloc_boot_frame() else {
                return Err(LoadError::NoFrame);
            };
            asm::zero_boot_ram(pa, 0x1000);
            // Copy phần giao giữa [filesz) và trang này — `.bss` ở lại 0.
            let copy_start = va.max(file_start);
            let copy_end = page_end.min(file_end);
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
            let Some(frame) = Frame::new(pa) else {
                return Err(LoadError::BadFrame);
            };
            let perms = perms_for(segment.executable(), segment.writable());
            vs.map_obj_with(va, frame, perms, MemAttr::Normal)
                .map_err(|_| LoadError::Map)?;
            pages += 1;
            va = page_end;
        }
    }
    if pages == 0 || base == u64::MAX || end <= base {
        return Err(LoadError::Elf(ElfError::NoSegments));
    }
    if !register_range(base, end - base) {
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
/// Trả `(base, bytes)`; đăng ký dải để hook đọc bộ nhớ user chấp nhận nếu chương trình in chuỗi
/// trên stack (không bắt buộc nhưng vô hại).
pub fn map_user_stack<M: TableMemory>(
    vs: &mut VSpace<M>,
    top: u64,
    pages: usize,
) -> Result<(u64, u64), LoadError> {
    let bytes = (pages as u64) * 0x1000;
    let base = top.checked_sub(bytes).ok_or(LoadError::BadFrame)?;
    let mut index = 0usize;
    while index < pages {
        let va = base + (index as u64) * 0x1000;
        let Some(pa) = bootmem::alloc_boot_frame() else {
            return Err(LoadError::NoFrame);
        };
        asm::zero_boot_ram(pa, 0x1000);
        let Some(frame) = Frame::new(pa) else {
            return Err(LoadError::BadFrame);
        };
        vs.map_obj_with(va, frame, Perms::USER_RW, MemAttr::Normal)
            .map_err(|_| LoadError::Map)?;
        index += 1;
    }
    if !register_range(base, bytes) {
        return Err(LoadError::TooManyRanges);
    }
    Ok((base, bytes))
}

/// [`vifast_ring::Mem`] trên **PA identity** của arena boot (kernel ghi ring trước khi `eret`).
struct PhysMem {
    base: u64,
}

impl vifast_ring::Mem for PhysMem {
    fn load(&self, offset: usize) -> u64 {
        asm::read_u64(self.base + offset as u64)
    }
    fn store(&self, offset: usize, value: u64) {
        asm::write_u64(self.base + offset as u64, value);
    }
    fn acquire(&self, offset: usize) -> u64 {
        // Một hart ghi trước khi `eret`; `read_u64` là `volatile` nên thứ tự phát sinh đủ trên
        // đường boot một chiều (không có consumer chạy song song ở thời điểm này).
        asm::read_u64(self.base + offset as u64)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        asm::write_u64(self.base + offset as u64, value);
    }
}

/// VA ring UART mà **cùng nguồn** `shell.rs` mở (`VA_CON_RING` của nó) — không đổi theo arch.
pub const RING_VA: u64 = 0x4300_0000;

/// Cấp một frame, khởi tạo **ring KT-69** hình học 32 slot × 32 B rồi map `USER_RW` tại
/// [`RING_VA`], sau đó bơm `script` (byte) thành từng slot 24 B qua codec `conchan`.
///
/// Đây là **nguồn vào** của shell trên aarch64: cùng định dạng ring mà `shell.rs` đọc, nên shell
/// **không** cần biết nó đang chạy trên arch nào (không `cfg` trong thân hàm).
pub fn map_ring_with_script<M: TableMemory>(
    vs: &mut VSpace<M>,
    script: &[u8],
) -> Result<u64, LoadError> {
    let Some(pa) = bootmem::alloc_boot_frame() else {
        return Err(LoadError::NoFrame);
    };
    asm::zero_boot_ram(pa, 0x1000);
    fill_ring(pa, script)?;
    let Some(frame) = Frame::new(pa) else {
        return Err(LoadError::BadFrame);
    };
    vs.map_obj_with(RING_VA, frame, Perms::USER_RW, MemAttr::Normal)
        .map_err(|_| LoadError::Map)?;
    if !register_range(RING_VA, 0x1000) {
        return Err(LoadError::TooManyRanges);
    }
    Ok(pa)
}

/// Khởi tạo lại **ring KT-69** trên `pa` (PA đã map identity) rồi bơm `script` — dùng cho pha
/// thứ hai của cùng một frame ring (KT-339a: ảnh kiểm chạy lại shell sản phẩm trên ring mà
/// KT-339 đã cấp) **và** cho đường nạp mới. Cùng codec `conchan`, không có bản sao thứ hai.
pub fn fill_ring(pa: u64, script: &[u8]) -> Result<(), LoadError> {
    let ring = vifast_ring::Ring::new(PhysMem { base: pa });
    ring.init(
        vifast_virtio::conchan::CHANNEL_SLOTS,
        vifast_virtio::conchan::SLOT_BYTES,
    )
    .map_err(|_| LoadError::Ring)?;
    let mut start = 0usize;
    while start < script.len() {
        let mut slot = [0u8; vifast_virtio::conchan::CHUNK_BYTES];
        let mut used = 0usize;
        while start < script.len() && used < slot.len() {
            slot[used] = script[start];
            used += 1;
            start += 1;
        }
        let (words, _) = vifast_virtio::conchan::pack(&slot[..used]);
        ring.send(&words).map_err(|_| LoadError::Ring)?;
    }
    Ok(())
}

/// VA physmap của một PA — tiện cho chẩn đoán (`# k5 ring: pa=… va=…`).
pub const fn physmap_of(pa: u64) -> u64 {
    PHYSMAP_BASE + pa
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bảng dải **fail-closed**: đăng ký rồi kiểm đúng biên; dải chưa đăng ký phải `false`.
    #[test]
    fn dai_user_fail_closed() {
        // Test host dùng bảng riêng của tiến trình test; `register_range` tuyến tính nên kiểm
        // đúng dải vừa đăng ký và dải ngoài.
        assert!(register_range(0x7100_0000, 0x4000));
        assert!(registered_range_ok(0x7100_0000, 16));
        assert!(registered_range_ok(0x7100_3FF0, 16));
        assert!(!registered_range_ok(0x7100_3FF8, 16));
        assert!(!registered_range_ok(0x7100_4000, 4));
        assert!(!registered_range_ok(0x70FF_FFF0, 16));
    }

    /// `perms_for` **không** bao giờ sinh W^X: exec ⇒ RX; write ⇒ RW; còn lại ⇒ RO.
    #[test]
    fn quyen_wx() {
        assert_eq!(perms_for(true, false), Perms::USER_RX);
        assert_eq!(perms_for(false, true), Perms::USER_RW);
        assert_eq!(perms_for(false, false), Perms::USER_RO);
        let wx = perms_for(true, true);
        assert!(wx.exec && !wx.write, "exec thắng — không có W|X");
    }
}
