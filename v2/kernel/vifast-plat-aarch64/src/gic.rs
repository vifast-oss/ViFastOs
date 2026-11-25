// SPDX-License-Identifier: MIT OR Apache-2.0
//! **GICv3** trên QEMU `virt` aarch64 — distributor + redistributor + CPU interface (KT-259).
//!
//! Phạm vi **hẹp có chủ ý**: chỉ đủ để một **SGI** (IPI) đi từ hart 0 sang hart 1. **Không** LPI,
//! **không** ITS, **không** PPI/timer, **không** ưu tiên lồng nhau, **không** `GICD_IROUTER` cho
//! SPI (SPI thuộc bước sau). Mọi thứ khác giữ nguyên trạng thái reset.
//!
//! # Vì sao GICv3 (và vì sao runner phải `gic-version=3`)
//!
//! QEMU `virt` **mặc định GICv2** (`arm,cortex-a15-gic` — đo bằng `-M virt,dumpdtb` + `dtc`,
//! 2026-11-21). GICv2 **không** có redistributor và **không** có `ICC_*_EL1`; muốn dùng GICv3
//! phải truyền `-M virt,gic-version=3` (runner `run-aarch64.sh` làm việc đó). Với cờ ấy, DTB ghi
//! `arm,gic-v3` và memory map là:
//!
//! | Vùng | Base | Size |
//! |---|---|---|
//! | `GICD` distributor | `0x0800_0000` | `0x1_0000` |
//! | `GICR` redistributor (mỗi hart) | `0x080A_0000 + hart × 0x2_0000` | `0x2_0000` |
//!
//! Mỗi redistributor `0x2_0000` gồm **hai** frame 64 KiB: `RD_base` (điều khiển/wake) và
//! `SGI_base = RD_base + 0x1_0000` (enable/priority cho INTID `0..31`).
//!
//! # Chuỗi khởi tạo (thứ tự **bắt buộc**)
//!
//! ```text
//!   hart 0: GICD_CTLR ← ARE_NS | EnableGrp1NS   (chờ RWP = 0)
//!           GICR_WAKER(hart 0) ← clear ProcessorSleep (chờ ChildrenAsleep = 0)
//!           ICC_SRE_EL1.SRE = 1 → ICC_PMR_EL1 = 0xF0 → ICC_IGRPEN1_EL1 = 1
//!   hart 1: GICR_WAKER(hart 1) ← clear ProcessorSleep (chờ ChildrenAsleep = 0)
//!           GICR_IGROUPR0 bit SGI = 1 (Group 1 NS)
//!           GICR_ISENABLER0 bit SGI = 1
//!           ICC_SRE_EL1.SRE = 1 → ICC_PMR_EL1 = 0xF0 → ICC_IGRPEN1_EL1 = 1
//!           DAIF.I = 0 (nhận ngắt)
//! ```
//!
//! **Đột biến (i) của KT-259**: bỏ bước `GICR_WAKER` clear của hart 1 ⇒ redistributor của nó
//! vẫn ngủ ⇒ SGI **không** được chuyển tới CPU ⇒ TAP 11 đỏ (log `kt259-mutation.log`). Đó là lý
//! do bước wake nằm ở **hart 1** (mỗi CPU tự đánh thức redistributor của mình, như Linux
//! `gic_cpu_init`), không phải hart 0 làm hộ.
//!
//! # Host stub
//!
//! Các hàm `init_*`/`send_sgi`/`ack_irq` đọc/ghi MMIO qua [`crate::asm`] — trên host chúng
//! **panic** (stub fail-loud của `asm`). Phần **dữ liệu** (offset, bit, encoding `ICC_SGI1R_EL1`)
//! thì thuần số học và được test host khẳng định tuyệt đối (`tests/gic.rs`).

use crate::asm;
use crate::smp::{valid_hart, MAX_HARTS};

// =====================================================================================
// Địa chỉ (QEMU `virt`, `gic-version=3`)
// =====================================================================================

