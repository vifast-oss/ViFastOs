// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Cache maintenance trước/sau DMA** trên AArch64 — KT-277 (K4' bước 3).
//!
//! # Vì sao cần
//!
//! Vùng DMA của driver (`virtio_blk`, xem [`crate::virtio_blk`]) nằm trong `.bss` và được map
//! **Normal, cacheable** (đúng: RAM thường). Thiết bị DMA **không** đi qua cache của CPU, nên:
//!
//! - **trước** khi bàn giao ring/buffer cho thiết bị: mọi dòng cache **bẩn** phải được ghi ra
//!   PoC (`dc civac` = clean **và** invalidate theo VA), nếu không thiết bị đọc bản cũ trong RAM;
//! - **sau** khi thiết bị ghi xong: mọi dòng cache của CPU phải bị bỏ (`dc ivac` = invalidate
//!   theo VA), nếu không CPU đọc bản cũ trong cache thay vì dữ liệu thiết bị vừa ghi.
//!
//! # Encoding — đối chiếu bằng **hợp dịch thật**, không chép từ trí nhớ
//!
//! `DC` là system instruction: `SYS #<op1>, C7, C<crm>, #<op2>, Xt`. **`op1` khác nhau giữa hai
//! nhóm** — đây là chỗ dễ sai nhất và đã đo được:
//!
//! ```text
//!   dc civac, x0   d50b7e20   (op1=3, CRm=14, op2=1)
//!   dc cvac,  x0   d50b7a20   (op1=3, CRm=10, op2=1)
//!   dc cvau,  x0   d50b7b20   (op1=3, CRm=11, op2=1)
//!   dc ivac,  x0   d5087620   (op1=0, CRm=6,  op2=1)   ← op1 = 0, KHÔNG phải 3
//!   ic ivau,  x0   d50b7520   (op1=3, CRm=5,  op2=1)
//! ```
//!
//! Nguồn: `clang --target=aarch64-none-elf -c` + `llvm-objdump -d` (cùng cách `tlb.rs`/
//! `barrier.rs` đã làm). Test host `tests/cache.rs` so **giá trị tuyệt đối** của từng mã, nên
//! một opcode sai làm test **ĐỎ** thay vì chỉ làm hành vi sai trên máy thật.
//!
//! # Cỡ dòng cache lấy từ `CTR_EL0`, không hard-code
//!
//! `CTR_EL0.DminLine[19:16]` cho biết cỡ dòng dữ liệu nhỏ nhất là `4 << DminLine` byte. QEMU
//! `cortex-a72` trả `DminLine = 4` ⇒ **64 B**, nhưng hard-code 64 là sai trên CPU khác (Apple
//! M-series dùng 128 B). [`cache_line_bytes`] là hàm **thuần** trên giá trị `CTR_EL0`, test được
//! trên host; [`current_line_bytes`] đọc `CTR_EL0` thật.
//!
//! # Thứ tự bắt buộc là **dữ liệu**
//!
//! Như `MMU_ENABLE_SEQUENCE` của [`crate::boot_mmu`]: thứ tự "làm sạch → barrier → bàn giao" và
//! "bỏ cache → barrier → đọc" nằm ở [`HANDOFF_SEQUENCE`]/[`COMPLETE_SEQUENCE`] để test host
//! khẳng định **thứ tự tuyệt đối**. Đột biến "bỏ bước làm sạch" vì thế làm test **ĐỎ** ở tầng
//! dữ liệu, không chỉ là một dòng code bị xoá mà không ai thấy.
//!
//! # Phạm vi
//!
//! Chỉ **cache maintenance** + barrier. Không `unsafe` (asm nằm ở [`crate::asm`], D2-17), không
//! `cfg(target_arch)`, không đụng bảng trang.

use crate::asm;

/// Base encoding `SYS #0, C7, C<crm>, #<op2>, Xt` — nhóm chứa `dc ivac`/`ic ivau`.
pub const DC_SYS_BASE_OP1_0: u32 = 0xD508_7000;
/// Base encoding `SYS #3, C7, C<crm>, #<op2>, Xt` — nhóm chứa `dc civac`/`dc cvac`/`dc cvau`.
pub const DC_SYS_BASE_OP1_3: u32 = 0xD50B_7000;

/// Ghép encoding `SYS #<op1>, C7, C<crm>, #<op2>, Xt`.
///
/// `op1` chỉ nhận **0** hoặc **3** (hai nhóm `DC`); giá trị khác rơi vào nhóm `op1 = 3` và test
/// host chốt rằng hai giá trị hợp lệ cho đúng mã tuyệt đối.
pub const fn dc_encoding(op1: u32, crm: u32, op2: u32, rt: u32) -> u32 {
    let base = if op1 == 0 {
        DC_SYS_BASE_OP1_0
    } else {
        DC_SYS_BASE_OP1_3
    };
    base | ((crm & 0xF) << 8) | ((op2 & 0x7) << 5) | (rt & 0x1F)
}

