// SPDX-License-Identifier: MIT OR Apache-2.0
//! Platform riscv64 cho QEMU `virt` — mốc **K3** (KT-44, mở rộng ở KT-45).
//!
//! Phạm vi: entry `_start` (S-mode, OpenSBI `fw_dynamic` nạp trực tiếp tại `0x8020_0000`), MMIO
//! UART 16550 (`debug_put_str`), SBI SRST (`debug_halt`), cài đặt `Platform` trên phần cứng ảo
//! (`rdtime`, `tp`, `stvec`) và **khung trap direct-mode** của KT-45:
//!
//! - `trap_entry` (asm): đổi sang **trap stack** qua `sscratch`, lưu **32 thanh ghi** + `sepc/
//!   sstatus/scause/stval` vào [`TrapFrame`], gọi [`trap_dispatch`], khôi phục rồi `sret`.
//! - [`trap_dispatch`]: decode `scause`, resume exception recoverable (cộng `sepc` đúng cỡ
//!   instruction 2/4 byte), dump + `DebugHalt(1)` cho fatal/interrupt.
//! - Các probe KT-45 (`probe_*`): TAP phần cứng cho `breakpoint` và `ecall-S` (qua SBI), trap
//!   **tiêm** cho `illegal` — D2-73: OpenSBI v1.8.1 `delegate_traps()` đặt `MEDELEG = 0xf4b509`
//!   (bit 2 illegal và bit 9 ecall-S **không** delegate), nên hai loại đó không vào `stvec`.
//!
//! Trap/timer/PLIC/MMU/SMP **không** thuộc KT-45: MMU (KT-46), timer (KT-47, module [`timer`] —
//! SBI TIME + `sie.STIE` + `rdtime`), PLIC (KT-48, module [`plic`]), SMP (KT-49), U-mode/syscall
//! (KT-53).
//!
//! KT-47: `trap_dispatch` có nhánh [`Event::Timer`] (bit 63 + mã 5) — đếm tick, nạp deadline kế,
//! **resume** bằng `sret`; `sstatus.SIE` chỉ được bật ở các hàm `enable_*` của arch
//! ([`timer::enable_timer`], [`plic::enable_external`]) — P2a: không ghi `sstatus` trong trap
//! entry/handler.
//!
//! KT-48: ngắt external (bit 63 + mã 9) đi vào PLIC **claim → xử lý → complete** trong
//! [`trap_dispatch`] (module [`plic`]): UART0 IRQ 10 bật `IER.TX-empty`, handler đếm và tắt sau N
//! ngắt; line lạ được đếm + complete để không treo; `Platform::enable_irq`/`disable_irq` giờ là
//! PLIC thật (đọc lại bit enable).
//!
//! KT-49 mở module [`smp`] (SBI HSM + PerHart): `_start` và `_start_secondary` tính `sp`/
//! `sscratch` từ **record per-hart** (`__hart_area_base + hart*HART_STRIDE`), `trap_entry` đổi
//! sang `.trap_stack` **của chính hart đang chạy** (tính từ `sp` frame, không dùng biến toàn cục
//! một hart) và phát hiện trap lồng theo địa chỉ trap stack per-hart; `trap_dispatch` ghi
//! `hart_id()`/trapframe vào bộ trạng thái SMP để TAP chứng minh `TP`/`sscratch` per-hart.
//!
//! KT-50 mở module [`ipi`] (IPI thật): SBI `send_ipi` (EID `0x735049`) + ACLINT MSWI, `post` vào
//! **hộp thư per-hart** (coalesce theo kind `TlbFlush`/`VmKick`/`Wake`), `post` **self-target**
//! đi đường local (không IPI — D2-68). `trap_dispatch` có nhánh [`Event::Ipi`] (`scause = (1<<63)
//! | 1`, **không** fatal, **không** nhầm timer 5/external 9): xoá MSIP rồi drain hộp thư.
//! [`wait_drain`] = vòng chờ **drain trước mỗi bước** + timeout (`rdtime`) nên chờ ack không
//! deadlock và hart chết chỉ làm TAP đỏ. `park_forever` drain trong lúc park.
//!
//! Địa chỉ UART/PLIC/CLINT là **dữ kiện nền tảng QEMU virt** (D2-72 Q7), không phải hằng số thiết
//! kế `[TK]`; `spec/kernel.toml` **không** mở ở K3 (D2-72 Q1) nên chúng nằm ở đây và ở
//! `v2/docs/k3-riscv64.md`.
//!
//! Crate này **có `unsafe`** vì MMIO và asm không viết "safe" được; nằm trong danh sách đóng
//! `kernel/vifast-plat-*` của D2-17. Mọi phần **logic** dùng chung ở `vifast-plat-common`
//! (`#![forbid(unsafe_code)]`) — KT-45 giữ nguyên nguyên tắc này: trapframe/decode/chính sách ở
//! `plat-common`, chỉ asm/CSR ở đây.
#![no_std]
#![allow(unsafe_code)] // crate thuộc danh sách đóng của D2-17 (kernel/vifast-plat-*)

use core::arch::{asm, global_asm};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64_logic::trap::{
    decode_trap, handle_trap, report_fatal, report_guard_fault, report_interrupt, tf_reg_offset,
    Event, ExceptionKind, TrapAction, TrapFrame, GUARD_STATS, INTERRUPT_SUPERVISOR_EXTERNAL,
    INTERRUPT_SUPERVISOR_SOFTWARE, INTERRUPT_SUPERVISOR_TIMER, TF_SCAUSE_OFFSET, TF_SEPC_OFFSET,
    TF_SIZE, TF_SSTATUS_OFFSET, TF_STVAL_OFFSET, TRAP_STATS,
};

