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
//! # Vì sao "đổi `rsp` trước `iretq`" đi qua **thanh ghi**, không qua ô nhớ
//!
//! Stub chung trả `rip` trong `rax` và ghi nó vào frame **hiện tại**. Khi chuyển tác vụ, frame
//! đích đã có sẵn `rip` đúng, nên stub phải **đổi `rsp`** sang frame đích thay vì ghi `rax`.
//!
//! **KT-298** — hợp đồng ấy là **hai** qword trả trong **thanh ghi** `rax`/`rdx`
//! ([`FrameReturn`]), không phải một ô `static` toàn cục. Vì sao: KT-286 dùng
//! `static VIFAST_SCHED_NEW_RSP` — ô ấy **dùng chung mọi hart**, nên hart phụ nhận tick PIT
//! trong khoảng giữa lúc handler đặt `rsp` và lúc stub đọc nó sẽ **lấy trộm** giá trị của BSP,
//! `mov rsp, <frame của BSP>` rồi `iretq` vào **cùng** một ngữ cảnh trên **cùng** một stack.
//! Đo được (bản vá tạm kéo dài cửa sổ, xem `v2/docs/k5-x86_64-boot.md` §15.3): boot đỏ 3/3 — treo sau
//! `# sched: switch b→a n=2`, hoặc `#GP` storm. Đây là gốc của boot 410/600 của KT-293.
//!
//! Thanh ghi là **per-hart theo định nghĩa** (mỗi hart có `rax`/`rdx` riêng) ⇒ không cần khoá,
//! không cần `cli`, không có cửa sổ: cùng bản vá ấy sau khi sửa chỉ còn **bỏ đói** tác vụ, không
//! còn làm hỏng ngữ cảnh.
//!
//! # KT-304 — tác vụ **ring 3** trong scheduler
//!
//! Thêm slot [`TASK_U`] chạy ở **CPL3**: frame khởi tạo của nó cùng 22 qword nhưng `cs = 0x1B`
//! ([`crate::umode::USER_CS`]), `ss = 0x23` ([`crate::umode::USER_SS`]), `rsp` = **user rsp**
//! ([`init_user_frame_words`]). Frame nằm trên **stack kernel riêng** của tác vụ (không phải
//! stack user): khi timer ngắt từ CPL3, CPU đẩy `SS:RSP:RFLAGS:CS:RIP` vào `TSS.RSP0` rồi stub
//! đẩy 17 qword — frame mới rơi **đúng** chỗ frame khởi tạo (xem `init_user_frame_words`).
//!
//! [`switch_frame_ok`] vì vậy có **hai nhánh**: frame kernel (`cs = 0x08`, `rsp` kernel bất kỳ
//! canonical/căn 8) và frame ring 3 (`cs = 0x1B`, `ss = 0x23`, `rsp` trong trang stack user,
//! `rip` trong trang mã user). `ss` của frame kernel **không** kiểm được: frame thật do CPU đẩy
//! khi cùng CPL chỉ có 20 qword, hai ô `rsp`/`ss` mà QEMU pop thêm nằm **ngoài** frame.
//!
//! # Phạm vi
//!
//! **Ba** tác vụ (hai kernel + một ring 3) + preemption bằng timer. **Không** nhiều tác vụ ring
//! 3, **không** IPC giữa chúng, **không** nhiều hart schedule, **không** priority, **không**
//! sleep/wake — đó là các bước sau (D2-212, D2-219).

use crate::exc;

/// Số tác vụ **thật** của scheduler (không tính ngữ cảnh `main`): A, B (kernel) + U (ring 3).
pub const TASK_COUNT: usize = 3;

/// Slot ngữ cảnh của đường boot (`sched::run`) — nơi scheduler quay về để in tổng kết.
pub const TASK_MAIN: usize = 0;
/// Slot tác vụ **A** (kernel).
pub const TASK_A: usize = 1;
/// Slot tác vụ **B** (kernel).
pub const TASK_B: usize = 2;
/// Slot tác vụ **U** — chạy ở **ring 3** (KT-304).
pub const TASK_U: usize = 3;
/// Tổng số slot ngữ cảnh: `main` + ba tác vụ.
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

/// Thứ tự tác vụ sau **mỗi** lần chuyển (chỉ số = `switches - 1`), bắt đầu sau tick đầu
/// (`main → A`).
///
/// **KT-304** thay luân phiên thuần bằng **bảng cố định** vì có ba tác vụ và thứ tự phải giữ
/// **hai** bất biến cùng lúc:
///
/// 1. **cả hai** chiều `A→B` và `B→A` xuất hiện (TAP 23 của KT-286 — một chiều thôi chỉ là
///    "chạy B rồi dừng");
/// 2. tác vụ **U** (ring 3) được chọn **≥ 2** lần: lần đầu bị tick preempt, lần sau **chạy lại**
///    ở ring 3 (yêu cầu của K5' — "schedule() chọn nó, tick preempt nó, nó chạy lại").
///
/// Bảng: `A→B→A→B→A→B→U→A→U→main`. U bị preempt ở bước 7 (`U→A`) và bước 9 (`U→main`), được
/// chạy lại ở bước 8 (`A→U`).
pub const TASK_SEQUENCE: [usize; 9] = [
    TASK_B, TASK_A, TASK_B, TASK_A, TASK_B, TASK_U, TASK_A, TASK_U, TASK_MAIN,
];

