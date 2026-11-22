// SPDX-License-Identifier: MIT OR Apache-2.0
//! TCB state + **bảng chuyển trạng thái `ALLOWED` 11×11** (KT-59, port từ
//! `v2/sim/vifast-sim/src/tcb.rs`, oracle).
//!
//! Model-only `[TK]`, **không** phải spec/ABI: tên 11 trạng thái lấy từ
//! `spec/abi.toml` `[method_signature.state]` (qua `vifast_abi::TCB_STATES`, D2-33); bảng
//! chuyển trạng thái là mô hình K1b (D2-49(d)) — sim giữ nguồn, core port **nguyên** bảng,
//! cùng thứ tự hàng/cột và cùng lý do từng hàng.
//!
//! Mọi đường đổi trạng thái TCB của core đi qua **một cửa** `Machine::transition_tcb` như sim
//! (`tcb_configure` không đổi state; `suspend`/`resume`/`tick`/`dispatch`/`block_current`/
//! `wake`/`finalise` đều gọi cổng này).

use crate::error::{CapId, ObjId};

/// Số trạng thái của `TcbState` — khớp `vifast_abi::TCB_STATES` (D2-33).
pub const TCB_STATE_COUNT: usize = 11;

/// Quyền truy cập gây fault VM (`FaultKind::Vm.access`) — model-only `[TK]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultAccess {
    /// Đọc.
    Read,
    /// Ghi.
    Write,
    /// Thực thi.
    Execute,
}

impl FaultAccess {
    /// Tên ổn định (dùng cho digest/log).
    pub fn name(self) -> &'static str {
        match self {
            FaultAccess::Read => "Read",
            FaultAccess::Write => "Write",
            FaultAccess::Execute => "Execute",
        }
    }
}

/// Nguyên nhân fault VM — model-only `[TK]` (KT-41; core chỉ giữ để `TcbState` đủ payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmFaultReason {
    /// Chưa map.
    NotMapped,
    /// Thiếu quyền.
    Permission,
    /// Truy cập tạo W+X.
    Wx,
}

impl VmFaultReason {
    /// Tên ổn định.
    pub fn name(self) -> &'static str {
        match self {
            VmFaultReason::NotMapped => "NotMapped",
            VmFaultReason::Permission => "Permission",
            VmFaultReason::Wx => "Wx",
        }
    }
}

/// Nguyên nhân fault mang theo trạng thái chặn (D2-25/D2-33) — model-only `[TK]`.
///
/// KT-59 port **kiểu dữ liệu** để `TcbState` khớp sim; đường tạo fault (IPC/fault injection)
/// thuộc KT-60. `Exit` là biến thể duy nhất có trong `spec/`; ba biến thể còn lại là mô hình
/// K1c/K3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// Thoát với mã (D2-25).
    Exit {
        /// Mã thoát.
        code: u64,
    },
    /// Fault bộ nhớ (KT-41).
    Vm {
        /// VSpace bị chạm.
        vspace: ObjId,
        /// Địa chỉ yêu cầu.
        va: u64,
        /// Quyền truy cập.
        access: FaultAccess,
        /// Lý do.
        reason: VmFaultReason,
    },
    /// Syscall lạ.
    UnknownSyscall {
        /// Số syscall.
        nr: u64,
    },
    /// Exception user.
    UserException {
        /// IP.
        ip: u64,
        /// Loại.
        kind: u64,
    },
}

impl FaultKind {
    /// Tên biến thể.
    pub fn name(self) -> &'static str {
        match self {
            FaultKind::Exit { .. } => "Exit",
            FaultKind::Vm { .. } => "Vm",
            FaultKind::UnknownSyscall { .. } => "UnknownSyscall",
            FaultKind::UserException { .. } => "UserException",
        }
    }

    /// Chỉ `Exit` đi đường gửi không Reply (D2-25).
    pub fn is_exit(self) -> bool {
        matches!(self, FaultKind::Exit { .. })
    }

    /// Mã thoát nếu là `Exit`.
    pub fn exit_code(self) -> Option<u64> {
        match self {
            FaultKind::Exit { code } => Some(code),
            _ => None,
        }
    }
}

/// Lý do `Sleeping` — variants của `spec/[method_signature.state.Sleeping]` (D2-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepReason {
    /// Hết timeout.
    Timeout,
    /// Hết ngân sách.
    Budget,
}

