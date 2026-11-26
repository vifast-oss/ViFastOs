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
//! 3. Không `Retype` `Frame`/`PageTable`/`VSpace` (K2 object layer `vifast_core::vm`,
//!    KT-52) — trả `NotSupported`; sim (K2) đã mở `Frame`/`PageTable`, xem `docs/k3b-core.md`.
//! 4. **Donation KT-21** là phần của `Call`/`Reply` (KT-60): KT-59 giữ trường
//!    `donated_sc`/`donated_to`/`donor` + công thức `donated_priority`, nhưng chưa có đường tạo
//!    donation — parity KT-59 phủ trạng thái **không donation** (ghi trong docs §7).
//! 5. **Reply/Notification/IPC** chưa có body ⇒ `unlink_tcb_queues` chỉ là mốc (không có hàng
//!    chờ nào của KT-24..28 để gỡ); `TlbFlush` trong hộp thư là no-op (core chưa có TLB).

use crate::arena::Region;
use crate::captable::{CapChildren, CapTable};
use crate::error::{CapId, CapInfo, KError, ObjId, ObjType, SlotRef};
use crate::mdb::{push_id, sort_dedup};
use crate::observer::{CoreObserver, TestObserver};
use crate::slotmem::{arena_slot_base, harness_slot_base};
use crate::spec::{
    ARENA_ALIGN, CNODE_BASE_BYTES, DEFAULT_ALIGN_BYTES, FLAG_GUEST, RIGHTS_ALL, RIGHT_WRITE,
};
use crate::store::{KernelStore, StaticStore};
use crate::tcbdata::{ScData, TcbData};

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
    BodyRef, BODY_ENDPOINT, BODY_NOTIFICATION, BODY_REPLY, BODY_SC, BODY_TCB,
};
pub use crate::objmeta::{Cap, Meta};

