// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Machine` — model core K3b: Untyped + CNode + MDB + capability (KT-58) **và** TCB +
//! transition + scheduler + hộp thư (KT-59).
//!
//! Port **nguyên ngữ nghĩa** từ `v2/sim/vifast-sim/src/kobj.rs` (oracle): cùng id tuần tự
//! không tái dùng, cùng thứ tự cấp phát, cùng mã lỗi, cùng bất biến `check_caps`/`check_bytes`/
//! `check_sched`/`check_mailboxes`. Khác biệt **duy nhất** so với sim (ghi trong
//! `docs/k3b-core.md`):
//!
//! 1. Bảng tĩnh `.bss` thay `Vec` (không `alloc`); đầy bảng ⇒ `NotEnoughMemory` (harness
//!    `add_*` panic như sim khi arena kernel đầy).
//! 2. Chưa có body Endpoint/Notification/Reply/IPC (KT-60): object vẫn có `ObjType`, `alive`,
//!    `untyped/offset/len`, cap/MDB đầy đủ; TCB/SchedContext **đã có body** (KT-59).
//! 3. Không `Retype` `Frame`/`PageTable`/`VSpace` (K2 object layer `vifast-plat-common::obj`,
//!    KT-52) — trả `NotSupported`; sim (K2) đã mở `Frame`/`PageTable`, xem `docs/k3b-core.md`.
//! 4. **Donation KT-21** là phần của `Call`/`Reply` (KT-60): KT-59 giữ trường
//!    `donated_sc`/`donated_to`/`donor` + công thức `donated_priority`, nhưng chưa có đường tạo
//!    donation — parity KT-59 phủ trạng thái **không donation** (ghi trong docs §7).
//! 5. **Reply/Notification/IPC** chưa có body ⇒ `unlink_tcb_queues` chỉ là mốc (không có hàng
//!    chờ nào của KT-24..28 để gỡ); `TlbFlush` trong hộp thư là no-op (core chưa có TLB).

use crate::arena::Region;
use crate::captable::{CapChildren, CapTable};
use crate::error::{CapId, CapInfo, KError, ObjId, ObjType, SlotRef};
use crate::mailbox::HartMailbox;
use crate::mdb::{push_id, sort_dedup};
use crate::objstate::{
    AddrWord, EndpointData, NotificationData, QueuedMsg, ReplyData, WaitNode, MAX_ADDR_WORDS,
    MAX_WAIT_NODES, NODE_NONE, WAITADDR_BUCKETS,
};
use crate::sched::{HartSched, Published};
use crate::slotmem::{arena_slot_base, harness_slot_base, SlotMem, HARNESS_SLOT_WORDS};
use crate::spec::{
    ARENA_ALIGN, CNODE_BASE_BYTES, DEFAULT_ALIGN_BYTES, FLAG_GUEST, RIGHTS_ALL, RIGHT_WRITE,
    SLOT_BYTES,
};
use crate::tcb::{TcbState, TCB_STATE_COUNT};
use crate::tcbdata::{ScData, TcbData};
use crate::tlb::TlbPending;

/// Số object tối đa trong bảng tĩnh (id **không tái dùng**, kể cả object đã chết).
pub const MAX_OBJECTS: usize = 256;
/// Số cap tối đa trong bảng tĩnh (id **không tái dùng**, kể cả cap đã chết).
pub const MAX_CAPS: usize = 512;
/// Số Untyped tối đa (mỗi Untyped một arena).
pub const MAX_UNTYPEDS: usize = 16;
/// Số word slot CNode tối đa (1 word = 1 cap id; 48 B slot của spec vẫn được kế toán byte).
pub const MAX_SLOT_WORDS: usize = 4096;
/// Số **body TCB** tối đa (biên tĩnh `[TK]`; sim không biên vì `Box`).
pub const MAX_TCBS: usize = 64;
/// Số **body SchedContext** tối đa (biên tĩnh `[TK]`).
pub const MAX_SCHED_CONTEXTS: usize = 32;
/// Số **body Endpoint** tối đa (biên tĩnh `[TK]`; sim không biên vì `Box`).
pub const MAX_ENDPOINTS: usize = 16;
/// Số **body Reply** tối đa (biên tĩnh `[TK]`).
pub const MAX_REPLIES: usize = 32;
/// Số **body Notification** tối đa (biên tĩnh `[TK]`).
pub const MAX_NOTIFICATIONS: usize = 16;
/// Số message **đang chờ** tối đa trong mọi hàng gửi Endpoint (pool chung, biên tĩnh `[TK]`).
/// Mỗi Endpoint có sức chứa riêng [`crate::objstate::ENDPOINT_QUEUE_CAP`] = 8; pool chung nhỏ hơn
/// tổng 8 × `MAX_ENDPOINTS` là biên của mô hình (như `MAX_TCBS`).
pub const MAX_QUEUED_MESSAGES: usize = 24;
/// Số hart tối đa của mô hình (`MAX_HARTS = 8`, KT-49/D2-72 Q4).
pub const MAX_HARTS: usize = 8;
/// Số dòng log finalise TCB tối đa — **bằng `MAX_OBJECTS`** (không phải `MAX_TCBS`).
/// `finalise_tcb` ghi **một** dòng cho mỗi TCB **đang sống** khi finalise; body tái dùng nên một
/// `Machine` có thể finalise nhiều lượt TCB (tối đa bằng số object id từng cấp — `MAX_OBJECTS`).
/// Đầy ⇒ [`Machine::finalise_tcb`] trả `NotEnoughMemory` (không cắt im lặng, không panic).
pub const MAX_FINALISE: usize = MAX_OBJECTS;

