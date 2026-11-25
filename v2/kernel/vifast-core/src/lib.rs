// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-core` — core kernel `no_std` của K3b: **tầng 1** (KT-58) Untyped + CNode + MDB +
//! capability, **tầng 2** (KT-59) TCB + transition + scheduler 256 mức + ngân sách MCS + hộp
//! thư FIFO, port **nguyên ngữ nghĩa** từ `v2/sim/vifast-sim/src/kobj.rs` (sim vẫn là oracle;
//! core không thiết kế lại).
//!
//! # Phạm vi KT-58 (D2-82 §12 nợ K3b)
//!
//! - **Có**: `Untyped` (watermark, cỡ/căn lề, Retype trọn bộ object lõi, Revoke trả byte),
//!   `CNode` (slot 48 B theo spec, Copy/Mint/Delete/Revoke, quyền 4 bit, badge),
//!   cây dẫn xuất MDB kiểu seL4 (cha–con, reparent khi xoá, revoke cây con, finalise CNode
//!   lồng nhau), và các bất biến `check_caps`/`check_bytes`.
//! - **Không**: Endpoint/Notification/Reply/IPC/WaitAddr/fault (KT-60), Frame/PageTable/
//!   VSpace (K2 object layer `vifast-plat-common::obj`, KT-52) — `Retype` các loại reserved
//!   trả `NotSupported` như sim (trừ Frame/PageTable mà sim đã mở ở K2, xem
//!   `docs/k3b-core.md` §khác biệt), kết nối syscall/`init` (KT-61).
//! - **KT-59**: `TcbState` 11 trạng thái + bảng `ALLOWED` 11×11, `Tcb.Configure/SetFaultEp/
//!   SetSpace/SetSchedParams/SetPriority/SetAffinity/SetHome/Suspend/Resume/ReadState`, một
//!   cửa `transition_tcb`; scheduler 256 mức + bitmap, ready queue tĩnh FIFO (round-robin khi
//!   preempt/yield), `dispatch`/`block_current`/`wake`/`yield_current`, preempt ép; ngân sách
//!   MCS `SchedControl.Configure`/`tick`/refill + `pick` chỉ TCB có budget; hộp thư theo hart
//!   FIFO cố định 8 message + backpressure `Busy` + `post_mail`/`process_mail`/`send_mail`.
//!   Donation KT-21 là phần của `Call`/`Reply` (KT-60): giữ trường + công thức
//!   `donated_priority`, chưa có đường tạo donation.
//!
//! # Vì sao `no_std` + tĩnh
//!
//! Core chạy được **cả host test lẫn bare-metal riscv64**: `#![no_std]`,
//! `#![forbid(unsafe_code)]` (D2-17), **không** `alloc`/`Vec`/`String`. `Vec<Meta>`/`Vec<Cap>`
//! của sim thành bảng tĩnh `.bss` (`MAX_OBJECTS`, `MAX_CAPS`, `MAX_SLOT_WORDS`); byte
//! Untyped do [`arena::Region`] kế toán **và** là nguồn duy nhất của byte slot CNode (KT-64: slot
//! lấy từ arena tại `offset + cnode_base`, không còn pool riêng — [`slotmem`]). Mọi truy vấn trả
//! iterator/`Copy` thay `Vec`: `cap_table`, `cap_children`.
//!
//! # Hợp đồng ABI (D2-11)
//!
//! `spec::*` tái xuất hằng sinh từ `spec/abi.toml` (`vifast-abi`): 48 B slot, `cnode_base`
//! 64 B, TCB 512 B, page 4096 B, 16 syscall, 19 mã lỗi. Bốn bit quyền là hằng thiết kế
//! §4.3/QD §3.12 (spec không có bảng quyền).
//!
//! # Không `unsafe`
//!
//! Không con trỏ thô, không `transmute`, không trait `PhysMem`: mọi thứ là chỉ số + mảng tĩnh.
//! Kernel QEMU dùng cùng mã qua `vifast-core` (KT-61). KT-64 nối `TlbFlush` thật qua hook
//! [`TlbEnv`] (arch cài `sfence.vma` — `unsafe` chỉ ở `kernel/vifast-plat-*`, D2-17).

#![no_std]
#![forbid(unsafe_code)]

pub mod arena;
mod captable;
mod check;
mod check_ipc;
mod core_capxfer;
mod core_finalise;
mod core_foreign;
mod core_ipc;
mod core_sched;
mod core_sync;
mod core_tcb;
mod error;
mod footprint;
pub mod invoke;
pub mod ipc;
mod machine;
mod machine_slot;
pub mod mailbox;
pub mod mcs;
mod mdb;
mod objmeta;
mod objstate;
pub mod observer;
mod perhart;
pub mod sched;
mod slotmem;
pub mod spec;
mod state_write;
pub mod store;
pub mod syscall;
pub mod tcb;
mod tcbdata;
mod tlb;
pub mod trace;

