// SPDX-License-Identifier: MIT OR Apache-2.0
//! `PageTable` của K2 — cây **3 cấp** map/unmap Frame, model-only `[TK]` (KT-37; D2-65
//! Q2/Q3/Q7; `v2/docs/k2-vm.md` §11; thiết kế §4.10 `:353-366`).
//!
//! **K2 không phải kernel thật**: đây là **mô hình** page table trong sim — không PTE phần
//! cứng, không MMU/TLB, không `sfence.vma`; walker chỉ đọc mảng PTE trong bộ nhớ của mô hình.
//! VSpace/ASID/TLB là KT-38/KT-40, luật W^X là KT-39, VM fault là KT-41.
//!
//! # Hình 3 cấp (Sv39, VA 39 bit)
//!
//! | Cấp | Chỉ số | Lá cho Frame | Bảng phủ |
//! |---|---|---|---|
//! | 2 (root) | `va[38:30]` | 1 GiB | toàn bộ VA 39 bit |
//! | 1 | `va[29:21]` | 2 MiB | 1 GiB |
//! | 0 | `va[20:12]` | 4 KiB | 2 MiB |
//!
//! Chỉ số lấy từ [`vifast_pte::walk_shape`] (**không** chép tay `9/9/9/12`); cỡ Frame lấy từ
//! [`vifast_pte::FrameSize`]; biên vòng lặp là [`vifast_pte::MAX_MAP_DEPTH`]. Một bảng có
//! [`PT_ENTRY_COUNT`] = 512 PTE × 8 B = 4096 B (một trang).
//!
//! # PTE của **mô hình** (layout do KT-37 chốt)
//!
//! [`vifast_pte::Pte`] là `u64` thô; KT-37 gán nghĩa bit như sau:
//!
//! ```text
//! bit 0     : valid (bắt buộc với entry có dùng)
//! bit 1     : 1 = bảng con (Table), 0 = lá (Leaf)
//! bit 2..=3 : rights của lá: bit 2 = READ, bit 3 = WRITE
//! bit 4..=9 : attrs của lá: WB, WC, UC, Device, NX, EXEC (mỗi thuộc tính một bit)
//! bit 10..41: ObjId (u32) của Frame (lá) hoặc PageTable con (bảng)
//! ```
//!
//! Vì đây là mô hình, PTE **không** mang `pa` — `pa` đọc từ `FrameMeta` (KT-36) lúc walk;
//! một PTE thật của K3 sẽ mang PPN. Quy ước này ghi rõ để K3 không đọc nhầm `Pte` của mô hình.
//!
//! # Quyền/attrs **của từng mapping**
//!
//! [`MapRights`] (READ/WRITE) và [`MapAttrs`] (WB/WC/UC/Device/NX/EXEC) do **tham số** của
//! `map` quyết định, **không** suy từ quyền của cap Frame (K2-PLAN KT-37: "lưu rights/attrs
//! của từng mapping (không suy từ cap)"). KT-37 lưu nguyên giá trị; **KT-39** mới áp luật
//! ([`validate_attrs`]): đúng **một** kiểu bộ nhớ, `NX` ⊥ `EXEC`, `Device + EXEC` ⇒
//! `WxViolation` — đó là ngữ nghĩa VM, không phải của cây PageTable thuần.

use crate::kobj::KError;
use vifast_pte::{walk_shape, FrameSize, Pte, PteArray, VA_BITS};

/// Số PTE của một bảng: `2^9 = 512` (một trang 4096 B; hình `9/9/9/12`).
pub const PT_ENTRY_COUNT: usize = 512;

/// Cấp của bảng gốc (root): **2** — lá ở cấp này phủ 1 GiB.
pub const PT_ROOT_LEVEL: u8 = 2;

/// Số byte của một bảng PageTable: `512 × 8 = 4096` — cỡ object `PageTable` của mô hình.
pub const PT_BYTES: usize = PT_ENTRY_COUNT * core::mem::size_of::<Pte>();

/// Giới hạn VA của hình: `1 << VA_BITS` = `1 << 39`; VA `>=` giá trị này ⇒ `RangeError`.
pub const VA_LIMIT: u64 = 1u64 << VA_BITS;

