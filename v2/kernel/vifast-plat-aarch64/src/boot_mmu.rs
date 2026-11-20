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
//! **KT-249** (B3.3): identity map (VA == PA) cho vùng ảnh kernel + UART + GIC, bật MMU với
//! `TTBR0_EL1`. **KT-253** (B3.4, D2-199): thêm bước `TTBR1_EL1` vào chuỗi (nửa cao) và
//! [`ttbr1_base`] — bảng nửa cao do [`crate::split_mmu::SplitMmu`] dựng, không phải tệp này.

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
    /// `msr ttbr1_el1` — bảng gốc nửa cao (**mức 1**, xem [`ttbr1_base`]) — KT-253/B3.4.
    WriteTtbr1,
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
            Self::WriteTtbr1 => "ttbr1",
            Self::WriteSctlr => "sctlr",
            Self::Isb => "isb",
        }
    }
}

/// Chuỗi bật MMU **bắt buộc** — 6 bước, đúng thứ tự (xem tài liệu đầu tệp).
///
/// **KT-253** thêm [`MmuStep::WriteTtbr1`] giữa `TTBR0` và `SCTLR`: nửa cao phải có gốc **trước**
/// khi `SCTLR_EL1.M = 1`, nếu không lần nhảy sang VA nửa cao sẽ fault ngay (và trên QEMU TCG
/// không có "may mắn" nào cứu).
pub const MMU_ENABLE_SEQUENCE: [MmuStep; 6] = [
    MmuStep::WriteTcr,
    MmuStep::WriteMair,
    MmuStep::WriteTtbr0,
    MmuStep::WriteTtbr1,
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
pub fn apply_step(step: MmuStep, tcr: u64, mair: u64, ttbr0: u64, ttbr1: u64, sctlr: u64) {
    match step {
        MmuStep::WriteTcr => asm::write_tcr_el1(tcr),
        MmuStep::WriteMair => asm::write_mair_el1(mair),
        MmuStep::WriteTtbr0 => asm::write_ttbr0_el1(ttbr0),
        MmuStep::WriteTtbr1 => asm::write_ttbr1_el1(ttbr1),
        MmuStep::WriteSctlr => asm::write_sctlr_el1(sctlr),
        MmuStep::Isb => asm::isb(),
    }
}

/// Bật MMU theo đúng [`MMU_ENABLE_SEQUENCE`]; trả số bước đã thi hành.
///
/// Ảnh **không** dùng hàm này trực tiếp khi cần log từng bước — nó lặp qua
/// [`MMU_ENABLE_SEQUENCE`] và gọi [`apply_step`] để in marker trước/sau mỗi bước. Hàm này giữ
/// đường "một lần gọi" cho test/đường tắt và để chuỗi có **một** định nghĩa.
pub fn enable_mmu(tcr: u64, mair: u64, ttbr0: u64, ttbr1: u64, sctlr: u64) -> usize {
    for step in MMU_ENABLE_SEQUENCE {
        apply_step(step, tcr, mair, ttbr0, ttbr1, sctlr);
    }
    MMU_ENABLE_SEQUENCE.len()
}

/// PA bảng **mức 1** dưới gốc của `table` cho VA `va` (chỉ số mức 0 của `va`), `None` nếu chưa có.
///
/// Với `T0SZ = T1SZ = 25` walk bắt đầu ở **mức 1** (xem [`start_level`]); bảng gốc mức 0 chỉ là
/// "hộp" chứa entry trỏ xuống mức 1. Chỉ số mức 0 là `0` cho nửa thấp và `0x1FF` cho nửa cao
/// (39-bit VA canonical) — nên **một** hàm phục vụ cả hai nửa, không có bản thứ hai của logic.
pub fn level1_base<M: TableMemory>(table: &PageTable<Aarch64, M>, va: u64) -> Option<u64> {
    let index = Aarch64::index(va, 0);
    let entry = table.memory().read_entry(table.root(), index)?;
    match Aarch64::decode(entry, 0) {
        Descriptor::Table { pa } => Some(pa),
        _ => None,
    }
}

/// PA của bảng **mức 1** mà `TTBR0_EL1` phải trỏ tới (con của gốc mức 0 cho nửa thấp), `None`
/// nếu chưa có.
pub fn ttbr0_base<M: TableMemory>(table: &PageTable<Aarch64, M>) -> Option<u64> {
    level1_base(table, 0)
}

/// PA của bảng **mức 1** mà `TTBR1_EL1` phải trỏ tới (con của gốc mức 0 cho nửa cao), `None`
/// nếu chưa có — KT-253/B3.4.
pub fn ttbr1_base<M: TableMemory>(table: &PageTable<Aarch64, M>) -> Option<u64> {
    level1_base(table, crate::split_mmu::HIGH_HALF_BASE)
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
