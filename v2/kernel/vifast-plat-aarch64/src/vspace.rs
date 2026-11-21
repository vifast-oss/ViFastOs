// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`VSpace` thật cho aarch64** — KT-254 (K4' bước 3, B3.5).
//!
//! # Nối `PageTable` + wrapper + `Ttbr`
//!
//! [`VSpace`] giữ **một** [`SplitMmu`] (KT-253) — tức **hai** `PageTable` (nửa thấp `TTBR0_EL1`
//! cho VA người dùng, nửa cao `TTBR1_EL1` cho kernel) — cộng nhãn ASID. Mọi thao tác
//! map/unmap/walk **gọi thẳng** `SplitMmu`/`PageTable`: **không** có bản sao thứ hai của logic
//! walk/map (điều D2-199 muốn tránh, và cũng là điều object layer Sv39 đang vi phạm — nợ M17).
//!
//! [`VSpace::activate`] mã hoá `TTBR0_EL1`/`TTBR1_EL1` bằng [`Ttbr`] (BADDR + ASID), rồi
//! `ISB` → `TLBI VMALLE1` → `DSB ISH` → `ISB` — đúng thứ tự đổi gốc bảng trang.
//!
//! # Nợ M17 — **không** sửa trong task này
//!
//! Object layer `v2/kernel/vifast-plat-common/src/obj/vspace.rs` gắn **Sv39** (import
//! `walk_shape`, `PTE_LEVELS`, bit PTE riscv). Tổng quát hoá nó qua `Mmu` là nợ M17 của audit
//! K5'; B3.5 **không** sửa nó — dựng `VSpace` aarch64 ở đây và ghi nợ. Hệ quả: `VSpace` này
//! **chưa** có cap/Untyped/W^X toàn cục của object layer; nó là tầng **dữ liệu** của bảng trang
//! aarch64, đúng phạm vi B3.5.
//!
//! # Phạm vi
//!
//! `create`/`map_obj`/`unmap`/`walk`/`activate` + phần kernel nửa cao/physmap. **Không** TLBI
//! shootdown đa hart (B3.6), **không** U-mode (B3.7).

use crate::asm;
use crate::format::{Aarch64, MemAttr, Perms, Shareability};
use crate::mmu::{MapError, PageTable, TableMemory, Translation, Walk};
use crate::split_mmu::{map_high_alias, map_physmap, SplitMmu};
use crate::sysreg::Ttbr;
use crate::tlb;

/// Nhãn VSpace — giá trị `ASID` 16 bit ghi vào `TTBR0_EL1` và dùng làm `FaultKind::Vm.vspace`.
pub type VSpaceId = u16;

/// Một **Frame 4 KiB** vật lý để map vào `VSpace`.
///
/// Bất biến: `pa` căn 4 KiB — [`Frame::new`] là cửa duy nhất dựng được, nên không có `Frame`
/// lệch trang nào tồn tại để `map_obj` phải kiểm lại.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pa: u64,
}

impl Frame {
    /// Dựng Frame từ PA; `None` nếu `pa` không căn 4 KiB.
    pub const fn new(pa: u64) -> Option<Self> {
        if pa & 0xFFF != 0 {
            return None;
        }
        Some(Self { pa })
    }

    /// PA của Frame.
    pub const fn pa(self) -> u64 {
        self.pa
    }
}

/// Không gian địa chỉ ảo của **một** tiến trình: hai bảng trang (nửa thấp user + nửa cao kernel)
/// sau một nhãn ASID.
pub struct VSpace<M: TableMemory> {
    id: VSpaceId,
    mmu: SplitMmu<M>,
}

impl<M: TableMemory> VSpace<M> {
    /// **`vspace_create`** — dựng VSpace rỗng trên hai nguồn frame (thường là hai handle **chia
    /// sẻ** cùng một arena — `bootmem::shared_boot_memory`).
    pub fn create(id: VSpaceId, low_memory: M, high_memory: M) -> Result<Self, MapError> {
        Ok(Self {
            id,
            mmu: SplitMmu::new(low_memory, high_memory)?,
        })
    }

    /// Nhãn ASID của VSpace.
    pub const fn id(&self) -> VSpaceId {
        self.id
    }

    /// Wrapper định tuyến bên dưới (đọc).
    pub const fn mmu(&self) -> &SplitMmu<M> {
        &self.mmu
    }

    /// Wrapper định tuyến bên dưới (ghi) — cho chẩn đoán/đường đặc biệt.
    pub fn mmu_mut(&mut self) -> &mut SplitMmu<M> {
        &mut self.mmu
    }

