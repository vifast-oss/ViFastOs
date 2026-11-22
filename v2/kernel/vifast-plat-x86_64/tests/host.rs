// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-x86_64` — KT-186 / D2-163 (K5' bước M2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64
//! ```
//!
//! Bằng chứng đo được của bước M2 (không boot, không QEMU):
//!
//! 1. **Descriptor**: từng bit `P/RW/US/PWT/PCD/A/D/PS/PAT/G/NX` + giá trị **tuyệt đối** tính tay
//!    từ Intel SDM Vol 3A (không so với hằng của crate — xem module [`intel_sdm`]).
//! 2. **Hình học**: 4 mức, 4 KiB, chỉ số từng mức tính tay; nửa cao dùng `root[0x1FF]`; cửa sổ
//!    canonical 39-bit (D2-147) và hình tối đa 48-bit.
//! 3. **CR0/CR4/EFER/MSR**: từng bit + địa chỉ MSR theo Intel SDM (module [`intel_sdm`]).
//! 4. **SysV AMD64 ABI**: thứ tự thanh ghi tham số/trả về/callee-saved, căn 16 byte, red zone
//!    128 byte (module [`sysv`]).
//! 5. **Map/translate 4 mức**: walk đi đủ bốn cấp, chỉ số tính tay; quyền + `MemAttr` round-trip.
//! 6. **Ca âm**: VA/PA lệch 4 KiB, VA ngoài cửa sổ canonical, PA vượt 48 bit, quyền không biểu
//!    diễn được, map chồng, unmap VA trống, block descriptor, entry reserved — tất cả trả lỗi
//!    **và không đổi bộ đếm frame**.
//! 7. **Không rò rỉ frame**: `alloc − free` sau unmap hết = 1 (gốc); sau `destroy` = 0; frame cấp
//!    lại bị bẩn vẫn được zero trước khi dùng.

use vifast_plat_x86_64::calling::{
    Reg, INT80_CLOBBERED, INT80_MR_REGS, INT80_RETURN_MSGINFO_REG, INT80_VECTOR, SYSCALL_ARG_REGS,
    SYSCALL_CLOBBERED, SYSCALL_MR_REGS, SYSCALL_MSGINFO_REG, SYSCALL_MSRS, SYSCALL_NUMBER_REG,
    SYSCALL_RETURN_ERROR_REG, SYSCALL_RETURN_MSGINFO_REG, SYSCALL_RETURN_VALUE_REG, SYSV_ARG_REGS,
    SYSV_CALLEE_SAVED, SYSV_CALLER_SAVED, SYSV_DIRECTION_FLAG_CLEAR, SYSV_FRAME_POINTER,
    SYSV_RED_ZONE_BYTES, SYSV_RETURN_REGS, SYSV_STACK_ALIGN_BYTES, SYSV_STACK_POINTER,
    SYSV_VARIADIC_VECTOR_COUNT_REG, SYSV_VECTOR_ARG_REGS, SYSV_VECTOR_RETURN_REGS,
};
use vifast_plat_x86_64::control::{
    CR0_AM, CR0_CD, CR0_EM, CR0_ET, CR0_MP, CR0_NE, CR0_NW, CR0_PE, CR0_PG, CR0_TS, CR0_WP,
    CR3_ADDR_MASK, CR3_PCD, CR3_PCID_MASK, CR3_PWT, CR4_CET, CR4_DE, CR4_FSGSBASE, CR4_LA57,
    CR4_MCE, CR4_OSFXSR, CR4_OSXMMEXCPT, CR4_OSXSAVE, CR4_PAE, CR4_PCE, CR4_PCIDE, CR4_PGE,
    CR4_PKE, CR4_PKS, CR4_PSE, CR4_PVI, CR4_SMAP, CR4_SMEP, CR4_SMXE, CR4_TSD, CR4_UMIP, CR4_VME,
    CR4_VMXE, EFER_FFXSR, EFER_LMA, EFER_LME, EFER_LMSLE, EFER_NXE, EFER_SCE, EFER_SVME, EFER_TCE,
    LONG_MODE_CR0, LONG_MODE_CR4, LONG_MODE_EFER, MSR_IA32_APIC_BASE, MSR_IA32_EFER,
    MSR_IA32_FMASK, MSR_IA32_FS_BASE, MSR_IA32_GS_BASE, MSR_IA32_KERNEL_GS_BASE, MSR_IA32_LSTAR,
    MSR_IA32_PAT, MSR_IA32_STAR,
};
use vifast_plat_x86_64::format::{
    is_canonical_bits, Descriptor, MemAttr, PageTableFormat, Perms, Shareability, IA32_PAT_DEFAULT,
    PTE_PAT, PTE_PCD, PTE_PRESENT, PTE_PS, PTE_PWT, X86_64, X86_64_ENTRIES, X86_64_INDEX_BITS,
    X86_64_LEVELS, X86_64_MAX_VA_BITS, X86_64_PAGE_BITS, X86_64_PAGE_BYTES, X86_64_PA_BITS,
    X86_64_VA_BITS,
};
use vifast_plat_x86_64::mmu::{
    MapError, Mmu, PageTable, TableMemory, Translation, WalkOutcome, X86Mmu,
};

/// Số entry một bảng trang của test (4 KiB / 8 B).
const ENTRIES: usize = 512;
/// Cỡ frame của test.
const FRAME_BYTES: u64 = 4096;
/// Rác bơm vào frame lúc free — để chốt "frame cấp lại phải được zero".
const GARBAGE: u64 = 0xDEAD_BEEF_DEAD_BEEF;
/// PA gốc của nguồn frame test (4 KiB aligned).
const FRAME_BASE: u64 = 0x4000_0000;
/// PA mẫu (4 KiB aligned).
const PA: u64 = 0x0000_0000_8000_1000;
/// VA mẫu nửa thấp (canonical 39-bit).
const VA: u64 = 0x0000_0000_0000_1000;
/// VA nửa cao 39-bit canonical (bit 38 = 1, bit 63:39 = 1).
const VA_HIGH: u64 = 0xFFFF_FFC0_0000_0000;

// ---------------------------------------------------------------- nguồn frame test

/// Nguồn frame + bộ nhớ bảng trang cho host: `Vec` frame 4 KiB, free-list LIFO, đếm alloc/free.
///
/// Cố ý **không** zero ở `alloc_frame` (đúng hợp đồng [`TableMemory`]): frame tái sử dụng còn
/// rác [`GARBAGE`] từ `free_frame`, nên nếu `Mmu` quên `zero_frame` thì test đỏ ngay.
struct TestMemory {
    base: u64,
    frames: Vec<Option<Box<[u64; ENTRIES]>>>,
    free: Vec<usize>,
    allocs: usize,
    frees: usize,
    fail_after: Option<usize>,
}

impl TestMemory {
    fn new(base: u64) -> Self {
        Self {
            base,
            frames: Vec::new(),
            free: Vec::new(),
            allocs: 0,
            frees: 0,
            fail_after: None,
        }
    }

    fn with_fail_after(base: u64, limit: usize) -> Self {
        let mut memory = Self::new(base);
        memory.fail_after = Some(limit);
        memory
    }

    fn outstanding(&self) -> usize {
        self.allocs - self.frees
    }

