// SPDX-License-Identifier: MIT OR Apache-2.0
//! Định dạng bảng trang **x86_64 4-level paging** (4 KiB, 4 mức) — KT-186 / D2-163 (K5' bước M2).
//!
//! Tệp này là phần **thuần dữ liệu**: mô tả descriptor 64-bit của x86_64 (PML4E/PDPTE/PDE/PTE),
//! hình học bảng trang và cách bóc/ghép bit. **Không** asm, **không** `cfg(target_arch)`,
//! **không** `unsafe` — nhờ vậy chạy được trên host và là bằng chứng đo được của K5' bước M2
//! (`cargo test`, **không** QEMU). Phần `CR3`/`CR0.PG`/`CR4.PAE`/`EFER.LME`/`INVLPG` thuộc
//! bước M4 (crate arch, D2-17).
//!
//! # Nguồn
//!
//! - **Intel SDM Vol 3A**, ch. 4 "Paging", mục "4-Level Paging and 5-Level Paging" — định dạng
//!   PML4E/PDPTE/PDE/PTE, các bit `P/RW/US/PWT/PCD/A/D/PS/PAT/G/NX`, địa chỉ vật lý bit 51:12.
//! - **Intel SDM Vol 3A**, mục "Page-Attribute Table (PAT)" (trong §4.9 "Paging and Memory
//!   Typing") — bảng chọn kiểu bộ nhớ theo `PAT/PWT/PCD`; giá trị mặc định `IA32_PAT`.
//! - **Intel SDM Vol 3A**, mục "Extended Feature Enable Register (IA32_EFER)" — `NXE` (bit 11)
//!   bật bit `NX` của PTE.
//!
//! # Hình học
//!
//! ```text
//!   granule        4 KiB (12 bit offset)
//!   số mức         4 (PML4 = mức 0, PT = mức 3)
//!   chỉ số/mức     9 bit (512 entry × 8 B = 4 KiB/bảng)
//!   VA             39 bit **canonical** (cửa sổ kernel chọn — D2-147; xem dưới)
//!   PA             48 bit (bit 47:12 của descriptor)
//! ```
//!
//! **Vì sao 39-bit VA?** Phần cứng x86_64 4-level paging có VA canonical **48-bit** (bit 47 chép
//! vào bit 63:48). D2-147 (K4' bước một) đã chốt cửa sổ **39-bit canonical** cho kernel để giống
//! hình Sv39 của riscv64; K5' bước M2 giữ cùng lựa chọn (audit K5' §4 bước M2 ghi "VA canonical
//! ba mươi chín bit"). 39-bit canonical là **tập con** của 48-bit canonical, nên `is_canonical`
//! ở đây kiểm **chính sách phần mềm** của kernel, không phải giới hạn phần cứng; hằng
//! [`X86_64_MAX_VA_BITS`] ghi hình tối đa của format. Với VA 39-bit, chỉ số PML4 chỉ có hai giá
//! trị hợp lệ: `0` (nửa thấp) và `0x1FF` (nửa cao) — walk vẫn đi **đủ bốn mức**.
//!
//! # Descriptor 64-bit (4-level paging)
//!
//! ```text
//!  63    62          52 51                    12 11      9 8 7 6 5 4 3 2 1 0
//! [NX][  reserved     ][      addr[51:12]      ][ avail ][G][PAT/PS][D][A][PCD][PWT][US][RW][P]
//! ```
//!
//! - `P` bit 0: entry hợp lệ;
//! - `RW` bit 1: `1` ⇒ ghi được (chỉ có nghĩa với lá; với bảng, bit này bị bỏ qua nhưng vẫn đặt);
//! - `US` bit 2: `1` ⇒ CPL3 truy cập được. **Mọi** mức phải có `US = 1` thì trang user mới vào
//!   được (AMD64 APM Vol 2, mục "User/Supervisor (U/S) Bit": bit U/S của PML4E kiểm soát toàn bộ
//!   vùng nó phủ); vì vậy [`X86_64::encode_table`] đặt `US = 1`, còn lá kernel `US = 0` vẫn chặn;
//! - `PWT` bit 3 / `PCD` bit 4: chọn kiểu bộ nhớ (xem PAT);
//! - `A` bit 5 / `D` bit 6: accessed/dirty — bước M2 đặt **eager** cả hai để walk phần mềm không
//!   phụ thuộc việc phần cứng cập nhật A/D (M4 sẽ quyết định có bật A/D fault hay không);
//! - `PS` bit 7: ở mức 0..2, `1` ⇒ page lớn (1 GiB ở mức 1, 2 MiB ở mức 2); ở **mức 3 bit này là
//!   `PAT`**, không phải `PS` — đây là cái bẫy hay gặp, [`X86_64::decode`] xử lý đúng;
//! - `G` bit 8: global (bước M2 không dùng);
//! - `PAT` bit 7 (lá 4 KiB) / bit 12 (lá 2 MiB, 1 GiB): chọn entry `IA32_PAT`; bước M2 để `0`;
//! - `NX` bit 63: `1` ⇒ không thực thi (chỉ có hiệu lực khi `IA32_EFER.NXE = 1`).
//!
//! # PAT và `MemAttr`
//!
//! Với lá 4 KiB, chỉ số `IA32_PAT` = `PAT(bit 7) << 2 | PWT(bit 3) << 1 | PCD(bit 4)`; giá trị
//! mặc định `IA32_PAT = 0x0007_0406_0007_0406` cho: index 0 = WB, 1 = WT, 2 = UC-, 3 = UC.
//! [`X86_64::encode_leaf`] ánh xạ [`MemAttr::Normal`] → `PWT = PCD = 0` (WB) và
//! [`MemAttr::Device`] → `PWT = PCD = 1` (UC). [`Shareability`] **bị bỏ qua** (x86 không có bit
//! `SH`); đây là quy ước "bỏ qua" mà audit K5' §2.1 yêu cầu, không phải lỗi.
//!
//! # Quan hệ với `vifast-pte` (KT-164/KT-186)
//!
//! Trait [`PageTableFormat`] + [`Mmu`](vifast_pte::Mmu) + walker [`PageTable`](vifast_pte::PageTable)
//! nằm ở crate **dùng chung** `vifast-pte`; tệp này **chỉ còn impl** ([`X86_64`] +
//! `impl PageTableFormat for X86_64`), không định nghĩa lại trait/walker.

