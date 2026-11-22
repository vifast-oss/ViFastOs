// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340b** — `VSpace` **động** cho x86_64: bảng trang **riêng** cho từng tác vụ ring 3
//! (D2-238/D2-244; khuôn `KT-254` aarch64 + `mmu.rs` riscv64).
//!
//! # Vấn đề trước KT-340b
//!
//! Ảnh x86_64 có **một** cây bảng trang identity duy nhất (`__pml4` dựng trong `_start`), và
//! ring 3 dùng **một** cây con user tĩnh (`PDPT[256] → __pd_user → __pt_user`, một trang mã +
//! một trang stack). Mọi tác vụ chia sẻ đúng cây đó ⇒ "tác vụ A ghi vào vùng của tác vụ B" chỉ
//! bị chặn bởi **quy ước**, không bởi phần cứng: không có bảng trang riêng, không `CR3` riêng,
//! không quyền theo tác vụ (D2-238).
//!
//! # Cách làm (không bịa cơ chế mới)
//!
//! [`VSpace`] bọc `vifast_pte::PageTable<X86_64, M>` — **đúng** walker dùng chung mà ba định dạng
//! đang dùng (D2-163): mọi kiểm tra VA canonical/PA/quyền/trùng là của walker, tệp này không lặp
//! lại (D2-199). Nguồn frame là [`crate::bootmem::BootArena`] (RAM ngoài `.bss`).
//!
//! Một `VSpace` gồm:
//!
//! 1. **PML4 riêng** + **PDPT riêng** (hai frame đầu của arena);
//! 2. ba entry **chia sẻ** từ PDPT của ảnh (`[0]` identity kernel 1 GiB, `[2]` MMIO 2..3 GiB,
//!    `[3]` MMIO 3..4 GiB) — kernel/MMIO là của **chung**, chỉ cây con user là riêng. Nhờ vậy
//!    `CR3` đổi sang VSpace mà kernel vẫn chạy (GDT/IDT/TSS/stack/handler/COM1), timer vẫn `EOI`
//!    được LAPIC, và code của ảnh **không** phải sao chép mapping;
//! 3. cây con **user riêng**: `PageTable::map` cấp PD/PT từ arena khi map trang user đầu tiên.
//!
//! Quyền theo **từng trang** (`Perms` của `vifast-pte`): mã user = `USER_RX` (`P|US`, không `RW`,
//! không `NX`), stack/dữ liệu user = `USER_RW` (`P|RW|US|NX`), kernel/MMIO = supervisor (`US=0`).
//! `PageTableFormat::encode_table` của x86_64 đặt `US = 1` cho **bảng** (điều kiện phần cứng để
//! lá user dưới nó truy cập được — SDM Vol 3A §4.6), còn lá kernel giữ `US = 0` nên ring 3 không
//! chạm được kernel (bit `US` được **AND** qua các cấp).
//!
//! # TLB
//!
//! - [`VSpace::activate`] ghi `CR3` ⇒ x86 xả **mọi** entry không-`G` (ảnh không đặt `G` cho lá
//!   user) — đúng thứ tự "đổi gốc bảng trang rồi mới chạy tiếp";
//! - [`VSpace::map`]/[`VSpace::unmap`] phát `INVLPG` cho **đúng VA** vừa đổi. Đột biến
//!   `no-invlpg` bỏ lệnh này: ca `# vspace: remap` (đọc lại **cùng** VA sau khi đổi frame) sẽ đọc
//!   ra dữ liệu **cũ** từ TLB ⇒ ĐỎ. Đây là lý do phép đo dùng `map → read → unmap → map → read`
//!   trên **một** `VSpace`: nếu chỉ tin `CR3` reload thì `INVLPG` không quan sát được và "có
//!   invlpg" chỉ là chữ trong tài liệu.
//!
//! # **KT-340d** — TLB shootdown đa hart (D2-238/D2-244/D2-247)
//!
//! `VSpace` giữ **`active_on`** (bitmask hart đang chạy cây bảng trang này). Khi `map`/`unmap`
//! đổi một VA **mà hart khác đang dùng**, `INVLPG` cục bộ là **không đủ** (TLB của hart kia vẫn
//! ánh xạ PA cũ ⇒ nó **đọc dữ liệu cũ** — đúng lỗi v1 **N5**). Vì vậy [`VSpace::flush_va`]:
//!
//! 1. `INVLPG` **cục bộ** ngay (hart gọi, D2-68 — không self-target);
//! 2. gửi `TlbFlush(va)` tới **các hart khác** trong `active_on` qua `crate::ipi` (bit vào hộp
//!    thư trước, IPI fixed-delivery vector `0x30` sau) và **chờ đủ ACK có trần**.
//!
//! `mode` của yêu cầu: `map` yêu cầu hart nhận **đọc lại** VA sau `invlpg` ([`crate::ipi`]
//! `MODE_OBSERVE`) — bằng chứng "hart kia **thấy** mapping mới", không chỉ "đã gửi IPI"; `unmap`
//! chỉ xoá (đọc lại một VA vừa bỏ map sẽ fault — không bao giờ làm).
//!
//! `active_on` do [`VSpace::activate`]/[`VSpace::restore`] cập nhật cho hart **gọi**; một hart
//! **nhận** cây bảng trang qua `CR3` (không có đối tượng `VSpace` trên hart ấy) được đánh dấu bằng
//! [`VSpace::mark_active`] — hàm thuần trạng thái, không chạm phần cứng.
//!
//! # Phạm vi
//!
//! `create`/`map`/`unmap`/`walk`/`activate`/`restore` + shootdown theo **một VA**. **Không**
//! ASID/PCID (đóng **N5** bằng nhánh "tắt `PCID` có kiểm soát" — xem `crate::ipi`), **không**
//! shootdown theo **range**, **không** `free_frame` (arena boot không thu hồi — [`crate::bootmem`]).

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_pte::{MapError, PageTable, Perms, TableMemory, Translation, Walk, WalkOutcome};

