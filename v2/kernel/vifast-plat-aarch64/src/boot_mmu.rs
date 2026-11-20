// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bật MMU** trên aarch64: chuỗi `TCR → MAIR → TTBR0 → SCTLR → ISB` + identity map (KT-249).
//!
//! # Vì sao chuỗi là **dữ liệu** ([`MMU_ENABLE_SEQUENCE`])
//!
//! Thứ tự nạp thanh ghi điều khiển MMU là **bắt buộc** (`kt167-a64step2.md` §5): `TCR_EL1` và
//! `MAIR_EL1` phải có trước khi `TTBR0_EL1` được dùng, `SCTLR_EL1.M` phải là bước **cuối**, và
//! `ISB` phải theo sau để context synchronization. Sai thứ tự ⇒ walk dùng `TCR`/`MAIR` cũ, hoặc
//! MMU bật khi bảng trang chưa sẵn — cả hai đều **treo im lặng** (R5 của thiết kế).
//!
//! Vì vậy chuỗi nằm ở **dữ liệu** mà test host khẳng định được từng bước và đúng thứ tự
//! (`tests/mmu_boot.rs`), còn asm chỉ là "thi hành từng bước". Đột biến "bỏ `ISB`" vì thế làm
//! test **ĐỎ** thay vì chỉ làm hành vi sai trên máy thật.
//!
//! # Vì sao `TTBR0_EL1` là bảng **mức 1**, không phải gốc mức 0
//!
//! Với granule 4 KiB, mức bắt đầu của walk do `T0SZ` quyết định (DDI 0487 bảng D5-5):
//! `T0SZ = 25` ⇒ VA 39 bit ⇒ walk bắt đầu ở **mức 1**. `PageTable<Aarch64, _>` dựng cây **4
//! mức** (gốc mức 0 → … → lá mức 3), nên bảng gốc mức 0 **không** đưa thẳng vào `TTBR0_EL1` được:
//! phần cứng sẽ đọc nó như bảng mức 1 và mọi địa chỉ dịch sai. [`ttbr0_base`] lấy **con** của
//! gốc (bảng mức 1) — đúng mức mà phần cứng bắt đầu.
//!
//! Đột biến `T0SZ` 25→24 làm mức bắt đầu thành 0 ⇒ phần cứng đọc bảng mức 1 như bảng mức 0 ⇒
//! **translation fault** (trên QEMU: in `# trap: … ec=0x25` rồi dừng — xem journal KT-249).
//!
//! # Phạm vi
//!
//! Chỉ **identity map** (VA == PA) cho vùng ảnh kernel + UART + GIC. Nửa cao (`TTBR1_EL1`) và
//! physmap là B3.4 — **không** thuộc bước 3 (D2-195 hoãn Q4).

use crate::asm;
use crate::format::{Aarch64, Descriptor, MemAttr, Perms};
use crate::mmu::{MapError, PageTable, TableMemory};
use crate::sysreg::{Sctlr, SCTLR_M};

/// Một bước của chuỗi bật MMU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmuStep {
    /// `msr tcr_el1` — hình học walk (T0SZ/TG0/IPS/cacheability).
    WriteTcr,
    /// `msr mair_el1` — thuộc tính bộ nhớ (slot 0 Device, slot 1 Normal WB).
    WriteMair,
    /// `msr ttbr0_el1` — bảng gốc nửa thấp (**mức 1**, xem [`ttbr0_base`]).
    WriteTtbr0,
    /// `msr sctlr_el1` — bật `M` (bước **cuối** trước `ISB`).
    WriteSctlr,
    /// `isb` — context synchronization: không có nó, các lệnh sau có thể vẫn dùng trạng thái cũ.
    Isb,
}

impl MmuStep {
    /// Tên ngắn để in log (D2-102: in từ dữ liệu).
    pub const fn name(self) -> &'static str {
        match self {
            Self::WriteTcr => "tcr",
            Self::WriteMair => "mair",
            Self::WriteTtbr0 => "ttbr0",
            Self::WriteSctlr => "sctlr",
            Self::Isb => "isb",
        }
    }
}

/// Chuỗi bật MMU **bắt buộc** — 5 bước, đúng thứ tự (xem tài liệu đầu tệp).
pub const MMU_ENABLE_SEQUENCE: [MmuStep; 5] = [
    MmuStep::WriteTcr,
    MmuStep::WriteMair,
    MmuStep::WriteTtbr0,
    MmuStep::WriteSctlr,
    MmuStep::Isb,
];