/// `DC CIVAC` — clean **và** invalidate dòng dữ liệu theo VA tới PoC (tham số ở `x0`).
///
/// Đây là lệnh **bàn giao** DMA: ghi bản bẩn ra PoC rồi bỏ bản sao trong cache.
pub const DC_CIVAC: u32 = dc_encoding(3, 14, 1, 0);
/// `DC CVAC` — clean dòng dữ liệu theo VA tới PoC (không invalidate).
pub const DC_CVAC: u32 = dc_encoding(3, 10, 1, 0);
/// `DC CVAU` — clean dòng dữ liệu theo VA tới PoU.
pub const DC_CVAU: u32 = dc_encoding(3, 11, 1, 0);
/// `DC IVAC` — invalidate dòng dữ liệu theo VA tới PoC (tham số ở `x0`).
///
/// Đây là lệnh **nhận** DMA: bỏ bản sao trong cache để lần đọc sau lấy dữ liệu thiết bị vừa ghi.
pub const DC_IVAC: u32 = dc_encoding(0, 6, 1, 0);
/// `IC IVAU` — invalidate dòng **instruction** theo VA tới PoU (chưa dùng ở bước 3).
pub const IC_IVAU: u32 = dc_encoding(3, 5, 1, 0);

/// Vị trí `CTR_EL0.DminLine[19:16]`.
pub const CTR_DMINLINE_SHIFT: u32 = 16;
/// Mặt nạ `CTR_EL0.DminLine` (4 bit).
pub const CTR_DMINLINE_MASK: u64 = 0xF;

/// Cỡ dòng cache **dự phòng** khi `CTR_EL0` không đọc được giá trị hợp lý.
///
/// 64 B là giá trị của `cortex-a72` (QEMU `virt`) và của phần lớn CPU ARMv8 server; dùng nó
/// **chỉ** khi `DminLine` cho ra giá trị vô lý (0 hoặc > 4 KiB) — không dùng để thay `CTR_EL0`.
pub const DEFAULT_CACHE_LINE: u64 = 64;

/// Cỡ dòng cache dữ liệu nhỏ nhất suy từ `CTR_EL0`: `4 << DminLine`.
///
/// Hàm **thuần** (test host được). Giá trị vô lý (`< 16` hoặc `> 4096`) ⇒ [`DEFAULT_CACHE_LINE`]
/// thay vì một vòng lặp `dc` sai bước.
pub const fn cache_line_bytes(ctr: u64) -> u64 {
    let dmin = (ctr >> CTR_DMINLINE_SHIFT) & CTR_DMINLINE_MASK;
    let line = 4u64 << dmin;
    if line < 16 || line > 4096 {
        DEFAULT_CACHE_LINE
    } else {
        line
    }
}

/// Số dòng cache mà `[addr, addr + bytes)` **chạm tới** (làm tròn **ra ngoài** hai đầu).
///
/// `bytes == 0` hoặc `line == 0` ⇒ `0`. Đây là con số mà `# dma: … lines=` in ra, nên nó phải
/// khớp **chính xác** với số lần `dc` mà [`for_each_line`] phát ra.
pub const fn line_count(addr: u64, bytes: u64, line: u64) -> u64 {
    if bytes == 0 || line == 0 {
        return 0;
    }
    let first = addr & !(line - 1);
    let last = addr.saturating_add(bytes).saturating_add(line - 1) & !(line - 1);
    (last - first) / line
}

/// Gọi `f(địa_chỉ_dòng)` cho **từng** dòng cache của `[addr, addr + bytes)`; trả **số dòng**.
///
/// Đây là **vòng lặp duy nhất** phát `dc` — nhờ vậy con số đếm được và số lệnh thật không thể
/// lệch nhau. Test host gọi hàm này với closure ghi lại địa chỉ để khẳng định **số lệnh** và
/// **địa chỉ từng dòng** (yêu cầu "chứng minh bằng test host đếm số lệnh/địa chỉ" của KT-277).
pub fn for_each_line<F: FnMut(u64)>(addr: u64, bytes: u64, line: u64, mut f: F) -> u64 {
    if bytes == 0 || line == 0 {
        return 0;
    }
    let first = addr & !(line - 1);
    let end = addr.saturating_add(bytes);
    let mut at = first;
    let mut count = 0;
    while at < end {
        f(at);
        at += line;
        count += 1;
    }
    count
}

/// Cỡ dòng cache **thật** của CPU đang chạy (`CTR_EL0`).
pub fn current_line_bytes() -> u64 {
    cache_line_bytes(asm::read_ctr_el0())
}

/// `dc civac` từng dòng của `[addr, addr + bytes)` — **không** barrier (barrier là bước riêng
/// của [`HANDOFF_SEQUENCE`]). Trả số dòng đã làm sạch.
pub fn clean_invalidate_lines(addr: u64, bytes: u64) -> u64 {
    let line = current_line_bytes();
    for_each_line(addr, bytes, line, asm::dc_civac)
}

