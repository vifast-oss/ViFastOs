// SPDX-License-Identifier: MIT OR Apache-2.0
//! **SMP aarch64** — PSCI `CPU_ON`, `MPIDR_EL1`, arena stack **per-hart** (KT-259 / K4' B3.9).
//!
//! Tệp này là phần **dữ liệu + số học** của việc đánh thức hart phụ: nó **không** asm, **không**
//! `unsafe`, **không** `cfg(target_arch)` — nên `cargo test` trên host khẳng định được từng hằng
//! và từng phép tính địa chỉ. Phần `hvc #0`/`mrs mpidr_el1` thật nằm ở [`crate::asm`] (D2-17).
//!
//! # Vì sao hart id lấy từ `MPIDR_EL1`, **không** tin `x3` (context id)
//!
//! PSCI `CPU_ON(target_cpu, entry, context_id)` chuyển `context_id` cho hart đích trong `x0`.
//! Đó là **giá trị do hart gọi tự đặt** — nếu nó sai (đánh số lệch, gọi hai lần với hai context
//! khác nhau) thì hart phụ sẽ chạy trên stack của hart khác mà không ai biết. Vì vậy
//! [`_start_secondary`](crate) đọc `MPIDR_EL1` của **chính nó**, lấy `Aff0` làm hart id
//! ([`hart_index`]), rồi **so** với `context_id`; lệch ⇒ PANIC (fail-loud), không "đoán theo x3".
//!
//! # Vì sao `Aff0` là hart id (và khi nào **không** phải)
//!
//! QEMU `virt` đặt chỉ số CPU vào `Aff0` và để `Aff1..3 = 0` (đo được: DTB `cpu@0`/`cpu@1` có
//! `reg = <0>`/`<1>`, `MPIDR_EL1` của hart 1 = `0x8000_0001` — log `kt259-aarch64-smp.md`).
//! [`hart_index`] vì thế **từ chối** (`None`) mọi MPIDR có `Aff1..3` khác 0 hoặc `Aff0 >=
//! MAX_HARTS`: topology lạ thì dừng, không lấy `Aff0` của nó làm chỉ số arena.
//!
//! # Arena stack per-hart
//!
//! Mỗi hart một **record** `guard 4 KiB | stack 64 KiB` ([`HART_GUARD_BYTES`] +
//! [`HART_STACK_BYTES`] = [`HART_STRIDE`] `0x1_1000`), `MAX_HARTS = 8` (D2-195 Q5) ⇒ vùng NOLOAD
//! [`HART_AREA_BYTES`] = `0x8_8000` (544 KiB) do `linker.ld` cấp (`__smp_stacks_start`).
//!
//! - **Guard nằm ngay dưới đáy stack của chính hart đó** ⇒ tràn stack đi xuống chạm guard của
//!   mình, không phải guard của hart khác (cùng khuôn riscv64 KT-66).
//! - Hart 0 **cũng** dùng slot 0: `linker.ld` đặt `__stack_top = __smp_stacks_start + HART_STRIDE`
//!   nên `_start` không đổi, và mọi hart có **cùng một quy tắc** địa chỉ — không có nhánh riêng.
//! - [`check_layout`] là phép kiểm **thuần số học** của bố cục: record rời nhau, căn trang, không
//!   tràn. Đột biến "bỏ guard khỏi stride" làm hai record đè nhau ⇒ `Overlap` (test host
//!   `stack_overlap_without_guard_is_rejected`).
//!
//! # Phạm vi (D2-195 + D2-202)
//!
//! Chỉ **2 hart**: `CPU_ON` hart 1, GICv3 SGI 0 làm IPI, cả hai in hart id. **Không** smp 4,
//! **không** timer per-hart, **không** preemption, **không** U-mode. **Không** tuyên bố K4' đạt.

