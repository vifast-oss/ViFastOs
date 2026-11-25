// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Ring trace nhị phân 32 B luôn bật** (`D2-09`, `D2-40`, `spec/abi.toml [trace]`) — KT-408.
//!
//! # Vì sao "luôn bật"
//!
//! `D2-09`: trace **luôn bật**, **không** có cờ tắt. Vì vậy ring **không** là `Option`, không cấp
//! phát động, và **không** đẩy chi phí vào `.data+.bss` của ảnh sản phẩm (trần 262.144 B,
//! `D2-252`). `KT-403` đã làm **định dạng** (`v2/lib/vifast-traceformat`, `FixedRing`,
//! `v2/tools/vifast-trace.py`); KT-408 nối nó **vào kernel**.
//!
//! # Bố trí bộ nhớ (0 tăng `.data+.bss`, 0 tăng `size_of::<Machine>()`)
//!
//! | Phần | Ở đâu | Cỡ |
//! |---|---|---|
//! | Kho bản ghi | `vifast_sync::kernel_ring::RING` — section **NOLOAD `.corespace`** | 8 KiB |
//! | Đầu/cuối ring mỗi hart | cùng section | 128 B |
//! | Handle mỗi hart | [`TraceRing`] — **ZST** trong [`crate::PerHartState`] | **0 B** |
//!
//! Kho ring nằm ở crate `vifast-traceformat` (chủ sở hữu định dạng 32 B từ `KT-12`/`KT-403`) vì
//! `vifast-core` là `#![forbid(unsafe_code)]` (`D2-17`) và từ Rust 1.82 `link_section` là **thuộc
//! tính unsafe** — `forbid` từ chối nó. Nhờ vậy `vifast-core` **giữ nguyên** `forbid(unsafe_code)`
//! và `size_of::<Machine>()` **không đổi** (pin harness `184712`, `D2-84`/`D2-252`).
//!
//! `.corespace` là **NOLOAD** ở cả 3 arch (`linker.ld`: `KEEP(*(.corespace))`) nên kho ring
//! **không** chiếm `filesz`, **không** cộng vào trần `.data+.bss` — đúng yêu cầu `D2-252`.
//!
//! # Định dạng bản ghi (nguồn sự thật: `spec/abi.toml [trace]`)
//!
//! ```text
//! t_ns:u64  hart:u16  kind:u16  tcb:u32  a:u64  b:u64      (32 B, little-endian)
//! ```
//!
//! `kind`: 1 syscall, 2 ipc, 3 sched, 4 fault, 5 store, 6 run, 7 idle.

use crate::machine::MAX_HARTS;

/// Cỡ **một** bản ghi trace (B) — `spec/abi.toml [trace].record_bytes`.
pub const TRACE_RECORD_BYTES: usize = vifast_traceformat::RECORD_BYTES;
/// Số bản ghi **mỗi hart** (ring vòng, ghi đè bản ghi cũ nhất).
pub const RING_PER_HART: usize = vifast_sync::kernel_ring::PER_HART;
/// Tổng số bản ghi của ring (chia đều cho [`MAX_HARTS`]).
pub const RING_CAP: usize = vifast_sync::kernel_ring::CAP;
/// Giá trị `tcb` "không có TCB" (`vifast-trace.py` in `-`).
pub const TCB_NONE: u32 = 0xFFFF_FFFF;

/// Một bản ghi trace **32 B** — bố cục khớp `spec/abi.toml [trace].fields`.
///
/// Tái xuất **nguyên** kiểu của `vifast-traceformat` (`KT-12`/`KT-403`) để kernel, sim và
/// `vifast-trace.py` dùng **một** định dạng duy nhất (`D2-38`).
pub use vifast_traceformat::Record as TraceRecord;

/// Loại sự kiện — `spec/abi.toml [trace].kind` (1..7).
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    /// 1 — vào syscall.
    Syscall = 1,
    /// 2 — IPC (send/recv/call/reply).
    Ipc = 2,
    /// 3 — lập lịch (dispatch/block/wake).
    Sched = 3,
    /// 4 — fault.
    Fault = 4,
    /// 5 — thao tác store (retype/revoke/delete).
    Store = 5,
    /// 6 — TCB chạy.
    Run = 6,
    /// 7 — hart rảnh.
    Idle = 7,
}

impl TraceKind {
    /// Mã `u16` theo `spec/abi.toml [trace].kind`.
    pub const fn code(self) -> u16 {
        self as u16
    }
}

/// **Handle ring của một hart** — **ZST** (0 B) đặt trong [`crate::PerHartState`].
///
/// Kho bản ghi là `static` **dùng chung** cho ảnh (một ring cho mọi hart, mỗi hart một dải
/// `RING_PER_HART` bản ghi); handle này chỉ đánh dấu **kênh** trace thuộc hart đó. Nhờ ZST,
/// `size_of::<Machine>()` **không** đổi (pin `184712`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TraceRing;

impl TraceRing {
    /// Handle rỗng (hằng — dùng trong `PerHartState::new`).
    pub const fn new() -> Self {
        Self
    }

    /// Ghi một bản ghi cho hart `hart` (ring vòng: đầy ⇒ ghi đè bản ghi cũ nhất).
    ///
    /// Không panic, không cấp phát, không `unsafe`; `hart >= MAX_HARTS` ⇒ bỏ (fail-closed).
    pub fn push(&self, hart: usize, t_ns: u64, kind: TraceKind, tcb: u32, a: u64, b: u64) {
        vifast_sync::kernel_ring::push(hart, t_ns, kind.code(), tcb, a, b);
    }
}

/// Số bản ghi **hợp lệ** hiện có của hart `hart` (`0` nếu `hart >= MAX_HARTS`).
pub fn len_hart(hart: usize) -> usize {
    vifast_sync::kernel_ring::len(hart)
}

/// Đọc bản ghi thứ `idx` (**0 = cũ nhất**) của hart `hart`.
pub fn read_record(hart: usize, idx: usize) -> Option<TraceRecord> {
    vifast_sync::kernel_ring::read(hart, idx)
}

/// Sao **byte thô** của ring hart `hart` vào `out` (đúng định dạng `pmemsave`/`vifast-trace.py`).
///
/// Trả số byte đã ghi (`len_hart(hart) * 32`, cắt theo `out.len()`).
pub fn dump_hart(hart: usize, out: &mut [u8]) -> usize {
    vifast_sync::kernel_ring::dump(hart, out)
}

/// Xoá **toàn bộ** ring (mọi hart) — dùng lúc `init_zeroed` (section NOLOAD **không** được loader
/// zero) và trong test host.
pub fn reset_all() {
    vifast_sync::kernel_ring::reset_all();
}

// **KT-408** — bằng chứng **cỡ** (const assert: chạy cả khi build bare-metal).
const _: () = assert!(
    core::mem::size_of::<TraceRecord>() == TRACE_RECORD_BYTES,
    "TraceRecord phải đúng 32 B (spec/abi.toml [trace].record_bytes)"
);
const _: () = assert!(
    core::mem::size_of::<TraceRing>() == 0,
    "TraceRing phải 0 B (ZST) — size_of::<Machine>() không được đổi (pin 184712)"
);
const _: () = assert!(
    vifast_sync::kernel_ring::MAX_HARTS == MAX_HARTS,
    "kernel_ring::MAX_HARTS phải khớp vifast_core::MAX_HARTS"
);