pub use vifast_pte::{
    is_canonical_bits, Descriptor, MemAttr, PageTableFormat, Perms, Shareability,
};

/// Số mức bảng trang của x86_64 4-level paging.
pub const X86_64_LEVELS: usize = 4;
/// Số bit offset trong trang (4 KiB).
pub const X86_64_PAGE_BITS: u32 = 12;
/// Số bit chỉ số mỗi mức (512 entry).
pub const X86_64_INDEX_BITS: u32 = 9;
/// Số bit VA **canonical** mà kernel dùng (cửa sổ 39-bit — D2-147, audit K5' bước M2).
pub const X86_64_VA_BITS: u32 = 39;
/// Số bit VA **tối đa** của hình 4-level paging (phần cứng: 48-bit canonical).
pub const X86_64_MAX_VA_BITS: u32 = 48;
/// Số bit địa chỉ vật lý (PA) trong descriptor.
pub const X86_64_PA_BITS: u32 = 48;
/// Cỡ một trang/bảng: 4 KiB.
pub const X86_64_PAGE_BYTES: u64 = 1 << X86_64_PAGE_BITS;
/// Số entry một bảng trang (`2^9 = 512`).
pub const X86_64_ENTRIES: usize = 1 << X86_64_INDEX_BITS;

/// Bit `P` — entry hợp lệ.
pub const PTE_PRESENT: u64 = 1 << 0;
/// Bit `RW` — ghi được (lá).
pub const PTE_RW: u64 = 1 << 1;
/// Bit `US` — CPL3 truy cập được.
pub const PTE_USER: u64 = 1 << 2;
/// Bit `PWT` — page-level write-through.
pub const PTE_PWT: u64 = 1 << 3;
/// Bit `PCD` — page-level cache disable.
pub const PTE_PCD: u64 = 1 << 4;
/// Bit `A` — accessed.
pub const PTE_ACCESSED: u64 = 1 << 5;
/// Bit `D` — dirty.
pub const PTE_DIRTY: u64 = 1 << 6;
/// Bit `PS` ở mức 0..2 — page lớn; ở mức 3 bit này là `PAT`.
pub const PTE_PS: u64 = 1 << 7;
/// Bit `G` — global.
pub const PTE_GLOBAL: u64 = 1 << 8;
/// Bit `PAT` của lá 4 KiB — **bit 7** (cùng vị trí với `PS` ở mức 0..2; ở mức 3 nó là `PAT`).
pub const PTE_PAT: u64 = 1 << 7;
/// Bit `PAT` của lá lớn 2 MiB/1 GiB — bit 12 (với lá 4 KiB, bit 12 là địa chỉ vật lý).
pub const PTE_PAT_LARGE: u64 = 1 << 12;
/// Bit `NX` — không thực thi.
pub const PTE_NX: u64 = 1 << 63;
/// Vị trí địa chỉ vật lý trong descriptor (bit 12).
pub const PTE_ADDR_SHIFT: u32 = 12;
/// Mặt nạ địa chỉ vật lý 48-bit đã bỏ 12 bit offset.
pub const PTE_ADDR_MASK: u64 = ((1u64 << X86_64_PA_BITS) - 1) & !(X86_64_PAGE_BYTES - 1);
/// Các bit **reserved** của descriptor 4-level: bit 62:52 phải bằng 0 (bit 63 là `NX`).
pub const PTE_RESERVED_MASK: u64 = 0x7FF0_0000_0000_0000;

