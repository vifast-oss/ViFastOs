// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-62 (K3.5 tầng 1) — **16 syscall `0..15` đi vào `Machine` thật**.
//!
//! Trước KT-62, ảnh test U-mode gọi `vifast-plat-common::user::dispatch` — một *shim* KT-53
//! biết đúng 3 syscall (`DebugPutStr` 12, `DebugHalt` 13, `Exit` 14) và trả `NotSupported` cho
//! phần còn lại. KT-62 xoá đường tạm đó: mọi số hiệu `0..15` đi qua **một** bảng dispatch ở
//! đây rồi vào API công khai của [`Machine`] (TCB/scheduler/IPC/Notification/WaitAddr/fault) —
//! không nhánh giả, không `todo!`.
//!
//! # Bảng 16 syscall → core (nguồn: `spec/abi.toml` `[syscall]`, D2-29/D2-79)
//!
//! | nr | tên | đường vào core |
//! |---|---|---|
//! | 0 | `Call` | [`Machine::call_msg`] (Endpoint; cap khác ⇒ `NotSupported`, D2-53 Q11) |
//! | 1 | `Send` | [`Machine::send`] (Endpoint/Reply/Notification) |
//! | 2 | `NBSend` | [`Machine::nb_send`] |
//! | 3 | `Recv` | [`Machine::recv`] |
//! | 4 | `NBRecv` | [`Machine::nb_recv`] (`Busy` khi không có message) |
//! | 5 | `ReplyRecv` | [`Machine::reply_recv`] |
//! | 6 | `NBSendRecv` | [`Machine::nb_send_recv`] (D2-53 Q15a) |
//! | 7 | `NBSendWait` | [`Machine::nb_send_wait`] (D2-53 Q15a) |
//! | 8 | `Yield` | [`Machine::yield_current`] + ngân sách còn lại |
//! | 9 | `WaitAddr` | [`Machine::wait_addr`] qua hook [`SyscallEnv::resolve_addr`] |
//! | 10 | `WakeAddr` | [`Machine::wake_addr`] qua cùng hook |
//! | 11 | `ClockRead` | hook [`SyscallEnv::now_ns`] (đồng hồ đơn điệu của nền tảng) |
//! | 12 | `DebugPutStr` | hook [`SyscallEnv::put_str`] (đọc bộ nhớ user bằng `SUM`) |
//! | 13 | `DebugHalt` | trả [`SyscallOutcome::Halt`] — arch quyết định dừng máy |
//! | 14 | `Exit` | [`Machine::exit`] (fault_ep ⇒ `BlockedOnSend{call:false, fault:Exit}`) |
//! | 15 | `DebugGetInfo` | hằng sinh từ spec (`vifast_abi`) + `hart_count` + `features` |
//!
//! **Ngoài dải `0..15` (kể cả EID SBI) ⇒ `RangeError` 19** — D2-79, giữ nguyên hành vi đã chốt
//! từ KT-53; đây là chỗ **duy nhất** quyết định (arch/plat không kiểm lại).
//!
//! # Biên giới (D2-83, một chiều)
//!
//! Core **không** đọc/ghi bộ nhớ user, không biết VSpace/PTE: chỗ nào cần (12 `DebugPutStr`,
//! 9/10 futex theo **địa chỉ user**) thì đi qua trait [`SyscallEnv`] do arch cài. `resolve_addr`
//! trả **nhãn PPN + offset trong trang** — đúng tinh thần D2-83 (core chỉ giữ *khoá* WaitAddr,
//! không sở hữu Frame/PTE).
//!
//! # KT-282 — MR4..MR63 qua **IPC buffer** (nợ K3.5 đã trả)
//!
//! Trước KT-282 `build_message` chỉ nhận 4 MR thanh ghi ⇒ `length > 4` là `InvalidArgument`
//! (nợ K3.5 ghi ở đầu tệp). Từ KT-282:
//!
//! - `Tcb.Configure` bind **frame 4 KiB** + VA ([`Machine::tcb_configure_with_ipc_buffer`],
//!   tham số `ipc_buffer?` của `spec/abi.toml`);
//! - `length > 4` ⇒ MR4..MR63 đọc từ **IPC buffer** qua hook [`SyscallEnv::ipc_buffer_words`]
//!   (arch đọc bộ nhớ user — D2-83), offset [`crate::ipc::IPC_BUFFER_MR_OFFSET`] = 80 B;
//! - `length > 64` ⇒ `InvalidArgument` (5) từ [`MessageInfo::unpack`] (`max_words = 64`) ⇒
//!   **không tràn** vùng 60 word; `length > 4` mà **chưa bind** buffer ⇒ `InvalidArgument` (5).
//!
//! # Chưa làm (nợ K3.5, có chủ)
//!
//! - **Invocation** ngoài danh sách trắng `Untyped.Retype`/`Frame.Map`/`Frame.Unmap` trả
//!   `NotSupported` (D2-53 Q11) — xem [`crate::invoke`].
//!
//! # KT-67b — chuyển cap qua U-mode (`MessageInfo.caps > 0`)
//!
//! **KT-63** đã có cơ chế kernel (`resolve_caps`/`prepare_caps`/`deliver_caps`); phần thiếu là
//! phía U-mode: header IPC buffer (`spec/abi.toml` dòng 58) + [`build_message`] đọc từ đó. Từ
//! KT-67b [`build_message`] **lấy receive window từ IPC buffer** của TCB gọi (qua hook
//! [`SyscallEnv::ipc_header`], arch đọc user memory — D2-83) thay vì window do `Machine` đặt
//! ngoài băng, và giải mã `MessageInfo.caps` + CPTR nguồn trong header thành `CapId` của CSpace
//! caller. Không đổi `MessageInfo` 35 bit, không thêm syscall.
//!
//! `#![forbid(unsafe_code)]` của crate áp dụng nguyên vẹn: tệp này chỉ gọi API an toàn.

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::ipc::{
    CapSnapshot, IpcExtraCaps, IpcHeader, Message, MessageInfo, MAX_CAPS, MAX_EXTRA_CAPS,
    MAX_WORDS, WORDS_IN_IPC_BUFFER, WORDS_IN_REGISTERS,
};
use crate::machine::Machine;
use crate::spec::RIGHT_GRANT;
use crate::spec::SYSCALL_COUNT;
use crate::tcb::TcbState;

// ---------------------------------------------------------------------------------------------
// Hằng ABI — sinh từ `spec/abi.toml` (D2-11), không chép tay
// ---------------------------------------------------------------------------------------------

