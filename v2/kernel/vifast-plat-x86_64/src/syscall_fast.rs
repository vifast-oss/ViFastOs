// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `syscall_fast` — **KT-417c**: đường vào/ra `syscall`/`sysret` của x86_64 (`IA32_STAR`,
//! `IA32_LSTAR`, `IA32_FMASK`, `IA32_EFER.SCE`, `swapgs` + **GS scratch per-hart**).
//!
//! # Vì sao phải có mô-đun này (không chỉ "ghi ba MSR")
//!
//! `syscall` **không** đẩy frame và **không** đổi stack như `int 0x80`:
//!
//! | | `int 0x80` | `syscall` |
//! |---|---|---|
//! | đẩy `SS/RSP/RFLAGS/CS/RIP` | **có** (CPU, từ CPL3) | **không** |
//! | đổi `RSP` sang stack kernel | **có** (`TSS.RSP0`) | **không** |
//! | selector đích | từ **gate IDT** (0xEE) | `CS = STAR[47:32]`, cứng |
//! | đường ra | `iretq` (khôi phục từ frame) | `sysretq` (`CS = STAR[63:48]+16`, `SS = +8`) |
//! | đổi `GS.base` | không | **phải** `swapgs` (cặp đúng) |
//!
//! Hệ quả: nếu stub chỉ `call` handler rồi `sysretq` thì **kernel chạy trên stack của user** —
//! user ghi đè được khung ngắt (phá bảo mật) và `-smp 4` hỏng ngay. Vì vậy cần **GS scratch
//! per-hart**: `swapgs` đưa `GS.base` về [`GS_SCRATCH`] của hart, rồi `mov rsp, gs:[0]` nạp
//! `RSP0` của hart đó. Cùng ô đó lưu `RSP` của user để `sysretq` trả lại.
//!
//! # Bất biến (mỗi cái có test host / log ảnh)
//!
//! 1. **Cặp `swapgs`**: đúng **một** lần vào (ngay đầu stub) và đúng **một** lần ra (ngay trước
//!    `sysretq`) **trên đường `Return`**. `int 0x80` **không** `swapgs` — hai đường khác nhau,
//!    không dùng chung đuôi. **Hotfix audit X2**: đuôi stub **không** phải đường ra duy nhất —
//!    `Exit` đi qua [`crate::asm::enter_user`], nơi **chuẩn hoá** cặp GS bằng đúng một `swapgs`
//!    **có điều kiện** trước `iretq` ([`needs_swap_before_user`]/[`user_gs_ok`]); thiếu nó thì
//!    `syscall` kế tiếp ghi vào VA 8 (xem khối "mô hình GS" dưới).
//! 2. **`STAR[63:48] = SELECTOR_KERNEL_DATA (0x10)`** ⇒ `SYSRET` nạp `CS = 0x20 | 3 = 0x23` và
//!    `SS = 0x18 | 3 = 0x1B` (công thức **cứng** của `SYSRET`, SDM Vol 2B). Vì vậy GDT phải xếp
//!    user **data** ngay **trước** user code — đúng `[arch.x86_64.gdt.planned]` của
//!    `spec/kernel.toml` (KT-417c đã áp; `check_kernel.py` đối chiếu chéo).
//! 3. **`FMASK` xoá `AC`** (bit 18): SMAP đã bật từ `KT-418b`; nếu user vào với `AC = 1` thì
//!    `stac`/`clac` thành vô nghĩa — mọi `UserAccess` guard mất tác dụng. Cũng xoá `IF`/`TF`/`DF`/
//!    `NT` để kernel vào với ngắt tắt, không single-step, hướng chuỗi xác định.
//! 4. **`EFER.SCE` đọc-sửa-ghi**: `EFER` đang có `LME|NXE`; ghi đè mất `NXE` là mất NX/W^X
//!    (`wx_probe` đỏ). **Cấm** `mov efer, imm`.
//! 5. **MSR là per-hart**: `init` phải chạy trên **mỗi** hart (BSP + AP), nếu không `syscall`
//!    trên hart phụ `#UD`.

use core::sync::atomic::{AtomicU64, Ordering};

