// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **core kernel thật** của ảnh test `vifast-kbin-ktest` (KT-61) — TAP **43..52**; **KT-63**
//! thêm TAP **53..55** (chuyển cap qua IPC + finalise IPC khi Revoke); trap lồng dời sang **56**.
//!
//! Mọi thao tác đi qua [`vifast_plat_riscv64::core_bridge::with_machine`] — tức `Machine` **static
//! trong `.bss` của ảnh kernel** (không phải model host, không phải sim). Bằng chứng `# core:`
//! in số object/cap/TCB **thật** của lần boot này + trạng thái các bất biến
//! (`check_caps`/`check_bytes`/`check_sched`/`check_ipc`/…).
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 43 - core link` | `Machine` là static trong **vùng ảnh** `[0x8020_0000, guard)`; `size_of::<Machine>()` ≤ pin D2-84 **196.608 B**; bảng phân bố 27 dòng khớp `sum + scalars + padding == total`, padding ≤ 8 (D2-85c) |
//! | `ok 44 - core untyped boot` | Untyped boot dựng từ **vùng nhãn PA** ngay sau arena object layer (D2-83: không dereference); byte `used=0 free=size`, `check_bytes`/`check_all_bytes` xanh |
//! | `ok 45 - core tcb retype` | `Retype` **từ Untyped**: 1 CNode gốc (32 slot) + 3 TCB + Endpoint + Reply + Notification + SchedControl + SchedContext; cap vào slot, TCB `Inactive`, `check_caps`/`check_bytes` xanh |
//! | `ok 46 - core configure resume suspend` | `Tcb.Configure` (cspace) + `SchedControl.Configure(4,8)` + `SetSchedParams` + `SetPriority(3,10)` + `Resume`/`Suspend(Abort)`; `suspended` đúng, priority/cspace/SC đọc lại khớp, `check_sched`/`check_sc_refs`/`check_budget` xanh |
//! | `ok 47 - core sched 2 tcb` | 2 TCB Ready khác priority: `sched_pick` chọn đúng; `dispatch` chạy TCB 1; `Suspend` TCB 1 ⇒ **không** còn trong hàng đợi ⇒ `dispatch` chuyển sang TCB 2; `Resume` đưa lại |
//! | `ok 48 - core ipc round` | `Call → Recv → Reply` **một vòng** trên Endpoint/Reply thật; MR tới đích, `BlockedOnSend{call}`→`BlockedOnReply`→`Ready`, `Reply.caller` sạch, `check_ipc`/`check_reply_links` xanh |
//! | `ok 49 - core notification signal` | bind + `Wait` (chặn) + `Signal(0x42)` (đánh thức, giao word/badge) + `Signal` không waiter rồi `Poll`; `check_notifications` xanh |
//! | `ok 50 - core waitaddr` | `WaitAddr(nhãn Frame, +64, 0)` chặn, `SetAddr`/`WakeAddr(1)` đánh thức; nhãn **không có byte** (D2-83), bucket rỗng, `check_waitaddr` xanh |
//! | `ok 51 - core d2-83 boundary` | `Retype` `Frame`/`PageTable`/`VSpace` trong core ⇒ **`NotSupported` 3/3**, `untyped_used` không đổi (atomic); **cùng lúc** object layer tạo/revoke VSpace+Frame thật ⇒ biên giới một chiều giữ nguyên |
//! | `ok 52 - core invariants` | Revoke cây Untyped ⇒ byte về 0 (`used=0 free=size gap=0`), cap còn đúng 1; **12/12** bất biến xanh, 0 rò |
//! | `ok 53 - core cap transfer` | 4 cap Endpoint→Recv qua 2 TCB: cap nhận **bỏ `RIGHT_GRANT`** (giữ badge), thiếu GRANT/ô bận ⇒ thất bại **mềm**, unwrap badge thành `Message.badge`; `check_ipc`/`check_caps` xanh |
//! | `ok 54 - core cap message` | message mang cap qua **Call/Reply**: caller `BlockedOnReply` nhận 2 cap vào receive window, caller `Ready`, `Reply.caller` sạch; `check_ipc`/`check_reply_links` xanh |
//! | `ok 55 - core revoke finalise` | `Revoke` cây Untyped khi có caller/Recv waiter/Notification waiter/hàng ⇒ caller `Ready + Cancelled`, cả hai waiter thức (`Ready + Cancelled`), hàng rỗng, `ipc_dropped=1`, phủ QEMU cho đường finalise IPC |
//!
//! Số đo do chính boot in (`# core:`); `ktest.sh` grep đúng các dòng đó — tài liệu không chép số
//! tay (QD §9 Q12). Xem `v2/docs/k3b-core.md` §9.

use core::fmt::Write;

use vifast_core::{
    CapId, KError, Machine, Message, ObjId, ObjType, SlotRef, SuspendMode, TcbState, RIGHTS_ALL,
    RIGHT_GRANT,
};
use vifast_plat_common::obj::ObjKind;
use vifast_plat_common::{Platform, PlatformWriter};

use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// MR của vòng `Call` (hằng để TAP grep được giá trị cụ thể).
const IPC_MR: u64 = 0xfeed;
/// Giá trị futex của TAP 50.
const WA_VALUE: u64 = 0x1234;
/// Offset futex của TAP 50 (byte trong Frame).
const WA_OFFSET: u32 = 64;

/// Bộ định danh dựng từ Untyped cho 10 TAP core.
pub(crate) struct Rig {
    pub(crate) root: ObjId,
    pub(crate) us: SlotRef,
    pub(crate) t0s: SlotRef,
    pub(crate) t1s: SlotRef,
    pub(crate) t2s: SlotRef,
    pub(crate) eps: SlotRef,
    pub(crate) rs: SlotRef,
    pub(crate) ns: SlotRef,
    pub(crate) ctl: SlotRef,
    pub(crate) sc: SlotRef,
    pub(crate) sc1: SlotRef,
    pub(crate) cspace: SlotRef,
    pub(crate) t0: ObjId,
    pub(crate) t1: ObjId,
    pub(crate) t2: ObjId,
    pub(crate) ep: ObjId,
    pub(crate) reply: ObjId,
    pub(crate) ntfn: ObjId,
    pub(crate) sc_obj: ObjId,
    pub(crate) sc1_obj: ObjId,
}

