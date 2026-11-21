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
//! # KT-310 — **nhiều hart** schedule: hàng đợi chạy **toàn cục** + khoá
//!
//! KT-286..KT-308 chỉ **BSP** gọi `schedule()`: `on_tick` của ảnh chỉ chạy trên hart 0 (handler
//! timer so `LAPIC_ID` với BSP), còn hart phụ chỉ tăng `AP_TICK_COUNT`. KT-310 thêm **pha nhiều
//! hart** — một hàng đợi chạy **toàn cục** (FIFO ba tác vụ đã có) mà **cả BSP và hart 1** cùng
//! `schedule()`, dưới **một** khoá.
//!
//! Pha này chạy **sau** chuỗi cố định [`TASK_SEQUENCE`] (nên TAP 22/23/24 của KT-286/KT-304
//! **không** đổi nghĩa) và có **trần cứng** số vòng ⇒ không bao giờ treo boot.
//!
//! Bốn bất biến (mỗi cái là một lớp lỗi thật):
//!
//! 1. **Một chủ**: một tác vụ chỉ thuộc **một** hart tại một thời điểm ([`MH_FREE`] + CAS
//!    ownership). Không có nó, hai hart `iretq` vào **cùng** một frame trên **cùng** một stack —
//!    đúng lớp lỗi boot 410 của KT-298 nhưng ở tầng scheduler.
//! 2. **Không double-pick**: hàng đợi nói tác vụ rảnh mà ownership nói khác ⇒ dừng có chẩn đoán
//!    (`# sched: BAD double-pick`), không chạy tiếp. Đột biến `--features no-sched-lock` chứng
//!    minh phép kiểm này **có răng**.
//! 3. **Chờ hart kia**: pha chỉ kết thúc khi **mọi** hart tham gia đã chọn ≥ 1 tác vụ
//!    ([`mh_end`]) — nếu không, "nhiều hart" có thể là **một** hart chạy còn hart kia im lặng.
//! 4. **Trần cứng**: hết [`MH_HARD_ROUNDS`] thì kết thúc **dù** hart kia chưa chọn ⇒ fail-closed
//!    (TAP đỏ), **không** treo. Cùng khuôn "trần số vòng" của KT-301.
//!
//! # Phạm vi
//!
//! **Bốn** tác vụ (ba kernel + một ring 3) + preemption bằng timer + **pha nhiều hart** (**bốn**
//! hart, hàng đợi toàn cục, khoá). **Không** nhiều tác vụ ring 3, **không** IPC giữa chúng,
//! **không** load balancing, **không** priority, **không** migration chủ động, **không**
//! sleep/wake — đó là các bước sau (D2-212, D2-219). **Không** tuyên bố K5' đạt.
//!
//! # KT-315 — pha nhiều hart mở lên **bốn** hart
//!
//! KT-310 làm pha nhiều hart cho **hai** hart (BSP + hart 1) với **ba** tác vụ. Mở lên **bốn**
//! hart (K5' đòi "như K3") cần **hai** thay đổi, cả hai đều là **hệ quả của số học**, không phải
//! nới ngưỡng:
//!
//! 1. **Tác vụ thứ tư** ([`TASK_C`]). Với **ba** tác vụ và **bốn** hart, hart thứ tư **đói vĩnh
//!    viễn**: hàng đợi luôn có đúng **một** tác vụ rảnh, và mỗi hart trả tác vụ cũ rồi lấy lại
//!    **chính nó** trong cùng vùng tới hạn ⇒ hart không giữ tác vụ nào **không bao giờ** thấy
//!    hàng đợi khác rỗng. Test host `four_harts_need_four_tasks` chốt đúng điều kiện ấy.
//! 2. **Cổng khởi động**: hart phụ chỉ chọn **sau** khi BSP đã chọn lần đầu. Nhịp PIT của BSP là
//!    100 Hz (10 ms) còn LAPIC timer của hart phụ ~1 ms ⇒ không có cổng thì ba hart phụ hút sạch
//!    hàng đợi (và trả/lấy lại) trước tick đầu của BSP, và `first[0]` có thể trùng `first[i]` một
//!    cách **hợp lệ** ⇒ phép kiểm "hart phụ chọn khác BSP" mất nghĩa. Cổng làm phép đo **tất
//!    định**: BSP giữ tác vụ đầu hàng đợi, mọi hart phụ **không thể** chọn lại nó ở lần đầu.
//!
//! [`MH_HARTS`] = 4, [`MH_TASKS`] = 4, [`MH_QUEUE_SLOTS`] = 16 (> 4 + 4), [`MH_MAX_ROUNDS`] = 8
//! (hai lượt mỗi hart). [`MH_HARD_ROUNDS`] **giữ nguyên** 48: nó là trần **số vòng** chống treo,
//! không phải ngưỡng của phép kiểm, và với bốn hart nó vẫn dư (đo được: pha kết thúc ở **8..15**
//! vòng, xa trần 48).