/// `asm` (MSR + `RFLAGS`) chỉ dùng trên bare-metal — host không có lệnh đặc quyền.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::asm;
use crate::control::{EFER_LME, EFER_NXE, EFER_SCE};
/// Số hiệu MSR chỉ dùng trên bare-metal (host không có MSR của khách) — tách `use` để host build
/// không sinh cảnh báo "unused import".
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::control::{
    MSR_IA32_EFER, MSR_IA32_FMASK, MSR_IA32_GS_BASE, MSR_IA32_KERNEL_GS_BASE, MSR_IA32_LSTAR,
    MSR_IA32_STAR,
};
use crate::gdt;
use crate::smp::MAX_HARTS;

/// Dấu **đường vào** trong ô `vector` của frame — stub `syscall` đẩy giá trị này thay cho
/// `0x80` của `int 0x80`.
///
/// `0x100` **không** phải vector hợp lệ của x86 (vector là 0..255) nên không thể nhầm với một
/// ngắt thật; nhờ nó handler phân biệt được "vào bằng `syscall`" mà **không** cần thêm tham số
/// thứ 12 cho `syscall_dispatch` (giữ nguyên hợp đồng `7C-r13` + 11 tham số của KT-340c).
pub const SYSCALL_FAST_VEC: u64 = 0x100;

/// `STAR[47:32]` — `CS` mà `SYSCALL` nạp: kernel **code** (`0x08`).
pub const STAR_SYSCALL_CS: u64 = gdt::SELECTOR_KERNEL_CODE as u64;
/// `STAR[63:48]` — base mà `SYSRET` cộng: kernel **data** (`0x10`) ⇒ `CS = 0x20 | 3`,
/// `SS = 0x18 | 3`.
pub const STAR_SYSRET_BASE: u64 = gdt::SELECTOR_KERNEL_DATA as u64;

/// Giá trị `IA32_STAR` đúng cho GDT hiện tại: `[63:48] = 0x10`, `[47:32] = 0x08`.
pub const fn star_value() -> u64 {
    (STAR_SYSRET_BASE << 48) | (STAR_SYSCALL_CS << 32)
}

/// `RFLAGS.AC` — bit 18 (SMAP bypass).
pub const RFLAGS_AC: u64 = 1 << 18;
/// `RFLAGS.IF` — bit 9.
pub const RFLAGS_IF: u64 = 1 << 9;
/// `RFLAGS.TF` — bit 8 (single-step).
pub const RFLAGS_TF: u64 = 1 << 8;
/// `RFLAGS.DF` — bit 10 (hướng chuỗi).
pub const RFLAGS_DF: u64 = 1 << 10;
/// `RFLAGS.NT` — bit 14 (nested task).
pub const RFLAGS_NT: u64 = 1 << 14;

/// Giá trị `IA32_FMASK`: xoá `IF|TF|DF|NT|AC` khi vào kernel.
///
/// `AC` là bit **bắt buộc** (SMAP — xem doc mô-đun); `IF`/`TF`/`DF`/`NT` là chuẩn "vào kernel
/// sạch trạng thái". **Không** xoá `IOPL` (đã 0 ở ring 3).
pub const fn fmask_value() -> u64 {
    RFLAGS_IF | RFLAGS_TF | RFLAGS_DF | RFLAGS_NT | RFLAGS_AC
}

/// `CS` mà `SYSRET` nạp, suy **từ** [`star_value`] theo đúng công thức cứng của SDM:
/// `STAR[63:48] + 16`, thêm `RPL 3` (vì trả về CPL3).
///
/// Đây là giá trị mà stub `syscall` truyền cho handler ở tham số `cs` — **không** hardcode
/// `0x23`: nếu `STAR`/GDT lệch nhau thì dòng log cũng lệch và ca kiểm đỏ.
pub const fn sysret_cs() -> u16 {
    (((STAR_SYSRET_BASE + 16) & 0xFFFF) as u16) | gdt::RPL_USER
}

/// `SS` mà `SYSRET` nạp: `STAR[63:48] + 8`, thêm `RPL 3`.
pub const fn sysret_ss() -> u16 {
    (((STAR_SYSRET_BASE + 8) & 0xFFFF) as u16) | gdt::RPL_USER
}

