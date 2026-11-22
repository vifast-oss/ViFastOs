// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342** — mạng của ảnh **SẢN PHẨM** riscv64 (`product-net`): mở PCI/virtio-net + DHCP +
//! ARP **ở S-mode** trước khi hạ U-mode, dựng **cnode riêng** cho `netsrv` U-mode, và cho
//! `proc_rt` biết khi nào có request đang chờ trên `EP_NET_KERN` để gọi `net_svc::NetSvc`.
//!
//! # Vì sao tệp này tồn tại (không nhét vào `proc_rt`)
//!
//! `proc_rt` giữ vòng round-robin + dịch vụ FS; phần **dựng vật** của mạng (PCI ECAM, BAR, DMA,
//! DHCP, ARP, cnode/cap của server) là một miền riêng — tách để `proc_rt` giữ dưới trần R1 và để
//! ca âm "ảnh `product-all` boot không có `-netdev`" có **một** chỗ fail-closed (`open`).
//!
//! # Không có harness ở đây (D2-08)
//!
//! Tệp này **không** in TAP, không đếm `ok`, không gọi `probe`: nó chỉ mở thiết bị, dựng cap và
//! trả **số đo** cho `proc_rt` in một dòng evidence `# product net: …`. Cùng khuôn `proc_fs`.

use core::fmt::Write;

use vifast_core::{Machine, ObjId, ObjType, SlotRef};
use vifast_net::addr::MacAddr;
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::virtio_net::{self, NetConfig4};
use vifast_plat_riscv64::{pci, Riscv64Virt};
use vifast_virtio::pci::VirtioKind;

use crate::proc_abi::{
    EP_CTL, EP_FAULT, EP_NET, EP_NET_KERN, EP_NET_KERN_REPLY, EP_NET_REPLY, SC_SLOTS, TCB_SLOTS,
};

/// Thông số mạng đã đo được lúc mở (S-mode), giữ để in evidence + dựng `NetSvc`.
pub struct NetSetup {
    /// Địa chỉ/DHCP đã cấp.
    pub config: NetConfig4,
    /// MAC thật của thiết bị.
    pub mac: MacAddr,
    /// MAC gateway (ARP thật).
    pub gw_mac: MacAddr,
}

/// Mở thiết bị mạng của ảnh sản phẩm. Mọi lỗi ⇒ `Err` **có tên** để `proc_rt` PANIC (fail-closed,
/// không chạy tiếp với mạng dở); ca âm "không `-netdev`" dừng ở đây.
pub fn open() -> Result<NetSetup, &'static str> {
    let report = pci::init_for(VirtioKind::Net).map_err(|_| "pci net: khong tim thay thiet bi")?;
    virtio_net::init(&report).map_err(|_| "net: init driver that bai")?;
    let mac = MacAddr::new(virtio_net::mac());
    let config = virtio_net::dhcp_acquire(virtio_net::NET_STEP_TIMEOUT_TICKS)
        .map_err(|_| "net: dhcp khong cap duoc dia chi")?;
    let gw_mac = virtio_net::arp_resolve(config.ip, config.gw, virtio_net::NET_STEP_TIMEOUT_TICKS)
        .map_err(|_| "net: arp khong resolve duoc gateway")?;
    Ok(NetSetup {
        config,
        mac,
        gw_mac,
    })
}

/// In dòng evidence **một lần** lúc mở — số đo thật (MAC/DHCP), không phải hằng của bài test.
pub fn report(platform: &Riscv64Virt, setup: &NetSetup) {
    let mut w = PlatformWriter(platform);
    let _ = write!(w, "# product net: mac=");
    for (i, byte) in setup.mac.0.iter().enumerate() {
        if i > 0 {
            let _ = write!(w, ":");
        }
        let _ = write!(w, "{byte:02x}");
    }
    let _ = writeln!(
        w,
        " dhcp ip={} gw={} mask={} server={}",
        setup.config.ip, setup.config.gw, setup.config.mask, setup.config.server
    );
}

/// Cap + TCB của `netsrv` U-mode và endpoint dịch vụ kernel mà `proc_rt` dùng mỗi vòng.
pub struct NetRig {
    /// TCB core của `netsrv` (net server U-mode).
    pub tcb: SlotRef,
    /// Object TCB của `netsrv`.
    pub tcb_obj: ObjId,
    /// Endpoint `EP_NET_KERN` (kernel giữ) — nơi `NetSvc` nhận `Call`.
    pub ep_kern: SlotRef,
    /// Reply cap kernel dùng để trả lời `Call` của `netsrv`.
    pub reply_kern: SlotRef,
    /// Object Reply tương ứng (để `NetSvc` đọc `Reply.caller`).
    pub reply_kern_obj: ObjId,
    /// Object Endpoint `EP_NET_KERN` (để đếm request đang chờ, không chặn).
    pub ep_kern_obj: ObjId,
}

