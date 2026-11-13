// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-32** — driver **L1 trộn IPC** cho K1c: đóng nợ **C1-100k** của D2-61 / K1C-DOD §2 + §9.2
//! ("driver `l1_k1c` có 9 op sched/SMP/mail — **không** sinh Send/Recv/Call/Reply/Signal/
//! WaitAddr ⇒ 100k **không** chứng minh bảo toàn message").
//!
//! Giữ **nguyên 9 op** của KT-29 (`prio/affinity/dispatch/block/wake/process_mail/
//! push_migration/ready-wake/yield`) và thêm **10 op** đi qua đường K1c thật:
//!
//! | Nhóm | Op | API (cap **hợp lệ**, object **thật**) |
//! |---|---|---|
//! | Send | 9 | `Machine::send` trên cap **Endpoint** (chặn ⇒ `BlockedOnSend`) |
//! | NBSend | 18 | `Machine::nb_send` trên cap **Endpoint** (không chặn) |
//! | Recv | 10 | `Machine::recv` trên cap **Endpoint**, có/không cap **Reply** |
//! | Call | 11 | `Machine::call` (một MR) trên cap **Endpoint** |
//! | Reply | 12 | `Machine::reply_msg` trên cap **Reply** (chọn Reply đang có `caller` trước) |
//! | Signal | 13 | `Machine::send` trên cap **Notification** = Signal (badge cap, seL4) |
//! | Wait | 14 | `Machine::recv` trên cap **Notification** = Wait |
//! | WaitAddr | 15 | `Machine::wait_addr(tcb, frame, offset, expected=giá_trị_hiện_tại)` |
//! | WakeAddr | 16 | `Machine::wake_addr(frame, offset, n=1..2)` |
//! | Store | 17 | `Machine::set_addr_word` (ghi giá trị "từ" — mô hình bộ nhớ của K2) |
//!
//! Object/cap dựng trong `Rig::new` (test-only): CNode 16 ô, 2 Endpoint, 2 Reply,
//! 1 Notification (+1 cap `mint` badge `0x0010`), 1 Untyped/1 SchedContext/1 SchedControl,
//! 4 TCB đã `set_sched_params` + `set_priority` + `set_affinity` + `Inactive → Ready`.
//!
//! **Hợp lệ theo state, có nhánh fallback**: mọi op IPC/WaitAddr chỉ gọi khi TCB
//! `Ready|Running` (`ensure_started` của mô hình trả `IllegalOperation` cho trạng thái khác);
//! [`Rig::san_sang`] chọn TCB chạy được theo seed, cạn ứng viên thì `wake` **một** TCB rồi thử
//! lại, vẫn cạn thì **ghi nhận** (`- (không TCB chạy được)`) thay vì panic hay gọi op sai state.
//! `WaitAddr` lấy `expected = addr_word(frame, offset)` (đọc **rồi** chờ — đúng đường hợp lệ
//! của KT-28, không tạo lỗi giả); `Store` đổi giá trị để lượt sau có động lực đánh thức.
//!
//! **Bất biến sau MỖI op** (kể cả prologue): `check_sched` + `check_harts` + `check_ipc` +
//! `check_waitaddr` + `check_mailboxes` + `check_caps` — vi phạm ⇒ panic kèm dòng gây lỗi.
//! Cuối mỗi lượt (**KT-33**: siết từ `ipc_sent > 0`): `ipc_sent > 1` và
//! `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped` (C1 của K1C-PLAN §1.1); cuối **mỗi
//! smp**: `ipc_delivered > 0`. Prologue `NBSend` (không chặn ⇒ TCB vẫn `Ready`) chỉ bảo đảm
//! `ipc_sent ≥ 1`; `ipc_sent > 1` buộc **64 op ngẫu nhiên** phải sinh thêm ít nhất một message
//! (không còn tautology nhờ prologue). `ipc_delivered > 0` kiểm ở mức **tổng mỗi smp** vì một
//! seed đơn lẻ có thể không giao được message nào (hàng đầy/caller bị huỷ — huỷ hợp lệ D2-25).
//!
//! `gate --fast` vẫn chạy **1.000 seed / smp 1** (mặc định env, không nới ngưỡng); quét
//! 1/2/4/8 dùng `./v2/bin/v2 nightly --l1 --smp 1,2,4,8 [--seeds N]` (100k seed thuộc cổng
//! cuối phase §7.1). Tên test **không** bắt đầu bằng `l1_` nên L1 cũ của gate không chạy driver
//! này.
//!
//! **Model-only `[TK]`**, không phải spec/ABI; không thread/QEMU; không sửa semantics
//! `kobj/ipc/...` (chỉ driver test).

