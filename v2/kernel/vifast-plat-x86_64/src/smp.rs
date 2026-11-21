// SPDX-License-Identifier: MIT OR Apache-2.0
//! **SMP x86_64** — arena stack per-hart, `INIT`/`SIPI` qua ICR, bố cục trang trampoline —
//! KT-276 (K5' bước 6: 2 hart).
//!
//! Tệp này là phần **dữ liệu + số học** của việc đánh thức hart phụ trên x86_64: nó **không**
//! asm, **không** `unsafe`, **không** MMIO — nên `cargo test` trên host khẳng định được từng
//! hằng và từng phép tính địa chỉ. Phần ghi MMIO (`ICR`), `lgdt`/`lidt`/`mov crN` và trampoline
//! 16-bit nằm ở asm của ảnh (`vifast-kbin-x86_64::smp`) và [`crate::asm`] (D2-17).
//!
//! # Vì sao hart id **không** lấy từ `LAPIC_ID` một cách mù quáng
//!
//! QEMU `-smp 2` đặt `LAPIC_ID` = chỉ số CPU (đo được: BSP `id=0`, AP `id=1`), nhưng đó là
//! **topology của QEMU**, không phải bảo đảm của đặc tả: ACPI MADT mới là nguồn sự thật của
//! `APIC ID ↔ processor`. Bước này vì thế dùng [`apic_id_for_hart`] (topology phẳng của QEMU) để
//! **gửi** `INIT`/`SIPI`, và hart phụ **tự đọc** `LAPIC_ID` của mình rồi **so** với giá trị BSP
//! đã ghi vào boot block — lệch ⇒ PANIC (fail-loud), không "đoán theo chỉ số".
//!
//! # Vì sao trampoline nằm ở `0x8000`
//!
//! `SIPI` chỉ mang **8 bit vector**; CPU phụ bắt đầu ở **real mode** với `CS:IP = (vector << 8):0`
//! ⇒ địa chỉ **vật lý** `vector << 12`, phải `< 1 MiB` và căn 4 KiB (SDM §9.1.4, §10.12.10).
//! `0x8000` ⇒ vector `0x08`. Vùng `0x8000..0x9000` của PC là conventional memory **trên** EBDA
//! (`0x9FC00`) và trên `hvm_start_info` của PVH (`0x1580`, đo được) — nhưng **không** sống sót
//! qua SeaBIOS: đo được `u32 @ 0x8000 = 0` sau khi ảnh chạy, nên ảnh **copy** trampoline xuống
//! lúc chạy (xem `vifast-kbin-x86_64::smp`) thay vì tin linker/QEMU đặt sẵn.
//!
//! # Arena stack per-hart
//!
//! Mỗi hart một **record** `guard 4 KiB | stack 64 KiB` ([`HART_GUARD_BYTES`] +
//! [`HART_STACK_BYTES`] = [`HART_STRIDE`] `0x1_1000`), `MAX_HARTS = 8` (D2-195 Q5) ⇒ vùng NOLOAD
//! [`HART_AREA_BYTES`] = `0x8_8000` (544 KiB) do `linker.ld` cấp (`__smp_stacks_start`).
//!
//! - **Guard nằm ngay dưới đáy stack của chính hart đó** ⇒ tràn stack đi xuống chạm guard của
//!   mình, không phải guard của hart khác (cùng khuôn aarch64 KT-259 / riscv64 KT-66).
//! - Hart 0 **cũng** dùng slot 0: `linker.ld` đặt `__stack_top = __smp_stacks_start + HART_STRIDE`
//!   nên `_start` không đổi, và mọi hart có **cùng một quy tắc** địa chỉ — không có nhánh riêng.
//! - [`check_layout`] là phép kiểm **thuần số học** của bố cục: record rời nhau, căn trang, không
//!   tràn. Đột biến "bỏ guard khỏi stride" làm hai record đè nhau ⇒ `Overlap` (test host
//!   `layout_rejects_overlap_without_guard`).
//!
//! # Phạm vi (KT-276 → KT-278)
//!
//! KT-276 làm **2 hart** (`INIT-SIPI-SIPI` hart 1, LAPIC riêng cho AP, cả hai in hart id).
//! **KT-278** mở lên **4 hart** (`SMP_HARTS = 4`, `INIT-SIPI-SIPI` tuần tự cho hart 1..3) và thêm
//! **IPI** fixed-delivery qua `ICR` ([`icr_fixed_low`], vector [`crate::exc::VEC_IPI`] `0x30`).
//! **Không** scheduler/preemption, **không** `-cpu max`, **không** `syscall`/`sysret`, **không**
//! tuyên bố K5' đạt.

