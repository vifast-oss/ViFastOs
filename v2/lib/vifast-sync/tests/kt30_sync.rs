// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-30 — ngữ nghĩa **single-thread** của `vifast-sync` (K1C-PLAN §5 KT-30 DoD 2/3/4).
//!
//! Tệp này **không** tạo thread nên chạy được dưới Miri (`v2 miri -p vifast-sync`, DoD 6);
//! stress đa luồng nằm ở `kt30_stress.rs` (bị `cfg(not(miri))` gạt ra).
//!
//! Tệp này chỉ dùng **API công khai** (`lock`/`try_lock`/`queue_len`/`next_owner` + `Mailbox`).
//! Test FIFO/không-reentrant/pool cạn cần API **nội bộ** (`pub(crate)`: `test_enqueue`/
//! `test_woken`/`test_release`) nên nằm trong `#[cfg(test)] mod tests` của `src/mcs.rs` — API
//! đó **không** `pub` để code ngoài crate không thể mở khoá hộ slot và tạo hai guard cùng lúc
//! (alias `&mut T` = UB từ safe code; verdict `reviewer-kt30` mục 2).

use vifast_sync::{Busy, Mailbox, McsLock, Msg, MsgKind, PowerState};

// ---------------------------------------------------------------- MCS lock

#[test]
fn mcs_lock_unlock_mutates_value() {
    let lock = McsLock::new(0u64);
    assert_eq!(lock.queue_len(), 0);
    assert_eq!(lock.next_owner(), None);
    {
        let mut guard = lock.lock();
        assert_eq!(*guard, 0);
        *guard += 1;
        // Chủ hiện tại vẫn nằm ở đầu hàng (MCS: node của chủ chỉ rời hàng khi mở khoá).
        assert_eq!(lock.queue_len(), 1);
        assert_eq!(lock.next_owner(), Some(0), "ticket đầu tiên là 0");
    }
    assert_eq!(lock.queue_len(), 0);
    assert_eq!(lock.next_owner(), None);
    {
        let guard = lock.lock();
        assert_eq!(*guard, 1, "giá trị bền qua các lần lock/unlock");
    }
}

#[test]
fn mcs_try_lock_khi_dang_giu_tra_none() {
    let lock = McsLock::new(());
    let guard = lock.try_lock();
    assert!(guard.is_some(), "khoá rảnh ⇒ try_lock phải lấy được");
    assert!(lock.try_lock().is_none(), "đang giữ ⇒ try_lock trả None");
    assert!(lock.try_lock().is_none(), "vẫn None ở lần thử thứ hai");
    drop(guard);
    assert!(lock.queue_len() == 0);
    let again = lock.try_lock();
    assert!(again.is_some(), "mở khoá xong ⇒ try_lock lấy lại được");
    drop(again);
    assert_eq!(lock.queue_len(), 0);
    assert_eq!(lock.next_owner(), None);
}

#[test]
fn mcs_ticket_tang_theo_lan_xep_hang() {
    let lock = McsLock::new(());
    for expected in 0..3u32 {
        let guard = lock.lock();
        assert_eq!(lock.next_owner(), Some(expected), "ticket tăng đơn điệu");
        drop(guard);
        assert_eq!(lock.next_owner(), None, "hàng rỗng ⇒ không có chủ");
    }
}

#[test]
fn mcs_capacity_va_hang_cho_mac_dinh() {
    let lock = McsLock::<u8>::new(0);
    assert_eq!(lock.capacity(), vifast_sync::MAX_WAITERS);
    assert_eq!(lock.capacity(), 16, "[TK]: pool node cố định 16 slot");
    assert_eq!(lock.queue_len(), 0);
    assert_eq!(lock.next_owner(), None);
}

// ---------------------------------------------------------------- Mailbox

fn all_kinds() -> [Msg; 7] {
    [
        Msg::Wake { hart: 1, tcb: 10 },
        Msg::TlbFlush { hart: 2, tcb: 11 },
        Msg::Reschedule { hart: 3, tcb: 12 },
        Msg::Migrate { hart: 4, tcb: 13 },
        Msg::Stop { hart: 5 },
        Msg::VmKick { hart: 6 },
        Msg::PowerReq {
            hart: 7,
            state: PowerState::Suspend,
        },
    ]
}

#[test]
fn mailbox_du_7_loai_fifo() {
    let mb: Mailbox<8> = Mailbox::new();
    assert_eq!(mb.capacity(), 8);
    assert!(mb.is_empty());
    assert_eq!(mb.len(), 0);

    let msgs = all_kinds();
    for msg in msgs {
        assert_eq!(mb.send(msg), Ok(()));
    }
    assert_eq!(msg_kinds(&msgs), MsgKind::ALL.len());

    let mut got = Vec::new();
    let drained = mb.drain(|msg| got.push(msg));
    assert_eq!(drained, 7);
    assert_eq!(got, msgs.to_vec(), "drain đúng FIFO và đủ 7 loại");
    assert!(mb.is_empty());
    assert_eq!(mb.len(), 0);
    assert_eq!(mb.try_recv(), None);
}

fn msg_kinds(msgs: &[Msg; 7]) -> usize {
    let kinds: Vec<MsgKind> = msgs.iter().map(Msg::kind).collect();
    assert_eq!(kinds.len(), MsgKind::ALL.len());
    for kind in MsgKind::ALL {
        assert!(kinds.contains(&kind), "thiếu loại {kind:?}");
    }
    kinds.len()
}