/// Base **redistributor** của QEMU `virt` (DTB `intc@8000000` `reg[1]`).
pub const GICR_BASE: u64 = 0x080A_0000;

/// Bước giữa hai redistributor: `0x2_0000` (RD 64 KiB + SGI 64 KiB).
pub const GICR_STRIDE: u64 = 0x2_0000;

/// Offset frame **SGI** trong một redistributor (`SGI_base = RD_base + 0x1_0000`).
pub const GICR_SGI_OFFSET: u64 = 0x1_0000;

// =====================================================================================
// GICD — distributor
// =====================================================================================

/// `GICD_CTLR` — điều khiển chung (IHI 0069 §8.2.1).
pub const GICD_CTLR: u64 = 0x0000;
/// `GICD_TYPER` — số dòng ngắt + khả năng (IHI 0069 §8.2.2).
pub const GICD_TYPER: u64 = 0x0004;
/// `GICD_IGROUPR<n>` — nhóm của INTID `32n..32n+31` (IHI 0069 §8.2.4).
pub const GICD_IGROUPR: u64 = 0x0080;
/// `GICD_ISENABLER<n>` — bật INTID `32n..32n+31` (IHI 0069 §8.2.5).
pub const GICD_ISENABLER: u64 = 0x0100;
/// `GICD_IPRIORITYR<n>` — ưu tiên từng INTID (IHI 0069 §8.2.9).
pub const GICD_IPRIORITYR: u64 = 0x0400;
/// `GICD_ICENABLER<n>` — tắt INTID `32n..32n+31` (IHI 0069 §8.2.6). **KT-406** dùng để tắt lại
/// đúng SPI đã bật, không đụng SGI/PPI.
pub const GICD_ICENABLER: u64 = 0x0180;
/// `GICD_IROUTER<n>` — affinity đích của SPI `n`, **8 byte** mỗi INTID (IHI 0069 §8.2.12).
/// **KT-406**: SPI (UART) cần `IROUTER` vì `ARE_NS = 1`; SGI không dùng thanh ghi này.
pub const GICD_IROUTER: u64 = 0x6000;

/// INTID đầu tiên của dải **SPI** (IHI 0069 §2.2.1): SPI `n` ⇔ INTID `32 + n`.
pub const SPI_FIRST: u64 = 32;
/// INTID **kết thúc** dải SPI (không bao gồm): `1020` là đầu dải đặc biệt.
pub const SPI_END: u64 = 1020;
/// Ưu tiên gán cho SPI (thấp hơn `ICC_PMR_EL1 = 0xF0` nên được chuyển tới CPU).
pub const SPI_PRIORITY: u8 = 0xA0;

/// `GICD_CTLR.EnableGrp1NS` (bit 1) — bật nhóm 1 non-secure.
pub const GICD_CTLR_ENABLE_GRP1NS: u32 = 1 << 1;
/// `GICD_CTLR.ARE_NS` (bit 4) — affinity routing cho non-secure (chế độ GICv3).
pub const GICD_CTLR_ARE_NS: u32 = 1 << 4;
/// `GICD_CTLR.RWP` (bit 31) — `1` ⇔ còn ghi đang chờ; phải chờ về `0` sau mỗi lần ghi.
pub const GICD_CTLR_RWP: u32 = 1 << 31;

// =====================================================================================
// GICR — redistributor
// =====================================================================================

