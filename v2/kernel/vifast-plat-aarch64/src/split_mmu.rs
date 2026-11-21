// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Nửa cao (`TTBR1_EL1`) + physmap + wrapper `Mmu` định tuyến theo VA** — KT-253 (K4' bước 3,
//! B3.4; D2-199 chốt Q4 = (c)).
//!
//! # Vì sao **hai** `PageTable` + wrapper, không đổi trait
//!
//! D2-199: **không** thêm thao tác "nửa cao/physmap" vào `PageTableFormat`/`Mmu` của
//! `vifast-pte`. Lý do: Sv39 có **một** gốc (`satp`), AArch64 có **hai** gốc (`TTBR0_EL1`/
//! `TTBR1_EL1`); gộp vào một thao tác chung buộc **cả ba** impl + thân test generic đổi cùng lúc
//! (rủi ro KT-164 lặp lại) mà vẫn phải thêm tham số "gốc nào" vào mọi hàm.
//!
//! Thay vào đó:
//!
//! - [`SplitMmu`] giữ **hai** [`PageTable<Aarch64, M>`](crate::mmu::PageTable) — một cho vùng
//!   thấp (`TTBR0_EL1`), một cho nửa cao (`TTBR1_EL1`);
//! - mọi thao tác **định tuyến theo VA** ([`half_of`] = `is_canonical` + dải địa chỉ) rồi **gọi**
//!   `PageTable` tương ứng — **không** chép một dòng walk/map nào;
//! - `impl Mmu for SplitMmu` để tầng trên (VSpace, test generic) dùng được mà không biết nửa.
//!
//! # Bố cục VA 39-bit canonical (D2-147)
//!
//! ```text
//!   0x0000_0000_0000_0000 .. 0x0000_003F_FFFF_FFFF   nửa thấp   (bit 38 = 0)  → TTBR0
//!   0x0000_0040_0000_0000 .. 0xFFFF_FFBF_FFFF_FFFF   LỖ — không canonical
//!   0xFFFF_FFC0_0000_0000 .. 0xFFFF_FFFF_FFFF_FFFF   nửa cao    (bit 38 = 1)  → TTBR1
//! ```
//!
//! [`HIGH_HALF_BASE`] là **trang đầu** của nửa cao canonical 39-bit (`0xFFFF_FFC0_0000_0000`),
//! không phải `1 << 63`: với 39-bit VA, mọi VA canonical đều `< 2^38` hoặc `>= 0xFFFF_FFC0_…`,
//! nên ranh giới này phân nửa **chính xác** và không có VA canonical nào rơi vào lỗ.
//!
//! # Physmap
//!
//! Cửa sổ VA **cố định** trong bảng nửa cao: `VA = PHYSMAP_BASE + PA` ([`physmap_va`]). Nhờ vậy
//! kernel đọc/ghi được PA bất kỳ **đã map** mà không cần identity map, và `TTBR0_EL1` để trống
//! cho user (mục tiêu của B3.4). Physmap map bằng trang 4 KiB qua chính `PageTable::map_with`
//! (không có đường tắt nào khác) — `map_physmap` chỉ là vòng lặp cộng base.
//!
//! # Phạm vi
//!
//! Chỉ **dữ liệu + định tuyến**. `TTBR0_EL1`/`TTBR1_EL1`/`TCR`/`MAIR`/`TLBI` là việc của
//! [`crate::boot_mmu`]/[`crate::asm`] (D2-17). `VSpace` thật là B3.5 (KT-254).

use crate::format::{Aarch64, MemAttr, Perms, Shareability};
use crate::mmu::{MapError, Mmu, PageTable, TableMemory, Translation, Walk};

/// Trang đầu của **nửa cao canonical 39-bit** — ranh giới định tuyến `TTBR0`/`TTBR1`.
///
/// `0xFFFF_FFC0_0000_0000`: bit 38 = 1 và bit 63..39 = 1 ⇒ canonical; mọi VA canonical nhỏ hơn
/// giá trị này đều thuộc nửa thấp (`< 2^38`).
pub const HIGH_HALF_BASE: u64 = 0xFFFF_FFC0_0000_0000;

