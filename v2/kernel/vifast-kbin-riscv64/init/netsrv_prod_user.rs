// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-342 / KT-345b** — `netsrv` U-mode của **ảnh sản phẩm** riscv64 (`product-net`).
//!
//! Cùng vai trò với `init/netsrv_user.rs` của ảnh test `ktest-netsrv` (**KT-281**): server **chỉ
//! điều phối** — nhận request của client U-mode, gọi **dịch vụ mạng của kernel** bằng syscall
//! `Call` trên `Endpoint` (driver `virtio_net` nằm trong kernel), rồi gửi reply về client. Server
//! **không** giữ cap `Frame`/BAR, **không** map MMIO, **không** có `unsafe`.
//!
//! # **KT-345b** — đa caller (D2-242 nhóm 6A, `spec/protocols/socket.vidl`)
//!
//! Server phục vụ **≥ 2 caller** bằng **hai** cặp endpoint, và phân biệt caller bằng **danh tính
//! endpoint** (thay `badge` của seL4 — ảnh sản phẩm chưa mint badge cho client):
//!
//! | kênh | caller | request | reply |
//! |---|---|---|---|
//! | 1 | `shell` U-mode | `EP_NET` (slot 1) | `EP_NET_REPLY` (slot 2) |
//! | 2 | `procsrv` U-mode | `EP_NET_CLIENT` (slot 5) | `EP_NET_CLIENT_REPLY` (slot 6) |
//!
//! **Bảng phiên có biên** (`SESSIONS = 4`), mỗi phiên khoá theo `(client, handle)`:
//! - `handle = 0` ⇒ **mở hoặc dùng lại** phiên của caller (cấp `handle` một lần, đơn điệu tăng);
//! - bảng **đầy** ⇒ reply `Busy` (14), **không** gọi driver (fail-closed);
//! - `handle` khác 0 mà **không thuộc** caller ⇒ reply `InvalidCapability` (1) — không lộ oracle
//!   "handle có tồn tại hay không", đúng tinh thần 6A.
//!
//! Server giữ **duy nhất** một "khe" gọi kernel `NetSvc` tại một thời điểm (kernel `NetSvc` vẫn
//! **đơn caller** — chỉ TCB `netsrv`), nhưng hàng request của **hai** endpoint là khác nhau nên
//! hai caller không giành message của nhau; vòng lặp phục vụ cả hai cho tới khi hết request mới
//! `Yield` — **không** thoát sau một request (nợ của KT-342, đóng ở đây).
//!
//! Đột biến `mut_net_one_caller` (feature `mut-net-one-caller` của crate ảnh) thoát sau reply đầu
//! ⇒ caller thứ hai không bao giờ được phục vụ ⇒ ca đa caller của `product-riscv64.sh` **đỏ**.

#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{
    call, exit, info, info_label, init_ipc_buffer, nb_send, put, set_recv_window, syscall7,
    SYS_NB_RECV, SYS_YIELD,
};

/// `Endpoint` `EP_NET` — request của `shell`.
const SLOT_EP_NET: u64 = 1;
/// `Endpoint` `EP_NET_REPLY` — reply cho `shell`.
const SLOT_EP_NET_REPLY: u64 = 2;
/// `Endpoint` `EP_NET_KERN` — dịch vụ mạng của **kernel** (`net_svc::NetSvc`).
const SLOT_EP_KERN: u64 = 3;
/// **KT-345b** — request của caller thứ hai (`procsrv`).
const SLOT_EP_CLIENT: u64 = 5;
/// **KT-345b** — reply cho caller thứ hai.
const SLOT_EP_CLIENT_REPLY: u64 = 6;

/// Label request `Ping(ip, handle)` từ client (hợp đồng nội ảnh, **không** phải ABI).
const LABEL_REQ: u32 = 0x290;
/// Label request `NetPing(ip)` gửi **kernel** — **syscall thật** (KT-306).
const LABEL_KERN: u32 = 0x291;
/// Label reply `Reply(rtt, err, handle)` gửi lại client.
const LABEL_REPLY: u32 = 0x292;

/// Số phiên tối đa (biên cứng — đầy ⇒ `Busy`/14).
const SESSIONS: usize = 4;
/// Mã lỗi trả client: `InvalidCapability` (1) — handle không thuộc caller.
const ERR_INVALID: u64 = 1;
/// Mã lỗi trả client: `Busy` (14) — bảng phiên đầy.
const ERR_BUSY: u64 = 14;

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