    fn index_of(&self, pa: u64) -> Option<usize> {
        let off = pa.checked_sub(self.base)?;
        if off % FRAME_BYTES != 0 {
            return None;
        }
        let index = (off / FRAME_BYTES) as usize;
        if index < self.frames.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl TableMemory for TestMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        if let Some(limit) = self.fail_after {
            if self.allocs >= limit {
                return None;
            }
        }
        let index = match self.free.pop() {
            // Tái sử dụng: giữ nguyên rác — `Mmu` phải tự zero.
            Some(index) => index,
            None => {
                self.frames.push(Some(Box::new([0u64; ENTRIES])));
                self.frames.len() - 1
            }
        };
        self.allocs += 1;
        Some(self.base + index as u64 * FRAME_BYTES)
    }

    fn free_frame(&mut self, pa: u64) {
        let index = self.index_of(pa).expect("free_frame: PA lạ");
        assert!(
            !self.free.contains(&index),
            "free_frame: double free frame {pa:#x}"
        );
        if let Some(frame) = self.frames[index].as_mut() {
            for entry in frame.iter_mut() {
                *entry = GARBAGE;
            }
        }
        self.frees += 1;
        self.free.push(index);
    }

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        if index >= ENTRIES {
            return None;
        }
        let frame = self.index_of(pa)?;
        self.frames
            .get(frame)?
            .as_ref()
            .map(|entries| entries[index])
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        if index >= ENTRIES {
            return false;
        }
        let Some(frame) = self.index_of(pa) else {
            return false;
        };
        match self.frames.get_mut(frame) {
            Some(Some(entries)) => {
                entries[index] = entry;
                true
            }
            _ => false,
        }
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        let Some(frame) = self.index_of(pa) else {
            return false;
        };
        match self.frames.get_mut(frame) {
            Some(Some(entries)) => {
                for entry in entries.iter_mut() {
                    *entry = 0;
                }
                true
            }
            _ => false,
        }
    }
}

fn new_mmu() -> PageTable<X86_64, TestMemory> {
    PageTable::new(TestMemory::new(FRAME_BASE)).expect("cấp frame gốc")
}

// ------------------------------------------------- 1. bit **literal** Intel SDM

/// Vị trí bit **chép tay từ Intel SDM Vol 3A** (ch. 4 "Paging", mục "4-Level Paging and 5-Level
/// Paging"), **cố ý không** lấy từ hằng `PTE_*` của crate.
///
/// Vì sao có khối này: một test chỉ so bit bằng **chính** hằng mà encoder dùng là tautology —
/// đổi vị trí bit thì encoder và test cùng đổi theo và vẫn xanh (bài học KT-166). Bảng dưới là
/// nguồn **độc lập** để neo encoder vào đặc tả: đột biến vị trí bit **phải** làm test đỏ.
mod intel_sdm {
    /// `P` — bit 0.
    pub const P: u64 = 1 << 0;
    /// `RW` — bit 1.
    pub const RW: u64 = 1 << 1;
    /// `US` — bit 2.
    pub const US: u64 = 1 << 2;
    /// `PWT` — bit 3.
    pub const PWT: u64 = 1 << 3;
    /// `PCD` — bit 4.
    pub const PCD: u64 = 1 << 4;
    /// `A` — bit 5.
    pub const A: u64 = 1 << 5;
    /// `D` — bit 6.
    pub const D: u64 = 1 << 6;
    /// `PS` (mức 0..2) / `PAT` (mức 3) — bit 7.
    pub const PS_OR_PAT: u64 = 1 << 7;
    /// `G` — bit 8.
    pub const G: u64 = 1 << 8;
    /// `PAT` của lá 4 KiB — bit 7 (cùng vị trí `PS` ở mức 0..2).
    pub const PAT: u64 = 1 << 7;
    /// `NX` — bit 63.
    pub const NX: u64 = 1 << 63;
    /// Địa chỉ vật lý bắt đầu ở bit 12; PA 48-bit.
    pub const ADDR_SHIFT: u32 = 12;
    pub const ADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;
    /// Bit reserved 62:52.
    pub const RESERVED: u64 = 0x7FF0_0000_0000_0000;

    /// `CR0` — Intel SDM Vol 3A, mục "Control Registers".
    pub const CR0_PE: u64 = 1 << 0;
    pub const CR0_MP: u64 = 1 << 1;
    pub const CR0_EM: u64 = 1 << 2;
    pub const CR0_TS: u64 = 1 << 3;
    pub const CR0_ET: u64 = 1 << 4;
    pub const CR0_NE: u64 = 1 << 5;
    pub const CR0_WP: u64 = 1 << 16;
    pub const CR0_AM: u64 = 1 << 18;
    pub const CR0_NW: u64 = 1 << 29;
    pub const CR0_CD: u64 = 1 << 30;
    pub const CR0_PG: u64 = 1 << 31;

    /// `CR4` — Intel SDM Vol 3A, mục "Control Registers".
    pub const CR4_VME: u64 = 1 << 0;
    pub const CR4_PVI: u64 = 1 << 1;
    pub const CR4_TSD: u64 = 1 << 2;
    pub const CR4_DE: u64 = 1 << 3;
    pub const CR4_PSE: u64 = 1 << 4;
    pub const CR4_PAE: u64 = 1 << 5;
    pub const CR4_MCE: u64 = 1 << 6;
    pub const CR4_PGE: u64 = 1 << 7;
    pub const CR4_PCE: u64 = 1 << 8;
    pub const CR4_OSFXSR: u64 = 1 << 9;
    pub const CR4_OSXMMEXCPT: u64 = 1 << 10;
    pub const CR4_UMIP: u64 = 1 << 11;
    pub const CR4_LA57: u64 = 1 << 12;
    pub const CR4_VMXE: u64 = 1 << 13;
    pub const CR4_SMXE: u64 = 1 << 14;
    pub const CR4_FSGSBASE: u64 = 1 << 16;
    pub const CR4_PCIDE: u64 = 1 << 17;
    pub const CR4_OSXSAVE: u64 = 1 << 18;
    pub const CR4_SMEP: u64 = 1 << 20;
    pub const CR4_SMAP: u64 = 1 << 21;
    pub const CR4_PKE: u64 = 1 << 22;
    pub const CR4_CET: u64 = 1 << 23;
    pub const CR4_PKS: u64 = 1 << 24;

    /// `IA32_EFER` — Intel SDM Vol 3A, mục "Extended Feature Enable Register (IA32_EFER)".
    pub const EFER_SCE: u64 = 1 << 0;
    pub const EFER_LME: u64 = 1 << 8;
    pub const EFER_LMA: u64 = 1 << 10;
    pub const EFER_NXE: u64 = 1 << 11;
    pub const EFER_SVME: u64 = 1 << 12;
    pub const EFER_LMSLE: u64 = 1 << 13;
    pub const EFER_FFXSR: u64 = 1 << 14;
    pub const EFER_TCE: u64 = 1 << 15;

    /// MSR — Intel SDM Vol 4, ch. 2 "Model-Specific Registers (MSRs)".
    pub const MSR_EFER: u32 = 0xC000_0080;
    pub const MSR_STAR: u32 = 0xC000_0081;
    pub const MSR_LSTAR: u32 = 0xC000_0082;
    pub const MSR_FMASK: u32 = 0xC000_0084;
    pub const MSR_FS_BASE: u32 = 0xC000_0100;
    pub const MSR_GS_BASE: u32 = 0xC000_0101;
    pub const MSR_KERNEL_GS_BASE: u32 = 0xC000_0102;
    pub const MSR_PAT: u32 = 0x0000_0277;
    pub const MSR_APIC_BASE: u32 = 0x0000_001B;
}

