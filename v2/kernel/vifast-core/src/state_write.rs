// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Ranh giới `Ready`** của core (D2-187/D2-194, KT-241): cửa chuyển trạng thái thông thường
//! **không nhận kiểu biểu diễn được `Ready`**; `Ready` chỉ vào được qua **năm cửa ngữ nghĩa**
//! trong tệp này, mỗi cửa kiểm điều kiện nguồn của mình.
//!
//! # Vì sao là module riêng, không phải `pub(crate)`
//!
//! Nguyên thuỷ thô [`Machine::write_state`] là **private** của module này. Nếu nó là
//! `pub(crate)`, **mọi** module trong crate (`core_ipc`, `core_sync`, `core_finalise`, …) gọi
//! được `write_state(t, TcbState::Ready)` — đúng đường lọt cũ, chỉ đổi tên. Private trong
//! `state_write` thì **chỉ** thân module này gọi được; năm cửa ngữ nghĩa là **bề mặt duy nhất**
//! còn lại, và mỗi cửa có tên + điều kiện nguồn.
//!
//! # Bảng cửa
//!
//! | # | Cửa | Mức | Điều kiện **nguồn** (fail-closed) |
//! |---|---|---|---|
//! | 0 | [`Machine::transition_tcb`] | `pub` | bảng `ALLOWED` (trong `write_state`) |
//! | — | `write_state` | **private** | bảng `ALLOWED` + hook KT-21/KT-34 + bất biến `Running` |
//! | 1 | [`Machine::wake_step`] | `pub(crate)` | `Ready` ⇒ `Ok`; `is_blocked()`/`Sleeping` ⇒ đi |
//! | 2 | [`Machine::preempt_current`] | `pub(crate)` | `Running{hart}` **và** `sched[hart].current() == Some(tcb)` |
//! | 3a | [`Machine::yield_to_ready`] | `pub(crate)` | như (2) |
//! | 3b | [`Machine::tcb_yield`] | `pub` | gọi `yield_to_ready` (điều kiện do (3a) kiểm) |
//! | 4 | [`Machine::start_tcb`] | `pub` | `Inactive` |
//! | 5 | [`Machine::suspend_to_ready`] | `pub(crate)` | **đọc lại**: `is_blocked()` hoặc `Running{..}` |
//!
//! **Không** có `From`/`TryFrom` công khai nào từ [`TcbState`] sang [`NonReadyTarget`] — đó là
//! toàn bộ ranh giới.

