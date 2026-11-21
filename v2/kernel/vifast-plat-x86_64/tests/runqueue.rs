// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **hàng đợi chạy toàn cục** của pha nhiều hart — KT-310 (K5' bước 10),
//! **KT-315** mở lên **bốn** hart.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test runqueue
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-310 (và **KT-315** cho bốn hart): các mục
//! [`mh_ok`], [`mh_end`], [`mh_harts_picked`], [`task_name`] và các hằng `MH_*` **chưa** tồn tại
//! khi tệp này được viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do). Sau khi
//! hiện thực, các khẳng định dưới đây chốt **giá trị tuyệt đối** mà ảnh `vifast-kbin-x86_64`
//! dùng, và chốt **hai** bất biến của pha nhiều hart:
//!
//! 1. **Có khoá** ⇒ không hart nào nhận cùng một tác vụ (mỗi tác vụ có **một** chủ).
//! 2. **Không khoá** ⇒ hai hart đọc **cùng** đầu hàng đợi ⇒ cùng tác vụ ⇒ phép kiểm **phải** phát
//!    hiện (`Err(task)`), tức đột biến `--features no-sched-lock` **đỏ được**, không phải "hy vọng
//!    nó không xảy ra".
//!
//! # Vì sao **bốn** tác vụ cho **bốn** hart (KT-315)
//!
//! Với **ba** tác vụ và **bốn** hart, hart thứ tư **đói vĩnh viễn**: hàng đợi luôn có đúng **một**
//! tác vụ rảnh, và mỗi hart trả tác vụ cũ rồi lấy lại **chính nó** trong cùng vùng tới hạn ⇒ hart
//! không giữ tác vụ nào không bao giờ thấy hàng đợi khác rỗng. Vì vậy pha bốn hart cần **bốn**
//! tác vụ; test [`four_harts_need_four_tasks`] chốt đúng điều kiện ấy (và chứng minh ba tác vụ
//! **không** đủ — mô hình hoá đúng lý do đã hạ phạm vi ở KT-310).
//!
//! # Vì sao mô hình hàng đợi nằm **trong test**, không trong crate
//!
//! Hàng đợi thật là `AtomicU64` + `AtomicBool` trong ảnh (không test host được: cần MMIO/ngắt).
//! Phần **thuần** đáng chốt là **luật**: khi nào pha kết thúc ([`mh_end`]), khi nào pha đủ bằng
//! chứng ([`mh_ok`]), đếm số hart đã chọn ([`mh_harts_picked`]), tên tác vụ in ra log
//! ([`task_name`]). Mô hình FIFO dưới đây dùng **đúng** luật ấy nên nếu luật sai thì test đỏ, còn
//! cơ chế nguyên tử của ảnh thì đo bằng QEMU.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà ảnh boot
// nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::sched::{
    mh_end, mh_harts_picked, mh_hold, mh_ok, task_name, MH_FREE, MH_HARD_ROUNDS, MH_HARTS,
    MH_HOLD_TICKS, MH_MAX_ROUNDS, MH_QUEUE_SLOTS, MH_TASKS, TASK_A, TASK_B, TASK_C, TASK_SLOTS,
    TASK_U,
};

/// Mô hình **FIFO** của hàng đợi toàn cục: `slots` là vòng, `head`/`tail` là chỉ số **đơn điệu**
/// (như `AtomicU64` của ảnh), `owner[task]` là hart đang sở hữu tác vụ (`MH_FREE` = rảnh).
struct Queue {
    slots: [usize; MH_QUEUE_SLOTS],
    head: u64,
    tail: u64,
    owner: [u64; TASK_SLOTS],
}

impl Queue {
    fn new() -> Self {
        let mut slots = [0usize; MH_QUEUE_SLOTS];
        let mut i = 0;
        while i < MH_TASKS.len() {
            slots[i] = MH_TASKS[i];
            i += 1;
        }
        Self {
            slots,
            head: 0,
            tail: MH_TASKS.len() as u64,
            owner: [MH_FREE; TASK_SLOTS],
        }
    }

