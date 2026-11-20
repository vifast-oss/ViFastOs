// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-241** — cửa ghi trạng thái TCB (D2-187/D2-194): primitive thô **private** + năm cửa
//! ngữ nghĩa đưa TCB về `Ready`.
//!
//! Đây là **module con** của [`crate::kobj`] nên thấy được item private của `kobj`
//! (`pair_hits`, `forbidden_hits`, `budget_sleeps`, `sched`; `sched_sync`,
//! `unlink_tcb_queues`, `release_donation_of`, `reply_set_caller`, `is_donation_donor`,
//! `tcb_info_obj`, `body_mut`) — **không** phải nới `pub(crate)` cho bất kỳ field/method nào
//! của [`Machine`].
//!
//! Ranh giới (D2-187 §3): [`NonReadyTarget`] **không** biểu diễn được `Ready`; `Ready` chỉ vào
//! được qua **năm cửa** có kiểm điều kiện nguồn của mình. [`Machine::write_state`] là `fn`
//! **private** — không `pub`, không `pub(crate)`, không `From`/`TryFrom` công khai nào từ
//! [`TcbState`] sang [`NonReadyTarget`].

use super::{FaultKind, KError, Machine, ObjId, SleepReason, TcbState};
use super::{Kind, ObjType, TCB_STATE_COUNT};

/// Đích chuyển trạng thái TCB **loại trừ `Ready`** — 10 biến thể còn lại của [`TcbState`].
///
/// **D2-187/D2-194**: cửa chuyển trạng thái thông thường **không nhận kiểu biểu diễn được
/// `Ready`**. `Ready` chỉ vào được qua **năm cửa ngữ nghĩa** trong `state_write`, mỗi cửa
/// kiểm điều kiện nguồn của mình. Không có `From`/`TryFrom` công khai nào từ [`TcbState`]
/// sang kiểu này — đó là toàn bộ ranh giới.
///
/// **Không** `#[non_exhaustive]`: crate ngoài (`ktest_*`, `core_bridge`, `proc_rt`, test sim)
/// phải dựng được mọi biến thể.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonReadyTarget {
    /// Chưa kích hoạt.
    Inactive,
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

impl NonReadyTarget {
    /// **private** — chỉ module `state_write` đổi được sang [`TcbState`].
    ///
    /// **Không** `pub`, **không** `pub(crate)`, **không** `From`/`TryFrom`, **không** method
    /// công khai nào khác trả `TcbState`. Đây là ranh giới (D2-187 §3): `pub(crate)` là
    /// **không đủ** — mọi module trong crate sẽ gọi được, tức đường lọt cũ đổi tên.
    fn into_state(self) -> TcbState {
        match self {
            NonReadyTarget::Inactive => TcbState::Inactive,
            NonReadyTarget::Running { hart } => TcbState::Running { hart },
            NonReadyTarget::BlockedOnSend { ep, call, fault } => {
                TcbState::BlockedOnSend { ep, call, fault }
            }
            NonReadyTarget::BlockedOnRecv { ep, reply } => TcbState::BlockedOnRecv { ep, reply },
            NonReadyTarget::BlockedOnReply { reply } => TcbState::BlockedOnReply { reply },
            NonReadyTarget::BlockedOnNotification { ntfn } => {
                TcbState::BlockedOnNotification { ntfn }
            }
            NonReadyTarget::BlockedOnAddress { frame, offset } => {
                TcbState::BlockedOnAddress { frame, offset }
            }
            NonReadyTarget::Sleeping { reason } => TcbState::Sleeping { reason },
            NonReadyTarget::Stopping => TcbState::Stopping,
            NonReadyTarget::Dead => TcbState::Dead,
        }
    }

