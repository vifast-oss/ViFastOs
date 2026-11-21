// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-280** — `ipc_server` U-mode: `Recv` message của client U-mode rồi `Send` trên cap `Reply`.
//!
//! Đây là **server U-mode thật** đầu tiên của riscv64: trước KT-280 kernel luôn là bên phục vụ
//! (`procsrv`/`shell` chỉ nói chuyện với kernel qua `NBSend`/`NBRecv`). Server này chạy trong
//! VSpace riêng, cspace riêng, TCB riêng; nó **chặn** ở `Recv` cho tới khi client `Call` tới.
//!
//! Bố cục cspace (kernel dựng, xem `src/ktest_ipc.rs`):
//!
//! | slot | cap |
//! |---|---|
//! | 0 | `CNode` của chính cspace |
//! | 1 | `Endpoint` `EP` (nhận `Recv`) |
//! | 2 | `Reply` — kernel gắn `Reply.caller` khi message `Call` tới |
//! | 3 | `VSpace` của chính server (không dùng — giữ bố cục đối xứng với client) |
//!
//! Không `unsafe`, không `asm!` (R5).
#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{exit, info, init_ipc_buffer, put, set_recv_window, recv, send};

/// Slot `Endpoint` — nơi server `Recv`.
const SLOT_EP: u64 = 1;
/// Slot `Reply` — kernel đặt cap `Reply` ở đây; `Send` trên nó = `Reply` cho caller.
const SLOT_REPLY: u64 = 2;

/// Label request mà client gửi (hợp đồng của bài test, không phải ABI).
const LABEL_REQ: u32 = 0x280;
/// Label reply server gửi lại.
const LABEL_REPLY: u32 = 0x281;
/// MR0 của request — server kiểm đúng giá trị client gửi.
const ARG: u64 = 0x1234;
/// MR0 của reply.
const REPLY_VALUE: u64 = 0x5a5a;

/// Mã thoát khi hỏng (TAP đỏ vì khác 0).
const EXIT_RECV: u64 = 70;
const EXIT_LABEL: u64 = 71;
const EXIT_ARG: u64 = 72;
const EXIT_REPLY: u64 = 73;

/// In số thập phân (không cấp phát, không `unsafe`).
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

    // ---- 1) `Recv` **chặn** trên Endpoint: kernel chạy client trước, client `Call` tới ⇒
    // message được giao vào vùng nhận IPC của server và server được đánh thức.
    // `answer = [error, value, MessageInfo, MR0..MR3]`.
    let answer = recv(SLOT_EP, SLOT_REPLY);
    if answer[0] != 0 {
        put(b"# ipc: server recv err=");
        put_u64(answer[0]);
        put(b"\n");
        exit(EXIT_RECV);
    }
    if vifast_user_abi::info_label(answer[2]) != LABEL_REQ {
        exit(EXIT_LABEL);
    }
    if answer[3] != ARG {
        exit(EXIT_ARG);
    }

    // ---- 2) `Yield` — điểm nhường CPU của scheduler: kernel quan sát được trạng thái
    // `BlockedOnReply` của client **giữa** `Recv` và `Reply` (bằng chứng trực tiếp của hợp đồng
    // seL4: caller chặn ở Reply, không phải ở Send), rồi resume server để trả lời.
    let _ = vifast_user_abi::yield_now();

    // ---- 3) `Send` trên cap `Reply` = `Reply` của seL4: giao message cho caller đang
    // `BlockedOnReply` và đánh thức nó. MR1 = số request đã phục vụ.
    let reply = send(SLOT_REPLY, info(LABEL_REPLY, 0, false, 2), [REPLY_VALUE, 1, 0, 0]);
    if reply[0] != 0 {
        exit(EXIT_REPLY);
    }
    put(b"# ipc: server ran nr=");
    put_u64(1);
    put(b"\n");

    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
