// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bản ghi bàn giao `BootInfo` v1** — KT-414 §"chụp memory map ngay trước `ExitBootServices`
//! nạp chuẩn vào `BootInfo` v1" (D2-260).
//!
//! ## Vì sao là bản ghi riêng, không phải `BootInfo` của kernel
//!
//! `BootInfo` v1 (`v2/kernel/vifast-plat-common/src/bootinfo.rs`) là struct **nội bộ kernel**
//! (enum Rust, `Option<u64>`, `[RamRegion; 16]` — layout không phải ABI). Boot stub UEFI chạy
//! **trước** kernel, trong ảnh `.efi` riêng, nên nó ghi một bản ghi `#[repr(C)]` **ổn định**
//! ([`UefiHandoffV1`]) rồi kernel đọc bằng offset. Từ **D2-260**, kernel có
//! `BootSource::Uefi` (`name() => "uefi"`) và `BootInfo::validate()` fail-closed theo nguồn
//! (`Uefi` chỉ đòi `n_ram > 0` + mọi vùng RAM hợp lệ) — bản ghi này là **nguồn** để kernel dựng
//! `BootInfo { source: Uefi, .. }` mà **không** khai man `source=acpi`.
//!
//! ## Cách kernel đọc
//!
//! - Vị trí: một `static` trong ảnh `.efi`; địa chỉ runtime in ở dòng
//!   `# uefi: handoff … va=0x… size=…`.
//! - Trước khi nhảy `e_entry`, loader truyền địa chỉ này (x86_64: `rdi`; aarch64: `x0`).
//! - Kernel kiểm `magic`, `version`, `size_bytes`, `checksum`, `arch`, `source`, `n_ram` và
//!   từng vùng RAM; sai ⇒ fail-closed có tên (dùng `BootInfoError::BadMagic`/`BadChecksum`/
//!   `UnsupportedVersion`/`MissingMemory`/`BadRegion` của kernel).
//! - `rsdp_pa`/`dtb_pa` (0 = firmware không khai) cho kernel parse ACPI/DTB **sau** khi đã có
//!   bảng nhớ — không phải đoán địa chỉ.
//!
//! ## Bất biến **trang bàn giao** (reserved/copy) — bắt buộc khi nhảy `e_entry`
//!
//! Ảnh `.efi` nằm trong vùng `EfiLoaderCode/Data` — sau `ExitBootServices` vùng đó **được nhả**
//! cho kernel. Vì vậy trước khi nhảy `e_entry`, bên nhận **phải** hoặc:
//!
//! (a) đánh dấu `[pa, pa + size_bytes)` là **reserved** trong bảng nhớ bàn giao (không cấp cho
//!     frame allocator), hoặc
//! (b) **chép** bản ghi ra trang riêng **trước** khi cấp phát RAM.
//!
//! `pa` là con trỏ bản ghi loader truyền; `size_bytes` là trường trong bản ghi;
//! [`UefiHandoffV1::reserved_pages`] tính số trang 4 KiB cần giữ. Stub in bất biến này ở dòng
//! `# uefi: handoff reserved pa=0x… bytes=… pages=… invariant=kernel-must-reserve-or-copy` để
//! runner đối chiếu được. **Chưa** nhảy `e_entry` trong KT-414 (kernel chưa mở API nhận bản ghi);
//! bất biến được ghi ở đây + trong README §4 để lần nối dây không bỏ sót.

use crate::bootinfo::MAX_USABLE_RAM;
use crate::memmap::{MapInfo, MemMapStats, PAGE_BYTES};

