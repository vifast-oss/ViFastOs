// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-408** — test host cho Lõi Kép `KernelStore` + `Machine<S=StaticStore, O>` + ring trace 32 B.
//!
//! Ba nhóm ca (đỏ-trước, D2-05):
//!
//! 1. [`kt408_store_phu_100_phan_tram_bang`] — trait `KernelStore` phủ **đủ 100 %** bảng của
//!    `Machine` (23 bảng: `Region`, `Meta`, `Cap`, `SlotMem`, 5 thân ABI, `QueuedMsg`,
//!    `WaitNode`/`AddrWord`/`wait_buckets`, `PerHart`), `StaticStore` dựng được bằng `const`, và
//!    `Machine<StaticStore, O>` giữ **nguyên** cỡ trong đợt (pin harness 184712 → 172424 ở
//!    `KT-417b`) + `Copy` + `Meta == 48 B`.
//! 2. [`kt408_set_harts`] — `Machine::set_harts(n)` đổi số hart **tại chỗ** (không dựng lại model).
//! 3. [`kt408_ring_trace_32_b`] — ring trace nhị phân **32 B/bản ghi** luôn bật, byte khớp
//!    `spec/abi.toml [trace]` (offset `t_ns@0 hart@8 kind@10 tcb@12 a@16 b@24`), và dump thô giải
//!    mã được bằng `v2/tools/vifast-trace.py`.

use crate::machine::{
    Machine, MAX_CAPS, MAX_ENDPOINTS, MAX_HARTS, MAX_NOTIFICATIONS, MAX_OBJECTS,
    MAX_QUEUED_MESSAGES, MAX_REPLIES, MAX_SCHED_CONTEXTS, MAX_TCBS, MAX_UNTYPEDS,
};
use crate::objstate::{MAX_ADDR_WORDS, MAX_WAIT_NODES, WAITADDR_BUCKETS};
use crate::observer::{NoopObserver, TestObserver};
use crate::store::StaticStore;
use crate::trace::{TraceKind, TraceRecord, RING_CAP, TRACE_RECORD_BYTES};

/// **KT-408** — `KernelStore` phủ **đủ 100 %** bảng: mỗi bảng của `Machine` đọc được qua kiểu
/// liên kết của `StaticStore` với **đúng** biên `MAX_*` của `machine.rs` (không nới, không cắt).
#[test]
fn kt408_store_phu_100_phan_tram_bang() {
    // `StaticStore` là ZST dựng được bằng `const` (đường `.bss` của kernel).
    const _: () = {
        let _ = StaticStore::EMPTY;
    };
    fn assert_copy<T: Copy>() {}
    assert_copy::<StaticStore>();
    assert_eq!(
        core::mem::size_of::<StaticStore>(),
        0,
        "StaticStore phải 0 B"
    );
    // 23 bảng — đọc **qua** `Machine<StaticStore, _>` (kiểu liên kết của `S`), đếm bằng `len()`.
    let m = Machine::<StaticStore, TestObserver>::new();
    assert_eq!(m.regions.len(), MAX_UNTYPEDS, "regions");
    assert_eq!(m.objects.len(), MAX_OBJECTS, "objects");
    assert_eq!(m.caps.len(), MAX_CAPS, "caps");
    assert_eq!(m.tcbs.len(), MAX_TCBS, "tcbs");
    assert_eq!(m.tcb_free.len(), MAX_TCBS, "tcb_free");
    assert_eq!(m.scs.len(), MAX_SCHED_CONTEXTS, "scs");
    assert_eq!(m.sc_free.len(), MAX_SCHED_CONTEXTS, "sc_free");
    assert_eq!(m.endpoints.len(), MAX_ENDPOINTS, "endpoints");
    assert_eq!(m.endpoint_free.len(), MAX_ENDPOINTS, "endpoint_free");
    assert_eq!(m.replies.len(), MAX_REPLIES, "replies");
    assert_eq!(m.reply_free.len(), MAX_REPLIES, "reply_free");
    assert_eq!(m.notifications.len(), MAX_NOTIFICATIONS, "notifications");
    assert_eq!(
        m.notification_free.len(),
        MAX_NOTIFICATIONS,
        "notification_free"
    );
    assert_eq!(m.ipc_msgs.len(), MAX_QUEUED_MESSAGES, "ipc_msgs");
    assert_eq!(m.ipc_msg_free.len(), MAX_QUEUED_MESSAGES, "ipc_msg_free");
    assert_eq!(m.addr_words.len(), MAX_ADDR_WORDS, "addr_words");
    assert_eq!(m.wait_nodes.len(), MAX_WAIT_NODES, "wait_nodes");
    assert_eq!(m.wait_free.len(), MAX_WAIT_NODES, "wait_free");
    assert_eq!(m.wait_buckets.len(), WAITADDR_BUCKETS, "wait_buckets");
    assert_eq!(
        m.wait_buckets_tail.len(),
        WAITADDR_BUCKETS,
        "wait_buckets_tail"
    );
    assert_eq!(m.harts.len(), MAX_HARTS, "harts");
    // `SlotMem` + `Region` (arena harness) là hai bảng **đơn** — kiểm qua cỡ.
    assert_eq!(
        core::mem::size_of_val(&m.slots),
        core::mem::size_of::<crate::slotmem::SlotMem>(),
        "slots"
    );
    assert_eq!(
        core::mem::size_of_val(&m.harness),
        core::mem::size_of::<crate::arena::Region>(),
        "harness"
    );
    // `Table` trong suốt: cỡ bảng = cỡ mảng trần (pin `size_of::<Machine>()` không đổi).
    assert_eq!(
        core::mem::size_of_val(&m.objects),
        core::mem::size_of::<[crate::machine::Meta; MAX_OBJECTS]>(),
        "Table<Meta, 256> phải cùng cỡ [Meta; 256]"
    );
}