pub mod bootinfo;
pub mod core_bridge;
/// **KT-418** — ngữ cảnh FPU/SIMD **eager theo TCB** (`fs0..fs31` + `fcsr`, `sstatus.FS`): vùng
/// lưu 272 B **ngoài** `TcbCore`, lưu khi phần cứng báo bẩn, khôi phục khi khe hợp lệ.
pub mod fpu;
/// KT-69: danh sách trang của chương trình U-mode + `LoadedInit::release` (tách khỏi `user` cho R1).
pub mod init_pages;
/// KT-67b: đọc header IPC buffer của U-mode (chỉ ảnh có `user`; tách khỏi `user` cho R1).
#[cfg(feature = "user")]
pub mod ipc_buffer;
pub mod ipi;
/// **KT-336c** (D2-243): khoá MCS bọc lưu/tắt `sstatus.SIE` — thay spinlock tự chế
/// `core_bridge::LOCKED` (cùng khuôn `KT-336a` cho `x86_64`, khác CSR).
pub mod irq_lock;
/// **KT-411** — adapter `vifast_ktest::CoreBridge` cho ảnh kiểm (chỉ feature `ktest`).
#[cfg(feature = "ktest")]
pub mod ktest_bridge;
// KT-67: cầu nối D2-83 (core giữ cap ↔ object layer giữ Frame/PA) cho memsrv.
/// **KT-74** `exec(path)` — nạp ELF mới **thay** ảnh hiện tại (giữ pid/cspace, giải phóng
/// VSpace+Frame cũ, nguyên tử). Chỉ ảnh có U-mode (`feature = "exec"` ⇒ `user`).
#[cfg(feature = "exec")]
pub mod exec;
pub mod mem_bridge;
pub mod mmu;
pub mod obj;
/// KT-70: trạng thái fault VM giao pager server U-mode (tách khỏi `user.rs`, luật R1).
pub mod pager_fault;
/// **KT-72** PCI ECAM + gán BAR (`virtio-blk`) — **chỉ ảnh test** (`feature = "ktest"`): ảnh
/// sản phẩm chưa mở K6-driver nên giữ nguyên `.text` (KT-73 mở cho sản phẩm).
/// KT-73: console **dùng lại** module này (`init_for(VirtioKind::Console)`) — một đường quét
/// ECAM/BAR/cap cho mọi thiết bị virtio.
#[cfg(any(
    feature = "vblk",
    feature = "vcon",
    feature = "block",
    feature = "net",
    feature = "input",
    feature = "gpu"
))]
pub mod pci;
pub mod plic;
/// KT-56: probe trap **thật** cho `FaultKind` (chỉ ảnh test — `#[cfg(feature = "ktest")]`).
#[cfg(feature = "ktest")]
pub mod probe;
pub mod smp;
/// **KT-268** TCP client trên virtio-net (N1 phần 2) — mở/gửi/nhận/đóng một kết nối, timeout 3 s
/// mỗi bước, retransmit tối đa 3 lần. Máy trạng thái thuần ở `vifast-net::tcp`; tệp này chỉ dựng
/// frame + chờ segment + thi hành hành động. Chỉ ảnh test `ktest-net` (`feature = "net"`).
#[cfg(feature = "net")]
pub mod tcp;
pub mod tf_store;
pub mod timer;
pub mod tlb;
/// **KT-409b**: phần **ngữ nghĩa** (PLIC/trap/timer/TLB/U-mode) nay ở crate host-testable
/// `vifast-plat-riscv64-logic`; tái xuất để mọi đường dẫn `vifast_plat_riscv64::{trap,timer,…}`
/// giữ nguyên. Crate arch chỉ còn asm/CSR/MMIO (D2-17).
pub use vifast_plat_riscv64_logic::trap;
/// **KT-73** UART RX IRQ → ring của KT-69 → U-mode đọc byte thật (`feature = "user"` — chỉ ảnh
/// có chương trình U-mode; ảnh sản phẩm không có người nhận nên không link module này).
#[cfg(feature = "user")]
pub mod uart_console;
#[cfg(feature = "user")]
pub mod user;
/// **KT-74** bảng dải VA user theo ô tác vụ — tách khỏi `user` cho R1 (ngữ nghĩa không đổi).
#[cfg(feature = "user")]
pub mod user_ranges;
/// **KT-74** TLS của U-mode (`Tcb.SetTls`) + trapframe mới cho `exec` — tách khỏi `user` cho R1.
#[cfg(feature = "user")]
pub mod user_run;
/// KT-66: stack dispatch syscall per-hart + canary/nước thấp (tách khỏi `user` cho R1).
#[cfg(feature = "user")]
pub mod user_stack;
/// **KT-74** (như trên): `apply_tls`/`exec_frame`/`restore_kernel_tp`/`tls_observed`.
#[cfg(feature = "user")]
pub mod user_tls;
/// **KT-72** driver virtio-blk thật (virtqueue + read/write/flush) — chỉ ảnh test.
#[cfg(any(feature = "vblk", feature = "block"))]
pub mod virtio_blk;
/// **KT-73** driver virtio-console (`0x1003`) — hai virtqueue rx/tx, dùng lại `pci.rs` của KT-72
/// (không nhân đôi đường ECAM/BAR). Chỉ ảnh test `ktest-con` (`feature = "vcon"`).
#[cfg(feature = "vcon")]
pub mod virtio_console;
/// **KT-263** driver virtio-gpu 2D (`0x1050`/`0x1010`) — một virtqueue `controlq`, chuỗi lệnh
/// `GET_DISPLAY_INFO` → `RESOURCE_CREATE_2D` → `RESOURCE_ATTACH_BACKING` → `SET_SCANOUT` →
/// `TRANSFER_TO_HOST_2D` → `RESOURCE_FLUSH`, framebuffer 640×480 ở dải PA riêng; dùng lại `pci.rs`
/// của KT-72 (không nhân đôi đường ECAM/BAR). Chỉ ảnh test `ktest-gpu` (`feature = "gpu"`); ảnh
/// sản phẩm **không** bật (D2-203: chưa có người nhận màn hình ở U-mode).
#[cfg(feature = "gpu")]
pub mod virtio_gpu;
/// **KT-255** driver virtio-input (`0x1012`, bàn phím) — một virtqueue `event`, đọc
/// `virtio_input_config` (tên/danh tính/`EV_BITS`), dùng lại `pci.rs` của KT-72 (không nhân đôi
/// đường ECAM/BAR). Chỉ ảnh test `ktest-input` (`feature = "input"`); ảnh sản phẩm **không** bật
/// (D2-200: chưa có người nhận bàn phím ở U-mode).
#[cfg(feature = "input")]
pub mod virtio_input;
/// **KT-245** driver virtio-net (`0x1000`) — hai virtqueue rx/tx, MAC thật từ `virtio_net_config`,
/// dùng lại `pci.rs` của KT-72 (không nhân đôi đường ECAM/BAR). Chỉ ảnh test `ktest-net`
/// (`feature = "net"`); ảnh sản phẩm **không** bật (D2-196: chưa có net server U-mode).
#[cfg(feature = "net")]
pub mod virtio_net;

