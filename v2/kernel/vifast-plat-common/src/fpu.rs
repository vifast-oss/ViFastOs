// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — **ngữ cảnh FPU/SIMD eager theo TCB**: kiểu vùng lưu + sổ khe **an toàn** dùng
//! chung 3 arch.
//!
//! # Vì sao ở `vifast-plat-common` (và vì sao **không** ở `TcbCore`)
//!
//! `TcbCore` là **thân ABI** (`spec/abi.toml:159` `tcb = 512`, `const` assert ≤ 496 B ở
//! `vifast-core/src/tcbdata.rs`) — nhét 272 B vùng FPU vào đó là phá trần ABI. Vùng lưu vì vậy
//! nằm **ngoài** `TcbCore`, trong `.bss` của crate arch (mỗi arch một bản, cùng **một** kiểu
//! `FpuArea` ở đây). Phần **quyết định** (khe nào của TCB nào, đã có trạng thái để khôi phục
//! chưa, bộ đếm bằng chứng) là logic thuần ⇒ ở crate `#![forbid(unsafe_code)]` này, chạy được
//! trên host (`tests/fpu.rs`).
//!
//! # Vì sao **eager** + theo dõi bẩn
//!
//! Lazy switch (để cờ `CR0.TS`/`sstatus.FS=0` trap rồi mới lưu) làm mỗi lần dùng FP đầu tiên của
//! tác vụ tốn **một trap** — trên đường syscall/IPC dày đặc thì đó là chi phí không đoán được.
//! KT-418 chọn **eager**: lưu ngay khi ra U-mode, khôi phục ngay khi vào, và **chỉ** lưu khi phần
//! cứng báo **bẩn** (`sstatus.FS = Dirty` trên riscv64, `CR0.TS`/bit bẩn của `xsave` trên x86_64,
//! `fpsr`/cờ phần mềm trên aarch64) ⇒ tác vụ không dùng FP **không** tốn lần lưu nào.
//!
//! # Bất biến (kiểm bằng `tests/fpu.rs`)
//!
//! 1. Vùng lưu **272 B**, căn **16** (`fxsave64` đòi 16 B; `fs0..fs31`/`v0..v31` là 32 × 8 B).
//! 2. Khe **khoá theo TCB**: cùng TCB ⇒ cùng khe; hai TCB ⇒ hai khe khác nhau.
//! 3. **Tràn ⇒ fail-closed**: không cấp khe, không ghi đè khe của TCB khác, đếm `overflow`.
//! 4. `valid` **chỉ** sau khi đã lưu ⇒ lần vào đầu tiên của TCB là trạng thái **mới**, không phải
//!    rác của TCB khác.
//! 5. Mỗi lượt ra U-mode là **đúng một** trong `saves`/`skipped`; mỗi lượt vào là **đúng một**
//!    trong `restores`/`fresh` — bất biến đếm được cho TAP.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Số thanh ghi dữ liệu FPU/SIMD lưu/khôi phục: `fs0..fs31` (riscv64), `v0..v31` (aarch64),
/// `xmm0..xmm15` + `st0..st7` (x86_64 — `fxsave64` gói cả hai vào 512 B, xem [`FPU_AREA_BYTES`]).
pub const FPU_REGS: usize = 32;

/// Cỡ **một** thanh ghi trong vùng lưu: **16 B** = 128 bit — đủ cho `v0..v31` (NEON) và
/// `xmm0..xmm15`; riscv64 (`fs0..fs31`, 64 bit) dùng 8 B thấp của ô 16 B **cùng offset** để ba
/// arch chia **một** bố cục (cùng offset ⇒ cùng nghĩa "thanh ghi thứ `i`").
pub const FPU_REG_BYTES: usize = 16;

/// Số word trạng thái điều khiển: `fcsr` (riscv64) / `fpsr` + `fpcr` (aarch64) / `mxcsr` + từ
/// điều khiển x87 (x86_64).
pub const FPU_CTRL_WORDS: usize = 2;

/// Cỡ vùng lưu **một** TCB (byte) — **528** = 32 × 16 B + 2 × 8 B, bội số của 16.
///
/// Vì sao 528: `v0..v31` của aarch64 là **128 bit** (512 B) — vùng 272 B (chỉ 64 bit/thanh ghi)
/// **không** đủ. x86_64 dùng 512 B đầu cho ảnh `fxsave64` (bố cục riêng của nó, xem
/// `asm/fpu.rs` của `vifast-plat-x86_64`); riscv64 dùng 8 B thấp của mỗi ô 16 B + `fcsr` ở 512.
pub const FPU_AREA_BYTES: usize = FPU_REGS * FPU_REG_BYTES + FPU_CTRL_WORDS * 8;

/// Số khe tối đa: `MAX_TCBS = 64` (`spec/kernel.toml:33`) + 1 vì `ObjId` của core **1-based**
/// (khe `0` không dùng — TCB `0` là không hợp lệ).
pub const FPU_SLOTS: usize = 65;

/// Vùng lưu FPU/SIMD của **một** TCB — dữ liệu thuần, không `unsafe`.
///
/// Căn **16** để `fxsave64`/`fxrstor64` hợp lệ trên x86_64 (lệnh đòi địa chỉ 16 B); hai arch còn
/// lại không cần nhưng dùng chung kiểu nên giữ nguyên căn.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FpuArea {
    /// Ảnh thanh ghi + trạng thái điều khiển — **bố cục do từng arch quyết định** trong
    /// [`FPU_AREA_BYTES`] byte (cùng offset "thanh ghi thứ `i`" = `i * FPU_REG_BYTES`).
    pub bytes: [u8; FPU_AREA_BYTES],
}