// ---------------------------------------------------------------------------------------------
// GS scratch per-hart
// ---------------------------------------------------------------------------------------------
//
// Bố cục **byte-level** với stub asm (`gs:[N]`), nên offset là hằng công khai và có test host
// chốt `size_of`/`align_of` của mảng atomic.
//
// | offset | nội dung |
// |---|---|
// | 0 | `rsp0` — đỉnh stack kernel mà `syscall` nạp (per-hart) |
// | 8 | `user_rsp` — `RSP` của ring 3 lúc vào, `sysretq` trả lại |
// | 16 | dự phòng (0) |
// | 24 | dự phòng (0) |

/// Offset `gs:[…]` của `rsp0` trong [`GS_SCRATCH`].
pub const GS_OFF_RSP0: usize = 0;
/// Offset `gs:[…]` của `RSP` user đã lưu.
pub const GS_OFF_USER_RSP: usize = 8;
/// Số `u64` mỗi hart trong [`GS_SCRATCH`] (4 ⇒ 32 B, đủ cho 2 ô dùng + 2 ô dự phòng).
pub const GS_SCRATCH_WORDS: usize = 4;

// ---------------------------------------------------------------------------------------------
// **Hotfix audit W8b (X2)** — mô hình `GS` phải khép ở **mọi** đường rời kernel về user
// ---------------------------------------------------------------------------------------------
//
// Bản KT-417c đặt cặp `swapgs` **chỉ** trong thân stub `syscall`. Nhưng đuôi stub **không** phải
// đường ra duy nhất: `SyscallOutcome::Exit` gọi `coretest::start_core_phase` → `asm::enter_user`
// (`iretq` xuống CPL3) — không quay lại stub ⇒ `GS.base` ở lại **scratch của kernel** và
// `KERNEL_GS_BASE = 0`. Lần `syscall` **kế tiếp** `swapgs` đảo lại ⇒ `GS.base = 0`, rồi
// `mov gs:[8], rsp` ghi vào **địa chỉ tuyến tính 8** ⇒ `#PF` tại CPL0 **trên RSP của user**
// (exception từ CPL0 không đổi stack), và dưới `SMAP` lần đẩy khung ngầm định đó lại fault ⇒
// `#DF`. Đó là lỗi **leo thang** (kernel chạy trên bộ nhớ user), không chỉ DoS.
//
// # Mô hình chọn: (a) — "chuẩn hoá GS ngay trước **mọi** `iretq` về CPL3"
//
// * Bất biến: `GS.base == USER_GS_BASE` ⇔ **chuẩn bị chạy CPL3**; `GS.base == gs_scratch(hart)`
//   ⇔ đang trong kernel của cửa `syscall`.
// * `swapgs` **vẫn** đúng một cặp trong stub (đường `Return`), và [`crate::asm::enter_user`] —
//   nơi **duy nhất** mọi đường `iretq` về CPL3 đi qua (`umode`, `coretest::start_core_phase`,
//   `shellpath`) — **chuẩn hoá** cặp đó bằng **đúng một** `swapgs` **có điều kiện** trước `iretq`.
// * Lý do chọn (a) thay vì (b) ("không giữ GS lật suốt handler"): (a) không đổi hợp đồng
//   `gs:[0]`/`gs:[8]` mà stub asm và `GS_SCRATCH` đã dùng (không phải sửa stub, không thêm ô
//   "user_rsp per-hart" thứ hai), và **kiểm được trên host**: điều kiện là hàm thuần
//   [`needs_swap_before_user`]/[`user_gs_ok`] + bất biến "một `swapgs` trước `iretq`" đọc thẳng
//   từ mã asm của `enter_user` (`tests/syscall_frame_gs.rs`). Mô hình (b) đòi đổi **cả** stub
//   **và** mọi chỗ đọc `gs:[…]`, tức chạm nhiều đường hơn trong cùng ngân sách hotfix.
// * `USER_GS_BASE = 0` vì ring 3 **không** có TLS ở bước này (xem [`init`]). Nếu sau này có TLS
//   cho user, giá trị này phải thành **per-hart đã ghi nhận** — khi đó `enter_user` so với ô đó,
//   không so với 0.

