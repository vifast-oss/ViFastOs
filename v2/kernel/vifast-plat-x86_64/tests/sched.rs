// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **preemption x86_64** — KT-286 (K5' bước 8).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test sched
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-286: mô-đun [`vifast_plat_x86_64::sched`] chưa
//! tồn tại khi tệp này được viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do).
//! Sau khi hiện thực, các khẳng định dưới đây chốt **giá trị tuyệt đối** mà asm của ảnh
//! `vifast-kbin-x86_64` dùng làm `const` operand — nếu ai đổi một bit, test đỏ **trước** khi có
//! ảnh boot.
//!
//! # Vì sao frame khởi tạo có **22** qword
//!
//! Stub chung (`vifast_isr_common`) đẩy 15 GPR rồi `call exc_dispatch`; khi trả về nó `pop` 15
//! GPR, `add rsp, 16` (bỏ `vector` + `mã lỗi`) rồi `iretq` (bỏ `rip`/`cs`/`rflags`). Một frame
//! **khởi tạo** cho tác vụ mới phải có đúng bố cục ấy để `iretq` nhảy vào entry của tác vụ với
//! `rsp` trỏ vào stack riêng của nó. `vector`/`mã lỗi` không được `iretq` đọc, nhưng vẫn phải
//! nằm trong frame vì `add rsp, 16` bỏ qua chúng.
//!
//! **Hai** qword cuối (`rsp`, `ss`) là cho QEMU 11.1.3: `helper_ret_protected` trong
//! `target/i386/tcg/seg_helper.c` dùng điều kiện `rpl == cpl && (!(HF_CS64) || (HF_CS64 &&
//! !is_iret))`, nên trong 64-bit mode `iretq` **luôn** pop `SS:RSP` kể cả khi cùng CPL. Frame
//! 20 qword làm `rsp` lấy từ ô ngay sau frame (0) ⇒ `#PF`/`#DF`. Trên CPU thật (pop 3 theo SDM)
//! hai ô ấy không được đọc và `rsp` sau `iretq` chỉ hụt 16 byte — vẫn trong stack.
//!
//! # Vì sao `rsp` khởi tạo lệch **8 byte**
//!
//! SysV AMD64 đòi `rsp % 16 == 8` ngay sau `call` (địa chỉ trả về đã đẩy). Tác vụ mới được vào
//! bằng `iretq` chứ không bằng `call`, nên ảnh tự tạo đúng trạng thái ấy: frame kết thúc ở
//! `stack_top - 8`, tức `rsp` sau `iretq` là `stack_top - 8` ⇒ `% 16 == 8` khi `stack_top` căn 16.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà asm của
// ảnh boot nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::exc::{
    FRAME_CS_OFFSET, FRAME_ERROR_OFFSET, FRAME_GPR_COUNT, FRAME_RFLAGS_OFFSET, FRAME_RIP_OFFSET,
    FRAME_VECTOR_OFFSET, KERNEL_CS,
};
use vifast_plat_x86_64::sched::{
    init_frame_words, init_user_frame_words, next_task, switch_frame_ok, FrameReturn, FRAME_BYTES,
    FRAME_WORDS, INITIAL_RSP_SLACK, MAX_SWITCHES, TASK_A, TASK_B, TASK_C, TASK_COUNT, TASK_MAIN,
    TASK_RFLAGS, TASK_SEQUENCE, TASK_SLOTS, TASK_U,
};
use vifast_plat_x86_64::umode::{
    USER_CODE_VA, USER_CS, USER_RFLAGS, USER_SS, USER_STACK_TOP, USER_STACK_VA,
};

/// Stack top giả căn 16 byte (đúng như `#[repr(align(16))]` của stack tác vụ trong ảnh).
const STACK_TOP: u64 = 0x0000_0000_0020_0000;
/// Entry giả của tác vụ.
const ENTRY: u64 = 0x0000_0000_0010_1234;
/// Đỉnh stack **kernel** của tác vụ ring 3 — nơi frame khởi tạo 22 qword nằm.
const U_KERNEL_TOP: u64 = 0x0000_0000_0030_0000;
/// `rsp` **user** của tác vụ ring 3 — chừa 16 byte trên đỉnh trang stack user cho bộ đếm.
const U_RSP: u64 = USER_STACK_TOP - 16;
/// Entry **user** giả — phải nằm trong trang mã user (khác `ENTRY` của tác vụ kernel).
const USER_ENTRY: u64 = USER_CODE_VA + 0x40;

