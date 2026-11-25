// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342b** — test host cho **bảo toàn message `NBSend`** trên `Endpoint`.
//!
//! # Vì sao test này tồn tại (lỗi thật đã đo được)
//!
//! Ảnh sản phẩm `riscv64` (`KT-342`) cho `netsrv` U-mode trả lời `shell` bằng **một** `NBSend`
//! trên `EP_NET_REPLY` rồi `Exit(0)` ngay. Đo trên QEMU: `nb_send` trả **0** (thành công),
//! `Endpoint.last_mr` có giá trị, nhưng hàng gửi **rỗng** ngay sau đó và `shell` `NBRecv` mãi
//! `Busy`(14) — message **mất**. Bộ đếm của Endpoint chỉ đúng chỗ đó: `push=1 pop=0 **rm=1**`
//! (message bị **gỡ** chứ không bị lấy).
//!
//! Gốc rễ: [`Machine::exit`]/[`Machine::finalise_tcb`] gọi `unlink_tcb_queues`, mà bản cũ gỡ
//! **mọi** message có `sender == tcb` — kể cả message `NBSend` **đã trả về thành công** (người
//! gửi không còn chờ gì, message đã thuộc về bên nhận). Ngữ nghĩa đúng: chỉ message **chặn**
//! (`QueuedMsg.blocking`, tức `Send`/`Call`/`fault` đang `BlockedOnSend`) mới phải gỡ khi TCB
//! gửi chết — message `NBSend` phải **ở lại** hàng.
//!
//! # Ca kiểm
//!
//! | # | Tên | Kiểm |
//! |---|---|---|
//! | 1 | [`nb_send_khong_waiter_vao_hang`] | không có người chờ ⇒ message **vào hàng**, `NBRecv` lấy được đúng MR |
//! | 2 | [`nb_send_song_sot_qua_exit_cua_sender`] | **ca đỏ-trước**: sender `Exit` sau `NBSend` ⇒ message **vẫn** ở hàng và vẫn nhận được |
//! | 3 | [`blocking_send_cua_sender_chet_bi_go`] | `Send` **chặn** đang xếp hàng + sender chết ⇒ gỡ **có đếm** `ipc_dropped` (không đổi ngữ nghĩa cũ) |
//! | 4 | [`loi_cap_dung_truoc_loi_trang_thai`] | ca âm thứ tự: TCB không chạy được + ô cap **trống** ⇒ `InvalidCapability` 1 (không phải `IllegalOperation` 7) |
//! | 5 | [`nb_send_song_sot_qua_suspend_cua_sender`] | **KT-401** — `Tcb.Suspend(Abort)` (đường **chuyển trạng thái**) không được rút message `NBSend` đã `Ok` |
//! | 6 | [`nb_send_song_sot_qua_block_cua_sender`] | **KT-401** — `block_current` (Running → BlockedOnRecv) không được rút message `NBSend` đã `Ok` |
//!
//! # Đột biến (D2-05)
//!
//! | Feature | Sửa | Ca phải **đỏ** |
//! |---|---|---|
//! | `mut-drop-nb-no-waiter` | `endpoint_send` **bỏ** message khi không có waiter | 1, 2 |
//! | `mut-state-before-cap` | `nb_send` kiểm **trạng thái TCB trước** kiểm cap | 4 |
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::ipc::{Message, MessageInfo};
use crate::machine::Machine;
use crate::state_write::NonReadyTarget;
use crate::tcb::{SuspendMode, TcbState};

/// Nhãn message của ca kiểm — chỉ để khẳng định đúng message được giao.
const LABEL: u32 = 0x290;
/// MR0 của message ca kiểm (giá trị `rtt_ms` trong lỗi thật).
const MR0: u64 = 6;

/// `MessageInfo` một MR: `length` ở bit 28.., `label` ở bit 0.., `caps = 0`.
fn info(label: u32, len: u8) -> MessageInfo {
    MessageInfo::new(label, 0, false, 0, len).expect("MessageInfo hợp lệ")
}

fn message() -> Message {
    Message::new(info(LABEL, 1), &[MR0]).expect("Message một MR")
}

/// Cảnh nhỏ nhất đủ cho đường IPC: CNode 16 ô, một Endpoint dịch vụ, hai TCB (`Ready`).
///
/// | ô | nội dung |
/// |---|---|
/// | 0 | cap `CNode` tự (CSpace) |
/// | 1 | cap `Untyped` (nguồn retype) |
/// | 2 | `Endpoint` dịch vụ (kênh `NBSend` → `NBRecv`) |
/// | 3 | `SchedControl` (cấu hình SC — bất biến `Running ⇒ còn budget`) |
/// | 4 | TCB **người gửi** (`Ready`) |
/// | 5 | TCB **người nhận** (`Ready`) |
/// | 6 | ô **trống** (ca âm: cap không tồn tại) |
/// | 7 | `SchedContext` cấp cho hai TCB |
struct Rig {
    m: Machine,
    ep: SlotRef,
    ep_obj: ObjId,
    sender: ObjId,
    receiver: ObjId,
    empty: SlotRef,
}

