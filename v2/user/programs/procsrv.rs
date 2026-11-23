// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-65 — **procsrv** chạy U-mode (Rust, ELF riêng nạp như `init`).
//!
//! Giao thức (không thêm syscall; chỉ `NBRecv`/`NBSend`/`Yield`/`DebugPutStr`/`Exit`):
//!
//! - `NBRecv(fault_ep)`: nhận **message chuyển tiếp fault** do kernel dựng từ
//!   `FaultKind::Exit{code}` mà core đã ghi vào `Tcb.last_fault` (label [`FAULT_EXIT`], MR0 =
//!   pid, MR1 = mã thoát). Sau lượt `NBRecv` này core đã đưa TCB chết về `Inactive` (D2-25);
//!   **procsrv không in trạng thái** — TAP ở phía kernel đọc `Machine` để chứng minh `Inactive`
//!   (D2-93/kt65 review: không in chữ mà chương trình không đọc).
//! - `NBRecv(proc_ep)`: nhận yêu cầu `WAIT pid` / `KILL pid` từ shell (giao thức procsrv, không
//!   phải syscall), trả lời qua `reply_ep` bằng `NBSend`.
//! - `NBSend(events_ep)`: báo sự kiện cho kernel (TAP/evidence), dùng đúng kênh IPC.
//!
//! Vòng lặp là **polling + `Yield`**: ảnh K3.5 chưa có scheduler U-mode trong core, kernel đóng
//! vai scheduler tuần tự (xem `docs/k35-procsrv.md` §2); như vậy không cần syscall mới và không
//! có procsrv-in-kernel.
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

extern crate vifast_user_abi;
use vifast_user_abi::{
    exit, put, syscall7, SYS_EXIT, SYS_NB_RECV, SYS_NB_SEND, SYS_YIELD,
};

// Cspace dùng chung của ảnh demo (kernel dựng; xem `proc_demo.rs`).
const EP_FAULT: u64 = 1;
const EP_PROC: u64 = 2;
const EP_REPLY: u64 = 4;
const EP_EVENTS: u64 = 8;

/// Label của message chuyển tiếp `FaultKind::Exit` từ kernel sang procsrv.
const FAULT_EXIT: u64 = 0x608;
const WAIT_REQ: u64 = 0x604;
const KILL_REQ: u64 = 0x605;
const REPLY_WAIT: u64 = 0x606;
const REPLY_KILL: u64 = 0x607;
const EV_FAULT: u64 = 0x502;
const EV_WAIT: u64 = 0x503;
const EV_KILL: u64 = 0x504;
const EV_UP: u64 = 0x501;
/// **KT-345b** — caller thứ hai của `netsrv` (`EP_NET_CLIENT`/`EP_NET_CLIENT_REPLY` của ảnh sản
/// phẩm, khớp `proc_abi`); chỉ có trong ELF build với `--cfg product_net`.
#[cfg(product_net)]
const EP_NET_CLIENT: u64 = 26;
#[cfg(product_net)]
const EP_NET_CLIENT_REPLY: u64 = 27;
/// **KT-345b** — label `Ping(ip, handle=0)` / `Reply(rtt, err, handle)` (hợp đồng nội ảnh).
#[cfg(product_net)]
const NET_REQ: u64 = 0x290;
#[cfg(product_net)]
const NET_REPLY: u64 = 0x292;
/// Gateway slirp của QEMU (`10.0.2.2`) — dữ kiện nền tảng, không phải số đo.
#[cfg(product_net)]
const TARGET_IP: u64 = 0x0A00_0202;

const INFO_LEN_SHIFT: u64 = 28;

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

fn yield_cpu() {
    // Ghi chú: syscall 8 không tham số; trong pha đa nhiệm kernel dùng làm điểm nhường CPU.
    
    let _ = syscall7(SYS_YIELD, 0, 0, 0, 0, 0, 0);

}

/// `NBSend(endpoint, label, MR0..MR3)` — message 0 cap, tối đa 4 MR thanh ghi.
fn send(ep: u64, label: u64, mr: [u64; 4]) {
    // Luôn gửi đủ **4 MR** (kể cả 0) để bên nhận phân biệt "giá trị 0" với "không gửi".
    let words = 4u64;
    let info = label | (words << INFO_LEN_SHIFT);
    // Ghi chú: syscall 2 với cap Endpoint trong cspace của chính tiến trình.
    
    let _ = syscall7(SYS_NB_SEND, ep, info, mr[0], mr[1], mr[2], mr[3]);

}

/// `NBRecv(endpoint)` → `(error, info, MR0..MR3)`. `error != 0` ⇔ `Busy` (14).
fn recv(ep: u64) -> (u64, u64, [u64; 4]) {
    // Ghi chú: syscall 4 với cap Endpoint; kernel trả info a2 + MR a3..a6.
    let r =  syscall7(SYS_NB_RECV, ep, 0, 0, 0, 0, 0) ;
    (r[0], r[2], [r[3], r[4], r[5], r[6]])
}

