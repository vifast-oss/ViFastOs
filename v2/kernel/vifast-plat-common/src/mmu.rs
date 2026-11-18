// SPDX-License-Identifier: MIT OR Apache-2.0
//! Dựng **page table Sv39 thật** trên một vùng RAM (arena) — KT-46.
//!
//! Tệp này là phần **logic thuần, an toàn** của MMU (chạy được trên host, D2-72 Q12):
//! biểu diễn PTE Sv39, `PageTableBuilder` cấp bảng 4 KiB từ một `&mut [u8]` (arena RAM, **không**
//! mảng tĩnh), `map` 4 KiB / 2 MiB / 1 GiB, và `walk` phần mềm dùng chung với phần cứng để đối
//! chiếu. Phần asm/CSR/MMIO (`csrw satp`, `sfence.vma`, `satp` readback, cast arena) nằm ở
//! `kernel/vifast-plat-riscv64` (D2-17); ở đây **không** có `unsafe`
//! ([`#![forbid(unsafe_code)]`] của crate).
//!
//! Hằng số hình học lấy từ [`vifast_pte`] (`VA_BITS = 39`, `PTE_LEVELS = 3`,
//! `PAGE_SIZE_BITS = [12, 21, 30]`, `walk_shape`) — không chép tay lần thứ hai. Bảng trang là
//! **dữ liệu trong RAM**, mỗi bảng 512 × 8 B = 4 KiB, căn lề 4 KiB.
//!
//! # Bit PTE Sv39 (RISC-V privileged spec)
//!
//! ```text
//!  63       61 60 54        10 9 8 7 6 5 4 3 2 1 0
//! [ N | PBMT | reserved | PPN | RSW | D A G U X W R V ]
//! ```
//!
//! KT-46 dùng `V/R/W/X/U/G/A/D`; đặt sẵn `A|D` cho leaf để không phụ thuộc hardware A/D update
//! (QEMU `-cpu rv64`), và **không** đặt `U` (kernel-only; U-mode là KT-53). `X=0` là NX: physmap
//! và stack/trap/arena đều NX; W^X đầy đủ (tách text/rodata) là KT-56.
//!
//! # Phạm vi
//!
//! KT-46 chỉ dựng bảng + map + walk; `satp`/`sfence.vma`/TLB/ASID/shootdown (asm) nằm ở
//! `kernel/vifast-plat-riscv64` (D2-17; ASID thật KT-51); không cấp phát frame động (KT-52),
//! không U-mode (KT-53).

use vifast_pte::{
    walk_shape, Descriptor, MemAttr, Mmu, PageTableFormat, Perms, Pte, Shareability, Translation,
    PAGE_SIZE_BITS, PTE_LEVELS, VA_BITS,
};

// KT-164: `MapError` là kiểu **dùng chung** ở `vifast-pte` (thiết kế §358/§1101) — cùng một enum
// với `vifast-plat-aarch64`, nên tầng trên và test generic so lỗi mà không phải biết định dạng.
// Sv39 dùng tập con `OutOfArena`/`LevelOutOfRange`/`InvalidFlags`/...; các biến thể của AArch64
// (`OutOfFrames`, `PaOutOfRange`, `InvalidPerms`, `NotMapped`, `BlockUnsupported`) vẫn có mặt để
// hai định dạng dùng chung một kiểu lỗi.
pub use vifast_pte::MapError;

/// Cỡ một trang/bảng cơ sở: 4 KiB.
pub const PAGE_BYTES: u64 = 1 << 12;

/// Số entry một bảng trang (`2^9 = 512`).
pub const PTE_ENTRIES: usize = 1 << 9;

/// Cỡ một entry PTE theo byte.
pub const PTE_BYTES: usize = 8;

/// Bit `V` — entry hợp lệ.
pub const PTE_VALID: u64 = 1 << 0;
/// Bit `R` — đọc được.
pub const PTE_READ: u64 = 1 << 1;
/// Bit `W` — ghi được (RISC-V cấm `W=1, R=0`).
pub const PTE_WRITE: u64 = 1 << 2;
/// Bit `X` — thực thi được.
pub const PTE_EXEC: u64 = 1 << 3;
/// Bit `U` — U-mode truy cập được (KT-46 **không** dùng; U-mode là KT-53).
pub const PTE_USER: u64 = 1 << 4;
/// Bit `G` — global.
pub const PTE_GLOBAL: u64 = 1 << 5;
/// Bit `A` — đã được truy cập.
pub const PTE_ACCESSED: u64 = 1 << 6;
/// Bit `D` — đã được ghi.
pub const PTE_DIRTY: u64 = 1 << 7;