fn rig() -> Rig {
    let mut m = Machine::new();
    let u = m.add_untyped(0x8200_0000, 1 << 16);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 1, u_cap).expect("cap untyped");
    let ep_cap = m
        .retype(SlotRef::new(c, 1), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint");
    let ep_obj = m.cap_obj(ep_cap).expect("obj endpoint");
    m.put_cap(c, 2, ep_cap).expect("put endpoint");
    let mut tcb = [0u32; 2];
    for (i, slot) in [4usize, 5].iter().enumerate() {
        let cap = m
            .retype(SlotRef::new(c, 1), ObjType::Tcb, 0, 0)
            .expect("retype tcb");
        m.put_cap(c, *slot, cap).expect("put tcb");
        tcb[i] = m.cap_obj(cap).expect("obj tcb");
    }
    for slot in [4usize, 5] {
        m.tcb_configure(SlotRef::new(c, slot), Some(SlotRef::new(c, 0)))
            .expect("cspace");
    }
    // Bất biến KT-20 (`Running ⇒ còn budget`): cấp SC cho cả hai TCB trước khi `Ready`.
    let ctl = m
        .retype(SlotRef::new(c, 1), ObjType::SchedControl, 0, 0)
        .expect("retype sched control");
    m.put_cap(c, 3, ctl).expect("put ctl");
    let sc = m
        .retype(SlotRef::new(c, 1), ObjType::SchedContext, 0, 0)
        .expect("retype sched context");
    m.put_cap(c, 7, sc).expect("put sc");
    m.sched_control_configure(SlotRef::new(c, 3), SlotRef::new(c, 7), 1_000, 10_000)
        .expect("SC budget");
    for slot in [4usize, 5] {
        m.set_sched_params(SlotRef::new(c, slot), Some(SlotRef::new(c, 7)))
            .expect("sched params");
    }
    for id in tcb {
        m.start_tcb(id).expect("Ready");
    }
    Rig {
        m,
        ep: SlotRef::new(c, 2),
        ep_obj,
        sender: tcb[0],
        receiver: tcb[1],
        empty: SlotRef::new(c, 6),
    }
}