// D2-88(c): tên + số hiệu syscall **sinh từ** `spec/abi.toml` `[syscall]` (`vifast-gen`), không
// chép tay. Danh sách đủ 16 hằng ở [`crate::spec`]; ở đây tái xuất những số hiệu có nhánh riêng.
pub use crate::spec::{
    SYS_CLOCK_READ, SYS_DEBUG_GET_INFO, SYS_DEBUG_HALT, SYS_DEBUG_PUT_STR, SYS_EXIT, SYS_YIELD,
};

/// Số hiệu syscall lớn nhất của ABI (`[syscall]` 0..15; dải 16..31 reserved).
pub const SYSCALL_MAX: u64 = SYSCALL_COUNT as u64 - 1;

/// `abi_version` trả bởi `DebugGetInfo` — sinh từ spec.
pub const ABI_VERSION: u64 = crate::spec::ABI_VERSION as u64;
/// `min_compat` — sinh từ spec (`[abi].min_compat`).
pub const MIN_COMPAT: u64 = crate::spec::MIN_COMPAT as u64;

/// Bit `features` của `DebugGetInfo`: `DebugPutStr` (12) có mặt (spec không định nghĩa bitmap —
/// KT-62 dùng bit 0/1, ghi ở `docs/k3b-core.md` §11).
pub const FEATURE_DEBUG_PUT_STR: u64 = 1 << 0;
/// Bit `features`: `DebugHalt` (13) có mặt.
pub const FEATURE_DEBUG_HALT: u64 = 1 << 1;
/// Bit `features`: `WaitAddr`/`WakeAddr` dùng hook địa chỉ thật.
pub const FEATURE_WAIT_ADDR: u64 = 1 << 2;
/// Bit `features`: `Yield` có ngân sách MCS thật.
pub const FEATURE_MCS_YIELD: u64 = 1 << 3;

/// Trần số byte một lần `DebugPutStr` (chặn user giữ kernel trong vòng đọc vô hạn; `[TK]`).
pub const MAX_DEBUG_PUT: u64 = 4096;

/// `a1` của `Recv` / `NBSendRecv` khi **không** có cap Reply.
pub const REPLY_NONE: u64 = u64::MAX;

// ---------------------------------------------------------------------------------------------
// Kết quả dispatch
// ---------------------------------------------------------------------------------------------

/// Kết quả trả về user qua `a0/a1/a2/a3..a6` — đúng kênh `spec/abi.toml`
/// (`return_error = a0`, `return_value = a1`, `message_info = a2`, `mr = a3..a6`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyscallReturn {
    /// `a0` — 0 = OK, 1..19 = mã lỗi ABI ([`KError::id`]).
    pub error: u64,
    /// `a1`.
    pub value: u64,
    /// `a2` — `MessageInfo` **đã đóng gói** (Recv/NBRecv/ReplyRecv/NBSendRecv/NBSendWait) hoặc 0.
    pub info: u64,
    /// `a3..a6` — MR0..MR3 (hoặc `min_compat`/`features`/`num_harts` của `DebugGetInfo` ở
    /// `mr[0..3]`).
    pub mr: [u64; 4],
}

impl SyscallReturn {
    /// Trả lỗi ABI `error` (`a1..a6` = 0).
    pub const fn err(error: u64) -> Self {
        Self {
            error,
            value: 0,
            info: 0,
            mr: [0; 4],
        }
    }

    /// Trả `(error, value)`.
    pub const fn new(error: u64, value: u64) -> Self {
        Self {
            error,
            value,
            info: 0,
            mr: [0; 4],
        }
    }
}

/// Việc arch phải làm sau khi core xử lý xong một syscall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallOutcome {
    /// Ghi `a0/a1/a2/a3..a6` rồi `sepc += 4` — TCB **vẫn** `Running` trên hart này.
    Return(SyscallReturn),
    /// `Exit(code)` — TCB đã vào `BlockedOnSend{fault_ep}` hoặc `Inactive`; **không** quay lại
    /// U-mode (QD §1 Q-A/D2-25).
    Exit {
        /// Mã thoát.
        code: u64,
    },
    /// `DebugHalt(code)` — arch log rồi dừng máy (chỉ ảnh test dùng).
    Halt {
        /// Mã halt.
        code: u64,
    },
    /// TCB **không còn** `Running` (chặn/ngủ/sẵn sàng) — arch phải lập lịch; KT-62 chưa có
    /// scheduler U-mode nên ảnh test coi đây là lỗi có chẩn đoán, TAP kernel đọc trạng thái.
    Switched {
        /// Trạng thái TCB sau syscall (đã qua `dispatch` nếu là `Ready`).
        state: TcbState,
    },
}

/// Hook **arch** cho những gì core không được/không thể làm (D2-83, D2-17).
///
/// `no_std`, không `unsafe`: arch cài bằng asm/`SUM`/CSR của chính nó.
pub trait SyscallEnv {
    /// Đọc `len` byte user tại `ptr` và in ra console; trả số byte đã in hoặc **mã lỗi ABI**
    /// (1..19).
    fn put_str(&mut self, ptr: u64, len: u64) -> Result<u64, u64>;
    /// Dịch địa chỉ user `va` thành **nhãn** `(ppn, offset_trong_trang)` cho WaitAddr — D2-83:
    /// core chỉ giữ khoá, không sở hữu Frame/PTE. Lỗi ⇒ mã ABI (vd 11 `NotMapped`).
    fn resolve_addr(&mut self, va: u64) -> Result<(u32, u32), u64>;
    /// Đồng hồ đơn điệu (ns) — `ClockRead` 11.
    fn now_ns(&mut self) -> u64;
    /// Bitmap `features` trả bởi `DebugGetInfo` (bit 0/1/2/3 — xem hằng `FEATURE_*`).
    fn features(&mut self) -> u64;
    /// Số hart **nền tảng đang online** (≤ 8) — D2-88(a): `DebugGetInfo.num_harts` là số hart
    /// phần cứng (arch đọc SBI HSM), **không** phải số hart mà core model đang chạy.
    fn num_harts(&mut self) -> u64;

    /// **Invocation** (KT-67a, D2-97) trên object không phải Endpoint — arch cài bằng object
    /// layer (D2-83: core không sở hữu `Frame`/`PageTable`/`VSpace`). Trả **mã lỗi ABI** khi
    /// hỏng. Mặc định `NotSupported` (18) — đúng D2-53 Q11 cho nền tảng chưa cài invocation.
    fn invoke(
        &mut self,
        _op: crate::invoke::InvokeOp,
    ) -> Result<crate::invoke::InvokeOutcome, u64> {
        Err(KError::NotSupported.id() as u64)
    }

    /// Rollback object vừa cấp bên object layer khi core **không** dựng nổi object đại diện
    /// (bảng đầy) — giữ nguyên tử của `Untyped.Retype`. Mặc định không làm gì.
    fn release_object(&mut self, _key: u64) {}