/// Frame khởi tạo: **22** qword, `vector`/`err` = 0, `rip` = entry, `cs` = kernel code,
/// `rflags` = [`TASK_RFLAGS`], `rsp`/`ss` cho `iretq` pop 5 của QEMU; 15 GPR = 0.
#[test]
fn init_frame_layout_literal() {
    assert_eq!(FRAME_WORDS, 22);
    assert_eq!(FRAME_BYTES, 176);
    assert_eq!(INITIAL_RSP_SLACK, 8);

    let (words, rsp) = init_frame_words(STACK_TOP, ENTRY, TASK_RFLAGS);
    assert_eq!(words.len(), FRAME_WORDS);

    // 15 GPR đầu = 0 (tác vụ mới không thừa hưởng thanh ghi của tác vụ trước).
    for (i, word) in words.iter().take(FRAME_GPR_COUNT).enumerate() {
        assert_eq!(*word, 0, "GPR thứ {i} phải 0");
    }
    // `vector`/`err` = 0 — stub bỏ qua 16 byte này bằng `add rsp, 16`.
    assert_eq!(words[15], 0, "vector phải 0");
    assert_eq!(words[16], 0, "mã lỗi phải 0");
    // `rip`/`cs`/`rflags` — ba trường `iretq` đọc.
    assert_eq!(words[17], ENTRY, "rip phải là entry của tác vụ");
    assert_eq!(words[18], KERNEL_CS, "cs phải là kernel code");
    assert_eq!(words[19], TASK_RFLAGS, "rflags phải bật IF");
    // `rsp`/`ss` — hai trường QEMU pop thêm (xem đầu tệp).
    assert_eq!(
        words[20],
        STACK_TOP - INITIAL_RSP_SLACK,
        "rsp cho iretq pop 5"
    );
    assert_eq!(words[21], 0x10, "ss cho iretq pop 5");

    // `rsp` khởi tạo: frame kết thúc ở `stack_top - 8`.
    assert_eq!(rsp, STACK_TOP - FRAME_BYTES as u64 - INITIAL_RSP_SLACK);
    assert_eq!(rsp % 16, 8, "rsp khởi tạo phải % 16 == 8 (SysV sau call)");
    // QEMU pop 5: `rsp` sau `iretq` = ô `rsp` trong frame = `stack_top - 8` (đúng SysV).
    assert_eq!(words[20] % 16, 8, "rsp sau iretq (QEMU) phải % 16 == 8");
    // CPU thật pop 3: `rsp` sau `iretq` = `frame + 160` = `stack_top - 24` — vẫn trong stack.
    assert_eq!(rsp + 160, STACK_TOP - 24);
    assert!(
        rsp + 160 < STACK_TOP,
        "rsp sau iretq (CPU thật) phải dưới stack_top"
    );
}

/// Offset của `rip`/`cs`/`rflags` trong frame **khớp** hằng của `exc` — asm của ảnh dùng chung
/// các hằng ấy, nên lệch một ô là `iretq` nhảy sai chỗ.
#[test]
fn init_frame_offsets_match_exc() {
    let (words, rsp) = init_frame_words(STACK_TOP, ENTRY, TASK_RFLAGS);
    // Chỉ số qword trong frame ↔ offset byte mà stub dùng.
    assert_eq!(FRAME_VECTOR_OFFSET, 15 * 8);
    assert_eq!(FRAME_ERROR_OFFSET, 16 * 8);
    assert_eq!(FRAME_RIP_OFFSET, 17 * 8);
    assert_eq!(FRAME_CS_OFFSET, 18 * 8);
    assert_eq!(FRAME_RFLAGS_OFFSET, 19 * 8);
    // Hai ô QEMU pop thêm nằm ngay sau `rflags`.
    assert_eq!(FRAME_RFLAGS_OFFSET + 8, 160);
    assert_eq!(FRAME_RFLAGS_OFFSET + 16, 168);
    // Giá trị tại đúng offset ấy (đọc từ mảng, không từ bộ nhớ).
    assert_eq!(words[FRAME_RIP_OFFSET / 8], ENTRY);
    assert_eq!(words[FRAME_CS_OFFSET / 8], KERNEL_CS);
    assert_eq!(words[FRAME_RFLAGS_OFFSET / 8], TASK_RFLAGS);
    // `rsp` trỏ vào **đầu** frame (ô `r15`), không phải giữa frame.
    assert_eq!(rsp % 8, 0);
}