/// Descriptor leaf phải khớp **giá trị tuyệt đối** tính tay từ Intel SDM — không so với hằng
/// của crate. Đột biến `PTE_PRESENT 1<<0 → 1<<1`, `PTE_NX 1<<63 → 1<<62`, hay đổi `PWT/PCD`
/// của `Device` làm test này **ĐỎ**.
#[test]
fn descriptor_bits_literal_intel_sdm() {
    // KERNEL_RW, Normal (WB), PA = 0x8000_1000:
    // P|RW|A|D|NX|PA = 0x1|0x2|0x20|0x40|NX|PA.
    let d = X86_64::encode_leaf(PA, Perms::KERNEL_RW, MemAttr::Normal, Shareability::Inner);
    assert_eq!(
        d,
        intel_sdm::P
            | intel_sdm::RW
            | intel_sdm::A
            | intel_sdm::D
            | intel_sdm::NX
            | (PA & intel_sdm::ADDR_MASK),
        "leaf KERNEL_RW phải đúng từng bit Intel SDM"
    );
    assert_eq!(
        d, 0x8000_0000_8000_1063,
        "giá trị tuyệt đối của leaf KERNEL_RW (tính tay từ đặc tả)"
    );
    assert_eq!(d & intel_sdm::P, intel_sdm::P, "P = 1 ở bit 0");
    assert_eq!(d & intel_sdm::RW, intel_sdm::RW, "RW = 1 ở bit 1");
    assert_eq!(d & intel_sdm::US, 0, "kernel: US = 0 ở bit 2");
    assert_eq!(d & intel_sdm::PWT, 0, "Normal: PWT = 0 ở bit 3");
    assert_eq!(d & intel_sdm::PCD, 0, "Normal: PCD = 0 ở bit 4");
    assert_eq!(d & intel_sdm::A, intel_sdm::A, "A = 1 ở bit 5");
    assert_eq!(d & intel_sdm::D, intel_sdm::D, "D = 1 ở bit 6");
    assert_eq!(d & intel_sdm::PS_OR_PAT, 0, "lá 4 KiB: PS = 0 ở bit 7");
    assert_eq!(d & intel_sdm::G, 0, "bước M2: G = 0 ở bit 8");
    assert_eq!(d & intel_sdm::PAT, 0, "bước M2: PAT = 0 ở bit 7");
    assert_eq!(d & intel_sdm::NX, intel_sdm::NX, "NX = 1 ở bit 63");
    assert_eq!(d & intel_sdm::RESERVED, 0, "bit 62:52 phải bằng 0");
    assert_eq!(
        (d >> intel_sdm::ADDR_SHIFT) & (intel_sdm::ADDR_MASK >> intel_sdm::ADDR_SHIFT),
        PA >> intel_sdm::ADDR_SHIFT,
        "PA nằm ở bit 47:12, không đè trường thuộc tính"
    );

    // USER_RX, Normal: P|US|A|D|PA (RW = 0, NX = 0).
    let u = X86_64::encode_leaf(PA, Perms::USER_RX, MemAttr::Normal, Shareability::Inner);
    assert_eq!(
        u,
        intel_sdm::P | intel_sdm::US | intel_sdm::A | intel_sdm::D | (PA & intel_sdm::ADDR_MASK),
        "leaf USER_RX phải đúng từng bit Intel SDM"
    );
    assert_eq!(
        u, 0x0000_0000_8000_1065,
        "giá trị tuyệt đối của leaf USER_RX (tính tay từ đặc tả)"
    );
    assert_eq!(u & intel_sdm::RW, 0, "user RX: RW = 0 (chỉ đọc)");
    assert_eq!(u & intel_sdm::US, intel_sdm::US, "user: US = 1");
    assert_eq!(u & intel_sdm::NX, 0, "user exec: NX = 0");

    // KERNEL_RX, Normal: P|A|D|PA (RW = 0, NX = 0, US = 0).
    let kx = X86_64::encode_leaf(PA, Perms::KERNEL_RX, MemAttr::Normal, Shareability::Inner);
    assert_eq!(
        kx,
        intel_sdm::P | intel_sdm::A | intel_sdm::D | (PA & intel_sdm::ADDR_MASK)
    );
    assert_eq!(kx, 0x0000_0000_8000_1061);

    // Device (UC): P|RW|PWT|PCD|A|D|NX|PA.
    let dev = X86_64::encode_leaf(PA, Perms::KERNEL_RW, MemAttr::Device, Shareability::Inner);
    assert_eq!(
        dev,
        intel_sdm::P
            | intel_sdm::RW
            | intel_sdm::PWT
            | intel_sdm::PCD
            | intel_sdm::A
            | intel_sdm::D
            | intel_sdm::NX
            | (PA & intel_sdm::ADDR_MASK),
        "Device phải đặt PWT|PCD (UC) — Intel SDM, mục PAT"
    );
    assert_eq!(dev, 0x8000_0000_8000_107B);
    assert_eq!(dev & intel_sdm::PWT, intel_sdm::PWT, "Device: PWT = 1");
    assert_eq!(dev & intel_sdm::PCD, intel_sdm::PCD, "Device: PCD = 1");

    // Table descriptor: P|RW|US|PA (US = 1 để lá user dưới bảng vào được).
    let t = X86_64::encode_table(0x4000_0000);
    assert_eq!(
        t,
        intel_sdm::P | intel_sdm::RW | intel_sdm::US | 0x4000_0000,
        "table descriptor = P|RW|US|PA"
    );
    assert_eq!(t, 0x0000_0000_4000_0007);
    assert_eq!(X86_64::decode(t, 0), Descriptor::Table { pa: 0x4000_0000 });
    assert_eq!(X86_64::decode(t, 1), Descriptor::Table { pa: 0x4000_0000 });
    assert_eq!(X86_64::decode(t, 2), Descriptor::Table { pa: 0x4000_0000 });
    // Ở mức 3 (PT), P = 1 **luôn** là lá; bit 7 là PAT, không phải PS.
    assert!(matches!(
        X86_64::decode(t, 3),
        Descriptor::Leaf {
            pa: 0x4000_0000,
            ..
        }
    ));

    // Entry trống.
    assert_eq!(X86_64::decode(0, 0), Descriptor::Invalid);
    assert_eq!(X86_64::decode(0, 3), Descriptor::Invalid);
}