use crate::exc;

/// Số tác vụ **thật** của scheduler (không tính ngữ cảnh `main`): A, B, C (kernel) + U (ring 3).
///
/// **KT-315** — **bốn** tác vụ: tác vụ **C** là tác vụ thứ tư của pha nhiều hart (bốn hart cần
/// bốn tác vụ — xem đầu tệp).
pub const TASK_COUNT: usize = 4;

/// Slot ngữ cảnh của đường boot (`sched::run`) — nơi scheduler quay về để in tổng kết.
pub const TASK_MAIN: usize = 0;
/// Slot tác vụ **A** (kernel).
pub const TASK_A: usize = 1;
/// Slot tác vụ **B** (kernel).
pub const TASK_B: usize = 2;
/// Slot tác vụ **U** — chạy ở **ring 3** (KT-304).
pub const TASK_U: usize = 3;
/// **KT-315** — slot tác vụ **C** (kernel): tác vụ thứ tư, chỉ sống trong **pha nhiều hart**.
///
/// **Không** nằm trong [`TASK_SEQUENCE`] (chuỗi cố định của BSP vẫn là A/B/U) nên TAP 22/23/24
/// của KT-286/KT-304 **không** đổi nghĩa.
pub const TASK_C: usize = 4;
/// Tổng số slot ngữ cảnh: `main` + bốn tác vụ.
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

// ---------------------------------------------------------------------------------------------
// KT-310 — pha **nhiều hart**: hàng đợi chạy toàn cục + khoá
// ---------------------------------------------------------------------------------------------

/// Số hart **tham gia** pha nhiều hart: BSP (hart 0) + hart 1..3 — **bốn** hart.
///
/// **KT-315** mở từ **hai** lên **bốn** (K5' đòi "nhiều hart schedule" **như K3**, tức bốn hart).
/// Bốn là con số **đủ** để chứng minh "nhiều hart" ở mức K5' và **khớp** `smp::SMP_HARTS` của ảnh
/// (bốn hart đã online ở TAP 18/21 của KT-278) — không cần đánh thức thêm hart nào.
pub const MH_HARTS: usize = 4;

/// Hàng đợi chạy **toàn cục** của pha nhiều hart — **bốn** tác vụ của scheduler.
///
/// **KT-315** thêm [`TASK_C`]: bốn hart cần **bốn** tác vụ. Với ba tác vụ, hart thứ tư **đói**
/// (hàng đợi luôn có đúng một tác vụ rảnh và mỗi hart trả rồi lấy lại chính nó) — test host
/// `four_harts_need_four_tasks` chốt đúng điều kiện ấy.
pub const MH_TASKS: [usize; 4] = [TASK_A, TASK_B, TASK_U, TASK_C];

/// Số phần tử của hàng đợi **vòng** (chỉ số `head`/`tail` đơn điệu, lấy `% MH_QUEUE_SLOTS`).
///
/// Phải **rộng hơn** `MH_TASKS.len() + MH_HARTS`: mỗi hart đẩy tác vụ cũ về **trước** khi lấy tác
/// vụ mới, nên hàng đợi tạm thời chứa tới `4 + 4 = 8` phần tử. Đây là **sức chứa**, không phải
/// ngưỡng thời gian.
pub const MH_QUEUE_SLOTS: usize = 16;

