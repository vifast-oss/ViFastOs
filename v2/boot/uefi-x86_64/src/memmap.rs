// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Kiểm bảng nhớ UEFI fail-closed có tên** (KT-414).
//!
//! `GetMemoryMap` trả một **mảng byte** với *stride* `DescriptorSize` do firmware khai; mọi thứ
//! (số mô tả, loại, trang, thuộc tính) đều là dữ liệu firmware. Một mô tả hỏng mà bên đọc bỏ qua
//! sẽ khiến kernel cấp phát đè lên firmware/ACPI — treo khó tái hiện (xem
//! `v2/docs/u1-uefi-readiness.md` §4.4). Vì vậy trước khi bàn giao:
//!
//! 1. kiểm **siêu dữ liệu** (`desc_version`, `desc_size`, `map_size` chia hết, không vượt đệm);
//! 2. kiểm **từng mô tả** (`NumberOfPages > 0`, `PhysicalStart` căn 4 KiB, không tràn địa chỉ,
//!    `Type` trong dải UEFI, `Attribute` có đúng một bit cacheability);
//! 3. tính **checksum cộng dồn** của toàn bộ mảng (UEFI *không* có trường checksum — đây là phép
//!    kiểm của ta, ghi vào bản ghi bàn giao và in ở dòng `# uefi: memmap=ok ... sum=0x…`).
//!
//! Mọi lỗi là **một tên chữ** ([`UefiError::name`]), không panic, không dùng giá trị mặc định.

use crate::bootinfo::{RamRegion, MAX_USABLE_RAM};
use crate::efi::MemoryDescriptor;

/// Phiên bản `EFI_MEMORY_DESCRIPTOR` duy nhất đang dùng (`EFI_MEMORY_DESCRIPTOR_VERSION`).
pub const DESC_VERSION: u32 = 1;
/// Cỡ tối thiểu của một mô tả (40 byte với UINTN 64-bit).
pub const DESC_MIN: usize = core::mem::size_of::<MemoryDescriptor>();
/// Stride mô tả phải là bội của 8 (ABI của `EFI_MEMORY_DESCRIPTOR`).
pub const DESC_ALIGN: usize = 8;
/// Cỡ trang UEFI (4 KiB) — `NumberOfPages` đếm theo đơn vị này.
pub const PAGE_BYTES: u64 = 4096;
/// `EfiMaxMemoryType` (UEFI 2.10): `Type` hợp lệ nằm trong `0..=0x10`.
pub const TYPE_MAX: u32 = 0x10;
/// Trần mô tả giữ trong một mảng (đệm 64 KiB / 40 B ≈ 1.600; trần này chỉ để chặn số vô lý).
pub const MAX_DESCRIPTORS: usize = 4096;

/// Loại bộ nhớ "dùng được cho kernel sau `ExitBootServices`".
///
/// `1..=4` = loader/boot-services code+data (firmware nhả sau khi thoát), `7` =
/// `EfiConventionalMemory`. **Không** gồm runtime services (`5`,`6`), ACPI reclaim/NVS (`9`,`10`),
/// MMIO (`11`,`12`), reserved (`0`), unusable (`8`), PAL (`13`), persistent (`14`), unaccepted
/// (`15` — cần `AcceptMemory` trước khi dùng).
pub fn is_usable(ty: u32) -> bool {
    matches!(ty, 1..=4 | 7)
}

/// Tên loại bộ nhớ ổn định cho log (chỉ các loại phổ biến; loại khác in `type-N`).
pub fn type_name(ty: u32) -> &'static str {
    match ty {
        0 => "reserved",
        1 => "loader-code",
        2 => "loader-data",
        3 => "bs-code",
        4 => "bs-data",
        5 => "rt-code",
        6 => "rt-data",
        7 => "conventional",
        8 => "unusable",
        9 => "acpi-reclaim",
        10 => "acpi-nvs",
        11 => "mmio",
        12 => "mmio-port",
        13 => "pal",
        14 => "persistent",
        15 => "unaccepted",
        16 => "max",
        _ => "type-other",
    }
}