#[test]
fn descriptor_ps_bit_by_level() {
    // PS = 1 ở mức 1 ⇒ block 1 GiB; mức 2 ⇒ block 2 MiB; mức 0 ⇒ reserved (không có 512 GiB).
    let block = intel_sdm::P | intel_sdm::PS_OR_PAT | 0x4000_0000;
    // KT-305: `Descriptor::Block` mang thêm `perms` (giải mã từ `RW`/`US`/`NX` của entry).
    // Entry chỉ có `P | PS` ⇒ đọc + thực thi, không ghi, không user = `KERNEL_RX`.
    assert_eq!(
        X86_64::decode(block, 1),
        Descriptor::Block {
            pa: 0x4000_0000,
            level: 1,
            perms: Perms::KERNEL_RX
        }
    );
    assert_eq!(
        X86_64::decode(block, 2),
        Descriptor::Block {
            pa: 0x4000_0000,
            level: 2,
            perms: Perms::KERNEL_RX
        }
    );
    assert_eq!(X86_64::decode(block, 0), Descriptor::Reserved);

    // Ở mức 3, bit 7 là PAT: entry P|bit7 là **lá** với PAT = 1, không phải block.
    let pat_leaf = intel_sdm::P | intel_sdm::PS_OR_PAT | PA;
    assert!(matches!(
        X86_64::decode(pat_leaf, 3),
        Descriptor::Leaf { .. }
    ));
    assert!(X86_64::leaf_pat(pat_leaf), "bit 7 ở mức 3 là PAT");
    assert!(!X86_64::leaf_pat(intel_sdm::P | PA));

    // Bit reserved 62:52 khác 0 ⇒ Reserved (không được coi là địa chỉ).
    let reserved = intel_sdm::P | (1u64 << 52) | PA;
    assert_eq!(X86_64::decode(reserved, 3), Descriptor::Reserved);
    assert_eq!(X86_64::decode(reserved, 0), Descriptor::Reserved);
}

#[test]
fn geometry_4_levels() {
    assert_eq!(X86_64_LEVELS, 4);
    assert_eq!(X86_64_PAGE_BITS, 12);
    assert_eq!(X86_64_INDEX_BITS, 9);
    assert_eq!(X86_64_VA_BITS, 39);
    assert_eq!(X86_64_MAX_VA_BITS, 48);
    assert_eq!(X86_64_PA_BITS, 48);
    assert_eq!(X86_64_PAGE_BYTES, 4096);
    assert_eq!(X86_64_ENTRIES, 512);
    assert_eq!(<X86_64 as PageTableFormat>::LEVELS, 4);
    assert_eq!(<X86_64 as PageTableFormat>::PAGE_BYTES, 4096);
    assert_eq!(<X86_64 as PageTableFormat>::ENTRIES, 512);

    // VA 0x1000: chỉ số [0, 0, 0, 1] — tính tay từ hình 9/9/9/12.
    assert_eq!(X86_64::index(VA, 0), 0);
    assert_eq!(X86_64::index(VA, 1), 0);
    assert_eq!(X86_64::index(VA, 2), 0);
    assert_eq!(X86_64::index(VA, 3), 1);
    // Nửa cao 39-bit: PML4 = 0x1FF (bit 47:39), PDPT = 0x100 (bit 38 bật), PD/PT = 0.
    assert_eq!(X86_64::index(VA_HIGH, 0), 0x1FF);
    assert_eq!(X86_64::index(VA_HIGH, 1), 0x100);
    assert_eq!(X86_64::index(VA_HIGH, 2), 0);
    assert_eq!(X86_64::index(VA_HIGH, 3), 0);
    // Ngoài dải mức ⇒ 0, không panic.
    assert_eq!(X86_64::index(VA, 4), 0);
    assert_eq!(X86_64::index(VA, usize::MAX), 0);

    // Vùng phủ mỗi mức: 2^39, 2^30, 2^21, 2^12.
    assert_eq!(X86_64::span(0), 1u64 << 39);
    assert_eq!(X86_64::span(1), 1u64 << 30);
    assert_eq!(X86_64::span(2), 1u64 << 21);
    assert_eq!(X86_64::span(3), 1u64 << 12);
    assert_eq!(X86_64::span(4), 0);
}

#[test]
fn canonical_39_bit_window() {
    // Nửa thấp và nửa cao 39-bit canonical.
    assert!(X86_64::is_canonical(VA));
    assert!(X86_64::is_canonical(VA_HIGH));
    assert!(X86_64::is_canonical(0xFFFF_FFFF_FFFF_F000));
    // bit 39 bật nhưng bit 38 tắt ⇒ không canonical 39-bit.
    assert!(!X86_64::is_canonical(0x0000_0080_0000_0000));
    // bit 38 tắt nhưng bit 63:39 bật ⇒ không canonical.
    assert!(!X86_64::is_canonical(0xFFFF_FF80_0000_0000));
    // 48-bit canonical nhưng **ngoài** cửa sổ 39-bit của kernel (bit 38 = 1, bit 63:39 = 0).
    assert!(!X86_64::is_canonical(0x0000_7FFF_FFFF_F000));
    // Chứng minh cửa sổ 39-bit là **tập con** của hình phần cứng 48-bit.
    assert!(is_canonical_bits(0x0000_7FFF_FFFF_F000, 48));
    assert!(is_canonical_bits(VA_HIGH, 48));
}

// ------------------------------------------------- 2. CR0/CR4/EFER/MSR literal

#[test]
fn control_bits_literal_intel_sdm() {
    assert_eq!(CR0_PE, intel_sdm::CR0_PE);
    assert_eq!(CR0_MP, intel_sdm::CR0_MP);
    assert_eq!(CR0_EM, intel_sdm::CR0_EM);
    assert_eq!(CR0_TS, intel_sdm::CR0_TS);
    assert_eq!(CR0_ET, intel_sdm::CR0_ET);
    assert_eq!(CR0_NE, intel_sdm::CR0_NE);
    assert_eq!(CR0_WP, intel_sdm::CR0_WP);
    assert_eq!(CR0_AM, intel_sdm::CR0_AM);
    assert_eq!(CR0_NW, intel_sdm::CR0_NW);
    assert_eq!(CR0_CD, intel_sdm::CR0_CD);
    assert_eq!(CR0_PG, intel_sdm::CR0_PG);
    assert_eq!(CR0_PG, 0x8000_0000, "PG ở bit 31");
    assert_eq!(CR0_WP, 0x0001_0000, "WP ở bit 16");

    assert_eq!(CR4_VME, intel_sdm::CR4_VME);
    assert_eq!(CR4_PVI, intel_sdm::CR4_PVI);
    assert_eq!(CR4_TSD, intel_sdm::CR4_TSD);
    assert_eq!(CR4_DE, intel_sdm::CR4_DE);
    assert_eq!(CR4_PSE, intel_sdm::CR4_PSE);
    assert_eq!(CR4_PAE, intel_sdm::CR4_PAE);
    assert_eq!(CR4_MCE, intel_sdm::CR4_MCE);
    assert_eq!(CR4_PGE, intel_sdm::CR4_PGE);
    assert_eq!(CR4_PCE, intel_sdm::CR4_PCE);
    assert_eq!(CR4_OSFXSR, intel_sdm::CR4_OSFXSR);
    assert_eq!(CR4_OSXMMEXCPT, intel_sdm::CR4_OSXMMEXCPT);
    assert_eq!(CR4_UMIP, intel_sdm::CR4_UMIP);
    assert_eq!(CR4_LA57, intel_sdm::CR4_LA57);
    assert_eq!(CR4_VMXE, intel_sdm::CR4_VMXE);
    assert_eq!(CR4_SMXE, intel_sdm::CR4_SMXE);
    assert_eq!(CR4_FSGSBASE, intel_sdm::CR4_FSGSBASE);
    assert_eq!(CR4_PCIDE, intel_sdm::CR4_PCIDE);
    assert_eq!(CR4_OSXSAVE, intel_sdm::CR4_OSXSAVE);
    assert_eq!(CR4_SMEP, intel_sdm::CR4_SMEP);
    assert_eq!(CR4_SMAP, intel_sdm::CR4_SMAP);
    assert_eq!(CR4_PKE, intel_sdm::CR4_PKE);
    assert_eq!(CR4_CET, intel_sdm::CR4_CET);
    assert_eq!(CR4_PKS, intel_sdm::CR4_PKS);
    assert_eq!(CR4_PAE, 0x20, "PAE ở bit 5");
    assert_eq!(CR4_SMEP, 0x0010_0000, "SMEP ở bit 20");
    assert_eq!(CR4_SMAP, 0x0020_0000, "SMAP ở bit 21");

    assert_eq!(EFER_SCE, intel_sdm::EFER_SCE);
    assert_eq!(EFER_LME, intel_sdm::EFER_LME);
    assert_eq!(EFER_LMA, intel_sdm::EFER_LMA);
    assert_eq!(EFER_NXE, intel_sdm::EFER_NXE);
    assert_eq!(EFER_SVME, intel_sdm::EFER_SVME);
    assert_eq!(EFER_LMSLE, intel_sdm::EFER_LMSLE);
    assert_eq!(EFER_FFXSR, intel_sdm::EFER_FFXSR);
    assert_eq!(EFER_TCE, intel_sdm::EFER_TCE);
    assert_eq!(EFER_LME, 0x100, "LME ở bit 8");
    assert_eq!(EFER_LMA, 0x400, "LMA ở bit 10");
    assert_eq!(EFER_NXE, 0x800, "NXE ở bit 11");

    // Thứ tự bật long mode (Intel SDM Vol 3A §9.8.5): PE|PG, PAE, LME.
    assert_eq!(LONG_MODE_CR0, intel_sdm::CR0_PE | intel_sdm::CR0_PG);
    assert_eq!(LONG_MODE_CR4, intel_sdm::CR4_PAE);
    assert_eq!(LONG_MODE_EFER, intel_sdm::EFER_LME);
    assert_eq!(LONG_MODE_CR0, 0x8000_0001);
    assert_eq!(LONG_MODE_CR4, 0x20);
    assert_eq!(LONG_MODE_EFER, 0x100);
}