/// `GICR_CTLR` (RD_base) — điều khiển redistributor (IHI 0069 §9.2.1).
pub const GICR_CTLR: u64 = 0x0000;
/// `GICR_TYPER` (RD_base, 64-bit) — affinity + `Last` (IHI 0069 §9.2.3).
pub const GICR_TYPER: u64 = 0x0008;
/// `GICR_WAKER` (RD_base) — đánh thức redistributor (IHI 0069 §9.2.7).
pub const GICR_WAKER: u64 = 0x0014;
/// `GICR_WAKER.ProcessorSleep` (bit 1) — `1` ⇒ redistributor ngủ, **không** chuyển ngắt tới CPU.
pub const GICR_WAKER_PROCESSOR_SLEEP: u32 = 1 << 1;
/// `GICR_WAKER.ChildrenAsleep` (bit 2) — `0` sau khi clear `ProcessorSleep` ⇒ đã thức.
pub const GICR_WAKER_CHILDREN_ASLEEP: u32 = 1 << 2;

/// `GICR_IGROUPR0` (SGI_base) — nhóm của INTID `0..31` (IHI 0069 §9.3.2).
pub const GICR_IGROUPR0: u64 = 0x0080;
/// `GICR_ISENABLER0` (SGI_base) — bật INTID `0..31` (IHI 0069 §9.3.3).
pub const GICR_ISENABLER0: u64 = 0x0100;
/// `GICR_IPRIORITYR<n>` (SGI_base) — ưu tiên INTID `0..31` (IHI 0069 §9.3.7).
pub const GICR_IPRIORITYR: u64 = 0x0400;

/// Ưu tiên gán cho SGI của IPI (`0xA0`) — thấp hơn `ICC_PMR_EL1` (`0xF0`) nên được chuyển.
pub const SGI_PRIORITY: u8 = 0xA0;

// =====================================================================================
// ICC_*_EL1 — CPU interface (system registers)
// =====================================================================================

/// `ICC_SRE_EL1.SRE` (bit 0) — `1` ⇒ dùng **system register** interface (bắt buộc trước `ICC_PMR`).
pub const ICC_SRE_EL1_SRE: u64 = 1 << 0;
/// `ICC_PMR_EL1` — mặt nạ ưu tiên: `0xF0` cho qua mọi ưu tiên `0x00..0xEF`.
pub const ICC_PMR_ALL: u64 = 0xF0;
/// `ICC_IGRPEN1_EL1.Enable` (bit 0) — bật nhóm 1.
pub const ICC_IGRPEN1_ENABLE: u64 = 1;

/// INTID nhỏ nhất của dải **đặc biệt** `1020..=1023` (IHI 0069 §2.2.1) — không phải ngắt thật.
pub const ICC_SPURIOUS_MIN: u64 = 1020;

/// SGI dùng làm IPI của KT-259 (`SGI 0`).
pub const SGI_IPI: u32 = 0;

/// SGI **shootdown `TLBI`** của KT-262 (`SGI 1`) — hart 0 → mọi hart phụ.
///
/// Tách khỏi [`SGI_IPI`] có chủ ý: `note_ipi` của KT-259 đếm **mọi** SGI 0 nhận được và in
/// `# smp: ipi recv=<n>`; dùng chung số thì dòng ấy xuất hiện nhiều lần và runner ĐỎ (đúng ca âm
/// (g) của `run-aarch64.sh`). Ba SGI của KT-262 vì thế là **ba số khác nhau**.
pub const SGI_SHOOTDOWN: u32 = 1;

/// SGI **lệnh** của KT-262 (`SGI 2`) — hart 0 yêu cầu một hart phụ thực thi một việc
/// (in alive / đọc VA để nạp TLB / đọc lại để sinh fault).
pub const SGI_COMMAND: u32 = 2;