/// Base **physmap**: `VA = PHYSMAP_BASE + PA` (trùng base nửa cao — cửa sổ đầu tiên của nó).
pub const PHYSMAP_BASE: u64 = HIGH_HALF_BASE;

/// Base **kernel nửa cao**: ảnh kernel được map lại ở `KERNEL_HIGH_BASE + (pa - text_start)`.
///
/// Cách [`PHYSMAP_BASE`] **2 GiB** để hai cửa sổ không đè nhau kể cả khi physmap phủ hết RAM
/// 128 MiB của QEMU `virt` (bắt đầu ở PA `0x4000_0000`).
pub const KERNEL_HIGH_BASE: u64 = 0xFFFF_FFC0_8000_0000;

/// Nửa bảng trang mà một VA thuộc về.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    /// Nửa thấp — `TTBR0_EL1` (VA `< 2^38`).
    Low,
    /// Nửa cao — `TTBR1_EL1` (VA `>= HIGH_HALF_BASE`).
    High,
}

/// Định tuyến một VA về nửa của nó; `None` nếu VA **không canonical** (không đoán nửa).
///
/// Đây là **toàn bộ** logic định tuyến của [`SplitMmu`] — một hàm thuần, test được tuyệt đối.
pub const fn half_of(va: u64) -> Option<Half> {
    if !Aarch64::is_canonical(va) {
        return None;
    }
    if va >= HIGH_HALF_BASE {
        Some(Half::High)
    } else {
        Some(Half::Low)
    }
}

/// VA physmap của `pa`: `PHYSMAP_BASE + pa`.
///
/// Điều kiện: `pa < 2^48` (`AARCH64_PA_BITS`) — khi đó phép cộng không tràn `u64` và VA là
/// canonical (nửa cao). [`map_physmap`] kiểm điều kiện này trước khi gọi.
pub const fn physmap_va(pa: u64) -> u64 {
    PHYSMAP_BASE + pa
}

/// VA **alias nửa cao** của `pa` khi ảnh bắt đầu ở `start_pa` được map tại `high_base`:
/// `high_base + (pa - start_pa)`.
///
/// Điều kiện: `pa >= start_pa` và `start_pa` căn 4 KiB (linker script đặt `.text` bằng
/// `ALIGN(4K)`). Nhờ vậy alias của ảnh là một phép cộng thuần, không phụ thuộc hằng địa chỉ nạp
/// viết tay nào.
pub const fn high_alias_va(pa: u64, start_pa: u64, high_base: u64) -> u64 {
    high_base + (pa - start_pa)
}

/// Map **alias nửa cao** của `[start_pa, end_pa)` vào bảng `table` (phải là bảng **nửa cao**):
/// `VA = high_base + (PA - start_pa)`.
///
/// `start_pa` phải căn 4 KiB (ảnh kernel căn trang bằng `ALIGN(4K)` của linker script) — nếu
/// không, hàm trả [`MapError::Unaligned`] thay vì lặng lẽ map lệch. Trả **số trang** đã map;
/// khoảng rỗng ⇒ `Ok(0)`.
pub fn map_high_alias<M: TableMemory>(
    table: &mut PageTable<Aarch64, M>,
    start_pa: u64,
    end_pa: u64,
    high_base: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    map_high_alias_from(table, start_pa, end_pa, start_pa, high_base, perms, attr)
}

/// Như [`map_high_alias`] nhưng **gốc alias** là `origin_pa` riêng:
/// `VA = high_base + (PA - origin_pa)`.
///
/// **Vì sao cần** (KT-277): W^X tách vùng ảnh thành nhiều khoảng (`.text` RX, `.data`/`.bss`/
/// stack RW). Mọi khoảng phải dùng **cùng** một gốc alias (`__text_start`) — nếu mỗi khoảng lấy
/// đầu của chính nó làm gốc thì alias của `.data` lệch đúng bằng kích thước vùng RX và kernel
/// nhảy vào giữa `.data` sau khi đổi `TTBR1`. Tách tham số gốc thay vì chép vòng lặp map.
pub fn map_high_alias_from<M: TableMemory>(
    table: &mut PageTable<Aarch64, M>,
    start_pa: u64,
    end_pa: u64,
    origin_pa: u64,
    high_base: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    if end_pa <= start_pa {
        return Ok(0);
    }
    if start_pa & 0xFFF != 0 || origin_pa & 0xFFF != 0 {
        return Err(MapError::Unaligned);
    }
    let last = (end_pa + 0xFFF) & !0xFFF;
    let mut pages = 0;
    let mut pa = start_pa;
    while pa < last {
        table.map_with(
            high_alias_va(pa, origin_pa, high_base),
            pa,
            perms,
            attr,
            Shareability::Inner,
        )?;
        pages += 1;
        pa += 0x1000;
    }
    Ok(pages)
}