// KT-68 — test host cho `CNode.*` invocation + thuật toán capalloc (chỉ khi `cargo test`).
#[cfg(test)]
mod capalloc_tests;
// KT-74 — test host cho `Tcb.SetTls` (method 15 của `[object].Tcb`), chỉ khi `cargo test`.
#[cfg(test)]
mod tls_tests;
// KT-290 — test host cho `Tcb.Configure` (method 0 của `[object].Tcb`) qua invocation, chỉ khi
// `cargo test`.
#[cfg(test)]
mod configure_tests;
// KT-314 — test host cho `Tcb.Resume` (method 12 của `[object].Tcb`) qua invocation, chỉ khi
// `cargo test`.
#[cfg(test)]
mod resume_tests;
// KT-319 — test host cho `Tcb.Suspend` (method 3 của `[object].Tcb`) qua invocation, chỉ khi
// `cargo test`.
#[cfg(test)]
mod suspend_tests;
// **KT-342b** — test host cho **bảo toàn message `NBSend`** (message vào hàng khi không có
// waiter, và không bị rút lại khi TCB gửi `Exit`), chỉ khi `cargo test`.
#[cfg(test)]
mod nb_queue_tests;
// **KT-345b** — test host cho **vòng đời TCB spawn→destroy** của `loop` (Revoke trả byte về 0),
// chỉ khi `cargo test`.
#[cfg(test)]
mod kt345b_tests;
// **KT-408** — test host cho Lõi Kép `KernelStore` + `Machine<S=StaticStore, O>` + ring trace 32 B,
// chỉ khi `cargo test`.
#[cfg(test)]
mod kt408_tests;
// **KT-408** — khoá dùng chung cho các ca đọc bộ đếm toàn cục `invoke::stats()`.
#[cfg(test)]
mod test_lock;

pub use arena::Region;
pub use captable::{CapChildren, CapTable};
pub use check_ipc::TcbIds;
pub use core_ipc::NotificationValue;
pub use core_ipc::{EndpointInfo, MsgIds, QueuedIter, QueuedView, SendItems, TcbIpcArea};
pub use core_sched::MailItems;
pub use core_sync::Waiters;
pub use error::{CapId, CapInfo, CheckFailure, KError, ObjId, ObjType, SlotRef};
pub use footprint::{Footprint, FOOTPRINT_PARTS};
pub use ipc::{
    mr_buffer_offset, transfer, IpcArea, IpcBufferBinding, Message, MessageInfo, IPC_BUFFER_ALIGN,
    IPC_BUFFER_BYTES, IPC_BUFFER_MR_END, IPC_BUFFER_MR_OFFSET, IPC_BUFFER_MR_WORDS,
    MAX_CAPS as IPC_MAX_CAPS, MAX_WORDS as IPC_MAX_WORDS, WORDS_IN_IPC_BUFFER, WORDS_IN_REGISTERS,
};
pub use machine::{
    FinaliseEntry, Machine, MAX_CAPS, MAX_ENDPOINTS, MAX_FINALISE, MAX_HARTS, MAX_NOTIFICATIONS,
    MAX_OBJECTS, MAX_QUEUED_MESSAGES, MAX_REPLIES, MAX_SCHED_CONTEXTS, MAX_SLOT_WORDS, MAX_TCBS,
    MAX_UNTYPEDS,
};
pub use mailbox::{FlushMsg, HartMailbox, Mailbox, MailboxMsg, MAILBOX_CAP, MAX_MAIL_LOG};
pub use mcs::{donated_priority, ScBudget, ScInfo};
pub use objstate::{
    waitaddr_bucket, ENDPOINT_QUEUE_CAP, MAX_ADDR_WORDS, MAX_WAIT_NODES, WAITADDR_BUCKETS,
};
pub use observer::{CoreObserver, NoopObserver, TestObserver, PAIR_SLOTS};
pub use perhart::PerHartState;
pub use sched::{HartSched, Published, BITMAP_WORDS, MAX_PRIORITY};
pub use slotmem::{ARENA_SLOT_WORDS, HARNESS_SLOT_WORDS};
pub use store::{HarnessArena, KernelStore, SlotWindow, StaticStore, Table};
pub use trace::{TraceKind, TraceRecord, TraceRing, RING_CAP, RING_PER_HART, TRACE_RECORD_BYTES};
// **KT-408** — kiểu **bảng** mà [`KernelStore`] phủ (trước đây `pub(crate)`): trait công khai cần
// gọi được tên các kiểu này (KT-410 cài `UntypedStore` với cùng bộ kiểu phần tử).
pub use objstate::{AddrWord, EndpointData, NotificationData, QueuedMsg, ReplyData, WaitNode};
pub use slotmem::SlotMem;
pub use spec::{
    ARENA_ALIGN, CNODE_BASE_BYTES, DEFAULT_ALIGN_BYTES, FLAG_GUEST, PAGE_BYTES, RIGHTS_ALL,
    RIGHT_GRANT, RIGHT_GRANT_REPLY, RIGHT_READ, RIGHT_WRITE, SLOT_BYTES, UNTYPED_MIN_BYTES,
};
pub use state_write::NonReadyTarget;
pub use tcb::{
    allowed, allowed_count, coverage, coverage_design, forbidden_count, reserved_count,
    BlockedOnKind, FaultAccess, FaultKind, SleepReason, SuspendMode, TcbInfo, TcbSnapshot,
    TcbState, VmFaultReason, ALLOWED, TCB_STATE_COUNT,
};
pub use tcbdata::{ScData, TcbData};
pub use tlb::{TlbEnv, TlbFlushReport, TlbScope, TlbShootdownReport};
