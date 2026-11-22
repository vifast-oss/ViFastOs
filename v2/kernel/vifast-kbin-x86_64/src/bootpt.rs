// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340b** — bảng trang boot + **arena bảng trang động** ra **ngoài `.bss`** (D2-244).
//!
//! # Vì sao tệp này tồn tại
//!
//! Trước KT-340b, tám bảng trang của `_start` (`__pml4`, `__pdpt`, `__pd`, `__pt_kernel`,
//! `__pd_mmio`, `__pd_mmio_low`, `__pd_user`, `__pt_user` = **32 KiB**) là `.skip 4096` trong
//! `.bss` của `global_asm!`, cộng thêm `__syscall_stack` 32 KiB và bốn stack IST1 16 KiB ⇒
//! `.data+.bss` ảnh sản phẩm **96.320 B** (D2-236/D2-243). Ở `KT-340c`, `vifast-core::Machine`
//! (`184.792 B`) sẽ được link vào chính ảnh này ⇒ `96.320 + 184.792 = 281.112 B > 262.144 B`
//! (trần I7) — **cổng chặn** thật sự.
//!
//! # Cách hạ (khuôn `KT-338a` của aarch64, thích ứng cho boot 32-bit của x86_64)
//!
//! Các bảng trang **không** còn là byte trong `.bss`. Chúng nằm trong output section
//! **NOLOAD** `.bootpt` (linker `x86_64` đặt ngay sau `.smp_stacks`, phía trên ảnh, trong
//! `[__text_start, __kernel_end)` nên identity map `0..2 MiB` của `_start` phủ tới):
//!
//! - **8 bảng boot** — cùng symbol TUYỆT ĐỐI (`__pml4`…`__pt_user`) nên `_start` 32-bit và
//!   `wx::enable` **không đổi một lệnh asm nào** (điều quan trọng: ở real/protected mode 32-bit
//!   không có cách tính PA nào khác ngoài symbol tuyệt đối);
//! - **arena 32 frame = 128 KiB** — nguồn frame cho `vifast-pte::PageTable` của [`crate::vspace`]
//!   (mỗi `VSpace` cấp phát bảng trang riêng từ đây, **không** `static` nào trong `.bss`).
//!
//! NOLOAD nghĩa là **không** chiếm `filesz` của LOAD segment, không phải writable data của ảnh —
//! đúng loại section mà D2-243 cho phép ("section stack riêng kiểu `.smp_stacks`"). Bộ nhớ vẫn
//! là RAM thật; `_start` zero `.bootpt` (`.bss` zero hộ không phủ tới nó) rồi mới ghi entry.
//!
//! # Đột biến (i) — `--features mut-pt-bss`
//!
//! Tám bảng boot mất `link_section` ⇒ rơi về `.bss` (đúng trạng thái trước KT-340b, 32 KiB).
//! `budget-x86_64.sh` phải **ĐỎ** (`.data+.bss` 43.072 + 32.768 = 75.840 > 65.536 B). Đột biến
//! **không** đổi hành vi boot — chứng minh phép đo ngân sách là thứ duy nhất bắt được nó.
//!
//! # Vì sao **không** dùng `__kernel_end` + `AtomicU64` như `KT-338a`
//!
//! KT-338a tính arena = `align_up(__kernel_end)` lúc chạy vì `mmu_bringup` của aarch64 chạy
//! **sau** khi Rust đã có mặt. Ở x86_64, bảng trang boot được dựng trong `_start` **32-bit**
//! trước long mode: mọi symbol phải là địa chỉ tuyệt đối, không có allocator, không có biến
//! 64-bit. Đặt arena ở **địa chỉ liên kết** trong `.bootpt` cho cùng kết quả (RAM ngoài `.bss`,
//! `VA == PA` nhờ identity map) mà **không** thêm một dòng asm 16/32-bit nào. `__kernel_end`
//! vẫn là biên trên: linker đặt `.bootpt` bên trong nó nên mọi phép đo `pages=` của W^X không
//! đổi.

use core::sync::atomic::{AtomicU64, Ordering};

/// Số entry một bảng trang x86_64 (`2^9 = 512`).
pub const ENTRIES_PER_FRAME: usize = 512;

/// Số frame 4 KiB của **arena bảng trang động** (khuôn `KT-338a`: 8 frame boot + dự phòng cho
/// `VSpace` của tác vụ ring 3 — mỗi `VSpace` ~5 frame; 32 frame = 8 `VSpace` cùng lúc).
pub const BOOT_ARENA_FRAMES: usize = 32;

/// Một bảng trang 4 KiB (512 entry × 8 B), căn 4 KiB.
///
/// `AtomicU64` (không `[u64; 512]`) vì entry được ghi/đọc qua `asm::write_u64/read_u64` và vì
/// kiểu này **không** cần `static mut`/`unsafe` ở phía Rust (D2-17: `unsafe` chỉ trong crate
/// `vifast-plat-*`; tệp này không có dòng `unsafe` nào).
#[repr(C, align(4096))]
pub struct Table(pub [AtomicU64; ENTRIES_PER_FRAME]);