use crate::asm;
use crate::bootmem::FRAME_BYTES;
use crate::format::{MemAttr, X86_64};
use crate::ipi;

/// Mặt nạ địa chỉ vật lý 4 KiB của descriptor x86_64 (`PA[51:12]`).
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
/// Bit `P` của descriptor — entry có mặt.
const PTE_PRESENT: u64 = 1 << 0;

/// **`vspace_create`** — một không gian địa chỉ của tác vụ.
///
/// `id` chỉ là nhãn chẩn đoán (chưa có ASID/PCID — `KT-340d`); `boot_root` là `CR3` của ảnh lúc
/// tạo, giữ lại để [`VSpace::restore`] đưa kernel về cây gốc mà **không** phải đoán.
pub struct VSpace<M: TableMemory> {
    id: u16,
    pt: PageTable<X86_64, M>,
    boot_root: u64,
    /// Bit `h` ⇔ hart `h` đang chạy cây bảng trang này (**KT-340d**).
    active_on: AtomicU32,
    /// Bộ đếm `seq` của shootdown do VSpace này phát (bắt đầu từ 1 — `seq = 0` là "không có yêu
    /// cầu", xem [`ipi::ack_ok`]).
    flush_seq: AtomicU64,
}

impl<M: TableMemory> VSpace<M> {
    /// Dựng VSpace rỗng: cấp PML4 + PDPT từ nguồn frame `memory`, rồi **chia sẻ** ba entry
    /// kernel/MMIO của PDPT đang hoạt động (`CR3` lúc gọi).
    ///
    /// Từ chối (`MapError`) khi: arena hết frame (`OutOfFrames`), ghi entry hỏng (`CorruptTable`),
    /// hoặc `CR3` không trỏ bảng hợp lệ. **Không** cấp frame nào bị rò khi từ chối (walker đã
    /// zero + trả frame gốc; PDPT lỗi được `free_frame` — no-op của arena boot).
    pub fn create(id: u16, memory: M) -> Result<Self, MapError> {
        let mut pt = PageTable::new(memory)?;
        let root = pt.root();
        let boot_root = asm::read_cr3() & !(FRAME_BYTES - 1);
        let boot_pdpt = asm::read_u64(boot_root) & ADDR_MASK;
        if boot_pdpt == 0 {
            return Err(MapError::CorruptTable);
        }

        // PDPT riêng — frame thứ hai của cây.
        let pdpt = {
            let mem = pt.memory_mut();
            let frame = mem.alloc_frame().ok_or(MapError::OutOfFrames)?;
            if !mem.zero_frame(frame) {
                mem.free_frame(frame);
                return Err(MapError::CorruptTable);
            }
            frame
        };
        // PML4[0] = PDPT riêng. `US = 1` ở gốc là điều kiện **cần** để cây con user (dưới
        // PDPT[64] do walker tạo) truy cập được từ CPL3; identity/MMIO vẫn supervisor vì entry
        // PDPT tương ứng giữ `US = 0` (bit US AND qua các cấp).
        if !pt
            .memory_mut()
            .write_entry(root, 0, X86_64::encode_table(pdpt))
        {
            return Err(MapError::CorruptTable);
        }
        // Ba entry **chia sẻ** của ảnh: `[0]` identity 1 GiB (→ `__pd`, `PD[0]` = `__pt_kernel`
        // của W^X), `[2]` MMIO 2..3 GiB (ECAM), `[3]` MMIO 3..4 GiB (LAPIC/IOAPIC).
        for index in [0usize, 2, 3] {
            let entry = asm::read_u64(boot_pdpt + (index as u64) * 8);
            if entry & PTE_PRESENT == 0 {
                continue; // ảnh không map vùng này — VSpace cũng không.
            }
            if !pt.memory_mut().write_entry(pdpt, index, entry) {
                return Err(MapError::CorruptTable);
            }
        }

        Ok(Self {
            id,
            pt,
            boot_root,
            active_on: AtomicU32::new(0),
            flush_seq: AtomicU64::new(0),
        })
    }

