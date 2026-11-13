// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-28 — **WaitOnAddress**: bảng băm `(frame, offset)` với `WAITADDR_BUCKETS` model `[TK]`,
//! `WaitAddr(frame, offset, expected)` đọc giá trị **và** xếp hàng trong **một** lần giữ khoá
//! (lệch giá trị ⇒ trả ngay, không xếp hàng), **không timeout** (D2-53 Q4), `WakeAddr(n)` đánh
//! thức **tối đa n** waiter theo FIFO tất định, không mất đánh thức (thiết kế §4.7 `:330-334`,
//! QD §3.8, K1C-PLAN §5 KT-28 DoD 1..8).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Khoá theo `(ObjId frame, u32 offset)` vì object
//! `Frame` là reserved (K2) — "Frame chia sẻ hai tiến trình" **chưa đạt** (K1C-PLAN §5 KT-28
//! rủi ro (b)). "Khoá" trong mô hình một luồng là **hàm nguyên tử**: đọc từ + xếp hàng nằm
//! trong cùng một hàm, không có public op chen giữa; bằng chứng "không mất đánh thức" là
//! property theo **thứ tự bước** (không phải thread thật — stress `Mutex` thuộc K2/K3).
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt28-red-compile.log`](../../agent/JOURNAL/kt28-red-compile.log)
//! (`WAITADDR_BUCKETS`, `waitaddr_bucket`, `Machine::{wait_addr, wake_addr, set_addr_word,
//! addr_word, check_waitaddr, address_waiters_for, waitaddr_bucket_len,
//! waitaddr_nonempty_buckets}` chưa có ⇒ E0432/E0599) và
//! [`JOURNAL/kt28-red-semantics.log`](../../agent/JOURNAL/kt28-red-semantics.log) (probe tạm:
//! `tcb_wait_address` **xếp hàng vô điều kiện**, không so `expected`; **không có** `WakeAddr`
//! nên waiter chỉ ra khỏi hàng bằng `wake(tcb)` thủ công).
//!
//! Bất biến được kiểm sau **mỗi** thao tác: `check_sched` + `check_ipc` + `check_waitaddr` +
//! `check_caps` + `check_all_bytes`.

use vifast_sim::kobj::{
    waitaddr_bucket, KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbState,
    WAITADDR_BUCKETS,
};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

// ---------------------------------------------------------------------------
// Rig: CNode 16 slot; 1 Endpoint (frame A), 1 Untyped (frame B); 3 TCB + SC đã Configure
// (test-only D2-50: `pick` cần SC), SchedControl. Slot: 0 Untyped, 1 Endpoint, 5 SC,
// 6 SchedControl, 8..10 TCB.
// ---------------------------------------------------------------------------

const S_UNTYPED: usize = 0;
const S_EP: usize = 1;
const S_SC: usize = 5;
const S_CTL: usize = 6;
const S_T0: usize = 8;
/// Budget = period = 64 là hằng **test**, không phải ngưỡng cổng.
const SC_MAC_DINH: u64 = 64;

struct Rig {
    m: Machine,
    ep: ObjId,
    u0: ObjId,
    t: [ObjId; 3],
    s: [SlotRef; 3],
}

impl Rig {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 65536);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let sc = m.add_kernel_object(ObjType::SchedContext);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let c1 = m.add_cnode(4);
        for (i, obj) in [
            (S_UNTYPED, u0),
            (S_EP, ep),
            (S_SC, sc),
            (S_CTL, ctl),
            (7, c1),
        ]
        .into_iter()
        {
            let cap = m.root_cap(obj);
            m.put_cap(c0, i, cap).expect("cap object");
        }
        m.sched_control_configure(slot(c0, S_CTL), slot(c0, S_SC), SC_MAC_DINH, SC_MAC_DINH)
            .expect("SchedControl.Configure SC mặc định (test-only)");
        let mut t = [0u32; 3];
        let mut s = [slot(c0, S_T0); 3];
        for i in 0..3 {
            t[i] = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(t[i]);
            m.put_cap(c0, S_T0 + i, cap).expect("cap TCB");
            s[i] = slot(c0, S_T0 + i);
            m.set_sched_params(s[i], Some(slot(c0, S_SC)))
                .expect("set_sched_params SC mặc định (test-only)");
            m.tcb_set_priority(s[i], 5, 255).expect("Tcb.SetPriority");
        }
        let mut r = Rig { m, ep, u0, t, s };
        for i in 0..3 {
            r.m.transition_tcb(r.t[i], TcbState::Ready)
                .expect("Inactive→Ready");
        }
        r
    }

    /// Bất biến của KT-28 sau **mỗi** bước: scheduler + IPC + WaitAddr + cap + byte.
    fn kiem(&self, buoc: &str) {
        if let Err(e) = self.m.check_sched() {
            panic!("check_sched sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_ipc() {
            panic!("check_ipc sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_waitaddr() {
            panic!("check_waitaddr sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_caps() {
            panic!("check_caps sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_all_bytes() {
            panic!("check_all_bytes sau {buoc}: {e}");
        }
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn wait(&mut self, i: usize, frame: ObjId, offset: u32, expected: u64) -> Result<(), KError> {
        self.m.wait_addr(self.s[i], frame, offset, expected)
    }

    fn wake(&mut self, frame: ObjId, offset: u32, n: u32) -> Result<u32, KError> {
        self.m.wake_addr(frame, offset, n)
    }

    fn set(&mut self, frame: ObjId, offset: u32, value: u64) {
        self.m.set_addr_word(frame, offset, value);
    }

    fn waiters(&self, frame: ObjId, offset: u32) -> Vec<ObjId> {
        self.m.address_waiters_for(frame, offset)
    }

    fn bucket_len(&self, frame: ObjId, offset: u32) -> usize {
        self.m.waitaddr_bucket_len(waitaddr_bucket(frame, offset))
    }
}

// ===========================================================================
// 1. Bảng băm: 64 bucket `[TK]`, cùng khoá ⇒ cùng bucket, khoá khác nhau vẫn phân bố.
// ===========================================================================

#[test]
fn bang_bam_64_bucket_va_cung_khoa_cung_bucket() {
    assert_eq!(WAITADDR_BUCKETS, 64, "hằng số model [TK] (D2-53 Q3)");
    let ep: ObjId = 7;
    // 1.000 cặp `(frame, offset)`: bucket luôn trong dải và **cùng khoá ⇒ cùng bucket**.
    let mut buckets = [0u32; WAITADDR_BUCKETS];
    for k in 0..1_000u32 {
        let frame = ep + (k % 3);
        let offset = k.wrapping_mul(8);
        let b = waitaddr_bucket(frame, offset);
        assert!(
            b < WAITADDR_BUCKETS,
            "bucket {b} ngoài dải 0..{WAITADDR_BUCKETS}"
        );
        assert_eq!(b, waitaddr_bucket(frame, offset), "hàm băm phải tất định");
        buckets[b] += 1;
    }
    // Không suy biến: 1.000 khoá trải trên **nhiều** bucket (không phải tất cả vào một chỗ).
    let dung = buckets.iter().filter(|n| **n > 0).count();
    assert!(
        dung >= 8,
        "hàm băm phải trải khoá trên >= 8 bucket, thực tế {dung}"
    );
    // Cùng frame, offset lệch bội 64 ⇒ cùng bucket (64 = 2^6, bit thấp của phép XOR).
    assert_eq!(waitaddr_bucket(ep, 0), waitaddr_bucket(ep, 64));
}

// ===========================================================================
// 2. Va chạm bucket: hai khoá **khác nhau** cùng bucket vẫn tìm/đánh thức đúng khoá.
// ===========================================================================

#[test]
fn va_cham_bucket_khong_anh_huong_tim_kiem() {
    let mut r = Rig::new();
    let ep = r.ep;
    // Cố ý chọn cặp va chạm: `offset` 0 và 64 (bit 6 không nằm trong phần `% 64`).
    assert_ne!((ep, 0u32), (ep, 64u32), "phải là hai khoá khác nhau");
    assert_eq!(
        waitaddr_bucket(ep, 0),
        waitaddr_bucket(ep, 64),
        "cặp chọn phải va chạm bucket"
    );

    r.set(ep, 0, 1);
    r.set(ep, 64, 1);
    r.wait(0, ep, 0, 1).expect("WaitAddr khoá A");
    r.wait(1, ep, 64, 1).expect("WaitAddr khoá B (va chạm)");
    r.kiem("2 waiter cùng bucket");

    assert_eq!(r.bucket_len(ep, 0), 2, "bucket chứa cả hai khoá");
    assert_eq!(
        r.m.waitaddr_nonempty_buckets(),
        1,
        "chỉ một bucket có waiter"
    );
    assert_eq!(r.waiters(ep, 0), vec![r.t[0]], "khoá A chỉ có t0");
    assert_eq!(r.waiters(ep, 64), vec![r.t[1]], "khoá B chỉ có t1");
    assert_eq!(r.m.address_waiter(ep, 0), Some(r.t[0]));
    assert_eq!(r.m.address_waiter(ep, 64), Some(r.t[1]));

    // Wake khoá A: **chỉ** t0 (không đụng waiter cùng bucket của khoá B).
    assert_eq!(r.wake(ep, 0, u32::MAX).expect("WakeAddr A"), 1);
    r.kiem("wake A");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnAddress {
            frame: ep,
            offset: 64
        }
    );
    assert_eq!(r.bucket_len(ep, 0), 1, "bucket còn waiter của khoá B");
    assert_eq!(r.waiters(ep, 64), vec![r.t[1]]);

    // Wake khoá B: waiter còn lại ra.
    assert_eq!(r.wake(ep, 64, u32::MAX).expect("WakeAddr B"), 1);
    r.kiem("wake B");
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.bucket_len(ep, 0), 0);
    assert_eq!(r.m.waitaddr_nonempty_buckets(), 0);
    assert_eq!(r.m.address_waiters_len(), 0, "không còn waiter");
}

// ===========================================================================
// 3. `expected` lệch: trả ngay, **không** xếp hàng (đột biến "xếp hàng trước rồi so").
// ===========================================================================

#[test]
fn expected_lech_tra_ngay_khong_xep_hang() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 16, 7);
    assert_eq!(r.m.addr_word(ep, 16), 7);

    let e = r
        .wait(0, ep, 16, 6)
        .expect_err("lệch expected phải trả lỗi");
    assert_eq!(e, KError::InvalidArgument, "mã lỗi K1C-PLAN §5 KT-28 DoD 2");
    r.kiem("expected lệch");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "TCB phải **giữ** Ready, không bị chặn"
    );
    assert!(r.m.ready_contains(0, r.t[0]), "vẫn ở hàng chạy");
    assert_eq!(r.m.address_waiters_len(), 0, "không được để lại waiter");
    assert_eq!(r.bucket_len(ep, 16), 0);
    assert_eq!(r.m.waitaddr_nonempty_buckets(), 0);

    // Giá trị khớp ⇒ xếp hàng bình thường; lệch 0 vẫn là lệch (không dùng 0 làm "chưa có").
    r.wait(0, ep, 16, 7).expect("đúng expected ⇒ xếp hàng");
    r.kiem("expected đúng");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnAddress {
            frame: ep,
            offset: 16
        }
    );
    assert_eq!(r.waiters(ep, 16), vec![r.t[0]]);

    // Word bị đổi **sau** khi waiter đã xếp hàng: waiter vẫn ở hàng (giá trị chỉ để chọn
    // thời điểm xếp hàng, không phải điều kiện tỉnh dậy) — WakeAddr mới đánh thức.
    r.set(ep, 16, 8);
    r.kiem("đổi word sau khi chờ");
    assert_eq!(r.waiters(ep, 16), vec![r.t[0]]);
}

