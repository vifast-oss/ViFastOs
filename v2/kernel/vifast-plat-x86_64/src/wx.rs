// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340a** — **W^X cho ảnh kernel x86_64**: `.text` = **R+X** (không W), `.rodata` = **R**
//! (không W, NX), `.data`/`.bss`/stack = **RW NX**, bật **sau** khi việc dựng ảnh đã xong.
//!
//! # Cơ chế (khuôn của riscv64 `mmu.rs`/`init_pages.rs` — **không** nghĩ cơ chế mới)
//!
//! Bảng trang boot của `_start` map identity `0..1 GiB` bằng **lá 2 MiB** `P|RW|PS` — cả ảnh
//! kernel nằm trong **một** lá ⇒ không thể tách quyền. W^X thay **lá lớn `PD[0]`** (0..2 MiB)
//! bằng **một bảng PT 4 KiB** (`__pt_kernel`) với quyền **theo từng đoạn của ảnh**, rồi:
//!
//! 1. `PD[0] = __pt_kernel | P|RW` (`US = 0` — identity map vẫn supervisor);
//! 2. **xả TLB** (`mov cr3, cr3`);
//! 3. bật **`CR0.WP`** — nếu thiếu, ring 0 vẫn ghi được trang chỉ-đọc và "W^X" chỉ là nhãn.
//!
//! Quyền của lá (bit theo SDM Vol 3A §4.5): `P` (0), `RW` (1), `US` (2), `NX` (63). Lá 4 KiB
//! `PS = 0`; `NX` chỉ có hiệu lực vì `_start` đã bật `EFER.NXE` (KT-270).
//!
//! # Đo được, không phải lời hứa
//!
//! [`enable`] trả [`WxReport`] đọc lại **từ bảng trang** (`walk_leaf`) — TAP dùng đúng giá trị
//! này, không dùng hằng mong đợi. Ca W^X thật (ghi vào `.text` phải `#PF`, ghi `.data` phải
//! thành công) nằm ở ảnh `ktest` (`wx_probe.rs`); ảnh sản phẩm chỉ bật + đi tiếp.
//!
//! # Ngoại lệ **có ghi rõ**: trang trampoline AP
//!
//! `smp.rs` copy trang trampoline xuống `0x8000` rồi hart phụ **chạy** tại đó (real mode →
//! long mode) và BSP **ghi** boot block trong cùng trang (`0x8900`). Trang ấy vì vậy **phải**
//! `RWX` — chỉ trong ảnh **ktest** (ảnh sản phẩm không có SMP). Đây là ngoại lệ duy nhất; mọi
//! trang khác dưới 1 MiB là `RW NX` (RAM thấp/BIOS — không phải mã kernel).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::asm;

/// Lá **có mặt**.
pub const PTE_P: u64 = 1 << 0;
/// Lá **ghi được**.
pub const PTE_RW: u64 = 1 << 1;
/// Lá **user-accessible**.
pub const PTE_US: u64 = 1 << 2;
/// Lá **không thực thi** (cần `EFER.NXE`).
pub const PTE_NX: u64 = 1u64 << 63;
/// Cờ lá lớn 2 MiB trong bảng PD (`PS`).
pub const PDE_PS: u64 = 1 << 7;
/// `CR0.WP` — ring 0 **không** ghi được trang chỉ-đọc.
pub const CR0_WP: u64 = 1 << 16;

/// Một trang 4 KiB.
pub const PAGE_BYTES: u64 = 4096;
/// Số ô của một bảng trang.
pub const PT_ENTRIES: usize = 512;
/// Vùng identity mà W^X tách nhỏ: `0..2 MiB` (lá `PD[0]` của bảng boot).
pub const WX_BYTES: u64 = 2 << 20;

/// **Đoạn của ảnh** mà W^X cần — lấy từ symbol linker của ảnh (kbin), không hardcode.
#[derive(Clone, Copy, Debug)]
pub struct ImageLayout {
    /// Bắt đầu `.text` (RX).
    pub text_start: u64,
    /// Kết thúc `.text` (hết RX).
    pub text_end: u64,
    /// Bắt đầu `.data` — `[text_end, data_start)` là `.rodata`+note (R, NX).
    pub data_start: u64,
    /// `__kernel_end` — `[data_start, kernel_end)` là `.data/.user/.bss/.smp_stacks` (RW NX).
    pub kernel_end: u64,
}

/// Tuỳ chọn — **chỉ** để đột biến chứng minh TAP có răng (ảnh thật luôn dùng [`Default`]).
#[derive(Clone, Copy, Debug)]
pub struct WxOptions {
    /// `false` = **không** bật (đột biến `no-wx`): `.text` thành RWX, `CR0.WP` giữ 0.
    pub protect: bool,
    /// `false` = **không** đặt NX cho `.data`/`.bss` (đột biến `wx-data-exec`).
    pub nx_data: bool,
    /// `true` = trang `0x8000` là RWX (trampoline AP của ảnh ktest).
    pub ap_trampoline: bool,
}

