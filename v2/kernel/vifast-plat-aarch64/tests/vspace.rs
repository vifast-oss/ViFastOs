// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `VSpace` aarch64 — KT-254 (K4' bước 3, B3.5).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vspace
//! ```
//!
//! # Phạm vi
//!
//! Phần **dữ liệu** của `VSpace`: `create` / `map_obj` / `unmap` / `walk` / `ttbr0` / `ttbr1` /
//! mã hoá `Ttbr`. Phần **fault thật** (đọc qua VA sau `unmap` ⇒ `FaultKind::Vm`) chỉ đo được
//! trên QEMU — xem `v2/agent/JOURNAL/kt254-vspace.md` §3.
//!
//! # Nợ M17 (ghi rõ, **không** sửa trong task này)
//!
//! Object layer `v2/kernel/vifast-plat-common/src/obj/vspace.rs` gắn **Sv39** (import
//! `walk_shape`, `PTE_LEVELS`, bit PTE của riscv). `VSpace` ở đây dựng **trong crate arch** trên
//! [`SplitMmu`] (KT-253) — **không** nhân bản logic `PageTable`, **không** sửa object layer.
//!
//! Đây là ca kiểm **đỏ trước** của KT-254 (log: `v2/agent/JOURNAL/kt254-red.log`).

use core::cell::Cell;
use core::sync::atomic::{AtomicU64, Ordering};
use std::rc::Rc;

use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::mmu::{MapError, TableMemory};
use vifast_plat_aarch64::split_mmu::{KERNEL_HIGH_BASE, PHYSMAP_BASE};
use vifast_plat_aarch64::sysreg::Ttbr;
use vifast_plat_aarch64::vspace::{Frame, VSpace, VSpaceId};

/// PA gốc của nguồn frame test (4 KiB aligned).
const FRAME_BASE: u64 = 0x4000_0000;
/// Số frame của nguồn test.
const FRAMES: usize = 128;
/// VA **người dùng** của test — ngoài RAM 128 MiB của QEMU `virt`, không identity.
const USER_VA: u64 = 0x6000_0000;
/// PA của "Frame 4 KiB" được map vào VSpace.
const USER_FRAME_PA: u64 = 0x4008_0000;
/// ASID của VSpace test.
const ASID: VSpaceId = 7;

// ---------------------------------------------------------------- nguồn frame host chia sẻ

/// Nguồn frame host **chia sẻ** giữa hai bảng của một `VSpace` (cùng arena, cùng bộ đếm).
struct SharedTestMemory {
    words: Rc<Vec<AtomicU64>>,
    base: u64,
    frames: usize,
    next: Rc<Cell<usize>>,
    /// **KT-415** — số frame đã `free_frame` (để kiểm `alloc == free` sau `destroy`).
    freed: Rc<Cell<usize>>,
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