/// Số hart tối đa `[TK]` — **8**, đúng D2-195 Q5 (giữ nguyên từ K3).
///
/// Crate arch **không** phụ thuộc `vifast-plat-common` (tránh vòng dev-dep — xem
/// `vifast-kbin-aarch64/Cargo.toml`), nên hằng được khai báo lại ở đây; ảnh kiểm bằng
/// `const _: () = assert!(smp::MAX_HARTS == vifast_plat_common::smp::MAX_HARTS);` — lệch là
/// **lỗi biên dịch**, không phải lỗi lúc chạy.
pub const MAX_HARTS: usize = 8;

/// Trang **guard** của một hart (4 KiB) `[TK]` — nằm ngay **dưới** đáy stack của chính hart đó.
pub const HART_GUARD_BYTES: u64 = 0x1000;

/// Stack của một hart (64 KiB) `[TK]` — NOLOAD, `sp` bắt đầu ở **đỉnh** và mọc xuống guard.
///
/// 64 KiB = đúng cỡ `.stack` mà KT-247 đã dùng cho hart 0, nên hart 0 **không** bị đổi cỡ stack
/// khi chuyển sang arena (đổi cỡ là thay đổi đường boot đã đo, không cần thiết cho KT-259).
pub const HART_STACK_BYTES: u64 = 0x1_0000;

/// Bước giữa hai record hart: `guard + stack` = `0x1_1000` (68 KiB).
pub const HART_STRIDE: u64 = HART_GUARD_BYTES + HART_STACK_BYTES;

/// Tổng vùng NOLOAD theo hart: `MAX_HARTS × HART_STRIDE` = `0x8_8000` (544 KiB).
pub const HART_AREA_BYTES: u64 = HART_STRIDE * MAX_HARTS as u64;

/// `true` ⇔ `hart` là hart id hợp lệ (`< MAX_HARTS`).
pub const fn valid_hart(hart: usize) -> bool {
    hart < MAX_HARTS
}

/// Offset **guard** của `hart` trong arena (đầu record); `None` nếu hart ngoài dải.
pub const fn guard_offset(hart: usize) -> Option<u64> {
    if valid_hart(hart) {
        Some(hart as u64 * HART_STRIDE)
    } else {
        None
    }
}

/// Offset **đáy stack** của `hart` (ngay trên guard của chính nó); `None` nếu hart ngoài dải.
pub const fn stack_bottom_offset(hart: usize) -> Option<u64> {
    match guard_offset(hart) {
        Some(offset) => Some(offset + HART_GUARD_BYTES),
        None => None,
    }
}

/// Offset **đỉnh stack** của `hart` (= `sp` ban đầu của hart đó); `None` nếu hart ngoài dải.
pub const fn stack_top_offset(hart: usize) -> Option<u64> {
    match guard_offset(hart) {
        Some(offset) => Some(offset + HART_STRIDE),
        None => None,
    }
}

/// Lỗi bố cục arena stack — **fail-closed**: mọi biến thể đều làm boot PANIC, không bỏ qua.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackError {
    /// `count == 0` — arena rỗng thì không hart nào có stack.
    Empty,
    /// `count > MAX_HARTS` — vượt trần D2-195 Q5.
    OutOfRange,
    /// `base` không căn 4 KiB — guard không cưỡng chế được ở granularity trang.
    Unaligned,
    /// `guard == 0`, `stack == 0`, hoặc `stride < guard + stack` (record không vừa stride).
    StrideTooSmall,
    /// Hai record **đè nhau** — đây là đột biến (iii) của KT-259.
    Overlap {
        /// Hart có record thấp hơn.
        a: usize,
        /// Hart có record cao hơn nhưng bắt đầu **trước** khi record `a` kết thúc.
        b: usize,
    },
}