    /// PA gốc bảng **nửa thấp** (`TTBR0`).
    pub const fn root_low(&self) -> u64 {
        self.mmu.root_low()
    }

    /// PA gốc bảng **nửa cao** (`TTBR1`).
    pub const fn root_high(&self) -> u64 {
        self.mmu.root_high()
    }

    /// **`vspace_map_obj`** — map một [`Frame`] 4 KiB vào `va` với quyền `perms`.
    ///
    /// Định tuyến theo VA (nửa thấp ⇒ `TTBR0`, nửa cao ⇒ `TTBR1`) qua [`SplitMmu`]; mọi kiểm
    /// tra (căn lề, canonical, dải PA, quyền, trùng) là của `PageTable` — **không** lặp lại ở đây.
    pub fn map_obj(&mut self, va: u64, frame: Frame, perms: Perms) -> Result<(), MapError> {
        self.mmu.map(va, frame.pa(), perms)
    }

    /// Như [`Self::map_obj`] nhưng chỉ định **thuộc tính bộ nhớ** (`Device` cho MMIO, `Normal`
    /// cho RAM) — cần cho UART/GIC trong VSpace của ảnh.
    pub fn map_obj_with(
        &mut self,
        va: u64,
        frame: Frame,
        perms: Perms,
        attr: MemAttr,
    ) -> Result<(), MapError> {
        self.mmu.map_with(
            va,
            frame.pa(),
            perms,
            attr,
            crate::format::Shareability::Inner,
        )
    }

    /// **`vspace_map_block`** — map **block** (superpage) `va → pa` ở `level` — KT-302.
    ///
    /// Dùng cho ECAM PCIe 256 MiB: 128 block 2 MiB ở mức 2 ⇒ **một** bảng mức 2 (2 frame) thay
    /// cho 128 bảng mức 3 (129 frame). Định tuyến theo VA qua [`SplitMmu`]; mọi kiểm tra là của
    /// [`PageTable::map_block`] — **không** lặp lại ở đây.
    pub fn map_block(
        &mut self,
        va: u64,
        pa: u64,
        level: usize,
        perms: Perms,
        attr: MemAttr,
    ) -> Result<(), MapError> {
        self.mmu
            .map_block(va, pa, level, perms, attr, Shareability::Inner)
    }

