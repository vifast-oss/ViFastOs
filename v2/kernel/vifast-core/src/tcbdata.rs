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
use crate::tcb::{SuspendFlag, TcbInfo, TcbState};

/// Ruột TCB — port `TcbData` của sim (D2-33/§4.4), bỏ các trường IPC của KT-24..28
/// (`last_ipc_error`/`recv_window`/`last_fault`/`notif` — KT-60).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TcbData {
    pub(crate) state: TcbState,
    /// **KT-329** — cờ treo **1 byte** (`None`/`Abort`/`Defer`); xem [`SuspendFlag`].
    pub(crate) suspend: SuspendFlag,
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
    /// **KT-309** — slot **đích** trong CSpace của TCB khi nó là **bên nhận** extra cap
    /// (`Recv` đọc word `dst` ở `IPC_BUFFER_EXTRA_CAPS_DST_OFFSET` của IPC buffer **của chính
    /// nó** — receiver đang chạy nên arch đọc được, D2-83). `None`/`Some(0)` = "slot trống đầu
    /// tiên" (quét từ ô 1). Model-only `[TK]`, không phải ABI.
    pub(crate) extra_caps_dst: Option<u16>,
    /// **KT-74** — con trỏ TLS của tiến trình (`Tcb.SetTls`, method **15** của `[object].Tcb`).
    /// Model-only `[TK]`: kernel **không** đọc vùng TLS, chỉ giữ giá trị và nạp vào `x4`/`tp`
    /// của U-mode mỗi lần resume (RISC-V: `tp` = `x4`). `0` = chưa đặt.
    pub(crate) tls: u64,
    /// **KT-311** — điểm vào (VA) của tiến trình (`Tcb.Configure` tham số `entry?`,
    /// `spec/abi.toml` `[method_signature."Tcb.Configure"]`). Model-only `[TK]`: kernel chỉ
    /// **giữ** giá trị (đã kiểm căn 4 B ở `Machine::tcb_configure_invocation`); đường `Resume`
    /// tiêu thụ giá trị này **chưa** cài (ghi nợ `JOURNAL/kt311-cfgfull.md` §7.2). `0` = chưa đặt.
    pub(crate) entry: u64,
    /// **KT-320** — **VSpace** của tiến trình (`Tcb.Configure` tham số `vspace?`,
    /// `spec/abi.toml` `[method_signature."Tcb.Configure"]`): `ObjId` của object **đại diện**
    /// trong core (D2-83: `Frame`/`PageTable`/`VSpace` thuộc object layer; core chỉ giữ **danh
    /// tính** + cap). `0` = **chưa cấu hình** — `ObjId` của core là **1-based**
    /// (`Machine::object_index` từ chối `0`), nên `0` không thể là object hợp lệ: cùng quy ước
    /// "0 = chưa đặt" của [`TcbData::entry`]/[`TcbData::tls`].
    ///
    /// **KT-325** — `0` cũng là giá trị **sau khi xoá**: `Tcb.Configure` với `vspace?` CPTR
    /// **rỗng** đưa trường về `0` = "dùng VSpace của ảnh đã nạp" (đường quay lui trước KT-325
    /// không có). Vì vậy `0` đọc là "không có VSpace riêng", bất kể chưa từng đặt hay đã xoá.
    ///
    /// Vì sao **không** `Option<ObjId>`: `Option<u32>` chiếm **8 B** (không niche) ⇒
    /// `+8 × MAX_TCBS(64) = 512 B` vào `size_of::<Machine>()`; `ObjId` (4 B) **vừa đúng** phần
    /// đệm sẵn có của `TcbData` ⇒ `TcbData` **824 B** và `size_of::<Machine>()` **184.792 B**
    /// **không đổi** (đo ở `footprint::tests`, pin D2-84 **196.608** và trần KT-312 **184.832**
    /// đều **không** bị chạm). Ngữ nghĩa `None`/`0` do [`Machine::tcb_vspace`] quy định.
    pub(crate) vspace: ObjId,
}

impl TcbData {
    pub(crate) const EMPTY: Self = Self {
        state: TcbState::Inactive,
        suspend: SuspendFlag::None,
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
        extra_caps_dst: None,
        tls: 0,
        entry: 0,
        vspace: 0,
    };

    pub(crate) fn snapshot(&self) -> TcbInfo {
        TcbInfo {
            state: self.state,
            suspended: self.suspend.is_suspended(),
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