#[test]
fn mailbox_day_tra_busy_khong_mat_message_dau() {
    let mb: Mailbox<4> = Mailbox::new();
    for tcb in 0..4u32 {
        assert_eq!(mb.send(Msg::Wake { hart: 0, tcb }), Ok(()));
    }
    assert_eq!(mb.len(), 4);
    // Đầy ⇒ Busy, **không** ghi đè và **không** làm mất message đầu.
    assert_eq!(mb.send(Msg::Stop { hart: 9 }), Err(Busy));
    assert_eq!(mb.len(), 4);

    let mut got = Vec::new();
    assert_eq!(mb.drain(|msg| got.push(msg)), 4);
    let want: Vec<Msg> = (0..4u32).map(|tcb| Msg::Wake { hart: 0, tcb }).collect();
    assert_eq!(got, want, "message đầu còn nguyên, đúng thứ tự");

    // Rút xong thì gửi lại được.
    assert_eq!(mb.send(Msg::VmKick { hart: 9 }), Ok(()));
    assert_eq!(mb.try_recv(), Some(Msg::VmKick { hart: 9 }));
    assert_eq!(mb.try_recv(), None);
}

#[test]
fn mailbox_backpressure_1000_vong() {
    // DoD 4: 1.000 vòng gửi/đầy/rút; dưới Miri giảm còn 40 vòng (chỉ để kiểm UB, không phải
    // ngưỡng cổng — bản stable chạy đủ 1.000 vòng).
    const ROUNDS: u32 = if cfg!(miri) { 40 } else { 1000 };
    let mb: Mailbox<8> = Mailbox::new();
    let mut ok = 0u32;
    let mut busy = 0u32;
    for round in 0..ROUNDS {
        let hart = (round % 8) as u16;
        for tcb in 0..8u32 {
            assert_eq!(mb.send(Msg::Wake { hart, tcb }), Ok(()));
            ok += 1;
        }
        assert_eq!(mb.send(Msg::Reschedule { hart, tcb: round }), Err(Busy));
        busy += 1;
        let mut got = Vec::new();
        assert_eq!(mb.drain(|msg| got.push(msg)), 8);
        for (tcb, msg) in got.iter().enumerate() {
            assert_eq!(
                *msg,
                Msg::Wake {
                    hart,
                    tcb: tcb as u32
                }
            );
        }
    }
    assert_eq!(ok, ROUNDS * 8);
    assert_eq!(busy, ROUNDS);
    assert!(mb.is_empty());
}

#[test]
fn mailbox_the_he_ack_pending() {
    let mb: Mailbox<4> = Mailbox::new();
    assert_eq!(mb.generation(), 0);
    assert_eq!(mb.acked(), 0);
    assert_eq!(mb.pending(0), 0);

    for tcb in 0..3u32 {
        assert_eq!(mb.send(Msg::Wake { hart: 0, tcb }), Ok(()));
    }
    assert_eq!(mb.generation(), 3, "thế hệ = số message đã nhận");
    assert_eq!(mb.pending(3), 3);
    assert_eq!(mb.pending(2), 2, "pending(gen) chỉ tính tới thế hệ gen");

    assert_eq!(mb.ack(1), 1, "ack(1) ack thêm 1 message");
    assert_eq!(mb.acked(), 1);
    assert_eq!(mb.pending(3), 2);
    assert_eq!(mb.pending(1), 0);

    assert_eq!(mb.ack(3), 2);
    assert_eq!(mb.acked(), 3);
    assert_eq!(mb.pending(3), 0);

    // ack lùi không kéo bộ đếm xuống; ack vượt generation bị kẹp.
    assert_eq!(mb.ack(1), 0);
    assert_eq!(mb.acked(), 3);
    assert_eq!(mb.ack(99), 0);
    assert_eq!(mb.acked(), 3);
    assert_eq!(mb.pending(99), 0);
}

#[test]
fn mailbox_generation_khong_tang_khi_busy() {
    let mb: Mailbox<2> = Mailbox::new();
    assert_eq!(mb.send(Msg::Stop { hart: 0 }), Ok(()));
    assert_eq!(mb.send(Msg::Stop { hart: 1 }), Ok(()));
    assert_eq!(mb.generation(), 2);
    assert_eq!(mb.send(Msg::Stop { hart: 2 }), Err(Busy));
    assert_eq!(mb.generation(), 2, "Busy không tiêu thế hệ");
    assert_eq!(mb.drain(|_| {}), 2);
    assert_eq!(mb.send(Msg::Stop { hart: 2 }), Ok(()));
    assert_eq!(mb.generation(), 3);
    assert_eq!(mb.len(), 1);
}

#[test]
fn mailbox_len_rong_va_capacity() {
    let mb: Mailbox<4> = Mailbox::new();
    assert_eq!(mb.capacity(), 4);
    assert!(mb.is_empty());
    assert_eq!(mb.try_recv(), None);
    assert_eq!(mb.send(Msg::Wake { hart: 0, tcb: 1 }), Ok(()));
    assert!(!mb.is_empty());
    assert_eq!(mb.len(), 1);
    assert_eq!(mb.try_recv(), Some(Msg::Wake { hart: 0, tcb: 1 }));
    assert!(mb.is_empty());
    assert_eq!(mb.try_recv(), None);
}
