// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`CoreObserver`** — tách dữ liệu **oracle/test** khỏi sổ cái sản phẩm (KT-405, D2-259 mục 4).
//!
//! `Machine` là **sổ cái** (object/cap/body/queue) mà kernel sản phẩm cần. Ba bảng dưới đây
//! **không** phục vụ sản phẩm — chúng chỉ để parity với `vifast-sim` (oracle) và cho test:
//!
//! | Dữ liệu | Vì sao là oracle/test | Ai đọc |
//! |---|---|---|
//! | `pair_hits[121]` | bộ đếm cặp chuyển trạng thái 11×11 (KT-18 DoD 4) | parity + test |
//! | `forbidden_hits[121]` | bộ đếm cặp **cấm** bị thử (KT-23 DoD 3) | parity + test |
//! | `finalise_log[256]` | nhật ký finalise TCB (KT-18 DoD 9) | parity + test |
//!
//! Trước KT-405 ba bảng này nằm **trong** `Machine` ⇒ `size_of::<Machine>()` phình
//! **13.256 B** (484 + 484 + 12.288) và mọi ảnh sản phẩm phải trả giá cho dữ liệu không dùng.
//! KT-405 rút chúng ra sau trait này:
//!
//! - [`NoopObserver`] — **0 B** (ZST), mọi thao tác là no-op; dùng cho **sản phẩm**
//!   (`Machine<StaticStore, NoopObserver>`).
//! - [`TestObserver`] — giữ nguyên ba bảng + ngữ nghĩa cũ; dùng cho **sim/ktest** và mọi test
//!   host (`Machine` mặc định = `Machine<StaticStore, TestObserver>`).
//!
//! # Vì sao `const EMPTY`
//!
//! `Machine::new()`/`with_harts()` là `const fn` (khởi tạo `.bss` tĩnh của kernel), nên observer
//! phải có hằng khởi tạo **const** — `Default::default()` không dùng được trong `const fn`.
//!
//! # Hợp đồng
//!
//! - `CoreObserver: Copy` — `Machine` là `Copy` (D2-84) nên observer cũng phải `Copy`.
//! - Mọi phương thức **không** được panic; `NoopObserver` trả `0`/`&[]`/`false` và không ghi gì.
//! - `finalise_is_full` là **cổng nguyên tử**: `finalise_tcb` kiểm nó **trước** mọi thay đổi
//!   (F1 reviewer-kt59), nên `NoopObserver` trả `false` ⇒ sản phẩm **không bao giờ** từ chối
//!   finalise vì log đầy (đúng: sản phẩm không có log).

use crate::machine::MAX_FINALISE;
use crate::objmeta::FinaliseEntry;
use crate::tcb::{TcbState, TCB_STATE_COUNT};

/// Số ô của một bảng đếm cặp trạng thái (`11 × 11`).
pub const PAIR_SLOTS: usize = TCB_STATE_COUNT * TCB_STATE_COUNT;

/// Dữ liệu **oracle/test** của [`crate::Machine`] — xem tài liệu module.
///
/// `Machine<O>` gọi **đúng** các phương thức này ở ba cửa: `write_state` (đếm cặp),
/// `finalise_tcb` (nhật ký), và các accessor `pair_hit_count`/`transition_coverage`/
/// `forbidden_attempt_count`/`design_coverage`/`finalise_log`.
///
/// **Không** thuộc trait: `finalise_overflow` (bộ đếm chẩn đoán `u32`) vẫn nằm trong
/// [`crate::Machine`] — nó **không** phải dữ liệu oracle (sản phẩm cũng có, luôn `0` vì
/// `NoopObserver::finalise_is_full()` luôn `false`), và giữ nó ở `Machine` giữ nguyên
/// `scalars = 311` mà `kt60_core_parity` của sim pin.
pub trait CoreObserver: Copy {
    /// Giá trị khởi tạo **const** (dùng trong `Machine::new`/`with_harts`).
    const EMPTY: Self;

    /// Ghi nhận một cặp `(from, to)` **hợp lệ** đã đi qua cổng trạng thái.
    fn pair_hit(&mut self, from: TcbState, to: TcbState);