/// Số lần **chuyển** tác vụ trước khi scheduler nhường lại `main` — độ dài [`TASK_SEQUENCE`].
pub const MAX_SWITCHES: u64 = TASK_SEQUENCE.len() as u64;

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

/// Dựng **frame khởi tạo** cho tác vụ **ring 3** (KT-304): trả `(22 qword, rsp)`.
///
/// Khác [`init_frame_words`] ở **ba** ô:
///
/// - `cs = USER_CS` (`0x1B`) — `iretq` hạ đặc quyền xuống CPL3;
/// - `ss = USER_SS` (`0x23`) — selector stack user;
/// - `rsp = user_rsp` — `RSP` **user** sau `iretq` (không phải kernel rsp).
///
/// `kernel_stack_top` là đỉnh **stack kernel riêng** của tác vụ (nơi frame nằm và nơi
/// `TSS.RSP0` trỏ tới). Frame kết thúc **đúng** `kernel_stack_top` — **không** slack 8 như tác
/// vụ kernel: `iretq` đổi CPL **luôn** pop 5 qword (SDM Vol 3A §5.5), nên sau `iretq` kernel
/// `rsp = kernel_stack_top`; khi timer ngắt từ CPL3, CPU đẩy 5 qword rồi stub đẩy 17 qword ⇒
/// frame mới ở **đúng** `kernel_stack_top - 176`, trùng chỗ frame khởi tạo, không tràn.
///
/// Hàm **thuần** như [`init_frame_words`]: ảnh ghi từng qword bằng `asm::write_u64`.
pub const fn init_user_frame_words(
    kernel_stack_top: u64,
    entry: u64,
    rflags: u64,
    user_rsp: u64,
) -> ([u64; FRAME_WORDS], u64) {
    let mut words = [0u64; FRAME_WORDS];
    // 15 GPR đầu = 0: tác vụ ring 3 không thừa hưởng thanh ghi của kernel.
    words[FRAME_WORDS - 7] = 0; // vector — stub bỏ qua bằng `add rsp, 16`
    words[FRAME_WORDS - 6] = 0; // mã lỗi — stub bỏ qua
    words[FRAME_WORDS - 5] = entry; // rip — `iretq` nhảy vào đây (VA user)
    words[FRAME_WORDS - 4] = crate::umode::USER_CS as u64; // cs — user code `0x1B`
    words[FRAME_WORDS - 3] = rflags; // rflags — `IF = 1`
    words[FRAME_WORDS - 2] = user_rsp; // rsp — **user** rsp sau `iretq`
    words[FRAME_WORDS - 1] = crate::umode::USER_SS as u64; // ss — user data `0x23`
    let rsp = kernel_stack_top.saturating_sub(FRAME_BYTES as u64);
    (words, rsp)
}

/// Quyết định tác vụ kế theo [`TASK_SEQUENCE`]; sau `max_switches` lần chuyển thì trả về
/// [`TASK_MAIN`] để `sched::run` in tổng kết rồi nhường lại đường boot.
///
/// `switches == 0` (tick đầu) khởi động [`TASK_A`]; `current` lạ (slot ngoài `0..=TASK_U`) cũng
/// khởi động [`TASK_A`] — không bao giờ trả về chính nó, nên một trạng thái hỏng không thể làm
/// scheduler "chuyển sang chính mình" (vòng lặp vô hạn im lặng).
pub const fn next_task(current: usize, switches: u64, max_switches: u64) -> usize {
    if switches >= max_switches {
        return TASK_MAIN;
    }
    if switches == 0 || current > TASK_U {
        return TASK_A;
    }
    TASK_SEQUENCE[(switches - 1) as usize]
}

/// Giá trị stub chung (`vifast_isr_common`) nhận lại từ handler `exc_dispatch` — **hai** qword
/// trả trong **thanh ghi** `rax`/`rdx`.
///
/// `#[repr(C)]` với hai trường `u64` ⇒ SysV AMD64 trả ở `RAX` (trường 1) và `RDX` (trường 2);
/// stub asm đọc đúng hai thanh ghi ấy, **không** đọc bộ nhớ. Vì `rax`/`rdx` là **của hart đang
/// chạy**, bàn giao `rsp` giữa handler và stub **không** có trạng thái chia sẻ giữa các hart —
/// đó là toàn bộ bản sửa của KT-298 (xem đầu tệp).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameReturn {
    /// `rip` để `iretq` về. `0` ⇒ stub **halt sạch** (không quay lại chỗ lỗi).
    pub rip: u64,
    /// `rsp` của frame tác vụ kế. `0` ⇒ giữ frame hiện tại (đường exception/ngắt thường).
    pub new_rsp: u64,
}

