<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-43 — snippet 17 đột biến (before → after) + test đích

Nguồn: harness `v2/agent/JOURNAL/kt43_mut.py` (mỗi ca thay **đúng một** chuỗi, chạy đúng một test, khôi phục sha256 == HEAD).

| # | File | Test đích | Tóm tắt |
|---|---|---|---|
| **M1** | `v2/sim/vifast-sim/src/ipc.rs` | `kt24_ipc::message_64_mr_hai_vung_thanh_ghi_va_ipc_buffer` | ipc::transfer bỏ copy 1 trong 64 MR |
| **M2** | `v2/sim/vifast-sim/src/ipc.rs` | `kt25_ipc2::thieu_grant_that_bai_mem_message_van_giao` | Bỏ kiểm RIGHT_GRANT khi chuyển cap |
| **M3** | `v2/sim/vifast-sim/src/kobj.rs` | `kt25_ipc2::bon_cap_cai_du_bo_grant_giu_badge` | Cap nhận giữ RIGHT_GRANT (không bỏ GRANT) |
| **M4** | `v2/sim/vifast-sim/src/ipc.rs` | `kt25_ipc2::unwrap_badge_khong_chiem_receive_window` | Bỏ unwrap badge (cap badge vào receive window) |
| **M5** | `v2/sim/vifast-sim/src/kobj.rs` | `kt27_notification::mot_waiter_duy_nhat_ba_tcb` | Signal không xoá word/badge sau khi giao |
| **M6** | `v2/sim/vifast-sim/src/kobj.rs` | `kt27_notification::mot_waiter_duy_nhat_ba_tcb` | Bỏ cổng 'một waiter' kiểu v1 |
| **M7** | `v2/sim/vifast-sim/src/kobj.rs` | `kt28_waitaddr::expected_lech_tra_ngay_khong_xep_hang` | WaitAddr xếp hàng trước rồi mới so expected |
| **M8** | `v2/sim/vifast-sim/src/kobj.rs` | `kt28_waitaddr::va_cham_bucket_khong_anh_huong_tim_kiem` | WakeAddr bỏ kiểm khoá (wake mọi waiter trong bucket) |
| **M9** | `v2/sim/vifast-sim/src/kobj.rs` | `kt29_smp::push_dung_mot_tcb_cuoi_hang_qua_migrate` | Push migration giữ home cũ (bỏ home = to) |
| **M10** | `v2/sim/vifast-sim/src/kobj.rs` | `kt26_fault::resume_with_regs_khi_dang_cho_reply_ghi_thanh_ghi_truoc_ready` | Fault-resume không ghi MR trước Ready (đường r319) |
| **M11** | `v2/lib/vifast-sync/src/mailbox.rs` | `kt30_sync::mailbox_day_tra_busy_khong_mat_message_dau` | Hộp thư đầy ghi đè thay vì Err(Busy) |
| **M12** | `v2/sim/vifast-sim/src/kobj.rs` | `kt25_ipc2::cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller` | Rời BlockedOnReply không gỡ Reply.caller (cổng transition_tcb; guard hiện hành sau KT-34) |
| **V1** | `v2/sim/vifast-sim/src/pagetable.rs` | `kt37_pagetable::map_walk_unmap_du_ba_co` | Map đổi lá/thứ tự cấp (AP→BSP): level_for_size_bits đảo cấp |
| **V2** | `v2/sim/vifast-sim/src/kobj.rs` | `kt40_tlb::shootdown_1_2_4_8_hart_ack_du` | Bỏ kiểm ack khi unmap (trả Ok khi chưa đủ ack) |
| **V3** | `v2/sim/vifast-sim/src/kobj.rs` | `kt39_wx::map_w_sau_x_bi_tu_choi_atomic_khong_ha_ngam` | Bỏ W^X khi map W (cho W sau X) |
| **V4** | `v2/sim/vifast-sim/src/kobj.rs` | `kt40_tlb::flush_theo_trang_asid_va_gen` | Bỏ flush TLB khi deactivate/teardown |
| **V5** | `v2/sim/vifast-sim/src/kobj.rs` | `kt40_tlb::deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` | Bỏ rút hộp thư khi chờ ack (AP→BSP deadlock) |

## M1 — ipc::transfer bỏ copy 1 trong 64 MR

- File: `v2/sim/vifast-sim/src/ipc.rs`
- Test: `kt24_ipc::message_64_mr_hai_vung_thanh_ghi_va_ipc_buffer`

**Trước (HEAD):**
```rust
    for (i, word) in msg.mr[..len].iter().enumerate() {
        if i < WORDS_IN_REGISTERS {
```

**Sau (đột biến):**
```rust
    for (i, word) in msg.mr[..len].iter().enumerate() {
        if i == 32 {
            continue; // M1: bỏ copy 1/64 MR
        }
        if i < WORDS_IN_REGISTERS {
```