    /// Số lần cặp `(from, to)` đã đi qua cổng (chỉ số thô `0..TCB_STATE_COUNT`).
    fn pair_hit_count(&self, from: usize, to: usize) -> u32;

    /// Ghi nhận một cặp `(from, to)` **cấm** bị thử.
    fn forbidden_hit(&mut self, from: TcbState, to: TcbState);

    /// Số lần cặp **cấm** `(from, to)` bị thử (chỉ số thô).
    fn forbidden_attempt_count(&self, from: usize, to: usize) -> u32;

    /// `(số cặp ALLOWED đã có hit, 48)`.
    fn coverage(&self) -> (usize, usize);

    /// `(số cặp thiết kế đã có hit, 46)` — bỏ 2 cặp `Stopping` reserved.
    fn design_coverage(&self) -> (usize, usize);

    /// Nhật ký finalise **đã đầy**? (`finalise_tcb` kiểm **trước** mọi thay đổi.)
    fn finalise_is_full(&self) -> bool;

    /// Ghi một dòng finalise (chỉ gọi khi `finalise_is_full() == false`).
    fn finalise_push(&mut self, entry: FinaliseEntry);

    /// Nhật ký finalise theo thứ tự (rỗng với [`NoopObserver`]).
    fn finalise_log(&self) -> &[FinaliseEntry];

    /// Ba phần của [`crate::Footprint`] do observer sở hữu (`pair_hits`, `forbidden_hits`,
    /// `finalise_log`) — `[0; 3]` với [`NoopObserver`].
    fn footprint_parts() -> [usize; 3];
}

/// Observer **rỗng** — **0 B** (ZST), dùng cho sản phẩm (`Machine<StaticStore, NoopObserver>`).
///
/// Mọi thao tác là no-op; `finalise_is_full()` luôn `false` nên sản phẩm **không** bao giờ
/// từ chối `finalise_tcb` vì nhật ký (sản phẩm không có nhật ký).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoopObserver;

impl CoreObserver for NoopObserver {
    const EMPTY: Self = Self;

    fn pair_hit(&mut self, _from: TcbState, _to: TcbState) {}

    fn pair_hit_count(&self, _from: usize, _to: usize) -> u32 {
        0
    }

    fn forbidden_hit(&mut self, _from: TcbState, _to: TcbState) {}

    fn forbidden_attempt_count(&self, _from: usize, _to: usize) -> u32 {
        0
    }

    fn coverage(&self) -> (usize, usize) {
        (0, crate::tcb::allowed_count())
    }

    fn design_coverage(&self) -> (usize, usize) {
        (
            0,
            crate::tcb::allowed_count() - crate::tcb::reserved_count(),
        )
    }

    fn finalise_is_full(&self) -> bool {
        false
    }

    fn finalise_push(&mut self, _entry: FinaliseEntry) {}

    fn finalise_log(&self) -> &[FinaliseEntry] {
        &[]
    }

    fn footprint_parts() -> [usize; 3] {
        [0; 3]
    }
}

/// Observer **đầy đủ** — giữ nguyên ba bảng của `Machine` trước KT-405 (sim/ktest + test host).
#[derive(Debug, Clone, Copy)]
pub struct TestObserver {
    pair_hits: [u32; PAIR_SLOTS],
    forbidden_hits: [u32; PAIR_SLOTS],
    finalise_log: [FinaliseEntry; MAX_FINALISE],
    /// Số dòng đã ghi — `u16` (biên `MAX_FINALISE = 256`) để `TestObserver` **align 4**,
    /// không đẩy `Machine` lên đệm 8 B (KT-405: giữ `padding ≤ 8` của test pin D2-84).
    n_finalise: u16,
}

impl TestObserver {
    /// Observer rỗng (mọi bộ đếm 0, nhật ký rỗng).
    pub const EMPTY: Self = Self {
        pair_hits: [0; PAIR_SLOTS],
        forbidden_hits: [0; PAIR_SLOTS],
        finalise_log: [FinaliseEntry {
            tcb: 0,
            state: TcbState::Dead,
            released_sched_context: None,
        }; MAX_FINALISE],
        n_finalise: 0,
    };
}

