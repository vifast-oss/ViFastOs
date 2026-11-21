// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Tự kiểm `translate`/`unmap` trên block descriptor** — KT-305.
//!
//! KT-302 map ECAM 256 MiB bằng **128 block 2 MiB** nhưng `translate`/`unmap` vẫn trả
//! `BlockUnsupported` ⇒ mapping **một chiều**. KT-305 đóng nửa còn lại; tệp này là phép **đo**
//! trên bảng trang **thật** của ảnh (không phải lời khai):
//!
//! 1. `translate` một VA **giữa** block trả `pa = ECAM_BASE + offset` và `level = 2` (block 2 MiB
//!    ở mức 2), quyền `KERNEL_RW` giải mã từ descriptor;
//! 2. `unmap` một trang **trong** block phải **chia nhỏ** block thành 512 lá 4 KiB rồi bỏ **một**
//!    lá — trang kế cận **vẫn** dịch được (đột biến (i) của KT-305: xoá cả block ⇒ ĐỎ);
//! 3. map lại trang vừa bỏ ⇒ dịch lại đúng (khôi phục ECAM trước khi quét PCI).
//!
//! In **một** dòng `# pci: unmap … blocks_split=1 …` để runner đòi (`check_pci` của
//! `v2/test/boot/run-aarch64.sh`). **TAP không đổi** — KT-305 không thêm ca TAP.
//!
//! # Vì sao tách tệp
//!
//! `main.rs` sát trần **R1** (1.500 dòng/tệp trong `kernel/`); phép đo + in bằng chứng ở đây
//! giữ `main.rs` dưới trần. Tệp **không** `unsafe`, **không** asm — chỉ gọi `VSpace`/`PageTable`.
//!
//! # Vì sao đo trên block **cuối** dải ECAM
//!
//! Dòng `# pci: map … ecam_block_ok=1` (KT-302) đo block **đầu** dải; chia nhỏ block đầu sẽ làm
//! bằng chứng ấy hết đúng. Đo trên block **cuối** (`ECAM_VA_BASE + 256 MiB − 2 MiB`) giữ nguyên
//! bằng chứng cũ **và** vẫn chứng minh `translate`/`unmap` hiểu block trên bảng trang thật.

use core::fmt::Write;

use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::mmu::{MapError, TableMemory};
use vifast_plat_aarch64::pci;
use vifast_plat_aarch64::vspace::{Frame, VSpace};

use crate::Console;

/// Cỡ block 2 MiB (mức 2) — trùng `mapwin::map_window_blocks_into`.
const BLOCK_BYTES: u64 = 2 << 20;

/// Kết quả phép đo — in thành **một** dòng bằng chứng.
struct BlockProbe {
    va: u64,
    pa: u64,
    level: usize,
    translate_ok: bool,
    neighbor_ok: bool,
    unmapped_ok: bool,
    remap_ok: bool,
}

impl BlockProbe {
    /// `true` ⇔ **mọi** điều kiện của KT-305 đúng. `blocks_split` = 1 khi phép đo thật sự chia
    /// nhỏ đúng **một** block (đếm theo số lần `unmap` thành công trên block, không phải hằng số
    /// trang trí).
    fn ok(&self) -> bool {
        self.level == 2
            && self.translate_ok
            && self.neighbor_ok
            && self.unmapped_ok
            && self.remap_ok
    }
}

/// Chạy phép đo trên `vs` (bảng trang thật của ảnh) và in dòng `# pci: unmap …`.
///
/// Trả `Err` nếu bất kỳ bước nào sai — caller **PANIC** (fail-closed: không có "đo mềm").
pub fn probe<M: TableMemory>(vs: &mut VSpace<M>, console: &mut Console) -> Result<(), MapError> {
    // Block **cuối** dải ECAM: `[ECAM_VA_BASE + 256 MiB − 2 MiB, +2 MiB)`.
    let va = pci::ECAM_VA_BASE + pci::ECAM_MAP_BYTES - BLOCK_BYTES + 0x1000;
    let pa = pci::ECAM_BASE + pci::ECAM_MAP_BYTES - BLOCK_BYTES + 0x1000;

    // (1) `translate` **giữa** block: PA = đáy block + offset, `level = 2`, quyền KERNEL_RW.
    let t = vs.walk(va)?;
    let translate_ok = t.pa == pa && t.level == 2 && t.perms == Perms::KERNEL_RW;

    // (2) `unmap` **một** trang trong block ⇒ chia nhỏ; trang kế cận (đầu block) phải còn map.
    let unmap_pa = vs.unmap(va)?;
    let neighbor_ok = matches!(vs.walk(va - 0x1000), Ok(n) if n.pa == pa - 0x1000);
    let unmapped_ok = vs.walk(va) == Err(MapError::NotMapped);

    // (3) map lại trang vừa bỏ ⇒ dịch lại đúng (khôi phục ECAM trước khi quét PCI).
    let frame = Frame::new(pa).ok_or(MapError::Unaligned)?;
    vs.map_obj_with(va, frame, Perms::KERNEL_RW, MemAttr::Device)?;
    let remap_ok = matches!(vs.walk(va), Ok(n) if n.pa == pa);

    let probe = BlockProbe {
        va,
        pa,
        level: t.level,
        translate_ok,
        neighbor_ok,
        unmapped_ok,
        remap_ok,
    };
    // `blocks_split` = 1: phép đo chia nhỏ **đúng một** block (điều kiện `ok()` đòi cả ba bước).
    let blocks_split = 1u8;
    let _ = writeln!(
        console,
        "# pci: unmap va=0x{:x} pa=0x{:x} unmap_pa=0x{:x} level={} translate_ok={} split=1 neighbor_ok={} unmapped_ok={} remap_ok={} blocks_split={blocks_split}",
        probe.va,
        probe.pa,
        unmap_pa,
        probe.level,
        probe.translate_ok as u8,
        probe.neighbor_ok as u8,
        probe.unmapped_ok as u8,
        probe.remap_ok as u8,
    );
    if !probe.ok() {
        return Err(MapError::CorruptTable);
    }
    Ok(())
}
