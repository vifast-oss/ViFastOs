// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **hàng đợi chạy toàn cục** của pha nhiều hart — KT-310 (K5' bước 10),
//! **KT-315** mở lên **bốn** hart, **KT-313**/**KT-317** sửa bất biến của phép kiểm.
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
//! # Vì sao bất biến là `diff >= 1`, **không** phải `first[0] != first[i]` (KT-313/KT-317)
//!
//! `first[0] != first[i]` **phụ thuộc thời điểm**: hàng đợi FIFO **bốn** tác vụ quay vòng, hart
//! phụ kịp **bốn** lượt trước lượt đầu của BSP thì tác vụ `a` về lại đầu hàng đợi đúng lúc BSP
//! pop lần đầu ⇒ `first=[a, a, …]` dù `double=0` và scheduler đúng. Soak KT-313 đo **5/600** boot
//! đỏ vì điều kiện ấy (cùng lớp lỗi `last > first` của KT-308). Bất biến **thật** là `diff >= 1`:
//! **tồn tại** một lượt hart **phụ** chọn tác vụ **khác** tác vụ BSP **đang chạy** — ownership bảo
//! đảm điều này sau lượt chọn đầu của BSP, nên nó **không** phụ thuộc hart nào tick trước.
//! `first=[…]` vẫn **in** để **chẩn đoán**, **không** còn là điều kiện.
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
    MH_HOLD_TICKS, MH_MAX_ROUNDS, MH_QUEUE_SLOTS, MH_TASKS, TASK_A, TASK_B, TASK_C, TASK_MAIN,
    TASK_SLOTS, TASK_U,
};

