// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Ring trace kernel** — kho bản ghi 32 B **luôn bật** trong ảnh (`D2-09`, KT-408).
//!
//! # Vì sao kho ring ở **crate này**, không ở `vifast-core`/`vifast-traceformat`
//!
//! `vifast-core` và `vifast-traceformat` đều là `#![forbid(unsafe_code)]` (`D2-17`), còn từ
//! Rust 1.82 `link_section` là **thuộc tính unsafe** ⇒ `forbid(unsafe_code)` **từ chối** nó.
//! Kho ring **phải** nằm trong section **NOLOAD `.corespace`** (0 tăng `.data+.bss`, `D2-252`)
//! nên phải có `link_section`. `vifast-sync` là crate **thấp nhất** trong danh sách **đóng**
//! được dùng `unsafe` của `D2-17` mà `vifast-core` được phép phụ thuộc ⇒ kho ring đặt ở đây;
//! `vifast-traceformat` giữ **định dạng** 32 B (`Record`/`RECORD_BYTES`, `forbid(unsafe_code)`),
//! `vifast-core` chỉ **gọi** API an toàn ở đây và giữ nguyên `forbid(unsafe_code)`.
//!
//! # Bố trí
//!
//! | Phần | Ở đâu | Cỡ |
//! |---|---|---|
//! | `RING` (bản ghi) | `.corespace` NOLOAD trên target `none`; `.bss` trên host | 8 KiB |
//! | `HEADS`/`COUNTS` | cùng section | 128 B |
//!
//! `.corespace` là **NOLOAD** ở cả 3 arch (`linker.ld`: `KEEP(*(.corespace))`) ⇒ kho ring **không**
//! chiếm `filesz`, **không** cộng vào trần `.data+.bss`. Trên host (`cargo test`, Mach-O) `cfg_attr`
//! **không** gắn `link_section` (Mach-O không có section `.corespace`) ⇒ ring vào `.bss` tiến trình
//! test — vô hại.
//!
//! # Vì sao `AtomicU64`
//!
//! Không `static mut` (không `unsafe`), không cấp phát. `Ordering::Relaxed` vì trace là **chẩn
//! đoán**, không đồng bộ hoá gì. Mỗi bản ghi là 4 word 64-bit **liền nhau** ⇒ `pmemsave` đọc ra
//! đúng 32 B/bản ghi như `spec/abi.toml [trace]` mô tả.

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_traceformat::{Record, RECORD_BYTES};

/// Số hart tối đa của ring — **phải khớp** `vifast_core::MAX_HARTS` (const assert ở `vifast-core`).
pub const MAX_HARTS: usize = 8;
/// Số bản ghi **mỗi hart** (ring vòng, ghi đè bản ghi cũ nhất).
pub const PER_HART: usize = 32;
/// Tổng số bản ghi của ring.
pub const CAP: usize = PER_HART * MAX_HARTS;

/// Số word 64-bit của một bản ghi.
const WORDS: usize = RECORD_BYTES / 8;
/// Số word của kho ring.
const RING_WORDS: usize = CAP * WORDS;

/// Kho bản ghi — `.corespace` NOLOAD trên bare-metal (`D2-252`).
///
/// **`#[used]` là bắt buộc** (`KT-408b`, đo được): trong ảnh bare-metal **không ai đọc** ring
/// (`len_hart`/`read_record`/`dump_hart` chỉ có ở `#[cfg(test)]`), nên `RING` là `static` nội bộ
/// **không thoát** ⇒ LTO (`lto = true`, `codegen-units = 1`) + DSE **xoá sạch** cả kho lẫn store
/// của nó. Đo trên ảnh `vifast-kbin-ktest` (`--features suite-base`): `llvm-nm` chỉ thấy
/// `HEADS`/`COUNTS`, `.corespace` = `0x2d208` = `Machine` + `0x80` ⇒ **8 KiB kho không có trong
/// ảnh** và `pmemsave` không có gì để đọc. `#[used]` đưa ký hiệu vào `llvm.used` (địa chỉ thoát
/// ⇒ DSE không xoá được store) và **không** đổi `.data+.bss` (section NOLOAD) lẫn
/// `size_of::<Machine>()` (ring là `static` rời, không nằm trong `Machine`).
#[used]
#[cfg_attr(target_os = "none", link_section = ".corespace")]
static RING: [AtomicU64; RING_WORDS] = [const { AtomicU64::new(0) }; RING_WORDS];

/// Con trỏ ghi (số bản ghi đã ghi, tăng đơn điệu) của từng hart.
#[used]
#[cfg_attr(target_os = "none", link_section = ".corespace")]
static HEADS: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];

