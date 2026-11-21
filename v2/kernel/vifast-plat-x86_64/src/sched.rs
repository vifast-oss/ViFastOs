// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Dữ liệu + logic thuần của preemption x86_64** — KT-286 (K5' bước 8).
//!
//! Tệp này **không** asm, **không** `unsafe`: nó chốt những gì asm của ảnh dùng làm `const`
//! operand và những quyết định thuần số học mà test host kiểm được **trước** khi có ảnh boot.
//!
//! # Vì sao preemption cần một frame **khởi tạo**
//!
//! Ảnh x86_64 đã có stub ngắt chung (`vifast_isr_common`): CPU đẩy `rflags/cs/rip`, stub đẩy
//! `vector` + `mã lỗi` + **15 GPR**, gọi `exc_dispatch`, rồi `pop` 15 GPR, `add rsp, 16` và
//! `iretq`. Nhờ vậy **một tác vụ mới** có thể được "vào" bằng cách dựng sẵn frame ấy trên stack
//! riêng của nó rồi cho stub `iretq` vào — không cần `call`, không cần asm riêng cho tác vụ.
//!
//! Bố cục frame khởi tạo (**22** qword = 176 byte), từ `rsp` tăng dần:
//!
//! ```text
//!   rsp + 0     r15      ┐
//!   rsp + 8     r14      │
//!   …                    │ 15 GPR — tác vụ mới nhận **0** hết
//!   rsp + 112   rax      ┘
//!   rsp + 120   vector   ← stub `add rsp, 16` bỏ qua (không `iretq` đọc)
//!   rsp + 128   mã lỗi   ←
//!   rsp + 136   rip      ← `iretq` đọc: entry của tác vụ
//!   rsp + 144   cs       ← `iretq` đọc: kernel code `0x08`
//!   rsp + 152   rflags   ← `iretq` đọc: `IF = 1` để tác vụ bị ngắt được
//!   rsp + 160   rsp      ← `iretq` đọc **chỉ khi** nó pop `SS:RSP` (xem dưới)
//!   rsp + 168   ss       ←
//! ```
//!
//! # Vì sao frame có **22** qword chứ không phải 20
//!
//! Intel SDM nói `IRET` chỉ pop `SS:RSP` khi **đổi mức đặc quyền** (`CS.RPL != CPL`); frame
//! cùng CPL đáng lẽ chỉ 20 qword. Nhưng **QEMU 11.1.3** (`target/i386/tcg/seg_helper.c`,
//! `helper_ret_protected`) dùng điều kiện:
//!
//! ```text
//!   rpl == cpl && (!(HF_CS64) || (HF_CS64 && !is_iret))
//! ```
//!
//! Trong **64-bit mode** (`HF_CS64 = 1`) với **`iret`** (`is_iret = 1`) điều kiện này **luôn
//! sai** ⇒ QEMU **luôn** pop `SS:RSP`, kể cả khi `CS.RPL == CPL`. Đo được: frame 20 qword làm
//! `iretq` lấy `rsp` từ ô ngay sau frame (0) ⇒ tác vụ chạy với `rsp = 0` ⇒ `#PF` khi timer đẩy
//! frame ⇒ `#DF` (log `# exc: vec=8 … rip=<task_a>`).
//!
//! Vì vậy frame khởi tạo mang **thêm** hai ô `rsp`/`ss`:
//!
//! - **QEMU** (pop 5): `rsp = stack_top - 8`, `ss = 0x10` — đúng trạng thái kernel.
//! - **CPU thật** (pop 3, đúng SDM): hai ô ấy **không** được đọc; `rsp` sau `iretq` =
//!   `frame + 160` = `stack_top - 24` — vẫn nằm trong stack, chỉ hụt 16 byte (vô hại).
//!
//! Frame 22 qword vì vậy đúng trên **cả hai**, không phải workaround chỉ cho QEMU.
//!
//! # Vì sao `rsp` khởi tạo lệch **8 byte**
//!
//! SysV AMD64 đòi `rsp % 16 == 8` ngay sau `call` (địa chỉ trả về đã đẩy). Tác vụ mới vào bằng
//! `iretq` chứ không bằng `call`, nên ảnh tự tạo đúng trạng thái ấy: frame kết thúc ở
//! `stack_top - 8`, tức `rsp` sau `iretq` là `stack_top - 8` ⇒ `% 16 == 8` khi `stack_top` căn 16.
//!
//! # Vì sao "đổi `rsp` trước `iretq`" là **một ô nhớ**, không phải tham số
//!
//! Stub chung trả `rip` trong `rax` và ghi nó vào frame **hiện tại**. Khi chuyển tác vụ, frame
//! đích đã có sẵn `rip` đúng, nên stub phải **đổi `rsp`** sang frame đích thay vì ghi `rax`.
//! Ô [`VIFAST_SCHED_NEW_RSP`] là hợp đồng giữa handler Rust và stub asm: handler đặt `rsp` của
//! tác vụ kế, stub đọc rồi **xoá** (một lần dùng), sau đó `pop`/`iretq` trên frame mới.
//!
//! # Phạm vi
//!
//! **Hai** tác vụ kernel + preemption bằng timer. **Không** U-mode task, **không** nhiều hart
//! schedule, **không** priority, **không** sleep/wake — đó là các bước sau (D2-212).

use core::sync::atomic::Ordering;

use crate::exc;

/// Số tác vụ **thật** của scheduler (không tính ngữ cảnh `main`).
pub const TASK_COUNT: usize = 2;

