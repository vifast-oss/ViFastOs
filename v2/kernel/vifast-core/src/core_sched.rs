// SPDX-License-Identifier: MIT OR Apache-2.0
//! Phần `impl Machine` của **KT-59**: scheduler theo hart + hộp thư + bất biến
//! (`check_sched`/`check_sc_refs`/`check_budget`/`check_mailboxes`).
//!
//! Tách khỏi `core_tcb.rs` để giữ luật cấu trúc **R1** (mỗi tệp ≤ 1.500 dòng); toàn bộ trạng
//! thái vẫn nằm trong [`Machine`] (`machine.rs`) — ba tệp là ba khối `impl Machine` của **cùng
//! một kiểu**, không có trạng thái thứ hai. Port nguyên ngữ nghĩa `v2/sim/vifast-sim/src/{sched,
//! mailbox}.rs` + `kobj.rs`; biên KT-59: chưa có IPC/Reply/Notification (KT-60).

use crate::error::{CheckFailure, KError, ObjId, ObjType};
use crate::machine::{Machine, MAX_HARTS, MAX_OBJECTS};
use crate::mailbox::{HartMailbox, MailboxMsg};
use crate::sched::Published;
use crate::tcb::TcbState;
use crate::NonReadyTarget;

impl Machine {
    // ------------------------------------------------------------- scheduler (KT-19/29)

    /// Số hart của mô hình (`1..=MAX_HARTS`).
    pub fn hart_count(&self) -> u16 {
        self.n_harts as u16
    }

    /// Hart "đang chạy" của mô hình tại thời điểm gọi (KT-29).
    pub fn current_hart(&self) -> u16 {
        self.current_hart
    }