    /// Bitmask hart đang chạy VSpace này (`bit h` = hart `h`).
    pub fn active_on(&self) -> u32 {
        self.active_on.load(Ordering::Acquire)
    }

    /// Đánh dấu `hart` **đang chạy** VSpace này — gọi khi hart ấy nạp `CR3` bằng đường khác
    /// [`Self::activate`] (hart nhận cây bảng trang qua yêu cầu IPI, `KT-340d`).
    pub fn mark_active(&self, hart: usize) {
        if hart < ipi::MAX_HARTS {
            self.active_on.fetch_or(1u32 << hart, Ordering::Release);
        }
    }

    /// Bỏ đánh dấu `hart` (hart đã rời VSpace này).
    pub fn mark_inactive(&self, hart: usize) {
        if hart < ipi::MAX_HARTS {
            self.active_on.fetch_and(!(1u32 << hart), Ordering::Release);
        }
    }

    /// Nhãn của VSpace (chẩn đoán).
    pub const fn id(&self) -> u16 {
        self.id
    }

    /// PA bảng gốc (PML4) — giá trị ghi vào `CR3`.
    pub const fn root(&self) -> u64 {
        self.pt.root()
    }

    /// `seq` của lần shootdown **gần nhất** do VSpace này phát (`0` = chưa phát lần nào).
    ///
    /// Dùng cho bằng chứng `# tlb: … ack=<n>`: ACK của hart đích phải **đúng** số này
    /// ([`ipi::ack_ok`]) — "gửi xong" không bao giờ bị nhầm với "hart kia đã xoá TLB".
    pub fn flush_seq(&self) -> u64 {
        self.flush_seq.load(Ordering::Acquire)
    }

    /// `CR3` của ảnh tại thời điểm tạo — để [`Self::restore`] quay về.
    pub const fn boot_root(&self) -> u64 {
        self.boot_root
    }

    /// Bọc `vifast-pte` (đọc) — chẩn đoán/walk.
    pub const fn page_table(&self) -> &PageTable<X86_64, M> {
        &self.pt
    }