use vifast_sim::ipc::{Message, MessageInfo};
use vifast_sim::kobj::{Machine, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL};
use vifast_sim::tcb::SleepReason;
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` hợp lệ với `info.length() == words.len()` (`ipc::Message::new` kiểm).
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

fn msg_empty() -> Message {
    msg(0, &[])
}

/// `VIFAST_SMP` (mặc định `1`): danh sách hart, ví dụ `1,2,4,8`. Mỗi phần tử phải trong
/// `1..=MAX_HARTS` (8) — sai ⇒ panic rõ (không im lặng bỏ).
fn smp_tu_env() -> Vec<u16> {
    let raw = std::env::var("VIFAST_SMP").unwrap_or_else(|_| "1".to_string());
    let mut ds = Vec::new();
    for phan in raw.split(',') {
        let n: u16 = phan
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("VIFAST_SMP: '{phan}' không phải số hart"));
        assert!(
            (1..=vifast_sim::kobj::MAX_HARTS).contains(&n),
            "VIFAST_SMP: hart {n} ngoài 1..={}",
            vifast_sim::kobj::MAX_HARTS
        );
        ds.push(n);
    }
    ds
}

/// `VIFAST_SEEDS` (mặc định `1000`); `0` ⇒ panic (không chạy rỗng để "xanh").
fn seeds_tu_env() -> u64 {
    let raw = std::env::var("VIFAST_SEEDS").unwrap_or_else(|_| "1000".to_string());
    let n: u64 = raw
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("VIFAST_SEEDS: '{raw}' không phải số"));
    assert!(n > 0, "VIFAST_SEEDS phải dương");
    n
}

// ---------------------------------------------------------------------------
// Rig: bố cục slot **giữ nguyên 9 slot của KT-29** (0 Untyped, 4 SC, 5 SchedControl,
// 8..11 TCB) và thêm 1..2 Endpoint, 6..7 Reply, 12 Notification (13 = cap mint badge).
// ---------------------------------------------------------------------------

const S_U: usize = 0;
const S_EP: [usize; 2] = [1, 2];
const S_SC: usize = 4;
const S_CTL: usize = 5;
const S_REP: [usize; 2] = [6, 7];
const S_TCB: usize = 8;
const S_NTFN: usize = 12;
const S_NTFN_BADGE: usize = 13;
/// Số TCB của rig (như KT-29: 4 TCB ⇒ chạy được trên mọi smp 1/2/4/8).
const N_TCB: usize = 4;
/// Số op/lượt — giữ **64** như KT-29/KT-31 (không đổi ngân sách; thêm op chỉ đổi **thành phần**).
const SO_OP: usize = 64;
/// Số nhánh op: 0..=8 của KT-29 + 9..=18 của KT-32.
const N_OP: u64 = 19;
/// 4 offset "từ" của WaitAddr (bội 8; 0/64 cùng bucket — KT-28).
const OFFSETS: [u32; 4] = [0, 8, 16, 64];

struct Rig {
    m: Machine,
    ep_s: [SlotRef; 2],
    rep: [ObjId; 2],
    rep_s: [SlotRef; 2],
    ntfn_s: SlotRef,
    ntfn_badge_s: SlotRef,
    /// "Frame" của WaitAddr: object `Frame` là **reserved** tới K2 nên khoá dùng **ObjId thô**
    /// (đúng như KT-28); lấy Untyped gốc để khoá ổn định suốt lượt.
    frame: ObjId,
    t: [ObjId; N_TCB],
    t_s: [SlotRef; N_TCB],
    harts: u16,
}

impl Rig {
    fn new(harts: u16) -> Self {
        let mut m = Machine::with_harts(harts);
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 1 << 16);
        let sc = m.add_kernel_object(ObjType::SchedContext);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        for (i, obj) in [(S_U, u0), (S_SC, sc), (S_CTL, ctl)] {
            let cap = m.root_cap(obj);
            m.put_cap(c0, i, cap).expect("cap");
        }
        m.sched_control_configure(slot(c0, S_CTL), slot(c0, S_SC), 64, 64)
            .expect("SC mặc định (test-only)");

        // 2 Endpoint + 2 Reply: cap gốc đặt trong CNode ⇒ **cap hợp lệ** cho Send/Recv/Call/Reply.
        let mut ep = [0u32; 2];
        let mut ep_s = [slot(c0, 0); 2];
        for (j, s) in S_EP.iter().enumerate() {
            ep[j] = m.add_kernel_object(ObjType::Endpoint);
            let cap = m.root_cap(ep[j]);
            m.put_cap(c0, *s, cap).expect("cap Endpoint");
            ep_s[j] = slot(c0, *s);
        }
        let mut rep = [0u32; 2];
        let mut rep_s = [slot(c0, 0); 2];
        for (j, s) in S_REP.iter().enumerate() {
            rep[j] = m.add_kernel_object(ObjType::Reply);
            let cap = m.root_cap(rep[j]);
            m.put_cap(c0, *s, cap).expect("cap Reply");
            rep_s[j] = slot(c0, *s);
        }
        // Notification: cap thô (badge `None` = 0 — **hợp lệ**, KT-27) + cap `mint` badge
        // `0x0010` để Signal có badge ≠ 0 ⇒ word ≠ 0 ⇒ Wait sau đó giao ngay (có động lực).
        let ntfn = m.add_kernel_object(ObjType::Notification);
        let ntfn_cap = m.root_cap(ntfn);
        m.put_cap(c0, S_NTFN, ntfn_cap).expect("cap Notification");
        let ntfn_s = slot(c0, S_NTFN);
        m.mint(ntfn_s, slot(c0, S_NTFN_BADGE), RIGHTS_ALL, Some(0x0010))
            .expect("mint badge trên Notification");
        let ntfn_badge_s = slot(c0, S_NTFN_BADGE);

        let mut t = [0u32; N_TCB];
        let mut t_s = [slot(c0, S_TCB); N_TCB];
        for (i, (obj, s)) in t.iter_mut().zip(t_s.iter_mut()).enumerate() {
            *obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(*obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            m.set_sched_params(slot(c0, S_TCB + i), Some(slot(c0, S_SC)))
                .expect("SC cho TCB");
            m.tcb_set_priority(slot(c0, S_TCB + i), 3 + i as u8, 255)
                .expect("prio");
            m.tcb_set_affinity(slot(c0, S_TCB + i), (1u64 << harts) - 1)
                .expect("affinity");
            m.transition_tcb(*obj, TcbState::Ready).expect("Ready");
            *s = slot(c0, S_TCB + i);
        }
        Self {
            m,
            ep_s,
            rep,
            rep_s,
            ntfn_s,
            ntfn_badge_s,
            frame: u0,
            t,
            t_s,
            harts,
        }
    }

    /// TCB `Ready|Running` — điều kiện của **mọi** op IPC/Wait/WakeAddr trong mô hình
    /// (`ensure_started` trả `IllegalOperation` cho trạng thái khác).
    fn chay_duoc(&self, i: usize) -> bool {
        matches!(
            self.m.tcb_state(self.t[i]),
            Ok(TcbState::Ready) | Ok(TcbState::Running { .. })
        )
    }

    /// Chọn TCB chạy được theo `chon` (thead định): duyệt `0..N_TCB` tăng dần ⇒ tất định.
    fn chon_tcb(&self, chon: u64) -> Option<usize> {
        let ung: Vec<usize> = (0..N_TCB).filter(|i| self.chay_duoc(*i)).collect();
        if ung.is_empty() {
            return None;
        }
        Some(ung[(chon % ung.len() as u64) as usize])
    }

    /// Nhánh **fallback**: không TCB nào chạy được ⇒ `wake` **một** TCB theo seed rồi thử lại.
    /// `wake` trên SMP có thể chỉ **gửi mail** cho hart home (KT-22) nên vẫn có thể `None`; khi
    /// đó op ghi nhận là bỏ qua — **không** gọi op sai state, **không** panic.
    fn san_sang(&mut self, chon: u64) -> Option<usize> {
        if let Some(i) = self.chon_tcb(chon) {
            return Some(i);
        }
        let i = (chon % N_TCB as u64) as usize;
        let _ = self.m.wake(self.t[i]);
        self.chon_tcb(chon)
    }

    /// Reply đang có `caller` (Call chờ trả lời) được ưu tiên — Reply "rỗng" vẫn **hợp lệ**
    /// (thiết kế §4.4 `:281`: Reply không có caller ⇒ bỏ reply, không đánh thức ai).
    fn chon_reply(&self, chon: u64) -> usize {
        let uu_tien = (chon % 2) as usize;
        let khac = 1 - uu_tien;
        let co_caller = |j: usize| self.m.reply_caller(self.rep[j]).ok().flatten().is_some();
        if co_caller(uu_tien) || !co_caller(khac) {
            uu_tien
        } else {
            khac
        }
    }

    /// Một op: 9 nhánh KT-29 giữ **nguyên văn**, 10 nhánh K1c thêm (xem bảng ở đầu tệp).
    fn mot_op(&mut self, op: u64, rng: &mut Lcg, dong: &mut String) {
        match op {
            0 => {
                let i = rng.below(N_TCB as u64) as usize;
                let p = rng.below(16) as u8;
                if self.m.tcb_set_priority(self.t_s[i], p, 255).is_ok() {
                    dong.push_str(&format!(" prio{i}={p}"));
                }
            }
            1 => {
                let i = rng.below(N_TCB as u64) as usize;
                let home = self.m.tcb_info(self.t_s[i]).expect("info").home;
                let mask = (1u64 << home) | (rng.below(1u64 << self.harts) & !(1u64 << home));
                if self.m.tcb_set_affinity(self.t_s[i], mask).is_ok() {
                    dong.push_str(&format!(" aff{i}={mask:#x}"));
                }
            }
            2 => {
                let h = rng.below(self.harts as u64) as u16;
                let got = self.m.dispatch(h).expect("dispatch");
                dong.push_str(&format!(" disp{h}={got:?}"));
            }
            3 => {
                let h = rng.below(self.harts as u64) as u16;
                if self.m.current(h).is_some() {
                    let _ = self.m.block_current(
                        h,
                        TcbState::Sleeping {
                            reason: SleepReason::Timeout,
                        },
                    );
                    dong.push_str(&format!(" block{h}"));
                }
            }
            4 => {
                let i = rng.below(N_TCB as u64) as usize;
                if self.m.wake(self.t[i]).is_ok() {
                    dong.push_str(&format!(" wake{i}"));
                }
            }
            5 => {
                let h = rng.below(self.harts as u64) as u16;
                let n = self.m.process_mail(h).expect("process_mail");
                dong.push_str(&format!(" mail{h}={n}"));
            }
            6 => {
                let n = self.m.push_migration().expect("push_migration");
                dong.push_str(&format!(" push={n}"));
            }
            7 => {
                let i = rng.below(N_TCB as u64) as usize;
                let st = self.m.tcb_state(self.t[i]).expect("state");
                if matches!(st, TcbState::Inactive) {
                    self.m
                        .transition_tcb(self.t[i], TcbState::Ready)
                        .expect("Ready");
                    dong.push_str(&format!(" ready{i}"));
                } else if !matches!(st, TcbState::Ready | TcbState::Running { .. }) {
                    let _ = self.m.wake(self.t[i]);
                    dong.push_str(&format!(" wake{i}"));
                }
            }
            8 => {
                let h = rng.below(self.harts as u64) as u16;
                if self.m.current(h).is_some() {
                    let _ = self.m.yield_current(h).expect("yield");
                    dong.push_str(&format!(" yield{h}"));
                }
            }
            // ---- KT-32: nhóm Send/Recv/Call/Reply trên Endpoint/Reply thật ----
            9 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" send - (không TCB chạy được)");
                    return;
                };
                let j = rng.below(2) as usize;
                let w = [rng.next_u64(), rng.next_u64()];
                match self
                    .m
                    .send(self.t[i], self.ep_s[j], msg(0x100 + j as u32, &w))
                {
                    Ok(()) => dong.push_str(&format!(" send{i}->ep{j}")),
                    Err(e) => dong.push_str(&format!(" send{i}->ep{j} {e:?}")),
                }
            }
            10 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" recv - (không TCB chạy được)");
                    return;
                };
                let j = rng.below(2) as usize;
                let co_reply = rng.below(2) == 0;
                let rep = if co_reply { Some(self.rep_s[j]) } else { None };
                match self.m.recv(self.t[i], self.ep_s[j], rep) {
                    Ok(()) => dong.push_str(&format!(" recv{i}<-ep{j} rep={co_reply}")),
                    Err(e) => dong.push_str(&format!(" recv{i}<-ep{j} {e:?}")),
                }
            }
            11 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" call - (không TCB chạy được)");
                    return;
                };
                let j = rng.below(2) as usize;
                let mr = rng.next_u64();
                match self.m.call(self.t[i], self.ep_s[j], mr) {
                    Ok(()) => dong.push_str(&format!(" call{i}->ep{j}")),
                    Err(e) => dong.push_str(&format!(" call{i}->ep{j} {e:?}")),
                }
            }
            12 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" reply - (không TCB chạy được)");
                    return;
                };
                let k = self.chon_reply(rng.below(2));
                let w = [rng.next_u64()];
                match self.m.reply_msg(self.t[i], self.rep_s[k], msg(0x200, &w)) {
                    Ok(()) => dong.push_str(&format!(" reply{i} rep{k}")),
                    Err(e) => dong.push_str(&format!(" reply{i} rep{k} {e:?}")),
                }
            }
            // ---- KT-32: nhóm Signal/Wait trên Notification ----
            13 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" signal - (không TCB chạy được)");
                    return;
                };
                // 3/4 dùng cap **mint badge** (word ≠ 0 ⇒ Wait sau giao ngay), 1/4 cap thô
                // (badge 0 — **hợp lệ** theo KT-27, không dùng 0 làm "chưa có tín hiệu").
                let badge = rng.below(4) != 0;
                let s = if badge {
                    self.ntfn_badge_s
                } else {
                    self.ntfn_s
                };
                match self.m.send(self.t[i], s, msg_empty()) {
                    Ok(()) => dong.push_str(&format!(" signal{i} badge={badge}")),
                    Err(e) => dong.push_str(&format!(" signal{i} {e:?}")),
                }
            }
            14 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" wait - (không TCB chạy được)");
                    return;
                };
                match self.m.recv(self.t[i], self.ntfn_s, None) {
                    Ok(()) => dong.push_str(&format!(" wait{i}")),
                    Err(e) => dong.push_str(&format!(" wait{i} {e:?}")),
                }
            }
            // ---- KT-32: nhóm WaitAddr/WakeAddr trên khoá (frame, offset) ----
            15 => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" waitaddr - (không TCB chạy được)");
                    return;
                };
                let off = OFFSETS[rng.below(OFFSETS.len() as u64) as usize];
                // `expected = giá trị hiện tại` ⇒ op **hợp lệ** (KT-28: đọc rồi xếp hàng cùng
                // một lần giữ khoá); không tạo lỗi giả `InvalidArgument`.
                let cur = self.m.addr_word(self.frame, off);
                match self.m.wait_addr(self.t_s[i], self.frame, off, cur) {
                    Ok(()) => dong.push_str(&format!(" waitaddr{i}@{off}={cur:#x}")),
                    Err(e) => dong.push_str(&format!(" waitaddr{i}@{off} {e:?}")),
                }
            }
            16 => {
                let off = OFFSETS[rng.below(OFFSETS.len() as u64) as usize];
                let n = 1 + rng.below(2) as u32;
                match self.m.wake_addr(self.frame, off, n) {
                    Ok(k) => dong.push_str(&format!(" wakeaddr@{off} n={n} ->{k}")),
                    Err(e) => dong.push_str(&format!(" wakeaddr@{off} {e:?}")),
                }
            }
            17 => {
                // Ghi giá trị "từ" (mô hình bộ nhớ của K2 — `addr_words`): tạo động lực thật cho
                // WaitAddr/WakeAddr ở các bước sau (không phải syscall).
                let off = OFFSETS[rng.below(OFFSETS.len() as u64) as usize];
                let v = rng.next_u64();
                self.m.set_addr_word(self.frame, off, v);
                dong.push_str(&format!(" store@{off}={v:#x}"));
            }
            _ => {
                let Some(i) = self.san_sang(rng.below(N_TCB as u64)) else {
                    dong.push_str(" nbsend - (không TCB chạy được)");
                    return;
                };
                let j = rng.below(2) as usize;
                let w = [rng.next_u64()];
                match self
                    .m
                    .nb_send(self.t[i], self.ep_s[j], msg(0x300 + j as u32, &w))
                {
                    Ok(()) => dong.push_str(&format!(" nbsend{i}->ep{j}")),
                    Err(e) => dong.push_str(&format!(" nbsend{i}->ep{j} {e:?}")),
                }
            }
        }
    }

    /// Ảnh chụp scheduler (như KT-29) — phần cuối mỗi dòng trace.
    fn anh_chup(&self) -> String {
        let mut anh = String::new();
        for h in 0..self.harts {
            let p = self.m.published(h).expect("published");
            anh.push_str(&format!(
                "h{h}:{}/{}:{};",
                self.m.current(h).is_some(),
                p.cur_prio,
                self.m.ready_len(h)
            ));
        }
        anh
    }
}

/// Bất biến K1c sau **mỗi** op (KT-32): scheduler, hart, bảo toàn message, WaitAddr, hộp thư,
/// cap. Vi phạm ⇒ panic kèm dòng gây lỗi **và** `seed`/`smp` (KT-33 — log đỏ tự đủ ngữ cảnh,
/// không phải truy ngược dòng `# L1 OK`): không nuốt lỗi, không "chạy tiếp để xanh".
fn kiem(m: &Machine, dong: &str, seed: u64, harts: u16) {
    if let Err(e) = m.check_sched() {
        panic!("check_sched sau {dong} (seed {seed}, smp {harts}): {e}");
    }
    if let Err(e) = m.check_harts() {
        panic!("check_harts sau {dong} (seed {seed}, smp {harts}): {e}");
    }
    if let Err(e) = m.check_ipc() {
        panic!("check_ipc sau {dong} (seed {seed}, smp {harts}): {e}");
    }
    if let Err(e) = m.check_waitaddr() {
        panic!("check_waitaddr sau {dong} (seed {seed}, smp {harts}): {e}");
    }
    if let Err(e) = m.check_mailboxes() {
        panic!("check_mailboxes sau {dong} (seed {seed}, smp {harts}): {e}");
    }
    if let Err(e) = m.check_caps() {
        panic!("check_caps sau {dong} (seed {seed}, smp {harts}): {e}");
    }
}