/// Vị trí PPN trong PTE.
pub const PTE_PPN_SHIFT: u32 = 10;

/// `satp.MODE` cho Sv39 (RISC-V privileged spec).
pub const SATP_MODE_SV39: u64 = 8;
/// Vị trí trường `MODE` trong `satp`.
pub const SATP_MODE_SHIFT: u32 = 60;
/// Số bit trường `ASID` trong `satp` (RV64: 16 bit, bit 59..44).
pub const SATP_ASID_BITS: u32 = 16;
/// Số bit trường `PPN` trong `satp` (RV64: 44 bit, bit 43..0).
pub const SATP_PPN_BITS: u32 = 44;

/// `satp` cho Sv39 với `root_phys` (ASID = 0) — KT-46 chưa dùng ASID thật (KT-51).
pub const fn satp_sv39(root_phys: u64) -> u64 {
    (SATP_MODE_SV39 << SATP_MODE_SHIFT) | ((root_phys >> 12) & ((1u64 << SATP_PPN_BITS) - 1))
}

/// `satp` cho Sv39 với `root_phys` **và ASID 8 bit** của kernel (`crate::tlb`) — KT-51.
///
/// Trường `ASID` của `satp` rộng 16 bit (bit 59..44, privileged spec); kernel K3 chỉ dùng **8
/// bit** (D2-65 Q4) nên 8 bit cao của trường luôn 0. Hàm **thuần** nên host test chốt được
/// `MODE`/`PPN` không bị thay khi đổi ASID.
pub const fn satp_sv39_asid(root_phys: u64, asid: u16) -> u64 {
    satp_sv39(root_phys) | (((asid as u64) & crate::tlb::ASID_MASK) << crate::tlb::SATP_ASID_SHIFT)
}

/// Trường `ASID` (8 bit kernel dùng) của một giá trị `satp`.
pub const fn satp_asid(satp: u64) -> u16 {
    ((satp >> crate::tlb::SATP_ASID_SHIFT) & crate::tlb::ASID_MASK) as u16
}

/// Trường `MODE` của một giá trị `satp`.
pub const fn satp_mode(satp: u64) -> u64 {
    satp >> SATP_MODE_SHIFT
}

/// PPN (địa chỉ bảng gốc, đã nhân 4 KiB) từ một giá trị `satp`.
pub const fn satp_root(satp: u64) -> u64 {
    (satp & ((1u64 << SATP_PPN_BITS) - 1)) << 12
}

/// Bit cờ của một PTE leaf/table — newtype để không lẫn với địa chỉ vật lý.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PteFlags(u64);

impl PteFlags {
    /// Không cờ nào.
    pub const EMPTY: Self = Self(0);
    /// Chỉ `V` — entry bảng (non-leaf).
    pub const TABLE: Self = Self(PTE_VALID);

    /// Bọc bit thô (dùng cho test/đọc PTE).
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Bit thô.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// `true` ⇔ có bit `V`.
    pub const fn is_valid(self) -> bool {
        self.0 & PTE_VALID != 0
    }

    /// `true` ⇔ có ít nhất một quyền `R/W/X` (leaf).
    pub const fn is_leaf(self) -> bool {
        self.0 & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0
    }

    /// `true` ⇔ `R`.
    pub const fn is_readable(self) -> bool {
        self.0 & PTE_READ != 0
    }

    /// `true` ⇔ `W`.
    pub const fn is_writable(self) -> bool {
        self.0 & PTE_WRITE != 0
    }

    /// `true` ⇔ `X`.
    pub const fn is_executable(self) -> bool {
        self.0 & PTE_EXEC != 0
    }

    /// Leaf kernel: `V|R|W|X|A|D` tuỳ quyền; luôn đặt `A|D`.
    pub const fn leaf(read: bool, write: bool, exec: bool) -> Self {
        let mut bits = PTE_VALID | PTE_ACCESSED | PTE_DIRTY;
        if read {
            bits |= PTE_READ;
        }
        if write {
            bits |= PTE_WRITE;
        }
        if exec {
            bits |= PTE_EXEC;
        }
        Self(bits)
    }

    /// Kernel image RWX (W^X đầy đủ là KT-56): `V|R|W|X|A|D`.
    pub const fn kernel_rwx() -> Self {
        Self::leaf(true, true, true)
    }

