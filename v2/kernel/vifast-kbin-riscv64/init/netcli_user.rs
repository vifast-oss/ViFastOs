// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-281** — `netcli` U-mode: client của **net server U-mode**.
//!
//! Gọi `Call` **chặn** tới server U-mode `netsrv` với `Ping(ip)` (label + MR0 = IP), rồi nhận
//! `Reply(rtt)` và in `# netsrv: client got reply=<v>`. Đây là **nửa client** của N1 phần 3: toàn
//! bộ đường đi là IPC U-mode → U-mode (KT-280) **cộng** một lần server gọi kernel driver; client
//! **không** giữ cap thiết bị nào, **không** map MMIO.
//!
//! Bố cục cspace (kernel dựng, xem `src/ktest_netsrv.rs`):
//!
//! | slot | cap |
//! |---|---|
//! | 0 | `CNode` của chính cspace |
//! | 1 | `Endpoint` `EP_SRV` (gửi `Call` tới server) |
//! | 2 | `Reply` (client **không** dùng — ca âm "Call trên cap không phải Endpoint") |
//! | 9 | **rỗng** — ca âm "Call trên ô rỗng" |
//!
//! Không `unsafe`, không `asm!` (R5): mọi `ecall` qua shim `vifast_user_abi`.

#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{call, exit, info, info_label, init_ipc_buffer, put, set_recv_window};

/// Slot `Endpoint` — nơi client gửi `Call` tới server.
const SLOT_EP_SRV: u64 = 1;
/// Slot `Reply` — **không** phải Endpoint: `Call` trên nó phải trả lỗi (ca âm).
const SLOT_REPLY: u64 = 2;
/// Slot **rỗng** — `Call` trên ô rỗng phải trả `InvalidCapability` (1) (ca âm).
const SLOT_EMPTY: u64 = 9;
/// **KT-306** — bản cap `EP_KERN` của **dịch vụ kernel** `NetPing`. Client **không** phải net
/// server ⇒ request **hợp lệ** vẫn bị từ chối `InsufficientRights` (3): đây là phép kiểm **quyền**
/// của dịch vụ (không phải kiểm định dạng chuỗi).
const SLOT_EP_KERN: u64 = 3;
/// Label `NetPing` **hợp lệ** — dùng cho ca âm quyền (chỉ **caller** sai).
const LABEL_NETPING: u32 = 0x291;

/// Label request `Ping(ip)` (hợp đồng giữa hai chương trình, **không** phải ABI).
const LABEL_REQ: u32 = 0x290;
/// Label reply `Reply(rtt)` mà server gửi lại.
const LABEL_REPLY: u32 = 0x292;

/// IP cần ping, mã hoá `(a<<24)|(b<<16)|(c<<8)|d` — gateway slirp `10.0.2.2` (dữ kiện nền tảng
/// QEMU, không phải số đo).
const TARGET_IP: u64 = 0x0A00_0202;

/// Mã thoát khi hỏng (TAP đỏ vì khác 0).
const EXIT_CALL: u64 = 80;
const EXIT_LABEL: u64 = 81;
const EXIT_VALUE: u64 = 82;
const EXIT_NEG_EMPTY: u64 = 84;
const EXIT_NEG_REPLY: u64 = 85;
const EXIT_NEG_CALLER: u64 = 86;

/// In số thập phân (không cấp phát, không `unsafe`) — cùng khuôn `ipc_client_user.rs`.
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

    // ---- 1) `Call` **chặn** trên Endpoint: kernel chạy server U-mode (server gọi kernel driver
    // ping) rồi resume ta với `Reply(rtt)`.
    // `answer = [error, value, MessageInfo, MR0..MR3]`.
    let answer = call(SLOT_EP_SRV, info(LABEL_REQ, 0, false, 1), [TARGET_IP, 0, 0, 0]);
    if answer[0] != 0 {
        put(b"# netsrv: client call err=");
        put_u64(answer[0]);
        put(b"\n");
        exit(EXIT_CALL);
    }
    // `a2` = MessageInfo của **reply** (label + length), `a3` = MR0 = RTT (ms).
    if info_label(answer[2]) != LABEL_REPLY {
        exit(EXIT_LABEL);
    }
    let rtt = answer[3];
    // RTT 0 ms là **hợp lệ** dưới `-icount shift=6` (đồng hồ ảo) — điều kiện là **có** reply,
    // không phải reply khác 0. Giá trị chỉ để in; TAP kiểm ở kernel.
    if answer[1] != 1 {
        exit(EXIT_VALUE);
    }
    put(b"# netsrv: client got reply=");
    put_u64(rtt);
    put(b"\n");

    // ---- 2) Ca âm (a): `Call` trên ô **rỗng** ⇒ `InvalidCapability` (1), không chặn.
    let empty = call(SLOT_EMPTY, info(LABEL_REQ, 0, false, 1), [TARGET_IP, 0, 0, 0]);
    if empty[0] != 1 {
        exit(EXIT_NEG_EMPTY);
    }
    // ---- 3) Ca âm (b): `Call` trên cap **Reply** (không phải Endpoint) ⇒ invocation không có
    // method ⇒ `NotSupported` (18), không chặn, không gửi message nào.
    let wrong = call(SLOT_REPLY, info(LABEL_REQ, 0, false, 1), [TARGET_IP, 0, 0, 0]);
    if wrong[0] != 18 {
        exit(EXIT_NEG_REPLY);
    }
    put(b"# netsrv: neg empty=");
    put_u64(empty[0]);
    put(b" reply=");
    put_u64(wrong[0]);
    put(b" ok=1\n");

    // ---- 4) **KT-306** ca âm **quyền**: client giữ cap `EP_KERN` và gửi request **hợp lệ**
    // (`label = 0x291`, `MR0 = ip`) nhưng **không phải** net server ⇒ dịch vụ kernel trả
    // `InsufficientRights` (3) trong `MR0` và **không** gửi gói nào. Cap một mình **không đủ**.
    let rogue = call(
        SLOT_EP_KERN,
        info(LABEL_NETPING, 0, false, 1),
        [TARGET_IP, 0, 0, 0],
    );
    if rogue[0] != 0 || rogue[3] != 3 {
        put(b"# netping: neg caller err=");
        put_u64(rogue[3]);
        put(b"\n");
        exit(EXIT_NEG_CALLER);
    }
    put(b"# netping: neg caller err=3 ok=1\n");

    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