/// Một lượt L1 tất định trên `harts` hart: trace là chuỗi sự kiện + ảnh chụp scheduler, kèm
/// `(ipc_sent, ipc_delivered)` cuối lượt để so hai lần chạy cùng seed.
fn mot_luot(seed: u64, harts: u16, so_op: usize) -> (Vec<String>, u64, u64, u64) {
    let mut r = Rig::new(harts);
    let mut rng = Lcg::new(seed ^ ((harts as u64) << 32));
    let mut trace = Vec::with_capacity(so_op);
    // Prologue **NBSend** (không chặn ⇒ TCB vẫn `Ready`, message nằm hợp lệ trong hàng gửi
    // `ep0`): bảo đảm `ipc_sent ≥ 1` từ seed đầu ⇒ C1 "ipc_sent > 0" là **bất biến của driver**,
    // không phụ thuộc 64 op ngẫu nhiên có gặp Send/Call hay không.
    r.m.nb_send(r.t[0], r.ep_s[0], msg(0xFFFF, &[0xA5]))
        .expect("prologue NBSend");
    kiem(&r.m, "prologue NBSend", seed, harts);
    for step in 0..so_op {
        let op = rng.below(N_OP);
        let mut dong = format!("s{step} op{op}");
        r.mot_op(op, &mut rng, &mut dong);
        kiem(&r.m, &dong, seed, harts);
        dong.push_str(&format!(" | {} mig={}", r.anh_chup(), r.m.migrate_count()));
        trace.push(dong);
    }
    let sent = r.m.ipc_sent();
    let delivered = r.m.ipc_delivered();
    let dropped = r.m.ipc_dropped();
    // C1 (K1C-PLAN §1.1): bảo toàn message — không mất, không thêm. **KT-33** siết `> 0`
    // thành `> 1`: prologue chỉ lo 1 message, 64 op còn lại phải sinh thêm ⇒ không tautology.
    assert!(
        sent > 1,
        "seed {seed} smp {harts}: ipc_sent = {sent} ≤ 1 — 64 op không sinh thêm message (C1)"
    );
    assert_eq!(
        sent,
        delivered + r.m.ipc_pending() + r.m.ipc_dropped(),
        "seed {seed} smp {harts}: message mất/thêm (sent={sent}, delivered={delivered})"
    );
    (trace, sent, delivered, dropped)
}

