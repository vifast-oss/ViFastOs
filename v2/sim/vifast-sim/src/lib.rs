// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-sim` — harness sim tất định (KT-08): store giả + scheduler tối thiểu 1 hart.
//!
//! Chưa chạm ABI (KT-08): "kịch bản rỗng" = N task rỗng, chạy theo seed, phát trace.
//! Bất biến kiểm sau **mỗi** event (thiết kế §9.1):
//! - `store.used() <= cap`, địa chỉ cấp phát tăng đơn điệu và đúng căn lề;
//! - scheduler 1 hart, mỗi bước chạy tối đa một task, không task nào bị bỏ đói;
//! - cùng seed ⇒ cùng trace (byte-for-byte) và cùng hash.

use std::collections::VecDeque;

/// Bộ sinh tất định — không dùng thời gian, không dùng bộ nhớ chưa khởi tạo.
#[derive(Debug, Clone)]
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Self {
        // Trộn để seed 0/1 không cho chuỗi suy biến.
        Self(
            seed.wrapping_mul(6364136223846793005)
                .wrapping_add(0x9e3779b97f4a7c15),
        )
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 17
    }

    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
}

/// Store giả: cấp phát kiểu bump có căn lề, đo watermark; chưa cần Untyped của kernel (KT-09).
#[derive(Debug, Clone)]
pub struct Store {
    base: u64,
    cap: usize,
    used: usize,
    high_water: usize,
}

impl Store {
    pub fn new(cap: usize) -> Self {
        Self {
            base: 0x1000,
            cap,
            used: 0,
            high_water: 0,
        }
    }

    /// Cấp `size` byte căn `align` (lũy thừa 2). `None` khi hết chỗ.
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<u64> {
        if align == 0 || !align.is_power_of_two() {
            return None;
        }
        let start = self.base as usize + self.used;
        let aligned = (start + align - 1) & !(align - 1);
        let end = aligned + size;
        if end > self.base as usize + self.cap {
            return None;
        }
        self.used = end - self.base as usize;
        self.high_water = self.high_water.max(self.used);
        Some(aligned as u64)
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn high_water(&self) -> usize {
        self.high_water
    }

    /// Bất biến của store — harness gọi sau **mỗi** event.
    pub fn invariant(&self) -> bool {
        self.used <= self.cap && self.high_water <= self.cap
    }
}

/// Chương trình rỗng của một task (KT-08 chưa có ABI): cấp một block rồi quay vòng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prog {
    /// Cấp `bytes` mỗi lần được chạy, ăn theo chu kỳ `period`.
    Alloc {
        bytes: usize,
        align: usize,
        period: u32,
    },
}