/// Lỗi **có tên** của boot stub. `name()` là mã chữ ổn định dùng trong
/// `# uefi: fail-closed err=<mã>` (đối chiếu được với runner, không phải số trần).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UefiError {
    /// Gọi `GetMemoryMap` thất bại (hoặc lặp `EFI_BUFFER_TOO_SMALL` vô hạn).
    MapQuery,
    /// `MapSize == 0` (bảng rỗng).
    MapEmpty,
    /// `DescriptorVersion != 1`.
    MapDescVersion,
    /// `DescriptorSize` nhỏ hơn 40 hoặc không là bội của 8.
    MapDescSize,
    /// `MapSize % DescriptorSize != 0` (số mô tả lẻ) — bảng bị cắt giữa mô tả.
    MapSize,
    /// `MapSize >` đệm (không đủ chỗ đọc; không đọc ngoài biên).
    MapTruncated,
    /// Một mô tả có `NumberOfPages == 0`, `PhysicalStart` lệch trang, hoặc tràn địa chỉ.
    MapBadRegion,
    /// `Type` ngoài `0..=0x10`.
    MapBadType,
    /// `Attribute` không có đúng một bit cacheability (`UC/WC/WT/WB`).
    MapBadAttribute,
    /// Số vùng RAM usable vượt trần `BootInfo` v1 (`MAX_USABLE_RAM = 16`).
    TooManyRegions,
    /// `BootInfo::validate()` từ chối bảng nhớ đã qua [`validate`] — **fail-closed theo nguồn**
    /// (D2-260): bảng nhớ "hợp lệ theo UEFI" nhưng không còn vùng RAM usable nào (hoặc vùng RAM
    /// tràn địa chỉ) ⇒ kernel sẽ không có frame nào để cấp. Tên lỗi là tên của `BootInfoError`.
    BootInfo(&'static str),
    /// `ExitBootServices` trả lỗi khác `EFI_INVALID_PARAMETER` hoặc cạn số lần thử.
    ExitBootServices,
}

impl UefiError {
    /// Mã chữ ổn định (dùng trong log + runner).
    pub const fn name(self) -> &'static str {
        match self {
            UefiError::MapQuery => "map-query",
            UefiError::MapEmpty => "map-empty",
            UefiError::MapDescVersion => "map-desc-version",
            UefiError::MapDescSize => "map-desc-size",
            UefiError::MapSize => "map-size",
            UefiError::MapTruncated => "map-truncated",
            UefiError::MapBadRegion => "map-bad-region",
            UefiError::MapBadType => "map-bad-type",
            UefiError::MapBadAttribute => "map-bad-attribute",
            UefiError::TooManyRegions => "too-many-regions",
            UefiError::BootInfo(name) => name,
            UefiError::ExitBootServices => "exit-boot-services",
        }
    }

    /// Mã thoát QEMU (`isa-debug-exit`) tương ứng — tách lớp lỗi bảng nhớ khỏi lỗi thoát dịch vụ.
    pub const fn exit_val(self) -> u32 {
        match self {
            UefiError::ExitBootServices => 0x21,
            _ => 0x20,
        }
    }
}

/// Siêu dữ liệu một lần `GetMemoryMap` thành công (đầu vào của [`validate`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapInfo {
    /// Số byte mảng mô tả (đã gồm padding cuối).
    pub map_size: usize,
    /// Stride một mô tả do firmware khai.
    pub desc_size: usize,
    /// `EFI_MEMORY_DESCRIPTOR_VERSION`.
    pub desc_version: u32,
    /// `MapKey` — bắt buộc khớp ở lần `ExitBootServices` kế tiếp.
    pub map_key: usize,
    /// Sức chứa đệm thật đã cấp.
    pub capacity: usize,
}

impl MapInfo {
    /// Số mô tả suy ra từ `map_size / desc_size` (0 nếu stride 0 — không chia cho 0).
    ///
    /// `const fn` trên Rust 1.100 chưa cho `checked_div().unwrap_or(0)` (chỉ const-stable một
    /// phần) ⇒ giữ `if`; allow lint vì đây là dạng đã kiểm chia 0 tường minh.
    #[allow(clippy::manual_checked_ops)]
    pub const fn count(&self) -> usize {
        if self.desc_size == 0 {
            0
        } else {
            self.map_size / self.desc_size
        }
    }
}