// **KT-81**: địa chỉ nền tảng **dẫn xuất** từ bảng một nguồn
// [`vifast_plat_common::resources::REGIONS`] — không còn hằng số viết tay thứ hai để lệch.
// Bảng ấy cũng có phép kiểm **chồng lấn** (`first_overlap`) chạy trong test.
/// UART0 của QEMU `virt` (16550A) — dữ kiện nền tảng, D2-72 Q7.
pub const UART0_BASE: usize = vifast_plat_common::resources::UART0.base as usize;
/// Dòng IRQ của UART0 trên PLIC của `virt` (D2-72 Q7).
pub const UART0_IRQ: u32 = 10;
/// PLIC cổ điển của `virt` (D2-72 Q7/Q8); `ndev` đọc từ register, không giả định.
pub const PLIC_BASE: usize = vifast_plat_common::resources::PLIC.base as usize;
/// CLINT/ACLINT của `virt` (`aclint=on`; D2-72 Q7).
pub const CLINT_BASE: usize = vifast_plat_common::resources::CLINT.base as usize;
/// Trang test của `virt` (`test@100000`; D2-72 Q7).
pub const TEST_PAGE_BASE: usize = vifast_plat_common::resources::TEST_PAGE.base as usize;
/// Timebase QEMU `virt`: 10 MHz (DTB `timebase-frequency`; D2-72 Q6) — nguồn duy nhất ở
/// [`vifast_plat_riscv64_logic::timer::TIMEBASE_HZ`] (KT-47), đây chỉ là tên ngắn cho code arch.
pub const TIME_HZ: u64 = vifast_plat_riscv64_logic::timer::TIMEBASE_HZ;