#[test]
fn msr_addresses_literal_intel_sdm() {
    assert_eq!(MSR_IA32_EFER, intel_sdm::MSR_EFER);
    assert_eq!(MSR_IA32_STAR, intel_sdm::MSR_STAR);
    assert_eq!(MSR_IA32_LSTAR, intel_sdm::MSR_LSTAR);
    assert_eq!(MSR_IA32_FMASK, intel_sdm::MSR_FMASK);
    assert_eq!(MSR_IA32_FS_BASE, intel_sdm::MSR_FS_BASE);
    assert_eq!(MSR_IA32_GS_BASE, intel_sdm::MSR_GS_BASE);
    assert_eq!(MSR_IA32_KERNEL_GS_BASE, intel_sdm::MSR_KERNEL_GS_BASE);
    assert_eq!(MSR_IA32_PAT, intel_sdm::MSR_PAT);
    assert_eq!(MSR_IA32_APIC_BASE, intel_sdm::MSR_APIC_BASE);
    assert_eq!(MSR_IA32_EFER, 0xC000_0080);
    assert_eq!(MSR_IA32_STAR, 0xC000_0081);
    assert_eq!(MSR_IA32_LSTAR, 0xC000_0082);
    assert_eq!(MSR_IA32_FMASK, 0xC000_0084);
    assert_eq!(MSR_IA32_PAT, 0x277);

    // `syscall` cần đúng ba MSR này (bước M7).
    assert_eq!(
        SYSCALL_MSRS,
        [MSR_IA32_STAR, MSR_IA32_LSTAR, MSR_IA32_FMASK]
    );

    // CR3: PCID 12 bit ở bit 11:0, địa chỉ PML4 ở bit 51:12.
    assert_eq!(CR3_PCD, 1 << 3);
    assert_eq!(CR3_PWT, 1 << 4);
    assert_eq!(CR3_PCID_MASK, 0xFFF);
    assert_eq!(CR3_ADDR_MASK, 0x000F_FFFF_FFFF_F000);
}

#[test]
fn pat_default_entries() {
    // Intel SDM Vol 3A, mục PAT: PAT0=WB(0x06), PAT1=WT(0x04), PAT2=UC-(0x07), PAT3=UC(0x00),
    // PAT4..7 lặp lại.
    assert_eq!(IA32_PAT_DEFAULT, 0x0007_0406_0007_0406);
    let expected = [0x06u64, 0x04, 0x07, 0x00, 0x06, 0x04, 0x07, 0x00];
    for (index, want) in expected.iter().enumerate() {
        assert_eq!(
            (IA32_PAT_DEFAULT >> (8 * index)) & 0xFF,
            *want,
            "PAT{index} phải là {want:#04x}"
        );
    }
    // Device = PWT|PCD|PAT=0 ⇒ index 3 ⇒ UC (0x00); Normal = 0 ⇒ index 0 ⇒ WB (0x06).
    let dev = X86_64::encode_leaf(PA, Perms::KERNEL_RW, MemAttr::Device, Shareability::Inner);
    let normal = X86_64::encode_leaf(PA, Perms::KERNEL_RW, MemAttr::Normal, Shareability::Inner);
    let dev_index = ((dev & PTE_PAT != 0) as u64) << 2
        | ((dev & PTE_PWT != 0) as u64) << 1
        | ((dev & PTE_PCD != 0) as u64);
    let normal_index = ((normal & PTE_PAT != 0) as u64) << 2
        | ((normal & PTE_PWT != 0) as u64) << 1
        | ((normal & PTE_PCD != 0) as u64);
    assert_eq!(dev_index, 3, "Device dùng PAT index 3");
    assert_eq!(normal_index, 0, "Normal dùng PAT index 0");
    assert_eq!((IA32_PAT_DEFAULT >> (8 * dev_index)) & 0xFF, 0x00, "UC");
    assert_eq!((IA32_PAT_DEFAULT >> (8 * normal_index)) & 0xFF, 0x06, "WB");
}

// ------------------------------------------------- 3. SysV AMD64 ABI literal

/// Bảng thanh ghi **chép tay từ System V AMD64 ABI** (x86-64 psABI §3.2.1/§3.2.2/§3.2.3),
/// **cố ý không** lấy từ hằng của crate.
mod sysv {
    /// Mã hoá phần cứng của 16 thanh ghi đa dụng (Intel SDM Vol 2A, mục "Registers").
    pub const RAX: u8 = 0;
    pub const RCX: u8 = 1;
    pub const RDX: u8 = 2;
    pub const RBX: u8 = 3;
    pub const RSP: u8 = 4;
    pub const RBP: u8 = 5;
    pub const RSI: u8 = 6;
    pub const RDI: u8 = 7;
    pub const R8: u8 = 8;
    pub const R9: u8 = 9;
    pub const R10: u8 = 10;
    pub const R11: u8 = 11;
    pub const R12: u8 = 12;
    pub const R13: u8 = 13;
    pub const R14: u8 = 14;
    pub const R15: u8 = 15;

