// SPDX-License-Identifier: MIT OR Apache-2.0
// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen từ spec/abi.toml — ĐỪNG SỬA TAY (gen-check sẽ bắt).
#![allow(clippy::all)]

#[rustfmt::skip]
pub const ABI_VERSION: u32 = 1;
#[rustfmt::skip]
pub const SYSCALL_COUNT: usize = 16;
#[rustfmt::skip]
pub const SYSCALLS: [(&str, u16); SYSCALL_COUNT] = [
    ("Call", 0),
    ("Send", 1),
    ("NBSend", 2),
    ("Recv", 3),
    ("NBRecv", 4),
    ("ReplyRecv", 5),
    ("NBSendRecv", 6),
    ("NBSendWait", 7),
    ("Yield", 8),
    ("WaitAddr", 9),
    ("WakeAddr", 10),
    ("ClockRead", 11),
    ("DebugPutStr", 12),
    ("DebugHalt", 13),
    ("Exit", 14),
    ("DebugGetInfo", 15),
];
#[rustfmt::skip]
pub const ERROR_COUNT: usize = 19;
#[rustfmt::skip]
pub const ERRORS: [(&str, u16); ERROR_COUNT] = [
    ("InvalidCapability", 1),
    ("WrongObjectType", 2),
    ("InsufficientRights", 3),
    ("WxViolation", 4),
    ("InvalidArgument", 5),
    ("Alignment", 6),
    ("SlotOccupied", 7),
    ("SlotEmpty", 8),
    ("NotEnoughMemory", 9),
    ("AlreadyMapped", 10),
    ("NotMapped", 11),
    ("MissingPageTable", 12),
    ("HasChildren", 13),
    ("Busy", 14),
    ("Cancelled", 15),
    ("Timeout", 16),
    ("IllegalOperation", 17),
    ("NotSupported", 18),
    ("RangeError", 19),
];
#[rustfmt::skip]
pub const CORE_OBJECTS: [(&str, u16); 8] = [
    ("Untyped", 1),
    ("CNode", 2),
    ("Tcb", 3),
    ("SchedContext", 4),
    ("Endpoint", 5),
    ("Notification", 6),
    ("Reply", 7),
    ("SchedControl", 8),
];
#[rustfmt::skip]
pub const RESERVED_OBJECTS: [(&str, u16); 8] = [
    ("VSpace", 9),
    ("PageTable", 10),
    ("Frame", 11),
    ("IrqControl", 12),
    ("IrqHandler", 13),
    ("IOSpace", 14),
    ("VCpu", 15),
    ("PowerControl", 16),
];
#[rustfmt::skip]
/// (method, params, returns) — từ `[method_signature]` (stable-draft).
#[rustfmt::skip]
pub const METHOD_SIGNATURES: [(&str, &[&str], &[&str]); 13] = [
    ("CNode.Copy", &["cnode", "src", "dst", "rights?"], &[]),
    ("CNode.Delete", &["cnode", "slot"], &[]),
    ("CNode.Mint", &["cnode", "src", "dst", "rights?", "badge?"], &[]),
    ("CNode.Move", &["cnode", "src", "dst"], &[]),
    ("CNode.Revoke", &["cnode", "slot?"], &[]),
    ("SchedContext.Bind", &["sched", "target"], &[]),
    ("SchedControl.Configure", &["sched_control", "sched", "budget?", "period?", "refills?"], &[]),
    ("Tcb.Configure", &["tcb", "cspace?", "vspace?", "entry?", "ipc_buffer?", "priority?"], &[]),
    ("Tcb.ReadState", &["tcb"], &["state", "suspended", "blocked_on_kind", "exit_code"]),
    ("Tcb.Resume", &["tcb"], &[]),
    ("Tcb.SetFaultEp", &["tcb", "cap"], &[]),
    ("Tcb.Suspend", &["tcb", "mode"], &[]),
    ("Untyped.Retype", &["cap", "type", "size_bits", "flags"], &[]),
];
#[rustfmt::skip]
pub const TCB_STATES: [&str; 11] = ["Inactive", "Ready", "Running", "BlockedOnSend", "BlockedOnRecv", "BlockedOnReply", "BlockedOnNotification", "BlockedOnAddress", "Sleeping", "Stopping", "Dead"];
