// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **priority tĩnh + sleep/wake** — KT-318 (K5' bước 11).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test priority
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-318: các mục [`prio_pick`], [`prio_pick_fifo`],
//! [`prio_ready`], [`prio_wake_at`], [`prio_violation`], [`prio_end`], [`prio_ok`], [`sleep_ok`]
//! và các hằng `PRIO_*`/`SLEEP_*` **chưa** tồn tại khi tệp này được viết, nên lượt chạy đầu
//! **không biên dịch được** (đỏ đúng lý do). Sau khi hiện thực, các khẳng định dưới đây chốt
//! **giá trị tuyệt đối** mà ảnh `vifast-kbin-x86_64` dùng, và chốt **bốn** bất biến:
//!
//! 1. **Priority**: lượt chọn **đầu** (cả bốn tác vụ sẵn sàng) là tác vụ **ưu tiên cao nhất**
//!    (`b`, priority 3), **không** phải đầu hàng đợi FIFO (`a`) — đột biến `--features no-prio`
//!    dùng [`prio_pick_fifo`] và chọn `a`, nên phép kiểm **có răng**.
//! 2. **Sleep**: tác vụ có `WakeAt > tick` **không** được chọn; `sleep(0)` thức **ngay** (cùng
//!    tick) — đột biến `--features no-wake` biến `sleep(0)` thành "không bao giờ thức".
//! 3. **Không phụ thuộc thời điểm**: chuỗi lượt chọn là **hàm thuần của số tick** (mô phỏng
//!    [`simulate`] dưới đây), không phụ thuộc hart nào tick trước hay host có deschedule hay
//!    không — bài học KT-308/KT-313 (đã lặp ba lần trong tuần).
//! 4. **Trần cứng**: pha kết thúc ở [`PRIO_HARD_TICKS`] **dù** thiếu bằng chứng ⇒ boot **không**
//!    treo khi một tác vụ không bao giờ thức (đột biến `no-wake` ⇒ ĐỎ, không treo).
//!
//! # Vì sao mô hình nằm **trong test**, không trong crate
//!
//! Hàng đợi thật là `AtomicU64` trong ảnh (không test host được: cần ngắt/MMIO). Phần **thuần**
//! đáng chốt là **luật**: chọn ai ([`prio_pick`]), ai sẵn sàng ([`prio_ready`]), khi nào pha kết
//! thúc ([`prio_end`]), khi nào đủ bằng chứng ([`prio_ok`]/[`sleep_ok`]). Mô hình dưới đây dùng
//! **đúng** luật ấy nên nếu luật sai thì test đỏ, còn cơ chế nguyên tử của ảnh thì đo bằng QEMU.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà ảnh boot
// nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::sched::{
    prio_end, prio_ok, prio_pick, prio_pick_fifo, prio_ready, prio_violation, prio_wake_at,
    sleep_ok, sleep_ticks, task_priority, PRIO_A, PRIO_B, PRIO_C, PRIO_HARD_TICKS, PRIO_MIN_TICKS,
    PRIO_ORDER, PRIO_TASKS, PRIO_U, SLEEP_A, SLEEP_B, SLEEP_C, SLEEP_U, TASK_A, TASK_B, TASK_C,
    TASK_MAIN, TASK_SLOTS, TASK_U,
};

/// Bảng `WakeAt` "không tác vụ nào ngủ" — `0` là sentinel "không ngủ" (xem [`prio_ready`]).
const AWAKE: [u64; TASK_SLOTS] = [0; TASK_SLOTS];