/// Tên trạng thái TCB ổn định để in evidence.
pub(crate) fn state_name(s: TcbState) -> &'static str {
    match s {
        TcbState::Inactive => "Inactive",
        TcbState::Ready => "Ready",
        TcbState::Running { .. } => "Running",
        TcbState::BlockedOnSend { .. } => "BlockedOnSend",
        TcbState::BlockedOnRecv { .. } => "BlockedOnRecv",
        TcbState::BlockedOnReply { .. } => "BlockedOnReply",
        TcbState::BlockedOnNotification { .. } => "BlockedOnNotification",
        TcbState::BlockedOnAddress { .. } => "BlockedOnAddress",
        TcbState::Sleeping { .. } => "Sleeping",
        TcbState::Stopping => "Stopping",
        TcbState::Dead => "Dead",
    }
}

/// `true` ⇔ mọi bất biến của core xanh (12 nhóm; gọi **sau** mỗi pha để bắt lệch sớm).
fn invariants_ok(m: &Machine) -> bool {
    m.check_caps().is_ok()
        && m.check_all_bytes().is_ok()
        && m.check_sched().is_ok()
        && m.check_sc_refs().is_ok()
        && m.check_budget().is_ok()
        && m.check_mailboxes().is_ok()
        && m.check_ipc().is_ok()
        && m.check_reply_links().is_ok()
        && m.check_notifications().is_ok()
        && m.check_waitaddr().is_ok()
        && m.check_fault().is_ok()
}

/// Đưa TCB về `Ready` qua **cửa** khớp trạng thái nguồn (test-only).
///
/// **D2-187/D2-194** — không còn cửa "đặt `Ready` vô điều kiện": `Inactive` ⇒ `start_tcb`,
/// `Running` ⇒ `tcb_yield`, `BlockedOn*`/`Sleeping` ⇒ `wake` (API **test-only**, D2-144).
fn task_ready(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    match m.tcb_state(tcb)? {
        TcbState::Inactive => m.start_tcb(tcb),
        TcbState::Ready => Ok(()),
        TcbState::Running { .. } => m.tcb_yield(tcb),
        _ => m.wake(tcb),
    }
}

/// Dựng CNode gốc + 8 object lõi từ **Untyped của core** (harness cấp, như `build()` parity
/// KT-60). Lỗi ở bước này là lỗi model ⇒ panic có chuỗi ổn định để runner đỏ.
fn build_rig(m: &mut Machine, u: ObjId) -> Rig {
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("core rig: cap Untyped");
    // 9 object lõi — **hai** SchedContext để cả 2 TCB có ngân sách chạy được
    // (`tcb_has_budget` chỉ đúng khi TCB có SC cấu hình — KT-20).
    let tys = [
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::Notification,
        ObjType::SchedControl,
        ObjType::SchedContext,
        ObjType::SchedContext,
    ];
    let mut obj = [0u32; 9];
    for (i, ty) in tys.iter().enumerate() {
        let cap = m
            .retype(us, *ty, 0, 0)
            .expect("core rig: retype object lõi");
        m.put_cap(root, 1 + i, cap)
            .expect("core rig: đặt cap object");
        obj[i] = m.cap_obj(cap).expect("core rig: cap_obj");
    }
    // Cap trỏ chính CNode gốc để làm CSpace của TCB (self-reference như sim).
    let rc = m.root_cap(root);
    m.put_cap(root, 10, rc).expect("core rig: cap cspace");
    Rig {
        root,
        us,
        t0s: SlotRef::new(root, 1),
        t1s: SlotRef::new(root, 2),
        t2s: SlotRef::new(root, 3),
        eps: SlotRef::new(root, 4),
        rs: SlotRef::new(root, 5),
        ns: SlotRef::new(root, 6),
        ctl: SlotRef::new(root, 7),
        sc: SlotRef::new(root, 8),
        sc1: SlotRef::new(root, 9),
        cspace: SlotRef::new(root, 10),
        t0: obj[0],
        t1: obj[1],
        t2: obj[2],
        ep: obj[3],
        reply: obj[4],
        ntfn: obj[5],
        sc_obj: obj[7],
        sc1_obj: obj[8],
    }
}

/// TAP **43**: `Machine` static trong ảnh + bảng phân bố byte (D2-84/D2-85c).
fn tap43_link(report: &core_bridge::CoreReport, fails: &mut usize) {
    let f = report.footprint;
    let mut sum = 0usize;
    for i in 0..vifast_core::FOOTPRINT_PARTS {
        sum += f.bytes[i];
    }
    let sum_ok = sum + f.scalars + f.padding == f.total;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# core: link addr=0x{:x} bytes={} pin={} image_cap={} parts={} sum={} scalars={} padding={} sum_ok={} in_image={}",
            report.addr,
            report.static_bytes,
            report.pin,
            core_bridge::IMAGE_CAP_BYTES,
            vifast_core::FOOTPRINT_PARTS,
            f.total,
            f.scalars,
            f.padding,
            sum_ok as u8,
            report.in_image as u8
        );
    }
    let ok = sum_ok
        && f.padding <= 8
        && report.static_bytes == f.total
        && report.static_bytes <= report.pin
        && report.in_image;
    if ok {
        PLATFORM.put_str("ok 43 - core link\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 43 - core link: sum_ok={} padding={} static={} pin={} in_image={}",
            sum_ok as u8, f.padding, report.static_bytes, report.pin, report.in_image as u8
        );
        *fails += 1;
    }
}

