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
    init_frame_words, next_task, set_new_rsp, take_new_rsp, FRAME_BYTES, FRAME_WORDS,
    INITIAL_RSP_SLACK, MAX_SWITCHES, TASK_A, TASK_B, TASK_COUNT, TASK_MAIN, TASK_RFLAGS,
    TASK_SLOTS,
};

/// Stack top giả căn 16 byte (đúng như `#[repr(align(16))]` của stack tác vụ trong ảnh).
const STACK_TOP: u64 = 0x0000_0000_0020_0000;
/// Entry giả của tác vụ.
const ENTRY: u64 = 0x0000_0000_0010_1234;

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

/// Luân phiên A → B → A; sau `max_switches` lần chuyển thì trả về **main** (để `sched::run`
/// in tổng kết rồi nhường lại đường boot).
#[test]
fn next_task_round_robin() {
    assert_eq!(TASK_COUNT, 2);
    assert_eq!(TASK_SLOTS, 3);
    assert_eq!(TASK_MAIN, 0);
    assert_eq!(TASK_A, 1);
    assert_eq!(TASK_B, 2);

    assert_eq!(
        MAX_SWITCHES, 6,
        "số chuyển phải chẵn để cả hai chiều có bằng chứng"
    );
    // Chưa chạm trần: luân phiên.
    assert_eq!(next_task(TASK_A, 1, MAX_SWITCHES), TASK_B);
    assert_eq!(next_task(TASK_B, 2, MAX_SWITCHES), TASK_A);
    assert_eq!(next_task(TASK_A, 3, MAX_SWITCHES), TASK_B);
    assert_eq!(next_task(TASK_B, 4, MAX_SWITCHES), TASK_A);
    assert_eq!(next_task(TASK_A, 5, MAX_SWITCHES), TASK_B);
    // Chạm trần: về main.
    assert_eq!(next_task(TASK_B, MAX_SWITCHES, MAX_SWITCHES), TASK_MAIN);
    assert_eq!(next_task(TASK_A, MAX_SWITCHES + 1, MAX_SWITCHES), TASK_MAIN);
    // `current` lạ (main) vẫn khởi động A — không bao giờ trả về chính nó.
    assert_eq!(next_task(TASK_MAIN, 0, MAX_SWITCHES), TASK_A);
}

/// Ô "đổi `rsp` trước `iretq`": đặt rồi lấy **một lần**; lấy lần hai trả `0` (không đổi `rsp`).
#[test]
fn new_rsp_set_then_take() {
    // Trạng thái đầu: 0 = "không đổi rsp".
    assert_eq!(take_new_rsp(), 0);
    set_new_rsp(0x0000_0000_0020_0000);
    assert_eq!(take_new_rsp(), 0x0000_0000_0020_0000);
    assert_eq!(take_new_rsp(), 0, "lấy lần hai phải trả 0");
    // Đặt lại 0 là no-op.
    set_new_rsp(0);
    assert_eq!(take_new_rsp(), 0);
}