/// Vị trí `TargetList[15:0]` trong `ICC_SGI1R_EL1` (DDI 0487 §D17.2.55).
pub const ICC_SGI1R_TARGET_LIST_SHIFT: u32 = 0;
/// Vị trí `Aff1[23:16]`.
pub const ICC_SGI1R_AFF1_SHIFT: u32 = 16;
/// Vị trí `INTID[31:24]`.
pub const ICC_SGI1R_INTID_SHIFT: u32 = 24;
/// Vị trí `Aff2[39:32]`.
pub const ICC_SGI1R_AFF2_SHIFT: u32 = 32;
/// Vị trí `IRM` (bit 40) — `1` ⇒ gửi tới mọi hart **trừ** hart gọi.
pub const ICC_SGI1R_IRM_SHIFT: u32 = 40;
/// Bit `IRM`.
pub const ICC_SGI1R_IRM: u64 = 1 << ICC_SGI1R_IRM_SHIFT;
/// Vị trí `Aff3[47:44]`.
pub const ICC_SGI1R_AFF3_SHIFT: u32 = 44;
/// Vị trí `RS[55:48]` — chọn dải affinity (0 = dải của hart gọi).
pub const ICC_SGI1R_RS_SHIFT: u32 = 48;

/// Số vòng chờ tối đa cho một bit trạng thái MMIO (`RWP`, `ChildrenAsleep`).
///
/// **Có trần** là bắt buộc: chờ vô hạn trên một thiết bị không trả lời là treo im lặng (R5 của
/// thiết kế). Trần này rộng hơn nhiều so với đo được trên QEMU (vài chục vòng).
pub const MMIO_WAIT_LIMIT: u32 = 1_000_000;

/// Lỗi khởi tạo GIC — mọi biến thể làm boot **PANIC** (fail-loud), không bỏ qua.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GicError {
    /// Hart ngoài `MAX_HARTS` — không có redistributor để lập trình.
    InvalidHart,
    /// `intid` ngoài dải **SPI** `32..1020` — `IROUTER`/`ISENABLER` word không tồn tại (**KT-406**).
    InvalidSpi,
    /// Chờ bit trạng thái quá [`MMIO_WAIT_LIMIT`] vòng (thiết bị không trả lời).
    Timeout(&'static str),
    /// `GICR_TYPER.Affinity_Value` không khớp hart đang lập trình — sai redistributor.
    AffinityMismatch {
        /// Affinity kỳ vọng (từ hart id).
        expected: u64,
        /// Affinity đọc được từ `GICR_TYPER`.
        got: u64,
    },
}

/// Thông tin distributor đọc lại sau khởi tạo — bằng chứng cho log boot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GicInfo {
    /// `GICD_CTLR` sau khi ghi (đã chờ `RWP = 0`).
    pub ctlr: u32,
    /// `GICD_TYPER` (số dòng ngắt + `ARE`).
    pub typer: u32,
    /// Số INTID hỗ trợ = `32 × (ITLinesNumber + 1)`.
    pub lines: u32,
}

/// Base **redistributor** của `hart`; `None` nếu hart ngoài `MAX_HARTS`.
pub const fn redistributor_base(hart: usize) -> Option<u64> {
    if valid_hart(hart) {
        Some(GICR_BASE + (hart as u64) * GICR_STRIDE)
    } else {
        None
    }
}

/// Base frame **SGI** của `hart` (`RD_base + 0x1_0000`); `None` nếu hart ngoài `MAX_HARTS`.
pub const fn sgi_frame_base(hart: usize) -> Option<u64> {
    match redistributor_base(hart) {
        Some(base) => Some(base + GICR_SGI_OFFSET),
        None => None,
    }
}

/// `TargetList` của `ICC_SGI1R_EL1`: **bitmask `Aff0`** — hart `n` là bit `n`.
///
/// `Aff0 >= 8` không biểu diễn được trong 16 bit của trường này theo cách của KT-259 (một hart
/// một bit, `Aff1..3 = 0`) ⇒ trả `0` để caller **từ chối**, không gửi nhầm.
pub const fn sgi_target_list(aff0: u8) -> u16 {
    if (aff0 as usize) < MAX_HARTS {
        1u16 << aff0
    } else {
        0
    }
}