/// Thống kê bảng nhớ đã kiểm — đủ để in bằng chứng và dựng bản ghi bàn giao.
#[derive(Clone, Copy, Debug)]
pub struct MemMapStats {
    /// Số mô tả.
    pub count: usize,
    /// Số mô tả loại "dùng được".
    pub n_usable: usize,
    /// Tổng byte RAM dùng được (bão hoà khi tràn).
    pub usable_bytes: u64,
    /// Địa chỉ vật lý cao nhất mà bảng mô tả (byte, không bao gồm).
    pub highest_phys: u64,
    /// Số mô tả runtime services (`5`,`6`).
    pub n_runtime: usize,
    /// Số mô tả ACPI reclaim/NVS (`9`,`10`).
    pub n_acpi: usize,
    /// Số mô tả reserved/unusable (`0`,`8`).
    pub n_reserved: usize,
    /// Số mô tả **không** usable mà `Attribute` không có đúng một bit cache (dị thường firmware;
    /// chỉ in bằng chứng, không fail-closed).
    pub n_attr_odd: usize,
    /// Bitmask các `Type` đã gặp (bit `i` ⇔ có loại `i`).
    pub type_mask: u32,
    /// Checksum cộng dồn (mod 2^64) của toàn bộ mảng mô tả — phép kiểm của ta (UEFI không có).
    pub sum: u64,
    /// Các vùng RAM usable đã gộp vùng kề nhau (≤ `MAX_USABLE_RAM`).
    pub ram: [RamRegion; MAX_USABLE_RAM],
    /// Số vùng RAM đã ghi.
    pub n_ram: u8,
}

impl MemMapStats {
    /// Các vùng RAM đã ghi (như `BootInfo::usable_ram`).
    pub fn usable_ram(&self) -> &[RamRegion] {
        &self.ram[..usize::from(self.n_ram)]
    }
}

/// Đọc một mô tả tại `base + i * info.desc_size` (không đọc ngoài `map_size`).
///
/// # Safety
/// `buf` phải trỏ tới `info.map_size` byte đọc được (do `GetMemoryMap` ghi).
unsafe fn descriptor_at(buf: *const u8, info: &MapInfo, i: usize) -> MemoryDescriptor {
    let off = i * info.desc_size;
    let p = unsafe { buf.add(off) } as *const MemoryDescriptor;
    // SAFETY: `off + 40 <= map_size <= capacity` đã kiểm ở `validate` (stride ≥ 40, không tràn).
    unsafe { core::ptr::read_unaligned(p) }
}

