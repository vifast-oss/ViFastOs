// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho đường `syscall`/`sysret` x86_64 — **KT-417c**.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test syscall_fast
//! ```
//!
//! Kỳ vọng là **giá trị tuyệt đối viết tay** lấy từ Intel SDM, **không** lấy hằng của crate để
//! so với chính nó:
//!
//! * Vol 2B "SYSCALL": `CS ← STAR[47:32]`, `RIP ← LSTAR`, `R11 ← RFLAGS`, `RCX ← RIP kế tiếp`;
//! * Vol 2B "SYSRET": `CS ← STAR[63:48] + 16`, `SS ← STAR[63:48] + 8` (cộng cứng), rồi `| RPL 3`
//!   khi trả về CPL3;
//! * Vol 3A §2.5 "Control Registers"/Vol 4 `IA32_FMASK`: `RFLAGS &= ~FMASK` khi vào kernel;
//! * Vol 4 `IA32_EFER`: bit 0 = `SCE`, bit 8 = `LME`, bit 11 = `NXE`.
//!
//! Đây là các bất biến **không** cần QEMU: chúng bắt lỗi "đảo GDT mà vẫn sai" và "ghi `EFER`
//! bằng hằng số làm mất `NXE`" trước khi tốn một lượt boot.

#![allow(clippy::assertions_on_constants)]

use core::mem::{align_of, size_of};

use vifast_plat_x86_64::control::{EFER_LME, EFER_NXE, EFER_SCE};
use vifast_plat_x86_64::gdt::{
    INDEX_USER_CODE, INDEX_USER_DATA, RPL_USER, SELECTOR_KERNEL_CODE, SELECTOR_KERNEL_DATA,
    SELECTOR_USER_CODE, SELECTOR_USER_DATA,
};
use vifast_plat_x86_64::smp::MAX_HARTS;
use vifast_plat_x86_64::syscall_fast::{
    efer_ok, efer_with_sce, fmask_ok, fmask_value, gs_scratch, kernel_stack, set_kernel_stack,
    star_value, sysret_cs, sysret_ss, user_rsp, GS_OFF_RSP0, GS_OFF_USER_RSP, GS_SCRATCH_WORDS,
    RFLAGS_AC, RFLAGS_DF, RFLAGS_IF, RFLAGS_NT, RFLAGS_TF, SYSCALL_FAST_VEC,
};
use vifast_plat_x86_64::umode::{USER_CS, USER_SS};

/// **Thứ tự `planned`** (D2-242 nhóm 5): user **data** đứng ngay **trước** user code — điều kiện
/// duy nhất để công thức cứng của `SYSRET` trỏ đúng cặp selector.
#[test]
fn gdt_planned_order_is_what_sysret_needs() {
    // Giá trị tuyệt đối, không lấy từ hằng của crate.
    assert_eq!(INDEX_USER_DATA, 3, "user data = entry 3");
    assert_eq!(INDEX_USER_CODE, 4, "user code = entry 4");
    assert!(
        INDEX_USER_CODE == INDEX_USER_DATA + 1,
        "SYSRET cộng cứng +16/+8 ⇒ user code phải là entry NGAY SAU user data"
    );
    assert_eq!(SELECTOR_USER_DATA, 0x18);
    assert_eq!(SELECTOR_USER_CODE, 0x20);
    assert_eq!(SELECTOR_KERNEL_CODE, 0x08);
    assert_eq!(SELECTOR_KERNEL_DATA, 0x10);
    assert_eq!(RPL_USER, 3);
}

/// `STAR` phải là `[63:48] = 0x10` (kernel data), `[47:32] = 0x08` (kernel code) — hai trường
/// **còn lại** của `STAR` (`[31:0]`, `[15:0]` của `SYSCALL` 32-bit) không dùng ở long mode.
#[test]
fn star_literal_geometry() {
    assert_eq!(
        star_value(),
        0x0010_0008_0000_0000,
        "STAR = (kernel_data << 48) | (kernel_code << 32)"
    );
    assert_eq!((star_value() >> 48) & 0xFFFF, 0x10);
    assert_eq!((star_value() >> 32) & 0xFFFF, 0x08);
}