impl FrameReturn {
    /// Trả về **đúng chỗ bị ngắt** — không đổi `rsp` (mọi exception/ngắt không chuyển tác vụ).
    pub const fn resume(rip: u64) -> Self {
        Self { rip, new_rsp: 0 }
    }

    /// **Chuyển** sang frame tác vụ ở `new_rsp`; `rip` chỉ để đối chiếu (stub `iretq` đọc `rip`
    /// từ **frame đích**, không dùng `rax`).
    pub const fn switch(new_rsp: u64, rip: u64) -> Self {
        Self { rip, new_rsp }
    }

    /// `true` ⇔ stub phải **halt** thay vì `iretq` (handler trả `rip = 0`).
    pub const fn is_halt(&self) -> bool {
        self.rip == 0 && self.new_rsp == 0
    }
}

/// `true` ⇔ frame tại `rsp` **đủ** để `iretq` vào tác vụ — kiểm **trước** khi stub đổi `rsp`
/// (KT-298, yêu cầu (b) của task; **KT-304** thêm nhánh ring 3).
///
/// Hàm **thuần** (nhận giá trị, không đọc bộ nhớ) nên test host chốt được từng điều kiện; ảnh
/// đọc năm ô của frame bằng `asm::read_u64` rồi gọi hàm này.
///
/// Điều kiện **chung** (mọi frame):
///
/// 1. `rip` **khác 0** và **canonical** — `iretq` tới địa chỉ không canonical ⇒ `#GP(0)` ngay
///    tại `iretq` (đúng dấu hiệu `# exc: vec=13 err=0x0` của boot 410).
/// 2. `rflags` có `IF` — thiếu `IF` thì tác vụ không bao giờ bị preempt (preemption chết im lặng).
///
/// Nhánh **kernel** (`cs == KERNEL_CS`): `rsp` **khác 0**, **canonical**, căn 8. **Không** kiểm
/// `ss`: frame **thật** của `main` chỉ có 20 qword (CPU không đẩy `SS:RSP` khi cùng CPL), nên
/// hai ô `rsp`/`ss` mà QEMU pop thêm nằm **ngoài** frame — với `main` chúng là stack của
/// `sched::run`, không phải dữ liệu frame.
///
/// Nhánh **ring 3** (`cs == USER_CS`, KT-304): `ss == USER_SS`, `rsp` nằm trong trang stack user
/// `[USER_STACK_VA, USER_STACK_TOP]` và căn 8, `rip` nằm trong trang mã user
/// `[USER_CODE_VA, USER_CODE_VA + 0x1000)`. Đây là **ba** lớp lỗi thật của tác vụ ring 3: `ss`
/// kernel ⇒ `iretq` không hạ đặc quyền đúng; `rsp` kernel ⇒ tác vụ chạy trên stack kernel;
/// `rip` kernel ⇒ tác vụ chạy mã kernel ở CPL3 (fault ngay lệnh đầu).
pub const fn switch_frame_ok(rip: u64, cs: u64, rflags: u64, rsp: u64, ss: u64) -> bool {
    if rip == 0 {
        return false;
    }
    // `IF` (bit 9) — thiếu nó thì tác vụ không bị ngắt được.
    if rflags & (1 << 9) == 0 {
        return false;
    }
    if cs == exc::KERNEL_CS {
        // Cửa sổ VA của kernel là 39-bit (D2-147); `rip`/`rsp` ngoài cửa sổ ⇒ `iretq` hỏng.
        return rsp != 0
            && rsp % 8 == 0
            && crate::format::is_canonical_bits(rip, crate::format::X86_64_VA_BITS)
            && crate::format::is_canonical_bits(rsp, crate::format::X86_64_VA_BITS);
    }
    if cs == crate::umode::USER_CS as u64 {
        // `ss` **có** nghĩa với frame ring 3: CPU đẩy `SS` khi đổi CPL, và frame khởi tạo do
        // chính ảnh ghi. Sai selector ⇒ `iretq` không hạ đặc quyền đúng.
        if ss != crate::umode::USER_SS as u64 {
            return false;
        }
        // `rsp` là **user** rsp: phải nằm trong trang stack user (một trang 4 KiB).
        if rsp < crate::umode::USER_STACK_VA || rsp > crate::umode::USER_STACK_TOP {
            return false;
        }
        if rsp % 8 != 0 {
            return false;
        }
        // `rip` phải nằm trong trang mã user đã map `US = 1` — không được chạy mã kernel ở CPL3.
        let code_end = crate::umode::USER_CODE_VA + crate::umode::USER_CODE_BYTES;
        if rip < crate::umode::USER_CODE_VA || rip >= code_end {
            return false;
        }
        return true;
    }
    false
}