/// **Mô phỏng** pha priority/sleep bằng **đúng** các hàm thuần của crate — không thời gian, không
/// hart, không host: đầu vào duy nhất là số tick.
///
/// Trả `(chuỗi lượt chọn, số lần thức, số tick đã chạy)`. Luật ngủ giống ảnh: tác vụ **rời CPU ở
/// tick kế** mới ghi `WakeAt = (tick + 1) + sleep_ticks` (ghi lúc chạy lệnh `sleep` sẽ **phụ
/// thuộc thời điểm** — tác vụ có thể bị preempt trước lệnh đầu, bài học KT-308).
fn simulate() -> (Vec<usize>, u64, u64) {
    let mut wake_at = AWAKE;
    let mut picks = [0u64; TASK_SLOTS];
    let mut wakes = 0u64;
    let mut seq = Vec::new();
    let mut tick = 0u64;
    while !prio_end(tick, &picks, wakes) {
        let next = prio_pick(&wake_at, tick).expect("pha phải luôn có tác vụ sẵn sàng");
        // Thức: tác vụ **đã ngủ** (`WakeAt != 0`) và nay `WakeAt <= tick`.
        if wake_at[next] != 0 {
            wakes += 1;
            wake_at[next] = 0;
        }
        picks[next] += 1;
        seq.push(next);
        // Ngủ: ghi lúc tác vụ **rời CPU** (tick kế), không phải lúc được chọn.
        wake_at[next] = prio_wake_at(tick + 1, sleep_ticks(next));
        tick += 1;
    }
    (seq, wakes, tick)
}

/// Bảng priority/sleep là **hằng số tuyệt đối** — ảnh in đúng bảng này ở dòng
/// `# sched: prio a=… b=… c=… u=… picked=…`, runner so **nguyên văn**.
#[test]
fn priority_table_is_literal() {
    // Bốn tác vụ của hàng đợi ưu tiên: A, B, C (kernel) + U (ring 3).
    assert_eq!(PRIO_TASKS, 4);
    assert_eq!(PRIO_ORDER, [TASK_A, TASK_B, TASK_C, TASK_U]);
    // Priority **tĩnh**: B cao nhất (3), rồi C (2), A (1), U thấp nhất (0).
    assert_eq!(PRIO_A, 1);
    assert_eq!(PRIO_B, 3);
    assert_eq!(PRIO_C, 2);
    assert_eq!(PRIO_U, 0);
    assert_eq!(task_priority(TASK_A), PRIO_A);
    assert_eq!(task_priority(TASK_B), PRIO_B);
    assert_eq!(task_priority(TASK_C), PRIO_C);
    assert_eq!(task_priority(TASK_U), PRIO_U);
    // Slot không phải tác vụ (`main`) và slot lạ: priority thấp nhất, **không** panic — dòng
    // chẩn đoán phải in được cả trạng thái hỏng (cùng lý do `task_name`).
    assert_eq!(task_priority(TASK_MAIN), 0);
    assert_eq!(task_priority(TASK_SLOTS), 0);
    assert_eq!(task_priority(usize::MAX), 0);
    // Bảng priority **không** trùng thứ tự FIFO: FIFO là A đầu, priority là B đầu. Đó chính là
    // khác biệt mà đột biến `--features no-prio` khai thác.
    assert_ne!(PRIO_ORDER[0], TASK_B);

    // Số tick ngủ **sau mỗi lần chạy**: U ngủ **0** tick (yield) — ca `sleep(0)` của task.
    assert_eq!(SLEEP_A, 2);
    assert_eq!(SLEEP_B, 3);
    assert_eq!(SLEEP_C, 2);
    assert_eq!(SLEEP_U, 0);
    assert_eq!(sleep_ticks(TASK_A), SLEEP_A);
    assert_eq!(sleep_ticks(TASK_B), SLEEP_B);
    assert_eq!(sleep_ticks(TASK_C), SLEEP_C);
    assert_eq!(sleep_ticks(TASK_U), SLEEP_U);
    assert_eq!(sleep_ticks(TASK_MAIN), 0);
    assert_eq!(sleep_ticks(usize::MAX), 0);

    // Trần **mềm** đủ cho mỗi tác vụ chạy → ngủ → chạy lại; trần **cứng** lớn hơn (chống treo).
    assert_eq!(PRIO_MIN_TICKS, 8);
    assert_eq!(PRIO_HARD_TICKS, 48);
    assert!(PRIO_MIN_TICKS < PRIO_HARD_TICKS);
}

