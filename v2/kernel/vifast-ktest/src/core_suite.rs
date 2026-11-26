// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bộ ca kiểm core dùng chung 3 arch** — TAP **43..52** (KT-411, `B5`, D2-259 mục 2).
//!
//! Trước KT-411 mỗi arch chép lại cùng một bộ khẳng định trên `vifast_core::Machine`:
//! `vifast-kbin-riscv64/src/ktest_core.rs` (1.090 LOC), `vifast-kbin-aarch64/src/coretest.rs`
//! (845), `vifast-kbin-x86_64/src/coretest.rs` (1.079). Tệp này giữ **một** bản duy nhất.
//!
//! # Vì sao generic được (và phần nào **không**)
//!
//! 8/10 ca (43..49, 52) chỉ chạm API của `vifast-core` — không một dòng nào nhắc CSR/asm/MMIO.
//! Hai ca còn lại cần **object layer** (Frame/PageTable/VSpace thuộc `vifast_core::vm`,
//! D2-83) và **dải RAM thật** của nền tảng; hai thứ đó đi qua [`CoreBridge`] nên vẫn dùng chung
//! được, chỉ khác phần cài đặt ở crate arch.
//!
//! # Bất biến giữ nguyên (không nới ngưỡng, không bỏ ca)
//!
//! Mỗi TAP giữ **đúng** điều kiện đo và **đúng** chuỗi `ok <n> - <tên>` của bản riscv64 gốc —
//! runner `v2/test/boot/ktest.sh` grep theo chuỗi đó, nên đổi nhãn là đổi kỳ vọng (AGENTS §4.2).
//! Dòng evidence `# core:` cũng giữ nguyên định dạng.

use core::fmt::Write;

use vifast_core::{
    KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbState, FOOTPRINT_PARTS,
};
use vifast_plat_common::{Platform, PlatformWriter};

use crate::{CoreBridge, LinkReport};

/// MR của vòng `Call` (hằng để TAP grep được giá trị cụ thể).
pub const IPC_MR: u64 = 0xfeed;
/// Giá trị futex của TAP 50.
pub const WA_VALUE: u64 = 0x1234;
/// Offset futex của TAP 50 (byte trong Frame).
pub const WA_OFFSET: u32 = 64;

/// Một dòng TAP — kiểu **dùng chung** thay cho chuỗi viết tay ở từng arch.
///
/// `ok_line`/`not_ok_line` là **nguồn duy nhất** của định dạng TAP, nên ba arch không thể trôi
/// nhãn của nhau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tap {
    /// Số TAP (43..52).
    pub number: u32,
    /// Nhãn sau ` - ` (runner grep đúng chuỗi này).
    pub name: &'static str,
}

impl Tap {
    /// Tạo một dòng TAP.
    pub const fn new(number: u32, name: &'static str) -> Self {
        Self { number, name }
    }

    /// Dòng ĐẠT (`ok <n> - <tên>\n`).
    pub fn ok_line(&self) -> TapLine {
        TapLine {
            tap: *self,
            ok: true,
            reason: "",
        }
    }

    /// Dòng ĐỎ (`not ok <n> - <tên>: <lý do>\n`).
    pub fn not_ok_line(&self, reason: &'static str) -> TapLine {
        TapLine {
            tap: *self,
            ok: false,
            reason,
        }
    }
}

/// Dòng TAP đã dựng — in được bằng [`TapLine::emit`] hoặc `format!` (host test).
#[derive(Clone, Copy, Debug)]
pub struct TapLine {
    tap: Tap,
    ok: bool,
    reason: &'static str,
}

impl TapLine {
    /// In dòng TAP ra console của nền tảng.
    pub fn emit<P: Platform + ?Sized>(&self, plat: &P) {
        let mut w = PlatformWriter(plat);
        let _ = write!(w, "{self}");
    }
}

impl core::fmt::Display for TapLine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.ok {
            writeln!(f, "ok {} - {}", self.tap.number, self.tap.name)
        } else {
            writeln!(
                f,
                "not ok {} - {}: {}",
                self.tap.number, self.tap.name, self.reason
            )
        }
    }
}