    /// **`pub(crate)`** — **vị từ**, KHÔNG phải phép chuyển đổi: chỉ trả lời "đích này có phải
    /// đích **block** hợp lệ không", **không** sinh ra [`TcbState`].
    ///
    /// **D2-194 §1(d)**: "không phải `Ready`" **chưa** đồng nghĩa "được phép block vào" —
    /// `block_current` phải giữ kiểm tra tập trạng thái block riêng.
    pub(crate) fn is_block_target(self) -> bool {
        matches!(
            self,
            NonReadyTarget::BlockedOnSend { .. }
                | NonReadyTarget::BlockedOnRecv { .. }
                | NonReadyTarget::BlockedOnReply { .. }
                | NonReadyTarget::BlockedOnNotification { .. }
                | NonReadyTarget::BlockedOnAddress { .. }
                | NonReadyTarget::Sleeping { .. }
        )
    }
}

impl Machine {
    // ------------------------------------------- nguyên thuỷ thô (**private**)

    /// **Một cửa** đổi trạng thái TCB (D2-49(d)): tra bảng [`crate::tcb::ALLOWED`]; cặp cấm trả
    /// `IllegalOperation` và **không** đổi trạng thái; mỗi lần chuyển hợp lệ cộng `pair_hits` để
    /// `transition_coverage()` báo phủ cặp.
    ///
    /// **KT-241** — đây là **nguyên thuỷ thô**, `fn` **private** của module này: chỉ năm cửa
    /// `Ready` bên dưới và [`Machine::transition_tcb`] gọi được. Thân là nguyên văn
    /// `transition_tcb` cũ (KT-18..KT-34), **chỉ** đổi lời gọi đệ quy cũ
    /// `transition_tcb(cur, Ready)` ⇒ [`Machine::preempt_current`] (cửa preemption có kiểm
    /// `Running{hart}` + `current`).
    ///
    /// Thứ tự hook **giữ nguyên** (parity core/sim): trả donation (KT-21) → `Reply.caller`
    /// (KT-34) → preempt khi vào `Running` → ghi `state` → `pair_hits` → `budget_sleeps` →
    /// `sched_sync`.
    fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        let from = self.tcb_state(tcb)?;
        if !crate::tcb::allowed(from, to) {
            // KT-23: đếm **cặp cấm bị thử** (DoD 3) — không đổi trạng thái, không đếm hit.
            self.forbidden_hits[from.index() * TCB_STATE_COUNT + to.index()] += 1;
            return Err(KError::IllegalOperation);
        }
        // KT-21: rời `BlockedOnReply` bằng đường **khác** `Reply` (wake/suspend/exit/finalise
        // hoặc `transition_tcb` trực tiếp) = bỏ cuộc gọi ⇒ SC donation phải trả về client ngay,
        // không để SC treo trên TCB không còn chờ reply (bất biến "donor đang BlockedOnReply").
        if matches!(from, TcbState::BlockedOnReply { .. })
            && !matches!(to, TcbState::BlockedOnReply { .. })
            && self.is_donation_donor(tcb)
        {
            self.release_donation_of(tcb)?;
        }
        // **KT-34** (C2, D2-63) — giữ liên kết `Reply ⇔ BlockedOnReply` ở **chính** cổng trạng
        // thái (một cửa, cùng chỗ hook KT-21 donation ở trên):
        // - vào `BlockedOnReply{reply}` ⇒ `Reply.caller = Some(tcb)` (Reply **đã** bind người
        //   khác ⇒ `IllegalOperation`, **không** ghi đè — chặn lỗi C2 ngay cả ở cổng thấp);
        // - rời `BlockedOnReply` ⇒ gỡ `caller` nếu đang là `tcb` (giống `wake`/`finalise` đã làm
        //   qua `unlink_tcb_queues`), để **không** còn `caller = Some(t)` mà `t` không chờ.
        // Object **không** phải `Reply` sống thì bỏ qua: ma trận 11×11 (KT-23) dựng trạng thái
        // bằng id tượng trưng — cùng quy ước chiều ngược của `check_notifications` (KT-27).
        if let TcbState::BlockedOnReply { reply } = to {
            if self.object_alive(reply) && self.object_type(reply) == Some(ObjType::Reply) {
                match self.reply_caller(reply)? {
                    None => self.reply_set_caller(reply, Some(tcb))?,
                    Some(c) if c == tcb => {}
                    Some(_) => return Err(KError::IllegalOperation),
                }
            }
        } else if let TcbState::BlockedOnReply { reply } = from {
            if self.object_alive(reply)
                && self.object_type(reply) == Some(ObjType::Reply)
                && self.reply_caller(reply)? == Some(tcb)
            {
                self.reply_set_caller(reply, None)?;
            }
        }
        // Bất biến ≤1 `Running`/hart (KT-19 DoD 2): TCB vào `Running{hart}` khi hart đang có
        // TCB chạy ⇒ đưa TCB cũ về `Ready` **trước**, qua đúng cửa này (không đường tắt).
        if let TcbState::Running { hart } = to {
            if hart as usize >= self.sched.len() {
                return Err(KError::InvalidArgument);
            }
            // **KT-29**: `Running{hart}` chỉ hợp lệ khi `home == hart` (bất biến `check_smp`:
            // home khớp hart đang giữ `current`); TCB muốn đổi hart phải qua `Ready`/dispatch.
            if self.tcb_info_obj(tcb)?.home != hart {
                return Err(KError::IllegalOperation);
            }
            if let Some(cur) = self.sched[hart as usize].current() {
                if cur != tcb {
                    self.preempt_current(cur)?;
                }
            }
        }
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.state = to;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.pair_hits[from.index() * TCB_STATE_COUNT + to.index()] += 1;
        // KT-23: đếm hoạt động ngân sách ở **một cửa** (DoD 2/8): `Running` hết budget ⇒
        // `Sleeping{Budget}` (bảng §4.5 `:302-311`; refill đếm ở `process_refills`).
        if matches!(from, TcbState::Running { .. })
            && matches!(
                to,
                TcbState::Sleeping {
                    reason: SleepReason::Budget
                }
            )
        {
            self.budget_sleeps += 1;
        }
        // KT-19: state đổi ⇒ ready queue/`current` đổi theo (một cửa duy nhất).
        self.sched_sync(tcb)?;
        Ok(())
    }

    // ------------------------------------------- cửa 0: thông thường (loại trừ `Ready`)

    /// **Cửa thông thường duy nhất** đổi trạng thái TCB sang một đích **khác `Ready`**
    /// (D2-187/D2-194). Tra bảng [`crate::tcb::ALLOWED`]; cặp cấm trả `IllegalOperation` và
    /// **không** đổi trạng thái.
    ///
    /// Mọi đường `call`/`recv`/`reply`/`exit`/`tcb_suspend`/`finalise_tcb` dùng hàm này.
    /// Thao tác **không** đổi trạng thái (ví dụ `Resume`, `Suspend` trên `Ready`/`Sleeping`)
    /// **không** gọi cổng — bảng không có cặp danh tính ngoài `BlockedOnSend → BlockedOnSend`.
    ///
    /// `Ready` **không** biểu diễn được ở kiểu đích ⇒ đi qua năm cửa ngữ nghĩa:
    /// [`Machine::start_tcb`], [`Machine::tcb_yield`], [`Machine::wake_step`],
    /// [`Machine::preempt_current`], [`Machine::suspend_to_ready`].
    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError> {
        self.write_state(tcb, to.into_state())
    }

    // ------------------------------------------- cửa 1: wake

    /// Thân chuyển `blocked/sleeping → Ready` — **một** chỗ duy nhất (máy kiểm
    /// `v2/tools/check-wake-paths.py --strict` đòi điều này). `unlink` quyết định có gỡ TCB
    /// khỏi **mọi** hàng chờ trước khi `Ready` hay không: đường hộp thư/`wake`/`wake_addr`
    /// cần (`true`); đường sản phẩm đã tự gỡ nên dùng [`Machine::wake_from`] (`false`).
    ///
    /// **KT-241** — cửa `Ready` #1: nguồn phải `BlockedOn*`/`Sleeping`; `Ready` ⇒ `Ok`
    /// (idempotent, giữ nguyên); còn lại ⇒ `IllegalOperation`. `pub(super)`: chỉ `kobj` gọi.
    pub(super) fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Ready {
            return Ok(());
        }
        if !(state.is_blocked() || matches!(state, TcbState::Sleeping { .. })) {
            return Err(KError::IllegalOperation);
        }
        if unlink {
            self.unlink_tcb_queues(tcb);
        }
        self.write_state(tcb, TcbState::Ready)
    }

    // ------------------------------------------- cửa 2 + 3a: preemption / yield

    /// Điều kiện nguồn **chung** của cửa preemption (2) và yield (3a): TCB phải đang
    /// `Running { hart }` **và** là `current` của hart đó; sai ⇒ `IllegalOperation`.
    fn require_running_current(&self, tcb: ObjId) -> Result<(), KError> {
        let TcbState::Running { hart } = self.tcb_state(tcb)? else {
            return Err(KError::IllegalOperation);
        };
        if self.sched.get(hart as usize).and_then(|h| h.current()) != Some(tcb) {
            return Err(KError::IllegalOperation);
        }
        Ok(())
    }

    /// **Cửa `Ready` #2 — preemption**: TCB đang chạy của hart bị đưa về `Ready` khi hart nhận
    /// TCB khác (`dispatch_step`, nhánh `Running` của [`Machine::write_state`]).
    ///
    /// Nguồn phải `Running { hart }` **và** `sched[hart].current() == Some(tcb)`; sai ⇒
    /// `IllegalOperation`. `pub(super)`: chỉ `kobj` gọi.
    pub(super) fn preempt_current(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.require_running_current(tcb)?;
        self.write_state(tcb, TcbState::Ready)
    }

    /// **Cửa `Ready` #3a — yield (nội bộ)**: TCB đang chạy tự nhường CPU (`yield_current`,
    /// `reply_with`). Điều kiện nguồn **như** [`Machine::preempt_current`].
    ///
    /// `pub(super)`: chỉ `kobj` gọi; crate ngoài dùng [`Machine::tcb_yield`].
    pub(super) fn yield_to_ready(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.require_running_current(tcb)?;
        self.write_state(tcb, TcbState::Ready)
    }

    /// **Cửa `Ready` #3b — yield (crate ngoài)**: `pub` cho `proc_rt`/test; điều kiện nguồn do
    /// [`Machine::yield_to_ready`] kiểm (`Running { hart }` + `current`).
    pub fn tcb_yield(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.yield_to_ready(tcb)
    }

    // ------------------------------------------- cửa 4: khởi tạo/start

    /// **Cửa `Ready` #4 — khởi tạo**: TCB `Inactive` được kích hoạt (`ensure_started`).
    ///
    /// Nguồn phải `Inactive`; sai ⇒ `IllegalOperation`. `pub` cho crate ngoài (`core_bridge`,
    /// `proc_rt`, test sim).
    pub fn start_tcb(&mut self, tcb: ObjId) -> Result<(), KError> {
        if self.tcb_state(tcb)? != TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        self.write_state(tcb, TcbState::Ready)
    }

    // ------------------------------------------- cửa 5: suspend/abort

    /// **Cửa `Ready` #5 — suspend/abort**: `tcb_suspend(Abort)` đưa TCB `BlockedOn*`/`Running`
    /// về `Ready` (kèm cờ `suspended`, TCB **không** chạy được).
    ///
    /// **Đọc lại** trạng thái: phải `is_blocked()` hoặc `Running { .. }`; sai ⇒
    /// `IllegalOperation`. `pub(super)`: chỉ `kobj` gọi.
    pub(super) fn suspend_to_ready(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if !(state.is_blocked() || matches!(state, TcbState::Running { .. })) {
            return Err(KError::IllegalOperation);
        }
        self.write_state(tcb, TcbState::Ready)
    }
}