// ===========================================================================
// 4. Xếp hàng **một lần**: gọi WaitAddr lần hai trên TCB đã chờ ⇒ lỗi, không nhân đôi.
// ===========================================================================

#[test]
fn xep_hang_mot_lan() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 8, 3);
    r.wait(0, ep, 8, 3).expect("WaitAddr lần 1");
    r.kiem("WaitAddr lần 1");
    assert_eq!(r.m.address_waiters_len(), 1);

    let e = r
        .wait(0, ep, 8, 3)
        .expect_err("TCB đã BlockedOnAddress ⇒ không chờ lần hai");
    assert_eq!(e, KError::IllegalOperation);
    r.kiem("WaitAddr lần 2");
    assert_eq!(r.waiters(ep, 8), vec![r.t[0]], "đúng một waiter");
    assert_eq!(r.m.address_waiters_len(), 1);
    assert_eq!(r.bucket_len(ep, 8), 1);
}

// ===========================================================================
// 5. `WakeAddr(n)`: tối đa n, FIFO, trả số đã đánh thức; waiter còn lại vẫn chờ.
// ===========================================================================

#[test]
fn wake_0_1_n_nhieu_va_fifo() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 32, 5);
    for i in 0..3 {
        r.wait(i, ep, 32, 5).expect("3 waiter");
    }
    r.kiem("3 waiter");
    assert_eq!(r.waiters(ep, 32), vec![r.t[0], r.t[1], r.t[2]], "FIFO");

    // n = 0: không ai ra, trả 0.
    assert_eq!(r.wake(ep, 32, 0).expect("WakeAddr 0"), 0);
    r.kiem("wake 0");
    assert_eq!(r.m.address_waiters_len(), 3);
    for i in 0..3 {
        assert!(matches!(r.st(i), TcbState::BlockedOnAddress { .. }));
    }

    // n = 1: đúng waiter **đầu** (FIFO), hai người sau còn chờ.
    assert_eq!(r.wake(ep, 32, 1).expect("WakeAddr 1"), 1);
    r.kiem("wake 1");
    assert_eq!(r.st(0), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(r.waiters(ep, 32), vec![r.t[1], r.t[2]]);
    assert_eq!(r.m.address_waiters_len(), 2);

    // n = 2 > số còn lại: đánh thức 2, trả 2, không vượt n và không bỏ sót.
    assert_eq!(r.wake(ep, 32, 2).expect("WakeAddr 2"), 2);
    r.kiem("wake 2");
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.st(2), TcbState::Ready);
    assert_eq!(r.m.address_waiters_len(), 0);

    // Không còn waiter: trả 0 (không lỗi, không nuốt tín hiệu vào hàng).
    assert_eq!(r.wake(ep, 32, u32::MAX).expect("WakeAddr rỗng"), 0);
    r.kiem("wake rỗng");
}