/// `TASK_RFLAGS` bật `IF` (bit 9) và bit dự phòng 1 — tác vụ phải bị ngắt được, nếu không
/// preemption không bao giờ xảy ra.
#[test]
fn task_rflags_enables_interrupts() {
    assert_eq!(TASK_RFLAGS & (1 << 9), 1 << 9, "IF phải bật");
    assert_eq!(TASK_RFLAGS & 0x2, 0x2, "bit dự phòng 1 phải đặt");
    assert_eq!(TASK_RFLAGS, 0x202);
}

/// **KT-304** — thứ tự tác vụ có **ba** slot: A/B (kernel) + **U (ring 3)**; sau `max_switches`
/// lần chuyển thì trả về **main** (để `sched::run` in tổng kết rồi nhường lại đường boot).
///
/// Bảng `TASK_SEQUENCE` là **nguồn sự thật** của thứ tự (không còn luân phiên thuần): nó phải
/// giữ **cả hai** chiều A↔B (TAP 23 của KT-286) **và** chọn U **≥ 2** lần (preempt rồi chạy lại).
#[test]
fn next_task_sequence_includes_umode() {
    // **KT-315** — **bốn** tác vụ: ba tác vụ kernel (A, B, C) + một tác vụ ring 3 (U). Tác vụ
    // **C** là tác vụ thứ tư của **pha nhiều hart**: bốn hart cần **bốn** tác vụ, nếu không hart
    // thứ tư **đói** (hàng đợi luôn có đúng một tác vụ rảnh, mỗi hart trả rồi lấy lại chính nó).
    assert_eq!(TASK_COUNT, 4, "ba tác vụ kernel + một tác vụ ring 3");
    assert_eq!(TASK_SLOTS, 5, "main + A + B + U + C");
    assert_eq!(TASK_MAIN, 0);
    assert_eq!(TASK_A, 1);
    assert_eq!(TASK_B, 2);
    assert_eq!(TASK_U, 3);
    assert_eq!(TASK_C, 4);

    assert_eq!(MAX_SWITCHES, 9, "bảng có 9 bước (bước cuối về main)");
    assert_eq!(TASK_SEQUENCE.len(), MAX_SWITCHES as usize);
    // Sau tick đầu (main→A): A→B→A→B→A→B→U→A→U→main.
    assert_eq!(next_task(TASK_A, 1, MAX_SWITCHES), TASK_B);
    assert_eq!(next_task(TASK_B, 2, MAX_SWITCHES), TASK_A);
    assert_eq!(next_task(TASK_A, 3, MAX_SWITCHES), TASK_B);
    assert_eq!(next_task(TASK_B, 4, MAX_SWITCHES), TASK_A);
    assert_eq!(next_task(TASK_A, 5, MAX_SWITCHES), TASK_B);
    assert_eq!(next_task(TASK_B, 6, MAX_SWITCHES), TASK_U);
    assert_eq!(next_task(TASK_U, 7, MAX_SWITCHES), TASK_A);
    assert_eq!(next_task(TASK_A, 8, MAX_SWITCHES), TASK_U);
    // Chạm trần: về main.
    assert_eq!(next_task(TASK_U, MAX_SWITCHES, MAX_SWITCHES), TASK_MAIN);
    assert_eq!(next_task(TASK_A, MAX_SWITCHES + 1, MAX_SWITCHES), TASK_MAIN);
    // `current` lạ (main) vẫn khởi động A — không bao giờ trả về chính nó.
    assert_eq!(next_task(TASK_MAIN, 0, MAX_SWITCHES), TASK_A);
    // **KT-315** — `TASK_C` **không** nằm trong bảng cố định (nó chỉ sống trong pha nhiều hart),
    // nên nếu ai đó đưa nó vào `next_task` thì hàm phải **khởi động lại A** thay vì chỉ số bảng
    // ngoài phạm vi (bảng chỉ có A/B/U/MAIN).
    assert_eq!(next_task(TASK_C, 1, MAX_SWITCHES), TASK_A);

    // Bất biến của bảng: cả hai chiều A↔B **và** U được chọn ≥ 2 lần (preempt + chạy lại).
    let mut ab = 0;
    let mut ba = 0;
    let mut u = 0;
    let mut prev = TASK_A; // sau tick đầu
    for &next in TASK_SEQUENCE.iter() {
        if prev == TASK_A && next == TASK_B {
            ab += 1;
        }
        if prev == TASK_B && next == TASK_A {
            ba += 1;
        }
        if next == TASK_U {
            u += 1;
        }
        prev = next;
    }
    assert!(ab >= 1, "bảng phải có A→B (TAP 23)");
    assert!(ba >= 1, "bảng phải có B→A (TAP 23)");
    assert!(u >= 2, "U phải được chọn ≥ 2 lần (preempt rồi chạy lại)");
}