## M2 — Bỏ kiểm RIGHT_GRANT khi chuyển cap

- File: `v2/sim/vifast-sim/src/ipc.rs`
- Test: `kt25_ipc2::thieu_grant_that_bai_mem_message_van_giao`

**Trước (HEAD):**
```rust
        if c.rights & crate::kobj::RIGHT_GRANT == 0 || occupied[i] {
```

**Sau (đột biến):**
```rust
        if occupied[i] { // M2: bỏ kiểm RIGHT_GRANT
```

## M3 — Cap nhận giữ RIGHT_GRANT (không bỏ GRANT)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt25_ipc2::bon_cap_cai_du_bo_grant_giu_badge`

**Trước (HEAD):**
```rust
            let rights = snap.rights & !RIGHT_GRANT;
```

**Sau (đột biến):**
```rust
            let rights = snap.rights; // M3: giữ GRANT trên cap nhận
```

## M4 — Bỏ unwrap badge (cap badge vào receive window)

- File: `v2/sim/vifast-sim/src/ipc.rs`
- Test: `kt25_ipc2::unwrap_badge_khong_chiem_receive_window`

**Trước (HEAD):**
```rust
        if let (Some(ep), Some(badge)) = (endpoint, c.badge) {
            if ep == c.obj {
                // Unwrap badge: không chuyển cap, badge OR vào message (thiết kế `:319`).
                plan.unwrap_badges.push(badge);
                continue;
            }
        }
```

**Sau (đột biến):**
```rust
        // M4: bỏ nhánh unwrap badge
        let _ = endpoint;
```