// ===========================================================================
// 6. Hai `(frame, offset)` độc lập: frame khác nhau / offset khác nhau.
// ===========================================================================

#[test]
fn hai_frame_offset_doc_lap() {
    let mut r = Rig::new();
    let (ep, u0) = (r.ep, r.u0);
    r.set(ep, 16, 1);
    r.set(u0, 16, 1);
    r.wait(0, ep, 16, 1).expect("waiter (ep,16)");
    r.wait(1, u0, 16, 1).expect("waiter (u0,16)");
    r.wait(2, ep, 24, 0).expect("waiter (ep,24)");
    r.kiem("3 khoá độc lập");
    assert_eq!(r.m.address_waiters_len(), 3);
    assert!(
        r.m.waitaddr_nonempty_buckets() >= 1,
        "số bucket có waiter phải > 0"
    );

    assert_eq!(r.wake(ep, 16, u32::MAX).expect("wake (ep,16)"), 1);
    r.kiem("wake (ep,16)");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnAddress {
            frame: u0,
            offset: 16
        },
        "frame khác không bị đánh thức"
    );
    assert_eq!(
        r.st(2),
        TcbState::BlockedOnAddress {
            frame: ep,
            offset: 24
        },
        "offset khác không bị đánh thức"
    );

    assert_eq!(r.wake(ep, 24, 1).expect("wake (ep,24)"), 1);
    assert_eq!(r.wake(u0, 16, 1).expect("wake (u0,16)"), 1);
    r.kiem("wake hai khoá còn lại");
    assert_eq!(r.m.address_waiters_len(), 0);
}