/// Kiểm **thuần số học** một bố cục arena: `count` record `guard | stack`, bước `stride`.
///
/// Tách khỏi [`HartStacks`] để test host gọi được với **stride sai** (đột biến iii) mà không phải
/// thêm API "layout tuỳ ý" vào đường sản phẩm. Thứ tự kiểm là **có chủ ý**: overlap được báo
/// **trước** `StrideTooSmall` khi `count >= 2`, vì đó là triệu chứng cụ thể hơn (hai hart dùng
/// chung bộ nhớ) và là đúng ca đột biến.
pub fn check_layout(
    base: u64,
    stride: u64,
    guard: u64,
    stack: u64,
    count: usize,
) -> Result<(), StackError> {
    if count == 0 {
        return Err(StackError::Empty);
    }
    if count > MAX_HARTS {
        return Err(StackError::OutOfRange);
    }
    if base & 0xFFF != 0 {
        return Err(StackError::Unaligned);
    }
    if guard == 0 || stack == 0 {
        return Err(StackError::StrideTooSmall);
    }
    let record = guard.checked_add(stack).ok_or(StackError::StrideTooSmall)?;
    // Record `i` = [base + i*stride, base + i*stride + record). Hai record đè nhau ⇔
    // `stride < record` (với `stride > 0`); kiểm tường minh từng cặp để **báo đúng cặp**.
    let mut a = 0;
    while a < count {
        let a_start = base
            .checked_add(
                (a as u64)
                    .checked_mul(stride)
                    .ok_or(StackError::OutOfRange)?,
            )
            .ok_or(StackError::OutOfRange)?;
        let a_end = a_start.checked_add(record).ok_or(StackError::OutOfRange)?;
        let mut b = a + 1;
        while b < count {
            let b_start = base
                .checked_add(
                    (b as u64)
                        .checked_mul(stride)
                        .ok_or(StackError::OutOfRange)?,
                )
                .ok_or(StackError::OutOfRange)?;
            if b_start < a_end {
                return Err(StackError::Overlap { a, b });
            }
            b += 1;
        }
        a += 1;
    }
    if stride < record {
        return Err(StackError::StrideTooSmall);
    }
    Ok(())
}

/// Arena stack per-hart đã kiểm căn trang — mọi phép đổi `hart → địa chỉ` đi qua đây.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HartStacks {
    base: u64,
}

impl HartStacks {
    /// Dựng arena tại `base` (PA của `__smp_stacks_start`); `None` nếu `base` không căn 4 KiB.
    ///
    /// Không kiểm toàn bộ bố cục ở đây (đó là việc của [`Self::check`]): hàm này chỉ chặn giá trị
    /// **không thể dùng** (căn trang) để không có `HartStacks` nào ở trạng thái nửa hợp lệ.
    pub const fn new(base: u64) -> Option<Self> {
        if base & 0xFFF != 0 {
            return None;
        }
        Some(Self { base })
    }

    /// PA gốc arena (`__smp_stacks_start`).
    pub const fn base(&self) -> u64 {
        self.base
    }

    /// `(đầu, cuối)` vùng **guard** của `hart`; `None` nếu hart ngoài dải.
    pub const fn guard(&self, hart: usize) -> Option<(u64, u64)> {
        match guard_offset(hart) {
            Some(offset) => {
                let start = self.base + offset;
                Some((start, start + HART_GUARD_BYTES))
            }
            None => None,
        }
    }

    /// `(đáy, đỉnh)` vùng **stack** của `hart`; `None` nếu hart ngoài dải.
    pub const fn stack(&self, hart: usize) -> Option<(u64, u64)> {
        match (stack_bottom_offset(hart), stack_top_offset(hart)) {
            (Some(bottom), Some(top)) => Some((self.base + bottom, self.base + top)),
            _ => None,
        }
    }

    /// `sp` ban đầu của `hart` (đỉnh stack); `None` nếu hart ngoài dải.
    pub const fn stack_top(&self, hart: usize) -> Option<u64> {
        match stack_top_offset(hart) {
            Some(offset) => Some(self.base + offset),
            None => None,
        }
    }

    /// Hart **sở hữu** địa chỉ `addr` nếu `addr` nằm trong stack của hart đó; `None` nếu nằm trong
    /// guard hoặc ngoài arena.
    ///
    /// Đây là phép kiểm của đột biến (iii): hai hart chồng stack ⇒ `slot_of(sp_hart0) ==
    /// slot_of(sp_hart1)` ⇒ boot đỏ.
    pub fn slot_of(&self, addr: u64) -> Option<usize> {
        let mut hart = 0;
        while hart < MAX_HARTS {
            if let Some((bottom, top)) = self.stack(hart) {
                if addr >= bottom && addr < top {
                    return Some(hart);
                }
            }
            hart += 1;
        }
        None
    }

