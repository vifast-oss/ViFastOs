// SPDX-License-Identifier: MIT OR Apache-2.0
//! `shell_net.rs` — nhánh **mạng của shell** (`#[cfg(shell_net)]`, KT-342): `ping <ip>` qua
//! `netsrv` U-mode (`EP_NET`, `NET_REQ`/`NET_REPLY`) + in/phân tích IPv4; tách cơ học khỏi
//! `shell.rs` (KT-401, < 950 dòng). Chỉ là IPC, không mở ABI.

#![allow(unused_imports)] // glob lấy helper/cfg từ tệp cha — không phải mọi mục đều dùng.

use super::*;

/// **KT-342** — in IPv4 `a.b.c.d` từ mã hoá `(a<<24)|(b<<16)|(c<<8)|d` (chỉ `shell_net`).
#[cfg(shell_net)]
fn put_ipv4(ip: u64) {
    put_u64((ip >> 24) & 0xff);
    put(b".");
    put_u64((ip >> 16) & 0xff);
    put(b".");
    put_u64((ip >> 8) & 0xff);
    put(b".");
    put_u64(ip & 0xff);
}

/// **KT-342** — parse `a.b.c.d` thành `(a<<24)|(b<<16)|(c<<8)|d`; sai ⇒ `None`.
#[cfg(shell_net)]
fn parse_ipv4(bytes: &[u8]) -> Option<u64> {
    let mut value = 0u64;
    let mut octets = 0u32;
    let mut current: Option<u64> = None;
    for &b in bytes {
        if b == b'.' {
            let part = current?;
            if octets >= 3 {
                return None;
            }
            value = (value << 8) | part;
            octets += 1;
            current = None;
        } else if b.is_ascii_digit() {
            let digit = u64::from(b - b'0');
            let part = current.unwrap_or(0) * 10 + digit;
            if part > 255 {
                return None;
            }
            current = Some(part);
        } else {
            return None;
        }
    }
    let last = current?;
    if octets != 3 {
        return None;
    }
    Some((value << 8) | last)
}

/// `ping <ip>` — **KT-342**: gửi `Ping(ip)` tới `netsrv` U-mode rồi chờ reply. Đây là đường
/// **duy nhất** shell chạm mạng, và nó chỉ là IPC — driver `virtio_net` chạy trong kernel.
#[cfg(shell_net)]
pub(super) fn cmd_ping(arg: &[u8]) {
    let Some(ip) = parse_ipv4(arg) else {
        put(b"# shell: ping ip khong hop le\n");
        return;
    };
    send(EP_NET, NET_REQ, [ip, 0, 0, 0]);
    let mut budget = NET_REPLY_BUDGET;
    loop {
        if budget == 0 {
            put(b"# shell: ping timeout\n");
            return;
        }
        budget -= 1;
        // **KT-342b** — reply đến **trực tiếp** từ `netsrv` U-mode trên `EP_NET_REPLY`
        // (`NBSend`), không đi qua `EP_REPLY` của kernel.
        let (err, info, mr) = recv(EP_NET_REPLY);
        if err == 0 && info & 0xFFFFF != NET_REPLY {
            // Chẩn đoán: message đến nhưng sai nhãn (đỏ thật ⇒ in ra, không nuốt).
            put(b"# shell: ping msg info=0x");
            put_hex(info);
            put(b"\n");
        }
        if err == 0 && info & 0xFFFFF == NET_REPLY {
            put(b"# shell: ping ");
            put_ipv4(ip);
            put(b" rtt_ms=");
            put_u64(mr[0]);
            put(b" err=");
            put_u64(mr[1]);
            put(b"\n");
            return;
        }
        yield_cpu();
    }
}
