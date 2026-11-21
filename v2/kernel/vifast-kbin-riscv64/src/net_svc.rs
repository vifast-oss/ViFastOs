// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-306** — `NetPing` là **syscall thật**: dịch vụ mạng của kernel trên một `Endpoint`.
//!
//! # Vì sao "syscall thật" mà **không** thêm số syscall
//!
//! `spec/abi.toml` cấp đúng **16** syscall `0..15`; dải `16..31` là `[syscall_reserved]` và luật
//! vòng này **cấm sửa `spec/`** (D2-04). Nhưng syscall **0 `Call`** trên cap `Endpoint` đã có
//! đúng ngữ nghĩa cần: chương trình U-mode `ecall` ⇒ kernel chặn/kiểm/xử lý ⇒ trả `(error, value)`
//! qua `a0/a1`. Vậy `NetPing` **là** một syscall từ U-mode; điều còn thiếu trước KT-306 là chỗ
//! **cài đặt** của nó: `virtio_net::ping` được gọi **trực tiếp trong rig** `ktest_netsrv` (mã
//! harness), không phải một **dịch vụ kernel** có kiểm quyền. KT-306 đưa nó vào module này —
//! cùng khuôn dịch vụ FS của kernel (`proc_fs::FsSvc` + `proc_rt::service`).
//!
//! # Hợp đồng (không phải ABI — label/MR là thoả thuận giữa hai chương trình)
//!
//! ```text
//! server U-mode (netsrv)                         kernel (dịch vụ NetPing)
//!   Call(EP_KERN, {label=0x291,length=1}, MR0=ip) ──►  recv(EP_KERN, REPLY)
//!     [BlockedOnSend{call}]                            kiểm quyền (caller/label/length/IP)
//!                                                      virtio_net::ping(...)  ← chỗ DUY NHẤT chạm thiết bị
//!   resume: a0=error, a1=1, a2=MessageInfo, a3=rtt ◄──  reply_msg(REPLY, {label=0x291}, MR0=rtt)
//! ```
//!
//! # Kiểm quyền (fail-closed — từ chối thì **không** gửi gói nào)
//!
//! | Điều kiện | Mã trả |
//! |---|---|
//! | `Reply.caller` **không phải** TCB net server đã cấp cap | `InsufficientRights` (3) |
//! | `label != 0x291` hoặc `length != 1` | `InvalidArgument` (5) |
//! | `MR0` ngoài **chính sách IP** (gateway hoặc cùng `/24` với địa chỉ DHCP) | `InvalidArgument` (5) |
//! | driver lỗi (`Timeout`/`Io`/…) | `Timeout` (16) — giữ nguyên ngữ nghĩa KT-281 |
//!
//! Đột biến M1 (bỏ nhánh `label != 0x291`) làm ca âm trong cùng boot **đỏ**: request label sai
//! nhận `reply=0` thay vì `5`, và `sent` tăng lên 2 (đã gửi gói cho một request không hợp lệ).

use vifast_core::{KError, Machine, Message, MessageInfo, ObjId, SlotRef};
use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_plat_riscv64::virtio_net;

/// Label request `NetPing(ip)` — hợp đồng giữa chương trình U-mode và dịch vụ kernel.
pub const LABEL_NETPING: u32 = 0x291;
/// Label **sai** dùng cho ca âm: dịch vụ phải trả `InvalidArgument` (5) và **không** gửi gói.
pub const LABEL_BAD: u32 = 0x999;

/// Kết quả một lượt phục vụ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Không có request nào trong hàng (không chặn).
    Empty,
    /// Đã ping **thật** và đã trả `(0, rtt_ms)`.
    Served,
    /// Từ chối (caller/label/length/IP sai) — **không** gửi gói, đã trả mã lỗi.
    Denied,
    /// Đã kiểm quyền xong nhưng **driver** lỗi (timeout/Io) — đã trả `Timeout` (16).
    Failed,
    /// Lỗi core khi `recv`/`reply` (không phục vụ được).
    Error,
}

/// Dịch vụ `NetPing` của kernel: giữ cap `Endpoint`/`Reply` của TCB kernel + chính sách IP.
pub struct NetSvc {
    tcb_k: ObjId,
    ep: SlotRef,
    reply: SlotRef,
    /// Object `Reply` (để đọc `Reply.caller` — `reply_caller` nhận `ObjId`, không phải slot).
    reply_obj: ObjId,
    /// TCB **duy nhất** được phép gọi (net server U-mode) — kiểm qua `Reply.caller`.
    caller: ObjId,
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    gw: Ipv4Addr,
    gw_mac: MacAddr,
    /// Số request đã phục vụ **thật** (đã gửi gói).
    pub served: u64,
    /// Số request bị **từ chối** (không gửi gói nào).
    pub denied: u64,
    /// Số request đã kiểm quyền xong nhưng **driver** lỗi.
    pub failed: u64,
    /// `label` của request vừa xử lý (kernel đọc từ vùng nhận IPC — D2-83).
    pub last_label: u32,
    /// `length` của request vừa xử lý.
    pub last_length: u8,
    /// `MR0` (IP) của request vừa xử lý.
    pub last_ip: u64,
    /// `Reply.caller` của request vừa xử lý (`0` = không có).
    pub last_caller: u64,
    /// Mã lỗi đã trả cho request vừa xử lý (`0` = OK).
    pub last_err: u64,
    /// RTT (ms) của request vừa phục vụ.
    pub last_rtt_ms: u64,
    /// Số gói echo request **driver** đã gửi (đo từ `PingStats`, không phải hằng).
    pub sent: u64,
    /// Số echo reply **driver** đã nhận.
    pub received: u64,
}