/// **Bất biến priority**: lượt chọn đầu (cả bốn sẵn sàng) là tác vụ **ưu tiên cao nhất**, không
/// phải đầu hàng đợi FIFO.
///
/// Đây là ca **đỏ-trước** của đột biến (i): [`prio_pick_fifo`] là hàm **thuần** mà ảnh dùng khi
/// bật `--features no-prio`; nó chọn `a` (đầu FIFO) trong khi [`prio_pick`] chọn `b` (priority 3).
/// Nếu hai hàm cho **cùng** kết quả thì đột biến không thể bị bắt — test này chốt chúng **khác**.
#[test]
fn pick_highest_priority_not_fifo() {
    // Cả bốn tác vụ sẵn sàng ở tick 0.
    assert_eq!(
        prio_pick(&AWAKE, 0),
        Some(TASK_B),
        "priority: B (3) cao nhất"
    );
    assert_eq!(
        prio_pick_fifo(&AWAKE, 0),
        Some(TASK_A),
        "FIFO (đột biến no-prio): A đứng đầu hàng đợi"
    );
    assert_ne!(
        prio_pick(&AWAKE, 0),
        prio_pick_fifo(&AWAKE, 0),
        "đột biến no-prio phải **khác** bản đúng, nếu không phép kiểm là dấu cao su"
    );
    // Hoà priority ⇒ FIFO: bỏ B đi thì C (2) thắng A (1); bỏ cả B/C thì A thắng U.
    let mut wake = AWAKE;
    wake[TASK_B] = u64::MAX; // B "ngủ vĩnh viễn" chỉ trong test này
    assert_eq!(prio_pick(&wake, 0), Some(TASK_C));
    wake[TASK_C] = u64::MAX;
    assert_eq!(prio_pick(&wake, 0), Some(TASK_A));
    wake[TASK_A] = u64::MAX;
    assert_eq!(prio_pick(&wake, 0), Some(TASK_U));
    // Không ai sẵn sàng ⇒ `None` (ảnh fail-closed: kết thúc pha, không treo).
    wake[TASK_U] = u64::MAX;
    assert_eq!(prio_pick(&wake, 0), None);
}

/// **Bất biến sleep**: tác vụ có `WakeAt > tick` **không** được chọn; hết hạn thì được chọn lại.
#[test]
fn pick_skips_sleeping_task() {
    let mut wake = AWAKE;
    // B ngủ tới tick 4 (ngủ 3 tick kể từ tick 1 — đúng bảng `SLEEP_B`).
    wake[TASK_B] = prio_wake_at(1, SLEEP_B);
    assert_eq!(wake[TASK_B], 4);
    // tick 3: B còn ngủ ⇒ chọn C (priority kế).
    assert_eq!(prio_pick(&wake, 3), Some(TASK_C));
    assert!(!prio_ready(wake[TASK_B], 3), "B chưa tới hạn thức");
    // tick 4: B thức ⇒ được chọn (priority cao nhất).
    assert!(prio_ready(wake[TASK_B], 4), "B đã tới hạn thức");
    assert_eq!(prio_pick(&wake, 4), Some(TASK_B));
    // tick 5: vẫn thức (hạn đã qua, không "ngủ lại").
    assert_eq!(prio_pick(&wake, 5), Some(TASK_B));
}

/// **`sleep(0)` thức ngay** — ca mà đột biến (ii) làm hỏng: nếu `sleep(0)` **không** thức thì pha
/// chờ lượt chọn thứ hai của tác vụ ấy **mãi mãi**; chỉ trần cứng mới kết thúc được (ĐỎ, không
/// treo).
#[test]
fn sleep_zero_wakes_immediately() {
    // `sleep(0)` tại tick 4 ⇒ `WakeAt = 4` ⇒ sẵn sàng **ngay** ở tick 4 (cùng tick).
    let wake_at = prio_wake_at(4, SLEEP_U);
    assert_eq!(wake_at, 4);
    assert!(prio_ready(wake_at, 4), "sleep(0) phải thức ngay");
    assert!(!prio_ready(wake_at, 3), "trước hạn thì vẫn ngủ");
    // Sentinel "không ngủ" = 0: sẵn sàng ở **mọi** tick (kể cả tick 0).
    assert!(prio_ready(0, 0));
    assert!(prio_ready(0, 1_000_000));
    // **Đột biến (ii)** `--features no-wake`: `sleep(0)` ⇒ `WakeAt = u64::MAX` ⇒ **không bao giờ**
    // thức. Test chốt rằng giá trị ấy **không** sẵn sàng ở bất kỳ tick nào — đó là lý do pha phải
    // có trần cứng.
    assert!(!prio_ready(u64::MAX, 0));
    assert!(!prio_ready(u64::MAX, PRIO_HARD_TICKS));
    assert!(!prio_ready(u64::MAX, u64::MAX - 1));
}