    /// psABI §3.2.3: tham số nguyên theo thứ tự RDI, RSI, RDX, RCX, R8, R9.
    pub const ARG: [u8; 6] = [RDI, RSI, RDX, RCX, R8, R9];
    /// psABI §3.2.3 "Returning of Values": RAX, RDX.
    pub const RETURN: [u8; 2] = [RAX, RDX];
    /// psABI §3.2.1: callee-saved RBX, RBP, R12–R15.
    pub const CALLEE_SAVED: [u8; 6] = [RBX, RBP, R12, R13, R14, R15];
    /// psABI §3.2.1: caller-saved (scratch) — phần còn lại trừ RSP.
    pub const CALLER_SAVED: [u8; 9] = [RAX, RCX, RDX, RSI, RDI, R8, R9, R10, R11];
    /// psABI §3.2.1: `DF` phải bằng 0 khi vào/ra hàm.
    pub const DF_CLEAR: bool = true;
    /// psABI §3.2.2: căn 16 byte.
    pub const STACK_ALIGN: u64 = 16;
    /// psABI §3.2.2: red zone 128 byte.
    pub const RED_ZONE: u64 = 128;
    /// psABI §3.2.3: XMM0–XMM7 nhận tham số vector.
    pub const VECTOR_ARGS: u8 = 8;
    /// psABI §3.2.3: XMM0/XMM1 trả về.
    pub const VECTOR_RETURNS: u8 = 2;
}

#[test]
fn sysv_register_table_literal() {
    // Mã hoá phần cứng 0..15.
    assert_eq!(Reg::Rax.number(), sysv::RAX);
    assert_eq!(Reg::Rcx.number(), sysv::RCX);
    assert_eq!(Reg::Rdx.number(), sysv::RDX);
    assert_eq!(Reg::Rbx.number(), sysv::RBX);
    assert_eq!(Reg::Rsp.number(), sysv::RSP);
    assert_eq!(Reg::Rbp.number(), sysv::RBP);
    assert_eq!(Reg::Rsi.number(), sysv::RSI);
    assert_eq!(Reg::Rdi.number(), sysv::RDI);
    assert_eq!(Reg::R8.number(), sysv::R8);
    assert_eq!(Reg::R9.number(), sysv::R9);
    assert_eq!(Reg::R10.number(), sysv::R10);
    assert_eq!(Reg::R11.number(), sysv::R11);
    assert_eq!(Reg::R12.number(), sysv::R12);
    assert_eq!(Reg::R13.number(), sysv::R13);
    assert_eq!(Reg::R14.number(), sysv::R14);
    assert_eq!(Reg::R15.number(), sysv::R15);

    // Tên thanh ghi.
    assert_eq!(Reg::Rax.name(), "RAX");
    assert_eq!(Reg::Rdi.name(), "RDI");
    assert_eq!(Reg::R15.name(), "R15");

    // Thứ tự tham số / trả về / callee-saved.
    let arg_numbers: Vec<u8> = SYSV_ARG_REGS.iter().map(|r| r.number()).collect();
    assert_eq!(
        arg_numbers,
        sysv::ARG,
        "tham số SysV: RDI, RSI, RDX, RCX, R8, R9"
    );
    let return_numbers: Vec<u8> = SYSV_RETURN_REGS.iter().map(|r| r.number()).collect();
    assert_eq!(return_numbers, sysv::RETURN, "trả về SysV: RAX, RDX");
    let callee_numbers: Vec<u8> = SYSV_CALLEE_SAVED.iter().map(|r| r.number()).collect();
    assert_eq!(
        callee_numbers,
        sysv::CALLEE_SAVED,
        "callee-saved SysV: RBX, RBP, R12–R15"
    );
    let caller_numbers: Vec<u8> = SYSV_CALLER_SAVED.iter().map(|r| r.number()).collect();
    assert_eq!(caller_numbers, sysv::CALLER_SAVED, "caller-saved SysV");

    // Hai tập rời nhau và phủ đủ 15 thanh ghi (trừ RSP).
    for callee in SYSV_CALLEE_SAVED {
        assert!(
            !SYSV_CALLER_SAVED.contains(&callee),
            "{callee:?} không được vừa callee-saved vừa caller-saved"
        );
    }
    let mut seen = [false; 16];
    for reg in SYSV_CALLEE_SAVED.iter().chain(SYSV_CALLER_SAVED.iter()) {
        assert!(!seen[reg.number() as usize], "thanh ghi lặp");
        seen[reg.number() as usize] = true;
    }
    assert!(!seen[sysv::RSP as usize], "RSP không nằm trong hai tập");
    assert_eq!(seen.iter().filter(|seen| **seen).count(), 15);

    // Căn ngăn xếp + red zone + vector.
    assert_eq!(SYSV_STACK_ALIGN_BYTES, sysv::STACK_ALIGN);
    assert_eq!(SYSV_RED_ZONE_BYTES, sysv::RED_ZONE);
    assert_eq!(SYSV_VECTOR_ARG_REGS, sysv::VECTOR_ARGS);
    assert_eq!(SYSV_VECTOR_RETURN_REGS, sysv::VECTOR_RETURNS);
    assert_eq!(SYSV_VARIADIC_VECTOR_COUNT_REG, Reg::Rax, "AL giữ số vector");
    assert_eq!(SYSV_DIRECTION_FLAG_CLEAR, sysv::DF_CLEAR, "DF phải bằng 0");
    assert_eq!(SYSV_STACK_POINTER, Reg::Rsp);
    assert_eq!(SYSV_FRAME_POINTER, Reg::Rbp);
}

#[test]
fn syscall_proposal_registers() {
    // **KT-270 / D2-207**: `spec/abi.toml [calling.x86_64]` đã mở — các hằng dưới đây khớp
    // đúng spec (nguồn sự thật vẫn là `spec/`, D2-11).
    assert_eq!(SYSCALL_NUMBER_REG, Reg::Rax);
    assert_eq!(SYSCALL_ARG_REGS, [Reg::Rdi, Reg::Rsi]);
    assert_eq!(SYSCALL_MSGINFO_REG, Reg::Rdx);
    assert_eq!(SYSCALL_MR_REGS, [Reg::R10, Reg::R8, Reg::R9, Reg::R12]);
    assert_eq!(SYSCALL_RETURN_ERROR_REG, Reg::Rax);
    assert_eq!(SYSCALL_RETURN_VALUE_REG, Reg::Rdx);
    assert_eq!(SYSCALL_RETURN_MSGINFO_REG, Reg::R13);
    assert_eq!(SYSCALL_CLOBBERED, [Reg::Rcx, Reg::R11]);

    // Bất biến: MR của `syscall` **không** được nằm trong hai thanh ghi bị clobber.
    for mr in SYSCALL_MR_REGS {
        assert!(
            !SYSCALL_CLOBBERED.contains(&mr),
            "{mr:?} bị `syscall` clobber — không được làm MR"
        );
    }
    // Bốn MR phải phân biệt.
    for (i, a) in SYSCALL_MR_REGS.iter().enumerate() {
        for b in &SYSCALL_MR_REGS[i + 1..] {
            assert_ne!(a, b, "MR trùng thanh ghi");
        }
    }

    // Đường **đang chạy** (`int 0x80`) — **D2-242 nhóm 7 = 7C-r13**: MỘT hợp đồng thanh ghi
    // cho cả `int 0x80` lẫn `syscall`/`sysret` sau này.
    assert_eq!(INT80_VECTOR, 0x80);
    assert_eq!(
        INT80_MR_REGS, SYSCALL_MR_REGS,
        "MR phải giống nhau giữa hai đường vào ring 0"
    );
    assert_eq!(INT80_MR_REGS, [Reg::R10, Reg::R8, Reg::R9, Reg::R12]);
    assert!(
        !INT80_MR_REGS.contains(&Reg::Rcx),
        "RCX phải trống cho CPU (`syscall` ghi RIP kế tiếp vào RCX)"
    );
    assert_eq!(INT80_RETURN_MSGINFO_REG, Reg::R13);
    assert_eq!(
        INT80_RETURN_MSGINFO_REG, SYSCALL_RETURN_MSGINFO_REG,
        "MessageInfo trả về dùng CÙNG thanh ghi R13 trên cả hai đường (7C-r13)"
    );
    assert!(
        !INT80_CLOBBERED.contains(&Reg::R11),
        "R11 phải trống cho CPU (`syscall` ghi RFLAGS vào R11)"
    );
    // `[calling.x86_64].clobbered` = error, value, MessageInfo, MR0..MR3 (7 thanh ghi).
    assert_eq!(
        INT80_CLOBBERED,
        [
            Reg::Rax,
            Reg::Rdx,
            Reg::R13,
            Reg::R10,
            Reg::R8,
            Reg::R9,
            Reg::R12
        ]
    );
    assert!(INT80_CLOBBERED.contains(&SYSCALL_RETURN_ERROR_REG));
    assert!(INT80_CLOBBERED.contains(&SYSCALL_RETURN_VALUE_REG));
    assert!(INT80_CLOBBERED.contains(&INT80_RETURN_MSGINFO_REG));
    for mr in INT80_MR_REGS {
        assert!(
            INT80_CLOBBERED.contains(&mr),
            "MR {mr:?} phải được phép đổi"
        );
    }
    // `preserved` của spec (`rsp/rbx/rbp/r14/r15`) không được nằm trong clobbered.
    for reg in [Reg::Rsp, Reg::Rbx, Reg::Rbp, Reg::R14, Reg::R15] {
        assert!(!INT80_CLOBBERED.contains(&reg), "{reg:?} là preserved");
    }
}