/// **KT-408** — `Machine<S=StaticStore, O>` khớp pin harness (`172424` từ `KT-417b`), `Copy`,
/// `Meta == 48 B`, `≤ 184.832 B` (trần KT-312), và `MissionMachine` (observer rỗng) nhỏ hơn.
#[test]
fn kt408_machine_giu_co_copy_va_meta_48_b() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<Machine<StaticStore, TestObserver>>();
    assert_copy::<Machine<StaticStore, NoopObserver>>();
    assert_eq!(
        core::mem::size_of::<crate::machine::Meta>(),
        48,
        "Meta phải 48 B"
    );
    let test = core::mem::size_of::<Machine<StaticStore, TestObserver>>();
    let mission = core::mem::size_of::<Machine<StaticStore, NoopObserver>>();
    assert_eq!(
        test, 172_424,
        "size_of::<Machine<StaticStore, TestObserver>>() phải khớp pin harness 172424 (đo {test})"
    );
    assert!(test <= 184_832, "trần KT-312: Machine {test} > 184832");
    assert!(
        mission < test,
        "MissionMachine (NoopObserver) phải nhỏ hơn bản test: {mission} < {test}"
    );
    // Đường dựng **không** chú thích kiểu vẫn chạy (như trước KT-408).
    let m = Machine::new();
    assert_eq!(m.n_harts(), 1);
    let m2 = Machine::with_harts(4);
    assert_eq!(m2.n_harts(), 4);
    let m3 = Machine::<StaticStore, NoopObserver>::with_observer(2);
    assert_eq!(m3.n_harts(), 2);
}

/// **KT-408** — `Machine::set_harts(n)` đổi số hart **tại chỗ** (không dựng lại model, không mất
/// trạng thái object/cap) và chặn biên `1..=MAX_HARTS`.
#[test]
fn kt408_set_harts() {
    let mut m = Machine::new();
    assert_eq!(m.n_harts(), 1);
    let root = m.add_cnode(4);
    m.set_harts(4);
    assert_eq!(m.n_harts(), 4, "set_harts(4) phải đổi n_harts");
    // Trạng thái sổ cái **không** bị dựng lại: CNode vừa thêm vẫn còn.
    assert_eq!(
        m.cnode_slot_count(root),
        Ok(4),
        "set_harts không được xoá sổ cái"
    );
    m.set_harts(MAX_HARTS as u16);
    assert_eq!(m.n_harts(), MAX_HARTS);
    m.set_harts(1);
    assert_eq!(m.n_harts(), 1);
}