impl Default for WxOptions {
    fn default() -> Self {
        Self {
            protect: true,
            nx_data: true,
            ap_trampoline: false,
        }
    }
}

/// Kết quả **đọc lại từ bảng trang** sau khi bật — nguồn số liệu của TAP W^X.
#[derive(Clone, Copy, Debug, Default)]
pub struct WxReport {
    /// `CR0.WP` sau khi bật.
    pub wp: bool,
    /// Lá 4 KiB tại `text_start` (phải `P|X`, **không** `W`).
    pub text: u64,
    /// Lá 4 KiB tại `text_end` (đầu `.rodata`: `P|NX`, **không** `W`).
    pub rodata: u64,
    /// Lá 4 KiB tại `data_start` (phải `P|RW|NX`).
    pub data: u64,
    /// Lá 4 KiB tại `0x8000` (trampoline AP; `RWX` khi ảnh ktest, `RW NX` khi sản phẩm).
    pub low_tramp: u64,
    /// Số trang 4 KiB **có** `W` **và** không `NX` (W+X) trong `0..2 MiB` — phải 0 ngoài
    /// ngoại lệ trampoline. Đếm **từ bảng trang**, không từ layout.
    pub wx_pages: u64,
}

impl WxReport {
    /// `true` ⇔ `.text` là **R+X** (có `P`, không `RW`, không `NX`).
    pub fn text_is_rx(&self) -> bool {
        self.text & PTE_P != 0 && self.text & PTE_RW == 0 && self.text & PTE_NX == 0
    }
    /// `true` ⇔ `.rodata` là **R** (có `P`, không `RW`, có `NX`).
    pub fn rodata_is_ro(&self) -> bool {
        self.rodata & PTE_P != 0 && self.rodata & PTE_RW == 0 && self.rodata & PTE_NX != 0
    }
    /// `true` ⇔ `.data` là **RW NX** (có `P|RW|NX`, không `US`).
    pub fn data_is_rw_nx(&self) -> bool {
        self.data & PTE_P != 0
            && self.data & PTE_RW != 0
            && self.data & PTE_NX != 0
            && self.data & PTE_US == 0
    }
}

/// Quyền **mong đợi** của trang `va` theo [`ImageLayout`] — hàm **thuần**, host-test được.
///
/// Đây là nguồn duy nhất của luật quyền; [`enable`] chỉ ghi nó vào bảng trang.
pub fn leaf_flags(va: u64, layout: &ImageLayout, opts: &WxOptions) -> u64 {
    let mut flags = PTE_P;
    if !opts.protect {
        // Đột biến: map cả ảnh RWX như bảng boot (để TAP bắt được).
        return flags | PTE_RW;
    }
    if va >= layout.text_start && va < layout.text_end {
        // .text: R+X.
        flags
    } else if va >= layout.text_end && va < layout.data_start {
        // .rodata + PVH note: R, không thực thi.
        flags | PTE_NX
    } else if va >= layout.data_start && va < layout.kernel_end {
        // .data/.user/.bss/.smp_stacks: RW, không thực thi.
        flags | PTE_RW | if opts.nx_data { PTE_NX } else { 0 }
    } else if va == 0x8000 && opts.ap_trampoline {
        // Ngoại lệ duy nhất: trang trampoline AP (BSP ghi boot block + AP chạy tại đây).
        flags | PTE_RW
    } else {
        // RAM thấp (BIOS/IVT/stack 16-bit) và phần dư `kernel_end..2 MiB`: dữ liệu, RW NX.
        flags | PTE_RW | PTE_NX
    }
}

/// Địa chỉ vật lý của bảng PT kernel — ảnh đặt trong `.bss` (`__pt_kernel`, 4 KiB).
static PT_PA: AtomicU64 = AtomicU64::new(0);