// ------------------------------------------------- 4. map/translate 4 mức

#[test]
fn map_translate_4_levels() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map 4 KiB");
    let t = mmu.translate(VA).expect("translate");
    assert_eq!(t.pa, PA, "PA round-trip");
    assert_eq!(t.perms, Perms::KERNEL_RW, "quyền round-trip");
    assert_eq!(t.level, 3, "lá ở mức cuối (PT)");

    // Walk đi đủ bốn cấp, chỉ số tính tay [0, 0, 0, 1].
    let walk = mmu.walk(VA);
    assert_eq!(walk.len, 4, "walk phải đi đủ bốn mức");
    assert_eq!(walk.step(0).unwrap().index, 0);
    assert_eq!(walk.step(1).unwrap().index, 0);
    assert_eq!(walk.step(2).unwrap().index, 0);
    assert_eq!(walk.step(3).unwrap().index, 1);
    assert_eq!(walk.step(0).unwrap().level, 0);
    assert_eq!(walk.step(3).unwrap().level, 3);
    assert!(matches!(
        walk.outcome,
        WalkOutcome::Leaf { pa, .. } if pa == PA
    ));
    // Bốn bảng khác nhau (gốc + ba bảng trung gian).
    let tables: Vec<u64> = (0..4).map(|i| walk.step(i).unwrap().table).collect();
    assert_eq!(tables.len(), 4);
    for (i, a) in tables.iter().enumerate() {
        for b in &tables[i + 1..] {
            assert_ne!(a, b, "hai mức dùng cùng một bảng");
        }
    }

    // Nửa cao dùng root[0x1FF].
    mmu.map(VA_HIGH, PA + 0x1000, Perms::KERNEL_RX)
        .expect("map nửa cao");
    let high_walk = mmu.walk(VA_HIGH);
    assert_eq!(high_walk.step(0).unwrap().index, 0x1FF, "nửa cao root[511]");
    assert_eq!(high_walk.step(1).unwrap().index, 0x100);
    assert_eq!(high_walk.step(2).unwrap().index, 0);
    assert_eq!(high_walk.step(3).unwrap().index, 0);
    assert_eq!(mmu.translate(VA_HIGH).unwrap().pa, PA + 0x1000);

    // `Mmu` trait + alias `X86Mmu` dùng cùng walker.
    fn assert_mmu<M: Mmu>() {}
    assert_mmu::<X86Mmu<TestMemory>>();
    assert_eq!(Mmu::root(&mmu), mmu.root());
    assert_eq!(Mmu::translate(&mmu, VA).unwrap().pa, PA);
}

#[test]
fn permissions_roundtrip() {
    let mut mmu = new_mmu();
    let cases = [
        (Perms::KERNEL_RW, 0x1000u64),
        (Perms::KERNEL_RX, 0x2000),
        (Perms::KERNEL_RWX, 0x3000),
        (Perms::USER_RW, 0x4000),
        (Perms::USER_RX, 0x5000),
        (Perms::USER_RO, 0x6000),
    ];
    for (i, (perms, va)) in cases.iter().enumerate() {
        let pa = PA + (i as u64) * 0x1000;
        mmu.map(*va, pa, *perms).expect("map quyền");
        let t = mmu.translate(*va).expect("translate quyền");
        assert_eq!(t.pa, pa);
        assert_eq!(t.perms, *perms, "quyền round-trip cho {perms:?}");
    }

    // x86 không có bit R: `read = false` không biểu diễn được ⇒ từ chối.
    assert_eq!(
        mmu.map(0x7000, PA, Perms::new(false, true, false, false)),
        Err(MapError::InvalidPerms)
    );
    assert_eq!(
        mmu.map(0x7000, PA, Perms::new(false, false, true, false)),
        Err(MapError::InvalidPerms)
    );
    assert_eq!(
        mmu.map(0x7000, PA, Perms::NONE),
        Err(MapError::InvalidPerms)
    );
}

#[test]
fn mem_attr_roundtrip() {
    let mut mmu = new_mmu();
    mmu.map_with(
        VA,
        PA,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::NonShareable,
    )
    .expect("map Device");
    mmu.map_with(
        VA + 0x1000,
        PA + 0x1000,
        Perms::KERNEL_RW,
        MemAttr::Normal,
        Shareability::Inner,
    )
    .expect("map Normal");

    let dev_entry = mmu.walk(VA).step(3).unwrap().entry;
    let normal_entry = mmu.walk(VA + 0x1000).step(3).unwrap().entry;
    assert_eq!(X86_64::leaf_mem_attr(dev_entry), Some(MemAttr::Device));
    assert_eq!(X86_64::leaf_mem_attr(normal_entry), Some(MemAttr::Normal));
    assert!(!X86_64::leaf_pat(dev_entry));
    assert!(!X86_64::leaf_pat(normal_entry));

    // WT (PCD=1, PWT=0) và UC- (PCD=0, PWT=1) không biểu diễn được bằng `MemAttr`.
    let wt = PTE_PRESENT | PTE_PCD | PA;
    let uc_minus = PTE_PRESENT | PTE_PWT | PA;
    assert_eq!(X86_64::leaf_mem_attr(wt), None);
    assert_eq!(X86_64::leaf_mem_attr(uc_minus), None);
    // PAT = 1 ⇒ bảng PAT khác, bước M2 không dùng.
    assert_eq!(X86_64::leaf_mem_attr(PTE_PRESENT | PTE_PAT | PA), None);
}

// ------------------------------------------------- 5. ca âm + không rò frame