/// Bit valid của PTE mô hình.
pub(crate) const PTE_VALID: u64 = 1 << 0;
/// Bit "entry là bảng con" (không phải lá).
pub(crate) const PTE_TABLE: u64 = 1 << 1;
/// Vị trí bit rights của lá (2 bit).
const PTE_RIGHTS_SHIFT: u32 = 2;
/// Vị trí bit attrs của lá (6 bit).
const PTE_ATTRS_SHIFT: u32 = 4;
/// Vị trí bit ObjId (Frame hoặc PageTable con).
const PTE_OBJ_SHIFT: u32 = 10;
/// Mặt nạ rights/attrs trong PTE.
const PTE_RIGHTS_MASK: u8 = 0b11;
const PTE_ATTRS_MASK: u8 = 0b11_1111;

/// Quyền của một mapping: `READ` / `WRITE` (K2 **không** có `EXEC` trong quyền cap — QD §4
/// `:142`; `EXEC` là **attr** lúc map, xem [`MapAttrs::EXEC`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MapRights(u8);

impl MapRights {
    /// Không quyền nào.
    pub const NONE: Self = Self(0);
    /// Đọc được.
    pub const READ: Self = Self(1 << 0);
    /// Ghi được.
    pub const WRITE: Self = Self(1 << 1);

    /// Bit thô (2 bit thấp).
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Bỏ bit lạ (chỉ giữ READ/WRITE).
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & PTE_RIGHTS_MASK)
    }

    /// `true` ⇔ có **đủ** các bit của `other`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Hợp hai tập quyền.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Không có quyền nào.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl core::ops::BitOr for MapRights {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl core::fmt::Debug for MapRights {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_empty() {
            return f.write_str("MapRights(NONE)");
        }
        f.write_str("MapRights(")?;
        let mut sep = "";
        if self.contains(Self::READ) {
            f.write_str("READ")?;
            sep = "|";
        }
        if self.contains(Self::WRITE) {
            write!(f, "{sep}WRITE")?;
        }
        f.write_str(")")
    }
}

/// Thuộc tính của một mapping: `WB/WC/UC/Device` (kiểu bộ nhớ) + `NX/EXEC` (thực thi).
///
/// KT-37 lưu/đọc lại **đúng** giá trị người gọi truyền. **KT-39** áp ràng buộc ngữ nghĩa VM ở
/// [`validate_attrs`] (một kiểu bộ nhớ, `NX`⊥`EXEC`, `Device + EXEC` ⇒ `WxViolation`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MapAttrs(u8);

impl MapAttrs {
    /// Không thuộc tính nào.
    pub const NONE: Self = Self(0);
    /// Write-back (mặc định).
    pub const WB: Self = Self(1 << 0);
    /// Write-combining.
    pub const WC: Self = Self(1 << 1);
    /// Uncached.
    pub const UC: Self = Self(1 << 2);
    /// Device.
    pub const DEVICE: Self = Self(1 << 3);
    /// Không thực thi.
    pub const NX: Self = Self(1 << 4);
    /// Thực thi (thuộc tính lúc map; **không** phải quyền cap).
    pub const EXEC: Self = Self(1 << 5);

    /// Bit thô (6 bit thấp).
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Bỏ bit lạ (chỉ giữ 6 thuộc tính của K2).
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & PTE_ATTRS_MASK)
    }

    /// `true` ⇔ có **đủ** các bit của `other`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Hợp hai tập thuộc tính.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Không có thuộc tính nào.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl core::ops::BitOr for MapAttrs {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl core::fmt::Debug for MapAttrs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_empty() {
            return f.write_str("MapAttrs(NONE)");
        }
        let mut first = true;
        f.write_str("MapAttrs(")?;
        for (bit, name) in [
            (Self::WB, "WB"),
            (Self::WC, "WC"),
            (Self::UC, "UC"),
            (Self::DEVICE, "Device"),
            (Self::NX, "NX"),
            (Self::EXEC, "EXEC"),
        ] {
            if self.contains(bit) {
                if !first {
                    f.write_str("|")?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        f.write_str(")")
    }
}

/// Một mapping **sống** của cây: lá ở `va` (đã căn theo `size_bits`) → `pa` của Frame.
///
/// `rights`/`attrs` là giá trị **lưu lúc map**, không suy lại từ cap (KT-37 DoD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    /// Địa chỉ ảo **gốc** của mapping (đã căn `1 << size_bits`).
    pub va: u64,
    /// Địa chỉ vật lý của mô hình (= `FrameMeta.pa`).
    pub pa: u64,
    /// `12` / `21` / `30` — cỡ Frame, cũng là cấp lá `(size_bits - 12) / 9`.
    pub size_bits: u8,
    /// Frame được map (ObjId = u32 của mô hình).
    pub frame: u32,
    /// Quyền READ/WRITE lưu lúc map.
    pub rights: MapRights,
    /// Attrs WB/WC/UC/Device/NX/EXEC lưu lúc map.
    pub attrs: MapAttrs,
}