/// Ghép `ICC_SGI1R_EL1` từ các trường (DDI 0487 §D17.2.55).
///
/// `rs = 0` (dải affinity của hart gọi) và `irm = false` (gửi theo `TargetList`) là đường của
/// KT-259; hai tham số còn lại để hàm **đủ** theo đặc tả và test được vị trí bit.
pub const fn sgi1r(intid: u32, target_list: u16, aff1: u8, aff2: u8, aff3: u8, irm: bool) -> u64 {
    ((intid as u64) << ICC_SGI1R_INTID_SHIFT)
        | ((target_list as u64) << ICC_SGI1R_TARGET_LIST_SHIFT)
        | ((aff1 as u64) << ICC_SGI1R_AFF1_SHIFT)
        | ((aff2 as u64) << ICC_SGI1R_AFF2_SHIFT)
        | ((aff3 as u64) << ICC_SGI1R_AFF3_SHIFT)
        | if irm { ICC_SGI1R_IRM } else { 0 }
}

/// `ICC_SGI1R_EL1` để gửi `intid` tới **hart** (topology phẳng: `Aff0 = hart`, `Aff1..3 = 0`);
/// `None` nếu hart ngoài `MAX_HARTS`.
pub const fn sgi1r_for_hart(intid: u32, hart: usize) -> Option<u64> {
    if !valid_hart(hart) {
        return None;
    }
    let target = sgi_target_list(hart as u8);
    if target == 0 {
        return None;
    }
    Some(sgi1r(intid, target, 0, 0, 0, false))
}

/// `true` ⇔ `intid` là INTID **đặc biệt** `1020..=1023` (spurious) — handler **không** được đếm.
pub const fn is_spurious(intid: u64) -> bool {
    intid >= ICC_SPURIOUS_MIN
}

/// `true` ⇔ `intid` là **SGI** (`0..15`) — dải duy nhất mà KT-259 dùng cho IPI.
pub const fn is_sgi(intid: u64) -> bool {
    intid < 16
}

// =====================================================================================
// SPI — **KT-406** (UART RX IRQ thật)
// =====================================================================================

/// `true` ⇔ `intid` thuộc dải **SPI** `32..1020`.
pub const fn is_spi(intid: u64) -> bool {
    intid >= SPI_FIRST && intid < SPI_END
}

/// `intid` của SPI `spi` (`Some(32 + spi)`); `None` nếu ra ngoài dải SPI.
///
/// Tách khỏi [`spi_word`]/[`spi_bit`] để ba hàm cùng một nguồn tính dải — không chỗ nào tự
/// cộng `32` bằng tay.
pub const fn spi_intid(spi: u32) -> Option<u64> {
    let intid = SPI_FIRST + spi as u64;
    if intid < SPI_END {
        Some(intid)
    } else {
        None
    }
}

/// Chỉ số **word** `n` của `GICD_ISENABLER<n>`/`GICD_IGROUPR<n>`/`GICD_IPRIORITYR` cho `intid`:
/// `intid / 32` (INTID `32..63` ⇒ word 1). `None` nếu không phải SPI.
pub const fn spi_word(intid: u64) -> Option<u64> {
    if is_spi(intid) {
        Some(intid / 32)
    } else {
        None
    }
}

/// Bit trong word của `intid` (`intid % 32`), kèm `GICD_ISENABLER`/`IGROUPR`.
pub const fn spi_bit(intid: u64) -> Option<u32> {
    if is_spi(intid) {
        Some(1u32 << (intid % 32))
    } else {
        None
    }
}

/// Giá trị `GICD_IROUTER<n>` cho affinity phẳng (IHI 0069 §8.2.12):
/// `Aff0[7:0] | Aff1[15:8] | Aff2[23:16] | Aff3[39:32] | IRM[40]`.
pub const fn spi_router(aff0: u8, aff1: u8, aff2: u8, aff3: u8, irm: bool) -> u64 {
    (aff0 as u64)
        | ((aff1 as u64) << 8)
        | ((aff2 as u64) << 16)
        | ((aff3 as u64) << 32)
        | if irm { 1 << 40 } else { 0 }
}