impl NetSvc {
    /// Dựng dịch vụ. `caller` = TCB net server U-mode (đã được cấp cap `EP_KERN`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tcb_k: ObjId,
        ep: SlotRef,
        reply: SlotRef,
        reply_obj: ObjId,
        caller: ObjId,
        our_ip: Ipv4Addr,
        our_mac: MacAddr,
        gw: Ipv4Addr,
        gw_mac: MacAddr,
    ) -> Self {
        Self {
            tcb_k,
            ep,
            reply,
            reply_obj,
            caller,
            our_ip,
            our_mac,
            gw,
            gw_mac,
            served: 0,
            denied: 0,
            failed: 0,
            last_label: 0,
            last_length: 0,
            last_ip: 0,
            last_caller: 0,
            last_err: 0,
            last_rtt_ms: 0,
            sent: 0,
            received: 0,
        }
    }

    /// **Chính sách IP**: chỉ gateway hoặc host cùng `/24` với địa chỉ DHCP của ta. Không có
    /// chính sách này thì bất kỳ chương trình U-mode nào giữ cap cũng biến kernel thành máy
    /// phát gói ICMP tuỳ ý (khuếch đại / dò mạng).
    fn ip_allowed(&self, ip: u64) -> bool {
        let target = Ipv4Addr::from_u32_be(ip as u32);
        if target == self.gw {
            return true;
        }
        let a = target.octets();
        let b = self.our_ip.octets();
        a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] != 0 && a[3] != 255
    }

    /// Rút **một** request khỏi endpoint dịch vụ và xử lý (không chặn: `recv` trả ngay khi có
    /// message trong hàng, `Empty` khi không có ai gọi).
    pub fn service(&mut self, m: &mut Machine) -> Outcome {
        // 1) Nhận request. `recv` trên Endpoint có message ⇒ giao ngay; caller chuyển
        //    `BlockedOnSend{call}` → `BlockedOnReply` và `Reply.caller` = TCB gọi.
        if m.recv(self.tcb_k, self.ep, Some(self.reply)).is_err() {
            return Outcome::Error;
        }
        // 2) Đọc message **từ core** (D2-83: core sở hữu message; kernel không tin lời chương trình).
        let area = match m.tcb_ipc_area(self.tcb_k) {
            Ok(area) => area,
            Err(_) => return Outcome::Error,
        };
        let Some(info) = area.info else {
            return Outcome::Error;
        };
        self.last_label = info.label();
        self.last_length = info.length();
        self.last_ip = area.mr(0).unwrap_or(0);
        self.last_caller = m
            .reply_caller(self.reply_obj)
            .ok()
            .flatten()
            .map(|t| u64::from(t))
            .unwrap_or(0);

        // 3) **Kiểm quyền** — fail-closed: từ chối thì **không** chạm driver.
        let deny = if self.last_caller != u64::from(self.caller) {
            Some(KError::InsufficientRights)
        } else if self.last_label != LABEL_NETPING || self.last_length != 1 {
            Some(KError::InvalidArgument)
        } else if !self.ip_allowed(self.last_ip) {
            Some(KError::InvalidArgument)
        } else {
            None
        };
        if let Some(code) = deny {
            self.denied += 1;
            self.last_err = u64::from(code.id());
            self.last_rtt_ms = 0;
            let _ = m.reply_msg(
                self.tcb_k,
                self.reply,
                msg(self.last_label, &[u64::from(code.id())]),
            );
            return Outcome::Denied;
        }

        // 4) Gọi driver **thật** — chỗ **duy nhất** trong toàn ảnh chạm thiết bị mạng.
        let result = virtio_net::ping(
            self.our_ip,
            self.our_mac,
            Ipv4Addr::from_u32_be(self.last_ip as u32),
            self.gw_mac,
            1,
            virtio_net::NET_STEP_TIMEOUT_TICKS,
        );
        let (err, rtt_ms, sent, received) = match result {
            Ok(stats) => {
                let ok = stats.received == stats.sent && stats.sent >= 1;
                (
                    if ok {
                        0
                    } else {
                        u64::from(KError::Timeout.id())
                    },
                    stats.rtt_min_ms,
                    u64::from(stats.sent),
                    u64::from(stats.received),
                )
            }
            Err(_) => (u64::from(KError::Timeout.id()), 0, 0, 0),
        };
        self.sent += sent;
        self.received += received;
        self.last_err = err;
        self.last_rtt_ms = rtt_ms;
        if err == 0 {
            self.served += 1;
        } else {
            self.failed += 1;
        }
        // 5) Trả lời trên cap `Reply` — label **echo** label request để phía U-mode in được
        //    `label=0x…` từ chính `MessageInfo` của reply (đo, không phải hằng của bài test).
        let _ = m.reply_msg(
            self.tcb_k,
            self.reply,
            msg(self.last_label, &[if err == 0 { rtt_ms } else { 0 }]),
        );
        if err == 0 {
            Outcome::Served
        } else {
            Outcome::Failed
        }
    }
}

/// Message `label` + `words` — cùng khuôn `proc_rt::msg`.
fn msg(label: u32, words: &[u64]) -> Message {
    let info =
        MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("KT-306: MessageInfo");
    Message::new(info, words).expect("KT-306: Message")
}