/// Ảnh chụp một `PageTable` object cho test/harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableInfo {
    /// Cấp của bảng: 2 = root, 1, 0.
    pub level: u8,
    /// `Some((bảng cha, chỉ số))` nếu đã attach; `None` nếu là root chưa gắn.
    pub attached: Option<(u32, u16)>,
    /// Số entry **khác rỗng** trong bảng này.
    pub entries: u16,
    /// Số lá **sống** trong cả cây con của bảng này (gồm chính nó).
    pub mapped: u32,
}

/// Dữ liệu của một `PageTable` object — **model-only**, nằm trong `Machine`, không phải object
/// body của objmem (body chỉ là mốc [`crate::kobj::Kind::PageTable`], giống Frame).
#[derive(Debug)]
pub(crate) struct PageTableData {
    /// Cấp: 2 = root, 1, 0. Bảng mới tạo có `level = 2` (chưa gắn thì là root).
    pub(crate) level: u8,
    /// `(bảng cha, chỉ số trong bảng cha)` khi đã attach; root ⇒ `None`.
    pub(crate) attached: Option<(u32, u16)>,
    /// 512 PTE của bảng (layout ở doc module).
    pub(crate) entries: PteArray<PT_ENTRY_COUNT>,
    /// Số lá sống trong **cả cây con** (bảng này + các bảng con) — đếm để kiểm
    /// `MAX_FRAMES_PER_VSPACE` và `check_pages`; không phải nguồn sự thật duy nhất.
    pub(crate) mapped: u32,
}

impl PageTableData {
    /// Bảng mới: cấp 2, chưa attach, rỗng.
    pub(crate) fn new() -> Self {
        Self {
            level: PT_ROOT_LEVEL,
            attached: None,
            entries: PteArray::new(),
            mapped: 0,
        }
    }

    /// Số entry khác rỗng (0..=512).
    pub(crate) fn entry_count(&self) -> u16 {
        self.entries.iter().filter(|p| !p.is_empty()).count() as u16
    }

    /// Bảng **tinh khiết**: chưa attach, `mapped == 0`, mọi entry rỗng — điều kiện để finalise
    /// (nếu không ⇒ `HasChildren`, không im lặng bỏ mapping/bảng con).
    pub(crate) fn is_pristine(&self) -> bool {
        self.attached.is_none() && self.mapped == 0 && self.entries.iter().all(|p| p.is_empty())
    }
}

/// Mặt nạ bit **kiểu bộ nhớ** trong [`MapAttrs`]: `WB|WC|UC|Device`.
const MEMORY_TYPE_MASK: u8 = 0b1111;