    /// Đẩy tác vụ về **cuối** hàng đợi (FIFO) — như `mh_push` của ảnh.
    fn push(&mut self, task: usize) {
        self.slots[(self.tail as usize) % MH_QUEUE_SLOTS] = task;
        self.tail += 1;
    }

    /// Lấy tác vụ ở **đầu** hàng đợi rồi **giành quyền sở hữu**.
    ///
    /// `Ok(None)` = hàng đợi rỗng; `Err(task)` = tác vụ đã có chủ ⇒ **double-pick** (chỉ xảy ra
    /// khi không khoá: hai hart đọc cùng `head`).
    fn pop(&mut self, hart: u64) -> Result<Option<usize>, usize> {
        if self.head == self.tail {
            return Ok(None);
        }
        let head = self.head;
        self.pop_at(head, hart)
    }

    /// Như [`Queue::pop`] nhưng nhận `head` **đã đọc trước** — mô hình hoá đúng cửa sổ của đột
    /// biến `--features no-sched-lock`: hai hart cùng đọc `head` rồi cùng lấy **một** tác vụ.
    fn pop_at(&mut self, head: u64, hart: u64) -> Result<Option<usize>, usize> {
        if head == self.tail {
            return Ok(None);
        }
        let task = self.slots[(head as usize) % MH_QUEUE_SLOTS];
        self.head = head + 1;
        if self.owner[task] != MH_FREE {
            return Err(task);
        }
        self.owner[task] = hart;
        Ok(Some(task))
    }

    /// Trả tác vụ về hàng đợi (nhả quyền sở hữu rồi đẩy về cuối).
    fn release(&mut self, task: usize) {
        self.owner[task] = MH_FREE;
        self.push(task);
    }
}

/// Một hart trong mô hình: `current` = tác vụ đang chạy (`MH_FREE` = chưa có).
struct Hart {
    id: u64,
    current: u64,
    picks: u64,
    first: u64,
}

impl Hart {
    fn new(id: u64) -> Self {
        Self {
            id,
            current: MH_FREE,
            picks: 0,
            first: MH_FREE,
        }
    }

    /// Một tick **có khoá**: trả tác vụ cũ về hàng đợi rồi lấy tác vụ mới.
    fn tick_locked(&mut self, q: &mut Queue) -> Result<(), usize> {
        if self.current != MH_FREE {
            q.release(self.current as usize);
            self.current = MH_FREE;
        }
        match q.pop(self.id)? {
            Some(task) => {
                self.current = task as u64;
                self.picks += 1;
                if self.first == MH_FREE {
                    self.first = task as u64;
                }
                Ok(())
            }
            None => Ok(()),
        }
    }
}

/// Bốn hart của pha — `harts[0]` là BSP.
fn four_harts() -> [Hart; MH_HARTS] {
    [Hart::new(0), Hart::new(1), Hart::new(2), Hart::new(3)]
}

/// Gom `picks`/`first` của bốn hart thành mảng cho [`mh_ok`].
fn snapshot(harts: &[Hart; MH_HARTS]) -> ([u64; MH_HARTS], [u64; MH_HARTS]) {
    let mut picks = [0u64; MH_HARTS];
    let mut first = [0u64; MH_HARTS];
    let mut i = 0;
    while i < MH_HARTS {
        picks[i] = harts[i].picks;
        first[i] = harts[i].first;
        i += 1;
    }
    (picks, first)
}

#[test]
fn constants_are_literal() {
    // **Bốn** hart tham gia (BSP + hart 1..3) — "nhiều hart" theo đúng nghĩa đếm được, và là con
    // số K5' đòi ("như K3": bốn hart).
    assert_eq!(MH_HARTS, 4);
    // Hàng đợi là **bốn** tác vụ: ba tác vụ cũ (A, B, U) + tác vụ **C** của KT-315. Bốn hart cần
    // bốn tác vụ — xem [`four_harts_need_four_tasks`].
    assert_eq!(MH_TASKS, [TASK_A, TASK_B, TASK_U, TASK_C]);
    // Vòng phải **rộng hơn** số tác vụ: mỗi hart push tác vụ cũ **trước** khi pop tác vụ mới, nên
    // hàng đợi tạm thời có tới `MH_TASKS.len() + MH_HARTS` phần tử.
    assert!(MH_QUEUE_SLOTS > MH_TASKS.len() + MH_HARTS);
    // Trần **mềm** phải đủ cho mỗi hart chọn ≥ 1 tác vụ, và nhỏ hơn trần **cứng** — nếu không thì
    // pha không bao giờ "chờ hart kia".
    assert!(MH_MAX_ROUNDS >= MH_HARTS as u64);
    assert!(MH_MAX_ROUNDS < MH_HARD_ROUNDS);
    // `MH_FREE` phải là giá trị **không** trùng slot nào (`TASK_SLOTS` = 5).
    assert_eq!(MH_FREE, u64::MAX);
    assert!(MH_FREE >= TASK_SLOTS as u64);
}