    /// Đặt hart "đang chạy" của mô hình (`0..hart_count()`), dùng trước [`Machine::publish`].
    pub fn set_current_hart(&mut self, hart: u16) -> Result<(), KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        self.current_hart = hart;
        Ok(())
    }

    /// Trường **công bố** của `hart` (`idle`, `cur_prio`) — đọc được từ mọi hart.
    pub fn published(&self, hart: u16) -> Result<Published, KError> {
        self.published
            .get(hart as usize)
            .copied()
            .ok_or(KError::InvalidArgument)
    }

    /// Tự công bố `idle`/`cur_prio` của hart **đang chạy** (guard luật 3 §4.16).
    pub fn publish(&mut self, hart: u16, idle: bool, prio: u8) -> Result<(), KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        if self.current_hart != hart {
            return Err(KError::IllegalOperation);
        }
        if idle != self.sched[hart as usize].idle() || prio != self.sched[hart as usize].cur_prio()
        {
            return Err(KError::IllegalOperation);
        }
        self.published[hart as usize] = Published {
            idle,
            cur_prio: prio,
        };
        Ok(())
    }

    /// Số TCB đã di trú thành công từ đầu vòng đời `Machine`.
    pub fn migrate_count(&self) -> u64 {
        self.migrate_count
    }

    /// Bí danh của `mail_len` (tên dùng trong test/brief KT-29).
    pub fn mailbox_len(&self, hart: u16) -> Result<usize, KError> {
        self.mail_len(hart)
    }

    /// Làm tươi trường công bố từ `HartSched` sau mỗi op.
    fn refresh_published(&mut self) {
        for h in 0..self.n_harts {
            self.published[h] = Published {
                idle: self.sched[h].idle(),
                cur_prio: self.sched[h].cur_prio(),
            };
        }
    }

    fn hart_index(&self, hart: u16) -> usize {
        assert!(
            (hart as usize) < self.n_harts,
            "hart {hart} ngoài số hart {} của mô hình",
            self.n_harts
        );
        hart as usize
    }

    /// Số TCB trong ready queue của `hart` (không tính `current`).
    pub fn ready_len(&self, hart: u16) -> usize {
        self.sched[self.hart_index(hart)].len()
    }

    /// Số TCB trong bucket mức `prio` của `hart`.
    pub fn ready_len_at(&self, hart: u16, prio: u8) -> usize {
        self.sched[self.hart_index(hart)].len_at(prio)
    }

    /// TCB có nằm trong ready queue của `hart` không.
    pub fn ready_contains(&self, hart: u16, tcb: ObjId) -> bool {
        self.sched[self.hart_index(hart)].contains(tcb)
    }

    /// TCB đang chạy của `hart` (`None` ⇔ hart rảnh).
    pub fn current(&self, hart: u16) -> Option<ObjId> {
        self.sched[self.hart_index(hart)].current()
    }

    /// Hart có rảnh không.
    pub fn sched_idle(&self, hart: u16) -> bool {
        self.sched[self.hart_index(hart)].idle()
    }

    /// Mức ưu tiên của TCB đang chạy; `0` khi hart rảnh.
    pub fn sched_cur_prio(&self, hart: u16) -> u8 {
        self.sched[self.hart_index(hart)].cur_prio()
    }

    /// `pick()` O(1) của hart 0.
    pub fn sched_pick(&self) -> Option<ObjId> {
        self.sched_pick_on(0)
    }

    /// `pick()` của `hart`: mức cao nhất, cùng mức FIFO, chỉ TCB **có budget** (KT-20).
    pub fn sched_pick_on(&self, hart: u16) -> Option<ObjId> {
        self.sched[self.hart_index(hart)].pick_where(|tcb| self.tcb_has_budget(tcb))
    }

    /// Bật/tắt preempt **ép sau mỗi** public op (thiết kế §4.13). Mặc định tắt.
    pub fn set_preempt_every_step(&mut self, on: bool) {
        self.preempt_every_step = on;
    }

    /// Preempt ép có bật không.
    pub fn preempt_every_step(&self) -> bool {
        self.preempt_every_step
    }

    /// `dispatch(hart)`: `Running → Ready` (enqueue cuối hàng) rồi chọn TCB mức cao nhất.
    /// Trả `None` khi hàng rỗng. Một vòng push migration sau `dispatch` khi `n > 1`.
    pub fn dispatch(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        let got = self.dispatch_step(hart)?;
        if self.n_harts > 1 {
            self.refresh_published();
            let _ = self.push_step();
        }
        self.sched_after_op(false);
        Ok(got)
    }

    /// Thân của [`Machine::dispatch`] **không** tự kiểm bất biến.
    fn dispatch_step(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        // **KT-64** — hart còn `TlbFlush` chưa phục vụ **không** được chạy TCB: TLB có thể còn
        // ánh xạ cũ của trang vừa unmap/protect. `Busy` cho tới khi `tlb_service` flush thật.
        if self.tlb_blocks_dispatch(hart) {
            return Err(KError::Busy);
        }
        let h = hart as usize;
        if let Some(cur) = self.sched[h].current() {
            if !matches!(self.tcb_state(cur)?, TcbState::Running { .. }) {
                return Err(KError::IllegalOperation);
            }
            self.preempt_current(cur)?;
        }
        let next = match self.sched_pick_on(hart) {
            Some(tcb) => tcb,
            None => {
                self.sched[h].clear_current();
                return Ok(None);
            }
        };
        let info = self.tcb_data(next)?.snapshot();
        if info.state != TcbState::Ready || info.suspended {
            return Err(KError::IllegalOperation);
        }
        self.transition_tcb(next, NonReadyTarget::Running { hart })?;
        Ok(Some(next))
    }

    /// `block_current(hart, to)` — TCB đang chạy chuyển sang trạng thái chặn/ngủ `to` qua
    /// [`Machine::transition_tcb`] và rời `current`. Chỉ nhận `BlockedOn*`/`Sleeping`.
    ///
    /// **D2-194 §1(d)** — "không phải `Ready`" **chưa** đồng nghĩa "được phép block vào":
    /// `Running`/`Inactive`/`Stopping`/`Dead` vẫn phải bị từ chối ở **runtime** qua
    /// `NonReadyTarget::is_block_target`. So khớp `BlockedOnNotification` làm **trên
    /// `NonReadyTarget`** — module này **không** thấy `into_state` (private của `state_write`).
    pub fn block_current(&mut self, hart: u16, to: NonReadyTarget) -> Result<ObjId, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        if !to.is_block_target() {
            return Err(KError::InvalidArgument);
        }
        let cur = self.sched[hart as usize]
            .current()
            .ok_or(KError::IllegalOperation)?;
        // KT-27 (F3 reviewer-kt60): `BlockedOnNotification` phải ghi waiter trên object — bất
        // biến "waiter ⇔ TcbState"; kiểm **trước** khi gỡ hàng. Notification sai kiểu/chết
        // trả lỗi như sim.
        if let NonReadyTarget::BlockedOnNotification { ntfn } = to {
            if let Some(w) = self.notification_waiter(ntfn)? {
                if w != cur {
                    return Err(KError::Busy);
                }
            }
        }
        self.unlink_tcb_queues(cur);
        self.transition_tcb(cur, to)?;
        if let NonReadyTarget::BlockedOnNotification { ntfn } = to {
            self.notification_data_mut(ntfn)?.waiter = Some(cur);
        }
        self.sched_after_op(false);
        Ok(cur)
    }

    /// `wake(tcb)` — đánh thức TCB đang chặn/ngủ rồi `→ Ready`; **không** phát lại IPC.
    /// **KT-29**: khác hart ⇒ gửi `MailboxMsg::Wake` vào hộp thư `home` (luật 3).
    ///
    /// # CHỈ dùng cho test / ảnh test — sản phẩm dùng [`Machine::wake_from`]
    ///
    /// **KT-159 (N7, D2-144).** Hàm này **không** có call site sản phẩm nào (đo ở D2-144): chỉ
    /// ảnh test (`vifast-kbin-riscv64/src/ktest_inv.rs`, `ktest_user.rs`) và test sim gọi nó.
    /// Mọi đường đánh thức TCB **trong** kernel (IPC/Reply/Notification/finalise/refill) đi qua
    /// [`Machine::wake_from`] hoặc [`Machine::wake_from_unlink`]. Luật **máy kiểm**:
    /// `v2/tools/check-wake-paths.py --strict` **đỏ** (`E_STRICT_TEST_ONLY_WAKE`) khi có lời gọi
    /// `wake(` trong `v2/kernel/vifast-core/src/` hoặc `v2/sim/vifast-sim/src/` ngoài
    /// `#[cfg(test)]`/`mod tests`.
    ///
    /// **Vì sao hai ngữ nghĩa — cố ý, không phải trùng lặp:**
    ///
    /// - `wake()` = **post-only** khi khác hart: chỉ gửi `MailboxMsg::Wake` vào hộp thư `home`,
    ///   **không** đổi trạng thái TCB ngay. Đúng luật 3 (hart này không ghi trạng thái của hart
    ///   khác) và đúng cho người gọi **không** giữ bookkeeping nào của TCB — hart `home` tự
    ///   `wake_step` khi xử lý hộp thư.
    /// - [`Machine::wake_from`] = **post + `Ready` ngay**: các đường sản phẩm đã tự gỡ phần
    ///   bookkeeping của mình (waiter khỏi hàng chờ, `Reply.caller`, message khỏi
    ///   `Endpoint.send`) nên trạng thái **phải** nhất quán ngay; hoãn sang hart `home` sẽ vi
    ///   phạm bất biến C2 (`Reply.caller ⇔ BlockedOnReply`) và `check_ipc`.
    ///
    /// Gọi `wake()` trong mã sản phẩm là **bẫy**: TCB nằm chờ trong hộp thư trong khi
    /// bookkeeping đã bị gỡ ⇒ bất biến đỏ. Đó là lý do có luật máy kiểm ở trên.
    pub fn wake(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        if self.n_harts > 1 && info.home != self.current_hart {
            return self.post_mail(info.home, MailboxMsg::Wake(tcb));
        }
        self.wake_step(tcb, true)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-154 (N7-2)** — cửa **duy nhất** cho mọi đường đánh thức TCB **trong** kernel
    /// (IPC/Reply/Notification/finalise/refill). Khác hart ⇒ gửi `MailboxMsg::Wake` vào hộp
    /// thư `home` (đánh thức hart đích) **và** vẫn chuyển `Ready` cục bộ: các đường gọi đã
    /// tự gỡ đúng phần bookkeeping của mình (waiter khỏi hàng, `Reply.caller`, message khỏi
    /// `Endpoint.send`) nên trạng thái phải nhất quán **ngay** — hoãn sang hart `home` sẽ vi
    /// phạm bất biến C2 (`Reply.caller ⇔ BlockedOnReply`) và `check_ipc` (object bị finalise
    /// không còn để chờ). Cùng hart (và `n_harts == 1`) giữ nguyên đường cũ: chỉ `wake_step`,
    /// **không** message.
    ///
    /// `unlink = false` (xem [`Machine::wake_step`]): **không** gỡ TCB khỏi mọi hàng chờ —
    /// các đường gọi đã gỡ đúng phần của mình, và `unlink_tcb_queues` sẽ gỡ **cả** message
    /// `NBSend` không liên quan rồi đếm lại `ipc_dropped` (đường finalise đã đếm ⇒ hai lần).
    pub(crate) fn wake_from(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        let cheo_hart = self.n_harts > 1 && info.home != self.current_hart;
        self.wake_step(tcb, false)?;
        if cheo_hart {
            self.push_mail(info.home, MailboxMsg::Wake(tcb))?;
        }
        Ok(())
    }

    /// **KT-155 (N7-4)** — như [`Machine::wake_from`] nhưng **gỡ** TCB khỏi mọi hàng chờ
    /// trước khi `Ready` (`unlink = true`). Dùng cho `wake_addr`: waiter `BlockedOnAddress`
    /// phải được gỡ khỏi bucket WaitAddr, và khác hart thì phải gửi `Wake` về `home` (trước
    /// KT-155 `wake_addr` gọi `wake_step(tcb, true)` trực tiếp nên waiter ở hart khác **không**
    /// được đánh thức — nợ N7-4).
    pub(crate) fn wake_from_unlink(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        let cheo_hart = self.n_harts > 1 && info.home != self.current_hart;
        self.wake_step(tcb, true)?;
        if cheo_hart {
            self.push_mail(info.home, MailboxMsg::Wake(tcb))?;
        }
        Ok(())
    }

    /// `yield_current(hart)` — `Running → Ready` (cuối hàng cùng mức) rồi `dispatch`.
    pub fn yield_current(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let cur = self.sched[hart as usize]
            .current()
            .ok_or(KError::IllegalOperation)?;
        self.yield_to_ready(cur)?;
        let next = self.dispatch(hart)?;
        self.sched_after_op(false);
        Ok(next)
    }

    /// Chạy **một** vòng push migration công khai (model API `[TK]`).
    pub fn push_migration(&mut self) -> Result<usize, KError> {
        self.refresh_published();
        let n = self.push_step();
        self.sched_after_op(false);
        Ok(n)
    }

    /// Đồng bộ **một** TCB với ready queue sau khi `state`/`suspended`/`priority`/`home`/
    /// `affinity` đổi. Idempotent: gỡ bản cũ rồi đặt chỗ mới theo state hiện tại.
    pub(crate) fn sched_sync(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        let prio = self.effective_priority(tcb)?;
        for h in 0..self.n_harts {
            self.sched[h].remove_at(tcb, prio);
        }
        for h in 0..self.n_harts {
            let giu = matches!(info.state, TcbState::Running { hart } if hart as usize == h);
            if !giu && self.sched[h].current() == Some(tcb) {
                self.sched[h].clear_current();
            }
        }
        match info.state {
            TcbState::Ready if !info.suspended && info.affinity & (1u64 << info.home) != 0 => {
                let h = info.home as usize;
                if h >= self.n_harts {
                    return Err(KError::InvalidArgument);
                }
                self.sched[h].enqueue(tcb, prio);
            }
            TcbState::Running { hart } => {
                let h = hart as usize;
                if h >= self.n_harts {
                    return Err(KError::InvalidArgument);
                }
                if let Some(cur) = self.sched[h].current() {
                    if cur != tcb {
                        return Err(KError::IllegalOperation);
                    }
                }
                self.sched[h].set_current(tcb, prio);
            }
            _ => {}
        }
        Ok(())
    }

    /// Gỡ `tcb` khỏi **mọi** bucket của mọi hart (gọi trước khi sửa priority/home/affinity).
    pub(crate) fn sched_remove_all(&mut self, tcb: ObjId) {
        for h in 0..self.n_harts {
            self.sched[h].remove_all(tcb);
        }
    }

    /// Sau **mỗi** public op: preempt ép (nếu bật) + làm tươi `published` + kiểm bất biến
    /// scheduler/mailbox (debug assertion — lỗi mô hình không được im lặng).
    pub(crate) fn sched_after_op(&mut self, preempt: bool) {
        if preempt && self.preempt_every_step {
            for h in 0..self.n_harts {
                if self.dispatch_step(h as u16).is_err() {
                    debug_assert!(false, "preempt_every_step: dispatch lỗi");
                    return;
                }
            }
            self.refresh_published();
            if self.n_harts > 1 {
                let _ = self.push_step();
            }
        }
        self.refresh_published();
        if let Err(e) = self.check_sched() {
            debug_assert!(false, "bất biến scheduler vỡ sau public op: {e}");
        }
        if let Err(e) = self.check_mailboxes() {
            debug_assert!(false, "bất biến mailbox vỡ sau public op: {e}");
        }
        // KT-60: bất biến IPC/Reply/Notification/WaitAddr/fault — như `sched_after_op` của sim
        // (cùng chỗ, mỗi hàm có cửa sổ nhanh).
        if let Err(e) = self.check_ipc() {
            debug_assert!(
                false,
                "bất biến IPC (bảo toàn message) vỡ sau public op: {e:?}"
            );
        }
        if let Err(e) = self.check_reply_links() {
            debug_assert!(false, "bất biến Reply (C2) vỡ sau public op: {e:?}");
        }
        if let Err(e) = self.check_notifications() {
            debug_assert!(false, "bất biến Notification vỡ sau public op: {e:?}");
        }
        if let Err(e) = self.check_waitaddr() {
            debug_assert!(false, "bất biến WaitOnAddress vỡ sau public op: {e:?}");
        }
        if let Err(e) = self.check_fault() {
            debug_assert!(false, "bất biến fault vỡ sau public op: {e:?}");
        }
    }

    /// Một vòng **push migration** tất định (thiết kế §4.8 `:338-342`, D2-53 Q2).
    fn push_step(&mut self) -> usize {
        let n = self.n_harts;
        if n <= 1 {
            return 0;
        }
        let mut nhan = [false; MAX_HARTS];
        for x in 0..n {
            let Some(top) = self.sched[x].highest_priority() else {
                continue;
            };
            if self.sched[x].len_at(top) < 2 {
                continue;
            }
            let Some(cand) = self.sched[x].last_at(top) else {
                continue;
            };
            let Ok(info) = self.tcb_data(cand) else {
                continue;
            };
            let mut dest = None;
            for y in 0..n {
                if y == x || info.affinity & (1u64 << y) == 0 {
                    continue;
                }
                if self.published[y].idle {
                    dest = Some(y);
                    break;
                }
            }
            let Some(y) = dest else {
                continue;
            };
            let msg = MailboxMsg::Migrate {
                tcb: cand,
                to_hart: y as u16,
            };
            if self.mail[y].try_push(msg).is_ok() {
                nhan[y] = true;
            }
        }
        let mut applied = 0usize;
        for (y, co_nhan) in nhan.iter().enumerate() {
            if *co_nhan {
                applied += self.drain_mail_fifo(y as u16).1;
            }
        }
        applied
    }

    /// KT-29: thân di trú một TCB sang `to` (kiểm lại trạng thái lúc xử lý).
    fn apply_migrate(&mut self, tcb: ObjId, to: u16) -> bool {
        let Ok(info) = self.tcb_data(tcb) else {
            return false;
        };
        if info.state != TcbState::Ready
            || info.suspended
            || info.home == to
            || to as usize >= self.n_harts
            || info.affinity & (1u64 << to) == 0
        {
            return false;
        }
        self.sched_remove_all(tcb);
        if self.tcb_data_mut(tcb).is_err() {
            return false;
        }
        self.tcb_data_mut(tcb).unwrap().home = to;
        if self.sched_sync(tcb).is_err() {
            return false;
        }
        self.migrate_count += 1;
        true
    }

    /// KT-59: core chưa có hàng chờ Endpoint/Reply/Notification/WaitAddr (KT-60) nên chỉ là
    /// mốc — scheduler queue do `sched_sync` gỡ; trả số hàng đã gỡ (0).
    pub(crate) fn set_tcb_suspended(&mut self, tcb: ObjId, suspended: bool) -> Result<(), KError> {
        self.tcb_data_mut(tcb)?.suspended = suspended;
        self.sched_sync(tcb)
    }

    // ------------------------------------------------------------- hộp thư (KT-22)

    /// Gửi một message vào hộp thư của hart (không chờ).
    pub fn post_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        let posted = self.push_mail(hart, msg);
        self.sched_after_op(false);
        posted
    }

    fn push_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        if let MailboxMsg::Migrate { tcb, to_hart } = msg {
            if self.n_harts == 1 {
                return Err(KError::NotSupported);
            }
            if hart != to_hart {
                return Err(KError::InvalidArgument);
            }
            self.check_migrate_request(tcb, to_hart)?;
        }
        if self.mail[hart as usize].is_full() {
            return Err(KError::Busy);
        }
        match self.mail[hart as usize].try_push(msg) {
            Ok(()) => Ok(()),
            Err(_) => {
                debug_assert!(false, "hộp thư hart {hart} đầy dù vừa kiểm");
                Err(KError::Busy)
            }
        }
    }

    /// KT-29: kiểm một yêu cầu `Migrate` **trước** khi vào hộp thư.
    fn check_migrate_request(&self, tcb: ObjId, to_hart: u16) -> Result<(), KError> {
        if to_hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let info = self.tcb_data(tcb)?;
        if info.state != TcbState::Ready || info.suspended {
            return Err(KError::InvalidArgument);
        }
        if info.home == to_hart {
            return Err(KError::InvalidArgument);
        }
        let can = (1u64 << info.home) | (1u64 << to_hart);
        if info.affinity & can != can {
            return Err(KError::InvalidArgument);
        }
        for hb in &self.mail[..self.n_harts] {
            if hb
                .iter()
                .any(|m| matches!(m, MailboxMsg::Migrate { tcb: t, .. } if *t == tcb))
            {
                return Err(KError::InvalidArgument);
            }
        }
        Ok(())
    }

    /// Xử lý (drain) hộp thư của hart theo **FIFO**; mỗi message áp dụng **đúng một lần**.
    pub fn process_mail(&mut self, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let (n, _) = self.drain_mail_fifo(hart);
        self.sched_after_op(false);
        Ok(n)
    }

    /// `send_mail` = `post_mail` + `process_mail` trong **một** bước.
    pub fn send_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        self.post_mail(hart, msg)?;
        self.process_mail(hart)?;
        Ok(())
    }

    /// Số message đang nằm trong hộp thư của hart.
    pub fn mail_len(&self, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].len())
    }

    /// **KT-60** — nội dung hàng chờ hộp thư hart (FIFO) — parity so head/tail.
    pub fn mail_items(&self, hart: u16) -> Result<MailItems<'_>, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        Ok(MailItems {
            m: &self.mail[hart as usize],
            idx: 0,
        })
    }

    /// Nhật ký message **đã xử lý** của hộp thư hart, đúng thứ tự drain.
    pub fn mail_log(&self, hart: u16) -> Result<&[MailboxMsg], KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].log())
    }

    /// Hart đã nhận message `Stop` chưa.
    pub fn hart_stopped(&self, hart: u16) -> Result<bool, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].stopped())
    }

    /// Số message đã xử lý nhưng không còn chỗ trong nhật ký (biên tĩnh `MAX_MAIL_LOG`).
    pub fn mail_log_overflow(&self, hart: u16) -> Result<u64, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].log_overflow())
    }

    /// Drain hộp thư hart theo FIFO; trả `(số message đã xử lý, số TCB di trú thành công)`.
    fn drain_mail_fifo(&mut self, hart: u16) -> (usize, usize) {
        let mut n = 0usize;
        let mut di_tru = 0usize;
        while let Some(msg) = self.mail[hart as usize].pop() {
            match msg {
                MailboxMsg::Migrate { tcb, to_hart } => {
                    if to_hart != hart {
                        debug_assert!(false, "Migrate nằm sai hộp thư hart đích");
                    } else if self.apply_migrate(tcb, to_hart) {
                        di_tru += 1;
                    }
                }
                other => self.apply_mail(hart, other),
            }
            self.mail[hart as usize].record(msg);
            n += 1;
        }
        (n, di_tru)
    }

    /// Áp dụng **một** message lên mô hình — infallible (message không hợp lệ là no-op).
    fn apply_mail(&mut self, hart: u16, msg: MailboxMsg) {
        match msg {
            MailboxMsg::Wake(tcb) => {
                let Ok(info) = self.tcb_data(tcb) else {
                    return;
                };
                if info.home != hart {
                    return;
                }
                if info.state.is_blocked() || matches!(info.state, TcbState::Sleeping { .. }) {
                    let _ = self.wake_step(tcb, true);
                }
                // **KT-64** — hart đang chờ `TlbFlush` thì **không** preempt/dispatch: phải
                // phục vụ flush trước (tránh chạy trên TLB cũ); message vẫn được xử lý xong.
                if self.n_harts > 1 && !self.tlb_pending[hart as usize].active {
                    let Ok(eff) = self.effective_priority(tcb) else {
                        return;
                    };
                    if eff > self.sched[hart as usize].cur_prio()
                        && self.dispatch_step(hart).is_err()
                    {
                        debug_assert!(false, "Wake: preempt lỗi (bất biến scheduler vỡ)");
                    }
                }
            }
            MailboxMsg::Reschedule => {
                if !self.tlb_pending[hart as usize].active && self.dispatch_step(hart).is_err() {
                    debug_assert!(false, "Reschedule: dispatch lỗi (bất biến scheduler vỡ)");
                }
            }
            MailboxMsg::Stop => self.mail[hart as usize].set_stopped(),
            // **KT-64** — `TlbFlush` chỉ được **thi hành** bởi [`Machine::tlb_service`] (có
            // `TlbEnv`: `sfence.vma` thật trên hart đó + ack). Drain hộp thư **không có env**
            // (đường model cũ) chỉ đếm message đã xử lý và **giữ** `tlb_pending` ⇒
            // `dispatch` vẫn `Busy` (fail-closed, không chạy trên TLB cũ).
            MailboxMsg::TlbFlush(_) => {}
            MailboxMsg::Migrate { tcb, to_hart } => {
                if to_hart != hart {
                    debug_assert!(false, "Migrate nằm sai hộp thư hart đích");
                    return;
                }
                self.apply_migrate(tcb, to_hart);
            }
        }
    }

    // ------------------------------------------------------------- bất biến (KT-19/20/22/29)

    /// Bất biến scheduler sau **mỗi** public op:
    /// - `Ready` (không treo) ⇔ có trong ready queue đúng hart home;
    /// - `Running{hart}` ⇔ là `current` của hart, **không** nằm trong hàng; ≤1 `Running`/hart;
    /// - bucket == priority hiệu lực; home nằm trong affinity; `len`/bitmap/idle khớp.
    pub fn check_sched(&self) -> Result<(), CheckFailure> {
        if self.n_harts == 0 {
            return Err(CheckFailure::one("mo hinh khong co hart", 0));
        }
        // 255 = không nằm trong hàng; ngược lại là chỉ số hart.
        let mut hang_cua = [255u8; MAX_OBJECTS + 1];
        for h in 0..self.n_harts {
            let hs = &self.sched[h];
            hs.check_local()?;
            hs.for_each_queued(|prio, tcb| {
                if tcb == 0 || tcb as usize > MAX_OBJECTS {
                    return Err(CheckFailure::two("TCB ngoai bang", tcb, 0));
                }
                if hang_cua[tcb as usize] != 255 {
                    return Err(CheckFailure::two("TCB trung trong ready queue", tcb, 0));
                }
                let info = self
                    .tcb_data(tcb)
                    .map_err(|_| CheckFailure::two("TCB trong hang khong doc duoc", tcb, 0))?;
                if info.state != TcbState::Ready {
                    return Err(CheckFailure::two(
                        "TCB trong hang nhung khong Ready",
                        tcb,
                        info.state.index() as u32,
                    ));
                }
                if info.suspended {
                    return Err(CheckFailure::two("TCB suspended van trong hang", tcb, 0));
                }
                let eff = self
                    .effective_priority(tcb)
                    .map_err(|_| CheckFailure::two("priority hieu luc loi", tcb, 0))?;
                if eff != prio {
                    return Err(CheckFailure::two(
                        "bucket lech priority hieu luc",
                        tcb,
                        eff as u32,
                    ));
                }
                if info.home as usize >= self.n_harts || info.affinity & (1u64 << info.home) == 0 {
                    return Err(CheckFailure::two(
                        "home ngoai affinity",
                        tcb,
                        info.home as u32,
                    ));
                }
                hang_cua[tcb as usize] = h as u8;
                Ok(())
            })?;
            match hs.current() {
                Some(cur) => {
                    if cur == 0 || cur as usize > MAX_OBJECTS {
                        return Err(CheckFailure::one("current ngoai bang", cur));
                    }
                    let info = self
                        .tcb_data(cur)
                        .map_err(|_| CheckFailure::two("current khong doc duoc", cur, 0))?;
                    match info.state {
                        TcbState::Running { hart } if hart as usize == h => {}
                        _ => {
                            return Err(CheckFailure::two(
                                "current khong Running dung hart",
                                cur,
                                h as u32,
                            ));
                        }
                    }
                    if hang_cua[cur as usize] != 255 {
                        return Err(CheckFailure::two("TCB Running van trong hang", cur, 0));
                    }
                    let eff = self
                        .effective_priority(cur)
                        .map_err(|_| CheckFailure::two("current priority loi", cur, 0))?;
                    if eff != hs.cur_prio() {
                        return Err(CheckFailure::two(
                            "cur_prio lech priority hieu luc",
                            hs.cur_prio() as u32,
                            eff as u32,
                        ));
                    }
                    if hs.idle() {
                        return Err(CheckFailure::two("hart co current nhung idle", h as u32, 0));
                    }
                }
                None => {
                    if !hs.idle() {
                        return Err(CheckFailure::two(
                            "hart khong current nhung busy",
                            h as u32,
                            0,
                        ));
                    }
                }
            }
        }
        // Mọi TCB sống: Ready ⇔ trong hàng (đúng hart home); Running ⇔ current; còn lại
        // **không** được nằm trong hàng.
        for i in 0..self.n_objects {
            if !self.objects[i].alive || self.objects[i].ty != ObjType::Tcb {
                continue;
            }
            let obj = (i as u32) + 1;
            let info = self
                .tcb_data(obj)
                .map_err(|_| CheckFailure::two("TCB song khong doc duoc", obj, 0))?;
            let hang = hang_cua[obj as usize];
            match info.state {
                TcbState::Ready => {
                    if info.suspended {
                        if hang != 255 {
                            return Err(CheckFailure::two(
                                "Ready+suspended van trong hang",
                                obj,
                                0,
                            ));
                        }
                    } else if hang != info.home as u8 {
                        return Err(CheckFailure::two(
                            "Ready khong nam trong hang home",
                            obj,
                            info.home as u32,
                        ));
                    }
                }
                TcbState::Running { hart } => {
                    if self.sched.get(hart as usize).and_then(|x| x.current()) != Some(obj) {
                        return Err(CheckFailure::two(
                            "Running khong phai current",
                            obj,
                            hart as u32,
                        ));
                    }
                    if hang != 255 {
                        return Err(CheckFailure::two("Running van trong hang", obj, 0));
                    }
                }
                _ => {
                    if hang != 255 {
                        return Err(CheckFailure::two("TCB khong Ready van trong hang", obj, 0));
                    }
                }
            }
        }
        self.check_sc_refs()?;
        self.check_budget()?;
        Ok(())
    }

    /// KT-20/KT-21: tham chiếu `SchedContext` của TCB + donation hai chiều (KT-59 chưa tạo
    /// donation; kiểm đối xứng để KT-60 không phải sửa checker).
    pub fn check_sc_refs(&self) -> Result<(), CheckFailure> {
        for i in 0..self.n_objects {
            if !self.objects[i].alive {
                continue;
            }
            let obj = (i as u32) + 1;
            match self.objects[i].ty {
                ObjType::Tcb => {
                    let info = self
                        .tcb_data(obj)
                        .map_err(|_| CheckFailure::two("TCB khong doc duoc", obj, 0))?;
                    if let Some(sc) = info.sched_context {
                        let s = self
                            .sc_data(sc)
                            .map_err(|_| CheckFailure::two("TCB tro SC chet", obj, sc))?;
                        // KT-60: donor (chủ SC) vẫn giữ `sched_context` khi SC đang donate —
                        // chỉ TCB khác dùng SC đang donate mới là lỗi.
                        if s.donated_to.is_some() && s.donor != Some(obj) {
                            return Err(CheckFailure::two("TCB dung SC dang donate", obj, sc));
                        }
                    }
                    if let Some(sc) = info.donated_sc {
                        if info.sched_context.is_some() {
                            return Err(CheckFailure::two("donee vua co SC rieng", obj, sc));
                        }
                        let s = self
                            .sc_data(sc)
                            .map_err(|_| CheckFailure::two("donee tro SC chet", obj, sc))?;
                        if s.donated_to != Some(obj) || s.donor.is_none() {
                            return Err(CheckFailure::two("donation khong doi xung", obj, sc));
                        }
                    }
                    if info.home as usize >= self.n_harts
                        || info.affinity & (1u64 << info.home) == 0
                    {
                        return Err(CheckFailure::two(
                            "home ngoai affinity",
                            obj,
                            info.home as u32,
                        ));
                    }
                }
                ObjType::SchedContext => {
                    let s = self
                        .sc_data(obj)
                        .map_err(|_| CheckFailure::two("SC khong doc duoc", obj, 0))?;
                    match (s.donated_to, s.donor) {
                        (None, None) => {}
                        (Some(donee), Some(donor)) => {
                            let di = self.tcb_data(donee).map_err(|_| {
                                CheckFailure::two("donee khong doc duoc", donee, obj)
                            })?;
                            let oi = self.tcb_data(donor).map_err(|_| {
                                CheckFailure::two("donor khong doc duoc", donor, obj)
                            })?;
                            if di.donated_sc != Some(obj) || oi.sched_context != Some(obj) {
                                return Err(CheckFailure::two(
                                    "donation khong doi xung",
                                    obj,
                                    donee,
                                ));
                            }
                            if !matches!(oi.state, TcbState::BlockedOnReply { .. }) {
                                return Err(CheckFailure::two(
                                    "donor khong BlockedOnReply",
                                    donor,
                                    oi.state.index() as u32,
                                ));
                            }
                        }
                        _ => return Err(CheckFailure::one("donation nua voi", obj)),
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// KT-20: `Running ⇒ còn budget`; mọi `SchedContext` sống thoả `ScBudget::check`.
    pub fn check_budget(&self) -> Result<(), CheckFailure> {
        for i in 0..self.n_objects {
            if !self.objects[i].alive {
                continue;
            }
            let obj = (i as u32) + 1;
            if self.objects[i].ty == ObjType::Tcb {
                let info = self
                    .tcb_data(obj)
                    .map_err(|_| CheckFailure::two("TCB khong doc duoc", obj, 0))?;
                if matches!(info.state, TcbState::Running { .. }) && !self.tcb_has_budget(obj) {
                    return Err(CheckFailure::one("Running nhung het budget", obj));
                }
            } else if self.objects[i].ty == ObjType::SchedContext {
                let s = self
                    .sc_data(obj)
                    .map_err(|_| CheckFailure::two("SC khong doc duoc", obj, 0))?;
                s.budget.check(self.now)?;
            }
        }
        Ok(())
    }

    /// KT-22: bất biến hàng FIFO mỗi hộp thư + ≤1 `Migrate`/TCB đang chờ.
    pub fn check_mailboxes(&self) -> Result<(), CheckFailure> {
        for h in 0..self.n_harts {
            self.mail[h].check(h as u16)?;
            for msg in self.mail[h].iter() {
                if let MailboxMsg::Migrate { tcb, to_hart } = msg {
                    if *to_hart != h as u16 {
                        return Err(CheckFailure::two("Migrate sai hop thu", h as u32, *tcb));
                    }
                }
            }
        }
        // ≤1 yêu cầu Migrate cho mỗi TCB trên mọi hộp thư.
        for i in 0..self.n_objects {
            if !self.objects[i].alive || self.objects[i].ty != ObjType::Tcb {
                continue;
            }
            let obj = (i as u32) + 1;
            let mut count = 0u32;
            for hb in &self.mail[..self.n_harts] {
                for m in hb.iter() {
                    if matches!(m, MailboxMsg::Migrate { tcb, .. } if *tcb == obj) {
                        count += 1;
                    }
                }
            }
            if count > 1 {
                return Err(CheckFailure::one("nhieu Migrate cho mot TCB", obj));
            }
        }
        Ok(())
    }
}

/// Iterator nội dung hàng chờ hộp thư hart (`mail_items`).
#[derive(Debug, Clone)]
pub struct MailItems<'a> {
    m: &'a HartMailbox,
    idx: usize,
}

impl Iterator for MailItems<'_> {
    type Item = MailboxMsg;

    fn next(&mut self) -> Option<MailboxMsg> {
        let msg = self.m.iter().nth(self.idx).copied();
        if msg.is_some() {
            self.idx += 1;
        }
        msg
    }
}