impl FpuArea {
    /// Vùng **rỗng** — trạng thái khởi tạo của tác vụ chưa từng dùng FP.
    pub const ZERO: Self = Self {
        bytes: [0; FPU_AREA_BYTES],
    };

    /// Word 8 B tại `offset` (dùng cho test/asm đọc lại bằng chứng).
    pub fn word(&self, offset: usize) -> u64 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&self.bytes[offset..offset + 8]);
        u64::from_le_bytes(raw)
    }

    /// Ghi word 8 B tại `offset`.
    pub fn set_word(&mut self, offset: usize, value: u64) {
        self.bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}

/// Ảnh chụp bộ đếm — **bằng chứng** cho TAP/journal, không phải API sản phẩm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FpuStats {
    /// Số lần **lưu** vì phần cứng báo bẩn.
    pub saves: u32,
    /// Số lần **bỏ lưu** vì phần cứng báo sạch (tác vụ không dùng FP từ lần lưu trước).
    pub skipped: u32,
    /// Số lần **khôi phục** từ vùng lưu hợp lệ.
    pub restores: u32,
    /// Số lần vào U-mode với trạng thái **mới** (TCB chưa có vùng lưu hợp lệ).
    pub fresh: u32,
    /// Số lần **tràn khe** (fail-closed: không cấp khe, không ghi đè).
    pub overflow: u32,
}

/// Sổ khe FPU theo TCB — **chỉ bookkeeping**, không chứa vùng lưu (vùng lưu ở crate arch).
///
/// `N` = số khe; mọi thao tác **không** panic khi tràn (fail-closed + đếm `overflow`).
pub struct FpuTracker<const N: usize> {
    /// TCB sở hữu khe (`0` = khe trống).
    tcb: [AtomicU32; N],
    /// Khe đã có trạng thái **hợp lệ** để khôi phục (đã lưu ít nhất một lần).
    valid: [AtomicBool; N],
    saves: AtomicU32,
    skipped: AtomicU32,
    restores: AtomicU32,
    fresh: AtomicU32,
    overflow: AtomicU32,
}

impl<const N: usize> FpuTracker<N> {
    /// Sổ rỗng — `const` để đặt thẳng vào `static` của crate arch.
    pub const fn new() -> Self {
        Self {
            tcb: [const { AtomicU32::new(0) }; N],
            valid: [const { AtomicBool::new(false) }; N],
            saves: AtomicU32::new(0),
            skipped: AtomicU32::new(0),
            restores: AtomicU32::new(0),
            fresh: AtomicU32::new(0),
            overflow: AtomicU32::new(0),
        }
    }

    /// Khe của `tcb` — cấp mới nếu chưa có. `None` ⇔ `tcb == 0` (không hợp lệ) hoặc **tràn**
    /// (fail-closed: không ghi đè khe của TCB khác).
    pub fn slot_for(&self, tcb: u32) -> Option<usize> {
        if tcb == 0 {
            return None;
        }
        for (index, slot) in self.tcb.iter().enumerate() {
            if slot.load(Ordering::Acquire) == tcb {
                return Some(index);
            }
        }
        for (index, slot) in self.tcb.iter().enumerate() {
            if slot
                .compare_exchange(0, tcb, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(index);
            }
        }
        self.overflow.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Khe của `tcb` **nếu đã có** (không cấp mới) — dùng cho đường đọc/kiểm.
    pub fn slot_of(&self, tcb: u32) -> Option<usize> {
        if tcb == 0 {
            return None;
        }
        self.tcb
            .iter()
            .position(|slot| slot.load(Ordering::Acquire) == tcb)
    }

    /// `true` ⇔ `tcb` đã có trạng thái **hợp lệ** để khôi phục.
    pub fn is_valid(&self, tcb: u32) -> bool {
        match self.slot_of(tcb) {
            Some(index) => self.valid[index].load(Ordering::Acquire),
            None => false,
        }
    }

    /// Đánh dấu `tcb` đã có trạng thái hợp lệ (gọi **sau** khi asm đã ghi xong vùng lưu).
    ///
    /// Tràn khe ⇒ không làm gì (đã đếm `overflow` ở [`Self::slot_for`]).
    pub fn mark_valid(&self, tcb: u32) {
        if let Some(index) = self.slot_for(tcb) {
            self.valid[index].store(true, Ordering::Release);
        }
    }

    /// Xoá trạng thái hợp lệ của `tcb` (lần vào kế tiếp là trạng thái **mới**).
    pub fn invalidate(&self, tcb: u32) {
        if let Some(index) = self.slot_of(tcb) {
            self.valid[index].store(false, Ordering::Release);
        }
    }

    /// Ghi sổ một lần **lưu** (phần cứng báo bẩn).
    pub fn record_save(&self) {
        self.saves.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi sổ một lần **bỏ lưu** (phần cứng báo sạch).
    pub fn record_skip(&self) {
        self.skipped.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi sổ một lần **khôi phục**.
    pub fn record_restore(&self) {
        self.restores.fetch_add(1, Ordering::Relaxed);
    }

    /// Ghi sổ một lần vào U-mode với trạng thái **mới**.
    pub fn record_fresh(&self) {
        self.fresh.fetch_add(1, Ordering::Relaxed);
    }

    /// Số lần tràn khe đã đếm.
    pub fn overflow(&self) -> u32 {
        self.overflow.load(Ordering::Relaxed)
    }

    /// Ảnh chụp bộ đếm.
    pub fn stats(&self) -> FpuStats {
        FpuStats {
            saves: self.saves.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
            restores: self.restores.load(Ordering::Relaxed),
            fresh: self.fresh.load(Ordering::Relaxed),
            overflow: self.overflow.load(Ordering::Relaxed),
        }
    }
}

impl<const N: usize> Default for FpuTracker<N> {
    fn default() -> Self {
        Self::new()
    }
}