    fn free_frame(&mut self, _pa: u64) {
        // **KT-415**: đếm thật để `destroy` kiểm được "không rò rỉ frame".
        self.freed.set(self.freed.get() + 1);
    }

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

/// Hai nguồn frame chia sẻ một arena — đúng cách ảnh boot cấp frame cho một `VSpace`.
fn pair() -> (SharedTestMemory, SharedTestMemory) {
    let words = Rc::new((0..FRAMES * 512).map(|_| AtomicU64::new(0)).collect());
    let next = Rc::new(Cell::new(0usize));
    let freed = Rc::new(Cell::new(0usize));
    let low = SharedTestMemory {
        words: Rc::clone(&words),
        base: FRAME_BASE,
        frames: FRAMES,
        next: Rc::clone(&next),
        freed: Rc::clone(&freed),
    };
    let high = SharedTestMemory {
        words,
        base: FRAME_BASE,
        frames: FRAMES,
        next,
        freed,
    };
    (low, high)
}

/// `VSpace` rỗng (chưa map gì).
fn empty() -> VSpace<SharedTestMemory> {
    let (low, high) = pair();
    VSpace::create(ASID, low, high).expect("tạo VSpace")
}

// ---------------------------------------------------------------- create

/// `create` cấp **hai** gốc khác nhau (nửa thấp + nửa cao), chưa có bảng mức 1 ⇒ `ttbr0`/`ttbr1`
/// là `None` (không bịa địa chỉ).
#[test]
fn create_has_two_roots_and_no_ttbr() {
    let vs = empty();
    assert_eq!(vs.id(), ASID);
    assert_ne!(vs.root_low(), 0);
    assert_ne!(vs.root_high(), 0);
    assert_ne!(vs.root_low(), vs.root_high(), "hai gốc riêng");
    assert_eq!(vs.ttbr0(), None, "bảng rỗng chưa có mức 1");
    assert_eq!(vs.ttbr1(), None, "bảng rỗng chưa có mức 1");
    assert_eq!(vs.ttbr0_value(), None);
    assert_eq!(vs.ttbr1_value(), None);
}

// ---------------------------------------------------------------- map_obj + walk

/// Map **một Frame 4 KiB** vào VSpace ⇒ `walk` trả **đúng PA**; quyền của lá giữ nguyên.
#[test]
fn map_obj_walk_returns_pa() {
    let mut vs = empty();
    let frame = Frame::new(USER_FRAME_PA).expect("Frame căn 4 KiB");
    vs.map_obj(USER_VA, frame, Perms::USER_RW).expect("map_obj");
    let t = vs.walk(USER_VA).expect("walk thấy mapping");
    assert_eq!(t.pa, USER_FRAME_PA, "walk trả đúng PA của Frame");
    assert_eq!(t.perms, Perms::USER_RW);
    // VA khác chưa map ⇒ lỗi, không trả PA rác.
    assert_eq!(vs.walk(USER_VA + 0x1000), Err(MapError::NotMapped));
}

/// `walk_steps` là chẩn đoán: 4 bước (4 mức) và bước 0 đọc **gốc nửa thấp**.
#[test]
fn walk_steps_uses_low_root() {
    let mut vs = empty();
    vs.map_obj(USER_VA, Frame::new(USER_FRAME_PA).unwrap(), Perms::USER_RW)
        .expect("map_obj");
    let walk = vs.walk_steps(USER_VA);
    assert_eq!(walk.len, 4, "bảng 4 mức ⇒ 4 bước");
    assert_eq!(walk.step(0).expect("bước 0").table, vs.root_low());
}

// ---------------------------------------------------------------- unmap

/// `unmap` rồi `walk` trả **lỗi** (`NotMapped`), và trả đúng PA đã map.
#[test]
fn unmap_then_walk_not_mapped() {
    let mut vs = empty();
    vs.map_obj(USER_VA, Frame::new(USER_FRAME_PA).unwrap(), Perms::USER_RW)
        .expect("map_obj");
    assert_eq!(vs.unmap(USER_VA), Ok(USER_FRAME_PA), "unmap trả PA");
    assert_eq!(vs.walk(USER_VA), Err(MapError::NotMapped));
    // Unmap lần hai ⇒ `NotMapped` (không panic).
    assert_eq!(vs.unmap(USER_VA), Err(MapError::NotMapped));
}

/// `unmap` một VA **không** ảnh hưởng VA khác trong cùng VSpace.
#[test]
fn unmap_does_not_affect_other_va() {
    let mut vs = empty();
    vs.map_obj(USER_VA, Frame::new(USER_FRAME_PA).unwrap(), Perms::USER_RW)
        .expect("map 1");
    vs.map_obj(
        USER_VA + 0x1000,
        Frame::new(USER_FRAME_PA + 0x1000).unwrap(),
        Perms::USER_RW,
    )
    .expect("map 2");
    assert_eq!(vs.unmap(USER_VA), Ok(USER_FRAME_PA));
    assert_eq!(
        vs.walk(USER_VA + 0x1000).expect("VA kia còn").pa,
        USER_FRAME_PA + 0x1000
    );
}

// ---------------------------------------------------------------- ca âm

/// `Frame` phải căn 4 KiB; VA phải canonical; map trùng ⇒ `AlreadyMapped` — **cùng lỗi** như
/// `PageTable` (không có bản sao logic kiểm).
#[test]
fn map_obj_rejects_bad_inputs() {
    assert_eq!(Frame::new(USER_FRAME_PA + 1), None, "Frame lệch 4 KiB");
    assert_eq!(
        Frame::new(USER_FRAME_PA),
        Some(Frame::new(USER_FRAME_PA).unwrap())
    );

    let mut vs = empty();
    // VA không canonical (giữa hai nửa).
    assert_eq!(
        vs.map_obj(
            0x0000_0080_0000_0000,
            Frame::new(USER_FRAME_PA).unwrap(),
            Perms::USER_RW
        ),
        Err(MapError::NotCanonical)
    );
    // Map trùng.
    vs.map_obj(USER_VA, Frame::new(USER_FRAME_PA).unwrap(), Perms::USER_RW)
        .expect("map");
    assert_eq!(
        vs.map_obj(
            USER_VA,
            Frame::new(USER_FRAME_PA + 0x1000).unwrap(),
            Perms::USER_RW
        ),
        Err(MapError::AlreadyMapped)
    );
    // PA vượt 48 bit.
    assert_eq!(
        vs.map_obj(
            USER_VA + 0x2000,
            Frame::new(1u64 << 48).unwrap(),
            Perms::USER_RW
        ),
        Err(MapError::PaOutOfRange)
    );
}

// ---------------------------------------------------------------- TTBR + Ttbr

/// `ttbr0`/`ttbr1` trỏ **bảng mức 1** (không phải gốc), và `Ttbr` mã hoá đúng `BADDR` + `ASID`.
#[test]
fn ttbr_values_are_level1_and_ttbr_encoded() {
    let mut vs = empty();
    vs.map_obj(USER_VA, Frame::new(USER_FRAME_PA).unwrap(), Perms::USER_RW)
        .expect("map user");
    vs.map_kernel_high(
        FRAME_BASE,
        FRAME_BASE + 0x2000,
        KERNEL_HIGH_BASE,
        Perms::KERNEL_RWX,
        MemAttr::Normal,
    )
    .expect("map kernel high");

    let ttbr0 = vs.ttbr0().expect("TTBR0 có bảng mức 1");
    let ttbr1 = vs.ttbr1().expect("TTBR1 có bảng mức 1");
    assert_ne!(ttbr0, vs.root_low(), "TTBR0 = bảng mức 1, KHÔNG phải gốc");
    assert_ne!(ttbr1, vs.root_high(), "TTBR1 = bảng mức 1, KHÔNG phải gốc");
    assert_ne!(ttbr0, ttbr1, "hai nửa hai bảng mức 1 riêng");
    assert_eq!(ttbr0 & 0xFFF, 0, "BADDR căn 4 KiB");
    assert_eq!(ttbr1 & 0xFFF, 0, "BADDR căn 4 KiB");

    // Mã hoá `Ttbr`: BADDR ở bit [47:12], ASID ở [63:48].
    let v0 = vs.ttbr0_value().expect("giá trị TTBR0");
    let v1 = vs.ttbr1_value().expect("giá trị TTBR1");
    assert_eq!(
        v0,
        Ttbr::new(ttbr0, ASID).bits(),
        "TTBR0 mang ASID của VSpace"
    );
    assert_eq!(v1, Ttbr::new(ttbr1, 0).bits(), "TTBR1 mang ASID 0 (kernel)");
    assert_eq!(Ttbr::new(ttbr0, ASID).baddr(), ttbr0);
    assert_eq!(Ttbr::new(ttbr0, ASID).asid(), ASID);
}

// ---------------------------------------------------------------- kernel high + physmap

/// `map_kernel_high` map alias nửa cao của ảnh; `walk` nửa cao trả đúng PA (dùng lại
/// `SplitMmu::map_high_alias` — **không** nhân bản logic).
#[test]
fn map_kernel_high_alias_walks() {
    let mut vs = empty();
    let pages = vs
        .map_kernel_high(
            FRAME_BASE,
            FRAME_BASE + 0x3000,
            KERNEL_HIGH_BASE,
            Perms::KERNEL_RWX,
            MemAttr::Normal,
        )
        .expect("map kernel high");
    assert_eq!(pages, 3);
    for page in 0..3u64 {
        let pa = FRAME_BASE + page * 0x1000;
        let va = KERNEL_HIGH_BASE + page * 0x1000;
        assert_eq!(vs.walk(va).expect("alias nửa cao").pa, pa);
    }
}

/// `map_physmap` map cửa sổ `PHYSMAP_BASE + PA`; `walk` trả đúng PA.
#[test]
fn map_physmap_window_walks() {
    let mut vs = empty();
    let pages = vs
        .map_physmap(
            FRAME_BASE,
            FRAME_BASE + 0x2000,
            Perms::KERNEL_RW,
            MemAttr::Normal,
        )
        .expect("map physmap");
    assert_eq!(pages, 2);
    assert_eq!(
        vs.walk(PHYSMAP_BASE + FRAME_BASE).expect("physmap").pa,
        FRAME_BASE
    );
    assert_eq!(
        vs.walk(PHYSMAP_BASE + FRAME_BASE + 0x1000)
            .expect("physmap")
            .pa,
        FRAME_BASE + 0x1000
    );
}

// ---------------------------------------------------------------- nhiều VSpace độc lập

/// Hai `VSpace` cùng VA nhưng khác PA ⇒ `walk` mỗi cái trả PA của mình (không lẫn bảng).
#[test]
fn two_vspaces_are_independent() {
    let mut a = empty();
    let mut b = empty();
    a.map_obj(USER_VA, Frame::new(USER_FRAME_PA).unwrap(), Perms::USER_RW)
        .expect("map a");
    b.map_obj(
        USER_VA,
        Frame::new(USER_FRAME_PA + 0x5000).unwrap(),
        Perms::USER_RW,
    )
    .expect("map b");
    assert_eq!(a.walk(USER_VA).expect("a").pa, USER_FRAME_PA);
    assert_eq!(b.walk(USER_VA).expect("b").pa, USER_FRAME_PA + 0x5000);
    // Unmap ở a không đụng b.
    assert_eq!(a.unmap(USER_VA), Ok(USER_FRAME_PA));
    assert_eq!(b.walk(USER_VA).expect("b còn").pa, USER_FRAME_PA + 0x5000);
}

// ---------------------------------------------------------------- KT-415: destroy + range

/// **KT-415** — `destroy` giải phóng **mọi** frame bảng trang của **cả hai** nửa: `alloc == free`.
///
/// Đây là bất biến "không rò rỉ frame" của walker dùng chung (`vifast-pte`), kiểm ở tầng `VSpace`
/// (nơi có **hai** cây): nếu `SplitMmu::destroy` bỏ một nửa thì `free < alloc` và test đỏ.
#[test]
fn destroy_giai_phong_moi_frame_bang_alloc() {
    let (low, high) = pair();
    let freed = Rc::clone(&low.freed);
    let alloc_counter = Rc::clone(&low.next);
    let mut vs = VSpace::create(ASID, low, high).expect("tạo VSpace");
    // Map vài trang ở **cả hai** nửa để cây có bảng trung gian thật.
    for i in 0..4u64 {
        vs.map_obj(
            USER_VA + i * 0x1000,
            Frame::new(USER_FRAME_PA + i * 0x1000).unwrap(),
            Perms::USER_RW,
        )
        .expect("map user");
    }
    vs.map_kernel_high(
        0x8000_0000,
        0x8000_4000,
        KERNEL_HIGH_BASE,
        Perms::KERNEL_RW,
        MemAttr::Normal,
    )
    .expect("map high");
    let alloc = alloc_counter.get();
    assert!(alloc >= 4, "phải có frame bảng trang thật (alloc={alloc})");
    vs.destroy().expect("destroy");
    assert_eq!(
        freed.get(),
        alloc,
        "destroy phải trả **đúng** số frame đã cấp (alloc={alloc}, free={})",
        freed.get()
    );
}

/// **KT-415** — `range_pages` là đơn vị của shootdown theo range: `0` ⇒ 1 trang, làm tròn **lên**.
#[test]
fn range_pages_lam_tron_len_va_mot_trang_khi_len_bang_khong() {
    use vifast_plat_aarch64::shootdown::{range_pages, PAGE_BYTES};
    assert_eq!(PAGE_BYTES, 4096);
    assert_eq!(range_pages(0), 1, "len=0 ⇒ đúng một trang (đường cũ)");
    assert_eq!(range_pages(1), 1);
    assert_eq!(range_pages(4096), 1);
    assert_eq!(
        range_pages(4097),
        2,
        "range không căn trang vẫn phủ trang cuối"
    );
    assert_eq!(range_pages(4 * 4096), 4);
}

/// **KT-415** — `Shootdown` mang **cả** `len` của range: một yêu cầu cho N trang, không phải N
/// yêu cầu (đó là điều `free_range` dựa vào).
#[test]
fn shootdown_mang_len_cua_range() {
    use vifast_plat_aarch64::shootdown::Shootdown;
    let sd = Shootdown {
        va: 0x4000,
        len: 4 * 4096,
        targets: 0b10,
        expected: 1,
        acks: 1,
        waited: 3,
        timed_out: false,
        send_failed: 0,
    };
    assert_eq!(sd.len, 16_384);
    assert!(sd.ok());
}