/// Báo cáo SPI đọc lại từ thanh ghi sau [`enable_spi`] — bằng chứng `key=value` cho log boot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpiInfo {
    /// INTID đã bật (SPI `n` ⇒ `32 + n`).
    pub intid: u64,
    /// `true` ⇔ `GICD_ISENABLER<n>` đọc lại có bit của INTID.
    pub enabled: bool,
    /// `GICD_IROUTER<n>` đọc lại (affinity đích).
    pub router: u64,
    /// `GICD_IPRIORITYR<intid>` đọc lại (một byte).
    pub priority: u8,
}

// =====================================================================================
// MMIO — chỉ chạy trên ảnh (host stub panic)
// =====================================================================================

/// Đọc 32 bit MMIO.
fn read32(addr: u64) -> u32 {
    asm::mmio_read_u32(addr)
}

/// Ghi 32 bit MMIO.
fn write32(addr: u64, value: u32) {
    asm::mmio_write_u32(addr, value)
}

/// Chờ `GICD_CTLR.RWP = 0` (ghi distributor đã hoàn tất); lỗi ⇒ [`GicError::Timeout`].
fn wait_rwp(dist_base: u64) -> Result<(), GicError> {
    let mut i = 0;
    while i < MMIO_WAIT_LIMIT {
        if read32(dist_base + GICD_CTLR) & GICD_CTLR_RWP == 0 {
            return Ok(());
        }
        i += 1;
    }
    Err(GicError::Timeout("gicd-ctlr-rwp"))
}

/// Khởi tạo **distributor**: `ARE_NS | EnableGrp1NS`, chờ `RWP = 0`, đọc lại `GICD_TYPER`.
///
/// Chỉ hart 0 gọi (distributor là tài nguyên **dùng chung**; gọi hai lần từ hai hart là tranh
/// chấp không cần thiết cho phạm vi 2 hart).
pub fn init_distributor(dist_base: u64) -> Result<GicInfo, GicError> {
    let typer = read32(dist_base + GICD_TYPER);
    let lines = 32 * ((typer & 0x1F) + 1);
    // Một lần ghi: bật affinity routing **và** nhóm 1 non-secure (trạng thái reset là 0 nên
    // không cần bước "tắt trước" của GICv3 §4.3).
    write32(
        dist_base + GICD_CTLR,
        GICD_CTLR_ARE_NS | GICD_CTLR_ENABLE_GRP1NS,
    );
    wait_rwp(dist_base)?;
    Ok(GicInfo {
        ctlr: read32(dist_base + GICD_CTLR),
        typer,
        lines,
    })
}

/// Đánh thức redistributor của `hart`: clear `ProcessorSleep`, chờ `ChildrenAsleep = 0`.
///
/// Trả `GICR_TYPER` để caller **kiểm affinity** (đang lập trình đúng redistributor của hart mình).
/// **Đột biến (i)**: bỏ hàm này ở hart 1 ⇒ SGI không tới ⇒ TAP 11 đỏ.
pub fn wake_redistributor(hart: usize) -> Result<u64, GicError> {
    let base = redistributor_base(hart).ok_or(GicError::InvalidHart)?;
    let typer = read32(base + GICR_TYPER) as u64 | ((read32(base + GICR_TYPER + 4) as u64) << 32);
    let affinity = typer >> 32;
    let expected = hart as u64;
    if affinity != expected {
        return Err(GicError::AffinityMismatch {
            expected,
            got: affinity,
        });
    }
    let mut waker = read32(base + GICR_WAKER);
    waker &= !GICR_WAKER_PROCESSOR_SLEEP;
    write32(base + GICR_WAKER, waker);
    let mut i = 0;
    while i < MMIO_WAIT_LIMIT {
        if read32(base + GICR_WAKER) & GICR_WAKER_CHILDREN_ASLEEP == 0 {
            return Ok(typer);
        }
        i += 1;
    }
    Err(GicError::Timeout("gicr-waker-children-asleep"))
}