#[test]
fn task_name_covers_every_slot() {
    assert_eq!(task_name(TASK_A), "a");
    assert_eq!(task_name(TASK_B), "b");
    assert_eq!(task_name(TASK_U), "u");
    assert_eq!(task_name(TASK_C), "c");
    // Slot lạ (kể cả `MH_FREE` khi chưa hart nào chọn) **không** được panic: log chẩn đoán phải
    // in được cả trạng thái hỏng.
    assert_eq!(task_name(0), "?");
    assert_eq!(task_name(TASK_SLOTS), "?");
    assert_eq!(task_name(usize::MAX), "?");
}

/// **KT-315** — **bốn** hart cần **bốn** tác vụ: mô hình hoá đúng lý do.
///
/// Với **ba** tác vụ, hart thứ tư **không bao giờ** chọn được tác vụ: sau khi ba hart đầu giữ ba
/// tác vụ, hàng đợi rỗng; mỗi hart trả tác vụ cũ rồi lấy lại **chính nó** trong cùng vùng tới hạn
/// ⇒ hàng đợi luôn có đúng **một** phần tử khi hart thứ tư nhìn vào. Đây là lý do KT-315 thêm tác
/// vụ **C** thay vì nới ngưỡng hay hạ phạm vi xuống ba hart.
#[test]
fn four_harts_need_four_tasks() {
    // Ba tác vụ + bốn hart ⇒ hart 3 **đói** (picks = 0) dù chạy bao nhiêu vòng.
    let mut q = Queue::new();
    let mut harts = four_harts();
    // Mô phỏng hàng đợi **ba** tác vụ: bỏ tác vụ thứ tư khỏi `tail`.
    q.tail = 3;
    harts[0].tick_locked(&mut q).expect("hart 0: double-pick");
    let mut round = 0u64;
    while round < 6 {
        let mut i = 1;
        while i < MH_HARTS {
            harts[i].tick_locked(&mut q).expect("hart phụ: double-pick");
            i += 1;
        }
        harts[0].tick_locked(&mut q).expect("hart 0: double-pick");
        round += 1;
    }
    let (picks3, _) = snapshot(&harts);
    assert_eq!(
        picks3[3], 0,
        "ba tác vụ ⇒ hart 3 phải đói (đúng lý do thêm C)"
    );
    assert_eq!(mh_harts_picked(&picks3), 3);

    // Bốn tác vụ + bốn hart ⇒ **mọi** hart chọn được.
    let mut q = Queue::new();
    let mut harts = four_harts();
    harts[0].tick_locked(&mut q).expect("hart 0: double-pick");
    let mut round = 0u64;
    while round < 2 {
        let mut i = 1;
        while i < MH_HARTS {
            harts[i].tick_locked(&mut q).expect("hart phụ: double-pick");
            i += 1;
        }
        harts[0].tick_locked(&mut q).expect("hart 0: double-pick");
        round += 1;
    }
    let (picks4, _) = snapshot(&harts);
    assert_eq!(mh_harts_picked(&picks4), MH_HARTS as u64);
}