// ===========================================================================
// 7. Không mất đánh thức (C2): Wake sau khi chờ ⇒ Ready; Wake trước ⇒ 0 và **không**
//    nuốt tín hiệu (giá trị mới còn đó cho lần Wait sau).
// ===========================================================================

#[test]
fn khong_mat_danh_thuc_wake_truoc_va_sau() {
    let mut r = Rig::new();
    let ep = r.ep;

    // Wake **sau** khi waiter xếp hàng ⇒ waiter phải Ready (C2 chiều "sau").
    r.set(ep, 0, 0);
    r.wait(0, ep, 0, 0).expect("WaitAddr");
    r.set(ep, 0, 1);
    assert_eq!(r.wake(ep, 0, 1).expect("WakeAddr sau"), 1);
    r.kiem("wake sau");
    assert_eq!(r.st(0), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));

    // Wake **trước** (chưa ai chờ) ⇒ trả 0, và giá trị vẫn còn để lần Wait sau thấy (C2
    // chiều "trước" — mô hình không có cờ "wake treo", đúng thiết kế futex: state là giá trị).
    r.set(ep, 8, 9);
    assert_eq!(r.wake(ep, 8, u32::MAX).expect("WakeAddr trước"), 0);
    r.kiem("wake trước");
    assert_eq!(r.m.addr_word(ep, 8), 9, "không được nuốt giá trị");
    assert_eq!(r.m.address_waiters_len(), 0);

    // Lần Wait sau thấy giá trị mới: expected = 8 (cũ) ⇒ lệch; expected = 9 (mới) ⇒ chờ.
    assert_eq!(
        r.wait(1, ep, 8, 8).expect_err("giá trị cũ đã đổi"),
        KError::InvalidArgument
    );
    r.wait(1, ep, 8, 9).expect("Wait theo giá trị mới");
    r.kiem("wait theo giá trị mới");
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnAddress {
            frame: ep,
            offset: 8
        }
    );
    assert_eq!(r.wake(ep, 8, 1).expect("WakeAddr"), 1);
    assert_eq!(r.st(1), TcbState::Ready);
}