    /// **KT-67b** — header **IPC buffer** của TCB gọi (D2-98): magic + `MessageInfo` bản ghi +
    /// receive window 4 CPTR + 4 CPTR nguồn cap. Arch đọc **bộ nhớ user** (D2-83: core không
    /// đọc/ghi bộ nhớ user) sau khi kiểm trang đã map; `None` = chưa dùng header (magic 0 —
    /// mọi ảnh cũ giữ nguyên hành vi) hoặc nền tảng không có IPC buffer.
    fn ipc_header(&mut self, _tcb: ObjId) -> Option<IpcHeader> {
        None
    }

    /// **KT-282** — 60 word **MR4..MR63** trong **IPC buffer** của TCB gọi (D2-83: core không
    /// đọc bộ nhớ user). Arch đọc trang 4 KiB tại **VA đã bind** (`Tcb.Configure` tham số
    /// `ipc_buffer?`, xem [`Machine::tcb_ipc_buffer_binding`]) ở offset
    /// [`crate::ipc::IPC_BUFFER_MR_OFFSET`] (80 B, ngay sau header 10 word).
    ///
    /// `va` do **core** truyền vào (lấy từ [`Machine::tcb_ipc_buffer_binding`]) — arch **không**
    /// gọi lại `with_machine` (sẽ tự khoá lồng trong syscall). `None` = trang chưa map / nền tảng
    /// không có IPC buffer ⇒ message có `length > 4` bị `InvalidArgument` (5) — **fail-closed**,
    /// không đoán.
    fn ipc_buffer_words(&mut self, _tcb: ObjId, _va: u64) -> Option<[u64; WORDS_IN_IPC_BUFFER]> {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------------------------

/// `slot` của con trỏ cap user trong **CSpace của TCB**; TCB chưa cấu hình cspace hoặc cap index
/// quá lớn ⇒ `InvalidCapability` (1).
fn cap_slot(m: &Machine, tcb: ObjId, cptr: u64) -> Result<SlotRef, KError> {
    let cspace = m
        .tcb_info_obj(tcb)
        .map_err(|_| KError::InvalidCapability)?
        .cspace
        .ok_or(KError::InvalidCapability)?;
    let slot = usize::try_from(cptr).map_err(|_| KError::InvalidCapability)?;
    // Chỉ số cap ngoài bảng slot của CSpace = **cap không hợp lệ** (1), không phải `RangeError`:
    // 19 dành cho **số hiệu syscall** ngoài dải (D2-79). `cap_at` của core trả `RangeError` cho
    // chỉ số ngoài bảng nên chặn ở đây để ngữ nghĩa ABI không lẫn (ghi docs §11).
    if slot >= m.cnode_slot_count(cspace)? {
        return Err(KError::InvalidCapability);
    }
    Ok(SlotRef::new(cspace, slot))
}

/// Loại object của cap tại `slot` — invocation cần biết trước khi quyết định đường IPC hay
/// invocation (syscall 0). Slot rỗng/cap chết ⇒ `InvalidCapability` (1) như `cap_slot`.
fn cap_type_at(m: &Machine, slot: SlotRef) -> Result<ObjType, KError> {
    let cap = m.cap_at(slot.cnode, slot.slot).map_err(|e| match e {
        KError::SlotEmpty => KError::InvalidCapability,
        other => other,
    })?;
    let info = m.cap_info(cap)?;
    if !info.alive {
        return Err(KError::InvalidCapability);
    }
    Ok(info.ty)
}

/// Đọc header IPC buffer của TCB gọi và **áp** receive window của nó vào `Machine` — KT-67b.
///
/// Đây là chỗ duy nhất window vào core: trước KT-67b window do kernel đặt ngoài băng
/// (`Machine::tcb_set_recv_window` gọi trực tiếp từ rig); từ KT-67b nó **lấy từ header IPC
/// buffer** (spec `abi.toml` dòng 58) qua hook arch. Header có magic ⇒ cả 4 CPTR phải là ô hợp
/// lệ của CSpace caller (sai ⇒ lỗi, không im lặng).
fn apply_ipc_header(
    m: &mut Machine,
    tcb: ObjId,
    env: &mut impl SyscallEnv,
) -> Result<Option<IpcHeader>, KError> {
    let Some(header) = env.ipc_header(tcb) else {
        return Ok(None);
    };
    let mut window = [SlotRef::new(0, 0); MAX_CAPS];
    for (index, cptr) in header.recv_window.iter().enumerate() {
        window[index] = cap_slot(m, tcb, *cptr)?;
    }
    m.tcb_set_recv_window_obj(tcb, window)?;
    // **KT-309** — slot đích của extra cap (receiver khai trong **cùng** header).
    apply_extra_caps(m, tcb, Some(header));
    Ok(Some(header))
}

/// **KT-309** — ghi slot đích (`dst`) của **extra cap** vào TCB từ header IPC buffer — dùng cho
/// **cả hai** chiều: **gửi** (`build_with_header` đọc `n` + CPTR nguồn) và **nhận**
/// (`Recv`/`NBRecv`/`ReplyRecv`/… chỉ cần `dst`; receiver khai **trước** khi nhận, kernel đọc lúc
/// receiver đang chạy — D2-83: không đọc được bộ nhớ của TCB khác).
///
/// `dst == 0` ⇒ `None` = "slot trống đầu tiên" (quét từ ô 1).
fn apply_extra_caps(m: &mut Machine, tcb: ObjId, header: Option<IpcHeader>) {
    let dst = match header.map(|h| h.extra.dst) {
        Some(dst) if dst != 0 && dst <= u16::MAX as u64 => Some(dst as u16),
        _ => None,
    };
    let _ = m.tcb_set_extra_caps_dst(tcb, dst);
}

/// Dựng `Message` từ `MessageInfo` thô + 4 MR thanh ghi + (khi `length > 4`) 60 word
/// **MR4..MR63** trong IPC buffer (+ header IPC buffer nếu có).
///
/// **KT-282**: `length > 4` ⇒ MR4..MR63 lấy từ `buffer` (arch đọc trang user tại VA đã bind);
/// **không** có buffer ⇒ `InvalidArgument` (5) — fail-closed, không đoán. `length > 64` đã bị
/// [`MessageInfo::unpack`] chặn (`max_words = 64`) nên **không tràn** vùng 60 word.
/// **KT-67b**: `info.caps() > 0` ⇒ giải mã `caps` CPTR nguồn từ header (không có header ⇒
/// `InvalidArgument`), cap nguồn phải sống trong CSpace caller (rỗng/chết ⇒ `InvalidCapability`).
/// Cap **không** được kiểm quyền `RIGHT_GRANT` ở đây — `plan_cap_transfer` xử lý **mềm** khi giao
/// (D2-53 Q6).
fn build_message(
    m: &Machine,
    tcb: ObjId,
    raw_info: u64,
    mrs: &[u64; WORDS_IN_REGISTERS],
    buffer: Option<&[u64; WORDS_IN_IPC_BUFFER]>,
    header: Option<IpcHeader>,
    extra: Option<IpcExtraCaps>,
) -> Result<Message, KError> {
    let info = MessageInfo::unpack(raw_info)?;
    let len = info.length() as usize;
    // MR0..MR3 ở thanh ghi; MR4..MR63 ở IPC buffer (spec `words_in_ipc_buffer = 60`).
    let mut words = [0u64; MAX_WORDS];
    let in_regs = core::cmp::min(len, WORDS_IN_REGISTERS);
    words[..in_regs].copy_from_slice(&mrs[..in_regs]);
    if len > WORDS_IN_REGISTERS {
        let Some(buffer) = buffer else {
            return Err(KError::InvalidArgument);
        };
        let extra = len - WORDS_IN_REGISTERS;
        words[WORDS_IN_REGISTERS..len].copy_from_slice(&buffer[..extra]);
    }
    let n_caps = usize::from(info.caps());
    let mut msg = if n_caps == 0 {
        Message::new(info, &words[..len])?
    } else {
        let Some(header) = header else {
            return Err(KError::InvalidArgument);
        };
        let mut caps = [0u32; MAX_CAPS];
        for (index, cptr) in header.send_caps.iter().enumerate().take(n_caps) {
            let slot = cap_slot(m, tcb, *cptr)?;
            let cap = m
                .cap_at(slot.cnode, slot.slot)
                .map_err(|error| match error {
                    KError::SlotEmpty => KError::InvalidCapability,
                    other => other,
                })?;
            if !m.cap_info(cap)?.alive {
                return Err(KError::InvalidCapability);
            }
            caps[index] = cap;
        }
        Message::with_caps(info, &words[..len], &caps[..n_caps], 0)?
    };
    // **KT-309** — extra cap đi qua **vùng dự phòng** của IPC buffer (offset 560): `n` CPTR
    // nguồn trong CSpace **caller**. Kiểm **cứng** ngay ở đường gửi (caller chủ động khai `n`):
    // `n > 4` ⇒ `InvalidArgument` (5); cap rỗng/chết ⇒ `InvalidCapability` (1); thiếu
    // `RIGHT_GRANT` ⇒ `InsufficientRights` (3). Ảnh chụp chụp **tại đây** (caller đang chạy) —
    // extra cap **không** đi qua `resolve_caps`.
    let n_extra = extra.map(|e| e.count).unwrap_or(0);
    if n_extra > MAX_EXTRA_CAPS as u64 {
        return Err(KError::InvalidArgument);
    }
    if n_extra > 0 {
        let Some(extra) = extra else {
            return Err(KError::InvalidArgument);
        };
        let mut snaps = [CapSnapshot::EMPTY; MAX_EXTRA_CAPS];
        for (index, snap) in snaps.iter_mut().enumerate().take(n_extra as usize) {
            let slot = cap_slot(m, tcb, extra.cptrs[index])?;
            let cap = m
                .cap_at(slot.cnode, slot.slot)
                .map_err(|error| match error {
                    KError::SlotEmpty => KError::InvalidCapability,
                    other => other,
                })?;
            let info = m.cap_info(cap)?;
            if !info.alive {
                return Err(KError::InvalidCapability);
            }
            if info.rights & RIGHT_GRANT == 0 {
                return Err(KError::InsufficientRights);
            }
            *snap = CapSnapshot {
                cap,
                obj: info.obj,
                ty: info.ty,
                rights: info.rights,
                badge: info.badge,
            };
        }
        // `min` là **phòng thủ** (biên đã kiểm ở trên): nếu ai bỏ kiểm `n > 4`, message vẫn
        // không tràn mảng — nó chỉ **cắt** còn 4 (đột biến M2 làm ca âm (ii) đỏ, không panic).
        let n = core::cmp::min(n_extra as usize, MAX_EXTRA_CAPS);
        msg = msg.with_extra_caps(&snaps[..n])?;
    }
    Ok(msg)
}

/// [`apply_ipc_header`] rồi [`build_message`] — đường chuẩn của mọi syscall IPC **gửi** (0/1/2/
/// 5/6/7) sau KT-67b.
fn build_with_header(
    m: &mut Machine,
    tcb: ObjId,
    raw_info: u64,
    mrs: &[u64; WORDS_IN_REGISTERS],
    env: &mut impl SyscallEnv,
) -> Result<Message, KError> {
    let header = apply_ipc_header(m, tcb, env)?;
    // KT-282: chỉ chạm **bộ nhớ user** khi message thật sự cần (`length > 4`) — message ≤ 4 MR
    // giữ nguyên hành vi trước KT-282 (mọi ảnh cũ không đổi).
    let len = MessageInfo::unpack(raw_info)?.length() as usize;
    let buffer = if len > WORDS_IN_REGISTERS {
        // Core quyết **có** buffer hay không (và VA nào); arch chỉ đọc bộ nhớ user tại VA đó.
        match m.tcb_ipc_buffer_binding(tcb) {
            Ok(Some(binding)) => env.ipc_buffer_words(tcb, binding.va),
            _ => None,
        }
    } else {
        None
    };
    // **KT-309** — extra cap đi **cùng** header (cùng trang IPC buffer, offset 560).
    let extra = header.map(|h| h.extra);
    build_message(m, tcb, raw_info, mrs, buffer.as_ref(), header, extra)
}

/// `true` ⇔ cap tại `slot` là **Notification** (nhánh trả `word/badge`, không có `MessageInfo`).
fn cap_is_notification(m: &Machine, slot: SlotRef) -> bool {
    m.cap_at(slot.cnode, slot.slot)
        .ok()
        .and_then(|cap| m.cap_info(cap).ok())
        .map(|info| info.ty == crate::error::ObjType::Notification)
        .unwrap_or(false)
}

/// Payload Notification **vừa nhận** vào TCB (`word`/`badge`): Notification không có
/// `MessageInfo` ⇒ `a1 = value = word`, `mr[0] = word`, `mr[1] = badge` (docs §11.2). `None` nếu
/// TCB chưa nhận payload nào.
fn notification_return(m: &mut Machine, tcb: ObjId, error: u64) -> Option<SyscallReturn> {
    let value = m.take_tcb_notification(tcb).ok().flatten()?;
    let mut r = SyscallReturn::new(error, value.word);
    r.mr[0] = value.word;
    r.mr[1] = value.badge;
    Some(r)
}

/// `true` ⇔ TCB đang `Running` — syscall trả về user được.
fn is_running(m: &Machine, tcb: ObjId) -> bool {
    matches!(m.tcb_state(tcb), Ok(TcbState::Running { .. }))
}

/// Trả `info` + MR0..MR3 của message **vừa nhận** trong vùng nhận IPC của TCB.
fn received(m: &Machine, tcb: ObjId, error: u64, value: u64) -> SyscallReturn {
    let Ok(area) = m.tcb_ipc_area(tcb) else {
        return SyscallReturn::new(error, value);
    };
    let mut out = SyscallReturn::new(error, value);
    if let Some(info) = area.info {
        out.info = info.pack();
    }
    for i in 0..out.mr.len() {
        out.mr[i] = area.mr(i).unwrap_or(0);
    }
    out
}

/// Sau một thao tác IPC: TCB còn `Running` ⇒ trả về user; `Ready` ⇒ `dispatch` lại (Reply/
/// Signal làm TCB về `Ready` — mô hình một-hart, nếu chính nó được chọn thì tiếp tục); ngược lại
/// ⇒ [`SyscallOutcome::Switched`].
/// Số MR của message **vừa nhận** (0 nếu vùng nhận chưa có `MessageInfo`).
fn recv_len(m: &Machine, tcb: ObjId) -> u64 {
    m.tcb_ipc_area(tcb)
        .ok()
        .and_then(|a| a.info)
        .map(|i| u64::from(i.length()))
        .unwrap_or(0)
}

fn after_ipc(m: &mut Machine, tcb: ObjId, hart: u16, r: SyscallReturn) -> SyscallOutcome {
    if is_running(m, tcb) {
        return SyscallOutcome::Return(received(m, tcb, r.error, r.value));
    }
    if matches!(m.tcb_state(tcb), Ok(TcbState::Ready)) {
        let _ = m.dispatch(hart);
        if is_running(m, tcb) {
            return SyscallOutcome::Return(received(m, tcb, r.error, r.value));
        }
    }
    let state = m.tcb_state(tcb).unwrap_or(TcbState::Inactive);
    SyscallOutcome::Switched { state }
}

/// Một syscall đi vào [`Machine`]. **Không panic** với mọi `nr`/tham số.
///
/// `tcb` là TCB gọi (đang `Running` trên `hart`); `args` = `a0..a5`.
pub fn dispatch(
    m: &mut Machine,
    tcb: ObjId,
    hart: u16,
    nr: u64,
    args: [u64; 6],
    env: &mut impl SyscallEnv,
) -> SyscallOutcome {
    // D2-79: số syscall là **chỉ số** vào bảng dispatch ⇒ ngoài 0..15 là `RangeError` 19.
    if nr >= SYSCALL_COUNT as u64 {
        return SyscallOutcome::Return(SyscallReturn::err(KError::RangeError.id() as u64));
    }
    match nr {
        // ---------------------------------------------------------------- IPC 0..7
        0 => match cap_slot(m, tcb, args[0]) {
            // D2-97: cap **không** phải Endpoint ⇒ invocation (method = `label`), trả ngay.
            Ok(slot) => match cap_type_at(m, slot) {
                Ok(ty) if ty != ObjType::Endpoint => {
                    let mrs = [args[2], args[3], args[4], args[5]];
                    match crate::invoke::dispatch(m, tcb, slot, args[1], &mrs, env) {
                        Ok((error, value)) => {
                            SyscallOutcome::Return(SyscallReturn::new(error, value))
                        }
                        Err(code) => SyscallOutcome::Return(SyscallReturn::err(code)),
                    }
                }
                Ok(_) => match build_with_header(
                    m,
                    tcb,
                    args[1],
                    &[args[2], args[3], args[4], args[5]],
                    env,
                ) {
                    Ok(msg) => match m.call_msg(tcb, slot, msg) {
                        Ok(()) => after_ipc(m, tcb, hart, SyscallReturn::new(0, 0)),
                        Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                    },
                    Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                },
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            },
            Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
        },
        1 => match cap_slot(m, tcb, args[0]).and_then(|cap| {
            build_with_header(m, tcb, args[1], &[args[2], args[3], args[4], args[5]], env)
                .map(|msg| (cap, msg))
        }) {
            Ok((cap, msg)) => match m.send(tcb, cap, msg) {
                Ok(()) => after_ipc(m, tcb, hart, SyscallReturn::new(0, 0)),
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            },
            Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
        },
        2 => match cap_slot(m, tcb, args[0]).and_then(|cap| {
            build_with_header(m, tcb, args[1], &[args[2], args[3], args[4], args[5]], env)
                .map(|msg| (cap, msg))
        }) {
            Ok((cap, msg)) => match m.nb_send(tcb, cap, msg) {
                Ok(()) => after_ipc(m, tcb, hart, SyscallReturn::new(0, 0)),
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            },
            Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
        },
        3 => {
            let reply = if args[1] == REPLY_NONE {
                None
            } else {
                match cap_slot(m, tcb, args[1]) {
                    Ok(slot) => Some(slot),
                    Err(e) => {
                        return SyscallOutcome::Return(SyscallReturn::err(e.id() as u64));
                    }
                }
            };
            let (target, is_ntfn) = match cap_slot(m, tcb, args[0]) {
                Ok(slot) => (slot, cap_is_notification(m, slot)),
                Err(e) => return SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            };
            // KT-67b: receive window lấy từ header IPC buffer **trước** khi giao (message có
            // cap cần window). Notification không dùng window ⇒ bỏ qua.
            if !is_ntfn {
                if let Err(e) = apply_ipc_header(m, tcb, env) {
                    return SyscallOutcome::Return(SyscallReturn::err(e.id() as u64));
                }
            }
            match m.recv(tcb, target, reply) {
                Ok(()) => {
                    // Notification: payload `(word, badge)` trong `tcb.notif` — **không** đọc
                    // `ipc_area` (có thể là message Endpoint cũ ⇒ stale).
                    if is_ntfn {
                        if is_running(m, tcb) {
                            if let Some(r) = notification_return(m, tcb, 0) {
                                return SyscallOutcome::Return(r);
                            }
                        }
                        return SyscallOutcome::Switched {
                            state: m.tcb_state(tcb).unwrap_or(TcbState::Inactive),
                        };
                    }
                    // Endpoint: giao ngay ⇒ `value` = số MR; chặn ⇒ 0.
                    let value = if is_running(m, tcb) {
                        recv_len(m, tcb)
                    } else {
                        0
                    };
                    after_ipc(m, tcb, hart, SyscallReturn::new(0, value))
                }
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            }
        }
        4 => {
            let (target, is_ntfn) = match cap_slot(m, tcb, args[0]) {
                Ok(slot) => (slot, cap_is_notification(m, slot)),
                Err(e) => return SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            };
            if !is_ntfn {
                if let Err(e) = apply_ipc_header(m, tcb, env) {
                    return SyscallOutcome::Return(SyscallReturn::err(e.id() as u64));
                }
            }
            match m.nb_recv(tcb, target) {
                Ok(Some(msg)) => {
                    if is_ntfn {
                        // `notification_message`: MR0 = word, `badge` = badge.
                        let mut r = SyscallReturn::new(0, msg.mr[0]);
                        r.mr[0] = msg.mr[0];
                        r.mr[1] = msg.badge;
                        SyscallOutcome::Return(r)
                    } else {
                        SyscallOutcome::Return(received(m, tcb, 0, msg.info.length() as u64))
                    }
                }
                Ok(None) => SyscallOutcome::Return(SyscallReturn::err(KError::Busy.id() as u64)),
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            }
        }
        5 => {
            let built = cap_slot(m, tcb, args[0]).and_then(|reply| {
                cap_slot(m, tcb, args[1]).and_then(|ep| {
                    build_with_header(m, tcb, args[2], &[args[3], args[4], args[5], 0], env)
                        .map(|msg| (reply, ep, msg))
                })
            });
            match built {
                Ok((reply, ep, msg)) => match m.reply_recv(tcb, reply, ep, msg, None) {
                    Ok(()) => after_ipc(m, tcb, hart, SyscallReturn::new(0, 0)),
                    Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                },
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            }
        }
        6 => {
            // 3 cap nên `MessageInfo` đi `a3`, MR0..MR2 ở `a4..a6` (ghi ở docs §11).
            let recv_reply = if args[2] == REPLY_NONE {
                None
            } else {
                match cap_slot(m, tcb, args[2]) {
                    Ok(slot) => Some(slot),
                    Err(e) => {
                        return SyscallOutcome::Return(SyscallReturn::err(e.id() as u64));
                    }
                }
            };
            let built = cap_slot(m, tcb, args[0]).and_then(|send| {
                cap_slot(m, tcb, args[1]).and_then(|recv| {
                    build_with_header(m, tcb, args[3], &[args[4], args[5], 0, 0], env)
                        .map(|msg| (send, recv, msg))
                })
            });
            match built {
                Ok((send, recv, msg)) => match m.nb_send_recv(tcb, send, msg, recv, recv_reply) {
                    Ok(send_err) => {
                        let error = send_err.map(|e| e.id() as u64).unwrap_or(0);
                        after_ipc(m, tcb, hart, SyscallReturn::new(error, 0))
                    }
                    Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                },
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            }
        }
        7 => {
            let built = cap_slot(m, tcb, args[0]).and_then(|send| {
                cap_slot(m, tcb, args[1]).and_then(|ntfn| {
                    build_with_header(m, tcb, args[2], &[args[3], args[4], args[5], 0], env)
                        .map(|msg| (send, ntfn, msg))
                })
            });
            match built {
                Ok((send, ntfn, msg)) => match m.nb_send_wait(tcb, send, msg, ntfn) {
                    Ok(send_err) => {
                        let error = send_err.map(|e| e.id() as u64).unwrap_or(0);
                        // Wait giao ngay ⇒ payload `(word, badge)`; chặn ⇒ Switched.
                        if is_running(m, tcb) {
                            if let Some(r) = notification_return(m, tcb, error) {
                                return SyscallOutcome::Return(r);
                            }
                            return SyscallOutcome::Return(SyscallReturn::new(error, 0));
                        }
                        SyscallOutcome::Switched {
                            state: m.tcb_state(tcb).unwrap_or(TcbState::Inactive),
                        }
                    }
                    Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                },
                Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
            }
        }
        // ---------------------------------------------------------------- scheduler 8
        SYS_YIELD => match m.yield_current(hart) {
            Ok(next) => {
                let value = m.tcb_remaining(tcb);
                if next == Some(tcb) || is_running(m, tcb) {
                    SyscallOutcome::Return(SyscallReturn::new(0, value))
                } else {
                    let state = m.tcb_state(tcb).unwrap_or(TcbState::Inactive);
                    SyscallOutcome::Switched { state }
                }
            }
            Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
        },
        // ---------------------------------------------------------------- futex 9/10
        9 => match env.resolve_addr(args[0]) {
            Ok((frame, offset)) => {
                let expected = args[1];
                match m.wait_addr_obj(tcb, frame, offset, expected) {
                    Ok(()) => SyscallOutcome::Switched {
                        state: TcbState::BlockedOnAddress { frame, offset },
                    },
                    Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                }
            }
            Err(code) => SyscallOutcome::Return(SyscallReturn::err(code)),
        },
        10 => match env.resolve_addr(args[0]) {
            Ok((frame, offset)) => {
                let n = u32::try_from(args[1]).unwrap_or(u32::MAX);
                match m.wake_addr(frame, offset, n) {
                    Ok(woken) => SyscallOutcome::Return(SyscallReturn::new(0, u64::from(woken))),
                    Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
                }
            }
            Err(code) => SyscallOutcome::Return(SyscallReturn::err(code)),
        },
        // ---------------------------------------------------------------- clock/debug 11..15
        11 => SyscallOutcome::Return(SyscallReturn::new(0, env.now_ns())),
        SYS_DEBUG_PUT_STR => {
            let (ptr, len) = (args[0], args[1]);
            if len > MAX_DEBUG_PUT {
                return SyscallOutcome::Return(SyscallReturn::err(
                    KError::InvalidArgument.id() as u64
                ));
            }
            match env.put_str(ptr, len) {
                Ok(bytes) => SyscallOutcome::Return(SyscallReturn::new(0, bytes)),
                Err(code) => SyscallOutcome::Return(SyscallReturn::err(code)),
            }
        }
        SYS_DEBUG_HALT => SyscallOutcome::Halt { code: args[0] },
        SYS_EXIT => match m.exit(tcb, args[0]) {
            Ok(()) => SyscallOutcome::Exit { code: args[0] },
            Err(e) => SyscallOutcome::Return(SyscallReturn::err(e.id() as u64)),
        },
        SYS_DEBUG_GET_INFO => {
            let mut r = SyscallReturn::new(0, ABI_VERSION);
            r.mr[0] = MIN_COMPAT;
            r.mr[1] = env.features();
            r.mr[2] = env.num_harts();
            SyscallOutcome::Return(r)
        }
        // `nr < SYSCALL_COUNT` bao trùm mọi nhánh ở trên; nhánh này chỉ để compiler thấy đủ.
        _ => SyscallOutcome::Return(SyscallReturn::err(KError::RangeError.id() as u64)),
    }
}

/// `true` ⇔ TCB có `cspace` (dùng cho chẩn đoán/TAP).
pub fn tcb_has_cspace(m: &Machine, tcb: ObjId) -> bool {
    m.tcb_info_obj(tcb)
        .map(|i| i.cspace.is_some())
        .unwrap_or(false)
}

/// `WORDS_IN_REGISTERS` — số MR đi trong thanh ghi (`spec/abi.toml` = 4).
pub const MR_IN_REGS: usize = WORDS_IN_REGISTERS;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CapId;
    use crate::ipc::IPC_HEADER_MAGIC;

    /// Env giả chỉ trả **header IPC buffer** — các hook khác không dùng trong test KT-67b
    /// (giữ `no_std`-an toàn, không chạm bộ nhớ user).
    struct HeaderEnv(Option<IpcHeader>);

    impl SyscallEnv for HeaderEnv {
        fn put_str(&mut self, _ptr: u64, _len: u64) -> Result<u64, u64> {
            Ok(0)
        }
        fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
            Err(KError::NotMapped.id() as u64)
        }
        fn now_ns(&mut self) -> u64 {
            0
        }
        fn features(&mut self) -> u64 {
            0
        }
        fn num_harts(&mut self) -> u64 {
            1
        }
        fn ipc_header(&mut self, _tcb: ObjId) -> Option<IpcHeader> {
            self.0
        }
    }

