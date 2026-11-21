// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **hàng đợi chạy toàn cục** của pha nhiều hart — KT-310 (K5' bước 10).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test runqueue
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-310: các mục [`mh_ok`], [`mh_end`], [`task_name`]
//! và bốn hằng `MH_*` **chưa** tồn tại khi tệp này được viết, nên lượt chạy đầu **không biên dịch
//! được** (đỏ đúng lý do). Sau khi hiện thực, các khẳng định dưới đây chốt **giá trị tuyệt đối**
//! mà ảnh `vifast-kbin-x86_64` dùng, và chốt **hai** bất biến của pha nhiều hart:
//!
//! 1. **Có khoá** ⇒ hai hart **không bao giờ** nhận cùng một tác vụ (mỗi tác vụ có **một** chủ).
//! 2. **Không khoá** ⇒ hai hart đọc **cùng** đầu hàng đợi ⇒ cùng tác vụ ⇒ phép kiểm **phải** phát
//!    hiện (`Err(task)`), tức đột biến `--features no-sched-lock` **đỏ được**, không phải "hy vọng
//!    nó không xảy ra".
//!
//! # Vì sao mô hình hàng đợi nằm **trong test**, không trong crate
//!
//! Hàng đợi thật là `AtomicU64` + `AtomicBool` trong ảnh (không test host được: cần MMIO/ngắt).
//! Phần **thuần** đáng chốt là **luật**: khi nào pha kết thúc ([`mh_end`]), khi nào pha đủ bằng
//! chứng ([`mh_ok`]), tên tác vụ in ra log ([`task_name`]). Mô hình FIFO dưới đây dùng **đúng**
//! luật ấy nên nếu luật sai thì test đỏ, còn cơ chế nguyên tử của ảnh thì đo bằng QEMU.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà ảnh boot
// nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::sched::{
    mh_end, mh_ok, task_name, MH_FREE, MH_HARD_ROUNDS, MH_HARTS, MH_MAX_ROUNDS, MH_QUEUE_SLOTS,
    MH_TASKS, TASK_A, TASK_B, TASK_SLOTS, TASK_U,
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

#[test]
fn constants_are_literal() {
    // **Hai** hart tham gia (BSP + hart 1) — "nhiều hart" theo đúng nghĩa đếm được.
    assert_eq!(MH_HARTS, 2);
    // Hàng đợi là **ba** tác vụ đã có của scheduler (không thêm tác vụ mới trong KT-310).
    assert_eq!(MH_TASKS, [TASK_A, TASK_B, TASK_U]);
    // Vòng phải **rộng hơn** số tác vụ: mỗi hart push tác vụ cũ **trước** khi pop tác vụ mới, nên
    // hàng đợi tạm thời có tới `MH_TASKS.len() + MH_HARTS` phần tử.
    assert!(MH_QUEUE_SLOTS > MH_TASKS.len() + MH_HARTS);
    // Trần **mềm** phải nhỏ hơn trần **cứng** — nếu không thì pha không bao giờ "chờ hart kia".
    assert!(MH_MAX_ROUNDS < MH_HARD_ROUNDS);
    // `MH_FREE` phải là giá trị **không** trùng slot nào (`TASK_SLOTS` = 4).
    assert_eq!(MH_FREE, u64::MAX);
    assert!(MH_FREE >= TASK_SLOTS as u64);
}

#[test]
fn task_name_covers_every_slot() {
    assert_eq!(task_name(TASK_A), "a");
    assert_eq!(task_name(TASK_B), "b");
    assert_eq!(task_name(TASK_U), "u");
    // Slot lạ (kể cả `MH_FREE` khi chưa hart nào chọn) **không** được panic: log chẩn đoán phải
    // in được cả trạng thái hỏng.
    assert_eq!(task_name(0), "?");
    assert_eq!(task_name(TASK_SLOTS), "?");
    assert_eq!(task_name(usize::MAX), "?");
}

#[test]
fn locked_harts_never_share_a_task() {
    let mut q = Queue::new();
    let mut h0 = Hart::new(0);
    let mut h1 = Hart::new(1);
    // Xen kẽ 3 tick mỗi hart — đúng nhịp hai hart khác tần số (PIT 100 Hz vs LAPIC timer).
    let mut round = 0u64;
    while round < 3 {
        h0.tick_locked(&mut q).expect("hart 0: double-pick");
        h1.tick_locked(&mut q).expect("hart 1: double-pick");
        round += 1;
    }
    assert!(h0.picks >= 1 && h1.picks >= 1);
    // Hai hart **phải** chọn tác vụ **khác** nhau ở lần đầu — đó là bằng chứng khoá có tác dụng.
    assert_ne!(h0.first, h1.first);
    // Và mỗi tác vụ chỉ có **một** chủ tại một thời điểm.
    assert_ne!(h0.current, h1.current);
    let picks = [h0.picks, h1.picks];
    let first = [h0.first, h1.first];
    assert!(mh_ok(round * 2, &picks, &first, 0));
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
    // Và pha **không** được coi là đạt: `double != 0` ⇒ `mh_ok` sai, dù hai hart đều đã chọn.
    let picks = [1u64, 1u64];
    let first = [task0 as u64, task0 as u64];
    assert!(!mh_ok(2, &picks, &first, 1));
    // Cùng dữ liệu nhưng `double = 0` thì **vẫn** sai vì `first` trùng — chứng minh điều kiện
    // "hai hart chọn khác tác vụ" là **có răng**, không phải hằng sai.
    assert!(!mh_ok(2, &picks, &first, 0));
    // Hai tác vụ **khác** nhau + không double-pick ⇒ đạt.
    let first_ok = [task0 as u64, task1 as u64 + 1];
    assert!(mh_ok(2, &picks, &first_ok, 0));
}

#[test]
fn phase_end_waits_for_every_hart_but_never_hangs() {
    // Chưa đủ vòng mềm ⇒ chưa kết thúc, dù cả hai đã chọn.
    assert!(!mh_end(0, &[0, 0]));
    assert!(!mh_end(MH_MAX_ROUNDS - 1, &[3, 3]));
    // Đủ vòng mềm **và** mọi hart đã chọn ⇒ kết thúc.
    assert!(mh_end(MH_MAX_ROUNDS, &[3, 3]));
    // Đủ vòng mềm nhưng **một** hart chưa chọn ⇒ **chưa** kết thúc (chờ hart kia).
    assert!(!mh_end(MH_MAX_ROUNDS, &[6, 0]));
    // Trần **cứng** ⇒ kết thúc **dù** hart kia chưa chọn: fail-closed, không treo boot.
    assert!(mh_end(MH_HARD_ROUNDS, &[MH_HARD_ROUNDS, 0]));
}

#[test]
fn phase_ok_requires_two_distinct_harts() {
    let first = [TASK_A as u64, TASK_B as u64];
    // Đủ: hai hart, hai tác vụ khác nhau, không double-pick, ≥ 2 vòng.
    assert!(mh_ok(2, &[1, 1], &first, 0));
    // Thiếu: một hart chưa chọn (đột biến `--features bsp-only-sched`).
    assert!(!mh_ok(2, &[2, 0], &first, 0));
    // Thiếu: hai hart chọn **cùng** tác vụ (đột biến `--features no-sched-lock`).
    assert!(!mh_ok(2, &[1, 1], &[TASK_A as u64, TASK_A as u64], 0));
    // Thiếu: có double-pick dù `first` khác nhau.
    assert!(!mh_ok(2, &[1, 1], &first, 1));
    // Thiếu: chưa đủ 2 vòng.
    assert!(!mh_ok(1, &[1, 1], &first, 0));
}
