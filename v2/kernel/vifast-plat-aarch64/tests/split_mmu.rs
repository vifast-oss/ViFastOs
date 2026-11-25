// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **nửa cao (`TTBR1_EL1`) + physmap + wrapper định tuyến theo VA** — KT-253
//! (K4' bước 3, B3.4; D2-199 chốt Q4 = (c)).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test split_mmu
//! ```
//!
//! # Vì sao wrapper định tuyến mà **không** đổi trait
//!
//! D2-199: **không** thêm thao tác "nửa cao/physmap" vào `PageTableFormat`/`Mmu` (buộc cả ba
//! impl và thân test generic đổi cùng lúc — rủi ro KT-164 lặp lại). Thay vào đó crate arch dựng
//! **hai**
//! [`PageTable`] (một cho `TTBR0_EL1` vùng thấp, một cho `TTBR1_EL1` nửa cao) và một wrapper
//! [`SplitMmu`] `impl Mmu` **định tuyến theo VA** — nó **gọi** `PageTable` hai lần, **không** chép
//! walk/map.
//!
//! # Bố cục VA 39-bit canonical (D2-147)
//!
//! ```text
//!   nửa thấp   0x0000_0000_0000_0000 .. 0x0000_003F_FFFF_FFFF   (bit 38 = 0)
//!   (lỗ)       0x0000_0040_0000_0000 .. 0xFFFF_FFBF_FFFF_FFFF   KHÔNG canonical
//!   nửa cao    0xFFFF_FFC0_0000_0000 .. 0xFFFF_FFFF_FFFF_FFFF   (bit 38 = 1)
//! ```
//!
//! Đây là ca kiểm **đỏ trước** của KT-253 (log: `v2/agent/JOURNAL/kt253-red.log`).
// Khẳng định dưới đây **cố ý** so sánh hằng số (bố cục VA nửa cao/physmap) — chốt giá trị
// tuyệt đối; một hằng đổi giá trị thì test phải đỏ (cùng khuôn `tests/boot.rs`).
#![allow(clippy::assertions_on_constants)]

use core::cell::Cell;
use core::sync::atomic::{AtomicU64, Ordering};
use std::rc::Rc;

use vifast_plat_aarch64::format::{Aarch64, MemAttr, Perms};
use vifast_plat_aarch64::mmu::{MapError, Mmu, TableMemory};
use vifast_plat_aarch64::split_mmu::{
    half_of, high_alias_va, physmap_va, Half, SplitMmu, HIGH_HALF_BASE, KERNEL_HIGH_BASE,
    PHYSMAP_BASE,
};

/// PA gốc của nguồn frame test (4 KiB aligned).
const FRAME_BASE: u64 = 0x4000_0000;
/// Số frame của nguồn test (đủ cho hai cây + physmap 2 MiB).
const FRAMES: usize = 96;

// ---------------------------------------------------------------- nguồn frame host **chia sẻ**

/// Nguồn frame + bộ nhớ bảng trang cho host, **chia sẻ** giữa hai `PageTable` (nửa thấp + nửa
/// cao): cùng một mảng `words`, cùng một con trỏ bump. Đây đúng là hợp đồng của arena boot
/// (`bootmem::SharedBootMemory`), chỉ khác là cấp phát bằng `Rc<Cell<usize>>` thay vì `static`.
struct SharedTestMemory {
    words: Rc<Vec<AtomicU64>>,
    base: u64,
    frames: usize,
    next: Rc<Cell<usize>>,
}

impl SharedTestMemory {
    fn index(&self, pa: u64, index: usize) -> Option<usize> {
        if pa < self.base || pa & 0xFFF != 0 || index >= 512 {
            return None;
        }
        let frame = ((pa - self.base) / 4096) as usize;
        if frame >= self.frames {
            return None;
        }
        Some(frame * 512 + index)
    }
}

