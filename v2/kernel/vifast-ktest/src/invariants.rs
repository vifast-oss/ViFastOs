// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bộ bất biến dùng chung** của core — 12 nhóm `check_*` (core/IPC/VM/SMP).
//!
//! Trước KT-411 mỗi arch tự viết lại hàm `invariants_ok` (riscv64 `ktest_core.rs`,
//! aarch64/x86_64 `coretest.rs`) với **cùng** danh sách nhưng thứ tự và cách gộp khác nhau —
//! thêm một `check_*` mới ở core thì phải nhớ sửa ba nơi. Tệp này là **một** nguồn.
//!
//! Mười hai nhóm phủ đúng bốn miền mà `B5` đòi dùng chung:
//!
//! | Miền | Nhóm |
//! |---|---|
//! | **core** (cap/MDB/byte) | [`Invariants::caps`], [`Invariants::bytes`], [`Invariants::all_bytes`] |
//! | **SMP** (scheduler/hộp thư) | [`Invariants::sched`], [`Invariants::sc_refs`], [`Invariants::budget`], [`Invariants::mailboxes`] |
//! | **IPC** | [`Invariants::ipc`], [`Invariants::reply_links`], [`Invariants::notifications`], [`Invariants::waitaddr`] |
//! | **VM** (fault/finalise) | [`Invariants::fault`] |
//!
//! Không nới ngưỡng: mỗi cờ là **đúng** `Result::is_ok()` của hàm `check_*` tương ứng trong
//! `vifast-core`; `all_ok()` đòi **cả 12**.

use vifast_core::{Machine, ObjId};

/// Kết quả 12 nhóm bất biến của một `Machine`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invariants {
    /// `check_caps` — bảng cap/MDB.
    pub caps: bool,
    /// `check_bytes(untyped)` — kế toán byte của Untyped đang xét.
    pub bytes: bool,
    /// `check_all_bytes` — kế toán byte **mọi** Untyped.
    pub all_bytes: bool,
    /// `check_sched` — hàng đợi sẵn sàng/`published`/`current`.
    pub sched: bool,
    /// `check_sc_refs` — tham chiếu SchedContext hai chiều.
    pub sc_refs: bool,
    /// `check_budget` — ngân sách MCS.
    pub budget: bool,
    /// `check_mailboxes` — hộp thư theo hart.
    pub mailboxes: bool,
    /// `check_ipc` — Endpoint/Reply/hàng đợi.
    pub ipc: bool,
    /// `check_reply_links` — liên kết Reply ↔ caller.
    pub reply_links: bool,
    /// `check_notifications` — Notification ↔ waiter.
    pub notifications: bool,
    /// `check_waitaddr` — bucket futex.
    pub waitaddr: bool,
    /// `check_fault` — đường fault/`fault_ep`.
    pub fault: bool,
}

impl Invariants {
    /// Số nhóm — **12** (host test pin con số này để thêm/bớt nhóm là đỏ có chủ đích).
    pub const GROUPS: usize = 12;

    /// Số nhóm của kiểu này (bằng [`Self::GROUPS`]).
    pub const fn groups(&self) -> usize {
        Self::GROUPS
    }

    /// `true` ⇔ **cả 12** nhóm xanh.
    pub fn all_ok(&self) -> bool {
        self.caps
            && self.bytes
            && self.all_bytes
            && self.sched
            && self.sc_refs
            && self.budget
            && self.mailboxes
            && self.ipc
            && self.reply_links
            && self.notifications
            && self.waitaddr
            && self.fault
    }

    /// Số nhóm **đỏ** (0 ⇔ [`Self::all_ok`]).
    pub fn failures(&self) -> usize {
        [
            self.caps,
            self.bytes,
            self.all_bytes,
            self.sched,
            self.sc_refs,
            self.budget,
            self.mailboxes,
            self.ipc,
            self.reply_links,
            self.notifications,
            self.waitaddr,
            self.fault,
        ]
        .iter()
        .filter(|ok| !**ok)
        .count()
    }
}

/// Chạy **cả 12** nhóm `check_*` trên `m`; `untyped` là Untyped dùng cho `check_bytes`.
pub fn check_all(m: &Machine, untyped: ObjId) -> Invariants {
    Invariants {
        caps: m.check_caps().is_ok(),
        bytes: m.check_bytes(untyped).is_ok(),
        all_bytes: m.check_all_bytes().is_ok(),
        sched: m.check_sched().is_ok(),
        sc_refs: m.check_sc_refs().is_ok(),
        budget: m.check_budget().is_ok(),
        mailboxes: m.check_mailboxes().is_ok(),
        ipc: m.check_ipc().is_ok(),
        reply_links: m.check_reply_links().is_ok(),
        notifications: m.check_notifications().is_ok(),
        waitaddr: m.check_waitaddr().is_ok(),
        fault: m.check_fault().is_ok(),
    }
}