/// `GS.base` của ring 3: **0** (ring 3 không có TLS ở bước này — [`init`] ghi đúng giá trị này
/// vào `IA32_GS_BASE`).
pub const USER_GS_BASE: u64 = 0;

/// `true` ⇔ trước `iretq` về CPL3 **phải** `swapgs`: `GS.base` đang là scratch của kernel (đường
/// `Exit`/`suspend-resume` không đi qua đuôi stub — audit X2).
pub const fn needs_swap_before_user(gs_base: u64) -> bool {
    gs_base != USER_GS_BASE
}

/// `true` ⇔ `GS.base` đã đúng giá trị của ring 3 (chuẩn bị `iretq` xuống CPL3).
pub const fn user_gs_ok(gs_base: u64) -> bool {
    gs_base == USER_GS_BASE
}

/// `true` ⇔ `GS.base` là vùng scratch của **kernel** cho `hart` — trạng thái **bắt buộc** ngay
/// sau `swapgs` ở đầu stub `syscall`. Lệch ⇒ cặp `swapgs` đã bị lật từ trước (đúng lỗi X2).
pub fn gs_is_kernel(hart: usize, gs_base: u64) -> bool {
    gs_base == gs_scratch(hart)
}

/// Vùng scratch **per-hart** mà `swapgs` trỏ tới. `0` = chưa cấp.
///
/// `AtomicU64` (không `unsafe`) vì kernel và stub asm cùng chạm; mọi truy cập đều là "một hart
/// của chính nó" nên `Relaxed` là đủ (không có bất biến thứ tự nào giữa các hart).
static GS_SCRATCH: [[AtomicU64; GS_SCRATCH_WORDS]; MAX_HARTS] = [const {
    [
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ]
}; MAX_HARTS];

/// Địa chỉ (VA = PA, identity map) vùng scratch của `hart` — giá trị ghi vào
/// `IA32_KERNEL_GS_BASE`.
///
/// `hart >= MAX_HARTS` là lỗi lập trình ⇒ panic fail-loud.
pub fn gs_scratch(hart: usize) -> u64 {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    GS_SCRATCH[hart].as_ptr() as u64
}

/// Ghi `rsp0` (stack kernel cho `syscall`) của `hart` — stub đọc `gs:[0]`.
pub fn set_kernel_stack(hart: usize, rsp0: u64) {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    GS_SCRATCH[hart][GS_OFF_RSP0 / 8].store(rsp0, Ordering::Relaxed);
}

/// Đọc lại `rsp0` của `hart` (bằng chứng "đã cấp stack", không chỉ tin lời gọi).
pub fn kernel_stack(hart: usize) -> u64 {
    if hart >= MAX_HARTS {
        return 0;
    }
    GS_SCRATCH[hart][GS_OFF_RSP0 / 8].load(Ordering::Relaxed)
}

/// Đọc lại `RSP` user đã lưu của `hart` (`0` = chưa lần nào vào bằng `syscall`).
pub fn user_rsp(hart: usize) -> u64 {
    if hart >= MAX_HARTS {
        return 0;
    }
    GS_SCRATCH[hart][GS_OFF_USER_RSP / 8].load(Ordering::Relaxed)
}

/// `EFER` **đích** sau khi bật `SCE`, giữ nguyên `LME`/`NXE` (và mọi bit khác đang có).
///
/// Hàm **thuần** để test host chứng minh "không ghi đè mất `LME`/`NXE`" mà không cần CPU thật.
pub const fn efer_with_sce(current: u64) -> u64 {
    let kept = current | EFER_LME | EFER_NXE;
    kept | EFER_SCE
}

/// `true` ⇔ tập bit `EFER` đã đủ cho `syscall`/`sysret` **và** còn giữ `NXE` (W^X).
pub const fn efer_ok(efer: u64) -> bool {
    efer & EFER_SCE != 0 && efer & EFER_NXE != 0 && efer & EFER_LME != 0
}

/// `true` ⇔ `mask` có đủ năm bit phải xoá khi vào kernel (bắt buộc có `AC`).
pub const fn fmask_ok(mask: u64) -> bool {
    mask & RFLAGS_AC != 0
        && mask & RFLAGS_IF != 0
        && mask & RFLAGS_TF != 0
        && mask & RFLAGS_DF != 0
        && mask & RFLAGS_NT != 0
}

