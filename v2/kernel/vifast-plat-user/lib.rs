// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Shim ABI U-mode** cho các chương trình Rust của KT-65 (`procsrv`/`shell`/`con`).
//!
//! Vì sao có crate này: `core::arch::asm!` là `unsafe`, mà luật cấu trúc R5 (D2-17) chỉ cho
//! `unsafe` trong danh sách đóng — `kernel/vifast-plat-*` nằm trong danh sách. Gom toàn bộ asm
//! `ecall` vào **một** crate nền tảng, các chương trình U-mode chỉ còn lời gọi an toàn:
//! [`syscall7`]/[`syscall2`]/[`put`]/[`exit`]. ABI đúng bảng syscall hiện có (không thêm syscall).
//!
//! # KT-67b — header **IPC buffer** của U-mode (D2-98)
//!
//! `spec/abi.toml` `[calling.riscv64].ipc_buffer` đã chốt: *"Frame bind qua Tcb.Configure; header
//! chứa receive window (CPTR, ≤4 cap)"*. KT-63 đã có cơ chế chuyển cap ở kernel
//! (`plan_cap_transfer`/`deliver_caps`) nhưng phía U-mode **chưa** ghi/đọc header đó. Task này
//! cài phần còn thiếu:
//!
//! - loader kernel map **một trang 4 KiB** tại VA cố định [`IPC_BUFFER_VA`] (`R|W|U|NX`) cho
//!   **mọi** chương trình U-mode (không phải ABI mới — chỉ là nơi đặt buffer);
//! - 10 word đầu của buffer là **header** `[TK]`:
//!
//! ```text
//! word 0     : magic [`HEADER_MAGIC`]  (0 = header chưa dùng ⇒ kernel bỏ qua)
//! word 1     : MessageInfo 35 bit (bản ghi của shim: info gửi đi hoặc info **nhận** được)
//! word 2..5  : receive window — 4 CPTR (đích cài cap khi **nhận**; spec dòng 58)
//! word 6..9  : send caps — 4 CPTR nguồn cap của message đang **gửi**
//! ```
//!
//! [`set_recv_window`] ghi window + magic; [`read_received_caps`] đọc số cap đã cài
//! (`MessageInfo.caps` do kernel cập nhật theo số cap **thật sự** cài được) và các CPTR tương ứng.
//! Kernel (`vifast-core/src/syscall.rs`, `build_message`) đọc header qua hook `SyscallEnv`
//! (`build_message` **lấy window từ đây**, không lấy từ `Machine` trực tiếp) và giải mã
//! `MessageInfo.caps` + CPTR nguồn thành `CapId` trong CSpace của caller.
//!
//! `MessageInfo` **vẫn** là thanh ghi `a2` theo ABI đóng băng (D2-15); header chỉ là **bản ghi**
//! giúp shim/kernel trao đổi window + cap slot, không đổi layout 35 bit.
//!
//! Crate được `build.rs` của `vifast-kbin-riscv64` biên dịch thành `rlib` riscv64 rồi truyền
//! `--extern` cho từng ELF user; **không** là thành viên workspace (không build cho host).
#![no_std]

use core::arch::asm;
use core::sync::atomic::Ordering;

/// Syscall `Call` — Endpoint: IPC; object khác: invocation (D2-97).
pub const SYS_CALL: u64 = 0;
/// Syscall `Send` — Endpoint/Reply/Notification (spec `:107`). **KT-280**: server U-mode dùng
/// `Send` trên cap `Reply` để trả lời `Call` (đường `Reply` của seL4).
pub const SYS_SEND: u64 = 1;
/// Syscall `NBSend`.
pub const SYS_NB_SEND: u64 = 2;
/// Syscall `Recv`.
pub const SYS_RECV: u64 = 3;
/// Syscall `NBRecv` — không chặn (`Busy` = 14 khi không có message; vòng poll KT-68).
pub const SYS_NB_RECV: u64 = 4;
/// Syscall `Yield`.
pub const SYS_YIELD: u64 = 8;
/// Syscall `ClockRead` — đồng hồ **đơn điệu** của nền tảng (`rdtime`, ns; D2-88(b)).
/// **KT-114**: shell sản phẩm dùng cho `time`/`uptime`; số hiệu khớp
/// `vifast-abi::generated::SYS_CLOCK_READ` (11) — không mở ABI mới.
pub const SYS_CLOCK_READ: u64 = 11;
/// Syscall `DebugPutStr` — kernel đọc vùng nhớ user và in ra UART.
pub const SYS_DEBUG_PUT_STR: u64 = 12;
/// Syscall `Exit` — core ghi `FaultKind::Exit{code}` vào `fault_ep` (QD §1 Q-A).
pub const SYS_EXIT: u64 = 14;