    fn header(window: [u64; 4], send_caps: [u64; 4]) -> IpcHeader {
        IpcHeader {
            info: 0,
            recv_window: window,
            send_caps,
            extra: IpcExtraCaps::EMPTY,
            configure: crate::ipc::IpcConfigureParams::EMPTY,
        }
    }

    /// Machine nhỏ: CNode 16 ô (0 Untyped, 1 TCB, 2 cap cspace, 3 Endpoint), TCB đã configure.
    fn fixture() -> (Machine, ObjId, ObjId, CapId) {
        let mut m = Machine::new();
        let root = m.add_cnode(16);
        let u = m.add_untyped(0x1000, 1 << 16);
        let ucap = m.root_cap(u);
        let us = SlotRef::new(root, 0);
        m.put_cap(root, 0, ucap).expect("cap Untyped");
        let cs = m.root_cap(root);
        m.put_cap(root, 2, cs).expect("cap cspace");
        let tcb_cap = m.retype(us, ObjType::Tcb, 0, 0).expect("retype Tcb");
        m.put_cap(root, 1, tcb_cap).expect("cap Tcb");
        let tcb = m.cap_obj(tcb_cap).expect("obj Tcb");
        m.tcb_configure(SlotRef::new(root, 1), Some(SlotRef::new(root, 2)))
            .expect("cspace cua TCB");
        let ep_cap = m
            .retype(us, ObjType::Endpoint, 0, 0)
            .expect("retype Endpoint");
        m.put_cap(root, 3, ep_cap).expect("cap Endpoint");
        (m, root, tcb, ep_cap)
    }