/// **Ca 1** — `NBSend` khi Endpoint **không có** người chờ: message phải **vào hàng**, không bị
/// bỏ; `NBRecv` sau đó lấy đúng message (label + MR0) và hàng rỗng lại.
#[test]
fn nb_send_khong_waiter_vao_hang() {
    let mut r = rig();
    r.m.nb_send(r.sender, r.ep, message()).expect("NBSend Ok");
    assert_eq!(
        r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"),
        1,
        "NBSend khi không có waiter phải XẾP HÀNG, không được bỏ message"
    );
    let got =
        r.m.nb_recv(r.receiver, r.ep)
            .expect("NBRecv Ok")
            .expect("phải có message");
    assert_eq!(u64::from(got.info.label()), u64::from(LABEL));
    assert_eq!(got.mr[0], MR0);
    assert_eq!(r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"), 0);
}

/// **Ca 2 (đỏ-trước)** — `NBSend` trả **thành công** rồi TCB gửi `Exit`: message **đã thuộc về**
/// bên nhận nên phải **ở lại** hàng. Đây đúng là đường `netsrv` trả lời `shell` rồi thoát.
#[test]
fn nb_send_song_sot_qua_exit_cua_sender() {
    let mut r = rig();
    r.m.nb_send(r.sender, r.ep, message()).expect("NBSend Ok");
    // `n_reply`/fault_ep = None ⇒ `Exit` đưa TCB về `Inactive` sau `unlink_tcb_queues`.
    r.m.exit(r.sender, 0).expect("Exit");
    assert_eq!(
        r.m.tcb_state(r.sender).expect("state"),
        TcbState::Inactive,
        "sender phải đã chết (Inactive)"
    );
    assert_eq!(
        r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"),
        1,
        "message `NBSend` đã trả thành công KHÔNG được rút lại khi sender Exit"
    );
    let got =
        r.m.nb_recv(r.receiver, r.ep)
            .expect("NBRecv Ok")
            .expect("message NBSend phải còn sau khi sender chết");
    assert_eq!(got.mr[0], MR0);
    assert_eq!(r.m.ipc_dropped(), 0, "không message nào bị huỷ");
}

/// **Ca 3** — ngược lại ca 2: `Send` **chặn** đang nằm trong hàng (sender `BlockedOnSend`) mà
/// TCB gửi chết ⇒ gỡ message **có đếm** (`ipc_dropped`), vì giao nó sẽ phải đánh thức một TCB
/// đã chết. Ngữ nghĩa này **không đổi** (D2-25 / `check_ipc`: `blocking ⇒ sender BlockedOnSend`).
#[test]
fn blocking_send_cua_sender_chet_bi_go() {
    let mut r = rig();
    r.m.send(r.sender, r.ep, message()).expect("Send xếp hàng");
    assert_eq!(r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"), 1);
    assert!(matches!(
        r.m.tcb_state(r.sender).expect("state"),
        TcbState::BlockedOnSend { .. }
    ));
    r.m.exit(r.sender, 0).expect("Exit");
    assert_eq!(
        r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"),
        0,
        "message của sender ĐANG CHỜ phải bị gỡ khi nó chết"
    );
    assert_eq!(r.m.ipc_dropped(), 1, "gỡ message phải đếm `ipc_dropped`");
}

/// **Ca 4** — thứ tự lỗi: cap phải được kiểm **trước** trạng thái TCB. TCB đang `BlockedOnRecv`
/// (không gửi được) gọi `NBSend` trên ô cap **trống** ⇒ `InvalidCapability` (1), **không** phải
/// `IllegalOperation` (7). Đột biến `mut-state-before-cap` đảo thứ tự ⇒ ca này đỏ.
#[test]
fn loi_cap_dung_truoc_loi_trang_thai() {
    let mut r = rig();
    // Đưa TCB người gửi vào `BlockedOnRecv` (không còn `Ready`/`Running`).
    r.m.recv(r.sender, r.ep, None).expect("Recv chặn");
    assert!(matches!(
        r.m.tcb_state(r.sender).expect("state"),
        TcbState::BlockedOnRecv { .. }
    ));
    let err =
        r.m.nb_send(r.sender, r.empty, message())
            .expect_err("ô cap trống phải lỗi");
    assert_eq!(
        err,
        KError::InvalidCapability,
        "lỗi cap phải đứng trước lỗi trạng thái (đột biến `mut-state-before-cap` đảo lại)"
    );
}

/// **Ca 5 (KT-401, đỏ-trước)** — `NBSend` trả `Ok` rồi TCB gửi bị `Tcb.Suspend(Abort)`: đây là
/// đường **chuyển trạng thái** (`tcb_suspend` → `unlink_tcb_queues`), không phải chết. Message
/// **đã thuộc về bên nhận** nên phải ở lại hàng; trước KT-401 luật cũ gỡ mọi message của TCB ⇒
/// `left: 0`.
#[test]
fn nb_send_song_sot_qua_suspend_cua_sender() {
    let mut r = rig();
    r.m.nb_send(r.sender, r.ep, message()).expect("NBSend Ok");
    // `Tcb.Suspend(Abort)` trên TCB gửi (ô 4) — `Ready` ⇒ huỷ hàng chờ rồi giữ `Ready` + cờ
    // `suspended`; **không** phải đường chết.
    r.m.tcb_suspend(SlotRef::new(r.ep.cnode, 4), SuspendMode::Abort)
        .expect("Suspend(Abort)");
    assert_eq!(
        r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"),
        1,
        "message `NBSend` đã trả thành công KHÔNG được rút lại khi sender bị Suspend"
    );
    assert_eq!(r.m.ipc_dropped(), 0, "không message nào bị huỷ");
    let got =
        r.m.nb_recv(r.receiver, r.ep)
            .expect("NBRecv Ok")
            .expect("message NBSend phải còn sau khi sender bị Suspend");
    assert_eq!(got.mr[0], MR0);
}

/// **Ca 6 (KT-401, đỏ-trước)** — `NBSend` trả `Ok` rồi TCB gửi **tự chặn** (`block_current`
/// `Running → BlockedOnRecv`): cũng là đường **chuyển trạng thái** ⇒ message phải ở lại hàng.
#[test]
fn nb_send_song_sot_qua_block_cua_sender() {
    let mut r = rig();
    r.m.nb_send(r.sender, r.ep, message()).expect("NBSend Ok");
    // Chọn TCB gửi làm `Running` (cả hai `Ready`; nếu chọn nhầm thì `yield` rồi chọn lại).
    let cur = r.m.dispatch(0).expect("dispatch").expect("có TCB");
    if cur != r.sender {
        r.m.yield_current(0).expect("yield");
        let cur = r.m.dispatch(0).expect("dispatch").expect("có TCB");
        assert_eq!(cur, r.sender, "phải chọn được TCB gửi");
    }
    r.m.block_current(
        0,
        NonReadyTarget::BlockedOnRecv {
            ep: r.ep_obj,
            reply: None,
        },
    )
    .expect("block_current");
    assert_eq!(
        r.m.endpoint_send_len(r.ep_obj).expect("đọc hàng"),
        1,
        "message `NBSend` đã trả thành công KHÔNG được rút lại khi sender tự chặn"
    );
    assert_eq!(r.m.ipc_dropped(), 0, "không message nào bị huỷ");
    let got =
        r.m.nb_recv(r.receiver, r.ep)
            .expect("NBRecv Ok")
            .expect("message NBSend phải còn sau khi sender block");
    assert_eq!(got.mr[0], MR0);
}