/// Số bản ghi **hợp lệ** hiện có của từng hart (`min(head, PER_HART)`).
#[used]
#[cfg_attr(target_os = "none", link_section = ".corespace")]
static COUNTS: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];

/// Ghi một bản ghi cho hart `hart` (ring vòng; `hart >= MAX_HARTS` ⇒ bỏ, fail-closed).
///
/// Không panic, không cấp phát, không `unsafe`.
pub fn push(hart: usize, t_ns: u64, kind: u16, tcb: u32, a: u64, b: u64) {
    if hart >= MAX_HARTS {
        return;
    }
    let head = HEADS[hart].load(Ordering::Relaxed);
    let slot = (head as usize) % PER_HART;
    let base = (hart * PER_HART + slot) * WORDS;
    // 4 word liền nhau = đúng 32 B của `spec/abi.toml [trace]`:
    //   w0 = t_ns | w1 = hart:u16 | kind:u16 | tcb:u32 | w2 = a | w3 = b
    RING[base].store(t_ns, Ordering::Relaxed);
    RING[base + 1].store(
        (hart as u64) | ((kind as u64) << 16) | ((tcb as u64) << 32),
        Ordering::Relaxed,
    );
    RING[base + 2].store(a, Ordering::Relaxed);
    RING[base + 3].store(b, Ordering::Relaxed);
    HEADS[hart].store(head.wrapping_add(1), Ordering::Relaxed);
    let n = COUNTS[hart].load(Ordering::Relaxed);
    if (n as usize) < PER_HART {
        COUNTS[hart].store(n + 1, Ordering::Relaxed);
    }
}

/// Số bản ghi **hợp lệ** hiện có của hart `hart` (`0` nếu `hart >= MAX_HARTS`).
pub fn len(hart: usize) -> usize {
    if hart >= MAX_HARTS {
        return 0;
    }
    COUNTS[hart].load(Ordering::Relaxed) as usize
}

/// Đọc bản ghi thứ `idx` (**0 = cũ nhất còn giữ**) của hart `hart`.
pub fn read(hart: usize, idx: usize) -> Option<Record> {
    let n = len(hart);
    if hart >= MAX_HARTS || idx >= n {
        return None;
    }
    let head = HEADS[hart].load(Ordering::Relaxed) as usize;
    let slot = (head + PER_HART - n + idx) % PER_HART;
    let base = (hart * PER_HART + slot) * WORDS;
    let w0 = RING[base].load(Ordering::Relaxed);
    let w1 = RING[base + 1].load(Ordering::Relaxed);
    let w2 = RING[base + 2].load(Ordering::Relaxed);
    let w3 = RING[base + 3].load(Ordering::Relaxed);
    Some(Record {
        t_ns: w0,
        hart: (w1 & 0xFFFF) as u16,
        kind: ((w1 >> 16) & 0xFFFF) as u16,
        tcb: ((w1 >> 32) & 0xFFFF_FFFF) as u32,
        a: w2,
        b: w3,
    })
}

/// Sao **byte thô** của ring hart `hart` vào `out` (đúng định dạng `pmemsave`/`vifast-trace.py`).
///
/// Trả số byte đã ghi (`len(hart) * 32`, cắt theo `out.len()`).
pub fn dump(hart: usize, out: &mut [u8]) -> usize {
    let n = len(hart);
    let mut written = 0usize;
    for idx in 0..n {
        let Some(rec) = read(hart, idx) else {
            break;
        };
        if written + RECORD_BYTES > out.len() {
            break;
        }
        let w1 = (rec.hart as u64) | ((rec.kind as u64) << 16) | ((rec.tcb as u64) << 32);
        for (i, w) in [rec.t_ns, w1, rec.a, rec.b].iter().enumerate() {
            let at = written + i * 8;
            out[at..at + 8].copy_from_slice(&w.to_le_bytes());
        }
        written += RECORD_BYTES;
    }
    written
}

/// Xoá **toàn bộ** ring (mọi hart) — gọi lúc `Machine::init_zeroed` (section NOLOAD **không** được
/// loader zero) và trong test host.
pub fn reset_all() {
    for w in RING.iter() {
        w.store(0, Ordering::Relaxed);
    }
    for h in HEADS.iter() {
        h.store(0, Ordering::Relaxed);
    }
    for c in COUNTS.iter() {
        c.store(0, Ordering::Relaxed);
    }
}

// **KT-408** — bằng chứng cỡ (const assert: chạy cả khi build bare-metal).
const _: () = assert!(
    core::mem::size_of::<Record>() == RECORD_BYTES,
    "Record phải đúng 32 B (spec/abi.toml [trace].record_bytes)"
);