    /// Map **một trang 4 KiB** `va → pa` với quyền `perms`, rồi `INVLPG` đúng `va`.
    ///
    /// Mọi kiểm tra (căn lề, canonical 39-bit, PA, quyền, đã map) là của walker — không lặp lại.
    /// `INVLPG` **sau** khi ghi entry: nếu đổi mapping của một VA đang có entry TLB, thiếu lệnh
    /// này thì CPU vẫn dùng PA cũ.
    pub fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        self.pt.map(va, pa, perms)?;
        self.flush_va(va, ipi::MODE_OBSERVE);
        Ok(())
    }

    /// Như [`Self::map`] nhưng cho **MMIO** (`Device`: `PWT|PCD` = uncached).
    pub fn map_device(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        self.pt.map_with(
            va,
            pa,
            perms,
            MemAttr::Device,
            vifast_pte::Shareability::Inner,
        )?;
        self.flush_va(va, ipi::MODE_OBSERVE);
        Ok(())
    }

    /// Bỏ mapping của `va` (walker giải phóng bảng trung gian rỗng), rồi `INVLPG` đúng `va`.
    /// Trả PA cũ.
    pub fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        let pa = self.pt.unmap(va)?;
        // VA vừa **bỏ** map: hart nhận chỉ `invlpg`, **không** đọc lại (đọc sẽ fault).
        self.flush_va(va, 0);
        Ok(pa)
    }

    /// Dịch `va` (tra bảng trang thật của VSpace này).
    pub fn translate(&self, va: u64) -> Result<Translation, MapError> {
        self.pt.translate(va)
    }

    /// Walk chẩn đoán — bằng chứng quyền đọc **từ bảng trang**, không từ hằng mong đợi.
    pub fn walk(&self, va: u64) -> Walk {
        self.pt.walk(va)
    }

    /// Entry **lá** phủ `va` (bit thô của descriptor), `None` nếu chưa map. Dùng cho log
    /// `# vspace: code=0x… stack=0x…` (đọc `P/RW/US/NX` trực tiếp).
    pub fn leaf(&self, va: u64) -> Option<u64> {
        let walk = self.pt.walk(va);
        match walk.outcome {
            WalkOutcome::Leaf { .. } | WalkOutcome::Block { .. } => {
                walk.steps[walk.len - 1].map(|s| s.entry)
            }
            _ => None,
        }
    }

    /// Đổi `CR3` sang VSpace này và đánh dấu hart **đang chạy** nó. x86 xả TLB **mọi** entry
    /// không-`G`, đúng ngữ nghĩa "đổi không gian địa chỉ"; kernel vẫn chạy vì PDPT[0]/[2]/[3]
    /// được chia sẻ từ ảnh.
    pub fn activate(&self) {
        asm::write_cr3(self.root());
        if let Some(hart) = ipi::current_hart() {
            self.mark_active(hart);
        }
    }

    /// Đưa `CR3` về cây gốc của ảnh (gọi sau khi tác vụ ring 3 kết thúc, trước khi kernel chạy
    /// tiếp các pha không thuộc VSpace) và bỏ đánh dấu hart gọi.
    pub fn restore(&self) {
        if let Some(hart) = ipi::current_hart() {
            self.mark_inactive(hart);
        }
        asm::write_cr3(self.boot_root);
    }

    /// `INVLPG` **cục bộ** cho `va` rồi **shootdown** các hart khác đang chạy VSpace này
    /// (**KT-340d**). Tách hàm để đột biến `no-invlpg` chỉ **bỏ đúng lệnh `INVLPG` cục bộ** và
    /// `mut-no-shootdown` chỉ **bỏ đúng phần IPI** — hai cơ chế, hai đột biến độc lập.
    fn flush_va(&self, va: u64, mode: u64) {
        #[cfg(not(feature = "no-invlpg"))]
        {
            asm::invlpg(va);
        }
        #[cfg(feature = "no-invlpg")]
        {
            let _ = va;
        }
        // **Đột biến (i)** (`mut-no-shootdown`, KT-340d): **không** gửi `TlbFlush` tới hart khác
        // ⇒ TLB của hart kia còn ánh xạ cũ ⇒ nó **đọc dữ liệu cũ** ⇒ ca `# tlb: shootdown` ĐỎ.
        // Đây là đột biến trực tiếp của lỗi v1 N5; chỉ dùng cho kiểm, không phải sản phẩm.
        if cfg!(feature = "mut-no-shootdown") {
            return;
        }
        let me = match ipi::current_hart() {
            Some(hart) => hart,
            // Không biết mình là hart nào ⇒ **không** đoán: flush cục bộ đã xong, không gửi IPI
            // (gửi mù có thể self-target — D2-68). Trạng thái này không xảy ra trên ảnh (bảng
            // `LAPIC_ID` được công bố trước khi dùng `VSpace`).
            None => return,
        };
        let targets = ipi::shootdown_targets(self.active_on.load(Ordering::Acquire), me);
        if targets == 0 {
            return;
        }
        // `seq` lấy từ **một** nguồn toàn ảnh (`ipi::next_seq`) — hai bộ đếm độc lập sẽ trùng số
        // và một ACK cũ có thể thoả phép chờ của yêu cầu mới (xem `ipi::SEQ_SOURCE`).
        let seq = ipi::next_seq();
        self.flush_seq.store(seq, Ordering::Release);
        let _ = ipi::shootdown(targets, va, mode, seq);
    }
}