/// Map cửa sổ physmap `[start_pa, end_pa)` vào bảng `table` (phải là bảng **nửa cao**).
///
/// Hai đầu được làm tròn **ra ngoài** (map đủ trang chứa chúng), giống
/// [`map_identity`](crate::boot_mmu::map_identity). Trả **số trang** đã map; khoảng rỗng ⇒ `Ok(0)`.
///
/// Lỗi của `PageTable::map_with` được trả nguyên (không nuốt): `AlreadyMapped` khi map chồng,
/// `OutOfFrames` khi arena cạn, `PaOutOfRange` khi PA vượt 48 bit — tất cả phải làm boot
/// **PANIC**, không im lặng bỏ trang.
pub fn map_physmap<M: TableMemory>(
    table: &mut PageTable<Aarch64, M>,
    start_pa: u64,
    end_pa: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    if end_pa <= start_pa {
        return Ok(0);
    }
    let first = start_pa & !0xFFF;
    let last = (end_pa + 0xFFF) & !0xFFF;
    let mut pages = 0;
    let mut pa = first;
    while pa < last {
        if pa >> crate::format::AARCH64_PA_BITS != 0 {
            return Err(MapError::PaOutOfRange);
        }
        table.map_with(physmap_va(pa), pa, perms, attr, Shareability::Inner)?;
        pages += 1;
        pa += 0x1000;
    }
    Ok(pages)
}

/// `Mmu` **định tuyến theo VA**: hai [`PageTable`] (nửa thấp + nửa cao) sau **một** giao diện.
///
/// Mọi thao tác chọn bảng bằng [`half_of`] rồi gọi thẳng `PageTable` — **không** có bản sao thứ
/// hai của walk/map/unmap (đó là điều D2-199 muốn tránh). VA không canonical ⇒
/// [`MapError::NotCanonical`] ở **mọi** thao tác, không panic.
///
/// # `root()` trả gốc **nửa thấp**
///
/// Trait [`Mmu`] chỉ có **một** `root()`; với hai gốc, `root()` trả gốc `TTBR0` (nửa thấp) và
/// [`SplitMmu::root_high`] trả gốc `TTBR1`. Đây là chỗ **duy nhất** giao diện một-gốc không diễn
/// tả đủ hai-gốc — ghi rõ thay vì đổi trait (D2-199).
pub struct SplitMmu<M: TableMemory> {
    low: PageTable<Aarch64, M>,
    high: PageTable<Aarch64, M>,
}

impl<M: TableMemory> SplitMmu<M> {
    /// Dựng **hai** bảng trang rỗng trên hai nguồn frame (thường là hai handle **chia sẻ** cùng
    /// một arena — xem `bootmem::shared_boot_memory`).
    pub fn new(low_memory: M, high_memory: M) -> Result<Self, MapError> {
        Ok(Self {
            low: PageTable::new(low_memory)?,
            high: PageTable::new(high_memory)?,
        })
    }

    /// Bảng nửa thấp (`TTBR0_EL1`).
    pub const fn low(&self) -> &PageTable<Aarch64, M> {
        &self.low
    }

    /// Bảng nửa cao (`TTBR1_EL1`).
    pub const fn high(&self) -> &PageTable<Aarch64, M> {
        &self.high
    }

    /// Bảng nửa thấp (ghi) — dùng cho chẩn đoán/đường đặc biệt.
    pub fn low_mut(&mut self) -> &mut PageTable<Aarch64, M> {
        &mut self.low
    }

