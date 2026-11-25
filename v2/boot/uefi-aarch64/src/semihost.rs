// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ARM semihosting** (QEMU `-semihosting-config enable=on,target=native`) — hai thao tác duy
//! nhất mà boot stub cần **sau** `ExitBootServices`, khi `ConOut` của firmware đã chết:
//!
//! - [`SYS_WRITE0`] (0x04): in chuỗi NUL-terminated ra console semihosting (QEMU: **stderr**,
//!   tức `qemu.out` của runner) — dùng cho bằng chứng `# uefi: after-ebs=ok`;
//! - [`SYS_EXIT_EXTENDED`] (0x20): thoát QEMU với **mã thoát** của ta, lý do
//!   [`ADP_STOPPED_APPLICATION_EXIT`].
//!
//! # Vì sao semihosting (không phải `PSCI SYSTEM_OFF`)
//!
//! `PSCI SYSTEM_OFF` tắt máy nhưng **luôn** cho `rc = 0`, nên runner không phân biệt được "xanh"
//! với "fail-closed" nếu chỉ nhìn mã thoát. Semihosting `SYS_EXIT_EXTENDED` mang **mã thoát của
//! ta** ra tiến trình QEMU ⇒ cùng thang `rc` với `v2/boot/uefi-x86_64` (`isa-debug-exit`):
//! **33** xanh, **65** fail-closed bảng nhớ, **67** lỗi `ExitBootServices`.
//!
//! # Đo được (QEMU 11.1.3 + AAVMF `edk2-aarch64-code.fd`, 2026-11-25)
//!
//! `SYS_EXIT_EXTENDED` với block `{0x20026, code}` cho `rc = code` **nguyên văn** (không phải
//! `(code<<1)|1` như `isa-debug-exit`), nên [`qemu_rc`] nhân đôi + 1 để **giữ nguyên** thang rc
//! của x86_64. `SYS_WRITE0` ghi ra stderr của QEMU (không phải `-serial`).
//!
//! # An toàn
//!
//! `hlt #0xF000` chỉ có nghĩa khi QEMU bật semihosting; trên máy thật nó là lệnh không xác định.
//! Boot stub này **chỉ** chạy trong khung test QEMU (cùng giả định với `isa-debug-exit` của
//! x86_64). Nếu QEMU không bật semihosting, lệnh trap vào firmware (đã chết sau
//! `ExitBootServices`) ⇒ treo; runner có watchdog và `rc = 124` **không** phải kết quả hợp lệ.

/// `ADP_Stopped_ApplicationExit` — lý do thoát "ứng dụng kết thúc bình thường" (ARM semihosting).
pub const ADP_STOPPED_APPLICATION_EXIT: u64 = 0x20026;
/// `SYS_WRITE0` — ghi chuỗi NUL-terminated ra console semihosting.
pub const SYS_WRITE0: u64 = 0x04;
/// `SYS_EXIT_EXTENDED` — thoát với block `{reason, exit_code}` (hai trường 64-bit).
pub const SYS_EXIT_EXTENDED: u64 = 0x20;

/// Trần một dòng semihosting (đủ cho mọi dòng bằng chứng; dài hơn bị cắt, không panic).
const LINE_MAX: usize = 300;

/// Mã thoát tiến trình QEMU cho một giá trị `isa-debug-exit`-tương đương: `(val << 1) | 1`.
///
/// Giữ **cùng** thang `rc` với `v2/boot/uefi-x86_64` (33/65/67) để runner hai arch so sánh được.
pub const fn qemu_rc(val: u32) -> u32 {
    (val << 1) | 1
}

/// Gọi semihosting: `x0` = thao tác, `x1` = tham số; trả `x0` sau lời gọi.
///
/// # Safety
/// Chỉ gọi khi QEMU bật `-semihosting-config`; `arg` phải là địa chỉ đọc được theo ABI của
/// thao tác (chuỗi NUL-terminated với `SYS_WRITE0`, block 16 byte với `SYS_EXIT_EXTENDED`).
#[cfg(target_arch = "aarch64")]
unsafe fn semi(op: u64, arg: u64) -> u64 {
    let ret: u64;
    // SAFETY: `hlt #0xF000` là cửa semihosting của ARM; không chạm bộ nhớ của ta.
    unsafe {
        core::arch::asm!("hlt #0xF000", inout("x0") op => ret, in("x1") arg, options(nostack));
    }
    ret
}

/// Trên host **không** phải aarch64 (chỉ `cargo test` chạy được ở arch khác): không có
/// semihosting — trả 0. Không lời gọi nào trong test chạm tới đường này.
///
/// # Safety
/// Hàm không chạm bộ nhớ.
#[cfg(not(target_arch = "aarch64"))]
unsafe fn semi(_op: u64, _arg: u64) -> u64 {
    0
}

/// In **một** dòng ra console semihosting (thêm `\n`; cắt ở [`LINE_MAX`], không panic).
///
/// # Safety
/// Như [`semi`]; `s` chỉ cần đọc được trong lúc gọi (bản sao nằm trên stack của ta).
pub unsafe fn write0(s: &str) {
    let mut buf = [0u8; LINE_MAX + 2];
    let n = s.len().min(LINE_MAX);
    buf[..n].copy_from_slice(&s.as_bytes()[..n]);
    buf[n] = b'\n';
    buf[n + 1] = 0;
    // SAFETY: `buf` là chuỗi NUL-terminated sống tới hết lời gọi (điều kiện của `semi`).
    unsafe {
        semi(SYS_WRITE0, buf.as_ptr() as u64);
    }
}

/// Thoát QEMU với mã `qemu_rc(val)` qua `SYS_EXIT_EXTENDED`; **không** trả về.
///
/// # Safety
/// Như [`semi`]; chỉ gọi trong QEMU có `-semihosting-config enable=on`.
pub unsafe fn exit_qemu(val: u32) -> ! {
    let block: [u64; 2] = [ADP_STOPPED_APPLICATION_EXIT, u64::from(qemu_rc(val))];
    // SAFETY: `block` là hai trường 64-bit sống tới hết lời gọi (điều kiện của `semi`).
    unsafe {
        semi(SYS_EXIT_EXTENDED, block.as_ptr() as u64);
    }
    // QEMU không thoát (semihosting tắt) ⇒ treo tới watchdog của runner; `wfi` cho CPU nghỉ.
    loop {
        // SAFETY: `wfi` chỉ đưa lõi vào trạng thái chờ; không chạm bộ nhớ.
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack, preserves_flags));
        }
    }
}