    /// Kiểm bố cục **thật** của arena (stride/guard/stack của crate, `MAX_HARTS` record).
    pub fn check(&self) -> Result<(), StackError> {
        check_layout(
            self.base,
            HART_STRIDE,
            HART_GUARD_BYTES,
            HART_STACK_BYTES,
            MAX_HARTS,
        )
    }
}

// =====================================================================================
// MPIDR_EL1
// =====================================================================================

/// Vị trí `Aff0[7:0]` trong `MPIDR_EL1` (DDI 0487 §D17.2.103).
pub const MPIDR_AFF0_SHIFT: u32 = 0;
/// Vị trí `Aff1[15:8]`.
pub const MPIDR_AFF1_SHIFT: u32 = 8;
/// Vị trí `Aff2[23:16]`.
pub const MPIDR_AFF2_SHIFT: u32 = 16;
/// Vị trí `Aff3[39:32]`.
pub const MPIDR_AFF3_SHIFT: u32 = 32;
/// Mặt nạ một trường affinity (8 bit).
pub const MPIDR_AFF_MASK: u64 = 0xFF;
/// Bit `MT` (bit 31) — `1` với mọi CPU ARMv8 đơn luồng; **không** thuộc affinity.
pub const MPIDR_MT: u64 = 1 << 31;

/// Bóc `(Aff0, Aff1, Aff2, Aff3)` từ `MPIDR_EL1` — bốn trường 8 bit, bỏ `MT`/`U`.
pub const fn mpidr_affinity(mpidr: u64) -> (u8, u8, u8, u8) {
    (
        ((mpidr >> MPIDR_AFF0_SHIFT) & MPIDR_AFF_MASK) as u8,
        ((mpidr >> MPIDR_AFF1_SHIFT) & MPIDR_AFF_MASK) as u8,
        ((mpidr >> MPIDR_AFF2_SHIFT) & MPIDR_AFF_MASK) as u8,
        ((mpidr >> MPIDR_AFF3_SHIFT) & MPIDR_AFF_MASK) as u8,
    )
}

/// Hart id tuyến tính từ `MPIDR_EL1`: `Aff0` **chỉ khi** `Aff1..3 == 0` và `Aff0 < MAX_HARTS`.
///
/// `None` = topology không phải phẳng (hoặc hart ngoài trần) ⇒ caller **dừng**, không đoán.
pub const fn hart_index(mpidr: u64) -> Option<usize> {
    let (aff0, aff1, aff2, aff3) = mpidr_affinity(mpidr);
    if aff1 != 0 || aff2 != 0 || aff3 != 0 {
        return None;
    }
    if (aff0 as usize) < MAX_HARTS {
        Some(aff0 as usize)
    } else {
        None
    }
}

/// `MPIDR_EL1` **kỳ vọng** của `hart` theo topology phẳng của QEMU `virt` (`Aff0 = hart`,
/// `Aff1..3 = 0`, `MT = 1`); `None` nếu hart ngoài dải.
pub const fn mpidr_for_hart(hart: usize) -> Option<u64> {
    if valid_hart(hart) {
        Some(MPIDR_MT | (hart as u64))
    } else {
        None
    }
}

// =====================================================================================
// PSCI
// =====================================================================================

/// `PSCI CPU_ON` — function id **64-bit** `0xC400_0003` (ARM DEN 0022 §5.7).
///
/// Tham số: `x1 = target_cpu` (MPIDR của hart đích, QEMU `virt` dùng `Aff0` = chỉ số CPU như
/// `reg` trong DTB), `x2 = entry_pa` (**PA**, MMU của hart đích còn tắt), `x3 = context_id`
/// (PSCI chuyển nguyên văn vào `x0` của hart đích).
pub const PSCI_CPU_ON: u64 = 0xC400_0003;