// ---------------------------------------------------------------------------------------------
// Nạp MSR + GS — chỉ trên bare-metal
// ---------------------------------------------------------------------------------------------

// Stub asm `syscall` (định nghĩa trong `asm/syscall.rs`) — `LSTAR` trỏ vào đây.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
extern "C" {
    fn vifast_isr_syscall_fast();
}

/// Nạp **toàn bộ** trạng thái `syscall`/`sysret` cho `hart`: `GS`/`KERNEL_GS_BASE`, `STAR`,
/// `LSTAR`, `FMASK`, rồi **cuối cùng** bật `EFER.SCE`.
///
/// Thứ tự có chủ đích: bật `SCE` **sau** khi mọi thứ khác đã sẵn sàng — nếu ai đó lỡ chạy
/// `syscall` giữa chừng thì `#UD` (lỗi có tên) còn hơn nhảy vào `LSTAR = 0` (nhảy vào rác).
///
/// `rsp0` = đỉnh stack kernel mà `syscall` sẽ dùng cho hart này (khác `TSS.RSP0`: `int 0x80`
/// dùng `TSS`).
///
/// Gọi **một lần cho mỗi hart** (BSP ở `main.rs`/`umode::run`, AP ở `smp.rs`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn init(hart: usize, rsp0: u64) {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    set_kernel_stack(hart, rsp0);
    // GS: **kernel** dùng scratch, **user** dùng [`USER_GS_BASE`] (0 — không có TLS ở ring 3).
    // `swapgs` đảo hai ô MSR; "đang ở kernel" ⇔ `GS.base == gs_scratch(hart)`.
    asm::write_msr(MSR_IA32_GS_BASE, USER_GS_BASE);
    asm::write_msr(MSR_IA32_KERNEL_GS_BASE, gs_scratch(hart));
    asm::write_msr(MSR_IA32_STAR, star_value());
    asm::write_msr(MSR_IA32_LSTAR, vifast_isr_syscall_fast as *const () as u64);
    asm::write_msr(MSR_IA32_FMASK, fmask_value());
    // **Đọc-sửa-ghi** `EFER`: giữ `LME`/`NXE` đang bật (ghi đè mất `NXE` = mất W^X).
    let efer = asm::read_msr(MSR_IA32_EFER);
    asm::write_msr(MSR_IA32_EFER, efer_with_sce(efer));
}

/// Đọc lại 4 MSR + `EFER` **và** `IA32_GS_BASE` của hart **hiện tại** — bằng chứng "đã ghi thật"
/// in vào log ảnh.
///
/// Trả `(efer, star, fmask, kernel_gs_base, gs_base)`: `lstarcs` = `CS` mà `SYSRET` sẽ nạp, suy
/// từ `STAR` **đọc lại** (không phải hằng) ⇒ dòng log chứng minh `STAR` trên CPU đúng thứ
/// `sysret_cs()` cần. `gs_base` là ô mà stub asm **giả định** = 0 khi `swapgs` (audit X6): đọc lại
/// là cách duy nhất chứng minh "stub sẽ ghi vào `GS_SCRATCH`" ngay lần `syscall` đầu.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn readback() -> (u64, u64, u64, u64, u64) {
    let efer = asm::read_msr(MSR_IA32_EFER);
    let star = asm::read_msr(MSR_IA32_STAR);
    let fmask = asm::read_msr(MSR_IA32_FMASK);
    let kgs = asm::read_msr(MSR_IA32_KERNEL_GS_BASE);
    let gs_base = asm::read_msr(MSR_IA32_GS_BASE);
    (efer, star, fmask, kgs, gs_base)
}

/// Stub host của [`init`] — **panic**: không có MSR của khách trên host.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn init(_hart: usize, _rsp0: u64) {
    unimplemented!("vifast_plat_x86_64::syscall_fast::init: chỉ chạy trên bare-metal")
}

/// Stub host của [`readback`] — **panic** (không có MSR).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn readback() -> (u64, u64, u64, u64, u64) {
    unimplemented!("vifast_plat_x86_64::syscall_fast::readback: chỉ chạy trên bare-metal")
}