/// **KT-298** — bàn giao `rsp` đi qua **thanh ghi** (`rax`/`rdx`), không qua ô `static` dùng
/// chung mọi hart.
///
/// Test này chốt **bố cục** mà stub asm dựa vào: `#[repr(C)]` hai `u64` ⇒ SysV trả ở `RAX`
/// (trường 1) và `RDX` (trường 2). Nếu ai đổi thứ tự trường hoặc thêm trường, stub sẽ đọc sai
/// thanh ghi — và đó là lỗi **chỉ hiện khi boot**, nên phải chốt ở host.
#[test]
fn frame_return_layout_is_rax_rdx() {
    assert_eq!(core::mem::size_of::<FrameReturn>(), 16, "hai qword");
    assert_eq!(core::mem::align_of::<FrameReturn>(), 8);
    // Thứ tự trường = thứ tự thanh ghi: `rip` ở `RAX`, `new_rsp` ở `RDX`.
    assert_eq!(core::mem::offset_of!(FrameReturn, rip), 0);
    assert_eq!(core::mem::offset_of!(FrameReturn, new_rsp), 8);

    // `resume` = không đổi `rsp` (đường exception/ngắt thường).
    let r = FrameReturn::resume(0x0000_0000_0010_1234);
    assert_eq!(r.rip, 0x0000_0000_0010_1234);
    assert_eq!(r.new_rsp, 0, "resume không được đổi rsp");
    assert!(!r.is_halt());

    // `switch` = mang `rsp` frame tác vụ kế.
    let s = FrameReturn::switch(0x0000_0000_0020_0000, 0x0000_0000_0010_5678);
    assert_eq!(s.new_rsp, 0x0000_0000_0020_0000);
    assert_eq!(s.rip, 0x0000_0000_0010_5678);
    assert!(!s.is_halt());

    // `rip = 0` ⇒ stub halt sạch (không quay lại chỗ lỗi).
    let h = FrameReturn::resume(0);
    assert!(h.is_halt());
    assert_eq!(h.new_rsp, 0);
}

/// **KT-298** — phép kiểm frame **trước** khi stub `mov rsp, rdx`: bốn điều kiện, mỗi điều kiện
/// là một lớp lỗi thật. Ca dương dùng đúng giá trị ảnh ghi cho tác vụ mới.
#[test]
fn switch_frame_ok_accepts_task_frame() {
    let (words, _rsp) = init_frame_words(STACK_TOP, ENTRY, TASK_RFLAGS);
    let rip = words[FRAME_RIP_OFFSET / 8];
    let cs = words[FRAME_CS_OFFSET / 8];
    let rflags = words[FRAME_RFLAGS_OFFSET / 8];
    let task_rsp = words[20];
    assert!(
        switch_frame_ok(rip, cs, rflags, task_rsp, 0x10),
        "frame khởi tạo hợp lệ phải qua"
    );
}