    /// Magic trong core phải **đúng** chuỗi `b"KT67b"` little-endian (shim ghi cùng hằng — nếu
    /// một bên đổi, TAP 19 đỏ ngay vì magic không khớp).
    #[test]
    fn magic_header_khop_b_kt67b() {
        assert_eq!(IPC_HEADER_MAGIC.to_le_bytes(), *b"KT67b\0\0\0");
    }

    /// `apply_ipc_header` áp **cả 4** CPTR vào receive window của TCB.
    #[test]
    fn window_tu_header_vao_tcb() {
        let (mut m, root, tcb, _ep) = fixture();
        let mut env = HeaderEnv(Some(header([8, 9, 10, 11], [0; 4])));
        assert!(apply_ipc_header(&mut m, tcb, &mut env)
            .expect("header hợp lệ")
            .is_some());
        let window = m
            .tcb_recv_window(tcb)
            .expect("đọc window")
            .expect("đã bind");
        assert_eq!(
            window,
            [
                SlotRef::new(root, 8),
                SlotRef::new(root, 9),
                SlotRef::new(root, 10),
                SlotRef::new(root, 11),
            ]
        );
    }

    /// CPTR ngoài bảng CSpace ⇒ `InvalidCapability` (1) — không im lặng bỏ qua window.
    #[test]
    fn window_ngoai_bang_la_invalid_capability() {
        let (mut m, _root, tcb, _ep) = fixture();
        let mut env = HeaderEnv(Some(header([99, 0, 0, 0], [0; 4])));
        assert_eq!(
            apply_ipc_header(&mut m, tcb, &mut env),
            Err(KError::InvalidCapability)
        );
    }