#[test]
fn locked_harts_never_share_a_task() {
    let mut q = Queue::new();
    let mut harts = four_harts();
    // **Cổng khởi động** của ảnh: hart phụ chỉ chọn sau khi BSP đã chọn lần đầu (nhịp PIT của BSP
    // 100 Hz chậm hơn LAPIC timer của hart phụ ~1 ms; không có cổng thì hart phụ hút sạch hàng
    // đợi trước tick đầu của BSP). Mô hình hoá đúng thứ tự ấy.
    harts[0].tick_locked(&mut q).expect("hart 0: double-pick");
    let mut round = 0u64;
    while round < 2 {
        let mut i = 1;
        while i < MH_HARTS {
            harts[i].tick_locked(&mut q).expect("hart phụ: double-pick");
            i += 1;
        }
        harts[0].tick_locked(&mut q).expect("hart 0: double-pick");
        round += 1;
    }
    let (picks, first) = snapshot(&harts);
    let mut i = 0;
    while i < MH_HARTS {
        assert!(picks[i] >= 1, "hart {i} chưa chọn tác vụ nào");
        i += 1;
    }
    // BSP và **mọi** hart phụ chọn tác vụ **khác** nhau ở lần đầu — đó là bằng chứng khoá + cổng
    // khởi động có tác dụng (BSP giữ tác vụ đầu hàng đợi, hart phụ không thể lấy lại nó).
    let mut i = 1;
    while i < MH_HARTS {
        assert_ne!(first[0], first[i], "hart {i} chọn cùng tác vụ với BSP");
        i += 1;
    }
    // Và mỗi tác vụ chỉ có **một** chủ tại một thời điểm.
    let mut i = 0;
    while i < MH_HARTS {
        let mut j = i + 1;
        while j < MH_HARTS {
            assert_ne!(harts[i].current, harts[j].current);
            j += 1;
        }
        i += 1;
    }
    let rounds: u64 = picks.iter().sum();
    assert!(mh_ok(rounds, &picks, &first, 0));
}

#[test]
fn unlocked_harts_double_pick_is_detected() {
    let mut q = Queue::new();
    // Không khoá: **hai** hart cùng đọc `head = 0` trước khi ai kịp tăng nó.
    let head = q.head;
    let task0 = q.slots[(head as usize) % MH_QUEUE_SLOTS];
    let task1 = q.slots[(head as usize) % MH_QUEUE_SLOTS];
    assert_eq!(task0, task1, "hai hart đọc cùng đầu hàng đợi ⇒ cùng tác vụ");
    // Hart 0 thắng CAS ownership.
    assert_eq!(q.pop_at(head, 0), Ok(Some(task0)));
    // Hart 1 cũng đọc `head = 0` (giá trị **cũ**) ⇒ lấy **cùng** tác vụ ⇒ ownership từ chối.
    assert_eq!(q.pop_at(head, 1), Err(task0));
    // Và pha **không** được coi là đạt: `double != 0` ⇒ `mh_ok` sai, dù mọi hart đều đã chọn.
    let picks = [1u64; MH_HARTS];
    let first = [task0 as u64; MH_HARTS];
    assert!(!mh_ok(MH_HARTS as u64, &picks, &first, 1));
    // Cùng dữ liệu nhưng `double = 0` thì **vẫn** sai vì `first` trùng — chứng minh điều kiện
    // "hart phụ chọn khác BSP" là **có răng**, không phải hằng sai.
    assert!(!mh_ok(MH_HARTS as u64, &picks, &first, 0));
    // Bốn tác vụ **khác** nhau + không double-pick ⇒ đạt.
    let first_ok = [TASK_A as u64, TASK_B as u64, TASK_U as u64, TASK_C as u64];
    assert!(mh_ok(MH_HARTS as u64, &picks, &first_ok, 0));
}

#[test]
fn phase_end_waits_for_every_hart_but_never_hangs() {
    // Chưa đủ vòng mềm ⇒ chưa kết thúc, dù cả bốn đã chọn.
    assert!(!mh_end(0, &[0, 0, 0, 0]));
    assert!(!mh_end(MH_MAX_ROUNDS - 1, &[3, 3, 3, 3]));
    // Đủ vòng mềm **và** mọi hart đã chọn ⇒ kết thúc.
    assert!(mh_end(MH_MAX_ROUNDS, &[3, 3, 3, 3]));
    // Đủ vòng mềm nhưng **một** hart chưa chọn ⇒ **chưa** kết thúc (chờ hart kia).
    assert!(!mh_end(MH_MAX_ROUNDS, &[6, 6, 6, 0]));
    // Trần **cứng** ⇒ kết thúc **dù** hart kia chưa chọn: fail-closed, không treo boot.
    assert!(mh_end(MH_HARD_ROUNDS, &[MH_HARD_ROUNDS, 0, 0, 0]));
}

