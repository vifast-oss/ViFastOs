// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `sched`: scheduler hart, ready queue, wake/block/yield, hộp thư + migration.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------- scheduler theo hart (KT-19) `[TK]`
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-49(a)): KT-19 dựng **1 hart** nhưng mọi
    // thao tác viết theo hart để K1c thêm hart/push migration. `HartSched` nằm ở
    // [`crate::sched`]; phần dưới là **một cửa** nối TCB ↔ ready queue.
    //
    // Bất biến (DoD 2/3, `check_sched` kiểm sau **mỗi** public op):
    // - `Ready` (không treo) ⇔ có trong ready queue; cùng mức FIFO;
    // - `Running{hart}` ⇔ `current(hart)`, **không** nằm trong hàng; ≤1 `Running`/hart;
    // - bucket == `priority`; `len`/bitmap/`idle`/`cur_prio` khớp.

    /// Số hart của mô hình (KT-19: 1 — D2-49(a); **KT-29**: 1/2/4/8 qua
    /// [`Machine::with_harts`]).
    pub fn hart_count(&self) -> u16 {
        self.sched.len() as u16
    }

    /// **KT-29** — hart "đang chạy" của mô hình tại thời điểm gọi (mô hình một luồng mô phỏng
    /// N hart tất định, D2-53 Q1). Mặc định 0; [`Machine::publish`] chỉ ghi được hart này
    /// (luật 3 §4.16).
    pub fn current_hart(&self) -> u16 {
        self.current_hart
    }

    /// **KT-29** — đặt hart "đang chạy" của mô hình (`0..hart_count()`), dùng trước
    /// [`Machine::publish`] để mô phỏng hart nào tự công bố trạng thái của mình.
    /// Không đổi scheduler/ready queue; hart không tồn tại ⇒ `InvalidArgument`.
    pub fn set_current_hart(&mut self, hart: u16) -> Result<(), KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        self.current_hart = hart;
        Ok(())
    }

    /// **KT-29** — trường **công bố** của `hart` (`idle`, `cur_prio`; thiết kế §4.8/§4.16 luật
    /// 3). **Đọc được từ mọi hart** (không tính vi phạm luật 3); hart ngoài mô hình ⇒
    /// `InvalidArgument`.
    pub fn published(&self, hart: u16) -> Result<Published, KError> {
        self.published
            .get(hart as usize)
            .copied()
            .ok_or(KError::InvalidArgument)
    }

    /// **KT-29** — tự công bố `idle`/`cur_prio` của hart **đang chạy** (`current_hart()`).
    ///
    /// Guard **luật 3** (§4.16 `:415,419`): ghi trạng thái hart khác ⇒ `Err(IllegalOperation)`
    /// và **không** đổi gì. Thiết kế nói "panic" cho build sim; mô hình trả lỗi để test âm
    /// quan sát được **không** cần `#[should_panic]` (K1C-PLAN §5 KT-29 DoD 2 — ghi rõ).
    /// Mô hình một luồng giữ `published` là **ảnh trung thực** của `HartSched` (bất biến
    /// `check_smp`: `published.idle ⇔ current == None`), nên giá trị công bố phải khớp trạng
    /// thái thật; lệch ⇒ `IllegalOperation` (hart không thể "nói dối" trong mô hình này).
    pub fn publish(&mut self, hart: u16, idle: bool, prio: u8) -> Result<(), KError> {
        if hart as usize >= self.sched.len() {
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
        // Bất biến (iv) của `check_smp`: published là **ảnh trung thực** của scheduler; assert
        // tại cửa ghi để lỗi nội bộ lộ ngay (lỗi người dùng ở trên đã trả `Err` trước).
        debug_assert_eq!(
            self.published[hart as usize].idle,
            self.sched[hart as usize].idle(),
            "publish: published.idle lệch current == None"
        );
        debug_assert_eq!(
            self.published[hart as usize].cur_prio,
            self.sched[hart as usize].cur_prio(),
            "publish: published.cur_prio lệch cur_prio thật"
        );
        Ok(())
    }

    /// **KT-29** — số TCB đã di trú thành công (qua `MailboxMsg::Migrate`) từ đầu vòng đời
    /// `Machine` (nguồn test/đo "số migrate", thiết kế §4.8).
    pub fn migrate_count(&self) -> u64 {
        self.migrate_count
    }

    /// **KT-29** — bí danh của [`Machine::mail_len`] (tên dùng trong test/brief KT-29:
    /// "mailbox_len(hart)"); hart ngoài mô hình ⇒ `InvalidArgument`.
    pub fn mailbox_len(&self, hart: u16) -> Result<usize, KError> {
        self.mail_len(hart)
    }

    /// **KT-29** — làm tươi trường công bố từ `HartSched` (một luồng, tất định): sau mỗi op
    /// `published[h] = { idle: sched[h].idle(), cur_prio: sched[h].cur_prio() }`.
    pub(super) fn refresh_published(&mut self) {
        for h in 0..self.sched.len() {
            self.published[h] = Published {
                idle: self.sched[h].idle(),
                cur_prio: self.sched[h].cur_prio(),
            };
        }
    }

    pub(super) fn hart_index(&self, hart: u16) -> usize {
        assert!(
            (hart as usize) < self.sched.len(),
            "hart {hart} ngoài số hart {} của mô hình",
            self.sched.len()
        );
        hart as usize
    }

    /// Số TCB trong ready queue của `hart` (không tính `current`).
    pub fn ready_len(&self, hart: u16) -> usize {
        self.sched[self.hart_index(hart)].len()
    }

    /// Số TCB trong bucket mức `prio` của `hart` — kiểm "bucket khớp priority".
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

    /// Hart có rảnh không (`current == None`).
    pub fn sched_idle(&self, hart: u16) -> bool {
        self.sched[self.hart_index(hart)].idle()
    }

    /// Mức ưu tiên của TCB đang chạy; `0` khi hart rảnh.
    pub fn sched_cur_prio(&self, hart: u16) -> u8 {
        self.sched[self.hart_index(hart)].cur_prio()
    }

    /// `pick()` O(1) của hart 0 — tiện cho mô hình **1 hart**; nhiều hart dùng
    /// [`Machine::sched_pick_on`].
    pub fn sched_pick(&self) -> Option<ObjId> {
        self.sched_pick_on(0)
    }

    /// `pick()` O(1) của `hart`: mức cao nhất, cùng mức FIFO; **không** gỡ khỏi hàng.
    ///
    /// **KT-20** (follow-up KT-19): chỉ trả TCB **có budget** — TCB không SC **hoặc** SC chưa
    /// `SchedControl.Configure` **không có budget** nên bị bỏ qua, TCB có SC đã cấu hình mà
    /// `consumed == budget` cũng bị bỏ qua (§4.5, K1B-PLAN §5 KT-20 DoD 4). `pick` vẫn O(1)
    /// khi phần tử đầu hàng hợp lệ (KT-19), chỉ quét tiếp khi gặp TCB bị loại.
    /// **KT-21**: server passive (`donated_sc`) được `pick` nhờ ngân sách SC của client; server
    /// passive **không** được donate vẫn bị bỏ qua (D2-50).
    pub fn sched_pick_on(&self, hart: u16) -> Option<ObjId> {
        self.sched[self.hart_index(hart)].pick_where(|tcb| self.tcb_has_budget(tcb))
    }

    /// Bật/tắt preempt **ép sau mỗi** public op (thiết kế §4.13). Mặc định tắt để giữ
    /// nguyên ngữ nghĩa KT-18; test KT-19 bật.
    pub fn set_preempt_every_step(&mut self, on: bool) {
        self.preempt_every_step = on;
    }

    pub fn preempt_every_step(&self) -> bool {
        self.preempt_every_step
    }

    /// `dispatch(hart)`: đưa TCB đang chạy (`Running`) về `Ready` (enqueue lại cuối hàng
    /// cùng mức) rồi chọn TCB mức cao nhất (`Ready → Running`). Trả `None` khi hàng rỗng
    /// (hart rảnh). Mọi chuyển trạng thái đi qua [`Machine::transition_tcb`].
    ///
    /// **KT-29**: với mô hình nhiều hart, sau `dispatch` chạy **một** vòng push migration
    /// (DoD 3: "sau mỗi op … và/hoặc `dispatch`") — `dispatch` của `n = 1` **không** đổi gì.
    pub fn dispatch(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        let got = self.dispatch_step(hart)?;
        if self.sched.len() > 1 {
            self.refresh_published();
            let _ = self.push_step();
        }
        self.sched_after_op(false);
        Ok(got)
    }

    /// Thân của [`Machine::dispatch`] **không** tự kiểm bất biến — `sched_after_op` gọi
    /// thẳng để một op không phải kiểm hai lần (hiệu năng Miri), còn `dispatch` công khai
    /// kiểm sau khi xong.
    pub(super) fn dispatch_step(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
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
        // Hàng chỉ chứa TCB `Ready` + `!suspended`; kiểm lại để lỗi rõ nếu bất biến vỡ.
        let info = self.tcb_info_obj(next)?;
        if info.state != TcbState::Ready || info.suspended {
            return Err(KError::IllegalOperation);
        }
        self.transition_tcb(next, NonReadyTarget::Running { hart })?;
        Ok(Some(next))
    }

    /// `block_current(hart, to)` — model API `[TK]` (DoD 5): TCB đang chạy của hart chuyển
    /// sang trạng thái chặn/ngủ `to` qua [`Machine::transition_tcb`] và rời `current`.
    /// Chỉ nhận trạng thái `BlockedOn*`/`Sleeping` (không dùng để đổi sang `Ready`).
    ///
    /// **KT-241** — `to` là [`NonReadyTarget`] (kiểu **không** biểu diễn được `Ready`), nhưng
    /// vẫn phải kiểm [`NonReadyTarget::is_block_target`] ở **runtime**: `Running`/`Inactive`/
    /// `Stopping`/`Dead` **không** phải đích block hợp lệ (D2-194 §1(d)).
    ///
    /// Đây là **primitive scheduler** cho test, không công bố wait trên object nào — trừ
    /// **KT-27**: `BlockedOnNotification{ntfn}` ghi `waiter` lên Notification để bất biến
    /// "waiter ⇔ `TcbState`" luôn đúng. Các đường IPC thật (`call`/`recv`/`tcb_wait_*`) mới ghi
    /// hàng chờ của Endpoint/Reply/Notification; [`Machine::wake`] gỡ mọi tham chiếu còn lại
    /// trước khi `→ Ready`.
    pub fn block_current(&mut self, hart: u16, to: NonReadyTarget) -> Result<ObjId, KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        // **D2-194 §1(d)** — "không phải `Ready`" CHƯA đồng nghĩa "được phép block vào":
        // `Running`/`Inactive`/`Stopping`/`Dead` vẫn phải bị từ chối ở **runtime** (kiểu đích
        // `NonReadyTarget` chỉ loại `Ready`, không loại bốn trạng thái kia).
        if !to.is_block_target() {
            return Err(KError::InvalidArgument);
        }
        let cur = self.sched[hart as usize]
            .current()
            .ok_or(KError::IllegalOperation)?;
        // **KT-27** — `BlockedOnNotification` phải ghi waiter trên object (bất biến
        // `check_notifications`: `waiter` ⇔ `TcbState`); kiểm **trước** khi gỡ hàng chờ.
        // So khớp **trên `NonReadyTarget`** — `kobj` **không** được thấy `into_state` (private
        // của `state_write`).
        if let NonReadyTarget::BlockedOnNotification { ntfn } = to {
            let waiter = self.body(ntfn, |k| match k {
                Kind::Notification(n) => Ok(n.waiter),
                _ => Err(KError::WrongObjectType),
            })?;
            if let Some(w) = waiter {
                if w != cur {
                    return Err(KError::Busy);
                }
            }
        }
        self.unlink_tcb_queues(cur);
        self.transition_tcb(cur, to)?;
        if let NonReadyTarget::BlockedOnNotification { ntfn } = to {
            self.body_mut(ntfn, |k| match k {
                Kind::Notification(n) => {
                    n.waiter = Some(cur);
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
        }
        self.sched_after_op(false);
        Ok(cur)
    }

    /// `wake(tcb)` — model API `[TK]` (DoD 5): đánh thức TCB đang chặn/ngủ bằng cách gỡ
    /// khỏi hàng chờ của object (endpoint/Reply/notification/address) rồi `→ Ready`.
    /// **Không** phát lại Call/Send/Recv (như `Resume`). TCB `suspended` ⇒ vẫn `Ready` nhưng
    /// **không** vào ready queue (D2-33: `Ready + suspended`). TCB đã `Ready` ⇒ no-op.
    ///
    /// **KT-29 — wake chéo hart** (thiết kế §4.8 `:342`, D2-53 Q1): với mô hình nhiều hart,
    /// nếu `home(tcb) != current_hart()` thì **không** ghi trạng thái hart khác (luật 3) mà gửi
    /// `MailboxMsg::Wake(tcb)` vào hộp thư của `home`; hart đích đánh thức khi xử lý hộp thư và
    /// **chỉ preempt** nếu ưu tiên của TCB **>** `published.cur_prio` của đích. Cùng hart (và
    /// `n = 1`) giữ nguyên đường cũ.
    ///
    /// # CHỈ dùng cho test / ảnh test — sản phẩm dùng [`Machine::wake_from`]
    ///
    /// **KT-159 (N7, D2-144).** Đối xứng `vifast-core::Machine::wake`: hàm này **không** có call
    /// site sản phẩm nào (đo ở D2-144) — chỉ ảnh test và test sim gọi nó. Mọi đường đánh thức
    /// TCB trong sim (IPC/Reply/Notification/finalise/refill) đi qua [`Machine::wake_from`]
    /// hoặc [`Machine::wake_from_unlink`]. Luật **máy kiểm**:
    /// `v2/tools/check-wake-paths.py --strict` **đỏ** (`E_STRICT_TEST_ONLY_WAKE`) khi có lời gọi
    /// `wake(` trong `v2/kernel/vifast-core/src/` hoặc `v2/sim/vifast-sim/src/` ngoài
    /// `#[cfg(test)]`/`mod tests`.
    ///
    /// **Vì sao hai ngữ nghĩa — cố ý, không phải trùng lặp:** `wake()` = **post-only** khi khác
    /// hart (chỉ gửi `MailboxMsg::Wake` về `home`, **không** đổi trạng thái ngay — đúng luật 3,
    /// đúng cho người gọi không giữ bookkeeping nào); [`Machine::wake_from`] = **post + `Ready`
    /// ngay** (đường sản phẩm đã tự gỡ bookkeeping nên trạng thái phải nhất quán ngay, nếu hoãn
    /// sẽ vi phạm C2 `Reply.caller ⇔ BlockedOnReply` và `check_ipc`). Gọi `wake()` trong mã sản
    /// phẩm là **bẫy** — xem doc của `vifast-core::Machine::wake`.
    pub fn wake(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        if self.sched.len() > 1 && info.home != self.current_hart {
            return self.post_mail(info.home, MailboxMsg::Wake(tcb));
        }
        self.wake_step(tcb, true)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-155 (N7-2b)** — bản **đối xứng** của `vifast-core::Machine::wake_from` cho sim:
    /// cửa **duy nhất** cho mọi đường đánh thức TCB trong sim (IPC/Reply/Notification/
    /// finalise/refill). Khác hart ⇒ gửi `MailboxMsg::Wake` vào hộp thư `home` (đánh thức
    /// hart đích) **và** vẫn chuyển `Ready` cục bộ — cùng lý do như kernel: các đường gọi đã
    /// tự gỡ bookkeeping của mình nên trạng thái phải nhất quán **ngay**; hoãn sang hart
    /// `home` sẽ vi phạm bất biến C2 (`Reply.caller ⇔ BlockedOnReply`) và `check_ipc`.
    ///
    /// `unlink = false` (xem [`Machine::wake_step`]): **không** gỡ TCB khỏi mọi hàng chờ —
    /// các đường gọi đã gỡ đúng phần của mình, và `unlink_tcb_queues` sẽ gỡ **cả** message
    /// `NBSend` không liên quan rồi đếm lại `ipc_dropped` (đường finalise đã đếm ⇒ hai lần).
    pub(super) fn wake_from(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        let cheo_hart = self.sched.len() > 1 && info.home != self.current_hart;
        self.wake_step(tcb, false)?;
        if cheo_hart {
            self.push_mail(info.home, MailboxMsg::Wake(tcb))?;
        }
        Ok(())
    }

    /// **KT-155 (N7-4)** — như [`Machine::wake_from`] nhưng **gỡ** TCB khỏi mọi hàng chờ
    /// trước khi `Ready` (`unlink = true`). Dùng cho `wake_addr`: waiter `BlockedOnAddress`
    /// phải được gỡ khỏi bucket WaitAddr, và khác hart thì phải gửi `Wake` về `home` (đối
    /// xứng kernel; trước KT-155 `wake_addr` gọi `wake_step` trực tiếp — nợ N7-4).
    pub(super) fn wake_from_unlink(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        let cheo_hart = self.sched.len() > 1 && info.home != self.current_hart;
        self.wake_step(tcb, true)?;
        if cheo_hart {
            self.push_mail(info.home, MailboxMsg::Wake(tcb))?;
        }
        Ok(())
    }

    /// `yield_current(hart)` — model API `[TK]` (DoD 5): TCB đang chạy nhường CPU
    /// (`Running → Ready`, vào **cuối** hàng cùng mức) rồi `dispatch` chọn TCB kế tiếp.
    /// Trả `current` mới (có thể chính TCB vừa nhường nếu nó vẫn cao nhất và hàng rỗng).
    pub fn yield_current(&mut self, hart: u16) -> Result<Option<ObjId>, KError> {
        if hart as usize >= self.sched.len() {
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

    // ------------------------------------------- hộp thư theo hart (KT-22) `[TK]`
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-49(f)): mô hình **1 luồng tất định**, không
    // atomic/thread/loom. `post_mail` là **cạnh gửi** (kiểm sức chứa ⇒ `Busy`/`NotSupported`);
    // `process_mail` là **cuối bước** (drain FIFO, mỗi message áp dụng đúng một lần);
    // `send_mail` = gửi + xử lý trong một bước (DoD 6: không có "đã pop chưa áp dụng").

    /// Gửi một message vào hộp thư của hart (không chờ).
    ///
    /// - hart không tồn tại ⇒ `InvalidArgument`; `HART_COUNT == 1` (K1b, D2-49(a)) và message
    ///   `Migrate` ⇒ `NotSupported` — **không** im lặng bỏ (giữ nguyên ngữ nghĩa KT-22);
    /// - **KT-29**: với ≥ 2 hart, `Migrate { tcb, to_hart }` được kiểm **trước** khi vào hàng:
    ///   `to_hart` phải là hart có thật, TCB phải sống và `Ready` (không `suspended`),
    ///   `to_hart != home(tcb)`, và cả `home` lẫn `to_hart` nằm trong affinity của TCB; sai ⇒
    ///   `InvalidArgument` (message **không** vào hàng);
    /// - hộp thư đầy ⇒ `Busy` (backpressure §4.17) — message **không** bị ghi đè/mất, người gửi
    ///   xử lý hộp thư (hoặc gọi [`Machine::process_mail`]) rồi thử lại.
    pub fn post_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        let posted = self.push_mail(hart, msg);
        self.sched_after_op(false);
        posted
    }

    /// Thân của [`Machine::post_mail`] **không** tự kiểm bất biến — dùng cho đường nội bộ
    /// (shootdown của KT-40) để **một** bước chỉ kiểm bất biến một lần.
    ///
    /// **KT-40**: message `TlbFlush` được **kernel** gán `ack` tại đây (= `delivered` của hart
    /// đích sau khi tăng — Q5 "`seq` = số send Ok"); người gọi không tự đặt. Hộp thư đầy ⇒
    /// `Busy`, **không** tăng `delivered` (bất biến `delivered = acked + số message trong hàng`).
    pub(super) fn push_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        if let MailboxMsg::Migrate { tcb, to_hart } = msg {
            if self.mail.len() == 1 {
                return Err(KError::NotSupported);
            }
            // `Migrate` phải đi vào hộp thư của **hart đích** (thiết kế §4.8: X đẩy sang Y).
            if hart != to_hart {
                return Err(KError::InvalidArgument);
            }
            self.check_migrate_request(tcb, to_hart)?;
        }
        if self.mail[hart as usize].is_full() {
            return Err(KError::Busy);
        }
        let msg = match msg {
            MailboxMsg::TlbFlush(mut f) => {
                let ack = self.tlbs[hart as usize].deliver();
                f.ack = ack;
                MailboxMsg::TlbFlush(f)
            }
            other => other,
        };
        match self.mail[hart as usize].try_push(msg) {
            Ok(()) => Ok(()),
            Err(_) => {
                // `is_full` đã kiểm ở trên và mô hình một luồng ⇒ không thể xảy ra; nếu xảy ra
                // thì đó là lỗi mô hình, không im lặng.
                debug_assert!(false, "hộp thư hart {hart} đầy dù vừa kiểm");
                Err(KError::Busy)
            }
        }
    }

    /// **KT-29** — kiểm một yêu cầu `Migrate` **trước** khi vào hộp thư (cửa của `post_mail`):
    /// hart đích có thật, TCB sống/`Ready`/không treo, `to_hart != home`, `home` và `to_hart`
    /// đều trong affinity, và **không** đã có yêu cầu `Migrate` nào khác cho **cùng TCB** đang
    /// chờ trong bất kỳ hộp thư (bất biến (ii) `check_smp`: ≤ 1 yêu cầu/TCB — reviewer-kt29b).
    /// Vi phạm ⇒ `InvalidArgument` (message **không** vào hàng).
    pub(super) fn check_migrate_request(&self, tcb: ObjId, to_hart: u16) -> Result<(), KError> {
        if to_hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        let info = self.tcb_info_obj(tcb)?;
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
        let da_co = self.mail.iter().any(|hb| {
            hb.iter()
                .any(|m| matches!(m, MailboxMsg::Migrate { tcb: t, .. } if *t == tcb))
        });
        if da_co {
            return Err(KError::InvalidArgument);
        }
        Ok(())
    }

    /// Xử lý (drain) hộp thư của hart theo **FIFO**; mỗi message áp dụng **đúng một lần**:
    ///
    /// - `Wake(tcb)` — đánh thức TCB đang chặn/ngủ có `home == hart` (idempotent: TCB khác
    ///   `Ready` ⇒ no-op); **KT-29**: preempt nếu ưu tiên > `cur_prio` công bố;
    /// - `Reschedule` — `dispatch` hart một lượt;
    /// - `Stop` — bật cờ `stopped` của hart (mô hình; không đổi hàng chờ);
    /// - **KT-40** `TlbFlush(f)` — xả entry `(f.generation, f.asid)` phủ `f.va` của hart này rồi
    ///   `ack(f.ack)` theo kiểu **fetch_max** (mô hình của "IPI + `sfence.vma`"); xem
    ///   [`Machine::tlb_shootdown_begin`];
    /// - **KT-29** `Migrate { tcb, to_hart }` — di trú thật nếu TCB còn `Ready`/trong affinity;
    ///   `n = 1` thì `post_mail` đã từ chối (`NotSupported`).
    ///
    /// Trả số message đã xử lý. Hộp thư rỗng sau khi xong (drain tới cạn) ⇒ không có trạng thái
    /// trung gian "message đã pop nhưng chưa áp dụng".
    pub fn process_mail(&mut self, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        let (n, _) = self.drain_mail_fifo(hart);
        self.sched_after_op(false);
        Ok(n)
    }

    /// `send_mail` = [`Machine::post_mail`] + [`Machine::process_mail`] trong **một** bước:
    /// qua được backpressure thì message được xử lý ngay, không quan sát được hàng đợi giữa
    /// chừng (DoD 6). Lỗi `Busy`/`NotSupported` của `post_mail` giữ nguyên, **không** drain.
    pub fn send_mail(&mut self, hart: u16, msg: MailboxMsg) -> Result<(), KError> {
        self.post_mail(hart, msg)?;
        self.process_mail(hart)?;
        Ok(())
    }

    /// Số message đang nằm trong hộp thư của hart (bất biến: `≤ MAILBOX_CAP`).
    pub fn mail_len(&self, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].len())
    }

    /// **KT-60** — đọc **nội dung** hàng chờ hộp thư hart (chỉ đọc; đóng nợ §7.5 "oracle
    /// không có accessor công khai cho message đang treo"). Thứ tự FIFO = thứ tự drain.
    pub fn mail_items(&self, hart: u16) -> Result<Vec<MailboxMsg>, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].iter().copied().collect())
    }

    /// Nhật ký message **đã xử lý** của hộp thư hart, đúng thứ tự drain (nguồn kiểm FIFO).
    pub fn mail_log(&self, hart: u16) -> Result<&[MailboxMsg], KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].log())
    }

    /// Hart đã nhận message `Stop` chưa (mô hình `[TK]`; không tắt scheduler — K1b chưa có
    /// quy trình dừng hart).
    pub fn hart_stopped(&self, hart: u16) -> Result<bool, KError> {
        if hart as usize >= self.mail.len() {
            return Err(KError::InvalidArgument);
        }
        Ok(self.mail[hart as usize].stopped())
    }

    /// Áp dụng **một** message lên mô hình — infallible (message không hợp lệ là **no-op** hoặc
    /// `debug_assert`), để drain không để lại trạng thái nửa vời.
    ///
    /// **KT-29**: `Wake` chỉ áp dụng cho TCB có `home == hart` (đúng hart đích; message `Wake`
    /// gửi sai hart là no-op) và — với ≥ 2 hart — **preempt** hart đích khi ưu tiên TCB được
    /// đánh thức **>** `cur_prio` công bố (thiết kế §4.8 `:342`, DoD 5). `Migrate` di trú thật
    /// khi hart đích xử lý (xem [`Machine::apply_migrate`]).
    pub(super) fn apply_mail(&mut self, hart: u16, msg: MailboxMsg) {
        match msg {
            MailboxMsg::Wake(tcb) => {
                let info = match self.tcb_info_obj(tcb) {
                    Ok(i) => i,
                    Err(_) => return,
                };
                if info.home != hart {
                    return;
                }
                if info.state.is_blocked() || matches!(info.state, TcbState::Sleeping { .. }) {
                    let _ = self.wake_step(tcb, true);
                }
                if self.sched.len() > 1 {
                    let eff = match self.effective_priority(tcb) {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    if eff > self.sched[hart as usize].cur_prio()
                        && self.dispatch_step(hart).is_err()
                    {
                        debug_assert!(false, "Wake: preempt lỗi (bất biến scheduler vỡ)");
                    }
                }
            }
            MailboxMsg::Reschedule => {
                if self.dispatch_step(hart).is_err() {
                    debug_assert!(false, "Reschedule: dispatch lỗi (bất biến scheduler vỡ)");
                }
            }
            MailboxMsg::Stop => self.mail[hart as usize].set_stopped(),
            MailboxMsg::TlbFlush(f) => {
                // **KT-40** — handler "IPI": xả entry `(generation, asid)` phủ `va` rồi **ack**
                // thế hệ `f.ack` của hộp thư này (`fetch_max`, không lùi).
                let _ = self
                    .tlb_at_mut(hart)
                    .map(|t| t.flush(f.generation, f.asid, TlbFlushScope::Page { va: f.va }));
                let newly = self.tlb_at_mut(hart).map(|t| t.ack(f.ack)).unwrap_or(0);
                self.tlb_ack_total += newly;
            }
            MailboxMsg::Migrate { tcb, to_hart } => {
                if to_hart != hart {
                    debug_assert!(false, "Migrate nằm sai hộp thư hart đích");
                    return;
                }
                self.apply_migrate(tcb, to_hart);
            }
        }
    }

    /// **KT-29** — thân di trú một TCB sang `to` (đã qua `check_migrate_request` khi vào hàng;
    /// trạng thái có thể đổi giữa lúc gửi và lúc xử lý nên kiểm lại): TCB còn sống, `Ready`,
    /// không treo, `home != to`, `to` trong affinity ⇒ gỡ khỏi hàng nguồn, đặt `home = to`,
    /// enqueue **cuối** hàng mức hiện tại của hart đích, `migrate_count += 1`. Không thoả ⇒
    /// no-op (message vẫn được đếm đã xử lý; không mất TCB).
    pub(super) fn apply_migrate(&mut self, tcb: ObjId, to: u16) -> bool {
        let info = match self.tcb_info_obj(tcb) {
            Ok(i) => i,
            Err(_) => return false,
        };
        if info.state != TcbState::Ready
            || info.suspended
            || info.home == to
            || to as usize >= self.sched.len()
            || info.affinity & (1u64 << to) == 0
        {
            return false;
        }
        self.sched_remove_all(tcb);
        let doi = self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.home = to;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        });
        if doi.is_err() {
            return false;
        }
        if self.sched_sync(tcb).is_err() {
            return false;
        }
        self.migrate_count += 1;
        true
    }

    /// **KT-29** — một vòng **push migration** tất định (thiết kế §4.8 `:338-342`, D2-53 Q2).
    ///
    /// Với **mỗi** hart nguồn `x` theo thứ tự tăng dần: nếu hart `x` có **> 1** TCB `Ready` ở
    /// mức cao nhất và tồn tại hart `y != x` **trong affinity** của TCB **cuối hàng** mức đó mà
    /// `published(y).idle` ⇒ chọn `y` **nhỏ nhất** và đẩy **đúng một** TCB (TCB cuối hàng mức
    /// Ready cao nhất) qua `MailboxMsg::Migrate` vào hộp thư `y`. Cuối vòng, mỗi hart nhận
    /// `Migrate` xử lý message **trong cùng bước** (không có trạng thái in-flight — rủi ro (a)
    /// K1C-PLAN §5 KT-29): gỡ khỏi hàng nguồn, `home = y`, enqueue **cuối** hàng `y`,
    /// `migrate_count += 1`.
    ///
    /// Không work stealing: chỉ hart **nguồn** quyết định đẩy; hart đích không tự kéo. Không
    /// bao giờ chọn `current`/TCB `Running` (chỉ lấy từ ready queue). Hộp thư đầy ⇒ **bỏ** yêu
    /// cầu (backpressure, không mất TCB). Trả số TCB di trú thành công.
    pub(super) fn push_step(&mut self) -> usize {
        let n = self.sched.len();
        if n <= 1 {
            return 0;
        }
        let mut nhan = vec![false; n];
        for x in 0..n {
            let Some(top) = self.sched[x].highest_priority() else {
                continue;
            };
            // Chốt Q2: chỉ đẩy khi **cùng mức Ready cao nhất** có > 1 TCB (giữ ít nhất một).
            if self.sched[x].len_at(top) < 2 {
                continue;
            }
            let Some(cand) = self.sched[x].last_at(top) else {
                continue;
            };
            let Ok(info) = self.tcb_info_obj(cand) else {
                continue;
            };
            let mut dest = None;
            // KT-242: slicing giữ đúng hành vi cũ (panic nếu `published.len() < n`).
            for (y, pub_slot) in self.published[..n].iter().enumerate() {
                if y == x || info.affinity & (1u64 << y) == 0 {
                    continue;
                }
                if pub_slot.idle {
                    dest = Some(y as u16);
                    break;
                }
            }
            let Some(y) = dest else {
                continue;
            };
            let msg = MailboxMsg::Migrate {
                tcb: cand,
                to_hart: y,
            };
            if self.mail[y as usize].try_push(msg).is_ok() {
                nhan[y as usize] = true;
            }
        }
        let mut applied = 0usize;
        for (y, co_nhan) in nhan.iter().enumerate() {
            if !*co_nhan {
                continue;
            }
            // **Drain FIFO** (reviewer-kt29 mục 1): `Migrate` không bị rút từ giữa hàng; mọi
            // message đến trước (Wake/Reschedule/Stop) được xử lý + ghi log **trước** nó, đúng
            // thứ tự KT-22. `Migrate` được đẩy vào **cuối** hàng nên drain tới cạn là đủ.
            applied += self.drain_mail_fifo(y as u16).1;
        }
        applied
    }

    /// **KT-29** — drain hộp thư hart theo **FIFO** (dùng chung cho [`Machine::process_mail`]
    /// và cuối vòng push): pop từng message, áp dụng đúng một lần, ghi log theo thứ tự; trả
    /// `(số message đã xử lý, số TCB di trú thành công)`.
    pub(super) fn drain_mail_fifo(&mut self, hart: u16) -> (usize, usize) {
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

    /// **KT-29** — chạy **một** vòng push migration công khai (model API `[TK]`, không phải
    /// ABI). `n = 1` ⇒ `Ok(0)` (không có hart đích; `Migrate` vẫn `NotSupported`). Dùng cho
    /// test tất định; đường tự động là `dispatch` và `preempt_every_step`.
    pub fn push_migration(&mut self) -> Result<usize, KError> {
        self.refresh_published();
        let n = self.push_step();
        self.sched_after_op(false);
        Ok(n)
    }

    /// Đồng bộ **một** TCB với ready queue sau khi `state`/`suspended`/`priority`/`home`/
    /// `affinity` đổi. Idempotent: gỡ bản cũ rồi đặt chỗ mới theo state hiện tại.
    pub(super) fn sched_sync(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        // KT-21: bucket dùng **priority hiệu lực** (`donated_priority` khi nhận donation, ngược
        // lại priority gốc) — giống hệt `check_sched`.
        let prio = self.effective_priority(tcb)?;
        // 1) gỡ khỏi bucket priority hiện tại (bất biến: bucket == priority hiệu lực).
        for h in 0..self.sched.len() {
            self.sched[h].remove_at(tcb, prio);
        }
        // 2) nhả `current` nếu TCB không còn `Running` đúng hart đó.
        for h in 0..self.sched.len() {
            let giu = matches!(info.state, TcbState::Running { hart } if hart as usize == h);
            if !giu && self.sched[h].current() == Some(tcb) {
                self.sched[h].clear_current();
            }
        }
        // 3) đặt chỗ mới: Ready (không treo, home trong affinity) hoặc Running.
        match info.state {
            TcbState::Ready if !info.suspended && info.affinity & (1u64 << info.home) != 0 => {
                let h = info.home as usize;
                if h >= self.sched.len() {
                    return Err(KError::InvalidArgument);
                }
                self.sched[h].enqueue(tcb, prio);
            }
            TcbState::Running { hart } => {
                let h = hart as usize;
                if h >= self.sched.len() {
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

    /// Gỡ `tcb` khỏi **mọi** bucket của mọi hart — gọi **trước** khi sửa
    /// `priority`/`home`/`affinity` để bucket luôn khớp priority.
    pub(super) fn sched_remove_all(&mut self, tcb: ObjId) {
        for h in 0..self.sched.len() {
            self.sched[h].remove_all(tcb);
        }
    }

    /// KT-19 DoD 3+4: sau **mỗi** public op — nếu bật `preempt_every_step` thì ép preempt
    /// (`dispatch` đưa `Running` về `Ready` rồi chọn lại), rồi tự kiểm bất biến
    /// [`Machine::check_sched`]. Vi phạm là lỗi mô hình: panic ở build có debug assertion.
    ///
    /// **KT-29**: (i) làm tươi `published` từ scheduler; (ii) khi `preempt_every_step` bật, sau
    /// vòng dispatch chạy **một** vòng push migration (DoD 3 — "sau mỗi op khi
    /// `preempt_every_step` bật"); (iii) với `n > 1` kiểm thêm [`Machine::check_smp`]. `n = 1`
    /// giữ nguyên chi phí/ngữ nghĩa cũ (check_smp trùng `check_sched` nên chỉ chạy khi n > 1).
    pub(super) fn sched_after_op(&mut self, preempt: bool) {
        if preempt && self.preempt_every_step {
            for h in 0..self.sched.len() {
                if self.dispatch_step(h as u16).is_err() {
                    debug_assert!(false, "preempt_every_step: dispatch lỗi");
                    return;
                }
            }
            self.refresh_published();
            if self.sched.len() > 1 {
                let _ = self.push_step();
            }
        }
        self.refresh_published();
        if let Err(e) = self.check_sched() {
            debug_assert!(false, "bất biến scheduler vỡ sau public op: {e}");
        }
        if self.sched.len() > 1 {
            if let Err(e) = self.check_smp() {
                debug_assert!(false, "bất biến SMP vỡ sau public op: {e}");
            }
        }
        if let Err(e) = self.check_mailboxes() {
            debug_assert!(false, "bất biến mailbox vỡ sau public op: {e}");
        }
        if let Err(e) = self.check_ipc() {
            debug_assert!(
                false,
                "bất biến IPC (bảo toàn message) vỡ sau public op: {e}"
            );
        }
        // **KT-34** — bất biến liên kết Reply (C2): `BlockedOnReply ⇔ Reply.caller == Some(tcb)`.
        if let Err(e) = self.check_reply_links() {
            debug_assert!(
                false,
                "bất biến Reply (C2: caller ⇔ BlockedOnReply) vỡ sau public op: {e}"
            );
        }
        if let Err(e) = self.check_notifications() {
            debug_assert!(false, "bất biến Notification vỡ sau public op: {e}");
        }
        if let Err(e) = self.check_waitaddr() {
            debug_assert!(false, "bất biến WaitOnAddress vỡ sau public op: {e}");
        }
        // **KT-36** — bất biến Frame/cửa sổ PA cũng chạy sau **mỗi** public op ("check_frames
        // sớm"): Frame mồ côi, `pa` lệch căn/ngoài cửa sổ, frame chết còn mapping, watermark
        // sai đều bị bắt ngay tại op gây ra, không đợi test gọi tay.
        if let Err(e) = self.check_frames() {
            debug_assert!(false, "bất biến Frame vỡ sau public op: {e}");
        }
        // **KT-37** — cây PageTable + bộ đếm lá/Frame cũng chạy sau **mỗi** public op
        // ("check_pages sớm"); cửa sổ nhanh `page_table_objects == 0` giữ chi phí per-op của
        // mọi test K1a..K1c như trước KT-37.
        if let Err(e) = self.check_pages() {
            debug_assert!(false, "bất biến PageTable vỡ sau public op: {e}");
        }
        // **KT-39** — W^X toàn cục theo Frame (bất biến `!(w>0 && x>0)`, `w+x ≤ maps`) chạy
        // sau **mỗi** public op ("check_wx sớm"); cửa sổ nhanh `frame_objects == 0` giữ chi phí
        // per-op của mọi test K1a..K1c như trước KT-39.
        if let Err(e) = self.check_wx() {
            debug_assert!(false, "bất biến W^X vỡ sau public op: {e}");
        }
        // **KT-40** — TLB-safety (mọi entry còn sống khớp cây PageTable) chạy sau **mỗi**
        // public op; cửa sổ nhanh `tlb_fill_total == 0 && tlb_msg_total == 0` giữ chi phí của
        // mọi test K1a..K1c/KT-36..39 như trước KT-40.
        if let Err(e) = self.check_tlb() {
            debug_assert!(false, "bất biến TLB vỡ sau public op: {e}");
        }
        // **KT-41** — fault không mất/không nhân bản (Exit/Vm/UnknownSyscall/UserException đi
        // **cùng một** đường fault); cửa sổ nhanh `fault_events == 0` giữ chi phí per-op của mọi
        // test trước KT-41.
        if let Err(e) = self.check_fault() {
            debug_assert!(false, "bất biến fault vỡ sau public op: {e}");
        }
        // **KT-38** — VSpace + `active_on`/hart online và ASID/free-list/thế hệ cũng chạy sau
        // **mỗi** public op ("check_vspace/check_asid sớm"); hai cửa sổ nhanh giữ chi phí per-op
        // của mọi test K1a..K1c như trước KT-38.
        if let Err(e) = self.check_vspace() {
            debug_assert!(false, "bất biến VSpace vỡ sau public op: {e}");
        }
        if let Err(e) = self.check_asid() {
            debug_assert!(false, "bất biến ASID vỡ sau public op: {e}");
        }
    }

    /// KT-22 — kiểm **toàn bộ** bất biến hộp thư (chạy sau **mỗi** public op qua
    /// [`Machine::sched_after_op`], cùng chỗ với `check_sched`):
    ///
    /// 1. mỗi hộp thư hart: `pushed == popped + removed + len`, `len ≤ cap`;
    /// 2. mỗi Endpoint: sức chứa **cố định** [`MAILBOX_CAP`], cùng bất biến trên, và "có message
    ///    ⇒ không có Recv waiter";
    /// 3. mọi message trong hàng gửi có sender **còn sống**; message **chặn** (`Send`/`Call`
    ///    phải xếp hàng) có sender đúng `BlockedOnSend{ep, call, fault}`; message **không chặn**
    ///    (`NBSend`/`Call no_reply=1` — KT-24) chỉ cần sender còn sống vì người gửi vẫn
    ///    `Ready`/`Running`; fault `Exit` không có `msg_id`, không bao giờ `call = true`.
    ///
    /// **KT-24** đã nới bất biến (3) của KT-22 (trước đây mã hoá `call == fault.is_none()` vì
    /// K1b chỉ có `Call` + fault `Exit`): nay `call` là trường của **message** (`Call` chờ Reply)
    /// và `blocking` là trường của **đường gửi** (chặn/không chặn), không suy từ `fault`.
    ///
    /// Không dùng `HashMap`/`format!` trên đường nóng trừ khi **có** vi phạm (thông điệp lỗi).
    pub fn check_mailboxes(&self) -> Result<(), String> {
        for (h, hb) in self.mail.iter().enumerate() {
            if !hb.invariant() {
                return Err(format!(
                    "hộp thư hart {h}: bất biến pushed/popped/removed vỡ"
                ));
            }
            // **KT-40** — ack theo thế hệ của `TlbFlush` (D2-60 semantics): `delivered` = số
            // message đã giao (tăng ở `push_mail`, **chỉ khi** có slot), `acked` = fetch_max đã
            // xử lý. Hàng FIFO + ack cấp tăng dần ⇒ `delivered - acked` **phải bằng** số
            // `TlbFlush` còn trong hàng (không mất, không ack khống, không ack lùi).
            let trong_hang = hb
                .iter()
                .filter(|m| matches!(m, MailboxMsg::TlbFlush(_)))
                .count() as u64;
            let t = &self.tlbs[h];
            if t.acked > t.delivered {
                return Err(format!(
                    "hộp thư hart {h}: acked {} > delivered {} (ack khống)",
                    t.acked, t.delivered
                ));
            }
            if t.delivered - t.acked != trong_hang {
                return Err(format!(
                    "hộp thư hart {h}: delivered={} acked={} nhưng có {trong_hang} TlbFlush trong hàng",
                    t.delivered, t.acked
                ));
            }
        }
        // Bảng sống theo `ObjId` để tra sender O(1) thay vì quét object (giống `check_sched`).
        let mut song: Vec<bool> = vec![false; self.objects.len() + 1];
        for (i, m) in self.objects.iter().enumerate() {
            song[i + 1] = m.alive;
        }
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            let (cap, len, pushed, popped, removed, items, waiter) = self
                .body(ep, |k| match k {
                    Kind::Endpoint(e) => Ok((
                        e.send.capacity(),
                        e.send.len(),
                        e.send.pushed(),
                        e.send.popped(),
                        e.send.removed(),
                        e.send
                            .iter()
                            .map(|q| {
                                (
                                    q.sender,
                                    q.call,
                                    q.fault,
                                    q.blocking,
                                    q.msg.as_ref().map(|m| (m.msg_id, m.info.length())),
                                )
                            })
                            .collect::<Vec<_>>(),
                        e.recv.map(|(t, _)| t),
                    )),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("Endpoint {ep}: không đọc được body: {e:?}"))?;
            if cap != MAILBOX_CAP {
                return Err(format!("Endpoint {ep}: sức chứa {cap} != {MAILBOX_CAP}"));
            }
            if len > cap {
                return Err(format!("Endpoint {ep}: len {len} > cap {cap}"));
            }
            if pushed != popped + removed + len as u64 {
                return Err(format!(
                    "Endpoint {ep}: message mất/thêm (pushed={pushed} popped={popped} removed={removed} len={len})"
                ));
            }
            if len > 0 && waiter.is_some() {
                return Err(format!(
                    "Endpoint {ep}: có {len} message mà vẫn có Recv waiter"
                ));
            }
            for (sender, call, fault, blocking, msg) in items {
                if !song.get(sender as usize).copied().unwrap_or(false) {
                    return Err(format!(
                        "Endpoint {ep}: sender {sender} đã chết nhưng còn trong hàng"
                    ));
                }
                match msg {
                    Some((msg_id, _len)) => {
                        if msg_id == 0 {
                            return Err(format!(
                                "Endpoint {ep}: message người dùng chưa được cấp msg_id"
                            ));
                        }
                    }
                    None => {
                        if fault.is_none() {
                            return Err(format!(
                                "Endpoint {ep}: message không có nội dung và không phải fault"
                            ));
                        }
                    }
                }
                // **KT-26**: fault `Exit` **không bao giờ** là `Call` (D2-25: đi qua hàng gửi,
                // không Reply); fault ≠ Exit **phải** là `Call` (thiết kế §4.6 `:321`: procsrv
                // nhận qua Call rồi Reply để chạy tiếp). Hai chiều đều cấm nửa vời.
                match fault {
                    None => {}
                    Some(kind) if kind.is_exit() && call => {
                        return Err(format!("Endpoint {ep}: fault Exit nhưng cờ call=true"));
                    }
                    Some(kind) if !kind.is_exit() && !call => {
                        return Err(format!(
                            "Endpoint {ep}: fault {} phải là Call (có Reply)",
                            kind.name()
                        ));
                    }
                    Some(_) => {}
                }
                // Người gửi **chặn** ⇒ phải đang `BlockedOnSend` khớp đúng hàng/cờ/fault.
                // Người gửi **không chặn** (`NBSend`/`Call no_reply=1`) ⇒ chỉ cần còn sống.
                if blocking {
                    match self.tcb_state(sender) {
                        Ok(TcbState::BlockedOnSend {
                            ep: hang_ep,
                            call: hang_call,
                            fault: hang_fault,
                        }) => {
                            if hang_ep != ep || hang_call != call || hang_fault != fault {
                                return Err(format!(
                                    "Endpoint {ep}: sender {sender} ở trạng thái chặn không khớp message"
                                ));
                            }
                        }
                        other => {
                            return Err(format!(
                                "Endpoint {ep}: sender {sender} ở {other:?} nhưng còn trong hàng"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Kiểm **toàn bộ** bất biến scheduler (DoD 3):
    ///
    /// 1. không TCB nào trùng trong hàng; bucket khớp `priority`;
    /// 2. mọi TCB trong hàng tồn tại, `Ready`, **không** `suspended`, home trong affinity;
    /// 3. `Ready` (không treo) ⇔ trong hàng; `Running{hart}` ⇔ `current`; trạng thái khác
    ///    **không** nằm trong hàng; ≤ 1 `Running`/hart;
    /// 4. `len`/bitmap khớp; `idle ⇔ current == None`; `cur_prio` khớp priority của `current`.
    ///
    /// Dùng **bảng đánh dấu theo `ObjId`** (`ObjId` 1-based, tối đa `objects.len()`) nên mỗi
    /// TCB tra O(1) thay vì quét 256 hàng — kiểm được sau **mỗi** op mà vẫn rẻ.
    pub fn check_sched(&self) -> Result<(), String> {
        if self.sched.is_empty() {
            return Err("mô hình không có hart nào".to_string());
        }
        // `None` = không nằm trong hàng; `Some(h)` = đang ở ready queue của hart `h`.
        let mut hang_cua: Vec<Option<usize>> = vec![None; self.objects.len() + 1];
        for (h, hs) in self.sched.iter().enumerate() {
            hs.check_local()?;
            hs.for_each_queued(|prio, tcb| {
                let o = hang_cua
                    .get_mut(tcb as usize)
                    .ok_or_else(|| format!("TCB {tcb} trong hàng nhưng ngoài bảng object"))?;
                if o.is_some() {
                    return Err(format!("TCB {tcb} trùng trong ready queue"));
                }
                *o = Some(h);
                let info = self
                    .tcb_info_obj(tcb)
                    .map_err(|e| format!("TCB {tcb} trong hàng nhưng không đọc được: {e:?}"))?;
                if info.state != TcbState::Ready {
                    return Err(format!(
                        "TCB {tcb} trong hàng nhưng state {}",
                        info.state.name()
                    ));
                }
                if info.suspended {
                    return Err(format!("TCB {tcb} suspended nhưng vẫn nằm trong hàng"));
                }
                // KT-21: bucket phải khớp **priority hiệu lực** (priority gốc khi không nhận
                // donation, `donated_priority` khi đang nhận).
                let eff = self
                    .effective_priority(tcb)
                    .map_err(|e| format!("TCB {tcb}: priority hiệu lực lỗi {e:?}"))?;
                if eff != prio {
                    return Err(format!(
                        "TCB {tcb} ở bucket {prio} nhưng priority hiệu lực {eff}"
                    ));
                }
                if info.affinity & (1u64 << info.home) == 0 {
                    return Err(format!(
                        "TCB {tcb} home {} không nằm trong affinity",
                        info.home
                    ));
                }
                Ok(())
            })?;
            match hs.current() {
                Some(cur) => {
                    let info = self
                        .tcb_info_obj(cur)
                        .map_err(|e| format!("current {cur} của hart {h} không đọc được: {e:?}"))?;
                    match info.state {
                        TcbState::Running { hart } if hart as usize == h => {}
                        other => {
                            return Err(format!(
                                "current của hart {h} là TCB {cur} nhưng state {}",
                                other.name()
                            ));
                        }
                    }
                    if hang_cua.get(cur as usize).copied().flatten().is_some() {
                        return Err(format!("TCB {cur} Running nhưng vẫn nằm trong hàng"));
                    }
                    let eff = self
                        .effective_priority(cur)
                        .map_err(|e| format!("current {cur}: priority hiệu lực lỗi {e:?}"))?;
                    if eff != hs.cur_prio() {
                        return Err(format!(
                            "cur_prio {} nhưng priority hiệu lực của TCB {cur} là {eff}",
                            hs.cur_prio()
                        ));
                    }
                    if hs.idle() {
                        return Err(format!("hart {h} có current nhưng idle=true"));
                    }
                }
                None => {
                    if !hs.idle() {
                        return Err(format!("hart {h} không có current nhưng idle=false"));
                    }
                }
            }
        }
        // Mọi TCB sống: Ready ⇔ trong hàng (đúng hart home); Running ⇔ current; còn lại
        // **không** được nằm trong hàng.
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB sống {obj} không đọc được: {e:?}"))?;
            let hang = hang_cua.get(obj as usize).copied().flatten();
            // KT-23: phần "Running còn budget" **tách sang** `check_budget` (DoD 2 — kiểm riêng
            // từng miền bất biến); `check_sched` vẫn gọi `check_budget` ở cuối nên sau mỗi op
            // bất biến vẫn được kiểm đủ.
            match info.state {
                TcbState::Ready => {
                    if info.suspended {
                        if hang.is_some() {
                            return Err(format!("TCB {obj} Ready+suspended nhưng nằm trong hàng"));
                        }
                    } else if hang != Some(info.home as usize) {
                        return Err(format!(
                            "TCB {obj} Ready nhưng không nằm trong ready queue (hart home {})",
                            info.home
                        ));
                    }
                }
                TcbState::Running { hart } => {
                    if self.sched.get(hart as usize).and_then(|x| x.current()) != Some(obj) {
                        return Err(format!(
                            "TCB {obj} Running{{hart={hart}}} nhưng không phải current"
                        ));
                    }
                    if hang.is_some() {
                        return Err(format!("TCB {obj} Running nhưng vẫn nằm trong hàng"));
                    }
                }
                _ => {
                    if hang.is_some() {
                        return Err(format!(
                            "TCB {obj} state {} nhưng vẫn nằm trong hàng",
                            info.state.name()
                        ));
                    }
                }
            }
        }
        // KT-23: hai miền bất biến còn lại tách thành hàm riêng (`check_sc_refs`: tham chiếu
        // `SchedContext`/donation; `check_budget`: ngân sách) để DoD 2 kiểm **từng miền**;
        // `check_sched` vẫn gọi cả hai nên sau **mỗi** public op bất biến vẫn được kiểm đủ
        // (không nới lỏng so với KT-20/KT-21).
        self.check_sc_refs()?;
        self.check_budget()?;
        Ok(())
    }

    /// **KT-29** — bất biến **SMP** sau **mỗi** op (K1C-PLAN §5 KT-29 DoD 6), tách khỏi
    /// `check_sched` để kiểm **riêng** miền hart (DoD 2 kiểu KT-23). Năm phần:
    ///
    /// 1. **không TCB nào `Running` trên 2 hart** (và `current` mỗi hart trùng `Running{hart}`);
    /// 2. **bảo toàn TCB**: số TCB `Ready` (không treo)/`Running` = số TCB trong 256 hàng +
    ///    `current` + số yêu cầu `Migrate` đang chờ. Mô hình **không** có in-flight (rủi ro (a)
    ///    KT-29): message `Migrate` là **yêu cầu** còn TCB vẫn nằm hàng nguồn, nên hạng tử
    ///    mailbox được đếm ở **cả hai vế** (đẳng thức rút gọn `hàng + current = Ready/Running`,
    ///    vẫn bắt thừa/thiếu/nhân bản); nội dung từng yêu cầu được kiểm riêng;
    /// 3. `home` khớp hart đang giữ `current`/hàng;
    /// 4. `published.idle ⇔ current == None` và `published.cur_prio == cur_prio` thật;
    /// 5. không TCB nào nằm trong hàng của hart **ngoài** affinity của nó; `home` luôn trong
    ///    affinity; mỗi TCB tối đa một chỗ (một hàng hoặc một `current`).
    pub fn check_smp(&self) -> Result<(), String> {
        let n = self.sched.len();
        if n == 0 {
            return Err("mô hình không có hart nào".to_string());
        }
        if n > MAX_HARTS as usize {
            return Err(format!("số hart {n} vượt MAX_HARTS {MAX_HARTS}"));
        }
        if self.mail.len() != n || self.published.len() != n {
            return Err(format!(
                "số hộp thư {} / published {} khác số hart {n}",
                self.mail.len(),
                self.published.len()
            ));
        }
        // (4) published phải là ảnh trung thực của scheduler (idle ⇔ current == None).
        for h in 0..n {
            let p = self.published[h];
            let idle = self.sched[h].idle();
            let prio = self.sched[h].cur_prio();
            if p.idle != idle {
                return Err(format!(
                    "hart {h}: published.idle={} nhưng current={:?}",
                    p.idle,
                    self.sched[h].current()
                ));
            }
            if p.cur_prio != prio {
                return Err(format!(
                    "hart {h}: published.cur_prio={} nhưng cur_prio thật {prio}",
                    p.cur_prio
                ));
            }
        }
        // (5) + (3): mỗi TCB trong hàng thuộc đúng hart home, trong affinity, không trùng.
        let mut hang_cua: Vec<Option<usize>> = vec![None; self.objects.len() + 1];
        let mut queued_total = 0usize;
        for h in 0..n {
            self.sched[h].for_each_queued(|_prio, tcb| {
                let info = self
                    .tcb_info_obj(tcb)
                    .map_err(|e| format!("TCB {tcb} trong hàng hart {h} không đọc được: {e:?}"))?;
                if info.state != TcbState::Ready || info.suspended {
                    return Err(format!(
                        "TCB {tcb} trong hàng hart {h} nhưng state {} (suspended={})",
                        info.state.name(),
                        info.suspended
                    ));
                }
                if info.home as usize != h {
                    return Err(format!(
                        "TCB {tcb} home {} nhưng nằm trong hàng hart {h}",
                        info.home
                    ));
                }
                if info.affinity & (1u64 << h) == 0 {
                    return Err(format!("TCB {tcb} ở hart {h} ngoài affinity"));
                }
                let slot = hang_cua
                    .get_mut(tcb as usize)
                    .ok_or_else(|| format!("TCB {tcb} ngoài bảng object"))?;
                if slot.replace(h).is_some() {
                    return Err(format!("TCB {tcb} nằm trong hàng nhiều hart"));
                }
                queued_total += 1;
                Ok(())
            })?;
        }
        // (1) + (3): `current` mỗi hart đúng `Running{hart}`, `home == hart`, không trùng.
        let mut running_on: Vec<Option<usize>> = vec![None; self.objects.len() + 1];
        let mut current_total = 0usize;
        for h in 0..n {
            let Some(cur) = self.sched[h].current() else {
                continue;
            };
            let info = self
                .tcb_info_obj(cur)
                .map_err(|e| format!("current {cur} của hart {h} không đọc được: {e:?}"))?;
            if !matches!(info.state, TcbState::Running { hart } if hart as usize == h) {
                return Err(format!(
                    "current của hart {h} là TCB {cur} state {}",
                    info.state.name()
                ));
            }
            if info.home as usize != h {
                return Err(format!(
                    "TCB {cur} Running{{hart={h}}} nhưng home {}",
                    info.home
                ));
            }
            let slot = running_on
                .get_mut(cur as usize)
                .ok_or_else(|| format!("current {cur} ngoài bảng object"))?;
            if slot.replace(h).is_some() {
                return Err(format!("TCB {cur} Running trên nhiều hart"));
            }
            if hang_cua.get(cur as usize).copied().flatten().is_some() {
                return Err(format!("TCB {cur} Running nhưng vẫn nằm trong hàng"));
            }
            current_total += 1;
        }
        // (2): mọi TCB sống — `Ready` (không treo) ⇔ đúng một chỗ; `Running` ⇔ `current`.
        let mut ready_or_running = 0usize;
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB sống {obj} không đọc được: {e:?}"))?;
            let hang = hang_cua.get(obj as usize).copied().flatten();
            match info.state {
                TcbState::Ready => {
                    if info.suspended {
                        if hang.is_some() {
                            return Err(format!("TCB {obj} Ready+suspended nhưng nằm trong hàng"));
                        }
                    } else {
                        ready_or_running += 1;
                        if hang != Some(info.home as usize) {
                            return Err(format!(
                                "TCB {obj} Ready nhưng không nằm trong hàng hart home {}",
                                info.home
                            ));
                        }
                    }
                }
                TcbState::Running { hart } => {
                    ready_or_running += 1;
                    if running_on.get(obj as usize).copied().flatten() != Some(hart as usize) {
                        return Err(format!(
                            "TCB {obj} Running{{hart={hart}}} nhưng không phải current"
                        ));
                    }
                }
                _ => {
                    if hang.is_some() {
                        return Err(format!(
                            "TCB {obj} state {} nhưng vẫn nằm trong hàng",
                            info.state.name()
                        ));
                    }
                }
            }
        }
        // (2) phần mailbox: đếm yêu cầu `Migrate` đang chờ + kiểm nội dung từng yêu cầu.
        // Mô hình **không in-flight** nên hạng tử `pending` triệt tiêu ở hai vế; phần kiểm
        // thật sự là **nội dung** yêu cầu và "≤ 1 yêu cầu/TCB" (reviewer-kt29 mục 3).
        let mut pending = 0usize;
        let mut yeu_cau_cua: Vec<u8> = vec![0; self.objects.len() + 1];
        for (y, hb) in self.mail.iter().enumerate() {
            for msg in hb.iter() {
                let MailboxMsg::Migrate { tcb, to_hart } = *msg else {
                    continue;
                };
                pending += 1;
                if to_hart as usize != y {
                    return Err(format!(
                        "Migrate {{ tcb: {tcb}, to_hart: {to_hart} }} nằm trong hộp thư hart {y}"
                    ));
                }
                if to_hart as usize >= n {
                    return Err(format!("Migrate tới hart {to_hart} ngoài mô hình"));
                }
                let info = self
                    .tcb_info_obj(tcb)
                    .map_err(|e| format!("Migrate tới TCB {tcb} không đọc được: {e:?}"))?;
                if !(matches!(info.state, TcbState::Ready | TcbState::Running { .. })) {
                    return Err(format!(
                        "Migrate tới TCB {tcb} state {} (phải Ready/Running)",
                        info.state.name()
                    ));
                }
                if info.home == to_hart {
                    return Err(format!("Migrate TCB {tcb}: home == to_hart"));
                }
                let can = (1u64 << info.home) | (1u64 << to_hart);
                if info.affinity & can != can {
                    return Err(format!("Migrate TCB {tcb}: hart ngoài affinity"));
                }
                if let Some(dem) = yeu_cau_cua.get_mut(tcb as usize) {
                    *dem += 1;
                    if *dem > 1 {
                        return Err(format!(
                            "TCB {tcb} có nhiều hơn một yêu cầu Migrate đang chờ"
                        ));
                    }
                }
            }
        }
        // (2) đẳng thức bảo toàn: hạng tử mailbox đếm ở **cả hai vế** (không in-flight).
        if ready_or_running + pending != queued_total + current_total + pending {
            return Err(format!(
                "bảo toàn TCB vỡ: Ready/Running={ready_or_running} + migrate chờ={pending} \
                 != hàng={queued_total} + current={current_total} + migrate chờ={pending}"
            ));
        }
        Ok(())
    }

    /// **KT-29** — bí danh của [`Machine::check_smp`] (tên dùng trong test/brief KT-29:
    /// "check_harts"): cùng một bộ bất biến, không nới lỏng.
    pub fn check_harts(&self) -> Result<(), String> {
        self.check_smp()
    }
}
