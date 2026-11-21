// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-281** — `netsrv` U-mode: **net server** riscv64 (N1 phần 3).
//!
//! Server này **chỉ điều phối**: nó nhận `Ping(ip)` từ client U-mode, gọi **syscall** `Call` trên
//! `Endpoint` của **kernel** (dịch vụ mạng của kernel — driver `virtio_net` nằm trong kernel), rồi
//! `Reply(rtt)` về client. Nó **không** giữ cap `Frame`/BAR, **không** map MMIO, **không** có
//! `unsafe`: kể cả muốn chạm thiết bị cũng không có đường.
//!
//! # Vì sao `Call` trên Endpoint mà không phải một syscall `NetPing` riêng
//!
//! `spec/abi.toml` cấp đúng **16** syscall `0..15` (dải `16..31` reserved) và luật vòng này **cấm
//! sửa `spec/`**. Syscall **0 `Call`** trên cap `Endpoint` đã có đúng ngữ nghĩa cần: chặn, kernel
//! xử lý, trả `(error, value)` qua `a0/a1` — cùng khuôn dịch vụ FS của kernel (`proc_fs.rs`).
//! Xem `JOURNAL/kt281-netsrv.md` §1.2.
//!
//! Bố cục cspace (kernel dựng, xem `src/ktest_netsrv.rs`):
//!
//! | slot | cap |
//! |---|---|
//! | 0 | `CNode` của chính cspace |
//! | 1 | `Endpoint` `EP_SRV` — `Recv` request của client |
//! | 2 | `Reply` — kernel gắn `Reply.caller` khi `Call` của client tới |
//! | 3 | `Endpoint` `EP_KERN` — `Call` tới **dịch vụ mạng của kernel** |
//!
//! Không `unsafe`, không `asm!` (R5).

#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{
    call, exit, info, info_label, init_ipc_buffer, put, recv, send, set_recv_window,
};

/// Slot `Endpoint` — nơi server `Recv` request của client.
const SLOT_EP_SRV: u64 = 1;
/// Slot `Reply` — `Send` trên nó = `Reply` cho client đang `BlockedOnReply`.
const SLOT_REPLY: u64 = 2;
/// Slot `Endpoint` — dịch vụ mạng của **kernel** (driver `virtio_net` ở kernel).
const SLOT_EP_KERN: u64 = 3;

/// Label request `Ping(ip)` từ client (hợp đồng của bài test, **không** phải ABI).
const LABEL_REQ: u32 = 0x290;
/// Label request `NetPing(ip)` gửi **kernel** — **syscall thật** (KT-306: dịch vụ kernel
/// `net_svc::NetSvc` kiểm quyền rồi mới chạm driver).
const LABEL_KERN: u32 = 0x291;
/// Label reply `Reply(rtt)` gửi lại client.
const LABEL_REPLY: u32 = 0x292;
/// Label **sai** cho ca âm: dịch vụ kernel phải trả `InvalidArgument` (5) và **không** gửi gói.
const LABEL_BAD: u32 = 0x999;

/// Mã thoát khi hỏng (TAP đỏ vì khác 0).
const EXIT_RECV: u64 = 70;
const EXIT_LABEL: u64 = 71;
const EXIT_KERN: u64 = 72;
const EXIT_REPLY: u64 = 73;
const EXIT_NEG_LABEL: u64 = 74;

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

/// In số hex **không** tiền tố (dùng cho `label=0x…`).
fn put_hex(mut value: u64) {
    let mut buf = [0u8; 16];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        let digit = (value & 0xf) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        value >>= 4;
    }
    put(&buf[i..]);
}