/// Slot ngữ cảnh của đường boot (`sched::run`) — nơi scheduler quay về để in tổng kết.
pub const TASK_MAIN: usize = 0;
/// Slot tác vụ **A**.
pub const TASK_A: usize = 1;
/// Slot tác vụ **B**.
pub const TASK_B: usize = 2;
/// Tổng số slot ngữ cảnh: `main` + hai tác vụ.
pub const TASK_SLOTS: usize = TASK_COUNT + 1;

/// Số qword của một frame ngắt **khởi tạo**: 15 GPR + vector + mã lỗi + rip + cs + rflags +
/// **rsp + ss** (hai ô cuối cho `iretq` pop 5 của QEMU — xem đầu tệp).
pub const FRAME_WORDS: usize = 22;
/// Cỡ frame khởi tạo tính bằng byte (176).
pub const FRAME_BYTES: usize = FRAME_WORDS * 8;
/// Khoảng lệch 8 byte giữa frame và `stack_top` — tạo `rsp % 16 == 8` sau `iretq` (SysV).
pub const INITIAL_RSP_SLACK: u64 = 8;

/// `RFLAGS` của tác vụ mới: `IF = 1` (bit 9) + bit dự phòng 1 (bit 1) — **bắt buộc** để timer
/// ngắt được tác vụ; thiếu `IF` thì preemption không bao giờ xảy ra.
pub const TASK_RFLAGS: u64 = 0x202;

/// Số lần **chuyển** tác vụ trước khi scheduler nhường lại `main` (A→B, B→A, … ba vòng).
///
/// Số chẵn để lần chuyển cuối là B→A: cả hai chiều đều xuất hiện **ba** lần, nên dòng
/// `# sched: switch a→b` và `# sched: switch b→a` đều có bằng chứng thật.
pub const MAX_SWITCHES: u64 = 6;

/// Dựng **frame khởi tạo** cho một tác vụ: trả `(22 qword, rsp)`.
///
/// `stack_top` phải căn **16 byte** (stack tác vụ trong ảnh khai `#[repr(align(16))]`); `entry`
/// là địa chỉ hàm entry của tác vụ; `rflags` thường là [`TASK_RFLAGS`].
///
/// Hàm **thuần**: không ghi bộ nhớ — ảnh ghi từng qword bằng `asm::write_u64` (API an toàn của
/// crate arch), nhờ vậy ảnh vẫn **không** `unsafe` (R5).
pub const fn init_frame_words(
    stack_top: u64,
    entry: u64,
    rflags: u64,
) -> ([u64; FRAME_WORDS], u64) {
    let mut words = [0u64; FRAME_WORDS];
    // 15 GPR đầu = 0: tác vụ mới không thừa hưởng thanh ghi của tác vụ trước.
    words[FRAME_WORDS - 7] = 0; // vector — stub bỏ qua bằng `add rsp, 16`
    words[FRAME_WORDS - 6] = 0; // mã lỗi — stub bỏ qua
    words[FRAME_WORDS - 5] = entry; // rip — `iretq` nhảy vào đây
    words[FRAME_WORDS - 4] = exc::KERNEL_CS; // cs — kernel code `0x08`
    words[FRAME_WORDS - 3] = rflags; // rflags — `IF = 1`
    words[FRAME_WORDS - 2] = stack_top.saturating_sub(INITIAL_RSP_SLACK); // rsp — QEMU pop 5
    words[FRAME_WORDS - 1] = crate::umode::KERNEL_DS as u64; // ss — QEMU pop 5 (`0x10`)
    let rsp = stack_top.saturating_sub(FRAME_BYTES as u64 + INITIAL_RSP_SLACK);
    (words, rsp)
}

/// Quyết định tác vụ kế: luân phiên `A → B → A …`; sau `max_switches` lần chuyển thì trả về
/// [`TASK_MAIN`] để `sched::run` in tổng kết rồi nhường lại đường boot.
///
/// `current` lạ (kể cả [`TASK_MAIN`]) khởi động [`TASK_A`] — không bao giờ trả về chính nó, nên
/// một trạng thái hỏng không thể làm scheduler "chuyển sang chính mình" (vòng lặp vô hạn im lặng).
pub const fn next_task(current: usize, switches: u64, max_switches: u64) -> usize {
    if switches >= max_switches {
        return TASK_MAIN;
    }
    match current {
        TASK_A => TASK_B,
        TASK_B => TASK_A,
        _ => TASK_A,
    }
}

/// Ô "đổi `rsp` trước `iretq`" — **định nghĩa** ở [`crate::asm`] (mô-đun duy nhất của crate được
/// `unsafe`, D2-17) vì stub asm đọc **trực tiếp** symbol này; ở đây chỉ **dùng lại**.
///
/// `0` = không đổi `rsp` (đường exception/ngắt bình thường); khác `0` = `rsp` của frame tác vụ
/// kế. Stub **xoá** ô này sau khi đọc (một lần dùng) để một tick không chuyển tác vụ không thể
/// "dính" sang tick sau.
pub use crate::asm::VIFAST_SCHED_NEW_RSP;

/// Đặt `rsp` của frame tác vụ kế — handler gọi **trước** khi trả về stub.
pub fn set_new_rsp(rsp: u64) {
    VIFAST_SCHED_NEW_RSP.store(rsp, Ordering::Release);
}

/// Đọc rồi **xoá** ô đổi `rsp`; trả `0` nếu không có yêu cầu đổi.
pub fn take_new_rsp() -> u64 {
    VIFAST_SCHED_NEW_RSP.swap(0, Ordering::AcqRel)
}