#[cfg(test)]
impl TestObserver {
    /// **Test-only** — đặt số dòng nhật ký đã ghi (dựng ca "log đầy" của F1 reviewer-kt59 mà
    /// không phải finalise 256 TCB thật). Không dùng ở sản phẩm.
    pub(crate) fn set_finalise_len(&mut self, n: usize) {
        self.n_finalise = n.min(MAX_FINALISE) as u16;
    }
}

impl CoreObserver for TestObserver {
    const EMPTY: Self = Self::EMPTY;

    fn pair_hit(&mut self, from: TcbState, to: TcbState) {
        self.pair_hits[from.index() * TCB_STATE_COUNT + to.index()] += 1;
    }

    fn pair_hit_count(&self, from: usize, to: usize) -> u32 {
        self.pair_hits[from * TCB_STATE_COUNT + to]
    }

    fn forbidden_hit(&mut self, from: TcbState, to: TcbState) {
        self.forbidden_hits[from.index() * TCB_STATE_COUNT + to.index()] += 1;
    }

    fn forbidden_attempt_count(&self, from: usize, to: usize) -> u32 {
        self.forbidden_hits[from * TCB_STATE_COUNT + to]
    }

    fn coverage(&self) -> (usize, usize) {
        crate::tcb::coverage(&self.pair_hits)
    }

    fn design_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage_design(&self.pair_hits)
    }

    fn finalise_is_full(&self) -> bool {
        self.n_finalise as usize >= MAX_FINALISE
    }

    fn finalise_push(&mut self, entry: FinaliseEntry) {
        self.finalise_log[self.n_finalise as usize] = entry;
        self.n_finalise += 1;
    }

    fn finalise_log(&self) -> &[FinaliseEntry] {
        &self.finalise_log[..self.n_finalise as usize]
    }

    fn footprint_parts() -> [usize; 3] {
        [
            core::mem::size_of::<[u32; PAIR_SLOTS]>(),
            core::mem::size_of::<[u32; PAIR_SLOTS]>(),
            core::mem::size_of::<[FinaliseEntry; MAX_FINALISE]>(),
        ]
    }
}

// **KT-405** — bằng chứng **0 B** của observer sản phẩm (D2-259 mục 4). Đây là `const` assert
// (không phải test) nên nó chạy cả khi build sản phẩm bare-metal, không chỉ `cargo test`.
const _: () = assert!(
    core::mem::size_of::<NoopObserver>() == 0,
    "NoopObserver phải 0 B (ZST) — sản phẩm không trả byte cho dữ liệu oracle"
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;
    use crate::store::StaticStore;

    /// **KT-405** — bằng chứng đo được của DoD "`NoopObserver` 0 B ở sản phẩm":
    /// `Machine<StaticStore, NoopObserver>` nhỏ hơn `Machine<StaticStore, TestObserver>` **đúng** phần dữ liệu oracle
    /// (hai bảng đếm cặp + nhật ký finalise), và vẫn nằm dưới trần D2-84.
    #[test]
    fn kt405_noop_observer_0_b_va_machine_san_pham_nho_hon() {
        assert_eq!(
            core::mem::size_of::<NoopObserver>(),
            0,
            "NoopObserver phải là ZST"
        );
        let test = core::mem::size_of::<Machine<StaticStore, TestObserver>>();
        let noop = core::mem::size_of::<Machine<StaticStore, NoopObserver>>();
        let oracle = core::mem::size_of::<TestObserver>();
        assert_eq!(
            test - noop,
            oracle,
            "chênh lệch phải đúng bằng dữ liệu oracle ({oracle} B)"
        );
        assert!(
            noop <= 184_832,
            "Machine<StaticStore, NoopObserver> = {noop} B > trần KT-312 184.832 B"
        );
        // Sản phẩm: finalise **không bao giờ** bị từ chối vì nhật ký (không có nhật ký).
        let m = Machine::<StaticStore, NoopObserver>::with_observer(1);
        assert!(!m.obs.finalise_is_full());
        assert!(m.obs.finalise_log().is_empty());
        assert_eq!(m.finalise_overflow(), 0);
        assert_eq!(m.obs.pair_hit_count(0, 0), 0);
        assert_eq!(m.obs.coverage().0, 0);
    }
}