/// Magic bản ghi: `"VIFASTH1"` (little-endian u64).
pub const HANDOFF_MAGIC: u64 = u64::from_le_bytes(*b"VIFASTH1");
/// Phiên bản bản ghi bàn giao.
pub const HANDOFF_VERSION: u16 = 1;
/// `arch` x86_64 (ánh xạ cố định; `BootArch` của kernel không `repr(C)`).
pub const ARCH_X86_64: u8 = 3;
/// `source` = bảng nhớ UEFI (`GetMemoryMap`) — ánh xạ tới `BootSource::Uefi` của kernel
/// (`name() => "uefi"`, D2-260). Giá trị cố định của **bản ghi**, không phải `repr` của enum.
pub const SOURCE_UEFI_MEMMAP: u8 = 2;
/// Cỡ tối đa một bản ghi (kiểm khi kernel đọc).
pub const HANDOFF_BYTES: usize = core::mem::size_of::<UefiHandoffV1>();
/// Tên bất biến trang bàn giao (in ở dòng `# uefi: handoff reserved …`).
pub const HANDOFF_RESERVED_INVARIANT: &str = "kernel-must-reserve-or-copy";

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
    /// Địa chỉ vật lý `RSDP` (ACPI 2.0 ưu tiên, fallback ACPI 1.0) lấy từ
    /// `EFI_CONFIGURATION_TABLE`; **0** nếu firmware không khai (chưa cấp).
    pub rsdp_pa: u64,
    /// Địa chỉ vật lý DTB lấy từ `EFI_CONFIGURATION_TABLE`; **0** nếu firmware không khai
    /// (`x86_64` thường 0 — ACPI là nguồn cấu hình).
    pub dtb_pa: u64,
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
    rsdp_pa: 0,
    dtb_pa: 0,
    ram: [HandoffRam { base: 0, size: 0 }; MAX_USABLE_RAM],
};

impl UefiHandoffV1 {
    /// Dựng bản ghi từ thống kê bảng nhớ (chưa có checksum — gọi [`Self::seal`]).
    ///
    /// `rsdp_pa`/`dtb_pa` là địa chỉ vật lý lấy từ `EFI_CONFIGURATION_TABLE`; **0** khi firmware
    /// không khai (không phải lỗi — `x86_64` không cần DTB).
    pub fn build(st: &MemMapStats, info: &MapInfo, rsdp_pa: u64, dtb_pa: u64) -> Self {
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
        rec.rsdp_pa = rsdp_pa;
        rec.dtb_pa = dtb_pa;
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

    /// Số trang 4 KiB mà bản ghi chiếm khi được đặt tại `pa` — đầu vào của bất biến
    /// reserved/copy ([`HANDOFF_RESERVED_INVARIANT`]): kernel phải giữ đúng `[pa, pa +
    /// size_bytes)` (hoặc chép bản ghi ra trang riêng) **trước** khi cấp phát RAM.
    pub const fn reserved_pages(&self, pa: u64) -> u64 {
        let bytes = (pa % PAGE_BYTES) + self.size_bytes as u64;
        bytes.div_ceil(PAGE_BYTES)
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

    /// Kiểm bản ghi (dùng cho kernel tương lai **và** test host): magic/version/size/arch/source/
    /// checksum, số vùng RAM **> 0** và từng vùng hợp lệ, căn của `rsdp_pa`/`dtb_pa` khi khác 0.
    /// Trả tên lỗi chữ ổn định (khớp `BootInfoError::name` khi có thể).
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
        if self.arch != ARCH_X86_64 {
            return Err("bad-arch");
        }
        if self.source != SOURCE_UEFI_MEMMAP {
            return Err("bad-source");
        }
        if usize::from(self.n_ram) > MAX_USABLE_RAM {
            return Err("too-many-regions");
        }
        // **Fail-closed theo nguồn** (D2-260): bản ghi `Uefi` không có vùng RAM nào là vô nghĩa
        // (kernel sẽ không có frame nào để cấp) ⇒ từ chối ngay, không để kernel phát hiện muộn.
        if self.n_ram == 0 {
            return Err("missing-memory");
        }
        for r in &self.ram[..usize::from(self.n_ram)] {
            if r.size == 0 || r.base.checked_add(r.size).is_none() {
                return Err("bad-region");
            }
        }
        // RSDP phải căn 16 byte (ACPI 6.x §5.2.2); DTB căn 8 byte (devicetree spec §5.1).
        if self.rsdp_pa != 0 && !self.rsdp_pa.is_multiple_of(16) {
            return Err("bad-rsdp");
        }
        if self.dtb_pa != 0 && !self.dtb_pa.is_multiple_of(8) {
            return Err("bad-dtb");
        }
        if self.checksum != self.compute_checksum() {
            return Err("bad-checksum");
        }
        Ok(())
    }
}