/// TAP **44..52** — toàn bộ chạy trong **một** lượt giữ khoá `Machine` (không re-entrant).
fn core_taps(
    report: &core_bridge::CoreReport,
    obj: &vifast_plat_riscv64::obj::ObjReport,
    fails: &mut usize,
) {
    core_bridge::with_machine(|m| {
        let u = report.untyped;
        // ------------------------------------------------------------ TAP 44: Untyped boot
        let size = m.untyped_size(u);
        let used = m.untyped_used(u);
        let free = m.untyped_free(u);
        let allocated = m.untyped_allocated(u);
        let gap = m.untyped_gap(u);
        let check_bytes = m.check_bytes(u).is_ok();
        let check_all = m.check_all_bytes().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: untyped id={} base=0x{:x} bytes={} used={} free={} allocated={} gap={} objects={} caps={} check_bytes={} check_all_bytes={}",
                u,
                report.base,
                size,
                used,
                free,
                allocated,
                gap,
                m.object_count(),
                m.live_cap_count(),
                check_bytes as u8,
                check_all as u8
            );
        }
        let ok44 = size == core_bridge::CORE_ARENA_BYTES
            && used == 0
            && free == size
            && allocated == 0
            && gap == 0
            && check_bytes
            && check_all;
        if ok44 {
            PLATFORM.put_str("ok 44 - core untyped boot\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "not ok 44 - core untyped boot: byte/check khong khop");
            *fails += 1;
        }

        let rig = build_rig(m, u);

        // ------------------------------------------------------------ TAP 45: Retype TCB
        let t0_state = m.tcb_state(rig.t0).unwrap_or(TcbState::Dead);
        let used45 = m.untyped_used(u);
        let caps45 = m.live_cap_count();
        let objects45 = m.object_count();
        let ok45 = t0_state == TcbState::Inactive
            && used45 > 0
            && caps45 == 11
            && m.check_caps().is_ok()
            && m.check_bytes(u).is_ok()
            && m.check_all_bytes().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: tcb retype tcb={} slot=1 objects={} caps={} used={} state={} check_caps={} check_bytes={}",
                rig.t0,
                objects45,
                caps45,
                used45,
                state_name(t0_state),
                m.check_caps().is_ok() as u8,
                m.check_bytes(u).is_ok() as u8
            );
        }
        if ok45 {
            PLATFORM.put_str("ok 45 - core tcb retype\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 45 - core tcb retype: state={} caps={} used={}",
                state_name(t0_state),
                caps45,
                used45
            );
            *fails += 1;
        }

        // ------------------------------------- TAP 46: Configure + SetSchedParams + Resume/Suspend
        let mut ok46 = m.tcb_configure(rig.t0s, Some(rig.cspace)).is_ok();
        ok46 &= m.sched_control_configure(rig.ctl, rig.sc, 4, 8).is_ok();
        ok46 &= m.set_sched_params(rig.t0s, Some(rig.sc)).is_ok();
        ok46 &= m.tcb_set_priority(rig.t0s, 3, 10).is_ok();
        ok46 &= m.start_tcb(rig.t0).is_ok();
        ok46 &= m.tcb_suspend(rig.t0s, SuspendMode::Abort).is_ok();
        let after_suspend = m.tcb_read_state(rig.t0s).ok();
        let suspended_ok = after_suspend.map(|s| s.suspended).unwrap_or(false);
        ok46 &= suspended_ok;
        ok46 &= m.tcb_resume(rig.t0s).is_ok();
        let info = m.tcb_info(rig.t0s).ok();
        let sc = m.sched_info(rig.sc).ok();
        let state46 = info.map(|i| i.state).unwrap_or(TcbState::Dead);
        let ok46_fields = info
            .map(|i| {
                !i.suspended
                    && i.state == TcbState::Ready
                    && i.priority == 3
                    && i.max_controlled_priority == 10
                    && i.cspace == Some(rig.root)
                    && i.sched_context == Some(rig.sc_obj)
            })
            .unwrap_or(false)
            && sc
                .map(|s| s.configured && s.budget == 4 && s.period == 8)
                .unwrap_or(false);
        ok46 &= ok46_fields
            && m.check_sched().is_ok()
            && m.check_sc_refs().is_ok()
            && m.check_budget().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: configure cspace=1 sc=1 priority=3 mcp=10 budget=4 period=8 suspended_ok={} resume=1 state={} check_sched={} check_sc_refs={} check_budget={}",
                suspended_ok as u8,
                state_name(state46),
                m.check_sched().is_ok() as u8,
                m.check_sc_refs().is_ok() as u8,
                m.check_budget().is_ok() as u8
            );
        }
        if ok46 {
            PLATFORM.put_str("ok 46 - core configure resume suspend\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 46 - core configure resume suspend: state={} fields={}",
                state_name(state46),
                ok46_fields as u8
            );
            *fails += 1;
        }

        // -------------------------------------------------- TAP 47: schedule 2 TCB + suspend
        // Cả hai TCB phải có SchedContext đã cấu hình — `tcb_has_budget` (KT-20) chỉ cho TCB
        // có ngân sách vào `pick`; đây là biên của mô hình, không phải mẹo test.
        let mut ok47 = m.sched_control_configure(rig.ctl, rig.sc1, 4, 8).is_ok();
        ok47 &= m.set_sched_params(rig.t1s, Some(rig.sc1)).is_ok();
        ok47 &= m.tcb_set_priority(rig.t1s, 2, 10).is_ok();
        ok47 &= m.start_tcb(rig.t1).is_ok();
        let pick0 = m.sched_pick_on(0);
        let d0 = m.dispatch(0).ok().flatten();
        ok47 &= pick0 == Some(rig.t0) && d0 == Some(rig.t0);
        ok47 &= m.tcb_state(rig.t0) == Ok(TcbState::Running { hart: 0 });
        ok47 &= m.tcb_state(rig.t1) == Ok(TcbState::Ready);
        ok47 &= m.tcb_suspend(rig.t0s, SuspendMode::Abort).is_ok();
        let pick1 = m.sched_pick_on(0);
        let d1 = m.dispatch(0).ok().flatten();
        ok47 &= pick1 == Some(rig.t1) && d1 == Some(rig.t1);
        ok47 &= m.tcb_state(rig.t1) == Ok(TcbState::Running { hart: 0 });
        // TCB 2 chạy bằng **SC riêng** đã cấu hình (không mượn SC của TCB 1).
        let info1 = m.tcb_info(rig.t1s).ok();
        ok47 &= info1
            .map(|i| i.sched_context == Some(rig.sc1_obj))
            .unwrap_or(false);
        ok47 &= m.tcb_resume(rig.t0s).is_ok();
        let info0 = m.tcb_info(rig.t0s).ok();
        let st0 = info0.map(|i| i.state).unwrap_or(TcbState::Dead);
        let sus0 = info0.map(|i| i.suspended).unwrap_or(true);
        let st1 = m.tcb_state(rig.t1).unwrap_or(TcbState::Dead);
        ok47 &= st0 == TcbState::Ready && !sus0 && st1 == TcbState::Running { hart: 0 };
        ok47 &= m.check_sched().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: sched harts={} pick0={} pick1={} dispatch0={} dispatch1={} t0={} t1={} suspended={} check_sched={}",
                m.hart_count(),
                pick0.unwrap_or(0),
                pick1.unwrap_or(0),
                d0.unwrap_or(0),
                d1.unwrap_or(0),
                state_name(st0),
                state_name(st1),
                sus0 as u8,
                m.check_sched().is_ok() as u8
            );
        }
        if ok47 {
            PLATFORM.put_str("ok 47 - core sched 2 tcb\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 47 - core sched 2 tcb: pick={:?}/{} dispatch={:?}/{} t0={} t1={}",
                pick0,
                rig.t0,
                d0,
                rig.t1,
                state_name(st0),
                state_name(st1)
            );
            *fails += 1;
        }

        // ------------------------------------------------------------ TAP 48: IPC một vòng
        let mut ok48 = m.call(rig.t0, rig.eps, IPC_MR).is_ok();
        let st_send = m.tcb_state(rig.t0).unwrap_or(TcbState::Dead);
        let blocked_send = matches!(
            st_send,
            TcbState::BlockedOnSend {
                ep,
                call: true,
                ..
            } if ep == rig.ep
        );
        ok48 &= blocked_send;
        ok48 &= m.recv(rig.t1, rig.eps, Some(rig.rs)).is_ok();
        let st_reply = m.tcb_state(rig.t0).unwrap_or(TcbState::Dead);
        let blocked_reply = st_reply == TcbState::BlockedOnReply { reply: rig.reply };
        ok48 &= blocked_reply;
        let mr = m.endpoint_last_mr(rig.ep).ok().flatten();
        ok48 &= mr == Some(IPC_MR);
        ok48 &= m.reply(rig.t1, rig.rs).is_ok();
        let st_after = m.tcb_state(rig.t0).unwrap_or(TcbState::Dead);
        let caller = m.reply_caller(rig.reply).ok().flatten();
        let ep_len = m.endpoint_send_len(rig.ep).unwrap_or(usize::MAX);
        let sent = m.ipc_sent();
        let delivered = m.ipc_delivered();
        ok48 &= st_after == TcbState::Ready && caller.is_none() && ep_len == 0;
        ok48 &= sent >= 1 && delivered >= 1;
        ok48 &= m.check_ipc().is_ok() && m.check_reply_links().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: ipc call=1 recv=1 reply=1 mr=0x{:x} blocked_send={} blocked_reply={} t0={} ep_len={} reply_caller={} sent={} delivered={} check_ipc={} check_reply_links={}",
                mr.unwrap_or(0),
                blocked_send as u8,
                blocked_reply as u8,
                state_name(st_after),
                ep_len,
                caller.map(|_| 1).unwrap_or(0),
                sent,
                delivered,
                m.check_ipc().is_ok() as u8,
                m.check_reply_links().is_ok() as u8
            );
        }
        if ok48 {
            PLATFORM.put_str("ok 48 - core ipc round\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 48 - core ipc round: send={} reply={} mr={:?} state={} ep_len={}",
                blocked_send as u8,
                blocked_reply as u8,
                mr,
                state_name(st_after),
                ep_len
            );
            *fails += 1;
        }

        // ------------------------------------------------------ TAP 49: Notification signal
        let mut ok49 = m.tcb_bind_notification(rig.t2s, rig.ns).is_ok();
        ok49 &= m.notification_wait(rig.t2, rig.ntfn).is_ok();
        ok49 &= m.tcb_state(rig.t2) == Ok(TcbState::BlockedOnNotification { ntfn: rig.ntfn });
        ok49 &= m.notification_signal(rig.ntfn, 0x42).is_ok();
        ok49 &= m.tcb_state(rig.t2) == Ok(TcbState::Ready);
        let consumed = m.take_tcb_notification(rig.t2).ok().flatten();
        ok49 &= consumed
            .map(|v| v.word == 0x42 && v.badge == 0x42)
            .unwrap_or(false);
        // Signal không có waiter ⇒ tích word/badge; Poll nhận rồi xoá cả hai.
        ok49 &= m.notification_signal(rig.ntfn, 0x8).is_ok();
        let polled = m.notification_poll(rig.ntfn).ok().flatten();
        let word = m.notification_word(rig.ntfn).unwrap_or(u64::MAX);
        let badge = m.notification_badge(rig.ntfn).unwrap_or(u64::MAX);
        let waiter = m.notification_waiter(rig.ntfn).ok().flatten();
        ok49 &= polled
            .map(|v| v.word == 0x8 && v.badge == 0x8)
            .unwrap_or(false);
        ok49 &= word == 0 && badge == 0 && waiter.is_none();
        ok49 &= m.check_notifications().is_ok() && m.check_ipc().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: ntfn signal=1 wake=1 word=0x{:x} badge=0x{:x} waiter={} consumed=0x{:x} poll=0x{:x} state={} check_notifications={}",
                word,
                badge,
                waiter.is_some() as u8,
                consumed.map(|v| v.word).unwrap_or(0),
                polled.map(|v| v.word).unwrap_or(0),
                state_name(m.tcb_state(rig.t2).unwrap_or(TcbState::Dead)),
                m.check_notifications().is_ok() as u8
            );
        }
        if ok49 {
            PLATFORM.put_str("ok 49 - core notification signal\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 49 - core notification signal: word=0x{:x} badge=0x{:x} consumed={:?}",
                word,
                badge,
                consumed.map(|v| v.word)
            );
            *fails += 1;
        }

        // ------------------------------------------------------------ TAP 50: WaitAddr
        // D2-83: Frame **thuộc object layer**; core chỉ nhận **nhãn** (PPN) làm khoá futex —
        // một chiều, không đọc byte/PTE của Frame. Đây là cầu nối duy nhất cho WaitAddr.
        let om = aobj::machine();
        let ofr = om
            .retype(obj.untyped0, ObjKind::Frame, 12)
            .expect("core TAP 50: obj layer Frame");
        let frame_pa = om.frame_pa(ofr).unwrap_or(0);
        let label = (frame_pa >> 12) as u32;
        // Bất biến **độc lập** (D2-66): PA của Frame phải nằm trong **cửa sổ PA** mà allocator
        // của Untyped đã cấp (watermark `frame_budget`, nguồn khác `frame_pa`) và trong dải RAM
        // thật của nền tảng (`mmu::RAM_PHYS_BASE..+RAM_BYTES`); nhãn futex là PPN của đúng PA
        // đó — kiểm **ngược** `label << 12 == frame_pa` (không lấy lại `frame_pa >> 12` làm kỳ
        // vọng). Nhờ vậy `frame_pa` sai (khác 0) không lọt: nhãn sẽ ra ngoài cửa sổ/RAM hoặc
        // lệch PA, thay vì chỉ khớp với chính nó như trước.
        let budget = om.frame_budget(obj.untyped0).unwrap_or_default();
        let pa_in_window =
            frame_pa >= budget.base && frame_pa + (1 << 12) <= budget.base + budget.used;
        let pa_in_ram = frame_pa >= mmu::RAM_PHYS_BASE
            && frame_pa + (1 << 12) <= mmu::RAM_PHYS_BASE + mmu::RAM_BYTES;
        let label_roundtrip = u64::from(label) << 12 == frame_pa;
        let mut ok50 = frame_pa != 0 && pa_in_window && pa_in_ram && label_roundtrip;
        ok50 &= m.wait_addr(rig.t2s, label, WA_OFFSET, 0).is_ok();
        ok50 &= m.tcb_state(rig.t2)
            == Ok(TcbState::BlockedOnAddress {
                frame: label,
                offset: WA_OFFSET,
            });
        ok50 &= m.set_addr_word(label, WA_OFFSET, WA_VALUE).is_ok();
        let woke = m.wake_addr(label, WA_OFFSET, 1).unwrap_or(u32::MAX);
        ok50 &= woke == 1;
        let value = m.addr_word(label, WA_OFFSET);
        let waiter = m.address_waiter(label, WA_OFFSET);
        let buckets = m.waitaddr_nonempty_buckets();
        let st50 = m.tcb_state(rig.t2).unwrap_or(TcbState::Dead);
        ok50 &= value == WA_VALUE && waiter.is_none() && buckets == 0;
        ok50 &= st50 == TcbState::Ready;
        ok50 &= m.check_waitaddr().is_ok() && m.check_sched().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: waitaddr frame=0x{:x} label={} offset={} value=0x{:x} woke={} waiters={} buckets={} state={} check_waitaddr={}",
                frame_pa,
                label,
                WA_OFFSET,
                value,
                woke,
                waiter.is_some() as u8,
                buckets,
                state_name(st50),
                m.check_waitaddr().is_ok() as u8
            );
        }
        let _ = om.revoke(ofr);
        if ok50 {
            PLATFORM.put_str("ok 50 - core waitaddr\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 50 - core waitaddr: woke={} value=0x{:x} buckets={} state={}",
                woke,
                value,
                buckets,
                state_name(st50)
            );
            *fails += 1;
        }

        // ----------------------------------------------------- TAP 51: biên giới D2-83
        let used_before = m.untyped_used(u);
        let f51 = m.retype(rig.us, ObjType::Frame, 0, 0);
        let pt51 = m.retype(rig.us, ObjType::PageTable, 0, 0);
        let vs51 = m.retype(rig.us, ObjType::VSpace, 0, 0);
        let used_after = m.untyped_used(u);
        let core_not_supported = [f51, pt51, vs51]
            .iter()
            .all(|r| matches!(r, Err(KError::NotSupported)));
        // Cùng lúc: object layer (chủ sở hữu VM) vẫn tạo/revoke VSpace + Frame **thật**.
        let obj = aobj::init();
        let om = aobj::machine();
        let ovs = om.retype(obj.untyped0, ObjKind::VSpace, 0);
        let ofr = om.retype(obj.untyped0, ObjKind::Frame, 12);
        let obj_ok = ovs.is_ok() && ofr.is_ok() && om.check_objmem().is_ok();
        let obj_counts = om.counts();
        if let Ok(cap) = ofr {
            let _ = om.revoke(cap);
        }
        if let Ok(cap) = ovs {
            let _ = om.revoke(cap);
        }
        let obj_clean = om.check_objmem().is_ok();
        let ok51 = core_not_supported && used_after == used_before && obj_ok && obj_clean;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: d2-83 frame={} page_table={} vspace={} atomic={} untyped_used={} obj_frame={} obj_vspace={} obj_health={} owner=obj",
                err_name(&f51),
                err_name(&pt51),
                err_name(&vs51),
                (used_after == used_before) as u8,
                used_after,
                obj_counts.frames,
                obj_counts.vspaces,
                obj_clean as u8
            );
        }
        if ok51 {
            PLATFORM.put_str("ok 51 - core d2-83 boundary\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 51 - core d2-83 boundary: frame={} pt={} vspace={} atomic={} obj_ok={}",
                err_name(&f51),
                err_name(&pt51),
                err_name(&vs51),
                (used_after == used_before) as u8,
                obj_ok as u8
            );
            *fails += 1;
        }

        // ---------------------------------------------------------- TAP 52: invariants
        // Đưa mọi TCB về Ready trước khi revoke (Running→Revoke sẽ làm current treo).
        let _ = task_ready(m, rig.t1);
        let _ = m.delete(rig.cspace);
        let deleted = m.revoke(rig.root, Some(0)).unwrap_or(0);
        let used52 = m.untyped_used(u);
        let free52 = m.untyped_free(u);
        let gap52 = m.untyped_gap(u);
        let caps52 = m.live_cap_count();
        let inv = invariants_ok(m);
        let ok52 = used52 == 0
            && free52 == core_bridge::CORE_ARENA_BYTES
            && gap52 == 0
            && caps52 == 1
            && deleted >= 9
            && inv;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# core: invariants caps={} objects={} used={} free={} deleted={} check_caps={} check_bytes={} check_all_bytes={} check_sched={} check_sc_refs={} check_budget={} check_mailboxes={} check_ipc={} check_reply_links={} check_notifications={} check_waitaddr={} check_fault={} leak={}",
                caps52,
                m.object_count(),
                used52,
                free52,
                deleted,
                m.check_caps().is_ok() as u8,
                m.check_bytes(u).is_ok() as u8,
                m.check_all_bytes().is_ok() as u8,
                m.check_sched().is_ok() as u8,
                m.check_sc_refs().is_ok() as u8,
                m.check_budget().is_ok() as u8,
                m.check_mailboxes().is_ok() as u8,
                m.check_ipc().is_ok() as u8,
                m.check_reply_links().is_ok() as u8,
                m.check_notifications().is_ok() as u8,
                m.check_waitaddr().is_ok() as u8,
                m.check_fault().is_ok() as u8,
                (!(used52 == 0 && gap52 == 0)) as u8
            );
        }
        if ok52 {
            PLATFORM.put_str("ok 52 - core invariants\n");
        } else {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 52 - core invariants: used={} free={} gap={} caps={} deleted={} inv={}",
                used52, free52, gap52, caps52, deleted, inv as u8
            );
            *fails += 1;
        }
        // ------------------------------------------------- TAP 53..55: chuyển cap + Revoke (KT-63)
        cap_ipc_taps(m, u, fails);
        // ------------------------------------------------- TAP 58..60: KT-64 (trong khoá core)
        // TAP 59 **ngoài** khoá: nó tự `with_machine` (lock không re-entrant) cho từng
        // `FlushRequest`, nên không được gọi khi đang giữ khoá core.
        crate::ktest_core64::slot_arena_tap(m, u, &rig, fails);
        crate::ktest_core64::bind_tap(m, u, fails);
        crate::ktest_core64::resume_tap(m, u, fails);
    });
}