/// Kiểm bảng nhớ và dựng thống kê. **Không** panic; mọi bất thường trả [`UefiError`] có tên.
///
/// # Safety
/// `buf` phải trỏ tới `info.map_size` byte đọc được (đúng như `GetMemoryMap` vừa ghi).
pub unsafe fn validate(buf: *const u8, info: &MapInfo) -> Result<MemMapStats, UefiError> {
    // Thứ tự kiểm có chủ đích: siêu dữ liệu trước (rẻ, chặn mọi phép đọc nguy hiểm), rồi mô tả.
    if info.desc_version != DESC_VERSION {
        return Err(UefiError::MapDescVersion);
    }
    if info.desc_size < DESC_MIN || !info.desc_size.is_multiple_of(DESC_ALIGN) {
        return Err(UefiError::MapDescSize);
    }
    if info.map_size == 0 {
        return Err(UefiError::MapEmpty);
    }
    if !info.map_size.is_multiple_of(info.desc_size) {
        return Err(UefiError::MapSize);
    }
    if info.map_size > info.capacity {
        return Err(UefiError::MapTruncated);
    }
    if buf.is_null() {
        return Err(UefiError::MapQuery);
    }
    let count = info.map_size / info.desc_size;
    if count == 0 || count > MAX_DESCRIPTORS {
        return Err(UefiError::MapTruncated);
    }

    let mut st = MemMapStats {
        count,
        n_usable: 0,
        usable_bytes: 0,
        highest_phys: 0,
        n_runtime: 0,
        n_acpi: 0,
        n_reserved: 0,
        n_attr_odd: 0,
        type_mask: 0,
        sum: 0,
        ram: [RamRegion { base: 0, size: 0 }; MAX_USABLE_RAM],
        n_ram: 0,
    };

    for i in 0..count {
        // SAFETY: xem `descriptor_at`.
        let d = unsafe { descriptor_at(buf, info, i) };
        let end = d
            .physical_start
            .checked_add(d.number_of_pages.saturating_mul(PAGE_BYTES))
            .ok_or(UefiError::MapBadRegion)?;
        if d.number_of_pages == 0 || d.physical_start % PAGE_BYTES != 0 || end <= d.physical_start {
            return Err(UefiError::MapBadRegion);
        }
        if d.ty > TYPE_MAX {
            return Err(UefiError::MapBadType);
        }
        // UEFI 2.10 §7.2 khuyến nghị **đúng một** bit cacheability (UC/WC/WT/WB) trong `0xF`.
        // Đo trên OVMF của QEMU 11 (`edk2-x86_64-code.fd`, rev 0x00020046): **mọi** mô tả trả
        // `Attribute = 0xF` — bốn bit cùng bật. Vì vậy phép kiểm fail-closed là: vùng **usable**
        // (kernel sẽ map) phải có **ít nhất một** bit cacheability; `0` ⇒ lỗi có tên. Trường hợp
        // nhiều-bit (khác khuyến nghị nhưng gặp thật) chỉ đếm vào `n_attr_odd` để in bằng chứng,
        // không chặn boot — chặn ở đây là false-red trên firmware thật.
        let cache = d.attribute & 0xF;
        if cache == 0 {
            st.n_attr_odd += 1;
            if is_usable(d.ty) {
                return Err(UefiError::MapBadAttribute);
            }
        } else if cache.count_ones() != 1 {
            st.n_attr_odd += 1;
        }
        st.type_mask |= 1u32 << d.ty;
        st.highest_phys = st.highest_phys.max(end);
        match d.ty {
            5 | 6 => st.n_runtime += 1,
            9 | 10 => st.n_acpi += 1,
            0 | 8 => st.n_reserved += 1,
            _ => {}
        }
        if is_usable(d.ty) {
            st.n_usable += 1;
            let base = d.physical_start;
            // An toàn: `end` ở trên đã kiểm `checked_add(saturating_mul)` ⇒ phép nhân không tràn.
            let size = d.number_of_pages * PAGE_BYTES;
            // Gộp vùng kề nhau để `BootInfo` (trần 16 vùng) không bị phân mảnh vô ích; vẫn giữ
            // nguyên tổng byte và địa chỉ — không bỏ vùng nào.
            let n = usize::from(st.n_ram);
            if n > 0 && st.ram[n - 1].end() == base {
                st.ram[n - 1].size = st.ram[n - 1].size.saturating_add(size);
            } else if n < MAX_USABLE_RAM {
                st.ram[n] = RamRegion { base, size };
                st.n_ram += 1;
            } else {
                return Err(UefiError::TooManyRegions);
            }
            st.usable_bytes = st.usable_bytes.saturating_add(d.number_of_pages * PAGE_BYTES);
        }
    }

    // Checksum cộng dồn của mảng mô tả thật (byte-by-byte).
    let mut sum = 0u64;
    let mut off = 0usize;
    while off < info.map_size {
        // SAFETY: `off < map_size <= capacity`.
        let b = unsafe { core::ptr::read(buf.add(off)) };
        sum = sum.wrapping_add(u64::from(b));
        off += 1;
    }
    st.sum = sum;
    Ok(st)
}

/// Tên các lỗi tiêm được của `run.sh --selftest` (danh sách đóng, khớp `build.rs`).
pub const FAULTS: &[&str] = &[
    "desc-size",
    "desc-version",
    "map-size",
    "map-empty",
    "bad-type",
    "bad-page",
    "bad-attr",
    "no-usable",
];

