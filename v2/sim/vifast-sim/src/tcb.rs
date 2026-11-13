// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-18 — trạng thái TCB + **bảng chuyển trạng thái `ALLOWED`** 11×11 (D2-49(d)).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Bảng được xuất ra
//! [`v2/docs/k1b-tcb-transition.md`](../../../docs/k1b-tcb-transition.md) để Agent 0 review;
//! `v2/sim/**` không có đường đổi trạng thái TCB nào khác ngoài `Machine::transition_tcb`
//! (một cửa) — `call`/`recv`/`reply`/`exit`/`suspend`/`resume`/`finalise` đều gọi nó.
//!
//! Nguồn ngữ nghĩa: thiết kế §4.4 (`docs/v2/ViFastOs-v2-thiet-ke.md:256-286`), §4.5 bảng
//! ngân sách (`:302-311`), **D2-33** (enum đầy đủ), **D2-25** (`Exit`), **D2-49(d)** (vị trí
//! bảng). Mỗi hàng của bảng có lý do ghi kèm; cặp **không** suy ra được từ thiết kế thì cấm
//! (mặc định an toàn), không "mở cho tiện".

use crate::kobj::ObjId;

/// Số trạng thái của `TcbState` — khớp `vifast_abi::TCB_STATES` (D2-33).
pub const TCB_STATE_COUNT: usize = 11;

/// Nguyên nhân fault mang theo trạng thái chặn (D2-25: Exit mang mã thoát).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    Exit { code: u64 },
}

/// Lý do `Sleeping` — variants của `spec/[method_signature.state.Sleeping]` (D2-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepReason {
    Timeout,
    Budget,
}

impl SleepReason {
    pub fn name(self) -> &'static str {
        match self {
            SleepReason::Timeout => "Timeout",
            SleepReason::Budget => "Budget",
        }
    }
}

/// Trạng thái TCB — **đủ 11 tên** của `spec/[method_signature.state]` (D2-33);
/// unit test `tcb_state_du_11_ten_theo_spec` so trực tiếp với `vifast_abi::TCB_STATES`.
///
/// Thứ tự khai báo **chính là** thứ tự chỉ số của bảng [`ALLOWED`]; đổi thứ tự là đổi bảng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcbState {
    Inactive,
    Ready,
    Running {
        hart: u16,
    },
    BlockedOnSend {
        ep: ObjId,
        call: bool,
        fault: Option<FaultKind>,
    },
    BlockedOnRecv {
        ep: ObjId,
        reply: Option<ObjId>,
    },
    BlockedOnReply {
        reply: ObjId,
    },
    BlockedOnNotification {
        ntfn: ObjId,
    },
    BlockedOnAddress {
        frame: ObjId,
        offset: u32,
    },
    Sleeping {
        reason: SleepReason,
    },
    Stopping,
    Dead,
}

/// Loại hàng chờ của `Tcb.ReadState.blocked_on_kind` (D2-33/thiết kế §4.4 `:281`).
///
/// Giá trị `[TK]` cho mô hình K1b: một trạng thái chặn ứng với đúng một loại hàng chờ;
/// trạng thái không chặn ⇒ `None`. ABI chưa chốt tên chuỗi của trường này (draft
/// `v2/docs/abi-call-signatures.toml` chỉ ghi `returns = ["state", "suspended",
/// "blocked_on_kind", "exit_code"]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedOnKind {
    Send,
    Recv,
    Reply,
    Notification,
    Address,
}

impl BlockedOnKind {
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
    Abort,
    Defer,
}

impl SuspendMode {
    pub fn name(self) -> &'static str {
        match self {
            SuspendMode::Abort => "Abort",
            SuspendMode::Defer => "Defer",
        }
    }
}

/// Bốn giá trị trả của `Tcb.ReadState` (thiết kế §4.4 `:282`, D2-33).
///
/// KT-18 sửa drift: trước đây `tcb_read_state` trả `TcbState` (một giá trị) — nay trả
/// `TcbSnapshot` gồm `state`, `suspended`, `blocked_on_kind`, `exit_code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcbSnapshot {
    pub state: TcbState,
    pub suspended: bool,
    pub blocked_on_kind: Option<BlockedOnKind>,
    pub exit_code: Option<u64>,
}

impl TcbState {
    /// Chỉ số của trạng thái trong [`ALLOWED`] — khớp thứ tự `vifast_abi::TCB_STATES`.
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

    /// Chỉ `Inactive`/`Ready`/`Running` là chạy được; còn lại là chặn/ngủ/dừng/chết.
    pub fn is_runnable(&self) -> bool {
        matches!(
            self,
            TcbState::Inactive | TcbState::Ready | TcbState::Running { .. }
        )
    }

    /// Đang nằm trong một hàng chờ (`BlockedOn*`) — phải gỡ khỏi hàng trước `Abort`/`Exit`/
    /// `finalise` (thiết kế §4.4 invariant).
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

    /// Trạng thái "sống" mà `Exit`/`finalise` được phép rời khỏi (mọi trạng thái trừ
    /// `Stopping`/`Dead`).
    pub fn is_live(&self) -> bool {
        !matches!(self, TcbState::Stopping | TcbState::Dead)
    }