/// **Vi phạm priority** phải bị phát hiện: chọn tác vụ **không** phải ưu tiên cao nhất trong các
/// tác vụ sẵn sàng ⇒ `true`. Đây là bộ đếm `viol=` của ảnh (phải `0`).
#[test]
fn priority_violation_is_detected() {
    // tick 0, cả bốn sẵn sàng: chọn B (đúng) ⇒ không vi phạm.
    assert!(!prio_violation(&AWAKE, 0, TASK_B));
    // Chọn A trong khi B (3) sẵn sàng ⇒ vi phạm (đúng log của đột biến `no-prio`).
    assert!(prio_violation(&AWAKE, 0, TASK_A));
    // Chọn U (0) trong khi A/B/C sẵn sàng ⇒ vi phạm.
    assert!(prio_violation(&AWAKE, 0, TASK_U));
    // B đang ngủ ⇒ chọn C **không** vi phạm (A và U thấp hơn C).
    let mut wake = AWAKE;
    wake[TASK_B] = 4;
    assert!(!prio_violation(&wake, 3, TASK_C));
    // Nhưng chọn A ở tick 3 (C sẵn sàng, priority 2 > 1) ⇒ vi phạm.
    assert!(prio_violation(&wake, 3, TASK_A));
}

/// **Trần cứng**: pha kết thúc ở [`PRIO_HARD_TICKS`] **dù** thiếu bằng chứng ⇒ boot **không** treo
/// khi một tác vụ không bao giờ thức (đột biến `no-wake`).
#[test]
fn phase_end_has_hard_cap() {
    let zero = [0u64; TASK_SLOTS];
    let two = [2u64; TASK_SLOTS];
    // `main` = 0, A = 2, B = 2, U = 1, C = 2 — **U** chưa được chọn lần thứ hai.
    let one_short = [0u64, 2, 2, 1, 2];
    // Chưa đủ tick mềm ⇒ chưa kết thúc, dù mọi tác vụ đã chạy 2 lượt và đã có 4 lần thức.
    assert!(!prio_end(0, &zero, 0));
    assert!(!prio_end(PRIO_MIN_TICKS - 1, &two, 4));
    // Đủ tick mềm + mọi tác vụ chọn ≥ 2 + đủ 4 lần thức ⇒ kết thúc.
    assert!(prio_end(PRIO_MIN_TICKS, &two, 4));
    // Thiếu **một** tác vụ chưa chọn lần thứ hai (đột biến `no-wake`: U ngủ 0 không thức) ⇒ chưa.
    assert!(!prio_end(PRIO_MIN_TICKS, &one_short, 4));
    // Thiếu lần thức ⇒ chưa.
    assert!(!prio_end(PRIO_MIN_TICKS, &two, 3));
    // Trần **cứng** ⇒ kết thúc **dù** không có gì: fail-closed, không treo.
    assert!(prio_end(PRIO_HARD_TICKS, &zero, 0));
    assert!(prio_end(PRIO_HARD_TICKS, &one_short, 3));
}

