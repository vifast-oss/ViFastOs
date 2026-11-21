// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-280** — `ipc_client` U-mode: gọi `Call` **chặn** tới server U-mode rồi nhận reply.
//!
//! Đây là mắt xích mà K3 chưa có: trước KT-280 mọi `call()` của chương trình U-mode đều nhắm cap
//! **không phải Endpoint** (invocation `Frame.Map`/`Untyped.Retype`/`Tcb.SetTls`/`CNode.*`) nên
//! trả về ngay; IPC U-mode hiện có là `NBSend`+`NBRecv` polling. Ở đây client gọi `Call` trên
//! **Endpoint** ⇒ core đưa TCB sang `BlockedOnReply` ⇒ kernel chạy **server U-mode** ⇒ server
//! `Recv` + `Send` trên cap `Reply` ⇒ client được **resume** với `a0..a6` lấy từ vùng nhận IPC.
//!
//! Bố cục cspace (kernel dựng, xem `src/ktest_ipc.rs`):
//!
//! | slot | cap |
//! |---|---|
//! | 0 | `CNode` của chính cspace |
//! | 1 | `Endpoint` `EP` (gửi `Call`) |
//! | 2 | `Reply` (client **không** dùng — ca âm "Call trên cap không phải Endpoint") |
//! | 3 | `VSpace` của chính client (không dùng — giữ bố cục đối xứng với server) |
//!
//! Không `unsafe`, không `asm!` (R5): mọi `ecall` qua shim `vifast_user_abi`.
#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{call, exit, info, init_ipc_buffer, put, set_recv_window};

/// Slot `Endpoint` — nơi client gửi `Call`.
const SLOT_EP: u64 = 1;
/// Slot `Reply` — **không** phải Endpoint: `Call` trên nó phải trả lỗi (ca âm).
const SLOT_REPLY: u64 = 2;
/// Slot **rỗng** — `Call` trên ô rỗng phải trả `InvalidCapability` (1) (ca âm).
const SLOT_EMPTY: u64 = 9;

/// Label request (không phải ABI — hợp đồng giữa client và server của bài test).
const LABEL_REQ: u32 = 0x280;
/// Label reply mà server gửi lại.
const LABEL_REPLY: u32 = 0x281;
/// MR0 của request — server phải thấy đúng giá trị này.
const ARG: u64 = 0x1234;
/// MR0 của reply — client phải nhận đúng giá trị này.
const REPLY_VALUE: u64 = 0x5a5a;

/// Mã thoát khi hỏng (TAP đỏ vì khác 0).
const EXIT_CALL: u64 = 80;
const EXIT_LABEL: u64 = 81;
const EXIT_VALUE: u64 = 82;
const EXIT_NR: u64 = 83;
const EXIT_NEG_EMPTY: u64 = 84;
const EXIT_NEG_REPLY: u64 = 85;

/// In số thập phân (không cấp phát, không `unsafe`) — cùng khuôn `con_client_user.rs`.
fn put_u64(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    put(&buf[i..]);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_ipc_buffer();
    // Receive window 4 ô **phân biệt** (spec dòng 58): core từ chối window trùng ô
    // (`InvalidArgument` 5) — message của bài test không mang cap nên window chỉ cần hợp lệ.
    set_recv_window([4, 5, 6, 7]);

    // ---- 1) `Call` **chặn** trên Endpoint: kernel chuyển sang server U-mode rồi resume ta.
    // `answer = [error, value, MessageInfo, MR0..MR3]`.
    let answer = call(SLOT_EP, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if answer[0] != 0 {
        put(b"# ipc: client call err=");
        put_u64(answer[0]);
        put(b"\n");
        exit(EXIT_CALL);
    }
    // `a2` = MessageInfo của **reply** (label + length), `a3` = MR0, `a4` = MR1.
    if vifast_user_abi::info_label(answer[2]) != LABEL_REPLY {
        exit(EXIT_LABEL);
    }
    if answer[3] != REPLY_VALUE {
        exit(EXIT_VALUE);
    }
    // MR1 = số request server đã phục vụ (phải là 1 — server chạy đúng một lượt).
    if answer[4] != 1 {
        exit(EXIT_NR);
    }
    put(b"# ipc: umode call ok reply=");
    put_u64(answer[3]);
    put(b"\n");

    // ---- 2) Ca âm (a): `Call` trên ô **rỗng** ⇒ `InvalidCapability` (1), không chặn.
    let empty = call(SLOT_EMPTY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if empty[0] != 1 {
        exit(EXIT_NEG_EMPTY);
    }
    // ---- 3) Ca âm (b): `Call` trên cap **Reply** (không phải Endpoint) ⇒ invocation không có
    // method ⇒ `NotSupported` (18), không chặn, không gửi message nào.
    let wrong = call(SLOT_REPLY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if wrong[0] != 18 {
        exit(EXIT_NEG_REPLY);
    }
    put(b"# ipc: neg empty=");
    put_u64(empty[0]);
    put(b" reply=");
    put_u64(wrong[0]);
    put(b" ok=1\n");

    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