    /// Hai CPTR trùng nhau ⇒ `InvalidArgument` (5): hai cap cài cùng ô sẽ đè nhau.
    #[test]
    fn window_trung_o_la_invalid_argument() {
        let (mut m, _root, tcb, _ep) = fixture();
        let mut env = HeaderEnv(Some(header([8, 8, 9, 10], [0; 4])));
        assert_eq!(
            apply_ipc_header(&mut m, tcb, &mut env),
            Err(KError::InvalidArgument)
        );
    }

    /// `caps = 1` + header ⇒ message mang **đúng** cap nguồn từ CPTR trong header.
    #[test]
    fn build_message_lay_cap_tu_header() {
        let (m, _root, tcb, ep_cap) = fixture();
        let info = MessageInfo::new(0x670, 1, false, 0, 1)
            .expect("MessageInfo hợp lệ")
            .pack();
        let msg = build_message(
            &m,
            tcb,
            info,
            &[0xAA, 0, 0, 0],
            None,
            Some(header([8, 9, 10, 11], [3, 0, 0, 0])),
            None,
        )
        .expect("message mang cap");
        assert_eq!(msg.cap_count(), 1);
        assert_eq!(msg.cap_ids(), &[ep_cap]);
        assert_eq!(msg.words(), &[0xAA]);
    }