/// Làm hỏng **bản sao** bảng nhớ trong đệm theo `fault` (tiêm lỗi của harness).
///
/// Chỉ dùng cho ca âm: bản naive (trước khi vá) vẫn `ExitBootServices` ⇒ phép kiểm không có giá
/// trị. Trả `false` nếu `fault` rỗng (không tiêm).
///
/// # Safety
/// `buf` phải trỏ tới `info.capacity` byte ghi được; `info` phải khớp lần `GetMemoryMap` vừa ghi.
pub unsafe fn apply_fault(buf: *mut u8, info: &mut MapInfo, fault: &str) -> bool {
    match fault {
        "" => false,
        "desc-size" => {
            // 39 < 40 và không phải bội 8 ⇒ `map-desc-size`.
            info.desc_size = 39;
            true
        }
        "desc-version" => {
            info.desc_version = 2;
            true
        }
        "map-size" => {
            // Lệch stride ⇒ `map-size` (bảng bị cắt giữa mô tả).
            info.map_size = info.map_size.saturating_add(1);
            true
        }
        "map-empty" => {
            info.map_size = 0;
            true
        }
        "bad-type" => {
            // 0x11 = 17 > `EfiMaxMemoryType` (0x10). Cố ý **không** dùng 0 (reserved là hợp lệ).
            unsafe { patch_descriptor(buf, info, 0, |d| d.ty = 0x11) };
            true
        }
        "bad-page" => {
            unsafe { patch_descriptor(buf, info, 0, |d| d.number_of_pages = 0) };
            true
        }
        "bad-attr" => {
            // Vùng **usable** đầu tiên (mô tả 0 của firmware thật thường là reserved; vá nó sẽ
            // không kích hoạt phép kiểm ⇒ ca âm giả).
            let n = info.count();
            let mut i = 0usize;
            let mut hit = false;
            while i < n {
                let d = unsafe { descriptor_at(buf as *const u8, info, i) };
                if is_usable(d.ty) {
                    unsafe { patch_descriptor(buf, info, i, |d| d.attribute = 0) };
                    hit = true;
                    break;
                }
                i += 1;
            }
            hit
        }
        "no-usable" => {
            // **Ca âm của `BootInfo::validate` theo nguồn `Uefi`** (D2-260): bảng nhớ vẫn **hợp
            // lệ theo UEFI** (mọi mô tả đúng stride/trang/thuộc tính) nhưng **không còn** vùng
            // RAM usable nào ⇒ `memmap::validate` xanh, `BootInfo::validate` phải fail-closed
            // `missing-memory`. Đây là ca âm mà 7 lỗi kia **không** chạm tới (chúng hỏng bảng
            // nhớ, không hỏng "không có RAM").
            let n = info.count();
            let mut i = 0usize;
            let mut hit = false;
            while i < n {
                let d = unsafe { descriptor_at(buf as *const u8, info, i) };
                if is_usable(d.ty) {
                    // `0` = `EfiReservedMemoryType` (hợp lệ, không usable).
                    unsafe { patch_descriptor(buf, info, i, |d| d.ty = 0) };
                    hit = true;
                }
                i += 1;
            }
            hit
        }
        _ => false,
    }
}

/// Sửa mô tả thứ `i` tại chỗ (stride theo firmware, không theo `size_of`).
///
/// # Safety
/// `buf + i*desc_size + 40` phải nằm trong đệm; `info.map_size >= (i+1)*desc_size`.
unsafe fn patch_descriptor(
    buf: *mut u8,
    info: &MapInfo,
    i: usize,
    f: impl FnOnce(&mut MemoryDescriptor),
) {
    let off = i * info.desc_size;
    let p = unsafe { buf.add(off) } as *mut MemoryDescriptor;
    // SAFETY: 40 byte đầu của mô tả luôn nằm trong bảng; đọc/sửa không đổi phần dư stride.
    let mut d = unsafe { core::ptr::read_unaligned(p) };
    f(&mut d);
    unsafe { core::ptr::write_unaligned(p, d) };
}