/// Bật **một SGI** trong redistributor của `hart`: nhóm 1 non-secure + ưu tiên + enable.
///
/// Thứ tự `IGROUPR0` → `IPRIORITYR` → `ISENABLER0` là thứ tự của GICv3 §4.3 (cấu hình trước khi
/// bật); đảo lại thì ngắt có thể tới với nhóm/ưu tiên cũ.
///
/// **`GICR_IPRIORITYR<n>` là thanh ghi theo BYTE** (IHI 0069 §9.3.7): mỗi INTID một byte, nên
/// ghi phải là **ghi byte**. KT-259 ghi 32 bit vào `base + 0x400 + sgi` và chỉ bật `SGI 0` nên
/// địa chỉ tình cờ căn 4; KT-262 bật thêm `SGI 1`/`SGI 2` ⇒ địa chỉ lệch lề ⇒ **alignment
/// fault** trên Device memory (`ec=0x25`, `FAR=0x080B_0401` — log `kt262-red.log`).
pub fn enable_sgi(hart: usize, sgi: u32) -> Result<(), GicError> {
    let base = sgi_frame_base(hart).ok_or(GicError::InvalidHart)?;
    if sgi >= 16 {
        return Err(GicError::InvalidHart);
    }
    let bit = 1u32 << sgi;
    write32(base + GICR_IGROUPR0, read32(base + GICR_IGROUPR0) | bit);
    let priority = base + GICR_IPRIORITYR + (sgi as u64);
    asm::mmio_write_u8(priority, SGI_PRIORITY);
    write32(base + GICR_ISENABLER0, bit);
    Ok(())
}

/// Bật **một SPI** trên distributor cho hart `aff0` — **KT-406** (UART RX IRQ thật).
///
/// Thứ tự cấu hình (GICv3 §4.3): `IGROUPR` (nhóm 1 NS) → `IPRIORITYR` → `IROUTER` (đích) →
/// `ISENABLER`; rồi chờ `RWP = 0` và **đọc lại** cả ba thanh ghi. Đọc lại là bắt buộc: một
/// write bị bỏ (ví dụ `IROUTER` khi `ARE_NS = 0`) vẫn "thành công" im lặng, và ngắt SPI sẽ
/// không bao giờ tới CPU.
///
/// Trả [`SpiInfo`] chứa giá trị **đọc lại** — log boot in từ đây, không in hằng mong đợi.
pub fn enable_spi(dist_base: u64, intid: u64, aff0: u8) -> Result<SpiInfo, GicError> {
    let word = spi_word(intid).ok_or(GicError::InvalidSpi)?;
    let bit = spi_bit(intid).ok_or(GicError::InvalidSpi)?;
    write32(
        dist_base + GICD_IGROUPR + word * 4,
        read32(dist_base + GICD_IGROUPR + word * 4) | bit,
    );
    asm::mmio_write_u8(dist_base + GICD_IPRIORITYR + intid, SPI_PRIORITY);
    asm::mmio_write_u64(
        dist_base + GICD_IROUTER + intid * 8,
        spi_router(aff0, 0, 0, 0, false),
    );
    write32(dist_base + GICD_ISENABLER + word * 4, bit);
    wait_rwp(dist_base)?;
    let enabled = read32(dist_base + GICD_ISENABLER + word * 4) & bit != 0;
    let priority = asm::mmio_read_u8(dist_base + GICD_IPRIORITYR + intid);
    let router = asm::mmio_read_u8(dist_base + GICD_IROUTER + intid * 8) as u64
        | ((asm::mmio_read_u8(dist_base + GICD_IROUTER + intid * 8 + 1) as u64) << 8)
        | ((asm::mmio_read_u8(dist_base + GICD_IROUTER + intid * 8 + 2) as u64) << 16)
        | ((asm::mmio_read_u8(dist_base + GICD_IROUTER + intid * 8 + 4) as u64) << 32);
    Ok(SpiInfo {
        intid,
        enabled,
        router,
        priority,
    })
}