/// Viết message mang `caps` (CapId) — dùng cho TAP 53..55.
fn msg_caps(label: u32, words: &[u64], caps: &[CapId]) -> Message {
    Message::with_cap_ids(label, words, caps).expect("kt63: message cap hợp lệ")
}

/// `true` ⇔ `n` ô window `base..base+n` đều có cap quyền **đã bỏ `RIGHT_GRANT`**.
fn window_rights_ok_n(m: &Machine, root: ObjId, base: usize, n: usize) -> bool {
    (0..n).all(|k| {
        m.cap_at(root, base + k)
            .ok()
            .and_then(|c| m.cap_info(c).ok())
            .map(|i| i.rights == (RIGHTS_ALL & !RIGHT_GRANT))
            .unwrap_or(false)
    })
}

/// TAP **53..55** (KT-63): chuyển cap Endpoint→Recv qua 2 TCB, message mang cap qua
/// Call/Reply, và `Revoke` finalise IPC (caller/waiter/hàng). Chạy trên `Machine` **thật**
/// của ảnh kernel; mọi số trong `# ipc:` là số đo của lần boot này.
fn cap_ipc_taps(m: &mut Machine, u: ObjId, fails: &mut usize) {
    let root = m.add_cnode(96);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("kt63: cap Untyped");
    let mk = |m: &mut Machine, ty: ObjType, slot: usize| -> ObjId {
        let cap = m.retype(us, ty, 0, 0).expect("kt63: retype");
        let obj = m.cap_obj(cap).expect("kt63: cap_obj");
        m.put_cap(root, slot, cap).expect("kt63: put cap");
        obj
    };
    let t0 = mk(m, ObjType::Tcb, 1);
    let t1 = mk(m, ObjType::Tcb, 2);
    let t2 = mk(m, ObjType::Tcb, 3);
    let t3 = mk(m, ObjType::Tcb, 4);
    let t4 = mk(m, ObjType::Tcb, 5);
    let _ep0 = mk(m, ObjType::Endpoint, 10);
    let _ep1 = mk(m, ObjType::Endpoint, 11);
    let _rep = mk(m, ObjType::Reply, 12);
    for t in [t0, t1, t2, t3, t4] {
        m.start_tcb(t).expect("kt63: Ready");
    }
    let win = |base: usize| {
        [
            SlotRef::new(root, base),
            SlotRef::new(root, base + 1),
            SlotRef::new(root, base + 2),
            SlotRef::new(root, base + 3),
        ]
    };
    let ep0s = SlotRef::new(root, 10);
    let ep1s = SlotRef::new(root, 11);
    let reps = SlotRef::new(root, 12);
    // 4 cap nguồn (Endpoint 2 — khác endpoint nhận ⇒ không unwrap), 1 cap thiếu GRANT,
    // 1 cap badge trên **chính** ep0 (unwrap khi gửi tới ep0).
    let mut src = [0u32; 4];
    for (i, slot) in [40usize, 41, 42, 43].iter().enumerate() {
        src[i] = m
            .copy(ep1s, SlotRef::new(root, *slot))
            .expect("kt63: copy cap nguồn");
    }
    let nogrant = m
        .copy_rights(
            SlotRef::new(root, 11),
            SlotRef::new(root, 44),
            Some(RIGHTS_ALL & !RIGHT_GRANT),
        )
        .expect("kt63: cap thiếu GRANT");
    let badge_cap = m
        .mint(ep0s, SlotRef::new(root, 45), RIGHTS_ALL, Some(0xA5A5))
        .expect("kt63: cap badge");

    // ---------------------------------------------------------------- TAP 53: chuyển 4 cap
    m.tcb_set_recv_window(SlotRef::new(root, 2), win(30))
        .expect("kt63: bind window t1");
    m.recv(t1, ep0s, Some(reps)).expect("kt63: Recv t1");
    let words = [0x11u64, 0x22];
    m.send(t0, ep0s, msg_caps(7, &words, &src))
        .expect("kt63: Send 4 cap");
    let info1 = m.tcb_ipc_info(t1).ok().flatten();
    let caps1 = info1.map(|i| i.caps()).unwrap_or(9);
    let len1 = info1.map(|i| i.length()).unwrap_or(0);
    let badge1 = m.tcb_ipc_badge(t1).unwrap_or(0);
    let rights_ok1 = window_rights_ok_n(m, root, 30, 4);
    // TCB t2 (window sạch): 1 cap thiếu GRANT ⇒ 3 cài, 1 thất bại mềm.
    m.tcb_set_recv_window(SlotRef::new(root, 3), win(34))
        .expect("kt63: bind window t2");
    m.recv(t2, ep0s, None).expect("kt63: Recv t2");
    m.send(
        t0,
        ep0s,
        msg_caps(8, &[0x33], &[src[0], nogrant, src[2], src[3]]),
    )
    .expect("kt63: Send 3+1 cap");
    let caps2 = m
        .tcb_ipc_info(t2)
        .ok()
        .flatten()
        .map(|i| i.caps())
        .unwrap_or(9);
    // TCB t3 (window sạch): unwrap badge ep0 + 1 cap thường ⇒ caps=1, extra=1.
    m.tcb_set_recv_window(SlotRef::new(root, 4), win(46))
        .expect("kt63: bind window t3");
    m.recv(t3, ep0s, None).expect("kt63: Recv t3");
    m.send(t0, ep0s, msg_caps(12, &[0xAB], &[badge_cap, src[1]]))
        .expect("kt63: Send unwrap");
    let info3 = m.tcb_ipc_info(t3).ok().flatten();
    let caps3 = info3.map(|i| i.caps()).unwrap_or(9);
    let extra3 = info3.map(|i| i.extra_caps_unwrapped()).unwrap_or(9);
    let badge3 = m.tcb_ipc_badge(t3).unwrap_or(0);
    let (installed, unwrapped, failed) = m.cap_transfer_stats();
    let rights_ok3 = m.cap_at(root, 46).is_err()
        && m.cap_at(root, 47)
            .ok()
            .and_then(|c| m.cap_info(c).ok())
            .map(|i| i.rights == (RIGHTS_ALL & !RIGHT_GRANT))
            .unwrap_or(false);
    let ok53 = caps1 == 4
        && len1 == 2
        && badge1 == 0
        && rights_ok1
        && caps2 == 3
        && caps3 == 1
        && extra3 == 1
        && badge3 == 0xA5A5
        && rights_ok3
        && installed == 8
        && failed == 1
        && unwrapped == 1
        && m.check_ipc().is_ok()
        && m.check_caps().is_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: cap-transfer installed={} failed={} unwrap={} caps4={} len={} rights=0x{:x} caps3={} caps1={} extra={} badge=0x{:x} check_ipc={} check_caps={}",
            installed,
            failed,
            unwrapped,
            caps1,
            len1,
            if rights_ok1 { RIGHTS_ALL & !RIGHT_GRANT } else { 0 },
            caps2,
            caps3,
            extra3,
            badge3,
            m.check_ipc().is_ok() as u8,
            m.check_caps().is_ok() as u8
        );
    }
    if ok53 {
        PLATFORM.put_str("ok 53 - core cap transfer\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 53 - core cap transfer: caps4={caps1} caps3={caps2} caps1={caps3} extra={extra3} badge=0x{badge3:x} inst={installed} fail={failed} unwrap={unwrapped}"
        );
        *fails += 1;
    }

    // ------------------------------------------------- TAP 54: message mang cap qua Call/Reply
    let t5 = mk(m, ObjType::Tcb, 6);
    m.start_tcb(t5).expect("kt63: Ready t5");
    m.tcb_set_recv_window(SlotRef::new(root, 6), win(50))
        .expect("kt63: bind window t5");
    m.tcb_set_recv_window(SlotRef::new(root, 1), win(54))
        .expect("kt63: bind window t0");
    m.recv(t5, ep1s, Some(reps)).expect("kt63: Recv t5");
    m.call_msg(t0, ep1s, msg_caps(20, &[0x55], &[src[0], src[1]]))
        .expect("kt63: Call 2 cap");
    let caller = m
        .reply_caller(m.cap_obj_at(root, 12).expect("kt63: rep obj"))
        .unwrap_or(None);
    let t5_caps = m
        .tcb_ipc_info(t5)
        .ok()
        .flatten()
        .map(|i| i.caps())
        .unwrap_or(9);
    let blocked = m.tcb_state(t0)
        == Ok(TcbState::BlockedOnReply {
            reply: m.cap_obj_at(root, 12).expect("kt63: rep obj"),
        });
    m.reply_msg(t5, reps, msg_caps(21, &[0x66], &[src[2], src[3]]))
        .expect("kt63: Reply 2 cap");
    let t0_caps = m
        .tcb_ipc_info(t0)
        .ok()
        .flatten()
        .map(|i| i.caps())
        .unwrap_or(9);
    let t0_ready = m.tcb_state(t0) == Ok(TcbState::Ready);
    let t0_rights = window_rights_ok_n(m, root, 54, 2);
    let reply_clear = m
        .reply_caller(m.cap_obj_at(root, 12).expect("kt63: rep obj"))
        .unwrap_or(None)
        .is_none();
    let ok54 = t5_caps == 2
        && t0_caps == 2
        && caller == Some(t0)
        && blocked
        && t0_ready
        && t0_rights
        && reply_clear
        && m.check_ipc().is_ok()
        && m.check_reply_links().is_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: cap-message direction=call-reply caller_caps=2 reply_caps=2 caller_bound={} caller_ready={} rights=0x{:x} reply_caller=0 check_ipc={} check_reply_links={}",
            blocked as u8,
            t0_ready as u8,
            if t0_rights { RIGHTS_ALL & !RIGHT_GRANT } else { 0 },
            m.check_ipc().is_ok() as u8,
            m.check_reply_links().is_ok() as u8
        );
    }
    if ok54 {
        PLATFORM.put_str("ok 54 - core cap message\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 54 - core cap message: recv_caps={t5_caps} caller_caps={t0_caps} bound={} ready={} caller={caller:?}",
            blocked as u8, t0_ready as u8
        );
        *fails += 1;
    }

    // --------------------------------------- TAP 55: Revoke finalise (caller/waiter/hàng)
    // Cây Untyped 2: epA/epB/epC/repA/ntnA — Revoke ⇒ caller Ready, Recv/Notif waiter thức,
    // hàng rỗng + `ipc_dropped` cộng đủ.
    let u2cap = m
        .retype(us, ObjType::Untyped, 12, 0)
        .expect("kt63: retype Untyped 2");
    m.put_cap(root, 20, u2cap).expect("kt63: put cap u2");
    let u2s = SlotRef::new(root, 20);
    let mk2 = |m: &mut Machine, ty: ObjType, slot: usize| -> ObjId {
        let cap = m.retype(u2s, ty, 0, 0).expect("kt63: retype cây 2");
        let obj = m.cap_obj(cap).expect("kt63: cap_obj cây 2");
        m.put_cap(root, slot, cap).expect("kt63: put cây 2");
        obj
    };
    let _ep_a = mk2(m, ObjType::Endpoint, 21);
    let ep_b = mk2(m, ObjType::Endpoint, 22);
    let _ep_c = mk2(m, ObjType::Endpoint, 23);
    let rep_a = mk2(m, ObjType::Reply, 24);
    let _ntn_a = mk2(m, ObjType::Notification, 25);
    let ep_as = SlotRef::new(root, 21);
    let ep_bs = SlotRef::new(root, 22);
    let ep_cs = SlotRef::new(root, 23);
    let rep_as = SlotRef::new(root, 24);
    let ntn_as = SlotRef::new(root, 25);
    let sent_truoc = m.ipc_sent();
    let dropped_truoc = m.ipc_dropped();
    // repA.caller = t0 qua Call epC (t4 là waiter cấp Reply — phải có receive window).
    m.tcb_set_recv_window(SlotRef::new(root, 5), win(58))
        .expect("kt63: bind window t4");
    m.recv(t4, ep_cs, Some(rep_as)).expect("kt63: Recv epC");
    m.call_msg(t0, ep_cs, msg_caps(30, &[0x77], &[src[0]]))
        .expect("kt63: Call epC");
    let caller55 = m.reply_caller(rep_a).unwrap_or(None) == Some(t0);
    // Recv waiter trên epA (t3) + hàng trên epB (t2) + Notif waiter trên ntnA (t4).
    m.recv(t3, ep_as, None).expect("kt63: Recv epA");
    m.send(t2, ep_bs, msg_caps(31, &[0x88], &[src[1]]))
        .expect("kt63: Send epB");
    let queued55 = m.endpoint_send_len(ep_b).unwrap_or(9);
    m.recv(t4, ntn_as, None).expect("kt63: Wait ntnA");
    let deleted55 = m.revoke(root, Some(20)).unwrap_or(0);
    let t0_free = m.tcb_state(t0) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t0) == Ok(Some(KError::Cancelled));
    let t3_free = m.tcb_state(t3) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t3) == Ok(Some(KError::Cancelled));
    let t4_free = m.tcb_state(t4) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t4) == Ok(Some(KError::Cancelled));
    // **KT-64a (D2-90)** — sender `BlockedOnSend` trên Endpoint bị finalise cũng phải
    // `Cancelled + Ready` (nhánh core trước KT-64a để **treo**; nay đóng nợ D2-90).
    let t2_free = m.tcb_state(t2) == Ok(TcbState::Ready)
        && m.tcb_last_ipc_error(t2) == Ok(Some(KError::Cancelled));
    let pending55 = m.ipc_pending();
    let dropped55 = m.ipc_dropped().saturating_sub(dropped_truoc);
    let sent55 = m.ipc_sent().saturating_sub(sent_truoc);
    let ok55 = caller55
        && queued55 == 1
        && deleted55 >= 5
        && t0_free
        && t2_free
        && t3_free
        && t4_free
        && pending55 == 0
        && dropped55 == 1
        && sent55 >= 2
        && m.check_ipc().is_ok()
        && m.check_reply_links().is_ok()
        && m.check_notifications().is_ok()
        && invariants_ok(m);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: revoke deleted={} caller=Ready caller_err=Cancelled recv_waiter=Ready notif_waiter=Ready sender=Ready sender_err=Cancelled queue={} queued={} pending={} sent={} dropped={} check_ipc={} check_reply_links={} check_notifications={}",
            deleted55,
            pending55,
            queued55,
            pending55,
            sent55,
            dropped55,
            m.check_ipc().is_ok() as u8,
            m.check_reply_links().is_ok() as u8,
            m.check_notifications().is_ok() as u8
        );
    }
    if ok55 {
        PLATFORM.put_str("ok 55 - core revoke finalise\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 55 - core revoke finalise: caller={caller55} t0={t0_free} t2={t2_free} t3={t3_free} t4={t4_free} pending={pending55} dropped={dropped55} deleted={deleted55}"
        );
        *fails += 1;
    }
}

