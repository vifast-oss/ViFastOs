// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`BootInfo` Dynamic Arena** cho chế độ `ViFastOs-General` (`KT-410`, `D2-259` mục 5).
//!
//! # Vì sao có tệp này
//!
//! `Machine<UntypedStore>` (`vifast-core::store::UntypedStore`) mang bảng **rộng** cho mô hình
//! General: `GENERAL_MAX_TCBS = 512`, `GENERAL_MAX_CAPS = 1.024`, `GENERAL_MAX_OBJECTS = 1.024`
//! ⇒ `size_of::<Machine<UntypedStore, NoopObserver>>()` ≈ **0,6 MiB**. Vùng đó **không** được
//! nằm trong `.bss` của ảnh (trần `.data + .bss ≤ 256 KiB`, QD §3.5) — nó phải được **cắt từ
//! RAM usable mà firmware khai** (`BootInfo::ram`), tức là **arena động lúc boot**.
//!
//! Tệp này là tầng **chính sách an toàn** (crate giữ `#![forbid(unsafe_code)]`):
//!
//! 1. chọn vùng RAM usable **lớn nhất** trong [`BootInfo`];
//! 2. cắt **một** arena ở **cuối** vùng đó (`[end - bytes, end)`), căn theo `align_of::<Machine>()`;
//! 3. tính **offset** của `Machine` trong arena bằng [`vifast_objmem::Plan`] (cùng công thức căn
//!    lề với [`vifast_objmem::Arena`] thật);
//! 4. `validate()` **fail-closed**: arena phải đủ chỗ + `base_pa` phải căn `align`.
//!
//! Việc **ghi** `Machine` vào `base_pa + offset` (qua `physmap`) nằm ở crate arch
//! (`kernel/vifast-plat-*`, `unsafe` được phép theo D2-17) — tệp này **không** chạm con trỏ.
//!
//! # Hợp đồng
//!
//! - `from_bootinfo` trả `None` khi không vùng RAM nào đủ chỗ ⇒ caller **phải** rơi về
//!   `StaticStore` (Mission) hoặc báo lỗi boot; **không** cắt chồng lên vùng khác.
//! - Arena cắt ở **cuối** vùng lớn nhất để không đụng ảnh kernel (nạp ở đầu RAM).
//! - `machine_offset()` là offset **tương đối** trong arena; `machine_pa()` là PA tuyệt đối.

use core::mem::{align_of, size_of};

use vifast_core::observer::NoopObserver;
use vifast_core::store::{UntypedStore, GENERAL_MAX_CAPS, GENERAL_MAX_OBJECTS, GENERAL_MAX_TCBS};
use vifast_core::Machine;
use vifast_objmem::Plan;

use crate::bootinfo::BootInfo;

/// Kiểu `Machine` của chế độ **General** — `UntypedStore` + `NoopObserver` (0 B oracle ở sản
/// phẩm, `D2-259` mục 4).
pub type GeneralMachine = Machine<UntypedStore, NoopObserver>;

/// Số byte **tối thiểu** của arena để chứa trọn `Machine<GeneralMachine>`.
pub const GENERAL_MACHINE_BYTES: usize = size_of::<GeneralMachine>();

/// Căn lề **tối thiểu** của arena (căn của `Machine` — `u64`/`usize` ⇒ thường 8).
pub const GENERAL_MACHINE_ALIGN: usize = align_of::<GeneralMachine>();

/// Lỗi của [`BootArena`] — **fail-closed**, không panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaError {
    /// Không vùng RAM usable nào đủ chỗ cho `Machine<UntypedStore>`.
    NotEnoughRam,
    /// `base_pa` không căn `GENERAL_MACHINE_ALIGN`.
    Misaligned,
    /// Arena nhỏ hơn `GENERAL_MACHINE_BYTES`.
    TooSmall,
    /// `Plan` không cấp được offset (tràn/hết chỗ) — không xảy ra khi `validate` xanh.
    Layout,
}

/// Arena động cắt từ `BootInfo` cho `Machine<UntypedStore>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootArena {
    base_pa: u64,
    bytes: usize,
    machine_off: usize,
}

impl BootArena {
    /// Cắt arena lớn nhất có thể từ vùng RAM usable **lớn nhất** của `bi`.
    ///
    /// Trả `None` khi không vùng nào đủ `GENERAL_MACHINE_BYTES` (caller rơi về `StaticStore`).
    #[must_use]
    pub fn from_bootinfo(bi: &BootInfo) -> Option<Self> {
        let need = GENERAL_MACHINE_BYTES;
        let align = GENERAL_MACHINE_ALIGN;
        let mut best: Option<(u64, u64)> = None;
        for r in bi.usable_ram() {
            if !r.is_valid() || r.size < need as u64 {
                continue;
            }
            if best.is_none_or(|(_, size)| r.size > size) {
                best = Some((r.base, r.size));
            }
        }
        let (base, size) = best?;
        // Cắt ở **cuối** vùng (không đụng ảnh kernel nạp ở đầu RAM), căn xuống theo `align`.
        let end = base.checked_add(size)?;
        let bytes = need.checked_add(align - 1)? & !(align - 1);
        let arena_base = end.checked_sub(bytes as u64)? & !((align as u64) - 1);
        if arena_base < base {
            return None;
        }
        let mut plan = Plan::new(bytes);
        let machine_off = plan.alloc(need, align)?;
        Some(Self {
            base_pa: arena_base,
            bytes,
            machine_off,
        })
    }

    /// PA bắt đầu của arena.
    #[must_use]
    pub const fn base_pa(&self) -> u64 {
        self.base_pa
    }