    /// `caps > 0` mà **không** có header ⇒ `InvalidArgument` (5): không đoán CPTR nguồn.
    #[test]
    fn caps_khong_header_la_invalid_argument() {
        let (m, _root, tcb, _ep) = fixture();
        let info = MessageInfo::new(0, 1, false, 0, 0)
            .expect("MessageInfo hợp lệ")
            .pack();
        assert_eq!(
            build_message(&m, tcb, info, &[0; 4], None, None, None),
            Err(KError::InvalidArgument)
        );
    }

    /// **KT-282** — `length = 8` ⇒ MR4..MR7 lấy từ **IPC buffer**; không có buffer ⇒
    /// `InvalidArgument` (5); `length = 65` ⇒ `InvalidArgument` (không tràn 60 word).
    #[test]
    fn mr4_qua_ipc_buffer() {
        let (m, _root, tcb, _ep) = fixture();
        let info = MessageInfo::new(0x282, 0, false, 0, 8)
            .expect("MessageInfo 8 MR")
            .pack();
        let mut buffer = [0u64; WORDS_IN_IPC_BUFFER];
        buffer[0] = 0x2820_0004;
        buffer[3] = 0x2820_0007;
        let msg = build_message(&m, tcb, info, &[1, 2, 3, 4], Some(&buffer), None, None)
            .expect("8 MR qua buffer");
        assert_eq!(msg.words(), &[1, 2, 3, 4, 0x2820_0004, 0, 0, 0x2820_0007]);
        // Chưa bind buffer ⇒ fail-closed (đúng hành vi trước KT-282 — đột biến M1).
        assert_eq!(
            build_message(&m, tcb, info, &[1, 2, 3, 4], None, None, None),
            Err(KError::InvalidArgument)
        );
        // `length = 65` vượt `max_words = 64` ⇒ `InvalidArgument` (không tràn).
        assert_eq!(
            MessageInfo::new(0x282, 0, false, 0, 65),
            Err(KError::InvalidArgument)
        );
        // `length = 64` là **hợp lệ** (MR0..MR63) — đột biến M2 (`>= 64`) làm ca này đỏ.
        let full = MessageInfo::new(0x282, 0, false, 0, 64).expect("64 MR hợp lệ");
        let mut all = [0u64; WORDS_IN_IPC_BUFFER];
        all[59] = 0x2820_003f;
        let msg = build_message(&m, tcb, full.pack(), &[1, 2, 3, 4], Some(&all), None, None)
            .expect("64 MR qua buffer");
        assert_eq!(msg.words().len(), 64);
        assert_eq!(msg.words()[63], 0x2820_003f);
    }