## M5 — Signal không xoá word/badge sau khi giao

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt27_notification::mot_waiter_duy_nhat_ba_tcb`

**Trước (HEAD):**
```rust
        self.unlink_tcb_queues(target);
        self.transition_tcb(target, TcbState::Ready)?;
        self.body_mut(ntfn, |k| match k {
            Kind::Notification(n) => {
                n.word = 0;
                n.badge = 0;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
```

**Sau (đột biến):**
```rust
        self.unlink_tcb_queues(target);
        self.transition_tcb(target, TcbState::Ready)?;
        // M5: KHÔNG xoá word/badge sau khi giao
```

## M6 — Bỏ cổng 'một waiter' kiểu v1

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt27_notification::mot_waiter_duy_nhat_ba_tcb`

**Trước (HEAD):**
```rust
        if busy {
            return Err(KError::Busy);
        }
```

**Sau (đột biến):**
```rust
        // M6: bỏ cổng một-waiter
        let _ = busy;
```

## M7 — WaitAddr xếp hàng trước rồi mới so expected

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt28_waitaddr::expected_lech_tra_ngay_khong_xep_hang`

**Trước (HEAD):**
```rust
        let current = self.addr_word(frame, offset);
        if current != expected {
            return Err(KError::InvalidArgument);
        }
        // Chuyển trạng thái **trước** khi push: `transition_tcb` có thể từ chối cặp (bảng
        // 11×11) và không được để nút mồ côi; push không lỗi ⇒ hàng và trạng thái luôn khớp.
        self.transition_tcb(tcb_obj, TcbState::BlockedOnAddress { frame, offset })?;
        let bucket = waitaddr_bucket(frame, offset);
        self.address_waiters[bucket].push((frame, offset, tcb_obj));
```

**Sau (đột biến):**
```rust
        let current = self.addr_word(frame, offset);
        // M7: xếp hàng TRƯỚC rồi mới so expected
        self.transition_tcb(tcb_obj, TcbState::BlockedOnAddress { frame, offset })?;
        let bucket = waitaddr_bucket(frame, offset);
        self.address_waiters[bucket].push((frame, offset, tcb_obj));
        if current != expected {
            return Err(KError::InvalidArgument);
        }
```

## M8 — WakeAddr bỏ kiểm khoá (wake mọi waiter trong bucket)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt28_waitaddr::va_cham_bucket_khong_anh_huong_tim_kiem`

**Trước (HEAD):**
```rust
            for (f, o, t) in self.address_waiters[bucket].iter() {
                if *f == frame && *o == offset {
                    chon.push(*t);
                    if chon.len() == n as usize {
                        break;
                    }
                }
            }
```

**Sau (đột biến):**
```rust
            for (_f, _o, t) in self.address_waiters[bucket].iter() {
                // M8: bỏ kiểm (frame, offset)
                chon.push(*t);
                if chon.len() == n as usize {
                    break;
                }
            }
```

## M9 — Push migration giữ home cũ (bỏ home = to)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt29_smp::push_dung_mot_tcb_cuoi_hang_qua_migrate`

**Trước (HEAD):**
```rust
        let doi = self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.home = to;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        });
```

**Sau (đột biến):**
```rust
        let doi = self.body_mut(tcb, |k| match k {
            Kind::Tcb(_t) => Ok(()), // M9: giữ home cũ
            _ => Err(KError::WrongObjectType),
        });
```

## M10 — Fault-resume không ghi MR trước Ready (đường r319)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt26_fault::resume_with_regs_khi_dang_cho_reply_ghi_thanh_ghi_truoc_ready`

**Trước (HEAD):**
```rust
        let snaps = self.resolve_caps(&msg)?;
        let mut m = self.admit_message(msg, snaps.clone())?;
        self.deliver_message(obj, &mut m, &snaps, None)?;
```

**Sau (đột biến):**
```rust
        // M10: KHÔNG ghi MR trước Ready
```

## M11 — Hộp thư đầy ghi đè thay vì Err(Busy)

- File: `v2/lib/vifast-sync/src/mailbox.rs`
- Test: `kt30_sync::mailbox_day_tra_busy_khong_mat_message_dau`

**Trước (HEAD):**
```rust
                // Slot còn thuộc vòng trước ⇒ hộp thư đầy.
                return Err(Busy);
```

**Sau (đột biến):**
```rust
                // M11: hộp thư đầy ⇒ GHI ĐÈ message cũ (đột biến tạm).
                unsafe {
                    (*slot.value.get()).write(msg);
                }
                slot.seq.store(pos.wrapping_add(1), Release);
                self.head.store(pos.wrapping_add(1), Relaxed);
                self.generation.fetch_add(1, SeqCst);
                return Ok(());
```

## M12 — Rời BlockedOnReply không gỡ Reply.caller (cổng transition_tcb; guard hiện hành sau KT-34)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt25_ipc2::cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller`

**Trước (HEAD):**
```rust
        } else if let TcbState::BlockedOnReply { reply } = from {
            if self.object_alive(reply)
                && self.object_type(reply) == Some(ObjType::Reply)
                && self.reply_caller(reply)? == Some(tcb)
            {
                self.reply_set_caller(reply, None)?;
            }
        }
```

**Sau (đột biến):**
```rust
        }
        // M12: không gỡ Reply.caller khi rời BlockedOnReply
```

## V1 — Map đổi lá/thứ tự cấp (AP→BSP): level_for_size_bits đảo cấp

- File: `v2/sim/vifast-sim/src/pagetable.rs`
- Test: `kt37_pagetable::map_walk_unmap_du_ba_co`

**Trước (HEAD):**
```rust
    Some((size.size_bits() - 12) / 9)
```

**Sau (đột biến):**
```rust
    Some(2 - (size.size_bits() - 12) / 9) // V1: đảo cấp lá (AP→BSP)
```

## V2 — Bỏ kiểm ack khi unmap (trả Ok khi chưa đủ ack)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt40_tlb::shootdown_1_2_4_8_hart_ack_du`

**Trước (HEAD):**
```rust
        let mapping = match self.page_table_unmap(info.root, va) {
            Ok(m) => m,
            Err(e) => {
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
        };
        self.tlb_shootdown_post(hart)?;
        self.tlb_shootdown_wait(hart)?;
        Ok(mapping)
```

**Sau (đột biến):**
```rust
        let mapping = match self.page_table_unmap(info.root, va) {
            Ok(m) => m,
            Err(e) => {
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
        };
        self.tlb_shootdown_post(hart)?;
        // V2: bỏ chờ ack khi unmap
        Ok(mapping)
```

## V3 — Bỏ W^X khi map W (cho W sau X)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt39_wx::map_w_sau_x_bi_tu_choi_atomic_khong_ha_ngam`

**Trước (HEAD):**
```rust
        if write && (exec || f.mapped_x > 0) {
```

**Sau (đột biến):**
```rust
        if write && exec { // V3: bỏ nhánh mapped_x > 0
```

## V4 — Bỏ flush TLB khi deactivate/teardown

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt40_tlb::flush_theo_trang_asid_va_gen`

**Trước (HEAD):**
```rust
            self.tlb_flush_hart_only(hart, generation, asid, TlbFlushScope::Asid);
```

**Sau (đột biến):**
```rust
            // V4: bỏ flush TLB khi deactivate
            let _ = (generation, asid);
```

## V5 — Bỏ rút hộp thư khi chờ ack (AP→BSP deadlock)

- File: `v2/sim/vifast-sim/src/kobj.rs`
- Test: `kt40_tlb::deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu`

**Trước (HEAD):**
```rust
        let _ = self.drain_mail_fifo(hart);
        p.rounds += 1;
```

**Sau (đột biến):**
```rust
        // V5: bỏ drain hộp thư khi chờ ack (AP→BSP)
        p.rounds += 1;
```
