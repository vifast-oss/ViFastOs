// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342** — `netsrv` U-mode của **ảnh sản phẩm** riscv64 (`product-net`).
//!
//! Cùng vai trò với `init/netsrv_user.rs` của ảnh test `ktest-netsrv` (**KT-281**): server **chỉ
//! điều phối** — nhận `Ping(ip)` từ shell U-mode, gọi **dịch vụ mạng của kernel** bằng syscall
//! `Call` trên `Endpoint` (driver `virtio_net` nằm trong kernel), rồi gửi `Reply(rtt)` về shell.
//! Server **không** giữ cap `Frame`/BAR, **không** map MMIO, **không** có `unsafe`.
//!
//! # Khác ảnh test — chỉ ở **bố cục cspace** và **kiểu trả lời**
//!
//! Ảnh sản phẩm dùng **một** cspace gốc cho shell + procsrv và **một** cnode riêng cho server
//! (kernel dựng trong `proc_rt::build_rig`), nên slot ở đây là:
//!
//! | slot | cap | dùng |
//! |---|---|---|
//! | 0 | `CNode` của chính cspace | — |
//! | 1 | `Endpoint` `EP_NET` (`proc_abi::EP_NET`) | `Recv` request `ping` của shell |
//! | 2 | `Endpoint` `EP_NET_REPLY` (`proc_abi::EP_NET_REPLY`) | `NBSend` reply về shell |
//! | 3 | `Endpoint` `EP_NET_KERN` (`proc_abi::EP_NET_KERN`) | `Call` dịch vụ mạng của kernel |
//! | 4 | `Endpoint` `EP_FAULT` | core ghi `FaultKind` (không in gì) |
//!
//! Reply dùng **`NBSend` trên Endpoint** (không phải `Reply` cap) vì shell của ảnh sản phẩm là
//! chương trình **polling** (`NBRecv` + `Yield`, xem `init/shell_user.rs`) — cùng khuôn dịch vụ FS
//! của `proc_fs.rs`/`proc_rt.rs` (KT-77c), không mở ABI mới.
//!
//! Một request là đủ cho evidence của `KT-342` ("netsrv trả lời ≥ 1 yêu cầu"); sau khi trả lời,
//! server `Exit(0)` — kernel coi đó là kết thúc bình thường của tác vụ phụ.

#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{
    call, exit, info, info_label, init_ipc_buffer, nb_send, put, recv, set_recv_window, REPLY_NONE,
};

/// `Endpoint` `EP_NET` — nơi server `Recv` request `Ping(ip)` của shell.
const SLOT_EP_NET: u64 = 1;
/// `Endpoint` `EP_NET_REPLY` — `NBSend` reply về shell (shell `NBRecv` ô này).
const SLOT_EP_NET_REPLY: u64 = 2;
/// `Endpoint` `EP_NET_KERN` — dịch vụ mạng của **kernel** (`net_svc::NetSvc`).
const SLOT_EP_KERN: u64 = 3;

/// Label request `Ping(ip)` từ shell (hợp đồng nội ảnh, **không** phải ABI).
const LABEL_REQ: u32 = 0x290;
/// Label request `NetPing(ip)` gửi **kernel** — **syscall thật** (KT-306).
const LABEL_KERN: u32 = 0x291;
/// Label reply `Reply(rtt)` gửi lại shell.
const LABEL_REPLY: u32 = 0x292;

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

/// In IPv4 dạng `a.b.c.d` từ mã hoá `(a<<24)|(b<<16)|(c<<8)|d` — in **đúng giá trị nhận được**.
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
    // Window 4 ô **phân biệt** (`spec` dòng 58) — message không mang cap nên chỉ cần hợp lệ.
    set_recv_window([4, 5, 6, 7]);

    // 1) `Recv` **chặn** trên `EP_NET`: shell `NBSend` `Ping(ip)` tới ⇒ message vào vùng nhận IPC
    //    của server và server được đánh thức. `request = [error, value, MessageInfo, MR0..MR3]`.
    let request = recv(SLOT_EP_NET, REPLY_NONE);
    if request[0] != 0 {
        put(b"# netsrv: recv err=");
        put_u64(request[0]);
        put(b"\n");
        exit(70)
    }
    if info_label(request[2]) != LABEL_REQ {
        put(b"# netsrv: label la\n");
        exit(71)
    }
    let ip = request[3];

    // 2) Gọi **dịch vụ mạng của kernel** — chỗ **duy nhất** trong toàn ảnh chạm tới mạng, và nó
    //    chỉ là một lời gọi IPC: driver (`virtio_net`) chạy trong kernel, server không thấy MMIO.
    //    `Call` **chặn** cho tới khi `NetSvc` `reply_msg` ⇒ resume với `a3 = MR0` = RTT (ms) khi
    //    thành công, hoặc mã lỗi khi dịch vụ từ chối (KT-306 giữ nguyên hợp đồng).
    let ping = call(SLOT_EP_KERN, info(LABEL_KERN, 0, false, 1), [ip, 0, 0, 0]);
    // `ping[0]` = lỗi syscall `Call` (0 = IPC thành công); `ping[3]` = MR0 của reply = RTT (ms)
    // khi `NetSvc` phục vụ thành công (KT-306 giữ nguyên hợp đồng: từ chối thì MR0 = mã lỗi).
    let err = ping[0];
    let rtt = if err == 0 { ping[3] } else { 0 };

    // 3) Bằng chứng của server (ảnh sản phẩm không có TAP): một dòng, in **giá trị nhận được**.
    put(b"# netsrv: ping ");
    put_ipv4(ip);
    put(b" rtt_ms=");
    put_u64(rtt);
    put(b" err=");
    put_u64(err);
    put(b"\n");

    // 4) **KT-342b** — trả lời shell **trực tiếp** U-mode→U-mode trên `EP_NET_REPLY`
    //    (`NBSend`, không chặn): shell là chương trình polling nên không cần `Reply` cap, và
    //    kernel **không** chuyển tiếp kết quả nữa. Message `NBSend` **đã trả về** thuộc về bên
    //    nhận ⇒ nó **không** bị rút lại khi server `Exit` ngay sau đó (lỗi KT-342: core gỡ mọi
    //    message của TCB chết; sửa ở `unlink_tcb_queues`, xem `nb_queue_tests.rs`).
    let rep = nb_send(
        SLOT_EP_NET_REPLY,
        info(LABEL_REPLY, 0, false, 2),
        [rtt, err, 0, 0],
    );
    put(b"# netsrv: reply nb err=");
    put_u64(rep[0]);
    put(b"\n");
    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