/// **Không phụ thuộc thời điểm**: chuỗi lượt chọn là **hàm thuần của số tick**.
///
/// Mô phỏng cho đúng `b, c, a, u, b, c, a, u` — mỗi tác vụ chạy, ngủ, rồi chạy lại; bốn lần thức;
/// pha kết thúc ở tick 8 (trước trần cứng 48). Không có đầu vào nào khác số tick, nên **không**
/// có giả định về hart nào tick trước hay host có deschedule hay không (bài học KT-308/KT-313).
#[test]
fn deterministic_sequence_is_tick_only() {
    let (seq, wakes, ticks) = simulate();
    assert_eq!(
        seq,
        vec![TASK_B, TASK_C, TASK_A, TASK_U, TASK_B, TASK_C, TASK_A, TASK_U],
        "chuỗi lượt chọn phải là hàm thuần của tick"
    );
    assert_eq!(wakes, 4, "mỗi tác vụ thức đúng một lần");
    assert_eq!(ticks, PRIO_MIN_TICKS, "pha kết thúc ở trần mềm");
    // Chạy lại mô phỏng: **cùng** kết quả (không có trạng thái ẩn, không có thời gian).
    let (seq2, wakes2, ticks2) = simulate();
    assert_eq!((seq2, wakes2, ticks2), (seq.clone(), wakes, ticks));
    // Mọi tác vụ được chọn ≥ 2 lần (chạy → ngủ → chạy lại).
    let mut picks = [0u64; TASK_SLOTS];
    for &slot in seq.iter() {
        picks[slot] += 1;
    }
    for &slot in PRIO_ORDER.iter() {
        assert!(picks[slot] >= 2, "tác vụ {slot} phải chạy lại sau khi ngủ");
    }
    // Và **không** lượt nào chọn tác vụ đang ngủ: mô phỏng dùng `prio_pick` nên điều này đúng
    // theo cấu trúc; chốt thêm bằng bộ đếm `sleep_viol` của ảnh (phải 0).
    assert_eq!(picks[TASK_B], 2);
    assert_eq!(picks[TASK_C], 2);
    assert_eq!(picks[TASK_A], 2);
    assert_eq!(picks[TASK_U], 2);
}

/// **Điều kiện TAP** phải **có răng**: mỗi trường hợp sai đều làm `prio_ok`/`sleep_ok` sai.
#[test]
fn tap_conditions_are_toothed() {
    let two = [2u64; TASK_SLOTS];
    // `main` = 0, A = 2, B = 2, U = 1, C = 2 — **U** chưa được chọn lần thứ hai.
    let one_short = [0u64, 2, 2, 1, 2];
    // Priority: lượt chọn đầu là B, không vi phạm, kết thúc trước trần cứng.
    assert!(prio_ok(TASK_B, 0, PRIO_MIN_TICKS));
    // Đột biến (i): lượt chọn đầu là A (FIFO) ⇒ ĐỎ.
    assert!(!prio_ok(TASK_A, 0, PRIO_MIN_TICKS));
    // Có vi phạm ⇒ ĐỎ.
    assert!(!prio_ok(TASK_B, 1, PRIO_MIN_TICKS));
    // Kết thúc ở trần cứng (thiếu bằng chứng) ⇒ ĐỎ.
    assert!(!prio_ok(TASK_B, 0, PRIO_HARD_TICKS));

    // Sleep/wake: bốn lần thức, không chọn tác vụ đang ngủ, mọi tác vụ chọn ≥ 2, trước trần cứng.
    assert!(sleep_ok(4, 0, PRIO_MIN_TICKS, &two));
    // Đột biến (ii): U (sleep 0) không thức ⇒ chỉ 3 lần thức + U chọn 1 lần + pha chạm trần cứng.
    assert!(!sleep_ok(3, 0, PRIO_HARD_TICKS, &one_short));
    assert!(!sleep_ok(4, 0, PRIO_MIN_TICKS, &one_short));
    // Chọn tác vụ đang ngủ ⇒ ĐỎ.
    assert!(!sleep_ok(4, 1, PRIO_MIN_TICKS, &two));
    // Kết thúc ở trần cứng ⇒ ĐỎ.
    assert!(!sleep_ok(4, 0, PRIO_HARD_TICKS, &two));
}
