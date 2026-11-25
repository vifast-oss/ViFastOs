// SPDX-License-Identifier: MIT OR Apache-2.0
//! In dòng bằng chứng ra `ConOut` **trước** `ExitBootServices`, và cổng thoát QEMU sau đó.
//!
//! Mọi dòng log của boot stub có dạng ổn định `# uefi: key=value ...` (hoặc `# bootinfo: ...`
//! cho phần bàn giao) để runner `grep` được, không phụ thuộc thứ tự hay văn xuôi.

use core::fmt::{self, Write};

use crate::efi::{SimpleTextOutput, EfiStatus};

/// Trần một dòng (đủ cho mọi dòng bằng chứng; dài hơn bị cắt, không panic).
const LINE_MAX: usize = 300;

/// Đệm ASCII cho `core::fmt` — không cần cấp phát.
pub struct LineBuf {
    buf: [u8; LINE_MAX],
    len: usize,
}

impl LineBuf {
    /// Đệm rỗng.
    pub const fn new() -> Self {
        LineBuf {
            buf: [0; LINE_MAX],
            len: 0,
        }
    }

    /// Nội dung đã ghi.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl Default for LineBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for LineBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = LINE_MAX - self.len;
        let n = s.len().min(room);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// In **một** dòng qua `ConOut->OutputString` (tự thêm `\r\n`; UTF-16 tại chỗ).
///
/// Chỉ hợp lệ **trước** `ExitBootServices`: sau đó `ConOut` là dịch vụ boot đã chết.
///
/// # Safety
/// `out` phải là `ConOut` lấy từ `EFI_SYSTEM_TABLE` do firmware truyền, còn hiệu lực.
pub unsafe fn line(out: *mut SimpleTextOutput, args: fmt::Arguments<'_>) {
    if out.is_null() {
        return;
    }
    let mut lb = LineBuf::new();
    let _ = lb.write_fmt(args);
    let bytes = lb.as_bytes();
    let mut wide = [0u16; LINE_MAX + 3];
    let mut n = 0usize;
    for &b in bytes {
        // Log của ta là ASCII; byte ngoài ASCII in thành '?' để không cắt chuỗi giữa chừng.
        wide[n] = if b.is_ascii() { u16::from(b) } else { u16::from(b'?') };
        n += 1;
    }
    wide[n] = u16::from(b'\r');
    wide[n + 1] = u16::from(b'\n');
    wide[n + 2] = 0;
    // SAFETY: `out` là ConOut hợp lệ (điều kiện của hàm); `wide` sống tới hết lời gọi.
    unsafe {
        ((*out).output_string)(out, wide.as_ptr());
    }
}

/// Cổng debug QEMU (`isa-debugcon`, mặc định `0xE9`) — **sống sau** `ExitBootServices` vì là
/// cổng I/O của QEMU, không phải dịch vụ firmware.
///
/// # Safety
/// Chỉ gọi trên `x86_64` dưới QEMU có `-debugcon`; trên máy thật cổng 0xE9 có thể là thiết bị
/// khác — boot stub này **chỉ** chạy trong khung test QEMU.
pub unsafe fn debugcon_str(s: &str) {
    for &b in s.as_bytes() {
        // SAFETY: chỉ ghi cổng I/O; không truy cập bộ nhớ.
        unsafe {
            debugcon_byte(b);
        }
    }
}

/// Ghi một byte ra cổng debug QEMU `0xE9` (`isa-debugcon`).
///
/// # Safety
/// Như [`debugcon_str`].
#[cfg(target_arch = "x86_64")]
pub unsafe fn debugcon_byte(b: u8) {
    // SAFETY: `out dx, al` chỉ ghi cổng I/O; không truy cập bộ nhớ.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0xE9u16, in("al") b, options(nomem, nostack, preserves_flags));
    }
}

/// Trên host khác `x86_64` (chỉ `cargo test --lib`), cổng debug QEMU không tồn tại: bỏ qua.
///
/// # Safety
/// Hàm không chạm bộ nhớ; giữ chữ ký để `debugcon_str` dùng chung.
#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn debugcon_byte(_b: u8) {}

/// Thoát QEMU qua `isa-debug-exit` (`iobase=0x604`): mã thoát tiến trình = `(val << 1) | 1`.
///
/// Dùng để runner phân biệt **xanh** (`val=0x10` ⇒ rc 33) với **fail-closed** (`val=0x20` ⇒ rc
/// 65; `val=0x21` ⇒ rc 67) mà không phải đọc log mới biết.
///
/// # Safety
/// Chỉ gọi trong QEMU có `-device isa-debug-exit,iobase=0x604`; không trả về.
pub unsafe fn qemu_exit(val: u32) -> ! {
    // SAFETY: ghi cổng I/O 32-bit; QEMU thoát tiến trình.
    unsafe {
        qemu_exit_port(val);
    }
    loop {
        // SAFETY: nếu QEMU không thoát thì treo; runner có watchdog.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
        #[cfg(not(target_arch = "x86_64"))]
        core::hint::spin_loop();
    }
}

/// Ghi giá trị thoát ra `isa-debug-exit` tại `iobase=0x501` (mặc định của QEMU; `0x604`
/// **không** thoát trên q35 của QEMU 11 — đo được trong ngày làm task, xem README §3).
///
/// # Safety
/// Như [`qemu_exit`].
#[cfg(target_arch = "x86_64")]
unsafe fn qemu_exit_port(val: u32) {
    // SAFETY: `out dx, eax` chỉ ghi cổng I/O.
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") 0x501u16, in("eax") val, options(nomem, nostack, preserves_flags));
    }
}

/// Trên host khác `x86_64` (chỉ `cargo test --lib`): không có `isa-debug-exit` — không làm gì;
/// `qemu_exit` sau đó rơi vào vòng `hlt` (không gọi trong test).
///
/// # Safety
/// Hàm không chạm bộ nhớ.
#[cfg(not(target_arch = "x86_64"))]
unsafe fn qemu_exit_port(_val: u32) {}

/// Mã thoát QEMU cho đường **xanh** (`isa-debug-exit` ⇒ rc 33).
pub const EXIT_OK: u32 = 0x10;
/// Mã thoát QEMU cho **fail-closed bảng nhớ** (⇒ rc 65).
pub const EXIT_FAIL_MAP: u32 = 0x20;
/// Mã thoát QEMU cho **fail-closed `ExitBootServices`** (⇒ rc 67).
pub const EXIT_FAIL_EBS: u32 = 0x21;

/// `EFI_SUCCESS` có phải thành công không (dùng chung với `efi`).
pub const fn is_ok(status: EfiStatus) -> bool {
    status == 0
}