/// `a1` của `Recv` khi **không** có cap Reply (`vifast-core::syscall::REPLY_NONE`).
pub const REPLY_NONE: u64 = u64::MAX;

// ---------------------------------------------------------------------------------------------
// KT-67b — header IPC buffer (spec dòng 58: header chứa receive window CPTR, ≤4 cap)
// ---------------------------------------------------------------------------------------------

/// VA cố định của trang **IPC buffer** — loader map `R|W|U|NX` cho mọi ELF U-mode.
pub const IPC_BUFFER_VA: u64 = 0x4002_0000;
/// Cỡ IPC buffer — `spec/abi.toml` `[message].ipc_buffer_bytes = 4096`.
pub const IPC_BUFFER_BYTES: usize = 4096;
/// Số CPTR của receive window — `spec/abi.toml` `[message].max_caps = 4`.
pub const MAX_CAPS: usize = 4;
/// Số word của header (magic + info + window 4 + send caps 4).
pub const HEADER_WORDS: usize = 10;
/// Magic `b"KT67b"` little-endian — kernel chỉ đọc header khi khớp (vùng zero = chưa dùng).
pub const HEADER_MAGIC: u64 = 0x0000_0062_3736_544b;

/// Vị trí word của `MessageInfo` trong header.
const W_INFO: usize = 1;
/// Vị trí word đầu của receive window.
const W_WINDOW: usize = 2;
/// Vị trí word đầu của send caps.
const W_SEND_CAPS: usize = 6;

/// Bit `label` trong `MessageInfo` (spec `[message_info].label_bits = 20`).
pub const INFO_LABEL_BITS: u32 = 20;
/// Dịch bit của `caps` (spec `[message_info].caps:20..22`).
pub const INFO_CAPS_SHIFT: u32 = 20;
/// Dịch bit của `no_reply` (spec `[message_info].no_reply = 23`).
pub const INFO_NO_REPLY_SHIFT: u32 = 23;
/// Dịch bit của `length` (spec `[message_info].length:28..34`).
pub const INFO_LENGTH_SHIFT: u32 = 28;

fn read_word(index: usize) -> u64 {
    // SAFETY: trang IPC buffer đã được loader map `R|W|U|NX` tại VA cố định cho **mọi** ELF
    // U-mode; `index < HEADER_WORDS` nên địa chỉ nằm trong 10 word đầu của trang. Một tiến trình
    // U-mode một luồng ⇒ không có writer đồng thời.
    unsafe { core::ptr::read_volatile((IPC_BUFFER_VA + (index as u64) * 8) as *const u64) }
}

fn write_word(index: usize, value: u64) {
    // SAFETY: như [`read_word`] — ghi trong 10 word đầu của trang RW của chính tiến trình.
    unsafe { core::ptr::write_volatile((IPC_BUFFER_VA + (index as u64) * 8) as *mut u64, value) }
}

/// Địa chỉ IPC buffer của tiến trình (bằng [`IPC_BUFFER_VA`]; có hàm để chương trình in/log).
pub const fn ipc_buffer_addr() -> u64 {
    IPC_BUFFER_VA
}

// ---------------------------------------------------------------------------------------------
// KT-282 — vùng **MR4..MR63** trong IPC buffer (frame 4 KiB bind qua `Tcb.Configure`)
// ---------------------------------------------------------------------------------------------

