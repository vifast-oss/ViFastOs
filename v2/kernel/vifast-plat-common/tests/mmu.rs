// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-common::mmu` + guard stats (KT-46).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! Kiểm phần logic thuần: dựng bảng Sv39 từ arena RAM, map 4 KiB/2 MiB/1 GiB, walk phần mềm,
//! cờ PTE (physmap NX, guard NONE), từ chối mapping sai, `satp` mode 8, thống kê guard fault và
//! dump `PANIC: guard page fault`. Phần asm/`csrw satp`/QEMU nằm ở arch + TAP (D2-72 Q12).

use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};

use vifast_plat_common::mmu::{
    align_up, is_canonical, satp_mode, satp_root, satp_sv39, walk_with, MapError, PageTableBuilder,
    PteFlags, PteMemory, Walk, PAGE_BYTES, PTE_ACCESSED, PTE_DIRTY, PTE_EXEC, PTE_READ, PTE_VALID,
    PTE_WRITE, SATP_MODE_SV39,
};
use vifast_plat_common::trap::{
    report_guard_fault, Exception, ExceptionKind, GuardStats, TrapFrame,
};
use vifast_plat_common::{Platform, PlatformWriter};

const HIGH: u64 = 0xFFFF_FFC0_4000_0000;
const PHYS: u64 = 0xFFFF_FFC0_0000_0000;

fn builder_with(bytes: usize) -> PageTableBuilder<'static> {
    // `Box::leak` để builder giữ `&mut [u8]` với lifetime 'static cho test ngắn — dọn cùng process.
    let arena: &'static mut [u8] = Box::leak(vec![0u8; bytes].into_boxed_slice());
    PageTableBuilder::new(arena, 0x8021_9000).expect("arena hợp lệ")
}

#[test]
fn builder_map_walk_bon_cap_4k_2m_1g() {
    let mut b = builder_with(64 * 1024);
    let root = b.root();
    assert_eq!(root, 0x8021_9000);
    b.map(PHYS, 0, 2, PteFlags::kernel_rw_nx()).unwrap();
    b.map(HIGH, 0x8000_0000, 1, PteFlags::kernel_rw_nx())
        .unwrap();
    b.map(HIGH + 0x20_0000, 0x8020_1000, 0, PteFlags::kernel_rwx())
        .unwrap();
    let one_gib = b.walk(PHYS + 0x1234_5678);
    assert!(one_gib.valid && one_gib.leaf);
    assert_eq!(one_gib.level, 2);
    assert_eq!(one_gib.phys, 0);
    let two_mib = b.walk(HIGH + 0x1F_FFFF);
    assert!(two_mib.valid && two_mib.leaf);
    assert_eq!(two_mib.level, 1);
    assert_eq!(two_mib.phys, 0x8000_0000);
    let four_kib = b.walk(HIGH + 0x20_0000);
    assert!(four_kib.valid && four_kib.leaf);
    assert_eq!(four_kib.level, 0);
    assert_eq!(four_kib.phys, 0x8020_1000);
    // 4 KiB leaf mang cờ RWX + A|D; 2 MiB leaf NX.
    assert_ne!(four_kib.raw() & PTE_EXEC, 0);
    assert_eq!(two_mib.raw() & PTE_EXEC, 0);
    assert_ne!(two_mib.raw() & (PTE_VALID | PTE_READ | PTE_WRITE), 0);
    assert_ne!(two_mib.raw() & (PTE_ACCESSED | PTE_DIRTY), 0);
    // 1G leaf nằm ngay trong bảng gốc (không cần bảng trung gian); 2M + 4K cần L1/L0.
    assert_eq!(b.tables(), 3, "root + L1 + L0");
}

#[test]
fn builder_tu_choi_mapping_sai() {
    let mut b = builder_with(16 * 1024);
    assert_eq!(
        b.map(0x1000, 0x1000, 3, PteFlags::kernel_rw_nx()),
        Err(MapError::LevelOutOfRange)
    );
    assert_eq!(
        b.map(0x1800, 0x1000, 0, PteFlags::kernel_rw_nx()),
        Err(MapError::Unaligned)
    );
    assert_eq!(
        b.map(0x8020_0000, 0x8010_0000, 1, PteFlags::kernel_rw_nx()),
        Err(MapError::Unaligned)
    );
    assert_eq!(
        b.map(1 << 39, (1 << 39) + 0x1000, 0, PteFlags::kernel_rw_nx()),
        Err(MapError::NotCanonical)
    );
    assert_eq!(
        b.map(
            HIGH,
            0x8020_0000,
            0,
            PteFlags::from_bits(PTE_VALID | PTE_WRITE)
        ),
        Err(MapError::InvalidFlags)
    );
    b.map(HIGH, 0x8020_0000, 0, PteFlags::kernel_rw_nx())
        .unwrap();
    assert_eq!(
        b.map(HIGH, 0x8020_1000, 0, PteFlags::kernel_rw_nx()),
        Err(MapError::AlreadyMapped)
    );
    // Arena 16 KiB: root + 2 bảng trung gian = 12 KiB; mapping nhánh khác cần thêm bảng ⇒ hết
    // arena (không panic, trả lỗi).
    let mut tiny = builder_with(PAGE_BYTES as usize);
    assert_eq!(
        tiny.map(HIGH, 0x8020_0000, 0, PteFlags::kernel_rw_nx()),
        Err(MapError::OutOfArena)
    );
    // Arena chỉ có bảng gốc: map 2 MiB cần một bảng L1 ⇒ hết chỗ.
    assert_eq!(
        tiny.map(HIGH, 0x8020_0000, 1, PteFlags::kernel_rw_nx()),
        Err(MapError::OutOfArena)
    );
}

