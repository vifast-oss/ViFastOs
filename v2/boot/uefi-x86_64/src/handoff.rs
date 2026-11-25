// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bản ghi bàn giao `BootInfo` v1 (nháp)** — KT-414 §"chụp memory map ngay trước
//! `ExitBootServices` nạp chuẩn vào `BootInfo` v1".
//!
//! ## Vì sao là "nháp" chứ không ghi thẳng vào `BootInfo`
//!
//! `BootInfo` v1 (`v2/kernel/vifast-plat-common/src/bootinfo.rs`) có
//! `BootSource::{Dtb, Acpi}` — **không** có nguồn `Uefi`, và `BootInfo::validate()` đòi
//! `apic/uart/pcie_ecam` cho `x86_64` (đọc từ ACPI). Boot stub UEFI lấy bảng nhớ từ
//! `GetMemoryMap` — nguồn thứ ba — nên ghi thẳng `source=Acpi` là **khai man**. Thêm
//! `BootSource::Uefi` là đổi ngữ nghĩa `BootInfo` v1 ⇒ cần A0/Lead duyệt (D2-04), không thuộc
//! quyền task này (ràng buộc "0 chạm kernel/plat-*").
//!
//! Vì vậy tệp này giữ **bản ghi nháp** `UefiHandoffV1` với magic/phiên bản/checksum, dùng
//! **chính** `RamRegion`/`MAX_USABLE_RAM` của kernel (include `bootinfo.rs`, không sao chép),
//! và in các dòng `# bootinfo:` **đúng định dạng** của `BootInfo::write_log` để đối chiếu. Khi
//! A0 duyệt `BootSource::Uefi`, bản ghi này chuyển thành `BootInfo` v1 (hoặc `BootInfo` v2) với
//! chi phí rất nhỏ — xem README §5 "Việc còn lại".
//!
//! ## Cách kernel đọc (khi nối dây)
//!
//! - Vị trí: một `static` trong ảnh `.efi`; địa chỉ runtime in ở dòng
//!   `# uefi: handoff va=0x… size=…`.
//! - Trước khi nhảy `e_entry`, loader truyền địa chỉ này (x86_64: `rdi`; aarch64: `x0`) — API
//!   kernel **chưa** mở nên task này chỉ in ra.
//! - Kernel kiểm `magic`, `version`, `size_bytes`, `checksum`; sai ⇒ fail-closed có tên (dùng
//!   `BootInfoError::BadMagic`/`BadChecksum`/`UnsupportedVersion` của kernel).
//! - **Cạm bẫy**: ảnh `.efi` nằm trong vùng `EfiLoaderCode/Data` — sau `ExitBootServices` vùng
//!   đó **được nhả** cho kernel. Trước khi nhảy phải hoặc (a) đánh dấu trang chứa bản ghi là
//!   reserved trong bảng nhớ bàn giao, hoặc (b) kernel chép bản ghi ra trang riêng **trước** khi
//!   dùng bộ nhớ. Task này chọn (a) ở mức ghi tài liệu; khi nối dây phải làm thật (README §5).

use crate::bootinfo::MAX_USABLE_RAM;
use crate::memmap::{MapInfo, MemMapStats};

/// Magic bản ghi: `"VIFASTH1"` (little-endian u64).
pub const HANDOFF_MAGIC: u64 = u64::from_le_bytes(*b"VIFASTH1");
/// Phiên bản bản ghi bàn giao.
pub const HANDOFF_VERSION: u16 = 1;
/// `arch` x86_64 (ánh xạ cố định; `BootArch` của kernel không `repr(C)`).
pub const ARCH_X86_64: u8 = 3;
/// `source` = bảng nhớ UEFI (`GetMemoryMap`). Giá trị **chưa** có trong `BootSource` của v1.
pub const SOURCE_UEFI_MEMMAP: u8 = 2;
/// Cỡ tối đa một bản ghi (kiểm khi kernel đọc).
pub const HANDOFF_BYTES: usize = core::mem::size_of::<UefiHandoffV1>();

/// Một vùng RAM trong bản ghi — `repr(C)` (hai `u64`) để kernel đọc được bằng ABI, không phụ
/// thuộc layout Rust.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandoffRam {
    /// Địa chỉ vật lý bắt đầu.
    pub base: u64,
    /// Kích thước (byte).
    pub size: u64,
}