/// Byte offset của vùng **MR4..MR63** trong IPC buffer — `[TK]` KT-282: ngay sau header 10 word
/// (80 B, đã căn 16 theo `spec/abi.toml` `[message].ipc_buffer_align`). Cùng hằng với kernel
/// (`vifast_core::ipc::IPC_BUFFER_MR_OFFSET`) — một layout, hai phía đọc/ghi.
pub const MR_OFFSET: usize = HEADER_WORDS * 8;
/// Số word của vùng MR4..MR63 — `spec/abi.toml` `[message].words_in_ipc_buffer = 60`.
pub const MR_WORDS: usize = 60;

fn read_mr_word(index: usize) -> u64 {
    // SAFETY: `MR_OFFSET + index*8` nằm trong trang IPC buffer 4 KiB đã map `R|W|U|NX` cho mọi
    // ELF U-mode; `index < MR_WORDS` ⇒ địa chỉ < 80 + 480 = 560 < 4096. Một tiến trình một
    // luồng ⇒ không có writer đồng thời.
    unsafe {
        core::ptr::read_volatile((IPC_BUFFER_VA + (MR_OFFSET + index * 8) as u64) as *const u64)
    }
}

fn write_mr_word(index: usize, value: u64) {
    // SAFETY: như [`read_mr_word`] — ghi trong vùng MR của trang RW của chính tiến trình.
    unsafe {
        core::ptr::write_volatile(
            (IPC_BUFFER_VA + (MR_OFFSET + index * 8) as u64) as *mut u64,
            value,
        )
    }
}

/// Ghi **MR4..** vào vùng buffer (word 0 = MR4, word 1 = MR5, …). Trả số word đã ghi
/// (≤ [`MR_WORDS`] = 60; phần thừa của `words` bị bỏ — biên `max_words = 64`).
///
/// Chương trình U-mode gọi hàm này **trước** `call`/`send` khi `MessageInfo.length > 4`; kernel
/// đọc đúng vùng này qua hook `SyscallEnv::ipc_buffer_words` (D2-83).
pub fn set_mrs(words: &[u64]) -> usize {
    let n = if words.len() > MR_WORDS {
        MR_WORDS
    } else {
        words.len()
    };
    let mut index = 0;
    while index < n {
        write_mr_word(index, words[index]);
        index += 1;
    }
    n
}

/// Đọc **MR4..MR63** từ vùng buffer (word 0 = MR4). Kernel ghi vùng này khi giao message có
/// `length > 4` (đường `Recv`/`ReplyRecv`/resume sau `Call`).
pub fn mrs() -> [u64; MR_WORDS] {
    let mut words = [0u64; MR_WORDS];
    let mut index = 0;
    while index < MR_WORDS {
        words[index] = read_mr_word(index);
        index += 1;
    }
    words
}

/// MR thứ `index` (0..60) của vùng buffer = **MR(index + 4)** của message vừa nhận/gửi.
pub fn mr(index: usize) -> u64 {
    if index >= MR_WORDS {
        return 0;
    }
    read_mr_word(index)
}

/// Ghi **một** MR vào vùng buffer: `index` 0..60 = MR(index+4). Trả `false` nếu ngoài biên.
pub fn set_mr(index: usize, value: u64) -> bool {
    if index >= MR_WORDS {
        return false;
    }
    write_mr_word(index, value);
    true
}

// ---------------------------------------------------------------------------------------------
// KT-309 — vùng **extra caps** trong IPC buffer (cap transfer **qua buffer**, > 4 cap)
// ---------------------------------------------------------------------------------------------

/// Byte offset của word **`n`** (số extra cap) — `[TK]` KT-309: ngay sau vùng MR4..MR63
/// (`MR_OFFSET + MR_WORDS*8` = 560). Cùng hằng với kernel
/// (`vifast_core::ipc::IPC_BUFFER_EXTRA_CAPS_OFFSET`) — một layout, hai phía đọc/ghi.
pub const EXTRA_CAPS_OFFSET: usize = MR_OFFSET + MR_WORDS * 8;
/// Số **extra cap** tối đa — `spec/abi.toml` `[message].max_caps = 4` (biên **không nới**).
pub const MAX_EXTRA_CAPS: usize = 4;
/// Byte offset của word **`dst`** (slot đích trong CSpace **của chính tiến trình này** khi nó là
/// bên nhận) — `560 + (1 + 4)*8 = 600`.
pub const EXTRA_CAPS_DST_OFFSET: usize = EXTRA_CAPS_OFFSET + (1 + MAX_EXTRA_CAPS) * 8;