// Entry point: OpenSBI nhảy vào S-mode với `a0 = hartid`, `a1 = địa chỉ DTB` (SBI spec).
// Giữ hartid ở `tp` để `Riscv64Virt::hart_id` đọc lại mà không cần `static mut`; đặt `sp` từ
// linker script (`.stack` NOLOAD), đặt `sscratch = __trap_stack_top` để **mọi** trap đổi sang
// trap stack, rồi vào `rust_main` do crate ảnh cung cấp.
//
// `trap_entry` là **direct mode** (`stvec.MODE = 0`), 4-byte aligned. Bố cục trapframe lấy từ
// `vifast-plat-common::trap` bằng `const` operand — không hardcode offset trong asm (rủi ro
// "sai offset trapframe" của K3-PLAN §4.1), `const` assert ở `trap.rs` chặn lệch bố cục.
global_asm!(
    ".section .text.entry",
    ".globl _start",
    "_start:",
    "mv tp, a0", // hartid (a0) — OpenSBI truyền; `a1` là DTB (KT-45 trở đi mới dùng)
    // KT-49/KT-66: stack/sscratch tính từ **record per-hart** (không dùng __stack_top toàn cục):
    // record của hart h ở `__hart_area_base + h*HART_STRIDE`, gồm guard | stack | trap stack |
    // syscall stack. Dùng phép dịch/cộng (không cần M extension): h*0x1D000 =
    // h<<16 + h<<15 + h<<14 + h<<12 (KT-66: trước là 0x15000 = h<<16 + h<<14 + h<<12).
    "la t0, __hart_area_base",
    "slli t2, a0, 16",
    "slli t3, a0, 15",
    "add t2, t2, t3",
    "slli t3, a0, 14",
    "add t2, t2, t3",
    "slli t3, a0, 12",
    "add t2, t2, t3",
    "add t0, t0, t2",
    "li t1, 0x11000", // guard 4 KiB + stack 64 KiB
    "add sp, t0, t1", // đỉnh stack của hart này
    "li t1, 0x15000", // + trap stack 16 KiB (offset KHÔNG đổi; sau nó là syscall stack)
    "add t0, t0, t1",
    "csrw sscratch, t0", // đỉnh trap stack của hart này
    // KT-45 P4: zero `.bss` tường minh (không dựa vào "QEMU/RAM tình cờ bằng 0"). Vòng zero chỉ
    // dùng t0/t1, không đụng a0 (hartid)/a1 (DTB). `.bss` luôn 8-byte aligned theo linker script.
    "la t0, __bss_start",
    "la t1, __bss_end",
    "1:",
    "bgeu t0, t1, 2f",
    "sd zero, 0(t0)",
    "addi t0, t0, 8",
    "j 1b",
    "2:",
    // KT-49: giữ DTB (a1) cho `smp::init` kiểm magic (D2-72 Q7) — lưu SAU khi zero `.bss` để
    // không bị xoá (SMP_DTB nằm trong `.bss`).
    "la t0, SMP_DTB",
    "sd a1, 0(t0)",
    "call rust_main",
    "j .",
    // ------------------------------------------------------- entry hart thứ cấp (KT-49, SBI HSM)
    // SBI HSM gọi `start_addr` ở S-mode với `a0 = hartid`, `a1 = opaque` (SBI spec §8.1), satp=0.
    // Không zero `.bss` (BSP đã làm), không đụng a0/a1 trước khi gọi Rust.
    ".balign 4",
    ".globl _start_secondary",
    "_start_secondary:",
    "mv tp, a0",
    "la t0, __hart_area_base",
    "slli t2, a0, 16",
    "slli t3, a0, 15",
    "add t2, t2, t3",
    "slli t3, a0, 14",
    "add t2, t2, t3",
    "slli t3, a0, 12",
    "add t2, t2, t3",
    "add t0, t0, t2",
    "li t1, 0x11000",
    "add sp, t0, t1",
    "li t1, 0x15000",
    "add t0, t0, t1",
    "csrw sscratch, t0",
    "call rust_secondary_main",
    "j .",
    // ------------------------------------------------------------------ trap entry (KT-45)
    ".balign 4",
    ".globl trap_entry",
    "trap_entry:",
    // sp = trap stack top, sscratch = sp bị ngắt. Cấp phát trapframe 288 B NGAY TRONG
    // `.trap_stack` (`addi sp, sp, -TF_SIZE`; 288 chia hết 16 nên ABI vẫn aligned) — thiếu bước
    // này thì frame ghi NGOÀI vùng stack (và ngoài `filesz` của LOAD) dù QEMU RAM phẳng che mất.
    // Lưu x5 (t0) TRƯỚC khi dùng nó.
    "csrrw sp, sscratch, sp",
    "addi sp, sp, -{tf_size}",
    "sd zero, {x0}(sp)",
    "sd x1, {x1}(sp)",
    "sd x3, {x3}(sp)",
    "sd x4, {x4}(sp)",
    "sd x5, {x5}(sp)",
    "csrr t0, sscratch",
    "sd t0, {x2}(sp)",
    // KT-49: sscratch = đỉnh trap stack của **hart này** = frame + TF_SIZE (không global).
    "addi t0, sp, {tf_size}",
    "csrw sscratch, t0",
    "sd x6, {x6}(sp)",
    "sd x7, {x7}(sp)",
    "sd x8, {x8}(sp)",
    "sd x9, {x9}(sp)",
    "sd x10, {x10}(sp)",
    "sd x11, {x11}(sp)",
    "sd x12, {x12}(sp)",
    "sd x13, {x13}(sp)",
    "sd x14, {x14}(sp)",
    "sd x15, {x15}(sp)",
    "sd x16, {x16}(sp)",
    "sd x17, {x17}(sp)",
    "sd x18, {x18}(sp)",
    "sd x19, {x19}(sp)",
    "sd x20, {x20}(sp)",
    "sd x21, {x21}(sp)",
    "sd x22, {x22}(sp)",
    "sd x23, {x23}(sp)",
    "sd x24, {x24}(sp)",
    "sd x25, {x25}(sp)",
    "sd x26, {x26}(sp)",
    "sd x27, {x27}(sp)",
    "sd x28, {x28}(sp)",
    "sd x29, {x29}(sp)",
    "sd x30, {x30}(sp)",
    "sd x31, {x31}(sp)",
    // KT-45 P2b: phát hiện trap LỒNG (trap xảy ra khi đang chạy trên trap stack). Đọc sp bị ngắt
    // từ **frame đã lưu** (`{x2}(sp)`) — KHÔNG đọc `sscratch` vì nó đã được đặt lại thành
    // `__trap_stack_top` ở trên. Nếu sp đó nằm trong [__trap_stack_bottom, __trap_stack_top) thì
    // đây là trap trong handler ⇒ **fatal** (PANIC + halt), không khôi phục frame cũ. Frame lồng
    // đè đúng slot frame ngoài (`top-288`) — vô hại vì halt ngay, không resume. t0/t1 đã lưu ở
    // trên nên dùng tự do. Normal: sp bị ngắt ≤ __stack_top = __trap_stack_bottom.
    "ld t0, {x2}(sp)",
    // KT-49: biên trap stack **per-hart** = trap_top - HART_TRAP_STACK_BYTES, với trap_top suy
    // từ sp frame (`sp + TF_SIZE`) — không dùng symbol toàn cục của hart 0.
    "addi t1, sp, {tf_size}",
    "li t2, {trap_bytes}",
    "sub t1, t1, t2",
    "bgeu t1, t0, .Ltrap_normal", // old_sp <= bottom ⇒ trap từ stack thường, bình thường
    "addi t1, sp, {tf_size}",
    "bltu t0, t1, .Ltrap_nested", // bottom < old_sp < top ⇒ trap lồng trong handler
    ".Ltrap_normal:",
    "csrr t0, sepc",
    "sd t0, {sepc}(sp)",
    "csrr t0, sstatus",
    "sd t0, {sstatus}(sp)",
    "csrr t0, scause",
    "sd t0, {scause}(sp)",
    "csrr t0, stval",
    "sd t0, {stval}(sp)",
    "mv a0, sp",
    "call trap_dispatch",
    "j .Ltrap_resume",
    ".Ltrap_nested:",
    "call nested_trap_halt", // P2b: không quay lại
    "j .Ltrap_nested",
    ".Ltrap_resume:",
    // Khôi phục: CSR trước, thanh ghi sau, `sp` CUỐI CÙNG (đang dùng sp làm base).
    // P2a: **không** ghi `sstatus` — `sret` tự khôi phục SIE từ SPIE và privilege từ SPP; ghi
    // sstatus sớm có thể bật interrupt khi frame/handler chưa thoát (KT-47). KHÔNG ghi đè
    // sstatus cũng tránh làm hỏng trạng thái mà `sret` cần.
    "ld t0, {sepc}(sp)",
    "csrw sepc, t0",
    "addi t0, sp, {tf_size}", // KT-49: trả sscratch về đỉnh trap stack của hart này
    "csrw sscratch, t0",
    "ld x1, {x1}(sp)",
    "ld x3, {x3}(sp)",
    "ld x4, {x4}(sp)",
    "ld x6, {x6}(sp)",
    "ld x7, {x7}(sp)",
    "ld x8, {x8}(sp)",
    "ld x9, {x9}(sp)",
    "ld x10, {x10}(sp)",
    "ld x11, {x11}(sp)",
    "ld x12, {x12}(sp)",
    "ld x13, {x13}(sp)",
    "ld x14, {x14}(sp)",
    "ld x15, {x15}(sp)",
    "ld x16, {x16}(sp)",
    "ld x17, {x17}(sp)",
    "ld x18, {x18}(sp)",
    "ld x19, {x19}(sp)",
    "ld x20, {x20}(sp)",
    "ld x21, {x21}(sp)",
    "ld x22, {x22}(sp)",
    "ld x23, {x23}(sp)",
    "ld x24, {x24}(sp)",
    "ld x25, {x25}(sp)",
    "ld x26, {x26}(sp)",
    "ld x27, {x27}(sp)",
    "ld x28, {x28}(sp)",
    "ld x29, {x29}(sp)",
    "ld x30, {x30}(sp)",
    "ld x31, {x31}(sp)",
    "ld x5, {x5}(sp)",
    "ld sp, {x2}(sp)",
    "sret",
    x0 = const tf_reg_offset(0),
    x1 = const tf_reg_offset(1),
    x2 = const tf_reg_offset(2),
    x3 = const tf_reg_offset(3),
    x4 = const tf_reg_offset(4),
    x5 = const tf_reg_offset(5),
    x6 = const tf_reg_offset(6),
    x7 = const tf_reg_offset(7),
    x8 = const tf_reg_offset(8),
    x9 = const tf_reg_offset(9),
    x10 = const tf_reg_offset(10),
    x11 = const tf_reg_offset(11),
    x12 = const tf_reg_offset(12),
    x13 = const tf_reg_offset(13),
    x14 = const tf_reg_offset(14),
    x15 = const tf_reg_offset(15),
    x16 = const tf_reg_offset(16),
    x17 = const tf_reg_offset(17),
    x18 = const tf_reg_offset(18),
    x19 = const tf_reg_offset(19),
    x20 = const tf_reg_offset(20),
    x21 = const tf_reg_offset(21),
    x22 = const tf_reg_offset(22),
    x23 = const tf_reg_offset(23),
    x24 = const tf_reg_offset(24),
    x25 = const tf_reg_offset(25),
    x26 = const tf_reg_offset(26),
    x27 = const tf_reg_offset(27),
    x28 = const tf_reg_offset(28),
    x29 = const tf_reg_offset(29),
    x30 = const tf_reg_offset(30),
    x31 = const tf_reg_offset(31),
    sepc = const TF_SEPC_OFFSET,
    sstatus = const TF_SSTATUS_OFFSET,
    scause = const TF_SCAUSE_OFFSET,
    stval = const TF_STVAL_OFFSET,
    tf_size = const TF_SIZE,
    trap_bytes = const vifast_plat_common::smp::HART_TRAP_STACK_BYTES,
);