/// Tắt lại **một SPI** đã bật bằng [`enable_spi`]; `true` ⇔ đọc lại thấy bit đã 0.
pub fn disable_spi(dist_base: u64, intid: u64) -> Result<bool, GicError> {
    let word = spi_word(intid).ok_or(GicError::InvalidSpi)?;
    let bit = spi_bit(intid).ok_or(GicError::InvalidSpi)?;
    write32(dist_base + GICD_ICENABLER + word * 4, bit);
    wait_rwp(dist_base)?;
    Ok(read32(dist_base + GICD_ISENABLER + word * 4) & bit == 0)
}

/// `true` ⇔ SPI `intid` đang bật (đọc `GICD_ISENABLER`), `false` nếu `intid` ngoài dải SPI.
pub fn spi_enabled(dist_base: u64, intid: u64) -> bool {
    match (spi_word(intid), spi_bit(intid)) {
        (Some(word), Some(bit)) => read32(dist_base + GICD_ISENABLER + word * 4) & bit != 0,
        _ => false,
    }
}

/// Khởi tạo **CPU interface** của hart đang chạy: `ICC_SRE_EL1.SRE = 1`, `ICC_PMR_EL1 = 0xF0`,
/// `ICC_IGRPEN1_EL1 = 1`; trả giá trị `ICC_SRE_EL1` đọc lại (bằng chứng `SRE = 1`).
///
/// `SRE` phải bật **trước** `PMR`/`IGRPEN1`: khi `SRE = 0`, các ghi `ICC_PMR_EL1`/
/// `ICC_IGRPEN1_EL1` bị bỏ qua (DDI 0487 §D17.2.53) ⇒ ngắt không bao giờ tới, và lỗi ấy **im
/// lặng** nếu không đọc lại.
pub fn init_cpu_interface() -> Result<u64, GicError> {
    asm::write_icc_sre_el1(asm::read_icc_sre_el1() | ICC_SRE_EL1_SRE);
    asm::isb();
    let sre = asm::read_icc_sre_el1();
    if sre & ICC_SRE_EL1_SRE == 0 {
        return Err(GicError::Timeout("icc-sre-el1"));
    }
    asm::write_icc_pmr_el1(ICC_PMR_ALL);
    asm::write_icc_igrpen1_el1(ICC_IGRPEN1_ENABLE);
    asm::isb();
    Ok(sre)
}

/// Đọc `GICR_WAKER` của `hart` — bằng chứng redistributor đã thức.
///
/// `ProcessorSleep = 0` **và** `ChildrenAsleep = 0` là trạng thái "đã thức". Đọc lại sau
/// [`wake_redistributor`] để log chứng minh bước wake **thật sự** chạy (không phải giả định).
pub fn redistributor_waker(hart: usize) -> Result<u32, GicError> {
    let base = redistributor_base(hart).ok_or(GicError::InvalidHart)?;
    Ok(read32(base + GICR_WAKER))
}

/// Gửi **SGI** `sgi` tới `hart` qua `ICC_SGI1R_EL1` (topology phẳng).
pub fn send_sgi(hart: usize, sgi: u32) -> Result<(), GicError> {
    let value = sgi1r_for_hart(sgi, hart).ok_or(GicError::InvalidHart)?;
    asm::write_icc_sgi1r_el1(value);
    asm::isb();
    Ok(())
}

/// Đọc `ICC_IAR1_EL1` — INTID của ngắt đang chờ (`>= 1020` là spurious, xem [`is_spurious`]).
pub fn ack_irq() -> u64 {
    asm::read_icc_iar1_el1()
}

/// Ghi `ICC_EOIR1_EL1` — kết thúc ngắt `intid` (bắt buộc, nếu không ngắt không bao giờ tới lại).
pub fn end_irq(intid: u64) {
    asm::write_icc_eoir1_el1(intid);
    asm::isb();
}
