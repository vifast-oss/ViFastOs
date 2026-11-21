// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ruột TCB (port `TcbData`) và ruột SchedContext (`SchedData`) — KT-59, tách khỏi
//! `machine.rs` để giữ giới hạn dòng mỗi tệp (QD §3.15); KT-60 thêm vùng nhận IPC + trường
//! `last_ipc_error`/`last_fault`/`notif`.
//!
//! Model-only `[TK]`, **không** phải ABI: `Tcb.ReadState` chỉ trả 4 giá trị; các trường còn
//! lại là model API của sim (oracle) và được parity so từng bước.

use crate::error::{CapId, KError, ObjId, SlotRef};
use crate::ipc::{IpcArea, IpcBufferBinding};
use crate::mcs::ScBudget;
use crate::tcb::{TcbInfo, TcbState};

/// Ruột TCB — port `TcbData` của sim (D2-33/§4.4), bỏ các trường IPC của KT-24..28
/// (`last_ipc_error`/`recv_window`/`last_fault`/`notif` — KT-60).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TcbData {
    pub(crate) state: TcbState,
    pub(crate) suspended: bool,
    pub(crate) exit_code: Option<u64>,
    /// Cap `fault_ep` (`Tcb.SetFaultEp`).
    pub(crate) fault_ep: Option<CapId>,
    /// CSpace của TCB (object CNode).
    pub(crate) cspace: Option<ObjId>,
    pub(crate) priority: u8,
    pub(crate) max_controlled_priority: u8,
    /// HartMask ≤ 64 bit.
    pub(crate) affinity: u64,
    pub(crate) home: u16,
    /// SchedContext riêng (`set_sched_params`).
    pub(crate) sched_context: Option<ObjId>,
    /// SC đang nhận donation (KT-21 — chưa có đường tạo ở KT-59, luôn `None`).
    pub(crate) donated_sc: Option<ObjId>,
    /// Notification đang bind (KT-27).
    pub(crate) bound_notification: Option<ObjId>,
    /// Vùng nhận IPC: 4 MR thanh ghi + 60 word buffer + message vừa nhận (KT-60).
    pub(crate) ipc: IpcArea,
    /// **KT-63** — **receive window** (CPTR) của TCB: 4 ô CSpace nhận cap của message vào.
    /// `None` = **chưa bind** (message có cap cần cài ⇒ `InvalidArgument`); `Some([s0..s3])`
    /// = đã bind (ô nào đang có cap thì cap tương ứng **thất bại mềm**, D2-53 Q6). Model-only
    /// `[TK]`, như `TcbData.recv_window` của sim.
    pub(crate) recv_window: Option<[SlotRef; crate::ipc::MAX_CAPS]>,
    /// Lỗi IPC kernel gán ở lần giao gần nhất (`Cancelled` — D2-53 Q7(b), KT-60).
    pub(crate) last_ipc_error: Option<KError>,
    /// `(sender, FaultKind)` của **fault vừa giao** cho TCB làm bên nhận (KT-26, KT-60).
    pub(crate) last_fault: Option<(ObjId, crate::tcb::FaultKind)>,
    /// `(word, badge)` của lần `Wait`/`Poll` Notification gần nhất (KT-27, KT-60).
    pub(crate) notif: Option<(u64, u64)>,
    /// **KT-282** — **IPC buffer** đã bind (`Tcb.Configure` tham số `ipc_buffer?`,
    /// `spec/abi.toml` `[method_signature."Tcb.Configure"]`): `Frame` 4 KiB + VA. `None` =
    /// chưa bind ⇒ message có `length > 4` bị `InvalidArgument` (không đoán buffer).
    pub(crate) ipc_buffer: Option<IpcBufferBinding>,
    /// **KT-74** — con trỏ TLS của tiến trình (`Tcb.SetTls`, method **15** của `[object].Tcb`).
    /// Model-only `[TK]`: kernel **không** đọc vùng TLS, chỉ giữ giá trị và nạp vào `x4`/`tp`
    /// của U-mode mỗi lần resume (RISC-V: `tp` = `x4`). `0` = chưa đặt.
    pub(crate) tls: u64,
}

impl TcbData {
    pub(crate) const EMPTY: Self = Self {
        state: TcbState::Inactive,
        suspended: false,
        exit_code: None,
        fault_ep: None,
        cspace: None,
        priority: 0,
        max_controlled_priority: 0,
        affinity: 1,
        home: 0,
        sched_context: None,
        donated_sc: None,
        bound_notification: None,
        ipc: IpcArea::new(),
        recv_window: None,
        last_ipc_error: None,
        last_fault: None,
        notif: None,
        ipc_buffer: None,
        tls: 0,
    };

    pub(crate) fn snapshot(&self) -> TcbInfo {
        TcbInfo {
            state: self.state,
            suspended: self.suspended,
            exit_code: self.exit_code,
            fault_ep: self.fault_ep,
            cspace: self.cspace,
            priority: self.priority,
            max_controlled_priority: self.max_controlled_priority,
            affinity: self.affinity,
            home: self.home,
            sched_context: self.sched_context,
            donated_sc: self.donated_sc,
            bound_notification: self.bound_notification,
            tls: self.tls,
        }
    }
}

/// Ruột `SchedContext` — port `SchedData` của sim.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScData {
    pub(crate) budget: ScBudget,
    /// TCB đang nhận donation (KT-21 — chưa tạo ở KT-59).
    pub(crate) donated_to: Option<ObjId>,
    /// TCB đang cho mượn SC (KT-21 — chưa tạo ở KT-59).
    pub(crate) donor: Option<ObjId>,
    /// **KT-64** — cap **Reply/Notification** đang bind `SchedContext.Bind(sc, target)`
    /// (1:1 với `NotificationData.bound_sc`; `None` = chưa bind). Trước KT-64 trường này
    /// **không có** ⇒ `bound_sc` là trường chết (§8.6.0).
    pub(crate) bound: Option<CapId>,
}

impl ScData {
    pub(crate) const EMPTY: Self = Self {
        budget: ScBudget::new(),
        donated_to: None,
        donor: None,
        bound: None,
    };
}