impl TableMemory for SharedTestMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        let n = self.next.get();
        if n >= self.frames {
            return None;
        }
        self.next.set(n + 1);
        Some(self.base + (n as u64) * 4096)
    }

    fn free_frame(&mut self, _pa: u64) {}

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        self.index(pa, index)
            .map(|i| self.words[i].load(Ordering::Relaxed))
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        match self.index(pa, index) {
            Some(i) => {
                self.words[i].store(entry, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        for index in 0..512 {
            if !self.write_entry(pa, index, 0) {
                return false;
            }
        }
        true
    }
}

/// Hai nguồn frame **chia sẻ** một arena — đúng cách ảnh boot cấp frame cho hai bảng.
fn pair() -> (SharedTestMemory, SharedTestMemory) {
    let words = Rc::new((0..FRAMES * 512).map(|_| AtomicU64::new(0)).collect());
    let next = Rc::new(Cell::new(0usize));
    let low = SharedTestMemory {
        words: Rc::clone(&words),
        base: FRAME_BASE,
        frames: FRAMES,
        next: Rc::clone(&next),
    };
    let high = SharedTestMemory {
        words,
        base: FRAME_BASE,
        frames: FRAMES,
        next,
    };
    (low, high)
}

/// Dựng wrapper rỗng trên hai nguồn frame chia sẻ.
fn empty() -> SplitMmu<SharedTestMemory> {
    let (low, high) = pair();
    SplitMmu::new(low, high).expect("dựng SplitMmu")
}

/// VA nửa thấp dùng trong test (identity, trong RAM QEMU `virt`).
const LOW_VA: u64 = 0x4008_0000;
/// VA nửa cao dùng trong test — cùng offset trong cửa sổ kernel nửa cao.
const HIGH_VA: u64 = KERNEL_HIGH_BASE + 0x1000;

// ---------------------------------------------------------------- (i) định tuyến theo nửa

/// **(i)** VA nửa thấp đi `TTBR0` (bảng thấp), **không** xuất hiện ở bảng nửa cao.
#[test]
fn low_va_routes_to_low_table() {
    let mut mmu = empty();
    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    assert!(
        mmu.low().translate(LOW_VA).is_ok(),
        "VA nửa thấp phải nằm trong bảng TTBR0"
    );
    assert_eq!(
        mmu.high().translate(LOW_VA),
        Err(MapError::NotMapped),
        "VA nửa thấp KHÔNG được xuất hiện ở bảng TTBR1"
    );
}

/// **(i)** VA nửa cao đi `TTBR1` (bảng nửa cao), **không** xuất hiện ở bảng thấp.
#[test]
fn high_va_routes_to_high_table() {
    let mut mmu = empty();
    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");
    assert!(
        mmu.high().translate(HIGH_VA).is_ok(),
        "VA nửa cao phải nằm trong bảng TTBR1"
    );
    assert_eq!(
        mmu.low().translate(HIGH_VA),
        Err(MapError::NotMapped),
        "VA nửa cao KHÔNG được xuất hiện ở bảng TTBR0"
    );
}

// ---------------------------------------------------------------- (ii) translate đúng PA

/// **(ii)** `translate` trả **đúng PA** cho **cả hai** nửa, kể cả khi hai nửa map cùng một PA
/// (alias hợp lệ: kernel nửa cao + identity).
#[test]
fn translate_returns_pa_for_both_halves() {
    let mut mmu = empty();
    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    mmu.map(HIGH_VA, LOW_VA, Perms::KERNEL_RW)
        .expect("map high");
    assert_eq!(mmu.translate(LOW_VA).expect("dịch low").pa, LOW_VA);
    assert_eq!(
        mmu.translate(HIGH_VA).expect("dịch high").pa,
        LOW_VA,
        "hai alias cùng PA — đúng thiết kế nửa cao"
    );
    // Quyền của lá được giữ nguyên qua wrapper.
    assert_eq!(
        mmu.translate(HIGH_VA).expect("dịch high").perms,
        Perms::KERNEL_RW
    );
}

/// **(ii)** `Mmu` trait dùng được với wrapper — tầng trên không cần biết nửa nào.
#[test]
fn split_mmu_implements_mmu_trait() {
    fn pa_via_trait<M: Mmu>(mmu: &M, va: u64) -> Result<u64, MapError> {
        mmu.translate(va).map(|t| t.pa)
    }
    let mut mmu = empty();
    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");
    assert_eq!(pa_via_trait(&mmu, LOW_VA), Ok(LOW_VA));
    assert_eq!(pa_via_trait(&mmu, HIGH_VA), Ok(0x4009_0000));
}

// ---------------------------------------------------------------- (iii) unmap độc lập

/// **(iii)** `unmap` ở một nửa **không** ảnh hưởng nửa kia (cả hai chiều).
#[test]
fn unmap_one_half_does_not_affect_other() {
    let mut mmu = empty();
    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");

    assert_eq!(mmu.unmap(LOW_VA), Ok(LOW_VA), "unmap nửa thấp trả PA");
    assert_eq!(mmu.translate(LOW_VA), Err(MapError::NotMapped));
    assert_eq!(
        mmu.translate(HIGH_VA).expect("nửa cao còn nguyên").pa,
        0x4009_0000,
        "unmap nửa thấp KHÔNG được đụng nửa cao"
    );

    assert_eq!(mmu.unmap(HIGH_VA), Ok(0x4009_0000), "unmap nửa cao trả PA");
    assert_eq!(mmu.translate(HIGH_VA), Err(MapError::NotMapped));
}

/// **(iii)** `unmap` nửa thấp giải phóng **bảng trung gian của nửa thấp**, không đụng bảng nửa
/// cao: sau khi unmap hết nửa thấp, nửa cao vẫn dịch được và ngược lại.
#[test]
fn unmap_frees_only_own_half() {
    let mut mmu = empty();
    // Hai VA nửa thấp khác nhánh + một VA nửa cao.
    mmu.map(0x4008_0000, 0x4008_0000, Perms::KERNEL_RW)
        .expect("map low 1");
    mmu.map(0x5000_0000, 0x5000_0000, Perms::KERNEL_RW)
        .expect("map low 2");
    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");

    assert!(mmu.unmap(0x4008_0000).is_ok());
    assert!(mmu.unmap(0x5000_0000).is_ok());
    assert_eq!(
        mmu.translate(HIGH_VA).expect("nửa cao sống sót").pa,
        0x4009_0000
    );
    // Map lại nửa thấp sau khi unmap — bảng trung gian được cấp lại, không hỏng.
    mmu.map(0x4008_0000, 0x4008_0000, Perms::KERNEL_RW)
        .expect("map lại low");
    assert_eq!(
        mmu.translate(0x4008_0000).expect("dịch lại").pa,
        0x4008_0000
    );
}

// ---------------------------------------------------------------- (iv) physmap

/// **(iv)** Physmap: `VA = PHYSMAP_BASE + PA` dịch **đúng** PA cho mọi trang trong cửa sổ, và
/// **không** map gì ngoài cửa sổ.
#[test]
fn physmap_maps_any_pa_in_window() {
    let mut mmu = empty();
    let start = 0x4000_0000u64;
    let end = start + 0x20_0000; // 2 MiB = 512 trang
    let pages = mmu
        .map_physmap(start, end, Perms::KERNEL_RW, MemAttr::Normal)
        .expect("map physmap");
    assert_eq!(pages, 512, "2 MiB / 4 KiB");

    for pa in [start, start + 0x1000, start + 0x1F_F000] {
        let t = mmu.translate(physmap_va(pa)).expect("physmap dịch được");
        assert_eq!(t.pa, pa, "physmap: VA = BASE + PA ⇒ PA");
        assert_eq!(t.perms, Perms::KERNEL_RW);
    }
    assert_eq!(
        mmu.translate(physmap_va(end)),
        Err(MapError::NotMapped),
        "ngoài cửa sổ physmap ⇒ chưa map"
    );
}

/// **(iv)** Physmap nằm ở **nửa cao** (bảng `TTBR1`), không phải bảng thấp.
#[test]
fn physmap_lives_in_high_table() {
    let mut mmu = empty();
    mmu.map_physmap(0x4000_0000, 0x4000_1000, Perms::KERNEL_RW, MemAttr::Normal)
        .expect("map physmap");
    let va = physmap_va(0x4000_0000);
    assert!(mmu.high().translate(va).is_ok(), "physmap ở bảng nửa cao");
    assert_eq!(mmu.low().translate(va), Err(MapError::NotMapped));
}

/// **(iv)** `unmap` qua physmap trả đúng PA và bỏ mapping; `physmap_va` là phép cộng thuần.
#[test]
fn physmap_unmap_and_va_math() {
    assert_eq!(physmap_va(0), PHYSMAP_BASE);
    assert_eq!(physmap_va(0x4000_0000), PHYSMAP_BASE + 0x4000_0000);

    let mut mmu = empty();
    mmu.map_physmap(0x4000_0000, 0x4000_2000, Perms::KERNEL_RW, MemAttr::Normal)
        .expect("map physmap");
    assert_eq!(mmu.unmap(physmap_va(0x4000_0000)), Ok(0x4000_0000));
    assert_eq!(
        mmu.translate(physmap_va(0x4000_0000)),
        Err(MapError::NotMapped)
    );
    assert_eq!(
        mmu.translate(physmap_va(0x4000_1000)).expect("trang kế").pa,
        0x4000_1000
    );
}

// ---------------------------------------------------------------- (v) VA không canonical

/// **(v)** VA **không canonical** ⇒ lỗi rõ `NotCanonical` ở **mọi** thao tác, không panic, không
/// đoán nửa.
#[test]
fn non_canonical_va_is_rejected() {
    let mut mmu = empty();
    // Nằm giữa nửa thấp và nửa cao: bit 38 = 0 nhưng bit 63..39 khác 0 ⇒ không canonical.
    let bad = 0x0000_0080_0000_0000u64;
    assert!(!Aarch64::is_canonical(bad), "VA này phải không canonical");
    assert_eq!(half_of(bad), None, "không đoán nửa cho VA không canonical");
    assert_eq!(
        mmu.map(bad, 0x4000_0000, Perms::KERNEL_RW),
        Err(MapError::NotCanonical)
    );
    assert_eq!(mmu.translate(bad), Err(MapError::NotCanonical));
    assert_eq!(mmu.unmap(bad), Err(MapError::NotCanonical));
    // VA lệch 4 KiB vẫn là `Unaligned` (kiểm căn lề trước canonical, như `PageTable`).
    assert_eq!(
        mmu.map(LOW_VA + 1, LOW_VA, Perms::KERNEL_RW),
        Err(MapError::Unaligned)
    );
}

// ---------------------------------------------------------------- (vi) map trùng

/// **(vi)** Map trùng ⇒ `AlreadyMapped` **như `PageTable`** (cả hai nửa), không ghi đè.
#[test]
fn duplicate_map_is_already_mapped() {
    let mut mmu = empty();
    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    assert_eq!(
        mmu.map(LOW_VA, 0x4009_0000, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped)
    );
    // PA cũ vẫn còn (không bị ghi đè bởi lần map hỏng).
    assert_eq!(mmu.translate(LOW_VA).expect("dịch").pa, LOW_VA);

    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");
    assert_eq!(
        mmu.map(HIGH_VA, 0x400A_0000, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped)
    );
    assert_eq!(mmu.translate(HIGH_VA).expect("dịch").pa, 0x4009_0000);
}

// ---------------------------------------------------------------- gốc + walk định tuyến

/// Hai gốc **khác nhau**; `root()` (hợp đồng `Mmu`) là gốc nửa thấp; `ttbr0_base`/`ttbr1_base`
/// trỏ **bảng mức 1** (không phải gốc mức 0) — đúng mức phần cứng bắt đầu với `T0SZ = T1SZ = 25`.
#[test]
fn roots_are_separate_and_ttbr_bases_are_level1() {
    let mut mmu = empty();
    assert_eq!(mmu.root(), mmu.low().root(), "root() = gốc nửa thấp");
    assert_ne!(mmu.root_high(), mmu.root(), "hai gốc phải khác nhau");
    assert_eq!(mmu.ttbr0_base(), None, "bảng rỗng chưa có mức 1");
    assert_eq!(mmu.ttbr1_base(), None, "bảng rỗng chưa có mức 1");

    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");
    let ttbr0 = mmu.ttbr0_base().expect("TTBR0 có bảng mức 1");
    let ttbr1 = mmu.ttbr1_base().expect("TTBR1 có bảng mức 1");
    assert_ne!(ttbr0, mmu.root(), "TTBR0 = bảng mức 1, KHÔNG phải gốc");
    assert_ne!(ttbr1, mmu.root_high(), "TTBR1 = bảng mức 1, KHÔNG phải gốc");
    assert_ne!(ttbr0, ttbr1, "hai nửa dùng hai bảng mức 1 riêng");
    assert_eq!(ttbr0 & 0xFFF, 0, "BADDR căn 4 KiB");
    assert_eq!(ttbr1 & 0xFFF, 0, "BADDR căn 4 KiB");
}

/// `walk` chẩn đoán cũng định tuyến: bước 0 của VA nửa thấp đọc **gốc nửa thấp**, của VA nửa
/// cao đọc **gốc nửa cao**.
#[test]
fn walk_routes_by_half() {
    let mut mmu = empty();
    mmu.map(LOW_VA, LOW_VA, Perms::KERNEL_RW).expect("map low");
    mmu.map(HIGH_VA, 0x4009_0000, Perms::KERNEL_RW)
        .expect("map high");

    let low = mmu.walk(LOW_VA);
    assert_eq!(low.step(0).expect("bước 0").table, mmu.root());
    let high = mmu.walk(HIGH_VA);
    assert_eq!(high.step(0).expect("bước 0").table, mmu.root_high());
    // VA không canonical ⇒ walk rỗng (không đoán nửa).
    assert_eq!(mmu.walk(0x0000_0080_0000_0000).len, 0);
}

// ---------------------------------------------------------------- alias nửa cao của ảnh

/// Alias nửa cao của **ảnh kernel**: `VA = KERNEL_HIGH_BASE + (PA - text_start)` — mỗi trang
/// dịch đúng PA, và alias nằm ở bảng nửa cao (không phải bảng thấp).
#[test]
fn high_alias_maps_image_at_high_base() {
    let mut mmu = empty();
    let text_start = 0x4008_0000u64;
    let end = text_start + 0x3000; // 3 trang
    let pages = mmu
        .map_high_alias(
            text_start,
            end,
            KERNEL_HIGH_BASE,
            Perms::KERNEL_RWX,
            MemAttr::Normal,
        )
        .expect("map alias nửa cao");
    assert_eq!(pages, 3);
    for page in 0..3u64 {
        let pa = text_start + page * 0x1000;
        let va = high_alias_va(pa, text_start, KERNEL_HIGH_BASE);
        assert_eq!(va, KERNEL_HIGH_BASE + page * 0x1000, "alias bắt đầu ở base");
        let t = mmu.translate(va).expect("alias dịch được");
        assert_eq!(t.pa, pa, "alias nửa cao ⇒ đúng PA");
        assert_eq!(t.perms, Perms::KERNEL_RWX);
        assert!(mmu.high().translate(va).is_ok(), "alias ở bảng nửa cao");
        assert_eq!(mmu.low().translate(va), Err(MapError::NotMapped));
    }
    // `start_pa` lệch trang ⇒ `Unaligned` (không map lệch âm thầm).
    assert_eq!(
        mmu.map_high_alias(
            text_start + 1,
            end,
            KERNEL_HIGH_BASE,
            Perms::KERNEL_RW,
            MemAttr::Normal
        ),
        Err(MapError::Unaligned)
    );
}

/// `high_alias_va` là phép cộng thuần: `pa == start_pa` ⇒ đúng `high_base`.
#[test]
fn high_alias_va_math() {
    let text_start = 0x4008_0000u64;
    assert_eq!(
        high_alias_va(text_start, text_start, KERNEL_HIGH_BASE),
        KERNEL_HIGH_BASE
    );
    assert_eq!(
        high_alias_va(text_start + 0x1234, text_start, KERNEL_HIGH_BASE),
        KERNEL_HIGH_BASE + 0x1234
    );
}

// ---------------------------------------------------------------- bố cục VA

/// Hằng bố cục VA: nửa cao + physmap + kernel nửa cao đều **canonical 39-bit**, và ranh giới
/// `half_of` đúng (không có VA canonical nào rơi vào lỗ giữa hai nửa).
#[test]
fn va_layout_constants_are_canonical() {
    for va in [HIGH_HALF_BASE, PHYSMAP_BASE, KERNEL_HIGH_BASE] {
        assert!(Aarch64::is_canonical(va), "0x{va:x} phải canonical 39-bit");
        assert!(va > (1u64 << 63), "nửa cao phải trên 2^63");
    }
    assert_eq!(half_of(HIGH_HALF_BASE), Some(Half::High));
    assert_eq!(half_of(0), Some(Half::Low));
    assert_eq!(
        half_of(0x0000_003F_FFFF_F000),
        Some(Half::Low),
        "trang cuối nửa thấp"
    );
    assert_eq!(
        half_of(HIGH_HALF_BASE - 0x1000),
        None,
        "ngay dưới nửa cao là lỗ — không canonical"
    );
    // Physmap và kernel nửa cao **không** đè nhau (hai cửa sổ rời).
    assert!(
        KERNEL_HIGH_BASE >= PHYSMAP_BASE + 0x8000_0000,
        "cách ≥ 2 GiB"
    );
}