/// Công thức **cứng** của `SYSRET`, áp lên `STAR` **của chính crate** ⇒ phải ra đúng selector
/// ring 3 mà `# umode: iretq` công bố. Đây là ca bắt lỗi "đảo GDT rồi vẫn lệch".
#[test]
fn sysret_formula_matches_user_selectors() {
    let base = (star_value() >> 48) & 0xFFFF;
    assert_eq!(base + 16, 0x20, "SYSRET: CS = STAR[63:48] + 16");
    assert_eq!(base + 8, 0x18, "SYSRET: SS = STAR[63:48] + 8");
    assert_eq!(sysret_cs(), 0x23, "CS thật = 0x20 | RPL 3");
    assert_eq!(sysret_ss(), 0x1B, "SS thật = 0x18 | RPL 3");
    assert_eq!(sysret_cs(), USER_CS, "khớp USER_CS của # umode: iretq");
    assert_eq!(sysret_ss(), USER_SS, "khớp USER_SS");
    assert_eq!(sysret_cs() & 3, 3, "RPL 3");
    assert_eq!(sysret_ss() & 3, 3, "RPL 3");
    // Nghịch đảo: nếu ai đảo GDT về hình học CŨ thì công thức **không** còn khớp USER_CS — test
    // này phải đỏ, không được im lặng.
    assert_ne!(sysret_cs(), 0x1B, "0x1B là user data sau KT-417c");
}

/// `FMASK` **phải** xoá `AC` (SMAP), `IF` (ngắt tắt khi vào kernel), `TF` (không single-step),
/// `DF` (hướng chuỗi), `NT` (không task-switch nối). `AC` là bit **bắt buộc** — thiếu nó thì
/// ring 3 đặt `AC = 1` là vô hiệu hoá mọi `stac`/`clac`.
#[test]
fn fmask_must_clear_ac() {
    let mask = fmask_value();
    assert_eq!(mask & RFLAGS_AC, RFLAGS_AC, "FMASK phải có bit AC (18)");
    assert_eq!(RFLAGS_AC, 1 << 18);
    assert_eq!(mask & RFLAGS_IF, RFLAGS_IF, "FMASK phải có IF");
    assert_eq!(mask & RFLAGS_TF, RFLAGS_TF, "FMASK phải có TF");
    assert_eq!(mask & RFLAGS_DF, RFLAGS_DF, "FMASK phải có DF");
    assert_eq!(mask & RFLAGS_NT, RFLAGS_NT, "FMASK phải có NT");
    assert!(fmask_ok(mask), "fmask_ok phải chấp nhận giá trị đúng");
    // Ca âm: thiếu AC ⇒ `fmask_ok` sai (chứng minh hàm kiểm có răng).
    assert!(!fmask_ok(mask & !RFLAGS_AC));
    assert!(!fmask_ok(0));
}

/// `EFER` phải **đọc-sửa-ghi**: bật `SCE` mà **giữ** `LME` (bit 8) và `NXE` (bit 11).
///
/// Ghi `mov efer, imm` làm mất `NXE` ⇒ mọi PTE có bit 63 thành reserved ⇒ `wx_probe`/`#PF` đỏ.
#[test]
fn efer_write_keeps_lme_and_nxe() {
    assert_eq!(EFER_SCE, 1);
    assert_eq!(EFER_LME, 1 << 8);
    assert_eq!(EFER_NXE, 1 << 11);

    // Trạng thái thật của ảnh trước KT-417c: `LME | NXE` = 0x900, `SCE = 0`.
    let before = EFER_LME | EFER_NXE;
    let after = efer_with_sce(before);
    assert_eq!(after, 0x901, "0x900 | SCE(1)");
    assert_eq!(after & EFER_LME, EFER_LME, "LME phải còn");
    assert_eq!(after & EFER_NXE, EFER_NXE, "NXE phải còn");
    assert_eq!(after & EFER_SCE, EFER_SCE, "SCE phải bật");
    assert!(efer_ok(after));

    // `LMA` (bit 10, chỉ đọc) không được bị hàm này xoá.
    let with_lma = before | (1 << 10);
    assert_eq!(efer_with_sce(with_lma) & (1 << 10), 1 << 10);

    // Ca âm: mất NXE hoặc mất SCE ⇒ `efer_ok` sai.
    assert!(!efer_ok(after & !EFER_NXE), "mất NXE phải bị từ chối");
    assert!(!efer_ok(after & !EFER_SCE), "mất SCE phải bị từ chối");
    assert!(!efer_ok(0));
}