/// Số hart tối đa `[TK]` — **8**, đúng D2-195 Q5 (giữ nguyên từ K3).
///
/// Crate arch **không** phụ thuộc `vifast-plat-common` (tránh vòng dev-dep), nên hằng được khai
/// báo lại ở đây; ảnh kiểm bằng `const _: () = assert!(smp::MAX_HARTS ==
/// vifast_plat_common::smp::MAX_HARTS);` — lệch là **lỗi biên dịch**, không phải lỗi lúc chạy.
pub const MAX_HARTS: usize = 8;

/// Trang **guard** của một hart (4 KiB) `[TK]` — nằm ngay **dưới** đáy stack của chính hart đó.
pub const HART_GUARD_BYTES: u64 = 0x1000;

/// Stack của một hart (64 KiB) `[TK]` — NOLOAD, `sp` bắt đầu ở **đỉnh** và mọc xuống guard.
pub const HART_STACK_BYTES: u64 = 0x1_0000;

/// Bước giữa hai record hart: `guard + stack` = `0x1_1000` (68 KiB).
pub const HART_STRIDE: u64 = HART_GUARD_BYTES + HART_STACK_BYTES;

/// Tổng vùng NOLOAD theo hart: `MAX_HARTS × HART_STRIDE` = `0x8_8000` (544 KiB).
pub const HART_AREA_BYTES: u64 = HART_STRIDE * MAX_HARTS as u64;

/// Số hart ảnh KT-278 **thật sự đánh thức** — hart 0 (BSP) + hart 1..3 (AP), runner `-smp 4`.
///
/// KT-276 đánh thức **một** hart phụ (2 hart); KT-278 mở lên **bốn** — arena stack đã đủ 8 slot
/// từ KT-276 nên không đổi bố cục, chỉ đổi số hart ảnh đánh thức.
pub const SMP_HARTS: usize = 4;