    /// **KT-309** — `IpcExtraCaps::from_words` đọc đúng layout 6 word: `n`, 4 CPTR, `dst`.
    #[test]
    fn extra_caps_layout_sau_word() {
        let words = [1u64, 5, 0, 0, 0, 7];
        let extra = IpcExtraCaps::from_words(&words);
        assert_eq!(extra.count, 1);
        assert_eq!(extra.cptrs, [5, 0, 0, 0]);
        assert_eq!(extra.dst, 7);
        assert!(!extra.is_empty());
        assert!(IpcExtraCaps::EMPTY.is_empty());
    }

    /// **KT-309** — extra cap đi qua **vùng dự phòng** của IPC buffer: `n = 1` + CPTR ⇒ message
    /// mang **một** ảnh chụp extra cap (obj/ty/rights/badge của cap nguồn, chụp **ngay** ở
    /// `build_message` vì extra cap **không** đi qua `resolve_caps`).
    #[test]
    fn extra_caps_qua_ipc_buffer() {
        let (m, _root, tcb, ep_cap) = fixture();
        let info = MessageInfo::new(0x309, 0, false, 0, 1)
            .expect("MessageInfo hợp lệ")
            .pack();
        let extra = IpcExtraCaps {
            count: 1,
            cptrs: [3, 0, 0, 0],
            dst: 0,
        };
        let msg = build_message(&m, tcb, info, &[0xAA, 0, 0, 0], None, None, Some(extra))
            .expect("extra cap hợp lệ");
        assert_eq!(msg.n_extra, 1);
        let snaps = msg.extra_snapshots();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].cap, ep_cap);
        assert_eq!(snaps[0].ty, ObjType::Endpoint);
        assert_eq!(snaps[0].rights, crate::spec::RIGHTS_ALL);
        // Message **không** mang cap trong thanh ghi ⇒ `caps` vẫn 0 (hai kênh **tách biệt**).
        assert_eq!(msg.cap_count(), 0);
        assert_eq!(msg.info.caps(), 0);
    }

    /// **KT-309** — cap nguồn **thiếu `RIGHT_GRANT`** ⇒ `InsufficientRights` (3): lỗi **cứng**
    /// ngay ở đường gửi (caller **chủ động** khai `n` nên sai là lỗi) — đột biến **M1** (bỏ kiểm)
    /// làm test này đỏ.
    #[test]
    fn extra_caps_thieu_grant_la_insufficient_rights() {
        let (mut m, root, tcb, _ep) = fixture();
        m.copy_rights(
            SlotRef::new(root, 3),
            SlotRef::new(root, 4),
            Some(crate::spec::RIGHTS_ALL & !RIGHT_GRANT),
        )
        .expect("cap thiếu GRANT");
        let info = MessageInfo::new(0x309, 0, false, 0, 1)
            .expect("MessageInfo hợp lệ")
            .pack();
        let extra = IpcExtraCaps {
            count: 1,
            cptrs: [4, 0, 0, 0],
            dst: 0,
        };
        assert_eq!(
            build_message(&m, tcb, info, &[0xAA, 0, 0, 0], None, None, Some(extra)),
            Err(KError::InsufficientRights)
        );
    }

    /// **KT-309** — `n = 5` > `max_caps = 4` ⇒ `InvalidArgument` (5): biên ABI **không** nới —
    /// đột biến **M2** (bỏ kiểm) làm test này đỏ.
    #[test]
    fn extra_caps_qua_bon_la_invalid_argument() {
        let (m, _root, tcb, _ep) = fixture();
        let info = MessageInfo::new(0x309, 0, false, 0, 1)
            .expect("MessageInfo hợp lệ")
            .pack();
        let extra = IpcExtraCaps {
            count: 5,
            cptrs: [3, 3, 3, 3],
            dst: 0,
        };
        assert_eq!(
            build_message(&m, tcb, info, &[0xAA, 0, 0, 0], None, None, Some(extra)),
            Err(KError::InvalidArgument)
        );
    }

    /// **KT-309** — CPTR extra cap trỏ ô **rỗng** ⇒ `InvalidCapability` (1), không gửi gì.
    #[test]
    fn extra_caps_cap_rong_la_invalid_capability() {
        let (m, _root, tcb, _ep) = fixture();
        let info = MessageInfo::new(0x309, 0, false, 0, 1)
            .expect("MessageInfo hợp lệ")
            .pack();
        let extra = IpcExtraCaps {
            count: 1,
            cptrs: [9, 0, 0, 0],
            dst: 0,
        };
        assert_eq!(
            build_message(&m, tcb, info, &[0xAA, 0, 0, 0], None, None, Some(extra)),
            Err(KError::InvalidCapability)
        );
    }

    /// CPTR nguồn trỏ ô **rỗng** ⇒ `InvalidCapability` (1) — ca âm của memsrv trong TAP 19.
    #[test]
    fn cap_nguon_rong_la_invalid_capability() {
        let (m, _root, tcb, _ep) = fixture();
        let info = MessageInfo::new(0, 1, false, 0, 0)
            .expect("MessageInfo hợp lệ")
            .pack();
        assert_eq!(
            build_message(
                &m,
                tcb,
                info,
                &[0; 4],
                None,
                Some(header([8, 9, 10, 11], [9, 0, 0, 0])),
                None,
            ),
            Err(KError::InvalidCapability)
        );
    }
}
