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

    // ------------------------------------------------------------- cấp phát

    /// Thêm Untyped **do harness cấp** (đáy `base`, `size` byte arena).
    ///
    /// Sim `add_untyped` (KT-16): arena do objmem sở hữu; ở đây arena là `[base, base+size)`
    /// **logic** — `untyped_arena()` trả đúng `(base, size)`, watermark bắt đầu 0.
    ///
    /// # Panics
    /// `size == 0`, hoặc hết bảng `MAX_UNTYPEDS`/`MAX_OBJECTS` (lỗi init của harness; sim
    /// cũng panic khi arena kernel/Untyped hết chỗ).
    pub fn add_untyped(&mut self, base: u64, size: usize) -> ObjId {
        assert!(size > 0, "Untyped phải có cỡ dương");
        assert!(
            self.n_regions < MAX_UNTYPEDS,
            "vifast-core: hết MAX_UNTYPEDS ({MAX_UNTYPEDS})"
        );
        let region = self.n_regions;
        self.regions[region] = Region::new(base, size);
        self.n_regions += 1;
        self.push_meta(
            ObjType::Untyped,
            None,
            0,
            0,
            Some(region),
            None,
            0,
            0,
            BodyRef::NONE,
        )
    }

    /// Thêm CNode **do harness cấp** (init giữ CNode gốc) — như sim `add_cnode`.
    ///
    /// # Panics
    /// `slots == 0` hoặc pool slot/bảng đầy (lỗi init).
    pub fn add_cnode(&mut self, slots: usize) -> ObjId {
        assert!(slots > 0, "CNode phải có ít nhất một slot");
        let offset = self
            .harness
            .probe(ObjType::cnode_bytes(slots), CNODE_BASE_BYTES)
            .expect("vifast-core: arena harness hết chỗ (HARNESS_SLOT_WORDS)");
        let base = harness_slot_base(offset, slots)
            .expect("vifast-core: vùng B slot hết chỗ (HARNESS_SLOT_WORDS)");
        let _ = self
            .harness
            .alloc(ObjType::cnode_bytes(slots), CNODE_BASE_BYTES)
            .expect("arena harness đã probe");
        self.slots.fill_zero(base, slots);
        self.push_meta(
            ObjType::CNode,
            None,
            0,
            0,
            None,
            Some(slots),
            base,
            slots,
            BodyRef::NONE,
        )
    }

    /// Thêm object lõi **do harness cấp** (SchedControl, TCB khởi tạo, ...).
    ///
    /// # Panics
    /// `ty` không phải object lõi, hoặc bảng đầy.
    pub fn add_kernel_object(&mut self, ty: ObjType) -> ObjId {
        assert!(ty.is_core(), "chỉ object lõi mới dựng trực tiếp");
        // Sim: `fresh_kind(CNode, 0)` cho body 1 slot dù metadata `slots = None`; giữ đúng
        // hành vi đó để `cnode_slot_count` của CNode harness-cấp là 1 (không dùng trong test).
        let (slot_base, slot_len, slots_meta) = if ty == ObjType::CNode {
            let offset = self
                .harness
                .probe(ObjType::cnode_bytes(1), CNODE_BASE_BYTES)
                .expect("vifast-core: arena harness hết chỗ (CNode harness)");
            let base = harness_slot_base(offset, 1)
                .expect("vifast-core: vùng B slot hết chỗ (CNode harness)");
            let _ = self
                .harness
                .alloc(ObjType::cnode_bytes(1), CNODE_BASE_BYTES)
                .expect("arena harness đã probe");
            self.slots.fill_zero(base, 1);
            (base, 1, None)
        } else {
            (0, 0, None)
        };
        let body = match ty {
            ObjType::Tcb => self
                .alloc_tcb_body()
                .expect("vifast-core: hết MAX_TCBS (harness)"),
            ObjType::SchedContext => self
                .alloc_sc_body()
                .expect("vifast-core: hết MAX_SCHED_CONTEXTS (harness)"),
            ObjType::Endpoint => self
                .alloc_endpoint_body()
                .expect("vifast-core: hết MAX_ENDPOINTS (harness)"),
            ObjType::Reply => self
                .alloc_reply_body()
                .expect("vifast-core: hết MAX_REPLIES (harness)"),
            ObjType::Notification => self
                .alloc_notification_body()
                .expect("vifast-core: hết MAX_NOTIFICATIONS (harness)"),
            _ => BodyRef::NONE,
        };
        self.push_meta(ty, None, 0, 0, None, slots_meta, slot_base, slot_len, body)
    }

    /// Cap gốc cho một object (không cha) — harness đặt vào CNode bằng [`Machine::put_cap`].
    ///
    /// # Panics
    /// Object không tồn tại hoặc bảng cap đầy.
    pub fn root_cap(&mut self, obj: ObjId) -> CapId {
        let ty = self
            .object_type(obj)
            .expect("root_cap: object phải tồn tại");
        self.push_cap(obj, ty, None, RIGHTS_ALL, None)
    }

    /// Đặt cap (chưa nằm trong slot nào) vào slot của CNode.
    pub fn put_cap(&mut self, cnode: ObjId, slot: usize, cap: CapId) -> Result<(), KError> {
        self.check_cnode_slot(cnode, slot)?;
        if self.cap(cap)?.slot.is_some() {
            return Err(KError::IllegalOperation);
        }
        if self.cnode_slot(cnode, slot)?.is_some() {
            return Err(KError::SlotOccupied);
        }
        self.cnode_set_slot(cnode, slot, Some(cap))?;
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(SlotRef::new(cnode, slot));
        }
        Ok(())
    }

    // ------------------------------------------------------------- truy vấn

    /// Cap trong slot; slot rỗng/ngoài dải/cap chết trả đúng mã của sim.
    pub fn cap_at(&self, cnode: ObjId, slot: usize) -> Result<CapId, KError> {
        let n = self.cnode_slot_count(cnode)?;
        if slot >= n {
            return Err(KError::RangeError);
        }
        match self.cnode_slot(cnode, slot)? {
            None => Err(KError::SlotEmpty),
            Some(cap) => {
                if self.cap(cap)?.alive {
                    Ok(cap)
                } else {
                    Err(KError::InvalidCapability)
                }
            }
        }
    }

    /// Object của cap (cap phải sống).
    pub fn cap_obj(&self, cap: CapId) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if c.alive {
            Ok(c.obj)
        } else {
            Err(KError::InvalidCapability)
        }
    }

    /// Object của cap trong slot.
    pub fn cap_obj_at(&self, cnode: ObjId, slot: usize) -> Result<ObjId, KError> {
        self.cap_obj(self.cap_at(cnode, slot)?)
    }

    /// Ảnh chụp cap; cap **đã chết** vẫn `Ok` với `alive = false` (như sim).
    pub fn cap_info(&self, cap: CapId) -> Result<CapInfo, KError> {
        let c = self.cap(cap)?;
        Ok(CapInfo {
            cap,
            obj: c.obj,
            ty: c.ty,
            parent: c.parent,
            slot: c.slot,
            rights: c.rights,
            badge: c.badge,
            alive: c.alive,
        })
    }

    /// Duyệt toàn bộ bảng cap theo thứ tự cấp phát (gồm cap đã chết).
    pub fn cap_table(&self) -> CapTable<'_> {
        CapTable {
            caps: &self.caps[..self.n_caps],
            idx: 0,
        }
    }

    /// Số cap còn sống.
    pub fn live_cap_count(&self) -> usize {
        self.caps[..self.n_caps].iter().filter(|c| c.alive).count()
    }

    /// Con trực tiếp còn sống của `cap`, theo thứ tự cấp phát (thứ tự MDB của mô hình).
    pub fn cap_children(&self, cap: CapId) -> Result<CapChildren<'_>, KError> {
        self.cap(cap)?;
        Ok(CapChildren {
            caps: &self.caps[..self.n_caps],
            parent: cap,
            idx: 0,
        })
    }

    /// Object còn sống?
    pub fn object_alive(&self, obj: ObjId) -> bool {
        if obj == 0 || obj as usize > self.n_objects {
            return false;
        }
        self.objects[obj as usize - 1].alive
    }

    /// Loại object (kể cả object đã chết — như sim).
    pub fn object_type(&self, obj: ObjId) -> Option<ObjType> {
        if obj == 0 || obj as usize > self.n_objects {
            return None;
        }
        Some(self.objects[obj as usize - 1].ty)
    }

    /// `(offset, len)` trong Untyped; `None` với object harness cấp (như sim `object_bytes`).
    pub fn object_bytes(&self, obj: ObjId) -> Option<(usize, usize)> {
        if obj == 0 || obj as usize > self.n_objects {
            return None;
        }
        let m = &self.objects[obj as usize - 1];
        m.untyped.map(|_| (m.offset, m.len))
    }

    /// Số object đã từng cấp (kể cả đã chết; id = 1..=số này).
    pub fn object_count(&self) -> usize {
        self.n_objects
    }

    /// Số cap đã từng cấp (kể cả đã chết; id = 1..=số này).
    pub fn cap_count(&self) -> usize {
        self.n_caps
    }

    /// Untyped: cỡ byte arena (0 nếu không phải Untyped).
    pub fn untyped_size(&self, obj: ObjId) -> usize {
        if obj == 0 || obj as usize > self.n_objects {
            return 0;
        }
        let m = &self.objects[obj as usize - 1];
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region
            .map(|r| self.regions[r as usize].capacity)
            .unwrap_or(0)
    }

    /// Untyped: watermark byte đã cấp (0 nếu không phải Untyped).
    pub fn untyped_used(&self, obj: ObjId) -> usize {
        if obj == 0 || obj as usize > self.n_objects {
            return 0;
        }
        let m = &self.objects[obj as usize - 1];
        if m.ty != ObjType::Untyped {
            return 0;
        }
        m.region
            .map(|r| self.regions[r as usize].cursor)
            .unwrap_or(0)
    }

    /// Untyped: byte còn tự do.
    pub fn untyped_free(&self, obj: ObjId) -> usize {
        self.untyped_size(obj)
            .saturating_sub(self.untyped_used(obj))
    }

    /// Tổng `len` của object **sống** cấp từ Untyped (không gồm byte đệm căn lề).
    pub fn untyped_allocated(&self, obj: ObjId) -> usize {
        if obj == 0 || obj as usize > self.n_objects {
            return 0;
        }
        let idx = obj as usize - 1;
        self.objects[..self.n_objects]
            .iter()
            .filter(|m| m.alive && m.untyped.map(usize::from) == Some(idx))
            .map(|m| m.len)
            .sum()
    }

    /// Byte đệm căn lề nằm giữa watermark và tổng `len` object sống.
    pub fn untyped_gap(&self, obj: ObjId) -> usize {
        self.untyped_used(obj)
            .saturating_sub(self.untyped_allocated(obj))
    }

    /// Tổng watermark của mọi Untyped còn sống.
    pub fn total_used(&self) -> usize {
        self.objects[..self.n_objects]
            .iter()
            .filter(|m| m.alive && m.ty == ObjType::Untyped)
            .map(|m| {
                m.region
                    .map(|r| self.regions[r as usize].cursor)
                    .unwrap_or(0)
            })
            .sum()
    }

    /// `(base, capacity)` arena của Untyped (kể cả đã chết — như sim).
    pub fn untyped_arena(&self, obj: ObjId) -> Option<(u64, usize)> {
        if obj == 0 || obj as usize > self.n_objects {
            return None;
        }
        let m = &self.objects[obj as usize - 1];
        if m.ty != ObjType::Untyped {
            return None;
        }
        let r = m.region? as usize;
        Some((self.regions[r].base, self.regions[r].capacity))
    }

    // ------------------------------------------------------------- Retype

    /// Cấp object từ Untyped theo `type/size_bits/flags` (spec `Untyped.Retype`).
    ///
    /// Trả `CapId` cho harness (ABI `returns = []`); cap là con của cap nguồn, **kế thừa**
    /// quyền cap nguồn. Mọi lỗi **nguyên tử**: không đốt byte/cap/object.
    pub fn retype(
        &mut self,
        src: SlotRef,
        ty: ObjType,
        size_bits: u32,
        flags: u32,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (u_idx, u_region, u_size, src_rights) = {
            let c = self.cap(src_cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let idx = self.object_index(c.obj)?;
            let m = &self.objects[idx];
            if m.ty != ObjType::Untyped {
                return Err(KError::WrongObjectType);
            }
            let region = m.region.map(usize::from).ok_or(KError::WrongObjectType)?;
            (idx, region, self.regions[region].capacity, c.rights)
        };
        if src_rights & RIGHT_WRITE == 0 {
            return Err(KError::InsufficientRights);
        }
        if flags & FLAG_GUEST != 0 {
            return Err(if ty == ObjType::VSpace {
                KError::NotSupported
            } else {
                KError::InvalidArgument
            });
        }
        if !ty.is_core() {
            return Err(KError::NotSupported);
        }

        let (len, align, child_slots) = match ty {
            ObjType::Untyped => {
                if !(12..=30).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let len = 1usize << size_bits;
                if len > u_size {
                    return Err(KError::NotEnoughMemory);
                }
                // KT-17: arena của sim chỉ bảo đảm căn `ARENA_ALIGN` (4096 B).
                if len > ARENA_ALIGN {
                    return Err(KError::Alignment);
                }
                (len, len, 0)
            }
            ObjType::CNode => {
                if !(4..=20).contains(&size_bits) {
                    return Err(KError::InvalidArgument);
                }
                let slots = 1usize << size_bits;
                (ObjType::cnode_bytes(slots), CNODE_BASE_BYTES, slots)
            }
            _ => {
                let fixed = ty.fixed_size().ok_or(KError::InvalidArgument)?;
                if size_bits != 0 {
                    if size_bits > 30 {
                        return Err(KError::InvalidArgument);
                    }
                    let requested = 1usize << size_bits;
                    if requested < fixed {
                        return Err(KError::Alignment);
                    }
                    if requested > fixed {
                        return Err(KError::InvalidArgument);
                    }
                }
                (fixed, DEFAULT_ALIGN_BYTES, 0)
            }
        };

        // Hạn mức bảng tĩnh (khác sim chỉ có hạn arena): kiểm **trước** mọi thay đổi để
        // `NotEnoughMemory` nguyên tử.
        let need_region = ty == ObjType::Untyped;
        if self.n_objects >= MAX_OBJECTS
            || self.n_caps >= MAX_CAPS
            || (need_region && self.n_regions >= MAX_UNTYPEDS)
            || (ty == ObjType::Tcb && !self.tcb_body_available())
            || (ty == ObjType::SchedContext && !self.sc_body_available())
            || (ty == ObjType::Endpoint && !self.endpoint_body_available())
            || (ty == ObjType::Reply && !self.reply_body_available())
            || (ty == ObjType::Notification && !self.notification_body_available())
        {
            return Err(KError::NotEnoughMemory);
        }
        // **KT-64** — cửa sổ byte slot phải nằm trong arena (vùng A) **trước** khi cấp: kiểm
        // bằng `Region::probe` (không đổi watermark) ⇒ lỗi nguyên tử, không đọc ngoài mảng.
        let slot_win = if ty == ObjType::CNode {
            let probe = self.regions[u_region]
                .probe(len, align)
                .ok_or(KError::NotEnoughMemory)?;
            Some(arena_slot_base(u_region, probe, child_slots).ok_or(KError::NotEnoughMemory)?)
        } else {
            None
        };
        let offset = self.regions[u_region]
            .alloc(len, align)
            .ok_or(KError::NotEnoughMemory)?;
        let region = if ty == ObjType::Untyped {
            let base = self.regions[u_region]
                .base
                .checked_add(offset as u64)
                .ok_or(KError::InvalidArgument)?;
            let idx = self.n_regions;
            self.regions[idx] = Region::new(base, len);
            self.n_regions += 1;
            Some(idx)
        } else {
            None
        };
        let (slot_base, slot_len) = match slot_win {
            Some(base) => {
                // Cửa sổ mới (hoặc tái dùng sau `Revoke`) phải **trắng** — như CNode mới của sim.
                self.slots.fill_zero(base, child_slots);
                (base, child_slots)
            }
            None => (0, 0),
        };
        // Đã kiểm sức chứa body ở trên ⇒ chỉ có thể thành công; lỗi nội bộ là bất biến vỡ.
        let body = match ty {
            ObjType::Tcb => self.alloc_tcb_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::SchedContext => self.alloc_sc_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::Endpoint => self.alloc_endpoint_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::Reply => self.alloc_reply_body().ok_or(KError::NotEnoughMemory)?,
            ObjType::Notification => self
                .alloc_notification_body()
                .ok_or(KError::NotEnoughMemory)?,
            _ => BodyRef::NONE,
        };
        let obj = self.push_meta(
            ty,
            Some(u_idx),
            offset,
            len,
            region,
            if ty == ObjType::CNode {
                Some(child_slots)
            } else {
                None
            },
            slot_base,
            slot_len,
            body,
        );
        Ok(self.push_cap(obj, ty, Some(src_cap), src_rights, None))
    }

    // ------------------------------------------------------------- CSpace

    /// `CNode.Copy`: tạo cap con ở đích; đích phải rỗng. Giữ quyền + badge nguồn.
    pub fn copy(&mut self, src: SlotRef, dst: SlotRef) -> Result<CapId, KError> {
        self.copy_rights(src, dst, None)
    }

    /// `CNode.Copy` với `rights?`: `None` = giữ quyền nguồn; `Some(r)` phải là tập con.
    pub fn copy_rights(
        &mut self,
        src: SlotRef,
        dst: SlotRef,
        rights: Option<u8>,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (obj, ty, src_rights, badge) = {
            let c = self.cap(src_cap)?;
            (c.obj, c.ty, c.rights, c.badge)
        };
        let rights = match rights {
            Some(r) => {
                if r & !src_rights != 0 {
                    return Err(KError::InsufficientRights);
                }
                r
            }
            None => src_rights,
        };
        self.derive(src_cap, obj, ty, dst, rights, badge)
    }

    /// `CNode.Mint`: như Copy nhưng thu hẹp quyền và đặt badge (chỉ Endpoint/Notification).
    pub fn mint(
        &mut self,
        src: SlotRef,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        let src_cap = self.cap_at(src.cnode, src.slot)?;
        let (obj, ty, src_rights, src_badge) = {
            let c = self.cap(src_cap)?;
            (c.obj, c.ty, c.rights, c.badge)
        };
        if rights & !src_rights != 0 {
            return Err(KError::InsufficientRights);
        }
        let badge = match badge {
            None => src_badge,
            Some(0) => None,
            Some(b) => {
                if !matches!(ty, ObjType::Endpoint | ObjType::Notification) {
                    return Err(KError::InvalidArgument);
                }
                Some(b)
            }
        };
        self.derive(src_cap, obj, ty, dst, rights, badge)
    }

    /// Tạo cap con của `src_cap` rồi gắn vào `dst` — đã kiểm nguồn/quyền trước khi cấp.
    fn derive(
        &mut self,
        src_cap: CapId,
        obj: ObjId,
        ty: ObjType,
        dst: SlotRef,
        rights: u8,
        badge: Option<u64>,
    ) -> Result<CapId, KError> {
        self.check_cnode_slot(dst.cnode, dst.slot)?;
        if self.cnode_slot(dst.cnode, dst.slot)?.is_some() {
            return Err(KError::SlotOccupied);
        }
        let new_cap = self.push_cap(obj, ty, Some(src_cap), rights, badge);
        self.cnode_set_slot(dst.cnode, dst.slot, Some(new_cap))?;
        self.caps[new_cap as usize - 1].slot = Some(dst);
        Ok(new_cap)
    }

    /// `CNode.Move` (KT-68): chuyển cap nguồn sang `dst`, **xoá nguồn**.
    ///
    /// Ngữ nghĩa **D2-44**: `dst` đã có cap thì cap đích bị `delete` trước (con của cap đích
    /// được reparent lên ông — MDB kiểu seL4), rồi cap nguồn rời slot cũ sang `dst`; **không**
    /// tạo cap mới (cùng `CapId`, giữ nguyên cha MDB, quyền, badge). `src == dst` ⇒
    /// `IllegalOperation` (không có việc gì để làm, không được tự xoá).
    pub fn move_cap(&mut self, src: SlotRef, dst: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(src.cnode, src.slot)?;
        self.check_cnode_slot(dst.cnode, dst.slot)?;
        if src == dst {
            return Err(KError::IllegalOperation);
        }
        if self.cnode_slot(dst.cnode, dst.slot)?.is_some() {
            self.delete(dst)?;
        }
        self.clear_slot_of(cap);
        self.cnode_set_slot(dst.cnode, dst.slot, Some(cap))?;
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.slot = Some(dst);
        }
        Ok(())
    }

    /// `CNode.Delete`: xoá cap trong slot (object không bị finalise — chỉ Revoke mới).
    ///
    /// Con của cap bị xoá thuộc về **ông** (tổ tiên sống gần nhất) — MDB kiểu seL4.
    pub fn delete(&mut self, slot: SlotRef) -> Result<(), KError> {
        let cap = self.cap_at(slot.cnode, slot.slot)?;
        let (cap_ty, cap_obj) = {
            let c = self.cap(cap)?;
            (c.ty, c.obj)
        };
        let mut new_parent = self.cap(cap)?.parent;
        while let Some(p) = new_parent {
            if p != 0 && (p as usize) <= self.n_caps && self.caps[p as usize - 1].alive {
                break;
            }
            new_parent = if p != 0 && (p as usize) <= self.n_caps {
                self.caps[p as usize - 1].parent
            } else {
                None
            };
        }
        for c in &mut self.caps[..self.n_caps] {
            if c.alive && c.parent == Some(cap) {
                c.parent = new_parent;
            }
        }
        self.clear_slot_of(cap);
        if let Some(c) = self.caps.get_mut(cap as usize - 1) {
            c.alive = false;
            c.parent = None;
        }
        // **KT-63** — cap Reply bị xoá ⇒ caller đang `BlockedOnReply` được đánh thức
        // `Cancelled` + `Ready` (D2-53 Q8, như `Machine::delete` của sim); waiter Recv còn giữ
        // cap Reply **đã chết** được coi như "không cấp Reply" (Q7b) thay vì lỗi cứng.
        if cap_ty == ObjType::Reply && self.object_alive(cap_obj) {
            self.cancel_reply_object(cap_obj);
        }
        if cap_ty == ObjType::Reply {
            self.clear_dead_reply_in_waiters();
        }
        // **KT-64** — cap Notification bị xoá ⇒ gỡ binding `SchedContext.bound` đang trỏ **cap
        // này** (không để `bound` trỏ cap chết, `bound_sc` mồ côi) — như sim `delete`.
        if cap_ty == ObjType::Notification && self.object_alive(cap_obj) {
            self.drop_sched_binding_to_cap(cap, cap_obj);
        }
        Ok(())
    }

    /// **KT-67a (reviewer L2)** — bỏ một cap **chưa nằm trong slot** (rollback nội bộ khi cài
    /// cap đại diện thất bại): đánh dấu cap chết + reparent con về ông gần nhất, **không** đụng
    /// CNode. Từ chối cap đang ở slot (dùng `delete`) hoặc cap đã chết.
    pub fn drop_root_cap(&mut self, cap: CapId) -> Result<(), KError> {
        let c = self.cap(cap)?;
        if !c.alive || c.slot.is_some() {
            return Err(KError::IllegalOperation);
        }
        let mut new_parent = c.parent;
        while let Some(p) = new_parent {
            if p != 0 && (p as usize) <= self.n_caps && self.caps[p as usize - 1].alive {
                break;
            }
            new_parent = if p != 0 && (p as usize) <= self.n_caps {
                self.caps[p as usize - 1].parent
            } else {
                None
            };
        }
        for other in &mut self.caps[..self.n_caps] {
            if other.alive && other.parent == Some(cap) {
                other.parent = new_parent;
            }
        }
        if let Some(entry) = self.caps.get_mut(cap as usize - 1) {
            entry.alive = false;
            entry.parent = None;
        }
        Ok(())
    }

    /// `CNode.Revoke`: xoá con/cháu của cap trong `slot` (hoặc mọi cap của CNode khi
    /// `slot = None`). Revoke cap Untyped finalise mọi object trong cây Untyped và đặt
    /// watermark về 0. Trả số cap bị xoá.
    pub fn revoke(&mut self, cnode: ObjId, slot: Option<usize>) -> Result<usize, KError> {
        // D2-83/KT-58: sim đọc **body** CNode trước (`cnode_slots_of`), nên CNode đã finalise
        // trả `InvalidCapability` (body `Dead`), không đọc metadata. `cnode_slot_count` thì
        // vẫn trả số slot của metadata (API truy vấn) — `Revoke` phải đi cửa body như sim.
        let n = {
            let m = self.meta(cnode)?;
            if !m.alive {
                return Err(KError::InvalidCapability);
            }
            if m.ty != ObjType::CNode {
                return Err(KError::WrongObjectType);
            }
            m.slot_len
        };
        let mut deleted = 0;
        match slot {
            Some(s) => {
                if s >= n {
                    return Err(KError::RangeError);
                }
                match self.cnode_slot(cnode, s)? {
                    None => Err(KError::SlotEmpty),
                    Some(cap) => {
                        if self.cap(cap)?.alive {
                            deleted += self.revoke_children(cap)?;
                        }
                        Ok(deleted)
                    }
                }
            }
            None => {
                for s in 0..n {
                    if let Some(cap) = self.cnode_slot(cnode, s)? {
                        if self.cap(cap)?.alive {
                            deleted += self.revoke_children(cap)?;
                        }
                    }
                }
                Ok(deleted)
            }
        }
    }

    /// Xoá cap con/cháu của `root`; nếu `root` là cap Untyped thì finalise object + reset
    /// arena của cả cây Untyped (watermark về 0, thiết kế §4.2).
    fn revoke_children(&mut self, root: CapId) -> Result<usize, KError> {
        if !self.cap(root)?.alive {
            return Err(KError::InvalidCapability);
        }
        let mut doomed = [0u32; MAX_CAPS];
        let mut nd = 0usize;
        for i in 1..=self.n_caps as CapId {
            if !self.caps[i as usize - 1].alive || i == root {
                continue;
            }
            let mut parent = self.caps[i as usize - 1].parent;
            let mut steps = 0usize;
            while let Some(p) = parent {
                if p == root {
                    push_id(&mut doomed, &mut nd, i);
                    break;
                }
                if p == 0 || p as usize > self.n_caps {
                    break;
                }
                parent = self.caps[p as usize - 1].parent;
                steps += 1;
                if steps > self.n_caps {
                    break;
                }
            }
        }

        let root_ty = self.cap(root)?.ty;
        let root_obj = self.cap(root)?.obj;
        if root_ty == ObjType::Untyped {
            let u_idx = self.object_index(root_obj)?;
            let mut dead = [0u32; MAX_OBJECTS];
            let mut ndead = 0usize;
            self.collect_untyped_tree(u_idx, &mut dead, &mut ndead);
            // Finalise CNode (seL4 `finaliseCNode`): cap nằm trong slot của CNode chết phải
            // chết theo **cả cây con của nó**, kể cả khi object đích còn sống.
            for &id in &dead[..ndead] {
                if self.objects[id as usize - 1].ty != ObjType::CNode {
                    continue;
                }
                let len = self.objects[id as usize - 1].slot_len;
                for s in 0..len {
                    if let Some(cap) = self.cnode_slot(id, s)? {
                        if self.cap(cap)?.alive {
                            self.collect_subtree(cap, &mut doomed, &mut nd);
                        }
                    }
                }
            }
            // Slot của CNode chết không giữ cap nào.
            for &id in &dead[..ndead] {
                if self.objects[id as usize - 1].ty == ObjType::CNode {
                    let (base, len) = {
                        let m = &self.objects[id as usize - 1];
                        (m.slot_base, m.slot_len)
                    };
                    self.slots.fill_zero(base, len);
                }
            }
            // KT-18/KT-59: finalise TCB **trước** khi object chết (gỡ ready queue, `Dead`,
            // trả `sched_context` qua hook, ghi log) — như sim `revoke_children`.
            for &id in &dead[..ndead] {
                if self.objects[id as usize - 1].ty == ObjType::Tcb {
                    let _ = self.finalise_tcb(id);
                }
            }
            // **KT-63** — finalise IPC của object chết **trước** khi body về free-list:
            // Reply huỷ caller (`Cancelled + Ready`), Endpoint đánh thức Recv waiter + đếm
            // message bị huỷ (`ipc_dropped`), Notification đánh thức waiter (chống treo).
            for &id in &dead[..ndead] {
                match self.objects[id as usize - 1].ty {
                    ObjType::Reply => {
                        self.cancel_reply_object(id);
                    }
                    ObjType::Endpoint => self.finalise_endpoint_body(id),
                    ObjType::Notification => {
                        // **KT-64** — gỡ `SchedContext.bound` trỏ notification sắp chết.
                        self.drop_sched_binding_to_obj(id);
                        self.finalise_notification_body(id);
                    }
                    _ => {}
                }
            }
            // Đánh dấu chết + trả word slot pool + trả body TCB/SchedContext về free-list.
            for &id in &dead[..ndead] {
                let idx = id as usize - 1;
                let body = self.objects[idx].body;
                {
                    let m = &mut self.objects[idx];
                    m.alive = false;
                    m.body.alive = false;
                }
                match body.kind {
                    BODY_TCB => self.free_tcb_body(body.idx),
                    BODY_SC => self.free_sc_body(body.idx),
                    BODY_ENDPOINT => self.free_endpoint_body(body.idx),
                    BODY_REPLY => self.free_reply_body(body.idx),
                    BODY_NOTIFICATION => self.free_notification_body(body.idx),
                    _ => {}
                }
                let (base, len) = (self.objects[idx].slot_base, self.objects[idx].slot_len);
                if len > 0 {
                    // Không còn free-list: byte slot trả lại **cùng** byte arena (`Region::reset`
                    // ngay dưới); chỉ xoá cửa sổ để không ai đọc rác.
                    self.slots.fill_zero(base, len);
                }
            }
            // Reset arena của **mọi** Untyped trong cây + Untyped gốc.
            for &id in &dead[..ndead] {
                if let Some(r) = self.objects[id as usize - 1].region {
                    self.regions[r as usize].reset();
                }
            }
            if let Some(r) = self.objects[u_idx].region {
                self.regions[r as usize].reset();
            }
            // Cap còn sống trỏ object vừa chết cũng phải chết (không cap nào trỏ object chết).
            for i in 1..=self.n_caps as CapId {
                if self.caps[i as usize - 1].alive
                    && dead[..ndead].contains(&self.caps[i as usize - 1].obj)
                {
                    push_id(&mut doomed, &mut nd, i);
                }
            }
        }

        sort_dedup(&mut doomed, &mut nd);
        // **KT-63** — Reply mất cap cuối qua `Revoke` cũng phải huỷ đường reply (Q8): ghi lại
        // object Reply của các cap sắp chết để huỷ **sau** khi xoá (như `revoke_children` sim).
        let mut reply_objs = [0u32; MAX_CAPS];
        let mut n_reply = 0usize;
        for &cap in &doomed[..nd] {
            let c = self.caps[cap as usize - 1];
            if c.alive && c.ty == ObjType::Reply && !reply_objs[..n_reply].contains(&c.obj) {
                reply_objs[n_reply] = c.obj;
                n_reply += 1;
            }
        }
        let mut deleted = 0;
        for &cap in &doomed[..nd] {
            if self.caps[cap as usize - 1].alive {
                self.clear_slot_of(cap);
                self.caps[cap as usize - 1].alive = false;
                deleted += 1;
            }
        }
        for &obj in &reply_objs[..n_reply] {
            self.cancel_reply_object(obj);
        }
        self.clear_dead_reply_in_waiters();
        Ok(deleted)
    }

    // ------------------------------------------------------------- nội bộ

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_meta(
        &mut self,
        ty: ObjType,
        untyped: Option<usize>,
        offset: usize,
        len: usize,
        region: Option<usize>,
        slots_meta: Option<usize>,
        slot_base: usize,
        slot_len: usize,
        body: BodyRef,
    ) -> ObjId {
        assert!(
            self.n_objects < MAX_OBJECTS,
            "vifast-core: hết MAX_OBJECTS ({MAX_OBJECTS})"
        );
        // **KT-312** — thu hẹp kiểu **có chứng minh**: ba trường chỉ số chỉ nhận giá trị trong
        // miền của bảng tương ứng (`untyped` = chỉ số **object** < `MAX_OBJECTS`; `region` =
        // chỉ số arena < `MAX_UNTYPEDS`; `slots_meta` = số slot CNode ≤ `MAX_SLOT_WORDS`).
        // `debug_assert` giữ bất biến này khi phát triển; bản release dùng `as` (không thể tràn
        // vì mọi caller đã bị chặn bởi `MAX_OBJECTS`/`MAX_UNTYPEDS`/`HARNESS_SLOT_WORDS`).
        let untyped = untyped.map(|u| {
            debug_assert!(u < MAX_OBJECTS, "untyped {u} >= MAX_OBJECTS");
            u as u8
        });
        let region = region.map(|r| {
            debug_assert!(r < MAX_UNTYPEDS, "region {r} >= MAX_UNTYPEDS");
            r as u8
        });
        let slots_meta = slots_meta.map(|n| {
            debug_assert!(n <= MAX_SLOT_WORDS, "slots_meta {n} > MAX_SLOT_WORDS");
            n as u16
        });
        self.objects[self.n_objects] = Meta {
            ty,
            alive: true,
            untyped,
            offset,
            len,
            region,
            slots_meta,
            slot_len,
            slot_base,
            body,
        };
        self.n_objects += 1;
        self.n_objects as ObjId
    }

    pub(crate) fn push_cap(
        &mut self,
        obj: ObjId,
        ty: ObjType,
        parent: Option<CapId>,
        rights: u8,
        badge: Option<u64>,
    ) -> CapId {
        assert!(
            self.n_caps < MAX_CAPS,
            "vifast-core: hết MAX_CAPS ({MAX_CAPS})"
        );
        self.caps[self.n_caps] = Cap {
            obj,
            ty,
            parent,
            slot: None,
            rights,
            badge,
            alive: true,
        };
        self.n_caps += 1;
        self.n_caps as CapId
    }

    fn tcb_body_available(&self) -> bool {
        self.n_tcb_free > 0 || self.n_tcbs < MAX_TCBS
    }

    fn sc_body_available(&self) -> bool {
        self.n_sc_free > 0 || self.n_scs < MAX_SCHED_CONTEXTS
    }

    fn alloc_tcb_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_tcb_free > 0 {
            self.n_tcb_free -= 1;
            self.tcb_free[self.n_tcb_free] as usize
        } else if self.n_tcbs < MAX_TCBS {
            let i = self.n_tcbs;
            self.n_tcbs += 1;
            i
        } else {
            return None;
        };
        self.tcbs[idx] = TcbData::EMPTY;
        Some(BodyRef {
            kind: BODY_TCB,
            idx: idx as u16,
            alive: true,
        })
    }

    fn alloc_sc_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_sc_free > 0 {
            self.n_sc_free -= 1;
            self.sc_free[self.n_sc_free] as usize
        } else if self.n_scs < MAX_SCHED_CONTEXTS {
            let i = self.n_scs;
            self.n_scs += 1;
            i
        } else {
            return None;
        };
        self.scs[idx] = ScData::EMPTY;
        Some(BodyRef {
            kind: BODY_SC,
            idx: idx as u16,
            alive: true,
        })
    }

    fn free_tcb_body(&mut self, idx: u16) {
        if self.n_tcb_free < MAX_TCBS {
            self.tcb_free[self.n_tcb_free] = idx;
            self.n_tcb_free += 1;
        }
    }

    fn free_sc_body(&mut self, idx: u16) {
        if self.n_sc_free < MAX_SCHED_CONTEXTS {
            self.sc_free[self.n_sc_free] = idx;
            self.n_sc_free += 1;
        }
    }

    pub(crate) fn endpoint_body_available(&self) -> bool {
        self.n_endpoint_free > 0 || self.n_endpoints < MAX_ENDPOINTS
    }

    pub(crate) fn reply_body_available(&self) -> bool {
        self.n_reply_free > 0 || self.n_replies < MAX_REPLIES
    }

    pub(crate) fn notification_body_available(&self) -> bool {
        self.n_notification_free > 0 || self.n_notifications < MAX_NOTIFICATIONS
    }

    fn alloc_endpoint_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_endpoint_free > 0 {
            self.n_endpoint_free -= 1;
            self.endpoint_free[self.n_endpoint_free] as usize
        } else if self.n_endpoints < MAX_ENDPOINTS {
            let i = self.n_endpoints;
            self.n_endpoints += 1;
            i
        } else {
            return None;
        };
        self.endpoints[idx] = EndpointData::EMPTY;
        Some(BodyRef {
            kind: BODY_ENDPOINT,
            idx: idx as u16,
            alive: true,
        })
    }

    fn alloc_reply_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_reply_free > 0 {
            self.n_reply_free -= 1;
            self.reply_free[self.n_reply_free] as usize
        } else if self.n_replies < MAX_REPLIES {
            let i = self.n_replies;
            self.n_replies += 1;
            i
        } else {
            return None;
        };
        self.replies[idx] = ReplyData::EMPTY;
        Some(BodyRef {
            kind: BODY_REPLY,
            idx: idx as u16,
            alive: true,
        })
    }

    fn alloc_notification_body(&mut self) -> Option<BodyRef> {
        let idx = if self.n_notification_free > 0 {
            self.n_notification_free -= 1;
            self.notification_free[self.n_notification_free] as usize
        } else if self.n_notifications < MAX_NOTIFICATIONS {
            let i = self.n_notifications;
            self.n_notifications += 1;
            i
        } else {
            return None;
        };
        self.notifications[idx] = NotificationData::default();
        Some(BodyRef {
            kind: BODY_NOTIFICATION,
            idx: idx as u16,
            alive: true,
        })
    }

    fn free_endpoint_body(&mut self, idx: u16) {
        // Message đang chờ trong hàng của Endpoint bị finalise phải trả về pool (không rò
        // chỉ số pool sau revoke) — tương đương sim huỷ `Box` khi body chết.
        let mut q = self.endpoints[idx as usize].send;
        while let Some(m) = q.pop_front() {
            self.free_ipc_msg(m);
        }
        self.endpoints[idx as usize] = EndpointData::EMPTY;
        if self.n_endpoint_free < MAX_ENDPOINTS {
            self.endpoint_free[self.n_endpoint_free] = idx;
            self.n_endpoint_free += 1;
        }
    }

    fn free_reply_body(&mut self, idx: u16) {
        self.replies[idx as usize] = ReplyData::EMPTY;
        if self.n_reply_free < MAX_REPLIES {
            self.reply_free[self.n_reply_free] = idx;
            self.n_reply_free += 1;
        }
    }

    fn free_notification_body(&mut self, idx: u16) {
        self.notifications[idx as usize] = NotificationData::default();
        if self.n_notification_free < MAX_NOTIFICATIONS {
            self.notification_free[self.n_notification_free] = idx;
            self.n_notification_free += 1;
        }
    }

    pub(crate) fn object_index(&self, obj: ObjId) -> Result<usize, KError> {
        if obj == 0 || obj as usize > self.n_objects {
            return Err(KError::InvalidCapability);
        }
        Ok(obj as usize - 1)
    }
}