/// Core model K3b — **không** `unsafe`, **không** cấp phát động.
///
/// **KT-405** — `O: CoreObserver` tách dữ liệu **oracle/test** (`pair_hits`/`forbidden_hits`/
/// `finalise_log`) khỏi sổ cái sản phẩm: `Machine<StaticStore, NoopObserver>` **0 B** cho phần đó,
/// `Machine<StaticStore, TestObserver>` (mặc định) giữ nguyên ngữ nghĩa cũ cho sim/ktest/test host.
#[derive(Debug, Clone, Copy)]
pub struct Machine<S: KernelStore = StaticStore, O: CoreObserver = TestObserver> {
    pub(crate) regions: S::Regions,
    pub(crate) n_regions: usize,
    pub(crate) objects: S::Objects,
    pub(crate) n_objects: usize,
    pub(crate) caps: S::Caps,
    pub(crate) n_caps: usize,
    /// Cửa sổ backing của **byte slot trong arena** (KT-64) — không còn pool riêng.
    pub(crate) slots: S::Slots,
    /// Arena logic cho CNode **harness** (kernel cấp, không có byte Untyped) — như body heap
    /// của `add_cnode` trong sim; byte slot của nó dùng vùng B của [`SlotMem`].
    pub(crate) harness: S::Harness,
    /// Body TCB (KT-59) + free-list chỉ số.
    pub(crate) tcbs: S::Tcbs,
    pub(crate) n_tcbs: usize,
    pub(crate) tcb_free: S::TcbFree,
    pub(crate) n_tcb_free: usize,
    /// Body SchedContext (KT-59) + free-list chỉ số.
    pub(crate) scs: S::Scs,
    pub(crate) n_scs: usize,
    pub(crate) sc_free: S::ScFree,
    pub(crate) n_sc_free: usize,
    /// Body Endpoint/Reply/Notification (KT-60) + free-list chỉ số.
    pub(crate) endpoints: S::Endpoints,
    pub(crate) n_endpoints: usize,
    pub(crate) endpoint_free: S::EndpointFree,
    pub(crate) n_endpoint_free: usize,
    pub(crate) replies: S::Replies,
    pub(crate) n_replies: usize,
    pub(crate) reply_free: S::ReplyFree,
    pub(crate) n_reply_free: usize,
    pub(crate) notifications: S::Notifications,
    pub(crate) n_notifications: usize,
    pub(crate) notification_free: S::NotificationFree,
    pub(crate) n_notification_free: usize,
    /// Pool message đang chờ trong hàng gửi Endpoint (KT-60) + free-list chỉ số.
    pub(crate) ipc_msgs: S::IpcMsgs,
    pub(crate) n_ipc_msgs: usize,
    pub(crate) ipc_msg_free: S::IpcMsgFree,
    pub(crate) n_ipc_msg_free: usize,
    /// Bảng WaitOnAddress: `(frame, offset) -> u64` + bucket waiter (KT-60).
    pub(crate) addr_words: S::AddrWords,
    pub(crate) n_addr_words: usize,
    pub(crate) wait_nodes: S::WaitNodes,
    pub(crate) n_wait_nodes: usize,
    pub(crate) wait_free: S::WaitFree,
    pub(crate) n_wait_free: usize,
    pub(crate) wait_buckets: S::WaitBuckets,
    pub(crate) wait_buckets_tail: S::WaitBucketsTail,
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
    /// Số lần `Running → Sleeping{Budget}` (KT-20/KT-23).
    pub(crate) budget_sleeps: u64,
    /// Số lần một `SchedContext` được nạp lại (KT-20/KT-23).
    pub(crate) budget_refills: u64,
    /// **KT-405** — trạng thái **theo hart** (`sched`/`published`/`mail`/`tlb_pending`) tách
    /// khỏi sổ cái object: một khối `[PerHartState; MAX_HARTS]` thay bốn bảng rời. Đây là
    /// **ranh giới dữ liệu** để `KT-412`/`KT-413` hạ khoá từ "toàn `Machine`" xuống "một hart"
    /// (D2-259 mục 6) — KT-405 **chưa** đổi khoá, chỉ tách cấu trúc + accessor.
    pub(crate) harts: S::Harts,
    /// Số hart đang dùng (`1..=MAX_HARTS`).
    pub(crate) n_harts: usize,
    /// Hart "đang chạy" của mô hình tại thời điểm gọi (KT-29).
    pub(crate) current_hart: u16,
    /// Số lần `finalise_tcb` bị từ chối vì nhật ký đầy (chẩn đoán; **không** phải ABI).
    ///
    /// **KT-405** — **không** chuyển vào [`CoreObserver`]: đây là bộ đếm chẩn đoán mà sản phẩm
    /// cũng có (luôn `0` vì `NoopObserver::finalise_is_full()` luôn `false`), không phải dữ liệu
    /// oracle. Đặt ngay sau `current_hart` để **lấp** 4 trong 6 byte đệm sẵn có trước
    /// `migrate_count: u64` ⇒ `size_of::<Machine>()` **không** đổi và `scalars` giữ `311`
    /// (pin của `kt60_core_parity` phía sim).
    pub(crate) finalise_overflow: u32,
    /// Số TCB đã di trú thành công (KT-29).
    pub(crate) migrate_count: u64,
    /// Preempt ép tại mọi public op (KT-19, mặc định tắt).
    pub(crate) preempt_every_step: bool,
    /// Thời gian ảo (tick) — **không** đọc wall clock (D2-49(h)).
    pub(crate) now: u64,
    /// Số hiệu shootdown toàn cục (1-based, tăng đơn điệu).
    pub(crate) tlb_seq: u64,
    /// Số lần flush **cục bộ** đã thực thi qua hook (mọi hart).
    pub(crate) tlb_local_flushes: u64,
    /// Số `TlbFlush` đã post remote.
    pub(crate) tlb_remote_posts: u64,
    /// Số lần hart đích đã phục vụ + ack.
    pub(crate) tlb_serviced: u64,
    /// **KT-405** — dữ liệu oracle/test (bộ đếm cặp + nhật ký finalise) sau trait
    /// [`CoreObserver`]; `NoopObserver` = **0 B** ở sản phẩm.
    pub(crate) obs: O,
}