fn read_extra_word(index: usize) -> u64 {
    // SAFETY: `EXTRA_CAPS_OFFSET + index*8` nằm trong trang IPC buffer 4 KiB đã map `R|W|U|NX`
    // cho mọi ELF U-mode; `index < 6` ⇒ địa chỉ < 560 + 48 = 608 < 4096. Một tiến trình một
    // luồng ⇒ không có writer đồng thời.
    unsafe {
        core::ptr::read_volatile(
            (IPC_BUFFER_VA + (EXTRA_CAPS_OFFSET + index * 8) as u64) as *const u64
        )
    }
}

fn write_extra_word(index: usize, value: u64) {
    // SAFETY: như [`read_extra_word`] — ghi trong vùng extra caps của trang RW của chính nó.
    unsafe {
        core::ptr::write_volatile(
            (IPC_BUFFER_VA + (EXTRA_CAPS_OFFSET + index * 8) as u64) as *mut u64,
            value,
        )
    }
}

/// **KT-309** — khai **extra cap** cho message **đang gửi**: `cptrs` = CPTR trong CSpace **của
/// chính tiến trình** (≤ [`MAX_EXTRA_CAPS`] = 4). Kernel đọc vùng này ở `Call`/`Send`/`NBSend`
/// qua hook `SyscallEnv::ipc_buffer_extra_caps` (D2-83) và **copy** cap sang CSpace bên nhận.
///
/// Trả số CPTR đã ghi (phần thừa của `cptrs` bị bỏ — biên `max_caps = 4`, **không** nới).
pub fn set_extra_caps(cptrs: &[u64]) -> usize {
    // `n` = **đúng** số caller khai (kể cả > 4): shim **không** che giấu lời khai — kernel là
    // bên phán quyết (`n > max_caps = 4` ⇒ `InvalidArgument` 5). Chỉ **4** CPTR đầu được ghi
    // (biên vùng), phần thừa không có chỗ trong buffer.
    let n = if cptrs.len() > MAX_EXTRA_CAPS {
        MAX_EXTRA_CAPS
    } else {
        cptrs.len()
    };
    let mut index = 0;
    while index < n {
        write_extra_word(1 + index, cptrs[index]);
        index += 1;
    }
    write_extra_word(0, cptrs.len() as u64);
    n
}

/// **KT-309** — số extra cap đã khai (word `n` của vùng extra caps).
pub fn extra_caps() -> u64 {
    read_extra_word(0)
}

/// **KT-309** — CPTR extra cap thứ `index` (0..4) đã khai.
pub fn extra_cap(index: usize) -> u64 {
    if index >= MAX_EXTRA_CAPS {
        return 0;
    }
    read_extra_word(1 + index)
}

/// **KT-309** — khai **slot đích** trong CSpace **của chính tiến trình** cho extra cap khi nó là
/// **bên nhận** (`Recv`/`NBRecv`/`ReplyRecv`/…). `0` = "slot trống đầu tiên" (kernel quét từ ô 1).
/// Cap thứ `i` vào `dst + i`.
pub fn set_extra_caps_dst(slot: u64) {
    // SAFETY: word `dst` nằm trong cùng trang IPC buffer RW của chính tiến trình.
    unsafe {
        core::ptr::write_volatile(
            (IPC_BUFFER_VA + EXTRA_CAPS_DST_OFFSET as u64) as *mut u64,
            slot,
        )
    }
}

/// **KT-309** — slot đích đã khai (0 = "slot trống đầu tiên").
pub fn extra_caps_dst() -> u64 {
    // SAFETY: như [`set_extra_caps_dst`].
    unsafe { core::ptr::read_volatile((IPC_BUFFER_VA + EXTRA_CAPS_DST_OFFSET as u64) as *const u64) }
}