impl SleepReason {
    /// Tên ổn định.
    pub fn name(self) -> &'static str {
        match self {
            SleepReason::Timeout => "Timeout",
            SleepReason::Budget => "Budget",
        }
    }
}

/// Trạng thái TCB — **đủ 11 tên** của `spec/[method_signature.state]` (D2-33); thứ tự khai
/// báo **chính là** thứ tự chỉ số của bảng [`ALLOWED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcbState {
    /// Chưa kích hoạt.
    Inactive,
    /// Sẵn sàng chạy (trong ready queue nếu `!suspended`).
    Ready,
    /// Đang chạy trên hart.
    Running {
        /// Hart đang chạy.
        hart: u16,
    },
    /// Đang chờ gửi.
    BlockedOnSend {
        /// Endpoint chờ.
        ep: ObjId,
        /// Lời gọi (Call) hay gửi thường.
        call: bool,
        /// Fault đi kèm (nếu có).
        fault: Option<FaultKind>,
    },
    /// Đang chờ nhận.
    BlockedOnRecv {
        /// Endpoint chờ.
        ep: ObjId,
        /// Cap Reply (nếu có).
        reply: Option<ObjId>,
    },
    /// Đang chờ Reply.
    BlockedOnReply {
        /// Reply object.
        reply: ObjId,
    },
    /// Đang chờ Notification.
    BlockedOnNotification {
        /// Notification object.
        ntfn: ObjId,
    },
    /// Đang chờ địa chỉ.
    BlockedOnAddress {
        /// Frame.
        frame: ObjId,
        /// Offset.
        offset: u32,
    },
    /// Đang ngủ.
    Sleeping {
        /// Lý do ngủ.
        reason: SleepReason,
    },
    /// Đang dừng.
    Stopping,
    /// Đã chết (trạng thái cuối).
    Dead,
}

/// Loại hàng chờ của `Tcb.ReadState.blocked_on_kind` (D2-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedOnKind {
    /// Chờ gửi.
    Send,
    /// Chờ nhận.
    Recv,
    /// Chờ Reply.
    Reply,
    /// Chờ Notification.
    Notification,
    /// Chờ địa chỉ.
    Address,
}

impl BlockedOnKind {
    /// Tên ổn định.
    pub fn name(self) -> &'static str {
        match self {
            BlockedOnKind::Send => "Send",
            BlockedOnKind::Recv => "Recv",
            BlockedOnKind::Reply => "Reply",
            BlockedOnKind::Notification => "Notification",
            BlockedOnKind::Address => "Address",
        }
    }
}

/// `Tcb.Suspend(mode)` — variants đã duyệt của `v2/docs/abi-call-signatures.toml:51-54`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspendMode {
    /// Huỷ thao tác đang chờ rồi treo.
    Abort,
    /// Hoãn (mô hình K1b: `InvalidArgument` vì chưa có `Restart`).
    Defer,
}

impl SuspendMode {
    /// Tên ổn định.
    pub fn name(self) -> &'static str {
        match self {
            SuspendMode::Abort => "Abort",
            SuspendMode::Defer => "Defer",
        }
    }
}

/// **KT-329** — trạng thái treo **per-TCB**, đúng **1 byte** (thay `bool` cũ, cũng 1 byte ⇒
/// `size_of::<Machine>()` **không đổi**; headroom chỉ **40 B** nên **cấm** thêm trường).
///
/// Tách khỏi `TcbInfo::suspended` (vẫn `bool`): chỉ [`SuspendFlag::Abort`] hiện ra là "đang
/// treo". [`SuspendFlag::Defer`] là **cờ chờ điểm an toàn** — TCB **vẫn** `Running`/`Ready` và
/// **vẫn** được scheduler chọn (nếu bị loại khỏi hàng chạy, nó không bao giờ chạy syscall kế
/// tiếp để tới điểm an toàn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuspendFlag {
    /// Không treo.
    None,
    /// `Suspend(Abort)` đường **model** KT-65 (tương đương `suspended = true` trước KT-329).
    Abort,
    /// `Suspend(Defer)` — chờ **điểm an toàn** kế tiếp (cuối `syscall::dispatch`).
    Defer,
}

impl SuspendFlag {
    /// Cờ treo **quan sát được** qua `Tcb.ReadState`/`TcbInfo`: chỉ `Abort` — giữ nguyên ngữ
    /// nghĩa KT-65/KT-319 (đường invocation `Abort` **không** đặt cờ).
    pub(crate) fn is_suspended(self) -> bool {
        matches!(self, SuspendFlag::Abort)
    }
}