    /// Kernel data/stack/arena/physmap: `V|R|W|A|D`, **NX**.
    pub const fn kernel_rw_nx() -> Self {
        Self::leaf(true, true, false)
    }

    /// MMIO Device 4 KiB: `V|R|W|A|D`, **NX**.
    pub const fn device_rw() -> Self {
        Self::leaf(true, true, false)
    }
}

/// Kết quả `walk` phần mềm: dừng ở entry invalid, leaf, hay bảng non-leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walk {
    /// Cấp dừng lại (`2` gốc, `1` giữa, `0` lá) — với leaf là cấp của leaf.
    pub level: usize,
    /// Entry PTE tại cấp dừng (0 nếu chưa có).
    pub pte: Pte,
    /// `true` ⇔ entry có bit `V` (bảng hoặc leaf).
    pub valid: bool,
    /// `true` ⇔ entry là leaf (có `R/W/X`).
    pub leaf: bool,
    /// Địa chỉ vật lý **đáy trang** của leaf (0 nếu invalid/non-leaf).
    pub phys: u64,
}

impl Walk {
    /// Kết quả invalid tại `level`.
    pub const fn invalid(level: usize) -> Self {
        Self {
            level,
            pte: Pte::EMPTY,
            valid: false,
            leaf: false,
            phys: 0,
        }
    }

    /// Bit thô của entry (tiện log).
    pub fn raw(self) -> u64 {
        self.pte.raw()
    }
}

/// Nguồn đọc PTE — tách phần "đọc bộ nhớ" khỏi hình walk để test host.
pub trait PteMemory {
    /// Đọc entry `index` của bảng ở địa chỉ vật lý `table_phys`; `None` nếu bảng không đọc được.
    fn load(&self, table_phys: u64, index: usize) -> Option<u64>;
}

/// VA có canonical theo `VA_BITS` không (bit `VA_BITS-1` sao chép vào các bit cao hơn).
///
/// Sv39: `bit38 == bit39..63` (tất cả 1 = nửa cao, tất cả 0 = nửa thấp). Hàm **const**, không
/// panic với mọi `u64`.
pub const fn is_canonical(va: u64) -> bool {
    let sign = (va >> (VA_BITS as u32 - 1)) & 1;
    let upper = va >> VA_BITS;
    if sign == 1 {
        upper == (1u64 << (64 - VA_BITS as u32)) - 1
    } else {
        upper == 0
    }
}

/// Căn `value` lên bội số `align` (luỹ thừa 2).
pub const fn align_up(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}

/// Đi ba cấp từ `root` theo **hình** `walk_shape(va)`, đọc PTE qua [`PteMemory`].
///
/// Không panic: bảng không đọc được trả [`Walk::invalid`] ở cấp hiện tại. VA không canonical
/// trả invalid ngay (kể cả `u64::MAX`, không đọc bộ nhớ).
pub fn walk_with<M: PteMemory + ?Sized>(memory: &M, root: u64, va: u64) -> Walk {
    if !is_canonical(va) {
        return Walk::invalid(PTE_LEVELS - 1);
    }
    let indices = walk_shape(va).indices();
    let mut table = root;
    let mut level = PTE_LEVELS - 1;
    loop {
        let index = indices[PTE_LEVELS - 1 - level] as usize;
        let Some(entry) = memory.load(table, index) else {
            return Walk::invalid(level);
        };
        let pte = Pte::from_raw(entry);
        if entry & PTE_VALID == 0 {
            return Walk {
                level,
                pte,
                valid: false,
                leaf: false,
                phys: 0,
            };
        }
        if entry & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0 {
            return Walk {
                level,
                pte,
                valid: true,
                leaf: true,
                phys: (entry >> PTE_PPN_SHIFT) << 12,
            };
        }
        if level == 0 {
            // Bảng lá mà không có quyền nào: phần cứng coi là fault; trả về để caller thấy.
            return Walk {
                level,
                pte,
                valid: true,
                leaf: false,
                phys: 0,
            };
        }
        table = (entry >> PTE_PPN_SHIFT) << 12;
        level -= 1;
    }
}

/// Bộ dựng page table **trên một vùng arena RAM** (không cấp phát động, không `static`).
///
/// Arena là `&mut [u8]`; địa chỉ vật lý của nó là `arena_phys` (identity-mapped ở kernel K3 nên
/// walk phần cứng và phần mềm cùng nhìn một bộ nhớ). Mỗi lần cần bảng mới, builder "bump" 4 KiB,
/// zero bảng rồi trả địa chỉ vật lý; hết arena trả [`MapError::OutOfArena`].
pub struct PageTableBuilder<'a> {
    arena: &'a mut [u8],
    arena_phys: u64,
    used: usize,
    root: u64,
    tables: usize,
    leaves: usize,
}