extern "C" {
    fn trap_entry();
}

/// Địa chỉ vector trap (`trap_entry`) — dùng cho [`Platform::set_trap_vector`].
pub fn trap_vector() -> usize {
    trap_entry as *const () as usize
}

/// Cài vector trap direct-mode vào `stvec` (tiện cho ảnh kernel).
pub fn install_trap_vector() {
    Riscv64Virt.set_trap_vector(trap_vector());
}

/// Đọc lại `stvec` (4 byte đầu) — ktest xác nhận đã cài đúng direct-mode.
pub fn read_trap_vector() -> usize {
    let value: usize;
    // SAFETY: chỉ đọc CSR `stvec`, không tác dụng phụ.
    unsafe { asm!("csrr {}, stvec", out(reg) value, options(nomem, nostack)) };
    value
}

/// Handler C mà asm `trap_entry` gọi sau khi đã lưu [`TrapFrame`] trên trap stack.
///
/// - Exception recoverable (`breakpoint`, `illegal`, `ecall`) ⇒ cộng `sepc` đúng cỡ instruction,
///   ghi [`TRAP_STATS`], in một dòng `# trap …`, rồi asm `sret` về instruction kế.
/// - Exception fatal ⇒ ghi stats, dump `PANIC` + `scause/sepc/stval` + thanh ghi chính, halt(1).
/// - **Ngắt timer** (`scause` bit 63 + mã 5, KT-47) ⇒ đếm tick + bằng chứng `sstatus.SIE`, nạp
///   deadline kế qua SBI TIME rồi **resume** (`sret`) — **không** phải exception, không fatal.
/// - Ngắt khác ⇒ ghi stats, dump `PANIC: unexpected interrupt` (PLIC/UART IRQ là KT-48), halt(1).
#[no_mangle]
pub extern "C" fn trap_dispatch(tf: &mut TrapFrame) {
    // KT-49: mỗi hart ghi trapframe gần nhất vào slot **của mình**; TAP `per-hart stack/guard`
    // kiểm frame nằm trong `.trap_stack` của chính hart đã trap (chứng minh `sscratch` per-hart).
    // KT-53: trap từ U-mode có `tp = 0` (không lộ hart id/kernel pointer cho user); handler U-mode
    // không dùng PerHart nào nên **không** ghi `note_trap_frame` (hart_id đọc `tp` sẽ là 0).
    if tf.from_supervisor() {
        let hart = Riscv64Virt.hart_id();
        smp::note_trap_frame(hart, tf as *const TrapFrame as usize);
    }
    let saved = *tf;
    match decode_trap(&saved) {
        Event::Exception(exc) => {
            // KT-53: trap từ **U-mode** (SPP=0) đi đường riêng: `ecall` (scause=8) ⇒ dispatch
            // 3 syscall ABI; fault U-mode ⇒ log + halt (không rơi vào chính sách guard/S-mode).
            #[cfg(feature = "user")]
            if !exc.from_supervisor {
                user::on_u_exception(tf, &exc);
                return;
            }
            // KT-56: trap do probe **thật** (load/store/insn) — ghi `scause/stval/sepc` rồi
            // resume ngay sau instruction lỗi; không đi chính sách fatal/PANIC.
            #[cfg(feature = "ktest")]
            if probe::on_trap(tf) {
                TRAP_STATS.record_exception(&saved, exc.kind, true);
                return;
            }
            // KT-46: guard page fault là page fault **thật** (scause 12/13/15, stval trong guard
            // page). Chính sách mặc định fatal dump+halt; probe test load-guard được resume một
            // lần để kiểm cả scause 13 và 15 trong một boot (xem `mmu::handle_guard_fault`).
            if mmu::is_guard_fault(&exc) {
                match mmu::handle_guard_fault(tf) {
                    mmu::GuardOutcome::Recovered => {
                        TRAP_STATS.record_exception(&saved, exc.kind, true);
                        GUARD_STATS.record(&saved, true);
                    }
                    mmu::GuardOutcome::Fatal => {
                        TRAP_STATS.record_exception(&saved, exc.kind, false);
                        GUARD_STATS.record(&saved, false);
                        report_guard_fault(&Riscv64Virt, &exc, &saved)
                    }
                }
                return;
            }
            let insn_len = if exc.kind.is_recoverable() {
                instruction_len(exc.kind, saved.sepc as usize)
            } else {
                4 // không dùng: fatal không cộng sepc; tránh đọc instruction lỗi
            };
            match handle_trap(tf, insn_len) {
                TrapAction::Resume => {
                    TRAP_STATS.record_exception(&saved, exc.kind, true);
                    // `frame` = địa chỉ trapframe trên trap stack: bằng chứng P1 (frame nằm trong
                    // `.trap_stack`, KHÔNG ngoài vùng stack) xuất hiện ngay trong log TAP.
                    let frame = tf as *const TrapFrame as usize;
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = writeln!(
                        w,
                        "# trap {} scause=0x{:x} sepc=0x{:x} stval=0x{:x} insn={} resume=0x{:x} frame=0x{:x}",
                        exc.name(),
                        saved.scause,
                        saved.sepc,
                        saved.stval,
                        insn_len,
                        tf.sepc,
                        frame
                    );
                }
                TrapAction::Fatal(fatal) => {
                    TRAP_STATS.record_exception(&saved, fatal.kind, false);
                    report_fatal(&Riscv64Virt, &fatal, &saved)
                }
                TrapAction::Interrupt { .. } => unreachable!("decode_trap đã trả Exception"),
                TrapAction::Timer => unreachable!("decode_trap đã trả Exception"),
                TrapAction::Ipi => unreachable!("decode_trap đã trả Exception"),
            }
        }
        // KT-47: ngắt **S-mode timer** thật (scause bit 63 + mã 5). Handler chạy với `SIE = 0`
        // (hardware xoá khi vào trap) nên không có trap lồng; `trap_entry`/handler **không** ghi
        // `sstatus` — `sret` khôi phục `SIE` từ `SPIE` (P2a của KT-45). Đếm tick rồi nạp deadline
        // kế ngay trong handler (trước khi resume) để dập ngắt đang chờ; nếu chưa arm thì nạp
        // `u64::MAX` để không bắn lại.
        Event::Timer => {
            let handler_sstatus = timer::read_sstatus();
            let sample = timer::on_timer_interrupt(saved.sstatus, handler_sstatus);
            timer::set_timer_deadline(sample.map(|s| s.next_deadline).unwrap_or(u64::MAX));
            TRAP_STATS.record_interrupt(&saved, INTERRUPT_SUPERVISOR_TIMER);
        }
        // KT-48: ngắt **external** (mã 9) = PLIC — đường duy nhất không fatal. Handler claim
        // của context `2*hart+1`, xử lý UART0 (claim → IIR → re-arm/ tắt sau N → complete) hoặc
        // đếm line lạ rồi complete để không treo; mọi bộ đếm nằm ở `PLIC_STATS` (không in trong
        // handler — in làm trễ/ngập log; TAP đọc sau). `sstatus.SIE` đã bị hardware xoá khi vào
        // trap nên handler không bị lồng.
        Event::Interrupt { code } if code == INTERRUPT_SUPERVISOR_EXTERNAL => {
            let _ = plic::on_external_interrupt();
            TRAP_STATS.record_interrupt(&saved, code);
        }
        // KT-50: ngắt **phần mềm S-mode** (mã 1) = IPI thật. Handler xoá MSIP của hart đang chạy
        // rồi drain hộp thư per-hart; **không** fatal và **không** đi qua nhánh timer (5)/
        // external (9). Drain ở đây là "drain trong handler" (yêu cầu KT-50); đường chờ khoá dùng
        // `ipi::wait_drain` để drain cả khi `SIE = 0`.
        Event::Ipi => {
            let _ = ipi::on_ipi_interrupt();
            TRAP_STATS.record_interrupt(&saved, INTERRUPT_SUPERVISOR_SOFTWARE);
        }
        Event::Interrupt { code } => {
            TRAP_STATS.record_interrupt(&saved, code);
            report_interrupt(&Riscv64Virt, code, &saved)
        }
    }
}

