// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `tlb`: TLB + shootdown + ack.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------- TLB + shootdown (KT-40) `[TK]`
    //
    // **K2 không phải kernel thật**: "IPI" = đẩy `MailboxMsg::TlbFlush` vào hộp thư hart đích;
    // "ack" = bộ đếm thế hệ `delivered`/`acked` của hart (D2-65 Q5, D2-60). Vòng chờ ack của
    // hart hiện tại rút **hộp thư của chính nó** (luật chống deadlock, thiết kế §4.10.3) rồi
    // cho các hart đích "chạy" handler — cả hai đều cần thiết; xem `k2-vm.md` §14.

    /// TLB của một hart (lỗi `InvalidArgument` nếu hart không có thật).
    pub(super) fn tlb_at(&self, hart: u16) -> Result<&HartTlb, KError> {
        self.tlbs.get(hart as usize).ok_or(KError::InvalidArgument)
    }

    pub(super) fn tlb_at_mut(&mut self, hart: u16) -> Result<&mut HartTlb, KError> {
        self.tlbs
            .get_mut(hart as usize)
            .ok_or(KError::InvalidArgument)
    }

    /// Số entry TLB đang sống của hart (`<= TLB_ENTRIES_PER_HART`).
    pub fn tlb_entries(&self, hart: u16) -> Result<usize, KError> {
        Ok(self.tlb_at(hart)?.len())
    }

    /// Ảnh chụp TLB một hart: entry + hit/miss/fill/evict/flush + `delivered`/`acked`.
    pub fn tlb_info(&self, hart: u16) -> Result<TlbHartInfo, KError> {
        Ok(self.tlb_at(hart)?.info(hart))
    }

    /// **`translate(vspace, va)`** của mô hình (K2-PLAN §3 KT-40): dùng entry nếu
    /// `(generation, asid)` khớp và entry phủ `va`; ngược lại **miss** rồi walk PageTable →
    /// nạp entry (LRU) → trả translation.
    ///
    /// Hart hiện tại phải **đang chạy** VSpace (`active_on`), nếu không ⇒
    /// [`KError::IllegalOperation`] (Touch chỉ có nghĩa khi hart nạp ASID). Walk lỗi (chưa map,
    /// ngoài dải) ⇒ trả nguyên lỗi, **không** nạp entry rác.
    pub fn tlb_translate(&mut self, vs: ObjId, va: u64) -> Result<TlbTranslation, KError> {
        let hart = self.current_hart;
        let info = self.vspace_info(vs)?;
        if !info.active_on.contains(hart) {
            return Err(KError::IllegalOperation);
        }
        if va >= VA_LIMIT {
            return Err(KError::RangeError);
        }
        if let Some(e) = self
            .tlb_at_mut(hart)?
            .lookup(info.generation, info.asid, va)
        {
            return Ok(e.translation(true));
        }
        let m = self.vspace_walk(vs, va)?;
        let e = self
            .tlb_at_mut(hart)?
            .fill_keyed(info.generation, info.asid, m);
        self.tlb_fill_total += 1;
        self.sched_after_op(false);
        Ok(e.translation(false))
    }

    /// Tra TLB **không** nạp/không đổi LRU (test/oracle): `Some` ⇔ có entry của `vs` phủ `va`.
    pub fn tlb_lookup(
        &self,
        hart: u16,
        vs: ObjId,
        va: u64,
    ) -> Result<Option<TlbTranslation>, KError> {
        let t = self.tlb_at(hart)?;
        let info = self.vspace_info(vs)?;
        Ok(t.peek(info.generation, info.asid, va)
            .map(|e| e.translation(true)))
    }

    /// **Flush cục bộ** một hart theo phạm vi Q4 (`Page { va }` / `Asid` / `Generation`) — model
    /// API, không qua hộp thư/ack. `Generation` dùng `generation` của `vs` làm mốc: xoá mọi
    /// entry **cũ hơn** (cạn ASID). Trả số entry bị vô hiệu.
    pub fn tlb_flush_local(
        &mut self,
        hart: u16,
        vs: ObjId,
        scope: TlbFlushScope,
    ) -> Result<usize, KError> {
        let info = self.vspace_info(vs)?;
        let n = self
            .tlb_at_mut(hart)?
            .flush(info.generation, info.asid, scope);
        self.sched_after_op(false);
        Ok(n)
    }

    /// **Bắt đầu** một shootdown (model API): flush cục bộ hart `hart` + giao `TlbFlush` cho
    /// **mọi hart khác** trong `active_on` (không tự IPI mình — D2-68; hart gọi ack ngầm sau
    /// flush cục bộ); **không** chờ (xem [`Machine::tlb_shootdown_wait`]). Trả số hiệu shootdown
    /// toàn cục (1-based). Đã có shootdown đang chờ ⇒ [`KError::Busy`].
    pub fn tlb_shootdown_begin(&mut self, hart: u16, vs: ObjId, va: u64) -> Result<u64, KError> {
        let info = self.vspace_info(vs)?;
        self.tlb_shootdown_prepare(hart, info, va)?;
        let seq = self.tlb_shootdown_post(hart)?;
        self.sched_after_op(false);
        Ok(seq)
    }

    /// **Chờ đủ ack** (`acked_mask == targets`) — `Ok` chỉ khi mọi hart đích đã xử lý
    /// `TlbFlush`; không có gì chờ ⇒ `Ok` (idempotent). Hết số vòng an toàn ⇒
    /// [`KError::Busy`] (không bao giờ treo — mutation "bỏ xử lý hộp thư khi chờ ack" bị bắt).
    pub fn tlb_shootdown_wait(&mut self, hart: u16) -> Result<(), KError> {
        self.tlb_at(hart)?;
        let Some(mut p) = self.tlb_pending[hart as usize].take() else {
            return Ok(());
        };
        if !p.posted {
            self.tlb_pending[hart as usize] = Some(p);
            return Err(KError::IllegalOperation);
        }
        // Mỗi vòng rút cạn hộp thư của hart chờ + các hart đích; bình thường xong ở vòng 1.
        let max_rounds = MAILBOX_CAP as u64 + 2;
        loop {
            // (1) **Luật chống deadlock** (thiết kế §4.10.3): hart chờ ack **phải rút hộp thư
            //     của mình** — ack của hart khác có thể đang nằm đó (AP→BSP của v1). Đột biến
            //     "bỏ xử lý hộp thư khi chờ ack" xoá đúng dòng trong `tlb_wait_round`.
            if self.tlb_wait_round(hart, &mut p) {
                let acked = self.tlb_acked_mask(&p);
                self.last_tlb_shootdown = Some(p.info(acked, true));
                self.sched_after_op(false);
                return Ok(());
            }
            // (2) Mô phỏng IPI: hart đích còn thiếu ack được "chạy" handler của nó.
            for &(t, ack) in &p.acks {
                if t != hart && self.tlbs[t as usize].acked < ack {
                    let _ = self.drain_mail_fifo(t);
                }
            }
            if self.tlb_acked_mask(&p) == p.targets {
                let acked = self.tlb_acked_mask(&p);
                self.last_tlb_shootdown = Some(p.info(acked, true));
                self.sched_after_op(false);
                return Ok(());
            }
            if p.rounds >= max_rounds {
                self.tlb_wait_timeouts += 1;
                let acked = self.tlb_acked_mask(&p);
                self.last_tlb_shootdown = Some(p.info(acked, false));
                // Không im lặng: `Ok` **không** được trả; `check_tlb` sau op sẽ thấy entry còn
                // stale (nếu có) và bắt lỗi bất biến.
                self.sched_after_op(false);
                return Err(KError::Busy);
            }
        }
    }

    /// **Một vòng chờ ack của chính hart** (model API, test interleaving AP→BSP): rút hộp thư
    /// **của hart này** (luật §4.10.3) rồi kiểm `acked_mask == targets`; **không** cho hart
    /// khác "chạy" — đúng nghĩa "hart đang chờ tự phục vụ hộp thư của mình". Trả `true` ⇔ xong.
    ///
    /// Không có shootdown đang chờ ⇒ `Ok(true)`. Dùng cho test 2 hart **chờ nhau**: mỗi hart
    /// chỉ tiến được khi tự rút hộp thư của mình; thiếu bước đó là deadlock (AP→BSP).
    pub fn tlb_shootdown_poll(&mut self, hart: u16) -> Result<bool, KError> {
        self.tlb_at(hart)?;
        let Some(mut p) = self.tlb_pending[hart as usize].take() else {
            return Ok(true);
        };
        if !p.posted {
            self.tlb_pending[hart as usize] = Some(p);
            return Err(KError::IllegalOperation);
        }
        let done = self.tlb_wait_round(hart, &mut p);
        if done {
            let acked = self.tlb_acked_mask(&p);
            self.last_tlb_shootdown = Some(p.info(acked, true));
        } else {
            self.tlb_pending[hart as usize] = Some(p);
        }
        self.sched_after_op(false);
        Ok(done)
    }

    /// Một vòng **tự phục vụ** của vòng chờ: rút hộp thư của chính hart (chống deadlock) rồi
    /// kiểm ack. Dùng chung cho [`Machine::tlb_shootdown_wait`] và
    /// [`Machine::tlb_shootdown_poll`]; đột biến "bỏ xử lý hộp thư khi chờ ack" xoá đúng
    /// `drain_mail_fifo` ở đây.
    pub(super) fn tlb_wait_round(&mut self, hart: u16, p: &mut PendingShootdown) -> bool {
        let _ = self.drain_mail_fifo(hart);
        p.rounds += 1;
        self.tlb_acked_mask(p) == p.targets
    }

    /// Ảnh chụp shootdown **đang chờ** của hart (`None` nếu không chờ gì).
    pub fn tlb_pending(&self, hart: u16) -> Result<Option<TlbShootdownInfo>, KError> {
        self.tlb_at(hart)?;
        Ok(self.tlb_pending[hart as usize]
            .as_ref()
            .map(|p| p.info(self.tlb_acked_mask(p), false)))
    }

    /// Shootdown gần nhất đã kết thúc (thành công ⇒ `completed`, `acked == targets`).
    pub fn last_tlb_shootdown(&self) -> Option<TlbShootdownInfo> {
        self.last_tlb_shootdown
    }

    /// **KT-64a (D2-92c)** — `Some(seq)` ⇔ `hart` còn **nợ ack** của shootdown `seq`: nó là hart
    /// **đích** của một `TlbFlush` (`acks` của shootdown còn `acked < ack`). Vai thứ hai của cùng
    /// khái niệm `pending` mà core giữ **một** ô/hart ([`Machine::tlb_pending`] của core = hart gọi
    /// hoặc hart nợ ack); sim tách hai vai nên cần cửa đọc riêng để parity `pending` so được
    /// (`kt64_core_parity.rs` kịch bản C). Không có gì nợ ⇒ `None`; hart ngoài dải ⇒
    /// `InvalidArgument`.
    pub fn tlb_pending_ack(&self, hart: u16) -> Result<Option<u64>, KError> {
        self.tlb_at(hart)?;
        for p in self.tlb_pending.iter().flatten() {
            if !p.posted {
                continue;
            }
            for &(t, ack) in &p.acks {
                if t == hart && self.tlbs[t as usize].acked < ack {
                    return Ok(Some(p.seq));
                }
            }
        }
        Ok(None)
    }

    /// Số lệnh shootdown đã phát (kể cả `active_on` rỗng).
    pub fn tlb_shootdown_count(&self) -> u64 {
        self.tlb_shootdown_count
    }

    /// Tổng số `TlbFlush` đã giao vào hộp thư (`= số send Ok`).
    pub fn tlb_message_count(&self) -> u64 {
        self.tlb_msg_total
    }

    /// Tổng số ack **mới** đã ghi nhận (`fetch_max` delta).
    pub fn tlb_ack_count(&self) -> u64 {
        self.tlb_ack_total
    }

    /// Số lần vòng chờ ack hết hạn (chẩn đoán; phải bằng 0 ở mô hình đúng).
    pub fn tlb_timeouts(&self) -> u64 {
        self.tlb_wait_timeouts
    }

    /// Mask hart **đã** ack đủ shootdown `p` (so `acked` của từng hart với `ack_seq` đã giao).
    pub(super) fn tlb_acked_mask(&self, p: &PendingShootdown) -> HartMask {
        // Hart gọi (nếu thuộc `active_on`) ack **ngầm** sau flush cục bộ — không có message.
        let mut m = p.implicit;
        for &(t, ack) in &p.acks {
            if self.tlbs[t as usize].acked >= ack {
                m = m.insert(t);
            }
        }
        m
    }

    /// Chuẩn bị shootdown: đăng ký pending + **flush cục bộ** hart `hart` (trước khi đổi PTE để
    /// `check_tlb` cho phép trạng thái chuyển tiếp trên chính hart đó). Chưa giao message.
    pub(super) fn tlb_shootdown_prepare(
        &mut self,
        hart: u16,
        info: VSpaceInfo,
        va: u64,
    ) -> Result<(), KError> {
        if hart as usize >= self.tlbs.len() {
            return Err(KError::InvalidArgument);
        }
        if self.tlb_pending[hart as usize].is_some() {
            return Err(KError::Busy);
        }
        self.tlbs[hart as usize].flush(info.generation, info.asid, TlbFlushScope::Page { va });
        // **Không IPI cho chính mình**: hart gọi flush cục bộ đồng bộ (dòng trên) và **ack
        // ngầm**; chỉ gửi `TlbFlush` cho các hart **khác** trong `active_on` (D2-68).
        let implicit = if info.active_on.contains(hart) {
            HartMask::EMPTY.insert(hart)
        } else {
            HartMask::EMPTY
        };
        self.tlb_pending[hart as usize] = Some(PendingShootdown {
            seq: 0,
            generation: info.generation,
            asid: info.asid,
            va,
            targets: info.active_on,
            implicit,
            acks: Vec::new(),
            messages: 0,
            rounds: 0,
            posted: false,
        });
        Ok(())
    }

    /// Giao `TlbFlush` cho **mọi** hart trong `targets` (thứ tự hart tăng dần ⇒ tất định). Mỗi
    /// message được kernel gán `ack = delivered` của hart đích (`= số send Ok` của Q5).
    pub(super) fn tlb_shootdown_post(&mut self, hart: u16) -> Result<u64, KError> {
        let mut p = self.tlb_pending[hart as usize]
            .take()
            .ok_or(KError::IllegalOperation)?;
        if p.posted {
            self.tlb_pending[hart as usize] = Some(p);
            return Err(KError::IllegalOperation);
        }
        self.tlb_shootdown_seq += 1;
        let seq = self.tlb_shootdown_seq;
        p.seq = seq;
        for t in 0..self.hart_vspace.len() as u16 {
            // Không gửi cho hart **gọi** (nó đã flush cục bộ + ack ngầm — D2-68).
            if t == hart || !p.targets.contains(t) {
                continue;
            }
            // Hộp thư đầy ⇒ cho hart đích "chạy" một vòng để có chỗ (backpressure §4.17); sau
            // đó còn slot nên `TlbFlush` vào hàng được.
            if self.mail[t as usize].is_full() {
                let _ = self.drain_mail_fifo(t);
            }
            let msg = MailboxMsg::TlbFlush(TlbFlush {
                seq,
                // Kernel gán lại (= `delivered` sau khi tăng) — người gọi không tự đặt.
                ack: 0,
                generation: p.generation,
                asid: p.asid,
                va: p.va,
            });
            if let Err(e) = self.push_mail(t, msg) {
                // Không để pending nửa vời: xoá trạng thái chờ rồi trả lỗi (PTE đã đổi).
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
            let ack = self.tlbs[t as usize].delivered;
            p.acks.push((t, ack));
            p.messages += 1;
        }
        self.tlb_msg_total += p.messages as u64;
        self.tlb_shootdown_count += 1;
        p.posted = true;
        // **KT-64a (D2-92c)** — **không còn hart nào phải ack** (mọi hart đích trùng hart gọi ⇒
        // ack ngầm, hoặc `active_on` rỗng) ⇒ shootdown **xong ngay ở `begin`**: không giữ ô
        // `pending` rỗng. Đây là chi tiết sim **thiếu** so với chuẩn core (core: `targets == 0`
        // ⇒ không `pending`, `TlbFlushReport{local:true, acked==targets==0}`) và so với kernel
        // KT-51 (`completed` ở vòng chờ đầu vì ack ngầm đã đủ) — trước KT-64a sim để
        // `tlb_pending(hart) = Some(seq)` tới `wait` nên parity `pending` ca 1 hart lệch.
        if p.acks.is_empty() {
            let acked = self.tlb_acked_mask(&p);
            self.last_tlb_shootdown = Some(p.info(acked, true));
            return Ok(seq);
        }
        self.tlb_pending[hart as usize] = Some(p);
        Ok(seq)
    }

    /// Flush TLB một hart **không** chạy `sched_after_op` — dùng bên trong op khác.
    pub(super) fn tlb_flush_hart_only(
        &mut self,
        hart: u16,
        generation: u64,
        asid: u16,
        scope: TlbFlushScope,
    ) -> usize {
        self.tlbs
            .get_mut(hart as usize)
            .map(|t| t.flush(generation, asid, scope))
            .unwrap_or(0)
    }

    /// Entry `e` có đang nằm dưới một shootdown **chưa xong** không — oracle bỏ qua trạng thái
    /// chuyển tiếp "PTE đã đổi, `TlbFlush` chưa được xử lý".
    pub(super) fn tlb_pending_covers(&self, e: &TlbEntry) -> bool {
        self.tlb_pending
            .iter()
            .flatten()
            .any(|p| p.generation == e.key.generation && p.asid == e.key.asid && e.covers(p.va))
    }

    /// Lý do entry `e` của hart **không** nhất quán với PageTable hiện tại (oracle TLB-safety).
    pub(super) fn tlb_entry_violation(&self, hart: u16, e: &TlbEntry) -> Option<String> {
        // Tìm VSpace **sống** có cùng danh tính `(generation, asid)`.
        let mut vs = None;
        for (i, m) in self.objects.iter().enumerate() {
            if !m.alive || m.ty != ObjType::VSpace {
                continue;
            }
            let Some(sidx) = m.space else { continue };
            let Some(d) = self.vspaces.get(sidx).and_then(|x| x.as_ref()) else {
                continue;
            };
            if d.generation == e.key.generation && d.asid == e.key.asid {
                vs = Some(((i as ObjId) + 1, d.active_on));
                break;
            }
        }
        let Some((vs, active_on)) = vs else {
            return Some(format!(
                "TLB hart {hart}: entry (gen {}, asid {}, va {:#x}) không còn VSpace sống",
                e.key.generation, e.key.asid, e.key.va
            ));
        };
        // (1b) **KT-40** — entry chỉ được tồn tại trên hart **đang chạy** VSpace: hart rời
        // VSpace (`deactivate`/đổi VSpace/offline) phải flush; entry trên hart ngoài `active_on`
        // là rò rỉ TLB (mutation "bỏ flush khi deactivate" bị bắt ở đây).
        if !active_on.contains(hart) {
            return Some(format!(
                "TLB hart {hart}: entry (asid {}, va {:#x}) của VSpace nhưng hart **không** \
                 thuộc active_on",
                e.key.asid, e.key.va
            ));
        }
        match self.vspace_walk(vs, e.key.va) {
            Ok(m) => {
                if m.va == e.key.va
                    && m.pa == e.pa
                    && m.size_bits == e.size_bits
                    && m.frame == e.frame
                    && m.rights == e.rights
                    && m.attrs == e.attrs
                {
                    None
                } else {
                    Some(format!(
                        "TLB hart {hart}: entry (asid {}, va {:#x}) không khớp cây                          (pa/cỡ/frame/quyền/attrs)",
                        e.key.asid, e.key.va
                    ))
                }
            }
            Err(err) => Some(format!(
                "TLB hart {hart}: entry (asid {}, va {:#x}) nhưng walk trả {err:?}",
                e.key.asid, e.key.va
            )),
        }
    }

    /// **KT-40** — bất biến **TLB-safety** (thiết kế §9.6 mục 7, dòng DoD 876): mọi entry còn
    /// sống phải khớp **cây PageTable hiện tại** — đúng `(generation, asid)` của một VSpace sống,
    /// VA còn map, cùng `pa`/cỡ/frame/quyền/attrs. Entry đang dưới một shootdown chưa xong được
    /// bỏ qua (trạng thái chuyển tiếp có chủ đích).
    ///
    /// Chạy sau **mỗi** public op qua [`Machine::sched_after_op`], cửa sổ nhanh
    /// `tlb_fill_total == 0 && tlb_msg_total == 0` (mọi test K1a..K1c/ KT-36..39 giữ nguyên chi
    /// phí). Ca **dương tính** (checker không rỗng) ở unit test `kobj::tests::check_tlb_*`.
    pub fn check_tlb(&self) -> Result<(), String> {
        if self.tlb_fill_total == 0 && self.tlb_msg_total == 0 {
            return Ok(());
        }
        for (h, t) in self.tlbs.iter().enumerate() {
            if !t.invariant() {
                return Err(format!(
                    "TLB hart {h}: bất biến vỡ (entries {} > {}, acked {} > delivered {})",
                    t.len(),
                    TLB_ENTRIES_PER_HART,
                    t.acked,
                    t.delivered
                ));
            }
            for e in t.entries() {
                if self.tlb_pending_covers(e) {
                    continue;
                }
                if let Some(msg) = self.tlb_entry_violation(h as u16, e) {
                    return Err(msg);
                }
            }
        }
        Ok(())
    }

    /// Số entry TLB **không** nhất quán với cây (0 ⇔ TLB-safety đúng). Dùng cho test/đo.
    pub fn tlb_violations(&self) -> u64 {
        let mut n = 0u64;
        for (h, t) in self.tlbs.iter().enumerate() {
            for e in t.entries() {
                if self.tlb_pending_covers(e) {
                    continue;
                }
                if self.tlb_entry_violation(h as u16, e).is_some() {
                    n += 1;
                }
            }
        }
        n
    }
}