/// **KT-304** — frame khởi tạo của tác vụ **ring 3**: 22 qword, `cs = 0x1B`, `ss = 0x23`,
/// `rsp` = user rsp; frame kết thúc **đúng** đỉnh stack kernel (không slack) để lần ngắt đầu
/// tiên từ CPL3 đẩy frame vào **đúng** chỗ ấy.
#[test]
fn init_user_frame_layout_literal() {
    let (words, rsp) = init_user_frame_words(U_KERNEL_TOP, USER_ENTRY, USER_RFLAGS, U_RSP);
    assert_eq!(words.len(), FRAME_WORDS);

    // 15 GPR = 0; vector/mã lỗi = 0 (stub bỏ qua 16 byte).
    for (i, word) in words.iter().take(FRAME_GPR_COUNT).enumerate() {
        assert_eq!(*word, 0, "GPR thứ {i} phải 0");
    }
    assert_eq!(words[15], 0, "vector phải 0");
    assert_eq!(words[16], 0, "mã lỗi phải 0");
    // Năm trường `iretq` đọc — **khác** frame kernel ở `cs`/`ss`/`rsp`.
    assert_eq!(words[17], USER_ENTRY, "rip phải là entry ring 3");
    assert_eq!(words[18], USER_CS as u64, "cs phải user code 0x1B");
    assert_eq!(words[19], USER_RFLAGS, "rflags phải bật IF");
    assert_eq!(words[20], U_RSP, "rsp phải là user rsp");
    assert_eq!(words[21], USER_SS as u64, "ss phải user data 0x23");

    // Frame kết thúc đúng đỉnh stack kernel: sau `iretq` (pop 5) kernel `rsp` = top.
    assert_eq!(rsp, U_KERNEL_TOP - FRAME_BYTES as u64);
    assert_eq!(rsp % 8, 0);
    assert_eq!(rsp + FRAME_BYTES as u64, U_KERNEL_TOP);
    // Timer ngắt từ CPL3: CPU đẩy 5 qword rồi stub đẩy 17 ⇒ frame mới ở **đúng** chỗ này.
    assert_eq!(U_KERNEL_TOP - 5 * 8 - 17 * 8, rsp);
}

/// **KT-304** — `switch_frame_ok` phải nhận **cả** frame ring 3 (ca dương), và `ss` của frame
/// kernel thật (rác, nằm ngoài frame) **không** được làm nó từ chối.
#[test]
fn switch_frame_ok_accepts_user_frame() {
    let (words, _rsp) = init_user_frame_words(U_KERNEL_TOP, USER_ENTRY, USER_RFLAGS, U_RSP);
    let rip = words[FRAME_RIP_OFFSET / 8];
    let cs = words[FRAME_CS_OFFSET / 8];
    let rflags = words[FRAME_RFLAGS_OFFSET / 8];
    let user_rsp = words[20];
    let ss = words[21];
    assert!(
        switch_frame_ok(rip, cs, rflags, user_rsp, ss),
        "frame ring 3 hợp lệ phải qua"
    );
    // Frame kernel: `ss` đọc từ ô 168 của frame thật là **rác** (QEMU pop thêm) ⇒ phải bỏ qua.
    let (kw, _) = init_frame_words(STACK_TOP, ENTRY, TASK_RFLAGS);
    assert!(
        switch_frame_ok(kw[17], kw[18], kw[19], kw[20], 0xDEAD_BEEF),
        "frame kernel phải qua dù ss rác"
    );
}