/// Mặt nạ hart **kỳ vọng** online (`0b1111` với [`SMP_HARTS`] = 4).
pub const SMP_EXPECT_MASK: u32 = (1u32 << SMP_HARTS) - 1;

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
    /// Hai record **đè nhau** — đây là đột biến (iii) của KT-276.
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

    /// Hart có **đỉnh stack** đúng bằng `top` — dùng cho `sp` **ban đầu** (giá trị `__stack_top`
    /// truyền vào `kmain`), vì `slot_of` loại trừ đúng biên trên.
    pub fn slot_of_top(&self, top: u64) -> Option<usize> {
        let mut hart = 0;
        while hart < MAX_HARTS {
            if self.stack_top(hart) == Some(top) {
                return Some(hart);
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
// ICR — Interrupt Command Register (SDM Vol 3A §10.6.1, Hình 10-27)
// =====================================================================================

/// `ICR` thanh ghi **thấp** (offset trong trang MMIO LAPIC).
pub const LAPIC_ICR_LOW: u32 = 0x300;
/// `ICR` thanh ghi **cao** — chứa destination field ở bit 24..31 (`ICR[63:56]`).
pub const LAPIC_ICR_HIGH: u32 = 0x310;

/// Mặt nạ vector trong `ICR` thấp (bit 0..7).
pub const ICR_VECTOR_MASK: u32 = 0xFF;
/// Vị trí delivery mode (bit 8..10).
pub const ICR_DELIVERY_MODE_SHIFT: u32 = 8;
/// Mặt nạ delivery mode (3 bit).
pub const ICR_DELIVERY_MODE_MASK: u32 = 0b111 << ICR_DELIVERY_MODE_SHIFT;
/// Delivery mode `Fixed` (`000`).
pub const ICR_DM_FIXED: u32 = 0b000;
/// Delivery mode `INIT` (`101`) — Bảng 10-1.
pub const ICR_DM_INIT: u32 = 0b101;
/// Delivery mode `Start-Up` / SIPI (`110`) — Bảng 10-1.
pub const ICR_DM_STARTUP: u32 = 0b110;

/// `ICR.Delivery Status` (bit 12) — `1` ⇔ lệnh trước **chưa gửi xong**; phải chờ `0` trước khi
/// ghi lệnh mới (SDM §10.6.1: "software should not write to the ICR until the delivery status
/// bit is clear").
pub const ICR_DELIVERY_PENDING: u32 = 1 << 12;
/// `ICR.Level` (bit 14) — `1` = assert. `INIT` phải assert.
pub const ICR_LEVEL_ASSERT: u32 = 1 << 14;
/// `ICR.Trigger Mode` (bit 15) — `1` = level. `INIT` phải level; `SIPI` phải edge (`0`).
pub const ICR_LEVEL_TRIGGER: u32 = 1 << 15;
/// Vị trí destination field trong thanh ghi **cao** (bit 24..31).
pub const ICR_DEST_SHIFT: u32 = 24;

/// Dựng `ICR` thấp từ delivery mode + vector + hai bit mức/kiểu kích.
pub const fn icr_low(
    delivery_mode: u32,
    vector: u8,
    level_assert: bool,
    level_trigger: bool,
) -> u32 {
    ((delivery_mode & 0b111) << ICR_DELIVERY_MODE_SHIFT)
        | ((vector as u32) & ICR_VECTOR_MASK)
        | if level_assert { ICR_LEVEL_ASSERT } else { 0 }
        | if level_trigger { ICR_LEVEL_TRIGGER } else { 0 }
}

/// `ICR` thấp cho **INIT**: delivery mode `101`, level assert + level trigger, vector bỏ qua.
pub const fn icr_init_low() -> u32 {
    icr_low(ICR_DM_INIT, 0, true, true)
}

/// `ICR` thấp cho **SIPI**: delivery mode `110`, vector = `addr >> 12`, edge trigger.
pub const fn icr_sipi_low(vector: u8) -> u32 {
    icr_low(ICR_DM_STARTUP, vector, false, false)
}

/// `ICR` thấp cho **IPI** (KT-278): delivery mode `000` (**fixed**), `Level = 1` (assert),
/// `Trigger = 0` (edge), vector = `vector`.
///
/// SDM Vol 3A §10.6.1 + Hình 10-27: với fixed delivery, `Level`/`Trigger` chỉ có nghĩa khi
/// `Trigger = 1` (level); nhưng **mọi** IPI fixed-delivery của hệ điều hành (kể cả Linux:
/// `APIC_INT_ASSERT | APIC_DM_FIXED`) đặt `Level = 1` — đó là giá trị **đo được** trên QEMU và
/// là giá trị test host chốt (`icr_fixed_low(0x30) == 0x4030`). `Trigger = 0` (edge) là điều kiện
/// để một IPI **không** bị lặp lại khi `EOI`.
pub const fn icr_fixed_low(vector: u8) -> u32 {
    icr_low(ICR_DM_FIXED, vector, true, false)
}

/// `ICR` cao: destination APIC ID ở bit 24..31.
pub const fn icr_high(dest_apic_id: u8) -> u32 {
    ((dest_apic_id as u32) & 0xFF) << ICR_DEST_SHIFT
}

/// `true` ⇔ `ICR` thấp còn **đang gửi** (bit 12) — caller phải chờ trước khi ghi lệnh mới.
pub const fn icr_delivery_pending(low: u32) -> bool {
    low & ICR_DELIVERY_PENDING != 0
}

// =====================================================================================
// SIPI vector
// =====================================================================================

/// Trần địa chỉ trampoline: `1 MiB` — `SIPI` chỉ mang 8 bit vector nên địa chỉ phải
/// `< 1 MiB` (SDM §10.12.10).
pub const SIPI_MAX_ADDR: u64 = 0x0010_0000;

/// Vector `SIPI` cho địa chỉ `addr`: `addr >> 12` nếu `addr` căn 4 KiB và `< 1 MiB`; `None` nếu
/// không (fail-closed — vector trỏ vào giữa trang là lỗi im lặng).
pub const fn sipi_vector(addr: u64) -> Option<u8> {
    if addr >= SIPI_MAX_ADDR || addr & 0xFFF != 0 {
        return None;
    }
    Some((addr >> 12) as u8)
}

// =====================================================================================
// Topology APIC ID
// =====================================================================================

/// `APIC ID` kỳ vọng của `hart` theo topology **phẳng** của QEMU (`-smp N` ⇒ `APIC ID = chỉ số
/// CPU`); `None` nếu hart ngoài dải.
///
/// Đây **không** phải bảo đảm của đặc tả: ACPI MADT mới là nguồn sự thật. Ảnh vì thế **kiểm lại**
/// trên hart phụ (`LAPIC_ID` đọc từ MMIO phải bằng giá trị đã ghi vào boot block) và PANIC nếu
/// lệch — không tin con số này mù.
pub const fn apic_id_for_hart(hart: usize) -> Option<u8> {
    if valid_hart(hart) {
        Some(hart as u8)
    } else {
        None
    }
}

// =====================================================================================
// Trang trampoline 0x8000 — bố cục là **hợp đồng** với asm
// =====================================================================================

/// Địa chỉ **vật lý** của trang trampoline: `0x8000` ⇒ `SIPI` vector `0x08`.
pub const TRAMPOLINE_BASE: u64 = 0x8000;
/// Cỡ trang trampoline: **một** trang 4 KiB (đủ code 16-bit + GDT tạm + boot block).
pub const TRAMPOLINE_BYTES: u64 = 0x1000;

/// Offset của **GDT tạm** trong trang trampoline (3 entry × 8 byte = 24 byte).
pub const TRAMPOLINE_GDT_OFFSET: u64 = 0x800;
/// Offset của pseudo-descriptor `GDTR` **6 byte** (real mode: limit 16-bit + base 32-bit).
pub const TRAMPOLINE_GDTR_OFFSET: u64 = 0x880;
/// Offset của **boot block** — BSP ghi, trampoline đọc.
pub const TRAMPOLINE_BOOT_OFFSET: u64 = 0x900;

/// `PML4` (PA) mà hart phụ nạp vào `CR3`.
pub const BOOT_PML4: u64 = 0x900;
/// `sp` ban đầu của hart phụ (đỉnh slot của nó trong arena).
pub const BOOT_STACK_TOP: u64 = 0x908;
/// Hart id của hart phụ (BSP ghi trước `SIPI`).
pub const BOOT_HART: u64 = 0x910;
/// Địa chỉ `kmain_secondary` (PA, identity map) — trampoline `call` tuyệt đối.
pub const BOOT_ENTRY: u64 = 0x918;
/// `IDTR.limit` — **ngay trước** [`BOOT_IDT_BASE`] để `lidt [mem]` đọc 10 byte liền.
pub const BOOT_IDT_LIMIT: u64 = 0x920;
/// `IDTR.base` — 8 byte ngay sau limit (offset 0x922, **không** căn 8: đúng layout `lidt`).
pub const BOOT_IDT_BASE: u64 = 0x922;
/// `CR0` mà hart phụ ghi (giá trị BSP đã dùng: `PG|PE|ET`).
pub const BOOT_CR0: u64 = 0x930;
/// `CR4` mà hart phụ ghi (`PAE`).
pub const BOOT_CR4: u64 = 0x938;
/// `IA32_EFER` mà hart phụ ghi (`LME|NXE`).
pub const BOOT_EFER: u64 = 0x940;
/// Địa chỉ MMIO LAPIC mà hart phụ dùng (BSP đọc từ `IA32_APIC_BASE`).
pub const BOOT_LAPIC_BASE: u64 = 0x948;
/// `LAPIC_ID` **kỳ vọng** của hart phụ — hart phụ so với `LAPIC_ID` nó tự đọc.
pub const BOOT_LAPIC_ID: u64 = 0x950;
/// Magic chứng minh **copy** trampoline đã chạy: `"SMP2"` little-endian.
pub const TRAMPOLINE_MAGIC: u64 = 0x534D_5032;
/// Offset của magic trong boot block.
pub const BOOT_MAGIC: u64 = 0x958;

/// Địa chỉ **tuyệt đối** của một offset trong trang trampoline.
pub const fn trampoline_addr(offset: u64) -> u64 {
    TRAMPOLINE_BASE + offset
}

// =====================================================================================
// Thời gian + timer của hart phụ
// =====================================================================================

/// Chờ **10 ms** sau `INIT` trước `SIPI` đầu (SDM §10.12.9).
pub const INIT_WAIT_US: u64 = 10_000;
/// Chờ **200 µs** giữa hai `SIPI` (SDM §10.12.10).
pub const SIPI_WAIT_US: u64 = 200;
/// Trần chờ hart phụ **online** — ngân sách của ảnh (2 s), đo bằng PIT kênh 2.
pub const ONLINE_TIMEOUT_US: u64 = 2_000_000;

/// Trần chờ **IPI** tới (KT-278) — **100 ms**, đo bằng PIT kênh 2.
///
/// Hữu hạn là điều kiện của đột biến (ii): gửi IPI tới một `APIC ID` **không tồn tại** thì không
/// hart nào nhận, và BSP phải **hết trần rồi đi tiếp** (TAP 20 ĐỎ) — **không treo**. 100 ms là
/// dư ~10× so với độ trễ đo được của một IPI trên QEMU TCG (hàng chục µs).
pub const IPI_TIMEOUT_US: u64 = 100_000;

/// Trần **số vòng** của một lần chờ SMP (KT-301) — **điều kiện dừng chính**, thay trần **thời
/// gian tuyệt đối**.
///
/// # Vì sao đổi **đại lượng** chờ, không phải nới ngưỡng
///
/// KT-298 soak 600 bắt boot 422: `# smp: ipi sent=1 count=0 waited_us=18446744073709551615 ok=0`
/// **trước** khi `# smp: ipi recv=1` in ra, dưới `qemu_foreign=3 loadavg_1m=6.95`. Trần thời gian
/// đo bằng PIT là **thời gian tường**: host deschedule vCPU hart phụ quá 100 ms thì IPI **không
/// thể** tới trong hạn — phép chờ sai **đại lượng**, không phải ngưỡng quá chặt. Vì vậy điều kiện
/// dừng là **biến đếm** (hart đích tăng trong handler), với trần **số vòng** rất lớn; thời gian
/// (`IPI_TIMEOUT_US` 100 ms, `ONLINE_TIMEOUT_US` 2 s — **giữ nguyên**) chỉ còn là **chốt an toàn**
/// hỏi **sau** khi trần vòng cạn.
///
/// # Số đo chọn trần (QEMU 11.1.3 TCG, máy 12 core)
///
/// Vòng `load` + `pause` mỗi 64 vòng chạy **~39 triệu vòng/s** (đo 2 lần: 2.000.000 vòng ≈ 51 ms;
/// `pause` mỗi vòng đắt ~0,5 µs nên **không** dùng mỗi vòng) ⇒ 100.000.000 vòng ≈ **2,5 s**, dư
/// ~25× so với trần cũ 100 ms. Đây là **trần số vòng**, không phải ngưỡng thời gian: máy chậm thì
/// chờ **lâu hơn** (đúng ý), máy nhanh thì chốt thời gian bên dưới mới có tác dụng.
pub const SMP_SPIN_LIMIT: u64 = 100_000_000;

/// Số chia LAPIC timer của hart phụ: **1** (mã `0b1011`).
pub const AP_TIMER_DIVIDE: u32 = 1;
/// `initial_count` của LAPIC timer hart phụ — 100 µs danh định (mô hình QEMU 1 count/ns).
///
/// Đo được trên QEMU TCG: ngắt LAPIC timer tới không đều (2,8–38,5 ms cho chu kỳ danh định
/// 100 µs), nên đây chỉ là **nhịp đánh thức** để hart phụ kiểm lệnh rồi `hlt` lại — **không**
/// phải nguồn tick (nguồn tick của BSP vẫn là PIT, KT-266).
pub const AP_TIMER_INITIAL_COUNT: u32 = 100_000;

/// `true` ⇔ `opt == Some(want)` — so sánh `Option<u64>` trong **const** (const trait `PartialEq`
/// chưa ổn định, nên bất biến bố cục dùng helper này thay vì `==`).
const fn opt_is(opt: Option<u64>, want: u64) -> bool {
    match opt {
        Some(value) => value == want,
        None => false,
    }
}

// Bất biến bố cục: sai một trong số này thì địa chỉ stack/guard tính từ linker sẽ lệch, và guard
// của hart này có thể rơi vào stack của hart khác (rủi ro "stack AP chồng" của K5').
const _: () = {
    assert!(MAX_HARTS == 8);
    assert!(HART_GUARD_BYTES == 0x1000);
    assert!(HART_STACK_BYTES == 0x1_0000);
    assert!(HART_STRIDE == 0x1_1000);
    assert!(HART_STRIDE.is_multiple_of(0x1000)); // mọi record bắt đầu ở biên trang
    assert!(HART_AREA_BYTES == 0x8_8000);
    assert!(opt_is(stack_bottom_offset(0), HART_GUARD_BYTES));
    assert!(opt_is(stack_top_offset(0), HART_STRIDE));
    assert!(opt_is(stack_top_offset(MAX_HARTS - 1), HART_AREA_BYTES));
    assert!(opt_is(guard_offset(1), HART_STRIDE));
    assert!(SMP_HARTS <= MAX_HARTS);
    assert!(SMP_EXPECT_MASK == 0b1111);
    assert!(SMP_EXPECT_MASK.count_ones() as usize == SMP_HARTS);
    // Trampoline: GDT + GDTR + boot block không đè nhau và nằm trong một trang.
    assert!(TRAMPOLINE_GDT_OFFSET + 24 <= TRAMPOLINE_GDTR_OFFSET);
    assert!(TRAMPOLINE_GDTR_OFFSET + 6 <= TRAMPOLINE_BOOT_OFFSET);
    assert!(BOOT_MAGIC + 8 <= TRAMPOLINE_BYTES);
    assert!(TRAMPOLINE_BASE + TRAMPOLINE_BYTES <= SIPI_MAX_ADDR);
    assert!(TRAMPOLINE_BASE.is_multiple_of(0x1000));
    // ICR: INIT phải level-asserted, SIPI phải edge, IPI fixed phải level-asserted + edge.
    assert!(icr_init_low() == 0xC500);
    assert!(icr_sipi_low(0x08) == 0x608);
    assert!(icr_high(1) == 0x0100_0000);
    assert!(icr_fixed_low(crate::exc::VEC_IPI) == 0x4030);
    assert!(icr_fixed_low(crate::exc::VEC_IPI) & ICR_LEVEL_TRIGGER == 0);
    assert!(icr_fixed_low(crate::exc::VEC_IPI) & ICR_LEVEL_ASSERT != 0);
    assert!(IPI_TIMEOUT_US > 0 && IPI_TIMEOUT_US < ONLINE_TIMEOUT_US);
    assert!(SMP_SPIN_LIMIT > 0);
};