    /// Số byte của arena.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Offset của `Machine` **trong** arena.
    #[must_use]
    pub const fn machine_offset(&self) -> usize {
        self.machine_off
    }

    /// PA tuyệt đối của `Machine` (`base_pa + machine_offset`).
    #[must_use]
    pub const fn machine_pa(&self) -> u64 {
        self.base_pa + self.machine_off as u64
    }

    /// Kiểm **fail-closed**: đủ chỗ, căn lề đúng, `Machine` nằm trọn trong arena.
    pub fn validate(&self) -> Result<(), ArenaError> {
        if self.bytes < GENERAL_MACHINE_BYTES {
            return Err(ArenaError::TooSmall);
        }
        if !self.base_pa.is_multiple_of(GENERAL_MACHINE_ALIGN as u64) {
            return Err(ArenaError::Misaligned);
        }
        let end = self
            .machine_off
            .checked_add(GENERAL_MACHINE_BYTES)
            .ok_or(ArenaError::Layout)?;
        if end > self.bytes {
            return Err(ArenaError::Layout);
        }
        Ok(())
    }

    /// Bố cục bảng General mà arena phải phủ (chẩn đoán/TAP) — `(objects, caps, tcbs)`.
    #[must_use]
    pub const fn general_bounds(&self) -> (usize, usize, usize) {
        (GENERAL_MAX_OBJECTS, GENERAL_MAX_CAPS, GENERAL_MAX_TCBS)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::bootinfo::{BootArch, BootSource, BOOTINFO_VERSION};

    fn bootinfo_with(ram: &[(u64, u64)]) -> BootInfo {
        let mut bi = BootInfo::new(BootArch::Riscv64, BootSource::Dtb);
        bi.version = BOOTINFO_VERSION;
        bi.n_harts = 1;
        for &(base, size) in ram {
            bi.push_ram(base, size).expect("vùng RAM");
        }
        bi
    }

    /// **Ca chính** — arena cắt từ vùng RAM lớn nhất, đủ chỗ cho `Machine<UntypedStore>`, căn
    /// lề đúng, và **không** chồng lên ảnh kernel (nằm ở **cuối** vùng).
    #[test]
    fn arena_cat_tu_cuoi_vung_ram_lon_nhat() {
        let bi = bootinfo_with(&[(0x8000_0000, 128 << 20), (0x9000_0000, 4 << 20)]);
        let a = BootArena::from_bootinfo(&bi).expect("128 MiB phải đủ chỗ");
        a.validate().expect("arena hợp lệ");
        assert!(
            a.bytes().is_multiple_of(GENERAL_MACHINE_ALIGN),
            "arena căn lề"
        );
        assert!(
            a.base_pa().is_multiple_of(GENERAL_MACHINE_ALIGN as u64),
            "base căn lề"
        );
        assert_eq!(a.machine_pa(), a.base_pa() + a.machine_offset() as u64);
        assert!(
            a.machine_pa() + GENERAL_MACHINE_BYTES as u64 <= 0x8000_0000 + (128 << 20),
            "Machine phải nằm trong vùng RAM đã cắt"
        );
        assert!(
            a.base_pa() >= 0x8000_0000 + (128 << 20) - a.bytes() as u64,
            "arena cắt ở CUỐI vùng (không đụng ảnh kernel ở đầu RAM)"
        );
        assert_eq!(
            a.general_bounds(),
            (GENERAL_MAX_OBJECTS, GENERAL_MAX_CAPS, GENERAL_MAX_TCBS)
        );
        std::println!(
            "# kt410 arena: base_pa=0x{:x} bytes={} machine_pa=0x{:x} machine_bytes={} \
             objects={} caps={} tcbs={}",
            a.base_pa(),
            a.bytes(),
            a.machine_pa(),
            GENERAL_MACHINE_BYTES,
            GENERAL_MAX_OBJECTS,
            GENERAL_MAX_CAPS,
            GENERAL_MAX_TCBS,
        );
    }

    /// **Ca âm** — RAM quá nhỏ ⇒ `None` (caller phải rơi về `StaticStore`, **không** cắt bừa).
    #[test]
    fn arena_ram_qua_nho_tra_none() {
        let bi = bootinfo_with(&[(0x8000_0000, 64 << 10)]);
        assert_eq!(
            BootArena::from_bootinfo(&bi),
            None,
            "64 KiB < Machine<UntypedStore> ⇒ None (fail-closed)"
        );
        // Không có vùng RAM nào ⇒ None.
        let empty = bootinfo_with(&[]);
        assert_eq!(BootArena::from_bootinfo(&empty), None);
    }

    /// **Ca biên** — `validate` bắt arena hỏng (nhỏ hơn `Machine`, lệch căn, `Machine` tràn).
    #[test]
    fn validate_bat_arena_hong() {
        let small = BootArena {
            base_pa: 0x8000_0000,
            bytes: GENERAL_MACHINE_BYTES - 1,
            machine_off: 0,
        };
        assert_eq!(small.validate(), Err(ArenaError::TooSmall));
        let misaligned = BootArena {
            base_pa: 0x8000_0001,
            bytes: GENERAL_MACHINE_BYTES,
            machine_off: 0,
        };
        assert_eq!(misaligned.validate(), Err(ArenaError::Misaligned));
        let over = BootArena {
            base_pa: 0x8000_0000,
            bytes: GENERAL_MACHINE_BYTES,
            machine_off: 8,
        };
        assert_eq!(over.validate(), Err(ArenaError::Layout));
    }
}