/// Kết quả một lượt chạy bộ ca chung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Số TAP ĐỎ.
    pub fails: usize,
    /// Số TAP đã phát.
    pub taps: usize,
}

/// Cửa đưa TCB về `Ready` — **do ảnh kiểm truyền vào**, không cài ở đây.
///
/// Vì sao là tham số chứ không phải hàm trong crate này: `wake()` là API **CHỈ-DÙNG-CHO-TEST**
/// (D2-144) và luật `E_STRICT_TEST_ONLY_WAKE` chỉ miễn tệp **test-only theo đồ thị module** —
/// tức tệp chỉ tới được từ bin có `required-features` chứa `ktest` (KT-227 P0-4: `#[cfg(feature =
/// "ktest")]` **không** còn được miễn). Crate thư viện dùng chung không thoả điều kiện đó, nên
/// `wake()` phải nằm ở `vifast-kbin-*/src/ktest*.rs` và được truyền vào qua con trỏ hàm này.
pub type ReadyFn = fn(&mut Machine, ObjId) -> Result<(), KError>;

/// Tên trạng thái TCB ổn định để in evidence (dùng chung 3 arch).
pub fn state_name(s: TcbState) -> &'static str {
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

/// Tên mã lỗi hoặc `ok` cho evidence của TAP 51.
pub fn err_name<T>(r: &Result<T, KError>) -> &'static str {
    match r {
        Ok(_) => "ok",
        Err(e) => e.name(),
    }
}

/// Tên mã lỗi hoặc `ok` cho `Option<KError>` (evidence của TAP 56..60).
pub fn err_name_opt(e: Option<KError>) -> &'static str {
    match e {
        None => "ok",
        Some(e) => e.name(),
    }
}

/// Bộ định danh dựng từ Untyped cho 10 TAP core — **dùng chung** (trước đây mỗi arch một bản).
pub struct Rig {
    /// CNode gốc.
    pub root: ObjId,
    /// Ô cap Untyped.
    pub us: SlotRef,
    /// Ô cap TCB 1.
    pub t0s: SlotRef,
    /// Ô cap TCB 2.
    pub t1s: SlotRef,
    /// Ô cap TCB 3.
    pub t2s: SlotRef,
    /// Ô cap Endpoint.
    pub eps: SlotRef,
    /// Ô cap Reply.
    pub rs: SlotRef,
    /// Ô cap Notification.
    pub ns: SlotRef,
    /// Ô cap SchedControl.
    pub ctl: SlotRef,
    /// Ô cap SchedContext 1.
    pub sc: SlotRef,
    /// Ô cap SchedContext 2.
    pub sc1: SlotRef,
    /// Ô cap CSpace.
    pub cspace: SlotRef,
    /// TCB 1.
    pub t0: ObjId,
    /// TCB 2.
    pub t1: ObjId,
    /// TCB 3.
    pub t2: ObjId,
    /// Endpoint.
    pub ep: ObjId,
    /// Reply.
    pub reply: ObjId,
    /// Notification.
    pub ntfn: ObjId,
    /// SchedContext 1.
    pub sc_obj: ObjId,
    /// SchedContext 2.
    pub sc1_obj: ObjId,
}