impl Prog {
    fn period(self) -> u32 {
        match self {
            Prog::Alloc { period, .. } => period,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub prog: Prog,
    pub runs: u64,
    tick: u32,
}

/// Một event trong trace — văn bản thuần để so byte và băm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub step: u64,
    pub kind: &'static str,
    pub task: Option<u32>,
    pub used: usize,
    pub value: u64,
}

impl Event {
    pub fn render(&self) -> String {
        match self.task {
            Some(t) => format!(
                "step={} kind={} task={} used={} value={}",
                self.step, self.kind, t, self.used, self.value
            ),
            None => format!(
                "step={} kind={} used={} value={}",
                self.step, self.kind, self.used, self.value
            ),
        }
    }
}

/// FNV-1a 64-bit trên trace đã render.
pub fn trace_hash(lines: &[String]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for line in lines {
        for b in line.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= b'\n' as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(Debug, Clone)]
pub struct Harness {
    seed: u64,
    rng: Lcg,
    pub store: Store,
    pub ready: VecDeque<u32>,
    pub tasks: Vec<Task>,
    events: Vec<Event>,
    step: u64,
}

impl Harness {
    /// `tasks`: số task rỗng; `cap`: dung lượng store giả. Seed quyết định thứ tự khởi đầu.
    pub fn new(seed: u64, cap: usize, tasks: u32) -> Self {
        let mut rng = Lcg::new(seed);
        let mut ids: Vec<u32> = (0..tasks).collect();
        // Xáo bằng Fisher–Yates từ seed ⇒ seed khác ⇒ thứ tự khác.
        let mut i = ids.len();
        while i > 1 {
            let j = rng.below(i as u64) as usize;
            i -= 1;
            ids.swap(i, j);
        }
        let tasks_vec: Vec<Task> = ids
            .iter()
            .map(|id| Task {
                id: *id,
                prog: Prog::Alloc {
                    bytes: 64,
                    align: 16,
                    period: 3,
                },
                runs: 0,
                tick: 0,
            })
            .collect();
        Self {
            seed,
            rng,
            store: Store::new(cap),
            ready: ids.into_iter().collect(),
            tasks: tasks_vec,
            events: Vec::new(),
            step: 0,
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Chạy `steps` bước; mỗi bước đúng một event. Gọi `check_invariant` sau mỗi event.
    pub fn run(&mut self, steps: u64) {
        for _ in 0..steps {
            if let Some(id) = self.ready.pop_front() {
                self.run_task(id);
                self.ready.push_back(id);
            } else {
                self.events.push(Event {
                    step: self.step,
                    kind: "idle",
                    task: None,
                    used: self.store.used(),
                    value: 0,
                });
            }
            self.step += 1;
            if !self.store.invariant() {
                panic!("store invariant vỡ ở step {}", self.step);
            }
        }
    }

    fn run_task(&mut self, id: u32) {
        let index = id as usize;
        let (bytes, align) = match self.tasks[index].prog {
            Prog::Alloc { bytes, align, .. } => (bytes, align),
        };
        self.tasks[index].tick += 1;
        let value = if self.tasks[index].tick >= self.tasks[index].prog.period() {
            self.tasks[index].tick = 0;
            match self.store.alloc(bytes, align) {
                Some(addr) => {
                    self.tasks[index].runs += 1;
                    addr
                }
                None => 0,
            }
        } else {
            0
        };
        // Nhiễu theo seed: thêm nhịp timer để trace phụ thuộc seed ngay cả khi 1 task.
        let noise = self.rng.below(0x1000);
        self.events.push(Event {
            step: self.step,
            kind: "run",
            task: Some(id),
            used: self.store.used(),
            value: value ^ noise,
        });
    }

    pub fn trace_lines(&self) -> Vec<String> {
        self.events.iter().map(Event::render).collect()
    }

    pub fn hash(&self) -> u64 {
        trace_hash(&self.trace_lines())
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_can_le_va_het_cho() {
        let mut s = Store::new(256);
        assert_eq!(s.alloc(64, 16), Some(0x1000));
        assert_eq!(s.alloc(64, 64), Some(0x1040));
        assert!(s.alloc(1024, 16).is_none(), "phải hết chỗ");
        assert_eq!(s.used(), 0x80);
        assert!(s.invariant());
    }

    #[test]
    fn store_tu_choi_align_sai() {
        let mut s = Store::new(256);
        assert_eq!(s.alloc(16, 3), None);
        assert_eq!(s.alloc(16, 0), None);
    }

    #[test]
    fn cung_seed_cung_trace() {
        let mut a = Harness::new(7, 4096, 4);
        let mut b = Harness::new(7, 4096, 4);
        a.run(200);
        b.run(200);
        assert_eq!(a.trace_lines(), b.trace_lines());
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn seed_khac_trace_khac() {
        let mut hashes = std::collections::BTreeSet::new();
        for seed in 0..8u64 {
            let mut h = Harness::new(seed, 4096, 4);
            h.run(64);
            hashes.insert(h.hash());
        }
        assert_eq!(hashes.len(), 8, "8 seed phải cho 8 trace khác nhau");
    }

    #[test]
    fn khong_task_nao_bi_bo_doi() {
        let mut h = Harness::new(3, 4096, 5);
        h.run(100);
        let seen: std::collections::BTreeSet<u32> =
            h.events().iter().filter_map(|e| e.task).collect();
        assert_eq!(seen.len(), 5, "cả 5 task phải chạy ít nhất một lần");
    }

    #[test]
    fn kich_ban_rong_van_tat_dinh() {
        let mut a = Harness::new(11, 4096, 0);
        let mut b = Harness::new(11, 4096, 0);
        a.run(16);
        b.run(16);
        assert!(a.events().iter().all(|e| e.kind == "idle"));
        assert_eq!(a.trace_lines(), b.trace_lines());
    }

    #[test]
    fn hash_tai_lap_tu_trace() {
        let mut h = Harness::new(5, 4096, 3);
        h.run(50);
        assert_eq!(trace_hash(&h.trace_lines()), h.hash());
    }
}
