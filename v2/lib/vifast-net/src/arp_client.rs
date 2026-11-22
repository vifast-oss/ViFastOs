// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ARP resolve** dùng chung 3 arch (KT-335b) — dựng request, kiểm reply, chờ trong hạn.
//!
//! Trước KT-335b khối này nằm **ba bản** gần như y hệt trong
//! `vifast-plat-{riscv64,aarch64,x86_64}/src/virtio_net.rs` (riscv64 KT-246, aarch64 KT-275,
//! x86_64 KT-274): cùng codec `vifast_net::arp`, cùng điều kiện nhận, chỉ khác đồng hồ. Tách về
//! đây để **một** chỗ giữ bẫy ARP thật (xem [`arp_parse_reply`]).
//!
//! # Bất biến
//!
//! - **Một** request, chờ tới `timeout`; không reply ⇒ [`LinkError::Timeout`] (fail-closed).
//! - Reply phải khớp **cả bốn**: `oper=2`, `ar_spa = target_ip`, `ar_tpa = our_ip`,
//!   `ar_tha = our_mac`; MAC nguồn toàn 0/broadcast là giá trị vô nghĩa ⇒ không nhận.
//! - Frame không phải ARP/hỏng bị **bỏ qua** rồi chờ tiếp — không PANIC, không đoán.

use crate::addr::{Ipv4Addr, MacAddr};
use crate::arp::{ArpPacket, ARP_LEN};
use crate::client::{past_deadline, Clock, Link, LinkError, MAX_FRAME_BYTES};
use crate::eth::{EthHeader, ETHERTYPE_ARP};
use crate::frame::EthernetFrame;

/// Dựng **ARP request** (Ethernet/IPv4) hỏi MAC của `target_ip`; trả tổng số byte (42).
///
/// Chỉ là lớp ghép khung cho driver: codec ARP + Ethernet nằm ở [`crate::arp`]/[`crate::eth`].
pub fn arp_build_request(
    mac: MacAddr,
    source_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    out: &mut [u8],
) -> Option<usize> {
    let mut arp = [0u8; ARP_LEN];
    let arp_len = ArpPacket::request(mac, source_ip, target_ip)
        .encode(&mut arp)
        .ok()?;
    EthernetFrame {
        header: EthHeader::new(MacAddr::BROADCAST, mac, ETHERTYPE_ARP),
        payload: &arp[..arp_len],
    }
    .encode(out)
    .ok()
}

/// Kiểm một **ARP reply** cho `target_ip` gửi tới `our_ip`/`our_mac`; trả MAC người trả lời.
///
/// # Bẫy đã tự gặp (KT-246, giữ nguyên ở đây)
///
/// Trong **ARP reply**, `ar_tip` là địa chỉ của **người hỏi** (`our_ip`), **không** phải
/// `target_ip` — `ar_spa` mới là `target_ip`. Bản đầu kiểm `ar_tip == target_ip` nên **mọi**
/// reply hợp lệ đều bị từ chối và ARP timeout dù thiết bị đã trả lời. Ba arch đã dính đúng bẫy
/// này; test host (`vifast-plat-aarch64/tests/virtio_net.rs::arp_reply_uses_spa_not_tip`) khoá
/// hành vi đã sửa.
pub fn arp_parse_reply(
    frame: &[u8],
    our_ip: Ipv4Addr,
    target_ip: Ipv4Addr,
    our_mac: MacAddr,
) -> Option<MacAddr> {
    let eth = EthernetFrame::parse(frame).ok()?;
    if eth.header.ethertype != ETHERTYPE_ARP {
        return None;
    }
    let arp = ArpPacket::parse(eth.payload).ok()?;
    if !arp.is_reply() || arp.spa != target_ip || arp.tpa != our_ip || arp.tha != our_mac {
        return None;
    }
    // MAC toàn 0 / broadcast là giá trị vô nghĩa cho một host — không nhận.
    if arp.sha.is_zero() || arp.sha.is_broadcast() {
        return None;
    }
    Some(arp.sha)
}

/// **ARP resolve**: hỏi MAC của `target_ip` (dùng `our_ip`/`our_mac` làm nguồn).
///
/// `Err(LinkError::Timeout)` nếu không có reply trong `timeout` (cùng đơn vị [`Clock`]) — đường
/// "không có mạng" (fail-closed, không treo).
pub fn arp_resolve<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    timeout: u64,
) -> Result<MacAddr, LinkError> {
    let mut frame = [0u8; 64];
    let len =
        arp_build_request(our_mac, our_ip, target_ip, &mut frame).ok_or(LinkError::BadFrame)?;
    link.send_frame(&frame[..len])?;
    let deadline = clock.now().wrapping_add(timeout);
    let mut reply = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = clock.now();
        if past_deadline(now, deadline) {
            return Err(LinkError::Timeout);
        }
        let remaining = deadline.wrapping_sub(now);
        if let Some(got) = link.recv_frame(&mut reply, remaining) {
            if let Some(found) = arp_parse_reply(&reply[..got], our_ip, target_ip, our_mac) {
                return Ok(found);
            }
        }
    }
}