    /// Bảng nửa cao (ghi) — dùng cho chẩn đoán/đường đặc biệt.
    pub fn high_mut(&mut self) -> &mut PageTable<Aarch64, M> {
        &mut self.high
    }

    /// PA gốc bảng **nửa thấp** (cũng là giá trị `Mmu::root` trả).
    pub const fn root_low(&self) -> u64 {
        self.low.root()
    }

    /// PA gốc bảng **nửa cao**.
    pub const fn root_high(&self) -> u64 {
        self.high.root()
    }

    /// PA bảng **mức 1** mà `TTBR0_EL1` phải trỏ tới (walk bắt đầu ở mức 1 với `T0SZ = 25`).
    pub fn ttbr0_base(&self) -> Option<u64> {
        crate::boot_mmu::level1_base(&self.low, 0)
    }

    /// PA bảng **mức 1** mà `TTBR1_EL1` phải trỏ tới (cùng mức bắt đầu, `T1SZ = 25`).
    pub fn ttbr1_base(&self) -> Option<u64> {
        crate::boot_mmu::level1_base(&self.high, HIGH_HALF_BASE)
    }

    /// Map 4 KiB `va → pa` với thuộc tính mặc định (Normal, Inner shareable), định tuyến theo VA.
    pub fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        self.map_with(va, pa, perms, MemAttr::Normal, Shareability::Inner)
    }

    /// Map 4 KiB `va → pa` với thuộc tính/shareability chỉ định, định tuyến theo VA.
    pub fn map_with(
        &mut self,
        va: u64,
        pa: u64,
        perms: Perms,
        attr: MemAttr,
        share: Shareability,
    ) -> Result<(), MapError> {
        match half_of(va) {
            Some(Half::Low) => self.low.map_with(va, pa, perms, attr, share),
            Some(Half::High) => self.high.map_with(va, pa, perms, attr, share),
            None => Err(MapError::NotCanonical),
        }
    }

    /// Bỏ mapping 4 KiB tại `va`, trả PA; định tuyến theo VA.
    pub fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        match half_of(va) {
            Some(Half::Low) => self.low.unmap(va),
            Some(Half::High) => self.high.unmap(va),
            None => Err(MapError::NotCanonical),
        }
    }

    /// Dịch `va → pa`; định tuyến theo VA.
    pub fn translate(&self, va: u64) -> Result<Translation, MapError> {
        match half_of(va) {
            Some(Half::Low) => self.low.translate(va),
            Some(Half::High) => self.high.translate(va),
            None => Err(MapError::NotCanonical),
        }
    }

    /// Walk chẩn đoán; định tuyến theo VA (VA không canonical ⇒ walk rỗng).
    pub fn walk(&self, va: u64) -> Walk {
        match half_of(va) {
            Some(Half::Low) => self.low.walk(va),
            Some(Half::High) => self.high.walk(va),
            None => Walk::empty(),
        }
    }

    /// Map cửa sổ physmap `[start_pa, end_pa)` vào bảng **nửa cao**; trả số trang.
    pub fn map_physmap(
        &mut self,
        start_pa: u64,
        end_pa: u64,
        perms: Perms,
        attr: MemAttr,
    ) -> Result<usize, MapError> {
        map_physmap(&mut self.high, start_pa, end_pa, perms, attr)
    }

    /// Map alias nửa cao của `[start_pa, end_pa)` vào bảng **nửa cao**; trả số trang.
    pub fn map_high_alias(
        &mut self,
        start_pa: u64,
        end_pa: u64,
        high_base: u64,
        perms: Perms,
        attr: MemAttr,
    ) -> Result<usize, MapError> {
        map_high_alias(&mut self.high, start_pa, end_pa, high_base, perms, attr)
    }
}

impl<M: TableMemory> Mmu for SplitMmu<M> {
    type Fmt = Aarch64;

    /// Gốc **nửa thấp** — xem ghi chú ở [`SplitMmu`] (trait chỉ có một `root()`).
    fn root(&self) -> u64 {
        self.low.root()
    }

    fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        SplitMmu::map(self, va, pa, perms)
    }

    fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        SplitMmu::unmap(self, va)
    }

    fn translate(&self, va: u64) -> Result<Translation, MapError> {
        SplitMmu::translate(self, va)
    }
}