/// Cỡ instruction tại `pc` (2 hoặc 4 byte) — chỉ gọi với exception **fetch được** instruction
/// (`breakpoint`, `illegal`), nên đọc bộ nhớ tại `sepc` an toàn.
///
/// `ecall` theo spec RISC-V luôn 4 byte (không có dạng nén), nên không cần đọc.
pub(crate) fn instruction_len(kind: ExceptionKind, pc: usize) -> usize {
    match kind {
        ExceptionKind::EnvironmentCallFromU | ExceptionKind::EnvironmentCallFromS => 4,
        _ => {
            // SAFETY: `pc` là địa chỉ instruction vừa gây exception (đã fetch được). Trước KT-46
            // PA == VA; sau KT-46 image được identity-map RWX nên vẫn đọc trực tiếp được. Chỉ
            // đọc 2 byte, không ghi.
            let half = unsafe { core::ptr::read_volatile(pc as *const u16) };
            if half & 0b11 == 0b11 {
                4
            } else {
                2
            }
        }
    }
}

/// Nền tảng QEMU `virt` riscv64 — zero-sized, dựng được trong `static`/`const`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Riscv64Virt;

impl Platform for Riscv64Virt {
    fn put_str(&self, s: &str) {
        debug_put_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        debug_halt(code)
    }

    fn now_ticks(&self) -> u64 {
        timer::now_ticks()
    }

    fn timebase_hz(&self) -> u64 {
        TIME_HZ
    }

    fn hart_id(&self) -> usize {
        read_hart_id()
    }

    fn set_trap_vector(&self, vector: usize) {
        // Direct mode: MODE = low 2 bit = 0; địa chỉ phải 4-byte aligned. Symbol `trap_entry`
        // luôn aligned (`.balign 4`), `debug_assert` chặn cấu hình sai khi test.
        debug_assert_eq!(
            vector & 0b11,
            0,
            "stvec direct mode cần vector 4-byte aligned"
        );
        let value = vector & !0b11;
        // SAFETY: ghi CSR `stvec` của hart hiện tại; giá trị đã kiểm aligned, mode direct.
        unsafe { asm!("csrw stvec, {}", in(reg) value, options(nomem, nostack)) };
    }

    fn enable_irq(&self, irq: u32) -> bool {
        // KT-48: PLIC thật — set bit enable của context S-mode `2*hart+1` rồi đọc lại bit.
        plic::enable_irq(irq)
    }

    fn disable_irq(&self, irq: u32) -> bool {
        // KT-48: PLIC thật — clear bit enable rồi đọc lại (bit đã về 0).
        plic::disable_irq(irq)
    }

    fn set_timer_deadline(&self, deadline: u64) {
        // KT-47: SBI TIME (EID 0x54494D45) — deadline tuyệt đối, đơn vị timebase.
        timer::set_timer_deadline(deadline);
    }

    fn enable_timer(&self) -> bool {
        timer::enable_timer()
    }

    fn disable_timer(&self) {
        timer::disable_timer();
    }
}

/// Hart id đọc từ `tp` — `_start` đã chép `a0` (hartid OpenSBI truyền) vào đó.
fn read_hart_id() -> usize {
    let id: usize;
    // SAFETY: `tp` do chính `_start` đặt từ `a0`; kernel `no_std` chưa dùng TLS/`tp`.
    unsafe { asm!("mv {}, tp", out(reg) id, options(nomem, nostack, preserves_flags)) };
    id
}

/// Khoá in console cho SMP (KT-49): mỗi lần chỉ một hart ghi UART; hart khác spin.
///
/// Chỉ dùng cho **đường in chủ động** của test/product (vd `# hart h alive`). Handler trap **không**
/// dùng khoá này (P2a: handler không được chờ khoá do hart khác giữ khi hart đó bị ngắt).
static DEBUG_LOCK: AtomicBool = AtomicBool::new(false);

/// Chạy `f` trong vùng **độc quyền UART** — dùng để in một dòng của từng hart mà không xen ký tự.
pub fn debug_lock_do<R>(f: impl FnOnce() -> R) -> R {
    while DEBUG_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    let result = f();
    DEBUG_LOCK.store(false, Ordering::Release);
    result
}

/// In một chuỗi ra UART trong vùng khoá (một lần `put_str`, không xen ký tự giữa các hart).
pub fn debug_put_str_atomic(s: &str) {
    debug_lock_do(|| debug_put_str(s));
}

// ---------------------------------------------------------------------------------------------
// KT-55 / D2-80 — BUILD_ID của **chính ảnh** (16 hex FNV-1a của `.text`)
// ---------------------------------------------------------------------------------------------