impl<S: KernelStore, O: CoreObserver> Default for Machine<S, O> {
    fn default() -> Self {
        Self::with_observer(1)
    }
}

/// **KT-405** — hàm dựng **không cần chú thích kiểu** cho observer mặc định.
///
/// Vì sao tách khỏi `impl<O: CoreObserver> Machine<O>`: tham số kiểu **mặc định**
/// (`Machine<O = TestObserver>`) chỉ áp ở **vị trí kiểu**, **không** áp ở **vị trí biểu thức**
/// (`Machine::new()` là lời gọi hàm liên kết ⇒ `O` không suy ra được ⇒ `E0283`). Đây đúng
/// khuôn `HashMap::new()` của std: `new`/`with_harts` chỉ định nghĩa trên **một** kiểu cụ thể
/// (`Machine<StaticStore, TestObserver>`), còn [`Machine::with_observer`] là đường generic.
///
/// Nhờ vậy **0 sửa** mọi nơi gọi `Machine::new()`/`Machine::with_harts(n)`/`Machine::footprint()`
/// ngoài crate (sim parity, `plat-common` test, `core_bridge` của ảnh sản phẩm).
impl Machine<StaticStore, TestObserver> {
    /// Model rỗng **1 hart** (Untyped/CNode do harness thêm).
    pub const fn new() -> Self {
        Self::with_observer(1)
    }