/// **L1 theo smp**: cùng seed ⇒ **cùng trace** byte-for-byte, 0 vi phạm bất biến sau mỗi op,
/// `ipc_sent > 1` mỗi lượt, `ipc_delivered > 0` tổng mỗi smp, và bảo toàn message cuối lượt.
///
/// Tên **không** bắt đầu bằng `l1_` để `gate --fast` (bộ lọc `l1_` của L1 cũ) **không** chạy
/// driver này: gate giữ L1 1.000 seed cũ ≤ 60 s (K1C-PLAN §5 KT-29 DoD 8); `nightly --l1` đặt
/// env `VIFAST_SMP`/`VIFAST_SEEDS` và chạy theo bộ lọc `driver_smp`.
#[test]
fn driver_smp_cung_seed_cung_trace() {
    let smps = smp_tu_env();
    let seeds = seeds_tu_env();
    let mut tong = 0u64;
    for &harts in &smps {
        let mut ipc_sent = 0u64;
        let mut ipc_delivered = 0u64;
        let mut ipc_dropped = 0u64;
        for seed in 0..seeds {
            let (a, sent_a, del_a, dr_a) = mot_luot(seed, harts, SO_OP);
            let (b, sent_b, del_b, dr_b) = mot_luot(seed, harts, SO_OP);
            assert_eq!(
                a, b,
                "smp {harts} seed {seed}: cùng seed phải cùng trace (tất định)"
            );
            assert_eq!(
                (sent_a, del_a, dr_a),
                (sent_b, del_b, dr_b),
                "smp {harts} seed {seed}: cùng seed phải cùng bộ đếm IPC"
            );
            assert_eq!(a.len(), SO_OP, "smp {harts} seed {seed}: thiếu op");
            assert!(
                sent_a > 1,
                "smp {harts} seed {seed}: ipc_sent = {sent_a} ≤ 1 (KT-33)"
            );
            ipc_sent += sent_a;
            ipc_delivered += del_a;
            ipc_dropped += dr_a;
            tong += 1;
        }
        // **KT-33**: 64 op × `seeds` phải **thật sự giao** được message (không chỉ xếp hàng
        // hoặc rơi hợp lệ) — kiểm ở mức tổng mỗi smp, không bắt một seed đơn lẻ.
        assert!(
            ipc_delivered > 0,
            "smp {harts}: tổng ipc_delivered = 0 trên {seeds} seed — driver không giao message nào"
        );
        println!(
            "# L1 OK: smp {harts}, {seeds} seed × {SO_OP} op (ipc_sent {ipc_sent}, ipc_delivered {ipc_delivered}, ipc_dropped {ipc_dropped})"
        );
    }
    println!("# L1 OK: {} lượt (smp {smps:?}, seeds {seeds})", tong);
}