extern "C" {
    /// Đầu `.text` (linker.ld) — bắt đầu vùng mã **đã nạp**.
    static __text_start: u8;
    /// Cuối `.text` (linker.ld) — dấu chấm hết của vùng mã.
    static __text_end: u8;
}

/// FNV-1a 64-bit — **fingerprint** ảnh (BUILD_ID 16 hex, D2-80); không phải hàm băm mật mã.
pub fn image_digest() -> (u64, usize) {
    // SAFETY: hai symbol do linker.ld cấp trong cùng ảnh; `__text_start <= __text_end` và vùng
    // `[__text_start, __text_end)` là đoạn `.text` **đã nạp** của chính tiến trình này (đọc, không
    // ghi), không phụ thuộc dữ liệu ngoài. `unsafe` nằm trong danh sách đóng D2-17.
    unsafe {
        let start = core::ptr::addr_of!(__text_start) as usize;
        let end = core::ptr::addr_of!(__text_end) as usize;
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut p = start;
        while p < end {
            let byte = core::ptr::read_volatile(p as *const u8);
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            p += 1;
        }
        (hash, end.saturating_sub(start))
    }
}

/// In `# conf image: build=<16 hex> text=<len>` — dòng **D2-80** buộc runner đối chiếu với ảnh.
pub fn report_image_digest() {
    let (hash, len) = image_digest();
    let mut buf = [0u8; 96];
    let mut n = 0usize;
    let mut put = |s: &[u8], n: &mut usize| {
        for &byte in s {
            if *n < buf.len() {
                buf[*n] = byte;
                *n += 1;
            }
        }
    };
    put(b"# conf image: build=", &mut n);
    let mut hex = [0u8; 16];
    for (i, h) in hex.iter_mut().enumerate() {
        let nib = ((hash >> ((15 - i) * 4)) & 0xf) as u8;
        *h = if nib < 10 {
            b'0' + nib
        } else {
            b'a' + nib - 10
        };
    }
    put(&hex, &mut n);
    put(b" text=", &mut n);
    let mut dec = [0u8; 20];
    let mut t = 0usize;
    let mut v = len;
    if v == 0 {
        dec[t] = b'0';
        t += 1;
    }
    while v > 0 {
        dec[t] = b'0' + (v % 10) as u8;
        v /= 10;
        t += 1;
    }
    while t > 0 {
        t -= 1;
        put(&[dec[t]], &mut n);
    }
    put(b"\n", &mut n);
    crate::debug_put_str(core::str::from_utf8(&buf[..n]).unwrap_or(""));
}

pub fn park_forever() -> ! {
    loop {
        let _ = ipi::drain_current();
        core::hint::spin_loop();
    }
}

/// In một byte ra UART — ghi thẳng THR; QEMU không cần chờ LSR.
pub fn debug_put_byte(byte: u8) {
    // SAFETY: MMIO của thiết bị 16550 trên QEMU virt; ghi một byte vào THR.
    unsafe {
        core::ptr::write_volatile(UART0_BASE as *mut u8, byte);
    }
}

/// `DebugPutStr` mức platform (syscall 12 dùng lại hàm này ở K3 trở đi).
pub fn debug_put_str(s: &str) {
    for byte in s.bytes() {
        debug_put_byte(byte);
    }
}