/// Số vòng **mềm**: đủ để mỗi hart chọn ≥ 1 tác vụ **và** có ít nhất một lần chuyển tác vụ.
///
/// Với bốn hart, 8 vòng = 2 lượt mỗi hart ⇒ mỗi hart chạm **hai** tác vụ (có chuyển tác vụ thật).
pub const MH_MAX_ROUNDS: u64 = 8;

/// Trần **cứng** số vòng: hết số này thì pha kết thúc **dù** hart kia chưa chọn.
///
/// Đây là **trần số vòng**, không phải ngưỡng thời gian (bài học KT-301): boot **không** treo khi
/// một hart im lặng, và TAP nhiều hart **đỏ** vì thiếu bằng chứng — đúng fail-closed.
pub const MH_HARD_ROUNDS: u64 = 48;

/// Giá trị "**không** hart nào sở hữu tác vụ" trong bảng ownership.
///
/// `u64::MAX` không trùng slot nào (`TASK_SLOTS` = 5) và cũng là giá trị "chưa chọn" của
/// [`MH_FIRST`](crate) bên ảnh — một sentinel, hai cách dùng, không mơ hồ.
pub const MH_FREE: u64 = u64::MAX;

/// Tên tác vụ để in `# sched: hart=<n> picked=<task> n=<n>` — **chữ thường**, một ký tự.
///
/// Slot lạ trả `"?"` thay vì panic: dòng chẩn đoán phải in được **cả** trạng thái hỏng (cùng lý
/// do `bad_frame` của KT-298 đọc được frame rác).
pub const fn task_name(slot: usize) -> &'static str {
    if slot == TASK_A {
        return "a";
    }
    if slot == TASK_B {
        return "b";
    }
    if slot == TASK_U {
        return "u";
    }
    if slot == TASK_C {
        return "c";
    }
    "?"
}

/// `true` ⇔ **mọi** hart tham gia đã chọn ít nhất một tác vụ.
pub const fn mh_all_picked(picks: &[u64; MH_HARTS]) -> bool {
    let mut i = 0;
    while i < MH_HARTS {
        if picks[i] == 0 {
            return false;
        }
        i += 1;
    }
    true
}

/// `true` ⇔ pha nhiều hart phải **kết thúc** ở vòng `rounds` với `picks` hiện tại.
///
/// Hai điều kiện, theo thứ tự **ưu tiên**:
///
/// 1. `rounds >= MH_HARD_ROUNDS` ⇒ kết thúc **bất kể** `picks` — trần cứng, chống treo.
/// 2. `rounds >= MH_MAX_ROUNDS` **và** mọi hart đã chọn ⇒ kết thúc bình thường.
///
/// Hàm **thuần** nên test host chốt được cả hai nhánh, kể cả nhánh "hart kia im lặng".
pub const fn mh_end(rounds: u64, picks: &[u64; MH_HARTS]) -> bool {
    if rounds >= MH_HARD_ROUNDS {
        return true;
    }
    rounds >= MH_MAX_ROUNDS && mh_all_picked(picks)
}

/// Số tick **của BSP** BSP **giữ** tác vụ đầu của nó để chờ mọi hart chọn lần đầu.
///
/// **KT-315** — cổng khởi động (hart phụ chờ BSP chọn lần đầu) **không đủ**: nó chỉ bảo đảm hart
/// phụ chọn **sau** BSP, không bảo đảm BSP còn **giữ** tác vụ đầu hàng đợi lúc hart phụ chọn. Đo
/// được (boot 4/5 của lượt runner đầu): hart 1 bị host deschedule, BSP đã trả `a` ở tick 2 ⇒
/// hart 1 chọn `a` ⇒ `first=[a, a, b, u]` — **hợp lệ** nhưng làm phép kiểm "hart phụ chọn khác
/// BSP" đỏ oan. Vì vậy BSP giữ tác vụ của nó tối đa **bốn** tick của chính nó (≈ 40 ms với PIT
/// 100 Hz) — dư cho hart phụ chậm nhất (LAPIC timer ~1 ms) chọn lần đầu.
///
/// Đây là **cửa sổ chờ**, không phải ngưỡng của phép kiểm: hết hạn thì BSP trả tác vụ và chọn
/// lại mỗi tick ⇒ [`MH_ROUNDS`] tăng đều ⇒ [`MH_HARD_ROUNDS`] chắc chắn chạm (boot **không**
/// treo, kể cả khi không hart phụ nào chọn).
pub const MH_HOLD_TICKS: u64 = 4;