// ===========================================================================
// 8. Suspend(Abort)/Exit/finalise gỡ waiter đúng **bucket** (không chỉ vector tuyến tính).
// ===========================================================================

#[test]
fn suspend_abort_go_dung_bucket() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 16, 1);
    r.wait(0, ep, 16, 1).expect("waiter t0");
    assert_eq!(r.bucket_len(ep, 16), 1);

    r.m.tcb_suspend(r.s[0], SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    r.kiem("suspend Abort");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "Suspend(Abort) đưa waiter về Ready"
    );
    assert_eq!(r.bucket_len(ep, 16), 0, "gỡ khỏi **đúng** bucket");
    assert_eq!(r.m.address_waiters_len(), 0);
    assert_eq!(r.m.waitaddr_nonempty_buckets(), 0, "bucket phải rỗng");

    // Waiter khác khoá không bị ảnh hưởng.
    r.set(ep, 24, 1);
    r.wait(1, ep, 24, 1).expect("waiter t1");
    r.m.tcb_suspend(r.s[0], SuspendMode::Abort)
        .expect("Suspend Abort lần hai");
    r.kiem("suspend không đụng waiter khác");
    assert_eq!(r.waiters(ep, 24), vec![r.t[1]]);
    assert_eq!(r.m.address_waiters_len(), 1);
}