/// Bốn giá trị trả của `Tcb.ReadState` (thiết kế §4.4 `:282`, D2-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcbSnapshot {
    /// Trạng thái.
    pub state: TcbState,
    /// Cờ treo (trực giao với `state`).
    pub suspended: bool,
    /// Loại hàng chờ.
    pub blocked_on_kind: Option<BlockedOnKind>,
    /// Mã thoát (nếu có).
    pub exit_code: Option<u64>,
}

/// Ảnh chụp **toàn bộ** trường TCB của mô hình — model API `[TK]` (D2-49(b,c)); parity
/// sim↔core so directly từng trường.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcbInfo {
    /// Trạng thái.
    pub state: TcbState,
    /// Cờ treo.
    pub suspended: bool,
    /// Mã thoát.
    pub exit_code: Option<u64>,
    /// Cap `fault_ep`.
    pub fault_ep: Option<CapId>,
    /// CSpace (CNode).
    pub cspace: Option<ObjId>,
    /// Priority gốc.
    pub priority: u8,
    /// MCP.
    pub max_controlled_priority: u8,
    /// Affinity mask.
    pub affinity: u64,
    /// Hart home.
    pub home: u16,
    /// SchedContext riêng.
    pub sched_context: Option<ObjId>,
    /// SchedContext đang nhận donation (KT-21 — KT-59 chưa tạo donation, luôn `None`).
    pub donated_sc: Option<ObjId>,
    /// Notification đang bind (KT-27 — KT-59 chưa bind, luôn `None`).
    pub bound_notification: Option<ObjId>,
    /// **KT-74** — con trỏ TLS (`Tcb.SetTls`, method 15 của `[object].Tcb`; KHÔNG phải
    /// `Model` API của sim). `0` = chưa đặt. Kernel nạp giá trị này vào `x4`/`tp` khi resume.
    pub tls: u64,
}

impl TcbState {
    /// Chỉ số trong [`ALLOWED`] — khớp `vifast_abi::TCB_STATES`.
    pub fn index(&self) -> usize {
        match self {
            TcbState::Inactive => 0,
            TcbState::Ready => 1,
            TcbState::Running { .. } => 2,
            TcbState::BlockedOnSend { .. } => 3,
            TcbState::BlockedOnRecv { .. } => 4,
            TcbState::BlockedOnReply { .. } => 5,
            TcbState::BlockedOnNotification { .. } => 6,
            TcbState::BlockedOnAddress { .. } => 7,
            TcbState::Sleeping { .. } => 8,
            TcbState::Stopping => 9,
            TcbState::Dead => 10,
        }
    }

    /// Tên trạng thái (không kèm trường) — khớp `vifast_abi::TCB_STATES`.
    pub fn name(&self) -> &'static str {
        match self {
            TcbState::Inactive => "Inactive",
            TcbState::Ready => "Ready",
            TcbState::Running { .. } => "Running",
            TcbState::BlockedOnSend { .. } => "BlockedOnSend",
            TcbState::BlockedOnRecv { .. } => "BlockedOnRecv",
            TcbState::BlockedOnReply { .. } => "BlockedOnReply",
            TcbState::BlockedOnNotification { .. } => "BlockedOnNotification",
            TcbState::BlockedOnAddress { .. } => "BlockedOnAddress",
            TcbState::Sleeping { .. } => "Sleeping",
            TcbState::Stopping => "Stopping",
            TcbState::Dead => "Dead",
        }
    }

    /// Chỉ `Inactive`/`Ready`/`Running` là chạy được.
    pub fn is_runnable(&self) -> bool {
        matches!(
            self,
            TcbState::Inactive | TcbState::Ready | TcbState::Running { .. }
        )
    }

    /// Đang nằm trong một hàng chờ (`BlockedOn*`).
    pub fn is_blocked(&self) -> bool {
        matches!(
            self,
            TcbState::BlockedOnSend { .. }
                | TcbState::BlockedOnRecv { .. }
                | TcbState::BlockedOnReply { .. }
                | TcbState::BlockedOnNotification { .. }
                | TcbState::BlockedOnAddress { .. }
        )
    }

    /// Trạng thái "sống" (mọi trạng thái trừ `Stopping`/`Dead`).
    pub fn is_live(&self) -> bool {
        !matches!(self, TcbState::Stopping | TcbState::Dead)
    }

    /// Loại hàng chờ tương ứng.
    pub fn blocked_on_kind(&self) -> Option<BlockedOnKind> {
        match self {
            TcbState::BlockedOnSend { .. } => Some(BlockedOnKind::Send),
            TcbState::BlockedOnRecv { .. } => Some(BlockedOnKind::Recv),
            TcbState::BlockedOnReply { .. } => Some(BlockedOnKind::Reply),
            TcbState::BlockedOnNotification { .. } => Some(BlockedOnKind::Notification),
            TcbState::BlockedOnAddress { .. } => Some(BlockedOnKind::Address),
            _ => None,
        }
    }
}