    /// Loại hàng chờ tương ứng — `Tcb.ReadState.blocked_on_kind`.
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

    pub fn render(&self) -> String {
        match self {
            TcbState::Inactive => "Inactive".to_string(),
            TcbState::Ready => "Ready".to_string(),
            TcbState::Running { hart } => format!("Running{{hart={hart}}}"),
            TcbState::BlockedOnSend { ep, call, fault } => {
                let fault = match fault {
                    None => "none".to_string(),
                    Some(FaultKind::Exit { code }) => format!("Exit{{{code}}}"),
                };
                format!("BlockedOnSend{{ep={ep}, call={call}, fault={fault}}}")
            }
            TcbState::BlockedOnRecv { ep, reply } => match reply {
                None => format!("BlockedOnRecv{{ep={ep}, reply=none}}"),
                Some(reply) => format!("BlockedOnRecv{{ep={ep}, reply={reply}}}"),
            },
            TcbState::BlockedOnReply { reply } => format!("BlockedOnReply{{reply={reply}}}"),
            TcbState::BlockedOnNotification { ntfn } => {
                format!("BlockedOnNotification{{ntfn={ntfn}}}")
            }
            TcbState::BlockedOnAddress { frame, offset } => {
                format!("BlockedOnAddress{{frame={frame}, offset={offset}}}")
            }
            TcbState::Sleeping { reason } => format!("Sleeping{{reason={}}}", reason.name()),
            TcbState::Stopping => "Stopping".to_string(),
            TcbState::Dead => "Dead".to_string(),
        }
    }
}

/// Bảng chuyển trạng thái TCB **11×11** (D2-49(d), model-only `[TK]`).
///
/// Hàng = trạng thái nguồn, cột = trạng thái đích, thứ tự hai chiều là thứ tự khai báo
/// [`TcbState`] (khớp `vifast_abi::TCB_STATES`):
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
/// (`x` = ALLOWED, `.` = cặp cấm ⇒ `IllegalOperation`, **không** đổi trạng thái).
///
/// Lý do từng hàng (nguồn ghi kèm; chi tiết ở `v2/docs/k1b-tcb-transition.md`):
///
/// - **Inactive**: `→Ready` kích hoạt (mô hình K1b chưa có method `Activate`); `→BlockedOnSend`
///   là đường `Exit` có `fault_ep` từ TCB chưa chạy (D2-25, `.scn` `exit`); `→Dead` finalise.
///   `Inactive → Running` **cấm** (phải qua Ready) — ca âm chuẩn của KT-18.
/// - **Ready**: `→Running` khi `pick` (thiết kế §4.8, KT-19); `→BlockedOn*` năm loại (syscall
///   chặn từ hàng đợi chạy); `→Inactive` `Exit` không `fault_ep`; `→Stopping`/`→Dead` finalise.
/// - **Running**: `→Ready` preempt (§4.8/§4.13); `→BlockedOn*`; `→Sleeping` hết budget
///   (bảng §4.5 `:302-311`); `→Inactive` `Exit`; `→Stopping`/`→Dead` finalise.
/// - **BlockedOnSend**: `→BlockedOnReply` người nhận `Recv` kèm Reply cap; `→Ready` gửi xong
///   (không Reply) **hoặc** `Suspend(Abort)`; `→Inactive` fault `Exit` được procsrv `Recv`;
///   `→BlockedOnSend` (cặp **danh tính**) `Exit` khi đang chờ gửi: message cũ bị huỷ, fault
///   chuyển sang hàng của `fault_ep` (D2-25); `→Dead` finalise.
/// - **BlockedOnRecv/Reply/Notification/Address**: `→Ready` hoàn tất chờ **hoặc**
///   `Suspend(Abort)` (thiết kế §4.4 `:278-281`); `→BlockedOnSend` `Exit` chuyển sang hàng
///   `fault_ep`; `→Inactive` `Exit` không `fault_ep`; `→Dead` finalise.
/// - **Sleeping**: `→Ready` hết timeout/được nạp refill (bảng §4.5); `→BlockedOnSend`
///   `Exit`; `→Inactive` `Exit` không `fault_ep`; `→Dead` finalise. `Suspend(Abort)` trên
///   `Sleeping` **giữ** `Sleeping` + bật cờ (bảng §4.5, không phải chuyển trạng thái).
/// - **Stopping**: chỉ `→Dead`. K1b **1 hart** (D2-49(a)) không có nguồn nào nhập `Stopping`;
///   cặp `Ready/Running → Stopping` có trong bảng để hàng/cột phủ đủ khi KT-19/K1c dùng.
/// - **Dead**: không ra được (trạng thái cuối).
///
/// Không có cặp danh tính nào khác ngoài `BlockedOnSend → BlockedOnSend`: thao tác **không**
/// đổi trạng thái (ví dụ `Resume`, `Suspend` trên `Ready`/`Sleeping`) **không** gọi cổng.
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