/// Dựng CNode gốc + 9 object lõi từ **Untyped của core** (harness cấp, như `build()` parity
/// KT-60). Lỗi ở bước này là lỗi model ⇒ panic có chuỗi ổn định để runner đỏ.
pub fn build_rig(m: &mut Machine, u: ObjId) -> Rig {
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

/// Chạy **toàn bộ** bộ ca core dùng chung (TAP 43..52) trên nền tảng `plat` + cầu nối `bridge`.
///
/// Trả [`Outcome`] — số TAP đỏ và số TAP đã phát. Ảnh kernel cộng `fails` vào bộ đếm chung của
/// nó; host test đòi `fails == 0 && taps == 10`.
///
/// Ảnh kernel **đã** giữ khoá `Machine` cho các TAP 53..60 của riêng nó thì dùng
/// [`run_locked`] thay vì hàm này (khoá không re-entrant).
pub fn run<P, B>(plat: &P, bridge: &B, ready: ReadyFn) -> Outcome
where
    P: Platform + ?Sized,
    B: CoreBridge + ?Sized,
{
    let link = bridge.link();
    let mut fails = 0usize;
    let mut taps = 0usize;
    taps += 1;
    if !link_tap(plat, &link) {
        fails += 1;
    }
    let (f, t, _rig) = bridge.with_machine(|m| run_locked(plat, bridge, m, &link, ready));
    Outcome {
        fails: fails + f,
        taps: taps + t,
    }
}

/// TAP **43** — tách khỏi [`run_locked`] vì nó **không** cần khoá `Machine` (chỉ đọc báo cáo
/// link). Ảnh kernel gọi hàm này trước khi lấy khoá cho TAP 44..60.
pub fn link_tap<P: Platform + ?Sized>(plat: &P, link: &LinkReport) -> bool {
    tap43_link(plat, link)
}

/// TAP **44..52** — chạy **trong** khoá `Machine` do caller giữ (không re-entrant).
///
/// Trả `(số đỏ, số TAP, Rig)`; `Rig` được trả để ảnh kernel dùng tiếp cho các TAP riêng của nó
/// (53..60) trên **cùng** CNode gốc — đúng thứ tự cũ, không dựng lại bộ định danh.
pub fn run_locked<P, B>(
    plat: &P,
    bridge: &B,
    m: &mut Machine,
    link: &LinkReport,
    ready: ReadyFn,
) -> (usize, usize, Rig)
where
    P: Platform + ?Sized,
    B: CoreBridge + ?Sized,
{
    core_taps(plat, bridge, m, link, ready)
}

/// TAP **43**: `Machine` static trong ảnh + bảng phân bố byte (D2-84/D2-85c).
fn tap43_link<P: Platform + ?Sized>(plat: &P, link: &LinkReport) -> bool {
    let f = link.footprint;
    let mut sum = 0usize;
    for part in f.bytes.iter().take(FOOTPRINT_PARTS) {
        sum += *part;
    }
    let sum_ok = sum + f.scalars + f.padding == f.total;
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# core: link addr=0x{:x} bytes={} pin={} image_cap={} parts={} sum={} scalars={} padding={} sum_ok={} in_image={}",
            link.addr,
            link.static_bytes,
            link.pin,
            crate::IMAGE_CAP_BYTES,
            FOOTPRINT_PARTS,
            f.total,
            f.scalars,
            f.padding,
            sum_ok as u8,
            link.in_image as u8
        );
    }
    let ok = sum_ok
        && f.padding <= 8
        && link.static_bytes == f.total
        && link.static_bytes <= link.pin
        && link.in_image;
    let tap = Tap::new(43, "core link");
    if ok {
        tap.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 43 - core link: sum_ok={} padding={} static={} pin={} in_image={}",
            sum_ok as u8, f.padding, link.static_bytes, link.pin, link.in_image as u8
        );
    }
    ok
}