/// **KT-408** — ring trace nhị phân **32 B** luôn bật (`D2-09`): bản ghi đúng offset
/// `spec/abi.toml [trace]`, `kind` đúng bảng mã, và dump thô giải mã được bằng
/// `v2/tools/vifast-trace.py` (bằng chứng byte nằm ở journal).
#[test]
fn kt408_ring_trace_32_b() {
    assert_eq!(TRACE_RECORD_BYTES, 32, "bản ghi trace phải 32 B");
    assert_eq!(
        core::mem::size_of::<TraceRecord>(),
        32,
        "size_of::<TraceRecord>() phải 32 B"
    );
    assert_eq!(RING_CAP % MAX_HARTS, 0, "ring chia đều cho MAX_HARTS");
    crate::trace::reset_all();
    let mut m = Machine::new();
    m.set_harts(2);
    // Ghi 3 bản ghi qua **đúng** cửa của kernel (`Machine::trace_push`).
    m.trace_push(TraceKind::Run, 7, 0x11, 0x22);
    m.trace_push(TraceKind::Ipc, 9, 0x33, 0x44);
    m.trace_push(TraceKind::Sched, 0, 0x55, 0x66);
    assert_eq!(crate::trace::len_hart(0), 3, "hart 0 có 3 bản ghi");
    let mut buf = [0u8; 3 * TRACE_RECORD_BYTES];
    let n = crate::trace::dump_hart(0, &mut buf);
    assert_eq!(n, 3 * TRACE_RECORD_BYTES, "3 bản ghi × 32 B");
    // Byte-level: `t_ns@0 u64`, `hart@8 u16`, `kind@10 u16`, `tcb@12 u32`, `a@16 u64`, `b@24 u64`.
    let rec = &buf[..32];
    let t_ns = u64::from_le_bytes(rec[0..8].try_into().unwrap());
    let hart = u16::from_le_bytes(rec[8..10].try_into().unwrap());
    let kind = u16::from_le_bytes(rec[10..12].try_into().unwrap());
    let tcb = u32::from_le_bytes(rec[12..16].try_into().unwrap());
    let a = u64::from_le_bytes(rec[16..24].try_into().unwrap());
    let b = u64::from_le_bytes(rec[24..32].try_into().unwrap());
    assert_eq!(hart, 0, "hart của bản ghi 1");
    assert_eq!(kind, TraceKind::Run as u16, "kind@10 = run (6)");
    assert_eq!(tcb, 7, "tcb@12");
    assert_eq!(a, 0x11, "a@16");
    assert_eq!(b, 0x22, "b@24");
    assert!(
        t_ns <= 1,
        "t_ns là tick ảo (D2-49(h)), không đọc wall clock"
    );
    // Bản ghi 2/3 đúng thứ tự FIFO.
    assert_eq!(
        u16::from_le_bytes(buf[32 + 10..32 + 12].try_into().unwrap()),
        TraceKind::Ipc as u16
    );
    assert_eq!(
        u32::from_le_bytes(buf[32 + 12..32 + 16].try_into().unwrap()),
        9
    );
    assert_eq!(
        u16::from_le_bytes(buf[64 + 10..64 + 12].try_into().unwrap()),
        TraceKind::Sched as u16
    );
    // Ring **theo hart**: hart 1 rỗng (chưa ghi), hart 0 có 3.
    assert_eq!(crate::trace::len_hart(1), 0, "hart 1 chưa ghi");
    // Tràn ring ⇒ giữ **bản ghi mới nhất** (FIFO vòng), không panic, không ghi đè hart khác.
    for i in 0..crate::trace::RING_PER_HART {
        m.trace_push(TraceKind::Store, i as u32, 0, 0);
    }
    assert_eq!(
        crate::trace::len_hart(0),
        crate::trace::RING_PER_HART,
        "ring đầy đúng sức chứa"
    );
    let last = crate::trace::read_record(0, crate::trace::RING_PER_HART - 1).expect("bản ghi cuối");
    assert_eq!(
        last.tcb,
        (crate::trace::RING_PER_HART - 1) as u32,
        "bản ghi cuối là bản ghi mới nhất"
    );
    assert_eq!(crate::trace::len_hart(1), 0, "hart 1 vẫn rỗng");
    crate::trace::reset_all();
}