/// Mô hình **FIFO** của hàng đợi toàn cục: `slots` là vòng, `head`/`tail` là chỉ số **đơn điệu**
/// (như `AtomicU64` của ảnh), `owner[task]` là hart đang sở hữu tác vụ (`MH_FREE` = rảnh).
struct Queue {
    slots: [usize; MH_QUEUE_SLOTS],
    head: u64,
    tail: u64,
    owner: [u64; TASK_SLOTS],
    /// **KT-313** — số lượt hart **phụ** chọn tác vụ **khác** tác vụ BSP **đang chạy** (như
    /// `MH_DIFF` của ảnh). Bất biến của pha là `diff >= 1`.
    diff: u64,
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
            diff: 0,
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
    ///
    /// `bsp_current` là `MH_CURRENT[0]` **tại thời điểm** hart này giữ khoá (ảnh đọc nó **trong**
    /// vùng tới hạn nên giá trị đứng yên). Chỉ hart **phụ** mới đếm `diff` — như `MH_DIFF` của ảnh.
    fn tick_locked(&mut self, q: &mut Queue, bsp_current: u64) -> Result<(), usize> {
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
                // **KT-313** — bằng chứng **không phụ thuộc thời điểm**: hart **phụ** vừa chọn
                // `task` trong khi BSP **đang chạy** một tác vụ **thật** khác `task`. `task` không
                // thể trùng tác vụ BSP đang giữ (ownership) nên điều kiện `bsp != task` chỉ là
                // phòng thủ — đúng như ảnh.
                if self.id != 0
                    && bsp_current != MH_FREE
                    && bsp_current != TASK_MAIN as u64
                    && bsp_current != task as u64
                {
                    q.diff += 1;
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
    harts[0]
        .tick_locked(&mut q, MH_FREE)
        .expect("hart 0: double-pick");
    let mut round = 0u64;
    while round < 6 {
        let mut i = 1;
        while i < MH_HARTS {
            let bsp = harts[0].current;
            harts[i]
                .tick_locked(&mut q, bsp)
                .expect("hart phụ: double-pick");
            i += 1;
        }
        harts[0]
            .tick_locked(&mut q, MH_FREE)
            .expect("hart 0: double-pick");
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
    harts[0]
        .tick_locked(&mut q, MH_FREE)
        .expect("hart 0: double-pick");
    let mut round = 0u64;
    while round < 2 {
        let mut i = 1;
        while i < MH_HARTS {
            let bsp = harts[0].current;
            harts[i]
                .tick_locked(&mut q, bsp)
                .expect("hart phụ: double-pick");
            i += 1;
        }
        harts[0]
            .tick_locked(&mut q, MH_FREE)
            .expect("hart 0: double-pick");
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
    harts[0]
        .tick_locked(&mut q, MH_FREE)
        .expect("hart 0: double-pick");
    let mut round = 0u64;
    while round < 2 {
        let mut i = 1;
        while i < MH_HARTS {
            let bsp = harts[0].current;
            harts[i]
                .tick_locked(&mut q, bsp)
                .expect("hart phụ: double-pick");
            i += 1;
        }
        harts[0]
            .tick_locked(&mut q, MH_FREE)
            .expect("hart 0: double-pick");
        round += 1;
    }
    let (picks, first) = snapshot(&harts);
    let mut i = 0;
    while i < MH_HARTS {
        assert!(picks[i] >= 1, "hart {i} chưa chọn tác vụ nào");
        i += 1;
    }
    // **Tính chất của cổng** (không phải điều kiện của pha — KT-313): với cổng khởi động, BSP giữ
    // tác vụ đầu hàng đợi nên hart phụ **không thể** lấy lại nó ở lần đầu. Đây là lý do `diff`
    // **tất định**; `mh_ok` **không** đòi `first` khác nhau (xem
    // [`kt313_first_collision_is_not_a_failure`]).
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
    assert!(
        q.diff >= 1,
        "pha phải có ít nhất một lượt hart phụ chọn khác BSP"
    );
    assert!(mh_ok(rounds, &picks, 0, q.diff));
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
    assert!(!mh_ok(MH_HARTS as u64, &picks, 1, 1));
    // Cùng dữ liệu nhưng `double = 0` và **không** có lượt hart phụ chọn khác tác vụ BSP
    // (`diff = 0`) thì **vẫn** sai — chứng minh điều kiện `diff >= 1` là **có răng**.
    assert!(!mh_ok(MH_HARTS as u64, &picks, 0, 0));
    // Bốn tác vụ **khác** nhau + không double-pick + có `diff` ⇒ đạt.
    assert!(mh_ok(MH_HARTS as u64, &picks, 0, 1));
}

#[test]
fn phase_end_waits_for_every_hart_but_never_hangs() {
    // Chưa đủ vòng mềm ⇒ chưa kết thúc, dù cả bốn đã chọn.
    assert!(!mh_end(0, &[0, 0, 0, 0], 0));
    assert!(!mh_end(MH_MAX_ROUNDS - 1, &[3, 3, 3, 3], 1));
    // Đủ vòng mềm **và** mọi hart đã chọn **và** có `diff` ⇒ kết thúc.
    assert!(mh_end(MH_MAX_ROUNDS, &[3, 3, 3, 3], 1));
    // Đủ vòng mềm nhưng **chưa** có lượt hart phụ chọn khác tác vụ BSP ⇒ **chưa** kết thúc
    // (**KT-313**: kéo dài pha cho tới khi bằng chứng tồn tại, thay vì đòi nó ở vòng cố định).
    assert!(!mh_end(MH_MAX_ROUNDS, &[3, 3, 3, 3], 0));
    // Đủ vòng mềm nhưng **một** hart chưa chọn ⇒ **chưa** kết thúc (chờ hart kia).
    assert!(!mh_end(MH_MAX_ROUNDS, &[6, 6, 6, 0], 1));
    // Trần **cứng** ⇒ kết thúc **dù** hart kia chưa chọn / chưa có `diff`: fail-closed, không treo.
    assert!(mh_end(MH_HARD_ROUNDS, &[MH_HARD_ROUNDS, 0, 0, 0], 0));
}

#[test]
fn phase_ok_requires_four_harts() {
    // Đủ: bốn hart, không double-pick, ≥ 4 vòng, có lượt hart phụ chọn khác tác vụ BSP.
    assert!(mh_ok(4, &[1, 1, 1, 1], 0, 1));
    // Thiếu: một hart chưa chọn (đột biến `--features two-hart-sched`).
    assert!(!mh_ok(4, &[2, 2, 0, 0], 0, 1));
    // Thiếu: có double-pick dù `diff` khác 0.
    assert!(!mh_ok(4, &[1, 1, 1, 1], 1, 1));
    // Thiếu: **không** có lượt hart phụ chọn tác vụ khác tác vụ BSP đang chạy.
    assert!(!mh_ok(4, &[1, 1, 1, 1], 0, 0));
    // Thiếu: chưa đủ 4 vòng.
    assert!(!mh_ok(3, &[1, 1, 1, 1], 0, 1));
}

/// **KT-313** — hồi quy **đúng dữ liệu soak**: `first=[a, a, …] double=0 diff>=1` **ĐẠT**.
///
/// Đây là khuôn của **5/600** boot đỏ ở soak KT-313 (boot 98, 211, 269, 304, 431 — lúc ấy **hai**
/// hart). Trước KT-313 `mh_ok` đòi `first[0] != first[1]` nên khuôn này **ĐỎ** dù scheduler đúng:
/// hàng đợi FIFO quay vòng, hart phụ kịp **bốn** lượt trước lượt đầu của BSP nên tác vụ `a` về
/// lại đầu hàng đợi đúng lúc BSP pop lần đầu. Mô hình dưới đây **tái hiện đúng** chuỗi ấy với
/// **bốn** hart của KT-315.
#[test]
fn kt313_first_collision_is_not_a_failure() {
    let mut q = Queue::new();
    let mut harts = four_harts();
    // R1..R4: hart phụ chọn `a`, `b`, `u`, `c` trong khi BSP còn ở `main` (`MH_FREE`).
    let mut k = 0;
    while k < MH_TASKS.len() {
        let bsp = harts[0].current;
        harts[1]
            .tick_locked(&mut q, bsp)
            .expect("hart 1: double-pick");
        k += 1;
    }
    // R5: BSP chọn lần đầu — `a` đã quay lại đầu hàng đợi.
    harts[0]
        .tick_locked(&mut q, MH_FREE)
        .expect("hart 0: double-pick");
    assert_eq!(harts[0].first, TASK_A as u64);
    assert_eq!(
        harts[0].first, harts[1].first,
        "race KT-313: hai hart chọn **cùng** tác vụ ở lần đầu"
    );
    // R6: hart 1 chọn tiếp — **khác** tác vụ BSP đang chạy ⇒ `diff >= 1`.
    let bsp = harts[0].current;
    harts[1]
        .tick_locked(&mut q, bsp)
        .expect("hart 1: double-pick");
    // R7..R8: hai hart phụ còn lại chọn lần đầu (đủ điều kiện "mọi hart đã chọn").
    harts[2]
        .tick_locked(&mut q, bsp)
        .expect("hart 2: double-pick");
    harts[3]
        .tick_locked(&mut q, bsp)
        .expect("hart 3: double-pick");
    let (picks, _) = snapshot(&harts);
    assert_eq!(picks, [1, 5, 1, 1], "đúng khuôn soak KT-313 (bốn hart)");
    assert!(q.diff >= 1, "phải có lượt hart phụ chọn khác tác vụ BSP");
    let rounds: u64 = picks.iter().sum();
    // **KT-313**: dữ liệu này **ĐẠT** — `first` trùng **không** phải lỗi scheduler.
    assert!(mh_ok(rounds, &picks, 0, q.diff));
    // Nhưng nếu **không** có lượt nào khác tác vụ BSP thì vẫn ĐỎ (điều kiện có răng).
    assert!(!mh_ok(rounds, &picks, 0, 0));
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
/// `first[0] == first[i]` **hợp lệ** (KT-313: **không** còn là điều kiện) và `diff` có thể bằng 0
/// lâu hơn. Cổng giữ làm `diff >= 1` **tất định**: trong lúc BSP giữ một tác vụ thật, mọi lượt
/// chọn của hart phụ **buộc** phải khác tác vụ ấy (ownership) ⇒ `diff` tăng.
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
    assert!(!mh_end(MH_MAX_ROUNDS - 1, &[1, 1, 1, 1], 1));
    assert!(mh_end(MH_MAX_ROUNDS, &[1, 1, 1, 1], 1));
}