    /// Model rỗng `harts` hart (`1..=MAX_HARTS`) — như sim `Machine::with_harts`.
    ///
    /// # Panics
    /// `harts == 0` hoặc `harts > MAX_HARTS` (lỗi init của harness, như sim).
    pub const fn with_harts(harts: u16) -> Self {
        Self::with_observer(harts)
    }
}

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    /// Model rỗng `harts` hart với observer `O` — đường dựng **generic** (dùng cho
    /// `Machine::<StaticStore, NoopObserver>` của sản phẩm).
    ///
    /// # Panics
    /// `harts == 0` hoặc `harts > MAX_HARTS` (lỗi init của harness, như sim).
    pub const fn with_observer(harts: u16) -> Self {
        assert!(
            harts > 0 && harts as usize <= MAX_HARTS,
            "vifast-core: số hart phải trong 1..=MAX_HARTS"
        );
        Self {
            regions: S::REGIONS_EMPTY,
            n_regions: 0,
            objects: S::OBJECTS_EMPTY,
            n_objects: 0,
            caps: S::CAPS_EMPTY,
            n_caps: 0,
            slots: S::SLOTS_EMPTY,
            harness: S::HARNESS_EMPTY,
            tcbs: S::TCBS_EMPTY,
            n_tcbs: 0,
            tcb_free: S::TCB_FREE_EMPTY,
            n_tcb_free: 0,
            scs: S::SCS_EMPTY,
            n_scs: 0,
            sc_free: S::SC_FREE_EMPTY,
            n_sc_free: 0,
            endpoints: S::ENDPOINTS_EMPTY,
            n_endpoints: 0,
            endpoint_free: S::ENDPOINT_FREE_EMPTY,
            n_endpoint_free: 0,
            replies: S::REPLIES_EMPTY,
            n_replies: 0,
            reply_free: S::REPLY_FREE_EMPTY,
            n_reply_free: 0,
            notifications: S::NOTIFICATIONS_EMPTY,
            n_notifications: 0,
            notification_free: S::NOTIFICATION_FREE_EMPTY,
            n_notification_free: 0,
            ipc_msgs: S::IPC_MSGS_EMPTY,
            n_ipc_msgs: 0,
            ipc_msg_free: S::IPC_MSG_FREE_EMPTY,
            n_ipc_msg_free: 0,
            addr_words: S::ADDR_WORDS_EMPTY,
            n_addr_words: 0,
            wait_nodes: S::WAIT_NODES_EMPTY,
            n_wait_nodes: 0,
            wait_free: S::WAIT_FREE_EMPTY,
            n_wait_free: 0,
            wait_buckets: S::WAIT_BUCKETS_EMPTY,
            wait_buckets_tail: S::WAIT_BUCKETS_TAIL_EMPTY,
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
            obs: O::EMPTY,
            budget_sleeps: 0,
            budget_refills: 0,
            harts: S::HARTS_EMPTY,
            n_harts: harts as usize,
            current_hart: 0,
            finalise_overflow: 0,
            migrate_count: 0,
            preempt_every_step: false,
            now: 0,
            tlb_seq: 0,
            tlb_local_flushes: 0,
            tlb_remote_posts: 0,
            tlb_serviced: 0,
        }
    }

    /// **KT-408** — ghi **một** bản ghi ring trace 32 B cho hart đang chạy (`D2-09`: luôn bật,
    /// không cờ tắt).
    ///
    /// `t_ns` là **tick ảo** [`Self::now`] (không đọc wall clock — `D2-49(h)`); `tcb` là id object
    /// TCB (`0` = không có, dùng [`crate::trace::TCB_NONE`] nếu muốn in `-`).
    pub fn trace_push(&self, kind: crate::trace::TraceKind, tcb: u32, a: u64, b: u64) {
        self.trace_push_on(self.current_hart, kind, tcb, a, b);
    }

    /// **KT-408** — như [`Self::trace_push`] nhưng chỉ rõ hart (dùng khi `dispatch(hart)` khác
    /// `current_hart`).
    pub fn trace_push_on(
        &self,
        hart: u16,
        kind: crate::trace::TraceKind,
        tcb: u32,
        a: u64,
        b: u64,
    ) {
        let h = (hart as usize).min(MAX_HARTS - 1);
        self.harts[h].trace.push(h, self.now, kind, tcb, a, b);
    }

    /// **KT-408c** — đặt **đồng hồ tick ảo** từ nguồn đơn điệu của nền tảng (`rdtime` riscv64).
    ///
    /// Vì sao có: `now` chỉ được khởi tạo `0` và **không nơi nào** tiến ⇒ `t_ns` của **mọi** bản
    /// ghi trace = 0 (`KT-408b` §6.4: ring có thật trong RAM nhưng mất trục thời gian). Nền tảng
    /// gọi hàm này trước mỗi lượt vào core (`core_bridge::with_machine`) với `rdtime` quy đổi ns.
    ///
    /// Giữ `D2-49(h)` (**tick ảo**, không wall clock): giá trị do nền tảng cấp, đơn điệu, không đọc
    /// RTC; ngân sách/preempt vẫn tính trên tick của model, không dùng timer HW để quyết định.
    ///
    /// Bất biến:
    /// 1. **Đơn điệu** — `now` không lùi (`now <= self.now` ⇒ không đổi, trả giá trị cũ). Nguồn
    ///    `rdtime` là monotonic theo đặc tả RISC-V; hàm vẫn tự chặn để một nguồn hỏng không làm
    ///    trace nhảy lùi.
    /// 2. **Biên nạp lại** — khi đồng hồ tiến, mọi `SchedContext` sống được xử lý biên
    ///    (`ScBudget::refill_due`) trước khi ai đó đọc. Không có bước này, một đồng hồ **thật**
    ///    làm `check_budget` đỏ (`refill <= now`) dù không có lỗi nào — `refill` là mốc tuyệt đối
    ///    sinh từ `now` lúc `Configure`.
    ///
    /// Trả tick hiện hành **sau** khi cập nhật.
    pub fn set_now(&mut self, now: u64) -> u64 {
        if now <= self.now {
            return self.now;
        }
        self.now = now;
        for i in 0..self.n_objects {
            if !self.objects[i].alive || self.objects[i].ty != ObjType::SchedContext {
                continue;
            }
            let obj = (i as u32) + 1;
            if let Ok(sc) = self.sc_data_mut(obj) {
                let _ = sc.budget.refill_due(now);
            }
        }
        now
    }

    /// **KT-408c** — tick ảo hiện hành (giá trị mà `trace_push` ghi vào `t_ns`).
    pub fn now_ticks(&self) -> u64 {
        self.now
    }

    /// **KT-408** — số hart đang dùng (`1..=MAX_HARTS`).
    pub fn n_harts(&self) -> usize {
        self.n_harts
    }

    /// **KT-408** — đổi số hart **tại chỗ** (không dựng lại model, không xoá sổ cái).
    ///
    /// Dùng cho `KT-412`/`KT-413` (SMP 4 hart): `BootInfo` thật cho biết số hart, kernel gọi
    /// `set_harts(n)` **sau** khi đã dựng `Machine` — trước KT-408 chỉ có `with_harts(n)` lúc
    /// dựng, buộc phải biết số hart **trước** khi có `BootInfo`.
    ///
    /// # Panics
    /// `harts == 0` hoặc `harts > MAX_HARTS` (lỗi init của harness, như [`Self::with_harts`]).
    pub fn set_harts(&mut self, harts: u16) {
        assert!(
            harts > 0 && harts as usize <= MAX_HARTS,
            "vifast-core: số hart phải trong 1..=MAX_HARTS"
        );
        // **KT-412** — chỉ hart **mới mở** cần trạng thái rỗng. Bản trước reset **mọi** hart
        // `0..n_harts`, nên gọi `set_harts` trên mô hình **đang chạy** (đường `BootInfo` đến muộn,
        // hoặc mở thêm hart giữa vòng đời) sẽ **xoá ready queue + `current` của hart 0** ⇒ mọi TCB
        // `Ready` mồ côi và `check_sched` đỏ. Đo được trên ảnh kiểm `riscv64` (`# smp: … check_sched=0`).
        // Ngữ nghĩa đúng: hart đã dùng giữ nguyên `PerHartState`; chỉ hart vượt `n_harts` cũ bị
        // `reset` (chúng chưa từng chạy, hoặc là tàn dư của lượt thu nhỏ trước đó).
        let old = self.n_harts;
        self.n_harts = harts as usize;
        // **Đột biến `mut-smp-set-harts-reset-all`** — tái tạo đúng lỗi mà KT-412 bắt được:
        // reset **mọi** hart `0..n_harts` ⇒ gọi `set_harts` trên mô hình đang chạy xoá ready
        // queue của hart 0 ⇒ TCB `Ready` mồ côi ⇒ TAP `smp harts bootinfo` ĐỎ (`check_sched=0`).
        #[cfg(feature = "mut-smp-set-harts-reset-all")]
        for h in 0..self.n_harts {
            self.harts[h].reset();
        }
        #[cfg(not(feature = "mut-smp-set-harts-reset-all"))]
        for h in old..self.n_harts {
            self.harts[h].reset();
        }
        if self.current_hart as usize >= self.n_harts {
            self.current_hart = 0;
        }
    }

    /// Khởi tạo **tại chỗ** `Machine` trong `.bss` đã zero (KT-61): chỉ đặt trường **khác 0**
    /// (`n_harts`, `wait_buckets`/`_tail = NODE_NONE`, `PerHartState::reset`).
    pub fn init_zeroed(&mut self, harts: u16) {
        assert!(
            harts > 0 && harts as usize <= MAX_HARTS,
            "vifast-core: số hart phải trong 1..=MAX_HARTS"
        );
        self.n_harts = harts as usize;
        // **KT-64**: arena harness + cửa sổ slot có **cận khác 0** nên zero `.bss` không đủ —
        // đặt lại tại chỗ (cùng lý do `wait_buckets`/`sched`/`mail`).
        self.harness = S::HARNESS_EMPTY;
        self.wait_buckets = S::WAIT_BUCKETS_EMPTY;
        self.wait_buckets_tail = S::WAIT_BUCKETS_TAIL_EMPTY;
        // **KT-405** — `PerHartState` có trường **không zero** (`idle = true`; 8 ô
        // `Option<MailboxMsg>` với `None != 0`) ⇒ reset tại chỗ từng hart (không bản tạm lớn).
        for h in self.harts.iter_mut() {
            h.reset();
        }
        // **KT-408** — `.corespace` là NOLOAD nên loader **không** zero kho ring; zero tường minh
        // một lần lúc boot (ring trace luôn bật, `D2-09`).
        crate::trace::reset_all();
    }
}

mod init;
mod lifecycle;