/// **KT-39** — luật `attrs` của **một mapping VM** (D2-67; `k2-vm.md` §13):
///
/// 1. `WB/WC/UC/Device` phải **đúng một** giá trị (0 hoặc > 1 ⇒ [`KError::InvalidArgument`]);
/// 2. `NX` và `EXEC` **xung khắc** — cùng lúc ⇒ [`KError::InvalidArgument`] (không có = mặc
///    định không thực thi, giữ tương thích KT-37/38 với `MapAttrs::WB`);
/// 3. `Device + EXEC` ⇒ [`KError::WxViolation`] — thiết bị không bao giờ thực thi; D2-67 chốt
///    dùng chung mã `WxViolation` cho mọi vi phạm W^X (nhất quán "Device không W+X").
///
/// Kiểm **trước** mọi thay đổi cây/bộ đếm (atomic). Không kiểm quyền `READ/WRITE` ở đây —
/// luật W^X theo bộ đếm do [`crate::kobj::Machine::frame_note_map`] áp.
pub(crate) fn validate_attrs(attrs: MapAttrs) -> Result<(), KError> {
    if (attrs.bits() & MEMORY_TYPE_MASK).count_ones() != 1 {
        return Err(KError::InvalidArgument);
    }
    let nx = attrs.contains(MapAttrs::NX);
    let exec = attrs.contains(MapAttrs::EXEC);
    if nx && exec {
        return Err(KError::InvalidArgument);
    }
    if attrs.contains(MapAttrs::DEVICE) && exec {
        return Err(KError::WxViolation);
    }
    Ok(())
}

/// Cấp lá tương ứng `size_bits` của Frame: `12 → 0`, `21 → 1`, `30 → 2`; cỡ khác ⇒ `None`.
pub(crate) fn level_for_size_bits(size_bits: u8) -> Option<u8> {
    let size = FrameSize::from_size_bits(size_bits).ok()?;
    Some((size.size_bits() - 12) / 9)
}

/// `size_bits` của lá ở `level`: `0 → 12`, `1 → 21`, `2 → 30`; cấp khác ⇒ `None`.
pub(crate) const fn size_bits_for_level(level: u8) -> Option<u8> {
    match level {
        0 => Some(12),
        1 => Some(21),
        2 => Some(30),
        _ => None,
    }
}

/// Vùng VA (byte) do **bảng** ở `level` phủ: cấp 0 phủ 2 MiB, cấp 1 phủ 1 GiB, cấp 2 phủ 2^39.
///
/// Dùng để kiểm `va` căn lề khi `attach`/`map` (2 MiB/1 GiB "phải phủ đúng một entry lá,
/// không cắt ngang" — Q2).
pub(crate) const fn table_span(level: u8) -> Option<u64> {
    match level {
        0 => Some(1u64 << 21),
        1 => Some(1u64 << 30),
        2 => Some(1u64 << 39),
        _ => None,
    }
}

/// Chỉ số entry của `va` trong bảng ở `level` (2 = root, 0 = lá), từ `walk_shape`.
pub(crate) fn index_in_level(va: u64, level: u8) -> Option<usize> {
    let shape = walk_shape(va);
    match level {
        2 => Some(shape.level2() as usize),
        1 => Some(shape.level1() as usize),
        0 => Some(shape.level0() as usize),
        _ => None,
    }
}

/// Mã hoá PTE **lá** (Frame + rights + attrs) theo layout ở doc module.
pub(crate) fn leaf_pte(frame: u32, rights: MapRights, attrs: MapAttrs) -> Pte {
    Pte::from_raw(
        PTE_VALID
            | (((rights.bits() & PTE_RIGHTS_MASK) as u64) << PTE_RIGHTS_SHIFT)
            | (((attrs.bits() & PTE_ATTRS_MASK) as u64) << PTE_ATTRS_SHIFT)
            | ((frame as u64) << PTE_OBJ_SHIFT),
    )
}

/// Mã hoá PTE **bảng con** (PageTable con).
pub(crate) fn table_pte(child: u32) -> Pte {
    Pte::from_raw(PTE_VALID | PTE_TABLE | ((child as u64) << PTE_OBJ_SHIFT))
}

/// PTE đã giải mã.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodedPte {
    /// Entry rỗng (bit valid = 0), không dùng.
    Empty,
    /// Bảng con.
    Table(u32),
    /// Lá (Frame + rights/attrs lưu lúc map).
    Leaf {
        frame: u32,
        rights: MapRights,
        attrs: MapAttrs,
    },
}