#[test]
fn exit_va_finalise_go_waiter() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 40, 1);
    r.wait(1, ep, 40, 1).expect("waiter t1");
    r.m.exit(r.t[1], 7).expect("Exit");
    r.kiem("Exit");
    assert_eq!(r.st(1), TcbState::Inactive);
    assert_eq!(r.m.address_waiters_len(), 0, "Exit gỡ waiter");
    assert_eq!(r.bucket_len(ep, 40), 0);

    // finalise: waiter đã chết không còn trong bucket ⇒ check_waitaddr (không waiter cho
    // TCB chết) vẫn xanh.
    r.set(ep, 48, 1);
    r.wait(2, ep, 48, 1).expect("waiter t2");
    r.m.finalise_tcb(r.t[2]).expect("finalise TCB");
    r.kiem("finalise");
    assert_eq!(r.m.address_waiters_len(), 0);
    assert_eq!(r.m.waitaddr_nonempty_buckets(), 0);
    assert!(
        r.m.check_waitaddr().is_ok(),
        "không được để waiter cho TCB chết"
    );
}

// ===========================================================================
// 9. Không timeout (D2-53 Q4): tick ảo chạy bao nhiêu cũng **không** đánh thức waiter.
// ===========================================================================

#[test]
fn khong_timeout_du_tick_rat_nhieu() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 56, 1);
    r.wait(0, ep, 56, 1).expect("WaitAddr không timeout");
    r.kiem("WaitAddr");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnAddress {
            frame: ep,
            offset: 56
        },
        "D2-53 Q4: không có Sleeping{{Timeout}} cho WaitAddr"
    );
    for _ in 0..10 {
        r.m.tick(1_000).expect("tick ảo");
    }
    r.kiem("sau 10.000 tick");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnAddress {
            frame: ep,
            offset: 56
        },
        "tick không được làm waiter tỉnh (không timeout)"
    );
    assert_eq!(r.waiters(ep, 56), vec![r.t[0]]);
    assert_eq!(r.wake(ep, 56, 1).expect("chỉ WakeAddr đánh thức"), 1);
    assert_eq!(r.st(0), TcbState::Ready);
}

// ===========================================================================
// 10. `check_waitaddr` bắt waiter lệch trạng thái (đường tắt không qua unlink).
// ===========================================================================

#[test]
fn check_waitaddr_bat_waiter_lech_trang_thai() {
    let mut r = Rig::new();
    let ep = r.ep;
    r.set(ep, 16, 1);
    r.wait(0, ep, 16, 1).expect("waiter");
    // Đường tắt **sai**: đổi trạng thái trực tiếp (BlockedOnAddress → Ready là cặp ALLOWED)
    // ⇒ waiter còn treo trong bucket nhưng TCB đã Ready.
    r.m.transition_tcb(r.t[0], TcbState::Ready)
        .expect("transition trực tiếp");
    assert_eq!(r.m.address_waiters_len(), 1, "waiter vẫn còn (đường sai)");
    let e =
        r.m.check_waitaddr()
            .expect_err("phải bắt waiter lệch trạng thái");
    assert!(
        e.contains("waiter") || e.contains("BlockedOnAddress"),
        "thông điệp phải nêu waiter/BlockedOnAddress: {e}"
    );
}

// ===========================================================================
// 11. Fuzz 1.000 seed × 64 op: bất biến sau mỗi bước + cùng seed cùng trace (DoD 4/5).
// ===========================================================================

/// Một bước fuzz: `(op, kết quả, trạng thái 3 TCB, 3 giá trị từ, số waiter mỗi khoá)` — so
/// hai lượt cùng seed ⇒ cùng trace.
type BuocFuzz = (u8, String, [String; 3], [u64; 3], [u32; 3]);

fn ten_ket_qua(r: Result<(), KError>) -> String {
    match r {
        Ok(()) => "ok".to_string(),
        Err(e) => e.name().to_string(),
    }
}

/// Ba khoá: `(ep,0)`, `(ep,64)` (va chạm bucket với khoá đầu — bit 6 bị `% 64` bỏ), `(u0,16)`.
fn khoa(r: &Rig, k: usize) -> (ObjId, u32) {
    match k {
        0 => (r.ep, 0),
        1 => (r.ep, 64),
        _ => (r.u0, 16),
    }
}