impl<'a> PageTableBuilder<'a> {
    /// Tạo builder, cấp bảng gốc ở đầu arena (đã zero). `arena_phys` phải 4 KiB aligned.
    pub fn new(arena: &'a mut [u8], arena_phys: u64) -> Result<Self, MapError> {
        if arena.len() < PAGE_BYTES as usize {
            return Err(MapError::OutOfArena);
        }
        if arena_phys & (PAGE_BYTES - 1) != 0 {
            return Err(MapError::Unaligned);
        }
        let mut builder = Self {
            arena,
            arena_phys,
            used: PAGE_BYTES as usize,
            root: arena_phys,
            tables: 1,
            leaves: 0,
        };
        builder.zero_table(arena_phys)?;
        Ok(builder)
    }

    /// Địa chỉ vật lý bảng gốc.
    pub const fn root(&self) -> u64 {
        self.root
    }

    /// Số bảng đã cấp (gốc + trung gian).
    pub const fn tables(&self) -> usize {
        self.tables
    }

    /// Số byte arena đã dùng (bội số 4 KiB).
    pub const fn used_bytes(&self) -> usize {
        self.used
    }

    /// Số leaf đã map.
    pub const fn leaves(&self) -> usize {
        self.leaves
    }

    /// Phần arena còn trống (byte).
    pub const fn free_bytes(&self) -> usize {
        self.arena.len() - self.used
    }

    fn offset_of(&self, table_phys: u64) -> Option<usize> {
        let off = table_phys.checked_sub(self.arena_phys)? as usize;
        if off + PAGE_BYTES as usize > self.arena.len() {
            return None;
        }
        Some(off)
    }

    fn zero_table(&mut self, table_phys: u64) -> Result<(), MapError> {
        let off = self.offset_of(table_phys).ok_or(MapError::CorruptTable)?;
        let end = off + PAGE_BYTES as usize;
        let table = self.arena.get_mut(off..end).ok_or(MapError::CorruptTable)?;
        for byte in table.iter_mut() {
            *byte = 0;
        }
        Ok(())
    }

    fn alloc_table(&mut self) -> Result<u64, MapError> {
        if self.used + PAGE_BYTES as usize > self.arena.len() {
            return Err(MapError::OutOfArena);
        }
        let phys = self.arena_phys + self.used as u64;
        self.used += PAGE_BYTES as usize;
        self.tables += 1;
        self.zero_table(phys)?;
        Ok(phys)
    }

    fn store(&mut self, table_phys: u64, index: usize, value: u64) -> Result<(), MapError> {
        let off = self.offset_of(table_phys).ok_or(MapError::CorruptTable)?;
        let at = off + index * PTE_BYTES;
        let slot = self
            .arena
            .get_mut(at..at + PTE_BYTES)
            .ok_or(MapError::CorruptTable)?;
        slot.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// Map `va → pa` bằng leaf ở `level` (`0` = 4 KiB, `1` = 2 MiB, `2` = 1 GiB).
    ///
    /// Tự cấp bảng trung gian còn thiếu. Từ chối mapping đè (`AlreadyMapped`), VA không canonical,
    /// căn lề sai, cờ leaf sai. Không đụng `satp`/TLB — việc bật MMU là của arch (KT-46).
    pub fn map(&mut self, va: u64, pa: u64, level: usize, flags: PteFlags) -> Result<(), MapError> {
        if level >= PTE_LEVELS {
            return Err(MapError::LevelOutOfRange);
        }
        let size = 1u64 << PAGE_SIZE_BITS[level] as u32;
        if va & (size - 1) != 0 || pa & (size - 1) != 0 {
            return Err(MapError::Unaligned);
        }
        if !is_canonical(va) {
            return Err(MapError::NotCanonical);
        }
        if !flags.is_valid() || !flags.is_leaf() || (flags.is_writable() && !flags.is_readable()) {
            return Err(MapError::InvalidFlags);
        }

        let indices = walk_shape(va).indices();
        let mut table = self.root;
        let mut current = PTE_LEVELS - 1;
        while current > level {
            let index = indices[PTE_LEVELS - 1 - current] as usize;
            let entry = self.load(table, index).ok_or(MapError::CorruptTable)?;
            if entry & PTE_VALID == 0 {
                let new_table = self.alloc_table()?;
                let table_pte = ((new_table >> 12) << PTE_PPN_SHIFT) | PTE_VALID;
                self.store(table, index, table_pte)?;
                table = new_table;
            } else if entry & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0 {
                return Err(MapError::AlreadyMapped);
            } else {
                table = (entry >> PTE_PPN_SHIFT) << 12;
            }
            current -= 1;
        }

        let leaf_index = indices[PTE_LEVELS - 1 - level] as usize;
        let old = self.load(table, leaf_index).ok_or(MapError::CorruptTable)?;
        if old & PTE_VALID != 0 {
            return Err(MapError::AlreadyMapped);
        }
        let pte = ((pa >> 12) << PTE_PPN_SHIFT) | flags.bits();
        self.store(table, leaf_index, pte)?;
        self.leaves += 1;
        Ok(())
    }

    /// Walk phần mềm trên chính arena này — cùng hình với walk phần cứng.
    pub fn walk(&self, va: u64) -> Walk {
        walk_with(self, self.root, va)
    }
}

impl PteMemory for PageTableBuilder<'_> {
    fn load(&self, table_phys: u64, index: usize) -> Option<u64> {
        if index >= PTE_ENTRIES {
            return None;
        }
        let off = self.offset_of(table_phys)?;
        let at = off + index * PTE_BYTES;
        let mut slot = [0u8; PTE_BYTES];
        slot.copy_from_slice(self.arena.get(at..at + PTE_BYTES)?);
        Some(u64::from_le_bytes(slot))
    }
}