/// Dựng cnode 16 ô + TCB/SC của `netsrv`. Bố cục cnode (khớp `init/netsrv_prod_user.rs`):
/// `0` self, `1` `EP_NET`, `2` `EP_NET_REPLY`, `3` `EP_NET_KERN`, `4` `EP_FAULT`.
///
/// `root` là CNode gốc của ảnh sản phẩm (`proc_rt::build_rig` dựng); `us` là cap Untyped ở ô 0.
pub fn build(m: &mut Machine, root: ObjId, us: SlotRef) -> NetRig {
    // 1) Ba endpoint + một Reply của đường netsrv. Tạo **tuần tự** (không closure) để không giữ
    //    mượn `m` qua các bước sau.
    let ep_net_cap = m
        .retype(us, ObjType::Endpoint, 0, 0)
        .expect("KT-342: retype EP_NET");
    m.put_cap(root, EP_NET, ep_net_cap)
        .expect("KT-342: put EP_NET");
    let ep_reply_cap = m
        .retype(us, ObjType::Endpoint, 0, 0)
        .expect("KT-342: retype EP_NET_REPLY");
    m.put_cap(root, EP_NET_REPLY, ep_reply_cap)
        .expect("KT-342: put EP_NET_REPLY");
    let ep_kern_cap = m
        .retype(us, ObjType::Endpoint, 0, 0)
        .expect("KT-342: retype EP_NET_KERN");
    m.put_cap(root, EP_NET_KERN, ep_kern_cap)
        .expect("KT-342: put EP_NET_KERN");
    let ep_kern_obj = m.cap_obj(ep_kern_cap).expect("KT-342: ep_kern obj");
    let reply_cap = m
        .retype(us, ObjType::Reply, 0, 0)
        .expect("KT-342: retype net reply");
    m.put_cap(root, EP_NET_KERN_REPLY, reply_cap)
        .expect("KT-342: put net reply");
    let reply_obj = m.cap_obj(reply_cap).expect("KT-342: reply obj");

    // 2) CNode riêng của netsrv — server không dùng chung cspace với shell/procsrv.
    let cnode = m.add_cnode(16);
    let self_cap = m.root_cap(cnode);
    m.put_cap(cnode, 0, self_cap).expect("KT-342: self net");
    for (from, to) in [
        (EP_NET, 1usize),
        (EP_NET_REPLY, 2),
        (EP_NET_KERN, 3),
        (EP_FAULT, 4),
    ] {
        m.copy(SlotRef::new(root, from), SlotRef::new(cnode, to))
            .expect("KT-342: copy cap net");
    }

    // 3) TCB + SchedContext + SC của netsrv (ô 4 của bảng TCB/SC — ô 0..3 là procsrv/shell/
    //    victim/kernel, giữ nguyên bố cục `proc_abi`).
    let tcb_slot = TCB_SLOTS[4];
    let sc_slot = SC_SLOTS[4];
    let tcb_cap = m
        .retype(us, ObjType::Tcb, 0, 0)
        .expect("KT-342: retype tcb net");
    m.put_cap(root, tcb_slot, tcb_cap)
        .expect("KT-342: put tcb net");
    let sc_cap = m
        .retype(us, ObjType::SchedContext, 0, 0)
        .expect("KT-342: retype sc net");
    m.put_cap(root, sc_slot, sc_cap)
        .expect("KT-342: put sc net");
    let tcb = SlotRef::new(root, tcb_slot);
    let tcb_obj = m.cap_obj(tcb_cap).expect("KT-342: tcb obj net");
    m.tcb_configure(tcb, Some(SlotRef::new(cnode, 0)))
        .expect("KT-342: cspace net");
    m.tcb_set_fault_ep(tcb, SlotRef::new(cnode, 4))
        .expect("KT-342: fault ep net");
    m.sched_control_configure(
        SlotRef::new(root, EP_CTL),
        SlotRef::new(root, sc_slot),
        1_000,
        10_000,
    )
    .expect("KT-342: SC net");
    m.set_sched_params(tcb, Some(SlotRef::new(root, sc_slot)))
        .expect("KT-342: params net");
    // Ưu tiên **cao hơn** shell để sau `Reply` của kernel server được chạy tiếp ngay (cùng lý do
    // `PRIO_SERVER` của `ktest_netsrv`).
    m.tcb_set_priority(tcb, 6, 10)
        .expect("KT-342: priority net");
    m.start_tcb(tcb_obj).expect("KT-342: start net");

    NetRig {
        tcb,
        tcb_obj,
        ep_kern: SlotRef::new(root, EP_NET_KERN),
        reply_kern: SlotRef::new(root, EP_NET_KERN_REPLY),
        reply_kern_obj: reply_obj,
        ep_kern_obj,
    }
}

/// Số request đang chờ trên `EP_NET_KERN`. `NetSvc::service` dùng `recv` **chặn**, nên
/// `proc_rt` chỉ gọi khi hàm này trả `true` (cùng lý do `ktest_netsrv` gọi tuần tự).
pub fn pending(m: &mut Machine, ep_obj: ObjId) -> bool {
    matches!(m.endpoint_send_len(ep_obj), Ok(n) if n > 0)
}