/// Giá trị mặc định của `IA32_PAT` (Intel SDM Vol 3A, mục PAT): PAT0=WB, PAT1=WT, PAT2=UC-,
/// PAT3=UC, PAT4=WB, PAT5=WT, PAT6=UC-, PAT7=UC.
pub const IA32_PAT_DEFAULT: u64 = 0x0007_0406_0007_0406;

/// Định dạng x86_64 4-level paging, 4 KiB, 39-bit VA canonical, 48-bit PA (D2-163 / KT-186).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct X86_64;

impl X86_64 {
    /// Chỉ số entry của `va` tại `level` — `level` ngoài `0..4` trả `0` (hàm thuần, không panic).
    pub const fn index(va: u64, level: usize) -> usize {
        if level >= X86_64_LEVELS {
            return 0;
        }
        let shift =
            X86_64_PAGE_BITS + X86_64_INDEX_BITS * (X86_64_LEVELS as u32 - 1 - level as u32);
        ((va >> shift) & (X86_64_ENTRIES as u64 - 1)) as usize
    }

    /// `va` canonical theo cửa sổ 39-bit của kernel: bit 38 chép vào bit 39..63.
    pub const fn is_canonical(va: u64) -> bool {
        is_canonical_bits(va, X86_64_VA_BITS)
    }

    /// Cỡ vùng VA mà một entry ở `level` phủ (`level` ngoài dải ⇒ 0).
    pub const fn span(level: usize) -> u64 {
        if level >= X86_64_LEVELS {
            return 0;
        }
        1u64 << (X86_64_PAGE_BITS + X86_64_INDEX_BITS * (X86_64_LEVELS as u32 - 1 - level as u32))
    }

    /// Descriptor bảng trỏ tới `pa` (`P | RW | US | addr`).
    ///
    /// `RW` bị bỏ qua với entry không phải lá nhưng vẫn đặt `1` cho nhất quán; `US = 1` là **bắt
    /// buộc** để lá user nằm dưới bảng này truy cập được (bit U/S của mọi mức phải bằng 1). Lá
    /// kernel vẫn có `US = 0` nên không bị hở.
    pub const fn encode_table(pa: u64) -> u64 {
        PTE_PRESENT | PTE_RW | PTE_USER | (pa & PTE_ADDR_MASK)
    }