/// Đặt header về 0 rồi bật magic (chưa có window ⇒ kernel sẽ **từ chối** IPC cho tới khi
/// [`set_recv_window`] được gọi — fail-closed, không im lặng).
pub fn init_ipc_buffer() {
    let mut index = 1;
    while index < HEADER_WORDS {
        write_word(index, 0);
        index += 1;
    }
    write_word(0, HEADER_MAGIC);
}

/// Ghi **receive window** (4 CPTR) vào header và bật magic — đây là 4 ô CSpace nơi kernel cài
/// cap của message **nhận** (`spec` dòng 58). Bốn CPTR phải **khác nhau** và hợp lệ trong CSpace
/// của tiến trình; kernel kiểm lại (`tcb_set_recv_window`) và trả lỗi nếu sai.
pub fn set_recv_window(window: [u64; MAX_CAPS]) {
    let mut index = 0;
    while index < MAX_CAPS {
        write_word(W_WINDOW + index, window[index]);
        index += 1;
    }
    write_word(0, HEADER_MAGIC);
}

/// Receive window hiện tại trong header.
pub fn recv_window() -> [u64; MAX_CAPS] {
    let mut window = [0u64; MAX_CAPS];
    let mut index = 0;
    while index < MAX_CAPS {
        window[index] = read_word(W_WINDOW + index);
        index += 1;
    }
    window
}

/// Ghi CPTR nguồn cho `MessageInfo.caps` cap của message **đang gửi** (word 6..9), phần thừa
/// zero-hoá. Trả số cap đã ghi (≤ 4, biên `spec` `max_caps = 4`).
pub fn set_send_caps(caps: &[u64]) -> usize {
    let n = if caps.len() > MAX_CAPS {
        MAX_CAPS
    } else {
        caps.len()
    };
    let mut index = 0;
    while index < MAX_CAPS {
        write_word(
            W_SEND_CAPS + index,
            if index < n { caps[index] } else { 0 },
        );
        index += 1;
    }
    n
}

/// CPTR nguồn cap đang ghi trong header (word 6..9).
pub fn send_caps() -> [u64; MAX_CAPS] {
    let mut caps = [0u64; MAX_CAPS];
    let mut index = 0;
    while index < MAX_CAPS {
        caps[index] = read_word(W_SEND_CAPS + index);
        index += 1;
    }
    caps
}

/// Đóng gói `MessageInfo` 35 bit (layout `spec/abi.toml` `[message_info]`): `label` bit 0..19,
/// `caps` 20..22, `no_reply` 23, `length` 28..34.
pub const fn info(label: u32, caps: u8, no_reply: bool, length: u8) -> u64 {
    let mut value = (label as u64) & 0xF_FFFF;
    value |= ((caps as u64) & 0x7) << INFO_CAPS_SHIFT;
    if no_reply {
        value |= 1 << INFO_NO_REPLY_SHIFT;
    }
    value |= ((length as u64) & 0x7F) << INFO_LENGTH_SHIFT;
    value
}

/// `label` của một `MessageInfo` đã đóng gói.
pub const fn info_label(raw: u64) -> u32 {
    (raw & 0xF_FFFF) as u32
}

/// Số cap của một `MessageInfo` đã đóng gói.
pub const fn info_caps(raw: u64) -> u8 {
    ((raw >> INFO_CAPS_SHIFT) & 0x7) as u8
}

/// Số MR (`length`) của một `MessageInfo` đã đóng gói.
pub const fn info_length(raw: u64) -> u8 {
    ((raw >> INFO_LENGTH_SHIFT) & 0x7F) as u8
}

/// Ghi `MessageInfo` của message **vừa nhận** vào header (sau `Recv`/`NBRecv`/`ReplyRecv`), để
/// [`read_received_caps`] đọc được **số cap đã cài thật**.
pub fn note_received(raw_info: u64) {
    write_word(W_INFO, raw_info);
}

/// `MessageInfo` của message vừa nhận (0 = chưa nhận).
pub fn received_info() -> u64 {
    read_word(W_INFO)
}

