// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-sim` — harness sim tất định (KT-08): store giả + scheduler tối thiểu 1 hart.
//!
//! Chưa chạm ABI (KT-08): "kịch bản rỗng" = N task rỗng, chạy theo seed, phát trace.
//! Bất biến kiểm sau **mỗi** event (thiết kế §9.1):
//! - `store.used() <= cap`, địa chỉ cấp phát tăng đơn điệu và đúng căn lề;
//! - scheduler 1 hart, mỗi bước chạy tối đa một task, không task nào bị bỏ đói;
//! - cùng seed ⇒ cùng trace (byte-for-byte) và cùng hash.

use std::collections::VecDeque;

use vifast_objmem::OwnedArena;

pub mod kobj;
pub mod scenario;
pub mod tcb;

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

/// Store của sim: bump allocator **của `vifast-objmem`** (KT-16, D2-14) + căn lề/watermark.
///
/// Địa chỉ trả về là **địa chỉ logic** (`0x1000 + offset trong arena`) để trace/golden của
/// sim vẫn tất định; vùng nhớ thật do [`vifast_objmem::OwnedArena`] sở hữu và căn lề 4096.
/// `used()`/`cap()` là watermark/dung lượng của arena objmem, nên không còn bộ đếm bump
/// viết tay nào trong `v2/sim/**`.
#[derive(Debug)]
pub struct Store {
    base: u64,
    arena: OwnedArena,
    high_water: usize,
}

impl Store {
    pub fn new(cap: usize) -> Self {
        Self {
            base: 0x1000,
            arena: OwnedArena::new(cap).expect("cấp vùng nhớ cho store của sim"),
            high_water: 0,
        }
    }

    /// Cấp `size` byte căn `align` (lũy thừa 2, ≤ `ARENA_ALIGN` = 4096). `None` khi hết chỗ.
    ///
    /// **Hợp đồng chốt ở KT-17** (follow-up KT-16): `align` không phải lũy thừa 2, `align = 0`,
    /// hoặc `align > ARENA_ALIGN` ⇒ `None` (không đốt byte, không trả địa chỉ sai căn) — đáy
    /// vùng chỉ được bảo đảm căn `ARENA_ALIGN`. Mô hình chỉ dùng 16/32/64 nên không đổi
    /// test/L1; `pa` trả về là địa chỉ **logic** `0x1000 + offset` (trace/golden tất định).
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<u64> {
        let offset = self.arena.alloc_offset(size, align)?;
        self.high_water = self.high_water.max(self.arena.used());
        Some(self.base + offset as u64)
    }

    pub fn used(&self) -> usize {
        self.arena.used()
    }

    pub fn cap(&self) -> usize {
        self.arena.capacity()
    }

    pub fn high_water(&self) -> usize {
        self.high_water
    }

    /// Địa chỉ logic của đáy vùng (KT-16: bump do objmem cấp, không phải hằng số giả).
    pub fn arena_base(&self) -> u64 {
        self.base
    }

    /// Watermark của arena objmem — bằng `used()`.
    pub fn arena_used(&self) -> usize {
        self.arena.used()
    }

    /// Bất biến của store — harness gọi sau **mỗi** event.
    pub fn invariant(&self) -> bool {
        self.used() <= self.cap() && self.high_water <= self.cap()
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

#[derive(Debug)]
pub struct Harness {
    seed: u64,
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
        // Quỹ đạo store phải phụ thuộc seed: mỗi task nhận bytes/align/period suy từ seed.
        let mut tasks_vec: Vec<Task> = Vec::with_capacity(ids.len());
        for id in &ids {
            let bytes = 32usize << rng.below(3); // 32 / 64 / 128
            let align = 16usize << rng.below(2); // 16 / 32
            let period = 1 + rng.below(4) as u32; // 1..=4
            tasks_vec.push(Task {
                id: *id,
                prog: Prog::Alloc {
                    bytes,
                    align,
                    period,
                },
                runs: 0,
                tick: 0,
            });
        }
        Self {
            seed,
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
        self.events.push(Event {
            step: self.step,
            kind: "run",
            task: Some(id),
            used: self.store.used(),
            value,
        });
    }

    pub fn trace_lines(&self) -> Vec<String> {
        self.events.iter().map(Event::render).collect()
    }

    /// Dòng đầu của bản dump: cấu hình tái lập được (review #11 — hash phải quy được về cấu hình).
    pub fn header(&self, steps: u64) -> String {
        format!(
            "# scn seed={} steps={} tasks={} cap={}",
            self.seed,
            steps,
            self.tasks.len(),
            self.store.cap()
        )
    }

    /// Trace đầy đủ kèm header — dùng cho golden và cho CLI.
    pub fn dump(&self, steps: u64) -> Vec<String> {
        let mut out = vec![self.header(steps)];
        out.extend(self.trace_lines());
        out
    }

    pub fn dump_hash(&self, steps: u64) -> u64 {
        trace_hash(&self.dump(steps))
    }

    #[allow(dead_code)] // dùng trong unit test và công cụ cũ; dump_hash mới là hash có header
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