/// Bảng (pid, mã thoát) — tuyến tính, tối đa 8 mục; không phụ thuộc dải `ObjId`.
/// Dùng `AtomicU64` thay `static mut` để nguồn U-mode **không cần `unsafe`** (luật R5).
static EXIT_PID: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
static EXIT_CODE: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
static EXIT_N: AtomicUsize = AtomicUsize::new(0);

fn record_exit(pid: u64, code: u64) {
    // Một tiến trình U-mode một hart; cập nhật tuần tự sau khi `Recv` fault.
    let n = EXIT_N.load(Ordering::Relaxed);
    let mut i = 0;
    while i < n {
        if EXIT_PID[i].load(Ordering::Relaxed) == pid {
            EXIT_CODE[i].store(code, Ordering::Relaxed);
            return;
        }
        i += 1;
    }
    if n < 8 {
        EXIT_PID[n].store(pid, Ordering::Relaxed);
        EXIT_CODE[n].store(code, Ordering::Relaxed);
        EXIT_N.store(n + 1, Ordering::Relaxed);
    }
}

fn exit_code_of(pid: u64) -> u64 {
    // Như `record_exit` — chỉ đọc sau khi ghi tuần tự.
    let n = EXIT_N.load(Ordering::Relaxed);
    let mut i = 0;
    while i < n {
        if EXIT_PID[i].load(Ordering::Relaxed) == pid {
            return EXIT_CODE[i].load(Ordering::Relaxed);
        }
        i += 1;
    }
    0xFFFF
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    put(b"# proc: up proto=wait/kill\n");
    send(EP_EVENTS, EV_UP, [0, 0, 0, 0]);
    // **KT-345b** — procsrv là **caller thứ hai** của `netsrv` (6A): gửi một `Ping` lúc boot trên
    // kênh riêng (`EP_NET_CLIENT`), rồi poll `EP_NET_CLIENT_REPLY` trong vòng lặp. Không có mạng
    // (ảnh không `product-net`/không NIC) ⇒ syscall trả lỗi, không in gì — không đổi ảnh test.
    #[cfg(product_net)]
    send(EP_NET_CLIENT, NET_REQ, [TARGET_IP, 0, 0, 0]);
    #[cfg(product_net)]
    let mut net_replied = false;
    loop {
        // ---------------------------------------------------------- fault (Exit của con)
        let (err, info, mr) = recv(EP_FAULT);
        if err == 0 {
            let label = info & 0xFFFFF;
            if label == FAULT_EXIT {
                let pid = mr[0];
                let code = mr[1];
                put(b"# proc: recv exit pid=");
                put_u64(pid);
                put(b" code=");
                put_u64(code);
                put(b"\n");
                record_exit(pid, code);
                send(EP_EVENTS, EV_FAULT, [pid, code, 0, 0]);
            }
        }
        // `Tcb.last_fault` bị core xoá ở **đầu** mọi lệnh IPC kế (reset_ipc_error), nên kernel
        // phải có cơ hội đọc ngay sau lượt `NBRecv` vừa thụ lý fault: nhường CPU ở đây.
        yield_cpu();
        // **KT-345b** — reply của `netsrv` cho caller thứ hai; in **giá trị nhận được**.
        #[cfg(product_net)]
        if !net_replied {
            let (net_err, net_info, net_mr) = recv(EP_NET_CLIENT_REPLY);
            if net_err == 0 && net_info & 0xFFFFF == NET_REPLY {
                net_replied = true;
                put(b"# proc: ping 10.0.2.2 rtt_ms=");
                put_u64(net_mr[0]);
                put(b" err=");
                put_u64(net_mr[1]);
                put(b" handle=");
                put_u64(net_mr[2]);
                put(b"\n");
            }
        }
        // ---------------------------------------------------------- yêu cầu shell (wait/kill)
        let (err, info, mr) = recv(EP_PROC);
        if err == 0 {
            let label = info & 0xFFFFF;
            let pid = mr[0];
            if label == WAIT_REQ {
                let code = exit_code_of(pid);
                put(b"# proc: wait pid=");
                put_u64(pid);
                put(b" code=");
                put_u64(code);
                put(b"\n");
                send(EP_REPLY, REPLY_WAIT, [pid, code, 0, 0]);
                send(EP_EVENTS, EV_WAIT, [pid, code, 0, 0]);
            } else if label == KILL_REQ {
                put(b"# proc: kill pid=");
                put_u64(pid);
                put(b" code=0\n");
                send(EP_REPLY, REPLY_KILL, [pid, 0, 0, 0]);
                send(EP_EVENTS, EV_KILL, [pid, 0, 0, 0]);
            }
        }
        yield_cpu();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // Panic U-mode là lỗi không phục hồi; thoát mã 99 để kernel đỏ.
    exit(99)
}