/// Cap **nhận được** trong message gần nhất: `count` = `MessageInfo.caps` (kernel cập nhật theo
/// số cap **thật sự** cài vào window), `slots[..count]` = CPTR đích trong CSpace của tiến trình.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceivedCaps {
    /// Số cap nhận được (0..=4).
    pub count: usize,
    /// CPTR nơi cap được cài (`slots[..count]` có nghĩa).
    pub slots: [u64; MAX_CAPS],
}

/// Đọc cap nhận được từ header (xem [`ReceivedCaps`]).
pub fn read_received_caps() -> ReceivedCaps {
    let count = usize::from(info_caps(received_info())).min(MAX_CAPS);
    let window = recv_window();
    ReceivedCaps {
        count,
        slots: window,
    }
}

/// Gọi syscall 7 tham số; trả `[error, value, info, MR0..MR3]` theo façade hiện có.
#[inline(always)]
pub fn syscall7(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> [u64; 7] {
    let mut r0 = a0;
    let mut r1 = a1;
    let mut r2 = a2;
    let mut r3 = a3;
    let mut r4 = a4;
    let mut r5 = a5;
    let mut r6 = 0u64;
    // SAFETY: `ecall` vào kernel đã kiểm; thanh ghi a0..a6/a7 đúng ABI syscall của K3.
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") r0,
            inlateout("a1") r1,
            inlateout("a2") r2,
            inlateout("a3") r3,
            inlateout("a4") r4,
            inlateout("a5") r5,
            inlateout("a6") r6,
            in("a7") nr,
            options(nostack)
        );
    }
    [r0, r1, r2, r3, r4, r5, r6]
}

/// Gọi syscall 2 tham số (dùng cho `DebugPutStr`/`Exit` của chương trình tối giản).
#[inline(always)]
pub fn syscall2(nr: u64, a0: u64, a1: u64) -> [u64; 2] {
    let mut r0 = a0;
    let mut r1 = a1;
    // SAFETY: như [`syscall7`].
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") r0,
            inlateout("a1") r1,
            in("a7") nr,
            options(nostack)
        );
    }
    [r0, r1]
}

/// `Call(cap, MessageInfo, MR0..MR3)` — IPC trên Endpoint hoặc invocation (D2-97).
pub fn call(cap: u64, raw_info: u64, mrs: [u64; 4]) -> [u64; 7] {
    syscall7(SYS_CALL, cap, raw_info, mrs[0], mrs[1], mrs[2], mrs[3])
}

/// `NBSend(cap, MessageInfo, MR0..MR3)` — không chặn.
pub fn nb_send(cap: u64, raw_info: u64, mrs: [u64; 4]) -> [u64; 7] {
    syscall7(SYS_NB_SEND, cap, raw_info, mrs[0], mrs[1], mrs[2], mrs[3])
}

/// `Send(cap, MessageInfo, MR0..MR3)` — **KT-280**: trên cap `Reply` đây là `Reply` của seL4
/// (giao message cho caller đang `BlockedOnReply` và đánh thức nó).
pub fn send(cap: u64, raw_info: u64, mrs: [u64; 4]) -> [u64; 7] {
    syscall7(SYS_SEND, cap, raw_info, mrs[0], mrs[1], mrs[2], mrs[3])
}

/// `Recv(cap, reply_cptr?)` — `reply = `[`REPLY_NONE`] khi không có cap Reply.
pub fn recv(cap: u64, reply: u64) -> [u64; 7] {
    syscall7(SYS_RECV, cap, reply, 0, 0, 0, 0)
}

/// `NBRecv(cap)` — không chặn; `[0] = Busy` (14) khi Endpoint rỗng (KT-68). Dùng vòng
/// `NBRecv` + `Yield` thay cho `Recv` để chương trình test không bị chặn cứng.
pub fn nb_recv(cap: u64) -> [u64; 7] {
    syscall7(SYS_NB_RECV, cap, 0, 0, 0, 0, 0)
}

/// In một lát `&[u8]` của chính tiến trình (vùng đã map `U|R`) qua syscall `DebugPutStr`.
pub fn put(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let _ = syscall7(
        SYS_DEBUG_PUT_STR,
        bytes.as_ptr() as u64,
        bytes.len() as u64,
        0,
        0,
        0,
        0,
    );
}