fn mot_buoc(r: &mut Rig, seed: u64, i: u64, trace: &mut Vec<BuocFuzz>) {
    let mut rng = Lcg::new(seed.wrapping_mul(64).wrapping_add(i));
    let op = rng.below(10) as u8;
    let ti = rng.below(3) as usize;
    let ki = rng.below(3) as usize;
    let (frame, offset) = khoa(r, ki);
    let val = rng.next_u64();
    let expected = if rng.below(4) == 0 {
        // 1/4 ca: expected **lệch** (thường là lệch) ⇒ phải trả ngay, không xếp hàng.
        val.wrapping_add(1)
    } else {
        r.m.addr_word(frame, offset)
    };
    let res = match op {
        // Ghi giá trị từ — nền của "wake trước ⇒ lần Wait sau thấy giá trị mới".
        0 => {
            r.set(frame, offset, val);
            Ok(())
        }
        // WaitAddr với expected = giá trị hiện tại (hoặc lệch).
        1 => r.wait(ti, frame, offset, expected),
        // WakeAddr n ngẫu nhiên (0/1/2/tất cả).
        2 => r
            .wake(frame, offset, [0, 1, 2, u32::MAX][rng.below(4) as usize])
            .map(|_| ()),
        // Wake **một** TCB theo model API cũ (đường khác đưa TCB ra khỏi hàng).
        3 => r.m.wake(r.t[ti]),
        // Suspend(Abort) — phải gỡ waiter đúng bucket.
        4 => r.m.tcb_suspend(r.s[ti], SuspendMode::Abort),
        // Resume — không phát lại WaitAddr.
        5 => r.m.tcb_resume(r.s[ti]),
        // Exit — gỡ waiter, TCB → Inactive (không fault_ep).
        6 => r.m.exit(r.t[ti], val & 0xff),
        // Tick ảo — không được làm waiter tỉnh (không timeout).
        7 => r.m.tick(1 + rng.below(3)),
        // WaitAddr trên TCB vừa Inactive: `ensure_started` đưa về Ready rồi chờ.
        8 => r.wait(ti, frame, offset, r.m.addr_word(frame, offset)),
        // WakeAddr rỗng / đọc lại giá trị (không đổi trạng thái).
        _ => {
            assert_eq!(r.m.addr_word(frame, offset), {
                r.m.addr_word(frame, offset)
            });
            r.wake(frame, offset, 1).map(|_| ())
        }
    };
    let mut states = [String::new(), String::new(), String::new()];
    for (k, s) in states.iter_mut().enumerate() {
        *s = r.st(k).render();
    }
    trace.push((
        op,
        ten_ket_qua(res),
        states,
        [
            r.m.addr_word(r.ep, 0),
            r.m.addr_word(r.ep, 64),
            r.m.addr_word(r.u0, 16),
        ],
        [
            r.waiters(r.ep, 0).len() as u32,
            r.waiters(r.ep, 64).len() as u32,
            r.waiters(r.u0, 16).len() as u32,
        ],
    ));
}

#[test]
fn fuzz_1000_seed_64_op_bat_bien_va_tat_dinh() {
    #[cfg(miri)]
    let seeds: u64 = 3;
    #[cfg(not(miri))]
    let seeds: u64 = 1_000;
    #[cfg(miri)]
    let ops: u64 = 8;
    #[cfg(not(miri))]
    let ops: u64 = 64;

    for seed in 0..seeds {
        let mut trace = Vec::with_capacity(ops as usize);
        {
            let mut r = Rig::new();
            for i in 0..ops {
                mot_buoc(&mut r, seed, i, &mut trace);
                // Bất biến sau **mỗi** bước (DoD 4/5).
                r.kiem(&format!("seed {seed} bước {i}"));
            }
        }
        // Cùng seed ⇒ cùng trace.
        let mut trace2 = Vec::with_capacity(ops as usize);
        {
            let mut r2 = Rig::new();
            for i in 0..ops {
                mot_buoc(&mut r2, seed, i, &mut trace2);
            }
        }
        assert_eq!(trace, trace2, "seed {seed}: hai lượt phải cùng trace");
    }
}