fn yield_cpu() {
    // Syscall 8 không tham số — điểm nhường CPU giữa hai vòng polling.
    let _ = syscall7(SYS_YIELD, 0, 0, 0, 0, 0, 0);
}

/// `NBRecv` **không chặn** — trả `None` khi hàng rỗng (khác `recv` chặn của KT-342).
fn nb_recv(ep: u64) -> Option<(u64, [u64; 4])> {
    let r = syscall7(SYS_NB_RECV, ep, 0, 0, 0, 0, 0);
    if r[0] != 0 {
        return None;
    }
    Some((r[2], [r[3], r[4], r[5], r[6]]))
}

/// Khe phiên: `(client, handle)`; `client = 0` ⇒ ô trống.
struct Table {
    client: [u64; SESSIONS],
    handle: [u64; SESSIONS],
    next: u64,
}

impl Table {
    /// Mở hoặc dùng lại phiên của `client`. `Err(code)` = từ chối (đầy / handle lạ).
    fn resolve(&mut self, client: u64, handle: u64) -> Result<u64, u64> {
        if handle != 0 {
            let mut i = 0;
            while i < SESSIONS {
                if self.client[i] == client && self.handle[i] == handle {
                    return Ok(handle);
                }
                i += 1;
            }
            return Err(ERR_INVALID);
        }
        let mut free = SESSIONS;
        let mut i = 0;
        while i < SESSIONS {
            if self.client[i] == client {
                return Ok(self.handle[i]);
            }
            if self.client[i] == 0 && free == SESSIONS {
                free = i;
            }
            i += 1;
        }
        if free == SESSIONS {
            return Err(ERR_BUSY);
        }
        self.client[free] = client;
        self.handle[free] = self.next;
        self.next += 1;
        Ok(self.handle[free])
    }
}

/// Phục vụ **mọi** request đang chờ trên cả hai kênh. Trả `false` ⇔ không có request nào.
fn serve(table: &mut Table) -> bool {
    let mut any = false;
    for (slot, reply_slot, client) in [
        (SLOT_EP_NET, SLOT_EP_NET_REPLY, 1u64),
        (SLOT_EP_CLIENT, SLOT_EP_CLIENT_REPLY, 2),
    ] {
        while let Some((raw, mr)) = nb_recv(slot) {
            let label = info_label(raw);
            if label != LABEL_REQ {
                put(b"# netsrv: label la\n");
                exit(71)
            }
            any = true;
            let ip = mr[0];
            let handle = mr[1];
            let (rtt, err, handle) = match table.resolve(client, handle) {
                Ok(h) => {
                    // Gọi **dịch vụ mạng của kernel** — chỗ duy nhất trong ảnh chạm mạng; chỉ là
                    // một lời gọi IPC, driver `virtio_net` chạy trong kernel.
                    let ping = call(SLOT_EP_KERN, info(LABEL_KERN, 0, false, 1), [ip, 0, 0, 0]);
                    let err = ping[0];
                    let rtt = if err == 0 { ping[3] } else { 0 };
                    (rtt, err, h)
                }
                // Fail-closed: **không** gọi driver khi phiên không hợp lệ/đầy.
                Err(code) => (0, code, 0),
            };
            put(b"# netsrv: ping ");
            put_ipv4(ip);
            put(b" rtt_ms=");
            put_u64(rtt);
            put(b" err=");
            put_u64(err);
            put(b" handle=");
            put_u64(handle);
            put(b" client=");
            put_u64(client);
            put(b"\n");
            let rep = nb_send(
                reply_slot,
                info(LABEL_REPLY, 0, false, 3),
                [rtt, err, handle, 0],
            );
            put(b"# netsrv: reply nb err=");
            put_u64(rep[0]);
            put(b"\n");
            // Đột biến (ii): chỉ phục vụ **một** caller rồi thoát ⇒ caller thứ hai đói reply.
            #[cfg(mut_net_one_caller)]
            exit(0)
        }
    }
    any
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_ipc_buffer();
    // Window 4 ô **phân biệt** (`spec` dòng 58) — message không mang cap nên chỉ cần hợp lệ.
    set_recv_window([7, 8, 9, 10]);
    let mut table = Table {
        client: [0; SESSIONS],
        handle: [0; SESSIONS],
        next: 1,
    };
    loop {
        if !serve(&mut table) {
            yield_cpu();
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
