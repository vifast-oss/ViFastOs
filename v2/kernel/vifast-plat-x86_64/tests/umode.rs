// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho U-mode (ring 3) x86_64 — KT-270 (K5' bước 4; D2-207).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test umode
//! ```
//!
//! Kỳ vọng là **giá trị tuyệt đối viết tay** từ Intel SDM (Vol 3A §3.4.5: selector + RPL;
//! §6.14: gate IDT; §5.5: `iretq` về CPL3; Vol 2B "INT n": `int 0x80` lưu RIP của lệnh **kế
//! tiếp**), **không** lấy hằng của crate để so — nếu lấy chính hằng cần kiểm thì test không có
//! răng.
//!
//! Đây là ca kiểm **đỏ trước** của KT-270 (D2-05): viết trước khi `umode` tồn tại. **Đột biến
//! bắt buộc** (i) — `cs` để ring 0 thay vì `0x1B` — làm [`is_user_cs`] sai ngay ở tầng dữ liệu
//! (`kernel_cs_is_not_user` ĐỎ); log QEMU của đột biến ở `v2/docs/k5-x86_64-boot.md` §10.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà asm của
// ảnh boot nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use core::mem::{align_of, size_of};

use vifast_plat_x86_64::exc::{
    FRAME_CS_OFFSET, FRAME_ERROR_OFFSET, FRAME_RFLAGS_OFFSET, FRAME_RIP_OFFSET,
    FRAME_VECTOR_OFFSET, VEC_SYSCALL,
};
use vifast_plat_x86_64::gdt::{SELECTOR_USER_CODE_RPL3, SELECTOR_USER_DATA_RPL3};
use vifast_plat_x86_64::umode::{
    is_kernel_cs, is_user_cs, rflags_if, SyscallRet, KERNEL_CS, KERNEL_DS, SYSCALL_FRAME_CS,
    SYSCALL_FRAME_ERROR, SYSCALL_FRAME_R10, SYSCALL_FRAME_R11, SYSCALL_FRAME_R12,
    SYSCALL_FRAME_R13, SYSCALL_FRAME_R14, SYSCALL_FRAME_R15, SYSCALL_FRAME_R8, SYSCALL_FRAME_R9,
    SYSCALL_FRAME_RAX, SYSCALL_FRAME_RBP, SYSCALL_FRAME_RBX, SYSCALL_FRAME_RCX, SYSCALL_FRAME_RDI,
    SYSCALL_FRAME_RDX, SYSCALL_FRAME_RFLAGS, SYSCALL_FRAME_RIP, SYSCALL_FRAME_RSI,
    SYSCALL_FRAME_VECTOR, USER_CODE_VA, USER_CS, USER_RFLAGS, USER_SS, USER_STACK_BYTES,
    USER_STACK_TOP, USER_STACK_VA,
};

/// `USER_CS`/`USER_SS` phải là selector **user** với `RPL = 3` — GDT thứ tự `planned` của
/// **KT-417c** (user data entry 3, user code entry 4).
#[test]
fn user_selectors_literal() {
    assert_eq!(USER_CS, 0x23, "user code = index 4 × 8 | RPL 3 (KT-417c)");
    assert_eq!(USER_SS, 0x1B, "user data = index 3 × 8 | RPL 3 (KT-417c)");
    assert_eq!(USER_CS, SELECTOR_USER_CODE_RPL3);
    assert_eq!(USER_SS, SELECTOR_USER_DATA_RPL3);
    assert_eq!(USER_CS & 3, 3, "RPL của CS phải là 3");
    assert_eq!(USER_SS & 3, 3, "RPL của SS phải là 3");
    assert_eq!(KERNEL_CS, 0x08);
    assert_eq!(KERNEL_DS, 0x10);
}

/// **Đột biến (i)** ở tầng dữ liệu: `cs` để ring 0 **không** được nhận là user.
///
/// Đây là điều kiện để handler từ chối "đã vào ring 3" khi `iretq` không hạ đặc quyền.
#[test]
fn kernel_cs_is_not_user() {
    assert!(is_user_cs(USER_CS as u64), "0x23 phải là user CS (KT-417c)");
    assert!(
        !is_user_cs(KERNEL_CS as u64),
        "0x08 KHÔNG được coi là user CS"
    );
    assert!(is_kernel_cs(KERNEL_CS as u64));
    assert!(!is_kernel_cs(USER_CS as u64));
    // RPL 3 nhưng **không** phải selector user code (ví dụ 0x13 = kernel data | RPL 3) cũng sai.
    assert!(!is_user_cs(0x13), "0x13 không phải user code");
    // **KT-417c** đảo chiều: `0x1B` **nay** là user **data** (entry 3) — không phải user code.
    assert!(
        !is_user_cs(0x1B),
        "0x1B là user data sau KT-417c, không phải user code"
    );
    assert!(is_user_cs(0x23), "0x23 là user code sau KT-417c");
}

/// `RFLAGS` của ring 3: `IF = 1` (bit 9) + bit 1 luôn 1; **không** đặt `IOPL`/`NT`/`TF`.
#[test]
fn user_rflags_literal() {
    assert_eq!(USER_RFLAGS, 0x202);
    assert!(rflags_if(USER_RFLAGS), "IF phải bật khi vào ring 3");
    assert_eq!(USER_RFLAGS & (1 << 1), 1 << 1, "bit 1 luôn 1");
    assert_eq!(USER_RFLAGS & (3 << 12), 0, "IOPL phải 0");
    assert_eq!(USER_RFLAGS & (1 << 14), 0, "NT phải 0");
    assert_eq!(USER_RFLAGS & (1 << 8), 0, "TF phải 0");
    assert!(!rflags_if(0x002), "IF=0 phải được nhận là tắt");
}