/// Đọc 8 B tại `va` của **chính** tiến trình — dùng cho chương trình test cần chứng minh
/// Frame vừa map đọc/ghi được mà không phải viết `asm!`/`unsafe` ngoài danh sách D2-17.
///
/// # Safety nội bộ
/// `va` phải là địa chỉ **đã map và đọc được** trong VSpace của tiến trình gọi (caller chịu
/// trách nhiệm); đọc địa chỉ chưa map ⇒ fault U-mode (kernel xử lý theo chính sách của ảnh).
pub fn peek_u64(va: u64) -> u64 {
    // SAFETY: xem trên — caller bảo đảm `va` đã map trong VSpace của chính tiến trình.
    unsafe { core::ptr::read_volatile(va as *const u64) }
}

/// Ghi 8 B tại `va` của **chính** tiến trình (cùng điều kiện an toàn như [`peek_u64`]).
pub fn poke_u64(va: u64, value: u64) {
    // SAFETY: xem [`peek_u64`] — caller bảo đảm `va` đã map **ghi được** (R|W) trong VSpace.
    unsafe { core::ptr::write_volatile(va as *mut u64, value) };
}

/// `Yield` — nhường CPU theo đúng syscall 8 (dùng khi chương trình cần **giữ nguyên** trạng thái
/// sống của mình, vd. giữ message đang chờ trong hàng Endpoint; `Exit` sẽ huỷ hàng chờ — D2-25).
pub fn yield_now() -> [u64; 7] {
    syscall7(SYS_YIELD, 0, 0, 0, 0, 0, 0)
}

// ---------------------------------------------------------------------------------------------
// KT-69 — truy cập **nguyên tử** trên vùng nhớ user (ring chia sẻ)
// ---------------------------------------------------------------------------------------------

/// Đọc 8 B kiểu **Acquire** tại `va` của **chính** tiến trình (D2-101: chỉ bộ nhớ user, không
/// CSR/không lệnh đặc quyền). Dùng cho ring 1 producer/1 consumer: consumer thấy slot trước khi
/// thấy `head` tăng.
///
/// # Safety nội bộ
/// `va` phải **căn 8** và nằm trong vùng đã map `R` của tiến trình gọi; caller bảo đảm (ring
/// dùng offset hằng trong Frame đã map). Không có writer đồng thời cùng ô (SPSC).
pub fn atomic_load_acquire(va: u64) -> u64 {
    // SAFETY: xem trên — `AtomicU64` chỉ yêu cầu căn lề + vùng nhớ hợp lệ; đây là bộ nhớ user
    // của chính tiến trình, không phải CSR/physmap.
    unsafe { (*(va as *const core::sync::atomic::AtomicU64)).load(Ordering::Acquire) }
}

/// Ghi 8 B kiểu **Release** tại `va` (cùng điều kiện như [`atomic_load_acquire`]) — publish
/// `head`/`tail` sau khi slot đã ghi/đọc xong.
pub fn atomic_store_release(va: u64, value: u64) {
    // SAFETY: xem [`atomic_load_acquire`] — chỉ ghi bộ nhớ user đã map `R|W`, ô 8 B căn lề.
    unsafe {
        (*(va as *const core::sync::atomic::AtomicU64)).store(value, Ordering::Release);
    }
}

/// Đọc 8 B thường (Relaxed) tại `va` — cho word payload của slot (không phải điểm đồng bộ).
pub fn atomic_load_relaxed(va: u64) -> u64 {
    // SAFETY: như [`atomic_load_acquire`].
    unsafe { (*(va as *const core::sync::atomic::AtomicU64)).load(Ordering::Relaxed) }
}

/// Ghi 8 B thường (Relaxed) tại `va` — cho word payload của slot.
pub fn atomic_store_relaxed(va: u64, value: u64) {
    // SAFETY: như [`atomic_load_acquire`].
    unsafe {
        (*(va as *const core::sync::atomic::AtomicU64)).store(value, Ordering::Relaxed);
    }
}

/// `Exit(code)` rồi chờ kernel thu hồi — không quay lại U-mode.
pub fn exit(code: u64) -> ! {
    let _ = syscall7(SYS_EXIT, code, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}
