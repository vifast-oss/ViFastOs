// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340a** — pha kiểm **W^X kernel x86_64** (TAP 35/36).
//!
//! # Hai phép đo — hai TAP
//!
//! 1. **TAP 35 `wx: kernel sections`** — quyền của **ba đoạn ảnh** đọc lại **từ bảng trang đang
//!    hoạt động** (`wx::walk_leaf`), **không** từ hằng số:
//!    - `.text` = `P|X`, **không** `W`, không `NX`;
//!    - `.rodata` = `P|NX`, **không** `W`;
//!    - `.data` = `P|RW|NX`, **không** `US`;
//!    - `CR0.WP = 1` (thiếu nó thì ring 0 vẫn ghi được trang RO);
//!    - đếm **từ bảng trang** số trang `W` **và** `X` trong `0..2 MiB`: ảnh ktest đúng **1**
//!      (trang trampoline AP `0x8000` — ngoại lệ có ghi rõ trong `wx.rs`), ảnh sản phẩm **0**.
//!
//! 2. **TAP 36 `wx: text write faults`** — **ghi thật** vào `.text` từ CPL0:
//!    - `#PF` với `err` có `P=1, W=1` (`0x3`) và `CR2` = đúng địa chỉ đã ghi;
//!    - byte của `.text` **không đổi** (ghi bị từ chối, không phải "rồi hoàn tác");
//!    - ghi cùng giá trị vào `.data` thì **thành công** và đọc lại đúng ⇒ `.data` là RW thật,
//!      không phải "cả ảnh chỉ-đọc".
//!
//! # Vì sao pha này **không** nằm trong ảnh sản phẩm
//!
//! Ảnh sản phẩm **bật** W^X (`product::wx_init`) nhưng **không** đo — đo là harness (D2-08).
//! Việc bật ở ảnh sản phẩm vẫn kiểm được bằng máy: `product::wx_init` trả report và ảnh halt; lỗi
//! bật (ví dụ quên `CR0.WP`) không làm boot đỏ, nên pha đo **chỉ** có ở ảnh ktest.

use core::fmt::Write;

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::exc;

use crate::product::{wx_layout, wx_report};
use crate::{check, Console, EXC_CR2, EXC_ERR, EXC_VEC};

/// Byte "mồi" của stub asm [`asm::probe_write8`].
const PROBE_BYTE: u8 = 0x5A;

/// Chạy pha W^X; `tap_first` = số hiệu TAP của ca đầu (35).
pub(crate) fn run(console: &mut Console, tap_first: usize) {
    let layout = wx_layout();
    let rep = wx_report();

    // ---- TAP 35: quyền ba đoạn + CR0.WP, đọc từ bảng trang.
    // Ảnh ktest có **đúng một** trang W+X: trampoline AP ở `0x8000` (BSP ghi boot block + AP
    // chạy tại đó). Ảnh sản phẩm không có `smp.rs` ⇒ 0.
    let wx_expected = if cfg!(feature = "ktest") { 1 } else { 0 };
    let sections_ok = rep.wp
        && rep.text_is_rx()
        && rep.rodata_is_ro()
        && rep.data_is_rw_nx()
        && rep.wx_pages == wx_expected;
    let _ = writeln!(
        console,
        "# wx: cr0_wp={} text_end=0x{:x} rodata_end=0x{:x} data_end=0x{:x} text=0x{:016x} rodata=0x{:016x} data=0x{:016x} low=0x{:016x} wx_pages={} pt=0x{:x}",
        rep.wp as u8,
        layout.text_end,
        layout.data_start,
        layout.kernel_end,
        rep.text,
        rep.rodata,
        rep.data,
        rep.low_tramp,
        rep.wx_pages,
        vifast_plat_x86_64::wx::pt_pa(),
    );
    check(console, tap_first, "wx: kernel sections", sections_ok);

    // ---- TAP 36: ghi vào `.text` phải `#PF` (P=1, W=1); `.text` không đổi; `.data` ghi được.
    // Địa chỉ trong **giữa** `.text` (bỏ 0x40 byte đầu để không chạm header bắt buộc).
    let text_addr = layout.text_start + 0x40;
    let text_before = asm::read_u8(text_addr);

    crate::exc_reset();
    crate::exc_arm(exc::VEC_PF as u64, asm::probe_write8_resume());
    asm::probe_write8(text_addr);
    let fault_ok = EXC_VEC.load(core::sync::atomic::Ordering::Relaxed) == exc::VEC_PF as u64
        && EXC_ERR.load(core::sync::atomic::Ordering::Relaxed) & 0x3 == 0x3
        && EXC_CR2.load(core::sync::atomic::Ordering::Relaxed) == text_addr
        && asm::read_u8(text_addr) == text_before;
    let _ = writeln!(
        console,
        "# wx: text write addr=0x{text_addr:x} err=0x{:x} cr2=0x{:x} unchanged={}",
        EXC_ERR.load(core::sync::atomic::Ordering::Relaxed),
        EXC_CR2.load(core::sync::atomic::Ordering::Relaxed),
        (asm::read_u8(text_addr) == text_before) as u8,
    );

    // Đối chứng `.data`: cùng stub ghi, **không** arm `#PF` — phải ghi được và đọc lại đúng.
    // Chọn giữa `.data` để chắc chắn không trùng byte đã dùng; khôi phục giá trị cũ sau khi đo.
    let data_addr = layout.data_start + 0x100;
    let data_before = asm::read_u8(data_addr);
    asm::probe_write8(data_addr);
    let data_written = asm::read_u8(data_addr) == PROBE_BYTE;
    asm::write_u8(data_addr, data_before);
    let restored = asm::read_u8(data_addr) == data_before;
    let _ = writeln!(
        console,
        "# wx: data write addr=0x{data_addr:x} written={} restored={}",
        data_written as u8, restored as u8,
    );
    check(
        console,
        tap_first + 1,
        "wx: text write faults",
        fault_ok && data_written && restored,
    );
}