/// Hai VA của U-mode: căn 4 KiB, không đè nhau, **canonical nửa thấp** (bit 47 = 0, bit 63..48 = 0).
#[test]
fn user_vas_are_page_aligned_and_canonical() {
    assert_eq!(USER_CODE_VA & 0xFFF, 0, "VA mã ring 3 phải căn 4 KiB");
    assert_eq!(USER_STACK_VA & 0xFFF, 0, "VA stack ring 3 phải căn 4 KiB");
    assert_eq!(USER_STACK_BYTES, 0x1000, "stack user là một trang 4 KiB");
    assert_eq!(USER_STACK_TOP, USER_STACK_VA + USER_STACK_BYTES);
    assert_ne!(USER_CODE_VA, USER_STACK_VA, "mã và stack phải khác trang");
    assert!(
        USER_CODE_VA + 0x1000 <= USER_STACK_VA,
        "trang mã phải nằm dưới stack"
    );
    // Canonical nửa thấp: bit 47 = 0 và bit 63..48 = 0 (4-level paging, 48-bit VA).
    for va in [USER_CODE_VA, USER_STACK_VA, USER_STACK_TOP] {
        assert_eq!(va >> 47, 0, "VA 0x{va:x} phải ở nửa thấp");
    }
    // Ngoài identity map 1 GiB và ngoài MMIO map 3..4 GiB của ảnh — không đè phép đo cũ.
    assert!(
        USER_CODE_VA >= 0x1_0000_0000,
        "VA phải ngoài identity map 1 GiB"
    );
    assert!(
        !(0xC000_0000..0x1_0000_0000).contains(&USER_CODE_VA),
        "VA không được nằm trong MMIO map"
    );
}

/// `SyscallRet` là **hai** `u64` theo SysV AMD64: trả ở `rax` (error) và `rdx` (value).
#[test]
fn syscall_ret_layout() {
    assert_eq!(size_of::<SyscallRet>(), 16);
    assert_eq!(align_of::<SyscallRet>(), 8);
    let ret = SyscallRet {
        error: 0,
        value: 14,
    };
    assert_eq!(ret.error, 0);
    assert_eq!(ret.value, 14);
}

/// Bố cục frame của stub `int 0x80`: 15 GPR (120 B) rồi vector/mã lỗi/rip/cs/rflags/rsp/ss.
///
/// Đây là **hợp đồng** giữa stub asm (crate plat) và handler (ảnh): lệch một ô là handler đọc
/// `nr` từ chỗ của `rdi` — không lỗi biên dịch, chỉ sai khi đã chạy.
#[test]
fn syscall_frame_offsets_literal() {
    // Thứ tự `push` của stub: `rax` đẩy **đầu** ⇒ offset 112; `r15` đẩy **cuối** ⇒ offset 0.
    assert_eq!(SYSCALL_FRAME_RAX, 112);
    assert_eq!(SYSCALL_FRAME_RCX, 104);
    assert_eq!(SYSCALL_FRAME_RDX, 96);
    assert_eq!(SYSCALL_FRAME_RBX, 88);
    assert_eq!(SYSCALL_FRAME_RBP, 80);
    assert_eq!(SYSCALL_FRAME_RSI, 72);
    assert_eq!(SYSCALL_FRAME_RDI, 64);
    assert_eq!(SYSCALL_FRAME_R8, 56);
    assert_eq!(SYSCALL_FRAME_R9, 48);
    assert_eq!(SYSCALL_FRAME_R10, 40);
    assert_eq!(SYSCALL_FRAME_R11, 32);
    assert_eq!(SYSCALL_FRAME_R12, 24);
    assert_eq!(SYSCALL_FRAME_R13, 16);
    assert_eq!(SYSCALL_FRAME_R14, 8);
    assert_eq!(SYSCALL_FRAME_R15, 0);
    // Cùng offset với frame exception chung — một bố cục, hai đường vào.
    assert_eq!(SYSCALL_FRAME_VECTOR, FRAME_VECTOR_OFFSET);
    assert_eq!(SYSCALL_FRAME_ERROR, FRAME_ERROR_OFFSET);
    assert_eq!(SYSCALL_FRAME_RIP, FRAME_RIP_OFFSET);
    assert_eq!(SYSCALL_FRAME_CS, FRAME_CS_OFFSET);
    assert_eq!(SYSCALL_FRAME_RFLAGS, FRAME_RFLAGS_OFFSET);
    // `int 0x80` từ CPL3 đẩy thêm RSP/SS (5 qword) — hai ô cuối.
    assert_eq!(SYSCALL_FRAME_RFLAGS + 8, 160);
    assert_eq!(160 + 8, 168);
}

/// Vector `int 0x80` là **0x80** — không trùng exception (0..31), timer (32), spurious (255).
#[test]
fn syscall_vector_literal() {
    assert_eq!(VEC_SYSCALL, 0x80);
    assert!(VEC_SYSCALL > 31, "0x80 ngoài dải exception");
    assert_ne!(VEC_SYSCALL, 32, "không trùng timer");
    assert_ne!(VEC_SYSCALL, 255, "không trùng spurious");
}