    /// **`vspace_unmap`** — bỏ mapping tại `va`, trả PA đã map.
    ///
    /// **Không** đụng TLB: đây là thao tác **bảng trang thuần**. Đường sản phẩm là
    /// [`Self::unmap_shootdown`] — nó gọi hàm này rồi chạy chuỗi `TLBI` + shootdown đa hart
    /// (KT-262). Gọi `unmap` trần rồi đọc lại cùng VA là **sai** (TLB còn entry cũ).
    pub fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        self.mmu.unmap(va)
    }

    /// **`vspace_unmap_shootdown`** — `unmap` + `TLBI` + shootdown đa hart (KT-262, B3.6).
    ///
    /// Thứ tự **bắt buộc** (xem [`crate::shootdown`]):
    ///
    /// 1. xoá lá bảng trang ([`Self::unmap`]);
    /// 2. chuỗi cục bộ `dsb ishst` → `tlbi vae1is` → `dsb ish` → `isb`;
    /// 3. SGI `SGI_SHOOTDOWN` tới từng hart trong `targets`; handler của chúng chạy
    ///    `tlbi vae1is` + `dsb ish` + `isb` rồi **ack**;
    /// 4. chờ đủ ack (trần [`crate::shootdown::ACK_WAIT_LIMIT`]).
    ///
    /// `targets = Targets::NONE` ⇒ chỉ bước 2 (đường lúc chưa có hart phụ nào online — ví dụ
    /// TAP 9 của ảnh). Caller **phải** kiểm [`Shootdown::ok`] của kết quả: `unmap` đã xong bảng
    /// trang kể cả khi shootdown hết trần, nên bỏ qua kết quả là bỏ qua lỗi thật.
    pub fn unmap_shootdown(
        &mut self,
        va: u64,
        targets: crate::shootdown::Targets,
    ) -> Result<(u64, crate::shootdown::Shootdown), MapError> {
        let pa = self.mmu.unmap(va)?;
        Ok((pa, crate::shootdown::invalidate(va, targets)))
    }

    /// **`vspace_walk`** — dịch `va → pa` trên bảng **thật** của VSpace.
    pub fn walk(&self, va: u64) -> Result<Translation, MapError> {
        self.mmu.translate(va)
    }

    /// Walk **chẩn đoán** (mọi bước đã đi) — dùng cho log/TAP, không phải đường map.
    pub fn walk_steps(&self, va: u64) -> Walk {
        self.mmu.walk(va)
    }

    /// Map **alias nửa cao** của ảnh kernel `[start_pa, end_pa)` tại `high_base` vào bảng nửa
    /// cao của VSpace — để kernel chạy tiếp sau khi `activate` đổi `TTBR1`.
    pub fn map_kernel_high(
        &mut self,
        start_pa: u64,
        end_pa: u64,
        high_base: u64,
        perms: Perms,
        attr: MemAttr,
    ) -> Result<usize, MapError> {
        map_high_alias(
            self.mmu.high_mut(),
            start_pa,
            end_pa,
            high_base,
            perms,
            attr,
        )
    }

    /// Map cửa sổ **physmap** `[start_pa, end_pa)` vào bảng nửa cao của VSpace.
    pub fn map_physmap(
        &mut self,
        start_pa: u64,
        end_pa: u64,
        perms: Perms,
        attr: MemAttr,
    ) -> Result<usize, MapError> {
        map_physmap(self.mmu.high_mut(), start_pa, end_pa, perms, attr)
    }

    /// PA bảng **mức 1** cho `TTBR0_EL1` (`None` nếu VSpace chưa map gì ở nửa thấp).
    pub fn ttbr0(&self) -> Option<u64> {
        self.mmu.ttbr0_base()
    }

    /// PA bảng **mức 1** cho `TTBR1_EL1` (`None` nếu chưa map gì ở nửa cao).
    pub fn ttbr1(&self) -> Option<u64> {
        self.mmu.ttbr1_base()
    }

    /// Giá trị 64 bit ghi vào `TTBR0_EL1`: `BADDR` + `ASID` của VSpace ([`Ttbr`]).
    pub fn ttbr0_value(&self) -> Option<u64> {
        self.ttbr0().map(|baddr| Ttbr::new(baddr, self.id).bits())
    }

    /// Giá trị 64 bit ghi vào `TTBR1_EL1`: `BADDR` + `ASID = 0` (kernel dùng chung).
    pub fn ttbr1_value(&self) -> Option<u64> {
        self.ttbr1().map(|baddr| Ttbr::new(baddr, 0).bits())
    }

    /// **`vspace_activate`** — đổi gốc bảng trang đang hoạt động sang VSpace này.
    ///
    /// Thứ tự **bắt buộc**: ghi `TTBR0_EL1` + `TTBR1_EL1` → `ISB` (context sync) → `TLBI
    /// VMALLE1` (bỏ entry cũ của bảng trước) → `DSB ISH` → `ISB`. Thiếu `TLBI` thì entry TLB
    /// của bảng cũ còn sống ⇒ đọc/ghi đi sai bảng (đúng lỗi mà B3.6 sẽ kiểm đa hart).
    ///
    /// Lỗi: `CorruptTable` nếu VSpace chưa có bảng mức 1 ở một trong hai nửa (không bịa địa chỉ).
    pub fn activate(&self) -> Result<(), MapError> {
        let ttbr0 = self.ttbr0().ok_or(MapError::CorruptTable)?;
        let ttbr1 = self.ttbr1().ok_or(MapError::CorruptTable)?;
        asm::write_ttbr0_el1(Ttbr::new(ttbr0, self.id).bits());
        asm::write_ttbr1_el1(Ttbr::new(ttbr1, 0).bits());
        asm::isb();
        tlb::vmalle1();
        asm::dsb_ish();
        asm::isb();
        Ok(())
    }
}

/// `VSpace` cũng là một `Mmu` (định tuyến qua wrapper) — tầng trên dùng được mà không biết
/// aarch64. `root()` trả gốc **nửa thấp** (xem [`SplitMmu`]).
impl<M: TableMemory> crate::mmu::Mmu for VSpace<M> {
    type Fmt = Aarch64;

    fn root(&self) -> u64 {
        self.root_low()
    }

    fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        self.mmu.map(va, pa, perms)
    }

    fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        self.mmu.unmap(va)
    }

    fn translate(&self, va: u64) -> Result<Translation, MapError> {
        self.mmu.translate(va)
    }
}

/// `PageTable` của VSpace (chẩn đoán) — tránh import thừa ở ảnh.
pub type VSpaceTable<M> = PageTable<Aarch64, M>;