/// Bảng chuyển trạng thái TCB **11×11** (D2-49(d)) — **port nguyên** bảng của sim
/// (`v2/sim/vifast-sim/src/tcb.rs`), cùng thứ tự hàng/cột:
///
/// ```text
///        In  Re  Ru  BS  BR  BP  BN  BA  Sl  St  De
/// In     .   x   .   x   .   .   .   .   .   .   x
/// Re     x   .   x   x   x   x   x   x   .   x   x
/// Ru     x   x   .   x   x   x   x   x   x   x   x
/// BS     x   x   .   x   .   x   .   .   .   .   x
/// BR     x   x   .   x   .   .   .   .   .   .   x
/// BP     x   x   .   x   .   .   .   .   .   .   x
/// BN     x   x   .   x   .   .   .   .   .   .   x
/// BA     x   x   .   x   .   .   .   .   .   .   x
/// Sl     x   x   .   x   .   .   .   .   .   .   x
/// St     .   .   .   .   .   .   .   .   .   .   x
/// De     .   .   .   .   .   .   .   .   .   .   .
/// ```
///
/// (`x` = ALLOWED, `.` = cặp cấm ⇒ `IllegalOperation`, **không** đổi trạng thái.) Cặp danh
/// tính duy nhất là `BlockedOnSend → BlockedOnSend` (Exit khi đang chờ gửi, D2-25).
pub const ALLOWED: [[bool; TCB_STATE_COUNT]; TCB_STATE_COUNT] = [
    // Inactive
    [
        false, true, false, true, false, false, false, false, false, false, true,
    ],
    // Ready
    [
        true, false, true, true, true, true, true, true, false, true, true,
    ],
    // Running
    [
        true, true, false, true, true, true, true, true, true, true, true,
    ],
    // BlockedOnSend
    [
        true, true, false, true, false, true, false, false, false, false, true,
    ],
    // BlockedOnRecv
    [
        true, true, false, true, false, false, false, false, false, false, true,
    ],
    // BlockedOnReply
    [
        true, true, false, true, false, false, false, false, false, false, true,
    ],
    // BlockedOnNotification
    [
        true, true, false, true, false, false, false, false, false, false, true,
    ],
    // BlockedOnAddress
    [
        true, true, false, true, false, false, false, false, false, false, true,
    ],
    // Sleeping
    [
        true, true, false, true, false, false, false, false, false, false, true,
    ],
    // Stopping
    [
        false, false, false, false, false, false, false, false, false, false, true,
    ],
    // Dead
    [
        false, false, false, false, false, false, false, false, false, false, false,
    ],
];

/// Cặp `(from, to)` có được phép chuyển không.
pub fn allowed(from: TcbState, to: TcbState) -> bool {
    ALLOWED[from.index()][to.index()]
}

/// Số cặp ALLOWED của bảng (48).
pub fn allowed_count() -> usize {
    ALLOWED.iter().flatten().filter(|ok| **ok).count()
}

/// Cặp **`Stopping` reserved** cho K1c: `Ready → Stopping`, `Running → Stopping`.
pub fn is_reserved(from: TcbState, to: TcbState) -> bool {
    matches!(from, TcbState::Ready | TcbState::Running { .. }) && matches!(to, TcbState::Stopping)
}

/// Số cặp reserved (2).
pub fn reserved_count() -> usize {
    ALLOWED
        .iter()
        .enumerate()
        .flat_map(|(from, row)| row.iter().enumerate().map(move |(to, ok)| (*ok, from, to)))
        .filter(|(ok, from, to)| *ok && is_reserved(state_at(*from), state_at(*to)))
        .count()
}