/// Bản ghi bàn giao UEFI v1 (nháp). Mọi trường `repr(C)`, không con trỏ (kernel chỉ cần
/// `memcpy` vùng này), checksum cộng dồn byte.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UefiHandoffV1 {
    /// `HANDOFF_MAGIC`.
    pub magic: u64,
    /// `HANDOFF_VERSION`.
    pub version: u16,
    /// `ARCH_X86_64`.
    pub arch: u8,
    /// `SOURCE_UEFI_MEMMAP`.
    pub source: u8,
    /// Số vùng RAM đã ghi (`<= MAX_USABLE_RAM`).
    pub n_ram: u8,
    /// Số hart (UEFI boot stub chạy 1 hart; SMP để kernel tự đọc ACPI sau).
    pub n_harts: u16,
    /// Dành riêng (0) — giữ căn 8.
    pub _pad0: u16,
    /// `size_of::<UefiHandoffV1>()`.
    pub size_bytes: u32,
    /// Checksum cộng dồn mọi byte **trừ** trường này (đặt 0 khi tính).
    pub checksum: u32,
    /// Stride mô tả của firmware (`GetMemoryMap`).
    pub desc_size: u32,
    /// `EFI_MEMORY_DESCRIPTOR_VERSION` đã kiểm.
    pub desc_version: u32,
    /// Số mô tả.
    pub memmap_count: u32,
    /// `MapKey` của lần `GetMemoryMap` **ngay trước** `ExitBootServices`.
    pub memmap_key: u64,
    /// Địa chỉ vật lý cao nhất bảng mô tả (byte, không bao gồm).
    pub highest_phys: u64,
    /// Tổng byte RAM usable (đã gộp vùng kề).
    pub usable_bytes: u64,
    /// Checksum cộng dồn mảng mô tả thô (phép kiểm của boot stub).
    pub memmap_sum: u64,
    /// Hart đang boot (chỉ số trong bảng), 0 với stub này.
    pub boot_hart: u32,
    /// Dành riêng (0).
    pub _pad1: u32,
    /// Các vùng RAM usable, `HANDOFF_RAM_ENTRIES` phần tử.
    pub ram: [HandoffRam; MAX_USABLE_RAM],
}

/// Giá trị rỗng (dùng cho `static` khởi tạo).
pub const HANDOFF_EMPTY: UefiHandoffV1 = UefiHandoffV1 {
    magic: 0,
    version: 0,
    arch: 0,
    source: 0,
    n_ram: 0,
    n_harts: 0,
    _pad0: 0,
    size_bytes: 0,
    checksum: 0,
    desc_size: 0,
    desc_version: 0,
    memmap_count: 0,
    memmap_key: 0,
    highest_phys: 0,
    usable_bytes: 0,
    memmap_sum: 0,
    boot_hart: 0,
    _pad1: 0,
    ram: [HandoffRam { base: 0, size: 0 }; MAX_USABLE_RAM],
};

impl UefiHandoffV1 {
    /// Dựng bản ghi từ thống kê bảng nhớ (chưa có checksum — gọi [`Self::seal`]).
    pub fn build(st: &MemMapStats, info: &MapInfo) -> Self {
        let mut rec = HANDOFF_EMPTY;
        rec.magic = HANDOFF_MAGIC;
        rec.version = HANDOFF_VERSION;
        rec.arch = ARCH_X86_64;
        rec.source = SOURCE_UEFI_MEMMAP;
        rec.n_ram = st.n_ram;
        rec.n_harts = 1;
        rec.size_bytes = HANDOFF_BYTES as u32;
        rec.desc_size = info.desc_size as u32;
        rec.desc_version = info.desc_version;
        rec.memmap_count = st.count as u32;
        rec.memmap_key = info.map_key as u64;
        rec.highest_phys = st.highest_phys;
        rec.usable_bytes = st.usable_bytes;
        rec.memmap_sum = st.sum;
        let n = core::cmp::min(usize::from(st.n_ram), MAX_USABLE_RAM);
        for i in 0..n {
            let r = st.ram[i];
            rec.ram[i] = HandoffRam {
                base: r.base,
                size: r.size,
            };
        }
        rec
    }

    /// Tính checksum cộng dồn byte với trường `checksum = 0`.
    pub fn compute_checksum(&self) -> u32 {
        let mut copy = *self;
        copy.checksum = 0;
        let bytes = unsafe {
            // SAFETY: `repr(C)`, không padding "lạ thường" — mọi byte trong `size_of` đọc được
            // (struct nằm trong bộ nhớ hợp lệ).
            core::slice::from_raw_parts(
                (&raw const copy).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        };
        bytes
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b)))
    }

    /// Gắn checksum (gọi **sau** khi đã điền mọi trường).
    pub fn seal(&mut self) {
        self.checksum = 0;
        self.checksum = self.compute_checksum();
    }

    /// Kiểm bản ghi (dùng cho kernel tương lai **và** test host): magic/version/size/checksum và
    /// số vùng RAM. Trả tên lỗi chữ ổn định (khớp `BootInfoError::name` khi có thể).
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.magic != HANDOFF_MAGIC {
            return Err("bad-magic");
        }
        if self.version != HANDOFF_VERSION {
            return Err("unsupported-version");
        }
        if self.size_bytes as usize != HANDOFF_BYTES {
            return Err("bad-size");
        }
        if usize::from(self.n_ram) > MAX_USABLE_RAM {
            return Err("too-many-regions");
        }
        if self.checksum != self.compute_checksum() {
            return Err("bad-checksum");
        }
        Ok(())
    }
}