/// In IPv4 dạng `a.b.c.d` từ mã hoá `(a<<24)|(b<<16)|(c<<8)|d` — in **đúng giá trị nhận được**
/// từ client, không dùng hằng của bài test.
fn put_ipv4(ip: u64) {
    put_u64((ip >> 24) & 0xff);
    put(b".");
    put_u64((ip >> 16) & 0xff);
    put(b".");
    put_u64((ip >> 8) & 0xff);
    put(b".");
    put_u64(ip & 0xff);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_ipc_buffer();
    // Receive window 4 ô **phân biệt** (spec dòng 58): core từ chối window trùng ô
    // (`InvalidArgument` 5) — message của bài test không mang cap nên window chỉ cần hợp lệ.
    set_recv_window([4, 5, 6, 7]);

    // ---- 1) `Recv` **chặn** trên Endpoint: kernel chạy client trước, client `Call` tới ⇒ message
    // `Ping(ip)` được giao vào vùng nhận IPC của server và server được đánh thức.
    // `answer = [error, value, MessageInfo, MR0..MR3]`.
    let request = recv(SLOT_EP_SRV, SLOT_REPLY);
    if request[0] != 0 {
        put(b"# netsrv: server recv err=");
        put_u64(request[0]);
        put(b"\n");
        exit(EXIT_RECV);
    }
    if info_label(request[2]) != LABEL_REQ {
        exit(EXIT_LABEL);
    }
    let ip = request[3];

    // ---- 2) Gọi **dịch vụ mạng của kernel** bằng syscall `Call` trên `Endpoint` — đây là chỗ
    // **duy nhất** trong toàn ảnh chạm tới mạng, và nó chỉ là một lời gọi IPC: driver
    // (`virtio_net`) chạy trong kernel, server không thấy thanh ghi nào.
    // `Call` **chặn** (`BlockedOnSend{call}`) cho tới khi kernel `reply_msg` ⇒ server được resume
    // với `a0 = error`, `a1 = length`, `a3 = MR0 = RTT (ms)`.
    let ping = call(SLOT_EP_KERN, info(LABEL_KERN, 0, false, 1), [ip, 0, 0, 0]);
    if ping[0] != 0 {
        // Kernel từ chối (không có driver / ping lỗi): **vẫn** trả lời client để nó không treo —
        // đỏ có kiểm soát, có chẩn đoán.
        put(b"# netsrv: ping err=");
        put_u64(ping[0]);
        put(b"\n");
        let _ = send(SLOT_REPLY, info(LABEL_REPLY, 0, false, 1), [ping[0], 0, 0, 0]);
        exit(EXIT_KERN);
    }
    let rtt = ping[3];
    put(b"# netsrv: ping ");
    put_ipv4(ip);
    put(b" ok rtt=");
    put_u64(rtt);
    put(b"\n");

    // ---- 3) **KT-306** — `NetPing` là **syscall thật**: in **kết quả syscall** vừa nhận. `label`
    // lấy từ `MessageInfo` của **reply** (kernel echo label request), `reply` = mã lỗi `a0`,
    // `rtt_ms` = MR0 — cả ba đều là **số đo**, không phải hằng của bài test.
    put(b"# netping: label=0x");
    put_hex(u64::from(info_label(ping[2])));
    put(b" reply=");
    put_u64(ping[0]);
    put(b" rtt_ms=");
    put_u64(rtt);
    put(b"\n");

    // ---- 4) Ca âm: **label sai** ⇒ dịch vụ kernel **kiểm quyền** rồi từ chối (`InvalidArgument`
    // 5) và **không** gửi gói nào. Đây là phép kiểm **quyền**, không phải kiểm định dạng chuỗi.
    // `Call` trả `a0 = 0` (IPC thành công) + `a1 = length` + `a3 = MR0` = **mã lỗi** của dịch vụ.
    let bad = call(SLOT_EP_KERN, info(LABEL_BAD, 0, false, 1), [ip, 0, 0, 0]);
    if bad[0] != 0 || bad[3] != 5 {
        put(b"# netping: neg label err=");
        put_u64(bad[3]);
        put(b"\n");
        exit(EXIT_NEG_LABEL);
    }
    put(b"# netping: neg label err=5 ok=1\n");

    // ---- 5) `Send` trên cap `Reply` = `Reply` của seL4: giao `Reply(rtt)` cho client đang
    // `BlockedOnReply` và đánh thức nó.
    let reply = send(SLOT_REPLY, info(LABEL_REPLY, 0, false, 1), [rtt, 0, 0, 0]);
    if reply[0] != 0 {
        exit(EXIT_REPLY);
    }

    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