/// Giải mã PTE; bit valid = 0 ⇒ [`DecodedPte::Empty`] (kể cả raw khác 0 — `check_pages` coi
/// entry "không valid" là rỗng, không có đường nào của KT-37 ghi raw như vậy).
pub(crate) fn decode_pte(pte: Pte) -> DecodedPte {
    let raw = pte.raw();
    if raw & PTE_VALID == 0 {
        return DecodedPte::Empty;
    }
    if raw & PTE_TABLE != 0 {
        return DecodedPte::Table((raw >> PTE_OBJ_SHIFT) as u32);
    }
    DecodedPte::Leaf {
        frame: (raw >> PTE_OBJ_SHIFT) as u32,
        rights: MapRights::from_bits_truncate(((raw >> PTE_RIGHTS_SHIFT) & 0x3) as u8),
        attrs: MapAttrs::from_bits_truncate(((raw >> PTE_ATTRS_SHIFT) & 0x3f) as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vifast_pte::MAX_FRAMES_PER_VSPACE;

    #[test]
    fn hang_so_va_hinh_ba_cap() {
        assert_eq!(PT_ENTRY_COUNT, 512);
        assert_eq!(PT_BYTES, 4096);
        assert_eq!(PT_ROOT_LEVEL, 2);
        assert_eq!(VA_LIMIT, 1u64 << 39);
        assert_eq!(size_bits_for_level(0), Some(12));
        assert_eq!(size_bits_for_level(1), Some(21));
        assert_eq!(size_bits_for_level(2), Some(30));
        assert_eq!(size_bits_for_level(3), None);
        assert_eq!(level_for_size_bits(12), Some(0));
        assert_eq!(level_for_size_bits(21), Some(1));
        assert_eq!(level_for_size_bits(30), Some(2));
        assert_eq!(level_for_size_bits(13), None);
        assert_eq!(table_span(0), Some(1 << 21));
        assert_eq!(table_span(1), Some(1 << 30));
        assert_eq!(table_span(2), Some(1 << 39));
        assert_eq!(table_span(3), None);
        assert_eq!(index_in_level(0, 2), Some(0));
        assert_eq!(index_in_level(1 << 21, 1), Some(1));
        assert_eq!(index_in_level(1 << 39, 2), Some(0), "bit trên VA bị cắt");
        assert!(MAX_FRAMES_PER_VSPACE.is_power_of_two());
    }

    #[test]
    fn pte_lai_roundtrip_moi_to_hop() {
        let mut n = 0u32;
        for &frame in &[1u32, 7, 4096, u32::MAX] {
            for rbits in 0..=3u8 {
                for abits in 0..=0x3fu8 {
                    let rights = MapRights::from_bits_truncate(rbits);
                    let attrs = MapAttrs::from_bits_truncate(abits);
                    let pte = leaf_pte(frame, rights, attrs);
                    assert_eq!(
                        decode_pte(pte),
                        DecodedPte::Leaf {
                            frame,
                            rights,
                            attrs
                        }
                    );
                    n += 1;
                }
            }
            let pte = table_pte(frame);
            assert_eq!(decode_pte(pte), DecodedPte::Table(frame));
        }
        assert_eq!(decode_pte(Pte::EMPTY), DecodedPte::Empty);
        assert!(n > 0);
    }

    #[test]
    fn quyen_attrs_bit_va_contains() {
        assert_eq!(MapRights::READ.bits(), 1);
        assert_eq!(MapRights::WRITE.bits(), 2);
        assert_eq!((MapRights::READ | MapRights::WRITE).bits(), 3);
        assert!(MapRights::READ.contains(MapRights::READ));
        assert!(!MapRights::READ.contains(MapRights::WRITE));
        assert!(MapRights::from_bits_truncate(0xff).bits() <= 3);
        assert!((MapAttrs::WB | MapAttrs::NX).contains(MapAttrs::WB));
        assert_eq!(MapAttrs::from_bits_truncate(0xff).bits(), 0x3f);
        assert_eq!(PT_BYTES, PT_ENTRY_COUNT * 8);
    }

    #[test]
    fn page_table_data_moi_la_root_tinh_khiet() {
        let d = PageTableData::new();
        assert_eq!(d.level, 2);
        assert_eq!(d.attached, None);
        assert_eq!(d.entry_count(), 0);
        assert_eq!(d.mapped, 0);
        assert!(d.is_pristine());
    }
}