/// **KT-304** — từng điều kiện của nhánh **ring 3** phải **có răng**: `ss` sai, `rsp` ngoài trang
/// stack user, `rip` ngoài trang mã user, `cs` lạ, thiếu `IF` — mỗi thứ phải bị từ chối.
#[test]
fn switch_frame_ok_rejects_bad_user_frame() {
    let (words, _rsp) = init_user_frame_words(U_KERNEL_TOP, USER_ENTRY, USER_RFLAGS, U_RSP);
    let rip = words[FRAME_RIP_OFFSET / 8];
    let cs = words[FRAME_CS_OFFSET / 8];
    let rflags = words[FRAME_RFLAGS_OFFSET / 8];
    let user_rsp = words[20];
    let ss = words[21];

    // (1) `ss` kernel ⇒ không phải frame ring 3.
    assert!(
        !switch_frame_ok(rip, cs, rflags, user_rsp, 0x10),
        "ss kernel phải bị từ chối"
    );
    // (2) `rsp` kernel (ngoài trang stack user) ⇒ không phải frame ring 3.
    assert!(
        !switch_frame_ok(rip, cs, rflags, STACK_TOP - 8, ss),
        "rsp kernel phải bị từ chối"
    );
    // (3) `rsp` dưới đáy / trên đỉnh trang stack user.
    assert!(
        !switch_frame_ok(rip, cs, rflags, USER_STACK_VA - 8, ss),
        "rsp dưới stack user phải bị từ chối"
    );
    assert!(
        !switch_frame_ok(rip, cs, rflags, USER_STACK_TOP + 8, ss),
        "rsp trên stack user phải bị từ chối"
    );
    // (4) `rip` ngoài trang mã user (ví dụ `.text` kernel) ⇒ không phải frame ring 3.
    assert!(
        !switch_frame_ok(0x0000_0000_0010_0000, cs, rflags, user_rsp, ss),
        "rip kernel phải bị từ chối"
    );
    assert!(
        !switch_frame_ok(USER_CODE_VA + 0x1000, cs, rflags, user_rsp, ss),
        "rip ngoài trang mã user phải bị từ chối"
    );
    // (5) `cs` lạ (kernel data | RPL 3) ⇒ từ chối.
    assert!(
        !switch_frame_ok(rip, 0x13, rflags, user_rsp, ss),
        "cs lạ phải bị từ chối"
    );
    // (6) thiếu `IF` ⇒ tác vụ không bị preempt.
    assert!(
        !switch_frame_ok(rip, cs, USER_RFLAGS & !(1 << 9), user_rsp, ss),
        "thiếu IF phải bị từ chối"
    );
}

/// **KT-298** — từng điều kiện của [`switch_frame_ok`] phải **có răng**: phá đúng một trường thì
/// phép kiểm phải từ chối. Đây là ca âm của lỗi boot 410 (`#GP(0)` tại `iretq` vì `rip` hỏng).
#[test]
fn switch_frame_ok_rejects_each_bad_field() {
    let (words, _rsp) = init_frame_words(STACK_TOP, ENTRY, TASK_RFLAGS);
    let rip = words[FRAME_RIP_OFFSET / 8];
    let cs = words[FRAME_CS_OFFSET / 8];
    let rflags = words[FRAME_RFLAGS_OFFSET / 8];
    let task_rsp = words[20];

    // (1) `rip` = 0 ⇒ không có chỗ để `iretq` về.
    assert!(
        !switch_frame_ok(0, cs, rflags, task_rsp, 0x10),
        "rip=0 phải bị từ chối"
    );
    // (1b) `rip` **không canonical** ⇒ `iretq` sinh `#GP(0)` — đúng dấu hiệu boot 410.
    assert!(
        !switch_frame_ok(0x0000_8000_0000_0000, cs, rflags, task_rsp, 0x10),
        "rip không canonical phải bị từ chối"
    );
    // (2) `cs` khác kernel code **và** khác user code ⇒ `#GP` có mã lỗi là selector.
    assert!(
        !switch_frame_ok(rip, 0x13, rflags, task_rsp, 0x10),
        "cs lạ phải bị từ chối"
    );
    // (2b) `cs` user nhưng `rsp` kernel ⇒ không phải frame ring 3 (KT-304).
    assert!(
        !switch_frame_ok(rip, 0x1B, rflags, task_rsp, 0x10),
        "cs user + rsp kernel phải bị từ chối"
    );
    // (3) thiếu `IF` ⇒ tác vụ không bao giờ bị preempt (preemption chết im lặng).
    assert!(
        !switch_frame_ok(rip, cs, TASK_RFLAGS & !(1 << 9), task_rsp, 0x10),
        "rflags thiếu IF phải bị từ chối"
    );
    // (4) `rsp` hỏng: 0, lệch căn 8, hoặc không canonical.
    assert!(
        !switch_frame_ok(rip, cs, rflags, 0, 0x10),
        "rsp=0 phải bị từ chối"
    );
    assert!(
        !switch_frame_ok(rip, cs, rflags, task_rsp + 4, 0x10),
        "rsp lệch căn 8 phải bị từ chối"
    );
    assert!(
        !switch_frame_ok(rip, cs, rflags, 0xFFFF_8000_0000_0000, 0x10),
        "rsp không canonical phải bị từ chối"
    );
}