/// TAP **44..52** — toàn bộ chạy trong **một** lượt giữ khoá `Machine` (không re-entrant).
fn core_taps<P, B>(
    plat: &P,
    bridge: &B,
    m: &mut Machine,
    link: &LinkReport,
    ready: ReadyFn,
) -> (usize, usize, Rig)
where
    P: Platform + ?Sized,
    B: CoreBridge + ?Sized,
{
    let mut fails = 0usize;
    let mut taps = 0usize;
    let u = link.untyped;

    // ------------------------------------------------------------ TAP 44: Untyped boot
    taps += 1;
    let size = m.untyped_size(u);
    let used = m.untyped_used(u);
    let free = m.untyped_free(u);
    let allocated = m.untyped_allocated(u);
    let gap = m.untyped_gap(u);
    let check_bytes = m.check_bytes(u).is_ok();
    let check_all = m.check_all_bytes().is_ok();
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# core: untyped id={} base=0x{:x} bytes={} used={} free={} allocated={} gap={} objects={} caps={} check_bytes={} check_all_bytes={}",
            u,
            link.base,
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
    let ok44 = size == link.arena_bytes
        && used == 0
        && free == size
        && allocated == 0
        && gap == 0
        && check_bytes
        && check_all;
    let tap44 = Tap::new(44, "core untyped boot");
    if ok44 {
        tap44.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(w, "not ok 44 - core untyped boot: byte/check khong khop");
        fails += 1;
    }

    let rig = build_rig(m, u);

    // ------------------------------------------------------------ TAP 45: Retype TCB
    taps += 1;
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
        let mut w = PlatformWriter(plat);
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
    let tap45 = Tap::new(45, "core tcb retype");
    if ok45 {
        tap45.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 45 - core tcb retype: state={} caps={} used={}",
            state_name(t0_state),
            caps45,
            used45
        );
        fails += 1;
    }

    // ------------------------------------- TAP 46: Configure + SetSchedParams + Resume/Suspend
    taps += 1;
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
        let mut w = PlatformWriter(plat);
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
    let tap46 = Tap::new(46, "core configure resume suspend");
    if ok46 {
        tap46.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 46 - core configure resume suspend: state={} fields={}",
            state_name(state46),
            ok46_fields as u8
        );
        fails += 1;
    }

    // -------------------------------------------------- TAP 47: schedule 2 TCB + suspend
    taps += 1;
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
        let mut w = PlatformWriter(plat);
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
    let tap47 = Tap::new(47, "core sched 2 tcb");
    if ok47 {
        tap47.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
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
        fails += 1;
    }

    // ------------------------------------------------------------ TAP 48: IPC một vòng
    taps += 1;
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
        let mut w = PlatformWriter(plat);
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
    let tap48 = Tap::new(48, "core ipc round");
    if ok48 {
        tap48.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 48 - core ipc round: send={} reply={} mr={:?} state={} ep_len={}",
            blocked_send as u8,
            blocked_reply as u8,
            mr,
            state_name(st_after),
            ep_len
        );
        fails += 1;
    }

    // ------------------------------------------------------ TAP 49: Notification signal
    taps += 1;
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
        let mut w = PlatformWriter(plat);
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
    let tap49 = Tap::new(49, "core notification signal");
    if ok49 {
        tap49.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 49 - core notification signal: word=0x{:x} badge=0x{:x} consumed={:?}",
            word,
            badge,
            consumed.map(|v| v.word)
        );
        fails += 1;
    }

    // ------------------------------------------------------------ TAP 50: WaitAddr
    taps += 1;
    // D2-83: Frame **thuộc object layer**; core chỉ nhận **nhãn** (PPN) làm khoá futex —
    // một chiều, không đọc byte/PTE của Frame. Đây là cầu nối duy nhất cho WaitAddr.
    let frame = bridge.obj_alloc_frame();
    let (frame_pa, label) = frame.map(|f| (f.pa, f.label)).unwrap_or((0, 0));
    // Bất biến **độc lập** (D2-66): PA của Frame phải nằm trong **cửa sổ PA** mà allocator
    // của Untyped đã cấp (watermark `frame_budget`, nguồn khác `frame_pa`) và trong dải RAM
    // thật của nền tảng; nhãn futex là PPN của đúng PA đó — kiểm **ngược**
    // `label << 12 == frame_pa` (không lấy lại `frame_pa >> 12` làm kỳ vọng).
    let (ram_base, ram_bytes) = bridge.ram_window();
    let pa_in_window = frame
        .map(|f| f.pa >= f.window_base && f.pa + (1 << 12) <= f.window_base + f.window_used)
        .unwrap_or(false);
    let pa_in_ram = frame_pa >= ram_base && frame_pa + (1 << 12) <= ram_base + ram_bytes;
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
        let mut w = PlatformWriter(plat);
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
    bridge.obj_free_frame(label);
    let tap50 = Tap::new(50, "core waitaddr");
    if ok50 {
        tap50.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 50 - core waitaddr: woke={} value=0x{:x} buckets={} state={}",
            woke,
            value,
            buckets,
            state_name(st50)
        );
        fails += 1;
    }

    // ----------------------------------------------------- TAP 51: biên giới D2-83
    taps += 1;
    let used_before = m.untyped_used(u);
    let f51 = m.retype(rig.us, ObjType::Frame, 0, 0);
    let pt51 = m.retype(rig.us, ObjType::PageTable, 0, 0);
    let vs51 = m.retype(rig.us, ObjType::VSpace, 0, 0);
    let used_after = m.untyped_used(u);
    let core_not_supported = [f51, pt51, vs51]
        .iter()
        .all(|r| matches!(r, Err(KError::NotSupported)));
    // Cùng lúc: object layer (chủ sở hữu VM) vẫn tạo/revoke VSpace + Frame **thật**.
    let vm = bridge.obj_vm_selftest();
    let obj_ok = vm.vspace && vm.frame && vm.health;
    let obj_clean = vm.clean;
    let ok51 = core_not_supported && used_after == used_before && obj_ok && obj_clean;
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# core: d2-83 frame={} page_table={} vspace={} atomic={} untyped_used={} obj_frame={} obj_vspace={} obj_health={} owner=obj",
            err_name(&f51),
            err_name(&pt51),
            err_name(&vs51),
            (used_after == used_before) as u8,
            used_after,
            vm.frames,
            vm.vspaces,
            obj_clean as u8
        );
    }
    let tap51 = Tap::new(51, "core d2-83 boundary");
    if ok51 {
        tap51.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 51 - core d2-83 boundary: frame={} pt={} vspace={} atomic={} obj_ok={}",
            err_name(&f51),
            err_name(&pt51),
            err_name(&vs51),
            (used_after == used_before) as u8,
            obj_ok as u8
        );
        fails += 1;
    }

    // ---------------------------------------------------------- TAP 52: invariants
    taps += 1;
    // Đưa mọi TCB về Ready trước khi revoke (Running→Revoke sẽ làm current treo).
    let _ = ready(m, rig.t1);
    let _ = m.delete(rig.cspace);
    let deleted = m.revoke(rig.root, Some(0)).unwrap_or(0);
    let used52 = m.untyped_used(u);
    let free52 = m.untyped_free(u);
    let gap52 = m.untyped_gap(u);
    let caps52 = m.live_cap_count();
    let inv = crate::invariants::check_all(m, u);
    let ok52 = used52 == 0
        && free52 == link.arena_bytes
        && gap52 == 0
        && caps52 == 1
        && deleted >= 9
        && inv.all_ok();
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# core: invariants caps={} objects={} used={} free={} deleted={} check_caps={} check_bytes={} check_all_bytes={} check_sched={} check_sc_refs={} check_budget={} check_mailboxes={} check_ipc={} check_reply_links={} check_notifications={} check_waitaddr={} check_fault={} leak={}",
            caps52,
            m.object_count(),
            used52,
            free52,
            deleted,
            inv.caps as u8,
            inv.bytes as u8,
            inv.all_bytes as u8,
            inv.sched as u8,
            inv.sc_refs as u8,
            inv.budget as u8,
            inv.mailboxes as u8,
            inv.ipc as u8,
            inv.reply_links as u8,
            inv.notifications as u8,
            inv.waitaddr as u8,
            inv.fault as u8,
            (!(used52 == 0 && gap52 == 0)) as u8
        );
    }
    let tap52 = Tap::new(52, "core invariants");
    if ok52 {
        tap52.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 52 - core invariants: used={} free={} gap={} caps={} deleted={} inv={}",
            used52,
            free52,
            gap52,
            caps52,
            deleted,
            inv.all_ok() as u8
        );
        fails += 1;
    }

    (fails, taps, rig)
}