// ---------------------------------------------------------------- định dạng dùng chung (KT-164)

/// Định dạng **Sv39** (RISC-V, 3 mức, 4 KiB, 39-bit VA, 56-bit PA) — KT-164.
///
/// Đây là "mặt định dạng" của [`PageTableBuilder`]: cùng trait [`PageTableFormat`] với `Aarch64`
/// của `vifast-plat-aarch64`, nhờ vậy **một** thân test generic (`kiem_tung_dinh_dang`) chạy được
/// cho cả hai định dạng — bằng chứng trait dùng chung thật, không phải hai bản trùng.
///
/// Hằng số hình học lấy từ [`vifast_pte`] (`PTE_LEVELS`, `VA_BITS`, `PAGE_SIZE_BITS`,
/// `walk_shape`), bit PTE lấy từ chính module này — không chép tay lần thứ hai.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sv39;

impl Sv39 {
    /// Số bit PA của Sv39: PPN 44 bit + 12 bit offset.
    pub const PA_BITS: u32 = 56;
}

impl PageTableFormat for Sv39 {
    const LEVELS: usize = vifast_pte::PTE_LEVELS;
    const PAGE_BITS: u32 = PAGE_SIZE_BITS[0] as u32;
    const INDEX_BITS: u32 = 9;
    const VA_BITS: u32 = vifast_pte::VA_BITS as u32;
    const PA_BITS: u32 = Sv39::PA_BITS;
    const PAGE_BYTES: u64 = crate::mmu::PAGE_BYTES;
    const ENTRIES: usize = crate::mmu::PTE_ENTRIES;

    /// Chỉ số entry của `va` tại `level` (0 = gốc): `walk_shape` trả `[gốc, giữa, lá]` nên
    /// `indices()[level]` đúng quy ước của trait.
    fn index(va: u64, level: usize) -> usize {
        if level >= vifast_pte::PTE_LEVELS {
            return 0;
        }
        walk_shape(va).indices()[level] as usize
    }

    fn is_canonical(va: u64) -> bool {
        is_canonical(va)
    }

    fn encode_table(pa: u64) -> u64 {
        ((pa >> 12) << PTE_PPN_SHIFT) | PTE_VALID
    }

    /// Lá 4 KiB: `PPN | V | A | D | R/W/X` (+ `U` nếu user). Sv39 **không** có khái niệm
    /// thuộc tính bộ nhớ/shareability trong PTE (PBMT không dùng) nên bỏ qua `attr`/`share`.
    fn encode_leaf(pa: u64, perms: Perms, _attr: MemAttr, _share: Shareability) -> u64 {
        let mut pte = ((pa >> 12) << PTE_PPN_SHIFT)
            | PteFlags::leaf(perms.read, perms.write, perms.exec).bits();
        if perms.user {
            pte |= PTE_USER;
        }
        pte
    }