/// `true` ⇔ BSP phải **giữ** tác vụ đang chạy thay vì trả về hàng đợi.
///
/// `bsp_ticks` là số tick **của BSP** đã nhận trong pha (đếm cả tick không chọn được tác vụ).
/// Hàm **thuần** nên test host chốt được cả hai nhánh: còn hạn **và** chưa đủ hart ⇒ giữ; hết
/// hạn **hoặc** mọi hart đã chọn ⇒ trả.
pub const fn mh_hold(bsp_ticks: u64, picks: &[u64; MH_HARTS]) -> bool {
    bsp_ticks <= MH_HOLD_TICKS && !mh_all_picked(picks)
}

/// Số hart **đã chọn** ít nhất một tác vụ — giá trị của `harts=` trong dòng tổng hợp.
///
/// **KT-315** — `harts=` phải là **số đo**, không phải hằng [`MH_HARTS`]: đột biến "chỉ 2 hart"
/// (`--features two-hart-sched`) in `harts=2` và runner **đỏ**. Nếu in hằng thì đột biến ấy không
/// thể bị bắt — phép kiểm sẽ là dấu cao su.
pub const fn mh_harts_picked(picks: &[u64; MH_HARTS]) -> u64 {
    let mut n = 0;
    let mut i = 0;
    while i < MH_HARTS {
        if picks[i] != 0 {
            n += 1;
        }
        i += 1;
    }
    n
}

/// `true` ⇔ pha nhiều hart **đủ bằng chứng** để TAP `sched: multi-hart` xanh.
///
/// Bốn điều kiện, mỗi điều kiện là một lớp lỗi **khác** nhau:
///
/// 1. `double == 0` — không có hai hart nào nhận **cùng** một tác vụ (khoá + ownership có tác
///    dụng). Đột biến `--features no-sched-lock` làm điều kiện này sai.
/// 2. `rounds >= MH_HARTS` — có **ít nhất** một lượt chọn mỗi hart (tổng), tức không phải "một
///    hart chọn một lần rồi thôi".
/// 3. **mọi** hart đã chọn ≥ 1 — đột biến `--features two-hart-sched` (chỉ BSP + hart 1) làm
///    điều kiện này sai.
/// 4. `first[0] != first[i]` với **mọi** `i ≥ 1` — BSP chọn tác vụ **khác** mọi hart phụ ở lần
///    đầu. Đây là bằng chứng **trực tiếp** của "hart phụ chọn được tác vụ khác BSP" (mốc thật của
///    KT-310/KT-315), mạnh hơn một bộ đếm chung. **Hai** cơ chế làm điều kiện này **tất định**:
///    cổng khởi động (hart phụ chờ BSP chọn lần đầu) **và** cổng giữ ([`mh_hold`]: BSP giữ tác
///    vụ đầu của nó cho tới khi mọi hart đã chọn, tối đa [`MH_HOLD_TICKS`] tick).
pub const fn mh_ok(
    rounds: u64,
    picks: &[u64; MH_HARTS],
    first: &[u64; MH_HARTS],
    double: u64,
) -> bool {
    if double != 0 {
        return false;
    }
    if rounds < MH_HARTS as u64 {
        return false;
    }
    if !mh_all_picked(picks) {
        return false;
    }
    let mut i = 1;
    while i < MH_HARTS {
        if first[i] == first[0] {
            return false;
        }
        i += 1;
    }
    true
}