// **KT-312** — `BodyRef`/`Meta`/`Cap`/`FinaliseEntry` + mã body chuyển sang
// [`crate::objmeta`] (giữ **R1** ≤ 1.500 dòng/tệp); tái xuất **đúng tên cũ** nên mọi
// `use crate::machine::{…}` giữ nguyên.
pub use crate::objmeta::FinaliseEntry;
pub(crate) use crate::objmeta::{
    BodyRef, Cap, Meta, BODY_ENDPOINT, BODY_NOTIFICATION, BODY_REPLY, BODY_SC, BODY_TCB,
};

/// Core model K3b — **không** `unsafe`, **không** cấp phát động.
#[derive(Debug, Clone, Copy)]
pub struct Machine {
    pub(crate) regions: [Region; MAX_UNTYPEDS],
    pub(crate) n_regions: usize,
    pub(crate) objects: [Meta; MAX_OBJECTS],
    pub(crate) n_objects: usize,
    pub(crate) caps: [Cap; MAX_CAPS],
    pub(crate) n_caps: usize,
    /// Cửa sổ backing của **byte slot trong arena** (KT-64) — không còn pool riêng.
    pub(crate) slots: SlotMem,
    /// Arena logic cho CNode **harness** (kernel cấp, không có byte Untyped) — như body heap
    /// của `add_cnode` trong sim; byte slot của nó dùng vùng B của [`SlotMem`].
    pub(crate) harness: Region,
    /// Body TCB (KT-59) + free-list chỉ số.
    pub(crate) tcbs: [TcbData; MAX_TCBS],
    pub(crate) n_tcbs: usize,
    pub(crate) tcb_free: [u16; MAX_TCBS],
    pub(crate) n_tcb_free: usize,
    /// Body SchedContext (KT-59) + free-list chỉ số.
    pub(crate) scs: [ScData; MAX_SCHED_CONTEXTS],
    pub(crate) n_scs: usize,
    pub(crate) sc_free: [u16; MAX_SCHED_CONTEXTS],
    pub(crate) n_sc_free: usize,
    /// Body Endpoint/Reply/Notification (KT-60) + free-list chỉ số.
    pub(crate) endpoints: [EndpointData; MAX_ENDPOINTS],
    pub(crate) n_endpoints: usize,
    pub(crate) endpoint_free: [u16; MAX_ENDPOINTS],
    pub(crate) n_endpoint_free: usize,
    pub(crate) replies: [ReplyData; MAX_REPLIES],
    pub(crate) n_replies: usize,
    pub(crate) reply_free: [u16; MAX_REPLIES],
    pub(crate) n_reply_free: usize,
    pub(crate) notifications: [NotificationData; MAX_NOTIFICATIONS],
    pub(crate) n_notifications: usize,
    pub(crate) notification_free: [u16; MAX_NOTIFICATIONS],
    pub(crate) n_notification_free: usize,
    /// Pool message đang chờ trong hàng gửi Endpoint (KT-60) + free-list chỉ số.
    pub(crate) ipc_msgs: [QueuedMsg; MAX_QUEUED_MESSAGES],
    pub(crate) n_ipc_msgs: usize,
    pub(crate) ipc_msg_free: [u16; MAX_QUEUED_MESSAGES],
    pub(crate) n_ipc_msg_free: usize,
    /// Bảng WaitOnAddress: `(frame, offset) -> u64` + bucket waiter (KT-60).
    pub(crate) addr_words: [AddrWord; MAX_ADDR_WORDS],
    pub(crate) n_addr_words: usize,
    pub(crate) wait_nodes: [WaitNode; MAX_WAIT_NODES],
    pub(crate) n_wait_nodes: usize,
    pub(crate) wait_free: [u16; MAX_WAIT_NODES],
    pub(crate) n_wait_free: usize,
    pub(crate) wait_buckets: [u16; WAITADDR_BUCKETS],
    pub(crate) wait_buckets_tail: [u16; WAITADDR_BUCKETS],
    /// Bộ đếm IPC (KT-24/25/26/27 — cùng tên/nghĩa sim).
    pub(crate) ipc_sent: u64,
    pub(crate) ipc_delivered: u64,
    pub(crate) ipc_dropped: u64,
    pub(crate) transfer_calls: u64,
    /// **KT-63** — bộ đếm chuyển cap: cài được, unwrap, thất bại mềm (cùng tên/nghĩa sim).
    pub(crate) cap_transfer_installed: u64,
    pub(crate) cap_unwrapped: u64,
    pub(crate) cap_transfer_failed: u64,
    /// **KT-309** — extra cap: (khai qua IPC buffer, cài được vào CSpace receiver).
    pub(crate) extra_caps_sent: u64,
    pub(crate) extra_caps_installed: u64,
    pub(crate) fault_events: u64,
    /// Bộ đếm cặp chuyển trạng thái 11×11 (KT-18 DoD 4).
    pub(crate) pair_hits: [u32; TCB_STATE_COUNT * TCB_STATE_COUNT],
    /// Bộ đếm cặp **cấm** bị thử (KT-23 DoD 3).
    pub(crate) forbidden_hits: [u32; TCB_STATE_COUNT * TCB_STATE_COUNT],
    /// Số lần `Running → Sleeping{Budget}` (KT-20/KT-23).
    pub(crate) budget_sleeps: u64,
    /// Số lần một `SchedContext` được nạp lại (KT-20/KT-23).
    pub(crate) budget_refills: u64,
    /// Log finalise TCB (KT-18 DoD 9), có biên `MAX_FINALISE`.
    pub(crate) finalise_log: [FinaliseEntry; MAX_FINALISE],
    pub(crate) n_finalise: usize,
    /// Số lần `finalise_tcb` bị từ chối vì log đầy (chẩn đoán; không phải ABI).
    pub(crate) finalise_overflow: u32,
    /// Scheduler theo hart (KT-19/KT-29).
    pub(crate) sched: [HartSched; MAX_HARTS],
    /// Số hart đang dùng (`1..=MAX_HARTS`).
    pub(crate) n_harts: usize,
    /// Hộp thư theo hart (KT-22).
    pub(crate) mail: [HartMailbox; MAX_HARTS],
    /// Trường công bố mỗi hart (KT-29).
    pub(crate) published: [Published; MAX_HARTS],
    /// Hart "đang chạy" của mô hình tại thời điểm gọi (KT-29).
    pub(crate) current_hart: u16,
    /// Số TCB đã di trú thành công (KT-29).
    pub(crate) migrate_count: u64,
    /// Preempt ép tại mọi public op (KT-19, mặc định tắt).
    pub(crate) preempt_every_step: bool,
    /// Thời gian ảo (tick) — **không** đọc wall clock (D2-49(h)).
    pub(crate) now: u64,
    /// **KT-64** — shootdown TLB đang chờ của mỗi hart (`TlbFlush` thật, chống trang cũ).
    pub(crate) tlb_pending: [TlbPending; MAX_HARTS],
    /// Số hiệu shootdown toàn cục (1-based, tăng đơn điệu).
    pub(crate) tlb_seq: u64,
    /// Số lần flush **cục bộ** đã thực thi qua hook (mọi hart).
    pub(crate) tlb_local_flushes: u64,
    /// Số `TlbFlush` đã post remote.
    pub(crate) tlb_remote_posts: u64,
    /// Số lần hart đích đã phục vụ + ack.
    pub(crate) tlb_serviced: u64,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    /// Model rỗng **1 hart** (Untyped/CNode do harness thêm).
    pub const fn new() -> Self {
        Self::with_harts(1)
    }

