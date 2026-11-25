// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
#![forbid(unsafe_code)]
//! `vifast-plat-riscv64-logic` — **ngữ nghĩa** nền tảng riscv64, tách khỏi `vifast-plat-common`
//! (KT-409b, D2-259/D2-260 mục 3) để `plat-common` **0 từ khoá RISC-V**.
//!
//! Vì sao **không** dời thẳng vào `vifast-plat-riscv64`: crate đó chứa `asm!` RISC-V **không**
//! `cfg`-gate nên **không** biên dịch được trên host (`invalid register a7`), mà AGENTS §4.8
//! **cấm** thêm `cfg(target_arch)` vào kernel. Dời vào đó sẽ khiến 5 tệp test host của 5 module
//! **không còn chạy** — trái AGENTS §4.3. Vì vậy phần **ngữ nghĩa** ở đây (host-testable), phần
//! **asm/CSR/MMIO** ở `vifast-plat-riscv64` (D2-17).
//!
//! Crate **không** có `unsafe` ([`#![forbid(unsafe_code)]`]), **không** asm, **không** MMIO.
//!
//! - [`plic`] — hình học đăng ký PLIC, context `2*hart+1`, phân loại claim, bộ đếm (KT-48);
//! - [`trap`] — trapframe 32 thanh ghi + decode **mọi** `scause`, chính sách resume/fatal (KT-45);
//! - [`timer`] — hằng số `[TK]`, phân loại tick/deadline tuyệt đối, phát hiện mất tick (KT-47);
//! - [`tlb`] — ASID 8 bit + thế hệ, `FlushScope` ba dạng, `TlbFlushRequest` (KT-51);
//! - [`user`] — ngữ nghĩa U-mode/syscall: 3 syscall ABI, 19 mã lỗi, state "TCB-like" (KT-53).

pub mod plic;
pub mod timer;
pub mod tlb;
pub mod trap;
pub mod user;