// ===========================================================================
// 14. KT-34 (C2) — **property trên MỌI thứ tự** của 3 bước {Wait(t0, expected = 0), Store(1),
//     Wake(1)}: Wake **sau** khi waiter xếp hàng ⇒ waiter **phải** Ready; Wake **trước** ⇒ trả 0
//     và **không** nuốt giá trị (lần Wait sau thấy giá trị mới ⇒ `InvalidArgument`, không chờ
//     trên giá trị cũ). Bổ sung cho ca định hướng `khong_mat_danh_thuc_wake_truoc_va_sau`.
// ===========================================================================

#[test]
fn property_wakeaddr_sau_khi_xep_hang_danh_thuc_moi_thu_tu() {
    // Mọi hoán vị của 3 bước: 0 = Wait t[0] (expected = 0), 1 = Store(1), 2 = Wake(1).
    let hoan_vi: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for thu_tu in hoan_vi {
        let mut r = Rig::new();
        let ep = r.ep;
        // Giá trị khởi đầu 0 = `expected` của Wait ⇒ Wait **hợp lệ** (không tạo lỗi giả).
        r.set(ep, 0, 0);
        let mut dang_cho = false;
        let mut da_store = false;
        for (buoc, &op) in thu_tu.iter().enumerate() {
            match op {
                0 => {
                    if da_store {
                        // Giá trị đã đổi ⇒ **không** xếp hàng (KT-28: so giá trị **trước** khi
                        // chờ), không để lại waiter ma.
                        assert_eq!(
                            r.wait(0, ep, 0, 0),
                            Err(KError::InvalidArgument),
                            "thứ tự {thu_tu:?} bước {buoc}: expected cũ ⇒ InvalidArgument"
                        );
                        assert_eq!(r.waiters(ep, 0).len(), 0, "không được để lại waiter");
                        assert!(!dang_cho);
                    } else {
                        r.wait(0, ep, 0, 0).expect("WaitAddr khớp giá trị hiện tại");
                        dang_cho = true;
                        assert_eq!(
                            r.st(0),
                            TcbState::BlockedOnAddress {
                                frame: ep,
                                offset: 0
                            },
                            "thứ tự {thu_tu:?} bước {buoc}: phải xếp hàng"
                        );
                    }
                }
                1 => {
                    r.set(ep, 0, 1);
                    da_store = true;
                }
                _ => {
                    let k = r.wake(ep, 0, 1).expect("WakeAddr");
                    // **C2 (chiều "sau")**: Wake sau khi waiter xếp hàng ⇒ đúng waiter đó Ready.
                    // **C2 (chiều "trước")**: chưa ai chờ ⇒ 0, không đánh thức nhầm.
                    assert_eq!(
                        k,
                        u32::from(dang_cho),
                        "thứ tự {thu_tu:?} bước {buoc}: số waiter được đánh thức"
                    );
                    if dang_cho {
                        assert_eq!(
                            r.st(0),
                            TcbState::Ready,
                            "thứ tự {thu_tu:?}: Wake sau khi xếp hàng phải đánh thức t[0]"
                        );
                        assert!(r.m.ready_contains(0, r.t[0]), "phải vào hàng Ready");
                        dang_cho = false;
                    }
                    // Không nuốt giá trị (mô hình futex: state là giá trị, không phải "cờ treo").
                    assert_eq!(r.m.addr_word(ep, 0), if da_store { 1 } else { 0 });
                }
            }
            r.kiem(&format!("thứ tự {thu_tu:?} bước {buoc}"));
        }
        if dang_cho {
            assert_eq!(r.waiters(ep, 0), vec![r.t[0]], "waiter còn chờ đúng khoá");
            assert_eq!(
                r.st(0),
                TcbState::BlockedOnAddress {
                    frame: ep,
                    offset: 0
                }
            );
        } else {
            assert_eq!(r.waiters(ep, 0).len(), 0, "không còn waiter ma");
        }
    }
}