impl Table {
    /// Bảng rỗng — zero từng entry (không phụ thuộc `_start` zero `.bootpt`, để giá trị khởi tạo
    /// vẫn đúng nếu ai đó đọc trước `_start`).
    pub const ZERO: Self = Self([const { AtomicU64::new(0) }; ENTRIES_PER_FRAME]);
}

/// Arena bảng trang động — 32 frame 4 KiB liên tiếp.
#[repr(C, align(4096))]
pub struct Arena(pub [AtomicU64; BOOT_ARENA_FRAMES * ENTRIES_PER_FRAME]);

impl Arena {
    /// Arena rỗng (zero) — frame cấp phát vẫn được `TableMemory::zero_frame` zero lại.
    pub const ZERO: Self =
        Self([const { AtomicU64::new(0) }; BOOT_ARENA_FRAMES * ENTRIES_PER_FRAME]);
}

/// **Đột biến (i)**: có feature `mut-pt-bss` ⇒ **không** `link_section` ⇒ bảng rơi về `.bss`
/// (đúng trạng thái trước KT-340b để `budget-x86_64.sh` bắt). Không phải feature sản phẩm.
///
/// Tám khai báo viết **tường minh** (không `macro_rules!`): `check-wake-paths.py --strict` từ
/// chối macro có `$…:meta`/thuộc tính không đọc được (`E_STRICT_MACRO_INCLUDE`, fail-closed
/// KT-194/KT-202) — một macro "cho gọn" ở đây làm đỏ cổng `v2 check`, đúng thứ luật muốn chặn.
///
/// PML4 — gốc cây 4 cấp (`CR3` trỏ vào đây).
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pml4: Table = Table::ZERO;

/// PDPT — 512 entry × 1 GiB: `[0]` identity 1 GiB, `[2]` MMIO 2..3 GiB, `[3]` MMIO 3..4 GiB,
/// `[256]` cây con **user** ring 3 của đường tĩnh (VA `0x40_0000_0000`, KT-270/KT-304).
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pdpt: Table = Table::ZERO;

/// PD identity 1 GiB bằng 512 lá lớn 2 MiB (`PD[0]` bị `wx::enable` thay bằng `__pt_kernel`).
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pd: Table = Table::ZERO;

/// PT 4 KiB cho `0..2 MiB` — W^X của `KT-340a` (RX/R/RW-NX theo đoạn linker).
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pt_kernel: Table = Table::ZERO;

/// PD MMIO `3..4 GiB` (LAPIC/IOAPIC) — 512 lá 2 MiB `PCD`.
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pd_mmio: Table = Table::ZERO;

/// PD MMIO `2..3 GiB` (ECAM q35 + PCI hole) — 512 lá 2 MiB `PCD`.
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pd_mmio_low: Table = Table::ZERO;

/// PD của cây con **user** tĩnh (`PDPT[256]`).
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pd_user: Table = Table::ZERO;

/// PT của cây con user tĩnh — lá 0 = trang mã ring 3, lá 1 = trang stack user.
#[cfg_attr(not(feature = "mut-pt-bss"), link_section = ".bootpt")]
#[no_mangle]
pub static __pt_user: Table = Table::ZERO;

/// **Arena** bảng trang động — `link_section = ".bootpt"` **luôn** (chỉ 8 bảng boot mới là đối
/// tượng của đột biến (i); arena là thứ mới của KT-340b, không có "trạng thái trước" để trả về).
#[link_section = ".bootpt"]
#[no_mangle]
pub static __boot_arena: Arena = Arena::ZERO;

/// Bộ đếm frame đã cấp của arena — `.data` (8 B), **không** nằm trong `.bss`.
static ARENA_NEXT: AtomicU64 = AtomicU64::new(0);

/// PA (== VA, identity map) của frame đầu arena.
///
/// `__boot_arena` do linker đặt 4 KiB-aligned (`#[repr(C, align(4096))]`), nên địa chỉ symbol
/// chính là PA. Không có `unsafe`: `addr_of!` chỉ lấy địa chỉ, không deref.
pub fn arena_base() -> u64 {
    core::ptr::addr_of!(__boot_arena) as u64
}

/// Số frame của arena ([`BOOT_ARENA_FRAMES`]).
pub const fn arena_frames() -> usize {
    BOOT_ARENA_FRAMES
}

/// Bộ đếm cấp phát **chia sẻ** giữa mọi `TableMemory` handle (mỗi `VSpace` một handle, cùng
/// arena) — `&'static AtomicU64` để `vifast-pte` cấp frame nguyên tử mà không cần `&mut`.
pub fn arena_next() -> &'static AtomicU64 {
    &ARENA_NEXT
}

/// Frame đã cấp (bằng chứng cho log `# vspace: … frames=…`).
pub fn arena_used() -> usize {
    ARENA_NEXT.load(Ordering::Acquire) as usize
}