/// `PSCI_SUCCESS` — hart đích đã được đánh thức.
pub const PSCI_SUCCESS: i64 = 0;
/// `PSCI_INVALID_PARAMETERS` (`-2`) — `target_cpu`/`entry` không hợp lệ.
pub const PSCI_INVALID_PARAMS: i64 = -2;
/// `PSCI_ALREADY_ON` (`-4`) — hart đích đã chạy (gọi `CPU_ON` hai lần).
pub const PSCI_ALREADY_ON: i64 = -4;

/// `true` ⇔ mã trả PSCI là **thành công** (`0`). Mọi mã khác — kể cả mã lạ — là đỏ (fail-closed).
pub const fn cpu_on_ok(rc: i64) -> bool {
    rc == PSCI_SUCCESS
}

/// Tên **chữ** của mã trả PSCI để in log (D2-102: in từ dữ liệu, không in số trần một mình).
pub const fn cpu_on_error_name(rc: i64) -> &'static str {
    match rc {
        PSCI_SUCCESS => "success",
        PSCI_INVALID_PARAMS => "invalid-params",
        PSCI_ALREADY_ON => "already-on",
        -1 => "not-supported",
        -3 => "interrupted",
        -5 => "denied",
        _ => "unknown",
    }
}

// =====================================================================================
// Thanh ghi MMU mà hart phụ phải nạp (hart 0 công bố trước `CPU_ON`)
// =====================================================================================

/// Năm thanh ghi điều khiển MMU mà hart phụ phải nạp để dùng **cùng** bảng trang với hart 0.
///
/// Hart phụ khởi động với MMU **tắt** (PSCI reset state) nên nó không thể đọc thanh ghi của hart
/// 0; hart 0 vì thế **công bố** giá trị đã dùng vào [`crate::smp`] (ảnh giữ trong `AtomicU64`,
/// `dsb sy` trước `hvc`) và hart phụ nạp lại đúng chuỗi [`crate::boot_mmu::MMU_ENABLE_SEQUENCE`].
///
/// `sctlr` là giá trị **sau** khi bật MMU trên hart 0 (`M = 1`): hart phụ ghi nó ở bước cuối, nên
/// không có khoảng thời gian nào nó chạy với `M = 0` sau khi đã nạp `TTBR*`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MmuBootRegs {
    /// `TCR_EL1` — hình học walk (39-bit VA, 4 KiB granule, IPS 48-bit).
    pub tcr: u64,
    /// `MAIR_EL1` — thuộc tính bộ nhớ (slot 0 Device, slot 1 Normal WB).
    pub mair: u64,
    /// `TTBR0_EL1` — gốc nửa thấp (bảng **mức 1**).
    pub ttbr0: u64,
    /// `TTBR1_EL1` — gốc nửa cao (bảng **mức 1**).
    pub ttbr1: u64,
    /// `SCTLR_EL1` **sau** khi bật MMU (`M = 1`).
    pub sctlr: u64,
}

impl MmuBootRegs {
    /// Dựng bộ giá trị (hàm const để ảnh dùng được trong `static`).
    pub const fn new(tcr: u64, mair: u64, ttbr0: u64, ttbr1: u64, sctlr: u64) -> Self {
        Self {
            tcr,
            mair,
            ttbr0,
            ttbr1,
            sctlr,
        }
    }

    /// `true` ⇔ mọi trường **đã được công bố** (khác 0) — hart phụ kiểm trước khi nạp.
    ///
    /// `ttbr0`/`ttbr1` khác 0 là điều kiện sống còn: nạp `TTBR0 = 0` rồi bật `M` ⇒ walk đọc
    /// frame 0 (không phải bảng trang) ⇒ fault ngay ở lệnh kế tiếp, không log.
    pub const fn is_complete(&self) -> bool {
        self.tcr != 0 && self.mair != 0 && self.ttbr0 != 0 && self.ttbr1 != 0 && self.sctlr != 0
    }
}