    /// Model rỗng `harts` hart (`1..=MAX_HARTS`) — như sim `Machine::with_harts`.
    ///
    /// # Panics
    /// `harts == 0` hoặc `harts > MAX_HARTS` (lỗi init của harness, như sim).
    pub const fn with_harts(harts: u16) -> Self {
        assert!(
            harts > 0 && harts as usize <= MAX_HARTS,
            "vifast-core: số hart phải trong 1..=MAX_HARTS"
        );
        Self {
            regions: [Region::EMPTY; MAX_UNTYPEDS],
            n_regions: 0,
            objects: [Meta::EMPTY; MAX_OBJECTS],
            n_objects: 0,
            caps: [Cap::EMPTY; MAX_CAPS],
            n_caps: 0,
            slots: SlotMem::EMPTY,
            harness: Region::new(0, HARNESS_SLOT_WORDS * SLOT_BYTES),
            tcbs: [TcbData::EMPTY; MAX_TCBS],
            n_tcbs: 0,
            tcb_free: [0; MAX_TCBS],
            n_tcb_free: 0,
            scs: [ScData::EMPTY; MAX_SCHED_CONTEXTS],
            n_scs: 0,
            sc_free: [0; MAX_SCHED_CONTEXTS],
            n_sc_free: 0,
            endpoints: [EndpointData::EMPTY; MAX_ENDPOINTS],
            n_endpoints: 0,
            endpoint_free: [0; MAX_ENDPOINTS],
            n_endpoint_free: 0,
            replies: [ReplyData::EMPTY; MAX_REPLIES],
            n_replies: 0,
            reply_free: [0; MAX_REPLIES],
            n_reply_free: 0,
            notifications: [NotificationData {
                word: 0,
                badge: 0,
                waiter: None,
                bound_sc: None,
            }; MAX_NOTIFICATIONS],
            n_notifications: 0,
            notification_free: [0; MAX_NOTIFICATIONS],
            n_notification_free: 0,
            ipc_msgs: [QueuedMsg::EMPTY; MAX_QUEUED_MESSAGES],
            n_ipc_msgs: 0,
            ipc_msg_free: [0; MAX_QUEUED_MESSAGES],
            n_ipc_msg_free: 0,
            addr_words: [AddrWord::EMPTY; MAX_ADDR_WORDS],
            n_addr_words: 0,
            wait_nodes: [WaitNode::EMPTY; MAX_WAIT_NODES],
            n_wait_nodes: 0,
            wait_free: [0; MAX_WAIT_NODES],
            n_wait_free: 0,
            wait_buckets: [NODE_NONE; WAITADDR_BUCKETS],
            wait_buckets_tail: [NODE_NONE; WAITADDR_BUCKETS],
            ipc_sent: 0,
            ipc_delivered: 0,
            ipc_dropped: 0,
            transfer_calls: 0,
            cap_transfer_installed: 0,
            cap_unwrapped: 0,
            cap_transfer_failed: 0,
            extra_caps_sent: 0,
            extra_caps_installed: 0,
            fault_events: 0,
            pair_hits: [0; TCB_STATE_COUNT * TCB_STATE_COUNT],
            forbidden_hits: [0; TCB_STATE_COUNT * TCB_STATE_COUNT],
            budget_sleeps: 0,
            budget_refills: 0,
            finalise_log: [FinaliseEntry {
                tcb: 0,
                state: TcbState::Dead,
                released_sched_context: None,
            }; MAX_FINALISE],
            n_finalise: 0,
            finalise_overflow: 0,
            sched: [HartSched::new(); MAX_HARTS],
            n_harts: harts as usize,
            mail: [HartMailbox::new(); MAX_HARTS],
            published: [Published {
                idle: true,
                cur_prio: 0,
            }; MAX_HARTS],
            current_hart: 0,
            migrate_count: 0,
            preempt_every_step: false,
            now: 0,
            tlb_pending: [TlbPending::EMPTY; MAX_HARTS],
            tlb_seq: 0,
            tlb_local_flushes: 0,
            tlb_remote_posts: 0,
            tlb_serviced: 0,
        }
    }