    /// Descriptor lá 4 KiB.
    ///
    /// Bất biến (test `descriptor_bits_literal_intel_sdm` chốt từng bit bằng giá trị tuyệt đối):
    /// - `P = 1`, `A = 1`, `D = 1` (eager — M2 không phụ thuộc A/D fault);
    /// - `RW = write`, `US = user`, `NX = !exec`;
    /// - `PS = 0` (lá 4 KiB), `PAT = 0` (dùng entry PAT mặc định);
    /// - `PWT = PCD = 1` với [`MemAttr::Device`] (UC), `0` với [`MemAttr::Normal`] (WB);
    /// - [`Shareability`] bị bỏ qua (x86 không có `SH`).
    pub const fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, _share: Shareability) -> u64 {
        let mut d = PTE_PRESENT | PTE_ACCESSED | PTE_DIRTY;
        if perms.write {
            d |= PTE_RW;
        }
        if perms.user {
            d |= PTE_USER;
        }
        if !perms.exec {
            d |= PTE_NX;
        }
        match attr {
            MemAttr::Device => d |= PTE_PWT | PTE_PCD,
            MemAttr::Normal => {}
        }
        d | (pa & PTE_ADDR_MASK)
    }

    /// Bóc descriptor tại `level` (0 = PML4, 3 = PT).
    ///
    /// - `P = 0` ⇒ [`Descriptor::Invalid`];
    /// - bit reserved 62:52 khác 0 ⇒ [`Descriptor::Reserved`];
    /// - mức 3: `P = 1` **luôn** là lá (bit 7 là `PAT`, không phải `PS`);
    /// - mức 0..2: `PS = 1` ⇒ block (mức 1 = 1 GiB, mức 2 = 2 MiB); `PS = 1` ở mức 0 là reserved;
    /// - còn lại ⇒ bảng.
    pub const fn decode(entry: u64, level: usize) -> Descriptor {
        if entry & PTE_PRESENT == 0 {
            return Descriptor::Invalid;
        }
        if entry & PTE_RESERVED_MASK != 0 {
            return Descriptor::Reserved;
        }
        let pa = entry & PTE_ADDR_MASK;
        if level >= X86_64_LEVELS {
            return Descriptor::Reserved;
        }
        if level + 1 == X86_64_LEVELS {
            // PT: P=1 là lá; bit 7 là PAT, không phải PS.
            return Descriptor::Leaf {
                pa,
                perms: decode_perms(entry),
            };
        }
        if entry & PTE_PS != 0 {
            if level == 0 {
                // 512 GiB page không tồn tại trong 4-level paging.
                Descriptor::Reserved
            } else {
                Descriptor::Block { pa, level }
            }
        } else {
            Descriptor::Table { pa }
        }
    }

    /// `PAT` của một lá 4 KiB (bit 7).
    pub const fn leaf_pat(entry: u64) -> bool {
        entry & PTE_PAT != 0
    }

    /// [`MemAttr`] của một lá 4 KiB, suy từ `PAT/PWT/PCD` + `IA32_PAT` mặc định.
    ///
    /// Trả `None` khi tổ hợp là WT/UC- (không biểu diễn được bằng [`MemAttr`]) hoặc khi `PAT = 1`
    /// (bảng PAT khác — bước M2 không dùng).
    pub const fn leaf_mem_attr(entry: u64) -> Option<MemAttr> {
        if entry & PTE_PAT != 0 {
            return None;
        }
        let pwt = entry & PTE_PWT != 0;
        let pcd = entry & PTE_PCD != 0;
        match (pwt, pcd) {
            (false, false) => Some(MemAttr::Normal),
            (true, true) => Some(MemAttr::Device),
            _ => None,
        }
    }
}

/// Bóc quyền từ descriptor lá (dùng chung cho [`X86_64::decode`]).
///
/// x86 **không có** bit `R`: mọi trang `P = 1` đều đọc được, nên `read` luôn `true`.
const fn decode_perms(entry: u64) -> Perms {
    Perms {
        read: true,
        write: entry & PTE_RW != 0,
        exec: entry & PTE_NX == 0,
        user: entry & PTE_USER != 0,
    }
}

impl PageTableFormat for X86_64 {
    const LEVELS: usize = X86_64_LEVELS;
    const PAGE_BITS: u32 = X86_64_PAGE_BITS;
    const INDEX_BITS: u32 = X86_64_INDEX_BITS;
    const VA_BITS: u32 = X86_64_VA_BITS;
    const PA_BITS: u32 = X86_64_PA_BITS;
    const PAGE_BYTES: u64 = X86_64_PAGE_BYTES;
    const ENTRIES: usize = X86_64_ENTRIES;

    fn index(va: u64, level: usize) -> usize {
        X86_64::index(va, level)
    }

    fn is_canonical(va: u64) -> bool {
        X86_64::is_canonical(va)
    }

    fn encode_table(pa: u64) -> u64 {
        X86_64::encode_table(pa)
    }

    fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, share: Shareability) -> u64 {
        X86_64::encode_leaf(pa, perms, attr, share)
    }

    fn decode(entry: u64, level: usize) -> Descriptor {
        X86_64::decode(entry, level)
    }

    /// x86 **không có** bit `R`: trang `P = 1` luôn đọc được. `read = false` là quyền **không
    /// biểu diễn được** ⇒ từ chối (`InvalidPerms`) thay vì lặng lẽ cấp quyền đọc.
    fn perms_valid(perms: Perms) -> bool {
        perms.read
    }
}