    fn decode(entry: u64, level: usize) -> Descriptor {
        if entry & PTE_VALID == 0 {
            return Descriptor::Invalid;
        }
        let pa = (entry >> PTE_PPN_SHIFT) << 12;
        let leaf = entry & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0;
        if level + 1 == vifast_pte::PTE_LEVELS {
            // Mức lá: entry không có quyền nào là bảng (phần cứng coi là fault), `W=1, R=0` là
            // reserved — cả hai đều không dùng được.
            if !leaf || (entry & PTE_WRITE != 0 && entry & PTE_READ == 0) {
                return Descriptor::Reserved;
            }
            Descriptor::Leaf {
                pa,
                perms: Perms {
                    read: entry & PTE_READ != 0,
                    write: entry & PTE_WRITE != 0,
                    exec: entry & PTE_EXEC != 0,
                    user: entry & PTE_USER != 0,
                },
            }
        } else if leaf {
            // Superpage 2 MiB (mức 1) / 1 GiB (mức 0) — `PageTableBuilder` không tạo ở đây.
            Descriptor::Block { pa, level }
        } else {
            Descriptor::Table { pa }
        }
    }

    /// Sv39 cấm `W=1, R=0` (privileged spec) và một lá không quyền nào là vô nghĩa.
    fn perms_valid(perms: Perms) -> bool {
        perms.is_valid() && (!perms.write || perms.read)
    }
}

/// `PageTableBuilder` cài **cùng** trait [`Mmu`] với `PageTable` của `vifast-plat-aarch64`
/// (KT-164) — tầng trên đổi MMU mà không đổi lời gọi.
///
/// Khác biệt có chủ đích: arena là bump allocator nên `unmap` **không** thu hồi frame (chỉ xoá
/// lá); `translate` trả `level = PTE_LEVELS - 1` (quy ước của trait: 0 = gốc) và từ chối
/// superpage bằng `BlockUnsupported`.
impl Mmu for PageTableBuilder<'_> {
    type Fmt = Sv39;

    fn root(&self) -> u64 {
        PageTableBuilder::root(self)
    }

    fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        if !<Sv39 as PageTableFormat>::perms_valid(perms) {
            return Err(MapError::InvalidPerms);
        }
        let mut flags = PteFlags::leaf(perms.read, perms.write, perms.exec);
        if perms.user {
            flags = PteFlags::from_bits(flags.bits() | PTE_USER);
        }
        PageTableBuilder::map(self, va, pa, 0, flags)
    }

    fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        if va & (PAGE_BYTES - 1) != 0 {
            return Err(MapError::Unaligned);
        }
        if !is_canonical(va) {
            return Err(MapError::NotCanonical);
        }
        let indices = walk_shape(va).indices();
        let mut table = self.root;
        let mut current = PTE_LEVELS - 1;
        while current > 0 {
            let index = indices[PTE_LEVELS - 1 - current] as usize;
            let entry = self.load(table, index).ok_or(MapError::CorruptTable)?;
            if entry & PTE_VALID == 0 {
                return Err(MapError::NotMapped);
            }
            if entry & (PTE_READ | PTE_WRITE | PTE_EXEC) != 0 {
                return Err(MapError::BlockUnsupported);
            }
            table = (entry >> PTE_PPN_SHIFT) << 12;
            current -= 1;
        }
        let leaf_index = indices[PTE_LEVELS - 1] as usize;
        let entry = self.load(table, leaf_index).ok_or(MapError::CorruptTable)?;
        if entry & PTE_VALID == 0 {
            return Err(MapError::NotMapped);
        }
        if entry & (PTE_READ | PTE_WRITE | PTE_EXEC) == 0 {
            return Err(MapError::CorruptTable);
        }
        let pa = (entry >> PTE_PPN_SHIFT) << 12;
        self.store(table, leaf_index, 0)?;
        self.leaves -= 1;
        Ok(pa)
    }

    fn translate(&self, va: u64) -> Result<Translation, MapError> {
        if !is_canonical(va) {
            return Err(MapError::NotCanonical);
        }
        let walk = self.walk(va);
        if !walk.valid {
            return Err(MapError::NotMapped);
        }
        if !walk.leaf {
            return Err(MapError::CorruptTable);
        }
        if walk.level != 0 {
            return Err(MapError::BlockUnsupported);
        }
        let raw = walk.raw();
        Ok(Translation {
            pa: walk.phys,
            perms: Perms {
                read: raw & PTE_READ != 0,
                write: raw & PTE_WRITE != 0,
                exec: raw & PTE_EXEC != 0,
                user: raw & PTE_USER != 0,
            },
            level: PTE_LEVELS - 1,
        })
    }
}