/// Tên mã lỗi của `Option<KError>` (helper KT-64) hoặc `ok`.
pub(crate) fn err_name_opt(e: Option<KError>) -> &'static str {
    match e {
        None => "ok",
        Some(e) => e.name(),
    }
}

/// Tên mã lỗi hoặc `ok` cho evidence của TAP 51.
pub(crate) fn err_name<T>(r: &Result<T, KError>) -> &'static str {
    match r {
        Ok(_) => "ok",
        Err(e) => e.name(),
    }
}

/// Pha core: TAP **43..52**; trả số TAP đỏ.
pub fn core_phase() -> usize {
    let mut fails = 0usize;
    // D2-83: object layer **vẫn** là chủ sở hữu Frame/PageTable/VSpace; `init` cho report +
    // cap Untyped để TAP 51 chứng minh VM sống ở đó, không nhân đôi chủ sở hữu.
    let obj = aobj::init();
    let report = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    tap43_link(&report, &mut fails);
    core_taps(&report, &obj, &mut fails);
    // TAP 59 chạy **ngoài** khoá core (tự `with_machine` per `FlushRequest`), trước TAP 58..60
    // về số hiệu nhưng sau phần khoá — thứ tự `ok` là ...55, 56, 57, 58, 59, 60.
    crate::ktest_core64::tlb_tap(&mut fails);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# core: phase fails={} static={} pin={} image_cap={}",
            fails,
            report.static_bytes,
            report.pin,
            core_bridge::IMAGE_CAP_BYTES
        );
    }
    fails
}