#[test]
fn walk_with_va_khong_canonical_khong_doc_bo_nho() {
    struct KhongDuocGoi;
    impl PteMemory for KhongDuocGoi {
        fn load(&self, _table_phys: u64, _index: usize) -> Option<u64> {
            panic!("không được đọc bộ nhớ với VA không canonical");
        }
    }
    let walk = walk_with(&KhongDuocGoi, 0x8021_9000, 1 << 39);
    assert_eq!(walk, Walk::invalid(2));
    // Nửa cao canonical và u64::MAX vẫn canonical (sign-extension).
    assert!(is_canonical(u64::MAX));
    assert!(is_canonical(HIGH));
    assert!(is_canonical(0x8020_0000));
    assert!(is_canonical((1 << 38) - 1));
    assert!(!is_canonical(1 << 39));
    assert!(!is_canonical(0x0000_8000_0000_0000));
}

#[test]
fn guard_none_o_ca_ba_alias_va_physmap_nx() {
    let mut b = builder_with(64 * 1024);
    let guard_start = 0x8020_4000u64;
    let guard_end = 0x8020_5000u64;
    let kernel_end = 0x8021_9000u64;
    let arena_end = 0x8022_9000u64;
    let mut va = 0x8020_0000u64;
    while va < arena_end {
        if !(va >= guard_start && va < guard_end) {
            let flags = if va >= 0x8020_5000 {
                PteFlags::kernel_rw_nx()
            } else {
                PteFlags::kernel_rwx()
            };
            b.map(va, va, 0, flags).unwrap();
            b.map(HIGH + (va - 0x8020_0000), va, 0, flags).unwrap();
        }
        va += PAGE_BYTES;
    }
    // Physmap: chunk chứa guard tách 4 KiB, phần còn lại 2 MiB NX.
    let mut pa = 0x8000_0000u64;
    while pa < 0x8800_0000 {
        if pa == 0x8020_0000 {
            let mut page = pa;
            while page < pa + (1 << 21) {
                if !(page >= guard_start && page < guard_end) {
                    b.map(PHYS + page, page, 0, PteFlags::kernel_rw_nx())
                        .unwrap();
                }
                page += PAGE_BYTES;
            }
        } else {
            b.map(PHYS + pa, pa, 1, PteFlags::kernel_rw_nx()).unwrap();
        }
        pa += 1 << 21;
    }
    assert!(!b.walk(guard_start).valid);
    assert!(!b.walk(HIGH + (guard_start - 0x8020_0000)).valid);
    assert!(!b.walk(PHYS + guard_start).valid);
    let stack = b.walk(0x8020_5000);
    assert!(stack.valid && stack.leaf && stack.level == 0);
    assert_eq!(stack.raw() & PTE_EXEC, 0);
    let high_stack = b.walk(HIGH + 0x5000);
    assert_eq!(high_stack.raw(), stack.raw());
    let ram = b.walk(PHYS + 0x8000_0000);
    assert!(ram.valid && ram.leaf && ram.level == 1);
    assert_eq!(ram.raw() & PTE_EXEC, 0);
    // Guard không lộ qua 2 MiB leaf: chunk đó là các leaf 4 KiB.
    assert!(!b.walk(PHYS + 0x8020_4000).valid);
    assert!(b.leaves() > 500, "leaves = {}", b.leaves());
    assert!(align_up(kernel_end, PAGE_BYTES) == 0x8021_9000);
    assert!(arena_end > kernel_end);
}

#[test]
fn satp_sv39_mode_8_va_ppn_root() {
    let satp = satp_sv39(0x8021_9000);
    assert_eq!(satp_mode(satp), SATP_MODE_SV39);
    assert_eq!(satp_mode(satp), 8);
    assert_eq!(satp_root(satp), 0x8021_9000);
    assert_eq!(satp >> 60, 8);
    assert_eq!(satp_sv39(PAGE_BYTES), (8u64 << 60) | 1);
    // Giá trị khác mode 8 phải bị nhận ra (TAP `mmu on` kiểm `satp >> 60`).
    assert_ne!(satp_mode(0), 8);
}