    /// Khởi tạo **tại chỗ** `Machine` trong `.bss` đã zero (KT-61): chỉ đặt trường **khác 0**
    /// (`n_harts`, `wait_buckets`/`_tail = NODE_NONE`, `sched`/`mail.reset`, `published.idle`).
    pub fn init_zeroed(&mut self, harts: u16) {
        assert!(
            harts > 0 && harts as usize <= MAX_HARTS,
            "vifast-core: số hart phải trong 1..=MAX_HARTS"
        );
        self.n_harts = harts as usize;
        // **KT-64**: arena harness + cửa sổ slot có **cận khác 0** nên zero `.bss` không đủ —
        // đặt lại tại chỗ (cùng lý do `wait_buckets`/`sched`/`mail`).
        self.harness = Region::new(0, HARNESS_SLOT_WORDS * SLOT_BYTES);
        self.wait_buckets = [NODE_NONE; WAITADDR_BUCKETS];
        self.wait_buckets_tail = [NODE_NONE; WAITADDR_BUCKETS];
        // `HartSched`/`HartMailbox` có trường **không zero** (`idle = true`; 8 ô
        // `Option<MailboxMsg>` với `None != 0`) ⇒ reset tại chỗ từng phần tử (không bản tạm lớn).
        for s in self.sched.iter_mut() {
            s.reset();
        }
        for mb in self.mail.iter_mut() {
            mb.reset();
        }
        self.published = [Published {
            idle: true,
            cur_prio: 0,
        }; MAX_HARTS];
    }
}

mod init;
mod lifecycle;