/// Dựng bảng PT 4 KiB cho `0..2 MiB`, thay `PD[0]`, xả TLB rồi bật `CR0.WP`.
///
/// `pt_pa` = PA của `__pt_kernel` (4 KiB, trong `.bss` — **ghi được** ở thời điểm gọi), `pd_pa`
/// = PA của `__pd` (bảng PD 2 MiB của `_start`). Trả [`WxReport`] **đọc lại** từ bảng trang.
///
/// # An toàn
///
/// Chỉ gọi **một lần**, sau khi `_start` đã bật paging và **sau** khi mọi bản sao mã (trampoline,
/// trang user) đã xong — sau khi bật, ghi vào `.text`/`.rodata` là `#PF`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn enable(pt_pa: u64, pd_pa: u64, layout: &ImageLayout, opts: &WxOptions) -> WxReport {
    // 1. Dựng 512 lá 4 KiB. Bảng PT mới nằm trong `.bss` (RW) nên ghi được.
    let mut va = 0u64;
    let mut i = 0usize;
    while i < PT_ENTRIES {
        let flags = leaf_flags(va, layout, opts);
        // Ghi qua `asm::write_u64` (volatile trong `asm.rs` — tệp duy nhất có `unsafe`, D2-201).
        asm::write_u64(pt_pa + (i as u64) * 8, (va & !0xFFF) | flags);
        va += PAGE_BYTES;
        i += 1;
    }
    PT_PA.store(pt_pa, Ordering::Release);

    // 2. `PD[0]` = PT | P|RW (thay lá lớn 2 MiB). `US = 0`: identity map vẫn supervisor.
    let pd0 = pt_pa | PTE_P | PTE_RW;
    // Ghi `PD[0]` qua `asm::write_u64` (volatile; `asm.rs` giữ `unsafe` duy nhất).
    asm::write_u64(pd_pa, pd0);

    // 3. Xả TLB (mapping 2 MiB cũ đã cache cho 0..2 MiB).
    asm::write_cr3(asm::read_cr3());

    // 4. Bật CR0.WP để ring 0 cũng bị chặn ghi trang chỉ-đọc.
    if opts.protect {
        asm::write_cr0(asm::read_cr0() | CR0_WP);
    }

    report(layout)
}

/// Stub **kiểm kiểu** cho target không phải x86_64 (crate này vẫn được `cargo check` trong
/// workspace kernel khi target là riscv64/aarch64): không bật gì, trả report rỗng. Không có
/// đường nào gọi nó trên ảnh thật (ảnh x86_64 chỉ được build cho `x86_64-unknown-none`).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn enable(_pt_pa: u64, _pd_pa: u64, _layout: &ImageLayout, _opts: &WxOptions) -> WxReport {
    WxReport::default()
}

/// Đọc lại quyền từ bảng trang **đang hoạt động** và đóng gói [`WxReport`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn report(layout: &ImageLayout) -> WxReport {
    let root = asm::read_cr3() & !0xFFF;
    let text = walk_leaf(root, layout.text_start).unwrap_or(0);
    let rodata = walk_leaf(root, layout.text_end).unwrap_or(0);
    let data = walk_leaf(root, layout.data_start).unwrap_or(0);
    let low_tramp = walk_leaf(root, 0x8000).unwrap_or(0);
    // Đếm trang W+X **thật** trong 0..2 MiB từ bảng trang.
    let mut wx_pages = 0u64;
    let mut va = 0u64;
    while va < WX_BYTES {
        if let Some(entry) = walk_leaf(root, va) {
            if entry & PTE_RW != 0 && entry & PTE_NX == 0 {
                wx_pages += 1;
            }
        }
        va += PAGE_BYTES;
    }
    WxReport {
        wp: asm::read_cr0() & CR0_WP != 0,
        text,
        rodata,
        data,
        low_tramp,
        wx_pages,
    }
}

/// Đi bảng trang 4 cấp từ `root_pa`, trả **lá** phủ `va` (4 KiB hoặc 2 MiB `PS`).
///
/// `None` ⇔ `va` **không map**. Chỉ đọc bộ nhớ bảng trang; không đụng cache.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn walk_leaf(root_pa: u64, va: u64) -> Option<u64> {
    let mut table = root_pa & !0xFFF;
    let mut level = 4;
    loop {
        let index = ((va >> (12 + 9 * (level - 1))) & 0x1FF) as usize;
        // Đọc qua `asm::read_u64` (volatile; bảng trang do kernel dựng, 4 KiB căn).
        let entry = asm::read_u64(table + (index as u64) * 8);
        if entry & PTE_P == 0 {
            return None;
        }
        if level == 1 || entry & PDE_PS != 0 {
            return Some(entry);
        }
        // KB: `US` của cấp trên AND xuống — trả quyền **hiệu lực** bằng cách AND dồn.
        table = entry & 0x000F_FFFF_FFFF_F000;
        level -= 1;
    }
}

/// Stub kiểm kiểu của [`report`] cho target không phải x86_64.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn report(_layout: &ImageLayout) -> WxReport {
    WxReport::default()
}

/// `true` ⇔ `CR0.WP` đang bật.
pub fn cr0_wp() -> bool {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        asm::read_cr0() & CR0_WP != 0
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    {
        false
    }
}

/// PA bảng PT kernel đã cài (0 = chưa bật).
pub fn pt_pa() -> u64 {
    PT_PA.load(Ordering::Acquire)
}
