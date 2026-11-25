// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-56 — **fault probe phần cứng thật** (chỉ ảnh test, `#[cfg(feature = "ktest")]`).
//!
//! Không tiêm `scause` (D2-73 cấm claim trap tiêm là trap thật): probe thực thi **một**
//! instruction load/store/jump tới địa chỉ đã biết là không hợp lệ, phần cứng trap **thật** vào
//! `stvec`; `trap_dispatch` thấy probe đang "armed" thì ghi lại `scause/stval/sepc/sstatus.SPP`
//! và đặt `sepc` về [`probe_after`] để `sret` chạy tiếp ngay sau instruction lỗi. Nhờ vậy một
//! boot kiểm được nhiều fault (page fault 12/13/15) mà không halt và không mô phỏng.
//!
//! Dùng ở hai nơi:
//! - TAP `faultkind vm load/store/insn` của ảnh `ktest`;
//! - pha conformance `.scn` 25/26: `vm_expect_fault` gây trap thật trên VA của rig (sau khi đổi
//!   `satp` sang root VSpace), rồi phân loại từ `scause/stval` + PTE thật.
//!
//! `unsafe` nằm trong danh sách đóng D2-17 (`kernel/vifast-plat-*`); logic phân loại ở
//! `vifast-plat-common::fault` (host-test được).

use core::arch::global_asm;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_riscv64_logic::trap::TrapFrame;

/// Loại truy cập của probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeKind {
    /// `ld` từ địa chỉ đích (`scause` 13 nếu phép đọc fault).
    Load,
    /// `sd` tới địa chỉ đích (`scause` 15 nếu phép ghi fault).
    Store,
    /// `jr` tới địa chỉ đích (`scause` 12 nếu fetch fault).
    Exec,
}

impl ProbeKind {
    /// Mã cho asm (0/1/2).
    pub const fn code(self) -> u64 {
        match self {
            ProbeKind::Load => 0,
            ProbeKind::Store => 1,
            ProbeKind::Exec => 2,
        }
    }
}

/// Trap **thật** đã quan sát trong lúc probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeTrap {
    /// Loại truy cập đã yêu cầu.
    pub kind: ProbeKind,
    /// `scause` nguyên giá trị.
    pub scause: u64,
    /// `stval`.
    pub stval: u64,
    /// `sepc` tại thời điểm trap.
    pub sepc: u64,
    /// `sstatus.SPP` tại thời điểm trap (0 = U-mode).
    pub spp: u64,
}

static ARMED: AtomicU64 = AtomicU64::new(0);
static HIT: AtomicU64 = AtomicU64::new(0);
static P_SCAUSE: AtomicU64 = AtomicU64::new(0);
static P_STVAL: AtomicU64 = AtomicU64::new(0);
static P_SEPC: AtomicU64 = AtomicU64::new(0);
static P_SPP: AtomicU64 = AtomicU64::new(0);

extern "C" {
    /// Thực thi instruction của probe rồi `ret`; `a0` = kind, `a1` = địa chỉ.
    fn probe_fault_entry(kind: u64, addr: u64);
    /// Nhãn sau instruction lỗi — `sepc` được đặt về đây khi trap.
    fn probe_after();
}

/// Thực thi **một** instruction gây fault thật tại `addr`.
///
/// Trả `Some` ⇔ phần cứng đã trap và handler của kernel đã ghi nhận; `None` ⇔ không fault
/// (kỳ vọng của caller sai — TAP đỏ, không im lặng).
pub fn probe_fault(kind: ProbeKind, addr: u64) -> Option<ProbeTrap> {
    HIT.store(0, Ordering::SeqCst);
    ARMED.store(1, Ordering::SeqCst);
    // SAFETY: `probe_fault_entry` chỉ chạy đúng một instruction load/store/jr tới `addr` do
    // caller chọn (địa chỉ **đã biết là không map/quyền sai**); nếu không fault, nó nhảy tới
    // `probe_after` và `ret`. Không ghi bộ nhớ hợp lệ nào ngoài `sd` tới trang mà caller muốn
    // kiểm là **fault** (trang read-only/không map) — trap xảy ra trước khi hoàn tất.
    unsafe { probe_fault_entry(kind.code(), addr) };
    ARMED.store(0, Ordering::SeqCst);
    if HIT.swap(0, Ordering::SeqCst) == 0 {
        return None;
    }
    Some(ProbeTrap {
        kind,
        scause: P_SCAUSE.load(Ordering::SeqCst),
        stval: P_STVAL.load(Ordering::SeqCst),
        sepc: P_SEPC.load(Ordering::SeqCst),
        spp: P_SPP.load(Ordering::SeqCst),
    })
}

/// `trap_dispatch` gọi **trước** mọi chính sách fatal: `true` ⇔ trap thuộc probe đang armed.
///
/// Ghi lại trapframe (nguồn duy nhất của `FaultKind`), đặt `sepc` về `probe_after` để resume
/// ngay sau instruction lỗi, rồi trả `true` (handler không dump/PANIC).
pub fn on_trap(tf: &mut TrapFrame) -> bool {
    if ARMED.load(Ordering::SeqCst) == 0 {
        return false;
    }
    P_SCAUSE.store(tf.scause, Ordering::SeqCst);
    P_STVAL.store(tf.stval, Ordering::SeqCst);
    P_SEPC.store(tf.sepc, Ordering::SeqCst);
    P_SPP.store(u64::from(tf.from_supervisor()), Ordering::SeqCst);
    tf.sepc = probe_after as *const () as usize as u64;
    ARMED.store(0, Ordering::SeqCst);
    HIT.store(1, Ordering::SeqCst);
    true
}

global_asm!(
    ".section .text",
    ".balign 4",
    ".globl probe_fault_entry",
    "probe_fault_entry:",
    // a0 = kind: 0 Load, 1 Store, 2 Exec; a1 = địa chỉ đích. Không đụng `ra`/`sp`.
    "li t0, 0",
    "beq a0, t0, 1f",
    "li t0, 1",
    "beq a0, t0, 2f",
    // Exec: fetch tại a1 (fault ⇒ scause 12, stval/sepc = a1).
    "jr a1",
    "j probe_after",
    "1:",
    "ld t0, 0(a1)",
    "j probe_after",
    "2:",
    "sd zero, 0(a1)",
    "j probe_after",
    ".globl probe_after",
    "probe_after:",
    "ret",
);
