// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `notification`: Notification + WaitOnAddress.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------- KT-27: bất biến Notification

    /// **KT-27** — bất biến Notification (chạy sau **mỗi** public op qua
    /// [`Machine::sched_after_op`], cùng chỗ với `check_sched`/`check_ipc`):
    ///
    /// 1. `waiter == Some(t)` ⇒ `t` là TCB sống đang `BlockedOnNotification{ntfn}` (một waiter,
    ///    khớp trạng thái — bắt đường "đổi trạng thái trực tiếp" không qua `unlink`). Chiều
    ///    ngược (`TcbState ⇒ waiter`) **không** kiểm ở đây: `transition_tcb` là cổng thấp được
    ///    phép dựng trạng thái cho ma trận 11×11 (KT-23) mà không cần object thật; mọi đường
    ///    IPC thật (`notification_wait`/`signal`/`block_current`) đều ghi waiter.
    /// 2. `badge == word` (mô hình: `Signal` OR badge vào **cả hai**, `Wait`/`Poll` xoá **cả
    ///    hai** — bắt lệch xoá/giao);
    /// 3. `bound_sc == Some(sc)` ⇔ `sc` sống, là `SchedContext`, và `SchedData.bound` của nó là
    ///    cap Notification còn sống trỏ **đúng** notification này (bind 1:1 **hai chiều**);
    /// 4. `Tcb.bound_notification` trỏ Notification sống; mỗi notification bind **một** TCB.
    pub fn check_notifications(&self) -> Result<(), String> {
        let n_objs = self.objects.len();
        let obj_alive = |id: ObjId| -> bool {
            id > 0 && (id as usize) <= n_objs && self.objects[id as usize - 1].alive
        };
        let cap_alive = |cap: CapId| -> bool {
            cap > 0 && (cap as usize) <= self.caps.len() && self.caps[cap as usize - 1].alive
        };
        // `sc_bind[sc]` = Notification mà SC đang bind (cap còn sống, đúng kiểu).
        let mut sc_bind: Vec<Option<ObjId>> = vec![None; n_objs + 1];
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::SchedContext {
                continue;
            }
            let sc = (i + 1) as ObjId;
            let bound = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.bound),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SC {sc}: bound không đọc được: {e:?}"))?;
            if let Some(cap) = bound {
                if cap_alive(cap) {
                    let c = self
                        .cap(cap)
                        .map_err(|e| format!("SC {sc}: bound cap {cap} lỗi: {e:?}"))?;
                    if c.ty == ObjType::Notification {
                        sc_bind[sc as usize] = Some(c.obj);
                    }
                }
            }
        }
        // 1 + 4: mọi TCB sống.
        let mut bound_by: Vec<Option<ObjId>> = vec![None; n_objs + 1];
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i + 1) as ObjId;
            let info = self
                .tcb_info_obj(tcb)
                .map_err(|e| format!("TCB {tcb} không đọc được: {e:?}"))?;
            if let Some(ntfn) = info.bound_notification {
                if !obj_alive(ntfn) {
                    return Err(format!("TCB {tcb}: bound_notification {ntfn} đã chết"));
                }
                if self.objects[ntfn as usize - 1].ty != ObjType::Notification {
                    return Err(format!(
                        "TCB {tcb}: bound_notification {ntfn} không phải Notification"
                    ));
                }
                if let Some(other) = bound_by[ntfn as usize] {
                    return Err(format!(
                        "Notification {ntfn} bind hai TCB: {other} và {tcb} (1:1)"
                    ));
                }
                bound_by[ntfn as usize] = Some(tcb);
            }
        }
        // 2 + 3: mọi Notification sống.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Notification {
                continue;
            }
            let ntfn = (i + 1) as ObjId;
            let (word, badge, waiter, bound_sc) = self
                .body(ntfn, |k| match k {
                    Kind::Notification(n) => Ok((n.word, n.badge, n.waiter, n.bound_sc)),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("Notification {ntfn} không đọc được: {e:?}"))?;
            if badge != word {
                return Err(format!(
                    "Notification {ntfn}: badge {badge:#x} != word {word:#x}"
                ));
            }
            if let Some(t) = waiter {
                if !obj_alive(t) {
                    return Err(format!("Notification {ntfn}: waiter {t} đã chết"));
                }
                if self.objects[t as usize - 1].ty != ObjType::Tcb {
                    return Err(format!("Notification {ntfn}: waiter {t} không phải TCB"));
                }
                let info = self
                    .tcb_info_obj(t)
                    .map_err(|e| format!("Notification {ntfn}: waiter {t} lỗi: {e:?}"))?;
                if info.state != (TcbState::BlockedOnNotification { ntfn }) {
                    return Err(format!(
                        "Notification {ntfn}: waiter {t} ở trạng thái {} thay vì BlockedOnNotification",
                        info.state.name()
                    ));
                }
            }
            if let Some(sc) = bound_sc {
                if !obj_alive(sc) {
                    return Err(format!("Notification {ntfn}: bound_sc {sc} đã chết"));
                }
                if self.objects[sc as usize - 1].ty != ObjType::SchedContext {
                    return Err(format!(
                        "Notification {ntfn}: bound_sc {sc} không phải SchedContext"
                    ));
                }
                if sc_bind[sc as usize] != Some(ntfn) {
                    return Err(format!(
                        "Notification {ntfn}: bound_sc {sc} nhưng SC không bind notification này"
                    ));
                }
            }
        }
        // 3 (chiều ngược): SC bind Notification ⇒ notification.bound_sc == SC.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::SchedContext {
                continue;
            }
            let sc = (i + 1) as ObjId;
            if let Some(ntfn) = sc_bind[sc as usize] {
                let bsc = self
                    .body(ntfn, |k| match k {
                        Kind::Notification(n) => Ok(n.bound_sc),
                        _ => Err(KError::WrongObjectType),
                    })
                    .map_err(|e| format!("Notification {ntfn}: bound_sc lỗi: {e:?}"))?;
                if bsc != Some(sc) {
                    return Err(format!(
                        "SC {sc} bind Notification {ntfn} nhưng notification.bound_sc = {bsc:?}"
                    ));
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------- KT-28: bất biến WaitOnAddress

    /// **KT-28** — bất biến bảng băm WaitOnAddress (chạy sau **mỗi** public op qua
    /// [`Machine::sched_after_op`], cùng chỗ `check_sched`/`check_ipc`/`check_notifications`):
    ///
    /// 1. mỗi nút nằm **đúng** bucket `waitaddr_bucket(frame, offset)` của khoá (bắt bảng băm
    ///    bị ghi sai chỗ — va chạm bucket **không** phải lỗi, khoá được so đầy đủ);
    /// 2. TCB của nút **còn sống**, **không** `suspended`, và trạng thái đúng
    ///    `BlockedOnAddress{frame, offset}` của nút (bắt "đổi trạng thái trực tiếp" không qua
    ///    `unlink_tcb_queues` — như `check_notifications`, chiều `TcbState ⇒ nút` **không** kiểm
    ///    vì `transition_tcb` là cổng thấp của ma trận 11×11 KT-23 dựng trạng thái với `ObjId`
    ///    không tồn tại; mọi đường thật đều đi qua [`Machine::wait_addr`]);
    /// 3. **không trùng**: một TCB tối đa **một** nút waiter.
    ///
    /// Đường nhanh khi **chưa từng** có waiter (L1 1.000 seed của cổng): mọi bucket rỗng ⇒ bất
    /// biến vacuous, không đọc trạng thái TCB nào.
    pub fn check_waitaddr(&self) -> Result<(), String> {
        if self.address_waiters.iter().all(|v| v.is_empty()) {
            return Ok(());
        }
        let n_objs = self.objects.len();
        let mut seen: Vec<bool> = vec![false; n_objs + 1];
        for (b, bucket) in self.address_waiters.iter().enumerate() {
            for (frame, offset, tcb) in bucket {
                let mong = waitaddr_bucket(*frame, *offset);
                if mong != b {
                    return Err(format!(
                        "waiter khoá ({frame},{offset}) nằm ở bucket {b}, phải là {mong}"
                    ));
                }
                if *tcb == 0 || *tcb as usize > n_objs || !self.objects[*tcb as usize - 1].alive {
                    return Err(format!("waiter cho TCB không tồn tại/đã chết: {tcb}"));
                }
                if seen[*tcb as usize] {
                    return Err(format!("TCB {tcb} có nhiều hơn một nút waiter (trùng)"));
                }
                seen[*tcb as usize] = true;
                let st = self
                    .tcb_state(*tcb)
                    .map_err(|e| format!("TCB {tcb}: không đọc được trạng thái: {e:?}"))?;
                if matches!(st, TcbState::Stopping | TcbState::Dead) {
                    return Err(format!(
                        "waiter khoá ({frame},{offset}) cho TCB {tcb} đã chết ({})",
                        st.name()
                    ));
                }
                match st {
                    TcbState::BlockedOnAddress {
                        frame: f,
                        offset: o,
                    } if f == *frame && o == *offset => {}
                    other => {
                        return Err(format!(
                            "waiter khoá ({frame},{offset}) của TCB {tcb} lệch trạng thái {}",
                            other.render()
                        ));
                    }
                }
                let suspended = self
                    .body(*tcb, |k| match k {
                        Kind::Tcb(t) => Ok(t.suspended),
                        _ => Err(KError::WrongObjectType),
                    })
                    .map_err(|e| format!("TCB {tcb}: không đọc được cờ suspended: {e:?}"))?;
                if suspended {
                    return Err(format!("waiter của TCB {tcb} đang bị treo (suspended)"));
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------- hàng chờ phụ của mô hình K1b `[TK]`

    /// `Wait` trên Notification — model API `[TK]` (KT-18, hoàn thiện ở **KT-27**): word ≠ 0 ⇒
    /// giao ngay; word = 0 ⇒ `BlockedOnNotification` với **một** waiter. Xem
    /// [`Machine::notification_wait`].
    pub fn tcb_wait_notification(&mut self, tcb: SlotRef, ntfn: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        self.notification_wait(tcb_obj, ntfn_obj)
    }

    /// Thân của `Wait` trên Notification theo `ObjId` — dùng chung cho `tcb_wait_notification`
    /// (model API K1b) và **`NBSendWait`** của KT-25.
    pub(super) fn wait_notification_obj(
        &mut self,
        tcb_obj: ObjId,
        ntfn_obj: ObjId,
    ) -> Result<(), KError> {
        self.notification_wait(tcb_obj, ntfn_obj)
    }

    /// **KT-27** — `Wait` trên Notification theo `ObjId` (model API `[TK]`; `recv` trên cap
    /// Notification và `NBSendWait` dùng chung):
    ///
    /// - `word != 0` ⇒ **trả ngay**: giao `(word, badge)` cho TCB (đọc bằng
    ///   [`Machine::tcb_notification`]), rồi **xoá** word + badge trên object; TCB giữ nguyên
    ///   `Ready`/`Running`.
    /// - `word == 0` ⇒ TCB `BlockedOnNotification`; notification đã có waiter ⇒ `Busy`
    ///   (thiết kế §4.7: **một** waiter — khác v1 "đánh thức tất cả", QD §3.11).
    pub fn notification_wait(&mut self, tcb_obj: ObjId, ntfn_obj: ObjId) -> Result<(), KError> {
        let state = self.ensure_started(tcb_obj)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let (word, badge, busy) = self.body(ntfn_obj, |k| match k {
            Kind::Notification(n) => Ok((n.word, n.badge, n.waiter.is_some())),
            _ => Err(KError::WrongObjectType),
        })?;
        if word != 0 {
            self.body_mut(ntfn_obj, |k| match k {
                Kind::Notification(n) => {
                    n.word = 0;
                    n.badge = 0;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            self.deliver_notification(tcb_obj, word, badge)?;
            self.sched_after_op(false);
            return Ok(());
        }
        if busy {
            return Err(KError::Busy);
        }
        self.body_mut(ntfn_obj, |k| match k {
            Kind::Notification(n) => {
                n.waiter = Some(tcb_obj);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.transition_tcb(
            tcb_obj,
            NonReadyTarget::BlockedOnNotification { ntfn: ntfn_obj },
        )?;
        self.sched_after_op(true);
        Ok(())
    }

    /// Ghi `(word, badge)` vừa giao cho TCB (model-only `[TK]`, KT-27) — **không** đi qua
    /// `ipc::transfer`: Notification không phải message IPC nên `check_ipc`/`transfer_calls`
    /// không bị ảnh hưởng.
    pub(super) fn deliver_notification(
        &mut self,
        tcb: ObjId,
        word: u64,
        badge: u64,
    ) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.notif = Some((word, badge));
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    // ------------------------------------------- WaitOnAddress (KT-28) `[TK]`

    /// **KT-28** — ghi giá trị "từ" tại `(frame, offset)` (mô hình thay vùng nhớ Frame của
    /// K2; object `Frame` mới là reserved). Giá trị mặc định 0 như bộ nhớ mới.
    pub fn set_addr_word(&mut self, frame: ObjId, offset: u32, value: u64) {
        for e in self.addr_words.iter_mut() {
            if e.0 == frame && e.1 == offset {
                e.2 = value;
                return;
            }
        }
        self.addr_words.push((frame, offset, value));
    }

    /// **KT-28** — đọc giá trị "từ" tại `(frame, offset)`; chưa ghi ⇒ 0.
    pub fn addr_word(&self, frame: ObjId, offset: u32) -> u64 {
        self.addr_words
            .iter()
            .find(|e| e.0 == frame && e.1 == offset)
            .map_or(0, |e| e.2)
    }

    /// **KT-28** — `WaitAddr(frame, offset, expected)` đúng thiết kế `:330-334` + QD §3.8:
    /// **đọc giá trị hiện tại và xếp hàng trong cùng một lần giữ khoá**.
    ///
    /// "Giữ khoá" của mô hình **một luồng** = **một hàm**: đọc [`Machine::addr_word`] và push
    /// nút vào [`waitaddr_bucket`] không có public op nào chen giữa (không `sched_after_op` ở
    /// giữa) ⇒ mô hình **không có** cửa sổ lost wake để biểu diễn. Bằng chứng "không mất đánh
    /// thức" là property theo **thứ tự bước** (fuzz KT-28), không phải thread thật — khoá/Mutex
    /// thật thuộc K2/K3 (K1C-PLAN §5 KT-28 rủi ro (a)).
    ///
    /// - `expected` **khác** giá trị hiện tại ⇒ `InvalidArgument`, TCB **không** xếp hàng và giữ
    ///   `Ready`/`Running` (K1C-PLAN §5 KT-28 DoD 2; bắt đột biến "xếp hàng trước rồi so").
    /// - Bằng ⇒ TCB `BlockedOnAddress{frame, offset}` + đúng **một** nút trong bucket của khoá.
    /// - **Không timeout** (D2-53 Q4 — spec thắng, `timeout` thuộc K6): không sinh
    ///   `Sleeping{Timeout}`, `tick` ảo không đánh thức waiter; chỉ `WakeAddr` (hoặc đường huỷ
    ///   `Suspend(Abort)`/`Exit`/finalise) đưa TCB ra khỏi hàng.
    pub fn wait_addr(
        &mut self,
        tcb: SlotRef,
        frame: ObjId,
        offset: u32,
        expected: u64,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let state = self.ensure_started(tcb_obj)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        // TCB đang bị treo (`Ready + suspended`) **không** được vào hàng chờ: bất biến §4.4 là
        // "TCB nằm trong hàng của X ⇔ trạng thái mang X **và** `!suspended`" — nếu không chặn,
        // `check_waitaddr` sẽ thấy waiter của TCB suspended.
        if self.tcb_info_obj(tcb_obj)?.suspended {
            return Err(KError::IllegalOperation);
        }
        // ---- "khoá": đọc giá trị **và** xếp hàng, không public op nào ở giữa ----
        let current = self.addr_word(frame, offset);
        if current != expected {
            return Err(KError::InvalidArgument);
        }
        // Chuyển trạng thái **trước** khi push: `transition_tcb` có thể từ chối cặp (bảng
        // 11×11) và không được để nút mồ côi; push không lỗi ⇒ hàng và trạng thái luôn khớp.
        self.transition_tcb(tcb_obj, NonReadyTarget::BlockedOnAddress { frame, offset })?;
        let bucket = waitaddr_bucket(frame, offset);
        self.address_waiters[bucket].push((frame, offset, tcb_obj));
        self.sched_after_op(true);
        Ok(())
    }

    /// `WaitAddr` bản **K1b** `[TK]` (không có `expected`) — **giữ nguyên API** cho KT-18/19/23:
    /// đọc giá trị hiện tại rồi đi **cùng một cửa** [`Machine::wait_addr`] (một đường xếp hàng),
    /// nên luôn khớp ⇒ luôn chặn như trước.
    pub fn tcb_wait_address(
        &mut self,
        tcb: SlotRef,
        frame: ObjId,
        offset: u32,
    ) -> Result<(), KError> {
        let current = self.addr_word(frame, offset);
        self.wait_addr(tcb, frame, offset, current)
    }

    /// **KT-28** — `WakeAddr(frame, offset, n)`: đánh thức **tối đa `n`** waiter **cùng khoá**
    /// đầy đủ `(frame, offset)` theo **FIFO** (thứ tự xếp hàng), trả **số đã đánh thức**;
    /// `n = u32::MAX` = tất cả; waiter còn lại vẫn `BlockedOnAddress` (**không** đánh thức quá
    /// n, **không** mất). Waiter của khoá khác trong **cùng bucket** (va chạm) không bị đụng.
    ///
    /// "Khoá" mô hình như [`Machine::wait_addr`]: chọn nút và gỡ nút (`wake_step` → `unlink`)
    /// trong cùng một hàm, không có public op ở giữa ⇒ không có cửa sổ lost wake.
    pub fn wake_addr(&mut self, frame: ObjId, offset: u32, n: u32) -> Result<u32, KError> {
        let bucket = waitaddr_bucket(frame, offset);
        let mut chon: Vec<ObjId> = Vec::new();
        if n > 0 {
            for (f, o, t) in self.address_waiters[bucket].iter() {
                if *f == frame && *o == offset {
                    chon.push(*t);
                    if chon.len() == n as usize {
                        break;
                    }
                }
            }
        }
        for tcb in &chon {
            // **KT-155 (N7-4)**: gỡ bucket WaitAddr (`unlink = true`) **và** gửi `Wake` về
            // `home` khi waiter ở hart khác — đối xứng kernel.
            self.wake_from_unlink(*tcb)?;
        }
        self.sched_after_op(false);
        Ok(chon.len() as u32)
    }

    pub fn notification_waiter(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok(n.waiter),
            _ => Err(KError::WrongObjectType),
        })
    }

    // ------------------------------------------- KT-27: Notification word/badge/Signal/Poll

    /// **KT-27** — `Send`/`NBSend` trên Notification = **Signal** (thiết kế §4.7 `:328`):
    /// `word |= badge` và `badge |= badge`; nếu có **một** waiter (hoặc TCB **bound** đang
    /// `BlockedOnRecv`) ⇒ đánh thức **đúng một** người, giao `(word, badge)` rồi **xoá** cả
    /// hai; không có ai chờ ⇒ giữ word cho lần `Wait` sau (không mất tín hiệu, C2).
    ///
    /// Badge **0 hợp lệ**: có waiter thì vẫn đánh thức (giao `(0,0)`); không dùng 0 làm
    /// "chưa có tín hiệu" (K1C-PLAN §5 KT-27 rủi ro (b)).
    pub fn notification_signal(&mut self, ntfn: ObjId, badge: u64) -> Result<(), KError> {
        let (word, acc_badge, waiter) = self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok((n.word, n.badge, n.waiter)),
            _ => Err(KError::WrongObjectType),
        })?;
        let new_word = word | badge;
        let new_badge = acc_badge | badge;
        // Chọn **một** người được đánh thức: waiter của notification trước, rồi TCB **bound**
        // đang `BlockedOnRecv` (model-only `[TK]`, K1C-PLAN §5 KT-27 DoD 5).
        let target = match waiter {
            Some(t) => {
                if self.tcb_state(t)? != (TcbState::BlockedOnNotification { ntfn }) {
                    return Err(KError::IllegalOperation);
                }
                Some(t)
            }
            None => self.bound_tcb_blocked_on_recv(ntfn)?,
        };
        let Some(target) = target else {
            // Không ai chờ: word giữ nguyên cho `Wait` sau.
            self.body_mut(ntfn, |k| match k {
                Kind::Notification(n) => {
                    n.word = new_word;
                    n.badge = new_badge;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            self.sched_after_op(false);
            return Ok(());
        };
        self.body_mut(ntfn, |k| match k {
            Kind::Notification(n) => {
                n.word = new_word;
                n.badge = new_badge;
                n.waiter = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.deliver_notification(target, new_word, new_badge)?;
        // Gỡ TCB khỏi **mọi** hàng chờ (waiter notification hoặc `Recv` của Endpoint) trước
        // khi `→ Ready` — cùng cửa với `Suspend(Abort)`/finalise.
        self.unlink_tcb_queues(target);
        self.wake_from(target)?;
        self.body_mut(ntfn, |k| match k {
            Kind::Notification(n) => {
                n.word = 0;
                n.badge = 0;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(true);
        Ok(())
    }

    /// TCB **bound** với `ntfn` đang `BlockedOnRecv` (nếu có) — nhánh đánh thức thứ hai của
    /// `Signal` (K1C-PLAN §5 KT-27 DoD 5). Quét theo `ObjId` tăng dần ⇒ **tất định**.
    pub(super) fn bound_tcb_blocked_on_recv(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i as u32) + 1;
            let (bound, state) = self.body(tcb, |k| match k {
                Kind::Tcb(t) => Ok((t.bound_notification, t.state)),
                _ => Err(KError::WrongObjectType),
            })?;
            if bound == Some(ntfn) && matches!(state, TcbState::BlockedOnRecv { .. }) {
                return Ok(Some(tcb));
            }
        }
        Ok(None)
    }

    /// **KT-27** — `Poll` trên Notification (model API `[TK]`): word ≠ 0 ⇒ trả `(word, badge)`
    /// và **xoá**; rỗng ⇒ `None` (`NBRecv` trên cap Notification trả `Busy`).
    pub fn notification_poll(&mut self, ntfn: ObjId) -> Result<Option<NotificationValue>, KError> {
        let (word, badge) = self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok((n.word, n.badge)),
            _ => Err(KError::WrongObjectType),
        })?;
        if word == 0 {
            return Ok(None);
        }
        self.body_mut(ntfn, |k| match k {
            Kind::Notification(n) => {
                n.word = 0;
                n.badge = 0;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(Some(NotificationValue { word, badge }))
    }

    /// **KT-27** — word 64 bit hiện tại của Notification (model API `[TK]`).
    pub fn notification_word(&self, ntfn: ObjId) -> Result<u64, KError> {
        self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok(n.word),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-27** — badge OR hiện tại của Notification (model API `[TK]`; bất biến
    /// `badge == word`).
    pub fn notification_badge(&self, ntfn: ObjId) -> Result<u64, KError> {
        self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok(n.badge),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-27** — `SchedContext` đang bind notification (model API `[TK]`, 1:1); `None` nếu
    /// chưa bind.
    pub fn notification_bound_sc(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ntfn, |k| match k {
            Kind::Notification(n) => Ok(n.bound_sc),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-27** — `(word, badge)` của lần `Wait`/`Poll` gần nhất giao cho TCB (model API
    /// `[TK]`); `None` nếu chưa nhận lần nào. Đọc không tiêu thụ — dùng
    /// [`Machine::take_tcb_notification`] nếu cần "lấy rồi xoá".
    pub fn tcb_notification(&self, tcb: ObjId) -> Result<Option<NotificationValue>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t
                .notif
                .map(|(word, badge)| NotificationValue { word, badge })),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-27** — như [`Machine::tcb_notification`] nhưng **tiêu thụ** bản ghi (`None` sau khi
    /// lấy) — tránh đọc hai lần cùng một tín hiệu.
    pub fn take_tcb_notification(
        &mut self,
        tcb: ObjId,
    ) -> Result<Option<NotificationValue>, KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t
                .notif
                .take()
                .map(|(word, badge)| NotificationValue { word, badge })),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-28** — waiter **đầu tiên** (FIFO) của đúng khoá `(frame, offset)`; API K1b giữ
    /// nguyên tên/chữ ký. Va chạm bucket không ảnh hưởng: khoá được so **đầy đủ**.
    pub fn address_waiter(&self, frame: ObjId, offset: u32) -> Option<ObjId> {
        self.address_waiters_for(frame, offset).first().copied()
    }

    /// **KT-28** — tổng số nút waiter trên **mọi** bucket.
    pub fn address_waiters_len(&self) -> usize {
        self.address_waiters.iter().map(Vec::len).sum()
    }

    /// **KT-28** — danh sách waiter **cùng khoá** `(frame, offset)` theo **FIFO** (test/đối
    /// chứng); nút của khoá va chạm bucket **khác** không lẫn vào.
    pub fn address_waiters_for(&self, frame: ObjId, offset: u32) -> Vec<ObjId> {
        self.address_waiters[waitaddr_bucket(frame, offset)]
            .iter()
            .filter(|(f, o, _)| *f == frame && *o == offset)
            .map(|(_, _, t)| *t)
            .collect()
    }

    /// **KT-28** — số nút trong **bucket** `bucket` (gồm cả khoá va chạm) — test "gỡ đúng
    /// bucket": sau `Suspend(Abort)`/`Exit`/finalise, bucket của khoá phải **rỗng**.
    pub fn waitaddr_bucket_len(&self, bucket: usize) -> usize {
        self.address_waiters.get(bucket).map_or(0, Vec::len)
    }

    /// **KT-28** — số bucket **đang có** waiter (`0` ⇒ mọi bucket rỗng) — test gỡ đúng bucket.
    pub fn waitaddr_nonempty_buckets(&self) -> usize {
        self.address_waiters
            .iter()
            .filter(|v| !v.is_empty())
            .count()
    }
}