/// `DebugHalt` mức platform (syscall 13). Thoát QEMU qua SBI SRST:
/// `EID=0x53525354`, `FID=0` (system_reset), `a0=0` (shutdown), `a1=0` (no reason).
///
/// **Không** in marker: mỗi ảnh tự in dòng `# k3 halt` / `# ktest halt` của mình trước khi gọi,
/// để TAP sạch và runner phân biệt được ảnh nào (D2-08).
pub fn debug_halt(_code: usize) -> ! {
    // SAFETY: ecall SBI SRST theo SBI v2.0; OpenSBI đã nạp trước khi vào S-mode.
    unsafe {
        asm!(
            "ecall",
            in("a7") 0x5352_5354usize, // SRST
            in("a6") 0usize,          // FID 0 = system_reset
            in("a0") 0usize,          // reset_type = shutdown
            in("a1") 0usize,          // reset_reason = none
            options(noreturn),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Probe KT-45 (chỉ dùng trong ảnh ktest; API an toàn để `vifast-kbin-riscv64` không cần `unsafe` —
// R5/D2-17 chỉ cho `unsafe` trong `kernel/vifast-plat-*`).
// ---------------------------------------------------------------------------------------------

/// Chạy `ebreak` và trả địa chỉ instruction **ngay sau** `ebreak` (đã resume).
///
/// TAP: `scause=3`, `TRAP_STATS.last_sepc + 4 ==` kết quả (ebreak 4 byte), và trả về bình thường
/// ⇒ chứng minh trap S-mode thật + resume đúng `sepc`.
pub fn probe_breakpoint() -> usize {
    let after: usize;
    // SAFETY: `ebreak` 4 byte do `.4byte` cưỡng bức; handler của chính kernel resume qua nó.
    // Inline asm khai báo output `after`; mọi thanh ghi khác được `trap_entry` lưu/khôi phục.
    unsafe {
        asm!(
            "2:",
            ".4byte 0x00100073", // ebreak (4 byte, không phụ thuộc C extension)
            "3:",
            "la {after}, 3b",
            after = out(reg) after,
        );
    }
    after
}

/// Gọi SBI bằng `ecall` **từ S-mode** với EID không tồn tại, trả `(a0, địa chỉ ecall, địa chỉ sau
/// ecall)`.
///
/// D2-73: `MEDELEG` bit 9 = 0 ⇒ ecall-S do **M-mode/OpenSBI** xử lý, không vào `stvec`; TAP chỉ
/// chứng minh `a0 = SBI_ERR_NOT_SUPPORTED (-2)` và resume đúng `sepc` (không treo), **không** claim
/// là trap S-mode.
pub fn probe_ecall_sbi() -> (u64, usize, usize) {
    let ret: u64;
    let ecall_addr: usize;
    let after: usize;
    // SAFETY: ecall SBI hợp lệ; OpenSBI trả NOT_SUPPORTED cho EID lạ và quay lại `sepc + 4`.
    // a0 = mã trả về; t3/t4 giữ địa chỉ (SBI bảo toàn mọi thanh ghi trừ a0/a1).
    unsafe {
        asm!(
            "la t3, 2f",
            "2:",
            "ecall",
            "3:",
            "la t4, 3b",
            in("a7") 0xdead_beefu64, // EID không tồn tại
            lateout("a0") ret,
            lateout("a1") _, // SBI spec: a0/a1 là hai thanh ghi có thể bị clobber
            out("t3") ecall_addr,
            out("t4") after,
        );
    }
    (ret, ecall_addr, after)
}

/// Trap **tiêm** (D2-73): đặt `scause`/`stval`/`sepc` bằng CSR rồi nhảy thẳng vào `trap_entry`.
///
/// - `scause=2` (`illegal`) ⇒ handler resume qua `nop` 4 byte và trả địa chỉ sau `nop`.
///   **Không phải trap phần cứng**: OpenSBI v1.8.1 không delegate illegal (`MEDELEG` bit 2 = 0).
/// - `scause` fatal (vd 13/15) ⇒ handler dump `PANIC` rồi `DebugHalt(1)` — **không quay lại**.
///
/// Mục đích: kiểm **cùng đường asm/handler** với trap thật (`breakpoint`) cho các `scause` mà
/// firmware không delegate; TAP phải ghi rõ "inject".
pub fn probe_injected_trap(scause: u64, stval: u64) -> usize {
    let after: usize;
    let vector = trap_vector();
    // SAFETY: ghi các CSR trap (`sepc/scause/stval/sstatus.SPP`) rồi nhảy vào vector trap của
    // chính kernel; handler lưu/khôi phục mọi thanh ghi nên inline asm vẫn đúng ABI.
    unsafe {
        asm!(
            "csrr t3, sstatus", // giữ sstatus cũ để khôi phục sau trap tiêm
            "la t4, 3f",
            "csrw sepc, t4",
            "csrw scause, a0",
            "csrw stval, a1",
            "li t4, 0x100", // sstatus.SPP = 1: sret quay lại S-mode
            "csrs sstatus, t4",
            "jr t5",
            "3:",
            ".4byte 0x00000013", // nop 4 byte — nguồn độ dài instruction cho handler
            "4:",
            "csrw sstatus, t3", // khôi phục nguyên sstatus trước khi tiêm
            "la t6, 4b",
            in("a0") scause,
            in("a1") stval,
            in("t5") vector,
            out("t3") _,
            out("t4") _,
            lateout("t6") after,
        );
    }
    after
}

/// Xử lý trap **lồng** (trap xảy ra khi đang chạy trên trap stack) — KT-45 coi là **fatal**.
///
/// Được `trap_entry` gọi khi phát hiện sp bị ngắt nằm trong trap stack. In `PANIC` cố định rồi
/// `DebugHalt(1)`; **không** khôi phục/không resume frame cũ. Lưu ý bố cục: `sscratch` đã được
/// đặt lại về `__trap_stack_top` trước khi phát hiện, nên frame lồng được cấp phát ở **đúng slot**
/// `top-288` và **đè** frame ngoài — vô hại vì halt ngay, nhưng không được mô tả là "nằm dưới
/// frame ngoài".
#[no_mangle]
pub extern "C" fn nested_trap_halt() -> ! {
    debug_put_str("PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)\n");
    debug_halt(1)
}

/// Trap **lồng** để test P2b: đặt sp GIẢ vào trong trap stack (`__trap_stack_top - 0x100`) rồi
/// nhảy `trap_entry` ⇒ handler phải phát hiện lồng, in `PANIC: nested trap` và halt, **không**
/// treo/không resume. Dùng cho TAP 5 của ảnh test (boot dừng có chủ đích).
#[allow(unreachable_code)] // asm noreturn ⇒ `loop {}` chỉ để thoả kiểu `!`
pub fn probe_nested_trap() -> ! {
    let vector = trap_vector();
    // SAFETY: đổi sp sang vùng trap stack, đặt CSR trap rồi nhảy vào vector của chính kernel;
    // `options(noreturn)` vì đường này luôn kết thúc bằng PANIC + SBI SRST.
    unsafe {
        asm!(
            "la t3, 3f",
            "csrw sepc, t3",
            "li t4, 3", // scause = breakpoint (nếu không bị chặn là lồng, handler sẽ resume)
            "csrw scause, t4",
            "li t4, 0",
            "csrw stval, t4",
            "li t4, 0x100", // sstatus.SPP = 1
            "csrs sstatus, t4",
            "la t5, __trap_stack_top",
            "addi sp, t5, -0x100", // sp GIẢ nằm trong trap stack
            "jr t6",
            "3:",
            ".4byte 0x00000013", // nop 4 byte
            "4:",
            "j 4b",
            in("t6") vector,
            options(noreturn),
        );
    }
    loop {
        core::hint::spin_loop();
    }
}
// ---------------------------------------------------------------------------------------------
// **KT-77c** — vì sao có feature `block` (crate này, `Cargo.toml`) và vì sao hai `#[cfg]` ở trên
// (dòng `pci` + dòng `virtio_blk`) chỉ được **sửa tại chỗ**, không thêm/bớt dòng:
//
// * `ktest` là nhãn **harness** (probe/TAP). Trước KT-77c `vblk = ["ktest"]` nên ảnh **sản phẩm**
//   bật virtio-blk sẽ kéo `probe` vào ảnh — **cùng bẫy** `user = ["ktest"]` mà KT-77b đã sửa cho
//   U-mode (OD-K6-1, D2-08). Không tệp nào của driver (`virtio_blk.rs`, `pci.rs`,
//   `lib/vifast-virtio`) có `cfg(feature = "ktest")` ⇒ phụ thuộc **chỉ là nhãn**, không phải mã.
// * `block = []` = phần kernel của virtio-blk; `vblk = ["block", "ktest"]` giữ nguyên hành vi ảnh
//   test (một dòng mã driver không đổi).
// * **Byte-repro**: ảnh **không** feature phải giữ đúng baseline `c1072d72` (KT-79, D2-122). Một
//   dòng `//` thêm vào giữa tệp làm lệch **số dòng** của mọi panic location phía sau ⇒ `.rodata`
//   đổi (đo được: `JOURNAL/kt77c-logs/kt77c-baseline-lineshift.log`). Vì vậy giải thích nằm ở
//   **cuối tệp** và hai `#[cfg]` được sửa **tại chỗ**.
/// **KT-77c** — ô `static` một lần ghi cho object lớn (`Fs` ≈ 13 KiB) sống trong `.bss` thay vì
/// trên stack 64 KiB. API **an toàn**; `unsafe` nằm trong tệp này (crate thuộc danh sách đóng
/// D2-17) để `vifast-kbin-riscv64` không cần `unsafe` (luật R5).
pub mod static_slot;