#[test]
fn phase_ok_requires_four_harts() {
    let first = [TASK_A as u64, TASK_B as u64, TASK_U as u64, TASK_C as u64];
    // Đủ: bốn hart, bốn tác vụ khác nhau ở lần đầu, không double-pick, ≥ 4 vòng.
    assert!(mh_ok(4, &[1, 1, 1, 1], &first, 0));
    // Thiếu: một hart chưa chọn (đột biến `--features two-hart-sched`).
    assert!(!mh_ok(4, &[2, 2, 0, 0], &first, 0));
    // Thiếu: hart phụ chọn **cùng** tác vụ với BSP ở lần đầu.
    assert!(!mh_ok(
        4,
        &[1, 1, 1, 1],
        &[TASK_A as u64, TASK_A as u64, TASK_U as u64, TASK_C as u64],
        0
    ));
    // Thiếu: có double-pick dù `first` khác nhau.
    assert!(!mh_ok(4, &[1, 1, 1, 1], &first, 1));
    // Thiếu: chưa đủ 4 vòng.
    assert!(!mh_ok(3, &[1, 1, 1, 1], &first, 0));
}

/// **KT-315** — `harts=` của dòng tổng hợp là **số đo** (số hart đã chọn ≥ 1), không phải hằng
/// `MH_HARTS`. Nhờ vậy đột biến "chỉ 2 hart" in `harts=2` và runner **đỏ** — nếu in hằng thì đột
/// biến ấy không thể bị bắt.
#[test]
fn harts_picked_counts_only_harts_with_a_pick() {
    assert_eq!(mh_harts_picked(&[0, 0, 0, 0]), 0);
    assert_eq!(mh_harts_picked(&[1, 0, 0, 0]), 1);
    assert_eq!(mh_harts_picked(&[3, 0, 2, 0]), 2);
    assert_eq!(mh_harts_picked(&[1, 1, 1, 0]), 3);
    assert_eq!(mh_harts_picked(&[1, 1, 1, 1]), MH_HARTS as u64);
}

/// **KT-315** — **cổng giữ** của BSP: giữ tác vụ đầu cho tới khi mọi hart chọn lần đầu, nhưng
/// **có hạn** (hết [`MH_HOLD_TICKS`] tick của BSP thì trả) ⇒ boot **không** treo khi một hart phụ
/// im lặng.
///
/// Vì sao cần: cổng khởi động chỉ bảo đảm hart phụ chọn **sau** BSP. Nếu BSP đã **trả** tác vụ
/// đầu trước khi hart phụ chậm nhất chọn thì hart ấy chọn **đúng** tác vụ của BSP ⇒
/// `first[0] == first[i]` **hợp lệ** ⇒ phép kiểm "hart phụ chọn khác BSP" đỏ oan (đo được: boot
/// 4/5 của lượt runner đầu, `first=[a, a, b, u]`).
#[test]
fn bsp_holds_its_first_task_but_only_for_a_bounded_window() {
    // Chưa đủ hart ⇒ giữ, trong hạn.
    assert!(mh_hold(1, &[1, 0, 0, 0]));
    assert!(mh_hold(MH_HOLD_TICKS, &[1, 1, 0, 0]));
    // Hết hạn ⇒ **trả** dù chưa đủ hart (chống treo: BSP chọn lại mỗi tick ⇒ `MH_ROUNDS` tăng).
    assert!(!mh_hold(MH_HOLD_TICKS + 1, &[1, 1, 0, 0]));
    // Đủ hart ⇒ **trả** ngay, không chờ hết hạn.
    assert!(!mh_hold(1, &[1, 1, 1, 1]));
    // Cổng giữ **không** thay đổi luật kết thúc pha.
    assert!(!mh_end(MH_MAX_ROUNDS - 1, &[1, 1, 1, 1]));
    assert!(mh_end(MH_MAX_ROUNDS, &[1, 1, 1, 1]));
}