#[test]
fn guard_stats_dem_load_store_va_fatal() {
    let stats = GuardStats::new();
    let mut tf = TrapFrame::zeroed();
    tf.scause = 13;
    tf.stval = 0x8020_4000;
    tf.sepc = 0x8020_0200;
    stats.record(&tf, true);
    let snap = stats.snapshot();
    assert_eq!(snap.faults, 1);
    assert_eq!(snap.load_faults, 1);
    assert_eq!(snap.store_faults, 0);
    assert_eq!(snap.recovered, 1);
    assert_eq!(snap.fatal, 0);
    assert_eq!(snap.last_scause, 13);
    assert_eq!(snap.last_stval, 0x8020_4000);
    assert_eq!(snap.last_sepc, 0x8020_0200);

    tf.scause = 15;
    tf.stval = 0x8020_4008;
    tf.sepc = 0x8020_0300;
    stats.record(&tf, false);
    let snap = stats.snapshot();
    assert_eq!(snap.faults, 2);
    assert_eq!(snap.load_faults, 1);
    assert_eq!(snap.store_faults, 1);
    assert_eq!(snap.recovered, 1);
    assert_eq!(snap.fatal, 1);
    assert_eq!(snap.last_scause, 15);
    assert_eq!(snap.last_stval, 0x8020_4008);
    stats.reset();
    assert_eq!(stats.snapshot(), Default::default());
}

#[derive(Default)]
struct Recorder {
    out: RefCell<String>,
    halt_code: RefCell<Option<usize>>,
}

impl Platform for Recorder {
    fn put_str(&self, s: &str) {
        self.out.borrow_mut().push_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        *self.halt_code.borrow_mut() = Some(code);
        panic!("halt({code})");
    }

    fn now_ticks(&self) -> u64 {
        0
    }

    fn timebase_hz(&self) -> u64 {
        10_000_000
    }

    fn hart_id(&self) -> usize {
        0
    }

    fn set_trap_vector(&self, _vector: usize) {}

    fn enable_irq(&self, _irq: u32) -> bool {
        false
    }

    fn disable_irq(&self, _irq: u32) -> bool {
        false
    }

    fn set_timer_deadline(&self, _deadline: u64) {}

    fn enable_timer(&self) -> bool {
        false
    }

    fn disable_timer(&self) {}
}

#[test]
fn report_guard_fault_panic_va_halt_1() {
    let recorder = Recorder::default();
    let mut tf = TrapFrame::zeroed();
    tf.scause = 15;
    tf.sepc = 0x8020_0300;
    tf.stval = 0x8020_4008;
    tf.set_reg(1, 0x8020_0010);
    tf.set_reg(2, 0x8021_3000);
    tf.set_arg(0, 0x1234);
    let exc = Exception {
        scause: 15,
        kind: ExceptionKind::StorePageFault,
        sepc: 0x8020_0300,
        stval: 0x8020_4008,
        from_supervisor: true,
    };
    let caught = catch_unwind(AssertUnwindSafe(|| {
        report_guard_fault(&recorder, &exc, &tf)
    }));
    assert!(caught.is_err(), "guard fatal phải halt (panic ở host)");
    {
        let out = recorder.out.borrow();
        assert!(
            out.starts_with("PANIC: guard page fault scause=0xf stval=0x80204008 sepc=0x80200300"),
            "out = {out:?}"
        );
        assert!(out.contains("ra=0x80200010"), "out = {out:?}");
        assert!(out.contains("a0=0x1234"), "out = {out:?}");
    }
    assert_eq!(*recorder.halt_code.borrow(), Some(1));
    let mut w = PlatformWriter(&recorder);
    use std::fmt::Write as _;
    let _ = write!(w, "ok");
    assert!(recorder.out.borrow().ends_with("ok"));
}

#[test]
fn builder_map_kernel_2m_image_va_noload_4k() {
    let mut b = builder_with(64 * 1024);
    let kbase = 0x8020_0000u64;
    let high = 0xFFFF_FFC0_4000_0000u64;
    b.map(kbase, kbase, 1, PteFlags::kernel_rwx()).unwrap();
    b.map(high, kbase, 1, PteFlags::kernel_rwx()).unwrap();
    let mut va = 0x8040_0000u64;
    while va < 0x8042_5000 {
        if !(va >= 0x8040_0000 && va < 0x8040_1000) {
            b.map(va, va, 0, PteFlags::kernel_rw_nx()).unwrap();
            b.map(high + (va - kbase), va, 0, PteFlags::kernel_rw_nx())
                .unwrap();
        }
        va += PAGE_BYTES;
    }
    // Image identity + nửa cao là leaf 2 MiB (level 1); vùng NOLOAD phía sau 4 KiB.
    let kl = b.walk(kbase);
    let kh = b.walk(high);
    assert!(kl.valid && kl.leaf);
    assert!(kh.valid && kh.leaf);
    assert_eq!(kl.level, 1);
    assert_eq!(kh.level, 1);
    assert_eq!(kl.raw(), kh.raw());
    let stack = b.walk(0x8040_1000);
    assert!(stack.valid && stack.leaf && stack.level == 0);
    let stack_high = b.walk(high + 0x201000);
    assert_eq!(stack_high.raw(), stack.raw());
    assert!(!b.walk(0x8040_0000).valid, "guard phải NONE");
    assert!(b.tables() >= 5);
}