use crate::error::{KError, ObjId, ObjType};
use crate::machine::Machine;
use crate::observer::CoreObserver;
use crate::store::KernelStore;
use crate::tcb::{FaultKind, SleepReason, TcbState};

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

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    // ------------------------------------------------------------------ cửa thông thường

    /// **Một cửa** đổi trạng thái TCB (D2-49(d)) — **không** nhận `Ready` (D2-187/D2-194):
    /// tra bảng [`crate::tcb::ALLOWED`]; cặp cấm trả `IllegalOperation`, **không** đổi trạng
    /// thái, đếm vào `forbidden_hits`.
    ///
    /// KT-21/25/27/34 hook của sim (`release_donation_of`, liên kết `Reply.caller`) nằm trong
    /// `write_state` — **một** cửa duy nhất.
    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError> {
        self.write_state(tcb, to.into_state())
    }

    /// Nguyên thuỷ thô — **private** của module này (xem đầu tệp). Thân là nguyên văn
    /// `transition_tcb` cũ (`core_tcb.rs`), chỉ đổi hai thứ: chữ ký/mức công khai, và lời gọi
    /// đệ quy `transition_tcb(cur, Ready)` ⇒ [`Machine::preempt_current`].
    ///
    /// **Giữ nguyên thứ tự hook** (parity core/sim): trả donation (KT-21) → `Reply.caller`
    /// (KT-34) → preempt khi vào `Running` → ghi `state` → `pair_hits` → `budget_sleeps` →
    /// `sched_sync`.
    fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        let from = self.tcb_state(tcb)?;
        if !crate::tcb::allowed(from, to) {
            self.obs.forbidden_hit(from, to);
            return Err(KError::IllegalOperation);
        }
        // **KT-21** — rời `BlockedOnReply` bằng đường **khác** `Reply` = bỏ cuộc gọi ⇒ trả SC
        // donation về client ngay (hook thứ nhất còn nợ từ KT-59).
        if matches!(from, TcbState::BlockedOnReply { .. })
            && !matches!(to, TcbState::BlockedOnReply { .. })
            && self.is_donation_donor(tcb)
        {
            self.release_donation_of(tcb)?;
        }
        // **KT-34** (C2, D2-63) — giữ liên kết `Reply ⇔ BlockedOnReply` ở **chính** cổng
        // trạng thái (hook thứ hai còn nợ từ KT-59):
        // - vào `BlockedOnReply{reply}` ⇒ `Reply.caller = Some(tcb)`; Reply **đã** bind người
        //   khác ⇒ `IllegalOperation`, **không** ghi đè;
        // - rời `BlockedOnReply` ⇒ gỡ `caller` nếu đang là `tcb`.
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
        // TCB chạy ⇒ đưa TCB cũ về `Ready` **trước**, qua cửa `preempt_current`.
        if let TcbState::Running { hart } = to {
            if hart as usize >= self.n_harts {
                return Err(KError::InvalidArgument);
            }
            // KT-29: `Running{hart}` chỉ hợp lệ khi `home == hart`.
            if self.tcb_data(tcb)?.home != hart {
                return Err(KError::IllegalOperation);
            }
            if let Some(cur) = self.harts[hart as usize].sched.current() {
                if cur != tcb {
                    self.preempt_current(cur)?;
                }
            }
        }
        self.tcb_data_mut(tcb)?.state = to;
        self.obs.pair_hit(from, to);
        // KT-23: đếm hoạt động ngân sách ở **một cửa**.
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
        self.sched_sync(tcb)?;
        Ok(())
    }

    // ------------------------------------------------------------------ năm cửa `Ready`

    /// **Cửa 1 — wake.** Thân chuyển `blocked/sleeping → Ready` — **một** chỗ duy nhất (máy
    /// kiểm `v2/tools/check-wake-paths.py --strict` đòi điều này). `unlink` quyết định có gỡ
    /// TCB khỏi **mọi** hàng chờ trước khi `Ready` hay không: đường hộp thư/`wake`/`wake_addr`
    /// cần (`true`); đường sản phẩm đã tự gỡ nên dùng [`Machine::wake_from`] (`false`).
    ///
    /// Điều kiện nguồn: `Ready` ⇒ `Ok` (idempotent, giữ nguyên); `is_blocked()`/`Sleeping` ⇒
    /// đi; còn lại ⇒ `IllegalOperation`.
    pub(crate) fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
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

    /// **Cửa 2 — preemption.** Nguồn phải `Running{hart}` **và** `sched[hart].current() ==
    /// Some(tcb)`; sai ⇒ `IllegalOperation` (fail-closed, không ghi gì).
    pub(crate) fn preempt_current(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        let TcbState::Running { hart } = state else {
            return Err(KError::IllegalOperation);
        };
        if hart as usize >= self.n_harts || self.harts[hart as usize].sched.current() != Some(tcb) {
            return Err(KError::IllegalOperation);
        }
        self.write_state(tcb, TcbState::Ready)
    }

    /// **Cửa 3a — yield (nội bộ).** Cùng điều kiện nguồn như [`Machine::preempt_current`].
    pub(crate) fn yield_to_ready(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        let TcbState::Running { hart } = state else {
            return Err(KError::IllegalOperation);
        };
        if hart as usize >= self.n_harts || self.harts[hart as usize].sched.current() != Some(tcb) {
            return Err(KError::IllegalOperation);
        }
        self.write_state(tcb, TcbState::Ready)
    }

    /// **Cửa 3b — yield (crate ngoài).** **Không** phải wrapper ghi `Ready`: đi qua
    /// `yield_to_ready` có kiểm điều kiện nguồn.
    pub fn tcb_yield(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.yield_to_ready(tcb)
    }

    /// **Cửa 4 — khởi tạo/start.** Nguồn phải `Inactive`; sai ⇒ `IllegalOperation`.
    pub fn start_tcb(&mut self, tcb: ObjId) -> Result<(), KError> {
        if self.tcb_state(tcb)? != TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        self.write_state(tcb, TcbState::Ready)
    }

    /// **Cửa 5 — suspend/abort.** **Đọc lại** trạng thái: `is_blocked()` hoặc `Running{..}`;
    /// sai ⇒ `IllegalOperation`.
    pub(crate) fn suspend_to_ready(&mut self, tcb: ObjId) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if !(state.is_blocked() || matches!(state, TcbState::Running { .. })) {
            return Err(KError::IllegalOperation);
        }
        self.write_state(tcb, TcbState::Ready)
    }
}