/// Số cặp **cấm** của bảng (`121 − 48 = 73`).
pub fn forbidden_count() -> usize {
    ALLOWED.iter().flatten().filter(|ok| !**ok).count()
}

/// `(số cặp ALLOWED suy ra từ thiết kế đã hit, 46)` — **không** tính 2 cặp `Stopping`.
pub fn coverage_design(pair_hits: &[u32; TCB_STATE_COUNT * TCB_STATE_COUNT]) -> (usize, usize) {
    let mut hit = 0;
    let mut total = 0;
    for from in 0..TCB_STATE_COUNT {
        for to in 0..TCB_STATE_COUNT {
            if !ALLOWED[from][to] || is_reserved(state_at(from), state_at(to)) {
                continue;
            }
            total += 1;
            if pair_hits[from * TCB_STATE_COUNT + to] > 0 {
                hit += 1;
            }
        }
    }
    (hit, total)
}

/// `(số cặp ALLOWED đã hit, tổng 48)`.
pub fn coverage(pair_hits: &[u32; TCB_STATE_COUNT * TCB_STATE_COUNT]) -> (usize, usize) {
    let mut hit = 0;
    let mut total = 0;
    for from in 0..TCB_STATE_COUNT {
        for to in 0..TCB_STATE_COUNT {
            if ALLOWED[from][to] {
                total += 1;
                if pair_hits[from * TCB_STATE_COUNT + to] > 0 {
                    hit += 1;
                }
            }
        }
    }
    (hit, total)
}

/// `TcbState` chỉ để tra bảng theo chỉ số.
fn state_at(index: usize) -> TcbState {
    match index {
        0 => TcbState::Inactive,
        1 => TcbState::Ready,
        2 => TcbState::Running { hart: 0 },
        3 => TcbState::BlockedOnSend {
            ep: 0,
            call: true,
            fault: None,
        },
        4 => TcbState::BlockedOnRecv { ep: 0, reply: None },
        5 => TcbState::BlockedOnReply { reply: 0 },
        6 => TcbState::BlockedOnNotification { ntfn: 0 },
        7 => TcbState::BlockedOnAddress {
            frame: 0,
            offset: 0,
        },
        8 => TcbState::Sleeping {
            reason: SleepReason::Budget,
        },
        9 => TcbState::Stopping,
        _ => TcbState::Dead,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bảng 11×11, 48 cặp ALLOWED, 2 reserved, 73 cấm; cặp danh tính duy nhất là (3,3).
    #[test]
    fn bang_allowed_11x11_dung_sim() {
        assert_eq!(ALLOWED.len(), TCB_STATE_COUNT);
        assert_eq!(allowed_count(), 48);
        assert_eq!(reserved_count(), 2);
        assert_eq!(forbidden_count(), 73);
        for (i, row) in ALLOWED.iter().enumerate() {
            assert_eq!(row.len(), TCB_STATE_COUNT);
            if i != 3 {
                assert!(!row[i], "hàng {i}: cặp danh tính phải cấm");
            }
        }
        assert!(ALLOWED[3][3]);
        let hits = [0u32; TCB_STATE_COUNT * TCB_STATE_COUNT];
        assert_eq!(coverage(&hits), (0, 48));
        assert_eq!(coverage_design(&hits), (0, 46));
    }

    /// Chỉ số trạng thái khớp `vifast_abi::TCB_STATES` (11 tên của spec, D2-33).
    #[test]
    fn chi_so_trang_thai_khop_tcb_states() {
        let all = [
            TcbState::Inactive,
            TcbState::Ready,
            TcbState::Running { hart: 0 },
            TcbState::BlockedOnSend {
                ep: 1,
                call: true,
                fault: None,
            },
            TcbState::BlockedOnRecv { ep: 1, reply: None },
            TcbState::BlockedOnReply { reply: 1 },
            TcbState::BlockedOnNotification { ntfn: 1 },
            TcbState::BlockedOnAddress {
                frame: 1,
                offset: 0,
            },
            TcbState::Sleeping {
                reason: SleepReason::Budget,
            },
            TcbState::Stopping,
            TcbState::Dead,
        ];
        for (i, st) in all.iter().enumerate() {
            assert_eq!(st.index(), i);
            assert_eq!(st.name(), vifast_abi::TCB_STATES[i]);
        }
    }
}