/// Mức bắt đầu của walk theo `T0SZ` (granule 4 KiB), bảng D5-5 của DDI 0487.
///
/// `None` ngoài dải biểu diễn được (`T0SZ` 16..=51) — không đoán.
pub const fn start_level(t0sz: u32) -> Option<usize> {
    match t0sz {
        16..=24 => Some(0),
        25..=33 => Some(1),
        34..=42 => Some(2),
        43..=51 => Some(3),
        _ => None,
    }
}

/// `SCTLR_EL1` sau khi bật MMU: **giữ** mọi bit hiện có, chỉ đặt `M`.
///
/// Không tự bật cache (`C`/`I`): bước 3 chỉ cần MMU; bật cache là quyết định riêng (và phải đo
/// lại). Giữ bit là quan trọng — ghi đè `SCTLR_EL1` bằng hằng số sẽ tắt cấu hình mà
/// firmware/QEMU đã đặt (ví dụ `RES1` bit).
pub fn sctlr_for_mmu_on(current: u64) -> u64 {
    Sctlr::from_bits(current).with_mmu(true).bits()
}

/// `true` ⇔ `SCTLR_EL1` đang bật MMU (`M = 1`).
pub fn mmu_is_on(sctlr: u64) -> bool {
    sctlr & SCTLR_M != 0
}

/// Thi hành **một** bước của chuỗi với các giá trị đã dựng.
///
/// Tách khỏi [`enable_mmu`] để ảnh in được marker **trước và sau từng bước** (R5 của thiết kế:
/// bật MMU sai thứ tự ⇒ treo không log).
pub fn apply_step(step: MmuStep, tcr: u64, mair: u64, ttbr0: u64, sctlr: u64) {
    match step {
        MmuStep::WriteTcr => asm::write_tcr_el1(tcr),
        MmuStep::WriteMair => asm::write_mair_el1(mair),
        MmuStep::WriteTtbr0 => asm::write_ttbr0_el1(ttbr0),
        MmuStep::WriteSctlr => asm::write_sctlr_el1(sctlr),
        MmuStep::Isb => asm::isb(),
    }
}

/// Bật MMU theo đúng [`MMU_ENABLE_SEQUENCE`]; trả số bước đã thi hành.
///
/// Ảnh **không** dùng hàm này trực tiếp khi cần log từng bước — nó lặp qua
/// [`MMU_ENABLE_SEQUENCE`] và gọi [`apply_step`] để in marker trước/sau mỗi bước. Hàm này giữ
/// đường "một lần gọi" cho test/đường tắt và để chuỗi có **một** định nghĩa.
pub fn enable_mmu(tcr: u64, mair: u64, ttbr0: u64, sctlr: u64) -> usize {
    for step in MMU_ENABLE_SEQUENCE {
        apply_step(step, tcr, mair, ttbr0, sctlr);
    }
    MMU_ENABLE_SEQUENCE.len()
}

/// PA của bảng **mức 1** mà `TTBR0_EL1` phải trỏ tới (con của gốc mức 0), `None` nếu chưa có.
///
/// Với `T0SZ = 25` walk bắt đầu ở mức 1 (xem [`start_level`]); bảng gốc mức 0 chỉ là "hộp" chứa
/// đúng một entry (chỉ số 0) trỏ xuống mức 1 cho nửa thấp 39-bit.
pub fn ttbr0_base<M: TableMemory>(table: &PageTable<Aarch64, M>) -> Option<u64> {
    let entry = table.memory().read_entry(table.root(), 0)?;
    match Aarch64::decode(entry, 0) {
        Descriptor::Table { pa } => Some(pa),
        _ => None,
    }
}

/// Identity map `[start, end)` bằng trang 4 KiB; trả **số trang** đã map.
///
/// Hai đầu được làm tròn **ra ngoài** (map đủ trang chứa chúng) vì ảnh map theo symbol linker
/// (`__text_start`, `__kernel_end`) — hai giá trị này không hứa căn 4 KiB. Khoảng rỗng ⇒ `Ok(0)`.
///
/// Lỗi của `PageTable::map` được trả nguyên (không nuốt): `AlreadyMapped` khi map chồng, `OutOfFrames`
/// khi arena cạn — cả hai phải làm boot **PANIC**, không im lặng bỏ trang.
pub fn map_identity<M: TableMemory>(
    table: &mut PageTable<Aarch64, M>,
    start: u64,
    end: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    if end <= start {
        return Ok(0);
    }
    let first = start & !0xFFF;
    let last = (end + 0xFFF) & !0xFFF;
    let mut pages = 0;
    let mut va = first;
    while va < last {
        table.map_with(va, va, perms, attr, crate::format::Shareability::Inner)?;
        pages += 1;
        va += 0x1000;
    }
    Ok(pages)
}