#[test]
fn negative_cases() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map nền");
    let before = mmu.memory().outstanding();

    assert_eq!(
        mmu.map(VA + 1, PA, Perms::KERNEL_RW),
        Err(MapError::Unaligned),
        "VA lệch trang"
    );
    assert_eq!(
        mmu.map(VA + 0x1000, PA + 1, Perms::KERNEL_RW),
        Err(MapError::Unaligned),
        "PA lệch trang"
    );
    assert_eq!(
        mmu.map(0x0000_0080_0000_0000, PA, Perms::KERNEL_RW),
        Err(MapError::NotCanonical),
        "VA ngoài cửa sổ 39-bit"
    );
    assert_eq!(
        mmu.map(VA + 0x1000, 0x0001_0000_0000_0000, Perms::KERNEL_RW),
        Err(MapError::PaOutOfRange),
        "PA vượt 48 bit"
    );
    assert_eq!(
        mmu.map(VA + 0x1000, PA + 0x1000, Perms::NONE),
        Err(MapError::InvalidPerms),
        "không quyền nào"
    );
    assert_eq!(
        mmu.map(VA, PA, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped),
        "map chồng"
    );
    assert_eq!(
        mmu.unmap(VA + 0x1000),
        Err(MapError::NotMapped),
        "unmap VA trống"
    );
    assert_eq!(
        mmu.unmap(VA + 1),
        Err(MapError::Unaligned),
        "unmap VA lệch trang"
    );
    assert_eq!(
        mmu.unmap(0x0000_0080_0000_0000),
        Err(MapError::NotCanonical),
        "unmap VA ngoài cửa sổ"
    );
    assert_eq!(
        mmu.translate(0x0000_0080_0000_0000),
        Err(MapError::NotCanonical),
        "translate VA ngoài cửa sổ"
    );
    assert_eq!(
        mmu.translate(VA + 0x1000),
        Err(MapError::NotMapped),
        "translate VA trống"
    );
    assert_eq!(
        mmu.memory().outstanding(),
        before,
        "từ chối không được cấp frame"
    );
    assert_eq!(mmu.translate(VA).unwrap().pa, PA, "mapping cũ giữ nguyên");
}

#[test]
fn block_and_reserved_entries_are_rejected() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map nền");
    let walk = mmu.walk(VA);
    let pd = walk.step(2).unwrap().table;
    let pt = walk.step(3).unwrap().table;

    // Chèn block 2 MiB vào PD tại index 1 (VA 0x20_0000).
    let block_va = 0x20_0000u64;
    let block_pa = 0x4000_0000u64;
    let block_entry = PTE_PRESENT | PTE_PS | block_pa;
    assert!(mmu.memory_mut().write_entry(pd, 1, block_entry));
    assert_eq!(
        mmu.translate(block_va),
        Err(MapError::BlockUnsupported),
        "translate gặp block phải từ chối"
    );
    assert_eq!(
        mmu.unmap(block_va),
        Err(MapError::BlockUnsupported),
        "unmap gặp block phải từ chối"
    );
    assert_eq!(
        mmu.map(block_va + 0x1000, PA + 0x1000, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped),
        "map vào vùng block phải AlreadyMapped"
    );

    // Entry reserved (bit 52) trong PT ⇒ CorruptTable, không ghi đè.
    // VA + 0x2000 có chỉ số PT = 3 (VA = 0x1000 ⇒ PT index 1).
    let reserved = PTE_PRESENT | (1u64 << 52) | PA;
    assert!(mmu.memory_mut().write_entry(pt, 3, reserved));
    assert_eq!(
        mmu.translate(VA + 0x2000),
        Err(MapError::CorruptTable),
        "entry reserved phải CorruptTable"
    );
    assert_eq!(
        mmu.map(VA + 0x2000, PA + 0x2000, Perms::KERNEL_RW),
        Err(MapError::CorruptTable),
        "map qua entry reserved phải CorruptTable"
    );

    // PS = 1 ở PML4 (mức 0) là reserved ⇒ CorruptTable. Dùng bảng mới để không phá mapping
    // hiện có; VA 0x1000 có PML4 index 0.
    let mut fresh = new_mmu();
    let root = fresh.root();
    let pml4e = PTE_PRESENT | PTE_PS | 0x4000_0000;
    assert!(fresh.memory_mut().write_entry(root, 0, pml4e));
    assert_eq!(
        fresh.translate(VA),
        Err(MapError::CorruptTable),
        "PS ở PML4 là reserved"
    );
}

#[test]
fn no_frame_leak() {
    let mut mmu = new_mmu();
    for i in 0..3u64 {
        mmu.map(VA + i * 0x1000, PA + i * 0x1000, Perms::KERNEL_RW)
            .expect("map 4 KiB");
    }
    mmu.map(VA + 0x4000, PA + 0x4000, Perms::USER_RX)
        .expect("map user RX");
    assert_eq!(
        mmu.memory().outstanding(),
        4,
        "gốc + ba bảng trung gian = bốn frame"
    );

    for i in [0u64, 1, 2, 4] {
        assert_eq!(
            mmu.unmap(VA + i * 0x1000),
            Ok(PA + i * 0x1000),
            "unmap trả PA"
        );
    }
    assert_eq!(mmu.memory().outstanding(), 1, "chỉ còn frame gốc");
    assert_eq!(mmu.translate(VA), Err(MapError::NotMapped));
    assert_eq!(mmu.unmap(VA), Err(MapError::NotMapped), "unmap hai lần");

    let memory = mmu.destroy().expect("destroy");
    assert_eq!(memory.outstanding(), 0, "không rò rỉ frame");
}

#[test]
fn stale_frame_is_zeroed() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map lần một");
    assert_eq!(mmu.unmap(VA), Ok(PA));
    // Frame PT vừa free chứa GARBAGE; map lại phải zero trước khi dùng.
    mmu.map(VA + 0x1000, PA + 0x1000, Perms::KERNEL_RW)
        .expect("map lại trên frame bẩn");
    let t = mmu
        .translate(VA + 0x1000)
        .expect("translate sau tái sử dụng");
    assert_eq!(t.pa, PA + 0x1000);
    assert_eq!(t.perms, Perms::KERNEL_RW);
    assert_eq!(mmu.memory().outstanding(), 4);
}

#[test]
fn out_of_frames() {
    // Không cấp được frame gốc.
    assert_eq!(
        PageTable::<X86_64, _>::new(TestMemory::with_fail_after(FRAME_BASE, 0)).err(),
        Some(MapError::OutOfFrames)
    );

    // Có gốc nhưng hết frame cho bảng trung gian: map lỗi, không đổi bộ đếm.
    let mut mmu = PageTable::<X86_64, _>::new(TestMemory::with_fail_after(FRAME_BASE, 1))
        .expect("cấp frame gốc");
    assert_eq!(
        mmu.map(VA, PA, Perms::KERNEL_RW),
        Err(MapError::OutOfFrames)
    );
    assert_eq!(mmu.memory().outstanding(), 1);
    assert_eq!(mmu.translate(VA), Err(MapError::NotMapped));
}

#[test]
fn translation_type_is_shared() {
    // `Translation` là kiểu dùng chung ở `vifast-pte` — chốt bằng cách dựng trực tiếp.
    let t = Translation {
        pa: PA,
        perms: Perms::KERNEL_RW,
        level: 3,
    };
    assert_eq!(t.pa, PA);
    assert_eq!(t.perms, Perms::KERNEL_RW);
    assert_eq!(t.level, 3);
}