/// Cặp `(from, to)` có được phép chuyển không (tra [`ALLOWED`]).
pub fn allowed(from: TcbState, to: TcbState) -> bool {
    ALLOWED[from.index()][to.index()]
}

/// Số cặp ALLOWED của bảng (dùng cho `Machine::transition_coverage`).
pub fn allowed_count() -> usize {
    ALLOWED.iter().flatten().filter(|ok| **ok).count()
}

/// Cặp **`Stopping` reserved cho KT-19/K1c**: `Ready → Stopping`, `Running → Stopping`
/// (bảng vẫn ALLOWED để hàng/cột đủ khi K1c có nguồn nhập `Stopping`, nhưng **K1b 1 hart**
/// chưa có thao tác nào đi qua hai cặp này). 46 cặp **suy ra từ thiết kế** = 48 − 2.
pub fn is_reserved(from: TcbState, to: TcbState) -> bool {
    matches!(from, TcbState::Ready | TcbState::Running { .. }) && matches!(to, TcbState::Stopping)
}

/// Số cặp reserved (phải là **2** — KT-23 kiểm trong unit test).
pub fn reserved_count() -> usize {
    ALLOWED
        .iter()
        .enumerate()
        .flat_map(|(from, row)| row.iter().enumerate().map(move |(to, ok)| (*ok, from, to)))
        .filter(|(ok, from, to)| *ok && is_reserved(state_at(*from), state_at(*to)))
        .count()
}

/// Số cặp **cấm** của bảng (`121 − 48 = 73`) — mọi cặp này phải bị thử ít nhất một lần ở
/// KT-23 DoD 3 và trả `IllegalOperation`.
pub fn forbidden_count() -> usize {
    ALLOWED.iter().flatten().filter(|ok| !**ok).count()
}

/// `(số cặp ALLOWED **suy ra từ thiết kế** đã có hit, 46)` — **không** tính 2 cặp `Stopping`
/// reserved; là "hit/allowed" mà KT-23 dùng cho DoD 3 (`46/46`, 100%).
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

/// `TcbState` chỉ để tra bảng theo chỉ số (trường payload không mang nghĩa khi tra).
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

/// `(số cặp ALLOWED đã có hit, tổng số cặp ALLOWED)` từ bộ đếm `pair_hits` 121 ô.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Bảng phải hình vuông 11×11 và **không** có cặp danh tính nào ngoài `BlockedOnSend`.
    #[test]
    fn bang_allowed_khong_co_cap_danh_tinh_ngoai_blocked_on_send() {
        assert_eq!(ALLOWED.len(), TCB_STATE_COUNT);
        for (i, row) in ALLOWED.iter().enumerate() {
            assert_eq!(row.len(), TCB_STATE_COUNT);
            if i != 3 {
                // 3 = BlockedOnSend (index của nó trong TCB_STATES).
                assert!(!row[i], "hàng {i}: cặp danh tính phải cấm");
            }
        }
        assert!(
            ALLOWED[3][3],
            "Exit khi đang chờ gửi dùng cặp danh tính (3,3)"
        );
    }

    /// **KT-23** — 46 cặp thiết kế + 2 reserved + 73 cặp cấm; `coverage_design` bỏ **đúng**
    /// 2 cặp `Stopping` reserved nên không bao giờ báo 48/46 hay 48/48 lẫn nhau.
    #[test]
    fn kt23_coverage_design_46_va_73_cap_cam() {
        assert_eq!(allowed_count(), 48);
        assert_eq!(reserved_count(), 2);
        assert_eq!(forbidden_count(), 73);
        let mut hits = [0u32; TCB_STATE_COUNT * TCB_STATE_COUNT];
        assert_eq!(coverage(&hits), (0, 48));
        assert_eq!(coverage_design(&hits), (0, 46));
        // Cặp **reserved** có hit: coverage bảng tăng, coverage thiết kế **không** đổi.
        hits[TcbState::Ready.index() * TCB_STATE_COUNT + TcbState::Stopping.index()] = 1;
        assert_eq!(coverage(&hits), (1, 48));
        assert_eq!(coverage_design(&hits), (0, 46));
        // Cặp **thiết kế** có hit: cả hai cùng tăng.
        hits[TcbState::Ready.index() * TCB_STATE_COUNT + TcbState::Running { hart: 0 }.index()] = 1;
        assert_eq!(coverage(&hits), (2, 48));
        assert_eq!(coverage_design(&hits), (1, 46));
        assert!(is_reserved(
            TcbState::Running { hart: 0 },
            TcbState::Stopping
        ));
        assert!(!is_reserved(TcbState::Stopping, TcbState::Dead));
    }

    /// Bảng phủ đủ 11 tên: mỗi trạng thái có chỉ số duy nhất khớp `TCB_STATES`.
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
        assert_eq!(allowed_count(), 48);
        let mut hits = [0u32; TCB_STATE_COUNT * TCB_STATE_COUNT];
        assert_eq!(coverage(&hits), (0, 48));
        hits[0] = 1; // (Inactive, Inactive) CẤM ⇒ không tính
        hits[1] = 1; // (Inactive, Ready) ALLOWED
        assert_eq!(coverage(&hits), (1, 48));
    }
}