/// GS scratch: **mỗi hart một vùng riêng**, 8-byte aligned, đủ chỗ cho hai ô (`rsp0` + `RSP`
/// user). `swapgs` đưa `GS.base` về đây; stub đọc `gs:[0]`/`gs:[8]` nên offset là hợp đồng.
#[test]
fn gs_scratch_per_hart_distinct_and_aligned() {
    assert_eq!(GS_OFF_RSP0, 0);
    assert_eq!(GS_OFF_USER_RSP, 8);
    assert!(GS_SCRATCH_WORDS * 8 >= 16, "phải đủ hai ô u64");
    assert_eq!(size_of::<u64>(), 8);
    assert!(align_of::<u64>() >= 8, "atomic u64 phải 8-byte aligned");

    let mut seen = [0u64; MAX_HARTS];
    for (hart, slot) in seen.iter_mut().enumerate() {
        *slot = gs_scratch(hart);
        assert_ne!(*slot, 0, "hart {hart} phải có vùng scratch");
        assert_eq!(
            *slot % 8,
            0,
            "hart {hart}: vùng scratch phải 8-byte aligned"
        );
    }
    // Rời nhau **và** cách nhau đủ một dòng (`GS_SCRATCH_WORDS` u64) — không hart nào ghi vào
    // vùng của hart khác.
    for hart in 0..MAX_HARTS {
        for other in (hart + 1)..MAX_HARTS {
            let d = seen[other].abs_diff(seen[hart]);
            assert!(
                d >= (GS_SCRATCH_WORDS * 8) as u64,
                "hart {hart}/hart {other} chồng vùng scratch (Δ={d})"
            );
        }
    }
}

/// `rsp0` (stack kernel cho `syscall`) là **per-hart**: ghi hart này không đổi hart khác, đọc lại
/// đúng giá trị đã ghi. Đây là bất biến mà `int 0x80` lấy từ `TSS.RSP0` nhưng `syscall` **không**
/// có — thiếu nó là kernel chạy trên stack user.
#[test]
fn kernel_stack_is_per_hart() {
    // `kernel_stack` là trạng thái **toàn cục** của crate; test này sở hữu hai slot đầu và ghi
    // giá trị đánh dấu riêng để không phụ thuộc thứ tự chạy của test khác.
    set_kernel_stack(0, 0xAAAA_0000);
    set_kernel_stack(1, 0xBBBB_0000);
    assert_eq!(kernel_stack(0), 0xAAAA_0000);
    assert_eq!(kernel_stack(1), 0xBBBB_0000);
    assert_ne!(
        kernel_stack(0),
        kernel_stack(1),
        "hai hart phải có hai stack khác nhau"
    );
    // Ngoài `MAX_HARTS` ⇒ 0 (không panic, không đọc rác).
    assert_eq!(kernel_stack(MAX_HARTS), 0);
    assert_eq!(user_rsp(MAX_HARTS), 0);
    // `user_rsp` khởi đầu bằng 0 (chưa lần nào vào bằng `syscall`).
    assert_eq!(user_rsp(0), 0, "chưa vào bằng syscall thì RSP user phải 0");
}

/// Dấu đường vào `SYSCALL_FAST_VEC` **không** được trùng một vector hợp lệ (0..255) — nếu trùng,
/// handler sẽ nhận nhầm `int 0x80`/exception thành `syscall`.
#[test]
fn fast_vec_marker_is_not_a_real_vector() {
    assert_eq!(SYSCALL_FAST_VEC, 0x100);
    assert!(SYSCALL_FAST_VEC > 0xFF, "vector x86 chỉ 0..255");
    assert_ne!(SYSCALL_FAST_VEC, 0x80, "0x80 là int 0x80");
}