/// `dc ivac` từng dòng của `[addr, addr + bytes)` — **không** barrier (xem
/// [`COMPLETE_SEQUENCE`]). Trả số dòng đã bỏ.
pub fn invalidate_lines(addr: u64, bytes: u64) -> u64 {
    let line = current_line_bytes();
    for_each_line(addr, bytes, line, asm::dc_ivac)
}

/// `dc ivac` **một** dòng chứa `addr` — dùng trong vòng poll `used->idx` (xem
/// [`crate::virtio_blk`]).
///
/// # Vì sao an toàn khi dòng ấy còn dữ liệu của driver
///
/// `used->idx` nằm cùng dòng với cuối `avail->ring` (bố cục split virtqueue). Trước khi vào vòng
/// poll, [`HANDOFF_SEQUENCE`] đã `dc civac` **toàn** vùng DMA ⇒ mọi dòng của vùng đều **sạch**;
/// `dc ivac` vì thế chỉ bỏ bản sao đã sạch, không mất dữ liệu chưa ghi ra.
pub fn invalidate_line(addr: u64) -> u64 {
    let line = current_line_bytes();
    asm::dc_ivac(addr & !(line - 1));
    1
}

/// Một bước **trước** khi bàn giao vùng DMA cho thiết bị.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffStep {
    /// `dc civac` từng dòng của vùng DMA (clean + invalidate).
    CleanInvalidate,
    /// `dsb ish` — chờ cache maintenance hoàn tất trong inner-shareable domain.
    DsbIsh,
    /// `dsb sy` — barrier **đầy đủ** trước MMIO notify (thiết bị nằm ngoài domain shareability).
    DsbSy,
    /// Ghi cửa thông báo (MMIO) — bàn giao ring cho thiết bị.
    Notify,
}

impl HandoffStep {
    /// Tên ngắn để in log (D2-102: in từ dữ liệu).
    pub const fn name(self) -> &'static str {
        match self {
            Self::CleanInvalidate => "clean",
            Self::DsbIsh => "dsb-ish",
            Self::DsbSy => "dsb-sy",
            Self::Notify => "notify",
        }
    }
}

/// Thứ tự **bắt buộc** trước khi bàn giao vùng DMA: làm sạch → barrier → barrier đầy đủ → notify.
///
/// Thiếu bước 1 ⇒ thiết bị có thể đọc bản cũ trong RAM; thiếu barrier ⇒ thiết bị thấy `avail->idx`
/// trước khi ring xong (spec virtio §2.7.13).
pub const HANDOFF_SEQUENCE: [HandoffStep; 4] = [
    HandoffStep::CleanInvalidate,
    HandoffStep::DsbIsh,
    HandoffStep::DsbSy,
    HandoffStep::Notify,
];

/// Một bước **sau** khi thiết bị báo hoàn tất, **trước** khi đọc dữ liệu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompleteStep {
    /// `dc ivac` từng dòng của vùng DMA (bỏ bản sao trong cache).
    Invalidate,
    /// `dsb ish` — chờ invalidate hoàn tất trước khi đọc.
    DsbIsh,
}

impl CompleteStep {
    /// Tên ngắn để in log.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Invalidate => "invalidate",
            Self::DsbIsh => "dsb-ish",
        }
    }
}

/// Thứ tự **bắt buộc** sau khi thiết bị ghi xong: bỏ cache → barrier → (caller đọc).
pub const COMPLETE_SEQUENCE: [CompleteStep; 2] = [CompleteStep::Invalidate, CompleteStep::DsbIsh];

/// Thi hành **một** bước bàn giao; trả số dòng đã làm sạch (0 với bước không phải `dc`).
///
/// `notify_addr` chỉ dùng ở [`HandoffStep::Notify`] (ghi **index queue 0** vào cửa thông báo).
pub fn apply_handoff(step: HandoffStep, addr: u64, bytes: u64, notify_addr: u64) -> u64 {
    match step {
        HandoffStep::CleanInvalidate => clean_invalidate_lines(addr, bytes),
        HandoffStep::DsbIsh => {
            asm::dsb_ish();
            0
        }
        HandoffStep::DsbSy => {
            asm::dsb_sy();
            0
        }
        HandoffStep::Notify => {
            asm::mmio_write_u16(notify_addr, 0);
            0
        }
    }
}

/// Thi hành **một** bước hoàn tất; trả số dòng đã bỏ (0 với barrier).
pub fn apply_complete(step: CompleteStep, addr: u64, bytes: u64) -> u64 {
    match step {
        CompleteStep::Invalidate => invalidate_lines(addr, bytes),
        CompleteStep::DsbIsh => {
            asm::dsb_ish();
            0
        }
    }
}
